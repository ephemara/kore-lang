// ══════════════════════════════════════════════════════════════════════════════
// KAIN Hybrid Runtime - Auto-generated WASM/JS bridge
// ══════════════════════════════════════════════════════════════════════════════

let __wasmInstance = null;
let __wasmMemory = null;
let __wasmReady = false;
let __wasmReadyPromise = null;
const __domNodes = new Map(); // node_id -> DOM element
let __nextNodeId = 1;

// ─────────────────────────────────────────────────────────────────────────────
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

// ─────────────────────────────────────────────────────────────────────────────
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

// ─────────────────────────────────────────────────────────────────────────────
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

// ─────────────────────────────────────────────────────────────────────────────
// WASM Function Bindings
// ─────────────────────────────────────────────────────────────────────────────

function bubble_sort_step(...args) {
    if (!__wasmReady) {
        console.warn('[KAIN] WASM not ready, bubble_sort_step call failed');
        return false;
    }
    try {
        const result = __wasmInstance.exports.bubble_sort_step(...args);
        return result;
    } catch (e) {
        console.error('[KAIN] Error calling bubble_sort_step:', e);
        throw new Error(`WASM function 'bubble_sort_step' failed: ${e.message}`);
    }
}

async function bubble_sort_step_async(...args) {
    await __ensureWasm();
    return bubble_sort_step(...args);
}

function merge_values(...args) {
    if (!__wasmReady) {
        console.warn('[KAIN] WASM not ready, merge_values call failed');
        return 0;
    }
    try {
        const result = __wasmInstance.exports.merge_values(...args);
        return result;
    } catch (e) {
        console.error('[KAIN] Error calling merge_values:', e);
        throw new Error(`WASM function 'merge_values' failed: ${e.message}`);
    }
}

async function merge_values_async(...args) {
    await __ensureWasm();
    return merge_values(...args);
}

function binary_search_recursive(...args) {
    if (!__wasmReady) {
        console.warn('[KAIN] WASM not ready, binary_search_recursive call failed');
        return 0;
    }
    try {
        const result = __wasmInstance.exports.binary_search_recursive(...args);
        return result;
    } catch (e) {
        console.error('[KAIN] Error calling binary_search_recursive:', e);
        throw new Error(`WASM function 'binary_search_recursive' failed: ${e.message}`);
    }
}

async function binary_search_recursive_async(...args) {
    await __ensureWasm();
    return binary_search_recursive(...args);
}

function insertion_position(...args) {
    if (!__wasmReady) {
        console.warn('[KAIN] WASM not ready, insertion_position call failed');
        return 0;
    }
    try {
        const result = __wasmInstance.exports.insertion_position(...args);
        return result;
    } catch (e) {
        console.error('[KAIN] Error calling insertion_position:', e);
        throw new Error(`WASM function 'insertion_position' failed: ${e.message}`);
    }
}

async function insertion_position_async(...args) {
    await __ensureWasm();
    return insertion_position(...args);
}

function compare_and_swap(...args) {
    if (!__wasmReady) {
        console.warn('[KAIN] WASM not ready, compare_and_swap call failed');
        return 0;
    }
    try {
        const result = __wasmInstance.exports.compare_and_swap(...args);
        return result;
    } catch (e) {
        console.error('[KAIN] Error calling compare_and_swap:', e);
        throw new Error(`WASM function 'compare_and_swap' failed: ${e.message}`);
    }
}

async function compare_and_swap_async(...args) {
    await __ensureWasm();
    return compare_and_swap(...args);
}

// Auto-initialize WASM on script load
__wasmReadyPromise = __initWasm();



// Generated by KAIN compiler
// Target: JavaScript (ES6+)

function main() {
let a = 64  ;
let b = 34  ;
print("Compare 64 and 34:")  ;
print("Swap needed:", compare_and_swap(a, b))  ;
print("Merge (ascending):", merge_values(a, b, true))  ;
print("Merge (descending):", merge_values(a, b, false))  ;
}

