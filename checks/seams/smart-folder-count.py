#!/usr/bin/env python3
"""Seam con-seams-s2/smart-folder-count: a smart folder's sidebar count equals its result count, before and
after an archive of a message it holds (contract C-J2.5 "its count equals its result count"). Folder =
the fixture's first smart folder; control = the fixture message with a quote attachment from a supplier."""
import sys, os, json
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from seamlib import *  # noqa: E402,F401,F403
from seamlib import load_corpus, NJ, nj, pick, equal, fixture, constant_row, constant_int, contract_row, contains_words, norm_words, run  # noqa: E402


def first(rows, field):
    if nj(rows):
        return rows
    return rows[0].get(field, NJ("no field %s" % field)) if rows else NJ("region empty")


def setk(path, val):
    def f(o):
        d = o
        for k in path[:-1]:
            d = d[k]
        d[path[-1]] = val
        return o
    return f


def expected():
    fx = fixture()
    cp = json.load(open(os.path.join(ROOT, "fixtures/tests-j2.json"), encoding="utf-8"))
    sf = fx["smart_folders"][0]["name"]
    assert sf == cp["expect"]["smart_folder_name"], "fixture and corpus name different first smart folders"
    ex = cp["expect"]
    return {"name": sf, "query": ex["exact_query"], "held": ex["exact_first_message"]}

# The J2 corpus load empties the fixture's folders, so the judge SAVES one first, through the search screen's save
# door (SearchMacFeature: mac-search.save-as-smart-folder, then the name and Enter, as journeys/J2.py J2.5).
# Count: the folder row's `count` on the search sidebar (mac-search.sidebar.smart-folders). Since round 11 it is the
# core's own row count for the folder's query (`smart_folder/<id>/count`, the figure mac-main.sidebar.smart-folders
# serves too), read again on every read of the rows, so an archive takes the row out of it. List: the rows
# mac-main.list shows after the main sidebar opens the folder (mac-main.sidebar.smart.<id>); its header names the folder.
FOLDERS = "mac-search.sidebar.smart-folders"

def _ids(rows):
    return rows if nj(rows) else [r.get("id") for r in rows]

def _folder(c, exp):
    rows = c.rows(FOLDERS)
    if nj(rows):
        return rows, rows
    for r in rows:
        if r.get("title") == exp["name"]:
            return r.get("count", NJ("folder row has no count")), r.get("id", NJ("folder row has no id"))
    return None, None

def _one(c, exp):
    cnt, fid = _folder(c, exp)
    if cnt is None:
        return {"count": NJ("no saved folder"), "results": NJ("no saved folder"), "held": False, "ids": []}
    if nj(fid):
        return {"count": cnt, "results": fid, "held": fid, "ids": fid}
    c.act("key", "Escape"); c.act("key", "Escape")
    if not c.act("click", "mac-main.sidebar.smart.%s" % fid):
        n = NJ("click mac-main.sidebar.smart.%s refused" % fid)
        return {"count": cnt, "results": n, "held": n, "ids": n}
    ids = _ids(c.rows("list"))
    return {"count": cnt, "results": ids if nj(ids) else len(ids), "held": ids if nj(ids) else exp["held"] in ids, "ids": ids}

def observe(c, exp):
    nc = load_corpus(c)
    if nc:
        z = {"count": nc, "results": nc, "held": nc}
        return {"before": z, "after": z, "focused": nc}
    c.act("key", "Escape"); c.act("key", "/"); c.act("key", "⌘A"); c.act("type", exp["query"]); c.act("key", "Enter")
    c.act("click", "mac-search.save-as-smart-folder"); c.act("type", exp["name"]); c.act("key", "Enter")
    a = _one(c, exp)
    ids = a.pop("ids")
    if nj(ids) or exp["held"] not in ids:
        n = NJ("the held message is not in the opened folder: no archive to make")
        return {"before": a, "after": {"count": n, "results": n, "held": n}, "focused": n}
    # The held row is addressed by its ID (its ax id is read from the folder's list by the row's id; the click focuses it), never by counting J
    # presses from the top: a row ahead of it in the list (another message, a seeded row) would move the count and archive a neighbour.
    ax = pick(c.rows("list"), "id", exp["held"], "ax")
    focused = ax if nj(ax) else (c.act("click", ax) and c.ui("focus_message"))
    if nj(focused) or focused != exp["held"]:
        n = NJ("the held message %s is not focused (focus %r): no archive is made" % (exp["held"], focused))
        return {"before": a, "after": {"count": n, "results": n, "held": n}, "focused": focused}
    c.act("key", "E")  # archive the held message from the folder's list
    gone = c.core("message/%s/archived" % exp["held"])
    if gone is not True:
        n = NJ("archive of %s did not take effect (core message/%s/archived = %r)" % (exp["held"], exp["held"], gone if not nj(gone) else gone.why))
        return {"before": a, "after": {"count": n, "results": n, "held": n}, "focused": focused}
    b = _one(c, exp)
    b.pop("ids")
    return {"before": a, "after": b, "focused": focused}

def judge(o, exp):
    out = equal("control-held-in-results", True, {"before": o["before"]["held"]})
    out += equal("held-focused", exp["held"], {"folder-list": o["focused"]})    # the archive's address: the count below moves only when THIS row is archived
    for k in ("before", "after"):
        r, cnt = o[k]["results"], o[k]["count"]
        if nj(r) or nj(cnt):
            out.append(("count-equals-results@%s" % k, None, "; ".join(x.why for x in (cnt, r) if nj(x))))
        else:
            out.append(("count-equals-results@%s" % k, cnt == r, "sidebar %r, results %r" % (cnt, r)))
    return out

