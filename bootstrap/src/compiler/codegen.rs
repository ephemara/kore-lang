// ============================================================================
// KAIN Bootstrap Compiler - LLVM IR Code Generator (Stub)
// ============================================================================
// This is a stub for the LLVM IR codegen. The Rust backend is primary.
// Full implementation will come later.
// ============================================================================

use crate::compiler::parser::{Program, Item, Stmt, Expr};

pub struct CodeGen {
    output: String,
    indent: usize,
    local_counter: usize,
    string_counter: usize,
    strings: Vec<String>,
}

impl CodeGen {
    pub fn new() -> CodeGen {
        CodeGen {
            output: String::new(),
            indent: 0,
            local_counter: 0,
            string_counter: 0,
            strings: Vec::new(),
        }
    }
    
    pub fn gen_program(&mut self, _program: Program) -> String {
        // Stub - not implemented yet
        "; KAIN LLVM IR Output (stub)\n; Use --target rust instead\n".to_string()
    }
}
