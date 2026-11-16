#!/bin/sh
# round: own-rig
# Manifest cell perf-launch: the perf script starts its own rig copies from WHOOSH_BUNDLE (timing needs a cold start of its own).
. "$(dirname "$0")/_sum.sh"
[ -n "${WHOOSH_BUNDLE:-}" ] || unjudged "WHOOSH_BUNDLE is not set"
wait_quiet
python3 perf/launch.py --bundle "$WHOOSH_BUNDLE" 2>&1
