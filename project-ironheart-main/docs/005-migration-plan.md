# Migration Plan

## Phase 0: Rewrite Line Setup

- Create isolated folder.
- Create branch.
- Add architecture contracts.
- Add compiling skeleton.

Exit criteria:

- `cargo check --workspace` passes inside the rewrite folder.
- Main branch is not modified.

## Phase 1: Engine Shell

- Implement window/event shell.
- Add engine state machine.
- Add fixed timestep and pause/speed model.
- No rendering parity work yet.

Exit criteria:

- Blank V9 shell runs.
- Engine can step a mock simulation snapshot.

## Phase 2: V9 UI First Screen

- Build V9 top bar and side rail.
- Use only V9 tokens.
- Add visual snapshot tests.

Exit criteria:

- No old UI route exists in the rewrite workspace.

## Phase 3: Renderer Frame Graph

- Build renderer facade.
- Add typed frame graph.
- Add resource quality gate model.
- Add debug fixture mode.

Exit criteria:

- A frame can be planned without the application shell knowing pass internals.

## Phase 4: Map Visual Rebuild

- Port only necessary asset loaders.
- Rebuild terrain/water/border/postprocess under new graph.
- Add fixed camera screenshot validation.

Exit criteria:

- Strategic map is readable before tactical details are added.

## Phase 5: Counter And Overlay Rebuild

- Add counter aggregation.
- Add collision/priority solver.
- Add front/order overlays only from real simulation snapshots.

Exit criteria:

- Wide-map screenshot is readable.

## Phase 6: Simulation Integration

- Port stable simulation facade.
- Add snapshot builders.
- Add command application.

Exit criteria:

- UI and renderer consume snapshots only.

## Phase 7: GitHub Milestones

- Push rewrite branch.
- Open milestone PRs by phase.
- Merge back only when the rewrite can replace the frozen line.
