#!/bin/sh
# The two no-build gates every main build passes first.
#   1. cargo check of the whole Rust workspace, tests included.
#   2. swiftc -typecheck of every Swift source, function bodies included.
#   3. (below) parity-paths; 4. Tests/** type-checked against the Sources module, MLX-only types excepted.
# Exit 0 only when all are clean. Exit 1 on a Rust error, 2 on a Swift error, 4 on an error in Tests/ (step 4),
# 3 when the tree changed while the gates ran (the result is then void).
#
# Limit: Swift 6 "sending ... risks causing data races" errors are found by a
# build only, not by -typecheck.
#
# Known noise (item 65a92888: "lists only real errors, known noise filtered and named"): the whole-tree swiftc steps (2 and 3) list every "error:" line
# unless GATES_SWIFT_NOISE names a file of extended regexes, one per line; the lines they match are counted and named in the output, never hidden.
# The Swift tests step (4) filters the MLX-only names itself and says so (scripts/gate-tests.sh).
#
# Usage: sh scripts/gates.sh [log-dir]
#        sh scripts/gates.sh --selftest-filter [log-dir]   the noise filter on sample lines, compiles nothing, one PASS or FAIL line per case
set -u
# keep_real_errors <raw error lines> <step name>: writes <raw>.real (the lines no noise pattern matches) and prints the noise line.
keep_real_errors() {
  if [ -n "${GATES_SWIFT_NOISE:-}" ] && [ -s "$GATES_SWIFT_NOISE" ]; then
    grep -vE -f "$GATES_SWIFT_NOISE" "$1" > "$1.real" || true
    echo "noise filtered ($2): $(grep -cE -f "$GATES_SWIFT_NOISE" "$1" || true) line(s) matching the patterns in $GATES_SWIFT_NOISE"
  else
    cp "$1" "$1.real"
    echo "noise filtered ($2): 0 line(s), no noise pattern set (GATES_SWIFT_NOISE)"
  fi
}
if [ "${1:-}" = "--selftest-filter" ]; then
  ST=${2:-${TMPDIR:-/tmp}/whoosh-gates}/gates-selftest; mkdir -p "$ST"
  printf '%s\n' "A.swift:1:1: error: known noise one" "B.swift:2:2: error: a real error" "C.swift:3:3: error: known noise two" > "$ST/raw.txt"
  printf '%s\n' "known noise" > "$ST/noise-patterns.txt"
  BAD=0
  check() { if [ "$2" = "$3" ]; then echo "PASS $1"; else echo "FAIL $1: expected '$3', got '$2'"; BAD=1; fi; }
  LINE=$(GATES_SWIFT_NOISE="$ST/noise-patterns.txt" keep_real_errors "$ST/raw.txt" selftest)
  check "noise-counted-and-named" "$LINE" "noise filtered (selftest): 2 line(s) matching the patterns in $ST/noise-patterns.txt"
  check "only-the-real-error-is-left" "$(cat "$ST/raw.txt.real")" "B.swift:2:2: error: a real error"
  LINE=$(GATES_SWIFT_NOISE="" keep_real_errors "$ST/raw.txt" selftest)
  check "no-pattern-says-so" "$LINE" "noise filtered (selftest): 0 line(s), no noise pattern set (GATES_SWIFT_NOISE)"
  check "no-pattern-keeps-every-line" "$(wc -l < "$ST/raw.txt.real" | tr -d ' ')" 3
  [ "$BAD" -eq 0 ] && echo "gates selftest-filter: GREEN" && exit 0
  echo "gates selftest-filter: RED"; exit 1
fi
ROOT=$(cd "$(dirname "$0")/.." && pwd)
LOGS=${1:-${TMPDIR:-/tmp}/whoosh-gates}
mkdir -p "$LOGS"
CARGO=${CARGO:-$HOME/.cargo/bin/cargo}
cd "$ROOT" || exit 3
HEAD_BEFORE=$(git rev-parse HEAD)
DIRTY_BEFORE=$(git status --porcelain -- Sources core | cksum)
echo "gates at $(git rev-parse --short HEAD)"

