package archive

import (
	"archive/tar"
	"bytes"
	"compress/gzip"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
	"time"
)

type tarEntry struct {
	name     string
	typeflag byte
	mode     int64
	body     []byte
	link     string
	pax      map[string]string
}

func writeArchive(t *testing.T, entries []tarEntry) (string, string) {
	t.Helper()
	var compressed bytes.Buffer
	gz := gzip.NewWriter(&compressed)
	tw := tar.NewWriter(gz)
	for _, entry := range entries {
		typeflag := entry.typeflag
		if typeflag == 0 {
			typeflag = tar.TypeReg
		}
		mode := entry.mode
		if mode == 0 {
			mode = 0o644
		}
		h := &tar.Header{
			Name: entry.name, Typeflag: typeflag, Mode: mode,
			Size: int64(len(entry.body)), Linkname: entry.link,
			PAXRecords: entry.pax, ModTime: time.Unix(0, 0),
		}
		if typeflag != tar.TypeReg && typeflag != tar.TypeRegA {
			h.Size = 0
		}
		if err := tw.WriteHeader(h); err != nil {
			t.Fatal(err)
		}
		if h.Size > 0 {
			if _, err := tw.Write(entry.body); err != nil {
				t.Fatal(err)
			}
		}
	}
	if err := tw.Close(); err != nil {
		t.Fatal(err)
	}
	if err := gz.Close(); err != nil {
		t.Fatal(err)
	}
	name := filepath.Join(t.TempDir(), "package.seenpkg.tgz")
	if err := os.WriteFile(name, compressed.Bytes(), 0o600); err != nil {
		t.Fatal(err)
	}
	h := sha256.Sum256(compressed.Bytes())
	return name, hex.EncodeToString(h[:])
}

func noopBinder(t *testing.T) ManifestBinder {
	t.Helper()
	return ManifestBindingFunc(func(manifest []byte, paths []string) error {
		if !bytes.Contains(manifest, []byte("[project]")) {
			return errors.New("manifest was not passed exactly")
		}
		if len(paths) == 0 || paths[0] != "Seen.toml" {
			return errors.New("effective paths missing")
		}
		return nil
	})
}

func archiveCode(t *testing.T, err error) string {
	t.Helper()
	var policy *Error
	if !errors.As(err, &policy) {
		t.Fatalf("expected archive Error, got %T: %v", err, err)
	}
	return policy.Code
}

func options(t *testing.T, digest string) Options {
	return Options{ExpectedSHA256: digest, Limits: DefaultLimits(), Binder: noopBinder(t)}
}

func TestExtractValidArchiveIsReadOnlyAndBound(t *testing.T) {
	manifest := []byte("[project]\nversion = \"1.2.3\"\n[package]\nidentity = \"alice/mathx\"\n")
	archivePath, digest := writeArchive(t, []tarEntry{
		{name: "Seen.toml", body: manifest},
		{name: "src/main.seen", body: []byte("fun main() { println(\"ok\") }\n")},
		{name: "README.md", body: []byte("docs\n")},
	})
	dest := filepath.Join(t.TempDir(), "installed")
	t.Cleanup(func() { makeWritable(dest) })
	report, err := Extract(context.Background(), archivePath, dest, options(t, digest))
	if err != nil {
		t.Fatal(err)
	}
	if report.ArchiveSHA256 != digest || !bytes.Equal(report.Manifest, manifest) {
		t.Fatalf("bad report: %+v", report)
	}
	want := []string{"Seen.toml", "src/main.seen", "README.md"}
	var got []string
	for _, entry := range report.Entries {
		got = append(got, entry.Path)
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("paths = %#v", got)
	}
	err = filepath.Walk(dest, func(_ string, info os.FileInfo, err error) error {
		if err == nil && info.Mode().Perm()&0o222 != 0 {
			t.Errorf("writable extracted path: %v", info.Mode())
		}
		return err
	})
	if err != nil {
		t.Fatal(err)
	}
}

