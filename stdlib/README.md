# Experimental Features (V2 Development)

This folder contains **~9,000 lines** of experimental KAIN source code for the V2 self-hosting compiler. These features are fully written but not yet integrated into the main compiler pipeline.

## Status: Work in Progress

These modules represent the **future** of KAIN but are not yet production-ready. They're part of the V2 self-hosting effort (Project Ouroboros).

## Feature Overview

| Feature | File | Lines | Status | Description |
|---------|------|-------|--------|-------------|
| **Monomorphization** | `monomorphize.kn` | 1,315 | In Progress | Generics instantiation, type substitution |
| **WebAssembly** | `wasm.kn` | 1,213 | In Progress | Full WASM codegen (V1 has stable WASM) |
| **Interpreter** | `runtime.kn` | 1,291 | In Progress | Runtime values, actor system |
| **SPIR-V** | `spirv.kn` | 1,075 | In Progress | GPU shader codegen (V1 has stable SPIR-V) |
| **LSP Server** | `lsp.kn` | 994 | In Progress | Language Server Protocol |
| **Formatter** | `formatter.kn` | 751 | In Progress | Code pretty-printing |
| **Comptime** | `comptime.kn` | 382 | In Progress | Compile-time evaluation |
| **REPL** | `repl.kn` | 432 | In Progress | Interactive shell |
| **Test Runner** | `test_runner.kn` | 432 | In Progress | Test discovery and execution |
| **Packager** | `packager.kn` | 422 | In Progress | Package management |
| **Suggestions** | `suggestions.kn` | 388 | In Progress | Error recovery suggestions |
| **Import Resolver** | `import_resolver.kn` | 365 | In Progress | Module resolution |

## Need These Features Now?

**For production use**, check out **`/kain-v1-stable/`** which has:
- Stable WASM codegen
- Stable SPIR-V shader pipeline
- Working interpreter with actor system
- LSP support
- REPL framework

## Contributing

These modules are excellent starting points for contributors! Each file is:
- Written in KAIN (self-hosting!)
- Well-structured with clear responsibilities
- Ready for integration testing

See [CONTRIBUTING.md](../CONTRIBUTING.md) for how to help integrate these features into V2.

---

**V2 Goal**: Make these experimental features production-ready in the self-hosting compiler.  
**V1 Reality**: Many of these features already work in `/kain-v1-stable/` - use that for production!
