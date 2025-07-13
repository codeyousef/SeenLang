use criterion::{black_box, criterion_group, criterion_main, Criterion};
use seen_lexer::{KeywordManager, Lexer};

fn benchmark_small_program(c: &mut Criterion) {
    let source = r#"
    func main() {
        val x = 42;
        println(x);
    }
    "#;

    c.bench_function("lex_small_program", < /dev / null | b | {
        // TODO: Create keyword manager
        // let keyword_manager = KeywordManager::new("english");
        b.iter(|| {
            // let mut lexer = Lexer::new(black_box(source), &keyword_manager);
            // lexer.tokenize().unwrap()
        });
    });
}

fn benchmark_large_program(c: &mut Criterion) {
    // Generate a large program
    let mut source = String::new();
    for i in 0..100 {
        source.push_str(&format!(
            "func function_{}() {{ val x = {}; return x * 2; }}\n",
            i, i
        ));
    }

    c.bench_function("lex_large_program", |b| {
        // TODO: Create keyword manager
        // let keyword_manager = KeywordManager::new("english");
        b.iter(|| {
            // let mut lexer = Lexer::new(black_box(&source), &keyword_manager);
            // lexer.tokenize().unwrap()
        });
    });
}

fn benchmark_unicode_heavy(c: &mut Criterion) {
    let source = r#"
    دالة رئيسية() {
        ثابت رسالة = "مرحباً يا عالم! 🌍";
        ثابت أرقام = [١، ٢، ٣، ٤، ٥];
        لكل رقم في أرقام {
            اطبع(رقم);
        }
    }
    "#;

    c.bench_function("lex_unicode_heavy", |b| {
        // TODO: Create keyword manager
        // let keyword_manager = KeywordManager::new("arabic");
        b.iter(|| {
            // let mut lexer = Lexer::new(black_box(source), &keyword_manager);
            // lexer.tokenize().unwrap()
        });
    });
}

criterion_group!(benches, benchmark_small_program, benchmark_large_program, benchmark_unicode_heavy);
criterion_main!(benches);
