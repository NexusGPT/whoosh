#!/usr/bin/env python3
"""Inbox list check: ix-inboxlist-triage, ux-inboxlist-states (J1.1-J1.5, J1.8, J1.12).

Drives the RUNNING rig copy over the control socket (docs/CONTROL-SOCKET.md) through journeys/lib; expected
values come from docs/design/fixtures/mac-main-sample.json and docs/CONSTANTS.md, never from a built screen.
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/inboxlist-check.py'
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0 (a check that could not run is not green).
"""
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, Journey, declared_mutants       # noqa: E402
from lib.ctl import Unreadable                                       # noqa: E402
from lib.fixture import served_unread, FX, ROWS, ACCOUNT, MSG, ALL_VIEW, const, FLOW_EXAMPLE, FLOW_UNREAD, TAB_IDS_NOW, _flow_row               # noqa: E402

ID = "inboxlist"
TITLE = "Inbox list rows, tabs and counts"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "the Primary tab lists the fixture mail with its fields and counts, and archive removes exactly one row"
LIST = "mac-main.list"
# List round 5 (docs/lanes/inboxlist.md "## Round 5"): the tab row is the PINNED LABELS, built-ins first. Social and VIPs are
# not tabs; the fixture core pins one user label, Provanta (no unread, nothing waiting). The built-in tabs keep the fixture's
# numbers; the pinned user label is declared here because the design fixture has no pinned-label list.
NOT_TABS = ("Social", "VIPs")
PINNED_USER_TABS = [{"name": "Provanta", "unread": 0, "waiting": 0}]
# Delta 16: Primary and All are views, first in the strip (contracts.md "## Delta 16"); All carries the sum of every category.
def _served(t):
    return dict(t, unread=served_unread(t["id"], t["unread"]))   # the flow example's seeded rows count (lib.fixture.FLOW_UNREAD)
TABS = [_served(FX["tabs"][0]), ALL_VIEW] + [_served(t) for t in FX["tabs"][1:] if t["name"] not in NOT_TABS] + PINNED_USER_TABS


def ids(s):
    return [r["id"] for r in s.ctl.rows(LIST)]


def accepted(s, *args):
    """True when the app answers ok; ok:false is False so the check after it turns red (dry run: not judged)."""
    ctl = s.ctl
    try:
        ctl.send(*args)
        return True
    except Unreadable:
        return False


# J1.12 addresses its rows and tabs by ID, never by position (main55: "J, J, E" archived the flow's rejected row, the third row since the flow
# rows are seeded into Primary, so the failing account's row was never touched and the check read a refusal that never happened).
FAILING_ACCOUNT = FX["states"]["sync"]["failing_account"]
FAILING_ROW = next(m["id"] for m in ROWS if m["account"] == FAILING_ACCOUNT)          # msg-03: the first message of the account the core refuses
# A tab with no mail on the served fixture: the first built-in tab after the views that no message sits in. The design file files every message
# under Primary; the flow example's entry and kept rows sit in Needs Reply (lib.fixture.FLOW_UNREAD), so Needs Reply is NOT empty any more.
HOLDS_MAIL = {"primary", "all"} | {t for t, n in FLOW_UNREAD.items() if n > 0}
EMPTY_TAB = next(t for t in TAB_IDS_NOW if t not in HOLDS_MAIL)                       # updates


def goto_tab(s, tab_id, back=False, limit=8):
    """Press Tab (Shift+Tab when back) until the app says it is on `tab_id`, at most `limit` presses. No-op in a dry run. A Tab that does nothing
    ends with the tab unchanged, so the check that reads the tab after it is red; the press count is a bound, not a verdict."""
    if s.j.ctl is None:
        return
    for _ in range(limit):
        try:
            if s.ctl.uistate().get("tab") == tab_id:
                return
        except Unreadable:
            return
        s.key("Shift+Tab" if back else "Tab")


def focus_row(s, mid):
    """Click the row of message `mid` (its ax id is read from the list by the row's id): the click focuses it. No-op in a dry run or when the row is not listed."""
    if s.j.ctl is None:
        return
    try:
        row = next((r for r in s.ctl.rows(LIST) if r["id"] == mid), None)
    except Unreadable:
        return
    if row:
        s.click(row["ax"])


