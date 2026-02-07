//! # KAIN - The Ultimate Programming Language
//! 
//! ## Philosophy
//! 1. **Rust's Safety** - Ownership, borrowing, no null, no data races
//! 2. **Python's Syntax** - Significant whitespace, minimal ceremony  
//! 3. **Lisp's Power** - Code as data, hygienic macros, DSL-friendly
//! 4. **Zig's Comptime** - Compile-time execution, no separate macro language
//! 5. **Effect Tracking** - Side effects in the type system
//! 6. **Actor Concurrency** - Erlang-style message passing built-in
//! 7. **Universal Targets** - WASM, LLVM native, SPIR-V shaders
//!
//! ## Example
//! ```KAIN
//! fn factorial(n: Int) -> Int with Pure:
//!     match n:
//!         0 => 1
//!         _ => n * factorial(n - 1)
//! ```

pub mod lexer;
pub mod ast;
pub mod parser;
pub mod types;
pub mod effects;
pub mod codegen;
pub mod runtime;
pub mod stdlib;
pub mod error;
pub mod span;
pub mod comptime;
pub mod diagnostics;
pub mod packager;
pub mod lsp;
pub mod monomorphize;


pub use lexer::Lexer;
pub use parser::Parser;
pub use ast::*;
pub use types::*;
pub use effects::*;
pub use error::KainError;
pub use span::Span;

/// Compile KAIN source to the specified target
pub fn compile(source: &str, target: CompileTarget) -> Result<Vec<u8>, KainError> {
    // 1. Lex
    let tokens = Lexer::new(source).tokenize()?;
    
    // 2. Parse
    let mut ast = Parser::new(&tokens).parse()?;
    
    // 2.5 Comptime Execution
    // Evaluate comptime blocks and expressions before type checking
    comptime::eval_program(&mut ast)?;

    // 3. Type check with effect inference
    let mut typed_ast = types::check(&ast)?;
    
    // 3.5 Monomorphization (for native targets and interpreter if we want to test lowering)
    if matches!(target, CompileTarget::Llvm | CompileTarget::Wasm | CompileTarget::SpirV | CompileTarget::Interpret) {
        let mono_prog = monomorphize::monomorphize(&typed_ast)?;
        // Replace items with monomorphized items
        // Since codegen expects TypedProgram, we can just update it.
        // But TypedProgram might have other fields later. 
        // For now, MonomorphizedProgram just has items.
        typed_ast.items = mono_prog.items; 
    }
    
    // 4. Generate code
    match target {
        CompileTarget::Wasm => codegen::wasm::generate(&typed_ast),
        #[cfg(feature = "llvm")]
        CompileTarget::Llvm => codegen::llvm::generate(&typed_ast),
        #[cfg(not(feature = "llvm"))]
        CompileTarget::Llvm => Err(KainError::codegen("LLVM backend not compiled. Rebuild with --features llvm", Span::new(0, 0))),
        CompileTarget::SpirV => codegen::spirv::generate(&typed_ast),
        CompileTarget::Hlsl => {
            let hlsl_code = codegen::hlsl::generate(&typed_ast)?;
            Ok(hlsl_code.into_bytes())
        },
        CompileTarget::Usf => {
            let usf_code = codegen::usf::generate(&typed_ast)?;
            Ok(usf_code.into_bytes())
        },
        CompileTarget::Js => {
            let js_code = codegen::js::generate(&typed_ast)?;
            Ok(js_code.into_bytes())
        },
        CompileTarget::Rust => {
            let rust_code = codegen::rust::generate(&typed_ast)?;
            Ok(rust_code.into_bytes())
        },
        CompileTarget::Interpret => {
            runtime::interpret(&typed_ast)?;
            Ok(vec![])
        }
        CompileTarget::Test => {
            runtime::run_tests(&typed_ast)?;
            Ok(vec![])
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileTarget {
    Wasm,
    Llvm,
    SpirV,
    Hlsl,
    Usf,
    Js,
    Rust,
    Interpret,
    Test,
}

/// Version of the KAIN language
pub const VERSION: &str = "0.1.0";
pub const LANGUAGE_NAME: &str = "KAIN";

