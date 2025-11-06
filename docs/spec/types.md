# Seen Language — Type System Specification

## 1. Overview
- Hindley–Milner inference with rank-1 polymorphism and monomorphisation at codegen.
- Traits provide ad-hoc polymorphism; sealed traits restrict downstream implementations.
- Typestates via phantom parameters express stateful protocols (e.g., Vulkan lifecycles).
- Nullability requires explicit `T?`; safe-call (`?.`) and Elvis (`?:`) handle optional values.

## 2. Type Forms
| Form | Description |
| --- | --- |
| `T` | Named types: structs, enums, traits, aliases. |
| `T<U, V>` | Generic instantiation; inference infers type arguments when unambiguous. |
| `&T` / `mut &T` | Shared / mutable references tied to region lifetimes. |
| `Ptr<T>` | Raw pointer type; no aliasing rules enforced. |
| `Result<T, E>` | Standard result type for recoverable errors. |
| `Option<T>` | Nullable optional; compiler desugars `T?` to `Option<T>`. |
| `[T; N]` | Fixed-size array with compile-time length. |
| `Slice<T>` | View into contiguous memory with runtime length. |
| `Fn(A, B) -> R` | Function pointers/closures capturing environment references. |

## 3. Generics & Inference
- **Type parameters** declared with `fun foo<T>(x: T) -> T`.
- **Traits bounds**: `fun draw<T: Renderable>(x: T)`.
- **Associated types**: `trait Iterator { type Item; fun next(self) -> Option<Self::Item>; }`.
- **Inference rules**:
  - Local type inference saturates function bodies; unspecified types default to `_`.
  - Trait resolution prefers concrete impls; conflicting impls raise deterministic diagnostics with source references.
  - Monomorphisation occurs before codegen; each instantiation is hashed for determinism.

## 4. Traits & Implementations
- Traits may include methods (with default implementations) and associated types/constants.
- **Sealed traits**: `sealed trait ShaderStage {}` restricts impls to defining crate; external impl attempts error.
- Orphan rule: trait `T` can be implemented for type `S` if either `T` or `S` is defined in current crate.
- Blanket impls allowed but must be deterministic; overlapping impls flagged with precise spans.

## 5. Typestates & Phantom Types
```seen
sealed trait CmdState {}
struct Recording: CmdState {}
struct Executable: CmdState {}

struct CommandBuffer<S: CmdState> {
  raw: Ptr<VkCommandBuffer>,
  state: Phantom<S>,
}

fun end(cb: CommandBuffer<Recording>) -> CommandBuffer<Executable> { /* ... */ }
```
- `Phantom<T>` markers have zero runtime size.
- State transitions are encoded via type parameters; illegal transitions fail during type checking.
- The compiler verifies that phantom parameters do not affect layout hashing to preserve ABI stability.

## 6. Borrowing & Regions
- References `&T`/`mut &T` track region lifetimes (see `regions.md`).
- Escape analysis prevents references from outliving their region; diagnostics cite both source of borrow and escape site.
- Async tasks capture by move; awaiting while holding `mut &` borrows is rejected with actionable error metadata.

## 7. Pattern Matching & Exhaustiveness
- Enums and `match` expressions enforce exhaustiveness; wildcard `_` is permitted but discouraged in deterministic mode unless explicitly annotated.
- Compiler performs nullability refinement: `if let Some(x) = opt { /* x: T */ }`.
- `is` operator triggers smart casts: after `if value is Mesh`, inside branch `value` has type `Mesh`.

## 8. Type Inference Diagnostics
- Errors surface with principal type explanations (`expected T`, `found U`) and include trait obligation stacks.
- Formatter preserves explicit type annotations supplied by the user; no inference rewrites occur post-formatting.
- Deterministic builds ensure inference order does not affect emitted diagnostics; regression hashes exist in `tests/type_inference`.
