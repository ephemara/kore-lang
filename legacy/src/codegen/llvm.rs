//! LLVM IR Generator
//! 
//! Generates textual LLVM IR (Intermediate Representation) which can be compiled
//! by `clang` or `llc`. This approach is chosen for maximum portability and 
//! reliability without requiring local LLVM library linking during the build.

use crate::types::{TypedProgram, TypedItem, TypedFunction, ResolvedType};
use crate::ast::{Expr, Stmt, BinaryOp, Block};
use crate::error::{KoreError, KoreResult};
use std::collections::HashMap;

pub fn generate(program: &TypedProgram) -> KoreResult<Vec<u8>> {
    let mut gen = LlvmGenerator::new();
    gen.compile_module(program)?;
    Ok(gen.output.into_bytes())
}

struct LlvmGenerator {
    output: String,
    reg_count: usize,
    label_count: usize,
    /// Maps variable names to (stack_ptr, type)
    locals: HashMap<String, (String, String)>,
    /// Maps function names to return type
    functions: HashMap<String, String>,
    /// Maps string content to global variable name
    strings: HashMap<String, String>,
    string_counter: usize,
    /// Stack of (continue_label, break_label) for loops
    loop_stack: Vec<(String, String)>,
    /// Stack of scopes, each containing list of variable names declared in that scope
    scopes: Vec<Vec<String>>,
    /// Struct definitions: Name -> Vec<(FieldName, Type)>
    struct_defs: HashMap<String, Vec<(String, String)>>,
    /// Current basic block label (for Phi nodes)
    current_block: String,
}

impl LlvmGenerator {
    fn new() -> Self {
        Self {
            output: String::new(),
            reg_count: 0,
            label_count: 0,
            locals: HashMap::new(),
            functions: HashMap::new(),
            strings: HashMap::new(),
            string_counter: 0,
            loop_stack: Vec::new(),
            scopes: Vec::new(),
            struct_defs: HashMap::new(),
            current_block: "entry".to_string(),
        }
    }

    fn emit(&mut self, s: &str) {
        self.output.push_str(s);
        self.output.push('\n');
    }

    fn emit_label(&mut self, label: &str) {
        self.emit(&format!("{}:", label));
        self.current_block = label.to_string();
    }

    fn next_reg(&mut self) -> String {
        let r = format!("%{}", self.reg_count);
        self.reg_count += 1;
        r
    }

    fn next_label(&mut self) -> String {
        let l = format!("L{}", self.label_count);
        self.label_count += 1;
        l
    }

    fn map_type_from_ast(&self, ty: &crate::ast::Type) -> String {
        match ty {
            crate::ast::Type::Named { name, .. } => self.map_type_from_str(name),
            _ => "i64".into(),
        }
    }

    fn map_type_from_str(&self, name: &str) -> String {
        match name {
            "Int" | "i64" => "i64".into(),
            "Float" | "f64" | "double" => "double".into(),
            "Bool" | "bool" => "i1".into(),
            "String" | "str" => "i8*".into(),
            "Unit" | "()" | "void" => "void".into(),
            _ => {
                // Check if it's a known struct/enum
                if self.struct_defs.contains_key(name) {
                    format!("%{}*", name)
                } else {
                    "i64".into()
                }
            }
        }
    }

    fn map_type(&self, ty: &crate::types::ResolvedType) -> String {
        use crate::types::ResolvedType;
        match ty {
            ResolvedType::Int(_) => "i64".into(),
            ResolvedType::Float(_) => "double".into(),
            ResolvedType::Bool => "i1".into(),
            ResolvedType::String => "i8*".into(),
            ResolvedType::Unit => "void".into(),
            ResolvedType::Char => "i8".into(),
            ResolvedType::Struct(name, _) => {
                if self.struct_defs.contains_key(name) {
                    format!("%{}*", name)
                } else {
                    self.map_type_from_str(name)
                }
            }
            ResolvedType::Enum(name, _) => format!("%{}*", name),
            ResolvedType::Array(_, _) => "i64".into(), // Arrays are opaque pointers for now
            ResolvedType::Slice(_) => "i64".into(),
            ResolvedType::Option(inner) => self.map_type(inner),
            ResolvedType::Result(ok, _) => self.map_type(ok),
            ResolvedType::Function { .. } => "i64".into(), // Function pointers
            ResolvedType::Generic(name) => self.map_type_from_str(name),
            ResolvedType::Tuple(_) => "i64".into(),
            ResolvedType::Ref { inner, .. } => self.map_type(inner),
            ResolvedType::Never => "void".into(),
            ResolvedType::Unknown => "i64".into(),
        }
    }

    fn hash_message_tag(&self, actor: &str, msg: &str) -> i64 {
        let s = format!("{}_{}", actor, msg);
        let mut hash: i64 = 5381;
        for c in s.bytes() {
            hash = ((hash << 5).wrapping_add(hash)) ^ (c as i64);
        }
        hash
    }

