surface: mac-notch
inherited from docs/DESIGN.md (the app-level pair; named on its inherits: line by the P1 plan, 2026-11-07)
picked-layout: three-pane — owner: "Classic three-pane", 2026-11-07 (inherited: the notch: new mail, login codes, buttons; hidden from screen shares; the pane spine applies where a pane exists, a sheet or window otherwise) ; re-stated 2026-11-08 03:00 unchanged (the app lock did not move; two false app rounds came from a per-screen note carrying the app-lock token in DESIGN.md)
picked-style:  native-glass — owner: "Native-glass is the best. Probably need to (i) be able to tweak the categories at the top (ii) have ability to connect or configure a local decision model like laya (https://huggingface.co/convaiinnovations/laya) or Jev with the API key and be able to properly configure it (iii) improve the 'account' at the bottom (iv) have a better view of each account on each email in the "all inbox" (this isn't clear) maybe for example deciding on a specific simple icon for each account (from a preloaded account library or an uploaded image + color for each account) etc.", 2026-11-07 (inherited) ; re-stated 2026-11-08 03:00 unchanged (the app lock did not move; two false app rounds came from a per-screen note carrying the app-lock token in DESIGN.md)
layout-alternatives: inherited
layout-axis:         inherited
style-alternatives:  inherited
style-axis:          inherited
motion:        restrained, by the lock: enters with a 120 ms fade; transitions on the system spring 220 ms; hover and focus acknowledge; nothing decorative
boards:        http://127.0.0.1:8792/design/boards/directions/native-glass/v2/index.html (the app-level pick)
journeys:      J6.1-J6.6
links-in:      see docs/JOURNEYS.md screen links for mac-notch
links-out:     see docs/JOURNEYS.md screen links for mac-notch
state: picked-on-localhost (inherited); locked-on-build when its first build is measured

## Notch look (screen round, night default 2026-11-09 02:50, Cue under the owner's rule "Default pick, re-opened tomorrow")
style-alternatives: 3 (a Continuum: the camera housing grows into the card, one continuous black shape · b Lantern: a glass card hung from a thin black lip · c Marquee: a wide black bar under the menu bar), boards http://127.0.0.1:8792/design/boards/notch/{a,b,c}/
picked-style-default: a Continuum — Cue: the notch idiom the owner named (GotMail, Nutshell: the housing and the card are one object; a Dynamic-Island reading); b Lantern is the Native Glass runner-up (a seam between black hardware and glass content); c covers the top strip of the front app. The owner re-opens this in the morning; the boards stay served.

