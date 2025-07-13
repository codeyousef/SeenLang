mod test_harness;
mod hello_world_tests;

fn main() {
    println!("Seen Language E2E Test Runner");
    println!("============================\n");

    // Run Hello World tests
    let success = hello_world_tests::run_hello_world_tests();

    // Exit with appropriate code
    if success {
        println!("\nAll tests passed!");
        std::process::exit(0);
    } else {
        println!("\nSome tests failed!");
        std::process::exit(1);
    }
}
