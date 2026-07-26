//go:build windows

package commands

import (
	"errors"
	"os"
)

func openPublishTokenFile(string) (*os.File, error) {
	return nil, errors.New("publish token files are unavailable on Windows; inject SEEN_REGISTRY_TOKEN through a trusted process environment")
}
