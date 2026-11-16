#!/usr/bin/env python3
"""Contract types check (fn-contracttests-s1, fn-contract-types).

    python3 checks/contract-types-check.py        prints `N checks, F failed, U not judged`; exit 1 when F > 0 or U > 0

Two promises, each read from the documents and the generated code, never from a hand-kept list:

  1. TYPES. Every type named in a `reads` cell of docs/CAPABILITY-CONTRACT.md exists in
     Sources/CoreBridge/Generated/contracts.swift. The name is the DATA-MAP record in kebab-case
     (`account-credential` -> `AccountCredential`); `thread` and `label` are renamed `MailThread` and
     `MailLabel` on the Swift side (docs/DIGEST.md "Shapes"). A name that is not a record is read as a
     write-up of the contract crate's `records!` table (core/crates/contracts/src/records.rs): the check
     reports it by name instead of passing it.
  2. COUNTS. The two count definitions of docs/DIGEST.md equal, word for word, the doc comments of
     `TabCounts.unread` and `TabCounts.waiting` in contracts.swift and the constants in
     core/crates/contracts/src/counts.rs.

Positive controls: the number of rows and of distinct types found must be above the floor the contract
doc states (57+ rows) or the parse itself is broken and the check says `not judged`, never green.

Overrides for the mutants (point the check at a broken copy; the repo files stay as they are):
  WHOOSH_CTC_CONTRACT, WHOOSH_CTC_SWIFT, WHOOSH_CTC_DIGEST, WHOOSH_CTC_COUNTS, WHOOSH_CTC_RECORDS

# MUTANTS
# - a type removed from contracts.swift (copy without `public struct AccountCredential`)  -> check "type account-credential" FAILS
# - a type renamed in the contract doc (a reads cell naming `acount`)                      -> check "type acount" FAILS
# - a word changed in the DIGEST unread line                                               -> check "DIGEST unread == TabCounts.unread" FAILS
# - a word changed in the contracts.swift waiting doc comment                              -> check "DIGEST waiting == TabCounts.waiting" FAILS
# - a word changed in counts.rs UNREAD_DEFINITION                                          -> check "DIGEST unread == counts.rs" FAILS
# - the reads column emptied (doc copy with no reads cells)                                -> `not judged` (parse floor), exit 1
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RENAMED = {"thread": "MailThread", "label": "MailLabel"}
ROW_FLOOR = 57          # docs/CAPABILITY-CONTRACT.md footer: "57 journey rows + ..." (the rows the doc itself counts)
TYPE_FLOOR = 20         # a parse that finds fewer distinct types than this found nothing


def path(env, rel):
    return os.environ.get(env) or os.path.join(ROOT, rel)


def pascal(kebab):
    return RENAMED.get(kebab) or "".join(w.capitalize() for w in kebab.split("-"))


class Report:
    def __init__(self):
        self.n = self.f = self.u = 0

    def ok(self, name, good, why=""):
        self.n += 1
        if not good:
            self.f += 1
            print("FAIL  %s%s" % (name, (": " + why) if why else ""))

    def unjudged(self, name, why):
        self.n += 1
        self.u += 1
        print("NOT JUDGED  %s: %s" % (name, why))


def reads_cells(text):
    """(row id, reads cell) for every `| C-... |` row of the contract table."""
    out = []
    for line in text.splitlines():
        if not line.startswith("| C-"):
            continue
        cols = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cols) < 8:
            out.append((cols[0], None, None))
            continue
        out.append((cols[0], cols[3], cols[5]))
    return out


def names_in(cell):
    """Type names of one reads cell: the part before the first ` · `, both sides of an arrow."""
    head = cell.split(" · ")[0]
    names = []
    for part in re.split(r"→|,", head):
        part = re.sub(r"\(.*?\)", "", part).strip()
        if part and part.lower() != "none":
            names.append(part)
    return names


def swift_types(text):
    return set(re.findall(r"^public (?:struct|enum|protocol|final class|class) (\w+)", text, re.M))


def swift_doc_for(text, struct, field):
    """The doc comment above `public var <field>` inside `public struct <struct>`."""
    m = re.search(r"public struct %s \{(.*?)\n\}" % struct, text, re.S)
    if not m:
        return None
    d = re.search(r"/\*\*\s*\n\s*\*\s*(.*?)\s*\n\s*\*/\s*\n\s*public var %s:" % field, m.group(1))
    return d.group(1) if d else None


def main():
    r = Report()
    contract = open(path("WHOOSH_CTC_CONTRACT", "docs/CAPABILITY-CONTRACT.md"), encoding="utf-8").read()
    swift = open(path("WHOOSH_CTC_SWIFT", "Sources/CoreBridge/Generated/contracts.swift"), encoding="utf-8").read()
    digest = open(path("WHOOSH_CTC_DIGEST", "docs/DIGEST.md"), encoding="utf-8").read()
    counts = open(path("WHOOSH_CTC_COUNTS", "core/crates/contracts/src/counts.rs"), encoding="utf-8").read()

    # 1. TYPES
    rows = reads_cells(contract)
    seen, held, ids = {}, {}, {rid for rid, _, _ in rows}
    for rid, cell, state in rows:
        if cell is None:
            r.ok("row %s has a reads cell" % rid, False, "the row has fewer than 8 columns")
            continue
        if cell.startswith("as "):
            # a parity row: its reads are those of the rows it names, which must exist
            for ref in re.findall(r"C-J\d+\.\d+", cell):
                r.ok("row %s refers to an existing row %s" % (rid, ref), ref in ids, "no such row in the table")
            continue
        for n in names_in(cell):
            (held if state == "not built" else seen).setdefault(n, rid)
    if len(rows) < ROW_FLOOR or len(seen) < TYPE_FLOOR:
        r.unjudged("parse floor", "found %d rows and %d distinct types in the reads cells; the contract has %d+ rows, so the parse is broken"
                   % (len(rows), len(seen), ROW_FLOOR))
    have = swift_types(swift)
    if len(have) < TYPE_FLOOR:
        r.unjudged("swift floor", "found only %d public types in contracts.swift" % len(have))
    for n in sorted(seen):
        if not re.fullmatch(r"[a-z][a-z0-9-]*", n):
            r.ok("type %s" % n, False, "named in %s but not a record name (lowercase words joined by '-')" % seen[n])
            continue
        r.ok("type %s (%s) exists in contracts.swift" % (n, pascal(n)), pascal(n) in have,
             "%s names it in a reads cell; contracts.swift has no `public struct/enum %s`" % (seen[n], pascal(n)))

    # A HELD row (state "not built") names a record outside the contract until its journey is signed. It stays
    # consistent: the type must still be absent; the day it appears the row has to be un-held (a stale hold fails).
    for n, rid in sorted(held.items()):
        base = re.sub(r"\s*\(.*", "", n).strip()
        r.ok("held row %s: type %s is still outside contracts.swift" % (rid, base), pascal(base) not in have,
             "contracts.swift now has %s: un-hold the row (state is no longer 'not built')" % pascal(base))

    # 2. COUNTS
    bullets = dict(re.findall(r"^- (unread|waiting) = (.*)$", digest, re.M))
    for word, const in (("unread", "UNREAD_DEFINITION"), ("waiting", "WAITING_DEFINITION")):
        want = "%s = %s" % (word, bullets[word]) if word in bullets else None
        if want is None:
            r.ok("DIGEST defines %s" % word, False, "no `- %s = ...` line in docs/DIGEST.md" % word)
            continue
        doc = swift_doc_for(swift, "TabCounts", word)
        r.ok("DIGEST %s == TabCounts.%s" % (word, word), doc == want, "DIGEST says %r, contracts.swift doc says %r" % (want, doc))
        c = re.search(r'pub const %s: &str = "(.*?)";' % const, counts)
        r.ok("DIGEST %s == counts.rs" % word, bool(c) and c.group(1) == want,
             "DIGEST says %r, counts.rs says %r" % (want, c.group(1) if c else None))
        c2 = re.search(r"/// (%s = .*)\n\s*pub %s: u32" % (word, word), counts)
        r.ok("DIGEST %s == counts.rs doc comment" % word, bool(c2) and c2.group(1) == want,
             "DIGEST says %r, counts.rs field doc says %r" % (want, c2.group(1) if c2 else None))

    print("%d checks, %d failed, %d not judged" % (r.n, r.f, r.u))
    return 1 if (r.f or r.u) else 0


if __name__ == "__main__":
    sys.exit(main())
