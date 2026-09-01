package commands

import (
	"bytes"
	"context"
	"errors"
	"reflect"
	"strings"
	"testing"
)

type backendFunc func(context.Context, string, []string, Streams) error

func (function backendFunc) Run(ctx context.Context, command string, arguments []string, streams Streams) error {
	return function(ctx, command, arguments, streams)
}

func TestVersionHandshake(t *testing.T) {
	t.Parallel()
	var out, errOut bytes.Buffer
	code := Run(context.Background(), []string{"--expect-version", "0.19.3", "version", "--machine"}, &out, &errOut)
	if code != 0 || out.String() != "protocol=SEENPKG1\nversion=0.19.3\n" {
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
	for _, command := range []string{"login", "logout", "whoami", "yank", "report"} {
		var out, errOut bytes.Buffer
		if code := Run(context.Background(), []string{command}, &out, &errOut); code == 0 || !strings.Contains(errOut.String(), "refusing to continue") {
			t.Errorf("%s code=%d err=%q", command, code, errOut.String())
		}
	}
}
func TestSurfaceRecognizesAllCommands(t *testing.T) {
	t.Parallel()
	commands := map[string][]string{
		"add":     {"alias", "owner/package", "^1.0.0"},
		"remove":  {"alias"},
		"fetch":   nil,
		"update":  nil,
		"pack":    nil,
		"publish": nil,
	}
	for command, arguments := range commands {
		var out, errOut bytes.Buffer
		input := append([]string{command}, arguments...)
		if code := Run(context.Background(), input, &out, &errOut); code != 69 || strings.Contains(errOut.String(), "unknown command") {
			t.Errorf("%s code=%d err=%q", command, code, errOut.String())
		}
	}
}

func TestGlobalCLIParsing(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name       string
		arguments  []string
		code       int
		stdoutText string
		stderrText string
	}{
		{"long help", []string{"--help"}, 0, "Usage: seen-pkg", ""},
		{"short help", []string{"-h"}, 0, "Usage: seen-pkg", ""},
		{"delimiter", []string{"--", "version", "--machine"}, 0, "protocol=", ""},
		{"identical repeat", []string{"--expect-version=0.19.3", "--expect-version", "0.19.3", "version"}, 0, "seen-pkg", ""},
		{"unknown", []string{"--unknown"}, 64, "", "unknown global option"},
		{"missing value", []string{"--expect-version"}, 64, "", "requires a value"},
		{"conflicting repeat", []string{"--expect-version", "0.19.3", "--expect-version", "0.10.2", "version"}, 64, "", "conflicting values"},
		{"protocol arity", []string{"--protocol-version", "extra"}, 64, "", "does not accept operands"},
		{"help arity", []string{"--help", "extra"}, 64, "", "does not accept operands"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			var stdout, stderr bytes.Buffer
			code := Run(context.Background(), test.arguments, &stdout, &stderr)
			if code != test.code || !strings.Contains(stdout.String(), test.stdoutText) || !strings.Contains(stderr.String(), test.stderrText) {
				t.Fatalf("code=%d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
			}
		})
	}
}

func TestEveryCommandSupportsHelp(t *testing.T) {
	t.Parallel()
	for _, command := range []string{"version", "login", "logout", "whoami", "add", "remove", "fetch", "update", "tree", "audit", "pack", "publish", "yank", "report"} {
		for _, help := range []string{"-h", "--help"} {
			var stdout, stderr bytes.Buffer
			code := Run(context.Background(), []string{command, help}, &stdout, &stderr)
			if code != 0 || !strings.Contains(stdout.String(), "Usage: seen-pkg "+command) || stderr.Len() != 0 {
				t.Errorf("%s %s code=%d stdout=%q stderr=%q", command, help, code, stdout.String(), stderr.String())
			}
		}
	}
}

func TestCommandCLIUsageFailuresReturn64(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name      string
		arguments []string
		message   string
	}{
		{"unknown option", []string{"fetch", "--unknown"}, "unknown option"},
		{"missing value", []string{"fetch", "--cache"}, "requires a value"},
		{"conflicting value", []string{"fetch", "--cache", "one", "--cache", "two"}, "conflicting values"},
		{"extra operand", []string{"fetch", "one", "two"}, "unexpected extra operand"},
		{"missing operand", []string{"remove"}, "missing a required operand"},
		{"conflicting add modes", []string{"add", "dep", "--path", "one", "--artifact", "two"}, "only one of"},
		{"invalid update mode", []string{"update", "--locked"}, "cannot be combined"},
		{"aliased repeat conflict", []string{"fetch", "--trusted-root", "custom=one", "--trusted-root", "custom=two"}, "conflicting values"},
		{"help with extra", []string{"fetch", "--help", "extra"}, "only as its sole argument"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			var stdout, stderr bytes.Buffer
			runner := Runner{Backend: NewProductionBackend(), Streams: Streams{Stdout: &stdout, Stderr: &stderr}}
			if code := runner.Run(context.Background(), test.arguments); code != 64 || !strings.Contains(stderr.String(), test.message) || !strings.Contains(stderr.String(), "Usage: seen-pkg") {
				t.Fatalf("code=%d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
			}
		})
	}
}

func TestCommandDelimiterAndIdenticalRepeatsReachBackend(t *testing.T) {
	t.Parallel()
	var gotCommand string
	var gotArguments []string
	backend := backendFunc(func(_ context.Context, command string, arguments []string, _ Streams) error {
		gotCommand = command
		gotArguments = append([]string(nil), arguments...)
		return nil
	})
	var stdout, stderr bytes.Buffer
	runner := Runner{Backend: backend, Streams: Streams{Stdout: &stdout, Stderr: &stderr}}
	input := []string{"remove", "--manifest=Seen.toml", "--manifest", "Seen.toml", "--", "--dependency"}
	if code := runner.Run(context.Background(), input); code != 0 {
		t.Fatalf("code=%d stderr=%q", code, stderr.String())
	}
	want := []string{"--manifest", "Seen.toml", "--", "--dependency"}
	if gotCommand != "remove" || !reflect.DeepEqual(gotArguments, want) {
		t.Fatalf("command=%q arguments=%q want=%q", gotCommand, gotArguments, want)
	}
}

func TestUsageAndOperationalBackendErrorsHaveDistinctExitCodes(t *testing.T) {
	t.Parallel()
	for _, test := range []struct {
		name string
		err  error
		code int
	}{
		{"usage", usageErrorf("invalid request syntax"), 64},
		{"operational", errors.New("disk unavailable"), 1},
	} {
		t.Run(test.name, func(t *testing.T) {
			var stdout, stderr bytes.Buffer
			runner := Runner{
				Backend: backendFunc(func(context.Context, string, []string, Streams) error { return test.err }),
				Streams: Streams{Stdout: &stdout, Stderr: &stderr},
			}
			if code := runner.Run(context.Background(), []string{"fetch"}); code != test.code {
				t.Fatalf("code=%d stderr=%q", code, stderr.String())
			}
		})
	}
}
