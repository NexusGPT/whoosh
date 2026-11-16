#!/bin/sh
# GENERATE THE SWIFT SIDE OF THE CONTRACT, AND BUILD THE STATIC LIBRARY SWIFT LINKS.
#
#   sh scripts/gen-swift.sh            regenerate, then build libwhoosh_ffi.a for macOS (and iOS when its targets exist)
#   sh scripts/gen-swift.sh --check    exit 1 when the committed Swift differs from what the contract generates
#
# WHOOSH_SLICES picks the Rust slices to build, pre-link and pack into the xcframework (default all, as before):
#   mac   aarch64-apple-darwin                              (scripts/build.sh sets it for a Mac bundle)
#   ios   aarch64-apple-ios-sim and aarch64-apple-ios       (an iOS build passes it)
#   all   the three
# Any other value is refused with exit 2.
#
# The contract crate is the single source; Sources/CoreBridge/Generated/contracts.swift and the C
# header module in core/crates/ffi/swift/WhooshFFI/include are DERIVED and never edited by hand.
# Static libraries land in core/target/<triple>/release/libwhoosh_ffi.a; each is then PRE-LINKED into
# core/target/<triple>/release/prelinked/libwhoosh_ffi.a, the archive the xcframework packs (Package.swift links it).
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd -P)
export PATH="$HOME/.cargo/bin:$PATH"
# Incremental dev builds drop UniFFI metadata statics from the dylib (measured 2026-11-07: MessageBody and
# OnlinePort vanished from the generated Swift after a merge); library-mode generation needs a non-incremental build.
export CARGO_INCREMENTAL=0
CORE="$ROOT/core"
# Cargo output: CARGO_TARGET_DIR when set (scripts/build.sh points a snapshot at the live tree's warm cache), else core/target.
# The xcframework always lands in $CORE/target, where Package.swift reads it.
TARGET="${CARGO_TARGET_DIR:-$CORE/target}"
case "${WHOOSH_SLICES:-all}" in
  mac) TRIPLES="aarch64-apple-darwin" ;;
  ios) TRIPLES="aarch64-apple-ios-sim aarch64-apple-ios" ;;
  all) TRIPLES="aarch64-apple-darwin aarch64-apple-ios-sim aarch64-apple-ios" ;;
  *) echo "REFUSED: WHOOSH_SLICES='${WHOOSH_SLICES}' is not mac, ios or all" >&2; exit 2 ;;
esac
TMP=$(mktemp -d "${TMPDIR:-/tmp}/whoosh-gen.XXXXXX")
trap 'rm -rf "$TMP"' EXIT
( cd "$CORE" && cargo build -j "${WHOOSH_JOBS:-2}" -q -p ffi --lib ) 
( cd "$CORE" && cargo run -j "${WHOOSH_JOBS:-2}" -q -p ffi --bin uniffi-bindgen -- generate --library "$TARGET/debug/libwhoosh_ffi.dylib" \
    --language swift --out-dir "$TMP" --no-format ) 
# One C module for both namespaces (real core slice 1): contracts.swift and engine.swift compile into the same Swift
# module, so they must see ONE RustBuffer. Two clang modules would each define it; one module holding both headers keeps
# the second definition behind the headers' shared UNIFFI_SHARED_H guard. engine.swift is pointed at that module.
sed -e 's/canImport(engineFFI)/canImport(contractsFFI)/' -e 's/^import engineFFI$/import contractsFFI/' "$TMP/engine.swift" > "$TMP/engine.swift.one" && mv "$TMP/engine.swift.one" "$TMP/engine.swift"
SWIFT_OUT="$ROOT/Sources/CoreBridge/Generated"
INC="$ROOT/core/crates/ffi/swift/WhooshFFI/include"
if [ "${1:-}" = "--check" ]; then
  rc=0
  cmp -s "$TMP/contracts.swift" "$SWIFT_OUT/contracts.swift" || { echo "DRIFT: Sources/CoreBridge/Generated/contracts.swift"; rc=1; }
  cmp -s "$TMP/contractsFFI.h" "$INC/contractsFFI.h" || { echo "DRIFT: core/crates/ffi/swift/WhooshFFI/include/contractsFFI.h"; rc=1; }
  cmp -s "$TMP/engine.swift" "$SWIFT_OUT/engine.swift" || { echo "DRIFT: Sources/CoreBridge/Generated/engine.swift"; rc=1; }
  cmp -s "$TMP/engineFFI.h" "$INC/engineFFI.h" || { echo "DRIFT: core/crates/ffi/swift/WhooshFFI/include/engineFFI.h"; rc=1; }
  [ $rc -eq 0 ] && echo "generated Swift matches the contract"
  exit $rc
