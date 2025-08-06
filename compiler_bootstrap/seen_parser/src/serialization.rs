//! AST serialization and deserialization module
//!
//! Provides efficient serialization of AST nodes to various formats
//! for persistence, caching, and debugging purposes.

use crate::ast::*;
use serde::{Serialize, Deserialize};
use serde_json;
use bincode;
use std::io::{Read, Write};
use thiserror::Error;

/// Serialization format options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerializationFormat {
    /// Standard JSON format with all fields
    Json,
    /// Compact JSON with minimal whitespace and abbreviated field names
    CompactJson,
    /// Binary format using bincode
    Binary,
    /// MessagePack format for efficient binary serialization
    MessagePack,
}

/// Serialization errors
#[derive(Debug, Error)]
pub enum SerializationError {
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Binary serialization error: {0}")]
    Binary(#[from] bincode::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Unsupported format: {0:?}")]
    UnsupportedFormat(SerializationFormat),
}

/// Deserialization errors
#[derive(Debug, Error)]
pub enum DeserializationError {
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
    #[error("Version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: String, actual: String },
    #[error("JSON deserialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Binary deserialization error: {0}")]
    Binary(#[from] bincode::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Version information for serialized AST
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

/// Wrapper for versioned AST serialization
#[derive(Debug, Serialize, Deserialize)]
struct VersionedAst<T> {
    version: Version,
    #[serde(flatten)]
    ast: T,
}

/// Options for controlling serialization
#[derive(Debug, Clone)]
pub struct SerializationOptions {
    pub pretty_print: bool,
    pub include_spans: bool,
    pub include_attributes: bool,
    pub include_node_ids: bool,
}

impl Default for SerializationOptions {
    fn default() -> Self {
        Self {
            pretty_print: false,
            include_spans: true,
            include_attributes: true,
            include_node_ids: true,
        }
    }
}

/// AST serializer
pub struct AstSerializer {
    format: SerializationFormat,
    options: SerializationOptions,
    version: Option<Version>,
}

impl AstSerializer {
    /// Create a new serializer with the specified format
    pub fn new(format: SerializationFormat) -> Self {
        Self {
            format,
            options: SerializationOptions::default(),
            version: None,
        }
    }

    /// Set version information
    pub fn with_version(mut self, major: u32, minor: u32, patch: u32) -> Self {
        self.version = Some(Version { major, minor, patch });
        self
    }

    /// Enable pretty printing for JSON formats
    pub fn with_pretty_print(mut self, pretty: bool) -> Self {
        self.options.pretty_print = pretty;
        self
    }

    /// Control whether to include source spans
    pub fn with_include_spans(mut self, include: bool) -> Self {
        self.options.include_spans = include;
        self
    }

    /// Control whether to include attributes
    pub fn with_include_attributes(mut self, include: bool) -> Self {
        self.options.include_attributes = include;
        self
    }

    /// Serialize an AST node to bytes
    pub fn serialize<T>(&self, ast: &T) -> Result<Vec<u8>, SerializationError>
    where
        T: Serialize,
    {
        let data = if let Some(ref version) = self.version {
            let versioned = VersionedAst {
                version: version.clone(),
                ast,
            };
            self.serialize_internal(&versioned)?
        } else {
            self.serialize_internal(ast)?
        };
        
        Ok(data)
    }

    /// Serialize to a writer/stream
    pub fn serialize_to_stream<T, W>(&self, ast: &T, writer: &mut W) -> Result<(), SerializationError>
    where
        T: Serialize,
        W: Write,
    {
        let data = self.serialize(ast)?;
        writer.write_all(&data)?;
        Ok(())
    }

    fn serialize_internal<T>(&self, value: &T) -> Result<Vec<u8>, SerializationError>
    where
        T: Serialize,
    {
        match self.format {
            SerializationFormat::Json => {
                if self.options.pretty_print {
                    Ok(serde_json::to_vec_pretty(value)?)
                } else {
                    Ok(serde_json::to_vec(value)?)
                }
            }
            SerializationFormat::CompactJson => {
                // Use custom compact serialization
                let json_value = serde_json::to_value(value)?;
                let compacted = compact_json(json_value);
                Ok(serde_json::to_vec(&compacted)?)
            }
            SerializationFormat::Binary => {
                Ok(bincode::serialize(value)?)
            }
            SerializationFormat::MessagePack => {
                // For now, fallback to bincode
                // In production, would use rmp-serde
                Ok(bincode::serialize(value)?)
            }
        }
    }
}

/// AST deserializer
pub struct AstDeserializer {
    format: SerializationFormat,
    check_version: bool,
    expected_version: Option<Version>,
}

impl AstDeserializer {
    /// Create a new deserializer with the specified format
    pub fn new(format: SerializationFormat) -> Self {
        Self {
            format,
            check_version: false,
            expected_version: None,
        }
    }

    /// Enable version checking during deserialization
    pub fn with_version_check(mut self, check: bool) -> Self {
        self.check_version = check;
        self
    }

    /// Set expected version for validation
    pub fn with_expected_version(mut self, major: u32, minor: u32, patch: u32) -> Self {
        self.expected_version = Some(Version { major, minor, patch });
        self.check_version = true;
        self
    }

    /// Deserialize from bytes
    pub fn deserialize<'a, T>(&self, data: &'a [u8]) -> Result<T, DeserializationError>
    where
        T: Deserialize<'a>,
    {
        match self.format {
            SerializationFormat::Json | SerializationFormat::CompactJson => {
                if self.check_version {
                    let versioned: VersionedAst<T> = serde_json::from_slice(data)?;
                    if let Some(ref expected) = self.expected_version {
                        if !version_compatible(&versioned.version, expected) {
                            return Err(DeserializationError::VersionMismatch {
                                expected: format!("{}.{}.{}", expected.major, expected.minor, expected.patch),
                                actual: format!("{}.{}.{}", versioned.version.major, versioned.version.minor, versioned.version.patch),
                            });
                        }
                    }
                    Ok(versioned.ast)
                } else {
                    Ok(serde_json::from_slice(data)?)
                }
            }
            SerializationFormat::Binary | SerializationFormat::MessagePack => {
                Ok(bincode::deserialize(data)?)
            }
        }
    }

    /// Deserialize from a reader/stream
    pub fn deserialize_from_stream<T, R>(&self, reader: &mut R) -> Result<T, DeserializationError>
    where
        T: for<'de> Deserialize<'de>,
        R: Read,
    {
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer)?;
        
        // Use the appropriate deserialization based on format
        match self.format {
            SerializationFormat::Json | SerializationFormat::CompactJson => {
                if self.check_version {
                    let versioned: VersionedAst<T> = serde_json::from_slice(&buffer)?;
                    if let Some(ref expected) = self.expected_version {
                        if !version_compatible(&versioned.version, expected) {
                            return Err(DeserializationError::VersionMismatch {
                                expected: format!("{}.{}.{}", expected.major, expected.minor, expected.patch),
                                actual: format!("{}.{}.{}", versioned.version.major, versioned.version.minor, versioned.version.patch),
                            });
                        }
                    }
                    Ok(versioned.ast)
                } else {
                    Ok(serde_json::from_slice(&buffer)?)
                }
            }
            SerializationFormat::Binary | SerializationFormat::MessagePack => {
                Ok(bincode::deserialize(&buffer)?)
            }
        }
    }

