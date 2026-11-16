# Shared by the manifest cell wrappers. Prints the one summary line every cell owes the round.
# emit N F U  ->  "N checks, F failed, U not judged"
emit() { printf '%s checks, %s failed, %s not judged\n' "$1" "$2" "$3"; }
unjudged() { printf 'NOT JUDGED: %s\n' "$1"; emit 0 0 1; exit 1; }
# wait_quiet: a perf cell is judged only at a load of PERF_LOAD_MAX or less (docs/CONSTANTS.md). The main58 round ran 6 timing cells at loads 34 to 44 (other
# lanes were building) and every one was NOT JUDGED. This waits, before the cell starts its rig, up to QUIET_WAIT_S (default 300 s) for the one-minute load to fall to
# the cap (QUIET_CAP overrides it for a test), then goes on either way: past the wait the cell runs and its own load gate says NOT JUDGED, never red.
wait_quiet() {
  cap=${QUIET_CAP:-$(sed -n 's/^| PERF_LOAD_MAX | \([0-9.]*\) |.*/\1/p' docs/CONSTANTS.md | head -1)}; cap=${cap:-12}
  limit=${QUIET_WAIT_S:-300}; waited=0
  while :; do
    ld=$(sysctl -n vm.loadavg | awk '{ print $2 }' | tr ',' '.')
    if awk -v l="$ld" -v c="$cap" 'BEGIN { exit !(l + 0 <= c + 0) }'; then break; fi
    if [ "$waited" -ge "$limit" ]; then printf 'quiet-wait: load %s still above %s after %s s: the cell runs now and its own gate decides\n' "$ld" "$cap" "$waited"; return 0; fi
    sleep 15; waited=$((waited + 15))
  done
  [ "$waited" -eq 0 ] || printf 'quiet-wait: load %s (cap %s) after %s s\n' "$ld" "$cap" "$waited"
}
