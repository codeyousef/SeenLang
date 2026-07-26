// Package cache owns the immutable, content-addressed package cache and safe
// project-local package views. Shared cache files are never hard-linked into a
// project: a project owner could chmod a hard link and mutate the shared inode.
package cache

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"time"

	"github.com/codeyousef/seen/tools/seen-pkg/internal/archive"
)

var (
	componentPattern = regexp.MustCompile(`^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$`)
	versionPattern   = regexp.MustCompile(`^(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:-(?:0|[1-9][0-9]*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*)(?:\.(?:0|[1-9][0-9]*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*))*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$`)
)

// Key is the full cache identity. Registry aliases are intentionally absent:
// the lock has already bound a canonical origin, version, and archive digest.
type Key struct {
	Owner   string
	Name    string
	Version string
	SHA256  string
}

func (k Key) Validate() error {
	if !componentPattern.MatchString(k.Owner) || !componentPattern.MatchString(k.Name) {
		return errors.New("canonical owner and package name required")
	}
	if !versionPattern.MatchString(k.Version) || len(k.Version) > 128 {
		return errors.New("canonical exact semantic version required")
	}
	if len(k.SHA256) != 64 || strings.ToLower(k.SHA256) != k.SHA256 {
		return errors.New("lowercase SHA-256 digest required")
	}
	_, err := hex.DecodeString(k.SHA256)
	return err
}

// Cache stores verified blobs and extracted source trees beneath Root.
type Cache struct {
	Root string
}

// Error is a stable cache failure.
type Error struct {
	Code string
	Err  error
}

func (e *Error) Error() string {
	if e.Err == nil {
		return e.Code
	}
	return e.Code + ": " + e.Err.Error()
}

func (e *Error) Unwrap() error { return e.Err }

func failure(code string, err error) error { return &Error{Code: code, Err: err} }

// New creates a private cache root and rejects a symlink as the root itself.
func New(root string) (*Cache, error) {
	abs, err := filepath.Abs(root)
	if err != nil {
		return nil, failure("cache_root_invalid", err)
	}
	if err := os.MkdirAll(abs, 0o700); err != nil {
		return nil, failure("cache_root_create_failed", err)
	}
	info, err := os.Lstat(abs)
	if err != nil || !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return nil, failure("cache_root_invalid", errors.New("cache root must be a real directory"))
	}
	if err := os.Chmod(abs, 0o700); err != nil {
		return nil, failure("cache_root_create_failed", err)
	}
	c := &Cache{Root: filepath.Clean(abs)}
	for _, dir := range []string{"blobs/sha256", "packages", ".staging", ".corrupt"} {
		if err := c.mkdir(dir, 0o700); err != nil {
			return nil, err
		}
	}
	return c, nil
}

func (c *Cache) mkdir(rel string, mode os.FileMode) error {
	if !safeRelative(rel) {
		return failure("cache_path_invalid", errors.New("unsafe relative path"))
	}
	current := c.Root
	for _, part := range strings.Split(filepath.Clean(rel), string(filepath.Separator)) {
		current = filepath.Join(current, part)
		if err := os.Mkdir(current, mode); err != nil && !errors.Is(err, os.ErrExist) {
			return failure("cache_directory_failed", err)
		}
		info, err := os.Lstat(current)
		if err != nil || !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
			return failure("cache_path_invalid", fmt.Errorf("cache component %q is not a real directory", current))
		}
	}
	return nil
}

func safeRelative(rel string) bool {
	return rel != "" && !filepath.IsAbs(rel) && rel != ".." && !strings.HasPrefix(rel, ".."+string(filepath.Separator))
}

// BlobPath returns the immutable archive blob location for digest.
func (c *Cache) BlobPath(digest string) (string, error) {
	k := Key{Owner: "a", Name: "a", Version: "0.0.0", SHA256: digest}
	if err := k.Validate(); err != nil {
		return "", failure("cache_key_invalid", err)
	}
	return filepath.Join(c.Root, "blobs", "sha256", digest), nil
}

