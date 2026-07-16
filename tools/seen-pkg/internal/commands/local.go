package commands

import (
	"context"
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/codeyousef/seen/tools/seen-pkg/internal/atomicfile"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/manifest"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/model"
	"github.com/pelletier/go-toml/v2"
)

const (
	maxLocalViewEntries = 100_000
	maxLocalViewBytes   = int64(2 * 1024 * 1024 * 1024)
	maxLocalViewFile    = int64(512 * 1024 * 1024)
)

type localSnapshot struct {
	sourceRoot string
	viewRoot   string
	manifest   *model.Manifest
}

type localMaterializer struct {
	ctx         context.Context
	projectRoot string
	snapshots   map[string]localSnapshot
	expanded    map[string]bool
	rows        map[PackageMapRow]struct{}
}

// materializeLocalDependencies validates every local dependency before the
// authoritative package map is promoted. Path packages are copied into
// immutable project-local views, so a mixed hosted/local graph never points
// the compiler at mutable source outside the project package state.
func materializeLocalDependencies(ctx context.Context, projectRoot string, dependencies []model.Dependency) ([]PackageMapRow, error) {
	materializer := &localMaterializer{
		ctx:         ctx,
		projectRoot: projectRoot,
		snapshots:   map[string]localSnapshot{},
		expanded:    map[string]bool{},
		rows:        map[PackageMapRow]struct{}{},
	}
	for _, dependency := range dependencies {
		switch dependency.Kind {
		case model.DependencyPath:
			if _, err := materializer.addPathEdge(projectRoot, projectRoot, dependency); err != nil {
				return nil, fmt.Errorf("dependency %q: %w", dependency.Alias, err)
			}
		case model.DependencyArtifact:
			if _, err := validateArtifactDependency(projectRoot, dependency); err != nil {
				return nil, fmt.Errorf("dependency %q: %w", dependency.Alias, err)
			}
		case model.DependencyRegistry, model.DependencySystem:
			// Hosted packages are handled by the signed resolver. System paths
			// are linker configuration, not source-package import edges.
		default:
			return nil, fmt.Errorf("dependency %q has unsupported source kind %q", dependency.Alias, dependency.Kind)
		}
	}
	rows := make([]PackageMapRow, 0, len(materializer.rows))
	for row := range materializer.rows {
		rows = append(rows, row)
	}
	sort.Slice(rows, func(i, j int) bool {
		if rows[i].RequesterRoot != rows[j].RequesterRoot {
			return rows[i].RequesterRoot < rows[j].RequesterRoot
		}
		if rows[i].Alias != rows[j].Alias {
			return rows[i].Alias < rows[j].Alias
		}
		return rows[i].DependencyRoot < rows[j].DependencyRoot
	})
	return rows, nil
}

