package atomicfile

import (
	"os"
	"path/filepath"
	"testing"
)

func TestReplaceExistingDestination(t *testing.T) {
	directory := t.TempDir()
	temporary := filepath.Join(directory, "temporary")
	destination := filepath.Join(directory, "destination")
	if err := os.WriteFile(temporary, []byte("new contents"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(destination, []byte("old contents"), 0o600); err != nil {
		t.Fatal(err)
	}

	if err := Replace(temporary, destination); err != nil {
		t.Fatalf("Replace() error = %v", err)
	}
	contents, err := os.ReadFile(destination)
	if err != nil {
		t.Fatal(err)
	}
	if got, want := string(contents), "new contents"; got != want {
		t.Fatalf("destination contents = %q, want %q", got, want)
	}
	if _, err := os.Stat(temporary); !os.IsNotExist(err) {
		t.Fatalf("temporary file still exists or cannot be checked: %v", err)
	}
}
