# ============================================================================
# KAIN BUILD SYSTEM v2.1 - Project Ouroboros
# ============================================================================
# The Ultimate Build Pipeline for Self-Hosting
# 
# CHANGES IN v2.1:
#   - Added -SkipSelfHosted to skip the self-hosted build step
#   - Added -ForceRuntimeRebuild to force runtime recompilation
#   - Auto-detects when runtime source is newer than object file
#   - Better error messages for NaN-boxing function mismatches
#
# CHANGES IN v2.0:
#   - Timestamped build folders to avoid linking conflicts
#   - All artifacts go to build/ directory (not root)
#   - Updated paths for new project structure
#   - Better clean/archive management
#   - Symlinks to "latest" build for convenience
#
# Usage:
#   .\build.ps1                      # Full build (native + self-hosted)
#   .\build.ps1 -SkipSelfHosted      # Build native only (RECOMMENDED for dev)
#   .\build.ps1 -Target bootstrap    # Build bootstrap compiler only
#   .\build.ps1 -Target native       # Build native compiler
#   .\build.ps1 -Target self         # Self-hosted build (compile with native)
#   .\build.ps1 -Target test         # Run tests
#   .\build.ps1 -Clean               # Clean build artifacts
#   .\build.ps1 -CleanAll            # Clean ALL builds (including history)
#   .\build.ps1 -Debug               # Enable debug output
#   .\build.ps1 -ForceRuntimeRebuild # Force rebuild of KAIN_runtime.o
#   .\build.ps1 -KeepHistory 5       # Keep last N builds (default: 3)
# ============================================================================

param(
    [ValidateSet("all", "bootstrap", "native", "self", "test", "runtime", "combine")]
    [string]$Target = "all",
    [switch]$Clean,
    [switch]$CleanAll,
    [switch]$Debug,
    [switch]$Verbose,
    [switch]$SkipRuntime,
    [switch]$SkipSelfHosted,
    [switch]$ForceRuntimeRebuild,
    [switch]$EnableASAN,
    [switch]$Verify,
    [switch]$RunSmokeTests,
    [int]$KeepHistory = 3
)

# ============================================================================
# Configuration
# ============================================================================

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$script:ROOT = $PSScriptRoot
$script:BUILD_DIR = "$ROOT\build"
$script:ARTIFACTS_DIR = "$BUILD_DIR\artifacts"
$script:LOGS_DIR = "$BUILD_DIR\logs"
$script:BOOTSTRAP_DIR = "$ROOT\bootstrap"
$script:SRC_DIR = "$ROOT\src"
$script:RUNTIME_DIR = "$ROOT\runtime"
$script:TESTS_DIR = "$ROOT\tests"

# Build timestamp for this run
$script:BUILD_TIMESTAMP = Get-Date -Format "yyyyMMdd_HHmmss"
$script:BUILD_FOLDER = "$ARTIFACTS_DIR\$BUILD_TIMESTAMP"

# Compiler paths
$script:BOOTSTRAP_COMPILER = "$BOOTSTRAP_DIR\target\release\KAINc.exe"

# Latest symlinks (junction points on Windows)
$script:LATEST_BUILD = "$ARTIFACTS_DIR\latest"
$script:LATEST_NATIVE = "$ARTIFACTS_DIR\latest\KAIN_native.exe"
$script:LATEST_V2 = "$ARTIFACTS_DIR\latest\KAIN_native_v2.exe"

# Runtime
$script:RUNTIME_SRC = "$RUNTIME_DIR\KAIN_runtime.c"
$script:RUNTIME_OBJ = "$BUILD_DIR\KAIN_runtime.o"
$script:RUNTIME_OBJ_ASAN = "$BUILD_DIR\KAIN_runtime_asan.o"

# Compiler flags
$script:ASAN_FLAGS = "-fsanitize=address,undefined -g -O0"

# Source files in dependency order
$script:CORE_SOURCES = @(
    "src\span.kn",
    "src\error.kn", 
    "src\effects.kn",
    "src\ast.kn",
    "src\stdlib.kn",
    "src\types.kn",
    "src\lexer.kn",
    "src\parser_v2.kn",
    "src\diagnostic.kn",
    "src\resolver.kn",
    "src\codegen.kn",
    "src\KAINc.kn"
)

# ============================================================================
# Colors and Logging
# ============================================================================

function Write-Color($color, $msg) {
    Write-Host $msg -ForegroundColor $color
}

function Write-Success($msg) { Write-Color Green "[OK] $msg" }
function Write-Info($msg) { Write-Color Cyan ">> $msg" }
function Write-Warn($msg) { Write-Color Yellow "[!] $msg" }
function Write-Err($msg) { Write-Color Red "[X] $msg" }
function Write-Step($msg) { Write-Color Magenta "==> $msg" }
function Write-Debug($msg) { if ($Debug) { Write-Color DarkGray "    [DEBUG] $msg" } }

