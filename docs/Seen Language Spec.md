# Seen Language Specification v0.9.1 (grounded)

> Corrections aligned with the Syntax Design: `let/var`, `fun`, word logical `and/or/not`, string interpolation with `{}` and `{{`/`}}`, **`if`/`match` as expressions**, `match` guards, borrowing spelled `ref` / `mut ref`, visibility by capitalization, null‑safety operators, `is` type tests with smart casts.

---

## 1. Lexical Structure
- **Encoding**: UTF‑8; identifiers, strings, comments are valid UTF‑8.
- **Identifiers**: `[_A-Za-z\p{Arabic}][A-Za-z0-9_\p{Arabic}]*`.
- **Comments**: `// …` and `/* … */` (no nesting).
- **Whitespace**: insignificant except as separator.
- **Visibility by capitalization** (symbol phase): Public items start **Capitalized**; private **lowercase**.

### 1.1 Strings & Interpolation
- Normal strings: `"…"` with escapes `\n \t \r \\ \" \' \u{XXXX}`.
- Triple‑quoted: `""" … """` (no escapes except closing fence).
- **Interpolation**: `{ Expression }` inside strings.
  - Literal braces: `{{` and `}}`.
  - Interpolated expression follows full `Expression` grammar.

---

## 2. Canonical Keywords & Bilingual Mapping (subset)
> Grammar uses canonical tokens; the lexer maps **English/Arabic** surface forms from `seen.toml`.

| Canonical | English | Arabic |
|---|---|---|
| `KW_LET` | `let` | `دع` |
| `KW_VAR` | `var` | `متغير` |
| `KW_FUN` | `fun` | `دالة` |
| `KW_RETURN` | `return` | `ارجع` |
| `KW_IF` | `if` | `إذا` |
| `KW_ELSE` | `else` | `إلا` |
| `KW_MATCH` | `match` | `طابق` |
| `KW_IS` | `is` | `هو` |
| `KW_AND` | `and` | `و` |
| `KW_OR` | `or` | `أو` |
| `KW_NOT` | `not` | `ليس` |
| `KW_FOR` | `for` | `لكل` |
| `KW_IN` | `in` | `في` |
| `KW_WHILE` | `while` | `طالما` |
| `KW_BREAK` | `break` | `اخرج` |
| `KW_CONTINUE` | `continue` | `استمر` |
| `KW_STRUCT` | `data` | `هيكل` |
| `KW_ENUM` | `enum` | `تعداد` |
| `KW_EXTENSION` | `extension` | `امتداد` |
| `KW_ASYNC` | `async` | `غير_متزامن` |
| `KW_AWAIT` | `await` | `انتظر` |
| `KW_UNSAFE` | `unsafe` | `غير_آمن` |
| `KW_REGION` | `region` | `منطقة` |
| `KW_TRUE` | `true` | `صحيح` |
| `KW_FALSE` | `false` | `خطأ` |
| `KW_NULL` | `null` | `لا_شيء` |

Reserved (future): `trait`, `actor`, `yield`, `type`, `where`.

---

## 3. Operators, Punctuation & Precedence
- **Logical (word) operators**: `and`, `or`, `not`.
- **Comparison/math**: `== != < <= > >=`, `+ - * / %`.
- **Null‑safety**: nullable `T?`, safe‑call `?.`, Elvis `?:`.
- **Other**: member `.`, indexing `[]`, call `()`, generics `< >`.

**Precedence (low → high)**
1. Assignment: `= += -= *= /=` (right‑assoc)
2. `or`
3. `and`
4. Equality: `== !=`
5. Relational: `< > <= >= is`
6. Additive: `+ -`
7. Multiplicative: `* / %`
8. Unary: `not -` (right‑assoc)
9. Postfix: call, index, member, safe‑call

---

## 4. Grammar (EBNF, canonical keywords)

### 4.1 Compilation Unit
```
CompilationUnit  = { ImportDecl } { TopLevelDecl } EOF ;
ImportDecl       = 'import' QualifiedIdent [ 'as' Identifier ] ';' ;
QualifiedIdent   = Identifier { '.' Identifier } ;
TopLevelDecl     = FunDecl | StructDecl | EnumDecl | ExtensionDecl | ConstDecl ;
```

### 4.2 Declarations
```
ConstDecl    = (KW_LET | KW_VAR) Identifier [ ':' Type ] '=' Expression ';' ;

FunDecl      = [ KW_ASYNC ] KW_FUN Name [ GenericParams ]
               '(' [ ParamList ] ')' [ ':' Type ] ( Block | '=>' Expression ) ;
ParamList    = Param { ',' Param } ;
Param        = Identifier ':' Type ;
Name         = Identifier ;

StructDecl   = KW_STRUCT Identifier [ GenericParams ] '{' { FieldDecl } '}' ;
FieldDecl    = Identifier ':' Type ';' ;

EnumDecl     = KW_ENUM Identifier [ GenericParams ] '{' EnumVariant { ',' EnumVariant } [ ',' ] '}' ;
EnumVariant  = Identifier [ '(' Type { ',' Type } ')' ] ;

ExtensionDecl = 'extension' TypeName '{' { FunDecl } '}' ;
GenericParams = '<' Type { ',' Type } '>' ;
```

