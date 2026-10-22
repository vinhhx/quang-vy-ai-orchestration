# API Coverage — Telegram Bot API (Phase 05.4)

> Full coverage by default. Opt-outs are explicit, reasoned decisions.
> Scope source: ROADMAP Phase 05.4 (locked 2026-10-21: `/task` and forwarded messages → one task through the dashboard's task-creation function; plans of Telegram tasks approved on the dashboard only; images/files in forwards deferred), 05.4-RESEARCH.md, and the 05.3 constraints still in force (long polling only, one paired chat, plain text). Every capability below was re-decided for this phase; nothing is carried over silently from 05.3's COVERAGE.md.

| capability | decision | reason |
|---|---|---|
| getMe | INTEGRATE | |
| getUpdates | INTEGRATE | |
| sendMessage | INTEGRATE | |
| sendDocument | INTEGRATE | |
| editMessageText | INTEGRATE | |
| editMessageReplyMarkup | INTEGRATE | |
| answerCallbackQuery | INTEGRATE | |
| setMyCommands | INTEGRATE | |
| forward_origin (MessageOrigin on received messages) | INTEGRATE | |
| setWebhook | OPT-OUT | explicitly out of scope — long polling only, no inbound port on the machine (05.3 D-01 still locked) |
| deleteWebhook | OPT-OUT | explicitly out of scope — long polling only; a leftover webhook is reported as the existing 409 error |
| getWebhookInfo | OPT-OUT | explicitly out of scope — long polling only |
| logOut | OPT-OUT | not needed — the bot always uses the cloud Bot API server |
| close | OPT-OUT | not needed — no local Bot API server |
| deleteMyCommands | OPT-OUT | not needed — setMyCommands replaces the menu on every serve start |
| getMyCommands | OPT-OUT | not needed — the menu is written, never read back |
| getFile | OPT-OUT | explicitly out of scope — images/files in forwarded messages are deferred by the user (ROADMAP 05.4 Notes: làm sau); only captions are used |
| forwardMessage | OPT-OUT | not needed — the bot receives forwards; it never forwards messages itself |
| forwardMessages | OPT-OUT | not needed — the bot never forwards messages itself |
| copyMessage | OPT-OUT | not needed — the bot only sends its own messages |
| copyMessages | OPT-OUT | not needed — the bot only sends its own messages |
| deleteMessage | OPT-OUT | not needed — flow and selection messages are edited to their final state and kept as history |
| deleteMessages | OPT-OUT | not needed — messages are kept as history |
| sendPhoto | OPT-OUT | not needed — intake and notifications are text plus one plan document |
| sendAudio | OPT-OUT | not needed — text and plan document only |
| sendVideo | OPT-OUT | not needed — text and plan document only |
| sendAnimation | OPT-OUT | not needed — text and plan document only |
| sendVoice | OPT-OUT | not needed — text and plan document only |
| sendVideoNote | OPT-OUT | not needed — text and plan document only |
| sendPaidMedia | OPT-OUT | explicitly out of scope — no payments |
| sendMediaGroup | OPT-OUT | not needed — one document per gate message |
| sendLocation | OPT-OUT | not needed |
| sendVenue | OPT-OUT | not needed |
| sendContact | OPT-OUT | not needed |
| sendPoll | OPT-OUT | not needed — project/workflow/☑ choices use inline buttons |
| stopPoll | OPT-OUT | not needed — no polls |
| sendDice | OPT-OUT | not needed |
| sendChatAction | OPT-OUT | not needed — replies are immediate; the outcome message follows within seconds |
| setMessageReaction | OPT-OUT | not needed — outcomes are reported as messages |
| editMessageCaption | OPT-OUT | not needed — the plan document is never edited |
| editMessageMedia | OPT-OUT | not needed — the plan document is never replaced in place |
| editMessageLiveLocation | OPT-OUT | not needed |
| stopMessageLiveLocation | OPT-OUT | not needed |
| getUserProfilePhotos | OPT-OUT | not needed — sender labels come from forward_origin names only |
| getChat | OPT-OUT | not needed — authorization is the paired chat ID only; forwarded chat titles come from forward_origin |
| getChatMember | OPT-OUT | not needed — authorization is the paired chat and sender ID only |
| getChatAdministrators | OPT-OUT | not needed — the bot works in one private chat |
| getChatMemberCount | OPT-OUT | not needed — the bot works in one private chat |
| chat-administration | OPT-OUT | explicitly out of scope — ban/unban/restrict/promote, sender chats, permissions, custom titles: the bot only talks to one private chat |
| invite-links-and-join-requests | OPT-OUT | explicitly out of scope — no group management |
| chat-appearance | OPT-OUT | explicitly out of scope — setChatPhoto, setChatTitle, setChatDescription, sticker set: no group management |
| pinning | OPT-OUT | not needed — pinChatMessage, unpinChatMessage, unpinAllChatMessages |
| leaveChat | OPT-OUT | not needed — strangers' chats are ignored, not managed |
| forum-topics | OPT-OUT | explicitly out of scope — no groups |
| bot-profile | OPT-OUT | not needed — name, description, short description, menu button and default admin rights are set by the user in @BotFather |
| inline-mode | OPT-OUT | explicitly out of scope — answerInlineQuery, answerWebAppQuery, savePreparedInlineMessage: only callback buttons on the bot's own messages |
| payments-and-stars | OPT-OUT | explicitly out of scope — invoices, shipping, pre-checkout, star transactions, refunds, subscriptions |
| telegram-passport | OPT-OUT | explicitly out of scope — setPassportDataErrors |
| games | OPT-OUT | explicitly out of scope — sendGame, setGameScore, getGameHighScores |
| stickers | OPT-OUT | explicitly out of scope — sendSticker and sticker-set management |
| gifts-stories-business-verification-boosts | OPT-OUT | explicitly out of scope — gifts, stories, business connections, verification, chat boosts |
