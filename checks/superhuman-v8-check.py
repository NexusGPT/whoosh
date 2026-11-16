#!/usr/bin/env python3
"""Superhuman v8 key check: the Superhuman preset equals docs/reference/superhuman-shortcuts-v8.json, key for key.

Reads Sources/Features/Shortcuts/KeyPreset.swift (the Superhuman block, the fixed keys) and Commands.swift (which id
is live, a stub owed by another lane, or coming soon) and the v8 JSON. It does not need the app. For every entry of
the JSON it derives the chords (or the two-key sequence) the way ChordKit writes them, then reports:
  differ   a chord the preset binds that v8 does not list (except the declared extras), or an entry whose status
           (live or stub against coming soon) is wrong
  missing  a chord or sequence v8 lists that the preset does not bind
Also: duplicates (a chord or a command bound twice in the rows), unregistered (a command with no title).
Last line is the proof line; exit 1 unless differ, missing, duplicates and unregistered are all 0.

  python3 checks/superhuman-v8-check.py                 check and print
  python3 checks/superhuman-v8-check.py --write-fixture rewrite the "shortcuts" key of docs/design/fixtures/mac-main-sample.json
                                                         from the preset rows (chord -> title), never by hand
Importable: expected(), preset(), commands() are used by checks/shortcuts-check.py.
"""
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
V8 = os.path.join(ROOT, "docs", "reference", "superhuman-shortcuts-v8.json")
# V8_SOURCE_DIR points the check at a mutated copy of the Shortcuts folder (the mutant proofs); unset it reads the real one.
SRC = os.environ.get("V8_SOURCE_DIR") or os.path.join(ROOT, "Sources", "Features", "Shortcuts")
PRESET = os.path.join(SRC, "KeyPreset.swift")
COMMANDS = os.path.join(SRC, "Commands.swift")
FIXTURE = os.path.join(ROOT, "docs", "design", "fixtures", "mac-main-sample.json")

# Whoosh keys the Superhuman preset binds that v8 does not list (PRD "Superhuman keys: replicated exactly").
EXTRAS = {"⌘P": "palette", "⇧P": "pin", "⌘\\": "sidebar", "⌘J": "ai-reply",
          "⇧H": "toggle-message-header"}  # ⌘\ hides or shows the sidebar (owner default 2026-11-08); ⇧H expands or collapses a message header (SPEC A70, owner 22:00)
# Groups of v8 entries whose feature is not built: calendar and tabs; plus the two Folders keys named below.
SOON_GROUPS = {"calendar", "tabs"}
SOON_SEQUENCES = {("G", "O"), ("G", ";")}

MODS = "⌃⌥⇧⌘"
NAMED = {"esc": "Escape", "enter": "Enter", "tab": "Tab", "space": "Space", "↑": "Up", "↓": "Down", "←": "Left", "→": "Right"}


def chord(token):
    """One v8 key as ChordKit writes it: modifiers in ⌃ ⌥ ⇧ ⌘ order, symbols shifted, named keys by name."""
    t = token.strip()
    if t.lower().startswith("ctrl "):
        t = "⌃" + t[5:].strip()
    mods = ""
    while t and t[0] in MODS:
        mods, t = mods + t[0], t[1:]
    key = NAMED.get(t.lower(), t)
    if key == "Tab" and mods == "⇧":
        return "Shift+Tab"
    if len(key) == 1 and key.isalpha():
        key = key.upper()
    if not mods and key in "!#?":
        mods = "⇧"
    return "".join(m for m in MODS if m in mods) + key


def chords_of(text):
    """The chords of a v8 chord cell, or (None, sequence) for a two-key sequence, or ([], None) for typed text."""
    t = text.strip()
    if t.startswith(":"):
        return [], None
    if " then " in t:
        a, b = t.split(" then ")
        return None, (chord(a), chord(b))
    if all(p in "↑↓←→" for p in t.split()) and t:
        return [chord(p) for p in t.split()], None
    if t.endswith("1-9"):
        prefix = t[:-3].strip()
        prefix = "⌃" if prefix.lower() == "ctrl" else prefix
        return [chord(prefix + str(n)) for n in range(1, 10)], None
    return [chord(p) for p in t.split(" / ")], None


