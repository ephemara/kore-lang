// output_bootstrap/src/compiler/mod.rs
// Module declarations for the KAIN compiler

pub mod lexer;
pub mod parser;
pub mod codegen;
pub mod codegen_rust;
pub mod codegen_llvm;

// Re-exports for convenience
pub use lexer::{Token, TokenKind, Lexer};
pub use parser::{Program, Parser};
pub use codegen_rust::RustGen;
pub use codegen_llvm::LLVMGen;