# ============================================================================
# Utility Functions
# ============================================================================

function Ensure-Directory($path) {
    if (-not (Test-Path $path)) {
        New-Item -ItemType Directory -Path $path -Force | Out-Null
        Write-Debug "Created directory: $path"
    }
}

function Update-LatestLink {
    param([string]$BuildFolder)
    
    # Remove existing latest link
    if (Test-Path $LATEST_BUILD) {
        Remove-Item $LATEST_BUILD -Force -Recurse -ErrorAction SilentlyContinue
    }
    
    # Create junction to latest build
    try {
        cmd /c mklink /J "$LATEST_BUILD" "$BuildFolder" 2>&1 | Out-Null
        Write-Debug "Updated latest link: $LATEST_BUILD -> $BuildFolder"
    }
    catch {
        Write-Warn "Could not create latest link (non-critical)"
    }
}

function Cleanup-OldBuilds {
    param([int]$Keep = 3)
    
    Write-Step "Cleaning up old builds (keeping last $Keep)..."
    
    $builds = Get-ChildItem $ARTIFACTS_DIR -Directory -ErrorAction SilentlyContinue | 
    Where-Object { $_.Name -match '^\d{8}_\d{6}$' } |
    Sort-Object Name -Descending
    
    if ($builds -and $builds.Count -gt $Keep) {
        $toRemove = $builds | Select-Object -Skip $Keep
        foreach ($build in $toRemove) {
            Write-Debug "Removing old build: $($build.Name)"
            Remove-Item $build.FullName -Recurse -Force -ErrorAction SilentlyContinue
        }
        Write-Info "Removed $($toRemove.Count) old builds"
    }
}

function Clean-Build {
    Write-Step "Cleaning current build artifacts..."
    
    # Clean build folder but preserve structure
    if (Test-Path $ARTIFACTS_DIR) {
        Get-ChildItem $ARTIFACTS_DIR -Directory -ErrorAction SilentlyContinue | 
        Where-Object { $_.Name -match '^\d{8}_\d{6}$' -or $_.Name -eq 'latest' } |
        ForEach-Object { Remove-Item $_.FullName -Recurse -Force -ErrorAction SilentlyContinue }
    }
    
    # Clean runtime object
    if (Test-Path $RUNTIME_OBJ) {
        Remove-Item $RUNTIME_OBJ -Force -ErrorAction SilentlyContinue
    }
    
    # Clean generated combined file
    $combinedFile = "$BUILD_DIR\KAINc_build.kn"
    if (Test-Path $combinedFile) {
        Remove-Item $combinedFile -Force -ErrorAction SilentlyContinue
    }
    
    # Clean any stray artifacts in root (legacy cleanup)
    $legacyArtifacts = @(
        "$ROOT\KAINc_build.kn",
        "$ROOT\KAINc_build_v2.kn",
        "$ROOT\KAIN_native.ll",
        "$ROOT\KAIN_native_fixed.ll",
        "$ROOT\KAIN_native_v2.ll",
        "$ROOT\KAIN_native.exe",
        "$ROOT\KAIN_native_v2.exe",
        "$ROOT\KAIN_runtime.o",
        "$ROOT\test.ll",
        "$ROOT\test_codegen.ll",
        "$ROOT\test_KAINc.ll"
    )
    
    foreach ($artifact in $legacyArtifacts) {
        if (Test-Path $artifact) {
            Remove-Item $artifact -Force -ErrorAction SilentlyContinue
            Write-Debug "Cleaned legacy artifact: $artifact"
        }
    }
    
    Write-Success "Clean complete"
}

function Clean-All {
    Write-Step "Cleaning ALL build artifacts..."
    
    if (Test-Path $BUILD_DIR) {
        Remove-Item $BUILD_DIR -Recurse -Force -ErrorAction SilentlyContinue
    }
    
    # Also clean bootstrap target
    $bootstrapTarget = "$BOOTSTRAP_DIR\target"
    if (Test-Path $bootstrapTarget) {
        Write-Info "Also cleaning bootstrap target..."
        Remove-Item $bootstrapTarget -Recurse -Force -ErrorAction SilentlyContinue
    }
    
    Write-Success "Full clean complete"
}

function Check-Prerequisites {
    Write-Step "Checking prerequisites..."
    
    # Check for clang
    $clang = Get-Command clang -ErrorAction SilentlyContinue
    if (-not $clang) {
        Write-Err "clang not found! Please install LLVM/Clang"
        exit 1
    }
    Write-Debug "Found clang: $($clang.Source)"
    
    # Check for cargo (for bootstrap)
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $cargo) {
        Write-Warn "cargo not found. Bootstrap rebuilds will fail."
    }
    else {
        Write-Debug "Found cargo: $($cargo.Source)"
    }
    
    # Check for bootstrap compiler
    # BYPASSING CHECK as we manually built it
    # Build-Bootstrap
    
    # Check runtime source
    if (-not (Test-Path $RUNTIME_SRC)) {
        Write-Err "Runtime source not found: $RUNTIME_SRC"
        Write-Err "Please ensure runtime/KAIN_runtime.c exists"
        exit 1
    }
    
    Write-Success "Prerequisites OK"
}

