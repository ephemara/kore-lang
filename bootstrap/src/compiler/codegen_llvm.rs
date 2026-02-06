// ============================================================================
// KAIN Bootstrap Compiler - LLVM IR Code Generator (Rust)
// ============================================================================
// Migrated to Inkwell (Safe LLVM Bindings)
// ============================================================================

use crate::compiler::parser::{
    Program, Item, FnDef, StructDef, EnumDef, ImplDef,
    Stmt, Expr, ExternFnDef
};
use std::collections::{HashMap, HashSet};

use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::builder::Builder;
use inkwell::values::{FunctionValue, IntValue, PointerValue, BasicMetadataValueEnum};
use inkwell::types::{IntType, StructType, VoidType, BasicType, BasicMetadataTypeEnum};
use inkwell::basic_block::BasicBlock;
use inkwell::{AddressSpace, IntPredicate};

// =============================================================================
// NaN-Boxing Constants
// =============================================================================

const NANBOX_QNAN: u64 = 0xFFF8000000000000;
const NANBOX_TAG_SHIFT: u64 = 45;
const NANBOX_PAYLOAD_MASK: u64 = 0x00001FFFFFFFFFFF;

const KAIN_TAG_PTR: u64 = 0;
const KAIN_TAG_INT: u64 = 1;

// =============================================================================
// LLVM Code Generator
// =============================================================================

pub struct LLVMGen<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    
    i64_type: IntType<'ctx>,
    void_type: VoidType<'ctx>,
    
    variables: HashMap<String, PointerValue<'ctx>>, 
    
    struct_types: HashMap<String, StructType<'ctx>>,
    field_indices: HashMap<String, HashMap<String, u32>>,
    field_types: HashMap<String, HashMap<String, String>>,
    
    scoped_variant_map: HashMap<(String, String), usize>,
    global_variant_map: HashMap<String, (String, usize)>,
    method_map: HashMap<String, String>,
    
    loop_stack: Vec<(BasicBlock<'ctx>, BasicBlock<'ctx>)>,

    current_impl_type: Option<String>,
    var_types: HashMap<String, String>,
    function_return_types: HashMap<String, String>,
    string_literals: HashMap<String, PointerValue<'ctx>>,
    functions: HashMap<String, FunctionValue<'ctx>>,
    
    // Track initialized variables for compile-time validation
    initialized_vars: HashSet<String>,
    
    // Track struct PARAMETERS (where variable IS the pointer, not an alloca containing boxed ptr)
    struct_params: HashSet<String>,
    string_id: u64,
    verify_passed: bool,
}

impl<'ctx> LLVMGen<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> LLVMGen<'ctx> {
        let module = context.create_module(module_name);
        let builder = context.create_builder();
        
        let i64_type = context.i64_type();
        let void_type = context.void_type();