# T-02 (tests-r58): every state path the shell reads has a row in docs/STATE-PATHS.json, and every async command lands on a path the fixture serves.
python3 checks/parity-paths.py > "$LOGS/gate-state-paths.log" 2>&1 || { cat "$LOGS/gate-state-paths.log"; echo "GATE RED: parity-paths (log: $LOGS/gate-state-paths.log)"; exit 1; }
echo "parity-paths: $(tail -1 "$LOGS/gate-state-paths.log")"
# The generated constants equal the ledger (main78: the gates were green and the build stopped at its first step, because
# three merges added rows to docs/CONSTANTS.md and nobody ran tools/gen-constants.py; build-mac.sh checks it, the gates did not).
python3 tools/gen-constants.py --check > "$LOGS/gate-constants.log" 2>&1 || { cat "$LOGS/gate-constants.log"; echo "GATE RED: constants drift: run python3 tools/gen-constants.py (log: $LOGS/gate-constants.log)"; exit 1; }
echo "constants: $(tail -1 "$LOGS/gate-constants.log")"

(
  cd core &&
  CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 \
  CARGO_TARGET_DIR=${WHOOSH_CHECK_TARGET:-$ROOT/core/target-check} \
  "$CARGO" check --workspace --tests --offline
) > "$LOGS/gate-cargo.log" 2>&1
CE=$?
if [ "$CE" -ne 0 ]; then
  grep -E "^error" "$LOGS/gate-cargo.log" | head -20
  echo "GATE RED: cargo check exit $CE (log: $LOGS/gate-cargo.log)"
  exit 1
fi
echo "cargo check: clean"

# The run must reach function bodies. Two things make swiftc stop before them and report
# almost nothing: Bundle.module unresolved (it exists only inside a package build) and the
# FFI header not found. So: a one-line stub for Bundle.module, the FFI include path, and the
# app's own SDK, target and module name. Every Swift file is read, the iPhone folders too.
STUB="$LOGS/BundleModuleStub.swift"
echo 'import Foundation
extension Bundle { static let module = Bundle.main }' > "$STUB"
# Steps 2 and 3 are off by default since 2026-11-12 (owner: "I'm expecting you to move much faster"; measured: 20 minutes at low priority
# before every build, and the build's own compile reports the same errors). GATES_SWIFT=1 runs them (a check before a long unattended build).
# Package modules the Sources import outside `#if canImport` (16 Nov: KeyboardShortcuts, the notch's key, since 14 Nov). They exist only in a build's
# products folder, so GATES_PKG_MODULES names one (build/Whoosh-<label>/Build/Products/Release). Only the modules of GATES_PKG_NAMES are linked into
# $LOGS/pkg-modules and given to swiftc: the MLX modules stay unknown on purpose (code under `#if canImport(MLX` is the build's to check).
PKGDIR=""
if [ -n "${GATES_PKG_MODULES:-}" ]; then
  PKGDIR="$LOGS/pkg-modules"; mkdir -p "$PKGDIR"
  for m in ${GATES_PKG_NAMES:-KeyboardShortcuts}; do
    if [ -e "$GATES_PKG_MODULES/$m.swiftmodule" ]; then ln -sfn "$GATES_PKG_MODULES/$m.swiftmodule" "$PKGDIR/$m.swiftmodule"; else echo "package module $m: not in $GATES_PKG_MODULES"; fi
  done
fi
SE=0; SR=0; SN=0; LE=0; LR=0; LN=0
if [ "${GATES_SWIFT:-0}" = 1 ]; then
[ -n "$PKGDIR" ] || echo "package modules: none given (GATES_PKG_MODULES): an import of a package outside canImport will read as an error"
{ find Sources -name '*.swift' -print0; printf '%s\0' "$STUB"; } |
  xargs -0 swiftc -typecheck -swift-version 6 -sdk "$(xcrun --sdk macosx --show-sdk-path)" \
    -target arm64-apple-macos15 -module-name WhooshUI ${PKGDIR:+-I} ${PKGDIR:+"$PKGDIR"} \
    -I core/crates/ffi/swift/WhooshFFI/include > "$LOGS/gate-swift.log" 2>&1
