#!/bin/sh
# ONE COPY OF THE APP, UNDER THE TEST IDENTITY, FOR ONE COMMAND. NEVER TWO.
#
#   sh scripts/launch-one.sh <bundle> -- <command> [args...]
#   sh scripts/launch-one.sh --selftest
#   sh scripts/launch-one.sh --relaunch            (from INSIDE the driven command: quit the rig copy by its pid, start the SAME copy
#                                                   on the SAME run folder $D; nothing in $D is touched; hidden mode stays; probes-r1 R5)
#
# Step 2 of native-desktop/pipeline.md, under the rules of rig-hygiene.md:
#   · the bundle is COPIED to $RIG_ROOT/run/rig/; the copy's CFBundleIdentifier gains ".rig",
#     LSUIElement is set (no Dock tile, no menu bar), and the copy is re-signed. This script starts
#     the COPY only: the bundle it is handed is never changed, launched in place or killed. Cue
#     drives the installed copy itself, with computer use, outside this script;
#   · one run at a time: a mkdir lock that holds the holder's pid (a dead holder's lock is cleared);
#   · the command runs with $D exported: the run directory, which is also the socket directory;
#   · the instance is stopped by the pid this run captured, never by a name pattern.
#   · no credential is seeded into the rig. A cell that needs a login seeds a test one itself.
#   · ALWAYS HIDDEN (owner 2026-11-08 16:30: "on all tests, EVER, i want you to stop the preview opening EVER"): the
#     app gets $HIDDEN_ENV=1 and is opened in the background (`open -g`), so no window of the copy reaches the screen and
#     it never takes the keyboard (the app's side: Sources/App/HiddenRig.swift). There is no visible mode.
#
# Exit: the command's exit code, or
#   3   NO INSTANCE (the app never started, or exited at once) or NO SOCKET (it published none)
#   4   DIED: the instance was gone before the command finished (a pass over a dead app is not a pass)
#   5   a copy this run did not start is running outside the install folder
#   75  BUSY: the lock stayed held for LAUNCH_BUSY_WAIT seconds
#
# Every run prints one count line:   instances: rig=N owner=N kept=N perf=N
#
# SEVERAL RIGS AT ONCE (the lead's parallel proof run): each rig has its own $RIG_ROOT (its own lock and
# run folders) and its own test identity. RIG_PEER_ROOTS names the OTHER rigs' roots, colon-separated; a
# copy running under one of them is a known test copy ("peer"): counted as peer=N on the count line and
# never refused, never stopped. Unset, nothing changes: any copy outside this rig is refused (exit 5).
#   rig   copies under $RIG_ROOT                 owner  $INSTALL_DIR/$APP_NAME.app
#   kept  $INSTALL_DIR/$APP_NAME <stamp>.app     perf   any other path (a build tree, a download)
#
# Config (the environment, then docs/pipeline.env written as `: "${VAR:=value}"`):
#   APP_NAME            bundle name without .app                          required
#   RIG_ROOT            where the run directory lives        default ${TMPDIR:-/tmp}/$APP_NAME-rig
#   INSTALL_DIR         the owner's install folder                        default /Applications
#   SOCKET_ENV          env var that tells the app its socket directory   default APP_SOCKET_DIR
#   SOCKET_NAME         the socket file the app publishes in $D           default control.sock
#   LAUNCH_ENV          extra "K=V K=V" handed to the app                 default empty
#   SCRUB_ENV_PREFIXES  variable-name prefixes removed before launch (a parent session's markers)
#                                                   default "CLAUDE_CODE_ CLAUDECODE CLAUDE_PID"
#   LAUNCH_MODE         open (Launch Services, the real path) | exec (run the binary; self-test)
#   LAUNCH_WAIT (60) · INSTANCE_WAIT (5) · LAUNCH_BUSY_WAIT (120)   seconds
#   SIGN_IDENTITY       the identity EVERY copy is signed with; "-" is ad hoc   default -
#   HIDDEN_ENV          env var that tells the app to stay hidden   default <APP_NAME in capitals>_RIG_HIDDEN
set -u

# ‼ NO WINDOW ON THE OWNER'S SCREEN, EVER (owner 2026-11-08 16:30: "on all tests, EVER, i want you to
# stop the preview opening EVER. NEVER do it again"). A launch runs ONLY in hidden rig mode: the app
# never orders a window front, takes the accessory activation policy and renders shots off-screen.
# Until the app honours WHOOSH_RIG_HIDDEN (appshell round 3), every launch is refused here.
# Exit 6 = REFUSED: hidden mode not requested or not supported by this bundle.
#
# ONE-RUN VISIBLE DOOR (owner 2026-11-09 09:02, card: "One window now, about a minute"), for the two traffic-light pixel
# checks only (par-sidebar --step J1.14). WHOOSH_VISIBLE_ONCE must hold the owner's go VERBATIM; the bundle must be the
# INSTALLED copy ($HOME/Applications/Whoosh.app, launched in place, which the rig guard leaves alone), not running already;
# the door closes on use (a stamp named by the go under the rig root; a second use is refused). Its preferences are
# exported before the run and imported back after it. Every other launch keeps the hidden rule above, unchanged.
VISIBLE_ONCE_GO="owner 2026-11-09 09:02: One window now, about a minute"
VISIBLE_ONCE_STAMP="$HOME/Library/Application Support/Whoosh-rig/visible-once-owner-2026-11-09-0902.used"
VISIBLE_ONCE_APP="$HOME/Applications/Whoosh.app"
DOOR=0
if [ "${1:-}" != "--selftest" ] && [ -n "${WHOOSH_VISIBLE_ONCE:-}" ]; then
  if [ "$WHOOSH_VISIBLE_ONCE" != "$VISIBLE_ONCE_GO" ]; then
    echo "launch-one: REFUSED: WHOOSH_VISIBLE_ONCE is not the owner's quoted go" >&2; exit 6
  fi
  if [ -e "$VISIBLE_ONCE_STAMP" ]; then
    echo "launch-one: REFUSED: the one visible run was already used ($(cat "$VISIBLE_ONCE_STAMP" 2>&1))" >&2; exit 6
  fi
  if [ "${1%/}" != "$VISIBLE_ONCE_APP" ]; then
    echo "launch-one: REFUSED: the visible door opens only for the installed copy $VISIBLE_ONCE_APP" >&2; exit 6
  fi
  if pgrep -f "$VISIBLE_ONCE_APP/Contents/MacOS/" > /dev/null 2>&1; then
    echo "launch-one: REFUSED: the installed copy is running; quit it first (the door never stops the owner's app)" >&2; exit 6
  fi
  DOOR=1
