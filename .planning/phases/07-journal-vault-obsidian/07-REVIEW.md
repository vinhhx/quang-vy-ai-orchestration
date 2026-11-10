---
phase: 07-journal-vault-obsidian
reviewed: 2026-11-10T00:00:00Z
depth: standard
files_reviewed: 33
files_reviewed_list:
  - agent-brain/src/journal/mod.rs
  - agent-brain/src/journal/collect.rs
  - agent-brain/src/journal/config.rs
  - agent-brain/src/journal/digest.rs
  - agent-brain/src/journal/fts.rs
  - agent-brain/src/journal/gitsync.rs
  - agent-brain/src/journal/loop_.rs
  - agent-brain/src/journal/model.rs
  - agent-brain/src/journal/render.rs
  - agent-brain/src/journal/sanitize.rs
  - agent-brain/src/journal/state.rs
  - agent-brain/src/journal/writer.rs
  - agent-brain/src/telegram/journal_cmd.rs
  - agent-brain/src/telegram/detect.rs
  - agent-brain/src/telegram/intake.rs
  - agent-brain/src/telegram/bot.rs
  - agent-brain/src/telegram/mod.rs
  - agent-brain/src/dashboard/mod.rs
  - agent-brain/src/db.rs
  - agent-brain/src/config.rs
  - agent-brain/src/main.rs
  - agent-brain/src/lib.rs
  - scripts/phase7-live-check.sh
  - agent-brain/tests/journal_export.rs
  - agent-brain/tests/journal_sanitize.rs
  - agent-brain/tests/journal_serve.rs
  - agent-brain/tests/journal_tim.rs
  - agent-brain/tests/journal_gitsync.rs
  - agent-brain/tests/journal_render.rs
  - agent-brain/tests/support/journal.rs
  - agent-brain/tests/fixtures/journal/golden/Today.md
  - agent-brain/assets/config.default.json
  - agent-brain/README.md
findings:
  critical: 0
  warning: 13
  info: 9
  total: 22
status: issues_found
---

# Phase 7: Code Review Report

**Reviewed:** 2026-11-10
**Depth:** standard (plus cross-file tracing of the export, git, mail and `/tim` paths)
**Files Reviewed:** 33
**Status:** issues_found

## Summary

Scope was the Phase 7 journal code and its tests. The "Merge & push" feature in the same diff range (workspace.rs, gitops.rs, task.js, merge_push in bot.rs/callback.rs) was only skimmed and is not covered here.

The core design holds up under adversarial reading:
- The secret gate runs after `render_all` and before `gitsync::prepare` and `writer::apply` (`journal/mod.rs:145-174`), so nothing is cloned, written or committed when it fires.
- Every string from brain.db reaches `render` as `SafeText`. Project slugs are the one exception; they are restricted by `slug_ok` and covered by the gate.
- Git is invoked without a shell, with `--`, with a validated branch and URL, and without force, `-A` or history rewrite.
- `/tim` is behind `authz`. The FTS5 query is built from quoted, folded tokens, so no operator or column filter survives.
- Migration 12 runs in one `IMMEDIATE` transaction, and the foreign-key comparison rolls back on a new violation.

No critical (BLOCKER) issue was found. I did not find a path that leaks an allowlisted-out field into the vault. The warnings below are real behavioural, safety and robustness defects. The first three can silently empty or wedge the vault, or lose alerts.

## Warnings

### WR-01: A clock jump or empty/wrong DB makes the exporter delete and push the whole generated vault

**File:** `agent-brain/src/journal/collect.rs:294-300`, `agent-brain/src/journal/writer.rs:177-188`, `agent-brain/src/journal/mod.rs:174-192`
**Issue:** `collect` drops every run whose local day is after `today` (`if d <= today`). `writer::apply` then removes every marker-bearing file under `Nhật ký/` and `Dự án/` that is not in the new file set, and `commit_and_push` stages the deletions and pushes them. There is no sanity guard on the size of the deletion.
- If `now_ms` is far in the past (RTC reset or NTP not yet synced at boot, which is exactly when an auto-started `serve` runs its first tick), the model is empty. Only `Today.md` is rendered; all day, month, year and project pages are deleted, the FTS index is emptied, and a deletion commit is pushed.
- The same thing happens when the CLI or serve is pointed at a fresh or wrong `brain.db` (`AGENT_BRAIN_DB`, a restored empty DB).
- It is self-healing on the next correct export, and git history keeps the files. But the phone vault is wiped and the remote gets churn commits.
**Fix:** Before `writer::apply`, compare the planned removals with the existing managed files. For example, refuse (`ExportOutcome::Stopped(GitStop::...)` plus a mail) when the model has no days but the clone has more than N marked files, or when removals exceed a configurable fraction. Also refuse to export when `now_ms` is earlier than `max(runs.started_at)` of the DB, since a clock behind the data is never valid.