func (materializer *localMaterializer) addPathEdge(sourceBase, requesterView string, dependency model.Dependency) (string, error) {
	sourceRoot, err := resolveLocalDirectory(sourceBase, dependency.Path)
	if err != nil {
		return "", err
	}
	if sourceRoot == materializer.projectRoot {
		return "", fmt.Errorf("local path resolves to the requesting project itself")
	}
	if inside, _ := pathWithin(filepath.Join(materializer.projectRoot, ".seen"), sourceRoot); inside {
		return "", fmt.Errorf("local path must not resolve inside project package state")
	}
	snapshot, exists := materializer.snapshots[sourceRoot]
	if !exists {
		manifestPath := filepath.Join(sourceRoot, "Seen.toml")
		if err := requireNonemptyRegularFile(manifestPath); err != nil {
			return "", fmt.Errorf("path %s does not contain a usable Seen.toml: %w", sourceRoot, err)
		}
		parsed, err := manifest.Load(manifestPath, manifest.Options{DefaultRegistryOrigin: "https://seen.dev.yousef.codes/packages"})
		if err != nil {
			return "", fmt.Errorf("validate local package manifest: %w", err)
		}
		viewRoot, err := createLocalProjectView(materializer.ctx, materializer.projectRoot, sourceRoot)
		if err != nil {
			return "", err
		}
		snapshot = localSnapshot{sourceRoot: sourceRoot, viewRoot: viewRoot, manifest: parsed}
		materializer.snapshots[sourceRoot] = snapshot
	}
	materializer.rows[PackageMapRow{RequesterRoot: requesterView, Alias: dependency.Alias, DependencyRoot: snapshot.viewRoot}] = struct{}{}
	if materializer.expanded[sourceRoot] {
		return snapshot.viewRoot, nil
	}
	// Mark before descending so explicit local cycles produce a finite,
	// complete map rather than recursive copying.
	materializer.expanded[sourceRoot] = true
	for _, child := range snapshot.manifest.Dependencies {
		switch child.Kind {
		case model.DependencyPath:
			if _, err := materializer.addPathEdge(sourceRoot, snapshot.viewRoot, child); err != nil {
				return "", fmt.Errorf("local package %s dependency %q: %w", sourceRoot, child.Alias, err)
			}
		case model.DependencyArtifact:
			artifactRoot, err := validateArtifactDependency(sourceRoot, child)
			if err != nil {
				return "", fmt.Errorf("local package %s dependency %q: %w", sourceRoot, child.Alias, err)
			}
			inside, err := pathWithin(sourceRoot, artifactRoot)
			if err != nil || !inside {
				return "", fmt.Errorf("local package %s dependency %q: artifact must be contained in the local package tree so its immutable view remains self-contained", sourceRoot, child.Alias)
			}
			relativeArtifact, err := filepath.Rel(sourceRoot, artifactRoot)
			if err != nil {
				return "", err
			}
			viewDependency := child
			viewDependency.Artifact = relativeArtifact
			if _, err := validateArtifactDependency(snapshot.viewRoot, viewDependency); err != nil {
				return "", fmt.Errorf("local package %s dependency %q is not preserved by its immutable view: %w", sourceRoot, child.Alias, err)
			}
		case model.DependencyRegistry:
			return "", fmt.Errorf("local package %s dependency %q: hosted dependencies must be declared by the root project in Seen 0.10", sourceRoot, child.Alias)
		case model.DependencySystem:
			// System dependencies are not package-map import edges.
		default:
			return "", fmt.Errorf("local package %s dependency %q has unsupported source kind %q", sourceRoot, child.Alias, child.Kind)
		}
	}
	return snapshot.viewRoot, nil
}

func resolveLocalDirectory(base, value string) (string, error) {
	name := filepath.FromSlash(value)
	if !filepath.IsAbs(name) {
		name = filepath.Join(base, name)
	}
	absolute, err := filepath.Abs(name)
	if err != nil {
		return "", err
	}
	real, err := filepath.EvalSymlinks(absolute)
	if err != nil {
		return "", fmt.Errorf("resolve local directory %s: %w", absolute, err)
	}
	real, err = filepath.Abs(real)
	if err != nil {
		return "", err
	}
	info, err := os.Lstat(real)
	if err != nil {
		return "", err
	}
	if !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return "", fmt.Errorf("local path %s must resolve to a real directory", real)
	}
	return filepath.Clean(real), nil
}

