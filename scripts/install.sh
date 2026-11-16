#!/bin/sh
# INSTALL A GREEN BUILD, READ ITS STAMP BACK FROM THE INSTALL LOCATION, KEEP THE ROLLBACK.
#
#   sh scripts/install.sh <label>
#   sh scripts/install.sh --selftest
#
# Step 4 of native-desktop/pipeline.md, run from the repo root. It refuses:
#   · a label with no stamped build ($BUILD_ROOT/$APP_NAME-<label>, its bundle and build-<label>.json)
#   · no verdict, a RED verdict, or a verdict measured on another commit. INSTALL_ANYWAY=1 together
#     with INSTALL_REASON="<why>" overrides, and the reason is written to docs/ARTIFACT.md.
#   · a rig copy running (a round is live)
#   · rollback copies already at KEEP_CAP (run sweep.sh first)
#   · a stamp read back from the installed copy that differs from the build's; the previous install
#     is put back.
#   · RELEASE MODE ONLY (WHOOSH_INSTALL_RELEASE=1): an installed copy that `spctl -a -vv` does not
#     call `accepted`; exit 3, the previous install is put back. Default mode runs no such step.
# Every other running copy is quit by its pid before the swap, the OWNER'S included: an install is a
# deliberate replacement of the copy they run.
#
# After the copy: Launch Services is told, the stamp is read back from $INSTALL_DIR, a row is appended
# to docs/ARTIFACT.md, and the smoke check (SMOKE_CELL, when the file exists) runs on the INSTALLED
# bytes through launch-one.sh under the test identity. The owner's line:
#   installed <label> <commit> <utc>, read back from <path>
#
# Config (the environment, then docs/pipeline.env written as `: "${VAR:=value}"`):
#   APP_NAME · BUILD_ROOT · BUNDLE_REL (as build.sh) · RIG_ROOT (as launch-one.sh)
#   INSTALL_DIR  default /Applications     KEEP_CAP  default 3
#   SMOKE_CELL   default checks/smoke-check.py      LSREGISTER  auto | none
#   WHOOSH_INSTALL_RELEASE=1   release mode: the Gatekeeper step above (needs a notarised build)
#
# Exit codes: 0 installed · 1 refused or failed (gate, copy, stamp, smoke) · 2 bad usage or no build ·
#             3 release mode and Gatekeeper did not accept the installed copy.
set -u
SELFTEST_ACTIVE=0   # set to 1 only by selftest(); a value from the environment never counts

say() { printf '%s\n' "$*"; }
HERE=$(cd "$(dirname "$0")" && pwd -P)

jget() {  # jget <file> <dotted.key>
  python3 -c 'import json, sys
d = json.load(open(sys.argv[1]))
for k in sys.argv[2].split("."):
    d = d.get(k) if isinstance(d, dict) else None
print("" if d is None else d)' "$1" "$2" 2>&1
}

# Only the first two words of each command line are read: the bundle path is the process's own
# executable, or the script it was handed. A process that merely mentions the path is not a copy.
classify() {
  ps -axo pid=,command= | awk -v app="/$APP_NAME.app/Contents/MacOS/" -v rig="$RIG_ROOT/" \
      -v owner="$INSTALL_DIR/$APP_NAME.app/" -v kept="$INSTALL_DIR/$APP_NAME " '
    { pid = $1; head = $2; if (NF > 2) head = head " " $3 }
    index(head, app) == 0 { next }
    index(head, rig) > 0   { print pid, "rig";   next }
    index(head, owner) > 0 { print pid, "owner"; next }
    index(head, kept) > 0  { print pid, "kept";  next }
    { print pid, "perf" }'
}

quit_pid() {  # TERM, then wait up to 10 s; never KILL: a copy that will not quit is the owner's to look at
  kill "$1" >> "$INSTALL_LOG" 2>&1
  _n=0
  while kill -0 "$1" >> "$INSTALL_LOG" 2>&1 && [ $_n -lt 20 ]; do sleep 0.5; _n=$((_n+1)); done
  if kill -0 "$1" >> "$INSTALL_LOG" 2>&1; then say "REFUSED: pid $1 did not quit within 10s"; return 1; fi
  return 0
}