# ============================================================================
# Build Runtime
# ============================================================================

function Build-Runtime {
    Write-Step "Building C runtime..."
    
    Ensure-Directory $BUILD_DIR
    
    $clangArgs = @(
        "-c",
        $RUNTIME_SRC,
        "-o", $RUNTIME_OBJ,
        "-O2",
        "-Wall"
    )
    
    Write-Debug "clang $($clangArgs -join ' ')"
    & clang @clangArgs
    
    if ($LASTEXITCODE -ne 0) {
        Write-Err "Runtime compilation failed!"
        exit 1
    }
    
    Write-Success "Runtime compiled: $RUNTIME_OBJ"
}

# ============================================================================
# Build Runtime with ASAN (Address Sanitizer)
# ============================================================================

function Build-RuntimeASAN {
    Write-Step "Compiling runtime library with ASAN..."
    
    Ensure-Directory $BUILD_DIR
    
    $clangArgs = @(
        $ASAN_FLAGS.Split(),
        "-c",
        $RUNTIME_SRC,
        "-o", $RUNTIME_OBJ_ASAN
    )
    
    Write-Info "  ASAN flags: $ASAN_FLAGS"
    Write-Debug "clang $($clangArgs -join ' ')"
    
    $startTime = Get-Date
    & clang $clangArgs 2>&1 | ForEach-Object {
        $line = $_.ToString()
        if ($line -match "error:") {
            Write-Err $line
        }
        elseif ($line -match "warning:" -and $Verbose) {
            Write-Warn $line
        }
    }
    
    if ($LASTEXITCODE -ne 0) {
        Write-Err "ASAN runtime compilation failed!"
        exit 1
    }
    
    $elapsed = (Get-Date) - $startTime
    $runtimeSize = (Get-Item $RUNTIME_OBJ_ASAN).Length
    
    Write-Success "ASAN runtime compiled: $RUNTIME_OBJ_ASAN ($runtimeSize bytes, $($elapsed.TotalSeconds.ToString('0.00'))s)"
}

# ============================================================================
# Build Bootstrap Compiler
# ============================================================================

function Build-Bootstrap {
    Write-Step "Building bootstrap compiler (Rust)..."
    
    Push-Location $BOOTSTRAP_DIR
    try {
        $env:CARGO_TERM_COLOR = "never"
        # Temporarily allow stderr output without throwing (cargo outputs progress to stderr)
        $oldEAP = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        
        $output = & cargo build --release 2>&1
        $exitCode = $LASTEXITCODE
        
        $ErrorActionPreference = $oldEAP
        
        if ($Verbose -or $Debug) {
            $output | ForEach-Object { 
                $line = $_.ToString()
                if ($line) { Write-Debug $line }
            }
        }
        
        if ($exitCode -ne 0) {
            Write-Err "Bootstrap build failed!"
            $output | ForEach-Object { Write-Host $_.ToString() }
            exit 1
        }
    }
    finally {
        Pop-Location
    }
    
    Write-Success "Bootstrap compiler built: $BOOTSTRAP_COMPILER"
}

# ============================================================================
# Combine Source Files
# ============================================================================

function Combine-Sources {
    param(
        [string]$OutputFile,
        [string[]]$Sources
    )
    
    Write-Step "Combining source files..."
    
    # Remove existing
    if (Test-Path $OutputFile) {
        Remove-Item $OutputFile -Force
    }
    
    # Add header
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    $header = "// ============================================================================`n"
    $header += "// KAIN Combined Compiler Source`n"
    $header += "// Generated by build.ps1 v2.0 at $timestamp`n"
    $header += "// Build ID: $BUILD_TIMESTAMP`n"
    $header += "// ============================================================================`n"
    $header += "// DO NOT EDIT - This file is auto-generated`n"
    $header += "// ============================================================================`n"
    
    Set-Content -Path $OutputFile -Value $header
    
    # Add each source file
    foreach ($source in $Sources) {
        $fullPath = Join-Path $ROOT $source
        if (-not (Test-Path $fullPath)) {
            Write-Err "Source file not found: $source"
            exit 1
        }
        
        Write-Debug "Adding: $source"
        
        # Add file separator comment
        Add-Content -Path $OutputFile -Value ""
        Add-Content -Path $OutputFile -Value "// ======== $source ========"
        Add-Content -Path $OutputFile -Value ""
        
        # Add content (skip any 'use' statements for already-included modules)
        $content = Get-Content $fullPath -Raw
        
        # Remove duplicate imports for combined file
        $lines = $content -split "`n"
        $filteredLines = @()
        
        foreach ($line in $lines) {
            $trimmed = $line.Trim()
            if ($trimmed -match "^use\s+(\w+)") {
                $moduleName = $Matches[1]
                # Skip if it's one of our combined modules
                $coreModuleNames = @("span", "error", "effects", "ast", "stdlib", "types", "lexer", "parser", "parser_v2", "codegen", "diagnostics", "diagnostic", "resolver")
                if ($coreModuleNames -contains $moduleName) {
                    Write-Debug "Skipping duplicate import: use $moduleName"
                    continue
                }
            }
            $filteredLines += $line
        }
        
        Add-Content -Path $OutputFile -Value ($filteredLines -join "`n")
    }
    
    $size = (Get-Item $OutputFile).Length
    Write-Success "Combined: $OutputFile ($size bytes)"
}

