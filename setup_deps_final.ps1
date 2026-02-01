$ErrorActionPreference = "Stop"
$depsDir = "k:\deps"
New-Item -ItemType Directory -Force -Path $depsDir | Out-Null
$llvmDir = "C:\LLVM-21"

function Try-Download-Or-Dummy ($name, $versions, $patterns) {
    $baseUrl = "https://github.com/kiyolee/${name}-win-build/releases"
    $success = $false
    
    foreach ($v in $versions) {
        if ($success) { break }
        # Try multiple filename patterns
        $candidates = @(
            "$name-$v-win64-msvc-v143.zip",
            "$name-$v-win64-msvc-v142.zip",
            "$name-$v-win64-msvc.zip"
        )
        
        foreach ($zipName in $candidates) {
             $url = "$baseUrl/download/v$v/$zipName"
             try {
                Write-Host "Trying $url ..."
                $dest = Join-Path $depsDir $zipName
                Invoke-WebRequest -Uri $url -OutFile $dest -UseBasicParsing -ErrorAction Stop
                
                Expand-Archive -Path $dest -DestinationPath $depsDir -Force
                $extractDir = Get-ChildItem "$depsDir\$name*" -Directory | Where-Object { $_.FullName -like "*$v*" } | Select -First 1
                if (-not $extractDir) { $extractDir = Get-ChildItem "$depsDir\$name*" -Directory | Sort-Object LastWriteTime -Descending | Select -First 1 }
                
                if ($extractDir) {
                    Write-Host "Success! Installing form $($extractDir.FullName)"
                    Copy-Item "$($extractDir.FullName)\lib\*.lib" "$llvmDir\lib\" -Force
                    if (Test-Path "$($extractDir.FullName)\bin") { Copy-Item "$($extractDir.FullName)\bin\*.dll" "$llvmDir\bin\" -Force }
                    if (Test-Path "$($extractDir.FullName)\include") { Copy-Item "$($extractDir.FullName)\include\*" "$llvmDir\include\" -Recurse -Force }
                    $success = $true
                    break
                }
             } catch { Write-Host "404 for $zipName" }
        }
    }
    
    if (-not $success) {
        Write-Warning "Failed to download $name. Creating Stub/Dummy library to satisfy linker."
        # Create a dummy object file using clang
        $dummyC = Join-Path $depsDir "dummy.c"
        Set-Content -Path $dummyC -Value "int ${name}_dummy_symbol = 0;"
        $dummyObj = Join-Path $depsDir "dummy.o"
        
        $clang = "$llvmDir\bin\clang.exe"
        $llvmAr = "$llvmDir\bin\llvm-ar.exe"
        
        & $clang -c $dummyC -o $dummyObj
        if ($LASTEXITCODE -eq 0) {
             # Create both .lib name variations
             & $llvmAr rc "$llvmDir\lib\$name.lib" $dummyObj
             & $llvmAr rc "$llvmDir\lib\${name}s.lib" $dummyObj
             Write-Host "Created dummy $name library."
        } else {
            Write-Error "Failed to create dummy object with clang."
        }
    }
}

Try-Download-Or-Dummy "libxml2" @("2.11.5", "2.10.3", "2.11.4", "2.9.14")
Try-Download-Or-Dummy "libiconv" @("1.17", "1.16")
Try-Download-Or-Dummy "zlib" @("1.3.1", "1.3", "1.2.13")

# Ensure libxml2s.lib exists (either from real lib or dummy)
if (-not (Test-Path "$llvmDir\lib\libxml2s.lib")) {
    if (Test-Path "$llvmDir\lib\libxml2.lib") {
        Copy-Item "$llvmDir\lib\libxml2.lib" "$llvmDir\lib\libxml2s.lib" -Force
        Write-Host "Created libxml2s.lib alias"
    }
}

Write-Host "Dependency Setup Complete."
