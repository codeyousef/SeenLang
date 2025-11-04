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
        .expect("program execution failed")
}

fn escaped_path(path: &std::path::Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('\"', "\\\"")
}

#[test]
fn parallel_for_writes_each_item() {
    let mut path = std::env::temp_dir();
    path.push("seen_parallel_for_test.txt");
    let _ = fs::remove_file(&path);
    fs::write(&path, "").expect("initial write failed");

    let path_literal = escaped_path(&path);

    let program = format!(
        r#"
        scope {{
            let path = "{path}"
            let values = ["1", "2", "3"]
            parallel_for value in values {{
                __WriteFile(path, __ReadFile(path) + value)
            }}
            __ReadFile(path)
        }}
    "#,
        path = path_literal
    );

    let value = run_program(&program);
    assert_eq!(
        value.to_string(),
        "123",
        "expected parallel_for to visit each element exactly once"
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn jobs_scope_executes_body() {
    let value = run_program(
        r#"
        jobs.scope {
            21 + 21
        }
    "#,
    );
    assert_eq!(
        value.to_string(),
        "42",
        "expected jobs.scope to return last expression"
    );
}
