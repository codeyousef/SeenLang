// Package manifest parses and validates Seen.toml package-manager fields using
// a standards-compliant TOML parser. Legacy/local reads tolerate unrelated
// compiler tables; hosted package manifests use the closed v1 structure.
package manifest

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"net/url"
	"os"
	"path"
	"sort"
	"strings"

	"github.com/codeyousef/seen/tools/seen-pkg/internal/model"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/semver"
	"github.com/pelletier/go-toml/v2"
)

type rawManifest struct {
	ManifestVersion int                 `toml:"manifest-version"`
	Project         rawProject          `toml:"project"`
	Package         *rawPackage         `toml:"package"`
	Registries      map[string]string   `toml:"registries"`
	Dependencies    map[string]any      `toml:"dependencies"`
	PackageGrants   map[string][]string `toml:"package-grants"`
	Native          *rawNative          `toml:"native"`
}

type rawProject struct {
	Name        string    `toml:"name"`
	Version     string    `toml:"version"`
	Language    *string   `toml:"language"`
	Description *string   `toml:"description"`
	Authors     *[]string `toml:"authors"`
	Edition     *string   `toml:"edition"`
	Modules     *[]string `toml:"modules"`
	License     *string   `toml:"license"`
	Repository  *string   `toml:"repository"`
}

type rawPackage struct {
	Identity     string   `toml:"identity"`
	Visibility   string   `toml:"visibility"`
	Include      []string `toml:"include"`
	Assets       []string `toml:"assets"`
	Capabilities []string `toml:"capabilities"`
}

type rawNative struct {
	Dependencies map[string]any `toml:"dependencies"`
}

type Options struct {
	DefaultRegistryOrigin string
	RequireManifestV1     bool
}

func Load(filename string, options Options) (*model.Manifest, error) {
	content, err := os.ReadFile(filename)
	if err != nil {
		return nil, fmt.Errorf("read manifest: %w", err)
	}
	return ParseWithOptions(content, options)
}

func Parse(content []byte) (*model.Manifest, error) {
	return ParseWithOptions(content, Options{})
}