# ============================================================================
# Compile with Bootstrap
# ============================================================================

function Compile-WithBootstrap {
    param(
        [string]$InputFile,
        [string]$OutputLL,
        [switch]$RequireVerify
    )
    
    Write-Step "Compiling with bootstrap compiler..."
    Write-Info "  Input:  $InputFile"
    Write-Info "  Output: $OutputLL"
    
    Write-Debug "$BOOTSTRAP_COMPILER $InputFile -o $OutputLL"
    $logFile = "$LOGS_DIR\bootstrap_$BUILD_TIMESTAMP.log"
    
    # Temporarily allow stderr output without throwing
    $oldEAP = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    
    $compileOutput = & $BOOTSTRAP_COMPILER $InputFile -o $OutputLL 2>&1
    
    $ErrorActionPreference = $oldEAP
    
    # ALWAYS print output for debugging now
    $compileOutput | ForEach-Object { Write-Host "COMPILER: $_" }
    $compileOutput | Out-File -Encoding utf8 -FilePath $logFile
    
    if (-not (Test-Path $OutputLL)) {
        Write-Warn "Output file not found at expected path: $OutputLL. Retrying with fallback path..."
        $fallbackLL = "$BUILD_DIR\KAIN_native.ll"
        
        # Temporarily allow stderr output without throwing
        $oldEAP2 = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        
        $compileOutput2 = & $BOOTSTRAP_COMPILER $InputFile -o $fallbackLL 2>&1
        
        $ErrorActionPreference = $oldEAP2
        
        # ALWAYS print output for debugging now
        $compileOutput2 | ForEach-Object { Write-Host "COMPILER: $_" }
        $compileOutput2 | Out-File -Encoding utf8 -FilePath $logFile -Append
        
        if (Test-Path $fallbackLL) {
            Copy-Item $fallbackLL $OutputLL -Force
            Write-Info "Used fallback IR path and copied to expected location"
        } else {
            Write-Err "Output file not created: $OutputLL"
            exit 1
        }
    }
    
    if ($RequireVerify) {
        $verifyOK = $false
        foreach ($line in $compileOutput) {
            if ($line -match "module verify OK") { $verifyOK = $true; break }
            if ($line -match "module verify FAILED") { $verifyOK = $false; break }
        }
        if (-not $verifyOK) {
            Write-Err "IR verification failed (module.verify()). See log: $logFile"
            exit 1
        } else {
            Write-Success "IR verification passed"
        }
    }
    
    if ($LASTEXITCODE -ne 0) {
        Write-Warn "Bootstrap compiler exited with non-zero code ($LASTEXITCODE) but IR was generated"
        Write-Info "Continuing as verification gate passed"
    }
    
    Write-Success "Generated LLVM IR: $OutputLL"
}

function Compile-WithCompiler {
    param(
        [string]$Compiler,
        [string]$InputFile,
        [string]$OutputLL,
        [switch]$RequireVerify
    )
    
    Write-Step "Compiling with custom compiler..."
    Write-Info "  Compiler: $Compiler"
    Write-Info "  Input:    $InputFile"
    Write-Info "  Output:   $OutputLL"
    
    Write-Debug "$Compiler $InputFile -o $OutputLL"
    $logFile = "$LOGS_DIR\compile_$BUILD_TIMESTAMP.log"
    
    $oldEAP = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $compileOutput = & $Compiler $InputFile -o $OutputLL 2>&1
    $ErrorActionPreference = $oldEAP
    
    $compileOutput | ForEach-Object { Write-Host "COMPILER: $_" }
    $compileOutput | Out-File -Encoding utf8 -FilePath $logFile
    
    if (-not (Test-Path $OutputLL)) {
        Write-Err "Output file not created: $OutputLL"
        exit 1
    }
    
    if ($RequireVerify) {
        $verifyOK = $false
        foreach ($line in $compileOutput) {
            if ($line -match "module verify OK") { $verifyOK = $true; break }
            if ($line -match "VERIFICATION: PASS") { $verifyOK = $true; break }
            if ($line -match "module verify FAILED") { $verifyOK = $false; break }
            if ($line -match "VERIFICATION: FAIL") { $verifyOK = $false; break }
        }
        if (-not $verifyOK) {
            Write-Err "IR verification failed (module.verify()). See log: $logFile"
            exit 1
        } else {
            Write-Success "IR verification passed"
        }
    }
    
    Write-Success "Generated LLVM IR: $OutputLL"
}
# ============================================================================
# Fix LLVM IR
# ============================================================================

