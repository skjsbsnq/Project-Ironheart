# 1937 War AI and Performance Repair Notes

Date: 2026-05-26

## Problem Summary

After the Second Sino-Japanese War starts in 1937, the simulation can feel much slower in the GUI, Japan's campaign phase can diverge from the historical sequence, and Japan may fail to launch coastal landings.

The main findings from read-only investigation were:

- Headless release benchmark to 1938-06-19 was about 42 ms/day. The largest cost was still economy, with military and AI each around 10-11%.
- GUI can feel worse than headless because war adds movement, path cache churn, counter rebuilds, arrows, and UI refresh on top of the simulation budget.
- The 1937 situation start sets both `jap_priority_north_china` and `jap_priority_shandong`. `strategy_for` prefers Shandong over North China, so Japan can skip the intended opening North China phase.
- Naval invasion orders are not issued by naval AI directly. They are issued by ground/theater AI and require usable ports, available divisions, convoys, and route naval support.
- Several invasion paths repeatedly call `usable_ports`, `candidate_invasion_divisions`, and route helpers inside loops. This is not the biggest benchmark cost, but it is unnecessary work exactly when war AI wakes up.

## Guardrails

- Do not make performance worse while changing AI behavior.
- Prefer reducing repeated scans before increasing AI frequency.
- Keep history pacing explicit: North China first, Shandong after date/progress/flag.
- Add diagnostics only if they are bounded and not printed every frame/day.

## First Repair Batch

1. Situation pacing:
   - On 1937-07-07, set `china_incident_escalated` and `jap_priority_north_china`.
   - Do not set `jap_priority_shandong` immediately.
   - Let Shandong be activated by existing scripted events or by strategy fallback logic.

2. China theater strategy:
   - Avoid entering Shandong phase before August 1937 unless Japan already controls or threatens relevant North China/Shandong states.

3. Naval invasion performance:
   - Reuse usable port lists when generating invasion plans.
   - Reuse candidate invasion divisions while assigning plan divisions.
   - Avoid rebuilding the same port list per target in naval mission evaluation.

4. Verification:
   - Run content/AI tests that cover Sino-Japanese behavior.
   - Run release headless benchmarks around 600 and/or 900 days and compare ms/day to prior 42 ms/day baseline.

## Follow-up Candidates

- Headless currently runs `ContentRuntimeState`, but app-side `situation_runtime::apply_situation_effects` is GUI-only. Add a runtime-level drain/apply path so headless and GUI share situation effects.
- Surface bounded invasion diagnostics in AI logs: failure reason counts for `NoEmbarkPort`, `NoConvoyCapacity`, and `InsufficientNavalSupport`.
- Add a scenario test that runs through 1937-07-07 and asserts Japan phase is North China before Shandong.
- Continue economy hot-path work, because economy remains the dominant total cost.
