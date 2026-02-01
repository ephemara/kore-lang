# KORE Legacy Commands Reference

> **Note**: This documents the legacy KORE compiler implementation (pre-2025). For the current self-hosted compiler, see the main [README.md](../README.md).

---

## Overview

The **legacy** folder contains two major compiler implementations:

1. **Legacy Compiler** (`legacy/compiler/`) - Original Rust implementation with SPIR-V/WASM backends
2. **Bootstrap Compiler** (`bootstrap/`) - Newer LLVM-based implementation with Rust transpilation

Both are preserved for historical reference and as alternative backends for specialized use cases (shaders, WASM, etc.).

---

## 1. Legacy Compiler (SPIR-V & WASM)

**Location**: `legacy/compiler/`

This was the original KORE compiler with multi-target support.

### Building

```bash
cd legacy/compiler
cargo build --release
```

**Output**: `target/release/kore.exe` (Windows) or `target/release/kore` (Linux/macOS)

### Compile to SPIR-V (GPU Shaders)

```bash
./target/release/kore <input.kr> --target spirv -o <output.spv>
```

**Example**:
```bash
./target/release/kore shaders/pbr.kr --target spirv -o shaders/pbr.spv
```

**Output**: Binary SPIR-V shader file (`.spv`)

### Compile to WASM

```bash
./target/release/kore <input.kr> --target wasm -o <output.wasm>
```

**Output**: WebAssembly binary (`.wasm`)

### Run in Interpreter

```bash
./target/release/kore <input.kr> --target run
```

**Example**:
```bash
./target/release/kore examples/hello.kr --target run
```

**Output**: Executes KORE code directly, prints to console

### Debug Options

```bash
# Emit AST
./target/release/kore <input.kr> --emit-ast

# Emit Typed AST
./target/release/kore <input.kr> --emit-typed

# Verbose output
./target/release/kore <input.kr> --target spirv -v
```

---

## 2. Bootstrap Compiler (LLVM & Rust)

**Location**: `bootstrap/`

The bootstrap compiler uses LLVM via Inkwell and includes a Rust transpiler.

### Building

```bash
cd bootstrap
cargo build --release
```

**Output**: `target/release/korec.exe` (Windows) or `target/release/korec` (Linux/macOS)

### Transpile to Rust

```bash
./bootstrap/target/release/korec <input.kr> --target rust -o <output.rs>
```

**Example**:
```bash
./bootstrap/target/release/korec src/parser.kr --target rust -o output/parser.rs
```

**Output**: Valid Rust source file (`.rs`)

### Compile to LLVM IR

```bash
./bootstrap/target/release/korec <input.kr> --target llvm -o <output.ll>
```

**Output**: LLVM IR file (`.ll`)

### Link and Execute LLVM Output

```bash
# Generate IR
./bootstrap/target/release/korec program.kr --target llvm -o program.ll

# Compile with Clang
clang program.ll runtime/kore_runtime.c -o program.exe

# Run
./program.exe
```

### Full Usage

```
Usage: korec <source.kr> [--target llvm|rust] [-o output]

Targets:
  llvm   - Generate LLVM IR (.ll) - compile with: clang output.ll -o output
  rust   - Transpile to Rust (.rs) - compile with: rustc output.rs
```

---

## 3. Shader Pipeline (KORE → SPIR-V → HLSL)

Use the legacy compiler for shaders, then convert with Naga.

### Install Naga

```bash
cargo install naga-cli
```

### Full Pipeline

```bash
# Step 1: Compile KORE to SPIR-V
./legacy/compiler/target/release/kore shader.kr --target spirv -o shader.spv

# Step 2: Convert SPIR-V to HLSL (for DirectX/UE5)
naga shader.spv shader.hlsl

# Step 3: Or convert to other formats
naga shader.spv shader.wgsl   # WebGPU
naga shader.spv shader.glsl   # OpenGL
naga shader.spv shader.metal  # Metal (macOS/iOS)
```

### One-liner

```bash
./legacy/compiler/target/release/kore shader.kr --target spirv -o shader.spv && naga shader.spv shader.hlsl
```

---

## 4. Unreal Engine 5 Shader Pipeline

The legacy compiler has a dedicated `ue5-shader` target that automates the full pipeline from KORE source to UE5-ready `.usf` shader files.

### What It Does

1. Compiles KORE shader to SPIR-V
2. Validates SPIR-V with `spirv-val` (if available)
3. Transpiles SPIR-V to HLSL using `naga`
4. Generates a `.usf` wrapper that includes the HLSL
5. Optionally copies everything to your UE5 plugin's Shaders directory

### Basic Usage

```bash
# Compile a KORE shader for UE5
./target/release/kore shader.kr --target ue5-shader
```

