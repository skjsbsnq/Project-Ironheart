# V9 UI Rewrite

V9 becomes the only UI route.

## Rules

- All colors come from V9 tokens.
- All spacing comes from V9 tokens.
- All typography comes from V9 text roles.
- All panels use V9 shell primitives.
- All commands are typed.
- Panels render from immutable data.

## Removed Concepts

- Legacy GPU panel/text/menu UI paths.
- Per-panel raw color constants.
- Mixed old/new panel shells.
- UI directly reaching into application state.
- Layout code that depends on deprecated immediate rect allocation patterns.

## Frame Model

The UI receives:

```text
UiFrameModel {
    topbar,
    side_rail,
    active_panel,
    map_hud,
    notifications,
    modals,
    debug,
}
```

It returns:

```text
Vec<UiCommand>
```

The engine maps commands to simulation or scene actions.

## Migration Order

1. Top bar.
2. Side rail.
3. Pause/speed/date controls.
4. Map HUD and selection panel.
5. Military/counter interaction UI.
6. Economy and construction panels.
7. Diplomacy and war panels.
8. Settings/save/end screens.
9. Main menu and country selection.

No new non-V9 panel work is allowed on the rewrite line.