### WR-02: A project slug containing `..` (or a trailing `.`) makes every export fail forever, silently, and re-run every minute

**File:** `agent-brain/src/journal/collect.rs:60-68`, `agent-brain/src/journal/writer.rs:32-42`, `agent-brain/src/journal/mod.rs:174,211-225`, `agent-brain/src/journal/loop_.rs:33-44`
**Issue:**
- `slug_ok` allows `.` anywhere after the first character. `a..b`, or `foo.` (which produces `Dự án/foo..md`), is accepted. `check_rel` then rejects any path containing `..` and `apply` returns `Err` for the whole export before writing anything.
- `export_with_tz` propagates the error with `?`, so `run_export_once` returns early and `state::record_outcome` is never called. There is no `journal_state` update, no `journal_mail` alert, and no `last_export_ms`.
- Because `last_export_ms` never advances, `export_due` is true on every 60 s tick. The loop re-collects, re-renders and re-scans the whole vault each minute, with only a terminal line as symptom.
- Project ids are not constrained by `projects.rs` (no slug pattern found), so this is reachable.
**Fix:** Make `slug_ok` reject `..` and a trailing `.` (or fall back to the `du-an-{id}` key). In `run_export_once`, record `Err` results as a failure (state plus a once-per-cause mail) and advance `last_export_ms` so the loop backs off to `interval_minutes`.

### WR-03: Alert dedupe keys are permanent, so a recurring conflict, dirty clone or secret block is never reported a second time

**File:** `agent-brain/src/journal/state.rs:143-171, 201-213, 216-229` (with `enqueue_mail`, 48-60)
**Issue:** `journal_mail.dedupe_key` is `UNIQUE` forever. The keys are `conflict:{fnv(detail)}`, `dirty:{fnv(paths)}`, `secret:{fnv(lines)}` and `foreign:{fnv(files)}`, with no time or streak component. After the user fixes a conflict or dirty clone and the same files conflict or go dirty again (very likely: the same `Today.md` or day file), `INSERT OR IGNORE` drops the alert. `KEY_BLOCKED` is set but nothing is sent. Meanwhile `Stopped` and `SecretBlocked` re-run every 30 min and block the vault indefinitely. Only `pushfail:{since}`, `recovered:{since}` and `notrepo:{day}` have a recurrence component.
**Fix:** Include the start of the blocked episode in the key (for example `KEY_BLOCKED` set time, cleared in `on_success`): `conflict:{blocked_since}:{hash}`. Alternatively, delete the previous mail row of that kind when `on_success` clears `KEY_BLOCKED`.

### WR-04: Git subprocesses inherit `GIT_DIR` and friends, and pathspecs are not literal

**File:** `agent-brain/src/journal/gitsync.rs:60-83, 388-397, 670-693`
**Issue:**
- `command()` sets `current_dir` but never removes `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_COMMON_DIR`, `GIT_OBJECT_DIRECTORY` or `GIT_NAMESPACE` from the inherited environment. If `serve` or the CLI is started from a hook, an agent worktree or any shell that exports them, `checkout HEAD -- <paths>`, `add`, `rm --cached`, `commit` and `push` act on that other repository (`gitops.rs` itself sets `GIT_DIR` for its shadow repos, so the project does use this variable).
- `add`, `rm --cached` and `checkout` take `--pathspec-from-file` without `GIT_LITERAL_PATHSPECS=1`, so a file name containing `*`, `?`, `[` or a leading `:` is treated as a glob or magic pathspec. `rm --cached --ignore-unmatch` is built from names found on disk (`removed`), which can be user-created marked files.
**Fix:** In `command()` add `env_remove` for the `GIT_*` repository variables above and `.env("GIT_LITERAL_PATHSPECS", "1")`. Add a test that runs an export with `GIT_DIR` pointing at another repo.

### WR-05: Hostname redaction uses a short TLD allow-list, so other internal hosts survive into titles

