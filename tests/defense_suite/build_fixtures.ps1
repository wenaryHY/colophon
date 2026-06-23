# 编译防御性测试所需的 Wasm fixture (killers)
#
# 前置条件: rustup target add wasm32-wasip1
# 产物: tests/defense_suite/fixtures/dead_loop.wasm
#        tests/defense_suite/fixtures/large_payload.wasm
#        tests/defense_suite/fixtures/memory_bomb.wasm

param (
    [switch]$SkipCheck = $false
)

$ErrorActionPreference = "Stop"
Push-Location $PSScriptRoot

if (-not $SkipCheck) {
    $targets = rustup target list --installed 2>$null | Out-String
    if ($targets -notmatch "wasm32-wasip1") {
        Write-Host "[ERROR] wasm32-wasip1 target not installed." -ForegroundColor Red
        Write-Host "  Run: rustup target add wasm32-wasip1" -ForegroundColor Yellow
        Pop-Location
        exit 1
    }
}

$fixturesDir = Join-Path $PSScriptRoot "fixtures"
$fixtures = @("dead_loop", "large_payload", "memory_bomb")

foreach ($fixture in $fixtures) {
    $projDir = Join-Path $fixturesDir $fixture
    $wasmOut = Join-Path $fixturesDir "$fixture.wasm"

    Write-Host "[BUILD] $fixture..." -ForegroundColor Cyan
    Push-Location $projDir
    try {
        cargo build --target wasm32-wasip1 --release 2>&1
        if ($LASTEXITCODE -ne 0) {
            Write-Host "[SKIP] $fixture build failed (check wasm32-wasip1 target)" -ForegroundColor Yellow
            Pop-Location
            continue
        }

        $srcWasm = Join-Path $projDir "target\wasm32-wasip1\release\*.wasm"
        $files = Get-ChildItem $srcWasm
        if ($files.Count -gt 0) {
            Copy-Item $files[0].FullName -Destination $wasmOut -Force
            Write-Host "  -> $wasmOut" -ForegroundColor Green
        } else {
            Write-Host "[SKIP] no .wasm found in target dir" -ForegroundColor Yellow
        }
    } finally {
        Pop-Location
    }
}

Pop-Location
Write-Host "[DONE] Fixture build complete" -ForegroundColor Green
