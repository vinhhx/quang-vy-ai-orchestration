---
phase: 05-embedded-dashboard
plan: 01
subsystem: infra
tags: [rust, cargo, axum, tailwind, alpine, vendoring, offline-build]

requires:
  - phase: 03-rust-cli-core
    provides: "agent-brain crate with =-pinned deps, committed Cargo.lock (98 packages), musl route A build"
provides:
  - "Cargo.toml pins axum =0.8.9 (http1, json, query, tokio; no defaults) and futures-util =0.3.34 (alloc); tokio gains rt-multi-thread and net"
  - "Cargo.lock: 98 existing packages unchanged + 22 axum packages; all crates in the local cargo cache, so later plans build --offline --locked"
  - "scripts/build-css.sh: offline Tailwind build and --check freshness gate, sha256-checked tool"
  - "agent-brain/dashboard-src/input.css: Tailwind v4 entry (source(none), class-based dark variant, scans assets/dashboard)"
  - "agent-brain/assets/dashboard/vendor/alpine.csp.min.js: @alpinejs/csp 3.17.4, sha256 0d18d7f8…"
  - "agent-brain/assets/dashboard/app.css: first generated CSS (6896 bytes, Tailwind v4.3.3 MIT banner)"
  - "agent-brain/assets/dashboard/VENDOR.md: provenance, hashes, licenses, regeneration commands"
affects: [05-02, 05-08, 05-09, 05-10, 05-11, 05-12]

actuals:
  tasks: 3
  commits: 2

tech-stack:
  added: [axum 0.8.9, futures-util 0.3.34, "@alpinejs/csp 3.17.4 (vendored)", "Tailwind CSS standalone CLI 4.3.3 (dev-time, gitignored)"]
  patterns:
    - "Every later plan builds with cargo --offline --locked"
    - "Front-end assets are vendored files with recorded sha256; CSS is generated ahead of time and committed"

key-files:
  created:
    - agent-brain/dashboard-src/input.css
    - scripts/build-css.sh
    - agent-brain/assets/dashboard/VENDOR.md
    - agent-brain/assets/dashboard/vendor/alpine.csp.min.js
    - agent-brain/assets/dashboard/app.css
  modified:
    - agent-brain/Cargo.toml
    - agent-brain/Cargo.lock
    - .gitignore

key-decisions:
  - "Task 1 executed by Antigravity (agy 1.4.0); reviewed by Claude"
  - "agy permission mode for Phase 5: --dangerously-skip-permissions, approved by the user on 2026-10-17 for all Phase 5 plan runs (recorded in memory)"
  - "Task 2 (package legitimacy: axum, @alpinejs/csp flagged SUS for recency, Tailwind release hash): user replied \"approved\""
  - "Task 3 network steps were run by Claude at the user's explicit request (\"You run them for me\") after two \"tooling ready\" reports left no trace in this checkout"

patterns-established:
  - "Network and toolchain steps are user checkpoints (or Claude on explicit request), never the executor"

requirements-completed: []

duration: 45min
completed: 2026-10-17
status: complete
---

# Phase 5 Plan 01: Dashboard Crates and Offline CSS Tooling Summary

**axum and futures-util are pinned, fetched and locked (22 new packages, no existing version changed). The Phase 3 suite and the static musl build still pass. Alpine CSP is vendored with a checked hash, and the first `app.css` is generated offline by a hash-checked Tailwind binary. Every later plan can build `--offline --locked`.**

## Performance

- **Duration:** about 45 min including the user checkpoints (agy run 272 s)
- **Completed:** 2026-10-17
- **Tasks:** 3 (Task 1 executor; Tasks 2-3 checkpoints)
- **Files:** 8

## Accomplishments

