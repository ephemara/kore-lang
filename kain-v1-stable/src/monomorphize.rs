use crate::types::*;
use crate::ast::*;
use crate::error::{KainResult, KainError};
use std::collections::{HashMap, HashSet};

/// Result of monomorphization
pub struct MonomorphizedProgram {
    pub items: Vec<TypedItem>,
}

pub fn monomorphize(program: &TypedProgram) -> KainResult<MonomorphizedProgram> {
    let mut ctx = MonoContext::new();
    

    
    // 1. First Pass: Collect all global items: functions, impls
    for item in &program.items {
        match item {
            TypedItem::Function(func) => {

                if !func.ast.generics.is_empty() {
                    ctx.generic_functions.insert(func.ast.name.clone(), func.clone());

                } else {
                    ctx.concrete_items.push(item.clone());

                }
            }
            TypedItem::Struct(s) => {
                let mut fields = HashMap::new();
                for f in &s.ast.fields {
                    if let Ok(ty) = resolve_ast_type(&f.ty) {
                        fields.insert(f.name.clone(), ty);
                    }
                }
                ctx.structs.insert(s.ast.name.clone(), fields);
                ctx.concrete_items.push(item.clone());
            }
            TypedItem::Impl(imp) => {
                // Register methods from impl blocks
                // Mangle them as Type_method
                let type_name = match &imp.ast.target_type {
                    Type::Named { name, .. } => name.clone(),
                    _ => continue, // Skip complex types for now
                };
                
                let target_ty = resolve_ast_type(&imp.ast.target_type).unwrap_or(ResolvedType::Unknown);
                
                // Register trait implementation
                if let Some(trait_name) = &imp.ast.trait_name {
                    let type_name_str = type_to_string(&target_ty);
                    ctx.trait_impls.insert((trait_name.clone(), type_name_str));
                }
                
                for method in &imp.ast.methods {
                    let mangled_name = format!("{}_{}", type_name, method.name);
                    
                    let mut standalone_fn = method.clone();
                    standalone_fn.name = mangled_name.clone();
                    
                    // Resolve method type
                    let mut params = Vec::new();
                    for p in &method.params {
                         if p.name == "self" {
                             params.push(target_ty.clone());
                         } else {
                             params.push(resolve_ast_type(&p.ty).unwrap_or(ResolvedType::Unknown));
                         }
                    }
                    let ret = method.return_type.as_ref()
                        .map(|t| resolve_ast_type(t).unwrap_or(ResolvedType::Unknown))
                        .unwrap_or(ResolvedType::Unit);
                    
                    let method_ty = ResolvedType::Function {
                        params,
                        ret: Box::new(ret),
                        effects: crate::effects::EffectSet::new(), // Todo scan effects?
                    };
                    
                    let typed_method = TypedFunction {
                        ast: standalone_fn,
                        resolved_type: method_ty,
                        effects: crate::effects::EffectSet::new(),
                    };
                    
                    ctx.methods.entry(type_name.clone()).or_default().insert(method.name.clone(), mangled_name.clone());
                    ctx.concrete_items.push(TypedItem::Function(typed_method));
                }
            }
            _ => {
                ctx.concrete_items.push(item.clone());
            }
        }
    }
    
    // 2. Scan concrete items for calls
    let mut i = 0;
    while i < ctx.concrete_items.len() {
        let item = ctx.concrete_items[i].clone();
        match item {
            TypedItem::Function(func) => {
                // Check if Async
                if func.effects.effects.contains(&crate::effects::Effect::Async) {
                    // Lower Async Function to State Machine
                    // This returns the transformed entry function (synchronous, returns Future struct)
                    // The State Machine Struct and Poll Function are pushed to ctx.concrete_items inside lower_async_fn
                    let entry_fn = lower_async_fn(&mut ctx, &func)?;
                    
                    // Replace the original async function with the transformed entry function
                    ctx.concrete_items[i] = TypedItem::Function(entry_fn);
                } else {
                    let new_func = scan_function(&mut ctx, &func)?;
                    ctx.concrete_items[i] = TypedItem::Function(new_func);
                }
            }
            _ => {}
        }
        i += 1;
    }
    
    Ok(MonomorphizedProgram { items: ctx.concrete_items })
}

struct MonoContext {
    generic_functions: HashMap<String, TypedFunction>,
    concrete_items: Vec<TypedItem>,
    instantiated: HashMap<String, String>,
    /// Type -> MethodName -> MangledName
    methods: HashMap<String, HashMap<String, String>>,
    /// Struct Name -> Field Name -> Type
    structs: HashMap<String, HashMap<String, ResolvedType>>,
    /// (TraitName, TypeName) -> Implemented
    trait_impls: HashSet<(String, String)>,
}

impl MonoContext {
    fn new() -> Self {
        Self {
            generic_functions: HashMap::new(),
            concrete_items: Vec::new(),
            instantiated: HashMap::new(),
            methods: HashMap::new(),
            structs: HashMap::new(),
            trait_impls: HashSet::new(),
        }
    }
    
    fn instantiate(&mut self, name: &str, type_args: &[ResolvedType]) -> KainResult<String> {
        let mangled_name = format!("{}_{}", name, mangle_types(type_args));
        
        if self.instantiated.contains_key(&mangled_name) {
            return Ok(mangled_name);
        }
        
        let generic_func = self.generic_functions.get(name)
            .ok_or_else(|| KainError::type_error(format!("Generic function {} not found", name), crate::span::Span::new(0,0)))?
            .clone();
            
        if generic_func.ast.generics.len() != type_args.len() {
             return Err(KainError::type_error(format!("Generic arg count mismatch for {}: expected {}, got {}", name, generic_func.ast.generics.len(), type_args.len()), generic_func.ast.span));
        }
        
        let mut mapping = HashMap::new();
        for (i, param) in generic_func.ast.generics.iter().enumerate() {
            mapping.insert(param.name.clone(), type_args[i].clone());
        }
        
        let mut new_func = generic_func.clone();
        new_func.ast.name = mangled_name.clone();
        new_func.ast.generics.clear();
        
        if let ResolvedType::Function { params, ret, .. } = &mut new_func.resolved_type {
            for p in params {
                *p = substitute_type(p, &mapping);
            }
            *ret = Box::new(substitute_type(&ret, &mapping));
        }
        
        self.instantiated.insert(mangled_name.clone(), mangled_name.clone());
        
        substitute_ast_types(&mut new_func.ast, &mapping);
        self.concrete_items.push(TypedItem::Function(new_func));
        
        Ok(mangled_name)
    }
}

