package transport

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func digestOf(b []byte) string {
	h := sha256.Sum256(b)
	return hex.EncodeToString(h[:])
}

func errorCode(t *testing.T, err error) string {
	t.Helper()
	var e *Error
	if !errors.As(err, &e) {
		t.Fatalf("expected *Error, got %T: %v", err, err)
	}
	return e.Code
}

func TestDownloadVerifiesAndPublishesAtomically(t *testing.T) {
	body := []byte("signed package bytes")
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if got := r.Header.Get("Accept-Encoding"); got != "identity" {
			t.Errorf("Accept-Encoding = %q", got)
		}
		w.Header().Set("Content-Encoding", "identity")
		_, _ = w.Write(body)
	}))
	defer server.Close()

	p := DefaultPolicy()
	p.AllowInsecureLoopback = true
	dest := filepath.Join(t.TempDir(), "blob")
	expect := Expectation{SHA256: digestOf(body), Length: int64(len(body)), Origin: server.URL + "/packages"}
	result, err := Download(context.Background(), NewClient(p), server.URL+"/packages/a/b", dest, expect, p)
	if err != nil {
		t.Fatal(err)
	}
	if result.SHA256 != expect.SHA256 || result.Length != expect.Length {
		t.Fatalf("unexpected result: %+v", result)
	}
	got, err := os.ReadFile(dest)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != string(body) {
		t.Fatalf("downloaded bytes = %q", got)
	}
	info, err := os.Stat(dest)
	if err != nil {
		t.Fatal(err)
	}
	if info.Mode().Perm()&0o222 != 0 {
		t.Fatalf("published blob is writable: %v", info.Mode())
	}
}

func TestDownloadRejectsDigestWithoutDestination(t *testing.T) {
	body := []byte("attacker bytes")
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) { _, _ = w.Write(body) }))
	defer server.Close()
	p := DefaultPolicy()
	p.AllowInsecureLoopback = true
	dest := filepath.Join(t.TempDir(), "blob")
	err := func() error {
		_, err := Download(context.Background(), NewClient(p), server.URL+"/packages/blob", dest, Expectation{
			SHA256: digestOf([]byte("expected")), Length: int64(len(body)), Origin: server.URL + "/packages",
		}, p)
		return err
	}()
	if got := errorCode(t, err); got != "signing_target_hash_mismatch" {
		t.Fatalf("code = %q", got)
	}
	if _, err := os.Stat(dest); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("destination exists after failure: %v", err)
	}
}

func TestDownloadRejectsCrossOriginRedirect(t *testing.T) {
	body := []byte("package")
	target := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) { _, _ = w.Write(body) }))
	defer target.Close()
	redirector := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		http.Redirect(w, r, target.URL+"/packages/blob", http.StatusFound)
	}))
	defer redirector.Close()
	p := DefaultPolicy()
	p.AllowInsecureLoopback = true
	_, err := Download(context.Background(), NewClient(p), redirector.URL+"/packages/blob", filepath.Join(t.TempDir(), "blob"), Expectation{
		SHA256: digestOf(body), Length: int64(len(body)), Origin: redirector.URL + "/packages",
	}, p)
	if got := errorCode(t, err); got != "transport_cross_origin_redirect" {
		t.Fatalf("code = %q", got)
	}
}

func TestDownloadDeadlineAndLengthLimits(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		time.Sleep(100 * time.Millisecond)
		_, _ = w.Write([]byte("x"))
	}))
	defer server.Close()
	p := DefaultPolicy()
	p.AllowInsecureLoopback = true
	p.RequestTimeout = 20 * time.Millisecond
	_, err := Download(context.Background(), NewClient(p), server.URL+"/packages/blob", filepath.Join(t.TempDir(), "blob"), Expectation{
		SHA256: digestOf([]byte("x")), Length: 1, Origin: server.URL + "/packages",
	}, p)
	if got := errorCode(t, err); got != "transport_request_failed" {
		t.Fatalf("deadline code = %q", got)
	}

	p.MaxBytes = 1
	_, err = Download(context.Background(), nil, server.URL+"/packages/blob", filepath.Join(t.TempDir(), "blob"), Expectation{
		SHA256: digestOf([]byte("xx")), Length: 2, Origin: server.URL + "/packages",
	}, p)
	if got := errorCode(t, err); got != "transport_invalid_length" {
		t.Fatalf("length code = %q", got)
	}
}

func TestProductionPolicyRequiresHTTPS(t *testing.T) {
	body := []byte("x")
	_, err := Download(context.Background(), nil, "http://127.0.0.1/packages/blob", filepath.Join(t.TempDir(), "blob"), Expectation{
		SHA256: digestOf(body), Length: 1, Origin: "http://127.0.0.1/packages",
	}, DefaultPolicy())
	if got := errorCode(t, err); got != "transport_https_required" {
		t.Fatalf("code = %q", got)
	}
}
