param(
    [string]$TraceDir = "C:\Users\19180\Documents\999\b1\tools\vanilla_trace",
    [string]$LogPath = "",
    [string]$RuntimeTargetsPath = "",
    [string]$RenderPassesPath = "",
    [string]$ShaderBindingsPath = "",
    [string]$StateObjectsPath = "",
    [string]$ConstantBuffersPath = "",
    [string]$RepresentativeFramePath = "",
    [string]$PassResourceFlowPath = ""
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($LogPath)) {
    $LogPath = Join-Path $TraceDir "logs\d3d11_trace.jsonl"
}
if ([string]::IsNullOrWhiteSpace($RuntimeTargetsPath)) {
    $RuntimeTargetsPath = Join-Path $TraceDir "runtime_targets.json"
}
if ([string]::IsNullOrWhiteSpace($RenderPassesPath)) {
    $RenderPassesPath = Join-Path $TraceDir "render_passes.json"
}
if ([string]::IsNullOrWhiteSpace($ShaderBindingsPath)) {
    $ShaderBindingsPath = Join-Path $TraceDir "shader_bindings.json"
}
if ([string]::IsNullOrWhiteSpace($StateObjectsPath)) {
    $StateObjectsPath = Join-Path $TraceDir "state_objects.json"
}
if ([string]::IsNullOrWhiteSpace($ConstantBuffersPath)) {
    $ConstantBuffersPath = Join-Path $TraceDir "constant_buffers.json"
}
if ([string]::IsNullOrWhiteSpace($RepresentativeFramePath)) {
    $RepresentativeFramePath = Join-Path $TraceDir "representative_frame.json"
}
if ([string]::IsNullOrWhiteSpace($PassResourceFlowPath)) {
    $PassResourceFlowPath = Join-Path $TraceDir "pass_resource_flow.json"
}
if (-not (Test-Path $LogPath)) {
    throw "Trace log does not exist: $LogPath"
}

$rxEvent = [regex]'"event":"([^"]+)"'
$rxSeq = [regex]'"seq":([0-9]+)'
$rxTexture = [regex]'"texture":"([^"]*)"'
$rxTextureId = [regex]'"texture_id":([0-9]+)'
$rxView = [regex]'"view":"([^"]*)"'
$rxViewId = [regex]'"view_id":([0-9]+)'
$rxResource = [regex]'"resource":"([^"]*)"'
$rxBuffer = [regex]'"buffer":"([^"]*)"'
$rxBufferId = [regex]'"buffer_id":([0-9]+)'
$rxByteWidth = [regex]'"byte_width":([0-9]+)'
$rxStructureByteStride = [regex]'"structure_byte_stride":([0-9]+)'
$rxState = [regex]'"state":"([^"]*)"'
$rxStateId = [regex]'"state_id":([0-9]+)'
$rxKind = [regex]'"kind":"([^"]*)"'
$rxFormat = [regex]'"format":"([^"]*)"'
$rxFormatId = [regex]'"format_id":([0-9]+)'
$rxViewDimension = [regex]'"view_dimension":([0-9]+)'
$rxWidth = [regex]'"width":([0-9]+)'
$rxHeight = [regex]'"height":([0-9]+)'
$rxMipLevels = [regex]'"mip_levels":([0-9]+)'
$rxArraySize = [regex]'"array_size":([0-9]+)'
$rxSampleCount = [regex]'"sample_count":([0-9]+)'
$rxUsage = [regex]'"usage":([0-9]+)'
$rxBindFlags = [regex]'"bind_flags":([0-9]+)'
$rxCpuAccessFlags = [regex]'"cpu_access_flags":([0-9]+)'
$rxMiscFlags = [regex]'"misc_flags":([0-9]+)'
$rxInitialData = [regex]'"initial_data":(true|false)'
$rxContext = [regex]'"context":"([^"]*)"'
$rxShader = [regex]'"shader":"([^"]*)"'
$rxHash = [regex]'"bytecode_hash":"([^"]*)"'
$rxVsHash = [regex]'"vs_hash":"([^"]*)"'
$rxPsHash = [regex]'"ps_hash":"([^"]*)"'
$rxDsv = [regex]'"dsv":"([^"]*)"'
$rxRtvs = [regex]'"rtvs":\[(.*?)\]'
$rxJsonString = [regex]'"([^"]*)"'
$rxChangedView = [regex]'\{"slot":([0-9]+),"view":"([^"]*)"\}'
$rxChangedBuffer = [regex]'\{"slot":([0-9]+),"buffer":"([^"]*)"\}'
$rxChangedSampler = [regex]'\{"slot":([0-9]+),"sampler":"([^"]*)"\}'
$rxChangedValue = [regex]'\{"slot":([0-9]+),"value":"([^"]*)"\}'
$rxBlendState = [regex]'"blend_state":"([^"]*)"'
$rxDepthStencilState = [regex]'"depth_stencil_state":"([^"]*)"'
$rxRasterizerState = [regex]'"rasterizer_state":"([^"]*)"'
$rxStencilRef = [regex]'"stencil_ref":([0-9]+)'
$rxBlendSampleMask = [regex]'"blend_sample_mask":([0-9]+)'

function Add-Count {
    param(
        [hashtable]$Map,
        [string]$Key,
        [long]$Value = 1
    )
    if ([string]::IsNullOrWhiteSpace($Key)) {
        return
    }
    if (-not $Map.ContainsKey($Key)) {
        $Map[$Key] = [long]0
    }
    $Map[$Key] = [long]$Map[$Key] + $Value
}

function Ordered {
    param([hashtable]$Map)
    $out = [ordered]@{}
    foreach ($key in ($Map.Keys | Sort-Object)) {
        $out[$key] = $Map[$key]
    }
    $out
}