- **Dependencies:** `Cargo.toml` replaces only the tokio line, which gains `rt-multi-thread` and `net`. It adds `axum =0.8.9` and `futures-util =0.3.34`, both with `default-features = false`. Every other pin, `[dev-dependencies]` and `[profile.release]` is unchanged.
- **Lockfile:** `Cargo.lock` adds exactly the 22 packages the research predicted, from `atomic-waker` to `tower-service`, and changes 0 pre-existing `version` lines.
- **CSS build script:** `scripts/build-css.sh` (mode 755) has three modes:
  - build: writes `app.css`;
  - `--check`: builds to a temp file and runs `cmp`, exiting 1 with the stale-file message when they differ;
  - anything else: usage text, exit 2, before any tool check.

  It refuses to run when the tool is missing or its sha256 doesn't match.
- **Vendoring record:** `VENDOR.md` records both vendored files (path, version, size, sha256, source URL, license) and the regeneration commands.
- **`.gitignore`:** adds `agent-brain/tools/`, and `git check-ignore` confirms the Tailwind binary is ignored.

## Task Commits

1. `fe31e50` chore(05-01): pin dashboard crates and add offline CSS tooling (agy, Task 1)
2. `2aa6fa4` build(05-01): lock dashboard crates, vendor Alpine CSP, first app.css (Claude at the user's request, Task 3)

## Deviations from Plan

- **Task 3 ran by Claude, not the user.** The user asked for this explicitly ("You run them for me") after two "tooling ready" replies. At both, this checkout still had no `build(05-01)` commit, no axum in `Cargo.lock`, and no `agent-brain/tools/`, Alpine file or `app.css`. The same thing happened in Phase 3 (03-01).
- **Step 6 used `npm pack --ignore-scripts`.** The plan's `npm pack` has no install scripts to run, so the result is the same; the flag is a precaution. The tarball's `package.json` declares MIT and has no `install` or `postinstall` script.
- **The first `app.css` holds only the Tailwind base and theme layers (6896 bytes).** `assets/dashboard/` has no HTML yet. Later UI plans regenerate it, and `build-css.sh --check` keeps it honest.

## Review (Claude)

Re-run by the reviewer, not taken from agy's report:

- **Scope:** commit `fe31e50` touches exactly the 5 Task 1 files. It holds neither `Cargo.lock` nor any user-owned file.
- **Task 1 verify commands:** both pass (`manifest ok`; the hash `grep`s give 1 and 2, `.gitignore` 1, `input ok`).
- **Task 1 acceptance:**
  - the Alpine hash appears 2 times in VENDOR.md;
  - `build-css.sh --bogus` exits 2;
  - the missing-tool message is printed with exit 1;
  - the lockfile was untouched by agy;
  - the commit holds no user-owned file;
  - the Python suite gives 124 tests OK.
- **Code read:** `build-css.sh` matches the plan's message strings and ordering: the argument is parsed before the tool and hash checks, and `--check` cleans up its temp file through a `trap`.
- **Task 3 verification:**
  - `grep -c 'name = "axum"' Cargo.lock` gives 1, and the three user-owned files are tracked.
  - The Alpine sha256 is `0d18d7f8d7910e2e0212f0f056b12f50bebc3abb7d88d2f7c7cb4c336fe4519a`.
  - `bash scripts/build-css.sh --check` prints "app.css khớp nguồn" (exit 0), and `app.css` starts with `/*! tailwindcss v4.3.3 | MIT License`.
  - `git check-ignore agent-brain/tools/tailwindcss` prints the path, and the Tailwind binary is 111749248 bytes with sha256 OK.
- **Builds:** `cargo test --manifest-path agent-brain/Cargo.toml --offline --locked -- --test-threads=1` exits 0, with every `test result: ok` and no failures. The musl route A release build finishes in 51 s and produces a `static-pie linked` binary with 0 `NEEDED` entries.
- **Secrets:** a manual pattern scan of the Task 1 diff (keys, tokens, JWT, emails, home path) is clean. `spikes/lib/scan-secrets.sh` only covers `spikes/`, so it doesn't apply.

## Next Phase Readiness

Plan 05-02 (tracer: `serve` → token gate → embedded page → live SSE) can start. axum compiles offline, the Alpine file and `app.css` exist for `include_bytes!`, and agy may run with `--dangerously-skip-permissions`.
