package commands

import (
	"archive/tar"
	"compress/gzip"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	seenarchive "github.com/codeyousef/seen/tools/seen-pkg/internal/archive"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/atomicfile"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/manifest"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/model"
)

type PackResult struct {
	Path   string
	SHA256 string
	Length int64
}

// Pack builds deterministic, source-only Seen package bytes and validates the
// resulting archive with the same policy used during installation.
func Pack(ctx context.Context, projectRoot, outputPath string) (PackResult, error) {
	root, err := filepath.Abs(projectRoot)
	if err != nil {
		return PackResult{}, err
	}
	root, err = filepath.EvalSymlinks(root)
	if err != nil {
		return PackResult{}, err
	}
	manifestPath := filepath.Join(root, "Seen.toml")
	parsed, err := manifest.Load(manifestPath, manifest.Options{DefaultRegistryOrigin: "https://seen.dev.yousef.codes/packages", RequireManifestV1: true})
	if err != nil {
		return PackResult{}, err
	}
	if parsed.Package == nil {
		return PackResult{}, fmt.Errorf("[package] is required for pack")
	}
	for _, dependency := range parsed.Dependencies {
		if dependency.Kind != model.DependencyRegistry {
			return PackResult{}, fmt.Errorf("publishable package dependency %q must use a hosted registry identity", dependency.Alias)
		}
	}
	if outputPath == "" {
		outputPath = filepath.Join(root, strings.Split(parsed.Package.Identity, "/")[1]+"-"+parsed.Project.Version+".seenpkg.tgz")
	}
	outputPath, err = filepath.Abs(outputPath)
	if err != nil {
		return PackResult{}, err
	}
	files, err := selectedPackageFiles(root, parsed)
	if err != nil {
		return PackResult{}, err
	}
	if err := os.MkdirAll(filepath.Dir(outputPath), 0o755); err != nil {
		return PackResult{}, err
	}
	temp, err := os.CreateTemp(filepath.Dir(outputPath), ".seen-pack-*.tgz")
	if err != nil {
		return PackResult{}, err
	}
	tempPath := temp.Name()
	committed := false
	defer func() {
		_ = temp.Close()
		if !committed {
			_ = os.Remove(tempPath)
		}
	}()
	hash := sha256.New()
	gzipWriter, err := gzip.NewWriterLevel(io.MultiWriter(temp, hash), gzip.BestCompression)
	if err != nil {
		return PackResult{}, err
	}
	gzipWriter.Header.ModTime = time.Unix(0, 0).UTC()
	gzipWriter.Header.OS = 255
	tarWriter := tar.NewWriter(gzipWriter)
	for _, relative := range files {
		full := filepath.Join(root, filepath.FromSlash(relative))
		info, err := os.Stat(full)
		if err != nil {
			return PackResult{}, err
		}
		header := &tar.Header{Name: relative, Mode: 0o644, Size: info.Size(), Typeflag: tar.TypeReg, ModTime: time.Unix(0, 0).UTC(), AccessTime: time.Time{}, ChangeTime: time.Time{}, Uid: 0, Gid: 0, Uname: "", Gname: "", Format: tar.FormatUSTAR}
		if err := tarWriter.WriteHeader(header); err != nil {
			return PackResult{}, err
		}
		file, err := os.Open(full)
		if err != nil {
			return PackResult{}, err
		}
		_, copyErr := io.Copy(tarWriter, file)
		closeErr := file.Close()
		if copyErr != nil {
			return PackResult{}, copyErr
		}
		if closeErr != nil {
			return PackResult{}, closeErr
		}
	}
	if err := tarWriter.Close(); err != nil {
		return PackResult{}, err
	}
	if err := gzipWriter.Close(); err != nil {
		return PackResult{}, err
	}
	if err := temp.Sync(); err != nil {
		return PackResult{}, err
	}
	if err := temp.Close(); err != nil {
		return PackResult{}, err
	}
	info, err := os.Stat(tempPath)
	if err != nil {
		return PackResult{}, err
	}
	digest := hex.EncodeToString(hash.Sum(nil))
	binder := &packageBinder{expected: parsed, expectedRaw: parsed.Raw}
	if _, err := seenarchive.Preflight(ctx, tempPath, seenarchive.Options{ExpectedSHA256: digest, Limits: seenarchive.DefaultLimits(), Binder: binder}); err != nil {
		return PackResult{}, err
	}
	if err := os.Chmod(tempPath, 0o644); err != nil {
		return PackResult{}, err
	}
	if err := atomicfile.Replace(tempPath, outputPath); err != nil {
		return PackResult{}, fmt.Errorf("promote package archive: %w", err)
	}
	if err := atomicfile.SyncDir(filepath.Dir(outputPath)); err != nil {
		return PackResult{}, fmt.Errorf("sync package archive directory: %w", err)
	}
	committed = true
	return PackResult{Path: outputPath, SHA256: digest, Length: info.Size()}, nil
}

