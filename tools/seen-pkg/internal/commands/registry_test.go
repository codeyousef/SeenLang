package commands

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/codeyousef/seen/tools/seen-pkg/internal/model"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/resolver"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/tuf"
)

func TestWritePrivateAtomicReplacesExistingMetadata(t *testing.T) {
	t.Parallel()
	filename := filepath.Join(t.TempDir(), "metadata.json")
	if err := writePrivateAtomic(filename, []byte("old\n")); err != nil {
		t.Fatal(err)
	}
	if err := writePrivateAtomic(filename, []byte("new\n")); err != nil {
		t.Fatalf("replace existing metadata: %v", err)
	}
	content, err := os.ReadFile(filename)
	if err != nil || string(content) != "new\n" {
		t.Fatalf("metadata = %q, %v", content, err)
	}
}

func TestInitialTrustedRootManualOverride(t *testing.T) {
	t.Parallel()
	manualRoot := []byte(`{"manual":true}`)
	manualPath := filepath.Join(t.TempDir(), "root.json")
	if err := os.WriteFile(manualPath, manualRoot, 0o600); err != nil {
		t.Fatal(err)
	}
	root, digest, err := initialTrustedRoot(registrySpec{
		Alias:                    "default",
		TrustedRoot:              manualPath,
		TrustedRootSHA256:        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
		BuiltinTrustedRoot:       `{"builtin":true}`,
		BuiltinTrustedRootSHA256: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
	})
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(root, manualRoot) || digest != "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" {
		t.Fatalf("manual trust selection = %q, %q", root, digest)
	}
}

