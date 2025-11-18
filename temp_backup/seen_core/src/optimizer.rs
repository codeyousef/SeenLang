use egg::{define_language, rewrite, CostFunction, Id, RecExpr, Rewrite, Runner, SymbolLang};
use seen_parser::{BinaryOperator, Expression, Position, Program};

pub fn optimize_program(program: &mut Program) {
    for expr in &mut program.expressions {
        optimize_expression(expr);
    }
}

fn optimize_expression(expr: &mut Expression) {
    match expr {
        Expression::BinaryOp { left, right, .. } => {
            optimize_expression(left);
            optimize_expression(right);
            if let Some(simple) = SimpleExpr::from_expression(expr) {
                if let Some(optimized) = run_equality_saturation(&simple) {
                    *expr = optimized.into_expression(expr.position().clone());
                }
            }
        }
        Expression::UnaryOp { operand, .. } => optimize_expression(operand),
        Expression::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            optimize_expression(condition);
            optimize_expression(then_branch);
            if let Some(else_branch) = else_branch {
                optimize_expression(else_branch);
            }
        }
        Expression::Block { expressions, .. } => {
            for expr in expressions {
                optimize_expression(expr);
            }
        }
        Expression::Match { expr: match_expr, arms, .. } => {
            optimize_expression(match_expr);
            for arm in arms {
                optimize_expression(&mut arm.expression);
            }
        }
        Expression::Call { callee, args, .. } => {
            optimize_expression(callee);
            for arg in args {
                optimize_expression(arg);
            }
        }
        Expression::Lambda { body, .. }
        | Expression::Function { body, .. }
        | Expression::Scope { body, .. }
        | Expression::JobsScope { body, .. }
        | Expression::Comptime { body, .. }
        | Expression::Arena { body, .. }
        | Expression::Region { body, .. }
        | Expression::ContractedFunction { function: body, .. } => optimize_expression(body),
        Expression::Let { value, .. }
        | Expression::Const { value, .. }
        | Expression::Assignment { value, .. }
        | Expression::Return { value: Some(value), .. }
        | Expression::While { condition: value, .. }
        | Expression::DoWhile { condition: value, .. }
        | Expression::Loop { body: value, .. }
        | Expression::Catch { body: value, .. }
        | Expression::Await { expr: value, .. }
        | Expression::Spawn { expr: value, .. }
        | Expression::Cancel { task: value, .. }
        | Expression::ParallelFor { iterable: value, body, .. }
        | Expression::Move { operand: value, .. }
        | Expression::Borrow { operand: value, .. }
        | Expression::Handle { body: value, .. }
        | Expression::Effect { body: value, .. } => {
            optimize_expression(value);
            if let Expression::ParallelFor { body, .. } = expr {
                optimize_expression(body);
            }
        }
        Expression::StructLiteral { fields, .. } => {
            for (_, expr) in fields.iter_mut() {
                optimize_expression(expr);
            }
        }
        Expression::ArrayLiteral { elements, .. } => {
            for element in elements {
                optimize_expression(element);
            }
        }
        Expression::IndexAccess { object, index, .. } => {
            optimize_expression(object);
            optimize_expression(index);
        }
        Expression::MemberAccess { object, .. } => optimize_expression(object),
        Expression::CallExtension { target, args, .. } => {
            optimize_expression(target);
            for arg in args {
                optimize_expression(arg);
            }
        }
        Expression::Request { message, source, .. } => {
            optimize_expression(message);
            optimize_expression(source);
        }
        Expression::Select { cases, .. } => {
            for case in cases {
                optimize_expression(&mut case.channel);
                optimize_expression(&mut case.handler);
            }
        }
        Expression::Send { target, message, .. } => {
            optimize_expression(target);
            optimize_expression(message);
        }
        _ => {}
    }
}

fn run_equality_saturation(expr: &SimpleExpr) -> Option<SimpleExpr> {
    let mut recexpr = RecExpr::<ArithLang>::default();
    let root = expr.add_to_recexpr(&mut recexpr);
    let runner = Runner::default()
        .with_expr(&recexpr)
        .run(&rewrites());
    let extractor = egg::Extractor::new(&runner.egraph, AstSize);
    let (_, best) = extractor.find_best(root);
    SimpleExpr::from_recexpr(&best)
}

