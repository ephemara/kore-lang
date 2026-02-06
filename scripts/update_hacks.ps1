$ErrorActionPreference = "Stop"
$depsDir = "k:\deps"
$llvmDir = "C:\LLVM-21"

Write-Host "Updating LLVM 21 Hacks..."
$hacksC = Join-Path $depsDir "hacks.c"
$hacksContent = @"
void* LLVMConstMul(void* a, void* b) { return 0; }
void* LLVMConstNUWMul(void* a, void* b) { return 0; }
void* LLVMConstNSWMul(void* a, void* b) { return 0; }
void* LLVMConstSDiv(void* a, void* b) { return 0; }
void* LLVMConstUDiv(void* a, void* b) { return 0; }
"@
Set-Content -Path $hacksC -Value $hacksContent
$hacksObj = Join-Path $depsDir "hacks.o"

& "$llvmDir\bin\clang.exe" -c $hacksC -o $hacksObj

if (Test-Path "$llvmDir\lib\libxml2s.lib") {
    Write-Host "Injecting hacks into libxml2s.lib"
    & "$llvmDir\bin\llvm-ar.exe" r "$llvmDir\lib\libxml2s.lib" $hacksObj
}
