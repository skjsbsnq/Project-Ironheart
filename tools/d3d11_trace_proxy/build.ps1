param(
    [string]$OutDir = "."
)

$ErrorActionPreference = "Stop"

$compiler = Get-Command g++ -ErrorAction Stop
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$out = Join-Path $root $OutDir
New-Item -ItemType Directory -Force -Path $out | Out-Null

$common = @(
    "-std=c++17",
    "-O2",
    "-Wall",
    "-Wextra",
    "-shared",
    "-static",
    "-static-libgcc",
    "-static-libstdc++",
    "-I$root"
)

function Invoke-ProxyBuild {
    param(
        [string]$OutputName,
        [string]$SourceName
    )

    & $compiler.Source @common `
        (Join-Path $root $SourceName) `
        (Join-Path $root "trace_common.cpp") `
        "-o" (Join-Path $out $OutputName)
    if ($LASTEXITCODE -ne 0) {
        throw "Build failed for $OutputName"
    }
}

Invoke-ProxyBuild "d3d11.dll" "proxy_d3d11.cpp"
Invoke-ProxyBuild "dxgi.dll" "proxy_dxgi.cpp"
Invoke-ProxyBuild "d3dcompiler_47.dll" "proxy_d3dcompiler_47.cpp"
Invoke-ProxyBuild "d3dx9_43.dll" "proxy_d3dx9_43.cpp"

Write-Host "Built proxy DLLs in $out"

$smoke = Join-Path $root "smoke_d3dcompile.cpp"
if (Test-Path $smoke) {
    & $compiler.Source "-std=c++17" "-O2" $smoke "-o" (Join-Path $out "smoke_d3dcompile.exe")
    if ($LASTEXITCODE -ne 0) {
        throw "Build failed for smoke_d3dcompile.exe"
    }
}
