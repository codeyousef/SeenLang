package commands

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/codeyousef/seen/tools/seen-pkg/internal/model"
)

func TestPublishUsesAuthenticatedContractFlowAndLeavesPublicReleaseQuarantined(t *testing.T) {
	project := t.TempDir()
	manifestText := `manifest-version = 1

[project]
name = "smoke"
version = "0.1.0"
license = "MIT"
repository = "https://github.com/codeyousef/seen"

[package]
identity = "seen/registry-smoke"
visibility = "public"
include = ["src/**/*.seen"]
assets = []
capabilities = []

[dependencies]
`
	if err := os.MkdirAll(filepath.Join(project, "src"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(project, "Seen.toml"), []byte(manifestText), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(project, "src", "main.seen"), []byte("let answer = 42\n"), 0o644); err != nil {
		t.Fatal(err)
	}

	const token = "test-registry-token-that-is-at-least-thirty-two-bytes"
	var mu sync.Mutex
	steps := make([]string, 0, 4)
	var reservedDigest string
	var reservedLength int64
	var reservedManifestSHA256 string
	handler := http.HandlerFunc(func(response http.ResponseWriter, request *http.Request) {
		if request.Header.Get("Authorization") != "Bearer "+token {
			http.Error(response, "missing credential", http.StatusUnauthorized)
			return
		}
		mu.Lock()
		steps = append(steps, request.Method+" "+request.URL.Path)
		mu.Unlock()
		switch request.Method + " " + request.URL.Path {
		case "POST /packages/api/v1/packages":
			if request.Header.Get("Idempotency-Key") == "" {
				t.Error("create package omitted Idempotency-Key")
			}
			response.Header().Set("Content-Type", "application/json")
			response.WriteHeader(http.StatusCreated)
			_, _ = io.WriteString(response, `{"contract_version":1,"identity":"seen/registry-smoke","namespace_status":"active","description":null,"repository":"https://github.com/codeyousef/seen","license_spdx":"MIT","latest_active_version":null,"created_at":"2026-07-16T08:00:00Z","updated_at":"2026-07-16T08:00:00Z","links":{"self":"/packages/api/v1/packages/seen/registry-smoke","releases":"/packages/api/v1/packages/seen/registry-smoke/releases"}}`)
		case "POST /packages/api/v1/packages/seen/registry-smoke/releases":
			if request.Header.Get("Idempotency-Key") == "" {
				t.Error("reserve release omitted Idempotency-Key")
			}
			var body reserveReleaseRequest
			if err := json.NewDecoder(request.Body).Decode(&body); err != nil {
				t.Error(err)
			}
			if body.Version != "0.1.0" || body.Visibility != "public" || body.Source.Forge != "gitlab" || body.Source.RepositoryID != "123456" {
				t.Errorf("unexpected reservation: %+v", body)
			}
			manifestDigest := sha256.Sum256([]byte(manifestText))
			if body.ManifestSHA256 != hex.EncodeToString(manifestDigest[:]) {
				t.Errorf("manifest digest = %s", body.ManifestSHA256)
			}
			reservedDigest = body.Archive.SHA256
			reservedLength = body.Archive.CompressedBytes
			reservedManifestSHA256 = body.ManifestSHA256
			response.Header().Set("Content-Type", "application/json")
			response.WriteHeader(http.StatusCreated)
			reservation := map[string]any{
				"release": validReleaseRecord("seen/registry-smoke", "0.1.0", "public", "reserved", "unavailable", reservedManifestSHA256, reservedDigest, reservedLength),
				"upload": map[string]any{
					"upload_id":     "upl_0123456789abcdef",
					"method":        "PUT",
					"path":          "/packages/api/v1/uploads/upl_0123456789abcdef/archive",
					"expires_at":    "2099-07-16T08:00:00Z",
					"maximum_bytes": maximumArchiveUploadBytes,
					"required_headers": map[string]any{
						"Content-Type":          "application/gzip",
						"Content-Length":        reservedLength,
						"X-Seen-Archive-Sha256": reservedDigest,
					},
				},
			}
			if err := json.NewEncoder(response).Encode(reservation); err != nil {
				t.Error(err)
			}
		case "PUT /packages/api/v1/uploads/upl_0123456789abcdef/archive":
			archive, err := io.ReadAll(request.Body)
			if err != nil {
				t.Error(err)
			}
			digest := sha256.Sum256(archive)
			if got := hex.EncodeToString(digest[:]); got != reservedDigest || request.Header.Get("X-Seen-Archive-Sha256") != got || request.Header.Get("Content-Type") != "application/gzip" || request.ContentLength != reservedLength {
				t.Errorf("uploaded digest=%s reserved=%s header=%s", got, reservedDigest, request.Header.Get("X-Seen-Archive-Sha256"))
			}
			response.Header().Set("X-Seen-Archive-Sha256", reservedDigest)
			response.Header().Set("ETag", `"sha256:`+reservedDigest+`"`)
			response.WriteHeader(http.StatusNoContent)
		case "POST /packages/api/v1/uploads/upl_0123456789abcdef/complete":
			if request.Header.Get("Idempotency-Key") == "" {
				t.Error("complete upload omitted Idempotency-Key")
			}
			var body struct {
				ArchiveSHA256   string `json:"archive_sha256"`
				CompressedBytes int64  `json:"compressed_bytes"`
			}
			if err := json.NewDecoder(request.Body).Decode(&body); err != nil {
				t.Error(err)
			}
			if body.ArchiveSHA256 != reservedDigest || body.CompressedBytes != reservedLength {
				t.Errorf("completion body = %+v", body)
			}
			response.Header().Set("Content-Type", "application/json")
			response.WriteHeader(http.StatusAccepted)
			if err := json.NewEncoder(response).Encode(validReleaseRecord("seen/registry-smoke", "0.1.0", "public", "quarantined", "unavailable", reservedManifestSHA256, reservedDigest, reservedLength)); err != nil {
				t.Error(err)
			}
		default:
			http.NotFound(response, request)
		}
	})
	server := httptest.NewTLSServer(handler)
	defer server.Close()
	transport := server.Client().Transport.(*http.Transport).Clone()
	transport.TLSClientConfig = transport.TLSClientConfig.Clone()
	transport.TLSClientConfig.ServerName = "example.com"
	serverAddress := server.Listener.Addr().String()
	transport.DialContext = func(ctx context.Context, network, _ string) (net.Conn, error) {
		return (&net.Dialer{}).DialContext(ctx, network, serverAddress)
	}
	client := &http.Client{Transport: transport}

	t.Setenv("SEEN_REGISTRY_TOKEN", token)
	t.Setenv("SEEN_SOURCE_FORGE", "gitlab")
	t.Setenv("SEEN_SOURCE_REPOSITORY_ID", "123456")
	t.Setenv("SEEN_SOURCE_INSTALLATION_ID", "internal-dev-publisher")
	t.Setenv("SEEN_SOURCE_REF", "refs/heads/feat/FEL-630-seen-registry-client")
	t.Setenv("SEEN_SOURCE_COMMIT", strings.Repeat("a", 40))
	var stdout, stderr bytes.Buffer
	runner := Runner{
		Backend: &ProductionBackend{httpClient: client},
		Streams: Streams{Stdout: &stdout, Stderr: &stderr},
	}
	code := runner.Run(context.Background(), []string{"publish", project, "--registry", "https://registry.test/packages"})
	if code != 0 {
		t.Fatalf("code=%d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
	}
	if !strings.Contains(stdout.String(), "Submitted seen/registry-smoke@0.1.0") || strings.Contains(stdout.String(), "Published") || !strings.Contains(stdout.String(), "lifecycle=quarantined") || !strings.Contains(stdout.String(), "72-hour delay") {
		t.Fatalf("stdout=%q", stdout.String())
	}
	want := []string{
		"POST /packages/api/v1/packages",
		"POST /packages/api/v1/packages/seen/registry-smoke/releases",
		"PUT /packages/api/v1/uploads/upl_0123456789abcdef/archive",
		"POST /packages/api/v1/uploads/upl_0123456789abcdef/complete",
	}
	mu.Lock()
	defer mu.Unlock()
	if strings.Join(steps, "\n") != strings.Join(want, "\n") {
		t.Fatalf("steps=%q", steps)
	}
}

func TestPublishTokenFileMustBePrivate(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("Windows uses the environment credential path")
	}
	path := filepath.Join(t.TempDir(), "token")
	if err := os.WriteFile(path, []byte(strings.Repeat("x", 40)), 0o644); err != nil {
		t.Fatal(err)
	}
	if _, err := loadPublishToken(path); err == nil || !strings.Contains(err.Error(), "private bounded regular file") {
		t.Fatalf("err=%v", err)
	}
}

func TestPublishTokenFileUsesOnePrivateBoundedRegularFile(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("Windows uses the environment credential path")
	}
	token := strings.Repeat("s", 40)
	secure := filepath.Join(t.TempDir(), "token")
	if err := os.WriteFile(secure, []byte(token+"\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if got, err := loadPublishToken(secure); err != nil || got != token {
		t.Fatalf("got=%q err=%v", got, err)
	}

	oversized := filepath.Join(t.TempDir(), "oversized-token")
	if err := os.WriteFile(oversized, []byte(strings.Repeat("x", maxPublishTokenFileBytes+1)), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := loadPublishToken(oversized); err == nil || !strings.Contains(err.Error(), "private bounded regular file") {
		t.Fatalf("oversized token err=%v", err)
	}

	symlink := filepath.Join(t.TempDir(), "token-link")
	if err := os.Symlink(secure, symlink); err != nil {
		t.Skipf("symlinks unavailable: %v", err)
	}
	if _, err := loadPublishToken(symlink); err == nil {
		t.Fatal("symlink token file was accepted")
	}
}

func TestPublishRejectsTokenFileSelectedAsPackageContent(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("Windows uses the environment credential path")
	}
	project := t.TempDir()
	manifestText := `manifest-version = 1

[project]
name = "smoke"
version = "0.1.0"
license = "MIT"

[package]
identity = "seen/registry-smoke"
visibility = "public"
include = ["**"]
assets = []
capabilities = []

[dependencies]
`
	if err := os.MkdirAll(filepath.Join(project, "src"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(project, "Seen.toml"), []byte(manifestText), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(project, "src", "main.seen"), []byte("let answer = 42\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	tokenPath := filepath.Join(project, ".publish-token")
	if err := os.WriteFile(tokenPath, []byte(strings.Repeat("s", 40)), 0o600); err != nil {
		t.Fatal(err)
	}
	credential, err := loadPublishCredential(tokenPath)
	if err != nil {
		t.Fatal(err)
	}
	defer credential.close()
	if _, err := packWithForbiddenFile(context.Background(), project, filepath.Join(t.TempDir(), "package.tgz"), credential.fileInfo); err == nil || !strings.Contains(err.Error(), "token file") {
		t.Fatalf("selected token file error = %v", err)
	}
}

func TestPublishStopsOnPackageIdentityConflict(t *testing.T) {
	project := t.TempDir()
	manifestText := `manifest-version = 1

[project]
name = "smoke"
version = "0.1.0"
license = "MIT"

[package]
identity = "seen/registry-smoke"
visibility = "public"
include = ["src/**/*.seen"]
assets = []
capabilities = []

[dependencies]
`
	if err := os.MkdirAll(filepath.Join(project, "src"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(project, "Seen.toml"), []byte(manifestText), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(project, "src", "main.seen"), []byte("let answer = 42\n"), 0o644); err != nil {
		t.Fatal(err)
	}

	requests := 0
	client := &http.Client{Transport: roundTripFunc(func(request *http.Request) (*http.Response, error) {
		requests++
		body := `{"error":{"code":"package_already_exists","message":"Package identity is already reserved"}}`
		return &http.Response{
			StatusCode: http.StatusConflict,
			Header:     make(http.Header),
			Body:       io.NopCloser(strings.NewReader(body)),
			Request:    request,
		}, nil
	})}
	t.Setenv("SEEN_REGISTRY_TOKEN", strings.Repeat("t", 40))
	t.Setenv("SEEN_SOURCE_REPOSITORY_ID", "123456")
	t.Setenv("SEEN_SOURCE_INSTALLATION_ID", "internal-dev-publisher")
	t.Setenv("SEEN_SOURCE_REF", "refs/heads/main")
	t.Setenv("SEEN_SOURCE_COMMIT", strings.Repeat("a", 40))
	var stdout, stderr bytes.Buffer
	runner := Runner{
		Backend: &ProductionBackend{httpClient: client},
		Streams: Streams{Stdout: &stdout, Stderr: &stderr},
	}
	code := runner.Run(context.Background(), []string{"publish", project, "--registry", "https://example.com/packages"})
	if code == 0 || requests != 1 || !strings.Contains(stderr.String(), "package_already_exists") {
		t.Fatalf("code=%d requests=%d stdout=%q stderr=%q", code, requests, stdout.String(), stderr.String())
	}
}

func TestPublishManifestMustRemainByteExactAfterPacking(t *testing.T) {
	path := filepath.Join(t.TempDir(), "Seen.toml")
	expected := []byte("manifest-version = 1\n")
	if err := os.WriteFile(path, expected, 0o644); err != nil {
		t.Fatal(err)
	}
	if err := verifyPublishManifestUnchanged(path, expected); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, append(append([]byte(nil), expected...), '\n'), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := verifyPublishManifestUnchanged(path, expected); err == nil || !strings.Contains(err.Error(), "changed while") {
		t.Fatalf("err=%v", err)
	}
}

func TestPublishSourceDeclarationEnforcesContractBoundsAndHTTPSRepository(t *testing.T) {
	valid := publishCLI{
		SourceForge:    "github",
		RepositoryID:   "123456",
		InstallationID: "internal-dev-publisher",
		SourceRef:      "refs/heads/main",
		SourceCommit:   strings.Repeat("a", 40),
		LicenseSPDX:    "MIT",
		RepositoryURL:  "https://github.com/codeyousef/seen",
	}
	if err := valid.validateSource(); err != nil {
		t.Fatal(err)
	}
	tests := []struct {
		name   string
		mutate func(*publishCLI)
	}{
		{"source-forge", func(cli *publishCLI) { cli.SourceForge = "bitbucket" }},
		{"source-forge-case", func(cli *publishCLI) { cli.SourceForge = "GitHub" }},
		{"repository-id", func(cli *publishCLI) { cli.RepositoryID = strings.Repeat("r", 129) }},
		{"installation-id", func(cli *publishCLI) { cli.InstallationID = strings.Repeat("i", 129) }},
		{"source-ref", func(cli *publishCLI) { cli.SourceRef = "refs/heads/" + strings.Repeat("r", 255) }},
		{"license", func(cli *publishCLI) { cli.LicenseSPDX = strings.Repeat("l", 129) }},
		{"description", func(cli *publishCLI) { cli.Description = strings.Repeat("d", 513) }},
		{"repository-http", func(cli *publishCLI) { cli.RepositoryURL = "http://github.com/codeyousef/seen" }},
		{"repository-relative", func(cli *publishCLI) { cli.RepositoryURL = "github.com/codeyousef/seen" }},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			candidate := valid
			test.mutate(&candidate)
			if err := candidate.validateSource(); err == nil {
				t.Fatal("invalid publish source declaration was accepted")
			}
		})
	}
}

func TestParsePublishCLISourceForgeDefaultsAndOverrides(t *testing.T) {
	t.Setenv("SEEN_SOURCE_FORGE", "")
	defaults, err := parsePublishCLI(nil)
	if err != nil {
		t.Fatal(err)
	}
	if defaults.SourceForge != "github" {
		t.Fatalf("default source forge = %q", defaults.SourceForge)
	}

	t.Setenv("SEEN_SOURCE_FORGE", "gitlab")
	fromEnvironment, err := parsePublishCLI(nil)
	if err != nil {
		t.Fatal(err)
	}
	if fromEnvironment.SourceForge != "gitlab" {
		t.Fatalf("environment source forge = %q", fromEnvironment.SourceForge)
	}

	fromFlag, err := parsePublishCLI([]string{"--source-forge", "github"})
	if err != nil {
		t.Fatal(err)
	}
	if fromFlag.SourceForge != "github" {
		t.Fatalf("flag source forge = %q", fromFlag.SourceForge)
	}
}

func TestReleaseReservationMustBindArchiveAndUploadInstruction(t *testing.T) {
	packed := PackResult{SHA256: strings.Repeat("a", 64), Length: 1234}
	manifestSHA256 := strings.Repeat("b", 64)
	now := time.Date(2026, 7, 16, 8, 0, 0, 0, time.UTC)
	valid := validReleaseReservation(packed, manifestSHA256)
	if err := validateReleaseReservation(valid, "seen/registry-smoke", "0.1.0", "public", manifestSHA256, nil, packed, now); err != nil {
		t.Fatal(err)
	}
	tests := []struct {
		name   string
		mutate func(*releaseReservationResponse)
	}{
		{"release-identity", func(value *releaseReservationResponse) { value.Release.Package = "seen/other" }},
		{"release-digest", func(value *releaseReservationResponse) { value.Release.Archive.SHA256 = strings.Repeat("b", 64) }},
		{"manifest-digest", func(value *releaseReservationResponse) { value.Release.ManifestSHA256 = strings.Repeat("c", 64) }},
		{"capabilities", func(value *releaseReservationResponse) {
			value.Release.Capabilities = []model.Capability{model.CapabilityNetwork}
		}},
		{"upload-id-path", func(value *releaseReservationResponse) {
			value.Upload.Path = "/packages/api/v1/uploads/upl_ffffffffffffffff/archive"
		}},
		{"upload-maximum", func(value *releaseReservationResponse) { value.Upload.MaximumBytes-- }},
		{"required-content-type", func(value *releaseReservationResponse) {
			value.Upload.RequiredHeader.ContentType = "application/octet-stream"
		}},
		{"required-length", func(value *releaseReservationResponse) { value.Upload.RequiredHeader.ContentLength++ }},
		{"required-digest", func(value *releaseReservationResponse) {
			value.Upload.RequiredHeader.ArchiveSHA256 = strings.Repeat("b", 64)
		}},
		{"expired", func(value *releaseReservationResponse) { value.Upload.ExpiresAt = "2026-07-16T07:59:59Z" }},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			candidate := valid
			test.mutate(&candidate)
			if err := validateReleaseReservation(candidate, "seen/registry-smoke", "0.1.0", "public", manifestSHA256, nil, packed, now); err == nil {
				t.Fatal("unbound registry reservation was accepted")
			}
		})
	}
}

func TestCompletedReleaseMustBindRequestAndAdvancePastReserved(t *testing.T) {
	packed := PackResult{SHA256: strings.Repeat("a", 64), Length: 1234}
	manifestSHA256 := strings.Repeat("b", 64)
	valid := validPublishRelease("seen/registry-smoke", "0.1.0", "public", "quarantined", "unavailable", manifestSHA256, packed.SHA256, packed.Length)
	if err := validateCompletedRelease(valid, "seen/registry-smoke", "0.1.0", "public", manifestSHA256, nil, packed); err != nil {
		t.Fatal(err)
	}
	tests := []struct {
		name   string
		mutate func(*publishRelease)
	}{
		{"wrong-version", func(value *publishRelease) { value.Version = "0.1.1" }},
		{"wrong-length", func(value *publishRelease) { value.Archive.CompressedBytes++ }},
		{"wrong-manifest", func(value *publishRelease) { value.ManifestSHA256 = strings.Repeat("c", 64) }},
		{"wrong-visibility", func(value *publishRelease) { value.State.Visibility = "private" }},
		{"missing-retention", func(value *publishRelease) { value.State.Retention = "" }},
		{"reserved", func(value *publishRelease) { value.State.Lifecycle = "reserved" }},
		{"visible-before-active", func(value *publishRelease) { value.State.Availability = "available" }},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			candidate := valid
			test.mutate(&candidate)
			if err := validateCompletedRelease(candidate, "seen/registry-smoke", "0.1.0", "public", manifestSHA256, nil, packed); err == nil {
				t.Fatal("unbound completed release was accepted")
			}
		})
	}
}

