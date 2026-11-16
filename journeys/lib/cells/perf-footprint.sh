#!/bin/sh
# round: own-rig
# Manifest cell perf-footprint: perf/footprint.py launches ONE rig copy itself through launch-one.sh (fixture core) and measures
# launch, idle RSS, idle CPU, RSS after a meaning search and after the unload window, each against its ledger budget (docs/CONSTANTS.md).
# Owner 2026-11-08 14:46: "beware of app speed and heavyness on ram and CPU". A missing weights folder leaves checks 4 and 5 NOT JUDGED.
. "$(dirname "$0")/_sum.sh"
[ -n "${WHOOSH_BUNDLE:-}" ] || unjudged "WHOOSH_BUNDLE is not set"
wait_quiet
WHOOSH_RIG_HIDDEN=1 LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1 WHOOSH_FIXTURE_CORE=1" python3 perf/footprint.py "$WHOOSH_BUNDLE" 2>&1
