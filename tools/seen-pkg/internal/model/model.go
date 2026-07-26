// Package model contains the package-manager data model shared by manifest,
// resolution, lockfile, and command code.  It deliberately contains no I/O.
package model

import (
	"fmt"
	"net/url"
	"path"
	"regexp"
	"sort"
	"strings"
)

type Capability string

const (
	CapabilityFile        Capability = "file"
	CapabilityNetwork     Capability = "network"
	CapabilityProcess     Capability = "process"
	CapabilityEnvironment Capability = "environment"
	CapabilityDynamicLoad Capability = "dynamic-load"
	CapabilityFFI         Capability = "ffi"
	CapabilityUnsafe      Capability = "unsafe"
	CapabilityNativeLink  Capability = "native-link"
	CapabilityMacro       Capability = "macro"
)

var capabilities = map[Capability]struct{}{
	CapabilityFile: {}, CapabilityNetwork: {}, CapabilityProcess: {},
	CapabilityEnvironment: {}, CapabilityDynamicLoad: {}, CapabilityFFI: {},
	CapabilityUnsafe: {}, CapabilityNativeLink: {}, CapabilityMacro: {},
}

var (
	aliasPattern    = regexp.MustCompile(`^[A-Za-z_][A-Za-z0-9_]{0,63}$`)
	identitySegment = regexp.MustCompile(`^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$`)
	sha256Pattern   = regexp.MustCompile(`^[0-9a-f]{64}$`)
	originPathPart  = regexp.MustCompile(`^[a-z0-9][a-z0-9._~-]*$`)
)

var reservedAliases = map[string]struct{}{
	"lexer": {}, "parser": {}, "typechecker": {}, "macro": {},
	"bootstrap": {}, "codegen": {}, "lsp": {}, "tools": {},
	"optimization": {}, "trace": {}, "ir": {}, "types": {},
	"testing": {}, "ffi": {},
}

func ValidateAlias(value string) error {
	if !aliasPattern.MatchString(value) {
		return fmt.Errorf("alias %q must match [A-Za-z_][A-Za-z0-9_]{0,63}", value)
	}
	if _, reserved := reservedAliases[value]; reserved {
		return fmt.Errorf("alias %q is reserved by the Seen compiler", value)
	}
	return nil
}

func ValidateRootName(value string) error {
	if !aliasPattern.MatchString(value) {
		return fmt.Errorf("project name %q must match [A-Za-z_][A-Za-z0-9_]{0,63}", value)
	}
	return nil
}

func ValidateIdentity(value string) error {
	parts := strings.Split(value, "/")
	if len(parts) != 2 || !identitySegment.MatchString(parts[0]) || !identitySegment.MatchString(parts[1]) {
		return fmt.Errorf("package identity %q must be canonical owner/name", value)
	}
	return nil
}

// ValidateRegistryOrigin enforces the byte-exact hosted-v1 origin grammar.
// It never repairs, lowercases, decodes, or trims caller input.
func ValidateRegistryOrigin(value string) error {
	if value == "" || strings.TrimSpace(value) != value || strings.Contains(value, "%") {
		return fmt.Errorf("registry origin %q is not canonical", value)
	}
	u, err := url.Parse(value)
	if err != nil || u.Scheme != "https" || u.User != nil || u.RawQuery != "" || u.Fragment != "" || u.Port() != "" {
		return fmt.Errorf("registry origin %q must be an absolute canonical HTTPS URL", value)
	}
	if u.Hostname() == "" || u.Hostname() != strings.ToLower(u.Hostname()) || !strings.Contains(u.Hostname(), ".") || u.Host != u.Hostname() {
		return fmt.Errorf("registry origin %q must use a lowercase DNS host without a port", value)
	}
	for _, label := range strings.Split(u.Hostname(), ".") {
		if !identitySegment.MatchString(label) {
			return fmt.Errorf("registry origin %q has a non-canonical DNS label", value)
		}
	}
	if u.Path == "" || u.Path == "/" || strings.HasSuffix(u.Path, "/") || path.Clean(u.Path) != u.Path {
		return fmt.Errorf("registry origin %q must have canonical path segments and no trailing slash", value)
	}
	for _, segment := range strings.Split(strings.TrimPrefix(u.Path, "/"), "/") {
		if !originPathPart.MatchString(segment) {
			return fmt.Errorf("registry origin %q has a non-canonical path segment", value)
		}
	}
	if u.String() != value {
		return fmt.Errorf("registry origin %q is not byte-canonical", value)
	}
	return nil
}

func ValidateSHA256(value string) error {
	if !sha256Pattern.MatchString(value) {
		return fmt.Errorf("digest %q must be 64 lowercase hexadecimal characters", value)
	}
	return nil
}

