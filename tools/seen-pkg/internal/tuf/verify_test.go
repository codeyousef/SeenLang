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
	testEnvironment       = "production"
	testRepository        = "seen-prod-registry-v1"
	testOrigin            = "https://seen.yousef.codes/packages"
	testTargetPath        = "packages/alice/mathx/1.2.3/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/mathx-1.2.3.seenpkg.tgz"
	fixtureActivationTime = "2026-07-16T11:59:00Z"
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

type clientConformanceContract struct {
	Cases []clientConformanceCase `json:"client_conformance_cases"`
}

type clientConformanceCase struct {
	Name         string                  `json:"name"`
	InitialState string                  `json:"initial_state"`
	Steps        []clientConformanceStep `json:"steps"`
}

type clientConformanceStep struct {
	Action    string `json:"action"`
	Expected  string `json:"expected"`
	ErrorCode string `json:"error_code"`
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
		Owner: "alice", Name: "mathx",
		ArchiveSHA256: strings.Repeat("a", 64), ArchiveFilename: "mathx-1.2.3.seenpkg.tgz",
		Blob:               AttestedBlob{SHA256: strings.Repeat("a", 64), Length: 8192},
		PublisherPrincipal: "publisher:alice", RegistryServiceIdentity: "release-promoter",
		SourceRepository: AttestedRepository{Forge: "github", RepositoryID: "987654321", CanonicalURL: "https://github.com/alice/mathx"},
		SourceCommit:     AttestedCommit{Algorithm: "sha1", Value: "0123456789abcdef0123456789abcdef01234567"},
		Review: AttestedReview{
			Result: "passed", PolicyVersion: "package-scan-v1.0.0",
			SourceProofID: "prf_01JZX8K9F7Q2W4N6M8R0", SourceProofSHA256: strings.Repeat("b", 64),
			ScanAttestationID: "scn_01JZX9M1N2P3Q4R5S6T7", ScanAttestationSHA256: strings.Repeat("d", 64),
			ScannerID: "seen-package-scanner", ScannerVersion: "1.0.0", AttestationSequence: 4,
		},
		Visibility: "public", Lifecycle: "active", Retention: "retained", Availability: "available",
		ActivatedAt:       fixtureActivationTime,
		SourceProofSHA256: strings.Repeat("b", 64),
		Dependencies:      []TargetDependency{}, Capabilities: []string{"file"},
	}
	attestationSHA256, err := registryAttestationDigest(baseCustom)
	if err != nil {
		t.Fatal(err)
	}
	baseCustom.RegistryAttestationSHA256 = attestationSHA256
	baseCustom.ProvenanceSHA256 = attestationSHA256
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

func loadClientConformanceCases(t *testing.T) map[string]clientConformanceCase {
	t.Helper()
	fixturePath := filepath.Join("..", "..", "..", "..", "contracts", "package-registry", "v1", "fixtures", "tuf-metadata-examples.json")
	raw, err := os.ReadFile(fixturePath)
	if err != nil {
		t.Fatal(err)
	}
	var contract clientConformanceContract
	if err := json.Unmarshal(raw, &contract); err != nil {
		t.Fatal(err)
	}
	cases := make(map[string]clientConformanceCase, len(contract.Cases))
	for _, item := range contract.Cases {
		if item.Name == "" || len(item.Steps) == 0 {
			t.Fatalf("invalid client conformance case: %+v", item)
		}
		if _, duplicate := cases[item.Name]; duplicate {
			t.Fatalf("duplicate client conformance case %q", item.Name)
		}
		cases[item.Name] = item
	}
	return cases
}

func requireConformanceSteps(t *testing.T, item clientConformanceCase, initialState string, actions ...string) {
	t.Helper()
	if item.InitialState != initialState {
		t.Fatalf("%s initial state = %q, want %q", item.Name, item.InitialState, initialState)
	}
	if len(item.Steps) != len(actions) {
		t.Fatalf("%s steps = %d, want %d", item.Name, len(item.Steps), len(actions))
	}
	for index, action := range actions {
		if item.Steps[index].Action != action {
			t.Fatalf("%s step %d action = %q, want %q", item.Name, index, item.Steps[index].Action, action)
		}
	}
}