func TestRealAdversarialArchivesFailBeforeExtraction(t *testing.T) {
	manifest := tarEntry{name: "Seen.toml", body: []byte("[project]\nversion = \"1.0.0\"\n")}
	tests := []struct {
		name  string
		entry tarEntry
		code  string
	}{
		{"parent traversal", tarEntry{name: "../escape.seen", body: []byte("bad")}, "archive_path_traversal"},
		{"absolute", tarEntry{name: "/tmp/escape.seen", body: []byte("bad")}, "archive_path_absolute"},
		{"backslash", tarEntry{name: `src\\escape.seen`, body: []byte("bad")}, "archive_path_backslash"},
		{"windows device", tarEntry{name: "assets/CON.txt", body: []byte("bad")}, "archive_path_not_portable"},
		{"alternate stream", tarEntry{name: "assets/data.txt:payload", body: []byte("bad")}, "archive_path_not_portable"},
		{"symlink", tarEntry{name: "src/main.seen", typeflag: tar.TypeSymlink, link: "../../escape"}, "archive_entry_type_forbidden"},
		{"hardlink", tarEntry{name: "src/main.seen", typeflag: tar.TypeLink, link: "Seen.toml"}, "archive_entry_type_forbidden"},
		{"device", tarEntry{name: "dev/tty", typeflag: tar.TypeChar}, "archive_entry_type_forbidden"},
		{"executable", tarEntry{name: "src/main.seen", mode: 0o755, body: []byte("source")}, "archive_executable_forbidden"},
		{"binary magic", tarEntry{name: "assets/innocent.dat", body: append([]byte{0x7f, 'E', 'L', 'F'}, []byte("payload")...)}, "archive_prebuilt_artifact_forbidden"},
		{"binary suffix", tarEntry{name: "assets/innocent.o", body: []byte("plain")}, "archive_prebuilt_artifact_forbidden"},
		{"lifecycle", tarEntry{name: "scripts/install.sh", body: []byte("#!/bin/sh\n")}, "archive_lifecycle_script_forbidden"},
		{"root package state", tarEntry{name: ".seen/package-map.tsv", body: []byte("/\tdep\t.\n")}, "archive_path_invalid"},
		{"nested package state", tarEntry{name: "src/.SeEn/metadata", body: []byte("state")}, "archive_path_invalid"},
		{"package map outside state", tarEntry{name: "src/PACKAGE-MAP.TSV", body: []byte("/\tdep\t.\n")}, "archive_path_invalid"},
		{"non nfc", tarEntry{name: "src/cafe\u0301.seen", body: []byte("source")}, "archive_unicode_normalization_collision"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			archivePath, digest := writeArchive(t, []tarEntry{manifest, tc.entry})
			dest := filepath.Join(t.TempDir(), "dest")
			_, err := Extract(context.Background(), archivePath, dest, options(t, digest))
			if got := archiveCode(t, err); got != tc.code {
				t.Fatalf("code = %q, want %q (%v)", got, tc.code, err)
			}
			if _, statErr := os.Stat(dest); !errors.Is(statErr, os.ErrNotExist) {
				t.Fatalf("destination materialized: %v", statErr)
			}
		})
	}
}

func TestPAXAbuseAndCaseCollision(t *testing.T) {
	manifest := tarEntry{name: "Seen.toml", body: []byte("[project]\nversion = \"1.0.0\"\n")}
	archivePath, digest := writeArchive(t, []tarEntry{
		manifest,
		{name: "src/placeholder.seen", body: []byte("bad"), pax: map[string]string{"comment": "untrusted override"}},
	})
	_, err := Preflight(context.Background(), archivePath, options(t, digest))
	if got := archiveCode(t, err); got != "archive_pax_header_forbidden" {
		t.Fatalf("PAX abuse code = %q: %v", got, err)
	}

	archivePath, digest = writeArchive(t, []tarEntry{
		manifest,
		{name: "src/Math.seen", body: []byte("a")},
		{name: "src/math.seen", body: []byte("b")},
	})
	_, err = Preflight(context.Background(), archivePath, options(t, digest))
	if got := archiveCode(t, err); got != "archive_portable_case_collision" {
		t.Fatalf("case collision code = %q: %v", got, err)
	}
}

