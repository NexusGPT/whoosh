#!/bin/sh
# A ROUND: every cell of the manifest, each on its own instance, one verdict read from exit codes.
#
#   sh scripts/round.sh <label> [docs/feature-manifest.md]
#   sh scripts/round.sh --selftest
#
# Step 3 of native-desktop/pipeline.md, run from the repo root. The manifest is the round's INPUT: a
# markdown table whose header has a `Cell` column. Each row is one feature, and its cell is the check
# file that proves it (a .py or .sh, or an executable, taking the run directory as its one argument).
# A row whose cell is empty or "—" is a MISSING cell, and a missing cell is red.
#
# A cell is RED when any of these holds, and the round exits 1 when any cell is red:
#   · its exit code is not 0
#   · it printed no line "N checks, F failed[, U not judged]"
#   · N is 0 (a check that checked nothing) · F is above 0 · U is above 0 (not judged is never green)
#   · a crash report for $APP_NAME is newer than the cell's start
#   · the cell is missing, or its file is not on this branch
#
# The verdict goes to $BUILD_ROOT/$APP_NAME-<label>/verdict-<label>.json: the build stamp (from
# build-<label>.json), the suite digest (the manifest and every cell file, hashed), the load at the
# start, and one row per cell {name, cell, exit, seconds, cases, failed, not_judged, verdict, reason}.
# install.sh reads it. Each cell's full output stays in the build tree's round/ folder.
#
# The round waits for running builds to end (a round beside a build measures the machine, not the
# build), up to ROUND_BUILD_WAIT seconds, and never stops a build.
#
# Config (the environment, then docs/pipeline.env written as `: "${VAR:=value}"`):
#   APP_NAME · BUILD_ROOT · BUNDLE_REL (as build.sh)
#   CRASH_DIR          default $HOME/Library/Logs/DiagnosticReports
#   BUILD_PROC_MATCH   default xcodebuild           ROUND_BUILD_WAIT  default 3600
#   ROUND_RUNNER       queue (default: queue.sh, then launch-one.sh on the build) | direct (self-test)
set -u
# THE ROUND NEEDS ROOM. Its cells resolve Swift packages (a clone per new dependency) and build the Rust tests;
# on main51 (2026-11-09 22:00) the disk reached 0 during the resolution and every cell after read red for
# nothing. Under ROUND_DISK_FLOOR_GB free the round refuses to start and says so, instead of failing midway.
: "${ROUND_DISK_FLOOR_GB:=6}"
free_gb=$(df -g "$(pwd)" | awk 'NR==2 {print $4}')
if [ "${free_gb:-0}" -lt "$ROUND_DISK_FLOOR_GB" ]; then
  echo "REFUSED: ${free_gb} GB free, the round needs ${ROUND_DISK_FLOOR_GB} GB (sweep first: sh scripts/sweep.sh)"; exit 3
fi

say() { printf '%s\n' "$*"; }
HERE=$(cd "$(dirname "$0")" && pwd -P)

cells_of() {  # "name<TAB>cell" per manifest row; cell "-" when missing
  python3 - "$1" <<'PY'
import re, sys
col = None
fcol = None
bcol = None
n = 0
for raw in open(sys.argv[1], encoding="utf-8"):
    s = raw.strip()
    if not s.startswith("|"):
        if not s:
            col = None
        continue
    parts = [p.strip() for p in s.strip("|").split("|")]
    if col is None:
        low = [p.strip("`* ").lower() for p in parts]
        if "cell" in low:
            col = low.index("cell")
            # a manifest with a separate `file` column names the cell in `cell` and its file in `file`
            fcol = low.index("file") if "file" in low else None
            bcol = low.index("budget") if "budget" in low else None
        continue
    if all(re.fullmatch(r":?-{2,}:?", p) for p in parts if p):
        continue
    name = parts[0] if parts else ""
    if not name:
        continue
    src = fcol if fcol is not None else col
    cell = (parts[src] if src < len(parts) else "").strip("`").strip()
    if cell in ("", "-", "—", "–", "none", "n/a"):
        cell = "-"
    slug = re.sub(r"[^A-Za-z0-9._-]+", "-", name).strip("-")[:48] or "row"
    b = parts[bcol].strip("` ") if bcol is not None and bcol < len(parts) else ""
    print("%s\t%s\t%s" % (slug, cell, b if b.isdigit() else "0"))
PY
}

