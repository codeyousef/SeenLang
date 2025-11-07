use crate::{instruction::BinaryOp, value::IRValue};
use egg::{define_language, rewrite as rw, CostFunction, Id, RecExpr, Rewrite, Runner, Symbol};
use std::collections::HashMap;

define_language! {
    enum SeenLang {
        "+" = Add([Id; 2]),
        "-" = Sub([Id; 2]),
        "*" = Mul([Id; 2]),
        "/" = Div([Id; 2]),
        Num(i64),
        Var(Symbol),
    }
}

#[derive(Default)]
struct AstCost;

impl CostFunction<SeenLang> for AstCost {
    type Cost = usize;
    fn cost<C>(&mut self, enode: &SeenLang, mut costs: C) -> usize
    where
        C: FnMut(Id) -> usize,
    {
        match enode {
            SeenLang::Num(_) | SeenLang::Var(_) => 1,
            SeenLang::Add([a, b])
            | SeenLang::Sub([a, b])
            | SeenLang::Mul([a, b])
            | SeenLang::Div([a, b]) => 1 + costs(*a) + costs(*b),
        }
    }
}

/// Represents the result of attempting to simplify a binary expression.
pub enum SimplifiedExpr {
    Value(IRValue),
    Binary {
        op: BinaryOp,
        left: IRValue,
        right: IRValue,
    },
}

/// Try to simplify a binary operation using a small equality-saturation rewrite set.
pub fn try_simplify_binary(
    op: &BinaryOp,
    left: &IRValue,
    right: &IRValue,
) -> Option<SimplifiedExpr> {
    use BinaryOp::*;
    match op {
        Add | Subtract | Multiply | Divide => {}
        _ => return None,
    }

    let mut sym_map: HashMap<Symbol, IRValue> = HashMap::new();
    let mut expr = RecExpr::<SeenLang>::default();
    let left_id = push_leaf(left, &mut expr, &mut sym_map)?;
    let right_id = push_leaf(right, &mut expr, &mut sym_map)?;
    let root = match op {
        Add => expr.add(SeenLang::Add([left_id, right_id])),
        Subtract => expr.add(SeenLang::Sub([left_id, right_id])),
        Multiply => expr.add(SeenLang::Mul([left_id, right_id])),
        Divide => expr.add(SeenLang::Div([left_id, right_id])),
        _ => unreachable!(),
    };

    let rewrites = rewrite_rules();
    let runner = Runner::default()
        .with_iter_limit(5)
        .with_expr(&expr)
        .run(&rewrites);
    let root_id = runner.roots[0];
    let extractor = egg::Extractor::new(&runner.egraph, AstCost::default());
    let (_best_cost, best) = extractor.find_best(root_id);
    if best == expr {
        return None;
    }
    build_simplified(&best, &sym_map)
}

fn rewrite_rules() -> Vec<Rewrite<SeenLang, ()>> {
    vec![
        rw!("add-zero-left"; "(+ ?a 0)" => "?a"),
        rw!("add-zero-right"; "(+ 0 ?a)" => "?a"),
        rw!("sub-zero"; "(- ?a 0)" => "?a"),
        rw!("sub-self"; "(- ?a ?a)" => "0"),
        rw!("mul-zero-left"; "(* 0 ?a)" => "0"),
        rw!("mul-zero-right"; "(* ?a 0)" => "0"),
        rw!("mul-one-left"; "(* 1 ?a)" => "?a"),
        rw!("mul-one-right"; "(* ?a 1)" => "?a"),
        rw!("div-one"; "(/ ?a 1)" => "?a"),
        rw!("add-commute"; "(+ ?a ?b)" => "(+ ?b ?a)"),
        rw!("mul-commute"; "(* ?a ?b)" => "(* ?b ?a)"),
    ]
}

fn push_leaf(
    value: &IRValue,
    expr: &mut RecExpr<SeenLang>,
    map: &mut HashMap<Symbol, IRValue>,
) -> Option<Id> {
    match value {
        IRValue::Integer(i) => Some(expr.add(SeenLang::Num(*i))),
        IRValue::Boolean(b) => Some(expr.add(SeenLang::Num(if *b { 1 } else { 0 }))),
        IRValue::Register(r) => {
            let sym = Symbol::from(format!("r{}", r));
            map.insert(sym, value.clone());
            Some(expr.add(SeenLang::Var(sym)))
        }
        IRValue::Variable(name) => {
            let sym = Symbol::from(name.as_str());
            map.insert(sym, value.clone());
            Some(expr.add(SeenLang::Var(sym)))
        }
        _ => None,
    }
}

fn build_simplified(
    expr: &RecExpr<SeenLang>,
    sym_map: &HashMap<Symbol, IRValue>,
) -> Option<SimplifiedExpr> {
    let root = expr.as_ref().last()?;
    match root {
        SeenLang::Num(i) => Some(SimplifiedExpr::Value(IRValue::Integer(*i))),
        SeenLang::Var(sym) => sym_map.get(sym).cloned().map(SimplifiedExpr::Value),
        SeenLang::Add([a, b]) => build_binary(BinaryOp::Add, expr, *a, *b, sym_map),
        SeenLang::Sub([a, b]) => build_binary(BinaryOp::Subtract, expr, *a, *b, sym_map),
        SeenLang::Mul([a, b]) => build_binary(BinaryOp::Multiply, expr, *a, *b, sym_map),
        SeenLang::Div([a, b]) => build_binary(BinaryOp::Divide, expr, *a, *b, sym_map),
    }
}

fn build_binary(
    op: BinaryOp,
    expr: &RecExpr<SeenLang>,
    left_id: Id,
    right_id: Id,
    sym_map: &HashMap<Symbol, IRValue>,
) -> Option<SimplifiedExpr> {
    let left = extract_leaf(expr, left_id, sym_map)?;
    let right = extract_leaf(expr, right_id, sym_map)?;
    Some(SimplifiedExpr::Binary { op, left, right })
}

fn extract_leaf(
    expr: &RecExpr<SeenLang>,
    id: Id,
    sym_map: &HashMap<Symbol, IRValue>,
) -> Option<IRValue> {
    match &expr[id] {
        SeenLang::Num(i) => Some(IRValue::Integer(*i)),
        SeenLang::Var(sym) => sym_map.get(sym).cloned(),
        _ => None,
    }
}
