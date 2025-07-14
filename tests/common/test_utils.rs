// Test utilities for the Seen language test suite
// This module provides common test helpers and fixtures

use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Test fixture builder for creating test environments
pub struct TestFixture {
    temp_dir: TempDir,
    project_name: String,
}

impl TestFixture {
    /// Create a new test fixture with a temporary directory
    pub fn new(project_name: &str) -> Result<Self> {
        let temp_dir = TempDir::new()?;
        Ok(Self {
            temp_dir,
            project_name: project_name.to_string(),
        })
    }

    /// Get the path to the temporary directory
    pub fn temp_dir(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Get the project directory path
    pub fn project_dir(&self) -> PathBuf {
        self.temp_dir.path().join(&self.project_name)
    }

    /// Create a basic project structure for testing
    pub fn create_project_structure(&self) -> Result<()> {
        let project_dir = self.project_dir();
        fs::create_dir_all(project_dir.join("src"))?;

        // Create a basic seen.toml
        let config_content = format!(
            r#"[package]
name = "{}"
version = "0.1.0"
language = "english"

[dependencies]

[build]
"#,
            self.project_name
        );
        fs::write(project_dir.join("seen.toml"), config_content)?;

        // Create a basic main.seen
        let main_content = r#"func main() {
    println("Hello from test!");
}"#;
        fs::write(project_dir.join("src/main.seen"), main_content)?;

        Ok(())
    }

    /// Create a source file with given content
    pub fn create_source_file(&self, path: &str, content: &str) -> Result<()> {
        let file_path = self.project_dir().join("src").join(path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(file_path, content)?;
        Ok(())
    }

    /// Read a file from the project
    pub fn read_file(&self, path: &str) -> Result<String> {
        let file_path = self.project_dir().join(path);
        Ok(fs::read_to_string(file_path)?)
    }

    /// Check if a file exists in the project
    pub fn file_exists(&self, path: &str) -> bool {
        self.project_dir().join(path).exists()
    }
}

/// Custom assertions for compiler-specific testing
pub mod assertions {
    use seen_lexer::token::{Token, TokenType};
    use seen_parser::ast::*;

    /// Assert two AST nodes are equivalent (ignoring source locations)
    pub fn assert_ast_eq(actual: &Program, expected: &Program) {
        assert_eq!(
            actual.declarations.len(),
            expected.declarations.len(),
            "Number of declarations should match"
        );

        for (actual_decl, expected_decl) in actual.declarations.iter().zip(&expected.declarations) {
            match (actual_decl, expected_decl) {
                (Declaration::Function(a), Declaration::Function(e)) => {
                    assert_eq!(a.name, e.name, "Function names should match");
                    assert_eq!(a.parameters.len(), e.parameters.len(), "Parameter count should match");
                    assert_eq!(a.return_type, e.return_type, "Return types should match");
                }
                (Declaration::Variable(a), Declaration::Variable(e)) => {
                    assert_eq!(a.name, e.name, "Variable names should match");
                    assert_eq!(a.is_mutable, e.is_mutable, "Mutability should match");
                    assert_eq!(a.var_type, e.var_type, "Variable types should match");
                }
                (Declaration::Struct(a), Declaration::Struct(e)) => {
                    assert_eq!(a.name, e.name, "Struct names should match");
                    assert_eq!(a.fields.len(), e.fields.len(), "Field count should match");
                }
                _ => panic!("Declaration types don't match"),
            }
        }
    }

    /// Assert token streams are equivalent (ignoring locations)
    pub fn assert_tokens_eq(actual: &[Token], expected: &[TokenType]) {
        assert_eq!(
            actual.len(),
            expected.len(),
            "Token count should match. Actual: {:?}, Expected: {:?}",
            actual.iter().map(|t| &t.token_type).collect::<Vec<_>>(),
            expected
        );

        for (actual_token, expected_type) in actual.iter().zip(expected) {
            assert_eq!(
                actual_token.token_type, *expected_type,
                "Token types should match at position"
            );
        }
    }

    /// Assert that a Result contains an error with a specific substring
    pub fn assert_error_contains<T, E: std::fmt::Display>(result: &Result<T, E>, substring: &str) {
        match result {
            Ok(_) => panic!("Expected error containing '{}', but got Ok", substring),
            Err(e) => {
                let error_str = e.to_string();
                assert!(
                    error_str.contains(substring),
                    "Error message '{}' should contain '{}'",
                    error_str,
                    substring
                );
            }
        }
    }
}

/// Test data builders using the builder pattern
pub mod builders {
    use seen_lexer::token::{Location, Position, Token, TokenType};
    use seen_parser::ast::*;

    /// Builder for creating test tokens
    pub struct TokenBuilder {
        tokens: Vec<Token>,
        line: u32,
        column: u32,
    }

    impl TokenBuilder {
        pub fn new() -> Self {
            Self {
                tokens: Vec::new(),
                line: 1,
                column: 1,
            }
        }

