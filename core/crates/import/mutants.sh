#!/bin/bash
# Each mutant breaks one property with one edit; the suite must go red on it. The file is restored from git after each run.
# Usage: core/crates/import/mutants.sh  (from anywhere; writes _captures/core-import/mutants.txt)
# Run `cargo fetch` once with the network on first: the zip crate is not in the offline cache.
set -u
CRATE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$CRATE/../../.." && pwd)"
LOG="$ROOT/_captures/core-import/mutants.txt"
export PATH="$HOME/.cargo/bin:$PATH" CARGO_INCREMENTAL=0
mkdir -p "$(dirname "$LOG")"
: > "$LOG"
red=0; total=0
mutant() { # name file from to
  local name="$1" file="$CRATE/$2" from="$3" to="$4"
  # ONLY=<name> runs that one mutant (a targeted re-run after a test fix)
  if [ -n "${ONLY:-}" ] && [ "$ONLY" != "$name" ]; then return; fi
  total=$((total+1))
  if ! grep -qF -- "$from" "$file"; then echo "MUTANT $name: NOT APPLIED (pattern absent in $2)" | tee -a "$LOG"; return; fi
  until [ "$(LC_ALL=C sysctl -n vm.loadavg | awk '{printf "%d", $2}')" -lt 40 ]; do sleep 20; done
  FROM="$from" TO="$to" perl -0pi -e 's/\Q$ENV{FROM}\E/$ENV{TO}/' "$file"
  if (cd "$ROOT" && cargo test --manifest-path core/crates/import/Cargo.toml -j 1 > "$LOG.run" 2>&1); then
    echo "MUTANT $name: GREEN (survived) -- the suite does not catch it" | tee -a "$LOG"
  else
    red=$((red+1))
    echo "MUTANT $name: RED, failing: $(grep -E '^test .* FAILED$|^error' "$LOG.run" | sed 's/^test //; s/ \.\.\. FAILED//' | tr '\n' ' ')" | tee -a "$LOG"
  fi
  git -C "$ROOT" checkout -- "core/crates/import/$2"
}
# the six the lane brief asks for
mutant mbox-from-not-unescaped src/mbox.rs 'if gt > 0 && line[gt..].starts_with(b"From ") {' 'if false && line[gt..].starts_with(b"From ") {'
mutant emlx-byte-count-ignored src/emlx.rs 'data.truncate(start + count);' 'data.truncate(data.len());'
mutant emlx-read-bit-inverted src/emlx.rs 'read: bits & FLAG_READ != 0,' 'read: bits & FLAG_READ == 0,'
mutant account-match-case-sensitive src/matching.rs 'a.address.trim().to_ascii_lowercase() == *c' 'a.address.trim() == *c'
mutant key-ignores-message-id src/convert.rs 'Some(m) => m.to_string(),' 'Some(_) => content_key(bytes),'
mutant permission-refusal-not-typed src/error.rs 'e.kind() == io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(1) || e.raw_os_error() == Some(13)' 'false'
# more
mutant unmatched-goes-to-first-account src/convert.rs '.unwrap_or_else(|| imported_account_id.to_string());' '.unwrap_or_else(|| accounts[0].id.clone());'
mutant stored-not-counted-per-mailbox src/pipeline.rs 'self.counts_for(&current).stored += stored;' 'self.counts_for(&current).stored += 0 * stored;'
mutant batch-off-by-one src/pipeline.rs 'self.batch.len() >= IMPORT_BATCH_MESSAGES' 'self.batch.len() > IMPORT_BATCH_MESSAGES'
mutant deleted-message-imported src/convert.rs 'if mail.flags.deleted {' 'if false {'
mutant sent-mailbox-not-recognised src/matching.rs '.any(|w| w.eq_ignore_ascii_case("sent"))' '.any(|w| w.eq_ignore_ascii_case("sentx"))'
mutant partial-attachment-dropped src/apple.rs 'partial_attachments(messages_dir, message_number(name)) } else' 'Vec::new() } else'
rm -f "$LOG.run"
echo "RESULT: $red of $total mutants red" | tee -a "$LOG"
[ "$red" -eq "$total" ]
