//go:build windows

package commands

import (
	"strings"
	"testing"
)

func TestWindowsPublishTokenFileFailsClosed(t *testing.T) {
	if _, err := loadPublishToken(`C:\\private\\seen-registry-token`); err == nil || !strings.Contains(err.Error(), "SEEN_REGISTRY_TOKEN") {
		t.Fatalf("Windows token file error = %v", err)
	}
}
