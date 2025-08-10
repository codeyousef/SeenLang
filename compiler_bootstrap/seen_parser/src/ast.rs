//! Abstract Syntax Tree definitions for the Seen language

use seen_common::{Span, Spanned};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Type alias for node IDs
pub type NodeId = u32;

/// Ownership mode for function parameters - supports automatic inference
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OwnershipMode {
    /// Automatic inference - compiler determines ownership from usage (default)
    Automatic,
    /// Explicit move - parameter ownership is transferred
    Move,
    /// Explicit immutable borrow - parameter is borrowed immutably
    Borrow,
    /// Explicit mutable borrow - parameter is borrowed mutably (using 'mut' keyword)
    BorrowMut,
    /// In-place modification - Vale-style inout parameter
    Inout,
}

/// A complete Seen program
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Program<'a> {
    pub items: Vec<Item<'a>>,
    pub span: Span,
}

/// Top-level items in a Seen program
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Item<'a> {
    pub kind: ItemKind<'a>,
    pub span: Span,
    pub id: NodeId,
}

#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
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
    // Kotlin-inspired features
    ExtensionFunction(ExtensionFunction<'a>),
    DataClass(DataClass<'a>),
    SealedClass(SealedClass<'a>),
    Class(Class<'a>),
    Property(Property<'a>),
}

/// Function definition
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Function<'a> {
    pub name: Spanned<&'a str>,
    pub type_params: Vec<TypeParam<'a>>,
    pub params: Vec<Parameter<'a>>,
    pub return_type: Option<Type<'a>>,
    pub body: Block<'a>,
    pub visibility: Visibility,
    pub attributes: Vec<Attribute<'a>>,
    pub is_inline: bool,
    pub is_suspend: bool,
    pub is_operator: bool,
    pub is_infix: bool,
    pub is_tailrec: bool,
}

/// Function parameter
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Parameter<'a> {
    pub name: Spanned<&'a str>,
    pub ty: Type<'a>,
    pub is_mutable: bool,
    pub default_value: Option<Expr<'a>>, // Kotlin-style default parameters
    pub ownership: OwnershipMode, // New: automatic inference or explicit control
    pub span: Span,
}

/// Generic type parameter
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct TypeParam<'a> {
    pub name: Spanned<&'a str>,
    pub bounds: Vec<Type<'a>>,
    pub default_type: Option<Type<'a>>,
    pub is_reified: bool,
    pub span: Span,
}

/// Struct definition
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Struct<'a> {
    pub name: Spanned<&'a str>,
    pub fields: Vec<Field<'a>>,
    pub visibility: Visibility,
    pub generic_params: Vec<GenericParam<'a>>,
    pub attributes: Vec<Attribute<'a>>,
    pub companion_object: Option<CompanionObject<'a>>,
}

/// Struct field
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Field<'a> {
    pub name: Spanned<&'a str>,
    pub ty: Type<'a>,
    pub visibility: Visibility,
    pub span: Span,
}

/// Enum definition
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Enum<'a> {
    pub name: Spanned<&'a str>,
    pub variants: Vec<Variant<'a>>,
    pub visibility: Visibility,
    pub generic_params: Vec<GenericParam<'a>>,
    pub attributes: Vec<Attribute<'a>>,
}

/// Enum variant
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Variant<'a> {
    pub name: Spanned<&'a str>,
    pub data: VariantData<'a>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub enum VariantData<'a> {
    Unit,
    Tuple(Vec<Type<'a>>),
    Struct(Vec<Field<'a>>),
}

/// Implementation block
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Impl<'a> {
    pub self_type: Type<'a>,
    pub trait_ref: Option<Type<'a>>,
    pub items: Vec<ImplItem<'a>>,
    pub generic_params: Vec<GenericParam<'a>>,
}

/// Item within an impl block
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct ImplItem<'a> {
    pub kind: ImplItemKind<'a>,
    pub span: Span,
    pub id: NodeId,
}

#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub enum ImplItemKind<'a> {
    Function(Function<'a>),
    Const(Const<'a>),
    Type(TypeAlias<'a>),
}

/// Trait definition
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct TraitDef<'a> {
    pub name: Spanned<&'a str>,
    pub items: Vec<TraitItem<'a>>,
    pub generic_params: Vec<GenericParam<'a>>,
    pub supertraits: Vec<Type<'a>>,
    pub visibility: Visibility,
}

/// Item within a trait
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct TraitItem<'a> {
    pub kind: TraitItemKind<'a>,
    pub span: Span,
    pub id: NodeId,
}

