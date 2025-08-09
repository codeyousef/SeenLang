// Codegen Benchmark - Rust Implementation (simulates code generation)
use std::time::Instant;
use std::fmt::Write;

struct CodeGenerator {
    output: String,
    instructions_generated: usize,
}

impl CodeGenerator {
    fn new() -> Self {
        CodeGenerator {
            output: String::new(),
            instructions_generated: 0,
        }
    }
    
    fn generate_function(&mut self, name: &str, params: usize) {
        write!(&mut self.output, "function {}(", name).unwrap();
        for i in 0..params {
            if i > 0 {
                self.output.push_str(", ");
            }
            write!(&mut self.output, "arg{}", i).unwrap();
        }
        self.output.push_str(") {\n");
        self.instructions_generated += 1;
        
        // Generate body
        for i in 0..10 {
            writeln!(&mut self.output, "  mov r{}, {}", i, i).unwrap();
            writeln!(&mut self.output, "  add r{}, r{}", i, (i + 1) % 10).unwrap();
            self.instructions_generated += 2;
        }
        
        self.output.push_str("  ret\n}\n");
        self.instructions_generated += 1;
    }
    
    fn generate_loop(&mut self, iterations: usize) {
        writeln!(&mut self.output, "loop_{}:", iterations).unwrap();
        for i in 0..iterations {
            writeln!(&mut self.output, "  cmp r0, {}", i).unwrap();
            writeln!(&mut self.output, "  jne skip_{}", i).unwrap();
            writeln!(&mut self.output, "  call func_{}", i).unwrap();
            writeln!(&mut self.output, "skip_{}:", i).unwrap();
            self.instructions_generated += 3;
        }
    }
    
    fn generate_class(&mut self, name: &str) {
        writeln!(&mut self.output, "class {} {{", name).unwrap();
        for i in 0..5 {
            writeln!(&mut self.output, "  field{}: i32", i).unwrap();
        }
        for i in 0..3 {
            self.generate_function(&format!("method{}", i), i + 1);
        }
        self.output.push_str("}\n");
    }
    
    fn benchmark(&mut self, operations: usize) -> f64 {
        let start = Instant::now();
        
        for i in 0..operations {
            self.generate_function(&format!("func{}", i), i % 5);
            if i % 10 == 0 {
                self.generate_loop(5);
            }
            if i % 20 == 0 {
                self.generate_class(&format!("Class{}", i));
            }
        }
        
        start.elapsed().as_secs_f64()
    }
    
    fn get_instruction_count(&self) -> usize {
        self.instructions_generated
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let iterations = args.get(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(30);
    
    let mut times = Vec::new();
    let mut total_instructions = 0;
    
    for _ in 0..iterations {
        let mut gen = CodeGenerator::new();
        let time = gen.benchmark(100);
        times.push(time);
        total_instructions = gen.get_instruction_count();
    }
    
    let sum: f64 = times.iter().sum();
    let mean = sum / times.len() as f64;
    
    println!("{{");
    println!("  \"language\": \"rust\",");
    println!("  \"benchmark\": \"codegen\",");
    println!("  \"iterations\": {},", iterations);
    println!("  \"instructions_generated\": {},", total_instructions);
    print!("  \"times\": [");
    for (i, time) in times.iter().enumerate() {
        if i > 0 { print!(", "); }
        print!("{}", time);
    }
    println!("],");
    println!("  \"average_time\": {},", mean);
    println!("  \"instructions_per_second\": {}", total_instructions as f64 / mean);
    println!("}}");
}