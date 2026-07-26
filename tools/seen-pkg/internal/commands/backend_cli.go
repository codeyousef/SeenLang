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
	manifestPath, dependency, err := parseAdd(arguments)
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
	for index := 0; index < len(arguments); index++ {
		argument := arguments[index]
		switch argument {
		case "--manifest", "--registry", "--allow", "--path", "--artifact", "--system-path":
			if index+1 >= len(arguments) {
				return "", model.Dependency{}, fmt.Errorf("%s requires a value", argument)
			}
			value := arguments[index+1]
			index++
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
						return "", model.Dependency{}, fmt.Errorf("empty capability")
					}
					allow = append(allow, model.Capability(item))
				}
			}
		default:
			if strings.HasPrefix(argument, "-") {
				return "", model.Dependency{}, fmt.Errorf("unknown option %s", argument)
			}
			positional = append(positional, argument)
		}
	}
	manifestPath, _, err := resolveManifestPath(manifestInput)
	if err != nil {
		return "", model.Dependency{}, err
	}
	if len(positional) < 1 {
		return "", model.Dependency{}, fmt.Errorf("usage: add ALIAS PACKAGE REQUIREMENT or add ALIAS --path PATH")
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
			return "", model.Dependency{}, fmt.Errorf("registry add requires ALIAS PACKAGE REQUIREMENT")
		}
		dependency.Kind, dependency.Package, dependency.Requirement = model.DependencyRegistry, positional[1], positional[2]
	} else if localModes != 1 || len(positional) != 1 {
		return "", model.Dependency{}, fmt.Errorf("local add requires one of --path, --artifact, or --system-path")
	}
	return manifestPath, dependency, nil
}

func (backend *ProductionBackend) remove(arguments []string, streams Streams) error {
	manifestInput, alias := "Seen.toml", ""
	for index := 0; index < len(arguments); index++ {
		if arguments[index] == "--manifest" {
			if index+1 >= len(arguments) {
				return fmt.Errorf("--manifest requires a value")
			}
			manifestInput = arguments[index+1]
			index++
			continue
		}
		if strings.HasPrefix(arguments[index], "-") {
			return fmt.Errorf("unknown option %s", arguments[index])
		}
		if alias != "" {
			return fmt.Errorf("remove accepts one alias")
		}
		alias = arguments[index]
	}
	if alias == "" {
		return fmt.Errorf("remove requires an alias")
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

func (backend *ProductionBackend) pack(ctx context.Context, arguments []string, streams Streams) error {
	project, output, quiet, positional := ".", "", false, false
	for index := 0; index < len(arguments); index++ {
		switch arguments[index] {
		case "--output":
			if index+1 >= len(arguments) {
				return fmt.Errorf("--output requires a value")
			}
			output = arguments[index+1]
			index++
		case "--quiet":
			quiet = true
		default:
			if strings.HasPrefix(arguments[index], "-") {
				return fmt.Errorf("unknown option %s", arguments[index])
			}
			if positional {
				return fmt.Errorf("pack accepts one project path")
			}
			project, positional = arguments[index], true
		}
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
