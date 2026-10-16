# Fix plan: remove the test-harness bypass from `run lead` (review of 03-10)

Repo: /mnt/data/IDE/QuangVy-Orchestration, branch `main`.

## Problem

Plan 03-10 (`0cd7212`) made `agent-brain run lead` validate the plan with `validate_plan`. Four older tests (written for the 03-04 tracer, when Lead did not validate) run `run lead` with a fake claude that never writes a `.plan.md`, so the skeleton plan fails validation with `no_changes` and the run exits 1 instead of 0.

To keep them green, `agent-brain/src/roles/lead.rs` (lines ~238-255) now drops the `no_changes` error whenever the environment variable `FAKE_CLAUDE_RECORD` is set and the `FAKE_CLAUDE_SCENARIO` file has no `ops`. That is test-only logic in production code: any user or caller who sets `FAKE_CLAUDE_RECORD` gets an empty plan committed. Production code must not read test-harness variables.

Tests that depend on the bypass (verified by removing it):

- `agent-brain/tests/cli_e2e.rs`: `run_lead_bad_json_line_recorded`
- `agent-brain/tests/runner_kill.rs`: `fast_run_drain_breaks_immediately`, `kill_unknown_or_idle_task`, `grandchild_holding_stdout_drain_deadline`

## Files in scope

- `agent-brain/src/roles/lead.rs`
- `agent-brain/tests/cli_e2e.rs`
- `agent-brain/tests/runner_kill.rs`

Nothing else. Do not edit `tests/common/mod.rs`, `tests/support/protocol.rs`, any `src/` file other than `roles/lead.rs`, the Python fakes, `brain/`, or `.planning/`.

## Steps

1. Tests first. Change the four tests so the run they start produces a valid plan, the way `agent-brain/tests/cli_lead.rs` does:
   - Tests that use the default fake claude (`run_lead_bad_json_line_recorded`, `kill_unknown_or_idle_task`): pass a scenario whose `ops` write a valid `.plan.md` with `keep_meta: true` and at least one Thay đổi row naming a file that exists in the test repo (reuse `tests/support/protocol.rs` `lead_scenario` / `plan_text` by including that module the same way `cli_lead.rs` does, or build the same JSON inline). Keep each test's own purpose and assertions (bad JSON line stored raw; kill on unknown/idle task).
   - Tests that use a custom shell fake (`fast_run_drain_breaks_immediately`, `grandchild_holding_stdout_drain_deadline`): make the script write a valid `.plan.md` before it prints the stream and exits. Keep the meta line that agent-brain wrote (line 2 of the existing `.plan.md`, `<!-- brain-meta: ... -->`), e.g. by rewriting only the body after that line. Keep the timing assertions exactly as they are.
   - The tests must still pass with the bypass present (this step changes only tests). Commit: `test(03-10): make tracer-era lead tests write a valid plan`.
2. Remove the whole `if std::env::var("FAKE_CLAUDE_RECORD").is_ok() { ... }` block from `roles/lead.rs` (and make `errors` immutable again). `src/` must contain no reference to `FAKE_CLAUDE` afterwards. Commit: `fix(03-10): drop test-harness bypass from lead validation`.

## Verify (run all, report output)

```bash
grep -rn 'FAKE_CLAUDE' agent-brain/src/ ; test $? -eq 1 && echo "no FAKE_CLAUDE in src"
cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test cli_e2e
cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test runner_kill -- --test-threads=1
cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test cli_lead
cargo test --manifest-path agent-brain/Cargo.toml --offline --locked
cargo fmt --manifest-path agent-brain/Cargo.toml --check
python3 -m unittest discover -s tests -t . -q
```

## Acceptance criteria

- `grep -rn 'FAKE_CLAUDE' agent-brain/src/` prints nothing.
- All cargo test binaries pass, including the four tests above with their original assertions (exit 0, timing bounds, bad-json row, kill messages) and all nine `cli_lead` tests.
- `cargo fmt --check` exits 0; Python suite OK.
- Only the three files in scope changed; two commits in the order above.

## Rules

No network or install commands; every cargo command uses `--offline --locked`; do not edit Cargo.toml/Cargo.lock. No real `claude`. Stage files by name (no `git add -A`), no `--no-verify`, no amend, push, rebase, stash or reset. Only signal processes your own tests spawned; never pkill/killall.