    /// Deserialize only specific parts of the AST using a JSON path
    pub fn deserialize_partial<T>(&self, data: &[u8], path: &str) -> Result<T, DeserializationError>
    where
        T: for<'de> Deserialize<'de>,
    {
        // This is a simplified implementation
        // In production, would use a proper JSON path library
        match self.format {
            SerializationFormat::Json | SerializationFormat::CompactJson => {
                let value: serde_json::Value = serde_json::from_slice(data)?;
                let partial = extract_path(&value, path)
                    .ok_or_else(|| DeserializationError::InvalidFormat(format!("Path not found: {}", path)))?;
                Ok(serde_json::from_value(partial)?)
            }
            _ => Err(DeserializationError::InvalidFormat(
                "Partial deserialization only supported for JSON formats".to_string()
            ))
        }
    }
}

// Helper functions

fn compact_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            // Remove null values and compact field names
            let mut compacted = serde_json::Map::new();
            for (key, val) in map {
                if !val.is_null() {
                    let compact_key = compact_field_name(&key);
                    compacted.insert(compact_key, compact_json(val));
                }
            }
            serde_json::Value::Object(compacted)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(compact_json).collect())
        }
        other => other,
    }
}

fn compact_field_name(name: &str) -> String {
    // Don't compact field names - it breaks deserialization
    // CompactJson just removes null values for now
    name.to_string()
}

