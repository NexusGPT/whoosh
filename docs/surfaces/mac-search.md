surface: mac-search
picked-layout: L1 In the list column — owner: "L1 In the list column" (card, 2026-11-11, redesign round 2, on http://127.0.0.1:8792/design/boards/search-r2-layout/ ; he first chose "Redesign the search screen first")
previous-picked-layout: field-dropdown-results — owner, 2026-11-08 02:05 (the reference he showed: Apple Mail's search field, its dropdown of suggestions, the results in the list pane): "IT CAN be as simple as that [Apple Mail search] no need for like etra menu and shit" · "just whatever you want to type and the actual result email and that's it" · "not in the toolbar necessarily. Keep the search cmd+K where it is. It is mroe about what happens when we click on it or cmd+k" ; re-stated 2026-11-08 03:00 unchanged (the app lock did not move; two false app rounds came from a per-screen note carrying the app-lock token in DESIGN.md)
picked-style:  S1 Quiet — owner: "S1 Quiet" (card, 2026-11-11, redesign round 2, on http://127.0.0.1:8792/design/boards/search-r2-style/ , on the layout L1)
previous-picked-style:  S1 Quiet rows — owner: "S1 Quiet rows" (card, 2026-11-08 08:15, on http://127.0.0.1:8792/design/boards/search/ ; overrides the night default S2 recorded 02:40 under "Default pick tonight, re-opened tomorrow")  [voided by the 2026-11-11 redesign: the round-2 looks are drawn on L1; no current picked-style until the owner picks]
layout-alternatives: 3 (0 blind) — round 2: L1 In the list column · L2 Command bar · L3 Search mode; drawn by the lead as labelled boxes (owner rule 2026-11-10: "I wanted YOU to make designs, not mlaunching lanes", no designer helper, so no blind lane)
layout-axis:         where the search lives and what the result screen is: inside the list column · a command bar over the window · the whole window as a search mode
style-alternatives:  3 (0 blind) — round 2: S1 Quiet · S2 Explained · S3 Top hit and faces; drawn by the lead on the main window with example mail, light and dark, 1280x820 and 960x632 (owner rule: no designer helper, so no blind lane)
style-axis:          how much a suggestion and a result say about themselves: one line and inbox rows · grouped suggestions and a third line that explains the result · a top hit card, initials discs and group titles
motion:        enters: the dropdown fades in and rises 2 px under the field, no delay; transitions: Enter swaps the list for skeleton rows at once, each row crossfades into its result as it arrives, no layout jump; a chip pops in from the accepted suggestion; responds: ↑↓ move a glass highlight (rule 11: short distance), hover acknowledges, Tab fills the chip; stays still: the field, the reader, the sidebar
boards:        http://127.0.0.1:8792/design/boards/search-r2-layout/ (layout) · http://127.0.0.1:8792/design/boards/search-r2-style/ (look)
journeys:      J2.1, J2.2, J2.3, J2.5, J2.8, J5.4, J5.5, J5.6
links-in:      Unified Inbox · the search field (click) · J2.1; Unified Inbox · ⌘K (GotEmail preset) or / (Superhuman preset) · J2.1; Settings closed (⌘W) · the field · J5.4
links-out:     Thread (Mac) · Enter on a result · J2.4, J5.5; Labels, Smart folders, Contacts (Mac) · Save as smart folder · J2.5, J2.6; Unified Inbox · Esc with an empty field · J2.1

## Round 3, 2026-11-11: the three parts of the list, the passage, the sender-domain filter

- layout: L1 In the list column (unchanged, owner's pick of 2026-11-11)
- look: S1 Quiet (unchanged, owner's pick of 2026-11-11)
- picked: P1 The passage on the row. Owner, card, 2026-11-11 (recorded in commit c7220114 of 21:12): "P1 The passage on the row (Recommended)"
- page: docs/design/boards/search-r3-parts/ (designs.js is the drawing: `results`, `row`, `more`, `typing`, `nothing`)
- what it fixes (SPEC A150, A152, A153): the list is Matches, then one thin line "Related", then the closed line "All other mail · N" (Enter or a click opens and closes it; the line and the bottom bar show the key); the count line reads "N matches · M related"; a result found in the text, an attachment or the thread shows a third line with the passage (a paperclip and the file name, or an arrow with the earlier mail's sender and date); a result found in its subject or sender stays two lines with the letters in bold; the reader opens at the passage, marked; typing a domain offers "From domain <domain>" and Tab makes the pill "From @<domain>"; with no match and no related mail the list says "No match" and keeps the closed line
- alternatives drawn: P1 · P2 Two-line rows, the passage in the reader
