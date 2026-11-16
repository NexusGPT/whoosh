#!/bin/sh
# Mutants of the segment classifier for tests/segments_audit.rs. Each is ONE exact edit (asserted to occur once) of a src file, then
# `cargo test -p parse --test segments_audit`, which must FAIL; the file is restored from git after each.
# usage: sh mutants-segments.sh <worktree> <log folder>     prints "<name>: RED (<failing tests>)" or "<name>: SURVIVED" per mutant.
W="$1"; L="$2"
[ -d "$W/core" ] && [ -d "$L" ] || { echo "usage: mutants-segments.sh <worktree> <log folder>"; exit 2; }
P=core/crates/parse/src
run() { # name file old new
  name="$1"; file="$2"; old="$3"; new="$4"
  python3 -I - "$W/$file" "$old" "$new" <<'PY' || { echo "$name: PATTERN NOT FOUND ONCE"; return; }
import sys
p, old, new = sys.argv[1], sys.argv[2], sys.argv[3]
s = open(p).read()
if s.count(old) != 1: sys.exit(1)
open(p, "w").write(s.replace(old, new, 1))
PY
  (cd "$W/core" && CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 perl -e 'alarm 1500; exec @ARGV' nice "$HOME/.cargo/bin/cargo" test -p parse --offline --test segments_audit -- --test-threads=2 > "$L/mutant-$name.log" 2>&1)
  code=$?
  (cd "$W" && git checkout -- "$file")
  if grep -q "^error\[" "$L/mutant-$name.log"; then echo "$name: DID NOT COMPILE (exit $code)";
  elif [ "$code" -ne 0 ] && grep -q "test result: FAILED" "$L/mutant-$name.log"; then echo "$name: RED (exit $code; $(grep '^test .* FAILED' "$L/mutant-$name.log" | sed 's/^test \(.*\) \.\.\. FAILED/\1/' | tr '\n' ' '))";
  else echo "$name: SURVIVED (exit $code; $(grep 'test result' "$L/mutant-$name.log" | tail -1))"; fi
}
run M1-band-ignores-anchor $P/segment.rs 'let anchored = reasons.iter().any(|r| r.is_anchor());' 'let anchored = true;'
run M2-never-empty-off $P/engine.rs 'if !p.canonical.trim().is_empty() && !segments.iter().any(|s| s.visibility == Visibility::Show) {' 'if false && !segments.iter().any(|s| s.visibility == Visibility::Show) {'
run M3-lossless-trims-text $P/engine.rs 'segments.push(Segment { kind: l.kind, text: t.to_string(), html,' 'segments.push(Segment { kind: l.kind, text: t.trim_end().to_string(), html,'
run M4-original-not-retained $P/engine.rs 'original_text: parts.text.clone(), original_html: parts.html.clone(), degraded: false,' 'original_text: parts.text.as_deref().map(crate::canonical), original_html: parts.html.clone(), degraded: false,'
run M5-panic-not-caught $P/engine.rs 'match catch_unwind(AssertUnwindSafe(|| run(parts, earlier, opts))) {' 'match Ok::<_, ()>(run(parts, earlier, opts)) {'
run M6-fold-nothing-above-allowed $P/segment.rs 'if !segments[..f].iter().any(|s| s.visibility == Visibility::Show) { return None; }' ''
run M7-inline-quote-as-history $P/segment.rs 'label(SegmentKind::InlineQuote, 0.9, &rs)' 'label(SegmentKind::ReplyHistory, 0.97, &rs)'
run M8-signature-one-signal $P/segment.rs 'if starts_kind + others >= 2 {' 'if starts_kind + others >= 1 {'
run M9-legal-rows-unread $P/segment.rs 'if any(&p.legal, &b.content) {' 'if false && any(&p.legal, &b.content) {'
run M10-forward-alone-collapsed $P/segment.rs 'if reasons.contains(&Reason::ForwardWithoutComment) { return Visibility::Show; }' ''
run M11-attribution-english-only $P/segment.rs 'for r in rows(include_str!("patterns/attribution.txt")) { p.attribution.push(ci(r[1])); }' 'for r in rows(include_str!("patterns/attribution.txt")) { if r[0] == "en" { p.attribution.push(ci(r[1])); } }'
run M12-authored-kinds-can-be-put-away $P/segment.rs 'if kind.authored() || reasons.is_empty() { return Visibility::Show; }' 'if reasons.is_empty() { return Visibility::Show; }'
run M13-fold-on-every-message $P/segment.rs 'if f == segments.len() { return None; }' ''
cd "$W" && git status --short $P