func TestPublishSubmissionNoticeDependsOnAvailability(t *testing.T) {
	packed := PackResult{SHA256: strings.Repeat("a", 64), Length: 1234}
	manifestSHA256 := strings.Repeat("b", 64)
	unavailable := validPublishRelease("seen/registry-smoke", "0.1.0", "public", "active", "unavailable", manifestSHA256, packed.SHA256, packed.Length)
	var output bytes.Buffer
	writePublishSubmission(&output, unavailable, packed.SHA256)
	if !strings.Contains(output.String(), "Submitted") || !strings.Contains(output.String(), "72-hour delay") {
		t.Fatalf("unavailable output=%q", output.String())
	}

	available := unavailable
	available.State.Availability = "available"
	output.Reset()
	writePublishSubmission(&output, available, packed.SHA256)
	if strings.Contains(output.String(), "72-hour delay") {
		t.Fatalf("available output=%q", output.String())
	}
}

func TestRegistryPublishClientRefusesRedirectWithoutForwardingAuthorization(t *testing.T) {
	const token = "redirect-test-token-that-is-at-least-thirty-two-bytes"
	targetHit := false
	target := httptest.NewServer(http.HandlerFunc(func(response http.ResponseWriter, request *http.Request) {
		targetHit = true
		if request.Header.Get("Authorization") != "" {
			t.Error("authorization was forwarded across a redirect")
		}
		response.WriteHeader(http.StatusNoContent)
	}))
	defer target.Close()
	redirect := httptest.NewServer(http.HandlerFunc(func(response http.ResponseWriter, request *http.Request) {
		if request.Header.Get("Authorization") != "Bearer "+token {
			t.Error("initial registry request omitted authorization")
		}
		response.Header().Set("Location", target.URL+"/sink")
		response.WriteHeader(http.StatusTemporaryRedirect)
	}))
	defer redirect.Close()

	client := &registryPublishClient{origin: redirect.URL, token: token, http: newRegistryPublishHTTPClient()}
	status, _, err := client.jsonRequest(context.Background(), http.MethodPost, "/api/v1/packages", map[string]string{"identity": "seen/registry-smoke"}, true)
	if err != nil {
		t.Fatal(err)
	}
	if status != http.StatusTemporaryRedirect || targetHit {
		t.Fatalf("status=%d targetHit=%v", status, targetHit)
	}
}

