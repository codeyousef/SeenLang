package commands

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/codeyousef/seen/tools/seen-pkg/internal/lockfile"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/model"
)

func writeTestFile(t *testing.T, name, content string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(name), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(name, []byte(content), 0o600); err != nil {
		t.Fatal(err)
	}
}

func runBackend(t *testing.T, arguments ...string) (int, string) {
	t.Helper()
	var stdout, stderr bytes.Buffer
	runner := Runner{Backend: NewProductionBackend(), Streams: Streams{Stdout: &stdout, Stderr: &stderr}}
	code := runner.Run(context.Background(), arguments)
	return code, stderr.String()
}

func localPackage(t *testing.T, root, name string) {
	t.Helper()
	writeTestFile(t, filepath.Join(root, "Seen.toml"), "[project]\nname = \""+name+"\"\nversion = \"0.1.0\"\n[dependencies]\n")
	writeTestFile(t, filepath.Join(root, "src", "value.seen"), "pub fun value() r: Int { return 1 }\n")
}

func prebuiltArtifactV2(t *testing.T, root string) {
	t.Helper()
	interfaceIndex := "schema\tseen-package-interface-v2\n" +
		"version\t2\n" +
		"layout_abi\tseen-layout-abi-v2\n" +
		"object_cache_abi\tseen-object-cache-abi-v2\n" +
		"module\tsrc/value.seen\tpackage\tvalue\tmodule-fingerprint\n" +
		"function\tsrc/value.seen\tpublic\tvalue\tdeclaration-fingerprint\tcanonical-declaration\n"
	digest := sha256.Sum256([]byte(interfaceIndex))
	manifest := "schema = \"seen-prebuilt-package-v2\"\n" +
		"version = 2\n" +
		"name = \"mathx\"\n" +
		"package_version = \"0.1.0\"\n" +
		"kind = \"prebuilt\"\n" +
		"language = \"en\"\n" +
		"interface_schema = \"seen-package-interface-v2\"\n" +
		"interface_version = 2\n" +
		"object_manifest_schema = \"seen-package-object-manifest-v2\"\n" +
		"object_manifest_version = 2\n" +
		"layout_abi = \"seen-layout-abi-v2\"\n" +
		"object_cache_abi = \"seen-object-cache-abi-v2\"\n" +
		"interface_path = \"src\"\n" +
		"interface_index = \"interface.index.tsv\"\n" +
		"object_manifest = \"objects.tsv\"\n" +
		"declaration_digest = \"" + hex.EncodeToString(digest[:]) + "\"\n" +
		"build_signature = \"target=linux-x86_64|pic=1|layout=seen-layout-abi-v2|object-cache=seen-object-cache-abi-v2\"\n"
	objectManifest := "schema\tseen-package-object-manifest-v2\n" +
		"version\t2\n" +
		"layout_abi\tseen-layout-abi-v2\n" +
		"object_cache_abi\tseen-object-cache-abi-v2\n" +
		"objects/value.o\tsrc/value.seen\n"
	writeTestFile(t, filepath.Join(root, "Seen.pkg.toml"), manifest)
	writeTestFile(t, filepath.Join(root, "interface.index.tsv"), interfaceIndex)
	writeTestFile(t, filepath.Join(root, "objects.tsv"), objectManifest)
	writeTestFile(t, filepath.Join(root, "objects", "value.o"), "object")
	writeTestFile(t, filepath.Join(root, "src", "value.seen"), "pub fun value() r: Int { return 1 }\n")
}

func thawLocalState(project string) {
	makeLocalTreeWritable(filepath.Join(project, ".seen"))
}

