# Security

Module: `security/enclave`

Security modules expose enclave/TEE-oriented data structures and attestation
helpers.

Notable types include `TEEType`, `TEEStatus`, `AttestationType`,
`SealPolicy`, `EnclaveHandle`, `AttestationReport`, `SealedData`, and `TEE`.

Convenience helpers include `getTEEType`, `isTEEAvailable`,
`isTEEHardwareBacked`, `sealData`, `unsealData`, `getLocalAttestation`, and
`getRemoteAttestation`.