func selectedPackageFiles(root string, parsed *model.Manifest) ([]string, error) {
	patterns := append(append([]string(nil), parsed.Package.Include...), parsed.Package.Assets...)
	matched := make([]bool, len(patterns))
	selected := map[string]bool{"Seen.toml": true}
	err := filepath.WalkDir(root, func(current string, entry os.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		relative, err := filepath.Rel(root, current)
		if err != nil {
			return err
		}
		if relative == "." {
			return nil
		}
		slash := filepath.ToSlash(relative)
		if entry.IsDir() && (slash == ".git" || slash == ".seen" || slash == "target" || slash == "target-release" || slash == "target-windows") {
			return filepath.SkipDir
		}
		if entry.Type()&os.ModeSymlink != 0 {
			return fmt.Errorf("package source contains symbolic link %q", slash)
		}
		if entry.IsDir() {
			return nil
		}
		info, err := entry.Info()
		if err != nil {
			return err
		}
		if !info.Mode().IsRegular() {
			return fmt.Errorf("package source contains non-regular entry %q", slash)
		}
		for index, pattern := range patterns {
			if globMatch(pattern, slash) {
				matched[index] = true
				selected[slash] = true
			}
		}
		return nil
	})
	if err != nil {
		return nil, err
	}
	for index, ok := range matched {
		if !ok {
			return nil, fmt.Errorf("package member pattern %q matched no files", patterns[index])
		}
	}
	files := make([]string, 0, len(selected))
	for value := range selected {
		files = append(files, value)
	}
	sort.Strings(files)
	return files, nil
}

func globMatch(pattern, name string) bool {
	patternParts := strings.Split(pattern, "/")
	nameParts := strings.Split(name, "/")
	var match func(int, int) bool
	match = func(pi, ni int) bool {
		if pi == len(patternParts) {
			return ni == len(nameParts)
		}
		if patternParts[pi] == "**" {
			for next := ni; next <= len(nameParts); next++ {
				if match(pi+1, next) {
					return true
				}
			}
			return false
		}
		if ni == len(nameParts) {
			return false
		}
		ok, err := filepath.Match(patternParts[pi], nameParts[ni])
		return err == nil && ok && match(pi+1, ni+1)
	}
	return match(0, 0)
}

type packageBinder struct {
	expected    *model.Manifest
	expectedRaw []byte
}

func (binder *packageBinder) BindManifest(raw []byte, paths []string) error {
	if binder.expected == nil || binder.expected.Package == nil {
		return fmt.Errorf("expected package manifest is missing")
	}
	if binder.expectedRaw != nil && !bytesEqual(raw, binder.expectedRaw) {
		return fmt.Errorf("archive Seen.toml bytes differ from reserved manifest")
	}
	parsed, err := manifest.ParseWithOptions(raw, manifest.Options{DefaultRegistryOrigin: "https://seen.dev.yousef.codes/packages", RequireManifestV1: true})
	if err != nil {
		return err
	}
	if parsed.Package == nil {
		return fmt.Errorf("archive package table is missing")
	}
	if parsed.Project.Version != binder.expected.Project.Version || parsed.Package.Identity != binder.expected.Package.Identity || !sameCapabilityList(parsed.Package.Capabilities, binder.expected.Package.Capabilities) || !sameStrings(parsed.Package.Include, binder.expected.Package.Include) || !sameStrings(parsed.Package.Assets, binder.expected.Package.Assets) {
		return fmt.Errorf("archive manifest reserved fields differ from signed metadata")
	}
	for _, name := range paths {
		if seenarchive.IsReservedPackageStatePath(name) {
			return fmt.Errorf("archive member %q attempts to provide project package-manager state", name)
		}
		if name == "Seen.toml" || isEffectiveDirectory(name, paths) {
			continue
		}
		allowed := false
		for _, pattern := range append(append([]string(nil), parsed.Package.Include...), parsed.Package.Assets...) {
			if globMatch(pattern, name) {
				allowed = true
				break
			}
		}
		if !allowed {
			return fmt.Errorf("archive member %q is not declared by include/assets", name)
		}
	}
	return nil
}
func bytesEqual(left, right []byte) bool {
	if len(left) != len(right) {
		return false
	}
	for i := range left {
		if left[i] != right[i] {
			return false
		}
	}
	return true
}
func sameStrings(left, right []string) bool {
	if len(left) != len(right) {
		return false
	}
	for i := range left {
		if left[i] != right[i] {
			return false
		}
	}
	return true
}
func sameCapabilityList(left, right []model.Capability) bool {
	l, r := model.CanonicalCapabilities(left), model.CanonicalCapabilities(right)
	if len(l) != len(r) {
		return false
	}
	for i := range l {
		if l[i] != r[i] {
			return false
		}
	}
	return true
}
func isEffectiveDirectory(name string, paths []string) bool {
	prefix := name + "/"
	for _, candidate := range paths {
		if strings.HasPrefix(candidate, prefix) {
			return true
		}
	}
	return false
}
