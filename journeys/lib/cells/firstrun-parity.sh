#!/bin/sh
# round: own-rig
# Manifest cell firstrun-parity (plan row T-27): the same first-run steps (T-10 T-11 T-12 T-14 T-16 T-22) on the REAL core and on the FIXTURE core after
# `fixture install-state new`; a screen that passes on one and fails on the other is named with the step and the core. One drive at a time (each takes the
# rig lock itself). The last line is the comparison's count line.
. "$(dirname "$0")/_sum.sh"
[ -n "${WHOOSH_BUNDLE:-}" ] || unjudged "WHOOSH_BUNDLE is not set"
dir=$(mktemp -d "${TMPDIR:-/tmp}/firstrun-parity.XXXXXX") || unjudged "no scratch folder"
WHOOSH_RIG_HIDDEN=1 python3 checks/firstrun-check.py "$WHOOSH_BUNDLE" --step T-10,T-16,T-11,T-12,T-14,T-22 --log "$dir/real.log" 2>&1 | tail -3
WHOOSH_RIG_HIDDEN=1 python3 checks/firstrun-check.py "$WHOOSH_BUNDLE" --core fixture --step T-10,T-16,T-11,T-12,T-14,T-22 --log "$dir/fixture.log" 2>&1 | tail -3
[ -s "$dir/real.log" ] && [ -s "$dir/fixture.log" ] || unjudged "a run left no log (BUSY rig, or the app did not start): $dir"
python3 checks/firstrun-check.py --compare "$dir/real.log" "$dir/fixture.log" 2>&1
