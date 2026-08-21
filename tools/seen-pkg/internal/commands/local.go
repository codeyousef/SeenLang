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
	maxLocalViewEntries             = 100_000
	maxLocalViewBytes               = int64(2 * 1024 * 1024 * 1024)
	maxLocalViewFile                = int64(512 * 1024 * 1024)
	prebuiltPackageSchema           = "seen-prebuilt-package-v2"
	prebuiltInterfaceSchema         = "seen-package-interface-v2"
	prebuiltObjectManifestSchema    = "seen-package-object-manifest-v2"
	prebuiltLayoutABI               = "seen-layout-abi-v2"
	prebuiltObjectCacheABI          = "seen-object-cache-abi-v3"
	prebuiltArtifactFormatVersion   = 2
	prebuiltArtifactRebuildGuidance = "rebuild using Seen 0.11"
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
	activeDirectories := map[string]bool{}
	return copyLocalDirectory(ctx, sourceRoot, sourceRoot, destinationRoot, ".",
		activeDirectories, &entries, &total)
}

// copyLocalDirectory follows only symlinks whose fully-resolved target remains
// inside sourceRoot. The immutable view contains real files/directories, never
// links, so publication paths can continue to reject links unconditionally.
func copyLocalDirectory(ctx context.Context, sourceRoot, sourceDirectory,
	destinationRoot, destinationRelative string, activeDirectories map[string]bool,
	entries *int, total *int64) error {

	canonicalDirectory, err := filepath.EvalSymlinks(sourceDirectory)
	if err != nil {
		return fmt.Errorf("resolve local package directory %q: %w",
			filepath.ToSlash(destinationRelative), err)
	}
	canonicalDirectory, err = filepath.Abs(canonicalDirectory)
	if err != nil {
		return err
	}
	inside, err := pathWithin(sourceRoot, canonicalDirectory)
	if err != nil || !inside {
		return fmt.Errorf("local package symbolic link %q escapes package root",
			filepath.ToSlash(destinationRelative))
	}
	if activeDirectories[canonicalDirectory] {
		return fmt.Errorf("local package symbolic link cycle at %q",
			filepath.ToSlash(destinationRelative))
	}
	activeDirectories[canonicalDirectory] = true
	defer delete(activeDirectories, canonicalDirectory)

	directoryEntries, err := os.ReadDir(canonicalDirectory)
	if err != nil {
		return err
	}
	for _, entry := range directoryEntries {
		if err := ctx.Err(); err != nil {
			return err
		}
		relative := entry.Name()
		if destinationRelative != "." {
			relative = filepath.Join(destinationRelative, entry.Name())
		}
		first := relative
		if separator := strings.IndexRune(relative, filepath.Separator); separator >= 0 {
			first = relative[:separator]
		}
		if first == ".git" || first == ".seen" {
			continue
		}
		*entries = *entries + 1
		if *entries > maxLocalViewEntries {
			return fmt.Errorf("local package exceeds %d entries", maxLocalViewEntries)
		}

		name := filepath.Join(canonicalDirectory, entry.Name())
		resolved := name
		if entry.Type()&os.ModeSymlink != 0 {
			resolved, err = filepath.EvalSymlinks(name)
			if err != nil {
				return fmt.Errorf("resolve local package symbolic link %q: %w",
					filepath.ToSlash(relative), err)
			}
			resolved, err = filepath.Abs(resolved)
			if err != nil {
				return err
			}
			inside, err := pathWithin(sourceRoot, resolved)
			if err != nil || !inside {
				return fmt.Errorf("local package symbolic link %q escapes package root",
					filepath.ToSlash(relative))
			}
			resolvedRelative, err := filepath.Rel(sourceRoot, resolved)
			if err != nil {
				return err
			}
			resolvedFirst := resolvedRelative
			if separator := strings.IndexRune(resolvedRelative, filepath.Separator); separator >= 0 {
				resolvedFirst = resolvedRelative[:separator]
			}
			if resolvedFirst == ".git" || resolvedFirst == ".seen" {
				return fmt.Errorf("local package symbolic link %q targets excluded package state",
					filepath.ToSlash(relative))
			}
		}

		info, err := os.Stat(resolved)
		if err != nil {
			return err
		}
		destination := filepath.Join(destinationRoot, relative)
		if info.IsDir() {
			if err := os.Mkdir(destination, 0o700); err != nil {
				return err
			}
			if err := copyLocalDirectory(ctx, sourceRoot, resolved, destinationRoot,
				relative, activeDirectories, entries, total); err != nil {
				return err
			}
			continue
		}
		if !info.Mode().IsRegular() {
			return fmt.Errorf("local package contains unsupported file type %q", filepath.ToSlash(relative))
		}
		if info.Size() > maxLocalViewFile {
			return fmt.Errorf("local package file %q exceeds %d bytes", filepath.ToSlash(relative), maxLocalViewFile)
		}
		*total = *total + info.Size()
		if *total > maxLocalViewBytes {
			return fmt.Errorf("local package exceeds %d bytes", maxLocalViewBytes)
		}
		input, err := os.Open(resolved)
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
	}
	return nil
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
	Schema                string `toml:"schema"`
	Version               int    `toml:"version"`
	InterfaceSchema       string `toml:"interface_schema"`
	InterfaceVersion      int    `toml:"interface_version"`
	ObjectManifestSchema  string `toml:"object_manifest_schema"`
	ObjectManifestVersion int    `toml:"object_manifest_version"`
	LayoutABI             string `toml:"layout_abi"`
	ObjectCacheABI        string `toml:"object_cache_abi"`
	Language              string `toml:"language"`
	InterfacePath         string `toml:"interface_path"`
	InterfaceIndex        string `toml:"interface_index"`
	ObjectManifest        string `toml:"object_manifest"`
	DeclarationDigest     string `toml:"declaration_digest"`
	BuildSignature        string `toml:"build_signature"`
}

