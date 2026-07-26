//go:build !windows && !darwin && !dragonfly && !freebsd && !illumos && !linux && !netbsd && !openbsd

package tuf

import (
	"fmt"
	"os"
	"runtime"
)

func tryFileLock(_ *os.File) (bool, error) {
	return false, fmt.Errorf("trusted-state locking is unsupported on %s", runtime.GOOS)
}

func unlockFile(_ *os.File) error { return nil }
