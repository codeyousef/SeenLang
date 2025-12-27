//! Tests for scoped defer execution in the interpreter runtime

use seen_interpreter::{Interpreter, Value};
use seen_lexer::{keyword_manager::KeywordManager, Lexer};
use seen_parser::Parser;
use std::fs;
use std::sync::Arc;

fn run_program(code: &str) -> Value {
    let mut interpreter = Interpreter::new();
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap_or(());
    let keyword_manager = Arc::new(keyword_manager);
    let lexer = Lexer::new(code.to_string(), keyword_manager);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program().expect("failed to parse program");
    interpreter
        .interpret(&program)
        .expect("interpreter execution failed")
}

fn escaped_path(path: &std::path::Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('\"', "\\\"")
}

#[test]
fn defer_executes_in_reverse_order() {
    let mut path = std::env::temp_dir();
    path.push("seen_defer_order_test.txt");
    let _ = fs::remove_file(&path);

    let path_literal = escaped_path(&path);

    let program = format!(
        r#"
        fun makeLogs(path: String) {{
            __WriteFile(path, "")
            defer {{
                let existing = __ReadFile(path)
                __WriteFile(path, existing + "first\n")
            }}
            defer {{
                let existing = __ReadFile(path)
                __WriteFile(path, existing + "second\n")
            }}
            let existing = __ReadFile(path)
            __WriteFile(path, existing + "body\n")
        }}

        let path = "{path}"
        makeLogs(path)
        __ReadFile(path)
    "#,
        path = path_literal
    );

    let value = run_program(&program);
    assert_eq!(
        value.to_string(),
        "body\nsecond\nfirst\n",
        "expected deferred cleanup in LIFO order"
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn defer_runs_on_return_path() {
    let mut path = std::env::temp_dir();
    path.push("seen_defer_return_test.txt");
    let _ = fs::remove_file(&path);

    let path_literal = escaped_path(&path);

    let program = format!(
        r#"
        fun writeLater(path: String) {{
            __WriteFile(path, "")
            defer {{ __WriteFile(path, "cleanup") }}
            return 99
        }}

        let path = "{path}"
        writeLater(path)
        __ReadFile(path)
    "#,
        path = path_literal
    );

    let value = run_program(&program);
    assert_eq!(value.to_string(), "cleanup");

    let _ = fs::remove_file(&path);
}
