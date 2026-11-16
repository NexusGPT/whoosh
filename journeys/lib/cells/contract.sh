#!/bin/sh
# round: rig
# Manifest cell contract: the socket-client tests of Tests/Contract (they need the running rig: launch-one hands the socket
# directory as WHOOSH_SOCKET_DIR, and without it they skip), then checks/contract-types-check.py. One summary line for both.
. "$(dirname "$0")/_sum.sh"
. "$(dirname "$0")/_swift.sh"
# a Swift half that is NOT JUDGED ends only itself (it runs in a subshell, `unjudged` calls exit): the python check below still runs, and the cell counts the Swift half as 1 not judged
swsum=$( ( swift_cell Contract; rc=$?; printf 'SWSUM %s %s %s\n' "${SW_N:-0}" "${SW_F:-0}" "$rc" ) 2>&1 ); printf '%s\n' "$swsum" | grep -v '^SWSUM\|checks, .* failed'
set -- $(printf '%s\n' "$swsum" | sed -n 's/^SWSUM //p' | tail -1)
n=${1:-0}; f=${2:-0}; rc=${3:-1}; su=0
printf '%s\n' "$swsum" | grep -q '^NOT JUDGED' && su=1
py=$(python3 checks/contract-types-check.py 2>&1); prc=$?
printf '%s\n' "$py" | grep -E '^ *FAIL' | head -4
set -- $(printf '%s\n' "$py" | grep -E '^[0-9]+ checks?, [0-9]+ failed' | tail -1 | sed -E 's/^([0-9]+) checks?, ([0-9]+) failed(, ([0-9]+) not judged)?.*/\1 \2 \4/')
n=$((n + ${1:-0})); f=$((f + ${2:-0})); u=${3:-0}
[ "$prc" -eq 0 ] || [ "${2:-0}" -gt 0 ] || { f=$((f + 1)); printf 'FAIL contract-types-check exited %s with no summary\n' "$prc"; }
u=$((${u:-0} + su)); emit "$n" "$f" "$u"; [ "$rc" -eq 0 ] && [ "$f" -eq 0 ]