func ParseWithOptions(content []byte, options Options) (*model.Manifest, error) {
	var raw rawManifest
	decoder := toml.NewDecoder(strings.NewReader(string(content)))
	if options.RequireManifestV1 {
		decoder.DisallowUnknownFields()
	}
	if err := decoder.Decode(&raw); err != nil {
		if strict, ok := err.(*toml.StrictMissingError); ok {
			return nil, fmt.Errorf("parse Seen.toml: %s", strict.String())
		}
		return nil, fmt.Errorf("parse Seen.toml: %w", err)
	}
	if options.RequireManifestV1 && raw.ManifestVersion != 1 {
		return nil, fmt.Errorf("manifest-version must be 1")
	}
	if raw.ManifestVersion != 0 && raw.ManifestVersion != 1 {
		return nil, fmt.Errorf("unsupported manifest-version %d", raw.ManifestVersion)
	}
	if options.RequireManifestV1 {
		var document map[string]any
		if err := toml.Unmarshal(content, &document); err != nil {
			return nil, fmt.Errorf("parse strict Seen.toml structure: %w", err)
		}
		if _, present := document["package"]; !present || raw.Package == nil {
			return nil, fmt.Errorf("[package] is required by the strict manifest v1 contract")
		}
		if _, present := document["dependencies"]; !present {
			return nil, fmt.Errorf("[dependencies] is required by the strict manifest v1 contract")
		}
		if raw.Native != nil && len(raw.Native.Dependencies) != 0 {
			return nil, fmt.Errorf("[native.dependencies] must be empty in the strict manifest v1 contract")
		}
	}
	if raw.Project.Name == "" || raw.Project.Version == "" {
		return nil, fmt.Errorf("[project] name and version are required")
	}
	if options.RequireManifestV1 {
		if err := validateStrictProject(raw.Project); err != nil {
			return nil, err
		}
	}
	if _, err := semver.Parse(raw.Project.Version); err != nil {
		return nil, fmt.Errorf("project.version: %w", err)
	}
	result := &model.Manifest{
		ManifestVersion: raw.ManifestVersion,
		Project:         model.Project{Name: raw.Project.Name, Version: raw.Project.Version},
		Registries:      make(map[string]string, len(raw.Registries)),
		Grants:          make(map[string][]model.Capability, len(raw.PackageGrants)),
		Raw:             append([]byte(nil), content...),
	}
	for alias, origin := range raw.Registries {
		if err := model.ValidateAlias(alias); err != nil {
			return nil, fmt.Errorf("registries.%s: %w", alias, err)
		}
		if err := model.ValidateRegistryOrigin(origin); err != nil {
			return nil, fmt.Errorf("registries.%s: signed registry origins must be canonical HTTPS URLs; unsigned local-directory registries were replaced by explicit dependency { path = \"...\" } for development: %w", alias, err)
		}
		result.Registries[alias] = origin
	}
	if options.DefaultRegistryOrigin != "" {
		if err := model.ValidateRegistryOrigin(options.DefaultRegistryOrigin); err != nil {
			return nil, fmt.Errorf("default registry: %w", err)
		}
		if _, exists := result.Registries["default"]; !exists {
			result.Registries["default"] = options.DefaultRegistryOrigin
		}
	}
	if raw.Package != nil {
		pkg, err := parsePackage(raw.Package)
		if err != nil {
			return nil, err
		}
		result.Package = pkg
	}
	for identity, rawCapabilities := range raw.PackageGrants {
		if err := model.ValidateIdentity(identity); err != nil {
			return nil, fmt.Errorf("package-grants.%s: %w", identity, err)
		}
		values, err := parseCapabilities(rawCapabilities)
		if err != nil {
			return nil, fmt.Errorf("package-grants.%s: %w", identity, err)
		}
		result.Grants[identity] = values
	}
	aliases := make([]string, 0, len(raw.Dependencies))
	for alias := range raw.Dependencies {
		aliases = append(aliases, alias)
	}
	sort.Strings(aliases)
	for _, alias := range aliases {
		if err := model.ValidateAlias(alias); err != nil {
			return nil, fmt.Errorf("dependencies.%s: %w", alias, err)
		}
		if alias == raw.Project.Name {
			return nil, fmt.Errorf("dependency alias %q collides with project import root", alias)
		}
		dependency, err := parseDependency(alias, raw.Dependencies[alias], result.Registries, options.RequireManifestV1)
		if err != nil {
			return nil, fmt.Errorf("dependencies.%s: %w", alias, err)
		}
		result.Dependencies = append(result.Dependencies, dependency)
	}
	if raw.ManifestVersion == 1 || result.Package != nil || hasRegistryDependencies(result.Dependencies) {
		if err := model.ValidateRootName(raw.Project.Name); err != nil {
			return nil, err
		}
		if raw.ManifestVersion != 1 {
			return nil, fmt.Errorf("manifest-version = 1 is required for hosted package fields")
		}
	}
	return result, nil
}

func validateStrictProject(project rawProject) error {
	if project.Language != nil {
		switch *project.Language {
		case "en", "ar", "es", "ru", "zh", "ja":
		default:
			return fmt.Errorf("project.language must be one of en, ar, es, ru, zh, or ja")
		}
	}
	if project.License != nil && *project.License == "" {
		return fmt.Errorf("project.license must be nonempty when declared")
	}
	if project.Repository != nil {
		repository, err := url.ParseRequestURI(*project.Repository)
		if err != nil || !repository.IsAbs() {
			return fmt.Errorf("project.repository must be an absolute URI")
		}
	}
	return nil
}

func hasRegistryDependencies(dependencies []model.Dependency) bool {
	for _, dependency := range dependencies {
		if dependency.Kind == model.DependencyRegistry {
			return true
		}
	}
	return false
}

func parsePackage(raw *rawPackage) (*model.Package, error) {
	if err := model.ValidateIdentity(raw.Identity); err != nil {
		return nil, fmt.Errorf("package.identity: %w", err)
	}
	if raw.Visibility != "public" && raw.Visibility != "private" {
		return nil, fmt.Errorf("package.visibility must be public or private")
	}
	if raw.Include == nil || raw.Assets == nil || raw.Capabilities == nil {
		return nil, fmt.Errorf("package.include, package.assets, and package.capabilities must be declared arrays")
	}
	if err := validateMembers("package.include", raw.Include); err != nil {
		return nil, err
	}
	if err := validateMembers("package.assets", raw.Assets); err != nil {
		return nil, err
	}
	capabilities, err := parseCapabilities(raw.Capabilities)
	if err != nil {
		return nil, fmt.Errorf("package.capabilities: %w", err)
	}
	return &model.Package{
		Identity: raw.Identity, Visibility: raw.Visibility,
		Include: append([]string(nil), raw.Include...), Assets: append([]string(nil), raw.Assets...),
		Capabilities: capabilities,
	}, nil
}

