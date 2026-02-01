$ErrorActionPreference = "Stop"
$depsDir = "k:\deps"
New-Item -ItemType Directory -Force -Path $depsDir | Out-Null
$llvmDir = "C:\LLVM-21"

function Install-Latest-From-Github ($repo, $name, $pattern) {
    $releasesUrl = "https://github.com/kiyolee/$repo/releases"
    Write-Host "Scraping $releasesUrl ..."
    try {
        $content = (Invoke-WebRequest -Uri $releasesUrl -UseBasicParsing).Content
        
        # Regex using single quotes to avoid escaping issues
        # Pattern looks for: /kiyolee/repo/releases/download/[tag]/[name]-...-win64-msvc-[...].zip
        $regex = "\/$repo\/releases\/download\/[^""]+?-win64-msvc-[^""]+?\.zip"
        
        if ($content -match $regex) {
            $relPath = $matches[0]
            $fullUrl = "https://github.com$relPath"
            $zipName = "$name.zip"
            
            Write-Host "Found URL: $fullUrl"
            $dest = Join-Path $depsDir $zipName
            Invoke-WebRequest -Uri $fullUrl -OutFile $dest -UseBasicParsing
            Write-Host "Downloaded."

            Expand-Archive -Path $dest -DestinationPath $depsDir -Force
            
            # Find extract dir
            $extractDir = Get-ChildItem "$depsDir\$pattern" -Directory | Sort-Object LastWriteTime -Descending | Select -First 1
            
            if ($extractDir) {
                Write-Host "Installing from $($extractDir.FullName)"
                Copy-Item "$($extractDir.FullName)\lib\*.lib" "$llvmDir\lib\" -Force
                if (Test-Path "$($extractDir.FullName)\bin") { Copy-Item "$($extractDir.FullName)\bin\*.dll" "$llvmDir\bin\" -Force }
                if (Test-Path "$($extractDir.FullName)\include") { Copy-Item "$($extractDir.FullName)\include\*" "$llvmDir\include\" -Recurse -Force }
            } else {
                Write-Error "Could not find extracted directory for $name"
            }
        } else {
            Write-Error "Could not find valid zip link on releases page for $repo"
        }
    } catch {
        Write-Error "Failed processing $name : $($_.Exception.Message)"
    }
}

Install-Latest-From-Github "libxml2-win-build" "libxml2" "libxml2*"
Install-Latest-From-Github "libiconv-win-build" "libiconv" "libiconv*"
Install-Latest-From-Github "zlib-win-build" "zlib" "zlib*"

if (Test-Path "$llvmDir\lib\libxml2.lib") {
    Copy-Item "$llvmDir\lib\libxml2.lib" "$llvmDir\lib\libxml2s.lib" -Force
    Write-Host "Fixed libxml2s.lib"
} else {
    Write-Error "Failed to install libxml2.lib"
}
