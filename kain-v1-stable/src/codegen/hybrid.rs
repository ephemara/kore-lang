//! Hybrid WASM/JS Code Generation
//!
//! Generates a hybrid output where:
//! - Functions marked with @wasm compile to WebAssembly for performance
//! - Everything else (components, DOM code) compiles to JavaScript
//!
//! The generated JS includes:
//! - WASM loader with memory management
//! - Host import implementations (print, DOM, etc.)
//! - Type-aware bindings with automatic marshaling
//! - Error handling across the JS/WASM boundary

use crate::error::KainResult;
use crate::types::{TypedProgram, TypedItem, TypedFunction, TypedComponent, ResolvedType};
use crate::codegen::{wasm, js};

/// Output from hybrid compilation
pub struct HybridOutput {
    pub wasm: Vec<u8>,
    pub js: String,
    pub wasm_exports: Vec<WasmExport>,
}

/// Metadata about an exported WASM function for binding generation
#[derive(Clone, Debug)]
pub struct WasmExport {
    pub name: String,
    pub params: Vec<(String, ResolvedType)>,
    pub return_type: Option<ResolvedType>,
}

/// Check if a function has the @wasm attribute
fn has_wasm_attr(func: &TypedFunction) -> bool {
    func.ast.attributes.iter().any(|attr| attr.name == "wasm")
}

/// Check if a component has the @wasm attribute
fn component_has_wasm_attr(comp: &TypedComponent) -> bool {
    comp.ast.attributes.iter().any(|attr| attr.name == "wasm")
}

/// Extract export metadata from a function
fn extract_export(func: &TypedFunction) -> WasmExport {
    let params: Vec<(String, ResolvedType)> = func.ast.params.iter()
        .map(|p| (p.name.clone(), func.resolved_type.clone()))
        .collect();
    
    let return_type = match &func.resolved_type {
        ResolvedType::Function { ret, .. } => Some((**ret).clone()),
        _ => None,
    };
    
    WasmExport {
        name: func.ast.name.clone(),
        params,
        return_type,
    }
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
                    wasm_exports.push(extract_export(f));
                    wasm_items.push(item.clone());
                } else {
                    js_items.push(item.clone());
                }
            }
            TypedItem::Component(c) => {
                if component_has_wasm_attr(c) {
                    wasm_items.push(item.clone());
                } else {
                    js_items.push(item.clone());
                }
            }
            // Structs & enums go to both for interop
            TypedItem::Struct(_) | TypedItem::Enum(_) => {
                wasm_items.push(item.clone());
                js_items.push(item.clone());
            }
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
    
    // Generate complete runtime + bindings
    if !wasm_exports.is_empty() {
        let runtime = generate_hybrid_runtime(&wasm_exports);
        js_code = format!("{}\n\n{}", runtime, js_code);
    }
    
    Ok(HybridOutput {
        wasm: wasm_bytes,
        js: js_code,
        wasm_exports,
    })
}

/// Generate the complete hybrid runtime with host imports and bindings
fn generate_hybrid_runtime(exports: &[WasmExport]) -> String {
    let mut code = String::new();
    
    // Header
    code.push_str("// ══════════════════════════════════════════════════════════════════════════════\n");
    code.push_str("// KAIN Hybrid Runtime - Auto-generated WASM/JS bridge\n");
    code.push_str("// ══════════════════════════════════════════════════════════════════════════════\n\n");
    
    // Core state
    code.push_str("let __wasmInstance = null;\n");
    code.push_str("let __wasmMemory = null;\n");
    code.push_str("let __wasmReady = false;\n");
    code.push_str("let __wasmReadyPromise = null;\n");
    code.push_str("const __domNodes = new Map(); // node_id -> DOM element\n");
    code.push_str("let __nextNodeId = 1;\n\n");
    
    // Memory helpers
    code.push_str(&generate_memory_helpers());
    
    // Host imports
    code.push_str(&generate_host_imports());
    
    // WASM loader
    code.push_str(&generate_wasm_loader());
    
    // Function bindings with marshaling
    code.push_str(&generate_function_bindings(exports));
    
    // Auto-init
    code.push_str("// Auto-initialize WASM on script load\n");
    code.push_str("__wasmReadyPromise = __initWasm();\n\n");
    
    code
}

