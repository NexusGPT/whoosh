#!/bin/sh
# round: own-rig
# Manifest cell firstrun: checks/firstrun-check.py starts the local mail fakes (60 real-looking messages, a second account with no SPECIAL-USE) and the
# local provider fake, launches ONE hidden rig copy on the REAL core with an empty data folder and transport redirections ONLY (it refuses to run when
# WHOOSH_CORE_CHECK_FLOW, WHOOSH_CORE_CHECK_AI or WHOOSH_CORE_CHECK_IMPORT is set), and walks the owner's first run: add an account, the inbox, bodies kept
# on this Mac, a search for a word only in a body, the built-in labels, Sources, auto-draft off, an agent's model, the calendar state, dictation, a relaunch,
# nothing leaving the Mac, every command under 50 ms (T-25 prints NOT JUDGED above PERF_LOAD_MAX). Owner 2026-11-10: ten faults found on the installed app that no
# hidden check could see. The check takes the bundle and the rig lock itself: never wrapped in launch-one.
. "$(dirname "$0")/_sum.sh"
[ -n "${WHOOSH_BUNDLE:-}" ] || unjudged "WHOOSH_BUNDLE is not set"
wait_quiet
mkdir -p _captures/perf-realcore-r1
WHOOSH_RIG_HIDDEN=1 python3 checks/firstrun-check.py "$WHOOSH_BUNDLE" --log "_captures/perf-realcore-r1/firstrun-${WHOOSH_STAMP:-round}.log" 2>&1
