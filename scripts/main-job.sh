#!/bin/sh
# usage: sh scripts/main-job.sh <label> "<install reason>"
# The main build job: build, INSTALL, perf gates, every check, the round, cleanup. Moved into the repo from the lead's scratchpad (main-job21.sh, tests-r58 round 3,
# plan T-21); checks/job-order-check.py reads this file. Changes from that script, and only these:
#   (a) the scratchpad and the repository are no longer typed in: JOB_SCRATCH (default: the lead's scratchpad folder below) and WHOOSH_REPO (default: the same
#       worktree as before); the signing identity is JOB_SIGN_IDENTITY (same default).
#   (b) the checks and the round after the install read a DETACHED WORKTREE at the build's own commit (NexusBuildCommit of the bundle), never the live tree,
#       which other lanes move while the round runs (main63: three cells failed on that). ROUND_TREE=live gives the old behaviour. The rust cache stays the live
#       tree's (CARGO_TARGET_DIR); `.build` is the worktree's own and goes with it.
#   (c) a build that fails removes its own build folder (and its snapshot registration), so the same label can be tried again.
LBL="$1"; REASON="$2"
REPO=${WHOOSH_REPO:-$(cd "$(dirname "$0")/.." && pwd)}
cd "$REPO" || exit 9
# owner 2026-11-10 01:0x: "we need to stop with all the authorisation asks, just once should be enough": every installed build is signed with ONE stable certificate so macOS asks for the Keychain once, never per build (ad hoc = a new app each install)
export SIGN_IDENTITY="${JOB_SIGN_IDENTITY:--}"   # "-" is an ad hoc signature; set JOB_SIGN_IDENTITY to your own stable certificate so macOS asks for the Keychain once
S=${JOB_SCRATCH:-${TMPDIR:-/tmp}/whoosh-job}; mkdir -p "$S" || exit 9
L="$S/$LBL-build.log"; SUM="$S/proof-$LBL-summary.txt"; : > "$L"; : > "$SUM"
echo "== bundle $LBL $(date +%H:%M) at $(git rev-parse --short HEAD) configuration ${WHOOSH_CONFIGURATION:-Debug} bundle ${BUNDLE_REL:-Build/Products/Debug/Whoosh.app}" >> "$L"
sh scripts/build.sh "$LBL" . >> "$L" 2>&1; echo "BUILD EXIT $?" >> "$L"
B="build/Whoosh-$LBL/${BUNDLE_REL:-Build/Products/Debug/Whoosh.app}"
# (c) a failed build removes the folder it made, so the label can be retried; a build that refused because the folder already existed made nothing: that folder is not ours
drop_failed_build() { if ! grep -q "A label names one build" "$L"; then rm -rf "build/Whoosh-$LBL"; git worktree prune >> "$L" 2>&1; echo "failed build folder build/Whoosh-$LBL removed: the label can be retried" >> "$SUM"; fi; }
grep -q "^BUILD EXIT 0" "$L" || { echo "NO BUNDLE $(date +%H:%M) (the build did not exit 0)" >> "$SUM"; tail -5 "$L"; drop_failed_build; exit 2; }
[ -d "$B" ] || { echo "NO BUNDLE $(date +%H:%M)" >> "$SUM"; tail -5 "$L"; drop_failed_build; exit 2; }
B="$REPO/$B"
echo "built $(date +%H:%M)" >> "$SUM"
# job22 (owner 2026-11-10 13:58: 'I see no new app since 12:12'): the install runs RIGHT AFTER the build, before the checks and the round;
# main63 was compiled at 12:12 and reached the owner at 13:35 because 80 minutes of checks ran first. The build tree is kept for the checks and removed at the end.
n=0
WHOOSH_RIG_HIDDEN=1 LAUNCH_ENV="WHOOSH_FIXTURE_CORE=1 WHOOSH_RIG_HIDDEN=1" INSTALL_ANYWAY=1 INSTALL_REASON="$REASON" INSTALL_DIR="$HOME/Applications" KEEP_CAP=3 BUILD_PROC_MATCH="^/Applications/Xcode.app/Contents/Developer/usr/bin/xcodebuild" sh scripts/install.sh "$LBL" > "$S/install-$LBL.log" 2>&1; irc=$?
echo "install exit=$irc (waited ${n}x30s) :: $(grep -E 'smoke|REFUSED|installed|FAILED' "$S/install-$LBL.log" | tail -3 | tr '\n' ' ')" >> "$SUM"
ST=$(defaults read $HOME/Applications/Whoosh.app/Contents/Info.plist NexusBuildLabel 2>&1)
if [ "$ST" = "$LBL" ] && grep -q "smoke: green" "$S/install-$LBL.log"; then
  mkdir -p $HOME/Library/Logs/Whoosh; open --stdout $HOME/Library/Logs/Whoosh/app.out --stderr $HOME/Library/Logs/Whoosh/app.err $HOME/Applications/Whoosh.app; sleep 3;  # job23: started through LaunchServices (open), never as a child of this shell: macOS attributes permission prompts (Calendar, microphone) to the process that launched a bare binary, so the owner's Allow button asked nothing (owner 14:30: 'Button does nothing')
  echo "installed stamp $ST; relaunched (owner rule 19:20): $(pgrep -fl 'Applications/Whoosh.app/Contents/MacOS/Whoosh' | head -1 | cut -c1-60)" >> "$SUM"
