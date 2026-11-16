#!/usr/bin/env python3
"""Seam con-seams-s2/search-colour: a search result names its account in the SAME colour as the sidebar
chip (contract C-J2.1 "account colour per result"; C-PICK-mark). Query = the fixed sender of the fixture's
focused message (positive control: that message must be among the results)."""
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
    ex = cp["expect"]
    colour = {a["id"]: a["colour"] for a in fx["accounts"]}
    acc = {m["id"]: m["account"] for m in cp["messages"]}
    mid = ex["exact_first_message"]
    return {"exact": ex["exact_query"], "meaning": ex["meaning_query"], "id": mid, "acc": acc[mid], "colour": colour[acc[mid]],
            "must_threads": ex["meaning_query_must_include_threads"], "by_msg": {k: colour[v] for k, v in acc.items()}, "all": colour}

def _search(c, q):
    c.act("key", "Escape"); c.act("key", "/")
    c.act("key", "⌘A"); c.act("type", q); c.act("key", "Enter")  # search runs on Enter (journeys/J2.py, main39)
    return c.rows("search")

def observe(c, exp):
    nc = load_corpus(c)
    if nc:
        return {"present": nc, "colour": {"search-result": nc, "sidebar-chip": nc, "account-book": nc, "list-row": nc}, "every": nc, "threads": nc}
    book = c.rows("mac-main.account-book")
    side = c.rows("sidebar_accounts")
    lst = c.rows("list")
    res = _search(c, exp["exact"])
    many = _search(c, exp["meaning"])
    every = many if nj(many) else {r.get("id"): r.get("colour") for r in many}
    return {"present": res if nj(res) else exp["id"] in [r.get("id") for r in res],
            "colour": {"search-result": pick(res, "id", exp["id"], "colour"), "sidebar-chip": pick(side, "id", exp["acc"], "colour"),
                       "account-book": pick(book, "id", exp["acc"], "colour"), "list-row": pick(lst, "id", exp["id"], "account_colour")},
            "every": every, "threads": many if nj(many) else sorted(set(r.get("thread") for r in many))}

def judge(o, exp):
    out = equal("control-result-present", True, {"search": o["present"]}) + equal("colour", exp["colour"], o["colour"])
    ev = o["every"]
    if nj(ev):
        out.append(("colour-every-result@search", None, ev.why))
    else:
        want = {k: exp["by_msg"].get(k) for k in ev}
        ok = bool(ev) and ev == want
        out.append(("colour-every-result@search", ok, "" if ok else "got %r, expected %r" % (ev, want)))
        have = o["threads"]
        miss = [t for t in exp["must_threads"] if t not in have]
        out.append(("control-threads-present@search", not miss, "" if not miss else "missing %r in %r" % (miss, have)))
    return out

def oracle(exp):
    ev = {"msg-01": exp["colour"], "msg-s05": exp["by_msg"]["msg-s05"]}
    return {"present": True, "every": ev, "threads": sorted(exp["must_threads"]),
            "colour": {"search-result": exp["colour"], "sidebar-chip": exp["colour"], "account-book": exp["colour"], "list-row": exp["colour"]}}

E = expected()
# MUTANTS
# provider-colour | the result paints the first account of the same provider             | colour@search-result
# one-row-wrong   | one row of a longer result list takes another account's colour         | colour-every-result
# no-results      | search returns nothing (dead bar): the positive control goes red       | control-result-present
MUTANTS = {
    "provider-colour": ("colour by provider", "colour", setk(["colour", "search-result"], E["all"]["acc-01"])),
    "one-row-wrong": ("a row mis-coloured", "colour-every-result", setk(["every", "msg-s05"], E["all"]["acc-01"])),
    "no-results": ("dead search", "control-result-present", setk(["present"], False)),
}
if __name__ == "__main__":
    sys.exit(run("search-colour", expected, observe, judge, oracle, MUTANTS))