func incompatibleArtifact(root, reason string) error {
	return fmt.Errorf("artifact %s %s; %s", root, reason, prebuiltArtifactRebuildGuidance)
}

func validateArtifactTableHeader(raw []byte, schema string) ([]string, error) {
	var lines []string
	for _, line := range strings.Split(string(raw), "\n") {
		line = strings.TrimSuffix(line, "\r")
		if strings.TrimSpace(line) == "" || strings.HasPrefix(strings.TrimSpace(line), "#") {
			continue
		}
		lines = append(lines, line)
	}
	expected := []string{
		"schema\t" + schema,
		"version\t2",
		"layout_abi\t" + prebuiltLayoutABI,
		"object_cache_abi\t" + prebuiltObjectCacheABI,
	}
	if len(lines) < len(expected) {
		return nil, fmt.Errorf("table header is incomplete")
	}
	for index := range expected {
		if lines[index] != expected[index] {
			return nil, fmt.Errorf("table header row %d is %q, want %q", index+1, lines[index], expected[index])
		}
	}
	return lines[len(expected):], nil
}

func requireArtifactRelativeMember(root, value string, nonempty bool) (string, error) {
	if err := validateArtifactRelativePath(value); err != nil {
		return "", err
	}
	member, err := resolveArtifactMember(root, value)
	if err != nil {
		return "", err
	}
	info, err := os.Lstat(member)
	if err != nil {
		return "", err
	}
	if !info.Mode().IsRegular() || info.Mode()&os.ModeSymlink != 0 || (nonempty && info.Size() == 0) {
		kind := ""
		if nonempty {
			kind = " nonempty"
		}
		return "", fmt.Errorf("member %s must be a%s regular file", member, kind)
	}
	return member, nil
}

func validateArtifactRelativePath(value string) error {
	name := filepath.FromSlash(value)
	if value == "" || filepath.IsAbs(name) {
		return fmt.Errorf("path must be nonempty and relative")
	}
	clean := filepath.Clean(name)
	if clean == "." || clean == ".." || strings.HasPrefix(clean, ".."+string(filepath.Separator)) || clean != name {
		return fmt.Errorf("path must be canonical and stay inside the artifact")
	}
	return nil
}

