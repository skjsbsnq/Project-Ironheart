# Visual Failure Audit

This audit is based on the current wide-map gameplay screenshot.

## Observed Failures

### Counter Layer

The counter layer is the most obvious failure. It draws too many tactical
objects at a strategic zoom level. Counters overlap across Europe, numbers
detach visually, and the player cannot read army density, front direction, or
ownership at a glance.

Root causes:

- No zoom-aware aggregation policy.
- No screen-space collision budget.
- No priority sorting by player relevance, war state, front proximity, or
  selected armies.
- Tactical symbols are rendered at a scale that belongs to a closer zoom.

Required rewrite:

- Strategic zoom: aggregate counters by army/front/state.
- Operational zoom: show stacks with fan-out only near hover/selection.
- Tactical zoom: show individual divisions.
- Renderer receives a prepared `CounterDrawList`, not raw world divisions.

### Map Material

The terrain reads as noisy and overprocessed. Political color, terrain texture,
cloud/snow/fog-like overlays, border grid, and province texture all compete in
the same frequency range.

Root causes:

- Terrain, political overlay, water, borders, season, and postprocess are not
  owned by a calibrated frame graph.
- The map does not have separate visual budgets for strategic, operational, and
  tactical zoom.
- Degraded or procedural paths are visually too close to production paths, so
  bad frames look "accepted" instead of clearly invalid.

Required rewrite:

- Separate base terrain, ownership tint, semantic overlays, and weather/season.
- Add per-zoom material budgets.
- Mark critical visual fallback as invalid for final-quality screenshots.

### Water And Coast

The sea is visually dominant and inconsistent with land readability. Coastlines
and shallow water are attractive in places, but the color pipeline makes the
Atlantic and Mediterranean overpower the strategic map.

Required rewrite:

- Water pass owns final sea color only when all critical water resources pass
  quality gates.
- Strategic map mode reduces water contrast behind counters and labels.
- Coast/foam intensity is calibrated per zoom.

### UI

The top bar and side rail still feel like a legacy skin. The rest of the screen
contains V9-like pieces, older rectangular widgets, raw one-character buttons,
and inconsistent spacing.

Root causes:

- Multiple UI systems are active.
- V9 tokens exist but are not the only source of colors, spacing, typography,
  and panel structure.
- UI is assembled from application state directly instead of a stable frame
  model.

Required rewrite:

- V9 is the only UI system.
- All UI receives `UiFrameModel` data.
- All UI actions return typed commands.
- No panel reads or mutates engine state directly.

### Composition

The screenshot has no clear hierarchy. Map, counters, labels, UI chrome, and
debug-like overlays all compete for attention.

Required rewrite:

- Define a frame composition contract:
  1. base map
  2. ownership/material overlays
  3. strategic overlays
  4. world objects
  5. counters
  6. labels
  7. V9 UI
  8. debug overlays
- Each layer has a per-zoom density and alpha budget.
