use seen_lexer::Position;
use seen_parser::ast::*;
use seen_typechecker::{TypeChecker, TypeError};

fn pos() -> Position {
    Position::new(1, 1, 0)
}

fn simple_type(name: &str) -> Type {
    Type {
        name: name.to_string(),
        is_nullable: false,
        generics: Vec::new(),
    }
}

#[test]
fn class_definition_registers_type_and_methods() {
    let class_pos = pos();
    let class_expr = Expression::ClassDefinition {
        name: "Vec".to_string(),
        generics: vec!["T".to_string()],
        superclass: None,
        fields: vec![
            ClassField {
                name: "data".to_string(),
                field_type: Type {
                    name: "Array".to_string(),
                    is_nullable: false,
                    generics: vec![simple_type("T")],
                },
                is_public: false,
                is_mutable: true,
                default_value: None,
                annotations: Vec::new(),
            },
            ClassField {
                name: "length".to_string(),
                field_type: simple_type("Int"),
                is_public: false,
                is_mutable: true,
                default_value: Some(Expression::IntegerLiteral {
                    value: 0,
                    pos: class_pos,
                }),
                annotations: Vec::new(),
            },
        ],
        methods: vec![Method {
            name: "len".to_string(),
            parameters: Vec::new(),
            return_type: Some(simple_type("Int")),
            body: Expression::Identifier {
                name: "length".to_string(),
                is_public: false,
                type_args: vec![],
                pos: class_pos,
            },
            is_public: true,
            is_static: false,
            receiver: None,
            annotations: Vec::new(),
            pos: class_pos,
        }],
        is_sealed: false,
        doc_comment: None,
        pos: class_pos,
    };

    let program = Program {
        expressions: vec![class_expr],
    };

    let mut checker = TypeChecker::new();
    let result = checker.check_program(&program);

    assert!(
        !result.has_errors(),
        "expected no errors, got {:?}",
        result.get_errors()
    );

    let vec_type = checker
        .env
        .get_type("Vec")
        .cloned()
        .expect("Vec type registered");
    match vec_type {
        seen_typechecker::Type::Struct { generics, .. } => {
            assert_eq!(generics.len(), 1);
        }
        other => panic!("expected struct type, got {:?}", other),
    }
}

#[test]
fn constructor_and_abort_calls_typecheck() {
    let program = Program {
        expressions: vec![
            Expression::Call {
                callee: Box::new(Expression::Identifier {
                    name: "Array".to_string(),
                    is_public: false,
                    type_args: vec![],
                    pos: pos(),
                }),
                args: vec![],
                pos: pos(),
            },
            Expression::Call {
                callee: Box::new(Expression::Identifier {
                    name: "abort".to_string(),
                    is_public: false,
                    type_args: vec![],
                    pos: pos(),
                }),
                args: vec![Expression::StringLiteral {
                    value: "boom".to_string(),
                    pos: pos(),
                }],
                pos: pos(),
            },
        ],
    };

    let mut checker = TypeChecker::new();
    let result = checker.check_program(&program);
    assert!(
        !result.has_errors(),
        "constructor/builtin calls should not error: {:?}",
        result.get_errors().iter().collect::<Vec<&TypeError>>()
    );
}

#[test]
fn class_methods_can_reference_fields() {
    let class_pos = pos();
    let method_body = Expression::BinaryOp {
        left: Box::new(Expression::Identifier {
            name: "length".to_string(),
            is_public: false,
            type_args: vec![],
            pos: class_pos,
        }),
        op: BinaryOperator::Add,
        right: Box::new(Expression::IntegerLiteral {
            value: 1,
            pos: class_pos,
        }),
        pos: class_pos,
    };

    let class_expr = Expression::ClassDefinition {
        name: "Counter".to_string(),
        generics: Vec::new(),
        superclass: None,
        fields: vec![ClassField {
            name: "length".to_string(),
            field_type: simple_type("Int"),
            is_public: false,
            is_mutable: true,
            default_value: None,
            annotations: Vec::new(),
        }],
        methods: vec![Method {
            name: "next".to_string(),
            parameters: Vec::new(),
            return_type: Some(simple_type("Int")),
            body: method_body,
            is_public: true,
            is_static: false,
            receiver: None,
            annotations: Vec::new(),
            pos: class_pos,
        }],
        is_sealed: false,
        doc_comment: None,
        pos: class_pos,
    };

    let program = Program {
        expressions: vec![class_expr],
    };

    let mut checker = TypeChecker::new();
    let result = checker.check_program(&program);
    assert!(
        !result.has_errors(),
        "expected method to see fields, got {:?}",
        result.get_errors()
    );
}