def oracle(exp):
    # equality only: 3 and 2 stand in for "some N, then N-1"; the fixture states the rule in words, never a count
    return {"before": {"count": 3, "results": 3, "held": True}, "after": {"count": 2, "results": 2, "held": False}, "focused": exp["held"]}

# MUTANTS
# cached-count | the sidebar count is stored at save time, not re-queried after the archive | count-equals-results@after
# wrong-row    | the click on a row of the folder's list focuses another row                      | held-focused
# empty-folder | the folder matches nothing: the positive control goes red                  | control-held-in-results
MUTANTS = {
    "wrong-row": ("click focuses another row", "held-focused", setk(["focused"], "msg-s02")),
    "cached-count": ("count cached at save", "count-equals-results@after", setk(["after", "count"], 3)),
    "empty-folder": ("dead rule", "control-held-in-results", lambda o: (o["before"].update(results=0, count=0, held=False) or o)),
}
# ---------------------------------------------------------------- --selftest: observe() driven against a stand-in socket
# The judge's mutants prove the JUDGE; these prove the DRIVE. The model is written from CONTROL-SOCKET.md and the corpus file, never from the app's
# reply: the J2 corpus load replaces the fixture's folders, the search door saves one, the main sidebar opens it, the list holds the folder's
# messages (the held message is NOT the first row: positional archiving would hit a neighbour), and an archive takes a row out of the list and
# out of the count. Clean it must be green on every label; with one defect of the app at a time the label that defect breaks must go red.
import seamlib as _seamlib


class ModelCtl(_seamlib.Ctl):
    def __init__(self, defect=None):
        self.defect, self.path, self.s, self.buf, self.log = defect, "model", True, b"", []
        cp = json.load(open(os.path.join(ROOT, "fixtures/tests-j2.json"), encoding="utf-8"))
        self.held, self.name = cp["expect"]["exact_first_message"], cp["expect"]["smart_folder_name"]
        self.members = ["msg-s02", self.held, "msg-01b", "msg-s03"]         # the folder's result list: the held message stands second
        self.saved, self.focus, self.archived, self.frozen = False, None, set(), None

    @property
    def live(self):
        return True

    def _live(self):
        return [m for m in self.members if m not in self.archived]

    def call(self, verb, *args):
        d = self.defect
        if verb == "fixture" and args[0] == "corpus":
            return {"ok": True}
        if verb == "key" or verb == "type":
            return {"ok": True} if verb == "type" or args[0] != "E" else self._archive()
        if verb == "click" and args[0] == "mac-search.save-as-smart-folder":
            self.saved = True
            self.frozen = len(self._live())
            return {"ok": True}
        if verb == "click" and args[0].startswith("mac-main.sidebar.smart."):
            return {"ok": True} if self.saved else {"ok": False, "error": "nothing to click"}
        if verb == "click" and args[0].startswith("mac-main.list.row."):
            if d != "click-no-focus":
                self.focus = self._live()[int(args[0].rsplit(".", 1)[1])]
            return {"ok": True}
        if verb == "uistate":
            return {"ok": True, "state": {"focus_message": self.focus}}
        if verb == "core":
            return {"ok": True, "value": args[1] == "message/%s/archived" % self.held and self.held in self.archived}
        if verb == "rows":
            if args[0] == "mac-search.sidebar.smart-folders":
                n = self.frozen if d == "count-cached" else len(self._live())
                return {"ok": True, "rows": [{"id": "sf-1", "title": self.name, "count": n}] if self.saved else []}
            if args[0] == "mac-main.list":
                return {"ok": True, "rows": [{"ax": "mac-main.list.row.%d" % i, "id": m} for i, m in enumerate(self._live())]}
        return {"ok": False, "error": "%s %s: not modelled" % (verb, args)}

    def _archive(self):
        if self.focus in self._live():
            self.archived.add(self.focus)
        return {"ok": True}


DEFECTS = {
    "click-no-focus": ("a click on a row does not focus it: the archive hits the focused neighbour or nothing", "held-focused"),
    "count-cached": ("the sidebar count is cached at save time and not re-read after an archive", "count-equals-results"),
}


def selftest():
    import io, contextlib
    bad = []
    def run_model(defect):
        _seamlib.Ctl = lambda: ModelCtl(defect)
        buf = io.StringIO()
        # the library's own `--selftest` (11 Nov) reads no app at all; this self-test DRIVES the model, so the word is taken out of the arguments for the call
        # (with it, the run printed no label and every defect below read STAYED GREEN)
        argv, sys.argv = sys.argv, [a for a in sys.argv if a != "--selftest"]
        try:
            with contextlib.redirect_stdout(buf):
                rc = _seamlib.run("smart-folder-count", expected, observe, judge, oracle, MUTANTS)
        finally:
            sys.argv = argv
        live = [l for l in buf.getvalue().split("== smart-folder-count  MUTANTS")[0].splitlines() if l.startswith("  ")]
        return rc, live
    rc, live = run_model(None)
    red = [l for l in live if l.strip().startswith(("FAIL", "NOT JUDGED"))]
    print("clean model: exit %d, %d labels, not green: %s" % (rc, len(live), red))
    if rc != 0 or red or len(live) != 4:
        bad.append("clean")
    print("| defect of the app | label that must go red | seen |")
    print("|---|---|---|")
    for d, (what, must) in DEFECTS.items():
        _, live = run_model(d)
        hit = [l.strip() for l in live if l.strip().startswith("FAIL") and must in l]
        print("| %s | %s | %s |" % (what, must, "RED: " + hit[0] if hit else "STAYED GREEN"))
        if not hit:
            bad.append(d)
    print("selftest: %d problems" % len(bad))
    return 1 if bad else 0


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        sys.exit(selftest())
    sys.exit(run("smart-folder-count", expected, observe, judge, oracle, MUTANTS))