func TestProductionFetchMaterializesLegacyLocalPathView(t *testing.T) {
	root := t.TempDir()
	project := filepath.Join(root, "project")
	dependency := filepath.Join(root, "seen_std")
	if err := os.MkdirAll(project, 0o700); err != nil {
		t.Fatal(err)
	}
	localPackage(t, dependency, "seen_std")
	content := `[project]
name = "old-project"
version = "0.1.0"
[dependencies]
seen_std = "../seen_std"
seen_platform = { system = true, path = "native/lib" }
`
	manifestPath := filepath.Join(project, "Seen.toml")
	writeTestFile(t, manifestPath, content)
	defer thawLocalState(project)
	if code, stderr := runBackend(t, "fetch", manifestPath, "--quiet"); code != 0 {
		t.Fatalf("code=%d stderr=%s", code, stderr)
	}
	mapBytes, err := os.ReadFile(filepath.Join(project, ".seen", "package-map.tsv"))
	if err != nil {
		t.Fatal(err)
	}
	fields := strings.Split(strings.TrimSpace(string(mapBytes)), "\t")
	if len(fields) != 3 || fields[0] != project || fields[1] != "seen_std" {
		t.Fatalf("map=%q", mapBytes)
	}
	view := fields[2]
	if view == dependency || !strings.HasPrefix(view, filepath.Join(project, ".seen", "views")+string(filepath.Separator)) {
		t.Fatalf("dependency view escaped project state: %s", view)
	}
	info, err := os.Stat(view)
	if err != nil || info.Mode().Perm()&0o222 != 0 {
		t.Fatalf("view mode=%v err=%v", info, err)
	}
	if _, err := os.Stat(filepath.Join(view, "Seen.toml")); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(filepath.Join(project, "Seen.lock")); !os.IsNotExist(err) {
		t.Fatalf("legacy local-only fetch left Seen.lock: %v", err)
	}
}

func TestProductionFetchRejectsMissingLocalManifestBeforeMapPromotion(t *testing.T) {
	root := t.TempDir()
	project := filepath.Join(root, "project")
	missingManifest := filepath.Join(root, "dependency")
	if err := os.MkdirAll(missingManifest, 0o700); err != nil {
		t.Fatal(err)
	}
	writeTestFile(t, filepath.Join(project, "Seen.toml"), "[project]\nname = \"app\"\nversion = \"0.1.0\"\n[dependencies]\ndep = \"../dependency\"\n")
	oldMap := []byte("previous-authoritative-map\n")
	writeTestFile(t, filepath.Join(project, ".seen", "package-map.tsv"), string(oldMap))
	if code, _ := runBackend(t, "fetch", filepath.Join(project, "Seen.toml"), "--quiet"); code == 0 {
		t.Fatal("missing local Seen.toml accepted")
	}
	after, err := os.ReadFile(filepath.Join(project, ".seen", "package-map.tsv"))
	if err != nil || !bytes.Equal(after, oldMap) {
		t.Fatalf("map changed on failure: %q err=%v", after, err)
	}
}

func TestLocalViewMaterializesContainedFileAndDirectorySymlinks(t *testing.T) {
	root := t.TempDir()
	source := filepath.Join(root, "source")
	destination := filepath.Join(root, "view")
	writeTestFile(t, filepath.Join(source, "src", "core", "value.seen"), "value")
	writeTestFile(t, filepath.Join(source, "README.md"), "readme")
	if err := os.Symlink("core", filepath.Join(source, "src", "core_alias")); err != nil {
		t.Skipf("symlinks unavailable: %v", err)
	}
	if err := os.Symlink("../README.md", filepath.Join(source, "src", "readme_alias")); err != nil {
		t.Fatal(err)
	}
	if err := copyLocalTree(context.Background(), source, destination); err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{
		filepath.Join(destination, "src", "core_alias", "value.seen"),
		filepath.Join(destination, "src", "readme_alias"),
	} {
		info, err := os.Lstat(name)
		if err != nil || info.Mode()&os.ModeSymlink != 0 || !info.Mode().IsRegular() {
			t.Fatalf("materialized path %s info=%v err=%v", name, info, err)
		}
	}
}

func TestLocalViewRejectsEscapingSymlink(t *testing.T) {
	root := t.TempDir()
	source := filepath.Join(root, "source")
	writeTestFile(t, filepath.Join(source, "src", "value.seen"), "inside")
	writeTestFile(t, filepath.Join(root, "outside.seen"), "outside")
	if err := os.Symlink(filepath.Join(root, "outside.seen"), filepath.Join(source, "src", "escape")); err != nil {
		t.Skipf("symlinks unavailable: %v", err)
	}
	err := copyLocalTree(context.Background(), source, filepath.Join(root, "view"))
	if err == nil || !strings.Contains(err.Error(), "escapes package root") {
		t.Fatalf("escaping symlink error=%v", err)
	}
}

func TestLocalViewRejectsSymlinkDirectoryCycle(t *testing.T) {
	root := t.TempDir()
	source := filepath.Join(root, "source")
	writeTestFile(t, filepath.Join(source, "src", "value.seen"), "value")
	if err := os.Symlink("..", filepath.Join(source, "src", "cycle")); err != nil {
		t.Skipf("symlinks unavailable: %v", err)
	}
	err := copyLocalTree(context.Background(), source, filepath.Join(root, "view"))
	if err == nil || !strings.Contains(err.Error(), "symbolic link cycle") {
		t.Fatalf("cycle error=%v", err)
	}
}

