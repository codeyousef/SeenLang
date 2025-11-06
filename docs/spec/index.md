# Seen Language Specification Index

> This `/docs/spec` directory slices the language spec into focused chapters to satisfy the MVP POST‑1 requirement. Each chapter remains deterministic, versioned alongside the compiler, and cross‑linked for LSP usage.

- [Lexical Structure](./lexical.md) — source encoding, tokens, NFC policy, visibility modes.
- [Grammar](./grammar.md) — canonical Seen EBNF plus precedence tables.
- [Types](./types.md) — type system, generics, traits, typestates, inference rules.
- [Regions & Memory](./regions.md) — region model, RAII, generational handles, async constraints.
- [Errors & Diagnostics](./errors.md) — recoverable vs aborting flow, diagnostic guarantees.
- [FFI & ABI](./ffi_abi.md) — layout attributes, symbol policy, interop guarantees.
- [Numerics & SIMD](./numerics.md) — numeric primitives, float environment, SIMD appendix.

> Companion references: [`docs/Seen Design Document.md`](../Seen%20Design%20Document.md) for architecture & rationale, [`docs/Seen Syntax Design Document.md`](../Seen%20Syntax%20Design%20Document.md) for idioms, and [`docs/Seen Language Spec.md`](../Seen%20Language%20Spec.md) for the monolithic legacy spec.
