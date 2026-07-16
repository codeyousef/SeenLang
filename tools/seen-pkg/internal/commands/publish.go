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
	"net/url"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"time"

	"github.com/codeyousef/seen/tools/seen-pkg/internal/manifest"
	"github.com/codeyousef/seen/tools/seen-pkg/internal/model"
	"github.com/pelletier/go-toml/v2"
)

const (
	defaultPublishOrigin      = "https://seen.dev.yousef.codes/packages"
	maxAPIResponseBytes       = 4 * 1024 * 1024
	maxPublishTokenFileBytes  = 4096
	maximumArchiveUploadBytes = 25 * 1024 * 1024
)

type publishCLI struct {
	ManifestPath, RegistryOrigin, TokenFile               string
	RepositoryID, InstallationID, SourceRef, SourceCommit string
	LicenseSPDX, Description, RepositoryURL               string
	Quiet                                                 bool
}

type createPackageRequest struct {
	Identity    string  `json:"identity"`
	Description *string `json:"description,omitempty"`
	Repository  *string `json:"repository,omitempty"`
	LicenseSPDX *string `json:"license_spdx,omitempty"`
}

type reserveReleaseRequest struct {
	Version        string          `json:"version"`
	Visibility     string          `json:"visibility"`
	Archive        publishArchive  `json:"archive"`
	ManifestSHA256 string          `json:"manifest_sha256"`
	Manifest       json.RawMessage `json:"manifest"`
	Source         publishSource   `json:"source"`
}

type publishArchive struct {
	Format          string `json:"format"`
	SHA256          string `json:"sha256"`
	CompressedBytes int64  `json:"compressed_bytes"`
}

type publishSource struct {
	Forge          string `json:"forge"`
	RepositoryID   string `json:"repository_id"`
	InstallationID string `json:"installation_id"`
	RequestedRef   string `json:"requested_ref"`
	ExpectedCommit string `json:"expected_commit"`
	LicenseSPDX    string `json:"license_spdx"`
}

type releaseReservationResponse struct {
	Release publishRelease    `json:"release"`
	Upload  uploadInstruction `json:"upload"`
}

type uploadInstruction struct {
	UploadID       string                `json:"upload_id"`
	Method         string                `json:"method"`
	Path           string                `json:"path"`
	ExpiresAt      string                `json:"expires_at"`
	MaximumBytes   int64                 `json:"maximum_bytes"`
	RequiredHeader uploadRequiredHeaders `json:"required_headers"`
}

type uploadRequiredHeaders struct {
	ContentType   string `json:"Content-Type"`
	ContentLength int64  `json:"Content-Length"`
	ArchiveSHA256 string `json:"X-Seen-Archive-Sha256"`
}

type publishRelease struct {
	Package        string             `json:"package"`
	Version        string             `json:"version"`
	Archive        publishArchive     `json:"archive"`
	ManifestSHA256 string             `json:"manifest_sha256"`
	Capabilities   []model.Capability `json:"capabilities"`
	State          struct {
		Lifecycle    string `json:"lifecycle"`
		Visibility   string `json:"visibility"`
		Availability string `json:"availability"`
		Retention    string `json:"retention"`
	} `json:"state"`
}

type registryErrorEnvelope struct {
	Error struct {
		Code    string `json:"code"`
		Message string `json:"message"`
	} `json:"error"`
}