**File:** `agent-brain/src/journal/sanitize.rs:115-119`
**Issue:** `RE_HOST` only matches `com|net|org|vn|io|dev|app|local|internal|lan|corp|test|cloud|xyz|info|biz`. Hosts such as `git.example.co`, `jira.example.ai`, `wiki.example.me`, `x.example.edu.vn` (matched only through `vn`), `intranet.example.tech`, `.site`, `.online`, `.uk`, `.de` and `.jp` pass through unchanged. D-03 lists endpoint and host as never exported, and D-05 passes the user's text verbatim, so this is the main leak route for the sanitizer. The gate does not backstop it (GATE_RULES has no host rule).
**Fix:** Redact any `label.label` token whose last label is alphabetic and at least two characters and that is not obviously a filename or number, or at least widen the TLD list substantially. Add a gate rule for `://` and for dotted hosts of the same shape as a last line of defence.

### WR-06: `RE_REL_PATH` erases ordinary Vietnamese text such as dates and "và/hoặc", and `/tim` then cannot find it

**File:** `agent-brain/src/journal/sanitize.rs:123`
**Issue:** `\S*[^\s/]/+[^\s/]\S*` matches any word with an inner `/`. "ngày 15/01", "24/7", "50/50", "CI/CD", "I/O", "và/hoặc" and "A/B" all become `(đường dẫn)` in the title, the callout body, the digest, and the FTS index. The journal is meant to read as "what did I do today"; the loss is silent, and searching for the date or term returns nothing.
**Fix:** Only treat a slash token as a path when it has two or more separators, starts with `.`, `~` or a drive, contains a dot extension, or a known path root. Keep the single-slash fast path for date-like (`\d{1,2}/\d{1,2}(/\d{2,4})?`) and short `a/b` alphabetic pairs.

### WR-07: Carry-over "Cần anh xử lý" never expires and ignores the task's own terminal state

**File:** `agent-brain/src/journal/collect.rs:127-157, 347-361`
**Issue:** `classify_as_of` checks the last run's `outcome`/`status` for `quota`, `env_down`, `protocol_violation`, `qa:fix`, `failed`, `timed_out` and so on before it looks at `task_status`. The carry-over loop adds every task whose last run is in an Attention state to today's list, regardless of age.
- A task abandoned or closed by hand (its last run failed or ended `qa:fix` and nothing ran afterwards) appears in "Cần anh xử lý" on `Today.md` every day indefinitely, from any past year.
- The digest then shows the first 10 stale items and "… và N mục khác".
- On a DB with thousands of historical tasks this makes the section noise and defeats D-16 ("chưa xử lý").
**Fix:** Define resolution (task status marks it closed or cancelled, or a later approval or no-op) and an age cap (`journal.attention_max_days`, for example 14). Also add a test with a task whose last run failed 60 days ago.

### WR-08: `/tim` indexes and returns more title text than the vault shows (D-15), and the test only covers the no-proposal case

**File:** `agent-brain/src/journal/fts.rs:36-37`, `agent-brain/src/journal/render.rs:57-61`, `agent-brain/tests/journal_tim.rs:114-166, 201-208`
**Issue:** `FtsRow.display` and `.title` use the full sanitized `t.title` (up to `title_max_chars`, 140 by default, 400 max). The vault callout header and the project page show only `t.title.cut(callout_title_chars)` (60 by default). When a proposal title exists, its characters 61-140 appear nowhere in the vault (the body shows the request, which is a different text), yet `/tim` matches on them and prints them. The test `tim_never_returns_text_outside_the_vault` only checks a request-derived title (cut at 140) and a plan file; it cannot detect this.
**Fix:** Build FTS `display` and `title` from `t.title.cut(cfg.callout_title_chars)` (the same cut the vault shows), or index the request text only when it is rendered. Add a proposal-title test whose distinguishing word lies between characters 61 and 140.

### WR-09: Mail rows are claimed before the scan finishes, so a later error or a full outbox loses them

**File:** `agent-brain/src/telegram/detect.rs:541-566, 671-690`; `agent-brain/src/telegram/outbox.rs:29,120`
**Issue:** `JournalMail::scan` sets `sent_at` for each row in turn (autocommit) and builds `out`. If the next `UPDATE` or the digest drop path returns `Err` (for example `SQLITE_BUSY` after the 5 s timeout while the exporter holds a write transaction), `?` returns and the whole `Vec<Outgoing>` is discarded. The already-claimed earlier rows are marked sent but never queued. The caller then treats the result as `Err` and `run` pushes nothing from this scan. Separately, `Outbox::push` drops items beyond `QUEUE_CAP` (200) while Telegram is unreachable, so a claimed digest or push-failure alert is lost silently. The comment (A11) accepts a crash window, but not these two non-crash paths.
**Fix:** Claim and enqueue per row. On `Err` return the items collected so far (or un-claim them with `UPDATE ... SET sent_at = NULL WHERE id = ?` on failure). Mark `sent_at` from the sender thread after a successful `sendMessage` rather than at scan time, or at least log which ids were claimed but not queued.

