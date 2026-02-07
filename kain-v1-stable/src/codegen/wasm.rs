//! WASM Code Generation using walrus
//! 
//! This module converts the Typed AST into WebAssembly.

use crate::ast::{Expr, BinaryOp, Stmt, Block};
use crate::types::{ResolvedType, TypedFunction, TypedItem, TypedProgram};
use crate::error::{KainResult, KainError};
use walrus::{FunctionBuilder, InstrSeqBuilder, LocalId, Module, ModuleConfig, ValType};
use std::collections::HashMap;

pub fn generate(program: &TypedProgram) -> KainResult<Vec<u8>> {
    let mut compiler = WasmCompiler::new();
    compiler.compile_program(program)?;
    Ok(compiler.module.emit_wasm())
}

struct WasmCompiler {
    module: Module,
    /// Map function names to their WASM function IDs for call resolution
    functions: HashMap<String, walrus::FunctionId>,
    /// Memory ID for linear memory
    memory_id: Option<walrus::MemoryId>,
    heap_ptr_global: walrus::GlobalId,
    /// Current offset in data segment for string allocation
    data_offset: u32,
    /// Map string literals to their memory offset (for deduplication)
    string_table: HashMap<String, u32>,
    /// Struct layouts: struct_name -> (field_name -> offset, total_size)
    struct_layouts: HashMap<String, (HashMap<String, u32>, u32)>,
    /// Enum layouts: enum_name -> (variant_name -> tag, max_payload_size, variant_name -> (field_name -> offset))
    enum_layouts: HashMap<String, (HashMap<String, u32>, u32, HashMap<String, HashMap<String, u32>>)>,
    /// Heap pointer (for runtime allocation) - starts after data segment
    // heap_ptr: u32, // Unused
    /// Funcref table for indirect calls (closures)
    funcref_table: Option<walrus::TableId>,
    /// Counter for generating unique lambda names
    lambda_counter: u32,
    /// Map lambda ID -> (table_index, func_id) for indirect calls
    lambda_table: HashMap<u32, (u32, walrus::FunctionId)>,
}

// Separate Context from Builder to avoid self-borrow issues
// Locals are pre-allocated, so we don't need mutable access during emission
struct CompilationContext<'a> {
    locals: HashMap<String, LocalId>,
    functions: &'a HashMap<String, walrus::FunctionId>,
    string_table: &'a HashMap<String, u32>,
    struct_layouts: &'a HashMap<String, (HashMap<String, u32>, u32)>,
    enum_layouts: &'a HashMap<String, (HashMap<String, u32>, u32, HashMap<String, HashMap<String, u32>>)>,
    memory_id: walrus::MemoryId,
    heap_ptr_global: walrus::GlobalId,
    tmp_i32: LocalId,
    tmp_i32_2: LocalId,
    tmp_i64: LocalId,
    funcref_table: Option<walrus::TableId>,
    lambda_table: &'a HashMap<u32, (u32, walrus::FunctionId)>,
}

impl WasmCompiler {
    fn new() -> Self {
        let config = ModuleConfig::new();
        let mut module = Module::with_config(config);
        
        // Create linear memory (1 page = 64KB)
        // add_local(shared, memory64, initial, maximum, page_size_log2)
        let memory_id = module.memories.add_local(false, false, 1, None, None);
        module.exports.add("memory", memory_id);

        let heap_ptr = 4096u32;
        let heap_ptr_global = module.globals.add_local(
            ValType::I32,
            true,
            false, // shared
            walrus::ConstExpr::Value(walrus::ir::Value::I32(heap_ptr as i32)),
        );
        
        // --- WASM Host Imports for I/O ---
        let mut functions = HashMap::new();
        
        // print_i64(value: i64) -> void
        let print_i64_type = module.types.add(&[ValType::I64], &[]);
        let (print_i64_func, _) = module.add_import_func("host", "print_i64", print_i64_type);
        functions.insert("print_i64".to_string(), print_i64_func);
        
        // print_f64(value: f64) -> void
        let print_f64_type = module.types.add(&[ValType::F64], &[]);
        let (print_f64_func, _) = module.add_import_func("host", "print_f64", print_f64_type);
        functions.insert("print_f64".to_string(), print_f64_func);
        
        // print_str(ptr: i32, len: i32) -> void
        let print_str_type = module.types.add(&[ValType::I32, ValType::I32], &[]);
        let (print_str_func, _) = module.add_import_func("host", "print_str", print_str_type);
        functions.insert("print_str".to_string(), print_str_func);
        
        // print_bool(value: i32) -> void  
        let print_bool_type = module.types.add(&[ValType::I32], &[]);
        let (print_bool_func, _) = module.add_import_func("host", "print_bool", print_bool_type);
        functions.insert("print_bool".to_string(), print_bool_func);
        
        // read_i64() -> i64
        let read_i64_type = module.types.add(&[], &[ValType::I64]);
        let (read_i64_func, _) = module.add_import_func("host", "read_i64", read_i64_type);
        functions.insert("read_i64".to_string(), read_i64_func);

        // int_to_str(val: i64) -> ptr: i32
        let int_to_str_type = module.types.add(&[ValType::I64], &[ValType::I32]);
        let (int_to_str_func, _) = module.add_import_func("host", "int_to_str", int_to_str_type);
        functions.insert("int_to_str".to_string(), int_to_str_func);

        // str_concat(ptr1: i32, len1: i32, ptr2: i32, len2: i32) -> ptr: i32
        // Note: For simplicity, we'll assume strings are just pointers in this specific hack, 
        // but robustly we need lengths.
        // If our runtime strings are (ptr, len), we can't easily pass them as single values.
        // Let's assume the host handles "String Objects" via pointers for concatenation.
        // BUT `print_str` takes (ptr, len). 
        // Let's change strategy: strings are pointers to [len: i32, data...].
        // So we just pass pointers.
        let str_concat_type = module.types.add(&[ValType::I32, ValType::I32], &[ValType::I32]);
        let (str_concat_func, _) = module.add_import_func("host", "str_concat", str_concat_type);
        functions.insert("str_concat".to_string(), str_concat_func);

        // time_now() -> i64
        let time_now_type = module.types.add(&[], &[ValType::I64]);
        let (time_now_func, _) = module.add_import_func("host", "time_now", time_now_type);
        functions.insert("time_now".to_string(), time_now_func);

        // --- DOM Imports ---
        // dom_create(tag_ptr: i32, tag_len: i32) -> node_id: i32
        let dom_create_type = module.types.add(&[ValType::I32, ValType::I32], &[ValType::I32]);
        let (dom_create_func, _) = module.add_import_func("host", "dom_create", dom_create_type);
        functions.insert("dom_create".to_string(), dom_create_func);

        // dom_append(parent_id: i32, child_id: i32) -> void
        let dom_append_type = module.types.add(&[ValType::I32, ValType::I32], &[]);
        let (dom_append_func, _) = module.add_import_func("host", "dom_append", dom_append_type);
        functions.insert("dom_append".to_string(), dom_append_func);

        // dom_attr(node_id: i32, key_ptr: i32, key_len: i32, val_ptr: i32, val_len: i32) -> void
        let dom_attr_type = module.types.add(&[ValType::I32, ValType::I32, ValType::I32, ValType::I32, ValType::I32], &[]);
        let (dom_attr_func, _) = module.add_import_func("host", "dom_attr", dom_attr_type);
        functions.insert("dom_attr".to_string(), dom_attr_func);
        
        // dom_text(text_ptr: i32, text_len: i32) -> node_id: i32
        let dom_text_type = module.types.add(&[ValType::I32, ValType::I32], &[ValType::I32]);
        let (dom_text_func, _) = module.add_import_func("host", "dom_text", dom_text_type);
        functions.insert("dom_text".to_string(), dom_text_func);
        
        // Create funcref table for closures/lambdas
        // Starts with 16 slots, can grow as needed
        let funcref_table = module.tables.add_local(false, 16, Some(256), walrus::RefType::Funcref);
        
        Self {
            module,
            functions,
            memory_id: Some(memory_id),
            heap_ptr_global,
            data_offset: 0,
            string_table: HashMap::new(),
            struct_layouts: HashMap::new(),
            enum_layouts: HashMap::new(),
            // heap_ptr, // Unused
            funcref_table: Some(funcref_table),
            lambda_counter: 0,
            lambda_table: HashMap::new(),
        }
    }

    fn compile_program(&mut self, program: &TypedProgram) -> KainResult<()> {
        // First pass: collect struct layouts
        for item in &program.items {
            if let TypedItem::Struct(s) = item {
                self.compute_struct_layout(s);
            }
            if let TypedItem::Component(c) = item {
                self.compute_component_layout(c);
            }
        }

        // Second pass: collect enum layouts
        for item in &program.items {
            if let TypedItem::Enum(e) = item {
                self.compute_enum_layout(e);
            }
        }
        
        // Third pass: collect all string literals
        for item in &program.items {
            if let TypedItem::Function(f) = item {
                self.collect_strings_in_block(&f.ast.body);
            }
        }

        // Fourth pass: collect and compile all lambdas
        let mut all_lambdas = Vec::new();
        for item in &program.items {
            if let TypedItem::Function(f) = item {
                self.collect_lambdas_in_block(&f.ast.body, &mut all_lambdas);
            }
        }
        // Compile each lambda to a WASM function
        for (id, params, body) in all_lambdas {
            self.compile_lambda(id, &params, &body)?;
        }

        // Fifth pass: declare functions (recursion support)
        for item in &program.items {
            if let TypedItem::Function(f) = item {
                self.declare_function(f)?;
            }
        }
        
        // Fifth pass: compile function bodies
        for item in &program.items {
            match item {
                TypedItem::Function(f) => {
                    self.compile_function_body(f)?;
                }
                _ => {} 
            }
        }
        
        // Sixth pass: compile components
        for item in &program.items {
            if let TypedItem::Component(c) = item {
                self.compile_component(c)?;
            }
        }
        
        Ok(())
    }
    
