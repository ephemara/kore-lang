<img src="assets/KAIN_timeline_final.gif" alt="KAIN - Timeline Animation" width="1041">


<a href="#quick-start">Quick Start</a> • <a href="#features">Features</a> • <a href="#architecture">Architecture</a> • <a href="#building">Building</a> • <a href="#cli-reference">CLI</a> • <a href="#contributing">Contributing</a>

- - -

## Repository Structure

This repository contains **two compiler implementations**:

| Version | Location | Status | Best For |
|---------|----------|--------|----------|
| **V2 Self-Hosting** | `/` (root) | Experimental | Next-gen compiler development, LLVM native |
| **V1 Production** | `/KAIN-v1-stable/` | Production-Ready | WASM, SPIR-V shaders, Actor runtime, UE5 integration |

**New users**: Start with **V1** (`/KAIN-v1-stable/`) for production work, especially for shaders and WASM. (Now on crates.io! cargo install KAIN-lang@0.1.1)

**Contributors**: V2 (root) is where the self-hosting magic happens - help us make it production-ready!

**Detailed comparison**: See [WHICH_VERSION.md](WHICH_VERSION.md) for a complete feature matrix and use case guide.

- - -

## The Motivation: Project Ouroboros

You might ask: *Why another language?*

1.  **Unified Graphics Pipeline**: Authoring shaders in GLSL/HLSL and binding them to C++ logic is friction-heavy. KAIN treats shaders as first-class citizens.
2.  **Solving Architectural Glue**: I am currently building a massive, professional-grade 3D DCC suite. Think **Blender meets Houdini, ZBrush, Substance, and C4D**. It handles everything from PBR painting and GPU sculpting to real-time particle simulations with 64+ million particles. The prototype was glued together using Rust, React, Tauri, TypeScript, Python, and C++ via a complex IPC system. It became a maintenance nightmare.
3.  **The "Dogfooding" Strategy**: It turned out to be more efficient to design a language that unifies these domains (UI, Logic, GPU) than to maintain the "cobweb" of legacy stacks.

**KAIN is being built to power this DCC.** The ultimate way to stress-test a language is to build complex 3D architecture with it. I am currently porting the DCC to KAIN to ensure the language handles real-world production edge cases.

