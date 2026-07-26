package tuf

import (
	"bytes"
	"crypto/ed25519"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/url"
	"os"
	"reflect"
	"regexp"
	"strings"
	"sync"
	"time"

	"github.com/codeyousef/seen/tools/seen-pkg/internal/semver"
)

var (
	specPattern       = regexp.MustCompile(`^1\.0(?:\.[0-9]+)?$`)
	componentPattern  = regexp.MustCompile(`^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$`)
	semverPattern     = regexp.MustCompile(`^(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:-(?:0|[1-9][0-9]*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*)(?:\.(?:0|[1-9][0-9]*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*))*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$`)
	incidentPattern   = regexp.MustCompile(`^inc_[A-Za-z0-9_-]{8,96}$`)
	proofIDPattern    = regexp.MustCompile(`^prf_[A-Za-z0-9_-]{8,96}$`)
	scanIDPattern     = regexp.MustCompile(`^scn_[A-Za-z0-9_-]{8,96}$`)
	repositoryPattern = regexp.MustCompile(`^seen-(dev|prod)-[a-z0-9-]+$`)
	aliasPattern      = regexp.MustCompile(`^[A-Za-z_][A-Za-z0-9_]{0,63}$`)
	originPathPattern = regexp.MustCompile(`^[a-z0-9][a-z0-9._~-]*$`)
	principalPattern  = regexp.MustCompile(`^[A-Za-z0-9][A-Za-z0-9:@._/-]{0,255}$`)
	servicePattern    = regexp.MustCompile(`^[a-z0-9](?:[a-z0-9-]{0,126}[a-z0-9])?$`)
	sourceIDPattern   = regexp.MustCompile(`^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$`)
	sourceURLPattern  = regexp.MustCompile(`^https://[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?(?:/[A-Za-z0-9._~-]+)+$`)
	commitPattern     = regexp.MustCompile(`^[0-9a-f]+$`)
)

var capabilities = map[string]bool{
	"file": true, "network": true, "process": true, "environment": true,
	"dynamic-load": true, "ffi": true, "unsafe": true, "native-link": true, "macro": true,
}

type Config struct {
	Environment    string
	RepositoryID   string
	RegistryOrigin string
	Store          StateStore
	Now            func() time.Time
}

// Error is a stable signing/metadata verification failure.
type Error struct {
	Code string
	Err  error
}

func (e *Error) Error() string {
	if e.Err == nil {
		return e.Code
	}
	return e.Code + ": " + e.Err.Error()
}

func (e *Error) Unwrap() error { return e.Err }

func failure(code string, err error) error { return &Error{Code: code, Err: err} }

// Verifier serializes trust-state transitions so concurrent fetches cannot
// race rollback counters or commit a partially verified metadata chain.
type Verifier struct {
	mu     sync.Mutex
	config Config
	state  TrustedState
	root   RootSigned
	ready  bool
}

func New(config Config) (*Verifier, error) {
	if config.Environment != "development" && config.Environment != "production" {
		return nil, failure("signing_environment_mismatch", errors.New("development or production environment required"))
	}
	if !repositoryPattern.MatchString(config.RepositoryID) || (config.Environment == "production" && !strings.HasPrefix(config.RepositoryID, "seen-prod-")) || (config.Environment == "development" && !strings.HasPrefix(config.RepositoryID, "seen-dev-")) {
		return nil, failure("signing_repository_mismatch", errors.New("repository ID does not match environment"))
	}
	if err := validateOrigin(config.RegistryOrigin); err != nil {
		return nil, failure("signing_origin_mismatch", err)
	}
	if config.Store == nil {
		config.Store = &MemoryStore{}
	}
	if config.Now == nil {
		config.Now = time.Now
	}
	v := &Verifier{config: config}
	state, err := config.Store.Load()
	if errors.Is(err, os.ErrNotExist) {
		return v, nil
	}
	if err != nil {
		return nil, failure("signing_trusted_state_invalid", err)
	}
	envelope, root, _, err := parseRoot(state.Root)
	if err != nil {
		return nil, failure("signing_trusted_state_invalid", err)
	}
	if err := v.validateRoot(root, false); err != nil {
		return nil, failure("signing_trusted_state_invalid", err)
	}
	if err := verifyThreshold(envelope, root.Keys, root.Roles["root"], "root"); err != nil {
		return nil, failure("signing_trusted_state_invalid", err)
	}
	state = normalizeState(state)
	if saved := state.Versions["root"]; saved != 0 && saved != root.Version {
		return nil, failure("signing_trusted_state_invalid", errors.New("root version counter mismatch"))
	}
	v.state, v.root, v.ready = state, root, true
	return v, nil
}

func normalizeState(state TrustedState) TrustedState {
	state.Version = 1
	if state.Versions == nil {
		state.Versions = make(map[string]int64)
	}
	if state.Expires == nil {
		state.Expires = make(map[string]string)
	}
	if state.Fingerprints == nil {
		state.Fingerprints = make(map[string]string)
	}
	return state
}

