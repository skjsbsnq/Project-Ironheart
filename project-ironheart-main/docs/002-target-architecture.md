# Target Architecture

Project Ironheart is split into explicit runtime domains.

## Crates

### `ironheart-main`

Binary shell only.

Responsibilities:

- Start the process.
- Own platform integration.
- Forward events into the engine.
- Never own rendering passes, UI panels, or simulation systems.

### `ironheart-engine`

Application orchestration.

Responsibilities:

- Scene state.
- Input command routing.
- Simulation stepping policy.
- Renderer and UI coordination.
- Save/load orchestration.

The engine does not draw. It builds frame inputs.

### `ironheart-render`

Renderer and frame graph.

Responsibilities:

- GPU device resources.
- Render graph planning.
- Pass execution.
- Resource quality gates.
- Screenshot and visual validation hooks.

The renderer does not know about UI panels or direct gameplay mutation.

### `ironheart-ui`

V9-only UI system.

Responsibilities:

- V9 tokens.
- Layout primitives.
- Panel shells.
- UI frame model rendering.
- Typed UI commands.

No old UI pass, no parallel theme system, no raw panel-specific style constants.

### `ironheart-sim`

Simulation facade.

Responsibilities:

- World state facade.
- Tick scheduling.
- Stable snapshots for UI and rendering.
- Command application.

The UI and renderer consume snapshots, not mutable simulation internals.

## Data Flow

```text
platform events
    -> ironheart-main
    -> ironheart-engine
    -> ironheart-sim commands/ticks
    -> snapshots
    -> ironheart-render frame inputs
    -> ironheart-ui frame model
```

## Hard Boundary Rules

- Renderer never calls UI.
- UI never mutates simulation directly.
- Simulation never imports renderer or UI.
- Main never owns pass internals.
- Debug fixtures are opt-in and cannot be compiled into normal gameplay by
  accident.
