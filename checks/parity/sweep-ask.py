#!/usr/bin/env python3
"""sweep-ask: the control sweep of Ask (`mac-ask`). Owner, 2026-11-11: "you haven't properly tested every button/feature".

RESULT OF THE READING: Ask has no control to click. The Ask panel was retired on purpose (commit 59f91f249 "Retire the Ask panel (Sources/Features/AskMac, par-askmac, the citation seam;
askmac lane retired in the partition; roster without mac-ask)"; docs/DESIGN.md:26 "mac-ask retires into mac-search ... the results are the matching messages and nothing else"; SPEC A26 "no
separate Ask AI"; docs/JOURNEYS.md J5.4). `mac-ask` is not in the screens of checks/shots-sheet.py SCREENS. What remained was one word in Sources/App/WindowBridge.swift:15 (the window title
table `titleKeys`: "mac-ask": "Ask"; lane fix-rest-app removed that entry, nothing read it: now no code line names `mac-ask`), and the question a person used to ask is typed in the search field of `mac-search` (its controls are swept by par-searchmac J2.9).
So this sweep proves the retirement, one case each, instead of clicking controls that do not exist (and never fakes a control):

  ask-panel-folder-is-gone                static: Sources/Features/AskMac does not exist
  mac-ask-is-in-no-source-line            static: no code line of the Swift sources names `mac-ask` (a comment that says why the panel is gone is not code)
  no-mac-ask-door-or-row-answers          driven: no click and no row of `mac-ask.*` answers (a door that answers would be a control no case clicks)
  the-question-goes-to-the-search-field   driven: the rows Ask used to fill (the answer is the results) are served by `mac-search.results` and `.dropdown` (the suggestions)

    python3 checks/parity/sweep-ask.py              drive the running rig copy (hidden, --fixture-core)
    python3 checks/parity/sweep-ask.py --dry        the same code with no socket: the two driven cases are `unjudged`, the count is exact
    python3 checks/parity/sweep-ask.py --selftest   every case against a model, clean, then once per case against its own broken copy: each must go red

Prints one `name  pass|fail|unjudged  detail` line per case (print_cases of journeys/lib/harness.py), the MUTANTS count, then 'N checks, F failed, U not judged'."""
import json
import os
import re
import shutil
import socketserver
import sys
import tempfile
import threading

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, Unreadable                                     # noqa: E402
from lib.harness import Journey, run_journey, print_cases, declared_mutants   # noqa: E402

ID = "par-ask-sweep"
TITLE = "Ask: the retired panel has no control, no door and no row"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "no mac-ask door or row answers; the question is answered by the search results"
TREE = [ROOT]          # the tree the static cases read; the self-test points it at a scratch copy
STATIC = {"ask-panel-folder-is-gone", "mac-ask-is-in-no-source-line"}
CASES = []


class Red(Exception):
    """The app (or the tree) answered and the answer is wrong: the case is red."""


def case(name, fn=None):
    """`case(name, fn)` or the decorator `@case(name)`."""
    def add(f):
        CASES.append((name, f))
        return f
    return add(fn) if fn else add


def need(cond, words):
    if not cond:
        raise Red(words)


def refused(ctl, verb, *args):
    """True when the app answers ok:false (nothing registered, or it did nothing). Any other failure (no socket, a timeout) stays Unreadable: not judged."""
    try:
        ctl.send(verb, *args)
        return False
    except Unreadable as e:
        if "nothing" in str(e) or "no rows" in str(e) or "did nothing" in str(e) or "unknown" in str(e):
            return True
        raise


def swift_files(root):
    out = []
    for d, _dirs, files in os.walk(os.path.join(root, "Sources")):
        out += [os.path.join(d, f) for f in files if f.endswith(".swift")]
    return sorted(out)


@case("ask-panel-folder-is-gone")
def _folder(ctl):
    p = os.path.join(TREE[0], "Sources", "Features", "AskMac")
    need(not os.path.exists(p), "%s exists again: the Ask panel is back without a sweep" % p)
    return "Sources/Features/AskMac does not exist"


@case("mac-ask-is-in-no-source-line")
def _title(ctl):
    hits = []
    for f in swift_files(TREE[0]):
        for n, line in enumerate(open(f, encoding="utf-8"), 1):
            if "mac-ask" in line and not line.lstrip().startswith("//"):      # a comment that says why the panel is gone is not code
                hits.append("%s:%d" % (os.path.relpath(f, TREE[0]), n))
    need(not hits, "`mac-ask` appears in %s; the retired panel is named by no code line (its window title entry was removed)" % hits)
    return "no code line of %d Swift files names `mac-ask`" % len(swift_files(TREE[0]))


