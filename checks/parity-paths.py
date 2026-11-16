#!/usr/bin/env python3
"""parity-paths (T-02, T-04 static half; plan realcore-firstrun section 5, P1 and P3).

The fault class (A119 "No mail account yet", A121 empty Needs Reply): a screen reads a state path that one core serves and the other does not.
This check lists, from the sources alone (no build, no app):
  READS   every literal state path the SHELL reads: `core.state(path: "...")`, `CoreText.value("...", in:)`, `ctl.core("...")`, in Sources/**, the fixture
          files (Sources/CoreBridge/Fixture*, Generated) and `#expect` test lines left out. `\\(x)` inside a literal is a wildcard `*`.
  SERVED  every path the fixture core's `state` answers, read off its shapes (FixtureCore.swift and the Fixture*.swift it delegates to):
          `path == "a/b"`, `hasPrefix("a/")` + `hasSuffix("/b")`, `case ("a", N) where p[2] == "x"`, `case ("a", N):` with a nested `switch p[K] { case "x": ...`.
          A heuristic over those shapes, not a Swift parser: a shape it does not know is a path it cannot see, and the selftest pins the shapes it does.
docs/STATE-PATHS.json holds one row per path pattern (`pattern`, or `path` when the file already uses that key; `kind`, `readers`, `empty_by_design`, any other
field a lane added is kept) and `async_commands` (command -> the state paths its answer lands on). Exit 1 when
  - the shell reads a path whose pattern has no row in the file: it names the path and the reader file;
  - a path listed under `async_commands` is not served by the fixture core: it names the command and the path.
    python3 checks/parity-paths.py            check (what scripts/gates.sh runs): `N paths, 0 unknown`
    python3 checks/parity-paths.py --write    add the rows for new reads, refresh `readers` and `served_by_fixture`, keep every other field; never deletes a row
    python3 checks/parity-paths.py --selftest the extractor on sample text, then a mutant per fault class: each must go red
"""
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FILE = os.path.join(ROOT, "docs", "STATE-PATHS.json")
READ_RE = re.compile(r'(?:\bstate\(path:|\bCoreText\.value\(|\bctl\.core\()\s*"((?:[^"\\]|\\.|\\\([^)]*\))*)"')
INTERP = re.compile(r'\\\([^)]*\)')


def display(p):
    """The notation of the file: an interpolation `\\(threadId)` becomes `<threadid>`, a wildcard segment stays `*`."""
    return "/".join(re.sub(r"\\\(([A-Za-z_]\w*)[^)]*\)", lambda m: "<%s>" % m.group(1).lower(), seg) if "\\(" in seg else seg for seg in p.split("/"))


def covers(a, b):
    """Two normalised patterns name the same paths when they have the same segments and each pair is equal or one side is a wildcard: a concrete id
    (`ai_provider/jev/state`) is covered by the typed `ai_provider/<provider>/state`, and the other way round."""
    sa, sb = a.split("/"), b.split("/")
    return len(sa) == len(sb) and all(x == y or x == "*" or y == "*" for x, y in zip(sa, sb))


def norm(p):
    """A path pattern: interpolations, <id>, {id} and * are one wildcard `*`."""
    p = INTERP.sub("*", p)
    return "/".join("*" if re.fullmatch(r"<[^>]*>|\{[^}]*\}|\*", seg) else seg for seg in p.split("/"))


def is_fixture(rel):
    return rel.startswith("Sources/CoreBridge/Fixture") or rel.startswith("Sources/CoreBridge/Generated")


def swift_files(root):
    for d, _, fs in os.walk(os.path.join(root, "Sources")):
        for f in fs:
            if f.endswith(".swift"):
                full = os.path.join(d, f)
                yield os.path.relpath(full, root), full


def reads(files):
    """({pattern: sorted reader files}, {pattern: display form with <name> placeholders}); files = iterable of (relative path, text)."""
    out = {}
    for rel, text in files:
        if is_fixture(rel):
            continue
        for line in text.splitlines():
            if "#expect" in line:
                continue
            for m in READ_RE.finditer(line):
                lit = m.group(1)
                if "/" not in lit and "\\(" not in lit and not re.fullmatch(r"[a-z_]+", lit):
                    continue
                out.setdefault(norm(lit), [display(lit), set()])[1].add(rel)
    return {k: sorted(v[1]) for k, v in sorted(out.items())}, {k: v[0] for k, v in out.items()}


