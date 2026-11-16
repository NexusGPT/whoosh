#!/bin/sh
# Gate 4 of scripts/gates.sh (tests-r58 round 6): the Swift tests type-checked against the Sources module, no MLX build, nothing run.
#   sh scripts/gate-tests.sh [log-dir] [Bundle.module-stub.swift]      exit 0 clean, 4 on a test error or when the Sources module cannot be emitted
set -u
# The known noise, filtered and named (item 65a92888: "lists only real errors, known noise filtered and named").
#   split_noise <all-error-lines> <pattern-file> <real-out> <noise-out>   one extended regex per line in the pattern file
#   noise_names <noise-out>                                               the distinct names the noise lines say they cannot find
split_noise() {
  if [ -s "$2" ]; then
    grep -vE -f "$2" "$1" > "$3" || true
    grep -E -f "$2" "$1" > "$4" || true
  else
    cp "$1" "$3"; : > "$4"
  fi
}
noise_names() { sed -nE "s/.*cannot find (type )?'([^']*)' in scope.*/\2/p" "$1" | sort -u | tr '\n' ' ' | sed -E 's/ $//'; }
# `sh scripts/gate-tests.sh --selftest-filter [log-dir]`: runs the filter above on sample lines, compiles nothing, prints one PASS or FAIL line per case.
if [ "${1:-}" = "--selftest-filter" ]; then
  ST=${2:-${TMPDIR:-/tmp}/whoosh-gates}/tests-gate-selftest; mkdir -p "$ST"
  printf '%s\n' "Tests/A.swift:1:1: error: cannot find 'LayaEngine' in scope" "Tests/B.swift:2:2: error: cannot find type 'GemmaEmbedder' in scope" \
    "Tests/C.swift:3:3: error: cannot find 'RealThing' in scope" "Tests/D.swift:4:4: error: value of type 'X' has no member 'y'" > "$ST/all.txt"
  printf '%s\n' "LayaEngine" "GemmaEmbedder" | sed -E "s/^(.*)$/cannot find (type )?'\\1' in scope/" > "$ST/pattern.txt"
  split_noise "$ST/all.txt" "$ST/pattern.txt" "$ST/real.txt" "$ST/noise.txt"
  BAD=0
  check() { if [ "$2" = "$3" ]; then echo "PASS $1"; else echo "FAIL $1: expected '$3', got '$2'"; BAD=1; fi; }
  check "noise-lines-counted" "$(wc -l < "$ST/noise.txt" | tr -d ' ')" 2
  check "real-lines-kept" "$(wc -l < "$ST/real.txt" | tr -d ' ')" 2
  check "real-error-about-an-unknown-name-is-not-noise" "$(grep -c "RealThing" "$ST/real.txt")" 1
  check "noise-named" "$(noise_names "$ST/noise.txt")" "GemmaEmbedder LayaEngine"
  : > "$ST/empty-pattern.txt"
  split_noise "$ST/all.txt" "$ST/empty-pattern.txt" "$ST/real2.txt" "$ST/noise2.txt"
  check "no-pattern-no-filter" "$(wc -l < "$ST/real2.txt" | tr -d ' ') $(wc -l < "$ST/noise2.txt" | tr -d ' ')" "4 0"
  [ "$BAD" -eq 0 ] && echo "gate-tests selftest-filter: GREEN" && exit 0
  echo "gate-tests selftest-filter: RED"; exit 4