/// Generate memory read/write helpers
fn generate_memory_helpers() -> String {
    r#"// ─────────────────────────────────────────────────────────────────────────────
// Memory Helpers
// ─────────────────────────────────────────────────────────────────────────────

function __readString(ptr) {
    if (!__wasmMemory || ptr === 0) return '';
    const view = new DataView(__wasmMemory.buffer);
    const len = view.getInt32(ptr, true);
    const bytes = new Uint8Array(__wasmMemory.buffer, ptr + 4, len);
    return new TextDecoder().decode(bytes);
}

function __writeString(str) {
    if (!__wasmMemory) return 0;
    const encoded = new TextEncoder().encode(str);
    const len = encoded.length;
    // Allocate: 4 bytes for length + string bytes
    const ptr = __wasmAlloc(4 + len);
    const view = new DataView(__wasmMemory.buffer);
    view.setInt32(ptr, len, true);
    new Uint8Array(__wasmMemory.buffer).set(encoded, ptr + 4);
    return ptr;
}

function __readArray(ptr, elemSize, readElem) {
    if (!__wasmMemory || ptr === 0) return [];
    const view = new DataView(__wasmMemory.buffer);
    const len = view.getInt32(ptr, true);
    const result = [];
    for (let i = 0; i < len; i++) {
        result.push(readElem(view, ptr + 4 + i * elemSize));
    }
    return result;
}

function __wasmAlloc(size) {
    // Use the WASM heap pointer global if exported, otherwise bump allocate
    if (__wasmInstance && __wasmInstance.exports.__alloc) {
        return __wasmInstance.exports.__alloc(size);
    }
    // Fallback: read/update heap pointer from WASM globals
    // This assumes heap_ptr is at a known location - we'll use a simple bump
    console.warn('Using fallback allocator - no __alloc export');
    return 0;
}

"#.to_string()
}

/// Generate host import implementations
fn generate_host_imports() -> String {
    r#"// ─────────────────────────────────────────────────────────────────────────────
// Host Import Implementations
// ─────────────────────────────────────────────────────────────────────────────

const __hostImports = {
    host: {
        // Print integer
        print_i64(val) {
            console.log(Number(val));
        },
        
        // Print float
        print_f64(val) {
            console.log(val);
        },
        
        // Print string from WASM memory
        print_str(ptr, len) {
            if (!__wasmMemory) return;
            const bytes = new Uint8Array(__wasmMemory.buffer, ptr, len);
            console.log(new TextDecoder().decode(bytes));
        },
        
        // Print boolean
        print_bool(val) {
            console.log(val !== 0);
        },
        
        // Read integer from prompt
        read_i64() {
            const input = prompt('Enter a number:');
            return BigInt(parseInt(input) || 0);
        },
        
        // Integer to string - returns pointer to new string in WASM memory
        int_to_str(val) {
            return __writeString(String(Number(val)));
        },
        
        // Concatenate two strings - returns pointer to new string
        str_concat(ptr1, ptr2) {
            const s1 = __readString(ptr1);
            const s2 = __readString(ptr2);
            return __writeString(s1 + s2);
        },
        
        // Get current time in milliseconds
        time_now() {
            return BigInt(Date.now());
        },
        
        // DOM: Create element
        dom_create(tagPtr, tagLen) {
            const bytes = new Uint8Array(__wasmMemory.buffer, tagPtr, tagLen);
            const tag = new TextDecoder().decode(bytes);
            const el = document.createElement(tag);
            const id = __nextNodeId++;
            __domNodes.set(id, el);
            return id;
        },
        
        // DOM: Append child
        dom_append(parentId, childId) {
            const parent = __domNodes.get(parentId);
            const child = __domNodes.get(childId);
            if (parent && child) {
                parent.appendChild(child);
            }
        },
        
        // DOM: Set attribute
        dom_attr(nodeId, keyPtr, keyLen, valPtr, valLen) {
            const node = __domNodes.get(nodeId);
            if (!node) return;
            const key = new TextDecoder().decode(new Uint8Array(__wasmMemory.buffer, keyPtr, keyLen));
            const val = new TextDecoder().decode(new Uint8Array(__wasmMemory.buffer, valPtr, valLen));
            node.setAttribute(key, val);
        },
        
        // DOM: Create text node
        dom_text(textPtr, textLen) {
            const text = new TextDecoder().decode(new Uint8Array(__wasmMemory.buffer, textPtr, textLen));
            const node = document.createTextNode(text);
            const id = __nextNodeId++;
            __domNodes.set(id, node);
            return id;
        },
    }
};

"#.to_string()
}