fi
if [ "${1:-}" != "--selftest" ] && [ $DOOR -eq 0 ]; then
  if [ "${WHOOSH_RIG_HIDDEN:-}" != "1" ]; then
    echo "launch-one: REFUSED: WHOOSH_RIG_HIDDEN=1 is required; no test window may open on the owner's screen (owner rule 2026-11-08 16:30)" >&2
    exit 6
  fi
  _b="${1:-}"; [ "$_b" = "--relaunch" ] && _b=""   # the relaunch step checks the rig copy's own rig-hidden.ok below
  if [ -n "$_b" ] && [ ! -f "$_b/Contents/Resources/rig-hidden.ok" ]; then
    echo "launch-one: REFUSED: $_b does not declare hidden rig support (Contents/Resources/rig-hidden.ok written by the build once the app honours WHOOSH_RIG_HIDDEN)" >&2
    exit 6
  fi
fi

say() { printf '%s\n' "$*"; }

cfg() {
  if [ -f "${PIPELINE_ENV:-docs/pipeline.env}" ]; then . "${PIPELINE_ENV:-docs/pipeline.env}"; fi
  _tmp=${TMPDIR:-/tmp}; _tmp=${_tmp%/}
  : "${RIG_ROOT:=$_tmp/${APP_NAME:-app}-rig}"; RIG_ROOT=${RIG_ROOT%/}
  : "${INSTALL_DIR:=/Applications}"; INSTALL_DIR=${INSTALL_DIR%/}
  : "${SOCKET_ENV:=APP_SOCKET_DIR}"
  : "${SOCKET_NAME:=control.sock}"
  : "${LAUNCH_ENV:=}"
  # A hidden copy reads the search model's download address only from its launch environment (WHOOSH_RIG_EMBED_BASE, Sources/Features/Embedding/MeaningModelRig.swift:30), and a check
  # starts its stub after the launch. WHOOSH_RIG_EMBED_PORT (digits) names the loopback port the stub will take; it becomes the loopback address here, nothing else.
  case "${WHOOSH_RIG_EMBED_PORT:-}" in
    ''|*[!0-9]*) ;;
    *) LAUNCH_ENV="$LAUNCH_ENV WHOOSH_RIG_EMBED_BASE=http://127.0.0.1:$WHOOSH_RIG_EMBED_PORT/" ;;
  esac
  : "${SCRUB_ENV_PREFIXES:=CLAUDE_CODE_ CLAUDECODE CLAUDE_PID}"
  : "${LAUNCH_MODE:=open}"
  : "${LAUNCH_WAIT:=60}"; : "${INSTANCE_WAIT:=5}"; : "${LAUNCH_BUSY_WAIT:=120}"
  : "${SIGN_IDENTITY:=-}"
  : "${HIDDEN_ENV:=$(printf '%s' "${APP_NAME:-app}" | tr '[:lower:]' '[:upper:]')_RIG_HIDDEN}"
}

# "pid class" for every process RUNNING an executable inside a $APP_NAME.app bundle.
#
# ONLY THE FIRST TWO WORDS OF THE COMMAND LINE ARE READ, and that is the whole point: the bundle path
# is the process's own executable (word 1), or the script it was handed (word 2). A process that merely
# MENTIONS the path further along its line is a different thing — a shell running the round, a grep, an
# editor — and counting those as running copies refuses every launch for a copy nobody started.
app_procs() {
  ps -axo pid=,command= | awk -v app="/$APP_NAME.app/Contents/MacOS/" -v rig="$RIG_ROOT/" \
      -v owner="$INSTALL_DIR/$APP_NAME.app/" -v kept="$INSTALL_DIR/$APP_NAME " -v peers="${RIG_PEER_ROOTS:-}" '
    BEGIN { np = split(peers, P, ":") }
    { pid = $1; head = $2; if (NF > 2) head = head " " $3 }
    index(head, app) == 0 { next }
    index(head, rig) > 0   { print pid, "rig";   next }
    { for (i = 1; i <= np; i++) if (P[i] != "" && index(head, P[i] "/") > 0) { print pid, "peer"; next } }
    index(head, owner) > 0 { print pid, "owner"; next }
    index(head, kept) > 0  { print pid, "kept";  next }
    { print pid, "perf" }'
}

instances() {
  app_procs | awk -v showpeer="${RIG_PEER_ROOTS:+1}" '{ c[$2]++ } END { printf "instances: rig=%d owner=%d kept=%d perf=%d", c["rig"], c["owner"], c["kept"], c["perf"]; if (showpeer) printf " peer=%d", c["peer"]; printf "\n" }'
}

alive() {  # a zombie is not alive
  _st=$(ps -o stat= -p "$1" 2>&1) || return 1
  case "$_st" in ''|Z*) return 1 ;; *) return 0 ;; esac
}

# One line per stop (and per stale lock cleared): clock time, this launcher's pid and command, the pid stopped, and the rule that allowed it.
# main112: a live run's copy ended between two journeys and kill.log held only "No such process" lines, with no time, no caller and no reason.
klog() {  # klog <file> <what> <rule>
  printf '%s | launcher pid %s (%s) | %s | rule: %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$$" "$(ps -o command= -p $$ | cut -c1-120)" "$2" "$3" >> "$1"
}

# stop_pid <pid> [rule]: the rule says why this launcher may stop it: own-exit (the copy it started), leftover (its launcher is provably dead), watchdog, relaunch.
# Clearing a stale lock is itself exclusive: two waiters that both judged the lock stale used to both `rm -rf` it, the second removing the lock the first had just taken,
# so two launchers ran at once and the later one's leftover rule stopped the earlier one's live copy. Now one waiter at a time holds `launch.lock.guard`, re-reads the holder
# under it, and moves the lock aside only when it is still the same dead (or still pid-less) lock. A guard older than 30 s is a dead waiter's and is cleared.
lock_takeover() {  # lock_takeover <expected holder pid or ""> <why>
  _g="$LOCK.guard"
  if ! _e=$(mkdir "$_g" 2>&1); then
    _m=$(stat -f %m "$_g" 2>&1) || _m=$(date +%s)
    _age=$(( $(date +%s) - _m ))
    if [ "$_age" -gt 30 ]; then klog "$RIG_ROOT/lock.log" "clears a guard $_age s old" "stale guard"; rm -rf "$_g"; fi
    return 1
  fi
  _now=$(cat "$LOCK/pid" 2>&1); case "$_now" in ''|*[!0-9]*) _now="" ;; esac
  _ok=1
  _dead=1; if [ -n "$_now" ]; then _e=$(kill -0 "$_now" 2>&1) && _dead=0; fi
  if [ "$_now" = "$1" ] && { [ -z "$_now" ] || [ $_dead -eq 1 ]; }; then
    klog "$RIG_ROOT/lock.log" "clears $LOCK" "$2"
    if _e=$(mv "$LOCK" "$LOCK.cleared.$$" 2>&1); then rm -rf "$LOCK.cleared.$$"; _ok=0; else klog "$RIG_ROOT/lock.log" "could not clear $LOCK: $_e" "$2"; fi
  fi
  _e=$(rmdir "$_g" 2>&1)
  return $_ok
}

