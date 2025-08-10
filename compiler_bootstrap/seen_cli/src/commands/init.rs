//! Init command implementation

use anyhow::{Result, Context};
use std::path::PathBuf;
use log::{info, warn};

/// Execute the init command
pub fn execute(name: String, path: Option<PathBuf>, language: String) -> Result<()> {
    info!("Creating new Seen project: {}", name);
    
    // Validate project name
    if !is_valid_project_name(&name) {
        return Err(anyhow::anyhow!(
            "Invalid project name '{}'. Project names must be valid identifiers.",
            name
        ));
    }
    
    // Validate language
    if !is_supported_language(&language) {
        warn!("Language '{}' may not be fully supported. Supported languages: en, ar", language);
    }
    
    // Determine project directory
    let project_dir = if let Some(path) = path {
        path.join(&name)
    } else {
        std::env::current_dir()
            .context("Failed to get current directory")?
            .join(&name)
    };
    
    // Create project directory
    if project_dir.exists() {
        return Err(anyhow::anyhow!(
            "Directory '{}' already exists",
            project_dir.display()
        ));
    }
    
    std::fs::create_dir_all(&project_dir)
        .with_context(|| format!("Failed to create directory {}", project_dir.display()))?;
    
    // Create project structure
    create_project_structure(&project_dir, &name, &language)?;
    
    info!("Created Seen project '{}' in {}", name, project_dir.display());
    info!("Run 'cd {}' and 'seen build' to get started!", project_dir.display());
    
    Ok(())
}

fn is_valid_project_name(name: &str) -> bool {
    !name.is_empty() 
        && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        && !name.starts_with('-')
        && !name.ends_with('-')
}

fn is_supported_language(lang: &str) -> bool {
    matches!(lang, "en" | "ar")
}

fn create_project_structure(project_dir: &PathBuf, name: &str, language: &str) -> Result<()> {
    // Create directories
    let src_dir = project_dir.join("src");
    std::fs::create_dir_all(&src_dir)?;
    
    // Create Seen.toml
    let seen_toml = format!(
        r#"[project]
name = "{}"
version = "0.1.0"
language = "{}"

[build]
targets = ["native"]
optimize = "speed"

[format]
line-width = 100
indent = 4
trailing-comma = true
document-types = [".seen", ".md", ".toml"]
"#,
        name, language
    );
    
    std::fs::write(project_dir.join("Seen.toml"), seen_toml)?;
    
    // Create main source file
    let main_content = match language {
        "ar" => create_arabic_main_content(),
        _ => create_english_main_content(),
    };
    
    std::fs::write(src_dir.join("main.seen"), main_content)?;
    
    // Create .gitignore
    let gitignore_content = r#"# Seen build artifacts
/target/

# IDE files
.vscode/
.idea/
*.swp
*.swo

# OS files
.DS_Store
Thumbs.db
"#;
    
    std::fs::write(project_dir.join(".gitignore"), gitignore_content)?;
    
    // Create README
    let readme_content = format!(
        r#"# {}

A Seen programming language project.

## Building

```bash
seen build
```

## Running

```bash
seen run
```

## Testing

```bash
seen test
```
"#,
        name
    );
    
    std::fs::write(project_dir.join("README.md"), readme_content)?;
    
    Ok(())
}

fn create_english_main_content() -> &'static str {
    r#"// Welcome to Seen!
// This is your main function - the entry point of your program.

fun main() {
    println("Hello, World!");
    println("Welcome to the Seen programming language!");
    
    // Try modifying this code and run 'seen run' to see the changes
    let message = "Seen is fast, safe, and expressive";
    println(message);
}
"#
}

fn create_arabic_main_content() -> &'static str {
    r#"// أهلاً بك في المرئية!
// هذه هي دالتك الرئيسية - نقطة دخول برنامجك.

دالة رئيسية() {
    اطبع("مرحباً بالعالم!");
    اطبع("أهلاً بك في لغة البرمجة المرئية!");
    
    // جرب تعديل هذا الكود وشغل 'seen run' لترى التغييرات
    اجعل رسالة = "المرئية سريعة وآمنة ومعبرة";
    اطبع("{}", رسالة);
}
"#
}