fn type_to_string(ty: &ResolvedType) -> String {
    match ty {
        ResolvedType::Int(_) => "Int".to_string(),
        ResolvedType::Float(_) => "Float".to_string(),
        ResolvedType::String => "String".to_string(),
        ResolvedType::Bool => "Bool".to_string(),
        ResolvedType::Unit => "Unit".to_string(),
        ResolvedType::Struct(n, _) => n.clone(),
        ResolvedType::Enum(n, _) => n.clone(),
        ResolvedType::Tuple(ts) => format!("({})", ts.iter().map(type_to_string).collect::<Vec<_>>().join(", ")),
        _ => "Any".to_string(),
    }
}

fn mangle_types(types: &[ResolvedType]) -> String {
    types.iter().map(type_to_string).collect::<Vec<_>>().join("_")
}

fn resolve_ast_type(ty: &Type) -> KainResult<ResolvedType> {
    crate::types::resolve_type(ty)
}

/// Unify a parameter type with an argument type to extract generic bindings.
/// For example, unifying `fn(T) -> T` with `fn(Int) -> Int` yields `{T: Int}`.
fn unify(
    param_type: &ResolvedType,
    arg_type: &ResolvedType,
    bindings: &mut HashMap<String, ResolvedType>,
) {
    match (param_type, arg_type) {
        // If the parameter type is a generic, bind it to the argument type
        (ResolvedType::Generic(name), concrete) => {
            if let Some(existing) = bindings.get(name) {
                // Already bound - ideally check for consistency, but for now just keep first binding
                let _ = existing;
            } else {
                bindings.insert(name.clone(), concrete.clone());
            }
        }
        
        // Recursively unify function types
        (ResolvedType::Function { params: p_params, ret: p_ret, .. }, 
         ResolvedType::Function { params: a_params, ret: a_ret, .. }) => {
            // Unify parameter types
            for (pp, ap) in p_params.iter().zip(a_params.iter()) {
                unify(pp, ap, bindings);
            }
            // Unify return type
            unify(p_ret, a_ret, bindings);
        }
        
        // Recursively unify array types
        (ResolvedType::Array(p_inner, _), ResolvedType::Array(a_inner, _)) => {
            unify(p_inner, a_inner, bindings);
        }
        
        // Recursively unify tuple types
        (ResolvedType::Tuple(p_elems), ResolvedType::Tuple(a_elems)) => {
            for (pe, ae) in p_elems.iter().zip(a_elems.iter()) {
                unify(pe, ae, bindings);
            }
        }
        
        // For concrete types that match, nothing to unify
        _ => {}
    }
}

/// Infer type arguments for a generic function call by unifying parameter types with argument types.
fn infer_type_args(
    ctx: &MonoContext,
    generic_func: &TypedFunction,
    arg_types: &[ResolvedType],
) -> KainResult<Vec<ResolvedType>> {
    let mut bindings: HashMap<String, ResolvedType> = HashMap::new();
    
    // Get the parameter types from the function signature
    let param_types: Vec<ResolvedType> = if let ResolvedType::Function { params, .. } = &generic_func.resolved_type {
        params.clone()
    } else {
        // Fallback: resolve from AST
        generic_func.ast.params.iter()
            .map(|p| resolve_ast_type(&p.ty).unwrap_or(ResolvedType::Unknown))
            .collect()
    };
    
    // Unify each parameter type with the corresponding argument type
    for (param_ty, arg_ty) in param_types.iter().zip(arg_types.iter()) {
        unify(param_ty, arg_ty, &mut bindings);
    }
    
    // Extract the inferred types in the order of the generic parameters
    let mut inferred = Vec::new();
    for generic in &generic_func.ast.generics {
        if let Some(ty) = bindings.get(&generic.name) {
             // Check Bounds!
             for bound in &generic.bounds {
                 let type_name = type_to_string(ty);
                 if !ctx.trait_impls.contains(&(bound.trait_name.clone(), type_name.clone())) {
                     return Err(KainError::type_error(
                         format!("Type '{}' does not satisfy bound '{}'", type_name, bound.trait_name), 
                         generic.span
                     ));
                 }
             }
             inferred.push(ty.clone());
        } else {
            // Generic wasn't inferred - could be an error, but let's use Unknown for now
            inferred.push(ResolvedType::Unknown);
        }
    }
    
    Ok(inferred)
}

fn substitute_type(ty: &ResolvedType, mapping: &HashMap<String, ResolvedType>) -> ResolvedType {
    match ty {
        ResolvedType::Generic(name) => mapping.get(name).cloned().unwrap_or(ty.clone()),
        ResolvedType::Function { params, ret, effects } => {
            ResolvedType::Function {
                params: params.iter().map(|p| substitute_type(p, mapping)).collect(),
                ret: Box::new(substitute_type(ret, mapping)),
                effects: effects.clone()
            }
        }
        ResolvedType::Array(inner, n) => ResolvedType::Array(Box::new(substitute_type(inner, mapping)), *n),
        _ => ty.clone() 
    }
}

fn substitute_ast_types(func: &mut Function, mapping: &HashMap<String, ResolvedType>) {
    // 1. Substitute param types
    for param in &mut func.params {
        substitute_type_ast(&mut param.ty, mapping);
    }
    
    // 2. Substitute return type
    if let Some(ret) = &mut func.return_type {
        substitute_type_ast(ret, mapping);
    }
    
    // 3. Substitute body
    substitute_block(&mut func.body, mapping);
}

fn substitute_block(block: &mut Block, mapping: &HashMap<String, ResolvedType>) {
    for stmt in &mut block.stmts {
        substitute_stmt(stmt, mapping);
    }
}

fn substitute_stmt(stmt: &mut Stmt, mapping: &HashMap<String, ResolvedType>) {
    match stmt {
        Stmt::Let { ty, value, .. } => {
            if let Some(t) = ty {
                substitute_type_ast(t, mapping);
            }
            if let Some(v) = value {
                substitute_expr(v, mapping);
            }
        }
        Stmt::Expr(e) => substitute_expr(e, mapping),
        Stmt::Return(Some(e), _) => substitute_expr(e, mapping),
        Stmt::For { iter, body, .. } => {
            substitute_expr(iter, mapping);
            substitute_block(body, mapping);
        }
        Stmt::While { condition, body, .. } => {
            substitute_expr(condition, mapping);
            substitute_block(body, mapping);
        }
        _ => {}
    }
}

