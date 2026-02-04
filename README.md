<img src="docs/kore_timeline_final.gif" alt="KORE - Timeline Animation" width="1041">


<a href="#quick-start">Quick Start</a> • <a href="#features">Features</a> • <a href="#architecture">Architecture</a> • <a href="#building">Building</a> • <a href="#cli-reference">CLI</a> • <a href="#contributing">Contributing</a>

- - -

## Repository Structure

This repository contains **two compiler implementations**:

| Version | Location | Status | Best For |
|---------|----------|--------|----------|
| **V2 Self-Hosting** | `/` (root) | Experimental | Next-gen compiler development, LLVM native |
| **V1 Production** | `/kore-v1-stable/` | Production-Ready | WASM, SPIR-V shaders, Actor runtime, UE5 integration |

**New users**: Start with **V1** (`/kore-v1-stable/`) for production work, especially for shaders and WASM.  
**Contributors**: V2 (root) is where the self-hosting magic happens - help us make it production-ready!

**Detailed comparison**: See [WHICH_VERSION.md](WHICH_VERSION.md) for a complete feature matrix and use case guide.

- - -
Kore has been a private repo for years, and you may be asking where did these 337~ files spawn from? My old repo had way too much personal info in the git commits so for the public release the only option was to start fresh and host it in a new repo.

If you're curious about what this language is designed to power, head over to [Greeble.co](https://www.greeble.co/). You can find a sample of my in-progress 3D DCC (1/14 modules) and a preview of a new Unreal Engine 5 plugin featuring effectors, cloners, and MoGraph tools in the style of C4D.

- - -

## What is KORE?

KORE is a **self-hosting programming language** that combines the best ideas from multiple paradigms:

| Paradigm | Inspiration | KORE Implementation |
| -------- | ----------- | ------------------- |
| **Safety** | Rust | Ownership, borrowing, no null, no data races |
| **Syntax** | Python | Significant whitespace, minimal ceremony |
| **Metaprogramming** | Lisp | Code as data, hygienic macros, DSL-friendly |
| **Compile-Time** | Zig | `comptime` execution, no separate macro language |
| **Effects** | Koka/Eff | Side effects tracked in the type system |
| **Concurrency** | Erlang | Actor model with message passing |
| **UI/Components** | React/JSX | Native JSX syntax, components, hot reloading |
| **Targets** | Universal | WASM, LLVM native, SPIR-V shaders, Rust transpilation |

### Example

``` kore
// Define a function with effect tracking
fn factorial(n: Int) -> Int with Pure:
    match n:
        0 => 1
        _ => n * factorial(n - 1)

// Actors for concurrency
actor Counter:
    var count: Int = 0

    on Increment(n: Int):
        count = count + n

    on GetCount -> Int:
        return count

fn main():
    let result = factorial(5)
    println("5! = " + str(result))
```

- - -

## Quick Start

```
╔═════════════════════════════════════════╗
║                                         ║
║   ██╗  ██╗ ██████╗ ██████╗ ███████╗     ║
║   ██║ ██╔╝██╔═══██╗██╔══██╗██╔════╝     ║
║   █████╔╝ ██║   ██║██████╔╝█████╗       ║
║   ██╔═██╗ ██║   ██║██╔══██╗██╔══╝       ║
║   ██║  ██╗╚██████╔╝██║  ██║███████╗     ║
║   ╚═╝  ╚═╝ ╚═════╝ ╚═╝  ╚═╝╚══════╝     ║
║                                         ║
║                                         ║
╚═════════════════════════════════════════╝
```

### Install V1 (Production)

```bash
cargo install kore-lang
```

Done. The `kore` command is now available. Use this for WASM, shaders, and production work.

### Build V2 (Self-Hosting, Experimental)

For compiler development or contributing to the self-hosting effort:

``` powershell
git clone https://github.com/ephemara/Kore-lang.git
cd Kore

# Build native compiler
.\build.ps1 -SkipSelfHosted

# Compile a KORE file
.\build\artifacts\latest\kore_native.exe examples/hello.kr -o hello.ll

# Link with runtime and execute
clang hello.ll build\kore_runtime.o -o hello.exe
.\hello.exe
```

