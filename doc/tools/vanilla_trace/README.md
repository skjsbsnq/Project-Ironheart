# Vanilla Trace Artifacts

This directory stores extracted facts from the vanilla HOI4 renderer.

Current phase outputs:

```text
shader_compiles.json
shader_bindings.json
logs/d3d11_trace.jsonl
runtime_targets.json
render_passes.json
state_objects.json
constant_buffers.json
representative_frame.json
pass_resource_flow.json
posteffect_values.json
terrain_pdxmap.json
binary_xrefs.md
project_mapping.md
```

Phase 3 runtime tracing is generated from the proxy DLLs in:

```text
tools/d3d11_trace_proxy/
```

Build and capture:

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\d3d11_trace_proxy\build.ps1
powershell -ExecutionPolicy Bypass -File .\tools\d3d11_trace_proxy\capture_hoi4_shaders.ps1 -TimeoutSeconds 120
powershell -ExecutionPolicy Bypass -File .\tools\vanilla_trace\summarize_d3d11_trace.ps1 -TraceDir .\tools\vanilla_trace
```

`runtime_targets.json` lists RTV/DSV/SRV views with DXGI format, size, bind flags,
and observed bind counts.

`state_objects.json` lists sampler, blend, depth-stencil, and rasterizer state
objects when captured with the R2 proxy. Older logs generated before R2 have this
file present with `state_object_count=0`.

`constant_buffers.json` lists D3D11 buffers and VS/PS constant-buffer slot usage
when captured with the R2 proxy. Older logs generated before R2 have this file
present with `constant_buffer_count=0`.

`representative_frame.json` stores the selected high-signal frame and its
state-reconstructed pass order.

`pass_resource_flow.json` resolves the representative frame's pass reads/writes
to SRV/RTV/DSV ids, texture format/size, shader hashes, cbuffers, samplers, and
state object ids where available.

`posteffect_values.json` explains the current postprocess LUT selection from
`gfx/posteffect_volumes.txt`, including ColorCube TGA loading, runtime LUT keys,
RestoreScene bindings, and HDR/LDR/sRGB responsibility boundaries.

`terrain_pdxmap.json` explains the current terrain/pdxmap binding and material
composition path, including map-pixel coordinate formulas, political tint
budget, terrain atlas/colormap/snow/mud/FOW inputs, ownership gates, and
project-only art feature isolation.

`binary_xrefs.md` records the trace-backed RestoreScene and ColorCube binding
cross-reference used by Phase 5.

`render_passes.json` contains:

- `passes`: draw-call pass groups from explicit draw hooks.
- `representative_pass_order`: a state-flow pass order reconstructed from
  `OMSetRenderTargets`, `VSSetShader`, `PSSetShader`, and
  `PSSetShaderResources`.
- `state_pass_kind_counts`: classified runtime shader state counts, including
  terrain, water, river, border, postfx, LUT blend, trees, pdxmesh, and UI.

Known phase 3 limitation: on the captured HOI4 run, explicit draw hooks only
recorded early startup `Draw` calls. The main map pass graph is therefore derived
from shader/target/resource state transitions, not direct draw-call counts. This
still links shader hashes, target views, resource names, and approximate pass
order for terrain/water/river/border/postfx.

No vanilla shaders, textures, binaries, or binary-derived payloads should be
committed to this repository.
