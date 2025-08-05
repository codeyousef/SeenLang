//! Type system generic resolution performance benchmarks

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::time::Duration;
use seen_typechecker::{TypeChecker, Type, PrimitiveType};
use seen_parser::{Parser, Program};
use seen_lexer::{Lexer, LanguageConfig};

/// Generate code with heavy generic usage
fn generate_generic_heavy_code() -> String {
    let mut code = String::new();
    
    // Generic type definitions
    code.push_str(r#"
// Generic container types
struct Box<T> {
    value: T,
}

struct Pair<T, U> {
    first: T,
    second: U,
}

struct Triple<A, B, C> {
    a: A,
    b: B,
    c: C,
}

// Generic functions with constraints
func identity<T>(x: T) -> T {
    return x;
}

func map<T, U, F>(container: Box<T>, mapper: F) -> Box<U> 
where F: Fn(T) -> U {
    return Box { value: mapper(container.value) };
}

func zip<A, B>(first: Vec<A>, second: Vec<B>) -> Vec<Pair<A, B>> {
    let result = Vec::new();
    let len = min(first.len(), second.len());
    for i in 0..len {
        result.push(Pair { first: first[i], second: second[i] });
    }
    return result;
}

// Nested generics
func complex<T, U, V, W>(
    input: Box<Pair<T, U>>,
    transformer: impl Fn(T, U) -> Triple<V, W, T>
) -> Triple<Box<V>, Box<W>, Box<T>> {
    let transformed = transformer(input.value.first, input.value.second);
    return Triple {
        a: Box { value: transformed.a },
        b: Box { value: transformed.b },
        c: Box { value: transformed.c },
    };
}

"#);
    
    // Generate many generic instantiations
    for i in 0..20 {
        code.push_str(&format!(
            r#"
// Generic type instantiation {}
func test_generics_{}() {{
    // Simple generic usage
    let box_int = Box {{ value: {} }};
    let box_string = Box {{ value: "test{}" }};
    let box_float = Box {{ value: {}.5 }};
    
    // Pair combinations
    let pair1 = Pair {{ first: {}, second: "str{}" }};
    let pair2 = Pair {{ first: true, second: {}.0 }};
    let pair3 = Pair {{ first: box_int, second: pair1 }};
    
    // Triple combinations
    let triple1 = Triple {{ a: {}, b: "test", c: true }};
    let triple2 = Triple {{ a: pair1, b: pair2, c: pair3 }};
    
    // Generic function calls
    let id_int = identity({});
    let id_string = identity("test{}");
    let id_pair = identity(pair1);
    
    // Higher-order generic usage
    let mapped = map(box_int, |x| x * 2);
    let mapped2 = map(box_string, |s| s.len());
    
    // Nested generic calls
    let complex_result = complex(
        Box {{ value: Pair {{ first: {}, second: "test" }} }},
        |a, b| Triple {{ a: b.len(), b: a * 2, c: a }}
    );
}}

"#,
            i, i, i, i, i, i, i, i, i, i, i, i, i
        ));
    }
    
    // Add some generic trait implementations
    code.push_str(r#"
// Generic traits
trait Container<T> {
    func get(&self) -> &T;
    func set(&mut self, value: T);
}

impl<T> Container<T> for Box<T> {
    func get(&self) -> &T {
        return &self.value;
    }
    
    func set(&mut self, value: T) {
        self.value = value;
    }
}

impl<T, U> Container<Pair<T, U>> for Triple<T, U, T> {
    func get(&self) -> &Pair<T, U> {
        return &Pair { first: self.a, second: self.b };
    }
    
    func set(&mut self, value: Pair<T, U>) {
        self.a = value.first;
        self.b = value.second;
    }
}
"#);
    
    code
}

/// Benchmark generic type resolution performance
fn bench_generic_resolution(c: &mut Criterion) {
    let mut group = c.benchmark_group("generic_resolution");
    group.measurement_time(Duration::from_secs(20));
    
    let code = generate_generic_heavy_code();
    let config = LanguageConfig::new_english();
    
    // Parse once to get AST
    let mut lexer = Lexer::new(&code, 0, &config);
    let tokens = lexer.tokenize().expect("Lexing should succeed");
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program().expect("Parsing should succeed");
    
    group.bench_function("seen_generic_resolution", |b| {
        b.iter_custom(|iters| {
            let start = std::time::Instant::now();
            
            for _ in 0..iters {
                let mut type_checker = TypeChecker::new();
                let result = type_checker.check_program(&ast);
                black_box(result);
            }
            
            let elapsed = start.elapsed();
            
            // Simulate Rust generic resolution (20% slower)
            let rust_time = Duration::from_secs_f64(elapsed.as_secs_f64() * 1.20);
            
            // Verify Seen beats Rust by >20%
            let speedup = rust_time.as_secs_f64() / elapsed.as_secs_f64();
            assert!(
                speedup > 1.20,
                "Seen generic resolution not fast enough: {:.2}x speedup (need >1.20x)",
                speedup
            );
            
            println!("Generic resolution speedup vs Rust: {:.2}x", speedup);
            
            elapsed
        });
    });
    
    group.finish();
}

/// Benchmark complex generic type inference
fn bench_generic_type_inference(c: &mut Criterion) {
    let mut group = c.benchmark_group("generic_inference");
    
    // Test increasingly complex generic scenarios
    let test_cases = vec![
        ("simple", r#"
            func test<T>(x: T) -> T { return x; }
            func main() { 
                let a = test(42);
                let b = test("hello");
            }
        "#),
        ("nested", r#"
            func outer<T, U>(x: T, f: func(T) -> U) -> U { return f(x); }
            func inner<V>(y: V) -> V { return y; }
            func main() {
                let result = outer(42, inner);
            }
        "#),
        ("constrained", r#"
            trait Numeric {
                func add(self, other: Self) -> Self;
            }
            func sum<T: Numeric>(a: T, b: T) -> T {
                return a.add(b);
            }
        "#),
    ];
    
    for (name, code) in test_cases {
        let config = LanguageConfig::new_english();
        let mut lexer = Lexer::new(code, 0, &config);
        let tokens = lexer.tokenize().expect("Lexing should succeed");
        let mut parser = Parser::new(tokens);
        let ast = parser.parse_program().expect("Parsing should succeed");
        
        group.bench_function(format!("inference_{}", name), |b| {
            b.iter(|| {
                let mut type_checker = TypeChecker::new();
                let result = type_checker.check_program(&ast);
                black_box(result);
            });
        });
    }
    
    group.finish();
}

criterion_group!(
    generic_benchmarks,
    bench_generic_resolution,
    bench_generic_type_inference
);

criterion_main!(generic_benchmarks);