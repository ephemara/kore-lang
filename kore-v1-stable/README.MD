HUGE UPDATE:2/5/25- HLSL shader pipeline is now fully functional. See /shaders for the full pipeline. 

# KORE V1 - Production Compiler

> **Production-Ready Release**: This is the stable, feature-complete V1 compiler with Actor runtime, UE5 shader pipeline, Python FFI, and multiple compilation targets. While the V2 self-hosting compiler (in the root directory) is under active development, V1 is battle-tested and ready for production use.

## Philosophy

KORE combines the best ideas from multiple paradigms:

1. **Rust's Safety** - Ownership, borrowing, no null, no data races
2. **Python's Syntax** - Significant whitespace, minimal ceremony  
3. **Lisp's Power** - Code as data, hygienic macros, DSL-friendly
4. **Zig's Comptime** - Compile-time execution, no separate macro language
5. **Effect Tracking** - Side effects in the type system
6. **Actor Concurrency** - Erlang-style message passing built-in
7. **Universal Targets** - WASM, LLVM native, SPIR-V shaders, **Rust transpilation**

---

## Quick Start

```bash
cargo install kore-lang
```

That's it. You now have the `kore` command available system-wide.

### Build from Source

If you prefer building from source:

```bash
cd kore-v1-stable
cargo build --release
```

**Binary**: `target/release/kore` (or `kore.exe` on Windows)

---

## CLI Reference

### Main Compiler (`kore`)

```bash
# Run in interpreter (instant execution)
./target/release/kore example.kr --target run

# Compile to WebAssembly
./target/release/kore example.kr --target wasm -o output.wasm

# Compile to LLVM IR (then link with clang)
./target/release/kore example.kr --target llvm -o output.ll

# Compile to SPIR-V (GPU shader bytecode)
./target/release/kore shader.kr --target spirv -o shader.spv

# Transpile to Rust source code
./target/release/kore example.kr --target rust -o output.rs

# Run tests
./target/release/kore test_file.kr --target test

# Watch mode (auto-recompile on file change)
./target/release/kore example.kr --target wasm --watch
```

### Compilation Targets

| Target | Flag(s) | Output |
|--------|---------|--------|
| WASM | `-t wasm`, `-t w` | `.wasm` |
| LLVM/Native | `-t llvm`, `-t native`, `-t n` | `.ll` |
| SPIR-V | `-t spirv`, `-t gpu`, `-t s` | `.spv` |
| **HLSL** | `-t hlsl` | `.hlsl` |
| Rust | `-t rust`, `-t rs` | `.rs` |
| Interpret | `-t run`, `-t interpret`, `-t i` | stdout |
| Test | `-t test`, `-t t` | stdout |

### CLI Options

| Flag | Description |
|------|-------------|
| `-o, --output <file>` | Output file path |
| `-t, --target <target>` | Compilation target (default: wasm) |
| `-r, --run` | Run immediately after compilation |
| `-w, --watch` | Watch for file changes and recompile |
| `--emit-ast` | Dump parsed AST for debugging |
| `--emit-typed` | Dump type-annotated AST |
| `-v, --verbose` | Verbose output |
| `--dry-run` | Print planned actions without executing |
| `--strict` | Treat warnings as errors |

### Subcommands (Main Compiler)

```bash
kore init [path] [--name NAME]    # Initialize a new project
kore lsp                          # Start Language Server Protocol
kore build <file>                 # Compile to WASM (default)
kore run <file>                   # Execute via interpreter
```

---

## Shader Pipeline (KORE → HLSL)

KORE includes a **direct HLSL codegen backend** - no intermediate SPIR-V or external tools required.

### Direct HLSL Compilation (Recommended)

```bash
# One command - KORE shader to production HLSL
kore shader.kr --target hlsl -o shader.hlsl
```

