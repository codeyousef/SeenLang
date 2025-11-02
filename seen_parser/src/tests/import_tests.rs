use crate::{Parser, ParseResult};
use seen_lexer::{Lexer, KeywordManager};

#[test]
fn parses_simple_import() {
    let mut km = KeywordManager::new();
    km.load_from_toml("en").unwrap();
    km.switch_language("en").unwrap();
    let src = "import main_compiler.{CompileSeenProgram}".to_string();
    let lexer = Lexer::new(src, std::sync::Arc::new(km));
    let mut parser = Parser::new(lexer);
    let prog = parser.parse_program().unwrap();
    assert_eq!(prog.expressions.len(), 1);
    match &prog.expressions[0] {
        crate::ast::Expression::Import { module_path, symbols, .. } => {
            assert_eq!(module_path, &vec!["main_compiler".to_string()]);
            assert_eq!(symbols, &vec!["CompileSeenProgram".to_string()]);
        }
        other => panic!("expected Import, got {:?}", other),
    }
}