function Top-Ordered {
    param(
        [hashtable]$Map,
        [int]$Limit = 200
    )
    $out = [ordered]@{}
    foreach ($entry in ($Map.GetEnumerator() | Sort-Object Value -Descending | Select-Object -First $Limit)) {
        $out[$entry.Key] = $entry.Value
    }
    $out
}

function Match-String {
    param([regex]$Regex, [string]$Line)
    $m = $Regex.Match($Line)
    if ($m.Success) {
        return $m.Groups[1].Value
    }
    ""
}

function Match-Int64 {
    param([regex]$Regex, [string]$Line, [long]$Default = 0)
    $m = $Regex.Match($Line)
    if ($m.Success) {
        return [long]$m.Groups[1].Value
    }
    $Default
}

function Match-Bool {
    param([regex]$Regex, [string]$Line)
    $m = $Regex.Match($Line)
    if ($m.Success) {
        return $m.Groups[1].Value -eq "true"
    }
    $false
}

function Match-StringArray {
    param([regex]$Regex, [string]$Line)
    $m = $Regex.Match($Line)
    if (-not $m.Success) {
        return @()
    }
    $items = @()
    foreach ($item in $script:rxJsonString.Matches($m.Groups[1].Value)) {
        $items += $item.Groups[1].Value
    }
    $items
}

function Srv-State-Array {
    param([hashtable]$Map)
    $out = @()
    foreach ($slot in ($Map.Keys | Sort-Object {[int]$_})) {
        $view = [string]$Map[$slot]
        if (-not [string]::IsNullOrWhiteSpace($view)) {
            $out += [ordered]@{
                slot = [int]$slot
                view = $view
            }
        }
    }
    $out
}

function Slot-State-Array {
    param([hashtable]$Map)
    $out = @()
    foreach ($slot in ($Map.Keys | Sort-Object {[int]$_})) {
        $value = [string]$Map[$slot]
        if (-not [string]::IsNullOrWhiteSpace($value)) {
            $out += [ordered]@{
                slot = [int]$slot
                value = $value
            }
        }
    }
    $out
}

function Classify-PassKind {
    param([string[]]$ResourceNames)

    $joined = (@($ResourceNames) -join "|")
    if ([string]::IsNullOrWhiteSpace($joined)) {
        return "unknown"
    }
    if ($joined -match "BaseLUT|BlendLUT") {
        return "lut_blend"
    }
    if ($joined -match "MainScene|RestoreBloom|ColorCube|AverageLuminance|BloomSource|BlurSample|LastLuminance|Scene_Texture") {
        return "postfx"
    }
    if ($joined -match "BorderDiffuse") {
        return "border"
    }
    if ($joined -match "RiverData") {
        return "river"
    }
    if ($joined -match "WaterColor|WaterRefraction|IceDiffuse|IceNoise|LeanTexture1|LeanTexture2|HeightTexture|ReflectionCubeMap") {
        return "water"
    }
    if ($joined -match "TerrainDiffuse|TerrainIDMap|TerrainColorTint|TexMask|TexPattern|ProvinceSecondaryColorMap|GradientBorderChannel|HeightMap|TerrainNormal|HeightNormal|SnowMudTexture") {
        return "terrain"
    }
    if ($joined -match "TreeMaskTexture|SeasonMap|TintMap") {
        return "tree"
    }
    if ($joined -match "LightIndexMap|LightDataMap|DiffuseMap|SpecularMap|NormalMap|FOWNoise|FOWHeight") {
        return "pdxmesh"
    }
    if ($joined -match "MapTexture|SimpleTexture|TextureOne|TextureTwo|BaseTexture|MaskTexture") {
        return "ui_or_sprite"
    }
    "unknown"
}

function Pass-Kind-Score {
    param([string]$Kind)
    switch ($Kind) {
        "terrain" { 10; return }
        "water" { 10; return }
        "river" { 10; return }
        "border" { 8; return }
        "postfx" { 6; return }
        "lut_blend" { 5; return }
        "pdxmesh" { 4; return }
        default { 1; return }
    }
}

