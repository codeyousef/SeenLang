// Package lockfile parses, validates, and atomically writes Seen.lock v2.
package lockfile

import (
	"bytes"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"

	"github.com/codeyousef/seen/tools/seen-pkg/internal/atomicfile"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/model"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/semver"
	"github.com/pelletier/go-toml/v2"
)

func Parse(content []byte) (*model.Lock, error) {
	var lock model.Lock
	decoder := toml.NewDecoder(bytes.NewReader(content)).DisallowUnknownFields()
	if err := decoder.Decode(&lock); err != nil {
		return nil, fmt.Errorf("parse Seen.lock: %w", err)
	}
	if err := Validate(&lock); err != nil {
		return nil, err
	}
	return &lock, nil
}

func Load(filename string) (*model.Lock, error) {
	content, err := os.ReadFile(filename)
	if err != nil {
		return nil, fmt.Errorf("read Seen.lock: %w", err)
	}
	return Parse(content)
}

func Validate(lock *model.Lock) error {
	if lock == nil {
		return fmt.Errorf("lock is nil")
	}
	if lock.Version != 2 {
		return fmt.Errorf("Seen.lock version must be 2")
	}
	if err := model.ValidateSHA256(lock.ManifestSHA256); err != nil {
		return fmt.Errorf("manifest_sha256: %w", err)
	}
	if err := validateRoot(lock.Root); err != nil {
		return err
	}
	if !edgesCanonical(lock.Root.Dependencies) {
		return fmt.Errorf("root dependencies are not in canonical alias order")
	}
	nodes := make(map[model.PackageKey]model.LockedPackage, len(lock.Packages))
	for index, pkg := range lock.Packages {
		if err := validatePackage(pkg); err != nil {
			return fmt.Errorf("packages[%d]: %w", index, err)
		}
		if _, exists := nodes[pkg.Key()]; exists {
			return fmt.Errorf("duplicate package node %s", pkg.Key())
		}
		nodes[pkg.Key()] = pkg
		if index > 0 && !packageLess(lock.Packages[index-1], pkg) {
			return fmt.Errorf("package nodes are not in canonical order")
		}
	}
	for _, edge := range lock.Root.Dependencies {
		if err := validateBinding(edge, nodes); err != nil {
			return fmt.Errorf("root edge %s: %w", edge.Alias, err)
		}
	}
	for _, pkg := range lock.Packages {
		for _, edge := range pkg.Dependencies {
			if err := validateBinding(edge, nodes); err != nil {
				return fmt.Errorf("%s edge %s: %w", pkg.Package, edge.Alias, err)
			}
		}
	}
	if err := validateClosure(lock, nodes); err != nil {
		return err
	}
	return nil
}

func validateRoot(root model.Root) error {
	if err := model.ValidateRootName(root.Name); err != nil {
		return fmt.Errorf("root.name: %w", err)
	}
	if _, err := semver.Parse(root.Version); err != nil {
		return fmt.Errorf("root.version: %w", err)
	}
	aliases := map[string]struct{}{}
	for _, edge := range root.Dependencies {
		if err := validateEdge(edge); err != nil {
			return err
		}
		if _, exists := aliases[edge.Alias]; exists {
			return fmt.Errorf("duplicate root alias %q", edge.Alias)
		}
		aliases[edge.Alias] = struct{}{}
	}
	return nil
}

