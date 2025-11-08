use crate::{ParseResult, Parser};
use seen_lexer::{KeywordManager, Lexer};

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
            assert_eq!(symbols.len(), 1);
            assert_eq!(symbols[0].name, "CompileSeenProgram");
            assert!(symbols[0].alias.is_none());
        }
        other => panic!("expected Import, got {:?}", other),
    }
}

#[test]
fn parses_import_alias() {
    let mut km = KeywordManager::new();
    km.load_from_toml("en").unwrap();
    km.switch_language("en").unwrap();
    let src = "import parser.{Type as ASTType, Function}".to_string();
    let lexer = Lexer::new(src, std::sync::Arc::new(km));
    let mut parser = Parser::new(lexer);
    let prog = parser.parse_program().unwrap();
    assert_eq!(prog.expressions.len(), 1);
    match &prog.expressions[0] {
        crate::ast::Expression::Import { symbols, .. } => {
            assert_eq!(symbols.len(), 2);
            assert_eq!(symbols[0].name, "Type");
            assert_eq!(symbols[0].alias.as_deref(), Some("ASTType"));
            assert_eq!(symbols[1].name, "Function");
            assert!(symbols[1].alias.is_none());
        }
        other => panic!("expected Import, got {:?}", other),
    }
}

#[test]
fn parses_nested_module_import() {
    let mut km = KeywordManager::new();
    km.load_from_toml("en").unwrap();
    km.switch_language("en").unwrap();
    let src = "import typechecker.typechecker.{TypeChecker as RealTypeChecker}".to_string();
    let lexer = Lexer::new(src, std::sync::Arc::new(km));
    let mut parser = Parser::new(lexer);
    let prog = parser.parse_program().unwrap();
    assert_eq!(prog.expressions.len(), 1);
    match &prog.expressions[0] {
        crate::ast::Expression::Import { module_path, symbols, .. } => {
            assert_eq!(module_path, &vec!["typechecker".to_string(), "typechecker".to_string()]);
            assert_eq!(symbols.len(), 1);
            assert_eq!(symbols[0].name, "TypeChecker");
            assert_eq!(symbols[0].alias.as_deref(), Some("RealTypeChecker"));
        }
        other => panic!("expected Import, got {:?}", other),
    }
}
