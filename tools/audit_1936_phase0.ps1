param(
    [string]$MarkdownPath = ""
)

[Console]::InputEncoding = [System.Text.UTF8Encoding]::new()
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
$OutputEncoding = [System.Text.UTF8Encoding]::new()
chcp 65001 | Out-Null

$ErrorActionPreference = "Stop"
$repo = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $repo

function Read-Utf8Raw($Path) {
    Get-Content -LiteralPath $Path -Encoding UTF8 -Raw
}

function Count-Regex($Text, $Pattern) {
    ([regex]::Matches($Text, $Pattern)).Count
}

function Regex-Values($Text, $Pattern, $Group = 1) {
    @([regex]::Matches($Text, $Pattern) |
        ForEach-Object { $_.Groups[$Group].Value } |
        Where-Object { $_ -ne $null -and $_.Trim() -ne "" })
}

function Sorted-Unique($Items) {
    @($Items | Where-Object { $_ -ne $null -and $_.Trim() -ne "" } | Sort-Object -Unique)
}

function Tags-From-Files($Path, $Pattern) {
    Sorted-Unique (Get-ChildItem -LiteralPath $Path -Filter $Pattern | ForEach-Object {
        ($_.BaseName -replace "^initial_", "").ToUpperInvariant()
    })
}

function Missing-From($Expected, $Actual) {
    $actualSet = @{}
    foreach ($item in @($Actual)) {
        $actualSet[$item] = $true
    }
    @($Expected | Where-Object { -not $actualSet.ContainsKey($_) })
}

function Join-List($Items) {
    $list = @($Items | Where-Object { $_ -ne $null -and "$_".Trim() -ne "" })
    if ($list.Count -eq 0) {
        return "(none)"
    }
    return ($list -join ", ")
}

function Group-Counts($Items) {
    @($Items |
        Group-Object |
        Sort-Object Name |
        ForEach-Object { "$($_.Name)=$($_.Count)" })
}

