extern crate seen_lexer;
extern crate seen_parser;
extern crate seen_ir;

use seen_lexer::{Lexer, KeywordManager};
use seen_parser::Parser;
use seen_ir::IRGenerator;
use std::sync::Arc;

fn main() {
    let source = r#"
var count = 0
while count < 5 {
    count = count + 1
}
count
"#;

    // Tokenize
    let keyword_manager = Arc::new(KeywordManager::new());
    let lexer = Lexer::new(source.to_string(), keyword_manager);
    
    // Parse
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program().unwrap();
    
    // Generate IR
    let mut generator = IRGenerator::new();
    let ir_program = generator.generate(&program).unwrap();
    
    // Debug print the raw IR structure
    for module in &ir_program.modules {
        println!("Module: {}", module.name);
        for function in module.functions.values() {
            println!("Function: {}", function.name);
            if let Some(entry) = &function.cfg.entry_block {
                if let Some(block) = function.cfg.get_block(entry) {
                    println!("  Entry Block: {}", entry);
                    println!("  Instructions ({} total):", block.instructions.len());
                    for (i, inst) in block.instructions.iter().enumerate() {
                        println!("    {}: {:?}", i, inst);
                    }
                    println!("  Terminator:");
                    if let Some(term) = &block.terminator {
                        println!("    {:?}", term);
                    } else {
                        println!("    None");
                    }
                }
            }
        }
    }
}
