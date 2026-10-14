# Phase 3: Rust CLI Core - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-10-14
**Phase:** 3-Rust CLI Core
**Areas discussed:** Build approach & handover, Data home & task identity, Live output & kill behavior, Fast-path & stats output

---

## Build approach & handover

| Option | Description | Selected |
|--------|-------------|----------|
| GSD execute-phase | Plans + executors build modules with tests and atomic commits | ✓ |
| Dogfood via brain | Each module as a brain lead/dev/qa task (spec §8) | |
| Hybrid | GSD for runner/stream/db, brain for small modules | |

| Option | Description | Selected |
|--------|-------------|----------|
| Retire after parity | Keep until parity + one real task, then drop PATH symlink, keep code as reference | ✓ |
| Keep both permanently | Python brain as fallback; protocol changes made twice | |
| Delete immediately | Remove brain/ at end of Phase 3 | |

| Option | Description | Selected |
|--------|-------------|----------|
| Shared fixture cases | Language-neutral JSON cases read by both Python and Rust tests | ✓ |
| Port tests by hand | Rewrite P1-xx tests in Rust | |
| Manual side-by-side | User compares outputs | |

| Option | Description | Selected |
|--------|-------------|----------|
| Static musl, Linux x86_64 | Fully static, SQLite bundled | ✓ |
| Normal glibc build | Simpler, may fail on older glibc | |
| Static + aarch64 too | Adds ARM cross-compile | |

**User's choice:** All recommended options.

---

## Data home & task identity

| Option | Description | Selected |
|--------|-------------|----------|
| JSON is truth, SQLite mirrors | Hand-edit projects_map.json; synced into SQLite each run | ✓ |
| SQLite replaces JSON | One-time import, edit via commands | |
| JSON only, no projects table | project_id as text | |

| Option | Description | Selected |
|--------|-------------|----------|
| ~/.local/share/agent-brain/ | XDG data dir, one DB for all projects, env override | ✓ |
| Next to projects_map.json | All in ~/.config/agent-brain/ | |
| Per project | .agent-brain.db per repo | |

| Option | Description | Selected |
|--------|-------------|----------|
| Task = task/<slug> branch | lead creates; dev/qa attach to checked-out task branch | ✓ |
| Explicit --task flag | dev/qa require --task <id> | |
| Latest open task per project | Attach to most recent unfinished task | |

| Option | Description | Selected |
|--------|-------------|----------|
| Short integer | SQLite row id printed at run start | ✓ |
| Slug | Readable, may collide across projects | |
| Both accepted | Number or slug, ambiguous slug refused | |

**User's choice:** All recommended options.

---

## Live output & kill behavior

| Option | Description | Selected |
|--------|-------------|----------|
| One compact line per event | Plus final summary; --verbose for full | ✓ |
| Full detail by default | Everything printed | |
| Quiet + summary | Spinner + summary only | |

| Option | Description | Selected |
|--------|-------------|----------|
| Kill whole group at once | SIGTERM→grace→SIGKILL; second Ctrl+C = SIGKILL now; status killed | ✓ |
| Ask to confirm | "Kill the agent? [y/N]" | |
| Detach, keep running | Ctrl+C stops output only | |

| Option | Description | Selected |
|--------|-------------|----------|
| 3 seconds, configurable | kill_grace_secs, margin within P2-02's 5 s | ✓ |
| 1 second | Faster, MCP may not shut down cleanly | |
| You decide | From S4 spike result | |

| Option | Description | Selected |
|--------|-------------|----------|
| Reconcile on next start | Dead process group → lost; never signal reused PID | ✓ |
| Leave it, manual cleanup | Stale running rows remain | |
| You decide | Any safe approach | |

**User's choice:** All recommended options.

---

## Fast-path & stats output

| Option | Description | Selected |
|--------|-------------|----------|
| Branch + status + recent log + task | git state + diff --stat vs task base + last run from SQLite | ✓ |
| Exactly the spec trio | git status/log/diff --stat only | |
| All projects overview | No-arg overview (new capability) | |

| Option | Description | Selected |
|--------|-------------|----------|
| Short review, Sonnet | Fixed prompt, printed, recorded as role=inspect, truncated large diffs | ✓ |
| Free question | inspect with user question | |
| Summary only, Haiku | Cheapest | |

| Option | Description | Selected |
|--------|-------------|----------|
| Totals + per project & role | --since window, cost_usd labelled equivalent | ✓ |
| Totals only | One block | |
| Per task detail | Overlaps Phase 4 OPT reports | |

| Option | Description | Selected |
|--------|-------------|----------|
| Delete the session file after | Prefer no-persist flag; else delete by session_id | ✓ |
| Accept a saved session | Relax P2-07 | |
| You decide | Any approach with a test | |

**User's choice:** All recommended options.

---

## Claude's Discretion

- Module/crate naming, config file format, DB path override env var name.
- SQLite schema details beyond spec §4.2, migrations.
- Stream-json event classification and one-line rendering.
- How the 3 Phase 2 prompts are bundled (include_str! vs files).
- Exit codes (Fallback Ask stays non-zero).

## Deferred Ideas

- `status` without a project → overview of all projects (later phase, e.g. dashboard).
- aarch64 / macOS / Windows builds.
- FAST-03 warm reuse `--resume` (v2, decide in /gsd-discuss-phase 4).
