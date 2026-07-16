package lockfile

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"

	"github.com/codeyousef/seen/tools/seen-pkg/internal/model"
)

const testOrigin = "https://seen.dev.yousef.codes/packages"

func hash(c byte) string {
	value := make([]byte, 64)
	for i := range value {
		value[i] = c
	}
	return string(value)
}
func testLock() *model.Lock {
	digest := hash('a')
	target, _ := model.TargetPath("seen/json", "2.1.0", digest)
	edge := model.Edge{Alias: "json_pkg", Package: "seen/json", RegistryOrigin: testOrigin, Requirement: "^2.0.0", ResolvedVersion: "2.1.0", ResolvedArchiveSHA256: digest, Allow: []model.Capability{model.CapabilityNetwork}}
	return &model.Lock{Version: 2, ManifestSHA256: hash('f'), Root: model.Root{Name: "example_app", Version: "1.0.0", Dependencies: []model.Edge{edge}}, Packages: []model.LockedPackage{{Package: "seen/json", Version: "2.1.0", Source: "hosted-registry", RegistryOrigin: testOrigin, ArchiveSHA256: digest, TargetPath: target, MetadataVersion: 42, Capabilities: []model.Capability{model.CapabilityNetwork}, Grants: []model.Capability{model.CapabilityNetwork}}}}
}

func TestDeterministicRoundTrip(t *testing.T) {
	t.Parallel()
	lock := testLock()
	first, err := Marshal(lock)
	if err != nil {
		t.Fatal(err)
	}
	parsed, err := Parse(first)
	if err != nil {
		t.Fatal(err)
	}
	second, err := Marshal(parsed)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(first, second) {
		t.Fatalf("nondeterministic roundtrip\n%s\n---\n%s", first, second)
	}
}
func TestGraphClosureAndCapabilityBindings(t *testing.T) {
	t.Parallel()
	lock := testLock()
	lock.Packages[0].Grants = nil
	if err := Validate(lock); err == nil {
		t.Fatal("missing root grant accepted")
	}
	lock = testLock()
	lock.Root.Dependencies[0].Allow = nil
	if err := Validate(lock); err == nil {
		t.Fatal("missing edge allow accepted")
	}
	lock = testLock()
	lock.Root.Dependencies = nil
	if err := Validate(lock); err == nil {
		t.Fatal("unreachable package accepted")
	}
}
func TestAtomicWrite(t *testing.T) {
	t.Parallel()
	directory := t.TempDir()
	filename := filepath.Join(directory, "Seen.lock")
	if err := Write(filename, testLock()); err != nil {
		t.Fatal(err)
	}
	content, err := os.ReadFile(filename)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := Parse(content); err != nil {
		t.Fatal(err)
	}
	replacement := testLock()
	replacement.ManifestSHA256 = hash('e')
	if err := Write(filename, replacement); err != nil {
		t.Fatalf("replace existing Seen.lock: %v", err)
	}
	content, err = os.ReadFile(filename)
	if err != nil {
		t.Fatal(err)
	}
	parsed, err := Parse(content)
	if err != nil || parsed.ManifestSHA256 != hash('e') {
		t.Fatalf("replacement lock = %#v, %v", parsed, err)
	}
	matches, err := filepath.Glob(filepath.Join(directory, ".Seen.lock.tmp-*"))
	if err != nil || len(matches) != 0 {
		t.Fatalf("temp files = %v, %v", matches, err)
	}
}
func TestRejectsUnknownFieldsAndManifestDrift(t *testing.T) {
	t.Parallel()
	content, err := Marshal(testLock())
	if err != nil {
		t.Fatal(err)
	}
	content = append(content, []byte("unexpected = true\n")...)
	if _, err := Parse(content); err == nil {
		t.Fatal("unknown field accepted")
	}
	lock := testLock()
	if err := Enforce(lock, hash('e'), lock.Root); err == nil {
		t.Fatal("manifest drift accepted")
	}
}