func validateOrigin(origin string) error {
	u, err := url.Parse(origin)
	if err != nil || !u.IsAbs() || u.Scheme != "https" || u.Host == "" || u.User != nil || u.RawQuery != "" || u.Fragment != "" || u.Path == "" || u.Port() != "" || u.RawPath != "" {
		return errors.New("canonical absolute HTTPS registry origin required")
	}
	if u.String() != origin || strings.HasSuffix(origin, "/") || u.Host != u.Hostname() {
		return errors.New("registry origin is not byte-canonical")
	}
	labels := strings.Split(u.Hostname(), ".")
	if len(labels) < 2 {
		return errors.New("registry origin requires a canonical DNS hostname")
	}
	for _, label := range labels {
		if !componentPattern.MatchString(label) {
			return errors.New("registry origin contains a noncanonical DNS label")
		}
	}
	for _, segment := range strings.Split(strings.TrimPrefix(u.Path, "/"), "/") {
		if segment == "" || segment == "." || segment == ".." || !originPathPattern.MatchString(segment) {
			return errors.New("registry origin contains a noncanonical path segment")
		}
	}
	return nil
}

// BootstrapRoot accepts only a locally provisioned, self-threshold-signed root
// whose complete canonical envelope matches an out-of-band SHA-256 pin.
func (v *Verifier) BootstrapRoot(raw []byte, pinnedSHA256 string) error {
	v.mu.Lock()
	defer v.mu.Unlock()
	if v.ready {
		return failure("signing_root_already_trusted", errors.New("trusted root already exists"))
	}
	envelope, root, canonical, err := parseRoot(raw)
	if err != nil {
		return failure("signing_metadata_invalid", err)
	}
	if err := v.validateRoot(root, true); err != nil {
		return err
	}
	if !validDigest(pinnedSHA256) || digest(canonical) != pinnedSHA256 {
		return failure("signing_root_pin_mismatch", errors.New("root envelope does not match out-of-band SHA-256 pin"))
	}
	if err := verifyThreshold(envelope, root.Keys, root.Roles["root"], "root"); err != nil {
		return err
	}
	fingerprint := digest(canonical)
	state := normalizeState(TrustedState{
		Root:         append(json.RawMessage(nil), canonical...),
		Versions:     map[string]int64{"root": root.Version},
		Expires:      map[string]string{"root": root.Expires},
		Fingerprints: map[string]string{"root": fingerprint},
	})
	if err := v.config.Store.Save(state); err != nil {
		return failure("signing_trusted_state_write_failed", err)
	}
	v.state, v.root, v.ready = state, root, true
	return nil
}

// UpdateRoot enforces sequential rotation and both old-root and new-root
// thresholds before replacing trusted root state.
func (v *Verifier) UpdateRoot(raw []byte) error {
	v.mu.Lock()
	defer v.mu.Unlock()
	if !v.ready {
		return failure("signing_no_trusted_root", errors.New("bootstrap root first"))
	}
	envelope, next, canonical, err := parseRoot(raw)
	if err != nil {
		return failure("signing_metadata_invalid", err)
	}
	if next.Version <= v.root.Version {
		return failure("signing_root_rollback", fmt.Errorf("trusted=%d received=%d", v.root.Version, next.Version))
	}
	if next.Version != v.root.Version+1 {
		return failure("signing_nonsequential_root_version", fmt.Errorf("trusted=%d received=%d", v.root.Version, next.Version))
	}
	if err := v.validateRoot(next, true); err != nil {
		return err
	}
	if err := verifyThreshold(envelope, v.root.Keys, v.root.Roles["root"], "old-root"); err != nil {
		return remapThreshold(err, "signing_old_root_threshold_not_met")
	}
	if err := verifyThreshold(envelope, next.Keys, next.Roles["root"], "new-root"); err != nil {
		return remapThreshold(err, "signing_new_root_threshold_not_met")
	}
	state := cloneState(v.state)
	state.Root = append(json.RawMessage(nil), canonical...)
	state.Versions["root"] = next.Version
	state.Expires["root"] = next.Expires
	state.Fingerprints["root"] = digest(canonical)
	if err := v.config.Store.Save(state); err != nil {
		return failure("signing_trusted_state_write_failed", err)
	}
	v.state, v.root = state, next
	return nil
}

func remapThreshold(err error, code string) error {
	var verification *Error
	if errors.As(err, &verification) && (verification.Code == "signing_threshold_not_met" || verification.Code == "signing_unknown_key" || verification.Code == "signing_wrong_role" || verification.Code == "signing_signature_invalid") {
		return failure(code, verification)
	}
	return err
}

func (v *Verifier) validateRoot(root RootSigned, checkExpiry bool) error {
	if err := v.validateCommon(root.Common, "root", checkExpiry); err != nil {
		return err
	}
	if !root.ConsistentSnapshot {
		return failure("signing_metadata_invalid", errors.New("consistent snapshots are required"))
	}
	if len(root.Roles) != 4 || len(root.Keys) == 0 {
		return failure("signing_metadata_invalid", errors.New("root roles or keys are incomplete"))
	}
	for _, name := range []string{"root", "targets", "snapshot", "timestamp"} {
		role, ok := root.Roles[name]
		if !ok {
			return failure("signing_metadata_invalid", fmt.Errorf("missing %s role", name))
		}
		if err := validateRole(role, root.Keys); err != nil {
			return failure("signing_metadata_invalid", fmt.Errorf("%s role: %w", name, err))
		}
	}
	for id, key := range root.Keys {
		if err := validateKey(key); err != nil {
			return failure("signing_metadata_invalid", fmt.Errorf("key %s: %w", id, err))
		}
		derived, err := deriveKeyID(key)
		if err != nil || id != derived {
			return failure("signing_keyid_mismatch", fmt.Errorf("key ID %q does not match canonical key object", id))
		}
	}
	return nil
}

