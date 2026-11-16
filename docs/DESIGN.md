# Design lock — Whoosh

state: picked-on-localhost (rung 1 layout three-pane; rung 2 style native-glass, with four owner-asked changes to draw into v2 and build; `locked-on-build` after the live build confirms) — history: layout-picked (rung 1 done; rung 2 round 1 REJECTED 2026-11-07 15:17 ("they are ALL really bad"); Cue's own direction Comet REJECTED 15:33 ("still not working"); round 2 = ten unique directions in progress, the owner picks on return; `picked-on-localhost` when both lines exist, `locked-on-build` after the live build confirms)
picked-layout: three-pane — owner: "Classic three-pane", 2026-11-07
picked-style:  native-glass — owner: "Native-glass is the best. Probably need to (i) be able to tweak the categories at the top (ii) have ability to connect or configure a local decision model like laya (https://huggingface.co/convaiinnovations/laya) or Jev with the API key and be able to properly configure it (iii) improve the 'account' at the bottom (iv) have a better view of each account on each email in the "all inbox" (this isn't clear) maybe for example deciding on a specific simple icon for each account (from a preloaded account library or an uploaded image + color for each account) etc.", 2026-11-07
picked-reader: conversation — owner: "Conversation is better actually. Couple of notes. in "Mailboxes" it would be good to have a button (drop down like to be able to select one or many inboxes) for the Mailboxes. Also we need to have each of the 2 panel draggable in width and "collapsable" like the mac native app", 2026-11-07 (the reading pane of mac-main, from five alternatives in docs/design/boards/reader/)
quality-bar: "the UI work needs to be absolutely amazing please. I want award winning design" (owner, 2026-11-07; binding on every surface: each unit is scored against it before any completion word)

## Taste (extracted from the owner before boards rendered, 2026-11-07)
references: "look at https://gotemail.net/ , superhuman, and other beautiful UI mail" (owner, first message); GotMail's notch, sign-in code pills and three-pane Mac layout; Superhuman's speed, split inbox and command palette. Reference material: the public pages and reviews in research/ (owner card: "Work from GotMail and Superhuman's public pages only").
anti_references: "Apple Mail: plain and slow" · "Hey: oversized spacing, playful chrome" · "Shortwave: busy and cluttered" · "Outlook: corporate density" (owner card, all four picked)
moodboard: seven images the owner sent 2026-11-07 ("Some moodboard inspiration"), kept at docs/design/moodboard/01.png to 07.png with the lead's reading in docs/design/moodboard/README.md: panels (white cards on grey, or frosted glass over a wallpaper, light and dark), one blue accent, SF-style sans, coloured status chips, inline key hints beside actions, rounded 10 to 14 px.
liked-most: "i like the ASCII pixelated type layout" (owner, 2026-11-07, on the moodboard): the coarse pixel-dither texture of images 04 and 05; the aurora board carries it as its signature, the other two show one dithered frame. Three more images at 15:03 (08 a dot streak, 09 a black hole in ASCII characters, 10 a crescent Earth in blue dots, all on pure black) plus a link to an image-to-ASCII generator: the owner likes artwork rendered as dots or characters on black; the dither is the ART, the UI text stays crisp.
register: "Calm precision, like Superhuman" (owner card) — quiet surfaces, one accent, typography does the work, motion only on state changes; plus the owner's first words: "REALLY fast and snappy (best in class)", "Beautiful UI", "Amazing UX".

## Brand (owner, 2026-11-07 18:35)
Owner: "for the logo, it's going to be this: [Image #23]" and "i have a version of the svg btw: ~/Downloads/envelope.svg". The app icon: a deep blue rounded square (a top-to-bottom blue gradient, lighter at the top) holding a tilted, slightly open envelope rendered as pale frosted glass with a light rim. Files: docs/design/brand/app-icon-reference.png (the owner's image) and docs/design/brand/envelope.svg (the owner's vector mark). Uses: the Mac and iPhone app icon sets (rendered from the reference at every size), the envelope mark alone as the in-app logo (Welcome, About, the notch's idle state), and the icon's blue family as the accent source for Native Glass (the accent stays the single blue already locked; the icon's gradient blues are for the icon and the welcome art only).

## Screen roster
surfaces: mac-main, mac-composer, mac-search, mac-palette, mac-trainer, mac-notch, mac-welcome-accounts, mac-settings-accounts, mac-settings-ai, mac-settings-general, mac-labels-contacts, mac-mcp-notes, iphone-welcome-accounts, iphone-inbox, iphone-thread, iphone-composer, iphone-search, iphone-palette, iphone-ask, iphone-settings, iphone-labels-contacts, iphone-notes, mac-add-account, mac-quickreply
inherits: mac-composer, mac-quickreply, mac-search, mac-palette, mac-trainer, mac-notch, mac-welcome-accounts, mac-settings-accounts, mac-settings-ai, mac-settings-general, mac-labels-contacts, mac-mcp-notes (the app-level pair: three-pane spine where a pane exists, Native Glass look; named by the P1 plan 2026-11-07). iPhone surfaces: no inheritance; iphone-inbox is core screen 2 and runs its own two picks with the owner on return (plan gap G2)., iphone-welcome-accounts, iphone-thread, iphone-composer, iphone-search, iphone-palette, iphone-ask, iphone-settings, iphone-labels-contacts, iphone-notes

mac-main = the Mac main window: sidebar, unified inbox list with split tabs, and the reading pane (Thread) in one window; core screen 1, whose boards are the app-level boards. Core screen 2 = iphone-inbox.

## Screen roster change (owner, 2026-11-08 01:52 to 02:05)
mac-ask retires into mac-search. mac-palette stays. mac-search keeps its field and key; on a click or ⌘K it shows a dropdown of suggestions (People, Labels, filter words, recent) as the user types, Enter launches the search behind skeleton rows, the results are the matching messages and nothing else. Inherits the Native Glass lock; the owner answers fix-inside-the-design or full redesign on the card of 02:10.

## Geometry
docs/design/geometry/mac-main.json · docs/design/geometry/mac-notch.json · docs/design/geometry/iphone-inbox.json (the owner's display measured; iPhone 17 Pro per the owner).

## Machine-readable axes
colorScheme: both (follows the system; Native Glass draws both at full size)
density:     regular (44 px two-line rows, sender + subject, no preview)
motion:      restrained (the glass tab indicator morphs 220 ms with the system spring; focus and hover acknowledge; nothing decorative) — owner 2026-11-07 20:50: "all the animation and micro animation and transition are AMAZING wihtout ADDING ANY LATENCY or HEAVYNESS": the canon docs/MOTION.md binds every screen; the value stays restrained (purposeful, weightless)

## Pick provenance
### Rung 1, layout (app level = core screen 1, mac-main)
layout_alternatives: 3
layout_axis: what the window fundamentally is: classic three-pane (reader dominates) · two panes and a rail (list dominates, folders behind a drawer) · one stream (thread expands in place, context rail)
layout_blind: n/a (inception: all three lanes ran from the objective, the data rows and the taste alone; none saw a screen or a sibling board)
boards: http://127.0.0.1:8792/design/boards/layout/ (served from docs/ on this Mac; files docs/design/boards/layout/<three-pane|two-pane-rail|stream>/)
owner_reasoning: picked "Classic three-pane" on the served page; my recommendation (rail, list, reader) and the stream were rejected without comment
picked_board: docs/design/boards/layout/three-pane/index.html (shot: docs/design/boards/layout/three-pane/shot-1280.png)
### Rung 2, style, round 1 (rejected)
style_alternatives: 3 rendered + 1 by Cue (Comet); owner: 'they are ALL really bad and shit' · 'none of these design are award winning. they feel cluttered, ugly and all' · 'This is still not working'
round_1_boards: http://127.0.0.1:8792/design/boards/style/ and http://127.0.0.1:8792/design/boards/direction/comet/index.html
### iphone-inbox layout (core screen 2), picked 2026-11-07
layout_alternatives: 7 over three rounds; picked mail-26 (the owner's Apple Mail 26 reference), owner: "Confirmed, this is it"; file docs/surfaces/iphone-inbox.md
### iphone-inbox style (core screen 2), picked 2026-11-07 19:16
style_alternatives: 2 (glass-dark, glass-light); picked glass-light "Paper and Glass", owner: "Light first"; the nine other iPhone screens inherit the app pair with the inbox as their frame (inherits: line)
### Rung 2, style, round 2: ten directions, served 2026-11-07 15:50
boards_round_2: http://127.0.0.1:8792/design/boards/directions/ (root + ten boards HTTP 200; files docs/design/boards/directions/<slug>/index.html, shots/<slug>-1280.png, alternatives.json)
judge: Cue read every board's shot twice (first render, then after corrections); nine of ten took one correction round, terminal accepted as drawn
owner_reasoning (rung 2): picked Native Glass with four changes: editable split tabs (i); a configurable decision model, local Laya or Jev with an API key (ii); a better accounts section at the sidebar foot (iii); a clear per-account mark on every row: a simple icon per account from a preloaded library or an uploaded image, plus the colour (iv). Cue's recommendation (Observatory) was not taken.
style_blind: n/a (inception; ten objective-driven lanes, none saw a sibling)
picked_board: docs/design/boards/directions/native-glass/index.html (shot: shots/native-glass-1280.png); v2 with the four changes: docs/design/boards/directions/native-glass/v2/ (confirming board, owner confirms on return)
style_alternatives: 10 planned (Observatory, Broadsheet, Instrument, Terminal, Atelier, Native Glass, Ledger, Blueprint, Nocturne, Transit), axis: the whole direction (concept, material, type, art, motion), each dark and light at full size; contract docs/design/boards/directions/ROUND.md; boards docs/design/boards/directions/<slug>/
old_round_1_plan: 3 planned, axis: the surface MATERIAL, each drawn from the owner's moodboard: silk (light frosted panels over a blue silk wallpaper, images 3 and 6) · aurora (dark glass panels over an aurora artwork, images 4 and 5) · cards (opaque white cards on a pale grey canvas, images 1, 2 and 7); all three on the three-pane layout, on the shared fixture docs/design/fixtures/mac-main-sample.json, one blue accent, inline key hints

### mac-search style (screen round, 2026-11-08 02:40)
mac-search look (screen round, not the app lock): S1 Quiet rows — owner: "S1 Quiet rows" (card 2026-11-08 08:15, three looks S1 Quiet rows · S2 Rich rows · S3 Media tiles on http://127.0.0.1:8792/design/boards/search/, 0 blind, settled paradigm from the owner's Apple Mail reference; the night default S2 of 02:40 is overridden by this pick).

### Labels, pinned labels and the sidebar (sub-surfaces of mac-main, 2026-11-08 09:45)
Direction D1 "Labels are the tabs" — owner card 09:45 with the D1 tab-row image: "for pin I meant here btw" (3 directions on http://127.0.0.1:8792/design/boards/labels/, D1 blind, D2 and D3 informed; the sidebar top, menu and add-label sheet are shared by all three). Label scope added by the owner in the same answer: shared by all mailboxes or specific to one.

## Account badge: emblem (owner pick, 2026-11-07)

Owner: "emblem is ok but you need to do a rich enough icon library and make the whole thing subtle enough on the UI so it is not overpowering." The board: docs/design/boards/badges/emblem (work home). Rules that follow from the pick:
- Every account is a lit glass disc in its own colour with a bold two-tone pictogram; the 56 pt drawing has five layers (base, inner shadow, specular, pictogram, rim); the 16 pt mark is the simplified drawing by rule (larger glyph box, no specular, no inner shadow, flatter gradient), never a shrunk copy.
- The library holds at least ICON_LIBRARY_MIN glyphs in named families; each glyph exists in the 56 pt and the 16 pt drawing.
- Subtle on the UI: on a list row the 16 pt mark uses the account colour at EMBLEM_LIST_SATURATION of its saturation and sits at ACCOUNT_MARK_ROW_PX; a row's text has more contrast against the row than the mark does; the owner judges a 16-row list side by side with the tile board's rows.
- The list can hide them: Settings > General "Show account emblems in the message list" (default on); off removes the mark and gives the text its width back (owner: "the option to hide or not emblems from list of mails").
- The sidebar's accounts section at the bottom is a pull-up / pull-down menu (owner: "the 'account' part at the bottom should be a pull up / pull down submenu"): collapsed it is one bar with the account marks and the sync line; pulled up it lists every account with unread, health and the All-accounts tick; the pull follows docs/MOTION.md rule 11 and ACCOUNTS_MENU_MS.

**Direction 2026-11-08 00:55 (owner, verbatim): "Emblems should be much calmer and fainter. More classy."** The treatment pick is open on the emblem board (http://127.0.0.1:8792/design/boards/emblems/): the current lit disc at 0.85 as reference against three calmer treatments (a faint fill with the glyph in the hue's dark shade; a thin hue ring with the surface inside; a neutral disc with the glyph alone in the hue). The owner's pick becomes the recipe; EMBLEM_LIST_SATURATION follows it. Until then the rules above stand.
**Direction 2026-11-08 01:35 (owner, verbatim): "We can go even more neutral and classy and consistent (glass style consistent shade) with rest of design"; the status dot: "Keep it strong".** The disc becomes the design's own glass material, one shade for every account; the glyph carries the colour, muted; the sign-in dot stays full orange. Three glass variants go on the board for the pick.

## Keys: simple by rule (owner, 2026-11-07)

Owner, quoted: 'dont be having complex shortcuts like "⌃⌘A"'. A Whoosh key is one letter (J, E, A) or ⌘ plus one key (⌘K, ⌘N). Never two modifiers, never ⌃ or ⌥ chords. The one exception is a preset that copies another app's keys for people who switch from it (the Mail preset keeps Apple Mail's own ⌃⌘A for archive because that is what their hands know); a Whoosh-native key never takes that shape.

## Locked on build (2026-11-08, bundle main21 at 67efaec): NOT LOCKED

The running window was measured against this lock (docs/lanes/design-build.md "## Round 1", checks/parity/par-design.py, last line `45 checks, 8 failed, 3 not judged`). The state stays `picked-on-localhost`. What blocks the lock:
- 960x600: sidebar 220 (lock 200), list 338 (lock 320), reader 400 (rest 440), window height 632 (min 600). Lane macshell.
- Differences from the v2 drawing still open: top chrome and command field, list header, row hairlines, tab strip clipping, sidebar material not confirmed, reader empty state, reader header actions, summary and attachment card, quick reply send button, reader title wrap at 960. Lanes inboxlist, reader, quickreply, macshell.
- Not judged: type sizes (not exposed), motion durations (no frame verb), perf/interaction.py (frame measure not served).
Green at 1280x820 in both themes: sidebar 220, list 400, foot 76, row 44, key strip 36, contrast (body 16.8/14.9, secondary 5.1/6.5, selection 5.5/6.1), emblem quiet saturation, rule 11 and rule 3 at state level.

### mac-labels-contacts layout (screen round, not the app lock), 2026-11-08 11:07
L-C Inspector beside the mail — owner: "L-C Inspector beside the mail (Recommended)" (card 11:07). Alternatives 3 (axis: where the definition is edited; L-A sheet and L-B dedicated screen blind, L-C informed by D1), boards http://127.0.0.1:8792/design/boards/labels-r1/. Lock amendment accepted with the pick: the reader pane gains an inspector mode (⌘I, Esc back; pane widths unchanged). Rung 2 (look) open.

### Account emblems, 2026-11-08 11:07
Owner's pick on the emblem board (http://127.0.0.1:8792/design/boards/emblems/, three glass treatments): "Grey glyph but no colour dot": the G3 glass chip with the grey glyph, the 2 pt colour dot removed; the account's colour stays in its sidebar row and the reader meta. Applied by the lead from lane/emblem-board-r3 (EmblemTreatment.swift), values to docs/CONSTANTS.md.

### mac-main sidebar structure (screen round, not the app lock), 2026-11-08 11:14
S-B Apple behaviour on Whoosh grouping — owner: "S-B Apple behaviour, Whoosh grouping (Recommended)" (card 11:14). Alternatives 2 (settled paradigm: Apple Mail, the owner's screenshots; no blind lane), boards http://127.0.0.1:8792/design/boards/sidebar-apple/. Lock amendment accepted: the list header's "All accounts ▾ · N unread" row folds into the title band. The look stays the Native Glass lock (no look rung for a structure change inside an already-styled screen).

### mac-add-account layout (screen round, redesign), 2026-11-08 11:50
A-A One calm card — owner: "A-A One calm card (Recommended)" (card 11:50). Alternatives 3 (axis: what dominates while the app works; A-C blind), boards http://127.0.0.1:8792/design/boards/addaccount-r1/. Frame fixed by the owner 10:57: a sheet over the main window, inline in Welcome's quick start. Rung 2 (look) open. Roster: mac-add-account added to the surfaces line.

### mac-labels-contacts look (screen round), 2026-11-08 11:37
LC-S1 Quiet inspector — owner: "LC-S1 Quiet inspector (Recommended)" (card 11:37). Alternatives 2 (axis: how loud the live proof is; LC-S2 blind to the layout pictures), boards http://127.0.0.1:8792/design/boards/labels-r2/. No lock amendment (LC-S2's two diff hues not taken). Both rungs picked: the labels build may start against this pair.

### mac-add-account look (screen round), 2026-11-08 11:45
AA-S1 Quiet glass — owner: "AA-S1 Quiet glass (Recommended)" (card 11:45). Alternatives 2 (axis: how much the sheet speaks; AA-S2 blind to the layout pictures), boards http://127.0.0.1:8792/design/boards/addaccount-r2/. Lock amendment accepted with the pick: the sheet in the dropdown's glass over a 0.30 scrim. Both rungs picked: the add-account rebuild may start against this pair.



lock-amendment (2026-11-08 15:39, Settings window): pane icons sit in coloured rounded squares (ten system hues, white SF Symbols) inside the Settings window only; everywhere else the single blue accent stands. Accepted with the owner's look pick "S3: System Settings materials (Recommended)". Cost: a second colour family in one window.


rule (2026-11-08 15:46, every surface): NO coloured strip, bar or stroke on a card's or row's edge (left accent bars, coloured borders to mark a state). State is told by a small dot, a word or a control, never by colouring an edge. Owner on the S3 Settings board's "Needs you" card: "DO NOT have this style of color on edge. This is too much AI geneated".

### mac-quickreply look (screen round), 2026-11-09 00:35
Lifted until it lands — owner: "3 Lifted until it lands but with amazing animation that combines the aurora effect to landing" (card 00:35). Alternatives 3 (axis: how the thread's foot tells what is yours and what is in flight; cards-s3 blind, cards-s1 and cards-s2 informed), boards http://127.0.0.1:8792/design/boards/thread-cards-r2/. The first build (quickreply round 4) was rejected 23:31 ("This is not it"). Lock amendment accepted with the pick: in the reader, in-flight things (the reply box, a waiting reply, a draft, a failed send) float as lifted glass over the thread, a second elevation level; a sent reply lies flat. The owner's addition: the Aurora send effect and the card's landing are one animation, rendered on the board and confirmed before the build.

### mac-quickreply landing animation default (2026-11-09 02:05)
Confirmed as the night default by Cue under the owner's rule of 01:25 (Default pick, re-opened tomorrow): the Aurora landing of the cards-s3 board (timings in docs/surfaces/mac-quickreply.md); re-opened with the owner in the morning.

### mac-notch look default (2026-11-09 02:50)
Night default by Cue (owner rule 01:25 Default pick, re-opened tomorrow): a Continuum from http://127.0.0.1:8792/design/boards/notch/ (3 alternatives); the owner re-opens it in the morning.

### mac-notch redesign of five states, layout (2026-11-14)
Owner, 14 Nov 2026: "We need to redesign multiple states in notch: New notifications (and settings on how to notify) · Opening a message · Replying to a message · Going to main app · Searching a message".
layout-alternatives: 3 (0 blind: Cue draws the rounds itself, by the owner's rule) · layout-axis: what the notch is (L1 one card that grows · L2 list and message side by side · L3 a search bar whose rows grow)
redesign-picked-layout: L1 one card that grows — owner: "L1 is fine" (14 Nov 2026, about 09:40)
boards: http://127.0.0.1:8792/design/boards/notch-r2-layout/
state: picked-on-localhost for the layout; the style round follows on L1; not locked on a build

### mac-notch redesign of five states, content, alerts, style and search (2026-11-14)
content-alternatives: 3 · content-axis: what the notch shows of a mail (A its words · B its gist · C its page)
redesign-picked-content: A its words by default, with a button on the open mail that shows the full mail — owner: "We should have its word by default an a button to display full mail maybe when on the email" · then "Ok. Full email load pictures by default" (14 Nov 2026)
alerts-alternatives: 3 · alerts-axis: how much of the screen a new mail may take (1A Quiet · 1B By weight · 1C Full)
redesign-picked-alerts: 1B By weight — owner: "1B 2A 3A" (14 Nov 2026, about 12:12)
style-alternatives: 3 (0 blind: Cue draws the rounds itself, by the owner's rule) · style-axis: what dresses the card (2A type alone on black · 2B the account's colour · 2C a raised reading sheet)
redesign-picked-style: 2A Quiet black (S1) — owner: "1B 2A 3A" (14 Nov 2026, about 12:12)
search-alternatives: 2 · search-axis: where the keyboard is when the notch opens (3A in the search field · 3B on the newest mail)
redesign-picked-search: 3A in the search field — owner: "1B 2A 3A" (14 Nov 2026, about 12:12)
boards: http://127.0.0.1:8792/design/boards/notch-r3-style/ · http://127.0.0.1:8794/ (the round on the owner's mail; the owner's mail removed after the pick)
state: picked-on-localhost; the build plan goes to the owner in plan mode; locked-on-build when the first build is measured

