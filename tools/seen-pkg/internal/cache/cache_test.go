package cache

import (
	"archive/tar"
	"bytes"
	"compress/gzip"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"testing"

	seenarchive "github.com/codeyousef/seen/tools/seen-pkg/internal/archive"
)

func packageArchive(t *testing.T) (string, string) {
	t.Helper()
	var data bytes.Buffer
	gz := gzip.NewWriter(&data)
	tw := tar.NewWriter(gz)
	entries := []struct {
		name string
		body []byte
	}{
		{"Seen.toml", []byte("[project]\nversion = \"1.2.3\"\n[package]\nidentity = \"alice/mathx\"\n")},
		{"src/main.seen", []byte("fun answer() r: Int { return 42 }\n")},
	}
	for _, entry := range entries {
		if err := tw.WriteHeader(&tar.Header{Name: entry.name, Mode: 0o644, Size: int64(len(entry.body)), Typeflag: tar.TypeReg}); err != nil {
			t.Fatal(err)
		}
		if _, err := tw.Write(entry.body); err != nil {
			t.Fatal(err)
		}
	}
	if err := tw.Close(); err != nil {
		t.Fatal(err)
	}
	if err := gz.Close(); err != nil {
		t.Fatal(err)
	}
	archivePath := filepath.Join(t.TempDir(), "mathx.seenpkg.tgz")
	if err := os.WriteFile(archivePath, data.Bytes(), 0o600); err != nil {
		t.Fatal(err)
	}
	h := sha256.Sum256(data.Bytes())
	return archivePath, hex.EncodeToString(h[:])
}

func packageOptions(digest string) seenarchive.Options {
	return seenarchive.Options{
		ExpectedSHA256: digest,
		Limits:         seenarchive.DefaultLimits(),
		Binder: seenarchive.ManifestBindingFunc(func(manifest []byte, paths []string) error {
			if !bytes.Contains(manifest, []byte(`identity = "alice/mathx"`)) || len(paths) != 2 {
				return errors.New("manifest binding mismatch")
			}
			return nil
		}),
	}
}

func testKey(digest string) Key {
	return Key{Owner: "alice", Name: "mathx", Version: "1.2.3", SHA256: digest}
}

func cacheCode(t *testing.T, err error) string {
	t.Helper()
	var cacheErr *Error
	if !errors.As(err, &cacheErr) {
		t.Fatalf("expected cache Error, got %T: %v", err, err)
	}
	return cacheErr.Code
}

