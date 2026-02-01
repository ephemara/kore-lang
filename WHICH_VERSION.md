# Which KORE Compiler Should I Use?

This repository contains two compiler implementations. Here's how to choose:

## Quick Decision Tree

```
Are you building production software?
├─ YES → Use V1 (/kore-v1-stable/)
│
└─ NO → Are you contributing to compiler development?
    ├─ YES → Use V2 (root directory)
    └─ NO → Still use V1 for stability
```

## Detailed Comparison

### V1 Production Compiler (`/kore-v1-stable/`)

**Status**: Production-Ready  
**Location**: `/kore-v1-stable/`  
**Best For**: Real projects, shaders, WASM apps, actor systems

#### Features
- WASM Codegen - Full WebAssembly support
- SPIR-V Shaders - GPU shader compilation with UE5 pipeline
- Actor Runtime - Erlang-style concurrency
- Python FFI - Call Python from KORE
- Interpreter - Instant execution without compilation
- LSP - Language Server Protocol support
- Rust Transpiler - Compile KORE to Rust source

#### Build & Run
```bash
cd kore-v1-stable
cargo build --release

# Compile to WASM
./target/release/kore program.kr --target wasm -o program.wasm

# Compile shader to SPIR-V
./target/release/kore shader.kr --target spirv -o shader.spv

# Run in interpreter
./target/release/kore program.kr --target run
```

#### Documentation
- [V1 README](kore-v1-stable/README.MD)
- [V1 Commands Reference](kore-v1-stable/commands.md)

---

### V2 Self-Hosting Compiler (Root Directory)

**Status**: Experimental (Project Ouroboros)  
**Location**: `/` (root)  
**Best For**: Compiler development, self-hosting research

#### Features
- Self-Hosting - KORE compiler written in KORE
- LLVM Native - Native code generation via LLVM IR
- NaN-Boxing Runtime - Efficient value representation
- Generics - Type parameter support (in progress)
- Advanced Type System - Effect tracking, ownership

#### Build & Run
```bash
# Build native compiler (recommended for dev)
./build.ps1 -SkipSelfHosted    # Windows
./build.sh                      # Linux/macOS

# Compile KORE to LLVM IR
./build/artifacts/latest/kore_native.exe program.kr -o program.ll

# Link and run
clang program.ll build/kore_runtime.o -o program.exe
./program.exe
```

#### Documentation
- [Main README](README.md)
- [Contributing Guide](CONTRIBUTING.md)
- [LLM Guide](LLM_GUIDE.md)

---

## Feature Matrix

| Feature | V1 Production | V2 Experimental |
|---------|:-------------:|:---------------:|
| **LLVM Native** | Via bootstrap | Primary target |
| **WASM** | Stable | In progress |
| **SPIR-V Shaders** | Stable | In progress |
| **Rust Transpiler** | Stable | Not yet |
| **Interpreter** | Full-featured | In progress |
| **Actor System** | Production | In progress |
| **Python FFI** | Working | Not yet |
| **LSP** | Working | In progress |
| **Generics** | Limited | Full support |
| **Self-Hosting** | No | Yes |
| **Effect System** | Basic | Advanced |

Legend:
- Stable/Production/Full support: Production-ready
- In progress / Working: Experimental or partial
- Limited: Partial support
- Not yet: Not available

---

## Use Case Recommendations

### Use V1 If You Need:
- **UE5 Shader Development** - V1 has the complete SPIR-V → HLSL pipeline
- **WebAssembly Apps** - V1's WASM codegen is battle-tested
- **Actor-Based Systems** - V1 has a full Erlang-style actor runtime
- **Python Integration** - V1 supports Python FFI via pyo3
- **Immediate Execution** - V1's interpreter is feature-complete
- **Stability** - V1 is production-ready

### Use V2 If You Need:
- **Native Performance** - V2's LLVM backend generates optimized native code
- **Generics** - V2 has full generic type support
- **Compiler Development** - V2 is self-hosting and written in KORE
- **Cutting Edge** - V2 has the latest language features
- **Contributing** - V2 is where active development happens

### Use Both If:
- You're developing shaders (V1) and native tools (V2)
- You want to compare implementations
- You're researching compiler design

---

## Migration Path

**V1 → V2**: When V2 reaches production status, migration should be straightforward as both implement the same language spec. The main differences are in compilation targets and runtime features.

**Timeline**: V2 is expected to reach feature parity with V1 by mid-2026, at which point V1 will be maintained for legacy support.

---

## Still Unsure?

**Default recommendation**: Start with **V1** (`/kore-v1-stable/`) unless you specifically need V2's self-hosting or native LLVM features.

**Questions?** See [CONTRIBUTING.md](CONTRIBUTING.md) or open a GitHub issue.
