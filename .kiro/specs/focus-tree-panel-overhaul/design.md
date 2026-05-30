# Design Document: Focus Tree Panel Overhaul

## Architecture Overview

The overhauled focus tree panel replaces the current `egui::Window`-based implementation with a near-fullscreen fixed overlay rendered via egui's immediate-mode API. The panel integrates into the existing `hoi4-app` main loop through the same `begin_frame` closure pattern used by all other UI panels.

```
┌─────────────────────────────────────────────────────────┐
│  hoi4-app (main.rs)                                     │
│  ┌───────────────────────────────────────────────────┐  │
│  │  begin_frame closure                              │  │
│  │    ├── topbar                                     │  │
│  │    ├── other panels...                            │  │
│  │    └── FocusTreePanel::show(ctx, tree, ...)       │  │
│  │            │                                      │  │
│  │            ├── reads FocusTree (hoi4-content)     │  │
│  │            ├── reads completed set (World)        │  │
│  │            ├── renders via egui Painter API       │  │
│  │            └── returns Option<FocusCommand>       │  │
│  └───────────────────────────────────────────────────┘  │
│  match focus_cmd { Start(id) => ..., Cancel => ... }    │
│  match pause_cmd { Pause => ..., Restore(speed) => ...} │
└─────────────────────────────────────────────────────────┘
```

### Data Flow

```
FocusTree (hoi4-content)
    │
    ▼
FocusTreePanel::show()
    ├── Input: &FocusTree, &HashSet<String>, Option<&str>, f32
    ├── Internal state: open, zoom, pan_offset, selected, saved_speed
    ├── Rendering: egui::Area (fixed overlay) + Painter API
    └── Output: Option<FocusCommand>, Option<PauseCommand>
         │
         ▼
    main.rs handles commands:
      - FocusCommand::Start(id) → start_focus()
      - FocusCommand::Cancel → clear current focus
      - PauseCommand::Pause → world.speed = Paused
      - PauseCommand::Restore(speed) → world.speed = saved
```

## Key Data Structures

### FocusTreePanel (revised)

```rust
/// Focus tree panel state — near-fullscreen overlay with pan/zoom canvas.
pub struct FocusTreePanel {
    /// Whether the panel is currently visible.
    pub open: bool,
    /// Uniform zoom factor applied to all canvas content.
    pub zoom: f32,
    /// Accumulated drag offset for canvas panning (pixels, unscaled).
    pub pan_offset: Vec2,
    /// Currently selected focus id (shown in detail bar).
    pub selected: Option<String>,
    /// Game speed saved when the panel was opened (for restore on close).
    saved_speed: Option<GameSpeed>,
}
```

### PauseCommand (new)

```rust
/// Command for game pause/restore, emitted alongside FocusCommand.
#[derive(Debug, Clone, PartialEq)]
pub enum PauseCommand {
    /// Set game speed to Paused.
    Pause,
    /// Restore game speed to the saved value.
    Restore(GameSpeed),
}
```

