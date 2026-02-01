#!/usr/bin/env bash
# ============================================================================
# KORE BUILD SYSTEM - Linux Edition
# ============================================================================
# Cross-platform build script for the Kore programming language
#
# Usage:
#   ./build.sh                    # Full build (bootstrap + native)
#   ./build.sh bootstrap          # Build bootstrap compiler only
#   ./build.sh native             # Build native compiler
#   ./build.sh runtime            # Build runtime only
#   ./build.sh test               # Run tests
#   ./build.sh clean              # Clean build artifacts
#   ./build.sh help               # Show this help
#
# Environment Variables:
#   DEBUG=1                       # Enable debug output
#   FORCE_RUNTIME=1               # Force runtime rebuild
#   SKIP_RUNTIME=1                # Skip runtime build
#   KEEP_HISTORY=N                # Keep last N builds (default: 3)
# ============================================================================

set -e  # Exit on error

# ============================================================================
# Colors & Formatting
# ============================================================================

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
GRAY='\033[0;90m'
NC='\033[0m' # No Color
BOLD='\033[1m'

log_ok()    { echo -e "${GREEN}[OK]${NC} $1"; }
log_info()  { echo -e "${CYAN}>>${NC} $1"; }
log_warn()  { echo -e "${YELLOW}[!]${NC} $1"; }
log_err()   { echo -e "${RED}[X]${NC} $1"; }
log_step()  { echo -e "${MAGENTA}==>${NC} ${BOLD}$1${NC}"; }
log_debug() { [[ -n "$DEBUG" ]] && echo -e "${GRAY}    [DEBUG] $1${NC}"; }

separator() {
    echo -e "${BLUE}$(printf '=%.0s' {1..70})${NC}"
}

# ============================================================================
# Configuration
# ============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$SCRIPT_DIR"

# Directories
BUILD_DIR="$ROOT/build"
ARTIFACTS_DIR="$BUILD_DIR/artifacts"
LOGS_DIR="$BUILD_DIR/logs"
BOOTSTRAP_DIR="$ROOT/bootstrap"
SRC_DIR="$ROOT/src"
RUNTIME_DIR="$ROOT/runtime"
TESTS_DIR="$ROOT/tests"

# Build timestamp
BUILD_TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BUILD_FOLDER="$ARTIFACTS_DIR/$BUILD_TIMESTAMP"

# Compiler paths
BOOTSTRAP_COMPILER="$BOOTSTRAP_DIR/target/release/korec"

# Symlinks
LATEST_BUILD="$ARTIFACTS_DIR/latest"
LATEST_NATIVE="$ARTIFACTS_DIR/latest/kore_native"

# Runtime
RUNTIME_SRC="$RUNTIME_DIR/kore_runtime.c"
RUNTIME_OBJ="$BUILD_DIR/kore_runtime.o"

# Keep history
KEEP_HISTORY="${KEEP_HISTORY:-3}"

# Source files in dependency order
CORE_SOURCES=(
    "src/span.kr"
    "src/error.kr"
    "src/effects.kr"
    "src/ast.kr"
    "src/stdlib.kr"
    "src/types.kr"
    "src/lexer.kr"
    "src/parser_v2.kr"
    "src/diagnostic.kr"
    "src/resolver.kr"
    "src/codegen.kr"
    "src/korec.kr"
)

# ============================================================================
# Utility Functions
# ============================================================================

ensure_dir() {
    [[ ! -d "$1" ]] && mkdir -p "$1" && log_debug "Created directory: $1"
    return 0
}

update_latest_link() {
    local build_folder="$1"
    rm -rf "$LATEST_BUILD" 2>/dev/null || true
    ln -sf "$build_folder" "$LATEST_BUILD"
    log_debug "Updated latest link: $LATEST_BUILD -> $build_folder"
}