func validateArtifactInterfaceRows(root string, rows []string) error {
	moduleCount := 0
	for _, row := range rows {
		fields := strings.Split(row, "\t")
		if len(fields) == 0 {
			continue
		}
		switch fields[0] {
		case "module":
			if len(fields) < 5 || fields[1] == "" || fields[4] == "" {
				return fmt.Errorf("malformed module row")
			}
			if _, err := requireArtifactRelativeMember(root, fields[1], false); err != nil {
				return fmt.Errorf("interface module %q: %w", fields[1], err)
			}
			moduleCount++
		case "import", "function", "class", "enum", "declaration":
			if len(fields) < 6 || fields[1] == "" || fields[4] == "" {
				return fmt.Errorf("malformed %s declaration row", fields[0])
			}
		default:
			return fmt.Errorf("unknown interface row kind %q", fields[0])
		}
	}
	if moduleCount == 0 {
		return fmt.Errorf("interface has no module rows")
	}
	return nil
}

func validateArtifactObjectRows(root string, rows []string) error {
	if len(rows) == 0 {
		return fmt.Errorf("object manifest has no object rows")
	}
	for _, row := range rows {
		fields := strings.Split(row, "\t")
		if len(fields) != 2 || fields[0] == "" || fields[1] == "" {
			return fmt.Errorf("malformed object row")
		}
		if _, err := requireArtifactRelativeMember(root, fields[0], true); err != nil {
			return fmt.Errorf("object %q: %w", fields[0], err)
		}
		// The source column is immutable provenance, not an interface fallback.
		// An outer prebuild may flatten a dependency object whose source is not
		// copied into the outer artifact, but the provenance path must remain
		// portable and incapable of escaping the artifact namespace.
		if err := validateArtifactRelativePath(fields[1]); err != nil {
			return fmt.Errorf("object source provenance %q: %w", fields[1], err)
		}
	}
	return nil
}

