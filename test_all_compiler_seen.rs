// Test parsing all compiler_seen files
use std::fs;
use seen_lexer::{Lexer, KeywordManager};
use seen_parser::Parser;
use std::sync::Arc;

fn main() {
    let files = vec![
        "compiler_seen/demo_test.seen",
        "compiler_seen/run_all_tests.seen", 
        "compiler_seen/run_complete_optimization_tests.seen",
        "compiler_seen/run_optimization_tests.seen",
        "compiler_seen/run_tests.seen",
        "compiler_seen/simple_test.seen",
        "compiler_seen/src/benchmarks/arithmetic_benchmarks.seen",
        "compiler_seen/src/bootstrap/mod.seen",
        "compiler_seen/src/bootstrap/rust_remover.seen",
        "compiler_seen/src/bootstrap/verifier.seen",
        "compiler_seen/src/codegen/complete_codegen.seen",
        "compiler_seen/src/codegen/llvm_backend.seen",
        "compiler_seen/src/codegen/real_codegen.seen",
        "compiler_seen/src/codegen/simple.seen",
        "compiler_seen/src/codegen/vectorization.seen",
        "compiler_seen/src/errors/diagnostics.seen",
        "compiler_seen/src/errors/error_types.seen",
        "compiler_seen/src/errors/formatter.seen",
        "compiler_seen/src/errors/mod.seen",
        "compiler_seen/src/lexer/complete_lexer.seen",
        "compiler_seen/src/lexer/language_config.seen",
        "compiler_seen/src/lexer/main.seen",
        "compiler_seen/src/lexer/real_lexer.seen",
        "compiler_seen/src/lexer/token.seen",
        "compiler_seen/src/lsp/server.seen",
        "compiler_seen/src/main.seen",
        "compiler_seen/src/main_compiler.seen",
        "compiler_seen/src/optimization/arch/architecture_optimizer.seen",
        "compiler_seen/src/optimization/egraph/cost_model.seen",
        "compiler_seen/src/optimization/egraph/egraph.seen",
        "compiler_seen/src/optimization/egraph/mod.seen",
        "compiler_seen/src/optimization/egraph/optimizer.seen",
        "compiler_seen/src/optimization/egraph/rewrite_rules.seen",
        "compiler_seen/src/optimization/memory/memory_optimizer.seen",
        "compiler_seen/src/optimization/ml/feature_extractor.seen",
        "compiler_seen/src/optimization/ml/ml_optimizer.seen",
        "compiler_seen/src/optimization/ml/model.seen",
        "compiler_seen/src/optimization/ml/training.seen",
        "compiler_seen/src/optimization/pgo/profiler.seen",
        "compiler_seen/src/optimization/pgo/profile_guided_optimizer.seen",
        "compiler_seen/src/optimization/superopt/mod.seen",
        "compiler_seen/src/optimization/superopt/program_synthesis.seen",
        "compiler_seen/src/optimization/superopt/superoptimizer.seen",
        "compiler_seen/src/optimization/superopt/z3_solver.seen",
        "compiler_seen/src/parser/ast.seen",
        "compiler_seen/src/parser/main.seen",
        "compiler_seen/src/parser/real_parser.seen",
        "compiler_seen/src/reactive/runtime.seen",
        "compiler_seen/src/runtime/runtime_intrinsics.seen",
        "compiler_seen/src/testing/assert.seen",
        "compiler_seen/src/testing/runner.seen",
        "compiler_seen/src/test_compiler.seen",
        "compiler_seen/src/tools/release_automation.seen",
        "compiler_seen/src/typechecker/main.seen",
        "compiler_seen/test_runner_bootstrap.seen",
    ];
    
    let keyword_manager = Arc::new(KeywordManager::new());
    let mut passed = 0;
    let mut failed = 0;
    
    println!("Testing parsing of all {} compiler_seen files...\n", files.len());
    
    for file_path in files {
        match fs::read_to_string(file_path) {
            Ok(content) => {
                let lexer = Lexer::new(content, keyword_manager.clone());
                let mut parser = Parser::new(lexer);
                
                match parser.parse_program() {
                    Ok(program) => {
                        println!("✓ {} - {} expressions", file_path, program.expressions.len());
                        passed += 1;
                    }
                    Err(e) => {
                        println!("✗ {} - Parse error: {}", file_path, e);
                        failed += 1;
                    }
                }
            }
            Err(e) => {
                println!("✗ {} - File error: {}", file_path, e);
                failed += 1;
            }
        }
    }
    
    println!("\n=== RESULTS ===");
    println!("Passed: {} ✓", passed);
    println!("Failed: {} ✗", failed);
    println!("Total:  {}", passed + failed);
    
    if failed == 0 {
        println!("\n🎉 ALL COMPILER_SEEN FILES PARSED SUCCESSFULLY!");
    } else {
        println!("\n❌ {} FILES FAILED TO PARSE", failed);
    }
}