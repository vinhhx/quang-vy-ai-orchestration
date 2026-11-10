---
phase: 07-journal-vault-obsidian
plan: 04
subsystem: journal
tags: [rust, sanitize, secrets, regex, gate, obsidian]
requires:
  - "07-02: journal::sanitize skeleton (SafeText, sanitize_text, scan_generated, SecretHit, ALLOWED_PUNCT)"
provides:
  - "sanitize_text pattern rules: diff/code/PEM cut, URL, e-mail, key=value, Bearer, provider token shapes, address, path, env-name and long-blob replacement before the cut"
  - "scan_generated gate rules: known-value, url, abs-path, pem, token-shape, jwt, telegram-token, key-assignment, email (one hit per file and rule, masked path, never the match)"
affects: [07-03, 07-05]
tech-stack:
  added: []
  patterns:
    - "regexes compiled once in std::sync::LazyLock statics; shared shape constants feed both the sanitizer and the gate"
    - "key-shaped test strings built at run time from prefix + body"
key-files:
  created:
    - agent-brain/tests/journal_sanitize.rs
  modified:
    - agent-brain/src/journal/sanitize.rs
key-decisions:
  - "Signatures of sanitize_text, scan_generated, SafeText, SecretHit, ALLOWED_PUNCT, the allow-list and cut_chars are unchanged (07-03 golden files stay valid)"
  - "Gate report path is masked with the same shape rules (known values and token/URL/e-mail shapes replaced by [đã ẩn]), so a secret that sits in a path cannot be echoed by the report"
requirements-completed: [JRN-03]
status: complete
commits: 2
plan_head_before: cad033c827859cf9837ab8a19642903e0dfa9389
plan_head_after: 5c4ba9c5fdee341dcdb6514bb6664a838a9748c9
actuals:
  tokens: 14000
  tasks: 2
  commits: 2
duration: ~50min (most of it waiting for cold cargo builds)
completed: 2026-11-10
---

# Phase 7 Plan 04: Sanitizer patterns and secret gate Summary

**`sanitize_text` now removes pasted diffs/code fences/PEM blocks and replaces URLs, e-mails, `key=value` secrets, Bearer values, provider token shapes (Anthropic, OpenAI, GitHub, GitLab, Slack, AWS, Google, JWT, Telegram), addresses, paths, ALL_CAPS env names and long letter+digit blobs with fixed Vietnamese words before the allow-list and the cut; `scan_generated` gates every generated body and path with nine rules and reports only a masked path and a rule name.**

## Tasks

| Task | Name | Commit |
| ---- | ---- | ------ |
| 1 | sanitize_text replaces paths, URLs, hosts, e-mails, key shapes and env names before the cut, drops diffs/code | 224a408 |
| 2 | Secret gate rules for every body and path; end-to-end leak and planted-secret tests | 5c4ba9c |

## What changed

- `agent-brain/src/journal/sanitize.rs`
  - New order in `sanitize_text`: NFC; drop invisible chars but keep newlines (`\r` becomes `\n`); `cut_code_or_pem` (earliest of: a line starting with `diff --git `, `--- a/`, `+++ b/`, `@@ -`, `index <7+ hex>..`; a triple backtick; `-----BEGIN`) drops it and everything after and appends ` (đã bỏ phần mã)` (` (đã ẩn)` for PEM); one line; known values via `redact_str`; `replace_patterns` (URLs `(liên kết)`, e-mail `(email)`, key assignment, Bearer, token/JWT/Telegram shapes `(đã ẩn)`, IPv4/localhost and `.com/.vn/...` hosts `(địa chỉ)`, Windows, `~/`, absolute and relative paths `(đường dẫn)`, ALL_CAPS env names and 24+ char blobs with a letter and a digit `(đã ẩn)`); allow-list, collapse, trim, cut unchanged.
  - `KEY_ASSIGN` swallows the whole name (`DEEPSEEK_API_KEY=...`), an optional quote, `:`/`=`, an optional `Bearer`/`Basic` word and the value.
  - `GATE_RULES` and `scan_generated`: `known-value` first, then `url`, `abs-path`, `pem`, `token-shape` (token shapes plus `Bearer` with 16+ chars), `jwt`, `telegram-token`, `key-assignment`, `email`; each tested on body and path; one `SecretHit` per (file, rule) with `mask_path` (known values and shaped strings replaced by `[đã ẩn]`). The long-blob and relative-path rules are deliberately not in the gate.
- `agent-brain/tests/journal_sanitize.rs` (new): `hostile_request_leaves_nothing_in_vault`, `planted_secret_blocks_export`.
- Unit tests added to `journal::sanitize::tests`: `plain_vietnamese_unchanged`, `markup_is_neutralized`, `paths_urls_hosts_emails_redacted`, `secret_shapes_redacted`, `diff_and_code_are_cut`, `redact_before_cut`, `pem_block_removed`, `zero_width_and_bidi_removed`, `gate_rules_fire`, `gate_report_never_holds_the_value` (10 new; the 4 from 07-02 still pass).

