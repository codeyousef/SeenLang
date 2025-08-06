//! Abstract Syntax Tree definitions for the Seen language

use seen_common::{Span, Spanned};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::fmt;

/// Type alias for node IDs
pub type NodeId = u32;

/// A complete Seen program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Program<'a> {
    pub items: Vec<Item<'a>>,
    pub span: Span,
}

/// Top-level items in a Seen program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item<'a> {
    pub kind: ItemKind<'a>,
    pub span: Span,
    pub id: NodeId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ItemKind<'a> {
    Function(Function<'a>),
    Struct(Struct<'a>),
    Enum(Enum<'a>),
    Impl(Impl<'a>),
    Trait(TraitDef<'a>),
    Module(ModuleDef<'a>),
    Import(Import<'a>),
    TypeAlias(TypeAlias<'a>),
    Const(Const<'a>),
    Static(Static<'a>),
}

/// Function definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function<'a> {
    pub name: Spanned<&'a str>,
    pub params: Vec<Parameter<'a>>,
    pub return_type: Option<Type<'a>>,
    pub body: Block<'a>,
    pub visibility: Visibility,
    pub attributes: Vec<Attribute<'a>>,
}

/// Function parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter<'a> {
    pub name: Spanned<&'a str>,
    pub ty: Type<'a>,
    pub is_mutable: bool,
    pub span: Span,
}

/// Struct definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Struct<'a> {
    pub name: Spanned<&'a str>,
    pub fields: Vec<Field<'a>>,
    pub visibility: Visibility,
    pub generic_params: Vec<GenericParam<'a>>,
    pub attributes: Vec<Attribute<'a>>,
}

/// Struct field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field<'a> {
    pub name: Spanned<&'a str>,
    pub ty: Type<'a>,
    pub visibility: Visibility,
    pub span: Span,
}

/// Enum definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Enum<'a> {
    pub name: Spanned<&'a str>,
    pub variants: Vec<Variant<'a>>,
    pub visibility: Visibility,
    pub generic_params: Vec<GenericParam<'a>>,
    pub attributes: Vec<Attribute<'a>>,
}

/// Enum variant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variant<'a> {
    pub name: Spanned<&'a str>,
    pub data: VariantData<'a>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VariantData<'a> {
    Unit,
    Tuple(Vec<Type<'a>>),
    Struct(Vec<Field<'a>>),
}

/// Implementation block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Impl<'a> {
    pub self_type: Type<'a>,
    pub trait_ref: Option<Type<'a>>,
    pub items: Vec<ImplItem<'a>>,
    pub generic_params: Vec<GenericParam<'a>>,
}

/// Item within an impl block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplItem<'a> {
    pub kind: ImplItemKind<'a>,
    pub span: Span,
    pub id: NodeId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImplItemKind<'a> {
    Function(Function<'a>),
    Const(Const<'a>),
    Type(TypeAlias<'a>),
}

/// Trait definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitDef<'a> {
    pub name: Spanned<&'a str>,
    pub items: Vec<TraitItem<'a>>,
    pub generic_params: Vec<GenericParam<'a>>,
    pub supertraits: Vec<Type<'a>>,
    pub visibility: Visibility,
}

/// Item within a trait
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitItem<'a> {
    pub kind: TraitItemKind<'a>,
    pub span: Span,
    pub id: NodeId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TraitItemKind<'a> {
    Function(TraitFunction<'a>),
    Const(TraitConst<'a>),
    Type(TraitType<'a>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitFunction<'a> {
    pub name: Spanned<&'a str>,
    pub params: Vec<Parameter<'a>>,
    pub return_type: Option<Type<'a>>,
    pub default_body: Option<Block<'a>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitConst<'a> {
    pub name: Spanned<&'a str>,
    pub ty: Type<'a>,
    pub default_value: Option<Expr<'a>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitType<'a> {
    pub name: Spanned<&'a str>,
    pub bounds: Vec<Type<'a>>,
    pub default_type: Option<Type<'a>>,
}

/// Module definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDef<'a> {
    pub name: Spanned<&'a str>,
    pub items: Vec<Item<'a>>,
    pub visibility: Visibility,
}

/// Import statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Import<'a> {
    pub path: ImportPath<'a>,
    pub kind: ImportKind<'a>,
    pub visibility: Visibility,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPath<'a> {
    pub segments: Vec<Spanned<&'a str>>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImportKind<'a> {
    Single,
    Glob,
    List(Vec<ImportItem<'a>>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportItem<'a> {
    pub name: Spanned<&'a str>,
    pub alias: Option<Spanned<&'a str>>,
}

/// Type alias
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeAlias<'a> {
    pub name: Spanned<&'a str>,
    pub ty: Type<'a>,
    pub generic_params: Vec<GenericParam<'a>>,
    pub visibility: Visibility,
}

/// Constant definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Const<'a> {
    pub name: Spanned<&'a str>,
    pub ty: Type<'a>,
    pub value: Expr<'a>,
    pub visibility: Visibility,
}

/// Static definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Static<'a> {
    pub name: Spanned<&'a str>,
    pub ty: Type<'a>,
    pub value: Expr<'a>,
    pub is_mutable: bool,
    pub visibility: Visibility,
}

/// Generic parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericParam<'a> {
    pub name: Spanned<&'a str>,
    pub bounds: Vec<Type<'a>>,
    pub default: Option<Type<'a>>,
    pub span: Span,
}

/// Visibility modifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    Public,
    Private,
}

/// Attribute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attribute<'a> {
    pub name: Spanned<&'a str>,
    pub args: Vec<AttrArg<'a>>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AttrArg<'a> {
    Literal(Literal<'a>),
    Identifier(Spanned<&'a str>),
    KeyValue { key: Spanned<&'a str>, value: Box<AttrArg<'a>> },
}

/// Type representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Type<'a> {
    pub kind: Box<TypeKind<'a>>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TypeKind<'a> {
    /// Primitive types (i32, f64, bool, etc.)
    Primitive(PrimitiveType),
    /// Named type (User-defined or from imports)
    Named { path: Path<'a>, generic_args: Vec<Type<'a>> },
    /// Tuple type
    Tuple(Vec<Type<'a>>),
    /// Array type
    Array { element_type: Box<Type<'a>>, size: Option<Box<Expr<'a>>> },
    /// Function type
    Function { params: Vec<Type<'a>>, return_type: Box<Type<'a>> },
    /// Reference type
    Reference { inner: Box<Type<'a>>, is_mutable: bool },
    /// Inferred type (for type inference)
    Infer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrimitiveType {
    I8, I16, I32, I64, I128,
    U8, U16, U32, U64, U128,
    F32, F64,
    Bool,
    Char,
    Str,
    Unit,
}

/// Path (for types, functions, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Path<'a> {
    pub segments: Vec<PathSegment<'a>>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathSegment<'a> {
    pub name: Spanned<&'a str>,
    pub generic_args: Vec<Type<'a>>,
}

/// Block of statements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block<'a> {
    pub statements: Vec<Stmt<'a>>,
    pub span: Span,
}

/// Statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stmt<'a> {
    pub kind: StmtKind<'a>,
    pub span: Span,
    pub id: NodeId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StmtKind<'a> {
    /// Expression statement
    Expr(Expr<'a>),
    /// Variable declaration
    Let(Let<'a>),
    /// Item declaration
    Item(Item<'a>),
    /// Empty statement (semicolon)
    Empty,
}

/// Variable declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Let<'a> {
    pub pattern: Pattern<'a>,
    pub ty: Option<Type<'a>>,
    pub initializer: Option<Expr<'a>>,
    pub is_mutable: bool,
}

/// Pattern (for destructuring)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern<'a> {
    pub kind: PatternKind<'a>,
    pub span: Span,
    pub id: NodeId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternKind<'a> {
    /// Variable binding
    Identifier(Spanned<&'a str>),
    /// Wildcard pattern
    Wildcard,
    /// Literal pattern
    Literal(Literal<'a>),
    /// Tuple pattern
    Tuple(Vec<Pattern<'a>>),
    /// Struct pattern
    Struct { path: Path<'a>, fields: Vec<FieldPattern<'a>>, rest: bool },
    /// Enum pattern
    Enum { path: Path<'a>, pattern: Option<Box<Pattern<'a>>> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldPattern<'a> {
    pub name: Spanned<&'a str>,
    pub pattern: Pattern<'a>,
}

/// Expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Expr<'a> {
    pub kind: Box<ExprKind<'a>>,
    pub span: Span,
    pub id: NodeId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExprKind<'a> {
    /// Literal value
    Literal(Literal<'a>),
    /// Variable reference
    Identifier(Spanned<&'a str>),
    /// Path expression
    Path(Path<'a>),
    /// Binary operation
    Binary { op: BinaryOp, left: Box<Expr<'a>>, right: Box<Expr<'a>> },
    /// Unary operation
    Unary { op: UnaryOp, operand: Box<Expr<'a>> },
    /// Function call
    Call { function: Box<Expr<'a>>, args: Vec<Expr<'a>> },
    /// Method call
    MethodCall { receiver: Box<Expr<'a>>, method: Spanned<&'a str>, args: Vec<Expr<'a>> },
    /// Field access
    FieldAccess { object: Box<Expr<'a>>, field: Spanned<&'a str> },
    /// Array indexing
    Index { array: Box<Expr<'a>>, index: Box<Expr<'a>> },
    /// Tuple expression
    Tuple(Vec<Expr<'a>>),
    /// Array expression
    Array(Vec<Expr<'a>>),
    /// Struct construction
    Struct { path: Path<'a>, fields: Vec<FieldExpr<'a>> },
    /// Block expression
    Block(Block<'a>),
    /// If expression
    If { condition: Box<Expr<'a>>, then_branch: Block<'a>, else_branch: Option<Box<Expr<'a>>> },
    /// Match expression
    Match { scrutinee: Box<Expr<'a>>, arms: Vec<MatchArm<'a>> },
    /// While loop
    While { condition: Box<Expr<'a>>, body: Block<'a> },
    /// For loop
    For { pattern: Pattern<'a>, iterator: Box<Expr<'a>>, body: Block<'a> },
    /// Break expression
    Break(Option<Box<Expr<'a>>>),
    /// Continue expression
    Continue,
    /// Return expression
    Return(Option<Box<Expr<'a>>>),
    /// Assignment
    Assign { target: Box<Expr<'a>>, value: Box<Expr<'a>> },
    /// Compound assignment
    AssignOp { op: BinaryOp, target: Box<Expr<'a>>, value: Box<Expr<'a>> },
    /// Range expression
    Range { start: Option<Box<Expr<'a>>>, end: Option<Box<Expr<'a>>>, inclusive: bool },
    /// Type cast
    Cast { expr: Box<Expr<'a>>, ty: Box<Type<'a>> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldExpr<'a> {
    pub name: Spanned<&'a str>,
    pub value: Expr<'a>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchArm<'a> {
    pub pattern: Pattern<'a>,
    pub guard: Option<Expr<'a>>,
    pub body: Expr<'a>,
}

/// Literal values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Literal<'a> {
    pub kind: LiteralKind<'a>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LiteralKind<'a> {
    Integer(i64),
    Float(f64),
    String(&'a str),
    Char(char),
    Boolean(bool),
    Unit,
}

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    // Arithmetic
    Add, Sub, Mul, Div, Mod,
    // Comparison
    Eq, Ne, Lt, Le, Gt, Ge,
    // Logical
    And, Or,
    // Bitwise
    BitAnd, BitOr, BitXor, Shl, Shr,
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Not,      // !
    Neg,      // -
    BitNot,   // ~
    Deref,    // *
    Ref,      // &
    RefMut,   // &mut
}

// Display implementations for better error messages
impl fmt::Display for PrimitiveType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            PrimitiveType::I8 => "i8",
            PrimitiveType::I16 => "i16",
            PrimitiveType::I32 => "i32",
            PrimitiveType::I64 => "i64",
            PrimitiveType::I128 => "i128",
            PrimitiveType::U8 => "u8",
            PrimitiveType::U16 => "u16",
            PrimitiveType::U32 => "u32",
            PrimitiveType::U64 => "u64",
            PrimitiveType::U128 => "u128",
            PrimitiveType::F32 => "f32",
            PrimitiveType::F64 => "f64",
            PrimitiveType::Bool => "bool",
            PrimitiveType::Char => "char",
            PrimitiveType::Str => "str",
            PrimitiveType::Unit => "()",
        };
        write!(f, "{}", name)
    }
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let op = match self {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Mod => "%",
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
            BinaryOp::And => "&&",
            BinaryOp::Or => "||",
            BinaryOp::BitAnd => "&",
            BinaryOp::BitOr => "|",
            BinaryOp::BitXor => "^",
            BinaryOp::Shl => "<<",
            BinaryOp::Shr => ">>",
        };
        write!(f, "{}", op)
    }
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let op = match self {
            UnaryOp::Not => "!",
            UnaryOp::Neg => "-",
            UnaryOp::BitNot => "~",
            UnaryOp::Deref => "*",
            UnaryOp::Ref => "&",
            UnaryOp::RefMut => "&mut",
        };
        write!(f, "{}", op)
    }
}