## Redesign of five states (owner, 14 Nov 2026: "We need to redesign multiple states in notch: New notifications (and settings on how to notify) · Opening a message · Replying to a message · Going to main app · Searching a message")
state: layout PICKED 14 Nov 2026 09:4x; style round open; the picked look above (a Continuum) stays until the style round
redesign-picked-layout: L1 one card that grows — owner: "L1 is fine"
not covered by that answer: the settings layout (Settings > Notifications) and the draft of what each state must let the owner do; both are carried into the style round as drawn
layout-alternatives: 3 (0 blind: the owner's rule is that Cue draws the design rounds itself, and Cue had seen the built notch; L3 was drawn from the goal, search first)
layout-axis:         what the notch IS: L1 one card that grows (one mail at a time) · L2 list and message side by side (a small main window) · L3 a search bar whose rows grow (keyboard and search first)
objectives (Cue's draft, on the page for the owner to correct): a new mail: one glance, act or ignore · opening: read it here, text first · replying: a short answer without the app, words never lost · the main app: one door, same place, same mail, the reply carried · searching: a few letters from any app, open here or in Whoosh
settings-layout:     one page Settings > Notifications, the two-column template picked for Replies (what you set, what it gives): where a new mail shows · which mail may interrupt · the card · quiet
data that must exist for it: which mail may interrupt (per label, per account, VIPs), how long the card stays, sound, quiet hours; a search read for the notch; a key that opens the notch from any app (L3)
boards:        http://127.0.0.1:8792/design/boards/notch-r2-layout/ (built from build.py and wire.css in that folder)
as built today: pictures of main172, hidden, sample mail (the lead's scratchpad, notch-now); read in the board diary, 14 Nov
style round on L1: OPEN since 14 Nov 2026 (no pick yet)
style-alternatives: 3 (0 blind, as above) · style-axis: what carries the meaning: S1 quiet hardware (type alone, one blue) · S2 account light (the account's two colours on the rim, the main button, the time line, a bar on each result) · S3 reading sheet (the mail on a raised sheet in 15 pt reading type, second acts as icons with their key)
one change to the geometry limit, drawn and said on the page: the reply state is 372 pt high (limit today 320 pt)
motion (as written on the page per style): enters: the housing springs open into the card under 260 ms, words fade in behind the edge · transitions: the same shape resizes, the mail folds to two lines while the reply rises · responds: buttons brighten, a pressed key flashes · stays still: no text slides; at rest only the time line moves (S2: the rim light runs once on arrival and drains over the hold; S3: the sheet rises 8 pt)
style boards: http://127.0.0.1:8792/design/boards/notch-r3-style/ (built from build.py and style.css in that folder; static, no script)
owner, 14 Nov 2026, on the style round: "You need to make me design alterantive with emails (real ones) that have different images/html/etc. so I know hwo you can easily handle them and only display necessary information" · "Not sure how you access search as well" · "Also in settings different notification style (colored bar, opening mail, etc.)"
round on the owner's mail: OPEN since 14 Nov 2026. Served on this Mac only, http://127.0.0.1:8794/ (built by a script from 7 stored mails picked by shape; the lead read a twin with every letter replaced, never the mail text; removed after the pick; nothing of it is in this repo)
content-alternatives: 3, axis: what the notch shows of a mail: A its words (the engine's new text; folded parts as one line) · B its gist (one short line, the mail's main button, its files; depends on the AI summary, none kept in the store today) · C its page (the sender's page made smaller, pictures on request)
way to search, as drawn: the list (pointer on the notch, or the notch's key from any app) opens with the search field first; typing in the list searches; / from a card or an open mail; Return opens here, Cmd+O in Whoosh
notification styles, as drawn: light (the account's colour, no words) · one line · card · the mail opened · nothing; one default and one per kind of mail (VIPs, Primary, Updates, Promotions, sign-in codes), in Settings > Notifications
data that must exist for it (added): the notch's key from any app; a notification style per kind of mail; the kept thread summary for B
redesign-picked-content: A its words by default, with a button on the open mail that shows the full mail — owner: "We should have its word by default an a button to display full mail maybe when on the email" (14 Nov 2026, about 10:10)
as drawn to confirm (top of http://127.0.0.1:8794/): a new-mail card always shows the words; on the open mail the button Full mail (key O, the preset's Expand Message) shows the sender's page in the same card, which grows from 320 pt to 404 pt; the same button, Its words, goes back; the choice lasts for that mail; a plain mail has no page and no button; pictures still load only on Load pictures (key I)
still open on that page: the notification styles and the one per kind of mail · the look (quiet hardware, account light, reading sheet) · the way to search
content pick confirmed and changed — owner: "Ok. Full email load pictures by default" (14 Nov 2026, about 10:25): the drawing of words by default with the Full mail button stands; the full mail loads its pictures at once (no Load pictures step)
Cue's defaults with it, said to the owner: tracking pixels stay blocked; a switch in Settings turns the loading off; cost said once: each picture comes from the sender's server, so the sender can see the mail was opened
owner, 14 Nov 2026 about 11:55, to the lead's message that listed those defaults and the main window's own rule (it still asks before it loads pictures): "ok" (the defaults stand; the main window is unchanged)
owner, 14 Nov 2026 about 11:58: "give me a clear recommendation and localhost that is well structured so I can pick"
the page rebuilt as three picks (14 Nov 2026, about 12:05), same address: each pick with its options drawn on the owner's mail, good at and weak point per option, one recommendation marked, the answer asked as three codes
pick 1, what a new mail shows: 3 starting sets, axis: how much of the screen a new mail may take. 1A Quiet (VIPs one line, needs reply one line, primary light, updates nothing, promotions and social nothing, sign-in codes card) · 1B By weight (card, card, one line, light, nothing, card) · 1C Full (opened, card, card, one line, light, card); five values per kind in Settings (light, one line, card, opened, nothing)
pick 2, the look: 2A Quiet black (S1) · 2B Account colour (S2) · 2C Reading sheet (S3), each drawn as a new-mail card and an open mail on the owner's mail
pick 3, where typing starts when the notch opens: 3A in the search field (Down goes to the mails, where the main window's keys act) · 3B on the newest mail (keys act at once, / goes to the field); three ways in for both (pointer on the notch, the notch's key from any app, / on a card or an open mail)
Cue recommends (a recommendation, not a pick): 1B · 2A · 3A, reasons on the page; counts read from the owner's store on 14 Nov 2026 (counts only): 303 inbox mails over 8 days, 85 on the busiest day, 155 from mailing systems, 1 account
proposed start value of the notch's key: Option+M (Option+Space is Raycast's usual key and Raycast runs on this Mac)
drawing rule added: on a card and on an open mail, mark read (U) and snooze (H) show picture and key only, so no button is cut at 460 pt and 540 pt
data that must exist for pick 1 (added): the kind of each mail (VIPs, needs reply, primary, updates, promotions, social, sign-in codes); until the sorting is finished, a mail with a mailing-system header counts as Updates and the rest as Primary
CORRECTION (lead, 14 Nov 2026, read from the code and from the owner's settings): the main window loads a mail's pictures when it opens unless the owner turned that off (Settings > General > Privacy, key whoosh.messagebody.pictures.onOpen; unset means on; the owner's copy holds no saved value, so it is on). The lead's earlier sentence to the owner ("the main window still asks before it loads pictures") was wrong: it asks only when that switch is off. So by default the notch's full mail and the main window do the same; each keeps its own switch (the notch's: notch_alerts/full_mail_loads_pictures). To say to the owner in the next message.
data that must exist for it (added): a setting "the notch's full mail loads pictures" (default on)
redesign-picked-alerts: 1B By weight — owner: "1B 2A 3A" (14 Nov 2026, about 12:12): VIPs card · needs reply card · primary, the rest one line · updates light · promotions and social nothing · sign-in codes card; Settings > Notifications gives each kind of mail one of five values (light, one line, card, opened, nothing)
redesign-picked-style: 2A Quiet black (S1) — owner: "1B 2A 3A" (14 Nov 2026, about 12:12)
redesign-picked-search: 3A in the search field — owner: "1B 2A 3A" (14 Nov 2026, about 12:12): the list opens with the cursor in the search field; Down goes to the mails, where the main window's keys act; / from a card or an open mail

### Built, step 2 of the plan: the open mail (14 Nov 2026)

- A mail opens on its words (contracts `notch_words` over the body the reader gets): no reply starts and the notch takes no keyboard. What the engine folds is one named line ("Folded: 2 earlier messages, Signature").
- A mail with a page of its own has "Full mail" (the preset's expand key, O); the page is the reader's own page view in 196 pt of the card, "Its words" goes back. The line under it counts pictures and tracking pixels; pictures follow Settings > Notifications, "The full mail loads its pictures".
- Buttons, left to right: Reply (filled, R), Archive (E), Mark read while unread (U, picture and key), Snooze (H, picture and key; three choices 1 2 3), a menu (Mark unread, Open in Whoosh, Delete).
- Reply folds the mail to one line and gives the card to the reply (From, the words, Write it with AI, send). A reply the notch started ends when its mail leaves the notch; its words stay as the conversation's draft.
- Whose key it is: the notch takes a key only while the pointer is on it or it holds the keyboard (a reply being written). A key pressed in the main window while a card or a mail shows in the notch is the main window's.
- Check: `checks/notch-open-check.py`; the notch's full check follows it (`checks/parity/par-notch.py` J6.22, J6.34).

### Built, step 3 of the plan: search in the notch, the notch's key, Open in Whoosh (14 Nov 2026)

- The list is 540 pt wide (as drawn on the pick page) and opens with the search field on its first line ("Search all mail").
- The pointer alone never takes the keyboard. The notch's key, "/" and a click on the field put the cursor in the field and take the keyboard for the notch. A notch taken this way stays when the pointer leaves, and an alert taken this way does not leave on its timer.
- Typed words are asked of the engine's one search (Command.search and the Search stream, the door of the main window's search). The answer takes the list's place: the mails that hold the letters, then under "Related" the related mail; "all other mail" stays the main window's. The main window's own search is not touched. A search is kept as a recent one only when one of its mails is opened.
- Keyboard: Down leaves the field for the first mail, Up from it goes back; Return opens the mail in the notch (from the field: the answer's first mail); a letter typed on a row goes to the field; the preset's archive, read, reply and snooze keys act on the row; ⌘O opens the mail in Whoosh; Escape goes back one step (snooze choices, the open mail back to its list, the typed words, the notch).
- A card or a line taken with the notch's key answers Return, R, E and U; "/" goes to search from a card and from an open mail.
- A mail the search found is acted on like any mail of the notch and never joins the Latest and Unread lists, the unread count or an alert (`NotchItem.found`).
- Open in Whoosh takes a reply started in the notch with it: the main window opens on the mail with the same reply.
- The notch's key: Option and the key that types M on the keyboard in use, written once at first launch; Settings > Notifications shows it, records a new one (one of Command, Option, Control with one other key) and puts the start key back. System-wide keys go through the KeyboardShortcuts package. A hidden test copy registers no system-wide key.
- While the notch holds the keyboard, every key it binds stops with it and a letter it does not bind does nothing: no key pressed on the notch runs as a command of the main window behind.
- Check: `checks/notch-search-check.py`. Not judged by any test copy: a real key press from another app (a hidden window gets none).
style-alternatives: 3 (0 blind: Cue draws the rounds itself, by the owner's rule) · style-axis: what dresses the card (type alone on black · the account's colour · a raised reading sheet) · boards: http://127.0.0.1:8792/design/boards/notch-r3-style/ and the round on the owner's mail (http://127.0.0.1:8794/, the owner's mail removed after the pick, the letter-replaced copy kept)
state of the redesign: every pick made on 14 Nov 2026 (layout L1 · content A with the Full mail button, pictures loaded · alerts 1B · style 2A · search 3A); the build plan goes to the owner in plan mode next; locked-on-build when the first build is measured
