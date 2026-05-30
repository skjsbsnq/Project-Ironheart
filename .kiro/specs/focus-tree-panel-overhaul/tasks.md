# Implementation Plan: Focus Tree Panel Overhaul

## Overview

Replace the current small draggable `egui::Window`-based focus tree panel with a near-fullscreen fixed overlay featuring square icon cards, drag-to-pan canvas, L-shaped connector lines, a top progress bar, a bottom detail bar, and game-pause-on-open behavior. The implementation touches three crates: `hoi4-content` (effect summary), `hoi4-ui` (panel rewrite), and `hoi4-app` (caller update).

## Tasks

- [ ] 1. Add `effect_summary()` method and `PauseCommand` enum
  - [ ] 1.1 Implement `effect_summary()` on the `Effect` enum in `crates/hoi4-content/src/focus.rs`
    - Add `pub fn effect_summary(&self) -> String` method on `impl Effect`
    - Cover all ~50 variants with human-readable Chinese strings (e.g. "+2 民工(柏林)")
    - Use `format!("{:?}", self)` as fallback for uncommon variants
    - _Requirements: 11.2_

  - [ ] 1.2 Add `PauseCommand` enum to `crates/hoi4-ui/src/focus_tree_panel.rs`
    - Define `PauseCommand { Pause, Restore(GameSpeed) }`
    - Add `use hoi4_state::GameSpeed;` import (or re-export path as needed)
    - _Requirements: 2.1, 2.2_

  - [ ]* 1.3 Write property test for `effect_summary()` non-empty output
    - **Property 10: Effect summary produces non-empty output**
    - **Validates: Requirements 11.2**

- [ ] 2. Rewrite `FocusTreePanel` struct and constants
  - [ ] 2.1 Replace constants at the top of `crates/hoi4-ui/src/focus_tree_panel.rs`
    - Remove old constants (`NODE_W`, `NODE_H`, `GRID_SPACING_X`, `GRID_SPACING_Y`, `RING_RADIUS`, `PROGRESS_H`)
    - Add new constants: `NODE_SIZE=80.0`, `ICON_SIZE=52.0`, `GRID_SPACING=150.0`, `FONT_SIZE=12.0`, `MIN_ZOOM=0.5`, `MAX_ZOOM=2.0`, `ZOOM_STEP=0.1`, `BG_COLOR`, `LINE_COLOR`, `LINE_WIDTH=2.0`, `PANEL_WIDTH_FRAC=0.80`, `PANEL_HEIGHT_FRAC=0.85`
    - _Requirements: 1.1, 1.2, 5.1, 5.4, 8.3_

  - [ ] 2.2 Update `FocusTreePanel` struct with new fields
    - Add `pan_offset: Vec2` field
    - Add `saved_speed: Option<GameSpeed>` field
    - Update `new()` to initialize `pan_offset: Vec2::ZERO` and `saved_speed: None`
    - _Requirements: 2.1, 3.1, 3.3_

- [ ] 3. Implement the new `show()` method structure
  - [ ] 3.1 Rewrite `show()` signature and panel frame using `egui::Area`
    - Change signature to accept `current_speed: GameSpeed` and return `(Option<FocusCommand>, Option<PauseCommand>)`
    - Replace `egui::Window` with `egui::Area` fixed at screen center
    - Compute panel rect using `PANEL_WIDTH_FRAC` / `PANEL_HEIGHT_FRAC`
    - Paint dark background (`BG_COLOR`) filling the panel rect
    - Add close button that sets `open = false`
    - _Requirements: 1.1, 1.2, 1.3_

  - [ ] 3.2 Implement game pause integration (open/close transitions)
    - On open transition (`saved_speed.is_none()` while `open == true`): store `current_speed` into `saved_speed`, emit `PauseCommand::Pause`
    - On close: emit `PauseCommand::Restore(saved_speed.unwrap())`, clear `saved_speed`, reset `pan_offset` to `Vec2::ZERO`
    - _Requirements: 2.1, 2.2, 3.3_

  - [ ] 3.3 Implement top progress bar section
    - When `current_focus.is_some()`: draw focus name + filled progress bar + percentage + Cancel button
    - When no focus active: show muted "No focus selected" label
    - Cancel button emits `FocusCommand::Cancel`
    - _Requirements: 10.1, 10.2, 12.1, 12.2_

  - [ ] 3.4 Implement canvas area with drag-to-pan and scroll-zoom
    - Allocate painter with `Sense::drag()` and `Sense::click()`
    - Accumulate `response.drag_delta()` into `self.pan_offset` on primary drag
    - Apply scroll-wheel zoom: `±ZOOM_STEP` per scroll notch, clamped `[MIN_ZOOM, MAX_ZOOM]`
    - _Requirements: 3.1, 3.2, 4.1, 4.2_

  - [ ] 3.5 Implement bottom detail bar
    - When `self.selected.is_some()`: draw 80px bar at panel bottom
    - Show focus name, cost in days, prerequisite names, and effect summaries (using `effect_summary()`)
    - Show "Start" button only when node is `Available` AND `current_focus.is_none()`
    - Start button emits `FocusCommand::Start(id)`
    - _Requirements: 7.2, 11.1, 11.2, 11.3, 11.4_

  - [ ]* 3.6 Write property test for game speed round-trip
    - **Property 2: Game speed round-trip on open/close**
    - **Validates: Requirements 2.1, 2.2**

  - [ ]* 3.7 Write property test for pan offset accumulation
    - **Property 3: Pan offset accumulates drag deltas**
    - **Validates: Requirements 3.1, 3.2**

  - [ ]* 3.8 Write property test for pan offset reset on reopen
    - **Property 4: Pan offset resets on reopen**
    - **Validates: Requirements 3.3**

  - [ ]* 3.9 Write property test for zoom clamping
    - **Property 5: Zoom clamping**
    - **Validates: Requirements 4.1**