stop_pid() {
  klog "$RIG_ROOT/kill.log" "stops pid $1 ($(ps -o command= -p "$1" | cut -c1-100))" "${2:-own-exit}"
  kill "$1" >> "$RIG_ROOT/kill.log" 2>&1
  _n=0
  while alive "$1" && [ $_n -lt 10 ]; do sleep 0.3; _n=$((_n+1)); done
  if alive "$1"; then klog "$RIG_ROOT/kill.log" "kill -9 pid $1" "${2:-own-exit} (did not quit)"; kill -9 "$1" >> "$RIG_ROOT/kill.log" 2>&1; fi
}

# Start the rig copy $RIG_APP (hidden, by pid) and wait for its socket. 0 = up; 3 = no instance or no socket. Used once per run;
# LAUNCH_RUNS=2 starts the SAME copy again (see launch_main), which is how a caller measures the second launch of one file.
start_rig() {
  HIDDEN=1; OPEN_BG="-g"   # no visible mode exists
  say "rig mode: hidden"
  if [ "$LAUNCH_MODE" = exec ]; then
    ( export "$SOCKET_ENV=$D" "XDG_CONFIG_HOME=$D/xdg" "$HIDDEN_ENV=$HIDDEN"
      for kv in $LAUNCH_ENV; do export "$kv"; done
      exec "$RIG_APP/Contents/MacOS/$EXE" $APP_ARGS ) > "$D/app.out" 2>&1 &
    BG=$!
  else
    ( set -- open -n -W $OPEN_BG "$RIG_APP" --stdout "$D/app.out" --stderr "$D/app.out" --env "$SOCKET_ENV=$D" --env "XDG_CONFIG_HOME=$D/xdg" --env "$HIDDEN_ENV=$HIDDEN"
      for kv in $LAUNCH_ENV; do set -- "$@" --env "$kv"; done
      [ -n "$APP_ARGS" ] && set -- "$@" --args $APP_ARGS
      exec "$@" ) > "$D/open.out" 2>&1 &
    BG=""
  fi

  # THE PID WE STARTED BEATS A SCAN. In exec mode the background pid IS the app, so a slow start is
  # never read as a missing instance, and an app that exits at once is caught at once. Launch Services
  # hands the app to launchd, so `open` mode has no such pid and the scan is the only instrument.
  if [ -n "$BG" ]; then
    # A pid is not in the process table the instant the fork returns, so "not there yet" is not "gone":
    # the only proof this child ENDED is a zombie state. Measured: reading the state with no wait at all
    # made a healthy app read as dead about half the time.
    i=0; APP=""; _gone=0
    while [ $i -lt $((INSTANCE_WAIT * 5)) ]; do
      st=$(ps -o stat= -p "$BG" 2>&1)
      case "$st" in
        # A zombie is an exit. An EMPTY state is ambiguous — a pid is not in the table the
        # instant the fork returns, and an exited child can also read empty before it is
        # reaped — so it takes three polls (0.6 s) before it counts as gone, which keeps the
        # exited-at-once case fast without reading a healthy slow start as a death.
        '') _gone=$((_gone+1)); [ $_gone -ge 3 ] && { APP=""; break; } ;;
        Z*) APP=""; break ;;
        *) APP="$BG"; break ;;
      esac
      sleep 0.2; i=$((i+1))
    done
  else
    i=0
    while [ $i -lt $((INSTANCE_WAIT * 5)) ]; do
      APP=$(app_procs | awk '$2=="rig"{print $1; exit}')
      if [ -n "$APP" ] && alive "$APP"; then break; fi
      APP=""; sleep 0.2; i=$((i+1))
    done
  fi
  if [ -z "$APP" ]; then
    say "NO INSTANCE: $RIG_APP did not start, or exited at once, within ${INSTANCE_WAIT}s"
    tail -5 "$D/app.out" 2>&1; instances; return 3
  fi
  i=0
  while [ ! -S "$D/$SOCKET_NAME" ] && [ $i -lt $((LAUNCH_WAIT * 2)) ]; do
    alive "$APP" || break
    sleep 0.5; i=$((i+1))
  done
  i=$((i / 2))
  if [ ! -S "$D/$SOCKET_NAME" ]; then
    if alive "$APP"; then say "NO SOCKET: the instance (pid $APP) published no $SOCKET_NAME in ${i}s"; stop_pid "$APP"
    else say "NO INSTANCE: the instance (pid $APP) exited before it published $SOCKET_NAME"; tail -5 "$D/app.out" 2>&1; fi
    instances; return 3
  fi
  say "instance pid $APP, socket $D/$SOCKET_NAME"
  printf '%s (while the command runs)\n' "$(instances)"
  return 0
}

