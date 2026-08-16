# FFI

Modules: `ffi/c_types`, `ffi/cinterop`

The FFI stdlib modules provide C type mapping helpers and string conversion
utilities used with `extern fun` and `seen import-c`.

| Type or Function | Module | Purpose |
|------------------|--------|---------|
| `CTypeInfo` | `ffi/c_types` | C type size/alignment metadata |
| `CFunctionSignature` | `ffi/c_types` | Parsed C function signature helper |
| `CString` | `ffi/cinterop` | bootstrap wrapper around a Seen `String`; not a raw NUL-terminated `char *` |

For an actual native string boundary, use a reviewed shim or the runtime
`seen_str_to_cstr`/`seen_cstr_to_str` helpers and document who owns any allocated
copy. A Seen `String` is the `{ length, data }` `SeenString` representation.

Related guide: [Foreign Function Interface](../ffi.md).
