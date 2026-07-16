package manifest

import (
	"strings"
	"testing"
)

const goodManifest = `manifest-version = 1

[project]
name = "registry_example"
version = "1.0.0"

[package]
identity = "alice/registry-example"
visibility = "public"
include = ["src/**/*.seen", "README.md"]
assets = []
capabilities = ["network"]

[registries]
default = "https://seen.dev.yousef.codes/packages"

[dependencies]
json_pkg = { package = "seen/json", version = "^2.1.0", allow = [] }
tls_pkg = { package = "alice/tls", version = "~1.4", allow = ["network", "ffi"] }

[package-grants]
"alice/tls" = ["ffi", "network"]

[build]
target = "native"
optimize = "speed"
`

const strictManifest = `manifest-version = 1

[project]
name = "registry_example"
version = "1.0.0"
language = "en"
description = "strict manifest fixture"
authors = ["Seen Language Team"]
edition = "2025"
modules = ["src/main.seen"]
license = "Apache-2.0"
repository = "https://example.invalid/registry-example"

[package]
identity = "alice/registry-example"
visibility = "public"
include = ["src/**/*.seen"]
assets = []
capabilities = ["network"]

[registries]
default = "https://seen.dev.yousef.codes/packages"

[dependencies]
hosted = { package = "seen/json", version = "^2.1.0", allow = [] }
source_tree = { path = "../source-tree" }
archive = { artifact = "../archive.seenpkg" }
platform = { system = true, path = "native/lib" }

[package-grants]
"seen/json" = []

[native.dependencies]
`

func TestParseTypedFieldsAndToleratesUnrelatedSections(t *testing.T) {
	t.Parallel()
	parsed, err := Parse([]byte(goodManifest))
	if err != nil {
		t.Fatal(err)
	}
	if parsed.Project.Name != "registry_example" || parsed.Package == nil || parsed.Package.Identity != "alice/registry-example" {
		t.Fatalf("unexpected manifest: %#v", parsed)
	}
	if len(parsed.Dependencies) != 2 || parsed.Dependencies[0].Alias != "json_pkg" || parsed.Dependencies[1].RegistryOrigin == "" {
		t.Fatalf("dependencies not canonical: %#v", parsed.Dependencies)
	}
	if len(parsed.Grants["alice/tls"]) != 2 {
		t.Fatalf("grants not parsed: %#v", parsed.Grants)
	}
}

func TestStrictManifestV1AllowsNormativeFieldsAndLocalDependencyForms(t *testing.T) {
	t.Parallel()
	parsed, err := ParseWithOptions([]byte(strictManifest), Options{RequireManifestV1: true})
	if err != nil {
		t.Fatal(err)
	}
	if parsed.Package == nil || len(parsed.Dependencies) != 4 {
		t.Fatalf("strict manifest = %#v", parsed)
	}
	wantKinds := []string{"artifact", "registry", "system", "path"}
	for index, dependency := range parsed.Dependencies {
		if string(dependency.Kind) != wantKinds[index] {
			t.Fatalf("dependency %d kind = %q, want %q", index, dependency.Kind, wantKinds[index])
		}
	}
}

func TestStrictManifestV1RejectsUnknownFields(t *testing.T) {
	t.Parallel()
	cases := map[string]string{
		"top-level field":  strings.Replace(strictManifest, "manifest-version = 1\n", "manifest-version = 1\nsurprise = true\n", 1),
		"top-level table":  strictManifest + "\n[build]\ntarget = \"native\"\n",
		"project field":    strings.Replace(strictManifest, "[project]\n", "[project]\nsurprise = true\n", 1),
		"package field":    strings.Replace(strictManifest, "[package]\n", "[package]\nsurprise = true\n", 1),
		"dependency field": strings.Replace(strictManifest, `version = "^2.1.0", allow = []`, `version = "^2.1.0", allow = [], surprise = true`, 1),
		"native field":     strings.Replace(strictManifest, "[native.dependencies]\n", "[native]\nsurprise = true\n[native.dependencies]\n", 1),
	}
	for name, input := range cases {
		name, input := name, input
		t.Run(name, func(t *testing.T) {
			t.Parallel()
			if _, err := ParseWithOptions([]byte(input), Options{RequireManifestV1: true}); err == nil {
				t.Fatal("unknown field was accepted")
			}
		})
	}
}

func TestStrictManifestV1ValidatesProjectSchemaFields(t *testing.T) {
	t.Parallel()
	cases := map[string]string{
		"language":   strings.Replace(strictManifest, `language = "en"`, `language = "klingon"`, 1),
		"license":    strings.Replace(strictManifest, `license = "Apache-2.0"`, `license = ""`, 1),
		"repository": strings.Replace(strictManifest, `repository = "https://example.invalid/registry-example"`, `repository = "not a URI"`, 1),
	}
	for name, input := range cases {
		name, input := name, input
		t.Run(name, func(t *testing.T) {
			t.Parallel()
			if _, err := ParseWithOptions([]byte(input), Options{RequireManifestV1: true}); err == nil {
				t.Fatal("schema-invalid project field was accepted")
			}
		})
	}
}

func TestStrictManifestV1RejectsLegacyStringDependency(t *testing.T) {
	t.Parallel()
	input := strings.Replace(
		strictManifest,
		`source_tree = { path = "../source-tree" }`,
		`source_tree = "../source-tree"`,
		1,
	)
	if _, err := ParseWithOptions([]byte(input), Options{RequireManifestV1: true}); err == nil {
		t.Fatal("legacy string dependency was accepted by strict manifest v1")
	}
	if _, err := Parse([]byte(input)); err != nil {
		t.Fatalf("legacy-compatible parser rejected string dependency: %v", err)
	}
}