func ValidateCapabilities(values []Capability) error {
	seen := make(map[Capability]struct{}, len(values))
	for _, capability := range values {
		if _, ok := capabilities[capability]; !ok {
			return fmt.Errorf("unknown package capability %q", capability)
		}
		if _, duplicate := seen[capability]; duplicate {
			return fmt.Errorf("duplicate package capability %q", capability)
		}
		seen[capability] = struct{}{}
	}
	return nil
}

func CanonicalCapabilities(values []Capability) []Capability {
	result := append([]Capability(nil), values...)
	sort.Slice(result, func(i, j int) bool { return result[i] < result[j] })
	return result
}

func MissingCapabilities(granted, requested []Capability) []Capability {
	have := make(map[Capability]struct{}, len(granted))
	for _, capability := range granted {
		have[capability] = struct{}{}
	}
	var missing []Capability
	for _, capability := range CanonicalCapabilities(requested) {
		if _, ok := have[capability]; !ok {
			missing = append(missing, capability)
		}
	}
	return missing
}

type DependencyKind string

const (
	DependencyRegistry DependencyKind = "registry"
	DependencyPath     DependencyKind = "path"
	DependencyArtifact DependencyKind = "artifact"
	DependencySystem   DependencyKind = "system"
)

type Dependency struct {
	Alias          string
	Kind           DependencyKind
	Package        string
	Requirement    string
	RegistryAlias  string
	RegistryOrigin string
	Path           string
	Artifact       string
	Allow          []Capability
}

type Project struct {
	Name    string
	Version string
}

type Package struct {
	Identity     string
	Visibility   string
	Include      []string
	Assets       []string
	Capabilities []Capability
}

type Manifest struct {
	ManifestVersion int
	Project         Project
	Package         *Package
	Registries      map[string]string
	Dependencies    []Dependency
	// Grants are root consent keyed by canonical package identity, including
	// transitive packages. They are separate from each publisher edge's Allow.
	Grants map[string][]Capability
	Raw    []byte
}

type PackageKey struct {
	RegistryOrigin string
	Package        string
}

func (key PackageKey) String() string { return key.RegistryOrigin + "#" + key.Package }

type Edge struct {
	Alias          string       `toml:"alias"`
	Package        string       `toml:"package"`
	RegistryOrigin string       `toml:"registry_origin"`
	Requirement    string       `toml:"requirement"`
	Allow          []Capability `toml:"allow"`

	ResolvedVersion       string `toml:"resolved_version"`
	ResolvedArchiveSHA256 string `toml:"resolved_archive_sha256"`
}

func (edge Edge) Key() PackageKey {
	return PackageKey{RegistryOrigin: edge.RegistryOrigin, Package: edge.Package}
}

type Root struct {
	Name         string                  `toml:"name"`
	Version      string                  `toml:"version"`
	Dependencies []Edge                  `toml:"dependencies"`
	Grants       map[string][]Capability `toml:"-"`
}

type Availability string

const (
	Available           Availability = "available"
	Yanked              Availability = "yanked"
	SecurityQuarantined Availability = "security-quarantined"
)

type Candidate struct {
	Package         string
	Version         string
	RegistryOrigin  string
	ArchiveSHA256   string
	TargetPath      string
	MetadataVersion uint64
	Availability    Availability
	Capabilities    []Capability
	Dependencies    []Edge
}

func (candidate Candidate) Key() PackageKey {
	return PackageKey{RegistryOrigin: candidate.RegistryOrigin, Package: candidate.Package}
}

type LockedPackage struct {
	Package         string       `toml:"package"`
	Version         string       `toml:"version"`
	Source          string       `toml:"source"`
	RegistryOrigin  string       `toml:"registry_origin"`
	ArchiveSHA256   string       `toml:"archive_sha256"`
	TargetPath      string       `toml:"target_path"`
	MetadataVersion uint64       `toml:"metadata_version"`
	Capabilities    []Capability `toml:"capabilities"`
	Grants          []Capability `toml:"grants"`
	Dependencies    []Edge       `toml:"dependencies"`
}

func (pkg LockedPackage) Key() PackageKey {
	return PackageKey{RegistryOrigin: pkg.RegistryOrigin, Package: pkg.Package}
}

type Lock struct {
	Version        int             `toml:"version"`
	ManifestSHA256 string          `toml:"manifest_sha256"`
	Root           Root            `toml:"root"`
	Packages       []LockedPackage `toml:"packages"`
}

type Resolution struct {
	Root     Root
	Packages []LockedPackage
	UsedLock bool
}

func TargetPath(packageIdentity, version, digest string) (string, error) {
	if err := ValidateIdentity(packageIdentity); err != nil {
		return "", err
	}
	if err := ValidateSHA256(digest); err != nil {
		return "", err
	}
	parts := strings.Split(packageIdentity, "/")
	return "packages/" + parts[0] + "/" + parts[1] + "/" + version + "/" + digest + "/" + parts[1] + "-" + version + ".seenpkg.tgz", nil
}
