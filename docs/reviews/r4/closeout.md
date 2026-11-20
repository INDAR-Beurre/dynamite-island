# Round 4 closeout

The configured council gate **passed** for implementation
`33d1b9f5d6f80906d5fa5072562033499d3a2c94`: the median of five surviving
reviews is **9/10**, with **no reported blocker**. All review attempts are
terminal. Ten of fifteen lenses failed through provider/transport errors after
their single retries and remain **unscored coverage gaps**. This is a limited
five-review sign-off, not a complete fifteen-lens review or a claim of perfect
replication. No implementation changes were made during this panel.

| Surviving lens | Score | Verdict |
| --- | ---: | --- |
| Collapsed pill fidelity | 9 | [Raw](verdict-1.json) |
| Expanded panels and launcher | 9 | [Raw](verdict-2.json) |
| Lock preview fidelity and safety | 7 | [Raw](verdict-3.json) |
| Verification integrity | 8 | [Raw](verdict-13.json) |
| Accessibility | 9 | [Raw](verdict-14.json) |

Exact task IDs, retry failures and the computed result are in [tasks.json](tasks.json).
Missing lenses: motion, open/idle performance, memory, theme state, browser
profile setup, packaging, website CSS, interactions, render exactness and process
ownership. Failure is not evidence for or against the product.

## Remaining findings

Three major follow-ups remain; none was classified as a blocker by the delivered
reviews:

1. **Lock clock visible ink.** Same-string `11:28` reference ink is about
   868×279; the vendored Quicksand raster is 821×277. Colon dots also differ.
   The existing advance-width assertion does not test visible ink. Preserve the
   small clock and cap height when correcting the large clock.
2. **Reproducible GTK parsing.** GTK 3 and GTK 4 parsing was checked in separate
   temporary processes, including a bad-color negative control by the final
   critic. A committed rerunnable check is missing from `scripts/theme_selftest.py`.
   This is a reproducibility gap, not a demonstrated generated-CSS error.
3. **Reproducible self+child CPU sampling.** The recorded six-second Control
   Center sample (52.0% → 2.83% of one core) lacks a committed sampler and raw
   transcript. `scripts/measure.py` measures the root process only. The historical
   sample must not be presented as a distribution or re-derived with that tool.

Seven minor groups remain:

- The launcher indicator and Power Lock caption still use fixed blue under an
  alternate selected theme; the reference small-clock ink intentionally stays fixed.
- Power uptime text measured 4.28:1 contrast, below 4.5:1.
- Some comments and archived motion rationale still say EaseOutCubic; production
  uses EaseOutQuad.
- The recorded 363 ms motion budget needs explicit inputs: 233 ms growth + the
  **allowed 100 ms capture-interval ceiling** + 30 ms slack, not the observed
  ~17 ms interval. wf-recorder absolute command-to-presentation metrics
  remain unresolved and must not be invented or counted as passes.
- Mutation-gate documentation advertises drop-all, but the actual cases are
  drop-calendar, drop-lock and fake-3000-ms latency.
- The stress comment overstates semantic coverage. 6,179 assertions include
  about 4,000 rapid-fire transport checks; subtracting those does not prove all
  remaining assertions are semantic. The run contains 1,207 stress cases.
- Archived live reports lack a report-level executable digest/commit stamp.
  The brief distinguishes intermediate tests from final tests; preserve that
  distinction instead of relabeling old reports.

## Evidence and limits

- Final release Rust suite: **336 passed, 1 ignored**; release build succeeded
  with existing warnings. No strict lint-clean claim.
- Theme fixture: two real headless-browser theme backgrounds matched expected
  colors; the red source image stayed red. Profile preservation/removal and
  installer fake-HOME tests passed. No private browser content was inspected.
- Archived merged-theme live stress: **1,207 cases, 6,179 mixed assertions,
  zero failures**; collapsed Pss about 32 MB, retained growth 0 MB, transient
  peak delta 1.3 MB, socket acceptance p95 0.7 ms. Acceptance is not display latency.
- Halo check: **8/8**, no holes. Render liveness: **9/9**. These archived live
  reports precede the final theme-ink/BOM refinement, as recorded in the brief.
- Five earlier trials of the unchanged morph path: **15/17/16/16/15 poses**,
  16.7 ms median pose hold, 216.8 ms relative settle, duplicate ratio .087,
  zero measured dropped frames; all resolved gates passed. Absolute presentation
  latency remains unmeasured by the recorder clock.

Theme changes affect the shell and generated GTK/common HTTP(S) styles. Existing
browser and GTK processes may need restarting to load stylesheet changes.
Browser toolbar themes and media are preserved; custom-rendered content is not
promised universal recoloring. The browser was not restarted automatically.

The lock surface remains a reversible **preview**, not authentication. Its click
exit was tested. No real locker or MangoWM restart was invoked during this panel.
Persistent workspace, tray and Bluetooth additions remain part of the product.

## Council record

- Round 1: 4.5; Round 2: 7; Round 3: superseded/incomplete (nine usable reviews,
  median 7); Round 4: **9 from five usable reviews**, ten provider gaps.
- Closeout decisions: intake 2 steps, merged findings 10 steps, gate 3 steps,
  final report 1 step: **4 recorded chains / 16 steps**. The historical log does
  not contain a reliable complete lifetime thought count; none is invented here.
- No new retry or implementation dispatch follows this passing gate. The three
  major and seven minor follow-up groups remain explicitly open as improvements.
