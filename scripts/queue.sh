#!/bin/sh
# TAKE THE ONE APP INSTANCE IN THE ORDER PEOPLE ASKED FOR IT.
#
#   sh scripts/queue.sh -- <command> [args...]
#   sh scripts/queue.sh --selftest
#
# A lock alone is a race: every waiter polls and whoever polls first after a release wins, so a
# waiter can lose the line to runs that arrived after it. The queue takes a TICKET first, named by
# the nanosecond it was taken and this pid, and runs the command only when this ticket is the
# oldest one whose holder is still alive. A ticket whose holder has ended is removed, so an
# interrupted run never blocks the line. The command is normally launch-one.sh, which holds the
# instance lock itself; the queue decides only the ORDER.
#
# Exit: the command's exit code; 75 when the wait passed QUEUE_WAIT seconds (nothing ran, which is
# not the same as a failed run).
#
# Config (the environment, then docs/pipeline.env written as `: "${VAR:=value}"`):
#   QUEUE_DIR   default $RIG_ROOT/queue (RIG_ROOT as in launch-one.sh)
#   QUEUE_WAIT  3600 seconds          QUEUE_POLL  1 second
set -u

say() { printf '%s\n' "$*"; }

stamp_ns() {
  _t=$(python3 -c 'import time; print(time.time_ns())' 2>&1)
  case "$_t" in ''|*[!0-9]*) _t="$(date +%s)000000000" ;; esac
  printf '%s' "$_t"
}

live_tickets() { ls "$QUEUE_DIR" | grep -v '\.tmp$' | sort; }