func validateKey(key Key) error {
	if key.KeyType != "ed25519" || key.Scheme != "ed25519" {
		return errors.New("Seen TUF v1 requires Ed25519")
	}
	public, err := hex.DecodeString(key.KeyVal.Public)
	if err != nil || len(key.KeyVal.Public) != 64 || len(public) != ed25519.PublicKeySize || strings.ToLower(key.KeyVal.Public) != key.KeyVal.Public {
		return errors.New("public key must be 32 raw Ed25519 bytes in lowercase hex")
	}
	return nil
}

func deriveKeyID(key Key) (string, error) {
	if err := validateKey(key); err != nil {
		return "", err
	}
	raw, err := json.Marshal(key)
	if err != nil {
		return "", err
	}
	canonical, err := CanonicalJSON(raw)
	if err != nil {
		return "", err
	}
	return digest(canonical), nil
}

func validateRole(role Role, keys map[string]Key) error {
	if role.Threshold < 1 || role.Threshold > len(role.KeyIDs) || len(role.KeyIDs) == 0 {
		return errors.New("invalid threshold")
	}
	seen := make(map[string]bool)
	for _, id := range role.KeyIDs {
		if !validDigest(id) {
			return errors.New("role key ID must be a lowercase SHA-256 digest")
		}
		if seen[id] {
			return errors.New("duplicate role key ID")
		}
		seen[id] = true
		if _, ok := keys[id]; !ok {
			return fmt.Errorf("role references unknown key %q", id)
		}
	}
	return nil
}

func (v *Verifier) validateCommon(common Common, expectedType string, checkExpiry bool) error {
	if common.Type != expectedType || !specPattern.MatchString(common.SpecVersion) || common.Version < 1 {
		return failure("signing_metadata_invalid", fmt.Errorf("invalid %s type, spec, or version", expectedType))
	}
	if common.Environment != v.config.Environment {
		return failure("signing_environment_mismatch", fmt.Errorf("configured=%s metadata=%s", v.config.Environment, common.Environment))
	}
	if common.RepositoryID != v.config.RepositoryID {
		return failure("signing_repository_mismatch", fmt.Errorf("configured=%s metadata=%s", v.config.RepositoryID, common.RepositoryID))
	}
	expires, err := time.Parse(time.RFC3339, common.Expires)
	if err != nil || !strings.HasSuffix(common.Expires, "Z") {
		return failure("signing_metadata_invalid", errors.New("canonical UTC expiry required"))
	}
	if checkExpiry && !v.config.Now().UTC().Before(expires) {
		return failure("signing_metadata_expired", fmt.Errorf("%s expired at %s", expectedType, common.Expires))
	}
	return nil
}

// Refresh verifies a newly fetched chain. A byte-identical timestamp may be
// reused while it still has a safe freshness margin; once it approaches expiry,
// an unchanged network response is a freeze signal. VerifyCached is the
// explicit offline/idempotent alternative.
func (v *Verifier) Refresh(set MetadataSet) (*Repository, error) {
	return v.verifySet(set, false)
}

func (v *Verifier) VerifyCached(set MetadataSet) (*Repository, error) {
	return v.verifySet(set, true)
}

