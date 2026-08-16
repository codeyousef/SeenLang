package commands

import (
	"fmt"
	"io"
	"strings"
)

// UsageError marks invalid command-line input. Runner maps these failures to
// EX_USAGE (64) while leaving filesystem, network, and package failures at 1.
type UsageError struct {
	Message string
}

func (err *UsageError) Error() string { return err.Message }

func usageErrorf(format string, arguments ...any) error {
	return &UsageError{Message: fmt.Sprintf(format, arguments...)}
}

type commandOption struct {
	value      bool
	repeatable bool
	nonempty   bool
}

type commandSchema struct {
	usage                  string
	options                map[string]commandOption
	minimum                int
	maximum                int
	nonemptyProjectOperand bool
	validate               func(commandArguments) error
}

type commandArguments struct {
	arguments   []string
	positionals []string
	values      map[string][]string
	help        bool
}

type globalArguments struct {
	commandLine     []string
	expectedVersion string
	help            bool
	protocolVersion bool
}

func parseGlobalArguments(input []string) (globalArguments, error) {
	var parsed globalArguments
	for index := 0; index < len(input); index++ {
		argument := input[index]
		if argument == "--" {
			parsed.commandLine = input[index+1:]
			return parsed, nil
		}
		if argument == "-h" || argument == "--help" {
			if index != len(input)-1 {
				return parsed, usageErrorf("global option %q does not accept operands", argument)
			}
			parsed.help = true
			return parsed, nil
		}
		if argument == "--protocol-version" {
			if index != len(input)-1 {
				return parsed, usageErrorf("--protocol-version does not accept operands")
			}
			parsed.protocolVersion = true
			return parsed, nil
		}
		if argument == "--expect-version" || strings.HasPrefix(argument, "--expect-version=") {
			value := ""
			if strings.HasPrefix(argument, "--expect-version=") {
				value = strings.TrimPrefix(argument, "--expect-version=")
			} else {
				if index+1 >= len(input) || strings.HasPrefix(input[index+1], "-") {
					return parsed, usageErrorf("--expect-version requires a value")
				}
				index++
				value = input[index]
			}
			if value == "" {
				return parsed, usageErrorf("--expect-version requires a value")
			}
			if parsed.expectedVersion != "" && parsed.expectedVersion != value {
				return parsed, usageErrorf("conflicting values for option %q: %q and %q", "--expect-version", parsed.expectedVersion, value)
			}
			parsed.expectedVersion = value
			continue
		}
		if strings.HasPrefix(argument, "-") {
			return parsed, usageErrorf("unknown global option %q", argument)
		}
		parsed.commandLine = input[index:]
		return parsed, nil
	}
	return parsed, nil
}

func flagOptions(names ...string) map[string]commandOption {
	result := make(map[string]commandOption, len(names))
	for _, name := range names {
		result[name] = commandOption{}
	}
	return result
}

func addValueOptions(options map[string]commandOption, repeatable bool, names ...string) {
	for _, name := range names {
		options[name] = commandOption{value: true, repeatable: repeatable}
	}
}

func addNonemptyValueOptions(options map[string]commandOption, names ...string) {
	for _, name := range names {
		options[name] = commandOption{value: true, nonempty: true}
	}
}

