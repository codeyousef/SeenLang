# Compute discovery and ownership

The `compute` modules provide explicit backend identities and bounded, owning
device reports. Importing them does not load a GPU driver, select a device, or
change the CPU default. They use the existing standard-library allocator and
introduce no native ABI symbols.

These additions belong to the standard-library component in the upcoming 0.23.0
release. They preserve the existing runtime ABI and do not replace the legacy
`graphics.gpu` device/context API.

`gpuBackendFromName` accepts exactly `none`, `vulkan`, `cuda`, `hip`, and `metal`.
There is no automatic backend selection. A `GpuDeviceCatalog` requires an
accelerator backend and a nonzero provider identity; `none` cannot create one.
The catalog holds metadata only. It does not authorize native operations or
represent an opened driver device or context.

`GpuLimits` bounds device, context, resource, metadata, and byte budgets. A catalog
owns a validated snapshot of these limits. `admit` consumes each `GpuDeviceInfo`
on both success and rejection. It validates geometry, feature bits, text lengths,
memory sizes, provider membership, and unique ordinals and UUIDs before growing
storage. A full catalog returns `gpu.limit`; malformed or duplicate reports
return `gpu.invalid`. These errors do not request retry.

`snapshot` returns an independent owning report, including its strings and
capabilities. It remains valid after the catalog closes. `close` releases admitted
reports exactly once and rejects further admission or snapshots. `release`
closes the catalog and consumes the catalog object and its limits. Catalogs and
reports are move-only; their creator owns mutation and cleanup. They provide no
implicit shared mutation or driver synchronization.

`GpuDevice.open` consumes an explicitly supplied `GpuDeviceProvider` and device
report on success and rejection. It validates provider membership and owns a
snapshot of the caller's limits. Provider dispatch owns its concrete payload and
dispatch box; shared memory cleanup releases each exactly once.

`createContext` accepts an immutable `OperationContext`. It checks cancellation,
deadline, context bounds and metadata allocation budget before calling the
provider, and observes the context again after native creation. Each returned
`GpuContext` is move-only and borrows its device core through a bounded,
generation-checked slot. A device with outstanding context borrowers returns
`gpu.busy` from `close` or `release` and remains owned by its caller.

Context destruction failure preserves the native handle and its borrower. A
late creation result is destroyed before returning cancellation or timeout. If
that destruction fails, the device retains the native handle without creating
an external borrower, and the error owns the cleanup failure as a cause. Device
close drains these retained handles before releasing the provider. It reports
partial cleanup progress and keeps ownership when a drain fails. Calls never
implicitly retry, wait for the whole device, or select another backend.

Successful `close` is idempotent; successful `release` also consumes the wrapper.
An inactive context can close after its device has been released, without
dereferencing the former owner. Exhausted generations retire slots instead of
wrapping to a stale identity. These owners are confined to their creator and do
not authorize sharing mutable provider state across threads.

The executable examples in `tests/production_gpu_runtime/` cover bounded backend
names, owned error messages, limit boundaries, hostile driver metadata,
cross-provider identities, duplicate reports, saturation, independent snapshots,
stale generations, metadata OOM, failed native destruction, late results and
cleanup on rejection and repeated close. The aggregate entry point is
`bash tests/misc_root_tests/seen_gpu_runtime_a.sh`. It runs fast, release/ThinLTO,
ASan, UBSan, and source coverage under the required containment wrapper.
The stress fixture performs 10,000 lifecycles with 22,500 native-call tokens,
injected destruction failures and late completions. It checks the live
allocation baseline after every cycle. Compile-negative fixtures reject owner
use after transfer and admission to cross-worker queues.
The metadata fuzzer mutates bounded ASCII, control and high-byte strings for
60 seconds with seed 1100, comparing backend names against fixed identities and
checking owned-result cleanup throughout the run.

These fixtures verify Seen metadata and lifecycle behavior on Linux x86-64
using a test-only provider with independent native-call counts. Actual driver
device/context creation and accelerator execution require native backend
implementation and verification. Other platforms have no execution
certification from these tests.