queue_main() {
  if [ -f "${PIPELINE_ENV:-docs/pipeline.env}" ]; then . "${PIPELINE_ENV:-docs/pipeline.env}"; fi
  if [ "${1:-}" != "--" ] || [ $# -lt 2 ]; then say "usage: queue.sh -- <command> [args...]"; return 2; fi
  shift
  _tmp=${TMPDIR:-/tmp}; _tmp=${_tmp%/}
  : "${RIG_ROOT:=$_tmp/${APP_NAME:-app}-rig}"; RIG_ROOT=${RIG_ROOT%/}
  : "${QUEUE_DIR:=$RIG_ROOT/queue}"; : "${QUEUE_WAIT:=3600}"; : "${QUEUE_POLL:=1}"
  # The queue lives under the rig root (the hidden root). A QUEUE_DIR that points elsewhere and cannot be
  # written (the old chmod-500 root) is replaced by $RIG_ROOT/queue, said out loud, never a silent exit 2.
  if ! mkdir -p "$QUEUE_DIR" > "${TMPDIR:-/tmp}/queue-mkdir.$$" 2>&1; then
    say "queue: $QUEUE_DIR is not writable ($(cat "${TMPDIR:-/tmp}/queue-mkdir.$$")); using $RIG_ROOT/queue"
    QUEUE_DIR="$RIG_ROOT/queue"
    mkdir -p "$QUEUE_DIR" || { say "REFUSED: no writable queue directory ($QUEUE_DIR)"; rm -f "${TMPDIR:-/tmp}/queue-mkdir.$$"; return 2; }
  fi
  rm -f "${TMPDIR:-/tmp}/queue-mkdir.$$"
  TICKET="$QUEUE_DIR/$(stamp_ns)-$(printf '%08d' $$)"
  echo $$ > "$TICKET.tmp" && mv "$TICKET.tmp" "$TICKET" || { say "REFUSED: could not write a ticket in $QUEUE_DIR"; return 2; }
  trap 'rm -f "$TICKET"' EXIT
  trap 'exit 130' INT TERM
  start=$(date +%s)
  while :; do
    for t in "$QUEUE_DIR"/*; do
      [ -f "$t" ] || continue
      case "$t" in *.tmp) continue ;; esac
      p=$(cat "$t" 2>&1)
      case "$p" in ''|*[!0-9]*) say "unreadable ticket removed: $(basename "$t")"; rm -f "$t"; continue ;; esac
      if ! kill -0 "$p" > "$QUEUE_DIR/.probe" 2>&1; then say "ticket of a run that ended removed: $(basename "$t")"; rm -f "$t"; fi
    done
    first=$(live_tickets | head -1)
    [ "$QUEUE_DIR/$first" = "$TICKET" ] && break
    waited=$(( $(date +%s) - start ))
    if [ $waited -ge "$QUEUE_WAIT" ]; then say "WAITED OUT: still behind $first after ${waited}s; nothing ran"; return 75; fi
    sleep "$QUEUE_POLL"
  done
  say "queue: this run is first of $(live_tickets | grep -c .) after $(( $(date +%s) - start ))s"
  "$@"
}

selftest() {
  ST=$(mktemp -d /tmp/nx-queue-st.XXXXXX) || return 2
  ST_N=0; ST_F=0
  ok() { ST_N=$((ST_N+1)); if [ "$1" -eq 0 ]; then say "  PASS  $2"; else ST_F=$((ST_F+1)); say "  FAIL  $2"; fi; }
  export QUEUE_DIR="$ST/q" QUEUE_POLL=0.2 PIPELINE_ENV="$ST/no-such-env"
  mkdir -p "$QUEUE_DIR"
  SELF="$0"

  for n in A B C; do
    QUEUE_WAIT=30 sh "$SELF" -- sh -c "echo start-$n >> '$ST/order'; sleep 1; echo end-$n >> '$ST/order'" > "$ST/$n.out" 2>&1 &
    sleep 0.4
  done
  wait
  got=$(tr '\n' ' ' < "$ST/order")
  [ "$got" = "start-A end-A start-B end-B start-C end-C " ]; ok $? "three waiters are served in the order they asked, one at a time ($got)"

  sleep 60 & HOLD=$!
  echo "$HOLD" > "$QUEUE_DIR/0000000000000000001-00000001"
  QUEUE_WAIT=30 sh "$SELF" -- sh -c "echo ran > '$ST/ran2'" > "$ST/w2.out" 2>&1 &
  W=$!
  sleep 1.5
  [ ! -f "$ST/ran2" ]; ok $? "a live older ticket holds the line"
  kill "$HOLD" >> "$ST/kill.log" 2>&1; wait "$HOLD" >> "$ST/kill.log" 2>&1
  wait "$W"; rc=$?
  [ $rc -eq 0 ] && [ -f "$ST/ran2" ]; ok $? "a holder that ended frees the line (rc=$rc)"

  sleep 60 & HOLD=$!
  echo "$HOLD" > "$QUEUE_DIR/0000000000000000001-00000002"
  QUEUE_WAIT=1 sh "$SELF" -- sh -c "echo ran > '$ST/ran3'" > "$ST/w3.out" 2>&1; rc=$?
  kill "$HOLD" >> "$ST/kill.log" 2>&1; wait "$HOLD" >> "$ST/kill.log" 2>&1
  rm -f "$QUEUE_DIR/0000000000000000001-00000002"
  [ $rc -eq 75 ] && [ ! -f "$ST/ran3" ]; ok $? "a wait past QUEUE_WAIT exits 75 and runs nothing (rc=$rc)"

  QUEUE_WAIT=5 sh "$SELF" -- sh -c 'exit 9' > "$ST/w4.out" 2>&1; rc=$?
  [ $rc -eq 9 ]; ok $? "the command's exit code is the queue's exit code (rc=$rc)"

  left=$(ls "$QUEUE_DIR" | grep -c .)
  [ "$left" -eq 0 ]; ok $? "no ticket is left behind ($left)"

  rm -rf "$ST"
  say "$ST_N checks, $ST_F failed, 0 not judged"
  [ $ST_F -eq 0 ]
}

case "${1:-}" in
  --selftest) selftest; exit $? ;;
  *) queue_main "$@"; exit $? ;;
esac
