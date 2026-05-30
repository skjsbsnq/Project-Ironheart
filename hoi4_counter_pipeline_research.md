# HOI4 Unit Counter (兵牌/NATO Counter) Rendering Pipeline
## Technical Reverse-Engineering Documentation

---

## 1. Architecture Overview

The HOI4 unit counter system is a GPU-driven billboard rendering pipeline built on the Clausewitz Engine's map icon subsystem. Counters are rendered as **camera-facing billboards** in 3D map space, composited from multiple texture layers in a fragment shader, with CPU-side grouping/stacking logic controlling which data gets sent per-instance.

The pipeline flow:

```
Game Sim (CPU)                    Map Icon System (CPU)              GPU
┌─────────────┐    ┌──────────────────────────┐    ┌─────────────────────────┐
│ Division     │───>│ CMapIconManager           │───>│ Vertex Shader           │
│ data: type,  │    │  - grouping passes        │    │  - billboard transform  │
│ org, country,│    │  - stack merging          │    │  - z-bias offset        │
│ combat state │    │  - LOD distance culling   │    │  - per-instance data    │
└─────────────┘    │  - priority sorting        │    ├─────────────────────────┤
                   └──────────────────────────┘    │ Fragment Shader          │
                                                   │  - 4-layer compositing   │
                                                   │  - SDF stack count chip  │
                                                   │  - combat blink pulse    │
                                                   │  - country color tinting │
                                                   └─────────────────────────┘
```

---

## 2. Defines: `NMapIcons` (from `00_graphics.lua`)

These are the key defines controlling the map icon pipeline. Found in `NDefines.NMapIcons` and `NDefines.NGraphics`:

### NMapIcons Section

| Define | Default | Description |
|--------|---------|-------------|
| `TOP_MAP_ICON` | 30 | Topmost z-priority for map icons |
| `INTERPOLATION_SNAP_DISTANCE` | 0.3 | Distance threshold for snapping counter movement interpolation |
| `INTEL_MAP_MODE_MAP_ICON_OFFSET` | { 12, 40 } | Pixel offset for intel map mode icons (counterintel, operatives) |
| `COARSE_RAILWAY_GUN_POSITION_OFFSET` | { -30, 0 } | World center offset for railway gun icons |
| `DEFAULT_PRIORITY_UNITS_STACK` | 10 | Render priority for individual unit stacks |
| `DEFAULT_PRIORITY_UNITS_STACK_GROUP` | 11 | Render priority for grouped unit stacks |
| `DEFAULT_PRIORITY_LAND_COMBAT` | 20 | Priority for land combat indicators |
| `DEFAULT_PRIORITY_NAVAL_COMBAT` | 20 | Priority for naval combat indicators |
| `DEFAULT_PRIORITY_SUPPLY` | 14 | Supply icon priority |
| `DEFAULT_PRIORITY_NAVAL_MISSION` | 13 | Naval mission icon priority |
| `DEFAULT_PRIORITY_AIR_MISSION` | 13 | Air mission icon priority |

Per-map-mode priority overrides exist for: STATES_*, SUPPLY_AREAS_*, STRATEGIC_AIR_*, STRATEGIC_NAVY_* prefixes (same fields with different values per map mode).

### NGraphics Section (Map Icon Grouping)

| Define | Default | Description |
|--------|---------|-------------|
| `MAP_ICONS_GROUP_CAM_DISTANCE` | 90.0 | Camera distance at which icons begin to group up |
| `MAP_ICONS_STATE_GROUP_CAM_DISTANCE` | 180.0 | Camera distance for state-level grouping |
| `MAP_ICONS_STRATEGIC_GROUP_CAM_DISTANCE` | 350 | Second camera distance threshold for grouping |
| `MAP_ICONS_STRATEGIC_AREA_HUGE` | 220 | Strategic area "huge" threshold |
| `MAP_ICONS_STATE_HUGE` | 100 | State "huge" threshold |
| **`MAPICON_GROUP_PASSES`** | **20** | **How many mapicons get processed per frame for grouping. More = quicker response, fewer = better performance** |
| `MAP_ICONS_GROUP_SPLIT_SELECTED_LIMIT` | 12 | Max number of selected units that will cause icon stacks to split |
| `MAP_ICONS_COARSE_COUNTRY_GROUPING_DISTANCE` | 350 | Distance at which grouping becomes very coarse (merges different unit types) |
| `MAP_ICONS_COARSE_COUNTRY_GROUPING_DISTANCE_STRATEGIC` | 350 | Same, for strategic mapmodes |
| `UNITS_DISTANCE_CUTOFF` | 120 | Camera distance below which individual units are shown |
| `SHIPS_DISTANCE_CUTOFF` | 240 | Camera distance for individual ships |
| `UNITS_ICONS_DISTANCE_CUTOFF` | 900 | Camera distance at which unit icons disappear |
| `UNIT_ARROW_DISTANCE_CUTOFF` | 875 | Distance for unit movement arrows |