func createLocalProjectView(ctx context.Context, projectRoot, sourceRoot string) (string, error) {
	stateRoot := filepath.Join(projectRoot, ".seen")
	if err := ensureRealDirectory(stateRoot, 0o700); err != nil {
		return "", err
	}
	viewsRoot := filepath.Join(stateRoot, "views")
	if err := ensureRealDirectory(viewsRoot, 0o700); err != nil {
		return "", err
	}
	localRoot := filepath.Join(viewsRoot, "local")
	if err := ensureRealDirectory(localRoot, 0o700); err != nil {
		return "", err
	}
	sourceSum := sha256.Sum256([]byte(sourceRoot))
	sourceDirectory := filepath.Join(localRoot, hex.EncodeToString(sourceSum[:]))
	if err := ensureRealDirectory(sourceDirectory, 0o700); err != nil {
		return "", err
	}
	stage, err := os.MkdirTemp(sourceDirectory, ".view-*")
	if err != nil {
		return "", err
	}
	committed := false
	defer func() {
		if !committed {
			makeLocalTreeWritable(stage)
			_ = os.RemoveAll(stage)
		}
	}()
	stageSource := filepath.Join(stage, "source")
	if err := copyLocalTree(ctx, sourceRoot, stageSource); err != nil {
		return "", err
	}
	if err := freezeLocalTree(stage); err != nil {
		return "", err
	}
	digest, err := digestLocalTree(ctx, stageSource, true)
	if err != nil {
		return "", err
	}
	final := filepath.Join(sourceDirectory, digest)
	finalSource := filepath.Join(final, "source")
	if existingDigest, verifyErr := digestLocalTree(ctx, finalSource, true); verifyErr == nil && existingDigest == digest {
		makeLocalTreeWritable(stage)
		if err := os.RemoveAll(stage); err != nil {
			return "", err
		}
		committed = true
		return finalSource, nil
	} else if !errors.Is(verifyErr, os.ErrNotExist) {
		quarantine := filepath.Join(sourceDirectory, fmt.Sprintf(".corrupt-%s-%d", digest, time.Now().UnixNano()))
		if err := os.Rename(final, quarantine); err != nil {
			return "", fmt.Errorf("quarantine corrupt local view: %w", err)
		}
	}
	if err := os.Rename(stage, final); err != nil {
		if existingDigest, verifyErr := digestLocalTree(ctx, finalSource, true); verifyErr == nil && existingDigest == digest {
			makeLocalTreeWritable(stage)
			_ = os.RemoveAll(stage)
			committed = true
			return finalSource, nil
		}
		return "", fmt.Errorf("promote local package view: %w", err)
	}
	committed = true
	if err := atomicfile.SyncDir(sourceDirectory); err != nil {
		return "", err
	}
	return finalSource, nil
}