fn substitute_expr(expr: &mut Expr, mapping: &HashMap<String, ResolvedType>) {
    match expr {
        Expr::Cast { value, target, .. } => {
            substitute_expr(value, mapping);
            substitute_type_ast(target, mapping);
        }
        Expr::Binary { left, right, .. } => {
            substitute_expr(left, mapping);
            substitute_expr(right, mapping);
        }
        Expr::Unary { operand, .. } => substitute_expr(operand, mapping),
        Expr::Call { callee, args, .. } => {
            substitute_expr(callee, mapping);
            for arg in args {
                substitute_expr(&mut arg.value, mapping);
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            substitute_expr(receiver, mapping);
            for arg in args {
                substitute_expr(&mut arg.value, mapping);
            }
        }
        Expr::Field { object, .. } => {
            substitute_expr(object, mapping);
        }
        Expr::Index { object, index, .. } => {
            substitute_expr(object, mapping);
            substitute_expr(index, mapping);
        }
        Expr::Struct { fields, .. } => {
             for (_, v) in fields {
                 substitute_expr(v, mapping);
             }
        }
        Expr::Array(items, _) => {
             for item in items {
                 substitute_expr(item, mapping);
             }
        }
        Expr::Tuple(items, _) => {
             for item in items {
                 substitute_expr(item, mapping);
             }
        }
        Expr::Block(b, _) => substitute_block(b, mapping),
        Expr::If { condition, then_branch, else_branch, .. } => {
             substitute_expr(condition, mapping);
             substitute_block(then_branch, mapping);
             if let Some(br) = else_branch {
                 match br.as_mut() {
                     ElseBranch::Else(b) => substitute_block(b, mapping),
                     ElseBranch::ElseIf(c, t, _) => { // Simplified recursion
                         substitute_expr(c, mapping);
                         substitute_block(t, mapping);
                     }
                 }
             }
        }
        Expr::Match { scrutinee, arms, .. } => {
            substitute_expr(scrutinee, mapping);
            for arm in arms {
                substitute_expr(&mut arm.body, mapping);
            }
        }
        Expr::Lambda { params, body, return_type, .. } => {
             for p in params {
                 substitute_type_ast(&mut p.ty, mapping);
             }
             if let Some(ret) = return_type {
                 substitute_type_ast(ret, mapping);
             }
             substitute_expr(body, mapping);
        }
        Expr::Await(inner, _) => {
            substitute_expr(inner, mapping);
        }
        _ => {}
    }
}

fn substitute_type_ast(ty: &mut Type, mapping: &HashMap<String, ResolvedType>) {
    match ty {
        Type::Named { name, generics, .. } => {
            if let Some(concrete) = mapping.get(name) {
                *ty = resolved_to_ast_type(concrete, ty.span());
            } else {
                for g in generics {
                    substitute_type_ast(g, mapping);
                }
            }
        }
        Type::Tuple(types, _) => {
            for t in types {
                substitute_type_ast(t, mapping);
            }
        }
        Type::Function { params, return_type, .. } => {
            for p in params {
                substitute_type_ast(p, mapping);
            }
            substitute_type_ast(return_type, mapping);
        }
        Type::Array(inner, _, _) => {
             substitute_type_ast(inner, mapping);
         }
         Type::Slice(inner, _) => {
             substitute_type_ast(inner, mapping);
         }
         _ => {}
    }
}

fn resolved_to_ast_type(res: &ResolvedType, span: crate::span::Span) -> Type {
    match res {
        ResolvedType::Int(_) => Type::Named { name: "Int".into(), generics: vec![], span },
        ResolvedType::Float(_) => Type::Named { name: "Float".into(), generics: vec![], span },
        ResolvedType::Bool => Type::Named { name: "Bool".into(), generics: vec![], span },
        ResolvedType::String => Type::Named { name: "String".into(), generics: vec![], span },
        ResolvedType::Unit => Type::Unit(span),
        ResolvedType::Struct(n, _) => Type::Named { name: n.clone(), generics: vec![], span },
        _ => Type::Named { name: "Any".into(), generics: vec![], span }, // Fallback
    }
}

struct MonoTypeEnv {
    scopes: Vec<HashMap<String, ResolvedType>>,
}

impl MonoTypeEnv {
    fn new() -> Self {
        Self { scopes: vec![HashMap::new()] }
    }
    fn push(&mut self) { self.scopes.push(HashMap::new()); }
    fn pop(&mut self) { self.scopes.pop(); }
    
    fn define(&mut self, name: String, ty: ResolvedType) {
        if let Some(s) = self.scopes.last_mut() {
            s.insert(name, ty);
        }
    }
    
    fn get(&self, name: &str) -> ResolvedType {
        for s in self.scopes.iter().rev() {
            if let Some(t) = s.get(name) { return t.clone(); }
        }
        ResolvedType::Unknown
    }
}

fn lower_async_fn(ctx: &mut MonoContext, func: &TypedFunction) -> KainResult<TypedFunction> {
    let state_machine_name = format!("{}_Future", func.ast.name);
    
    // 1. Create State Machine Struct
    // struct MyFn_Future { state: Int, ...args, ...locals }
    let mut fields = HashMap::new();
    fields.insert("state".to_string(), ResolvedType::Int(IntSize::I64));
    
    // Capture arguments
    for param in &func.ast.params {
        fields.insert(param.name.clone(), resolve_ast_type(&param.ty).unwrap_or(ResolvedType::Unknown));
    }
    
    // Capture locals (lifted to struct fields)
    let locals = collect_locals(&func.ast.body);
    for (name, ty) in locals {
        fields.entry(name).or_insert(ty);
    }
    
    let _struct_ty = ResolvedType::Struct(state_machine_name.clone(), fields.clone());
    
    // Register Struct
    ctx.structs.insert(state_machine_name.clone(), fields.clone());
    
    // Emit Struct Definition
    // We need to create a TypedStruct and push it
    let struct_def = TypedItem::Struct(TypedStruct {
        ast: Struct {
            name: state_machine_name.clone(),
            generics: vec![],
            fields: fields.iter().map(|(n, t)| Field {
                name: n.clone(),
                ty: resolved_to_ast_type(t, func.ast.span),
                visibility: Visibility::Public,
                default: None,
                weak: false,
                span: func.ast.span
            }).collect(),
            visibility: Visibility::Public,
            span: func.ast.span,
        },
        field_types: fields.clone(),
    });
    ctx.concrete_items.push(struct_def);
    
    // 2. Generate Poll Function
    // fn MyFn_Future_poll(self: &mut MyFn_Future) -> Poll<T>
    let poll_name = format!("{}_poll", state_machine_name);
    
    // Create 'self' param
    let self_type = ResolvedType::Struct(state_machine_name.clone(), fields.clone());
    let self_param = Param {
        name: "self".to_string(),
        ty: resolved_to_ast_type(&self_type, func.ast.span),
        mutable: true,
        default: None,
        span: func.ast.span,
    };
    
    // === AWAIT CHOPPING: Split function body at await points ===
    
    // Step 1: Collect all await points and statements between them
    let await_points = collect_await_points(&func.ast.body);
    
    // Step 2: Add storage fields for each await's pending future and its result
    for (i, _) in await_points.iter().enumerate() {
        let field_name = format!("_await_{}", i);
        // Store futures as Unknown type (dynamic typing for interpreter)
        fields.insert(field_name, ResolvedType::Unknown);
        
        // Store result of the future
        let res_name = format!("_await_{}_result", i);
        fields.insert(res_name, ResolvedType::Unknown);
    }
    
    // Update struct with new fields
    ctx.structs.insert(state_machine_name.clone(), fields.clone());
    
    // Step 3: Generate match arms for each state
    let mut arms = Vec::new();
    
    if await_points.is_empty() {
        // No awaits - just execute the whole body in state 0 and return Ready
        let mut rewritten_body = func.ast.body.clone();
        rewrite_access_to_self(&mut rewritten_body, &fields);
        
        // Wrap result in Poll::Ready
        let body_with_ready = wrap_return_in_poll_ready(rewritten_body, func.ast.span);
        
        let arm0 = MatchArm {
            pattern: Pattern::Literal(Expr::Int(0, func.ast.span)),
            guard: None,
            body: body_with_ready,
            span: func.ast.span,
        };
        arms.push(arm0);
    } else {
        // Has awaits - generate state machine
        let segments = split_at_awaits(&func.ast.body, &await_points);
        
        for (state_idx, segment) in segments.iter().enumerate() {
            let arm = generate_state_arm(
                state_idx,
                segment,
                &await_points,
                &fields,
                &state_machine_name,
                func.ast.span,
            );
            arms.push(arm);
        }
    }
    
    // Fallback arm for completed/invalid states
    let arm_wild = MatchArm {
        pattern: Pattern::Wildcard(func.ast.span),
        guard: None,
        body: Expr::Call {
            callee: Box::new(Expr::Ident("panic".to_string(), func.ast.span)),
            args: vec![CallArg {
                name: None,
                value: Expr::String("polled after completion".to_string(), func.ast.span),
                span: func.ast.span,
            }],
            span: func.ast.span,
        },
        span: func.ast.span,
    };
    arms.push(arm_wild);
    
    // Create poll body with the match expression
    let mut poll_body = Block { stmts: vec![], span: func.ast.span };
    
    let match_expr = Expr::Match {
        scrutinee: Box::new(Expr::Field {
            object: Box::new(Expr::Ident("self".to_string(), func.ast.span)),
            field: "state".to_string(),
            span: func.ast.span
        }),
        arms,
        span: func.ast.span,
    };
    
    poll_body.stmts.push(Stmt::Expr(match_expr));
    
    let poll_fn = TypedItem::Function(TypedFunction {
        ast: Function {
            name: poll_name.clone(),
            generics: vec![],
            params: vec![self_param],
            return_type: None, // Should be Poll<T>
            effects: vec![],
            body: poll_body,
            visibility: Visibility::Public,
            attributes: vec![],
            span: func.ast.span,
        },
        resolved_type: ResolvedType::Function {
            params: vec![self_type],
            ret: Box::new(ResolvedType::Unit), // Todo Poll
            effects: crate::effects::EffectSet::new(),
        },
        effects: crate::effects::EffectSet::new(),
    });
    ctx.concrete_items.push(poll_fn);
    
    // 3. Rewrite Original Function
    // fn MyFn(args) -> MyFn_Future
    let mut entry_fn = func.clone();
    
    // Construct Struct Init
    let mut init_fields = Vec::new();
    init_fields.push(("state".to_string(), Expr::Int(0, func.ast.span)));
    for param in &func.ast.params {
        init_fields.push((param.name.clone(), Expr::Ident(param.name.clone(), func.ast.span)));
    }
    
    // Initialize await fields
    for (i, _) in await_points.iter().enumerate() {
        init_fields.push((format!("_await_{}", i), Expr::None(func.ast.span)));
        init_fields.push((format!("_await_{}_result", i), Expr::None(func.ast.span)));
    }
    
    // Initialize captured locals
    let captured_locals = collect_locals(&func.ast.body);
    for (name, _) in captured_locals {
        // Skip params (already initialized)
        if func.ast.params.iter().any(|p| p.name == name) { continue; }
        init_fields.push((name, Expr::None(func.ast.span)));
    }
    
    let body_expr = Expr::Struct {
        name: state_machine_name.clone(),
        fields: init_fields,
        span: func.ast.span,
    };
    
    entry_fn.ast.body = Block {
        stmts: vec![Stmt::Return(Some(body_expr), func.ast.span)],
        span: func.ast.span,
    };
    
    // Update return type to Future (Struct)
    // Note: In real implementation this would be impl Future<Output=T>
    // For now, we return the struct directly.
    entry_fn.resolved_type = ResolvedType::Function {
        params: if let ResolvedType::Function{params, ..} = &func.resolved_type { params.clone() } else { vec![] },
        ret: Box::new(ResolvedType::Struct(state_machine_name, fields)),
        effects: crate::effects::EffectSet::new(), // Entry function is synchronous (returns Future)
    };
    
    // Clear async effect
    entry_fn.effects.effects.remove(&crate::effects::Effect::Async);
    entry_fn.ast.effects.retain(|e| *e != crate::effects::Effect::Async);
    
    Ok(entry_fn)
}

fn rewrite_access_to_self(block: &mut Block, fields: &HashMap<String, ResolvedType>) {
    for stmt in &mut block.stmts {
        rewrite_stmt(stmt, fields);
    }
}

fn rewrite_stmt(stmt: &mut Stmt, fields: &HashMap<String, ResolvedType>) {
    // 1. Rewrite expressions inside statements
    match stmt {
        Stmt::Expr(e) => rewrite_expr(e, fields),
        Stmt::Return(Some(e), _) => rewrite_expr(e, fields),
        Stmt::Let { value: Some(e), .. } => rewrite_expr(e, fields),
        Stmt::For { iter, body, .. } => {
            rewrite_expr(iter, fields);
            rewrite_access_to_self(body, fields);
        }
        Stmt::While { condition, body, .. } => {
            rewrite_expr(condition, fields);
            rewrite_access_to_self(body, fields);
        }
        _ => {}
    }
    
    // 2. Transform local bindings to struct assignments if captured
    let transform = if let Stmt::Let { pattern: Pattern::Binding { name, .. }, value: Some(e), span, .. } = stmt {
        if fields.contains_key(name) {
             Some((name.clone(), e.clone(), *span))
        } else { None }
    } else { None };
    
    if let Some((name, val, span)) = transform {
        *stmt = Stmt::Expr(Expr::Assign {
             target: Box::new(Expr::Field {
                 object: Box::new(Expr::Ident("self".to_string(), span)),
                 field: name,
                 span,
             }),
             value: Box::new(val),
             span,
        });
    }
}

fn rewrite_expr(expr: &mut Expr, fields: &HashMap<String, ResolvedType>) {
    match expr {
        Expr::Ident(name, span) => {
            if fields.contains_key(name) {
                // Transform `x` -> `self.x`
                *expr = Expr::Field {
                    object: Box::new(Expr::Ident("self".to_string(), *span)),
                    field: name.clone(),
                    span: *span,
                };
            }
        }
        Expr::Binary { left, right, .. } => {
            rewrite_expr(left, fields);
            rewrite_expr(right, fields);
        }
        Expr::Call { callee, args, .. } => {
            rewrite_expr(callee, fields);
            for arg in args {
                rewrite_expr(&mut arg.value, fields);
            }
        }
        Expr::Field { object, .. } => rewrite_expr(object, fields),
        Expr::Await(inner, _) => rewrite_expr(inner, fields),
        Expr::Block(b, _) => rewrite_access_to_self(b, fields),
        // Add other recursive cases...
        _ => {}
    }
}

// === AWAIT CHOPPING HELPERS ===

/// Represents an await point in the async function
#[derive(Clone, Debug)]
struct AwaitPoint {
    /// The expression being awaited
    awaited_expr: Expr,
    /// Variable to bind the result to (if any)
    result_binding: Option<String>,
    /// Index of this await point (for state numbering)
    index: usize,
}

/// Collect all await points from a block, in order of appearance
fn collect_await_points(block: &Block) -> Vec<AwaitPoint> {
    let mut points = Vec::new();
    collect_awaits_from_block(block, &mut points);
    points
}

fn collect_awaits_from_block(block: &Block, points: &mut Vec<AwaitPoint>) {
    for stmt in &block.stmts {
        collect_awaits_from_stmt(stmt, points);
    }
}

fn collect_awaits_from_stmt(stmt: &Stmt, points: &mut Vec<AwaitPoint>) {
    match stmt {
        Stmt::Let { pattern, value, .. } => {
            // Extract name from pattern if it's a simple binding
            let name = match pattern {
                Pattern::Binding { name: n, .. } => Some(n.clone()),
                _ => None,
            };
            
            // Check if the value is an await expression
            if let Some(expr) = value {
                if let Expr::Await(inner, _) = expr {
                    points.push(AwaitPoint {
                        awaited_expr: (**inner).clone(),
                        result_binding: name,
                        index: points.len(),
                    });
                } else {
                    collect_awaits_from_expr(expr, points);
                }
            }
        }
        Stmt::Expr(expr) => {
            if let Expr::Await(inner, _) = expr {
                points.push(AwaitPoint {
                    awaited_expr: (**inner).clone(),
                    result_binding: None,
                    index: points.len(),
                });
            } else {
                collect_awaits_from_expr(expr, points);
            }
        }
        Stmt::Return(Some(expr), _) => {
            if let Expr::Await(inner, _) = expr {
                points.push(AwaitPoint {
                    awaited_expr: (**inner).clone(),
                    result_binding: None, // Return will use the value directly
                    index: points.len(),
                });
            } else {
                collect_awaits_from_expr(expr, points);
            }
        }
        // Note: In KAIN, if is an expression, not a statement. If used in Stmt::Expr, 
        // collect_awaits_from_expr will handle it.
        Stmt::While { body, .. } | Stmt::Loop { body, .. } => {
            collect_awaits_from_block(body, points);
        }
        Stmt::For { body, .. } => {
            collect_awaits_from_block(body, points);
        }
        _ => {}
    }
}

fn collect_awaits_from_expr(expr: &Expr, points: &mut Vec<AwaitPoint>) {
    match expr {
        Expr::Await(inner, _) => {
            points.push(AwaitPoint {
                awaited_expr: (**inner).clone(),
                result_binding: None,
                index: points.len(),
            });
        }
        Expr::Binary { left, right, .. } => {
            collect_awaits_from_expr(left, points);
            collect_awaits_from_expr(right, points);
        }
        Expr::Call { callee, args, .. } => {
            collect_awaits_from_expr(callee, points);
            for arg in args {
                collect_awaits_from_expr(&arg.value, points);
            }
        }
        Expr::Block(block, _) => collect_awaits_from_block(block, points),
        Expr::If { then_branch, else_branch, .. } => {
            collect_awaits_from_block(then_branch, points);
            if let Some(else_b) = else_branch {
                match else_b.as_ref() {
                    ElseBranch::Else(b) => collect_awaits_from_block(b, points),
                    ElseBranch::ElseIf(_, then_b, _) => collect_awaits_from_block(then_b, points),
                }
            }
        }
        _ => {}
    }
}

/// Represents a segment of code between await points
#[derive(Clone)]
struct CodeSegment {
    /// Statements before the await (or all statements if no await in this segment)
    stmts_before: Vec<Stmt>,
    /// The await point (if this segment ends with an await)
    await_point: Option<AwaitPoint>,
    /// Whether this segment ends with a return
    ends_with_return: bool,
}

/// Split the function body at await points into segments
fn split_at_awaits(block: &Block, await_points: &[AwaitPoint]) -> Vec<CodeSegment> {
    let mut segments = Vec::new();
    let mut current_stmts = Vec::new();
    let mut await_idx = 0;
    
    for stmt in &block.stmts {
        // Check if this statement contains an await at the top level
        let contains_await = match stmt {
            Stmt::Let { value: Some(Expr::Await(_, _)), .. } => true,
            Stmt::Expr(Expr::Await(_, _)) => true,
            Stmt::Return(Some(Expr::Await(_, _)), _) => true,
            _ => false,
        };
        
        if contains_await && await_idx < await_points.len() {
            // End current segment, start new one after await
            segments.push(CodeSegment {
                stmts_before: current_stmts.clone(),
                await_point: Some(await_points[await_idx].clone()),
                ends_with_return: matches!(stmt, Stmt::Return(_, _)),
            });
            current_stmts.clear();
            await_idx += 1;
        } else {
            current_stmts.push(stmt.clone());
        }
    }
    
    // Add final segment (code after last await)
    if !current_stmts.is_empty() || segments.is_empty() {
        let ends_with_return = current_stmts.last()
            .map(|s| matches!(s, Stmt::Return(_, _)))
            .unwrap_or(false);
        segments.push(CodeSegment {
            stmts_before: current_stmts,
            await_point: None,
            ends_with_return,
        });
    }
    
    segments
}

/// Generate a match arm for a specific state in the state machine
fn generate_state_arm(
    state_idx: usize,
    segment: &CodeSegment,
    await_points: &[AwaitPoint],
    fields: &HashMap<String, ResolvedType>,
    _state_machine_name: &str,
    span: crate::span::Span,
) -> MatchArm {
    let mut body_stmts = Vec::new();
    
    // If this is a continuation state (after an await), we must POLL the future from the previous step
    if state_idx > 0 && state_idx <= await_points.len() {
        let prev_await = &await_points[state_idx - 1];
        let poll_field = format!("_await_{}", prev_await.index);
        let res_field = format!("_await_{}_result", prev_await.index);
        
        // Match expression to check poll status
        // match self._await_N.poll() { ... }
        let poll_call = Expr::MethodCall {
            receiver: Box::new(Expr::Field {
                object: Box::new(Expr::Ident("self".to_string(), span)),
                field: poll_field,
                span,
            }),
            method: "poll".to_string(),
            args: vec![],
            span,
        };
        
        // Arm 1: Poll::Pending => return Poll::Pending
        let pending_arm = MatchArm {
            pattern: Pattern::Variant {
                enum_name: Some("Poll".to_string()),
                variant: "Pending".to_string(),
                fields: VariantPatternFields::Unit,
                span,
            },
            guard: None,
            body: Expr::Return(
                Some(Box::new(Expr::EnumVariant {
                    enum_name: "Poll".to_string(),
                    variant: "Pending".to_string(),
                    fields: EnumVariantFields::Unit,
                    span,
                })),
                span,
            ),
            span,
        };
        
        // Arm 2: Poll::Ready(val) => { self._await_N_result = val; }
        // We capture 'val' in a binding
        let val_name = "val".to_string();
        let ready_arm = MatchArm {
            pattern: Pattern::Variant {
                enum_name: Some("Poll".to_string()),
                variant: "Ready".to_string(),
                fields: VariantPatternFields::Tuple(vec![
                    Pattern::Binding { name: val_name.clone(), mutable: false, span }
                ]),
                span,
            },
            guard: None,
            body: Expr::Assign {
                target: Box::new(Expr::Field {
                    object: Box::new(Expr::Ident("self".to_string(), span)),
                    field: res_field.clone(),
                    span,
                }),
                value: Box::new(Expr::Ident(val_name, span)),
                span,
            },
            span,
        };
        
        let poll_match = Expr::Match {
            scrutinee: Box::new(poll_call),
            arms: vec![pending_arm, ready_arm],
            span,
        };
        
        body_stmts.push(Stmt::Expr(poll_match));
        
        // Bind the result to the user's variable: let result_binding = self._await_N_result
        // Bind the result to the user's variable
        // If captured, we must assign to self.variable, otherwise use let for temporary
        if let Some(binding) = &prev_await.result_binding {
            if fields.contains_key(binding) {
                 // self.binding = self._await_N_result
                 body_stmts.push(Stmt::Expr(Expr::Assign {
                     target: Box::new(Expr::Field {
                         object: Box::new(Expr::Ident("self".to_string(), span)),
                         field: binding.clone(),
                         span,
                     }),
                     value: Box::new(Expr::Field {
                         object: Box::new(Expr::Ident("self".to_string(), span)),
                         field: res_field,
                         span,
                     }),
                     span,
                 }));
            } else {
                body_stmts.push(Stmt::Let {
                    pattern: Pattern::Binding { name: binding.clone(), mutable: false, span },
                    ty: None,
                    value: Some(Expr::Field {
                        object: Box::new(Expr::Ident("self".to_string(), span)),
                        field: res_field,
                        span,
                    }),
                    span,
                });
            }
        }
    }
    
    // Add the segment's statements (rewritten to use self.x)
    for stmt in &segment.stmts_before {
        let mut rewritten_stmt = stmt.clone();
        rewrite_stmt(&mut rewritten_stmt, fields);
        body_stmts.push(rewritten_stmt);
    }
    
    // Handle the await point (if any)
    if let Some(await_point) = &segment.await_point {
        // 1. Evaluate the future expression and store it
        let store_field = format!("_await_{}", await_point.index);
        let mut awaited_expr = await_point.awaited_expr.clone();
        rewrite_expr(&mut awaited_expr, fields);
        
        body_stmts.push(Stmt::Expr(Expr::Assign {
            target: Box::new(Expr::Field {
                object: Box::new(Expr::Ident("self".to_string(), span)),
                field: store_field,
                span,
            }),
            value: Box::new(awaited_expr),
            span,
        }));
        
        // 2. Increment state
        body_stmts.push(Stmt::Expr(Expr::Assign {
            target: Box::new(Expr::Field {
                object: Box::new(Expr::Ident("self".to_string(), span)),
                field: "state".to_string(),
                span,
            }),
            value: Box::new(Expr::Int((state_idx + 1) as i64, span)),
            span,
        }));
        
        // 3. Return Poll::Pending
        body_stmts.push(Stmt::Return(
            Some(Expr::EnumVariant {
                enum_name: "Poll".to_string(),
                variant: "Pending".to_string(),
                fields: EnumVariantFields::Unit,
                span,
            }),
            span,
        ));
    } else if segment.ends_with_return {
        // Already has a return - wrap it in Poll::Ready
        // (handled by the rewrite)
    } else if state_idx == await_points.len() {
        // Final state after all awaits - return Poll::Ready(Unit) or the result
        body_stmts.push(Stmt::Return(
            Some(Expr::EnumVariant {
                enum_name: "Poll".to_string(),
                variant: "Ready".to_string(),
                fields: EnumVariantFields::Tuple(vec![Expr::None(span)]),
                span,
            }),
            span,
        ));
    }
    
    MatchArm {
        pattern: Pattern::Literal(Expr::Int(state_idx as i64, span)),
        guard: None,
        body: Expr::Block(Block { stmts: body_stmts, span }, span),
        span,
    }
}

/// Wrap the body's returns in Poll::Ready
fn wrap_return_in_poll_ready(mut block: Block, span: crate::span::Span) -> Expr {
    for stmt in &mut block.stmts {
        wrap_stmt_returns(stmt, span);
    }
    Expr::Block(block, span)
}

fn wrap_stmt_returns(stmt: &mut Stmt, span: crate::span::Span) {
    match stmt {
        Stmt::Return(Some(expr), _) => {
            // Wrap: return x -> return Poll::Ready(x)
            let inner = std::mem::replace(expr, Expr::None(span));
            *expr = Expr::EnumVariant {
                enum_name: "Poll".to_string(),
                variant: "Ready".to_string(),
                fields: EnumVariantFields::Tuple(vec![inner]),
                span,
            };
        }
        Stmt::Return(None, s) => {
            // Wrap: return -> return Poll::Ready(())
            *stmt = Stmt::Return(
                Some(Expr::EnumVariant {
                    enum_name: "Poll".to_string(),
                    variant: "Ready".to_string(),
                    fields: EnumVariantFields::Tuple(vec![Expr::None(span)]),
                    span,
                }),
                *s,
            );
        }
        // Note: In KAIN, if is an expression. Expr::If would be in Stmt::Expr, 
        // but we handle expressions separately if needed.
        Stmt::While { body, .. } | Stmt::Loop { body, .. } => {
            for s in &mut body.stmts {
                wrap_stmt_returns(s, span);
            }
        }
        Stmt::For { body, .. } => {
            for s in &mut body.stmts {
                wrap_stmt_returns(s, span);
            }
        }
        _ => {}
    }
}

fn scan_function(ctx: &mut MonoContext, func: &TypedFunction) -> KainResult<TypedFunction> {
    let mut new_func = func.clone();
    let mut env = MonoTypeEnv::new();
    
    if let ResolvedType::Function { params, .. } = &func.resolved_type {
        for (i, p) in params.iter().enumerate() {
            if i < func.ast.params.len() {
                env.define(func.ast.params[i].name.clone(), p.clone());
            }
        }
    }
    
    scan_block(ctx, &mut env, &mut new_func.ast.body)?;
    Ok(new_func)
}

fn scan_block(ctx: &mut MonoContext, env: &mut MonoTypeEnv, block: &mut Block) -> KainResult<()> {
    env.push();
    for stmt in &mut block.stmts {
        scan_stmt(ctx, env, stmt)?;
    }
    env.pop();
    Ok(())
}

fn scan_stmt(ctx: &mut MonoContext, env: &mut MonoTypeEnv, stmt: &mut Stmt) -> KainResult<()> {
    match stmt {
        Stmt::Expr(e) => { scan_expr(ctx, env, e)?; }
        Stmt::Return(Some(e), _) => { scan_expr(ctx, env, e)?; }
        Stmt::Let { pattern, value, .. } => {
            // Scan the value expression (may contain generic calls like identity(42))
            if let Some(val_expr) = value {
                let ty = scan_expr(ctx, env, val_expr)?;
                // Also define the binding in the environment for type inference
                if let Pattern::Binding { name, .. } = pattern {
                    env.define(name.clone(), ty);
                }
            }
        }
        Stmt::For { binding, iter, body, .. } => {
            let iter_ty = scan_expr(ctx, env, iter)?;
            let elem_ty = match iter_ty {
                ResolvedType::Array(inner, _) => *inner,
                _ => ResolvedType::Int(IntSize::I64),
            };
            
            env.push();
            if let Pattern::Binding { name, .. } = binding {
                env.define(name.clone(), elem_ty);
            }
            scan_block(ctx, env, body)?;
            env.pop();
        }
        Stmt::While { condition, body, .. } => {
            scan_expr(ctx, env, condition)?;
            scan_block(ctx, env, body)?;
        }
        _ => {}
    }
    Ok(())
}

fn scan_expr(ctx: &mut MonoContext, env: &mut MonoTypeEnv, expr: &mut Expr) -> KainResult<ResolvedType> {
    match expr {
        Expr::Int(_, _) => Ok(ResolvedType::Int(IntSize::I64)),
        Expr::Float(_, _) => Ok(ResolvedType::Float(FloatSize::F64)),
        Expr::String(_, _) => Ok(ResolvedType::String),
        Expr::Bool(_, _) => Ok(ResolvedType::Bool),
        Expr::Ident(name, _) => Ok(env.get(name)),
        Expr::Struct { name, fields, .. } => {
            for (_, val) in fields {
                scan_expr(ctx, env, val)?;
            }
            // Return struct type
            // Ideally we check fields against definition, but here we just return the type
            Ok(ResolvedType::Struct(name.clone(), HashMap::new()))
        },
        Expr::Field { object, field, span: _ } => {
            let obj_ty = scan_expr(ctx, env, object)?;
            match obj_ty {
                ResolvedType::Struct(name, _) => {
                    if let Some(fields) = ctx.structs.get(&name) {
                         if let Some(ty) = fields.get(field) {
                             return Ok(ty.clone());
                         }
                    }
                    // If struct logic isn't fully loaded or field missing, return Unknown but maybe warn?
                    // For now, if we can't find it, we can't infer proper type for chain calls.
                    Ok(ResolvedType::Unknown)
                }
                _ => Ok(ResolvedType::Unknown),
            }
        },
        Expr::MethodCall { receiver, method, args, span } => {
            let receiver_ty = scan_expr(ctx, env, receiver)?;
            
            let type_name = match &receiver_ty {
                ResolvedType::Struct(name, _) => name.clone(),
                ResolvedType::Int(_) => "Int".to_string(),
                ResolvedType::Float(_) => "Float".to_string(),
                ResolvedType::String => "String".to_string(),
                _ => {
                    if let ResolvedType::Unknown = receiver_ty {
                         // Don't error hard yet, as we might be in partial state
                         return Ok(ResolvedType::Unknown);
                    }
                    format!("{:?}", receiver_ty)
                }
            };
            
            let mangled_target = {
                let methods = ctx.methods.get(&type_name);
                if let Some(lookup) = methods {
                    lookup.get(method).cloned()
                } else {
                    None
                }
            };
            
            if let Some(target_name) = mangled_target {
                 let mut new_args = args.clone();
                 new_args.insert(0, CallArg { name: None, value: *receiver.clone(), span: receiver.span() });
                 
                 for arg in &mut new_args {
                     scan_expr(ctx, env, &mut arg.value)?;
                 }

                 *expr = Expr::Call {
                     callee: Box::new(Expr::Ident(target_name, *span)), // No ctx borrow here
                     args: new_args,
                     span: *span
                 };
                 
                 // Ideally lookup return type of function
                 // For now Unknown is safe for logic
                 return Ok(ResolvedType::Unknown);
            }

            Ok(ResolvedType::Unknown)
        }
        Expr::Call { callee, args, .. } => {
            if let Expr::Ident(name, _) = callee.as_ref() {
                if let Some(generic_func) = ctx.generic_functions.get(name).cloned() {
                    // First, scan all arguments to get their types
                    let mut arg_types = Vec::new();
                    for arg in args {
                        arg_types.push(scan_expr(ctx, env, &mut arg.value)?);
                    }
                    
                    // Infer type arguments through unification
                    let inferred_type_args = infer_type_args(ctx, &generic_func, &arg_types)?;
                    
                    let new_name = ctx.instantiate(name, &inferred_type_args)?;
                    *callee = Box::new(Expr::Ident(new_name, callee.span()));
                    return Ok(ResolvedType::Unknown); 
                }
                
                // If it's a standard function, we might want to lookup return type
                // But for now, just scan args
            }
             for arg in args {
                scan_expr(ctx, env, &mut arg.value)?;
            }
            Ok(ResolvedType::Unknown)
        }
        Expr::Binary { left, right, .. } => {
            let t = scan_expr(ctx, env, left)?;
            scan_expr(ctx, env, right)?;
            Ok(t) 
        }
        Expr::Assign { value, .. } => scan_expr(ctx, env, value),
        Expr::Block(b, _) => {
            scan_block(ctx, env, b)?;
            Ok(ResolvedType::Unknown)
        }
        Expr::If { condition, then_branch, else_branch, .. } => {
            scan_expr(ctx, env, condition)?;
            scan_block(ctx, env, then_branch)?;
            if let Some(b) = else_branch {
                 match b.as_mut() {
                     ElseBranch::Else(blk) => { scan_block(ctx, env, blk)?; }
                     ElseBranch::ElseIf(_, _, _) => {} 
                 }
            }
             Ok(ResolvedType::Unknown)
        }
        Expr::Await(inner, _) => {
            // Scan the inner future expression for generic calls
            scan_expr(ctx, env, inner)
        }
        _ => Ok(ResolvedType::Unknown),
    }
}

fn collect_locals(block: &Block) -> HashMap<String, ResolvedType> {
    let mut locals = HashMap::new();
    collect_locals_recursive(block, &mut locals);
    locals
}

fn collect_locals_recursive(block: &Block, locals: &mut HashMap<String, ResolvedType>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let { pattern, .. } => collect_from_pattern(pattern, locals),
            Stmt::For { body, .. } => collect_locals_recursive(body, locals),
            Stmt::While { body, .. } => collect_locals_recursive(body, locals),
            Stmt::Expr(Expr::Block(b, _)) => collect_locals_recursive(b, locals),
            Stmt::Expr(Expr::If { then_branch, else_branch, .. }) => {
                collect_locals_recursive(then_branch, locals);
                if let Some(b) = else_branch {
                    collect_from_else(b, locals);
                }
            }
            _ => {}
        }
    }
}

