// Package resolver implements deterministic complete graph resolution over
// canonical (registry origin, package identity) keys.
package resolver

import (
	"context"
	"errors"
	"fmt"
	"sort"
	"strings"

	"github.com/codeyousef/seen/tools/seen-pkg/internal/model"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/semver"
)

type Strategy string

const (
	Normal Strategy = "normal"
	Update Strategy = "update"
)

type Access struct{ Offline bool }

type Repository interface {
	Candidates(context.Context, model.PackageKey, Access) ([]model.Candidate, error)
}

var ErrOfflineDataUnavailable = errors.New("offline_data_unavailable")

type Options struct {
	Strategy Strategy
	Locked   bool
	Offline  bool
	Frozen   bool
	Lock     *model.Lock
}

type Error struct {
	Code               string
	Package            string
	Capabilities       []model.Capability
	ConflictRequesters []string
	Detail             string
}

func (err *Error) Error() string {
	message := err.Code
	if err.Package != "" {
		message += ": " + err.Package
	}
	if len(err.Capabilities) != 0 {
		message += " requires [" + capabilityText(err.Capabilities) + "]"
	}
	if len(err.ConflictRequesters) != 0 {
		message += " requested by " + strings.Join(err.ConflictRequesters, ", ")
	}
	if err.Detail != "" {
		message += ": " + err.Detail
	}
	return message
}

func capabilityText(values []model.Capability) string {
	parts := make([]string, len(values))
	for index, value := range values {
		parts[index] = string(value)
	}
	return strings.Join(parts, ", ")
}

type constraint struct {
	Requester   string
	Requirement semver.Requirement
	Edge        model.Edge
}

type state struct {
	constraints map[model.PackageKey][]constraint
	selected    map[model.PackageKey]model.Candidate
	fromLock    map[model.PackageKey]bool
}

type solver struct {
	ctx        context.Context
	repository Repository
	options    Options
	root       model.Root
	cache      map[model.PackageKey][]model.Candidate
	locked     map[model.PackageKey]model.LockedPackage
}

func Resolve(ctx context.Context, root model.Root, repository Repository, options Options) (*model.Resolution, error) {
	if repository == nil {
		return nil, fmt.Errorf("resolver repository is required")
	}
	if options.Strategy == "" {
		options.Strategy = Normal
	}
	if options.Strategy != Normal && options.Strategy != Update {
		return nil, &Error{Code: "invalid_mode_combination", Detail: "strategy must be normal or update"}
	}
	if options.Frozen {
		options.Locked = true
		options.Offline = true
	}
	if options.Strategy == Update && options.Locked {
		return nil, &Error{Code: "invalid_mode_combination", Detail: "update cannot be combined with locked or frozen"}
	}
	if options.Locked && options.Lock == nil {
		return nil, &Error{Code: "lock_required"}
	}
	if err := validateRoot(root); err != nil {
		return nil, err
	}
	s := &solver{ctx: ctx, repository: repository, options: options, root: canonicalRoot(root), cache: map[model.PackageKey][]model.Candidate{}, locked: map[model.PackageKey]model.LockedPackage{}}
	if options.Lock != nil {
		if err := s.indexLock(options.Lock); err != nil {
			return nil, err
		}
		if options.Locked {
			if err := lockedRootMatches(s.root, options.Lock.Root); err != nil {
				return nil, err
			}
		}
	}
	initial := &state{constraints: map[model.PackageKey][]constraint{}, selected: map[model.PackageKey]model.Candidate{}, fromLock: map[model.PackageKey]bool{}}
	for _, edge := range s.root.Dependencies {
		requirement, _ := semver.ParseRequirement(edge.Requirement)
		initial.constraints[edge.Key()] = append(initial.constraints[edge.Key()], constraint{Requester: "root", Requirement: requirement, Edge: edge})
	}
	resolved, failure := s.search(initial)
	if failure != nil {
		return nil, failure
	}
	if err := validateCapabilityConsent(s.root, resolved); err != nil {
		return nil, err
	}
	return s.buildResolution(resolved), nil
}

