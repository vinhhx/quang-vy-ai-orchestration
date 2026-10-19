# Fix plan: automated keyboard checks for the dashboard (Phase 5 Nyquist gaps)

Repo: /mnt/data/IDE/QuangVy-Orchestration, branch `main`.

## Problem

`/gsd-validate-phase 5` found two UI behaviours that no automated test covers. Both are UI-SPEC contracts, and the static grep and lint checks cannot see either:

1. **Dialog focus management** (UI-SPEC delta 22, Interaction Contracts "Dialogs"). Opening a confirmation dialog must move focus to its safe button (`#ab-dlg-cancel`). Opening the raw-JSON dialog must move focus to `#ab-raw-close`. `Escape` must close both. This regressed until the 05-13 live check (fixed in `4b32bc4`, `AB.focusLater`), because `scripts/dashboard-smoke.sh` only uses `--dump-dom`.
2. **Combobox keyboard** (UI-SPEC delta 15, D-05). In Giao việc, ArrowDown/ArrowUp move the highlight and skip projects whose directory is missing, and Enter chooses the highlighted project. Only a grep for the key names covers this today.

Add an opt-in `--keys` mode to the existing smoke script. It drives the sandboxed dashboard in headless Chrome over the DevTools protocol, with real key events, and checks both behaviours. It uses the fake claude only: no real claude, no network.

## Files in scope

- `scripts/dashboard-smoke.sh` (extend)
- `scripts/dashboard-keys.mjs` (new)

Nothing else. Do not edit anything under `agent-brain/` (no product change, this is test tooling), `.planning/`, or `protocol-log/`.

## Steps

1. **`scripts/dashboard-smoke.sh`**
   - **Option parsing:** accept an optional `--keys` argument anywhere in the argument list. Remove it from the screen list. An empty remaining list still means the default `overview assign`.
   - **Second, missing project:** in the sandbox `projects_map.json`, add a second project after `web-ban-hang`, with id `web-ban-hang-cu`, path `$TMP/repos/khong-ton-tai` (never created), name_vi `Web bán hàng cũ`, aliases `["shop-cu"]`, type `code`, repo `null`, profile `work`, missing `true`. This exercises the "missing projects are skipped" rule. The existing `run lead -- web-ban-hang …` step and the five screen checks must still pass unchanged.
   - **After the screen loop, when `--keys` was given:**
     - Require `node` with a global `WebSocket` (`node -e 'process.exit(typeof WebSocket==="function"?0:1)'`), else print `Cần Node ≥ 22 (có WebSocket) cho --keys` and exit 1.
     - Start `"$CHROME_BIN" --headless=new --no-sandbox --disable-gpu --no-first-run --disable-background-networking --disable-component-update --disable-sync --remote-debugging-port=0 --user-data-dir="$TMP/chrome-keys" --window-size=1280,1000 about:blank` in the background, and record its PID in `CHROME_PID`.
     - Wait up to 10 s for `$TMP/chrome-keys/DevToolsActivePort`, and read the port from its first line.
     - Run `node "$REPO_ROOT/scripts/dashboard-keys.mjs" "$PORT" "$URL"`, where `$URL` is the printed dashboard URL with its token.
     - Propagate a non-zero exit as failure, printing the node output and the serve stderr.
   - **Cleanup:** the `cleanup` trap must also stop `CHROME_PID` if set, by that PID only: `kill -TERM`, wait up to 5 s, then `kill -KILL` if still alive. Never `pkill`, `killall` or kill by name.
   - **Success line:** keep it `Smoke OK: <screens>`, and print `Keys OK: combobox, confirm-dialog, raw-dialog` after it when `--keys` ran.
