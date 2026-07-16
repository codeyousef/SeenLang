// Package transport provides the deliberately small HTTP surface used by the
// package client. Downloads are bounded, content-addressed, and never exposed
// at their destination until all signed length and digest checks have passed.
package transport

import (
	"context"
	"crypto/sha256"
	"crypto/tls"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"time"
)

const defaultMaxBytes int64 = 25 * 1024 * 1024

// Policy controls the network and resource limits applied to a download.
type Policy struct {
	MaxBytes              int64
	RequestTimeout        time.Duration
	DialTimeout           time.Duration
	TLSHandshakeTimeout   time.Duration
	ResponseHeaderTimeout time.Duration
	MaxRedirects          int
	// AllowInsecureLoopback exists only so httptest can exercise the complete
	// downloader. Production callers must leave it false.
	AllowInsecureLoopback bool
}

// DefaultPolicy returns conservative defaults for small source archives.
func DefaultPolicy() Policy {
	return Policy{
		MaxBytes:              defaultMaxBytes,
		RequestTimeout:        2 * time.Minute,
		DialTimeout:           10 * time.Second,
		TLSHandshakeTimeout:   10 * time.Second,
		ResponseHeaderTimeout: 20 * time.Second,
		MaxRedirects:          3,
	}
}

// Expectation is derived from already verified signed metadata.
type Expectation struct {
	SHA256 string
	Length int64
	// Origin is the byte-exact configured registry base URL, including its
	// path prefix (for example https://seen.yousef.codes/packages).
	Origin string
}

// Result describes a verified, atomically published file.
type Result struct {
	Path   string
	SHA256 string
	Length int64
}

// Error is a stable, machine-readable transport failure.
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

func normalizedPolicy(p Policy) Policy {
	d := DefaultPolicy()
	if p.MaxBytes <= 0 {
		p.MaxBytes = d.MaxBytes
	}
	if p.RequestTimeout <= 0 {
		p.RequestTimeout = d.RequestTimeout
	}
	if p.DialTimeout <= 0 {
		p.DialTimeout = d.DialTimeout
	}
	if p.TLSHandshakeTimeout <= 0 {
		p.TLSHandshakeTimeout = d.TLSHandshakeTimeout
	}
	if p.ResponseHeaderTimeout <= 0 {
		p.ResponseHeaderTimeout = d.ResponseHeaderTimeout
	}
	if p.MaxRedirects <= 0 {
		p.MaxRedirects = d.MaxRedirects
	}
	return p
}

// NewClient constructs a client that does not transparently decompress signed
// bytes and only follows redirects within the byte-exact original origin.
func NewClient(policy Policy) *http.Client {
	p := normalizedPolicy(policy)
	dialer := &net.Dialer{Timeout: p.DialTimeout, KeepAlive: 30 * time.Second}
	tr := &http.Transport{
		Proxy:                 http.ProxyFromEnvironment,
		DialContext:           dialer.DialContext,
		ForceAttemptHTTP2:     true,
		DisableCompression:    true,
		TLSHandshakeTimeout:   p.TLSHandshakeTimeout,
		ResponseHeaderTimeout: p.ResponseHeaderTimeout,
		TLSClientConfig:       &tls.Config{MinVersion: tls.VersionTLS12},
	}
	return &http.Client{
		Transport: tr,
		Timeout:   p.RequestTimeout,
		CheckRedirect: func(req *http.Request, via []*http.Request) error {
			if len(via) >= p.MaxRedirects {
				return failure("transport_redirect_limit", fmt.Errorf("more than %d redirects", p.MaxRedirects))
			}
			if len(via) == 0 || !sameOrigin(via[0].URL, req.URL) {
				return failure("transport_cross_origin_redirect", errors.New("redirect changed origin"))
			}
			if req.URL.User != nil || req.URL.Fragment != "" {
				return failure("transport_invalid_redirect", errors.New("userinfo and fragments are forbidden"))
			}
			return nil
		},
	}
}

func sameOrigin(a, b *url.URL) bool {
	return a != nil && b != nil && a.Scheme == b.Scheme && a.Host == b.Host
}