func (backend *ProductionBackend) publish(ctx context.Context, arguments []string, streams Streams) error {
	cli, err := parsePublishCLI(arguments)
	if err != nil {
		return err
	}
	if err := model.ValidateRegistryOrigin(cli.RegistryOrigin); err != nil {
		return err
	}
	credential, err := loadPublishCredential(cli.TokenFile)
	if err != nil {
		return err
	}
	defer credential.close()
	manifestPath, projectRoot, err := resolveManifestPath(cli.ManifestPath)
	if err != nil {
		return err
	}
	parsed, err := manifest.Load(manifestPath, manifest.Options{DefaultRegistryOrigin: cli.RegistryOrigin, RequireManifestV1: true})
	if err != nil {
		return err
	}
	if parsed.Package == nil {
		return errors.New("[package] is required for publish")
	}
	manifestJSON, fields, err := parsedPublishManifest(parsed.Raw)
	if err != nil {
		return err
	}
	cli.fillManifestDefaults(fields)
	if err := cli.validateSource(); err != nil {
		return err
	}

	tempDir, err := os.MkdirTemp("", "seen-publish-*")
	if err != nil {
		return err
	}
	defer os.RemoveAll(tempDir)
	archiveName := strings.Split(parsed.Package.Identity, "/")[1] + "-" + parsed.Project.Version + ".seenpkg.tgz"
	packed, err := packWithForbiddenFile(ctx, projectRoot, filepath.Join(tempDir, archiveName), credential.fileInfo)
	if err != nil {
		return err
	}
	if err := verifyPublishManifestUnchanged(manifestPath, parsed.Raw); err != nil {
		return err
	}

	httpClient := backend.httpClient
	if httpClient == nil {
		httpClient = newRegistryPublishHTTPClient()
	}
	client := &registryPublishClient{
		origin: cli.RegistryOrigin,
		token:  credential.token,
		http:   httpClient,
	}
	create := createPackageRequest{Identity: parsed.Package.Identity}
	if cli.Description != "" {
		create.Description = &cli.Description
	}
	if cli.RepositoryURL != "" {
		create.Repository = &cli.RepositoryURL
	}
	if cli.LicenseSPDX != "" {
		create.LicenseSPDX = &cli.LicenseSPDX
	}
	status, body, err := client.jsonRequest(ctx, http.MethodPost, "/api/v1/packages", create, true)
	if err != nil {
		return err
	}
	if status != http.StatusCreated {
		return apiStatusError("reserve package identity", status, body)
	}

	manifestDigest := sha256.Sum256(parsed.Raw)
	manifestSHA256 := hex.EncodeToString(manifestDigest[:])
	reserve := reserveReleaseRequest{
		Version:        parsed.Project.Version,
		Visibility:     parsed.Package.Visibility,
		Archive:        publishArchive{Format: "tar+gzip", SHA256: packed.SHA256, CompressedBytes: packed.Length},
		ManifestSHA256: manifestSHA256,
		Manifest:       manifestJSON,
		Source:         publishSource{Forge: "github", RepositoryID: cli.RepositoryID, InstallationID: cli.InstallationID, RequestedRef: cli.SourceRef, ExpectedCommit: cli.SourceCommit, LicenseSPDX: cli.LicenseSPDX},
	}
	owner, name, _ := strings.Cut(parsed.Package.Identity, "/")
	path := fmt.Sprintf("/api/v1/packages/%s/%s/releases", url.PathEscape(owner), url.PathEscape(name))
	status, body, err = client.jsonRequest(ctx, http.MethodPost, path, reserve, true)
	if err != nil {
		return err
	}
	if status != http.StatusCreated {
		return apiStatusError("reserve release", status, body)
	}
	var reservation releaseReservationResponse
	if err := decodeStrictJSON(body, &reservation); err != nil {
		return fmt.Errorf("decode release reservation: %w", err)
	}
	if err := validateReleaseReservation(reservation, parsed.Package.Identity, parsed.Project.Version, parsed.Package.Visibility, manifestSHA256, parsed.Package.Capabilities, packed, time.Now().UTC()); err != nil {
		return err
	}
	uploadPath := strings.TrimPrefix(reservation.Upload.Path, "/packages")
	if err := client.upload(ctx, uploadPath, packed, reservation.Upload.RequiredHeader); err != nil {
		return err
	}
	completePath := strings.TrimSuffix(uploadPath, "/archive") + "/complete"
	complete := map[string]any{"archive_sha256": packed.SHA256, "compressed_bytes": packed.Length}
	status, body, err = client.jsonRequest(ctx, http.MethodPost, completePath, complete, true)
	if err != nil {
		return err
	}
	if status != http.StatusAccepted {
		return apiStatusError("complete upload", status, body)
	}
	var release publishRelease
	if err := decodeStrictJSON(body, &release); err != nil {
		return fmt.Errorf("decode completed release: %w", err)
	}
	if err := validateCompletedRelease(release, parsed.Package.Identity, parsed.Project.Version, parsed.Package.Visibility, manifestSHA256, parsed.Package.Capabilities, packed); err != nil {
		return err
	}
	if !cli.Quiet {
		writePublishSubmission(streams.Stdout, release, packed.SHA256)
	}
	return nil
}