2. **`scripts/dashboard-keys.mjs`**
   - **Basics:** plain Node ESM, no dependencies, using the global `fetch` and `WebSocket`. Arguments: CDP port and dashboard URL (with `?token=`).
   - **Connect:** open a page with `GET http://127.0.0.1:<port>/json/new?<url>` (method `PUT` on current Chrome) or reuse the first page target, and connect to its `webSocketDebuggerUrl`.
   - **CDP helpers:**
     - `Runtime.evaluate` with `awaitPromise` and `returnByValue`;
     - `Input.dispatchKeyEvent` keyDown/keyUp with `key`, `code` and `windowsVirtualKeyCode`;
     - `Input.insertText`;
     - `waitFor(expr, ms)`, polling every 100 ms.

     Every wait has a timeout and fails with a Vietnamese message naming the check.
   - **Check A, combobox:**
     1. Navigate to `<url>#/assign` and wait for `[data-screen="assign"][data-loaded="yes"]`.
     2. Focus `#ab-project`, insert the text `web`, and wait until the listbox `#ab-suggest` has options for both `(web-ban-hang)` and `(web-ban-hang-cu)`. This waits out the 200 ms debounce rather than acting on the stale list. Assert that the `web-ban-hang-cu` option has `aria-disabled="true"`.
     3. Press ArrowDown 3 times. After each press, read `#ab-project`'s `aria-activedescendant`, resolve that element, and assert it exists and is not `aria-disabled="true"`.
     4. Press Enter. Assert `#ab-project`'s value contains `(web-ban-hang)` and not `(web-ban-hang-cu)`.
   - **Check B, confirmation dialog:**
     1. Record `runs.length` from `GET /api/tasks/1` (via `fetch` in the page).
     2. Navigate to `#/task/1` and wait for the screen to load. The smoke's task 1 is planned, so it offers `Chạy lại Lead`.
     3. Click the visible enabled button whose text is `Chạy lại Lead`.
     4. Within 1.5 s, `document.activeElement.id` must be `ab-dlg-cancel`.
     5. Press Escape. Within 1 s, no `[role=alertdialog]` may be visible.
     6. `runs.length` must be unchanged, so no run started.
   - **Check C, raw-JSON dialog:**
     1. On the same screen, click the first visible `#ab-log button` (a log row).
     2. Within 1.5 s, `document.activeElement.id` must be `ab-raw-close`.
     3. Press Escape. Within 1 s, no `[role=dialog]` may be visible.
   - **Result:** print one `OK <check>` line per check. On the first failure, print `LỖI <check>: <detail>` and exit 1. Close the WebSocket at the end.
3. Commit `test(05): headless keyboard checks for combobox and dialogs (smoke --keys)`, staging the two files by name. No `git add -A`, no `--no-verify`, no amend.

## Verify (run all from the repo root, report the output)

```bash
bash -n scripts/dashboard-smoke.sh && node --check scripts/dashboard-keys.mjs && echo syntax-ok         # expect syntax-ok
cargo build --manifest-path agent-brain/Cargo.toml --offline --locked && bash scripts/dashboard-smoke.sh overview assign task stats projects   # expect "Smoke OK: overview assign task stats projects" (unchanged behaviour)
bash scripts/dashboard-smoke.sh --keys overview                                                           # expect "Smoke OK: overview" then "Keys OK: combobox, confirm-dialog, raw-dialog"
bash scripts/dashboard-smoke.sh --keys overview                                                           # second run, to check it is not flaky
grep -cE 'pkill|killall' scripts/dashboard-smoke.sh scripts/dashboard-keys.mjs                           # expect :0 for both
ps -eo pid,args | grep -E 'chrome.*chrome-keys|agent-brain serve' | grep -v grep || echo clean            # expect clean (nothing left after the runs)
git show --stat HEAD                                                                                       # only the two scripts
```

## Rules

No network and no installs (`cargo ... --offline --locked` only). Never run the real `claude`. Do not touch any `agent-brain serve` or Chrome that this plan did not start; the smoke script uses its own sandbox, port and Chrome profile. Stop processes only by recorded PID. Do not write a SUMMARY; Claude records the result in 05-VALIDATION.md.