# Everything --relaunch needs to start the same copy again, written where the driven command can find it ($D is exported to it).
q() { printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"; }
save_relaunch_env() {
  { for v in RIG_APP EXE APP_ARGS LAUNCH_ENV RIG_ROOT SOCKET_ENV SOCKET_NAME HIDDEN_ENV APP_NAME LAUNCH_MODE LAUNCH_WAIT INSTANCE_WAIT INSTALL_DIR; do
      eval "_set=\${$v+x}; _val=\${$v:-}"
      [ -n "$_set" ] && printf '%s=%s\n' "$v" "$(q "$_val")"
    done; true; } > "$D/relaunch.env"
}

# `--relaunch`, run from inside the driven command: the rig copy is quit BY ITS PID (the one in $D/rig.pid) and the same copy starts
# again on the same run folder, so a check can prove what survives a restart. $D is not cleaned, the preferences and Keychain clean of
# a first start is not repeated, and the hidden rule stays: WHOOSH_RIG_HIDDEN=1 is required (and the top of this file checks it first).
relaunch_main() {
  [ -n "${D:-}" ] && [ -f "$D/relaunch.env" ] || { say "REFUSED: --relaunch runs inside a driven command (no \$D/relaunch.env)"; return 2; }
  . "$D/relaunch.env"
  [ "${WHOOSH_RIG_HIDDEN:-}" = "1" ] || { say "REFUSED: WHOOSH_RIG_HIDDEN=1 is required"; return 6; }   # the copy was admitted at its launch; the rule is not relaxed here
  old=$(cat "$D/rig.pid" 2>&1)
  : > "$D/relaunching"
  alive "$old" && { say "relaunch: quitting pid $old"; stop_pid "$old"; }
  rm -f "$D/died" "$D/$SOCKET_NAME"
  start_rig; rc=$?
  [ $rc -eq 0 ] && echo "$APP" > "$D/rig.pid"
  rm -f "$D/relaunching"
  return $rc
}

launch_main() {
  cfg
  BUNDLE="${1:-}"
  if [ "${2:-}" != "--" ] || [ $# -lt 3 ]; then say "usage: launch-one.sh <bundle> -- <command> [args...]"; return 2; fi
  shift 2
  # Leading words that start with "--" are the APP's launch flags (e.g. --fixture-core), handed to the
  # app; the first other word starts the command. A command that is not a program on PATH or a path is
  # a control-socket verb (docs/CONTROL-SOCKET.md) and runs through the app's own client, tools/whoosh-ctl.py.
  APP_ARGS=""
  while [ $# -gt 0 ]; do
    case "$1" in --*) APP_ARGS="$APP_ARGS $1"; shift ;; *) break ;; esac
  done
  if [ $# -lt 1 ]; then say "usage: launch-one.sh <bundle> -- [--app-flag ...] <command> [args...]"; return 2; fi
  case "$1" in
    */*) ;;
    *) if ! command -v "$1" > /dev/null 2>&1; then
         set -- python3 "$(cd "$(dirname "$0")/.." && pwd -P)/tools/whoosh-ctl.py" "$@"
       fi ;;
  esac
  if [ -z "${APP_NAME:-}" ]; then say "REFUSED: APP_NAME is not set"; return 2; fi
  if [ ! -f "$BUNDLE/Contents/Info.plist" ]; then say "REFUSED: no bundle at $BUNDLE"; return 2; fi
  mkdir -p "$RIG_ROOT" || return 2

  LOCK="$RIG_ROOT/launch.lock"; w=0
  while :; do
    err=$(mkdir "$LOCK" 2>&1) && break
    hp=$(cat "$LOCK/pid" 2>&1); case "$hp" in ''|*[!0-9]*) hp="" ;; esac
    _alive=0; if [ -n "$hp" ]; then _e=$(kill -0 "$hp" 2>&1) && _alive=1; fi
    if [ -n "$hp" ] && [ $_alive -eq 0 ]; then
      lock_takeover "$hp" "stale lock: holder pid $hp is dead" && continue
    fi
    if [ -z "$hp" ] && [ $w -ge 20 ]; then lock_takeover "" "a lock with no holder pid for ${w}s" && continue; fi
    if [ $w -ge "$LAUNCH_BUSY_WAIT" ]; then say "BUSY: another run holds $LOCK (pid ${hp:-unknown}) after ${w}s"; return 75; fi
    sleep 2; w=$((w+2))
  done
  echo $$ > "$LOCK/pid"
  APP=""
  trap 'rm -rf "$LOCK"' EXIT
  trap '[ -n "$APP" ] && stop_pid "$APP" "signal"; exit 130' INT TERM

  # Rig copies left by a run whose lock is gone belong to this rig: stop them by pid.
  # Only a copy whose launcher is provably gone (the pid in its run folder name `run-<launcher pid>` is dead): a copy whose launcher is alive belongs to that launcher and is
  # never stopped here (it then makes the check below REFUSE this run, rc 5).
  for p in $(app_procs | awk '$2=="rig"{print $1}'); do
    _cmd=$(ps -o command= -p "$p"); _lp=$(printf '%s' "$_cmd" | sed -n 's|.*/run-\([0-9][0-9]*\)/.*|\1|p')
    if [ -n "$_lp" ] && [ "$_lp" != "$$" ] && alive "$_lp"; then
      say "rig copy pid $p belongs to the live launcher pid $_lp: left alone"; klog "$RIG_ROOT/kill.log" "leaves pid $p alone" "its launcher pid $_lp is alive"; continue
    fi
    say "leftover rig copy pid $p from a run that ended: stopping it"; stop_pid "$p" "leftover (launcher ${_lp:-unknown} is gone)"
  done
  foreign=$(app_procs | awk '$2=="rig" || $2=="perf"')
  if [ -n "$foreign" ]; then
    say "REFUSED: a copy this run did not start is running:"
    printf '%s\n' "$foreign" | while read -r p c; do say "  pid $p ($c) $(ps -o command= -p "$p" | cut -c1-160)"; done
    instances; return 5
  fi

  # EACH RUN GETS ITS OWN DIRECTORY. Re-using one `run/` directory means deleting it and
  # re-filling it while the copy that just exited may still hold file handles in it; driven in
  # this script's own self-test, the launch right after a failed one then started an app that
  # exited with no output at all. A run directory named by the pid cannot collide, and the
  # sweep below removes the ones whose run has ended.
  for _old in "$RIG_ROOT"/run-*; do
    [ -d "$_old" ] || continue
    _op=${_old##*/run-}
    case "$_op" in ''|*[!0-9]*) rm -rf "$_old"; continue ;; esac
    kill -0 "$_op" >> "$RIG_ROOT/kill.log" 2>&1 || rm -rf "$_old"
  done
  D="$RIG_ROOT/run-$$"
  rm -rf "$D"; mkdir -p "$D/rig" "$D/xdg" || return 2
  if [ $DOOR -eq 1 ]; then door_launch "$@"; return $?; fi
  RIG_APP="$D/rig/$APP_NAME.app"
  ditto "$BUNDLE" "$RIG_APP" > "$D/copy.log" 2>&1 || { say "REFUSED: could not copy $BUNDLE"; cat "$D/copy.log"; return 2; }
  PL="$RIG_APP/Contents/Info.plist"
  ID=$(plutil -extract CFBundleIdentifier raw -o - "$PL" 2>&1) || { say "REFUSED: the bundle has no CFBundleIdentifier"; return 2; }
  case "$ID" in
    *.rig) ;;
    *) plutil -replace CFBundleIdentifier -string "$ID.rig" "$PL" > "$D/plist.log" 2>&1 || { say "REFUSED: could not set the test identity"; return 2; } ;;
  esac
  # A TEST COPY STARTS CLEAN. The rig identity keeps its preferences between runs (measured: a check read a
  # switch as 0 then 1 within a minute; tests-r48). Every key goes, except AppleLanguages, which the job
  # sets on purpose before the checks (main-job: `defaults write io.nexus.whoosh.rig AppleLanguages`).
  # (main50/51: the clean wrote `en` back whatever was set, so the French i18n run read English: 16 "leaks". The
  # languages are read as JSON and written back as they were.) The same for `whoosh.language`, the picked language
  # (main52: the picked i18n run wrote it, the clean wiped it, 16 English values stayed with fr "picked").
  _pre=$(defaults export "$ID.rig" - 2>> "$D/prefs-clean.log" | python3 -c 'import plistlib,sys,shlex
try:
    d=plistlib.loads(sys.stdin.buffer.read())
    print(" ".join(shlex.quote(x) for x in d.get("AppleLanguages",[]) if isinstance(x,str)))
    print(shlex.quote(d.get("whoosh.language","")) if isinstance(d.get("whoosh.language",""),str) else "")
except Exception: print(); print()' 2>> "$D/prefs-clean.log")
  langs=$(printf '%s\n' "$_pre" | sed -n 1p); pick=$(printf '%s\n' "$_pre" | sed -n 2p)
  defaults delete "$ID.rig" >> "$D/prefs-clean.log" 2>&1 || true
  if [ -n "$langs" ]; then eval "defaults write \"\$ID.rig\" AppleLanguages -array $langs" >> "$D/prefs-clean.log" 2>&1; fi
  if [ -n "$pick" ] && [ "$pick" != "''" ]; then eval "defaults write \"\$ID.rig\" whoosh.language -string $pick" >> "$D/prefs-clean.log" 2>&1; fi
  # AND WITH NO KEYCHAIN ITEM OF AN EARLIER COPY. Every rig copy is re-signed ad hoc, so an item an earlier copy
  # wrote is another app's to macOS: the next copy's read raises a permission prompt no hidden window shows, and
  # the sign-in slice idled for an hour (2026-11-09 20:10 to 21:30, 7 of 8 red on two bundles; green once the
  # items were deleted). The rig namespace holds test data only.
  for svc in "$ID.rig.account-credential" "rig.whoosh.device-link"; do
    k=0; while security delete-generic-password -s "$svc" >> "$D/prefs-clean.log" 2>&1; do k=$((k+1)); [ $k -ge 200 ] && break; done
  done

  plutil -replace LSUIElement -bool true "$PL" >> "$D/plist.log" 2>&1 || { say "REFUSED: could not set LSUIElement"; return 2; }
  xattr -cr "$RIG_APP" > "$D/xattr.log" 2>&1
  codesign --force --deep --sign "$SIGN_IDENTITY" "$RIG_APP" > "$D/codesign.log" 2>&1 \
    || { say "REFUSED: could not re-sign the rig copy"; cat "$D/codesign.log"; return 2; }
  EXE=$(plutil -extract CFBundleExecutable raw -o - "$PL" 2>&1) || EXE=""
  if [ -z "$EXE" ] || [ ! -x "$RIG_APP/Contents/MacOS/$EXE" ]; then
    say "REFUSED: the bundle names executable '$EXE' and Contents/MacOS holds no such executable"; return 2
  fi
  say "rig copy: $RIG_APP id=$(plutil -extract CFBundleIdentifier raw -o - "$PL" 2>&1)"

  for pre in $SCRUB_ENV_PREFIXES; do
    for v in $(env | awk -F= -v p="$pre" 'index($1, p) == 1 { print $1 }'); do unset "$v"; done
  done

  RUNS=${LAUNCH_RUNS:-1}; case "$RUNS" in 1|2) ;; *) say "REFUSED: LAUNCH_RUNS must be 1 or 2"; return 2 ;; esac
  n=1; rc=0
  while :; do
    rm -f "$D/died" "$D/$SOCKET_NAME"
    start_rig || return $?
  # The driven command gets the socket directory under both names: $D, and the app's own variable
  # ($SOCKET_ENV), which the app's clients and checks read.
  export D
  export "$SOCKET_ENV=$D"
  RIG_PID=$APP; RIG_HIDDEN=$HIDDEN; export RIG_PID RIG_HIDDEN   # the hiddenness check reads the window server for this pid
  echo "$APP" > "$D/rig.pid"
  save_relaunch_env
  ( while :; do
      sleep 1
      [ -e "$D/relaunching" ] && continue          # --relaunch is replacing the instance
      _cur=$(cat "$D/rig.pid" 2>&1)
      alive "$_cur" || { : > "$D/died"; say "NOTE: the instance (pid $_cur) has gone; the command is talking to nothing."; break; }
    done ) &
  WATCH=$!
  [ "$RUNS" -gt 1 ] && say "launch-one: run $n of $RUNS (same copy $RIG_APP, pid $APP)"
  "$@"; rc=$?
  kill "$WATCH" >> "$RIG_ROOT/kill.log" 2>&1

  APP=$(cat "$D/rig.pid" 2>&1)   # a --relaunch inside the command changed it
  died=0
  if [ -f "$D/died" ] || ! alive "$APP"; then died=1; fi
  # a command that stops the instance itself and names the exact pid it stopped (checks/ownercopy-search.py deletes the snapshot the copy runs on)
  # is not a death; a copy that dies while the command runs still leaves the watcher's `died` file and stays one
  if [ $died -eq 1 ] && [ ! -f "$D/died" ] && [ -f "$D/stopped-by-command" ] && [ "$(cat "$D/stopped-by-command")" = "$APP" ]; then died=0; say "the command stopped the instance (pid $APP) itself, as it announced"; fi
  if alive "$APP"; then stop_pid "$APP"; fi
  if [ "$LAUNCH_MODE" = exec ]; then wait "$APP" >> "$RIG_ROOT/kill.log" 2>&1; fi
  # LAUNCH_RUNS=2: when run 1 ended well, the instance is gone (stopped by its pid above) and the SAME copy starts again inside this lock hold.
  [ $n -lt "$RUNS" ] && [ $died -eq 0 ] && [ $rc -eq 0 ] || break
  n=$((n+1))
  done
  for p in $(app_procs | awk '$2=="rig"{print $1}'); do stop_pid "$p"; done
  instances
  [ -n "${KEEP_RUN_DIR:-}" ] || rm -rf "$D"
  if [ $died -eq 1 ] && [ $rc -eq 0 ]; then
    say "DIED: the instance was gone before the command finished; a pass over a dead app is not a pass"; rc=4
  fi
  return $rc
}

