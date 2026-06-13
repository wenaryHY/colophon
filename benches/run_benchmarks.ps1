# Colophon Benchmark Runner (PowerShell)
# Usage: .\run_benchmarks.ps1

Write-Host "╔═══════════════════════════════════════════════════╗" -ForegroundColor Green
Write-Host "║   Colophon Performance Benchmark Suite          ║" -ForegroundColor Green
Write-Host "╚═══════════════════════════════════════════════════╝" -ForegroundColor Green
Write-Host ""

# 检查 Cargo
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "Error: cargo not found. Please install Rust." -ForegroundColor Red
    exit 1
}

# 创建报告目录
$ReportDir = "benches\reports\$(Get-Date -Format 'yyyyMMdd_HHmmss')"
New-Item -ItemType Directory -Path $ReportDir -Force | Out-Null

Write-Host "Report Directory: $ReportDir" -ForegroundColor Blue
Write-Host ""

# ========================================
# Step 1: Criterion 基准测试
# ========================================
Write-Host "═══ Step 1/2: Running Criterion Micro-Benchmarks ═══" -ForegroundColor Yellow
Write-Host ""

Write-Host "Compiling and running benchmarks (this may take a few minutes)..." -ForegroundColor Cyan
Write-Host ""

$BenchResult = cargo bench --bench api_benchmarks 2>&1

if ($LASTEXITCODE -eq 0) {
    Write-Host ""
    Write-Host "✓ Criterion benchmarks completed" -ForegroundColor Green
    Write-Host "  HTML Report: target\criterion\report\index.html" -ForegroundColor Blue
    Write-Host ""
    
    # 尝试在浏览器中打开报告
    $ReportPath = "target\criterion\report\index.html"
    if (Test-Path $ReportPath) {
        $OpenReport = Read-Host "Open HTML report in browser? [Y/n]"
        if ($OpenReport -ne "n" -and $OpenReport -ne "N") {
            Start-Process $ReportPath
        }
    }
} else {
    Write-Host ""
    Write-Host "✗ Criterion benchmarks failed" -ForegroundColor Red
    Write-Host $BenchResult
    exit 1
}

# ========================================
# Step 2: 负载测试（可选）
# ========================================
Write-Host "═══ Step 2/2: Load Testing (Optional) ═══" -ForegroundColor Yellow
Write-Host ""

Write-Host "Load testing requires:" -ForegroundColor Blue
Write-Host "  1. A running Colophon server (cargo run --release)" -ForegroundColor Blue
Write-Host "  2. wrk or bombardier installed" -ForegroundColor Blue
Write-Host ""

$RunLoadTest = Read-Host "Run load tests? [y/N]"

if ($RunLoadTest -eq "y" -or $RunLoadTest -eq "Y") {
    # 检查服务器是否运行
    try {
        $Response = Invoke-WebRequest -Uri "http://localhost:3000/api/health" -TimeoutSec 5 -ErrorAction Stop
        Write-Host "✓ Server is running" -ForegroundColor Green
        Write-Host ""
        
        # 检测工具
        if (Get-Command wrk -ErrorAction SilentlyContinue) {
            Write-Host "Using wrk for load testing" -ForegroundColor Green
            bash benches/load_test.sh
        } elseif (Get-Command bombardier -ErrorAction SilentlyContinue) {
            Write-Host "Using bombardier for load testing" -ForegroundColor Green
            bash benches/load_test_bombardier.sh
        } else {
            Write-Host "No load testing tool found" -ForegroundColor Red
            Write-Host "Install:" -ForegroundColor Yellow
            Write-Host "  wrk: https://github.com/wg/wrk" -ForegroundColor Yellow
            Write-Host "  bombardier: https://github.com/codesenberg/bombardier/releases" -ForegroundColor Yellow
        }
    } catch {
        Write-Host "✗ Server not responding at http://localhost:3000" -ForegroundColor Red
        Write-Host "Start server first: cargo run --release" -ForegroundColor Yellow
    }
} else {
    Write-Host "Skipping load tests" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "╔═══════════════════════════════════════════════════╗" -ForegroundColor Green
Write-Host "║            Benchmark Suite Complete              ║" -ForegroundColor Green
Write-Host "╚═══════════════════════════════════════════════════╝" -ForegroundColor Green
Write-Host ""

Write-Host "Next Steps:" -ForegroundColor Blue
Write-Host "  1. View Criterion report:" -ForegroundColor White
Write-Host "     target\criterion\report\index.html" -ForegroundColor Yellow
Write-Host ""
Write-Host "  2. Run memory monitoring (separate PowerShell window):" -ForegroundColor White
Write-Host "     .\benches\monitor_memory.ps1" -ForegroundColor Yellow
Write-Host ""
Write-Host "  3. Document baseline metrics:" -ForegroundColor White
Write-Host "     Edit benches\README.md" -ForegroundColor Yellow