func (v *Verifier) verifySet(set MetadataSet, cached bool) (*Repository, error) {
	v.mu.Lock()
	defer v.mu.Unlock()
	if !v.ready {
		return nil, failure("signing_no_trusted_root", errors.New("bootstrap root first"))
	}
	if err := v.validateCommon(v.root.Common, "root", true); err != nil {
		return nil, err
	}

	timestampEnv, timestamp, timestampCanonical, err := parseTimestamp(set.Timestamp)
	if err != nil {
		return nil, failure("signing_metadata_invalid", err)
	}
	if err := v.validateCommon(timestamp.Common, "timestamp", true); err != nil {
		return nil, err
	}
	if err := verifyThreshold(timestampEnv, v.root.Keys, v.root.Roles["timestamp"], "timestamp"); err != nil {
		return nil, err
	}
	if err := v.checkVersion("timestamp", timestamp.Common, timestampCanonical, cached); err != nil {
		return nil, err
	}
	if len(timestamp.Meta) != 1 {
		return nil, failure("signing_metadata_invalid", errors.New("timestamp must bind only snapshot.json"))
	}
	snapshotMeta, ok := timestamp.Meta["snapshot.json"]
	if !ok {
		return nil, failure("signing_metadata_invalid", errors.New("timestamp lacks snapshot.json"))
	}
	if err := verifyFileBinding(snapshotMeta, set.Snapshot); err != nil {
		return nil, err
	}

	snapshotEnv, snapshot, snapshotCanonical, err := parseSnapshot(set.Snapshot)
	if err != nil {
		return nil, failure("signing_metadata_invalid", err)
	}
	if snapshot.Version != snapshotMeta.Version {
		return nil, failure("signing_metadata_version_mismatch", errors.New("snapshot version does not match timestamp"))
	}
	if err := v.validateCommon(snapshot.Common, "snapshot", true); err != nil {
		return nil, err
	}
	if err := verifyThreshold(snapshotEnv, v.root.Keys, v.root.Roles["snapshot"], "snapshot"); err != nil {
		return nil, err
	}
	if err := v.checkVersion("snapshot", snapshot.Common, snapshotCanonical, true); err != nil {
		return nil, err
	}
	if len(snapshot.Meta) != 3 {
		return nil, failure("signing_metadata_invalid", errors.New("snapshot must bind targets, releases, and security"))
	}
	bound := []struct {
		name string
		raw  []byte
	}{
		{"targets.json", set.Targets}, {"releases.json", set.Releases}, {"security.json", set.Security},
	}
	for _, document := range bound {
		meta, ok := snapshot.Meta[document.name]
		if !ok {
			return nil, failure("signing_metadata_invalid", fmt.Errorf("snapshot lacks %s", document.name))
		}
		if err := verifyFileBinding(meta, document.raw); err != nil {
			return nil, err
		}
	}

	targetsEnv, targets, targetsCanonical, err := parseTargets(set.Targets)
	if err != nil {
		return nil, failure("signing_metadata_invalid", err)
	}
	if targets.Version != snapshot.Meta["targets.json"].Version {
		return nil, failure("signing_metadata_version_mismatch", errors.New("targets version does not match snapshot"))
	}
	if err := v.validateCommon(targets.Common, "targets", true); err != nil {
		return nil, err
	}
	if err := verifyThreshold(targetsEnv, v.root.Keys, v.root.Roles["targets"], "targets"); err != nil {
		return nil, err
	}
	if err := v.checkVersion("targets", targets.Common, targetsCanonical, true); err != nil {
		return nil, err
	}
	if len(targets.Targets) != 0 || targets.Delegations == nil {
		return nil, failure("signing_metadata_invalid", errors.New("top-level targets must delegate all package targets"))
	}
	roles, err := validateDelegations(*targets.Delegations)
	if err != nil {
		return nil, failure("signing_metadata_invalid", err)
	}

	releasesEnv, releases, releasesCanonical, err := parseTargets(set.Releases)
	if err != nil {
		return nil, failure("signing_metadata_invalid", err)
	}
	securityEnv, security, securityCanonical, err := parseTargets(set.Security)
	if err != nil {
		return nil, failure("signing_metadata_invalid", err)
	}
	delegated := []struct {
		name      string
		envelope  Envelope
		signed    TargetsSigned
		canonical []byte
	}{
		{"releases", releasesEnv, releases, releasesCanonical},
		{"security", securityEnv, security, securityCanonical},
	}
	for _, role := range delegated {
		if role.signed.Version != snapshot.Meta[role.name+".json"].Version {
			return nil, failure("signing_metadata_version_mismatch", fmt.Errorf("%s version does not match snapshot", role.name))
		}
		if err := v.validateCommon(role.signed.Common, "targets", true); err != nil {
			return nil, err
		}
		delegatedRole := roles[role.name]
		if err := verifyThreshold(role.envelope, targets.Delegations.Keys, Role{KeyIDs: delegatedRole.KeyIDs, Threshold: delegatedRole.Threshold}, role.name); err != nil {
			return nil, err
		}
		if err := v.checkVersion(role.name, role.signed.Common, role.canonical, true); err != nil {
			return nil, err
		}
		for targetPath, target := range role.signed.Targets {
			if err := v.validateTarget(role.name, targetPath, target); err != nil {
				return nil, err
			}
		}
	}
	if err := validateOverlays(releases.Targets, security.Targets); err != nil {
		return nil, err
	}

	state := cloneState(v.state)
	updates := []struct {
		name      string
		common    Common
		canonical []byte
	}{
		{"timestamp", timestamp.Common, timestampCanonical},
		{"snapshot", snapshot.Common, snapshotCanonical},
		{"targets", targets.Common, targetsCanonical},
		{"releases", releases.Common, releasesCanonical},
		{"security", security.Common, securityCanonical},
	}
	for _, update := range updates {
		state.Versions[update.name] = update.common.Version
		state.Expires[update.name] = update.common.Expires
		state.Fingerprints[update.name] = digest(update.canonical)
	}
	if err := v.config.Store.Save(state); err != nil {
		return nil, failure("signing_trusted_state_write_failed", err)
	}
	v.state = state
	return &Repository{Timestamp: timestamp, Snapshot: snapshot, Targets: targets, Releases: releases, Security: security}, nil
}