fi
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT" || exit 4
LOGS=${1:-${TMPDIR:-/tmp}/whoosh-gates}; mkdir -p "$LOGS"
STUB=${2:-$LOGS/BundleModuleStub.swift}
[ -f "$STUB" ] || printf 'import Foundation\nextension Bundle { static let module = Bundle.main }\n' > "$STUB"
# 4. Tests/** type-checked against the Sources module (tests-r58 round 6). The three steps above compile Sources only, so a test file that does not compile was
# invisible until a round ran `swift test` (main55 to main70: the package's one test target never compiled, and no Swift test ran). Without building MLX:
#   a. `swiftc -emit-module -enable-testing` over Sources (+ the same Bundle.module stub), function bodies skipped (step 3 already read them): the module the tests
#      `@testable import`; about a minute.
#   b. `swiftc -typecheck` over every Tests/**/*.swift against that module, with the Testing framework (`-F` the Platforms folder, `-plugin-path` the testing macros).
# What it cannot see: code under `#if canImport(MLX` (GemmaEmbedder, MlxBackend, LayaEngine ...) does not exist without MLX, so an error "cannot find '<type>' in
# scope" for a type declared in such a file is dropped (a test using one is not checked); and no test is RUN. GATES_TESTS_DIR points the check at another copy of Tests/.
TESTS_DIR=${GATES_TESTS_DIR:-Tests}
DEVDIR=$(xcode-select -p)
TG="$LOGS/tests-gate"; mkdir -p "$TG"
# Package modules imported outside `#if canImport` (scripts/gates.sh says why): the modules of GATES_PKG_NAMES from GATES_PKG_MODULES, linked into $LOGS/pkg-modules.
PKGDIR=""
if [ -n "${GATES_PKG_MODULES:-}" ]; then
  PKGDIR="$LOGS/pkg-modules"; mkdir -p "$PKGDIR"
  for m in ${GATES_PKG_NAMES:-KeyboardShortcuts}; do
    [ -e "$GATES_PKG_MODULES/$m.swiftmodule" ] && ln -sfn "$GATES_PKG_MODULES/$m.swiftmodule" "$PKGDIR/$m.swiftmodule"
  done
fi
{ find Sources -name '*.swift' -print0; printf '%s\0' "$STUB"; } |
  xargs -0 nice -n 19 swiftc -emit-module -enable-testing -parse-as-library -swift-version 6 -sdk "$(xcrun --sdk macosx --show-sdk-path)" \
    -target arm64-apple-macos15 -module-name WhooshUI ${PKGDIR:+-I} ${PKGDIR:+"$PKGDIR"} -I core/crates/ffi/swift/WhooshFFI/include \
    -emit-module-path "$TG/WhooshUI.swiftmodule" -Xfrontend -experimental-skip-all-function-bodies > "$TG/emit.log" 2>&1
TE=$?
if [ "$TE" -ne 0 ] || [ ! -f "$TG/WhooshUI.swiftmodule" ]; then
  grep -E '^[^ ].*: error:' "$TG/emit.log" | head -10
  echo "GATE RED: the Sources module for the tests could not be emitted (exit $TE, log: $TG/emit.log)"
  exit 4
fi
find "$TESTS_DIR" -name '*.swift' -print0 |
  xargs -0 nice -n 19 swiftc -typecheck -swift-version 6 -sdk "$(xcrun --sdk macosx --show-sdk-path)" -target arm64-apple-macos15 -module-name WhooshTests \
    -I "$TG" ${PKGDIR:+-I} ${PKGDIR:+"$PKGDIR"} -I core/crates/ffi/swift/WhooshFFI/include -F "$DEVDIR/Platforms/MacOSX.platform/Developer/Library/Frameworks" \
    -plugin-path "$DEVDIR/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift/host/plugins/testing" > "$TG/typecheck.log" 2>&1
grep -rl 'canImport(MLX' Sources | xargs grep -hoE '^[[:space:]]*(public |final |private |fileprivate |internal )*(class|struct|enum|actor|protocol|typealias) +[A-Za-z0-9_]+' |
  awk '{print $NF}' | sort -u > "$TG/mlx-only-names.txt"
grep -E '^[^ ].*: error:' "$TG/typecheck.log" | sort -u > "$TG/all-errors.txt" || true
: > "$TG/mlx-pattern.txt"
if [ -s "$TG/mlx-only-names.txt" ]; then
  sed -E "s/^(.*)$/cannot find (type )?'\\1' in scope/" "$TG/mlx-only-names.txt" > "$TG/mlx-pattern.txt"
fi
split_noise "$TG/all-errors.txt" "$TG/mlx-pattern.txt" "$TG/test-errors.txt" "$TG/test-noise.txt"
NN=$(wc -l < "$TG/test-noise.txt" | tr -d ' ')
echo "noise filtered: $NN line(s) of MLX-only names not found: $(noise_names "$TG/test-noise.txt") (list: $TG/mlx-only-names.txt, lines: $TG/test-noise.txt)"
TN=$(wc -l < "$TG/test-errors.txt" | tr -d ' ')
if [ "$TN" -ne 0 ]; then
  head -20 "$TG/test-errors.txt"
  echo "GATE RED: $TN error line(s) in the Swift tests (log: $TG/typecheck.log)"
  exit 4
fi
echo "swift tests type-check: clean"

