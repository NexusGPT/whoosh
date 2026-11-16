#!/usr/bin/env python3
"""Seam con-seams-pick/split-count: after an edit of the split tabs, a tab's count equals the rows it lists
(C-J1.13 "an edit re-sorts and never deletes mail"; contract: unread is one query). Since the round-5 split
rework a split is a PINNED LABEL: the edit pins the fixture label "Suppliers" through mac-main.splits.pin.<id>
(id read from mac-main.sidebar.labels by raw_name). Expected unread: the fixture label's unread; the tab label,
the rows it lists and the sidebar label row must all show it.
Since delta 13 (FixtureLabels.labelsLocked) a label's unread is the live unread mail filed under it, and the fixture files none under
"Suppliers" at load (main88: 0 on the tab, the rows and the sidebar, the file's fixed `unread: 3` is no longer served). So the check files the
first fixture message (unread, the focused row) under it with L, the way the owner does (J1.5), and expects exactly that one."""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from seamlib import *  # noqa: E402,F401,F403
from seamlib import NJ, nj, pick, equal, fixture, constant_row, constant_int, contract_row, contains_words, norm_words, run  # noqa: E402


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

LABEL = "Suppliers"

def expected():
    fx = fixture()
    def walk(ls):
        for l in ls:
            yield l
            yield from walk(l.get("children", []))
    lab = next(l for l in walk(fx["labels"]) if l["name"] == LABEL)
    first = fx["messages"][0]
    filed = [first["id"]] if first["unread"] and first["id"] == fx["focused_message"] else []
    return {"name": LABEL, "unread": len(filed), "filed": filed, "pinned": bool(lab.get("pinned"))}

def observe(c, exp):
    side = c.rows("sidebar_labels")
    lid = side if nj(side) else next((r.get("id") for r in side if r.get("raw_name") == exp["name"]), NJ("no sidebar label %r" % exp["name"]))
    if nj(lid):
        return {"tab-created": lid, "tab-unread": lid, "rows-unread": lid, "sidebar": lid}
    if not exp["filed"]:
        return {k: NJ("the fixture's first message is not an unread focused row to file under %r" % exp["name"]) for k in ("tab-created", "tab-unread", "rows-unread", "sidebar")}
    c.act("key", "L")
    c.act("type", exp["name"])
    c.act("key", "Enter")
    got = c.core("label_assignment/%s" % exp["filed"][0])
    if nj(got) or got != [exp["name"]]:
        why = got.why if nj(got) else "label_assignment is %r, not %r" % (got, [exp["name"]])
        return {k: NJ("the message was not filed under %r: %s" % (exp["name"], why)) for k in ("tab-created", "tab-unread", "rows-unread", "sidebar")}
    c.act("click", "mac-main.splits.pin.%s" % lid)
    tabs, splits = c.rows("tabs"), c.rows("mac-main.splits")
    idx = splits if nj(splits) else next((i for i, t in enumerate(splits) if t.get("id") == lid), None)
    o = {"tab-created": idx if nj(idx) else idx is not None, "tab-unread": pick(tabs, "id", lid, "unread"),
         "sidebar": pick(c.rows("sidebar_labels"), "id", lid, "count")}
    if o["tab-created"] is True:
        c.act("click", "mac-main.splits.%d" % idx)  # the split strip's click (ListClicks: mac-main.splits.<i>)
        rows = c.rows("list")
        o["rows-unread"] = rows if nj(rows) else sum(1 for r in rows if r.get("unread"))
    else:
        o["rows-unread"] = NJ("no tab to open")
    return o

def judge(o, exp):
    return (equal("tab-created", True, {"tabs": o["tab-created"]})
            + equal("unread", exp["unread"], {"tab-label": o["tab-unread"], "listed-rows": o["rows-unread"], "sidebar-label": o["sidebar"]}))

def oracle(exp):
    return {"tab-created": True, "tab-unread": exp["unread"], "rows-unread": exp["unread"], "sidebar": exp["unread"]}

# MUTANTS
# preview-zero    | the new tab shows the editor's pre-save "0" count                     | unread@tab-label
# rows-second-q   | the tab's rows come from a second query that finds no assignments     | unread@listed-rows
# pin-dead        | the pin control does nothing (act removed)                            | tab-created
MUTANTS = {
    "preview-zero": ("stale count", "unread@tab-label", setk(["tab-unread"], 0)),
    "rows-second-q": ("rows from a second query", "unread@listed-rows", setk(["rows-unread"], 0)),
    "pin-dead": ("dead pin control", "tab-created", setk(["tab-created"], False)),
}
if __name__ == "__main__":
    sys.exit(run("split-count", expected, observe, judge, oracle, MUTANTS))