function RelPath($Path) {
    $target = if ([System.IO.Path]::IsPathRooted($Path)) { $Path } else { Join-Path $repo $Path }
    $full = [System.IO.Path]::GetFullPath($target)
    $root = [System.IO.Path]::GetFullPath($repo)
    if ($full.StartsWith($root)) {
        return $full.Substring($root.Length).TrimStart("\", "/")
    }
    return $Path
}

function Scan-File($Path, $Patterns) {
    $text = Read-Utf8Raw $Path
    $rows = @()
    foreach ($pattern in $Patterns) {
        $count = Count-Regex $text $pattern.Regex
        if ($count -gt 0) {
            $firstLine = 0
            $lines = $text -split "`r?`n"
            for ($i = 0; $i -lt $lines.Count; $i++) {
                if ($lines[$i] -match $pattern.Regex) {
                    $firstLine = $i + 1
                    break
                }
            }
            $rows += [pscustomobject]@{
                File = $Path
                Category = $pattern.Category
                Pattern = $pattern.Name
                Count = $count
                FirstLine = $firstLine
            }
        }
    }
    $rows
}

$auditDate = "2026-06-04"
$mainLines = (Get-Content -LiteralPath "crates/hoi4-app/src/main.rs" -Encoding UTF8 | Measure-Object -Line).Lines

$countryProfiles = Tags-From-Files "crates/hoi4-content/content/history_1936/countries" "*.ron"
$initialPops = Tags-From-Files "crates/hoi4-content/content/economy_v6/pops" "initial_*.ron"

$countryProfileTexts = Get-ChildItem -LiteralPath "crates/hoi4-content/content/history_1936/countries" -Filter "*.ron" |
    ForEach-Object { [pscustomobject]@{ Tag = $_.BaseName.ToUpperInvariant(); Text = Read-Utf8Raw $_.FullName } }
$countryProfilesWithOldPopulation = Sorted-Unique ($countryProfileTexts | Where-Object { $_.Text -match "population:\s*\d+" } | ForEach-Object Tag)
$countryProfilesWithOldGdp = Sorted-Unique ($countryProfileTexts | Where-Object { $_.Text -match "gdp_1936_gbp:\s*" } | ForEach-Object Tag)
$countryProfilesWithReferencePopulation = Sorted-Unique ($countryProfileTexts | Where-Object { $_.Text -match "reference_population:\s*" } | ForEach-Object Tag)
$countryProfilesWithReferenceGdp = Sorted-Unique ($countryProfileTexts | Where-Object { $_.Text -match "reference_gdp:\s*" } | ForEach-Object Tag)

$statePopText = Read-Utf8Raw "crates/hoi4-content/content/history_1936/states/state_population.ron"
$statePopIds = Sorted-Unique (Regex-Values $statePopText "state_id:\s*(\d+)")
$stateQualities = Group-Counts (Regex-Values $statePopText "data_quality:\s*([A-Za-z]+)")
$stateIntegrations = Group-Counts (Regex-Values $statePopText "integration:\s*([A-Za-z]+)")

$resourceText = Read-Utf8Raw "crates/hoi4-content/content/history_1936/resources/state_deposits.ron"
$resourceStateIds = Sorted-Unique (Regex-Values $resourceText "state_id:\s*(\d+)")

$tradeText = Read-Utf8Raw "crates/hoi4-content/content/history_1936/trade/initial_trade_1936.ron"
$tradeImportTags = Regex-Values $tradeText 'importer:\s*"([A-Z]{3})"'
$tradeExportTags = Regex-Values $tradeText 'exporter:\s*"([A-Z]{3})"'
$tradeTags = Sorted-Unique ($tradeImportTags + $tradeExportTags)

$militaryText = Read-Utf8Raw "crates/hoi4-content/content/history_1936/military/force_profiles_1936.ron"
$militaryTags = Sorted-Unique (Regex-Values $militaryText 'tag:\s*"([A-Z]{3})"')

$headText = Read-Utf8Raw "crates/hoi4-content/content/history_1936/politics/head_of_state_1936.ron"
$headTags = Sorted-Unique (Regex-Values $headText 'tag:\s*"([A-Z]{3})"')

$stateOwnerText = Read-Utf8Raw "crates/hoi4-data/content/history_1936/state_owners.ron"
$stateOwnerIds = Sorted-Unique (Regex-Values $stateOwnerText "state_id:\s*(\d+)")
$stateOwnerTags = Sorted-Unique ((Regex-Values $stateOwnerText 'owner:\s*"([A-Z]{3})"') + (Regex-Values $stateOwnerText '"([A-Z]{3})"'))

$customCountryText = Read-Utf8Raw "crates/hoi4-data/content/history_1936/custom_countries.ron"
$customCountryTags = Sorted-Unique (Regex-Values $customCountryText 'tag:\s*"([A-Z]{3})"')

$financeText = Read-Utf8Raw "crates/hoi4-content/content/economy_v6/finance/historical_finance_profiles.ron"
$financeTags1936 = Sorted-Unique (Regex-Values $financeText 'country:\s*"([A-Z]{3})",\s*year:\s*1936')

$historyFiles = @(
    Get-ChildItem -Path "crates/hoi4-content/content/history_1936" -Recurse -File
    Get-ChildItem -Path "crates/hoi4-data/content/history_1936" -Recurse -File
)
$referencedTags = @()
foreach ($file in $historyFiles) {
    $text = Read-Utf8Raw $file.FullName
    $referencedTags += Regex-Values $text '"([A-Z]{3})"'
}
$referencedTags = Sorted-Unique $referencedTags
$observed1936Tags = Sorted-Unique ($countryProfiles + $stateOwnerTags + $customCountryTags + $headTags + $militaryTags + $tradeTags)

$missingCountryProfileForObserved = Missing-From $observed1936Tags $countryProfiles
$missingPopForProfiles = Missing-From $countryProfiles $initialPops
$missingPopForObserved = Missing-From $observed1936Tags $initialPops
$missingMilitaryForProfiles = Missing-From $countryProfiles $militaryTags
$missingHeadForProfiles = Missing-From $countryProfiles $headTags
$missingFinance1936ForProfiles = Missing-From $countryProfiles $financeTags1936
$stateOwnerStatesMissingPopulation = Missing-From $stateOwnerIds $statePopIds
$g1RepositoryWorldCoveragePass = (
    $observed1936Tags.Count -gt 0 -and
    $missingCountryProfileForObserved.Count -eq 0 -and
    $missingPopForObserved.Count -eq 0 -and
    $missingMilitaryForProfiles.Count -eq 0 -and
    $missingHeadForProfiles.Count -eq 0 -and
    $missingFinance1936ForProfiles.Count -eq 0 -and
    $stateOwnerStatesMissingPopulation.Count -eq 0
)
$g1RepositoryWorldCoverage = if ($g1RepositoryWorldCoveragePass) { "PASS" } else { "PARTIAL" }

$buildingText = Read-Utf8Raw "crates/hoi4-content/content/economy_v6/buildings/buildings.ron"
$buildingEntries = Count-Regex $buildingText '\(\s*id:\s*"[^"]+"'
$recipeCount = Count-Regex $buildingText "construction_recipe:\s*\("
$employmentCount = Count-Regex $buildingText "employment_profile:\s*\("
$sectorCounts = Group-Counts (Regex-Values $buildingText "economic_sector:\s*([A-Za-z]+)")
$classCounts = Group-Counts (Regex-Values $buildingText "gameplay_class:\s*([A-Za-z]+)")
$missingRoadmapSectors = Missing-From @("Government", "Infrastructure", "MilitarySupport") (Regex-Values $buildingText "economic_sector:\s*([A-Za-z]+)")
$missingRoadmapClasses = Missing-From @("Government", "HeavyIndustry", "LightIndustry", "ResourceExtraction", "MilitaryIndustry") (Regex-Values $buildingText "gameplay_class:\s*([A-Za-z]+)")

$v7LoaderText = Read-Utf8Raw "crates/hoi4-content/src/v7_history_loader.rs"
$manualCountryIncludes = Regex-Values $v7LoaderText 'content/history_1936/countries/([A-Z]{3})\.ron'

$v6LoaderText = Read-Utf8Raw "crates/hoi4-content/src/v6_loader.rs"
$fallbackFunctions = @(
    "inject_algorithmic_pops_for_remaining_countries",
    "inject_baseline_buildings_for_remaining_countries",
    "apply_fallback_finance_for_remaining_countries"
)
$fallbackPresence = @($fallbackFunctions | ForEach-Object {
    [pscustomobject]@{ Name = $_; Present = ($v6LoaderText.Contains($_)) }
})

$scanPatterns = @(
    [pscustomobject]@{ Category = "乱码"; Name = "mojibake"; Regex = "鐢|绋|鏂|鍥|鈺|鏋|€|�" },
    [pscustomobject]@{ Category = "占位"; Name = "question_marks"; Regex = "\?\?\?" },
    [pscustomobject]@{ Category = "内部名"; Name = "state_internal"; Regex = "STATE_|state_|State [0-9]|省份 [0-9]" },
    [pscustomobject]@{ Category = "英文占位"; Name = "english_placeholder"; Regex = '"(Source|Details|Label|Funding|Blocked|Paused|Active|Idle|Import|Export|Value)"' },
    [pscustomobject]@{ Category = "英文UI"; Name = "construction_english"; Regex = '"(Government|Private pool|Cartel pool|Overlord investment|Foreign investment|State|Private|Cartel|Not buildable|Requires law:|Requires technology:|State limit:)' }
)
$scanFiles = @(
    "crates/hoi4-app/src/ui_data/market.rs",
    "crates/hoi4-app/src/ui_data/pops.rs",
    "crates/hoi4-app/src/ui_data/construction.rs",
    "crates/hoi4-app/src/ui_data/economy.rs",
    "crates/hoi4-app/src/ui_data/country.rs",
    "crates/hoi4-ui/src/finance_panel.rs",
    "crates/hoi4-ui/src/construction_v6_panel.rs",
    "crates/hoi4-ui/src/market_panel.rs",
    "crates/hoi4-ui/src/pop_panel.rs",
    "crates/hoi4-content/src/v6_loader.rs"
) | Where-Object { Test-Path -LiteralPath $_ }
$scanRows = @()
foreach ($file in $scanFiles) {
    $scanRows += Scan-File $file $scanPatterns
}

$dtoIdFields = @()
$dtoFieldPatterns = @(
    [pscustomobject]@{ Field = "state_id"; Regex = "pub\s+state_id\s*:" },
    [pscustomobject]@{ Field = "building_def_id"; Regex = "pub\s+building_def_id\s*:" },
    [pscustomobject]@{ Field = "good_id"; Regex = "pub\s+good_id\s*:" },
    [pscustomobject]@{ Field = "related_good_id"; Regex = "pub\s+related_good_id\s*:" },
    [pscustomobject]@{ Field = "building_idx"; Regex = "pub\s+building_idx\s*:" }
)
foreach ($file in @(
    "crates/hoi4-ui/src/finance_panel.rs",
    "crates/hoi4-ui/src/construction_v6_panel.rs",
    "crates/hoi4-ui/src/market_panel.rs",
    "crates/hoi4-ui/src/pop_panel.rs"
)) {
    if (-not (Test-Path -LiteralPath $file)) { continue }
    $text = Read-Utf8Raw $file
    foreach ($pattern in $dtoFieldPatterns) {
        $count = Count-Regex $text $pattern.Regex
        if ($count -gt 0) {
            $dtoIdFields += [pscustomobject]@{
                File = $file
                Field = $pattern.Field
                Count = $count
            }
        }
    }
}

Write-Output "phase0_1936_audit=$auditDate"
Write-Output "main_rs_lines=$mainLines"
Write-Output "country_profile_count=$($countryProfiles.Count)"
Write-Output "observed_1936_tag_count=$($observed1936Tags.Count)"
Write-Output "observed_1936_tags=$(Join-List $observed1936Tags)"
Write-Output "g1_repository_world_coverage=$g1RepositoryWorldCoverage"
Write-Output "missing_country_profile_for_observed=$(Join-List $missingCountryProfileForObserved)"
Write-Output "initial_pop_count=$($initialPops.Count)"
Write-Output "missing_pop_for_country_profiles=$(Join-List $missingPopForProfiles)"
Write-Output "missing_pop_for_observed=$(Join-List $missingPopForObserved)"
Write-Output "state_population_count=$($statePopIds.Count)"
Write-Output "state_owner_override_count=$($stateOwnerIds.Count)"
Write-Output "state_owner_overrides_missing_population=$(Join-List $stateOwnerStatesMissingPopulation)"
Write-Output "state_population_quality=$(Join-List $stateQualities)"
Write-Output "state_population_integration=$(Join-List $stateIntegrations)"
Write-Output "state_deposit_count=$($resourceStateIds.Count)"
Write-Output "trade_route_count=$($tradeImportTags.Count)"
Write-Output "military_profile_count=$($militaryTags.Count)"
Write-Output "head_of_state_count=$($headTags.Count)"
Write-Output "missing_head_for_country_profiles=$(Join-List $missingHeadForProfiles)"
Write-Output "finance_1936_tag_count=$($financeTags1936.Count)"
Write-Output "missing_finance_1936_for_country_profiles=$(Join-List $missingFinance1936ForProfiles)"
Write-Output "manual_country_include_count=$($manualCountryIncludes.Count)"
Write-Output "building_entries=$buildingEntries"
Write-Output "building_construction_recipe_count=$recipeCount"
Write-Output "building_employment_profile_count=$employmentCount"
Write-Output "building_economic_sector_counts=$(Join-List $sectorCounts)"
Write-Output "building_gameplay_class_counts=$(Join-List $classCounts)"
Write-Output "missing_roadmap_economic_sectors=$(Join-List $missingRoadmapSectors)"
Write-Output "missing_roadmap_gameplay_classes=$(Join-List $missingRoadmapClasses)"
Write-Output "scan_issue_count=$($scanRows.Count)"
Write-Output "dto_internal_id_field_risk_count=$($dtoIdFields.Count)"

if ($MarkdownPath.Trim() -ne "") {
    $md = New-Object System.Collections.Generic.List[string]
    $md.Add("# Phase 0：1936 经济生产 UI 重构审计报告")
    $md.Add("")
    $md.Add("生成日期：$auditDate")
    $md.Add("")
    $md.Add("适用路线图：``ROADMAP_1936_ECONOMY_PRODUCTION_UI_REWORK_ZH.md``")
    $md.Add("")
    $md.Add("本报告由 ``tools/audit_1936_phase0.ps1 -MarkdownPath PHASE0_1936_ECONOMY_AUDIT_ZH.md`` 生成。范围只包括只读审计、编码基线和缺档报告；本阶段不修改运行逻辑、不改 tick、不改 loader 注入路径、不改 UI 行为。")
    $md.Add("")
    $md.Add("## 1. Phase 0 结论")
    $md.Add("")
    $md.Add("Phase 0 的必要产物已经补齐为可复跑脚本和固定 Markdown 报告。当前仓库定义的 1936 tag 覆盖可复跑审计结果为 ``$g1RepositoryWorldCoverage``；仓库仍没有外部完整 vanilla ``history/countries`` 和全地图 state owner 源，因此不能把该结果解释为外部 vanilla 全集验收。")
    $md.Add("")
    $md.Add("| 验收项 | 状态 | 当前事实 |")
    $md.Add("|---|---|---|")
    $md.Add("| 文档存在 | 通过 | 路线图、goal 文档、本报告存在 |")
    $md.Add("| 未修改运行逻辑 | 通过 | Phase 0 只读审计脚本和报告，不触碰 runtime 代码 |")
    $md.Add("| main.rs 经济职责审计 | 通过 | ``main.rs`` 当前 $mainLines 行，仍保留面板缓存、建设命令、显示名辅助、GDP fallback、后勤市场派生等职责 |")
    $md.Add("| G1 仓库定义 1936 世界覆盖 | $g1RepositoryWorldCoverage | 经济档案、初始 POP、1936 财政、军事、元首、owner override 州人口缺口均按第 4 节判定 |")
    $md.Add("| 缺档国家清单 | 通过 | 仓库可见 1936 tag 中缺经济档案：``$(Join-List $missingCountryProfileForObserved)`` |")
    $md.Add("| 缺 POP / 州人口 / 财政 / 建筑覆盖报告 | 通过 | 见第 3 节和第 4 节 |")
    $md.Add("| 乱码、英文占位、内部 id 泄露报告 | 通过 | 见第 6 节和第 7 节 |")
    $md.Add("| 编码规则固定 | 通过 | 见第 2 节 |")
    $md.Add("| 外部 vanilla 全 1936 实际国家覆盖 | 未通过 | 仓库没有完整 vanilla 数据源；runtime fallback 可保留但不计入 G1 覆盖 |")
    $md.Add("")
    $md.Add("## 2. 编码基线")
    $md.Add("")
    $md.Add("Windows 11 + PowerShell 中文环境下，所有中文读取和复查命令先设置 UTF-8：")
    $md.Add("")
    $md.Add('```powershell')
    $md.Add("chcp 65001")
    $md.Add("[Console]::InputEncoding = [System.Text.UTF8Encoding]::new()")
    $md.Add("[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()")
    $md.Add('$OutputEncoding = [System.Text.UTF8Encoding]::new()')
    $md.Add('```')
    $md.Add("")
    $md.Add("禁止用 ``Get-Content xxx | Set-Content xxx`` 批量重写中文文件。复查报告使用：")
    $md.Add("")
    $md.Add('```powershell')
    $md.Add("powershell -ExecutionPolicy Bypass -File tools/audit_1936_phase0.ps1 -MarkdownPath PHASE0_1936_ECONOMY_AUDIT_ZH.md")
    $md.Add('```')
    $md.Add("")
    $md.Add("## 3. 覆盖率摘要")
    $md.Add("")
    $md.Add("| 项目 | 数量 | 说明 |")
    $md.Add("|---|---:|---|")
    $md.Add("| 仓库可见 1936 tag | $($observed1936Tags.Count) | 国家档案、state owner override、自定义国家、元首、军事、贸易 tag 的并集 |")
    $md.Add("| 手写国家经济档案 | $($countryProfiles.Count) | ``crates/hoi4-content/content/history_1936/countries/*.ron`` |")
    $md.Add("| 初始 POP 档案 | $($initialPops.Count) | ``crates/hoi4-content/content/economy_v6/pops/initial_*.ron`` |")
    $md.Add("| 州人口档案 | $($statePopIds.Count) | ``history_1936/states/state_population.ron`` |")
    $md.Add("| 1936 state owner override | $($stateOwnerIds.Count) | ``crates/hoi4-data/content/history_1936/state_owners.ron`` |")
    $md.Add("| 州资源档案 | $($resourceStateIds.Count) | ``history_1936/resources/state_deposits.ron`` |")
    $md.Add("| 历史贸易路线 | $($tradeImportTags.Count) | ``history_1936/trade/initial_trade_1936.ron`` |")
    $md.Add("| 军事档案 | $($militaryTags.Count) | ``history_1936/military/force_profiles_1936.ron`` |")
    $md.Add("| 国家元首档案 | $($headTags.Count) | ``history_1936/politics/head_of_state_1936.ron`` |")
    $md.Add("| 1936 财政 profile tag | $($financeTags1936.Count) | ``economy_v6/finance/historical_finance_profiles.ron`` 中 year=1936 的 country |")
    $md.Add("| 建筑定义 | $buildingEntries | ``economy_v6/buildings/buildings.ron`` |")
    $md.Add("")
    $md.Add("州人口质量：``$(Join-List $stateQualities)``")
    $md.Add("")
    $md.Add("州人口整合类型：``$(Join-List $stateIntegrations)``")
    $md.Add("")
    $md.Add("## 4. 缺档清单")
    $md.Add("")
    $md.Add("仓库可见 1936 tag：")
    $md.Add("")
    $md.Add('```text')
    $md.Add((Join-List $observed1936Tags))
    $md.Add('```')
    $md.Add("")
    $md.Add("缺国家经济档案：")
    $md.Add("")
    $md.Add('```text')
    $md.Add((Join-List $missingCountryProfileForObserved))
    $md.Add('```')
    $md.Add("")
    $md.Add("有国家经济档案但缺初始 POP 档案：")
    $md.Add("")
    $md.Add('```text')
    $md.Add((Join-List $missingPopForProfiles))
    $md.Add('```')
    $md.Add("")
    $md.Add("仓库可见 1936 tag 中缺初始 POP 档案：")
    $md.Add("")
    $md.Add('```text')
    $md.Add((Join-List $missingPopForObserved))
    $md.Add('```')
    $md.Add("")
    $md.Add("有国家经济档案但缺元首档案：``$(Join-List $missingHeadForProfiles)``")
    $md.Add("")
    $md.Add("有国家经济档案但缺 1936 财政 profile：``$(Join-List $missingFinance1936ForProfiles)``")
    $md.Add("")
    $md.Add("state owner override 中缺州人口档案的 state：``$(Join-List $stateOwnerStatesMissingPopulation)``")
    $md.Add("")
    $md.Add("G1 仓库定义 1936 世界覆盖：``$g1RepositoryWorldCoverage``。该判定要求仓库可见 1936 tag 均有经济档案、初始 POP、1936 财政 profile、军事 profile、元首档案，且所有 state owner override 均有州人口档案。")
    $md.Add("")
    $md.Add("注意：这不是外部完整 vanilla 1936 国家全集。当前仓库没有完整 vanilla ``history/countries`` 和全地图 state owner 源；如后续接入外部源，必须重新按外部全集验收。")
    $md.Add("")
    $md.Add("## 5. Schema 和 fallback 审计")
    $md.Add("")
    $md.Add("国家经济档案仍是旧 schema：$($countryProfilesWithOldPopulation.Count) 个档案含 ``population``，$($countryProfilesWithOldGdp.Count) 个档案含 ``gdp_1936_gbp``；$($countryProfilesWithReferencePopulation.Count) 个档案含 ``reference_population``，$($countryProfilesWithReferenceGdp.Count) 个档案含 ``reference_gdp``。这表示 GDP/人口字段仍需要在 Phase 3/7 改成只做校验参考。")
    $md.Add("")
    $md.Add("``Historical1936Database::load()`` 仍手工 include 国家档案，include 数量：$($manualCountryIncludes.Count)。")
    $md.Add("")
    $md.Add("runtime fallback 路径仍存在：")
    $md.Add("")
    $md.Add("| 函数 | 存在 |")
    $md.Add("|---|---|")
    foreach ($fallback in $fallbackPresence) {
        $md.Add("| ``$($fallback.Name)`` | $($fallback.Present) |")
    }
    $md.Add("")
    $md.Add("建筑 schema 覆盖：")
    $md.Add("")
    $md.Add('```text')
    $md.Add("building_entries=$buildingEntries")
    $md.Add("construction_recipe_count=$recipeCount")
    $md.Add("employment_profile_count=$employmentCount")
    $md.Add("economic_sector_counts=$(Join-List $sectorCounts)")
    $md.Add("gameplay_class_counts=$(Join-List $classCounts)")
    $md.Add("missing_roadmap_economic_sectors=$(Join-List $missingRoadmapSectors)")
    $md.Add("missing_roadmap_gameplay_classes=$(Join-List $missingRoadmapClasses)")
    $md.Add('```')
    $md.Add("")
    $md.Add("## 6. 乱码、占位和英文 UI 扫描")
    $md.Add("")
    $md.Add("| 文件 | 类别 | 模式 | 次数 | 首行 |")
    $md.Add("|---|---|---|---:|---:|")
    foreach ($row in ($scanRows | Sort-Object File, Category, Pattern)) {
        $md.Add("| ``$($row.File)`` | $($row.Category) | ``$($row.Pattern)`` | $($row.Count) | $($row.FirstLine) |")
    }
    if ($scanRows.Count -eq 0) {
        $md.Add("| 无 | 无 | 无 | 0 | 0 |")
    }
    $md.Add("")
    $md.Add("扫描结果只固定缺口，不在 Phase 0 清理文本。玩家可见 DTO 的清理应进入 Phase 2/8；``v6_loader.rs`` 注释乱码可单独处理。")
    $md.Add("")
    $md.Add("## 7. 玩家 DTO 内部 id 字段风险")
    $md.Add("")
    $md.Add("这些字段不一定都会直接显示给玩家，但它们是 Phase 2/8 需要确认的 DTO 风险面：")
    $md.Add("")
    $md.Add("| 文件 | 字段 | 次数 |")
    $md.Add("|---|---|---:|")
    foreach ($row in ($dtoIdFields | Sort-Object File, Field)) {
        $md.Add("| ``$($row.File)`` | ``$($row.Field)`` | $($row.Count) |")
    }
    if ($dtoIdFields.Count -eq 0) {
        $md.Add("| 无 | 无 | 0 |")
    }
    $md.Add("")
    $md.Add("## 8. main.rs 经济职责审计")
    $md.Add("")
    $md.Add("当前 ``crates/hoi4-app/src/main.rs`` 实测为 $mainLines 行。Phase 1 已有部分拆分，但该文件仍不是薄 app 编排层。仍留在 ``main.rs`` 的相关职责包括：")
    $md.Add("")
    $md.Add("- UI 面板缓存和 tab/overlay 调度。")
    $md.Add("- 建设模式、建设高亮、省份点击建造、队列命令处理。")
    $md.Add("- 国家/州/省份显示名辅助调用和部分 fallback。")
    $md.Add("- GDP fallback、后勤市场派生、地图省份信息卡组装。")
    $md.Add("- 部分外交/国家信息 DTO 已迁出到 ``ui_data/country.rs``，但这属于 Phase 1 方向，不改变 Phase 0 只读结论。")
    $md.Add("")
    $md.Add("## 9. 复查命令")
    $md.Add("")
    $md.Add('```powershell')
    $md.Add("powershell -ExecutionPolicy Bypass -File tools/audit_1936_phase0.ps1")
    $md.Add("powershell -ExecutionPolicy Bypass -File tools/audit_1936_phase0.ps1 -MarkdownPath PHASE0_1936_ECONOMY_AUDIT_ZH.md")
    $md.Add('```')
    $md.Add("")
    $outPath = if ([System.IO.Path]::IsPathRooted($MarkdownPath)) { $MarkdownPath } else { Join-Path $repo $MarkdownPath }
    [System.IO.File]::WriteAllLines($outPath, $md, [System.Text.UTF8Encoding]::new($false))
    Write-Output "markdown_report_written=$(RelPath $outPath)"
}
