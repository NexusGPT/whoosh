# Surface: mac-first-start (the first-start flow, J9)

objective: on the first launch, teach the app in 8 steps and end with the first account connected (J9, signed 2026-11-09 11:19); a second launch never shows it.
picked-layout: 1a Staged inbox — owner: "Staged inbox with AMAZING smooth animation and animated illustrations that perfectly loops" (card 2026-11-09 11:43; boards docs/design/boards/first-start-r1/, blind lane)
picked-style: 2b Aurora stage, with 2a's real-row try-it on the keys step — LEAD DEFAULT under the owner's "Do everything on your own and choose default" (2026-11-09 14:29); reasons: the owner picked Aurora for the send wave and names dot art as a favourite, the stage carries the looping illustrations the pick asked for, every motion is transform/opacity within 260 ms; 2a is the more literal teacher and lends its real-row try-it; 2c (blind) is the heaviest in light mode. Boards docs/design/boards/first-start-r2/. The owner's word re-opens this pick.
skip-at-step-6: "Open an empty inbox with an Add account button" — owner card 2026-11-09 11:43: steps 7 and 8 are skipped too; the import offer returns in Settings > Accounts.
geometry: the main window's 1280x820 and 960x632 until docs/design/geometry/mac-first-start.json exists (the build writes it from the real add-account sheet size).
welcome-card: 3a Icon hero — owner: "3a Icon hero" (card 2026-11-09 15:02; boards docs/design/boards/first-start-r3/, one blind): the app icon crowns the card, its blue field holding the envelope as liquid glass, ringed by a halo of aurora dots; entrance 550 ms, ambient loop 8 s. Consequence (the informed lane's note): the shipped app icon's glyph is redrawn as the same glass (A86 family).

## Build

built: 2026-11-10, lane firststart (uncompiled round: `swiftc -parse` per file only; nothing has been drawn on a screen yet).
- Host: an overlay of `mac-main` (the `mac-main.overlay` slot), not a window of its own. Code: `Sources/Features/FirstStart/` (FirstStartFeature registers it; FirstStartRoot draws the veils, the stage art, the card, the medallion and the ring; FirstStartModel holds the 8 steps).
- Geometry: `docs/design/geometry/mac-first-start.json` (card, pane, medallion and halo anchors at 1280 x 820 and 960 x 632, written from `FirstStartGeometry`, whose numbers are the boards'). Not yet measured on a running rig.
- Built on the layout, style and welcome card the record above names, consumed from their three boards (outside the repo) word for word; the keys step carries 2a's real-row try-it (three sample rows, J K E H). No pick was made or changed by this build.
- Motion: the card morph and the veil use MOTION_MAX_MS (200 ms), the boards' 240 and 260 ms become that; the step-8 ring uses LAND_AURORA_MS (480 ms, decorative, gates nothing); every loop is 8 s on transform and opacity, frozen on the board's key frame under Reduced Motion, Low Power and the rig's `fixture first-start reduce on`.
- First-launch rule: the flow opens when no account exists and `whoosh.firstStart.finished` (the app's own UserDefaults) is unset; Finish and Skip at step 6 both set it. The core's `first_start` row (DATA-MAP first-start-state) follows with its contract.
- Shown again (A170, owner 2026-11-16): Settings, General `Quick start` > `Show again` (`mac-settings-general.quick-start.show`) and the palette's line `Show the quick start` (`mac-palette.result.show-the-quick-start`) call `FirstStartModel.replay()`: step 1 over the main window, the seen flag untouched; Skip at the account step goes on when an account exists.
- Check: `checks/parity/par-firststart.py` (J9.1 to J9.11). Lane note: `docs/lanes/firststart.md`.
