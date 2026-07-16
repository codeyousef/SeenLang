package commands

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	seenarchive "github.com/codeyousef/seen/tools/seen-pkg/internal/archive"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/manifest"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/model"
)

func TestRegistrySmokeFixtureIsFreshAndSourceProofReady(t *testing.T) {
	root := filepath.Join("..", "..", "..", "..", "examples", "registry-smoke")
	parsed, err := manifest.Load(filepath.Join(root, "Seen.toml"), manifest.Options{
		DefaultRegistryOrigin: "https://seen.dev.yousef.codes/packages",
		RequireManifestV1:     true,
	})
	if err != nil {
		t.Fatal(err)
	}
	_, document, err := parsedPublishManifest(parsed.Raw)
	if err != nil {
		t.Fatal(err)
	}
	project, _ := document["project"].(map[string]any)
	if parsed.Project.Version != "0.1.1" || project["license"] != "MIT" ||
		project["repository"] != "https://github.com/codeyousef/SeenLang" {
		t.Fatalf("unexpected smoke project binding: %+v", project)
	}
	if parsed.Package == nil || parsed.Package.Identity != "seen/registry-smoke" ||
		parsed.Package.Visibility != "public" {
		t.Fatalf("unexpected smoke package binding: %+v", parsed.Package)
	}
	files, err := selectedPackageFiles(root, parsed)
	if err != nil {
		t.Fatal(err)
	}
	if got, want := strings.Join(files, "\n"), strings.Join([]string{
		"LICENSE",
		"Seen.toml",
		"src/registry_smoke.seen",
	}, "\n"); got != want {
		t.Fatalf("smoke archive files:\n%s\nwant:\n%s", got, want)
	}
	if _, err := Pack(context.Background(), root, filepath.Join(t.TempDir(), "registry-smoke-0.1.1.seenpkg.tgz")); err != nil {
		t.Fatal(err)
	}
}

func TestPackIsDeterministicAndSourceOnly(t *testing.T) {
	t.Parallel()
	project := t.TempDir()
	manifest := `manifest-version = 1
[project]
name = "math_core"
version = "1.2.3"
[package]
identity = "alice/mathx"
visibility = "public"
include = ["src/**/*.seen", "README.md"]
assets = []
capabilities = []
[dependencies]
`
	if err := os.WriteFile(filepath.Join(project, "Seen.toml"), []byte(manifest), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(project, "src", "nested"), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(project, "src", "nested", "value.seen"), []byte("fun value() r: Int { return 1 }\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(project, "README.md"), []byte("math\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	one := filepath.Join(t.TempDir(), "one.tgz")
	two := filepath.Join(t.TempDir(), "two.tgz")
	first, err := Pack(context.Background(), project, one)
	if err != nil {
		t.Fatal(err)
	}
	second, err := Pack(context.Background(), project, two)
	if err != nil {
		t.Fatal(err)
	}
	if first.SHA256 != second.SHA256 || first.Length != second.Length {
		t.Fatalf("archives differ: %#v %#v", first, second)
	}
	third, err := Pack(context.Background(), project, one)
	if err != nil {
		t.Fatalf("replace existing archive: %v", err)
	}
	if third.SHA256 != first.SHA256 || third.Length != first.Length {
		t.Fatalf("replacement archive differs: %#v %#v", first, third)
	}
}
func TestPackRejectsUndeclaredSymlink(t *testing.T) {
	t.Parallel()
	project := t.TempDir()
	manifest := `manifest-version = 1
[project]
name = "pkg"
version = "1.0.0"
[package]
identity = "alice/pkg"
visibility = "public"
include = ["src/**"]
assets = []
capabilities = []
[dependencies]
`
	if err := os.WriteFile(filepath.Join(project, "Seen.toml"), []byte(manifest), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(filepath.Join(project, "src"), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("../Seen.toml", filepath.Join(project, "src", "link")); err != nil {
		t.Skip(err)
	}
	if _, err := Pack(context.Background(), project, filepath.Join(t.TempDir(), "pkg.tgz")); err == nil {
		t.Fatal("symlink accepted")
	}
}

func TestPackageBindersRejectReservedPackageState(t *testing.T) {
	raw := []byte(`manifest-version = 1
[project]
name = "pkg"
version = "1.0.0"
[package]
identity = "alice/pkg"
visibility = "public"
include = ["src/**"]
assets = []
capabilities = []
[dependencies]
`)
	parsed, err := manifest.ParseWithOptions(raw, manifest.Options{
		DefaultRegistryOrigin: "https://seen.dev.yousef.codes/packages",
		RequireManifestV1:     true,
	})
	if err != nil {
		t.Fatal(err)
	}
	paths := []string{"Seen.toml", "src/.seen/package-map.tsv"}
	binders := []struct {
		name   string
		binder seenarchive.ManifestBinder
	}{
		{"pack", &packageBinder{expected: parsed, expectedRaw: raw}},
		{"signed", &signedPackageBinder{candidate: model.Candidate{
			Package:        "alice/pkg",
			Version:        "1.0.0",
			RegistryOrigin: "https://seen.dev.yousef.codes/packages",
			Capabilities:   []model.Capability{},
			Dependencies:   []model.Edge{},
		}}},
	}
	for _, test := range binders {
		t.Run(test.name, func(t *testing.T) {
			if err := test.binder.BindManifest(raw, paths); err == nil {
				t.Fatal("reserved package-manager state accepted")
			}
		})
	}
}
