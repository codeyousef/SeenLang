//! Tests for AST visitor pattern functionality

use seen_parser::ast::*;
use seen_parser::visitor::{Visitor, MutVisitor};
use seen_common::{Span, Spanned, Position};

// Helper function to create test spans
fn test_span(start: u32, end: u32) -> Span {
    Span::new(
        Position::new(1, start, start),
        Position::new(1, end, end),
        0
    )
}

#[derive(Default)]
struct NodeCounter {
    functions: usize,
    structs: usize,
    enums: usize,
    expressions: usize,
    types: usize,
}

impl<'a> Visitor<'a> for NodeCounter {
    fn visit_function(&mut self, func: &Function<'a>) {
        self.functions += 1;
        // Continue traversing the function body
        seen_parser::visitor::walk_function(self, func);
    }

    fn visit_struct(&mut self, s: &Struct<'a>) {
        self.structs += 1;
        seen_parser::visitor::walk_struct(self, s);
    }

    fn visit_enum(&mut self, e: &Enum<'a>) {
        self.enums += 1;
        seen_parser::visitor::walk_enum(self, e);
    }

    fn visit_expr(&mut self, expr: &Expr<'a>) {
        self.expressions += 1;
        seen_parser::visitor::walk_expr(self, expr);
    }

    fn visit_type(&mut self, ty: &Type<'a>) {
        self.types += 1;
        seen_parser::visitor::walk_type(self, ty);
    }
}

#[test]
fn test_visitor_traversal() {
    // Create a simple AST
    let program = Program {
        items: vec![
            Item {
                kind: ItemKind::Function(Function {
                    name: Spanned::new("main", test_span(0, 4)),
                    type_params: vec![],
                    params: vec![],
                    return_type: None,
                    body: Block {
                        statements: vec![
                            Stmt {
                                kind: StmtKind::Expr(Expr {
                                    kind: Box::new(ExprKind::Literal(Literal {
                                        kind: LiteralKind::Integer(42),
                                        span: test_span(10, 12),
                                    })),
                                    span: test_span(10, 12),
                                    id: 1,
                                }),
                                span: test_span(10, 13),
                                id: 2,
                            }
                        ],
                        span: test_span(8, 15),
                    },
                    visibility: Visibility::Public,
                    attributes: vec![],
                    is_inline: false,
                    is_suspend: false,
                    is_operator: false,
                    is_infix: false,
                    is_tailrec: false,
                }),
                span: test_span(0, 15),
                id: 0,
            },
            Item {
                kind: ItemKind::Struct(Struct {
                    name: Spanned::new("Point", test_span(20, 25)),
                    fields: vec![
                        Field {
                            name: Spanned::new("x", test_span(30, 31)),
                            ty: Type {
                                kind: Box::new(TypeKind::Primitive(PrimitiveType::I32)),
                                span: test_span(33, 36),
                            },
                            visibility: Visibility::Public,
                            span: test_span(30, 36),
                        },
                        Field {
                            name: Spanned::new("y", test_span(40, 41)),
                            ty: Type {
                                kind: Box::new(TypeKind::Primitive(PrimitiveType::I32)),
                                span: test_span(43, 46),
                            },
                            visibility: Visibility::Public,
                            span: test_span(40, 46),
                        },
                    ],
                    visibility: Visibility::Public,
                    companion_object: None,
                    generic_params: vec![],
                    attributes: vec![],
                }),
                span: test_span(20, 50),
                id: 3,
            },
        ],
        span: test_span(0, 50),
    };

    let mut counter = NodeCounter::default();
    counter.visit_program(&program);

    assert_eq!(counter.functions, 1);
    assert_eq!(counter.structs, 1);
    assert_eq!(counter.enums, 0);
    assert_eq!(counter.expressions, 1);
    assert_eq!(counter.types, 2); // Two field types
}