func TestLocalDependencyRejectsEvenEmptyRegistryOnlyFields(t *testing.T) {
	t.Parallel()
	input := strings.Replace(
		strictManifest,
		`source_tree = { path = "../source-tree" }`,
		`source_tree = { path = "../source-tree", allow = [] }`,
		1,
	)
	if _, err := ParseWithOptions([]byte(input), Options{RequireManifestV1: true}); err == nil {
		t.Fatal("local path dependency accepted an allow field outside its schema variant")
	}
}

func TestStrictManifestV1RejectsDuplicateFields(t *testing.T) {
	t.Parallel()
	cases := map[string]string{
		"top-level":  strings.Replace(strictManifest, "manifest-version = 1\n", "manifest-version = 1\nmanifest-version = 1\n", 1),
		"project":    strings.Replace(strictManifest, `name = "registry_example"`, "name = \"registry_example\"\nname = \"registry_example\"", 1),
		"package":    strings.Replace(strictManifest, `visibility = "public"`, "visibility = \"public\"\nvisibility = \"public\"", 1),
		"dependency": strings.Replace(strictManifest, `version = "^2.1.0", allow = []`, `version = "^2.1.0", version = "^2.1.0", allow = []`, 1),
	}
	for name, input := range cases {
		name, input := name, input
		t.Run(name, func(t *testing.T) {
			t.Parallel()
			if _, err := ParseWithOptions([]byte(input), Options{RequireManifestV1: true}); err == nil {
				t.Fatal("duplicate field was accepted")
			}
		})
	}
}

func TestStrictManifestV1RequiresNormativeSections(t *testing.T) {
	t.Parallel()
	withoutDependencies := `manifest-version = 1
[project]
name = "pkg"
version = "1.0.0"
[package]
identity = "alice/pkg"
visibility = "public"
include = ["src/**"]
assets = []
capabilities = []
`
	if _, err := ParseWithOptions([]byte(withoutDependencies), Options{RequireManifestV1: true}); err == nil || !strings.Contains(err.Error(), "[dependencies]") {
		t.Fatalf("missing dependencies error = %v", err)
	}
	withoutPackage := `manifest-version = 1
[project]
name = "pkg"
version = "1.0.0"
[dependencies]
`
	if _, err := ParseWithOptions([]byte(withoutPackage), Options{RequireManifestV1: true}); err == nil || !strings.Contains(err.Error(), "[package]") {
		t.Fatalf("missing package error = %v", err)
	}
}

func TestRejectsMalformedAndAmbiguousDependencies(t *testing.T) {
	t.Parallel()
	cases := []string{
		strings.Replace(goodManifest, `version = "^2.1.0"`, `version = "latest"`, 1),
		strings.Replace(goodManifest, `package = "seen/json", version = "^2.1.0"`, `package = "seen/json", version = "^2.1.0", path = "../json"`, 1),
		strings.Replace(goodManifest, `allow = []`, `allow = ["network", "network"]`, 1),
		strings.Replace(goodManifest, `default = "https://seen.dev.yousef.codes/packages"`, `default = "http://seen.dev.yousef.codes/packages"`, 1),
	}
	for index, input := range cases {
		if _, err := Parse([]byte(input)); err == nil {
			t.Errorf("case %d unexpectedly succeeded", index)
		}
	}
}

func TestLegacyManifestRemainsReadable(t *testing.T) {
	t.Parallel()
	input := `[project]
name = "old-project"
version = "0.1.0"
[dependencies]
seen_std = "../seen_std"
[compiler-extra]
anything = true
`
	parsed, err := Parse([]byte(input))
	if err != nil {
		t.Fatal(err)
	}
	if len(parsed.Dependencies) != 1 || parsed.Dependencies[0].Path != "../seen_std" {
		t.Fatalf("legacy dependency = %#v", parsed.Dependencies)
	}
}

func TestLegacySystemDependencyRemainsReadable(t *testing.T) {
	t.Parallel()
	input := `[project]
name = "legacy"
version = "0.1.0"
[dependencies]
seen_platform = { system = true, path = "native/lib" }
`
	parsed, err := Parse([]byte(input))
	if err != nil {
		t.Fatal(err)
	}
	if len(parsed.Dependencies) != 1 || parsed.Dependencies[0].Kind != "system" || parsed.Dependencies[0].Path != "native/lib" {
		t.Fatalf("system dependency = %#v", parsed.Dependencies)
	}
	for _, invalid := range []string{
		strings.Replace(input, `, path = "native/lib"`, "", 1),
		strings.Replace(input, `native/lib`, `../native/lib`, 1),
		strings.Replace(input, `native/lib`, `/native/lib`, 1),
	} {
		if _, err := Parse([]byte(invalid)); err == nil {
			t.Errorf("unsafe system dependency accepted: %s", invalid)
		}
	}
}

func TestUnsignedLocalRegistryHasPathMigrationDiagnostic(t *testing.T) {
	t.Parallel()
	input := strings.Replace(goodManifest, `default = "https://seen.dev.yousef.codes/packages"`, `default = "../registry"`, 1)
	_, err := Parse([]byte(input))
	if err == nil || !strings.Contains(err.Error(), `dependency { path = "..." }`) {
		t.Fatalf("local registry diagnostic = %v", err)
	}
	pathManifest := `[project]
name = "app"
version = "1.0.0"
[dependencies]
mathx = { path = "../mathx" }
`
	if _, err := Parse([]byte(pathManifest)); err != nil {
		t.Fatalf("explicit development path rejected: %v", err)
	}
}