func validLoopbackHTTP(u *url.URL) bool {
	if u.Scheme != "http" {
		return false
	}
	host := u.Hostname()
	ip := net.ParseIP(host)
	return strings.EqualFold(host, "localhost") || (ip != nil && ip.IsLoopback())
}

func validateRequestURL(rawURL string, expectation Expectation, p Policy) (*url.URL, *url.URL, error) {
	u, err := url.Parse(rawURL)
	if err != nil || !u.IsAbs() || u.Opaque != "" {
		return nil, nil, failure("transport_invalid_url", errors.New("absolute hierarchical URL required"))
	}
	if u.User != nil || u.Fragment != "" {
		return nil, nil, failure("transport_invalid_url", errors.New("userinfo and fragments are forbidden"))
	}
	if u.Scheme != "https" && !(p.AllowInsecureLoopback && validLoopbackHTTP(u)) {
		return nil, nil, failure("transport_https_required", errors.New("HTTPS is required"))
	}
	origin, err := url.Parse(expectation.Origin)
	if err != nil || !origin.IsAbs() || origin.RawQuery != "" || origin.Fragment != "" || origin.User != nil {
		return nil, nil, failure("transport_invalid_origin", errors.New("invalid configured registry origin"))
	}
	if origin.String() != expectation.Origin || origin.Path == "" || strings.HasSuffix(origin.Path, "/") {
		return nil, nil, failure("transport_invalid_origin", errors.New("registry origin must be canonical and have no trailing slash"))
	}
	if !sameOrigin(u, origin) || (u.Path != origin.Path && !strings.HasPrefix(u.Path, origin.Path+"/")) {
		return nil, nil, failure("transport_origin_mismatch", errors.New("download URL is outside configured registry origin"))
	}
	return u, origin, nil
}

func validateExpectation(e Expectation, p Policy) error {
	if len(e.SHA256) != sha256.Size*2 || strings.ToLower(e.SHA256) != e.SHA256 {
		return failure("transport_invalid_digest", errors.New("lowercase SHA-256 digest required"))
	}
	if _, err := hex.DecodeString(e.SHA256); err != nil {
		return failure("transport_invalid_digest", err)
	}
	if e.Length <= 0 || e.Length > p.MaxBytes {
		return failure("transport_invalid_length", fmt.Errorf("signed length %d is outside limit", e.Length))
	}
	return nil
}