func parsePublishCLI(arguments []string) (publishCLI, error) {
	options := publishCLI{RegistryOrigin: envOr("SEEN_REGISTRY_ORIGIN", defaultPublishOrigin), TokenFile: os.Getenv("SEEN_REGISTRY_TOKEN_FILE"), RepositoryID: os.Getenv("SEEN_SOURCE_REPOSITORY_ID"), InstallationID: os.Getenv("SEEN_SOURCE_INSTALLATION_ID"), SourceRef: os.Getenv("SEEN_SOURCE_REF"), SourceCommit: os.Getenv("SEEN_SOURCE_COMMIT"), LicenseSPDX: os.Getenv("SEEN_SOURCE_LICENSE_SPDX")}
	for index := 0; index < len(arguments); index++ {
		argument := arguments[index]
		if argument == "--quiet" {
			options.Quiet = true
			continue
		}
		if strings.HasPrefix(argument, "-") {
			if index+1 >= len(arguments) {
				return options, fmt.Errorf("%s requires a value", argument)
			}
			value := arguments[index+1]
			index++
			switch argument {
			case "--registry":
				options.RegistryOrigin = value
			case "--token-file":
				options.TokenFile = value
			case "--source-repository-id":
				options.RepositoryID = value
			case "--source-installation-id":
				options.InstallationID = value
			case "--source-ref":
				options.SourceRef = value
			case "--source-commit":
				options.SourceCommit = value
			case "--license-spdx":
				options.LicenseSPDX = value
			case "--description":
				options.Description = value
			case "--repository":
				options.RepositoryURL = value
			default:
				return options, fmt.Errorf("unknown option %s", argument)
			}
			continue
		}
		if options.ManifestPath != "" {
			return options, errors.New("only one project or Seen.toml path is allowed")
		}
		options.ManifestPath = argument
	}
	if options.ManifestPath == "" {
		options.ManifestPath = "Seen.toml"
	}
	return options, nil
}

func (cli *publishCLI) fillManifestDefaults(document map[string]any) {
	project, _ := document["project"].(map[string]any)
	if cli.LicenseSPDX == "" {
		cli.LicenseSPDX, _ = project["license"].(string)
	}
	if cli.RepositoryURL == "" {
		cli.RepositoryURL, _ = project["repository"].(string)
	}
}

func (cli publishCLI) validateSource() error {
	missing := make([]string, 0, 5)
	for _, field := range []struct {
		name  string
		value string
	}{
		{"SEEN_SOURCE_REPOSITORY_ID/--source-repository-id", cli.RepositoryID},
		{"SEEN_SOURCE_INSTALLATION_ID/--source-installation-id", cli.InstallationID},
		{"SEEN_SOURCE_REF/--source-ref", cli.SourceRef},
		{"SEEN_SOURCE_COMMIT/--source-commit", cli.SourceCommit},
		{"SEEN_SOURCE_LICENSE_SPDX/--license-spdx", cli.LicenseSPDX},
	} {
		if field.value == "" {
			missing = append(missing, field.name)
		}
	}
	if len(missing) != 0 {
		return fmt.Errorf("publish source declaration is incomplete; set %s", strings.Join(missing, ", "))
	}
	if !regexpCommit.MatchString(cli.SourceCommit) {
		return errors.New("source commit must be 40 or 64 lowercase hexadecimal characters")
	}
	if !strings.HasPrefix(cli.SourceRef, "refs/") {
		return errors.New("source ref must start with refs/")
	}
	for _, field := range []struct {
		name    string
		value   string
		maximum int
	}{
		{"source repository ID", cli.RepositoryID, 128},
		{"source installation ID", cli.InstallationID, 128},
		{"source ref", cli.SourceRef, 255},
		{"source license SPDX", cli.LicenseSPDX, 128},
	} {
		if len(field.value) > field.maximum {
			return fmt.Errorf("%s must be at most %d bytes", field.name, field.maximum)
		}
	}
	if len(cli.Description) > 512 {
		return errors.New("package description must be at most 512 bytes")
	}
	if cli.RepositoryURL != "" {
		repository, err := url.ParseRequestURI(cli.RepositoryURL)
		if err != nil || repository.Scheme != "https" || repository.Host == "" {
			return errors.New("package repository must be an absolute HTTPS URL")
		}
	}
	return nil
}