func assertConformanceResult(t *testing.T, step clientConformanceStep, err error) {
	t.Helper()
	switch step.Expected {
	case "accept":
		if step.ErrorCode != "" {
			t.Fatalf("accepted conformance step declares error code %q", step.ErrorCode)
		}
		if err != nil {
			t.Fatalf("conformance step rejected: %v", err)
		}
	case "reject":
		if err == nil {
			t.Fatal("conformance step unexpectedly accepted")
		}
		if got := tufCode(t, err); got != step.ErrorCode {
			t.Fatalf("conformance error = %q, want %q: %v", got, step.ErrorCode, err)
		}
	default:
		t.Fatalf("unknown conformance expectation %q", step.Expected)
	}
}

func bootstrapAndRefreshFixture(t *testing.T, fixture fixture) *Verifier {
	t.Helper()
	verifier := verifierFor(t, fixture, &MemoryStore{})
	if err := verifier.BootstrapRoot(fixture.root, digest(fixture.root)); err != nil {
		t.Fatal(err)
	}
	if _, err := verifier.Refresh(fixture.metadata); err != nil {
		t.Fatal(err)
	}
	return verifier
}

func advanceReleaseTransaction(t *testing.T, fixture *fixture, signer signingKey) {
	t.Helper()
	fixture.releases.Version++
	fixture.snapshot.Version++
	fixture.timestamp.Version++
	fixture.rebuildWithDelegatedSigners(t, signer, fixture.keys["security-a"])
}

func replaceReleaseDelegation(t *testing.T, fixture *fixture, replacement signingKey) {
	t.Helper()
	if fixture.targets.Delegations == nil {
		t.Fatal("fixture has no delegated targets policy")
	}
	roles := append([]DelegatedRole(nil), fixture.targets.Delegations.Roles...)
	found := false
	for index := range roles {
		if roles[index].Name == "releases" {
			roles[index].KeyIDs = []string{replacement.id}
			found = true
		}
	}
	if !found {
		t.Fatal("fixture has no releases delegation")
	}
	fixture.targets.Delegations = &Delegations{
		Keys: map[string]Key{
			replacement.id:                tufKey(replacement),
			fixture.keys["security-a"].id: tufKey(fixture.keys["security-a"]),
		},
		Roles: roles,
	}
	fixture.targets.Version++
	fixture.releases.Version++
	fixture.snapshot.Version++
	fixture.timestamp.Version++
}

func runWrongEnvironmentConformance(t *testing.T, item clientConformanceCase) {
	requireConformanceSteps(t, item, "trusted-root-only", "refresh-complete-chain-resigned-with-release-environment-changed")
	fixture := newFixture(t)
	fixture.releases.Environment = "development"
	fixture.rebuild(t)
	verifier := verifierFor(t, fixture, &MemoryStore{})
	if err := verifier.BootstrapRoot(fixture.root, digest(fixture.root)); err != nil {
		t.Fatal(err)
	}
	_, err := verifier.Refresh(fixture.metadata)
	assertConformanceResult(t, item.Steps[0], err)
}

func runMissingMetadataConformance(t *testing.T, item clientConformanceCase) {
	requireConformanceSteps(t, item, "trusted-root-only", "refresh-complete-chain-with-security-metadata-absent")
	fixture := newFixture(t)
	store := &MemoryStore{}
	verifier := verifierFor(t, fixture, store)
	if err := verifier.BootstrapRoot(fixture.root, digest(fixture.root)); err != nil {
		t.Fatal(err)
	}
	fixture.metadata.Security = nil
	_, err := verifier.Refresh(fixture.metadata)
	assertConformanceResult(t, item.Steps[0], err)
	state, err := store.Load()
	if err != nil {
		t.Fatal(err)
	}
	if state.Versions["timestamp"] != 0 || state.Versions["snapshot"] != 0 || state.Versions["security"] != 0 {
		t.Fatalf("missing metadata advanced trusted state: %+v", state.Versions)
	}
}

