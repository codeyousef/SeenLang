use seen_lexer::keyword_config::KeywordManager;
use seen_lexer::lexer::Lexer;
use seen_parser::parser::Parser;

fn main() {
    // Test enum declaration parsing
    let source = r#"
enum Color {
    Red,
    Green,
    Blue
}

enum Option {
    Some(Int),
    None
}

func main() {
    val color = Color.Red;
    
    match color {
        Color.Red => println("It's red!"),
        Color.Green => println("It's green!"),
        Color.Blue => println("It's blue!")
    }
    
    val maybe_number = Option.Some(42);
    
    match maybe_number {
        Option.Some(value) => println("Got value"),
        Option.None => println("No value")
    }
}
"#;

    // Create keyword manager
    let keyword_manager = KeywordManager::new("en").expect("Failed to create keyword manager");
    
    // Tokenize
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().expect("Failed to tokenize");
    
    println!("Tokens:");
    for token in &tokens {
        println!("  {:?}", token);
    }
    
    // Parse
    let mut parser = Parser::new(tokens);
    match parser.parse() {
        Ok(program) => {
            println!("\nParsing successful!");
            println!("Program has {} declarations", program.declarations.len());
            
            for (i, decl) in program.declarations.iter().enumerate() {
                println!("Declaration {}: {:?}", i + 1, decl);
            }
        }
        Err(err) => {
            println!("Parsing failed: {:?}", err);
        }
    }
}