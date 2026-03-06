# seen_std

Bootstrap-friendly Seen standard library surface that the self-hosted compiler
relies on. The goal is to capture the minimal Option/Result/prelude helpers,
collections shims, async task handles, and FFI glue so Stage-1 no longer sprinkles
ad-hoc definitions throughout the tree.

This is **not** the final public stdlib—it mirrors just enough functionality to keep
the bootstrap pipeline deterministic. Once the ABI guard workflow lands we can lock
the module hashes recorded in `Seen.lock` and start shipping packaged releases
(`libseen_std.seenpkg`).