    fn compute_struct_layout(&mut self, s: &crate::types::TypedStruct) {
        let mut offset = 0u32;
        let mut field_offsets = HashMap::new();
        
        for field in &s.ast.fields {
            // Align to 4 bytes
            offset = (offset + 3) & !3;
            field_offsets.insert(field.name.clone(), offset);
            
            // Calculate field size based on type
            let field_size = self.type_size_of(&s.field_types.get(&field.name).cloned().unwrap_or(ResolvedType::Int(crate::types::IntSize::I64)));
            offset += field_size;
        }
        
        // Align total size to 4 bytes
        let total_size = (offset + 3) & !3;
        self.struct_layouts.insert(s.ast.name.clone(), (field_offsets, total_size));
    }

    fn compute_component_layout(&mut self, c: &crate::types::TypedComponent) {
        let mut offset = 0u32;
        let mut field_offsets = HashMap::new();
        
        for state in &c.ast.state {
            // Align to 4 bytes
            offset = (offset + 3) & !3;
            field_offsets.insert(state.name.clone(), offset);
            
            // Assume 8 bytes for now
            offset += 8;
        }
        
        let total_size = (offset + 3) & !3;
        self.struct_layouts.insert(c.ast.name.clone(), (field_offsets, total_size));
    }

    fn compile_component(&mut self, c: &crate::types::TypedComponent) -> KainResult<()> {
        let render_name = format!("{}_render", c.ast.name);
        
        // Params: self (i32)
        // Ret: VNode (i32)
        let wasm_params = vec![ValType::I32];
        let wasm_results = vec![ValType::I32];
        
        let mut builder = FunctionBuilder::new(&mut self.module.types, &wasm_params, &wasm_results);
        let self_local = self.module.locals.add(ValType::I32);
        
        // Locals
        let tmp_i32 = self.module.locals.add(ValType::I32);
        let tmp_i32_2 = self.module.locals.add(ValType::I32);
        let tmp_i64 = self.module.locals.add(ValType::I64);
        
        let mut locals_map = HashMap::new();
        locals_map.insert("self".to_string(), self_local);
        
        let ctx = CompilationContext {
            locals: locals_map,
            functions: &self.functions,
            string_table: &self.string_table,
            struct_layouts: &self.struct_layouts,
            enum_layouts: &self.enum_layouts,
            memory_id: self.memory_id.unwrap(),
            heap_ptr_global: self.heap_ptr_global,
            tmp_i32,
            tmp_i32_2,
            tmp_i64,
            funcref_table: self.funcref_table,
            lambda_table: &self.lambda_table,
        };
        
        let mut func_body = builder.func_body();
        self.compile_jsx_node(&ctx, &mut func_body, &c.ast.body)?;
        
        let func_id = builder.finish(vec![self_local], &mut self.module.funcs);
        self.functions.insert(render_name.clone(), func_id);
        self.module.exports.add(&render_name, func_id);
        
        Ok(())
    }

    fn compute_enum_layout(&mut self, e: &crate::types::TypedEnum) {
        let mut variant_tags = HashMap::new();
        let mut max_payload_size = 0u32;
        let mut variant_field_offsets = HashMap::new();

        for (idx, variant) in e.ast.variants.iter().enumerate() {
            variant_tags.insert(variant.name.clone(), idx as u32);
            let mut field_offsets = HashMap::new();
            let mut payload_size = 0u32;

            if let Some(payload_types) = e.variant_payload_types.get(&variant.name) {
                let mut current_offset = 0u32;
                
                // Determine offsets based on variant type
                match &variant.fields {
                    crate::ast::VariantFields::Struct(fields) => {
                         for (i, field) in fields.iter().enumerate() {
                             if let Some(ty) = payload_types.get(i) {
                                 // Align to 4 bytes for simplicity (WASM is 32-bit mostly)
                                 current_offset = (current_offset + 3) & !3;
                                 field_offsets.insert(field.name.clone(), current_offset);
                                 
                                 let size = self.type_size_of(ty);
                                 current_offset += size;
                             }
                         }
                    }
                    crate::ast::VariantFields::Tuple(_) => {
                         for (i, ty) in payload_types.iter().enumerate() {
                             current_offset = (current_offset + 3) & !3;
                             field_offsets.insert(i.to_string(), current_offset);
                             current_offset += self.type_size_of(ty);
                         }
                    }
                    crate::ast::VariantFields::Unit => {}
                }
                
                // Align final size
                payload_size = (current_offset + 3) & !3;
            }

            variant_field_offsets.insert(variant.name.clone(), field_offsets);
            max_payload_size = max_payload_size.max(payload_size);
        }

        self.enum_layouts
            .insert(e.ast.name.clone(), (variant_tags, max_payload_size, variant_field_offsets));
    }
    
    fn type_size_of(&self, ty: &ResolvedType) -> u32 {
        match ty {
            ResolvedType::Unit => 0,
            ResolvedType::Bool => 4,
            ResolvedType::Int(crate::types::IntSize::I8) | ResolvedType::Int(crate::types::IntSize::U8) => 1,
            ResolvedType::Int(crate::types::IntSize::I16) | ResolvedType::Int(crate::types::IntSize::U16) => 2,
            ResolvedType::Int(crate::types::IntSize::I32) | ResolvedType::Int(crate::types::IntSize::U32) => 4,
            ResolvedType::Int(crate::types::IntSize::I64) | ResolvedType::Int(crate::types::IntSize::U64) | ResolvedType::Int(crate::types::IntSize::Isize) | ResolvedType::Int(crate::types::IntSize::Usize) => 8,
            ResolvedType::Float(crate::types::FloatSize::F32) => 4,
            ResolvedType::Float(crate::types::FloatSize::F64) => 8,
            ResolvedType::String => 4, // pointer
            ResolvedType::Char => 4,
            ResolvedType::Array(_, len) => 4 + (*len as u32 * 8), // pointer + inline storage
            ResolvedType::Struct(_, _) => 4, // pointer
            _ => 8, // default to 8 bytes
        }
    }
    
    /// Emit bump allocator: allocates `size` bytes, returns pointer to start
    /// Stack effect: [] -> [i32 pointer]
    /// 
    /// Algorithm:
    ///   old_ptr = heap_ptr
    ///   heap_ptr = (heap_ptr + size + 7) & ~7  // 8-byte aligned
    ///   return old_ptr
    fn emit_alloc(&self, ctx: &CompilationContext, builder: &mut InstrSeqBuilder, size: u32) {
        // Get current heap pointer (this will be our return value)
        builder.global_get(ctx.heap_ptr_global);
        
        // Compute new heap pointer: (heap_ptr + size + 7) & ~7
        builder.global_get(ctx.heap_ptr_global);
        builder.i32_const(size as i32);
        builder.binop(walrus::ir::BinaryOp::I32Add);
        builder.i32_const(7);
        builder.binop(walrus::ir::BinaryOp::I32Add);
        builder.i32_const(-8); // ~7 in two's complement
        builder.binop(walrus::ir::BinaryOp::I32And);
        
        // Store new heap pointer
        builder.global_set(ctx.heap_ptr_global);
        
        // Stack now has: [old_ptr] - which is our allocated address
    }
    