    fn compile_module(&mut self, program: &TypedProgram) -> KoreResult<()> {
        // 1. Emit Header
        self.emit("; ModuleID = 'KORE'");
        self.emit("source_filename = \"KORE\"");
        self.emit("target datalayout = \"e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128\"");
        self.emit("target triple = \"x86_64-pc-windows-msvc\""); // Assuming Windows based on env
        self.emit("");

        // 2a. Pre-scan Structs to register and emit definitions
        for item in &program.items {
            if let TypedItem::Struct(s) = item {
                let mut fields = Vec::new();
                for field in &s.ast.fields {
                    // We need to resolve type from field_types map
                    if let Some(res_ty) = s.field_types.get(&field.name) {
                        fields.push((field.name.clone(), self.map_type(res_ty)));
                    } else {
                        // Should not happen if typed correctly
                        fields.push((field.name.clone(), "i64".into()));
                    }
                }
                self.struct_defs.insert(s.ast.name.clone(), fields.clone());
                
                // Emit type definition
                let field_types: Vec<String> = fields.iter().map(|(_, t)| t.clone()).collect();
                self.emit(&format!("%{} = type {{ {} }}", s.ast.name, field_types.join(", ")));
            } else if let TypedItem::Actor(a) = item {
                let mut fields = Vec::new();
                // Mailbox is always field 0 (MessageQueue*)
                fields.push(("__mailbox".to_string(), "i8*".into()));
                
                for state in &a.ast.state {
                    if let Some(res_ty) = a.state_types.get(&state.name) {
                        fields.push((state.name.clone(), self.map_type(res_ty)));
                    } else {
                        fields.push((state.name.clone(), "i64".into()));
                    }
                }
                self.struct_defs.insert(a.ast.name.clone(), fields.clone());
                
                let field_types: Vec<String> = fields.iter().map(|(_, t)| t.clone()).collect();
                self.emit(&format!("%{} = type {{ {} }}", a.ast.name, field_types.join(", ")));
                
                // Emit Message Payload Structs
                for handler in &a.ast.handlers {
                    let mut payload_fields = Vec::new();
                    let mut field_defs = Vec::new();
                    for param in &handler.params {
                         let p_ty = self.map_type_from_ast(&param.ty);
                         payload_fields.push(p_ty.clone());
                         field_defs.push((param.name.clone(), p_ty));
                    }
                    let msg_struct_name = format!("{}_{}", a.ast.name, handler.message_type);
                    self.struct_defs.insert(msg_struct_name.clone(), field_defs);
                    self.emit(&format!("%{} = type {{ {} }}", msg_struct_name, payload_fields.join(", ")));
                }
            } else if let TypedItem::Enum(e) = item {
                // Emit Enum definition: { tag, payload* }
                self.emit(&format!("%{} = type {{ i64, i8* }}", e.ast.name));
                
                // Emit Variant Payload Structs
                for (variant_name, payload_types) in &e.variant_payload_types {
                    if !payload_types.is_empty() {
                        let field_types: Vec<String> = payload_types.iter().map(|t| self.map_type(t)).collect();
                        let struct_name = format!("{}_{}", e.ast.name, variant_name);
                        self.emit(&format!("%{} = type {{ {} }}", struct_name, field_types.join(", ")));
                        
                        // Register payload struct fields for later lookup
                        let mut fields = Vec::new();
                        for (i, ty) in field_types.iter().enumerate() {
                            fields.push((format!("_{}", i), ty.clone()));
                        }
                        self.struct_defs.insert(struct_name, fields);
                    }
                }
            }
        }
        
        // 2b. Pre-scan functions to register return types
        for item in &program.items {
            if let TypedItem::Function(func) = item {
                if let ResolvedType::Function { ret, .. } = &func.resolved_type {
                    let mut ret_ty = self.map_type(ret);
                    // Heuristic: If void and not main, assume i64 (missing inference)
                    if ret_ty == "void" && func.ast.name != "main" {
                        ret_ty = "i64".into();
                    }
                    self.functions.insert(func.ast.name.clone(), ret_ty);
                }
            }
        }
        
        // 2c. Register StdLib functions
        let stdlib = crate::stdlib::StdLib::new();
        for (name, func) in stdlib.functions {
            let ret_ty = self.map_type_from_str(func.return_type);
            self.functions.insert(name, ret_ty);
        }
        
        // 3. Emit External Declarations (stdlib)
        self.emit_externs();
        self.emit_runtime();

        // 4. Compile Items
        for item in &program.items {
            match item {
                TypedItem::Function(func) => self.compile_function(func)?,
                TypedItem::Actor(actor) => self.compile_actor(actor)?,
                // TODO: Handle Structs, Enums, Consts
                _ => {} 
            }
        }
        
        // 5. Emit String Constants
        // Clone strings to avoid borrow issues
        let strings: Vec<(String, String)> = self.strings.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        for (content, name) in strings {
            let len = content.len() + 1;
            // Escape string content for LLVM (simplified)
            // LLVM expects \xx for hex bytes.
            let mut escaped = String::new();
            for b in content.bytes() {
                if b >= 32 && b < 127 && b != b'"' && b != b'\\' {
                    escaped.push(b as char);
                } else {
                    escaped.push_str(&format!("\\{:02X}", b));
                }
            }
            escaped.push_str("\\00"); // Null terminator
            
            self.emit(&format!("{} = private unnamed_addr constant [{} x i8] c\"{}\", align 1", 
                name, len, escaped));
        }
        
        // 6. Emit Struct Destructors
        self.emit_struct_destructors();

        Ok(())
    }



    fn compile_actor(&mut self, actor: &crate::types::TypedActor) -> KoreResult<()> {
        let name = &actor.ast.name;
        let struct_ty = format!("%{}", name);
        
        // Generate Run Loop Function
        self.emit(&format!("define void @{}_run(i8* %arg) {{", name));
        self.emit_label("entry");
        
        // Cast arg to Actor*
        let self_ptr = self.next_reg();
        self.emit(&format!("  {} = bitcast i8* %arg to {}*", self_ptr, struct_ty));
        
        // Get Mailbox
        let mailbox_ptr = self.next_reg();
        self.emit(&format!("  {} = getelementptr inbounds {}, {}* {}, i32 0, i32 0", mailbox_ptr, struct_ty, struct_ty, self_ptr));
        let mailbox = self.next_reg();
        self.emit(&format!("  {} = load i8*, i8** {}", mailbox, mailbox_ptr));
        
        // Loop
        let label_loop = self.next_label();
        let label_process = self.next_label();
        let label_sleep = self.next_label();
        
        self.emit(&format!("  br label %{}", label_loop));
        self.emit_label(&label_loop);
        
        // Prepare mq_pop args
        let tag_ptr = self.next_reg();
        self.emit(&format!("  {} = alloca i64", tag_ptr));
        let data_ptr = self.next_reg();
        self.emit(&format!("  {} = alloca i8*", data_ptr));
        
        let pop_res = self.next_reg();
        self.emit(&format!("  {} = call i32 @mq_pop(i8* {}, i64* {}, i8** {})", pop_res, mailbox, tag_ptr, data_ptr));
        
        let cond = self.next_reg();
        self.emit(&format!("  {} = icmp ne i32 {}, 0", cond, pop_res));
        self.emit(&format!("  br i1 {}, label %{}, label %{}", cond, label_process, label_sleep));
        
        // Sleep
        self.emit_label(&label_sleep);
        self.emit("  call void @kore_sleep(double 0.001)");
        self.emit(&format!("  br label %{}", label_loop));
        
        // Process
        self.emit_label(&label_process);
        let tag_val = self.next_reg();
        self.emit(&format!("  {} = load i64, i64* {}", tag_val, tag_ptr));
        
        // Switch
        let mut handler_labels = Vec::new();
        for _ in &actor.ast.handlers {
            handler_labels.push(self.next_label());
        }
        
        let mut switch_cases = String::new();
        for (i, handler) in actor.ast.handlers.iter().enumerate() {
             let tag = self.hash_message_tag(name, &handler.message_type);
             switch_cases.push_str(&format!("i64 {}, label %{} ", tag, handler_labels[i]));
        }
        self.emit(&format!("  switch i64 {}, label %{} [ {} ]", tag_val, label_loop, switch_cases));
        
        // Generate Handler Bodies
        for (i, handler) in actor.ast.handlers.iter().enumerate() {
            self.emit_label(&handler_labels[i]);
            
            // Extract Payload
            let payload_void = self.next_reg();
            self.emit(&format!("  {} = load i8*, i8** {}", payload_void, data_ptr));
            
            let msg_struct_name = format!("{}_{}", name, handler.message_type);
            let msg_struct_ty = format!("%{}", msg_struct_name);
            let payload = self.next_reg();
            self.emit(&format!("  {} = bitcast i8* {} to {}*", payload, payload_void, msg_struct_ty));
            
            // Setup Scope
            self.scopes.push(Vec::new());
            self.locals.clear(); 
            self.locals.insert("self".to_string(), (self_ptr.clone(), struct_ty.clone()));
            
            // Map params
            for (j, param) in handler.params.iter().enumerate() {
                let p_ty = self.map_type_from_ast(&param.ty);
                let field_ptr = self.next_reg();
                self.emit(&format!("  {} = getelementptr inbounds {}, {}* {}, i32 0, i32 {}", field_ptr, msg_struct_ty, msg_struct_ty, payload, j));
                let val = self.next_reg();
                self.emit(&format!("  {} = load {}, {}* {}", val, p_ty, p_ty, field_ptr));
                
                let addr_reg = format!("%{}.addr", param.name);
                self.emit(&format!("  {} = alloca {}", addr_reg, p_ty));
                self.emit(&format!("  store {} {}, {}* {}", p_ty, val, p_ty, addr_reg));
                self.locals.insert(param.name.clone(), (addr_reg, p_ty));
                if let Some(scope) = self.scopes.last_mut() {
                    scope.push(param.name.clone());
                }
            }
            
            // Compile Body
            self.compile_block(&handler.body)?;
            
            // Free payload
            self.emit(&format!("  call void @rc_release(i8* {})", payload_void));
            
            self.emit(&format!("  br label %{}", label_loop));
        }

        self.emit("}");
        self.emit("");
        Ok(())
    }

