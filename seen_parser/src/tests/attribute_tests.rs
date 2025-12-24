//! Tests for attribute parsing (e.g. @[embed(path="...")]).

use crate::{AttributeArgument, AttributeValue, Expression, ParseResult, Parser, Program};
use seen_lexer::{KeywordManager, Lexer};
use std::sync::Arc;

fn parse_program(input: &str) -> ParseResult<Program> {
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    let lexer = Lexer::new(input.to_string(), Arc::new(keyword_manager));
    let mut parser = Parser::new(lexer);
    parser.parse_program()
}

#[test]
fn parse_embed_attribute_on_const() {
    let program =
        parse_program("@embed(path=\"shaders/triangle.spv\") const TRIANGLE = 0").unwrap();

    assert_eq!(program.expressions.len(), 1);

    match &program.expressions[0] {
        Expression::Const {
            attributes, name, ..
        } => {
            assert_eq!(name, "TRIANGLE");
            assert_eq!(attributes.len(), 1);
            match &attributes[0].args[..] {
                [AttributeArgument::Named { name, value }] => {
                    assert_eq!(name, "path");
                    match value {
                        AttributeValue::String(val) => {
                            assert_eq!(val, "shaders/triangle.spv");
                        }
                        other => panic!("unexpected attribute value: {:?}", other),
                    }
                }
                other => panic!("unexpected attribute arguments: {:?}", other),
            }
        }
        expr => panic!("expected const expression, got {:?}", expr),
    }
}