func TestUploadRejectsMismatchedConfirmationHeaders(t *testing.T) {
	archive := []byte("archive bytes")
	digest := sha256.Sum256(archive)
	sha := hex.EncodeToString(digest[:])
	path := filepath.Join(t.TempDir(), "archive.tgz")
	if err := os.WriteFile(path, archive, 0o600); err != nil {
		t.Fatal(err)
	}
	server := httptest.NewServer(http.HandlerFunc(func(response http.ResponseWriter, request *http.Request) {
		if request.Header.Get("Content-Type") != "application/gzip" || request.Header.Get("X-Seen-Archive-Sha256") != sha || request.ContentLength != int64(len(archive)) {
			t.Errorf("unbound upload headers: %v", request.Header)
		}
		response.Header().Set("X-Seen-Archive-Sha256", sha)
		response.Header().Set("ETag", `"sha256:`+strings.Repeat("b", 64)+`"`)
		response.WriteHeader(http.StatusNoContent)
	}))
	defer server.Close()
	client := &registryPublishClient{origin: server.URL, token: strings.Repeat("t", 40), http: server.Client()}
	packed := PackResult{Path: path, SHA256: sha, Length: int64(len(archive))}
	required := uploadRequiredHeaders{ContentType: "application/gzip", ContentLength: packed.Length, ArchiveSHA256: packed.SHA256}
	if err := client.upload(context.Background(), "/upload", packed, required); err == nil || !strings.Contains(err.Error(), "mismatched archive confirmation") {
		t.Fatalf("err=%v", err)
	}
}

