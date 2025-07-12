use anyhow::{bail, Context, Result};
use colored::*;
use std::fs;
use std::path::{Path, PathBuf};
use tera::{Context as TeraContext, Tera};

use crate::config::SeenConfig;

/// Create a new Seen project with the given name
pub fn create_new_project(name: &str) -> Result<()> {
    let project_dir = PathBuf::from(name);
    
    // Check if the directory already exists
    if project_dir.exists() {
        bail!("Directory already exists: {}", project_dir.display());
    }
    
    // Create the project directory
    fs::create_dir_all(&project_dir)
        .with_context(|| format!("Failed to create project directory: {}", project_dir.display()))?;
    
    // Create the src directory
    let src_dir = project_dir.join("src");
    fs::create_dir_all(&src_dir)
        .with_context(|| format!("Failed to create src directory: {}", src_dir.display()))?;
    
    // Create configuration file
    let config = SeenConfig::new(name);
    let config_path = project_dir.join("seen.toml");
    config.save(&config_path)
        .with_context(|| format!("Failed to create configuration file: {}", config_path.display()))?;
    
    // Create main.seen file with Hello World example
    create_main_file(&src_dir, &config.language)
        .context("Failed to create main.seen file")?;
    
    // Create README.md file
    create_readme_file(&project_dir, name)
        .context("Failed to create README.md file")?;
    
    println!("{} Created new Seen project: {}", "✓".green().bold(), name.blue().bold());
    println!("  {} {}", "→".cyan(), "Project structure:".bold());
    println!("    {} seen.toml", "•".yellow());
    println!("    {} README.md", "•".yellow());
    println!("    {} src/", "•".yellow());
    println!("      {} main.seen", "•".yellow());
    println!("\n  {} {}", "→".cyan(), "Next steps:".bold());
    println!("    cd {}", name);
    println!("    seen build");
    println!("    seen run");
    
    Ok(())
}

/// Create a main.seen file with a Hello World example
fn create_main_file(src_dir: &Path, language: &str) -> Result<()> {
    let main_path = src_dir.join("main.seen");
    
    let content = match language {
        "english" => r#"// Hello World program in Seen

func main() {
    println("Hello, World!");
}
"#,
        "arabic" => r#"// برنامج مرحبا بالعالم في لغة سين

دالة main() {
    اطبع("مرحبا بالعالم!");
}
"#,
        _ => bail!("Unsupported language: {}", language),
    };
    
    fs::write(&main_path, content)
        .with_context(|| format!("Failed to write main.seen file: {}", main_path.display()))?;
    
    Ok(())
}

/// Create a README.md file for the project
fn create_readme_file(project_dir: &Path, name: &str) -> Result<()> {
    let readme_path = project_dir.join("README.md");
    
    // Initialize a one-off template with Tera
    let mut tera = Tera::default();
    tera.add_raw_template("readme", r#"# {{ project_name }}

A project written in the Seen programming language.

## Building

To build this project, run:

```
seen build
```

## Running

To run this project, use:

```
seen run
```
"#)
        .context("Failed to create template for README")?;
    
    // Set up context with variables
    let mut context = TeraContext::new();
    context.insert("project_name", name);
    
    // Render the template
    let content = tera.render("readme", &context)
        .context("Failed to render README template")?;
    
    fs::write(&readme_path, content)
        .with_context(|| format!("Failed to write README.md file: {}", readme_path.display()))?;
    
    Ok(())
}