func commandSchemaFor(command string) (commandSchema, bool) {
	switch command {
	case "version":
		return commandSchema{usage: "Usage: seen-pkg version [--machine]", options: flagOptions("--machine"), maximum: 0}, true
	case "login", "logout", "whoami", "yank", "report":
		return commandSchema{usage: "Usage: seen-pkg " + command, options: map[string]commandOption{}, maximum: 0}, true
	case "tree", "audit":
		options := map[string]commandOption{}
		addValueOptions(options, false, "--lock")
		return commandSchema{usage: "Usage: seen-pkg " + command + " [--lock <Seen.lock>]", options: options, maximum: 0}, true
	case "fetch", "update":
		options := flagOptions("--locked", "--offline", "--frozen", "--quiet")
		addValueOptions(options, false, "--cache")
		addValueOptions(options, true, "--trusted-root", "--trusted-root-sha256", "--environment", "--repository-id")
		schema := commandSchema{
			usage:                  "Usage: seen-pkg " + command + " [project-dir-or-Seen.toml] [--locked] [--offline] [--frozen] [options]",
			options:                options,
			maximum:                1,
			nonemptyProjectOperand: true,
		}
		if command == "update" {
			schema.validate = func(parsed commandArguments) error {
				if parsed.has("--locked") || parsed.has("--frozen") {
					return usageErrorf("update cannot be combined with --locked or --frozen")
				}
				return nil
			}
		}
		return schema, true
	case "add":
		options := map[string]commandOption{}
		addValueOptions(options, false, "--registry", "--path", "--artifact", "--system-path")
		addNonemptyValueOptions(options, "--manifest")
		addValueOptions(options, true, "--allow")
		return commandSchema{
			usage:   "Usage: seen-pkg add ALIAS PACKAGE REQUIREMENT [options]\n       seen-pkg add ALIAS (--path|--artifact|--system-path) VALUE [options]",
			options: options,
			minimum: 1,
			maximum: 3,
			validate: func(parsed commandArguments) error {
				modes := 0
				for _, name := range []string{"--path", "--artifact", "--system-path"} {
					if parsed.has(name) {
						modes++
					}
				}
				if modes > 1 {
					return usageErrorf("add accepts only one of --path, --artifact, or --system-path")
				}
				if modes == 1 && len(parsed.positionals) != 1 {
					return usageErrorf("local add requires exactly one ALIAS operand")
				}
				if modes == 0 && len(parsed.positionals) != 3 {
					return usageErrorf("registry add requires ALIAS PACKAGE REQUIREMENT")
				}
				return nil
			},
		}, true
	case "remove":
		options := map[string]commandOption{}
		addNonemptyValueOptions(options, "--manifest")
		return commandSchema{usage: "Usage: seen-pkg remove ALIAS [--manifest <Seen.toml>]", options: options, minimum: 1, maximum: 1}, true
	case "pack":
		options := flagOptions("--quiet")
		addValueOptions(options, false, "--output")
		return commandSchema{usage: "Usage: seen-pkg pack [project-dir-or-Seen.toml] [--output <archive>] [--quiet]", options: options, maximum: 1, nonemptyProjectOperand: true}, true
	case "publish":
		options := flagOptions("--quiet")
		addValueOptions(options, false, "--registry", "--token-file", "--source-forge", "--source-repository-id", "--source-installation-id", "--source-ref", "--source-commit", "--license-spdx", "--description", "--repository")
		return commandSchema{usage: "Usage: seen-pkg publish [project-dir-or-Seen.toml] [options]", options: options, maximum: 1, nonemptyProjectOperand: true}, true
	default:
		return commandSchema{}, false
	}
}

func (arguments commandArguments) has(name string) bool {
	return len(arguments.values[name]) != 0
}

func parseCommandArguments(command string, input []string, schema commandSchema) (commandArguments, error) {
	parsed := commandArguments{values: map[string][]string{}}
	optionsEnabled := true
	for index := 0; index < len(input); index++ {
		argument := input[index]
		if optionsEnabled && (argument == "-h" || argument == "--help") {
			if len(input) != 1 {
				return parsed, usageErrorf("%s accepts %s only as its sole argument", command, argument)
			}
			parsed.help = true
			return parsed, nil
		}
		if optionsEnabled && argument == "--" {
			optionsEnabled = false
			parsed.arguments = append(parsed.arguments, argument)
			continue
		}
		if optionsEnabled && strings.HasPrefix(argument, "-") {
			name, inline, hasInline := strings.Cut(argument, "=")
			option, found := schema.options[name]
			if !found {
				return parsed, usageErrorf("unknown option %q", name)
			}
			value := "true"
			if option.value {
				if hasInline {
					if inline == "" {
						return parsed, usageErrorf("option %q requires a value", name)
					}
					value = inline
				} else {
					if index+1 >= len(input) || input[index+1] == "--" || strings.HasPrefix(input[index+1], "-") {
						return parsed, usageErrorf("option %q requires a value", name)
					}
					index++
					value = input[index]
				}
				if option.nonempty && value == "" {
					return parsed, usageErrorf("option %q requires a non-empty value", name)
				}
			} else if hasInline {
				return parsed, usageErrorf("option %q does not take a value", name)
			}
			prior := parsed.values[name]
			if !option.repeatable && len(prior) != 0 {
				if prior[0] != value {
					return parsed, usageErrorf("conflicting values for option %q: %q and %q", name, prior[0], value)
				}
				continue
			}
			parsed.values[name] = append(prior, value)
			parsed.arguments = append(parsed.arguments, name)
			if option.value {
				parsed.arguments = append(parsed.arguments, value)
			}
			continue
		}
		if schema.nonemptyProjectOperand && argument == "" {
			return parsed, usageErrorf("project or Seen.toml path must not be empty")
		}
		parsed.positionals = append(parsed.positionals, argument)
		parsed.arguments = append(parsed.arguments, argument)
	}
	if len(parsed.positionals) < schema.minimum {
		return parsed, usageErrorf("%s is missing a required operand", command)
	}
	if len(parsed.positionals) > schema.maximum {
		return parsed, usageErrorf("unexpected extra operand %q for %s", parsed.positionals[schema.maximum], command)
	}
	if schema.validate != nil {
		if err := schema.validate(parsed); err != nil {
			return parsed, err
		}
	}
	return parsed, nil
}

func commandUsage(output io.Writer, schema commandSchema) {
	fmt.Fprintln(output, schema.usage)
}
