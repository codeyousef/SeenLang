A lot.

If your language already has **memory-safe ownership/regions** in the Vale/linear-usage neighborhood, then you’ve handled the classic lifetime bugs: use-after-free, double-free, many dangling-reference cases, and—if you also constrain shared mutable access—data races. Vale’s own design material explicitly ties regions and linear-aliasing to memory safety, and describes region isolation as part of making FFI boundaries safer as well. ([vale.dev][1])

The next big categories you can attack at compile time are these:

**1. Protocol / lifecycle bugs**
This is the biggest win after memory safety. Use **typestate** so APIs encode legal state transitions in the type system. Then the compiler can reject things like:

* reading from a file/socket before `open`
* using a handle after `close`
* `commit` after `rollback`
* `unlock` without holding the lock
* calling `finish()` twice
* sending on a closed channel
  Classical typestate is exactly about proving “operation B is only legal after operation A.” ([aegis-iisc.github.io][2])

**2. Resource-management bugs beyond memory**
Linear or affine types are not just for heap memory. They can enforce correct use of:

* file descriptors
* sockets
* GPU command buffers
* transactions
* temporary capabilities
* “must call” cleanup actions
  Linear types are specifically about tracking that a value is consumed exactly once, which is why they are so good for “acquire exactly once / release exactly once” style invariants. ([ghc.gitlab.haskell.org][3])

**3. Invalid-state bugs**
Make invalid states unrepresentable:

* `NonEmptyList<T>` instead of `List<T>` where emptiness is illegal
* `PositiveInt`, `NonZeroU32`, `Port(1..65535)`
* `Authenticated<User>` vs `AnonymousUser`
* `ParsedUrl` vs `RawString`
* distinct ID types so `UserId` cannot be passed where `OrderId` is expected
  This is not specific to regions, but once you already have ownership discipline, this is usually the next highest leverage move.

**4. Initialization bugs**
You can reject at compile time:

* use of uninitialized locals
* partially initialized aggregates
* missing field initialization
* “nullable by accident” references
  Definite-assignment checking plus non-null-by-default semantics eliminates a lot of extremely boring bugs.

**5. Exhaustiveness / impossible-branch bugs**
With exhaustive pattern matching and sealed algebraic data types, you can prevent:

* forgotten enum cases
* missing error branches
* illegal downcasts
* impossible default cases hiding future bugs
  This becomes especially strong when your state machine types are enums with explicit transitions.

**6. Bounds / range bugs**
At the simple end, checked indexing and slice APIs help. At the stronger end, **refinement-style** or proof-backed types can prove properties like:

* index is within bounds
* subtraction cannot underflow
* integer value stays inside a range
* loop preserves an invariant
  Languages and verifiers like Dafny are built around compile-time checking of preconditions, postconditions, and invariants. ([Dafny][4])

**7. Arithmetic bugs**
You can prevent or surface:

* integer overflow
* underflow
* division by zero
* lossy numeric conversions
* signed/unsigned confusion
  This is usually done with checked arithmetic by default, explicit narrowing conversions, and range/refinement types.

**8. Effect bugs**
An **effect system** lets the compiler reject functions that do more than their type says:

* “pure” code doing I/O
* code that may throw inside a no-throw context
* blocking calls inside non-blocking contexts
* allocation inside no-allocation code
* unexpected global state mutation
  Koka is a concrete example of a language that tracks effects in function types, distinguishing pure from effectful computations. ([koka-lang.github.io][5])

**9. Concurrency misuse bugs**
Beyond raw data races, you can statically prevent:

* sending non-sendable values across threads
* touching thread-confined objects from the wrong executor/thread
* aliasing mutable state across tasks
* actor isolation violations
  With extra machinery, you can also prevent some deadlocks by encoding lock ordering or by banning certain blocking patterns in certain effects/capabilities. That part is more design-heavy, but it is feasible.

**10. Obligation bugs**
This is underused and very powerful. You can encode “if you do X, you must later do Y”:

* begin transaction → must commit or rollback
* create temp file → must delete or persist
* spawn joinable task → must join or detach
* borrow capability → must return it
  Vale’s materials describe “Higher RAII” in this general direction: enforcing that a required future operation is not forgotten. ([vale.dev][6])

**11. Capability / privilege bugs**
A capability-oriented design can make it impossible to perform sensitive actions unless the function has the right token/capability:

* filesystem access
* network access
* env/process access
* randomness
* clock/time
* UI-thread-only APIs
  This prevents a lot of ambient-authority mistakes and makes security boundaries much more explicit.

**12. FFI contamination bugs**
If your language has unsafe interop, you can still compile-time contain some damage by forcing unsafe values into quarantined regions/capability domains, or requiring copy/serialize boundaries before they interact with safe data. Vale explicitly discusses region isolation and safer FFI boundaries as part of its safety story. ([vale.dev][7])

**13. Domain-specific semantic bugs**
This is where languages get really good:

* units-of-measure to stop meters/feet/seconds mixups
* SQL/HTML/regex/query DSLs to prevent malformed or unsafe stringly APIs
* typed routes, typed formats, typed permissions
* cryptographic key-type separation
* endian/encoding distinctions
* physical-dimension correctness
  These are often easier wins than full dependent types.

Where I’d focus, in order, if I were designing your language:

