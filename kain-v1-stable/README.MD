MAKE SURE TO CHECK THIS GITHUB REPO FREQUENTLY TO STAY UP TO DATE WITH DEVELOPMENT. WHILE KAIN-V1 IS STABLE, IT IS ACTIVELY BEING DEVELOPED EVERY DAY WITH NEW FEATURES AND OPTIMIZATIONS/BUG FIXES ETC.

HUGE UPDATE:

2/6/25 - Direct USF (Unreal Shader Format) backend is now live! Compile kain shaders directly to production UE5 code with `-t usf`. HLSL backend also fully functional. 

2/7/25 - PYTHON FFI is fully live. See the /python_FFI folder for an example. FIXED some edge cases in the USF pipeline, it is now production ready.

# KAIN V1 - Production Compiler

> **Production-Ready Release**: This is the stable, feature-complete V1 compiler with Actor runtime, UE5 shader pipeline, Python FFI, and multiple compilation targets. While the V2 self-hosting compiler (in the root directory) is under active development, V1 is battle-tested and ready for production use.

## Philosophy

kain combines the best ideas from multiple paradigms:

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
cargo install kain-lang
```

That's it. You now have the `kain` command available system-wide.

### Build from Source

If you prefer building from source:

```bash
cd kain-v1-stable
cargo build --release
```

**Binary**: `target/release/kain` (or `kain.exe` on Windows)

---

## CLI Reference

### Main Compiler (`kain`)

```bash
# Run in interpreter (instant execution)
./target/release/kain example.kn --target run

# Compile to WebAssembly
./target/release/kain example.kn --target wasm -o output.wasm

# Compile to LLVM IR (then link with clang)
./target/release/kain example.kn --target llvm -o output.ll

# Compile to SPIR-V (GPU shader bytecode)
./target/release/kain shader.kn --target spirv -o shader.spv

# Transpile to Rust source code
./target/release/kain example.kn --target rust -o output.rs

# Run tests
./target/release/kain test_file.kn --target test

# Watch mode (auto-recompile on file change)
./target/release/kain example.kn --target wasm --watch
```

### Compilation Targets

| Target | Flag(s) | Output | Notes |
|--------|---------|--------|-------|
| WASM | `-t wasm`, `-t w` | `.wasm` | Web applications |
| LLVM/Native | `-t llvm`, `-t native`, `-t n` | `.ll` | Native binaries |
| SPIR-V | `-t spirv`, `-t gpu`, `-t s` | `.spv` | Cross-platform shaders |
| **HLSL** | `-t hlsl`, `-t h` | `.hlsl` | DirectX (direct codegen, no middleman) |
| **USF** | `-t usf`, `-t ue5` | `.usf` | Unreal Engine 5 (direct codegen) |
| Rust | `-t rust`, `-t rs` | `.rs` | Rust transpilation |
| Interpret | `-t run`, `-t interpret`, `-t i` | stdout | Instant execution |
| Test | `-t test`, `-t t` | stdout | Unit tests |

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
kain init [path] [--name NAME]    # Initialize a new project
kain lsp                          # Start Language Server Protocol
kain build <file>                 # Compile to WASM (default)
kain run <file>                   # Execute via interpreter
```

---

## Shader Pipeline

kain includes **two direct shader backends** - no intermediate SPIR-V or external tools required.

### HLSL Backend (DirectX)

```bash
# Direct kain → HLSL (no middleman)
kain shader.kn --target hlsl -o shader.hlsl
```

**Generated output:**
- `cbuffer` with register bindings
- `Texture2D` + `SamplerState` pairs
- `VSInput`/`PSOutput` structs with semantics
- Full PBR, tonemapping, post-processing

### USF Backend (Unreal Engine 5)

```bash
# Direct kain → USF (production UE5 shaders)
kain shader.kn --target usf -o shader.usf
```

**Generated output:**
- `#include "/Engine/Public/Platform.ush"`
- Scalar parameters (C++ bindable via `SHADER_PARAMETER_STRUCT`)
- `Texture2D<float4>` + `SamplerState` pairs with registers
- `FPSInput`/`FPSOutput` UE5-style structs
- `[numthreads]` compute shader support

### Alternative: SPIR-V → Cross-Platform

For multi-platform output, use SPIR-V with naga:

```bash
kain shader.kn --target spirv -o shader.spv
naga shader.spv shader.wgsl   # WebGPU
naga shader.spv shader.glsl   # OpenGL
naga shader.spv shader.metal  # Metal
```

---

## Unreal Engine 5 Integration

Two approaches for UE5 shaders:

### Direct USF (Recommended)

```bash
# One command - production UE5 shader
kain shader.kn --target usf -o MyShader.usf
```

Drop the `.usf` file directly into your plugin's `Shaders/` directory.

**Example generated output:**
```hlsl
#include "/Engine/Public/Platform.ush"

float NormalStrength;   // C++ bindable
Texture2D<float4> albedo_map : register(t0);
SamplerState albedo_mapSampler : register(s0);

struct FPSInput { float2 uv : TEXCOORD0; };
struct FPSOutput { float4 Color : SV_Target0; };

FPSOutput ShaderNamePS(FPSInput Input) { ... }
```