var (
	regexpCommit       = regexp.MustCompile(`^(?:[0-9a-f]{40}|[0-9a-f]{64})$`)
	regexpUploadID     = regexp.MustCompile(`^upl_[A-Za-z0-9_-]{16,96}$`)
	regexpUTCTimestamp = regexp.MustCompile(`^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]{1,9})?Z$`)
)

func parsedPublishManifest(raw []byte) (json.RawMessage, map[string]any, error) {
	var document map[string]any
	if err := toml.Unmarshal(raw, &document); err != nil {
		return nil, nil, err
	}
	encoded, err := json.Marshal(document)
	if err != nil {
		return nil, nil, err
	}
	return encoded, document, nil
}

func verifyPublishManifestUnchanged(path string, expected []byte) error {
	current, err := os.ReadFile(path)
	if err != nil {
		return fmt.Errorf("re-read Seen.toml after packing: %w", err)
	}
	if !bytes.Equal(current, expected) {
		return errors.New("Seen.toml changed while the package archive was being built; retry publish")
	}
	return nil
}

type publishCredential struct {
	token    string
	file     *os.File
	fileInfo os.FileInfo
}

func (credential *publishCredential) close() {
	if credential.file != nil {
		_ = credential.file.Close()
		credential.file = nil
		credential.fileInfo = nil
	}
}

func loadPublishCredential(path string) (publishCredential, error) {
	var credential publishCredential
	var raw []byte
	if path != "" {
		file, err := openPublishTokenFile(path)
		if err != nil {
			return credential, fmt.Errorf("open publish token file: %w", err)
		}
		credential.file = file
		info, err := file.Stat()
		if err != nil {
			credential.close()
			return credential, err
		}
		if !info.Mode().IsRegular() || info.Mode().Perm()&0o077 != 0 || info.Size() > maxPublishTokenFileBytes {
			credential.close()
			return credential, errors.New("publish token file must be a private bounded regular file")
		}
		raw, err = io.ReadAll(io.LimitReader(file, maxPublishTokenFileBytes+1))
		if err != nil {
			credential.close()
			return credential, err
		}
		if len(raw) > maxPublishTokenFileBytes {
			credential.close()
			return credential, errors.New("publish token file must be a private bounded regular file")
		}
		credential.fileInfo = info
	} else {
		raw = []byte(os.Getenv("SEEN_REGISTRY_TOKEN"))
	}
	credential.token = strings.TrimSpace(string(raw))
	if len(credential.token) < 32 || len(credential.token) > 2048 || strings.ContainsAny(credential.token, "\r\n\t ") {
		credential.close()
		return publishCredential{}, errors.New("set a valid publish credential through a supported private credential source")
	}
	return credential, nil
}

func loadPublishToken(path string) (string, error) {
	credential, err := loadPublishCredential(path)
	if err != nil {
		return "", err
	}
	defer credential.close()
	return credential.token, nil
}

type registryPublishClient struct {
	origin, token string
	http          *http.Client
}