# The one visible run: the installed copy in place, its pid captured (exec), visible, quit by that pid; its preferences restored.
door_launch() {
  PL="$BUNDLE/Contents/Info.plist"
  ID=$(plutil -extract CFBundleIdentifier raw -o - "$PL" 2>&1) || { say "REFUSED: the bundle has no CFBundleIdentifier"; return 2; }
  EXE=$(plutil -extract CFBundleExecutable raw -o - "$PL" 2>&1) || EXE=""
  [ -x "$BUNDLE/Contents/MacOS/$EXE" ] || { say "REFUSED: no executable '$EXE' in $BUNDLE"; return 2; }
  printf '%s used by pid %s at %s\n' "$VISIBLE_ONCE_GO" "$$" "$(date '+%Y-%m-%d %H:%M:%S')" > "$VISIBLE_ONCE_STAMP" || return 2
  say "VISIBLE ONCE: the owner's go, quoted: \"$VISIBLE_ONCE_GO\" (door closed by $VISIBLE_ONCE_STAMP)"
  defaults export "$ID" "$D/owner-defaults.plist" > "$D/defaults.log" 2>&1 || { say "REFUSED: could not save the installed copy's preferences"; cat "$D/defaults.log"; return 2; }
  say "rig mode: VISIBLE (one run), installed copy in place: $BUNDLE id=$ID"
  ( export "$SOCKET_ENV=$D" "XDG_CONFIG_HOME=$D/xdg"
    for kv in $LAUNCH_ENV; do export "$kv"; done
    exec "$BUNDLE/Contents/MacOS/$EXE" $APP_ARGS ) > "$D/app.out" 2>&1 &
  APP=$!
  i=0
  while [ ! -S "$D/$SOCKET_NAME" ] && [ $i -lt $((LAUNCH_WAIT * 2)) ]; do alive "$APP" || break; sleep 0.5; i=$((i+1)); done
  if [ ! -S "$D/$SOCKET_NAME" ]; then
    say "NO SOCKET: the installed copy (pid $APP) published no $SOCKET_NAME"; alive "$APP" && stop_pid "$APP"
    defaults import "$ID" "$D/owner-defaults.plist" >> "$D/defaults.log" 2>&1; return 3
  fi
  say "instance pid $APP, socket $D/$SOCKET_NAME"
  export D "$SOCKET_ENV=$D"
  RIG_PID=$APP; RIG_HIDDEN=0; export RIG_PID RIG_HIDDEN
  "$@"; rc=$?
  alive "$APP" && stop_pid "$APP"
  wait "$APP" >> "$RIG_ROOT/kill.log" 2>&1
  defaults import "$ID" "$D/owner-defaults.plist" >> "$D/defaults.log" 2>&1 \
    && say "the installed copy's preferences restored" || { say "WARNING: could not restore the installed copy's preferences from $D/owner-defaults.plist"; KEEP_RUN_DIR=1; }
  instances
  [ -n "${KEEP_RUN_DIR:-}" ] || rm -rf "$D"
  return $rc
}