func TestInstallLookupAndProjectViewIsolation(t *testing.T) {
	archivePath, digest := packageArchive(t)
	c, err := New(filepath.Join(t.TempDir(), "cache"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { makeWritable(c.Root) })
	key := testKey(digest)
	cacheSource, err := c.Install(context.Background(), key, archivePath, packageOptions(digest))
	if err != nil {
		t.Fatal(err)
	}
	if got, err := c.Lookup(context.Background(), key); err != nil || got != cacheSource {
		t.Fatalf("lookup = %q, %v", got, err)
	}
	project := t.TempDir()
	t.Cleanup(func() { makeWritable(filepath.Join(project, ".seen")) })
	view, err := c.CreateProjectView(context.Background(), key, project)
	if err != nil {
		t.Fatal(err)
	}
	wantPrefix := filepath.Join(project, ".seen", "views") + string(filepath.Separator)
	if !stringsHasPathPrefix(view, wantPrefix) {
		t.Fatalf("view %q is not project-local", view)
	}
	viewSource := filepath.Join(view, "src", "main.seen")
	cacheFile := filepath.Join(cacheSource, "src", "main.seen")
	if err := os.Chmod(viewSource, 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(viewSource, []byte("tampered\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	// Project-local mutation cannot reach the shared inode.
	if _, err := c.Lookup(context.Background(), key); err != nil {
		t.Fatalf("view mutation corrupted shared cache: %v", err)
	}
	cacheBytes, err := os.ReadFile(cacheFile)
	if err != nil {
		t.Fatal(err)
	}
	if bytes.Equal(cacheBytes, []byte("tampered\n")) {
		t.Fatal("view and cache share a mutable inode")
	}
	// Re-materialization quarantines and replaces the damaged view.
	view, err = c.CreateProjectView(context.Background(), key, project)
	if err != nil {
		t.Fatal(err)
	}
	got, err := os.ReadFile(filepath.Join(view, "src", "main.seen"))
	if err != nil || !bytes.Equal(got, cacheBytes) {
		t.Fatalf("repaired view mismatch: %q, %v", got, err)
	}
}

func TestCorruptCacheIsRejectedAndRebuilt(t *testing.T) {
	archivePath, digest := packageArchive(t)
	c, err := New(filepath.Join(t.TempDir(), "cache"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { makeWritable(c.Root) })
	key := testKey(digest)
	source, err := c.Install(context.Background(), key, archivePath, packageOptions(digest))
	if err != nil {
		t.Fatal(err)
	}
	victim := filepath.Join(source, "src", "main.seen")
	if err := os.Chmod(victim, 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(victim, []byte("corrupt"), 0o644); err != nil {
		t.Fatal(err)
	}
	_, err = c.Lookup(context.Background(), key)
	if got := cacheCode(t, err); got != "cache_corrupt_entry" {
		t.Fatalf("code = %q: %v", got, err)
	}
	rebuilt, err := c.Install(context.Background(), key, archivePath, packageOptions(digest))
	if err != nil {
		t.Fatal(err)
	}
	if rebuilt == source {
		// The content-addressed path is intentionally stable; this assertion
		// documents that rebuild replaces bytes, not identity.
		t.Log("rebuilt at stable content-addressed path")
	}
	if _, err := c.Lookup(context.Background(), key); err != nil {
		t.Fatal(err)
	}
}

func TestForgedIntegrityCannotAuthorizeTamperedSource(t *testing.T) {
	archivePath, digest := packageArchive(t)
	c, err := New(filepath.Join(t.TempDir(), "cache"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { makeWritable(c.Root) })
	key := testKey(digest)
	source, err := c.Install(context.Background(), key, archivePath, packageOptions(digest))
	if err != nil {
		t.Fatal(err)
	}

	attackerBytes := []byte("fun answer() r: Int { return 99 }\n")
	victim := filepath.Join(source, "src", "main.seen")
	if err := os.Chmod(victim, 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(victim, attackerBytes, 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(victim, 0o444); err != nil {
		t.Fatal(err)
	}

	entry := filepath.Dir(source)
	integrityPath := filepath.Join(entry, "integrity.json")
	meta, err := readIntegrity(integrityPath)
	if err != nil {
		t.Fatal(err)
	}
	attackerDigest := sha256.Sum256(attackerBytes)
	for index := range meta.Files {
		if meta.Files[index].Path == "src/main.seen" {
			meta.Files[index].Size = int64(len(attackerBytes))
			meta.Files[index].SHA256 = hex.EncodeToString(attackerDigest[:])
		}
	}
	forged, err := json.Marshal(meta)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(integrityPath, 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(integrityPath, append(forged, '\n'), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(integrityPath, 0o444); err != nil {
		t.Fatal(err)
	}

	_, err = c.Lookup(context.Background(), key)
	if got := cacheCode(t, err); got != "cache_corrupt_entry" {
		t.Fatalf("forged inventory code = %q: %v", got, err)
	}
}

func TestInstallRebindsExistingContentAddressedEntry(t *testing.T) {
	archivePath, digest := packageArchive(t)
	c, err := New(filepath.Join(t.TempDir(), "cache"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { makeWritable(c.Root) })
	key := testKey(digest)
	if _, err := c.Install(context.Background(), key, archivePath, packageOptions(digest)); err != nil {
		t.Fatal(err)
	}

	called := false
	rejected := packageOptions(digest)
	rejected.Binder = seenarchive.ManifestBindingFunc(func([]byte, []string) error {
		called = true
		return errors.New("signed candidate does not match this manifest")
	})
	_, err = c.Install(context.Background(), key, archivePath, rejected)
	if !called {
		t.Fatal("existing cache entry bypassed the caller's manifest binder")
	}
	var archiveErr *seenarchive.Error
	if !errors.As(err, &archiveErr) || archiveErr.Code != "archive_manifest_binding_failed" {
		t.Fatalf("existing entry binder error = %T %v", err, err)
	}
	if _, err := c.Lookup(context.Background(), key); err != nil {
		t.Fatalf("binder rejection damaged valid cache entry: %v", err)
	}
}

func TestInvalidInstallSourceCannotEvictValidCache(t *testing.T) {
	archivePath, digest := packageArchive(t)
	c, err := New(filepath.Join(t.TempDir(), "cache"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { makeWritable(c.Root) })
	key := testKey(digest)
	if _, err := c.Install(context.Background(), key, archivePath, packageOptions(digest)); err != nil {
		t.Fatal(err)
	}

	invalid := filepath.Join(t.TempDir(), "attacker.tgz")
	if err := os.WriteFile(invalid, []byte("not the signed package archive"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := c.Install(context.Background(), key, invalid, packageOptions(digest)); err == nil {
		t.Fatal("invalid source was accepted")
	}
	if _, err := c.Lookup(context.Background(), key); err != nil {
		t.Fatalf("invalid source evicted the valid cache entry: %v", err)
	}
}

func TestInstallRebuildsIncompleteExistingEntry(t *testing.T) {
	archivePath, digest := packageArchive(t)
	c, err := New(filepath.Join(t.TempDir(), "cache"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { makeWritable(c.Root) })
	key := testKey(digest)
	source, err := c.Install(context.Background(), key, archivePath, packageOptions(digest))
	if err != nil {
		t.Fatal(err)
	}
	entry := filepath.Dir(source)
	if err := os.Chmod(entry, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(filepath.Join(entry, "integrity.json")); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(entry, 0o555); err != nil {
		t.Fatal(err)
	}

	rebuilt, err := c.Install(context.Background(), key, archivePath, packageOptions(digest))
	if err != nil {
		t.Fatalf("rebuild incomplete entry: %v", err)
	}
	if rebuilt != source {
		t.Fatalf("rebuilt source path = %q, want stable %q", rebuilt, source)
	}
	if _, err := c.Lookup(context.Background(), key); err != nil {
		t.Fatalf("rebuilt entry failed lookup: %v", err)
	}
}

func TestLookupRejectsSymlinkedIntegrityMetadata(t *testing.T) {
	archivePath, digest := packageArchive(t)
	c, err := New(filepath.Join(t.TempDir(), "cache"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { makeWritable(c.Root) })
	key := testKey(digest)
	source, err := c.Install(context.Background(), key, archivePath, packageOptions(digest))
	if err != nil {
		t.Fatal(err)
	}
	entry := filepath.Dir(source)
	integrityPath := filepath.Join(entry, "integrity.json")
	external := filepath.Join(t.TempDir(), "integrity.json")
	data, err := os.ReadFile(integrityPath)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(external, data, 0o444); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(entry, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(integrityPath); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(external, integrityPath); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	if err := os.Chmod(entry, 0o555); err != nil {
		t.Fatal(err)
	}

	_, err = c.Lookup(context.Background(), key)
	if got := cacheCode(t, err); got != "cache_integrity_invalid" {
		t.Fatalf("symlinked integrity code = %q: %v", got, err)
	}
}

func TestBlobDigestIsRevalidated(t *testing.T) {
	archivePath, digest := packageArchive(t)
	c, err := New(filepath.Join(t.TempDir(), "cache"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { makeWritable(c.Root) })
	key := testKey(digest)
	if _, err := c.Install(context.Background(), key, archivePath, packageOptions(digest)); err != nil {
		t.Fatal(err)
	}
	blob, err := c.BlobPath(digest)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(blob, 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(blob, []byte("wrong"), 0o644); err != nil {
		t.Fatal(err)
	}
	_, err = c.Lookup(context.Background(), key)
	if got := cacheCode(t, err); got != "cache_corrupt_blob" {
		t.Fatalf("code = %q: %v", got, err)
	}
}

func TestProjectViewRejectsSymlinkedSeenDirectory(t *testing.T) {
	archivePath, digest := packageArchive(t)
	c, err := New(filepath.Join(t.TempDir(), "cache"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { makeWritable(c.Root) })
	key := testKey(digest)
	if _, err := c.Install(context.Background(), key, archivePath, packageOptions(digest)); err != nil {
		t.Fatal(err)
	}
	project := t.TempDir()
	outside := t.TempDir()
	if err := os.Symlink(outside, filepath.Join(project, ".seen")); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	_, err = c.CreateProjectView(context.Background(), key, project)
	if got := cacheCode(t, err); got != "cache_view_root_invalid" {
		t.Fatalf("code = %q: %v", got, err)
	}
}

func stringsHasPathPrefix(name, prefix string) bool {
	return len(name) >= len(prefix) && name[:len(prefix)] == prefix
}
