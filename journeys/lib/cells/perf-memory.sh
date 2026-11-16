#!/bin/sh
# round: own-rig
# Manifest cell perf-memory: perf/memory-mac.py drives one rig on the fixture core (`fixture load full-history-30` needs --fixture-core)
# and reads the socket from WHOOSH_SOCKET_DIR, so the rig is started here, as perf-interaction does (round main48: the script was run
# bare, "WHOOSH_SOCKET_DIR is not set", not judged).
. "$(dirname "$0")/_sum.sh"
[ -n "${WHOOSH_BUNDLE:-}" ] || unjudged "WHOOSH_BUNDLE is not set"
wait_quiet
sh scripts/launch-one.sh "$WHOOSH_BUNDLE" -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 perf/memory-mac.py --accounts 30 --bundle "$0"' "$WHOOSH_BUNDLE" 2>&1
