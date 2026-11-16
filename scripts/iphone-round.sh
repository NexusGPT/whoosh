#!/bin/sh
# EVERY IPHONE CHECK, ONE AFTER ANOTHER, EACH THROUGH THE HIDDEN PHONE RIG, EACH TO ITS OWN LOG.
#
#   WHOOSH_RIG_HIDDEN=1 sh scripts/iphone-round.sh <iPhone .app> <built Mac .app> <out folder>
#   sh scripts/iphone-round.sh --selftest
#
# Phone-only checks run as   sh scripts/iphone-rig.sh <iPhone .app> -- <check>
# Mac-and-phone checks run as sh scripts/iphone-rig.sh --with-mac <Mac .app> <iPhone .app> -- <check>
# (scripts/iphone-rig.sh: no Simulator.app, no `open`, its own device only, one rig at a time; this script starts no second one, it waits for the
# first to end). Each check's whole output goes to <out>/logs/<name>.log (stdout and stderr together); the pictures are <out>/shots/index.html.
# A check that fails does not stop the round: the next one runs. The working folder becomes the repository root.
#
# One line per check:   <name>: exit <n> | <the check's last totals line "N checks, F failed, U not judged", or "no totals line">
# Last line:            round: <n> checks run, <k> not green      (green = exit 0 and a totals line with 0 failed and 0 not judged)
# Exit: 0 when every check is green, 1 otherwise, 6 refused (no WHOOSH_RIG_HIDDEN=1), 2 usage.
set -u
SELF="$0"
case "$SELF" in /*) ;; *) SELF="$(cd "$(dirname "$0")" && pwd -P)/$(basename "$0")" ;; esac
REPO="$(cd "$(dirname "$SELF")/.." && pwd -P)"
say() { printf '%s\n' "$*"; }

WELCOME_JUDGES="welcomephone J4 device-parity-shared J7-shared"   # the checks that judge the welcome flow itself: they start on it
# name | kind (p = phone only, m = Mac and phone, s = Mac and phone on ONE shared loopback mail server, --shared-server) | command (a shell line run by `sh -c` inside the rig)
CHECKS='inboxphone|p|python3 checks/inboxphone-check.py
par-inboxphone|p|python3 checks/parity/par-inboxphone.py
searchphone|p|python3 checks/searchphone-check.py --live
threadphone|p|python3 checks/threadphone-check.py
welcomephone|p|python3 checks/welcomephone-check.py
labelscontactsphone|p|python3 checks/labelscontactsphone-check.py
palettephone|p|python3 checks/palettephone-check.py
composerphone|p|python3 checks/composerphone-check.py
settingsphone|p|python3 checks/settingsphone-check.py
dictationphone|p|python3 checks/dictationphone-check.py --live
phone-shots|p|python3 checks/phone-shots.py "$OUT/shots"
device-parity|m|python3 checks/seams/device-parity.py
account-identity|m|python3 checks/seams/account-identity.py
account-mark|m|python3 checks/seams/account-mark.py
J4|m|python3 journeys/run.py J4
J7|m|WHOOSH_SOCKET_DIR=$WHOOSH_IPHONE_SOCKET_DIR python3 journeys/run.py J7 --themes light,dark
device-parity-shared|s|python3 checks/seams/device-parity.py
J7-shared|s|WHOOSH_SOCKET_DIR=$WHOOSH_IPHONE_SOCKET_DIR python3 journeys/run.py J7 --themes light,dark
memory-iphone|p|python3 perf/memory-iphone.py'

round() {  # round <iPhone app> <Mac app> <out>
  PHONE="$1"; MAC="$2"; OUT="$3"; export OUT
  RIG="${RIG_SCRIPT:-$REPO/scripts/iphone-rig.sh}"
  mkdir -p "$OUT/logs" "$OUT/shots" || return 2
  cd "$REPO" || return 2
  total=0; notgreen=0
  printf '%s\n' "$CHECKS" > "$OUT/logs/checks.list" 2>&1
  while IFS='|' read -r name kind cmd; do
    [ -n "$name" ] || continue
    log="$OUT/logs/$name.log"
    # a test copy of the phone app starts on its welcome (first launch): every check but the ones that judge that flow starts after it has been left
    lw=1; case " $WELCOME_JUDGES " in *" $name "*) lw="" ;; esac
    attempt=0
    while :; do
      attempt=$((attempt+1))
      if [ "$kind" = s ]; then RIG_LEAVE_WELCOME=$lw sh "$RIG" --with-mac --shared-server "$MAC" "$PHONE" -- sh -c "$cmd" < /dev/null > "$log" 2>&1; rc=$?
      elif [ "$kind" = m ]; then RIG_LEAVE_WELCOME=$lw sh "$RIG" --with-mac "$MAC" "$PHONE" -- sh -c "$cmd" < /dev/null > "$log" 2>&1; rc=$?
      else RIG_LEAVE_WELCOME=$lw sh "$RIG" "$PHONE" -- sh -c "$cmd" < /dev/null > "$log" 2>&1; rc=$?; fi
      # the memory check is not judged on a busy machine (its own rule, PERF_LOAD_MAX): wait and ask again, never judge it for it
      if [ "$name" = memory-iphone ] && [ $rc -ne 0 ] && grep -q 'NOT JUDGED: load average' "$log" && [ $attempt -lt "${ROUND_MEM_TRIES:-4}" ]; then
        say "memory-iphone: the machine is busy (attempt $attempt); waiting ${ROUND_MEM_WAIT:-90}s"; sleep "${ROUND_MEM_WAIT:-90}"; continue
      fi
      break
    done
    tot=$(grep -E '^[0-9]+ checks, [0-9]+ failed, [0-9]+ not judged' "$log" | tail -1)
    green=0
    case "$tot" in *" 0 failed, 0 not judged"*) [ $rc -eq 0 ] && green=1 ;; esac
    total=$((total+1)); [ $green -eq 1 ] || notgreen=$((notgreen+1))
    say "$name: exit $rc | ${tot:-no totals line}"
  done < "$OUT/logs/checks.list"
  say "round: $total checks run, $notgreen not green"
  [ $notgreen -eq 0 ]
}

selftest() {
  T=$(mktemp -d "${RIG_SELFTEST_ROOT:-/tmp}/wrdst.XXXXXX") || { say "selftest: no temp folder"; return 2; }
  n=0; bad=0
  ok() { n=$((n+1)); if [ "$2" = 1 ]; then say "PASS $1"; else say "FAIL $1"; bad=$((bad+1)); fi; }
  # a fake rig: logs its arguments, answers with a totals line from the command, and never runs the command
  cat > "$T/fake-rig.sh" <<'FAKERIG'
#!/bin/sh
echo "rig $* [welcome=${RIG_LEAVE_WELCOME:-}]" >> "$FAKE_LOG"
mode=phone; [ "$1" = --with-mac ] && mode=mac
while [ "$1" != "--" ]; do shift; done; shift
cmd="$*"
case "$cmd" in
  *threadphone*) echo "46 checks, 3 failed, 0 not judged"; exit 1 ;;
  *searchphone*) echo "FAIL x" >&2; exit 1 ;;
  *memory-iphone*) if [ "$(grep -c 'memory-iphone' "$FAKE_LOG")" -lt 2 ]; then echo "memory-iphone: NOT JUDGED: load average 198.54 is above PERF_LOAD_MAX 12.0"; echo "1 checks, 0 failed, 1 not judged"; exit 1; fi; echo "1 checks, 0 failed, 0 not judged" ;;
  *palettephone*) echo "9 checks, 0 failed, 0 not judged"; exit 1 ;;
  *account-mark*) echo "12 checks, 0 failed, 2 not judged"; exit 1 ;;
  *) echo "9 checks, 0 failed, 0 not judged ($mode)" ;;
esac
FAKERIG
  : > "$T/log"
  ( FAKE_LOG="$T/log" RIG_SCRIPT="$T/fake-rig.sh" WHOOSH_RIG_HIDDEN=1 ROUND_MEM_WAIT=0 sh "$SELF" /x/Phone.app /x/Mac.app "$T/out" ) > "$T/stdout" 2>&1; rc=$?
  lines=$(grep -c ': exit ' "$T/stdout")
  ok "one-line-per-check (19 lines)" $([ "$lines" = 19 ] && echo 1 || echo 0)
  ok "a-failing-check-does-not-stop-the-round (device-parity, after threadphone, still ran)" $(grep -q '^device-parity: exit 0 | 9 checks, 0 failed, 0 not judged' "$T/stdout" && grep -q '^threadphone: exit 1 | 46 checks, 3 failed, 0 not judged' "$T/stdout" && echo 1 || echo 0)
  ok "no-totals-line-is-said-so-and-is-not-green (searchphone)" $(grep -q '^searchphone: exit 1 | no totals line' "$T/stdout" && echo 1 || echo 0)
  ok "not-judged-is-not-green (account-mark exit 1, 2 not judged)" $(grep -q '^account-mark: exit 1 | 12 checks, 0 failed, 2 not judged' "$T/stdout" && echo 1 || echo 0)
  ok "last-line-counts-and-exit-is-1 (19 run, 4 not green)" $([ "$(tail -1 "$T/stdout")" = "round: 19 checks run, 4 not green" ] && [ $rc = 1 ] && echo 1 || echo 0)
  ok "a-green-totals-line-with-a-failing-exit-is-not-green (palettephone)" $(grep -q '^palettephone: exit 1 | 9 checks, 0 failed, 0 not judged' "$T/stdout" && [ "$(tail -1 "$T/stdout")" = "round: 19 checks run, 4 not green" ] && echo 1 || echo 0)
  ok "memory-check-is-asked-again-when-the-machine-was-busy-and-ends-green (last in the list)" $([ "$(grep -c 'python3 perf/memory-iphone.py' "$T/log")" = 2 ] && grep -q '^memory-iphone: exit 0 | 1 checks, 0 failed, 0 not judged' "$T/stdout" && [ "$(grep ': exit ' "$T/stdout" | tail -1 | cut -d: -f1)" = memory-iphone ] && echo 1 || echo 0)
  ok "leave-welcome-is-asked-for-every-check-except-the-ones-that-judge-the-welcome (welcomephone, J4, the two shared-server runs)" $([ "$(grep -c 'welcome=1\]' "$T/log")" = 16 ] && [ "$(grep -c 'welcome=\]' "$T/log")" = 4 ] && grep 'welcomephone' "$T/log" | grep -q 'welcome=\]' && echo 1 || echo 0)
  ok "the-two-shared-server-checks-run-with-with-mac-and-shared-server" $([ "$(grep -c '^rig --with-mac --shared-server /x/Mac.app /x/Phone.app -- ' "$T/log")" = 2 ] && grep -q 'device-parity.py' "$T/log" && echo 1 || echo 0)
  ok "each-check-has-its-own-log" $([ "$(ls "$T/out/logs"/*.log | wc -l | tr -d ' ')" = 19 ] && echo 1 || echo 0)
  ok "mac-and-phone-checks-use-with-mac-and-phone-checks-do-not" $([ "$(grep -c '^rig --with-mac /x/Mac.app /x/Phone.app -- ' "$T/log")" = 5 ] && [ "$(grep -c '^rig /x/Phone.app -- ' "$T/log")" = 13 ] && echo 1 || echo 0)
  ok "the-shots-check-is-given-the-out-folder" $(grep -q 'phone-shots.py "$OUT/shots"' "$T/log" && echo 1 || echo 0)
  ( FAKE_LOG="$T/log" RIG_SCRIPT="$T/fake-rig.sh" WHOOSH_RIG_HIDDEN= sh "$SELF" /x/Phone.app /x/Mac.app "$T/out2" ) > "$T/stdout2" 2>&1; rc2=$?
  ok "refused-without-WHOOSH_RIG_HIDDEN (exit 6, nothing run)" $([ $rc2 = 6 ] && [ ! -d "$T/out2" ] && echo 1 || echo 0)
  rm -rf "$T"
  say "selftest: $n cases, $bad failed"
  [ $bad -eq 0 ]
}

if [ "${1:-}" = "--selftest" ]; then selftest; exit $?; fi
if [ "${WHOOSH_RIG_HIDDEN:-}" != "1" ]; then
  say "iphone-round: REFUSED: WHOOSH_RIG_HIDDEN=1 is required; no test window may open on the owner's screen (owner rule 2026-11-08 16:30)"; exit 6
fi
if [ $# -ne 3 ]; then say "usage: iphone-round.sh <iPhone .app> <built Mac .app> <out folder> | --selftest"; exit 2; fi
case "$3" in /*) ;; *) say "REFUSED: the out folder must be an absolute path"; exit 2 ;; esac
round "$1" "$2" "$3"