// EntryPath returns the cache entry root; SourcePath appends its source tree.
func (c *Cache) EntryPath(key Key) (string, error) {
	if err := key.Validate(); err != nil {
		return "", failure("cache_key_invalid", err)
	}
	return filepath.Join(c.Root, "packages", key.Owner, key.Name, key.Version, key.SHA256), nil
}

func (c *Cache) SourcePath(key Key) (string, error) {
	entry, err := c.EntryPath(key)
	if err != nil {
		return "", err
	}
	return filepath.Join(entry, "source"), nil
}

// PutBlob copies source into the cache using a same-filesystem atomic no-replace
// hard-link promotion. The source inode itself is never linked into the cache.
func (c *Cache) PutBlob(ctx context.Context, source, digest string, length int64) (string, error) {
	final, err := c.BlobPath(digest)
	if err != nil {
		return "", err
	}
	if err := verifyRegularFile(ctx, final, digest, length, true); err == nil {
		return final, nil
	} else if !errors.Is(err, os.ErrNotExist) {
		if quarantineErr := c.quarantine(final, "blob-"+digest); quarantineErr != nil {
			return "", failure("cache_corrupt_blob", errors.Join(err, quarantineErr))
		}
	}
	tmp, err := os.CreateTemp(filepath.Dir(final), ".blob-*")
	if err != nil {
		return "", failure("cache_blob_write_failed", err)
	}
	tmpPath := tmp.Name()
	committed := false
	defer func() {
		_ = tmp.Close()
		if !committed {
			_ = os.Remove(tmpPath)
		}
	}()
	in, err := os.Open(source)
	if err != nil {
		return "", failure("cache_blob_read_failed", err)
	}
	defer in.Close()
	h := sha256.New()
	n, copyErr := io.Copy(io.MultiWriter(tmp, h), io.LimitReader(contextReader{ctx: ctx, r: in}, length+1))
	if copyErr != nil {
		return "", failure("cache_blob_read_failed", copyErr)
	}
	actual := hex.EncodeToString(h.Sum(nil))
	if n != length || actual != digest {
		return "", failure("cache_blob_digest_mismatch", fmt.Errorf("expected %s/%d, got %s/%d", digest, length, actual, n))
	}
	if err := tmp.Sync(); err != nil {
		return "", failure("cache_blob_write_failed", err)
	}
	if err := tmp.Close(); err != nil {
		return "", failure("cache_blob_write_failed", err)
	}
	if err := os.Chmod(tmpPath, 0o444); err != nil {
		return "", failure("cache_blob_write_failed", err)
	}
	if err := os.Link(tmpPath, final); err != nil {
		if verifyErr := verifyRegularFile(ctx, final, digest, length, true); verifyErr == nil {
			_ = os.Remove(tmpPath)
			committed = true
			return final, nil
		}
		return "", failure("cache_blob_promote_failed", err)
	}
	if err := os.Remove(tmpPath); err != nil {
		return "", failure("cache_blob_promote_failed", err)
	}
	committed = true
	return final, nil
}

type integrity struct {
	Version       int             `json:"version"`
	ArchiveSHA256 string          `json:"archive_sha256"`
	ArchiveLength int64           `json:"archive_length"`
	Files         []integrityFile `json:"files"`
}

type integrityFile struct {
	Path   string `json:"path"`
	Kind   string `json:"kind"`
	Size   int64  `json:"size"`
	SHA256 string `json:"sha256,omitempty"`
}

