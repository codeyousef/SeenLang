package commands

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	seenarchive "github.com/codeyousef/seen/tools/seen-pkg/internal/archive"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/cache"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/lockfile"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/manifest"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/model"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/resolver"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/transport"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/tuf"
)

type ProductionBackend struct{}

func NewProductionBackend() *ProductionBackend { return &ProductionBackend{} }

func (backend *ProductionBackend) Run(ctx context.Context, command string, arguments []string, streams Streams) error {
	switch command {
	case "fetch", "update":
		return backend.resolve(ctx, command, arguments, streams)
	case "add":
		return backend.add(arguments, streams)
	case "remove":
		return backend.remove(arguments, streams)
	case "pack":
		return backend.pack(ctx, arguments, streams)
	default:
		return fmt.Errorf("unsupported backend command %q", command)
	}
}

type resolutionCLI struct {
	ManifestPath, CacheRoot                                      string
	Locked, Offline, Frozen, Quiet                               bool
	TrustedRoots, TrustedRootSHA256, Environments, RepositoryIDs map[string]string
}

func parseResolutionCLI(arguments []string) (resolutionCLI, error) {
	options := resolutionCLI{TrustedRoots: map[string]string{}, TrustedRootSHA256: map[string]string{}, Environments: map[string]string{}, RepositoryIDs: map[string]string{}}
	for index := 0; index < len(arguments); index++ {
		argument := arguments[index]
		switch argument {
		case "--locked":
			options.Locked = true
		case "--offline":
			options.Offline = true
		case "--frozen":
			options.Frozen = true
		case "--quiet":
			options.Quiet = true
		case "--cache", "--trusted-root", "--trusted-root-sha256", "--environment", "--repository-id":
			if index+1 >= len(arguments) {
				return options, fmt.Errorf("%s requires a value", argument)
			}
			value := arguments[index+1]
			index++
			switch argument {
			case "--cache":
				options.CacheRoot = value
			case "--trusted-root":
				alias, item, err := aliasValue(value)
				if err != nil {
					return options, err
				}
				options.TrustedRoots[alias] = item
			case "--trusted-root-sha256":
				alias, item, err := aliasValue(value)
				if err != nil {
					return options, err
				}
				if err := model.ValidateSHA256(item); err != nil {
					return options, err
				}
				options.TrustedRootSHA256[alias] = item
			case "--environment":
				alias, item, err := aliasValue(value)
				if err != nil {
					return options, err
				}
				options.Environments[alias] = item
			case "--repository-id":
				alias, item, err := aliasValue(value)
				if err != nil {
					return options, err
				}
				options.RepositoryIDs[alias] = item
			}
		default:
			if strings.HasPrefix(argument, "-") {
				return options, fmt.Errorf("unknown option %s", argument)
			}
			if options.ManifestPath != "" {
				return options, fmt.Errorf("only one project or Seen.toml path is allowed")
			}
			options.ManifestPath = argument
		}
	}
	if options.ManifestPath == "" {
		options.ManifestPath = "Seen.toml"
	}
	if options.CacheRoot == "" {
		options.CacheRoot = os.Getenv("SEEN_PACKAGE_CACHE")
	}
	if options.CacheRoot == "" {
		base, err := os.UserCacheDir()
		if err != nil {
			return options, err
		}
		options.CacheRoot = filepath.Join(base, "seen", "package-registry-v1")
	}
	return options, nil
}

func aliasValue(value string) (string, string, error) {
	alias, item, found := strings.Cut(value, "=")
	if !found {
		alias, item = "default", value
	}
	if err := model.ValidateAlias(alias); err != nil {
		return "", "", err
	}
	if item == "" {
		return "", "", fmt.Errorf("empty value for %s", alias)
	}
	return alias, item, nil
}

func resolveManifestPath(input string) (string, string, error) {
	absolute, err := filepath.Abs(input)
	if err != nil {
		return "", "", err
	}
	info, err := os.Stat(absolute)
	if err != nil {
		return "", "", err
	}
	manifestPath := absolute
	if info.IsDir() {
		manifestPath = filepath.Join(absolute, "Seen.toml")
	} else if filepath.Base(absolute) != "Seen.toml" {
		return "", "", fmt.Errorf("manifest path must name Seen.toml")
	}
	root, err := filepath.EvalSymlinks(filepath.Dir(manifestPath))
	if err != nil {
		return "", "", err
	}
	return filepath.Join(root, "Seen.toml"), root, nil
}

