package commands

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"

	"github.com/codeyousef/seen/tools/seen-pkg/internal/atomicfile"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/model"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/resolver"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/transport"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/tuf"
)

const maxMetadataBytes int64 = 4 * 1024 * 1024

type registrySpec struct {
	Alias, Origin, Environment, RepositoryID, TrustedRoot, TrustedRootSHA256 string
}

type registryRuntime struct {
	mu           sync.Mutex
	specs        map[string]registrySpec
	metadataRoot string
	offline      bool
	preferCached bool
	verified     map[string]*tuf.Repository
	policy       transport.Policy
}

func newRegistryRuntime(specs []registrySpec, cacheRoot string, offline bool) *registryRuntime {
	byOrigin := make(map[string]registrySpec, len(specs))
	for _, spec := range specs {
		byOrigin[spec.Origin] = spec
	}
	return &registryRuntime{specs: byOrigin, metadataRoot: filepath.Join(cacheRoot, "metadata"), offline: offline, verified: map[string]*tuf.Repository{}, policy: transport.DefaultPolicy()}
}

func (runtime *registryRuntime) Candidates(ctx context.Context, key model.PackageKey, access resolver.Access) ([]model.Candidate, error) {
	repository, err := runtime.load(ctx, key.RegistryOrigin, access.Offline || runtime.offline)
	if err != nil {
		return nil, err
	}
	var result []model.Candidate
	paths := make([]string, 0, len(repository.Releases.Targets))
	for targetPath := range repository.Releases.Targets {
		paths = append(paths, targetPath)
	}
	sortStrings(paths)
	for _, targetPath := range paths {
		selection, err := repository.Select(targetPath)
		if err != nil {
			return nil, err
		}
		custom := selection.Target.Custom
		if custom.Package != key.Package || custom.RegistryOrigin != key.RegistryOrigin {
			continue
		}
		candidate, err := candidateFromSelection(repository, selection)
		if err != nil {
			return nil, err
		}
		result = append(result, candidate)
	}
	return result, nil
}

func candidateFromSelection(repository *tuf.Repository, selection tuf.Selection) (model.Candidate, error) {
	custom := selection.Target.Custom
	capabilities := make([]model.Capability, len(custom.Capabilities))
	for index, value := range custom.Capabilities {
		capabilities[index] = model.Capability(value)
	}
	dependencies := make([]model.Edge, len(custom.Dependencies))
	for index, dependency := range custom.Dependencies {
		allow := make([]model.Capability, len(dependency.Allow))
		for allowIndex, value := range dependency.Allow {
			allow[allowIndex] = model.Capability(value)
		}
		dependencies[index] = model.Edge{Alias: dependency.Alias, Package: dependency.Package, RegistryOrigin: dependency.RegistryOrigin, Requirement: dependency.Requirement, Allow: allow}
	}
	metadataVersion := repository.Releases.Version
	if selection.Role == "security" {
		metadataVersion = repository.Security.Version
	}
	if metadataVersion < 1 {
		return model.Candidate{}, fmt.Errorf("signed target has invalid metadata version")
	}
	return model.Candidate{Package: custom.Package, Version: custom.Version, RegistryOrigin: custom.RegistryOrigin, ArchiveSHA256: custom.ArchiveSHA256, TargetPath: selection.Path, MetadataVersion: uint64(metadataVersion), Availability: model.Availability(custom.Availability), Capabilities: capabilities, Dependencies: dependencies}, nil
}

func (runtime *registryRuntime) target(ctx context.Context, candidate model.Candidate) (tuf.TargetMeta, error) {
	repository, err := runtime.load(ctx, candidate.RegistryOrigin, runtime.offline)
	if err != nil {
		return tuf.TargetMeta{}, err
	}
	selection, err := repository.Select(candidate.TargetPath)
	if err != nil {
		return tuf.TargetMeta{}, err
	}
	custom := selection.Target.Custom
	if custom.Package != candidate.Package || custom.Version != candidate.Version || custom.ArchiveSHA256 != candidate.ArchiveSHA256 {
		return tuf.TargetMeta{}, fmt.Errorf("signed target changed after resolution")
	}
	return selection.Target, nil
}