func validateRoot(root model.Root) error {
	if err := model.ValidateRootName(root.Name); err != nil {
		return err
	}
	if _, err := semver.Parse(root.Version); err != nil {
		return fmt.Errorf("root version: %w", err)
	}
	aliases := map[string]struct{}{}
	for _, edge := range root.Dependencies {
		if err := validateEdge(edge); err != nil {
			return fmt.Errorf("root dependency %q: %w", edge.Alias, err)
		}
		if _, duplicate := aliases[edge.Alias]; duplicate {
			return fmt.Errorf("duplicate root dependency alias %q", edge.Alias)
		}
		aliases[edge.Alias] = struct{}{}
	}
	for identity, grants := range root.Grants {
		if err := model.ValidateIdentity(identity); err != nil {
			return fmt.Errorf("root grant: %w", err)
		}
		if err := model.ValidateCapabilities(grants); err != nil {
			return fmt.Errorf("root grant %s: %w", identity, err)
		}
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
	if err := model.ValidateCapabilities(edge.Allow); err != nil {
		return err
	}
	return nil
}

func canonicalRoot(root model.Root) model.Root {
	result := root
	result.Dependencies = append([]model.Edge(nil), root.Dependencies...)
	for index := range result.Dependencies {
		result.Dependencies[index].Allow = model.CanonicalCapabilities(result.Dependencies[index].Allow)
	}
	sort.Slice(result.Dependencies, func(i, j int) bool { return result.Dependencies[i].Alias < result.Dependencies[j].Alias })
	result.Grants = make(map[string][]model.Capability, len(root.Grants))
	for identity, grants := range root.Grants {
		result.Grants[identity] = model.CanonicalCapabilities(grants)
	}
	return result
}

func (s *solver) indexLock(lock *model.Lock) error {
	for _, pkg := range lock.Packages {
		key := pkg.Key()
		if existing, duplicate := s.locked[key]; duplicate {
			return &Error{Code: "metadata_equivocation", Package: pkg.Package, Detail: "duplicate lock nodes " + existing.Version + " and " + pkg.Version}
		}
		s.locked[key] = pkg
	}
	// A package identity found under a different origin is never silently used.
	for key := range s.constraintsFromRoot() {
		for lockedKey := range s.locked {
			if lockedKey.Package == key.Package && lockedKey.RegistryOrigin != key.RegistryOrigin {
				return &Error{Code: "locked_origin_mismatch", Package: key.Package, Detail: "lock origin does not match manifest origin"}
			}
		}
	}
	return nil
}

func (s *solver) constraintsFromRoot() map[model.PackageKey]struct{} {
	keys := map[model.PackageKey]struct{}{}
	for _, edge := range s.root.Dependencies {
		keys[edge.Key()] = struct{}{}
	}
	return keys
}

func lockedRootMatches(root, locked model.Root) error {
	if root.Name != locked.Name || root.Version != locked.Version || len(root.Dependencies) != len(locked.Dependencies) {
		return &Error{Code: "locked_requirement_mismatch", Detail: "lock root differs from Seen.toml"}
	}
	left, right := canonicalRoot(root), canonicalRoot(locked)
	for index := range left.Dependencies {
		l, r := left.Dependencies[index], right.Dependencies[index]
		if l.Alias != r.Alias || l.Package != r.Package || l.RegistryOrigin != r.RegistryOrigin || l.Requirement != r.Requirement || !sameCapabilities(l.Allow, r.Allow) {
			return &Error{Code: "locked_requirement_mismatch", Package: l.Package, Detail: "lock edge differs from Seen.toml"}
		}
	}
	return nil
}

func (s *solver) search(current *state) (*state, *Error) {
	if err := s.validateSelected(current); err != nil {
		return nil, err
	}
	key, unresolved := nextUnresolved(current)
	if !unresolved {
		return current, nil
	}
	candidates, err := s.orderedCandidates(key, current.constraints[key])
	if err != nil {
		return nil, err
	}
	if len(candidates) == 0 {
		return nil, conflictError(key.Package, current.constraints[key])
	}
	var best *Error
	for _, option := range candidates {
		next := cloneState(current)
		next.selected[key] = option.candidate
		next.fromLock[key] = option.fromLock
		requester := option.candidate.Package + "@" + option.candidate.Version
		for _, edge := range option.candidate.Dependencies {
			requirement, _ := semver.ParseRequirement(edge.Requirement)
			next.constraints[edge.Key()] = append(next.constraints[edge.Key()], constraint{Requester: requester, Requirement: requirement, Edge: edge})
		}
		result, failure := s.search(next)
		if failure == nil {
			return result, nil
		}
		if !backtrackable(failure.Code) {
			return nil, failure
		}
		best = chooseFailure(best, failure)
	}
	if best == nil {
		best = conflictError(key.Package, current.constraints[key])
	}
	return nil, best
}

func (s *solver) validateSelected(current *state) *Error {
	keys := sortedKeys(current.selected)
	for _, key := range keys {
		candidate := current.selected[key]
		for _, requirement := range sortedConstraints(current.constraints[key]) {
			version, _ := semver.Parse(candidate.Version)
			if !requirement.Requirement.Matches(version) {
				return conflictError(key.Package, current.constraints[key])
			}
		}
	}
	return nil
}

type orderedCandidate struct {
	candidate model.Candidate
	fromLock  bool
}

func (s *solver) orderedCandidates(key model.PackageKey, constraints []constraint) ([]orderedCandidate, *Error) {
	all, err := s.loadCandidates(key)
	if err != nil {
		return nil, err
	}
	locked, hasLock := s.locked[key]
	if s.options.Locked && !hasLock {
		return nil, &Error{Code: "lock_required", Package: key.Package, Detail: "package is absent from lock"}
	}
	var matching []model.Candidate
	for _, candidate := range all {
		if !matchesAll(candidate.Version, constraints) {
			continue
		}
		isLocked := hasLock && locked.Version == candidate.Version && locked.ArchiveSHA256 == candidate.ArchiveSHA256
		if candidate.Availability == model.SecurityQuarantined {
			if hasLock && locked.Version == candidate.Version {
				return nil, &Error{Code: "locked_release_quarantined", Package: key.Package}
			}
			continue
		}
		if candidate.Availability == model.Yanked && !(isLocked && s.options.Strategy == Normal) {
			continue
		}
		if s.options.Locked && !isLocked {
			continue
		}
		matching = append(matching, candidate)
	}
	if hasLock {
		for _, candidate := range all {
			if candidate.Version == locked.Version && candidate.ArchiveSHA256 != locked.ArchiveSHA256 {
				return nil, &Error{Code: "lock_digest_mismatch", Package: key.Package}
			}
		}
		if s.options.Locked && !matchesLockedRequirements(locked, constraints) {
			return nil, &Error{Code: "locked_requirement_mismatch", Package: key.Package}
		}
	}
	if s.options.Locked && len(matching) == 0 {
		if s.options.Offline {
			return nil, &Error{Code: "offline_data_unavailable", Package: key.Package, Detail: "locked metadata is not locally verified"}
		}
		return nil, &Error{Code: "locked_requirement_mismatch", Package: key.Package, Detail: "locked release is unavailable"}
	}
	sort.SliceStable(matching, func(i, j int) bool {
		left, _ := semver.Parse(matching[i].Version)
		right, _ := semver.Parse(matching[j].Version)
		return semver.Compare(left, right) > 0
	})
	ordered := make([]orderedCandidate, 0, len(matching))
	if s.options.Strategy == Normal && hasLock {
		for index, candidate := range matching {
			if candidate.Version == locked.Version && candidate.ArchiveSHA256 == locked.ArchiveSHA256 {
				ordered = append(ordered, orderedCandidate{candidate: candidate, fromLock: true})
				matching = append(matching[:index], matching[index+1:]...)
				break
			}
		}
	}
	if len(ordered) == 0 && ambiguousTop(matching, constraints) {
		return nil, &Error{Code: "ambiguous_build_metadata", Package: key.Package}
	}
	for _, candidate := range matching {
		ordered = append(ordered, orderedCandidate{candidate: candidate})
	}
	return ordered, nil
}

func (s *solver) loadCandidates(key model.PackageKey) ([]model.Candidate, *Error) {
	if cached, ok := s.cache[key]; ok {
		return cached, nil
	}
	items, err := s.repository.Candidates(s.ctx, key, Access{Offline: s.options.Offline})
	if err != nil {
		if errors.Is(err, ErrOfflineDataUnavailable) || s.options.Offline {
			return nil, &Error{Code: "offline_data_unavailable", Package: key.Package}
		}
		return nil, &Error{Code: "no_matching_version", Package: key.Package, Detail: err.Error()}
	}
	seen := map[string]model.Candidate{}
	for _, candidate := range items {
		if candidate.RegistryOrigin != key.RegistryOrigin || candidate.Package != key.Package {
			return nil, &Error{Code: "registry_origin_mismatch", Package: key.Package, Detail: "repository returned metadata for another resolution key"}
		}
		if err := validateCandidate(candidate); err != nil {
			return nil, &Error{Code: "invalid_candidate_metadata", Package: key.Package, Detail: err.Error()}
		}
		identity := candidate.Version
		if prior, exists := seen[identity]; exists {
			if prior.ArchiveSHA256 != candidate.ArchiveSHA256 {
				return nil, &Error{Code: "metadata_equivocation", Package: key.Package, Detail: "one exact version has multiple digests"}
			}
			continue
		}
		seen[identity] = candidate
	}
	canonical := make([]model.Candidate, 0, len(seen))
	for _, candidate := range seen {
		canonical = append(canonical, candidate)
	}
	sort.Slice(canonical, func(i, j int) bool { return canonical[i].Version < canonical[j].Version })
	s.cache[key] = canonical
	return canonical, nil
}

func validateCandidate(candidate model.Candidate) error {
	if err := model.ValidateIdentity(candidate.Package); err != nil {
		return err
	}
	if err := model.ValidateRegistryOrigin(candidate.RegistryOrigin); err != nil {
		return err
	}
	if _, err := semver.Parse(candidate.Version); err != nil {
		return err
	}
	if err := model.ValidateSHA256(candidate.ArchiveSHA256); err != nil {
		return err
	}
	if candidate.Availability != model.Available && candidate.Availability != model.Yanked && candidate.Availability != model.SecurityQuarantined {
		return fmt.Errorf("invalid availability %q", candidate.Availability)
	}
	if err := model.ValidateCapabilities(candidate.Capabilities); err != nil {
		return err
	}
	expected, err := model.TargetPath(candidate.Package, candidate.Version, candidate.ArchiveSHA256)
	if err != nil {
		return err
	}
	if candidate.TargetPath != expected {
		return fmt.Errorf("target path is not bound to package, version, and digest")
	}
	if candidate.MetadataVersion == 0 {
		return fmt.Errorf("metadata version must be positive")
	}
	aliases := map[string]struct{}{}
	for _, edge := range candidate.Dependencies {
		if err := validateEdge(edge); err != nil {
			return err
		}
		if _, duplicate := aliases[edge.Alias]; duplicate {
			return fmt.Errorf("duplicate dependency alias %q", edge.Alias)
		}
		aliases[edge.Alias] = struct{}{}
	}
	return nil
}

func matchesAll(version string, constraints []constraint) bool {
	parsed, err := semver.Parse(version)
	if err != nil {
		return false
	}
	for _, item := range constraints {
		if !item.Requirement.Matches(parsed) {
			return false
		}
	}
	return true
}

func matchesLockedRequirements(locked model.LockedPackage, constraints []constraint) bool {
	return matchesAll(locked.Version, constraints)
}

func ambiguousTop(candidates []model.Candidate, constraints []constraint) bool {
	if len(candidates) < 2 {
		return false
	}
	// Exact requirements include build metadata byte-for-byte and cannot be ambiguous.
	for _, item := range constraints {
		if item.Requirement.Kind == semver.Exact {
			return false
		}
	}
	left, _ := semver.Parse(candidates[0].Version)
	right, _ := semver.Parse(candidates[1].Version)
	return semver.Compare(left, right) == 0 && candidates[0].Version != candidates[1].Version
}

func validateCapabilityConsent(root model.Root, resolved *state) *Error {
	for _, key := range sortedKeys(resolved.selected) {
		candidate := resolved.selected[key]
		constraints := sortedConstraints(resolved.constraints[key])
		for _, incoming := range constraints {
			if missing := model.MissingCapabilities(incoming.Edge.Allow, candidate.Capabilities); len(missing) != 0 {
				return &Error{Code: "dependency_capability_not_allowed", Package: candidate.Package, Capabilities: missing, Detail: "incoming edge " + incoming.Requester + " does not allow the signed request"}
			}
		}
		if missing := model.MissingCapabilities(root.Grants[candidate.Package], candidate.Capabilities); len(missing) != 0 {
			return &Error{Code: "capability_consent_required", Package: candidate.Package, Capabilities: missing}
		}
	}
	return nil
}

func (s *solver) buildResolution(resolved *state) *model.Resolution {
	result := &model.Resolution{Root: canonicalRoot(s.root), UsedLock: true}
	for index := range result.Root.Dependencies {
		bindEdge(&result.Root.Dependencies[index], resolved.selected[result.Root.Dependencies[index].Key()])
	}
	for _, key := range sortedKeys(resolved.selected) {
		candidate := resolved.selected[key]
		pkg := model.LockedPackage{
			Package: candidate.Package, Version: candidate.Version, Source: "hosted-registry",
			RegistryOrigin: candidate.RegistryOrigin, ArchiveSHA256: candidate.ArchiveSHA256,
			TargetPath: candidate.TargetPath, MetadataVersion: candidate.MetadataVersion,
			Capabilities: model.CanonicalCapabilities(candidate.Capabilities), Grants: model.CanonicalCapabilities(s.root.Grants[candidate.Package]),
			Dependencies: append([]model.Edge(nil), candidate.Dependencies...),
		}
		for index := range pkg.Dependencies {
			pkg.Dependencies[index].Allow = model.CanonicalCapabilities(pkg.Dependencies[index].Allow)
			bindEdge(&pkg.Dependencies[index], resolved.selected[pkg.Dependencies[index].Key()])
		}
		sort.Slice(pkg.Dependencies, func(i, j int) bool { return pkg.Dependencies[i].Alias < pkg.Dependencies[j].Alias })
		result.Packages = append(result.Packages, pkg)
		if !resolved.fromLock[key] {
			result.UsedLock = false
		}
	}
	return result
}

func bindEdge(edge *model.Edge, candidate model.Candidate) {
	edge.ResolvedVersion = candidate.Version
	edge.ResolvedArchiveSHA256 = candidate.ArchiveSHA256
}

func nextUnresolved(current *state) (model.PackageKey, bool) {
	keys := make([]model.PackageKey, 0)
	for key := range current.constraints {
		if _, selected := current.selected[key]; !selected {
			keys = append(keys, key)
		}
	}
	if len(keys) == 0 {
		return model.PackageKey{}, false
	}
	sort.Slice(keys, func(i, j int) bool {
		if keys[i].RegistryOrigin != keys[j].RegistryOrigin {
			return keys[i].RegistryOrigin < keys[j].RegistryOrigin
		}
		return keys[i].Package < keys[j].Package
	})
	return keys[0], true
}

func sortedKeys[V any](values map[model.PackageKey]V) []model.PackageKey {
	keys := make([]model.PackageKey, 0, len(values))
	for key := range values {
		keys = append(keys, key)
	}
	sort.Slice(keys, func(i, j int) bool {
		if keys[i].RegistryOrigin != keys[j].RegistryOrigin {
			return keys[i].RegistryOrigin < keys[j].RegistryOrigin
		}
		return keys[i].Package < keys[j].Package
	})
	return keys
}

func sortedConstraints(values []constraint) []constraint {
	result := append([]constraint(nil), values...)
	sort.Slice(result, func(i, j int) bool {
		if result[i].Requester != result[j].Requester {
			return result[i].Requester < result[j].Requester
		}
		return result[i].Requirement.Raw < result[j].Requirement.Raw
	})
	return result
}

func cloneState(input *state) *state {
	result := &state{constraints: make(map[model.PackageKey][]constraint, len(input.constraints)), selected: make(map[model.PackageKey]model.Candidate, len(input.selected)), fromLock: make(map[model.PackageKey]bool, len(input.fromLock))}
	for key, values := range input.constraints {
		result.constraints[key] = append([]constraint(nil), values...)
	}
	for key, value := range input.selected {
		result.selected[key] = value
	}
	for key, value := range input.fromLock {
		result.fromLock[key] = value
	}
	return result
}

func conflictError(packageName string, constraints []constraint) *Error {
	requesters := make([]string, 0, len(constraints))
	seen := map[string]struct{}{}
	for _, item := range sortedConstraints(constraints) {
		if item.Requester != "root" {
			if _, exists := seen[item.Requester]; !exists {
				seen[item.Requester] = struct{}{}
				requesters = append(requesters, item.Requester)
			}
		}
	}
	code := "dependency_constraint_conflict"
	if len(requesters) == 0 {
		code = "no_matching_version"
	}
	return &Error{Code: code, Package: packageName, ConflictRequesters: requesters}
}

func backtrackable(code string) bool {
	return code == "dependency_constraint_conflict" || code == "no_matching_version"
}
func chooseFailure(left, right *Error) *Error {
	if left == nil {
		return right
	}
	if len(right.ConflictRequesters) > len(left.ConflictRequesters) {
		return right
	}
	if len(right.ConflictRequesters) == len(left.ConflictRequesters) && right.Error() < left.Error() {
		return right
	}
	return left
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

// MemoryRepository is deterministic test/offline infrastructure. Production
// repositories must only return locally verified candidates when Access.Offline.
type MemoryRepository struct {
	Items          map[model.PackageKey][]model.Candidate
	OfflineMissing map[model.PackageKey]bool
}

func (repository MemoryRepository) Candidates(_ context.Context, key model.PackageKey, access Access) ([]model.Candidate, error) {
	if access.Offline && repository.OfflineMissing[key] {
		return nil, ErrOfflineDataUnavailable
	}
	return append([]model.Candidate(nil), repository.Items[key]...), nil
}
