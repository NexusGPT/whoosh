#!/usr/bin/env python3
"""Seam con-seams-s1/account-colour: one account, one colour. The sidebar chip, the list row and the
reader header show the fixture colour of the row's account (fixture accounts[].colour; contract
C-PICK-mark "the same account shows the same icon id and colour on every surface").

    sh scripts/launch-one.sh <bundle> -- --fixture-core python3 checks/seams/account-colour.py
"""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from seamlib import fixture, pick, nj, NJ, equal, run  # noqa: E402


def expected():
    fx = fixture()
    colour = {a["id"]: a["colour"] for a in fx["accounts"]}
    return {"rows": [(m["id"], m["account"], colour[m["account"]]) for m in fx["messages"]],
            "focused": fx["focused_message"], "accounts": [a["colour"] for a in fx["accounts"]]}


def observe(c, exp):
    # main41: the list row serves account_colour; the sidebar's account colours are served on mac-main.account-book
    rows, side = c.rows("list"), c.rows("mac-main.account-book")
    o = {}
    for mid, acc, _ in exp["rows"]:
        lc = pick(rows, "id", mid, "account_colour")
        o[mid] = {"list-row": lc, "account-book": pick(side, "id", acc, "colour")}
    c.act("key", "Enter")  # open the focused row in the reader
    hdr = c.rows("reader_header")
    o[exp["focused"]]["reader-header"] = hdr if nj(hdr) else (hdr[0].get("colour", NJ("header has no colour")) if hdr else NJ("header empty"))
    return o


def judge(o, exp):
    out = []
    for mid, _, col in exp["rows"]:
        out += equal("colour:%s" % mid, col, o[mid])
    return out


def oracle(exp):
    o = {mid: {"list-row": c, "account-book": c} for mid, _, c in exp["rows"]}
    o[exp["focused"]]["reader-header"] = dict((m, c) for m, _, c in exp["rows"])[exp["focused"]]
    return o


E = expected()
F = E["focused"]
# MUTANTS
# index-lookup     | the list row takes accounts[i] by its own position, not by its account id     | colour:msg-01
# reader-filter    | the reader header paints the account filter's first account, not the thread's  | colour:<focused>
# sidebar-shift    | the sidebar chips are off by one account (colour of the next account)          | colour:msg-03
MUTANTS = {
    "index-lookup": ("row colour by position", "colour:msg-01",
                     lambda o: (o["msg-01"].__setitem__("list-row", E["accounts"][0]) or o)),
    "reader-filter": ("reader colour from the filter", "colour:%s" % F,
                      lambda o: (o[F].__setitem__("reader-header", E["accounts"][0]) or o)),
    "sidebar-shift": ("sidebar chip off by one", "colour:msg-03",
                      lambda o: (o["msg-03"].__setitem__("account-book", E["accounts"][18]) or o)),
}

if __name__ == "__main__":
    sys.exit(run("account-colour", expected, observe, judge, oracle, MUTANTS))