tree_of() {  # the pid and every descendant, parents first
  local _k
  printf '%s\n' "$1"
  for _k in $(pgrep -P "$1" 2>&1 | grep -E '^[0-9]+$'); do tree_of "$_k"; done
}
kill_tree() {  # TERM the whole tree (launch-one's trap then tears its rig copy down), then KILL what stays after 8 s
  local _all _p
  _all=$(tree_of "$1")
  for _p in $_all; do kill "$_p" > /dev/null 2>&1; done
  sleep 8
  for _p in $_all; do kill -9 "$_p" > /dev/null 2>&1; done
}

summary_of() {  # "N F U" from the last summary line, or nothing
  grep -E '^[[:space:]]*[0-9]+ checks?, [0-9]+ failed' "$1" | tail -1 \
    | sed -E 's/^[[:space:]]*([0-9]+) checks?, ([0-9]+) failed(, ([0-9]+) not (judged|proven))?.*/\1 \2 \4/'
}

round_main() {
  L="${1:-}"; MAN="${2:-docs/feature-manifest.md}"
  if [ -z "$L" ]; then say "usage: round.sh <label> [manifest]"; return 2; fi
  if [ -f "${PIPELINE_ENV:-docs/pipeline.env}" ]; then . "${PIPELINE_ENV:-docs/pipeline.env}"; fi
  if [ -z "${APP_NAME:-}" ]; then say "REFUSED: APP_NAME is not set"; return 2; fi
  : "${BUILD_ROOT:=$HOME/Builds/$APP_NAME}"; : "${BUNDLE_REL:=Build/Products/Release/$APP_NAME.app}"
  : "${CRASH_DIR:=$HOME/Library/Logs/DiagnosticReports}"; : "${BUILD_PROC_MATCH:=xcodebuild}"
  : "${ROUND_BUILD_WAIT:=3600}"; : "${ROUND_RUNNER:=queue}"
  if [ ! -f "$MAN" ]; then say "REFUSED: no manifest at $MAN. The manifest is the round's input (test-plan.md)."; return 2; fi
  OUT="$BUILD_ROOT/$APP_NAME-$L"; BJ="$OUT/build-$L.json"
  if [ ! -f "$BJ" ]; then say "REFUSED: no stamped build at $BJ. Run build.sh first."; return 2; fi
  BUNDLE="${ROUND_BUNDLE:-$OUT/$BUNDLE_REL}"   # ROUND_BUNDLE points the round at an installed or otherwise built app
  # Every driven cell runs exactly like the direct launch of the main jobs: the fixture core (flag AND env) and the hidden
  # rig (flag AND env). Without them the app starts on the real, empty core and a list check reads 0 rows.
  : "${ROUND_APP_ARGS:=--fixture-core --rig-hidden}"
  : "${LAUNCH_ENV:=WHOOSH_RIG_HIDDEN=1 WHOOSH_FIXTURE_CORE=1}"; export LAUNCH_ENV
  ROOT=$(pwd -P)
  RD="$OUT/round"; rm -rf "$RD"; mkdir -p "$RD" || return 2

  cells_of "$MAN" > "$RD/manifest-cells.txt" 2> "$RD/manifest-parse.log" \
    || { say "REFUSED: the manifest could not be parsed"; cat "$RD/manifest-parse.log"; return 2; }
  if [ ! -s "$RD/manifest-cells.txt" ]; then say "REFUSED: $MAN has no table with a Cell column, or the table has no rows"; return 2; fi

  w=0
  while :; do
    nb=$(pgrep -f "$BUILD_PROC_MATCH" | grep -c .)
    [ "$nb" -eq 0 ] && break
    if [ $w -ge "$ROUND_BUILD_WAIT" ]; then say "REFUSED: $nb build process(es) still running after ${w}s; a round beside a build measures the machine"; return 2; fi
    [ $w -eq 0 ] && say "waiting for $nb build process(es) to end before the round starts"
    sleep 30; w=$((w+30))
  done
  LOAD=$(sysctl -n vm.loadavg 2>&1 | tr -d '{}' | awk '{print $1}')
  say "== round $L  manifest $MAN  load at start $LOAD"

  if [ -z "${ROUND_CELL_BUDGET_S:-}" ]; then say "REFUSED: ROUND_CELL_BUDGET_S is not set (docs/pipeline.env, from docs/CONSTANTS.md)"; return 2; fi
  TSV="$RD/cells.tsv"; : > "$TSV"
  red=0; i=0; TAB=$(printf '\t')
  while IFS="$TAB" read -r name cell cbudget <&3; do
    i=$((i+1)); nm=$(printf '%02d-%s' "$i" "$name")
    log="$RD/$nm.txt"; : > "$log"
    : > "$RD/$nm.start"; start=$(date +%s)
    reason=""; ec=""; N=""; F=""; U=""
    if [ "$cell" = "-" ]; then
      reason="missing cell"
    elif [ ! -f "$ROOT/$cell" ]; then
      case "$cell" in
        *"{"*|*" + "*|*";"*|*"("*) reason="cell is not one runnable file (a pattern or a list): $cell" ;;
        *) reason="cell file not on this branch: $cell" ;;
      esac
    else
      cs="$RD/$nm.cell.sh"
      case "$cell" in
        *.py) printf 'cd "%s" && exec python3 "%s" "$D"\n' "$ROOT" "$ROOT/$cell" > "$cs" ;;
        *.sh) printf 'cd "%s" && exec sh "%s" "$D"\n' "$ROOT" "$ROOT/$cell" > "$cs" ;;
        *)    printf 'cd "%s" && exec "%s" "$D"\n' "$ROOT" "$ROOT/$cell" > "$cs" ;;
      esac
      mode=$(sed -n '2,8s/^# round: *//p' "$ROOT/$cell" | head -1)   # rig (default) | own-rig | no-rig, named in the cell's own header
      bud="${cbudget:-0}"; [ "$bud" -gt 0 ] 2> /dev/null || bud="$ROUND_CELL_BUDGET_S"
      rm -f "$RD/$nm.over"
      (
      if [ "$ROUND_RUNNER" = direct ]; then
          mkdir -p "$RD/$nm.d"; D="$RD/$nm.d" sh "$cs"
        elif [ "$mode" = own-rig ] || [ "$mode" = no-rig ]; then
          # the cell starts its own rig copies (own-rig) or needs none (no-rig); the queue still holds the line
          mkdir -p "$RD/$nm.d"; WHOOSH_BUNDLE="$BUNDLE" WHOOSH_RIG_HIDDEN=1 D="$RD/$nm.d" QUEUE_DIR="${RIG_ROOT:-$HOME/Library/Application Support/Whoosh-rig-hidden}/queue" sh "$HERE/queue.sh" -- sh "$cs"
        else
          WHOOSH_BUNDLE="$BUNDLE" WHOOSH_RIG_HIDDEN=1 QUEUE_DIR="${RIG_ROOT:-$HOME/Library/Application Support/Whoosh-rig-hidden}/queue" sh "$HERE/queue.sh" -- sh "$HERE/launch-one.sh" "$BUNDLE" -- $ROUND_APP_ARGS sh "$cs"   # every rig copy is hidden (owner 16:30)
        fi
      ) > "$log" 2>&1 &
      cpid=$!
      ( sleep "$bud"; : > "$RD/$nm.over"; kill_tree "$cpid" ) > /dev/null 2>&1 &
      wd=$!
      wait "$cpid"; ec=$?
      if [ -f "$RD/$nm.over" ]; then wait "$wd"; else kill "$wd" > /dev/null 2>&1; pkill -P "$wd" > /dev/null 2>&1; fi   # a running watchdog is stopped first; one that fired is let finish its kill
      set -- $(summary_of "$log"); N="${1:-}"; F="${2:-}"; U="${3:-0}"
      if [ -f "$RD/$nm.over" ]; then reason="over budget: ${bud} s (killed with its rig copy)"
      elif [ "$ec" -ne 0 ]; then reason="exit $ec"
      elif [ -z "$N" ]; then reason="no summary line"
      elif [ "$N" -eq 0 ]; then reason="zero cases"
      elif [ "$F" -gt 0 ]; then reason="$F failed"
      elif [ "$U" -gt 0 ]; then reason="$U not judged"
      fi
      if [ -z "$reason" ] && [ -d "$CRASH_DIR" ]; then
        cr=$(find "$CRASH_DIR" -maxdepth 1 -name "$APP_NAME*" -newer "$RD/$nm.start" -print | head -1)
        [ -n "$cr" ] && reason="crash report newer than the cell start: $(basename "$cr")"
      fi
    fi
    secs=$(( $(date +%s) - start ))
    if [ -n "$reason" ]; then v=RED; red=1; else v=GREEN; fi
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$nm" "$cell" "$ec" "$secs" "$N" "$F" "$U" "$v" "$reason" >> "$TSV"
    printf '  %-5s %-40s %4ss  %s\n' "$v" "$nm" "$secs" "${reason:-$(tail -1 "$log" | cut -c1-80)}"
    if [ "$v" = RED ]; then grep -E '^(FAIL|NOT JUDGED|SKIP)' "$log" | head -4 | sed 's/^/          /'; fi
  done 3< "$RD/manifest-cells.txt"

  DIG=$( { cat "$MAN"; cut -f2 "$TSV" | while read -r c; do if [ "$c" != "-" ] && [ -f "$ROOT/$c" ]; then cat "$ROOT/$c"; fi; done; } | shasum -a 256 | cut -c1-16)
  V=GREEN; [ $red -eq 1 ] && V=RED
  AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  python3 - "$OUT/verdict-$L.json" "$BJ" "$TSV" "$L" "$V" "$AT" "$LOAD" "$DIG" "$MAN" <<'PY'
