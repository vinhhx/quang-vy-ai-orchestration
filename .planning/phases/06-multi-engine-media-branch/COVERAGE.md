# API Coverage — Phase 6 (Anthropic-format endpoints, dsh through docker exec, Antigravity `agy` CLI, Telegram Bot API)

> Full coverage by default. Opt-outs are explicit, reasoned decisions.
> Scope source: 06-CONTEXT.md D-01…D-44 (locked 2026-10-24, amended 2026-10-26 and 2026-10-28), 06-RESEARCH.md § Engine Facts and the 2026-10-28 amendment, 06-UI-SPEC.md. Re-decided for the user-declared engine list (D-30…D-44): any Anthropic-format endpoint (the company gateway `https://ai-gateway.vcadm.vn/proxyllm` first, DeepSeek's official endpoint second) is reached only through `claude -p` with `ANTHROPIC_BASE_URL` (D-01, D-31, D-34); `dsh` is a black box reached only through `docker exec` (D-35); agy only through `agy -p` headless (D-04), now possibly several entries (D-30); Telegram only through the existing 05.3/05.4 bot. Every capability below was re-decided for this amendment; nothing is carried over silently from 05.3/05.4's COVERAGE.md or from the earlier `deepseek:*` block.

| capability | decision | reason |
|---|---|---|
| anthropic:messages (Anthropic-format POST /v1/messages through `claude -p`, company gateway first, then any other endpoint with E-09 evidence) | INTEGRATE | |
| anthropic:streaming (stream-json events parsed by `stream::Summary`) | INTEGRATE | |
| anthropic:tool-use (Read/Edit/Write/Bash under `--allowedTools` and guard hooks) | INTEGRATE | |
| anthropic:usage-accounting (input/output/cache tokens into `runs`, cost priced per engine id) | INTEGRATE | |
| anthropic:error-statuses (429 and 402 become a quota outcome, any other status becomes `run lỗi`) | INTEGRATE | |
| anthropic:model-mapping (per role and per entry from `config.json` `engines[].models`, decided per model at E-09) | INTEGRATE | |
| anthropic:context-caching (automatic; cache-hit tokens mapped to `cache_read_tokens`) | INTEGRATE | |
| anthropic:compat-knobs (`CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS`, `CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING`, set only on E-09's one retry or per entry) | INTEGRATE | |
| anthropic:route-precheck (one unauthenticated zero-token POST to `{base}/v1/messages`, printing only the HTTP status; user-run E-09 only) | INTEGRATE | |
| anthropic:models-list (GET /v1/models) | OPT-OUT | not needed — engines and models are declared in config.json and are not editable on the UI (D-08, D-33) |
| anthropic:balance-or-budget-endpoints (provider balance, spend or key-info routes) | OPT-OUT | not needed — readiness is local only (UI-SPEC C8: no model or API call, no tokens); budget stops are read from the error result |
| anthropic:openai-format-chat (OpenAI-format chat completions on the same gateway) | OPT-OUT | deferred (D-31) — only the Anthropic format through `claude` is integrated; an `openai` kind is a later phase |
| anthropic:thinking-budget-controls | OPT-OUT | not needed — Claude Code decides thinking; the two compat knobs are the only fallback |
| anthropic:batch-api (message batches) | OPT-OUT | not needed — roles are interactive agentic sessions, not batch jobs |
| dsh:headless-exec (`docker exec -w <cwd> <container> sh -c '…' sh <profile> <prompt>` with the marker wrapper) | INTEGRATE | |
| dsh:plain-text-output (final message as plain text lines, no tokens, `usage_source` none) | INTEGRATE | |
| dsh:in-container-kill (stop hook records the marker subtree, TERM then KILL by recorded PID, never by name) | INTEGRATE | |
| dsh:container-readiness (`docker ps --filter name=^C$ --format {{.Status}}`, read-only, local) | INTEGRATE | |
| dsh:mount-preflight (`docker inspect <container> --format '{{json .Mounts}}'` only, to refuse a project outside the same-path mount) | INTEGRATE | |
| dsh:web-ui (`dsh web`) | OPT-OUT | explicitly out of scope — D-35: dsh is a black box headless engine, not a service to integrate |
| dsh:plugin (`dsh plugin`) | OPT-OUT | not needed — plugins are the dsh user's own setup, invisible to agent-brain |
| dsh:container-lifecycle (start, stop, restart, rebuild, rm) | OPT-OUT | explicitly out of scope — agent-brain never changes the container's state (D-07, D-35) |
| dsh:full-inspect (bare `docker inspect`, container env listing) | OPT-OUT | explicitly out of scope — it prints the container's secrets; only `--format` forms are allowed |
| dsh:uid-mapping (`docker exec -u <uid>:<gid>`) | OPT-OUT | not needed yet — assumption A19 is observed by the user-run `--uid-probe` only, nothing depends on it |
| dsh:usage-tokens | OPT-OUT | not possible — dsh headless prints no token counts (S6), so cost stays 0 and the stats row says no usage |
| agy:print-mode (`-p`) | INTEGRATE | |
| agy:stream-json-output (`--output-format stream-json`, `event` key: init / step_update / result) | INTEGRATE | |
| agy:print-timeout (`--print-timeout`, always explicit) | INTEGRATE | |
| agy:multiple-entries (several `agy` engines in `engines[]`, each with its own id, command and model) | INTEGRATE | |
| agy:permission-modes (Dev only, limited to `--mode accept-edits` as decided at the 06-02 probe, D-26; no skip-permissions flag) | INTEGRATE | |
| agy:model-selection (`--model` from the entry's `model` when set) | INTEGRATE | |
| agy:usage (result.usage → runs tokens) | INTEGRATE | |
| agy:effort (`--effort`) | OPT-OUT | not needed yet — agy defaults are used; no requirement asks for effort control |
| agy:add-dir (`--add-dir`) | OPT-OUT | not needed — the run's cwd is the project root and agy must not see other folders |
| agy:conversation-resume (`--conversation` / `--continue`) | OPT-OUT | explicitly out of scope — sessions are never resumed across engines (D-18); each run is single-shot |
| agy:json-output (`--output-format json`) | OPT-OUT | not needed — stream-json covers result and usage and keeps live log lines |
| agy:interactive-ide | OPT-OUT | explicitly out of scope — CLAUDE.md: only the `agy` CLI, never the bare IDE launcher |
| telegram:getUpdates | INTEGRATE | |
| telegram:sendMessage | INTEGRATE | |
| telegram:editMessageText | INTEGRATE | |
| telegram:editMessageReplyMarkup | INTEGRATE | |
| telegram:answerCallbackQuery | INTEGRATE | |
| telegram:sendDocument | INTEGRATE | |
| telegram:setMyCommands | INTEGRATE | |
| telegram:getMe | INTEGRATE | |
| telegram:forward_origin | INTEGRATE | |
| telegram:webhooks (setWebhook, deleteWebhook, getWebhookInfo) | OPT-OUT | explicitly out of scope — long polling only, no inbound port (05.3 D-01 still locked) |
| telegram:local-bot-api (logOut, close) | OPT-OUT | not needed — the bot always uses the cloud Bot API server |
| telegram:command-menu-read (getMyCommands, deleteMyCommands) | OPT-OUT | not needed — setMyCommands replaces the menu at every serve start |
| telegram:getFile | OPT-OUT | explicitly out of scope — files in forwarded messages stay deferred (05.4) |
| telegram:message-copy-forward (forwardMessage(s), copyMessage(s)) | OPT-OUT | not needed — the bot only sends its own messages |
| telegram:deleteMessage(s) | OPT-OUT | not needed — quota and team-pick messages are edited to their final state and kept as history |
| telegram:media-sends (photo, audio, video, animation, voice, video note, paid media, media group) | OPT-OUT | not needed — quota and gate messages are text plus the existing plan document |
| telegram:location-venue-contact-dice | OPT-OUT | not needed |
| telegram:polls (sendPoll, stopPoll) | OPT-OUT | not needed — team choice uses inline buttons with a second confirm tap (D-28) |
| telegram:sendChatAction | OPT-OUT | not needed — the outcome message follows within seconds |
| telegram:setMessageReaction | OPT-OUT | not needed — outcomes are reported as messages |
| telegram:edit-caption-media-live-location | OPT-OUT | not needed — documents are never edited in place |
| telegram:chat-and-member-queries (getChat, getChatMember, getChatAdministrators, getChatMemberCount, getUserProfilePhotos) | OPT-OUT | not needed — authorization is the paired chat ID only; staff have no Telegram access (D-20) |
| telegram:chat-administration (ban, restrict, promote, permissions, titles, invite links, join requests, appearance, pinning, leaveChat) | OPT-OUT | explicitly out of scope — one private chat, no group management |
| telegram:forum-topics | OPT-OUT | explicitly out of scope — no groups |
| telegram:bot-profile | OPT-OUT | not needed — set by the user in @BotFather |
| telegram:inline-mode-and-web-apps | OPT-OUT | explicitly out of scope — only callback buttons on the bot's own messages |
| telegram:payments-stars-passport-games-stickers-gifts-stories-business | OPT-OUT | explicitly out of scope — none of these features exist in agent-brain |
