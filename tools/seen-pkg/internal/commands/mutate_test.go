package commands

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/codeyousef/seen/tools/seen-pkg/internal/manifest"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/model"
)

func TestAddAndRemovePreserveUnrelatedManifestText(t *testing.T) {
	t.Parallel()
	filename := filepath.Join(t.TempDir(), "Seen.toml")
	original := `manifest-version = 1
[project]
name = "app"
version = "1.0.0"
[registries]
default = "https://seen.dev.yousef.codes/packages"
[dependencies]
# keep this comment
[build]
target = "native"
`
	if err := os.WriteFile(filename, []byte(original), 0o600); err != nil {
		t.Fatal(err)
	}
	dependency := model.Dependency{Alias: "json_pkg", Kind: model.DependencyRegistry, Package: "seen/json", Requirement: "^2.0.0", RegistryAlias: "default"}
	if err := AddDependency(filename, dependency); err != nil {
		t.Fatal(err)
	}
	content, err := os.ReadFile(filename)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(content), "# keep this comment") || !strings.Contains(string(content), `json_pkg = { package = "seen/json", version = "^2.0.0" }`) {
		t.Fatalf("content = %s", content)
	}
	if _, err := manifest.Parse(content); err != nil {
		t.Fatal(err)
	}
	if err := RemoveDependency(filename, "json_pkg"); err != nil {
		t.Fatal(err)
	}
	content, err = os.ReadFile(filename)
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(content), "json_pkg =") || !strings.Contains(string(content), "[build]") {
		t.Fatalf("content = %s", content)
	}
}
