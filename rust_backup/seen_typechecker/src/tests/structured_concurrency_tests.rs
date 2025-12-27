//! Tests for structured concurrency typing (spawn/scope/cancel).

use crate::{errors::TypeError, Type, TypeChecker};
use seen_lexer::Position;
use seen_parser::ast::Expression;

fn pos() -> Position {
    Position::start()
}

#[test]
fn spawn_outside_scope_is_error() {
    let mut checker = TypeChecker::new();
    let mut spawn_expr = Expression::Spawn {
        expr: Box::new(Expression::NullLiteral { pos: pos() }),
        detached: false,
        pos: pos(),
    };

    checker.check_expression(&mut spawn_expr);

    assert!(
        checker
            .result
            .errors
            .iter()
            .any(|err| matches!(err, TypeError::TaskRequiresScope { .. })),
        "Expected TaskRequiresScope error, got {:?}",
        checker.result.errors
    );
}

#[test]
fn spawn_inside_scope_is_allowed() {
    let mut checker = TypeChecker::new();
    let pos = pos();
    let mut scope_expr = Expression::Scope {
        body: Box::new(Expression::Block {
            expressions: vec![Expression::Spawn {
                expr: Box::new(Expression::IntegerLiteral { value: 1, pos }),
                detached: false,
                pos,
            }],
            pos,
        }),
        pos,
    };

    checker.check_expression(&mut scope_expr);

    assert!(
        checker
            .result
            .errors
            .iter()
            .all(|err| !matches!(err, TypeError::TaskRequiresScope { .. })),
        "Did not expect TaskRequiresScope error inside scope: {:?}",
        checker.result.errors
    );
}

#[test]
fn spawn_inside_jobs_scope_is_allowed() {
    let mut checker = TypeChecker::new();
    let pos = pos();
    let mut jobs_scope_expr = Expression::JobsScope {
        body: Box::new(Expression::Block {
            expressions: vec![Expression::Spawn {
                expr: Box::new(Expression::IntegerLiteral { value: 1, pos }),
                detached: false,
                pos,
            }],
            pos,
        }),
        pos,
    };

    checker.check_expression(&mut jobs_scope_expr);

    assert!(
        checker
            .result
            .errors
            .iter()
            .all(|err| !matches!(err, TypeError::TaskRequiresScope { .. })),
        "Did not expect TaskRequiresScope error inside jobs.scope: {:?}",
        checker.result.errors
    );
}

#[test]
fn cancel_requires_task() {
    let mut checker = TypeChecker::new();
    let mut cancel_expr = Expression::Cancel {
        task: Box::new(Expression::IntegerLiteral {
            value: 5,
            pos: pos(),
        }),
        pos: pos(),
    };

    checker.check_expression(&mut cancel_expr);

    assert!(
        checker
            .result
            .errors
            .iter()
            .any(|err| matches!(err, TypeError::CancelRequiresTask { .. })),
        "Expected CancelRequiresTask error, got {:?}",
        checker.result.errors
    );
}

#[test]
fn await_returns_task_payload_type() {
    let mut checker = TypeChecker::new();
    // Manually register variable with Task type
    checker
        .env
        .define_variable("task".to_string(), Type::Task(Box::new(Type::Int)));

    let mut await_expr = Expression::Await {
        expr: Box::new(Expression::Identifier {
            name: "task".to_string(),
            is_public: false,
            type_args: vec![],
            pos: pos(),
        }),
        pos: pos(),
    };

    let result_type = checker.check_expression(&mut await_expr);
    assert_eq!(result_type, Type::Int);
    assert!(
        checker
            .result
            .errors
            .iter()
            .all(|err| !matches!(err, TypeError::InvalidAwaitTarget { .. })),
        "Did not expect InvalidAwaitTarget error: {:?}",
        checker.result.errors
    );
}

#[test]
fn parallel_for_registers_binding() {
    let mut checker = TypeChecker::new();

    let mut array_expr = Expression::ArrayLiteral {
        elements: vec![Expression::IntegerLiteral {
            value: 1,
            pos: pos(),
        }],
        pos: pos(),
    };

    let mut body = Expression::Block {
        expressions: vec![Expression::Block {
            expressions: Vec::new(),
            pos: pos(),
        }],
        pos: pos(),
    };

    let mut expr = Expression::ParallelFor {
        binding: "item".to_string(),
        iterable: Box::new(array_expr),
        body: Box::new(body),
        pos: pos(),
    };

    let ty = checker.check_expression(&mut expr);
    assert_eq!(ty, Type::Unit);
    assert!(
        checker.result.errors.is_empty(),
        "Unexpected errors: {:?}",
        checker.result.errors
    );
}

#[test]
fn parallel_for_rejects_non_iterable() {
    let mut checker = TypeChecker::new();
    let mut expr = Expression::ParallelFor {
        binding: "item".to_string(),
        iterable: Box::new(Expression::IntegerLiteral {
            value: 42,
            pos: pos(),
        }),
        body: Box::new(Expression::Block {
            expressions: Vec::new(),
            pos: pos(),
        }),
        pos: pos(),
    };

    checker.check_expression(&mut expr);

    assert!(checker
                .result
                .errors
                .iter()
                .any(|err| matches!(err, TypeError::InvalidOperation { operation, .. } if operation == "parallel_for iterable")),
            "Expected invalid iterable error, got {:?}",
            checker.result.errors);
}
