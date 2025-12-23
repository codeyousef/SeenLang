# Seen Language — Lexical Specification

## 1. Encoding & Source Text
- **Encoding:** UTF-8 (NFC normalized). The lexer rejects malformed UTF-8, non-character code points, and lone surrogates.
- **Normalization:** All identifiers, string literals, and doc comments are normalized to **Unicode NFC** prior to tokenisation. Confusable detection prevents mixed-script spoofs.
- **Line Terminators:** `\n` (Unix), `\r\n` (Windows), and Unicode line separators are accepted; internally the lexer records logical line numbers only.

## 2. Tokens
Seen uses a conventional token stream with explicit word-operators.

| Kind | Description |
| --- | --- |
| identifiers | `[_A-Za-z\p{XID_Start}][A-Za-z0-9_\p{XID_Continue}]*`, normalized to NFC |
| integer literals | Decimal, hex (`0x`), binary (`0b`), with optional `_` separators |
| float literals | Decimal with `.` or exponent; hex floats follow C99 syntax |
| string literals | `"..."` with escapes `\n \t \r \\ \" \' \u{XXXX}` |
| triple strings | `""" ... """`, no escapes except doubled fence |
| byte strings | `b"..."`/`b"""..."""`, emit `[u8]` |
| char literals | `'c'` or Unicode scalar escapes |
| punctuation | `()[]{},.;:?->` `=>` |
| operators | `+ - * / % == != < <= > >= && || !` and word forms `and`, `or`, `not`, `is` |
| keywords | `let var fun return if else match` … see **§3** |
| whitespace | spaces, tabs, Unicode category Zs — ignored except to separate tokens |
| comments | `// …` single-line, `/* … */` block (non-nesting) |

## 3. Keywords & Word Operators
Canonical keywords are fixed; surface translations are configured in `Seen.toml` and mapped to canonical tokens during lexing.

| Canonical | English | Arabic |
| --- | --- | --- |
| `KW_LET` | `let` | `دع` |
| `KW_VAR` | `var` | `متغير` |
| `KW_FUN` | `fun` | `دالة` |
| `KW_IF` | `if` | `إذا` |
| `KW_ELSE` | `else` | `إلا` |
| `KW_MATCH` | `match` | `طابق` |
| `KW_FOR` | `for` | `لكل` |
| `KW_IN` | `in` | `في` |
| `KW_WHILE` | `while` | `طالما` |
| `KW_RETURN` | `return` | `ارجع` |
| `KW_BREAK` | `break` | `اخرج` |
| `KW_CONTINUE` | `continue` | `استمر` |
| `KW_STRUCT` | `data` | `هيكل` |
| `KW_ENUM` | `enum` | `تعداد` |
| `KW_EXTENSION` | `extension` | `امتداد` |
| `KW_SPEC` | `spec` | `سمة` |
| `KW_ASYNC` | `async` | `غير_متزامن` |
| `KW_AWAIT` | `await` | `انتظر` |
| `KW_REGION` | `region` | `منطقة` |
| `KW_SPAWN` | `spawn` | `اطلق` |
| `KW_SCOPE` | `scope` | `نطاق` |
| `KW_CANCEL` | `cancel` | `الغ` |
| `KW_TRUE` | `true` | `صحيح` |
| `KW_FALSE` | `false` | `خطأ` |
| `KW_NULL` | `null` | `لا_شيء` |

Word operators (`and`, `or`, `not`, `is`) participate in precedence and are emitted as distinct token kinds so the formatter can enforce spacing rules.

## 4. Visibility Policy
Project-level visibility is controlled via `Seen.toml`:
```toml
[lang]
visibility = "caps"      # capitalised identifiers exported
export_alias = "ascii"   # ASCII export symbol alias
```
- **Caps mode:** any top-level identifier beginning with an uppercase code point exports automatically; lowercase stays module-local.

The lexer enforces policy by tagging identifier tokens with their surface casing so subsequent phases can emit diagnostics early.

## 5. Literal Semantics
- **Numeric suffixes:** `i32`, `u64`, `f32`, `f64`, `isize`, `usize`; missing suffix triggers inference guided by context.
- **Underscore separators:** permitted between digits but not at start/end.
- **Escape validation:** `\u{XXXX}` accepts up to 6 hex digits; resulting scalar must be valid Unicode.
- **Interpolation:** `{ expression }` inside strings lexes as `{` token, expression tokens, then `}`; `{{`/`}}` yield literal braces.

## 6. Trivia & Formatting Locks
- The lexer attaches leading/trailing trivia (whitespace/comments) to tokens for deterministic formatting.
- Word/operator precedence tables are frozen; formatter cross-checks token sequences to ensure round-trip stability (see `grammar.md`, §2).
