#!/usr/bin/env python3
"""Seam con-seams-s1/undo: one undo semantics. An archive from the list, from the reader and from the
palette each pushes exactly ONE undo entry, and Z puts the row back at its index and consumes the entry
(contract C-J1.2 "one undo-entry per action", C-J1.8 "archived -> back at its index").

    sh scripts/launch-one.sh <bundle> -- --fixture-core python3 checks/seams/undo.py
"""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from seamlib import fixture, nj, NJ, equal, run  # noqa: E402

PATHS = ("list", "reader", "palette")


def expected():
    fx = fixture()
    ids = [m["id"] for m in fx["messages"]]
    return {"target": fx["focused_message"], "index": ids.index(fx["focused_message"])}


def _ids(c):
    r = c.rows("list")
    return r if nj(r) else [x.get("id") for x in r]


def _act(c, path):
    if path == "list":
        c.act("key", "E")
    elif path == "reader":
        c.act("key", "Enter")
        c.act("key", "E")
    else:
        c.act("key", "⌘P")  # the palette's key on main41 (MacFootHints: "palette": "⌘P"); ⌘K opens search
        c.act("type", "archive")
        c.act("key", "Enter")


def observe(c, exp):
    o = {}
    for p in PATHS:
        n0 = c.core("undo_entry/count")
        _act(c, p)
        ids = _ids(c)
        n1 = c.core("undo_entry/count")
        c.act("key", "Z")
        ids2 = _ids(c)
        o[p] = {"gone": ids if nj(ids) else exp["target"] not in ids,
                "entries": n0 if nj(n0) else (n1 if nj(n1) else n1 - n0),
                "index": ids2 if nj(ids2) else (ids2.index(exp["target"]) if exp["target"] in ids2 else NJ("target not back in the list")),
                "consumed": c.core("undo_entry/consumed_last")}
        if nj(o[p]["index"]):  # NJ for a missing row would hide a real failure: absent after Z is a FAIL
            o[p]["index"] = None if not nj(ids2) else o[p]["index"]
        c.act("key", "Escape")
    return o


def _gated(o, p, k):
    # Z is only judged on a path whose act took effect: after a dead act the row never left, so
    # "back at index" and "entry consumed" would pass as decoration (seen live on the palette path).
    return o[p][k] if o[p]["gone"] is True else NJ("act had no effect on the %s path, Z not judged" % p)


def judge(o, exp):
    return (equal("gone-after-act", True, {p: o[p]["gone"] for p in PATHS})
            + equal("one-entry", 1, {p: o[p]["entries"] for p in PATHS})
            + equal("back-at-index", exp["index"], {p: _gated(o, p, "index") for p in PATHS})
            + equal("entry-consumed", True, {p: _gated(o, p, "consumed") for p in PATHS}))


def oracle(exp):
    return {p: {"gone": True, "entries": 1, "index": exp["index"], "consumed": True} for p in PATHS}


def _m(path, **kv):
    return lambda o: (o[path].update(kv) or o)


# MUTANTS
# palette-double   | the palette path pushes two entries (command + archive)                       | one-entry@palette
# reader-to-end    | undo from the reader re-inserts the row at the end of the list                 | back-at-index@reader
# palette-dead     | the palette archive command does nothing (act removed)                          | gone-after-act@palette
# list-z-dead      | Z does nothing after a list archive: the row stays gone, entry unconsumed       | entry-consumed@list
# stale-consumed   | Z does nothing but consumed_last still reads true from an earlier undo          | back-at-index@list
MUTANTS = {
    "palette-double": ("two entries for one action", "one-entry@palette", _m("palette", entries=2)),
    "reader-to-end": ("undo restores at the end", "back-at-index@reader", _m("reader", index=len(fixture()["messages"]) - 1)),
    "palette-dead": ("dead palette command", "gone-after-act@palette", _m("palette", gone=False, entries=0)),
    "list-z-dead": ("dead Z", "entry-consumed@list", _m("list", consumed=False, index=None)),
    "stale-consumed": ("dead Z, stale flag", "back-at-index@list", _m("list", index=None)),
}

if __name__ == "__main__":
    sys.exit(run("undo", expected, observe, judge, oracle, MUTANTS))
