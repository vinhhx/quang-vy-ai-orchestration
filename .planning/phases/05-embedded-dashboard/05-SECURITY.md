---
phase: "5"
slug: "embedded-dashboard"
status: verified
# threats_open = count of OPEN threats at or above workflow.security_block_on severity (the blocking gate)
threats_open: 0
asvs_level: 1
created: "2026-10-19"
---

# Phase 5 — Security

> Per-phase security contract: threat register, accepted risks, and audit trail.
> Register taken verbatim from the `<threat_model>` blocks of 05-01..05-13-PLAN.md (authored at plan time). Evidence (file:line per threat) was verified by gsd-security-auditor on 2026-10-19, with behavioural probes against a sandboxed serve: 303/401/421/403/415/413, path traversal 404, CSP + nosniff + no-referrer, serve.lock 0600 in a 0700 dir.

---

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| browser / other local process → 127.0.0.1:port | untrusted HTTP to a server that can start agents and kill process trees | request text, project ids, aliases (user input); session cookie |
| other web pages in the same browser → dashboard | cross-site requests, DNS rebinding | forged requests (blocked by Host/Origin/SameSite) |
| SQLite and git written by agents → SSE / API → DOM | agent output and plan/QA text are untrusted | log text, plan and QA content (rendered as text only) |
| serve → child `agent-brain run` | the server spawns processes that edit code | argument vectors (no shell) |
| serve lifetime → child lifetime | a stopped or crashed server must not leave agents running | signals (SIGINT forward, PDEATHSIG, kill_tree) |
| crates.io / npm / GitHub releases → developer machine | third-party code and tools fetched once by the user | Cargo.lock, vendored Alpine CSP, Tailwind binary (sha256-pinned) |
| live checks → real accounts and processes | the user (or Claude at the user's request) runs the real CLI | subscription tokens (work profile), process trees on the throwaway repo |

---

## Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation | Status |
|-----------|----------|-----------|----------|-------------|------------|--------|
| T-5-01 | Tampering | agent-brain/Cargo.toml + Cargo.lock (dependency drift) (05-01) | high | mitigate | exact `=` pins for axum and futures-util; user commits the lockfile; reviewer checks no pre-existing version changed; all later builds `--offline --locked` | closed |
| T-5-02 | Tampering | agent-brain/assets/dashboard/vendor/alpine.csp.min.js (05-01) | high | mitigate | sha256 pinned in VENDOR.md and checked with `sha256sum -c` before copying; blocking-human legitimacy check (Task 2); CSP build allows `script-src 'self'` with no eval | closed |
| T-5-03 | Tampering | agent-brain/tools/tailwindcss (dev tool) (05-01) | medium | mitigate | sha256 checked at download and on every run of scripts/build-css.sh; gitignored and never embedded or shipped | closed |
| T-5-04 | Information Disclosure / Elevation | dashboard/mod.rs bind (05-02) | high | mitigate | bind `Ipv4Addr::LOCALHOST` only; grep gate forbids other addresses; plan 05-03's dash02 test reads /proc/net/tcp | closed |
| T-5-05 | Spoofing | dashboard/auth.rs gate (05-02) | high | mitigate | 256-bit per-start token, constant-time compare, HttpOnly SameSite=Strict session cookie; one middleware layered after routes and fallback covers every path (tracer test: SSE without cookie → 401; full matrix: 05-03 dash03) | closed |
| T-5-06 | Spoofing (DNS rebinding) | auth::host_allowed (05-02) | high | mitigate | Host allow-list 127.0.0.1:port / localhost:port, else 421 | closed |
| T-5-07 | Tampering (CSRF) | auth::gate for non-GET (05-02) | high | mitigate | SameSite=Strict + exact Origin check (403) + JSON Content-Type (415) | closed |
| T-5-08 | Information Disclosure | token handling (05-02) | medium | mitigate | token only in the printed URL and the cookie; 303 strips it from the address bar; Referrer-Policy no-referrer; never logged; never in serve.lock (05-03 d14 test) | closed |
| T-5-09 | Tampering (XSS) | CSP header, assets (05-02) | high | mitigate | CSP `script-src 'self'` without eval, `frame-ancestors 'none'`; SSE frames carry derived text, not raw JSON; no inline scripts in index.html | closed |
| T-5-10 | Information Disclosure (path traversal) | assets::serve (05-02) | medium | mitigate | fixed include_bytes! table, exact match, no filesystem access | closed |
| T-5-11 | Information Disclosure | cookie sent to other localhost ports (05-02) | low | accept | RFC 6265 §8.5 limit; session cookie and per-start token bound the exposure; documented in README (plan 05-12) | closed |
| T-5-12 | Denial of Service | SSE connections, request bodies (05-02) | low | mitigate | DefaultBodyLimit 64 KiB; SSE ends on run end or shutdown; 15 s keep-alive; 3 s hard stop on shutdown | closed |
| T-5-13 | Elevation | second serve / stale lock (05-03) | medium | mitigate | flock on serve.lock (auto-released on exit, CLOEXEC so children never hold it); refusal before any DB side effect (d14 test) | closed |
| T-5-14 | Elevation (command injection) | api_tasks POST → jobs.start (05-04) | high | mitigate | argument vector via `Command::args` with `--` before project and request; no shell anywhere (grep gate); slug through routing::slugify | closed |
| T-5-15 | Tampering (wrong repo) | resolve_project (05-04) | high | mitigate | routing::resolve must return exactly one project; 409 with candidates otherwise; zero side effects asserted (DASH-07) | closed |
| T-5-16 | Denial / Elevation (orphaned agents) | jobs.rs shutdown + PDEATHSIG (05-04) | high | mitigate | SIGINT forwarding, grace, kill_tree stragglers; PR_SET_PDEATHSIG from one never-exiting spawner thread; d08 graceful/SIGHUP/crash tests | closed |
| T-5-17 | Elevation (account misuse) | NewTask body (05-04) | high | mitigate | `deny_unknown_fields`; profile only from projects_map.json; d15 test (D-15, SPK-10 account policy) | closed |
| T-5-18 | Tampering (concurrent checkout fight) | busy_reason (05-04) | medium | mitigate | one live run or reservation per project; personal rule E-06; reservation taken under one mutex before the spawn | closed |
| T-5-19 | Denial of Service | request size (05-04) | low | mitigate | 20,000-char request cap and the 64 KiB body limit from 05-02 | closed |
| T-5-20 | Denial / Tampering (wrong process killed) | api_runs kill (05-05) | high | mitigate | only runner::cmd_kill (PID + start-time identity, recorded descendants, S4 method); grep gate forbids other signalling; refusals for non-running and task-less runs | closed |
| T-5-21 | Information Disclosure / Tampering (path or ref injection) | plan/QA reads (05-05) | medium | mitigate | branch taken from tasks.branch in the DB, file names fixed (.plan.md, .qa-report.md), gitops::git argument vectors, no client-supplied path | closed |
| T-5-22 | Information Disclosure | raw log endpoint (05-05) | low | accept | same authenticated single user who owns the runs; raw rows already live in their own brain.db | closed |
| T-5-23 | Elevation (unwanted chaining) | POST dev/qa/relead (05-05) | medium | mitigate | each start requires an explicit request and an enabled action derived from the CLI's preconditions; test asserts no run starts between presses (D-01) | closed |
| T-5-24 | Tampering (SQL injection) | stats queries (05-06) | medium | mitigate | bound parameters with the nullable-filter idiom; profile validated by check_profile; since parsed by parse_since to an integer | closed |
| T-5-25 | Information Disclosure | statistics (05-06) | low | accept | single local user, same data as `agent-brain stats`; behind the token gate | closed |
| T-5-26 | Tampering (lost hand edits / corrupt map) | PUT aliases (05-07) | high | mitigate | etag re-check under a process mutex; only the `aliases` value changes; projects::save_map atomic temp+rename 0600; tests assert other keys unchanged and 409 on conflict | closed |
| T-5-27 | Elevation (profile/account switch) | PUT body (05-07) | high | mitigate | `deny_unknown_fields` with only aliases + etag; grep gate forbids profile writes (D-15) | closed |
| T-5-28 | Tampering (routing confusion) | duplicate aliases (05-07) | medium | mitigate | duplicates returned as CLI warnings; DASH-07 gate in 05-04 still refuses ambiguous names at start time | closed |
| T-5-29 | Elevation (command injection) | scan child (05-07) | medium | mitigate | fixed argument vector `projects scan`, no shell, no user input in the command | closed |
| T-5-30 | Tampering (XSS) | overview screen and app.js (05-08) | high | mitigate | text only through x-text/textContent (server error texts and banners included); the CSP build has no HTML-injection directive and 05-02's CSP forbids inline script; grep gates on the HTML files | closed |
| T-5-31 | Information Disclosure (external loads) | index.html, screens (05-08) | medium | mitigate | no external URL (grep gate), all assets same-origin and embedded, system fonts only (no web font); DASH-09 static test in 05-11 | closed |
| T-5-32 | Spoofing (token theft by script) | cookie (05-08) | medium | mitigate | the token cookie is HttpOnly; app.js never reads or stores it; localStorage only holds 'ab.profile' and 'ab.theme'; the session banner tells the user to reopen the printed URL rather than asking for a token | closed |
| T-5-33 | Elevation (profile override) | assign.js (05-09) | medium | mitigate | POST body is exactly {project, request, name}; server rejects other fields (05-04); the third `<automated>` of Task 1 checks every POST / PUT body in assign.js for a profile key | closed |
| T-5-34 | Tampering (stored XSS via agent output) | task.html / task.js (05-10) | high | mitigate | every untrusted string through x-text or textContent; raw JSON pretty-printed into a `<pre>` via x-text; CSP from 05-02; grep gate | closed |
| T-5-35 | Denial of Service (browser memory) | log pane (05-10) | low | mitigate | at most 5000 rows kept; SSE closed on end and on leaving the screen | closed |
| T-5-36 | Tampering (accidental kill) | Kill button (05-10) | low | mitigate | in-page alertdialog with the consequences spelled out and `Giữ chạy tiếp` focused by default; Escape cancels and the overlay does not close it; the server only kills through cmd_kill (05-05) | closed |
| T-5-37 | Tampering (XSS) | stats/projects screens (05-11) | high | mitigate | x-text only; CSP; grep gates on the HTML files | closed |
| T-5-38 | Information Disclosure (external loads, offline break) | all embedded assets (05-11) | medium | mitigate | dashboard_assets.rs forbids external loads and checks the prebuilt CSS and pinned Alpine (DASH-09) | closed |
| T-5-39 | Elevation (profile change) | projects.js (05-11) | medium | mitigate | no profile control; PUT body only aliases + etag (server also refuses other fields, 05-07); the third `<automated>` of Task 2 checks every POST / PUT body in projects.js for a profile key | closed |
| T-5-40 | Information Disclosure | README / LIVE-CHECKS / protocol log (05-12) | medium | mitigate | placeholder TOKEN only; grep gate for hex tokens; Claude's review checks the diff for secrets (scan-secrets.sh covers spikes/ only) | closed |
| T-5-41 | Information Disclosure | build-release.sh output (05-12) | low | mitigate | the token URL is never printed; output checked for 64-hex strings other than the sha256 line | closed |
| T-5-42 | Denial of Service | smoke server left running (05-12) | low | mitigate | EXIT trap kills the recorded PID; SIGINT then SIGKILL by PID only | closed |
| T-5-43 | Denial of Service (account) | personal-profile run in P4-08 (05-13) | medium | mitigate | optional; only when no other session uses ~/.claude-orca; otherwise recorded as not run (E-06) | closed |
| T-5-44 | Tampering | live kill checks (05-13) | medium | mitigate | stop only via the UI Kill, `agent-brain kill`, Ctrl+C or terminal close; `ps -p` with recorded PIDs, read-only | closed |
| T-5-45 | Information Disclosure | protocol-log/nghiem-thu-p4.md (05-13) | low | mitigate | ids, counts and short notes only; reviewer greps for tokens before committing | closed |
| T-5-46 | Information Disclosure | lock.rs serve.lock, second-serve refusal (05-03) | medium | mitigate | lock file mode 0600 in a 0700 data dir; JSON keys pid, port, started_at only; d14 asserts the token is in neither the second serve's stdout/stderr nor serve.lock | closed |
| T-5-47 | Denial of Service | port selection (`--port`, `dashboard_port`) (05-03) | low | mitigate | a value above 65535 is refused with a Vietnamese error; a taken port exits 1 with a clear message instead of retrying or binding another address; the bind stays `Ipv4Addr::LOCALHOST` (d10 tests, grep gate) | closed |
| T-5-48 | Tampering (XSS) | assign.html / assign.js (05-09) | high | mitigate | project names, aliases, refusal reasons, dirty-file lists, warnings and job output only through x-text; 05-02's CSP forbids inline script; grep gate on assign.html | closed |
| T-5-49 | Tampering (wrong project started) | assign.js submit (05-09) | medium | mitigate | the button stays disabled until exactly one listed, non-missing project is chosen by click or Enter; the arrow keys skip missing projects and a single match is never auto-chosen; typing after a choice clears it; submit sends `selected.id` only, never the typed text; a 409 with candidates becomes a pick list, never a run; the 05-04 resolve gate refuses ambiguity server-side | closed |
| T-5-50 | Tampering (accidental re-plan) | Chạy lại Lead button (05-10) | low | mitigate | the same alertdialog with `Giữ plan hiện tại` focused by default; the server rebuilds the facts and re-checks the CLI preconditions before starting (05-05); the replaced plan stays in git history | closed |
| T-5-51 | Elevation (new action path) | taskstate.rs (R1) (05-10) | low | mitigate | only adds `timed_out` to the open-task relead list; POST /relead still requires the derived action to exist and be enabled after rebuilding the facts (05-05); a unit test pins the new case and the unchanged ones | closed |
| T-5-SC | Tampering | npm/pip/cargo installs (05-01) | high | mitigate | package-legitimacy gate: research audit (axum OK, @alpinejs/csp SUS) + blocking-human Task 2 before Task 3 downloads anything; executor never installs | closed |

*Status: open · closed · open — below high threshold (non-blocking)*
*Severity: critical > high > medium > low — only open threats at or above workflow.security_block_on count toward threats_open*
*Disposition: mitigate (implementation required) · accept (documented risk) · transfer (third-party)*

---

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-5-01 | T-5-11 | RFC 6265 §8.5: cookies are not port-isolated, so another local service on 127.0.0.1 could receive the session cookie; bounded by the per-start random token, session (no Max-Age) cookie and the Host/Origin checks; documented in agent-brain/README.md (Security Notes) | plan 05-02 (user-approved plans) | 2026-10-17 |
| AR-5-02 | T-5-22 | The raw log endpoint serves the same authenticated single user who owns the runs; the raw rows already live in their own brain.db; behind the token gate | plan 05-05 | 2026-10-17 |
| AR-5-03 | T-5-25 | Statistics are the same data as `agent-brain stats` for the single local user; behind the token gate (`stats_requires_cookie`) | plan 05-06 | 2026-10-17 |

*Accepted risks do not resurface in future audit runs.*

**Informational notes from the audit (non-blocking, outside or below the register):**
- `--port` above 65535 is refused by clap's `u16` parser with an English message; only the `dashboard_port` config path has the Vietnamese text (T-5-47 still holds).
- E-06 personal rule across two different personal projects is checked at preflight, so two near-simultaneous starts could both pass (single-user window; per-project reservation is atomic) (T-5-18 still holds).
- `dash09_vendor_alpine_is_pinned_build` pins the vendored Alpine by size/version; the sha256 itself matches VENDOR.md.
- Outside the register: `stats::parse_since` multiplies without `checked_mul` (a huge `since` would wrap in release / 500 in debug; no injection, value is bound); README API catalogue lists four routes that do not exist (docs only).

---

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-10-19 | 52 | 52 | 0 | gsd-security-auditor (via /gsd-secure-phase 5, verify:post hook of /gsd-execute-phase 5) |

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-10-19