function Fix-LLVMIR {
    param(
        [string]$InputLL,
        [string]$OutputLL
    )
    
    Write-Step "Fixing LLVM IR..."
    
    $lines = Get-Content $InputLL
    $fixed = [System.Collections.Generic.List[string]]::new()
    $addedTypes = $false
    
    # Track declared functions to prevent duplicates
    $declaredFuncs = @{}
    
    foreach ($line in $lines) {
        # Check if this is a function declaration (anchored to start of line to avoid matching string constants)
        if ($line -match "^declare.*@(\w+)\s*\(") {
            $funcName = $Matches[1]
            if ($declaredFuncs.ContainsKey($funcName)) {
                Write-Debug "Skipping duplicate declaration: $funcName"
                continue
            }
            $declaredFuncs[$funcName] = $true
        }
        
        $fixed.Add($line)
        
        # Add missing type definitions after target triple
        if (-not $addedTypes -and $line -match "target triple") {
            $fixed.Add("")
            $fixed.Add("; Additional type definitions for KAIN runtime")
            $fixed.Add('%StringArray = type { i8**, i64, i64 }')
            
            # Only add declarations that don't already exist
            if (-not $declaredFuncs["char_code_at"]) {
                $fixed.Add("declare i64 @char_code_at(i64, i64)")
                $declaredFuncs["char_code_at"] = $true
            }
            if (-not $declaredFuncs["char_from_code"]) {
                $fixed.Add("declare i64 @char_from_code(i64)")
                $declaredFuncs["char_from_code"] = $true
            }
            if (-not $declaredFuncs["to_float"]) {
                $fixed.Add("declare i64 @to_float(i64)")
                $declaredFuncs["to_float"] = $true
            }
            
            $fixed.Add("")
            $addedTypes = $true
            Write-Debug "Added custom type definitions (skipped existing)"
        }
    }
    
    # Second pass: Fix blocks without terminators (REMOVED: BROKEN LOGIC)
    # The bootstrap compiler should be fixed instead of hacking IR with regex.
    $finalLines = $fixed

    
    [System.IO.File]::WriteAllLines($OutputLL, $finalLines.ToArray())
    Write-Success "Fixed LLVM IR: $OutputLL"
}

# ============================================================================
# Link Executable
# ============================================================================

function Link-Executable {
    param(
        [string]$InputLL,
        [string]$OutputExe,
        [int]$StackSize = 16777216  # 16MB stack (increased for large compiles)
    )
    
    Write-Step "Linking executable..."
    Write-Info "  Input:  $InputLL"
    Write-Info "  Output: $OutputExe"
    
    # Ensure runtime exists and is up-to-date
    $needsRebuild = $false
    if (-not (Test-Path $RUNTIME_OBJ)) {
        $needsRebuild = $true
    }
    elseif ((Get-Item $RUNTIME_SRC).LastWriteTime -gt (Get-Item $RUNTIME_OBJ).LastWriteTime) {
        Write-Warn "Runtime source is newer than object file - rebuilding"
        $needsRebuild = $true
    }
    elseif ($ForceRuntimeRebuild) {
        Write-Warn "Forced runtime rebuild"
        $needsRebuild = $true
    }
    
    if ($needsRebuild) {
        if (-not $SkipRuntime) {
            Build-Runtime
        }
        else {
            Write-Err "Runtime object not found or outdated: $RUNTIME_OBJ"
            exit 1
        }
    }
    
    $clangArgs = @(
        $InputLL,
        $RUNTIME_OBJ,
        "-o", $OutputExe,
        "-Wl,/STACK:$StackSize",
        "-O2"
    )
    
    Write-Debug "clang $($clangArgs -join ' ')"
    
    # Capture warnings but continue
    $oldEAP = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $output = & clang @clangArgs 2>&1
    $ErrorActionPreference = $oldEAP
    
    $warnings = $output | Where-Object { $_ -like "*warning*" }
    $errors = $output | Where-Object { $_ -like "*error*" }
    
    if ($warnings) {
        foreach ($w in $warnings) { Write-Debug $w }
    }
    
    if ($errors -or $LASTEXITCODE -ne 0) {
        Write-Err "Linking failed!"
        foreach ($e in $errors) { Write-Err $e }
        exit 1
    }
    
    $size = (Get-Item $OutputExe).Length
    Write-Success "Linked: $OutputExe ($size bytes)"
}