TUPLE = re.compile(r'case\s+((?:\("[a-z_0-9]+",\s*\d+\)\s*,?\s*)+)(?:where\s+p\[(\d+)\]\s*==\s*"([a-z_0-9]+)")?\s*:?')
ONE = re.compile(r'\("([a-z_0-9]+)",\s*(\d+)\)')


def served(files):
    """The set of path patterns the fixture's `state` answers, from the shapes named in the module doc."""
    out = set()
    for rel, text in files:
        if not rel.startswith("Sources/CoreBridge/Fixture") or "Generated" in rel:
            continue
        lines = text.splitlines()
        for i, line in enumerate(lines):
            for m in re.finditer(r'path\s*==\s*"([a-z_0-9/]+)"', line):
                out.add(norm(m.group(1)))
            m = re.search(r'path\.hasPrefix\("([a-z_0-9/]+)"\)', line)
            if m:
                pre = m.group(1)
                suf = re.search(r'path\.hasSuffix\("([a-z_0-9/]+)"\)', line + (lines[i + 1] if i + 1 < len(lines) else ""))
                out.add(norm(pre + "*" + suf.group(1)) if suf else norm(pre.rstrip("/") + "/*"))
            t = TUPLE.search(line)
            if t:
                where = (int(t.group(2)), t.group(3)) if t.group(2) else None
                for first, n in ONE.findall(t.group(1)):
                    n = int(n)
                    if where:
                        segs = [first] + ["*"] * (n - 1)
                        segs[where[0]] = where[1]
                        out.add("/".join(segs))
                        continue
                    nested = None
                    if i + 1 < len(lines):
                        sw = re.match(r'\s*switch\s+p\[(\d+)\]\s*\{', lines[i + 1])
                        if sw:
                            nested = int(sw.group(1))
                    if nested is None:
                        out.add("/".join([first] + ["*"] * (n - 1)))
                        continue
                    indent = len(lines[i + 1]) - len(lines[i + 1].lstrip())
                    for nxt in lines[i + 2:]:
                        ind = len(nxt) - len(nxt.lstrip())
                        if nxt.strip() and (ind < indent or (ind == indent and nxt.strip().startswith("}"))):
                            break
                        c = re.match(r'\s*case\s+"([a-z_0-9]+)"', nxt)
                        if c:
                            segs = [first] + ["*"] * (n - 1)
                            segs[nested] = c.group(1)
                            out.add("/".join(segs))
    return out


def load(path=FILE):
    if not os.path.exists(path):
        return {"version": 1, "paths": [], "async_commands": {}}
    return json.load(open(path, encoding="utf-8"))


def key_of(doc):
    return "path" if any("path" in r and "pattern" not in r for r in doc.get("paths", [])) else "pattern"


def commands(doc):
    """[(command, [paths])] whether async_commands is a dict or a list of {command, paths|state_paths}."""
    ac = doc.get("async_commands", {})
    if isinstance(ac, dict):
        return [(c, v if isinstance(v, list) else v.get("paths", v.get("state_paths", []))) for c, v in ac.items()]
    return [(r.get("command", "?"), r.get("paths", r.get("state_paths", []))) for r in ac]


def untyped_rows(doc):
    """Rows whose kind is not a string (null): a path the shell reads and nobody typed (docs/STATE-PATHS.json `about`)."""
    k = key_of(doc)
    return [r.get(k, "?") for r in doc.get("paths", []) if not isinstance(r.get("kind"), str)]


def check(read_map, serve, doc):
    """-> (problem lines, untyped paths): unknown reads and unserved async paths are problems; untyped rows are listed apart."""
    k = key_of(doc)
    known = [norm(r.get(k, "")) for r in doc.get("paths", [])]
    bad = ["the shell reads %s (%s): no row in docs/STATE-PATHS.json" % (p, ", ".join(fs[:3])) for p, fs in read_map.items() if not any(covers(r, p) for r in known)]
    for cmd, ps in commands(doc):
        for p in ps:
            if norm(p) not in serve:
                bad.append("async command %s lands on %s: the fixture core does not serve it" % (cmd, p))
    return bad, untyped_rows(doc)