func runCompromisedCurrentKeyConformance(t *testing.T, item clientConformanceCase) {
	requireConformanceSteps(t, item, "base-metadata-refreshed", "refresh-next-release-version-signed-by-current-delegated-key")
	fixture := newFixture(t)
	verifier := bootstrapAndRefreshFixture(t, fixture)
	target := fixture.releases.Targets[testTargetPath]
	target.Custom.Availability = "yanked"
	target.Custom.YankReason = "compromised delegated key changed availability"
	fixture.releases.Targets[testTargetPath] = target
	advanceReleaseTransaction(t, &fixture, fixture.keys["releases-a"])
	_, err := verifier.Refresh(fixture.metadata)
	assertConformanceResult(t, item.Steps[0], err)
}

func runRevokedDelegationConformance(t *testing.T, item clientConformanceCase) {
	requireConformanceSteps(t, item, "base-metadata-refreshed", "refresh-resigned-chain-after-offline-targets-replaces-release-key-but-release-uses-former-key")
	fixture := newFixture(t)
	verifier := bootstrapAndRefreshFixture(t, fixture)
	former := fixture.keys["releases-a"]
	replacement := newSigningKey(t)
	replaceReleaseDelegation(t, &fixture, replacement)
	fixture.rebuildWithDelegatedSigners(t, former, fixture.keys["security-a"])
	_, err := verifier.Refresh(fixture.metadata)
	assertConformanceResult(t, item.Steps[0], err)
}

func runRecoveryConformance(t *testing.T, item clientConformanceCase) {
	requireConformanceSteps(
		t,
		item,
		"base-metadata-refreshed",
		"reject-resigned-chain-after-offline-targets-replaces-release-key-but-release-uses-former-key",
		"refresh-same-next-versions-resigned-by-replacement-release-key",
	)
	fixture := newFixture(t)
	verifier := bootstrapAndRefreshFixture(t, fixture)
	former := fixture.keys["releases-a"]
	replacement := newSigningKey(t)
	replaceReleaseDelegation(t, &fixture, replacement)
	fixture.rebuildWithDelegatedSigners(t, former, fixture.keys["security-a"])
	_, err := verifier.Refresh(fixture.metadata)
	assertConformanceResult(t, item.Steps[0], err)

	// The failed transaction did not advance trust. Re-signing the same next
	// signed versions with the newly delegated key must therefore recover.
	fixture.rebuildWithDelegatedSigners(t, replacement, fixture.keys["security-a"])
	repository, err := verifier.Refresh(fixture.metadata)
	assertConformanceResult(t, item.Steps[1], err)
	if repository == nil || repository.Releases.Version != fixture.releases.Version {
		t.Fatalf("recovery did not install replacement metadata: %+v", repository)
	}
}

