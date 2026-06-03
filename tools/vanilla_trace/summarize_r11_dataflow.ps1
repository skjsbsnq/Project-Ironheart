param(
    [string]$TraceDir = "C:\Users\19180\Documents\999\b1\tools\vanilla_trace",
    [string]$LogPath = "",
    [string]$OutputPath = ""
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($LogPath)) {
    $LogPath = Join-Path $TraceDir "logs\d3d11_trace.jsonl"
}
if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path $TraceDir "r11_dataflow.json"
}
if (-not (Test-Path -LiteralPath $LogPath)) {
    throw "Trace log does not exist: $LogPath"
}

$rxEvent = [regex]'"event":"([^"]+)"'
$rxSeq = [regex]'"seq":([0-9]+)'
$rxTick = [regex]'"tick_ms":([0-9]+)'
$rxResourceKind = [regex]'"kind":"(buffer|texture2d|unknown)"'
$rxBufferId = [regex]'"buffer_id":([0-9]+)'
$rxTextureId = [regex]'"texture_id":([0-9]+)'
$rxByteWidth = [regex]'"byte_width":([0-9]+)'
$rxWidth = [regex]'"width":([0-9]+)'
$rxHeight = [regex]'"height":([0-9]+)'
$rxFormat = [regex]'"format":"([^"]+)"'
$rxDigest = [regex]'"digest":\{([^}]*)\}'
$rxInitialDigest = [regex]'"initial_digest":\{([^}]*)\}'
$rxHash = [regex]'"hash":"([^"]*)"'
$rxSize = [regex]'"size":([0-9]+)'
$rxPrefixHex = [regex]'"prefix_hex":"([^"]*)"'
$rxMapType = [regex]'"map_type":([0-9]+)'
$rxSubresource = [regex]'"subresource":([0-9]+)'
$rxDstSubresource = [regex]'"dst_subresource":([0-9]+)'
$rxSrcSubresource = [regex]'"src_subresource":([0-9]+)'

function Match-String {
    param([regex]$Regex, [string]$Line)
    $m = $Regex.Match($Line)
    if ($m.Success) { return $m.Groups[1].Value }
    ""
}

function Match-Int64 {
    param([regex]$Regex, [string]$Line, [long]$Default = 0)
    $m = $Regex.Match($Line)
    if ($m.Success) { return [long]$m.Groups[1].Value }
    $Default
}

function Digest-From {
    param([regex]$Regex, [string]$Line)
    $m = $Regex.Match($Line)
    if (-not $m.Success) {
        return [ordered]@{}
    }
    $body = $m.Groups[1].Value
    return [ordered]@{
        size = Match-Int64 -Regex $script:rxSize -Line $body
        hash = Match-String -Regex $script:rxHash -Line $body
        prefix_hex = Match-String -Regex $script:rxPrefixHex -Line $body
    }
}

function Resource-Key {
    param([string]$Line)
    $kind = Match-String -Regex $script:rxResourceKind -Line $Line
    if ($kind -eq "buffer") {
        return "buffer:" + (Match-Int64 -Regex $script:rxBufferId -Line $Line)
    }
    if ($kind -eq "texture2d") {
        return "texture:" + (Match-Int64 -Regex $script:rxTextureId -Line $Line)
    }
    "unknown"
}

function Add-Count {
    param([hashtable]$Map, [string]$Key)
    if ([string]::IsNullOrWhiteSpace($Key)) { return }
    if (-not $Map.ContainsKey($Key)) { $Map[$Key] = [long]0 }
    $Map[$Key] = [long]$Map[$Key] + 1
}

$eventCounts = @{}
$resourceWriteCounts = @{}
$cbufferUpdates = New-Object System.Collections.Generic.List[object]
$textureUpdates = New-Object System.Collections.Generic.List[object]
$copies = New-Object System.Collections.Generic.List[object]
$clears = New-Object System.Collections.Generic.List[object]
$allWriteSamples = New-Object System.Collections.Generic.List[object]

