// Test nested generics parsing

use std::process::Command;

fn main() {
    let test_cases = vec![
        "func simple(): List<String> { return List.empty() }",
        "func nested(): Observable<List<String>> { return Observable.empty() }", 
        "suspend func suspend_nested(): Observable<List<User>> { return Observable.empty() }",
    ];

    for (i, case) in test_cases.iter().enumerate() {
        println!("Testing case {}: {}", i + 1, case);
        
        // Write test case to file
        std::fs::write("/tmp/test_case.seen", case).unwrap();
        
        // Try to parse with our parser
        let output = Command::new("cargo")
            .args(&["run", "--bin", "seen_parser_test", "/tmp/test_case.seen"])
            .current_dir("/mnt/d/Projects/Rust/seenlang")
            .output();
        
        match output {
            Ok(result) => {
                if result.status.success() {
                    println!("  ✅ PASSED");
                } else {
                    println!("  ❌ FAILED");
                    println!("    stderr: {}", String::from_utf8_lossy(&result.stderr));
                }
            }
            Err(e) => println!("  ❌ ERROR: {}", e),
        }
    }
}