// Install revalidates, extracts, and atomically promotes a package. It returns
// the read-only source root used by the compiler.
func (c *Cache) Install(ctx context.Context, key Key, archiveSource string, options archive.Options) (string, error) {
	if err := key.Validate(); err != nil {
		return "", failure("cache_key_invalid", err)
	}
	if options.ExpectedSHA256 != key.SHA256 {
		return "", failure("cache_key_digest_mismatch", errors.New("archive digest does not match cache key"))
	}
	info, err := os.Stat(archiveSource)
	if err != nil || !info.Mode().IsRegular() {
		return "", failure("cache_blob_read_failed", errors.New("archive source must be a regular file"))
	}
	maxCompressed := options.Limits.CompressedBytes
	if maxCompressed <= 0 {
		maxCompressed = archive.DefaultLimits().CompressedBytes
	}
	if info.Size() <= 0 || info.Size() > maxCompressed {
		return "", failure("archive_compressed_size_limit", fmt.Errorf("archive length %d is outside limit", info.Size()))
	}
	// Bind and inventory the supplied bytes before changing any cache state.
	// PutBlob repeats the digest/length check while copying, closing the source
	// replacement window between this preflight and promotion.
	boundReport, err := archive.Preflight(ctx, archiveSource, options)
	if err != nil {
		return "", err
	}
	blob, err := c.PutBlob(ctx, archiveSource, key.SHA256, boundReport.Compressed)
	if err != nil {
		return "", err
	}
	final, err := c.EntryPath(key)
	if err != nil {
		return "", err
	}
	// A content-addressed entry may have been installed under a different
	// signed candidate. Reuse is safe only after the caller's binder accepts
	// the canonical blob and the extracted tree matches the exact inventory
	// from that same preflight report. In particular, integrity.json is never
	// allowed to define (or expand) the trusted inventory.
	if _, statErr := os.Lstat(final); statErr == nil {
		expected := integrityFromReport(boundReport)
		if source, lookupErr := c.lookupWithExpected(ctx, key, expected); lookupErr == nil {
			return source, nil
		} else {
			// The content-addressed root itself was observed above, so an ENOENT
			// here means the entry is incomplete (or disappeared concurrently),
			// not that there is nothing to repair. Quarantine any surviving root
			// before promoting the rebuilt entry.
			if quarantineErr := c.quarantine(final, "entry-"+key.SHA256); quarantineErr != nil {
				return "", failure("cache_corrupt_entry", errors.Join(lookupErr, quarantineErr))
			}
		}
	} else if !errors.Is(statErr, os.ErrNotExist) {
		return "", failure("cache_entry_read_failed", statErr)
	}
	if err := c.mkdir(filepath.Join("packages", key.Owner, key.Name, key.Version), 0o700); err != nil {
		return "", err
	}
	// Stage beside the final entry. Besides guaranteeing the same filesystem,
	// this permits a no-window rename after the staging root becomes read-only.
	stage, err := os.MkdirTemp(filepath.Dir(final), ".entry-*")
	if err != nil {
		return "", failure("cache_entry_write_failed", err)
	}
	committed := false
	defer func() {
		if !committed {
			makeWritable(stage)
			_ = os.RemoveAll(stage)
		}
	}()
	report, err := archive.Extract(ctx, blob, filepath.Join(stage, "source"), options)
	if err != nil {
		return "", err
	}
	meta := integrityFromReport(report)
	if err := writeIntegrity(filepath.Join(stage, "integrity.json"), meta); err != nil {
		return "", failure("cache_entry_write_failed", err)
	}
	if err := os.Chmod(stage, 0o555); err != nil {
		return "", failure("cache_entry_write_failed", err)
	}
	if err := os.Rename(stage, final); err != nil {
		// A concurrent winner is acceptable only when it has the exact archive
		// inventory that this call already validated with its own binder.
		if source, verifyErr := c.lookupWithExpected(ctx, key, meta); verifyErr == nil {
			makeWritable(stage)
			_ = os.RemoveAll(stage)
			committed = true
			return source, nil
		}
		return "", failure("cache_entry_promote_failed", err)
	}
	committed = true
	return filepath.Join(final, "source"), nil
}

// Lookup derives the expected source inventory afresh from the signed archive
// blob, then rehashes every extracted regular file against that inventory.
// integrity.json is only a cached copy: changing it alongside extracted source
// can never make bytes that differ from the archive validate.
func (c *Cache) Lookup(ctx context.Context, key Key) (string, error) {
	source, _, err := c.lookupVerified(ctx, key)
	return source, err
}

func (c *Cache) lookupVerified(ctx context.Context, key Key) (string, integrity, error) {
	blob, err := c.BlobPath(key.SHA256)
	if err != nil {
		return "", integrity{}, err
	}
	expected, err := integrityFromArchive(ctx, key, blob)
	if err != nil {
		return "", integrity{}, err
	}
	source, err := c.lookupWithExpected(ctx, key, expected)
	return source, expected, err
}

