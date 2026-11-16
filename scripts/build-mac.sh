#!/bin/sh
# THE COMPILE scripts/build.sh RUNS (BUILD_CMD in docs/pipeline.env), in the worktree, with $OUT set.
#   1. the Rust core and the generated Swift contract (scripts/gen-swift.sh, which fails on drift of nothing: it rewrites)
#   2. the Xcode project from apps/mac/project.yml (xcodegen), written to $OUT/project
#   3. xcodebuild into $OUT. -skipMacroValidation trusts the one package macro in the tree
#      (MLXHuggingFaceMacros from mlx-swift-lm, MIT, pinned by revision in Package.swift); headless
#      xcodebuild cannot show Xcode's trust prompt.
# Exit: the first failing step's code.
set -eu
: "${OUT:?OUT is set by scripts/build.sh}"
sh scripts/gen-swift.sh
python3 tools/gen-constants.py --check
# The Xcode project is generated into the build tree, never into the worktree: it is not tracked
# (apps/mac/project.yml is the source), so a build never dirties the tree.
mkdir -p "$OUT/project"
WHOOSH_ROOT=$(pwd -P); export WHOOSH_ROOT
/opt/homebrew/bin/xcodegen generate --quiet --spec apps/mac/project.yml --project "$OUT/project"
# WHOOSH_SWIFT_NO_DEBUG_INFO=1: the Swift code is compiled without debug information (Xcode's SWIFT_DEBUG_INFORMATION_FORMAT set empty: no -g).
# The optimised compile of the app's module peaked at 8.0 GB in one compiler process on 16 Nov, and the memory guard stops a process at 8 GB (main199).
# The installed app is stripped, and no step reads the dSYM. (SWIFT_USE_PARALLEL_WHOLE_MODULE_OPTIMIZATION=NO was tried first, main201: the link of the
# macro package then looks for per-file objects the one-thread compile does not write.)
NODBG=""; [ "${WHOOSH_SWIFT_NO_DEBUG_INFO:-0}" = 1 ] && NODBG="SWIFT_DEBUG_INFORMATION_FORMAT="
xcodebuild -jobs "${WHOOSH_JOBS:-2}" -skipMacroValidation -project "$OUT/project/Whoosh.xcodeproj" -scheme Whoosh -configuration "${WHOOSH_CONFIGURATION:-Debug}" ARCHS=arm64 ONLY_ACTIVE_ARCH=YES $NODBG -destination 'platform=macOS,arch=arm64' -derivedDataPath "$OUT" build
