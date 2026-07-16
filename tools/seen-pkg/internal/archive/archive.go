// Package archive validates and extracts Seen source package archives.
//
// Validation and extraction are separate passes. No archive-controlled path is
// created until every header and every regular file has passed the complete
// policy, the archive digest is known, and the root manifest binding hook has
// accepted the exact Seen.toml bytes.
package archive

import (
	"archive/tar"
	"bufio"
	"bytes"
	"compress/gzip"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"hash"
	"io"
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"
	"time"
	"unicode"
	"unicode/utf8"

	"golang.org/x/text/cases"
	"golang.org/x/text/unicode/norm"
)

var portableFold = cases.Fold()

// Limits is the executable archive-policy-v1 resource budget.
type Limits struct {
	CompressedBytes  int64
	ExpandedBytes    int64
	EntryCount       int
	RegularFileBytes int64
	PathBytes        int
	PathDepth        int
	CompressionRatio float64
	ValidationTime   time.Duration
	InspectPrefix    int64
}

// DefaultLimits matches contracts/package-registry/v1/fixtures/archive-policy-v1.json.
func DefaultLimits() Limits {
	return Limits{
		CompressedBytes:  25 * 1024 * 1024,
		ExpandedBytes:    100 * 1024 * 1024,
		EntryCount:       4096,
		RegularFileBytes: 10 * 1024 * 1024,
		PathBytes:        240,
		PathDepth:        16,
		CompressionRatio: 100,
		ValidationTime:   30 * time.Second,
		InspectPrefix:    4096,
	}
}

// ManifestBinder strictly parses and checks the exact root Seen.toml bytes and
// recomputes include/asset membership from the complete regular-file path list.
// Implementations must reject unknown/duplicate fields and reservation drift.
type ManifestBinder interface {
	BindManifest(manifest []byte, effectivePaths []string) error
}

// ManifestBindingFunc adapts a function to ManifestBinder.
type ManifestBindingFunc func(manifest []byte, effectivePaths []string) error

func (f ManifestBindingFunc) BindManifest(manifest []byte, effectivePaths []string) error {
	return f(manifest, effectivePaths)
}

// Options supplies the signed digest, hard limits, and mandatory manifest hook.
type Options struct {
	ExpectedSHA256 string
	Limits         Limits
	Binder         ManifestBinder
}

// Entry is an immutable preflight record used to verify the extraction pass.
type Entry struct {
	Path   string `json:"path"`
	Kind   string `json:"kind"`
	Size   int64  `json:"size"`
	SHA256 string `json:"sha256,omitempty"`
}

// Report records the archive facts established before promotion.
type Report struct {
	ArchiveSHA256 string
	Compressed    int64
	Expanded      int64
	Manifest      []byte
	Entries       []Entry
}

// Error is a stable, machine-readable policy failure.
type Error struct {
	Code  string
	Entry int
	Err   error
}

func (e *Error) Error() string {
	where := ""
	if e.Entry >= 0 {
		where = fmt.Sprintf(" at entry %d", e.Entry)
	}
	if e.Err == nil {
		return e.Code + where
	}
	return e.Code + where + ": " + e.Err.Error()
}

func (e *Error) Unwrap() error { return e.Err }

func failure(code string, entry int, err error) error {
	return &Error{Code: code, Entry: entry, Err: err}
}

func normalizeLimits(l Limits) Limits {
	d := DefaultLimits()
	if l.CompressedBytes <= 0 {
		l.CompressedBytes = d.CompressedBytes
	}
	if l.ExpandedBytes <= 0 {
		l.ExpandedBytes = d.ExpandedBytes
	}
	if l.EntryCount <= 0 {
		l.EntryCount = d.EntryCount
	}
	if l.RegularFileBytes <= 0 {
		l.RegularFileBytes = d.RegularFileBytes
	}
	if l.PathBytes <= 0 {
		l.PathBytes = d.PathBytes
	}
	if l.PathDepth <= 0 {
		l.PathDepth = d.PathDepth
	}
	if l.CompressionRatio <= 0 {
		l.CompressionRatio = d.CompressionRatio
	}
	if l.ValidationTime <= 0 {
		l.ValidationTime = d.ValidationTime
	}
	if l.InspectPrefix <= 0 {
		l.InspectPrefix = d.InspectPrefix
	}
	return l
}

