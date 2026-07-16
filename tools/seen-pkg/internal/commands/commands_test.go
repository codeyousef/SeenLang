package commands

import (
	"bytes"
	"context"
	"strings"
	"testing"
)

func TestVersionHandshake(t *testing.T) {
	t.Parallel()
	var out, errOut bytes.Buffer
	code := Run(context.Background(), []string{"--expect-version", "0.10.0", "version", "--machine"}, &out, &errOut)
	if code != 0 || out.String() != "protocol=SEENPKG1\nversion=0.10.0\n" {
		t.Fatalf("code=%d out=%q err=%q", code, out.String(), errOut.String())
	}
	out.Reset()
	errOut.Reset()
	code = Run(context.Background(), []string{"--expect-version", "0.9.5", "fetch"}, &out, &errOut)
	if code == 0 || !strings.Contains(errOut.String(), "version mismatch") {
		t.Fatalf("code=%d err=%q", code, errOut.String())
	}
}
func TestHostedAndAuthCommandsFailClosed(t *testing.T) {
	t.Parallel()
	for _, command := range []string{"login", "logout", "whoami", "publish", "yank", "report"} {
		var out, errOut bytes.Buffer
		if code := Run(context.Background(), []string{command}, &out, &errOut); code == 0 || !strings.Contains(errOut.String(), "refusing to continue") {
			t.Errorf("%s code=%d err=%q", command, code, errOut.String())
		}
	}
}
func TestSurfaceRecognizesAllCommands(t *testing.T) {
	t.Parallel()
	for _, command := range []string{"add", "remove", "fetch", "update", "pack"} {
		var out, errOut bytes.Buffer
		if code := Run(context.Background(), []string{command}, &out, &errOut); code != 69 || strings.Contains(errOut.String(), "unknown command") {
			t.Errorf("%s code=%d err=%q", command, code, errOut.String())
		}
	}
}
