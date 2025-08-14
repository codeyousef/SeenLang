extern crate seen_lexer;
extern crate seen_parser;
extern crate seen_ir;

use seen_lexer::{Lexer, Token, TokenType, KeywordManager};
use seen_parser::Parser;
use seen_ir::generator::IRGenerator;

fn main() {
    let source = r#"
var count = 0
while count < 5 {
    count = count + 1
}
count
"#;

    // Tokenize
    let keyword_manager = KeywordManager::new();
    let mut lexer = Lexer::new(source.to_string(), keyword_manager);
    let mut tokens = Vec::new();
    
    loop {
        let token = lexer.next_token().unwrap();
        if matches!(token.token_type, TokenType::EOF) {
            break;
        }
        tokens.push(token);
    }
    
    // Parse
    let mut parser = Parser::new(tokens);
    let program = parser.parse().unwrap();
    
    // Generate IR
    let mut generator = IRGenerator::new();
    let ir_program = generator.generate(&program).unwrap();
    
    // Debug print the IR
    for module in &ir_program.modules {
        println!("Module: {}", module.name);
        for function in module.functions.values() {
            println!("Function: {}", function.name);
            if let Some(entry) = &function.cfg.entry_block {
                if let Some(block) = function.cfg.get_block(entry) {
                    println!("  Block: {}", block.label.0);
                    println!("  Instructions:");
                    for inst in &block.instructions {
                        println!("    {:?}", inst);
                    }
                    println!("  Terminator:");
                    if let Some(term) = &block.terminator {
                        println!("    {:?}", term);
                    }
                }
            }
        }
    }
}