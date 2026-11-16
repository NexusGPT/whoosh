# Surface: mac-sendwave (the send shockwave across every open Whoosh window)

picked-scheme: aurora — owner card 2026-11-08 19:45: "Aurora (Recommended)", picked on the whole-window strips (_captures/sendwave/strip-window-{aurora,halo,frost,spark}-{light,dark}.png). Aurora is the default (SENDWAVE_SCHEME in docs/CONSTANTS.md). Halo, Frost and Spark stay in the code as schemes and are selectable only through the rig's WHOOSH_SENDWAVE_SCHEME switch; no new UI.
picked-reach: the whole app — owner 2026-11-08: "We need the wave ACROSS THE WHOLE SCREEN OF THE APP", "And the WHOLE window not just the mail part of the window", "like the whole app".
picked-pace: SENDWAVE_MS 720, the front scaled to the reach so it arrives at the farthest window corner at the end of the run, the fade in the last 20 % (lead ruling, Cue's default disclosed to the owner, 2026-11-08).

## Objective
Owner, 2026-11-08 12:32: "when sending email and pressing enter we can make a cool animation with shockwave like explained here". PRD "## Send shockwave: a halo spreads from the send button". The send happens on the key's frame; the wave follows on the next display frame, never gates the next action, and is off under reduced motion.
