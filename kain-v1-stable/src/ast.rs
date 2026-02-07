//! KAIN Abstract Syntax Tree
//!
//! The AST represents the structure of a KAIN program after parsing.
//! It captures all language constructs as first-class citizens:
//! - Functions with effect annotations
//! - Components (React-like UI)
//! - Shaders (GPU programs)
//! - Actors (Erlang-style concurrency)
//! - Comptime blocks (Zig-style compile-time execution)

use crate::span::Span;
use crate::effects::Effect;

/// A complete KAIN program/module
#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Item>,
    pub span: Span,
}

/// Top-level items in a module
#[derive(Debug, Clone)]
pub enum Item {
    /// `fn name(args) -> Type with Effects: body`
    Function(Function),
    
    /// `component Name(props) -> UI with Reactive: jsx`
    Component(Component),
    
    /// `shader Name(inputs) -> Fragment with GPU: body`
    Shader(Shader),
    
    /// `actor Name: handlers`
    Actor(Actor),
    
    /// `struct Name { fields }`
    Struct(Struct),
    
    /// `enum Name { variants }`
    Enum(Enum),
    
    /// `trait Name { methods }`
    Trait(Trait),
    
    /// `impl Trait for Type { methods }`
    Impl(Impl),
    
    /// `type Alias = Type`
    TypeAlias(TypeAlias),
    
    /// `use path::to::item`
    Use(Use),
    
    /// `mod name`
    Mod(Mod),
    
    /// `const NAME: Type = value`
    Const(Const),
    
    /// `comptime { code }`
    Comptime(ComptimeBlock),
    
    /// `macro name!(params) { expansion }`
    Macro(MacroDef),

    /// `test "name": body`
    Test(TestDef),
}

#[derive(Debug, Clone)]
pub struct TestDef {
    pub name: String,
    pub body: Block,
    pub span: Span,
}

// === FUNCTIONS ===