func (c *Cache) lookupWithExpected(ctx context.Context, key Key, expected integrity) (string, error) {
	entry, err := c.EntryPath(key)
	if err != nil {
		return "", err
	}
	if err := verifyEntryLayout(entry); err != nil {
		return "", err
	}
	stored, err := readIntegrity(filepath.Join(entry, "integrity.json"))
	if err != nil {
		return "", err
	}
	if !sameIntegrity(stored, expected) {
		return "", failure("cache_corrupt_entry", errors.New("integrity metadata is not derived from the verified archive"))
	}
	source := filepath.Join(entry, "source")
	if err := verifyTree(ctx, source, expected.Files); err != nil {
		return "", failure("cache_corrupt_entry", err)
	}
	return source, nil
}

// ProjectViewPath is the deterministic project-local source-view location.
func ProjectViewPath(projectRoot string, key Key) (string, error) {
	if err := key.Validate(); err != nil {
		return "", failure("cache_key_invalid", err)
	}
	abs, err := filepath.Abs(projectRoot)
	if err != nil {
		return "", failure("cache_view_root_invalid", err)
	}
	info, err := os.Stat(abs)
	if err != nil || !info.IsDir() {
		return "", failure("cache_view_root_invalid", errors.New("project root must exist"))
	}
	real, err := filepath.EvalSymlinks(abs)
	if err != nil {
		return "", failure("cache_view_root_invalid", err)
	}
	return filepath.Join(real, ".seen", "views", key.Owner, key.Name, key.Version, key.SHA256), nil
}

// CreateProjectView atomically copies a verified source tree beneath the
// project. Copies isolate shared cache inodes from project-local chmod/write.
func (c *Cache) CreateProjectView(ctx context.Context, key Key, projectRoot string) (string, error) {
	cacheSource, meta, err := c.lookupVerified(ctx, key)
	if err != nil {
		return "", err
	}
	final, err := ProjectViewPath(projectRoot, key)
	if err != nil {
		return "", err
	}
	if view, err := verifyView(ctx, final, meta); err == nil {
		return view, nil
	} else if !errors.Is(err, os.ErrNotExist) {
		if err := quarantineProjectView(final); err != nil {
			return "", failure("cache_view_corrupt", err)
		}
	}
	parent := filepath.Dir(final)
	projectAbs, err := filepath.Abs(projectRoot)
	if err != nil {
		return "", failure("cache_view_root_invalid", err)
	}
	projectReal, err := filepath.EvalSymlinks(projectAbs)
	if err != nil {
		return "", failure("cache_view_root_invalid", err)
	}
	if err := secureMkdirUnder(projectReal, parent); err != nil {
		return "", failure("cache_view_root_invalid", err)
	}
	stage, err := os.MkdirTemp(parent, ".view-*")
	if err != nil {
		return "", failure("cache_view_write_failed", err)
	}
	committed := false
	defer func() {
		if !committed {
			makeWritable(stage)
			_ = os.RemoveAll(stage)
		}
	}()
	if err := copyVerifiedTree(ctx, cacheSource, filepath.Join(stage, "source"), meta.Files); err != nil {
		return "", failure("cache_view_write_failed", err)
	}
	if err := writeIntegrity(filepath.Join(stage, "integrity.json"), meta); err != nil {
		return "", failure("cache_view_write_failed", err)
	}
	if err := os.Chmod(stage, 0o555); err != nil {
		return "", failure("cache_view_write_failed", err)
	}
	if err := os.Rename(stage, final); err != nil {
		if view, verifyErr := verifyView(ctx, final, meta); verifyErr == nil {
			makeWritable(stage)
			_ = os.RemoveAll(stage)
			committed = true
			return view, nil
		}
		return "", failure("cache_view_promote_failed", err)
	}
	committed = true
	return filepath.Join(final, "source"), nil
}

func integrityFromArchive(ctx context.Context, key Key, blob string) (integrity, error) {
	report, err := archive.Preflight(ctx, blob, archive.Options{
		ExpectedSHA256: key.SHA256,
		Limits:         archive.DefaultLimits(),
		Binder: archive.ManifestBindingFunc(func([]byte, []string) error {
			return nil
		}),
	})
	if err != nil {
		return integrity{}, failure("cache_corrupt_blob", err)
	}
	return integrityFromReport(report), nil
}

