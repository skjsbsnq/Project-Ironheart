param(
    [string]$GameDir = "C:\Program Files (x86)\Steam\steamapps\common\Hearts of Iron IV",
    [string]$TraceDir = "C:\Users\19180\Documents\999\b1\tools\vanilla_trace",
    [string]$ProxySourceDir = "",
    [int]$TimeoutSeconds = 120,
    [string[]]$GameArgs = @("-debug", "-quickstart", "start_tag=GER")
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
if ([string]::IsNullOrWhiteSpace($ProxySourceDir)) {
    $ProxySourceDir = $root
}
$bindings = Join-Path $TraceDir "shader_bindings.json"
$compiles = Join-Path $TraceDir "shader_compiles.json"
$runtimeLog = Join-Path $TraceDir "logs\d3d11_trace.jsonl"

New-Item -ItemType Directory -Force -Path $TraceDir | Out-Null
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $runtimeLog) | Out-Null
Set-Content -Encoding UTF8 $bindings "[]`n"
Set-Content -Encoding UTF8 $compiles "[]`n"
Set-Content -Encoding UTF8 $runtimeLog ""

try {
    & powershell -ExecutionPolicy Bypass -File (Join-Path $root "deploy_to_hoi4.ps1") -GameDir $GameDir -TraceDir $TraceDir -ProxySourceDir $ProxySourceDir

    $env:HOI4_TRACE_DIR = $TraceDir
    $exe = Join-Path $GameDir "hoi4.exe"
    $process = Start-Process -FilePath $exe -WorkingDirectory $GameDir -ArgumentList $GameArgs -PassThru
    Write-Host "Started hoi4 pid=$($process.Id)"

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $lastSize = -1
    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Seconds 5
        $size = 0
        if (Test-Path $bindings) { $size += (Get-Item $bindings).Length }
        if (Test-Path $compiles) { $size += (Get-Item $compiles).Length }
        if (Test-Path $runtimeLog) { $size += (Get-Item $runtimeLog).Length }
        Write-Host "trace_size=$size"

        $lastSize = $size

        if ($process.HasExited) {
            Write-Host "hoi4 exited code=$($process.ExitCode)"
            break
        }
    }

    if (-not $process.HasExited) {
        Stop-Process -Id $process.Id -Force
        Write-Host "Stopped hoi4 pid=$($process.Id)"
        Start-Sleep -Seconds 3
    }
} finally {
    & powershell -ExecutionPolicy Bypass -File (Join-Path $root "deploy_to_hoi4.ps1") -GameDir $GameDir -TraceDir $TraceDir -Restore
}

Write-Host "Wrote $compiles"
Write-Host "Wrote $bindings"
Write-Host "Wrote $runtimeLog"
