package tuf

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"
)

const (
	transactionHelperMode = "SEEN_TUF_TRANSACTION_HELPER"
	transactionHelperPath = "SEEN_TUF_TRANSACTION_PATH"
)

func transactionState(timestampVersion int64) TrustedState {
	return TrustedState{
		Version:      1,
		Root:         json.RawMessage(`{}`),
		Versions:     map[string]int64{"timestamp": timestampVersion},
		Expires:      map[string]string{},
		Fingerprints: map[string]string{},
	}
}

func TestFileStoreTransactionDefersStateUntilCommit(t *testing.T) {
	store := &FileStore{Path: filepath.Join(t.TempDir(), "trusted-state.json")}
	if err := store.Save(transactionState(1)); err != nil {
		t.Fatal(err)
	}
	transaction, err := store.Begin(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	defer transaction.Close()
	state, err := transaction.Load()
	if err != nil {
		t.Fatal(err)
	}
	state.Versions["timestamp"] = 2
	if err := transaction.Save(state); err != nil {
		t.Fatal(err)
	}
	before, err := store.Load()
	if err != nil {
		t.Fatal(err)
	}
	if before.Versions["timestamp"] != 1 {
		t.Fatalf("pending trusted state escaped before commit: %+v", before.Versions)
	}
	if err := transaction.Commit(); err != nil {
		t.Fatal(err)
	}
	after, err := store.Load()
	if err != nil {
		t.Fatal(err)
	}
	if after.Versions["timestamp"] != 2 {
		t.Fatalf("committed timestamp version = %d, want 2", after.Versions["timestamp"])
	}
}

func TestFileStoreTransactionSerializesStaleWritersAcrossProcesses(t *testing.T) {
	path := filepath.Join(t.TempDir(), "trusted-state.json")
	store := &FileStore{Path: path}
	if err := store.Save(transactionState(1)); err != nil {
		t.Fatal(err)
	}
	first, err := store.Begin(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	firstClosed := false
	defer func() {
		if !firstClosed {
			_ = first.Close()
		}
	}()

	probe := exec.Command(os.Args[0], "-test.run=^TestFileStoreTransactionProcessHelper$")
	probe.Env = append(os.Environ(), transactionHelperMode+"=probe", transactionHelperPath+"="+path)
	if output, err := probe.CombinedOutput(); err != nil {
		t.Fatalf("cross-process lock probe: %v\n%s", err, output)
	}

	second := exec.Command(os.Args[0], "-test.run=^TestFileStoreTransactionProcessHelper$")
	second.Env = append(os.Environ(), transactionHelperMode+"=increment", transactionHelperPath+"="+path)
	var secondOutput bytes.Buffer
	second.Stdout = &secondOutput
	second.Stderr = &secondOutput
	if err := second.Start(); err != nil {
		t.Fatal(err)
	}
	state, err := first.Load()
	if err != nil {
		t.Fatal(err)
	}
	state.Versions["timestamp"] = 2
	if err := first.Save(state); err != nil {
		t.Fatal(err)
	}
	if err := first.Commit(); err != nil {
		t.Fatal(err)
	}
	if err := first.Close(); err != nil {
		t.Fatal(err)
	}
	firstClosed = true
	if err := second.Wait(); err != nil {
		t.Fatalf("second transaction: %v\n%s", err, secondOutput.Bytes())
	}
	final, err := store.Load()
	if err != nil {
		t.Fatal(err)
	}
	if final.Versions["timestamp"] != 3 {
		t.Fatalf("stale writer overwrote trusted version: got %d, want 3", final.Versions["timestamp"])
	}
}

func TestFileStoreTransactionProcessHelper(t *testing.T) {
	mode := os.Getenv(transactionHelperMode)
	if mode == "" {
		t.Skip("subprocess helper")
	}
	store := &FileStore{Path: os.Getenv(transactionHelperPath)}
	switch mode {
	case "probe":
		ctx, cancel := context.WithTimeout(context.Background(), 150*time.Millisecond)
		defer cancel()
		transaction, err := store.Begin(ctx)
		if transaction != nil {
			_ = transaction.Close()
			t.Fatal("acquired a trusted-state transaction held by another process")
		}
		if !errors.Is(err, context.DeadlineExceeded) {
			t.Fatalf("blocked transaction error = %v, want deadline exceeded", err)
		}
	case "increment":
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		transaction, err := store.Begin(ctx)
		if err != nil {
			t.Fatal(err)
		}
		defer transaction.Close()
		state, err := transaction.Load()
		if err != nil {
			t.Fatal(err)
		}
		state.Versions["timestamp"]++
		if err := transaction.Save(state); err != nil {
			t.Fatal(err)
		}
		if err := transaction.Commit(); err != nil {
			t.Fatal(err)
		}
	default:
		t.Fatalf("unknown helper mode %q", mode)
	}
}
