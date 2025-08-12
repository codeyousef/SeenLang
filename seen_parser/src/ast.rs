//! Abstract Syntax Tree definitions
//! 
//! Everything in Seen is an expression that returns a value.
//! There are NO statements - even declarations return values.

use serde::{Deserialize, Serialize};
use seen_lexer::Position;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub expressions: Vec<Expression>,
}

/// Core expression type - everything in Seen is an expression
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expression {
    // Literals
    IntegerLiteral { value: i64, pos: Position },
    FloatLiteral { value: f64, pos: Position },
    StringLiteral { value: String, pos: Position },
    InterpolatedString { parts: Vec<InterpolationPart>, pos: Position },
    BooleanLiteral { value: bool, pos: Position },
    NullLiteral { pos: Position },
    
    // Identifiers (capitalization determines visibility)
    Identifier { name: String, is_public: bool, pos: Position },
    
    // Binary operations
    BinaryOp {
        left: Box<Expression>,
        op: BinaryOperator,
        right: Box<Expression>,
        pos: Position,
    },
    
    // Unary operations
    UnaryOp {
        op: UnaryOperator,
        operand: Box<Expression>,
        pos: Position,
    },
    
    // Control flow (all return values)
    If {
        condition: Box<Expression>,
        then_branch: Box<Expression>,
        else_branch: Option<Box<Expression>>,
        pos: Position,
    },
    
    Match {
        expr: Box<Expression>,
        arms: Vec<MatchArm>,
        pos: Position,
    },
    
    // Blocks return the value of their last expression
    Block {
        expressions: Vec<Expression>,
        pos: Position,
    },
    
    // Variable binding (returns the bound value)
    Let {
        name: String,
        type_annotation: Option<Type>,
        value: Box<Expression>,
        is_mutable: bool, // var vs let
        pos: Position,
    },
    
    // Function definition
    Function {
        name: String,
        params: Vec<Parameter>,
        return_type: Option<Type>,
        body: Box<Expression>,
        is_async: bool,
        receiver: Option<Receiver>, // For method syntax
        pos: Position,
    },
    
    // Lambda expression
    Lambda {
        params: Vec<Parameter>,
        body: Box<Expression>,
        return_type: Option<Type>,
        pos: Position,
    },
    
    // Function/method call
    Call {
        callee: Box<Expression>,
        args: Vec<Expression>,
        pos: Position,
    },
    
    // Member access (dot notation)
    MemberAccess {
        object: Box<Expression>,
        member: String,
        is_safe: bool, // true for ?. operator
        pos: Position,
    },
    
    // Index access (brackets)
    IndexAccess {
        object: Box<Expression>,
        index: Box<Expression>,
        pos: Position,
    },
    
    // Nullable operators
    Elvis {
        nullable: Box<Expression>,
        default: Box<Expression>,
        pos: Position,
    },
    
    ForceUnwrap {
        nullable: Box<Expression>,
        pos: Position,
    },
    
    // Struct literal
    StructLiteral {
        name: String,
        fields: Vec<(String, Expression)>,
        pos: Position,
    },
    
    // Array/List literal
    ArrayLiteral {
        elements: Vec<Expression>,
        pos: Position,
    },
    
    // Loop expressions (can return values with break)
    While {
        condition: Box<Expression>,
        body: Box<Expression>,
        pos: Position,
    },
    
    For {
        variable: String,
        iterable: Box<Expression>,
        body: Box<Expression>,
        pos: Position,
    },
    
    // Control flow
    Break {
        value: Option<Box<Expression>>, // break with value
        pos: Position,
    },
    
    Continue { pos: Position },
    
    Return {
        value: Option<Box<Expression>>,
        pos: Position,
    },
    
    // Async/await
    Await {
        expr: Box<Expression>,
        pos: Position,
    },
    
    // Type cast
    Cast {
        expr: Box<Expression>,
        target_type: Type,
        pos: Position,
    },
    
    // Assignment (returns the assigned value)
    Assignment {
        target: Box<Expression>,
        value: Box<Expression>,
        pos: Position,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BinaryOperator {
    // Arithmetic
    Add, Subtract, Multiply, Divide, Modulo,
    
    // Comparison
    Equal, NotEqual, Less, Greater, LessEqual, GreaterEqual,
    
    // Logical (word-based)
    And, Or,
    
    // Range
    InclusiveRange, // ..
    ExclusiveRange, // ..<
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UnaryOperator {
    Not,    // logical not
    Negate, // arithmetic negation
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expression>,
    pub body: Expression,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Pattern {
    Literal(Expression),
    Identifier(String),
    Wildcard,
    Range { start: Expression, end: Expression, inclusive: bool },
    Struct { name: String, fields: Vec<(String, Pattern)> },
    Array(Vec<Pattern>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub type_annotation: Option<Type>,
    pub default_value: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Receiver {
    pub name: String,
    pub type_name: String,
    pub is_mutable: bool, // inout keyword
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterpolationPart {
    pub kind: InterpolationKind,
    pub pos: Position,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InterpolationKind {
    Text(String),
    Expression(Box<Expression>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Type {
    pub name: String,
    pub is_nullable: bool,
    pub generics: Vec<Type>,
}

impl Type {
    pub fn new(name: &str) -> Self {
        Type {
            name: name.to_string(),
            is_nullable: false,
            generics: Vec::new(),
        }
    }
    
    pub fn nullable(mut self) -> Self {
        self.is_nullable = true;
        self
    }
    
    pub fn with_generics(mut self, generics: Vec<Type>) -> Self {
        self.generics = generics;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ast_creation() {
        let program = Program {
            expressions: vec![Expression::Placeholder],
        };
        assert_eq!(program.expressions.len(), 1);
    }
}