func (runtime *registryRuntime) load(ctx context.Context, origin string, offline bool) (*tuf.Repository, error) {
	runtime.mu.Lock()
	defer runtime.mu.Unlock()
	if existing := runtime.verified[origin]; existing != nil {
		return existing, nil
	}
	spec, ok := runtime.specs[origin]
	if !ok {
		return nil, fmt.Errorf("registry origin %s has no immutable runtime configuration", origin)
	}
	directory := filepath.Join(runtime.metadataRoot, originDigest(origin))
	store := &tuf.FileStore{Path: filepath.Join(directory, "trusted-state.json")}
	transaction, err := store.Begin(ctx)
	if err != nil {
		return nil, fmt.Errorf("lock registry trust transaction: %w", err)
	}
	defer transaction.Close()
	state, loadErr := transaction.Load()
	hasState := loadErr == nil
	if loadErr != nil && !errors.Is(loadErr, os.ErrNotExist) {
		return nil, loadErr
	}
	verifier, err := tuf.New(tuf.Config{Environment: spec.Environment, RepositoryID: spec.RepositoryID, RegistryOrigin: spec.Origin, Store: transaction})
	if err != nil {
		return nil, err
	}
	if !hasState {
		if offline {
			return nil, resolver.ErrOfflineDataUnavailable
		}
		if spec.TrustedRoot == "" || spec.TrustedRootSHA256 == "" {
			return nil, fmt.Errorf("trusted root for registry %q is not provisioned; pass --trusted-root %s=PATH and --trusted-root-sha256 %s=DIGEST", spec.Alias, spec.Alias, spec.Alias)
		}
		root, err := os.ReadFile(spec.TrustedRoot)
		if err != nil {
			return nil, fmt.Errorf("read trusted root for %s: %w", spec.Alias, err)
		}
		if err := verifier.BootstrapRoot(root, spec.TrustedRootSHA256); err != nil {
			return nil, err
		}
		state, err = transaction.Load()
		if err != nil {
			return nil, err
		}
	}
	var set tuf.MetadataSet
	if offline {
		set, err = loadCachedMetadata(directory)
		if err != nil {
			return nil, resolver.ErrOfflineDataUnavailable
		}
		repository, err := verifier.VerifyCached(set)
		if err != nil {
			return nil, err
		}
		if err := transaction.Commit(); err != nil {
			return nil, err
		}
		runtime.verified[origin] = repository
		return repository, nil
	}
	// A normal fetch with an enforceable lock does not need network metadata
	// when the complete locally cached set is still valid. Missing, expired, or
	// otherwise invalid cached metadata falls through to the online refresh.
	if runtime.preferCached {
		if cached, cachedErr := loadCachedMetadata(directory); cachedErr == nil {
			if repository, verifyErr := verifier.VerifyCached(cached); verifyErr == nil {
				if err := transaction.Commit(); err != nil {
					return nil, err
				}
				runtime.verified[origin] = repository
				return repository, nil
			}
		}
	}
	client := transport.NewClient(runtime.policy)
	if err := updateRoots(ctx, client, verifier, spec, int(state.Versions["root"])); err != nil {
		return nil, err
	}
	set, names, tempDir, err := fetchMetadata(ctx, client, spec, directory, runtime.policy)
	if err != nil {
		return nil, err
	}
	defer os.RemoveAll(tempDir)
	repository, err := verifier.Refresh(set)
	if err != nil {
		return nil, err
	}
	if err := commitMetadataAndTrustedState(directory, set, names, tempDir, transaction); err != nil {
		return nil, err
	}
	runtime.verified[origin] = repository
	return repository, nil
}

func defaultRegistrySpec(alias, origin string) (registrySpec, error) {
	spec := registrySpec{Alias: alias, Origin: origin}
	switch origin {
	case "https://seen.dev.yousef.codes/packages":
		spec.Environment = "development"
		spec.RepositoryID = "seen-dev-registry-v1"
	case "https://seen.yousef.codes/packages":
		spec.Environment = "production"
		spec.RepositoryID = "seen-prod-registry-v1"
	}
	return spec, nil
}

type statusError struct {
	Status int
	URL    string
}

func (err *statusError) Error() string {
	return fmt.Sprintf("registry returned HTTP %d for %s", err.Status, err.URL)
}

func fetchBounded(ctx context.Context, client *http.Client, url string) ([]byte, error) {
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return nil, err
	}
	request.Header.Set("Accept", "application/vnd.seen.tuf+json")
	request.Header.Set("Accept-Encoding", "identity")
	response, err := client.Do(request)
	if err != nil {
		return nil, err
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		return nil, &statusError{Status: response.StatusCode, URL: url}
	}
	if response.ContentLength > maxMetadataBytes {
		return nil, fmt.Errorf("metadata exceeds byte limit")
	}
	if response.Header.Get("Content-Encoding") != "" && response.Header.Get("Content-Encoding") != "identity" {
		return nil, fmt.Errorf("compressed metadata response is forbidden")
	}
	content, err := io.ReadAll(io.LimitReader(response.Body, maxMetadataBytes+1))
	if err != nil {
		return nil, err
	}
	if int64(len(content)) > maxMetadataBytes {
		return nil, fmt.Errorf("metadata exceeds byte limit")
	}
	return content, nil
}

