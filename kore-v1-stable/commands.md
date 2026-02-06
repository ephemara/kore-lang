# KORE V1 Commands Reference

> **Production Compiler**: Full documentation for the unified KORE V1 compiler with all targets in one binary.

---

## Overview

The KORE V1 compiler (`kore`) is a single unified binary that supports **8 compilation targets**:

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
cd kore-v1-stable
cargo build --release
```

**Binary**: `target/release/kore` (or `kore.exe` on Windows)

---

## 1. Running KORE Code (Interpreter)

```bash
# Run directly
./target/release/kore examples/hello.kr --target run

# Shorthand
./target/release/kore examples/hello.kr -t r
```

---

## 2. Compiling to WebAssembly

```bash
./target/release/kore examples/app.kr --target wasm -o output.wasm
```

---

## 3. Compiling GPU Shaders

### Direct HLSL (DirectX)

```bash
# KORE → HLSL (direct codegen, no intermediate step)
./target/release/kore shaders/pbr.kr --target hlsl -o pbr.hlsl
```

### Direct USF (Unreal Engine 5)

```bash
# KORE → USF (production UE5 shader)
./target/release/kore shaders/pbr.kr --target usf -o pbr.usf
```

Drop the `.usf` file directly into your UE5 plugin's `Shaders/` folder.

### SPIR-V (Cross-Platform)

```bash
# Compile to SPIR-V (for naga cross-compilation)
./target/release/kore shaders/pbr.kr --target spirv -o pbr.spv

# Convert with naga
naga pbr.spv pbr.wgsl   # WebGPU
naga pbr.spv pbr.glsl   # OpenGL
naga pbr.spv pbr.metal  # Metal
```

---

## 4. Transpiling to Rust

```bash
# Transpile KORE to Rust
./target/release/kore stdlib/runtime.kr --target rust -o runtime.rs

# Compile the generated Rust
rustc runtime.rs -o runtime
```

---

## 5. UE5 Shader Pipeline

### Direct USF (Recommended)

```bash
# Direct compilation to UE5-ready shader
./target/release/kore shaders/effect.kr --target usf -o effect.usf
```

### Legacy: ue5-shader Target

For SPIR-V workflow with naga conversion:

```bash
./target/release/kore shaders/effect.kr --target ue5-shader --plugin MyPlugin
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
kore-v1-stable/
├── src/           # Rust compiler source
├── stdlib/        # KORE standard library
├── shaders/       # GPU shader examples
├── examples/      # General examples
├── bootstrap/     # Self-hosting compiler (KORE)
├── tests/         # Test files
└── runtime/       # C FFI runtime
```

---

## 9. Common Workflows

### Test KORE Code
```bash
./target/release/kore examples/test.kr --target run
```

### Compile UE5 Shader (Direct)
```bash
./target/release/kore shaders/pbr.kr --target usf -o MyPlugin/Shaders/pbr.usf
```

### Compile UE5 Shader (Legacy Pipeline)
```bash
./target/release/kore shaders/pbr.kr --target ue5-shader --plugin MyPlugin -v
```

### Generate Rust Library
```bash
./target/release/kore stdlib/runtime.kr --target rust -o kore_runtime.rs
rustc kore_runtime.rs --crate-type lib -o libkore_runtime.rlib
```

---

## 10. Troubleshooting

| Issue | Solution |
|-------|----------|
| SPIR-V validation fails | Check uniform/builtin declarations |
| Naga errors | Validate with `spirv-val` first |
| Rust compile errors | Check type annotations in KORE source |

### Install Required Tools

```bash
# Naga (SPIR-V to HLSL/GLSL/WGSL converter)
cargo install naga-cli

# SPIR-V Tools (optional validation)
# Install Vulkan SDK from https://vulkan.lunarg.com/
```

---

*For language syntax and features, see the main [README.MD](./README.MD)*