func integrityFromReport(report archive.Report) integrity {
	result := integrity{
		Version:       1,
		ArchiveSHA256: report.ArchiveSHA256,
		ArchiveLength: report.Compressed,
		Files:         make([]integrityFile, 0, len(report.Entries)),
	}
	for _, entry := range report.Entries {
		result.Files = append(result.Files, integrityFile{
			Path: entry.Path, Kind: entry.Kind, Size: entry.Size, SHA256: entry.SHA256,
		})
	}
	return result
}

func sameIntegrity(left, right integrity) bool {
	if left.Version != right.Version || left.ArchiveSHA256 != right.ArchiveSHA256 ||
		left.ArchiveLength != right.ArchiveLength || len(left.Files) != len(right.Files) {
		return false
	}
	for index := range left.Files {
		if left.Files[index] != right.Files[index] {
			return false
		}
	}
	return true
}

func verifyView(ctx context.Context, entry string, expected integrity) (string, error) {
	if err := verifyEntryLayout(entry); err != nil {
		return "", err
	}
	actual, err := readIntegrity(filepath.Join(entry, "integrity.json"))
	if err != nil {
		return "", err
	}
	if actual.Version != expected.Version || actual.ArchiveSHA256 != expected.ArchiveSHA256 || actual.ArchiveLength != expected.ArchiveLength || len(actual.Files) != len(expected.Files) {
		return "", errors.New("view integrity metadata mismatch")
	}
	for i := range actual.Files {
		if actual.Files[i] != expected.Files[i] {
			return "", errors.New("view file inventory mismatch")
		}
	}
	source := filepath.Join(entry, "source")
	if err := verifyTree(ctx, source, expected.Files); err != nil {
		return "", err
	}
	return source, nil
}

func writeIntegrity(name string, meta integrity) error {
	data, err := json.Marshal(meta)
	if err != nil {
		return err
	}
	data = append(data, '\n')
	f, err := os.OpenFile(name, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if err != nil {
		return err
	}
	_, writeErr := f.Write(data)
	syncErr := f.Sync()
	closeErr := f.Close()
	if err := errors.Join(writeErr, syncErr, closeErr); err != nil {
		return err
	}
	return os.Chmod(name, 0o444)
}

func readIntegrity(name string) (integrity, error) {
	pathInfo, err := os.Lstat(name)
	if err != nil {
		return integrity{}, err
	}
	if !pathInfo.Mode().IsRegular() || pathInfo.Mode()&os.ModeSymlink != 0 || pathInfo.Mode().Perm()&0o222 != 0 || pathInfo.Size() > 2*1024*1024 {
		return integrity{}, failure("cache_integrity_invalid", errors.New("integrity metadata is not an immutable real regular file"))
	}
	f, err := os.Open(name)
	if err != nil {
		return integrity{}, err
	}
	defer f.Close()
	info, err := f.Stat()
	if err != nil || !info.Mode().IsRegular() || !os.SameFile(pathInfo, info) || info.Mode().Perm()&0o222 != 0 || info.Size() > 2*1024*1024 {
		return integrity{}, failure("cache_integrity_invalid", errors.New("integrity metadata changed before it could be opened"))
	}
	var meta integrity
	decoder := json.NewDecoder(io.LimitReader(f, 2*1024*1024))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&meta); err != nil {
		return integrity{}, failure("cache_integrity_invalid", err)
	}
	var extra any
	if err := decoder.Decode(&extra); !errors.Is(err, io.EOF) {
		return integrity{}, failure("cache_integrity_invalid", errors.New("trailing JSON value"))
	}
	return meta, nil
}

