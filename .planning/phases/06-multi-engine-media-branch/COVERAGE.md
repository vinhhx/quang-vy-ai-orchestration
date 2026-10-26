# API Coverage — Phase 6 (DeepSeek Anthropic-format endpoint, Antigravity `agy` CLI, Telegram Bot API)

> Full coverage by default. Opt-outs are explicit, reasoned decisions.
> Scope source: 06-CONTEXT.md D-01…D-29 (locked 2026-10-24, amended 2026-10-26), 06-RESEARCH.md § Engine Facts, 06-UI-SPEC.md. DeepSeek is reached only through `claude -p` with `ANTHROPIC_BASE_URL` (D-01); agy only through `agy -p` headless (D-04); Telegram only through the existing 05.3/05.4 bot. Every capability below was re-decided for this phase; nothing is carried over silently from 05.3/05.4's COVERAGE.md.

| capability | decision | reason |
|---|---|---|
| deepseek:messages (Anthropic-format POST /anthropic/v1/messages through `claude -p`) | INTEGRATE | |
| deepseek:streaming (stream-json events parsed by `stream::Summary`) | INTEGRATE | |
| deepseek:tool-use (Read/Edit/Write/Bash under `--allowedTools` and guard hooks) | INTEGRATE | |
| deepseek:usage-accounting (input/output/cache tokens into `runs`, cost from `pricing.deepseek`) | INTEGRATE | |
| deepseek:error-statuses (401/402/429/503 → `quota` outcome or `run lỗi`) | INTEGRATE | |
| deepseek:model-mapping (role aliases or explicit model ids from `config.json engines.deepseek.models`, decided at E-09) | INTEGRATE | |
| deepseek:context-caching (automatic; cache-hit tokens mapped to `cache_read_tokens`) | INTEGRATE | |
| deepseek:thinking-controls | OPT-OUT | not needed — Claude Code decides thinking; DeepSeek ignores the budget; fallback env knobs only if E-09 shows 400s |
| deepseek:balance-endpoint (GET /user/balance) | OPT-OUT | not needed — engine readiness is local only (UI-SPEC C8: no model or API call, no tokens) |
| deepseek:models-list (GET /models) | OPT-OUT | not needed — engines and models are fixed in config.json and not editable on the UI (D-08) |
| deepseek:openai-format-chat-api | OPT-OUT | explicitly out of scope — D-01 uses the Anthropic-format endpoint through `claude` only |
| deepseek:fim-completion-beta | OPT-OUT | not needed — roles are agentic sessions, not completions |
| deepseek:json-output-mode | OPT-OUT | not needed — roles exchange results through protocol files (.plan.md, reports) |
| deepseek:dsh-container | OPT-OUT | explicitly out of scope — S6 black box; only if E-09 fails and the user chooses it (deferred, D-02) |
| agy:print-mode (`-p`) | INTEGRATE | |
| agy:stream-json-output (`--output-format stream-json`, `event` key: init / step_update / result) | INTEGRATE | |
| agy:print-timeout (`--print-timeout`, always explicit) | INTEGRATE | |
| agy:permission-modes (Dev only: `--mode accept-edits`, `--sandbox` or, as fallback, `--dangerously-skip-permissions`, decided at the 06-02 probe, D-26) | INTEGRATE | |
| agy:model-selection (`--model` from config.json engines.agy.model when set) | INTEGRATE | |
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
