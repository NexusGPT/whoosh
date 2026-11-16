surface: mac-settings-ai
inherited from docs/DESIGN.md (the app-level pair; named on its inherits: line by the P1 plan, 2026-11-07)
picked-layout: three-pane — owner: "Classic three-pane", 2026-11-07 (inherited: AI: providers, keys, local models, agents, dictation engine, the decision model; the pane spine applies where a pane exists, a sheet or window otherwise) ; re-stated 2026-11-08 03:00 unchanged (the app lock did not move; two false app rounds came from a per-screen note carrying the app-lock token in DESIGN.md)
picked-style:  native-glass — owner: "Native-glass is the best. Probably need to (i) be able to tweak the categories at the top (ii) have ability to connect or configure a local decision model like laya (https://huggingface.co/convaiinnovations/laya) or Jev with the API key and be able to properly configure it (iii) improve the 'account' at the bottom (iv) have a better view of each account on each email in the "all inbox" (this isn't clear) maybe for example deciding on a specific simple icon for each account (from a preloaded account library or an uploaded image + color for each account) etc.", 2026-11-07 (inherited) ; re-stated 2026-11-08 03:00 unchanged (the app lock did not move; two false app rounds came from a per-screen note carrying the app-lock token in DESIGN.md)
layout-alternatives: inherited
layout-axis:         inherited
style-alternatives:  inherited
style-axis:          inherited
motion:        restrained, by the lock: enters with a 120 ms fade; transitions on the system spring 220 ms; hover and focus acknowledge; nothing decorative
boards:        http://127.0.0.1:8792/design/boards/directions/native-glass/v2/index.html (the app-level pick)
journeys:      J5.1-J5.3, J5.8
links-in:      see docs/JOURNEYS.md screen links for mac-settings-ai
links-out:     see docs/JOURNEYS.md screen links for mac-settings-ai
state: picked-on-localhost (inherited); locked-on-build when its first build is measured

picked-layout: SB System Settings window — owner: "SB: System Settings sidebar + cards" (card 15:19, 2026-11-08; boards http://127.0.0.1:8792/design/boards/settings-r1/, 3 alternatives on the axis "how the panes are reached and how each pane is laid out", one blind; a searchable sidebar of panes at the left, grouped cards at the right, an account opening as its own page; the redesign round opened by the owner's words "Your settings popup on its own needs to be fully reworked to be award winning settings of an app on par with Apple Native settings")
picked-style:  SB-S3 System Settings materials — owner: "S3: System Settings materials (Recommended)" (card 15:39, 2026-11-08; boards http://127.0.0.1:8792/design/boards/settings-r2/, 3 looks on the axis "how much glass and how dense", S3 from the blind lane: a see-through sidebar, blue selection, a coloured rounded-square icon with a white symbol per pane, grouped cards with hairline rows, keys on every pane row; lock amendment accepted with the pick: coloured pane icons, ten system hues, inside Settings only)