## Verification (real output)

- RED, Task 1: `cargo test --offline --locked --lib journal::sanitize` before the implementation: `8 passed; 4 failed` (`diff_and_code_are_cut`, `paths_urls_hosts_emails_redacted`, `pem_block_removed`, `secret_shapes_redacted`). GREEN after: `12 passed; 0 failed` (all of plain_vietnamese_unchanged, markup_is_neutralized, paths_urls_hosts_emails_redacted, secret_shapes_redacted, diff_and_code_are_cut, redact_before_cut `... ok`).
- RED, Task 2: `journal::sanitize::tests::gate` before the gate rules: `gate_rules_fire` and `gate_report_never_holds_the_value` FAILED. After: both ok.
- `cargo test ... --lib journal -- --test-threads=1` (stderr dropped): `test result: ok. 35 passed; 0 failed` including `gate_rules_fire ... ok`.
- `cargo test ... --test journal_sanitize -- --test-threads=1`: `hostile_request_leaves_nothing_in_vault ... ok`, `planted_secret_blocks_export ... ok`; `2 passed; 0 failed`.
- `cargo test ... --test journal_export -- --test-threads=1`: `5 passed; 0 failed` (no tracer regression).
- Whole lib suite: `761 passed; 0 failed`.
- Acceptance greps: `đã bỏ phần mã` present; `(đường dẫn)|(liên kết)|(địa chỉ)` count 7; `sanitize_text` signature line unchanged (1 line); rule-name grep count 19 (>= 8); the two test fn names found; complete-token literal grep prints `0` for both files; `rustfmt --check --edition 2021` clean on both files; `git diff` shows no change to `Cargo.toml`/`Cargo.lock`.
- `planted_secret_blocks_export` asserts: exit 1, stderr has `known-value`, neither stdout nor stderr contains the value, `remote_commits` empty, and the clone directory has no `Nhật ký`, no `Dự án` and no `.git` (the gate stops before the clone, so nothing is written, committed or pushed).

## Deviations from Plan

1. **[Rule 3 - Blocking] `cargo fmt --check` is not clean repo-wide (pre-existing, ~100 files).** The Task 2 verify clause `cargo fmt --check` therefore still fails for out-of-scope reasons. I did not run `cargo fmt` on the crate; both touched files are `rustfmt --check --edition 2021` clean. Same deviation as 07-01 and 07-02.
2. **`hostile_request_leaves_nothing_in_vault` was not RED before the gate rules.** The plan expected it to fail before Task 2, but Task 1 already makes the sanitizer remove every listed item, so it passed from the start; it still guards the end-to-end behavior on the remote's checkout. `gate_rules_fire` did have a real RED run. To keep the hostile items inside the cut, the test sets `journal.title_max_chars` to 400 and `callout_title_chars` to 200, puts the pasted diff last, and asserts the replacement words are present so the test cannot pass just because the text was truncated.
3. **Gate hit path is masked beyond known values (additive).** Besides `redact_str`, the hit path goes through the shape rules, so a token-shaped project slug cannot be echoed by the report. The existing `scan_finds_known_value_and_redacts_path` still holds.
4. **Plan commit ledger not persisted.** The worktree git dir is not writable from the sandbox (as in 07-01); `plan_head_before` is the base SHA verified at start (`cad033c...`) and `commits: 2` is measured with `git rev-list --count cad033c..HEAD`.
5. **Known limits of the rules (by plan, not bugs):** relative-path rule turns ordinary `a/b` text such as dates `15/01/2026` or `và/hoặc` into `(đường dẫn)`; a `key:`/`token:` phrase in prose hides the next word; ordinary words equal to an env secret of 8+ bytes block the export until text or value changes (flagged assumption A9).

## Threat model

T-07-15 and T-07-16: mitigated (`paths_urls_hosts_emails_redacted`, `secret_shapes_redacted`, `redact_before_cut`, `markup_is_neutralized`, `diff_and_code_are_cut`, end-to-end `hostile_request_leaves_nothing_in_vault`). T-07-17: mitigated (`planted_secret_blocks_export` asserts the value is absent from stdout/stderr; `gate_report_never_holds_the_value`). T-07-18: accepted (Rust `regex` is linear-time). T-07-SC: no dependency added. Threat flags: none.

No auth gates. No stubs.

## Self-Check: PASSED

- FOUND: agent-brain/src/journal/sanitize.rs (`đã bỏ phần mã`, `GATE_RULES`), agent-brain/tests/journal_sanitize.rs (`fn planted_secret_blocks_export`, `fn hostile_request_leaves_nothing_in_vault`)
- FOUND (ancestors of HEAD): 224a408, 5c4ba9c
