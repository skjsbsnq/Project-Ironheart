# Main Freeze Contract

Project Ironheart now has a dedicated rewrite line.

## Branches

- Frozen reference line: `main`
- Rewrite line: `project-ironheart/main-rewrite`

## Policy

1. The frozen line is not used for broad architecture work.
2. The rewrite line owns all new architecture, naming, rendering, and V9 UI work.
3. Historical code may be read, measured, and selectively ported, but not copied
   wholesale into a new giant entry file.
4. Any ported feature must enter through a defined boundary: engine, renderer,
   UI, simulation, assets, or content.

## GitHub Upload

Push the rewrite line as a separate branch:

```powershell
git push -u origin project-ironheart/main-rewrite
```

Recommended repository settings:

- Protect `main`.
- Require pull requests into `main`.
- Use the rewrite branch for long-running work.
- Merge back only through milestone PRs, not one huge final dump.