#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub enum TraitItemKind<'a> {
    Function(TraitFunction<'a>),
    Const(TraitConst<'a>),
    Type(TraitType<'a>),
}

#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct TraitFunction<'a> {
    pub name: Spanned<&'a str>,
    pub params: Vec<Parameter<'a>>,
    pub return_type: Option<Type<'a>>,
    pub default_body: Option<Block<'a>>,
    pub generic_params: Vec<TypeParam<'a>>,
}

#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct TraitConst<'a> {
    pub name: Spanned<&'a str>,
    pub ty: Type<'a>,
    pub default_value: Option<Expr<'a>>,
}

#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct TraitType<'a> {
    pub name: Spanned<&'a str>,
    pub bounds: Vec<Type<'a>>,
    pub default_type: Option<Type<'a>>,
}

/// Module definition
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct ModuleDef<'a> {
    pub name: Spanned<&'a str>,
    pub items: Vec<Item<'a>>,
    pub visibility: Visibility,
}

/// Import statement
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Import<'a> {
    pub path: ImportPath<'a>,
    pub kind: ImportKind<'a>,
    pub visibility: Visibility,
}

#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct ImportPath<'a> {
    pub segments: Vec<Spanned<&'a str>>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub enum ImportKind<'a> {
    Single,
    Glob,
    List(Vec<ImportItem<'a>>),
}

#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct ImportItem<'a> {
    pub name: Spanned<&'a str>,
    pub alias: Option<Spanned<&'a str>>,
}

/// Type alias
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct TypeAlias<'a> {
    pub name: Spanned<&'a str>,
    pub ty: Type<'a>,
    pub generic_params: Vec<GenericParam<'a>>,
    pub visibility: Visibility,
}

/// Constant definition
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Const<'a> {
    pub name: Spanned<&'a str>,
    pub ty: Type<'a>,
    pub value: Expr<'a>,
    pub visibility: Visibility,
}

/// Static definition
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Static<'a> {
    pub name: Spanned<&'a str>,
    pub ty: Type<'a>,
    pub value: Expr<'a>,
    pub is_mutable: bool,
    pub visibility: Visibility,
}

/// Generic parameter
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
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
    Internal,
}

/// Attribute
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Attribute<'a> {
    pub name: Spanned<&'a str>,
    pub args: Vec<AttrArg<'a>>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub enum AttrArg<'a> {
    Literal(Literal<'a>),
    Identifier(Spanned<&'a str>),
    KeyValue { key: Spanned<&'a str>, value: Box<AttrArg<'a>> },
}

/// Type representation
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Type<'a> {
    pub kind: Box<TypeKind<'a>>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
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
    /// Nullable type (Kotlin-style T?)
    Nullable(Box<Type<'a>>),
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
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Path<'a> {
    pub segments: Vec<PathSegment<'a>>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct PathSegment<'a> {
    pub name: Spanned<&'a str>,
    pub generic_args: Vec<Type<'a>>,
}

/// Block of statements
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Block<'a> {
    pub statements: Vec<Stmt<'a>>,
    pub span: Span,
}

/// Statement
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Stmt<'a> {
    pub kind: StmtKind<'a>,
    pub span: Span,
    pub id: NodeId,
}

#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub enum StmtKind<'a> {
    /// Expression statement
    Expr(Expr<'a>),
    /// Variable declaration
    Let(Let<'a>),
    /// Item declaration
    Item(Item<'a>),
    /// Return statement
    Return(Option<Expr<'a>>),
    /// Empty statement (semicolon)
    Empty,
}

/// Variable declaration
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Let<'a> {
    pub pattern: Pattern<'a>,
    pub ty: Option<Type<'a>>,
    pub initializer: Option<Expr<'a>>,
    pub is_mutable: bool,
}

/// Pattern (for destructuring)
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Pattern<'a> {
    pub kind: PatternKind<'a>,
    pub span: Span,
    pub id: NodeId,
}

#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
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
    /// Destructuring pattern for data classes/tuples
    Destructuring(Vec<Pattern<'a>>),
}

#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct FieldPattern<'a> {
    pub name: Spanned<&'a str>,
    pub pattern: Pattern<'a>,
}

/// Expression
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Expr<'a> {
    pub kind: Box<ExprKind<'a>>,
    pub span: Span,
    pub id: NodeId,
}

