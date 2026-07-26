//go:build windows

package tuf

import (
	"os"
	"runtime"
	"syscall"
	"unsafe"
)

const (
	lockfileFailImmediately = 0x00000001
	lockfileExclusiveLock   = 0x00000002
	lockfileAllBytes        = ^uint32(0)
	errorLockViolation      = syscall.Errno(33)
)

var (
	kernel32         = syscall.NewLazyDLL("kernel32.dll")
	procLockFileEx   = kernel32.NewProc("LockFileEx")
	procUnlockFileEx = kernel32.NewProc("UnlockFileEx")
)

func tryFileLock(file *os.File) (bool, error) {
	overlapped := new(syscall.Overlapped)
	result, _, callErr := procLockFileEx.Call(
		file.Fd(),
		uintptr(lockfileExclusiveLock|lockfileFailImmediately),
		0,
		uintptr(lockfileAllBytes),
		uintptr(lockfileAllBytes),
		uintptr(unsafe.Pointer(overlapped)),
	)
	runtime.KeepAlive(overlapped)
	if result != 0 {
		return true, nil
	}
	if callErr == errorLockViolation {
		return false, nil
	}
	return false, callErr
}

func unlockFile(file *os.File) error {
	overlapped := new(syscall.Overlapped)
	result, _, callErr := procUnlockFileEx.Call(
		file.Fd(),
		0,
		uintptr(lockfileAllBytes),
		uintptr(lockfileAllBytes),
		uintptr(unsafe.Pointer(overlapped)),
	)
	runtime.KeepAlive(overlapped)
	if result == 0 {
		return callErr
	}
	return nil
}