func TestProductionFetchValidatesArtifactAndRemovesStaleMap(t *testing.T) {
	root := t.TempDir()
	project := filepath.Join(root, "project")
	artifact := filepath.Join(root, "dist", "mathx.seenpkg")
	prebuiltArtifactV2(t, artifact)
	writeTestFile(t, filepath.Join(project, "Seen.toml"), "[project]\nname = \"app\"\nversion = \"0.1.0\"\n[dependencies]\nmathx = { artifact = \"../dist/mathx.seenpkg\" }\n")
	writeTestFile(t, filepath.Join(project, ".seen", "package-map.tsv"), "stale\n")
	if code, stderr := runBackend(t, "fetch", filepath.Join(project, "Seen.toml"), "--quiet"); code != 0 {
		t.Fatalf("code=%d stderr=%s", code, stderr)
	}
	if _, err := os.Stat(filepath.Join(project, ".seen", "package-map.tsv")); !os.IsNotExist(err) {
		t.Fatalf("artifact-only fetch left authoritative map: %v", err)
	}
	if err := os.Remove(filepath.Join(artifact, "objects.tsv")); err != nil {
		t.Fatal(err)
	}
	if code, _ := runBackend(t, "fetch", filepath.Join(project, "Seen.toml"), "--quiet"); code == 0 {
		t.Fatal("artifact without object manifest accepted")
	}
}

func TestProductionFetchRejectsV1ArtifactWithRebuildGuidance(t *testing.T) {
	for _, manifestName := range []string{"Seen.pkg.toml", "seenpkg.toml"} {
		t.Run(manifestName, func(t *testing.T) {
			root := t.TempDir()
			project := filepath.Join(root, "project")
			artifact := filepath.Join(root, "dist", "mathx.seenpkg")
			writeTestFile(t, filepath.Join(artifact, manifestName), "name = \"mathx\"\nversion = 1\n")
			writeTestFile(t, filepath.Join(artifact, "objects.tsv"), "objects/value.o\tsrc/value.seen\n")
			writeTestFile(t, filepath.Join(artifact, "objects", "value.o"), "object")
			writeTestFile(t, filepath.Join(artifact, "src", "value.seen"), "pub fun value() r: Int { return 1 }\n")
			writeTestFile(t, filepath.Join(project, "Seen.toml"), "[project]\nname = \"app\"\nversion = \"0.1.0\"\n[dependencies]\nmathx = { artifact = \"../dist/mathx.seenpkg\" }\n")
			code, stderr := runBackend(t, "fetch", filepath.Join(project, "Seen.toml"), "--quiet")
			if code == 0 || !strings.Contains(stderr, "rebuild using Seen 0.11") {
				t.Fatalf("legacy artifact code=%d stderr=%s", code, stderr)
			}
		})
	}
}

func TestProductionFetchRejectsArtifactDeclarationDigestMismatch(t *testing.T) {
	root := t.TempDir()
	project := filepath.Join(root, "project")
	artifact := filepath.Join(root, "dist", "mathx.seenpkg")
	prebuiltArtifactV2(t, artifact)
	indexPath := filepath.Join(artifact, "interface.index.tsv")
	indexRaw, err := os.ReadFile(indexPath)
	if err != nil {
		t.Fatal(err)
	}
	writeTestFile(t, indexPath, string(indexRaw)+"# declaration drift\n")
	writeTestFile(t, filepath.Join(project, "Seen.toml"), "[project]\nname = \"app\"\nversion = \"0.1.0\"\n[dependencies]\nmathx = { artifact = \"../dist/mathx.seenpkg\" }\n")
	code, stderr := runBackend(t, "fetch", filepath.Join(project, "Seen.toml"), "--quiet")
	if code == 0 || !strings.Contains(stderr, "declaration fingerprint mismatch") ||
		!strings.Contains(stderr, "rebuild using Seen 0.11") {
		t.Fatalf("digest mismatch code=%d stderr=%s", code, stderr)
	}
}

