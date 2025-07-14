//\! Integration tests for the Seen IR generation

use inkwell::context::Context;
use seen_ir::{compile_program, CodeGenerator};
use seen_parser::{parse_program, Program};
use std::process::Command;
use tempfile::NamedTempFile;

#[test]
fn test_hello_world_compilation() {
    let source = r#"
    func main() {
        println("Hello, World\!");
    }
    "#;

    // Parse the source code
    let program = parse_program(source).unwrap();

    // Generate LLVM IR
    let context = Context::create();
    let module = compile_program(&context, &program).unwrap();

    // Verify the module contains main function
    assert!(module.get_function("main").is_some());

    // Optionally: write to file and compile with LLVM
    let temp_file = NamedTempFile::new().unwrap();
    module.write_bitcode_to_path(temp_file.path());

    // Could compile with llc and link if LLVM tools are available
}

#[test]
fn test_arithmetic_program() {
    let source = r#"
    func add(a: Int, b: Int) -> Int {
        return a + b;
    }
    
    func main() {
        val result = add(5, 3);
        println(result);
    }
    "#;

    let program = parse_program(source).unwrap();
    let context = Context::create();
    let module = compile_program(&context, &program).unwrap();

    // Verify both functions exist
    assert!(module.get_function("add").is_some());
    assert!(module.get_function("main").is_some());
}

#[test]
fn test_control_flow_program() {
    let source = r#"
    func max(a: Int, b: Int) -> Int {
        if a > b {
            return a;
        } else {
            return b;
        }
    }
    
    func main() {
        val result = max(10, 20);
        println(result);
    }
    "#;

    let program = parse_program(source).unwrap();
    let context = Context::create();
    let module = compile_program(&context, &program).unwrap();

    assert!(module.get_function("max").is_some());
    assert!(module.get_function("main").is_some());
}

#[test]
fn test_loop_program() {
    let source = r#"
    func sum_to_n(n: Int) -> Int {
        var sum = 0;
        var i = 1;
        while i <= n {
            sum = sum + i;
            i = i + 1;
        }
        return sum;
    }
    
    func main() {
        val result = sum_to_n(10);
        println(result);
    }
    "#;

    let program = parse_program(source).unwrap();
    let context = Context::create();
    let module = compile_program(&context, &program).unwrap();

    assert!(module.get_function("sum_to_n").is_some());
    assert!(module.get_function("main").is_some());
}

#[test]
fn test_recursive_function() {
    let source = r#"
    func factorial(n: Int) -> Int {
        if n <= 1 {
            return 1;
        }
        return n * factorial(n - 1);
    }
    
    func main() {
        val result = factorial(5);
        println(result);
    }
    "#;

    let program = parse_program(source).unwrap();
    let context = Context::create();
    let module = compile_program(&context, &program).unwrap();

    assert!(module.get_function("factorial").is_some());
    assert!(module.get_function("main").is_some());
}

#[test]
fn test_multiple_types() {
    let source = r#"
    func test_types() {
        val i: Int = 42;
        val f: Float = 3.14;
        val b: Bool = true;
        val s: String = "test";
        
        println(i);
        println(f);
        println(b);
        println(s);
    }
    "#;

    let program = parse_program(source).unwrap();
    let context = Context::create();
    let result = compile_program(&context, &program);

    // This should compile successfully
    assert!(result.is_ok());
}

#[test]
fn test_error_undefined_variable() {
    let source = r#"
    func main() {
        println(undefined_var);
    }
    "#;

    let program = parse_program(source).unwrap();
    let context = Context::create();
    let result = compile_program(&context, &program);

    // This should fail with undefined variable error
    assert!(result.is_err());
}

#[test]
fn test_error_type_mismatch() {
    let source = r#"
    func main() {
        val x: Int = "not an integer";
    }
    "#;

    let program = parse_program(source).unwrap();
    let context = Context::create();
    let result = compile_program(&context, &program);

    // This should fail with type mismatch error
    assert!(result.is_err());
}

#[test]
fn test_error_undefined_function() {
    let source = r#"
    func main() {
        undefined_function();
    }
    "#;

    let program = parse_program(source).unwrap();
    let context = Context::create();
    let result = compile_program(&context, &program);

    // This should fail with undefined function error
    assert!(result.is_err());
}