GHOSTS = [("click", "mac-ask.open"), ("click", "mac-ask.send"), ("click", "mac-ask.citation.1"), ("click", "mac-ask.proposal.approve"),
          ("rows", "mac-ask.citations"), ("rows", "mac-ask.refusal"), ("rows", "mac-ask.proposal")]


def _doors(ctl):
    answered = [f"{v} {a}" for v, a in GHOSTS if not refused(ctl, v, a)]
    need(not answered, "the app answers on the retired Ask panel: %s" % answered)
    return "%d ids of the retired panel (clicks and rows) are all refused" % len(GHOSTS)


def _search(ctl):
    got = {}
    for region in ("mac-search.results", "mac-search.dropdown"):
        need(not refused(ctl, "rows", region), "%s is not registered: the question has nowhere to be answered" % region)
        got[region] = len(ctl.rows(region))
    need(set(got) == {"mac-search.results", "mac-search.dropdown"}, "search rows: %r" % got)
    return "the answer is the search results: %s are served (%s)" % (", ".join(got), ", ".join("%s: %d rows" % kv for kv in got.items()))


case("no-mac-ask-door-or-row-answers", _doors)
case("the-question-goes-to-the-search-field", _search)


def run_case(s, name, fn):
    def run():
        ctl = None if name in STATIC else s.ctl
        try:
            return True, fn(ctl)
        except Red as e:
            return False, str(e)
    s._record(name, run)


def drive(j, only=None):
    s = j.step(ID, TITLE)
    for name, fn in CASES:
        if only is None or name == only:
            run_case(s, name, fn)


# MUTANTS: per case, the change that must turn it red (the self-test makes the same change in a scratch copy of the tree, or in its model).
# par-ask-sweep/ask-panel-folder-is-gone | Sources/Features/AskMac/AskFeature.swift is added again (any Swift file in a folder named AskMac)
# par-ask-sweep/mac-ask-is-in-no-source-line | Sources/App/WindowBridge.swift titleKeys gets `"mac-ask": "Ask"` back (the old entry), or any FeatureModule names it: a door `"mac-ask.open": { _ in true }` in its clickTargets
# par-ask-sweep/no-mac-ask-door-or-row-answers | a feature registers the click `mac-ask.open` (the model answers it ok:true)
# par-ask-sweep/the-question-goes-to-the-search-field | `mac-search.results` is no longer registered (SearchMacFeature.rowSources drops it; the model refuses it)


# ---------------------------------------------------------------- the self-test
class Model:
    """The app as it should be: nothing under mac-ask answers; the search rows do. One mutant at a time."""
    def __init__(self, mut):
        self.m = mut

    def handle(self, verb, args):
        args = args or [""]
        if verb == "click" and args[0] == "mac-ask.open" and "no-mac-ask-door-or-row-answers" in self.m:
            return {"ok": True}
        if verb == "rows" and args[0] in ("mac-search.results", "mac-search.dropdown") and "the-question-goes-to-the-search-field" not in self.m:
            return {"ok": True, "rows": []}
        return {"ok": False, "error": "nothing registered at %s" % args[0]}


