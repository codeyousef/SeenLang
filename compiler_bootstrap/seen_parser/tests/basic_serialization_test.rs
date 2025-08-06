//! Basic tests for AST serialization
//! Tests core serialization functionality that's implemented

use seen_parser::ast::*;
use seen_parser::serialization::{AstSerializer, AstDeserializer, SerializationFormat};
use seen_common::{Span, Spanned, Position};

// Helper function to create test spans
fn test_span(start: u32, end: u32) -> Span {
    Span::new(
        Position::new(1, start, start),
        Position::new(1, end, end),
        0
    )
}

#[test]
fn test_basic_json_serialization() {
    // Create a simple expression
    let expr = Expr {
        kind: Box::new(ExprKind::Literal(Literal {
            kind: LiteralKind::Integer(42),
            span: test_span(0, 2),
        })),
        span: test_span(0, 2),
        id: 1,
    };

    // Serialize to JSON
    let serializer = AstSerializer::new(SerializationFormat::Json);
    let json_data = serializer.serialize(&expr).unwrap();
    
    // Check that we got valid JSON
    let json_value: serde_json::Value = serde_json::from_slice(&json_data).unwrap();
    assert!(json_value.is_object());
    
    // Deserialize back
    let deserializer = AstDeserializer::new(SerializationFormat::Json);
    let deserialized: Expr = deserializer.deserialize(&json_data).unwrap();
    
    // Compare
    assert_eq!(expr, deserialized);
}

#[test]
fn test_binary_serialization() {
    // Create a type
    let ty = Type {
        kind: Box::new(TypeKind::Primitive(PrimitiveType::I32)),
        span: test_span(0, 3),
    };

    // Serialize to binary
    let serializer = AstSerializer::new(SerializationFormat::Binary);
    let binary_data = serializer.serialize(&ty).unwrap();
    
    // Should produce compact binary
    assert!(binary_data.len() > 0);
    
    // Deserialize back
    let deserializer = AstDeserializer::new(SerializationFormat::Binary);
    let deserialized: Type = deserializer.deserialize(&binary_data).unwrap();
    
    // Compare
    assert_eq!(ty, deserialized);
}

#[test]
fn test_function_serialization() {
    let func = Function {
        name: Spanned::new("test", test_span(0, 4)),
        params: vec![],
        return_type: None,
        body: Block {
            statements: vec![],
            span: test_span(5, 7),
        },
        visibility: Visibility::Public,
        attributes: vec![],
    };

    let serializer = AstSerializer::new(SerializationFormat::Json);
    let data = serializer.serialize(&func).unwrap();
    
    let deserializer = AstDeserializer::new(SerializationFormat::Json);
    let deserialized: Function = deserializer.deserialize(&data).unwrap();
    
    assert_eq!(func, deserialized);
}

#[test]
fn test_pretty_print_option() {
    let expr = Expr {
        kind: Box::new(ExprKind::Literal(Literal {
            kind: LiteralKind::Integer(42),
            span: test_span(0, 2),
        })),
        span: test_span(0, 2),
        id: 1,
    };

    // Normal JSON
    let normal_serializer = AstSerializer::new(SerializationFormat::Json);
    let normal_data = normal_serializer.serialize(&expr).unwrap();
    
    // Pretty printed JSON
    let pretty_serializer = AstSerializer::new(SerializationFormat::Json)
        .with_pretty_print(true);
    let pretty_data = pretty_serializer.serialize(&expr).unwrap();
    
    // Pretty version should be longer (has indentation)
    assert!(pretty_data.len() > normal_data.len());
    
    // Both should deserialize to the same thing
    let deserializer = AstDeserializer::new(SerializationFormat::Json);
    let normal_deserialized: Expr = deserializer.deserialize(&normal_data).unwrap();
    let pretty_deserialized: Expr = deserializer.deserialize(&pretty_data).unwrap();
    
    assert_eq!(normal_deserialized, pretty_deserialized);
    assert_eq!(expr, normal_deserialized);
}