### Prerequisites (V2 only)

* **Windows**: PowerShell 5.1+, Clang/LLVM 17+
* **Rust**: 1.70+ (for bootstrap compiler)

- - -

## Features

### Language Features

* **Type Inference** \- Hindley\-Milner style with effect tracking
* **Pattern Matching** \- Exhaustive checking with destructuring
* **Generics** \- Monomorphized at compile time
* **Actors** \- Message\-passing concurrency \(Erlang\-style\)
* **Effects** - `Pure`, `IO`, `Async` tracked in types
* **Macros** \- Hygienic compile\-time code generation

### Compilation Targets

| Target | Status | Output |
| ------ | ------ | ------ |
| **LLVM IR** | Stable | Native executables via Clang |
| **WASM** | V1 Stable | WebAssembly modules (see kore-v1-stable/) |
| **SPIR-V** | V1 Stable | GPU shader bytecode (see kore-v1-stable/) |
| **Rust** | Bootstrap | Transpiled Rust source |

### Unreal Engine 5 Integration

KORE features a specialized `ue5-shader` pipeline that compiles KORE source directly into validated SPIR-V and transpiled HLSL/USF files, ready for seamless use in UE5 plugins.

KORE was born from a love for Unreal Engine - it's the foundation that made this language possible. The production-ready V1 compiler with full UE5 shader support is available in `/kore-v1-stable/`.

### Current Limitations

* **Generics**: Supported in native compiler's `parser_v2`, not in bootstrap
* **IR Verification**: Occasional failures under edge cases
* **Self-Hosting**: Stage 2 (self-compiled) is experimental

- - -

## Architecture

KORE uses a **three-stage bootstrap architecture**:

```
┌─────────────────────────────────────────────────────────────────┐
│                     KORE COMPILER PIPELINE                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐       │
│  │   STAGE 0    │    │   STAGE 1    │    │   STAGE 2    │       │
│  │  Bootstrap   │───>│    Native    │───>│ Self-Hosted  │       │
│  │   (Rust)     │    │  (kore.exe)  │    │  (kore_v2)   │       │
│  └──────────────┘    └──────────────┘    └──────────────┘       │
│        │                    │                    │              │
│        v                    v                    v              │
│   Rust/Inkwell         LLVM IR via          LLVM IR via         │
│   LLVM bindings        NaN-boxing           self-compiled       │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

| Stage | Location | Technology | Purpose |
| ----- | -------- | ---------- | ------- |
| **Stage 0** | `bootstrap/` | Rust + Inkwell (LLVM 21) | Initial compiler, fallback |
| **Stage 1** | `build/artifacts/latest/kore_native.exe` | KORE source → LLVM IR | Primary development compiler |
| **Stage 2** | `build/artifacts/latest/kore_native_v2.exe` | Stage 1 compiles itself | Validation target |

### Project Structure

```