func validatePackage(pkg model.LockedPackage) error {
	if err := model.ValidateIdentity(pkg.Package); err != nil {
		return err
	}
	if _, err := semver.Parse(pkg.Version); err != nil {
		return err
	}
	if pkg.Source != "hosted-registry" {
		return fmt.Errorf("source must be hosted-registry")
	}
	if err := model.ValidateRegistryOrigin(pkg.RegistryOrigin); err != nil {
		return err
	}
	if err := model.ValidateSHA256(pkg.ArchiveSHA256); err != nil {
		return err
	}
	expected, err := model.TargetPath(pkg.Package, pkg.Version, pkg.ArchiveSHA256)
	if err != nil {
		return err
	}
	if pkg.TargetPath != expected {
		return fmt.Errorf("target_path is not bound to package, version, and digest")
	}
	if pkg.MetadataVersion == 0 {
		return fmt.Errorf("metadata_version must be positive")
	}
	if err := model.ValidateCapabilities(pkg.Capabilities); err != nil {
		return err
	}
	if err := model.ValidateCapabilities(pkg.Grants); err != nil {
		return err
	}
	if !capabilitiesCanonical(pkg.Capabilities) || !capabilitiesCanonical(pkg.Grants) {
		return fmt.Errorf("capabilities and grants must be in canonical order")
	}
	if missing := model.MissingCapabilities(pkg.Grants, pkg.Capabilities); len(missing) != 0 {
		return fmt.Errorf("root grants do not cover capabilities %v", missing)
	}
	if !edgesCanonical(pkg.Dependencies) {
		return fmt.Errorf("dependencies are not in canonical alias order")
	}
	aliases := map[string]struct{}{}
	for _, edge := range pkg.Dependencies {
		if err := validateEdge(edge); err != nil {
			return err
		}
		if _, exists := aliases[edge.Alias]; exists {
			return fmt.Errorf("duplicate dependency alias %q", edge.Alias)
		}
		aliases[edge.Alias] = struct{}{}
	}
	return nil
}

func validateEdge(edge model.Edge) error {
	if err := model.ValidateAlias(edge.Alias); err != nil {
		return err
	}
	if err := model.ValidateIdentity(edge.Package); err != nil {
		return err
	}
	if err := model.ValidateRegistryOrigin(edge.RegistryOrigin); err != nil {
		return err
	}
	if _, err := semver.ParseRequirement(edge.Requirement); err != nil {
		return err
	}
	if _, err := semver.Parse(edge.ResolvedVersion); err != nil {
		return fmt.Errorf("resolved_version: %w", err)
	}
	if err := model.ValidateSHA256(edge.ResolvedArchiveSHA256); err != nil {
		return fmt.Errorf("resolved_archive_sha256: %w", err)
	}
	if err := model.ValidateCapabilities(edge.Allow); err != nil {
		return err
	}
	if !capabilitiesCanonical(edge.Allow) {
		return fmt.Errorf("allow must be in canonical order")
	}
	return nil
}

func validateBinding(edge model.Edge, nodes map[model.PackageKey]model.LockedPackage) error {
	pkg, exists := nodes[edge.Key()]
	if !exists {
		return fmt.Errorf("does not resolve to a package node")
	}
	if pkg.Version != edge.ResolvedVersion || pkg.ArchiveSHA256 != edge.ResolvedArchiveSHA256 {
		return fmt.Errorf("resolved tuple does not match package node")
	}
	if missing := model.MissingCapabilities(edge.Allow, pkg.Capabilities); len(missing) != 0 {
		return fmt.Errorf("allow does not cover capabilities %v", missing)
	}
	return nil
}

func validateClosure(lock *model.Lock, nodes map[model.PackageKey]model.LockedPackage) error {
	reachable := map[model.PackageKey]bool{}
	queue := make([]model.PackageKey, 0, len(lock.Root.Dependencies))
	for _, edge := range lock.Root.Dependencies {
		queue = append(queue, edge.Key())
	}
	for len(queue) != 0 {
		key := queue[0]
		queue = queue[1:]
		if reachable[key] {
			continue
		}
		reachable[key] = true
		for _, edge := range nodes[key].Dependencies {
			queue = append(queue, edge.Key())
		}
	}
	for key := range nodes {
		if !reachable[key] {
			return fmt.Errorf("unreachable package node %s", key)
		}
	}
	return nil
}

