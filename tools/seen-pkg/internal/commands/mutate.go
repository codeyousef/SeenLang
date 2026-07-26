package commands

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/codeyousef/seen/tools/seen-pkg/internal/atomicfile"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/manifest"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/model"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/semver"
)

func AddDependency(filename string, dependency model.Dependency) error {
	if err := model.ValidateAlias(dependency.Alias); err != nil {
		return err
	}
	content, mode, parsed, err := readMutableManifest(filename)
	if err != nil {
		return err
	}
	for _, existing := range parsed.Dependencies {
		if existing.Alias == dependency.Alias {
			return fmt.Errorf("dependency alias %q already exists", dependency.Alias)
		}
	}
	line, err := renderDependency(dependency, parsed)
	if err != nil {
		return err
	}
	updated, err := insertDependencyLine(content, line)
	if err != nil {
		return err
	}
	if _, err := manifest.ParseWithOptions(updated, manifest.Options{DefaultRegistryOrigin: "https://seen.dev.yousef.codes/packages"}); err != nil {
		return fmt.Errorf("updated manifest would be invalid: %w", err)
	}
	return writeManifestAtomic(filename, updated, mode)
}

func RemoveDependency(filename, alias string) error {
	if err := model.ValidateAlias(alias); err != nil {
		return err
	}
	content, mode, parsed, err := readMutableManifest(filename)
	if err != nil {
		return err
	}
	found := false
	for _, dependency := range parsed.Dependencies {
		if dependency.Alias == alias {
			found = true
			break
		}
	}
	if !found {
		return fmt.Errorf("dependency alias %q does not exist", alias)
	}
	updated, err := removeDependencyLine(content, alias)
	if err != nil {
		return err
	}
	if _, err := manifest.ParseWithOptions(updated, manifest.Options{DefaultRegistryOrigin: "https://seen.dev.yousef.codes/packages"}); err != nil {
		return fmt.Errorf("updated manifest would be invalid: %w", err)
	}
	return writeManifestAtomic(filename, updated, mode)
}

func readMutableManifest(filename string) ([]byte, os.FileMode, *model.Manifest, error) {
	content, err := os.ReadFile(filename)
	if err != nil {
		return nil, 0, nil, err
	}
	if strings.Contains(string(content), `'''`) || strings.Contains(string(content), `"""`) {
		return nil, 0, nil, fmt.Errorf("manifest mutation refuses multiline TOML strings; edit [dependencies] manually")
	}
	info, err := os.Stat(filename)
	if err != nil {
		return nil, 0, nil, err
	}
	parsed, err := manifest.ParseWithOptions(content, manifest.Options{DefaultRegistryOrigin: "https://seen.dev.yousef.codes/packages"})
	if err != nil {
		return nil, 0, nil, err
	}
	return content, info.Mode().Perm(), parsed, nil
}

func renderDependency(dependency model.Dependency, parsed *model.Manifest) (string, error) {
	switch dependency.Kind {
	case model.DependencyRegistry:
		if err := model.ValidateIdentity(dependency.Package); err != nil {
			return "", err
		}
		if _, err := semver.ParseRequirement(dependency.Requirement); err != nil {
			return "", err
		}
		registry := dependency.RegistryAlias
		if registry == "" {
			registry = "default"
		}
		if _, ok := parsed.Registries[registry]; !ok {
			return "", fmt.Errorf("registry alias %q is not configured", registry)
		}
		if err := model.ValidateCapabilities(dependency.Allow); err != nil {
			return "", err
		}
		line := dependency.Alias + " = { package = " + strconv.Quote(dependency.Package) + ", version = " + strconv.Quote(dependency.Requirement)
		if registry != "default" {
			line += ", registry = " + strconv.Quote(registry)
		}
		if len(dependency.Allow) != 0 {
			line += ", allow = " + renderCapabilityArray(model.CanonicalCapabilities(dependency.Allow))
		}
		return line + " }", nil
	case model.DependencyPath:
		if dependency.Path == "" || strings.ContainsAny(dependency.Path, "\x00\r\n") {
			return "", fmt.Errorf("path is invalid")
		}
		return dependency.Alias + " = { path = " + strconv.Quote(dependency.Path) + " }", nil
	case model.DependencyArtifact:
		if dependency.Artifact == "" || strings.ContainsAny(dependency.Artifact, "\x00\r\n") {
			return "", fmt.Errorf("artifact path is invalid")
		}
		return dependency.Alias + " = { artifact = " + strconv.Quote(dependency.Artifact) + " }", nil
	case model.DependencySystem:
		if err := validateSystemMutationPath(dependency.Path); err != nil {
			return "", err
		}
		return dependency.Alias + " = { system = true, path = " + strconv.Quote(dependency.Path) + " }", nil
	default:
		return "", fmt.Errorf("unsupported dependency kind %q", dependency.Kind)
	}
}
func renderCapabilityArray(values []model.Capability) string {
	parts := make([]string, len(values))
	for i, value := range values {
		parts[i] = strconv.Quote(string(value))
	}
	return "[" + strings.Join(parts, ", ") + "]"
}