fn version_compatible(actual: &Version, expected: &Version) -> bool {
    // Major version must match, minor/patch can be higher
    actual.major == expected.major && 
    (actual.minor > expected.minor || 
     (actual.minor == expected.minor && actual.patch >= expected.patch))
}

fn extract_path(value: &serde_json::Value, path: &str) -> Option<serde_json::Value> {
    // Simplified JSON path extraction
    // In production, would use jsonpath or similar library
    
    if path == "/" {
        return Some(value.clone());
    }
    
    // Handle simple paths like "/items/*/kind/Function"
    if path.contains("*") {
        // Extract all matching elements
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        extract_wildcard_path(value, &parts)
    } else {
        // Direct path access
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        extract_direct_path(value, &parts)
    }
}

fn extract_direct_path(value: &serde_json::Value, parts: &[&str]) -> Option<serde_json::Value> {
    if parts.is_empty() {
        return Some(value.clone());
    }
    
    match value {
        serde_json::Value::Object(map) => {
            map.get(parts[0]).and_then(|v| extract_direct_path(v, &parts[1..]))
        }
        serde_json::Value::Array(arr) => {
            if let Ok(index) = parts[0].parse::<usize>() {
                arr.get(index).and_then(|v| extract_direct_path(v, &parts[1..]))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn extract_wildcard_path(value: &serde_json::Value, parts: &[&str]) -> Option<serde_json::Value> {
    if parts.is_empty() {
        return Some(value.clone());
    }
    
    if parts[0] == "*" {
        // Wildcard - collect from all array elements
        if let serde_json::Value::Array(arr) = value {
            let mut results = Vec::new();
            for item in arr {
                if let Some(extracted) = extract_wildcard_path(item, &parts[1..]) {
                    if let serde_json::Value::Array(mut sub_results) = extracted {
                        results.append(&mut sub_results);
                    } else {
                        results.push(extracted);
                    }
                }
            }
            return Some(serde_json::Value::Array(results));
        }
    }
    
    extract_direct_path(value, parts)
}

// Implement PartialEq for AST nodes to support testing
// This would normally be derived, but we need custom implementation for some fields

impl PartialEq for Program<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.items == other.items && self.span == other.span
    }
}

impl PartialEq for Item<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.span == other.span && self.id == other.id
    }
}

impl PartialEq for ItemKind<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ItemKind::Function(a), ItemKind::Function(b)) => a == b,
            (ItemKind::Struct(a), ItemKind::Struct(b)) => a == b,
            (ItemKind::Enum(a), ItemKind::Enum(b)) => a == b,
            _ => false,
        }
    }
}

impl PartialEq for Function<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name &&
        self.params == other.params &&
        self.return_type == other.return_type &&
        self.body == other.body &&
        self.visibility == other.visibility
    }
}

impl PartialEq for Block<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.statements == other.statements && self.span == other.span
    }
}

impl PartialEq for Stmt<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.span == other.span && self.id == other.id
    }
}

impl PartialEq for StmtKind<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (StmtKind::Expr(a), StmtKind::Expr(b)) => a == b,
            (StmtKind::Let(a), StmtKind::Let(b)) => a == b,
            (StmtKind::Item(a), StmtKind::Item(b)) => a == b,
            (StmtKind::Empty, StmtKind::Empty) => true,
            _ => false,
        }
    }
}

impl PartialEq for Expr<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.span == other.span && self.id == other.id
    }
}