The generated HLSL includes:
- Proper `cbuffer` for uniforms with register bindings
- `Texture2D` and `SamplerState` declarations
- Correct `VSInput`/`PSOutput` structs with semantics
- Full PBR math, tonemapping, post-processing

### Alternative: SPIR-V Pipeline

For cross-platform shader compilation, you can still use the SPIR-V path:

```bash
# Compile to SPIR-V
kore shader.kr --target spirv -o shader.spv

# Convert with naga (cargo install naga-cli)
naga shader.spv shader.hlsl   # DirectX
naga shader.spv shader.wgsl   # WebGPU
naga shader.spv shader.glsl   # OpenGL
naga shader.spv shader.metal  # Apple Metal
```

---

## Unreal Engine 5 Shader Integration

KORE includes an automated `ue5-shader` target that compiles shaders directly for Unreal Engine plugins.

### One-Command UE5 Pipeline

```bash
# Compile KORE shader and generate UE5-ready files
./target/release/kore shader.kr --target ue5-shader

# Deploy directly to a UE5 plugin
./target/release/kore shader.kr --target ue5-shader --plugin MyRenderPlugin
```

### What Gets Generated

| File | Description |
|------|-------------|
| `shader.spv` | SPIR-V bytecode (validated with spirv-val if available) |
| `shader.hlsl` | HLSL source (transpiled via naga) |
| `shader.usf` | Unreal Shader File wrapper (`#include "shader.hlsl"`) |

### Pipeline Steps

1. **Compile** - KORE source → SPIR-V binary
2. **Validate** - Optional `spirv-val` check
3. **Transpile** - SPIR-V → HLSL via `naga`
4. **Wrap** - Generate `.usf` include wrapper
5. **Deploy** - Copy to plugin's `Shaders/` directory

### Using in UE5

Once deployed, reference your shader in C++:
```cpp
// In your plugin's module
IMPLEMENT_GLOBAL_SHADER(FMyEffectPS, "/Plugin/MyRenderPlugin/Shaders/my_effect.usf", "MainPS", SF_Pixel);
```

Or in a material:
```
// Custom node
#include "/Plugin/MyRenderPlugin/Shaders/my_effect.usf"
```

---

## Rust Transpiler

KORE can be transpiled to valid Rust source code:

```bash
# Transpile to Rust
./target/release/kore logic.kr --target rust -o logic.rs

# Compile with rustc
rustc logic.rs -o logic.exe

# Or integrate into a Cargo project
```

The generated Rust includes:
- Standard derive macros (`Debug`, `Clone`)
- Proper type mappings (`Int` → `i64`, `String` → `String`, etc.)
- `println!` macro conversion
- Struct and enum definitions

---

## Directory Structure

```
kore-v1-stable/
├── Cargo.toml              # Compiler manifest
├── README.md               # This file
├── commands.md             # CLI reference
│
├── src/                    # Rust compiler source
│   ├── lib.rs              # Library entry, compile() function
│   ├── main.rs             # CLI entry point
│   ├── lexer.rs            # Tokenizer
│   ├── parser.rs           # Parser (82KB)
│   ├── ast.rs              # AST types
│   ├── types.rs            # Type system
│   ├── runtime.rs          # Interpreter + Actor system (135KB)
│   ├── monomorphize.rs     # Generic instantiation (55KB)
│   └── codegen/            # Code generation backends
│       ├── wasm.rs         # WebAssembly (95KB)
│       ├── llvm.rs         # LLVM IR (66KB)
│       ├── spirv.rs        # SPIR-V shaders (14KB)
│       └── rust.rs         # Rust transpiler (28KB)
│
├── stdlib/                 # KORE standard library
│   ├── runtime.kr          # Runtime utilities
│   ├── wasm.kr             # WASM utilities
│   ├── spirv.kr            # SPIR-V utilities
│   └── ...                 # 12 modules total
│
├── shaders/                # GPU shader examples
│   ├── pbr_material.kr     # PBR shader
│   ├── post_processing.kr  # Post-processing effects
│   └── ...                 # 5 shaders
│
├── examples/               # General KORE examples
├── bootstrap/              # Self-hosting compiler (in KORE)
├── tests/kore/             # Test files
└── runtime/                # C FFI runtime
```

