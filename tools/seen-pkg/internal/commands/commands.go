package commands

import (
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/codeyousef/seen/tools/seen-pkg/internal/lockfile"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/model"
)

type Streams struct{ Stdout, Stderr io.Writer }
type Backend interface {
	Run(context.Context, string, []string, Streams) error
}
type Runner struct {
	Backend Backend
	Streams Streams
}

func Run(ctx context.Context, args []string, stdout, stderr io.Writer) int {
	return Runner{Streams: Streams{Stdout: stdout, Stderr: stderr}}.Run(ctx, args)
}

func (runner Runner) Run(ctx context.Context, args []string) int {
	if runner.Streams.Stdout == nil {
		runner.Streams.Stdout = io.Discard
	}
	if runner.Streams.Stderr == nil {
		runner.Streams.Stderr = io.Discard
	}
	global, err := parseGlobalArguments(args)
	if err != nil {
		fmt.Fprintln(runner.Streams.Stderr, "seen-pkg:", err)
		usage(runner.Streams.Stderr)
		return 64
	}
	if global.expectedVersion != "" && global.expectedVersion != SidecarVersion {
		fmt.Fprintf(runner.Streams.Stderr, "seen-pkg: compiler/sidecar version mismatch: compiler expects %s but sidecar is %s\n", global.expectedVersion, SidecarVersion)
		return 78
	}
	if global.help {
		usage(runner.Streams.Stdout)
		return 0
	}
	if global.protocolVersion {
		fmt.Fprintln(runner.Streams.Stdout, ProtocolVersion)
		return 0
	}
	args = global.commandLine
	if len(args) == 0 {
		usage(runner.Streams.Stderr)
		return 64
	}
	command, arguments := args[0], args[1:]
	if command == "help" {
		if len(arguments) == 0 {
			usage(runner.Streams.Stdout)
			return 0
		}
		if len(arguments) != 1 {
			fmt.Fprintln(runner.Streams.Stderr, "seen-pkg help: unexpected extra operand", arguments[1])
			usage(runner.Streams.Stderr)
			return 64
		}
		schema, found := commandSchemaFor(arguments[0])
		if !found {
			fmt.Fprintf(runner.Streams.Stderr, "seen-pkg help: unknown command %q\n", arguments[0])
			usage(runner.Streams.Stderr)
			return 64
		}
		commandUsage(runner.Streams.Stdout, schema)
		return 0
	}
	schema, found := commandSchemaFor(command)
	if !found {
		fmt.Fprintf(runner.Streams.Stderr, "seen-pkg: unknown command %q\n", command)
		usage(runner.Streams.Stderr)
		return 64
	}
	parsed, err := parseCommandArguments(command, arguments, schema)
	if err != nil {
		return runner.reportUsageError(command, schema, err)
	}
	if parsed.help {
		commandUsage(runner.Streams.Stdout, schema)
		return 0
	}
	arguments = parsed.arguments
	switch command {
	case "version":
		if parsed.has("--machine") {
			fmt.Fprintf(runner.Streams.Stdout, "protocol=%s\nversion=%s\n", ProtocolVersion, SidecarVersion)
			return 0
		}
		fmt.Fprintf(runner.Streams.Stdout, "seen-pkg %s (%s)\n", SidecarVersion, ProtocolVersion)
		return 0
	case "login", "logout", "whoami":
		fmt.Fprintf(runner.Streams.Stderr, "seen-pkg %s: authentication bridge/service is not available; refusing to continue\n", command)
		return 69
	case "yank", "report":
		fmt.Fprintf(runner.Streams.Stderr, "seen-pkg %s: hosted registry write service is not available; refusing to continue\n", command)
		return 69
	case "tree":
		if err := runTree(arguments, runner.Streams.Stdout); err != nil {
			var usageErr *UsageError
			if errors.As(err, &usageErr) {
				return runner.reportUsageError(command, schema, usageErr)
			}
			fmt.Fprintln(runner.Streams.Stderr, "seen-pkg tree:", err)
			return 1
		}
		return 0
	case "audit":
		if err := runAudit(arguments, runner.Streams.Stdout); err != nil {
			var usageErr *UsageError
			if errors.As(err, &usageErr) {
				return runner.reportUsageError(command, schema, usageErr)
			}
			fmt.Fprintln(runner.Streams.Stderr, "seen-pkg audit:", err)
			return 1
		}
		return 0
	case "add", "remove", "fetch", "update", "pack", "publish":
		if runner.Backend == nil {
			fmt.Fprintf(runner.Streams.Stderr, "seen-pkg %s: package engine is not connected; refusing to continue\n", command)
			return 69
		}
		if err := runner.Backend.Run(ctx, command, arguments, runner.Streams); err != nil {
			var usageErr *UsageError
			if errors.As(err, &usageErr) {
				return runner.reportUsageError(command, schema, usageErr)
			}
			fmt.Fprintf(runner.Streams.Stderr, "seen-pkg %s: %v\n", command, err)
			return 1
		}
		return 0
	}
	return 64
}