    fn emit_runtime(&mut self) {
        // Runtime implemented in C (kore_runtime.c)
    }

    fn emit_externs(&mut self) {
        // Core Runtime
        self.emit("declare void @print_i64(i64)");
        self.emit("declare void @print_f64(double)");
        self.emit("declare void @print_bool(i1)");
        self.emit("declare void @print_str(i8*, i64)");
        self.emit("declare i8* @to_string(i64)");
        self.emit("declare i8* @str_concat(i8*, i8*)");
        self.emit("declare i64 @clock_wrapper()");
        self.emit("declare i8* @kore_alloc(i64)");
        self.emit("declare void @rc_retain(i8*)");
        self.emit("declare void @rc_release(i8*)");
        self.emit("declare i8* @string_new(i8*)");
        self.emit("declare i8* @array_new(i64)");
        self.emit("declare void @array_push(i8*, i64)");
        self.emit("declare i64 @array_get(i8*, i64)");
        self.emit("declare void @array_set(i8*, i64, i64)");
        self.emit("declare i64 @array_len(i8*)");
        
        // Message Queue & Concurrency
        self.emit("declare i8* @mq_new()");
        self.emit("declare void @mq_push(i8*, i64, i8*)");
        self.emit("declare i32 @mq_pop(i8*, i64*, i8**)");
        self.emit("declare void @kore_spawn(i8*, i8*)");
        self.emit("declare void @kore_set_destructor(i8*, void(i8*)*)");
        self.emit("declare void @kore_sleep(double)");
        self.emit("declare i1 @deep_eq(i8*, i8*)");

        // KOS Bridge
        self.emit("declare void @spawn_cube(double, double)");

        // StdLib
        self.emit_stdlib_externs();
    }

    fn emit_stdlib_externs(&mut self) {
        let stdlib = crate::stdlib::StdLib::new();
        // Skip functions that conflict with manual runtime declarations or are handled specially
        let skip_list = ["print", "println", "to_string"];
        
        for (name, func) in stdlib.functions {
            if skip_list.contains(&name.as_str()) {
                continue;
            }

            let ret_ty = self.map_type_from_str(func.return_type);
            let mut param_tys = Vec::new();
            for (_, p_ty) in func.params {
                param_tys.push(self.map_type_from_str(p_ty));
            }
            
            self.emit(&format!("declare {} @{}({})", ret_ty, name, param_tys.join(", ")));
        }
    }

    fn emit_struct_destructors(&mut self) {
        let structs: Vec<(String, Vec<(String, String)>)> = self.struct_defs.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
            
        for (name, fields) in structs {
            // Only generate if there are RC fields
            let has_rc_fields = fields.iter().any(|(_, ty)| ty == "i8*" || ty.starts_with("%"));
            if !has_rc_fields { continue; }
            
            let struct_ty = format!("%{}", name);
            let dtor_name = format!("dtor_{}", name);
            
            self.emit(&format!("define void @{}(i8* %ptr_void) {{", dtor_name));
            self.emit_label("entry");
            
            // Cast to struct*
            let ptr_typed = self.next_reg();
            self.emit(&format!("  {} = bitcast i8* %ptr_void to {}*", ptr_typed, struct_ty));
            
            // Release fields
            for (i, (_, field_ty)) in fields.iter().enumerate() {
                 if field_ty == "i8*" || field_ty.starts_with("%") {
                     let field_ptr = self.next_reg();
                     self.emit(&format!("  {} = getelementptr inbounds {}, {}* {}, i32 0, i32 {}", field_ptr, struct_ty, struct_ty, ptr_typed, i));
                     let loaded = self.next_reg();
                     self.emit(&format!("  {} = load {}, {}* {}", loaded, field_ty, field_ty, field_ptr));
                     
                     self.emit_release(&loaded, field_ty);
                 }
            }
            
            self.emit("  ret void");
            self.emit("}");
        }
    }

    fn compile_function(&mut self, func: &TypedFunction) -> KoreResult<()> {
        self.reg_count = 0;
        self.locals.clear();
        self.scopes.clear();
        self.scopes.push(Vec::new()); // Top level scope for params

        let name = &func.ast.name;
        
        // Get param types and return type from resolved_type
        let (param_types, ret_type_resolved) = if let ResolvedType::Function { params, ret, .. } = &func.resolved_type {
            (params, ret)
        } else {
            return Err(KoreError::codegen("Function has non-function type", func.ast.span));
        };
        
        let mut ret_type = self.map_type(ret_type_resolved);
        // Heuristic: If void and not main, assume i64
        if ret_type == "void" && func.ast.name != "main" {
            ret_type = "i64".into();
        }
        
        // Special case for main
        let (llvm_name, is_main) = if name == "main" {
            if ret_type == "void" {
                ret_type = "i64".into();
            }
            ("main", true)
        } else {
            (name.as_str(), false)
        };

        // Params
        let mut param_str = String::new();
        for (i, _) in func.ast.params.iter().enumerate() {
            if i > 0 { param_str.push_str(", "); }
            let p_ty = self.map_type(&param_types[i]);
            param_str.push_str(&format!("{} %arg{}", p_ty, i));
        }

        self.emit(&format!("define {} @{}({}) {{", ret_type, llvm_name, param_str));
        self.emit_label("entry");

        // Alloc parameters to stack (standard "alloca" pattern for debuggable IR)
        for (i, param) in func.ast.params.iter().enumerate() {
            let p_ty = self.map_type(&param_types[i]);
            // %param.addr = alloca type
            let addr_reg = format!("%{}.addr", param.name);
            self.emit(&format!("  {} = alloca {}", addr_reg, p_ty));
            self.emit(&format!("  store {} %arg{}, {}* {}", p_ty, i, p_ty, addr_reg));
            self.locals.insert(param.name.clone(), (addr_reg, p_ty));
            if let Some(scope) = self.scopes.last_mut() {
                scope.push(param.name.clone());
            }
        }

        // Compile Body
        self.compile_block(&func.ast.body)?;

        // Implicit return cleanup
        self.emit_scope_exit(); // Clean up params

        // Implicit return for void/Unit functions
        if ret_type == "void" {
            self.emit("  ret void");
        } else if is_main {
            // Main always returns 0 if not specified
            self.emit("  ret i64 0");
        } else {
            // Add a dummy return to ensure validity if flow analysis failed
            self.emit("  unreachable");
        }

        self.emit("}");
        self.emit("");
        Ok(())
    }

