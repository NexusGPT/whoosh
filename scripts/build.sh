#!/bin/sh
# BUILD ONE COPY OF THE APP, STAMPED WITH WHAT IT WAS BUILT FROM.
#
#   sh scripts/build.sh <label> <worktree>
#   sh scripts/build.sh --dry <label> [<worktree>]     print what would run, compile nothing
#   sh scripts/build.sh --selftest
#
# Step 1 of native-desktop/pipeline.md. The build tree is $BUILD_ROOT/$APP_NAME-<label>. The bundle
# inside it carries five Info.plist keys (NexusBuildLabel, NexusBuildCommit, NexusBuildWorktree,
# NexusBuiltAt, NexusBuildDirtyFiles) and build-<label>.json sits in the build tree. There is no
# default worktree: a build from the wrong tree is how an install ends up older than the fix
# (--dry alone defaults to the repo this script sits in, and says so).
#
# SNAPSHOT: the compile never reads the live worktree. The script exports the commit it stamps
# (git worktree add --detach, into <build tree>/src), runs BUILD_CMD from that folder (the Rust build,
# gen-swift.sh and xcodebuild all run there) and removes the snapshot when it ends, on success, on
# failure and on a signal. A merge or an edit in the live worktree during a job cannot reach the
# bundle. An uncommitted change is NOT in the build: the script prints how many files that is.
# docs/pipeline.env is still read from the live worktree (it only names paths and commands).
#
# SLICES: WHOOSH_SLICES picks the Rust slices the compile links (read by scripts/gen-swift.sh).
# Default mac (a Mac bundle needs the Mac slice only); all builds the two iOS slices too; ios is for
# the iOS scripts. Any other value is refused with exit 2.
#
# Exit: the compiler's exit code; 2 for a refusal before the compile; 3 when the stamp cannot be
# written, signed or read back. DONE is printed only on exit 0.
#
# Config: the environment, then docs/pipeline.env in the worktree, written as `: "${VAR:=value}"`
# so the environment wins.
#   APP_NAME       bundle name without .app                                    required
#   BUILD_CMD      the compile, run in the worktree with $OUT set to the build tree
#   APP_PROJECT, APP_SCHEME   compose the default: xcodebuild … -derivedDataPath "$OUT" build
#   BUILD_ROOT     where build trees live                  default $HOME/Builds/$APP_NAME
#   BUNDLE_REL     the bundle's path inside the build tree default Build/Products/Release/$APP_NAME.app
#   SIGN_IDENTITY  codesign identity applied after stamping, "-" being ad hoc. EVERY copy is
#                  signed: there is no skip value, and a signature that fails fails the build. default -
set -u

say() { printf '%s\n' "$*"; }

SNAP_MADE=""; SNAP_REPO=""
# Remove the snapshot worktree: git first (so the repo forgets it), then the folder if git left it.
snap_cleanup() {
  [ -n "$SNAP_MADE" ] || return 0
  _s="$SNAP_MADE"; SNAP_MADE=""
  git -C "$SNAP_REPO" worktree remove --force "$_s" >> "$LOG" 2>&1
  if [ -e "$_s" ]; then rm -rf "$_s" >> "$LOG" 2>&1; git -C "$SNAP_REPO" worktree prune >> "$LOG" 2>&1; fi
  if [ -e "$_s" ]; then say "   snapshot NOT removed: $_s"; else say "   snapshot removed: $_s"; fi
}

build_main() {
  trap 'snap_cleanup; exit 130' INT TERM
  build_run "$@"; _rc=$?
  snap_cleanup
  trap - INT TERM
  return $_rc
}