#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
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
    /// Generic type instantiation (e.g., MutableStateFlow<User?>)
    GenericInstantiation { base: Box<Expr<'a>>, args: Vec<Type<'a>> },
    // Kotlin-inspired expressions
    /// Closure expression (|param| body)
    Closure(Closure<'a>),
    /// Named argument in function call
    NamedArg { name: Spanned<&'a str>, value: Box<Expr<'a>> },
    /// Null literal
    Null,
    /// Safe call operator (?.)
    SafeCall { receiver: Box<Expr<'a>>, method: Spanned<&'a str>, args: Vec<Expr<'a>> },
    /// Elvis operator (?:)
    Elvis { expr: Box<Expr<'a>>, fallback: Box<Expr<'a>> },
    // Coroutine expressions
    /// Await expression (await expr)
    Await { expr: Box<Expr<'a>> },
    /// Launch block expression (launch { ... })
    Launch { block: Block<'a> },
    /// Try-catch-finally expression
    TryCatch {
        try_block: Box<Block<'a>>,
        catch_blocks: Vec<(&'a str, Option<Type<'a>>, Block<'a>)>,
        finally_block: Option<Box<Block<'a>>>,
    },
    /// Flow builder expression (flow { ... })
    FlowBuilder { block: Block<'a> },
    /// Object expression (object : Interface { ... })
    ObjectExpression {
        supertype: Option<Box<Type<'a>>>,
        members: Vec<Item<'a>>,
    },
    /// Ownership cast expression (move expr, borrow expr, etc.)
    OwnershipCast {
        expr: Box<Expr<'a>>,
        mode: OwnershipMode,
    },
    /// String interpolation expression ("Hello {name}!")
    StringInterpolation {
        parts: Vec<StringInterpolationPart<'a>>,
    },
}

#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct FieldExpr<'a> {
    pub name: Spanned<&'a str>,
    pub value: Expr<'a>,
}

/// String interpolation parts
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub enum StringInterpolationPart<'a> {
    /// Static string literal part
    Literal(String),
    /// Expression to be interpolated
    Expression(Expr<'a>),
}

#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct MatchArm<'a> {
    pub pattern: Pattern<'a>,
    pub guard: Option<Expr<'a>>,
    pub body: Expr<'a>,
}

/// Literal values
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Literal<'a> {
    pub kind: LiteralKind<'a>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
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
    // Type checking
    Is, NotIs,
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

/// Companion object definition
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct CompanionObject<'a> {
    pub name: Option<Spanned<&'a str>>, // Optional name for companion object
    pub members: Vec<Item<'a>>, // Functions, properties, etc.
    pub span: Span,
}

// ========================= Kotlin-Inspired Features =========================

/// Extension function definition
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct ExtensionFunction<'a> {
    pub receiver_type: Type<'a>,
    pub function: Function<'a>,
}

/// Property declaration (Kotlin-style with optional delegation)
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Property<'a> {
    pub name: Spanned<&'a str>,
    pub ty: Option<Type<'a>>, // Optional type (can be inferred)
    pub is_mutable: bool, // val (false) vs var (true)
    pub initializer: Option<Expr<'a>>, // Initial value
    pub delegate: Option<Expr<'a>>, // Delegated to (by delegate)
    pub getter: Option<Block<'a>>, // Custom getter
    pub setter: Option<Block<'a>>, // Custom setter (only for var)
    pub visibility: Visibility,
    pub attributes: Vec<Attribute<'a>>,
}

/// Data class definition (Kotlin-style)
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct DataClass<'a> {
    pub name: Spanned<&'a str>,
    pub fields: Vec<DataClassField<'a>>,
    pub visibility: Visibility,
    pub generic_params: Vec<GenericParam<'a>>,
    pub attributes: Vec<Attribute<'a>>,
}

/// Data class field with mutability and default values
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct DataClassField<'a> {
    pub name: Spanned<&'a str>,
    pub ty: Type<'a>,
    pub is_mutable: bool, // val (false) vs var (true)
    pub default_value: Option<Expr<'a>>,
    pub delegate: Option<Expr<'a>>, // For delegated properties (by delegate)
    pub visibility: Visibility,
    pub span: Span,
}