- [ ] 4. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 5. Implement node rendering and connector lines
  - [ ] 5.1 Rewrite `draw_node()` function (circle → square card)
    - Draw square card (`NODE_SIZE × NODE_SIZE * zoom`) with rounded corners
    - Use `state_colors()` for background/border based on `FocusState`
    - Draw icon placeholder rect (`ICON_SIZE × ICON_SIZE * zoom`) centered
    - Draw golden selection highlight border when selected
    - Draw checkmark overlay for `Completed`, progress bar for `InProgress`
    - Draw focus name label below card at `FONT_SIZE * zoom`
    - _Requirements: 5.1, 5.2, 5.3, 6.1, 6.2, 6.3, 6.4_

  - [ ] 5.2 Rewrite `draw_lines()` function (straight → L-shaped connectors)
    - For each prerequisite edge: compute 4-point L-shaped path (vertical-horizontal-vertical)
    - Midpoint Y = average of parent bottom and child top
    - Draw as polyline with `LINE_COLOR` and `LINE_WIDTH * zoom`
    - Handle same-column case (zero-length horizontal segment = straight line)
    - _Requirements: 8.1, 8.2, 8.3_

  - [ ] 5.3 Update `draw_mutual_exclusive()` for new node geometry
    - Adjust midpoint calculation to use new `NODE_SIZE`-based positions
    - Keep red X marker rendering and deduplication logic
    - _Requirements: 9.1, 9.2_

  - [ ] 5.4 Implement hit testing for node selection (click detection)
    - Use AABB test against `NODE_SIZE * zoom` rect at each node center
    - Test nodes in reverse draw order for correct overlap handling
    - Set `self.selected` on click
    - _Requirements: 7.1_

  - [ ] 5.5 Update `node_center_z()` helper for uniform `GRID_SPACING`
    - Replace separate `GRID_SPACING_X` / `GRID_SPACING_Y` with single `GRID_SPACING`
    - Apply `pan_offset` and `zoom` in the world-to-screen transform
    - _Requirements: 4.2, 5.4_

  - [ ]* 5.6 Write property test for L-shaped connector geometry
    - **Property 7: L-shaped connector path geometry**
    - **Validates: Requirements 8.2**

  - [ ]* 5.7 Write property test for zoom scaling node positions uniformly
    - **Property 6: Zoom scales node positions uniformly**
    - **Validates: Requirements 4.2, 5.4**

  - [ ]* 5.8 Write property test for mutual exclusion deduplication
    - **Property 8: Mutual exclusion deduplication**
    - **Validates: Requirements 9.1, 9.2**

  - [ ]* 5.9 Write property test for hit test correctness
    - **Property 9: Hit test correctness**
    - **Validates: Requirements 7.1**

- [ ] 6. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 7. Update caller in `main.rs` and wire everything together
  - [ ] 7.1 Update `focus_panel.show()` call site in `crates/hoi4-app/src/main.rs`
    - Pass `current_speed: GameSpeed` (from `world.speed`) as new parameter
    - Destructure return value as `(Option<FocusCommand>, Option<PauseCommand>)`
    - Handle `PauseCommand::Pause` by setting `world.speed = GameSpeed::Paused`
    - Handle `PauseCommand::Restore(speed)` by setting `world.speed = speed`
    - _Requirements: 2.1, 2.2_

  - [ ] 7.2 Extract `focus_state()` as a public function
    - Make `focus_state()` `pub` for external testability
    - Ensure it handles all prerequisite AND/OR logic and mutual exclusion checks
    - _Requirements: 6.1, 6.2, 6.3, 6.4_

  - [ ]* 7.3 Write property test for focus state determination
    - **Property 12: Focus state determination correctness**
    - **Validates: Requirements 6.1, 6.2, 6.3, 6.4**

  - [ ]* 7.4 Write property test for Start button visibility invariant
    - **Property 11: Start button visibility invariant**
    - **Validates: Requirements 11.3**

  - [ ]* 7.5 Write property test for panel dimensions
    - **Property 1: Panel dimensions meet minimum coverage**
    - **Validates: Requirements 1.1**

- [ ] 8. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties from the design document
- Unit tests validate specific examples and edge cases
- The implementation language is Rust, matching the existing codebase
- `GameSpeed` is imported from `hoi4_state::GameSpeed` (variants: `Paused`, `Speed1`, `Speed2`, `Speed3`, `Speed4`, `Speed5`)
- The `Effect` enum has ~50 variants; `effect_summary()` should cover all with Chinese UI strings

## Task Dependency Graph

```json
{
  "waves": [
    { "id": 0, "tasks": ["1.1", "1.2", "2.1", "2.2"] },
    { "id": 1, "tasks": ["1.3", "3.1"] },
    { "id": 2, "tasks": ["3.2", "3.3", "3.4", "3.5"] },
    { "id": 3, "tasks": ["3.6", "3.7", "3.8", "3.9", "5.5"] },
    { "id": 4, "tasks": ["5.1", "5.2", "5.3", "5.4"] },
    { "id": 5, "tasks": ["5.6", "5.7", "5.8", "5.9"] },
    { "id": 6, "tasks": ["7.1", "7.2"] },
    { "id": 7, "tasks": ["7.3", "7.4", "7.5"] }
  ]
}
```