func (v *Verifier) checkVersion(role string, common Common, canonical []byte, allowEqual bool) error {
	previous := v.state.Versions[role]
	if previous == 0 {
		return nil
	}
	if common.Version < previous {
		return failure("signing_metadata_rollback", fmt.Errorf("%s trusted=%d received=%d", role, previous, common.Version))
	}
	fingerprint := digest(canonical)
	if common.Version == previous {
		if v.state.Fingerprints[role] == "" || v.state.Fingerprints[role] != fingerprint {
			return failure("signing_same_version_changed", fmt.Errorf("%s bytes changed without version increment", role))
		}
		if role == "timestamp" && !allowEqual {
			expires, err := time.Parse(time.RFC3339, common.Expires)
			if err != nil || !v.config.Now().UTC().Add(minimumNetworkTimestampMargin).Before(expires) {
				return failure("signing_freeze_detected", errors.New("refreshed timestamp did not advance before its freshness margin"))
			}
		}
	}
	return nil
}

const minimumNetworkTimestampMargin = 5 * time.Minute

func verifyFileBinding(meta FileMeta, raw []byte) error {
	if meta.Version < 1 || meta.Length < 1 || len(meta.Hashes) != 1 || !validDigest(meta.Hashes["sha256"]) {
		return failure("signing_metadata_invalid", errors.New("invalid metadata file binding"))
	}
	canonical, err := CanonicalJSON(raw)
	if err != nil {
		return failure("signing_metadata_invalid", err)
	}
	if int64(len(canonical)) != meta.Length {
		return failure("signing_metadata_length_mismatch", fmt.Errorf("expected=%d actual=%d", meta.Length, len(canonical)))
	}
	if digest(canonical) != meta.Hashes["sha256"] {
		return failure("signing_metadata_hash_mismatch", errors.New("metadata SHA-256 mismatch"))
	}
	return nil
}

func validateDelegations(delegations Delegations) (map[string]DelegatedRole, error) {
	if len(delegations.Roles) != 2 || len(delegations.Keys) < 2 {
		return nil, errors.New("security and releases delegations are required")
	}
	if delegations.Roles[0].Name != "security" || delegations.Roles[1].Name != "releases" {
		return nil, errors.New("delegation order must be security then releases")
	}
	result := make(map[string]DelegatedRole)
	for _, role := range delegations.Roles {
		if role.Terminating || len(role.Paths) != 1 || role.Paths[0] != "packages/*/*/*/*/*" {
			return nil, fmt.Errorf("invalid %s delegated path policy", role.Name)
		}
		baseRole := Role{KeyIDs: role.KeyIDs, Threshold: role.Threshold}
		if err := validateRole(baseRole, delegations.Keys); err != nil {
			return nil, err
		}
		result[role.Name] = role
	}
	for id, key := range delegations.Keys {
		if err := validateKey(key); err != nil {
			return nil, fmt.Errorf("delegated key %s: %w", id, err)
		}
		derived, err := deriveKeyID(key)
		if err != nil || id != derived {
			return nil, fmt.Errorf("delegated key ID %q does not match canonical key object", id)
		}
	}
	return result, nil
}

func (v *Verifier) validateTarget(role, targetPath string, target TargetMeta) error {
	parts := strings.Split(targetPath, "/")
	if len(parts) != 6 || parts[0] != "packages" {
		return failure("signing_path_not_delegated", errors.New("target is outside package delegation"))
	}
	owner, name, version, archiveDigest, leaf := parts[1], parts[2], parts[3], parts[4], parts[5]
	if !componentPattern.MatchString(owner) || !componentPattern.MatchString(name) || !semverPattern.MatchString(version) || len(version) > 128 || !validDigest(archiveDigest) {
		return failure("signing_target_path_invalid", errors.New("target path is not canonical"))
	}
	custom := target.Custom
	if custom.Package != owner+"/"+name {
		return failure("signing_target_path_identity_mismatch", errors.New("path and custom package differ"))
	}
	if custom.Owner != owner || custom.Name != name {
		return failure("signing_target_path_identity_mismatch", errors.New("path and attested owner/name differ"))
	}
	if custom.Version != version {
		return failure("signing_target_path_version_mismatch", errors.New("path and custom version differ"))
	}
	if custom.ArchiveSHA256 != archiveDigest {
		return failure("signing_target_path_digest_mismatch", errors.New("path and custom digest differ"))
	}
	wantLeaf := name + "-" + version + ".seenpkg.tgz"
	if custom.ArchiveFilename != leaf || leaf != wantLeaf {
		return failure("signing_target_path_leaf_mismatch", errors.New("path and custom archive filename differ"))
	}
	if target.Length < 1 || target.Length > 25*1024*1024 || len(target.Hashes) != 1 || target.Hashes["sha256"] != archiveDigest {
		return failure("signing_target_hash_mismatch", errors.New("target length or digest binding is invalid"))
	}
	if custom.Blob.SHA256 != archiveDigest || custom.Blob.Length != target.Length {
		return failure("signing_target_attestation_invalid", errors.New("attested blob does not match target bytes"))
	}
	if custom.Environment != v.config.Environment {
		return failure("signing_environment_mismatch", errors.New("target environment mismatch"))
	}
	if custom.RegistryOrigin != v.config.RegistryOrigin {
		return failure("signing_origin_mismatch", errors.New("target registry origin mismatch"))
	}
	if custom.Visibility != "public" {
		return failure("signing_release_not_public", errors.New("public metadata contains nonpublic release"))
	}
	if custom.Lifecycle != "active" {
		return failure("signing_release_not_active", errors.New("public metadata contains nonactive release"))
	}
	if custom.Retention != "retained" {
		return failure("signing_release_not_retained", errors.New("public metadata contains nonretained release"))
	}
	if err := v.validateRegistryAttestation(custom); err != nil {
		return err
	}
	if custom.Dependencies == nil || custom.Capabilities == nil {
		return failure("signing_target_graph_invalid", errors.New("signed dependencies and capabilities arrays are required"))
	}
	if err := validateTargetDependencies(custom.Dependencies); err != nil {
		return failure("signing_target_graph_invalid", err)
	}
	if err := validateCapabilityList(custom.Capabilities, "capabilities"); err != nil {
		return failure("signing_target_graph_invalid", err)
	}
	switch role {
	case "releases":
		if custom.Availability != "available" && custom.Availability != "yanked" {
			return failure("signing_wrong_role", errors.New("release role cannot publish security availability"))
		}
		if custom.Availability == "yanked" && custom.YankReason == "" {
			return failure("signing_metadata_invalid", errors.New("yanked target lacks reason"))
		}
		if custom.Availability != "yanked" && custom.YankReason != "" {
			return failure("signing_metadata_invalid", errors.New("non-yanked target has yank reason"))
		}
		if custom.IncidentID != "" || custom.SecurityAction != "" {
			return failure("signing_wrong_role", errors.New("release role contains security action"))
		}
	case "security":
		if custom.YankReason != "" {
			return failure("signing_metadata_invalid", errors.New("security target has yank reason"))
		}
		if !incidentPattern.MatchString(custom.IncidentID) {
			return failure("signing_security_incident_required", errors.New("security target lacks incident ID"))
		}
		if custom.Availability == "security-quarantined" && custom.SecurityAction == "quarantine" {
			return nil
		}
		if custom.Availability == "available" && custom.SecurityAction == "reinstate-reviewed" {
			return nil
		}
		return failure("signing_wrong_role", errors.New("invalid security action or availability"))
	default:
		return failure("signing_wrong_role", errors.New("unknown target role"))
	}
	return nil
}