else
  echo "installed stamp is '$ST' (not $LBL) or the smoke was not green; the checks still run on the bundle" >> "$SUM"
fi
# (b) from here the checks and the round read a detached worktree at the build's commit, not the live tree (the install above already ran from the live tree)
RT=""
COMMIT=$(plutil -extract NexusBuildCommit raw -o - "$B/Contents/Info.plist" 2>&1)
if [ "${ROUND_TREE:-snapshot}" = "snapshot" ] && git cat-file -e "$COMMIT^{commit}" 2>> "$L"; then
  RT="$REPO/build/Whoosh-$LBL-round-tree"
  if git worktree add --detach "$RT" "$COMMIT" >> "$L" 2>&1 && [ "$(git -C "$RT" rev-parse HEAD)" = "$COMMIT" ]; then
    cd "$RT" || exit 9
    export BUILD_ROOT="$REPO/build" CARGO_TARGET_DIR="$REPO/core/target"
    echo "checks and round read the detached tree $RT at $COMMIT" >> "$SUM"
  else
    echo "the round tree could not be made: the checks run on the live tree" >> "$SUM"; RT=""
  fi
else
  echo "round tree: live (ROUND_TREE=${ROUND_TREE:-snapshot}, build commit '$COMMIT')" >> "$SUM"