func Enforce(lock *model.Lock, manifestSHA256 string, root model.Root) error {
	if err := Validate(lock); err != nil {
		return err
	}
	if lock.ManifestSHA256 != manifestSHA256 {
		return fmt.Errorf("locked_manifest_mismatch: Seen.toml changed")
	}
	if root.Name != lock.Root.Name || root.Version != lock.Root.Version || len(root.Dependencies) != len(lock.Root.Dependencies) {
		return fmt.Errorf("locked_requirement_mismatch: root changed")
	}
	left := canonicalEdges(root.Dependencies)
	right := canonicalEdges(lock.Root.Dependencies)
	for index := range left {
		l, r := left[index], right[index]
		if l.Alias != r.Alias || l.Package != r.Package || l.RegistryOrigin != r.RegistryOrigin || l.Requirement != r.Requirement || !sameCapabilities(l.Allow, r.Allow) {
			return fmt.Errorf("locked_requirement_mismatch: dependency %s changed", l.Alias)
		}
	}
	return nil
}

func FromResolution(manifestSHA256 string, resolution *model.Resolution) *model.Lock {
	lock := &model.Lock{Version: 2, ManifestSHA256: manifestSHA256, Root: resolution.Root, Packages: append([]model.LockedPackage(nil), resolution.Packages...)}
	canonicalize(lock)
	return lock
}

func Marshal(lock *model.Lock) ([]byte, error) {
	copy := cloneLock(lock)
	canonicalize(copy)
	if err := Validate(copy); err != nil {
		return nil, err
	}
	var output strings.Builder
	output.WriteString("version = 2\nmanifest_sha256 = ")
	writeString(&output, copy.ManifestSHA256)
	output.WriteString("\n\n[root]\nname = ")
	writeString(&output, copy.Root.Name)
	output.WriteString("\nversion = ")
	writeString(&output, copy.Root.Version)
	output.WriteByte('\n')
	if len(copy.Root.Dependencies) == 0 {
		output.WriteString("dependencies = []\n")
	} else {
		for _, edge := range copy.Root.Dependencies {
			output.WriteString("\n[[root.dependencies]]\n")
			writeEdge(&output, edge)
		}
	}
	for _, pkg := range copy.Packages {
		output.WriteString("\n[[packages]]\npackage = ")
		writeString(&output, pkg.Package)
		output.WriteString("\nversion = ")
		writeString(&output, pkg.Version)
		output.WriteString("\nsource = ")
		writeString(&output, pkg.Source)
		output.WriteString("\nregistry_origin = ")
		writeString(&output, pkg.RegistryOrigin)
		output.WriteString("\narchive_sha256 = ")
		writeString(&output, pkg.ArchiveSHA256)
		output.WriteString("\ntarget_path = ")
		writeString(&output, pkg.TargetPath)
		output.WriteString("\nmetadata_version = ")
		output.WriteString(strconv.FormatUint(pkg.MetadataVersion, 10))
		output.WriteString("\ncapabilities = ")
		writeCapabilities(&output, pkg.Capabilities)
		output.WriteString("\ngrants = ")
		writeCapabilities(&output, pkg.Grants)
		output.WriteByte('\n')
		if len(pkg.Dependencies) == 0 {
			output.WriteString("dependencies = []\n")
		} else {
			for _, edge := range pkg.Dependencies {
				output.WriteString("\n[[packages.dependencies]]\n")
				writeEdge(&output, edge)
			}
		}
	}
	return []byte(output.String()), nil
}

func Write(filename string, lock *model.Lock) error {
	content, err := Marshal(lock)
	if err != nil {
		return err
	}
	directory := filepath.Dir(filename)
	temp, err := os.CreateTemp(directory, ".Seen.lock.tmp-*")
	if err != nil {
		return fmt.Errorf("create lock temp file: %w", err)
	}
	tempName := temp.Name()
	ok := false
	defer func() {
		if !ok {
			_ = os.Remove(tempName)
		}
	}()
	if err := temp.Chmod(0o600); err != nil {
		_ = temp.Close()
		return err
	}
	if _, err := temp.Write(content); err != nil {
		_ = temp.Close()
		return fmt.Errorf("write lock temp file: %w", err)
	}
	if err := temp.Sync(); err != nil {
		_ = temp.Close()
		return fmt.Errorf("sync lock temp file: %w", err)
	}
	if err := temp.Close(); err != nil {
		return fmt.Errorf("close lock temp file: %w", err)
	}
	if err := atomicfile.Replace(tempName, filename); err != nil {
		return fmt.Errorf("atomically replace Seen.lock: %w", err)
	}
	if err := atomicfile.SyncDir(directory); err != nil {
		return fmt.Errorf("sync lock directory: %w", err)
	}
	ok = true
	return nil
}