func validReleaseReservation(packed PackResult, manifestSHA256 string) releaseReservationResponse {
	return releaseReservationResponse{
		Release: validPublishRelease("seen/registry-smoke", "0.1.0", "public", "reserved", "unavailable", manifestSHA256, packed.SHA256, packed.Length),
		Upload: uploadInstruction{
			UploadID:     "upl_0123456789abcdef",
			Method:       http.MethodPut,
			Path:         "/packages/api/v1/uploads/upl_0123456789abcdef/archive",
			ExpiresAt:    "2099-07-16T08:00:00Z",
			MaximumBytes: maximumArchiveUploadBytes,
			RequiredHeader: uploadRequiredHeaders{
				ContentType:   "application/gzip",
				ContentLength: packed.Length,
				ArchiveSHA256: packed.SHA256,
			},
		},
	}
}

type roundTripFunc func(*http.Request) (*http.Response, error)

func (roundTrip roundTripFunc) RoundTrip(request *http.Request) (*http.Response, error) {
	return roundTrip(request)
}

func validPublishRelease(identity, version, visibility, lifecycle, availability, manifestSHA256, archiveSHA string, compressedBytes int64) publishRelease {
	value := publishRelease{
		Package:        identity,
		Version:        version,
		Archive:        publishArchive{Format: "tar+gzip", SHA256: archiveSHA, CompressedBytes: compressedBytes},
		ManifestSHA256: manifestSHA256,
		Capabilities:   []model.Capability{},
	}
	value.State.Lifecycle = lifecycle
	value.State.Visibility = visibility
	value.State.Availability = availability
	value.State.Retention = "retained"
	return value
}

