# Qwen prerequisite foundations

Seen 0.14 adds the bounded host and accelerator contracts required before the
Qwen3.8-27B engine can be implemented. These are infrastructure APIs, model
locks, and certification gates; the model engine itself is not part of 0.14.

## Storage and geometry

`inference/scalars` defines stable C-layout storage representations for
`Float16`, `BFloat16`, `Float8E4M3`, and `Float8E5M2`, plus checked 64-bit
tensor geometry. The existing fixed-width integer names remain canonical.
`inference/span` provides checked borrowed spans and bounded 2/3/4/8-bit
packed views. `memory/aligned_buffer` owns 16–256-byte-aligned storage and
closes idempotently. The matching public C declarations are in
`seen_runtime/seen_inference.h`.

## Large immutable files and manifests

`memory/mapped_file` owns read-only files and bounded mapped windows with
offsets above 4 GiB. A file cannot close while windows remain active. Window
validation detects truncation; sequential, random, prefetch, huge-page,
page-lock, and Linux NUMA requests return stable results rather than silently
falling back. Unsupported advisory controls are explicit.

`crypto/sha256` is incremental and reads files in bounded chunks. Digests use
canonical lowercase hexadecimal and can be parsed for fixed-work comparison.
`json/strict` rejects invalid UTF-8, duplicate names, unpaired surrogates,
limit violations, and non-canonical or overflowing `Int64` text.
`json/canonical` deterministically orders object names by UTF-16 code units for
canonical manifest output.

`formats/safetensors` validates the little-endian header length, bounded
metadata, exact geometry, dtypes, non-overlap, local shard-index paths, and
deterministic name ordering. Tensor byte windows remain zero-copy and are
invalidated by explicit owner closure.

## CUDA feature boundary

The CUDA subsystem is separate from Vulkan and is built only with
`-DSEEN_ENABLE_CUDA=ON`. Disabled configuration stops before CUDA language or
SDK discovery, so ordinary CPU builds do not locate or link NVIDIA libraries.
The initial backend is Linux x86-64, targets compute capability 8.9, and has
`experimental-hardware` maturity.

The versioned `seen_cuda_*` ABI owns only device/resource adaptation. Seen owns
allocation bounds, move-only lifetime, fallback policy, algorithm identity,
and diagnostics. It covers devices, device and pinned memory, asynchronous
copies, streams, events, graph capture/replay/update, and deterministic
cuBLASLt F16/BF16 algorithm selection. Silent fallback is not supported.

## Frozen Qwen inputs and evidence

`projects/seen_ml/qwen38` contains the strict model/source lock contracts,
exact configuration validator, tensor-role classifier, tokenizer asset pins,
content-addressed artifact promotion, and environment capture. The official
model revision and every required file are immutable inputs. The concrete
source lock also pins the Qwen and Transformers references and the initial
llama.cpp/vLLM comparison revisions. Remote model code is never executed.

Benchmark evidence requires at least 30 measured samples, correctness and
quality state, host and VRAM usage, transfers, effective bandwidth, qualified
hardware conditions, and the shared five-percent kernel regression gate.
Generated model assets and machine evidence remain outside Git.

## Certification

Release acceptance is serial and runs inside the repository's current-memory
derived, swap-disabled hard scope. It includes a 60 GiB sparse mapping and
truncation test, hostile JSON/Safetensors cases, native-boundary inventory,
CPU-only dependency scanning, and real RTX 4090 memory, stream, event, graph,
F16, and BF16 cuBLASLt execution. Compile-only evidence cannot replace the
hardware result.
