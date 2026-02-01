# Contributing to KORE

Thank you for your interest in contributing to KORE! This document provides guidelines for contributing to the project.

## Table of Contents

- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Build System](#build-system)
- [Making Changes](#making-changes)
- [Testing](#testing)
- [Code Style](#code-style)
- [Pull Request Process](#pull-request-process)
- [Architecture Notes](#architecture-notes)

---

## Development Setup

### Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| PowerShell | 5.1+ | Build scripts (Windows) |
| Clang/LLVM | 17+ | Linking and optimization |
| Rust | 1.70+ | Bootstrap compiler |

### Initial Setup

```powershell
# Clone the repository
git clone https://github.com/ephemara/kore-lang.git
cd Kore

# Build the native compiler (skip self-hosted for faster iteration)
.\build.ps1 -SkipSelfHosted

# Verify the build
.\build\artifacts\latest\kore_native.exe --help
```

### IDE Setup

KORE files use the `.kr` extension. Configure your editor for:
- 4-space indentation (no tabs)
- Significant whitespace (Python-like)

---

## Project Structure

```
kore-main/
├── src/                    # KORE compiler source (KORE language)
│   ├── korec.kr            # Entry point, CLI argument parsing
│   ├── lexer.kr            # Tokenizer
│   ├── parser_v2.kr        # Parser with generics support
│   ├── types.kr            # Type checker with effect inference
│   ├── codegen.kr          # LLVM IR generation
│   ├── ast.kr              # Abstract syntax tree definitions
│   ├── resolver.kr         # Import/module resolution
│   ├── diagnostic.kr       # Error message formatting
│   ├── effects.kr          # Effect system types
│   ├── span.kr             # Source location tracking
│   └── stdlib.kr           # Standard library function bindings
│
├── bootstrap/              # Stage 0: Rust bootstrap compiler
│   ├── Cargo.toml
│   └── src/
│       └── ...             # Rust implementation
│
├── runtime/                # C runtime library
│   ├── kore_runtime.c      # NaN-boxing runtime implementation
│   └── kore_runtime.o      # Pre-compiled object (rebuilt as needed)
│
├── tests/                  # Test suite
│   ├── unit/               # Unit tests for individual features
│   ├── integration/        # End-to-end tests
│   └── examples/           # Demo programs (smoke tests)
│
├── legacy/                 # Legacy compiler (stable WASM/SPIR-V backends)
├── build/                  # Build artifacts (gitignored)
└── build.ps1               # Main build script
```

---

## Build System

### Build Targets

| Command | Description |
|---------|-------------|
| `.\build.ps1` | Full build (native + self-hosted) |
| `.\build.ps1 -SkipSelfHosted` | Build native only (recommended for dev) |
| `.\build.ps1 -Target native` | Build Stage 1 native compiler |
| `.\build.ps1 -Target bootstrap` | Build Stage 0 Rust compiler |
| `.\build.ps1 -Target self` | Build Stage 2 self-hosted |
| `.\build.ps1 -Target test` | Run test suite |
| `.\build.ps1 -Target runtime` | Compile C runtime |
| `.\build.ps1 -Clean` | Clean build artifacts |

### Build Pipeline

1. **Combine Sources**: All `src/*.kr` files are combined into `build/korec_build.kr`
2. **Stage 0 → IR**: Bootstrap compiler generates LLVM IR
3. **Fix IR**: Deduplicate declarations, add missing types
4. **Link**: Clang links IR with `kore_runtime.o`
5. **Stage 1 Ready**: Native compiler at `build/artifacts/latest/kore_native.exe`

### Debugging a Build

```powershell
# Enable verbose output
.\build.ps1 -Verbose -Debug

# Check build logs
Get-Content .\build\logs\bootstrap_*.log

# Force runtime rebuild if runtime changed
.\build.ps1 -ForceRuntimeRebuild

# Require IR verification (will fail on edge cases)
.\build.ps1 -Verify
```

---

## Making Changes

### Compiler Changes

The compiler source is in `src/`. Each file has a specific responsibility:

| File | Purpose | Key Types |
|------|---------|-----------|
| `lexer.kr` | Tokenization | `Token`, `TokenKind`, `Lexer` |
| `parser_v2.kr` | Parsing | `Parser`, `Item`, `Stmt`, `Expr` |
| `types.kr` | Type checking | `TypeChecker`, `ResolvedType`, `TypedExpr` |
| `codegen.kr` | LLVM IR output | `CodeGen`, `StringBuilder` |
| `korec.kr` | CLI & orchestration | `ArgParser`, `Compiler`, `CompilerConfig` |

### Adding a New Feature

1. **AST Changes**: Add new variants to `ast.kr`
2. **Parser Changes**: Handle syntax in `parser_v2.kr`
3. **Type Checker**: Add type rules in `types.kr`
4. **Codegen**: Emit LLVM IR in `codegen.kr`
5. **Runtime**: Add C functions if needed in `runtime/kore_runtime.c`
6. **Tests**: Add test cases in `tests/`

### Runtime Changes

The runtime uses NaN-boxing. Key constants in `codegen.kr`:

```kore
// NaN-boxing tags (must match kore_runtime.c)
fn nanbox_int_tag() -> String:
    return "-2216615441596416"

fn nanbox_bool_tag() -> String:
    return "-2181431069507584"
```

If you change runtime signatures, update:
1. `runtime/kore_runtime.c` - Implementation
2. `src/codegen.kr` - `emit_externals()` function
3. `src/stdlib.kr` - Standard library bindings

---

## Testing

### Running Tests

```powershell
# Run all tests
.\build.ps1 -Target test

# Run smoke tests (after native build)
.\build.ps1 -RunSmokeTests
```

### Test Structure

```
tests/
├── unit/
│   ├── test_lexer.kr       # Lexer tests
│   ├── test_parser.kr      # Parser tests
│   └── test_*.kr           # Other unit tests
│
├── integration/
│   ├── test_full.kr        # Full pipeline tests
│   └── ...
│
├── examples/
│   └── demo_pack_*/        # Smoke test programs
│       ├── hello.kr
│       └── hello.expected  # Expected output
│
└── *.kr                    # Root-level tests
```

### Writing Tests

Each test file should:
1. Use `assert` or produce deterministic output
2. Have a matching `.expected` file for output verification

```kore
// tests/unit/test_factorial.kr
fn factorial(n: Int) -> Int:
    if n <= 1:
        return 1
    return n * factorial(n - 1)

fn main():
    println(str(factorial(5)))  // Output: 120
```

```
// tests/unit/test_factorial.expected
120
```

---

## Code Style

### KORE Style Guide

```kore
// Use 4-space indentation
fn example():
    let x = 10
    if x > 5:
        println("big")

// Function names: snake_case
fn calculate_total(items: Array<Item>) -> Int:
    ...

// Type names: PascalCase
struct UserProfile:
    name: String
    age: Int

// Constants: SCREAMING_SNAKE_CASE (via convention)
let MAX_BUFFER_SIZE = 4096

// Comments: explain WHY, not WHAT
// Avoid redundant comments that just restate the code
```

### Comment Guidelines

- Use `//` for line comments
- Explain non-obvious decisions
- Document public APIs with doc comments (`///`)
- Avoid emojis in comments (professional codebase)

---

## Pull Request Process

### Before Submitting

1. **Build succeeds**: `.\build.ps1 -SkipSelfHosted`
2. **Tests pass**: `.\build.ps1 -Target test`
3. **No new warnings**: Check compiler output
4. **Code formatted**: Follow style guide

### PR Checklist

- [ ] Descriptive title and description
- [ ] Tests added for new functionality
- [ ] Documentation updated if needed
- [ ] No hardcoded paths or personal configuration

### Review Process

1. **Automated checks**: Build and test verification
2. **Code review**: Maintainer reviews changes
3. **Iteration**: Address feedback
4. **Merge**: Squash and merge when approved

---

## Architecture Notes

### Self-Hosting Stages

| Stage | Compiler | Input | Output |
|-------|----------|-------|--------|
| 0 | Rust (`bootstrap/`) | KORE source | LLVM IR |
| 1 | Native (`kore_native.exe`) | KORE source | LLVM IR |
| 2 | Self-hosted (`kore_native_v2.exe`) | KORE source | LLVM IR |

The goal is for Stage 1 to compile itself (Stage 2), validating the compiler's correctness.

### NaN-Boxing

All values are represented as 64-bit integers using IEEE 754 NaN-boxing:

- **Floats**: Valid IEEE 754 doubles (< quiet NaN threshold)
- **Tagged values**: Upper bits indicate type, lower 45 bits are payload

### Known Limitations

1. **Bootstrap Parser**: No generics support - use native compiler for generic code
2. **IR Verification**: Occasional failures on complex patterns
3. **Self-Host Cycle**: Not fully automated yet

### Priority Contributions

- [ ] Fix IR verification edge cases
- [ ] Improve error messages in `diagnostic.kr`
- [ ] Expand test coverage
- [ ] Document standard library functions
- [ ] Add pattern matching exhaustiveness checking

---

## Getting Help

- **Issues**: Open a GitHub issue for bugs or feature requests
- **Discussions**: Use GitHub Discussions for questions
- **Documentation**: Check `docs/` and this file

---

Thank you for contributing to KORE!