fi
# job21: the perf gates run FIRST, at low load, before the checks and the round push the load past PERF_LOAD_MAX (main56: 31 during the round, perf cells not judged)
defaults write io.nexus.whoosh.rig AppleLanguages -array en
L1=$(sysctl -n vm.loadavg | awk '{print int($2)}'); echo "perf phase load $L1 $(date +%H:%M)" >> "$SUM"
log="$S/perf-launch-$LBL.log"; WHOOSH_RIG_HIDDEN=1 LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1 WHOOSH_FIXTURE_CORE=1" python3 perf/launch.py --bundle "$B" > "$log" 2>&1; rc=$?; echo "perf-launch exit=$rc :: $(grep -E 'checks,|RED|GREEN|median|not judged' "$log" | tail -2 | tr '\n' ' ')" >> "$SUM"
log="$S/perf-interaction-$LBL.log"; WHOOSH_RIG_HIDDEN=1 LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1 WHOOSH_FIXTURE_CORE=1" sh scripts/launch-one.sh "$B" -- --fixture-core --rig-hidden sh -c 'WHOOSH_SOCKET_DIR="$D" python3 perf/interaction.py --bundle "$0"' "$B" > "$log" 2>&1; rc=$?; echo "perf-interaction exit=$rc :: $(grep -E 'checks,|OVER|ok' "$log" | tail -4 | tr '\n' ' ')" >> "$SUM"
log="$S/perf-memory-$LBL.log"; WHOOSH_RIG_HIDDEN=1 LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1 WHOOSH_FIXTURE_CORE=1" sh scripts/launch-one.sh "$B" -- --fixture-core --rig-hidden sh -c 'WHOOSH_SOCKET_DIR="$D" python3 perf/memory-mac.py --accounts 30 --bundle "$0"' "$B" > "$log" 2>&1; rc=$?; echo "perf-memory exit=$rc :: $(grep -E 'checks,|MB|not judged' "$log" | tail -2 | tr '\n' ' ')" >> "$SUM"
log="$S/perf-footprint-$LBL.log"; WHOOSH_RIG_HIDDEN=1 LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1 WHOOSH_FIXTURE_CORE=1" python3 perf/footprint.py "$B" > "$log" 2>&1; rc=$?; echo "perf-footprint exit=$rc :: $(grep -E 'checks,|RED|OVER' "$log" | tail -2 | tr '\n' ' ')" >> "$SUM"
defaults write io.nexus.whoosh.rig AppleLanguages -array en
for c in checks/parity/par-sidebar.py checks/parity/par-inboxlist.py checks/parity/par-reader.py checks/parity/par-macshell.py checks/parity/par-searchmac.py checks/parity/par-labelscontactsmac.py checks/parity/par-addaccount.py checks/parity/par-quickreply.py checks/parity/par-attachments.py checks/parity/par-entraguide.py checks/parity/par-composermac.py checks/parity/par-keys.py checks/keycaps-check.py checks/parity/par-notch.py checks/dictation-check.py checks/ondeviceai-check.py checks/suggestion-check.py checks/messagebody-check.py checks/rig-hidden-check.py checks/settingsgeneralmac-check.py checks/shortcuts-check.py checks/parity/par-settingswindow.py checks/settingsaimac-check.py checks/settingsreplies-check.py checks/brandmark-check.py checks/parity/par-welcomemac.py checks/mcpnotesmac-check.py checks/richtext-check.py $( [ -f checks/parity/par-firststart.py ] && echo checks/parity/par-firststart.py ) checks/seams/notch-inbox.py checks/realcore-check.py; do
  n=$(basename "$c" .py); log="$S/$n-$LBL.log"
  if [ "$n" = "rig-hidden-check" ] || [ "$n" = "realcore-check" ]; then
    # this check launches its own instance: it takes the bundle, not a socket (main42: called via launch-one it printed usage, exit 2)
    WHOOSH_RIG_HIDDEN=1 LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1 WHOOSH_FIXTURE_CORE=1" python3 "$c" "$B" > "$log" 2>&1; rc=$?
  else
  WHOOSH_RIG_HIDDEN=1 LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1 WHOOSH_FIXTURE_CORE=1" sh scripts/launch-one.sh "$B" -- --fixture-core --rig-hidden sh -c 'WHOOSH_SOCKET_DIR="$D" python3 "$0"' "$c" > "$log" 2>&1; rc=$?
  fi
  echo "$n exit=$rc :: $(grep -E '^[0-9]+ checks,' "$log" | tail -1)" >> "$SUM"
