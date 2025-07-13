use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

/// A simple utility to convert LLVM IR to an executable
///
/// Usage: ir_to_executable <input.ll> <output>
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: ir_to_executable <input.ll> <output>");
        std::process::exit(1);
    }

    let input_file = &args[1];
    let output_file = &args[2];

    // Check that the input file exists
    if !Path::new(input_file).exists() {
        eprintln!("Input file does not exist: {}", input_file);
        std::process::exit(1);
    }

    // Temporary file paths
    let temp_dir = env::temp_dir();
    let object_file = temp_dir.join("temp_object.o");
    let object_file_path = object_file.to_str().unwrap();

    println!("Converting LLVM IR to object file...");

    // Use LLVM tools to convert IR to object file
    let llc_status = Command::new("llc")
        .args(["-filetype=obj", input_file, "-o", object_file_path])
        .status()?;

    if !llc_status.success() {
        eprintln!("Failed to convert IR to object file");
        std::process::exit(1);
    }

    println!("Linking object file to create executable...");

    // Link the object file to create an executable
    let clang_status = Command::new("clang")
        .args([object_file_path, "-o", output_file])
        .status()?;

    if !clang_status.success() {
        eprintln!("Failed to link object file");
        std::process::exit(1);
    }

    // Clean up temporary files
    fs::remove_file(object_file)?;

    println!("Successfully created executable: {}", output_file);

    Ok(())
}