func verifyTree(ctx context.Context, root string, expected []integrityFile) error {
	info, err := os.Lstat(root)
	if err != nil {
		return err
	}
	if !info.IsDir() || info.Mode()&os.ModeSymlink != 0 || info.Mode().Perm()&0o222 != 0 {
		return errors.New("source root is not an immutable directory")
	}
	want := make(map[string]integrityFile, len(expected))
	allowedDirs := make(map[string]bool)
	for _, file := range expected {
		if !safeSlashPath(file.Path) {
			return errors.New("unsafe integrity path")
		}
		if _, duplicate := want[file.Path]; duplicate {
			return errors.New("duplicate integrity path")
		}
		want[file.Path] = file
		parent := filepath.ToSlash(filepath.Dir(filepath.FromSlash(file.Path)))
		for parent != "." && parent != "/" {
			allowedDirs[parent] = true
			parent = filepath.ToSlash(filepath.Dir(filepath.FromSlash(parent)))
		}
	}
	seen := make(map[string]bool, len(expected))
	err = filepath.Walk(root, func(name string, info os.FileInfo, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if err := ctx.Err(); err != nil {
			return err
		}
		if name == root {
			return nil
		}
		rel, err := filepath.Rel(root, name)
		if err != nil {
			return err
		}
		rel = filepath.ToSlash(rel)
		if info.Mode()&os.ModeSymlink != 0 || info.Mode().Perm()&0o222 != 0 {
			return fmt.Errorf("mutable or linked path %q", rel)
		}
		record, ok := want[rel]
		if !ok {
			if info.IsDir() && allowedDirs[rel] {
				return nil
			}
			return fmt.Errorf("unexpected path %q", rel)
		}
		kind := "file"
		if info.IsDir() {
			kind = "directory"
		} else if !info.Mode().IsRegular() {
			return fmt.Errorf("unsupported path type %q", rel)
		}
		if record.Kind != kind || record.Size != info.Size() {
			return fmt.Errorf("metadata mismatch at %q", rel)
		}
		if kind == "file" {
			if err := verifyRegularFile(ctx, name, record.SHA256, record.Size, true); err != nil {
				return err
			}
		}
		seen[rel] = true
		return nil
	})
	if err != nil {
		return err
	}
	if len(seen) != len(want) {
		return errors.New("source tree is missing an integrity path")
	}
	return nil
}

func verifyRegularFile(ctx context.Context, name, digest string, length int64, requireReadOnly bool) error {
	pathInfo, err := os.Lstat(name)
	if err != nil {
		return err
	}
	if !pathInfo.Mode().IsRegular() || pathInfo.Mode()&os.ModeSymlink != 0 {
		return errors.New("file path is not a real regular file")
	}
	f, err := os.Open(name)
	if err != nil {
		return err
	}
	defer f.Close()
	info, err := f.Stat()
	if err != nil {
		return err
	}
	if !info.Mode().IsRegular() || !os.SameFile(pathInfo, info) || info.Size() != length || (requireReadOnly && info.Mode().Perm()&0o222 != 0) {
		return errors.New("file mode, type, or length mismatch")
	}
	h := sha256.New()
	n, err := io.Copy(h, io.LimitReader(contextReader{ctx: ctx, r: f}, length+1))
	if err != nil {
		return err
	}
	if n != length || hex.EncodeToString(h.Sum(nil)) != digest {
		return errors.New("file digest mismatch")
	}
	return nil
}

