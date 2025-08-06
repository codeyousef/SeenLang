//! Pretty printing utilities for code and data formatting
//!
//! High-performance pretty printer optimized for compiler output:
//! - Code formatting with configurable styles
//! - Data structure visualization
//! - Diagnostic output formatting
//! - Indentation and layout management

use crate::string::String;
use crate::collections::Vec;

/// Pretty printing configuration
#[derive(Debug, Clone)]
pub struct PrettyConfig {
    /// Indentation string (spaces or tabs)
    pub indent: String,
    /// Maximum line width before wrapping
    pub max_width: usize,
    /// Whether to use compact mode
    pub compact: bool,
    /// Whether to show type annotations
    pub show_types: bool,
    /// Whether to use colors (ANSI codes)
    pub use_colors: bool,
}

impl Default for PrettyConfig {
    fn default() -> Self {
        PrettyConfig {
            indent: String::from("  "), // 2 spaces
            max_width: 80,
            compact: false,
            show_types: false,
            use_colors: false,
        }
    }
}

/// Pretty printer document structure
#[derive(Debug, Clone)]
pub enum Doc {
    /// Plain text
    Text(String),
    /// Line break
    Line,
    /// Concatenation of documents
    Concat(Vec<Doc>),
    /// Nested document with increased indentation
    Nest(usize, Box<Doc>),
    /// Group that can be flattened to single line if it fits
    Group(Box<Doc>),
    /// Alternative layouts: try first, fallback to second if too wide
    Alt(Box<Doc>, Box<Doc>),
    /// Colored text (ANSI codes)
    Color(Color, Box<Doc>),
}

/// ANSI color codes for terminal output
#[derive(Debug, Clone, Copy)]
pub enum Color {
    Reset,
    Bold,
    Red,
    Green, 
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    Gray,
}

impl Color {
    fn to_ansi_code(self) -> &'static str {
        match self {
            Color::Reset => "\x1b[0m",
            Color::Bold => "\x1b[1m",
            Color::Red => "\x1b[31m",
            Color::Green => "\x1b[32m",
            Color::Yellow => "\x1b[33m",
            Color::Blue => "\x1b[34m",
            Color::Magenta => "\x1b[35m",
            Color::Cyan => "\x1b[36m",
            Color::White => "\x1b[37m",
            Color::Gray => "\x1b[90m",
        }
    }
}

/// Pretty printer engine
pub struct PrettyPrinter {
    config: PrettyConfig,
}

impl PrettyPrinter {
    /// Create a new pretty printer with default config
    pub fn new() -> Self {
        PrettyPrinter {
            config: PrettyConfig::default(),
        }
    }

    /// Create a pretty printer with custom config
    pub fn with_config(config: PrettyConfig) -> Self {
        PrettyPrinter { config }
    }

    /// Render a document to string
    pub fn render(&self, doc: &Doc) -> String {
        let mut output = String::new();
        self.render_doc(doc, 0, &mut output);
        output
    }

    /// Render document with specific width limit
    pub fn render_with_width(&self, doc: &Doc, width: usize) -> String {
        let mut config = self.config.clone();
        config.max_width = width;
        let printer = PrettyPrinter::with_config(config);
        printer.render(doc)
    }

    /// Check if document fits on single line within width
    pub fn fits(&self, doc: &Doc, width: usize) -> bool {
        let rendered = self.render_flat(doc);
        rendered.len() <= width
    }

    /// Render document as single line (flatten)
    fn render_flat(&self, doc: &Doc) -> String {
        let mut output = String::new();
        self.render_flat_doc(doc, &mut output);
        output
    }

    /// Internal rendering with indentation tracking
    fn render_doc(&self, doc: &Doc, indent: usize, output: &mut String) {
        match doc {
            Doc::Text(text) => {
                output.push_str(text.as_str());
            }
            Doc::Line => {
                output.push('\n');
                for _ in 0..indent {
                    output.push_str(self.config.indent.as_str());
                }
            }
            Doc::Concat(docs) => {
                for d in docs {
                    self.render_doc(d, indent, output);
                }
            }
            Doc::Nest(extra_indent, inner) => {
                self.render_doc(inner, indent + extra_indent, output);
            }
            Doc::Group(inner) => {
                // Try to fit on single line first
                let flat = self.render_flat(inner);
                let current_line_len = self.current_line_length(output);
                
                if current_line_len + flat.len() <= self.config.max_width {
                    output.push_str(&flat);
                } else {
                    self.render_doc(inner, indent, output);
                }
            }
            Doc::Alt(primary, fallback) => {
                let primary_flat = self.render_flat(primary);
                let current_line_len = self.current_line_length(output);
                
                if current_line_len + primary_flat.len() <= self.config.max_width {
                    self.render_doc(primary, indent, output);
                } else {
                    self.render_doc(fallback, indent, output);
                }
            }
            Doc::Color(color, inner) => {
                if self.config.use_colors {
                    output.push_str(color.to_ansi_code());
                    self.render_doc(inner, indent, output);
                    output.push_str(Color::Reset.to_ansi_code());
                } else {
                    self.render_doc(inner, indent, output);
                }
            }
        }
    }