def drive(j):
    s = j.step("J1.1", "Rows and tabs")
    s.arrive(("click", "dock.whoosh"), "mac-main")
    s.expect("row-fields-first-row", lambda: {k: s.ctl.rows(LIST)[0][k] for k in ("id", "sender", "subject", "account", "pinned")},
             {"id": ROWS[0]["id"], "sender": ROWS[0]["sender"], "subject": ROWS[0]["subject"], "account": ROWS[0]["account"], "pinned": True})
    s.expect("subjects-all-rows", lambda: [r["subject"] for r in s.ctl.rows(LIST)], [m["subject"] for m in ROWS])
    s.expect("unread-flag-per-row", lambda: [r["unread"] for r in s.ctl.rows(LIST)], [m["unread"] for m in ROWS])
    s.expect("tab-titles", lambda: [r["title"] for r in s.ctl.rows("mac-main.tabs")], [t["name"] for t in TABS])
    s.expect("tab-unread", lambda: [r["unread"] for r in s.ctl.rows("mac-main.tabs")], [t["unread"] for t in TABS])
    s.expect("tab-waiting", lambda: [r["waiting"] for r in s.ctl.rows("mac-main.tabs")], [t["waiting"] for t in TABS])
    s.expect("sender-logo-or-initials", lambda: s.ctl.rows(LIST)[0]["initials"], ROWS[0]["initials"])
    s = j.step("J1.2", "Archive removes exactly one row")
    s.key("E")
    s.expect("one-row-fewer", lambda: len(s.ctl.rows(LIST)), len(ROWS) - 1)
    s.expect("the-right-row-left", lambda: ids(s), [m["id"] for m in ROWS[1:]])
    s.expect("undo-restores-count", lambda: (s.key("Z") or True) and len(s.ctl.rows(LIST)), len(ROWS))
    s = j.step("J1.12", "Empty tab and the failure path")
    goto_tab(s, "all")                                   # delta 16: the next tab after Primary is the All view, which lists the inbox
    # the probe's control for the empty-state check below: the tab read is part of the value, so a Tab that did nothing (still on Primary, which also lists mail) is red
    s.expect("all-view-is-not-empty", lambda: (s.ctl.uistate()["tab"], s.ctl.uistate()["empty_state"] is None, len(ids(s)) > 0), ("all", True, True))
    goto_tab(s, EMPTY_TAB)                               # the tab no message sits in on the served fixture (EMPTY_TAB, derived above), reached by Tab
    s.expect("empty-tab-has-empty-state", lambda: (s.ctl.uistate()["tab"], len(ids(s)), s.ctl.uistate()["empty_state"] is not None), (EMPTY_TAB, 0, True))
    s.expect("list-probe-control", lambda: len(s.ctl.rows("mac-main.tabs")) > 0, True)
    goto_tab(s, "primary", back=True)
    focus_row(s, FAILING_ROW)                            # by id: the click on the failing account's row focuses it; J J E hit another row on main55
    s.expect("failing-row-is-focused", lambda: s.ctl.uistate()["focus_message"], FAILING_ROW)
    order = rd_ids(s)
    s.key("E")                                           # the core refuses the archive on the failing account: ok:false from the key, the row stays
    # the list is the very list from before AND a refusal is on record: with the E deleted, or aimed at another row, one half of the pair is wrong
    s.expect("failing-row-comes-back", lambda: (ids(s) == order and FAILING_ROW in ids(s), s.ctl.uistate()["last_failure"] is not None), (True, True))
    s.expect("failure-names-the-account", lambda: s.ctl.uistate()["last_failure"]["account_name"], ACCOUNT[FAILING_ACCOUNT]["name"])


def rd_ids(s):
    """The list's ids before an act (None when unreadable: the check after it then cannot match, so it is red or not judged, never green)."""
    try:
        return ids(s)
    except Unreadable:
        return None


# MUTANTS: per check, the mutation of the app that must turn it red.
# J1.1/arrives-by-control | the Dock launch opens another surface
# J1.1/row-fields-first-row | the first row shows another message's fields
# J1.1/subjects-all-rows | subjects are truncated to empty
# J1.1/unread-flag-per-row | every row looks unread
# J1.1/tab-titles | tabs show ids instead of names
# J1.1/tab-unread | tab badges show 0
# J1.1/tab-waiting | waiting counts are dropped
# J1.1/sender-logo-or-initials | the sender mark is blank
# J1.2/one-row-fewer | archive removes two rows or none
# J1.2/the-right-row-left | archive removes a neighbour
# J1.2/undo-restores-count | undo does not bring the row back
# J1.12/all-view-is-not-empty | the All view shows the empty state over the inbox's rows, or lists nothing
# J1.12/empty-tab-has-empty-state | an empty tab shows a blank pane
# J1.12/list-probe-control | the tabs probe returns nothing
# J1.12/failing-row-is-focused | the click on a row does not move the focus to it
# J1.12/failing-row-comes-back | a refused archive removes the row anyway, or records no refusal
# J1.12/failure-names-the-account | the failure line omits the account name


