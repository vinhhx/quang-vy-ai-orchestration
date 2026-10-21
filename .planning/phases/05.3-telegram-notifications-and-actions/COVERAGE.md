# API Coverage — Telegram Bot API (Phase 05.3)

> Full coverage by default. Opt-outs are explicit, reasoned decisions.
> Scope source: 05.3-CONTEXT.md D-01 (narrow exception: notifications + gate/stop actions, long polling, one paired chat) and PROJECT.md Out of Scope. Task intake (`/task`, forwarded messages) is Phase 05.4.

| capability | decision | reason |
|---|---|---|
| getMe | INTEGRATE | |
| getUpdates | INTEGRATE | |
| sendMessage | INTEGRATE | |
| sendDocument | INTEGRATE | |
| editMessageText | INTEGRATE | |
| editMessageReplyMarkup | INTEGRATE | |
| answerCallbackQuery | INTEGRATE | |
| setWebhook | OPT-OUT | explicitly out of scope — D-01 long polling only, no inbound port on the machine |
| deleteWebhook | OPT-OUT | explicitly out of scope — long polling only; a webhook left on the bot is reported as a 409 error with a fix hint |
| getWebhookInfo | OPT-OUT | explicitly out of scope — long polling only |
| logOut | OPT-OUT | not needed — the bot always uses the cloud Bot API server |
| close | OPT-OUT | not needed — no local Bot API server |
| forwardMessage | OPT-OUT | not needed yet — forwarded-message intake belongs to Phase 05.4 |
| forwardMessages | OPT-OUT | not needed yet — Phase 05.4 scope |
| copyMessage | OPT-OUT | not needed — the bot only sends its own notifications |
| copyMessages | OPT-OUT | not needed — the bot only sends its own notifications |
| sendPhoto | OPT-OUT | not needed — notifications are text plus one plan document |
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
| sendPoll | OPT-OUT | not needed — actions use inline buttons |
| stopPoll | OPT-OUT | not needed — no polls |
| sendDice | OPT-OUT | not needed |
| sendChatAction | OPT-OUT | not needed — replies are immediate |
| setMessageReaction | OPT-OUT | not needed |
| editMessageCaption | OPT-OUT | not needed — results are written on the text message; the plan document is never edited |
| editMessageMedia | OPT-OUT | not needed — the plan document is never replaced in place |
| editMessageLiveLocation | OPT-OUT | not needed |
| stopMessageLiveLocation | OPT-OUT | not needed |
| deleteMessage | OPT-OUT | not needed — messages are kept as history and edited with the result (D-15) |
| deleteMessages | OPT-OUT | not needed — messages are kept as history (D-15) |
| getFile | OPT-OUT | not needed yet — receiving files belongs to Phase 05.4 intake |
| getUserProfilePhotos | OPT-OUT | not needed |
| getChat | OPT-OUT | not needed — authorization is the paired chat ID only (D-10) |
| getChatMember | OPT-OUT | not needed — authorization is the paired chat ID only (D-10) |
| getChatAdministrators | OPT-OUT | not needed — the bot works in one private chat |
| getChatMemberCount | OPT-OUT | not needed — the bot works in one private chat |
| chat-administration | OPT-OUT | explicitly out of scope — ban/unban/restrict/promote members, sender chats, permissions, custom titles: the bot only talks to one private chat |
| invite-links-and-join-requests | OPT-OUT | explicitly out of scope — export/create/edit/revoke links, approve/decline requests: no group management |
| chat-appearance | OPT-OUT | explicitly out of scope — setChatPhoto, deleteChatPhoto, setChatTitle, setChatDescription, chat sticker set: no group management |
| pinning | OPT-OUT | not needed — pinChatMessage, unpinChatMessage, unpinAllChatMessages |
| leaveChat | OPT-OUT | not needed — strangers' chats are ignored, not managed (D-10) |
| forum-topics | OPT-OUT | explicitly out of scope — create/edit/close/reopen/delete topics, general topic: no groups |
| bot-commands | OPT-OUT | not needed yet — setMyCommands, deleteMyCommands, getMyCommands: text commands arrive with Phase 05.4 (/task) |
| bot-profile | OPT-OUT | not needed — set/getMyName, set/getMyDescription, set/getMyShortDescription, set/getChatMenuButton, default admin rights are configured by the user in @BotFather |
| inline-mode | OPT-OUT | explicitly out of scope — answerInlineQuery, answerWebAppQuery, savePreparedInlineMessage: only callback buttons on the bot's own messages |
| payments-and-stars | OPT-OUT | explicitly out of scope — sendInvoice, createInvoiceLink, answerShippingQuery, answerPreCheckoutQuery, star transactions, refunds, subscriptions |
| telegram-passport | OPT-OUT | explicitly out of scope — setPassportDataErrors |
| games | OPT-OUT | explicitly out of scope — sendGame, setGameScore, getGameHighScores |
| stickers | OPT-OUT | explicitly out of scope — sendSticker and sticker-set management |
| gifts-stories-business-verification-boosts | OPT-OUT | explicitly out of scope — gifts, stories, business connections, verification, user chat boosts |