done
# job21: J6 (the notch journey) driven as a whole, hidden; par-notch alone never proved the board row fn-tests-j6
log="$S/J6-$LBL.log"; WHOOSH_RIG_HIDDEN=1 LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1 WHOOSH_FIXTURE_CORE=1" sh scripts/launch-one.sh "$B" -- --fixture-core --rig-hidden sh -c 'WHOOSH_SOCKET_DIR="$D" python3 journeys/run.py J6' > "$log" 2>&1; rc=$?; echo "J6 exit=$rc :: $(grep -E 'checks,' "$log" | tail -1)" >> "$SUM"
# main47: the real core's browser sign-in, slice 2b (own instance, local fakes; never the owner's copy)
L1=$(sysctl -n vm.loadavg | awk '{print int($2)}'); if [ "$L1" -lt 12 ]; then log="$S/perf-notch-$LBL.log"; WHOOSH_RIG_HIDDEN=1 LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1 WHOOSH_FIXTURE_CORE=1" sh scripts/launch-one.sh "$B" -- --fixture-core --rig-hidden sh -c 'WHOOSH_SOCKET_DIR="$D" python3 perf/notch-arrival.py --bundle "$0"' "$B" > "$log" 2>&1; rc=$?; echo "perf-notch exit=$rc (load $L1) :: $(tail -1 "$log")" >> "$SUM"; else echo "perf-notch not run: load $L1 over PERF_LOAD_MAX 12" >> "$SUM"; fi
log="$S/realcore-2b-$LBL.log"; WHOOSH_RIG_HIDDEN=1 LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1" python3 checks/realcore-check.py "$B" --slice2b > "$log" 2>&1; rc=$?; echo "realcore-2b exit=$rc :: $(grep -E 'checks,|RED|GREEN|green|red' "$log" | tail -1)" >> "$SUM"
log="$S/realcore-3-$LBL.log"; WHOOSH_RIG_HIDDEN=1 LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1" python3 checks/realcore-check.py "$B" --slice3 > "$log" 2>&1; rc=$?; echo "realcore-3 exit=$rc :: $(grep -E 'checks,|RED|GREEN|green|red' "$log" | tail -1)" >> "$SUM"
log="$S/realcore-4-$LBL.log"; WHOOSH_RIG_HIDDEN=1 LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1" python3 checks/realcore-check.py "$B" --slice4 > "$log" 2>&1; rc=$?; echo "realcore-4 exit=$rc :: $(grep -E 'checks,|RED|GREEN|green|red' "$log" | tail -1)" >> "$SUM"
log="$S/realcore-3b-$LBL.log"; WHOOSH_RIG_HIDDEN=1 LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1" python3 checks/realcore-check.py "$B" --slice3b > "$log" 2>&1; rc=$?; echo "realcore-3b exit=$rc :: $(grep -E 'checks,|RED|GREEN|green|red' "$log" | tail -1)" >> "$SUM"
log="$S/realcore-3b-mutant-$LBL.log"; WHOOSH_RIG_HIDDEN=1 LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1" python3 checks/realcore-check.py "$B" --slice3b --mutant > "$log" 2>&1; rc=$?; echo "realcore-3b-mutant exit=$rc (red expected) :: $(grep -E 'checks,|RED|GREEN|green|red' "$log" | tail -1)" >> "$SUM"
log="$S/realcore-5-$LBL.log"; WHOOSH_RIG_HIDDEN=1 LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1" python3 checks/realcore-check.py "$B" --slice5 > "$log" 2>&1; rc=$?; echo "realcore-5 exit=$rc :: $(grep -E 'checks,|RED|GREEN|green|red' "$log" | tail -1)" >> "$SUM"
log="$S/realcore-bye-$LBL.log"; WHOOSH_RIG_HIDDEN=1 LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1" python3 checks/realcore-check.py "$B" --slice-bye > "$log" 2>&1; rc=$?; echo "realcore-bye exit=$rc :: $(grep -E 'checks,|RED|GREEN|green|red' "$log" | tail -1)" >> "$SUM"
log="$S/realcore-bye-refuse-$LBL.log"; WHOOSH_RIG_HIDDEN=1 LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1" python3 checks/realcore-check.py "$B" --slice-bye --refuse > "$log" 2>&1; rc=$?; echo "realcore-bye-refuse exit=$rc :: $(grep -E 'checks,|RED|GREEN|green|red' "$log" | tail -1)" >> "$SUM"
log="$S/realcore-3-mutant-$LBL.log"; WHOOSH_RIG_HIDDEN=1 LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1" python3 checks/realcore-check.py "$B" --slice3 --mutant > "$log" 2>&1; rc=$?; echo "realcore-3-mutant exit=$rc (red expected) :: $(grep -E 'checks,|RED|GREEN|green|red' "$log" | tail -1)" >> "$SUM"
log="$S/realcore-2b-mutant-$LBL.log"; WHOOSH_RIG_HIDDEN=1 LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1" python3 checks/realcore-check.py "$B" --slice2b --mutant > "$log" 2>&1; rc=$?; echo "realcore-2b-mutant exit=$rc (red expected) :: $(grep -E 'checks,|RED|GREEN|green|red' "$log" | tail -1)" >> "$SUM"
defaults delete io.nexus.whoosh.rig AppleLanguages 2>&1 | grep -v "does not exist" >> "$L"
for m in leaks picked; do log="$S/i18n-$m-$LBL.log"; WHOOSH_RIG_HIDDEN=1 LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1 WHOOSH_FIXTURE_CORE=1" python3 checks/i18n-check.py --$m "$B" > "$log" 2>&1; rc=$?; echo "i18n-$m exit=$rc :: $(grep -E 'checks,|RED|GREEN|green|red' "$log" | tail -1)" >> "$SUM"; done
defaults write io.nexus.whoosh.rig AppleLanguages -array en
sh scripts/gen-swift.sh >> "$L" 2>&1; echo "gen-swift (live tree, before the round) exit=$?" >> "$SUM"
BUILD_PROC_MATCH="^/Applications/Xcode.app/Contents/Developer/usr/bin/xcodebuild" ROUND_BUILD_WAIT=120 sh scripts/round.sh "$LBL" > "$S/round-$LBL.log" 2>&1; rrc=$?
defaults delete io.nexus.whoosh.rig AppleLanguages 2>&1 | grep -v "does not exist" >> "$L"
cd "$REPO" || exit 9
if [ -n "$RT" ]; then git worktree remove --force "$RT" >> "$L" 2>&1; git worktree prune >> "$L" 2>&1; echo "round tree removed" >> "$SUM"; fi
rm -rf core/target/debug .build; echo "round test caches removed (core/target/debug, .build)" >> "$SUM"
echo "round exit=$rrc :: $(grep -E 'verdict|REFUSED|red' "$S/round-$LBL.log" | tail -2 | tr '\n' ' ')" >> "$SUM"
# tests-r58 round 4: three engine tests the lead reads on every build (reported red on a tree with contract delta 24b but without settingsaimac-r3's fixture; state_parity reads docs/STATE-PATHS.json):
# their line is named in the summary, or its absence is (a passing test prints nothing in the round's FAIL-only output)
for t in engine_ai_settings::the_ai_state_paths_and_their_json_are_the_fixture_cores the_ai_state_paths_and_their_json_are_the_fixture_cores every_state_path_the_shell_reads_answers_its_json_kind_on_a_new_installation; do
  TL=$(grep -h "$t" "$REPO/build/Whoosh-$LBL/round/"*.txt "$S/round-$LBL.log" 2>&1 | head -2 | tr '\n' ' ' | cut -c1-240)
  echo "named engine test $t: ${TL:-no FAIL line in the round output (passed, or the core cells did not run it)}" >> "$SUM"
done
ST=$(defaults read $HOME/Applications/Whoosh.app/Contents/Info.plist NexusBuildLabel 2>&1)
if [ "$ST" = "$LBL" ]; then rm -rf "build/Whoosh-$LBL"; echo "build tree removed" >> "$SUM"; rm -rf core/target; echo "rust build cache removed (disk rule: one build at a time, nothing kept)" >> "$SUM"; else echo "not installed (stamp '$ST'): build tree kept (read build/Whoosh-$LBL/smoke.log first)" >> "$SUM"; fi
echo "JOB DONE $(date +%H:%M) $(df -g "$HOME" | tail -1 | awk '{print $4" GB free"}')" >> "$SUM"; cat "$SUM"
