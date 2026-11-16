#!/bin/sh
# round: own-rig
# Manifest cell design-build: J1 on the fixture core, started with --fixture-core through launch-one.sh.
. "$(dirname "$0")/_sum.sh"
[ -n "${WHOOSH_BUNDLE:-}" ] || unjudged "WHOOSH_BUNDLE is not set"
sh scripts/launch-one.sh "$WHOOSH_BUNDLE" -- --fixture-core python3 journeys/run.py J1 2>&1
