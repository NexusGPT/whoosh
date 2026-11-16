#!/bin/sh
# SWEEP BUILD TREES, ROLLBACK COPIES AND DEAD WORKTREES DOWN TO THE FLOOR. RUNS ALONE.
#
#   sh scripts/sweep.sh [--dry-run]
#   sh scripts/sweep.sh --selftest
#
# Step 5 of native-desktop/pipeline.md. It keeps the build tree of the INSTALLED copy (its label read
# from $INSTALL_DIR/$APP_NAME.app) and the NEWEST stamped build tree. Every other
# $BUILD_ROOT/$APP_NAME-<label> tree is removed by name, each printed before it goes. The rollback copies
# in $INSTALL_DIR are capped at KEEP_CAP, newest kept. Worktrees whose folder is gone are pruned
# (WORKTREE_REPO, when set). The rig run folder is cleared when no rig copy runs. --dry-run prints what
# would go and removes nothing.
#
# Refuses (exit 2): an installed copy whose label cannot be read (nothing says which tree the owner runs,
# so nothing is removed); a build, a round ticket or a rig copy in flight; another sweep running.
# The installed build's tree is never removed, whatever the listing says.
# Exit 3: free space is still under DISK_FLOOR_GB after the sweep.
#
# Config (the environment, then docs/pipeline.env written as `: "${VAR:=value}"`):
#   APP_NAME · BUILD_ROOT (as build.sh) · RIG_ROOT, QUEUE_DIR (as launch-one.sh, queue.sh)
#   INSTALL_DIR (/Applications) · KEEP_CAP (3) · WORKTREE_REPO (unset) · DISK_FLOOR_GB (20)
#   BUILD_PROC_MATCH (xcodebuild) · DISK_PATH (/)
set -u

say() { printf '%s\n' "$*"; }

free_gb() { df -g "$DISK_PATH" | awk 'NR==2 {print $4}'; }