def write(read_map, names, serve, doc):
    """Adds a row (kind null, the untyped finding) for each read no row covers; refreshes `readers` and `served_by_fixture`; keeps every other field."""
    k = key_of(doc)
    rows = doc.setdefault("paths", [])
    files, serves = {}, {}
    for p, fs in read_map.items():
        hit = [r for r in rows if covers(norm(r.get(k, "")), p)]
        if not hit:
            r = {k: names.get(p, p), "kind": None, "readers": []}
            rows.append(r)
            hit = [r]
        for r in hit:       # one row may be read under several spellings (chains/entry, chains/exit): its readers are all of them
            files.setdefault(id(r), set()).update(fs)
            serves[id(r)] = serves.get(id(r), False) or p in serve or any(covers(norm(r.get(k, "")), x) for x in serve)
    for r in rows:
        r["readers"] = sorted(files.get(id(r), ()))
        if id(r) in serves:
            r["served_by_fixture"] = serves[id(r)]
    rows.sort(key=lambda r: r.get(k, ""))
    doc.setdefault("version", 1)
    doc.setdefault("async_commands", {})
    return doc


def load_files(root=ROOT):
    return [(rel, open(full, encoding="utf-8", errors="replace").read()) for rel, full in swift_files(root)]


def main(argv):
    if "--selftest" in argv:
        return selftest()
    files = load_files()
    rd, names = reads(files)
    sv = served(files)
    doc = load()
    if "--write" in argv:
        doc = write(rd, names, sv, doc)
        json.dump(doc, open(FILE, "w", encoding="utf-8"), indent=1, ensure_ascii=False)
        open(FILE, "a").write("\n")
        print("wrote %s: %d rows" % (os.path.relpath(FILE, ROOT), len(doc["paths"])))
    bad, untyped = check(rd, sv, doc)
    for b in bad:
        print("FAIL " + b)
    for u in untyped:
        print("FAIL UNTYPED %s: add its kind (docs/STATE-PATHS.json `about`)" % u)
    print("%d paths, %d unknown or unserved, %d untyped" % (len(rd), len(bad), len(untyped)))
    return 1 if bad or untyped else 0


# ---------------------------------------------------------------- selftest
SHELL = ('let a = core.state(path: "smart_folder/\\(id)/count")\n'
         'let b = CoreText.value("view/\\(id)/name", in: m)\n'
         'let c = ctl.core("reply/sources")\n'
         'let d = core.state(path: path)\n'
         '#expect(spy.inner.state(path: "message/\\(first)/archived") == "true")\n')
FIXTURE = ('        if path == "fixture/digest" { return 1 }\n'
           '        if path.hasPrefix("detection_cache/"), path.hasSuffix("/source") {\n'
           '            switch (p.first, p.count) {\n'
           '            case ("message", 3):\n'
           '                switch p[2] {\n'
           '                case "archived": return 1\n'
           '                case "read": return 2\n'
           '                }\n'
           '            case ("ai_provider", 3) where p[2] == "connected": return 1\n'
           '            case ("view", 3) where p[2] == "name": return 1\n'
           '            case ("smart_folder", 3) where p[2] == "count": return 2\n'
           '            case ("inbox", 2): return 3\n')


def selftest(cover=None):
    """`cover` swaps `covers` for a mutant (mutants are listed at the end)."""
    global covers
    real_covers = covers
    if cover:
        covers = cover
    try:
        return _selftest(real_covers)
    finally:
        covers = real_covers