    fn collect_strings_in_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.collect_strings_in_stmt(stmt);
        }
    }
    
    fn collect_strings_in_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Expr(expr) => self.collect_strings_in_expr(expr),
            Stmt::Let { value: Some(expr), .. } => self.collect_strings_in_expr(expr),
            Stmt::Return(Some(expr), _) => self.collect_strings_in_expr(expr),
            Stmt::While { condition, body, .. } => {
                self.collect_strings_in_expr(condition);
                self.collect_strings_in_block(body);
            }
            _ => {}
        }
    }
    
    fn collect_strings_in_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::String(s, _) => {
                self.allocate_string(s);
            }
            Expr::Binary { left, right, .. } => {
                self.collect_strings_in_expr(left);
                self.collect_strings_in_expr(right);
            }
            Expr::Call { args, .. } => {
                for arg in args {
                    self.collect_strings_in_expr(&arg.value);
                }
            }
            Expr::If { condition, then_branch, else_branch, .. } => {
                self.collect_strings_in_expr(condition);
                self.collect_strings_in_block(then_branch);
                if let Some(else_br) = else_branch {
                    self.collect_strings_in_else(else_br);
                }
            }
            Expr::JSX(node, _) => {
                self.collect_strings_in_jsx(node);
            }
            _ => {}
        }
    }
    
    fn collect_strings_in_jsx(&mut self, node: &crate::ast::JSXNode) {
        match node {
            crate::ast::JSXNode::Element { tag, attributes, children, .. } => {
                self.allocate_string(tag);
                for attr in attributes {
                    self.allocate_string(&attr.name);
                    match &attr.value {
                        crate::ast::JSXAttrValue::String(s) => { self.allocate_string(s); },
                        crate::ast::JSXAttrValue::Expr(e) => self.collect_strings_in_expr(e),
                        _ => {}
                    }
                }
                for child in children {
                    self.collect_strings_in_jsx(child);
                }
            }
            crate::ast::JSXNode::Text(s, _) => {
                self.allocate_string(s);
            }
            crate::ast::JSXNode::Expression(e) => {
                self.collect_strings_in_expr(e);
            }
            crate::ast::JSXNode::ComponentCall { name, props, children, .. } => {
                // Name might not be a string literal in runtime, but let's alloc it anyway
                self.allocate_string(name);
                for attr in props {
                    self.allocate_string(&attr.name);
                    match &attr.value {
                        crate::ast::JSXAttrValue::String(s) => { self.allocate_string(s); },
                        crate::ast::JSXAttrValue::Expr(e) => self.collect_strings_in_expr(e),
                        _ => {}
                    }
                }
                for child in children {
                    self.collect_strings_in_jsx(child);
                }
            }
            crate::ast::JSXNode::Fragment(children, _) => {
                for child in children {
                    self.collect_strings_in_jsx(child);
                }
            }
            crate::ast::JSXNode::For { iter, body, .. } => {
                self.collect_strings_in_expr(iter);
                self.collect_strings_in_jsx(body);
            }
            crate::ast::JSXNode::If { condition, then_branch, else_branch, .. } => {
                self.collect_strings_in_expr(condition);
                self.collect_strings_in_jsx(then_branch);
                if let Some(else_br) = else_branch {
                    self.collect_strings_in_jsx(else_br);
                }
            }
        }
    }
    
    fn collect_strings_in_else(&mut self, branch: &crate::ast::ElseBranch) {
        match branch {
            crate::ast::ElseBranch::Else(block) => self.collect_strings_in_block(block),
            crate::ast::ElseBranch::ElseIf(cond, then, next) => {
                self.collect_strings_in_expr(cond);
                self.collect_strings_in_block(then);
                if let Some(next_br) = next {
                    self.collect_strings_in_else(next_br);
                }
            }
        }
    }

    // === LAMBDA COLLECTION AND COMPILATION ===

    fn collect_lambdas_in_block(&mut self, block: &Block, lambdas: &mut Vec<(u32, Vec<crate::ast::Param>, Expr)>) {
        for stmt in &block.stmts {
            self.collect_lambdas_in_stmt(stmt, lambdas);
        }
    }

    fn collect_lambdas_in_stmt(&mut self, stmt: &Stmt, lambdas: &mut Vec<(u32, Vec<crate::ast::Param>, Expr)>) {
        match stmt {
            Stmt::Expr(expr) => self.collect_lambdas_in_expr(expr, lambdas),
            Stmt::Let { value: Some(expr), .. } => self.collect_lambdas_in_expr(expr, lambdas),
            Stmt::Return(Some(expr), _) => self.collect_lambdas_in_expr(expr, lambdas),
            Stmt::While { condition, body, .. } => {
                self.collect_lambdas_in_expr(condition, lambdas);
                self.collect_lambdas_in_block(body, lambdas);
            }
            Stmt::For { iter, body, .. } => {
                self.collect_lambdas_in_expr(iter, lambdas);
                self.collect_lambdas_in_block(body, lambdas);
            }
            Stmt::Loop { body, .. } => {
                self.collect_lambdas_in_block(body, lambdas);
            }
            _ => {}
        }
    }

    fn collect_lambdas_in_expr(&mut self, expr: &Expr, lambdas: &mut Vec<(u32, Vec<crate::ast::Param>, Expr)>) {
        match expr {
            Expr::Lambda { params, body, .. } => {
                let id = self.lambda_counter;
                self.lambda_counter += 1;
                lambdas.push((id, params.clone(), (**body).clone()));
                // Also collect nested lambdas in body
                self.collect_lambdas_in_expr(body, lambdas);
            }
            Expr::Binary { left, right, .. } => {
                self.collect_lambdas_in_expr(left, lambdas);
                self.collect_lambdas_in_expr(right, lambdas);
            }
            Expr::Unary { operand, .. } => {
                self.collect_lambdas_in_expr(operand, lambdas);
            }
            Expr::Call { callee, args, .. } => {
                self.collect_lambdas_in_expr(callee, lambdas);
                for arg in args {
                    self.collect_lambdas_in_expr(&arg.value, lambdas);
                }
            }
            Expr::MethodCall { receiver, args, .. } => {
                self.collect_lambdas_in_expr(receiver, lambdas);
                for arg in args {
                    self.collect_lambdas_in_expr(&arg.value, lambdas);
                }
            }
            Expr::If { condition, then_branch, else_branch, .. } => {
                self.collect_lambdas_in_expr(condition, lambdas);
                self.collect_lambdas_in_block(then_branch, lambdas);
                if let Some(else_br) = else_branch {
                    self.collect_lambdas_in_else_branch(else_br, lambdas);
                }
            }
            Expr::Match { scrutinee, arms, .. } => {
                self.collect_lambdas_in_expr(scrutinee, lambdas);
                for arm in arms {
                    self.collect_lambdas_in_expr(&arm.body, lambdas);
                }
            }
            Expr::Array(elements, _) => {
                for e in elements {
                    self.collect_lambdas_in_expr(e, lambdas);
                }
            }
            Expr::Tuple(elements, _) => {
                for e in elements {
                    self.collect_lambdas_in_expr(e, lambdas);
                }
            }
            Expr::Block(block, _) => {
                self.collect_lambdas_in_block(block, lambdas);
            }
            _ => {}
        }
    }

    fn collect_lambdas_in_else_branch(&mut self, branch: &crate::ast::ElseBranch, lambdas: &mut Vec<(u32, Vec<crate::ast::Param>, Expr)>) {
        match branch {
            crate::ast::ElseBranch::Else(block) => self.collect_lambdas_in_block(block, lambdas),
            crate::ast::ElseBranch::ElseIf(cond, then, next) => {
                self.collect_lambdas_in_expr(cond, lambdas);
                self.collect_lambdas_in_block(then, lambdas);
                if let Some(next_br) = next {
                    self.collect_lambdas_in_else_branch(next_br, lambdas);
                }
            }
        }
    }

    /// Compile a collected lambda into a WASM function and add to funcref table
    fn compile_lambda(&mut self, id: u32, params: &[crate::ast::Param], body: &Expr) -> KainResult<()> {
        // Create function type: all params i64, returns i64
        let wasm_params: Vec<ValType> = params.iter().map(|_| ValType::I64).collect();
        let wasm_results = vec![ValType::I64];
        
        let mut builder = FunctionBuilder::new(&mut self.module.types, &wasm_params, &wasm_results);
        
        // Create parameter locals
        let mut locals = HashMap::new();
        let mut param_local_ids = Vec::new();
        for (i, param) in params.iter().enumerate() {
            let local_id = self.module.locals.add(wasm_params[i]);
            locals.insert(param.name.clone(), local_id);
            param_local_ids.push(local_id);
        }
        
        let tmp_i32 = self.module.locals.add(ValType::I32);
        let tmp_i32_2 = self.module.locals.add(ValType::I32);
        let tmp_i64 = self.module.locals.add(ValType::I64);
        
        let ctx = CompilationContext {
            locals,
            functions: &self.functions,
            string_table: &self.string_table,
            struct_layouts: &self.struct_layouts,
            enum_layouts: &self.enum_layouts,
            memory_id: self.memory_id.unwrap(),
            heap_ptr_global: self.heap_ptr_global,
            tmp_i32,
            tmp_i32_2,
            tmp_i64,
            funcref_table: self.funcref_table,
            lambda_table: &self.lambda_table,
        };
        
        // Compile lambda body
        let mut func_body = builder.func_body();
        self.compile_expr(&ctx, &mut func_body, body)?;
        
        // Finish function
        let func_id = builder.finish(param_local_ids, &mut self.module.funcs);
        
        // Add to function table via elem segment
        let table_index = id; // Use lambda ID as table index
        if let Some(table_id) = self.funcref_table {
            // Add function to table via elem segment
            self.module.elements.add(
                walrus::ElementKind::Active {
                    table: table_id,
                    offset: walrus::ConstExpr::Value(walrus::ir::Value::I32(table_index as i32)),
                },
                walrus::ElementItems::Functions(vec![func_id]),
            );
        }
        
        // Store in lambda_table for lookup during compilation
        self.lambda_table.insert(id, (table_index, func_id));
        
        // Also add to functions map with generated name
        let lambda_name = format!("__lambda_{}", id);
        self.functions.insert(lambda_name, func_id);
        
        Ok(())
    }

    fn declare_function(&mut self, func: &TypedFunction) -> KainResult<()> {
        let (param_types, ret_type) = if let ResolvedType::Function { params, ret, .. } = &func.resolved_type {
            (params, ret)
        } else {
            return Err(KainError::codegen("Expected function type", func.ast.span));
        };

        let wasm_params: Vec<ValType> = param_types.iter().map(|t| self.map_type(t)).collect();
        let wasm_results = if **ret_type == ResolvedType::Unit {
            vec![]
        } else {
            vec![self.map_type(ret_type)]
        };

        // Use FunctionBuilder to create the function correctly with empty body
        let builder = FunctionBuilder::new(&mut self.module.types, &wasm_params, &wasm_results);
        
        // Create parameter locals manually to pass to finish
        let mut param_local_ids = Vec::new();
        for &param_type in &wasm_params {
            param_local_ids.push(self.module.locals.add(param_type));
        }

        let func_id = builder.finish(param_local_ids, &mut self.module.funcs);
        self.functions.insert(func.ast.name.clone(), func_id);

        if matches!(func.ast.visibility, crate::ast::Visibility::Public) {
            self.module.exports.add(&func.ast.name, func_id);
        }

        Ok(())
    }

    fn compile_function_body(&mut self, func: &TypedFunction) -> KainResult<()> {
        let func_id = *self.functions.get(&func.ast.name).unwrap();

        let (param_types, ret_type) = if let ResolvedType::Function { params, ret, .. } = &func.resolved_type {
            (params, ret)
        } else {
            return Ok(()); // Should have failed in declare
        };

        let wasm_params: Vec<ValType> = param_types.iter().map(|t| self.map_type(t)).collect();
        let wasm_results = if **ret_type == ResolvedType::Unit {
            vec![]
        } else {
            vec![self.map_type(ret_type)]
        };

        let mut builder = FunctionBuilder::new(&mut self.module.types, &wasm_params, &wasm_results);
        
        let mut text_locals_map = HashMap::new();
        let mut param_local_ids = Vec::new();

        // 1. Argument Locals
        for (i, param) in func.ast.params.iter().enumerate() {
            let local_id = self.module.locals.add(wasm_params[i]);
            text_locals_map.insert(param.name.clone(), local_id);
            param_local_ids.push(local_id);
        }
        
        // 2. Scan body for Let bindings and pre-allocate locals
        self.preallocate_locals(&func.ast.body, &mut text_locals_map);

        let tmp_i32 = self.module.locals.add(ValType::I32);
        let tmp_i32_2 = self.module.locals.add(ValType::I32);
        let tmp_i64 = self.module.locals.add(ValType::I64);

        let ctx = CompilationContext {
            locals: text_locals_map,
            functions: &self.functions,
            string_table: &self.string_table,
            struct_layouts: &self.struct_layouts,
            enum_layouts: &self.enum_layouts,
            memory_id: self.memory_id.unwrap(),
            heap_ptr_global: self.heap_ptr_global,
            tmp_i32,
            tmp_i32_2,
            tmp_i64,
            funcref_table: self.funcref_table,
            lambda_table: &self.lambda_table,
        };

        // 3. Compile body
        let mut func_body = builder.func_body();
        self.compile_block(&ctx, &mut func_body, &func.ast.body)?;
        
        // Return default value if needed
        if func.ast.body.stmts.is_empty() && !wasm_results.is_empty() {
             match wasm_results[0] {
                 ValType::I64 => func_body.i64_const(0),
                 ValType::I32 => func_body.i32_const(0),
                 ValType::F64 => func_body.f64_const(0.0),
                 ValType::F32 => func_body.f32_const(0.0),
                 _ => func_body.i64_const(0),
             };
        }

        // Finish the builder to get a NEW function ID with the compiled body
        let temp_func_id = builder.finish(param_local_ids, &mut self.module.funcs);

        // 4. Move body from temp function to the reserved function
        // We use a dummy ImportFunction kind to facilitate the swap, 
        // derived from a dummy Global import to avoid circular dependencies with Function imports.
        
        let dummy_type = self.module.types.add(&[], &[]);
        let (_dummy_global_id, dummy_import_id) = self.module.add_import_global("KAIN_internal", "dummy", ValType::I32, false, false);
        
        let dummy_kind = walrus::FunctionKind::Import(walrus::ImportedFunction {
            import: dummy_import_id,
            ty: dummy_type,
        });

        // Swap out the new body from temp_func
        let new_func = self.module.funcs.get_mut(temp_func_id);
        let new_kind = std::mem::replace(&mut new_func.kind, dummy_kind);
        
        // Swap in the new body to the old function
        let old_func = self.module.funcs.get_mut(func_id);
        let _old_kind = std::mem::replace(&mut old_func.kind, new_kind);

        // Clean up
        self.module.funcs.delete(temp_func_id);
        self.module.imports.delete(dummy_import_id);
        // Globals cleanup? module.globals.delete(_dummy_global_id)?
        
        Ok(())
    }

    fn preallocate_locals(&mut self, block: &Block, locals: &mut HashMap<String, LocalId>) {
        for stmt in &block.stmts {
            match stmt {
                Stmt::Let { pattern, value, .. } => {
                     // Recursively find bindings and infer type from value
                     if let crate::ast::Pattern::Binding { name, .. } = pattern {
                        if !locals.contains_key(name) {
                            // Infer type from the assigned value expression
                            let val_type = if let Some(expr) = value {
                                self.infer_wasm_type(expr)
                            } else {
                                ValType::I64 // Default for uninitialized
                            };
                            let local = self.module.locals.add(val_type);
                            locals.insert(name.clone(), local);
                        }
                     }
                }
                Stmt::While { body, .. } => {
                    self.preallocate_locals(body, locals);
                }
                Stmt::For { binding, body, .. } => {
                    // Allocate loop variable
                    if let crate::ast::Pattern::Binding { name, .. } = binding {
                        if !locals.contains_key(name) {
                            let local = self.module.locals.add(ValType::I64);
                            locals.insert(name.clone(), local);
                        }
                    }
                    self.preallocate_locals(body, locals);
                }
                Stmt::Loop { body, .. } => {
                    self.preallocate_locals(body, locals);
                }
                _ => {}
            }
        }
    }

    fn map_type(&self, ty: &ResolvedType) -> ValType {
        match ty {
            ResolvedType::Int(_) => ValType::I64,
            ResolvedType::Float(_) => ValType::F64,
            ResolvedType::Bool => ValType::I32,
            ResolvedType::String => ValType::I32, // Strings are pointers (i32 offset)
            _ => ValType::I64, 
        }
    }

    // Infer WASM ValType from an expression (for local allocation)
    fn infer_wasm_type(&self, expr: &Expr) -> ValType {
        match expr {
            Expr::Int(_, _) => ValType::I64,
            Expr::Float(_, _) => ValType::F64,
            Expr::Bool(_, _) => ValType::I32,
            Expr::String(_, _) => ValType::I32,
            Expr::JSX(_, _) => ValType::I32, // JSX nodes are DOM element IDs (i32)
            Expr::Call { callee, .. } => {
                if let Expr::Ident(name, _) = callee.as_ref() {
                    // Component calls return i32 (DOM node IDs)
                    if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                        return ValType::I32;
                    }
                    // String functions return i32 (pointers)
                    if name == "to_string" || name == "str_concat" {
                        return ValType::I32;
                    }
                    // DOM functions return i32
                    if name.starts_with("dom_") {
                        return ValType::I32;
                    }
                }
                ValType::I64 // Default for other functions
            }
            Expr::Binary { op, .. } => {
                // Most binary ops return same type as operands
                // Comparisons return bool (i32)
                match op {
                    BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Gt |
                    BinaryOp::Le | BinaryOp::Ge | BinaryOp::And | BinaryOp::Or => ValType::I32,
                    _ => ValType::I64,
                }
            }
            Expr::Unary { op, .. } => {
                match op {
                    crate::ast::UnaryOp::Not => ValType::I32,
                    _ => ValType::I64,
                }
            }
            _ => ValType::I64, // Default fallback
        }
    }
    
    /// Allocate a string literal in the data segment
    /// Returns the memory offset where the string starts
    /// Format: [length: 4 bytes][utf8 data]
    fn allocate_string(&mut self, s: &str) -> u32 {
        // Check if string already allocated (deduplication)
        if let Some(&offset) = self.string_table.get(s) {
            return offset;
        }
        
        let offset = self.data_offset;
        let bytes = s.as_bytes();
        let len = bytes.len() as u32;
        
        // Build data: length (4 bytes, little-endian) + string bytes
        let mut data = Vec::with_capacity(4 + bytes.len());
        data.extend_from_slice(&len.to_le_bytes());
        data.extend_from_slice(bytes);
        
        // Add to data segment
        if let Some(memory_id) = self.memory_id {
            self.module.data.add(
                walrus::DataKind::Active {
                    memory: memory_id,
                    offset: walrus::ConstExpr::Value(walrus::ir::Value::I32(offset as i32)),
                },
                data,
            );
        }
        
        // Update offset for next allocation
        self.data_offset += 4 + len;
        // Align to 4 bytes
        self.data_offset = (self.data_offset + 3) & !3;
        
        // Cache for deduplication
        self.string_table.insert(s.to_string(), offset);
        
        offset
    }
    
    // --- Compilation Logic (Stateless regarding Module, uses passed Builder) ---

    fn compile_block(&self, ctx: &CompilationContext, builder: &mut InstrSeqBuilder, block: &Block) -> KainResult<()> {
        for stmt in &block.stmts {
           self.compile_stmt(ctx, builder, stmt)?;
        }
        Ok(())
    }

    fn compile_stmt(&self, ctx: &CompilationContext, builder: &mut InstrSeqBuilder, stmt: &Stmt) -> KainResult<()> {
        match stmt {
            Stmt::Expr(expr) => {
                self.compile_expr(ctx, builder, expr)?;
                // Expression statements discard their result
                builder.drop(); 
            }
            Stmt::Let { value, pattern, .. } => {
                if let Some(val_expr) = value {
                    self.compile_expr(ctx, builder, val_expr)?;
                    if let crate::ast::Pattern::Binding { name, .. } = pattern {
                         if let Some(local_id) = ctx.locals.get(name) {
                             builder.local_set(*local_id);
                         }
                    }
                }
            }
            Stmt::Return(opt_expr, _) => {
                if let Some(expr) = opt_expr {
                    self.compile_expr(ctx, builder, expr)?;
                }
                builder.return_(); 
            }
            Stmt::While { condition, body, .. } => {
                builder.block(None, |block_builder| {
                    let block_id = block_builder.id();
                    
                    block_builder.loop_(None, |loop_builder| {
                        let loop_id = loop_builder.id();
                        
                        if self.compile_expr(ctx, loop_builder, condition).is_err() {
                            return;
                        }

                        loop_builder.unop(walrus::ir::UnaryOp::I32Eqz);
                        loop_builder.br_if(block_id);
                        
                        if self.compile_block(ctx, loop_builder, body).is_err() {
                            return;
                        }

                        loop_builder.br(loop_id);
                    });
                });
            }
            // For loop: `for i in start..end: body`
            // Desugars to: let i = start; while i < end: body; i = i + 1
            Stmt::For { binding, iter, body, span: _ } => {
                // Get the loop variable name
                let loop_var = match binding {
                    crate::ast::Pattern::Binding { name, .. } => name.clone(),
                    _ => "".to_string(),
                };
                
                // Get start and end from range expression
                if let Expr::Range { start, end, inclusive, .. } = iter {
                    let start_expr = start.as_ref().map(|e| e.as_ref());
                    let end_expr = end.as_ref().map(|e| e.as_ref());
                    
                    // Initialize loop variable with start value
                    if let Some(start_e) = start_expr {
                        self.compile_expr(ctx, builder, start_e)?;
                    } else {
                        builder.i64_const(0);
                    }
                    
                    if let Some(local_id) = ctx.locals.get(&loop_var) {
                        builder.local_set(*local_id);
                    }
                    
                    // block { loop { if i >= end: break; body; i++; br loop } }
                    builder.block(None, |block_builder| {
                        let block_id = block_builder.id();
                        
                        block_builder.loop_(None, |loop_builder| {
                            let loop_id = loop_builder.id();
                            
                            // Check condition: i < end (or i <= end if inclusive)
                            if let Some(local_id) = ctx.locals.get(&loop_var) {
                                loop_builder.local_get(*local_id);
                            }
                            
                            if let Some(end_e) = end_expr {
                                if self.compile_expr(ctx, loop_builder, end_e).is_err() {
                                    return;
                                }
                            } else {
                                loop_builder.i64_const(i64::MAX);
                            }
                            
                            // Compare: if i >= end (or i > end if inclusive), break
                            if *inclusive {
                                loop_builder.binop(walrus::ir::BinaryOp::I64GtS);
                            } else {
                                loop_builder.binop(walrus::ir::BinaryOp::I64GeS);
                            }
                            loop_builder.br_if(block_id);
                            
                            // Execute body
                            if self.compile_block(ctx, loop_builder, body).is_err() {
                                return;
                            }
                            
                            // Increment loop variable: i = i + 1
                            if let Some(local_id) = ctx.locals.get(&loop_var) {
                                loop_builder.local_get(*local_id);
                                loop_builder.i64_const(1);
                                loop_builder.binop(walrus::ir::BinaryOp::I64Add);
                                loop_builder.local_set(*local_id);
                            }
                            
                            loop_builder.br(loop_id);
                        });
                    });
                } else {
                    // Non-range iterators not yet supported
                    // For arrays: would need to get length, index each element
                }
            }
            // Infinite loop: `loop: body` - can be exited with break
            Stmt::Loop { body, span: _ } => {
                builder.block(None, |block_builder| {
                    let _block_id = block_builder.id();
                    
                    block_builder.loop_(None, |loop_builder| {
                        let loop_id = loop_builder.id();
                        
                        // Execute body
                        if self.compile_block(ctx, loop_builder, body).is_err() {
                            return;
                        }
                        
                        // Continue loop
                        loop_builder.br(loop_id);
                    });
                });
            }
            // Break statement
            Stmt::Break(_, _) => {
                // Break out of innermost block
                // Note: This is simplified - would need proper block tracking for nested loops
                builder.unreachable(); // Placeholder - real impl needs block ID tracking
            }
            // Continue statement  
            Stmt::Continue(_) => {
                // Jump to loop header
                builder.unreachable(); // Placeholder - real impl needs loop ID tracking
            }
            _ => {}
        }
        Ok(())
    }

    fn is_string_expr(&self, expr: &Expr) -> bool {
        match expr {
            Expr::String(_, _) => true,
            Expr::Call { callee, .. } => {
                if let Expr::Ident(name, _) = callee.as_ref() {
                    name == "to_string" || name == "str_concat" 
                } else {
                    false
                }
            }
            Expr::Binary { op, left, right, .. } => {
                 match op {
                     BinaryOp::Add => self.is_string_expr(left) || self.is_string_expr(right),
                     _ => false
                 }
            }
            _ => false
        }
    }

    // Check if expression produces an i32 (JSX node IDs, bools, etc)
    fn is_i32_expr(&self, expr: &Expr) -> bool {
        match expr {
            Expr::JSX(_, _) => true,
            Expr::Bool(_, _) => true,
            Expr::String(_, _) => true, // Strings are i32 pointers
            Expr::Call { callee, .. } => {
                if let Expr::Ident(name, _) = callee.as_ref() {
                    // Component calls and DOM functions return i32
                    name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                        || name.starts_with("dom_")
                        || name == "to_string" || name == "str_concat"
                } else {
                    false
                }
            }
            // For identifiers, we can't know without context - return false and handle separately
            Expr::Ident(_, _) => false, // Will be checked via is_i32_ident with context
            _ => false
        }
    }

    // Check if an identifier refers to an i32 local (needs module access)
    fn is_i32_local(&self, name: &str, locals: &HashMap<String, LocalId>) -> bool {
        if let Some(local_id) = locals.get(name) {
            // Check the local's type in the module
            let local = self.module.locals.get(*local_id);
            return local.ty() == ValType::I32;
        }
        false
    }

    fn compile_expr(&self, ctx: &CompilationContext, builder: &mut InstrSeqBuilder, expr: &Expr) -> KainResult<()> {
        match expr {
            Expr::Int(n, _) => {
                builder.i64_const(*n);
            }
            Expr::Float(f, _) => {
                builder.f64_const(*f);
            }
            Expr::Bool(b, _) => {
                builder.i32_const(if *b { 1 } else { 0 });
            }
            Expr::String(s, span) => {
                // String literals are stored in data segment during pre-pass
                // Here we just emit the memory offset as an i32
                if let Some(&offset) = ctx.string_table.get(s) {
                    builder.i32_const((offset + 4) as i32); // Return pointer to data (skip length)
                } else {
                    return Err(KainError::codegen("String not found in table", *span));
                }
            }
            Expr::Binary { left, op, right, .. } => {
                self.compile_expr(ctx, builder, left)?;
                self.compile_expr(ctx, builder, right)?;
                match op {
                    // Arithmetic
                    BinaryOp::Add => { 
                        if self.is_string_expr(left) || self.is_string_expr(right) {
                            if let Some(func_id) = ctx.functions.get("str_concat") {
                                builder.call(*func_id);
                            }
                        } else {
                            builder.binop(walrus::ir::BinaryOp::I64Add); 
                        }
                    },
                    BinaryOp::Sub => { builder.binop(walrus::ir::BinaryOp::I64Sub); },
                    BinaryOp::Mul => { builder.binop(walrus::ir::BinaryOp::I64Mul); },
                    BinaryOp::Div => { builder.binop(walrus::ir::BinaryOp::I64DivS); },
                    BinaryOp::Mod => { builder.binop(walrus::ir::BinaryOp::I64RemS); },
                    // Comparison
                    BinaryOp::Eq => { builder.binop(walrus::ir::BinaryOp::I64Eq); },
                    BinaryOp::Ne => { builder.binop(walrus::ir::BinaryOp::I64Ne); },
                    BinaryOp::Lt => { builder.binop(walrus::ir::BinaryOp::I64LtS); },
                    BinaryOp::Gt => { builder.binop(walrus::ir::BinaryOp::I64GtS); },
                    BinaryOp::Le => { builder.binop(walrus::ir::BinaryOp::I64LeS); },
                    BinaryOp::Ge => { builder.binop(walrus::ir::BinaryOp::I64GeS); },
                    // Logical (short-circuit would need control flow, treat as bitwise for now)
                    BinaryOp::And => { builder.binop(walrus::ir::BinaryOp::I64And); },
                    BinaryOp::Or => { builder.binop(walrus::ir::BinaryOp::I64Or); },
                    // Bitwise
                    BinaryOp::BitAnd => { builder.binop(walrus::ir::BinaryOp::I64And); },
                    BinaryOp::BitOr => { builder.binop(walrus::ir::BinaryOp::I64Or); },
                    BinaryOp::BitXor => { builder.binop(walrus::ir::BinaryOp::I64Xor); },
                    BinaryOp::Shl => { builder.binop(walrus::ir::BinaryOp::I64Shl); },
                    BinaryOp::Shr => { builder.binop(walrus::ir::BinaryOp::I64ShrS); },
                     _ => {}
                }
            }
            Expr::Unary { op, operand, .. } => {
                use crate::ast::UnaryOp;
                match op {
                    UnaryOp::Neg => { 
                        // -x = 0 - x: push 0 first, then operand, then sub
                        builder.i64_const(0);
                        self.compile_expr(ctx, builder, operand)?;
                        builder.binop(walrus::ir::BinaryOp::I64Sub);
                    },
                    UnaryOp::Not => {
                        // !x = x == 0 (logical not)
                        self.compile_expr(ctx, builder, operand)?;
                        builder.unop(walrus::ir::UnaryOp::I64Eqz);
                    },
                    UnaryOp::BitNot => {
                        // ~x = x xor -1
                        self.compile_expr(ctx, builder, operand)?;
                        builder.i64_const(-1);
                        builder.binop(walrus::ir::BinaryOp::I64Xor);
                    },
                    _ => {
                        // Ref, Deref - just compile operand for now
                        self.compile_expr(ctx, builder, operand)?;
                    }
                }
            }
            Expr::Ident(name, span) => {
                if let Some(local_id) = ctx.locals.get(name) {
                    builder.local_get(*local_id);
                } else {
                     return Err(KainError::codegen(format!("Variable '{}' not found in locals", name), *span));
                }
            }
            Expr::If { condition, then_branch, else_branch, .. } => {
                 self.compile_expr(ctx, builder, condition)?;
                 
                 builder.if_else(
                    None, 
                    |then_builder| {
                        let _ = self.compile_block(ctx, then_builder, then_branch);
                    },
                    |else_builder| {
                        if let Some(else_br) = else_branch {
                            let _ = self.compile_else_branch(ctx, else_builder, else_br);
                        }
                    }
                 );
            }
            Expr::JSX(node, _) => {
                self.compile_jsx_node(ctx, builder, node)?;
            }
            Expr::Call { callee, args, span } => {
                // Get function name from callee
                if let Expr::Ident(func_name, _) = callee.as_ref() {
                    // Special intrinsic: print
                    if func_name == "print" {
                        for arg in args {
                            match &arg.value {
                                Expr::Int(_, _) => {
                                    self.compile_expr(ctx, builder, &arg.value)?;
                                    if let Some(func_id) = ctx.functions.get("print_i64") {
                                        builder.call(*func_id);
                                    }
                                }
                                Expr::Float(_, _) => {
                                    self.compile_expr(ctx, builder, &arg.value)?;
                                    if let Some(func_id) = ctx.functions.get("print_f64") {
                                        builder.call(*func_id);
                                    }
                                }
                                Expr::Bool(_, _) => {
                                    self.compile_expr(ctx, builder, &arg.value)?;
                                    if let Some(func_id) = ctx.functions.get("print_bool") {
                                        builder.call(*func_id);
                                    }
                                }
                                Expr::String(s, _) => {
                                    if let Some(&offset) = ctx.string_table.get(s) {
                                        builder.i32_const((offset + 4) as i32);
                                        builder.i32_const(s.len() as i32);
                                        if let Some(func_id) = ctx.functions.get("print_str") {
                                            builder.call(*func_id);
                                        }
                                    }
                                }
                                _ => {
                                    let is_string = self.is_string_expr(&arg.value);
                                    
                                    // Check if this is an i32 variable (JSX, bool, string ptr)
                                    let is_i32_var = match &arg.value {
                                        Expr::Ident(name, _) => self.is_i32_local(name, &ctx.locals),
                                        _ => self.is_i32_expr(&arg.value),
                                    };
                                    
                                    self.compile_expr(ctx, builder, &arg.value)?;
                                    
                                    if is_string {
                                        // ptr is on stack. Len is at ptr - 4.
                                        builder.local_set(ctx.tmp_i32);
                                        builder.local_get(ctx.tmp_i32); // ptr
                                        
                                        builder.local_get(ctx.tmp_i32);
                                        builder.i32_const(4);
                                        builder.binop(walrus::ir::BinaryOp::I32Sub);
                                        builder.load(ctx.memory_id, walrus::ir::LoadKind::I32 { atomic: false }, walrus::ir::MemArg { align: 4, offset: 0 }); // len
                                        
                                        if let Some(func_id) = ctx.functions.get("print_str") {
                                            builder.call(*func_id);
                                        }
                                    } else if is_i32_var {
                                        // JSX nodes, bools, components return i32
                                        // For JSX the rendering already happened, just drop the node ID
                                        builder.drop();
                                    } else {
                                        if let Some(func_id) = ctx.functions.get("print_i64") {
                                            builder.call(*func_id);
                                        }
                                    }
                                }
                            }
                        }
                        builder.i64_const(0); // Return Unit/0
                        return Ok(());
                    }

                    // Special intrinsic: to_string
                    if func_name == "to_string" {
                        if let Some(arg) = args.first() {
                             self.compile_expr(ctx, builder, &arg.value)?;
                             if let Some(func_id) = ctx.functions.get("int_to_str") {
                                 builder.call(*func_id);
                             }
                        } else {
                            builder.i32_const(0);
                        }
                        return Ok(());
                    }

                    // Special intrinsic: now
                    if func_name == "now" {
                        if let Some(func_id) = ctx.functions.get("time_now") {
                            builder.call(*func_id);
                        }
                        return Ok(());
                    }

                    // Look up function ID
                    if let Some(func_id) = ctx.functions.get(func_name) {
                        // Compile arguments (push onto stack)
                        for arg in args {
                            self.compile_expr(ctx, builder, &arg.value)?;
                        }
                        // Emit call instruction
                        builder.call(*func_id);
                    } else {
                        return Err(KainError::codegen(format!("Function '{}' not found", func_name), *span));
                    }
                } else {
                    // For now, only support direct function calls by name
                    return Err(KainError::codegen("Only direct function calls supported in WASM", *span));
                }
            }
            // Struct literal: allocate memory and initialize fields
            Expr::EnumVariant { enum_name, variant, fields, span } => {
                if let Some((tags, max_payload, field_offsets_map)) = ctx.enum_layouts.get(enum_name) {
                     let tag = *tags.get(variant).ok_or_else(|| KainError::codegen("Variant tag not found", *span))?;
                     
                     // 4 bytes tag + payload
                     let total_size = 4 + max_payload;
                     self.emit_alloc(ctx, builder, total_size);
                     // Stack: [base_ptr]
                     
                     // Drop base_ptr to recompute for stores
                     builder.drop();

                     // Store tag at offset 0
                     let aligned_size = (total_size + 7) & !7;
                     
                     builder.global_get(ctx.heap_ptr_global);
                     builder.i32_const(aligned_size as i32);
                     builder.binop(walrus::ir::BinaryOp::I32Sub);
                     
                     builder.i32_const(tag as i32);
                     builder.store(
                         ctx.memory_id,
                         walrus::ir::StoreKind::I32 { atomic: false },
                         walrus::ir::MemArg { align: 4, offset: 0 },
                     );

                     match fields {
                         crate::ast::EnumVariantFields::Unit => {},
                         crate::ast::EnumVariantFields::Tuple(exprs) => {
                             let variant_offsets = field_offsets_map.get(variant).expect("Variant offsets missing");
                             for (i, expr) in exprs.iter().enumerate() {
                                 if let Some(&offset) = variant_offsets.get(&i.to_string()) {
                                     builder.global_get(ctx.heap_ptr_global);
                                     builder.i32_const(aligned_size as i32);
                                     builder.binop(walrus::ir::BinaryOp::I32Sub);
                                     builder.i32_const((4 + offset) as i32);
                                     builder.binop(walrus::ir::BinaryOp::I32Add);
                                     
                                     self.compile_expr(ctx, builder, expr)?;
                                     self.emit_store_for_expr(ctx, builder, expr, 0); 
                                 }
                             }
                         },
                         crate::ast::EnumVariantFields::Struct(named_fields) => {
                             let variant_offsets = field_offsets_map.get(variant).expect("Variant offsets missing");
                             for (name, expr) in named_fields {
                                 if let Some(&offset) = variant_offsets.get(name) {
                                     builder.global_get(ctx.heap_ptr_global);
                                     builder.i32_const(aligned_size as i32);
                                     builder.binop(walrus::ir::BinaryOp::I32Sub);
                                     builder.i32_const((4 + offset) as i32);
                                     builder.binop(walrus::ir::BinaryOp::I32Add);
                                     
                                     self.compile_expr(ctx, builder, expr)?;
                                     self.emit_store_for_expr(ctx, builder, expr, 0);
                                 }
                             }
                         }
                     }

                     // Return base pointer
                     builder.global_get(ctx.heap_ptr_global);
                     builder.i32_const(aligned_size as i32);
                     builder.binop(walrus::ir::BinaryOp::I32Sub);
                } else {
                    return Err(KainError::codegen(format!("Enum layout not found for {}", enum_name), *span));
                }
            }
            Expr::Struct { name, fields, span } => {
                if let Some((field_offsets, total_size)) = ctx.struct_layouts.get(name).cloned() {
                    // Allocate memory for struct using bump allocator
                    self.emit_alloc(ctx, builder, total_size);
                    // Stack: [base_ptr]
                    
                    // We need to keep base_ptr for field stores AND return it
                    // Strategy: for each field, dup the ptr, add offset, store
                    // But walrus doesn't have dup... so we emit base_ptr before each store
                    
                    // Store fields: emit [addr, value] then store
                    for (field_name, field_expr) in fields {
                        if let Some(&field_offset) = field_offsets.get(field_name) {
                            // Emit base_ptr + offset for store address
                            builder.global_get(ctx.heap_ptr_global);
                            // Need to subtract total_size to get back to our base
                            // Actually, heap_ptr now points PAST our allocation
                            // Our base = heap_ptr - aligned_size
                            // Simpler: re-emit the base calculation
                            
                            // Get the base we just allocated (heap_ptr - aligned_total_size)
                            let aligned_size = (total_size + 7) & !7;
                            builder.i32_const(aligned_size as i32);
                            builder.binop(walrus::ir::BinaryOp::I32Sub);
                            builder.i32_const(field_offset as i32);
                            builder.binop(walrus::ir::BinaryOp::I32Add);
                            // Stack: [field_addr]
                            
                            // Compile the field value
                            self.compile_expr(ctx, builder, field_expr)?;
                            // Stack: [field_addr, value]
                            
                            // Store (assumes i64 for now)
                            builder.store(
                                ctx.memory_id,
                                walrus::ir::StoreKind::I64 { atomic: false },
                                walrus::ir::MemArg { align: 8, offset: 0 },
                            );
                        }
                    }
                    
                    // Leave struct pointer on stack (base address)
                    let aligned_size = (total_size + 7) & !7;
                    builder.global_get(ctx.heap_ptr_global);
                    builder.i32_const(aligned_size as i32);
                    builder.binop(walrus::ir::BinaryOp::I32Sub);
                } else {
                    return Err(KainError::codegen(format!("Struct '{}' layout not found", name), *span));
                }
            }
            // Field access: load from struct pointer + offset
            Expr::Field { object, field, span: _ } => {
                // Compile the object to get struct pointer
                self.compile_expr(ctx, builder, object)?;
                // Stack: [ptr]
                
                // Try to find field offset from any struct layout
                // This is a heuristic - proper impl would use type info
                let mut field_offset = 0u32;
                let mut found = false;
                for (_struct_name, (offsets, _size)) in ctx.struct_layouts.iter() {
                    if let Some(&offset) = offsets.get(field) {
                        field_offset = offset;
                        found = true;
                        break;
                    }
                }
                
                if found && field_offset > 0 {
                    builder.i32_const(field_offset as i32);
                    builder.binop(walrus::ir::BinaryOp::I32Add);
                }
                
                // Load value from memory (default to i64)
                builder.load(
                    ctx.memory_id,
                    walrus::ir::LoadKind::I64 { atomic: false },
                    walrus::ir::MemArg { align: 8, offset: 0 },
                );
            }
            // Method call: obj.method(args) desugars to Type.method(obj, args)
            Expr::MethodCall { receiver, method, args, span } => {
                // Compile the receiver (self)
                self.compile_expr(ctx, builder, receiver)?;
                
                // Compile arguments
                for arg in args {
                    self.compile_expr(ctx, builder, &arg.value)?;
                }
                
                // Look for method in functions map
                // Methods are typically named "TypeName.method_name"
                // For now, try just the method name
                if let Some(func_id) = ctx.functions.get(method) {
                    builder.call(*func_id);
                } else {
                    // Method not found - leave result on stack as placeholder
                    // Real impl would look for impl blocks
                    return Err(KainError::codegen(format!("Method '{}' not found", method), *span));
                }
            }
            // Array literal: allocate memory and store length + elements
            Expr::Array(elements, _span) => {
                let len = elements.len() as u32;
                let element_size = 8u32; // i64 elements
                let total_size = 4 + (len * element_size); // 4 bytes for length + elements
                let aligned_size = (total_size + 7) & !7;
                
                // Allocate using bump allocator
                self.emit_alloc(ctx, builder, total_size);
                // Stack: [base_ptr] - but emit_alloc leaves OLD ptr, heap_ptr is now past us
                // Actually emit_alloc returns old heap_ptr which IS our base. Perfect!
                
                // Drop the base_ptr from stack for now, we'll recompute for stores
                builder.drop();
                
                // Compute base address: heap_ptr - aligned_size
                let get_base = |b: &mut InstrSeqBuilder, hp: walrus::GlobalId, sz: u32| {
                    b.global_get(hp);
                    b.i32_const(sz as i32);
                    b.binop(walrus::ir::BinaryOp::I32Sub);
                };
                
                // Store length at base
                get_base(builder, ctx.heap_ptr_global, aligned_size);
                builder.i32_const(len as i32);
                builder.store(
                    ctx.memory_id,
                    walrus::ir::StoreKind::I32 { atomic: false },
                    walrus::ir::MemArg { align: 4, offset: 0 },
                );
                
                // Store each element
                for (i, elem) in elements.iter().enumerate() {
                    // Address = base + 4 + (i * 8)
                    get_base(builder, ctx.heap_ptr_global, aligned_size);
                    builder.i32_const((4 + i as u32 * element_size) as i32);
                    builder.binop(walrus::ir::BinaryOp::I32Add);
                    
                    self.compile_expr(ctx, builder, elem)?;
                    builder.store(
                        ctx.memory_id,
                        walrus::ir::StoreKind::I64 { atomic: false },
                        walrus::ir::MemArg { align: 8, offset: 0 },
                    );
                }
                
                // Leave array pointer on stack
                get_base(builder, ctx.heap_ptr_global, aligned_size);
            }
            // Index access: arr[i] - load from array pointer + 4 + (i * 8)
            Expr::Index { object, index, span: _ } => {
                // Compile array pointer
                self.compile_expr(ctx, builder, object)?;
                // Save to compute address: base + 4 + (index * 8)
                // Stack: [base_ptr]
                
                builder.i32_const(4); // Skip length field
                builder.binop(walrus::ir::BinaryOp::I32Add);
                // Stack: [base_ptr + 4]
                
                // Compile index
                self.compile_expr(ctx, builder, index)?;
                // Convert i64 index to i32 for address calculation
                builder.unop(walrus::ir::UnaryOp::I32WrapI64);
                builder.i32_const(8); // element size
                builder.binop(walrus::ir::BinaryOp::I32Mul);
                // Stack: [base_ptr + 4, index * 8]
                
                builder.binop(walrus::ir::BinaryOp::I32Add);
                // Stack: [base_ptr + 4 + index * 8]
                
                // Load i64 element
                builder.load(
                    ctx.memory_id,
                    walrus::ir::LoadKind::I64 { atomic: false },
                    walrus::ir::MemArg { align: 8, offset: 0 },
                );
            }
            // Tuple literal: allocate memory and store elements (like struct with indexed fields)
            Expr::Tuple(elements, _span) => {
                let len = elements.len() as u32;
                let element_size = 8u32; // All elements i64 for now
                let total_size = len * element_size;
                let aligned_size = (total_size + 7) & !7;
                
                // Allocate
                self.emit_alloc(ctx, builder, total_size);
                builder.drop(); // We'll recompute base for each store
                
                // Store each element
                for (i, elem) in elements.iter().enumerate() {
                    // Address = heap_ptr - aligned_size + (i * 8)
                    builder.global_get(ctx.heap_ptr_global);
                    builder.i32_const(aligned_size as i32);
                    builder.binop(walrus::ir::BinaryOp::I32Sub);
                    builder.i32_const((i as u32 * element_size) as i32);
                    builder.binop(walrus::ir::BinaryOp::I32Add);
                    
                    self.compile_expr(ctx, builder, elem)?;
                    builder.store(
                        ctx.memory_id,
                        walrus::ir::StoreKind::I64 { atomic: false },
                        walrus::ir::MemArg { align: 8, offset: 0 },
                    );
                }
                
                // Leave tuple pointer on stack
                builder.global_get(ctx.heap_ptr_global);
                builder.i32_const(aligned_size as i32);
                builder.binop(walrus::ir::BinaryOp::I32Sub);
            }
            // Match expression: compile as chained if-else
            Expr::Match { scrutinee, arms, span: _ } => {
                // Compile scrutinee and store in temp local
                self.compile_expr(ctx, builder, scrutinee)?;
                builder.local_set(ctx.tmp_i32);
                
                // Build nested if-else chain for arms
                // Each arm: check pattern, if matches execute body
                // We'll use a simple approach: each arm is an if/else
                
                for (i, arm) in arms.iter().enumerate() {
                    let is_last = i == arms.len() - 1;
                    
                    match &arm.pattern {
                        crate::ast::Pattern::Wildcard(_) => {
                            // Wildcard always matches - just emit the body
                            self.compile_expr(ctx, builder, &arm.body)?;
                        }
                        crate::ast::Pattern::Literal(lit_expr) => {
                            // Compare scrutinee with literal
                            builder.local_get(ctx.tmp_i32);
                            self.compile_expr(ctx, builder, lit_expr)?;
                            // Wrap i64 to i32 for comparison if needed
                            builder.unop(walrus::ir::UnaryOp::I32WrapI64);
                            builder.binop(walrus::ir::BinaryOp::I32Eq);
                            
                            if is_last {
                                // Last arm: just emit body conditionally
                                builder.if_else(
                                    None,
                                    |then_b| { let _ = self.compile_expr(ctx, then_b, &arm.body); },
                                    |_else_b| {}
                                );
                            } else {
                                builder.if_else(
                                    None,
                                    |then_b| { let _ = self.compile_expr(ctx, then_b, &arm.body); },
                                    |_else_b| {
                                        // Continue to next arm - but we can't recurse easily here
                                        // For now, just leave empty - full impl needs restructuring
                                    }
                                );
                            }
                        }
                        crate::ast::Pattern::Binding { name, .. } => {
                            // Binding: bind scrutinee to local and execute body
                            if let Some(local_id) = ctx.locals.get(name) {
                                builder.local_get(ctx.tmp_i32);
                                builder.unop(walrus::ir::UnaryOp::I64ExtendSI32); // Convert back to i64
                                builder.local_set(*local_id);
                            }
                            self.compile_expr(ctx, builder, &arm.body)?;
                        }
                        crate::ast::Pattern::Variant { variant, .. } => {
                            // For enum patterns: load tag, compare with variant tag
                            // Load tag from scrutinee pointer
                            builder.local_get(ctx.tmp_i32);
                            builder.load(
                                ctx.memory_id,
                                walrus::ir::LoadKind::I32 { atomic: false },
                                walrus::ir::MemArg { align: 4, offset: 0 },
                            );
                            
                            // TODO: look up variant tag from enum_layouts
                            // For now just use the variant name hash as placeholder
                            let tag = variant.len() as i32 % 256; // Placeholder
                            builder.i32_const(tag);
                            builder.binop(walrus::ir::BinaryOp::I32Eq);
                            
                            builder.if_else(
                                None,
                                |then_b| { let _ = self.compile_expr(ctx, then_b, &arm.body); },
                                |_else_b| {}
                            );
                        }
                        _ => {
                            // Other patterns: just emit body (fallback)
                            self.compile_expr(ctx, builder, &arm.body)?;
                        }
                    }
                }
            }
            // MacroCall: handle println!, print!, dbg!
            Expr::MacroCall { name, args, span: _ } => {
                match name.as_str() {
                    "println" | "print" => {
                        // For each argument, determine type and call appropriate print function
                        for arg in args {
                            match arg {
                                Expr::Int(_, _) => {
                                    self.compile_expr(ctx, builder, arg)?;
                                    if let Some(func_id) = ctx.functions.get("print_i64") {
                                        builder.call(*func_id);
                                    }
                                }
                                Expr::Float(_, _) => {
                                    self.compile_expr(ctx, builder, arg)?;
                                    if let Some(func_id) = ctx.functions.get("print_f64") {
                                        builder.call(*func_id);
                                    }
                                }
                                Expr::Bool(_, _) => {
                                    self.compile_expr(ctx, builder, arg)?;
                                    if let Some(func_id) = ctx.functions.get("print_bool") {
                                        builder.call(*func_id);
                                    }
                                }
                                Expr::String(s, _) => {
                                    // For strings, we need ptr and len
                                    if let Some(&offset) = ctx.string_table.get(s) {
                                        // Push pointer (offset + 4 to skip length prefix)
                                        builder.i32_const((offset + 4) as i32);
                                        // Push length
                                        builder.i32_const(s.len() as i32);
                                        if let Some(func_id) = ctx.functions.get("print_str") {
                                            builder.call(*func_id);
                                        }
                                    }
                                }
                                Expr::Ident(_, _) => {
                                    // For variables, compile and assume i64 for now
                                    self.compile_expr(ctx, builder, arg)?;
                                    if let Some(func_id) = ctx.functions.get("print_i64") {
                                        builder.call(*func_id);
                                    }
                                }
                                _ => {
                                    // Default: compile and print as i64
                                    self.compile_expr(ctx, builder, arg)?;
                                    if let Some(func_id) = ctx.functions.get("print_i64") {
                                        builder.call(*func_id);
                                    }
                                }
                            }
                        }
                        // Push a dummy value since expressions need to produce something
                        builder.i64_const(0);
                    }
                    "dbg" => {
                        // Debug: print and return the value
                        if let Some(arg) = args.first() {
                            self.compile_expr(ctx, builder, arg)?;
                            // Duplicate for print and return
                            // Actually can't dup easily, so just print
                            if let Some(func_id) = ctx.functions.get("print_i64") {
                                builder.call(*func_id);
                            }
                        }
                        builder.i64_const(0);
                    }
                    _ => {
                        // Unknown macro - just push 0
                        builder.i64_const(0);
                    }
                }
            }
            // Range expression: for now just push start value since ranges are handled inline in for loops
            Expr::Range { start, end: _, inclusive: _, span: _ } => {
                // Ranges are typically used inline in for loops
                // If used standalone, just return the start value
                if let Some(start_expr) = start {
                    self.compile_expr(ctx, builder, start_expr)?;
                } else {
                    builder.i64_const(0);
                }
            }
            // Lambda expression: return table index for the pre-compiled lambda function
            Expr::Lambda { params, return_type: _, body: _, span: _ } => {
                // Lambdas are compiled in pre-pass and stored in lambda_table
                // Find the lambda by matching parameter count (simplified - proper impl would use unique IDs)
                // For now, we need to track which lambda this is
                // 
                // Since lambdas are assigned IDs in order during collection,
                // we need to find which ID this lambda has
                // This is a limitation - proper impl would tag each lambda AST with an ID
                //
                // For now, push the table index based on param count heuristic
                // This works if lambdas are unique by param count
                let _param_count = params.len() as u32;
                
                // Search lambda_table for a lambda with matching param count
                let mut found_index = 0i32;
                for (_id, (table_idx, _func_id)) in ctx.lambda_table.iter() {
                    // Simple heuristic: use first lambda if param counts can't be matched
                    found_index = *table_idx as i32;
                    break; // TODO: proper ID tracking
                }
                
                // Push table index as i32 (for call_indirect)
                builder.i32_const(found_index);
            }
            // Block expression: compile all statements, return last expression value
            Expr::Block(block, _span) => {
                // Compile all statements except the last
                for (i, stmt) in block.stmts.iter().enumerate() {
                    if i < block.stmts.len() - 1 {
                        self.compile_stmt(ctx, builder, stmt)?;
                    } else {
                        // Last statement - if it's an expression, keep its value
                        if let Stmt::Expr(expr) = stmt {
                            self.compile_expr(ctx, builder, expr)?;
                        } else {
                            self.compile_stmt(ctx, builder, stmt)?;
                            builder.i64_const(0); // Block returns unit
                        }
                    }
                }
                if block.stmts.is_empty() {
                    builder.i64_const(0);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn compile_else_branch(&self, ctx: &CompilationContext, builder: &mut InstrSeqBuilder, branch: &crate::ast::ElseBranch) -> KainResult<()> {
        match branch {
            crate::ast::ElseBranch::Else(block) => {
                let _ = self.compile_block(ctx, builder, block);
            }
            crate::ast::ElseBranch::ElseIf(cond, then, next_else) => {
                self.compile_expr(ctx, builder, cond)?;
                
                builder.if_else(
                    None, 
                    |then_builder| {
                        let _ = self.compile_block(ctx, then_builder, then);
                    },
                    |else_builder| {
                        if let Some(next) = next_else {
                            let _ = self.compile_else_branch(ctx, else_builder, next);
                        }
                    }
                );
            }
        }
        Ok(())
    }

    fn compile_jsx_node(&self, ctx: &CompilationContext, builder: &mut InstrSeqBuilder, node: &crate::ast::JSXNode) -> KainResult<()> {
        match node {
            crate::ast::JSXNode::Element { tag, attributes, children, .. } => {
                // 1. Compile Children
                for child in children {
                    self.compile_jsx_node(ctx, builder, child)?;
                }
                
                // 2. Allocate Children Array
                let child_count = children.len() as u32;
                let children_size = 4 + (child_count * 4);
                self.emit_alloc(ctx, builder, children_size);
                builder.local_set(ctx.tmp_i32); // Save array ptr
                
                // Store children (Reverse order because they are on stack)
                for i in (0..child_count).rev() {
                    // Stack: [.., child_val]
                    builder.local_set(ctx.tmp_i32_2); // Pop child val
                    
                    // Addr = base + 4 + i*4
                    builder.local_get(ctx.tmp_i32);
                    builder.i32_const((4 + i * 4) as i32);
                    builder.binop(walrus::ir::BinaryOp::I32Add);
                    
                    builder.local_get(ctx.tmp_i32_2); // Val
                    
                    builder.store(ctx.memory_id, walrus::ir::StoreKind::I32 { atomic: false }, walrus::ir::MemArg { align: 4, offset: 0 });
                }
                
                // Store length
                builder.local_get(ctx.tmp_i32);
                builder.i32_const(child_count as i32);
                builder.store(ctx.memory_id, walrus::ir::StoreKind::I32 { atomic: false }, walrus::ir::MemArg { align: 4, offset: 0 });
                
                // Keep Children Array Ptr on stack (Wait, we stored it in tmp_i32, but we need to push it back)
                // BUT we have Props to compile. If Props use tmp_i32, we lose it.
                // We MUST push it to stack now.
                builder.local_get(ctx.tmp_i32);
                // Stack: [children_ptr]
                
                // 3. Compile Props
                let props_count = attributes.len() as u32;
                for attr in attributes {
                     // Key
                     if let Some(&offset) = ctx.string_table.get(&attr.name) {
                         builder.i32_const((offset + 4) as i32);
                     } else {
                         builder.i32_const(0);
                     }
                     
                     // Value
                     match &attr.value {
                         crate::ast::JSXAttrValue::String(s) => {
                             if let Some(&offset) = ctx.string_table.get(s) {
                                 builder.i32_const((offset + 4) as i32);
                             } else {
                                 builder.i32_const(0);
                             }
                             builder.unop(walrus::ir::UnaryOp::I64ExtendUI32);
                         },
                         crate::ast::JSXAttrValue::Expr(e) => {
                             self.compile_expr(ctx, builder, e)?;
                         },
                         crate::ast::JSXAttrValue::Bool(b) => {
                             builder.i64_const(if *b { 1 } else { 0 });
                         }
                     }
                }
                
                // Allocate Props Array
                let props_item_size = 12;
                let props_size = 4 + (props_count * props_item_size);
                self.emit_alloc(ctx, builder, props_size);
                builder.local_set(ctx.tmp_i32); // Save props array ptr
                
                // Store Props (Reverse)
                for i in (0..props_count).rev() {
                    builder.local_set(ctx.tmp_i64); // Pop val (i64)
                    builder.local_set(ctx.tmp_i32_2); // Pop key (i32)
                    
                    // Store Key
                    builder.local_get(ctx.tmp_i32);
                    builder.i32_const((4 + i * props_item_size) as i32);
                    builder.binop(walrus::ir::BinaryOp::I32Add);
                    builder.local_get(ctx.tmp_i32_2);
                    builder.store(ctx.memory_id, walrus::ir::StoreKind::I32 { atomic: false }, walrus::ir::MemArg { align: 4, offset: 0 });

                    // Store Val
                    builder.local_get(ctx.tmp_i32);
                    builder.i32_const((4 + i * props_item_size + 4) as i32);
                    builder.binop(walrus::ir::BinaryOp::I32Add);
                    builder.local_get(ctx.tmp_i64);
                    builder.store(ctx.memory_id, walrus::ir::StoreKind::I64 { atomic: false }, walrus::ir::MemArg { align: 8, offset: 0 });
                }
                
                // Store Props Length
                builder.local_get(ctx.tmp_i32);
                builder.i32_const(props_count as i32);
                builder.store(ctx.memory_id, walrus::ir::StoreKind::I32 { atomic: false }, walrus::ir::MemArg { align: 4, offset: 0 });
                
                // Push Props Ptr
                builder.local_get(ctx.tmp_i32);
                
                // Stack: [children_ptr, props_ptr]
                
                // 4. Allocate VNode (16 bytes)
                self.emit_alloc(ctx, builder, 16);
                builder.local_set(ctx.tmp_i32); // VNode Ptr
                
                // Store Props Ptr (offset 8)
                // Stack: [children_ptr, props_ptr]
                builder.local_set(ctx.tmp_i32_2); // props_ptr
                
                builder.local_get(ctx.tmp_i32);
                builder.i32_const(8);
                builder.binop(walrus::ir::BinaryOp::I32Add);
                builder.local_get(ctx.tmp_i32_2);
                builder.store(ctx.memory_id, walrus::ir::StoreKind::I32 { atomic: false }, walrus::ir::MemArg { align: 4, offset: 0 });
                
                // Store Children Ptr (offset 12)
                // Stack: [children_ptr]
                builder.local_set(ctx.tmp_i32_2); // children_ptr
                
                builder.local_get(ctx.tmp_i32);
                builder.i32_const(12);
                builder.binop(walrus::ir::BinaryOp::I32Add);
                builder.local_get(ctx.tmp_i32_2);
                builder.store(ctx.memory_id, walrus::ir::StoreKind::I32 { atomic: false }, walrus::ir::MemArg { align: 4, offset: 0 });
                
                // Store Type = 1 (Element) (offset 0)
                builder.local_get(ctx.tmp_i32);
                builder.i32_const(1);
                builder.store(ctx.memory_id, walrus::ir::StoreKind::I32 { atomic: false }, walrus::ir::MemArg { align: 4, offset: 0 });
                
                // Store Tag (offset 4)
                let tag_ptr = if let Some(&offset) = ctx.string_table.get(tag) { offset + 4 } else { 0 };
                builder.local_get(ctx.tmp_i32);
                builder.i32_const(tag_ptr as i32);
                builder.store(ctx.memory_id, walrus::ir::StoreKind::I32 { atomic: false }, walrus::ir::MemArg { align: 4, offset: 4 });
                
                // Return VNode Ptr
                builder.local_get(ctx.tmp_i32);
            }
            crate::ast::JSXNode::Text(s, _) => {
                self.emit_alloc(ctx, builder, 16);
                builder.local_set(ctx.tmp_i32);
                
                builder.local_get(ctx.tmp_i32);
                builder.i32_const(0); // Type = 0 (Text)
                builder.store(ctx.memory_id, walrus::ir::StoreKind::I32 { atomic: false }, walrus::ir::MemArg { align: 4, offset: 0 });
                
                let text_ptr = if let Some(&offset) = ctx.string_table.get(s) { offset + 4 } else { 0 };
                builder.local_get(ctx.tmp_i32);
                builder.i32_const(text_ptr as i32);
                builder.store(ctx.memory_id, walrus::ir::StoreKind::I32 { atomic: false }, walrus::ir::MemArg { align: 4, offset: 12 }); // Store in text field (offset 12)
                
                builder.local_get(ctx.tmp_i32);
            }
            crate::ast::JSXNode::Expression(e) => {
                 self.compile_expr(ctx, builder, e)?;
                 builder.unop(walrus::ir::UnaryOp::I32WrapI64);
            }
            _ => {
                 builder.i32_const(0);
            }
        }
        Ok(())
    }

    fn emit_store_for_expr(&self, ctx: &CompilationContext, builder: &mut InstrSeqBuilder, expr: &Expr, offset: u32) {
        match expr {
            Expr::Int(_, _) => {
                builder.store(
                    ctx.memory_id,
                    walrus::ir::StoreKind::I64 { atomic: false },
                    walrus::ir::MemArg { align: 8, offset },
                );
            }
            Expr::Float(_, _) => {
                builder.store(
                    ctx.memory_id,
                    walrus::ir::StoreKind::F64,
                    walrus::ir::MemArg { align: 8, offset },
                );
            }
            Expr::Bool(_, _) | Expr::String(_, _) => {
                builder.store(
                    ctx.memory_id,
                    walrus::ir::StoreKind::I32 { atomic: false },
                    walrus::ir::MemArg { align: 4, offset },
                );
            }
            _ => {
                // Default to I64 (pointers, arrays, structs, etc)
                builder.store(
                    ctx.memory_id,
                    walrus::ir::StoreKind::I64 { atomic: false },
                    walrus::ir::MemArg { align: 8, offset },
                );
            }
        }
    }
}

