//! KORE Type System - Rust-like with effect tracking

use crate::ast::*;
use crate::effects::EffectSet;
use crate::span::Span;
use crate::error::{KoreError, KoreResult};
use std::collections::HashMap;

/// Type-checked AST node
#[derive(Debug, Clone)]
pub struct TypedProgram {
    pub items: Vec<TypedItem>,
}

// Comptime blocks should be empty/removed by now if fully evaluated, or we check them
#[derive(Debug, Clone)]
pub struct TypedComptime {
    pub ast: Block,
}

#[derive(Debug, Clone)]
pub struct TypedConst {
    pub ast: Const,
    pub ty: ResolvedType,
}

#[derive(Debug, Clone)]
pub struct TypedUse {
    pub ast: Use,
}

#[derive(Debug, Clone)]
pub enum TypedItem {
    Function(TypedFunction),
    Component(TypedComponent),
    Shader(TypedShader),
    Actor(TypedActor),
    Struct(TypedStruct),
    Enum(TypedEnum),
    Comptime(TypedComptime),
    Const(TypedConst),
    Macro(TypedMacro),
    Use(TypedUse),
    Impl(TypedImpl),
    Test(TypedTest),
}

#[derive(Debug, Clone)]
pub struct TypedTest {
    pub ast: TestDef,
}

#[derive(Debug, Clone)]
pub struct TypedImpl {
    pub ast: Impl,
}

#[derive(Debug, Clone)]
pub struct TypedMacro {
    pub ast: MacroDef,
}

#[derive(Debug, Clone)]
pub struct TypedActor {
    pub ast: Actor,
    pub state_types: HashMap<String, ResolvedType>,
}

#[derive(Debug, Clone)]
pub struct TypedFunction {
    pub ast: Function,
    pub resolved_type: ResolvedType,
    pub effects: EffectSet,
}

#[derive(Debug, Clone)]
pub struct TypedComponent {
    pub ast: Component,
    pub prop_types: HashMap<String, ResolvedType>,
}

#[derive(Debug, Clone)]
pub struct TypedShader {
    pub ast: Shader,
    pub input_types: Vec<ResolvedType>,
    pub output_type: ResolvedType,
}

#[derive(Debug, Clone)]
pub struct TypedStruct {
    pub ast: Struct,
    pub field_types: HashMap<String, ResolvedType>,
}

#[derive(Debug, Clone)]
pub struct TypedEnum {
    pub ast: Enum,
    pub variant_payload_types: HashMap<String, Vec<ResolvedType>>,
}

/// Fully resolved type
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedType {
    Unit,
    Bool,
    Int(IntSize),
    Float(FloatSize),
    String,
    Char,
    Array(Box<ResolvedType>, usize),
    Slice(Box<ResolvedType>),
    Tuple(Vec<ResolvedType>),
    Option(Box<ResolvedType>),
    Result(Box<ResolvedType>, Box<ResolvedType>),
    Ref { mutable: bool, inner: Box<ResolvedType> },
    Function { params: Vec<ResolvedType>, ret: Box<ResolvedType>, effects: EffectSet },
    Struct(String, HashMap<String, ResolvedType>),
    Enum(String, Vec<(String, ResolvedType)>),
    Generic(String),
    Never,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntSize { I8, I16, I32, I64, I128, Isize, U8, U16, U32, U64, U128, Usize }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatSize { F32, F64 }

/// Type environment for checking
pub struct TypeEnv {
    scopes: Vec<HashMap<String, ResolvedType>>,
    types: HashMap<String, ResolvedType>,
}

impl TypeEnv {
    pub fn new() -> Self {
        let mut env = Self { scopes: vec![HashMap::new()], types: HashMap::new() };
        // Built-in types
        env.types.insert("Int".into(), ResolvedType::Int(IntSize::I64));
        env.types.insert("Float".into(), ResolvedType::Float(FloatSize::F64));
        env.types.insert("Bool".into(), ResolvedType::Bool);
        env.types.insert("String".into(), ResolvedType::String);
        env.types.insert("Vec2".into(), ResolvedType::Tuple(vec![
            ResolvedType::Float(FloatSize::F32),
            ResolvedType::Float(FloatSize::F32),
        ]));
        env.types.insert("Vec3".into(), ResolvedType::Tuple(vec![
            ResolvedType::Float(FloatSize::F32),
            ResolvedType::Float(FloatSize::F32),
            ResolvedType::Float(FloatSize::F32),
        ]));
        env
    }

    pub fn push_scope(&mut self) { self.scopes.push(HashMap::new()); }
    pub fn pop_scope(&mut self) { self.scopes.pop(); }
    
    pub fn define(&mut self, name: String, ty: ResolvedType) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, ty);
        }
    }
    
    pub fn lookup(&self, name: &str) -> Option<&ResolvedType> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) { return Some(ty); }
        }
        self.types.get(name)
    }
}