        /// Add a token with automatic position tracking
        pub fn add_token(mut self, token_type: TokenType, lexeme: &str) -> Self {
            let start_pos = Position::new(self.line, self.column);
            self.column += lexeme.len() as u32;
            let end_pos = Position::new(self.line, self.column);

            self.tokens.push(Token {
                token_type,
                lexeme: lexeme.to_string(),
                location: Location::new(start_pos, end_pos),
                language: "test".to_string(),
            });

            // Add space after token
            self.column += 1;
            self
        }

        /// Add a newline to advance position tracking
        pub fn newline(mut self) -> Self {
            self.line += 1;
            self.column = 1;
            self
        }

        /// Build the final token vector with EOF
        pub fn build(mut self) -> Vec<Token> {
            // Add EOF token
            let pos = Position::new(self.line, self.column);
            self.tokens.push(Token {
                token_type: TokenType::EOF,
                lexeme: String::new(),
                location: Location::new(pos, pos),
                language: "test".to_string(),
            });
            self.tokens
        }
    }

    /// Builder for creating test AST nodes
    pub struct AstBuilder;

    impl AstBuilder {
        /// Create a simple variable declaration
        pub fn variable(name: &str, value: i64, is_mutable: bool) -> Declaration {
            Declaration::Variable(VariableDeclaration {
                is_mutable,
                name: name.to_string(),
                var_type: None,
                initializer: Box::new(Expression::Literal(LiteralExpression::Number(
                    NumberLiteral {
                        value: value.to_string(),
                        is_float: false,
                        location: Location::dummy(),
                    }
                ))),
                location: Location::dummy(),
            })
        }

        /// Create a simple function declaration
        pub fn function(name: &str, params: Vec<(&str, &str)>, body: Vec<Statement>) -> Declaration {
            Declaration::Function(FunctionDeclaration {
                name: name.to_string(),
                parameters: params
                    .into_iter()
                    .map(|(name, type_name)| Parameter {
                        name: name.to_string(),
                        param_type: Type::Simple(type_name.to_string()),
                        location: Location::dummy(),
                    })
                    .collect(),
                return_type: None,
                body: Block {
                    statements: body,
                    location: Location::dummy(),
                },
                location: Location::dummy(),
            })
        }

        /// Create a struct declaration
        pub fn struct_decl(name: &str, fields: Vec<(&str, &str)>) -> Declaration {
            Declaration::Struct(StructDeclaration {
                name: name.to_string(),
                fields: fields
                    .into_iter()
                    .map(|(field_name, type_name)| StructField {
                        name: field_name.to_string(),
                        field_type: Type::Simple(type_name.to_string()),
                        location: Location::dummy(),
                    })
                    .collect(),
                location: Location::dummy(),
            })
        }
    }
}

/// Helper to create a dummy Location for testing
impl Location {
    pub fn dummy() -> Self {
        let pos = Position::new(1, 1);
        Location::new(pos, pos)
    }
}

/// Snapshot testing utilities
pub mod snapshot {
    use anyhow::Result;
    use std::fs;
    use std::path::Path;

    /// Compare actual output with expected snapshot
    pub fn assert_snapshot(name: &str, actual: &str) -> Result<()> {
        let snapshot_dir = Path::new("tests/snapshots");
        fs::create_dir_all(&snapshot_dir)?;

        let snapshot_file = snapshot_dir.join(format!("{}.snap", name));

        if snapshot_file.exists() {
            let expected = fs::read_to_string(&snapshot_file)?;
            assert_eq!(
                actual.trim(),
                expected.trim(),
                "Snapshot '{}' doesn't match",
                name
            );
        } else {
            // Create new snapshot
            fs::write(&snapshot_file, actual)?;
            println!("Created new snapshot: {}", snapshot_file.display());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixture_creation() -> Result<()> {
        let fixture = TestFixture::new("test_project")?;
        fixture.create_project_structure()?;

        assert!(fixture.file_exists("seen.toml"));
        assert!(fixture.file_exists("src/main.seen"));

        let config = fixture.read_file("seen.toml")?;
        assert!(config.contains("name = \"test_project\""));

        Ok(())
    }

    #[test]
    fn test_token_builder() {
        use builders::TokenBuilder;
        use TokenType::*;

        let tokens = TokenBuilder::new()
            .add_token(Func, "func")
            .add_token(Identifier, "main")
            .add_token(LeftParen, "(")
            .add_token(RightParen, ")")
            .add_token(LeftBrace, "{")
            .newline()
            .add_token(Identifier, "println")
            .build();

        assert_eq!(tokens.len(), 7); // 6 tokens + EOF
        assert_eq!(tokens[0].token_type, Func);
        assert_eq!(tokens[1].token_type, Identifier);
        assert_eq!(tokens[1].lexeme, "main");
    }
}