        LLVMGen {
            context,
            module,
            builder,
            i64_type,
            void_type,
            variables: HashMap::new(),
            struct_types: HashMap::new(),
            field_indices: HashMap::new(),
            field_types: HashMap::new(),
            scoped_variant_map: HashMap::new(),
            global_variant_map: HashMap::new(),
            method_map: HashMap::new(),
            loop_stack: Vec::new(),
            current_impl_type: None,
            var_types: HashMap::new(),
            function_return_types: HashMap::new(),
            string_literals: HashMap::new(),
            functions: HashMap::new(),
            initialized_vars: HashSet::new(),
            struct_params: HashSet::new(),
            string_id: 0,
            verify_passed: false,
        }
    }
    
    // =========================================================================
    // Helpers
    // =========================================================================

    fn box_int(&self, val: IntValue<'ctx>) -> IntValue<'ctx> {
        let payload_mask = self.i64_type.const_int(NANBOX_PAYLOAD_MASK, false);
        let tag_shift = self.i64_type.const_int(NANBOX_TAG_SHIFT, false);
        let tag = self.i64_type.const_int(KAIN_TAG_INT, false);
        let qnan = self.i64_type.const_int(NANBOX_QNAN, false);
        
        let masked = self.builder.build_and(val, payload_mask, "box_int_mask").unwrap();
        let shifted_tag = self.builder.build_left_shift(tag, tag_shift, "box_int_tag_shift").unwrap();
        let tagged = self.builder.build_or(masked, shifted_tag, "box_int_or_tag").unwrap();
        self.builder.build_or(tagged, qnan, "box_int_or_qnan").unwrap()
    }

    fn unbox_int(&self, val: IntValue<'ctx>) -> IntValue<'ctx> {
        let payload_mask = self.i64_type.const_int(NANBOX_PAYLOAD_MASK, false);
        self.builder.build_and(val, payload_mask, "unbox_int_mask").unwrap()
    }
    
    fn box_ptr(&self, ptr: PointerValue<'ctx>) -> IntValue<'ctx> {
        let ptr_as_int = self.builder.build_ptr_to_int(ptr, self.i64_type, "ptr_to_int").unwrap();
        let shifted = self.builder.build_right_shift(ptr_as_int, self.i64_type.const_int(3, false), false, "ptr_shr_3").unwrap();
        let prefix = self.i64_type.const_int(0xFFF8000000000000u64, false);
        self.builder.build_or(shifted, prefix, "box_ptr_prefix").unwrap()
    }

    fn box_string(&self, ptr: PointerValue<'ctx>) -> IntValue<'ctx> {
        // String boxing: QNAN | (TAG_STR << 45) | (ptr >> 3)
        // This matches the runtime's KAIN_BOX_STR macro
        let ptr_as_int = self.builder.build_ptr_to_int(ptr, self.i64_type, "ptr_to_int").unwrap();

        // Shift pointer right by 3 (pointer compression for 45-bit address space)
        let shift_amount = self.i64_type.const_int(3, false);
        let ptr_shifted = self.builder.build_right_shift(ptr_as_int, shift_amount, false, "ptr_shr_3").unwrap();

        // Create tag: (KAIN_TAG_STR << 45)
        const KAIN_TAG_STR: u64 = 4;
        let tag = self.i64_type.const_int(KAIN_TAG_STR, false);
        let tag_shift = self.i64_type.const_int(NANBOX_TAG_SHIFT, false);
        let shifted_tag = self.builder.build_left_shift(tag, tag_shift, "tag_shift").unwrap();

        // Combine: QNAN | shifted_tag | ptr_shifted
        let qnan = self.i64_type.const_int(NANBOX_QNAN, false);
        let with_tag = self.builder.build_or(qnan, shifted_tag, "qnan_or_tag").unwrap();
        self.builder.build_or(with_tag, ptr_shifted, "box_string").unwrap()
    }

    fn unbox_ptr(&self, val: IntValue<'ctx>, ptr_type: inkwell::types::PointerType<'ctx>) -> PointerValue<'ctx> {
        let payload_mask = self.i64_type.const_int(NANBOX_PAYLOAD_MASK, false);
        let masked = self.builder.build_and(val, payload_mask, "unbox_ptr_mask").unwrap();
        
        // Shift left by 3 to restore the full pointer address
        let shift_amount = self.i64_type.const_int(3, false);
        let restored = self.builder.build_left_shift(masked, shift_amount, "ptr_shl_3").unwrap();
        
        self.builder.build_int_to_ptr(restored, ptr_type, "unbox_ptr_cast").unwrap()
    }

    fn infer_expr_type(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Ident(name) => self.var_types.get(name).cloned(),
            Expr::Struct(name, _) => Some(name.clone()),
            Expr::String(_) => Some("String".to_string()),
            Expr::Int(_) => Some("Int".to_string()),
            Expr::Field(obj, field) => {
                 let obj_type = self.infer_expr_type(obj)?;
                 self.field_types.get(&obj_type)?.get(field).cloned()
            },
            Expr::Call(callee, _args) => {
                // Try to extract function name from callee
                let func_name = match callee.as_ref() {
                    Expr::Ident(name) => Some(name.clone()),
                    Expr::Field(obj, method) => {
                        // Handle Type::method() static calls
                        if let Expr::Ident(type_name) = obj.as_ref() {
                            Some(format!("{}_{}", type_name, method))
                        } else {
                            None
                        }
                    },
                    _ => None,
                }?;
                
                // Look up return type
                self.function_return_types.get(&func_name).cloned()
            },
            Expr::EnumVariant(type_name, variant_name, _) => {
                let mangled = format!("{}_{}", type_name, variant_name);
                // Check if it's a registered function (static method)
                if let Some(ret_type) = self.function_return_types.get(&mangled) {
                    Some(ret_type.clone())
                } else {
                    // Otherwise assume it's an enum constructor returning the type itself
                    Some(type_name.clone())
                }
            },
            Expr::Tuple(_) => None, // Tuple type inference not yet supported
            _ => None,
        }
    }

    // =========================================================================
    // Program Generation
    // =========================================================================
    
    pub fn gen_program(&mut self, program: Program) {
        self.declare_builtins();

        eprintln!("DEBUG: Total program items: {}", program.items.len());

        // Count item types
        let mut fn_count = 0;
        let mut struct_count = 0;
        let mut impl_count = 0;
        for item in &program.items {
            match item {
                Item::Function(_) => fn_count += 1,
                Item::Struct(_) => struct_count += 1,
                Item::Impl(_) => impl_count += 1,
                _ => {}
            }
        }
        eprintln!("DEBUG: Functions: {}, Structs: {}, Impl blocks: {}", fn_count, struct_count, impl_count);

        // Pass 1: Generate struct/enum types
        for item in &program.items {
            match item {
                Item::Struct(def) => self.gen_struct(def.clone()),
                Item::Enum(def) => self.gen_enum(def.clone()),
                _ => {}
            }
        }

        // Pass 2: Forward-declare all functions and impl methods with deterministic signatures
        eprintln!("DEBUG: Pass 2 - Forward declaring functions and methods...");
        for item in &program.items {
            match item {
                Item::Function(def) => {
                    // Free function prototype
                    let mut name = def.name.clone();
                    if name == "main" { name = "main_KAIN".to_string(); }
                    let mut param_types: Vec<BasicMetadataTypeEnum> = Vec::new();
                    for p in &def.params {
                        if let Some(ty) = &p.ty {
                            let actual_ty = if ty == "Self" { ty } else { ty };
                            if let Some(struct_ty) = self.struct_types.get(actual_ty) {
                                param_types.push(self.context.ptr_type(AddressSpace::default()).into());
                            } else {
                                param_types.push(self.i64_type.as_basic_type_enum().into());
                            }
                        } else {
                            param_types.push(self.i64_type.as_basic_type_enum().into());
                        }
                    }
                    let fn_type = self.i64_type.fn_type(&param_types, false);
                    if self.module.get_function(&name).is_none() {
                        self.module.add_function(&name, fn_type, None);
                    }
                },
                Item::Impl(impl_def) => {
                    // Methods: include implicit self only when instance-style (declares self or Self)
                    for m in &impl_def.methods {
                        let mangled = format!("{}_{}", impl_def.target, m.name);
                        let mut param_types: Vec<BasicMetadataTypeEnum> = Vec::new();
                        let mut is_instance = false;
                        for p in &m.params {
                            if p.name == "self" || p.ty.as_deref() == Some("Self") {
                                is_instance = true;
                                break;
                            }
                        }
                        if is_instance {
                            // implicit self pointer
                            param_types.push(self.context.ptr_type(AddressSpace::default()).into());
                        }
                        for p in &m.params {
                            if p.name == "self" { continue; }
                            if let Some(ty) = &p.ty {
                                let actual_ty = if ty == "Self" { &impl_def.target } else { ty };
                                let is_struct = self.struct_types.get(actual_ty).is_some();
                                if is_struct { param_types.push(self.context.ptr_type(AddressSpace::default()).into()); }
                                else { param_types.push(self.i64_type.as_basic_type_enum().into()); }
                            } else {
                                param_types.push(self.i64_type.as_basic_type_enum().into());
                            }
                        }
                        let fn_type = self.i64_type.fn_type(&param_types, false);
                        if self.module.get_function(&mangled).is_none() {
                            self.module.add_function(&mangled, fn_type, None);
                        }
                    }
                },
                _ => {}
            }
        }
        eprintln!("DEBUG: Pass 2 complete");

        // Pass 3: Generate extern decls and functions
        for item in &program.items {
            match item {
                Item::Extern(def) => self.gen_extern_decl(def.clone()),
                Item::Function(def) => self.gen_function(def.clone()),
                _ => {}
            }
        }

        // Pass 4: Generate impl method bodies
        for item in &program.items {
            if let Item::Impl(def) = item {
                self.gen_impl(def.clone());
            }
        }

        eprintln!("DEBUG: gen_program complete (verification pending)");
    }
    
    pub fn verify(&mut self) -> bool {
        eprintln!("DEBUG: Starting module verification...");
        match self.module.verify() {
            Ok(()) => { 
                eprintln!("DEBUG: module verify OK"); 
                self.verify_passed = true;
                true
            }
            Err(e) => { 
                eprintln!("DEBUG: module verify FAILED: {}", e.to_string()); 
                self.verify_passed = false;
                false
            }
        }
    }
    
    pub fn write_to_file(&self, path: &std::path::Path) -> Result<(), String> {
        eprintln!("DEBUG: Writing IR to file: {:?}", path);
        match self.module.print_to_file(path) {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
    
    pub fn verification_passed(&self) -> bool {
        self.verify_passed
    }
    
    pub fn current_ir_string(&self) -> String {
        self.module.print_to_string().to_string()
    }
    
    fn declare_builtins(&mut self) {
        let i64_type = self.i64_type;
        let ptr_i8_type = self.context.ptr_type(AddressSpace::default());
        let void_type = self.void_type;
        
        let malloc_type = ptr_i8_type.fn_type(&[i64_type.into()], false);
        self.module.add_function("malloc", malloc_type, None);
        
        // (i64, i64) -> i64 functions
        let ii_i = i64_type.fn_type(&[i64_type.into(), i64_type.into()], false);
        let builtins = ["KAIN_add_op", "KAIN_sub_op", "KAIN_mul_op", "KAIN_div_op", "KAIN_rem_op",
                        "KAIN_eq_op", "KAIN_neq_op", "KAIN_lt_op", "KAIN_le_op", "KAIN_str_concat_boxed",
                        "KAIN_gt_op", "KAIN_ge_op", "KAIN_contains", "KAIN_str_eq", "KAIN_array_get",
                        "KAIN_map_get", "KAIN_array_push"];
        for name in builtins {
            self.module.add_function(name, ii_i, None);
        }
        
        // (i64, i64, i64) -> i64 functions  
        let iii_i = i64_type.fn_type(&[i64_type.into(), i64_type.into(), i64_type.into()], false);
        self.module.add_function("KAIN_substring", iii_i, None);
        self.module.add_function("KAIN_map_set", iii_i, None);
        
        // i64 -> i64 functions
        let i_i = i64_type.fn_type(&[i64_type.into()], false);
        self.module.add_function("KAIN_to_string", i_i, None);
        self.module.add_function("KAIN_str_len", i_i, None);
        self.module.add_function("KAIN_array_len", i_i, None);
        self.module.add_function("KAIN_array_pop", i_i, None);
        self.module.add_function("KAIN_unbox_any_ptr", ptr_i8_type.fn_type(&[i64_type.into()], false), None);
        self.module.add_function("KAIN_is_string", i_i, None);
        self.module.add_function("KAIN_is_ptr", i_i, None);

        // Trace functions
        let trace_enter_ty = void_type.fn_type(&[ptr_i8_type.into(), ptr_i8_type.into(), i64_type.into()], false);
        self.module.add_function("KAIN_trace_enter", trace_enter_ty, None);
        
        let trace_exit_ty = void_type.fn_type(&[], false);
        self.module.add_function("KAIN_trace_exit", trace_exit_ty, None);
        let unary_builtins = ["KAIN_array_new", "KAIN_str_len", "KAIN_to_string", "KAIN_is_truthy", 
                               "exit", "KAIN_array_len"];
        for name in unary_builtins {
            self.module.add_function(name, i_i, None);
        }
        
        // 0-param functions
        let v_i = i64_type.fn_type(&[], false);
        self.module.add_function("args", v_i, None);
        
        let i_v = i64_type.fn_type(&[i64_type.into()], false);
        self.module.add_function("KAIN_print_str", i_v, None);
        self.module.add_function("KAIN_println_str", i_v, None);
    }

    
    fn gen_struct(&mut self, def: StructDef) {
        let struct_type = self.context.opaque_struct_type(&def.name);
        self.struct_types.insert(def.name.clone(), struct_type);
        
        let field_types = vec![self.i64_type.as_basic_type_enum(); def.fields.len()];
        struct_type.set_body(&field_types, false);
        
        let mut idx_map = HashMap::new();
        let mut type_map = HashMap::new();
        for (i, field) in def.fields.into_iter().enumerate() {
            idx_map.insert(field.name.clone(), i as u32);
            type_map.insert(field.name.clone(), field.ty);
        }
        self.field_indices.insert(def.name.clone(), idx_map);
        self.field_types.insert(def.name.clone(), type_map);
    }
    
    fn gen_enum(&mut self, def: EnumDef) {
        for (idx, variant) in def.variants.iter().enumerate() {
            self.scoped_variant_map.insert((def.name.clone(), variant.name.clone()), idx);
            self.global_variant_map.insert(variant.name.clone(), (def.name.clone(), idx));
        }
    }
    
    fn gen_extern_decl(&mut self, def: ExternFnDef) {
        if self.module.get_function(&def.name).is_some() { return; }
        
        let param_types = vec![self.i64_type.as_basic_type_enum().into(); def.params.len()];
        let fn_type = self.i64_type.fn_type(&param_types, false);
        self.module.add_function(&def.name, fn_type, None);
    }
    
    fn gen_function(&mut self, def: FnDef) {
        // Clear function-scoped state to prevent leakage between functions
        self.variables.clear();
        self.initialized_vars.clear();
        self.struct_params.clear();
        self.var_types.clear();
        
        let mut name = def.name.clone();
        eprintln!("DEBUG: gen_function enter: {} (body_len={})", name, def.body.len());
        if name == "main" { name = "main_KAIN".to_string(); }

        // FIXED: Handle struct parameters by reference, others by value
        let mut param_types = vec![];
        let has_explicit_self = def.params.iter().any(|p| p.name == "self");
        if self.current_impl_type.is_some() && !has_explicit_self {
            // Insert implicit 'self' pointer param for impl methods
            param_types.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        for param in &def.params {
            if let Some(ty) = &param.ty {
                // Handle Self type by mapping to current_impl_type
                let actual_ty = if ty == "Self" {
                    self.current_impl_type.as_ref().unwrap_or(ty)
                } else {
                    ty
                };
                
                    if let Some(struct_ty) = self.struct_types.get(actual_ty) {
                    param_types.push(self.context.ptr_type(AddressSpace::default()).into());
                } else {
                    // Non-struct parameters remain as i64
                    param_types.push(self.i64_type.as_basic_type_enum().into());
                }
            } else if param.name == "self" {
                // Untyped self parameter - use current_impl_type
                if let Some(impl_type) = &self.current_impl_type {
                    if let Some(struct_ty) = self.struct_types.get(impl_type) {
                        param_types.push(self.context.ptr_type(AddressSpace::default()).into());
                    } else {
                        param_types.push(self.i64_type.as_basic_type_enum().into());
                    }
                } else {
                    param_types.push(self.i64_type.as_basic_type_enum().into());
                }
            } else {
                // Unknown type, default to i64
                param_types.push(self.i64_type.as_basic_type_enum().into());
            }
        }
        
        let fn_type = self.i64_type.fn_type(&param_types, false);

        // Get existing function (from forward decl) or create new one
        let function = self.module.get_function(&name).unwrap_or_else(|| {
            self.module.add_function(&name, fn_type, None)
        });
        self.functions.insert(name.clone(), function);
        
        // Store return type for type inference
        if let Some(return_type) = &def.return_type {
            self.function_return_types.insert(name.clone(), return_type.clone());
        }
        
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        
        self.variables.clear();
        self.var_types.clear();
        self.initialized_vars.clear();
        self.loop_stack.clear();
        
        // TRACE ENTER
        eprintln!("DEBUG: gen_function before trace_enter");
        let trace_enter = self.module.get_function("KAIN_trace_enter").unwrap();
        let name_global = format!("func_name_{}", name);
        let file_global = format!("file_name_{}", name);
        let name_str = self.builder.build_global_string_ptr(&name, &name_global).unwrap().as_pointer_value();
        let file_str = self.builder.build_global_string_ptr("unknown.kn", &file_global).unwrap().as_pointer_value();
        let line_val = self.i64_type.const_int(0, false);
        let _ = self.builder.build_call(trace_enter, &[name_str.into(), file_str.into(), line_val.into()], "").unwrap();
        eprintln!("DEBUG: gen_function after trace_enter");
        
        // Implicit 'self' for impl methods when not explicitly declared in params
        let has_explicit_self = def.params.iter().any(|p| p.name == "self");
        if self.current_impl_type.is_some() && !has_explicit_self {
            if let Some(p0) = function.get_nth_param(0) {
                if p0.get_type().is_pointer_type() {
                    let alloca = p0.into_pointer_value();
                    self.variables.insert("self".to_string(), alloca);
                    self.struct_params.insert("self".to_string());
                    self.initialized_vars.insert("self".to_string());
                    if let Some(impl_type) = &self.current_impl_type {
                        self.var_types.insert("self".to_string(), impl_type.clone());
                    }
                }
            }
        }
        
        eprintln!("DEBUG: gen_function param_count={}", def.params.len());
        let implicit_self_offset = if self.current_impl_type.is_some() && !has_explicit_self {
            let formal_count = function.count_params() as usize;
            if formal_count == def.params.len() + 1 { 1 } else { 0 }
        } else { 0 };
        for (i, param) in def.params.iter().enumerate() {
            eprintln!("DEBUG: map_param idx={} name={}", i, param.name);
            let param_index = i as u32 + implicit_self_offset as u32;
            if let Some(p_val) = function.get_nth_param(param_index) {
                p_val.set_name(&param.name);
            
            // Determine if this parameter is a pointer from the actual function signature
            let is_struct_param = p_val.get_type().is_pointer_type();
            
            // FIXED: Handle struct parameters differently
            let alloca = if is_struct_param {
                // Struct parameter: already a pointer, use it directly
                // Record that this is a struct param (not a local with boxed ptr)
                self.struct_params.insert(param.name.clone());
                p_val.into_pointer_value()
            } else {
                // Non-struct parameter: create alloca and store
                let alloca = self.builder.build_alloca(self.i64_type, &param.name).unwrap();
                let p_val_int = p_val.into_int_value();
                self.builder.build_store(alloca, p_val_int).unwrap();
                alloca
            };
            
            self.variables.insert(param.name.clone(), alloca);
            self.initialized_vars.insert(param.name.clone());
            } else {
                // Parameter index not present in existing function declaration; skip mapping
                continue;
            }
            
            // Register parameter type
            if let Some(ty) = &param.ty {
                let mut param_ty = ty.clone();
                if param_ty == "Self" {
                    if let Some(impl_type) = &self.current_impl_type {
                        param_ty = impl_type.clone();
                    }
                }
                self.var_types.insert(param.name.clone(), param_ty);
            } else if param.name == "self" {
                // Fallback for 'self' parameters without explicit type
                if let Some(impl_type) = &self.current_impl_type {
                    self.var_types.insert(param.name.clone(), impl_type.clone());
                }
            }
        }

        
        eprintln!("DEBUG: gen_function start body");
        for stmt in def.body {
            self.gen_stmt(stmt);
        }
        eprintln!("DEBUG: gen_function end body");
        
        if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
            let trace_exit = self.module.get_function("KAIN_trace_exit").unwrap();
            self.builder.build_call(trace_exit, &[], "").unwrap();
            self.builder.build_return(Some(&self.i64_type.const_int(0, false))).unwrap();
        }
    }
    
    fn gen_impl(&mut self, def: ImplDef) {
        self.current_impl_type = Some(def.target.clone());
        for mut method in def.methods {
            self.method_map.insert(method.name.clone(), def.target.clone());
            method.name = format!("{}_{}", def.target, method.name);
            self.gen_function(method);
        }
        self.current_impl_type = None;
    }

    fn gen_stmt(&mut self, stmt: Stmt) {
        match stmt {
            Stmt::Let(name, ty_opt, init) => {


                 let val = self.gen_expr(init.clone());
                 
                 // DEBUG-LET logging disabled - produces 247K+ lines of output
                 // Uncomment to trace variable assignments:
                 // let debug_fn = self.module.get_function("KAIN_debug_log_var").unwrap_or_else(|| {
                 //     let i8_ptr = self.context.ptr_type(AddressSpace::default());
                 //     let fn_type = self.void_type.fn_type(&[i8_ptr.into(), self.i64_type.into()], false);
                 //     self.module.add_function("KAIN_debug_log_var", fn_type, None)
                 // });
                 // let name_str = self.builder.build_global_string_ptr(&name, "var_name").unwrap().as_pointer_value();
                 // self.builder.build_call(debug_fn, &[name_str.into(), val.into()], "").unwrap();

                 let alloca = self.builder.build_alloca(self.i64_type, &name).unwrap();
                 self.builder.build_store(alloca, val).unwrap();
                 self.variables.insert(name.clone(), alloca);
                 self.initialized_vars.insert(name.clone());
                 if let Some(ty) = ty_opt {
                     self.var_types.insert(name, ty);
                 } else if let Some(inferred_ty) = self.infer_expr_type(&init) {
                     self.var_types.insert(name, inferred_ty);
                 }
            },
            Stmt::Var(name, ty_opt, init) => {
                 let val = self.gen_expr(init);
                 let alloca = self.builder.build_alloca(self.i64_type, &name).unwrap();
                 self.builder.build_store(alloca, val).unwrap();
                 self.variables.insert(name.clone(), alloca);
                 self.initialized_vars.insert(name.clone());
                 if let Some(ty) = ty_opt {
                     self.var_types.insert(name, ty);
                 }
            },
            Stmt::Assign(lhs, rhs) => {
                let val = self.gen_expr(rhs);
                match lhs {
                    Expr::Ident(name) => {
                        if let Some(ptr) = self.variables.get(&name) {
                            self.builder.build_store(*ptr, val).unwrap();
                        }
                    },
                    Expr::Field(obj, field) => {
                        if let Some(struct_name) = self.infer_expr_type(&obj) {
                            let obj_val = self.gen_expr(*obj.clone());
                            if let Some(struct_ty) = self.struct_types.get(&struct_name).cloned() {
                                let ptr_ty = self.context.ptr_type(AddressSpace::default());
                                let ptr_struct = self.unbox_ptr(obj_val, ptr_ty);
                                if let Some(idx) = self.get_field_index(&struct_name, &field) {
                                    let field_ptr = self.builder.build_struct_gep(struct_ty, ptr_struct, idx, "field_assign").unwrap();
                                    self.builder.build_store(field_ptr, val).unwrap();
                                }
                            }
                        }
                    },
                    _ => {}
                }
            },
            Stmt::Return(e_opt) => {
                 let val = if let Some(e) = e_opt {
                    self.gen_expr(e)
                 } else {
                    self.i64_type.const_int(0, false)
                 };
                 let trace_exit = self.module.get_function("KAIN_trace_exit").unwrap();
                 self.builder.build_call(trace_exit, &[], "").unwrap();
                 self.builder.build_return(Some(&val)).unwrap();
            },
            Stmt::Expr(e) => {
                self.gen_expr(e);
            },
            Stmt::If(cond, then_block, else_block_opt) => {
                 let cond_val = self.gen_expr(cond);
                 let zero = self.i64_type.const_int(0, false);
                 let is_truthy = self.module.get_function("KAIN_is_truthy").unwrap(); let truthy_val = self.builder.build_call(is_truthy, &[cond_val.into()], "if_truthy").unwrap().try_as_basic_value().unwrap_basic().into_int_value(); let is_true = self.builder.build_int_compare(IntPredicate::NE, truthy_val, zero, "ifcond").unwrap();
                 
                 let parent = self.builder.get_insert_block().unwrap().get_parent().unwrap();
                 let then_bb = self.context.append_basic_block(parent, "then");
                 let else_bb = self.context.append_basic_block(parent, "else");
                 let merge_bb = self.context.append_basic_block(parent, "merge");
                 
                 self.builder.build_conditional_branch(is_true, then_bb, else_bb).unwrap();
                 
                 self.builder.position_at_end(then_bb);
                 for s in then_block { self.gen_stmt(s); }
                 if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                     self.builder.build_unconditional_branch(merge_bb).unwrap();
                 }
                 
                 self.builder.position_at_end(else_bb);
                 if let Some(else_stmts) = else_block_opt {
                     for s in else_stmts { self.gen_stmt(s); }
                 }
                 if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                     self.builder.build_unconditional_branch(merge_bb).unwrap();
                 }
                 
                 self.builder.position_at_end(merge_bb);
            },
            Stmt::While(cond, body) => {
                 let parent = self.builder.get_insert_block().unwrap().get_parent().unwrap();
                 let cond_bb = self.context.append_basic_block(parent, "while_cond");
                 let body_bb = self.context.append_basic_block(parent, "while_body");
                 let end_bb = self.context.append_basic_block(parent, "while_end");
                 
                 self.builder.build_unconditional_branch(cond_bb).unwrap();
                 
                 self.builder.position_at_end(cond_bb);
                 let cond_val = self.gen_expr(cond);
                 let zero = self.i64_type.const_int(0, false);
                 let is_truthy = self.module.get_function("KAIN_is_truthy").unwrap(); let truthy_val = self.builder.build_call(is_truthy, &[cond_val.into()], "while_truthy").unwrap().try_as_basic_value().unwrap_basic().into_int_value(); let is_true = self.builder.build_int_compare(IntPredicate::NE, truthy_val, zero, "whilecheck").unwrap();
                 self.builder.build_conditional_branch(is_true, body_bb, end_bb).unwrap();
                 
                 self.builder.position_at_end(body_bb);
                 self.loop_stack.push((cond_bb, end_bb));
                 for s in body { self.gen_stmt(s); }
                 
                 if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                     self.builder.build_unconditional_branch(cond_bb).unwrap();
                 }
                 
                 self.loop_stack.pop();
                 self.builder.position_at_end(end_bb);
            },
            Stmt::Break => {
                if let Some((_, end_bb)) = self.loop_stack.last() {
                    self.builder.build_unconditional_branch(*end_bb).unwrap();
                }
            },
            Stmt::Continue => {
                if let Some((cond_bb, _)) = self.loop_stack.last() {
                    self.builder.build_unconditional_branch(*cond_bb).unwrap();
                }
            },
            Stmt::For(var, iter, body) => {
                 // Desugar to while loop:
                 // let _iter = iter;
                 // let _len = len(_iter); // or just array access
                 // let _i = 0;
                 // while _i < _len {
                 //     let var = _iter[_i];
                 //     body;
                 //     _i += 1;
                 // }
                 
                 let iter_val = self.gen_expr(iter);
                 let iter_var = "iter";
                 let len_var = "len";
                 let idx_var = "i";
                 
                 // Store iter
                 let iter_alloca = self.builder.build_alloca(self.i64_type, iter_var).unwrap();
                 self.builder.build_store(iter_alloca, iter_val).unwrap();
                 
                 // Get len
                 let len_fn = self.module.get_function("KAIN_array_len").unwrap(); // Runtime must have this
                 let len_val = self.builder.build_call(len_fn, &[iter_val.into()], "len").unwrap().try_as_basic_value().unwrap_basic().into_int_value();
                 let len_alloca = self.builder.build_alloca(self.i64_type, len_var).unwrap();
                 self.builder.build_store(len_alloca, len_val).unwrap();
                 
                 // Init index = 0
                 let zero = self.i64_type.const_int(0, false);
                 let idx_val = self.box_int(zero); // Boxed 0
                 let idx_alloca = self.builder.build_alloca(self.i64_type, idx_var).unwrap();
                 self.builder.build_store(idx_alloca, idx_val).unwrap();
                 
                 // Loop structure
                 let parent = self.builder.get_insert_block().unwrap().get_parent().unwrap();
                 let cond_bb = self.context.append_basic_block(parent, "for_cond");
                 let body_bb = self.context.append_basic_block(parent, "for_body");
                 let end_bb = self.context.append_basic_block(parent, "for_end");
                 
                 self.builder.build_unconditional_branch(cond_bb).unwrap();
                 
                 // Condition: i < len
                 self.builder.position_at_end(cond_bb);
                 let curr_idx = self.builder.build_load(self.i64_type, idx_alloca, "curr_idx").unwrap().into_int_value();
                 let curr_len = self.builder.build_load(self.i64_type, len_alloca, "curr_len").unwrap().into_int_value();
                 
                 // Unbox for comparison? Or use KAIN_lt_op?
                 // Array len returns raw int? No, core runtime functions return boxed/values usually.
                 // Wait, KAIN_array_len returns raw int? No, it returns Value (boxed).
                 // Use KAIN_lt_op
                 let lt_fn = self.module.get_function("KAIN_lt_op").unwrap();
                 let lt_res = self.builder.build_call(lt_fn, &[curr_idx.into(), curr_len.into()], "lt").unwrap().try_as_basic_value().unwrap_basic().into_int_value();
                 let is_truthy_fn = self.module.get_function("KAIN_is_truthy").unwrap();
                 let is_true_val = self.builder.build_call(is_truthy_fn, &[lt_res.into()], "truthy").unwrap().try_as_basic_value().unwrap_basic().into_int_value();
                 
                 let zero_raw = self.i64_type.const_int(0, false);
                 let cond_bool = self.builder.build_int_compare(IntPredicate::NE, is_true_val, zero_raw, "cond").unwrap();
                 
                 self.builder.build_conditional_branch(cond_bool, body_bb, end_bb).unwrap();
                 
                 // Body
                 self.builder.position_at_end(body_bb);
                 self.loop_stack.push((cond_bb, end_bb));
                 
                 // let var = iter[i]
                 let get_fn = self.module.get_function("KAIN_array_get").unwrap();
                 let elem_val = self.builder.build_call(get_fn, &[iter_val.into(), curr_idx.into()], "elem").unwrap().try_as_basic_value().unwrap_basic().into_int_value();
                 
                 let var_alloca = self.builder.build_alloca(self.i64_type, &var).unwrap();
                 self.builder.build_store(var_alloca, elem_val).unwrap();
                 self.variables.insert(var.clone(), var_alloca);
                 self.initialized_vars.insert(var.clone());
                 
                 for s in body { self.gen_stmt(s); }
                 
                 // Increment i
                 let add_fn = self.module.get_function("KAIN_add_op").unwrap();
                 let one = self.box_int(self.i64_type.const_int(1, false));
                 let next_idx = self.builder.build_call(add_fn, &[curr_idx.into(), one.into()], "inc").unwrap().try_as_basic_value().unwrap_basic().into_int_value();
                 self.builder.build_store(idx_alloca, next_idx).unwrap();
                 
                 if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                     self.builder.build_unconditional_branch(cond_bb).unwrap();
                 }
                 
                 self.loop_stack.pop();
                 self.variables.remove(&var); // Scope exit (simple)
                 
                 self.builder.position_at_end(end_bb);
            },
            Stmt::Loop(body) => {
                 let parent = self.builder.get_insert_block().unwrap().get_parent().unwrap();
                 let loop_bb = self.context.append_basic_block(parent, "loop_start");
                 let end_bb = self.context.append_basic_block(parent, "loop_end");
                 
                 self.builder.build_unconditional_branch(loop_bb).unwrap();
                 self.builder.position_at_end(loop_bb);
                 
                 self.loop_stack.push((loop_bb, end_bb));
                 for s in body { self.gen_stmt(s); }
                 
                 if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                     self.builder.build_unconditional_branch(loop_bb).unwrap();
                 }
                 self.loop_stack.pop();
                 
                 self.builder.position_at_end(end_bb);
            },
            Stmt::Match(_, _) => {
                // TODO: Implement Match (complex)
                // For now, emit a warning or panic?
                // Or try to implement basic one?
                // Just do basic no-op or panic if reached
                // panic!("Match not implemented in bootstrap");
            },
            _ => { println!("Warning: Unhandled stmt: {:?}", stmt); }
        }
    }
    
    fn gen_expr(&mut self, expr: Expr) -> IntValue<'ctx> {
        match expr {
            Expr::Int(n) => {
                let val = self.i64_type.const_int(n as u64, false);
                self.box_int(val)
            },
            Expr::String(s) => {
                let ptr = if let Some(ptr) = self.string_literals.get(&s) {
                    *ptr
                } else {
                    // Create string with 8-byte alignment (required for pointer compression)
                    let string_val = self.context.const_string(s.as_bytes(), true);
                    let name = format!("str_{}", self.string_id);
                    self.string_id += 1;
                    let global = self.module.add_global(string_val.get_type(), Some(AddressSpace::default()), &name);
                    global.set_initializer(&string_val);
                    global.set_constant(true);
                    global.set_unnamed_addr(true);
                    global.set_alignment(8);  // CRITICAL: 8-byte alignment for pointer shift!

                    let p = global.as_pointer_value();
                    self.string_literals.insert(s.clone(), p);
                    p
                };
                self.box_string(ptr)  // Fixed: use box_string() not box_ptr()
            },
            Expr::Ident(name) => {
                if let Some(ptr) = self.variables.get(&name) {
                    if !self.initialized_vars.contains(&name) {
                        println!("ERROR: Variable '{}' used before initialization", name);
                        return self.i64_type.const_int(0, false);
                    }
                    
                    // FIXED: Only struct PARAMETERS need box_ptr treatment
                    // (tracked in struct_params - where variable IS the pointer)
                    // Struct LOCAL variables contain boxed pointers and should be loaded normally
                    if self.struct_params.contains(&name) {
                        // Struct parameter: convert pointer to i64 (boxed pointer)
                        return self.box_ptr(*ptr);
                    }
                    
                    // Non-struct-param: load the i64 value
                    self.builder.build_load(self.i64_type, *ptr, &name).unwrap().into_int_value()
                } else {
                    println!("ERROR: Undefined variable '{}'", name);
                    return self.i64_type.const_int(0, false);
                }
            },
            Expr::Struct(name, fields) => {
                 let num_fields = self.field_indices.get(&name).map(|m| m.len()).unwrap_or(fields.len());
                 let size = num_fields * 8;
                 let size_val = self.i64_type.const_int(size as u64, false);
                 
                 let malloc = self.module.get_function("malloc").unwrap();
                 let ptr_i8 = self.builder.build_call(malloc, &[size_val.into()], "malloc").unwrap()
                     .try_as_basic_value().unwrap_basic().into_pointer_value();
                 
                if let Some(struct_ty) = self.struct_types.get(&name).cloned() {
                    let ptr_struct = self.builder.build_pointer_cast(ptr_i8, self.context.ptr_type(AddressSpace::default()), "struct_cast").unwrap();
                    for init in fields {
                        if let Some(idx) = self.get_field_index(&name, &init.name) {
                            let val = self.gen_expr(init.value.clone());
                            let field_ptr = self.builder.build_struct_gep(struct_ty, ptr_struct, idx, "field_ptr").unwrap();
                            self.builder.build_store(field_ptr, val).unwrap();
                        }
                    }
                } else {
                    // Fallback: treat as i64 array and store sequentially by field order
                    let ptr_i64 = self.builder.build_pointer_cast(ptr_i8, self.context.ptr_type(AddressSpace::default()), "struct_fallback_cast").unwrap();
                    for (i, init) in fields.iter().enumerate() {
                        let val = self.gen_expr(init.value.clone());
                        let idx = self.i64_type.const_int(i as u64, false);
                        let field_ptr = unsafe { self.builder.build_in_bounds_gep(self.i64_type, ptr_i64, &[idx], "struct_fallback_gep").unwrap() };
                        self.builder.build_store(field_ptr, val).unwrap();
                    }
                }
                 
                 self.box_ptr(ptr_i8)
            },
            Expr::Array(elements) => {
                 let malloc = self.module.get_function("malloc").unwrap();
                 
                 // 1. Allocate KAINArray header (3 words: data_ptr, len, cap) - RUNTIME ORDER!
                 let header_size = self.i64_type.const_int(24, false);
                 let header_ptr_i8 = self.builder.build_call(malloc, &[header_size.into()], "arr_header_malloc").unwrap()
                     .try_as_basic_value().unwrap_basic().into_pointer_value();
                 
                 // 2. Allocate data buffer
                 let data_size = self.i64_type.const_int((elements.len().max(4) * 8) as u64, false);
                 let data_ptr_i8 = self.builder.build_call(malloc, &[data_size.into()], "arr_data_malloc").unwrap()
                     .try_as_basic_value().unwrap_basic().into_pointer_value();
                 
                // 3. Set data_ptr (index 0)
                let header_arr_ty = self.i64_type.array_type(3);
                let data_as_i64 = self.builder.build_ptr_to_int(data_ptr_i8, self.i64_type, "data_ptr_as_i64").unwrap();
                let data_ptr = unsafe { self.builder.build_in_bounds_gep(header_arr_ty, header_ptr_i8, &[self.i64_type.const_int(0, false), self.i64_type.const_int(0, false)], "data_ptr").unwrap() };
                self.builder.build_store(data_ptr, data_as_i64).unwrap();
                 
                 // 4. Set len (index 1)
                 let len_val = self.i64_type.const_int(elements.len() as u64, false);
                let header_arr_ty = self.i64_type.array_type(3);
                let header_ptr_arr = self.builder.build_pointer_cast(header_ptr_i8, self.context.ptr_type(AddressSpace::default()), "header_arr_cast").unwrap();
                let len_ptr = unsafe { self.builder.build_in_bounds_gep(header_arr_ty, header_ptr_arr, &[self.i64_type.const_int(0, false), self.i64_type.const_int(1, false)], "len_ptr").unwrap() };
                self.builder.build_store(len_ptr, len_val).unwrap();
                 
                 // 5. Set cap (index 2)
                let cap_val = self.i64_type.const_int(elements.len().max(4) as u64, false);
                let cap_ptr = unsafe { self.builder.build_in_bounds_gep(header_arr_ty, header_ptr_arr, &[self.i64_type.const_int(0, false), self.i64_type.const_int(2, false)], "cap_ptr").unwrap() };
                self.builder.build_store(cap_ptr, cap_val).unwrap();
                 
                 // 6. Populate data
                 let data_arr_ty = self.i64_type.array_type(elements.len().max(4) as u32);
                let data_ptr_arr = self.builder.build_pointer_cast(data_ptr_i8, self.context.ptr_type(AddressSpace::default()), "data_arr_cast").unwrap();
                for (i, el) in elements.iter().enumerate() {
                    let val = self.gen_expr(el.clone());
                    let el_ptr = unsafe { self.builder.build_in_bounds_gep(data_arr_ty, data_ptr_arr, &[self.i64_type.const_int(0, false), self.i64_type.const_int(i as u64, false)], "el_ptr").unwrap() };
                    self.builder.build_store(el_ptr, val).unwrap();
                }
                 
                 self.box_ptr(header_ptr_i8)
            },
            Expr::Tuple(elements) => {
                 let count = elements.len();
                 // Allocate size = count * 8 bytes
                 let size = (count * 8).max(8); // Min 8 bytes to be safe
                 let size_val = self.i64_type.const_int(size as u64, false);
                 
                 let malloc = self.module.get_function("malloc").unwrap();
                 let ptr_i8 = self.builder.build_call(malloc, &[size_val.into()], "tuple_malloc").unwrap()
                     .try_as_basic_value().unwrap_basic().into_pointer_value();
                     
                 // Cast to i64* for element storage
                    let ptr_i64 = self.builder.build_pointer_cast(ptr_i8, self.context.ptr_type(AddressSpace::default()), "tuple_cast").unwrap();
                 
                 for (i, el) in elements.iter().enumerate() {
                     let val = self.gen_expr(el.clone());
                     
                     // GEP
                     let idx = self.i64_type.const_int(i as u64, false);
                     let el_ptr = unsafe {
                         self.builder.build_in_bounds_gep(self.i64_type, ptr_i64, &[idx], "tuple_el_ptr").unwrap()
                     };
                     self.builder.build_store(el_ptr, val).unwrap();
                 }
                 
                 self.box_ptr(ptr_i8)
            },
            Expr::Field(obj, field) => {
                 // Check if field is numeric (Tuple index)
                 if let Ok(idx) = field.parse::<u32>() {
                     // Tuple access!
                     let obj_val = self.gen_expr(*obj.clone());
                     // Unbox ptr - Tuples are stored as pointers to i64 (or array of i64)
                    let ptr_i8 = self.unbox_ptr(obj_val, self.context.ptr_type(AddressSpace::default()));
                    let ptr_i64 = self.builder.build_pointer_cast(ptr_i8, self.context.ptr_type(AddressSpace::default()), "tuple_cast").unwrap();
                     
                     let idx_val = self.i64_type.const_int(idx as u64, false);
                     let el_ptr = unsafe {
                         self.builder.build_in_bounds_gep(self.i64_type, ptr_i64, &[idx_val], "tuple_elem_ptr").unwrap()
                     };
                     self.builder.build_load(self.i64_type, el_ptr, "tuple_load").unwrap().into_int_value()
                 } else if let Some(struct_name) = self.infer_expr_type(&obj) {
                     let obj_val = self.gen_expr(*obj.clone());
                     if let Some(struct_ty) = self.struct_types.get(&struct_name).cloned() {
                         let ptr_ty = self.context.ptr_type(AddressSpace::default());
                         let ptr_struct = self.unbox_ptr(obj_val, ptr_ty);
                         
                         if let Some(idx) = self.get_field_index(&struct_name, &field) {
                             let field_ptr = self.builder.build_struct_gep(struct_ty, ptr_struct, idx, "field_gep").unwrap();
                             self.builder.build_load(self.i64_type, field_ptr, "field_load").unwrap().into_int_value()
                         } else {
                             // Maybe a method call syntax on struct without parens? (Property getter?)
                             // For now return 0
                             self.i64_type.const_int(0, false)
                         }
                     } else {
                         self.i64_type.const_int(0, false)
                     }
                 } else {
                     self.i64_type.const_int(0, false)
                 }
            },
            Expr::Call(callee, args) => {
                 let func_name = if let Expr::Ident(name) = *callee { name } else { "".to_string() };
                 
                 // Map KAIN function names to runtime function names
                 let mapped_name = match func_name.as_str() {
                     "contains" => "KAIN_contains",
                     "str_eq" => "KAIN_str_eq",
                     "array_get" => "KAIN_array_get",
                     "array_len" => "KAIN_array_len",
                     "array_push" => "KAIN_array_push",
                     "array_new" => "KAIN_array_new",
                     "map_get" => "KAIN_map_get",
                     "map_set" => "KAIN_map_set",
                     "str_len" => "KAIN_str_len",
                     "substring" => "KAIN_substring",
                     "to_string" => "KAIN_to_string",
                     "println" => "KAIN_println_str",
                     "print" => "KAIN_print_str",
                     _ => &func_name
                 };
                 
                let f = match self.module.get_function(mapped_name) {
                    Some(f) => f,
                    None => return self.i64_type.const_int(0, false),
                };
                // FIXED: Type-aware argument casting - unbox first for ptr params
                let mut arg_vals = vec![];
                    for (i, arg) in args.iter().enumerate() {
                        let arg_val = self.gen_expr(arg.clone());
                        
                        // Check if this parameter expects a pointer type (struct)
                        if let Some(param) = f.get_nth_param(i as u32) {
                            if param.get_type().is_pointer_type() {
                                // Parameter expects ptr - unbox the boxed pointer first!
                                let ptr_type = param.get_type().into_pointer_type();
                                let casted = self.unbox_ptr(arg_val, ptr_type);
                                arg_vals.push(casted.into());
                            } else {
                                // Parameter expects i64, argument is i64 → no cast needed
                                arg_vals.push(arg_val.into());
                            }
                        } else {
                            // No type info, pass as-is
                            arg_vals.push(arg_val.into());
                        }
                    }
                    
                    let declared = f.count_params() as usize;
                    if arg_vals.len() > declared {
                        arg_vals.truncate(declared);
                    } else if arg_vals.len() < declared {
                        for idx in arg_vals.len()..declared {
                            if let Some(param) = f.get_nth_param(idx as u32) {
                                if param.get_type().is_pointer_type() {
                                    let null_ptr = param.get_type().into_pointer_type().const_null();
                                    arg_vals.push(null_ptr.into());
                                } else {
                                    arg_vals.push(self.i64_type.const_int(0, false).into());
                                }
                            } else {
                                arg_vals.push(self.i64_type.const_int(0, false).into());
                            }
                        }
                    }
                    let call = self.builder.build_call(f, &arg_vals, "call").unwrap();
                    match call.try_as_basic_value().basic() {
                        Some(val) => val.into_int_value(),
                        None => self.i64_type.const_int(0, false),
                    }
                
            },

            Expr::MethodCall(obj, method_name, args) => {
                 let obj_type = self.infer_expr_type(&obj).unwrap_or("".to_string());
                 let mangled = format!("{}_{}", obj_type, method_name);
                 
                 let obj_val = self.gen_expr(*obj.clone());
                 
                let f = match self.module.get_function(&mangled) {
                    Some(f) => f,
                    None => return self.i64_type.const_int(0, false),
                };
                // FIXED: Type-aware argument casting for methods - unbox first!
                let mut call_args: Vec<BasicMetadataValueEnum> = vec![];
                     
                     // Process first argument (self/obj)
                     if let Some(param) = f.get_nth_param(0) {
                         if param.get_type().is_pointer_type() {
                             // Unbox the boxed pointer to get raw struct ptr
                             let ptr_type = param.get_type().into_pointer_type();
                             let casted = self.unbox_ptr(obj_val, ptr_type);
                             call_args.push(casted.into());
                         } else {
                             call_args.push(obj_val.into());
                         }
                     } else {
                         call_args.push(obj_val.into());
                     }
                     
                     // Process remaining arguments
                     for (i, arg) in args.iter().enumerate() {
                         let arg_val = self.gen_expr(arg.clone());
                         let param_idx = i + 1; // +1 because of self
                         
                         if let Some(param) = f.get_nth_param(param_idx as u32) {
                             if param.get_type().is_pointer_type() {
                                 // Unbox the boxed pointer first
                                 let ptr_type = param.get_type().into_pointer_type();
                                 let casted = self.unbox_ptr(arg_val, ptr_type);
                                 call_args.push(casted.into());
                             } else {
                                 call_args.push(arg_val.into());
                             }
                         } else {
                             call_args.push(arg_val.into());
                         }
                     }
                     
                     // Align arity with declared function
                     let declared = f.count_params() as usize;
                     if call_args.len() > declared {
                         call_args.truncate(declared);
                     } else if call_args.len() < declared {
                         for idx in call_args.len()..declared {
                             if let Some(param) = f.get_nth_param(idx as u32) {
                                 if param.get_type().is_pointer_type() {
                                     let null_ptr = param.get_type().into_pointer_type().const_null();
                                     call_args.push(null_ptr.into());
                                 } else {
                                     call_args.push(self.i64_type.const_int(0, false).into());
                                 }
                             } else {
                                 call_args.push(self.i64_type.const_int(0, false).into());
                             }
                         }
                     }
                     let call = self.builder.build_call(f, &call_args, "mcall").unwrap();
                     match call.try_as_basic_value().basic() {
                        Some(val) => val.into_int_value(),
                        None => self.i64_type.const_int(0, false),
                     }
                
            },
            Expr::Binary(lhs, op, rhs) => {
                // Handle logical operators separately (need truthiness via runtime call)
                if op == "||" || op == "&&" {
                    let l = self.gen_expr(*lhs);
                    let r = self.gen_expr(*rhs);
                    
                    // Call KAIN_is_truthy to properly handle NaN-boxed booleans
                    let is_truthy = self.module.get_function("KAIN_is_truthy").unwrap();
                    let l_truthy = self.builder.build_call(is_truthy, &[l.into()], "l_truthy").unwrap()
                        .try_as_basic_value().unwrap_basic().into_int_value();
                    let r_truthy = self.builder.build_call(is_truthy, &[r.into()], "r_truthy").unwrap()
                        .try_as_basic_value().unwrap_basic().into_int_value();
                    
                    // Convert to i1 booleans for LLVM operations
                    let zero = self.i64_type.const_int(0, false);
                    let l_bool = self.builder.build_int_compare(IntPredicate::NE, l_truthy, zero, "l_bool").unwrap();
                    let r_bool = self.builder.build_int_compare(IntPredicate::NE, r_truthy, zero, "r_bool").unwrap();
                    
                    // Apply logical operation
                    let result_bool = if op == "||" {
                        self.builder.build_or(l_bool, r_bool, "or_result").unwrap()
                    } else {
                        self.builder.build_and(l_bool, r_bool, "and_result").unwrap()
                    };
                    
                    // Convert back to i64 (1 or 0)
                    self.builder.build_int_z_extend(result_bool, self.i64_type, "bool_to_i64").unwrap()
                } else {
                    let l = self.gen_expr(*lhs);
                    let r = self.gen_expr(*rhs);
                    
                    // Handle bitwise operators directly with LLVM instructions
                    match op.as_str() {
                        "&" => {
                            return self.builder.build_and(l, r, "bitwise_and").unwrap();
                        },
                        "|" => {
                            return self.builder.build_or(l, r, "bitwise_or").unwrap();
                        },
                        "^" => {
                            return self.builder.build_xor(l, r, "bitwise_xor").unwrap();
                        },
                        _ => {}
                    }
                    
                    let func_name = match op.as_str() {
                        "+" => "KAIN_add_op",
                        "-" => "KAIN_sub_op",
                        "*" => "KAIN_mul_op",
                        "/" => "KAIN_div_op",
                        "%" => "KAIN_rem_op",
                        "==" => "KAIN_eq_op",
                        "!=" => "KAIN_neq_op",
                        "<" => "KAIN_lt_op",
                        "<=" => "KAIN_le_op",
                        ">" => "KAIN_gt_op",
                        ">=" => "KAIN_ge_op",
                        _ => "",
                    };
                    
                    if !func_name.is_empty() {
                        if let Some(f) = self.module.get_function(func_name) {
                             let call = self.builder.build_call(f, &[l.into(), r.into()], "binop").unwrap();
                             match call.try_as_basic_value().basic() {
                                Some(val) => val.into_int_value(),
                                None => self.i64_type.const_int(0, false),
                             }
                        } else {
                             self.i64_type.const_int(0, false)
                        }
                    } else {
                         self.i64_type.const_int(0, false)
                    }
                }
            },


            Expr::EnumVariant(type_name, variant_name, args) => {
                 let mangled = format!("{}_{}", type_name, variant_name);
                 let f = match self.module.get_function(&mangled) {
                     Some(f) => f,
                     None => return self.i64_type.const_int(0, false),
                 };
                 let mut arg_vals: Vec<BasicMetadataValueEnum> = vec![];
                 for (i, arg) in args.iter().enumerate() {
                     let arg_val = self.gen_expr(arg.clone());
                     if let Some(param) = f.get_nth_param(i as u32) {
                         if param.get_type().is_pointer_type() {
                             let ptr_type = param.get_type().into_pointer_type();
                             let casted = self.unbox_ptr(arg_val, ptr_type);
                             arg_vals.push(casted.into());
                         } else {
                             arg_vals.push(arg_val.into());
                         }
                     } else {
                         arg_vals.push(arg_val.into());
                     }
                 }
                 let declared = f.count_params() as usize;
                 if arg_vals.len() > declared {
                     arg_vals.truncate(declared);
                 } else if arg_vals.len() < declared {
                     for idx in arg_vals.len()..declared {
                         if let Some(param) = f.get_nth_param(idx as u32) {
                             if param.get_type().is_pointer_type() {
                                 let null_ptr = param.get_type().into_pointer_type().const_null();
                                 arg_vals.push(null_ptr.into());
                             } else {
                                 arg_vals.push(self.i64_type.const_int(0, false).into());
                             }
                         } else {
                             arg_vals.push(self.i64_type.const_int(0, false).into());
                         }
                     }
                 }
                 let call = self.builder.build_call(f, &arg_vals, "static_call").unwrap();
                 match call.try_as_basic_value().basic() {
                     Some(val) => val.into_int_value(),
                     None => self.i64_type.const_int(0, false),
                 }
            },

            Expr::Unary(op, operand) => {
                let val = self.gen_expr(*operand);
                match op.as_str() {
                    "!" => {
                        // Logical NOT: call KAIN_is_truthy, then compare result == 0
                        let is_truthy = self.module.get_function("KAIN_is_truthy").unwrap();
                        let truthy_val = self.builder.build_call(is_truthy, &[val.into()], "truthy").unwrap()
                            .try_as_basic_value().unwrap_basic().into_int_value();
                        let zero = self.i64_type.const_int(0, false);
                        let is_falsy = self.builder.build_int_compare(IntPredicate::EQ, truthy_val, zero, "is_falsy").unwrap();
                        self.builder.build_int_z_extend(is_falsy, self.i64_type, "not_result").unwrap()
                    },
                    "-" => {
                        // Unary minus: 0 - val
                        let zero = self.i64_type.const_int(0, false);
                        self.builder.build_int_sub(zero, val, "neg").unwrap()
                    },
                    _ => val
                }
            },
            Expr::Index(array, index) => {
                // Array indexing: arr[i] => KAIN_array_get(arr, i)
                let arr_val = self.gen_expr(*array);
                let idx_val = self.gen_expr(*index);
                if let Some(f) = self.module.get_function("KAIN_array_get") {
                    let call = self.builder.build_call(f, &[arr_val.into(), idx_val.into()], "array_get").unwrap();
                    match call.try_as_basic_value().basic() {
                        Some(val) => val.into_int_value(),
                        None => self.i64_type.const_int(0, false),
                    }
                } else {
                    self.i64_type.const_int(0, false)
                }
            },
            _ => self.i64_type.const_int(0, false)

        }
    }
    
    fn get_field_index(&self, struct_name: &str, field_name: &str) -> Option<u32> {
        self.field_indices.get(struct_name)?.get(field_name).cloned()
    }

    fn get_variant_tag(&self, type_name: &str, variant_name: &str) -> Option<usize> {
        self.scoped_variant_map.get(&(type_name.to_string(), variant_name.to_string())).cloned()
    }
}
