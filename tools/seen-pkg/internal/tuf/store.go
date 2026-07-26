package tuf

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"os"
	"path/filepath"
	"strings"
	"sync"

	"github.com/codeyousef/seen/tools/seen-pkg/internal/atomicfile"
)

// TrustedState is committed only after a complete metadata transaction.
type TrustedState struct {
	Version      int               `json:"version"`
	Root         json.RawMessage   `json:"root"`
	Versions     map[string]int64  `json:"versions"`
	Expires      map[string]string `json:"expires"`
	Fingerprints map[string]string `json:"fingerprints"`
}

// TrustedIdentity is the immutable signing identity carried by the
// threshold-signed root in local trusted state.
type TrustedIdentity struct {
	Environment  string
	RepositoryID string
}

// IdentityFromTrustedState verifies the stored root envelope before returning
// its signing identity. Callers may use this to reconstruct verifier
// configuration, but must still supply the manifest-bound registry origin to
// New; the network and manifest never get to choose this identity.
func IdentityFromTrustedState(state TrustedState) (TrustedIdentity, error) {
	if state.Version != 1 || len(state.Root) == 0 {
		return TrustedIdentity{}, failure("signing_trusted_state_invalid", errors.New("trusted state version or root is invalid"))
	}
	envelope, root, canonical, err := parseRoot(state.Root)
	if err != nil {
		return TrustedIdentity{}, failure("signing_trusted_state_invalid", err)
	}
	identity := TrustedIdentity{Environment: root.Environment, RepositoryID: root.RepositoryID}
	if identity.Environment != "development" && identity.Environment != "production" {
		return TrustedIdentity{}, failure("signing_trusted_state_invalid", errors.New("trusted root environment is invalid"))
	}
	if !repositoryPattern.MatchString(identity.RepositoryID) ||
		(identity.Environment == "production" && !strings.HasPrefix(identity.RepositoryID, "seen-prod-")) ||
		(identity.Environment == "development" && !strings.HasPrefix(identity.RepositoryID, "seen-dev-")) {
		return TrustedIdentity{}, failure("signing_trusted_state_invalid", errors.New("trusted root repository ID is invalid"))
	}
	verifier := &Verifier{config: Config{Environment: identity.Environment, RepositoryID: identity.RepositoryID}}
	if err := verifier.validateRoot(root, false); err != nil {
		return TrustedIdentity{}, failure("signing_trusted_state_invalid", err)
	}
	if err := verifyThreshold(envelope, root.Keys, root.Roles["root"], "root"); err != nil {
		return TrustedIdentity{}, failure("signing_trusted_state_invalid", err)
	}
	if saved := state.Versions["root"]; saved != root.Version {
		return TrustedIdentity{}, failure("signing_trusted_state_invalid", errors.New("root version counter mismatch"))
	}
	if saved := state.Fingerprints["root"]; saved == "" || saved != digest(canonical) {
		return TrustedIdentity{}, failure("signing_trusted_state_invalid", errors.New("root fingerprint mismatch"))
	}
	return identity, nil
}

type StateStore interface {
	Load() (TrustedState, error)
	Save(TrustedState) error
}

// MemoryStore is useful for embedded clients and deterministic tests.
type MemoryStore struct {
	mu    sync.Mutex
	state *TrustedState
}

func (m *MemoryStore) Load() (TrustedState, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.state == nil {
		return TrustedState{}, os.ErrNotExist
	}
	return cloneState(*m.state), nil
}

func (m *MemoryStore) Save(state TrustedState) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	cloned := cloneState(state)
	m.state = &cloned
	return nil
}

func cloneState(state TrustedState) TrustedState {
	cloned := state
	cloned.Root = append(json.RawMessage(nil), state.Root...)
	cloned.Versions = cloneMap(state.Versions)
	cloned.Expires = cloneMap(state.Expires)
	cloned.Fingerprints = cloneMap(state.Fingerprints)
	return cloned
}

func cloneMap[K comparable, V any](source map[K]V) map[K]V {
	result := make(map[K]V, len(source))
	for key, value := range source {
		result[key] = value
	}
	return result
}

// FileStore persists trust state with an fsync + same-directory atomic rename.
type FileStore struct {
	Path string
	mu   sync.Mutex
}

// StateTransaction holds the cross-process lock for one trusted-state path and
// buffers verifier writes until Commit. Callers can therefore install every
// metadata file first, commit trusted rollback counters last, and drop the
// pending state simply by closing an unsuccessful transaction.
type StateTransaction struct {
	store   *FileStore
	lock    *fileLock
	mu      sync.Mutex
	pending *TrustedState
	closed  bool
}