func TestZeroHostedManifestV1LockModes(t *testing.T) {
	root := t.TempDir()
	project := filepath.Join(root, "project")
	dependency := filepath.Join(root, "dependency")
	localPackage(t, dependency, "dependency")
	manifestPath := filepath.Join(project, "Seen.toml")
	content := "manifest-version = 1\n[project]\nname = \"app\"\nversion = \"0.1.0\"\n[dependencies]\ndep = { path = \"../dependency\" }\n"
	writeTestFile(t, manifestPath, content)
	defer thawLocalState(project)
	if code, stderr := runBackend(t, "fetch", manifestPath, "--quiet"); code != 0 {
		t.Fatalf("initial fetch code=%d stderr=%s", code, stderr)
	}
	locked, err := lockfile.Load(filepath.Join(project, "Seen.lock"))
	if err != nil || len(locked.Packages) != 0 || len(locked.Root.Dependencies) != 0 {
		t.Fatalf("empty lock=%#v err=%v", locked, err)
	}
	if code, stderr := runBackend(t, "fetch", manifestPath, "--locked", "--quiet"); code != 0 {
		t.Fatalf("locked fetch code=%d stderr=%s", code, stderr)
	}
	mapBefore, err := os.ReadFile(filepath.Join(project, ".seen", "package-map.tsv"))
	if err != nil {
		t.Fatal(err)
	}
	writeTestFile(t, manifestPath, content+"# lock drift\n")
	if code, _ := runBackend(t, "fetch", manifestPath, "--locked", "--quiet"); code == 0 {
		t.Fatal("locked fetch accepted manifest drift")
	}
	mapAfter, err := os.ReadFile(filepath.Join(project, ".seen", "package-map.tsv"))
	if err != nil || !bytes.Equal(mapBefore, mapAfter) {
		t.Fatalf("failed locked fetch changed map: %q err=%v", mapAfter, err)
	}
	if code, _ := runBackend(t, "update", manifestPath, "--locked", "--quiet"); code == 0 {
		t.Fatal("update --locked accepted")
	}
}

func TestMixedHostedAndLocalRowsRemainComplete(t *testing.T) {
	root := t.TempDir()
	project := filepath.Join(root, "project")
	dependency := filepath.Join(root, "dependency")
	if err := os.MkdirAll(project, 0o700); err != nil {
		t.Fatal(err)
	}
	localPackage(t, dependency, "dependency")
	writeTestFile(t, filepath.Join(project, "Seen.toml"), "[project]\nname = \"app\"\nversion = \"0.1.0\"\n[dependencies]\nlocal_dep = { path = \"../dependency\" }\n")
	defer thawLocalState(project)
	localRows, err := materializeLocalDependencies(context.Background(), project, []model.Dependency{{Alias: "local_dep", Kind: model.DependencyPath, Path: "../dependency"}})
	if err != nil {
		t.Fatal(err)
	}
	hostedView := filepath.Join(project, ".seen", "views", "hosted", "source")
	writeTestFile(t, filepath.Join(hostedView, "Seen.toml"), "[project]\nname = \"hosted\"\nversion = \"1.0.0\"\n[dependencies]\n")
	if err := freezeLocalTree(filepath.Dir(hostedView)); err != nil {
		t.Fatal(err)
	}
	key := model.PackageKey{RegistryOrigin: "https://seen.dev.yousef.codes/packages", Package: "seen/hosted"}
	resolution := &model.Resolution{Root: model.Root{Dependencies: []model.Edge{{Alias: "hosted_dep", Package: key.Package, RegistryOrigin: key.RegistryOrigin}}}}
	hostedRows, err := packageMapRows(project, resolution, map[model.PackageKey]string{key: hostedView})
	if err != nil {
		t.Fatal(err)
	}
	if err := WritePackageMap(project, append(hostedRows, localRows...)); err != nil {
		t.Fatal(err)
	}
	content, err := os.ReadFile(filepath.Join(project, ".seen", "package-map.tsv"))
	if err != nil || !bytes.Contains(content, []byte("\thosted_dep\t")) || !bytes.Contains(content, []byte("\tlocal_dep\t")) {
		t.Fatalf("mixed map=%q err=%v", content, err)
	}
}

func TestProductionAddLocalPath(t *testing.T) {
	project := t.TempDir()
	manifestPath := filepath.Join(project, "Seen.toml")
	content := "[project]\nname = \"app\"\nversion = \"1.0.0\"\n[dependencies]\n"
	writeTestFile(t, manifestPath, content)
	if code, stderr := runBackend(t, "add", "local_math", "--path", "../math", "--manifest", manifestPath); code != 0 {
		t.Fatalf("code=%d stderr=%s", code, stderr)
	}
	updated, err := os.ReadFile(manifestPath)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Contains(updated, []byte(`local_math = { path = "../math" }`)) {
		t.Fatalf("manifest=%s", updated)
	}
}