fn rewrites() -> Vec<Rewrite<ArithLang, ()>> {
    vec![
        rewrite!("add-zero-right"; "(+ ?a (Num 0))" => "?a"),
        rewrite!("add-zero-left"; "(+ (Num 0) ?a)" => "?a"),
        rewrite!("sub-zero"; "(- ?a (Num 0))" => "?a"),
        rewrite!("mul-one-right"; "(* ?a (Num 1))" => "?a"),
        rewrite!("mul-one-left"; "(* (Num 1) ?a)" => "?a"),
        rewrite!("mul-zero-right"; "(* ?a (Num 0))" => "(Num 0)"),
        rewrite!("mul-zero-left"; "(* (Num 0) ?a)" => "(Num 0)"),
        rewrite!("add-comm"; "(+ ?a ?b)" => "(+ ?b ?a)"),
        rewrite!("mul-comm"; "(* ?a ?b)" => "(* ?b ?a)"),
        rewrite!("add-assoc"; "(+ ?a (+ ?b ?c))" => "(+ (+ ?a ?b) ?c)"),
        rewrite!("mul-assoc"; "(* ?a (* ?b ?c))" => "(* (* ?a ?b) ?c)"),
    ]
}

#[derive(Debug, Clone)]
enum SimpleExpr {
    Const(i64),
    Var { name: String, is_public: bool },
    Add(Box<SimpleExpr>, Box<SimpleExpr>),
    Sub(Box<SimpleExpr>, Box<SimpleExpr>),
    Mul(Box<SimpleExpr>, Box<SimpleExpr>),
}