type registryAttestationSubject struct {
	Package    string       `json:"package"`
	Owner      string       `json:"owner"`
	Name       string       `json:"name"`
	Version    string       `json:"version"`
	Blob       AttestedBlob `json:"blob"`
	Visibility string       `json:"visibility"`
}

type registryAttestationProjection struct {
	Subject                 registryAttestationSubject `json:"subject"`
	PublisherPrincipal      string                     `json:"publisher_principal"`
	RegistryServiceIdentity string                     `json:"registry_service_identity"`
	SourceRepository        AttestedRepository         `json:"source_repository"`
	SourceCommit            AttestedCommit             `json:"source_commit"`
	Review                  AttestedReview             `json:"review"`
	ActivatedAt             string                     `json:"activated_at"`
}

func registryAttestationDigest(custom TargetCustom) (string, error) {
	projection := registryAttestationProjection{
		Subject: registryAttestationSubject{
			Package: custom.Package, Owner: custom.Owner, Name: custom.Name,
			Version: custom.Version, Blob: custom.Blob, Visibility: custom.Visibility,
		},
		PublisherPrincipal: custom.PublisherPrincipal, RegistryServiceIdentity: custom.RegistryServiceIdentity,
		SourceRepository: custom.SourceRepository, SourceCommit: custom.SourceCommit,
		Review: custom.Review, ActivatedAt: custom.ActivatedAt,
	}
	raw, err := json.Marshal(projection)
	if err != nil {
		return "", err
	}
	canonical, err := CanonicalJSON(raw)
	if err != nil {
		return "", err
	}
	return digest(canonical), nil
}