    /// Render document flattened (no line breaks)
    fn render_flat_doc(&self, doc: &Doc, output: &mut String) {
        match doc {
            Doc::Text(text) => {
                output.push_str(text.as_str());
            }
            Doc::Line => {
                output.push(' '); // Replace line break with space
            }
            Doc::Concat(docs) => {
                for d in docs {
                    self.render_flat_doc(d, output);
                }
            }
            Doc::Nest(_, inner) => {
                self.render_flat_doc(inner, output);
            }
            Doc::Group(inner) => {
                self.render_flat_doc(inner, output);
            }
            Doc::Alt(primary, _) => {
                self.render_flat_doc(primary, output);
            }
            Doc::Color(color, inner) => {
                if self.config.use_colors {
                    output.push_str(color.to_ansi_code());
                    self.render_flat_doc(inner, output);
                    output.push_str(Color::Reset.to_ansi_code());
                } else {
                    self.render_flat_doc(inner, output);
                }
            }
        }
    }

    /// Get length of current line in output
    fn current_line_length(&self, output: &String) -> usize {
        let text = output.as_str();
        if let Some(last_newline) = text.rfind('\n') {
            text.len() - last_newline - 1
        } else {
            text.len()
        }
    }
}

impl Default for PrettyPrinter {
    fn default() -> Self {
        Self::new()
    }
}

// Document construction helpers
impl Doc {
    /// Create text document
    pub fn text(s: &str) -> Doc {
        Doc::Text(String::from(s))
    }

    /// Create line break
    pub fn line() -> Doc {
        Doc::Line
    }

    /// Create space
    pub fn space() -> Doc {
        Doc::Text(String::from(" "))
    }

    /// Concatenate documents
    pub fn concat(docs: Vec<Doc>) -> Doc {
        Doc::Concat(docs)
    }

    /// Nest document with extra indentation
    pub fn nest(indent: usize, doc: Doc) -> Doc {
        Doc::Nest(indent, Box::new(doc))
    }

    /// Group document for layout choices
    pub fn group(doc: Doc) -> Doc {
        Doc::Group(Box::new(doc))
    }

    /// Alternative layouts
    pub fn alt(primary: Doc, fallback: Doc) -> Doc {
        Doc::Alt(Box::new(primary), Box::new(fallback))
    }

    /// Colored document
    pub fn color(color: Color, doc: Doc) -> Doc {
        Doc::Color(color, Box::new(doc))
    }

    /// Join documents with separator
    pub fn join(docs: Vec<Doc>, sep: Doc) -> Doc {
        if docs.is_empty() {
            return Doc::Text(String::new());
        }

        let mut result = Vec::new();
        for (i, doc) in docs.into_iter().enumerate() {
            if i > 0 {
                result.push(sep.clone());
            }
            result.push(doc);
        }
        Doc::Concat(result)
    }

    /// Surround document with brackets
    pub fn brackets(doc: Doc) -> Doc {
        let mut docs = Vec::new();
        docs.push(Doc::text("["));
        docs.push(doc);
        docs.push(Doc::text("]"));
        Doc::concat(docs)
    }

    /// Surround document with braces
    pub fn braces(doc: Doc) -> Doc {
        let mut docs = Vec::new();
        docs.push(Doc::text("{"));
        docs.push(doc);
        docs.push(Doc::text("}"));
        Doc::concat(docs)
    }

    /// Surround document with parentheses
    pub fn parens(doc: Doc) -> Doc {
        let mut docs = Vec::new();
        docs.push(Doc::text("("));
        docs.push(doc);
        docs.push(Doc::text(")"));
        Doc::concat(docs)
    }
}

/// High-level pretty printing functions
pub mod pretty {
    use super::*;

