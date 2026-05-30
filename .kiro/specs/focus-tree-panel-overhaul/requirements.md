# Requirements Document

## Introduction

Overhaul the focus tree panel to deliver a near-fullscreen, HOI4-vanilla-style experience. The panel replaces the current small draggable `egui::Window` with a large overlay panel featuring square icon cards, drag-to-pan canvas navigation, a top progress bar for the currently researching focus, a bottom detail bar for the selected node, L-shaped connector lines, and game-pause-on-open behavior. Focus icons use solid-color placeholders (no DDS loading).

## Glossary

- **Focus_Panel**: The near-fullscreen egui panel that displays the national focus tree and provides interaction controls.
- **Focus_Node**: A single square card (80×80 px base) representing one national focus in the tree.
- **Canvas**: The pannable, zoomable drawing area inside Focus_Panel where Focus_Nodes and connector lines are rendered.
- **Pan_Offset**: A `Vec2` value tracking the cumulative drag displacement applied to the Canvas origin.
- **Progress_Bar**: A horizontal bar at the top of Focus_Panel showing the currently researching focus name and completion percentage.
- **Detail_Bar**: A horizontal bar at the bottom of Focus_Panel showing information about the currently selected Focus_Node.
- **Connector_Line**: An L-shaped (vertical-horizontal-vertical) polyline connecting a parent Focus_Node to a child Focus_Node.
- **Node_State**: One of four visual states a Focus_Node can be in: Completed, InProgress, Available, or Locked.
- **Effect_Summary**: A human-readable short string describing a focus completion effect with concrete values (e.g. "+2 民工(柏林)").
- **Game_State**: The simulation state object that holds the `paused` flag and current game speed.

## Requirements

### Requirement 1: Panel Layout

**User Story:** As a player, I want the focus tree panel to cover most of the screen, so that I can see the full tree without it feeling cramped.

#### Acceptance Criteria

1. WHEN the player opens Focus_Panel, THE Focus_Panel SHALL render as a fixed overlay occupying at least 80% of the screen width and 85% of the screen height, centered on screen.
2. THE Focus_Panel SHALL display a dark wood-tone background with color value `#1a1510` (RGB 26, 21, 16).
3. WHEN the player clicks the close button, THE Focus_Panel SHALL close and restore the previous game pause state.

### Requirement 2: Game Pause on Open

**User Story:** As a player, I want the game to pause when I open the focus tree, so that I can browse and decide without time pressure.

#### Acceptance Criteria

1. WHEN Focus_Panel transitions from closed to open, THE Focus_Panel SHALL emit a command that sets Game_State paused to true.
2. WHEN Focus_Panel transitions from open to closed, THE Focus_Panel SHALL emit a command that restores Game_State paused to the value it held before Focus_Panel was opened.

### Requirement 3: Canvas Drag-to-Pan

**User Story:** As a player, I want to drag the canvas with my left mouse button to navigate the tree, so that scrollbars are unnecessary.

#### Acceptance Criteria

1. WHILE the player holds the left mouse button on Canvas and drags, THE Canvas SHALL translate all rendered content by the drag delta accumulated in Pan_Offset.
2. THE Canvas SHALL retain Pan_Offset between frames while Focus_Panel remains open.
3. WHEN Focus_Panel is closed and reopened, THE Canvas SHALL reset Pan_Offset to `Vec2::ZERO`.

### Requirement 4: Canvas Zoom

**User Story:** As a player, I want to zoom in and out of the focus tree with my scroll wheel, so that I can see the big picture or inspect details.

#### Acceptance Criteria

1. WHILE the pointer is over Canvas, WHEN the player scrolls the mouse wheel, THE Canvas SHALL adjust the zoom factor by ±0.1 per scroll step, clamped between 0.5 and 2.0.
2. THE Canvas SHALL apply the zoom factor as a uniform scale to all node positions, node sizes, and connector lines.

### Requirement 5: Focus Node Appearance

**User Story:** As a player, I want focus nodes to look like square icon cards similar to HOI4 vanilla, so that the tree feels authentic.

#### Acceptance Criteria