$eventCount = 0L
Get-Content -LiteralPath $LogPath | ForEach-Object {
    $line = $_
    if ([string]::IsNullOrWhiteSpace($line)) { return }
    $event = Match-String -Regex $rxEvent -Line $line
    if ([string]::IsNullOrWhiteSpace($event)) { return }
    $eventCount++
    Add-Count -Map $eventCounts -Key $event

    $isWrite =
        $event -eq "ID3D11Device::CreateBuffer" -or
        $event -eq "ID3D11Device::CreateTexture2D" -or
        $event -eq "ID3D11DeviceContext::UpdateSubresource" -or
        $event -eq "ID3D11DeviceContext::Unmap" -or
        $event -eq "ID3D11DeviceContext::CopySubresourceRegion" -or
        $event -eq "ID3D11DeviceContext::CopyResource" -or
        $event -eq "ID3D11DeviceContext::ResolveSubresource" -or
        $event -eq "ID3D11DeviceContext::ClearRenderTargetView" -or
        $event -eq "ID3D11DeviceContext::ClearDepthStencilView"

    if (-not $isWrite) { return }

    $resourceKey = Resource-Key -Line $line
    Add-Count -Map $resourceWriteCounts -Key $resourceKey

    $sample = [ordered]@{
        seq = Match-Int64 -Regex $rxSeq -Line $line
        tick_ms = Match-Int64 -Regex $rxTick -Line $line
        event = $event
        resource = $resourceKey
        subresource = Match-Int64 -Regex $rxSubresource -Line $line -Default (Match-Int64 -Regex $rxDstSubresource -Line $line)
        format = Match-String -Regex $rxFormat -Line $line
        width = Match-Int64 -Regex $rxWidth -Line $line
        height = Match-Int64 -Regex $rxHeight -Line $line
        byte_width = Match-Int64 -Regex $rxByteWidth -Line $line
        map_type = Match-Int64 -Regex $rxMapType -Line $line
        digest = if ($event -eq "ID3D11Device::CreateBuffer" -or $event -eq "ID3D11Device::CreateTexture2D") {
            Digest-From -Regex $rxInitialDigest -Line $line
        } else {
            Digest-From -Regex $rxDigest -Line $line
        }
    }

    if ($allWriteSamples.Count -lt 2000) {
        $allWriteSamples.Add($sample)
    }

    if (($sample.resource -like "buffer:*") -and ($sample.digest.prefix_hex -or $sample.digest.hash)) {
        if ($cbufferUpdates.Count -lt 2000) {
            $cbufferUpdates.Add($sample)
        }
    } elseif (($sample.resource -like "texture:*") -and ($sample.digest.hash)) {
        if ($textureUpdates.Count -lt 2000) {
            $textureUpdates.Add($sample)
        }
    } elseif ($event -match "Copy|Resolve") {
        if ($copies.Count -lt 2000) {
            $copies.Add($sample)
        }
    } elseif ($event -match "Clear") {
        if ($clears.Count -lt 2000) {
            $clears.Add($sample)
        }
    }
}

$eventCountsOut = [ordered]@{}
$resourceWriteCountsOut = [ordered]@{}

$output = [ordered]@{}
$output.source_log = $LogPath
$output.generated_at = (Get-Date).ToString("s")
$output.event_count = $eventCount
$output.event_counts = $eventCountsOut
$output.resource_write_counts = $resourceWriteCountsOut
$output.cbuffer_update_samples = @($cbufferUpdates.ToArray())
$output.texture_update_samples = @($textureUpdates.ToArray())
$output.copy_samples = @($copies.ToArray())
$output.clear_samples = @($clears.ToArray())
$output.write_samples = @($allWriteSamples.ToArray())

foreach ($entry in ($eventCounts.GetEnumerator() | Sort-Object Name)) {
    $eventCountsOut[$entry.Key] = $entry.Value
}
foreach ($entry in ($resourceWriteCounts.GetEnumerator() | Sort-Object Value -Descending | Select-Object -First 500)) {
    $resourceWriteCountsOut[$entry.Key] = $entry.Value
}

New-Item -ItemType Directory -Force -Path (Split-Path -Parent $OutputPath) | Out-Null
$output | ConvertTo-Json -Depth 14 | Set-Content -Encoding UTF8 -LiteralPath $OutputPath
Write-Host "Wrote $OutputPath"
Write-Host "events=$eventCount cbuffer_samples=$($cbufferUpdates.Count) texture_samples=$($textureUpdates.Count) copies=$($copies.Count) clears=$($clears.Count)"