def expected():
    """[{entry, section, action, group, chords, sequence, soon, typed}] from the v8 JSON."""
    out = []
    for i, e in enumerate(json.load(open(V8, encoding="utf-8"))["entries"]):
        cs, seq = chords_of(e["chord"])
        soon = e["group"] in SOON_GROUPS or (seq is not None and seq in SOON_SEQUENCES)
        out.append({"entry": i, "section": e["section"], "action": e["action"], "group": e["group"], "chords": cs or [],
                    "sequence": seq, "soon": soon, "typed": cs == [] and seq is None})
    return out


def block(text, start, end):
    a = text.index(start)
    return text[a:text.index(end, a)]


def bindings(src):
    return [(ch.replace('\\\\', '\\'), cmd) for ch, cmd in re.findall(r'KeyBinding\("([^"]+)", "([^"]+)"\)', src)]  # a Swift literal "⌘\\\\" is the chord ⌘\\


def preset():
    """The Superhuman preset as the Swift source writes it: rows, second keys, context meanings, sequences, fixed keys."""
    text = open(PRESET, encoding="utf-8").read()
    sup = block(text, "public static let superhuman = KeyPreset(", "/// GotEmail:")
    rows_src = block(sup, "private static let superhumanRows", "\n    ]\n")
    rows = bindings(rows_src)
    head = sup[:sup.index("private static let superhumanRows")]
    for mod, prefix in re.findall(r'KeyBinding\.digits\("(.)", "([\w-]+)"\)', head):
        rows += [("%s%d" % (mod, n), "%s-%d" % (prefix, n)) for n in range(1, 10)]
    also = bindings(block(head, "alsoKeys: [", "]"))
    ctx = bindings(block(head, "contextKeys: [", "],"))
    seqs = re.findall(r'KeySequence\("([^"]+)", "([^"]+)", "([^"]+)"\)', head)
    fixed = bindings(block(text, "public static let fixed", "\n    ]"))
    return {"rows": rows, "also": also, "context": ctx, "sequences": seqs, "fixed": fixed}


def commands():
    """{id: (title, status, owner)} where status is live, stub or soon and owner the lane or the missing feature."""
    text = open(COMMANDS, encoding="utf-8").read()
    out = {}
    for kind, cid, title, _, owner in re.findall(r'\b(live|stub|soon)\("([^"]+)", "([^"]+)"(, (?:by|feature): "([^"]+)")?\)', text):
        out[cid] = (title, kind, owner)
    return out


