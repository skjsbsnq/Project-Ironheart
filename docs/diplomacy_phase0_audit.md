# Diplomacy Phase 0 Audit

This checklist is the phase 0 guardrail for the diplomacy/faction/UI rework. Keep it updated when a diplomacy state writer moves behind the future unified action API.

## Diplomacy State Writers

- `crates/hoi4-logic/src/diplomacy/factions.rs`: canonical faction create/join/leave/kick/dissolve/leadership transfer. Directly writes `world.diplomacy.factions` and currently reindexes `FactionId` after removal.
- `crates/hoi4-logic/src/diplomacy/war.rs`: canonical war declaration and war membership reconciliation. Writes `next_war_id`, `wars`, `world_tension`, `countries.at_war`; drains justified `pending_wargoals` through wargoal helpers.
- `crates/hoi4-logic/src/diplomacy/peace.rs`: canonical peace conference path. Removes `wars`, mutates `world_tension`, `opinions`, `annexed_countries`, state ownership, and `countries.at_war`.
- `crates/hoi4-logic/src/diplomacy/wargoal.rs`: canonical wargoal justification path. Writes `pending_wargoals` and claimant PP; advances/removes/drains pending goals.
- `crates/hoi4-logic/src/diplomacy/puppet.rs`: canonical autonomy path. Writes `autonomy`, `annexed_countries`, and subject state ownership during integration.
- `crates/hoi4-logic/src/diplomacy/tension.rs`: daily world tension update. Reads wars/pending goals and writes `world_tension`.
- `crates/hoi4-logic/src/feedback.rs`: event feedback helper. Writes `opinions` and `world_tension`; reads factions.
- `crates/hoi4-logic/src/trade/mod.rs`: trade/autonomy economy side effects. Reads diplomacy broadly and mutates `autonomy.progress` for subject-exporter trade effects.
- `crates/hoi4-content/src/eval.rs`: script effect executor. Directly writes `world_tension`, `opinions`, `factions`, `annexed_countries`, and `autonomy`.
- `crates/hoi4-content/src/v6_loader.rs`: historical initialization. Directly writes `autonomy` and initial `opinions`.
- `crates/hoi4-app/src/main.rs`: app/UI/debug command handling. Contains direct war creation, annexation, opinion modification, faction command dispatch, wargoal UI dispatch, and peace command dispatch; these are prime phase 1 migration targets.
- `crates/hoi4-ai/src/ground_orders.rs`: test/helper path directly inserts a war into `world.diplomacy.wars`; runtime AI mostly reads diplomacy and calls logic APIs.
- `crates/hoi4-state/src/save/text.rs` and `crates/hoi4-state/src/save/binary.rs`: save/load now serialize and restore diplomacy state for roundtrip coverage.

## Direct Writes To Tracked Fields

- `factions`: `hoi4-logic/src/diplomacy/factions.rs`; `hoi4-content/src/eval.rs`; command dispatch in `hoi4-app/src/main.rs` calls faction logic but also reads internals by index; AI strategic profile reads internals by index.
- `autonomy`: `hoi4-logic/src/diplomacy/puppet.rs`; `hoi4-logic/src/trade/mod.rs` (`get_mut` progress updates); `hoi4-content/src/eval.rs`; `hoi4-content/src/v6_loader.rs`; `hoi4-app/src/main.rs` reads autonomy for UI and subject summaries.
- `pending_wargoals`: `hoi4-logic/src/diplomacy/wargoal.rs`; `hoi4-logic/src/diplomacy/war.rs` drains via helpers; `hoi4-ai/src/diplomacy.rs` reads existing goals before action; `hoi4-app/src/main.rs` reads for UI and invokes wargoal/declare paths.
- `military_access`: `hoi4-state/src/diplomacy.rs` exposes `grant_military_access`/`revoke_military_access`; `hoi4-logic/src/military/frontline.rs` reads the raw set; future work should route reads through `has_military_access` and writes through unified actions.

## Read Entrypoints By System

- Military movement/frontline/air/naval arbiters: query `at_war_with`, `is_at_war`, `faction_of`, `factions`, and `military_access` for combat/movement legality.
- Trade and market logic: query war, faction, subject, autonomy, and opinions for route validity and trade scoring.
- AI: diplomacy strategy reads world tension, opinions, pending wargoals, faction membership, wars, and annexed countries; ground/naval/air order code reads war/annexation state.
- Content scripts and event ticks: triggers read war/faction/subject/opinion/tension/annexed state; effects can still mutate diplomacy directly.
- Render/app/UI: map border texture, topbar/status, diplomacy panel DTOs, right-click country info, province menu commands, and peace UI read diplomacy state.
- Save/load: text and binary save formats now include diplomacy state and preserve old saves by treating a missing diplomacy block/chunk as empty/default runtime state.

## Current UI Entrypoints

- Main diplomacy panel: `crates/hoi4-ui/src/diplomacy.rs`, data built and commands handled in `crates/hoi4-app/src/main.rs` around the diplomacy panel DTO and command loop.
- Right-click country info panel: `crates/hoi4-ui/src/country_info_panel.rs`, data built in `App::build_country_info_data` and commands handled in `crates/hoi4-app/src/main.rs`.
- Province menu declaration entry: `crates/hoi4-ui/src/province_menu.rs`, command handled in `crates/hoi4-app/src/main.rs`.

## Phase 1 Migration Risks

- Script effects and app/debug command handling currently bypass diplomacy logic in several places and must either call `execute_action` or be renamed as privileged effects.
- `FactionId` remains `Vec` index based; any phase 2 stable-ID migration must update logic, AI, UI DTO construction, rendering, script triggers, and save/load together.
- Military access has a state API but some readers still use the raw set, so request/treaty migration must replace raw reads before changing representation.
- Existing UI commands have inconsistent failure feedback; phase 1 should move availability and failure reasons into the logic layer before the UI rework.

## Guardrails Added In Phase 0

- `save_roundtrip` now mutates diplomacy state and compares roundtripped factions, wars, autonomy, pending wargoals, opinions, world tension, annexed countries, and military access.
- Text saves write a `diplomacy={...}` block and tolerate missing diplomacy blocks for old/minimal saves.
- Binary saves append an optional versioned `DIPLO` chunk and tolerate old binaries without the chunk.