func copyVerifiedTree(ctx context.Context, source, destination string, files []integrityFile) error {
	if err := os.Mkdir(destination, 0o700); err != nil {
		return err
	}
	records := append([]integrityFile(nil), files...)
	sort.SliceStable(records, func(i, j int) bool {
		if records[i].Kind != records[j].Kind {
			return records[i].Kind == "directory"
		}
		return records[i].Path < records[j].Path
	})
	for _, record := range records {
		if err := ctx.Err(); err != nil {
			return err
		}
		src := filepath.Join(source, filepath.FromSlash(record.Path))
		dst := filepath.Join(destination, filepath.FromSlash(record.Path))
		if record.Kind == "directory" {
			if err := os.MkdirAll(dst, 0o700); err != nil {
				return err
			}
			continue
		}
		if err := os.MkdirAll(filepath.Dir(dst), 0o700); err != nil {
			return err
		}
		in, err := os.Open(src)
		if err != nil {
			return err
		}
		out, err := os.OpenFile(dst, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
		if err != nil {
			_ = in.Close()
			return err
		}
		h := sha256.New()
		n, copyErr := io.Copy(io.MultiWriter(out, h), io.LimitReader(contextReader{ctx: ctx, r: in}, record.Size+1))
		closeErr := errors.Join(in.Close(), out.Sync(), out.Close())
		if copyErr != nil || closeErr != nil {
			return errors.Join(copyErr, closeErr)
		}
		if n != record.Size || hex.EncodeToString(h.Sum(nil)) != record.SHA256 {
			return errors.New("shared cache changed while creating view")
		}
		if err := os.Chmod(dst, 0o444); err != nil {
			return err
		}
	}
	return freeze(destination)
}

func freeze(root string) error {
	var dirs []string
	err := filepath.Walk(root, func(name string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		if info.IsDir() {
			dirs = append(dirs, name)
			return nil
		}
		return os.Chmod(name, 0o444)
	})
	if err != nil {
		return err
	}
	sort.Slice(dirs, func(i, j int) bool { return len(dirs[i]) > len(dirs[j]) })
	for _, dir := range dirs {
		if err := os.Chmod(dir, 0o555); err != nil {
			return err
		}
	}
	return nil
}

func safeSlashPath(name string) bool {
	if name == "" || strings.Contains(name, "\\") || strings.HasPrefix(name, "/") {
		return false
	}
	for _, part := range strings.Split(name, "/") {
		if part == "" || part == "." || part == ".." {
			return false
		}
	}
	return true
}

func (c *Cache) quarantine(name, label string) error {
	if _, err := os.Lstat(name); errors.Is(err, os.ErrNotExist) {
		return nil
	} else if err != nil {
		return err
	}
	// Rename beside the corrupted object. Some hardened filesystems reject a
	// cross-parent rename of an immutable directory; same-parent quarantine is
	// still atomic and removes the object from its trusted content address.
	target := filepath.Join(filepath.Dir(name), fmt.Sprintf(".corrupt-%s-%d", label, time.Now().UnixNano()))
	return os.Rename(name, target)
}

func quarantineProjectView(name string) error {
	if _, err := os.Lstat(name); errors.Is(err, os.ErrNotExist) {
		return nil
	} else if err != nil {
		return err
	}
	return os.Rename(name, filepath.Join(filepath.Dir(name), fmt.Sprintf(".corrupt-view-%d", time.Now().UnixNano())))
}

func secureMkdirUnder(root, leaf string) error {
	rootInfo, err := os.Lstat(root)
	if err != nil || !rootInfo.IsDir() || rootInfo.Mode()&os.ModeSymlink != 0 {
		return errors.New("project root must be a real directory")
	}
	rel, err := filepath.Rel(root, leaf)
	if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return errors.New("view path escaped project")
	}
	current := root
	for _, part := range strings.Split(rel, string(filepath.Separator)) {
		current = filepath.Join(current, part)
		if err := os.Mkdir(current, 0o700); err != nil && !errors.Is(err, os.ErrExist) {
			return err
		}
		info, err := os.Lstat(current)
		if err != nil || !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
			return fmt.Errorf("view component %q is not a real directory", current)
		}
	}
	return nil
}

func verifyEntryLayout(entry string) error {
	info, err := os.Lstat(entry)
	if err != nil {
		return err
	}
	if !info.IsDir() || info.Mode()&os.ModeSymlink != 0 || info.Mode().Perm()&0o222 != 0 {
		return errors.New("cache entry root is not an immutable directory")
	}
	children, err := os.ReadDir(entry)
	if err != nil {
		return err
	}
	if len(children) != 2 {
		return errors.New("cache entry contains unexpected top-level content")
	}
	seen := map[string]bool{}
	for _, child := range children {
		seen[child.Name()] = true
	}
	if !seen["source"] || !seen["integrity.json"] {
		return errors.New("cache entry layout is incomplete")
	}
	return nil
}

func makeWritable(root string) {
	_ = filepath.Walk(root, func(name string, info os.FileInfo, err error) error {
		if err == nil && info.IsDir() {
			_ = os.Chmod(name, 0o700)
		}
		return nil
	})
}

type contextReader struct {
	ctx context.Context
	r   io.Reader
}

func (r contextReader) Read(p []byte) (int, error) {
	if err := r.ctx.Err(); err != nil {
		return 0, err
	}
	return r.r.Read(p)
}