>> **Funding & Roadmap**: To support this development, I am releasing modules of the DCC as **Unreal Engine 5 Plugins** (powered by KAIN's V1 compiler).
>
>> **Status Update:** Active work on the KAIN compiler core will be minimal until these plugins are released. I am currently developing on a legacy mobile workstation (i7 9th Gen, RTX 2060, 32GB RAM) which has become a hard bottleneck; compiler memory spikes during self-hosting frequently crash the machine. **Revenue from these plugins will directly fund the hardware upgrade needed to finish Project Ouroboros.**

- - -

## The Origin

KAIN has been under active private development for years. You might ask: *Where did these ~337 files suddenly spawn from?*

The legacy repository contained sensitive personal information in the git history. For this public release, the only option was to start fresh with a clean slate. FURTHERMORE, I prefer using the classic version of git (zipping folders and using that for backup instead of github). Only recently have I started using github for version control. In the scripts folder, you can find a robust Python GUI script that handled backups for me along with dozens of other features KAIN-main\scripts\ilovebuttons.pyw. I would love if other people got the same use out of the script as I did. (IT EVEN HAS A BUILT IN TODO LIST, CLIPBOARD HISTORY, SCREENSHOT MANAGING, PRESET TERMINAL COMMANDS, AND MUCH MORE.)

Curious what this language is designed to power? Head over to **[Greeble.co](https://www.greeble.co/)**. There you can find a sample of the in-progress 3D DCC (1/14 modules) and previews of Unreal Engine 5 plugins—featuring C4D-style effectors, cloners, and MoGraph tools. Next plugin is going to be GPU based image to PBR generation. It is currently ~95% done. More info soon! 

## What is KAIN?

KAIN is a **self-hosting programming language** that combines the best ideas from multiple paradigms:

| Paradigm | Inspiration | KAIN Implementation |
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

``` KAIN
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
cargo install KAIN-lang
```

Done. The `KAIN` command is now available. Use this for WASM, shaders, and production work.

### Build V2 (Self-Hosting, Experimental)

For compiler development or contributing to the self-hosting effort:

``` powershell
git clone https://github.com/ephemara/KAIN-lang.git
cd KAIN

# Build native compiler
.\build.ps1 -SkipSelfHosted

# Compile a KAIN file
.\build\artifacts\latest\KAIN_native.exe examples/hello.kn -o hello.ll

# Link with runtime and execute
clang hello.ll build\KAIN_runtime.o -o hello.exe
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
| **WASM** | V1 Stable | WebAssembly modules (see KAIN-v1-stable/) |
| **SPIR-V** | V1 Stable | GPU shader bytecode (see KAIN-v1-stable/) |
| **Rust** | Bootstrap | Transpiled Rust source |

### Unreal Engine 5 Integration

KAIN features a specialized `ue5-shader` pipeline that compiles KAIN source directly into validated SPIR-V and transpiled HLSL/USF files, ready for seamless use in UE5 plugins.

KAIN was born from a love for Unreal Engine - it's the foundation that made this language possible. The production-ready V1 compiler with full UE5 shader support is available in `/KAIN-v1-stable/`.

### Current Limitations

* **Generics**: Supported in native compiler's `parser_v2`, not in bootstrap
* **IR Verification**: Occasional failures under edge cases
* **Self-Hosting**: Stage 2 (self-compiled) is experimental

- - -

## Architecture

KAIN uses a **three-stage bootstrap architecture**:

```
┌─────────────────────────────────────────────────────────────────┐
│                     KAIN COMPILER PIPELINE                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐       │
│  │   STAGE 0    │    │   STAGE 1    │    │   STAGE 2    │       │
│  │  Bootstrap   │───>│    Native    │───>│ Self-Hosted  │       │
│  │   (Rust)     │    │  (KAIN.exe)  │    │  (KAIN_v2)   │       │
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
| **Stage 1** | `build/artifacts/latest/KAIN_native.exe` | KAIN source → LLVM IR | Primary development compiler |
| **Stage 2** | `build/artifacts/latest/KAIN_native_v2.exe` | Stage 1 compiles itself | Validation target |

### Project Structure

```
KAIN-main/
├── src/                    # KAIN compiler source (written in KAIN)
│   ├── KAINc.kn            # Compiler entry point & CLI
│   ├── lexer.kn            # Tokenizer (23KB)
│   ├── parser_v2.kn        # Parser with generics (54KB)
│   ├── types.kn            # Type checker (66KB)
│   ├── codegen.kn          # LLVM IR generator (86KB)
│   └── ...                 # AST, resolver, diagnostics, effects
│
├── bootstrap/              # Stage 0: Rust bootstrap compiler
│   └── src/                # Rust implementation
│
├── runtime/                # C runtime library
│   └── KAIN_runtime.c      # NaN-boxing runtime (65KB)
│
├── stdlib/                 # Experimental features (V2 development)
│   ├── monomorphize.kn     # Generics instantiation
│   ├── wasm.kn             # WebAssembly codegen
│   ├── runtime.kn          # Interpreter with actors
│   └── ...                 # 12 modules total
│
├── tests/                  # Test suite
│   ├── unit/               # Unit tests
│   ├── examples/           # Demo programs
│   └── whacky/             # Edge case tests
│
├── KAIN-v1-stable/         # V1 Production Compiler
│   ├── src/                # Rust compiler source
│   ├── stdlib/             # KAIN standard library
│   ├── shaders/            # GPU shader examples
│   ├── bootstrap/          # Self-hosting compiler (KAIN)
│   └── runtime/            # C FFI runtime
│
├── docs/                   # Documentation
└── scripts/                # Development utilities
```

- - -

## Building

KAIN supports both **Windows** (PowerShell) and **Linux/macOS** (Bash) build systems.

- - -

### Linux/macOS Build System (`build.sh`)

> **610 lines** of a complete cross-platform build system with colored output, artifact management, and comprehensive build lifecycle.

#### Quick Reference

``` bash
# Full build (bootstrap + native)
./build.sh

# Build specific targets
./build.sh bootstrap    # Stage 0: Rust compiler
./build.sh native       # Stage 1: Native KAIN compiler
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
├─ Runtime Build (167-193)      # Compile KAIN_runtime.c → KAIN_runtime.o
├─ Bootstrap Build (199-218)    # Build Rust compiler with cargo
├─ Source Combiner (224-280)    # Merge KAIN files, filter duplicate imports
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
    "src/span.kn"       # Source locations
    "src/error.kn"      # Error types
    "src/effects.kn"    # Effect system
    "src/ast.kn"        # AST definitions
    "src/stdlib.kn"     # Standard library
    "src/types.kn"      # Type checker
    "src/lexer.kn"      # Tokenizer
    "src/parser_v2.kn"  # Parser
    "src/diagnostic.kn" # Error formatting
    "src/resolver.kn"   # Import resolution
    "src/codegen.kn"    # LLVM IR generator
    "src/kainc.kn"      # CLI entry point
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
| `build/artifacts/latest/KAIN_native.exe` | Stage 1 native compiler |
| `build/artifacts/latest/KAIN_native_v2.exe` | Stage 2 self-hosted compiler |
| `build/KAIN_runtime.o` | Compiled C runtime |
| `build/KAINc_build.kn` | Combined source file |

- - -

## CLI Reference

### Compiler (`kainc` / `KAIN_native.exe`)

``` bash
KAINc <input.kn> [OPTIONS]

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
# Step 1: Compile KAIN to LLVM IR
.\build\artifacts\latest\KAIN_native.exe program.kn -o program.ll

# Step 2: Link with runtime
clang program.ll build\KAIN_runtime.o -o program.exe

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

### The C Runtime (`runtime/KAIN_runtime.c`)

> **1,897 lines** (65KB) of C code that provides the execution environment for compiled KAIN programs.

This is the glue between LLVM IR and the operating system. Every compiled KAIN program links against this runtime.

#### NaN-Boxing (IEEE 754 Exploitation)

All KAIN values fit in 64 bits using **NaN-boxing** \- exploiting unused bit patterns in IEEE 754 doubles:

``` c
// Quiet NaN prefix - values >= this are tagged, not doubles
#define NANBOX_QNAN     0xFFF8000000000000ULL

// Bit layout:
//   Float:  Any value < NANBOX_QNAN is a valid IEEE 754 double
//   Tagged: [0xFFF8 prefix (16 bits)][tag (3 bits)][payload (45 bits)]

// Type tags (3 bits, stored in bits 45-47)
#define KAIN_TAG_PTR    0  // Heap pointer (45-bit = 32TB address space)
#define KAIN_TAG_INT    1  // Signed 45-bit integer (±17.5 trillion)
#define KAIN_TAG_BOOL   2  // Boolean (payload = 0 or 1)
#define KAIN_TAG_NULL   3  // Null/Unit
#define KAIN_TAG_STR    4  // String pointer (quick type checks)

// Sentinel values
#define KAIN_NULL   (NANBOX_QNAN | (KAIN_TAG_NULL << 45))
#define KAIN_TRUE   (NANBOX_QNAN | (KAIN_TAG_BOOL << 45) | 1)
#define KAIN_FALSE  (NANBOX_QNAN | (KAIN_TAG_BOOL << 45) | 0)
```

#### Architecture

```
KAIN_runtime.c (1,897 lines)
├─ NaN-Boxing System (17-230)       # Type tags, boxing, unboxing, type checks
├─ Memory Management (280-328)      # Arena allocator, 1MB pages, 16GB limit
├─ Print Functions (250-277)        # KAIN_print_str, KAIN_println_str
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
├─ Stack Trace Support (1779-1825)  # KAIN_trace_enter/exit, print_stack_trace
└─ Main Entry Point (1827-1832)     # Sets up args, calls main_KAIN()
```

#### Key Runtime Functions

| Category | Functions | Lines |
| -------- | --------- | ----- |
| **Boxing** | `kain_box_int/string/ptr/bool/null` | 120-146 |
| **Unboxing** | `kain_unbox_int/string/ptr/bool` | 150-204 |
| **Type Checks** | `kain_is_int/string/ptr/bool/null/truthy` | 72-229 |
| **Strings** | `kain_str_concat`, `kain_str_eq`, `kain_str_len`, `kain_substring` | 330-414 |
| **Arrays** | `kain_array_new`, `kain_array_push/pop/get/set/len` | 807-980 |
| **Maps** | `Map_new`, `kain_map_get/set`, `kain_contains_key` | 1400-1507 |
| **Options** | `KAIN_some`, `KAIN_none`, `KAIN_unwrap` | 1251-1290 |
| **Files** | `KAIN_file_read`, `KAIN_file_write`, `file_exists` | 1341-1388 |
| **Variants** | `KAIN_variant_of`, `KAIN_variant_field` | 1524-1587 |
| **Debug** | `KAIN_print_stack_trace`, `KAIN_debug_log_var` | 1812-1849 |

#### V1/V2 Compatibility Layer

The runtime includes extensive **transition hacks** to support both raw integers (V1 codegen) and NaN-boxed values (V2 codegen):

``` c
static inline uint64_t kain_get_tag(uint64_t v) {
    if (v == 0) return KAIN_TAG_NULL;
    if (v < NANBOX_QNAN) {
        // TRANSITION HACK: Small values are raw integers from V1 compiler
        if (v < 0x0010000000000000ULL) return KAIN_TAG_INT;
        return (uint64_t)-1;  // -1 = double
    }
    return (v >> 45) & 0x7;
}
```

#### Stack Trace Support (lines 1779-1825)

``` c
void kain_print_stack_trace(void) {
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
clang -c runtime/KAIN_runtime.c -o build/KAIN_runtime.o -O2

# Link with compiled KAIN program
clang program.ll build/KAIN_runtime.o -o program.exe
```

- - -

KAIN uses a **NaN-boxing** runtime where all values fit in 64 bits:

| Type | Representation |
| ---- | -------------- |
| Float | Raw IEEE 754 double |
| Int | Tagged 45-bit integer |
| Bool | Tagged boolean |
| String | Tagged pointer |
| Array | Tagged pointer |
| Struct | Tagged pointer |

### Runtime Functions

The C runtime (`runtime/KAIN_runtime.c`) provides:

* **I/O**: `kain_print_str`, `kain_println_str`, `read_file`, `write_file`
* **Strings**: `kain_str_concat`, `kain_str_len`, `kain_substring`, `kain_split`, `kain_join`
* **Arrays**: `kain_array_new`, `kain_array_push`, `kain_array_get`, `kain_array_len`
* **Maps**: `Map_new`, `kain_map_get`, `kain_map_set`
* **Conversions**: `kain_to_string`, `kain_to_int`, `kain_to_float`
* **Introspection**: `kain_variant_of`, `kain_variant_field`

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
| `codegen.kn` | For loop codegen | Placeholder |
| `codegen.kn` | Match expression | Placeholder |
| `codegen.kn` | Break/Continue | Missing jump labels |
| `codegen.kn` | Field/Index assignment | Partial |
| `codegen.kn` | Float constants | Needs IEEE 754 handling |
| `types.kn` | Generic method calls | Not implemented |
| `types.kn` | Variant payload types | Partial |
| `KAINc.kn` | Path.stem() | Returns hardcoded "project" |

- - -

## Experimental Features (`stdlib/`)

The `stdlib/` folder contains **~9,000 lines** of experimental KAIN source code for upcoming features. These are fully written but not yet integrated into the main compiler pipeline.

### Feature Status

| Feature | File | Lines | Description |
| ------- | ---- | ----- | ----------- |
| **Monomorphization** | `monomorphize.kn` | 1,315 | Generics instantiation, type substitution |
| **WebAssembly** | `wasm.kn` | 1,213 | Full WASM codegen with opcodes, locals, memory |
| **Interpreter** | `runtime.kn` | 1,291 | Runtime values, actor system, native functions |
| **SPIR-V** | `spirv.kn` | 1,075 | GPU shader codegen with capabilities, types |
| **LSP Server** | `lsp.kn` | 994 | Document sync, symbol lookup, diagnostics |
| **Formatter** | `formatter.kn` | 751 | Code pretty-printing with configurable style |

**Full details**: See `stdlib/README.md`

- - -

## The Interpreter Runtime (`runtime.kn`)

> **1,291 lines** of a complete tree-walking interpreter that executes KAIN without compilation.
A full-featured runtime with JSX rendering, an actor system, HTTP networking, and 65+ native functions.

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    KAIN Interpreter Runtime                     |
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

KAIN has **first-class JSX** that compiles to a virtual DOM:

``` KAIN
// runtime.kn lines 21-24
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

``` KAIN
// runtime.kn lines 119-144
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
3. Make changes in `src/*.kn`
4. Run tests with `.\build.ps1 -Target test`
5. Submit a pull request

### Priority Areas

* IR verification edge cases
* Pattern matching exhaustiveness
* Standard library expansion
* Documentation improvements

- - -

## Support & Funding

Funding for a future 3D DCC along with future development of KAIN-Lang. At the moment, I am completely broke and have no funds for the infrastructure of my in progress DCC and KAIN-lang.

**Support via Ko-fi**: [ko-fi.com/kgreeble](https://ko-fi.com/kgreeble)

- - -

## License

MIT License - see [LICENSE](LICENSE) for details.

- - -

**Project Ouroboros**
*The snake that compiles itself*
