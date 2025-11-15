use seen_lexer::Position;
use seen_parser::ast::*;
use seen_typechecker::TypeChecker;

fn pos() -> Position {
    Position::new(1, 1, 0)
}

fn ast_type(name: &str) -> Type {
    Type {
        name: name.to_string(),
        is_nullable: false,
        generics: Vec::new(),
    }
}

#[test]
fn method_resolution_handles_instance_calls() {
    let p = pos();

    let struct_expr = Expression::StructDefinition {
        name: "Point".to_string(),
        generics: Vec::new(),
        fields: vec![StructField {
            name: "x".to_string(),
            field_type: ast_type("Int"),
            is_public: false,
            annotations: Vec::new(),
        }],
        doc_comment: None,
        pos: p,
    };

    let method_expr = Expression::Function {
        name: "Point::length".to_string(),
        generics: Vec::new(),
        params: vec![Parameter {
            name: "this".to_string(),
            type_annotation: Some(ast_type("Point")),
            default_value: None,
            memory_modifier: None,
        }],
        return_type: Some(ast_type("Int")),
        body: Box::new(Expression::IntegerLiteral { value: 1, pos: p }),
        is_async: false,
        receiver: None,
        uses_effects: Vec::new(),
        is_pure: true,
        is_external: false,
        is_public: true,
        doc_comment: None,
        pos: p,
    };

    let point_literal = Expression::StructLiteral {
        name: "Point".to_string(),
        fields: vec![(
            "x".to_string(),
            Expression::IntegerLiteral { value: 10, pos: p },
        )],
        pos: p,
    };

    let call_expr = Expression::Call {
        callee: Box::new(Expression::MemberAccess {
            object: Box::new(point_literal),
            member: "length".to_string(),
            is_safe: false,
            pos: p,
        }),
        args: Vec::new(),
        pos: p,
    };

    let program = Program {
        expressions: vec![struct_expr, method_expr, call_expr],
    };

    let mut checker = TypeChecker::new();
    let result = checker.check_program(&program);
    assert!(
        !result.has_errors(),
        "method call should type-check without errors: {:?}",
        result.get_errors()
    );
}

#[test]
fn enum_variant_access_round_trips_through_typechecker() {
    let p = pos();
    let enum_expr = Expression::EnumDefinition {
        name: "Target".to_string(),
        generics: Vec::new(),
        variants: vec![
            EnumVariant {
                name: "Linux".to_string(),
                fields: None,
            },
            EnumVariant {
                name: "Windows".to_string(),
                fields: None,
            },
        ],
        doc_comment: None,
        pos: p,
    };

    let variant_expr = Expression::MemberAccess {
        object: Box::new(Expression::Identifier {
            name: "Target".to_string(),
            is_public: false,
            pos: p,
        }),
        member: "Linux".to_string(),
        is_safe: false,
        pos: p,
    };

    let program = Program {
        expressions: vec![enum_expr, variant_expr],
    };

    let mut checker = TypeChecker::new();
    let result = checker.check_program(&program);
    assert!(
        !result.has_errors(),
        "enum variant lookups should be resolved: {:?}",
        result.get_errors()
    );
}

#[test]
fn operator_typing_covers_numeric_and_string_cases() {
    let p = pos();
    let ge_expr = Expression::BinaryOp {
        left: Box::new(Expression::IntegerLiteral { value: 10, pos: p }),
        op: BinaryOperator::GreaterEqual,
        right: Box::new(Expression::IntegerLiteral { value: 2, pos: p }),
        pos: p,
    };

    let concat_expr = Expression::BinaryOp {
        left: Box::new(Expression::StringLiteral {
            value: "hello".to_string(),
            pos: p,
        }),
        op: BinaryOperator::Add,
        right: Box::new(Expression::IntegerLiteral { value: 5, pos: p }),
        pos: p,
    };

    let program = Program {
        expressions: vec![ge_expr, concat_expr],
    };

    let mut checker = TypeChecker::new();
    let result = checker.check_program(&program);
    assert!(
        !result.has_errors(),
        ">= and string concatenation typing regressions detected: {:?}",
        result.get_errors()
    );
}
