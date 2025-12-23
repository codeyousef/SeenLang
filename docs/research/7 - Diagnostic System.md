# Design Proposal for the [[Seen]] Programming Language Diagnostic System

## 1. Seen Diagnostic System: Foundational Philosophy and Guiding Principles

The diagnostic system of the Seen programming language is conceived not merely as an error-reporting mechanism, but as a cornerstone of the developer experience and a critical pedagogical tool. Given Seen's ambition to offer a significantly improved developer experience over languages like Rust, particularly concerning its novel automated memory management and concurrency models, the diagnostic system must be built upon a foundation of clarity, actionability, consistency, and learnability. This philosophy guides every aspect of the diagnostic design, aiming to empower developers, reduce frustration, and facilitate a deeper understanding of the language's unique features.

### 1.1. Core Tenets: Clarity, Actionability, Consistency, and Learnability

These four tenets form the bedrock of Seen's diagnostic philosophy:

- **Clarity:** Diagnostic messages must be unambiguous and readily comprehensible, even for programmers encountering Seen or systems programming for the first time. Achieving clarity necessitates avoiding opaque compiler jargon and internal implementation details whenever possible, instead favoring plain, direct language expressed in terms of the user's own source code.1 Messages should be understandable even under non-ideal conditions, such as on a small or unclean screen, or when the programmer is fatigued.3 The cognitive load imposed by technical jargon can be a significant barrier, especially when users might also be navigating Seen's bilingual keyword system. Therefore, Seen's diagnostics must actively strive to use terminology that aligns with the user's conceptual model of the language features, rather than abstract theoretical terms. For instance, explaining a memory error should relate to Seen's specific memory management concepts in user-understandable terms.
    
- **Actionability:** Every diagnostic should guide the user toward a resolution.1 It is insufficient to merely state that an error exists; the system must provide pertinent information to help the user answer the crucial questions: "How do I fix this?" or, for warnings, "Is this a genuine problem, and if so, how do I address it?".1 This principle mandates the inclusion of concrete suggestions and helpful context within the diagnostic output.
    
- **Consistency:** A predictable and uniform presentation of diagnostics is essential for reducing cognitive load and building user trust. All diagnostic messages, regardless of type or origin within the compiler, should adhere to a consistent structure, terminology, and stylistic conventions.2 This includes consistent formatting, such as starting messages with a lowercase letter and omitting trailing punctuation, as practiced by `rustc` and Clang 3, and extends to maintaining consistency between the English and Arabic versions of the diagnostics.
    
- **Learnability (Pedagogy):** Diagnostics serve as a primary educational resource within the Seen ecosystem.3 This is particularly vital for Seen's novel automated memory management and concurrency models, which are intended to be intuitive. To achieve this perceived intuitiveness, diagnostics must go beyond simply identifying errors; they must explain the _reason_ behind the error in the context of Seen's design principles. They function as embedded, contextual documentation, bridging the gap between the user's understanding and the language's specific semantics. Failing to provide clear, pedagogical explanations for novel features risks incurring "pedagogical debt," hindering user adoption and undermining the core goal of an improved developer experience.
    

### 1.2. Diagnostics as a Pedagogical Instrument for Seen's Novel Features

Seen's unique selling points – its potentially more intuitive memory and concurrency models – require diagnostics specifically designed to teach these systems.

- **Explaining Automated Memory Management:** When Seen's automated memory analysis flags an issue or requires manual intervention (if applicable), the diagnostic message must demystify the process. It should articulate the analysis's objective, the specific rule or invariant violated by the user's code, the relevant state of memory or variables leading to the failure, and how this relates to Seen's core memory safety principles. Explaining the failure of an _automated_ system presents a unique challenge compared to explaining the misuse of a manual one. The system must clarify why it couldn't find a valid state for the user's code without overwhelming the user with the internal details of its search or analysis process. This involves showing the conflicting constraints clearly, akin to how theorem provers explain failed proofs.6
    
- **Guiding Concurrency Model Usage:** Concurrency errors are often subtle and difficult to debug. Seen's diagnostics must clearly illustrate the problematic interaction, race condition, or invalid state, explicitly linking the error back to the language's "intuitive mechanisms." For example, if Seen uses message channels as a primary concurrency primitive, a data race error should be framed in terms of accessing shared data without channel mediation, reinforcing the intended programming model.8 The terminology used must consistently reflect Seen's conceptual model, avoiding reliance on analogies to other languages if Seen's approach differs significantly.
    

### 1.3. Building upon `rustc`'s Strengths and Innovating for Developer Experience

Seen's diagnostic system will draw inspiration from the best practices established by the Rust compiler (`rustc`), while strategically innovating to further enhance developer experience.

- **Adopting Proven Practices:** Seen will adopt `rustc`'s successful diagnostic features, including:
    
    - A structured message format with clear levels (error, warning, note, help).3
    - Unique error codes (e.g., `Sxxxx`) linked to extended explanations accessible via a command-line flag (`seen --explain Sxxxx`) and potentially an online index.3
    - Precise source location highlighting using primary and secondary spans.3
    - Actionable `help` messages providing concrete suggestions.3
- **Innovating for Clarity:** While `rustc`'s diagnostics are highly regarded, there remains room for improvement, particularly in explaining complex type system interactions, which often underpin memory and concurrency checks.10 Research systems like Argus, which explores interactive visualizations for trait inference failures, highlight the limitations of static text for complex errors.10 Seen should prioritize explaining the _why_ behind an error, especially for its unique features, going beyond merely identifying the _what_ and _where_. Diagnostics for memory and concurrency must proactively teach the underlying principles of Seen's models within the context of the specific error encountered.
    

## 2. Crafting Diagnostic Messages: Content, Structure, and Stylistic Conventions

The effectiveness of the diagnostic philosophy hinges on the careful crafting of individual messages. This section outlines the specific guidelines for content, structure, and style.