def model_run(mut, only=None):
    d = tempfile.mkdtemp(prefix="sw-ask-")
    model = Model(mut)

    class H(socketserver.StreamRequestHandler):
        def handle(self):
            for line in self.rfile:
                q = json.loads(line.decode())
                self.wfile.write((json.dumps(model.handle(q["verb"], q["args"])) + "\n").encode())
    srv = socketserver.ThreadingUnixStreamServer(os.path.join(d, "whoosh.sock"), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    c = Ctl(d).connect()
    try:
        j = Journey(ID, c)
        drive(j, only)
        return {x.label.split("/", 1)[1]: (x.state, x.detail) for x in j.checks()}
    finally:
        c.close()
        srv.shutdown()
        srv.server_close()
        shutil.rmtree(d, ignore_errors=True)


def scratch(extra_file=None, extra_text="", title_entry=False):
    """A tree with only what the static cases read: the real WindowBridge.swift (with the old `mac-ask` title entry put back when `title_entry`), optionally one more file."""
    t = tempfile.mkdtemp(prefix="sw-ask-tree-")
    os.makedirs(os.path.join(t, "Sources", "App"))
    shutil.copy(os.path.join(ROOT, "Sources", "App", "WindowBridge.swift"), os.path.join(t, "Sources", "App", "WindowBridge.swift"))
    if title_entry:
        wb = os.path.join(t, "Sources", "App", "WindowBridge.swift")
        text = open(wb, encoding="utf-8").read()
        assert '"mac-palette": "Command palette", "mac-search": "Search",' in text
        open(wb, "w", encoding="utf-8").write(text.replace('"mac-palette": "Command palette", "mac-search": "Search",', '"mac-palette": "Command palette", "mac-ask": "Ask", "mac-search": "Search",'))
    if extra_file:
        p = os.path.join(t, extra_file)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        open(p, "w", encoding="utf-8").write(extra_text)
    return t


def static_red(name, tree):
    TREE[0] = tree
    try:
        [fn for n, fn in CASES if n == name][0](None)
        return False
    except Red:
        return True
    finally:
        TREE[0] = ROOT
        shutil.rmtree(tree, ignore_errors=True)


def selftest():
    bad = 0
    clean = model_run(set())
    clean.update({n: ("pass", fn(None)) for n, fn in CASES if n in STATIC and static_ok(n)})
    for name, _fn in CASES:
        st = clean.get(name, ("missing", ""))
        print("model %-44s %-8s %s" % (name, st[0], " ".join(st[1].split())[:150]))
        bad += st[0] != "pass"
    declared = {l.split(" | ")[0][2:].strip().split("/", 1)[1] for l in open(os.path.abspath(__file__), encoding="utf-8") if l.startswith("# " + ID + "/")}
    reds = {
        "ask-panel-folder-is-gone": lambda: static_red("ask-panel-folder-is-gone", scratch("Sources/Features/AskMac/AskFeature.swift", "// the panel is back\n")),
        "mac-ask-is-in-no-source-line": lambda: static_red("mac-ask-is-in-no-source-line", scratch(title_entry=True)) and
                                            static_red("mac-ask-is-in-no-source-line", scratch("Sources/Features/X/XFeature.swift", 'out["mac-ask.open"] = { _ in true }\n')),
        "no-mac-ask-door-or-row-answers": lambda: model_run({"no-mac-ask-door-or-row-answers"}, "no-mac-ask-door-or-row-answers")["no-mac-ask-door-or-row-answers"][0] == "fail",
        "the-question-goes-to-the-search-field": lambda: model_run({"the-question-goes-to-the-search-field"}, "the-question-goes-to-the-search-field")["the-question-goes-to-the-search-field"][0] == "fail",
    }
    for name, _fn in CASES:
        if name not in declared:
            print("NO MUTANT declared for %s" % name)
            bad += 1
        red = reds[name]()
        print("mutant %-44s -> %s" % (name, "RED" if red else "NOT RED"))
        bad += not red
    for l in sorted(declared - {c[0] for c in CASES}):
        print("STRAY MUTANT %s" % l)
        bad += 1
    dry = Journey(ID, None)
    drive(dry)
    states = {c.label.split("/", 1)[1]: c.state for c in dry.checks()}
    ok = len(states) == len(CASES) and all(st == ("pass" if n in STATIC else "unjudged") for n, st in states.items())
    print("whole check, undriven: %d cases (the driven ones unjudged, the two source ones read): %s" % (len(states), ok))
    bad += not ok
    print("selftest: %d cases, %d mutants, %d not red or missing" % (len(CASES), len(declared), bad))
    if not bad:
        print("selftest: every check was seen red on its broken model")
    return 1 if bad else 0


def static_ok(name):
    try:
        [fn for n, fn in CASES if n == name][0](None)
        return True
    except Red:
        return False


def main():
    path = os.path.abspath(__file__)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    print_cases(r)
    print("MUTANTS: %d declared in %s (a check is green only after it was seen red on its broken copy)" % (len(declared_mutants(path)), os.path.basename(path)))
    for line in r["fail_list"]:
        sys.stderr.write("FAIL %s\n" % line)
    for line in r["no_mutant"]:
        sys.stderr.write("NO MUTANT %s\n" % line)
    for line in r["stray_mutant"]:
        sys.stderr.write("STRAY MUTANT %s\n" % line)
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    return 0 if r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"] else 1


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else main())
