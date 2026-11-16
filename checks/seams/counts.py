#!/usr/bin/env python3
"""Seam con-seams-s1/counts: one count, one query. The unread of a tab, a mailbox's count and the status
line show the SAME number on every surface, before and after an archive (contract: "unread = unread
messages in the tab across the included accounts; one query each"; C-J1.2 "counts -1").

    sh scripts/launch-one.sh <bundle> -- --fixture-core python3 checks/seams/counts.py
"""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from seamlib import fixture, Ctl, pick, nj, NJ, equal, run  # noqa: E402
sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "journeys"))
from lib.fixture import served_unread, FLOW_UNREAD  # noqa: E402   (the one definition of the served figures)


def expected():
    fx = fixture()
    tab = {t["id"]: t for t in fx["tabs"]}
    mbx = {m["name"].lower(): m.get("count") for m in fx["mailboxes"]}
    assert tab["vips"]["unread"] == mbx["vips"], "fixture: VIPs tab and VIPs mailbox disagree"
    focused = next(m for m in fx["messages"] if m["id"] == fx["focused_message"])
    return {
        # the flow example's rejected rows are seeded into Primary, unread (lib.fixture FLOW_UNREAD): the list serves the file's figure plus their count
        "primary": served_unread("primary", tab["primary"]["unread"]),
        "vips": tab["vips"]["unread"],
        "snoozed": mbx["snoozed"],
        "after": served_unread("primary", tab["primary"]["unread"]) - (1 if focused["unread"] and focused["tab"] == "primary" else 0),
        "second_query": sum(1 for m in fx["messages"] if m["unread"] and m["tab"] == "primary") + FLOW_UNREAD["primary"],
    }


def _primary(c):
    st = c.ui()
    status = c.rows("status")
    return {
        "tab-label": pick(c.rows("tabs"), "id", "primary", "unread"),
        "status-line": status if nj(status) else (status[0].get("unread", NJ("status row has no unread")) if status else NJ("status empty")),
        "uistate": st if nj(st) else st.get("counts", {}).get("primary", NJ("no counts.primary")),
        "core": c.core("counts/primary"),
    }


def observe(c, exp):
    st = c.ui()
    counts = st if nj(st) else st.get("counts", {})
    o = {"primary": _primary(c)}
    # VIPs is a mailbox on main41 (the round-5 split strip shows pinned labels, no VIPs tab); the sidebar rows carry
    # ax ids mac-main.sidebar.mailbox.<slug>
    o["vips"] = {"sidebar-mailbox": pick(c.rows("sidebar_mailboxes"), "ax", "mac-main.sidebar.mailbox.vips", "count"),
                 "uistate": counts if nj(counts) else counts.get("vips", NJ("no counts.vips")),
                 "core": c.core("counts/vips")}
    o["snoozed"] = {"sidebar-mailbox": pick(c.rows("sidebar_mailboxes"), "ax", "mac-main.sidebar.mailbox.snoozed", "count"),
                    "uistate": counts if nj(counts) else counts.get("snoozed", NJ("no counts.snoozed")),
                    "core": c.core("counts/snoozed")}
    c.act("key", "E")  # archive the focused row (fixture focused_message)
    o["after"] = _primary(c)
    return o


def judge(o, exp):
    return (equal("primary", exp["primary"], o["primary"]) + equal("vips", exp["vips"], o["vips"])
            + equal("snoozed", exp["snoozed"], o["snoozed"]) + equal("primary-after-archive", exp["after"], o["after"]))


def oracle(exp):
    return {"primary": {s: exp["primary"] for s in ("tab-label", "status-line", "uistate", "core")},
            "vips": {s: exp["vips"] for s in ("sidebar-mailbox", "uistate", "core")},
            "snoozed": {s: exp["snoozed"] for s in ("sidebar-mailbox", "uistate", "core")},
            "after": {s: exp["after"] for s in ("tab-label", "status-line", "uistate", "core")}}


def _set(path, val):
    def f(o):
        d = o
        for k in path[:-1]:
            d = d[k]
        d[path[-1]] = val(o) if callable(val) else val
        return o
    return f


E = expected()
# MUTANTS
# second-query     | the status line counts the unread rows the list shows instead of the tab query  | primary
# stale-tab        | the tab label is not refreshed by CountsChanged after an archive               | primary-after-archive
# act-deleted      | the E act is removed: every surface still shows the pre-archive count          | primary-after-archive
# mailbox-waiting  | the sidebar VIPs mailbox shows the tab's waiting instead of its unread         | vips
MUTANTS = {
    "second-query": ("status line from a second query", "primary", _set(["primary", "status-line"], E["second_query"])),
    "stale-tab": ("tab label not refreshed", "primary-after-archive", _set(["after", "tab-label"], E["primary"])),
    "act-deleted": ("archive act removed", "primary-after-archive", lambda o: dict(o, after=dict(o["primary"]))),
    "mailbox-waiting": ("VIPs mailbox reads waiting", "vips",
                        _set(["vips", "sidebar-mailbox"], next(t["waiting"] for t in fixture()["tabs"] if t["id"] == "vips"))),
}

if __name__ == "__main__":
    sys.exit(run("counts", expected, observe, judge, oracle, MUTANTS))