func (backend *ProductionBackend) resolve(ctx context.Context, command string, arguments []string, streams Streams) error {
	cli, err := parseResolutionCLI(arguments)
	if err != nil {
		return err
	}
	if command == "update" && (cli.Locked || cli.Frozen) {
		return &resolver.Error{Code: "invalid_mode_combination", Detail: "update cannot be combined with locked or frozen"}
	}
	manifestPath, projectRoot, err := resolveManifestPath(cli.ManifestPath)
	if err != nil {
		return err
	}
	parsed, err := manifest.Load(manifestPath, manifest.Options{DefaultRegistryOrigin: "https://seen.dev.yousef.codes/packages"})
	if err != nil {
		return err
	}
	root, manifestDigest := manifest.Root(parsed), manifest.Digest(parsed.Raw)
	lockPath := filepath.Join(projectRoot, "Seen.lock")
	var existing *model.Lock
	loaded, loadErr := lockfile.Load(lockPath)
	if loadErr == nil {
		existing = loaded
	} else if !errors.Is(loadErr, os.ErrNotExist) {
		return loadErr
	}
	locked := cli.Locked || cli.Frozen
	if locked {
		if existing == nil {
			return &resolver.Error{Code: "lock_required"}
		}
		if err := lockfile.Enforce(existing, manifestDigest, root); err != nil {
			return err
		}
	}
	localRows, err := materializeLocalDependencies(ctx, projectRoot, parsed.Dependencies)
	if err != nil {
		return err
	}
	if len(root.Dependencies) == 0 {
		if !locked {
			if parsed.ManifestVersion == 1 {
				empty := &model.Resolution{Root: root}
				if err := lockfile.Write(lockPath, lockfile.FromResolution(manifestDigest, empty)); err != nil {
					return err
				}
			} else if err := removeMutableFile(lockPath); err != nil {
				return err
			}
		}
		if len(localRows) == 0 {
			if err := RemovePackageMap(projectRoot); err != nil {
				return err
			}
		} else if err := WritePackageMap(projectRoot, localRows); err != nil {
			return err
		}
		if !cli.Quiet {
			fmt.Fprintf(streams.Stdout, "Prepared %d local package edges; no hosted packages were required\n", len(localRows))
		}
		return nil
	}
	cacheRoot, err := filepath.Abs(cli.CacheRoot)
	if err != nil {
		return err
	}
	shared, err := cache.New(filepath.Join(cacheRoot, "content"))
	if err != nil {
		return err
	}
	specs, err := registrySpecs(parsed, cli, cacheRoot)
	if err != nil {
		return err
	}
	runtime := newRegistryRuntime(specs, cacheRoot, cli.Offline || cli.Frozen)
	prior := existing
	priorValid := existing != nil && locked
	if existing != nil && !locked {
		enforceErr := lockfile.Enforce(existing, manifestDigest, root)
		priorValid = enforceErr == nil
		if enforceErr != nil && command != "update" {
			prior = nil
		}
	}
	runtime.preferCached = command == "fetch" && prior != nil && priorValid
	strategy := resolver.Normal
	if command == "update" {
		strategy = resolver.Update
	}
	resolution, err := resolver.Resolve(ctx, root, runtime, resolver.Options{Strategy: strategy, Locked: locked, Offline: cli.Offline, Frozen: cli.Frozen, Lock: prior})
	if err != nil {
		return err
	}
	views, err := materializeResolution(ctx, projectRoot, shared, runtime, resolution, cli.Offline || cli.Frozen)
	if err != nil {
		return err
	}
	rows, err := packageMapRows(projectRoot, resolution, views)
	if err != nil {
		return err
	}
	rows = append(rows, localRows...)
	newLock := lockfile.FromResolution(manifestDigest, resolution)
	if !locked && (command == "update" || prior == nil || !resolution.UsedLock) {
		if err := lockfile.Write(lockPath, newLock); err != nil {
			return err
		}
	}
	if err := WritePackageMap(projectRoot, rows); err != nil {
		return err
	}
	if !cli.Quiet {
		verb := "Fetched"
		if command == "update" {
			verb = "Updated"
		}
		fmt.Fprintf(streams.Stdout, "%s %d hosted packages; Seen.lock v2 and package map are ready\n", verb, len(resolution.Packages))
	}
	return nil
}