impl PartialEq for ExprKind<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ExprKind::Literal(a), ExprKind::Literal(b)) => a == b,
            (ExprKind::Identifier(a), ExprKind::Identifier(b)) => a == b,
            (ExprKind::Binary { left: la, op: oa, right: ra }, 
             ExprKind::Binary { left: lb, op: ob, right: rb }) => {
                la == lb && oa == ob && ra == rb
            }
            (ExprKind::Return(a), ExprKind::Return(b)) => a == b,
            _ => false,
        }
    }
}

impl PartialEq for Literal<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.span == other.span
    }
}

impl PartialEq for LiteralKind<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (LiteralKind::Integer(a), LiteralKind::Integer(b)) => a == b,
            (LiteralKind::Float(a), LiteralKind::Float(b)) => (a - b).abs() < f64::EPSILON,
            (LiteralKind::String(a), LiteralKind::String(b)) => a == b,
            (LiteralKind::Char(a), LiteralKind::Char(b)) => a == b,
            (LiteralKind::Boolean(a), LiteralKind::Boolean(b)) => a == b,
            (LiteralKind::Unit, LiteralKind::Unit) => true,
            _ => false,
        }
    }
}

impl PartialEq for Type<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.span == other.span
    }
}

impl PartialEq for Path<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.segments == other.segments && self.span == other.span
    }
}

impl PartialEq for PathSegment<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.generic_args == other.generic_args
    }
}

impl PartialEq for Box<TypeKind<'_>> {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (TypeKind::Primitive(a), TypeKind::Primitive(b)) => a == b,
            (TypeKind::Infer, TypeKind::Infer) => true,
            (TypeKind::Named { path: pa, generic_args: ga }, 
             TypeKind::Named { path: pb, generic_args: gb }) => {
                pa == pb && ga == gb
            }
            (TypeKind::Tuple(a), TypeKind::Tuple(b)) => a == b,
            (TypeKind::Array { element_type: ea, size: sa },
             TypeKind::Array { element_type: eb, size: sb }) => {
                ea == eb && sa == sb
            }
            (TypeKind::Function { params: pa, return_type: ra },
             TypeKind::Function { params: pb, return_type: rb }) => {
                pa == pb && ra == rb
            }
            (TypeKind::Reference { inner: ia, is_mutable: ma },
             TypeKind::Reference { inner: ib, is_mutable: mb }) => {
                ia == ib && ma == mb
            }
            _ => false,
        }
    }
}

impl PartialEq for Parameter<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name &&
        self.ty == other.ty &&
        self.is_mutable == other.is_mutable &&
        self.span == other.span
    }
}

impl PartialEq for Let<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern &&
        self.ty == other.ty &&
        self.initializer == other.initializer &&
        self.is_mutable == other.is_mutable
    }
}

impl PartialEq for Pattern<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.span == other.span && self.id == other.id
    }
}

impl PartialEq for PatternKind<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PatternKind::Identifier(a), PatternKind::Identifier(b)) => a == b,
            (PatternKind::Wildcard, PatternKind::Wildcard) => true,
            (PatternKind::Literal(a), PatternKind::Literal(b)) => a == b,
            _ => false,
        }
    }
}

impl PartialEq for Struct<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name &&
        self.fields == other.fields &&
        self.visibility == other.visibility &&
        self.generic_params == other.generic_params
    }
}

impl PartialEq for Field<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name &&
        self.ty == other.ty &&
        self.visibility == other.visibility &&
        self.span == other.span
    }
}

impl PartialEq for Enum<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name &&
        self.variants == other.variants &&
        self.visibility == other.visibility &&
        self.generic_params == other.generic_params
    }
}

impl PartialEq for Variant<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name &&
        self.data == other.data &&
        self.span == other.span
    }
}

impl PartialEq for VariantData<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (VariantData::Unit, VariantData::Unit) => true,
            (VariantData::Tuple(a), VariantData::Tuple(b)) => a == b,
            (VariantData::Struct(a), VariantData::Struct(b)) => a == b,
            _ => false,
        }
    }
}

impl PartialEq for GenericParam<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name &&
        self.bounds == other.bounds &&
        self.default == other.default &&
        self.span == other.span
    }
}