1. THE Focus_Node SHALL render as a square card with base dimensions 80×80 pixels (before zoom scaling).
2. THE Focus_Node SHALL display a solid-color filled rectangle (52×52 px) centered within the card as an icon placeholder.
3. THE Focus_Node SHALL display the focus name below the card using a font size of 12px (before zoom scaling).
4. THE Focus_Panel SHALL use a grid spacing of 150 pixels horizontally and 150 pixels vertically between Focus_Node centers (before zoom scaling).

### Requirement 6: Node State Visualization

**User Story:** As a player, I want to see at a glance which focuses are completed, in progress, available, or locked, so that I can plan my path.

#### Acceptance Criteria

1. WHILE a Focus_Node is in Completed state, THE Focus_Node SHALL render with a green-tinted border and a checkmark overlay icon.
2. WHILE a Focus_Node is in InProgress state, THE Focus_Node SHALL render with a yellow-tinted border and a progress bar beneath the card showing completion percentage.
3. WHILE a Focus_Node is in Available state, THE Focus_Node SHALL render with full brightness and a light border.
4. WHILE a Focus_Node is in Locked state, THE Focus_Node SHALL render with reduced brightness (desaturated colors) and a dark border.

### Requirement 7: Node Selection

**User Story:** As a player, I want to click a focus node to select it and see its details, so that I can decide whether to research it.

#### Acceptance Criteria

1. WHEN the player left-clicks a Focus_Node, THE Focus_Panel SHALL set that node as the selected node and highlight it with a golden border.
2. WHEN a Focus_Node is selected, THE Detail_Bar SHALL become visible at the bottom of Focus_Panel.

### Requirement 8: Connector Lines

**User Story:** As a player, I want prerequisite lines to route cleanly without crossing through other nodes, so that the tree is readable.

#### Acceptance Criteria

1. THE Focus_Panel SHALL draw Connector_Lines between each Focus_Node and its prerequisite parent nodes.
2. THE Connector_Line SHALL follow an L-shaped path: vertical segment from parent bottom, horizontal segment at a midpoint Y, then vertical segment to child top.
3. THE Connector_Line SHALL use a muted color (RGB approximately 150, 140, 120) with 2px stroke width (before zoom scaling).

### Requirement 9: Mutual Exclusion Indicator

**User Story:** As a player, I want to see which focuses are mutually exclusive, so that I understand branching choices.

#### Acceptance Criteria

1. THE Focus_Panel SHALL draw a red X marker at the midpoint between each pair of mutually exclusive Focus_Nodes.
2. THE Focus_Panel SHALL draw each mutual exclusion pair only once regardless of declaration order.

### Requirement 10: Progress Bar (Top)

**User Story:** As a player, I want to see which focus I am currently researching and how far along it is, so that I know my progress at a glance.

#### Acceptance Criteria

1. WHILE a focus is currently being researched, THE Focus_Panel SHALL display Progress_Bar at the top of the panel showing the focus name and completion percentage.
2. IF no focus is currently being researched, THEN THE Focus_Panel SHALL hide Progress_Bar or display a "no focus selected" message.

### Requirement 11: Detail Bar (Bottom)

**User Story:** As a player, I want a detail bar showing the selected focus information including effects, so that I can make informed decisions.

#### Acceptance Criteria

1. WHEN a Focus_Node is selected, THE Detail_Bar SHALL display the focus name, cost in days, prerequisite focus names, and Effect_Summary.
2. THE Detail_Bar SHALL display Effect_Summary as human-readable strings with concrete numeric values (e.g. "+2 民工(柏林)").
3. WHILE the selected Focus_Node is in Available state and no other focus is in progress, THE Detail_Bar SHALL display a "Start" button.
4. WHEN the player clicks the "Start" button in Detail_Bar, THE Focus_Panel SHALL emit a `FocusCommand::Start` command with the selected focus id.

### Requirement 12: Cancel Current Focus

**User Story:** As a player, I want to cancel the focus I am currently researching, so that I can change my strategy.

#### Acceptance Criteria

1. WHILE a focus is currently being researched, THE Progress_Bar SHALL display a "Cancel" button.
2. WHEN the player clicks the "Cancel" button, THE Focus_Panel SHALL emit a `FocusCommand::Cancel` command.
