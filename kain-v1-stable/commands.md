# KAIN V1 Commands Reference

> **Production Compiler**: Full documentation for the unified KAIN V1 compiler with all targets in one binary.

---

## Overview

The KAIN V1 compiler (`kain`) is a single unified binary that supports **8 compilation targets**:

| Target | Output | Use Case |
|--------|--------|----------|
| WASM | `.wasm` | Web applications |
| LLVM | `.ll` | Native binaries |
| SPIR-V | `.spv` | Cross-platform GPU shaders |
| **HLSL** | `.hlsl` | DirectX shaders (direct, no middleman) |
| **USF** | `.usf` | Unreal Engine 5 shaders (direct) |
| Rust | `.rs` | Rust interop |
| Interpret | stdout | Quick testing |
| Test | stdout | Unit tests |

---

## Building

```bash
cd kain-v1-stable
cargo build --release
```

**Binary**: `target/release/kain` (or `kain.exe` on Windows)

---

## 1. Running KAIN Code (Interpreter)

```bash
# Run directly
./target/release/kain examples/hello.kn --target run

# Shorthand
./target/release/kain examples/hello.kn -t r
```

---

## 2. Compiling to WebAssembly

```bash
./target/release/kain examples/app.kn --target wasm -o output.wasm
```

---

## 3. Compiling GPU Shaders

### Direct HLSL (DirectX)

```bash
# KAIN → HLSL (direct codegen, no intermediate step)
./target/release/kain shaders/pbr.kn --target hlsl -o pbr.hlsl
```

### Direct USF (Unreal Engine 5)

```bash
# KAIN → USF (production UE5 shader)
./target/release/kain shaders/pbr.kn --target usf -o pbr.usf
```

Drop the `.usf` file directly into your UE5 plugin's `Shaders/` folder.

### SPIR-V (Cross-Platform)

```bash
# Compile to SPIR-V (for naga cross-compilation)
./target/release/kain shaders/pbr.kn --target spirv -o pbr.spv

# Convert with naga
naga pbr.spv pbr.wgsl   # WebGPU
naga pbr.spv pbr.glsl   # OpenGL
naga pbr.spv pbr.metal  # Metal
```

---

## 4. Transpiling to Rust

```bash
# Transpile KAIN to Rust
./target/release/kain stdlib/runtime.kn --target rust -o runtime.rs

# Compile the generated Rust
rustc runtime.rs -o runtime
```

---

## 5. UE5 Shader Pipeline

### Direct USF (Recommended)

```bash
# Direct compilation to UE5-ready shader
./target/release/kain shaders/effect.kn --target usf -o effect.usf
```

### Legacy: ue5-shader Target

For SPIR-V workflow with naga conversion:

```bash
./target/release/kain shaders/effect.kn --target ue5-shader --plugin MyPlugin
```

**Generated files** (in `./stage/`):
- `effect.spv` - SPIR-V binary
- `effect.hlsl` - Transpiled HLSL
- `effect.usf` - UE5 wrapper

---

## 6. CLI Options

| Flag | Description |
|------|-------------|
| `-o, --output <file>` | Output file path |
| `-t, --target <target>` | Compilation target |
| `-r, --run` | Run after compilation |
| `-w, --watch` | Watch mode (auto-recompile) |
| `--emit-ast` | Dump parsed AST |
| `--emit-typed` | Dump typed AST |
| `-v, --verbose` | Verbose output |
| `--dry-run` | Preview actions |

---

## 7. Target Aliases

| Target | Aliases | Notes |
|--------|---------|-------|
| WASM | `wasm`, `w` | Web applications |
| LLVM | `llvm`, `native`, `n` | Native binaries |
| SPIR-V | `spirv`, `gpu`, `shader`, `s` | Cross-platform shaders |
| **HLSL** | `hlsl`, `h` | DirectX (direct) |
| **USF** | `usf`, `ue5` | Unreal Engine 5 (direct) |
| Rust | `rust`, `rs` | Rust transpilation |
| Interpret | `run`, `interpret`, `i`, `r` | Quick testing |
| Test | `test`, `t` | Unit tests |

---

## 8. Directory Structure

```
kain-v1-stable/
├── src/           # Rust compiler source
├── stdlib/        # KAIN standard library
├── shaders/       # GPU shader examples
├── examples/      # General examples
├── bootstrap/     # Self-hosting compiler (KAIN)
├── tests/         # Test files
└── runtime/       # C FFI runtime
```

---

## 9. Common Workflows

### Test KAIN Code
```bash
./target/release/kain examples/test.kn --target run
```

### Compile UE5 Shader (Direct)
```bash
./target/release/kain shaders/pbr.kn --target usf -o MyPlugin/Shaders/pbr.usf
```

### Compile UE5 Shader (Legacy Pipeline)
```bash
./target/release/kain shaders/pbr.kn --target ue5-shader --plugin MyPlugin -v
```

### Generate Rust Library
```bash
./target/release/kain stdlib/runtime.kn --target rust -o kain_runtime.rs
rustc kain_runtime.rs --crate-type lib -o libkain_runtime.rlib
```

---

## 10. Troubleshooting

| Issue | Solution |
|-------|----------|
| SPIR-V validation fails | Check uniform/builtin declarations |
| Naga errors | Validate with `spirv-val` first |
| Rust compile errors | Check type annotations in KAIN source |

### Install Required Tools

```bash
# Naga (SPIR-V to HLSL/GLSL/WGSL converter)
cargo install naga-cli

# SPIR-V Tools (optional validation)
# Install Vulkan SDK from https://vulkan.lunarg.com/
```

---

*For language syntax and features, see the main [README.MD](./README.MD)*