    fn compile_block(&mut self, block: &Block) -> KoreResult<()> {
        self.scopes.push(Vec::new());
        for stmt in &block.stmts {
            self.compile_stmt(stmt)?;
        }
        self.emit_scope_exit();
        Ok(())
    }

    fn compile_block_with_result(&mut self, block: &Block) -> KoreResult<Option<(String, String)>> {
        self.scopes.push(Vec::new());
        let mut last_res = None;
        let mut last_is_new = false;
        
        for (i, stmt) in block.stmts.iter().enumerate() {
            if i == block.stmts.len() - 1 {
                 if let Stmt::Expr(expr) = stmt {
                     let (val, ty) = self.compile_expr(expr)?;
                     last_res = Some((val, ty));
                     last_is_new = self.is_new_object(expr);
                 } else {
                     self.compile_stmt(stmt)?;
                 }
            } else {
                self.compile_stmt(stmt)?;
            }
        }
        
        // If we are returning a value from the block, we must retain it before scope exit
        // destroys the local variables it might depend on.
        // Optimization: If the value is already a "new object" (owned with RC=1), we don't need to retain it
        // because no local variable owns it yet, so scope exit won't destroy it.
        if let Some((val, ty)) = &last_res {
            if ty == "i8*" && !last_is_new {
                 self.emit(&format!("  call void @rc_retain(i8* {})", val));
            }
        }
        
        self.emit_scope_exit();
        Ok(last_res)
    }

    fn emit_release(&mut self, val: &str, ty: &str) {
        if ty == "i8*" {
            self.emit(&format!("  call void @rc_release(i8* {})", val));
        } else if ty.starts_with("%") {
            let struct_name = &ty[1..];
            // Clone fields to avoid borrowing self while emitting
            if let Some(fields) = self.struct_defs.get(struct_name).cloned() {
                for (i, (_, field_ty)) in fields.iter().enumerate() {
                    if field_ty == "i8*" || field_ty.starts_with("%") {
                        let field_val = self.next_reg();
                        self.emit(&format!("  {} = extractvalue {} {}, {}", field_val, ty, val, i));
                        self.emit_release(&field_val, field_ty);
                    }
                }
            }
        }
    }

    fn emit_scope_exit(&mut self) {
        if let Some(vars) = self.scopes.pop() {
            for var_name in vars.iter().rev() {
                if let Some((addr, ty)) = self.locals.get(var_name).cloned() {
                    // Release if it's a pointer or struct
                    if ty == "i8*" || ty.starts_with("%") {
                        let tmp = self.next_reg();
                        self.emit(&format!("  {} = load {}, {}* {}", tmp, ty, ty, addr));
                        self.emit_release(&tmp, &ty);
                    }
                }
            }
        }
    }

    fn emit_all_scopes_cleanup(&mut self) {
        let mut vars_to_release = Vec::new();
        for scope in self.scopes.iter().rev() {
            for var in scope.iter().rev() {
                vars_to_release.push(var.clone());
            }
        }
        
        for var_name in vars_to_release {
             if let Some((addr, ty)) = self.locals.get(&var_name).cloned() {
                 if ty == "i8*" || ty.starts_with("%") {
                    let tmp = self.next_reg();
                    self.emit(&format!("  {} = load {}, {}* {}", tmp, ty, ty, addr));
                    self.emit_release(&tmp, &ty);
                 }
             }
        }
    }

    fn is_new_object(&self, expr: &Expr) -> bool {
        match expr {
            Expr::String(..) => true,
            Expr::Array(..) => true,
            Expr::Struct { .. } => true,
            Expr::Call { .. } => true, // Function calls return owned values
            Expr::Binary { op, .. } => *op == BinaryOp::Add, // String concat
            Expr::If { .. } => true, // If expressions return new objects (Phi result)
            _ => false,
        }
    }

