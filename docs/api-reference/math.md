# Math

## Basic Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `abs` | `(x: Float) r: Float` | Absolute value (float) |
| `absInt` | `(x: Int) r: Int` | Absolute value (integer) |
| `min` | `(a: Float, b: Float) r: Float` | Minimum of two floats |
| `max` | `(a: Float, b: Float) r: Float` | Maximum of two floats |
| `minInt` | `(a: Int, b: Int) r: Int` | Minimum of two integers |
| `maxInt` | `(a: Int, b: Int) r: Int` | Maximum of two integers |
| `clamp` | `(x: Float, lo: Float, hi: Float) r: Float` | Clamp to range |
| `clampInt` | `(x: Int, lo: Int, hi: Int) r: Int` | Clamp integer to range |
| `clamp01` | `(x: Float) r: Float` | Clamp to [0, 1] |
| `sign` | `(x: Float) r: Float` | Sign (-1, 0, or 1) |
| `approxEqual` | `(a: Float, b: Float) r: Bool` | Approximate equality |

## Trigonometric Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `sin` | `(x: Float) r: Float` | Sine |
| `cos` | `(x: Float) r: Float` | Cosine |
| `tan` | `(x: Float) r: Float` | Tangent |
| `asin` | `(x: Float) r: Float` | Arcsine |
| `acos` | `(x: Float) r: Float` | Arccosine |
| `atan` | `(x: Float) r: Float` | Arctangent |
| `atan2` | `(y: Float, x: Float) r: Float` | Two-argument arctangent |

## Hyperbolic Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `sinh` | `(x: Float) r: Float` | Hyperbolic sine |
| `cosh` | `(x: Float) r: Float` | Hyperbolic cosine |
| `tanh` | `(x: Float) r: Float` | Hyperbolic tangent |

## Exponential and Logarithmic

| Function | Signature | Description |
|----------|-----------|-------------|
| `exp` | `(x: Float) r: Float` | e^x |
| `exp2` | `(x: Float) r: Float` | 2^x |
| `ln` / `log` | `(x: Float) r: Float` | Natural logarithm |
| `log2` | `(x: Float) r: Float` | Base-2 logarithm |
| `log10` | `(x: Float) r: Float` | Base-10 logarithm |
| `pow` | `(x: Float, y: Float) r: Float` | Power (x^y) |
| `sqrt` | `(x: Float) r: Float` | Square root |

## Rounding

| Function | Signature | Description |
|----------|-----------|-------------|
| `floor` | `(x: Float) r: Float` | Round down |
| `ceil` | `(x: Float) r: Float` | Round up |
| `round` | `(x: Float) r: Float` | Round to nearest |

## Angle Conversion

| Function | Signature | Description |
|----------|-----------|-------------|
| `degToRad` | `(deg: Float) r: Float` | Degrees to radians |
| `radToDeg` | `(rad: Float) r: Float` | Radians to degrees |

## Runtime Math Functions

These are mapped to LLVM intrinsics for maximum performance:

| Runtime Function | LLVM Intrinsic |
|-----------------|----------------|
| `__Sqrt(x)` | `llvm.sqrt.f64` |
| `__Sin(x)` | `llvm.sin.f64` |
| `__Cos(x)` | `llvm.cos.f64` |
| `__Exp(x)` | `llvm.exp.f64` |
| `__Log(x)` | `llvm.log.f64` |
| `__Log10(x)` | `llvm.log10.f64` |
| `__Fabs(x)` | `llvm.fabs.f64` |
| `__Pow(x, y)` | `llvm.pow.f64` |

Additional runtime functions:

| Function | Description |
|----------|-------------|
| `__Asin(x)` | Arcsine |
| `__Acos(x)` | Arccosine |
| `__Atan(x)` | Arctangent |
| `__Atan2(y, x)` | Two-argument arctangent |
| `__Sinh(x)` | Hyperbolic sine |
| `__Cosh(x)` | Hyperbolic cosine |
| `__Tanh(x)` | Hyperbolic tangent |

## Time

| Function | Signature | Description |
|----------|-----------|-------------|
| `__GetTime()` | `r: Float` | Current time in seconds since epoch |
| `nowSeconds()` | `r: Int` | Current time in seconds |
| `nowMillis()` | `r: Int` | Current time in milliseconds |
| `nowNanos()` | `r: Int` | Current time in nanoseconds |

## Integer Bounds

| Function | Return |
|----------|--------|
| `Int_min()` | Minimum Int value (-2^63) |
| `Int_max()` | Maximum Int value (2^63 - 1) |

## Example

```seen
fun main() {
    let angle = 45.0
    let rad = degToRad(angle)
    println("sin(45) = {sin(rad)}")
    println("cos(45) = {cos(rad)}")
    println("sqrt(2) = {sqrt(2.0)}")
    println("2^10 = {pow(2.0, 10.0)}")
    println("clamp(15, 0, 10) = {clamp(15.0, 0.0, 10.0)}")
}
```
