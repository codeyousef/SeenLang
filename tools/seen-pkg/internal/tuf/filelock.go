package tuf

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"
)

type fileLock struct {
	file *os.File
	once sync.Once
	err  error
}

func acquireFileLock(ctx context.Context, path string) (*fileLock, error) {
	if ctx == nil {
		return nil, errors.New("file-lock context is nil")
	}
	parent := filepath.Dir(path)
	if err := os.MkdirAll(parent, 0o700); err != nil {
		return nil, err
	}
	if info, err := os.Lstat(path); err == nil {
		if !info.Mode().IsRegular() || info.Mode()&os.ModeSymlink != 0 {
			return nil, fmt.Errorf("transaction lock is not a regular file: %s", path)
		}
	} else if !errors.Is(err, os.ErrNotExist) {
		return nil, err
	}
	file, err := os.OpenFile(path, os.O_CREATE|os.O_RDWR, 0o600)
	if err != nil {
		return nil, err
	}
	closeOnError := func(err error) (*fileLock, error) {
		_ = file.Close()
		return nil, err
	}
	opened, err := file.Stat()
	if err != nil || !opened.Mode().IsRegular() {
		return closeOnError(errors.New("transaction lock is not a regular file"))
	}
	linked, err := os.Lstat(path)
	if err != nil || linked.Mode()&os.ModeSymlink != 0 || !os.SameFile(opened, linked) {
		return closeOnError(errors.New("transaction lock path changed while opening"))
	}
	// Do not chmod until the opened handle has been proven to be the regular
	// file still linked at path; a raced symlink must not let this client chmod
	// an unrelated target before it fails closed.
	if err := file.Chmod(0o600); err != nil {
		return closeOnError(err)
	}

	ticker := time.NewTicker(10 * time.Millisecond)
	defer ticker.Stop()
	for {
		locked, err := tryFileLock(file)
		if err != nil {
			return closeOnError(err)
		}
		if locked {
			return &fileLock{file: file}, nil
		}
		select {
		case <-ctx.Done():
			return closeOnError(ctx.Err())
		case <-ticker.C:
		}
	}
}

func (lock *fileLock) Close() error {
	lock.once.Do(func() {
		lock.err = errors.Join(unlockFile(lock.file), lock.file.Close())
	})
	return lock.err
}
