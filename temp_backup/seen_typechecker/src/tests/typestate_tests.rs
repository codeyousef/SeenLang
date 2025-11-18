//! Tests for typestate generics and sealed traits

use seen_lexer::Position;
use seen_parser::ast::*;
use seen_typechecker::{Type, TypeChecker};
use std::collections::HashMap;

fn pos() -> Position {
    Position::start()
}

fn make_struct_definition(name: &str) -> Expression {
    Expression::StructDefinition {
        name: name.to_string(),
        generics: Vec::new(),
        fields: Vec::new(),
        doc_comment: None,
        pos: pos(),
    }
}

#[test]
fn typestate_structs_enforce_generic_arguments() {
    let mut checker = TypeChecker::new();

    // Base state structs
    checker.check_expression(&make_struct_definition("Recording"));
    checker.check_expression(&make_struct_definition("Executable"));

    // CommandBuffer<S> with phantom field
    let phantom_field = StructField {
        name: "phantom".to_string(),
        field_type: Type {
            name: "Phantom".to_string(),
            is_nullable: false,
            generics: vec![Type {
                name: "S".to_string(),
                is_nullable: false,
                generics: Vec::new(),
            }],
        },
        is_public: false,
        annotations: Vec::new(),
    };

    let command_buffer_def = Expression::StructDefinition {
        name: "CommandBuffer".to_string(),
        generics: vec!["S".to_string()],
        fields: vec![phantom_field],
        doc_comment: None,
        pos: pos(),
    };

    checker.check_expression(&command_buffer_def);
    assert!(checker.result.errors.is_empty(), "Struct definitions should succeed: {:?}", checker.result.errors);

    // Seed environment with a variable of type CommandBuffer<Executable>
    checker
        .env
        .define_variable(
            "source_cb".to_string(),
            Type::Struct {
                name: "CommandBuffer".to_string(),
                fields: HashMap::new(),
                generics: vec![Type::Struct {
                    name: "Executable".to_string(),
                    fields: HashMap::new(),
                    generics: Vec::new(),
                }],
            },
        );

    // let bad: CommandBuffer<Recording> = source_cb;
    let bad_let = Expression::Let {
        name: "bad".to_string(),
        type_annotation: Some(Type {
            name: "CommandBuffer".to_string(),
            is_nullable: false,
            generics: vec![Type {
                name: "Recording".to_string(),
                is_nullable: false,
                generics: Vec::new(),
            }],
        }),
        value: Box::new(Expression::Identifier {
            name: "source_cb".to_string(),
            is_public: false,
            pos: pos(),
        }),
        is_mutable: false,
        delegation: None,
        pos: pos(),
    };

    checker.check_expression(&bad_let);
    assert!(checker
                .result
                .errors
                .iter()
                .any(|err| matches!(err, seen_typechecker::TypeError::TypeMismatch { .. })),
            "Expected type mismatch when assigning different typestate, got {:?}",
            checker.result.errors);

    checker.result.errors.clear();

    // let good: CommandBuffer<Executable> = source_cb;
    let good_let = Expression::Let {
        name: "good".to_string(),
        type_annotation: Some(Type {
            name: "CommandBuffer".to_string(),
            is_nullable: false,
            generics: vec![Type {
                name: "Executable".to_string(),
                is_nullable: false,
                generics: Vec::new(),
            }],
        }),
        value: Box::new(Expression::Identifier {
            name: "source_cb".to_string(),
            is_public: false,
            pos: pos(),
        }),
        is_mutable: false,
        delegation: None,
        pos: pos(),
    };

    checker.check_expression(&good_let);
    assert!(checker.result.errors.is_empty(), "Matching typestate assignment should succeed: {:?}", checker.result.errors);
}

#[test]
fn sealed_interface_rejects_extensions() {
    let mut checker = TypeChecker::new();

    let trait_def = Expression::Interface {
        name: "CmdState".to_string(),
        generics: Vec::new(),
        methods: Vec::new(),
        is_sealed: true,
        pos: pos(),
    };

    checker.check_expression(&trait_def);
    assert!(checker.result.errors.is_empty());

    let extension = Expression::Extension {
        target_type: Type {
            name: "CmdState".to_string(),
            is_nullable: false,
            generics: Vec::new(),
        },
        methods: Vec::new(),
        pos: pos(),
    };

    checker.check_expression(&extension);
    assert!(checker
                .result
                .errors
                .iter()
                .any(|err| matches!(err, seen_typechecker::TypeError::SealedTypeExtension { .. })),
            "Expected sealed type extension error, got {:?}",
            checker.result.errors);
}