**Output** (in `./stage/` directory):
- `shader.spv` - SPIR-V binary
- `shader.hlsl` - Transpiled HLSL
- `shader.usf` - UE5 shader wrapper

### Deploy to UE5 Plugin

```bash
# Copy directly to a plugin's Shaders folder
./target/release/kore shader.kr --target ue5-shader --plugin MyPlugin

# Specify custom plugins directory
./target/release/kore shader.kr --target ue5-shader --plugin MyPlugin --plugins-dir "C:/UE5/MyProject/Plugins"
```

This copies `shader.hlsl` and `shader.usf` to:
```
{plugins-dir}/MyPlugin/Shaders/shader.hlsl
{plugins-dir}/MyPlugin/Shaders/shader.usf
```

### CLI Options for UE5

| Flag | Description |
|------|-------------|
| `--target ue5-shader` | Enable UE5 shader pipeline |
| `--plugin <name>` | Target UE5 plugin name |
| `--plugins-dir <path>` | UE5 plugins directory (default: scans common locations) |
| `--dry-run` | Show what would be done without executing |
| `--verbose` | Show detailed pipeline steps |

### Requirements

- **naga-cli**: `cargo install naga-cli`
- **spirv-val** (optional): Part of Vulkan SDK, used for validation

### Example Workflow

```bash
# 1. Write your shader in KORE
cat > my_effect.kr << 'EOF'
shader fragment MyEffect:
    uniform time: Float
    in uv: vec2
    out color: vec4
    
    fn main():
        let wave = sin(uv.x * 10.0 + time) * 0.5 + 0.5
        color = vec4(wave, uv.y, 0.5, 1.0)
EOF

# 2. Compile and deploy to your plugin
./target/release/kore my_effect.kr --target ue5-shader --plugin MyRenderPlugin -v

# 3. In UE5, reference it as:
#    /Plugin/MyRenderPlugin/Shaders/my_effect.usf
```

---

## 5. Common Workflows

### Workflow A: Test KORE Code (Interpreter)

```bash
# Write code in examples/test.kr
# Run directly without compilation
./legacy/compiler/target/release/kore examples/test.kr --target run
```

### Workflow B: Generate Rust Library

```bash
# Transpile KORE to Rust
./bootstrap/target/release/korec my_module.kr --target rust -o output/my_module.rs

# Review generated code
cat output/my_module.rs

# Integrate into Rust project
# (manually add to lib.rs or use as standalone module)
```

### Workflow C: Compile GPU Shader

```bash
# 1. Write shader in KORE
# 2. Compile to SPIR-V
./legacy/compiler/target/release/kore my_shader.kr --target spirv -o my_shader.spv

# 3. Validate with spirv-val (optional)
spirv-val my_shader.spv

# 4. Convert to target format
naga my_shader.spv my_shader.hlsl
```

---

## 5. Troubleshooting

### SPIR-V Validation Errors

If Naga fails with validation errors, the SPIR-V output is malformed:

| Error | Cause | Fix |
|-------|-------|-----|
| `Type isn't compatible with address space` | Wrong storage class | Check uniform declarations |
| `Missing @builtin(position)` | Vertex shader output | Ensure `-> Vec4` has Position builtin |
| `Global variable invalid` | Missing Block decoration | Wrap uniform in struct |

### Rust Transpilation Issues

If generated Rust fails to compile:
1. Check if KORE source uses patterns the codegen doesn't handle
2. Manually refactor or add type annotations
3. Report codegen bug for future improvement

### Build Performance

```bash
# Fast syntax check (no codegen)
cargo check

# Build only the binary, skip tests/docs
cargo build --release --bin kore

# Use release mode for real projects (dev builds are slow)
cargo build --release
```

---

## 6. Why Two Compilers?

| Compiler | Use Case | Backends |
|----------|----------|----------|
| **Legacy** | GPU shaders, WASM apps | SPIR-V, WASM, Interpreter |
| **Bootstrap** | Native binaries, Rust codegen | LLVM IR, Rust transpiler |

The **current self-hosted compiler** (main repo) replaces both with a single KORE-to-native pipeline. These legacy implementations remain for:
- SPIR-V shader generation
- WASM compilation
- Reference implementation for new backends
- Historical documentation

---

## 7. Migration Path

To use the **current compiler** instead:

```bash
# See main README.md for build instructions
./build.sh              # Linux/macOS
./build.ps1             # Windows

# Output: ./build/kore_native.exe

# Run KORE code
./build/kore_native.exe examples/hello.kr
```

The current compiler is **self-hosting** and actively maintained. Use legacy compilers only for specialized targets (SPIR-V, WASM).

---

*For full documentation, see [../README.md](../README.md)*
