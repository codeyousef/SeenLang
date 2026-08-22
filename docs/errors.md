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

## Typed error categories

ERR-001B defines `seen-typed-error-v1`. `SeenErrorKind` has exactly eight
categories: `Os`, `Io`, `Process`, `Network`, `Timeout`, `Cancelled`, `Parse`,
and `Resource`. A bounded `TypedErrorSpec` is converted with `typedSeenError`,
or with `typedSeenErrorInContext` when cancellation and operation limits must
be checked first. The resulting stable code and subsystem are
`err.001b.<category>` and `<category>` respectively.

Callers choose `Never` or `Transient` explicitly for OS, I/O, process,
network, timeout, and resource failures; the library does not infer retry from
an operating-system number. Cancellation and parse/policy failures must use
`Never` and fail validation otherwise. Native error numbers remain optional
signed 64-bit values. Sensitive messages, including paths, render only as
`[redacted]`.

## String-error and sentinel migration

ERR-001C defines `seen-error-api-migration-v1`. Public fallible `Result` APIs
in the standard library and bootstrap verifier now return `SeenError`; string
errors and the bespoke `FsFileResult` carrier are rejected by a repository
audit. Callers propagate the original structured value, so native codes,
retry classification, and redaction survive composition. `seenFailure` is the
bounded constructor for an immediate failure; bounded operations use
`typedSeenErrorInContext` to apply cancellation and operation limits first.

The frozen compiler bootstrap still consumes eight explicitly inventoried
primitive file helpers whose scalar return values are part of the current
bootstrap ABI. They are not the error contract for new code: ordinary callers
use `FsFile` or the checked text-read carrier, both of which expose
`SeenError`. The audit pins these bootstrap signatures exactly so the exception
cannot grow silently; replacing that frozen ABI requires a bootstrap migration,
not a deprecated public bridge.

## Retry cancellation exhaustion and redaction

ERR-001D defines `seen-error-policy-v1`. `classifySeenError` maps a validated
error and explicit bounded attempt state to exactly one `ErrorDisposition`:
`PermanentFailure`, `RetryAllowed`, `CancellationObserved`, or
`RetryExhausted`. A transient failure retries
only while `attempt < maxAttempts`; reaching the bound is exhaustion rather
than another implicit retry. Cancellation always wins over retry policy and a
cancelled error marked transient is rejected as contradictory input.

Redaction is orthogonal to disposition. Sensitive messages become the exact
text `[redacted]`; public messages remain unchanged. The decision records the
attempt and bound, never sleeps, schedules, retries, truncates, or repairs the
input, and accepts at most 1024 attempts.