### NGraphics Section (Distance Cutoffs)

| Define | Default | Description |
|--------|---------|-------------|
| `NAVAL_COMBAT_DISTANCE_CUTOFF` | 1500 | |
| `LAND_COMBAT_DISTANCE_CUTOFF` | 1500 | |
| `AIRBASE_ICON_DISTANCE_CUTOFF` | 900 | |
| `NAVALBASE_ICON_DISTANCE_CUTOFF` | 900 | |
| `RADAR_ICON_DISTANCE_CUTOFF` | 1100 | |
| `CAPITAL_ICON_CUTOFF` | 1100 | |
| `WEATHER_DISTANCE_CUTOFF` | 1500 | |
| `ADJACENCY_RULE_DISTANCE_CUTOFF` | 1700 | |

---

## 3. Shader Pipeline: `maparrow.shader` — `SymbolVertexShader` + `SymbolPixelShader`

**UPDATE (2026-05-17)**：原版兵牌不存在独立的 `unit_counter.fxh`。实际渲染使用的是 `gfx/FX/maparrow.shader` 中的 `MapSymbolDefault` Effect（`SymbolVertexShader` + `SymbolPixelShader`），与箭头/贸易路线共用同一个 shader 文件。源码 644 行纯文本，可直接从 vanilla 安装目录读取。

### 3.0 Actual Vanilla Source (from `gfx/FX/maparrow.shader`)

#### Vertex Struct

```hlsl
VertexStruct VS_INPUT_MAPSYMBOL
{
    float3 position   : POSITION;
    float3 uv         : TEXCOORD0;
};

VertexStruct VS_OUTPUT_MAPSYMBOL
{
    float4 position           : PDX_POSITION;
    float2 uv                 : TEXCOORD0;
    float4 vScreenCoord       : TEXCOORD1;
    float2 uv_terrain         : TEXCOORD2;
    float2 uv_terrain_id      : TEXCOORD3;
    float3 prepos             : TEXCOORD4;
};
```

#### ConstantBuffer(4) — Per-Instance Data

```hlsl
ConstantBuffer( 4, 32 ) # For symbol shader
{
    float4 SymbolColor;                           // Country color RGBA
    float4 Position_Scale;                        // xyz=world position, w=scale
    float4 vTime_IsSelected_IsIntersect_Rot;      // x=time, y=isSelected, z=isIntersect, w=rotation
};
```

#### SymbolVertexShader

```hlsl
VS_OUTPUT_MAPSYMBOL main( const VS_INPUT_MAPSYMBOL VertexIn )
{
    VS_OUTPUT_MAPSYMBOL VertexOut;
    float vSize = Position_Scale.w + ( vTime_IsSelected_IsIntersect_Rot.z * ( Position_Scale.w * 0.25f ) );
    float3 vTruePosition = VertexIn.position.xyz * vSize;
    vTruePosition.xz = RotateVector2D( vTruePosition.xz, vTime_IsSelected_IsIntersect_Rot.w );
    vTruePosition += Position_Scale.xyz;
    VertexOut.prepos = vTruePosition;
    VertexOut.position = mul( ViewProjectionMatrix, float4( vTruePosition, 1.0f ) );
    VertexOut.uv = VertexIn.uv.xy;
    // ... terrain UV calculation ...
    return VertexOut;
}
```

#### SymbolPixelShader