func TestFrozenClientConformanceCases(t *testing.T) {
	cases := loadClientConformanceCases(t)
	runners := []struct {
		name string
		run  func(*testing.T, clientConformanceCase)
	}{
		{"wrong-environment-signed-chain", runWrongEnvironmentConformance},
		{"missing-delegated-metadata", runMissingMetadataConformance},
		{"compromised-online-key-remains-authorized-before-revocation", runCompromisedCurrentKeyConformance},
		{"revoked-delegation-rejects-former-online-key", runRevokedDelegationConformance},
		{"replacement-delegation-recovers-after-compromise", runRecoveryConformance},
	}
	if len(cases) != len(runners) {
		t.Fatalf("client conformance cases = %d, want %d", len(cases), len(runners))
	}
	for _, runner := range runners {
		item, ok := cases[runner.name]
		if !ok {
			t.Fatalf("missing client conformance case %q", runner.name)
		}
		t.Run(runner.name, func(t *testing.T) { runner.run(t, item) })
	}
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
				target.Custom.Blob.Length = target.Length
				attestationSHA256, err := registryAttestationDigest(target.Custom)
				if err != nil {
					t.Fatal(err)
				}
				target.Custom.RegistryAttestationSHA256 = attestationSHA256
				target.Custom.ProvenanceSHA256 = attestationSHA256
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

func TestRegistryAttestationFailsClosed(t *testing.T) {
	baseFixture := newFixture(t)
	tests := []struct {
		name   string
		mutate func(*TargetCustom)
	}{
		{
			name: "publisher principal missing",
			mutate: func(custom *TargetCustom) {
				custom.PublisherPrincipal = ""
			},
		},
		{
			name: "source commit invalid",
			mutate: func(custom *TargetCustom) {
				custom.SourceCommit.Value = "not-a-commit"
			},
		},
		{
			name: "source repository identifier invalid",
			mutate: func(custom *TargetCustom) {
				custom.SourceRepository.RepositoryID = "repository id with spaces"
			},
		},
		{
			name: "source repository host is not canonical lowercase",
			mutate: func(custom *TargetCustom) {
				custom.SourceRepository.CanonicalURL = "https://GitHub.com/alice/mathx"
			},
		},
		{
			name: "source repository path is percent encoded",
			mutate: func(custom *TargetCustom) {
				custom.SourceRepository.CanonicalURL = "https://github.com/alice/math%78"
			},
		},
		{
			name: "source proof differs from review",
			mutate: func(custom *TargetCustom) {
				custom.SourceProofSHA256 = strings.Repeat("c", 64)
			},
		},
		{
			name: "activation is in the future",
			mutate: func(custom *TargetCustom) {
				custom.ActivatedAt = baseFixture.now.Add(10 * time.Minute).Format(time.RFC3339)
			},
		},
		{
			name: "canonical digest is stale",
			mutate: func(custom *TargetCustom) {
				custom.PublisherPrincipal = "publisher:bob"
			},
		},
		{
			name: "legacy provenance digest differs",
			mutate: func(custom *TargetCustom) {
				custom.ProvenanceSHA256 = strings.Repeat("c", 64)
			},
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			fixture := newFixture(t)
			target := fixture.releases.Targets[testTargetPath]
			tc.mutate(&target.Custom)
			fixture.releases.Targets[testTargetPath] = target
			fixture.rebuild(t)

			verifier := verifierFor(t, fixture, &MemoryStore{})
			if err := verifier.BootstrapRoot(fixture.root, digest(fixture.root)); err != nil {
				t.Fatal(err)
			}
			if _, err := verifier.Refresh(fixture.metadata); tufCode(t, err) != "signing_target_attestation_invalid" {
				t.Fatalf("attestation mutation = %v", err)
			}
		})
	}
}

func TestSecurityTargetRejectsYankReason(t *testing.T) {
	fixture := newFixture(t)
	target := fixture.security.Targets[testTargetPath]
	target.Custom.YankReason = "release-role field must not cross the security boundary"
	fixture.security.Targets[testTargetPath] = target
	fixture.rebuild(t)

	verifier := verifierFor(t, fixture, &MemoryStore{})
	if err := verifier.BootstrapRoot(fixture.root, digest(fixture.root)); err != nil {
		t.Fatal(err)
	}
	if _, err := verifier.Refresh(fixture.metadata); tufCode(t, err) != "signing_metadata_invalid" {
		t.Fatalf("security target with yank reason = %v", err)
	}
}

func (f *fixture) rebuild(t *testing.T) {
	f.rebuildWithDelegatedSigners(t, f.keys["releases-a"], f.keys["security-a"])
}

func (f *fixture) rebuildWithDelegatedSigners(t *testing.T, releasesSigner, securitySigner signingKey) {
	t.Helper()
	f.metadata.Releases = envelopeFor(t, f.releases, map[string]signingKey{"releases-a": releasesSigner})
	f.metadata.Security = envelopeFor(t, f.security, map[string]signingKey{"security-a": securitySigner})
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
