# Standard Library: Print Function API Specification

## Overview

This document specifies the API for the print function in the Seen programming language's standard library. The print function is a core part of the MVP functionality, allowing programs to output text to the console.

## API Names

The print function is available with both English and Arabic names:

| Language | Function Name | Description |
|----------|---------------|-------------|
| English  | `println`     | Print a string followed by a newline |
| Arabic   | `اطبع`        | Arabic alias for `println` |

## Function Signatures

### Basic String Printing

```
// English API
func println(text: string) -> int

// Arabic API
func اطبع(text: string) -> int
```

### Specialized Type Printing

```
// Integer printing (English)
func println_int(value: int) -> int

// Integer printing (Arabic)
func اطبع_رقم(value: int) -> int

// Float printing (English)
func println_float(value: float) -> int

// Float printing (Arabic)
func اطبع_عدد(value: float) -> int

// Boolean printing (English)
func println_bool(value: bool) -> int

// Boolean printing (Arabic)
func اطبع_منطقي(value: bool) -> int
```

## Parameters

### `text: string` / النص: سلسلة

The text to be printed. This must be a valid string. In the current implementation, this is a null-terminated C-style string.

### `value: int | float | bool`

The value to be printed for the specialized printing functions.

## Return Value

All print functions return an integer value, which represents:

- The number of characters successfully printed, or
- A negative value if an error occurred

This matches the behavior of the underlying C `printf` function.

## Implementation Details

The print functions in the Seen standard library are implemented as FFI wrappers around the C standard library's `printf` function. This allows for efficient and reliable string formatting and output.

### Memory Safety

The string passed to `println` must be a valid, null-terminated string. The function does not perform any memory safety checks, as these should be handled by the Seen compiler.

### Thread Safety

The print functions have the same thread safety guarantees as the underlying C `printf` function. Multiple threads calling print functions simultaneously may result in interleaved output.

## Examples

### Basic String Printing

```seen
// English
println("Hello, World!");  // Prints: Hello, World!

// Arabic
اطبع("مرحبا بالعالم!");   // Prints: مرحبا بالعالم!
```

### Specialized Type Printing

```seen
// Integer
println_int(42);     // Prints: 42
اطبع_رقم(42);        // Prints: 42

// Float
println_float(3.14); // Prints: 3.140000
اطبع_عدد(3.14);      // Prints: 3.140000

// Boolean
println_bool(true);  // Prints: true
اطبع_منطقي(false);   // Prints: false
```

## Error Handling

The print functions do not throw exceptions or return error objects. Instead, they follow the C convention of returning a negative value on error. In practice, print errors are rare and often indicate serious system issues.

## Future Enhancements

In future versions of the Seen language, the print function API may be enhanced to include:

1. Format string support for variable interpolation
2. Additional data type support
3. Output redirection options
4. Error handling improvements
5. Unicode and localization features

These enhancements will maintain backward compatibility with the MVP print function API.
