$dir = "C:\Program Files (x86)\Steam\steamapps\common\Hearts of Iron IV\gfx\interface\topbar\toolbar"
$files = @(
    "topbar_decisionview_button.dds",
    "science_button.dds",
    "diplomacy_button.dds",
    "trade_button.dds",
    "construction_button.dds",
    "production_button.dds",
    "ledger_button.dds"
)
foreach ($name in $files) {
    $f = Join-Path $dir $name
    $b = [System.IO.File]::ReadAllBytes($f)
    $h = [BitConverter]::ToInt32($b, 12)
    $w = [BitConverter]::ToInt32($b, 16)
    $halfW = $w / 2
    # uncompressed BGRA8: data starts at offset 128
    # average alpha across left half vs right half (alpha == solidness)
    $rowStride = $w * 4
    $leftA = 0; $rightA = 0; $leftN = 0; $rightN = 0
    for ($y = 0; $y -lt $h; $y += 4) {
        for ($x = 0; $x -lt $halfW; $x += 4) {
            $off = 128 + $y * $rowStride + $x * 4 + 3
            if ($off -lt $b.Length) { $leftA += $b[$off]; $leftN++ }
        }
        for ($x = $halfW; $x -lt $w; $x += 4) {
            $off = 128 + $y * $rowStride + $x * 4 + 3
            if ($off -lt $b.Length) { $rightA += $b[$off]; $rightN++ }
        }
    }
    $leftAvg = if ($leftN -gt 0) { $leftA / $leftN } else { 0 }
    $rightAvg = if ($rightN -gt 0) { $rightA / $rightN } else { 0 }
    Write-Host ("{0,-32} {1}x{2} leftA={3:N1} rightA={4:N1}" -f $name, $w, $h, $leftAvg, $rightAvg)
}
