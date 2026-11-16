surface: mac-labels-contacts
own round (2026-11-08): the layout picked on the labels-r1 page; the look follows on the picked layout
picked-layout: L-C Inspector beside the mail — owner: "L-C Inspector beside the mail (Recommended)" (card 11:07, 2026-11-08; the tree edited in place, the definition in the reader pane with live matching rows and a join/leave/stay preview before Re-run)
picked-style:  LC-S1 Quiet inspector — owner: "LC-S1 Quiet inspector (Recommended)" (card 11:37, 2026-11-08; a calm Settings-style form in Native Glass in pane 3; one line under the tab row becomes the match count with a Matches | Mailbox switch; matched rows carry a small grey tag naming the rule; join/leave/stay as a 2 px edge bar; no new colours)
layout-alternatives: 3 (2 blind: L-A, L-B; L-C informed by the D1 direction board only)
layout-axis:         where the label's definition is edited: a sheet over the mail (L-A) · a dedicated screen (L-B) · an inspector in the reader pane beside the live list (L-C)
style-alternatives:  2 (1 blind to the layout board's pictures: LC-S2 proof first)
style-axis:          how loud the live proof is: quiet (LC-S1) · proof first (LC-S2)
motion:        Inspector enter: the reader content cross-fades to the inspector under a 0 to 6 pt blur over READER_CROSSFADE_MS (120 ms), opacity and blur only; pane widths never move. List re-sort: when a keystroke changes the match set, matched rows insert or leave with ROW_INSERT_MS (180 ms), transform and opacity only; the count line changes in tabular figures with no layout jump; the last count stays until the new read lands (no flash). Tag appearance: a new row's tag fades in over HOVER_TINT_MS (80 ms) a
boards:        http://127.0.0.1:8792/design/boards/labels-r1/ (rung 1) · http://127.0.0.1:8792/design/boards/labels-r2/ (rung 2, 2026-11-08) · D1 direction http://127.0.0.1:8792/design/boards/labels/
journeys:      J1.5, J2.6, J2.9, J2.10, J5.6, J1.12, J1.13
links-in:      see docs/JOURNEYS.md screen links for mac-labels-contacts
links-out:     see docs/JOURNEYS.md screen links for mac-labels-contacts
state: picked-on-localhost (both rungs, 2026-11-08); locked-on-build when its first build is measured

lock-amendment: the reader pane gains a second mode, the label inspector (⌘I toggles it, Esc returns the reader; pane widths unchanged) — accepted with the L-C pick, card 11:07

## Flow round (2026-11-09, A92-A94; Cue's own design per the owner's 17:46 "no subagent")
Board: ../docs/design/boards/labels-flow-r1/index.html (LF-1 "Pipeline"): the L-C inspector as three numbered stops on one rail: 1 Before Jev (rule chains IF/AND, THEN "put it here, Jev skips it"), J Jev reads the description (a given; "N messages say yes"), 2 After Jev (KEEP IF chains, ELSE "send the rest to <label|Primary|Spam>"); result line with the path ("1 before · 31 Jev · 8 sent to Primary"); facts as three-state switches (any/yes/no); the list beside shows live matches with a who-placed-it tag; state 5 shows a flipped fact letting a message go. Keys: R rule, A condition, ? why-tags, ↩ save, esc close. Motion: opacity + a 6 px rise ≤ 220 ms. Shots: _shots/LF1-<state>-<theme>-<size>.png (11). Worse at: a long chain pushes the result line down at 960; the three-state switch is new; the ELSE line is one more decision, on purpose.
picked-flow-look: v4 "three cards in order, compact, every fact visible", names A « Règles d'entrée · Classement par l'IA · Filtre de sortie » — owner: "Names A, build it as drawn (Recommended)" (card 2026-11-09 18:45; board ../docs/design/boards/labels-flow-r1/v4.html?names=a, shot _shots/V4-a-dark.png; rounds 1 to 3 rejected in the owner's words, PRD A94). The L-C layout and LC-S1 look stand; this pick replaces the "Règles fixes" card by the three cards.


## Rule rows: field and condition (2026-11-13, owner: "for the rule we shoudl have the actual rule (e.g., "Domain") and condition (is/is not) separately"; SPEC A161)
A text rule row of the Entry card reads: FIELD menu (Domain, From, Subject, Body, Account, To, Cc, List header), CONDITION menu, the value chips and field, the count, Remove. The condition menu offers two forms per field (round 18): Domain and Account `is` / `is not`, the text fields `contains` / `does not contain` (the core's `RuleCondition::Not`). Changing the field keeps the typed values between text fields and clears them to or from Account (its values are account ids). A saved rule opens as the same field and condition it was. Ids: `<chain>.rule.<i>.kind.<field>`, `<chain>.rule.<i>.op.<is|is_not|contains|does_not_contain>`, measured `geo.ctl.<card>.r<i>.op`.