install_main() {
  L="${1:-}"
  if [ -z "$L" ]; then say "usage: install.sh <label>"; return 2; fi
  if [ -f "${PIPELINE_ENV:-docs/pipeline.env}" ]; then . "${PIPELINE_ENV:-docs/pipeline.env}"; fi
  if [ -z "${APP_NAME:-}" ]; then say "REFUSED: APP_NAME is not set"; return 2; fi
  : "${BUILD_ROOT:=$HOME/Builds/$APP_NAME}"; : "${BUNDLE_REL:=Build/Products/Release/$APP_NAME.app}"
  : "${INSTALL_DIR:=/Applications}"; INSTALL_DIR=${INSTALL_DIR%/}; : "${KEEP_CAP:=3}"
  _tmp=${TMPDIR:-/tmp}; _tmp=${_tmp%/}
  : "${RIG_ROOT:=$_tmp/$APP_NAME-rig}"; RIG_ROOT=${RIG_ROOT%/}
  : "${SMOKE_CELL:=checks/smoke-check.py}"; : "${LSREGISTER:=auto}"
  OUT="$BUILD_ROOT/$APP_NAME-$L"; SRC="$OUT/$BUNDLE_REL"; BJ="$OUT/build-$L.json"; VJ="$OUT/verdict-$L.json"
  DST="$INSTALL_DIR/$APP_NAME.app"
  if [ ! -d "$SRC" ] || [ ! -f "$BJ" ]; then say "REFUSED: no stamped build for $L (need $SRC and $BJ)"; return 2; fi
  INSTALL_LOG="$OUT/install.log"; : > "$INSTALL_LOG"
  COMMIT=$(jget "$BJ" commit)
  if [ -z "$COMMIT" ]; then say "REFUSED: $BJ carries no commit"; return 2; fi

  OVERRIDE=""; vmsg=""
  if [ ! -f "$VJ" ]; then
    vmsg="no round has been run on $L (sh scripts/round.sh $L)"
  else
    V=$(jget "$VJ" verdict); VC=$(jget "$VJ" build.commit)
    if [ "$VC" != "$COMMIT" ]; then vmsg="the verdict was measured on commit ${VC:-unknown}, and this build is $COMMIT"
    elif [ "$V" != GREEN ]; then vmsg="the round on $L read ${V:-no verdict}"
    fi
  fi
  if [ -n "$vmsg" ]; then
    say "REFUSED: $vmsg."
    if [ -f "$VJ" ]; then
      python3 -c 'import json, sys
for c in json.load(open(sys.argv[1])).get("cells", []):
    if c.get("verdict") != "GREEN":
        print("  RED  %s  %s" % (c.get("name"), c.get("reason")))' "$VJ" 2>&1 | head -12
    fi
    if [ "${INSTALL_ANYWAY:-0}" = 1 ] && [ -n "${INSTALL_REASON:-}" ]; then
      say "INSTALL_ANYWAY=1 with a reason: going on. The reason goes into docs/ARTIFACT.md."
      OVERRIDE="override: $INSTALL_REASON ($vmsg)"
    else
      say "Install it anyway only on purpose: INSTALL_ANYWAY=1 INSTALL_REASON=\"<why>\"."
      return 1
    fi
  fi

  if [ -d "$DST" ]; then
    nk=$(find "$INSTALL_DIR" -maxdepth 1 -name "$APP_NAME *.app" | grep -c .)
    if [ "$nk" -ge "$KEEP_CAP" ]; then
      say "REFUSED: $nk rollback copies already sit in $INSTALL_DIR (cap $KEEP_CAP). Run sweep.sh, then install."; return 1
    fi
  fi

  procs=$(classify)
  rigs=$(printf '%s\n' "$procs" | awk '$2=="rig"{print $1}')
  owners=$(printf '%s\n' "$procs" | awk '$2=="owner" || $2=="kept"{print $1}')
  perfs=$(printf '%s\n' "$procs" | awk '$2=="perf"{print $1}')
  if [ -n "$rigs" ]; then say "REFUSED: a rig copy is running (pid $(echo $rigs)); a round is live. Install after it ends."; return 1; fi
  # The install REPLACES the copy the owner runs, so it quits that copy by pid. Deliberate, every time.
  for p in $owners; do say "quitting the installed copy: pid $p"; quit_pid "$p" || return 1; done
  for p in $perfs; do
    say "quitting a copy running from outside the install folder: pid $p $(ps -o command= -p "$p" | cut -c1-120)"
    quit_pid "$p" || return 1
  done

  KEPT=""
  if [ -d "$DST" ]; then
    KEPT="$INSTALL_DIR/$APP_NAME $(date '+%Y-%m-%d %H%M%S').app"
    mv "$DST" "$KEPT" || { say "REFUSED: could not move the previous install aside"; return 1; }
    say "kept the previous install at: $KEPT"
  fi
  if ! ditto "$SRC" "$DST" >> "$INSTALL_LOG" 2>&1; then
    say "INSTALL FAILED: the copy did not complete. Putting the previous install back."
    rm -rf "$DST"; [ -n "$KEPT" ] && mv "$KEPT" "$DST"
    return 1
  fi
  if [ -n "${INSTALL_SELFTEST_TAMPER:-}" ]; then
    plutil -replace NexusBuildLabel -string "$INSTALL_SELFTEST_TAMPER" "$DST/Contents/Info.plist" >> "$INSTALL_LOG" 2>&1
  fi
  bl=$(plutil -extract NexusBuildLabel raw -o - "$DST/Contents/Info.plist" 2>&1)
  bc=$(plutil -extract NexusBuildCommit raw -o - "$DST/Contents/Info.plist" 2>&1)
  if [ "$bl" != "$L" ] || [ "$bc" != "$COMMIT" ]; then
    say "INSTALL FAILED: read back label '$bl' commit '$bc' from $DST; the build is $L $COMMIT. Putting the previous install back."
    rm -rf "$DST"; [ -n "$KEPT" ] && mv "$KEPT" "$DST"
    return 1
  fi
  if [ "${WHOOSH_INSTALL_RELEASE:-0}" = 1 ]; then
    # The stand-in command is honoured only inside selftest(); a real install always runs spctl.
    if [ "$SELFTEST_ACTIVE" = 1 ] && [ -n "${INSTALL_SELFTEST_SPCTL:-}" ]; then
      gk=$(sh "$INSTALL_SELFTEST_SPCTL" -a -vv "$DST" 2>&1)
    else
      gk=$(spctl -a -vv "$DST" 2>&1)
    fi
    gsrc=$(printf '%s\n' "$gk" | sed -n 's/^source=//p' | head -1)
    if printf '%s\n' "$gk" | grep -q ': accepted$'; then
      say "gatekeeper: accepted, source ${gsrc:-not named}"
    else
      say "INSTALL FAILED: gatekeeper did not accept $DST (source ${gsrc:-not named}). Putting the previous install back."
      printf '%s\n' "$gk" >> "$INSTALL_LOG"
      rm -rf "$DST"; [ -n "$KEPT" ] && mv "$KEPT" "$DST"
      return 3
    fi
  fi
  LSR=/System/Library/Frameworks/CoreServices.framework/Versions/Current/Frameworks/LaunchServices.framework/Versions/Current/Support/lsregister
  if [ "$LSREGISTER" != none ] && [ -x "$LSR" ]; then "$LSR" -f "$DST" >> "$INSTALL_LOG" 2>&1; fi

  AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  mkdir -p docs
  if [ ! -f docs/ARTIFACT.md ]; then
    printf '# Installed artifacts\n\n| Installed at (UTC) | Label | Commit | Location | Read back | Verdict | Rollback |\n|---|---|---|---|---|---|---|\n' > docs/ARTIFACT.md
  fi
  # The row is written AFTER the smoke check, with what happened (it was written before it, so an install whose smoke was red read GREEN: main112, main114):
  # GREEN only when the smoke was green; SMOKE RED (with the log) when it was red; SMOKE NOT RUN when there is no smoke check. The override text stays beside it.
  artifact_row() {  # artifact_row <verdict words>
    printf '| %s | %s | %s | %s | label + commit | %s | %s |\n' "$AT" "$L" "$COMMIT" "$DST" "$1${OVERRIDE:+ · $OVERRIDE}" "${KEPT:-none}" >> docs/ARTIFACT.md
  }

  smoke="not run: no smoke check at $SMOKE_CELL (test-plan.md makes one on day 0)"
  if [ -f "$SMOKE_CELL" ]; then
    say "smoke check on the installed bytes, under the test identity: $SMOKE_CELL"
    case "$SMOKE_CELL" in /*) sc="$SMOKE_CELL" ;; *) sc="$(pwd -P)/$SMOKE_CELL" ;; esac
    _launch="$HERE/launch-one.sh"   # the stand-in launcher is honoured only inside selftest(), as the spctl one is
    if [ "$SELFTEST_ACTIVE" = 1 ] && [ -n "${INSTALL_SELFTEST_LAUNCH:-}" ]; then _launch="$INSTALL_SELFTEST_LAUNCH"; fi
    WHOOSH_RIG_HIDDEN=1 sh "$_launch" "$DST" -- sh -c 'exec python3 "$0" "$D"' "$sc" > "$OUT/smoke.log" 2>&1; src=$?
    if [ $src -ne 0 ]; then
      say "SMOKE RED on the installed copy (rc=$src). Log: $OUT/smoke.log"
      artifact_row "SMOKE RED (rc=$src, log $OUT/smoke.log)"
      [ -n "$KEPT" ] && say "to roll back: quit $APP_NAME, then: rm -rf \"$DST\" && mv \"$KEPT\" \"$DST\""
      return 1
    fi
    smoke="green on the installed bytes"
    artifact_row "GREEN"
  else
    artifact_row "SMOKE NOT RUN"
  fi
  say "smoke: $smoke"
  if [ -n "$KEPT" ]; then say "to roll back: quit $APP_NAME, then: rm -rf \"$DST\" && mv \"$KEPT\" \"$DST\""; fi
  say "installed $L $COMMIT $AT, read back from $DST"
  return 0
}

selftest() {
  ST=$(mktemp -d "${TMPDIR:-/tmp}/nx-install-st.XXXXXX") || return 2
  SELFTEST_ACTIVE=1
  ST_N=0; ST_F=0
  ok() { ST_N=$((ST_N+1)); if [ "$1" -eq 0 ]; then say "  PASS  $2"; else ST_F=$((ST_F+1)); say "  FAIL  $2"; fi; }
  cd "$ST" || return 2
  export APP_NAME=Fake BUILD_ROOT="$ST/builds" INSTALL_DIR="$ST/Applications" RIG_ROOT="$ST/rig" \
         LSREGISTER=none SMOKE_CELL="$ST/no-smoke.py" KEEP_CAP=3 PIPELINE_ENV="$ST/no-such-env"
  mkdir -p "$INSTALL_DIR"
  mkbuild() {  # mkbuild <label> <commit> <GREEN|RED|none> [verdict commit]
    _o="$BUILD_ROOT/Fake-$1"; _b="$_o/Build/Products/Release/Fake.app/Contents"; mkdir -p "$_b/MacOS"
    plutil -create xml1 "$_b/Info.plist" > "$ST/plutil.log" 2>&1
    plutil -insert NexusBuildLabel -string "$1" "$_b/Info.plist" >> "$ST/plutil.log" 2>&1
    plutil -insert NexusBuildCommit -string "$2" "$_b/Info.plist" >> "$ST/plutil.log" 2>&1
    printf '{"label": "%s", "commit": "%s"}\n' "$1" "$2" > "$_o/build-$1.json"
    [ "$3" = none ] || printf '{"verdict": "%s", "build": {"commit": "%s"}, "cells": []}\n' "$3" "${4:-$2}" > "$_o/verdict-$1.json"
  }
  installed() { plutil -extract NexusBuildLabel raw -o - "$INSTALL_DIR/Fake.app/Contents/Info.plist" 2>&1; }
  kept() { find "$INSTALL_DIR" -maxdepth 1 -name "Fake *.app" | grep -c .; }

  out=$(install_main nope 2>&1); rc=$?
  [ $rc -eq 2 ]; ok $? "no build -> refused rc=2 (rc=$rc)"

  mkbuild b1 c1 none
  out=$(install_main b1 2>&1); rc=$?
  [ $rc -eq 1 ] && [ ! -d "$INSTALL_DIR/Fake.app" ]; ok $? "no verdict -> refused, nothing installed (rc=$rc)"

  mkbuild b2 c2 RED
  out=$(install_main b2 2>&1); rc=$?
  [ $rc -eq 1 ] && [ ! -d "$INSTALL_DIR/Fake.app" ]; ok $? "a RED verdict -> refused (rc=$rc)"
  export INSTALL_ANYWAY=1
  out=$(install_main b2 2>&1); rc=$?
  unset INSTALL_ANYWAY
  [ $rc -eq 1 ]; ok $? "INSTALL_ANYWAY with no reason -> still refused (rc=$rc)"

  mkbuild b3 c3 GREEN c-other
  out=$(install_main b3 2>&1); rc=$?
  [ $rc -eq 1 ]; ok $? "a GREEN verdict measured on another commit -> refused (rc=$rc)"

  mkbuild b4 c4 GREEN
  out=$(install_main b4 2>&1); rc=$?
  [ $rc -eq 0 ] && [ "$(installed)" = b4 ] && printf '%s' "$out" | grep -q "installed b4 c4 .*, read back from" \
    && grep -q "| b4 | c4 |" docs/ARTIFACT.md
  ok $? "a GREEN build -> installed, stamp read back, ARTIFACT.md row written (rc=$rc)"

  mkbuild b5 c5 GREEN
  export INSTALL_SELFTEST_TAMPER=zz
  out=$(install_main b5 2>&1); rc=$?
  unset INSTALL_SELFTEST_TAMPER
  [ $rc -eq 1 ] && [ "$(installed)" = b4 ] && [ "$(kept)" -eq 0 ]
  ok $? "an installed stamp that reads back wrong -> refused and the previous install is put back (rc=$rc, installed=$(installed))"

  out=$(install_main b5 2>&1); rc=$?
  [ $rc -eq 0 ] && [ "$(installed)" = b5 ] && [ "$(kept)" -eq 1 ]; ok $? "a second install keeps one rollback copy (rc=$rc, kept=$(kept))"

  mkdir -p "$INSTALL_DIR/Fake 2000-01-01 000001.app" "$INSTALL_DIR/Fake 2000-01-01 000002.app"
  mkbuild b6 c6 GREEN
  out=$(install_main b6 2>&1); rc=$?
  [ $rc -eq 1 ] && [ "$(installed)" = b5 ]; ok $? "rollback copies at the cap -> refused, sweep first (rc=$rc)"

  # Release mode: the assessment command is a stand-in script; no real spctl runs here.
  cat > "$ST/spctl-accepted.sh" <<'EOS'
printf 'x\n' >> "$ST_CALLS"
printf '%s: accepted\nsource=Notarized Developer ID\n' "$3"
EOS
  cat > "$ST/spctl-rejected.sh" <<'EOS'
printf 'x\n' >> "$ST_CALLS"
printf '%s: rejected\nsource=no usable signature\n' "$3"
EOS
  rm -rf "$INSTALL_DIR"/Fake\ *.app   # the cap case left rollback copies; start the release cases clean
  export ST_CALLS="$ST/spctl-calls"; : > "$ST_CALLS"
  mkbuild r1 d1 GREEN
  out=$(WHOOSH_INSTALL_RELEASE=1 INSTALL_SELFTEST_SPCTL="$ST/spctl-accepted.sh" install_main r1 2>&1); rc=$?
  [ $rc -eq 0 ] && [ "$(installed)" = r1 ] && printf '%s' "$out" | grep -q "gatekeeper: accepted, source Notarized Developer ID"
  ok $? "release mode and accepted -> installed, verdict and source printed (rc=$rc)"
  rm -rf "$INSTALL_DIR"/Fake\ *.app
  mkbuild r2 d2 GREEN
  out=$(WHOOSH_INSTALL_RELEASE=1 INSTALL_SELFTEST_SPCTL="$ST/spctl-rejected.sh" install_main r2 2>&1); rc=$?
  [ $rc -eq 3 ] && [ "$(installed)" = r1 ]; ok $? "release mode and rejected -> exit 3, previous install put back (rc=$rc, installed=$(installed))"
  rm -rf "$INSTALL_DIR"/Fake\ *.app
  : > "$ST_CALLS"; mkbuild r3 d3 GREEN
  out=$(INSTALL_SELFTEST_SPCTL="$ST/spctl-rejected.sh" install_main r3 2>&1); rc=$?
  [ $rc -eq 0 ] && [ "$(installed)" = r3 ] && [ ! -s "$ST_CALLS" ] && ! printf '%s' "$out" | grep -qi gatekeeper
  ok $? "default mode -> the assessment is never called, no gatekeeper line (rc=$rc, calls=$(wc -l < "$ST_CALLS" | tr -d ' '))"

  # The record says what the smoke did: a red smoke is SMOKE RED with its log (not GREEN), a green one GREEN, none SMOKE NOT RUN; the override text stays beside it.
  printf 'exit 1\n' > "$ST/launch-red.sh"; printf 'exit 0\n' > "$ST/launch-green.sh"; : > "$ST/smoke.py"
  rm -rf "$INSTALL_DIR"/Fake\ *.app
  mkbuild s1 e1 GREEN
  out=$( ( export SMOKE_CELL="$ST/smoke.py" INSTALL_SELFTEST_LAUNCH="$ST/launch-red.sh"; install_main s1 ) 2>&1); rc=$?
  [ $rc -eq 1 ] && [ "$(installed)" = s1 ] && grep "| s1 | e1 |" docs/ARTIFACT.md | grep -q "SMOKE RED (rc=1, log " && ! grep "| s1 | e1 |" docs/ARTIFACT.md | grep -q "| GREEN"
  ok $? "a red smoke -> rc=1, the copy stays installed, the row says SMOKE RED with its log and not GREEN (rc=$rc)"
  mkbuild s2 e2 GREEN
  out=$( ( export SMOKE_CELL="$ST/smoke.py" INSTALL_SELFTEST_LAUNCH="$ST/launch-green.sh"; install_main s2 ) 2>&1); rc=$?
  [ $rc -eq 0 ] && grep "| s2 | e2 |" docs/ARTIFACT.md | grep -q "| GREEN |"
  ok $? "a green smoke -> the row says GREEN (rc=$rc)"
  rm -rf "$INSTALL_DIR"/Fake\ *.app
  mkbuild s3 e3 GREEN
  out=$(install_main s3 2>&1); rc=$?; printf "%s\n" "$out" > "$ST/s3.out"
  [ $rc -eq 0 ] && grep "| s3 | e3 |" docs/ARTIFACT.md | grep -q "SMOKE NOT RUN"
  ok $? "no smoke check -> the row says SMOKE NOT RUN (rc=$rc)"

  cd / && rm -rf "$ST"
  say "$ST_N checks, $ST_F failed, 0 not judged"
  [ $ST_F -eq 0 ]
}

case "${1:-}" in
  --selftest) selftest; exit $? ;;
  *) install_main "$@"; exit $? ;;
esac
