//! SPIR-V Code Generation for GPU shaders

use crate::types::{TypedProgram, TypedItem, TypedShader};
use crate::error::{KoreResult, KoreError};
use crate::ast::{Type, ShaderStage, Expr, Stmt, Block, BinaryOp};
use rspirv::binary::Assemble;
use rspirv::dr::{Builder, Operand};
use rspirv::spirv::{Capability, AddressingModel, MemoryModel, ExecutionModel, ExecutionMode, StorageClass, Decoration};
use std::collections::HashMap;

pub fn generate(program: &TypedProgram) -> KoreResult<Vec<u8>> {
    let mut builder = Builder::new();
    
    // Set capabilities and memory model
    builder.capability(Capability::Shader);
    // Add VulkanMemoryModel if targeting Vulkan, but GLSL450 is standard for now
    builder.memory_model(AddressingModel::Logical, MemoryModel::GLSL450);
    
    for item in &program.items {
        if let TypedItem::Shader(shader) = item {
            emit_shader(&mut builder, shader)?;
        }
    }
    
    let module = builder.module();
    let bytes: Vec<u8> = module.assemble().iter().flat_map(|w| w.to_le_bytes()).collect();
    Ok(bytes)
}

struct ShaderContext<'a> {
    b: &'a mut Builder,
    // Name -> (SPIR-V ID, AST Type, IsPointer)
    vars: HashMap<String, (u32, Type, bool)>,
    output_var: Option<u32>,
    // Track which variables are struct-wrapped uniforms (need AccessChain)
    struct_uniforms: std::collections::HashSet<String>,
}

fn emit_shader(b: &mut Builder, shader: &TypedShader) -> KoreResult<()> {
    let exec_model = match shader.ast.stage {
        ShaderStage::Vertex => ExecutionModel::Vertex,
        ShaderStage::Fragment => ExecutionModel::Fragment,
        ShaderStage::Compute => ExecutionModel::GLCompute,
    };
    
    // 1. Define Basic Types
    let void = b.type_void();
    
    // 2. Define Entry Point Function Type
    let fn_void_void = b.type_function(void, vec![]);
    
    // 3. Declare Variables (Global Interface)
    let mut interface_vars = vec![];
    let mut ctx_vars = HashMap::new();
    let mut struct_uniforms = std::collections::HashSet::new();

    // Inputs
    for (i, param) in shader.ast.inputs.iter().enumerate() {
        let ty = map_ast_type(b, &param.ty);
        let ptr_ty = b.type_pointer(None, StorageClass::Input, ty);
        let var = b.variable(ptr_ty, None, StorageClass::Input, None);
        b.decorate(var, Decoration::Location, vec![Operand::LiteralBit32(i as u32)]);
        interface_vars.push(var);
        ctx_vars.insert(param.name.clone(), (var, param.ty.clone(), true));
    }

    // Outputs
    let output_var = if !is_void(&shader.ast.outputs) {
         let output_ty = map_ast_type(b, &shader.ast.outputs);
         let ptr_ty = b.type_pointer(None, StorageClass::Output, output_ty);
         let var = b.variable(ptr_ty, None, StorageClass::Output, None);
         
         // Vertex shader output is @builtin(position) for Vec4, otherwise use Location
         if exec_model == ExecutionModel::Vertex && is_vec4(&shader.ast.outputs) {
             b.decorate(var, Decoration::BuiltIn, vec![Operand::BuiltIn(rspirv::spirv::BuiltIn::Position)]);
         } else {
             b.decorate(var, Decoration::Location, vec![Operand::LiteralBit32(0)]);
         }
         
         interface_vars.push(var);
         Some(var)
    } else {
        None
    };

    // Uniforms
    for uniform in &shader.ast.uniforms {
        let inner_ty = map_ast_type(b, &uniform.ty);
        
        // Check if this is a sampler type (uses UniformConstant) or data type (uses Uniform with struct)
        let is_sampler = matches!(&uniform.ty, Type::Named { name, .. } if name == "Sampler2D");
        
        if is_sampler {
            // Samplers use UniformConstant storage class directly
            let ptr_ty = b.type_pointer(None, StorageClass::UniformConstant, inner_ty);
            let var = b.variable(ptr_ty, None, StorageClass::UniformConstant, None);
            b.decorate(var, Decoration::DescriptorSet, vec![Operand::LiteralBit32(0)]);
            b.decorate(var, Decoration::Binding, vec![Operand::LiteralBit32(uniform.binding)]);
            ctx_vars.insert(uniform.name.clone(), (var, uniform.ty.clone(), true));
        } else {
            // Data uniforms (matrices, vectors, etc.) need a struct wrapper with Block decoration
            let struct_ty = b.type_struct(vec![inner_ty]);
            b.decorate(struct_ty, Decoration::Block, vec![]);
            // Offset decoration for the first (and only) member
            b.member_decorate(struct_ty, 0, Decoration::Offset, vec![Operand::LiteralBit32(0)]);
            
            // For matrices, we need ColMajor and MatrixStride decorations
            if matches!(&uniform.ty, Type::Named { name, .. } if name == "Mat4") {
                b.member_decorate(struct_ty, 0, Decoration::ColMajor, vec![]);
                b.member_decorate(struct_ty, 0, Decoration::MatrixStride, vec![Operand::LiteralBit32(16)]);
            }
            
            let ptr_ty = b.type_pointer(None, StorageClass::Uniform, struct_ty);
            let var = b.variable(ptr_ty, None, StorageClass::Uniform, None);
            b.decorate(var, Decoration::DescriptorSet, vec![Operand::LiteralBit32(0)]);
            b.decorate(var, Decoration::Binding, vec![Operand::LiteralBit32(uniform.binding)]);
            ctx_vars.insert(uniform.name.clone(), (var, uniform.ty.clone(), true));
            struct_uniforms.insert(uniform.name.clone());
        }
    }

    // 4. Function Body
    let main_fn = b.begin_function(void, None, rspirv::spirv::FunctionControl::NONE, fn_void_void).unwrap();
    b.begin_block(None).unwrap();

    let mut ctx = ShaderContext {
        b,
        vars: ctx_vars,
        output_var,
        struct_uniforms,
    };

    emit_block(&mut ctx, &shader.ast.body)?;

    // Ensure we always have a return
    if shader.ast.body.stmts.last().map_or(true, |s| !matches!(s, Stmt::Return(_, _))) {
        ctx.b.ret().unwrap();
    }
    
    ctx.b.end_function().unwrap();

    // 5. Entry Point
    b.entry_point(exec_model, main_fn, &shader.ast.name, interface_vars);
    
    if exec_model == ExecutionModel::Fragment {
        b.execution_mode(main_fn, ExecutionMode::OriginUpperLeft, vec![]);
    }
    
    Ok(())
}