    /// Pretty print JSON-like structure
    pub fn json_object(fields: &[(String, String)]) -> Doc {
        if fields.is_empty() {
            return Doc::text("{}");
        }

        let mut field_docs = Vec::new();
        for (key, value) in fields {
            let mut field_doc = Vec::new();
            field_doc.push(Doc::text("\""));
            field_doc.push(Doc::text(key.as_str()));
            field_doc.push(Doc::text("\": "));
            field_doc.push(Doc::text(value.as_str()));
            field_docs.push(Doc::concat(field_doc));
        }

        let mut compact_sep = Vec::new();
        compact_sep.push(Doc::text(","));
        compact_sep.push(Doc::space());
        let compact_separator = Doc::concat(compact_sep);
        
        let mut expanded_sep = Vec::new();
        expanded_sep.push(Doc::text(","));
        expanded_sep.push(Doc::line());
        let expanded_separator = Doc::concat(expanded_sep);

        let inner = Doc::alt(
            // Compact: all on one line
            Doc::join(field_docs.clone(), compact_separator),
            // Expanded: each field on new line
            Doc::nest(1, Doc::join(field_docs, expanded_separator)),
        );

        let mut outer_content = Vec::new();
        outer_content.push(Doc::line());
        outer_content.push(inner.clone());
        outer_content.push(Doc::line());
        let outer_expanded = Doc::concat(outer_content);

        Doc::group(Doc::braces(Doc::alt(inner, outer_expanded)))
    }

    /// Pretty print array-like structure
    pub fn array(items: &Vec<String>) -> Doc {
        if items.is_empty() {
            return Doc::text("[]");
        }

        let mut item_docs = Vec::new();
        for item in items {
            item_docs.push(Doc::text(item.as_str()));
        }

        let mut compact_sep = Vec::new();
        compact_sep.push(Doc::text(","));
        compact_sep.push(Doc::space());
        let compact_separator = Doc::concat(compact_sep);
        
        let mut expanded_sep = Vec::new();
        expanded_sep.push(Doc::text(","));
        expanded_sep.push(Doc::line());
        let expanded_separator = Doc::concat(expanded_sep);

        let inner = Doc::alt(
            // Compact: all on one line
            Doc::join(item_docs.clone(), compact_separator),
            // Expanded: each item on new line
            Doc::nest(1, Doc::join(item_docs, expanded_separator)),
        );

        let mut outer_content = Vec::new();
        outer_content.push(Doc::line());
        outer_content.push(inner.clone());
        outer_content.push(Doc::line());
        let outer_expanded = Doc::concat(outer_content);

        Doc::group(Doc::brackets(Doc::alt(inner, outer_expanded)))
    }

    /// Pretty print code block with syntax highlighting
    pub fn code_block(lines: &Vec<String>, language: Option<&str>) -> Doc {
        let mut docs = Vec::new();

        if let Some(lang) = language {
            let mut lang_comment = String::from("// ");
            lang_comment.push_str(lang);
            docs.push(Doc::color(
                Color::Gray,
                Doc::text(lang_comment.as_str()),
            ));
            docs.push(Doc::line());
        }

        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                docs.push(Doc::line());
            }
            docs.push(highlight_code_line(line));
        }

        Doc::concat(docs)
    }

    /// Simple syntax highlighting for code lines
    fn highlight_code_line(line: &str) -> Doc {
        // Basic keyword highlighting
        let keywords = ["fn", "let", "if", "else", "for", "while", "match", "struct", "enum", "impl"];
        
        let result = String::from(line);
        for keyword in &keywords {
            let mut pattern = String::from(" ");
            pattern.push_str(keyword);
            pattern.push_str(" ");
            if result.contains(pattern.as_str()) {
                // In a real implementation, we'd use proper tokenization
                return Doc::color(Color::Blue, Doc::text(line));
            }
        }

        Doc::text(line)
    }

    /// Pretty print error message with context
    pub fn error_message(title: &str, message: &str, context: Option<&str>) -> Doc {
        let mut docs = Vec::new();

        // Error title in red
        let mut title_parts = Vec::new();
        title_parts.push(Doc::color(Color::Bold, Doc::text("Error: ")));
        title_parts.push(Doc::text(title));
        let title_doc = Doc::concat(title_parts);
        
        docs.push(Doc::color(Color::Red, title_doc));
        docs.push(Doc::line());

        // Message
        docs.push(Doc::text(message));

        // Context if provided
        if let Some(ctx) = context {
            docs.push(Doc::line());
            docs.push(Doc::line());
            docs.push(Doc::color(Color::Gray, Doc::text("Context:")));
            docs.push(Doc::line());
            docs.push(Doc::nest(2, Doc::text(ctx)));
        }

        Doc::concat(docs)
    }
}

#[cfg(test)]
mod tests;