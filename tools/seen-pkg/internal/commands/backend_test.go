package commands

import (
	"bytes"
	"context"
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

func TestProductionFetchValidatesArtifactAndRemovesStaleMap(t *testing.T) {
	root := t.TempDir()
	project := filepath.Join(root, "project")
	artifact := filepath.Join(root, "dist", "mathx.seenpkg")
	writeTestFile(t, filepath.Join(artifact, "seenpkg.toml"), "name = \"mathx\"\nversion = \"0.1.0\"\n")
	writeTestFile(t, filepath.Join(artifact, "objects.tsv"), "objects/value.o\tsrc/value.seen\n")
	writeTestFile(t, filepath.Join(artifact, "objects", "value.o"), "object")
	writeTestFile(t, filepath.Join(artifact, "src", "value.seen"), "pub fun value() r: Int { return 1 }\n")
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
