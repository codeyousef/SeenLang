package commands

import (
	"bytes"
	"context"
	"strings"
	"testing"
)

func TestManifestAndProjectParsersRejectExplicitEmptyOperands(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name  string
		parse func() error
	}{
		{"resolution project", func() error { _, err := parseResolutionCLI([]string{""}); return err }},
		{"resolution delimited project", func() error { _, err := parseResolutionCLI([]string{"--", ""}); return err }},
		{"pack project", func() error { _, _, _, err := parsePack([]string{""}); return err }},
		{"pack delimited project", func() error { _, _, _, err := parsePack([]string{"--", ""}); return err }},
		{"publish project", func() error { _, err := parsePublishCLI([]string{""}); return err }},
		{"publish delimited project", func() error { _, err := parsePublishCLI([]string{"--", ""}); return err }},
		{"add manifest", func() error {
			_, _, err := parseAdd([]string{"dependency", "owner/package", "^1.0.0", "--manifest", ""})
			return err
		}},
		{"remove manifest", func() error { _, _, err := parseRemove([]string{"dependency", "--manifest", ""}); return err }},
		{"manifest resolver", func() error { _, _, err := resolveManifestPath(""); return err }},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			err := test.parse()
			if err == nil || !strings.Contains(err.Error(), "empty") {
				t.Fatalf("explicit empty operand error=%v", err)
			}
		})
	}
}

func TestManifestAndProjectParsersPreserveOmissionDefaults(t *testing.T) {
	t.Parallel()
	resolution, err := parseResolutionCLI(nil)
	if err != nil || resolution.ManifestPath != "Seen.toml" {
		t.Fatalf("resolution manifest=%q err=%v", resolution.ManifestPath, err)
	}
	resolution, err = parseResolutionCLI([]string{"--"})
	if err != nil || resolution.ManifestPath != "Seen.toml" {
		t.Fatalf("delimited resolution manifest=%q err=%v", resolution.ManifestPath, err)
	}
	project, _, _, err := parsePack(nil)
	if err != nil || project != "." {
		t.Fatalf("pack project=%q err=%v", project, err)
	}
	project, _, _, err = parsePack([]string{"--"})
	if err != nil || project != "." {
		t.Fatalf("delimited pack project=%q err=%v", project, err)
	}
	publish, err := parsePublishCLI(nil)
	if err != nil || publish.ManifestPath != "Seen.toml" {
		t.Fatalf("publish manifest=%q err=%v", publish.ManifestPath, err)
	}
	publish, err = parsePublishCLI([]string{"--"})
	if err != nil || publish.ManifestPath != "Seen.toml" {
		t.Fatalf("delimited publish manifest=%q err=%v", publish.ManifestPath, err)
	}
	manifest, _, err := parseAdd([]string{"dependency", "owner/package", "^1.0.0"})
	if err != nil || manifest != "Seen.toml" {
		t.Fatalf("add manifest=%q err=%v", manifest, err)
	}
	manifest, _, err = parseRemove([]string{"dependency"})
	if err != nil || manifest != "Seen.toml" {
		t.Fatalf("remove manifest=%q err=%v", manifest, err)
	}
}

func TestCommandSchemasRejectExplicitEmptyManifestAndProjectOperands(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name       string
		command    string
		input      []string
		diagnostic string
	}{
		{"fetch plain project", "fetch", []string{""}, "empty"},
		{"fetch delimited project", "fetch", []string{"--", ""}, "empty"},
		{"update plain project", "update", []string{""}, "empty"},
		{"update delimited project", "update", []string{"--", ""}, "empty"},
		{"pack plain project", "pack", []string{""}, "empty"},
		{"pack delimited project", "pack", []string{"--", ""}, "empty"},
		{"publish plain project", "publish", []string{""}, "empty"},
		{"publish delimited project", "publish", []string{"--", ""}, "empty"},
		{"add separate manifest", "add", []string{"dependency", "owner/package", "^1.0.0", "--manifest", ""}, "empty"},
		{"add inline manifest", "add", []string{"dependency", "owner/package", "^1.0.0", "--manifest="}, "requires a value"},
		{"remove separate manifest", "remove", []string{"dependency", "--manifest", ""}, "empty"},
		{"remove inline manifest", "remove", []string{"dependency", "--manifest="}, "requires a value"},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			schema, found := commandSchemaFor(test.command)
			if !found {
				t.Fatalf("missing schema for %s", test.command)
			}
			_, err := parseCommandArguments(test.command, test.input, schema)
			if err == nil || !strings.Contains(err.Error(), test.diagnostic) {
				t.Fatalf("explicit empty operand error=%v", err)
			}
		})
	}
}

func TestRunnerReportsExplicitEmptyManifestAndProjectOperandsAsUsageErrors(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name       string
		command    string
		input      []string
		diagnostic string
	}{
		{"fetch plain project", "fetch", []string{""}, "empty"},
		{"fetch delimited project", "fetch", []string{"--", ""}, "empty"},
		{"update plain project", "update", []string{""}, "empty"},
		{"update delimited project", "update", []string{"--", ""}, "empty"},
		{"pack plain project", "pack", []string{""}, "empty"},
		{"pack delimited project", "pack", []string{"--", ""}, "empty"},
		{"publish plain project", "publish", []string{""}, "empty"},
		{"publish delimited project", "publish", []string{"--", ""}, "empty"},
		{"add separate manifest", "add", []string{"dependency", "owner/package", "^1.0.0", "--manifest", ""}, "empty"},
		{"add inline manifest", "add", []string{"dependency", "owner/package", "^1.0.0", "--manifest="}, "requires a value"},
		{"remove separate manifest", "remove", []string{"dependency", "--manifest", ""}, "empty"},
		{"remove inline manifest", "remove", []string{"dependency", "--manifest="}, "requires a value"},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			backendCalled := false
			backend := backendFunc(func(context.Context, string, []string, Streams) error {
				backendCalled = true
				return nil
			})
			var stdout, stderr bytes.Buffer
			runner := Runner{Backend: backend, Streams: Streams{Stdout: &stdout, Stderr: &stderr}}
			arguments := append([]string{test.command}, test.input...)
			if code := runner.Run(context.Background(), arguments); code != 64 || backendCalled || !strings.Contains(stderr.String(), test.diagnostic) {
				t.Fatalf("code=%d backendCalled=%t stdout=%q stderr=%q", code, backendCalled, stdout.String(), stderr.String())
			}
		})
	}
}