/// Main type checking entry point
pub fn check(program: &Program) -> KoreResult<TypedProgram> {
    let mut env = TypeEnv::new();
    let mut typed_items = Vec::new();
    
    for item in &program.items {
        typed_items.push(check_item(&mut env, item)?);
    }
    
    Ok(TypedProgram { items: typed_items })
}

fn check_item(env: &mut TypeEnv, item: &Item) -> KoreResult<TypedItem> {
    match item {
        Item::Function(f) => Ok(TypedItem::Function(check_function(env, f)?)),
        Item::Struct(s) => Ok(TypedItem::Struct(check_struct(env, s)?)),
        Item::Enum(e) => Ok(TypedItem::Enum(check_enum(env, e)?)),
        Item::Component(c) => Ok(TypedItem::Component(check_component(env, c)?)),
        Item::Shader(s) => Ok(TypedItem::Shader(check_shader(env, s)?)),
        Item::Actor(a) => Ok(TypedItem::Actor(check_actor(env, a)?)),
        Item::Comptime(b) => Ok(TypedItem::Comptime(TypedComptime { ast: b.body.clone() })),
        Item::Const(c) => Ok(TypedItem::Const(check_const(env, c)?)),
        Item::Macro(m) => Ok(TypedItem::Macro(TypedMacro { ast: m.clone() })),
        Item::Use(u) => Ok(TypedItem::Use(TypedUse { ast: u.clone() })),
        Item::Impl(i) => Ok(TypedItem::Impl(TypedImpl { ast: i.clone() })),
        Item::Test(t) => Ok(TypedItem::Test(TypedTest { ast: t.clone() })),
        _ => {
            // For now, ignore other items or provide dummy implementation
            // Since we are running in interpreter mode mostly, types are just for checking.
            // But we shouldn't fail hard if we encounter valid syntax.
            // Let's return a dummy TypedItem if possible, or just skip it?
            // TypedItem enum doesn't have a variant for others.
            // We should probably just return a dummy function or error with a better message?
            // Or better, expand TypedItem to include other items.
            // For now, let's just error if it's something we really don't support yet,
            // but for simple scripts, we likely only need the above.
            // If the script has top-level stmts, they are wrapped in main function.
            // So we are good.
            Err(KoreError::type_error("Item type not yet supported in type checker", item_span(item)))
        }
    }
}

fn check_const(_env: &mut TypeEnv, c: &Const) -> KoreResult<TypedConst> {
    let ty = resolve_type(&c.ty)?;
    // TODO: Check if value matches type
    Ok(TypedConst { ast: c.clone(), ty })
}

fn check_actor(_env: &mut TypeEnv, a: &Actor) -> KoreResult<TypedActor> {
    let mut state_types = HashMap::new();
    for s in &a.state {
        state_types.insert(s.name.clone(), resolve_type(&s.ty)?);
    }
    Ok(TypedActor { ast: a.clone(), state_types })
}

fn check_function(env: &mut TypeEnv, f: &Function) -> KoreResult<TypedFunction> {
    env.push_scope();
    let mut param_types = Vec::new();
    for p in &f.params {
        let ty = resolve_type(&p.ty)?;
        env.define(p.name.clone(), ty.clone());
        param_types.push(ty);
    }
    let ret = f.return_type.as_ref().map(|t| resolve_type(t)).transpose()?.unwrap_or(ResolvedType::Unit);
    let effects = EffectSet::from(f.effects.clone());
    env.pop_scope();
    
    Ok(TypedFunction {
        ast: f.clone(),
        resolved_type: ResolvedType::Function { params: param_types, ret: Box::new(ret), effects: effects.clone() },
        effects,
    })
}

