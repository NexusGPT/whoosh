#!/bin/sh
# round: own-rig
# Manifest cell perf-interaction: perf/interaction.py drives one rig on the fixture core (--fixture-core), started here.
. "$(dirname "$0")/_sum.sh"
[ -n "${WHOOSH_BUNDLE:-}" ] || unjudged "WHOOSH_BUNDLE is not set"
wait_quiet
sh scripts/launch-one.sh "$WHOOSH_BUNDLE" -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 perf/interaction.py --bundle "$0"' "$WHOOSH_BUNDLE" 2>&1
