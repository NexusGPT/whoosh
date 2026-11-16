# Surface: mac-quickreply (the in-thread quick reply + the thread's outgoing and draft cards)

picked-layout: three-pane — inherited from docs/DESIGN.md (the quick reply is the composer's in-thread sibling; owner card 13:35, 2026-11-08 "Keep the app look (Recommended)" on the composer; Cue records the inheritance 2026-11-08 17:50)
picked-cards: B full-width cards — owner: "B: full-width cards (Recommended)" (card 17:57, 2026-11-08)
picked-style:  Lifted until it lands — owner: "3 Lifted until it lands but with amazing animation that combines the aurora effect to landing" (card 2026-11-09 00:35; screen round on the picked layout B, 3 looks, 1 blind, boards http://127.0.0.1:8792/design/boards/thread-cards-r2/)
picked-style-history: native-glass inherited from docs/DESIGN.md (2026-11-08 17:50) until the owner rejected the first build 23:31

## Objective
After a send, the owner sees their message in the conversation at once, with the wait/undo control inside it; a ready draft is visible in the conversation and easy to delete. Owner, 17:19: "after mail is sent it should driectly show up in the conversations (with the wait/undo button on the chat too btw) and we should clearly show draft message that are ready to send (but also easily delete)". PRD "## Sent replies appear in the conversation at once"; SPEC A64, A65.

## Card layout rung (quickreply round 4)
Two labelled-box boards of the thread's bottom with the outgoing card and the draft card, on a named axis, served on the picks hub; the owner picks before the first source write. The pick line lands here as `picked-cards:` quoting the owner.

## Look rung (quickreply look round, 2026-11-09 00:35)
style-alternatives: 3 (1 blind): Fil calme (cards-s1, informed) · One sheet (cards-s2, informed) · Lifted until it lands (cards-s3, blind)
style-axis: how the thread's foot tells you what is yours and what is in flight (words and a dot · one released sheet · material: lifted glass until it lands)
picked: Lifted until it lands; the owner adds the landing animation: "amazing animation that combines the aurora effect to landing" (the Aurora send halo and the card's landing are one choreography; rendered as a playable board and confirmed before the build)
lock-amendment accepted with the pick: the reply box floats as a glass panel the thread scrolls under; every in-flight card (waiting, draft, failed) uses the same lifted glass; a sent reply lies flat. Cost: a second elevation level in the reader, about 30 pt of thread height.
motion: enters: the outgoing card rises 8 pt with opacity on the send frame (140 ms), the From picker scales from its anchor (120 ms) · transitions: at the end of the undo window the card LANDS: glass and shadow fade out, the hairline fades in, a 2 pt settle (160 ms), with the Aurora effect combined into the landing (to be designed on the board, FRAME_MS budget, no latency) · responds: hover tint 80 ms; the countdown ring is the only continuous motion · stays still: received messages, list, sidebar; reduced motion = cross-fades
boards: http://127.0.0.1:8792/design/boards/thread-cards-r2/ (cards-s3/index.html, ?play=1 runs the send)

## Landing animation (default confirm, 2026-11-09 02:05, Cue under the owner's 01:25 rule "Default pick, re-opened tomorrow")
Owner's words 00:35: "amazing animation that combines the aurora effect to landing". Board: http://127.0.0.1:8792/design/boards/thread-cards-r2/cards-s3/index.html?play=1 (Replay, Landing only; motion/story-light.mp4, motion/story-dark.mp4 and the frame strips). Judged by Cue from the frame strips: the Aurora ring (the send wave's own scheme) crosses the lifted card from the countdown ring; the frost clears in its wake; the hairline fades in; the card settles 2 pt; "Envoyé 23:19", the list row and the box header change on the landing frame; nothing waits on it; reduced motion = cross-fades.
Timings (board, constants LAND_*): LAND_AURORA_MS 480; LAND_BAND 0.11 of the card diagonal; LAND_GLASS_MS 360; LAND_STATE_MS 120; LAND_HAIRLINE_AT_MS 300 and LAND_HAIRLINE_MS 120; LAND_SETTLE_AT_MS 300, LAND_SETTLE_MS 160, LAND_SETTLE_PT 2; LAND_ACTS_AT_MS 420 and LAND_ACTS_MS 120; landing total 540 ms; SENDWAVE_MS 720 and ROW_INSERT_MS 140 unchanged. Board frame times: landing under 17 ms per frame in 4 runs. The owner re-opens this in the morning; the alternatives and the movie stay served.