func validateMembers(field string, values []string) error {
	seen := make(map[string]struct{}, len(values))
	for _, value := range values {
		if value == "" || strings.ContainsAny(value, "\x00\r\n\\") || strings.HasPrefix(value, "/") {
			return fmt.Errorf("%s contains unsafe relative member %q", field, value)
		}
		for _, segment := range strings.Split(value, "/") {
			if segment == ".." || segment == "." || segment == "" {
				return fmt.Errorf("%s contains non-canonical member %q", field, value)
			}
		}
		if _, duplicate := seen[value]; duplicate {
			return fmt.Errorf("%s contains duplicate member %q", field, value)
		}
		seen[value] = struct{}{}
	}
	return nil
}

func parseDependency(alias string, value any, registries map[string]string, strictV1 bool) (model.Dependency, error) {
	if legacy, ok := value.(string); ok {
		if strictV1 {
			return model.Dependency{}, fmt.Errorf("strict manifest v1 dependencies must use an explicit inline table")
		}
		if err := validateLocalPath(legacy); err != nil {
			return model.Dependency{}, err
		}
		return model.Dependency{Alias: alias, Kind: model.DependencyPath, Path: legacy}, nil
	}
	table, ok := value.(map[string]any)
	if !ok {
		return model.Dependency{}, fmt.Errorf("must be a TOML string or inline table")
	}
	allowedFields := map[string]struct{}{"package": {}, "version": {}, "registry": {}, "allow": {}, "path": {}, "artifact": {}, "system": {}}
	for key := range table {
		if _, allowed := allowedFields[key]; !allowed {
			return model.Dependency{}, fmt.Errorf("unknown field %q", key)
		}
	}
	packageName, packagePresent, err := stringField(table, "package")
	if err != nil {
		return model.Dependency{}, err
	}
	version, versionPresent, err := stringField(table, "version")
	if err != nil {
		return model.Dependency{}, err
	}
	registryAlias, registryPresent, err := stringField(table, "registry")
	if err != nil {
		return model.Dependency{}, err
	}
	pathValue, pathPresent, err := stringField(table, "path")
	if err != nil {
		return model.Dependency{}, err
	}
	artifact, artifactPresent, err := stringField(table, "artifact")
	if err != nil {
		return model.Dependency{}, err
	}
	system, systemPresent, err := boolField(table, "system")
	if err != nil {
		return model.Dependency{}, err
	}
	allow, err := capabilityField(table, "allow")
	if err != nil {
		return model.Dependency{}, err
	}
	_, allowPresent := table["allow"]
	sources := 0
	if packagePresent {
		sources++
	}
	if pathPresent {
		sources++
	}
	if artifactPresent {
		sources++
	}
	if systemPresent {
		if !system || !pathPresent || len(table) != 2 {
			return model.Dependency{}, fmt.Errorf("system dependency must be exactly { system = true, path = \"relative/path\" }")
		}
		if err := validateSafeSystemPath(pathValue); err != nil {
			return model.Dependency{}, err
		}
		return model.Dependency{Alias: alias, Kind: model.DependencySystem, Path: pathValue}, nil
	}
	if sources != 1 {
		return model.Dependency{}, fmt.Errorf("must select exactly one of package, path, or artifact")
	}
	if packagePresent {
		if !versionPresent || pathPresent || artifactPresent {
			return model.Dependency{}, fmt.Errorf("registry dependency requires package and version only")
		}
		if err := model.ValidateIdentity(packageName); err != nil {
			return model.Dependency{}, err
		}
		if _, err := semver.ParseRequirement(version); err != nil {
			return model.Dependency{}, err
		}
		if !registryPresent {
			registryAlias = "default"
		}
		if err := model.ValidateAlias(registryAlias); err != nil {
			return model.Dependency{}, fmt.Errorf("registry alias: %w", err)
		}
		origin, exists := registries[registryAlias]
		if !exists {
			return model.Dependency{}, fmt.Errorf("unknown registry alias %q", registryAlias)
		}
		return model.Dependency{Alias: alias, Kind: model.DependencyRegistry, Package: packageName, Requirement: version, RegistryAlias: registryAlias, RegistryOrigin: origin, Allow: allow}, nil
	}
	if versionPresent || registryPresent || allowPresent {
		return model.Dependency{}, fmt.Errorf("local dependency cannot contain version, registry, or allow")
	}
	if pathPresent {
		if err := validateLocalPath(pathValue); err != nil {
			return model.Dependency{}, err
		}
		return model.Dependency{Alias: alias, Kind: model.DependencyPath, Path: pathValue}, nil
	}
	if err := validateLocalPath(artifact); err != nil {
		return model.Dependency{}, err
	}
	return model.Dependency{Alias: alias, Kind: model.DependencyArtifact, Artifact: artifact}, nil
}