fi
mkdir -p "$SWIFT_OUT" "$INC"
cp "$TMP/contracts.swift" "$SWIFT_OUT/contracts.swift"
cp "$TMP/contractsFFI.h" "$INC/contractsFFI.h"
# The real core (slice 1, 2026-11-09) is its own UniFFI namespace, "engine": open_rust_core returns the contract's WhooshCore.
cp "$TMP/engine.swift" "$SWIFT_OUT/engine.swift"
cp "$TMP/engineFFI.h" "$INC/engineFFI.h"
printf 'module contractsFFI {\n    header "contractsFFI.h"\n    header "engineFFI.h"\n    export *\n}\n' > "$INC/module.modulemap"
# PRE-LINK (docs/lanes/ffi-link.md): a Rust staticlib carries its own copy of the Rust runtime (std, core, alloc,
# the unmangled _rust_eh_personality). A second Rust static library in the same link (FluidAudio's
# libtext_processing_rs.a) collides with it: "duplicate symbol '_rust_eh_personality'". One relocatable object per
# slice keeps every runtime symbol inside it as non-external and exports only the UniFFI entry points the header declares.
# The minimum OS versions come from Package.swift's platforms line, the one place they live.
MACOS_MIN=$(sed -nE 's/.*\.macOS\(\.v([0-9]+)\).*/\1/p' "$ROOT/Package.swift" | head -1).0
IOS_MIN=$(sed -nE 's/.*\.iOS\(\.v([0-9]+)\).*/\1/p' "$ROOT/Package.swift" | head -1).0
cat "$INC/contractsFFI.h" "$INC/engineFFI.h" | grep -oE '\b(uniffi|ffi)_[A-Za-z0-9_]+\(' | tr -d '(' | sort -u | sed 's/^/_/' > "$TMP/needed.txt"
printf '_uniffi_*\n_ffi_*\n' > "$TMP/exports.txt"
prelink() {  # $1 rust triple, $2 ld platform name, $3 minimum OS version
  src="$TARGET/$1/release/libwhoosh_ffi.a"
  out="$TARGET/$1/release/prelinked"
  rm -rf "$out"
  mkdir -p "$out"
  # shellcheck disable=SC2046 # one -u per entry point, word-split on purpose
  xcrun ld -r -arch arm64 -platform_version "$2" "$3" "$3" -exported_symbols_list "$TMP/exports.txt" \
    $(sed 's/^/-u /' "$TMP/needed.txt") -o "$out/whoosh_ffi.o" "$src"
  xcrun libtool -static -o "$out/libwhoosh_ffi.a" "$out/whoosh_ffi.o"
  # The check that can fail: exactly the header's entry points are exported, and no Rust runtime symbol is.
  # --no-llvm-bc: the object embeds Rust's LLVM bitcode, which Xcode's nm cannot parse; read the machine code only.
  # One C symbol is allowed beside the entry points, by name: `_nk_dispatch_table`, a COMMON data symbol of numkong (the
  # vector maths under usearch, linked since the engine indexes mail for meaning). `ld -r` cannot make a common symbol
  # private and `nmedit` does not either (both tried 2026-11-10); a common symbol merges at the final link, so it cannot
  # raise the duplicate-symbol error this check exists for. Any other extra symbol still fails the check.
  exported=$(xcrun nm --no-llvm-bc -gU "$out/whoosh_ffi.o" | awk 'NF>=3 {print $3}' | grep -v -x '_nk_dispatch_table' | sort)
  if [ "$exported" != "$(cat "$TMP/needed.txt")" ]; then
    echo "PRE-LINK FAILED for $1: exported symbols differ from the header's $(wc -l < "$TMP/needed.txt" | tr -d ' ') entry points" >&2
    printf '%s\n' "$exported" | diff "$TMP/needed.txt" - | head -20 >&2
    exit 1
  fi
  echo "pre-linked core/target/$1/release/prelinked/libwhoosh_ffi.a ($(wc -l < "$TMP/needed.txt" | tr -d ' ') symbols exported)"
}
for t in $TRIPLES; do
  if rustup target list --installed | grep -qx "$t"; then
    # The deployment target must reach rustc AND the cc crate (ring, libsqlite3-sys, usearch). Without it rustc links
    # at its own default (iOS 10.0) while the C objects take the SDK's newest (26.5): "built for newer iOS version", and
    # at 10.0 libSystem does not offer ___chkstk_darwin (iOS 13+), which the C objects call. Measured 2026-11-11, docs/lanes/ffi-link.md.
    case "$t" in
      aarch64-apple-darwin) dt="MACOSX_DEPLOYMENT_TARGET=$MACOS_MIN" ;;
      *) dt="IPHONEOS_DEPLOYMENT_TARGET=$IOS_MIN" ;;
    esac
    ( cd "$CORE" && env "$dt" cargo build -j "${WHOOSH_JOBS:-2}" -q -p ffi --lib --release --target "$t" )
    echo "built core/target/$t/release/libwhoosh_ffi.a"
    case "$t" in
      aarch64-apple-darwin) prelink "$t" macos "$MACOS_MIN" ;;
      aarch64-apple-ios-sim) prelink "$t" ios-simulator "$IOS_MIN" ;;
      aarch64-apple-ios) prelink "$t" ios "$IOS_MIN" ;;
    esac
  else
    echo "skipped $t: rustup target not installed"
  fi