├── src/                    # KORE compiler source (written in KORE)
│   ├── korec.kr            # Compiler entry point & CLI
│   ├── lexer.kr            # Tokenizer (23KB)
│   ├── parser_v2.kr        # Parser with generics (54KB)
│   ├── types.kr            # Type checker (66KB)
│   ├── codegen.kr          # LLVM IR generator (86KB)
│   ├── ast.kr              # AST definitions
│   ├── resolver.kr         # Import resolution
│   ├── diagnostic.kr       # Error formatting
│   ├── effects.kr          # Effect system
│   ├── span.kr             # Source locations
│   └── stdlib.kr           # Standard library bindings
│
├── bootstrap/              # Stage 0: Rust bootstrap compiler
│   ├── Cargo.toml          # Inkwell 0.7.1 / LLVM 21
│   └── src/                # Rust implementation
│
├── runtime/                # C runtime library
│   ├── kore_runtime.c      # NaN-boxing runtime (65KB)
│   └── kore_runtime.o      # Compiled object file
│
├── build/                  # Build artifacts (gitignored)
│   ├── artifacts/          # Timestamped builds
│   │   ├── latest/         # Symlink to newest build
│   │   └── YYYYMMDD_HHMMSS/
│   └── logs/               # Build logs
│
├── tests/                  # Test suite
│   ├── unit/               # Unit tests
│   └── examples/           # Demo programs
│
├── not_yet_implemented/    # Experimental features (~9,000 lines)
│   ├── monomorphize.kr     # Generics instantiation (1,315 lines)
│   ├── wasm.kr             # WebAssembly codegen (1,213 lines)
│   ├── runtime.kr          # Interpreter with actors (1,291 lines)
│   ├── spirv.kr            # GPU shader codegen (1,075 lines)
│   ├── lsp.kr              # Language Server Protocol (994 lines)
│   ├── formatter.kr        # Code formatter (751 lines)
│   ├── comptime.kr         # Compile-time evaluation (382 lines)
│   ├── repl.kr             # Interactive REPL (432 lines)
│   ├── test_runner.kr      # Test discovery/runner (432 lines)
│   ├── packager.kr         # Package manager (422 lines)
│   ├── suggestions.kr      # Error recovery suggestions (388 lines)
│   └── import_resolver.kr  # Module resolution (365 lines)
│
├── kore-v1-stable/         # V1 Production Compiler (WASM/SPIR-V/Rust/Actors)
├── docs/                   # Documentation
├── stdlib/                 # Standard library (KORE source)
└── scripts/                # Development utilities
```

- - -

## Building

KORE supports both **Windows** (PowerShell) and **Linux/macOS** (Bash) build systems.

- - -

### Linux/macOS Build System (`build.sh`)

> **610 lines** of a complete cross-platform build system with colored output, artifact management, and comprehensive build lifecycle.

#### Quick Reference

``` bash
# Full build (bootstrap + native)
./build.sh

# Build specific targets
./build.sh bootstrap    # Stage 0: Rust compiler
./build.sh native       # Stage 1: Native KORE compiler
./build.sh runtime      # C runtime only
./build.sh test         # Run test suite
./build.sh clean        # Clean artifacts
./build.sh clean-all    # Clean everything (including bootstrap)
```

#### Environment Variables

| Variable | Description |
| -------- | ----------- |
| `DEBUG=1` | Enable debug output |
| `FORCE_RUNTIME=1` | Force runtime rebuild |
| `SKIP_RUNTIME=1` | Skip runtime build |
| `KEEP_HISTORY=N` | Keep last N builds (default: 3) |

#### Architecture

```
build.sh (610 lines)
├─ Colors & Logging (29-48)     # ANSI colors, log_ok/info/warn/err/step/debug
├─ Configuration (50-98)        # Paths, source file list in dependency order
├─ Prerequisites (137-161)      # Check clang, cargo, runtime
├─ Runtime Build (167-193)      # Compile kore_runtime.c → kore_runtime.o
├─ Bootstrap Build (199-218)    # Build Rust compiler with cargo
├─ Source Combiner (224-280)    # Merge KORE files, filter duplicate imports
├─ Bootstrap Compile (286-305)  # Generate LLVM IR from combined source
├─ LLVM IR Fixer (311-342)      # Patch missing declarations with sed/awk
├─ Linker (348-378)             # clang + runtime → executable
├─ Native Build (384-422)       # Full native compiler pipeline
├─ Test Runner (428-489)        # Compile/link/run tests, report pass/fail
├─ Cleanup (495-512)            # Artifact removal
├─ Help (518-547)               # Usage documentation
└─ Main (553-609)               # Target dispatch + timing
```

#### Key Features

**Colored Logging** (lines 29-48):

``` bash
log_ok()    { echo -e "${GREEN}[OK]${NC} $1"; }
log_info()  { echo -e "${CYAN}>>${NC} $1"; }
log_warn()  { echo -e "${YELLOW}[!]${NC} $1"; }
log_err()   { echo -e "${RED}[X]${NC} $1"; }
log_step()  { echo -e "${MAGENTA}==>${NC} ${BOLD}$1${NC}"; }
```

**Dependency-Ordered Source Files** (lines 85-98):

``` bash
CORE_SOURCES=(
    "src/span.kr"       # Source locations
    "src/error.kr"      # Error types
    "src/effects.kr"    # Effect system
    "src/ast.kr"        # AST definitions
    "src/stdlib.kr"     # Standard library
    "src/types.kr"      # Type checker
    "src/lexer.kr"      # Tokenizer
    "src/parser_v2.kr"  # Parser
    "src/diagnostic.kr" # Error formatting
    "src/resolver.kr"   # Import resolution
    "src/codegen.kr"    # LLVM IR generator
    "src/korec.kr"      # CLI entry point
)
```

**Artifact Management** (lines 116-131):

* Timestamped build folders (`YYYYMMDD_HHMMSS/`)
* `latest` symlink for easy access
* Automatic cleanup of old builds (`KEEP_HISTORY`)

**LLVM IR Fixer** (lines 311-342):

``` bash
# Adds missing runtime type definitions
awk '
/target triple/ {
    print
    print "%StringArray = type { i8**, i64, i64 }"
    print "declare i64 @char_code_at(i64, i64)"
    # ...
}
' "$input_ll" > "$output_ll"
```

- - -

### Windows Build System (`build.ps1`)

The build system handles the full compilation pipeline:

``` powershell
# Default: Build native + self-hosted compilers
.\build.ps1