func updateRoots(ctx context.Context, client *http.Client, verifier *tuf.Verifier, spec registrySpec, current int) error {
	for steps := 0; steps < 32; steps++ {
		next := current + 1
		url := metadataURL(spec.Origin, strconv.Itoa(next)+".root.json")
		raw, err := fetchBounded(ctx, client, url)
		if err != nil {
			var status *statusError
			if errors.As(err, &status) && status.Status == http.StatusNotFound {
				return nil
			}
			return err
		}
		if err := verifier.UpdateRoot(raw); err != nil {
			return err
		}
		current = next
	}
	return fmt.Errorf("more than 32 root rotations require a separate update run")
}

type metadataNames struct{ Snapshot, Targets, Releases, Security string }

func fetchMetadata(ctx context.Context, client *http.Client, spec registrySpec, directory string, policy transport.Policy) (tuf.MetadataSet, metadataNames, string, error) {
	timestamp, err := fetchBounded(ctx, client, metadataURL(spec.Origin, "timestamp.json"))
	if err != nil {
		return tuf.MetadataSet{}, metadataNames{}, "", err
	}
	timestampSigned, err := peekTimestamp(timestamp)
	if err != nil {
		return tuf.MetadataSet{}, metadataNames{}, "", err
	}
	snapshotMeta, ok := timestampSigned.Meta["snapshot.json"]
	if !ok {
		return tuf.MetadataSet{}, metadataNames{}, "", fmt.Errorf("timestamp lacks snapshot metadata")
	}
	names := metadataNames{Snapshot: strconv.FormatInt(snapshotMeta.Version, 10) + ".snapshot.json"}
	tempDir, err := os.MkdirTemp(directory, ".metadata-fetch-*")
	if err != nil {
		return tuf.MetadataSet{}, metadataNames{}, "", err
	}
	fail := func(err error) (tuf.MetadataSet, metadataNames, string, error) {
		os.RemoveAll(tempDir)
		return tuf.MetadataSet{}, metadataNames{}, "", err
	}
	snapshot, err := downloadMetadata(ctx, client, spec, names.Snapshot, snapshotMeta, tempDir, policy)
	if err != nil {
		return fail(err)
	}
	snapshotSigned, err := peekSnapshot(snapshot)
	if err != nil {
		return fail(err)
	}
	documents := []struct {
		logical     string
		name        *string
		destination *[]byte
	}{{"targets.json", &names.Targets, nil}, {"releases.json", &names.Releases, nil}, {"security.json", &names.Security, nil}}
	set := tuf.MetadataSet{Timestamp: timestamp, Snapshot: snapshot}
	for _, document := range documents {
		meta, ok := snapshotSigned.Meta[document.logical]
		if !ok {
			return fail(fmt.Errorf("snapshot lacks %s", document.logical))
		}
		*document.name = strconv.FormatInt(meta.Version, 10) + "." + strings.TrimSuffix(document.logical, ".json") + ".json"
		raw, err := downloadMetadata(ctx, client, spec, *document.name, meta, tempDir, policy)
		if err != nil {
			return fail(err)
		}
		switch document.logical {
		case "targets.json":
			set.Targets = raw
		case "releases.json":
			set.Releases = raw
		case "security.json":
			set.Security = raw
		}
	}
	return set, names, tempDir, nil
}

func downloadMetadata(ctx context.Context, client *http.Client, spec registrySpec, name string, meta tuf.FileMeta, tempDir string, policy transport.Policy) ([]byte, error) {
	digest, ok := meta.Hashes["sha256"]
	if !ok || len(meta.Hashes) != 1 {
		return nil, fmt.Errorf("metadata %s lacks exact SHA-256 binding", name)
	}
	destination := filepath.Join(tempDir, name)
	_, err := transport.Download(ctx, client, metadataURL(spec.Origin, name), destination, transport.Expectation{SHA256: digest, Length: meta.Length, Origin: spec.Origin}, policy)
	if err != nil {
		return nil, err
	}
	return os.ReadFile(destination)
}
func metadataURL(origin, name string) string { return origin + "/api/v1/metadata/" + name }
func blobURL(origin, digest string) string   { return origin + "/api/v1/blobs/sha256/" + digest }

func peekTimestamp(raw []byte) (tuf.TimestampSigned, error) {
	var value tuf.TimestampSigned
	return value, peekSigned(raw, &value)
}
func peekSnapshot(raw []byte) (tuf.SnapshotSigned, error) {
	var value tuf.SnapshotSigned
	return value, peekSigned(raw, &value)
}
func peekSigned(raw []byte, destination any) error {
	var envelope tuf.Envelope
	if err := decodeOne(raw, &envelope); err != nil {
		return err
	}
	return decodeOne(envelope.Signed, destination)
}
func decodeOne(raw []byte, destination any) error {
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(destination); err != nil {
		return err
	}
	var extra any
	if err := decoder.Decode(&extra); !errors.Is(err, io.EOF) {
		return fmt.Errorf("JSON has trailing data")
	}
	return nil
}

