use seen_lexer::token::Location;
use serde::{Deserialize, Serialize};

/// The Abstract Syntax Tree (AST) for the Seen programming language
/// This represents the hierarchical structure of a Seen program

/// Type definition for an entire program
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub declarations: Vec<Declaration>,
    pub location: Location,
}

/// Declarations in the program (functions, variables, structs, or enums)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Declaration {
    Function(FunctionDeclaration),
    Variable(VariableDeclaration),
    Struct(StructDeclaration),
    Enum(EnumDeclaration),  // NEW: Enum support for Phase 2
}

/// Function declaration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionDeclaration {
    pub name: String,
    pub type_parameters: Vec<String>,  // Generic type parameters like <T, U>
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

/// Struct declaration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructDeclaration {
    pub name: String,
    pub type_parameters: Vec<String>,  // Generic type parameters like <T, U>
    pub fields: Vec<StructField>,
    pub location: Location,
}

/// Struct field definition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructField {
    pub name: String,
    pub field_type: Type,
    pub location: Location,
}

/// Enum declaration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumDeclaration {
    pub name: String,
    pub type_parameters: Vec<String>,  // Generic type parameters like <T, E>
    pub variants: Vec<EnumVariant>,
    pub location: Location,
}

/// Enum variant definition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumVariant {
    pub name: String,
    pub data: Option<Vec<Type>>,  // For variants with data like Some(T)
    pub location: Location,
}

/// Type definitions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Type {
    Simple(String),     // Simple types like "int", "string", etc.
    Array(Box<Type>),   // Array types like [int], [string], etc.
    Struct(String),     // Struct types like "Point", "Person", etc.
    Enum(String),       // Enum types like "Option", "Result", etc.
    Generic(String, Vec<Type>), // Generic types like Option<T>, Result<T,E>
    Pointer(Box<Type>), // Pointer types like *Int, *String, etc.
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
    DeclarationStatement(Declaration),
    For(ForStatement),
    Match(MatchStatement),  // NEW: Pattern matching for Phase 2
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
    pub arguments: Vec<Expression>,
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
    // Struct-related expressions
    StructLiteral(StructLiteralExpression),
    FieldAccess(FieldAccessExpression),
    // Array-related expressions
    ArrayLiteral(ArrayLiteralExpression),
    Index(IndexExpression),
    Range(RangeExpression),
    // Pattern matching and enum expressions
    Match(MatchExpression),
    EnumLiteral(EnumLiteralExpression),
    // Error handling expressions
    Try(TryExpression),
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

/// Struct literal expression: Point { x: 10, y: 20 }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructLiteralExpression {
    pub struct_name: String,
    pub fields: Vec<StructFieldInit>,
    pub location: Location,
}

/// Field initialization in struct literal
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructFieldInit {
    pub field_name: String,
    pub value: Box<Expression>,
    pub location: Location,
}

/// Field access expression: object.field
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldAccessExpression {
    pub object: Box<Expression>,
    pub field: String,
    pub location: Location,
}

/// Array literal expression: [1, 2, 3]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArrayLiteralExpression {
    pub elements: Vec<Expression>,
    pub location: Location,
}

/// Index expression: array[index]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexExpression {
    pub object: Box<Expression>,
    pub index: Box<Expression>,
    pub location: Location,
}

/// Range expression: start..end
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RangeExpression {
    pub start: Box<Expression>,
    pub end: Box<Expression>,
    pub location: Location,
}

/// For statement: for x in collection { ... }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForStatement {
    pub variable: String,
    pub iterable: Expression,
    pub body: Box<Statement>,
    pub location: Location,
}

/// Match statement: match value { pattern => expr, ... }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchStatement {
    pub value: Box<Expression>,
    pub arms: Vec<MatchArm>,
    pub location: Location,
}

/// Match expression: match value { pattern => expr, ... }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchExpression {
    pub value: Box<Expression>,
    pub arms: Vec<MatchArm>,
    pub location: Location,
}

/// Match arm: pattern => expression
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub expression: Box<Expression>,
    pub location: Location,
}

/// Patterns for match expressions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Pattern {
    Literal(LiteralPattern),
    Identifier(IdentifierPattern),
    EnumVariant(EnumVariantPattern),
    Wildcard(WildcardPattern),
}

/// Literal pattern: 42, "hello", true
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiteralPattern {
    pub value: LiteralExpression,
    pub location: Location,
}

/// Identifier pattern: x (binds to variable)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IdentifierPattern {
    pub name: String,
    pub location: Location,
}

/// Enum variant pattern: Some(x), None
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumVariantPattern {
    pub enum_name: String,
    pub variant_name: String,
    pub patterns: Option<Vec<Pattern>>,
    pub location: Location,
}

/// Wildcard pattern: _
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WildcardPattern {
    pub location: Location,
}

/// Enum literal expression: Some(42), None
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumLiteralExpression {
    pub enum_name: String,
    pub type_arguments: Option<Vec<Type>>,  // For generic enums like Option<Int>
    pub variant_name: String,
    pub arguments: Option<Vec<Expression>>,
    pub location: Location,
}

/// Try expression: expr? (for error propagation with Result<T, E>)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TryExpression {
    pub expression: Box<Expression>,
    pub location: Location,
}