fn emit_block(ctx: &mut ShaderContext, block: &Block) -> KoreResult<()> {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Return(expr, _) => {
                if let Some(expr) = expr {
                    if let Some(out_var) = ctx.output_var {
                        let (val, _) = emit_expr(ctx, expr)?;
                        ctx.b.store(out_var, val, None, vec![]).unwrap();
                    }
                }
                ctx.b.ret().unwrap();
            },
            Stmt::Let { pattern, value, .. } => {
                if let Some(value) = value {
                    let (val, ty) = emit_expr(ctx, value)?;
                    // For now, only simple bindings
                    if let crate::ast::Pattern::Binding { name, .. } = pattern {
                        // In SSA, we just map name -> value ID
                        // We don't support mutation of locals yet (need OpVariable + Store/Load)
                        ctx.vars.insert(name.clone(), (val, ty, false));
                    }
                }
            },
            Stmt::Expr(expr) => {
                emit_expr(ctx, expr)?;
            },
            _ => {} // Ignore others for now
        }
    }
    Ok(())
}

fn emit_expr(ctx: &mut ShaderContext, expr: &Expr) -> KoreResult<(u32, Type)> {
    match expr {
        Expr::Ident(name, span) => {
            if let Some((id, ty, is_ptr)) = ctx.vars.get(name).cloned() {
                if is_ptr {
                    // Need to load from pointer
                    let type_id = map_ast_type(ctx.b, &ty);
                    
                    // Check if this is a struct-wrapped uniform
                    if ctx.struct_uniforms.contains(name) {
                        // Use AccessChain to get pointer to member 0 of the struct
                        let ptr_ty = ctx.b.type_pointer(None, StorageClass::Uniform, type_id);
                        let int_ty = ctx.b.type_int(32, 0);
                        let zero = ctx.b.constant_bit32(int_ty, 0);
                        let member_ptr = ctx.b.access_chain(ptr_ty, None, id, vec![zero]).unwrap();
                        let val_id = ctx.b.load(type_id, None, member_ptr, None, std::iter::empty()).unwrap();
                        Ok((val_id, ty))
                    } else {
                        // Direct load for inputs and non-wrapped uniforms
                        let val_id = ctx.b.load(type_id, None, id, None, std::iter::empty()).unwrap();
                        Ok((val_id, ty))
                    }
                } else {
                    Ok((id, ty))
                }
            } else {
                 Err(KoreError::codegen(format!("Unknown variable: {}", name), *span))
            }
        },
        Expr::Binary { left, op, right, .. } => {
            let (lhs, lhs_ty) = emit_expr(ctx, left)?;
            let (rhs, rhs_ty) = emit_expr(ctx, right)?;
            
            // Map types to SPIR-V types
            let res_ty_id = map_ast_type(ctx.b, &lhs_ty); // Assume result type matches lhs for now
            
            let res_id = match op {
                BinaryOp::Mul => {
                    if is_mat4(&lhs_ty) && is_mat4(&rhs_ty) {
                        ctx.b.matrix_times_matrix(res_ty_id, None, lhs, rhs).unwrap()
                    } else if is_mat4(&lhs_ty) && is_vec4(&rhs_ty) {
                        // Mat4 * Vec4 -> Vec4
                         let vec4_ty = map_ast_type(ctx.b, &rhs_ty);
                         ctx.b.matrix_times_vector(vec4_ty, None, lhs, rhs).unwrap()
                    } else if is_vec4(&lhs_ty) && is_mat4(&rhs_ty) {
                        // Vec4 * Mat4 -> Vec4
                         let vec4_ty = map_ast_type(ctx.b, &lhs_ty);
                         ctx.b.vector_times_matrix(vec4_ty, None, lhs, rhs).unwrap()
                    } else if is_float(&lhs_ty) && is_float(&rhs_ty) {
                        ctx.b.f_mul(res_ty_id, None, lhs, rhs).unwrap()
                    } else {
                         // Fallback to FMul (vector * scalar, etc - simplified)
                        ctx.b.f_mul(res_ty_id, None, lhs, rhs).unwrap()
                    }
                },
                BinaryOp::Add => ctx.b.f_add(res_ty_id, None, lhs, rhs).unwrap(),
                BinaryOp::Sub => ctx.b.f_sub(res_ty_id, None, lhs, rhs).unwrap(),
                BinaryOp::Div => ctx.b.f_div(res_ty_id, None, lhs, rhs).unwrap(),
                _ => return Err(KoreError::codegen("Unsupported binary op in shader", expr.span())),
            };
            
            // Result type inference (simplified)
            let res_ty = if is_mat4(&lhs_ty) && is_vec4(&rhs_ty) {
                rhs_ty
            } else {
                lhs_ty
            };
            
            Ok((res_id, res_ty))
        },
        Expr::Call { callee, args, .. } => {
            if let Expr::Ident(name, _) = &**callee {
                if name == "Vec4" && args.len() == 4 {
                    // Constructor
                    let float = ctx.b.type_float(32);
                    let vec4 = ctx.b.type_vector(float, 4);
                    let mut components = vec![];
                    for arg in args {
                        let (val, _) = emit_expr(ctx, &arg.value)?;
                        components.push(val);
                    }
                    let res_id = ctx.b.composite_construct(vec4, None, components).unwrap();
                    return Ok((res_id, Type::Named { name: "Vec4".into(), generics: vec![], span: expr.span() }));
                }
            }
            Err(KoreError::codegen("Unsupported function call in shader", expr.span()))
        },
        Expr::Float(f, span) => {
            let float = ctx.b.type_float(32);
            let val = ctx.b.constant_bit32(float, (*f as f32).to_bits());
            Ok((val, Type::Named { name: "Float".into(), generics: vec![], span: *span }))
        },
        _ => Err(KoreError::codegen("Unsupported expression in shader", expr.span())),
    }
}