def _selftest(real_covers):
    bad = 0
    def case(name, got, want):
        nonlocal bad
        ok = got == want
        bad += not ok
        print("%-70s %s%s" % (name, "ok" if ok else "FAIL", "" if ok else "  got %r want %r" % (got, want)))
    def files_of(shell=SHELL, fix=FIXTURE):
        return [("Sources/Features/A.swift", shell), ("Sources/CoreBridge/FixtureCore.swift", fix),
                ("Sources/CoreBridge/FixtureX.swift", 'let z = core.state(path: "fixture_only/\\(x)")\n')]
    def run(shell=SHELL, fix=FIXTURE, doc=None):
        rd, names = reads(files_of(shell, fix)); sv = served(files_of(shell, fix))
        return rd, names, sv, check(rd, sv, doc if doc is not None else GOOD)
    typed = lambda *ps: [{"pattern": p, "kind": "string"} for p in ps]
    GOOD = {"paths": typed("smart_folder/*/count", "view/<view>/name", "reply/sources"), "async_commands": {"SaveKey": ["ai_provider/<id>/connected"]}}
    rd, names, sv, (bad_lines, untyped) = run()
    case("reads: the three literal styles, with wildcards", sorted(rd), ["reply/sources", "smart_folder/*/count", "view/*/name"])
    case("reads: the display form names the placeholder after its variable", names["smart_folder/*/count"], "smart_folder/<id>/count")
    case("reads: a variable path and an #expect line are not reads", "message/*/archived" in rd, False)
    case("reads: the fixture's own reads are left out", any("fixture_only" in p for p in rd), False)
    case("served: ==, hasPrefix+hasSuffix, tuple where, nested switch, plain tuple",
         sorted(sv), ["ai_provider/*/connected", "detection_cache/*/source", "fixture/digest", "inbox/*", "message/*/archived", "message/*/read", "smart_folder/*/count", "view/*/name"])
    case("clean sample: no problems, none untyped", (bad_lines, untyped), ([], []))
    m1 = run(doc={"paths": GOOD["paths"][:2], "async_commands": {}})[3][0]
    case("mutant: a read with no row in the file is named with its reader", [("reply/sources" in b and "A.swift" in b) for b in m1], [True])
    m2 = run(fix=FIXTURE.replace('case ("ai_provider", 3) where p[2] == "connected"', 'case ("ai_provider", 3) where p[2] == "key"'))[3][0]
    case("mutant: an async path the fixture stopped serving is named", [("SaveKey" in b and "ai_provider/<id>/connected" in b) for b in m2], [True])
    m3 = run(shell=SHELL + 'let e = core.state(path: "reply/sources2")\n')[3][0]
    case("mutant: the plan's `reply/sources2` read in a pane file is named", [("reply/sources2" in b) for b in m3], [True])
    case("positive control: a row no code reads is not an error", run(doc={"paths": GOOD["paths"] + typed("never/read/*"), "async_commands": {}})[3][0], [])
    # round 5: one notation, covering, untyped
    case("a concrete id read is covered by a typed <placeholder> row", run(shell='let a = core.state(path: "ai_provider/jev/state")\n', doc={"paths": typed("ai_provider/<provider>/state"), "async_commands": {}})[3][0], [])
    case("a `*` read is covered by a `<name>` row", run(shell='let a = core.state(path: "draft/\\(id)/body")\n', doc={"paths": typed("draft/<draft>/body"), "async_commands": {}})[3][0], [])
    case("a different segment is not covered", [("reply/other" in b) for b in run(shell='let a = core.state(path: "reply/other")\n', doc={"paths": typed("reply/sources"), "async_commands": {}})[3][0]], [True])
    u = run(doc={"paths": GOOD["paths"] + [{"pattern": "x/<y>", "kind": None}], "async_commands": {}})[3]
    case("untyped: a row with kind null is counted and named", u[1], ["x/<y>"])
    case("untyped: a typed row is not", run()[3][1], [])
    w = write({"fresh/<a>/b": ["F.swift"]}, {"fresh/<a>/b": "fresh/<a>/b"}, set(), {"paths": [], "async_commands": {}})
    case("--write adds an untyped row (kind null), in the <name> notation", [(r["pattern"], r["kind"]) for r in w["paths"]], [("fresh/<a>/b", None)])
    w2 = write({"ai_provider/jev/state": ["F.swift"]}, {"ai_provider/jev/state": "ai_provider/jev/state"}, set(), {"paths": typed("ai_provider/<provider>/state"), "async_commands": {}})
    case("--write adds no row for a read a typed row covers", [r["pattern"] for r in w2["paths"]], ["ai_provider/<provider>/state"])
    # mutants of the matcher: each must make a case above go red
    exact = lambda a, b: a == b
    never = lambda a, b: False
    for name, fn in (("covering is exact equality", exact), ("nothing covers anything", never)):
        covers_saved = globals()["covers"]
        globals()["covers"] = fn
        try:
            r = run(shell='let a = core.state(path: "ai_provider/jev/state")\n', doc={"paths": typed("ai_provider/<provider>/state"), "async_commands": {}})[3][0]
        finally:
            globals()["covers"] = covers_saved
        case("mutant: %s -> the covered read is reported unknown" % name, len(r), 1)
    saved = globals()["untyped_rows"]
    globals()["untyped_rows"] = lambda doc: []
    try:
        got = run(doc={"paths": GOOD["paths"] + [{"pattern": "x/<y>", "kind": None}], "async_commands": {}})[3][1]
    finally:
        globals()["untyped_rows"] = saved
    case("mutant: untyped rows are not counted -> the untyped case goes red", got != ["x/<y>"], True)
    print("%d problems" % bad)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