// Download fetches rawURL and publishes it at destination only after exact
// signed length and SHA-256 verification. Existing destinations are accepted
// only when they independently revalidate.
func Download(ctx context.Context, client *http.Client, rawURL, destination string, expectation Expectation, policy Policy) (Result, error) {
	p := normalizedPolicy(policy)
	if err := validateExpectation(expectation, p); err != nil {
		return Result{}, err
	}
	_, origin, err := validateRequestURL(rawURL, expectation, p)
	if err != nil {
		return Result{}, err
	}
	if client == nil {
		client = NewClient(p)
	}
	if existing, err := VerifyFile(destination, expectation); err == nil {
		return existing, nil
	} else if !errors.Is(err, os.ErrNotExist) {
		return Result{}, failure("transport_destination_corrupt", err)
	}

	parent := filepath.Dir(destination)
	if err := os.MkdirAll(parent, 0o700); err != nil {
		return Result{}, failure("transport_temp_create_failed", err)
	}
	tmp, err := os.CreateTemp(parent, ".seen-download-*")
	if err != nil {
		return Result{}, failure("transport_temp_create_failed", err)
	}
	tmpPath := tmp.Name()
	committed := false
	defer func() {
		_ = tmp.Close()
		if !committed {
			_ = os.Remove(tmpPath)
		}
	}()
	if err := tmp.Chmod(0o600); err != nil {
		return Result{}, failure("transport_temp_create_failed", err)
	}

	requestCtx := ctx
	var cancel context.CancelFunc
	if _, ok := ctx.Deadline(); !ok {
		requestCtx, cancel = context.WithTimeout(ctx, p.RequestTimeout)
		defer cancel()
	}
	req, err := http.NewRequestWithContext(requestCtx, http.MethodGet, rawURL, nil)
	if err != nil {
		return Result{}, failure("transport_request_failed", err)
	}
	req.Header.Set("Accept-Encoding", "identity")
	req.Header.Set("Accept", "application/octet-stream")
	resp, err := client.Do(req)
	if err != nil {
		var policyErr *Error
		if errors.As(err, &policyErr) {
			return Result{}, policyErr
		}
		return Result{}, failure("transport_request_failed", err)
	}
	defer resp.Body.Close()
	if !sameOrigin(resp.Request.URL, origin) || (resp.Request.URL.Path != origin.Path && !strings.HasPrefix(resp.Request.URL.Path, origin.Path+"/")) {
		return Result{}, failure("transport_origin_mismatch", errors.New("response URL is outside configured registry origin"))
	}
	if resp.StatusCode < 200 || resp.StatusCode > 299 {
		return Result{}, failure("transport_http_status", fmt.Errorf("unexpected HTTP status %d", resp.StatusCode))
	}
	encoding := strings.TrimSpace(strings.ToLower(resp.Header.Get("Content-Encoding")))
	if encoding != "" && encoding != "identity" {
		return Result{}, failure("transport_content_encoding", fmt.Errorf("unexpected content encoding %q", encoding))
	}
	if resp.ContentLength >= 0 && resp.ContentLength != expectation.Length {
		return Result{}, failure("signing_target_length_mismatch", fmt.Errorf("signed=%d received=%d", expectation.Length, resp.ContentLength))
	}

	h := sha256.New()
	limited := io.LimitReader(resp.Body, expectation.Length+1)
	n, err := io.Copy(io.MultiWriter(tmp, h), limited)
	if err != nil {
		return Result{}, failure("transport_read_failed", err)
	}
	if n != expectation.Length {
		return Result{}, failure("signing_target_length_mismatch", fmt.Errorf("signed=%d received=%d", expectation.Length, n))
	}
	actual := hex.EncodeToString(h.Sum(nil))
	if actual != expectation.SHA256 {
		return Result{}, failure("signing_target_hash_mismatch", fmt.Errorf("signed=%s received=%s", expectation.SHA256, actual))
	}
	if err := tmp.Sync(); err != nil {
		return Result{}, failure("transport_temp_sync_failed", err)
	}
	if err := tmp.Close(); err != nil {
		return Result{}, failure("transport_temp_close_failed", err)
	}
	if err := os.Chmod(tmpPath, 0o444); err != nil {
		return Result{}, failure("transport_publish_failed", err)
	}

	// A hard link gives us same-filesystem, atomic, no-replace publication.
	if err := os.Link(tmpPath, destination); err != nil {
		if existing, verifyErr := VerifyFile(destination, expectation); verifyErr == nil {
			_ = os.Remove(tmpPath)
			committed = true
			return existing, nil
		}
		return Result{}, failure("transport_publish_failed", err)
	}
	if err := os.Remove(tmpPath); err != nil {
		return Result{}, failure("transport_publish_failed", err)
	}
	committed = true
	if dir, err := os.Open(parent); err == nil {
		_ = dir.Sync()
		_ = dir.Close()
	}
	return Result{Path: destination, SHA256: actual, Length: n}, nil
}

// VerifyFile rehashes an existing artifact; cached digests are never trusted
// merely because their path contains the expected hash.
func VerifyFile(path string, expectation Expectation) (Result, error) {
	f, err := os.Open(path)
	if err != nil {
		return Result{}, err
	}
	defer f.Close()
	info, err := f.Stat()
	if err != nil {
		return Result{}, err
	}
	if !info.Mode().IsRegular() || info.Size() != expectation.Length {
		return Result{}, fmt.Errorf("signed length %d does not match cached file", expectation.Length)
	}
	h := sha256.New()
	n, err := io.Copy(h, io.LimitReader(f, expectation.Length+1))
	if err != nil {
		return Result{}, err
	}
	actual := hex.EncodeToString(h.Sum(nil))
	if n != expectation.Length || actual != expectation.SHA256 {
		return Result{}, fmt.Errorf("cached file failed digest or length verification")
	}
	return Result{Path: path, SHA256: actual, Length: n}, nil
}
