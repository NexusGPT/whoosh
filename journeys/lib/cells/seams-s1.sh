#!/bin/sh
# round: own-rig
# Manifest cell seams-s1: each slice-1 seam check runs on its OWN fresh rig copy, started by checks/seams/run.py
# through launch-one.sh. That is by design: a seam must not see the actions of the one before it. So the round does
# not wrap this cell in a rig; it hands over the bundle (WHOOSH_BUNDLE) and the queue still holds the line.
. "$(dirname "$0")/_sum.sh"
[ -n "${WHOOSH_BUNDLE:-}" ] || unjudged "WHOOSH_BUNDLE is not set"
out=$(python3 checks/seams/run.py --bundle "$WHOOSH_BUNDLE" --slice 1 2>&1); ec=$?
printf '%s\n' "$out" | grep '^>> ' | grep -v ' exit 0 ' | sed 's/^/FAIL /'
printf '%s\n' "$out" | grep -E '^  (FAIL|NOT JUDGED)' | head -6 | sed 's/^ */FAIL /'   # the reason behind each red seam
n=$(printf '%s\n' "$out" | grep -c '^>> '); f=$(printf '%s\n' "$out" | grep '^>> ' | grep -vc ' exit 0 ')
[ "$n" -gt 0 ] || { printf '%s\n' "$out" | tail -5; emit 0 1 0; exit 1; }
emit "$n" "$f" 0; [ "$f" -eq 0 ]