# Build native only (faster, recommended for development)
.\build.ps1 -SkipSelfHosted

# Build specific target
.\build.ps1 -Target native      # Build Stage 1 native compiler
.\build.ps1 -Target bootstrap   # Build Stage 0 Rust compiler
.\build.ps1 -Target self        # Build Stage 2 self-hosted
.\build.ps1 -Target test        # Run test suite
.\build.ps1 -Target runtime     # Compile C runtime only
.\build.ps1 -Target combine     # Combine sources into single file

# Maintenance
.\build.ps1 -Clean              # Clean current build artifacts
.\build.ps1 -CleanAll           # Clean ALL builds including bootstrap
```

### Build Flags

| Flag | Description |
| ---- | ----------- |
| `-Target <name>` | Build target: `all`, `bootstrap`, `native`, `self`, `test`, `runtime`, `combine` |
| `-SkipSelfHosted` | Skip Stage 2 build (recommended for development) |
| `-ForceRuntimeRebuild` | Force recompilation of C runtime |
| `-Clean` | Clean current build artifacts |
| `-CleanAll` | Clean ALL builds including bootstrap |
| `-Debug` | Enable debug output |
| `-Verbose` | Verbose logging |
| `-Verify` | Require LLVM IR verification to pass |
| `-EnableASAN` | Build with Address Sanitizer |
| `-RunSmokeTests` | Run smoke tests after native build |
| `-KeepHistory N` | Keep last N builds (default: 3) |

### Build Outputs

| File | Description |
| ---- | ----------- |
| `build/artifacts/latest/kore_native.exe` | Stage 1 native compiler |
| `build/artifacts/latest/kore_native_v2.exe` | Stage 2 self-hosted compiler |
| `build/kore_runtime.o` | Compiled C runtime |
| `build/korec_build.kr` | Combined source file |

- - -

## CLI Reference

### Compiler (`korec` / `kore_native.exe`)

``` bash
korec <input.kr> [OPTIONS]

OPTIONS:
    -o <FILE>           Output file path (default: input.ll)
    --target <TARGET>   Compilation target:
                        - llvm  : LLVM IR (default)
                        - rust  : Rust source code
                        - wasm  : WebAssembly
    -v, --verbose       Verbose output
    -s, --stats         Show compilation statistics
    -h, --help          Show help
```

### Compilation Workflow

``` powershell
# Step 1: Compile KORE to LLVM IR
.\build\artifacts\latest\kore_native.exe program.kr -o program.ll

# Step 2: Link with runtime
clang program.ll build\kore_runtime.o -o program.exe