func (v *Verifier) validateRegistryAttestation(custom TargetCustom) error {
	if !principalPattern.MatchString(custom.PublisherPrincipal) || !servicePattern.MatchString(custom.RegistryServiceIdentity) {
		return failure("signing_target_attestation_invalid", errors.New("publisher or registry service identity is invalid"))
	}
	repository := custom.SourceRepository
	if repository.Forge != "github" && repository.Forge != "gitlab" {
		return failure("signing_target_attestation_invalid", errors.New("source forge is invalid"))
	}
	if !sourceIDPattern.MatchString(repository.RepositoryID) || validateSourceRepositoryURL(repository.CanonicalURL) != nil {
		return failure("signing_target_attestation_invalid", errors.New("source repository identity is invalid"))
	}
	commitLength := 0
	switch custom.SourceCommit.Algorithm {
	case "sha1":
		commitLength = 40
	case "sha256":
		commitLength = 64
	default:
		return failure("signing_target_attestation_invalid", errors.New("source commit algorithm is invalid"))
	}
	if len(custom.SourceCommit.Value) != commitLength || !commitPattern.MatchString(custom.SourceCommit.Value) {
		return failure("signing_target_attestation_invalid", errors.New("source commit digest is invalid"))
	}
	review := custom.Review
	if review.Result != "passed" || review.PolicyVersion == "" || len(review.PolicyVersion) > 128 ||
		!proofIDPattern.MatchString(review.SourceProofID) || !validDigest(review.SourceProofSHA256) ||
		!scanIDPattern.MatchString(review.ScanAttestationID) || !validDigest(review.ScanAttestationSHA256) ||
		review.ScannerID == "" || len(review.ScannerID) > 128 || review.ScannerVersion == "" || len(review.ScannerVersion) > 128 ||
		review.AttestationSequence < 1 {
		return failure("signing_target_attestation_invalid", errors.New("review attestation is incomplete"))
	}
	if custom.SourceProofSHA256 != review.SourceProofSHA256 {
		return failure("signing_target_attestation_invalid", errors.New("source proof digest does not match review"))
	}
	activated, err := time.Parse(time.RFC3339, custom.ActivatedAt)
	if err != nil || !strings.HasSuffix(custom.ActivatedAt, "Z") || activated.After(v.config.Now().UTC().Add(5*time.Minute)) {
		return failure("signing_target_attestation_invalid", errors.New("activation time is invalid"))
	}
	attestationDigest, err := registryAttestationDigest(custom)
	if err != nil || !validDigest(custom.RegistryAttestationSHA256) ||
		custom.RegistryAttestationSHA256 != attestationDigest || custom.ProvenanceSHA256 != attestationDigest {
		return failure("signing_target_attestation_invalid", errors.New("registry attestation digest does not bind canonical provenance"))
	}
	return nil
}

func validateSourceRepositoryURL(value string) error {
	if !sourceURLPattern.MatchString(value) {
		return errors.New("canonical HTTPS source repository URL required")
	}
	u, err := url.Parse(value)
	if err != nil || !u.IsAbs() || u.Scheme != "https" || u.Host == "" || u.User != nil || u.RawQuery != "" || u.Fragment != "" || u.Port() != "" || u.Path == "" || u.Path == "/" {
		return errors.New("canonical HTTPS source repository URL required")
	}
	if u.String() != value || strings.HasSuffix(value, "/") || u.Host != u.Hostname() {
		return errors.New("source repository URL is not byte-canonical")
	}
	return nil
}

func validateOverlays(releases, security map[string]TargetMeta) error {
	for targetPath, overlay := range security {
		base, ok := releases[targetPath]
		if !ok {
			return failure("signing_security_base_missing", fmt.Errorf("security overlay %q lacks release target", targetPath))
		}
		if base.Hashes["sha256"] != overlay.Hashes["sha256"] || base.Custom.ArchiveSHA256 != overlay.Custom.ArchiveSHA256 || base.Length != overlay.Length {
			return failure("signing_overlay_digest_mismatch", fmt.Errorf("security overlay %q changes immutable bytes", targetPath))
		}
		baseCustom, overlayCustom := base.Custom, overlay.Custom
		baseCustom.Availability, baseCustom.YankReason = "", ""
		overlayCustom.Availability, overlayCustom.YankReason = "", ""
		baseCustom.IncidentID, baseCustom.SecurityAction = "", ""
		overlayCustom.IncidentID, overlayCustom.SecurityAction = "", ""
		if !reflect.DeepEqual(baseCustom, overlayCustom) {
			return failure("signing_overlay_custom_mismatch", fmt.Errorf("security overlay %q changes immutable custom metadata", targetPath))
		}
	}
	return nil
}

func validateTargetDependencies(dependencies []TargetDependency) error {
	aliases := make(map[string]bool)
	for _, dependency := range dependencies {
		if !aliasPattern.MatchString(dependency.Alias) || aliases[dependency.Alias] {
			return fmt.Errorf("dependency alias %q is invalid or duplicated", dependency.Alias)
		}
		aliases[dependency.Alias] = true
		parts := strings.Split(dependency.Package, "/")
		if len(parts) != 2 || !componentPattern.MatchString(parts[0]) || !componentPattern.MatchString(parts[1]) {
			return fmt.Errorf("dependency package %q is not canonical", dependency.Package)
		}
		if err := validateOrigin(dependency.RegistryOrigin); err != nil {
			return fmt.Errorf("dependency origin %q: %w", dependency.RegistryOrigin, err)
		}
		if _, err := semver.ParseRequirement(dependency.Requirement); err != nil {
			return fmt.Errorf("dependency requirement %q: %w", dependency.Requirement, err)
		}
		if dependency.Allow == nil {
			return fmt.Errorf("dependency %q lacks an explicit allow array", dependency.Alias)
		}
		if err := validateCapabilityList(dependency.Allow, "allow"); err != nil {
			return fmt.Errorf("dependency %q: %w", dependency.Alias, err)
		}
	}
	return nil
}

func validateCapabilityList(values []string, field string) error {
	seen := make(map[string]bool)
	for _, value := range values {
		if !capabilities[value] || seen[value] {
			return fmt.Errorf("%s contains invalid or duplicate capability %q", field, value)
		}
		seen[value] = true
	}
	return nil
}

