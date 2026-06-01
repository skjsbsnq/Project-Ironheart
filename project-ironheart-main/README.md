# Project Ironheart Main

This folder is the isolated rewrite line for Project Ironheart.

The existing main line is treated as frozen reference code. New work in this
folder uses the `ironheart-*` crate identity and is allowed to break away from
the old entry-layer structure.

## Rewrite Rules

- No giant application entry file.
- No rendering pass execution in the application shell.
- No mixed UI systems. V9 is the only UI style route.
- No silent visual fallback for production-quality rendering.
- No mock battlefield, route, or overlay data in normal gameplay frames.
- No product-facing legacy naming.

## Workspace

- `crates/ironheart-main`: binary shell only.
- `crates/ironheart-engine`: application orchestration and scene state.
- `crates/ironheart-render`: frame graph, pass contracts, resource quality gates.
- `crates/ironheart-ui`: V9-only UI model and style tokens.
- `crates/ironheart-sim`: simulation facade.

## First Checks

```powershell
cd project-ironheart-main
cargo check --workspace
```

## Git Line

This folder is intended to live on branch:

```text
project-ironheart/main-rewrite
```

The old `main` branch should remain frozen except for emergency fixes.