### 2.1. Message Composition: Prioritizing Plain Language and Minimizing Jargon

- **Language:** Messages will be written in clear, concise, and simple English and Arabic, avoiding compiler-internal terminology.1 The goal is accessibility, ensuring messages are understandable even to those new to Seen or systems programming. Reducing the reading level where possible without sacrificing precision is encouraged.2
- **Tone:** The tone should be helpful and guiding. Adopting a consistent persona, such as the "first person plural ('we')" suggested by Calebmer 2, can personify the compiler as a collaborative assistant ("we found an issue..."). This requires careful consideration for consistent application across both English and Arabic. Present tense ("we see" rather than "we found") should be used to reflect the current state of the code in an IDE context.2
- **Self-Containment:** Each primary error or warning message should be sufficiently self-contained to convey the core problem, even if viewed in isolation (e.g., in an IDE tooltip).3

### 2.2. Standardized Diagnostic Levels: Error, Warning, Note, and Help

Seen will adopt the standard hierarchy of diagnostic levels, providing clarity on the severity and nature of each message.3

- **Error:** A critical issue that prevents successful compilation. Errors indicate violations of Seen's language rules, including type errors, syntax errors, and violations of memory or concurrency safety guarantees. Compilation cannot proceed until all errors are resolved.
- **Warning:** A non-critical issue that does not prevent compilation but indicates potentially problematic, unsafe, non-idiomatic, or deprecated code. Examples include unused variables, potential performance issues, or use of features planned for removal. Seen will aim for a high signal-to-noise ratio with warnings, avoiding overly pedantic or numerous warnings that could lead to "warning fatigue".1 Users may have mechanisms to configure warning levels (e.g., promote specific warnings to errors, or silence others).
- **Note:** Provides additional context or factual information related to a preceding error or warning. Notes do not represent a separate problem but help clarify the circumstances of the primary diagnostic, such as pointing to the location of a conflicting definition or a previous relevant operation.3
- **Help:** Offers actionable advice or concrete code suggestions aimed at resolving an error or addressing a warning.3 Help messages are focused on solutions.

The following table summarizes the purpose and expected user action for each level:

|   |   |   |   |
|---|---|---|---|
|**Level**|**Purpose/Meaning in Seen**|**User Action Expected/Implied**|**Example Scenario in Seen**|
|Error|Violation of language rules (syntax, type, memory, concurrency). Prevents compilation.|Must fix the code to proceed.|Type mismatch; violation detected by automated memory analysis; misuse of concurrency primitive.|
|Warning|Potentially problematic, unsafe, non-idiomatic, or deprecated code. Does not prevent compilation.|Investigate and consider fixing; may configure level.|Unused variable; use of a deprecated API; code pattern known to be inefficient in Seen.|
|Note|Provides additional context, facts, or points to related code locations relevant to a primary Error or Warning.|Understand the context; no direct fix required for Note.|Location of a previous conflicting borrow (if applicable); definition site of a variable involved in a type error.|
|Help|Provides specific advice or code suggestions to fix an Error or address a Warning.|Consider applying the suggested fix or advice.|Suggest adding a type annotation; suggest importing a module; suggest replacing incorrect syntax with correct syntax.|

### 2.3. Formatting for Optimal Readability and Visual Hierarchy

Consistent and clear formatting enhances readability and helps users quickly parse diagnostic information.