// Select gives security metadata precedence for the identical target key.
func (r *Repository) Select(targetPath string) (Selection, error) {
	if target, ok := r.Security.Targets[targetPath]; ok {
		return Selection{Role: "security", Path: targetPath, Target: target}, nil
	}
	if target, ok := r.Releases.Targets[targetPath]; ok {
		return Selection{Role: "releases", Path: targetPath, Target: target}, nil
	}
	return Selection{}, failure("signing_target_not_found", errors.New("target absent from trusted metadata"))
}

func parseRoot(raw []byte) (Envelope, RootSigned, []byte, error) {
	var signed RootSigned
	envelope, canonical, err := parseEnvelope(raw, &signed)
	return envelope, signed, canonical, err
}

func parseTimestamp(raw []byte) (Envelope, TimestampSigned, []byte, error) {
	var signed TimestampSigned
	envelope, canonical, err := parseEnvelope(raw, &signed)
	return envelope, signed, canonical, err
}

func parseSnapshot(raw []byte) (Envelope, SnapshotSigned, []byte, error) {
	var signed SnapshotSigned
	envelope, canonical, err := parseEnvelope(raw, &signed)
	return envelope, signed, canonical, err
}

func parseTargets(raw []byte) (Envelope, TargetsSigned, []byte, error) {
	var signed TargetsSigned
	envelope, canonical, err := parseEnvelope(raw, &signed)
	return envelope, signed, canonical, err
}

func parseEnvelope(raw []byte, signed any) (Envelope, []byte, error) {
	canonical, err := CanonicalJSON(raw)
	if err != nil {
		return Envelope{}, nil, err
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	var envelope Envelope
	if err := decoder.Decode(&envelope); err != nil {
		return Envelope{}, nil, err
	}
	if len(envelope.Signatures) == 0 || len(envelope.Signed) == 0 {
		return Envelope{}, nil, errors.New("metadata envelope is incomplete")
	}
	if err := decodeStrict(envelope.Signed, signed); err != nil {
		return Envelope{}, nil, err
	}
	return envelope, canonical, nil
}

func decodeStrict(raw []byte, value any) error {
	if _, err := CanonicalJSON(raw); err != nil {
		return err
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(value); err != nil {
		return err
	}
	var extra any
	if err := decoder.Decode(&extra); !errors.Is(err, io.EOF) {
		return errors.New("trailing JSON value")
	}
	return nil
}

func verifyThreshold(envelope Envelope, keys map[string]Key, role Role, roleName string) error {
	if err := validateRole(role, keys); err != nil {
		return failure("signing_metadata_invalid", err)
	}
	canonicalSigned, err := CanonicalJSON(envelope.Signed)
	if err != nil {
		return failure("signing_metadata_invalid", err)
	}
	authorized := make(map[string]bool)
	for _, id := range role.KeyIDs {
		authorized[id] = true
	}
	valid := make(map[string]bool)
	seenSignatures := make(map[string]bool)
	unknown, wrongRole, invalid := false, false, false
	for _, signature := range envelope.Signatures {
		if !validDigest(signature.KeyID) || len(signature.Sig) != ed25519.SignatureSize*2 || strings.ToLower(signature.Sig) != signature.Sig || seenSignatures[signature.KeyID] {
			invalid = true
			continue
		}
		seenSignatures[signature.KeyID] = true
		key, exists := keys[signature.KeyID]
		if !exists {
			unknown = true
			continue
		}
		if !authorized[signature.KeyID] {
			wrongRole = true
			continue
		}
		if err := verifySignature(key, canonicalSigned, signature.Sig); err != nil {
			invalid = true
			continue
		}
		valid[signature.KeyID] = true
	}
	if len(valid) >= role.Threshold {
		return nil
	}
	if invalid {
		return failure("signing_signature_invalid", fmt.Errorf("%s valid signatures %d/%d", roleName, len(valid), role.Threshold))
	}
	if wrongRole {
		return failure("signing_wrong_role", fmt.Errorf("%s signature used a key from another role", roleName))
	}
	if unknown {
		return failure("signing_unknown_key", fmt.Errorf("%s signature used an unknown key", roleName))
	}
	return failure("signing_threshold_not_met", fmt.Errorf("%s valid signatures %d/%d", roleName, len(valid), role.Threshold))
}

func verifySignature(key Key, message []byte, encodedSignature string) error {
	if err := validateKey(key); err != nil {
		return err
	}
	if len(encodedSignature) != ed25519.SignatureSize*2 || strings.ToLower(encodedSignature) != encodedSignature {
		return errors.New("signature must be 64 raw Ed25519 bytes in lowercase hex")
	}
	signature, err := hex.DecodeString(encodedSignature)
	if err != nil {
		return err
	}
	publicBytes, err := hex.DecodeString(key.KeyVal.Public)
	if err != nil {
		return err
	}
	if !ed25519.Verify(ed25519.PublicKey(publicBytes), message, signature) {
		return errors.New("invalid Ed25519 signature")
	}
	return nil
}

func digest(data []byte) string {
	h := sha256.Sum256(data)
	return hex.EncodeToString(h[:])
}

func validDigest(value string) bool {
	if len(value) != sha256.Size*2 || strings.ToLower(value) != value {
		return false
	}
	_, err := hex.DecodeString(value)
	return err == nil
}