func copyLocalTree(ctx context.Context, sourceRoot, destinationRoot string) error {
	if err := os.Mkdir(destinationRoot, 0o700); err != nil {
		return err
	}
	entries, total := 0, int64(0)
	return filepath.WalkDir(sourceRoot, func(name string, entry os.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if err := ctx.Err(); err != nil {
			return err
		}
		relative, err := filepath.Rel(sourceRoot, name)
		if err != nil {
			return err
		}
		if relative == "." {
			return nil
		}
		first := relative
		if separator := strings.IndexRune(relative, filepath.Separator); separator >= 0 {
			first = relative[:separator]
		}
		if first == ".git" || first == ".seen" {
			if entry.IsDir() {
				return filepath.SkipDir
			}
			return nil
		}
		entries++
		if entries > maxLocalViewEntries {
			return fmt.Errorf("local package exceeds %d entries", maxLocalViewEntries)
		}
		if entry.Type()&os.ModeSymlink != 0 {
			return fmt.Errorf("local package contains symbolic link %q", filepath.ToSlash(relative))
		}
		info, err := entry.Info()
		if err != nil {
			return err
		}
		destination := filepath.Join(destinationRoot, relative)
		if info.IsDir() {
			return os.Mkdir(destination, 0o700)
		}
		if !info.Mode().IsRegular() {
			return fmt.Errorf("local package contains unsupported file type %q", filepath.ToSlash(relative))
		}
		if info.Size() > maxLocalViewFile {
			return fmt.Errorf("local package file %q exceeds %d bytes", filepath.ToSlash(relative), maxLocalViewFile)
		}
		total += info.Size()
		if total > maxLocalViewBytes {
			return fmt.Errorf("local package exceeds %d bytes", maxLocalViewBytes)
		}
		input, err := os.Open(name)
		if err != nil {
			return err
		}
		opened, err := input.Stat()
		if err != nil || !opened.Mode().IsRegular() || !os.SameFile(info, opened) {
			_ = input.Close()
			return fmt.Errorf("local package changed while reading %q", filepath.ToSlash(relative))
		}
		output, err := os.OpenFile(destination, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
		if err != nil {
			_ = input.Close()
			return err
		}
		written, copyErr := copyLocalFile(ctx, output, io.LimitReader(input, info.Size()+1))
		closeErr := errors.Join(input.Close(), output.Sync(), output.Close())
		if copyErr != nil || closeErr != nil {
			return errors.Join(copyErr, closeErr)
		}
		if written != info.Size() {
			return fmt.Errorf("local package changed while copying %q", filepath.ToSlash(relative))
		}
		return nil
	})
}

func copyLocalFile(ctx context.Context, destination io.Writer, source io.Reader) (int64, error) {
	buffer := make([]byte, 128*1024)
	var written int64
	for {
		if err := ctx.Err(); err != nil {
			return written, err
		}
		count, readErr := source.Read(buffer)
		if count != 0 {
			n, writeErr := destination.Write(buffer[:count])
			written += int64(n)
			if writeErr != nil {
				return written, writeErr
			}
			if n != count {
				return written, io.ErrShortWrite
			}
		}
		if errors.Is(readErr, io.EOF) {
			return written, nil
		}
		if readErr != nil {
			return written, readErr
		}
	}
}

type localTreeEntry struct {
	relative  string
	directory bool
	size      int64
}

func digestLocalTree(ctx context.Context, root string, requireReadOnly bool) (string, error) {
	rootInfo, err := os.Lstat(root)
	if err != nil {
		return "", err
	}
	if !rootInfo.IsDir() || rootInfo.Mode()&os.ModeSymlink != 0 || (requireReadOnly && rootInfo.Mode().Perm()&0o222 != 0) {
		return "", fmt.Errorf("local view root is not an immutable real directory")
	}
	var entries []localTreeEntry
	err = filepath.WalkDir(root, func(name string, entry os.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if err := ctx.Err(); err != nil {
			return err
		}
		if name == root {
			return nil
		}
		relative, err := filepath.Rel(root, name)
		if err != nil {
			return err
		}
		info, err := entry.Info()
		if err != nil {
			return err
		}
		if info.Mode()&os.ModeSymlink != 0 || (requireReadOnly && info.Mode().Perm()&0o222 != 0) {
			return fmt.Errorf("local view contains mutable or linked path %q", filepath.ToSlash(relative))
		}
		if !info.IsDir() && !info.Mode().IsRegular() {
			return fmt.Errorf("local view contains unsupported path %q", filepath.ToSlash(relative))
		}
		size := info.Size()
		if info.IsDir() {
			size = 0
		}
		entries = append(entries, localTreeEntry{relative: filepath.ToSlash(relative), directory: info.IsDir(), size: size})
		return nil
	})
	if err != nil {
		return "", err
	}
	sort.Slice(entries, func(i, j int) bool { return entries[i].relative < entries[j].relative })
	hash := sha256.New()
	for _, entry := range entries {
		if err := ctx.Err(); err != nil {
			return "", err
		}
		kind := byte('f')
		if entry.directory {
			kind = 'd'
		}
		_, _ = hash.Write([]byte{kind})
		_ = binary.Write(hash, binary.BigEndian, uint32(len(entry.relative)))
		_, _ = hash.Write([]byte(entry.relative))
		_ = binary.Write(hash, binary.BigEndian, uint64(entry.size))
		if entry.directory {
			continue
		}
		file, err := os.Open(filepath.Join(root, filepath.FromSlash(entry.relative)))
		if err != nil {
			return "", err
		}
		copied, copyErr := copyLocalFile(ctx, hash, io.LimitReader(file, entry.size+1))
		closeErr := file.Close()
		if copyErr != nil || closeErr != nil {
			return "", errors.Join(copyErr, closeErr)
		}
		if copied != entry.size {
			return "", fmt.Errorf("local view changed while hashing %q", entry.relative)
		}
	}
	return hex.EncodeToString(hash.Sum(nil)), nil
}

func freezeLocalTree(root string) error {
	var directories []string
	err := filepath.Walk(root, func(name string, info os.FileInfo, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if info.IsDir() {
			directories = append(directories, name)
			return nil
		}
		return os.Chmod(name, 0o444)
	})
	if err != nil {
		return err
	}
	sort.Slice(directories, func(i, j int) bool { return len(directories[i]) > len(directories[j]) })
	for _, directory := range directories {
		if err := os.Chmod(directory, 0o555); err != nil {
			return err
		}
	}
	return nil
}

func makeLocalTreeWritable(root string) {
	_ = filepath.Walk(root, func(name string, info os.FileInfo, walkErr error) error {
		if walkErr == nil {
			if info.IsDir() {
				_ = os.Chmod(name, 0o700)
			} else {
				_ = os.Chmod(name, 0o600)
			}
		}
		return nil
	})
}

type artifactManifest struct {
	InterfacePath  string `toml:"interface_path"`
	ObjectManifest string `toml:"object_manifest"`
}

func validateArtifactDependency(base string, dependency model.Dependency) (string, error) {
	artifactRoot, err := resolveLocalDirectory(base, dependency.Artifact)
	if err != nil {
		return "", fmt.Errorf("resolve artifact: %w", err)
	}
	manifestPath := filepath.Join(artifactRoot, "Seen.pkg.toml")
	if err := requireNonemptyRegularFile(manifestPath); err != nil {
		fallback := filepath.Join(artifactRoot, "seenpkg.toml")
		if fallbackErr := requireNonemptyRegularFile(fallback); fallbackErr != nil {
			return "", fmt.Errorf("artifact %s is missing a nonempty Seen.pkg.toml or seenpkg.toml", artifactRoot)
		}
		manifestPath = fallback
	}
	raw, err := os.ReadFile(manifestPath)
	if err != nil {
		return "", err
	}
	var metadata artifactManifest
	if err := toml.Unmarshal(raw, &metadata); err != nil {
		return "", fmt.Errorf("parse artifact manifest: %w", err)
	}
	if metadata.ObjectManifest == "" {
		metadata.ObjectManifest = "objects.tsv"
	}
	if metadata.InterfacePath == "" {
		metadata.InterfacePath = "src"
	}
	objectManifest, err := resolveArtifactMember(artifactRoot, metadata.ObjectManifest)
	if err != nil {
		return "", fmt.Errorf("artifact object_manifest: %w", err)
	}
	if err := requireNonemptyRegularFile(objectManifest); err != nil {
		return "", fmt.Errorf("artifact object manifest %s is unusable: %w", objectManifest, err)
	}
	interfaceRoot, err := resolveArtifactMember(artifactRoot, metadata.InterfacePath)
	if err != nil {
		return "", fmt.Errorf("artifact interface_path: %w", err)
	}
	info, err := os.Lstat(interfaceRoot)
	if err != nil || !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return "", fmt.Errorf("artifact interface directory %s is missing or not a real directory", interfaceRoot)
	}
	return artifactRoot, nil
}

func resolveArtifactMember(root, value string) (string, error) {
	name := filepath.FromSlash(value)
	if !filepath.IsAbs(name) {
		name = filepath.Join(root, name)
	}
	absolute, err := filepath.Abs(name)
	if err != nil {
		return "", err
	}
	real, err := filepath.EvalSymlinks(absolute)
	if err != nil {
		return "", err
	}
	return filepath.Clean(real), nil
}

func requireNonemptyRegularFile(name string) error {
	info, err := os.Lstat(name)
	if err != nil {
		return err
	}
	if !info.Mode().IsRegular() || info.Mode()&os.ModeSymlink != 0 || info.Size() == 0 {
		return fmt.Errorf("must be a nonempty regular file")
	}
	return nil
}

func pathWithin(root, candidate string) (bool, error) {
	relative, err := filepath.Rel(root, candidate)
	if err != nil {
		return false, err
	}
	return relative == "." || (relative != ".." && !strings.HasPrefix(relative, ".."+string(filepath.Separator))), nil
}
