# Metaprogramming

Seen 0.15.0 provides a focused set of compile-time expressions and generated
class features. Treat parser acceptance of an annotation as separate from a
guarantee that every lowering path implements it.

## Compile-time evaluation

### `comptime if`

Target predicates are the most established conditional-compilation surface:

```seen
comptime if (target.isLinux) {
    println("linux")
} else {
    println("another target")
}
```

Available target properties include `pointerSize`, `isLinux`, `isWindows`,
`isMacos`, `isIos`, `isSimulator`, `isAarch64`, `isRiscv64`, `hasSSE2`,
`hasAVX2`, `hasAVX512`, `hasFMA`, `hasNeon`, and `hasRVV`.

### Compile-time parameters

Mark a parameter `comptime` when its value selects code during compilation:

```seen
fun process(comptime width: Int, value: Int) r: Int {
    comptime if (width == 4) {
        return value * 4
    }
    return value
}
```

### Blocks and assertions

`comptime { ... }` supports the compiler's evaluated integer/string expressions,
local bindings, and simple control flow such as `while`. `comptime assert`
rejects a compile-time-false condition:

```seen
comptime assert(target.pointerSize == 8, "this program needs a 64-bit target")
```

The current evaluator is deliberately limited. Arbitrary recursive
`comptime fun` execution, heap-backed array construction, I/O, and general
runtime calls are not established 0.15.0 features.

## Derive and reflection

The compiler has tested lowering for derives including `Clone`, `Hash`, `Eq`,
`Debug`, `Serialize`, `Deserialize`, and `Json`:

```seen
@derive(Clone, Hash, Eq, Debug)
class Point {
    var x: Int
    var y: Int
}
```

`@reflect` and `@derive(Reflect)` generate the supported type/field metadata
helpers. See the focused API pages for reflection, JSON, and binary encoding;
their generated method names and supported field types are the contract to
follow.

## Decorators

Decorators use `@name` or `@name(arguments)` syntax. Active compiler paths use,
among others:

- `@async` for LLVM coroutines
- `@inline` and `@always_inline` for function attributes
- `@intrinsic("llvm.name")` for compiler-recognized LLVM intrinsics
- `@compute(workgroup: 64)` for experimental compute-shader extraction
- `@test` as test metadata
- `@derive(...)`, `@reflect`, `@repr(C)`, and `@packed` on types
- `@send`, `@sync`, `@move`, and `@nondeterministic` as type/policy markers
- `@cfg("feature")` for feature-gated functions

Some markers primarily record metadata or enable one narrow codegen path; they
do not automatically provide a complete runtime framework. For example,
`@send` and `@sync` do not add missing parallel-loop capture lowering.

Feature names can be supplied in `Seen.toml`; the compiler source also accepts
`--feature=<name>` on compile paths, though this flag is not listed in the
compact top-level help in 0.15.0.

## User macros

The repository contains experimental macro infrastructure, but the fabricated
`macro name { ... }` syntax found in older examples is not a shipped language
surface. Prefer tested derives and `comptime` forms. Verify any custom macro
path against focused compiler tests before relying on it.

## Related

- [Language Guide](language-guide.md)
- [Known Limitations](known-limitations.md)
- [API Reference: Reflect](api-reference/reflect.md)
- [API Reference: JSON](api-reference/json.md)
- [API Reference: Binary](api-reference/binary.md)
