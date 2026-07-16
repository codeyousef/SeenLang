package commands

import (
	"os"
	"path/filepath"
	"testing"
)

func TestWritePackageMapAtomicAndSorted(t *testing.T) {
	t.Parallel()
	project, err := filepath.Abs(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	views := filepath.Join(project, ".seen", "views")
	requester := filepath.Join(views, "root")
	depA := filepath.Join(views, "a")
	depB := filepath.Join(views, "b")
	for _, directory := range []string{requester, depA, depB} {
		if err := os.MkdirAll(directory, 0o700); err != nil {
			t.Fatal(err)
		}
		if err := os.Chmod(directory, 0o500); err != nil {
			t.Fatal(err)
		}
	}
	rows := []PackageMapRow{{project, "root_dep", depA}, {requester, "z_dep", depB}, {requester, "a_dep", depA}}
	if err := WritePackageMap(project, rows); err != nil {
		t.Fatal(err)
	}
	content, err := os.ReadFile(filepath.Join(project, ".seen", "package-map.tsv"))
	if err != nil {
		t.Fatal(err)
	}
	want := project + "\troot_dep\t" + depA + "\n" + requester + "\ta_dep\t" + depA + "\n" + requester + "\tz_dep\t" + depB + "\n"
	if string(content) != want {
		t.Fatalf("map = %q", content)
	}
	if err := WritePackageMap(project, []PackageMapRow{{project, "replacement", depB}}); err != nil {
		t.Fatalf("replace existing map: %v", err)
	}
	content, err = os.ReadFile(filepath.Join(project, ".seen", "package-map.tsv"))
	if err != nil || string(content) != project+"\treplacement\t"+depB+"\n" {
		t.Fatalf("replacement map = %q, %v", content, err)
	}
}

func TestWriteEmptyPackageMapCreatesSafeState(t *testing.T) {
	t.Parallel()
	project, err := filepath.Abs(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := WritePackageMap(project, nil); err != nil {
		t.Fatal(err)
	}
	content, err := os.ReadFile(filepath.Join(project, ".seen", "package-map.tsv"))
	if err != nil || len(content) != 0 {
		t.Fatalf("empty map = %q, %v", content, err)
	}
}

func TestWritePackageMapRejectsSymlinkedState(t *testing.T) {
	t.Parallel()
	project, err := filepath.Abs(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	outside := t.TempDir()
	if err := os.Symlink(outside, filepath.Join(project, ".seen")); err != nil {
		t.Skip(err)
	}
	if err := WritePackageMap(project, nil); err == nil {
		t.Fatal("symlinked state directory accepted")
	}
}

func TestWritePackageMapRejectsEscapesBeforePromotion(t *testing.T) {
	t.Parallel()
	project, err := filepath.Abs(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	views := filepath.Join(project, ".seen", "views")
	inside := filepath.Join(views, "inside")
	outside := filepath.Join(project, "outside")
	for _, directory := range []string{inside, outside} {
		if err := os.MkdirAll(directory, 0o700); err != nil {
			t.Fatal(err)
		}
		if err := os.Chmod(directory, 0o500); err != nil {
			t.Fatal(err)
		}
	}
	if err := WritePackageMap(project, []PackageMapRow{{inside, "dep", outside}}); err == nil {
		t.Fatal("escape accepted")
	}
	if _, err := os.Stat(filepath.Join(project, ".seen", "package-map.tsv")); !os.IsNotExist(err) {
		t.Fatalf("map promoted on failure: %v", err)
	}
}