fn check_struct(_env: &mut TypeEnv, s: &Struct) -> KoreResult<TypedStruct> {
    let mut fields = HashMap::new();
    for f in &s.fields {
        fields.insert(f.name.clone(), resolve_type(&f.ty)?);
    }
    Ok(TypedStruct { ast: s.clone(), field_types: fields })
}

fn check_enum(_env: &mut TypeEnv, e: &Enum) -> KoreResult<TypedEnum> {
    let mut variant_payload_types: HashMap<String, Vec<ResolvedType>> = HashMap::new();

    for v in &e.variants {
        let payload_types = match &v.fields {
            VariantFields::Unit => Vec::new(),
            VariantFields::Tuple(items) => items.iter().map(resolve_type).collect::<Result<Vec<_>, _>>()?,
            VariantFields::Struct(fields) => fields.iter().map(|f| resolve_type(&f.ty)).collect::<Result<Vec<_>, _>>()?,
        };
        variant_payload_types.insert(v.name.clone(), payload_types);
    }

    Ok(TypedEnum {
        ast: e.clone(),
        variant_payload_types,
    })
}

fn check_component(_env: &mut TypeEnv, c: &Component) -> KoreResult<TypedComponent> {
    let mut props = HashMap::new();
    for p in &c.props {
        props.insert(p.name.clone(), resolve_type(&p.ty)?);
    }
    Ok(TypedComponent { ast: c.clone(), prop_types: props })
}

fn check_shader(_env: &mut TypeEnv, s: &Shader) -> KoreResult<TypedShader> {
    let inputs: Vec<_> = s.inputs.iter().map(|p| resolve_type(&p.ty)).collect::<Result<_, _>>()?;
    let output = resolve_type(&s.outputs)?;
    Ok(TypedShader { ast: s.clone(), input_types: inputs, output_type: output })
}

pub fn resolve_type(ty: &Type) -> KoreResult<ResolvedType> {
    match ty {
        Type::Named { name, .. } => match name.as_str() {
            "Int" => Ok(ResolvedType::Int(IntSize::I64)),
            "Float" => Ok(ResolvedType::Float(FloatSize::F64)),
            "Bool" => Ok(ResolvedType::Bool),
            "String" => Ok(ResolvedType::String),
            _ => {
                // Check if this is a generic type parameter (single uppercase letter or _T style)
                if name.len() == 1 && name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    Ok(ResolvedType::Generic(name.clone()))
                } else if name.starts_with('_') && name.len() > 1 {
                    // _T, _Item, etc are also generic
                    Ok(ResolvedType::Generic(name.clone()))
                } else {
                    // Assume it's a struct
                    Ok(ResolvedType::Struct(name.clone(), HashMap::new()))
                }
            }
        },
        Type::Unit(_) => Ok(ResolvedType::Unit),
        Type::Never(_) => Ok(ResolvedType::Never),
        Type::Tuple(inner, _) => Ok(ResolvedType::Tuple(inner.iter().map(resolve_type).collect::<Result<_, _>>()?)),
        Type::Function { params, return_type, effects, .. } => {
            let resolved_params = params.iter().map(resolve_type).collect::<Result<Vec<_>, _>>()?;
            let resolved_ret = resolve_type(return_type)?;
            Ok(ResolvedType::Function {
                params: resolved_params,
                ret: Box::new(resolved_ret),
                effects: EffectSet::from(effects.clone()),
            })
        }
        _ => Ok(ResolvedType::Unknown),
    }
}

fn item_span(item: &Item) -> Span {
    match item {
        Item::Function(f) => f.span,
        Item::Struct(s) => s.span,
        Item::Enum(e) => e.span,
        Item::Component(c) => c.span,
        Item::Shader(s) => s.span,
        Item::Actor(a) => a.span,
        Item::Comptime(b) => b.span,
        Item::Const(c) => c.span,
        Item::Macro(m) => m.span,
        Item::Use(u) => u.span,
        Item::Impl(i) => i.span,
        Item::Test(t) => t.span,
        _ => Span::new(0, 0),
    }
}

impl From<Vec<crate::effects::Effect>> for EffectSet {
    fn from(v: Vec<crate::effects::Effect>) -> Self {
        let mut s = EffectSet::new();
        for e in v { s.effects.insert(e); }
        s
    }
}