func registrySpecs(parsed *model.Manifest, cli resolutionCLI, cacheRoot string) ([]registrySpec, error) {
	aliases := make([]string, 0, len(parsed.Registries))
	for alias := range parsed.Registries {
		aliases = append(aliases, alias)
	}
	sort.Strings(aliases)
	byOrigin := map[string]registrySpec{}
	for _, alias := range aliases {
		origin := parsed.Registries[alias]
		spec, _ := defaultRegistrySpec(alias, origin)
		if value := cli.Environments[alias]; value != "" {
			spec.Environment = value
		}
		if value := cli.RepositoryIDs[alias]; value != "" {
			spec.RepositoryID = value
		}
		if value := cli.TrustedRoots[alias]; value != "" {
			absolute, err := filepath.Abs(value)
			if err != nil {
				return nil, err
			}
			spec.TrustedRoot = absolute
		}
		if value := cli.TrustedRootSHA256[alias]; value != "" {
			spec.TrustedRootSHA256 = value
		}
		if alias == "default" && spec.TrustedRoot == "" && os.Getenv("SEEN_TRUST_ROOT") != "" {
			absolute, err := filepath.Abs(os.Getenv("SEEN_TRUST_ROOT"))
			if err != nil {
				return nil, err
			}
			spec.TrustedRoot = absolute
		}
		if alias == "default" && spec.TrustedRootSHA256 == "" && os.Getenv("SEEN_TRUST_ROOT_SHA256") != "" {
			spec.TrustedRootSHA256 = os.Getenv("SEEN_TRUST_ROOT_SHA256")
			if err := model.ValidateSHA256(spec.TrustedRootSHA256); err != nil {
				return nil, err
			}
		}
		identity, found, err := trustedRegistryIdentity(cacheRoot, origin)
		if err != nil {
			return nil, fmt.Errorf("registry %q trusted state: %w", alias, err)
		}
		if found {
			if spec.Environment != "" && spec.Environment != identity.Environment {
				return nil, fmt.Errorf("registry %q environment %q conflicts with pinned trusted root environment %q", alias, spec.Environment, identity.Environment)
			}
			if spec.RepositoryID != "" && spec.RepositoryID != identity.RepositoryID {
				return nil, fmt.Errorf("registry %q repository ID %q conflicts with pinned trusted root repository ID %q", alias, spec.RepositoryID, identity.RepositoryID)
			}
			spec.Environment = identity.Environment
			spec.RepositoryID = identity.RepositoryID
		}
		if spec.Environment == "" || spec.RepositoryID == "" {
			return nil, fmt.Errorf("custom registry %q requires --environment %s=development|production and --repository-id %s=ID for its initial pinned-root bootstrap", alias, alias, alias)
		}
		if prior, exists := byOrigin[origin]; exists {
			if prior.Environment != spec.Environment || prior.RepositoryID != spec.RepositoryID || prior.TrustedRoot != spec.TrustedRoot || prior.TrustedRootSHA256 != spec.TrustedRootSHA256 {
				return nil, fmt.Errorf("registry aliases for %s have inconsistent trust configuration", origin)
			}
			continue
		}
		byOrigin[origin] = spec
	}
	result := make([]registrySpec, 0, len(byOrigin))
	for _, spec := range byOrigin {
		result = append(result, spec)
	}
	sort.Slice(result, func(i, j int) bool { return result[i].Origin < result[j].Origin })
	return result, nil
}

func trustedRegistryIdentity(cacheRoot, origin string) (tuf.TrustedIdentity, bool, error) {
	store := &tuf.FileStore{Path: filepath.Join(cacheRoot, "metadata", originDigest(origin), "trusted-state.json")}
	state, err := store.Load()
	if errors.Is(err, os.ErrNotExist) {
		return tuf.TrustedIdentity{}, false, nil
	}
	if err != nil {
		return tuf.TrustedIdentity{}, false, err
	}
	identity, err := tuf.IdentityFromTrustedState(state)
	if err != nil {
		return tuf.TrustedIdentity{}, false, err
	}
	return identity, true, nil
}

func materializeResolution(ctx context.Context, projectRoot string, shared *cache.Cache, runtime *registryRuntime, resolution *model.Resolution, offline bool) (map[model.PackageKey]string, error) {
	views := make(map[model.PackageKey]string, len(resolution.Packages))
	for _, pkg := range resolution.Packages {
		key, err := cacheKey(pkg)
		if err != nil {
			return nil, err
		}
		candidate := candidateFromLocked(pkg)
		target, err := runtime.target(ctx, candidate)
		if err != nil {
			return nil, err
		}
		options := seenarchive.Options{ExpectedSHA256: pkg.ArchiveSHA256, Limits: seenarchive.DefaultLimits(), Binder: &signedPackageBinder{candidate: candidate}}
		if _, lookupErr := shared.Lookup(ctx, key); lookupErr == nil {
			blob, err := shared.BlobPath(pkg.ArchiveSHA256)
			if err != nil {
				return nil, err
			}
			if _, err := seenarchive.Preflight(ctx, blob, options); err != nil {
				return nil, err
			}
		} else {
			if offline {
				if errors.Is(lookupErr, os.ErrNotExist) {
					return nil, resolver.ErrOfflineDataUnavailable
				}
				return nil, lookupErr
			}
			downloadDir, err := os.MkdirTemp(shared.Root, ".download-*")
			if err != nil {
				return nil, err
			}
			archivePath := filepath.Join(downloadDir, "archive.seenpkg.tgz")
			_, downloadErr := transport.Download(ctx, transport.NewClient(transport.DefaultPolicy()), blobURL(pkg.RegistryOrigin, pkg.ArchiveSHA256), archivePath, transport.Expectation{SHA256: pkg.ArchiveSHA256, Length: target.Length, Origin: pkg.RegistryOrigin}, transport.DefaultPolicy())
			if downloadErr == nil {
				_, downloadErr = shared.Install(ctx, key, archivePath, options)
			}
			os.RemoveAll(downloadDir)
			if downloadErr != nil {
				return nil, downloadErr
			}
		}
		view, err := shared.CreateProjectView(ctx, key, projectRoot)
		if err != nil {
			return nil, err
		}
		views[pkg.Key()] = view
	}
	return views, nil
}