---

## Codegen Backends

### WASM Backend (`codegen/wasm.rs` - 95KB)

Full WebAssembly support:
- Struct memory layout computation
- Component system for UI
- Enum discriminant handling
- Lambda/closure collection and function table
- String pooling in data segment
- Bump allocator for heap memory

### LLVM Backend (`codegen/llvm.rs` - 66KB)

Native compilation via LLVM IR:
- Struct and actor compilation
- Reference counting helpers
- Scope-based cleanup (RAII-style)
- External C runtime linkage
- Debug info generation

### SPIR-V Backend (`codegen/spirv.rs` - 14KB)

GPU shader compilation:
- Vertex/Fragment/Compute shader stages
- Vector types: `vec2`, `vec3`, `vec4`
- Matrix types: `mat4`
- Input/Output location decorations
- BuiltIn position handling

### Rust Backend (`bootstrap/src/compiler/codegen_rust.rs` - 23KB)

Transpilation to Rust source:
- Full AST transformation
- Type mapping (KORE → Rust)
- Operator translation
- Pattern matching support
- Generated `Cargo.toml` scaffolding

---

## C Runtime

The LLVM backend links against a minimal C runtime (`src/runtime/c/kore_runtime.c`):

| Function | Description |
|----------|-------------|
| `kore_print_*` | Print functions for all types |
| `kore_str_*` | String operations (concat, len, eq, substring) |
| `kore_array_*` | Dynamic array (new, push, pop, get, set, len) |
| `kore_map_*` | Hash map (new, get, set, contains) |
| `kore_file_*` | File I/O (read, write) |
| `kore_alloc/free` | Memory management |
| `kore_panic` | Runtime panic |
| `args()` | Command-line arguments |

### Compiling with Runtime

```bash
# Step 1: Compile KORE to LLVM IR
./target/release/kore program.kr --target llvm -o program.ll

# Step 2: Compile C runtime
clang -c src/runtime/c/kore_runtime.c -o kore_runtime.o

# Step 3: Link everything
clang program.ll kore_runtime.o -o program.exe
```

---

## Self-Hosted Compiler

The `src/compiler/` directory contains the **self-hosted KORE compiler** written in KORE itself (Project Ouroboros).

### Components

| File | Lines | Description |
|------|-------|-------------|
| `main.kr` | 284 | Compiler driver, CLI parsing |
| `lexer.kr` | ~400 | Tokenizer |
| `parser.kr` | ~800 | Recursive descent parser |
| `codegen.kr` | ~800 | LLVM IR generation |
| `codegen_rust.kr` | 684 | Rust transpilation |

### Running Self-Hosted

```bash
# The self-hosted compiler runs through the interpreter
./target/release/kore src/compiler/main.kr --target run -- input.kr --target rust
```

---

## KORE Utilities (Written in KORE)

The `src/other/` directory contains utilities written in KORE itself:

| File | Lines | Description |
|------|-------|-------------|
| `repl.kr` | 432 | Interactive REPL with history, commands |
| `formatter.kr` | 751 | Code formatter (`kore fmt`) |
| `lsp.kr` | ~300 | Language Server Protocol |
| `test_runner.kr` | ~200 | Test framework |
| `suggestions.kr` | ~150 | Error message suggestions |
| `import_resolver.kr` | ~200 | Module import resolution |
| `packager.kr` | ~250 | Package management |

---

## Runtime Features

### Interpreter (`runtime.rs` - 135KB)

- Full expression evaluation
- Pattern matching with destructuring
- Async/await support with futures
- Import resolution for modules
- Hot reload capability

### Actor System

Erlang-style concurrency:
```kore
actor Counter:
    var count: Int = 0
    
    on Increment(n: Int):
        count = count + n
    
    on GetCount -> Int:
        return count
```