### FocusCommand (unchanged)

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum FocusCommand {
    Start(String),
    Cancel,
}
```

### FocusState (unchanged)

```rust
#[derive(Clone, Copy, PartialEq)]
pub enum FocusState {
    Completed,
    InProgress(f32),  // 0.0..1.0 completion fraction
    Available,
    Locked,
}
```

## Constants

```rust
const NODE_SIZE: f32 = 80.0;         // base node card size (px)
const ICON_SIZE: f32 = 52.0;         // icon placeholder rect (px)
const GRID_SPACING: f32 = 150.0;     // center-to-center distance (px)
const FONT_SIZE: f32 = 12.0;         // node label font size (px)
const MIN_ZOOM: f32 = 0.5;
const MAX_ZOOM: f32 = 2.0;
const ZOOM_STEP: f32 = 0.1;          // per scroll notch
const BG_COLOR: Color32 = Color32::from_rgb(26, 21, 16);  // #1a1510
const LINE_COLOR: Color32 = Color32::from_rgb(150, 140, 120);
const LINE_WIDTH: f32 = 2.0;
const PANEL_WIDTH_FRAC: f32 = 0.80;  // 80% of screen width
const PANEL_HEIGHT_FRAC: f32 = 0.85; // 85% of screen height
```

## Panel Layout

The panel is rendered as an `egui::Area` with fixed position (centered) rather than an `egui::Window`. This avoids the draggable title bar and gives full control over the overlay rectangle.

```rust
pub fn show(
    &mut self,
    ctx: &egui::Context,
    tree: &FocusTree,
    completed: &HashSet<String>,
    current_focus: Option<&str>,
    current_progress: f32,
    current_speed: GameSpeed,
) -> (Option<FocusCommand>, Option<PauseCommand>)
```

### Layout Computation

```rust
fn panel_rect(screen_size: Vec2) -> Rect {
    let w = screen_size.x * PANEL_WIDTH_FRAC;
    let h = screen_size.y * PANEL_HEIGHT_FRAC;
    let x = (screen_size.x - w) * 0.5;
    let y = (screen_size.y - h) * 0.5;
    Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h))
}
```

The panel is divided into three vertical zones:
1. **Top bar** (~32px): Progress bar showing current focus + cancel button
2. **Canvas** (fills remaining space): Pannable/zoomable node graph
3. **Detail bar** (~80px, visible only when a node is selected): Focus info + start button

## Canvas Pan & Zoom

### Pan (Drag-to-Pan)

The canvas uses egui's `Sense::drag()` on the allocated painter area. Each frame, the drag delta is accumulated into `pan_offset`:

```rust
if response.dragged_by(egui::PointerButton::Primary) {
    self.pan_offset += response.drag_delta();
}
```

`pan_offset` is reset to `Vec2::ZERO` when the panel transitions from closed to open (detected by checking `saved_speed.is_none()` on entry).

### Zoom (Scroll Wheel)

```rust
fn apply_scroll_zoom(&mut self, ctx: &egui::Context) {
    let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
    if scroll != 0.0 {
        let steps = (scroll / 50.0).round();  // normalize to discrete steps
        self.zoom = (self.zoom + steps * ZOOM_STEP).clamp(MIN_ZOOM, MAX_ZOOM);
    }
}
```

### World-to-Screen Transform

All node positions are computed from grid coordinates through a uniform transform:

```rust
fn node_screen_pos(grid_x: i32, grid_y: i32, min_x: i32, min_y: i32, 
                   zoom: f32, pan_offset: Vec2, canvas_origin: Pos2) -> Pos2 {
    let local_x = (grid_x - min_x) as f32 * GRID_SPACING + NODE_SIZE * 0.5;
    let local_y = (grid_y - min_y) as f32 * GRID_SPACING + NODE_SIZE * 0.5;
    Pos2::new(
        canvas_origin.x + local_x * zoom + pan_offset.x,
        canvas_origin.y + local_y * zoom + pan_offset.y,
    )
}
```

The zoom factor scales positions and sizes uniformly. Pan offset is applied after zoom (screen-space translation).

## Node Rendering

Each focus node is drawn using the egui `Painter` API:

```rust
fn draw_node(painter: &Painter, center: Pos2, focus: &Focus, state: FocusState, 
             zoom: f32, selected: bool) {
    let half = NODE_SIZE * 0.5 * zoom;
    let card_rect = Rect::from_center_size(center, Vec2::splat(NODE_SIZE * zoom));
    
    // 1. Card background + border (color depends on state)
    let (bg_color, border_color) = state_colors(state);
    painter.rect_filled(card_rect, CornerRadius::same(4.0 * zoom), bg_color);
    painter.rect_stroke(card_rect, CornerRadius::same(4.0 * zoom), 
                        Stroke::new(2.0 * zoom, border_color));
    
    // 2. Icon placeholder (solid color rect, 52×52 scaled)
    let icon_size = ICON_SIZE * zoom;
    let icon_rect = Rect::from_center_size(center, Vec2::splat(icon_size));
    painter.rect_filled(icon_rect, CornerRadius::ZERO, Color32::from_rgb(60, 50, 40));
    
    // 3. Selection highlight (golden border)
    if selected {
        painter.rect_stroke(card_rect, CornerRadius::same(4.0 * zoom),
                            Stroke::new(3.0 * zoom, Color32::from_rgb(255, 220, 100)));
    }
    
    // 4. State overlays (checkmark for completed, progress bar for in-progress)
    match state {
        FocusState::Completed => draw_checkmark(painter, center, zoom),
        FocusState::InProgress(pct) => draw_progress_bar(painter, card_rect, pct, zoom),
        _ => {}
    }
    
    // 5. Label below card
    let label_pos = Pos2::new(center.x, card_rect.max.y + 4.0 * zoom);
    // ... layout text with FONT_SIZE * zoom ...
}
```

### State Color Mapping

```rust
fn state_colors(state: FocusState) -> (Color32, Color32) {
    match state {
        FocusState::Completed   => (Color32::from_rgb(30, 45, 30), Color32::from_rgb(80, 200, 80)),
        FocusState::InProgress(_) => (Color32::from_rgb(40, 38, 25), Color32::from_rgb(220, 180, 50)),
        FocusState::Available   => (Color32::from_rgb(45, 40, 35), Color32::from_rgb(200, 200, 200)),
        FocusState::Locked      => (Color32::from_rgb(25, 25, 25), Color32::from_rgb(70, 70, 70)),
    }
}
```

## L-Shaped Connector Line Routing

### Algorithm

For each prerequisite relationship (parent → child), draw a 3-segment polyline:

1. **Vertical down** from parent card bottom center to a midpoint Y
2. **Horizontal** from parent X to child X at midpoint Y
3. **Vertical down** from midpoint Y to child card top center

The midpoint Y is computed as the average of parent bottom and child top:

```rust
fn connector_path(parent_center: Pos2, child_center: Pos2, zoom: f32) -> [Pos2; 4] {
    let half_node = NODE_SIZE * 0.5 * zoom;
    let parent_bottom = Pos2::new(parent_center.x, parent_center.y + half_node);
    let child_top = Pos2::new(child_center.x, child_center.y - half_node);
    let mid_y = (parent_bottom.y + child_top.y) * 0.5;
    
    [
        parent_bottom,                          // start: parent bottom
        Pos2::new(parent_center.x, mid_y),      // vertical down to mid
        Pos2::new(child_center.x, mid_y),       // horizontal to child X
        child_top,                              // vertical down to child top
    ]
}
```

The path is drawn as a polyline with `LINE_COLOR` and `LINE_WIDTH * zoom` stroke.

### Edge Case: Same Column

When parent and child share the same X coordinate, the horizontal segment has zero length, producing a straight vertical line (correct behavior, no special case needed).

## Mutual Exclusion Markers

Red X markers are drawn at the midpoint between each unique pair of mutually exclusive nodes. Deduplication uses a `HashSet<(min_id, max_id)>` to ensure each pair is drawn once regardless of declaration order in the data.

```rust
fn draw_mutual_exclusives(painter: &Painter, tree: &FocusTree, ...) {
    let mut drawn: HashSet<(&str, &str)> = HashSet::new();
    for focus in &tree.focuses {
        for me_id in &focus.mutually_exclusive {
            let pair = if focus.id.as_str() < me_id.as_str() {
                (focus.id.as_str(), me_id.as_str())
            } else {
                (me_id.as_str(), focus.id.as_str())
            };
            if !drawn.insert(pair) { continue; }
            // draw X at midpoint of the two node centers
        }
    }
}
```

## Hit Testing (Node Selection)

Click detection uses axis-aligned bounding box (AABB) testing against each node's screen-space rect:

```rust
fn hit_test_node(click_pos: Pos2, node_center: Pos2, zoom: f32) -> bool {
    let half = NODE_SIZE * 0.5 * zoom;
    let rect = Rect::from_center_size(node_center, Vec2::splat(NODE_SIZE * zoom));
    rect.contains(click_pos)
}
```

Nodes are tested in reverse draw order (front-to-back) so overlapping nodes at high zoom select the topmost.

## Progress Bar (Top)

When `current_focus.is_some()`, a horizontal bar is drawn at the top of the panel:

```
┌──────────────────────────────────────────────────────────┐
│  [Focus Name]  ████████████░░░░░░░░  67%    [Cancel]     │
└──────────────────────────────────────────────────────────┘
```

When no focus is active, the bar either hides or shows a muted "No focus selected" label.

## Detail Bar (Bottom)

When `self.selected.is_some()`, an 80px-tall bar appears at the panel bottom:

```
┌──────────────────────────────────────────────────────────┐
│  [Focus Name]     Cost: 70 days                          │
│  Requires: focus_a, focus_b                              │
│  Effects: +2 民工(柏林), +1 军工(柏林)     [Start]       │
└──────────────────────────────────────────────────────────┘
```

The "Start" button is only shown when:
- The selected node is in `Available` state
- No other focus is currently in progress (`current_focus.is_none()`)

## Effect Summary Generation

A new `effect_summary()` method on the `Effect` enum produces human-readable strings:

```rust
impl Effect {
    /// Produce a short human-readable summary string for UI display.
    pub fn effect_summary(&self) -> String {
        match self {
            Effect::AddPoliticalPower(v) => format!("{:+.0} 政治力量", v),
            Effect::AddStability(v) => format!("{:+.1}% 稳定度", v * 100.0),
            Effect::AddWarSupport(v) => format!("{:+.1}% 战争支持", v * 100.0),
            Effect::AddManpower(v) => format!("{:+} 人力", v),
            Effect::AddResearchSlot(v) => format!("{:+} 研究槽", v),
            Effect::AddBuildingInState { state, building, level } => {
                format!("{:+} {}(州{})", level, building_name(building), state)
            }
            Effect::AddIdea(id) => format!("获得国家精神: {}", id),
            Effect::RemoveIdea(id) => format!("移除国家精神: {}", id),
            Effect::SetTechnology(id) => format!("获得科技: {}", id),
            Effect::AddTechBonus { category, bonus } => {
                format!("+{:.0}% 研究加成({})", bonus * 100.0, category)
            }
            Effect::ArmyExperience(v) => format!("{:+.0} 陆军经验", v),
            Effect::NavyExperience(v) => format!("{:+.0} 海军经验", v),
            Effect::AirExperience(v) => format!("{:+.0} 空军经验", v),
            Effect::TransferState(s) => format!("获得州 {}", s),
            Effect::AnnexCountry(tag) => format!("吞并 {}", tag),
            Effect::DeclareWarOn(tag) => format!("对 {} 宣战", tag),
            Effect::CreateFaction(name) => format!("创建阵营: {}", name),
            Effect::AddToFaction(tag) => format!("{} 加入阵营", tag),
            Effect::If { effects, .. } => {
                effects.iter().map(|e| e.effect_summary()).collect::<Vec<_>>().join(", ")
            }
            // Fallback for less common effects
            other => format!("{:?}", other),
        }
    }
}
```

This function lives in `hoi4-content/src/focus.rs` (or a new `effect_display.rs` module) since it operates on the `Effect` enum defined there.

## Game Pause Integration

### Open Transition

When `show()` detects the panel just opened (via `saved_speed.is_none()` while `open == true`):
1. Store `current_speed` into `self.saved_speed`
2. Return `Some(PauseCommand::Pause)` alongside any focus command

### Close Transition

When the close button is clicked or `open` becomes false:
1. Return `Some(PauseCommand::Restore(saved_speed.unwrap()))`
2. Clear `saved_speed` to `None`
3. Reset `pan_offset` to `Vec2::ZERO`

The caller in `main.rs` handles `PauseCommand` by setting `world.speed` accordingly.

## Focus State Determination

The `focus_state()` function remains largely unchanged but is extracted as a public utility for testability:

```rust
pub fn focus_state(
    focus: &Focus,
    completed: &HashSet<String>,
    current_focus: Option<&str>,
    progress: f32,
) -> FocusState {
    if completed.contains(&focus.id) {
        return FocusState::Completed;
    }
    if current_focus == Some(focus.id.as_str()) {
        return FocusState::InProgress(progress / focus.cost_days as f32);
    }
    // Check prerequisites (AND of OR groups)
    for group in &focus.prerequisites {
        if !group.iter().any(|p| completed.contains(p)) {
            return FocusState::Locked;
        }
    }
    // Check mutual exclusions
    for me in &focus.mutually_exclusive {
        if completed.contains(me) {
            return FocusState::Locked;
        }
    }
    FocusState::Available
}
```

## Error Handling

- **Empty focus tree**: If `tree.focuses` is empty, the canvas renders nothing and the panel shows a "No focuses available" message.
- **Missing prerequisite references**: If a prerequisite ID doesn't exist in the tree, the connector line is silently skipped (no panic).
- **Zoom/pan overflow**: All positions are `f32`; extreme zoom + pan combinations are clamped by the zoom limits and egui's clip rect.

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system — essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Panel dimensions meet minimum coverage

*For any* screen size (width > 0, height > 0), the computed panel rect SHALL have width >= 80% of screen width and height >= 85% of screen height, and the panel center SHALL equal the screen center.

**Validates: Requirements 1.1**

### Property 2: Game speed round-trip on open/close

*For any* initial game speed (non-Paused), opening the focus panel and then closing it SHALL restore the game speed to its original value.

**Validates: Requirements 2.1, 2.2**

### Property 3: Pan offset accumulates drag deltas

*For any* sequence of drag delta vectors, the resulting pan_offset SHALL equal the vector sum of all deltas in the sequence.

**Validates: Requirements 3.1, 3.2**

### Property 4: Pan offset resets on reopen

*For any* pan_offset value, closing and reopening the panel SHALL result in pan_offset equal to Vec2::ZERO.

**Validates: Requirements 3.3**

### Property 5: Zoom clamping

*For any* current zoom value in [0.5, 2.0] and any scroll delta, the resulting zoom SHALL remain within [0.5, 2.0].

**Validates: Requirements 4.1**

### Property 6: Zoom scales node positions uniformly

*For any* grid position (x, y) and zoom factor z in [0.5, 2.0], the screen-space distance between two adjacent nodes SHALL equal GRID_SPACING * z.

**Validates: Requirements 4.2, 5.4**

### Property 7: L-shaped connector path geometry

*For any* parent node center and child node center where parent.y < child.y, the connector path SHALL consist of exactly 4 points forming: a vertical segment, a horizontal segment, and a vertical segment, with the horizontal segment at the Y-midpoint between parent bottom and child top.

**Validates: Requirements 8.2**

### Property 8: Mutual exclusion deduplication

*For any* focus tree with mutual exclusion declarations, the number of unique drawn markers SHALL equal the number of unique unordered pairs {A, B} where A declares B as mutually exclusive or B declares A as mutually exclusive.

**Validates: Requirements 9.1, 9.2**

### Property 9: Hit test correctness

*For any* click position within a node's screen-space bounding box (center ± NODE_SIZE/2 * zoom), the hit test SHALL return true for that node. For any click position outside all node bounding boxes, no node SHALL be selected.

**Validates: Requirements 7.1**

### Property 10: Effect summary produces non-empty output

*For any* Effect variant (excluding `If` with empty effects list), the `effect_summary()` function SHALL return a non-empty string.

**Validates: Requirements 11.2**

### Property 11: Start button visibility invariant

*For any* selected focus node, the "Start" button SHALL be visible if and only if the node's state is `Available` AND `current_focus` is `None`.

**Validates: Requirements 11.3**

### Property 12: Focus state determination correctness

*For any* focus node and completed set, if all prerequisite OR-groups are satisfied (at least one member completed per group) AND no mutually exclusive focus is completed AND the focus is not itself completed or in progress, then the focus state SHALL be `Available`.

**Validates: Requirements 6.1, 6.2, 6.3, 6.4**