    fn compile_stmt(&mut self, stmt: &Stmt) -> KoreResult<()> {
        match stmt {
            Stmt::Let { pattern, value, ty: _, .. } => {
                if let Some(val_expr) = value {
                    // Compile value
                    let (val_reg, val_ty) = self.compile_expr(val_expr)?;
                    
                    // Allocate and Store
                    if let crate::ast::Pattern::Binding { name, .. } = pattern {
                        let addr_reg = format!("%{}.addr_{}", name, self.reg_count);
                        self.reg_count += 1;
                        
                        self.emit(&format!("  {} = alloca {}", addr_reg, val_ty));
                        self.emit(&format!("  store {} {}, {}* {}", val_ty, val_reg, val_ty, addr_reg));
                        
                        // Retain if RC type AND it's not a new object (which already has RC=1)
                        if val_ty == "i8*" {
                            if !self.is_new_object(val_expr) {
                                self.emit(&format!("  call void @rc_retain(i8* {})", val_reg));
                            }
                        }
                        
                        self.locals.insert(name.clone(), (addr_reg, val_ty));
                        if let Some(scope) = self.scopes.last_mut() {
                            scope.push(name.clone());
                        }
                    }
                }
            }
            Stmt::Expr(expr) => {
                let (val, ty) = self.compile_expr(expr)?;
                // If it is a new object, and we are ignoring the result, release it.
                if (ty == "i8*" || ty.starts_with("%")) && self.is_new_object(expr) {
                    self.emit_release(&val, &ty);
                }
            }
            Stmt::Return(expr, _) => {
                if let Some(e) = expr {
                    let (val, ty) = self.compile_expr(e)?;
                    
                    if ty == "i8*" {
                        self.emit(&format!("  call void @rc_retain(i8* {})", val));
                    }
                    
                    self.emit_all_scopes_cleanup();

                    self.emit(&format!("  ret {} {}", ty, val));
                } else {
                    self.emit_all_scopes_cleanup();
                    self.emit("  ret void");
                }
                // Terminate block to keep LLVM happy if there's dead code
                let dead_label = self.next_label();
                self.emit_label(&dead_label);
            }
            Stmt::Break(_, _) => {
                if let Some((_, break_label)) = self.loop_stack.last() {
                    self.emit(&format!("  br label %{}", break_label));
                    let dead_label = self.next_label();
                    self.emit_label(&dead_label);
                }
            }
            Stmt::Continue(_) => {
                if let Some((continue_label, _)) = self.loop_stack.last() {
                    self.emit(&format!("  br label %{}", continue_label));
                    let dead_label = self.next_label();
                    self.emit_label(&dead_label);
                }
            }
            Stmt::While { condition, body, .. } => {
                let label_cond = self.next_label();
                let label_body = self.next_label();
                let label_end = self.next_label();

                self.emit(&format!("  br label %{}", label_cond));
                self.emit_label(&label_cond);
                
                let (cond_val, _) = self.compile_expr(condition)?;
                self.emit(&format!("  br i1 {}, label %{}, label %{}", cond_val, label_body, label_end));
                
                self.emit_label(&label_body);
                
                self.loop_stack.push((label_cond.clone(), label_end.clone()));
                self.compile_block(body)?;
                self.loop_stack.pop();
                
                self.emit(&format!("  br label %{}", label_cond));
                
                self.emit_label(&label_end);
            }
            Stmt::Loop { body, .. } => {
                let label_body = self.next_label();
                let label_end = self.next_label();
                
                self.emit(&format!("  br label %{}", label_body));
                self.emit_label(&label_body);
                
                self.loop_stack.push((label_body.clone(), label_end.clone()));
                self.compile_block(body)?;
                self.loop_stack.pop();
                
                self.emit(&format!("  br label %{}", label_body));
                self.emit_label(&label_end);
            }
            Stmt::For { binding, iter, body, span } => {
                // Determine start, end
                let (start_val, end_val) = match iter {
                    Expr::Call { callee, args, .. } => {
                         if let Expr::Ident(name, _) = callee.as_ref() {
                             if name == "range" && args.len() == 2 {
                                 let (s, _) = self.compile_expr(&args[0].value)?;
                                 let (e, _) = self.compile_expr(&args[1].value)?;
                                 (s, e)
                             } else {
                                 return Err(KoreError::codegen("Unsupported call in for loop", *span));
                             }
                         } else {
                             return Err(KoreError::codegen("Unsupported call in for loop", *span));
                         }
                    }
                    Expr::Range { start, end, inclusive, .. } => {
                        let s = if let Some(e) = start { self.compile_expr(e)?.0 } else { "0".into() };
                        let mut e = if let Some(e) = end { self.compile_expr(e)?.0 } else { 
                            "9223372036854775807".into() 
                        };
                        if *inclusive {
                            let tmp = self.next_reg();
                            self.emit(&format!("  {} = add i64 {}, 1", tmp, e));
                            e = tmp;
                        }
                        (s, e)
                    }
                    _ => return Err(KoreError::codegen("Unsupported iterator in for loop", *span)),
                };

                // Allocate loop variable
                let loop_var = if let crate::ast::Pattern::Binding { name, .. } = binding { name } else { "it" };
                let var_addr = format!("%{}.addr_{}", loop_var, self.reg_count);
                self.reg_count += 1;
                self.emit(&format!("  {} = alloca i64", var_addr));
                self.emit(&format!("  store i64 {}, i64* {}", start_val, var_addr));
                self.locals.insert(loop_var.to_string(), (var_addr.clone(), "i64".into()));
                
                let label_cond = self.next_label();
                let label_body = self.next_label();
                let label_step = self.next_label();
                let label_end = self.next_label();
                
                self.emit(&format!("  br label %{}", label_cond));
                self.emit_label(&label_cond);
                
                // Check condition: var < end
                let curr_val = self.next_reg();
                self.emit(&format!("  {} = load i64, i64* {}", curr_val, var_addr));
                let cond_res = self.next_reg();
                self.emit(&format!("  {} = icmp slt i64 {}, {}", cond_res, curr_val, end_val));
                self.emit(&format!("  br i1 {}, label %{}, label %{}", cond_res, label_body, label_end));
                
                self.emit_label(&label_body);
                
                self.loop_stack.push((label_step.clone(), label_end.clone()));
                self.compile_block(body)?;
                self.loop_stack.pop();
                
                self.emit(&format!("  br label %{}", label_step));
                self.emit_label(&label_step);
                
                // Increment
                let val_before_inc = self.next_reg();
                self.emit(&format!("  {} = load i64, i64* {}", val_before_inc, var_addr));
                let val_after_inc = self.next_reg();
                self.emit(&format!("  {} = add i64 {}, 1", val_after_inc, val_before_inc));
                self.emit(&format!("  store i64 {}, i64* {}", val_after_inc, var_addr));
                
                self.emit(&format!("  br label %{}", label_cond));
                self.emit_label(&label_end);
            }
            _ => {}
        }
        Ok(())
    }

