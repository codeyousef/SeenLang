use criterion::{black_box, criterion_group, criterion_main, Criterion};
use seen_ir::{CodeGenerator, compile_program};
use seen_parser::parse_program;
use inkwell::context::Context;

fn benchmark_simple_program(c: &mut Criterion) {
    let source = r#"
    func main() {
        val x = 42;
        println(x);
    }
    "#;
    
    let program = parse_program(source).unwrap();
    
    c.bench_function("ir_gen_simple",  < /dev/null | b| {
        b.iter(|| {
            let context = Context::create();
            compile_program(&context, black_box(&program))
        });
    });
}

fn benchmark_complex_program(c: &mut Criterion) {
    let mut source = String::new();
    // Generate a program with many functions
    for i in 0..50 {
        source.push_str(&format!(
            "func function_{}(x: Int) -> Int {{ return x * {}; }}\n",
            i, i
        ));
    }
    source.push_str("func main() { println(function_0(42)); }");
    
    let program = parse_program(&source).unwrap();
    
    c.bench_function("ir_gen_complex", |b| {
        b.iter(|| {
            let context = Context::create();
            compile_program(&context, black_box(&program))
        });
    });
}

fn benchmark_optimization_passes(c: &mut Criterion) {
    let source = r#"
    func compute() -> Int {
        val a = 10;
        val b = 20;
        val c = a + b;
        val d = c * 2;
        val e = d / 2;
        return e;
    }
    "#;
    
    let program = parse_program(source).unwrap();
    
    c.bench_function("ir_gen_with_opts", |b| {
        b.iter(|| {
            let context = Context::create();
            let module = compile_program(&context, black_box(&program)).unwrap();
            // Run optimization passes
            module.run_passes("instcombine,reassociate,gvn,simplifycfg", 
                inkwell::targets::TargetMachine::get_default_triple().as_str().to_str().unwrap(),
                inkwell::OptimizationLevel::Default).unwrap();
        });
    });
}

criterion_group!(benches, benchmark_simple_program, benchmark_complex_program, benchmark_optimization_passes);
criterion_main!(benches);
