#!/usr/bin/env python3
"""Seam con-seams-pick/account-mark: one account shows the same icon id and colour on the list row, the sidebar
ring, the reader meta, the search result, the notch and the iPhone row (C-PICK-mark; C-J4.12 "one AccountMark
control renders every place an account is named"). Expected: the fixture account's icon_id and colour.
Served fields: list row account_icon / account_colour; the sidebar ring (SidebarFeature.swift:42), the reader header
(ReaderHeaderProbe.swift:15) and the search result (SearchMacRows.swift:83) serve icon and colour too; a surface that
serves no field is not judged, named per surface. A search that
returns nothing for the row's sender is a broken probe: its surface is not judged, and the control is red."""
import sys, os, json
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

J6 = os.path.join(ROOT, "fixtures", "tests-j6.json")


def expected():
    fx = fixture()
    m = next(x for x in fx["messages"] if x["id"] == fx["focused_message"])
    a = next(x for x in fx["accounts"] if x["id"] == m["account"])
    arr = json.load(open(J6, encoding="utf-8"))["arrivals"]
    name = next(k for k, v in arr.items() if v["account"] == a["id"])
    return {"id": m["id"], "acc": a["id"], "colour": a["colour"], "icon": a.get("icon_id"), "sender": m["sender"],
            "arrival": name, "arrival_id": arr[name]["id"]}

def observe(c, exp):
    lst, side = c.rows("list"), c.rows("sidebar_accounts")
    s = {"list-row": pick(lst, "id", exp["id"], "account_icon"), "sidebar-ring": pick(side, "id", exp["acc"], "icon")}
    cl = {"list-row": pick(lst, "id", exp["id"], "account_colour"), "sidebar-ring": pick(side, "id", exp["acc"], "colour")}
    c.act("key", "Enter"); h = c.rows("reader_header")
    s["reader-meta"], cl["reader-meta"] = first(h, "account_icon"), first(h, "account_colour")
    c.act("key", "Escape"); c.act("key", "/"); c.act("key", "⌘A"); c.act("type", exp["sender"]); c.act("key", "Enter"); r = c.rows("search")
    found = r if nj(r) else exp["id"] in [x.get("id") for x in r]
    if found is True:
        s["search-result"], cl["search-result"] = pick(r, "id", exp["id"], "account_icon"), pick(r, "id", exp["id"], "colour")
    else:
        s["search-result"] = cl["search-result"] = NJ("search for %r did not return %s" % (exp["sender"], exp["id"]))
    # the notch shows an ARRIVAL: fixtures/tests-j6.json's arrival on the same account (fixture deliver <arrival> <file> <source>)
    c.act("key", "Escape"); c.act("fixture", "deliver", exp["arrival"], J6, "eventsource"); n = c.rows("notch_list")
    s["notch"], cl["notch"] = pick(n, "id", exp["arrival_id"], "account_icon"), pick(n, "id", exp["arrival_id"], "account_colour")
    ph = Ctl(("WHOOSH_IPHONE_SOCKET_DIR",))
    if ph.live:
        pr = ph.rows("iphone_list"); s["iphone-row"], cl["iphone-row"] = pick(pr, "id", exp["id"], "mark_icon"), pick(pr, "id", exp["id"], "mark_colour")   # the phone row's fields (InboxPhoneFeature.swift:76)
    else:
        s["iphone-row"] = cl["iphone-row"] = NJ("no iPhone rig socket (WHOOSH_IPHONE_SOCKET_DIR)")
    return {"icon": s, "colour": cl, "search-found": found}

def judge(o, exp):
    return (equal("control-search-finds-row", True, {"search": o["search-found"]})
            + equal("icon", exp["icon"], o["icon"]) + equal("colour", exp["colour"], o["colour"]))

def oracle(exp):
    S = ("list-row", "sidebar-ring", "reader-meta", "search-result", "notch", "iphone-row")
    return {"icon": {s: exp["icon"] for s in S}, "colour": {s: exp["colour"] for s in S}, "search-found": True}

E = expected()
# MUTANTS
# list-provider  | the list row draws the provider default, not the account's chosen icon   | icon@list-row
# search-generic | the search result draws a generic envelope icon                         | icon@search-result
# notch-colour   | the notch paints the system accent, not the account colour              | colour@notch
# search-dead    | search returns nothing for the sender: the control goes red             | control-search-finds-row
MUTANTS = {
    "list-provider": ("provider default icon", "icon@list-row", setk(["icon", "list-row"], "provider.fastmail")),
    "search-generic": ("generic icon", "icon@search-result", setk(["icon", "search-result"], "envelope")),
    "notch-colour": ("accent colour", "colour@notch", setk(["colour", "notch"], "#007AFF")),
    "search-dead": ("dead search", "control-search-finds-row", setk(["search-found"], False)),
}
if __name__ == "__main__":
    sys.exit(run("account-mark", expected, observe, judge, oracle, MUTANTS))