- **Style:** Message text (Error, Warning, Note, Help) will start with a lowercase letter and omit trailing punctuation, following conventions from `rustc` and Clang.3
- **Highlighting:** Code elements (identifiers, types, keywords) mentioned within messages will be enclosed in backticks (`) for clear visual distinction.2 Markdown formatting may be considered for richer text if supported well by rendering crates and IDEs.2
- **Color:** Terminal output will utilize color consistently to differentiate diagnostic levels (e.g., red for errors, yellow for warnings), code snippets, span highlights, and suggestion text.1 The specific capabilities will depend on the chosen rendering crate (Section 8.2).
- **Structure:** Diagnostics will be structured for scannability, ensuring the main error message and primary location are immediately apparent, followed by contextual notes and actionable help messages.

### 2.4. Systematic Use of Error Codes and Extended Explanations

To provide depth and facilitate external tooling, Seen will use error codes linked to detailed explanations.

- **Error Codes:** All significant errors (and potentially major warnings) will be assigned a unique, stable error code prefixed with 'S' (for Seen), e.g., `S0001`, `S0308`. These codes provide a concise identifier for the issue.3
- **Extended Explanations (`seen --explain`):** A command-line mechanism, `seen --explain <ErrorCode>`, will provide access to comprehensive, long-form explanations for each error code.3 These explanations are crucial for the pedagogical goals of the diagnostic system and must include:
    - A detailed breakdown of the error's nature and cause.
    - Illustrative code examples demonstrating how the error can occur.
    - Common pitfalls and detailed strategies for resolving the error.
    - For errors related to memory or concurrency, clear references and links to the relevant sections of Seen's official documentation explaining the underlying models or principles.
- **Bilingual Explanations:** Critically, these extended explanations must be available in both English and Arabic, managed through the same internationalization framework used for shorter diagnostics (Section 6). This ensures the learning benefits are equally accessible to all users.

## 3. Precision in Source Location: Tracking and Presentation

Accurate identification of the source code location associated with a diagnostic is paramount for usability. Errors pointing to the wrong location are misleading and frustrating. Seen's diagnostic system requires robust mechanisms for tracking and presenting source spans.

### 3.1. Comprehensive Span Tracking (Primary & Secondary) Across Compiler Phases

Source location information, represented as spans (start and end positions), must be meticulously maintained throughout the compilation pipeline.

- **Source Representation:** A central `SourceMap` data structure, analogous to `rustc_span::SourceMap` 17, will manage loaded source files and provide mappings from byte offsets to line/column numbers. `Span` objects, representing ranges within the source map, will be the fundamental unit of location information. `rustc_span` also handles complexities like macro expansion context (`SyntaxContext`, `ExpnData`) and edition differences, making it a strong model to emulate or adapt.17
- **Span Propagation:** Spans associated with source constructs must be carefully propagated through all intermediate representations (IRs) used by the Seen compiler, from the initial Abstract Syntax Tree (AST) through any High-Level IR (HIR) or Mid-Level IR (MIR) equivalents.18 Compiler passes that transform the code (e.g., desugaring, lowering, optimization) must preserve or correctly map span information. Losing span fidelity during these transformations is a common cause of inaccurate diagnostics. Given Seen's Rust-based implementation, leveraging the principles (if not the exact code) of `rustc`'s span handling across its IRs 18 is highly recommended. The use of stable identifiers like `HirId` and `DefId` in `rustc` 20 is also crucial for linking IR nodes back to source locations reliably, especially for incremental compilation.
- **Macro Expansion:** Spans originating from or affected by macro expansions require special handling to point both to the location within the macro invocation and, where relevant, to the source of the macro definition or arguments.17 The hygiene system (`SyntaxContext` in `rustc_span`) is essential for this. Inaccurate spans in macro-generated code are a frequent source of confusion, making robust macro span tracking critical.
- **Primary and Secondary Spans:** The diagnostic system will differentiate between:
    - **Primary Span:** The main location where the error occurred or was detected. This span should be as precise as possible, highlighting the specific token or construct at fault.3
    - **Secondary Spans:** Additional locations that provide necessary context for understanding the error. Examples include the definition site of a variable involved in a type error, or the location of a previous operation that conflicts with the current one (e.g., in memory or concurrency analysis).3
- **Span Correctness as a Guarantee:** For Seen, particularly with its goal of improved developer experience and intuitive models, the accuracy of span information is a foundational guarantee. Degraded span information directly undermines the clarity and actionability of diagnostics, especially for complex errors involving multiple code locations, such as those related to memory or concurrency. Compiler transformations must prioritize span preservation alongside semantic correctness.

### 3.2. Effective Visualization of Source Spans in Diagnostic Output

Tracked span information must be presented to the user effectively.

- **Code Snippets:** Diagnostics will include relevant snippets of the user's source code.3
- **Highlighting:** Primary and secondary spans within the snippet will be clearly marked, typically using underlining, carets (`^`), or color, depending on the output format and rendering capabilities.3
- **Labels:** Short, contextual labels will be attached to spans (especially secondary spans) to explain their relevance to the diagnostic.3 For example, a secondary span might be labeled "previous use occurred here" or "conflicting definition found here".
- **Readability:** The presentation must handle overlapping or adjacent spans gracefully, ensuring the output remains uncluttered and understandable.3 Advanced rendering crates like `ariadne` 24 and `miette` 25 provide sophisticated algorithms for label placement and overlap avoidance. The chosen rendering crate (Section 8.2) will significantly influence the visual quality.

## 4. Empowering Users: Generating Actionable Suggestions and Fix-its

Beyond identifying problems, Seen's diagnostic system should actively assist users in correcting them by providing intelligent and actionable suggestions.

### 4.1. Heuristics for Intelligent Correction Proposals

The compiler will employ various heuristics, leveraging semantic information gathered during analysis, to generate helpful suggestions.

- **Typo Correction ("Did you mean...?"):** When encountering unrecognized identifiers (variables, function names, types, keywords), the compiler will search the relevant scope (local, module, crate, dependencies) and global definitions for similarly spelled names. Suggestions will be based on algorithms like Levenshtein distance.27 This requires access to symbol tables and type information. For Seen's bilingual keywords, this heuristic needs enhancement: it must be aware of both English and Arabic keyword variants and the current language context. If a typo resembles a keyword from the _other_ language, or a keyword is used in the wrong language context, the suggestion should ideally point this out or suggest the correct equivalent.27
- **Missing Items:** If an identifier is unresolved, but known to exist in another module or crate, the compiler can suggest adding the necessary `import` or `use` statement, or potentially adding a dependency to the project configuration.
- **Common Idiomatic Fixes:** For recurring error patterns, especially those related to Seen's specific memory or concurrency idioms, the compiler can suggest standard, idiomatic solutions. This requires encoding knowledge about common mistakes and their canonical fixes.
- **Type Mismatch Suggestions:** In case of type mismatches, suggestions can include:
    - Applying standard type conversions (e.g., suggesting `.to_string()` if a string is needed but an integer is found).
    - Pointing to relevant specs that might provide the required conversion or functionality.
    - Suggesting adjustments to type annotations if inference led to an incompatible type.
- **Advanced Approaches (Future Consideration):** While potentially complex, exploring techniques like learning repair templates from code corpora, as seen in research tools like Rite 29 or DeepDelta 30, could offer more sophisticated suggestions in the future.

### 4.2. Communicating Suggestion Confidence and Applicability

Not all suggestions are created equal. It is crucial to communicate the compiler's confidence in a suggestion and whether it's suitable for automatic application.

- **Applicability Levels:** Seen will adopt a system similar to `rustc`'s `Applicability` enum 31 to classify suggestions:
    - **`MachineApplicable`:** High confidence; the suggestion is likely correct and can be applied automatically by tools (e.g., `seen fix`). Example: correcting a simple typo in a local variable name.
    - **`HasPlaceholders`:** The suggestion provides a template but requires the user to fill in details. Example: "add type annotation: `let x: <type>`". Cannot be applied automatically.
    - **`MaybeIncorrect`:** The suggestion is plausible but speculative; applying it might not fix the error or could introduce new issues. Requires careful user review. Example: suggesting a complex refactoring based on a heuristic.
    - **`Unspecified`:** Default level if applicability cannot be determined; treated as not machine-applicable.
- **Clear Distinction:** The diagnostic output format (both console and structured) must clearly indicate the applicability level, allowing users and tools like `seen fix` (analogous to `cargo fix`) to differentiate between safe automatic fixes and suggestions requiring manual intervention.

### 4.3. Exploring Automated Assistance via Fix-its

Suggestions marked as `MachineApplicable` form the basis for automated code correction tools.

- **`seen fix` Tool:** Seen should include a command-line tool (`seen fix`) that consumes structured diagnostic output (JSON) and automatically applies all `MachineApplicable` suggestions. This significantly improves developer productivity by automating trivial fixes.
- **Integration:** This requires the JSON output to contain precise span information for the code to be replaced and the exact replacement text for each `MachineApplicable` suggestion. GCC's `-fdiagnostics-parseable-fixits` format provides a model for representing fix-its.23
- **Scope:** Initially, `seen fix` would focus on high-confidence, simple fixes (typos, adding missing imports, simple syntax corrections). More complex automated repairs based on static analysis 32 or learned patterns 29 represent potential future enhancements but require significant research and careful implementation to avoid incorrect transformations.

## 5. Illuminating Seen's Memory and Concurrency: Specialized Diagnostics

This section details the critical diagnostic strategies for Seen's novel automated memory management and intuitive concurrency models. These diagnostics must be exceptionally clear and pedagogical to fulfill Seen's promise of an improved developer experience in these complex areas.

### 5.1. Clearly Explaining Failures in Seen's Automated Memory Analysis

When Seen's automated memory management system encounters a situation it cannot resolve or deems unsafe, the resulting diagnostics must provide deep insight, transforming the analysis from a "black box" into a understandable guardian.

- **Contextual Explanations:** Error messages must go beyond simply stating "analysis failed." They need to explain:
    - **Goal:** What safety property or invariant the automated analysis was attempting to enforce (e.g., "ensuring data is not accessed after being freed," "preventing simultaneous mutable access").
    - **Violation:** The specific rule violated by the user's code, referencing the relevant construct (e.g., "this variable is used here...").
    - **State:** The inferred state of relevant variables, memory regions, or lifetimes (if applicable) that led to the conflict (e.g., "...but it was already exclusively borrowed here [link to secondary span]"). This requires the diagnostic system to access information from the analysis passes.
    - **Principle:** How the violation relates to Seen's fundamental memory safety principles, reinforcing the language's model.
- **Visual Aids (Potential):** Leveraging the capabilities of the chosen rendering crate (Section 8.2), diagnostics could potentially use textual diagrams or structured notes to illustrate the conflicting access patterns, ownership transfers, or region relationships, similar to how `rustc` explains complex borrow check errors.3 Interactive visualization, as explored by Argus for Rust traits 10, represents a more advanced future possibility if integrated with IDE tooling.
- **Intuitive Language:** Explanations must consistently use the terminology defined by Seen's "intuitive mechanisms." If Seen uses concepts like "stewardship" or "regions," these terms must be central to the diagnostic messages, avoiding reliance on potentially confusing analogies to other languages like Rust's borrow checker unless the mechanisms are identical.
- **Learning from Related Fields:** Explaining static analysis failures is challenging.10 Insights can be drawn from:
    - **Rust Borrow Checker:** How it explains lifetime and borrowing conflicts.37
    - __Formal Verification Tools (Dafny, F_, Coq, Isabelle/HOL):_* These tools excel at explaining why a logical proof or verification condition failed, often providing detailed context about the state leading to the failure.6 Seen's diagnostics could adopt a similar approach, showing the state inferred by the automated analysis just before the error was detected.

### 5.2. Guidance on Correct Usage of Manual Memory and Concurrency Primitives

If Seen provides mechanisms for manual memory management or lower-level concurrency control (e.g., as escape hatches or for specific performance-critical code), diagnostics related to their misuse must be precise and informative.

- **Contract Violations:** Messages should clearly identify the specific manual primitive being used and explain exactly how its documented contract or safety invariants were violated by the user's code.
- **Reference Correct Usage:** Diagnostics should point towards the correct way to use the primitive, potentially referencing documentation examples.
- **Suggest Alternatives:** If the manual primitive is being used in a situation where Seen's automated mechanisms are applicable and safer, the `help` message should gently guide the user back towards the idiomatic, automated approach.

### 5.3. Designing Messages Aligned with Seen's "Intuitive Mechanisms"

The language and framing of all memory and concurrency diagnostics are critical for reinforcing Seen's intended conceptual model.

- **Consistent Terminology:** All diagnostic components (error messages, notes, help text, `--explain` content) must consistently use the specific terminology chosen for Seen's memory and concurrency features. This builds a coherent mental model for the user.
- **First Principles:** Explanations should derive from Seen's own design principles. Avoid relying on analogies to other languages (like Rust, Go, or Pony) unless the underlying mechanism is truly identical, as this can create confusion if the models differ subtly. The goal is to teach _Seen_, not Seen-as-a-variant-of-X.

### 5.4. Drawing Inspiration from Other Safety-Conscious Language Diagnostics

Examining how other languages handle diagnostics for their specific safety features can provide valuable insights.

- **Pony:** Pony employs reference capabilities (`iso`, `val`, `ref`, `box`, `trn`, `tag`) to ensure data-race freedom within its actor model.45 Its error messages often relate to violations of these capability rules.8 While potentially complex initially, they aim to explain capability mismatches. Seen can study how Pony communicates these specific constraints.
- **Chapel:** Chapel focuses on large-scale data parallelism with constructs like `forall` loops and promotion.9 Incorrect usage can lead to race conditions. Chapel's documentation and compiler likely have strategies for explaining parallelism-related errors, such as races introduced by implicit zippered iteration during promotion.46
- **Static Race Detectors:** Tools like RacerD 47 or techniques described in research 48 focus on detecting potential data races. The way these tools report races (e.g., identifying conflicting accesses and potentially the lack of locking) can inform how Seen explains concurrency violations.
- **Rust:** As previously mentioned, `rustc`'s borrow checker diagnostics, despite their complexity, are a benchmark in explaining resource lifetimes and access conflicts textually.37

By synthesizing best practices and tailoring explanations specifically to Seen's unique automated and intuitive systems, the diagnostic system can become a powerful ally for developers navigating memory and concurrency safety.

## 6. Bridging Languages: Bilingual Diagnostic Support (English/Arabic)

A defining feature of Seen is its bilingual nature, extending to keywords and, crucially, its diagnostic system. This requires a robust infrastructure for managing and presenting diagnostics in both English and Arabic.

### 6.1. Language Selection Mechanisms

Users need a clear way to specify their preferred language for diagnostic messages. Several mechanisms should be provided, with a defined order of precedence:

1. **Command-Line Flag:** A flag like `--lang ar` or `--lang en` overrides all other settings for a single compilation invocation.
2. **Environment Variable:** An environment variable, e.g., `SEEN_LANG`, allows users to set a persistent preference for their shell session or system.
3. **Project Configuration:** A setting within the Seen project manifest file (e.g., `Seen.toml`) specifies the default language for that specific project.
4. **Default:** If no other setting is provided, the diagnostics should default to English.

This layered approach provides flexibility for different user workflows. A single language setting per compilation invocation or LSP session is recommended initially for simplicity, avoiding the complexities of dynamic language switching within a single diagnostic output stream.

### 6.2. Robust Translation Management and Workflow

Managing translations for potentially numerous and complex diagnostic messages requires a well-defined system.

- **Internationalization (i18n) Framework:** Selecting an appropriate Rust i18n framework is critical. Key candidates include:
    
    - **Fluent:** Designed by Mozilla for natural-sounding translations, handling complex grammatical features like plurals and gender effectively. Uses `.ftl` files. Crates like `l10n` 50 build upon `fluent-bundle` and offer compile-time checks for message existence and arguments, which is highly advantageous for compiler development. Fluent's model emphasizes a clear contract between developers (providing identifiers and variables) and localizers (crafting the best translation using syntax features).51
    - **Gettext:** The traditional standard, widely supported by translation tools. Rust bindings exist (`gettext-rs` 52), and frameworks like `i18n-embed` 53 can utilize it. However, Gettext's handling of plurals is often considered less elegant than Fluent's, and it relies on source strings as keys by default, which can be brittle.51
    - **`rust-i18n`:** A Rust-specific solution using YAML, JSON, or TOML files.54 It performs compile-time code generation to embed translations directly into the binary, offering good performance and eliminating runtime file dependencies. It supports fallbacks and variable interpolation.
    
    Given the need for potentially complex, pedagogical explanations and the desire for robust compile-time checks, **Fluent, likely via the `l10n` crate, appears to be the most promising choice.** Its focus on natural language and separation of concerns aligns well with Seen's goals.
    
- **Message Keys:** All translatable strings (diagnostic messages, notes, help texts, extended explanations) must be identified by stable, semantic keys (e.g., `error.memory.use_after_free`). These keys are used in the compiler source code to request a localized string.
    
- **Translation Storage:** Translation files (e.g., `en/diagnostics.ftl`, `ar/diagnostics.ftl`) will map these keys to the corresponding strings in each language.
    
- **Translation Process:** A clear workflow for adding new messages and updating translations is needed. This might involve automatic extraction of keys, integration with translation platforms, or community contribution guidelines.
    
- **Fallback Mechanism:** If a translation for a specific key is missing in the requested language (e.g., Arabic), the system must gracefully fall back to a default language (English).50 Optionally, a note could indicate that the message is shown in the fallback language.
    

The following table evaluates potential i18n frameworks:

|   |   |   |   |   |   |   |   |
|---|---|---|---|---|---|---|---|
|**Framework/Crate**|**Message Format(s)**|**Plural/Gender Support**|**Interpolation**|**Tooling**|**Compile-time Aspects**|**Integration Ease**|**Suitability for Seen**|
|Fluent (`l10n`)|`.ftl`|Excellent (built-in)|Yes|Good|Compile-time checks|Moderate|High (expressive, natural language, compile-time safety)|
|Gettext|`.po`, `.mo`|Basic (plural forms)|Yes|Mature|Runtime loading|Moderate|Moderate (standard but less expressive than Fluent, default keying less robust)|
|`rust-i18n`|YAML, JSON, TOML|Basic (via variables)|Yes|Basic|Compile-time embedding|Easy|Moderate-High (simple, compile-time embedding good, but less expressive syntax than Fluent for complex messages)|

### 6.3. Addressing Challenges in Rendering Right-to-Left (RTL) Arabic Text in Terminals and IDEs

Displaying mixed English (LTR) and Arabic (RTL) text correctly presents significant technical challenges, primarily residing in the rendering capabilities of the terminal emulator or IDE, rather than the compiler itself.

- **The Core Challenge:** Many terminals and IDE components struggle with bidirectional text, leading to issues like disconnected Arabic letters (improper shaping), incorrect text directionality (LTR instead of RTL), and confusing cursor behavior when editing mixed text.55
- **Seen Compiler's Responsibility:** The Seen compiler's primary responsibility is to output **logically correct Unicode strings**. This means that for a message containing mixed LTR and RTL segments, the characters should be ordered in the sequence they are meant to be read (logical order), relying on the Unicode Bidirectional Algorithm (UBA).59 For example, the logical string `error: استخدم 'seen_let' بدلا من 'let'` should be output as such.
- **Terminal/IDE Responsibility:** The terminal emulator or IDE text rendering component is then responsible for applying the UBA to display the logical string correctly in visual order (e.g., rendering the previous example visually as `error: let بدلا من 'seen_let' استخدم`). It must also handle Arabic text shaping (connecting letters, selecting appropriate glyph forms) using libraries like HarfBuzz 62 or relying on the OS's text rendering stack.
- **Addressing Rendering Issues:**
    - **Terminal Emulators:** Support for BiDi and complex scripts varies significantly. Modern terminals like WezTerm 65 or those based on VTE (like GNOME Terminal) 59 may offer better support than older ones like xterm or terminals with known limitations like Alacritty or Kitty (historically).65 `mlterm` is noted for multilingual support.68 Windows Terminal requires specific configuration for BiDi.69 Seen should _not_ attempt complex BiDi rendering itself for console output.
    - **Explicit BiDi Controls:** While Unicode offers control characters (LRM, RLM, LRI, RLI, PDI, etc.) to explicitly manage directionality 60, embedding these in compiler output is fragile. They may not be supported by all terminals, can interfere with copy-pasting, and add significant complexity to the diagnostic generation. This approach should generally be avoided for standard console output.
    - **IDE Integration (LSP):** The LSP server sends diagnostic messages as strings within a structured format.74 The IDE (VS Code, JetBrains, etc.) is responsible for rendering these strings. Known issues exist in major IDEs regarding RTL/BiDi support.56 Seen's LSP server should provide the logically ordered, correctly translated string. Improving IDE rendering is outside Seen's direct control.
    - **Plain Text Fallback:** Ensure that even if BiDi rendering is imperfect in a given environment, the diagnostic text remains fundamentally understandable. This means avoiding formatting that relies heavily on precise visual alignment that might break in RTL contexts.
- **Recommendations for Seen:**
    1. **Output Logical Order:** Always output bilingual diagnostic strings in their correct Unicode logical order.
    2. **Rely on Standard Tools:** Depend on modern terminal emulators and IDEs to implement the UBA and text shaping correctly.
    3. **Documentation:** Provide documentation advising users on terminal/IDE configurations known to provide better BiDi support for Seen diagnostics.
    4. **Testing:** Establish a test suite to verify diagnostic output rendering across a matrix of common terminals and IDEs on different operating systems to identify and document common rendering problems.
    5. **Avoid Embedded Controls:** Do not embed explicit Unicode BiDi control characters in standard console output unless absolutely necessary and proven effective across target environments.

## 7. Seamless Ecosystem Integration: Tooling and Output Formats

For Seen to integrate smoothly into developer workflows and tooling ecosystems, its diagnostics must be available in standardized, machine-readable formats in addition to human-readable console output.

### 7.1. Standardized Console Output (GCC-like)

The default output format when invoking the Seen compiler from the command line should be familiar and easily parsable by humans and simple scripts.

- **Format:** Adopt a format similar to GCC and Clang: `filename:line:column: severity: message`.14
- **Content:** This primary line should be followed by:
    - The relevant source code snippet.
    - Visual highlighting (e.g., carets, underlines, color) of primary and secondary spans within the snippet.3
    - Associated `note:` and `help:` messages, clearly labelled.
- **Customization:** Consider basic formatting options similar to GCC's `-fmessage-length`, `-fdiagnostics-show-location`, and `-fdiagnostics-color` 23 to allow users some control over console output verbosity and appearance.

### 7.2. Structured Output Formats: JSON and SARIF

Machine-readable formats are essential for integration with IDEs, build systems, continuous integration pipelines, and static analysis dashboards.

- **JSON Output (`--error-format=json`):**
    - **Purpose:** Primary format for LSP servers and custom tooling.
    - **Structure:** Output a stream of self-contained JSON objects, one per diagnostic message (error, warning, etc.). This follows the model used by `rustc`.3
    - **Schema:** Each JSON object must contain standardized fields mapping to the core diagnostic components (see table below). Crucially, string fields like `message` and `suggestion_text` will contain the localized text based on the selected language.
- **SARIF Output (`--error-format=sarif`):**
    - **Purpose:** Interoperability with static analysis tools, security scanners, and platforms like GitHub code scanning.
    - **Structure:** Output a valid SARIF v2.1.0 document.79
    - **Mapping:** Map Seen diagnostic information to standard SARIF constructs (see table below). `ruleId` will correspond to the Seen error code, `level` maps to SARIF levels (error, warning, note), `locations` holds primary span info, and `relatedLocations` can represent secondary spans or contextual notes. MSVC's use of `relatedLocations` with `nestingLevel` provides a model for structured diagnostics within SARIF.79

The following table outlines the key elements and their mapping in the proposed JSON and SARIF formats:

|   |   |   |   |
|---|---|---|---|
|**Element**|**JSON Field Path (Example)**|**SARIF Field Path (Example)**|**Notes on Seen's Content**|
|Severity|`severity` ("error", "warning",...)|`level` ("error", "warning", "note")|Maps Seen's internal levels (Error, Warning, Note, Help) to standard JSON/SARIF terms. Help might map to SARIF `note`.|
|Message Text|`message`|`message.text`|Localized string (English or Arabic) based on selected language.|
|Error Code|`code` ("S0123")|`ruleId` ("S0123")|Seen's unique diagnostic code.|
|Primary Span|`span.file_name`, `span.line_start`, `span.column_start`, `span.line_end`, `span.column_end`|`locations.physicalLocation.artifactLocation.uri`, `locations.physicalLocation.region`|Precise location of the main diagnostic point.|
|Secondary Span(s)|`children.span`, `children.message`|`relatedLocations.physicalLocation...`, `relatedLocations.message.text`|Used for contextual locations, potentially with attached localized labels/messages.|
|Suggestion Text|`suggestion.text`|`fixes.artifactChanges.replacements.insertedContent.text`|Localized suggestion string. SARIF `fixes` is specifically for replacements.|
|Suggestion Applicability|`suggestion.applicability`|(Possibly via `fixes.description.text` or properties)|`MachineApplicable`, `MaybeIncorrect`, etc. Standard SARIF has less direct support for applicability levels.|

This structured output ensures that tools consuming Seen's diagnostics can reliably extract the necessary information, regardless of the human language used in the textual components.

### 7.3. LSP Integration: `textDocument/publishDiagnostics` for Rich IDE Feedback

The Language Server Protocol (LSP) is the standard for providing rich IDE features, including diagnostics. Seen's LSP server, implemented in Rust, will leverage the structured JSON output.

- **Mechanism:** The LSP server will parse the JSON diagnostic output from the Seen compiler (or receive diagnostic information directly if integrated more tightly) and send it to the connected IDE client using the `textDocument/publishDiagnostics` notification.74
- **`Diagnostic` Structure:** The information will be formatted according to the LSP `Diagnostic` structure 74:
    - `range`: Derived from the primary span.
    - `severity`: Mapped from Seen's severity to LSP `DiagnosticSeverity` (Error, Warning, Information, Hint).
    - `code`: Seen's error code (`Sxxxx`).
    - `source`: A fixed string, e.g., `"seen-compiler"`.
    - `message`: The localized diagnostic message string.
    - `relatedInformation`: Used to convey secondary spans and associated notes/labels, linking them back to source locations.
    - `tags`: Can be used for `DiagnosticTag.Unnecessary` or `DiagnosticTag.Deprecated` if Seen generates such warnings.
    - `data`: An opaque field that can carry extra information from the compiler to the LSP server, potentially facilitating `textDocument/codeAction` requests (quick fixes) based on `MachineApplicable` suggestions.
- **BiDi in LSP:** As noted in Section 6.3, the LSP server transmits the localized `message` string in its logical Unicode order. The responsibility for correctly rendering potentially mixed LTR/RTL text lies with the LSP client (the IDE). Seen's LSP ensures the data sent is correct; rendering fidelity depends on the client implementation.75 Clients might offer fallback mechanisms if they cannot render perfectly.85

## 8. Implementation Blueprint: A Rust-Powered Diagnostic Infrastructure

This section details the proposed technical architecture and implementation strategy for Seen's diagnostic system, leveraging the fact that the Seen compiler itself is implemented in Rust.

### 8.1. Architectural Design within the Seen Compiler (Rust Codebase)

A robust and maintainable diagnostic infrastructure is essential. Drawing inspiration from `rustc`'s design provides a solid foundation.

- **Central Diagnostic Context (`DiagCtxt`):** A central struct, analogous to `rustc_errors::DiagCtxt` 3, should be established. This context will likely be managed by or accessible through Seen's equivalent of `rustc`'s `Session` or `TyCtxt`.87 Its responsibilities include:
    - Serving as the primary interface for reporting diagnostics from any compiler phase.
    - Managing the selected language and loading/accessing translations.
    - Collecting diagnostic information (level, message key, arguments, spans).
    - Coordinating the formatting and emission process.
- **Diagnostic Emitters/Handlers:** Implement distinct emitter modules responsible for formatting diagnostics into specific output formats (human-readable console, JSON, SARIF). The `DiagCtxt` will select and invoke the appropriate emitter(s) based on command-line flags (e.g., `--error-format`).3 This separation of concerns follows patterns seen in Clang's `DiagnosticConsumer`.4
- **Diagnostic Builder:** Employ the builder pattern, similar to `rustc`'s `DiagnosticBuilder` 31, for constructing diagnostics. This allows compiler passes to create a diagnostic object, incrementally add primary/secondary spans, labels, notes, and suggestions, and finally "emit" the diagnostic via the `DiagCtxt`. This pattern prevents partially formed diagnostics from being accidentally emitted and enforces a structured approach. Failing to emit or cancel a builder could trigger an internal compiler error, ensuring diagnostics are always handled.31
- **Cross-Phase Reporting:** The `DiagCtxt` must be accessible throughout all compiler phases (parsing, semantic analysis, type checking, MIR analysis, etc.) to allow diagnostics to be generated wherever errors are detected.3 Spans generated in earlier phases (like parsing) must be preserved and usable when reporting errors detected in later phases (like type checking or MIR analysis).20
- **Error Buffering/Deduplication:** Consider implementing optional buffering within the `DiagCtxt` or its handlers. This can allow for deduplicating identical errors reported multiple times or potentially prioritizing more critical errors if an error limit is reached.3 `rustc` sometimes buffers errors to emit them in a specific order.3
- **Leveraging Rust Compiler Concepts:** While directly reusing `rustc_errors` or `rustc_session` might be infeasible due to their deep integration with `rustc` internals, their architectural patterns (central context, builders, emitters, integration with compiler session state) provide a proven model for Seen's Rust-based implementation.31

### 8.2. Strategic Evaluation and Integration of Rust Crates for Diagnostic Rendering

Choosing the right crate(s) for rendering the visually rich console output is crucial for achieving Seen's developer experience goals. LSP diagnostics rely on structured data, but console output requires explicit rendering.

- **Candidate Crates:**
    - **`codespan-reporting`:** Provides solid, `rustc`-like basic diagnostic formatting. Good starting point, potentially less flexible for highly custom or complex layouts.90
    - **`ariadne`:** Offers highly configurable and sophisticated rendering. Features include multi-line/multi-file support, complex label arrangements, color themes, and overlap avoidance heuristics. Mentions handling variable-width characters and choice of character sets, indicating attention to layout details.24 Its flexibility might offer pathways for BiDi customization, though none are explicit.92
    - **`miette`:** Focuses on "fancy," user-friendly output with features like code snippets, clickable error code URLs, and theming. Provides a `ReportHandler` trait for deep customization.25 Like `ariadne`, no explicit BiDi support, but the custom handler is a potential hook.93
    - **`annotate-snippets`:** Used internally by `rustc`.94 Focuses on ASCII-graphical rendering. Might be less feature-rich or customizable than `ariadne` or `miette` for Seen's advanced goals.
- **Evaluation Criteria for Seen:**
    - **Visual Quality & Clarity:** How well does it render complex diagnostics with multiple spans and notes?
    - **Customizability:** Can themes and styles be adjusted? Can the layout be influenced?
    - **BiDi/RTL Potential:** Does the API offer hooks for custom character/line drawing or layout logic that could be adapted for Arabic/BiDi rendering? (This is a key differentiator for Seen).
    - **API Ergonomics:** How easy is it to integrate with Seen's `DiagCtxt` and builder pattern?
    - **Performance & Dependencies:** Are there significant overheads or dependency conflicts?
    - **Maintenance:** Is the crate actively maintained?
- **Recommendation:** Both `ariadne` and `miette` appear to be strong contenders due to their focus on rich, customizable output and potential extension points (custom handlers in `miette`, general flexibility in `ariadne`). `ariadne`'s explicit handling of variable-width characters and layout heuristics might give it an edge for complex code displays. However, neither offers out-of-the-box BiDi support for mixed LTR/RTL text. The final choice may depend on a deeper investigation of their APIs regarding custom layout or drawing hooks relevant to BiDi challenges. It might be necessary to contribute upstream or build a BiDi-aware rendering layer.

The following table provides a comparative analysis:

|   |   |   |   |   |
|---|---|---|---|---|
|**Crate Name**|**Key Rendering Features**|**Customizability**|**BiDi/RTL Potential**|**Recommendation for Seen**|
|`codespan-reporting`|Basic span highlighting, labels, notes, color 90|Basic config (colors)|Low (less flexible API)|Suitable for basic needs, but likely insufficient for Seen's rich output and BiDi goals.|
|`ariadne`|Advanced spans (multi-line/file), complex labels, overlap heuristics, color/themes 24|High (config, char sets, label placement)|Moderate (flexible layout, but no explicit BiDi hooks identified) 92|Strong contender due to layout power and flexibility. Potential for BiDi needs investigation/extension.|
|`miette`|Fancy snippets, labels, code URLs, color/themes 25|High (themes, `ReportHandler` trait)|Moderate (custom `ReportHandler` provides hook, but requires manual BiDi impl) 93|Strong contender due to focus on DX and custom handler trait. Potential for BiDi requires significant custom implementation.|
|`annotate-snippets`|ASCII-graphical snippets, used by `rustc` 95|Moderate (options like color, margins)|Low (focused on ASCII representation)|Less likely to meet Seen's goals for rich, potentially graphical, and BiDi-aware output compared to `ariadne` or `miette`.|

### 8.3. Systematic Management of Diagnostic Messages and Their Translations

Efficiently managing a potentially large corpus of diagnostic messages and their translations is vital.

- **Storage:** Store message templates (keyed strings with placeholders for arguments) and their English/Arabic translations in external files. Using the `.ftl` format with the Fluent system is recommended due to its expressiveness for natural language.50
- **Loading:** Embed translations into the compiler binary at compile time. This improves robustness and performance by avoiding runtime file I/O dependencies. Crates like `l10n` (using macros with Fluent) 50 or `rust-i18n` 54 facilitate this. Compile-time embedding is strongly preferred over runtime loading for a compiler.
- **Access API:** The `DiagCtxt` should provide a simple internal API for retrieving localized strings. Example: `diag_ctxt.get_message("error.key", lang_id, args)` which would internally use the chosen i18n crate (e.g., `l10n`).
- **Translation Tooling:** Leverage tools compatible with the chosen i18n framework (e.g., Fluent-aware tools or `cargo i18n`) to help extract message keys, manage translation files (`.ftl`, `.yaml`, etc.), and potentially integrate with translation platforms or workflows.54

## 9. Conclusions and Recommendations

The design of Seen's diagnostic system presents a unique opportunity to significantly enhance the developer experience, particularly by providing pedagogical support for its novel memory and concurrency models and by offering full bilingual (English/Arabic) capabilities.

**Key Recommendations:**

1. **Prioritize Pedagogy:** Embed teaching directly into diagnostics for memory and concurrency. Explain the _why_ based on Seen's principles, not just the _what_. Make the automated systems transparent through clear explanations of their goals and failures.
2. **Adopt Structured Diagnostics:** Implement the standard Error/Warning/Note/Help hierarchy, unique error codes (`Sxxxx`), and an `--explain` mechanism, mirroring `rustc`'s successful model. Ensure explanations are also bilingual.
3. **Ensure Span Fidelity:** Rigorously track and propagate source spans through all compiler IRs. Accurate location information is non-negotiable.
4. **Implement Actionable Suggestions:** Develop heuristics for "Did you mean...?" (including bilingual keyword awareness) and common fixes. Use applicability levels (`MachineApplicable`, etc.) and build a `seen fix` tool for automated corrections.
5. **Choose Fluent for i18n:** Leverage the Fluent localization system (likely via the `l10n` crate) for its expressiveness in handling natural language, crucial for both English and Arabic pedagogical messages. Embed translations at compile time.
6. **Address BiDi Pragmatically:** Output logically correct Unicode strings. Rely on modern terminals/IDEs for rendering. Document recommended environments and test rendering extensively. Avoid embedding complex BiDi controls initially.
7. **Provide Standardized Output:** Offer GCC-like console output, JSON for tooling/LSP, and SARIF for static analysis integration. Ensure structured formats contain localized messages.
8. **Build a Robust Rust Infrastructure:** Model the internal diagnostic architecture on `rustc`'s patterns (`DiagCtxt`, Builders, Emitters).
9. **Select Rendering Crate Carefully:** Evaluate `ariadne` and `miette` based on layout capabilities, customizability, and potential extensibility for future BiDi improvements. `ariadne`'s layout features seem slightly more aligned with complex diagnostic needs, but `miette`'s custom handler offers a clear extension point.

By implementing this comprehensive diagnostic system design, Seen can establish a reputation for exceptional developer support, effectively teach its unique features, and cater to a diverse user base through its bilingual capabilities. The investment in high-quality diagnostics is an investment in the language's success and adoption.