```hlsl
float4 main( VS_OUTPUT_MAPSYMBOL Input ) : PDX_COLOR
{
    // 1. Sample terrain normal (heightmap + terrain_atlas_normal + water)
    float2 vUV = Input.uv;
    float vWaterValue = 0;
    float3 vNormal = CalculateTerrainNormal( Input.uv_terrain, Input.uv_terrain_id, vWaterValue, vTime_IsSelected_IsIntersect_Rot.x );
    // 2. UV distortion by terrain normal (counter "sticks" to terrain)
    vUV += vNormal.xz * ( vNormal.y * MAP_ARROW_NORMALS_STR_TERR );
    vUV -= vNormal.xz * ( vWaterValue * MAP_ARROW_NORMALS_STR_WATER );
    // 3. Sample counter texture
    float4 vColor = tex2D( TexPattern, vUV );
    // 4. Country color tint
    vColor *= SymbolColor;
    // 5. Full Blinn-Phong lighting
    vColor.rgb = CalculateLighting( Input.prepos, Input.vScreenCoord, vNormal, vColor );
    // 6. Distance fog
    vColor.rgb = ApplyDistanceFog( vColor.rgb, Input.prepos );
    // 7. Day/night (commented in source but engine enables it)
    //vColor.rgb = DayNight( vColor.rgb, CalcGlobeNormal( Input.prepos.xz ) );
    return vColor;
}
```

#### Effect Definitions

```hlsl
Effect MapSymbolDefault
{
    VertexShader = "SymbolVertexShader"
    PixelShader = "SymbolPixelShader"
    DepthStencilState = "NoDepthStencilState"    // DepthEnable=no, StencilEnable=no
}

Effect MapSymbolDefaultAdd
{
    VertexShader = "SymbolVertexShader"
    PixelShader = "SymbolPixelShader"
    DepthStencilState = "NoDepthStencilState"
    BlendState = "BlendStateAdd"                 // Additive blending
}
```

### 3.1 Previous Inference (SUPERSEDED by 3.0 above)

The following was inferred before the actual source was located. It is kept for reference but **Section 3.0 is the authoritative source**.

#### Previous Per-instance Input Data (inferred, now confirmed inaccurate)

```hlsl
// Likely per-instance data layout (reverse-engineered)
// ACTUAL: ConstantBuffer(4) with {SymbolColor, Position_Scale, Time_Selected_Intersect_Rot}
struct VS_INPUT_MAPICON {
    float3 worldPos;          // Position on the map (province center / stack position)
    float2 size;              // Billboard width/height in pixels
    float  zBias;             // Depth offset (from priority defines)
    float  camDistFade;       // Pre-computed fade based on camera distance
    uint   iconType;          // Type index (infantry, armor, etc.) → selects texture atlas frame
    uint   countryColor;      // Packed RGBA for country tinting
    uint   combatState;       // 0=none, 1=attacking, 2=defending, 3=retreating → blink
    uint   stackCount;        // Number of divisions in this stack → chip SDF
    float  isSelected;        // Highlight/selection state
    float  isGrouped;         // Whether this is a merged group icon
    float  combatProgress;    // For combat indicator bar
    uint   organizationalState; // Org/HP level for color overlay
};
```

**Billboard algorithm:**
- The counter always faces the camera (spherical billboard, not cylindrical)
- Position is projected from world space to screen space
- Z-bias is applied based on `DEFAULT_PRIORITY_*` values so combat indicators render on top of unit counters, which render on top of VP dots, etc.
- Size scales with camera distance (up to distance cutoffs defined in NGraphics)

#### 3.2 Fragment Shader: 4-Layer Compositing

The counter fragment shader composites **4 texture layers** to produce the final counter appearance:

```
Layer 0 (bottom):  Background / counter shape
                    - The rectangular counter base with rounded corners
                    - Country color fill or neutral background
                    - Source: "GFX_onmap_unit_counter" or "GFX_onmap_counter_background"

Layer 1:           Unit type icon (NATO symbol)
                    - Infantry cross, armor oval, etc.
                    - Selected from texture atlas via iconType index
                    - Source: "GFX_onmap_counter_nato_symbols" or similar

Layer 2:           Organization / combat overlay
                    - Color-coded bar showing organization level
                    - Combat state indicator (attacking arrows, defending shield, retreating)
                    - Source: "GFX_onmap_counter_combat_overlay"

Layer 3 (top):     Stack count chip + text
                    - Small number badge showing division count
                    - Rendered via SDF (Signed Distance Field) for crisp numbers at any scale
                    - Source: SDF font atlas + procedural chip shape
```

The compositing uses standard alpha blending, approximately:

```hlsl
// Pseudocode for the 4-layer composite
float4 bg       = tex2D(layer0Sampler, layer0UV) * countryColor;
float4 nato     = tex2D(layer1Sampler, layer1UV);
float4 combat   = tex2D(layer2Sampler, layer2UV);
float4 chip     = renderStackChipSDF(stackCount, chipUV);

float4 result = bg;
result = lerp(result, nato, nato.a);
result = lerp(result, combat, combat.a);
result = lerp(result, chip, chip.a);
```

#### 3.3 Combat Blink Effect

When a unit is in combat, the counter pulses/blinks. This is done in the fragment shader:

```hlsl
// Combat blink pseudocode
float blinkPhase = sin(time * blinkFrequency) * 0.5 + 0.5;  // ~2-4 Hz pulse
float4 blinkColor = combatState == COMBAT_ATTACKING ? float4(1,0.2,0.2,1)   // Red
                  : combatState == COMBAT_DEFENDING ? float4(0.2,0.2,1,1)   // Blue
                  : float4(0,0,0,0);
result = lerp(result, blinkColor, blinkPhase * combatBlendAmount);
```

The blink rate is likely driven by a global time uniform provided by the engine's `game_time` variable.

#### 3.4 Stack Count Chip SDF

The stack count (number of divisions merged into one icon) is rendered using **Signed Distance Field** rendering for the digit glyphs. This allows crisp text at any zoom level without requiring separate mipmapped font textures.

```hlsl
// SDF chip rendering pseudocode
float chipDist = sdfRoundedRect(chipUV, chipSize, chipRadius);  // SDF for chip background
float chipBg = 1.0 - smoothstep(0.0, chipSoftness, chipDist);   // Anti-aliased fill

// Digit rendering from SDF font atlas
float digitDist = tex2D(sdfFontSampler, digitUV).r;
float digit = 1.0 - smoothstep(0.45, 0.55, digitDist);           // SDF threshold

float4 chipResult = chipBg * chipBgColor + digit * digitColor;
```

The SDF font atlas is typically a pre-rendered texture containing digits 0-9 (and possibly "99+" for overflow) as distance fields.

---

## 4. `interface/unitcounters.gfx` Sprite Definitions

The vanilla `.gfx` file defines the sprite types used by the counter system. Based on known sprite naming conventions in HOI4:

```gfx
spriteTypes = {
    # Main counter background sprite
    spriteType = {
        name = "GFX_onmap_unit_counter"
        texturefile = "gfx/interface/counters/unit_counter_background.dds"
        noOfFrames = 1
        effectFile = "gfx/FX/countergui.lua"
    }

    # NATO symbol overlay atlas
    spriteType = {
        name = "GFX_onmap_counter_nato_symbols"
        texturefile = "gfx/interface/counters/nato_symbols.dds"
        noOfFrames = X    # One frame per unit type (inf, arm, mot, mech, cav, etc.)
    }

    # Combat state overlays
    spriteType = {
        name = "GFX_onmap_counter_combat"
        texturefile = "gfx/interface/counters/combat_overlays.dds"
        noOfFrames = 4    # none, attacking, defending, retreating
    }

    # Counter variant for "counters" map mode (兵牌模式)
    spriteType = {
        name = "GFX_onmap_unit_counter_nato"
        texturefile = "gfx/interface/counters/unit_counter_nato.dds"
        noOfFrames = 1
    }

    # Sprite-based (non-NATO) unit model icons for normal map mode
    spriteType = {
        name = "GFX_onmap_unit_icon"
        texturefile = "gfx/interface/mapicons/unit_icons.dds"
        noOfFrames = X
    }

    # Stack count chip background
    spriteType = {
        name = "GFX_onmap_counter_stack_chip"
        texturefile = "gfx/interface/counters/stack_chip.dds"
    }

    # Country flag overlay for counters
    spriteType = {
        name = "GFX_onmap_counter_country_flag"
        texturefile = "gfx/interface/counters/country_flag_strip.dds"
        noOfFrames = X    # One per country or uses dynamic flag system
    }
}
```

**Note**: The actual vanilla file contains more entries. The specific sprite names `GFX_onmap_unit_counter`, `GFX_onmap_counter_nato_symbols`, and `GFX_onmap_counter_combat` are the core ones. The `effectFile` `countergui.lua` (or similar) provides the shader effect that binds the `.fxo` compiled shader to the sprite.

---

## 5. FX Effect Files (`.lua` in `gfx/FX/`)