func newRegistryPublishHTTPClient() *http.Client {
	return &http.Client{
		Timeout:       90 * time.Second,
		CheckRedirect: func(_ *http.Request, _ []*http.Request) error { return http.ErrUseLastResponse },
	}
}

func (client *registryPublishClient) jsonRequest(ctx context.Context, method, path string, payload any, idempotent bool) (int, []byte, error) {
	body, err := json.Marshal(payload)
	if err != nil {
		return 0, nil, err
	}
	request, err := http.NewRequestWithContext(ctx, method, client.origin+path, bytes.NewReader(body))
	if err != nil {
		return 0, nil, err
	}
	request.Header.Set("Authorization", "Bearer "+client.token)
	request.Header.Set("Content-Type", "application/json")
	request.Header.Set("Accept", "application/json")
	if idempotent {
		// Bind the key to the exact deterministic request bytes. Reusing a logical
		// package/version key for changed metadata or source evidence must not turn
		// a materially different request into an accidental replay.
		request.Header.Set("Idempotency-Key", idempotencyKey(method, path, string(body)))
	}
	return client.do(request)
}

func (client *registryPublishClient) upload(ctx context.Context, path string, packed PackResult, required uploadRequiredHeaders) error {
	file, err := os.Open(packed.Path)
	if err != nil {
		return err
	}
	defer file.Close()
	request, err := http.NewRequestWithContext(ctx, http.MethodPut, client.origin+path, file)
	if err != nil {
		return err
	}
	request.ContentLength = packed.Length
	request.Header.Set("Authorization", "Bearer "+client.token)
	request.Header.Set("Content-Type", required.ContentType)
	request.Header.Set("X-Seen-Archive-Sha256", required.ArchiveSHA256)
	status, headers, body, err := client.doWithHeaders(request)
	if err != nil {
		return err
	}
	if status != http.StatusNoContent {
		return apiStatusError("upload archive", status, body)
	}
	if headers.Get("X-Seen-Archive-Sha256") != packed.SHA256 || headers.Get("ETag") != `"sha256:`+packed.SHA256+`"` {
		return errors.New("upload archive: registry returned mismatched archive confirmation headers")
	}
	return nil
}

func (client *registryPublishClient) do(request *http.Request) (int, []byte, error) {
	status, _, body, err := client.doWithHeaders(request)
	return status, body, err
}

func (client *registryPublishClient) doWithHeaders(request *http.Request) (int, http.Header, []byte, error) {
	response, err := client.http.Do(request)
	if err != nil {
		return 0, nil, nil, err
	}
	defer response.Body.Close()
	body, err := io.ReadAll(io.LimitReader(response.Body, maxAPIResponseBytes+1))
	if err != nil {
		return 0, nil, nil, err
	}
	if len(body) > maxAPIResponseBytes {
		return 0, nil, nil, errors.New("registry API response exceeded 4 MiB")
	}
	return response.StatusCode, response.Header.Clone(), body, nil
}

func validateReleaseReservation(reservation releaseReservationResponse, identity, version, visibility, manifestSHA256 string, capabilities []model.Capability, packed PackResult, now time.Time) error {
	if err := validateReleaseBinding(reservation.Release, identity, version, visibility, manifestSHA256, capabilities, packed); err != nil {
		return fmt.Errorf("registry returned an invalid release reservation: %w", err)
	}
	if reservation.Release.State.Lifecycle != "reserved" || reservation.Release.State.Availability != "unavailable" {
		return errors.New("registry returned an invalid release reservation state")
	}
	upload := reservation.Upload
	if !regexpUploadID.MatchString(upload.UploadID) || upload.Method != http.MethodPut {
		return errors.New("registry returned an invalid upload instruction")
	}
	expectedPath := "/packages/api/v1/uploads/" + upload.UploadID + "/archive"
	if upload.Path != expectedPath || upload.MaximumBytes != maximumArchiveUploadBytes {
		return errors.New("registry returned an invalid upload instruction")
	}
	if !regexpUTCTimestamp.MatchString(upload.ExpiresAt) {
		return errors.New("registry returned an invalid upload expiration")
	}
	expiresAt, err := time.Parse(time.RFC3339Nano, upload.ExpiresAt)
	if err != nil || !expiresAt.After(now) {
		return errors.New("registry returned an expired upload instruction")
	}
	required := upload.RequiredHeader
	if required.ContentType != "application/gzip" || required.ContentLength != packed.Length || required.ArchiveSHA256 != packed.SHA256 {
		return errors.New("registry returned upload headers that do not bind the reserved archive")
	}
	return nil
}