1. **Typestate**
2. **Non-null + definite initialization**
3. **Exhaustive ADTs**
4. **Distinct newtypes for IDs/units/capabilities**
5. **Checked arithmetic**
6. **Effect tracking**
7. **Refinement/contracts for hot spots**
8. **Capability-based security model**

That stack gives you a very large safety jump without immediately turning the language into a theorem prover.

What you generally **cannot** eliminate wholesale at compile time:

* arbitrary business-logic mistakes
* wrong algorithms
* bad UX/product assumptions
* performance pathologies in general
* starvation/liveness bugs in the general case
* numerical instability
* bugs hidden inside unrestricted `unsafe`/foreign code unless you quarantine them very aggressively

A good way to think about it is:

* **Regions/ownership** prevent “this value lives wrong.”
* **Typestate** prevents “this value is used in the wrong phase.”
* **Effects/capabilities** prevent “this code does the wrong kind of thing.”
* **Refinements/contracts** prevent “this value has the wrong shape/range.”
* **ADTs/newtypes** prevent “this program models the domain sloppily.”

That combination is where languages start feeling *actually* safer, not just memory-safe.

I can sketch a concrete feature roadmap for your language next—cheap features, medium-complexity features, and research-grade features.

[1]: https://vale.dev/?utm_source=chatgpt.com "The Vale Programming Language"
[2]: https://aegis-iisc.github.io/assets/pdf/phd_thesis.pdf?utm_source=chatgpt.com "Typestates and Beyond: Verifying Rich Behavioral ..."
[3]: https://ghc.gitlab.haskell.org/ghc/doc/users_guide/exts/linear_types.html?utm_source=chatgpt.com "6.4.22. Linear types"
[4]: https://dafny.org/latest/DafnyRef/DafnyRef?utm_source=chatgpt.com "Dafny Reference Manual"
[5]: https://koka-lang.github.io/koka/doc/book.html?utm_source=chatgpt.com "The Koka Programming Language"
[6]: https://vale.dev/linear-aliasing-model?utm_source=chatgpt.com "The Linear-Aliasing Model"
[7]: https://vale.dev/memory-safe?utm_source=chatgpt.com "The Most Memory Safe Native Programming Language"

## Seen implementation notes (repo findings)

The current Seen design can absorb a large part of the list above **without**
adding a whole new type-system model first. The best fit is to build on the
features the compiler and stdlib already expose:

* **Ownership / lifecycle**: use `@move` types plus `own` / `move` / `borrow`
  to model single-owner resources and phase transitions. This is the most
  natural way to encode typestate in Seen today.
* **Cleanup obligations**: use `defer` and `errdefer` for must-clean-up paths,
  and use `@c_resource("destroy_fn")` for FFI-backed handles that need
  automatic scope-exit destruction.
* **Invalid states**: use `distinct Name = Base` or the existing
  `@distinct("Base") class Name { var value: Base }` pattern, then expose
  smart constructors returning `Result<T, String>`.
* **Nullable / optional state**: use `T?` and `Option<T>` explicitly. Prefer
  constructors or factories that fully initialize values instead of relying on
  ambient defaults.
* **State machines / impossible branches**: use enums plus `match` / `when`
  for explicit states. Exhaustiveness checking is a good next compiler feature,
  but the modeling style already fits Seen well.
* **Range / arithmetic safety**: rely on smart constructors, `Result`, and the
  existing safety flags like `--bounds-check`, `--panic-on-overflow`,
  `--warn-uninit`, `--null-safety`, and `--warn-unused-result`.
* **Concurrency misuse**: use ownership and `@move` for uniqueness, and treat
  `@send` / `@sync` as the intended annotation surface for thread-safety
  enforcement.
* **Capabilities / effects**: the repo already has a capability checker based
  on `@using(Token)` with `FileToken`, `NetToken`, `ProcessToken`, `EnvToken`,
  `DynLoadToken`, and `AllToken`. This is the cleanest way to implement an
  "effect-lite" model in the current language design.
* **FFI quarantine**: keep `extern` declarations in narrow boundary modules,
  immediately wrap raw handles/pointers in safe Seen types, and only expose the
  wrapped API outward.
* **Domain-specific correctness**: use distinct wrappers, smart constructors,
  extension methods, and operator overloads for units, IDs, parsed values,
  permissions, encodings, and similar semantic distinctions.

### Recommended implementation order for Seen itself

1. Typestate encoded with `@move` wrappers and transition functions
2. Resource wrappers using `defer`, `errdefer`, and `@c_resource`
3. Definite-initialization and nullability enforcement behind existing flags
4. Enum exhaustiveness checking
5. Broaden capability checking beyond opt-in token usage and add richer effect kinds
6. Extend `@send` / `@sync` from structural validation to concurrency-boundary checks

### Practical summary

If you want to implement the bug-prevention ideas in this file using the
existing Seen language design, prefer:

* **move-only nominal wrappers** for protocols and resources
* **`Result` / `Option` + smart constructors** for invalid states and ranges
* **enums + pattern matching** for explicit state machines
* **decorators + dedicated checker passes** for capabilities, concurrency rules,
  and other policy-like constraints

That path matches the way the Seen compiler is already structured: specialized
checker modules plus lightweight syntax and annotations, rather than one large
research-heavy type system added all at once.