type metadataWriter func(string, []byte) error

func commitMetadata(directory string, set tuf.MetadataSet, names metadataNames, tempDir string) error {
	return commitMetadataWithWriter(directory, set, names, tempDir, writePrivateAtomic)
}

func commitMetadataWithWriter(directory string, set tuf.MetadataSet, names metadataNames, _ string, write metadataWriter) error {
	if err := os.MkdirAll(directory, 0o700); err != nil {
		return err
	}
	// Versioned metadata is installed and synced first. timestamp.json is the
	// only cache pointer, so replacing it last makes every crash resolve to
	// either the complete previous generation or the complete new generation.
	documents := []struct {
		name    string
		content []byte
	}{{names.Snapshot, set.Snapshot}, {names.Targets, set.Targets}, {names.Releases, set.Releases}, {names.Security, set.Security}, {"timestamp.json", set.Timestamp}}
	for _, document := range documents {
		if err := write(filepath.Join(directory, document.name), document.content); err != nil {
			return err
		}
	}
	return nil
}

func commitMetadataAndTrustedState(directory string, set tuf.MetadataSet, names metadataNames, tempDir string, transaction *tuf.StateTransaction) error {
	return commitMetadataAndTrustedStateWithWriter(directory, set, names, tempDir, transaction, writePrivateAtomic)
}

func commitMetadataAndTrustedStateWithWriter(directory string, set tuf.MetadataSet, names metadataNames, tempDir string, transaction *tuf.StateTransaction, write metadataWriter) error {
	if err := commitMetadataWithWriter(directory, set, names, tempDir, write); err != nil {
		return err
	}
	return transaction.Commit()
}
func loadCachedMetadata(directory string) (tuf.MetadataSet, error) {
	timestamp, err := os.ReadFile(filepath.Join(directory, "timestamp.json"))
	if err != nil {
		return tuf.MetadataSet{}, err
	}
	timestampSigned, err := peekTimestamp(timestamp)
	if err != nil {
		return tuf.MetadataSet{}, err
	}
	snapshotMeta := timestampSigned.Meta["snapshot.json"]
	snapshot, err := os.ReadFile(filepath.Join(directory, strconv.FormatInt(snapshotMeta.Version, 10)+".snapshot.json"))
	if err != nil {
		return tuf.MetadataSet{}, err
	}
	snapshotSigned, err := peekSnapshot(snapshot)
	if err != nil {
		return tuf.MetadataSet{}, err
	}
	read := func(logical string) ([]byte, error) {
		meta, ok := snapshotSigned.Meta[logical]
		if !ok {
			return nil, fmt.Errorf("cached snapshot lacks %s", logical)
		}
		name := strconv.FormatInt(meta.Version, 10) + "." + strings.TrimSuffix(logical, ".json") + ".json"
		return os.ReadFile(filepath.Join(directory, name))
	}
	targets, err := read("targets.json")
	if err != nil {
		return tuf.MetadataSet{}, err
	}
	releases, err := read("releases.json")
	if err != nil {
		return tuf.MetadataSet{}, err
	}
	security, err := read("security.json")
	if err != nil {
		return tuf.MetadataSet{}, err
	}
	return tuf.MetadataSet{Timestamp: timestamp, Snapshot: snapshot, Targets: targets, Releases: releases, Security: security}, nil
}
func writePrivateAtomic(filename string, content []byte) error {
	if err := os.MkdirAll(filepath.Dir(filename), 0o700); err != nil {
		return err
	}
	temp, err := os.CreateTemp(filepath.Dir(filename), ".metadata-*")
	if err != nil {
		return err
	}
	name := temp.Name()
	ok := false
	defer func() {
		if !ok {
			_ = os.Remove(name)
		}
	}()
	if err := temp.Chmod(0o600); err != nil {
		_ = temp.Close()
		return err
	}
	if _, err := temp.Write(content); err != nil {
		_ = temp.Close()
		return err
	}
	if err := temp.Sync(); err != nil {
		_ = temp.Close()
		return err
	}
	if err := temp.Close(); err != nil {
		return err
	}
	if err := atomicfile.Replace(name, filename); err != nil {
		return err
	}
	if err := atomicfile.SyncDir(filepath.Dir(filename)); err != nil {
		return err
	}
	ok = true
	return nil
}
func originDigest(origin string) string {
	sum := sha256.Sum256([]byte(origin))
	return hex.EncodeToString(sum[:])
}
func sortStrings(values []string) {
	for i := 1; i < len(values); i++ {
		for j := i; j > 0 && values[j] < values[j-1]; j-- {
			values[j], values[j-1] = values[j-1], values[j]
		}
	}
}
