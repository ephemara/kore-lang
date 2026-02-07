# KAIN JavaScript/WebAssembly Target

This directory contains examples and build artifacts demonstrating KAIN's hybrid WASM/JS compilation.

## Overview

KAIN supports a hybrid compilation model that combines:
- **WebAssembly** for performance-critical computations
- **JavaScript** for UI components and DOM manipulation

This approach delivers near-native performance where it matters while maintaining full DOM access for interactive applications.

## Quick Start

```bash
# Compile a KAIN file to hybrid WASM+JS
kain your_file.kn -t hybrid -o output.js

# The compiler generates:
# - output.js (runtime + bindings + JS code)
# - output.wasm (WASM module, if @wasm functions present)
```

## Usage

Mark functions with `@wasm` to compile them to WebAssembly:

```kain
@wasm
fn fibonacci(n: Int) -> Int:
    if n <= 1:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)

component App():
    render:
        <div>Result: {fibonacci(35)}</div>
```

Functions without the `@wasm` attribute compile to JavaScript.

## Examples

| File | Description |
|------|-------------|
| `examples/math.kn` | Mathematical functions (fibonacci, factorial, gcd, primality) |
| `examples/sorting.kn` | Sorting algorithm primitives |
| `examples/calculator.kn` | Hybrid UI with WASM-backed financial calculations |
| `examples/physics.kn` | Game physics (gravity, collision, interpolation) |

## Build Artifacts

Pre-compiled outputs are available in the `dist/` directory:

| File | Size | Description |
|------|------|-------------|
| `math.js` | 10KB | Mathematical computing runtime |
| `sorting.js` | 11KB | Sorting algorithm runtime |
| `calculator.js` | 12KB | Financial calculator with UI |
| `physics.js` | 13KB | Game physics engine |

## Generated Runtime

The hybrid compiler generates a complete runtime including:

- **Memory helpers**: String encoding/decoding, array marshaling
- **Host imports**: Console output, DOM manipulation, timing
- **WASM loader**: Async initialization with graceful fallback
- **Type-aware bindings**: Automatic marshaling for each exported function

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     KAIN Source (.kn)                       │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Hybrid Compiler                          │
│  ┌─────────────────────┐    ┌─────────────────────────────┐ │
│  │  @wasm functions    │    │  Components + JS functions  │ │
│  │         │           │    │             │               │ │
│  │         ▼           │    │             ▼               │ │
│  │   WASM Codegen      │    │       JS Codegen            │ │
│  └─────────────────────┘    └─────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
┌─────────────────────────┐     ┌─────────────────────────────┐
│     output.wasm         │     │         output.js           │
│  (compiled functions)   │     │  (runtime + bindings + UI)  │
└─────────────────────────┘     └─────────────────────────────┘
```

## Integration

Include the generated JavaScript in your HTML:

```html
<script src="dist/math.js"></script>
<script>
    // WASM functions are available globally after initialization
    async function run() {
        await __ensureWasm();
        console.log('Fibonacci(35):', fibonacci(35));
    }
    run();
</script>
```

## Performance

By compiling numerical code to WebAssembly, KAIN achieves:
- Near-native execution speed for compute-heavy operations
- Zero-cost abstraction between WASM and JS boundaries
- Automatic memory management and type marshaling

## Requirements

- KAIN compiler v0.1.0 or later
- Modern browser with WebAssembly support
- Node.js 16+ (for server-side execution)