fn map_ast_type(b: &mut Builder, ty: &Type) -> u32 {
    let float = b.type_float(32);
    match ty {
        Type::Named { name, .. } => match name.as_str() {
            "Float" | "f32" => float,
            "Int" | "i32" => b.type_int(32, 1),
            "Bool" => b.type_bool(),
            "Vec2" => b.type_vector(float, 2),
            "Vec3" => b.type_vector(float, 3),
            "Vec4" => b.type_vector(float, 4),
            "Mat4" => {
                let v4 = b.type_vector(float, 4);
                b.type_matrix(v4, 4)
            },
            "Sampler2D" => {
                // Dim2D, NotDepth, Arrayed=False, MS=False, Sampled=1, Format=Unknown
                let image = b.type_image(float, rspirv::spirv::Dim::Dim2D, 0, 0, 0, 1, rspirv::spirv::ImageFormat::Unknown, None);
                b.type_sampled_image(image)
            },
            "StorageBuffer" => {
                // Struct wrapper needed for buffer block
                // Simplified: just array of floats for now
                let rt_array = b.type_runtime_array(float);
                let struct_ty = b.type_struct(vec![rt_array]);
                b.decorate(struct_ty, Decoration::Block, vec![]);
                struct_ty
            },
            "Void" => b.type_void(),
            _ => b.type_void(),
        },
        _ => b.type_void(),
    }
}

fn is_void(ty: &Type) -> bool {
    matches!(ty, Type::Named { name, .. } if name == "Void")
}

fn is_vec4(ty: &Type) -> bool {
    matches!(ty, Type::Named { name, .. } if name == "Vec4")
}

fn is_mat4(ty: &Type) -> bool {
    matches!(ty, Type::Named { name, .. } if name == "Mat4")
}

fn is_float(ty: &Type) -> bool {
    matches!(ty, Type::Named { name, .. } if name == "Float" || name == "f32")
}