func (runner Runner) reportUsageError(command string, schema commandSchema, err error) int {
	fmt.Fprintf(runner.Streams.Stderr, "seen-pkg %s: %v\n", command, err)
	commandUsage(runner.Streams.Stderr, schema)
	return 64
}

func usage(output io.Writer) {
	fmt.Fprintln(output, "Usage: seen-pkg [--expect-version VERSION] <command> [options]")
	fmt.Fprintln(output, "Commands: login logout whoami add remove fetch update tree audit pack publish yank report version")
}

func lockPath(arguments []string) (string, error) {
	if len(arguments) == 0 {
		return "Seen.lock", nil
	}
	if len(arguments) == 2 && arguments[0] == "--lock" && arguments[1] != "" {
		return arguments[1], nil
	}
	return "", usageErrorf("expected --lock <path>")
}
func runAudit(arguments []string, output io.Writer) error {
	filename, err := lockPath(arguments)
	if err != nil {
		return err
	}
	lock, err := lockfile.Load(filename)
	if err != nil {
		return err
	}
	fmt.Fprintf(output, "Seen.lock v2: %d packages; graph and capability bindings valid\n", len(lock.Packages))
	for _, pkg := range lock.Packages {
		fmt.Fprintf(output, "%s@%s %s", pkg.Package, pkg.Version, pkg.ArchiveSHA256)
		if len(pkg.Capabilities) != 0 {
			fmt.Fprintf(output, " capabilities=[%s]", joinCapabilities(pkg.Capabilities))
		}
		fmt.Fprintln(output)
	}
	return nil
}
func runTree(arguments []string, output io.Writer) error {
	filename, err := lockPath(arguments)
	if err != nil {
		return err
	}
	lock, err := lockfile.Load(filename)
	if err != nil {
		return err
	}
	nodes := map[model.PackageKey]model.LockedPackage{}
	for _, pkg := range lock.Packages {
		nodes[pkg.Key()] = pkg
	}
	fmt.Fprintf(output, "%s@%s\n", lock.Root.Name, lock.Root.Version)
	for index, edge := range lock.Root.Dependencies {
		printTree(output, edge, nodes, "", index == len(lock.Root.Dependencies)-1, map[model.PackageKey]bool{})
	}
	return nil
}
func printTree(output io.Writer, edge model.Edge, nodes map[model.PackageKey]model.LockedPackage, prefix string, last bool, stack map[model.PackageKey]bool) {
	branch := "├── "
	continuation := "│   "
	if last {
		branch = "└── "
		continuation = "    "
	}
	fmt.Fprintf(output, "%s%s%s -> %s@%s\n", prefix, branch, edge.Alias, edge.Package, edge.ResolvedVersion)
	key := edge.Key()
	if stack[key] {
		fmt.Fprintf(output, "%s%s(cycle)\n", prefix+continuation, "└── ")
		return
	}
	nextStack := make(map[model.PackageKey]bool, len(stack)+1)
	for item, value := range stack {
		nextStack[item] = value
	}
	nextStack[key] = true
	children := append([]model.Edge(nil), nodes[key].Dependencies...)
	sort.Slice(children, func(i, j int) bool { return children[i].Alias < children[j].Alias })
	for index, child := range children {
		printTree(output, child, nodes, prefix+continuation, index == len(children)-1, nextStack)
	}
}
func joinCapabilities(values []model.Capability) string {
	parts := make([]string, len(values))
	for index, value := range values {
		parts[index] = string(value)
	}
	return strings.Join(parts, ",")
}

// DefaultLockPath gives compiler bridges a canonical project-local lock path.
func DefaultLockPath(project string) (string, error) {
	absolute, err := filepath.Abs(project)
	if err != nil {
		return "", err
	}
	return filepath.Join(absolute, "Seen.lock"), nil
}

// Ensure imports used on platforms where os.PathSeparator influences filepath.
var _ = os.PathSeparator
