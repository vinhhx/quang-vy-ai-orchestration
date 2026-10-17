# Fix plan: log-row glyphs invisible in the light theme (05-10)

Repo: /mnt/data/IDE/QuangVy-Orchestration, branch `main`.

## Problem

Review of plan 05-10 (headless screenshots of `#/task/1`, light and dark) showed that in the **light** theme the glyph column of the live log pane (▶ ↩ ⓘ …) is almost invisible. The pane is `bg-slate-950` in both themes (UI-SPEC Color: "Log pane | `bg-slate-950` (both themes)"). The glyph `<span>` has no colour class, so it inherits the body text colour: `text-slate-900` in the light theme (dark on dark) and `text-slate-100` in the dark theme (fine).

The row's time span (`text-slate-400`) and text span (`rowTextClass(r)`) set their own colours, so only the glyph is affected. The UI-SPEC "Log pane text colors (on `bg-slate-950`, all >= 4.5:1)" table gives each row kind a colour, and the glyph belongs to its row.

## Files in scope

- `agent-brain/assets/dashboard/screens/task.html`
- `agent-brain/assets/dashboard/app.css` (regenerated only if `bash scripts/build-css.sh --check` says it is stale)

Nothing else. Do not edit `js/task.js` (the existing `rowTextClass(r)` method is reused as is), any other screen, `index.html`, any Rust file, or anything under `.planning/`.

## Steps

1. In `agent-brain/assets/dashboard/screens/task.html`, find the glyph span inside the log row template. It is currently exactly:
   `<span aria-hidden="true" class="w-6 shrink-0 text-center" x-text="glyph(r)"></span>`
   Add the attribute `:class="rowTextClass(r)"` so it becomes exactly:
   `<span aria-hidden="true" class="w-6 shrink-0 text-center" :class="rowTextClass(r)" x-text="glyph(r)"></span>`
   The glyph then takes its row kind's colour on the dark pane (tool `text-slate-200`, assistant `text-slate-100`, tool_result `text-slate-400`, result `text-emerald-300`, error `text-rose-300`, system `text-slate-400`) in both themes. Change nothing else in the file.
2. Run `bash scripts/build-css.sh --check`. If it reports `app.css đã cũ`, run `bash scripts/build-css.sh` and include `agent-brain/assets/dashboard/app.css` in the commit; otherwise leave app.css untouched. (Every class `rowTextClass` returns already appears in task.js, so the CSS is expected to be current.)
3. Commit `fix(05-10): log glyphs use the row colour on the dark pane`, staging the changed file(s) by name. No `git add -A`, no `--no-verify`, no amend.

## Verify (run all from the repo root, report the output)

```bash
grep -cF '<span aria-hidden="true" class="w-6 shrink-0 text-center" :class="rowTextClass(r)" x-text="glyph(r)"></span>' agent-brain/assets/dashboard/screens/task.html   # expect 1
grep -cF '<span aria-hidden="true" class="w-6 shrink-0 text-center" x-text="glyph(r)"></span>' agent-brain/assets/dashboard/screens/task.html                           # expect 0
bash scripts/build-css.sh --check                                                                                                                                      # expect "app.css khớp nguồn"
bash scripts/dashboard-ui-lint.sh                                                                                                                                      # expect "UI lint OK"
cargo build --manifest-path agent-brain/Cargo.toml --offline --locked && bash scripts/dashboard-smoke.sh task                                                          # expect "Smoke OK: task"
git show --stat HEAD                                                                                                                                                   # only task.html (and app.css if regenerated)
```

## Rules

No network and no installs (`cargo ... --offline --locked` only). Never run the real `claude`. Stop processes only by recorded PID. Do not write a SUMMARY; Claude records the result in 05-10-SUMMARY.md.
