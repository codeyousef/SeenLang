//go:build !linux && !windows

package commands

import (
	"errors"
	"os"
)

func openPublishTokenFile(path string) (*os.File, error) {
	before, err := os.Lstat(path)
	if err != nil {
		return nil, err
	}
	if !before.Mode().IsRegular() {
		return nil, errors.New("publish token path is not a regular file")
	}
	file, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	after, err := file.Stat()
	if err != nil {
		_ = file.Close()
		return nil, err
	}
	if !os.SameFile(before, after) {
		_ = file.Close()
		return nil, errors.New("publish token file changed while opening")
	}
	return file, nil
}