# ---------------------------------------------------------------- the selftest: a stand-in socket that behaves as the contract says
# docs/CONTROL-SOCKET.md: key/click/rows/uistate. The model is written from the fixture and the contract, never from the app's reply, and its
# own mutants are the declared MUTANTS above (one per label); a second table deletes the ACT of the J1.12 cases (the `skip` hook drops one act
# of the journey) and demands the check after it goes red, because a check already true before its act is decoration.
class ModelCtl:
    instant = True
    EXTRA = [_flow_row(r) for r in FLOW_EXAMPLE if r["step"] != "rejected"]      # the entry and kept rows: Needs Reply's mail, and part of All
    ORDER = [t for t in TAB_IDS_NOW] + ["lbl-6"]

    def __init__(self, mut=(), skip=None):
        self.mut, self.skip = set(mut), skip
        self.surface, self.tab, self.focus, self.failure = "home", "primary", ROWS[0]["id"], None
        self.gone = []

    def _primary(self):
        return [r for r in ROWS if r["id"] not in self.gone]

    def _shown(self):
        rows = {"primary": self._primary(), "all": self._primary() + self.EXTRA, "needs-reply": list(self.EXTRA)}.get(self.tab, [])
        out = []
        for i, r in enumerate(rows):
            d = {"ax": "mac-main.list.row.%d" % i, "id": r["id"], "sender": r["sender"], "subject": r["subject"], "account": r["account"],
                 "unread": r["unread"], "pinned": i == 0 and self.tab == "primary", "initials": r["initials"]}
            if i == 0 and "J1.1/row-fields-first-row" in self.mut:
                d["sender"] = "Somebody Else"
            if "J1.1/subjects-all-rows" in self.mut:
                d["subject"] = ""
            if "J1.1/unread-flag-per-row" in self.mut:
                d["unread"] = True
            if i == 0 and "J1.1/sender-logo-or-initials" in self.mut:
                d["initials"] = ""
            out.append(d)
        return out

    def _drop(self, mid):
        self.gone.append(mid)

    def key(self, chord):
        if self.skip and self.skip("key", chord, self):
            return {"ok": True}
        if chord in ("Tab", "Shift+Tab"):
            i = self.ORDER.index(self.tab) + (1 if chord == "Tab" else -1)
            self.tab = self.ORDER[i % len(self.ORDER)]
            return {"ok": True}
        shown = [r["id"] for r in self._shown()]
        if chord == "E":
            if self.focus in self._focus_ids() and MSG.get(self.focus, {}).get("account") == FAILING_ACCOUNT:
                self.failure = {"account_name": "" if "J1.12/failure-names-the-account" in self.mut else ACCOUNT[FAILING_ACCOUNT]["name"],
                                "reason_words": "needs you to sign in again", "retryable": True}
                if "J1.12/failing-row-comes-back" in self.mut:
                    self._drop(self.focus)
                raise Unreadable("key ('E',): the key E does nothing on mac-main")
            ids_now = [r["id"] for r in self._primary()]
            at = ids_now.index(self.focus)
            if "J1.2/the-right-row-left" in self.mut:
                self._drop(ids_now[at + 1])
            else:
                self._drop(self.focus)
                if "J1.2/one-row-fewer" in self.mut:
                    self._drop(ids_now[at + 1])
            self.focus = next((i for i in ids_now[at + 1:] if i not in self.gone), self.focus)
            return {"ok": True}
        if chord == "Z":
            if "J1.2/undo-restores-count" not in self.mut and self.gone:
                self.gone.pop()
            return {"ok": True}
        raise Unreadable("key %s: not modelled" % chord)

    def _focus_ids(self):
        return [r["id"] for r in self._shown()]

    def click(self, ax):
        if self.skip and self.skip("click", ax, self):
            return {"ok": True}
        if ax == "dock.whoosh":
            self.surface = "mac-search" if "J1.1/arrives-by-control" in self.mut else "mac-main"
            return {"ok": True}
        for r in self._shown():
            if r["ax"] == ax:
                if "J1.12/failing-row-is-focused" not in self.mut:
                    self.focus = r["id"]
                return {"ok": True}
        raise Unreadable("click %s: nothing to click" % ax)

    def rows(self, ax):
        if ax == LIST:
            return self._shown()
        if ax == "mac-main.tabs":
            if "J1.12/list-probe-control" in self.mut:
                return []
            out = []
            for i, (tid, t) in enumerate(zip(self.ORDER, TABS)):
                out.append({"ax": "mac-main.tabs.%d" % i, "id": tid, "title": tid if "J1.1/tab-titles" in self.mut else t["name"],
                            "unread": 0 if "J1.1/tab-unread" in self.mut else t["unread"], "waiting": 0 if "J1.1/tab-waiting" in self.mut else t["waiting"]})
            return out
        raise Unreadable("rows %s: not modelled" % ax)

    def uistate(self):
        empty = not self._shown()
        es = {"title": "All done", "line": "Nothing in %s." % self.tab, "key_hint": "Tab"} if empty else None
        if "J1.12/empty-tab-has-empty-state" in self.mut and empty:
            es = None
        if "J1.12/all-view-is-not-empty" in self.mut and self.tab == "all":
            es = {"title": "All done", "line": "Nothing here.", "key_hint": "Tab"}
        return {"surface": self.surface, "tab": self.tab, "focus_message": self.focus, "last_failure": self.failure, "empty_state": es}

    def close(self):
        pass


