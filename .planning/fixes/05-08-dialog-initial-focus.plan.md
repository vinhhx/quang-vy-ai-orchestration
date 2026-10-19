# Fix plan: dialogs never receive their initial focus (05-08 `AB.focusLater`)

Repo: /mnt/data/IDE/QuangVy-Orchestration, branch `main`.

## Problem

05-13 live check (2026-10-19, real Chrome driven over the DevTools protocol): after pressing **Kill** or **Chạy lại Lead** on the task screen, the confirmation dialog opens but `document.activeElement` stays `<main id="screen">`, even 1 s later. The UI-SPEC (Interaction Contracts, "Dialogs") requires focus to move into the dialog on open: to the safe button for confirmations (`Giữ chạy tiếp`, `Giữ plan hiện tại`) and to `Đóng` for the raw-JSON viewer. Because focus stays outside, a keyboard user cannot reach the safe default, and `Escape` does nothing: the `@keydown.escape` handlers sit on the dialog elements, and the dialog never has focus.

Cause: `window.AB.focusLater(id)` in `agent-brain/assets/dashboard/js/app.js` runs `document.getElementById(id).focus()` after `setTimeout(…, 0)`. The dialogs are shown with Alpine `x-show`, which reveals the element in its own deferred step. When `focusLater` fires, the target button is still inside a `display: none` container, so `focus()` is a no-op and is never retried. The 05-08 plan notes record that "`x-show` reveals an element through a `setTimeout`". Callers: `js/task.js` line ~729 `focusLater('ab-dlg-cancel')` and line ~207 `focusLater('ab-raw-close')`.

## Files in scope

- `agent-brain/assets/dashboard/js/app.js`

Nothing else. Do not edit `js/task.js`, any screen HTML, `index.html`, `app.css`, any Rust file, or anything under `.planning/` or `protocol-log/`.

## Steps

1. In `agent-brain/assets/dashboard/js/app.js`, rewrite only the body of `focusLater: function (id) { … }`. It keeps the same name and signature, and callers keep calling it the same way. New behaviour: try to focus the element with that id, and retry until it is visible and focused or about 1 second has passed. Use the existing `setTimeout` style, not `requestAnimationFrame`, for headless parity. Details:
   - Start with a 0 ms delay, then retry every 25 ms for at most 40 attempts.
   - On each attempt, read `var el = document.getElementById(id)`.
   - If `el` exists and `el.offsetParent !== null` (visible), call `el.focus()`. Stop when `document.activeElement === el`.
   - Otherwise schedule the next attempt.
   - After the last attempt, give up silently (no throw, no console output).
   - Write it in the same plain ES5-style JavaScript as the rest of app.js (`var`, `function`). No arrow functions, no `async`, no new globals.
2. Run `node --check agent-brain/assets/dashboard/js/app.js`.
3. Commit `fix(05-08): focusLater waits until the dialog button is visible`, staging only `agent-brain/assets/dashboard/js/app.js` by name. No `git add -A`, no `--no-verify`, no amend.

## Verify (run all from the repo root, report the output)

```bash
node --check agent-brain/assets/dashboard/js/app.js && echo syntax-ok                                  # expect syntax-ok
grep -c 'offsetParent' agent-brain/assets/dashboard/js/app.js                                          # expect >= 1
grep -cE '=>|async ' agent-brain/assets/dashboard/js/app.js                                             # expect 0 (no arrow functions or async added)
{ grep -rnE '(^|[^a-z0-9-])(text-xs|text-3xl|font-medium|font-bold|p-5|px-2\.5|py-0\.5|px-3|py-3|p-3|gap-3|min-h-10|h-3|hover:bg-indigo-500)([^a-z0-9.-]|$)' agent-brain/assets/dashboard/index.html agent-brain/assets/dashboard/screens agent-brain/assets/dashboard/js; test $? -eq 1; } && echo banned-clean   # expect banned-clean
test "$(grep -oF 'uFE0E' agent-brain/assets/dashboard/js/app.js | wc -l)" -ge 8 && echo glyphs-ok       # expect glyphs-ok
bash scripts/build-css.sh --check                                                                        # expect "app.css khớp nguồn"
cargo build --manifest-path agent-brain/Cargo.toml --offline --locked && bash scripts/dashboard-smoke.sh overview assign task stats projects   # expect "Smoke OK: overview assign task stats projects"
git show --stat HEAD                                                                                     # only js/app.js
```

If the `=>|async ` grep prints a non-zero count before your change, report the count before and after instead of failing; the rule is that this fix adds none.

## Rules

No network and no installs (`cargo ... --offline --locked` only). Never run the real `claude`. Do not start, stop or signal the `agent-brain serve` running on 127.0.0.1:7437 or the Chrome window driving it (they belong to the live check); the smoke script uses its own sandbox and port. Stop processes only by recorded PID. Do not write a SUMMARY; Claude records the result.