# Step 3: Run
.\program.exe
```

### Output Markers

The compiler emits machine-parseable output for build system integration:

| Marker | Meaning |
| ------ | ------- |
| `[PHASE:X] START` | Phase X beginning |
| `[PHASE:X] DONE` | Phase X completed |
| `[STAT:NAME] value` | Compilation statistic |
| `[ERROR:PHASE] msg` | Error in phase |
| `[COMPILER:COMPLETE]` | Successful completion |
| `[RESULT:SUCCESS/FAILURE]` | Final status |

- - -

## Runtime

### The C Runtime (`runtime/kore_runtime.c`)

> **1,897 lines** (65KB) of C code that provides the execution environment for compiled KORE programs.

This is the glue between LLVM IR and the operating system. Every compiled KORE program links against this runtime.

#### NaN-Boxing (IEEE 754 Exploitation)

All KORE values fit in 64 bits using **NaN-boxing** \- exploiting unused bit patterns in IEEE 754 doubles:

``` c
// Quiet NaN prefix - values >= this are tagged, not doubles
#define NANBOX_QNAN     0xFFF8000000000000ULL

// Bit layout:
//   Float:  Any value < NANBOX_QNAN is a valid IEEE 754 double
//   Tagged: [0xFFF8 prefix (16 bits)][tag (3 bits)][payload (45 bits)]

// Type tags (3 bits, stored in bits 45-47)
#define KORE_TAG_PTR    0  // Heap pointer (45-bit = 32TB address space)
#define KORE_TAG_INT    1  // Signed 45-bit integer (±17.5 trillion)
#define KORE_TAG_BOOL   2  // Boolean (payload = 0 or 1)
#define KORE_TAG_NULL   3  // Null/Unit
#define KORE_TAG_STR    4  // String pointer (quick type checks)

// Sentinel values
#define KORE_NULL   (NANBOX_QNAN | (KORE_TAG_NULL << 45))
#define KORE_TRUE   (NANBOX_QNAN | (KORE_TAG_BOOL << 45) | 1)
#define KORE_FALSE  (NANBOX_QNAN | (KORE_TAG_BOOL << 45) | 0)
```

#### Architecture

```
kore_runtime.c (1,897 lines)
├─ NaN-Boxing System (17-230)       # Type tags, boxing, unboxing, type checks
├─ Memory Management (280-328)      # Arena allocator, 1MB pages, 16GB limit
├─ Print Functions (250-277)        # kore_print_str, kore_println_str
├─ String Operations (330-511)      # concat, starts_with, replace, eq, len
├─ Arithmetic Operators (515-696)   # add/sub/mul/div with auto-type dispatch
├─ Comparison Operators (698-765)   # lt/gt/le/ge/eq/neq with unboxing
├─ Array Operations (807-980)       # new, push, pop, get, set, len
├─ Helper Functions (982-1235)      # split, join, range, substring, slice
├─ Option/Box Types (1238-1302)     # Some, None, unwrap, box, unbox
├─ File I/O (1337-1388)             # read, write, exists
├─ Map Operations (1390-1507)       # parallel-array key-value store
├─ Variant Introspection (1519-1587)# variant_of, variant_field for enums
├─ System Functions (1589-1607)     # system(), exit(), panic()
├─ Stdlib Wrappers (1609-1754)      # 40+ function aliases for linking
├─ Stack Trace Support (1779-1825)  # kore_trace_enter/exit, print_stack_trace
└─ Main Entry Point (1827-1832)     # Sets up args, calls main_kore()
```

#### Key Runtime Functions

| Category | Functions | Lines |
| -------- | --------- | ----- |
| **Boxing** | `kore_box_int/string/ptr/bool/null` | 120-146 |
| **Unboxing** | `kore_unbox_int/string/ptr/bool` | 150-204 |
| **Type Checks** | `kore_is_int/string/ptr/bool/null/truthy` | 72-229 |
| **Strings** | `kore_str_concat`, `kore_str_eq`, `kore_str_len`, `kore_substring` | 330-414 |
| **Arrays** | `kore_array_new`, `kore_array_push/pop/get/set/len` | 807-980 |
| **Maps** | `Map_new`, `kore_map_get/set`, `kore_contains_key` | 1400-1507 |
| **Options** | `kore_some`, `kore_none`, `kore_unwrap` | 1251-1290 |
| **Files** | `kore_file_read`, `kore_file_write`, `file_exists` | 1341-1388 |
| **Variants** | `kore_variant_of`, `kore_variant_field` | 1524-1587 |
| **Debug** | `kore_print_stack_trace`, `kore_debug_log_var` | 1812-1849 |

#### V1/V2 Compatibility Layer

The runtime includes extensive **transition hacks** to support both raw integers (V1 codegen) and NaN-boxed values (V2 codegen):

``` c
static inline uint64_t kore_get_tag(uint64_t v) {
    if (v == 0) return KORE_TAG_NULL;
    if (v < NANBOX_QNAN) {
        // TRANSITION HACK: Small values are raw integers from V1 compiler
        if (v < 0x0010000000000000ULL) return KORE_TAG_INT;
        return (uint64_t)-1;  // -1 = double
    }
    return (v >> 45) & 0x7;
}
```

#### Stack Trace Support (lines 1779-1825)

``` c
void kore_print_stack_trace(void) {
    fprintf(stderr, "\n\033[1;36mStack trace (most recent call last):\033[0m\n");
    for (int i = g_stack_depth - 1; i >= 0; i--) {
        fprintf(stderr, "  at %s (%s:%d)\n",
            g_stack_frames[i].function_name,
            g_stack_frames[i].file,
            g_stack_frames[i].line);
    }
}
```

#### Usage

``` bash
# Compile runtime (done by build system)
clang -c runtime/kore_runtime.c -o build/kore_runtime.o -O2