func validateArtifactDependency(base string, dependency model.Dependency) (string, error) {
	artifactRoot, err := resolveLocalDirectory(base, dependency.Artifact)
	if err != nil {
		return "", fmt.Errorf("resolve artifact: %w", err)
	}
	manifestPath := filepath.Join(artifactRoot, "Seen.pkg.toml")
	if err := requireNonemptyRegularFile(manifestPath); err != nil {
		fallback := filepath.Join(artifactRoot, "seenpkg.toml")
		if fallbackErr := requireNonemptyRegularFile(fallback); fallbackErr == nil {
			return "", incompatibleArtifact(artifactRoot, "uses the legacy seenpkg.toml manifest")
		}
		return "", incompatibleArtifact(artifactRoot, "is missing a nonempty v2 Seen.pkg.toml")
	}
	raw, err := os.ReadFile(manifestPath)
	if err != nil {
		return "", incompatibleArtifact(artifactRoot, fmt.Sprintf("cannot read Seen.pkg.toml: %v", err))
	}
	var metadata artifactManifest
	if err := toml.Unmarshal(raw, &metadata); err != nil {
		return "", incompatibleArtifact(artifactRoot, fmt.Sprintf("has an invalid Seen.pkg.toml: %v", err))
	}
	if metadata.Schema != prebuiltPackageSchema ||
		metadata.Version != prebuiltArtifactFormatVersion ||
		metadata.InterfaceSchema != prebuiltInterfaceSchema ||
		metadata.InterfaceVersion != prebuiltArtifactFormatVersion ||
		metadata.ObjectManifestSchema != prebuiltObjectManifestSchema ||
		metadata.ObjectManifestVersion != prebuiltArtifactFormatVersion ||
		metadata.LayoutABI != prebuiltLayoutABI ||
		metadata.ObjectCacheABI != prebuiltObjectCacheABI {
		return "", incompatibleArtifact(artifactRoot, "has an unsupported schema or ABI")
	}
	if metadata.Language == "" || metadata.InterfacePath == "" || metadata.InterfaceIndex == "" ||
		metadata.ObjectManifest == "" || len(metadata.DeclarationDigest) != sha256.Size*2 ||
		!strings.Contains(metadata.BuildSignature, "layout="+prebuiltLayoutABI) ||
		!strings.Contains(metadata.BuildSignature, "object-cache="+prebuiltObjectCacheABI) {
		return "", incompatibleArtifact(artifactRoot, "has incomplete v2 metadata")
	}
	objectManifest, err := resolveArtifactMember(artifactRoot, metadata.ObjectManifest)
	if err != nil {
		return "", incompatibleArtifact(artifactRoot, fmt.Sprintf("has an unsafe object_manifest: %v", err))
	}
	if err := requireNonemptyRegularFile(objectManifest); err != nil {
		return "", incompatibleArtifact(artifactRoot, fmt.Sprintf("has an unusable object manifest: %v", err))
	}
	interfaceRoot, err := resolveArtifactMember(artifactRoot, metadata.InterfacePath)
	if err != nil {
		return "", incompatibleArtifact(artifactRoot, fmt.Sprintf("has an unsafe interface_path: %v", err))
	}
	info, err := os.Lstat(interfaceRoot)
	if err != nil || !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return "", incompatibleArtifact(artifactRoot, "has a missing or linked interface directory")
	}
	interfaceIndex, err := resolveArtifactMember(artifactRoot, metadata.InterfaceIndex)
	if err != nil {
		return "", incompatibleArtifact(artifactRoot, fmt.Sprintf("has an unsafe interface_index: %v", err))
	}
	if err := requireNonemptyRegularFile(interfaceIndex); err != nil {
		return "", incompatibleArtifact(artifactRoot, fmt.Sprintf("has an unusable interface index: %v", err))
	}
	interfaceRaw, err := os.ReadFile(interfaceIndex)
	if err != nil {
		return "", incompatibleArtifact(artifactRoot, fmt.Sprintf("cannot read interface index: %v", err))
	}
	interfaceRows, err := validateArtifactTableHeader(interfaceRaw, prebuiltInterfaceSchema)
	if err != nil {
		return "", incompatibleArtifact(artifactRoot, fmt.Sprintf("has an incompatible interface index: %v", err))
	}
	if err := validateArtifactInterfaceRows(artifactRoot, interfaceRows); err != nil {
		return "", incompatibleArtifact(artifactRoot, fmt.Sprintf("has an invalid interface index: %v", err))
	}
	actualDigest := sha256.Sum256(interfaceRaw)
	if hex.EncodeToString(actualDigest[:]) != strings.ToLower(metadata.DeclarationDigest) {
		return "", incompatibleArtifact(artifactRoot, "has a declaration fingerprint mismatch")
	}
	objectRaw, err := os.ReadFile(objectManifest)
	if err != nil {
		return "", incompatibleArtifact(artifactRoot, fmt.Sprintf("cannot read object manifest: %v", err))
	}
	objectRows, err := validateArtifactTableHeader(objectRaw, prebuiltObjectManifestSchema)
	if err != nil {
		return "", incompatibleArtifact(artifactRoot, fmt.Sprintf("has an incompatible object manifest: %v", err))
	}
	if err := validateArtifactObjectRows(artifactRoot, objectRows); err != nil {
		return "", incompatibleArtifact(artifactRoot, fmt.Sprintf("has an invalid object manifest: %v", err))
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
	if filepath.Clean(absolute) != filepath.Clean(real) {
		return "", fmt.Errorf("artifact member must not traverse symbolic links")
	}
	rootReal, err := filepath.EvalSymlinks(root)
	if err != nil {
		return "", err
	}
	inside, err := pathWithin(filepath.Clean(rootReal), filepath.Clean(real))
	if err != nil || !inside || filepath.Clean(rootReal) == filepath.Clean(real) {
		return "", fmt.Errorf("artifact member escapes artifact root")
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