#[test]
fn test_mut_visitor_transformation() {
    struct LiteralRewriter;

    impl<'a> MutVisitor<'a> for LiteralRewriter {
        fn visit_expr(&mut self, expr: &mut Expr<'a>) {
            match &mut *expr.kind {
                ExprKind::Literal(ref mut lit) => {
                    match &mut lit.kind {
                        LiteralKind::Integer(ref mut n) => {
                            *n *= 2; // Double all integer literals
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            seen_parser::visitor::walk_expr_mut(self, expr);
        }
    }

    let mut expr = Expr {
        kind: Box::new(ExprKind::Binary {
            left: Box::new(Expr {
                kind: Box::new(ExprKind::Literal(Literal {
                    kind: LiteralKind::Integer(10),
                    span: test_span(0, 2),
                })),
                span: test_span(0, 2),
                id: 0,
            }),
            op: BinaryOp::Add,
            right: Box::new(Expr {
                kind: Box::new(ExprKind::Literal(Literal {
                    kind: LiteralKind::Integer(20),
                    span: test_span(5, 7),
                })),
                span: test_span(5, 7),
                id: 1,
            }),
        }),
        span: test_span(0, 7),
        id: 2,
    };

    let mut rewriter = LiteralRewriter;
    rewriter.visit_expr(&mut expr);

    // Check that literals were doubled
    match &*expr.kind {
        ExprKind::Binary { left, right, .. } => {
            match &*left.kind {
                ExprKind::Literal(lit) => {
                    match lit.kind {
                        LiteralKind::Integer(n) => assert_eq!(n, 20),
                        _ => panic!("Expected integer literal"),
                    }
                }
                _ => panic!("Expected literal"),
            }
            match &*right.kind {
                ExprKind::Literal(lit) => {
                    match lit.kind {
                        LiteralKind::Integer(n) => assert_eq!(n, 40),
                        _ => panic!("Expected integer literal"),
                    }
                }
                _ => panic!("Expected literal"),
            }
        }
        _ => panic!("Expected binary expression"),
    }
}

#[test]
fn test_visitor_pattern_discovery() {
    struct PatternFinder {
        found_patterns: Vec<String>,
    }

    impl<'a> Visitor<'a> for PatternFinder {
        fn visit_expr(&mut self, expr: &Expr<'a>) {
            // Look for specific patterns
            match &*expr.kind {
                ExprKind::Binary { left, op: BinaryOp::Add, right } => {
                    match (&*left.kind, &*right.kind) {
                        (ExprKind::Literal(lit1), ExprKind::Literal(lit2)) => {
                            match (&lit1.kind, &lit2.kind) {
                                (LiteralKind::Integer(_), LiteralKind::Integer(_)) => {
                                    self.found_patterns.push("constant_addition".to_string());
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            seen_parser::visitor::walk_expr(self, expr);
        }
    }

    let expr = Expr {
        kind: Box::new(ExprKind::Binary {
            left: Box::new(Expr {
                kind: Box::new(ExprKind::Literal(Literal {
                    kind: LiteralKind::Integer(5),
                    span: test_span(0, 1),
                })),
                span: test_span(0, 1),
                id: 0,
            }),
            op: BinaryOp::Add,
            right: Box::new(Expr {
                kind: Box::new(ExprKind::Literal(Literal {
                    kind: LiteralKind::Integer(3),
                    span: test_span(4, 5),
                })),
                span: test_span(4, 5),
                id: 1,
            }),
        }),
        span: test_span(0, 5),
        id: 2,
    };

    let mut finder = PatternFinder { found_patterns: vec![] };
    finder.visit_expr(&expr);

    assert_eq!(finder.found_patterns.len(), 1);
    assert_eq!(finder.found_patterns[0], "constant_addition");
}

#[test]
fn test_visitor_depth_tracking() {
    struct DepthTracker {
        max_depth: usize,
        current_depth: usize,
    }

    impl<'a> Visitor<'a> for DepthTracker {
        fn visit_block(&mut self, block: &Block<'a>) {
            self.current_depth += 1;
            self.max_depth = self.max_depth.max(self.current_depth);
            seen_parser::visitor::walk_block(self, block);
            self.current_depth -= 1;
        }
    }

    // Create nested blocks
    let program = Program {
        items: vec![
            Item {
                kind: ItemKind::Function(Function {
                    name: Spanned::new("nested", test_span(0, 6)),
                    type_params: vec![],
                    is_inline: false,
                    is_suspend: false,
                    is_operator: false,
                    is_infix: false,
                    is_tailrec: false,
                    params: vec![],
                    return_type: None,
                    body: Block {
                        statements: vec![
                            Stmt {
                                kind: StmtKind::Expr(Expr {
                                    kind: Box::new(ExprKind::Block(Block {
                                        statements: vec![
                                            Stmt {
                                                kind: StmtKind::Expr(Expr {
                                                    kind: Box::new(ExprKind::Block(Block {
                                                        statements: vec![],
                                                        span: test_span(20, 22),
                                                    })),
                                                    span: test_span(20, 22),
                                                    id: 3,
                                                }),
                                                span: test_span(20, 22),
                                                id: 4,
                                            }
                                        ],
                                        span: test_span(15, 25),
                                    })),
                                    span: test_span(15, 25),
                                    id: 1,
                                }),
                                span: test_span(15, 25),
                                id: 2,
                            }
                        ],
                        span: test_span(10, 30),
                    },
                    visibility: Visibility::Public,
                    attributes: vec![],
                }),
                span: test_span(0, 30),
                id: 0,
            }
        ],
        span: test_span(0, 30),
    };

    let mut tracker = DepthTracker { max_depth: 0, current_depth: 0 };
    tracker.visit_program(&program);

    assert_eq!(tracker.max_depth, 3); // Function body + 2 nested blocks
}

#[test]
fn test_visitor_collection() {
    struct IdentifierCollector<'a> {
        identifiers: Vec<&'a str>,
    }

    impl<'a> Visitor<'a> for IdentifierCollector<'a> {
        fn visit_expr(&mut self, expr: &Expr<'a>) {
            match &*expr.kind {
                ExprKind::Identifier(ident) => {
                    self.identifiers.push(ident.value);
                }
                _ => {}
            }
            seen_parser::visitor::walk_expr(self, expr);
        }
    }

    let expr = Expr {
        kind: Box::new(ExprKind::Binary {
            left: Box::new(Expr {
                kind: Box::new(ExprKind::Identifier(Spanned::new("x", test_span(0, 1)))),
                span: test_span(0, 1),
                id: 0,
            }),
            op: BinaryOp::Add,
            right: Box::new(Expr {
                kind: Box::new(ExprKind::Identifier(Spanned::new("y", test_span(4, 5)))),
                span: test_span(4, 5),
                id: 1,
            }),
        }),
        span: test_span(0, 5),
        id: 2,
    };

    let mut collector = IdentifierCollector { identifiers: vec![] };
    collector.visit_expr(&expr);

    assert_eq!(collector.identifiers.len(), 2);
    assert_eq!(collector.identifiers[0], "x");
    assert_eq!(collector.identifiers[1], "y");
}