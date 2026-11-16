# iPhone parity: every decision made for the Mac, and what it means on the phone

Owner, 2026-11-09 20:20: "are all the design decision we've picked for main app translated to iphone ?" Answer: no. The iPhone screens were picked on 7 and 8 November (inbox layout "mail-26", look "Paper and Glass"; the eight other screens inherit that pair). Every decision since then was written for the Mac. This table carries each one to the phone with a fate: **Same** (the engine or the rule applies as is), **Adapted** (the phone gets its own shape of the same decision, to be designed), **Mac only** (no phone counterpart). Owner, 20:30: "Stands, carry on (Recommended)". A corrected line later re-opens that phone screen's design round.

| Decision (spec row) | What the Mac got | iPhone fate | Phone shape, where adapted |
|---|---|---|---|
| A26 One search, no separate Ask | search field answers questions too | Same | the search screen already picked (iphone-search) |
| A27 Search design S1 Quiet rows | the Mac search screen | Adapted | the picked iphone-search look, rows per S1's content order |
| A28 Meaning search (Gemma embeddings) | on-device embeddings | Same (engine) | the phone's model download is a later rollout question (size, battery) |
| A29 Attachments searched by content | PDF text, images, audio, video frames | Same (engine) | |
| A30, A34 Sidebar: Outbox on top, every standard mailbox | the Mac sidebar | Adapted | the phone's mailbox list (first screen of iphone-inbox), same order |
| A31 Right-click menu on the sidebar | context menu | Adapted | long-press menu |
| A32, A36, A37 Labels: add-label sheet, scope, modify with re-run, drag to re-arrange | the inspector (L-C) | Adapted | a full-screen label page (iphone-labels-contacts); drag with the standard reorder control |
| A33 Pinned labels as tabs | the tab row above the list | Adapted | a horizontal tab strip under the title, same order |
| A35, A43, A67 Superhuman keys, one per control; arrows; Enter | the key map | Adapted | no keyboard: swipe actions on rows (archive, done, snooze, label, delete), long-press for the rest; a hardware keyboard on iPad gets the map later |
| A38, A46, A47 ⌘K focuses the search; typing goes to one field | focus discipline | Adapted | the search field at the top of the inbox; keyboard dismiss on scroll |
| A39 Every control shows its key | key caps and tooltips | Mac only | the phone shows no keys |
| A40 Snappy arrow navigation, shorter motion constants | the list focus | Same (rule) | the same motion budgets (docs/MOTION.md) bind every phone screen |
| A41 Add account: one calm card | the sheet | Adapted | a full-screen card in the phone's first start |
| A42, A50, A56 Sidebar hides Apple style; the list widens | the Mac split view | Mac only | the phone is one pane at a time |
| A44 Every control clicked once by a check | the parity checks | Same (rule) | the phone checks drive the simulator |
| A48, A49 Accounts menu: all selected by default; an add-account button in it | the sidebar's accounts menu | Adapted | the account picker in the mailbox list, same rules |
| A51, A86, A89 The glass envelope mark; the app icon | BrandMark, the icon | Same | the icon set for iPhone (A14 pending) |
| A52 One language, 14-language catalogue, a picker | Settings › General | Same | the phone's Settings uses the same catalogue and picker |
| A53 Search inside content, attachments | | Same (engine) | |
| A54 The send shockwave across the window | the halo | Adapted | the halo across the phone screen on send |
| A55, A57, A69 Attachments: Quick Look, thumbnails, inline images listed once | the gallery | Adapted | the same gallery with Quick Look on tap |
| A58 Speed, RAM and CPU budgets | the footprint gate | Same (rule) | phone budgets to set (perf iphone row) |
| A59, A60 Settings window redesign; real vendor logos | the Mac Settings | Adapted | the phone's Settings list, grouped the same way, same logos |
| A61–A66, A70 Reader, quick reply, draft card, header toggle | the thread pane | Adapted | iphone-thread, same elements in one column |
| A71–A79 (notch, dictation, on-device AI, composer keys, perf, hidden test rig) | Mac | Mac only, except on-device AI (Same, engine) and dictation (Adapted: the phone's microphone button) | |
| A80 Relay host-agnostic | the hosted relay | Same | the phone's push depends on it |
| A81 Real-colour vendor logos everywhere | | Same | |
| A82, A84, A85, A89 First-start flow (Staged inbox, Aurora stage, Icon hero) | the Mac flow | Adapted | the phone's first start (iphone-welcome-accounts), same steps, phone geometry |
| A83 Import mail from Apple Mail, Outlook, Thunderbird | the import crate | Mac only | the phone has no local mail files |
| A87, A88 Plain refusal words; the default Microsoft sign-in ID | the sheet | Same | the phone's add account uses the same engine and ID |
| A90 The in-app Entra guide | 27 steps with screenshots | Adapted | the same steps, one per screen, phone geometry |
| A91 Primary and All as views; Unread as a filter chip; Spam folder | tabs and chip | Adapted | the tab strip gets Primary and All; the Unread chip at the strip's end; Spam in the mailbox list |
| A92–A94 Labelling flow: entry rules, AI sorting, exit filter | the three cards | Adapted | the same three cards on the phone's label page, full width |
| A95 App Store list | | Same | the iPhone app ships through the same Nexus account (items 1, 2, 5, 6, 14 apply to both) |
| A97 Admin consent on the Microsoft sign-in | the consent sentence and the two admin paths | Same | the phone's add account shows the same sentence |
| A98 The mailbox tenant's admin consents (the admin-consent link) | the sentence names the organisation and offers the link | Same | the phone shows the same sentence and link |
| A99 Digit chords by physical key (AZERTY) | ChordKit reads the digit row by key code | Mac only | the phone has no digit chords; an iPad hardware keyboard gets the same reader later |
| A100 One authorisation ask, ever (one stable signing certificate) | the Mac build signs with the owner's certificate | Same | the iPhone build signs with the same team's certificate through TestFlight or the App Store; the Keychain there is per app id by design |

Rule: a new decision for the Mac gets its iPhone line here the turn it is made, with its fate. A line marked Adapted is an owed phone design round before that screen is built.