build_run() {
  DRYRUN=0
  if [ "${1:-}" = "--dry" ]; then DRYRUN=1; shift; fi
  L="${1:-}"; WT="${2:-}"
  if [ $DRYRUN -eq 1 ] && [ -n "$L" ] && [ -z "$WT" ]; then
    WT=$(cd "$(dirname "$0")/.." && pwd -P)
    say "   (--dry without a worktree: using the repo this script sits in, $WT)"
  fi
  if [ -z "$L" ] || [ -z "$WT" ]; then
    say "usage: build.sh <label> <worktree>   (there is no default worktree)"; return 2
  fi
  case "$L" in *[!A-Za-z0-9._-]*) say "REFUSED: the label '$L' may hold only A-Z a-z 0-9 . _ -"; return 2 ;; esac
  if [ ! -d "$WT" ]; then say "REFUSED: no worktree at $WT"; return 2; fi
  WT=$(cd "$WT" && pwd -P)
  COMMIT=$(git -C "$WT" rev-parse HEAD 2>&1) || { say "REFUSED: $WT is not a git worktree: $COMMIT"; return 2; }
  if [ -f "$WT/docs/pipeline.env" ]; then . "$WT/docs/pipeline.env"; fi
  if [ -z "${APP_NAME:-}" ]; then say "REFUSED: APP_NAME is not set (environment or docs/pipeline.env)"; return 2; fi
  BUILD_ROOT="${BUILD_ROOT:-$HOME/Builds/$APP_NAME}"
  BUNDLE_REL="${BUNDLE_REL:-Build/Products/Release/$APP_NAME.app}"
  SIGN_IDENTITY="${SIGN_IDENTITY:--}"
  if [ -z "${BUILD_CMD:-}" ]; then
    if [ -n "${APP_PROJECT:-}" ] && [ -n "${APP_SCHEME:-}" ]; then
      BUILD_CMD='xcodebuild -project "$APP_PROJECT" -scheme "$APP_SCHEME" -configuration Release -derivedDataPath "$OUT" build'
    else
      say "REFUSED: set BUILD_CMD, or APP_PROJECT and APP_SCHEME"; return 2
    fi
  fi
  WHOOSH_SLICES="${WHOOSH_SLICES:-mac}"
  case "$WHOOSH_SLICES" in mac|ios|all) ;; *) say "REFUSED: WHOOSH_SLICES='$WHOOSH_SLICES' is not mac, ios or all"; return 2 ;; esac
  export WHOOSH_SLICES
  OUT="$BUILD_ROOT/$APP_NAME-$L"
  if [ -e "$OUT" ]; then say "REFUSED: $OUT exists. A label names one build; pick a new label."; return 2; fi
  # The compile runs in another folder, so it needs the build tree as an absolute path. The stamp, the
  # json and the messages keep $OUT exactly as before.
  case "$OUT" in /*) OUT_ABS="$OUT" ;; *) OUT_ABS="$(pwd -P)/$OUT" ;; esac
  SNAP="$OUT_ABS/src"
  # The snapshot's cargo writes into the live tree's core/target, so the Rust cache stays warm between builds.
  # Two builds sharing it would wait on cargo's lock; the owner's rule is one build at a time on this disk.
  CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$WT/core/target}"; export CARGO_TARGET_DIR
  LOG="$OUT/build.log"
  DIRTY=$(git -C "$WT" status --porcelain 2>&1 | grep -c .)
  say "== build $L  commit $COMMIT  uncommitted files $DIRTY (not in this build)"
  say "   worktree $WT"
  say "   snapshot $SNAP  (commit $COMMIT, removed when the build ends)"
  say "   slices   $WHOOSH_SLICES"
  say "   rust cache $CARGO_TARGET_DIR  (shared with the live tree)"
  say "   log      $LOG"
  if [ $DRYRUN -eq 1 ]; then
    say "DRY RUN: nothing is created and nothing is compiled. It would run, in this order:"
    say "   1. git -C \"$WT\" worktree add --detach \"$SNAP\" $COMMIT"
    say "   2. cd \"$SNAP\" && OUT=\"$OUT_ABS\" CARGO_TARGET_DIR=\"$CARGO_TARGET_DIR\" WHOOSH_SLICES=$WHOOSH_SLICES sh -c '$BUILD_CMD'   (log $LOG)"
    say "   3. stamp $OUT/$BUNDLE_REL/Contents/Info.plist (five Nexus* keys), codesign --sign \"$SIGN_IDENTITY\", write $OUT/build-$L.json"
    say "   4. git -C \"$WT\" worktree remove --force \"$SNAP\""
    say "BUILD $L DRY RUN OK commit=$COMMIT slices=$WHOOSH_SLICES snapshot=$SNAP"
    return 0
  fi
  # The no-build gates (cargo check, swift type-check, parity paths, Tests/) run first: a red gate refuses the build (item 65a92888).
  # GATES_CMD names what runs them (default: this worktree's scripts/gates.sh); the self-test below gives it a fake gate.
  GATES_CMD="${GATES_CMD:-sh $WT/scripts/gates.sh}"
  mkdir -p "$BUILD_ROOT" || return 2
  $GATES_CMD "$BUILD_ROOT/gates-$L" > "$BUILD_ROOT/gates-$L.out" 2>&1 || { say "REFUSED: gates red, see $BUILD_ROOT/gates-$L.out"; return 2; }
  mkdir -p "$OUT" || return 2
  git -C "$WT" worktree add --detach "$SNAP" "$COMMIT" > "$LOG" 2>&1 \
    || { say "REFUSED: could not export the snapshot of $COMMIT into $SNAP; see $LOG"; return 2; }
  SNAP_MADE="$SNAP"; SNAP_REPO="$WT"
  snap_head=$(git -C "$SNAP" rev-parse HEAD 2>&1)
  if [ "$snap_head" != "$COMMIT" ]; then say "REFUSED: the snapshot is at '$snap_head', expected $COMMIT"; return 2; fi
  ( cd "$SNAP" && OUT="$OUT_ABS" WHOOSH_SLICES="$WHOOSH_SLICES" CARGO_TARGET_DIR="$CARGO_TARGET_DIR" APP_PROJECT="${APP_PROJECT:-}" APP_SCHEME="${APP_SCHEME:-}" sh -c "$BUILD_CMD" ) >> "$LOG" 2>&1
  rc=$?
  if [ $rc -ne 0 ]; then
    say "BUILD $L FAILED rc=$rc errors=$(grep -c 'error:' "$LOG")"
    grep 'error:' "$LOG" | head -8
    return $rc
  fi
  APP="$OUT/$BUNDLE_REL"
  PL="$APP/Contents/Info.plist"
  if [ ! -f "$PL" ]; then say "BUILD $L STAMP FAILED: the compile exited 0 and left no bundle at $APP"; return 3; fi
  AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  for kv in "NexusBuildLabel=$L" "NexusBuildCommit=$COMMIT" "NexusBuildWorktree=$WT" "NexusBuiltAt=$AT" "NexusBuildDirtyFiles=$DIRTY"; do
    k=${kv%%=*}; v=${kv#*=}
    plutil -replace "$k" -string "$v" "$PL" >> "$LOG" 2>&1 || { say "BUILD $L STAMP FAILED: plutil could not write $k"; return 3; }
  done
  # LOCAL SYMBOLS LEAVE THE SHIPPED EXECUTABLE (owner 2026-11-12: "You're overbloating the app"; measured on main88: 196 MB, of which
  # 92 MB symbol tables; 114 MB after `strip -x`). KEEP_SYMBOLS=1 keeps them for a profiling build (perf/profile-aggregate.py names functions).
  EXE="$APP/Contents/MacOS/$APP_NAME"
  if [ "${KEEP_SYMBOLS:-0}" != 1 ] && [ -f "$EXE" ]; then
    b0=$(stat -f %z "$EXE"); xcrun strip -x "$EXE" >> "$LOG" 2>&1 || { say "BUILD $L STRIP FAILED; see $LOG"; return 3; }
    say "   stripped local symbols: $((b0 / 1048576)) MB -> $(($(stat -f %z "$EXE") / 1048576)) MB"
  fi
  xattr -cr "$APP" >> "$LOG" 2>&1
  codesign --force --deep --sign "$SIGN_IDENTITY" "$APP" >> "$LOG" 2>&1 \
    || { say "BUILD $L SIGN FAILED (identity $SIGN_IDENTITY); see $LOG"; return 3; }
  back=$(plutil -extract NexusBuildCommit raw -o - "$PL" 2>&1)
  if [ "$back" != "$COMMIT" ]; then say "BUILD $L STAMP FAILED: read back '$back', expected $COMMIT"; return 3; fi
  python3 - "$OUT/build-$L.json" "$L" "$COMMIT" "$WT" "$AT" "$DIRTY" "$APP" <<'PY'
import json, sys
p, label, commit, wt, at, dirty, app = sys.argv[1:]
with open(p, "w") as f:
    json.dump({"label": label, "commit": commit, "worktree": wt, "built_at": at,
               "uncommitted_files": int(dirty), "bundle": app}, f, indent=2)
PY
  if [ ! -f "$OUT/build-$L.json" ]; then say "BUILD $L STAMP FAILED: build-$L.json was not written"; return 3; fi
  say "BUILD $L DONE commit=$COMMIT bundle=$APP"
  return 0
}

selftest() {
  _t=${TMPDIR:-/tmp}; ST=$(mktemp -d "${_t%/}/nx-build-st.XXXXXX") || return 2
  ST_N=0; ST_F=0
  ok() { ST_N=$((ST_N+1)); if [ "$1" -eq 0 ]; then say "  PASS  $2"; else ST_F=$((ST_F+1)); say "  FAIL  $2"; fi; }
  R="$ST/repo"; mkdir -p "$R"
  git -C "$R" init -q > "$ST/git.log" 2>&1
  echo fine > "$R/src.txt"
  git -C "$R" add src.txt >> "$ST/git.log" 2>&1
  git -C "$R" -c user.email=selftest@example.invalid -c user.name=selftest commit -qm fixture >> "$ST/git.log" 2>&1
  export APP_NAME=Fake BUILD_ROOT="$ST/builds" SIGN_IDENTITY=-
  export GATES_CMD=true
  export BUILD_CMD='if grep -q BROKEN src.txt; then echo "src.txt:1: error: broken on purpose"; exit 65; fi
B="$OUT/Build/Products/Release/Fake.app/Contents"; mkdir -p "$B/MacOS"
printf "#!/bin/sh\nexit 0\n" > "$B/MacOS/Fake"; chmod +x "$B/MacOS/Fake"
plutil -create xml1 "$B/Info.plist" && plutil -insert CFBundleIdentifier -string com.example.fake "$B/Info.plist"'

  out=$(build_main "" "" 2>&1); rc=$?
  [ $rc -eq 2 ]; ok $? "no worktree named -> refused (rc=$rc)"

  echo BROKEN > "$R/src.txt"
  git -C "$R" -c user.email=selftest@example.invalid -c user.name=selftest commit -qam broken >> "$ST/git.log" 2>&1
  out=$(build_main bad "$R" 2>&1); rc=$?
  [ $rc -ne 0 ] && [ ! -f "$ST/builds/Fake-bad/build-bad.json" ]; ok $? "broken source -> rc!=0 and no build-bad.json (rc=$rc)"
  printf '%s\n' "$out" | grep -q "DONE"; [ $? -ne 0 ]; ok $? "broken source -> DONE is never printed"

  printf '%s\n' "$out" | grep -q "snapshot removed"; ok $? "broken source -> the snapshot is removed"
  git -C "$R" worktree list > "$ST/wl.log" 2>&1; [ "$(grep -c . "$ST/wl.log")" -eq 1 ]; ok $? "broken source -> git lists no snapshot worktree"

  echo fine > "$R/src.txt"
  git -C "$R" -c user.email=selftest@example.invalid -c user.name=selftest commit -qam fine >> "$ST/git.log" 2>&1
  echo BROKEN > "$R/src.txt"   # uncommitted: the snapshot must not see it
  out=$(build_main good "$R" 2>&1); rc=$?
  printf '%s\n' "$out" | grep -q "uncommitted files 1 (not in this build)"; ok $? "an uncommitted edit -> 'uncommitted files 1 (not in this build)' and the build still passes (rc=$rc)"
  [ ! -e "$ST/builds/Fake-good/src" ]; ok $? "good build -> no snapshot folder is left in the build tree"
  echo fine > "$R/src.txt"
  want=$(git -C "$R" rev-parse HEAD)
  got=$(plutil -extract NexusBuildCommit raw -o - "$ST/builds/Fake-good/Build/Products/Release/Fake.app/Contents/Info.plist" 2>&1)
  [ $rc -eq 0 ] && [ "$got" = "$want" ]; ok $? "good build -> the plist stamp equals git rev-parse HEAD (rc=$rc)"
  [ -f "$ST/builds/Fake-good/build-good.json" ]; ok $? "good build -> build-good.json in the build tree"

  out=$(build_main good "$R" 2>&1); rc=$?
  [ $rc -eq 2 ]; ok $? "a label used twice -> refused (rc=$rc)"

  export BUILD_CMD='printf "%s|%s|%s\n" "$(pwd -P)" "$WHOOSH_SLICES" "$CARGO_TARGET_DIR" > "$OUT/where.txt"; exit 7'
  out=$(build_main where "$R" 2>&1); rc=$?
  [ "$(cut -d'|' -f2 "$ST/builds/Fake-where/where.txt")" = "mac" ]; ok $? "WHOOSH_SLICES unset -> the compile sees mac"
  [ "$(cut -d'|' -f1 "$ST/builds/Fake-where/where.txt")" = "$(cd "$ST/builds/Fake-where" && pwd -P)/src" ]; ok $? "the compile runs inside the snapshot folder"
  [ "$(cut -d'|' -f3 "$ST/builds/Fake-where/where.txt")" = "$(cd "$R" && pwd -P)/core/target" ]; ok $? "the compile's cargo writes into the live tree's core/target"
  ( WHOOSH_SLICES=all build_main where2 "$R" ) > "$ST/w2.log" 2>&1
  [ "$(cut -d'|' -f2 "$ST/builds/Fake-where2/where.txt")" = "all" ]; ok $? "WHOOSH_SLICES=all -> the compile sees all"
  out=$(WHOOSH_SLICES=bogus build_main where3 "$R" 2>&1); rc=$?
  [ $rc -eq 2 ] && [ ! -e "$ST/builds/Fake-where3" ]; ok $? "WHOOSH_SLICES=bogus -> refused rc=2, nothing created (rc=$rc)"
  out=$(build_main --dry dry1 "$R" 2>&1); rc=$?
  [ $rc -eq 0 ] && [ ! -e "$ST/builds/Fake-dry1" ] && printf '%s\n' "$out" | grep -q "slices   mac"; ok $? "--dry -> rc=0, names the slices, creates nothing (rc=$rc)"

  ( GATES_CMD=false build_main gred "$R" ) > "$ST/gred.log" 2>&1; rc=$?
  [ $rc -eq 2 ] && [ ! -e "$ST/builds/Fake-gred" ] && grep -q "REFUSED: gates red" "$ST/gred.log"; ok $? "a red gate -> refused rc=2 and no build tree (rc=$rc)"
  ( GATES_CMD=true build_main gran "$R" ) > "$ST/gran.log" 2>&1
  [ -f "$ST/builds/gates-gran.out" ]; ok $? "a green gate runs once before the build, its output in gates-<label>.out"

  export BUILD_CMD='exit 0'
  out=$(build_main empty "$R" 2>&1); rc=$?
  [ $rc -eq 3 ]; ok $? "the compile exits 0 and leaves no bundle -> stamp failure rc=3 (rc=$rc)"

  rm -rf "$ST"
  say "$ST_N checks, $ST_F failed, 0 not judged"
  [ $ST_F -eq 0 ]
}

case "${1:-}" in
  --selftest) selftest; exit $? ;;
  *) build_main "$@"; exit $? ;;
esac
