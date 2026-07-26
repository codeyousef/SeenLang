//go:build windows

package atomicfile

import (
	"syscall"
	"unsafe"
)

const (
	moveFileReplaceExisting = 0x1
	moveFileWriteThrough    = 0x8
)

var moveFileExW = syscall.NewLazyDLL("kernel32.dll").NewProc("MoveFileExW")

// Replace atomically moves temp to destination, replacing destination if it
// already exists. The paths must be on the same filesystem.
func Replace(temp, destination string) error {
	from, err := syscall.UTF16PtrFromString(temp)
	if err != nil {
		return err
	}
	to, err := syscall.UTF16PtrFromString(destination)
	if err != nil {
		return err
	}

	succeeded, _, callErr := moveFileExW.Call(
		uintptr(unsafe.Pointer(from)),
		uintptr(unsafe.Pointer(to)),
		moveFileReplaceExisting|moveFileWriteThrough,
	)
	if succeeded != 0 {
		return nil
	}
	if callErr != syscall.Errno(0) {
		return callErr
	}
	return syscall.EINVAL
}

// MoveFileExW with MOVEFILE_WRITE_THROUGH already flushes the replacement;
// Windows does not expose portable directory-handle fsync semantics through os.
func SyncDir(string) error { return nil }