func writeEdge(output *strings.Builder, edge model.Edge) {
	output.WriteString("alias = ")
	writeString(output, edge.Alias)
	output.WriteString("\npackage = ")
	writeString(output, edge.Package)
	output.WriteString("\nregistry_origin = ")
	writeString(output, edge.RegistryOrigin)
	output.WriteString("\nrequirement = ")
	writeString(output, edge.Requirement)
	output.WriteString("\nresolved_version = ")
	writeString(output, edge.ResolvedVersion)
	output.WriteString("\nresolved_archive_sha256 = ")
	writeString(output, edge.ResolvedArchiveSHA256)
	output.WriteString("\nallow = ")
	writeCapabilities(output, edge.Allow)
	output.WriteByte('\n')
}
func writeString(output *strings.Builder, value string) { output.WriteString(strconv.Quote(value)) }
func writeCapabilities(output *strings.Builder, values []model.Capability) {
	output.WriteByte('[')
	for index, value := range values {
		if index != 0 {
			output.WriteString(", ")
		}
		writeString(output, string(value))
	}
	output.WriteByte(']')
}
func canonicalize(lock *model.Lock) {
	lock.Root.Dependencies = canonicalEdges(lock.Root.Dependencies)
	for i := range lock.Packages {
		lock.Packages[i].Capabilities = model.CanonicalCapabilities(lock.Packages[i].Capabilities)
		lock.Packages[i].Grants = model.CanonicalCapabilities(lock.Packages[i].Grants)
		lock.Packages[i].Dependencies = canonicalEdges(lock.Packages[i].Dependencies)
	}
	sort.Slice(lock.Packages, func(i, j int) bool { return packageLess(lock.Packages[i], lock.Packages[j]) })
}
func canonicalEdges(values []model.Edge) []model.Edge {
	result := append([]model.Edge(nil), values...)
	for i := range result {
		result[i].Allow = model.CanonicalCapabilities(result[i].Allow)
	}
	sort.Slice(result, func(i, j int) bool { return result[i].Alias < result[j].Alias })
	return result
}
func edgesCanonical(values []model.Edge) bool {
	for i := range values {
		if i > 0 && values[i-1].Alias >= values[i].Alias {
			return false
		}
		if !capabilitiesCanonical(values[i].Allow) {
			return false
		}
	}
	return true
}
func capabilitiesCanonical(values []model.Capability) bool {
	for i := range values {
		if i > 0 && values[i-1] >= values[i] {
			return false
		}
	}
	return true
}
func packageLess(left, right model.LockedPackage) bool {
	if left.RegistryOrigin != right.RegistryOrigin {
		return left.RegistryOrigin < right.RegistryOrigin
	}
	if left.Package != right.Package {
		return left.Package < right.Package
	}
	if left.Version != right.Version {
		return left.Version < right.Version
	}
	return left.ArchiveSHA256 < right.ArchiveSHA256
}
func sameCapabilities(left, right []model.Capability) bool {
	l, r := model.CanonicalCapabilities(left), model.CanonicalCapabilities(right)
	if len(l) != len(r) {
		return false
	}
	for i := range l {
		if l[i] != r[i] {
			return false
		}
	}
	return true
}
func cloneLock(lock *model.Lock) *model.Lock {
	if lock == nil {
		return nil
	}
	result := *lock
	result.Root.Dependencies = append([]model.Edge(nil), lock.Root.Dependencies...)
	result.Packages = append([]model.LockedPackage(nil), lock.Packages...)
	for i := range result.Packages {
		result.Packages[i].Capabilities = append([]model.Capability(nil), result.Packages[i].Capabilities...)
		result.Packages[i].Grants = append([]model.Capability(nil), result.Packages[i].Grants...)
		result.Packages[i].Dependencies = append([]model.Edge(nil), result.Packages[i].Dependencies...)
	}
	return &result
}