done
# One xcframework over the slices asked for (WHOOSH_SLICES), so SwiftPM links the right slice per platform (Package.swift).
XCF="$CORE/target/WhooshFFI.xcframework"
mkdir -p "$CORE/target"
rm -rf "$XCF"
set --
for t in $TRIPLES; do
  a="$TARGET/$t/release/prelinked/libwhoosh_ffi.a"
  [ -f "$a" ] && set -- "$@" -library "$a" -headers "$INC"
done
xcodebuild -create-xcframework "$@" -output "$XCF" > "$TMP/xcf.log" 2>&1 || { cat "$TMP/xcf.log"; exit 1; }
echo "wrote core/target/WhooshFFI.xcframework"
# A snapshot build (scripts/build.sh) points CARGO_TARGET_DIR at the live tree's core/target while $CORE is the snapshot's: the live
# Package.swift reads core/target/WhooshFFI.xcframework too, and the round's swift test cells run in the live tree (main49 to main52:
# 04-swift-unit, 05-integration and 06-contract red with "does not contain a binary artifact"). The same xcframework lands in both.
if [ "$(cd "$TARGET" && pwd -P)" != "$(cd "$CORE/target" && pwd -P)" ]; then
  rm -rf "$TARGET/WhooshFFI.xcframework"
  cp -R "$XCF" "$TARGET/WhooshFFI.xcframework" && echo "wrote $TARGET/WhooshFFI.xcframework (the live tree's copy)"
fi
echo "generated Swift written to Sources/CoreBridge/Generated/contracts.swift and engine.swift"