import json, sys
out, bj, tsv, label, verdict, at, load, dig, man = sys.argv[1:]
def num(x):
    try:
        return int(x)
    except ValueError:
        return None
cells = []
for line in open(tsv, encoding="utf-8"):
    f = line.rstrip("\n").split("\t")
    cells.append({"name": f[0], "cell": f[1], "exit": num(f[2]), "seconds": num(f[3]), "cases": num(f[4]),
                  "failed": num(f[5]), "not_judged": num(f[6]), "verdict": f[7], "reason": f[8]})
with open(bj) as fb:
    build = json.load(fb)
with open(out, "w") as fo:
    json.dump({"label": label, "verdict": verdict, "finished_at": at, "load_at_start": load,
               "suite_digest": dig, "manifest": man, "build": build, "cells": cells}, fo, indent=2)
PY
  if [ ! -f "$OUT/verdict-$L.json" ]; then say "REFUSED: the verdict file was not written"; return 2; fi
  total=$(grep -c . "$TSV"); green=$(awk -F'\t' '$8=="GREEN"' "$TSV" | grep -c .)
  say ""
  say "VERDICT: $V $L $AT  ($green of $total cells green; $OUT/verdict-$L.json)"
  [ $red -eq 0 ]
}

selftest() {
  ST=$(mktemp -d /tmp/nx-round-st.XXXXXX) || return 2
  ST_N=0; ST_F=0
  ok() { ST_N=$((ST_N+1)); if [ "$1" -eq 0 ]; then say "  PASS  $2"; else ST_F=$((ST_F+1)); say "  FAIL  $2"; fi; }
  export APP_NAME=Fake BUILD_ROOT="$ST/builds" ROUND_RUNNER=direct CRASH_DIR="$ST/crash" \
         BUILD_PROC_MATCH="nx-selftest-no-such-build-process" ROUND_BUILD_WAIT=0 ROUND_CELL_BUDGET_S=30 PIPELINE_ENV="$ST/no-such-env"
  mkdir -p "$ST/repo/checks" "$CRASH_DIR" "$BUILD_ROOT/Fake-t1"
  echo '{"label": "t1", "commit": "abc"}' > "$BUILD_ROOT/Fake-t1/build-t1.json"
  cd "$ST/repo" || return 2
  printf 'echo "3 checks, 1 failed"\nexit 1\n' > checks/red-exit.sh
  printf 'echo "0 checks, 0 failed"\n' > checks/zero.sh
  printf 'echo "4 checks, 0 failed, 2 not judged"\n' > checks/notjudged.sh
  printf 'echo "all good, trust me"\n' > checks/nosummary.sh
  printf 'echo "5 checks, 0 failed, 0 not judged"\n' > checks/green.sh
  printf 'sleep 1; touch "%s/Fake-2026-01-01-000000.ips"\necho "2 checks, 0 failed"\n' "$CRASH_DIR" > checks/crash.sh
  cat > all-man.md <<'MAN'
| Feature | Primitive | Cell |
|---|---|---|
| Red exit | local | `checks/red-exit.sh` |
| Zero cases | local | checks/zero.sh |
| Not judged | local | checks/notjudged.sh |
| Missing | local | — |
| No summary | local | checks/nosummary.sh |
| Absent file | local | checks/absent.sh |
| Green | local | checks/green.sh |
MAN
  vj="$BUILD_ROOT/Fake-t1/verdict-t1.json"
  verdicts() { python3 -c 'import json, sys
d = json.load(open(sys.argv[1]))
print(" ".join(c["verdict"] for c in d["cells"]), d["verdict"])' "$vj" 2>&1; }

  out=$(round_main t1 all-man.md 2>&1); rc=$?; got=$(verdicts)
  [ $rc -eq 1 ] && [ "$got" = "RED RED RED RED RED RED GREEN RED" ]
  ok $? "exit 1, zero cases, not judged, missing, no summary, absent file are each RED; green stays GREEN; the round is RED (rc=$rc: $got)"

  printf '| Feature | Cell |\n|---|---|\n| Green | checks/green.sh |\n' > green-man.md
  out=$(round_main t1 green-man.md 2>&1); rc=$?; got=$(verdicts)
  [ $rc -eq 0 ] && [ "$got" = "GREEN GREEN" ]; ok $? "an all-green manifest -> GREEN, rc=0 (rc=$rc: $got)"

  printf '| Feature | Cell |\n|---|---|\n| Crash | checks/crash.sh |\n' > crash-man.md
  out=$(round_main t1 crash-man.md 2>&1); rc=$?; got=$(verdicts)
  [ $rc -eq 1 ] && [ "$got" = "RED RED" ]; ok $? "a crash report newer than the cell start turns a passing cell RED (rc=$rc: $got)"

  printf '| Feature | Cell |\n|---|---|\n| Pattern | checks/{a,b}.sh |\n| Absent | checks/absent.sh |\n' > pat-man.md
  out=$(round_main t1 pat-man.md 2>&1); rc=$?
  printf '%s\n' "$out" | grep -q 'not one runnable file' && printf '%s\n' "$out" | grep -q 'not on this branch: checks/absent.sh'
  ok $? "a brace pattern says it is not one file; a plain absent file says it is not on this branch (rc=$rc)"

  printf 'sleep 60 &\necho $! > "%s/sleeper.pid"\nwait\necho "1 checks, 0 failed"\n' "$ST" > checks/sleeper.sh
  printf '| Feature | Cell | Budget |\n|---|---|---|\n| Sleeper | checks/sleeper.sh | 2 |\n| Green | checks/green.sh | |\n' > budget-man.md
  t0=$(date +%s); out=$(round_main t1 budget-man.md 2>&1); rc=$?; t1=$(( $(date +%s) - t0 ))
  reasons=$(python3 -c 'import json, sys
d = json.load(open(sys.argv[1]))
print(" | ".join(c["verdict"] + ":" + c["reason"] for c in d["cells"]))' "$vj" 2>&1)
  sleep 1; [ $rc -eq 1 ] && [ "$t1" -lt 30 ] && [ "$reasons" = "RED:over budget: 2 s (killed with its rig copy) | GREEN:" ] && ! kill -0 "$(cat "$ST/sleeper.pid")" 2> /dev/null
  ok $? "a sleeping cell over its manifest budget is killed with its children and reported over budget, the green cell beside it stays green (rc=$rc, ${t1}s: $reasons)"

  printf 'no table in this file\n' > empty-man.md
  out=$(round_main t1 empty-man.md 2>&1); rc=$?
  [ $rc -eq 2 ]; ok $? "a manifest with no Cell column is refused (rc=$rc)"

  out=$(round_main t9 green-man.md 2>&1); rc=$?
  [ $rc -eq 2 ]; ok $? "a label with no stamped build is refused (rc=$rc)"

  cd / && rm -rf "$ST"
  say "$ST_N checks, $ST_F failed, 0 not judged"
  [ $ST_F -eq 0 ]
}

case "${1:-}" in
  --selftest) selftest; exit $? ;;
  *) round_main "$@"; exit $? ;;
esac