func validReleaseRecord(identity, version, visibility, lifecycle, availability, manifestSHA256, archiveSHA string, compressedBytes int64) map[string]any {
	nullableTimestamps := map[string]any{
		"reserved_at":             "2026-07-16T08:00:00Z",
		"uploaded_at":             nil,
		"quarantined_at":          nil,
		"public_delay_started_at": nil,
		"public_delay_ends_at":    nil,
		"ready_at":                nil,
		"activated_at":            nil,
		"yanked_at":               nil,
		"security_quarantined_at": nil,
		"soft_deleted_at":         nil,
		"updated_at":              "2026-07-16T08:00:00Z",
	}
	return map[string]any{
		"contract_version": 1,
		"package":          identity,
		"version":          version,
		"archive": map[string]any{
			"format":                     "tar+gzip",
			"sha256":                     archiveSHA,
			"compressed_bytes":           compressedBytes,
			"expanded_bytes":             nil,
			"entry_count":                nil,
			"largest_regular_file_bytes": nil,
			"longest_path_bytes":         nil,
			"maximum_path_depth":         nil,
		},
		"manifest_sha256": manifestSHA256,
		"capabilities":    []string{},
		"state": map[string]any{
			"lifecycle":    lifecycle,
			"visibility":   visibility,
			"availability": availability,
			"retention":    "retained",
		},
		"source_proof_id": nil,
		"verification": map[string]any{
			"origin":               "pending",
			"integrity":            "pending",
			"source":               "pending",
			"first_scan":           "pending",
			"second_scan":          "pending",
			"attestation_sequence": 0,
		},
		"timestamps":                nullableTimestamps,
		"resolver_metadata_version": nil,
		"links": map[string]any{
			"self":         fmt.Sprintf("/packages/api/v1/packages/%s/releases/%s", identity, version),
			"package":      "/packages/api/v1/packages/" + identity,
			"source_proof": nil,
			"download":     nil,
		},
	}
}