func validateReleaseBinding(release publishRelease, identity, version, visibility, manifestSHA256 string, capabilities []model.Capability, packed PackResult) error {
	if release.Package != identity || release.Version != version {
		return errors.New("release identity or version does not match the request")
	}
	if release.Archive.Format != "tar+gzip" || release.Archive.SHA256 != packed.SHA256 || release.Archive.CompressedBytes != packed.Length {
		return errors.New("release archive does not match the uploaded bytes")
	}
	if release.ManifestSHA256 != manifestSHA256 || !sameCapabilityList(release.Capabilities, capabilities) {
		return errors.New("release manifest or capabilities do not match the reservation")
	}
	if release.State.Visibility != visibility || release.State.Retention != "retained" {
		return errors.New("release visibility or retention does not match the request")
	}
	switch release.State.Lifecycle {
	case "reserved", "quarantined", "first-scanning", "delayed", "second-scanning", "ready", "active", "rejected":
	default:
		return errors.New("release lifecycle is invalid")
	}
	switch release.State.Availability {
	case "unavailable", "available", "yanked", "security-quarantined":
	default:
		return errors.New("release availability is invalid")
	}
	if release.State.Availability != "unavailable" && release.State.Lifecycle != "active" {
		return errors.New("resolver-visible release is not active")
	}
	return nil
}

func validateCompletedRelease(release publishRelease, identity, version, visibility, manifestSHA256 string, capabilities []model.Capability, packed PackResult) error {
	if err := validateReleaseBinding(release, identity, version, visibility, manifestSHA256, capabilities, packed); err != nil {
		return fmt.Errorf("registry returned an invalid completed release: %w", err)
	}
	if release.State.Lifecycle == "reserved" {
		return errors.New("registry returned an invalid completed release: lifecycle remained reserved")
	}
	return nil
}

func writePublishSubmission(output io.Writer, release publishRelease, archiveSHA256 string) {
	fmt.Fprintf(output, "Submitted %s@%s (%s); lifecycle=%s availability=%s\n", release.Package, release.Version, archiveSHA256, release.State.Lifecycle, release.State.Availability)
	if release.State.Visibility == "public" && release.State.Availability != "available" {
		fmt.Fprintln(output, "Public resolution remains unavailable until validation, the 72-hour delay, and the second scan complete.")
	}
}

func apiStatusError(action string, status int, body []byte) error {
	var envelope registryErrorEnvelope
	if json.Unmarshal(body, &envelope) == nil && envelope.Error.Code != "" {
		return fmt.Errorf("%s: registry returned %d %s: %s", action, status, envelope.Error.Code, envelope.Error.Message)
	}
	return fmt.Errorf("%s: registry returned HTTP %d", action, status)
}
func decodeStrictJSON(raw []byte, target any) error {
	decoder := json.NewDecoder(bytes.NewReader(raw))
	if err := decoder.Decode(target); err != nil {
		return err
	}
	var extra any
	if err := decoder.Decode(&extra); !errors.Is(err, io.EOF) {
		return errors.New("trailing JSON value")
	}
	return nil
}
func idempotencyKey(parts ...string) string {
	digest := sha256.Sum256([]byte(strings.Join(parts, "\x00")))
	return "idem_" + hex.EncodeToString(digest[:16])
}
func envOr(name, fallback string) string {
	if value := os.Getenv(name); value != "" {
		return value
	}
	return fallback
}
