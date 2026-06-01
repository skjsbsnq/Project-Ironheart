# Render Pipeline Rewrite

The new renderer is frame-graph driven.

## Required Objects

### `Renderer`

Owns GPU state and persistent pass resources.

### `RendererFrameInput`

Immutable data for one frame:

- camera
- game date/time
- map mode
- zoom class
- selected/hovered entities
- prepared draw lists
- resource quality status

### `FrameGraph`

Typed list of render nodes. It decides order, dependencies, and quality gates.

### `FramePlan`

The concrete executable plan for one frame. It is built from `RendererFrameInput`
and resource status.

## Pass Order

1. shadow
2. sky
3. terrain base
4. water
5. borders
6. static decals
7. semantic overlays
8. world objects
9. counters
10. labels
11. particles
12. postprocess
13. V9 UI
14. debug overlays

## Quality Gates

Every visual resource is classified:

- `Required`: missing means frame is invalid for production visuals.
- `DegradedAllowed`: frame can run but gets a visible degraded status.
- `DebugOnly`: only available in explicit debug fixture mode.

## Forbidden In Normal Frames

- hardcoded arrows
- mock trade routes
- invisible fallback that pretends to be final quality
- production screenshots without resource quality metadata
- pass execution controlled by a loose set of unrelated booleans

## Counter Rewrite

Counters are not a raw draw of every division at every zoom.

Pipeline:

```text
simulation snapshot
    -> counter visibility system
    -> aggregation system
    -> screen-space placement
    -> collision resolver
    -> CounterDrawList
    -> counter pass
```

This is required before the map can look readable at strategic zoom.