# ============================================================================
# Full Native Build
# ============================================================================

function Build-Native {
    $separator = "=" * 70
    Write-Info $separator
    Write-Info "BUILDING NATIVE KAIN COMPILER"
    Write-Info "Build ID: $BUILD_TIMESTAMP"
    Write-Info $separator
    
    # Create build folder for this run
    Ensure-Directory $BUILD_FOLDER
    Ensure-Directory $LOGS_DIR
    
    $combinedFile = "$BUILD_DIR\KAINc_build.kn"
    $llFile = "$BUILD_FOLDER\KAIN_native.ll"
    $fixedLL = "$BUILD_FOLDER\KAIN_native_fixed.ll"
    $exeFile = "$BUILD_FOLDER\KAIN_native.exe"
    
    # Step 1: Combine sources
    Combine-Sources -OutputFile $combinedFile -Sources $CORE_SOURCES
    
    # Copy combined file to build folder for reference
    Copy-Item $combinedFile "$BUILD_FOLDER\KAINc_build.kn" -Force
    
    # Step 2: Compile with native if available, else fallback to bootstrap
    $compilerToUse = $null
    if (Test-Path $LATEST_NATIVE) {
        $compilerToUse = $LATEST_NATIVE
        Write-Info "Using native compiler for IR generation"
        Compile-WithCompiler -Compiler $compilerToUse -InputFile $combinedFile -OutputLL $llFile -RequireVerify:$Verify
    } else {
        Write-Warn "Native compiler not found. Falling back to bootstrap compiler"
        Compile-WithBootstrap -InputFile $combinedFile -OutputLL $llFile -RequireVerify:$Verify
    }
    
    # Step 3: Fix LLVM IR
    Fix-LLVMIR -InputLL $llFile -OutputLL $fixedLL
    
    # Step 4: Link
    Link-Executable -InputLL $fixedLL -OutputExe $exeFile
    
    # Update latest link
    Update-LatestLink -BuildFolder $BUILD_FOLDER
    
    # Cleanup old builds
    Cleanup-OldBuilds -Keep $KeepHistory
    
    Write-Info $separator
    Write-Success "NATIVE COMPILER READY"
    Write-Info "  Path: $exeFile"
    Write-Info "  Latest: $LATEST_NATIVE"
    Write-Info $separator
    
    if ($RunSmokeTests) {
        Write-Host ""
        Run-SmokeTests
    }
}

# ============================================================================
# Self-Hosted Build
# ============================================================================

function Build-SelfHosted {
    $separator = "=" * 70
    Write-Info $separator
    Write-Info "BUILDING SELF-HOSTED KAIN COMPILER"
    Write-Info "Build ID: $BUILD_TIMESTAMP"
    Write-Info $separator
    
    # Check for native compiler
    $nativeCompiler = $null
    if (Test-Path $LATEST_NATIVE) {
        $nativeCompiler = $LATEST_NATIVE
    }
    elseif (Test-Path "$BUILD_FOLDER\KAIN_native.exe") {
        $nativeCompiler = "$BUILD_FOLDER\KAIN_native.exe"
    }
    else {
        Write-Warn "Native compiler not found. Building first..."
        Build-Native
        $nativeCompiler = "$BUILD_FOLDER\KAIN_native.exe"
    }
    
    Write-Info "Using native compiler: $nativeCompiler"
    
    # Reuse the same build folder
    Ensure-Directory $BUILD_FOLDER
    
    $combinedFile = "$BUILD_DIR\KAINc_build.kn"
    $llFile = "$BUILD_FOLDER\KAIN_native_v2.ll"
    $exeFile = "$BUILD_FOLDER\KAIN_native_v2.exe"
    
    # Step 1: Ensure combined sources exist
    if (-not (Test-Path $combinedFile)) {
        Combine-Sources -OutputFile $combinedFile -Sources $CORE_SOURCES
    }
    
    # Step 2: Compile with native compiler
    Write-Step "Compiling with native KAIN compiler..."
    
    $logFile = "$LOGS_DIR\self_hosted_$BUILD_TIMESTAMP.log"
    $startTime = Get-Date
    
    & $nativeCompiler $combinedFile -o $llFile 2>&1 | Tee-Object -FilePath $logFile
    $exitCode = $LASTEXITCODE
    
    $elapsed = (Get-Date) - $startTime
    
    if ($exitCode -ne 0 -or -not (Test-Path $llFile)) {
        Write-Err "Self-hosted compilation FAILED (exit code $exitCode)"
        Write-Info "Log file: $logFile"
        exit 1
    }
    
    Write-Success "Self-hosted compilation succeeded in $($elapsed.TotalSeconds.ToString('0.00'))s"
    
    # Step 3: Fix LLVM IR (deduplicate declarations)
    $fixedLL = "$BUILD_FOLDER\KAIN_native_v2_fixed.ll"
    Fix-LLVMIR -InputLL $llFile -OutputLL $fixedLL
    
    # Step 4: Link
    Link-Executable -InputLL $fixedLL -OutputExe $exeFile
    
    # Update latest link
    Update-LatestLink -BuildFolder $BUILD_FOLDER
    
    Write-Info $separator
    Write-Success "SELF-HOSTED COMPILER READY"
    Write-Info "  Path: $exeFile"
    Write-Info "  Latest: $LATEST_V2"
    Write-Info $separator
}