selftest() {
  SELF=$(cd "$(dirname "$0")" && pwd -P)/$(basename "$0"); export SELF
  ST=$(mktemp -d /tmp/nx-launch-st.XXXXXX) || return 2
  ST_N=0; ST_F=0
  ok() { ST_N=$((ST_N+1))
         if [ "$1" -eq 0 ]; then say "  PASS  $2"
         else ST_F=$((ST_F+1)); say "  FAIL  $2"; [ -n "${3:-}" ] && printf '%s\n' "$3" | sed 's/^/        | /'; fi; }
  mkapp() {  # mkapp <dir> <executable file to copy>
    # ‼ THE STAND-IN IS SIGNED AND ITS PROVENANCE STRIPPED, AND THAT IS NOT CEREMONY.
    # An UNSIGNED .app assembled by an agent carries `com.apple.provenance`; macOS assesses the
    # bundle when its executable runs and puts "Fake is damaged and can't be opened" ON THE
    # OWNER'S SCREEN — measured twice in one night, reported by another session, from this very
    # self-test. The rig never reaches the owner's screen (rig-hygiene.md rule 1), and that binds
    # the test harness exactly as it binds a lane. It also explains a flake: Gatekeeper was
    # killing the stand-in, and the launcher read the death as "no instance".
    _b="$1/Fake.app/Contents"; mkdir -p "$_b/MacOS"
    plutil -create xml1 "$_b/Info.plist" > "$ST/plutil.log" 2>&1
    plutil -insert CFBundleIdentifier -string com.example.fake "$_b/Info.plist" >> "$ST/plutil.log" 2>&1
    plutil -insert CFBundleExecutable -string Fake "$_b/Info.plist" >> "$ST/plutil.log" 2>&1
    cp "$2" "$_b/MacOS/Fake"; chmod +x "$_b/MacOS/Fake"
    xattr -cr "$1/Fake.app" >> "$ST/xattr.log" 2>&1
    codesign --force --deep --sign - "$1/Fake.app" >> "$ST/codesign.log" 2>&1 \
      || { say "  REFUSED to run: the stand-in bundle could not be signed, and an unsigned one "; \
           say "  reaches the owner's screen. See $ST/codesign.log"; return 2; }
  }
  # The stand-in app IS the process the bundle names — a python script with a shebang, not a
  # shell wrapper that starts one. A wrapper made the app a `/bin/sh` whose child held the
  # socket, and the pid the launcher watched and the pid doing the work were different things.
  cat > "$ST/fake.sh" <<'FAKE'
#!/usr/bin/env python3
"""A stand-in desktop app: publish the control socket, then stay up."""
import os, socket, sys, time
d = os.environ.get("APP_SOCKET_DIR")
if not d:
    sys.stderr.write("APP_SOCKET_DIR is not set\n")
    raise SystemExit(2)
s = socket.socket(socket.AF_UNIX)
s.bind(os.path.join(d, "control.sock"))
s.listen(1)
open(os.path.join(d, "env-hidden"), "w").write(os.environ.get("FAKE_RIG_HIDDEN", "unset"))
open(os.path.join(d, "embed-base"), "w").write(os.environ.get("WHOOSH_RIG_EMBED_BASE", "unset"))
sys.stdout.write("socket up at %s\n" % d)
sys.stdout.flush()
time.sleep(float(os.environ.get("FAKE_LIFE", "60")))
FAKE
  # SIGN_IDENTITY is the ad-hoc "-": the rig COPY is signed the way every copy is.
  export APP_NAME=Fake RIG_ROOT="$ST/rig" INSTALL_DIR="$ST/Applications" LAUNCH_MODE=exec SIGN_IDENTITY=- \
         INSTANCE_WAIT=8 LAUNCH_WAIT=20 LAUNCH_BUSY_WAIT=2 PIPELINE_ENV="$ST/no-such-env"
  mkdir -p "$INSTALL_DIR"
  mkapp "$ST/b-false" /usr/bin/false
  mkapp "$ST/b-good" "$ST/fake.sh"

  t0=$(date +%s); out=$(launch_main "$ST/b-false/Fake.app" -- true 2>&1); rc=$?; t1=$(date +%s)
  [ $rc -eq 3 ] && printf '%s' "$out" | grep -q "NO INSTANCE" && [ $((t1 - t0)) -le 5 ]
  ok $? "an app that exits at once -> NO INSTANCE, rc=3, within 5s (rc=$rc, $((t1 - t0))s)" "$out"

  out=$(KEEP_RUN_DIR=1 launch_main "$ST/b-good/Fake.app" -- sh -c 'test -S "$D/control.sock"' 2>&1); rc=$?
  [ $rc -eq 0 ] && printf '%s' "$out" | grep -q "instances: rig=0 owner=0 kept=0 perf=0"
  ok $? "a good app -> the command sees the socket and no copy is left after (rc=$rc)" "$out"
  _rigpl=$(printf '%s\n' "$out" | sed -n 's/^rig copy: \(.*\)\.app id=.*/\1.app/p' | head -1)/Contents/Info.plist
  id=$(plutil -extract CFBundleIdentifier raw -o - "$_rigpl" 2>&1)
  ui=$(plutil -extract LSUIElement raw -o - "$_rigpl" 2>&1)
  [ "$id" = com.example.fake.rig ] && [ "$ui" = true ]; ok $? "the rig copy carries the test identity (id=$id LSUIElement=$ui)"
  sid=$(plutil -extract CFBundleIdentifier raw -o - "$ST/b-good/Fake.app/Contents/Info.plist" 2>&1)
  [ "$sid" = com.example.fake ]; ok $? "the source bundle keeps its own identity ($sid)"

  out=$(launch_main "$ST/b-good/Fake.app" -- sh -c 'test -n "$APP_SOCKET_DIR" && test "$APP_SOCKET_DIR" = "$D"' 2>&1); rc=$?
  [ $rc -eq 0 ]; ok $? "the command gets the socket directory under SOCKET_ENV too, equal to \$D (rc=$rc)" "$out"

  out=$(launch_main "$ST/b-good/Fake.app" -- sh -c 'test "$(cat "$D/env-hidden")" = 1 && test "$RIG_HIDDEN" = 1 && test -n "$RIG_PID"' 2>&1); rc=$?
  [ $rc -eq 0 ] && printf '%s' "$out" | grep -q "rig mode: hidden"; ok $? "by default the app is told to stay hidden and the command gets RIG_PID (rc=$rc)" "$out"
  out=$(RIG_VISIBLE=1 WHOOSH_RIG_VISIBLE=1 launch_main "$ST/b-good/Fake.app" -- sh -c 'test "$(cat "$D/env-hidden")" = 1 && test "$RIG_HIDDEN" = 1' 2>&1); rc=$?
  [ $rc -eq 0 ] && printf '%s' "$out" | grep -q "rig mode: hidden"; ok $? "no variable makes the copy visible (RIG_VISIBLE=1, WHOOSH_RIG_VISIBLE=1 ignored) (rc=$rc)" "$out"

  out=$(LAUNCH_RUNS=2 launch_main "$ST/b-good/Fake.app" -- sh -c 'echo "pid-seen $RIG_PID"; test -S "$D/control.sock"' 2>&1); rc=$?
  _p1=$(printf '%s\n' "$out" | sed -n 's/^pid-seen //p' | sed -n 1p); _p2=$(printf '%s\n' "$out" | sed -n 's/^pid-seen //p' | sed -n 2p)
  _nc=$(printf '%s\n' "$out" | grep -c '^rig copy: ')
  [ $rc -eq 0 ] && [ -n "$_p1" ] && [ -n "$_p2" ] && [ "$_p1" != "$_p2" ] && [ "$_nc" -eq 1 ] && printf '%s' "$out" | grep -q "run 2 of 2" \
    && printf '%s' "$out" | grep -q "instances: rig=0 owner=0 kept=0 perf=0"
  ok $? "LAUNCH_RUNS=2 -> the command runs twice against two instances of ONE rig copy, none left after (pids $_p1 $_p2, copies $_nc, rc=$rc)" "$out"
  out=$(LAUNCH_RUNS=3 launch_main "$ST/b-good/Fake.app" -- true 2>&1); rc=$?
  [ $rc -eq 2 ]; ok $? "LAUNCH_RUNS=3 -> refused rc=2 (rc=$rc)" "$out"

  out=$(launch_main "$ST/b-good/Fake.app" -- sh -c 'exit 7' 2>&1); rc=$?
  [ $rc -eq 7 ]; ok $? "the command's exit code is the script's exit code (rc=$rc)" "$out"

  out=$( ( export WHOOSH_RIG_EMBED_PORT=18793; launch_main "$ST/b-good/Fake.app" -- sh -c 'cat "$D/embed-base"' ) 2>&1 )
  printf '%s' "$out" | grep -q "http://127.0.0.1:18793/"; ok $? "WHOOSH_RIG_EMBED_PORT=18793 -> the copy is launched with WHOOSH_RIG_EMBED_BASE=http://127.0.0.1:18793/" "$out"
  out=$( ( export WHOOSH_RIG_EMBED_PORT='8080/@evil.example'; launch_main "$ST/b-good/Fake.app" -- sh -c 'cat "$D/embed-base"' ) 2>&1 )
  printf '%s' "$out" | grep -q "evil"; [ $? -ne 0 ]; ok $? "a port that is not digits gives the copy no WHOOSH_RIG_EMBED_BASE" "$out"

  export LAUNCH_ENV="FAKE_LIFE=2"
  out=$(launch_main "$ST/b-good/Fake.app" -- sleep 6 2>&1); rc=$?
  unset LAUNCH_ENV
  [ $rc -eq 4 ]; ok $? "the app dies while the command runs -> DIED rc=4 (rc=$rc)" "$out"

  mkapp "$INSTALL_DIR" "$ST/fake.sh"; mkdir -p "$ST/ownsock"
  ( export APP_SOCKET_DIR="$ST/ownsock" FAKE_LIFE=30; exec "$INSTALL_DIR/Fake.app/Contents/MacOS/Fake" ) > "$ST/owner.out" 2>&1 &
  OWNER=$!; sleep 1
  out=$(launch_main "$ST/b-good/Fake.app" -- true 2>&1); rc=$?
  alive "$OWNER"; owner_alive=$?
  [ $rc -eq 0 ] && printf '%s' "$out" | grep -q "owner=1" && [ $owner_alive -eq 0 ]
  ok $? "a running owner copy -> counted owner=1 and never stopped (rc=$rc)" "$out"
  kill "$OWNER" >> "$ST/kill.log" 2>&1; wait "$OWNER" >> "$ST/kill.log" 2>&1

  mkapp "$ST/elsewhere" "$ST/fake.sh"; mkdir -p "$ST/othsock"
  ( export APP_SOCKET_DIR="$ST/othsock" FAKE_LIFE=30; exec "$ST/elsewhere/Fake.app/Contents/MacOS/Fake" ) > "$ST/other.out" 2>&1 &
  OTHER=$!; sleep 1
  out=$(launch_main "$ST/b-good/Fake.app" -- true 2>&1); rc=$?
  [ $rc -eq 5 ] && printf '%s' "$out" | grep -q "perf=1"; ok $? "a copy this run did not start -> refused rc=5, counted perf=1 (rc=$rc)" "$out"
  kill "$OTHER" >> "$ST/kill.log" 2>&1; wait "$OTHER" >> "$ST/kill.log" 2>&1

  mkdir -p "$ST/rig/launch.lock"; echo $$ > "$ST/rig/launch.lock/pid"
  out=$(launch_main "$ST/b-good/Fake.app" -- true 2>&1); rc=$?
  rm -rf "$ST/rig/launch.lock"
  [ $rc -eq 75 ]; ok $? "a lock held by a live run -> BUSY rc=75 after LAUNCH_BUSY_WAIT (rc=$rc)"

  # Two launchers on ONE rig folder (main112: a live run's copy ended between two journeys): the first runs a command for 7 s; the second, started while it runs, must wait or
  # refuse (BUSY rc=75 after LAUNCH_BUSY_WAIT) and never stop the first one's copy; the first ends well (rc=0) and every stop is a line of kill.log with a rule.
  rm -f "$ST/rig/kill.log" "$ST/rig/lock.log"
  ( launch_main "$ST/b-good/Fake.app" -- sh -c 'sleep 7; test -S "$D/control.sock"' > "$ST/first.out" 2>&1; echo $? > "$ST/first.rc" ) &
  FIRST=$!; _w=0
  while ! grep -q "^rig copy: " "$ST/first.out" 2>&1 && [ $_w -lt 20 ]; do sleep 1; _w=$((_w+1)); done
  sleep 1
  _fp=$(awk '$2=="rig"{print $1}' <<EOF2
$(app_procs)
EOF2
)
  out=$(launch_main "$ST/b-good/Fake.app" -- true 2>&1); rc=$?
  _alive=1; for _p in $_fp; do alive "$_p" && _alive=0; done
  [ $rc -eq 75 ] && [ -n "$_fp" ] && [ $_alive -eq 0 ]; ok $? "a second launcher on the same rig folder -> BUSY rc=75 and the first one's copy (pid ${_fp:-none}) is still alive (rc=$rc)" "$out"
  wait "$FIRST"
  [ "$(cat "$ST/first.rc")" = 0 ]; ok $? "the first launcher ends well after the second was refused (rc=$(cat "$ST/first.rc"))" "$(cat "$ST/first.out")"
  _bad=$(grep -c "" "$ST/rig/kill.log" 2>&1); _norule=$(grep "stops pid" "$ST/rig/kill.log" | grep -vc "rule: ")
  [ "${_norule:-1}" -eq 0 ] && grep -q "stops pid" "$ST/rig/kill.log"; ok $? "every stop is one kill.log line with the time, the launcher, the pid and the rule ($_bad lines)" "$(cat "$ST/rig/kill.log")"
  mkdir -p "$ST/rig/launch.lock"; echo 99999999 > "$ST/rig/launch.lock/pid"
  out=$(launch_main "$ST/b-good/Fake.app" -- true 2>&1); rc=$?
  [ $rc -eq 0 ] && grep -q "clears .*launch.lock | rule: stale lock" "$ST/rig/lock.log"; ok $? "a lock whose holder is dead is cleared once and logged in lock.log (rc=$rc)" "$(cat "$ST/rig/lock.log")"

  out=$(KEEP_RUN_DIR=1 launch_main "$ST/b-good/Fake.app" -- sh -c 'echo keep > "$D/marker"; p1=$(cat "$D/rig.pid"); WHOOSH_RIG_HIDDEN=1 sh "$SELF" --relaunch; p2=$(cat "$D/rig.pid"); [ "$p1" != "$p2" ] && kill -0 "$p2" && [ -S "$D/control.sock" ] && [ "$(cat "$D/marker")" = keep ]' 2>&1); rc=$?
  [ $rc -eq 0 ] && printf '%s' "$out" | grep -q "instances: rig=0 owner=0 kept=0 perf=0"
  ok $? "--relaunch quits the copy by its pid and starts the same copy on the same folder (marker kept, new pid, socket back, no copy left after; rc=$rc)" "$out"

  rm -rf "$ST"
  say "$ST_N checks, $ST_F failed, 0 not judged"
  [ $ST_F -eq 0 ]
}

case "${1:-}" in
  --selftest) selftest; exit $? ;;
  --relaunch) cfg; relaunch_main; exit $? ;;
  *) launch_main "$@"; exit $? ;;
esac