/// Sealed class definition (for exhaustive pattern matching)
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct SealedClass<'a> {
    pub name: Spanned<&'a str>,
    pub variants: Vec<SealedClassVariant<'a>>,
    pub visibility: Visibility,
    pub generic_params: Vec<GenericParam<'a>>,
    pub attributes: Vec<Attribute<'a>>,
}

/// Sealed class variant
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct SealedClassVariant<'a> {
    pub name: Spanned<&'a str>,
    pub fields: Vec<DataClassField<'a>>,
    pub parent: Option<Type<'a>>,
    pub span: Span,
}

/// Regular class (like Kotlin class)
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Class<'a> {
    pub name: Spanned<&'a str>,
    pub generic_params: Vec<GenericParam<'a>>,
    pub superclass: Option<Type<'a>>,
    pub interfaces: Vec<Type<'a>>,
    pub body: Vec<ClassMember<'a>>,
    pub visibility: Visibility,
    pub attributes: Vec<Attribute<'a>>,
    pub span: Span,
}

/// Class member (method, property, nested class, etc.)
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub enum ClassMember<'a> {
    Method(Function<'a>),
    Property(Property<'a>),
    Constructor(Function<'a>),
    InitBlock(Block<'a>),
}

/// Closure expression
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct Closure<'a> {
    pub params: Vec<ClosureParam<'a>>,
    pub body: ClosureBody<'a>,
    pub return_type: Option<Type<'a>>,
}

/// Closure parameter (simplified compared to function parameters)
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct ClosureParam<'a> {
    pub name: Spanned<&'a str>,
    pub ty: Option<Type<'a>>, // Type can be inferred
}

/// Closure body can be expression or block
#[derive(Debug, Clone, Serialize)]
#[derive(Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub enum ClosureBody<'a> {
    Expression(Box<Expr<'a>>),
    Block(Block<'a>),
}

// ========================= End Kotlin Features =========================

impl<'a> Type<'a> {
    /// Create a nullable version of this type (T -> T?)
    pub fn make_nullable(self) -> Type<'a> {
        Type {
            kind: Box::new(TypeKind::Nullable(Box::new(self))),
            span: Span::default(),
        }
    }

    /// Check if this type is nullable
    pub fn is_nullable(&self) -> bool {
        matches!(*self.kind, TypeKind::Nullable(_))
    }

    /// Get the inner type if this is nullable, or self if not
    pub fn non_nullable_type(&self) -> &Type<'a> {
        match &*self.kind {
            TypeKind::Nullable(inner) => inner,
            _ => self,
        }
    }
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
            BinaryOp::Is => "is",
            BinaryOp::NotIs => "!is",
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

impl<'a> fmt::Display for Type<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &*self.kind {
            TypeKind::Primitive(p) => write!(f, "{}", p),
            TypeKind::Named { path, generic_args } => {
                write!(f, "{}", path)?;
                if !generic_args.is_empty() {
                    write!(f, "<")?;
                    for (i, arg) in generic_args.iter().enumerate() {
                        if i > 0 { write!(f, ", ")?; }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ">")?;
                }
                Ok(())
            },
            TypeKind::Tuple(types) => {
                write!(f, "(")?;
                for (i, ty) in types.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", ty)?;
                }
                write!(f, ")")
            },
            TypeKind::Array { element_type, size } => {
                write!(f, "[{}]", element_type)?;
                if let Some(_size_expr) = size {
                    // Size expression display would require implementing Display for Expr
                    // For now, just indicate that there's a size expression
                    write!(f, "; <size>")
                } else {
                    Ok(())
                }
            },
            TypeKind::Function { params, return_type } => {
                write!(f, "(")?;
                for (i, param) in params.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", param)?;
                }
                write!(f, ") -> {}", return_type)
            },
            TypeKind::Nullable(inner) => {
                write!(f, "{}?", inner)
            },
            TypeKind::Infer => write!(f, "_"),
        }
    }
}

impl<'a> fmt::Display for Path<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, segment) in self.segments.iter().enumerate() {
            if i > 0 { write!(f, "::")?; }
            write!(f, "{}", segment.name.value)?;
            if !segment.generic_args.is_empty() {
                write!(f, "<")?;
                for (j, arg) in segment.generic_args.iter().enumerate() {
                    if j > 0 { write!(f, ", ")?; }
                    write!(f, "{}", arg)?;
                }
                write!(f, ">")?;
            }
        }
        Ok(())
    }
}