func validateDigest(digest string) error {
	if len(digest) != 64 || strings.ToLower(digest) != digest {
		return errors.New("lowercase SHA-256 digest required")
	}
	_, err := hex.DecodeString(digest)
	return err
}

// Preflight performs the complete non-materializing validation pass.
func Preflight(ctx context.Context, archivePath string, options Options) (Report, error) {
	limits := normalizeLimits(options.Limits)
	if options.Binder == nil {
		return Report{}, failure("archive_manifest_binding_required", -1, errors.New("manifest binder is required"))
	}
	if err := validateDigest(options.ExpectedSHA256); err != nil {
		return Report{}, failure("archive_digest_invalid", -1, err)
	}
	ctx, cancel := boundedContext(ctx, limits.ValidationTime)
	defer cancel()

	actual, compressed, err := hashCompressed(ctx, archivePath, limits.CompressedBytes)
	if err != nil {
		return Report{}, err
	}
	if actual != options.ExpectedSHA256 {
		return Report{}, failure("archive_digest_mismatch", -1, fmt.Errorf("signed=%s received=%s", options.ExpectedSHA256, actual))
	}

	stream, err := openStream(ctx, archivePath, limits)
	if err != nil {
		return Report{}, streamError(err, -1)
	}
	defer stream.Close()

	report := Report{ArchiveSHA256: actual, Compressed: compressed}
	seen := make(map[string]int)
	portable := make(map[string]string)
	entryKinds := make(map[string]string)
	manifestCount := 0
	tr := tar.NewReader(stream.reader())
	for index := 0; ; index++ {
		if err := ctx.Err(); err != nil {
			return Report{}, failure("archive_validation_timeout", -1, err)
		}
		hdr, err := tr.Next()
		if errors.Is(err, io.EOF) {
			break
		}
		if err != nil {
			return Report{}, stream.readError(err, index)
		}
		if index >= limits.EntryCount {
			return Report{}, failure("archive_entry_count_limit", index, errors.New("entry limit exceeded"))
		}
		name, kind, err := inspectHeader(hdr, limits)
		if err != nil {
			return Report{}, withEntry(err, index)
		}
		if prior, ok := seen[name]; ok {
			return Report{}, failure("archive_duplicate_path", index, fmt.Errorf("duplicates entry %d", prior))
		}
		seen[name] = index
		folded := portableFold.String(norm.NFC.String(name))
		if prior, ok := portable[folded]; ok && prior != name {
			return Report{}, failure("archive_portable_case_collision", index, fmt.Errorf("%q collides with %q", name, prior))
		}
		portable[folded] = name
		entryKinds[name] = kind

		entry := Entry{Path: name, Kind: kind, Size: hdr.Size}
		if kind == "file" {
			report.Expanded += hdr.Size
			if report.Expanded > limits.ExpandedBytes {
				return Report{}, failure("archive_expanded_size_limit", index, errors.New("expanded byte limit exceeded"))
			}
			h := sha256.New()
			var manifestBuffer bytes.Buffer
			var contentWriter io.Writer = h
			if name == "Seen.toml" {
				contentWriter = io.MultiWriter(h, &manifestBuffer)
			}
			prefix := make([]byte, minInt64(limits.InspectPrefix, hdr.Size))
			if _, err := io.ReadFull(contextReader{ctx: ctx, r: tr}, prefix); err != nil {
				return Report{}, stream.readError(err, index)
			}
			if code := classify(name, prefix); code != "" {
				return Report{}, failure(code, index, fmt.Errorf("forbidden content at %q", name))
			}
			if _, err := contentWriter.Write(prefix); err != nil {
				return Report{}, failure("archive_parse_failed", index, err)
			}
			remaining := hdr.Size - int64(len(prefix))
			if remaining > 0 {
				if _, err := io.CopyN(contentWriter, contextReader{ctx: ctx, r: tr}, remaining); err != nil {
					return Report{}, stream.readError(err, index)
				}
			}
			entry.SHA256 = hex.EncodeToString(h.Sum(nil))
			if name == "Seen.toml" {
				manifestCount++
				if hdr.Size > limits.RegularFileBytes {
					return Report{}, failure("archive_file_size_limit", index, errors.New("manifest exceeds file limit"))
				}
				// Manifest is capped by the per-file limit, so retaining its exact raw
				// bytes does not create an additional unbounded allocation.
				report.Manifest = append([]byte(nil), manifestBuffer.Bytes()...)
			}
		}
		report.Entries = append(report.Entries, entry)
	}
	if err := stream.finish(); err != nil {
		return Report{}, streamError(err, -1)
	}
	if stream.digest() != options.ExpectedSHA256 || stream.count.n != compressed {
		return Report{}, failure("archive_changed_during_validation", -1, errors.New("compressed bytes changed during preflight"))
	}
	if manifestCount == 0 {
		return Report{}, failure("archive_manifest_missing", -1, errors.New("root Seen.toml is required"))
	}
	if manifestCount != 1 {
		return Report{}, failure("archive_manifest_duplicate", -1, errors.New("exactly one root Seen.toml is required"))
	}
	if err := validateHierarchy(entryKinds); err != nil {
		return Report{}, err
	}
	if compressed > 0 && float64(report.Expanded)/float64(compressed) > limits.CompressionRatio {
		return Report{}, failure("archive_compression_ratio_limit", -1, errors.New("compression ratio limit exceeded"))
	}
	paths := make([]string, 0, len(report.Entries))
	for i := range report.Entries {
		if report.Entries[i].Kind == "file" {
			paths = append(paths, report.Entries[i].Path)
		}
	}
	if err := options.Binder.BindManifest(append([]byte(nil), report.Manifest...), append([]string(nil), paths...)); err != nil {
		return Report{}, failure("archive_manifest_binding_failed", -1, err)
	}
	return report, nil
}