### WR-10: The clone's current branch is never verified against `journal.branch` before commit and push

**File:** `agent-brain/src/journal/gitsync.rs:485-516, 547-566, 640-650`
**Issue:** `push_head` pushes `HEAD:refs/heads/{branch}` and `integrate` compares `HEAD` with `origin/{branch}`, but nothing checks that `HEAD` is the local branch `branch`.
- If the clone was left on another branch (for example when the first `prepare` timed out after a successful `clone --no-checkout` but before `checkout -B`, leaving `HEAD` on the remote's default `master`; or the user changed `journal.branch`; or someone checked out another branch), the next export rebases that other branch's commits onto `origin/<branch>` and pushes them to `<branch>`. This contradicts D-22 ("không đụng master", only `main`).
- `sync_existing` also never repairs an unborn or detached `HEAD`.
**Fix:** In `sync_existing`, read `git symbolic-ref -q HEAD`; if it is not `refs/heads/<branch>`, either stop with a distinct `GitStop` and a mail, or `checkout -B <branch> origin/<branch>` when the tree is clean.

### WR-11: `writer::apply` writes through a symlinked managed directory

**File:** `agent-brain/src/journal/writer.rs:51-67, 153-175`
**Issue:** `managed_files_on_disk` refuses a symlinked root for deletion, but writes do not. `write_atomic` calls `create_dir_all(dir)` and writes the temp file beside the target. If `Nhật ký`, `Nhật ký/2026` or `Dự án` (or a sub-directory) is a symlink, for example one committed in the remote vault, the generated Markdown (and the `.…journal-tmp` file) is created outside the clone. The test `symlink_in_managed_root_not_followed` only checks removal. A tracked symlink is only reachable by someone who can push to the vault, but the exporter is meant to be safe to run against a repository others edit.
**Fix:** Before writing, walk each existing ancestor of `target` under `clone_dir` with `symlink_metadata` and refuse (or report as `foreign`) if any component is a symlink. Open the temp file with `O_NOFOLLOW`/`create_new`.

### WR-12: A relative `journal.clone_dir` resolves against the process cwd, so the CLI and `serve` use different clones

**File:** `agent-brain/src/journal/config.rs:294-301`
**Issue:** `clone_dir` only needs to be non-empty and free of control characters, then `expand_tilde`. A relative value (`journal`) becomes `<cwd>/journal`. `agent-brain journal export` run from a shell and the `serve` loop (started elsewhere) then create two independent clones of the same remote, each with its own dirty and rebase state. This also removes the "NotARepo" protection when the cwd happens to be a non-empty directory.
**Fix:** Require an absolute path after tilde expansion (`PathBuf::is_absolute`), with a message naming `journal.clone_dir`.

### WR-13: Test gaps leave the production paths of this phase unexercised

**File:** `agent-brain/tests/support/journal.rs:30-34`, `agent-brain/tests/journal_gitsync.rs`, `agent-brain/tests/journal_serve.rs`
**Issue:**
- Every journal test sets `utc_offset_minutes: 420`, so `JournalTz::Local` (the production default, `chrono::Local`, D-12) is never run end to end. A host in UTC would place the day boundary at 07:00 local; no test or config guard catches that.
- No test covers: an interrupted rebase (`.git/rebase-merge`), a stale `index.lock`, a diverged clone with `HEAD` on another branch, a persistent export `Err` (WR-02), a deletion guard (WR-01), recurrence of a `conflict` alert (WR-03), or a crash between claim and send.
- The serve-loop digest tests pin the clock with `NOW_ENV` for the whole run, so D-20's "retry later in the day after a failed push, drop after 24:00" is only covered by the pure `export_due` unit test, not by the loop.
**Fix:** Add a `JournalTz::Local` integration test (set `TZ=Asia/Ho_Chi_Minh` for the child process) and the missing git and mail scenarios above.

## Info

### IN-01: `project_file_key` fallback key can collide with another project's slug

**File:** `agent-brain/src/journal/collect.rs:72-93`
**Issue:** `du-an-{row_id}` is inserted into `taken` but never checked against it. A project whose real slug is `du-an-9` (processed earlier at a lower id) plus another project with id 9 and an invalid slug produce the same key; two projects share one page and the group of whichever was inserted last in `project_refs` is used.
**Fix:** Loop `du-an-{id}`, `du-an-{id}-2`, ... until the key is not in `taken`.

### IN-02: `Today.md` labels the previous non-empty day as "Hôm qua"

**File:** `agent-brain/src/journal/render.rs:332-334`
**Issue:** `day_files.iter().rev().find(|d| **d < today)` returns the latest earlier day with entries, which can be days or weeks ago, but the label is "Hôm qua" (yesterday).
**Fix:** Label it "Trước đó" (or "Hôm qua" only when `prev == today - 1`).

### IN-03: The CLI and the serve loop collect different known-secret sets

**File:** `agent-brain/src/journal/mod.rs:228-247` versus `agent-brain/src/inputs.rs:75-101`, `agent-brain/src/journal/loop_.rs:95`
**Issue:** `known_secrets` (used by `journal export`) omits the dashboard token (`st.token`) that `inputs::collect_secrets` (used by the loop) includes. A manual export is therefore weaker than the background one. It also reads the Telegram token from `telegram::config::path()` rather than next to the map file.
**Fix:** Have `cmd_export` reuse one shared collector (the dashboard token is available from the same data dir) so both paths scan the same set.

### IN-04: Foreign-key tolerance compares counts, not violation identity

**File:** `agent-brain/src/db.rs:238-260`
**Issue:** `after > before` passes when a migration removes one violation and introduces another (equal count), and when pre-existing orphans are removed while new ones appear. The comparison is acceptable for D-21 but is weaker than the old guard for the `runs` table rebuild.
**Fix:** Compare the set of `(table, rowid, parent, fkid)` rows from `pragma_foreign_key_check` before and after, and fail on any row not in the "before" set.

### IN-05: A vault `.gitignore` that ignores a managed path makes every push attempt fail with a generic error

**File:** `agent-brain/src/journal/gitsync.rs:670-679`
**Issue:** `git add --pathspec-from-file` exits non-zero for a path matched by the user's `.gitignore`. The code returns `local_failure()` (category `Other`), so the export counts as a failed push every cycle and eventually mails "chưa đẩy được" with a misleading cause.
**Fix:** Detect the ignored-path error (or run `git check-ignore -z --stdin` first) and stop with a `GitStop` that names the ignored files.

### IN-06: `sync_existing` aborts any rebase in progress, including one the user is resolving by hand

**File:** `agent-brain/src/journal/gitsync.rs:491-495`
**Issue:** If `.git/rebase-merge` or `rebase-apply` exists, `rebase --abort` runs unconditionally. The exporter's own conflict path already aborts, so a rebase in progress is either an interrupted exporter run or the user's manual work in the clone; the second case is lost every 30 minutes of the serve loop.
**Fix:** Abort only when the rebase directory is older than the git timeout and `journal.lock` shows no recent export, or stop with a `GitStop` instead.

### IN-07: `GIT_SSH_COMMAND` is overwritten, discarding the user's SSH identity configuration

**File:** `agent-brain/src/journal/gitsync.rs:67-76`
**Issue:** An existing `GIT_SSH_COMMAND` or `core.sshCommand` (for example `-i ~/.ssh/vault_key` or an agent socket) is replaced by a fixed command. Setups that depend on it fail with "Permission denied", and the failure is reported as an SSH/auth error with no hint.
**Fix:** Append the hardening options to the user's existing command when one is set, or document the limitation in the README section.

### IN-08: `/tim` replies are cut mid-line and mid-URL when the result exceeds `MAX_TEXT`

**File:** `agent-brain/src/telegram/journal_cmd.rs:22-37, 71`
**Issue:** `search_max_results` is allowed up to 20 and a title can be up to 400 characters, so the reply can exceed 3900 characters. `clip` then truncates the last result in the middle of its GitHub URL while the header still states the full count.
**Fix:** Stop appending results once the next one would exceed the budget and say "… và N kết quả khác".

### IN-09: `journal.repo_url` accepts embedded credentials

**File:** `agent-brain/src/journal/config.rs:186-199`
**Issue:** `https://user:token@host/...` passes validation, is stored in `config.json` and in the clone's `.git/config`, and is not in the known-secret set. It cannot reach the vault (the title sanitizer drops URLs and the gate has a `url` rule), but it is a plaintext credential in two files.
**Fix:** Reject a userinfo part containing `:` for `https://` and `ssh://` URLs (use SSH keys or a credential helper) and say so in the error.

---

_Reviewed: 2026-11-10_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
