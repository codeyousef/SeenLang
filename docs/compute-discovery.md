# Compute discovery metadata

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

The executable examples in `tests/production_gpu_runtime/` cover bounded backend
names, owned error messages, limit boundaries, hostile driver metadata,
cross-provider identities, duplicate reports, saturation, independent snapshots,
and cleanup on rejection and repeated close. The aggregate entry point is
`bash tests/misc_root_tests/seen_gpu_runtime_a.sh`. It runs fast, release/ThinLTO,
ASan, UBSan, and source coverage under the required containment wrapper.

These fixtures verify metadata behavior on Linux x86-64. Driver device/context
creation and accelerator execution require separate provider implementation and
verification. Other platforms have no execution certification from these tests.
