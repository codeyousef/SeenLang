# FFI

Modules: `ffi/c_types`, `ffi/cinterop`

The FFI stdlib modules provide C type mapping helpers and string conversion
utilities used with `extern fun` and `seen import-c`.

| Type or Function | Module | Purpose |
|------------------|--------|---------|
| `CTypeInfo` | `ffi/c_types` | C type size/alignment metadata |
| `CFunctionSignature` | `ffi/c_types` | Parsed C function signature helper |
| `CString` | `ffi/cinterop` | C string wrapper |

Related guide: [Foreign Function Interface](../ffi.md).
