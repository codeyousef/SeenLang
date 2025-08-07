use seen_typechecker::{TypeChecker, Type, PrimitiveType};

fn main() {
    let mut type_checker = TypeChecker::new();
    
    // Test that println is in the environment
    if let Some(println_type) = type_checker.env.get_function_type("println") {
        println!("✅ println function found in type environment");
        println!("   Type: {}", println_type);
    } else {
        println!("❌ println function NOT found in type environment");
    }
    
    // Test that print is in the environment  
    if let Some(print_type) = type_checker.env.get_function_type("print") {
        println!("✅ print function found in type environment");
        println!("   Type: {}", print_type);
    } else {
        println!("❌ print function NOT found in type environment");
    }
    
    // Test that debug is in the environment
    if let Some(debug_type) = type_checker.env.get_function_type("debug") {
        println!("✅ debug function found in type environment");
        println!("   Type: {}", debug_type);
    } else {
        println!("❌ debug function NOT found in type environment");
    }
    
    println!("\n🎯 Built-in function test completed!");
}