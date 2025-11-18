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
    let spawn_expr = Expression::Spawn {
        expr: Box::new(Expression::NullLiteral { pos: pos() }),
        detached: false,
        pos: pos(),
    };

    checker.check_expression(&spawn_expr);

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
    let scope_expr = Expression::Scope {
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

    checker.check_expression(&scope_expr);

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
    let jobs_scope_expr = Expression::JobsScope {
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

    checker.check_expression(&jobs_scope_expr);

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
    let cancel_expr = Expression::Cancel {
        task: Box::new(Expression::IntegerLiteral {
            value: 5,
            pos: pos(),
        }),
        pos: pos(),
    };

    checker.check_expression(&cancel_expr);

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

    let await_expr = Expression::Await {
        expr: Box::new(Expression::Identifier {
            name: "task".to_string(),
            is_public: false,
            pos: pos(),
        }),
        pos: pos(),
    };

    let result_type = checker.check_expression(&await_expr);
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

    let array_expr = Expression::ArrayLiteral {
        elements: vec![Expression::IntegerLiteral {
            value: 1,
            pos: pos(),
        }],
        pos: pos(),
    };

    let body = Expression::Block {
        expressions: vec![Expression::Block {
            expressions: Vec::new(),
            pos: pos(),
        }],
        pos: pos(),
    };

    let expr = Expression::ParallelFor {
        binding: "item".to_string(),
        iterable: Box::new(array_expr),
        body: Box::new(body),
        pos: pos(),
    };

    let ty = checker.check_expression(&expr);
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
    let expr = Expression::ParallelFor {
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

    checker.check_expression(&expr);

    assert!(checker
                .result
                .errors
                .iter()
                .any(|err| matches!(err, TypeError::InvalidOperation { operation, .. } if operation == "parallel_for iterable")),
            "Expected invalid iterable error, got {:?}",
            checker.result.errors);
}
