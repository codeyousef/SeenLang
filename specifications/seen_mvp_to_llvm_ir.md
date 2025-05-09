# Mapping Specification: Seen MVP to LLVM IR

## Overview

This document specifies how Seen language constructs are mapped to LLVM IR. It serves as a reference for the code generation phase of the compiler, ensuring consistent translation from the Seen Abstract Syntax Tree (AST) to LLVM IR.

## Primitive Types

| Seen Type | LLVM IR Type |
|-----------|--------------|
| `int`     | `i64`        |
| `float`   | `double` / `f64` |
| `bool`    | `i1`         |
| `string`  | `i8*` (pointer to null-terminated character array) |
| `void`    | `void`       |

## Compound Types

| Seen Type | LLVM IR Type |
|-----------|--------------|
| `[T]` (array of T) | `[N x T]` (fixed-size array) or `%Array` (runtime-sized array struct) |

## Variable Declarations

### Immutable Variable (`val` / `ثابت`)

```seen
val x: int = 42;
```

```llvm
; Local variable (in a function)
%x = alloca i64
store i64 42, i64* %x

; Global variable (at module level)
@x = constant i64 42
```

### Mutable Variable (`var` / `متغير`)

```seen
var y: int = 10;
```

```llvm
; Local variable
%y = alloca i64
store i64 10, i64* %y

; Global variable
@y = global i64 10
```

## Function Declarations

```seen
func add(a: int, b: int) -> int {
    return a + b;
}
```

```llvm
define i64 @add(i64 %a, i64 %b) {
entry:
  %a.addr = alloca i64
  %b.addr = alloca i64
  store i64 %a, i64* %a.addr
  store i64 %b, i64* %b.addr
  %0 = load i64, i64* %a.addr
  %1 = load i64, i64* %b.addr
  %add = add i64 %0, %1
  ret i64 %add
}
```

## Control Flow

### If Statement

```seen
if (condition) {
    // then branch
} else {
    // else branch
}
```

```llvm
; Evaluate condition
%cond = ...

; Create basic blocks
br i1 %cond, label %then, label %else

then:
  ; then branch code
  br label %end

else:
  ; else branch code
  br label %end

end:
  ; continue with the rest of the function
```

### While Loop

```seen
while (condition) {
    // loop body
}
```

```llvm
; Jump to the condition check
br label %while.cond

while.cond:
  %cond = ... ; Evaluate condition
  br i1 %cond, label %while.body, label %while.end

while.body:
  ; loop body code
  br label %while.cond

while.end:
  ; continue with the rest of the function
```

## Expressions

### Binary Operations

| Seen Operator | LLVM IR Instruction (Integer) | LLVM IR Instruction (Float) |
|---------------|-------------------------------|------------------------------|
| `+`           | `add`                         | `fadd`                       |
| `-`           | `sub`                         | `fsub`                       |
| `*`           | `mul`                         | `fmul`                       |
| `/`           | `sdiv` (signed division)      | `fdiv`                       |
| `%`           | `srem` (signed remainder)     | N/A                          |
| `==`          | `icmp eq`                     | `fcmp oeq`                   |
| `!=`          | `icmp ne`                     | `fcmp one`                   |
| `<`           | `icmp slt` (signed less than) | `fcmp olt`                   |
| `>`           | `icmp sgt` (signed greater than) | `fcmp ogt`               |
| `<=`          | `icmp sle` (signed less or equal) | `fcmp ole`              |
| `>=`          | `icmp sge` (signed greater or equal) | `fcmp oge`           |
| `&&`          | `and`                         | N/A                          |
| `||`          | `or`                          | N/A                          |

Example:
```seen
a + b
```

```llvm
%0 = load i64, i64* %a
%1 = load i64, i64* %b
%add = add i64 %0, %1
```

### Unary Operations

| Seen Operator | LLVM IR Instruction (Integer) | LLVM IR Instruction (Float) |
|---------------|-------------------------------|------------------------------|
| `-`           | `neg`                         | `fneg`                       |
| `!`           | `xor ... true` or `icmp eq ... false` | N/A                 |

Example:
```seen
-a
```

```llvm
%0 = load i64, i64* %a
%neg = sub i64 0, %0
```

### Function Calls

```seen
print("Hello, World!");
```

```llvm
@.str = private constant [14 x i8] c"Hello, World!\00"
declare i32 @printf(i8* nocapture, ...)

; Inside a function
%0 = call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([14 x i8], [14 x i8]* @.str, i32 0, i32 0))
```

## Standard Library Functions

### Print Function (`println` / `اطبع`)

The `println` function is implemented via LLVM's foreign function interface to the C `printf` function:

```seen
println("Hello, World!");
```

```llvm
@.str = private constant [14 x i8] c"Hello, World!\00"
@.str.1 = private constant [4 x i8] c"%s\0A\00"
declare i32 @printf(i8* nocapture, ...)

; Inside a function
%0 = call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([4 x i8], [4 x i8]* @.str.1, i32 0, i32 0), i8* getelementptr inbounds ([14 x i8], [14 x i8]* @.str, i32 0, i32 0))
```

## "Hello World" Example

A complete "Hello, World!" program in Seen:

```seen
func main() {
    println("Hello, World!");
}
```

LLVM IR:

```llvm
@.str = private constant [14 x i8] c"Hello, World!\00"
@.str.1 = private constant [4 x i8] c"%s\0A\00"
declare i32 @printf(i8* nocapture, ...)

define i64 @main() {
entry:
  %0 = call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([4 x i8], [4 x i8]* @.str.1, i32 0, i32 0), i8* getelementptr inbounds ([14 x i8], [14 x i8]* @.str, i32 0, i32 0))
  ret i64 0
}
```

## Memory Management in MVP

In the MVP, all variables are stack-allocated (for locals) or statically allocated (for globals). The heap memory management features of Seen will be implemented in future versions.

## Optimization Strategy

The LLVM IR generated from Seen code is processed through LLVM's optimization pipeline, which includes:

1. **Function-level optimizations**:
   - Instruction combining
   - Constant propagation
   - Dead code elimination
   - Control flow graph simplification

2. **Memory optimizations**:
   - Promotion of memory to registers
   - Load/store optimization

3. **Machine-specific optimizations**:
   - Target-specific instruction selection
   - Register allocation

These optimizations are applied at different levels using LLVM's `PassManager` infrastructure.