func stringField(table map[string]any, key string) (string, bool, error) {
	value, exists := table[key]
	if !exists {
		return "", false, nil
	}
	text, ok := value.(string)
	if !ok || text == "" {
		return "", true, fmt.Errorf("%s must be a nonempty string", key)
	}
	return text, true, nil
}

func boolField(table map[string]any, key string) (bool, bool, error) {
	value, exists := table[key]
	if !exists {
		return false, false, nil
	}
	boolean, ok := value.(bool)
	if !ok {
		return false, true, fmt.Errorf("%s must be a boolean", key)
	}
	return boolean, true, nil
}

func capabilityField(table map[string]any, key string) ([]model.Capability, error) {
	value, exists := table[key]
	if !exists {
		return []model.Capability{}, nil
	}
	items, ok := value.([]any)
	if !ok {
		return nil, fmt.Errorf("%s must be an array of capabilities", key)
	}
	values := make([]string, 0, len(items))
	for _, item := range items {
		text, ok := item.(string)
		if !ok {
			return nil, fmt.Errorf("%s must contain only strings", key)
		}
		values = append(values, text)
	}
	return parseCapabilities(values)
}

func parseCapabilities(values []string) ([]model.Capability, error) {
	result := make([]model.Capability, len(values))
	for index, value := range values {
		result[index] = model.Capability(value)
	}
	if err := model.ValidateCapabilities(result); err != nil {
		return nil, err
	}
	return model.CanonicalCapabilities(result), nil
}

func validateLocalPath(value string) error {
	if value == "" || strings.ContainsAny(value, "\x00\r\n") {
		return fmt.Errorf("local path must be a nonempty single-line string")
	}
	clean := path.Clean(strings.ReplaceAll(value, "\\", "/"))
	if clean == "." && value != "." {
		return fmt.Errorf("local path %q is not canonical", value)
	}
	return nil
}

func validateSafeSystemPath(value string) error {
	if value == "" || strings.ContainsAny(value, "\x00\r\n\\") || strings.HasPrefix(value, "/") || strings.Contains(value, ":") {
		return fmt.Errorf("system dependency path must be a safe relative slash path")
	}
	for _, segment := range strings.Split(value, "/") {
		if segment == "" || segment == "." || segment == ".." {
			return fmt.Errorf("system dependency path must not contain empty or traversal segments")
		}
	}
	return nil
}

func Digest(content []byte) string {
	sum := sha256.Sum256(content)
	return hex.EncodeToString(sum[:])
}

func Root(manifest *model.Manifest) model.Root {
	root := model.Root{Name: manifest.Project.Name, Version: manifest.Project.Version, Grants: make(map[string][]model.Capability, len(manifest.Grants))}
	for identity, values := range manifest.Grants {
		root.Grants[identity] = append([]model.Capability(nil), values...)
	}
	for _, dependency := range manifest.Dependencies {
		if dependency.Kind != model.DependencyRegistry {
			continue
		}
		root.Dependencies = append(root.Dependencies, model.Edge{
			Alias: dependency.Alias, Package: dependency.Package, RegistryOrigin: dependency.RegistryOrigin,
			Requirement: dependency.Requirement, Allow: append([]model.Capability(nil), dependency.Allow...),
		})
	}
	return root
}