SE=$?
grep "error:" "$LOGS/gate-swift.log" > "$LOGS/gate-swift-errors.log"
keep_real_errors "$LOGS/gate-swift-errors.log" "swift type-check"
SR=$(wc -l < "$LOGS/gate-swift-errors.log" | tr -d ' ')
SN=$(wc -l < "$LOGS/gate-swift-errors.log.real" | tr -d ' ')

# 3. What a build diagnoses and -typecheck does not (first seen: a closure with a missing return, which
# stopped a main build after both gates were green): -emit-sil of the whole module, no code generated.
{ find Sources -name '*.swift' -print0; printf '%s\0' "$STUB"; } |
  xargs -0 swiftc -emit-sil -wmo -Onone -swift-version 6 -sdk "$(xcrun --sdk macosx --show-sdk-path)" \
    -target arm64-apple-macos15 -module-name WhooshUI ${PKGDIR:+-I} ${PKGDIR:+"$PKGDIR"} \
    -I core/crates/ffi/swift/WhooshFFI/include -o "$LOGS/gate-sil.sil" > "$LOGS/gate-sil.log" 2>&1
LE=$?
rm -f "$LOGS/gate-sil.sil"
grep "error:" "$LOGS/gate-sil.log" > "$LOGS/gate-sil-errors.log"
keep_real_errors "$LOGS/gate-sil-errors.log" "emit-sil"
LR=$(wc -l < "$LOGS/gate-sil-errors.log" | tr -d ' ')
LN=$(wc -l < "$LOGS/gate-sil-errors.log.real" | tr -d ' ')
else
  echo "swift type-check and emit-sil: not run (GATES_SWIFT=1 runs them; the build's compile reports the same errors)"
fi

# 4. Tests/** type-checked against the Sources module (scripts/gate-tests.sh: what it does and what it cannot see). Exit 4 on a test error.
# Off by default since 2026-11-12 (owner: "stop with the 700+ swift tests"): the step added 13 minutes to every build. GATES_SWIFT_TESTS=1 runs it.
if [ "${GATES_SWIFT_TESTS:-0}" = 1 ]; then sh scripts/gate-tests.sh "$LOGS" "$STUB" || exit 4; else echo "swift tests type-check: not run (GATES_SWIFT_TESTS=1 runs it)"; fi

if [ "$(git rev-parse HEAD)" != "$HEAD_BEFORE" ] ||
   [ "$(git status --porcelain -- Sources core | cksum)" != "$DIRTY_BEFORE" ]; then
  echo "GATE VOID: the tree changed while the gates ran; run again"
  exit 3
fi
# A step that did not run says so above and prints no "clean" line (16 Nov: the default run printed "swift type-check: clean" for two steps it had skipped).
if [ "${GATES_SWIFT:-0}" != 1 ]; then echo "GATES GREEN at $(git rev-parse --short HEAD)"; exit 0; fi
# red on a real error line, or on a failed run that printed no error line at all (a crash); a run that failed only on noise lines is not red
if [ "$SN" -ne 0 ] || { [ "$SE" -ne 0 ] && [ "$SR" -eq 0 ]; }; then
  head -20 "$LOGS/gate-swift-errors.log.real"
  echo "GATE RED: swift type-check exit $SE, $SN error line(s) (log: $LOGS/gate-swift-errors.log)"
  exit 2
fi
echo "swift type-check: clean"
if [ "$LN" -ne 0 ] || { [ "$LE" -ne 0 ] && [ "$LR" -eq 0 ]; }; then
  sort -u "$LOGS/gate-sil-errors.log.real" | head -20
  echo "GATE RED: swift build diagnostics (emit-sil) exit $LE, $LN error line(s) (log: $LOGS/gate-sil-errors.log)"
  exit 2
fi
echo "swift build diagnostics: clean"
echo "GATES GREEN at $(git rev-parse --short HEAD)"
