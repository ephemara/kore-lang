# Kore Language Checkpoint — 2026-01-17

## Current Status
- Native compiler builds and links successfully via `build.ps1 -Target native -Verify`, producing `kore_native.exe` and initiating smoke tests.
- LLVM IR emission is stabilized by writing IR before verification in `bootstrap/src/main.rs`, ensuring artifacts exist even if `module.verify()` fails.
- Opaque pointer compliance and method `self` handling are implemented in LLVM codegen; extern/builtin prototypes are declared consistently.
- Lexer v2 string tokenization fixed (passing quote to `lex_string`), eliminating hangs seen in generics-related tests.
- Bootstrap compiler remains available as a fallback; native compiler is the intended primary path.

## Known Gaps
- Occasional IR verification failures remain; need to eliminate signature and control-flow edge cases.
- Full smoke and unit suite not yet reported as green across all tests; additional runs required.
- Bootstrap Rust parser does not support generics; native `parser_v2` does. Tests using generics must compile with the native compiler.

## Build & Tooling
- Timestamped artifacts with `latest` junction for convenience under `build/artifacts`.
- `build.ps1` enhancements: verification gating, IR dedup fixes, clean and history management, runtime rebuilds, smoke test runner.

## Retirement Criteria for Bootstrap
- Native compiler consistently emits verified IR and links to a working executable.
- Full smoke + unit tests pass under native toolchain.
- Successful self-host cycles (v1→v2 and repeat) without manual intervention.
- Default build never requires bootstrap except under explicit recovery flags.

## Next Steps
- Stabilize IR verification across native builds and fix remaining call/control-flow issues.
- Run full smoke and unit suites, publish pass/fail dashboard.
- Add automated self-host validation (build v2 with v1, compare IR/prototypes).
- Flip build default to native-first; keep bootstrap behind `-Target bootstrap` or recovery.

## Summary
We are at a turning point: the native compiler is producing executables and beginning tests with improved reliability in IR emission and lexing. The bootstrapper is no longer the primary path but remains as a safety net until verification stability, full test coverage, and self-host cycles are consistently green.