/// Function attribute/decorator (e.g., @wasm, @js, @inline)
#[derive(Debug, Clone)]
pub struct Attribute {
    pub name: String,
    pub args: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub generics: Vec<Generic>,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub effects: Vec<Effect>,
    pub body: Block,
    pub visibility: Visibility,
    pub attributes: Vec<Attribute>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub mutable: bool,
    pub default: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Generic {
    pub name: String,
    pub bounds: Vec<TypeBound>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeBound {
    pub trait_name: String,
    pub span: Span,
}

// === COMPONENTS (React-like UI) ===

#[derive(Debug, Clone)]
pub struct Component {
    pub name: String,
    pub props: Vec<Param>,
    pub state: Vec<StateDecl>,
    pub methods: Vec<Function>,
    pub effects: Vec<Effect>,
    pub body: JSXNode,
    pub visibility: Visibility,
    pub attributes: Vec<Attribute>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StateDecl {
    pub name: String,
    pub ty: Type,
    pub initial: Expr,
    pub weak: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum JSXNode {
    /// `<tag attr="value">children</tag>`
    Element {
        tag: String,
        attributes: Vec<JSXAttribute>,
        children: Vec<JSXNode>,
        span: Span,
    },
    /// `{expression}`
    Expression(Box<Expr>),
    /// Plain text
    Text(String, Span),
    /// `<Component prop={value} />`
    ComponentCall {
        name: String,
        props: Vec<JSXAttribute>,
        children: Vec<JSXNode>,
        span: Span,
    },
    /// `for item in list: jsx`
    For {
        binding: String,
        iter: Box<Expr>,
        body: Box<JSXNode>,
        span: Span,
    },
    /// `if cond: jsx [else: jsx]`
    If {
        condition: Box<Expr>,
        then_branch: Box<JSXNode>,
        else_branch: Option<Box<JSXNode>>,
        span: Span,
    },
    /// Fragment wrapper
    Fragment(Vec<JSXNode>, Span),
}

#[derive(Debug, Clone)]
pub struct JSXAttribute {
    pub name: String,
    pub value: JSXAttrValue,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum JSXAttrValue {
    String(String),
    Expr(Expr),
    Bool(bool),
}

// === SHADERS (GPU Programs) ===

#[derive(Debug, Clone)]
pub struct Shader {
    pub name: String,
    pub stage: ShaderStage,
    pub inputs: Vec<Param>,
    pub outputs: Type,
    pub uniforms: Vec<Uniform>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Compute,
}

#[derive(Debug, Clone)]
pub struct Uniform {
    pub name: String,
    pub ty: Type,
    pub binding: u32,
    pub span: Span,
}

// === ACTORS (Erlang-style Concurrency) ===

#[derive(Debug, Clone)]
pub struct Actor {
    pub name: String,
    pub state: Vec<StateDecl>,
    pub handlers: Vec<MessageHandler>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MessageHandler {
    pub message_type: String,
    pub params: Vec<Param>,
    pub body: Block,
    pub span: Span,
}

// === DATA STRUCTURES ===

#[derive(Debug, Clone)]
pub struct Struct {
    pub name: String,
    pub generics: Vec<Generic>,
    pub fields: Vec<Field>,
    pub visibility: Visibility,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: Type,
    pub visibility: Visibility,
    pub default: Option<Expr>,
    pub weak: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Enum {
    pub name: String,
    pub generics: Vec<Generic>,
    pub variants: Vec<Variant>,
    pub visibility: Visibility,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    pub fields: VariantFields,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum VariantFields {
    Unit,
    Tuple(Vec<Type>),
    Struct(Vec<Field>),
}

// === TRAITS AND IMPLS ===

#[derive(Debug, Clone)]
pub struct Trait {
    pub name: String,
    pub generics: Vec<Generic>,
    pub methods: Vec<TraitMethod>,
    pub visibility: Visibility,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TraitMethod {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub effects: Vec<Effect>,
    pub default_impl: Option<Block>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Impl {
    pub generics: Vec<Generic>,
    pub trait_name: Option<String>,
    pub target_type: Type,
    pub methods: Vec<Function>,
    pub span: Span,
}

// === TYPE SYSTEM ===

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    /// Named type: `Int`, `String`, `Vec<T>`
    Named {
        name: String,
        generics: Vec<Type>,
        span: Span,
    },
    /// Tuple: `(A, B, C)`
    Tuple(Vec<Type>, Span),
    /// Array: `[T; N]`
    Array(Box<Type>, usize, Span),
    /// Slice: `[T]`
    Slice(Box<Type>, Span),
    /// Reference: `&T`, `&mut T`
    Ref {
        mutable: bool,
        inner: Box<Type>,
        lifetime: Option<String>,
        span: Span,
    },
    /// Function type: `fn(A, B) -> C with Effects`
    Function {
        params: Vec<Type>,
        return_type: Box<Type>,
        effects: Vec<Effect>,
        span: Span,
    },
    /// Option shorthand: `T?`
    Option(Box<Type>, Span),
    /// Result shorthand: `T!E`
    Result(Box<Type>, Box<Type>, Span),
    /// Inferred: `_`
    Infer(Span),
    /// Never type: `!`
    Never(Span),
    /// Unit type: `()`
    Unit(Span),
    /// impl Trait: `impl Future`, `impl Iterator<Item = T>`
    Impl {
        trait_name: String,
        generics: Vec<Type>,
        span: Span,
    },
}

impl Type {
    pub fn span(&self) -> Span {
        match self {
            Type::Named { span, .. }
            | Type::Tuple(_, span)
            | Type::Array(_, _, span)
            | Type::Slice(_, span)
            | Type::Ref { span, .. }
            | Type::Function { span, .. }
            | Type::Option(_, span)
            | Type::Result(_, _, span)
            | Type::Infer(span)
            | Type::Never(span)
            | Type::Unit(span)
            | Type::Impl { span, .. } => *span,
        }
    }
}

// === OTHER TOP-LEVEL ITEMS ===

#[derive(Debug, Clone)]
pub struct TypeAlias {
    pub name: String,
    pub generics: Vec<Generic>,
    pub target: Type,
    pub visibility: Visibility,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Use {
    pub path: Vec<String>,
    pub alias: Option<String>,
    pub glob: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Mod {
    pub name: String,
    pub inline: Option<Vec<Item>>,
    pub visibility: Visibility,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Const {
    pub name: String,
    pub ty: Type,
    pub value: Expr,
    pub visibility: Visibility,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ComptimeBlock {
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MacroDef {
    pub name: String,
    pub params: Vec<MacroParam>,
    pub body: MacroBody,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MacroParam {
    pub name: String,
    pub kind: MacroParamKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum MacroParamKind {
    Expr,
    Type,
    Ident,
    Block,
    Token,
    Repetition(Box<MacroParamKind>),
}

#[derive(Debug, Clone)]
pub enum MacroBody {
    Tokens(Vec<MacroToken>),
    Block(Block),
}

#[derive(Debug, Clone)]
pub struct MacroToken {
    pub content: String,
    pub span: Span,
}

// === EXPRESSIONS ===

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    /// `let pattern [: Type] = value`
    Let {
        pattern: Pattern,
        ty: Option<Type>,
        value: Option<Expr>,
        span: Span,
    },
    /// Expression statement
    Expr(Expr),
    /// `return [value]`
    Return(Option<Expr>, Span),
    /// `break [value]`
    Break(Option<Expr>, Span),
    /// `continue`
    Continue(Span),
    /// `for binding in iter: body`
    For {
        binding: Pattern,
        iter: Expr,
        body: Block,
        span: Span,
    },
    /// `while cond: body`
    While {
        condition: Expr,
        body: Block,
        span: Span,
    },
    /// `loop: body`
    Loop {
        body: Block,
        span: Span,
    },
    /// Item declaration (nested function, struct, etc.)
    Item(Box<Item>),
}

#[derive(Debug, Clone)]
pub enum Expr {
    /// Literals
    Int(i64, Span),
    Float(f64, Span),
    String(String, Span),
    FString(Vec<Expr>, Span),
    Bool(bool, Span),
    None(Span),
    
    /// Identifier
    Ident(String, Span),
    
    /// Macro call
    MacroCall {
        name: String,
        args: Vec<Expr>,
        span: Span,
    },
    
    /// Binary operation
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
        span: Span,
    },
    
    /// Unary operation
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
        span: Span,
    },
    
    /// Function call: `func(args)`
    Call {
        callee: Box<Expr>,
        args: Vec<CallArg>,
        span: Span,
    },
    
    /// Method call: `obj.method(args)`
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        args: Vec<CallArg>,
        span: Span,
    },
    
    /// Field access: `obj.field`
    Field {
        object: Box<Expr>,
        field: String,
        span: Span,
    },
    
    /// Index: `arr[i]`
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },

    /// Assignment: `target = value`
    Assign {
        target: Box<Expr>,
        value: Box<Expr>,
        span: Span,
    },
    
    /// Struct literal: `Point { x: 1, y: 2 }`
    Struct {
        name: String,
        fields: Vec<(String, Expr)>,
        span: Span,
    },

    EnumVariant {
        enum_name: String,
        variant: String,
        fields: EnumVariantFields,
        span: Span,
    },
    
    /// Array literal: `[1, 2, 3]`
    Array(Vec<Expr>, Span),
    
    /// Tuple literal: `(a, b, c)`
    Tuple(Vec<Expr>, Span),
    
    /// Range: `start..end`, `start..=end`
    Range {
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        inclusive: bool,
        span: Span,
    },
    
    /// If expression
    If {
        condition: Box<Expr>,
        then_branch: Block,
        else_branch: Option<Box<ElseBranch>>,
        span: Span,
    },
    
    /// Match expression
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
        span: Span,
    },
    
    /// Lambda: `|args| body` or `|args| -> Type: body`
    Lambda {
        params: Vec<Param>,
        return_type: Option<Type>,
        body: Box<Expr>,
        span: Span,
    },
    
    /// Reference: `&value`, `&mut value`
    Ref {
        mutable: bool,
        value: Box<Expr>,
        span: Span,
    },
    
    /// Dereference: `*ptr`
    Deref(Box<Expr>, Span),
    
    /// Cast: `value as Type`
    Cast {
        value: Box<Expr>,
        target: Type,
        span: Span,
    },
    
    /// Try: `expr?`
    Try(Box<Expr>, Span),
    
    /// Await: `await expr`
    Await(Box<Expr>, Span),
    
    /// Spawn actor: `spawn ActorName { state }`
    Spawn {
        actor: String,
        init: Vec<(String, Expr)>,
        span: Span,
    },
    
    /// Send message: `send target <- Message { data }`
    SendMsg {
        target: Box<Expr>,
        message: String,
        data: Vec<(String, Expr)>,
        span: Span,
    },
    
    /// Comptime expression: `comptime { expr }`
    Comptime(Box<Expr>, Span),
    
    /// Macro invocation: `name!(args)`
    // Already defined above, remove duplicate
    
    /// Block expression
    Block(Block, Span),

    /// JSX embedded in expression
    JSX(JSXNode, Span),
    
    /// Grouped expression: `(expr)`
    Paren(Box<Expr>, Span),
    
    /// Return expression: `return [expr]`
    Return(Option<Box<Expr>>, Span),
    
    /// Break expression: `break [expr]`
    Break(Option<Box<Expr>>, Span),
    
    /// Continue expression: `continue`
    Continue(Span),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Int(_, s)
            | Expr::Float(_, s)
            | Expr::String(_, s)
            | Expr::FString(_, s)
            | Expr::Bool(_, s)
            | Expr::None(s)
            | Expr::Ident(_, s)
            | Expr::Binary { span: s, .. }
            | Expr::Unary { span: s, .. }
            | Expr::Call { span: s, .. }
            | Expr::MethodCall { span: s, .. }
            | Expr::Field { span: s, .. }
            | Expr::Index { span: s, .. }
            | Expr::Struct { span: s, .. }
            | Expr::EnumVariant { span: s, .. }
            | Expr::Array(_, s)
            | Expr::Tuple(_, s)
            | Expr::Range { span: s, .. }
            | Expr::If { span: s, .. }
            | Expr::Match { span: s, .. }
            | Expr::Lambda { span: s, .. }
            | Expr::Ref { span: s, .. }
            | Expr::Deref(_, s)
            | Expr::Cast { span: s, .. }
            | Expr::Try(_, s)
            | Expr::Await(_, s)
            | Expr::Spawn { span: s, .. }
            | Expr::SendMsg { span: s, .. }
            | Expr::Comptime(_, s)
            | Expr::MacroCall { span: s, .. }
            | Expr::Block(_, s)
            | Expr::JSX(_, s)
            | Expr::Assign { span: s, .. }
            | Expr::Paren(_, s)
            | Expr::Return(_, s)
            | Expr::Break(_, s)
            | Expr::Continue(s) => *s,
        }
    }
}

#[derive(Debug, Clone)]
pub enum EnumVariantFields {
    Unit,
    Tuple(Vec<Expr>),
    Struct(Vec<(String, Expr)>),
}

#[derive(Debug, Clone)]
pub struct CallArg {
    pub name: Option<String>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ElseBranch {
    Else(Block),
    ElseIf(Box<Expr>, Block, Option<Box<ElseBranch>>),
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    /// Wildcard: `_`
    Wildcard(Span),
    /// Literal: `1`, `"hello"`, `true`
    Literal(Expr),
    /// Binding: `x`, `mut x`
    Binding {
        name: String,
        mutable: bool,
        span: Span,
    },
    /// Struct: `Point { x, y }`
    Struct {
        name: String,
        fields: Vec<(String, Pattern)>,
        rest: bool,
        span: Span,
    },
    /// Tuple: `(a, b, c)`
    Tuple(Vec<Pattern>, Span),
    /// Enum variant: `Some(x)`, `None`
    Variant {
        enum_name: Option<String>,
        variant: String,
        fields: VariantPatternFields,
        span: Span,
    },
    /// Array/Slice: `[first, rest @ ..]`
    Slice {
        patterns: Vec<Pattern>,
        rest: Option<String>,
        span: Span,
    },
    /// Or pattern: `A | B`
    Or(Vec<Pattern>, Span),
    /// Range: `1..10`
    Range {
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        inclusive: bool,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub enum VariantPatternFields {
    Unit,
    Tuple(Vec<Pattern>),
    Struct(Vec<(String, Pattern)>),
}

// === OPERATORS ===

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    
    // Comparison
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    
    // Logical
    And,
    Or,
    
    // Bitwise
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    
    // Assignment
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    
    // Range
    Range,
    RangeInclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
    Ref,
    RefMut,
    Deref,
}

// === VISIBILITY ===

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Visibility {
    #[default]
    Private,
    Public,
    Crate,
    Super,
}