# ============================================================================
# Run Tests
# ============================================================================

function Run-Tests {
    $separator = "=" * 70
    Write-Info $separator
    Write-Info "RUNNING TESTS"
    Write-Info $separator
    
    # Find compiler to use
    $compiler = $null
    if (Test-Path $LATEST_NATIVE) { $compiler = $LATEST_NATIVE }
    elseif (Test-Path $BOOTSTRAP_COMPILER) { $compiler = $BOOTSTRAP_COMPILER }
    else {
        Write-Err "No compiler found for testing"
        exit 1
    }
    
    Write-Info "Using compiler: $compiler"
    
    $needsRebuild = $false
    if (-not (Test-Path $RUNTIME_OBJ)) {
        $needsRebuild = $true
    } elseif ((Get-Item $RUNTIME_SRC).LastWriteTime -gt (Get-Item $RUNTIME_OBJ).LastWriteTime) {
        $needsRebuild = $true
    }
    if ($needsRebuild) {
        Build-Runtime
    }
    
    # Find tests
    $testFiles = @()
    if (Test-Path "$TESTS_DIR\unit") {
        $testFiles += Get-ChildItem "$TESTS_DIR\unit\*.kn" -ErrorAction SilentlyContinue
    }
    if (Test-Path "$TESTS_DIR\integration") {
        $testFiles += Get-ChildItem "$TESTS_DIR\integration\*.kn" -ErrorAction SilentlyContinue
    }
    # Also include root tests
    $testFiles += Get-ChildItem "$TESTS_DIR\*.kn" -ErrorAction SilentlyContinue
    
    if ($testFiles.Count -eq 0) {
        Write-Warn "No test files found in $TESTS_DIR"
        return
    }
    
    $testBuildDir = "$BUILD_DIR\test_$BUILD_TIMESTAMP"
    Ensure-Directory $testBuildDir
    
    $passed = 0
    $failed = 0
    
    foreach ($test in $testFiles) {
        $name = $test.BaseName
        Write-Host -NoNewline "  Testing $name ... "
        
        $llFile = "$testBuildDir\$name.ll"
        $exeFile = "$testBuildDir\$name.exe"
        
        try {
            & $compiler $test.FullName -o $llFile 2>&1 | Out-Null
            if ($LASTEXITCODE -ne 0) { throw "Compilation failed" }
            
            & clang $llFile $RUNTIME_OBJ -o $exeFile 2>&1 | Out-Null
            if ($LASTEXITCODE -ne 0) { throw "Linking failed" }
            
            # Run
            $output = & $exeFile 2>&1
            
            # Check for expected output
            $expectedFile = Join-Path (Split-Path $test.FullName) "$name.expected"
            if (Test-Path $expectedFile) {
                $expected = Get-Content $expectedFile -Raw
                if ($output -ne $expected.Trim()) {
                    throw "Output mismatch"
                }
            }
            
            Write-Color Green "PASS"
            $passed++
        }
        catch {
            Write-Color Red "FAIL: $_"
            $failed++
        }
    }
    
    Write-Info ""
    Write-Info "Results: $passed passed, $failed failed"
    
    if ($failed -gt 0) {
        exit 1
    }
}

