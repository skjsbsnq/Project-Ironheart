# 1937 Shandong Phase Fix Roadmap

## Problem

- The 1937-07-07 Sino-Japanese War opening sets `china_incident_escalated` and `jap_priority_north_china`.
- The AI Shandong phase requires `jap_priority_shandong`, but normal 1937 content never sets that flag.
- Tests currently cover the AI phase by manually adding `jap_priority_shandong`, which masks the missing content trigger.

## Fix 1

- Add a hidden JAP auto event that runs after 1937-08-01 during war with CHI.
- The auto event sets `jap_priority_shandong` without opening a country-event modal.
- Keep the opening-day situation unchanged so Japan starts in the North China phase on 1937-07-07.

## Validation

- Content test verifies the hidden auto event exists and sets `jap_priority_shandong`.
- Existing situation test continues to verify the opening day does not force Shandong.
- Run `cargo test -p hoi4-content --test phase12_sino_japanese_war`.

## Fix 2

- Bound the daily frontline path-advance BFS to local movement instead of scanning all co-belligerent territory.
- Cap collected advance candidates at `MAX_PATH_LEN` so large unified fronts cannot flood the daily tick.
- Add a minimal line-front test proving the path does not jump across a whole friendly corridor in one tick, while still advancing when contact is locally reachable.

## Fix 3

- Use one shared China-invasion staging-port selector for invasion plans and naval AI route targeting.
- Recognize Japanese home/Korea staging states by historical IDs and names, including Kanto, Kansai, Kyushu, Korea and Chosen.
- Do not generate naval invasion plans without a real embark port.
- Add AI coverage proving Japan can generate a China invasion plan from a Kanto home-island port.

## Fix 4

- Add a hidden 1937-08-13 Shanghai campaign auto event.
- The event requires the Shandong phase, then sets `jap_priority_shanghai_nanjing` and fires only the Shanghai news event.
- This gives Japan a historical lower-Yangtze objective instead of waiting until Shanghai is already occupied.

## Fix 5

- Add a hidden 1937-11-15 Nanjing drive auto event.
- The event requires the Shanghai/Nanjing phase, then sets `jap_advance_on_nanjing` and fires only the Nanjing warning news event.
- This provides a historical fallback into the Nanjing stage if province-control triggers lag behind.

## Later Fixes

- Add integration coverage for historical 1937-1938 campaign progression.