// Extract performs preflight, reopens the archive for a second verification
// pass, and atomically renames a read-only staging tree to destination.
func Extract(ctx context.Context, archivePath, destination string, options Options) (Report, error) {
	report, err := Preflight(ctx, archivePath, options)
	if err != nil {
		return Report{}, err
	}
	if _, err := os.Lstat(destination); err == nil {
		return Report{}, failure("archive_destination_exists", -1, errors.New("destination already exists"))
	} else if !errors.Is(err, os.ErrNotExist) {
		return Report{}, failure("archive_extract_failed", -1, err)
	}
	parent := filepath.Dir(destination)
	if err := os.MkdirAll(parent, 0o700); err != nil {
		return Report{}, failure("archive_extract_failed", -1, err)
	}
	stage, err := os.MkdirTemp(parent, ".seen-extract-*")
	if err != nil {
		return Report{}, failure("archive_extract_failed", -1, err)
	}
	committed := false
	defer func() {
		if !committed {
			makeWritable(stage)
			_ = os.RemoveAll(stage)
		}
	}()
	limits := normalizeLimits(options.Limits)
	extractCtx, cancel := boundedContext(ctx, limits.ValidationTime)
	defer cancel()
	if err := extractPass(extractCtx, archivePath, stage, report, limits); err != nil {
		return Report{}, err
	}
	if err := freezeTree(stage); err != nil {
		return Report{}, failure("archive_extract_failed", -1, err)
	}
	if err := os.Rename(stage, destination); err != nil {
		return Report{}, failure("archive_atomic_promotion_failed", -1, err)
	}
	committed = true
	if dir, err := os.Open(parent); err == nil {
		_ = dir.Sync()
		_ = dir.Close()
	}
	return report, nil
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

func boundedContext(ctx context.Context, d time.Duration) (context.Context, context.CancelFunc) {
	if deadline, ok := ctx.Deadline(); ok && time.Until(deadline) <= d {
		return context.WithCancel(ctx)
	}
	return context.WithTimeout(ctx, d)
}

func hashCompressed(ctx context.Context, name string, max int64) (string, int64, error) {
	f, info, err := openRegularFile(name)
	if err != nil {
		return "", 0, failure("archive_read_failed", -1, err)
	}
	defer f.Close()
	if info.Size() > max {
		return "", info.Size(), failure("archive_compressed_size_limit", -1, errors.New("compressed byte limit exceeded"))
	}
	h := sha256.New()
	n, err := io.Copy(h, io.LimitReader(contextReader{ctx: ctx, r: f}, max+1))
	if err != nil {
		if errors.Is(err, context.DeadlineExceeded) {
			return "", n, failure("archive_validation_timeout", -1, err)
		}
		return "", n, failure("archive_read_failed", -1, err)
	}
	if n > max {
		return "", n, failure("archive_compressed_size_limit", -1, errors.New("compressed byte limit exceeded"))
	}
	return hex.EncodeToString(h.Sum(nil)), n, nil
}

type tarStream struct {
	file          *os.File
	buf           *bufio.Reader
	gz            *gzip.Reader
	rawHash       hash.Hash
	count         *countingReader
	expanded      *countingReader
	maxCompressed int64
	maxExpanded   int64
}

func openStream(ctx context.Context, name string, limits Limits) (*tarStream, error) {
	f, info, err := openRegularFile(name)
	if err != nil {
		return nil, err
	}
	if info.Size() > limits.CompressedBytes {
		_ = f.Close()
		return nil, failure("archive_compressed_size_limit", -1, errors.New("compressed byte limit exceeded"))
	}
	count := &countingReader{r: io.LimitReader(f, limits.CompressedBytes+1)}
	h := sha256.New()
	buf := bufio.NewReader(contextReader{ctx: ctx, r: io.TeeReader(count, h)})
	gz, err := gzip.NewReader(buf)
	if err != nil {
		_ = f.Close()
		return nil, err
	}
	gz.Multistream(false)
	expanded := &countingReader{r: io.LimitReader(contextReader{ctx: ctx, r: gz}, limits.ExpandedBytes+1)}
	return &tarStream{file: f, buf: buf, gz: gz, rawHash: h, count: count, expanded: expanded, maxCompressed: limits.CompressedBytes, maxExpanded: limits.ExpandedBytes}, nil
}

func openRegularFile(name string) (*os.File, os.FileInfo, error) {
	pathInfo, err := os.Lstat(name)
	if err != nil {
		return nil, nil, err
	}
	if !pathInfo.Mode().IsRegular() || pathInfo.Mode()&os.ModeSymlink != 0 {
		return nil, nil, errors.New("archive must be a real regular file")
	}
	f, err := os.Open(name)
	if err != nil {
		return nil, nil, err
	}
	openInfo, err := f.Stat()
	if err != nil || !openInfo.Mode().IsRegular() || !os.SameFile(pathInfo, openInfo) {
		_ = f.Close()
		return nil, nil, errors.New("archive changed before it could be opened")
	}
	return f, openInfo, nil
}

func (s *tarStream) reader() io.Reader { return s.expanded }

func (s *tarStream) finish() error {
	if _, err := io.Copy(io.Discard, s.expanded); err != nil {
		if s.count.n > s.maxCompressed {
			return failure("archive_compressed_size_limit", -1, errors.New("compressed byte limit exceeded"))
		}
		if s.expanded.n > s.maxExpanded {
			return failure("archive_expanded_size_limit", -1, errors.New("expanded byte limit exceeded"))
		}
		return err
	}
	if _, err := s.buf.Peek(1); err == nil {
		return errors.New("trailing or concatenated gzip stream")
	} else if !errors.Is(err, io.EOF) {
		return err
	}
	if s.count.n > s.maxCompressed {
		return failure("archive_compressed_size_limit", -1, errors.New("compressed byte limit exceeded"))
	}
	if s.expanded.n > s.maxExpanded {
		return failure("archive_expanded_size_limit", -1, errors.New("expanded byte limit exceeded"))
	}
	return nil
}

func (s *tarStream) digest() string { return hex.EncodeToString(s.rawHash.Sum(nil)) }

func (s *tarStream) readError(err error, entry int) error {
	if s.count.n > s.maxCompressed {
		return failure("archive_compressed_size_limit", entry, errors.New("compressed byte limit exceeded"))
	}
	if s.expanded.n > s.maxExpanded {
		return failure("archive_expanded_size_limit", entry, errors.New("expanded byte limit exceeded"))
	}
	return streamError(err, entry)
}

type countingReader struct {
	r io.Reader
	n int64
}

func (r *countingReader) Read(p []byte) (int, error) {
	n, err := r.r.Read(p)
	r.n += int64(n)
	return n, err
}

func streamError(err error, entry int) error {
	var policy *Error
	if errors.As(err, &policy) {
		return withEntry(policy, entry)
	}
	if errors.Is(err, context.DeadlineExceeded) || errors.Is(err, context.Canceled) {
		return failure("archive_validation_timeout", entry, err)
	}
	return failure("archive_parse_failed", entry, err)
}

func (s *tarStream) Close() error {
	first := s.gz.Close()
	if err := s.file.Close(); first == nil {
		first = err
	}
	return first
}

func inspectHeader(h *tar.Header, limits Limits) (string, string, error) {
	if h == nil {
		return "", "", failure("archive_parse_failed", -1, errors.New("nil tar header"))
	}
	for key := range h.PAXRecords {
		if strings.HasPrefix(strings.ToLower(key), "gnu.sparse") {
			return "", "", failure("archive_sparse_forbidden", -1, errors.New("sparse PAX header"))
		}
		if key != "path" && key != "mtime" && key != "atime" && key != "ctime" {
			return "", "", failure("archive_pax_header_forbidden", -1, fmt.Errorf("unsupported PAX key %q", key))
		}
	}
	kind := ""
	switch h.Typeflag {
	case tar.TypeReg, tar.TypeRegA:
		kind = "file"
		if h.Size < 0 || h.Size > limits.RegularFileBytes {
			return "", "", failure("archive_file_size_limit", -1, fmt.Errorf("file size %d exceeds limit", h.Size))
		}
		if h.Mode&0o111 != 0 {
			return "", "", failure("archive_executable_forbidden", -1, errors.New("executable mode bits are forbidden"))
		}
	case tar.TypeDir:
		kind = "directory"
		if h.Size != 0 {
			return "", "", failure("archive_parse_failed", -1, errors.New("directory has nonzero size"))
		}
	case tar.TypeGNUSparse:
		return "", "", failure("archive_sparse_forbidden", -1, errors.New("sparse entries are forbidden"))
	default:
		return "", "", failure("archive_entry_type_forbidden", -1, fmt.Errorf("tar type %d is forbidden", h.Typeflag))
	}
	name, err := validatePath(h.Name, limits)
	if err != nil {
		return "", "", err
	}
	return name, kind, nil
}

func validatePath(name string, limits Limits) (string, error) {
	if name == "" || !utf8.ValidString(name) {
		return "", failure("archive_path_encoding", -1, errors.New("valid UTF-8 path required"))
	}
	if len([]byte(name)) > limits.PathBytes {
		return "", failure("archive_path_length_limit", -1, errors.New("path byte limit exceeded"))
	}
	if strings.Contains(name, "\\") {
		if len(name) >= 2 && ((name[0] >= 'A' && name[0] <= 'Z') || (name[0] >= 'a' && name[0] <= 'z')) && name[1] == ':' {
			return "", failure("archive_path_absolute", -1, errors.New("drive-qualified path"))
		}
		return "", failure("archive_path_backslash", -1, errors.New("backslashes are forbidden"))
	}
	if path.IsAbs(name) || strings.HasPrefix(name, "//") || (len(name) >= 2 && name[1] == ':') {
		return "", failure("archive_path_absolute", -1, errors.New("absolute path"))
	}
	for _, r := range name {
		if unicode.IsControl(r) || r == 0x7f {
			return "", failure("archive_path_control_character", -1, errors.New("control character in path"))
		}
	}
	if !norm.NFC.IsNormalString(name) {
		return "", failure("archive_unicode_normalization_collision", -1, errors.New("path is not NFC"))
	}
	parts := strings.Split(name, "/")
	if len(parts) > limits.PathDepth {
		return "", failure("archive_path_depth_limit", -1, errors.New("path depth limit exceeded"))
	}
	if IsReservedPackageStatePath(name) {
		return "", failure("archive_path_invalid", -1, errors.New("package-manager state paths are forbidden in hosted archives"))
	}
	for _, part := range parts {
		if part == "" {
			return "", failure("archive_path_empty_segment", -1, errors.New("empty path segment"))
		}
		if part == "." || part == ".." {
			return "", failure("archive_path_traversal", -1, errors.New("dot path segment"))
		}
		if strings.ContainsAny(part, `:*?"<>|`) || strings.HasSuffix(part, ".") || strings.HasSuffix(part, " ") {
			return "", failure("archive_path_not_portable", -1, fmt.Errorf("path segment %q is not portable", part))
		}
		device := strings.ToUpper(strings.SplitN(part, ".", 2)[0])
		if device == "CON" || device == "PRN" || device == "AUX" || device == "NUL" || (len(device) == 4 && (strings.HasPrefix(device, "COM") || strings.HasPrefix(device, "LPT")) && device[3] >= '1' && device[3] <= '9') {
			return "", failure("archive_path_not_portable", -1, fmt.Errorf("reserved device segment %q", part))
		}
	}
	return name, nil
}

// IsReservedPackageStatePath reports paths that could impersonate the
// project-owned package-resolution state. Package archives must never carry a
// .seen directory or a package-map.tsv file: the compiler discovers that map
// while walking parent directories, so accepting either would let archive
// content shadow the complete map written by the package client.
func IsReservedPackageStatePath(name string) bool {
	parts := strings.Split(name, "/")
	for _, part := range parts {
		if strings.EqualFold(part, ".seen") {
			return true
		}
	}
	return len(parts) != 0 && strings.EqualFold(parts[len(parts)-1], "package-map.tsv")
}

func withEntry(err error, entry int) error {
	var policy *Error
	if errors.As(err, &policy) {
		return &Error{Code: policy.Code, Entry: entry, Err: policy.Err}
	}
	return failure("archive_parse_failed", entry, err)
}

func validateHierarchy(kinds map[string]string) error {
	for name := range kinds {
		parent := path.Dir(name)
		for parent != "." && parent != "/" {
			if kinds[parent] == "file" {
				return failure("archive_path_conflict", -1, fmt.Errorf("file %q is parent of %q", parent, name))
			}
			parent = path.Dir(parent)
		}
	}
	return nil
}

var prebuiltSuffixes = []string{
	".o", ".obj", ".a", ".lib", ".so", ".dylib", ".dll", ".exe", ".wasm",
	".class", ".jar", ".pyc", ".pyo", ".bc",
}

var scriptExtensions = map[string]bool{
	".sh": true, ".bash": true, ".zsh": true, ".fish": true, ".ps1": true,
	".cmd": true, ".bat": true, ".py": true, ".rb": true, ".js": true,
}

var lifecycleStems = map[string]bool{
	"preinstall": true, "install": true, "postinstall": true, "prebuild": true,
	"build": true, "postbuild": true, "prepare": true, "configure": true,
	"bootstrap": true, "prepublish": true, "postpublish": true, "release": true,
}

var forbiddenScriptSegments = map[string]bool{
	"scripts": true, ".hooks": true, "hooks": true, "lifecycle": true,
	"build-scripts": true, "install-scripts": true,
}

var binaryMagic = [][]byte{
	{0x7f, 'E', 'L', 'F'}, {'M', 'Z'}, {0xfe, 0xed, 0xfa, 0xce},
	{0xfe, 0xed, 0xfa, 0xcf}, {0xce, 0xfa, 0xed, 0xfe}, {0xcf, 0xfa, 0xed, 0xfe},
	{0x00, 'a', 's', 'm'}, {'!', '<', 'a', 'r', 'c', 'h', '>', '\n'},
	{'B', 'C', 0xc0, 0xde}, {0xca, 0xfe, 0xba, 0xbe}, {'P', 'K', 0x03, 0x04},
}

var scriptPrefixes = [][]byte{
	{'#', '!'}, {'@', 'e', 'c', 'h', 'o', ' ', 'o', 'f', 'f'},
	{'$', 'E', 'r', 'r', 'o', 'r', 'A', 'c', 't', 'i', 'o', 'n', 'P', 'r', 'e', 'f', 'e', 'r', 'e', 'n', 'c', 'e'},
}

func classify(name string, prefix []byte) string {
	lower := strings.ToLower(name)
	base := path.Base(lower)
	if base == "a.out" || base == "core" {
		return "archive_prebuilt_artifact_forbidden"
	}
	for _, suffix := range prebuiltSuffixes {
		if strings.HasSuffix(base, suffix) {
			return "archive_prebuilt_artifact_forbidden"
		}
	}
	for _, magic := range binaryMagic {
		if len(prefix) >= len(magic) && string(prefix[:len(magic)]) == string(magic) {
			return "archive_prebuilt_artifact_forbidden"
		}
	}
	ext := path.Ext(base)
	stem := strings.TrimSuffix(base, ext)
	if lifecycleStems[stem] {
		return "archive_lifecycle_script_forbidden"
	}
	underForbiddenPath := false
	for _, segment := range strings.Split(lower, "/")[:max(0, len(strings.Split(lower, "/"))-1)] {
		if forbiddenScriptSegments[segment] {
			underForbiddenPath = true
			break
		}
	}
	if underForbiddenPath && scriptExtensions[ext] {
		return "archive_lifecycle_script_forbidden"
	}
	if underForbiddenPath {
		for _, marker := range scriptPrefixes {
			if len(prefix) >= len(marker) && strings.EqualFold(string(prefix[:len(marker)]), string(marker)) {
				return "archive_lifecycle_script_forbidden"
			}
		}
	}
	return ""
}

func extractPass(ctx context.Context, archivePath, stage string, report Report, limits Limits) error {
	stream, err := openStream(ctx, archivePath, limits)
	if err != nil {
		return streamError(err, -1)
	}
	defer stream.Close()
	tr := tar.NewReader(stream.reader())
	for i, expected := range report.Entries {
		if err := ctx.Err(); err != nil {
			return failure("archive_validation_timeout", i, err)
		}
		hdr, err := tr.Next()
		if err != nil {
			return stream.readError(err, i)
		}
		name, kind, err := inspectHeader(hdr, limits)
		if err != nil {
			return withEntry(err, i)
		}
		if name != expected.Path || kind != expected.Kind || hdr.Size != expected.Size {
			return failure("archive_changed_between_passes", i, errors.New("header differs from preflight"))
		}
		target := filepath.Join(stage, filepath.FromSlash(name))
		rel, err := filepath.Rel(stage, target)
		if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
			return failure("archive_path_traversal", i, errors.New("extraction path escaped staging root"))
		}
		if kind == "directory" {
			if err := os.Mkdir(target, 0o700); err != nil && !errors.Is(err, os.ErrExist) {
				return failure("archive_extract_failed", i, err)
			}
			continue
		}
		if err := os.MkdirAll(filepath.Dir(target), 0o700); err != nil {
			return failure("archive_extract_failed", i, err)
		}
		out, err := os.OpenFile(target, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
		if err != nil {
			return failure("archive_extract_failed", i, err)
		}
		h := sha256.New()
		_, copyErr := io.CopyN(io.MultiWriter(out, h), contextReader{ctx: ctx, r: tr}, hdr.Size)
		syncErr := out.Sync()
		closeErr := out.Close()
		if copyErr != nil {
			return stream.readError(copyErr, i)
		}
		if syncErr != nil || closeErr != nil {
			return failure("archive_extract_failed", i, errors.Join(syncErr, closeErr))
		}
		if actual := hex.EncodeToString(h.Sum(nil)); actual != expected.SHA256 {
			return failure("archive_changed_between_passes", i, errors.New("file digest differs from preflight"))
		}
		if err := os.Chmod(target, 0o444); err != nil {
			return failure("archive_extract_failed", i, err)
		}
	}
	if _, err := tr.Next(); !errors.Is(err, io.EOF) {
		if err == nil {
			return failure("archive_changed_between_passes", len(report.Entries), errors.New("archive gained an entry"))
		}
		return stream.readError(err, len(report.Entries))
	}
	if err := stream.finish(); err != nil {
		return streamError(err, -1)
	}
	if stream.digest() != report.ArchiveSHA256 || stream.count.n != report.Compressed {
		return failure("archive_changed_between_passes", -1, errors.New("compressed digest changed during extraction"))
	}
	return nil
}

func freezeTree(root string) error {
	var dirs []string
	err := filepath.Walk(root, func(name string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		if info.Mode()&os.ModeSymlink != 0 || (!info.Mode().IsRegular() && !info.IsDir()) {
			return errors.New("unexpected extracted file type")
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

func makeWritable(root string) {
	_ = filepath.Walk(root, func(name string, info os.FileInfo, err error) error {
		if err == nil && info.IsDir() {
			_ = os.Chmod(name, 0o700)
		}
		return nil
	})
}

func minInt64(a, b int64) int {
	if a < b {
		return int(a)
	}
	return int(b)
}