### Python Interop

Via `pyo3`:
```kore
let result = py_call("math.sqrt", [16.0])  # Returns 4.0
```

### Standard Library

Built-in functions:
- I/O: `print`, `println`, `read_line`, `read_file`, `write_file`
- Collections: `push`, `pop`, `len`, `map`, `filter`, `reduce`
- Math: `abs`, `min`, `max`, `sqrt`, `pow`
- String: `split`, `join`, `trim`, `replace`, `substring`
- JSON: `json_parse`, `json_stringify`
- HTTP: `http_get`, `http_post`

---

## Build Configuration

### Default Build (No LLVM linking required)

```bash
cargo build --release
```

### With LLVM Backend (requires LLVM 17 installed)

```bash
cargo build --release --features llvm
```

### Dependencies

| Crate | Purpose |
|-------|---------|
| `logos` | Lexer generator |
| `chumsky` | Parser combinator |
| `clap` | CLI argument parsing |
| `ariadne` | Pretty error reporting |
| `inkwell` | LLVM bindings (optional) |
| `walrus` | WASM manipulation |
| `rspirv` | SPIR-V generation |
| `tokio` | Async runtime |
| `flume` | Actor message channels |
| `pyo3` | Python FFI |
| `tower-lsp` | Language Server Protocol |
| `notify` | File watching for hot reload |
| `reqwest` | HTTP client for stdlib |

---

## Example Code

### Basic Function

```kore
fn factorial(n: Int) -> Int with Pure:
    match n:
        0 => 1
        _ => n * factorial(n - 1)

fn main():
    let result = factorial(5)
    println("5! = {result}")
```

### Shader

```kore
shader vertex MainVS:
    in position: vec3
    in color: vec4
    out out_color: vec4
    
    fn main():
        gl_Position = vec4(position, 1.0)
        out_color = color

shader fragment MainPS:
    in in_color: vec4
    out frag_color: vec4
    
    fn main():
        frag_color = in_color
```

### Actor

```kore
actor ChatRoom:
    var messages: Array<String> = []
    var users: Array<String> = []
    
    on Join(name: String):
        push(users, name)
        broadcast("{name} joined")
    
    on Message(from: String, text: String):
        push(messages, "{from}: {text}")
        broadcast("{from}: {text}")
```

---

## Development Status

| Component | Status | Notes |
|-----------|--------|-------|
| Lexer | Production | Full token coverage |
| Parser | Production | 82KB of parsing logic |
| Type Checker | Production | Effect inference included |
| Interpreter | Production | Primary development target |
| WASM Codegen | Production | Full feature support |
| LLVM Codegen | Stable | Requires feature flag |
| SPIR-V Codegen | Stable | Shader pipeline functional |
| **Rust Transpiler** | Stable | Bootstrap compiler |
| Self-hosted Compiler | See V2 | Project Ouroboros (root dir) |
| LSP | Working | Basic features |
| Actor System | Production | Channel-based |
| Python FFI | Working | Via pyo3 |
| REPL | Planned | Framework exists |
| Formatter | Planned | Framework exists |

---

## Troubleshooting

### SPIR-V Validation Errors

If `naga` fails with validation errors:

| Error | Cause | Fix |
|-------|-------|-----|
| `Type isn't compatible with address space` | Wrong storage class | Check uniform handling |
| `Vertex shaders must return @builtin(position)` | Missing BuiltIn decoration | Ensure Vec4 output |
| `Global variable is invalid` | Missing Block decoration | Wrap uniform in struct |

### Rust Transpiler Borrow Errors

Generated Rust may need manual fixes:
1. Add `mut` or `&` as needed
2. Check ownership patterns
3. Report issues for codegen improvement

### Build Performance

```bash
# Fast syntax check (no binary output)
cargo check

# Build only the binary
cargo build --release --bin kore
```

---

## License

MIT