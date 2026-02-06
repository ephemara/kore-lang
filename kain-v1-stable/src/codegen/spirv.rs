//! SPIR-V Code Generation for GPU shaders

use crate::types::{TypedProgram, TypedItem, TypedShader};
use crate::error::{KainResult, KainError};
use crate::ast::{Type, ShaderStage, Expr, Stmt, Block, BinaryOp};
use rspirv::binary::Assemble;
use rspirv::dr::{Builder, Operand};
use rspirv::spirv::{Capability, AddressingModel, MemoryModel, ExecutionModel, ExecutionMode, StorageClass, Decoration};
use std::collections::HashMap;

pub fn generate(program: &TypedProgram) -> KainResult<Vec<u8>> {
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
    // Cache GLSL extension import
    glsl_ext: Option<u32>,
}

fn emit_shader(b: &mut Builder, shader: &TypedShader) -> KainResult<()> {
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
        glsl_ext: None,
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

impl<'a> ShaderContext<'a> {
    fn get_glsl_ext(&mut self) -> u32 {
        if let Some(ext) = self.glsl_ext {
            ext
        } else {
            let ext = self.b.ext_inst_import("GLSL.std.450");
            self.glsl_ext = Some(ext);
            ext
        }
    }
}

fn emit_block(ctx: &mut ShaderContext, block: &Block) -> KainResult<()> {
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

fn emit_expr(ctx: &mut ShaderContext, expr: &Expr) -> KainResult<(u32, Type)> {
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
                 Err(KainError::codegen(format!("Unknown variable: {}", name), *span))
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
                _ => return Err(KainError::codegen("Unsupported binary op in shader", expr.span())),
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
                let float = ctx.b.type_float(32);
                
                // Vector constructors
                match name.as_str() {
                    "vec2" | "Vec2" if args.len() == 2 => {
                        let vec2 = ctx.b.type_vector(float, 2);
                        let mut components = vec![];
                        for arg in args {
                            let (val, _) = emit_expr(ctx, &arg.value)?;
                            components.push(val);
                        }
                        let res_id = ctx.b.composite_construct(vec2, None, components).unwrap();
                        return Ok((res_id, Type::Named { name: "Vec2".into(), generics: vec![], span: expr.span() }));
                    },
                    "vec3" | "Vec3" if args.len() == 3 => {
                        let vec3 = ctx.b.type_vector(float, 3);
                        let mut components = vec![];
                        for arg in args {
                            let (val, _) = emit_expr(ctx, &arg.value)?;
                            components.push(val);
                        }
                        let res_id = ctx.b.composite_construct(vec3, None, components).unwrap();
                        return Ok((res_id, Type::Named { name: "Vec3".into(), generics: vec![], span: expr.span() }));
                    },
                    "vec4" | "Vec4" if args.len() == 4 => {
                        let vec4 = ctx.b.type_vector(float, 4);
                        let mut components = vec![];
                        for arg in args {
                            let (val, _) = emit_expr(ctx, &arg.value)?;
                            components.push(val);
                        }
                        let res_id = ctx.b.composite_construct(vec4, None, components).unwrap();
                        return Ok((res_id, Type::Named { name: "Vec4".into(), generics: vec![], span: expr.span() }));
                    },
                    
                    // Math functions (GLSL extended instructions)
                    "sin" if args.len() == 1 => {
                        let (val, ty) = emit_expr(ctx, &args[0].value)?;
                        let res_ty = map_ast_type(ctx.b, &ty);
                        let glsl = ctx.get_glsl_ext();
                        let res_id = ctx.b.ext_inst(res_ty, None, glsl, 13, vec![Operand::IdRef(val)]).unwrap(); // Sin = 13
                        return Ok((res_id, ty));
                    },
                    "cos" if args.len() == 1 => {
                        let (val, ty) = emit_expr(ctx, &args[0].value)?;
                        let res_ty = map_ast_type(ctx.b, &ty);
                        let glsl = ctx.get_glsl_ext();
                        let res_id = ctx.b.ext_inst(res_ty, None, glsl, 14, vec![Operand::IdRef(val)]).unwrap(); // Cos = 14
                        return Ok((res_id, ty));
                    },
                    "tan" if args.len() == 1 => {
                        let (val, ty) = emit_expr(ctx, &args[0].value)?;
                        let res_ty = map_ast_type(ctx.b, &ty);
                        let glsl = ctx.b.ext_inst_import("GLSL.std.450");
                        let res_id = ctx.b.ext_inst(res_ty, None, glsl, 15, vec![Operand::IdRef(val)]).unwrap(); // Tan = 15
                        return Ok((res_id, ty));
                    },
                    "pow" if args.len() == 2 => {
                        let (base, ty) = emit_expr(ctx, &args[0].value)?;
                        let (exp, _) = emit_expr(ctx, &args[1].value)?;
                        let res_ty = map_ast_type(ctx.b, &ty);
                        let glsl = ctx.b.ext_inst_import("GLSL.std.450");
                        let res_id = ctx.b.ext_inst(res_ty, None, glsl, 26, vec![Operand::IdRef(base), Operand::IdRef(exp)]).unwrap(); // Pow = 26
                        return Ok((res_id, ty));
                    },
                    "sqrt" if args.len() == 1 => {
                        let (val, ty) = emit_expr(ctx, &args[0].value)?;
                        let res_ty = map_ast_type(ctx.b, &ty);
                        let glsl = ctx.b.ext_inst_import("GLSL.std.450");
                        let res_id = ctx.b.ext_inst(res_ty, None, glsl, 31, vec![Operand::IdRef(val)]).unwrap(); // Sqrt = 31
                        return Ok((res_id, ty));
                    },
                    "abs" if args.len() == 1 => {
                        let (val, ty) = emit_expr(ctx, &args[0].value)?;
                        let res_ty = map_ast_type(ctx.b, &ty);
                        let glsl = ctx.b.ext_inst_import("GLSL.std.450");
                        let res_id = ctx.b.ext_inst(res_ty, None, glsl, 4, vec![Operand::IdRef(val)]).unwrap(); // FAbs = 4
                        return Ok((res_id, ty));
                    },
                    "floor" if args.len() == 1 => {
                        let (val, ty) = emit_expr(ctx, &args[0].value)?;
                        let res_ty = map_ast_type(ctx.b, &ty);
                        let glsl = ctx.b.ext_inst_import("GLSL.std.450");
                        let res_id = ctx.b.ext_inst(res_ty, None, glsl, 8, vec![Operand::IdRef(val)]).unwrap(); // Floor = 8
                        return Ok((res_id, ty));
                    },
                    "ceil" if args.len() == 1 => {
                        let (val, ty) = emit_expr(ctx, &args[0].value)?;
                        let res_ty = map_ast_type(ctx.b, &ty);
                        let glsl = ctx.b.ext_inst_import("GLSL.std.450");
                        let res_id = ctx.b.ext_inst(res_ty, None, glsl, 9, vec![Operand::IdRef(val)]).unwrap(); // Ceil = 9
                        return Ok((res_id, ty));
                    },
                    "fract" if args.len() == 1 => {
                        let (val, ty) = emit_expr(ctx, &args[0].value)?;
                        let res_ty = map_ast_type(ctx.b, &ty);
                        let glsl = ctx.b.ext_inst_import("GLSL.std.450");
                        let res_id = ctx.b.ext_inst(res_ty, None, glsl, 10, vec![Operand::IdRef(val)]).unwrap(); // Fract = 10
                        return Ok((res_id, ty));
                    },
                    "min" if args.len() == 2 => {
                        let (a, ty) = emit_expr(ctx, &args[0].value)?;
                        let (b, _) = emit_expr(ctx, &args[1].value)?;
                        let res_ty = map_ast_type(ctx.b, &ty);
                        let glsl = ctx.b.ext_inst_import("GLSL.std.450");
                        let res_id = ctx.b.ext_inst(res_ty, None, glsl, 37, vec![Operand::IdRef(a), Operand::IdRef(b)]).unwrap(); // FMin = 37
                        return Ok((res_id, ty));
                    },
                    "max" if args.len() == 2 => {
                        let (a, ty) = emit_expr(ctx, &args[0].value)?;
                        let (b, _) = emit_expr(ctx, &args[1].value)?;
                        let res_ty = map_ast_type(ctx.b, &ty);
                        let glsl = ctx.b.ext_inst_import("GLSL.std.450");
                        let res_id = ctx.b.ext_inst(res_ty, None, glsl, 40, vec![Operand::IdRef(a), Operand::IdRef(b)]).unwrap(); // FMax = 40
                        return Ok((res_id, ty));
                    },
                    "clamp" if args.len() == 3 => {
                        let (val, ty) = emit_expr(ctx, &args[0].value)?;
                        let (min_val, _) = emit_expr(ctx, &args[1].value)?;
                        let (max_val, _) = emit_expr(ctx, &args[2].value)?;
                        let res_ty = map_ast_type(ctx.b, &ty);
                        let glsl = ctx.b.ext_inst_import("GLSL.std.450");
                        let res_id = ctx.b.ext_inst(res_ty, None, glsl, 43, vec![Operand::IdRef(val), Operand::IdRef(min_val), Operand::IdRef(max_val)]).unwrap(); // FClamp = 43
                        return Ok((res_id, ty));
                    },
                    "mix" if args.len() == 3 => {
                        let (a, ty) = emit_expr(ctx, &args[0].value)?;
                        let (b, _) = emit_expr(ctx, &args[1].value)?;
                        let (t, _) = emit_expr(ctx, &args[2].value)?;
                        let res_ty = map_ast_type(ctx.b, &ty);
                        let glsl = ctx.b.ext_inst_import("GLSL.std.450");
                        let res_id = ctx.b.ext_inst(res_ty, None, glsl, 46, vec![Operand::IdRef(a), Operand::IdRef(b), Operand::IdRef(t)]).unwrap(); // FMix = 46
                        return Ok((res_id, ty));
                    },
                    "step" if args.len() == 2 => {
                        let (edge, ty) = emit_expr(ctx, &args[0].value)?;
                        let (x, _) = emit_expr(ctx, &args[1].value)?;
                        let res_ty = map_ast_type(ctx.b, &ty);
                        let glsl = ctx.b.ext_inst_import("GLSL.std.450");
                        let res_id = ctx.b.ext_inst(res_ty, None, glsl, 48, vec![Operand::IdRef(edge), Operand::IdRef(x)]).unwrap(); // Step = 48
                        return Ok((res_id, ty));
                    },
                    "smoothstep" if args.len() == 3 => {
                        let (edge0, ty) = emit_expr(ctx, &args[0].value)?;
                        let (edge1, _) = emit_expr(ctx, &args[1].value)?;
                        let (x, _) = emit_expr(ctx, &args[2].value)?;
                        let res_ty = map_ast_type(ctx.b, &ty);
                        let glsl = ctx.b.ext_inst_import("GLSL.std.450");
                        let res_id = ctx.b.ext_inst(res_ty, None, glsl, 49, vec![Operand::IdRef(edge0), Operand::IdRef(edge1), Operand::IdRef(x)]).unwrap(); // SmoothStep = 49
                        return Ok((res_id, ty));
                    },
                    "length" if args.len() == 1 => {
                        let (val, _) = emit_expr(ctx, &args[0].value)?;
                        let glsl = ctx.b.ext_inst_import("GLSL.std.450");
                        let res_id = ctx.b.ext_inst(float, None, glsl, 66, vec![Operand::IdRef(val)]).unwrap(); // Length = 66
                        return Ok((res_id, Type::Named { name: "Float".into(), generics: vec![], span: expr.span() }));
                    },
                    "normalize" if args.len() == 1 => {
                        let (val, ty) = emit_expr(ctx, &args[0].value)?;
                        let res_ty = map_ast_type(ctx.b, &ty);
                        let glsl = ctx.b.ext_inst_import("GLSL.std.450");
                        let res_id = ctx.b.ext_inst(res_ty, None, glsl, 69, vec![Operand::IdRef(val)]).unwrap(); // Normalize = 69
                        return Ok((res_id, ty));
                    },
                    "dot" if args.len() == 2 => {
                        let (a, _) = emit_expr(ctx, &args[0].value)?;
                        let (b, _) = emit_expr(ctx, &args[1].value)?;
                        let res_id = ctx.b.dot(float, None, a, b).unwrap();
                        return Ok((res_id, Type::Named { name: "Float".into(), generics: vec![], span: expr.span() }));
                    },
                    "cross" if args.len() == 2 => {
                        let (a, ty) = emit_expr(ctx, &args[0].value)?;
                        let (b, _) = emit_expr(ctx, &args[1].value)?;
                        let res_ty = map_ast_type(ctx.b, &ty);
                        let glsl = ctx.b.ext_inst_import("GLSL.std.450");
                        let res_id = ctx.b.ext_inst(res_ty, None, glsl, 68, vec![Operand::IdRef(a), Operand::IdRef(b)]).unwrap(); // Cross = 68
                        return Ok((res_id, ty));
                    },
                    "reflect" if args.len() == 2 => {
                        let (i, ty) = emit_expr(ctx, &args[0].value)?;
                        let (n, _) = emit_expr(ctx, &args[1].value)?;
                        let res_ty = map_ast_type(ctx.b, &ty);
                        let glsl = ctx.b.ext_inst_import("GLSL.std.450");
                        let res_id = ctx.b.ext_inst(res_ty, None, glsl, 71, vec![Operand::IdRef(i), Operand::IdRef(n)]).unwrap(); // Reflect = 71
                        return Ok((res_id, ty));
                    },
                    
                    // Texture sampling
                    "sample" if args.len() == 2 => {
                        let (sampler, _) = emit_expr(ctx, &args[0].value)?;
                        let (coords, _) = emit_expr(ctx, &args[1].value)?;
                        let vec4 = ctx.b.type_vector(float, 4);
                        let res_id = ctx.b.image_sample_implicit_lod(vec4, None, sampler, coords, None, std::iter::empty()).unwrap();
                        return Ok((res_id, Type::Named { name: "Vec4".into(), generics: vec![], span: expr.span() }));
                    },
                    "sample_lod" if args.len() == 3 => {
                        let (sampler, _) = emit_expr(ctx, &args[0].value)?;
                        let (coords, _) = emit_expr(ctx, &args[1].value)?;
                        let (lod, _) = emit_expr(ctx, &args[2].value)?;
                        let vec4 = ctx.b.type_vector(float, 4);
                        let res_id = ctx.b.image_sample_explicit_lod(vec4, None, sampler, coords, rspirv::spirv::ImageOperands::LOD, vec![Operand::IdRef(lod)]).unwrap();
                        return Ok((res_id, Type::Named { name: "Vec4".into(), generics: vec![], span: expr.span() }));
                    },
                    
                    _ => {}
                }
            }
            Err(KainError::codegen(format!("Unsupported function call in shader: {:?}", callee), expr.span()))
        },
        Expr::Float(f, span) => {
            let float = ctx.b.type_float(32);
            let val = ctx.b.constant_bit32(float, (*f as f32).to_bits());
            Ok((val, Type::Named { name: "Float".into(), generics: vec![], span: *span }))
        },
        Expr::Field { object, field, span } => {
            let (obj_id, _obj_ty) = emit_expr(ctx, object)?;
            
            // Swizzle/component access
            let float = ctx.b.type_float(32);
            match field.as_str() {
                // Single component access
                "x" | "r" => {
                    let res_id = ctx.b.composite_extract(float, None, obj_id, vec![0]).unwrap();
                    Ok((res_id, Type::Named { name: "Float".into(), generics: vec![], span: *span }))
                },
                "y" | "g" => {
                    let res_id = ctx.b.composite_extract(float, None, obj_id, vec![1]).unwrap();
                    Ok((res_id, Type::Named { name: "Float".into(), generics: vec![], span: *span }))
                },
                "z" | "b" => {
                    let res_id = ctx.b.composite_extract(float, None, obj_id, vec![2]).unwrap();
                    Ok((res_id, Type::Named { name: "Float".into(), generics: vec![], span: *span }))
                },
                "w" | "a" => {
                    let res_id = ctx.b.composite_extract(float, None, obj_id, vec![3]).unwrap();
                    Ok((res_id, Type::Named { name: "Float".into(), generics: vec![], span: *span }))
                },
                // Vec2 swizzles
                "xy" | "rg" => {
                    let vec2 = ctx.b.type_vector(float, 2);
                    let res_id = ctx.b.vector_shuffle(vec2, None, obj_id, obj_id, vec![0, 1]).unwrap();
                    Ok((res_id, Type::Named { name: "Vec2".into(), generics: vec![], span: *span }))
                },
                "xz" | "rb" => {
                    let vec2 = ctx.b.type_vector(float, 2);
                    let res_id = ctx.b.vector_shuffle(vec2, None, obj_id, obj_id, vec![0, 2]).unwrap();
                    Ok((res_id, Type::Named { name: "Vec2".into(), generics: vec![], span: *span }))
                },
                "yz" | "gb" => {
                    let vec2 = ctx.b.type_vector(float, 2);
                    let res_id = ctx.b.vector_shuffle(vec2, None, obj_id, obj_id, vec![1, 2]).unwrap();
                    Ok((res_id, Type::Named { name: "Vec2".into(), generics: vec![], span: *span }))
                },
                // Vec3 swizzles
                "xyz" | "rgb" => {
                    let vec3 = ctx.b.type_vector(float, 3);
                    let res_id = ctx.b.vector_shuffle(vec3, None, obj_id, obj_id, vec![0, 1, 2]).unwrap();
                    Ok((res_id, Type::Named { name: "Vec3".into(), generics: vec![], span: *span }))
                },
                _ => Err(KainError::codegen(format!("Unsupported field access: {}", field), *span))
            }
        },
        _ => Err(KainError::codegen("Unsupported expression in shader", expr.span())),
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