# Link with compiled KORE program
clang program.ll build/kore_runtime.o -o program.exe
```

- - -

KORE uses a **NaN-boxing** runtime where all values fit in 64 bits:

| Type | Representation |
| ---- | -------------- |
| Float | Raw IEEE 754 double |
| Int | Tagged 45-bit integer |
| Bool | Tagged boolean |
| String | Tagged pointer |
| Array | Tagged pointer |
| Struct | Tagged pointer |

### Runtime Functions

The C runtime (`runtime/kore_runtime.c`) provides:

* **I/O**: `kore_print_str`, `kore_println_str`, `read_file`, `write_file`
* **Strings**: `kore_str_concat`, `kore_str_len`, `kore_substring`, `kore_split`, `kore_join`
* **Arrays**: `kore_array_new`, `kore_array_push`, `kore_array_get`, `kore_array_len`
* **Maps**: `Map_new`, `kore_map_get`, `kore_map_set`
* **Conversions**: `kore_to_string`, `kore_to_int`, `kore_to_float`
* **Introspection**: `kore_variant_of`, `kore_variant_field`

- - -

## Development Status

| Component | Status | Notes |
| --------- | ------ | ----- |
| Lexer | Stable | Full token coverage |
| Parser (v2) | Stable | Generics support |
| Type Checker | Stable | Effect inference |
| LLVM Codegen | Working | NaN-boxing, most constructs |
| Native Build | Working | Primary development path |
| Bootstrap | Working | Fallback compiler |
| Self-Host | Experimental | Stage 2 validation |
| IR Verification | Partial | Known edge cases |

### Known Gaps

1. **IR Verification**: Occasional failures on complex control flow
2. **Bootstrap Parser**: No generics support (use native compiler)
3. **Self-Host Cycle**: Not fully validated for continuous use

### Unimplemented Features (TODOs in source)

| File | Feature | Status |
| ---- | ------- | ------ |
| `codegen.kr` | For loop codegen | Placeholder |
| `codegen.kr` | Match expression | Placeholder |
| `codegen.kr` | Break/Continue | Missing jump labels |
| `codegen.kr` | Field/Index assignment | Partial |
| `codegen.kr` | Float constants | Needs IEEE 754 handling |
| `types.kr` | Generic method calls | Not implemented |
| `types.kr` | Variant payload types | Partial |
| `korec.kr` | Path.stem() | Returns hardcoded "project" |

- - -

## Experimental Features (`not_yet_implemented/`)

The `not_yet_implemented/` folder contains **\~9,000 lines** of experimental KORE source code for upcoming features. These are fully written but not yet integrated into the main compiler pipeline.

### Feature Status

| Feature | File | Lines | Description |
| ------- | ---- | ----- | ----------- |
| **Monomorphization** | `monomorphize.kr` | 1,315 | Generics instantiation, type substitution |
| **WebAssembly** | `wasm.kr` | 1,213 | Full WASM codegen with opcodes, locals, memory |
| **Interpreter** | `runtime.kr` | 1,291 | Runtime values, actor system, native functions |
| **SPIR-V** | `spirv.kr` | 1,075 | GPU shader codegen with capabilities, types |
| **LSP Server** | `lsp.kr` | 994 | Document sync, symbol lookup, diagnostics |
| **Formatter** | `formatter.kr` | 751 | Code pretty-printing with configurable style |
| **Comptime** | `comptime.kr` | 382 | Zig-style compile-time expression evaluation |
| **REPL** | `repl.kr` | 432 | Interactive shell with history, commands |
| **Test Runner** | `test_runner.kr` | 432 | Test discovery, parallel execution, reporting |
| **Packager** | `packager.kr` | 422 | Package manifest (kore.toml), project init |
| **Suggestions** | `suggestions.kr` | 388 | Typo detection, smart error recovery |
| **Import Resolver** | `import_resolver.kr` | 365 | Module graph, cycle detection, caching |

### Key Highlights

**Monomorphization** (`monomorphize.kr`):

* Two-pass algorithm: collect generics → scan for instantiations
* Type substitution mapping
* Method registration from impl blocks
* Trait implementation tracking

**WebAssembly Codegen** (`wasm.kr`):

* Complete opcode definitions (40+ opcodes)
* Struct/enum layout computation
* Lambda and closure support
* Data section for string literals
* Component compilation (for UI)

**Interpreter Runtime** (`runtime.kr`):

* NaN-boxing value representation
* Actor system with mailboxes and message passing
* 50+ native functions (I/O, math, HTTP, JSON)
* VDOM support for JSX rendering

**SPIR-V Codegen** (`spirv.kr`):

* Full SPIR-V builder with proper section ordering
* Capabilities, execution models, decorations
* Type caching for primitives and vectors
* Shader entry point generation

**LSP Server** (`lsp.kr`):

* Document store with incremental text updates
* Symbol extraction from parsed AST
* Position ↔ offset conversion
* Go-to-definition support

**Comptime Evaluation** (`comptime.kr`):

* Zig-style `comptime` expression handling
* Value-to-literal conversion
* Recursive block and expression evaluation
* Const evaluation at compile time

- - -

## The Interpreter Runtime (`runtime.kr`)

> **1,291 lines** of a complete tree-walking interpreter that executes KORE without compilation.
A full-featured runtime with JSX rendering, an actor system, HTTP networking, and 65+ native functions.

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    KORE Interpreter Runtime                     |
├─────────────────────────────────────────────────────────────────┤
│  Value System (15 types)  │  Environment (Scope Chain)          │
│  ├── Int, Float, Bool     │  ├── Variable lookup O(1)           │
│  ├── String, Array, Map   │  ├── Lexical scoping                │
│  ├── Struct, Enum, Tuple  │  └── Closure capture                │
│  ├── Function, Lambda     │                                     |
│  ├── VNode (JSX)          │  Actor System (Erlang-style)        │
│  ├── ActorRef, Future     │  ├── spawn/send primitives          │
│  └── Poll (async)         │  └── Mailbox message queue          │
├─────────────────────────────────────────────────────────────────┤
│  Native Functions (65+)   │  FFI / External Calls               │
│  ├── I/O: print, input    │  ├── native_http_get                │
│  ├── Math: sin, cos, sqrt │  ├── native_http_post               │
│  ├── Collections: map,    │  ├── native_read_file               │
│  │   filter, fold, zip    │  └── native_write_file              │
│  └── JSON: parse, string  │                                     |
└─────────────────────────────────────────────────────────────────┘
```

