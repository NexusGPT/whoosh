#!/bin/sh
# round: no-rig
# Manifest cell core-unit: cargo test over the whole Rust workspace (no app is launched).
. "$(dirname "$0")/_sum.sh"
PATH="$HOME/.cargo/bin:$PATH"; export PATH
command -v cargo > /dev/null 2>&1 || unjudged "cargo is not installed on this Mac"
out=$(cd core && cargo test --workspace 2>&1); ec=$?
printf '%s\n' "$out" | grep -E '^(error|test .* FAILED|failures:)' | head -8 | sed 's/^/FAIL /'
n=$(printf '%s\n' "$out" | awk '/^test result:/{p+=$4; f+=$6} END{print p+f+0, f+0}')
set -- $n
if [ "$ec" -ne 0 ] && [ "$2" -eq 0 ]; then printf 'FAIL cargo test exited %s with no failing test (build error)\n' "$ec"; emit "$1" 1 0; exit 1; fi
emit "$1" "$2" 0; [ "$2" -eq 0 ]
