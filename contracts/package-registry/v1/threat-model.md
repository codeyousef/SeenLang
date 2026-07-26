# Seen package registry v1 threat model

The registry assumes uploaded source can be intentionally hostile, publisher
accounts and CI credentials can be compromised, scanners can fail, and clients
can receive stale or attacker-controlled network responses. Passing checks does
not establish that source is safe.

| Threat | Required mitigation | Residual risk | Detection | Owner | Response |
| --- | --- | --- | --- | --- | --- |
| Dependency confusion or registry substitution | Lock canonical identity, registry, version, digest, and signed metadata; never fall back to another registry | A trusted registry operator or root-key threshold can authorize bad metadata | Resolver telemetry and signature-failure alerts | Compiler + signing | Fail closed, revoke delegated metadata, rotate keys if needed |
| Typosquatting and confusables | ASCII canonical grammar, reserved/tombstoned comparison, pinned confusable skeletons, similarity review, 72-hour public delay | Similar names can still be legitimate or deceptive | Registration review queue and user reports | Trust and safety | Hold or reject registration; quarantine active release through signed metadata |
| Account takeover | Passkey-only Aether accounts, scoped roles, recent-auth checks, session/device revocation | Compromised authenticated device or coerced owner | Aether audit events, unusual publish/transfer alerts | Identity + trust and safety | Revoke sessions/credentials, freeze namespace, preserve evidence |
| Compromised publisher or CI token | Short-lived scoped service credentials, one-time display, rotation/revocation, immutable releases | Authorized malicious source may pass automated checks | Provenance drift, anomaly alerts, reports, delay window | Publisher + trust and safety | Revoke credential, quarantine releases, rotate dependent secrets |
| Archive traversal, links, devices, or bombs | Stream with compressed/expanded/file/path/depth limits; reject absolute/traversal paths, duplicates, case collisions, links, devices, and unsupported types before extraction | Parser/library implementation bugs | Hostile corpus, resource-limit metrics, sandbox alerts | Ingestion | Fail closed, isolate object, patch parser, replay corpus |
| Digest or blob substitution | SHA-256 content addressing, signed digest metadata, atomic promotion from quarantine, client rehash | SHA-256 implementation or signing compromise | Digest mismatch alarms and storage audit logs | Storage + signing | Stop promotion/download, quarantine, rotate affected signing role |
| Metadata rollback or freeze | TUF version/expiry checks, offline threshold root, environment-specific delegated online keys | Extended outage after metadata expiry | Expiry and rollback alerts | Signing + operations | Publish recovery metadata, rotate keys, follow compromise runbook |
| Signing-key compromise | Offline threshold root, narrowly scoped environment-specific online roles, KMS audit logs, short metadata expiry, rehearsed rotation and revocation | A root threshold compromise can authorize malicious trust metadata | KMS anomaly alerts, transparency/audit comparison, unexpected metadata-version alerts | Signing + security operations | Stop publication, revoke the delegated role, rotate keys, publish threshold-authorized recovery metadata, notify affected clients |
| Forged source provenance | Verify immutable forge/repository ID, installation identity, commit SHA, ref resolution, archive digest, and license; re-verify before activation | Compromised forge or repository maintainer can publish malicious source | Source-proof recheck and mismatch alerts | Forge verifier | Reject or quarantine; retain proof and notify namespace owner |
| Malicious source that passes scans | No install hooks or binaries, capability consent, delay, reporting, emergency quarantine | Static/dynamic analysis cannot prove benign behavior; FFI/unsafe can escape language policy | Reports, advisories, behavior telemetry where users opt in | Ecosystem security | Signed quarantine, advisory, fixed release; never rewrite history |
| Scanner escape or outage | Rootless isolated jobs, no credentials, no outbound network, CPU/memory/time limits, immutable inputs, fail closed | Kernel/runtime vulnerability | Sandbox and job-control alerts | Scanner platform | Disable promotion, rotate workload identity, rebuild environment |
| Cross-tenant private access or cache leakage | Deny-by-default authorization, non-enumerable metadata, short-lived authorized blob URLs, no public CDN caching | Application/IAM defect | Cross-tenant tests, access-log anomaly alerts | Registry service + cloud security | Revoke URLs/tokens, quarantine service, incident response and notification |
| Abuse or denial of service | Authenticated quotas, streaming limits, rate limits, idempotency keys, bounded pagination, budget alerts | Distributed low-rate abuse or storage exhaustion | Rate/quota/budget alerts | Operations + trust and safety | Throttle, suspend actor, preserve reports, adjust limits without weakening isolation |

Security reports receive an opaque identifier and acknowledgement. Evidence is
access-controlled and retained separately from public package data. Namespace
transfer and other high-risk operations require passkey reauthentication no
more than 15 minutes old. Appeals and reinstatements require an actor different
from the original enforcement actor; an emergency single-actor action requires
a time-bounded, audited waiver and retrospective review. Emergency quarantine
is immediately effective through signed metadata and always creates an incident
and audit event.