    fn compile_expr(&mut self, expr: &Expr) -> KoreResult<(String, String)> {
        match expr {
            Expr::Int(n, _) => Ok((format!("{}", n), "i64".to_string())),
            Expr::Float(f, _) => Ok((format!("{:.6}", f), "double".to_string())),
            Expr::Bool(b, _) => Ok((if *b { "1".into() } else { "0".into() }, "i1".to_string())),
            Expr::String(s, _) => {
                // Register global string constant
                let global_name = if let Some(name) = self.strings.get(s) {
                    name.clone()
                } else {
                    let name = format!("@.str.{}", self.string_counter);
                    self.string_counter += 1;
                    self.strings.insert(s.clone(), name.clone());
                    name
                };
                
                // Get pointer to static string
                let reg_static = self.next_reg();
                let len = s.len() + 1;
                self.emit(&format!("  {} = getelementptr inbounds [{} x i8], [{} x i8]* {}, i64 0, i64 0", 
                    reg_static, len, len, global_name));
                
                // Call string_new to get RC-managed copy
                let reg_rc = self.next_reg();
                self.emit(&format!("  {} = call i8* @string_new(i8* {})", reg_rc, reg_static));
                    
                Ok((reg_rc, "i8*".to_string()))
            }
            Expr::Spawn { actor, init, span } => {
                let def = self.struct_defs.get(actor).cloned().ok_or(
                    KoreError::codegen(format!("Unknown actor: {}", actor), *span)
                )?;
                
                let struct_ty = format!("%{}", actor);
                
                // Allocate on Heap
                let null_ptr = format!("{}* null", struct_ty);
                let size_ptr_reg = self.next_reg();
                self.emit(&format!("  {} = getelementptr {}, {}, i32 1", size_ptr_reg, struct_ty, null_ptr));
                let size_reg = self.next_reg();
                self.emit(&format!("  {} = ptrtoint {}* {} to i64", size_reg, struct_ty, size_ptr_reg));
                
                let mem_reg = self.next_reg();
                self.emit(&format!("  {} = call i8* @kore_alloc(i64 {})", mem_reg, size_reg));
                
                // Cast to struct ptr
                let struct_ptr = self.next_reg();
                self.emit(&format!("  {} = bitcast i8* {} to {}*", struct_ptr, mem_reg, struct_ty));
                
                // Initialize fields
                let mut provided: HashMap<String, Expr> = init.iter().cloned().collect();
                for (i, (field_name, field_ty)) in def.iter().enumerate() {
                     let (val, val_ty) = if let Some(expr) = provided.remove(field_name) {
                         self.compile_expr(&expr)?
                    } else {
                         // Default zero init
                         let zero_val = if field_ty == "double" { "0.0" } else if field_ty == "i8*" { "null" } else { "0" };
                         (zero_val.into(), field_ty.clone())
                    };
                    
                    let field_ptr = self.next_reg();
                    self.emit(&format!("  {} = getelementptr inbounds {}, {}* {}, i32 0, i32 {}", 
                        field_ptr, struct_ty, struct_ty, struct_ptr, i));
                    self.emit(&format!("  store {} {}, {}* {}", val_ty, val, val_ty, field_ptr));
                }
                
                // Spawn
                // Cast func to i8*
                let func_ptr = "bitcast (void (i8*)* @default_actor_run to i8*)"; 
                
                // Register Destructor if struct has RC fields
                let has_rc_fields = def.iter().any(|(_, ty)| ty == "i8*" || ty.starts_with("%"));
                if has_rc_fields {
                    let dtor_name = format!("dtor_{}", actor);
                    self.emit(&format!("  call void @kore_set_destructor(i8* {}, void (i8*)* @{})", mem_reg, dtor_name));
                }
                
                self.emit(&format!("  call void @kore_spawn(i8* {}, i8* {})", func_ptr, mem_reg));
                
                Ok((mem_reg, "i8*".into()))
            }
            Expr::If { condition, then_branch, else_branch, span } => {
                let start_block = self.current_block.clone();
                let (cond_val, _) = self.compile_expr(condition)?;
                
                let label_then = self.next_label();
                let label_else = self.next_label();
                let label_merge = self.next_label();
                
                let has_else = else_branch.is_some();
                let target_else = if has_else { &label_else } else { &label_merge };

                self.emit(&format!("  br i1 {}, label %{}, label %{}", cond_val, label_then, target_else));
                
                let mut incoming = Vec::new();

                // Then Block
                self.emit_label(&label_then);
                let then_res = self.compile_block_with_result(then_branch)?;
                let then_end_block = self.current_block.clone();
                self.emit(&format!("  br label %{}", label_merge));
                
                if let Some((val, ty)) = then_res {
                    incoming.push((val, ty, then_end_block));
                } else {
                    incoming.push(("0".into(), "i64".into(), then_end_block));
                }
                
                // Else Block
                if let Some(else_branch) = else_branch {
                    self.emit_label(&label_else);
                    let else_res = match else_branch.as_ref() {
                        crate::ast::ElseBranch::Else(b) => self.compile_block_with_result(b)?,
                        crate::ast::ElseBranch::ElseIf(cond, then, el) => {
                             let nested = Expr::If {
                                 condition: cond.clone(),
                                 then_branch: then.clone(),
                                 else_branch: el.clone(),
                                 span: *span
                             };
                             Some(self.compile_expr(&nested)?)
                        }
                    };
                    
                    let else_end_block = self.current_block.clone();
                    self.emit(&format!("  br label %{}", label_merge));
                    
                    if let Some((val, ty)) = else_res {
                        incoming.push((val, ty, else_end_block));
                    } else {
                         incoming.push(("0".into(), "i64".into(), else_end_block));
                    }
                } else {
                    // No else branch: path comes from start_block with value 0
                    incoming.push(("0".into(), "i64".into(), start_block));
                }

                self.emit_label(&label_merge);
                
                // Generate Phi
                let res_ty = incoming[0].1.clone();
                let res_reg = self.next_reg();
                
                // Check consistency (simple check)
                let consistent = incoming.iter().all(|(_, ty, _)| *ty == res_ty);
                
                if consistent {
                    let phi_args = incoming.iter()
                        .map(|(val, _, block)| format!("[ {}, %{} ]", val, block))
                        .collect::<Vec<_>>()
                        .join(", ");
                    
                    self.emit(&format!("  {} = phi {} {}", res_reg, res_ty, phi_args));
                    Ok((res_reg, res_ty))
                } else {
                    // Type mismatch or mixed void/value. Return 0 to be safe.
                    Ok(("0".into(), "i64".into()))
                }
            }
            Expr::Ident(name, span) => {
                if let Some((ptr, ty)) = self.locals.get(name).cloned() {
                    let reg = self.next_reg();
                    self.emit(&format!("  {} = load {}, {}* {}", reg, ty, ty, ptr));
                    Ok((reg, ty))
                } else {
                    Err(KoreError::codegen(format!("Undefined variable: {}", name), *span))
                }
            }
            Expr::Binary { left, op, right, .. } => {
                let (lhs, ty) = self.compile_expr(left)?;
                let (rhs, rhs_ty) = self.compile_expr(right)?;
                
                if *op == BinaryOp::Add && (ty == "i8*" || rhs_ty == "i8*") {
                    let res = self.next_reg();
                    self.emit(&format!("  {} = call i8* @str_concat(i8* {}, i8* {})", res, lhs, rhs));
                    return Ok((res, "i8*".into()));
                }

                if (*op == BinaryOp::Eq || *op == BinaryOp::Ne) && (ty == "i8*" || rhs_ty == "i8*") {
                     let res = self.next_reg();
                     self.emit(&format!("  {} = call i1 @deep_eq(i8* {}, i8* {})", res, lhs, rhs));
                     
                     if *op == BinaryOp::Ne {
                         let inv = self.next_reg();
                         self.emit(&format!("  {} = xor i1 {}, 1", inv, res));
                         return Ok((inv, "i1".into()));
                     }
                     return Ok((res, "i1".into()));
                }

                let res = self.next_reg();
                let op_str = match op {
                    BinaryOp::Add => "add",
                    BinaryOp::Sub => "sub",
                    BinaryOp::Mul => "mul",
                    BinaryOp::Div => "sdiv",
                    BinaryOp::Eq => "icmp eq",
                    BinaryOp::Ne => "icmp ne",
                    BinaryOp::Lt => "icmp slt",
                    BinaryOp::Gt => "icmp sgt",
                    BinaryOp::Le => "icmp sle",
                    BinaryOp::Ge => "icmp sge",
                    BinaryOp::And => "and",
                    BinaryOp::Or => "or",
                    _ => "add",
                };
                
                // If comparison, it returns i1, but we might want to cast back or keep as i1.
                self.emit(&format!("  {} = {} {} {}, {}", res, op_str, ty, lhs, rhs));
                
                if op_str.starts_with("icmp") {
                    Ok((res, "i1".to_string()))
                } else {
                    Ok((res, ty))
                }
            }
            Expr::MethodCall { receiver, method, args, span } => {
                // LLVM doesn't have native method dispatch. 
                // We resolve methods by checking the type of the receiver.
                
                let (obj_val, obj_ty) = self.compile_expr(receiver)?;
                
                // 1. Struct Methods: Call Struct_method(obj, args...)
                if obj_ty.starts_with("%") && obj_ty.ends_with("*") {
                    let struct_name = &obj_ty[1..obj_ty.len()-1]; // Remove % and *
                    let func_name = format!("{}_{}", struct_name, method);
                    
                    if self.functions.contains_key(&func_name) {
                        let mut compiled_args = Vec::new();
                        let mut arg_types = Vec::new();
                        
                        // Pass 'self' as first argument
                        compiled_args.push(obj_val);
                        arg_types.push(obj_ty);
                        
                        for arg in args {
                            let (val, ty) = self.compile_expr(&arg.value)?;
                            compiled_args.push(val);
                            arg_types.push(ty);
                        }
                        
                        let ret_ty = self.functions.get(&func_name).unwrap().clone();
                        let res = self.next_reg();
                        
                        let arg_str = compiled_args.iter().zip(arg_types.iter())
                            .map(|(val, ty)| format!("{} {}", ty, val))
                            .collect::<Vec<_>>()
                            .join(", ");
                            
                        self.emit(&format!("  {} = call {} @{}({})", res, ret_ty, func_name, arg_str));
                        return Ok((res, ret_ty));
                    }
                }
                
                return Err(KoreError::codegen(format!("Method {} not found on type {}", method, obj_ty), *span));
            }
            Expr::Call { callee, args, span } => {
                // Handle print intrinsic
                if let Expr::Ident(name, _) = callee.as_ref() {
                    if name == "to_string" && args.len() == 1 {
                        let (val, ty) = self.compile_expr(&args[0].value)?;
                        if ty == "i64" {
                            let res = self.next_reg();
                            self.emit(&format!("  {} = call i8* @to_string(i64 {})", res, val));
                            return Ok((res, "i8*".into()));
                        }
                    }
                    
                    if name == "now" {
                        let res = self.next_reg();
                        self.emit(&format!("  {} = call i64 @clock_wrapper()", res));
                        return Ok((res, "i64".into()));
                    }

                    if name == "print" || name == "println" {
                        // Just print the first arg for now
                        if let Some(arg) = args.first() {
                            let (val, ty) = self.compile_expr(&arg.value)?;
                            if ty == "i64" {
                                self.emit(&format!("  call void @print_i64(i64 {})", val));
                            } else if ty == "double" {
                                self.emit(&format!("  call void @print_f64(double {})", val));
                            } else if ty == "i1" {
                                self.emit(&format!("  call void @print_bool(i1 {})", val));
                            } else {
                                // Assume string or unknown
                                self.emit(&format!("  call void @print_str(i8* {}, i64 0)", val));
                            }
                            
                            // Release if temporary
                            if (ty == "i8*" || ty.starts_with("%")) && self.is_new_object(&arg.value) {
                                self.emit_release(&val, &ty);
                            }
                        }
                        return Ok(("0".into(), "i64".into()));
                    }
                }
                
                // Normal call - extract function name
                let func_name = match callee.as_ref() {
                    Expr::Ident(name, _) => name.clone(),
                    _ => return Err(KoreError::codegen("Only direct function calls supported", *span)),
                };
                
                let mut compiled_args = Vec::new();
                let mut arg_types = Vec::new();
                
                for (i, arg) in args.iter().enumerate() {
                    let (val, ty) = self.compile_expr(&arg.value)?;
                    
                    // --- HOTFIX: Intrinsic Pointer Casting ---
                    // Check if we are passing a pointer (i8* or %Struct*) to a function 
                    // that expects a generic i64 storage value (Array/Map storage).
                    let needs_cast_to_i64 = (ty == "i8*" || ty.starts_with("%")) && (
                        (func_name == "push" && i == 1) ||        // push(arr, VAL)
                        (func_name == "array_push" && i == 1) ||  // array_push(arr, VAL)
                        (func_name == "array_set" && i == 2) ||   // array_set(arr, idx, VAL)
                        (func_name == "map_set" && i == 2)        // map_set(map, key, VAL)
                    );

                    if needs_cast_to_i64 {
                        let int_val = self.next_reg();
                        // Explicitly cast pointer to integer for the runtime
                        self.emit(&format!("  {} = ptrtoint {} {} to i64", int_val, ty, val));
                        compiled_args.push(int_val);
                        arg_types.push("i64".to_string());
                        continue;
                    }
                    // --- END HOTFIX ---
                    
                    compiled_args.push(val);
                    arg_types.push(ty);
                }
                
                let ret_ty = if let Some(ty) = self.functions.get(&func_name) {
                    ty.clone()
                } else {
                    "i64".into() // Default
                };
                
                let res = self.next_reg();
                let arg_str = compiled_args.iter().zip(arg_types.iter())
                    .map(|(val, ty)| format!("{} {}", ty, val))
                    .collect::<Vec<_>>()
                    .join(", ");
                    
                self.emit(&format!("  {} = call {} @{}({})", res, ret_ty, func_name, arg_str));
                
                Ok((res, ret_ty))
            }
            Expr::EnumVariant { enum_name, variant, fields, .. } => {
                let struct_ty = format!("%{}", enum_name);
                let ptr_ty = format!("{}*", struct_ty);
                
                // Allocate Enum struct
                let null_ptr = format!("{} null", ptr_ty);
                let size_ptr_reg = self.next_reg();
                self.emit(&format!("  {} = getelementptr {}, {}, i32 1", size_ptr_reg, struct_ty, null_ptr));
                let size_reg = self.next_reg();
                self.emit(&format!("  {} = ptrtoint {} {} to i64", size_reg, ptr_ty, size_ptr_reg));
                
                let mem_reg = self.next_reg();
                self.emit(&format!("  {} = call i8* @kore_alloc(i64 {})", mem_reg, size_reg));
                
                let enum_ptr = self.next_reg();
                self.emit(&format!("  {} = bitcast i8* {} to {}", enum_ptr, mem_reg, ptr_ty));
                
                // Store Tag
                let tag = self.hash_message_tag(enum_name, variant);
                let tag_ptr = self.next_reg();
                self.emit(&format!("  {} = getelementptr inbounds {}, {} {}, i32 0, i32 0", tag_ptr, struct_ty, ptr_ty, enum_ptr));
                self.emit(&format!("  store i64 {}, i64* {}", tag, tag_ptr));
                
                // Handle Payload
                let payload_struct_name = format!("{}_{}", enum_name, variant);
                let payload_ty = format!("%{}", payload_struct_name);
                let payload_ptr_ty = format!("{}*", payload_ty);
                
                // Check if payload struct exists (implies non-empty payload)
                if self.struct_defs.contains_key(&payload_struct_name) {
                    // Allocate Payload
                    let p_null_ptr = format!("{} null", payload_ptr_ty);
                    let p_size_ptr = self.next_reg();
                    self.emit(&format!("  {} = getelementptr {}, {}, i32 1", p_size_ptr, payload_ty, p_null_ptr));
                    let p_size = self.next_reg();
                    self.emit(&format!("  {} = ptrtoint {} {} to i64", p_size, payload_ptr_ty, p_size_ptr));
                    
                    let p_mem = self.next_reg();
                    self.emit(&format!("  {} = call i8* @kore_alloc(i64 {})", p_mem, p_size));
                    
                    let p_ptr = self.next_reg();
                    self.emit(&format!("  {} = bitcast i8* {} to {}", p_ptr, p_mem, payload_ptr_ty));
                    
                    // Store Fields
                    match fields {
                        crate::ast::EnumVariantFields::Tuple(exprs) => {
                            for (i, expr) in exprs.iter().enumerate() {
                                let (val, val_ty) = self.compile_expr(expr)?;
                                let field_ptr = self.next_reg();
                                self.emit(&format!("  {} = getelementptr inbounds {}, {} {}, i32 0, i32 {}", field_ptr, payload_ty, payload_ptr_ty, p_ptr, i));
                                self.emit(&format!("  store {} {}, {}* {}", val_ty, val, val_ty, field_ptr));
                            }
                        }
                        crate::ast::EnumVariantFields::Struct(named_fields) => {
                             for (i, (_, expr)) in named_fields.iter().enumerate() {
                                 let (val, val_ty) = self.compile_expr(expr)?;
                                 let field_ptr = self.next_reg();
                                 self.emit(&format!("  {} = getelementptr inbounds {}, {} {}, i32 0, i32 {}", field_ptr, payload_ty, payload_ptr_ty, p_ptr, i));
                                 self.emit(&format!("  store {} {}, {}* {}", val_ty, val, val_ty, field_ptr));
                             }
                        }
                        _ => {}
                    }
                    
                    // Store Payload Pointer in Enum
                    let payload_ptr_ptr = self.next_reg();
                    self.emit(&format!("  {} = getelementptr inbounds {}, {} {}, i32 0, i32 1", payload_ptr_ptr, struct_ty, ptr_ty, enum_ptr));
                    self.emit(&format!("  store i8* {}, i8** {}", p_mem, payload_ptr_ptr));
                    
                } else {
                    // Store Null
                    let payload_ptr_ptr = self.next_reg();
                    self.emit(&format!("  {} = getelementptr inbounds {}, {} {}, i32 0, i32 1", payload_ptr_ptr, struct_ty, ptr_ty, enum_ptr));
                    self.emit(&format!("  store i8* null, i8** {}", payload_ptr_ptr));
                }
                
                Ok((enum_ptr, ptr_ty))
            }
            Expr::Match { scrutinee, arms, span } => {
                let (val, val_ty) = self.compile_expr(scrutinee)?;
                
                let (tag, is_enum) = if val_ty == "i64" {
                    (val.clone(), false)
                } else if val_ty.starts_with("%") && val_ty.ends_with("*") {
                    let struct_ty = &val_ty[0..val_ty.len()-1]; // Remove *
                    // Load Tag
                    let tag_ptr = self.next_reg();
                    self.emit(&format!("  {} = getelementptr inbounds {}, {} {}, i32 0, i32 0", tag_ptr, struct_ty, val_ty, val));
                    let tag = self.next_reg();
                    self.emit(&format!("  {} = load i64, i64* {}", tag, tag_ptr));
                    (tag, true)
                } else {
                     return Err(KoreError::codegen(format!("Match scrutinee must be an enum pointer or int, got {}", val_ty), *span));
                };
                
                // Labels
                let label_end = self.next_label();
                let mut arm_labels = Vec::new();
                let mut switch_cases = String::new();
                
                for _ in arms {
                    arm_labels.push(self.next_label());
                }
                
                let mut enum_name = "";
                if is_enum {
                    let struct_ty = &val_ty[0..val_ty.len()-1];
                    enum_name = &struct_ty[1..];
                }
                
                for (i, arm) in arms.iter().enumerate() {
                    let arm_tag = match &arm.pattern {
                        crate::ast::Pattern::Variant { variant, .. } => self.hash_message_tag(enum_name, variant),
                        crate::ast::Pattern::Literal(Expr::Int(n, _)) => *n, 
                        _ => 0, 
                    };
                    
                    if let crate::ast::Pattern::Variant { .. } = &arm.pattern {
                        switch_cases.push_str(&format!("i64 {}, label %{} ", arm_tag, arm_labels[i]));
                    } else if let crate::ast::Pattern::Literal(Expr::Int(..)) = &arm.pattern {
                        switch_cases.push_str(&format!("i64 {}, label %{} ", arm_tag, arm_labels[i]));
                    }
                }
                
                // Find default label
                let default_label = arms.iter().enumerate()
                    .find(|(_, arm)| matches!(arm.pattern, crate::ast::Pattern::Wildcard(_) | crate::ast::Pattern::Binding { .. }))
                    .map(|(i, _)| &arm_labels[i])
                    .unwrap_or(&label_end);
                    
                self.emit(&format!("  switch i64 {}, label %{} [ {} ]", tag, default_label, switch_cases));
                
                // Compile Arms
                let mut incoming = Vec::new();
                
                for (i, arm) in arms.iter().enumerate() {
                    self.emit_label(&arm_labels[i]);
                    self.scopes.push(Vec::new());
                    
                    // Bindings
                    if is_enum {
                        if let crate::ast::Pattern::Variant { variant, fields, .. } = &arm.pattern {
                            let struct_ty = &val_ty[0..val_ty.len()-1];
                            // Load Payload Ptr
                            let payload_ptr_ptr = self.next_reg();
                            self.emit(&format!("  {} = getelementptr inbounds {}, {} {}, i32 0, i32 1", payload_ptr_ptr, struct_ty, val_ty, val));
                            let payload_void = self.next_reg();
                            self.emit(&format!("  {} = load i8*, i8** {}", payload_void, payload_ptr_ptr));
                            
                            let payload_struct_name = format!("{}_{}", enum_name, variant);
                            let payload_ty = format!("%{}", payload_struct_name);
                            let payload_ptr_ty = format!("{}*", payload_ty);
                            
                            // Cast
                            let payload_ptr = self.next_reg();
                            self.emit(&format!("  {} = bitcast i8* {} to {}", payload_ptr, payload_void, payload_ptr_ty));
                            
                            // Bind Fields
                            match fields {
                                crate::ast::VariantPatternFields::Tuple(pats) => {
                                    for (j, pat) in pats.iter().enumerate() {
                                        if let crate::ast::Pattern::Binding { name, .. } = pat {
                                            let field_ptr = self.next_reg();
                                            self.emit(&format!("  {} = getelementptr inbounds {}, {} {}, i32 0, i32 {}", field_ptr, payload_ty, payload_ptr_ty, payload_ptr, j));
                                            
                                            // Need type of field
                                            let field_ty = if let Some(defs) = self.struct_defs.get(&payload_struct_name) {
                                                defs.get(j).map(|(_, t)| t.clone()).unwrap_or("i64".into())
                                            } else { "i64".into() };
                                            
                                            let field_val = self.next_reg();
                                            self.emit(&format!("  {} = load {}, {}* {}", field_val, field_ty, field_ty, field_ptr));
                                            
                                            let addr_reg = format!("%{}.addr_{}", name, self.reg_count);
                                            self.reg_count += 1;
                                            self.emit(&format!("  {} = alloca {}", addr_reg, field_ty));
                                            self.emit(&format!("  store {} {}, {}* {}", field_ty, field_val, field_ty, addr_reg));
                                            
                                            self.locals.insert(name.clone(), (addr_reg, field_ty));
                                            if let Some(scope) = self.scopes.last_mut() {
                                                scope.push(name.clone());
                                            }
                                        }
                                    }
                                }
                                _ => {} 
                            }
                        }
                    }
                    
                    let (res_val, res_ty) = self.compile_expr(&arm.body)?;
                    let arm_end_block = self.current_block.clone();
                    
                    self.emit_scope_exit();
                    self.emit(&format!("  br label %{}", label_end));
                    incoming.push((res_val, res_ty, arm_end_block));
                }
                
                self.emit_label(&label_end);
                
                // Phi
                if incoming.is_empty() {
                    Ok(("0".into(), "i64".into()))
                } else {
                    let res_ty = incoming[0].1.clone();
                    let res_reg = self.next_reg();
                    
                    let phi_args = incoming.iter()
                        .map(|(val, _, block)| format!("[ {}, %{} ]", val, block))
                        .collect::<Vec<_>>()
                        .join(", ");
                        
                    self.emit(&format!("  {} = phi {} {}", res_reg, res_ty, phi_args));
                    Ok((res_reg, res_ty))
                }
            }
            // Catch-all for unsupported expressions
            other => {
                // For unsupported expressions, return a dummy value
                // This allows compilation to continue for partial codegen
                Ok(("0".into(), "i64".into()))
            }
        }
    }
}