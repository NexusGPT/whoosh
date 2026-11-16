#!/bin/sh
# round: own-rig
# Manifest cell seams-pick: each pick-slice seam (account-mark, split-count, decided-by) check runs on its OWN fresh rig copy, started by checks/seams/run.py
# through launch-one.sh. That is by design: a seam must not see the actions of the one before it. So the round does
# not wrap this cell in a rig; it hands over the bundle (WHOOSH_BUNDLE) and the queue still holds the line.
. "$(dirname "$0")/_sum.sh"
[ -n "${WHOOSH_BUNDLE:-}" ] || unjudged "WHOOSH_BUNDLE is not set"
out=$(python3 checks/seams/run.py --bundle "$WHOOSH_BUNDLE" --slice pick 2>&1); ec=$?
printf '%s\n' "$out" | grep '^>> ' | grep -v ' exit 0 ' | sed 's/^/FAIL /'
# the reason behind each red seam: up to 4 lines PER SEAM, named by the seam (a flat `head -6` let account-mark's six lines hide split-count's: main58)
printf '%s\n' "$out" | awk '/^  (FAIL|NOT JUDGED)/ { sub(/^ +/, ""); buf[++k] = $0; next }
  /^>> / { if ($0 !~ / exit 0 /) { for (i = 1; i <= k && i <= 4; i++) print "FAIL [" $2 "] " buf[i]; if (k > 4) print "FAIL [" $2 "] ... " (k - 4) " more" } k = 0 }'
n=$(printf '%s\n' "$out" | grep -c '^>> '); f=$(printf '%s\n' "$out" | grep '^>> ' | grep -vc ' exit 0 ')
[ "$n" -gt 0 ] || { printf '%s\n' "$out" | tail -5; emit 0 1 0; exit 1; }
emit "$n" "$f" 0; [ "$f" -eq 0 ]