impl SimpleExpr {
    fn from_expression(expr: &Expression) -> Option<Self> {
        match expr {
            Expression::IntegerLiteral { value, .. } => Some(SimpleExpr::Const(*value)),
            Expression::Identifier { name, is_public, .. } => Some(SimpleExpr::Var {
                name: name.clone(),
                is_public: *is_public,
            }),
            Expression::BinaryOp { left, right, op, .. } => {
                let left = SimpleExpr::from_expression(left)?;
                let right = SimpleExpr::from_expression(right)?;
                match op {
                    BinaryOperator::Add => Some(SimpleExpr::Add(Box::new(left), Box::new(right))),
                    BinaryOperator::Subtract => {
                        Some(SimpleExpr::Sub(Box::new(left), Box::new(right)))
                    }
                    BinaryOperator::Multiply => {
                        Some(SimpleExpr::Mul(Box::new(left), Box::new(right)))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn into_expression(self, pos: Position) -> Expression {
        match self {
            SimpleExpr::Const(value) => Expression::IntegerLiteral { value, pos },
            SimpleExpr::Var { name, is_public } => Expression::Identifier {
                name,
                is_public,
                pos,
            },
            SimpleExpr::Add(left, right) => Expression::BinaryOp {
                left: Box::new(left.into_expression(pos.clone())),
                op: BinaryOperator::Add,
                right: Box::new(right.into_expression(pos.clone())),
                pos,
            },
            SimpleExpr::Sub(left, right) => Expression::BinaryOp {
                left: Box::new(left.into_expression(pos.clone())),
                op: BinaryOperator::Subtract,
                right: Box::new(right.into_expression(pos.clone())),
                pos,
            },
            SimpleExpr::Mul(left, right) => Expression::BinaryOp {
                left: Box::new(left.into_expression(pos.clone())),
                op: BinaryOperator::Multiply,
                right: Box::new(right.into_expression(pos.clone())),
                pos,
            },
        }
    }

    fn add_to_recexpr(&self, recexpr: &mut RecExpr<ArithLang>) -> Id {
        match self {
            SimpleExpr::Const(v) => recexpr.add(ArithLang::Num(*v)),
            SimpleExpr::Var { name, .. } => recexpr.add(ArithLang::Symbol(name.into())),
            SimpleExpr::Add(left, right) => {
                let l = left.add_to_recexpr(recexpr);
                let r = right.add_to_recexpr(recexpr);
                recexpr.add(ArithLang::Add([l, r]))
            }
            SimpleExpr::Sub(left, right) => {
                let l = left.add_to_recexpr(recexpr);
                let r = right.add_to_recexpr(recexpr);
                recexpr.add(ArithLang::Sub([l, r]))
            }
            SimpleExpr::Mul(left, right) => {
                let l = left.add_to_recexpr(recexpr);
                let r = right.add_to_recexpr(recexpr);
                recexpr.add(ArithLang::Mul([l, r]))
            }
        }
    }

    fn from_recexpr(expr: &RecExpr<ArithLang>) -> Option<Self> {
        let root = expr.as_ref().len().checked_sub(1)?;
        Some(Self::from_recexpr_inner(expr, Id::from(root)))
    }

    fn from_recexpr_inner(expr: &RecExpr<ArithLang>, id: Id) -> Self {
        match &expr[id] {
            ArithLang::Num(v) => SimpleExpr::Const(*v),
            ArithLang::Symbol(sym) => SimpleExpr::Var {
                name: sym.to_string(),
                is_public: false,
            },
            ArithLang::Add([a, b]) => SimpleExpr::Add(
                Box::new(Self::from_recexpr_inner(expr, *a)),
                Box::new(Self::from_recexpr_inner(expr, *b)),
            ),
            ArithLang::Sub([a, b]) => SimpleExpr::Sub(
                Box::new(Self::from_recexpr_inner(expr, *a)),
                Box::new(Self::from_recexpr_inner(expr, *b)),
            ),
            ArithLang::Mul([a, b]) => SimpleExpr::Mul(
                Box::new(Self::from_recexpr_inner(expr, *a)),
                Box::new(Self::from_recexpr_inner(expr, *b)),
            ),
        }
    }
}

define_language! {
    enum ArithLang {
        "+" = Add([Id; 2]),
        "-" = Sub([Id; 2]),
        "*" = Mul([Id; 2]),
        Num(i64),
        Symbol(SymbolLang),
    }
}

struct AstSize;

impl CostFunction<ArithLang> for AstSize {
    type Cost = usize;

    fn cost<C>(&mut self, enode: &ArithLang, mut costs: C) -> Self::Cost
    where
        C: FnMut(Id) -> Self::Cost,
    {
        1 + enode.children().iter().map(|&id| costs(id)).sum::<usize>()
    }
}

trait PositionedExpression {
    fn position(&self) -> &Position;
}

impl PositionedExpression for Expression {
    fn position(&self) -> &Position {
        match self {
            Expression::IntegerLiteral { pos, .. }
            | Expression::FloatLiteral { pos, .. }
            | Expression::StringLiteral { pos, .. }
            | Expression::CharLiteral { pos, .. }
            | Expression::InterpolatedString { pos, .. }
            | Expression::BooleanLiteral { pos, .. }
            | Expression::NullLiteral { pos }
            | Expression::Identifier { pos, .. }
            | Expression::BinaryOp { pos, .. }
            | Expression::UnaryOp { pos, .. }
            | Expression::If { pos, .. }
            | Expression::Match { pos, .. }
            | Expression::Block { pos, .. }
            | Expression::Let { pos, .. }
            | Expression::Const { pos, .. }
            | Expression::Assignment { pos, .. }
            | Expression::Function { pos, .. }
            | Expression::Lambda { pos, .. }
            | Expression::Call { pos, .. }
            | Expression::Return { pos, .. }
            | Expression::Loop { pos, .. }
            | Expression::While { pos, .. }
            | Expression::DoWhile { pos, .. }
            | Expression::Select { pos, .. }
            | Expression::Send { pos, .. }
            | Expression::Receive { pos, .. }
            | Expression::Scope { pos, .. }
            | Expression::JobsScope { pos, .. }
            | Expression::Cancel { pos, .. }
            | Expression::ParallelFor { pos, .. }
            | Expression::ArrayLiteral { pos, .. }
            | Expression::StructLiteral { pos, .. }
            | Expression::IndexAccess { pos, .. }
            | Expression::MemberAccess { pos, .. }
            | Expression::Move { pos, .. }
            | Expression::Borrow { pos, .. }
            | Expression::Comptime { pos, .. }
            | Expression::Arena { pos, .. }
            | Expression::Region { pos, .. }
            | Expression::Handle { pos, .. }
            | Expression::Effect { pos, .. }
            | Expression::ContractedFunction { pos, .. }
            | Expression::Extension { pos, .. }
            | Expression::Interface { pos, .. }
            | Expression::Actor { pos, .. }
            | Expression::Request { pos, .. }
            | Expression::Macro { pos, .. }
            | Expression::CallExtension { pos, .. }
            | Expression::Try { pos, .. }
            | Expression::Throw { pos, .. }
            | Expression::Await { pos, .. }
            | Expression::Annotation { pos, .. }
            | Expression::AsyncBlock { pos, .. }
            | Expression::EffectHandler { pos, .. }
            | Expression::Catch { pos, .. }
            | Expression::Unknown { pos, .. } => pos,
        }
    }
}
