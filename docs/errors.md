# Structured errors

## Stable contract

ERR-001A defines `seen-error-v1`, the shared native Seen error model. Public
code imports `AsciiString`, `RetryClass`, `RedactionClass`, and `SeenError` from
`error`; bounded work imports `OperationLimits` and `OperationContext` from
`operation_context`. The bootstrap-recognized `core.error` and
`core.operation_context` modules own the single implementation, while the
top-level modules are explicit public re-exports.

An error contains canonical lowercase ASCII code, subsystem, and operation
identities; a UTF-8 message of at most 4096 bytes; at most eight recursively
bounded causes; an optional signed 64-bit native code; a `Never` or `Transient`
retry class; and a `Public` or `Sensitive` redaction class. Rendering replaces
sensitive messages with `[redacted]`, including nested causes. Validation never
truncates, repairs, retries, or infers an identity.

`OperationContext` carries explicit cancellation, a nonnegative monotonic
deadline, finite limits, and a canonical trace identity. Cancellation returns
`err.001a.cancelled`; malformed identities return `err.001a.invalid`; bound
violations return `err.001a.limit`. These policy errors are never retryable.
The source contract is platform-neutral and has no accelerator dependency;
Linux x86-64 is the currently certified execution platform.
