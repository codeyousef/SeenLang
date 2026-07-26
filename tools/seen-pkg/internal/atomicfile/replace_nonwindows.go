//go:build !windows

package atomicfile

import (
	"errors"
	"os"
)

// Replace atomically moves temp to destination, replacing destination if it
// already exists. The paths must be on the same filesystem.
func Replace(temp, destination string) error {
	return os.Rename(temp, destination)
}

// SyncDir makes a completed directory-entry update durable.
func SyncDir(path string) error {
	directory, err := os.Open(path)
	if err != nil {
		return err
	}
	return errors.Join(directory.Sync(), directory.Close())
}