**Using in C++:**
```cpp
IMPLEMENT_GLOBAL_SHADER(FMyEffectPS, "/Plugin/MyPlugin/Shaders/MyShader.usf", "ShaderNamePS", SF_Pixel);
```

### Legacy: ue5-shader Pipeline

For SPIR-V workflow with naga conversion:

```bash
kain shader.kn --target ue5-shader --plugin MyPlugin
```

Generates `.spv`, `.hlsl`, and `.usf` wrapper.

---

## Rust Transpiler

kain can be transpiled to valid Rust source code:

```bash
# Transpile to Rust
./target/release/kain logic.kn --target rust -o logic.rs

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
kain-v1-stable/
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
├── stdlib/                 # kain standard library
│   ├── runtime.kn          # Runtime utilities
│   ├── wasm.kn             # WASM utilities
│   ├── spirv.kn            # SPIR-V utilities
│   └── ...                 # 12 modules total
│
├── shaders/                # GPU shader examples
│   ├── pbr_material.kn     # PBR shader
│   ├── post_processing.kn  # Post-processing effects
│   └── ...                 # 5 shaders
│
├── examples/               # General kain examples
├── bootstrap/              # Self-hosting compiler (in kain)
├── tests/kain/             # Test files
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

### HLSL Backend (`codegen/hlsl.rs` - 25KB)

Direct DirectX shader generation (no intermediate step):
- `cbuffer` with register bindings
- `Texture2D` + `SamplerState` pairs
- `VSInput`/`PSOutput` structs with semantics
- Full PBR math, tonemapping, post-processing
- SM5.0 compatible output

### USF Backend (`codegen/usf.rs` - 20KB)

Direct Unreal Engine 5 shader generation:
- `#include "/Engine/Public/Platform.ush"`
- Scalar parameters (C++ bindable via `SHADER_PARAMETER_STRUCT`)
- `Texture2D<float4>` + `SamplerState` pairs with registers
- `FPSInput`/`FPSOutput` UE5-style structs
- `[numthreads]` compute shader support
- `RWTexture2D` UAV bindings

### Rust Backend (`codegen/rust.rs` - 28KB)

Transpilation to Rust source:
- Full AST transformation
- Type mapping (kain → Rust)
- Operator translation
- Pattern matching support
- Generated `Cargo.toml` scaffolding

---

## C Runtime

The LLVM backend links against a minimal C runtime (`src/runtime/c/kain_runtime.c`):

| Function | Description |
|----------|-------------|
| `kain_print_*` | Print functions for all types |
| `kain_str_*` | String operations (concat, len, eq, substring) |
| `kain_array_*` | Dynamic array (new, push, pop, get, set, len) |
| `kain_map_*` | Hash map (new, get, set, contains) |
| `kain_file_*` | File I/O (read, write) |
| `kain_alloc/free` | Memory management |
| `kain_panic` | Runtime panic |
| `args()` | Command-line arguments |

### Compiling with Runtime

```bash
# Step 1: Compile kain to LLVM IR
./target/release/kain program.kn --target llvm -o program.ll

# Step 2: Compile C runtime
clang -c src/runtime/c/kain_runtime.c -o kain_runtime.o

# Step 3: Link everything
clang program.ll kain_runtime.o -o program.exe
```

---

## Self-Hosted Compiler

The `src/compiler/` directory contains the **self-hosted kain compiler** written in kain itself (Project Ouroboros).

### Components

| File | Lines | Description |
|------|-------|-------------|
| `main.kn` | 284 | Compiler driver, CLI parsing |
| `lexer.kn` | ~400 | Tokenizer |
| `parser.kn` | ~800 | Recursive descent parser |
| `codegen.kn` | ~800 | LLVM IR generation |
| `codegen_rust.kn` | 684 | Rust transpilation |

### Running Self-Hosted

```bash
# The self-hosted compiler runs through the interpreter
./target/release/kain src/compiler/main.kn --target run -- input.kn --target rust
```

---

## kain Utilities (Written in kain)

The `src/other/` directory contains utilities written in kain itself:

| File | Lines | Description |
|------|-------|-------------|
| `repl.kn` | 432 | Interactive REPL with history, commands |
| `formatter.kn` | 751 | Code formatter (`kain fmt`) |
| `lsp.kn` | ~300 | Language Server Protocol |
| `test_runner.kn` | ~200 | Test framework |
| `suggestions.kn` | ~150 | Error message suggestions |
| `import_resolver.kn` | ~200 | Module import resolution |
| `packager.kn` | ~250 | Package management |

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
```kain
actor Counter:
    var count: Int = 0
    
    on Increment(n: Int):
        count = count + n
    
    on GetCount -> Int:
        return count
```

### Python Interop

Via `pyo3`:
```kain
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

```kain
fn factorial(n: Int) -> Int with Pure:
    match n:
        0 => 1
        _ => n * factorial(n - 1)

fn main():
    let result = factorial(5)
    println("5! = {result}")
```

### Shader

```kain
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

```kain
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
| **HLSL Codegen** | Production | Direct DirectX output |
| **USF Codegen** | Production | Direct UE5 shader output |
| Rust Transpiler | Stable | Bootstrap compiler |
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
cargo build --release --bin kain
```

---

## License

MIT