def main():
    exp, p, cmds = expected(), preset(), commands()
    by_chord = {}
    for c, cid in p["rows"] + p["also"] + p["context"] + p["fixed"]:
        by_chord.setdefault(c, []).append(cid)
    seq_cmd = {(a, b): cid for a, b, cid in p["sequences"]}

    want_chords, want_seqs = set(), set()
    differ, missing = [], []
    for e in exp:
        for c in e["chords"]:
            want_chords.add(c)
            ids = by_chord.get(c)
            if not ids:
                missing.append("%s: %s" % (c, e["action"]))
                continue
            kinds = {cmds.get(i, ("", "unknown", ""))[1] for i in ids}
            if e["soon"] and "soon" not in kinds:
                differ.append("%s (%s) should be coming soon, bound to %s" % (c, e["action"], ids))
            if not e["soon"] and not (kinds - {"soon"}):
                differ.append("%s (%s) is live in v8 but only coming soon here: %s" % (c, e["action"], ids))
        if e["sequence"]:
            want_seqs.add(e["sequence"])
            cid = seq_cmd.get(e["sequence"])
            if cid is None:
                missing.append("%s then %s: %s" % (e["sequence"][0], e["sequence"][1], e["action"]))
            elif (cmds.get(cid, ("", "unknown", ""))[1] == "soon") != e["soon"]:
                differ.append("%s then %s (%s): status of %s is wrong" % (e["sequence"][0], e["sequence"][1], e["action"], cid))
    for c, ids in sorted(by_chord.items()):
        if c not in want_chords and c not in EXTRAS and c not in {"/", "⇧?"} | {f for f, _ in p["fixed"]}:
            differ.append("%s is bound (%s) and v8 does not list it" % (c, ids))
    for c in EXTRAS:
        if c not in by_chord:
            missing.append("%s (declared extra %s)" % (c, EXTRAS[c]))
    for s in sorted(set(seq_cmd) - want_seqs):
        differ.append("sequence %s then %s is bound and v8 does not list it" % s)
    # the v8 list names "/" (search) as its own entry, and "?" (Ask AI) opens the search here: both are second keys
    for c in ("/", "⇧?"):
        if c not in by_chord:
            missing.append(c)

    dupes = []
    seen_c, seen_k = set(), set()
    for c, cid in p["rows"]:
        if c in seen_c:
            dupes.append("chord " + c)
        if cid in seen_k:
            dupes.append("command " + cid)
        seen_c.add(c), seen_k.add(cid)
    used = {cid for _, cid in p["rows"] + p["also"] + p["context"]} | {cid for _, _, cid in p["sequences"]}
    unreg = sorted(i for i in used if i not in cmds)
    by_kind = {k: sorted(i for i in used if cmds.get(i, ("", "", ""))[1] == k) for k in ("live", "stub", "soon")}
    typed = [e["action"] for e in exp if e["typed"]]

    if "--write-fixture" in sys.argv:
        write_fixture(p, cmds)
        return 0
    for t, rows in (("differ", differ), ("missing", missing), ("duplicates", dupes), ("unregistered", unreg)):
        for r in rows:
            print("%s: %s" % (t.upper(), r))
    stubs = {}
    for i in by_kind["stub"]:
        stubs.setdefault(cmds[i][2], []).append(i)
    for lane in sorted(stubs):
        print("wired by %s: %s" % (lane, " ".join(stubs[lane])))
    print("not chords (typed text, native to the text view): %s" % ", ".join(typed))
    print("superhuman-v8: entries=%d chords=%d sequences=%d rows=%d live=%d stub=%d coming_soon=%d typed=%d extras=%d differ=%d missing=%d duplicates=%d unregistered=%d" % (
        len(exp), len(want_chords), len(want_seqs), len(p["rows"]), len(by_kind["live"]), len(by_kind["stub"]), len(by_kind["soon"]),
        len(typed), len(EXTRAS), len(differ), len(missing), len(dupes), len(unreg)))
    return 0 if not (differ or missing or dupes or unreg) else 1


def write_fixture(p, cmds):
    """Rewrite the fixture's "shortcuts" object: every row of the Superhuman preset, chord -> title, in row order."""
    text = open(FIXTURE, encoding="utf-8").read()
    m = re.search(r'^( +)"shortcuts": \{\n( +)', text, flags=re.M)      # the fixture's own indent, whatever it is now
    if not m:
        raise SystemExit("no \"shortcuts\" object found in the fixture")
    top, inner = m.group(1), m.group(2)
    body = ",\n".join('%s%s: %s' % (inner, json.dumps(c, ensure_ascii=False), json.dumps(cmds[cid][0], ensure_ascii=False)) for c, cid in p["rows"])
    new, n = re.subn(r'(^%s"shortcuts": \{\n).*?(\n%s\})' % (top, top), lambda mm: mm.group(1) + body + mm.group(2), text, count=1, flags=re.S | re.M)
    if n != 1:
        raise SystemExit("no \"shortcuts\" object found in the fixture")
    json.loads(new)
    open(FIXTURE, "w", encoding="utf-8").write(new)
    print("fixture shortcuts: %d rows written" % len(p["rows"]))


if __name__ == "__main__":
    sys.exit(main())