// Begin acquires the trusted-state transaction lock. The lock covers the first
// Load through the final Commit, so every competing process reloads state only
// after the previous writer has committed or rolled back.
func (s *FileStore) Begin(ctx context.Context) (*StateTransaction, error) {
	lock, err := acquireFileLock(ctx, s.Path+".lock")
	if err != nil {
		return nil, err
	}
	return &StateTransaction{store: s, lock: lock}, nil
}

func (tx *StateTransaction) Load() (TrustedState, error) {
	tx.mu.Lock()
	defer tx.mu.Unlock()
	if tx.closed {
		return TrustedState{}, errors.New("trusted-state transaction is closed")
	}
	if tx.pending != nil {
		return cloneState(*tx.pending), nil
	}
	return tx.store.Load()
}

func (tx *StateTransaction) Save(state TrustedState) error {
	tx.mu.Lock()
	defer tx.mu.Unlock()
	if tx.closed {
		return errors.New("trusted-state transaction is closed")
	}
	state.Version = 1
	cloned := cloneState(state)
	tx.pending = &cloned
	return nil
}

// Commit durably replaces trusted state. It must be called only after all
// metadata referenced by the pending state has been installed and synced.
func (tx *StateTransaction) Commit() error {
	tx.mu.Lock()
	defer tx.mu.Unlock()
	if tx.closed {
		return errors.New("trusted-state transaction is closed")
	}
	if tx.pending == nil {
		return nil
	}
	if err := tx.store.Save(*tx.pending); err != nil {
		return failure("signing_trusted_state_write_failed", err)
	}
	return nil
}

// Close releases the cross-process lock without implicitly committing.
func (tx *StateTransaction) Close() error {
	tx.mu.Lock()
	if tx.closed {
		tx.mu.Unlock()
		return nil
	}
	tx.closed = true
	lock := tx.lock
	tx.mu.Unlock()
	return lock.Close()
}

func (s *FileStore) Load() (TrustedState, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	pathInfo, err := os.Lstat(s.Path)
	if err != nil {
		return TrustedState{}, err
	}
	if !pathInfo.Mode().IsRegular() || pathInfo.Mode()&os.ModeSymlink != 0 || pathInfo.Mode().Perm()&0o077 != 0 || pathInfo.Size() > 4*1024*1024 {
		return TrustedState{}, errors.New("trusted state must be a private bounded regular file")
	}
	f, err := os.Open(s.Path)
	if err != nil {
		return TrustedState{}, err
	}
	defer f.Close()
	info, err := f.Stat()
	if err != nil || !info.Mode().IsRegular() {
		return TrustedState{}, errors.New("trusted state is not a regular file")
	}
	decoder := json.NewDecoder(io.LimitReader(f, 4*1024*1024))
	decoder.DisallowUnknownFields()
	var state TrustedState
	if err := decoder.Decode(&state); err != nil {
		return TrustedState{}, err
	}
	var extra any
	if err := decoder.Decode(&extra); !errors.Is(err, io.EOF) {
		return TrustedState{}, errors.New("trusted state has trailing JSON")
	}
	if state.Version != 1 || len(state.Root) == 0 {
		return TrustedState{}, errors.New("trusted state version or root is invalid")
	}
	return state, nil
}

func (s *FileStore) Save(state TrustedState) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	state.Version = 1
	data, err := json.Marshal(state)
	if err != nil {
		return err
	}
	data = append(data, '\n')
	parent := filepath.Dir(s.Path)
	if err := os.MkdirAll(parent, 0o700); err != nil {
		return err
	}
	if info, err := os.Lstat(s.Path); err == nil && info.Mode()&os.ModeSymlink != 0 {
		return errors.New("trusted state path is a symlink")
	} else if err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}
	tmp, err := os.CreateTemp(parent, ".tuf-state-*")
	if err != nil {
		return err
	}
	tmpName := tmp.Name()
	committed := false
	defer func() {
		_ = tmp.Close()
		if !committed {
			_ = os.Remove(tmpName)
		}
	}()
	if err := tmp.Chmod(0o600); err != nil {
		return err
	}
	if _, err := io.Copy(tmp, bytes.NewReader(data)); err != nil {
		return err
	}
	if err := tmp.Sync(); err != nil {
		return err
	}
	if err := tmp.Close(); err != nil {
		return err
	}
	if err := atomicfile.Replace(tmpName, s.Path); err != nil {
		return err
	}
	committed = true
	return atomicfile.SyncDir(parent)
}