/// Generate the WASM loader function
fn generate_wasm_loader() -> String {
    r#"// ─────────────────────────────────────────────────────────────────────────────
// WASM Loader
// ─────────────────────────────────────────────────────────────────────────────

async function __initWasm() {
    if (__wasmReady) return true;
    
    try {
        const response = await fetch('main.wasm');
        if (!response.ok) {
            console.warn('[KAIN] No WASM module found, running in JS-only mode');
            return false;
        }
        
        const buffer = await response.arrayBuffer();
        const result = await WebAssembly.instantiate(buffer, __hostImports);
        
        __wasmInstance = result.instance;
        __wasmMemory = __wasmInstance.exports.memory;
        __wasmReady = true;
        
        console.log('[KAIN] WASM module loaded successfully');
        return true;
    } catch (e) {
        console.error('[KAIN] Failed to load WASM:', e);
        return false;
    }
}

// Ensure WASM is ready before calling WASM functions
async function __ensureWasm() {
    if (__wasmReady) return true;
    if (__wasmReadyPromise) {
        return await __wasmReadyPromise;
    }
    return false;
}

"#.to_string()
}

/// Generate function bindings with type-aware marshaling
fn generate_function_bindings(exports: &[WasmExport]) -> String {
    let mut code = String::new();
    
    code.push_str("// ─────────────────────────────────────────────────────────────────────────────\n");
    code.push_str("// WASM Function Bindings\n");
    code.push_str("// ─────────────────────────────────────────────────────────────────────────────\n\n");
    
    for export in exports {
        // Create sync wrapper that handles marshaling
        code.push_str(&format!("function {}(...args) {{\n", export.name));
        code.push_str("    if (!__wasmReady) {\n");
        code.push_str(&format!("        console.warn('[KAIN] WASM not ready, {} call failed');\n", export.name));
        
        // Return appropriate default based on return type
        match &export.return_type {
            Some(ResolvedType::String) => code.push_str("        return '';\n"),
            Some(ResolvedType::Bool) => code.push_str("        return false;\n"),
            Some(ResolvedType::Array(_, _)) => code.push_str("        return [];\n"),
            _ => code.push_str("        return 0;\n"),
        }
        
        code.push_str("    }\n");
        code.push_str("    try {\n");
        code.push_str(&format!("        const result = __wasmInstance.exports.{}(...args);\n", export.name));
        
        // Unmarshal result based on type
        match &export.return_type {
            Some(ResolvedType::String) => {
                code.push_str("        return __readString(result);\n");
            }
            Some(ResolvedType::Array(_, _)) => {
                code.push_str("        return __readArray(result, 8, (v, p) => Number(v.getBigInt64(p, true)));\n");
            }
            _ => {
                code.push_str("        return result;\n");
            }
        }
        
        code.push_str("    } catch (e) {\n");
        code.push_str(&format!("        console.error('[KAIN] Error calling {}:', e);\n", export.name));
        code.push_str("        throw new Error(`WASM function '");
        code.push_str(&export.name);
        code.push_str("' failed: ${e.message}`);\n");
        code.push_str("    }\n");
        code.push_str("}\n\n");
        
        // Also create async version for when WASM isn't ready yet
        code.push_str(&format!("async function {}_async(...args) {{\n", export.name));
        code.push_str("    await __ensureWasm();\n");
        code.push_str(&format!("    return {}(...args);\n", export.name));
        code.push_str("}\n\n");
    }
    
    code
}
