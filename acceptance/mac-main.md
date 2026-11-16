# Acceptance: mac-main (the Unified Inbox)

Source: docs/JOURNEYS.md "J1: Triage the day's mail across 30+ accounts with keys   (roles: user)", signed by the owner 2026-11-07 ("Signed: the seven paths and your defaults stand"). These are the owner's signed words for this surface, copied from the journey, not written by a seat. The style pick adds the owner's own words below when the owner gives them.

## The owner's words (the journey's trigger and done-when, verbatim)

Trigger: the owner starts the day with new mail on many of the 30+ accounts · Done when: today's Primary tab holds no unhandled mail; each message shows in its new place (archived, snoozed until its time, labelled, pinned at the top, or answered), all done with keys

## What the owner sees at each step (the signed visible result)

| step | screen | the owner does | the owner sees |
|---|---|---|---|
| J1.1 | Unified Inbox (Mac) | reads the list; the first row has focus | the inbox paints inside the launch budget (docs/CONSTANTS.md) with today's mail from every account |
| J1.2 | Unified Inbox (Mac) | presses the archive key (E in the Superhuman preset) | the row leaves the list at once, the next row takes focus, the counts drop |
| J1.3 | Unified Inbox (Mac) | picks "tomorrow morning" | the row leaves; it shows in Snoozed with its return time |
| J1.4 | Unified Inbox (Mac) | pins it | the row moves to the top with a pin mark, on every account filter |
| J1.5 | Unified Inbox (Mac) | types "Suppliers", picks it (or creates it) | a "Suppliers" chip on the row; the message shows under that label in the sidebar |
| J1.6 | Thread (Mac) | reads | the thread opens inside the one-interaction budget; trackers blocked show as a count |
| J1.7 | Thread (Mac) | types two lines, sends (⌘Enter) | an undo-send bar counts down, then "Sent"; the reply shows in the thread |
| J1.8 | Unified Inbox (Mac) | presses undo (Z in the Superhuman preset) | the message returns to its place in the list |
| J1.9 | Command Palette (Mac) | types "move", picks "Move to another account", picks the account | the palette closes back to the Unified Inbox; the row leaves this account and shows under the target account; the palette showed the key for next time |
| J1.10 | Shortcut Trainer (Mac) | dismisses it, or presses E on the next row | the hint shows after the mouse action and stops after the owner has used that key three times (Settings turns hints back on) |
| J1.11 | Shortcut Trainer (Mac) | tries a key from the overlay, closes it | the overlay marks the keys the owner already uses |
| J1.12 | Unified Inbox (Mac) | moves to the next tab (Tab key) | an empty Primary tab that says "All done" with the next snooze return time; the next tab opens |
| J1.13 | mac-main | renames, reorders, adds or removes a split tab; sets its rule (sender, domain, account, label, decision-model category) | the strip updates in place; messages re-sort |

## Driven check

Driven check: `journeys/J1.py` (`python3 journeys/run.py J1`). This acceptance is met when that journey exits 0 on the installed bytes under the test identity, hidden. Cell: `acceptance-mac-main` in docs/feature-manifest.md; it reads this file and relays the J1 run.
