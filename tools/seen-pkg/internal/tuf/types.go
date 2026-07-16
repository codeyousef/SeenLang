package tuf

import "encoding/json"

type Signature struct {
	KeyID string `json:"keyid"`
	Sig   string `json:"sig"`
}

type Envelope struct {
	Signatures []Signature     `json:"signatures"`
	Signed     json.RawMessage `json:"signed"`
}

type Common struct {
	Type         string `json:"_type"`
	SpecVersion  string `json:"spec_version"`
	Version      int64  `json:"version"`
	Expires      string `json:"expires"`
	Environment  string `json:"environment"`
	RepositoryID string `json:"repository_id"`
}

type Key struct {
	KeyType string `json:"keytype"`
	Scheme  string `json:"scheme"`
	KeyVal  struct {
		Public string `json:"public"`
	} `json:"keyval"`
}

type Role struct {
	KeyIDs    []string `json:"keyids"`
	Threshold int      `json:"threshold"`
}

type RootSigned struct {
	Common
	ConsistentSnapshot bool            `json:"consistent_snapshot"`
	Keys               map[string]Key  `json:"keys"`
	Roles              map[string]Role `json:"roles"`
}

type FileMeta struct {
	Version int64             `json:"version"`
	Length  int64             `json:"length"`
	Hashes  map[string]string `json:"hashes"`
}

type TargetCustom struct {
	Environment       string             `json:"environment"`
	RegistryOrigin    string             `json:"registry_origin"`
	Package           string             `json:"package"`
	Version           string             `json:"version"`
	ArchiveSHA256     string             `json:"archive_sha256"`
	ArchiveFilename   string             `json:"archive_filename"`
	Visibility        string             `json:"visibility"`
	Lifecycle         string             `json:"lifecycle"`
	Retention         string             `json:"retention"`
	Availability      string             `json:"availability"`
	SourceProofSHA256 string             `json:"source_proof_sha256"`
	ProvenanceSHA256  string             `json:"provenance_sha256"`
	Dependencies      []TargetDependency `json:"dependencies"`
	Capabilities      []string           `json:"capabilities"`
	YankReason        string             `json:"yank_reason,omitempty"`
	IncidentID        string             `json:"incident_id,omitempty"`
	SecurityAction    string             `json:"security_action,omitempty"`
}

// TargetDependency is the signed transitive graph input. Alias belongs to the
// dependency edge; Package and RegistryOrigin identify the child node.
type TargetDependency struct {
	Alias          string   `json:"alias"`
	Package        string   `json:"package"`
	RegistryOrigin string   `json:"registry_origin"`
	Requirement    string   `json:"requirement"`
	Allow          []string `json:"allow"`
}

type TargetMeta struct {
	Length int64             `json:"length"`
	Hashes map[string]string `json:"hashes"`
	Custom TargetCustom      `json:"custom"`
}

type DelegatedRole struct {
	Name        string   `json:"name"`
	KeyIDs      []string `json:"keyids"`
	Threshold   int      `json:"threshold"`
	Terminating bool     `json:"terminating"`
	Paths       []string `json:"paths"`
}

type Delegations struct {
	Keys  map[string]Key  `json:"keys"`
	Roles []DelegatedRole `json:"roles"`
}

type TargetsSigned struct {
	Common
	Targets     map[string]TargetMeta `json:"targets"`
	Delegations *Delegations          `json:"delegations,omitempty"`
}

type SnapshotSigned struct {
	Common
	Meta map[string]FileMeta `json:"meta"`
}

type TimestampSigned struct {
	Common
	Meta map[string]FileMeta `json:"meta"`
}

// MetadataSet is one complete resolver metadata transaction.
type MetadataSet struct {
	Timestamp []byte
	Snapshot  []byte
	Targets   []byte
	Releases  []byte
	Security  []byte
}

// Repository is a completely verified, mutually bound metadata view.
type Repository struct {
	Timestamp TimestampSigned
	Snapshot  SnapshotSigned
	Targets   TargetsSigned
	Releases  TargetsSigned
	Security  TargetsSigned
}

// Selection preserves the authoritative role used for a target decision.
type Selection struct {
	Role   string
	Path   string
	Target TargetMeta
}