### Component Breakdown

| Component | Lines | What It Does |
| --------- | ----- | ------------ |
| **Value enum** | 27-113 | 15 runtime value types: primitives, collections, functions, VNodes, actors |
| **Actor System** | 115-144 | `ActorRef`, `Message`, `ActorState` with mailbox queues |
| **Environment** | 132-186 | Scope chain with push/pop, define/assign/lookup |
| **Stdlib Registration** | 191-285 | 65+ native functions across 10 categories |
| **Interpreter Entry** | 291-336 | Two-pass execution: register definitions → call `main()` |
| **Statement Eval** | 388-491 | `for`, `while`, `break`, `continue`, `return`, pattern binding |
| **Expression Eval** | 496-681 | All expression types including match, lambda, JSX |
| **Binary/Unary Ops** | 732-832 | Full operator dispatch with type coercion |
| **Pattern Matching** | 838-868 | Wildcard, binding, literal, variant, tuple destructuring |
| **Function Calls** | 874-903 | User functions + closure invocation |
| **Native Dispatch** | 909-1165 | 50+ native functions with full implementations |
| **JSX/VDOM Rendering** | 1171-1204 | Elements, text nodes, fragments, attribute evaluation |
| **JSON Serialization** | 1210-1255 | Parse/stringify with proper escaping |
| **Extern Stubs** | 1277-1291 | Platform-specific FFI declarations |