fn collect_from_else(branch: &ElseBranch, locals: &mut HashMap<String, ResolvedType>) {
     match branch {
         ElseBranch::Else(block) => collect_locals_recursive(block, locals),
         ElseBranch::ElseIf(_, block, next) => {
             collect_locals_recursive(block, locals);
             if let Some(n) = next {
                 collect_from_else(n, locals);
             }
         }
     }
}

fn collect_from_pattern(pattern: &Pattern, locals: &mut HashMap<String, ResolvedType>) {
    match pattern {
        Pattern::Binding { name, .. } => { locals.insert(name.clone(), ResolvedType::Unknown); },
        Pattern::Tuple(pats, _) => {
            for p in pats { collect_from_pattern(p, locals); }
        }
        Pattern::Slice { patterns, rest, .. } => {
             for p in patterns { collect_from_pattern(p, locals); }
             if let Some(r) = rest {
                 locals.insert(r.clone(), ResolvedType::Unknown);
             }
        }
        Pattern::Struct { fields, .. } => {
             for (_, p) in fields { collect_from_pattern(p, locals); }
        }
        Pattern::Variant { fields, .. } => {
              match fields {
                  VariantPatternFields::Tuple(pats) => { for p in pats { collect_from_pattern(p, locals); } },
                  VariantPatternFields::Struct(pats) => { for (_, p) in pats { collect_from_pattern(p, locals); } },
                  _ => {}
              }
        }
        Pattern::Or(pats, _) => {
             for p in pats { collect_from_pattern(p, locals); }
        }
        _ => {}
    }
}

