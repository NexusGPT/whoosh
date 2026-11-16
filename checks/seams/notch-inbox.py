#!/usr/bin/env python3
"""Seam con-seams-s6/notch-inbox: an archive in the notch equals an archive in the list: one undo entry, the
same count change, the row gone from both, and Z restores it (C-J6.5 "the same undo-entry as a list archive").
The arriving mail is tests-j6's "next-mail" arrival, delivered by 'fixture deliver <arrival> <file> <source>' (the notch
shows an arrival, and the same row is an inbox row in the core). The notch's first row is row 1 (NotchIds.archive)."""
import json
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from seamlib import *  # noqa: E402,F401,F403
from seamlib import ROOT, NJ, nj, pick, equal, fixture, constant_row, constant_int, contract_row, contains_words, norm_words, run  # noqa: E402


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


J6 = os.path.join(ROOT, "fixtures", "tests-j6.json")


def expected():
    fx = fixture()
    with open(J6, encoding="utf-8") as f:
        arr = json.load(f)["arrivals"]["next-mail"]
    return {"id": arr["id"], "arrival": "next-mail", "tabs": [t["id"] for t in fx["tabs"]], "drop": 1}


def unread_total(c, tabs):
    """The unread count across the fixture's tabs, from the app's own counts: the arrival lands on whichever tab the label
    rules file it under (a tab with no count, as `social` on main55, adds nothing)."""
    v = c.ui("counts")
    if nj(v):
        return v
    return sum(v.get(t, 0) for t in tabs)


def observe(c, exp):
    c.act("fixture", "deliver", exp["arrival"], J6, "eventsource")
    n0, k0 = c.core("undo_entry/count"), unread_total(c, exp["tabs"])
    notch0 = c.rows("notch_list")
    c.act("click", "mac-notch.list.row.1.archive")
    n1, k1 = c.core("undo_entry/count"), unread_total(c, exp["tabs"])
    lst, notch = c.rows("list"), c.rows("notch_list")
    c.act("key", "Z")
    lst2 = c.rows("list")
    ids = lambda r: r if nj(r) else [x.get("id") for x in r]
    d = lambda a, b: a if nj(a) else (b if nj(b) else b - a)
    return {"control-in-notch": ids(notch0) if nj(ids(notch0)) else exp["id"] in ids(notch0),
            "entries": d(n0, n1), "count-drop": d(k1, k0),
            "gone": {"list": ids(lst) if nj(ids(lst)) else exp["id"] not in ids(lst), "notch": ids(notch) if nj(ids(notch)) else exp["id"] not in ids(notch)},
            "undo-restores-list": ids(lst2) if nj(ids(lst2)) else exp["id"] in ids(lst2)}

def judge(o, exp):
    return (equal("control-in-notch", True, {"notch": o["control-in-notch"]}) + equal("one-entry", 1, {"core": o["entries"]})
            + equal("count-drop", exp["drop"], {"core": o["count-drop"]}) + equal("gone", True, o["gone"])
            + equal("undo-restores", True, {"list": o["undo-restores-list"] if o["gone"]["list"] is True
                                            else NJ("the archive never left the list; Z not judged")}))

def oracle(exp):
    return {"control-in-notch": True, "entries": 1, "count-drop": exp["drop"], "gone": {"list": True, "notch": True}, "undo-restores-list": True}

# MUTANTS
# notch-no-entry | the notch archive bypasses the undo stack                     | one-entry
# list-kept      | the notch hides the mail but the list keeps the row           | gone@list
# act-deleted    | the archive click does nothing                                 | gone@notch
MUTANTS = {
    "notch-no-entry": ("no undo entry", "one-entry", lambda o: (o.update(entries=0, **{"undo-restores-list": False}) or o)),
    "list-kept": ("list not updated", "gone", lambda o: (o["gone"].update(list=False) or o.update(**{"count-drop": 0}) or o)),
    "act-deleted": ("dead archive", "gone", lambda o: (o["gone"].update(list=False, notch=False) or o.update(entries=0) or o)),
}
if __name__ == "__main__":
    sys.exit(run("notch-inbox", expected, observe, judge, oracle, MUTANTS))
