//go:build darwin || dragonfly || freebsd || illumos || linux || netbsd || openbsd

package tuf

import (
	"errors"
	"os"
	"syscall"
)

func tryFileLock(file *os.File) (bool, error) {
	for {
		err := syscall.Flock(int(file.Fd()), syscall.LOCK_EX|syscall.LOCK_NB)
		if err == nil {
			return true, nil
		}
		if errors.Is(err, syscall.EWOULDBLOCK) || errors.Is(err, syscall.EAGAIN) {
			return false, nil
		}
		if !errors.Is(err, syscall.EINTR) {
			return false, err
		}
	}
}

func unlockFile(file *os.File) error {
	for {
		err := syscall.Flock(int(file.Fd()), syscall.LOCK_UN)
		if !errors.Is(err, syscall.EINTR) {
			return err
		}
	}
}
