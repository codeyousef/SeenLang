# Multi-language support

Seen 0.13.0 ships complete language packs for English (`en`), Arabic (`ar`),
Spanish (`es`), Russian (`ru`), Chinese (`zh`), and Japanese (`ja`). A program
uses the same AST and compiler after the lexer maps a supported localized
spelling to its canonical token.

Three layers must not be confused:

1. `languages/<lang>/*.toml` records spellings and standard-library aliases.
2. `KeywordManager.resolveTokenName` decides which keyword-token names the
   current compiler activates.
3. The parser, semantic checks, code generator, and passing tests determine
   whether a construct is actually supported.

A spelling in a TOML file is not by itself evidence that its proposed language
feature works. In particular, do not infer support for actors, `send`,
`receive`, `select`, structured async `scope`, `cancel`, `launch`, `flow`, or a
localized `external` FFI keyword from the pack inventory. Public FFI currently
uses the literal contextual spelling `extern`.

## Selecting a language

Choose the language on the command line:

```bash
seen compile hello.seen hello --language ar
seen check hello.seen --language es
seen run hello.seen -l zh
```

Or set the project default:

```toml
manifest-version = 1

[project]
name = "my_project"
language = "ar"
```

An explicit command-line option overrides the manifest. The supported values
are `en`, `ar`, `es`, `ru`, `zh`, and `ja`.

## Hello World

```seen
// English
fun main() {
    println("Hello, World!")
}
```

```seen
// Arabic
دالة main() {
    println("مرحبا بالعالم!")
}
```

```seen
// Spanish
función main() {
    println("¡Hola, Mundo!")
}
```

```seen
// Russian
функция main() {
    println("Привет, мир!")
}
```

```seen
// Chinese
函数 main() {
    println("你好，世界！")
}
```

```seen
// Japanese
関数 main() {
    println("こんにちは、世界！")
}
```

Library API names such as `println` are not assumed to be keywords. The
standard-library alias tables define the localized aliases that are actually
available for each pack.

## Activated keyword surface

The 0.13.0 lexer recognizes localized spellings whose pack value resolves to
one of the following canonical tokens:

- control flow: `fun`, `if`, `else`, `while`, `for`, `in`, `match`, `when`,
  `loop`, `break`, `continue`, `return`, `try`, `catch`, `finally`, `throw`,
  `defer`, `errdefer`, and `unsafe`;
- declarations and types: `let`, `var`, `mut`, `const`, `static`, `data`,
  `struct`, `enum`, `impl`, `type`, `class`, `sealed`, `interface`, `override`,
  `extension`, `extends`, `trait`, `distinct`, and `union`;
- modules and visibility: `module`, `import`, `use`, and `pub`;
- literals: `true`, `false`, and `null`;
- ownership and advanced syntax: `move`, `borrow`, `ref`, `own`, `region`,
  `transmute`, `gpu`, `async`, `await`, `spawn`, `parallel_for`, `comptime`,
  `inline`, and `effect`;
- word operators and tests: `is`, `as`, `and`, `or`, and `not`.

This is a lexer inventory, not a guarantee that every combination has complete
semantics. Check the relevant language guide and passing tests, then run
`seen check` with the checkout compiler. Some tokens are intentionally
contextual identifiers: for example, the current operator declaration syntax
uses `fun operator+(...)`, and `assert(...)` is parsed as a call. Activating the
aspirational `KeywordOperator` or `KeywordAssert` table entries would break
current source. Receiver names such as `this` and `super` also remain literal
English contextual spellings because the current pack tables do not localize
them.

To find a localized spelling, inspect the matching `languages/<lang>/` table or
use `seen translate`; the tables are maintained as data so the documentation
does not silently drift from them.

## Translating source

```bash
seen translate input.seen --to ar
seen translate input.seen --from ar --to en
seen translate input.seen --from en --to ja -o output.seen
```

`--to` is required and `--from` defaults to `en`. Without `-o`, translated
source is written to standard output. Translation changes lexer-activated
keyword spellings and recognized standard-library aliases. It preserves
strings, comments, unrelated identifiers, and layout. Empty input is valid. A
missing/incomplete pack, unreadable input, or failed output replacement is an
error; `-o` uses a same-directory atomic replacement.

The translator and lexer share language-pack discovery, so translation works
outside the checkout when the installation contains its data files. If
`SEEN_DATA_PATH` is set, it is authoritative and must name either the data root
containing `languages/` or the `languages/` directory itself. Otherwise the
resolver checks executable-relative install data, bounded checkout ancestors,
the current checkout directory, and supported system share locations. A pack
is accepted only if all 17 required files are readable and non-empty.

## Return label and operators

The canonical return label is `r:` in every language. Arabic also accepts
`ن:`:

```seen
fun add(a: Int, b: Int) r: Int { return a + b }
دالة add(a: Int, b: Int) ن: Int { رجع a + b }
```

Symbol operators are shared across languages:

```text
+  -  *  /  %  =  ==  !=  <  <=  >  >=
+=  -=  *=  /=  %=  &  |  ^  ~  <<  >>
->  =>  ?  .  ..  ...  ..<  ::  ?.  ?:  !!  _  @
```

## Pack structure and maintenance

Each shipped language directory contains ten `keywords-*` files plus
`operators`, `builtins`, `math`, `collections`, `io`, `env`, and `str` tables:

```text
languages/
├── en/
├── ar/
├── es/
├── ru/
├── zh/
└── ja/
```

The distribution has 17 files per language, 102 files across 6 languages.
Keyword entries map a spelling to a compiler token name:

```toml
[keywords]
"fun" = "KeywordFun"
"if" = "KeywordIf"
```

Adding a directory alone does not extend the supported public language list.
A new pack also needs validation against the token allowlist, translator,
formatter/LSP behavior, CLI help, installers, and multilingual end-to-end
tests. Keep proposed but inactive spellings documented as pack inventory, not
as working Seen syntax.
