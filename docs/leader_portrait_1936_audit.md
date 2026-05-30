# 1936 Leader Portrait Audit

## Scope

This audit covers the politics-panel leader portrait pipeline:

- `history/countries/*.txt` sets the 1936 ruling party and initial recruited characters.
- `common/characters/<TAG>.txt` defines `country_leader` roles and portrait GFX keys.
- `gfx/leaders/<TAG>` and DLC `gfx/leaders/<TAG>` provide portrait DDS files.

The project currently displays HOI4-style `country_leader`, not a separate constitutional head-of-state field.

## Findings

1. Leader selection ignored `recruit_character` order.

   Vanilla history files often rely on initial recruitment order to decide which matching `country_leader` becomes active. England explicitly documents this with `# Order matters - here Chamberlain becomes starting leader` in its history file. The previous loader selected the first matching character from globally sorted character definitions, which could pick the wrong person for countries with multiple leaders under the same top-level ideology.

2. Character sort order could change leader selection.

   The previous sort used `(tag, key)`, so same-tag leaders were ordered alphabetically rather than by source/history order. This made selection deterministic but historically wrong.

3. Portrait fallback did not scan DLC leader directories for fuzzy matches.

   `IconBank` registered DLC leader directories, but fuzzy leader portrait lookup only scanned vanilla `gfx/leaders/<TAG>`. Any portrait using older filename conventions inside DLC directories could fail even though the asset existed.

4. Some user-visible “wrong head of state” reports are semantic, not parser failures.

   HOI4 `country_leader` can represent a government/ruling-party leader rather than a constitutional head of state. Examples found during audit:

   | Tag | Current HOI4 `country_leader` | Historical 1936 head of state expectation |
   | --- | --- | --- |
   | `JAP` | `JAP_keisuke_okada` | Hirohito / `JAP_emperor_hirohito` |
   | `CHI` | `CHI_wang_jingwei` in current vanilla data path | Chiang Kai-shek / `CHI_chiang_kaishek` |
   | `ROM` | `ROM_gheorghe_tatarescu` | Carol II / `ROM_carol_ii` |
   | `AST` | `AST_john_curtin` in current vanilla data path | Joseph Lyons |
   | `RAJ` | `RAJ_freeman_freeman_thomas` | Victor Hope / Linlithgow |

   These are now handled by an explicit project-owned 1936 head-of-state layer where the UI label is meant literally as “国家元首”.

## Changes Made

- Added `CountryHistory::recruited_characters` and parse `recruit_character` from country history.
- Added `select_country_leader_with_recruits` to prefer the first recruited character matching the current top-level ideology.
- Updated `World::refresh_country_leader` to use history recruitment order before falling back to character source order.
- Preserved per-file character source order in `CharacterDef::source_order`.
- Updated portrait fuzzy lookup to scan all registered leader search directories for the tag, including DLC paths.
- Tightened integration coverage for major 1936 leader selection.
- Added `history_1936/politics/head_of_state_1936.ron` for project-owned 1936 head-of-state display overrides. `CHI` is explicitly set to Chiang Kai-shek.

## Recommended Next Step

Continue expanding `crates/hoi4-content/content/history_1936/politics/head_of_state_1936.ron` beyond the first major/Empire batch, especially for minors where vanilla uses generic or government-head portraits.
