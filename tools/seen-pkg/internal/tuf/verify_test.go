package tuf

import (
	"crypto/ed25519"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

const (
	testEnvironment = "production"
	testRepository  = "seen-prod-registry-v1"
	testOrigin      = "https://seen.yousef.codes/packages"
	testTargetPath  = "packages/alice/mathx/1.2.3/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/mathx-1.2.3.seenpkg.tgz"
)

type signingKey struct {
	public  ed25519.PublicKey
	private ed25519.PrivateKey
	id      string
	wire    Key
}

type fixture struct {
	now       time.Time
	keys      map[string]signingKey
	root      []byte
	metadata  MetadataSet
	releases  TargetsSigned
	security  TargetsSigned
	targets   TargetsSigned
	snapshot  SnapshotSigned
	timestamp TimestampSigned
}

func newSigningKey(t *testing.T) signingKey {
	t.Helper()
	public, private, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	var wire Key
	wire.KeyType = "ed25519"
	wire.Scheme = "ed25519"
	wire.KeyVal.Public = hex.EncodeToString(public)
	id, err := deriveKeyID(wire)
	if err != nil {
		t.Fatal(err)
	}
	return signingKey{public: public, private: private, id: id, wire: wire}
}

func tufKey(key signingKey) Key {
	return key.wire
}

func common(role string, version int64, expires time.Time) Common {
	return Common{
		Type: role, SpecVersion: "1.0", Version: version,
		Expires: expires.UTC().Format(time.RFC3339), Environment: testEnvironment,
		RepositoryID: testRepository,
	}
}

func envelopeFor(t *testing.T, signed any, signers map[string]signingKey) []byte {
	t.Helper()
	signedRaw, err := json.Marshal(signed)
	if err != nil {
		t.Fatal(err)
	}
	canonicalSigned, err := CanonicalJSON(signedRaw)
	if err != nil {
		t.Fatal(err)
	}
	signatures := make([]Signature, 0, len(signers))
	// Stable fixture ordering keeps full-envelope hashes deterministic.
	for _, id := range []string{"root-a", "root-b", "root-c", "targets-a", "snapshot-a", "timestamp-a", "releases-a", "security-a"} {
		key, ok := signers[id]
		if !ok {
			continue
		}
		signatures = append(signatures, Signature{
			KeyID: key.id,
			Sig:   hex.EncodeToString(ed25519.Sign(key.private, canonicalSigned)),
		})
	}
	raw, err := json.Marshal(Envelope{Signatures: signatures, Signed: signedRaw})
	if err != nil {
		t.Fatal(err)
	}
	canonical, err := CanonicalJSON(raw)
	if err != nil {
		t.Fatal(err)
	}
	return canonical
}

func fileMeta(t *testing.T, version int64, raw []byte) FileMeta {
	t.Helper()
	canonical, err := CanonicalJSON(raw)
	if err != nil {
		t.Fatal(err)
	}
	return FileMeta{Version: version, Length: int64(len(canonical)), Hashes: map[string]string{"sha256": digest(canonical)}}
}

func newFixture(t *testing.T) fixture {
	t.Helper()
	now := time.Date(2026, 7, 16, 12, 0, 0, 0, time.UTC)
	keys := make(map[string]signingKey)
	for _, id := range []string{"root-a", "root-b", "root-c", "targets-a", "snapshot-a", "timestamp-a", "releases-a", "security-a"} {
		keys[id] = newSigningKey(t)
	}
	rootKeys := make(map[string]Key)
	for _, id := range []string{"root-a", "root-b", "targets-a", "snapshot-a", "timestamp-a"} {
		rootKeys[keys[id].id] = tufKey(keys[id])
	}
	rootSigned := RootSigned{
		Common: common("root", 1, now.Add(365*24*time.Hour)), ConsistentSnapshot: true,
		Keys: rootKeys,
		Roles: map[string]Role{
			"root":      {KeyIDs: []string{keys["root-a"].id, keys["root-b"].id}, Threshold: 2},
			"targets":   {KeyIDs: []string{keys["targets-a"].id}, Threshold: 1},
			"snapshot":  {KeyIDs: []string{keys["snapshot-a"].id}, Threshold: 1},
			"timestamp": {KeyIDs: []string{keys["timestamp-a"].id}, Threshold: 1},
		},
	}
	root := envelopeFor(t, rootSigned, map[string]signingKey{"root-a": keys["root-a"], "root-b": keys["root-b"]})

	baseCustom := TargetCustom{
		Environment: testEnvironment, RegistryOrigin: testOrigin, Package: "alice/mathx", Version: "1.2.3",
		ArchiveSHA256: strings.Repeat("a", 64), ArchiveFilename: "mathx-1.2.3.seenpkg.tgz",
		Visibility: "public", Lifecycle: "active", Retention: "retained", Availability: "available",
		SourceProofSHA256: strings.Repeat("b", 64), ProvenanceSHA256: strings.Repeat("c", 64),
		Dependencies: []TargetDependency{}, Capabilities: []string{"file"},
	}
	baseTarget := TargetMeta{Length: 8192, Hashes: map[string]string{"sha256": strings.Repeat("a", 64)}, Custom: baseCustom}
	releases := TargetsSigned{Common: common("targets", 42, now.Add(4*24*time.Hour)), Targets: map[string]TargetMeta{testTargetPath: baseTarget}}
	securityTarget := baseTarget
	securityTarget.Custom.Availability = "security-quarantined"
	securityTarget.Custom.IncidentID = "inc_test_12345678"
	securityTarget.Custom.SecurityAction = "quarantine"
	security := TargetsSigned{Common: common("targets", 12, now.Add(6*time.Hour)), Targets: map[string]TargetMeta{testTargetPath: securityTarget}}
	targets := TargetsSigned{
		Common: common("targets", 17, now.Add(30*24*time.Hour)), Targets: map[string]TargetMeta{},
		Delegations: &Delegations{
			Keys: map[string]Key{keys["releases-a"].id: tufKey(keys["releases-a"]), keys["security-a"].id: tufKey(keys["security-a"])},
			Roles: []DelegatedRole{
				{Name: "security", KeyIDs: []string{keys["security-a"].id}, Threshold: 1, Paths: []string{"packages/*/*/*/*/*"}},
				{Name: "releases", KeyIDs: []string{keys["releases-a"].id}, Threshold: 1, Paths: []string{"packages/*/*/*/*/*"}},
			},
		},
	}
	releaseRaw := envelopeFor(t, releases, map[string]signingKey{"releases-a": keys["releases-a"]})
	securityRaw := envelopeFor(t, security, map[string]signingKey{"security-a": keys["security-a"]})
	targetsRaw := envelopeFor(t, targets, map[string]signingKey{"targets-a": keys["targets-a"]})
	snapshot := SnapshotSigned{
		Common: common("snapshot", 91, now.Add(24*time.Hour)),
		Meta: map[string]FileMeta{
			"targets.json": fileMeta(t, targets.Version, targetsRaw), "releases.json": fileMeta(t, releases.Version, releaseRaw), "security.json": fileMeta(t, security.Version, securityRaw),
		},
	}
	snapshotRaw := envelopeFor(t, snapshot, map[string]signingKey{"snapshot-a": keys["snapshot-a"]})
	timestamp := TimestampSigned{
		Common: common("timestamp", 108, now.Add(6*time.Hour)),
		Meta:   map[string]FileMeta{"snapshot.json": fileMeta(t, snapshot.Version, snapshotRaw)},
	}
	timestampRaw := envelopeFor(t, timestamp, map[string]signingKey{"timestamp-a": keys["timestamp-a"]})
	return fixture{
		now: now, keys: keys, root: root, releases: releases, security: security, targets: targets, snapshot: snapshot, timestamp: timestamp,
		metadata: MetadataSet{Timestamp: timestampRaw, Snapshot: snapshotRaw, Targets: targetsRaw, Releases: releaseRaw, Security: securityRaw},
	}
}

func verifierFor(t *testing.T, fixture fixture, store StateStore) *Verifier {
	t.Helper()
	v, err := New(Config{Environment: testEnvironment, RepositoryID: testRepository, RegistryOrigin: testOrigin, Store: store, Now: func() time.Time { return fixture.now }})
	if err != nil {
		t.Fatal(err)
	}
	return v
}

func tufCode(t *testing.T, err error) string {
	t.Helper()
	var verification *Error
	if !errors.As(err, &verification) {
		t.Fatalf("expected TUF Error, got %T: %v", err, err)
	}
	return verification.Code
}

func TestRefreshVerifiesChainAndSecurityOverlay(t *testing.T) {
	fixture := newFixture(t)
	v := verifierFor(t, fixture, &MemoryStore{})
	if err := v.BootstrapRoot(fixture.root, strings.Repeat("0", 64)); tufCode(t, err) != "signing_root_pin_mismatch" {
		t.Fatalf("root pin mismatch = %v", err)
	}
	if err := v.BootstrapRoot(fixture.root, digest(fixture.root)); err != nil {
		t.Fatal(err)
	}
	repository, err := v.Refresh(fixture.metadata)
	if err != nil {
		t.Fatal(err)
	}
	selection, err := repository.Select(testTargetPath)
	if err != nil {
		t.Fatal(err)
	}
	if selection.Role != "security" || selection.Target.Custom.Availability != "security-quarantined" {
		t.Fatalf("security overlay was not authoritative: %+v", selection)
	}
	if _, err := v.Refresh(fixture.metadata); err != nil {
		t.Fatalf("safely fresh unchanged network timestamp = %v", err)
	}
	if _, err := v.VerifyCached(fixture.metadata); err != nil {
		t.Fatalf("cached verification failed: %v", err)
	}
}

func TestRefreshRejectsUnchangedTimestampAtFrozenContractMargin(t *testing.T) {
	fixture := newFixture(t)
	now := fixture.now
	v, err := New(Config{
		Environment: testEnvironment, RepositoryID: testRepository, RegistryOrigin: testOrigin,
		Store: &MemoryStore{}, Now: func() time.Time { return now },
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := v.BootstrapRoot(fixture.root, digest(fixture.root)); err != nil {
		t.Fatal(err)
	}
	if _, err := v.Refresh(fixture.metadata); err != nil {
		t.Fatal(err)
	}
	expires, err := time.Parse(time.RFC3339, fixture.timestamp.Expires)
	if err != nil {
		t.Fatal(err)
	}
	now = expires.Add(-minimumNetworkTimestampMargin)
	if _, err := v.Refresh(fixture.metadata); tufCode(t, err) != "signing_freeze_detected" {
		t.Fatalf("unchanged near-expiry network timestamp = %v", err)
	}
	if _, err := v.VerifyCached(fixture.metadata); err != nil {
		t.Fatalf("still-valid cached metadata failed at the network freeze margin: %v", err)
	}
}

func TestMetadataHashLengthAndOriginFailClosed(t *testing.T) {
	baseFixture := newFixture(t)
	tests := []struct {
		name   string
		mutate func(*fixture)
		code   string
	}{
		{
			name: "snapshot bound length", code: "signing_metadata_length_mismatch",
			mutate: func(f *fixture) {
				f.timestamp.Meta["snapshot.json"] = FileMeta{Version: f.snapshot.Version, Length: 1, Hashes: map[string]string{"sha256": digest(f.metadata.Snapshot)}}
				f.metadata.Timestamp = envelopeFor(t, f.timestamp, map[string]signingKey{"timestamp-a": f.keys["timestamp-a"]})
			},
		},
		{
			name: "target origin", code: "signing_origin_mismatch",
			mutate: func(f *fixture) {
				target := f.releases.Targets[testTargetPath]
				target.Custom.RegistryOrigin = "https://seen.dev.yousef.codes/packages"
				f.releases.Targets[testTargetPath] = target
				f.rebuild(t)
			},
		},
		{
			name: "overlay immutable length", code: "signing_overlay_digest_mismatch",
			mutate: func(f *fixture) {
				target := f.security.Targets[testTargetPath]
				target.Length++
				f.security.Targets[testTargetPath] = target
				f.rebuild(t)
			},
		},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			copy := baseFixture
			copy.releases.Targets = cloneMap(baseFixture.releases.Targets)
			copy.security.Targets = cloneMap(baseFixture.security.Targets)
			copy.timestamp.Meta = cloneMap(baseFixture.timestamp.Meta)
			tc.mutate(&copy)
			v := verifierFor(t, copy, &MemoryStore{})
			if err := v.BootstrapRoot(copy.root, digest(copy.root)); err != nil {
				t.Fatal(err)
			}
			_, err := v.Refresh(copy.metadata)
			if got := tufCode(t, err); got != tc.code {
				t.Fatalf("code = %q, want %q: %v", got, tc.code, err)
			}
		})
	}
}

func (f *fixture) rebuild(t *testing.T) {
	t.Helper()
	f.metadata.Releases = envelopeFor(t, f.releases, map[string]signingKey{"releases-a": f.keys["releases-a"]})
	f.metadata.Security = envelopeFor(t, f.security, map[string]signingKey{"security-a": f.keys["security-a"]})
	f.metadata.Targets = envelopeFor(t, f.targets, map[string]signingKey{"targets-a": f.keys["targets-a"]})
	f.snapshot.Meta = map[string]FileMeta{
		"targets.json":  fileMeta(t, f.targets.Version, f.metadata.Targets),
		"releases.json": fileMeta(t, f.releases.Version, f.metadata.Releases),
		"security.json": fileMeta(t, f.security.Version, f.metadata.Security),
	}
	f.metadata.Snapshot = envelopeFor(t, f.snapshot, map[string]signingKey{"snapshot-a": f.keys["snapshot-a"]})
	f.timestamp.Meta = map[string]FileMeta{"snapshot.json": fileMeta(t, f.snapshot.Version, f.metadata.Snapshot)}
	f.metadata.Timestamp = envelopeFor(t, f.timestamp, map[string]signingKey{"timestamp-a": f.keys["timestamp-a"]})
}

func TestExpiryRollbackAndWrongRole(t *testing.T) {
	fixture := newFixture(t)
	v := verifierFor(t, fixture, &MemoryStore{})
	if err := v.BootstrapRoot(fixture.root, digest(fixture.root)); err != nil {
		t.Fatal(err)
	}
	if _, err := v.Refresh(fixture.metadata); err != nil {
		t.Fatal(err)
	}
	rollback := fixture
	rollback.timestamp.Version--
	rollback.metadata.Timestamp = envelopeFor(t, rollback.timestamp, map[string]signingKey{"timestamp-a": rollback.keys["timestamp-a"]})
	if _, err := v.Refresh(rollback.metadata); tufCode(t, err) != "signing_metadata_rollback" {
		t.Fatalf("rollback = %v", err)
	}

	expired := newFixture(t)
	expired.timestamp.Expires = expired.now.Add(-time.Second).Format(time.RFC3339)
	expired.metadata.Timestamp = envelopeFor(t, expired.timestamp, map[string]signingKey{"timestamp-a": expired.keys["timestamp-a"]})
	v = verifierFor(t, expired, &MemoryStore{})
	if err := v.BootstrapRoot(expired.root, digest(expired.root)); err != nil {
		t.Fatal(err)
	}
	if _, err := v.Refresh(expired.metadata); tufCode(t, err) != "signing_metadata_expired" {
		t.Fatalf("expired = %v", err)
	}

	wrong := newFixture(t)
	wrong.metadata.Releases = envelopeFor(t, wrong.releases, map[string]signingKey{"security-a": wrong.keys["security-a"]})
	wrong.snapshot.Meta["releases.json"] = fileMeta(t, wrong.releases.Version, wrong.metadata.Releases)
	wrong.metadata.Snapshot = envelopeFor(t, wrong.snapshot, map[string]signingKey{"snapshot-a": wrong.keys["snapshot-a"]})
	wrong.timestamp.Meta["snapshot.json"] = fileMeta(t, wrong.snapshot.Version, wrong.metadata.Snapshot)
	wrong.metadata.Timestamp = envelopeFor(t, wrong.timestamp, map[string]signingKey{"timestamp-a": wrong.keys["timestamp-a"]})
	v = verifierFor(t, wrong, &MemoryStore{})
	if err := v.BootstrapRoot(wrong.root, digest(wrong.root)); err != nil {
		t.Fatal(err)
	}
	if _, err := v.Refresh(wrong.metadata); tufCode(t, err) != "signing_wrong_role" {
		t.Fatalf("wrong role = %v", err)
	}
}

func TestSequentialRootRotationRequiresBothThresholds(t *testing.T) {
	fixture := newFixture(t)
	store := &MemoryStore{}
	v := verifierFor(t, fixture, store)
	if err := v.BootstrapRoot(fixture.root, digest(fixture.root)); err != nil {
		t.Fatal(err)
	}
	rootKeys := map[string]Key{
		fixture.keys["root-a"].id: tufKey(fixture.keys["root-a"]), fixture.keys["root-b"].id: tufKey(fixture.keys["root-b"]), fixture.keys["root-c"].id: tufKey(fixture.keys["root-c"]),
		fixture.keys["targets-a"].id: tufKey(fixture.keys["targets-a"]), fixture.keys["snapshot-a"].id: tufKey(fixture.keys["snapshot-a"]), fixture.keys["timestamp-a"].id: tufKey(fixture.keys["timestamp-a"]),
	}
	next := RootSigned{
		Common: common("root", 2, fixture.now.Add(365*24*time.Hour)), ConsistentSnapshot: true, Keys: rootKeys,
		Roles: map[string]Role{
			"root": {KeyIDs: []string{fixture.keys["root-b"].id, fixture.keys["root-c"].id}, Threshold: 2}, "targets": {KeyIDs: []string{fixture.keys["targets-a"].id}, Threshold: 1},
			"snapshot": {KeyIDs: []string{fixture.keys["snapshot-a"].id}, Threshold: 1}, "timestamp": {KeyIDs: []string{fixture.keys["timestamp-a"].id}, Threshold: 1},
		},
	}
	dualSigned := envelopeFor(t, next, map[string]signingKey{"root-a": fixture.keys["root-a"], "root-b": fixture.keys["root-b"], "root-c": fixture.keys["root-c"]})
	if err := v.UpdateRoot(dualSigned); err != nil {
		t.Fatal(err)
	}

	fresh := verifierFor(t, fixture, &MemoryStore{})
	if err := fresh.BootstrapRoot(fixture.root, digest(fixture.root)); err != nil {
		t.Fatal(err)
	}
	missingOld := envelopeFor(t, next, map[string]signingKey{"root-b": fixture.keys["root-b"], "root-c": fixture.keys["root-c"]})
	if err := fresh.UpdateRoot(missingOld); tufCode(t, err) != "signing_old_root_threshold_not_met" {
		t.Fatalf("missing old threshold = %v", err)
	}
}

func TestCanonicalJSONRejectsDuplicatesAndEd25519HexWire(t *testing.T) {
	if _, err := CanonicalJSON([]byte(`{"a":1,"a":2}`)); err == nil {
		t.Fatal("duplicate JSON members accepted")
	}
	if _, err := CanonicalJSON([]byte(`{"s":"\ud800"}`)); err == nil {
		t.Fatal("unpaired Unicode surrogate accepted")
	}
	canonical, err := CanonicalJSON([]byte(`{"s":"\u0007\u2028é","a":1}`))
	if err != nil {
		t.Fatal(err)
	}
	if want := "{\"a\":1,\"s\":\"\\u0007\u2028é\"}"; string(canonical) != want {
		t.Fatalf("canonical Unicode/control encoding = %q, want %q", canonical, want)
	}
	key := newSigningKey(t)
	message := []byte(`{"signed":"canonical"}`)
	signature := hex.EncodeToString(ed25519.Sign(key.private, message))
	if err := verifySignature(key.wire, message, signature); err != nil {
		t.Fatal(err)
	}
	if derived, err := deriveKeyID(key.wire); err != nil || derived != key.id {
		t.Fatalf("key ID derivation = %q, %v", derived, err)
	}
}

func TestFileStoreRoundTrip(t *testing.T) {
	fixture := newFixture(t)
	path := filepath.Join(t.TempDir(), "trusted", "state.json")
	store := &FileStore{Path: path}
	v := verifierFor(t, fixture, store)
	if err := v.BootstrapRoot(fixture.root, digest(fixture.root)); err != nil {
		t.Fatal(err)
	}
	if _, err := v.Refresh(fixture.metadata); err != nil {
		t.Fatal(err)
	}
	reloaded := verifierFor(t, fixture, &FileStore{Path: path})
	if _, err := reloaded.VerifyCached(fixture.metadata); err != nil {
		t.Fatal(err)
	}
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if info.Mode().Perm()&0o077 != 0 {
		t.Fatalf("trusted state permissions = %v", info.Mode())
	}
}

func TestIdentityFromTrustedStateVerifiesPinnedRoot(t *testing.T) {
	fixture := newFixture(t)
	store := &MemoryStore{}
	verifier := verifierFor(t, fixture, store)
	if err := verifier.BootstrapRoot(fixture.root, digest(fixture.root)); err != nil {
		t.Fatal(err)
	}
	state, err := store.Load()
	if err != nil {
		t.Fatal(err)
	}
	identity, err := IdentityFromTrustedState(state)
	if err != nil {
		t.Fatal(err)
	}
	if identity.Environment != testEnvironment || identity.RepositoryID != testRepository {
		t.Fatalf("identity = %+v", identity)
	}
	state.Fingerprints["root"] = strings.Repeat("0", 64)
	if _, err := IdentityFromTrustedState(state); err == nil {
		t.Fatal("trusted identity accepted a mismatched root fingerprint")
	}
}

func TestFrozenContractFixtureVerifies(t *testing.T) {
	fixturePath := filepath.Join("..", "..", "..", "..", "contracts", "package-registry", "v1", "fixtures", "tuf-metadata-examples.json")
	raw, err := os.ReadFile(fixturePath)
	if err != nil {
		t.Fatal(err)
	}
	var contract struct {
		ValidationTime string                     `json:"validation_time"`
		Metadata       map[string]json.RawMessage `json:"metadata"`
	}
	if err := json.Unmarshal(raw, &contract); err != nil {
		t.Fatal(err)
	}
	now, err := time.Parse(time.RFC3339, contract.ValidationTime)
	if err != nil {
		t.Fatal(err)
	}
	v, err := New(Config{
		Environment: "development", RepositoryID: "seen-dev-test-fixture-v1",
		RegistryOrigin: "https://test.invalid/packages", Store: &MemoryStore{}, Now: func() time.Time { return now },
	})
	if err != nil {
		t.Fatal(err)
	}
	set := MetadataSet{
		Timestamp: contract.Metadata["timestamp"], Snapshot: contract.Metadata["snapshot"],
		Targets: contract.Metadata["targets"], Releases: contract.Metadata["release_targets"], Security: contract.Metadata["security_targets"],
	}
	if _, err := v.Refresh(set); tufCode(t, err) != "signing_no_trusted_root" {
		t.Fatalf("official-root fail-closed check = %v", err)
	}
	rootCanonical, err := CanonicalJSON(contract.Metadata["root"])
	if err != nil {
		t.Fatal(err)
	}
	if err := v.BootstrapRoot(contract.Metadata["root"], digest(rootCanonical)); err != nil {
		t.Fatal(err)
	}
	repository, err := v.Refresh(set)
	if err != nil {
		t.Fatal(err)
	}
	selection, err := repository.Select(testTargetPath)
	if err != nil || selection.Role != "security" {
		t.Fatalf("contract target selection = %+v, %v", selection, err)
	}
	if len(selection.Target.Custom.Dependencies) != 1 || selection.Target.Custom.Dependencies[0].Package != "seen/util" {
		t.Fatalf("signed dependency graph was not retained: %+v", selection.Target.Custom.Dependencies)
	}
}