func validateSystemMutationPath(value string) error {
	if value == "" || strings.HasPrefix(value, "/") || strings.ContainsAny(value, "\\:\x00\r\n") {
		return fmt.Errorf("system dependency path must be a safe relative slash path")
	}
	for _, segment := range strings.Split(value, "/") {
		if segment == "" || segment == "." || segment == ".." {
			return fmt.Errorf("system dependency path must not contain traversal")
		}
	}
	return nil
}

func dependencySection(content []byte) (start, end int, found bool, errorValue error) {
	lines := splitLines(content)
	offset := 0
	inside := false
	start = -1
	end = len(content)
	for _, line := range lines {
		trimmed := strings.TrimSpace(strings.TrimSuffix(line, "\n"))
		if strings.HasPrefix(trimmed, "[") {
			header := trimmed
			if comment := strings.Index(header, "#"); comment >= 0 {
				header = strings.TrimSpace(header[:comment])
			}
			if !strings.HasSuffix(header, "]") {
				return 0, 0, false, fmt.Errorf("unsupported multiline table header")
			}
			if inside {
				end = offset
				break
			}
			if header == "[dependencies]" {
				inside = true
				found = true
				start = offset + len(line)
			}
		}
		offset += len(line)
	}
	return start, end, found, nil
}
func insertDependencyLine(content []byte, line string) ([]byte, error) {
	start, end, found, err := dependencySection(content)
	if err != nil {
		return nil, err
	}
	if !found {
		suffix := ""
		if len(content) != 0 && content[len(content)-1] != '\n' {
			suffix = "\n"
		}
		return append(append(append([]byte(nil), content...), []byte(suffix+"\n[dependencies]\n")...), []byte(line+"\n")...), nil
	}
	insertion := end
	if insertion > start && content[insertion-1] != '\n' {
		return nil, fmt.Errorf("dependencies section is not line-delimited")
	}
	prefix := append([]byte(nil), content[:insertion]...)
	if len(prefix) > 0 && prefix[len(prefix)-1] != '\n' {
		prefix = append(prefix, '\n')
	}
	prefix = append(prefix, []byte(line+"\n")...)
	return append(prefix, content[insertion:]...), nil
}
func removeDependencyLine(content []byte, alias string) ([]byte, error) {
	start, end, found, err := dependencySection(content)
	if err != nil {
		return nil, err
	}
	if !found {
		return nil, fmt.Errorf("[dependencies] table is missing")
	}
	section := content[start:end]
	lines := splitLines(section)
	offset := start
	for _, line := range lines {
		trimmed := strings.TrimSpace(line)
		if strings.HasPrefix(trimmed, alias) {
			rest := strings.TrimSpace(strings.TrimPrefix(trimmed, alias))
			if strings.HasPrefix(rest, "=") {
				if strings.Count(rest, "{") != strings.Count(rest, "}") {
					return nil, fmt.Errorf("dependency %q uses a multiline value; edit manually", alias)
				}
				return append(append([]byte(nil), content[:offset]...), content[offset+len(line):]...), nil
			}
		}
		offset += len(line)
	}
	return nil, fmt.Errorf("could not locate dependency %q safely", alias)
}
func splitLines(content []byte) []string {
	if len(content) == 0 {
		return nil
	}
	raw := strings.SplitAfter(string(content), "\n")
	if raw[len(raw)-1] == "" {
		raw = raw[:len(raw)-1]
	}
	return raw
}
func writeManifestAtomic(filename string, content []byte, mode os.FileMode) error {
	directory := filepath.Dir(filename)
	temp, err := os.CreateTemp(directory, ".Seen.toml.tmp-*")
	if err != nil {
		return err
	}
	name := temp.Name()
	ok := false
	defer func() {
		if !ok {
			_ = os.Remove(name)
		}
	}()
	if err := temp.Chmod(mode); err != nil {
		_ = temp.Close()
		return err
	}
	if _, err := temp.Write(content); err != nil {
		_ = temp.Close()
		return err
	}
	if err := temp.Sync(); err != nil {
		_ = temp.Close()
		return err
	}
	if err := temp.Close(); err != nil {
		return err
	}
	if err := atomicfile.Replace(name, filename); err != nil {
		return err
	}
	if err := atomicfile.SyncDir(directory); err != nil {
		return err
	}
	ok = true
	return nil
}
