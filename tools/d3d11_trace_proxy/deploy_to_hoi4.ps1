param(
    [string]$GameDir = "C:\Program Files (x86)\Steam\steamapps\common\Hearts of Iron IV",
    [string]$TraceDir = "C:\Users\19180\Documents\999\b1\tools\vanilla_trace",
    [string]$ProxySourceDir = "",
    [switch]$Launch,
    [switch]$Restore
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
if ([string]::IsNullOrWhiteSpace($ProxySourceDir)) {
    $ProxySourceDir = $root
}
$dlls = @("d3d11.dll", "dxgi.dll", "d3dcompiler_47.dll", "d3dx9_43.dll")
$backupDir = Join-Path $GameDir ".hoi4_trace_proxy_backup"

function Assert-InGameDir {
    param([string]$Path)

    $resolvedGame = [System.IO.Path]::GetFullPath($GameDir)
    $resolvedPath = [System.IO.Path]::GetFullPath($Path)
    if (-not $resolvedPath.StartsWith($resolvedGame, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to touch path outside game directory: $resolvedPath"
    }
}

function Invoke-WithRetry {
    param(
        [scriptblock]$Action,
        [string]$Label,
        [int]$Attempts = 10,
        [int]$DelayMilliseconds = 500
    )

    for ($i = 1; $i -le $Attempts; $i++) {
        try {
            & $Action
            return
        } catch {
            if ($i -eq $Attempts) {
                throw
            }
            Write-Host "$Label failed, retrying ($i/$Attempts): $($_.Exception.Message)"
            Start-Sleep -Milliseconds $DelayMilliseconds
        }
    }
}

if (-not (Test-Path $GameDir)) {
    throw "Game directory does not exist: $GameDir"
}

if ($Restore) {
    foreach ($dll in $dlls) {
        $target = Join-Path $GameDir $dll
        $backup = Join-Path $backupDir $dll
        Assert-InGameDir $target
        if (Test-Path $backup) {
            Invoke-WithRetry { Copy-Item -Force $backup $target } "Restore $dll"
            Invoke-WithRetry { Remove-Item -Force $backup } "Remove backup $dll"
            Write-Host "Restored $dll"
        } elseif (Test-Path $target) {
            Invoke-WithRetry { Remove-Item -Force $target } "Remove proxy $dll"
            Write-Host "Removed proxy $dll"
        }
    }
    if (Test-Path $backupDir -PathType Container) {
        $remaining = Get-ChildItem -Force $backupDir
        if (-not $remaining) {
            Remove-Item -Force $backupDir
        }
    }
    return
}

New-Item -ItemType Directory -Force -Path $backupDir | Out-Null
New-Item -ItemType Directory -Force -Path $TraceDir | Out-Null

foreach ($dll in $dlls) {
    $source = Join-Path $ProxySourceDir $dll
    $target = Join-Path $GameDir $dll
    $backup = Join-Path $backupDir $dll

    if (-not (Test-Path $source)) {
        throw "Missing proxy DLL. Build first: $source"
    }

    Assert-InGameDir $target
    if ((Test-Path $target) -and -not (Test-Path $backup)) {
        Invoke-WithRetry { Copy-Item -Force $target $backup } "Backup $dll"
        Write-Host "Backed up existing $dll"
    }

    Invoke-WithRetry { Copy-Item -Force $source $target } "Deploy $dll"
    Write-Host "Deployed $dll"
}

$env:HOI4_TRACE_DIR = $TraceDir
Write-Host "HOI4_TRACE_DIR=$TraceDir"

if ($Launch) {
    $exe = Join-Path $GameDir "hoi4.exe"
    Start-Process -FilePath $exe -WorkingDirectory $GameDir -ArgumentList @("-debug", "-quickstart")
}