func TestInitialTrustedRootRejectsIncompleteConfiguration(t *testing.T) {
	t.Parallel()
	cases := map[string]registrySpec{
		"manual root only":   {Alias: "custom", TrustedRoot: "/tmp/root.json"},
		"manual digest only": {Alias: "custom", TrustedRootSHA256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
		"builtin root only":  {Alias: "custom", BuiltinTrustedRoot: `{}`},
		"builtin digest only": {
			Alias:                    "custom",
			BuiltinTrustedRootSHA256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
		},
		"no root": {Alias: "custom"},
	}
	for name, spec := range cases {
		t.Run(name, func(t *testing.T) {
			t.Parallel()
			if _, _, err := initialTrustedRoot(spec); err == nil {
				t.Fatal("incomplete trust configuration accepted")
			}
		})
	}
}

func TestDevelopmentRegistryEmbedsVerifiedRoot(t *testing.T) {
	t.Parallel()
	spec, err := defaultRegistrySpec("default", "https://seen.dev.yousef.codes/packages")
	if err != nil {
		t.Fatal(err)
	}
	root, pinnedDigest, err := initialTrustedRoot(spec)
	if err != nil {
		t.Fatal(err)
	}
	canonicalRoot, err := tuf.CanonicalJSON(root)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.Sum256(canonicalRoot)
	if actual := hex.EncodeToString(digest[:]); actual != seenDevelopmentTrustedRootSHA256 || pinnedDigest != actual {
		t.Fatalf("development root digest = %q (pin %q), want %q", actual, pinnedDigest, seenDevelopmentTrustedRootSHA256)
	}
	store := &tuf.FileStore{Path: filepath.Join(t.TempDir(), "trusted-state.json")}
	verifier, err := tuf.New(tuf.Config{
		Environment:    spec.Environment,
		RepositoryID:   spec.RepositoryID,
		RegistryOrigin: spec.Origin,
		Store:          store,
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := verifier.BootstrapRoot(root, pinnedDigest); err != nil {
		t.Fatalf("bootstrap embedded development root: %v", err)
	}
}

func TestMetadataFailureKeepsPreviousCacheGenerationAndTrustedState(t *testing.T) {
	fixturePath := filepath.Join("..", "..", "..", "..", "contracts", "package-registry", "v1", "fixtures", "tuf-metadata-examples.json")
	raw, err := os.ReadFile(fixturePath)
	if err != nil {
		t.Fatal(err)
	}
	var fixture struct {
		Metadata map[string]json.RawMessage `json:"metadata"`
	}
	if err := json.Unmarshal(raw, &fixture); err != nil {
		t.Fatal(err)
	}
	set := tuf.MetadataSet{
		Timestamp: fixture.Metadata["timestamp"], Snapshot: fixture.Metadata["snapshot"],
		Targets: fixture.Metadata["targets"], Releases: fixture.Metadata["release_targets"], Security: fixture.Metadata["security_targets"],
	}
	directory := t.TempDir()
	oldNames := metadataNames{Snapshot: "4.snapshot.json", Targets: "1.targets.json", Releases: "2.releases.json", Security: "3.security.json"}
	if err := commitMetadata(directory, set, oldNames, ""); err != nil {
		t.Fatal(err)
	}
	oldTimestamp, err := os.ReadFile(filepath.Join(directory, "timestamp.json"))
	if err != nil {
		t.Fatal(err)
	}

	store := &tuf.FileStore{Path: filepath.Join(directory, "trusted-state.json")}
	initialState := tuf.TrustedState{
		Version: 1, Root: json.RawMessage(`{}`), Versions: map[string]int64{"timestamp": 1},
		Expires: map[string]string{}, Fingerprints: map[string]string{},
	}
	if err := store.Save(initialState); err != nil {
		t.Fatal(err)
	}
	transaction, err := store.Begin(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	defer transaction.Close()
	pendingState := initialState
	pendingState.Versions = map[string]int64{"timestamp": 2}
	if err := transaction.Save(pendingState); err != nil {
		t.Fatal(err)
	}

	injected := errors.New("injected timestamp commit failure")
	newNames := metadataNames{Snapshot: "104.snapshot.json", Targets: "101.targets.json", Releases: "102.releases.json", Security: "103.security.json"}
	err = commitMetadataAndTrustedStateWithWriter(directory, set, newNames, "", transaction, func(filename string, content []byte) error {
		if filepath.Base(filename) == "timestamp.json" {
			return injected
		}
		return writePrivateAtomic(filename, content)
	})
	if !errors.Is(err, injected) {
		t.Fatalf("metadata transaction error = %v, want injected failure", err)
	}
	for _, name := range []string{newNames.Snapshot, newNames.Targets, newNames.Releases, newNames.Security} {
		if _, err := os.Stat(filepath.Join(directory, name)); err != nil {
			t.Fatalf("versioned metadata %s was not installed before timestamp: %v", name, err)
		}
	}
	currentTimestamp, err := os.ReadFile(filepath.Join(directory, "timestamp.json"))
	if err != nil || !bytes.Equal(currentTimestamp, oldTimestamp) {
		t.Fatalf("timestamp pointer changed after failed commit: equal=%v err=%v", bytes.Equal(currentTimestamp, oldTimestamp), err)
	}
	cached, err := loadCachedMetadata(directory)
	if err != nil {
		t.Fatalf("previous metadata generation is not readable: %v", err)
	}
	if !bytes.Equal(cached.Timestamp, set.Timestamp) || !bytes.Equal(cached.Snapshot, set.Snapshot) ||
		!bytes.Equal(cached.Targets, set.Targets) || !bytes.Equal(cached.Releases, set.Releases) || !bytes.Equal(cached.Security, set.Security) {
		t.Fatal("failed commit changed the active cached metadata generation")
	}
	persisted, err := store.Load()
	if err != nil {
		t.Fatal(err)
	}
	if persisted.Versions["timestamp"] != 1 {
		t.Fatalf("trusted state advanced before metadata commit: %+v", persisted.Versions)
	}
}

func TestCustomRegistryAutomaticFetchReusesTrustedIdentity(t *testing.T) {
	fixturePath := filepath.Join("..", "..", "..", "..", "contracts", "package-registry", "v1", "fixtures", "tuf-metadata-examples.json")
	raw, err := os.ReadFile(fixturePath)
	if err != nil {
		t.Fatal(err)
	}
	var fixture struct {
		ValidationTime string                     `json:"validation_time"`
		Metadata       map[string]json.RawMessage `json:"metadata"`
	}
	if err := json.Unmarshal(raw, &fixture); err != nil {
		t.Fatal(err)
	}
	now, err := time.Parse(time.RFC3339, fixture.ValidationTime)
	if err != nil {
		t.Fatal(err)
	}
	const (
		origin       = "https://test.invalid/packages"
		environment  = "development"
		repositoryID = "seen-dev-test-fixture-v1"
	)
	cacheRoot, err := filepath.Abs(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	parsed := &model.Manifest{Registries: map[string]string{"custom": origin}}
	manualCLI := resolutionCLI{
		CacheRoot:         cacheRoot,
		TrustedRoots:      map[string]string{},
		TrustedRootSHA256: map[string]string{},
		Environments:      map[string]string{"custom": environment},
		RepositoryIDs:     map[string]string{"custom": repositoryID},
	}
	manualSpecs, err := registrySpecs(parsed, manualCLI, cacheRoot)
	if err != nil || len(manualSpecs) != 1 {
		t.Fatalf("manual registry configuration = %+v, %v", manualSpecs, err)
	}
	directory := filepath.Join(cacheRoot, "metadata", originDigest(origin))
	store := &tuf.FileStore{Path: filepath.Join(directory, "trusted-state.json")}
	verifier, err := tuf.New(tuf.Config{
		Environment: environment, RepositoryID: repositoryID, RegistryOrigin: origin,
		Store: store, Now: func() time.Time { return now },
	})
	if err != nil {
		t.Fatal(err)
	}
	canonicalRoot, err := tuf.CanonicalJSON(fixture.Metadata["root"])
	if err != nil {
		t.Fatal(err)
	}
	rootDigest := sha256.Sum256(canonicalRoot)
	if err := verifier.BootstrapRoot(fixture.Metadata["root"], hex.EncodeToString(rootDigest[:])); err != nil {
		t.Fatal(err)
	}
	set := tuf.MetadataSet{
		Timestamp: fixture.Metadata["timestamp"], Snapshot: fixture.Metadata["snapshot"],
		Targets: fixture.Metadata["targets"], Releases: fixture.Metadata["release_targets"], Security: fixture.Metadata["security_targets"],
	}
	if _, err := verifier.Refresh(set); err != nil {
		t.Fatal(err)
	}
	if err := commitMetadata(directory, set, metadataNames{Snapshot: "4.snapshot.json", Targets: "1.targets.json", Releases: "2.releases.json", Security: "3.security.json"}, ""); err != nil {
		t.Fatal(err)
	}

	// This is the configuration shape used by compiler-triggered fetches: only
	// the manifest/cache/mode survive, while signing identity comes from the
	// already verified local root rather than from the manifest or network.
	automaticCLI := resolutionCLI{
		CacheRoot: cacheRoot, TrustedRoots: map[string]string{}, TrustedRootSHA256: map[string]string{},
		Environments: map[string]string{}, RepositoryIDs: map[string]string{},
	}
	automaticSpecs, err := registrySpecs(parsed, automaticCLI, cacheRoot)
	if err != nil || len(automaticSpecs) != 1 {
		t.Fatalf("automatic registry configuration = %+v, %v", automaticSpecs, err)
	}
	if automaticSpecs[0].Environment != environment || automaticSpecs[0].RepositoryID != repositoryID {
		t.Fatalf("derived identity = %+v", automaticSpecs[0])
	}
	runtime := newRegistryRuntime(automaticSpecs, cacheRoot, true)
	runtime.now = func() time.Time { return now }
	candidates, err := runtime.Candidates(context.Background(), model.PackageKey{RegistryOrigin: origin, Package: "alice/mathx"}, resolver.Access{Offline: true})
	if err != nil || len(candidates) != 1 {
		t.Fatalf("automatic cached fetch candidates = %+v, %v", candidates, err)
	}

	conflicting := automaticCLI
	conflicting.Environments = map[string]string{"custom": "production"}
	if _, err := registrySpecs(parsed, conflicting, cacheRoot); err == nil {
		t.Fatal("conflicting environment accepted over pinned trusted identity")
	}
	conflicting = automaticCLI
	conflicting.RepositoryIDs = map[string]string{"custom": "seen-dev-conflict-v1"}
	if _, err := registrySpecs(parsed, conflicting, cacheRoot); err == nil {
		t.Fatal("conflicting repository ID accepted over pinned trusted identity")
	}
}
