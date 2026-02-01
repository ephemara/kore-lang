param(
  [switch]$VerifyOnly,
  [switch]$RunSelfHost
)

$ErrorActionPreference = "Stop"
$ROOT = Split-Path $PSScriptRoot -Parent
$BUILD = "$ROOT\build.ps1"

function Run-Native {
  & $BUILD -Target native -Verify
}

function Run-Tests {
  & $BUILD -Target test
}

function Run-Self {
  & $BUILD -Target self
}

Ensure-Native
function Ensure-Native {
  Run-Native
}

if ($VerifyOnly) {
  Run-Native
  Run-Tests
  exit 0
}

Run-Native
Run-Tests
if ($RunSelfHost) {
  Run-Self
}