sweep_main() {
  DRY=0; [ "${1:-}" = "--dry-run" ] && DRY=1
  if [ -f "${PIPELINE_ENV:-docs/pipeline.env}" ]; then . "${PIPELINE_ENV:-docs/pipeline.env}"; fi
  if [ -z "${APP_NAME:-}" ]; then say "REFUSED: APP_NAME is not set"; return 2; fi
  : "${BUILD_ROOT:=$HOME/Builds/$APP_NAME}"; BUILD_ROOT=${BUILD_ROOT%/}
  : "${INSTALL_DIR:=/Applications}"; INSTALL_DIR=${INSTALL_DIR%/}; : "${KEEP_CAP:=3}"
  _tmp=${TMPDIR:-/tmp}; _tmp=${_tmp%/}
  : "${RIG_ROOT:=$_tmp/$APP_NAME-rig}"; RIG_ROOT=${RIG_ROOT%/}; : "${QUEUE_DIR:=$RIG_ROOT/queue}"
  : "${DISK_FLOOR_GB:=20}"; : "${BUILD_PROC_MATCH:=xcodebuild}"; : "${DISK_PATH:=/}"
  mkdir -p "$BUILD_ROOT" || return 2

  LOCK="$BUILD_ROOT/.sweep.lock"
  if ! mkdir "$LOCK" > "$BUILD_ROOT/.sweep.log" 2>&1; then say "REFUSED: another sweep holds $LOCK"; return 2; fi
  trap 'rm -rf "$LOCK"' EXIT

  nb=$(pgrep -f "$BUILD_PROC_MATCH" | grep -c .)
  if [ "$nb" -gt 0 ]; then say "REFUSED: $nb build process(es) running; a sweep runs alone"; return 2; fi
  if [ -d "$QUEUE_DIR" ]; then
    nt=$(find "$QUEUE_DIR" -maxdepth 1 -type f ! -name '*.tmp' ! -name '.*' | grep -c .)
    if [ "$nt" -gt 0 ]; then say "REFUSED: $nt round ticket(s) in $QUEUE_DIR; a sweep runs alone"; return 2; fi
  fi
  # Only the first two words: a process that merely mentions the path is not a running copy.
  nr=$(ps -axo command= | awk -v app="/$APP_NAME.app/Contents/MacOS/" -v rig="$RIG_ROOT/" \
        '{ head = $1; if (NF > 1) head = head " " $2 } index(head, app) > 0 && index(head, rig) > 0' | grep -c .)
  if [ "$nr" -gt 0 ]; then say "REFUSED: $nr rig copy(ies) running; a sweep runs alone"; return 2; fi

  IL=""
  if [ -d "$INSTALL_DIR/$APP_NAME.app" ]; then
    IL=$(plutil -extract NexusBuildLabel raw -o - "$INSTALL_DIR/$APP_NAME.app/Contents/Info.plist" 2>&1) || IL=""
    if [ -z "$IL" ]; then
      say "REFUSED: $INSTALL_DIR/$APP_NAME.app carries no NexusBuildLabel. Nothing says which build tree the owner runs, so nothing is removed."
      return 2
    fi
  fi
  NEWEST=$(ls -t "$BUILD_ROOT/$APP_NAME-"*/build-*.json 2>&1 | grep '/build-.*\.json$' | head -1)
  NEWEST_TREE=""; [ -n "$NEWEST" ] && NEWEST_TREE=$(dirname "$NEWEST")
  before=$(free_gb)
  say "== sweep  installed label ${IL:-none}  newest ${NEWEST_TREE:-none}  free ${before} GB  $( [ $DRY -eq 1 ] && echo '(dry run)')"

  removed=0
  for t in $(find "$BUILD_ROOT" -maxdepth 1 -type d -name "$APP_NAME-*" | sort); do
    label=${t##*/}; label=${label#"$APP_NAME-"}
    # A build's source snapshot (scripts/build.sh removes it itself; this catches one a killed build left), kept trees included.
    if [ -d "$t/src" ]; then
      if [ $DRY -eq 1 ]; then say "  would remove snapshot $t/src"; else
        say "  remove  snapshot $t/src"
        if [ -n "${WORKTREE_REPO:-}" ]; then git -C "$WORKTREE_REPO" worktree remove --force "$t/src" >> "$BUILD_ROOT/.sweep.log" 2>&1; fi
        rm -rf "$t/src"
      fi
    fi
    if [ -n "$IL" ] && [ "$label" = "$IL" ]; then say "  keep    $t (installed)"; continue; fi
    if [ "$t" = "$NEWEST_TREE" ]; then say "  keep    $t (newest)"; continue; fi
    if [ $DRY -eq 1 ]; then say "  would remove $t"; continue; fi
    say "  remove  $t"
    rm -rf "$t"; removed=$((removed+1))
  done

  i=0
  for k in $(find "$INSTALL_DIR" -maxdepth 1 -name "$APP_NAME *.app" | sort -r | tr ' ' '\001'); do
    k=$(printf '%s' "$k" | tr '\001' ' ')
    i=$((i+1))
    if [ $i -le "$KEEP_CAP" ]; then say "  keep    $k (rollback $i of $KEEP_CAP)"; continue; fi
    if [ $DRY -eq 1 ]; then say "  would remove $k"; continue; fi
    say "  remove  $k"
    rm -rf "$k"; removed=$((removed+1))
  done

  if [ -n "${WORKTREE_REPO:-}" ]; then
    if [ $DRY -eq 1 ]; then git -C "$WORKTREE_REPO" worktree prune --dry-run -v 2>&1 | sed 's/^/  would prune /'
    else git -C "$WORKTREE_REPO" worktree prune -v 2>&1 | sed 's/^/  prune   /'; fi
  fi
  for _r in "$RIG_ROOT"/run-*; do
    [ -d "$_r" ] || continue
    if [ $DRY -eq 1 ]; then say "  would remove $_r"; else say "  remove  $_r"; rm -rf "$_r"; fi
  done

  after=$(free_gb)
  say "free ${before} GB -> ${after} GB; removed $removed"
  if [ $DRY -eq 0 ] && [ "$after" -lt "$DISK_FLOOR_GB" ]; then
    say "DISK UNDER FLOOR: ${after} GB free after the sweep, floor ${DISK_FLOOR_GB} GB. What is left is not this app's to remove."; return 3
  fi
  return 0
}

selftest() {
  _t=${TMPDIR:-/tmp}; ST=$(mktemp -d "${_t%/}/nx-sweep-st.XXXXXX") || return 2
  ST_N=0; ST_F=0
  ok() { ST_N=$((ST_N+1)); if [ "$1" -eq 0 ]; then say "  PASS  $2"; else ST_F=$((ST_F+1)); say "  FAIL  $2"; fi; }
  export APP_NAME=Fake BUILD_ROOT="$ST/builds" INSTALL_DIR="$ST/Applications" RIG_ROOT="$ST/rig" QUEUE_DIR="$ST/rig/queue" \
         KEEP_CAP=3 DISK_FLOOR_GB=0 BUILD_PROC_MATCH="nx-selftest-no-such-build-process" PIPELINE_ENV="$ST/no-such-env"
  fixture() {
    rm -rf "$BUILD_ROOT" "$INSTALL_DIR" "$RIG_ROOT"
    mkdir -p "$BUILD_ROOT" "$INSTALL_DIR/Fake.app/Contents" "$QUEUE_DIR"
    n=0
    for l in a1 a2 a3 a4; do
      n=$((n+1)); mkdir -p "$BUILD_ROOT/Fake-$l"; echo "{}" > "$BUILD_ROOT/Fake-$l/build-$l.json"
      touch -t "20260101010$n" "$BUILD_ROOT/Fake-$l/build-$l.json"
    done
    mkdir -p "$BUILD_ROOT/Fake-a5"
    plutil -create xml1 "$INSTALL_DIR/Fake.app/Contents/Info.plist" > "$ST/plutil.log" 2>&1
    plutil -insert NexusBuildLabel -string a2 "$INSTALL_DIR/Fake.app/Contents/Info.plist" >> "$ST/plutil.log" 2>&1
    for d in 1 2 3 4 5; do mkdir -p "$INSTALL_DIR/Fake 2026-01-0$d 000000.app"; done
  }
  present() { for x in "$@"; do [ -e "$x" ] || return 1; done; return 0; }
  absent() { for x in "$@"; do [ -e "$x" ] && return 1; done; return 0; }
  B="$BUILD_ROOT"; I="$INSTALL_DIR"

  fixture
  out=$(sweep_main --dry-run 2>&1); rc=$?
  w=$(printf '%s\n' "$out" | grep -c "would remove")
  [ $rc -eq 0 ] && [ "$w" -eq 5 ] && present "$B/Fake-a1" "$B/Fake-a3" "$B/Fake-a5" "$I/Fake 2026-01-01 000000.app" \
    && printf '%s\n' "$out" | grep -q "keep    $B/Fake-a2 (installed)" && printf '%s\n' "$out" | grep -q "keep    $B/Fake-a4 (newest)"
  ok $? "dry run: names a1, a3, a5 and the two oldest rollbacks; keeps a2 (installed) and a4 (newest); removes nothing (rc=$rc, would=$w)"

  out=$(sweep_main 2>&1); rc=$?
  [ $rc -eq 0 ] && absent "$B/Fake-a1" "$B/Fake-a3" "$B/Fake-a5" "$I/Fake 2026-01-01 000000.app" "$I/Fake 2026-01-02 000000.app" \
    && present "$B/Fake-a2" "$B/Fake-a4" "$I/Fake 2026-01-03 000000.app" "$I/Fake 2026-01-05 000000.app" "$I/Fake.app"
  ok $? "the sweep removes exactly the dry run's list and keeps the installed tree, the newest tree and three rollbacks (rc=$rc)"

  fixture
  plutil -remove NexusBuildLabel "$I/Fake.app/Contents/Info.plist" >> "$ST/plutil.log" 2>&1
  out=$(sweep_main 2>&1); rc=$?
  [ $rc -eq 2 ] && present "$B/Fake-a1" "$B/Fake-a3"; ok $? "an installed copy with no label -> refused, nothing removed (rc=$rc)"

  fixture
  sleep 60 & HOLD=$!
  echo "$HOLD" > "$QUEUE_DIR/0000000000000000001-00000001"
  out=$(sweep_main 2>&1); rc=$?
  kill "$HOLD" >> "$ST/kill.log" 2>&1; wait "$HOLD" >> "$ST/kill.log" 2>&1
  [ $rc -eq 2 ] && present "$B/Fake-a1"; ok $? "a round ticket in the queue -> refused, nothing removed (rc=$rc)"

  fixture
  git -C "$ST" init -q repo > "$ST/git.log" 2>&1
  git -C "$ST/repo" -c user.email=selftest@example.invalid -c user.name=selftest commit -q --allow-empty -m fixture >> "$ST/git.log" 2>&1
  git -C "$ST/repo" worktree add -q --detach "$B/Fake-a4/src" >> "$ST/git.log" 2>&1
  mkdir -p "$B/Fake-a3/src"; echo x > "$B/Fake-a3/src/f"
  out=$(WORKTREE_REPO="$ST/repo" sweep_main 2>&1); rc=$?
  git -C "$ST/repo" worktree list > "$ST/wl.log" 2>&1
  [ $rc -eq 0 ] && absent "$B/Fake-a4/src" "$B/Fake-a3" && present "$B/Fake-a4/build-a4.json" && [ "$(grep -c . "$ST/wl.log")" -eq 1 ]
  ok $? "a snapshot left in a kept tree is removed through git and the tree stays; one left in a swept tree goes with it (rc=$rc)"

  rm -rf "$ST"
  say "$ST_N checks, $ST_F failed, 0 not judged"
  [ $ST_F -eq 0 ]
}

case "${1:-}" in
  --selftest) selftest; exit $? ;;
  *) sweep_main "$@"; exit $? ;;
esac
