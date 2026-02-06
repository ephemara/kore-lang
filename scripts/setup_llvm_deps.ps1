$ErrorActionPreference = "Stop"
$depsDir = "k:\deps"
New-Item -ItemType Directory -Force -Path $depsDir | Out-Null
$llvmDir = "C:\LLVM-21"

function Try-Download ($repo, $name, $pattern, $versions) {
    $baseUrl = "https://github.com/kiyolee/$repo/releases"
    $success = $false
    
    foreach ($v in $versions) {
        if ($success) { break }
        foreach ($vs in @("v143", "v142")) {
            $zipName = "$name-$v-win64-msvc-$vs.zip"
            $url = "$baseUrl/download/v$v/$zipName"
            Write-Host "Trying $url..."
            try {
                $dest = Join-Path $depsDir $zipName
                Invoke-WebRequest -Uri $url -OutFile $dest -UseBasicParsing -ErrorAction Stop
                Write-Host "Downloaded $zipName"

                Expand-Archive -Path $dest -DestinationPath $depsDir -Force
                $extractDir = Get-ChildItem "$depsDir\$pattern" -Directory | Where-Object { $_.FullName -like "*$v*" } | Select -First 1
                
                if (-not $extractDir) {
                     # Fallback to just sorting by date if version match fails
                     $extractDir = Get-ChildItem "$depsDir\$pattern" -Directory | Sort-Object LastWriteTime -Descending | Select -First 1
                }

                if ($extractDir) {
                    Write-Host "Installing from $($extractDir.FullName)"
                    Copy-Item "$($extractDir.FullName)\lib\*.lib" "$llvmDir\lib\" -Force
                    if (Test-Path "$($extractDir.FullName)\bin") { Copy-Item "$($extractDir.FullName)\bin\*.dll" "$llvmDir\bin\" -Force }
                    if (Test-Path "$($extractDir.FullName)\include") { Copy-Item "$($extractDir.FullName)\include\*" "$llvmDir\include\" -Recurse -Force }
                    $success = $true
                    break
                }
            } catch {
                Write-Host "Failed to download/install $v ($vs): $_"
            }
        }
    }
    if (-not $success) {
        Write-Error "Could not download $name after trying all versions."
    }
}

# Try known working versions in order of preference
Try-Download "libxml2-win-build" "libxml2" "libxml2*" @("2.11.5", "2.10.3", "2.9.14")
Try-Download "libiconv-win-build" "libiconv" "libiconv*" @("1.17", "1.16")
Try-Download "zlib-win-build" "zlib" "zlib*" @("1.3.1", "1.2.13")

# Fix alias for llvm-config expectation
if (Test-Path "$llvmDir\lib\libxml2.lib") {
    Copy-Item "$llvmDir\lib\libxml2.lib" "$llvmDir\lib\libxml2s.lib" -Force
    Write-Host "Fixed libxml2s.lib link"
} else {
    Write-Error "Failed to locate libxml2.lib to create alias!"
}