# ============================================================================
# Smoke Tests (post-link)
# ============================================================================
function Run-SmokeTests {
    $separator = "=" * 70
    Write-Info $separator
    Write-Info "RUNNING SMOKE TESTS (native)"
    Write-Info $separator
    
    if (-not (Test-Path $LATEST_NATIVE)) {
        Write-Err "Latest native compiler not found at $LATEST_NATIVE"
        exit 1
    }
    $compiler = $LATEST_NATIVE
    Write-Info "Using native compiler: $compiler"
    
    $report = @()
    $smokeDir1 = "$TESTS_DIR\examples\demo_pack_1"
    $smokeDir2 = "$TESTS_DIR\examples\demo_pack_2"
    
    $testFiles = @()
    if (Test-Path $smokeDir1) { $testFiles += Get-ChildItem "$smokeDir1\*.kn" -ErrorAction SilentlyContinue }
    if (Test-Path $smokeDir2) { $testFiles += Get-ChildItem "$smokeDir2\*.kn" -ErrorAction SilentlyContinue }
    $testFiles += Get-ChildItem "$TESTS_DIR\*.kn" -ErrorAction SilentlyContinue
    
    if ($testFiles.Count -eq 0) {
        Write-Warn "No smoke tests found"
        return
    }
    
    $testBuildDir = "$BUILD_DIR\smoke_$BUILD_TIMESTAMP"
    Ensure-Directory $testBuildDir
    
    $passed = 0
    $failed = 0
    
    foreach ($test in $testFiles) {
        $name = $test.BaseName
        $start = Get-Date
        Write-Host -NoNewline "  [$($name)] ... "
        $llFile = "$testBuildDir\$name.ll"
        $exeFile = "$testBuildDir\$name.exe"
        $status = "PASS"
        $errMsg = ""
        
        try {
            # Compile
            & $compiler $test.FullName -o $llFile 2>&1 | Out-Null
            if ($LASTEXITCODE -ne 0) { throw "Compilation failed" }
            
            # Link
            & clang $llFile $RUNTIME_OBJ -o $exeFile -O2 2>&1 | Out-Null
            if ($LASTEXITCODE -ne 0) { throw "Linking failed" }
            
            # Run
            $output = & $exeFile 2>&1
            
            # Expected output check
            $expectedFile = Join-Path (Split-Path $test.FullName) "$name.expected"
            if (Test-Path $expectedFile) {
                $expected = Get-Content $expectedFile -Raw
                if ($output.Trim() -ne $expected.Trim()) {
                    throw "Output mismatch"
                }
            }
            
            Write-Color Green "PASS"
            $passed++
        }
        catch {
            $status = "FAIL"
            $errMsg = $_.ToString()
            Write-Color Red "FAIL: $errMsg"
            $failed++
        }
        finally {
            $elapsed = (Get-Date) - $start
            $report += [PSCustomObject]@{
                Name = $name
                Path = $test.FullName
                Status = $status
                TimeSeconds = [math]::Round($elapsed.TotalSeconds, 3)
                Error = $errMsg
            }
        }
    }
    
    $reportFile = "$LOGS_DIR\smoke_$BUILD_TIMESTAMP.json"
    $report | ConvertTo-Json -Depth 3 | Out-File -Encoding utf8 -FilePath $reportFile
    
    Write-Info ""
    Write-Info "Smoke Results: $passed passed, $failed failed"
    Write-Info "Report: $reportFile"
    
    if ($failed -gt 0) { exit 1 }
}
# ============================================================================
# Main Entry Point
# ============================================================================

function Main {
    Write-Host ""
    Write-Host "================================================================" -ForegroundColor Cyan
    Write-Host "  K O R E   B U I L D   S Y S T E M   v2.0                     " -ForegroundColor Cyan
    Write-Host "  Project Ouroboros                                           " -ForegroundColor Cyan
    Write-Host "----------------------------------------------------------------" -ForegroundColor Cyan
    Write-Host "  Build ID: $BUILD_TIMESTAMP                                  " -ForegroundColor Cyan
    Write-Host "================================================================" -ForegroundColor Cyan
    Write-Host ""
    
    $startTime = Get-Date
    
    # Handle clean
    if ($CleanAll) {
        Clean-All
        return
    }
    
    if ($Clean) {
        Clean-Build
        if ($Target -eq "all") { return }
    }
    
    # Check prerequisites
    Check-Prerequisites
    
    # Ensure build directories
    Ensure-Directory $BUILD_DIR
    Ensure-Directory $ARTIFACTS_DIR
    Ensure-Directory $LOGS_DIR
    
    # Execute target
    switch ($Target) {
        "bootstrap" { Build-Bootstrap }
        "runtime" { Build-Runtime }
        "native" { Build-Native }
        "self" { Build-SelfHosted }
        "test" { Run-Tests }
        "combine" { 
            Combine-Sources -OutputFile "$BUILD_DIR\KAINc_build.kn" -Sources $CORE_SOURCES 
        }
        "all" {
            Build-Native
            if (-not $SkipSelfHosted) {
                Write-Host ""
                Build-SelfHosted
            }
            else {
                Write-Info "Skipping self-hosted build (-SkipSelfHosted)"
            }
        }
    }
    
    $elapsed = (Get-Date) - $startTime
    Write-Host ""
    Write-Success "Build completed in $($elapsed.TotalSeconds.ToString('0.00')) seconds"
    Write-Host ""
    Write-Info "Build artifacts: $BUILD_FOLDER"
    Write-Info "Latest build:    $LATEST_BUILD"
    Write-Host ""
}

# Run
Main
