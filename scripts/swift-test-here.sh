#!/bin/sh
# Run `swift test` from a lane's OWN worktree (tests-r58 round 6; before this a fresh worktree could not: "could not run the test target").
#
#   sh scripts/swift-test-here.sh [--filter <Suite or test>] [other swift test flags]
#
# What a fresh worktree lacks: Package.swift declares `.binaryTarget(path: "core/target/WhooshFFI.xcframework")`, built by scripts/gen-swift.sh into the MAIN tree's
# core/target only (core/target is not in git, and a sparse worktree leaves it out). This script links that one xcframework into this worktree's core/target
# (a symlink; nothing is built or copied), then runs `swift test` under the disk rule: it REFUSES below 8 GB free on /System/Volumes/Data or at a one-minute
# load of 30 or more (ALLOW_BUSY=1 skips the load test only), runs `nice -n 19` with `-j 2`, keeps the build folder OUTSIDE the tree (SWIFT_SCRATCH, default
# /private/tmp/whoosh-swift-<pid>; the first build compiles MLX, about 3 GB) and deletes it at the end (KEEP_SCRATCH=1 keeps it for a rerun).
# Output: the swift test log is $SWIFT_SCRATCH.log; the last `Test run with N tests` line and the `error:` count are printed. Exit code = swift test's.
set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
MAIN=$(git -C "$ROOT" worktree list --porcelain | sed -n '1s/^worktree //p')
XC="$MAIN/core/target/WhooshFFI.xcframework"
# WHOOSH_FFI_XCFRAMEWORK=<folder>: a library built elsewhere (the lead's keep-ffi copy) replaces the main tree's. Its BUILT-FROM file holds the commit it was built from;
# it is refused unless the contract and the generated Swift of that commit equal this tree's (what scripts/gen-swift.sh --check compares: the contracts crate, the ffi crate, Sources/CoreBridge/Generated).
if [ -n "${WHOOSH_FFI_XCFRAMEWORK:-}" ]; then
  [ -d "$WHOOSH_FFI_XCFRAMEWORK" ] || { echo "REFUSED: WHOOSH_FFI_XCFRAMEWORK=$WHOOSH_FFI_XCFRAMEWORK is not a folder"; exit 2; }
  FROM=$(tr -d ' \n' < "$(dirname "$WHOOSH_FFI_XCFRAMEWORK")/BUILT-FROM" 2>&1) || { echo "REFUSED: no BUILT-FROM beside $WHOOSH_FFI_XCFRAMEWORK"; exit 2; }
  if ! git -C "$ROOT" diff --quiet "$FROM" HEAD -- core/crates/contracts core/crates/ffi Sources/CoreBridge/Generated; then
    echo "REFUSED: $WHOOSH_FFI_XCFRAMEWORK was built from $FROM; its contract differs from this tree's HEAD"; exit 2
  fi
  echo "using library $WHOOSH_FFI_XCFRAMEWORK built from $FROM (contract equals this tree)"
  XC="$WHOOSH_FFI_XCFRAMEWORK"
  rm -f "$ROOT/core/target/WhooshFFI.xcframework" 2>/dev/null
fi
# this worktree's OWN built xcframework (WHOOSH_SLICES=mac sh scripts/gen-swift.sh run here) is used as it is; else the main tree's is linked
if [ ! -e "$ROOT/core/target/WhooshFFI.xcframework" ] && [ ! -d "$XC" ]; then
  echo "REFUSED: neither $ROOT/core/target/WhooshFFI.xcframework nor $XC exists: no built xcframework (WHOOSH_SLICES=mac sh scripts/gen-swift.sh in this worktree builds one)"; exit 2
fi
free=$(df -g /System/Volumes/Data | awk 'NR==2 {print $4}')
[ "${free:-0}" -ge 8 ] || { echo "REFUSED: ${free} GB free on /System/Volumes/Data, the build needs 8"; exit 2; }
load=$(sysctl -n vm.loadavg | awk '{print int($2)}')
if [ "${ALLOW_BUSY:-0}" != 1 ] && [ "$load" -ge 30 ]; then echo "REFUSED: one-minute load $load, the limit is 30 (ALLOW_BUSY=1 skips this test)"; exit 2; fi
if [ ! -e "$ROOT/core/target/WhooshFFI.xcframework" ]; then
  mkdir -p "$ROOT/core/target" && ln -s "$XC" "$ROOT/core/target/WhooshFFI.xcframework" || exit 2
  echo "linked $ROOT/core/target/WhooshFFI.xcframework -> $XC"
fi
SCRATCH=${SWIFT_SCRATCH:-/private/tmp/whoosh-swift-$$}
case "$SCRATCH" in "$ROOT"/*) echo "REFUSED: SWIFT_SCRATCH must be outside the tree"; exit 2 ;; esac
cd "$ROOT" || exit 2
# the same two variables the round's cells pass (journeys/lib/cells/_swift.sh): without them CoreHost falls back to the real/unavailable core and the seam tests judge nothing real
export WHOOSH_FIXTURE_CORE=1 WHOOSH_FIXTURE_PATH="$ROOT/docs/design/fixtures/mac-main-sample.json"
export CARGO_INCREMENTAL=0
# --no-parallel: the unit tests share one AppModel and one fixture core (Tests/Support/SharedAppFresh.swift); async @MainActor tests that sleep (ChangesRefreshTests) let another
# test run on the main actor in between, so a parallel run gave a different red set every time (main76 vs the first lane run). One test at a time is the verdict that repeats.
nice -n 19 swift test --scratch-path "$SCRATCH" -j 2 --no-parallel "$@" > "$SCRATCH.log" 2>&1
rc=$?
echo "errors: $(grep -c 'error:' "$SCRATCH.log")"
grep -E "Test run with [0-9]+ tests|Executed [0-9]+ tests" "$SCRATCH.log" | tail -2
echo "swift test exit $rc (log $SCRATCH.log)"
[ "${KEEP_SCRATCH:-0}" = 1 ] || rm -rf "$SCRATCH"
exit $rc