### 4.3 Statements
```
Block        = '{' { Statement } '}' ;
Statement    = ConstDecl | IfStmt | WhileStmt | ForStmt | MatchStmt |
               ReturnStmt | BreakStmt | ContinueStmt | ExprStmt ;
ExprStmt     = Expression ';' ;
ReturnStmt   = KW_RETURN [ Expression ] ';' ;
BreakStmt    = KW_BREAK ';' ;
ContinueStmt = KW_CONTINUE ';' ;

IfStmt       = KW_IF '(' Expression ')' Block [ KW_ELSE Block ] ;
WhileStmt    = KW_WHILE '(' Expression ')' Block ;
ForStmt      = KW_FOR '(' Identifier KW_IN Expression ')' Block ;

MatchStmt    = MatchExpr ';' ;
```

### 4.4 Types
```
Type         = TypeTerm [ '?' ] ;
TypeTerm     = TypeName [ GenericArgs ] | '(' Type { ',' Type } ')' ;
TypeName     = QualifiedIdent ;
GenericArgs  = '<' Type { ',' Type } '>' ;
```

### 4.5 Expressions (incl. if/match as expressions)
```
Expression     = Assignment ;
Assignment     = LogicOr [ AssignOp LogicOr ] ;
AssignOp       = '=' | '+=' | '-=' | '*=' | '/=' ;

LogicOr        = LogicAnd { KW_OR LogicAnd } ;
LogicAnd       = Equality { KW_AND Equality } ;
Equality       = Relational { ( '==' | '!=' ) Relational } ;
Relational     = Additive { ( '<' | '>' | '<=' | '>=' | KW_IS ) Additive } ;
Additive       = Multiplicative { ( '+' | '-' ) Multiplicative } ;
Multiplicative = Unary { ( '*' | '/' | '%' ) Unary } ;

Unary          = ( KW_NOT | '-' ) Unary | Postfix ;
Postfix        = Primary { PostfixOp } ;
PostfixOp      = Call | Index | Member | SafeCall ;
Call           = '(' [ ArgList ] ')' ;
Index          = '[' Expression ']' ;
Member         = '.' Identifier ;
SafeCall       = '?.' Identifier [ Call ] ;
ArgList        = Expression { ',' Expression } ;

Primary        = Literal | Identifier | '(' Expression ')' | IfExpr | MatchExpr ;

IfExpr         = KW_IF '(' Expression ')' Block KW_ELSE Block ;

MatchExpr      = KW_MATCH Expression '{' MatchArm+ '}' ;
MatchArm       = Pattern [ KW_IF Expression ] '->' Expression ';' ;
Pattern        = '_' | Literal | Identifier | TypeName '(' [ PatternList ] ')' ;
PatternList    = Pattern { ',' Pattern } ;
```

---

## 5. Literals
- **Integer**: decimal, hex `0x`, binary `0b`, `_` separators.
- **Float**: `123.45`, `1e-9`.
- **String**: per §1.1, supports `{expr}` interpolation and `{{`/`}}` escapes.
- **Char**: `'x'`.
- **Bool**: `true`, `false`.
- **Null**: `null`.

---

## 6. Null Safety & Type Tests (syntax)
- Nullable types `T?`.
- Safe‑call `expr?.member` and Elvis `a ?: b`.
- Type test `expr is Type`; **smart‑cast** applies to positive `if` branches and matching arms.

---

## 7. Concurrency (syntax)
- `async fun … { … }` and `await expr`.
- Structured scoping uses ordinary blocks; cancellation semantics are runtime‑level.

---

## 8. Memory, Regions & Borrowing (surface syntax)
- **Region scopes**: `region name { … }` for bulk lifetime control.
- **Borrowing**: `ref expr` (immutable), `mut ref expr` (mutable) — surface markers for the compiler’s static analysis.
- **Unsafe**: raw pointers/deref only in `unsafe { … }`.

---

## 9. Modules & Imports
- One `.seen` file per compilation unit.
- `import pkg.path [as alias];`
- Active keyword set (English/Arabic) is read from `seen.toml` at lexing time.

---

## 10. Implementor Notes
- Grammar targets LALR(1) or Pratt; operator table in §3 is authoritative.
- Visibility by capitalization is enforced during name resolution.
- Blocks **yield the last expression** if not semicolon‑terminated (enables expression form of `if`/`match`).