cleanup_old_builds() {
    local keep="${1:-3}"
    log_step "Cleaning up old builds (keeping last $keep)..."
    
    local builds=($(ls -d "$ARTIFACTS_DIR"/[0-9]* 2>/dev/null | sort -r))
    local count=${#builds[@]}
    
    if [[ $count -gt $keep ]]; then
        local to_remove=("${builds[@]:$keep}")
        for build in "${to_remove[@]}"; do
            log_debug "Removing old build: $(basename "$build")"
            rm -rf "$build"
        done
        log_info "Removed $((count - keep)) old builds"
    fi
}

# ============================================================================
# Check Prerequisites
# ============================================================================

check_prerequisites() {
    log_step "Checking prerequisites..."
    
    # Check clang
    if ! command -v clang &> /dev/null; then
        log_err "clang not found! Install with: sudo apt install clang llvm"
        exit 1
    fi
    log_debug "Found clang: $(which clang)"
    
    # Check cargo
    if ! command -v cargo &> /dev/null; then
        log_warn "cargo not found. Install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    else
        log_debug "Found cargo: $(which cargo)"
    fi
    
    # Check runtime source
    if [[ ! -f "$RUNTIME_SRC" ]]; then
        log_err "Runtime source not found: $RUNTIME_SRC"
        exit 1
    fi
    
    log_ok "Prerequisites OK"
}

# ============================================================================
# Build Runtime
# ============================================================================

build_runtime() {
    log_step "Building C runtime..."
    ensure_dir "$BUILD_DIR"
    
    local needs_rebuild=0
    
    if [[ ! -f "$RUNTIME_OBJ" ]]; then
        needs_rebuild=1
    elif [[ "$RUNTIME_SRC" -nt "$RUNTIME_OBJ" ]]; then
        log_warn "Runtime source is newer than object - rebuilding"
        needs_rebuild=1
    elif [[ -n "$FORCE_RUNTIME" ]]; then
        log_warn "Forced runtime rebuild"
        needs_rebuild=1
    fi
    
    if [[ $needs_rebuild -eq 0 ]]; then
        log_ok "Runtime up-to-date: $RUNTIME_OBJ"
        return 0
    fi
    
    log_debug "clang -c $RUNTIME_SRC -o $RUNTIME_OBJ -O2 -Wall"
    clang -c "$RUNTIME_SRC" -o "$RUNTIME_OBJ" -O2 -Wall
    
    local size=$(stat -f%z "$RUNTIME_OBJ" 2>/dev/null || stat -c%s "$RUNTIME_OBJ")
    log_ok "Runtime compiled: $RUNTIME_OBJ ($size bytes)"
}

# ============================================================================
# Build Bootstrap Compiler
# ============================================================================

build_bootstrap() {
    log_step "Building bootstrap compiler (Rust)..."
    
    pushd "$BOOTSTRAP_DIR" > /dev/null
    
    if [[ -n "$DEBUG" ]]; then
        cargo build --release
    else
        cargo build --release 2>&1 | grep -E "(Compiling|Finished|error)" || true
    fi
    
    if [[ $? -ne 0 ]] || [[ ! -f "target/release/korec" ]]; then
        log_err "Bootstrap build failed!"
        popd > /dev/null
        exit 1
    fi
    
    popd > /dev/null
    log_ok "Bootstrap compiler built: $BOOTSTRAP_COMPILER"
}

# ============================================================================
# Combine Source Files
# ============================================================================

combine_sources() {
    local output_file="$1"
    shift
    local sources=("$@")
    
    log_step "Combining source files..."
    
    # Remove existing
    rm -f "$output_file"
    
    # Add header
    local timestamp=$(date +"%Y-%m-%d %H:%M:%S")
    cat > "$output_file" << EOF
// ============================================================================
// KORE Combined Compiler Source
// Generated by build.sh at $timestamp
// Build ID: $BUILD_TIMESTAMP
// ============================================================================
// DO NOT EDIT - This file is auto-generated
// ============================================================================
EOF
    
    # Core module names (to skip duplicate imports)
    local core_modules="span error effects ast stdlib types lexer parser parser_v2 codegen diagnostics diagnostic resolver"
    
    # Add each source file
    for source in "${sources[@]}"; do
        local full_path="$ROOT/$source"
        
        if [[ ! -f "$full_path" ]]; then
            log_err "Source file not found: $source"
            exit 1
        fi
        
        log_debug "Adding: $source"
        
        # Add separator
        echo "" >> "$output_file"
        echo "// ======== $source ========" >> "$output_file"
        echo "" >> "$output_file"
        
        # Filter out duplicate use statements for core modules
        while IFS= read -r line; do
            if [[ "$line" =~ ^[[:space:]]*use[[:space:]]+([a-zA-Z_]+) ]]; then
                local module="${BASH_REMATCH[1]}"
                if [[ " $core_modules " =~ " $module " ]]; then
                    log_debug "Skipping duplicate import: use $module"
                    continue
                fi
            fi
            echo "$line" >> "$output_file"
        done < "$full_path"
    done
    
    local size=$(stat -f%z "$output_file" 2>/dev/null || stat -c%s "$output_file")
    log_ok "Combined: $output_file ($size bytes)"
}

# ============================================================================
# Compile with Bootstrap
# ============================================================================

compile_with_bootstrap() {
    local input_file="$1"
    local output_ll="$2"
    
    log_step "Compiling with bootstrap compiler..."
    log_info "  Input:  $input_file"
    log_info "  Output: $output_ll"
    
    log_debug "$BOOTSTRAP_COMPILER $input_file -o $output_ll"
    "$BOOTSTRAP_COMPILER" "$input_file" -o "$output_ll" 2>&1 | while read -r line; do
        [[ -n "$DEBUG" ]] && echo "  $line"
    done
    
    if [[ ! -f "$output_ll" ]]; then
        log_err "Output file not created: $output_ll"
        exit 1
    fi
    
    log_ok "Generated LLVM IR: $output_ll"
}

# ============================================================================
# Fix LLVM IR (Minimal - aim to eliminate this)
# ============================================================================

fix_llvm_ir() {
    local input_ll="$1"
    local output_ll="$2"
    
    log_step "Fixing LLVM IR..."
    
    # Simple fixes using sed
    # TODO: These should be fixed in the bootstrap compiler
    sed -e '/^declare.*@char_code_at/d' \
        -e '/^declare.*@char_from_code/d' \
        -e '/^declare.*@to_float/d' \
        "$input_ll" > "${output_ll}.tmp"
    
    # Add declarations after target triple
    awk '
    /target triple/ {
        print
        print ""
        print "; Additional type definitions for Kore runtime"
        print "%StringArray = type { i8**, i64, i64 }"
        print "declare i64 @char_code_at(i64, i64)"
        print "declare i64 @char_from_code(i64)"
        print "declare i64 @to_float(i64)"
        print ""
        next
    }
    { print }
    ' "${output_ll}.tmp" > "$output_ll"
    
    rm -f "${output_ll}.tmp"
    log_ok "Fixed LLVM IR: $output_ll"
}

# ============================================================================
# Link Executable
# ============================================================================

link_executable() {
    local input_ll="$1"
    local output_exe="$2"
    
    log_step "Linking executable..."
    log_info "  Input:  $input_ll"
    log_info "  Output: $output_exe"
    
    # Ensure runtime exists
    if [[ ! -f "$RUNTIME_OBJ" ]]; then
        if [[ -n "$SKIP_RUNTIME" ]]; then
            log_err "Runtime object not found: $RUNTIME_OBJ"
            exit 1
        fi
        build_runtime
    fi
    
    log_debug "clang $input_ll $RUNTIME_OBJ -o $output_exe -O2"
    clang "$input_ll" "$RUNTIME_OBJ" -o "$output_exe" -O2 2>&1 | while read -r line; do
        [[ "$line" == *"warning"* ]] && log_debug "$line"
        [[ "$line" == *"error"* ]] && log_err "$line"
    done
    
    if [[ ! -f "$output_exe" ]]; then
        log_err "Linking failed!"
        exit 1
    fi
    
    local size=$(stat -f%z "$output_exe" 2>/dev/null || stat -c%s "$output_exe")
    log_ok "Linked: $output_exe ($size bytes)"
}

# ============================================================================
# Build Native Compiler
# ============================================================================

build_native() {
    separator
    log_info "BUILDING NATIVE KORE COMPILER"
    log_info "Build ID: $BUILD_TIMESTAMP"
    separator
    
    ensure_dir "$BUILD_FOLDER"
    ensure_dir "$LOGS_DIR"
    
    local combined_file="$BUILD_DIR/korec_build.kr"
    local ll_file="$BUILD_FOLDER/kore_native.ll"
    local fixed_ll="$BUILD_FOLDER/kore_native_fixed.ll"
    local exe_file="$BUILD_FOLDER/kore_native"
    
    # Step 1: Combine sources
    combine_sources "$combined_file" "${CORE_SOURCES[@]}"
    cp "$combined_file" "$BUILD_FOLDER/korec_build.kr"
    
    # Step 2: Compile with bootstrap
    compile_with_bootstrap "$combined_file" "$ll_file"
    
    # Step 3: Fix LLVM IR
    fix_llvm_ir "$ll_file" "$fixed_ll"
    
    # Step 4: Link
    link_executable "$fixed_ll" "$exe_file"
    
    # Update latest link
    update_latest_link "$BUILD_FOLDER"
    
    # Cleanup
    cleanup_old_builds "$KEEP_HISTORY"
    
    separator
    log_ok "NATIVE COMPILER READY"
    log_info "  Path: $exe_file"
    log_info "  Latest: $LATEST_NATIVE"
    separator
}

# ============================================================================
# Run Tests
# ============================================================================

run_tests() {
    separator
    log_info "RUNNING TESTS"
    separator
    
    local compiler=""
    if [[ -f "$LATEST_NATIVE" ]]; then
        compiler="$LATEST_NATIVE"
    elif [[ -f "$BOOTSTRAP_COMPILER" ]]; then
        compiler="$BOOTSTRAP_COMPILER"
    else
        log_err "No compiler found for testing!"
        exit 1
    fi
    
    log_info "Using compiler: $compiler"
    
    local test_dir="$BUILD_DIR/test_$BUILD_TIMESTAMP"
    ensure_dir "$test_dir"
    
    local passed=0
    local failed=0
    
    for test_file in "$TESTS_DIR"/*.kr; do
        [[ ! -f "$test_file" ]] && continue
        
        local name=$(basename "$test_file" .kr)
        echo -n "  Testing $name ... "
        
        local ll_file="$test_dir/$name.ll"
        local exe_file="$test_dir/$name"
        
        # Compile
        if ! "$compiler" "$test_file" -o "$ll_file" &>/dev/null; then
            echo -e "${RED}COMPILE FAIL${NC}"
            ((failed++))
            continue
        fi
        
        # Link
        if ! clang "$ll_file" "$RUNTIME_OBJ" -o "$exe_file" &>/dev/null; then
            echo -e "${RED}LINK FAIL${NC}"
            ((failed++))
            continue
        fi
        
        # Run
        if "$exe_file" &>/dev/null; then
            echo -e "${GREEN}PASS${NC}"
            ((passed++))
        else
            echo -e "${RED}RUNTIME FAIL${NC}"
            ((failed++))
        fi
    done
    
    separator
    log_info "Results: ${GREEN}$passed passed${NC}, ${RED}$failed failed${NC}"
    separator
    
    return $failed
}

# ============================================================================
# Clean
# ============================================================================

do_clean() {
    log_step "Cleaning build artifacts..."
    
    rm -rf "$ARTIFACTS_DIR" 2>/dev/null || true
    rm -f "$RUNTIME_OBJ" 2>/dev/null || true
    rm -f "$BUILD_DIR/korec_build.kr" 2>/dev/null || true
    
    log_ok "Clean complete"
}

do_clean_all() {
    log_step "Cleaning ALL build artifacts..."
    
    rm -rf "$BUILD_DIR" 2>/dev/null || true
    rm -rf "$BOOTSTRAP_DIR/target" 2>/dev/null || true
    
    log_ok "Full clean complete"
}

# ============================================================================
# Help
# ============================================================================

show_help() {
    cat << EOF
${BOLD}KORE Build System - Linux Edition${NC}

${CYAN}Usage:${NC}
  ./build.sh [target]

${CYAN}Targets:${NC}
  all         Full build (bootstrap + native) [default]
  bootstrap   Build bootstrap compiler (Rust) only
  native      Build native Kore compiler
  runtime     Build runtime library only
  test        Run test suite
  clean       Clean build artifacts
  clean-all   Clean ALL artifacts (including bootstrap)
  help        Show this help

${CYAN}Environment Variables:${NC}
  DEBUG=1           Enable debug output
  FORCE_RUNTIME=1   Force runtime rebuild
  SKIP_RUNTIME=1    Skip runtime build
  KEEP_HISTORY=N    Keep last N builds (default: 3)

${CYAN}Examples:${NC}
  ./build.sh                     # Full build
  DEBUG=1 ./build.sh native      # Native build with debug output
  ./build.sh test                # Run tests

EOF
}

# ============================================================================
# Main
# ============================================================================

main() {
    local target="${1:-all}"
    
    cd "$ROOT"
    
    case "$target" in
        all)
            check_prerequisites
            build_runtime
            build_bootstrap
            build_native
            ;;
        bootstrap)
            check_prerequisites
            build_bootstrap
            ;;
        native)
            check_prerequisites
            build_runtime
            build_native
            ;;
        runtime)
            check_prerequisites
            build_runtime
            ;;
        test)
            check_prerequisites
            build_runtime
            run_tests
            ;;
        clean)
            do_clean
            ;;
        clean-all|cleanall)
            do_clean_all
            ;;
        help|--help|-h)
            show_help
            ;;
        *)
            log_err "Unknown target: $target"
            show_help
            exit 1
            ;;
    esac
}

# Record start time
START_TIME=$(date +%s)

main "$@"

# Show elapsed time
END_TIME=$(date +%s)
ELAPSED=$((END_TIME - START_TIME))
echo ""
log_ok "Build completed in ${ELAPSED}s"