### JSX/VDOM Support

KORE has **first-class JSX** that compiles to a virtual DOM:

``` kore
// runtime.kr lines 21-24
enum VNode:
    Element(String, Map<String, Value>, Array<VNode>)  // tag, attrs, children
    Text(String)

// Evaluated at runtime (lines 1171-1204)
fn eval_jsx(env: Env, node: JSXNode) -> Result<Value, String>:
    match node:
        JSXNode::Element(el) =>
            var attrs: Map<String, Value> = Map::new()
            for attr in el.attributes:
                attrs[attr.name] = eval_expr(env, attr.value)?
            // ... build VNode tree
```

### Actor System (Erlang-Style Concurrency)

``` kore
// runtime.kr lines 119-144
struct ActorRef:
    id: Int
    name: String

struct Message:
    handler: String
    args: Array<Value>
    reply_to: Option<ActorRef>

struct ActorState:
    def: Actor
    state: Map<String, Value>
    mailbox: Array<Message>  // Message queue
```

### Native Function Categories

| Category | Functions | Lines |
| -------- | --------- | ----- |
| **I/O** | `print`, `println`, `input` | 911-924 |
| **Type Conversion** | `str`, `int`, `float`, `bool` | 926-948 |
| **Collections** | `len`, `push`, `pop`, `map`, `filter`, `fold`, `zip` | 950-1000 |
| **Strings** | `substring`, `starts_with`, `trim`, `replace` | 1000-1030 |
| **Math** | `abs`, `min`, `max`, `sqrt`, `sin`, `cos`, `pow` | 1030-1050 |
| **Option/Result** | `Some`, `unwrap`, `is_some`, `is_none`, `Ok`, `Err` | 1050-1078 |
| **Assertions** | `assert`, `assert_eq`, `panic` | 1079-1094 |
| **HTTP** | `http_get`, `http_post_json` | 1096-1112 |
| **JSON** | `json_parse`, `json_string` | 1114-1126 |
| **File I/O** | `read_file`, `write_file`, `file_exists` | 1128-1155 |

### Retirement Criteria for Bootstrap

The Rust bootstrap compiler will be retired when:

* Native compiler consistently emits verified IR
* Full test suite passes under native toolchain
* Self-host cycles complete without manual intervention
* Default build never requires bootstrap

### For AI Developers

See [LLM\_GUIDE.md](LLM_GUIDE.md) for a hyper-optimized reference:

* Exhaustive syntax rules in \~300 lines
* Complete examples with patterns
* Side-by-side comparison with Rust/Python/TypeScript
* Designed for context window efficiency

- - -

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

### Quick Contribution Guide

1. Fork and clone the repository
2. Build with `.\build.ps1 -SkipSelfHosted`
3. Make changes in `src/*.kr`
4. Run tests with `.\build.ps1 -Target test`
5. Submit a pull request

### Priority Areas

* IR verification edge cases
* Pattern matching exhaustiveness
* Standard library expansion
* Documentation improvements

- - -

## Support & Funding

Funding for a future 3D DCC along with future development of Kore-Lang. At the moment, I am completely broke and have no funds for the infrastructure of my in progress DCC and kore-lang.

**Support via Ko-fi**: [ko-fi.com/kgreeble](https://ko-fi.com/kgreeble)

- - -

## License

MIT License - see [LICENSE](LICENSE) for details.

- - -

**Project Ouroboros**
*The snake that compiles itself*