The Clausewitz engine uses `.lua` files in `gfx/FX/` to define rendering effects. These are NOT standard Lua scripts - they are effect definition files that the engine's rendering backend parses to set up shader passes.

Typical structure:

```lua
-- Pseudocode for countergui.lua effect definition
effect = {
    name = "counterEffect",
    pass = {
        shader = "unit_counter",    -- References unit_counter.fxo
        blendMode = "alpha",
        zWrite = false,
        zTest = true,
        cullMode = "none",
        uniforms = {
            { name = "game_time",    type = "float" },
            { name = "country_color", type = "float4" },
            { name = "combat_state",  type = "int" },
            { name = "stack_count",   type = "int" },
        },
        textures = {
            { name = "layer0",  binding = 0 },  -- Background
            { name = "layer1",  binding = 1 },  -- NATO symbols
            { name = "layer2",  binding = 2 },  -- Combat overlay
            { name = "sdfFont", binding = 3 },  -- SDF font atlas
        }
    }
}
```

The engine compiles `.fxh` / `.shader` HLSL source into `.fxo` binaries at build time. These `.fxo` files ship with the game and are loaded at runtime. **Modders cannot directly modify the compiled shaders**, but they CAN:
1. Replace texture atlases (the `.dds` files referenced by sprites)
2. Modify the `.gfx` sprite definitions to point to custom textures
3. Change defines in `00_graphics.lua` to alter grouping/culling behavior
4. Use the `effectFile` field to point to custom `.lua` effect definitions

---

## 6. CPU-Side: CMapIconManager Grouping Pipeline

The CPU-side map icon manager handles the grouping and LOD system:

1. **Pass 1 - Distance Culling**: Remove icons beyond their distance cutoff
2. **Pass 2 - Grouping** (controlled by `MAPICON_GROUP_PASSES = 20`):
   - Each frame, up to 20 icons are processed for grouping evaluation
   - Icons within `MAP_ICONS_GROUP_CAM_DISTANCE = 90` are merged into stacks
   - At `MAP_ICONS_STATE_GROUP_CAM_DISTANCE = 180`, grouping is by state
   - At `MAP_ICONS_STRATEGIC_GROUP_CAM_DISTANCE = 350`, grouping becomes coarse
3. **Pass 3 - Stack Splitting**: When the player selects units (up to `MAP_ICONS_GROUP_SPLIT_SELECTED_LIMIT = 12`), grouped stacks split to show individual counters
4. **Pass 4 - Priority Sorting**: Icons are sorted by their `DEFAULT_PRIORITY_*` values for correct z-ordering
5. **Pass 5 - Per-Instance Data Assembly**: The manager packages each visible icon's data into the instanced draw call buffer

---

## 7. Data Flow: CPU → GPU

```
Per-Frame Update:

1. CMapIconManager::Update()
   ├── Evaluate camera distance for each icon
   ├── Run grouping passes (MAPICON_GROUP_PASSES per frame)
   ├── Determine visibility (culling, fade ranges)
   └── Build instance data buffer

2. Upload to GPU
   ├── Vertex Buffer: array of VS_INPUT_MAPICON structs
   ├── Constant Buffer: game_time, camera matrices
   └── Texture Bindings: 4 compositing layers + SDF font

3. Draw Call
   ├── Instanced rendering (one draw call for all visible counters)
   ├── Vertex Shader: billboard transform + z-bias
   └── Fragment Shader: 4-layer composite + combat blink + SDF chip
```

---

## 8. Counter Mode vs. Sprite Mode

HOI4 has two display modes for units on the map:

| Feature | Sprite Mode (Default) | Counter/NATO Mode (兵牌) |
|---------|----------------------|--------------------------|
| Visual | 3D unit model on map | 2D billboard counter |
| Shape | Varies by unit type | Uniform rectangular counter |
| Info displayed | Country color + model | Country flag + NATO symbol + org bar + stack count |
| Shader | `pdxmesh` 3D rendering | `maparrow.shader` → `MapSymbolDefault` Effect |
| Toggle | Default | Settings → "Use NATO Counters" |
| Sprite | `GFX_onmap_unit_icon` | `GFX_onmap_unit_counter_nato` |

---

## 9. Community Resources & Reverse Engineering

### Known community efforts:

1. **Paradox Forum Modding Subforum**: Contains discussions about counter texture replacement, but shader internals are not exposed to modders.