func cacheKey(pkg model.LockedPackage) (cache.Key, error) {
	parts := strings.Split(pkg.Package, "/")
	if len(parts) != 2 {
		return cache.Key{}, fmt.Errorf("invalid locked package identity")
	}
	key := cache.Key{Owner: parts[0], Name: parts[1], Version: pkg.Version, SHA256: pkg.ArchiveSHA256}
	return key, key.Validate()
}
func candidateFromLocked(pkg model.LockedPackage) model.Candidate {
	return model.Candidate{Package: pkg.Package, Version: pkg.Version, RegistryOrigin: pkg.RegistryOrigin, ArchiveSHA256: pkg.ArchiveSHA256, TargetPath: pkg.TargetPath, MetadataVersion: pkg.MetadataVersion, Capabilities: pkg.Capabilities, Dependencies: pkg.Dependencies}
}

func packageMapRows(projectRoot string, resolution *model.Resolution, views map[model.PackageKey]string) ([]PackageMapRow, error) {
	var rows []PackageMapRow
	for _, edge := range resolution.Root.Dependencies {
		dependencyRoot, ok := views[edge.Key()]
		if !ok {
			return nil, fmt.Errorf("root edge %s has no project view", edge.Alias)
		}
		rows = append(rows, PackageMapRow{RequesterRoot: projectRoot, Alias: edge.Alias, DependencyRoot: dependencyRoot})
	}
	for _, pkg := range resolution.Packages {
		requester := views[pkg.Key()]
		for _, edge := range pkg.Dependencies {
			dependencyRoot, ok := views[edge.Key()]
			if !ok {
				return nil, fmt.Errorf("%s edge %s has no project view", pkg.Package, edge.Alias)
			}
			rows = append(rows, PackageMapRow{RequesterRoot: requester, Alias: edge.Alias, DependencyRoot: dependencyRoot})
		}
	}
	return rows, nil
}

type signedPackageBinder struct{ candidate model.Candidate }

func (binder *signedPackageBinder) BindManifest(raw []byte, paths []string) error {
	parsed, err := manifest.ParseWithOptions(raw, manifest.Options{DefaultRegistryOrigin: binder.candidate.RegistryOrigin, RequireManifestV1: true})
	if err != nil {
		return err
	}
	if parsed.Package == nil || parsed.Package.Identity != binder.candidate.Package || parsed.Project.Version != binder.candidate.Version || !sameCapabilityList(parsed.Package.Capabilities, binder.candidate.Capabilities) {
		return fmt.Errorf("archive manifest identity, version, or capabilities differ from signed metadata")
	}
	for _, dependency := range parsed.Dependencies {
		if dependency.Kind != model.DependencyRegistry {
			return fmt.Errorf("hosted package contains non-registry dependency %q", dependency.Alias)
		}
	}
	actual, expected := manifest.Root(parsed).Dependencies, append([]model.Edge(nil), binder.candidate.Dependencies...)
	sort.Slice(actual, func(i, j int) bool { return actual[i].Alias < actual[j].Alias })
	sort.Slice(expected, func(i, j int) bool { return expected[i].Alias < expected[j].Alias })
	if len(actual) != len(expected) {
		return fmt.Errorf("archive manifest dependency graph differs from signed metadata")
	}
	for index := range actual {
		left, right := actual[index], expected[index]
		if left.Alias != right.Alias || left.Package != right.Package || left.RegistryOrigin != right.RegistryOrigin || left.Requirement != right.Requirement || !sameCapabilityList(left.Allow, right.Allow) {
			return fmt.Errorf("archive manifest dependency %q differs from signed metadata", left.Alias)
		}
	}
	for _, name := range paths {
		if seenarchive.IsReservedPackageStatePath(name) {
			return fmt.Errorf("archive member %q attempts to provide project package-manager state", name)
		}
		if name == "Seen.toml" || isEffectiveDirectory(name, paths) {
			continue
		}
		allowed := false
		for _, pattern := range append(append([]string(nil), parsed.Package.Include...), parsed.Package.Assets...) {
			if globMatch(pattern, name) {
				allowed = true
				break
			}
		}
		if !allowed {
			return fmt.Errorf("archive member %q is outside package include/assets", name)
		}
	}
	return nil
}
