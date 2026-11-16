#!/bin/sh
# round: no-rig
# Manifest cell core-mutants: cargo mutants over the Rust workspace; a missed mutant is a failure.
# Six crates read docs/CONSTANTS.md at COMPILE time (include_str!("../../../../docs/CONSTANTS.md") in actions, ai-jobs, contracts,
# decision, outbox, smart-inbox) and ten build.rs read it too, so cargo-mutants' own copy of core/ alone cannot build (main52 to main56:
# "couldn't read crates/contracts/src/../../../../docs/CONSTANTS.md", exit 4, no summary). The cell therefore builds a SCRATCH tree that
# holds core/ (sources only) beside docs/CONSTANTS.md and runs cargo mutants --in-place THERE (never in the live tree: in-place mutates
# sources and a killed run leaves them mutated); the live core/target is reused so the baseline is an incremental build.
# One round cannot test every mutant inside its budget, so each build tests one SHARD of eight, picked from the build label's digits
# --jobs is NOT passed: cargo-mutants 27.1 refuses --in-place with --jobs (main58: exit 1, "the argument '--in-place' cannot be used with '--jobs'"), so the shard runs one mutant at a time.
# (main56 -> shard 0/8, main57 -> 1/8, ...): eight consecutive rounds cover the workspace; a MISSED mutant in the shard is red.
. "$(dirname "$0")/_sum.sh"
PATH="$HOME/.cargo/bin:$PATH"; export PATH
command -v cargo-mutants > /dev/null 2>&1 || unjudged "cargo-mutants is not installed on this Mac (cargo install cargo-mutants)"
command -v rsync > /dev/null 2>&1 || unjudged "rsync is not on the PATH of this run"
ROOT=$(pwd)
# the shard follows the build label's digits (WHOOSH_BUNDLE's stamp), else the commit count, so eight consecutive rounds cover the workspace
LBL=""; [ -n "${WHOOSH_BUNDLE:-}" ] && LBL=$(plutil -extract NexusBuildLabel raw -o - "$WHOOSH_BUNDLE/Contents/Info.plist" 2>&1 | tr -dc '0-9')
[ -n "$LBL" ] || LBL=$(git -C "$ROOT" rev-list --count HEAD 2>&1 | tr -dc '0-9'); LBL=${LBL:-0}
SHARDS=$(sed -n 's/^| MUTANTS_SHARDS | \([0-9]*\) |.*/\1/p' "$ROOT/docs/CONSTANTS.md" | head -1); SHARDS=${SHARDS:-8}; SHARD=$((LBL % SHARDS))
WHOOSH_CONSTANTS_MD="$ROOT/docs/CONSTANTS.md"; export WHOOSH_CONSTANTS_MD
TMP=$(mktemp -d "${TMPDIR:-/tmp}/whoosh-mutants.XXXXXX") || unjudged "no scratch folder for the mutants tree"
# What the cargo tests read OUTSIDE core/ (round main76, step 03: crates/parse/tests/fixtures.rs read $TMP/fixtures/parse, absent, so the baseline panicked and no mutant ran).
# Found with: grep -rnE '(\.\./){2,}|root\(\)\.join' core/crates --include='*.rs' (the CARGO_MANIFEST_DIR/../../.. joins), nothing more is copied:
#   docs/CONSTANTS.md (include_str and build.rs, and the ledger tests), docs/DATA-MAP.md and docs/DIGEST.md (contracts/tests/records.rs, store/migrations/tests/migration_v1.rs),
#   docs/STATE-PATHS.json (engine/tests/state_parity.rs), docs/FIRST-RUN.json (engine/tests/first_run_support), docs/design/fixtures/mac-main-sample.json (agents/tests/support),
#   fixtures/parse (parse/tests/fixtures.rs), fixtures/autoconfig (autoconfig tests and src/adapters/dns_hickory.rs), fixtures/smart-inbox.json (smart-inbox/tests/support),
#   fixtures/search-queries.json (search/tests/search_queries.rs), Sources/CoreBridge/FixtureCore.swift (engine/tests/engine_ai_settings.rs), checks/realcore_aifake.py (engine/tests/engine_rig_core.rs).
READS="docs/CONSTANTS.md docs/DATA-MAP.md docs/DIGEST.md docs/STATE-PATHS.json docs/FIRST-RUN.json docs/design/fixtures/mac-main-sample.json fixtures/parse fixtures/autoconfig fixtures/smart-inbox.json fixtures/search-queries.json fixtures/tests-j2.json Sources/CoreBridge/FixtureCore.swift checks/realcore_aifake.py"
for p in $READS; do [ -e "$ROOT/$p" ] || { rm -rf "$TMP"; unjudged "the mutants tree needs $p, which is not in the repo"; }; done
(cd "$ROOT" && rsync -aR $READS "$TMP/") && rsync -a --exclude target --exclude mutants.out "$ROOT/core/" "$TMP/core/" || { rm -rf "$TMP"; unjudged "the scratch copy of core/ and of the files its tests read could not be made"; }
out=$(cd "$TMP/core" && CARGO_TARGET_DIR="$ROOT/core/target" cargo mutants --workspace --in-place --shard "$SHARD/$SHARDS" --timeout 120 2>&1); ec=$?
rm -rf "$TMP"
printf 'mutants shard %s/%s (label %s)\n' "$SHARD" "$SHARDS" "$LBL"
printf '%s\n' "$out" | grep -E 'MISSED' | head -8 | sed 's/^/FAIL /'
t=$(printf '%s\n' "$out" | grep -Eo '[0-9]+ mutants tested' | head -1 | grep -Eo '^[0-9]+')
m=$(printf '%s\n' "$out" | grep -Eo '[0-9]+ missed' | head -1 | grep -Eo '^[0-9]+')
[ -n "$t" ] || { printf 'FAIL cargo mutants gave no summary (exit %s): %s\n' "$ec" "$(printf '%s\n' "$out" | grep -E '^error|cannot read|panicked|No such file' | head -3 | tr '\n' ' ')"; emit 0 1 0; exit 1; }
emit "$t" "${m:-0}" 0; [ "${m:-0}" -eq 0 ]
