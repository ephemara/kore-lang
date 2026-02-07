//! Hybrid WASM/JS Code Generation
//!
//! This module generates a hybrid output where:
//! - Functions marked with @wasm compile to WebAssembly for performance
//! - Everything else (components, DOM code) compiles to JavaScript
//!
//! Usage:
//!   @wasm
//!   fn fibonacci(n: Int) -> Int:
//!       if n <= 1: return n
//!       return fibonacci(n-1) + fibonacci(n-2)
//!
//!   component App():
//!       render: <div>{fibonacci(40)}</div>

use crate::error::KainResult;
use crate::types::{TypedProgram, TypedItem, TypedFunction, TypedComponent};
use crate::codegen::{wasm, js};

/// Output from hybrid compilation
pub struct HybridOutput {
    pub wasm: Vec<u8>,
    pub js: String,
    pub wasm_exports: Vec<String>,
}

/// Check if a function has the @wasm attribute
fn has_wasm_attr(func: &TypedFunction) -> bool {
    func.ast.attributes.iter().any(|attr| attr.name == "wasm")
}

/// Check if a component has the @wasm attribute (rare but possible)
fn component_has_wasm_attr(comp: &TypedComponent) -> bool {
    comp.ast.attributes.iter().any(|attr| attr.name == "wasm")
}

/// Generate hybrid WASM + JS output from a typed program
pub fn generate(program: &TypedProgram) -> KainResult<HybridOutput> {
    let mut wasm_items = Vec::new();
    let mut js_items = Vec::new();
    let mut wasm_exports = Vec::new();
    
    // Split items by @wasm attribute
    for item in &program.items {
        match item {
            TypedItem::Function(f) => {
                if has_wasm_attr(f) {
                    wasm_exports.push(f.ast.name.clone());
                    wasm_items.push(item.clone());
                } else {
                    js_items.push(item.clone());
                }
            }
            TypedItem::Component(c) => {
                // Components almost always stay in JS for DOM access
                if component_has_wasm_attr(c) {
                    wasm_items.push(item.clone());
                } else {
                    js_items.push(item.clone());
                }
            }
            // Structs, enums go to both - WASM needs them for data, JS for interop
            TypedItem::Struct(_) | TypedItem::Enum(_) => {
                wasm_items.push(item.clone());
                js_items.push(item.clone());
            }
            // Everything else goes to JS
            _ => {
                js_items.push(item.clone());
            }
        }
    }
    
    // Compile WASM items
    let wasm_program = TypedProgram { items: wasm_items };
    let wasm_bytes = if !wasm_program.items.is_empty() {
        wasm::generate(&wasm_program)?
    } else {
        Vec::new()
    };
    
    // Compile JS items
    let js_program = TypedProgram { items: js_items };
    let mut js_code = js::generate(&js_program)?;
    
    // Generate WASM loader and bindings
    if !wasm_exports.is_empty() {
        let bindings = generate_wasm_bindings(&wasm_exports);
        js_code = format!("{}\n\n{}", bindings, js_code);
    }
    
    Ok(HybridOutput {
        wasm: wasm_bytes,
        js: js_code,
        wasm_exports,
    })
}

/// Generate JavaScript code to load WASM and create bindings
fn generate_wasm_bindings(exports: &[String]) -> String {
    let mut code = String::new();
    
    code.push_str("// Auto-generated WASM bindings\n");
    code.push_str("let __wasmInstance = null;\n");
    code.push_str("let __wasmReady = false;\n\n");
    
    // Async init function
    code.push_str("async function __initWasm() {\n");
    code.push_str("    if (__wasmReady) return;\n");
    code.push_str("    try {\n");
    code.push_str("        const response = await fetch('main.wasm');\n");
    code.push_str("        const buffer = await response.arrayBuffer();\n");
    code.push_str("        const result = await WebAssembly.instantiate(buffer, {\n");
    code.push_str("            env: {\n");
    code.push_str("                // Import JS functions into WASM if needed\n");
    code.push_str("            }\n");
    code.push_str("        });\n");
    code.push_str("        __wasmInstance = result.instance;\n");
    code.push_str("        __wasmReady = true;\n");
    code.push_str("        console.log('WASM loaded successfully');\n");
    code.push_str("    } catch (e) {\n");
    code.push_str("        console.error('Failed to load WASM:', e);\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
    
    // Generate wrapper functions for each export
    for name in exports {
        code.push_str(&format!("function {}(...args) {{\n", name));
        code.push_str("    if (!__wasmReady) {\n");
        code.push_str(&format!("        console.warn('WASM not ready, {} call delayed');\n", name));
        code.push_str("        return null;\n");
        code.push_str("    }\n");
        code.push_str(&format!("    return __wasmInstance.exports.{}(...args);\n", name));
        code.push_str("}\n\n");
    }
    
    // Auto-init on load
    code.push_str("// Auto-initialize WASM on script load\n");
    code.push_str("__initWasm();\n");
    
    code
}
