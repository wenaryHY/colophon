# Colophon Memory Monitor (PowerShell)
# Usage: .\monitor_memory.ps1 [output.csv]

param(
    [string]$OutputFile = "memory_monitor.csv"
)

Write-Host "=== Colophon Memory Monitor ===" -ForegroundColor Green
Write-Host ""

# 查找 Colophon 进程
$Process = Get-Process -Name "colophon" -ErrorAction SilentlyContinue

if (-not $Process) {
    Write-Host "Error: Colophon process not found" -ForegroundColor Red
    Write-Host "Start server first: cargo run --release" -ForegroundColor Yellow
    exit 1
}

$PID = $Process.Id
Write-Host "Monitoring PID: $PID" -ForegroundColor Green
Write-Host "Output: $OutputFile" -ForegroundColor Blue
Write-Host "Press Ctrl+C to stop" -ForegroundColor Yellow
Write-Host ""

# 写入 CSV 头
"Timestamp,Elapsed(s),RSS(MB),WorkingSet(MB),PrivateMemory(MB),CPU(%)" | Out-File -FilePath $OutputFile -Encoding UTF8

$StartTime = Get-Date

try {
    while ($true) {
        # 刷新进程信息
        $Process = Get-Process -Id $PID -ErrorAction Stop
        
        $CurrentTime = Get-Date
        $Elapsed = [math]::Round(($CurrentTime - $StartTime).TotalSeconds, 0)
        
        # 获取内存信息（转换为 MB）
        $WorkingSet = [math]::Round($Process.WorkingSet64 / 1MB, 2)
        $PrivateMemory = [math]::Round($Process.PrivateMemorySize64 / 1MB, 2)
        $RSS = [math]::Round($Process.WorkingSet64 / 1MB, 2)  # RSS ≈ WorkingSet in Windows
        
        # CPU 使用率（需要两次采样）
        $CPU = [math]::Round($Process.CPU, 2)
        
        # 输出到控制台和文件
        $Line = "$($CurrentTime.ToString('yyyy-MM-dd HH:mm:ss')),$Elapsed,$RSS,$WorkingSet,$PrivateMemory,$CPU"
        Write-Host $Line
        $Line | Out-File -FilePath $OutputFile -Append -Encoding UTF8
        
        Start-Sleep -Seconds 1
    }
}
catch {
    Write-Host ""
    Write-Host "Process terminated or error occurred" -ForegroundColor Red
    Write-Host "Results saved to: $OutputFile" -ForegroundColor Blue
}

Write-Host ""
Write-Host "=== Monitoring Complete ===" -ForegroundColor Green
Write-Host "Results saved to: $OutputFile" -ForegroundColor Blue