func TestArchiveBombAndTruncationLimits(t *testing.T) {
	manifest := tarEntry{name: "Seen.toml", body: []byte("[project]\nversion = \"1.0.0\"\n")}
	archivePath, digest := writeArchive(t, []tarEntry{manifest, {name: "assets/data.txt", body: bytes.Repeat([]byte("A"), 16*1024)}})
	opts := options(t, digest)
	opts.Limits.CompressionRatio = 2
	_, err := Preflight(context.Background(), archivePath, opts)
	if got := archiveCode(t, err); got != "archive_compression_ratio_limit" {
		t.Fatalf("ratio code = %q: %v", got, err)
	}
	opts = options(t, digest)
	opts.Limits.ExpandedBytes = 1024 // Includes tar headers/padding, not only file bodies.
	_, err = Preflight(context.Background(), archivePath, opts)
	if got := archiveCode(t, err); got != "archive_expanded_size_limit" {
		t.Fatalf("raw expanded stream code = %q: %v", got, err)
	}

	data, err := os.ReadFile(archivePath)
	if err != nil {
		t.Fatal(err)
	}
	truncated := filepath.Join(t.TempDir(), "truncated.tgz")
	data = data[:len(data)-8]
	if err := os.WriteFile(truncated, data, 0o600); err != nil {
		t.Fatal(err)
	}
	h := sha256.Sum256(data)
	_, err = Preflight(context.Background(), truncated, options(t, hex.EncodeToString(h[:])))
	if got := archiveCode(t, err); got != "archive_parse_failed" {
		t.Fatalf("truncation code = %q: %v", got, err)
	}
}

func TestArchiveChangeBetweenPassesCannotPromote(t *testing.T) {
	manifest := []byte("[project]\nversion = \"1.0.0\"\n")
	original, digest := writeArchive(t, []tarEntry{
		{name: "Seen.toml", body: manifest},
		{name: "src/main.seen", body: []byte("original")},
	})
	replacement, _ := writeArchive(t, []tarEntry{
		{name: "Seen.toml", body: manifest},
		{name: "src/main.seen", body: []byte("attacker")},
	})
	replacementBytes, err := os.ReadFile(replacement)
	if err != nil {
		t.Fatal(err)
	}
	mutated := false
	opts := options(t, digest)
	opts.Binder = ManifestBindingFunc(func([]byte, []string) error {
		if !mutated {
			mutated = true
			return os.WriteFile(original, replacementBytes, 0o600)
		}
		return nil
	})
	destination := filepath.Join(t.TempDir(), "must-not-exist")
	_, err = Extract(context.Background(), original, destination, opts)
	if got := archiveCode(t, err); got != "archive_changed_between_passes" {
		t.Fatalf("changed archive code = %q: %v", got, err)
	}
	if _, statErr := os.Stat(destination); !errors.Is(statErr, os.ErrNotExist) {
		t.Fatalf("changed archive was promoted: %v", statErr)
	}
}

func TestManifestBinderAndDigestFailClosed(t *testing.T) {
	archivePath, digest := writeArchive(t, []tarEntry{{name: "Seen.toml", body: []byte("[project]\nversion = \"1.0.0\"\n")}})
	opts := options(t, digest)
	opts.Binder = ManifestBindingFunc(func([]byte, []string) error { return errors.New("reservation mismatch") })
	_, err := Preflight(context.Background(), archivePath, opts)
	if got := archiveCode(t, err); got != "archive_manifest_binding_failed" {
		t.Fatalf("binder code = %q", got)
	}

	opts = options(t, strings.Repeat("0", 64))
	_, err = Preflight(context.Background(), archivePath, opts)
	if got := archiveCode(t, err); got != "archive_digest_mismatch" {
		t.Fatalf("digest code = %q", got)
	}
}

func TestSymlinkedArchiveIsRejected(t *testing.T) {
	archivePath, digest := writeArchive(t, []tarEntry{{name: "Seen.toml", body: []byte("[project]\nversion = \"1.0.0\"\n")}})
	link := filepath.Join(t.TempDir(), "package-link.tgz")
	if err := os.Symlink(archivePath, link); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	_, err := Preflight(context.Background(), link, options(t, digest))
	if got := archiveCode(t, err); got != "archive_read_failed" {
		t.Fatalf("symlinked archive code = %q: %v", got, err)
	}
}