2. **Steam Workshop Counter Mods**: Popular mods (e.g., "Enhanced Counters", "Colorized Counters") replace the `.dds` texture atlases in `gfx/interface/counters/` and modify `unitcounters.gfx` to point to their new textures. They do NOT modify shaders.

3. **Clausewitz Engine Shader Tools**: The community tool `pdx_shader_decompiler` (various GitHub repos) can disassemble `.fxo` files back into HLSL-like bytecode, but the results are hard to read and lack original variable names.

4. **Jorodox/CEM Projects**: Various GitHub repos attempt Clausewitz engine documentation, but shader pipeline details remain sparse because the compiled `.fxo` format is proprietary.

5. **HOI4 Modding Wiki (paradoxwikis.com)**: Documents the `.gfx` format, defines, and interface modding, but does not cover shader internals.

### Key file paths in the vanilla game:

```
Hearts of Iron IV/
├── gfx/
│   ├── FX/                          # Effect definitions (.lua)
│   │   ├── countergui.lua           # Counter rendering effect
│   │   ├── gui.lua                  # General UI effects
│   │   └── *.fxo                    # Compiled shader binaries
│   ├── interface/
│   │   ├── countercounters/         # Counter textures
│   │   │   ├── unit_counter_background.dds
│   │   │   ├── nato_symbols.dds     # NATO symbol atlas
│   │   │   ├── combat_overlays.dds
│   │   │   └── stack_chip.dds
│   │   └── mapicons/               # Sprite-based icon textures
│   │       └── unit_icons.dds
│   └── ...
├── interface/
│   ├── unitcounters.gfx             # Sprite definitions for counters
│   ├── mapicons.gfx                 # Sprite definitions for map icons
│   └── *.gui                        # UI layout files
├── common/
│   └── defines/
│       ├── 00_defines.lua           # Gameplay defines
│       └── 00_graphics.lua          # Graphics defines (NMapIcons, NGraphics, etc.)
└── ...
```

---

## 10. Summary of Key Technical Details

| Aspect | Detail |
|--------|--------|
| **Shader source** | `gfx/FX/maparrow.shader` — **plain text, 644 lines, directly readable** (NOT compiled `.fxo`) |
| **Effect entry** | `MapSymbolDefault` (alpha blend) / `MapSymbolDefaultAdd` (additive) |
| **Rendering type** | **3D world-space instanced quad** (NOT screen-space billboard) |
| **Vertex shader** | `SymbolVertexShader` — `position * scale + rotate + translate → ViewProjectionMatrix` |
| **Fragment shader** | `SymbolPixelShader` — terrain normal → UV distortion → `SymbolColor` tint → `CalculateLighting` → `ApplyDistanceFog` → `DayNight` |
| **Per-instance data** | `ConstantBuffer(4)`: `{SymbolColor vec4, Position_Scale vec4, Time_Selected_Intersect_Rot vec4}` = 48 bytes |
| **Terrain interaction** | UV distorted by `CalculateTerrainNormal()` — counter "sticks" to terrain slope |
| **Lighting** | Full `CalculateLighting()` (Blinn-Phong + shadow PCF) — NOT flat/unlit |
| **Fog** | `ApplyDistanceFog()` — counters fade at distance |
| **Depth/Stencil** | `NoDepthStencilState` — depth disabled, stencil disabled, alpha blend, write RGB only |
| **Z-bias** | Controlled by `DEFAULT_PRIORITY_*` defines (10-30 range) |
| **Grouping** | `MAPICON_GROUP_PASSES = 20` icons processed per frame |
| **Distance grouping** | 90px → start grouping, 180px → state-level, 350px → strategic/coarse |
| **Combat state** | `isIntersect` flag → 25% scale increase in vertex shader |
| **Stack count** | Rendered by engine GUI layer (not in the shader itself) |
| **Country color** | `SymbolColor` vec4 multiplied in fragment shader |
| **Counter toggle** | Switches between `GFX_onmap_unit_icon` (sprite model) and counter textures |
| **Modding access** | Textures (`.dds`), sprites (`.gfx`), defines (`.lua`), effects (`.lua` in FX/) |

---

*Document updated 2026-05-17: Section 3.0 added with actual vanilla shader source code from `gfx/FX/maparrow.shader`. Previous inference (Section 3.1) superseded. The shader IS publicly available as plain text — earlier belief that it was compiled `.fxo` only was incorrect for the map symbol/counter pipeline.*
