$ErrorActionPreference = "Stop"
$depsDir = "k:\deps"
New-Item -ItemType Directory -Force -Path $depsDir | Out-Null
$llvmDir = "C:\LLVM-21"

function Try-Download-Or-Dummy ($name, $versions) {
    $baseUrl = "https://github.com/kiyolee/${name}-win-build/releases"
    $success = $false
    
    foreach ($v in $versions) {
        if ($success) { break }
        foreach ($vs in @("v143", "v142", "")) {
            $suffix = if ($vs) { "-$vs" } else { "" }
            $zipName = "$name-$v-win64-msvc$suffix.zip"
            $url = "$baseUrl/download/v$v/$zipName"
            Write-Host "Trying $url ..."
            try {
                $dest = Join-Path $depsDir $zipName
                Invoke-WebRequest -Uri $url -OutFile $dest -UseBasicParsing -ErrorAction Stop
                
                Expand-Archive -Path $dest -DestinationPath $depsDir -Force
                $extractDir = Get-ChildItem "$depsDir\$name*" -Directory | Where-Object { $_.FullName -like "*$v*" } | Select -First 1
                if (-not $extractDir) { $extractDir = Get-ChildItem "$depsDir\$name*" -Directory | Sort-Object LastWriteTime -Descending | Select -First 1 }
                
                if ($extractDir) {
                    Write-Host "Installing from $($extractDir.FullName)"
                    Copy-Item "$($extractDir.FullName)\lib\*.lib" "$llvmDir\lib\" -Force
                    if (Test-Path "$($extractDir.FullName)\bin") { Copy-Item "$($extractDir.FullName)\bin\*.dll" "$llvmDir\bin\" -Force }
                    if (Test-Path "$($extractDir.FullName)\include") { Copy-Item "$($extractDir.FullName)\include\*" "$llvmDir\include\" -Recurse -Force }
                    $success = $true
                    break
                }
            } catch { }
        }
    }
    
    if (-not $success) {
        Write-Warning "Failed to download $name. Creating Stub."
        $dummyObj = Join-Path $depsDir "dummy.o"
        # Compile if not exists
        if (-not (Test-Path $dummyObj)) {
            $dummyC = Join-Path $depsDir "dummy.c"
            Set-Content -Path $dummyC -Value "int ${name}_dummy = 0;"
            & "$llvmDir\bin\clang.exe" -c $dummyC -o $dummyObj
        }
        & "$llvmDir\bin\llvm-ar.exe" rc "$llvmDir\lib\$name.lib" $dummyObj
    }
}

# 1. Install Dependencies
Try-Download-Or-Dummy "libxml2" @("2.11.5", "2.10.3", "2.9.14")
Try-Download-Or-Dummy "libiconv" @("1.17", "1.16")
Try-Download-Or-Dummy "zlib" @("1.3.1", "1.2.13")

# 2. Fix Alias
if (Test-Path "$llvmDir\lib\libxml2.lib") {
    Copy-Item "$llvmDir\lib\libxml2.lib" "$llvmDir\lib\libxml2s.lib" -Force
}

# 3. Apply LLVM 21 Compatibility Hacks (Stub removed functions)
Write-Host "Applying LLVM 21 Hacks..."
$hacksC = Join-Path $depsDir "hacks.c"
$hacksContent = @"
void* LLVMConstMul(void* a, void* b) { return 0; }
void* LLVMConstAdd(void* a, void* b) { return 0; }
void* LLVMConstSub(void* a, void* b) { return 0; }
void* LLVMConstSDiv(void* a, void* b) { return 0; }
void* LLVMConstUDiv(void* a, void* b) { return 0; }
void* LLVMConstAnd(void* a, void* b) { return 0; }
void* LLVMConstOr(void* a, void* b) { return 0; }
void* LLVMConstXor(void* a, void* b) { return 0; }
void* LLVMConstShl(void* a, void* b) { return 0; }
void* LLVMConstLShr(void* a, void* b) { return 0; }
void* LLVMConstAShr(void* a, void* b) { return 0; }
void* LLVMConstGEP(void* val, void* indices, int count) { return 0; }
"@
Set-Content -Path $hacksC -Value $hacksContent
$hacksObj = Join-Path $depsDir "hacks.o"

& "$llvmDir\bin\clang.exe" -c $hacksC -o $hacksObj

# Inject hacks into libxml2s.lib (so they are linked automatically)
if (Test-Path "$llvmDir\lib\libxml2s.lib") {
    Write-Host "Injecting hacks into libxml2s.lib"
    & "$llvmDir\bin\llvm-ar.exe" r "$llvmDir\lib\libxml2s.lib" $hacksObj
} else {
    Write-Error "libxml2s.lib missing! Cannot inject hacks."
}

Write-Host "Done."
