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

## Two Compilers

This folder contains **two distinct compilers** at different stages of development:

| Compiler | Location | LLVM Version | Backends | Status |
|----------|----------|--------------|----------|--------|
| **Main V1 Compiler** | `./` (root) | inkwell 0.4 / LLVM 17 | WASM, LLVM, SPIR-V, Interpret | Production |
| **Bootstrap Compiler** | `./bootstrap/` | inkwell 0.7.1 / LLVM 21 | LLVM, **Rust** | Stable |

The bootstrap compiler includes the **Rust transpiler** - KORE code can be compiled to valid Rust source.

---

## Quick Start

### Build the Main V1 Compiler

```bash
cd kore-v1-stable
cargo build --release
```

**Binary**: `target/release/kore` (or `kore.exe` on Windows)

### Build the Bootstrap Compiler

```bash
cd kore-v1-stable/bootstrap
cargo build --release
```

**Binary**: `bootstrap/target/release/korec` (or `korec.exe` on Windows)

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

# Run tests
./target/release/kore test_file.kr --target test

# Watch mode (auto-recompile on file change)
./target/release/kore example.kr --target wasm --watch
```

### Bootstrap Compiler (`korec`)

```bash
# Compile to LLVM IR (default)
./bootstrap/target/release/korec example.kr -o output.ll

# Transpile to Rust
./bootstrap/target/release/korec example.kr --target rust -o output.rs

# Specify target explicitly
./bootstrap/target/release/korec example.kr --target llvm -o output.ll
```

### Compilation Targets

| Target | Main Compiler | Bootstrap | Flag(s) | Output |
|--------|:-------------:|:---------:|---------|--------|
| WASM | Yes | No | `-t wasm`, `-t w` | `.wasm` |
| LLVM/Native | Yes | Yes | `-t llvm`, `-t native`, `-t n` | `.ll` |
| SPIR-V | Yes | No | `-t spirv`, `-t gpu`, `-t shader`, `-t s` | `.spv` |
| Interpret | Yes | No | `-t run`, `-t interpret`, `-t i` | stdout |
| Test | Yes | No | `-t test`, `-t t` | stdout |
| **Rust** | No | **Yes** | `--target rust` | `.rs` |

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

KORE can compile shaders to SPIR-V, which can then be converted to HLSL/GLSL/WGSL using `naga`.

### Full Pipeline

```bash
# Step 1: Compile KORE shader to SPIR-V
./target/release/kore shader.kr --target spirv -o shader.spv

# Step 2: Convert SPIR-V to HLSL (using naga)
naga shader.spv shader.hlsl

# Step 3: (Optional) Convert to other formats
naga shader.spv shader.wgsl    # WebGPU
naga shader.spv shader.glsl    # OpenGL
naga shader.spv shader.metal   # Apple Metal
```

### Install Naga

```bash
cargo install naga-cli
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

## Rust Transpiler Pipeline

The bootstrap compiler can transpile KORE to Rust, giving access to the entire Rust ecosystem.

```bash
# Step 1: Transpile to Rust
./bootstrap/target/release/korec logic.kr --target rust -o logic.rs

# Step 2: Compile with rustc
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
├── Cargo.toml                  # Main compiler manifest (inkwell 0.4)
├── README.md                   # This file
├── commands.md                 # CLI reference
│
├── src/                        # Main compiler source (Rust)
│   ├── lib.rs                  # Library entry, compile() function
│   ├── main.rs                 # CLI entry point
│   │
│   ├── lexer.rs                # Tokenizer (logos)
│   ├── parser.rs               # Parser combinator (82KB, chumsky)
│   ├── ast.rs                  # Abstract Syntax Tree types
│   ├── types.rs                # Type system definitions
│   ├── span.rs                 # Source location tracking
│   │
│   ├── runtime.rs              # Interpreter + Actor system (135KB)
│   ├── stdlib.rs               # Standard library bindings
│   ├── effects.rs              # Effect system
│   ├── comptime.rs             # Compile-time evaluation
│   ├── monomorphize.rs         # Generic instantiation (55KB)
│   │
│   ├── codegen/                # Code generation backends
│   │   ├── mod.rs              # Module exports
│   │   ├── llvm.rs             # LLVM IR generator (66KB)
│   │   ├── wasm.rs             # WebAssembly codegen (95KB)
│   │   └── spirv.rs            # SPIR-V shader codegen (14KB)
│   │
│   ├── compiler/               # Self-hosted compiler (written in KORE!)
│   │   ├── main.kr             # Compiler driver (284 lines)
│   │   ├── lexer.kr            # Lexer in KORE
│   │   ├── parser.kr           # Parser in KORE
│   │   ├── codegen.kr          # LLVM codegen in KORE
│   │   ├── codegen_rust.kr     # Rust transpiler in KORE (684 lines)
│   │   ├── bootstrap.kr        # Bootstrap entry
│   │   ├── compile_bootstrap.kr
│   │   └── full_bootstrap.kr
│   │
│   ├── other/                  # Additional KORE-written utilities
│   │   ├── repl.kr             # Interactive REPL (432 lines)
│   │   ├── formatter.kr        # Code formatter (751 lines)
│   │   ├── lsp.kr              # Language Server
│   │   ├── runtime.kr          # Runtime utilities
│   │   ├── spirv.kr            # SPIR-V utilities
│   │   ├── wasm.kr             # WASM utilities
│   │   ├── comptime.kr         # Comptime evaluation
│   │   ├── monomorphize.kr     # Monomorphization
│   │   ├── import_resolver.kr  # Import resolution
│   │   ├── packager.kr         # Package management
│   │   ├── suggestions.kr      # Error suggestions
│   │   └── test_runner.kr      # Test framework
│   │
│   ├── runtime/c/              # C runtime for LLVM backend
│   │   ├── kore_runtime.c      # Runtime implementation (334 lines)
│   │   └── kore_runtime.obj    # Compiled object file
│   │
│   ├── lsp.rs                  # Language Server Protocol (20KB)
│   ├── diagnostics.rs          # Pretty error formatting
│   ├── error.rs                # Error types
│   └── packager.rs             # Project scaffolding
│
├── bootstrap/                  # Bootstrap compiler (inkwell 0.7.1 / LLVM 21)
│   ├── Cargo.toml              # Bootstrap manifest
│   ├── src/
│   │   ├── main.rs             # CLI entry (211 lines)
│   │   └── compiler/
│   │       ├── mod.rs          # Module exports
│   │       ├── lexer.rs        # Lexer (18KB)
│   │       ├── parser.rs       # Parser (43KB)
│   │       ├── codegen.rs      # Codegen entry
│   │       ├── codegen_llvm.rs # LLVM backend (71KB)
│   │       └── codegen_rust.rs # Rust transpiler (23KB)
│   │
│   └── backup/                 # Compiled LLVM IR artifacts
│       ├── kore_native.ll      # Bootstrap IR (623KB)
│       ├── kore_native.exe     # Compiled bootstrap
│       └── kore_runtime.o      # C runtime object
│
└── test/                       # Test files
    ├── test_shader.kr          # Sample shader
    ├── test_shader.spv         # Compiled SPIR-V
    ├── test_shader.hlsl        # Transpiled HLSL
    ├── test_rust.kr            # Rust codegen test
    └── test_rust_output.*      # Compiled artifacts
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