def model_run(mutants=(), skip=None):
    """The journey on the stand-in: (labels failed, labels not judged, labels run)."""
    j = Journey(ID, ModelCtl(mutants, skip))
    drive(j)
    cs = j.checks()
    return {c.label for c in cs if c.state == "fail"}, {c.label for c in cs if c.state == "unjudged"}, {c.label for c in cs}


# Deleting one act of the journey must turn the check that reads its effect red: label -> (what is deleted, the drop rule)
ACT_DELETED = {
    "J1.12/all-view-is-not-empty": ("the Tab press that leaves Primary", lambda v, a, m: v == "key" and a == "Tab" and m.tab == "primary"),
    "J1.12/empty-tab-has-empty-state": ("the Tab press that leaves Needs Reply", lambda v, a, m: v == "key" and a == "Tab" and m.tab == "needs-reply"),
    "J1.12/failing-row-is-focused": ("the click on the failing row", lambda v, a, m: v == "click" and a.startswith("mac-main.list.row.")),
    "J1.12/failing-row-comes-back": ("the E on the focused failing row", lambda v, a, m: v == "key" and a == "E" and m.focus == FAILING_ROW),
    "J1.12/failure-names-the-account": ("the E on the focused failing row", lambda v, a, m: v == "key" and a == "E" and m.focus == FAILING_ROW),
}


def selftest(path):
    labels = declared_mutants(path)
    base_f, base_u, base_run = model_run()
    rows, missed = [], []
    for label in labels:
        f, _, _ = model_run((label,))
        rows.append((label, label in f))
        if label not in f:
            missed.append(label)
    acts, act_missed = [], []
    for label, (what, rule) in ACT_DELETED.items():
        f, _, _ = model_run((), rule)
        acts.append((label, what, label in f))
        if label not in f:
            act_missed.append(label)
    stray = sorted(base_run - set(labels))
    print("clean model: failed=%s unjudged=%s; labels without a mutant=%s" % (sorted(base_f), sorted(base_u), stray))
    print("| check | mutant (declared) | seen red |")
    print("|---|---|---|")
    for label, red in rows:
        print("| %s | the model carries this mutant | %s |" % (label, "RED on this case" if red else "NOT RED"))
    print("| check | act deleted | seen red |")
    print("|---|---|---|")
    for label, what, red in acts:
        print("| %s | %s | %s |" % (label, what, "RED on this case" if red else "NOT RED"))
    bad = len(base_f) + len(base_u) + len(missed) + len(act_missed) + len(stray)
    print("%d mutants, %d not seen red; %d acts deleted, %d not seen red; %d clean failures, %d clean not judged" % (
        len(rows), len(missed), len(acts), len(act_missed), len(base_f), len(base_u)))
    return 0 if bad == 0 else 1


def main():
    path = os.path.abspath(__file__)
    if "--selftest" in sys.argv:
        return selftest(path)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    rows = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# J") and " | " in l]
    print("MUTANTS (a check is green only after it has been seen red on its mutant)")
    print("| check | mutant |")
    print("|---|---|")
    for k, v in rows:
        print("| %s | %s |" % (k, v))
    for line in r["fail_list"]:
        sys.stderr.write("FAIL %s\n" % line)
    for line in r["no_mutant"]:
        sys.stderr.write("NO MUTANT %s\n" % line)
    for line in r["stray_mutant"]:
        sys.stderr.write("STRAY MUTANT %s\n" % line)
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    ok = r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"]
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
