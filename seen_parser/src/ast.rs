use serde::{Serialize, Deserialize};
use seen_lexer::token::Location;

/// The Abstract Syntax Tree (AST) for the Seen programming language
/// This represents the hierarchical structure of a Seen program

/// Type definition for an entire program
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub declarations: Vec<Declaration>,
    pub location: Location,
}

/// Declarations in the program (functions or variables)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Declaration {
    Function(FunctionDeclaration),
    Variable(VariableDeclaration),
}

/// Function declaration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionDeclaration {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<Type>,
    pub body: Block,
    pub location: Location,
}

/// Function parameter
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub param_type: Type,
    pub location: Location,
}

/// Variable declaration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableDeclaration {
    pub is_mutable: bool,  // true for var/متغير, false for val/ثابت
    pub name: String,
    pub var_type: Option<Type>,
    pub initializer: Box<Expression>,
    pub location: Location,
}

/// Type definitions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Type {
    Simple(String),  // Simple types like "int", "string", etc.
    Array(Box<Type>), // Array types like [int], [string], etc.
}

/// Statements in the program
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Statement {
    Expression(ExpressionStatement),
    Block(Block),
    Return(ReturnStatement),
    If(IfStatement),
    While(WhileStatement),
    Print(PrintStatement),
}

/// Expression statement (expression followed by semicolon)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpressionStatement {
    pub expression: Box<Expression>,
    pub location: Location,
}

/// Block statement (a group of statements in curly braces)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub location: Location,
}

/// Return statement
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReturnStatement {
    pub value: Option<Box<Expression>>,
    pub location: Location,
}

/// If statement
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IfStatement {
    pub condition: Box<Expression>,
    pub then_branch: Box<Statement>,
    pub else_branch: Option<Box<Statement>>,
    pub location: Location,
}

/// While statement
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhileStatement {
    pub condition: Box<Expression>,
    pub body: Box<Statement>,
    pub location: Location,
}

/// Print statement (for Hello World MVP)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrintStatement {
    pub expression: Box<Expression>,
    pub location: Location,
}

/// Expressions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expression {
    Assignment(AssignmentExpression),
    Binary(BinaryExpression),
    Unary(UnaryExpression),
    Literal(LiteralExpression),
    Identifier(IdentifierExpression),
    Call(CallExpression),
    Parenthesized(ParenthesizedExpression),
}

/// Assignment expression
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssignmentExpression {
    pub name: String,
    pub value: Box<Expression>,
    pub location: Location,
}

/// Binary expression
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BinaryExpression {
    pub left: Box<Expression>,
    pub operator: BinaryOperator,
    pub right: Box<Expression>,
    pub location: Location,
}

/// Binary operators
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BinaryOperator {
    Add,        // +
    Subtract,   // -
    Multiply,   // *
    Divide,     // /
    Modulo,     // %
    Equal,      // ==
    NotEqual,   // !=
    LessThan,   // <
    GreaterThan, // >
    LessEqual,  // <=
    GreaterEqual, // >=
    And,        // &&
    Or,         // ||
}

/// Unary expression
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnaryExpression {
    pub operator: UnaryOperator,
    pub operand: Box<Expression>,
    pub location: Location,
}

/// Unary operators
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UnaryOperator {
    Negate,     // -
    Not,        // !
    Plus,       // + (usually a no-op)
}

/// Literal expression
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LiteralExpression {
    Number(NumberLiteral),
    String(StringLiteral),
    Boolean(BooleanLiteral),
    Null(NullLiteral),
}

/// Number literal
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NumberLiteral {
    pub value: String,  // Using string to preserve the original text
    pub is_float: bool, // Whether this is a floating-point value
    pub location: Location,
}

/// String literal
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StringLiteral {
    pub value: String,
    pub location: Location,
}

/// Boolean literal
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BooleanLiteral {
    pub value: bool,
    pub location: Location,
}

/// Null literal
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NullLiteral {
    pub location: Location,
}

/// Identifier expression
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IdentifierExpression {
    pub name: String,
    pub location: Location,
}

/// Function call expression
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallExpression {
    pub callee: String,
    pub arguments: Vec<Expression>,
    pub location: Location,
}

/// Parenthesized expression
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParenthesizedExpression {
    pub expression: Box<Expression>,
    pub location: Location,
}
