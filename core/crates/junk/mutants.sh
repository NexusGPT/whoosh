#!/bin/bash
# Each mutant is ONE edit that must turn at least one named test red. The file is restored from git after each run, so
# commit the crate first. Run by the lead at the build word; this lane never runs it (uncompiled round).
# Usage: core/crates/junk/mutants.sh  (from anywhere; writes _captures/core-junk/mutants.txt)
#        ONLY=<name> core/crates/junk/mutants.sh   runs one mutant
set -u
CRATE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$CRATE/../../.." && pwd)"
LOG="$ROOT/_captures/core-junk/mutants.txt"
mkdir -p "$(dirname "$LOG")"
export PATH="$HOME/.cargo/bin:$PATH" CARGO_INCREMENTAL=0
: > "$LOG"
red=0; total=0
mutant() { # name file from to expected-test
  local name="$1" file="$CRATE/$2" from="$3" to="$4" expect="$5"
  if [ -n "${ONLY:-}" ] && [ "$ONLY" != "$name" ]; then return; fi
  total=$((total+1))
  if ! grep -qF -- "$from" "$file"; then echo "MUTANT $name: NOT APPLIED (pattern absent in $2)" | tee -a "$LOG"; return; fi
  until [ "$(LC_ALL=C sysctl -n vm.loadavg | awk '{printf "%d", $2}')" -lt 40 ]; do sleep 20; done
  FROM="$from" TO="$to" perl -0pi -e 's/\Q$ENV{FROM}\E/$ENV{TO}/' "$file"
  if (cd "$ROOT" && cargo test --manifest-path core/crates/junk/Cargo.toml --offline -j 1 > "$LOG.run" 2>&1); then
    echo "MUTANT $name: GREEN (survived) -- the suite does not catch it" | tee -a "$LOG"
  else
    red=$((red+1))
    echo "MUTANT $name: RED (expected: $expect), failing: $(grep -E '^test .* FAILED$|^error' "$LOG.run" | sed 's/^test //; s/ \.\.\. FAILED//' | tr '\n' ' ')" | tee -a "$LOG"
  fi
  git -C "$ROOT" checkout -- "core/crates/junk/$2"
}
mutant chi2-sides-swapped src/chi2.rs '((s - h) + 1.0) / 2.0' '((h - s) + 1.0) / 2.0' 'combining_stays_in_bounds_and_is_symmetric'
mutant smoothing-dropped src/model.rs '(UNKNOWN_WORD_STRENGTH * UNKNOWN_WORD_PROB + n * raw) / (UNKNOWN_WORD_STRENGTH + n)' 'raw' 'a_token_seen_once_is_never_decisive'
mutant forged-auth-header-trusted src/rules.rs 'values.first()' 'values.last()' 'only_the_topmost_authentication_results_is_believed'
mutant hard-signal-ignored src/filter.rs '.find(|r| r.matches(&msg.from_email))' '.find(|r| !r.matches(&msg.from_email))' 'a_blocked_sender_moves_whatever_the_model_thinks'
mutant move-threshold-exclusive src/filter.rs 'if score >= self.move_at {' 'if score > self.move_at {' 'the_move_cut_off_is_inclusive_and_the_review_cut_off_too'
mutant prune-drops-frequent src/model.rs 'a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0))' 'b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0))' 'the_table_is_capped_and_pruning_drops_the_rare_tokens_first'
# two more beyond the six the brief asks for
mutant counts-swapped-in-bytes src/model.rs 'put_varint(&mut out, c.spam); put_varint(&mut out, c.ham);' 'put_varint(&mut out, c.ham); put_varint(&mut out, c.spam);' 'every_token_keeps_its_own_counts_through_the_bytes'
mutant token-cap-lifted src/tokens.rs 'if self.order.len() >= MAX_TOKENS_PER_MESSAGE {' 'if false {' 'a_message_gives_at_most_the_capped_number_of_distinct_tokens'
echo "$red of $total mutants red" | tee -a "$LOG"
