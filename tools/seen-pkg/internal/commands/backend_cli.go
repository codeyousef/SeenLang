package commands

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/codeyousef/seen/tools/seen-pkg/internal/model"
)

func (backend *ProductionBackend) add(arguments []string, streams Streams) error {
	manifestInput, dependency, err := parseAdd(arguments)
	if err != nil {
		return err
	}
	manifestPath, _, err := resolveManifestPath(manifestInput)
	if err != nil {
		return err
	}
	if err := AddDependency(manifestPath, dependency); err != nil {
		return err
	}
	fmt.Fprintf(streams.Stdout, "Added dependency %s to %s\n", dependency.Alias, manifestPath)
	return nil
}

func parseAdd(arguments []string) (string, model.Dependency, error) {
	manifestInput, registryAlias := "Seen.toml", "default"
	var allow []model.Capability
	var pathValue, artifact, systemPath string
	var positional []string
	optionsEnabled := true
	for index := 0; index < len(arguments); index++ {
		argument := arguments[index]
		if optionsEnabled && argument == "--" {
			optionsEnabled = false
			continue
		}
		if !optionsEnabled {
			positional = append(positional, argument)
			continue
		}
		switch argument {
		case "--manifest", "--registry", "--allow", "--path", "--artifact", "--system-path":
			if index+1 >= len(arguments) {
				return "", model.Dependency{}, usageErrorf("%s requires a value", argument)
			}
			value := arguments[index+1]
			index++
			if argument == "--manifest" && value == "" {
				return "", model.Dependency{}, usageErrorf("--manifest requires a non-empty value")
			}
			switch argument {
			case "--manifest":
				manifestInput = value
			case "--registry":
				registryAlias = value
			case "--path":
				pathValue = value
			case "--artifact":
				artifact = value
			case "--system-path":
				systemPath = value
			case "--allow":
				for _, item := range strings.Split(value, ",") {
					if item == "" {
						return "", model.Dependency{}, usageErrorf("empty capability")
					}
					allow = append(allow, model.Capability(item))
				}
			}
		default:
			if strings.HasPrefix(argument, "-") {
				return "", model.Dependency{}, usageErrorf("unknown option %s", argument)
			}
			positional = append(positional, argument)
		}
	}
	if len(positional) < 1 {
		return "", model.Dependency{}, usageErrorf("add is missing a required operand")
	}
	dependency := model.Dependency{Alias: positional[0], RegistryAlias: registryAlias, Allow: allow}
	localModes := 0
	if pathValue != "" {
		localModes++
		dependency.Kind, dependency.Path = model.DependencyPath, pathValue
	}
	if artifact != "" {
		localModes++
		dependency.Kind, dependency.Artifact = model.DependencyArtifact, artifact
	}
	if systemPath != "" {
		localModes++
		dependency.Kind, dependency.Path = model.DependencySystem, systemPath
	}
	if localModes == 0 {
		if len(positional) != 3 {
			return "", model.Dependency{}, usageErrorf("registry add requires ALIAS PACKAGE REQUIREMENT")
		}
		dependency.Kind, dependency.Package, dependency.Requirement = model.DependencyRegistry, positional[1], positional[2]
	} else if localModes != 1 || len(positional) != 1 {
		return "", model.Dependency{}, usageErrorf("local add requires one of --path, --artifact, or --system-path")
	}
	return manifestInput, dependency, nil
}

func (backend *ProductionBackend) remove(arguments []string, streams Streams) error {
	manifestInput, alias, err := parseRemove(arguments)
	if err != nil {
		return err
	}
	manifestPath, _, err := resolveManifestPath(manifestInput)
	if err != nil {
		return err
	}
	if err := RemoveDependency(manifestPath, alias); err != nil {
		return err
	}
	fmt.Fprintf(streams.Stdout, "Removed dependency %s from %s\n", alias, manifestPath)
	return nil
}

func parseRemove(arguments []string) (string, string, error) {
	manifestInput, alias := "Seen.toml", ""
	optionsEnabled := true
	for index := 0; index < len(arguments); index++ {
		argument := arguments[index]
		if optionsEnabled && argument == "--" {
			optionsEnabled = false
			continue
		}
		if optionsEnabled && argument == "--manifest" {
			if index+1 >= len(arguments) {
				return "", "", usageErrorf("--manifest requires a value")
			}
			manifestInput = arguments[index+1]
			if manifestInput == "" {
				return "", "", usageErrorf("--manifest requires a non-empty value")
			}
			index++
			continue
		}
		if optionsEnabled && strings.HasPrefix(argument, "-") {
			return "", "", usageErrorf("unknown option %s", argument)
		}
		if alias != "" {
			return "", "", usageErrorf("remove accepts one alias")
		}
		alias = argument
	}
	if alias == "" {
		return "", "", usageErrorf("remove requires an alias")
	}
	return manifestInput, alias, nil
}

func (backend *ProductionBackend) pack(ctx context.Context, arguments []string, streams Streams) error {
	project, output, quiet, err := parsePack(arguments)
	if err != nil {
		return err
	}
	if info, err := os.Stat(project); err == nil && !info.IsDir() {
		project = filepath.Dir(project)
	}
	result, err := Pack(ctx, project, output)
	if err != nil {
		return err
	}
	if !quiet {
		fmt.Fprintf(streams.Stdout, "Packed %s\nsha256 %s\nbytes %d\n", result.Path, result.SHA256, result.Length)
	}
	return nil
}

func parsePack(arguments []string) (string, string, bool, error) {
	project, output, quiet, positional := ".", "", false, false
	optionsEnabled := true
	for index := 0; index < len(arguments); index++ {
		argument := arguments[index]
		if optionsEnabled && argument == "--" {
			optionsEnabled = false
			continue
		}
		if !optionsEnabled {
			if positional {
				return "", "", false, usageErrorf("pack accepts one project path")
			}
			if argument == "" {
				return "", "", false, usageErrorf("project or Seen.toml path must not be empty")
			}
			project, positional = argument, true
			continue
		}
		switch argument {
		case "--output":
			if index+1 >= len(arguments) {
				return "", "", false, usageErrorf("--output requires a value")
			}
			output = arguments[index+1]
			index++
		case "--quiet":
			quiet = true
		default:
			if strings.HasPrefix(argument, "-") {
				return "", "", false, usageErrorf("unknown option %s", argument)
			}
			if positional {
				return "", "", false, usageErrorf("pack accepts one project path")
			}
			if argument == "" {
				return "", "", false, usageErrorf("project or Seen.toml path must not be empty")
			}
			project, positional = argument, true
		}
	}
	return project, output, quiet, nil
}