function Load-ShaderBindingMap {
    param([string]$Path)

    $map = @{}
    if (-not (Test-Path $Path)) {
        return $map
    }

    $items = Get-Content -Raw -LiteralPath $Path | ConvertFrom-Json
    foreach ($item in $items) {
        $hash = [string]$item.bytecode_hash
        if ([string]::IsNullOrWhiteSpace($hash) -or $map.ContainsKey($hash)) {
            continue
        }
        $names = @($item.resources | ForEach-Object { [string]$_.name } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
        $map[$hash] = [pscustomobject]@{
            stage = [string]$item.stage
            bytecode_size = [int]$item.bytecode_size
            resource_names = $names
            pass_kind = Classify-PassKind -ResourceNames $names
        }
    }
    $map
}

function Close-StatePass {
    param([long]$EndLine)

    if (-not $script:currentStatePass) {
        return
    }
    if ($EndLine -lt [long]$script:currentStatePass.start_line) {
        $EndLine = [long]$script:currentStatePass.start_line
    }
    $script:currentStatePass.end_line = $EndLine
    $script:currentStatePass.ps_srvs = @(Srv-State-Array -Map $script:statePsSrvs)
    $script:currentStatePass.vs_cbuffers = @(Slot-State-Array -Map $script:stateVsCbuffers)
    $script:currentStatePass.ps_cbuffers = @(Slot-State-Array -Map $script:statePsCbuffers)
    $script:currentStatePass.ps_samplers = @(Slot-State-Array -Map $script:statePsSamplers)
    $script:currentStatePass.blend_state = [string]$script:stateBlendState
    $script:currentStatePass.depth_stencil_state = [string]$script:stateDepthStencilState
    $script:currentStatePass.rasterizer_state = [string]$script:stateRasterizerState
    $script:currentStatePass.stencil_ref = [int]$script:stateStencilRef
    $script:currentStatePass.blend_sample_mask = [long]$script:stateBlendSampleMask

    $script:statePassCount += 1
    $kind = [string]$script:currentStatePass.pass_kind
    Add-Count -Map $script:stateKindCounts -Key $kind

    $sig = "$kind|$(@($script:currentStatePass.rtvs) -join '+')|$($script:currentStatePass.dsv)|$($script:currentStatePass.vs_hash)|$($script:currentStatePass.ps_hash)"
    Add-Count -Map $script:stateSignatureCounts -Key $sig

    $script:currentFrameScore += (Pass-Kind-Score -Kind $kind)
    if ($script:currentFramePasses.Count -lt $script:maxRepresentativePasses) {
        $script:currentFramePasses.Add([pscustomobject]$script:currentStatePass) | Out-Null
    }

    $script:currentStatePass = $null
}

function Start-StatePass {
    param(
        [long]$LineNo,
        [string]$TriggerEvent
    )

    if ([string]::IsNullOrWhiteSpace($script:statePsHash)) {
        return
    }

    $shader = $script:shaderMap[$script:statePsHash]
    $resourceNames = if ($shader) { @($shader.resource_names) } else { @() }
    $passKind = if ($shader) { [string]$shader.pass_kind } else { "unknown" }

    $script:currentStatePass = [ordered]@{
        frame = [int]$script:stateFrame
        start_line = [long]$LineNo
        end_line = [long]$LineNo
        trigger_event = $TriggerEvent
        pass_kind = $passKind
        rtvs = @($script:stateRtvs)
        dsv = [string]$script:stateDsv
        vs_hash = [string]$script:stateVsHash
        ps_hash = [string]$script:statePsHash
        ps_srvs = @(Srv-State-Array -Map $script:statePsSrvs)
        vs_cbuffers = @(Slot-State-Array -Map $script:stateVsCbuffers)
        ps_cbuffers = @(Slot-State-Array -Map $script:statePsCbuffers)
        ps_samplers = @(Slot-State-Array -Map $script:statePsSamplers)
        blend_state = [string]$script:stateBlendState
        depth_stencil_state = [string]$script:stateDepthStencilState
        rasterizer_state = [string]$script:stateRasterizerState
        stencil_ref = [int]$script:stateStencilRef
        blend_sample_mask = [long]$script:stateBlendSampleMask
        pixel_shader_resources = $resourceNames
    }
}

function Finalize-Frame {
    if ($script:currentFramePasses.Count -eq 0) {
        return
    }
    if ($script:currentFrameScore -gt $script:bestFrameScore) {
        $script:bestFrameScore = $script:currentFrameScore
        $script:bestFrame = $script:stateFrame
        $script:bestFramePasses = @($script:currentFramePasses.ToArray())
    }
}

function Reset-Frame {
    $script:currentFramePasses = New-Object System.Collections.Generic.List[object]
    $script:currentFrameScore = 0
}

$shaderMap = Load-ShaderBindingMap -Path $ShaderBindingsPath
$script:shaderMap = $shaderMap
$script:rxJsonString = $rxJsonString
$script:maxRepresentativePasses = 500

$textures = @{}
$buffers = @{}
$views = @{}
$stateObjects = @{}
$targetStateBindCounts = @{}
$srvUse = @{}
$constantBufferUse = @{}
$constantBufferStageSlotUse = @{}
$samplerUse = @{}
$stateObjectBindUse = @{}
$drawCountsByTarget = @{}
$drawCountsByShader = @{}
$stateKindCounts = @{}
$stateSignatureCounts = @{}
$drawPasses = New-Object System.Collections.Generic.List[object]
$currentDrawPass = $null

$script:stateKindCounts = $stateKindCounts
$script:stateSignatureCounts = $stateSignatureCounts
$script:currentStatePass = $null
$script:stateFrame = 1
$script:stateVsHash = ""
$script:statePsHash = ""
$script:stateRtvs = @()
$script:stateDsv = ""
$script:statePsSrvs = @{}
$script:stateVsCbuffers = @{}
$script:statePsCbuffers = @{}
$script:statePsSamplers = @{}
$script:stateBlendState = ""
$script:stateDepthStencilState = ""
$script:stateRasterizerState = ""
$script:stateStencilRef = 0
$script:stateBlendSampleMask = 0xffffffff
$script:statePassCount = 0
$script:bestFrame = $null
$script:bestFrameScore = -1
$script:bestFramePasses = @()
Reset-Frame

$eventCount = 0
$presentCount = 0
$lineNo = 0

$drawEventNames = @{
    "ID3D11DeviceContext::Draw" = $true
    "ID3D11DeviceContext::DrawIndexed" = $true
    "ID3D11DeviceContext::DrawInstanced" = $true
    "ID3D11DeviceContext::DrawIndexedInstanced" = $true
    "ID3D11DeviceContext::DrawAuto" = $true
    "ID3D11DeviceContext::DrawIndexedInstancedIndirect" = $true
    "ID3D11DeviceContext::DrawInstancedIndirect" = $true
}

foreach ($line in [System.IO.File]::ReadLines((Resolve-Path $LogPath))) {
    $lineNo += 1
    if ([string]::IsNullOrWhiteSpace($line)) {
        continue
    }
    $eventMatch = $rxEvent.Match($line)
    if (-not $eventMatch.Success) {
        continue
    }

    $eventCount += 1
    $eventName = $eventMatch.Groups[1].Value

    switch ($eventName) {
        "ID3D11Device::CreateTexture2D" {
            $id = Match-String -Regex $rxTextureId -Line $line
            if (-not [string]::IsNullOrWhiteSpace($id)) {
                $bindNames = Match-StringArray -Regex ([regex]'"bind_flag_names":\[(.*?)\]') -Line $line
                $textures[$id] = [ordered]@{
                    texture_id = [long]$id
                    ptr = Match-String -Regex $rxTexture -Line $line
                    width = [int](Match-Int64 -Regex $rxWidth -Line $line)
                    height = [int](Match-Int64 -Regex $rxHeight -Line $line)
                    mip_levels = [int](Match-Int64 -Regex $rxMipLevels -Line $line)
                    array_size = [int](Match-Int64 -Regex $rxArraySize -Line $line)
                    format = Match-String -Regex $rxFormat -Line $line
                    format_id = [int](Match-Int64 -Regex $rxFormatId -Line $line)
                    sample_count = [int](Match-Int64 -Regex $rxSampleCount -Line $line)
                    usage = [int](Match-Int64 -Regex $rxUsage -Line $line)
                    bind_flags = [int](Match-Int64 -Regex $rxBindFlags -Line $line)
                    bind_flag_names = @($bindNames)
                    cpu_access_flags = [int](Match-Int64 -Regex $rxCpuAccessFlags -Line $line)
                    misc_flags = [int](Match-Int64 -Regex $rxMiscFlags -Line $line)
                    initial_data = [bool](Match-Bool -Regex $rxInitialData -Line $line)
                    first_seq = [long](Match-Int64 -Regex $rxSeq -Line $line)
                    first_line = [long]$lineNo
                }
            }
        }
        "ID3D11Device::CreateBuffer" {
            $id = Match-String -Regex $rxBufferId -Line $line
            if (-not [string]::IsNullOrWhiteSpace($id)) {
                $bindNames = Match-StringArray -Regex ([regex]'"bind_flag_names":\[(.*?)\]') -Line $line
                $buffers[$id] = [ordered]@{
                    buffer_id = [long]$id
                    ptr = Match-String -Regex $rxBuffer -Line $line
                    byte_width = [int](Match-Int64 -Regex $rxByteWidth -Line $line)
                    usage = [int](Match-Int64 -Regex $rxUsage -Line $line)
                    bind_flags = [int](Match-Int64 -Regex $rxBindFlags -Line $line)
                    bind_flag_names = @($bindNames)
                    cpu_access_flags = [int](Match-Int64 -Regex $rxCpuAccessFlags -Line $line)
                    misc_flags = [int](Match-Int64 -Regex $rxMiscFlags -Line $line)
                    structure_byte_stride = [int](Match-Int64 -Regex $rxStructureByteStride -Line $line)
                    initial_data = [bool](Match-Bool -Regex $rxInitialData -Line $line)
                    first_seq = [long](Match-Int64 -Regex $rxSeq -Line $line)
                    first_line = [long]$lineNo
                }
            }
        }
        "ID3D11Device::CreateSamplerState" {
            $id = Match-String -Regex $rxStateId -Line $line
            if (-not [string]::IsNullOrWhiteSpace($id)) {
                $stateObjects[$id] = [ordered]@{
                    state_id = [long]$id
                    ptr = Match-String -Regex $rxState -Line $line
                    kind = "SamplerState"
                    desc_json = $line
                    first_seq = [long](Match-Int64 -Regex $rxSeq -Line $line)
                    first_line = [long]$lineNo
                }
            }
        }
        "ID3D11Device::CreateBlendState" {
            $id = Match-String -Regex $rxStateId -Line $line
            if (-not [string]::IsNullOrWhiteSpace($id)) {
                $stateObjects[$id] = [ordered]@{
                    state_id = [long]$id
                    ptr = Match-String -Regex $rxState -Line $line
                    kind = "BlendState"
                    desc_json = $line
                    first_seq = [long](Match-Int64 -Regex $rxSeq -Line $line)
                    first_line = [long]$lineNo
                }
            }
        }
        "ID3D11Device::CreateDepthStencilState" {
            $id = Match-String -Regex $rxStateId -Line $line
            if (-not [string]::IsNullOrWhiteSpace($id)) {
                $stateObjects[$id] = [ordered]@{
                    state_id = [long]$id
                    ptr = Match-String -Regex $rxState -Line $line
                    kind = "DepthStencilState"
                    desc_json = $line
                    first_seq = [long](Match-Int64 -Regex $rxSeq -Line $line)
                    first_line = [long]$lineNo
                }
            }
        }
        "ID3D11Device::CreateRasterizerState" {
            $id = Match-String -Regex $rxStateId -Line $line
            if (-not [string]::IsNullOrWhiteSpace($id)) {
                $stateObjects[$id] = [ordered]@{
                    state_id = [long]$id
                    ptr = Match-String -Regex $rxState -Line $line
                    kind = "RasterizerState"
                    desc_json = $line
                    first_seq = [long](Match-Int64 -Regex $rxSeq -Line $line)
                    first_line = [long]$lineNo
                }
            }
        }
        "ID3D11Device::CreateShaderResourceView" {
            $id = Match-String -Regex $rxViewId -Line $line
            if (-not [string]::IsNullOrWhiteSpace($id)) {
                $views[$id] = [ordered]@{
                    view_id = [long]$id
                    kind = "SRV"
                    ptr = Match-String -Regex $rxView -Line $line
                    resource = Match-String -Regex $rxResource -Line $line
                    texture_id = [long](Match-Int64 -Regex $rxTextureId -Line $line)
                    format = Match-String -Regex $rxFormat -Line $line
                    format_id = [int](Match-Int64 -Regex $rxFormatId -Line $line)
                    view_dimension = [int](Match-Int64 -Regex $rxViewDimension -Line $line)
                    first_seq = [long](Match-Int64 -Regex $rxSeq -Line $line)
                    first_line = [long]$lineNo
                }
            }
        }
        "ID3D11Device::CreateRenderTargetView" {
            $id = Match-String -Regex $rxViewId -Line $line
            if (-not [string]::IsNullOrWhiteSpace($id)) {
                $views[$id] = [ordered]@{
                    view_id = [long]$id
                    kind = "RTV"
                    ptr = Match-String -Regex $rxView -Line $line
                    resource = Match-String -Regex $rxResource -Line $line
                    texture_id = [long](Match-Int64 -Regex $rxTextureId -Line $line)
                    format = Match-String -Regex $rxFormat -Line $line
                    format_id = [int](Match-Int64 -Regex $rxFormatId -Line $line)
                    view_dimension = [int](Match-Int64 -Regex $rxViewDimension -Line $line)
                    first_seq = [long](Match-Int64 -Regex $rxSeq -Line $line)
                    first_line = [long]$lineNo
                }
            }
        }
        "ID3D11Device::CreateDepthStencilView" {
            $id = Match-String -Regex $rxViewId -Line $line
            if (-not [string]::IsNullOrWhiteSpace($id)) {
                $views[$id] = [ordered]@{
                    view_id = [long]$id
                    kind = "DSV"
                    ptr = Match-String -Regex $rxView -Line $line
                    resource = Match-String -Regex $rxResource -Line $line
                    texture_id = [long](Match-Int64 -Regex $rxTextureId -Line $line)
                    format = Match-String -Regex $rxFormat -Line $line
                    format_id = [int](Match-Int64 -Regex $rxFormatId -Line $line)
                    view_dimension = [int](Match-Int64 -Regex $rxViewDimension -Line $line)
                    first_seq = [long](Match-Int64 -Regex $rxSeq -Line $line)
                    first_line = [long]$lineNo
                }
            }
        }
        "IDXGISwapChain::Present" {
            Close-StatePass -EndLine ([long]$lineNo - 1)
            Finalize-Frame
            Reset-Frame
            $presentCount += 1
            $script:stateFrame = $presentCount + 1
            if ($currentDrawPass) {
                $currentDrawPass.end_line = [long]$lineNo - 1
                $drawPasses.Add([pscustomobject]$currentDrawPass) | Out-Null
                $currentDrawPass = $null
            }
        }
        "ID3D11DeviceContext::OMSetRenderTargets" {
            Close-StatePass -EndLine ([long]$lineNo - 1)
            $script:stateRtvs = @(Match-StringArray -Regex $rxRtvs -Line $line)
            $script:stateDsv = Match-String -Regex $rxDsv -Line $line
            foreach ($rtv in $script:stateRtvs) {
                Add-Count -Map $targetStateBindCounts -Key ([string]$rtv)
            }
            Add-Count -Map $targetStateBindCounts -Key ([string]$script:stateDsv)
        }
        "ID3D11DeviceContext::OMSetRenderTargetsAndUnorderedAccessViews" {
            Close-StatePass -EndLine ([long]$lineNo - 1)
            if ($line -notmatch '"keep_rt_and_dsv":true') {
                $script:stateRtvs = @(Match-StringArray -Regex $rxRtvs -Line $line)
                $script:stateDsv = Match-String -Regex $rxDsv -Line $line
                foreach ($rtv in $script:stateRtvs) {
                    Add-Count -Map $targetStateBindCounts -Key ([string]$rtv)
                }
                Add-Count -Map $targetStateBindCounts -Key ([string]$script:stateDsv)
            }
        }
        "ID3D11DeviceContext::VSSetShader" {
            $hash = Match-String -Regex $rxHash -Line $line
            if ($hash -ne $script:stateVsHash) {
                Close-StatePass -EndLine ([long]$lineNo - 1)
                $script:stateVsHash = $hash
            }
        }
        "ID3D11DeviceContext::PSSetShader" {
            Close-StatePass -EndLine ([long]$lineNo - 1)
            $script:statePsHash = Match-String -Regex $rxHash -Line $line
            Start-StatePass -LineNo ([long]$lineNo) -TriggerEvent $eventName
        }
        "ID3D11DeviceContext::PSSetShaderResources" {
            foreach ($m in $rxChangedView.Matches($line)) {
                $slot = [int]$m.Groups[1].Value
                $viewId = $m.Groups[2].Value
                if ([string]::IsNullOrWhiteSpace($viewId)) {
                    $script:statePsSrvs.Remove($slot)
                } else {
                    $script:statePsSrvs[$slot] = $viewId
                    Add-Count -Map $srvUse -Key $viewId
                }
            }
        }
        "ID3D11DeviceContext::VSSetConstantBuffers" {
            foreach ($m in $rxChangedBuffer.Matches($line)) {
                $slot = [int]$m.Groups[1].Value
                $bufferId = $m.Groups[2].Value
                if ([string]::IsNullOrWhiteSpace($bufferId)) {
                    $script:stateVsCbuffers.Remove($slot)
                } else {
                    $script:stateVsCbuffers[$slot] = $bufferId
                    Add-Count -Map $constantBufferUse -Key $bufferId
                    Add-Count -Map $constantBufferStageSlotUse -Key "VS:b${slot}:$bufferId"
                }
            }
        }
        "ID3D11DeviceContext::PSSetConstantBuffers" {
            foreach ($m in $rxChangedBuffer.Matches($line)) {
                $slot = [int]$m.Groups[1].Value
                $bufferId = $m.Groups[2].Value
                if ([string]::IsNullOrWhiteSpace($bufferId)) {
                    $script:statePsCbuffers.Remove($slot)
                } else {
                    $script:statePsCbuffers[$slot] = $bufferId
                    Add-Count -Map $constantBufferUse -Key $bufferId
                    Add-Count -Map $constantBufferStageSlotUse -Key "PS:b${slot}:$bufferId"
                }
            }
        }
        "ID3D11DeviceContext::PSSetSamplers" {
            foreach ($m in $rxChangedSampler.Matches($line)) {
                $slot = [int]$m.Groups[1].Value
                $samplerId = $m.Groups[2].Value
                if ([string]::IsNullOrWhiteSpace($samplerId)) {
                    $script:statePsSamplers.Remove($slot)
                } else {
                    $script:statePsSamplers[$slot] = $samplerId
                    Add-Count -Map $samplerUse -Key $samplerId
                }
            }
        }
        "ID3D11DeviceContext::OMSetBlendState" {
            $script:stateBlendState = Match-String -Regex $rxState -Line $line
            $script:stateBlendSampleMask = Match-Int64 -Regex ([regex]'"sample_mask":([0-9]+)') -Line $line -Default 0
            Add-Count -Map $stateObjectBindUse -Key $script:stateBlendState
        }
        "ID3D11DeviceContext::OMSetDepthStencilState" {
            $script:stateDepthStencilState = Match-String -Regex $rxState -Line $line
            $script:stateStencilRef = [int](Match-Int64 -Regex $rxStencilRef -Line $line -Default 0)
            Add-Count -Map $stateObjectBindUse -Key $script:stateDepthStencilState
        }
        "ID3D11DeviceContext::RSSetState" {
            $script:stateRasterizerState = Match-String -Regex $rxState -Line $line
            Add-Count -Map $stateObjectBindUse -Key $script:stateRasterizerState
        }
        default {
            if ($drawEventNames.ContainsKey($eventName)) {
                $rtvs = @(Match-StringArray -Regex $rxRtvs -Line $line)
                $dsv = Match-String -Regex $rxDsv -Line $line
                $vsHash = Match-String -Regex $rxVsHash -Line $line
                $psHash = Match-String -Regex $rxPsHash -Line $line
                $passKey = "$(@($rtvs) -join '+')|$dsv|$vsHash|$psHash"
                if (-not $currentDrawPass -or $currentDrawPass.key -ne $passKey) {
                    if ($currentDrawPass) {
                        $currentDrawPass.end_line = [long]$lineNo - 1
                        $drawPasses.Add([pscustomobject]$currentDrawPass) | Out-Null
                    }
                    $currentDrawPass = [ordered]@{
                        key = $passKey
                        start_seq = [long](Match-Int64 -Regex $rxSeq -Line $line)
                        end_seq = [long](Match-Int64 -Regex $rxSeq -Line $line)
                        start_line = [long]$lineNo
                        end_line = [long]$lineNo
                        draw_count = 0
                        indexed_draw_count = 0
                        instanced_draw_count = 0
                        rtvs = @($rtvs)
                        dsv = $dsv
                        vs_hash = $vsHash
                        ps_hash = $psHash
                        ps_srvs = @(foreach ($m in $rxChangedView.Matches($line)) {
                            [ordered]@{
                                slot = [int]$m.Groups[1].Value
                                view = $m.Groups[2].Value
                            }
                        })
                        vs_cbuffers = @(foreach ($m in $rxChangedValue.Matches((Match-String -Regex ([regex]'"vs_cbuffers":\[(.*?)\]') -Line $line))) {
                            [ordered]@{ slot = [int]$m.Groups[1].Value; value = $m.Groups[2].Value }
                        })
                        ps_cbuffers = @(foreach ($m in $rxChangedValue.Matches((Match-String -Regex ([regex]'"ps_cbuffers":\[(.*?)\]') -Line $line))) {
                            [ordered]@{ slot = [int]$m.Groups[1].Value; value = $m.Groups[2].Value }
                        })
                        ps_samplers = @(foreach ($m in $rxChangedValue.Matches((Match-String -Regex ([regex]'"ps_samplers":\[(.*?)\]') -Line $line))) {
                            [ordered]@{ slot = [int]$m.Groups[1].Value; value = $m.Groups[2].Value }
                        })
                        blend_state = Match-String -Regex $rxBlendState -Line $line
                        depth_stencil_state = Match-String -Regex $rxDepthStencilState -Line $line
                        rasterizer_state = Match-String -Regex $rxRasterizerState -Line $line
                        stencil_ref = [int](Match-Int64 -Regex $rxStencilRef -Line $line -Default 0)
                        blend_sample_mask = [long](Match-Int64 -Regex $rxBlendSampleMask -Line $line -Default 0)
                        first_draw_event = $eventName
                    }
                }
                $currentDrawPass.draw_count += 1
                $currentDrawPass.end_seq = [long](Match-Int64 -Regex $rxSeq -Line $line)
                $currentDrawPass.end_line = [long]$lineNo
                if ($eventName -like "*Indexed*") {
                    $currentDrawPass.indexed_draw_count += 1
                }
                if ($eventName -like "*Instanced*") {
                    $currentDrawPass.instanced_draw_count += 1
                }
                foreach ($rtv in $rtvs) {
                    Add-Count -Map $drawCountsByTarget -Key ([string]$rtv)
                }
                Add-Count -Map $drawCountsByShader -Key $psHash
            }
        }
    }
}

Close-StatePass -EndLine ([long]$lineNo)
Finalize-Frame
if ($currentDrawPass) {
    $drawPasses.Add([pscustomobject]$currentDrawPass) | Out-Null
}

$targets = foreach ($viewId in ($views.Keys | Sort-Object {[long]$_})) {
    $view = $views[$viewId]
    if ($view.kind -notin @("RTV", "DSV")) {
        continue
    }
    $texture = $textures[[string]$view.texture_id]
    [ordered]@{
        view_id = $view.view_id
        kind = $view.kind
        texture_id = $view.texture_id
        ptr = $view.ptr
        resource = $view.resource
        format = $view.format
        format_id = $view.format_id
        view_dimension = $view.view_dimension
        width = if ($texture) { $texture.width } else { $null }
        height = if ($texture) { $texture.height } else { $null }
        mip_levels = if ($texture) { $texture.mip_levels } else { $null }
        array_size = if ($texture) { $texture.array_size } else { $null }
        sample_count = if ($texture) { $texture.sample_count } else { $null }
        bind_flags = if ($texture) { $texture.bind_flags } else { $null }
        bind_flag_names = if ($texture) { $texture.bind_flag_names } else { @() }
        state_bind_count = if ($targetStateBindCounts.ContainsKey($viewId)) { $targetStateBindCounts[$viewId] } else { 0 }
        draw_bind_count = if ($drawCountsByTarget.ContainsKey($viewId)) { $drawCountsByTarget[$viewId] } else { 0 }
        first_seq = $view.first_seq
        first_line = $view.first_line
    }
}

$srvTargets = foreach ($viewId in ($views.Keys | Sort-Object {[long]$_})) {
    $view = $views[$viewId]
    if ($view.kind -ne "SRV") {
        continue
    }
    $texture = $textures[[string]$view.texture_id]
    [ordered]@{
        view_id = $view.view_id
        kind = $view.kind
        texture_id = $view.texture_id
        ptr = $view.ptr
        resource = $view.resource
        format = $view.format
        format_id = $view.format_id
        view_dimension = $view.view_dimension
        width = if ($texture) { $texture.width } else { $null }
        height = if ($texture) { $texture.height } else { $null }
        bind_flags = if ($texture) { $texture.bind_flags } else { $null }
        bind_flag_names = if ($texture) { $texture.bind_flag_names } else { @() }
        ps_bind_count = if ($srvUse.ContainsKey($viewId)) { $srvUse[$viewId] } else { 0 }
        first_seq = $view.first_seq
        first_line = $view.first_line
    }
}

$representativeOrder = @()
$order = 0
foreach ($pass in @($script:bestFramePasses)) {
    $order += 1
    $representativeOrder += [ordered]@{
        order = $order
        frame = [int]$pass.frame
        start_line = [long]$pass.start_line
        end_line = [long]$pass.end_line
        pass_kind = [string]$pass.pass_kind
        rtvs = @($pass.rtvs)
        dsv = [string]$pass.dsv
        vs_hash = [string]$pass.vs_hash
        ps_hash = [string]$pass.ps_hash
        ps_srvs = @($pass.ps_srvs)
        vs_cbuffers = @($pass.vs_cbuffers)
        ps_cbuffers = @($pass.ps_cbuffers)
        ps_samplers = @($pass.ps_samplers)
        blend_state = [string]$pass.blend_state
        depth_stencil_state = [string]$pass.depth_stencil_state
        rasterizer_state = [string]$pass.rasterizer_state
        stencil_ref = [int]$pass.stencil_ref
        blend_sample_mask = [long]$pass.blend_sample_mask
        pixel_shader_resources = @($pass.pixel_shader_resources)
    }
}

$bufferSummaries = foreach ($bufferId in ($buffers.Keys | Sort-Object {[long]$_})) {
    $buffer = $buffers[$bufferId]
    [ordered]@{
        buffer_id = $buffer.buffer_id
        ptr = $buffer.ptr
        byte_width = $buffer.byte_width
        usage = $buffer.usage
        bind_flags = $buffer.bind_flags
        bind_flag_names = @($buffer.bind_flag_names)
        cpu_access_flags = $buffer.cpu_access_flags
        misc_flags = $buffer.misc_flags
        structure_byte_stride = $buffer.structure_byte_stride
        initial_data = $buffer.initial_data
        bind_count = if ($constantBufferUse.ContainsKey($bufferId)) { $constantBufferUse[$bufferId] } else { 0 }
        first_seq = $buffer.first_seq
        first_line = $buffer.first_line
    }
}

$stateObjectSummaries = foreach ($stateId in ($stateObjects.Keys | Sort-Object {[long]$_})) {
    $state = $stateObjects[$stateId]
    [ordered]@{
        state_id = $state.state_id
        ptr = $state.ptr
        kind = $state.kind
        bind_count = if ($stateObjectBindUse.ContainsKey($stateId)) { $stateObjectBindUse[$stateId] } elseif ($samplerUse.ContainsKey($stateId)) { $samplerUse[$stateId] } else { 0 }
        first_seq = $state.first_seq
        first_line = $state.first_line
        source_event_json = $state.desc_json
    }
}

$passResourceFlow = @()
foreach ($pass in $representativeOrder) {
    $resolvedSrvs = @()
    foreach ($srv in @($pass.ps_srvs)) {
        $view = $views[[string]$srv.view]
        $texture = if ($view) { $textures[[string]$view.texture_id] } else { $null }
        $resolvedSrvs += [ordered]@{
            slot = [int]$srv.slot
            view = [string]$srv.view
            texture_id = if ($view) { $view.texture_id } else { $null }
            format = if ($view) { $view.format } else { $null }
            width = if ($texture) { $texture.width } else { $null }
            height = if ($texture) { $texture.height } else { $null }
        }
    }
    $resolvedRtvs = @()
    foreach ($rtv in @($pass.rtvs)) {
        $view = $views[[string]$rtv]
        $texture = if ($view) { $textures[[string]$view.texture_id] } else { $null }
        $resolvedRtvs += [ordered]@{
            view = [string]$rtv
            texture_id = if ($view) { $view.texture_id } else { $null }
            format = if ($view) { $view.format } else { $null }
            width = if ($texture) { $texture.width } else { $null }
            height = if ($texture) { $texture.height } else { $null }
        }
    }
    $passResourceFlow += [ordered]@{
        order = $pass.order
        frame = $pass.frame
        pass_kind = $pass.pass_kind
        writes = @($resolvedRtvs)
        depth = $pass.dsv
        reads = @($resolvedSrvs)
        vs_hash = $pass.vs_hash
        ps_hash = $pass.ps_hash
        ps_resource_names = @($pass.pixel_shader_resources)
        vs_cbuffers = @($pass.vs_cbuffers)
        ps_cbuffers = @($pass.ps_cbuffers)
        ps_samplers = @($pass.ps_samplers)
        blend_state = $pass.blend_state
        depth_stencil_state = $pass.depth_stencil_state
        rasterizer_state = $pass.rasterizer_state
    }
}

$runtime = [ordered]@{
    source_log = $LogPath
    generated_at = (Get-Date).ToString("s")
    event_count = $eventCount
    present_count = $presentCount
    texture_count = $textures.Count
    buffer_count = $buffers.Count
    view_count = $views.Count
    render_targets = @($targets)
    shader_resources = @($srvTargets)
}

$passes = [ordered]@{
    source_log = $LogPath
    generated_at = (Get-Date).ToString("s")
    event_count = $eventCount
    present_count = $presentCount
    draw_pass_count = $drawPasses.Count
    passes = @($drawPasses.ToArray())
    state_pass_count = $script:statePassCount
    representative_frame = $script:bestFrame
    representative_frame_score = $script:bestFrameScore
    representative_pass_order = @($representativeOrder)
    state_pass_kind_counts = (Ordered -Map $stateKindCounts)
    top_state_pass_signatures = (Top-Ordered -Map $stateSignatureCounts -Limit 200)
    draw_counts_by_pixel_shader = (Ordered -Map $drawCountsByShader)
    draw_counts_by_target_view = (Ordered -Map $drawCountsByTarget)
}

$stateObjectsOutput = [ordered]@{
    source_log = $LogPath
    generated_at = (Get-Date).ToString("s")
    state_object_count = $stateObjects.Count
    sampler_bind_counts = (Ordered -Map $samplerUse)
    state_bind_counts = (Ordered -Map $stateObjectBindUse)
    state_objects = @($stateObjectSummaries)
}

$constantBuffersOutput = [ordered]@{
    source_log = $LogPath
    generated_at = (Get-Date).ToString("s")
    buffer_count = $buffers.Count
    constant_buffer_count = @($bufferSummaries | Where-Object { @($_.bind_flag_names) -contains "CONSTANT_BUFFER" }).Count
    bind_counts = (Ordered -Map $constantBufferUse)
    stage_slot_bind_counts = (Ordered -Map $constantBufferStageSlotUse)
    buffers = @($bufferSummaries)
}

$representativeFrameOutput = [ordered]@{
    source_log = $LogPath
    generated_at = (Get-Date).ToString("s")
    representative_frame = $script:bestFrame
    representative_frame_score = $script:bestFrameScore
    present_count = $presentCount
    pass_count = @($representativeOrder).Count
    passes = @($representativeOrder)
}

$passResourceFlowOutput = [ordered]@{
    source_log = $LogPath
    generated_at = (Get-Date).ToString("s")
    representative_frame = $script:bestFrame
    pass_count = @($passResourceFlow).Count
    flow = @($passResourceFlow)
}

New-Item -ItemType Directory -Force -Path (Split-Path -Parent $RuntimeTargetsPath) | Out-Null
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $RenderPassesPath) | Out-Null
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $StateObjectsPath) | Out-Null
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $ConstantBuffersPath) | Out-Null
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $RepresentativeFramePath) | Out-Null
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $PassResourceFlowPath) | Out-Null
$runtime | ConvertTo-Json -Depth 12 | Set-Content -Encoding UTF8 -LiteralPath $RuntimeTargetsPath
$passes | ConvertTo-Json -Depth 14 | Set-Content -Encoding UTF8 -LiteralPath $RenderPassesPath
$stateObjectsOutput | ConvertTo-Json -Depth 14 | Set-Content -Encoding UTF8 -LiteralPath $StateObjectsPath
$constantBuffersOutput | ConvertTo-Json -Depth 14 | Set-Content -Encoding UTF8 -LiteralPath $ConstantBuffersPath
$representativeFrameOutput | ConvertTo-Json -Depth 14 | Set-Content -Encoding UTF8 -LiteralPath $RepresentativeFramePath
$passResourceFlowOutput | ConvertTo-Json -Depth 14 | Set-Content -Encoding UTF8 -LiteralPath $PassResourceFlowPath

Write-Host "Wrote $RuntimeTargetsPath"
Write-Host "Wrote $RenderPassesPath"
Write-Host "Wrote $StateObjectsPath"
Write-Host "Wrote $ConstantBuffersPath"
Write-Host "Wrote $RepresentativeFramePath"
Write-Host "Wrote $PassResourceFlowPath"
Write-Host "events=$eventCount textures=$($textures.Count) buffers=$($buffers.Count) views=$($views.Count) state_objects=$($stateObjects.Count) draw_passes=$($drawPasses.Count) state_passes=$($script:statePassCount) representative_frame=$($script:bestFrame) presents=$presentCount"
