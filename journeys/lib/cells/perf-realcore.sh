#!/bin/sh
# round: own-rig
# Manifest cell perf-realcore: checks/realcore-perf.py starts the local mail fake with a 300 ms wait before each SELECT and FETCH, launches ONE rig
# copy with the REAL core itself (scripts/launch-one.sh, hidden), adds the local account, then drives the J/K and Enter/Escape moves of
# perf/interaction.py and judges `.frame`, `.commit` and `.gap` against the ledger (docs/CONSTANTS.md). Owner 2026-11-10 07:2x: "also real life
# performance and lagging when moving in a real inbox is a pain". The check takes the bundle and the rig lock itself: never wrapped in launch-one.
# A load over PERF_LOAD_MAX records the numbers and leaves them NOT JUDGED.
. "$(dirname "$0")/_sum.sh"
[ -n "${WHOOSH_BUNDLE:-}" ] || unjudged "WHOOSH_BUNDLE is not set"
wait_quiet
WHOOSH_RIG_HIDDEN=1 python3 checks/realcore-perf.py "$WHOOSH_BUNDLE" --latency-ms 300 --messages 60 2>&1
