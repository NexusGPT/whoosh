# Motion canon: amazing, and never a millisecond slower

Owner (2026-11-07, verbatim): "make sure all the animation and micro animation and transition are AMAZING wihtout ADDING ANY LATENCY or HEAVYNESS (https://www.uilabs.dev/)". The reference is a gallery of micro-interactions (uilabs.dev: two-step popover, action toolbar, dynamic settings with blur, contextual toolbar, elastic card, dynamic header dropdown, flexible tags, calendar widget): state changes acknowledged in place, containers that morph instead of appearing, content that cross-fades under a blur, toolbars that suggest the next action. The design lock keeps `motion: restrained` (docs/DESIGN.md): purposeful, never decorative. This file binds every screen lane; the shared controls (Sources/Features/Components) and the tokens (WhooshMotion) carry the values.

## The law: feedback first, motion after
1. The state changes on the frame the input lands. A key or click never waits for an animation: archive removes the row from the model at once; the collapse animates while focus already sits on the next row. Input-to-state: 1 frame (MOTION_FEEDBACK_FRAMES = 1).
2. No animation gates the next action. Every animation is interruptible; a second key during a collapse is applied, not queued.
3. Nothing animates on a path the owner hits hundreds of times a day beyond a one-frame acknowledge: J/K focus moves never wait on motion: the focused row changes on the key, and the highlight's travel follows rule 11 (a slide of at most SLIDE_MAX_ROWS rows, a fade beyond; resolved 2026-11-07, the owner's sliding-indicator rule wins over the earlier 'no slide'); E/H/P/L on the focused row make it leave: the mail slides LEFT and dissolves while the mails below close the gap behind it (owner, 2026-11-11, verbatim: "on superhman they go on the left and dissolve gently", "but fast"). Held (key repeat), every mail leaves the same way, the exits overlap, none is skipped or queued; the model changes on each key's frame. One exit motion for every way a mail leaves the list. Reduced Motion: a cross-fade, no slide.
4. Durations: 120 to 260 ms, never more (MOTION_MAX_MS = 260). Springs: the system spring with response 0.22 to 0.35 and damping 0.8 to 0.9 (TAB_MORPH_DAMPING); no bounce, no overshoot beyond 2 %.
5. Only transform and opacity animate (position, scale, blur radius, alpha). Never animate layout-affecting values on a list (height, padding, font size) except the one collapsing row.
6. Containers morph, they do not pop: the split-tab indicator (matchedGeometryEffect, TAB_MORPH_MS), the reader cross-fade on selection (READER_CROSSFADE_MS), the undo capsule rising UNDO_RISE_PX, a popover scaling from its anchor (0.96 to 1.0, 160 ms ease-out).
7. Hover and focus acknowledge within HOVER_TINT_MS; a press darkens by one step the same frame.
8. Content swaps under a short blur cross-fade (reader body, settings pages): opacity + blur radius 0 to 6 pt over READER_CROSSFADE_MS.
9. Reduced Motion (system setting) turns every movement into a cross-fade; nothing breaks.
10. Weightlessness is measured, not felt: perf/interaction.py (frame >= 16 ms or commit >= 50 ms over 100 archive/next/open actions = red) is the gate for every lane that touches the list, the reader or the chrome; a lane adds no timer, no DispatchQueue delay and no animation completion handler on the input path.

## Vocabulary (from the reference, in Whoosh's terms)
- Action toolbar: the selection bar (X, ⇧J) rises as one glass bar; its buttons morph as the selection changes.
- Contextual toolbar: the reader's actions appear where the cursor is, not in a fixed bar.
- Dynamic settings: a Decisions page's model entry opens in place; the list around it reflows with a spring.
- Elastic card: a message row's swipe (iPhone) follows the finger; release past the word commits.
- Two-step popover: the snooze picker's "Pick a date" turns the popover into a calendar in place.

## Numbers (docs/CONSTANTS.md; a number here is a copy for reading, the ledger is the source)
MOTION_FEEDBACK_FRAMES 1 · MOTION_MAX_MS 260 · TAB_MORPH_MS 220 · TAB_MORPH_DAMPING 0.8 · ROW_COLLAPSE_MS 160 · ROW_INSERT_MS 180 · HOVER_TINT_MS 80 · READER_CROSSFADE_MS 120 · UNDO_RISE_PX 8 · POPOVER_SCALE_FROM 0.96 · POPOVER_MS 160
Row exit (WhooshMotion.rowExit*, ledger rows owed: ROW_EXIT_*, 2026-11-11): FINAL, the owner picked L2 on 2026-11-11 ("L2 a third of the way (Recommended)"): the whole card slides 130 pt left over 150 ms (opacity 1, 0.35 at 45 %, 0; cubic-bezier(.2,.6,.3,1)), the row's height closes over 100 ms starting 60 ms after the key (cubic-bezier(.3,.7,.2,1)), 160 ms in all, nothing longer (under MOTION_MAX_MS). The special case 'a distance of 0 means the row's width' (L3) is built but unused.

## 11. A sliding selection keeps its distance short (owner, 2026-11-07)

Owner's reference: Michael Filipiuk, "If you use a sliding indicator, keep the distance short" (the video in his Downloads). The selection highlight in the sidebar, the split tabs and any list slides from the old row to the new one only when the two are close: at most SLIDE_MAX_ROWS rows apart. Further than that, the old highlight fades out in place and the new one fades in at its destination (HOVER_TINT_MS). A long slide reads as a thing flying across the screen; a short one reads as the same thing moving. The slide follows the system spring, is interruptible, and never gates the next key.
