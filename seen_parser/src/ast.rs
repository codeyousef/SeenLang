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
    
    // Struct definition
    StructDefinition {
        name: String,
        fields: Vec<StructField>,
        pos: Position,
    },
    
    // Class definition
    ClassDefinition {
        name: String,
        superclass: Option<String>,
        fields: Vec<ClassField>,
        methods: Vec<Method>,
        pos: Position,
    },
    
    // Struct literal (instantiation)
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
    
    // Loop expression (returns break value)
    Loop {
        body: Box<Expression>,
        pos: Position,
    },
    
    // Spawn expression for concurrency
    Spawn {
        expr: Box<Expression>,
        pos: Position,
    },
    
    // Select expression for channel operations
    Select {
        cases: Vec<SelectCase>,
        pos: Position,
    },
    
    // Actor definition
    Actor {
        name: String,
        fields: Vec<(String, Type)>,
        handlers: Vec<MessageHandler>,
        pos: Position,
    },
    
    // Send message to actor
    Send {
        message: Box<Expression>,
        target: Box<Expression>,
        pos: Position,
    },
    
    // Receive message
    Receive {
        pattern: Pattern,
        handler: Box<Expression>,
        pos: Position,
    },
    
    // Memory management blocks
    Region {
        name: Option<String>,
        body: Box<Expression>,
        pos: Position,
    },
    
    Arena {
        body: Box<Expression>,
        pos: Position,
    },
    
    // Metaprogramming
    Comptime {
        body: Box<Expression>,
        pos: Position,
    },
    
    Macro {
        name: String,
        params: Vec<String>,
        body: Box<Expression>,
        pos: Position,
    },
    
    // Effects
    Effect {
        name: String,
        operations: Vec<EffectOperation>,
        pos: Position,
    },
    
    Handle {
        body: Box<Expression>,
        effect: String,
        handlers: Vec<EffectHandler>,
        pos: Position,
    },
    
    // Contracts
    ContractedFunction {
        function: Box<Expression>,
        requires: Option<Box<Expression>>,
        ensures: Option<Box<Expression>>,
        invariants: Vec<Expression>,
        pos: Position,
    },
    
    // Error handling
    Defer {
        body: Box<Expression>,
        pos: Position,
    },
    
    Assert {
        condition: Box<Expression>,
        message: Option<String>,
        pos: Position,
    },
    
    Try {
        body: Box<Expression>,
        catch_clauses: Vec<CatchClause>,
        finally: Option<Box<Expression>>,
        pos: Position,
    },
    
    // OOP
    Extension {
        target_type: Type,
        methods: Vec<Expression>,
        pos: Position,
    },
    
    Interface {
        name: String,
        methods: Vec<InterfaceMethod>,
        pos: Position,
    },
    
    Class {
        name: String,
        is_sealed: bool,
        is_open: bool,
        is_abstract: bool,
        fields: Vec<(String, Type)>,
        methods: Vec<Expression>,
        companion: Option<Box<Expression>>,
        pos: Position,
    },
    
    // Annotations
    Annotated {
        annotations: Vec<Annotation>,
        expr: Box<Expression>,
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
    Literal(Box<Expression>),
    Identifier(String),
    Wildcard,
    Range { start: Box<Expression>, end: Box<Expression>, inclusive: bool },
    Struct { name: String, fields: Vec<(String, Box<Pattern>)> },
    Array(Vec<Box<Pattern>>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub type_annotation: Option<Type>,
    pub default_value: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructField {
    pub name: String,
    pub field_type: Type,
    pub is_public: bool, // Capitalized fields are public
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassField {
    pub name: String,
    pub field_type: Type,
    pub is_public: bool, // Capitalized fields are public
    pub is_mutable: bool, // var vs let
    pub default_value: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Method {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<Type>,
    pub body: Expression,
    pub is_public: bool, // Capitalized methods are public
    pub is_static: bool, // Static methods don't have self
    pub receiver: Option<Receiver>, // None for static methods
    pub pos: Position,
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
pub struct Field {
    pub name: String,
    pub type_annotation: Type,
    pub is_public: bool,
    pub is_mutable: bool,
    pub default_value: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumVariant {
    pub name: String,
    pub fields: Option<Vec<Field>>, // None for simple variant, Some for tuple/struct variant
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraitMethod {
    pub name: String,
    pub params: Vec<Parameter>,
    pub return_type: Option<Type>,
    pub has_default_impl: bool,
    pub body: Option<Expression>, // None for abstract, Some for default impl
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Type {
    pub name: String,
    pub is_nullable: bool,
    pub generics: Vec<Type>,
}

// Supporting types for new AST nodes

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectCase {
    pub channel: Box<Expression>,
    pub pattern: Pattern,
    pub handler: Box<Expression>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageHandler {
    pub message_type: String,
    pub params: Vec<Parameter>,
    pub body: Box<Expression>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectOperation {
    pub name: String,
    pub params: Vec<Parameter>,
    pub return_type: Option<Type>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectHandler {
    pub operation: String,
    pub params: Vec<Parameter>,
    pub body: Box<Expression>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatchClause {
    pub exception_type: Option<Type>,
    pub variable: Option<String>,
    pub body: Box<Expression>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterfaceMethod {
    pub name: String,
    pub params: Vec<Parameter>,
    pub return_type: Option<Type>,
    pub is_default: bool,
    pub default_impl: Option<Box<Expression>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    pub name: String,
    pub args: Vec<Expression>,
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
            expressions: vec![Expression::IntegerLiteral { value: 42, pos: Position::new(1, 1, 0) }],
        };
        assert_eq!(program.expressions.len(), 1);
    }
}