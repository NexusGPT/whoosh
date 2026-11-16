#!/bin/sh
# Build the iPhone shell for the simulator (no signing identity at this commit).  sh scripts/build-ios.sh <out-dir>
set -eu
OUT="${1:?usage: build-ios.sh <out-dir>}"
WHOOSH_SLICES=ios sh scripts/gen-swift.sh
# The Xcode project is generated into the build tree, never into the worktree: it is not tracked
# (apps/ios/project.yml is the source), so a build never dirties the tree.
mkdir -p "$OUT/project"
WHOOSH_ROOT=$(pwd -P); export WHOOSH_ROOT
/opt/homebrew/bin/xcodegen generate --quiet --spec apps/ios/project.yml --project "$OUT/project"
xcodebuild -jobs "${WHOOSH_JOBS:-2}" -skipMacroValidation -project "$OUT/project/Whoosh.xcodeproj" -scheme Whoosh -configuration Debug -destination 'generic/platform=iOS Simulator' -derivedDataPath "$OUT" EXCLUDED_ARCHS=x86_64 build   # the Rust slice for the simulator is arm64 only: an x86_64 slice cannot link
