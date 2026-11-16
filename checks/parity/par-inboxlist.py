#!/usr/bin/env python3
"""Inbox list parity and triage: ix-inboxlist-triage, par-inboxlist, ix-split-tabs-edit, ix-row-decided-by.

Drives the RUNNING rig copy over the control socket (docs/CONTROL-SOCKET.md) through journeys/lib.
Expected values come from docs/design/fixtures/mac-main-sample.json, never from a built screen.
Run --selftest: J1.52 (the why tag) against a stand-in app, clean and once per mutant; exit 1 when a check is red clean or a mutant is not seen red.
(The self-test runs every step that has a stand-in: J1.52 to J1.63; J1.63 is the four row marks of board message-card-r1, lane row-marks.)
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/par-inboxlist.py'
A feature the core contract cannot serve yet (no Command for it) is a check here that stays RED with its reason.
"""
import json
import os
import re
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, Journey, print_cases, print_model_cases   # noqa: E402
from lib.ctl import Unreadable                                       # noqa: E402
sys.path.insert(0, os.path.join(ROOT, "checks", "checklib"))
import resetguard as RG                                              # noqa: E402
import featurecommit as FC                                           # noqa: E402
from lib.fixture import FX, ROWS, FLOW_PRIMARY, served_unread, ACCOUNT, FAILING_ACCOUNT, PRIMARY_UNREAD, AI_VIEW, AI_ARRIVAL, write_arrival, const                     # noqa: E402

ID = "par-inboxlist"
TITLE = "Inbox list triage keys, selection, grouping, split tabs and Why"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "every triage key changes the rows and the store as the todo says; grouping and compact change the list"
LIST = "mac-main.list"


def accepted(s, *args):
    """True when the app answers ok; a refusal is False so the check turns red with the app's words."""
    from lib.ctl import Unreadable
    try:
        s.ctl.send(*args)
        return True
    except Unreadable:
        return False


def rows(s):
    return s.ctl.rows(LIST)


def ids(s):
    return [r["id"] for r in rows(s)]


def frame_if_drawn(s, ax):
    """The frame of `ax`, or None when the list has not drawn it: the rows sit in a LazyVStack (InboxListView.swift), so a row below the fold has
    no text and no tag on screen (main88: 31 rows of the needs-reply label, 9 elements walked). Any other refusal still raises."""
    try:
        return s.ctl.measure(ax)["frame"]
    except Unreadable as e:
        if "no measurement" in str(e):
            return None
        raise


def picker_drawn(s):
    """(kind, number of lines, on screen): the open picker's own row and its frame in the window. A picker that is only a pending action has no row and no frame."""
    try:
        r = (s.ctl.rows("mac-main.picker") or [{}])[0]
    except Unreadable:
        r = {}
    try:
        f = (s.ctl.measure(r.get("ax") or "mac-main.snooze.picker") or {}).get("frame") or {}
    except Unreadable:
        f = {}
    return (r.get("kind"), len(r.get("lines") or []), (f.get("w") or 0) > 100 and (f.get("h") or 0) > 60)


def st(s):
    return s.ctl.rows("mac-main.list.state")[0]


def why_words(decided_by, tab):
    """The spec's words for a decided_by value (ix-row-decided-by), written here from the todo, not read from the app."""
    head = "Placed in %s by " % tab
    if not decided_by:
        return head + "default"
    if decided_by == "Owner":
        return head + "you"
    if decided_by in ("Laya", "Jev"):
        return head + decided_by
    return head + "the rule " + (decided_by.split(":", 1)[1].strip() if decided_by.lower().startswith("rule:") else decided_by)


def core(s, path):
    return s.ctl.core(path)


def core_json(s, path):
    v = core(s, path)
    return json.loads(v) if isinstance(v, str) else v


VIEWS = ["primary", "all"]       # contract delta 16: the two views come first in the row, before any pinned label
UNREAD_CHIP = "mac-main.splits.unread"


def tab_ids(s):
    return [r["id"] for r in s.ctl.rows("mac-main.splits")]


def label_tab_ids(s):
    """The tab row's pinned labels: the row without its two views."""
    return [i for i in tab_ids(s) if i not in VIEWS]


def chip_on(s):
    return st(s)["unread_chip"]["on"]


def safe_tabs(s):
    try:
        return tab_ids(s)
    except Unreadable:
        return []


def tree_pinned(s):
    """The pinned labels in the sidebar's order, read from the label tree (labels/tree, depth first), not from the tab row."""
    out = []

    def walk(nodes):
        for n in nodes:
            if n["pinned"]:
                out.append(n["id"])
            walk(n["children"])
    walk(core_json(s, "labels/tree"))
    return out


def unpinned_owner_labels(s):
    out = []

    def walk(nodes):
        for n in nodes:
            if not n["pinned"] and n["kind"] == "owner":
                out.append(n["id"])
            walk(n["children"])
    walk(core_json(s, "labels/tree"))
    return out


def agree(s, name, left, right, what):
    """A check that two reads say the same; the detail names both."""
    def run():
        a, b = left(), right()
        return a == b, "%s: %r against %r" % (what, a, b)
    s._record(name, run)


def last_dispatch(s):
    """The command the key REACHED: a key with two meanings (⇧R: pop-out reply where a reply exists, the No Reply filter from the plain
    list) logs the binding as `command` and the meaning it took as `resolved` (ShortcutEngine.resolved); the check judges the meaning."""
    d = s.ctl.rows("mac-shortcuts.dispatch")
    return ((d[-1].get("resolved") or d[-1]["command"]), d[-1]["outcome"]) if d else None


def wait_settled():
    time.sleep(const("FOCUS_SLIDE_MS") / 1000.0 + 0.25)


def wait_focus_settled(s):
    """The focus slide takes FOCUS_SLIDE_MS; a row measured inside it is a point off its resting place. No wait on a stand-in."""
    if s.j.ctl is not None and not getattr(s.j.ctl, "instant", False):
        wait_settled()


def onscreen(s, ax, wait=0.6):
    """True when the frame probe finds the id on screen (measure refuses an id nothing draws). Waits up to `wait` seconds for it."""
    end = time.time() + wait
    while True:
        try:
            if s.ctl.measure(ax)["frame"]["w"] > 0:
                return True
        except Unreadable:
            pass
        if time.time() >= end:
            return False
        time.sleep(0.1)


def gone(s, ax, wait=0.6):
    """True when the id is no longer drawn (the strip left, the mark left)."""
    end = time.time() + wait
    while True:
        try:
            s.ctl.measure(ax)
        except Unreadable:
            return True
        if time.time() >= end:
            return False
        time.sleep(0.1)


def row_of(s, mid):
    """The model index of a message in the list (the click ids count the model's rows); -1 when it is not there."""
    try:
        return [r["id"] for r in rows(s)].index(mid)
    except (Unreadable, ValueError):
        return -1


def last_row(s):
    """The model index of the last row, -1 when the list cannot be read."""
    try:
        return len(rows(s)) - 1
    except Unreadable:
        return -1


def grab(store, key, fn):
    """Keep a read for a later check. Not a check itself: a failed read stores None."""
    try:
        store[key] = fn()
    except (Unreadable, KeyError, IndexError, TypeError, StopIteration):   # StopIteration: a `next(...)` that found no row stores None, as the line above says
        store[key] = None


def focus_id(s):
    return s.ctl.uistate()["focus_message"]


def drive(j):
    s = j.step("J1.20", "Row facts")
    s.arrive(("click", "dock.whoosh"), "mac-main")
    s.expect("initials-every-row", lambda: [r["initials"] for r in rows(s)], [m["initials"] for m in ROWS])
    s.expect("preview-every-row", lambda: [r["preview"] for r in rows(s)], [m["preview"] for m in ROWS])
    s.expect("time-is-words", lambda: all(re.match(r"^(\d\d:\d\d|[A-Z][a-z]{2}|\d{1,2} [A-Z][a-z]{2})$", r["time"]) for r in rows(s)), True)
    s.expect("account-colour-per-row", lambda: [r["account_colour"] for r in rows(s)],
             [next(a["colour"] for a in FX["accounts"] if a["id"] == m["account"]) for m in ROWS])
    s.expect("pins-on-top", lambda: [r["pinned"] for r in rows(s)][:2], [True, True])

    s.expect("hairline-under-every-row-but-focus", lambda: [r["hairline"] for r in rows(s)], [False] + [True] * (len(ROWS) - 1))
    s.expect("header-line", lambda: st(s)["header"], "%s · %d unread" % (FX["account_filter"], served_unread("primary", PRIMARY_UNREAD)))   # the file's tab figure plus the flow rows Primary holds (lib.fixture)
    s.expect("command-field-above-header", lambda: (lambda c, h: c["y"] + c["h"] <= h["y"] and c["w"] > 0 and h["w"] > 0)(
        s.ctl.measure("mac-main.list.command")["frame"], s.ctl.measure("mac-main.list.header")["frame"]), True)

    s = j.step("J1.21", "Archive keeps focus and drops Primary unread")
    before = {}
    s.expect("focus-starts-at-0", lambda: before.setdefault("u", s.ctl.uistate()["counts"]["primary"]) is not None and s.ctl.uistate()["focus_index"], 0)
    s.key("E")
    s.expect("focus-index-unchanged", lambda: s.ctl.uistate()["focus_index"], 0)
    s.expect("primary-unread-minus-one", lambda: s.ctl.uistate()["counts"]["primary"], (before.get("u") or 0) - 1)
    s.key("Z")
    s.expect("undo-puts-it-back-at-0", lambda: ids(s)[0], ROWS[0]["id"])

    s = j.step("J1.22", "Snooze with words")
    s.key("J")
    target = ROWS[1]["id"]                      # msg-02: a healthy account (msg-03 sits on the refusing acc-18)
    s.key("H")
    s.expect("snooze-picker-open", lambda: st(s)["pending"], "snooze")
    # Lead, 13 Nov (pictures of a walk on main151): H and L held the pending action and drew NO picker; the next words were typed blind. The picker's own
    # row says what it lists and its frame says it is on screen; the arrow keys move its highlighted line, not the list's focus.
    s.expect("snooze-picker-is-drawn", lambda: picker_drawn(s), ("snooze", 4, True))
    moved = {}
    grab(moved, "from", lambda: (s.ctl.rows("mac-main.picker")[0]["highlight"], s.ctl.uistate()["focus_index"]))
    s.key("Down")
    s.expect("down-moves-the-snooze-highlight-not-the-list", lambda: (s.ctl.rows("mac-main.picker")[0]["highlight"], s.ctl.uistate()["focus_index"]),
             (min((moved.get("from") or (0, 0))[0] + 1, 3), (moved.get("from") or (0, 0))[1]))
    s.type("tomorrow morning")
    s.key("Enter")
    s.expect("snoozed-row-left", lambda: target in ids(s), False)
    s.expect("snoozed-in-store", lambda: core(s, "message/%s/snoozed" % target), True)
    s.expect("snoozed-rows-served", lambda: [(r["id"], r["snooze_words"]) for r in s.ctl.rows("mac-main.snooze")], [(target, "08:00")])
    s.key("Z")
    s.expect("snooze-undone", lambda: target in ids(s), True)

    s = j.step("J1.23", "Label and pin")
    # A picker that is up owns every typed key. Probe with real keys on main154: on the first label picker of a run the letters s, u, p typed after L ran as
    # list commands (the mail was starred and marked read) because the picker's field was not in its window yet. REAL key events, sent at once after L.
    flags = lambda: (lambda r: (r.get("id"), r.get("unread"), r.get("starred")))(rows(s)[s.ctl.uistate()["focus_index"]])
    kept = {}
    grab(kept, "mail", flags)
    s.raw("keyevent", "l")
    for ch in "supp":
        s.raw("keyevent", ch)
    s.expect("letters-typed-after-l-are-the-pickers-words", lambda: poll(s, lambda: (s.ctl.rows("mac-main.picker") or [{}])[0].get("typed"), "supp"), "supp")
    s.expect("the-mail-under-the-picker-is-untouched", flags, kept.get("mail"))
    s.raw("keyevent", "escape")
    # Esc with the cursor in the picker's field closes the picker WITH its action (key trace on main156: the view closed, the action stayed and took
    # the next keys blind).
    s.expect("esc-in-the-pickers-field-closes-its-action-too", lambda: poll(s, lambda: (s.ctl.uistate()["overlay"], st(s)["pending"]), (None, None)), (None, None))
    s.key("L")
    s.expect("label-picker-is-drawn", lambda: (lambda k, n, on: (k, n > 0, on))(*picker_drawn(s)), ("label", True, True))
    s.type("Suppliers")
    s.key("Enter")
    s.expect("label-chip-shows", lambda: "Suppliers" in rows(s)[s.ctl.uistate()["focus_index"]]["chips"], True)
    s.key("Z")
    last = ROWS[-1]["id"]
    for _ in range(len(ROWS)):
        s.key("J")
    s.key("⇧P")
    s.expect("pin-goes-first-group", lambda: ids(s).index(last) < 3, True)
    s.expect("pin-follows-focus", lambda: s.ctl.uistate()["focus_message"], last)
    s.key("Z")

    s = j.step("J1.24", "Select many")
    s.key("K")
    for _ in range(len(ROWS)):
        s.key("K")
    s.key("X")
    s.key("⇧J")
    s.expect("two-selected", lambda: len(s.ctl.rows("mac-main.list.selection")), 2)
    n = len(ROWS)
    s.key("E")
    s.expect("bulk-archive-removes-selection", lambda: len(rows(s)), n - 2)
    s.key("Z")
    s.expect("bulk-undo-restores", lambda: len(rows(s)), n)

    s = j.step("J1.25", "Read, unread, mark all read")
    for _ in range(len(ROWS)):
        s.key("K")
    s.key("U")
    s.expect("u-toggles-read", lambda: rows(s)[0]["unread"], not ROWS[0]["unread"])
    s.click("mac-main.list.mark-all-read")
    s.expect("mark-all-read-clears-unread", lambda: any(r["unread"] for r in rows(s)), False)

    s = j.step("J1.26", "Grouping and compact")
    s.click("mac-main.list.group.sender")
    s.expect("group-by-sender", lambda: [r["group"] for r in rows(s)], [m["sender"] for m in ROWS])
    s.click("mac-main.list.group.domain")
    s.expect("group-by-domain", lambda: [r["group"] for r in rows(s)], [m["from"].split("@")[1] for m in ROWS])
    s.click("mac-main.list.group.none")
    s.click("mac-main.list.compact")
    s.expect("compact-on", lambda: st(s)["compact"], True)
    s.click("mac-main.list.compact")

    s = j.step("J1.27", "Round 5: the tab row is the pinned labels in the sidebar's order; Tab and Shift+Tab; unpin and pin through the label record")
    s.expect("no-split-editor-control", lambda: (accepted(s, "click", "mac-main.splits.edit"), accepted(s, "measure", "mac-main.splits.edit")), (False, False))
    agree(s, "tab-row-is-the-pinned-labels-in-sidebar-order", lambda: label_tab_ids(s), lambda: tree_pinned(s), "tab row without its views, label tree")
    agree(s, "tab-names-are-the-label-names", lambda: [r["title"] for r in s.ctl.rows("mac-main.splits") if r["kind"] == "label"],
          lambda: [p["name"] for p in core_json(s, "labels/pinned")], "label tab names, label names")
    s.expect("vips-is-not-a-tab", lambda: "vips" in tab_ids(s), False)
    s.key("Tab")
    agree(s, "tab-goes-to-next-split", lambda: s.ctl.uistate()["tab"], lambda: (VIEWS + tree_pinned(s))[1], "open tab, second tab of the row")
    s.key("Shift+Tab")
    agree(s, "shift-tab-goes-back", lambda: s.ctl.uistate()["tab"], lambda: (VIEWS + tree_pinned(s))[0], "open tab, first tab of the row")
    order, target = {}, {}
    grab(order, "ids", lambda: tab_ids(s))
    grab(target, "id", lambda: (order.get("ids") or [])[len(VIEWS) + 1])    # the second pinned label; a view has no unpin
    s.click("mac-main.splits.unpin.%s" % target.get("id"))
    s.expect("unpin-writes-the-label-record", lambda: core_json(s, "label/%s/pinned" % target.get("id")), False)
    s.expect("unpinned-tab-leaves-the-row-and-the-rest-keep-order", lambda: tab_ids(s), [i for i in (order.get("ids") or []) if i != target.get("id")])
    s.click("mac-main.splits.pin.%s" % target.get("id"))
    s.expect("pin-writes-the-label-record", lambda: core_json(s, "label/%s/pinned" % target.get("id")), True)
    s.expect("pinned-tab-comes-back-at-its-tree-place", lambda: tab_ids(s), (order.get("ids") or []))

    s = j.step("J1.28", "Why and the Superhuman keys the core cannot serve yet")
    s.click("mac-main.list.row.0.why")
    s.expect("why-opens", lambda: st(s)["why_open"], ROWS[0]["id"])
    msg = {m["id"]: m for m in ROWS}
    agree(s, "decided-by-equals-core", lambda: [(r["id"], r["decided_by"]) for r in rows(s)],
          lambda: [(r["id"], msg[r["id"]].get("decided_by")) for r in rows(s)], "decided_by, fixture")
    agree(s, "why-names-decided-by", lambda: [r["why"] for r in rows(s)],
          lambda: [why_words(msg[r["id"]].get("decided_by"), "Primary") for r in rows(s)], "why line, spec words")
    agree(s, "sender-address-from-core", lambda: [r["sender_address"] for r in rows(s)], lambda: [msg[r["id"]]["from"] for r in rows(s)], "sender address, fixture")
    s.expect("key-star", lambda: accepted(s, "key", "S"), True)
    s.expect("star-in-store", lambda: core(s, "message/%s/starred" % ROWS[0]["id"]), True)
    s.key("S")                                                  # unstar it again: the starred filter below reads exactly the rows it stars
    for label, chord, field in (("mute", "⇧M", "muted"), ("spam", "!", "spam")):
        s.click("mac-main.list.row.%d" % last_row(s))      # the last row: the rows the later steps read stay in place
        victim = {}
        s.expect("key-%s" % label, lambda c=chord, v=victim: (v.setdefault("id", s.ctl.uistate()["focus_message"]), accepted(s, "key", c))[1], True)
        s.expect("%s-in-store" % label, lambda f=field, v=victim: core(s, "message/%s/%s" % (v["id"], f)), True)
    s.click("mac-main.list.row.0")

    s = j.step("J1.29", "Round 2: the shell's list pane, core accounts, the emblem toggle, rule 11 motion")
    s.expect("list-mounted-in-pane", lambda: s.ctl.measure("mac-main.list.view").get("frame", {}).get("w", 0) > 0, True)
    s.expect("account-address-from-core", lambda: rows(s)[0]["account_address"], ACCOUNT[ROWS[0]["account"]]["address"])
    s.expect("emblem-shown-by-default", lambda: (rows(s)[0]["emblem"], st(s)["show_emblems"]), ("row", True))
    s.click("mac-main.list.emblems")
    s.expect("emblem-hidden-when-off", lambda: (rows(s)[0]["emblem"], st(s)["show_emblems"]), ("hidden", False))
    s.click("mac-main.list.emblems")
    s.expect("emblem-back-when-on", lambda: rows(s)[0]["emblem"], "row")
    settle = const("FOCUS_SLIDE_MS") / 1000.0 + 0.15       # a move after the slide time is a slide; one inside it jumps
    s.click("mac-main.list.row.0")
    time.sleep(settle)
    s.click("mac-main.list.row.1")
    s.expect("near-move-slides", lambda: (st(s)["selection_motion"], st(s)["selection_distance"], st(s)["selection_rapid"]), ("slide", 1, False))
    far = min(len(ROWS) - 1, const("SLIDE_MAX_ROWS") + 2)
    time.sleep(settle)
    s.click("mac-main.list.row.%d" % far)
    s.expect("far-move-fades", lambda: (st(s)["selection_motion"], st(s)["selection_distance"], st(s)["selection_rapid"]), ("fade", far - 1, False))
    time.sleep(settle)
    s.click("mac-main.list.row.2")
    s.click("mac-main.list.row.3")
    s.expect("second-key-inside-focus-slide-jumps", lambda: st(s)["selection_rapid"], True)

    s = j.step("J1.30", "Round 3: trash through the core, first-sync words")
    healthy = {}
    grab(healthy, "i", lambda: next(i for i, r in enumerate(rows(s)) if r["account"] != FAILING_ACCOUNT))     # the refusing account's rows stay
    s.click("mac-main.list.row.%d" % (healthy.get("i") if healthy.get("i") is not None else -1))
    t30 = {}
    grab(t30, "id", lambda: s.ctl.uistate()["focus_message"])
    target = t30.get("id")
    n = {}
    grab(n, "n", lambda: len(rows(s)))
    s.expect("trash-key-accepted", lambda: accepted(s, "key", "#"), True)
    s.expect("trash-row-leaves-at-once", lambda: (target in ids(s), len(rows(s))), (False, (n.get("n") or 0) - 1))
    s.expect("trash-in-store", lambda: core(s, "message/%s/deleted" % target), True)
    s.expect("trash-offers-undo-because-the-core-keeps-an-entry", lambda: (st(s)["undo_available"], "no undo" in (st(s)["undo_words"] or "")), (True, False))
    s.key("Z")
    s.expect("undo-brings-the-deleted-row-back", lambda: (target in ids(s), core(s, "message/%s/deleted" % target)), (True, False))
    s.expect("first-sync-not-shown-when-healthy", lambda: st(s)["first_sync"], None)

    s = j.step("J1.32", "Round 4 (runs before J1.31: the ⌘K overlay close leaves FrameProbe ids nil): the hide-emblems toggle removes the mark from every row and the text takes its width")
    def drawn(kind):
        """per visible row index: the x of the row's emblem or text, None when nothing is drawn there (measure refused)"""
        time.sleep(1.0)                          # the frames settle after a collapse, a click or a toggle
        out = []
        for mid in ids(s)[:8]:                   # keyed by message id: a row index changes owner while rows move
            try:
                out.append(s.ctl.measure("mac-main.list.row.%s.%s" % (mid, kind))["frame"]["x"])
            except Unreadable:
                out.append(None)
        return out
    s.click("mac-main.list.row.0")
    s.expect("emblem-drawn-on-every-row-when-on", lambda: all(x is not None for x in drawn("emblem")), True)
    on_text = {}
    s.expect("text-measured-when-on", lambda: all(x is not None for x in on_text.setdefault("x", drawn("text"))), True)
    s.click("mac-main.list.emblems")
    s.expect("toggle-off-in-uistate", lambda: st(s)["show_emblems"], False)
    s.expect("no-emblem-drawn-on-any-row-when-off", lambda: set(drawn("emblem")), {None})
    s.expect("text-moves-left-on-every-row-when-off", lambda: all(a is not None and b is not None and b < a
             for a, b in zip(on_text["x"], drawn("text"))), True)
    s.click("mac-main.list.emblems")
    s.expect("emblem-back-on-every-row-when-on-again", lambda: all(x is not None for x in drawn("emblem")), True)
    s.expect("text-back-where-it-was", lambda: drawn("text"), on_text.get("x"))

    s = j.step("J1.31", "Round 4: the command field is the one search, it holds no Ask")
    src = os.path.join(ROOT, "Sources", "Features", "InboxList")
    code = "\n".join(open(os.path.join(src, f), encoding="utf-8").read() for f in sorted(os.listdir(src)) if f.endswith(".swift"))
    s.expect("no-ask-code-in-the-list", lambda: [w for w in ("mac-ask", "list.ask", '"sparkles"', 'Words.text("Ask")', "askButton") if w in code], [])
    s.expect("no-ask-control-in-the-header", lambda: (accepted(s, "measure", "mac-main.list.ask"), accepted(s, "click", "mac-main.list.ask")), (False, False))
    s.expect("command-field-still-served", lambda: s.ctl.measure("mac-main.list.command")["frame"]["w"] > 0, True)
    s.expect("cmd-k-still-accepted", lambda: accepted(s, "key", "⌘K"), True)
    s.key("Escape")
    s.expect("field-names-the-search-key", lambda: st(s)["field_key"], "⌘K")

    s = j.step("J1.33", "Round 5: the +N chip folds the pinned labels past TAB_PINNED_MAX")
    base = {}
    grab(base, "ids", lambda: tab_ids(s))
    agree(s, "cap-is-the-constant", lambda: st(s)["tab_cap"], lambda: const("TAB_PINNED_MAX"), "list cap, docs/CONSTANTS.md")
    added = []

    def pin_one_past_the_cap():
        need = const("TAB_PINNED_MAX") + 1 - len(label_tab_ids(s))
        for lid in unpinned_owner_labels(s)[:max(0, need)]:
            s.click("mac-main.splits.pin.%s" % lid)
            added.append(lid)
        return len(label_tab_ids(s))                                  # the pinned labels: the row without its two views (delta 16)
    agree(s, "one-label-past-the-cap-pinned", pin_one_past_the_cap, lambda: const("TAB_PINNED_MAX") + 1, "pinned label tabs, cap plus one")
    agree(s, "plus-n-counts-the-folded", lambda: st(s)["plus_n"], lambda: len(label_tab_ids(s)) - const("TAB_PINNED_MAX"), "chip count, labels past the cap")
    agree(s, "only-the-first-cap-are-shown", lambda: [r["shown"] for r in s.ctl.rows("mac-main.splits")],
          lambda: [True] * (len(VIEWS) + const("TAB_PINNED_MAX")) + [False], "shown flags, the views and the cap, then folded")
    s.expect("chip-draws", lambda: onscreen(s, "mac-main.splits.more"), True)
    s.expect("folded-tab-draws-no-segment", lambda: onscreen(s, "mac-main.splits.%d" % (len(VIEWS) + const("TAB_PINNED_MAX")), wait=0.3), False)
    s.click("mac-main.splits.more.%s" % (lambda t: t[-1] if t else "x")(safe_tabs(s)))
    agree(s, "chip-menu-opens-the-folded-tab", lambda: s.ctl.uistate()["tab"], lambda: tab_ids(s)[-1], "open tab, last pinned label")
    s.key("Tab")
    agree(s, "tab-wraps-from-the-last-to-the-first", lambda: s.ctl.uistate()["tab"], lambda: tab_ids(s)[0], "open tab, first pinned label")
    for lid in added:
        s.click("mac-main.splits.unpin.%s" % lid)
    s.expect("unpinning-the-extras-removes-the-chip", lambda: (st(s)["plus_n"], tab_ids(s)), (0, base.get("ids")))
    s.expect("chip-leaves", lambda: gone(s, "mac-main.splits.more"), True)

    s = j.step("J1.34", "Round 5: the hover action strip, five actions with their keys, never during a key run")
    ACTIONS = [("archive", "Archive", "E"), ("delete", "Delete", "⇧#"), ("junk", "Junk", "⇧!"), ("snooze", "Snooze", "H"), ("label", "Label", "L")]
    s.click("mac-main.list.row.%d" % row_of(s, "msg-05"))
    wait_settled()
    s.expect("strip-draws-all-five-on-the-settled-row", lambda: [onscreen(s, "mac-main.list.row.%s.act.%s" % (focus_id(s), a[0])) for a in ACTIONS], [True] * 5)
    s.expect("strip-has-the-keys-in-the-tooltips", lambda: [(r["id"], r["key"], r["help"]) for r in st(s)["strip"]],
             [(a[0], a[2], "%s · %s" % (a[1], a[2]) + (" · Remove · Y" if a[0] == "label" else "")) for a in ACTIONS])
    s.key("J")
    s.expect("strip-waits-during-a-key-run", lambda: st(s)["strip_settled"], False)
    wait_settled()
    s.expect("strip-returns-after-the-focus-settles", lambda: (st(s)["strip_settled"], onscreen(s, "mac-main.list.row.%s.act.archive" % focus_id(s))), (True, True))
    s.expect("strip-left-the-row-the-focus-left", lambda: gone(s, "mac-main.list.row.msg-05.act.archive"), True)
    s.click("mac-main.list.row.%d.act.archive" % row_of(s, "msg-05"))
    s.expect("strip-archive-runs-the-command", lambda: ("msg-05" in ids(s), core(s, "message/msg-05/archived")), (False, True))
    s.key("Z")
    s.expect("strip-archive-undone", lambda: "msg-05" in ids(s), True)
    s.click("mac-main.list.row.%d.act.delete" % row_of(s, "msg-09"))
    s.expect("strip-delete-runs-the-command", lambda: ("msg-09" in ids(s), core(s, "message/msg-09/deleted")), (False, True))
    s.click("mac-main.list.row.%d.act.junk" % row_of(s, "msg-07"))
    s.expect("strip-junk-runs-the-command", lambda: ("msg-07" in ids(s), core(s, "message/msg-07/spam")), (False, True))
    s.click("mac-main.list.row.%d.act.snooze" % row_of(s, "msg-02"))
    s.expect("strip-snooze-opens-its-picker", lambda: st(s)["pending"], "snooze")
    s.key("Escape")
    s.click("mac-main.list.row.%d.act.label" % row_of(s, "msg-02"))
    s.expect("strip-label-opens-its-picker", lambda: st(s)["pending"], "label")
    s.key("Escape")
    s.expect("pickers-closed", lambda: st(s)["pending"], None)

    s = j.step("J1.35", "Round 5: selection, X, Shift+J, Shift+K, Esc, Cmd+A from here, Cmd+Shift+A all, and the batch bar")
    s.click("mac-main.list.row.0")
    s.key("X")
    agree(s, "x-selects-the-focused-thread", lambda: [r["id"] for r in s.ctl.rows("mac-main.list.selection")], lambda: [ids(s)[0]], "selection, first row")
    s.expect("check-mark-in-the-row", lambda: onscreen(s, "mac-main.list.row.%s.check" % ids(s)[0]), True)
    s.key("⇧J")
    s.key("⇧J")
    agree(s, "shift-j-extends-down", lambda: [r["id"] for r in s.ctl.rows("mac-main.list.selection")], lambda: ids(s)[:3], "selection, first three rows")
    s.expect("bar-names-the-count-and-the-actions", lambda: st(s)["selection_bar"], {"count": 3, "actions": [a[0] for a in ACTIONS]})
    s.expect("bar-draws", lambda: onscreen(s, "mac-main.list.selectionbar.archive"), True)
    s.key("Escape")
    s.expect("esc-clears-the-selection", lambda: (len(s.ctl.rows("mac-main.list.selection")), st(s)["selection_bar"]), (0, None))
    s.expect("bar-leaves", lambda: gone(s, "mac-main.list.selectionbar.archive"), True)
    s.click("mac-main.list.row.5")
    s.key("⇧K")
    agree(s, "shift-k-extends-up", lambda: sorted(r["id"] for r in s.ctl.rows("mac-main.list.selection")), lambda: sorted(ids(s)[4:6]), "selection, rows 4 and 5")
    s.key("Escape")
    s.click("mac-main.list.row.3")
    s.key("⌘A")
    agree(s, "cmd-a-selects-from-the-focused-row-down", lambda: [r["id"] for r in s.ctl.rows("mac-main.list.selection")], lambda: ids(s)[3:], "selection, rows from 3")
    s.key("Escape")
    s.key("⇧⌘A")
    agree(s, "cmd-shift-a-selects-all", lambda: [r["id"] for r in s.ctl.rows("mac-main.list.selection")], lambda: ids(s), "selection, every row")
    s.click("mac-main.list.selectionbar.clear")
    s.expect("bar-cross-clears", lambda: len(s.ctl.rows("mac-main.list.selection")), 0)
    s.click("mac-main.list.row.%d" % row_of(s, "msg-04"))      # three healthy rows: msg-03 sits on the refusing account
    s.key("X")
    s.key("⇧J")
    s.key("⇧J")
    n = {}
    grab(n, "n", lambda: len(ids(s)))
    s.click("mac-main.list.selectionbar.archive")
    s.expect("batch-archive-takes-all-three", lambda: len(ids(s)), (n.get("n") or 0) - 3)
    s.key("Z")
    s.expect("one-undo-brings-all-three-back", lambda: len(ids(s)), n.get("n"))
    pair = {}
    s.click("mac-main.list.row.%d" % row_of(s, "msg-10"))
    s.key("X")
    s.key("⇧J")
    grab(pair, "ids", lambda: [r["id"] for r in s.ctl.rows("mac-main.list.selection")])
    s.click("mac-main.list.selectionbar.delete")
    s.expect("batch-delete-takes-both", lambda: [core(s, "message/%s/deleted" % i) for i in (pair.get("ids") or [])], [True, True])
    s.click("mac-main.list.row.%d" % row_of(s, "msg-12"))
    s.key("X")
    s.key("⇧J")
    pair2 = {}
    grab(pair2, "ids", lambda: [r["id"] for r in s.ctl.rows("mac-main.list.selection")])
    s.click("mac-main.list.selectionbar.junk")
    s.expect("batch-junk-takes-both", lambda: [core(s, "message/%s/spam" % i) for i in (pair2.get("ids") or [])], [True, True])

    s = j.step("J1.36", "Round 5: the filters, Shift+U unread, Shift+S starred, Shift+I important, Shift+R no reply, the chip")
    s.click("mac-main.list.row.%d" % row_of(s, "msg-06"))
    s.key("U")
    s.click("mac-main.list.row.%d" % row_of(s, "msg-08"))
    s.key("U")
    s.key("⇧U")
    s.expect("shift-u-keeps-the-unread-rows", lambda: ids(s), ["msg-06", "msg-08"])
    # round 10: the Unread chip is the one at the end of the tab row; it reads as on while the filter is
    s.expect("chip-names-the-filter", lambda: (st(s)["filter"], st(s)["filter_words"], onscreen(s, UNREAD_CHIP), chip_on(s)), ("unread", "Unread", True, True))
    s.key("J")
    s.expect("j-steps-over-the-hidden-rows", lambda: focus_id(s), "msg-08")
    s.key("K")
    s.expect("k-steps-back-over-them", lambda: focus_id(s), "msg-06")
    s.key("Escape")
    s.expect("esc-clears-the-filter", lambda: (st(s)["filter"], len(ids(s)) > 2), (None, True))
    s.expect("chip-leaves-with-the-filter", lambda: (chip_on(s), onscreen(s, UNREAD_CHIP)), (False, True))    # the chip stays in the row, off
    s.key("⇧U")
    s.key("⇧U")
    s.expect("same-key-again-clears-it", lambda: st(s)["filter"], None)
    s.key("⇧U")
    s.click(UNREAD_CHIP)
    s.expect("chip-click-clears-it", lambda: st(s)["filter"], None)
    s.click("mac-main.list.row.%d" % row_of(s, "msg-06"))
    s.key("S")
    s.click("mac-main.list.row.%d" % row_of(s, "msg-08"))
    s.key("S")
    s.key("⇧S")
    s.expect("shift-s-keeps-the-starred-rows", lambda: (ids(s), [r["starred"] for r in rows(s)]), (["msg-06", "msg-08"], [True, True]))
    s.key("Escape")
    before = {}
    grab(before, "ids", lambda: ids(s))
    s.key("⇧R")
    # round 6: the core decides (delta 10); the fixture files "no reply" as the Needs Reply rows, "important" as Needs Reply or VIPs
    s.expect("shift-r-keeps-the-conversations-nobody-answered", lambda: ids(s),
             [i for i in (before.get("ids") or []) if next(m for m in ROWS if m["id"] == i).get("tab") == "needs-reply"])
    s.key("Escape")
    s.expect("shift-i-filters-through-the-core", lambda: (accepted(s, "key", "⇧I"), st(s)["filter"], ids(s)),
             (True, "important", [i for i in (before.get("ids") or []) if next(m for m in ROWS if m["id"] == i).get("tab") in ("needs-reply", "vips")]))
    s.key("Escape")
    s.click("mac-main.list.row.%d" % row_of(s, "msg-06"))
    s.key("S")
    s.click("mac-main.list.row.%d" % row_of(s, "msg-08"))
    s.key("S")
    s.key("⇧S")
    s.expect("a-filter-that-keeps-nothing-says-so", lambda: (st(s)["filter_empty"], len(ids(s))), (True, 0))
    s.key("Escape")

    s = j.step("J1.37", "Round 5: the ids the shortcuts table names reach this list")
    for chord, command in (("X", "select-conversation"), ("⇧⌘A", "select-all"), ("⌘A", "select-from-here"), ("S", "star"),
                           ("⇧U", "filter-unread"), ("⇧R", "filter-no-reply"), ("⇧P", "pin")):
        s.click("mac-main.list.row.%d" % row_of(s, "msg-04"))
        s.key(chord)
        s.expect("id-%s-reached-the-list" % command, lambda: last_dispatch(s), (command, "ran"))
        s.key("Escape")
        s.key("Escape")
    s.click("mac-main.list.row.%d" % row_of(s, "msg-05"))
    s.key("⇧!")
    s.expect("id-junk-reached-the-list", lambda: (last_dispatch(s), core(s, "message/msg-05/spam")), (("junk", "ran"), True))
    s.click("mac-main.list.row.%d" % row_of(s, "msg-02"))
    s.key("⇧#")
    s.expect("id-delete-reached-the-list", lambda: (last_dispatch(s), core(s, "message/msg-02/deleted")), (("delete", "ran"), True))
    s.click("mac-main.list.row.%d" % row_of(s, "msg-06"))
    s.key("⇧M")
    s.expect("id-mute-reached-the-list", lambda: (last_dispatch(s), core(s, "message/msg-06/muted")), (("mute", "ran"), True))
    s.key("⇧I")
    s.expect("id-filter-important-reached-the-list", lambda: last_dispatch(s), ("filter-important", "ran"))
    s.key("Escape")

    s = j.step("J1.38", "Round 5: every control of the list clicked once")
    s.click("mac-main.list.command")
    s.expect("command-field-opens-the-search", lambda: st(s)["search_active"], True)
    s.key("Escape")
    for i in range(3):
        s.click("mac-main.splits.%d" % i)
        agree(s, "tab-segment-%d-opens-its-tab" % i, lambda: s.ctl.uistate()["tab"], lambda i=i: tab_ids(s)[i], "open tab, tab %d of the row" % i)
    s.click("mac-main.splits.0")
    src = os.path.join(ROOT, "Sources", "Features", "InboxList")
    doors = sorted(set(re.findall(r'"(mac-main\.[a-z.\-]+)', open(os.path.join(src, "ListClicks.swift"), encoding="utf-8").read())))
    me = open(os.path.abspath(__file__), encoding="utf-8").read()
    s.expect("every-click-door-is-clicked-by-this-check", lambda: [d for d in doors if me.count(d.rstrip(".")) < 1 and ".perf." not in d], [])

    s = j.step("J1.39", "Round 6 hook: the label keys Y, [, ], Shift+Y go through the labels lane's doors (removeLabel / removeAllLabels, delta 10); V mounts the move picker over the list")
    for _ in range(3):
        s.key("Escape")
    tgt = {}
    grab(tgt, "id", lambda: next(r["id"] for r in rows(s)[1:-1] if not r["chips"]))    # a row with a neighbour on each side
    t = lambda: tgt.get("id") or "none"
    for chord, command, step in (("Y", "remove-label", 0), ("[", "remove-label-next", 1), ("]", "remove-label-previous", -1), ("⇧Y", "remove-all-labels", 0)):
        s.click("mac-main.list.row.%d" % row_of(s, t()))
        s.key("L")
        s.type("Suppliers")
        s.key("Enter")
        s.expect("chip-is-there-before-%s" % command, lambda: "Suppliers" in rows(s)[row_of(s, t())]["chips"], True)
        here = {}
        grab(here, "at", lambda: row_of(s, t()))
        grab(here, "ids", lambda: ids(s))
        s.click("mac-main.list.row.%d" % row_of(s, t()))
        s.key(chord)
        s.expect("key-%s-reaches-the-list" % command, lambda: last_dispatch(s), (command, "ran"))
        s.expect("key-%s-removes-the-chip-and-steps" % command, lambda st_=step: (
            "Suppliers" in rows(s)[row_of(s, t())]["chips"],
            focus_id(s) == ((here.get("ids") or [None])[(here.get("at") or 0) + st_] if here.get("ids") else None)), (False, True))
        s.expect("key-%s-chip-gone-in-the-store" % command, lambda: "Suppliers" in str(core(s, "label_assignment/%s" % t())), False)
    s.click("mac-main.list.row.%d" % row_of(s, t()))
    s.expect("key-y-on-a-thread-without-a-label-is-refused", lambda: accepted(s, "key", "Y"), False)
    s.click("mac-main.list.row.%d" % row_of(s, t()))
    s.key("V")
    s.expect("key-v-opens-the-move-picker", lambda: s.ctl.uistate()["overlay"], "move")
    s.expect("move-picker-mounted-over-the-list", lambda: onscreen(s, "mac-main.list.overlay"), True)
    s.key("Escape")
    s.expect("esc-closes-the-picker", lambda: (s.ctl.uistate()["overlay"], gone(s, "mac-main.list.overlay")), (None, True))
    dest = {}
    s.click("mac-main.list.row.%d" % row_of(s, t()))
    s.key("V")
    grab(dest, "label", lambda: s.ctl.rows("mac-labels.move.targets")[0]["id"])
    grab(dest, "name", lambda: s.ctl.rows("mac-labels.move.targets")[0]["name"])
    s.click("mac-labels.move.%s" % (dest.get("label") or "none"))
    s.expect("move-takes-the-thread-off-the-list", lambda: t() in ids(s), False)
    s.expect("move-lands-labelled-and-archived", lambda: ((dest.get("name") or "\0") in str(core(s, "label_assignment/%s" % t())), core(s, "message/%s/archived" % t())), (True, True))
    s.key("Z")
    s.expect("one-undo-brings-the-move-back", lambda: (t() in ids(s), core(s, "message/%s/archived" % t())), (True, False))

    # -- Round 6 (lead, 2026-11-09): filters through the core (delta 10), mark not done, the list states.
    s = j.step("J1.40", "Round 6: the four filters read the core (InboxQuery.filters); the chip shows and clears")
    for _ in range(3):
        s.key("Escape")
    s.raw("fixture", "reset")
    tabs = safe_tabs(s)
    seen = {}
    if "needs-reply" in tabs:
        for _ in range(tabs.index("needs-reply")):
            s.key("Tab")
        grab(seen, "needs", lambda: ids(s))
        for _ in range(tabs.index("needs-reply")):
            s.key("Shift+Tab")
    grab(seen, "all", lambda: ids(s))
    s.key("⇧R")
    s.expect("no-reply-is-the-filed-needs-reply-rows", lambda: (st(s)["filter"], ids(s)),
             ("no-reply", [i for i in (seen.get("all") or []) if i in set((seen.get("needs") or []))]))
    s.expect("filter-chip-on-screen", lambda: onscreen(s, "mac-main.list.filter"), True)
    s.key("⇧R")
    s.expect("same-key-clears-the-filter", lambda: (st(s)["filter"], ids(s)), (None, seen.get("all")))
    s.key("⇧I")
    s.expect("important-filters-through-the-core", lambda: (st(s)["filter"], set((seen.get("needs") or [])) & set((seen.get("all") or [])) <= set(ids(s)) and set(ids(s)) <= set((seen.get("all") or []))),
             ("important", True))
    s.expect("important-no-longer-refused", lambda: st(s)["last_failure"], None)
    s.key("Escape")
    s.expect("esc-clears-filter-and-chip", lambda: (st(s)["filter"], gone(s, "mac-main.list.filter"), ids(s)), (None, True, seen.get("all")))
    s.key("⇧S")
    s.expect("starred-is-the-core-starred-set", lambda: ids(s), [i for i in (seen.get("all") or []) if core(s, "message/%s/starred" % i) is True])
    s.key("Escape")

    s = j.step("J1.41", "Mark not done (Shift+E): brings back the archived thread on the Archive view; elsewhere it passes on")
    for _ in range(3):
        s.key("Escape")
    s.click("mac-main.list.row.0")
    gone_id = {}
    grab(gone_id, "id", lambda: focus_id(s))
    g = lambda: gone_id.get("id") or "none"
    s.key("E")
    s.expect("archived-row-left", lambda: g() in ids(s), False)
    s.click("mac-main.sidebar.mailbox.archive")
    s.click("mac-main.list.row.%d" % max(0, row_of(s, g())))
    s.key("⇧E")
    s.expect("shift-e-on-the-archive-views-focused-thread-brings-it-back", lambda: (g() in ids(s), core(s, "message/%s/archived" % g())), (False, False))
    s.click("mac-main.splits.0")
    s.expect("the-thread-is-back-in-the-inbox", lambda: g() in ids(s), True)
    s.click("mac-main.list.row.0")
    s.expect("shift-e-on-an-inbox-thread-passes-on", lambda: (accepted(s, "key", "⇧E"), "Nothing archived" in (st(s)["last_failure"] or "")), (False, True))
    s.click("mac-main.list.row.0")
    again = {}
    grab(again, "id", lambda: focus_id(s))
    s.key("E")
    s.key("Z")
    s.expect("shift-e-after-undo-passes-on", lambda: (accepted(s, "key", "⇧E"), ids(s).count(again.get("id")), st(s)["undo_key"] != "Back in the list"), (False, 1, True))

    s = j.step("J1.42", "Round 6: the end of the list and the loading state")
    s.expect("end-of-list-names-tab-and-count", lambda: (st(s)["end_of_list"] or "").startswith("End of") and ("%d conversations" % len(ids(s))) in st(s)["end_of_list"], True)
    s.expect("end-line-drawn", lambda: onscreen(s, "mac-main.list.end", wait=1.5) or (s.click("mac-main.list.row.%d" % last_row(s)) or onscreen(s, "mac-main.list.end", wait=1.5)), True)   # no key is bound to scroll to the end (main66: "the key ⌘↓ does nothing"): the focus click scrolls the last row in
    s.expect("not-loading-with-rows", lambda: st(s)["loading"], False)

    s = j.step("J1.43", "Round 6: a filter that keeps nothing keeps its chip and says so")
    s.click("mac-main.list.mark-all-read")
    s.key("⇧U")
    s.expect("filtered-empty-words", lambda: (st(s)["filter_empty"], onscreen(s, "mac-main.list.empty")), (True, True))
    s.expect("filtered-empty-keeps-the-chip", lambda: (onscreen(s, UNREAD_CHIP), chip_on(s)), (True, True))
    s.key("Escape")

    # -- Round 7 (lead, 2026-11-09): a mailbox from the sidebar, the tab click back to the inbox, words that follow the language.
    s = j.step("J1.44", "Round 7: a mailbox shows its name and count; a tab click returns to the inbox rows")
    for _ in range(3):
        s.key("Escape")
    box = {}
    grab(box, "inbox", lambda: ids(s))
    grab(box, "tab", lambda: s.ctl.uistate()["tab"])
    s.click("mac-main.sidebar.mailbox.sent")
    s.expect("sidebar-sent-shows-the-sent-mailbox", lambda: st(s)["mailbox"], "sent")
    s.expect("header-names-the-mailbox-and-its-count", lambda: st(s)["header_line"] == "Sent · %d conversations" % len(ids(s)), True)
    s.expect("no-tab-is-current-while-a-mailbox-shows", lambda: [r["on"] for r in s.ctl.rows("mac-main.splits")].count(True), 0)
    s.expect("end-line-names-the-mailbox", lambda: (st(s)["end_of_list"] or "").startswith("End of Sent") if ids(s) else True, True)
    s.click("mac-main.splits.0")
    s.expect("tab-click-returns-to-the-inbox", lambda: (st(s)["mailbox"], ids(s) == (box.get("inbox") if box.get("tab") == (safe_tabs(s) or [None])[0] else ids(s))), (None, True))
    s.expect("tab-click-makes-the-tab-current", lambda: [r["on"] for r in s.ctl.rows("mac-main.splits")][0], True)
    s.expect("header-back-to-the-unread-line", lambda: "unread" in st(s)["header_line"], True)
    s.click("mac-main.sidebar.mailbox.archive")
    s.click("mac-main.splits.%d" % (safe_tabs(s).index(box.get("tab")) if box.get("tab") in safe_tabs(s) else 0))
    s.expect("current-tab-clicked-again-leaves-the-mailbox", lambda: (st(s)["mailbox"], ids(s)), (None, box.get("inbox")))

    s = j.step("J1.45", "Round 7: the undo words and the tab names follow a language switch made after the action")
    s.click("mac-main.list.row.0")
    s.key("⇧P")
    s.expect("undo-words-kept-as-a-key", lambda: st(s)["undo_key"] in ("Pinned", "Unpinned"), True)
    s.raw("click", "mac-settings-general.appearance.language.fr")
    s.expect("undo-words-redrawn-in-french", lambda: st(s)["undo_words"] in ("Épinglés", "Désépinglé", "Désépinglés") or st(s)["undo_words"] != st(s)["undo_key"], True)
    s.expect("built-in-tab-names-in-french", lambda: "Primary" in [r["title"] for r in s.ctl.rows("mac-main.splits")], False)
    s.raw("click", "mac-settings-general.appearance.language.en")
    s.expect("undo-words-back-in-english", lambda: st(s)["undo_words"] == st(s)["undo_key"], True)
    s.key("Z")

    s = j.step("J1.46", "Round 8: the why line names the tab as the tab row draws it, in the app's language")
    for _ in range(3):
        s.key("Escape")
    s.click("mac-main.splits.0")
    s.raw("click", "mac-settings-general.appearance.language.fr")
    cur = lambda: next(r["title"] for r in s.ctl.rows("mac-main.splits") if r["on"])
    s.expect("why-line-uses-the-tab-rows-name", lambda: all(cur() in r["why"] for r in rows(s)), True)
    s.expect("why-line-has-no-stored-english-name", lambda: any("Primary" in r["why"] for r in rows(s)) and cur() != "Primary", False)
    s.raw("click", "mac-settings-general.appearance.language.en")
    s.expect("why-line-back-in-english", lambda: all(cur() in r["why"] for r in rows(s)), True)

    s = j.step("J1.47", "Round 8: the list's foot strip is FOOT_STRIP_PX from its hairline to the window foot, with or without the capsule")
    for _ in range(3):
        s.key("Escape")
    strip = lambda: round(s.ctl.measure("mac-main.list.foot")["frame"]["h"], 1)
    s.expect("foot-strip-is-the-lock", lambda: strip(), float(const("FOOT_STRIP_PX")))
    s.click("mac-main.list.row.0")
    s.key("⇧P")
    s.expect("foot-strip-unchanged-under-the-capsule", lambda: (st(s)["undo_words"] is not None, strip()), (True, float(const("FOOT_STRIP_PX"))))
    s.key("Z")

    s = j.step("J1.48", "Round 8 (owner 08:58, SPEC A79): E collapses the leaving line and the rows below glide up; a second key jumps")
    # Instrument (main41 finding): `shot` takes ~280 ms, longer than ROW_COLLAPSE_MS, and the rows carry no frame under their
    # index id, so the samples are `measure` reads of the next row's text id (`mac-main.list.row.<id>.text`), probe warmed first.
    for _ in range(3):
        s.key("Escape")
    s.click("mac-main.list.row.0")
    time.sleep(0.3)
    fr = {}
    ty = lambda mid: round(s.ctl.measure("mac-main.list.row.%s.text" % mid)["frame"]["y"], 1)
    grab(fr, "ids", lambda: ids(s)[:2])
    first, nxt = ((fr.get("ids") or [None, None]) + [None, None])[:2]
    grab(fr, "warm", lambda: ty(first))          # the first request mounts the frame readers; its answer may be empty
    time.sleep(0.2)
    grab(fr, "y_first", lambda: ty(first))
    grab(fr, "y_next", lambda: ty(nxt))
    time.sleep(0.2)
    def sample():
        s.ctl.send("key", "E")
        out, t0 = [], time.time()
        while time.time() - t0 < 0.35:
            out.append(ty(nxt))
        return out
    grab(fr, "samples", sample)
    time.sleep(0.3)
    grab(fr, "y_settled", lambda: ty(nxt))
    s.expect("the-next-row-moved-up-into-place", lambda: (ids(s)[0] == nxt, abs((fr.get("y_settled") or -99) - (fr.get("y_first") or 99)) <= 3), (True, True))   # text bands of two rows sit up to 2 pt apart (main41: 150 vs 152)
    s.expect("mid-collapse-the-next-row-is-between-its-old-and-new-place",
             lambda: (fr.get("y_first") is not None and fr.get("y_next") is not None
                     and any(fr["y_first"] + 0.5 < v < fr["y_next"] - 0.5 for v in (fr.get("samples") or []) if isinstance(v, (int, float)))), True)
    s.expect("the-removal-glided", lambda: st(s)["collapse_glided"], True)
    s.key("Z")
    time.sleep(0.3)
    def rapid():
        s.ctl.send("key", "E"); s.ctl.send("key", "E")          # back to back, no harness work between them
        r = st(s)
        return r["collapse_glided"], r["collapse_gap_ms"]
    grab(fr, "rapid", rapid)
    # owner 2026-11-11 ("also when holding E to mark many as done"; pick L2): a held E animates EVERY mail, whatever the gap the app measured.
    # (Before: a second key inside FOCUS_SLIDE_MS dropped the row with no motion, which is why a held E read as rows vanishing.)
    s.expect("a-held-second-key-still-glides", lambda: (fr.get("rapid") or (None, None))[0], True)
    s.key("Z")
    s.key("Z")

    s = j.step("J1.49", "Round 9: a saved smart folder from the sidebar names itself in the header with its count; a tab click leaves it")
    for _ in range(3):
        s.key("Escape")
    sf = {}
    grab(sf, "row", lambda: s.ctl.rows("mac-main.sidebar.smart-folders")[0])
    s.click((sf.get("row") or {}).get("ax", "mac-main.sidebar.smart.none"))
    s.expect("smart-folder-header-names-it-with-its-count", lambda: (st(s)["smart_folder"] == (sf.get("row") or {}).get("ax", "").split(".")[-1],
             st(s)["header_line"] == "%s · %d conversations" % ((sf.get("row") or {}).get("name"), len(ids(s)))), (True, True))
    s.click("mac-main.splits.0")
    s.expect("tab-click-leaves-the-smart-folder", lambda: (st(s)["smart_folder"], "unread" in st(s)["header_line"]), (None, True))

    s = j.step("J1.50", "J1.12 (tests r4 verdict): with Primary emptied, the zero state names the next snooze return from the core")
    for _ in range(3):
        s.key("Escape")
    s.click("mac-main.splits.0")
    s.click("mac-main.list.row.0")
    sn = {}
    grab(sn, "id", lambda: focus_id(s))
    s.key("H")
    s.type("tomorrow morning")
    s.key("Enter")
    # a person archives one row at a time: focus it, select it (X), press E. Select-all is not used: the batch would
    # carry the failing account's row (acc-18 refuses every command), the core refuses the whole batch and E does nothing.
    s.raw("fixture", "reauth", FAILING_ACCOUNT)
    left = {}
    for _ in range(len(ROWS) + 2):
        grab(left, "n", lambda: len(ids(s)))
        if not left.get("n"):
            break
        s.key("X")
        s.key("E")
    def words():
        import datetime
        ms = core(s, "snooze/%s/return_ms" % sn.get("id"))
        return datetime.datetime.fromtimestamp(ms / 1000).strftime("%H:%M")
    s.expect("empty-primary-names-the-next-snooze-return", lambda: (len(ids(s)), words() in ((st(s)["empty_state"] or {}).get("line") or "")), (0, True))
    s.key("Z")
    s.key("Z")


    # -- Round 10 (lead, 2026-11-09, owner card 16:30, docs/PRD.md "A91 shape"): Primary and All are views, Unread is a chip.
    s = j.step("J1.51", "Round 10: the tab row reads inbox/tabs (Primary, All, then the pinned labels); a view has no label control; the Unread chip toggles the filter on any tab")
    for _ in range(3):
        s.key("Escape")
    RG.step_reset(s)      # a verified reset (checklib: the digest must equal the launch digest), not the bare verb
    s.click("mac-main.splits.0")
    strip = lambda: s.ctl.rows("mac-main.splits")
    tabs_state = lambda: core_json(s, "inbox/tabs")
    agree(s, "tab-order-is-views-first", lambda: tab_ids(s), lambda: VIEWS + tree_pinned(s), "tab row, the views then the label tree")
    agree(s, "tab-row-reads-inbox-tabs", lambda: [(r["id"], r["kind"]) for r in strip()], lambda: [(t["id"], t["kind"]) for t in tabs_state()],
          "tab row, inbox/tabs (id and kind)")
    s.expect("views-are-named-primary-and-all", lambda: [r["title"] for r in strip()][:2], ["Primary", "All"])
    s.expect("a-view-tab-offers-no-label-menu", lambda: [r["menu"] for r in strip() if r["kind"] == "view"], [[], []])
    s.expect("a-label-tab-keeps-its-unpin-menu", lambda: sorted(set(tuple(r["menu"]) for r in strip() if r["kind"] == "label")), [("unpin",)])
    src = os.path.join(ROOT, "Sources", "Features", "InboxList")
    bar = open(os.path.join(src, "SplitTabsBar.swift"), encoding="utf-8").read()
    chip_src = open(os.path.join(src, "UnreadCount.swift"), encoding="utf-8").read()
    s.expect("the-tab-row-has-no-drag", lambda: re.findall(r"\.(?:draggable|onDrag|onDrop|dropDestination)\(", bar + chip_src), [])
    # round 13 (list r8, board unread-filter-r1 F2): the Unread control left the tab row; its place is the count line (J1.54)
    s.expect("the-unread-chip-shows-its-key", lambda: st(s)["unread_chip"]["key"], "\u21e7U")
    # counts: the two views read counts().tabs; All holds every category, so it is never below Primary
    agree(s, "view-counts-come-from-the-core", lambda: [(r["unread"], r["waiting"]) for r in strip()[:2]],
          lambda: [(core_json(s, "view/%s/unread" % v), core_json(s, "view/%s/waiting" % v)) for v in VIEWS], "tab row counts, view/<id> counts")
    s.expect("all-is-at-least-primary", lambda: (lambda r: r[1]["unread"] >= r[0]["unread"] and r[1]["waiting"] >= r[0]["waiting"])(strip()), True)
    # the chip on a view, on the other view and on a label tab: the rows change, the same read as the key's
    labels = [i for i in safe_tabs(s) if i not in VIEWS]
    for tab in VIEWS + (labels[:1] or ["a-label-tab"]):
        at = safe_tabs(s).index(tab) if tab in safe_tabs(s) else 0
        s.click("mac-main.splits.%d" % at)
        before = {}
        grab(before, "rows", lambda: [(r["id"], r["unread"]) for r in rows(s)])
        s.click(UNREAD_CHIP)
        s.expect("chip-turns-the-filter-on-%s" % ("a-label-tab" if tab not in VIEWS else tab), lambda: (st(s)["filter"], chip_on(s), s.ctl.uistate()["tab"]), ("unread", True, tab))
        s.expect("chip-keeps-the-unread-rows-%s" % ("a-label-tab" if tab not in VIEWS else tab), lambda: ids(s), [i for i, u in (before.get("rows") or []) if u])
        if tab in VIEWS:
            s.expect("chip-changes-the-rows-%s" % tab, lambda: 0 < len(ids(s)) < len(before.get("rows") or []), True)
        s.click(UNREAD_CHIP)
        s.expect("chip-again-brings-the-rows-back-%s" % ("a-label-tab" if tab not in VIEWS else tab), lambda: (st(s)["filter"], chip_on(s), [i for i, _ in (before.get("rows") or [])] == ids(s)), (None, False, True))
    s.click("mac-main.splits.1")
    s.key("\u21e7U")
    s.expect("shift-u-turns-the-chip-on-all", lambda: (chip_on(s), st(s)["filter"], s.ctl.uistate()["tab"]), (True, "unread", "all"))
    s.key("\u21e7U")
    s.expect("shift-u-again-turns-it-off", lambda: chip_on(s), False)
    # motion: nothing in the row's source runs outside WhooshMotion, and the three durations it uses are inside 260 ms
    anims = re.findall(r"(?:withAnimation|\.animation)\(([^\n]*)", bar + chip_src)
    s.expect("the-tab-row-animates-only-through-whoosh-motion", lambda: [a for a in anims if "WhooshMotion." not in a], [])
    s.expect("the-tab-row-durations-are-within-260ms", lambda: max(const("TAB_MORPH_MS"), const("HOVER_TINT_MS"), const("ROW_COLLAPSE_MS")) <= 260, True)
    s.expect("the-chip-fades-its-fill-only", lambda: re.findall(r"\.(?:scaleEffect|rotationEffect|blur|offset)\(", chip_src), [])
    why_tag_step(j)
    collapse_step(j)
    list_r8_step(j)
    label_click_step(j)
    header_chevron_step(j)
    undo_timer_step(j)
    doors_list_step(j)
    language_dates_step(j)
    filter_stamp_step(j)
    triage_keys_step(j)
    reset_maps_step(j)
    sweep_step(j)
    row_marks_step(j)


# ---------------------------------------------------------------------------------------------------------------------
# Round 11 (lead, 2026-11-09, owner A94 pick): the why tag. With the inspector's « Pourquoi ici ? » switch on, every row of the open
# list shows a small grey tag naming what placed it there. The spec words are written HERE, from the owner's board (docs/PRD.md A94
# pick, v4.html): the card word first, then the first condition that held. The core's own read (message/<id>/why/<view>) is the
# input; the checks compare what the row draws to the words of that read.
WHY_SWITCH = "mac-labels.inspector.why"            # ASSUMPTION: the inspector lane's switch id (labelscontactsmac-r3 draws the switch; not in this fork)
WHY_VIEW = "needs-reply"                           # the fixture's À répondre label (FixtureFlow.exampleLabelId)
LIST_ROW_LINE = 18                                 # WhooshMetrics.listRowLine: a row's line, points; the tag capsule is 16 inside it
LANGS = ["da", "de", "en", "es", "fi", "fr", "it", "ja", "nb", "nl", "pl", "pt", "sv", "zh-Hans"]
FACT_NAMES = {"unread": "Unread", "unanswered": "Unanswered", "has_attachment": "Attachment", "starred": "Starred", "vip_sender": "VIP",
              "cc_me": "In copy", "mailing_list": "List", "replied_by_me": "Replied"}
KIND_NAMES = {"from": "From", "to": "To", "cc": "Cc", "domain": "Domain", "subject": "Subject", "body": "Body", "list_header": "List", "account": "Account"}
SRC = os.path.join(ROOT, "Sources", "Features", "InboxList")


def held_words(c, account=lambda a: a):
    """One condition that held, in words: a fact is "Name: yes|no", a text condition "Kind: value"."""
    kind = c.get("kind")
    if kind == "has_attachment":
        return "%s: yes" % FACT_NAMES["has_attachment"]
    if kind == "fact":
        return "%s: %s" % (FACT_NAMES[c["fact"]], "yes" if c["wanted"] else "no")
    value = c.get("domain") if kind == "domain" else account(c["account_id"]) if kind == "account" else c.get("words", "")
    return "%s: %s" % (KIND_NAMES[kind], value)


def tag_words(read, list_name):
    """The spec's tag for a why read: the card word, then its first held condition (never the model's name: "AI")."""
    card = read["card"]
    first = held_words(read["matched"][0]) if read.get("matched") else None
    if card == "entry":
        head, tail = "Rule", first
    elif card == "ai":
        head, tail = "✦ AI", None
    elif card == "exit":
        head, tail = "✦ AI", first
    elif card == "fallback":
        head, tail = "Fallback", list_name
    else:
        head, tail = "By hand", None
    return head if tail is None else "%s · %s" % (head, tail)


def why_core(s, mid, view):
    """The core's why read of one message in one view, or None when the view does not hold it."""
    try:
        v = core(s, "message/%s/why/%s" % (mid, view))
    except Unreadable as e:
        if "no core value" in str(e):                 # the core's own answer for a message the view does not hold; any other failure stays "not judged"
            return None
        raise
    return json.loads(v) if isinstance(v, str) else v


def view_title(s, view):
    return next(r["title"] for r in s.ctl.rows("mac-main.splits") if r["id"] == view)


def tagged(s, view):
    """[(row, read)] for the rows the open list holds, the read taken from the core."""
    return [(r, why_core(s, r["id"], view)) for r in rows(s)]


def pause(s):
    if s.j.ctl is not None and not getattr(s.j.ctl, "instant", False):
        time.sleep(0.3)


def reveal(s, lid):
    """Open the sidebar's closed parents of a label so its row is on screen: the sidebar door for a label does nothing while a parent is closed
    (SidebarState.select needs the row in visibleOrder; a label is visible only when every ancestor is open). Top-down, one toggle door each."""
    for _ in range(8):
        by = {r["id"]: r for r in s.ctl.rows("mac-main.sidebar.labels")}
        row = by.get(lid)
        if row is None or row.get("visible", True):
            return
        chain, p = [], row.get("parent")
        while p:
            chain.append(p)
            p = by.get(p, {}).get("parent")
        closed = [c for c in reversed(chain) if not by.get(c, {}).get("open", True)]
        if not closed:
            return
        s.click("mac-main.sidebar.label.%s.toggle" % closed[0])
        pause(s)


def open_view(s, view):
    tabs = safe_tabs(s)
    s.click("mac-main.splits.%d" % (tabs.index(view) if view in tabs else 0))
    pause(s)


def bare_strings(src):
    """User-facing string literals a source file holds outside the catalogue: everything with a letter that is not a Words.text key,
    a switch key (case "x":) or an accessibility id."""
    t = re.sub(r"//[^\n]*", "", src)
    t = re.sub(r"\\u\{[0-9A-Fa-f]+\}", "", t)           # \u{2726} and \u{00B7} are marks, not words
    t = re.sub(r'Words\.text\("[^"\n]*"', "", t)
    t = re.sub(r'case "[^"\n]*"\??:', "", t)
    t = re.sub(r'whooshAx\("[^"\n]*"', "", t)
    t = re.sub(r'\bcase\s+\w+\s*=\s*"[a-z0-9_-]+"', "", t)   # a String enum's raw value, one lower-case token (CountPill.Fill "onaccent"), is a name, not a word
    return re.findall(r'"[^"\n]*[A-Za-z][^"\n]*"', t)


def catalogue_faults(words_src, catalogue):
    """Every key the words file asks Words.text for must hold a non-empty value in each of the 14 languages."""
    out = []
    for k in sorted(set(re.findall(r'Words\.text\("([^"\n]+)"', words_src))):
        loc = (catalogue.get(k) or {}).get("localizations")
        if not loc:
            out.append("%s: not in the catalogue" % k)
            continue
        out += ["%s: %s has no value" % (k, l) for l in LANGS if not (loc.get(l) or {}).get("stringUnit", {}).get("value")]
    return out


def tag_motion_faults(row_src, view_src):
    """The tag appears by opacity over HOVER_TINT_MS and moves nothing: its transition is opacity, its animation is WhooshMotion.hover."""
    m = re.search(r"private var whyTagView.*?\n    \}\n", row_src, re.S)
    block = m.group(0) if m else ""
    faults = []
    if ".transition(.opacity)" not in block:
        faults.append("the tag has no opacity transition")
    anims = re.findall(r"\.animation\(([^\n]*)", block)
    if not anims:
        faults.append("the tag has no animation")
    faults += ["animation outside WhooshMotion.hover: %s" % a for a in anims if "WhooshMotion.hover" not in a]
    faults += ["tag view moves: %s" % x for x in re.findall(r"\.(?:scaleEffect|rotationEffect|blur|offset)\(", view_src)]
    if const("HOVER_TINT_MS") > 260:
        faults.append("HOVER_TINT_MS is over 260")
    return faults


def read_sources():
    def rd(name):
        return open(os.path.join(SRC, name), encoding="utf-8").read()
    cat = json.load(open(os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8"))["strings"]
    return {"row": rd("InboxRowView.swift"), "view": rd("WhyTagView.swift"), "words": rd("WhyTagWords.swift"), "catalogue": cat}



def card_case(s, card, view, must=None):
    """J1.52: every row of `view` that carries `card` draws the spec's words of its read. `must` names a row the case seeded for itself: the case
    is NOT JUDGED unless that row is on the screen and carries the card, so a row left over from the corpus can never stand in for it."""
    def hits():
        found = [(r, rd) for r, rd in tagged(s, view) if rd and rd["card"] == card and (must is None or r["id"] == must)]
        if not found:
            raise Unreadable("no row of %s%s holds the %s card in this corpus" % (view, "" if must is None else " named " + must, card))
        return found

    def drawn():
        found = hits()
        s.j.notes.append("J1.52 tag-text-%s-card observed in %s: %s" % (card, view, [(r["id"], r["why_tag"], rd["decided_by"]) for r, rd in found]))
        return [r["why_tag"] for r, _ in found]

    agree(s, "tag-text-%s-card" % card, drawn,
          lambda: [tag_words(rd, view_title(s, view)) for _, rd in hits()], "the %s card's tag, the spec's words of the core's read" % card)

def entry_card_case(s):
    """J1.52 tag-text-entry-card under A140: a row whose read says card "entry" (a label whose AI is off, filed by step 1 alone) draws "Rule" and its first held
    condition. Every open view is looked at; the example label holds no such row any more (its AI rules on every mail step 1 picks), so on its own the case
    can be empty on a build, and then it says so in a note and the case above judges the example."""
    def rows_of():
        out = []
        for view in [WHY_VIEW] + [v for v in safe_tabs(s) if v not in VIEWS + [WHY_VIEW]]:
            open_view(s, view)
            out += [(view, r, rd) for r, rd in tagged(s, view) if rd and rd["card"] == "entry"]
        return out
    found = []

    def drawn():
        found[:] = rows_of()
        return [(v, r["id"], r["why_tag"]) for v, r, _ in found]
    agree(s, "tag-text-entry-card", drawn, lambda: [(v, r["id"], tag_words(rd, view_title(s, v))) for v, r, rd in rows_of()],
          "the entry card's tag (a step-1-only label), the spec's words of the core's read, one entry per row")
    if not found:
        s.j.notes.append("J1.52 tag-text-entry-card: no row of any open view carries the entry card (A140: the AI rules on every mail step 1 picks); the example is judged by tag-picked-mails-wear-the-exit-card")
    open_view(s, WHY_VIEW)


def why_tag_step(j, src=None):
    """J1.52 (round 11): the why tag. `src` lets --selftest hand the source checks a mutated copy."""
    src = src or read_sources()
    s = j.step("J1.52", "Round 11: with the inspector's why switch on, every row of the open list wears a grey tag naming the card that placed it")
    for _ in range(3):
        s.key("Escape")
    s.raw("fixture", "reset")
    s.raw("resize", 1280, 820)
    open_view(s, WHY_VIEW)
    st_why = lambda: st(s)["why_tags"]
    def frames():
        got = {r["id"]: frame_if_drawn(s, "mac-main.list.row.%s.text" % r["id"]) for r in rows(s)}
        return {i: f for i, f in got.items() if f is not None}
    # switch OFF first: nothing is drawn and nothing is read, even while the list scrolls
    off = {}
    grab(off, "reads", lambda: st_why()["core_reads"])
    s.key("J")
    s.expect("tag-hidden-when-the-switch-is-off", lambda: (all(r["why_tag"] is None for r in rows(s)), st_why()["on"], st_why()["shown"],
             st_why()["core_reads"] == off["reads"]), (True, False, 0, True))
    before = {}
    s.key("\u2318I")
    s.click("mac-labels.inspector.switch.mailbox")      # the list itself, not the inspector's proof rows
    wait_focus_settled(s)    # the focused row rests 1 pt higher once the focus slide ends: compare rows at rest, not mid-slide
    grab(before, "frames", frames)                     # read with the inspector already open: the tag switch alone is judged (main52)
    grab(before, "reads", lambda: st_why()["core_reads"])
    s.click(WHY_SWITCH)
    pause(s)
    grab(before, "after", lambda: st_why()["core_reads"])

    def shows():
        pairs = tagged(s, WHY_VIEW)
        return st_why()["on"], any(rd for _, rd in pairs), all((r["why_tag"] is not None) == (rd is not None) for r, rd in pairs)
    s.expect("tag-shows-on-every-row-the-view-holds", shows, (True, True, True))
    # a row never moves because of its tag
    def moved():
        wait_focus_settled(s)
        now = frames()
        both = set(now) & set(before["frames"])    # rows drawn before and after: a row below the fold is drawn in neither
        if not both:
            raise Unreadable("not judged: no row's text was drawn both before and after the tag went on")
        return sorted("%s %s -> %s" % (i, before["frames"][i], now[i]) for i in both if now[i] != before["frames"][i])
    s.expect("tag-moves-no-row", moved, [])
    # one case per card: what the row draws is the spec's words of the core's own read. SPEC A140: step 1 only PICKS a mail the AI rules on, so every mail
    # of the needs-reply label (AI on, exit filter on) carries the exit card with the AI's answer; the entry card is what a label whose AI is OFF files by
    # step 1 alone. The fallback, AI and hand cards are judged below, where the corpus carries them.
    def picked_exit():
        reads = [rd for _, rd in tagged(s, WHY_VIEW) if rd]
        return sorted({rd["card"] for rd in reads}), all(rd.get("ai") == "yes" for rd in reads), len(reads) > 0
    # fails on: a mail of the example label whose read says card "entry" (the old example row flow-entry-1), or one the AI did not answer ("ai" not "yes")
    s.expect("tag-picked-mails-wear-the-exit-card", picked_exit, (["exit"], True, True))
    entry_card_case(s)
    card_case(s, "exit", WHY_VIEW)
    s.expect("tag-never-says-jev", lambda: [r["why_tag"] for r in rows(s) if "jev" in (r["why_tag"] or "").lower()], [])
    # the reads follow loads, never scrolls: one per loaded row when the switch went on, none while the list scrolls
    pre = {}
    grab(pre, "reads", lambda: st_why()["core_reads"])
    for _ in range(4):
        s.key("J")

    def once():
        n = len(rows(s))
        return (before["after"] - before["reads"]) == n, st_why()["core_reads"] == pre["reads"]
    s.expect("tags-are-read-once-per-load", once, (True, True))
    # at 960 wide the tag is one line inside the pane, its card word whole
    s.raw("resize", 960, 600)
    pause(s)

    def fit_faults():
        view = s.ctl.measure("mac-main.list.view")["frame"]
        out = []
        seen = 0
        for r in rows(s):
            if r["why_tag"] is None:
                continue
            t = frame_if_drawn(s, "mac-main.list.row.%s.whytag" % r["id"])
            if t is None:
                continue
            seen += 1
            if t["h"] > LIST_ROW_LINE:
                out.append((r["id"], "wraps", t["h"]))
            if t["x"] + t["w"] > view["x"] + view["w"] + 0.5:
                out.append((r["id"], "leaves the pane"))
        if not seen:
            raise Unreadable("not judged: no row's tag is drawn at 960")
        return out
    s.expect("tag-fits-one-line-at-960", fit_faults, [])

    def card_faults():
        out = []
        seen = 0
        for r in rows(s):
            if r["why_tag"] is None:
                continue
            t = frame_if_drawn(s, "mac-main.list.row.%s.whytag" % r["id"])
            if t is None:
                continue
            seen += 1
            c = s.ctl.measure("mac-main.list.row.%s.whytag.card" % r["id"])["frame"]
            if c["x"] < t["x"] - 0.5 or c["x"] + c["w"] > t["x"] + t["w"] + 0.5:
                out.append((r["id"], "card word cut"))
        if not seen:
            raise Unreadable("not judged: no row's tag is drawn at 960")
        return out
    s.expect("tag-keeps-its-card-word-when-cut", card_faults, [])
    s.raw("resize", 1280, 820)
    pause(s)
    # the reply flip: the exit-kept row leaves the list, its read goes, and it carries the fallback tag where it landed
    flip = {}
    grab(flip, "id", lambda: next(r["id"] for r, rd in tagged(s, WHY_VIEW) if rd and rd["card"] == "exit"))
    grab(flip, "n", lambda: len(rows(s)))
    s.raw("fixture", "reply", flip.get("id"))
    pause(s)

    def after_flip():
        left_list = flip["id"] not in ids(s) and len(rows(s)) == flip["n"] - 1
        read_gone = why_core(s, flip["id"], WHY_VIEW) is None
        fresh = all(r["why_tag"] == (tag_words(rd, view_title(s, WHY_VIEW)) if rd else None) for r, rd in tagged(s, WHY_VIEW))
        open_view(s, "primary")
        landed = next((r["why_tag"] for r in rows(s) if r["id"] == flip["id"]), "not in Primary")
        open_view(s, WHY_VIEW)
        return left_list, read_gone, fresh, landed
    s.expect("tag-gone-after-the-reply-flip", after_flip, (True, True, True, "Fallback · Primary"))
    # the fallback card: the flipped row now sits in Primary under the exit filter's rejection (the flow example's own rejected row is archived by
    # the earlier steps of this journey, and `fixture reset` does not bring it back)
    open_view(s, "primary")
    card_case(s, "fallback", "primary", must=flip.get("id"))
    # the AI card: an arrival whose words fit Provanta is filed by the model under a label whose exit filter is off
    s.raw("fixture", "deliver", AI_ARRIVAL["id"], write_arrival(AI_ARRIVAL["id"], AI_ARRIVAL))
    # the hand card: a Primary row the owner files under Provanta with the picker
    hand = {}
    grab(hand, "id", lambda: next(r["id"] for r in rows(s) if r["id"] not in (flip.get("id"), AI_ARRIVAL["id"])))
    s.click("mac-main.list.row.%d" % row_of(s, hand.get("id") or "none"))
    s.key("L")
    s.type("Provanta")
    s.key("Enter")
    open_view(s, AI_VIEW)
    card_case(s, "ai", AI_VIEW, must=AI_ARRIVAL["id"])
    card_case(s, "hand", AI_VIEW, must=hand.get("id"))
    open_view(s, WHY_VIEW)
    # the words and the motion: source checks (pure, so --selftest can hand them a mutated copy)
    s.expect("tag-has-no-bare-string", lambda: bare_strings(src["words"]) + bare_strings(src["view"]), [])
    s.expect("tag-words-are-in-14-languages", lambda: catalogue_faults(src["words"], src["catalogue"]), [])
    s.expect("tag-fades-over-hover-tint-ms", lambda: tag_motion_faults(src["row"], src["view"]), [])
    s.key("⌘I")


def collapse_sources():
    """The two list sources the collapse check reads (pure, so --selftest can hand it a mutated copy)."""
    def rd(name):
        return open(os.path.join(SRC, name), encoding="utf-8").read()
    return {"view": rd("InboxListView.swift"), "filter": rd("ListFilter.swift")}


def side_read_faults(src):
    """Round 12 (list r7, docs/PRD.md A105): the reads that follow a change of the rows (the star marks, the why tags) are core reads. On the real
    core they wait on the store, so run inside the transaction that commits the collapse they hold the first frame back past ROW_COLLAPSE_MS and the
    row vanishes with no motion. Spec: the view's `.onChange(of: model.rows)` calls `ListFilters.rowsChanged` and never a core read itself, and
    `rowsChanged` takes its reads after a Task.sleep."""
    faults = []
    line = next((l for l in src["view"].splitlines() if ".onChange(of: model.rows)" in l), "")
    if "ListFilters.rowsChanged(" not in line:
        faults.append("onChange(of: model.rows) does not call rowsChanged")
    if "refreshStarred(" in line or "WhyTags.refresh(" in line:
        faults.append("onChange(of: model.rows) reads the core on the key frame")
    body = src["filter"].split("static func rowsChanged", 1)[-1].split("\n    }\n", 1)[0]
    if "Task.sleep" not in body or body.find("Task.sleep") > body.find("refreshStarred("):
        faults.append("rowsChanged reads before it sleeps")
    return faults


def collapse_step(j, src=None):
    """J1.53 (round 12, list r7): E on the real core collapses the row like the fixture, the focus moves on the key frame, Z inserts it back.
    The driven cases read the probe the list keeps (`collapse_glided`: the removal ran inside the animated transaction); the source case
    pins the one difference found between the fixture path and the real core path (the side reads)."""
    src = src or collapse_sources()
    s = j.step("J1.53", "Round 12 (list r7): archive collapses the row inside the animated transaction, the focus moves on the key frame, Z inserts it back")
    for _ in range(3):
        s.key("Escape")
    s.raw("fixture", "reset")
    s.click("dock.whoosh")
    pause(s)
    before = {}
    grab(before, "ids", lambda: ids(s))
    s.key("J")                                  # the second row: an archive that sends the focus to the top is seen
    wait_focus_settled(s)
    # `grab` stores None when the list could not be read (main76: TypeError here stopped the whole run). Then the three driven cases are NOT JUDGED
    # with that reason, never a crash and never a pass; the source case below reads files and is judged either way.
    have = isinstance(before["ids"], list) and len(before["ids"]) >= 3
    target, after_target = (before["ids"][1], before["ids"][2]) if have else (None, None)

    def need_ids():
        if not have:
            raise Unreadable("the rows before the archive were not read (got %r), so the second and third row ids are unknown" % (before["ids"],))
    s.key("E")
    s.expect("archive-row-leaves-inside-the-collapse", lambda: (need_ids(), (target in ids(s), st(s)["collapse_glided"]))[1], (False, True))
    s.expect("archive-focus-moves-on-the-key-frame", lambda: (need_ids(), next((r["id"] for r in rows(s) if r["focused"]), None))[1], after_target)
    pause(s)
    s.key("Z")
    s.expect("undo-inserts-the-row-back", lambda: (need_ids(), (ids(s)[1], st(s)["collapse_glided"]))[1], (target, True))
    s.expect("side-reads-leave-the-key-frame", lambda: side_read_faults(src), [])


# MUTANTS: per check, the mutation of the app that must turn it red.
# J1.20/initials-every-row | initials read from the subject
# J1.20/preview-every-row | the preview field is dropped
# J1.20/time-is-words | the time shows raw milliseconds
# J1.20/account-colour-per-row | every row carries the neutral ring
# J1.20/pins-on-top | pinned rows sort by time
# J1.20/arrives-by-control | the Dock launch opens another surface
# J1.20/hairline-under-every-row-but-focus | the rows draw no hairline, or draw it under the focused row too
# J1.20/header-line | the header says "Inbox" or counts every tab's unread
# J1.20/command-field-above-header | the command field is not mounted, or sits under the header
# J1.22/snoozed-rows-served | rows mac-main.snooze answers no rows after the snooze
# J1.21/focus-starts-at-0 | launch focuses the second row
# J1.21/focus-index-unchanged | archive moves focus to the top
# J1.21/primary-unread-minus-one | archive does not change counts
# J1.21/undo-puts-it-back-at-0 | undo appends the row at the end
# J1.22/snooze-picker-open | H snoozes at once without the picker
# J1.22/snooze-picker-is-drawn | H holds the pending action and draws no picker (the list gives no view for the snooze overlay)
# J1.22/down-moves-the-snooze-highlight-not-the-list | with the snooze picker up the Down key moves the list's focus, not the picker's line
# J1.23/label-picker-is-drawn | L holds the pending action and draws no picker (the list gives no view for the label overlay)
# J1.23/letters-typed-after-l-are-the-pickers-words | letters typed while the picker's field is not in its window run as list commands (the key monitor's picker hook removed)
# J1.23/the-mail-under-the-picker-is-untouched | a letter typed after L stars the mail or marks it read
# J1.23/esc-in-the-pickers-field-closes-its-action-too | Esc in the label field closes the picker's view and leaves its action (clear-selection closes the overlay first)
# J1.22/snoozed-row-left | snooze leaves the row in place
# J1.22/snoozed-in-store | the snooze command is never sent
# J1.22/snooze-undone | undo skips the snooze entry
# J1.23/label-chip-shows | the label is typed but not applied
# J1.23/pin-goes-first-group | pin does not move the row
# J1.23/pin-follows-focus | focus stays on the old index after pin
# J1.24/two-selected | X does not add the focused row
# J1.24/bulk-archive-removes-selection | archive takes the focused row only
# J1.24/bulk-undo-restores | undo restores one of the two
# J1.25/u-toggles-read | U marks read twice
# J1.25/mark-all-read-clears-unread | mark all read skips the first row
# J1.26/group-by-sender | grouping by sender reports the account
# J1.26/group-by-domain | grouping by domain ignores the sender address and groups by name
# J1.26/compact-on | the compact switch does nothing
# J1.27/tab-goes-to-next-split | Tab stays on Primary
# J1.28/why-opens | Why? opens nothing
# J1.28/why-names-decided-by | the Why line says another model or tab than decided_by
# J1.28/decided-by-equals-core | the row reports no decided_by while the category says Laya
# J1.28/sender-address-from-core | the row reports an empty sender address
# J1.30/trash-key-accepted | # does nothing
# J1.30/trash-row-leaves-at-once | the row waits for its collapse before it leaves the model
# J1.30/trash-in-store | the delete command is never sent
# J1.30/trash-offers-undo-because-the-core-keeps-an-entry | the capsule hides Undo for a delete
# J1.30/undo-brings-the-deleted-row-back | Z does not undo the delete
# J1.30/first-sync-not-shown-when-healthy | an account with a sync error counts as still fetching
# J1.28/key-star | S does nothing
# J1.28/key-mute | Shift+M does nothing
# J1.28/key-spam | ! does nothing
# J1.31/no-ask-code-in-the-list | the header gets its Ask button back (a sparkles button calling mac-ask.open)
# J1.31/no-ask-control-in-the-header | the re-added Ask button is registered under mac-main.list.ask
# J1.31/command-field-still-served | the command field is not mounted
# J1.31/field-names-the-search-key | the key cap is typed as ⌘P, or not read from the live preset
# J1.31/cmd-k-still-accepted | the field no longer answers the ⌘K key
# J1.32/text-measured-when-on | the row text carries no probe id
# J1.32/emblem-drawn-on-every-row-when-on | the row draws no emblem
# J1.32/toggle-off-in-uistate | the toggle writes nothing
# J1.32/no-emblem-drawn-on-any-row-when-off | the rows draw the emblem whatever the setting says
# J1.32/text-moves-left-on-every-row-when-off | the text keeps the emblem's column when the emblem is off
# J1.32/emblem-back-on-every-row-when-on-again | the toggle turns off only
# J1.32/text-back-where-it-was | the text stays at the off position
# J1.27/no-split-editor-control | the edit button and the editor are back
# J1.27/tab-row-is-the-pinned-labels-in-sidebar-order | the tab row lists the label ids sorted by name, not in tree order
# J1.27/tab-names-are-the-label-names | the tab row reads the old split-tab names instead of the label names
# J1.27/vips-is-not-a-tab | the tab row adds VIPs after the pinned labels
# J1.27/shift-tab-goes-back | Shift+Tab moves forward like Tab
# J1.27/unpin-writes-the-label-record | the unpin door hides the tab in the view without sending PinLabel
# J1.27/unpinned-tab-leaves-the-row-and-the-rest-keep-order | the row keeps the unpinned tab, or reorders the others
# J1.27/pin-writes-the-label-record | the pin door draws the tab without sending PinLabel
# J1.27/pinned-tab-comes-back-at-its-tree-place | a pinned label is appended at the end of the row
# J1.28/star-in-store | S answers ok but sends no Star command
# J1.28/mute-in-store | Shift+M answers ok but sends no Mute command
# J1.28/spam-in-store | ! answers ok but sends no MarkSpam command
# J1.33/cap-is-the-constant | the list hard-codes its own cap
# J1.33/one-label-past-the-cap-pinned | the pin door pins nothing
# J1.33/plus-n-counts-the-folded | the chip counts every tab, or one too few
# J1.33/only-the-first-cap-are-shown | the row shows every pinned label
# J1.33/chip-draws | no chip is drawn past the cap
# J1.33/folded-tab-draws-no-segment | the folded tab keeps its segment beside the chip
# J1.33/chip-menu-opens-the-folded-tab | the chip's menu items do nothing
# J1.33/tab-wraps-from-the-last-to-the-first | Tab stops at the last tab
# J1.33/unpinning-the-extras-removes-the-chip | the chip stays at zero
# J1.33/chip-leaves | the chip is drawn with nothing folded
# J1.34/strip-draws-all-five-on-the-settled-row | the strip omits junk, or never shows on a focused row
# J1.34/strip-has-the-keys-in-the-tooltips | a tooltip names no key, or a key typed instead of read from the preset
# J1.34/strip-waits-during-a-key-run | the strip shows on every focus move
# J1.34/strip-returns-after-the-focus-settles | the strip never returns after a key run
# J1.34/strip-left-the-row-the-focus-left | the strip stays on the row the focus left
# J1.34/strip-archive-runs-the-command | the strip's archive button does nothing
# J1.34/strip-archive-undone | the strip's archive writes no undo entry
# J1.34/strip-delete-runs-the-command | the strip's delete button does nothing
# J1.34/strip-junk-runs-the-command | the strip's junk button does nothing
# J1.34/strip-snooze-opens-its-picker | the strip's snooze button does nothing
# J1.34/strip-label-opens-its-picker | the strip's label button does nothing
# J1.34/pickers-closed | Esc leaves the picker open
# J1.35/x-selects-the-focused-thread | X does nothing
# J1.35/check-mark-in-the-row | a selected row draws no check mark
# J1.35/shift-j-extends-down | Shift+J moves the focus without selecting
# J1.35/bar-names-the-count-and-the-actions | the bar counts one less, or lacks junk
# J1.35/bar-draws | no selection bar is drawn
# J1.35/esc-clears-the-selection | Esc leaves the selection
# J1.35/bar-leaves | the bar stays after the selection clears
# J1.35/shift-k-extends-up | Shift+K extends down
# J1.35/cmd-a-selects-from-the-focused-row-down | Cmd+A selects every row
# J1.35/cmd-shift-a-selects-all | Cmd+Shift+A selects from the focused row
# J1.35/bar-cross-clears | the bar's cross does nothing
# J1.35/batch-archive-takes-all-three | the bar's archive takes the focused row only
# J1.35/one-undo-brings-all-three-back | the batch writes one undo entry per row
# J1.35/batch-delete-takes-both | the bar's delete takes the focused row only
# J1.35/batch-junk-takes-both | the bar's junk takes the focused row only
# J1.36/shift-u-keeps-the-unread-rows | the unread filter keeps read rows
# J1.36/chip-names-the-filter | the chip is drawn without the filter's name
# J1.36/j-steps-over-the-hidden-rows | J moves to the next row whatever the filter hides
# J1.36/k-steps-back-over-them | K moves to the previous row whatever the filter hides
# J1.36/esc-clears-the-filter | Esc leaves the filter
# J1.36/chip-leaves-with-the-filter | the chip stays after the filter clears
# J1.36/same-key-again-clears-it | the second press keeps the filter
# J1.36/chip-click-clears-it | a click on the chip does nothing
# J1.36/shift-s-keeps-the-starred-rows | the starred filter keeps unstarred rows
# J1.36/shift-r-keeps-the-conversations-nobody-answered | the no-reply filter keeps threads of several messages
# J1.36/shift-i-filters-through-the-core | Shift+I filters by something, or says nothing
# J1.36/a-filter-that-keeps-nothing-says-so | an empty filter shows the full list
# J1.37/id-select-conversation-reached-the-list | X is not routed to the select-conversation id
# J1.37/id-select-all-reached-the-list | Cmd+Shift+A is not routed to the select-all id
# J1.37/id-select-from-here-reached-the-list | Cmd+A is not routed to the select-from-here id
# J1.37/id-star-reached-the-list | S is not routed to the star id
# J1.37/id-filter-unread-reached-the-list | Shift+U is not routed to the filter-unread id
# J1.37/id-filter-no-reply-reached-the-list | Shift+R is not routed to the filter-no-reply id
# J1.37/id-pin-reached-the-list | Shift+P is not routed to the pin id
# J1.37/id-junk-reached-the-list | the junk id has no handler in the list
# J1.37/id-delete-reached-the-list | the delete id has no handler in the list
# J1.37/id-mute-reached-the-list | the mute id has no handler in the list
# J1.37/id-filter-important-reached-the-list | Shift+I is taken by another handler
# J1.38/command-field-opens-the-search | a click on the field does nothing
# J1.38/tab-segment-0-opens-its-tab | a click on a tab does nothing
# J1.38/tab-segment-1-opens-its-tab | a click on a tab does nothing
# J1.38/tab-segment-2-opens-its-tab | a click on a tab does nothing
# J1.38/every-click-door-is-clicked-by-this-check | a click door is registered that this check never clicks
# J1.39/chip-is-there-before-remove-label | the label picker applies nothing
# J1.39/key-remove-label-reaches-the-list | Y is not routed to the remove-label id
# J1.39/key-remove-label-removes-the-chip-and-steps | Y still answers 'not in the core yet', or steps the wrong way
# J1.39/key-remove-label-chip-gone-in-the-store | Y drops the chip on screen only
# J1.39/chip-is-there-before-remove-label-next | the label picker applies nothing
# J1.39/key-remove-label-next-reaches-the-list | [ is not routed to the remove-label-next id
# J1.39/key-remove-label-next-removes-the-chip-and-steps | [ still answers 'not in the core yet', or steps the wrong way
# J1.39/key-remove-label-next-chip-gone-in-the-store | [ drops the chip on screen only
# J1.39/chip-is-there-before-remove-label-previous | the label picker applies nothing
# J1.39/key-remove-label-previous-reaches-the-list | ] is not routed to the remove-label-previous id
# J1.39/key-remove-label-previous-removes-the-chip-and-steps | ] still answers 'not in the core yet', or steps the wrong way
# J1.39/key-remove-label-previous-chip-gone-in-the-store | ] drops the chip on screen only
# J1.39/chip-is-there-before-remove-all-labels | the label picker applies nothing
# J1.39/key-remove-all-labels-reaches-the-list | Shift+Y is not routed to the remove-all-labels id
# J1.39/key-remove-all-labels-removes-the-chip-and-steps | Shift+Y still answers 'not in the core yet', or steps the wrong way
# J1.39/key-remove-all-labels-chip-gone-in-the-store | Shift+Y drops the chip on screen only
# J1.39/key-y-on-a-thread-without-a-label-is-refused | Y on a thread without a label claims success
# J1.39/key-v-opens-the-move-picker | V opens nothing
# J1.39/move-picker-mounted-over-the-list | the list's overlay host draws nothing for 'move'
# J1.39/esc-closes-the-picker | Esc leaves the picker up
# J1.39/move-takes-the-thread-off-the-list | the move leaves the thread on the list
# J1.39/move-lands-labelled-and-archived | the move labels without archiving
# J1.39/one-undo-brings-the-move-back | the move needs two undos
# J1.29/list-mounted-in-pane | InboxListFeature claims no slot, the shell placeholder draws
# J1.29/account-address-from-core | rows read no account book, every address is empty
# J1.29/emblem-shown-by-default | the setting defaults to off
# J1.29/emblem-hidden-when-off | the toggle is not read, the emblem stays
# J1.29/emblem-back-when-on | the toggle writes off only
# J1.29/second-key-inside-focus-slide-jumps | every move animates whatever the time since the last one
# J1.29/near-move-slides | every focus move fades
# J1.29/far-move-fades | SLIDE_MAX_ROWS is ignored, every move slides

# J1.40/no-reply-is-the-filed-needs-reply-rows | no reply keeps threads of one message instead of the core's filter
# J1.40/filter-chip-on-screen | the chip is not drawn
# J1.40/same-key-clears-the-filter | the same key opens the filter again
# J1.40/important-filters-through-the-core | important keeps every row
# J1.40/important-no-longer-refused | important still answers "not in the core yet"
# J1.40/esc-clears-filter-and-chip | Esc leaves the chip up
# J1.40/starred-is-the-core-starred-set | starred keeps unread rows
# J1.41/archived-row-left | archive leaves the row
# J1.41/shift-e-on-the-archive-views-focused-thread-brings-it-back | Shift+E on the Archive view unarchives a remembered id, not the focused thread
# J1.41/the-thread-is-back-in-the-inbox | unarchive sends nothing to the core
# J1.41/shift-e-on-an-inbox-thread-passes-on | Shift+E in the inbox unarchives the last archived ids
# J1.41/shift-e-after-undo-passes-on | Z leaves the last-archived memory set and Shift+E says "Back in the list" for a row already there
# J1.42/end-of-list-names-tab-and-count | the end line counts the model's rows, not the shown ones
# J1.42/end-line-drawn | the end line is not drawn
# J1.42/not-loading-with-rows | loading reads true while rows show
# J1.43/filtered-empty-words | a filter that keeps nothing shows a blank pane
# J1.43/filtered-empty-keeps-the-chip | the chip hides with the rows
# J1.44/sidebar-sent-shows-the-sent-mailbox | the list ignores AppModel.mailbox (probe reads nil)
# J1.44/header-names-the-mailbox-and-its-count | the header keeps the account filter and the unread count
# J1.44/no-tab-is-current-while-a-mailbox-shows | the tab thumb stays on the old tab over a mailbox
# J1.44/end-line-names-the-mailbox | the end line names the tab under a mailbox
# J1.44/tab-click-returns-to-the-inbox | a tab click switches the tab and leaves the mailbox set
# J1.44/tab-click-makes-the-tab-current | the clicked tab is not marked current
# J1.44/header-back-to-the-unread-line | the header keeps the mailbox after the tab click
# J1.44/current-tab-clicked-again-leaves-the-mailbox | clicking the current tab is a no-op over a mailbox
# J1.45/undo-words-kept-as-a-key | the capsule stores translated text, no key
# J1.45/undo-words-redrawn-in-french | the capsule keeps the English text after the switch
# J1.45/built-in-tab-names-in-french | the tab row reads the stored English names
# J1.45/undo-words-back-in-english | the capsule keeps the French text after switching back
# J1.46/why-line-uses-the-tab-rows-name | the why line reads the stored split name (CoreText.tabName)
# J1.46/why-line-has-no-stored-english-name | "Placé dans Primary" under French
# J1.46/why-line-back-in-english | the why line keeps the French name after switching back
# J1.47/foot-strip-is-the-lock | the list draws no foot strip, or the capsule's padding sizes it
# J1.47/foot-strip-unchanged-under-the-capsule | the strip grows while the undo capsule shows
# J1.48/the-next-row-moved-up-into-place | archive removes the wrong row, or the next row does not take its place
# J1.48/mid-collapse-the-next-row-is-between-its-old-and-new-place | the removal jumps (RowCollapse.run calls the command without the animation)
# J1.48/the-removal-glided | every removal is taken as rapid
# J1.48/a-held-second-key-still-glides | a second E inside FOCUS_SLIDE_MS drops the row with no motion (the old rapid rule is back)
# J1.49/smart-folder-header-names-it-with-its-count | the header keeps "All accounts · N unread" over a smart folder
# J1.49/tab-click-leaves-the-smart-folder | a tab click keeps AppModel.smartFolderId set
# J1.50/empty-primary-names-the-next-snooze-return | the zero state reads the snoozed rows from a tab named "snoozed" (no rows, no time); or the archive key E is ignored (TriageActions.archive returns false), so the loop leaves the rows and the list never reaches 0
# J1.51/tab-order-is-views-first | PinnedTabs.all sorts the labels before the views
# J1.51/tab-row-reads-inbox-tabs | a view is built with isView: false (the tab row ignores the kind inbox/tabs serves)
# J1.51/views-are-named-primary-and-all | a view's name falls back to its id ("primary")
# J1.51/a-view-tab-offers-no-label-menu | PinnedTab.menu answers ["unpin"] for a view
# J1.51/a-label-tab-keeps-its-unpin-menu | PinnedTab.menu answers [] for a label
# J1.51/the-tab-row-has-no-drag | a .draggable(t.id) on the tab segment
# J1.51/the-unread-chip-shows-its-key | the chip's probe key reads another command's chord
# J1.51/view-counts-come-from-the-core | PinnedTabs.all reads the first counts().tabs entry for every tab
# J1.51/all-is-at-least-primary | PinnedTabs.all reads Primary's figures for All, or All's for Primary
# J1.51/chip-turns-the-filter-on-primary | the chip button does nothing
# J1.51/chip-turns-the-filter-on-all | the chip toggles only on a label tab (the filter is cleared when a view is open)
# J1.51/chip-turns-the-filter-on-a-label-tab | the chip button does nothing on a label tab
# J1.51/chip-keeps-the-unread-rows-primary | the chip sets the state without the core's filter (every row stays)
# J1.51/chip-keeps-the-unread-rows-all | ListFilters.passing reads tabId "primary" for every tab
# J1.51/chip-keeps-the-unread-rows-a-label-tab | the chip keeps the unread rows of Primary on a label tab
# J1.51/chip-changes-the-rows-primary | the filter keeps every row on Primary (marks the first row the other way when Primary holds only read or only unread rows)
# J1.51/chip-changes-the-rows-all | the filter keeps every row on All
# J1.51/chip-again-brings-the-rows-back-primary | the second click leaves the filter on
# J1.51/chip-again-brings-the-rows-back-all | the second click leaves the filter on
# J1.51/chip-again-brings-the-rows-back-a-label-tab | the second click leaves the filter on
# J1.51/shift-u-turns-the-chip-on-all | the chip's `on` reads a state of its own, not the list's filter
# J1.51/shift-u-again-turns-it-off | the second key leaves the chip on
# J1.51/the-tab-row-animates-only-through-whoosh-motion | an .animation(.easeOut(duration: 0.4)) on the chip
# J1.51/the-tab-row-durations-are-within-260ms | TAB_MORPH_MS set to 300 in docs/CONSTANTS.md
# J1.51/the-chip-fades-its-fill-only | a .scaleEffect on the chip
# J1.52/tag-hidden-when-the-switch-is-off | the tags are drawn from the core's reads while the inspector's switch is off
# J1.52/tag-shows-on-every-row-the-view-holds | the last row of the list gets no tag
# J1.52/tag-text-entry-card | an entry tag names the chain's last held condition instead of its first
# J1.52/tag-picked-mails-wear-the-exit-card | a mail step 1 picked and the AI accepted still reads the entry card (step 1 filed it, the AI never ruled: the pre-A140 order)
# J1.52/tag-text-ai-card | the AI tag drops the sparkle and reads "AI"
# J1.52/tag-text-exit-card | an exit-kept row names no deciding fact: "✦ AI" alone
# J1.52/tag-text-hand-card | a hand-filed row reads "Rule"
# J1.52/tag-text-fallback-card | a fallback row names no destination: "Fallback" alone
# J1.52/tag-never-says-jev | a row the model decided (decided_by jev) reads "Jev"
# J1.52/tags-are-read-once-per-load | every scroll re-reads the why of every row
# J1.52/tag-moves-no-row | a row with a tag is taller: its sender line sits 4 pt lower
# J1.52/tag-fits-one-line-at-960 | a long condition wraps the tag to two lines and spills it out of the pane
# J1.52/tag-keeps-its-card-word-when-cut | the cut takes the card word with the condition
# J1.52/tag-gone-after-the-reply-flip | the reads are taken once and never again after the list changes (a flipped row keeps its old tag in Primary)
# J1.52/tag-has-no-bare-string | the fallback card word is a literal in WhyTagWords.swift
# J1.52/tag-words-are-in-14-languages | the catalogue lacks "By hand"
# J1.52/tag-fades-over-hover-tint-ms | the tag animation is .easeOut(duration: 0.4) instead of WhooshMotion.hover
# J1.53/archive-row-leaves-inside-the-collapse | the real core's archive removes the row outside the animated transaction (the row jumps)
# J1.53/archive-focus-moves-on-the-key-frame | the focus waits a frame, or jumps to the top, after the archive
# J1.53/undo-inserts-the-row-back | Z does not bring the archived row back at its place
# J1.53/side-reads-leave-the-key-frame | the view's onChange(of: model.rows) reads the core's starred marks again in the same turn


# ---------------------------------------------------------------------------------------------------------------------
# Round 13 (list r8, 2026-11-10): the owner's two picks. A, "Count badge" (board message-card-r1): a thread row (threadCount > 1) wears a count pill
# after the sender, a single message none, and a thread whose newest message is the owner's own wears a small reply arrow before the subject.
# F2, "The count is the switch" (board unread-filter-r1): the "Non lus" pill leaves the tab row; the count line's "N unread" is the control,
# with the key cap beside it; on, it reads "N unread · Show all". The drive reads the rows probe (`count_pill`, `replied_last`), the frame probe
# (the pill and the arrow are drawn where the probe says) and the control's probe (`unread_chip`).
def list_r8_sources():
    def rd(name):
        return open(os.path.join(SRC, name), encoding="utf-8").read()
    cat = json.load(open(os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8"))["strings"]
    return {"count": rd("UnreadCount.swift"), "pill": rd("CountPill.swift"), "catalogue": cat}


def unquoted(text):
    """A view file without the strings that are not words: an SF Symbol name, a number interpolated into a Text."""
    return re.sub(r'\\\([^)\n]*\)', "", re.sub(r'systemName: "[^"\n]*"', "", text))


def pill_ax(r):
    return "mac-main.list.row.%s.count" % r["id"]


def arrow_ax(r):
    return "mac-main.list.row.%s.replied" % r["id"]


def need(found, what):
    """A case the corpus cannot serve is NOT JUDGED, never passed."""
    if not found:
        raise Unreadable("the corpus holds no %s" % what)
    return found


def list_r8_step(j, src=None):
    """J1.54. `src` lets --selftest hand the source checks a mutated copy."""
    src = src or list_r8_sources()
    s = j.step("J1.54", "Round 13 (list r8): the count pill and the reply arrow on a thread row; the Unread control is the count line's count")
    for _ in range(3):
        s.key("Escape")
    s.raw("fixture", "reset")
    s.click("dock.whoosh")
    pause(s)
    s.click("mac-main.splits.0")
    pause(s)
    threads = lambda: need([r for r in rows(s) if r["thread_count"] > 1], "thread row (more than one message)")
    singles = lambda: need([r for r in rows(s) if r["thread_count"] <= 1], "single-message row")
    agree(s, "thread-row-shows-its-count-pill", lambda: [(r["count_pill"], (s.click(r["ax"]), onscreen(s, pill_ax(r), wait=0.3))[1]) for r in threads()],
          lambda: [(r["thread_count"], True) for r in threads()], "a thread row's pill (number, drawn), its message count")
    agree(s, "single-message-shows-no-pill", lambda: [(r["count_pill"], gone(s, pill_ax(r), wait=0.3)) for r in singles()],
          lambda: [(None, True) for _ in singles()], "a single message's pill (number, absent), none")
    # the arrow: drawn exactly where the probe says the owner's own message is the newest, and never on a single message
    agree(s, "reply-arrow-matches-the-thread-state", lambda: [(r["replied_last"] and r["thread_count"] > 1, (s.click(r["ax"]), onscreen(s, arrow_ax(r), wait=0.3))[1] if r["replied_last"] else gone(s, arrow_ax(r), wait=0.3))
                                                              for r in rows(s)], lambda: [(r["replied_last"], True) for r in rows(s)], "each row's arrow (a thread, drawn or absent as probed), the probe")
    # the pill's colours: accent while the row is unread, quiet once read, white-on-accent on the focused row (CountPill.fillKind: the one place the drawn fill is chosen)
    def pill_fills():
        seen = {}

        def look():
            for r in rows(s):
                if r["thread_count"] > 1:
                    seen.setdefault("focused" if r["focused"] else ("unread" if r["unread"] else "read"), set()).add(r["count_pill_fill"])
        look()
        for r in threads():
            s.click(r["ax"])
            look()
        for kind in ("unread", "read", "focused"):
            if kind not in seen:
                raise Unreadable("not judged: no %s thread row was seen with its pill" % kind)
        return {k: sorted(v) for k, v in sorted(seen.items())}
    s.expect("count-pill-fill-follows-unread-read-and-focus", pill_fills, {"focused": ["onaccent"], "read": ["quiet"], "unread": ["accent"]})
    # the control: in the count line, not in the tab row, at the line's trailing edge
    def place():
        c = s.ctl.measure(UNREAD_CHIP)["frame"]
        h, t, pane = (s.ctl.measure(x)["frame"] for x in ("mac-main.list.header", "mac-main.splits.0", "mac-main.list.view"))
        gap = pane["x"] + pane["w"] - (c["x"] + c["w"])
        return (h["y"] - 1 <= c["y"] and c["y"] + c["h"] <= h["y"] + h["h"] + 1, c["y"] >= t["y"] + t["h"], 0 <= gap <= 24, st(s)["unread_chip"]["place"])
    s.expect("unread-control-sits-in-the-count-line", place, (True, True, True, "count-line"))
    before = {}
    grab(before, "ids", lambda: ids(s))
    grab(before, "unread", lambda: [r["id"] for r in rows(s) if r["unread"]])
    ctl = lambda: st(s)["unread_chip"]
    s.expect("unread-count-is-secondary-while-off", lambda: ctl()["count_style"], "secondary")
    s.click(UNREAD_CHIP)
    s.expect("unread-count-is-accent-while-on", lambda: ctl()["count_style"], "accent")
    s.expect("unread-control-turns-the-filter-on-from-the-count-line",
             lambda: (st(s)["filter"], ctl()["on"], ctl()["show_all"], ctl()["count_words"] == "%d unread" % st(s)["header_unread"], ids(s)),
             ("unread", True, "\u00b7 Show all", True, before.get("unread")))
    s.click(UNREAD_CHIP)
    s.expect("unread-control-turns-it-off-from-the-count-line", lambda: (st(s)["filter"], ctl()["on"], ctl()["show_all"], ids(s)), (None, False, None, before.get("ids")))
    s.key("\u21e7U")
    s.click(UNREAD_CHIP)
    s.expect("shift-u-and-the-count-line-are-one-switch", lambda: (st(s)["filter"], ctl()["on"]), (None, False))
    s.expect("unread-control-words-come-from-the-catalogue", lambda: bare_strings(unquoted(src["count"])) + bare_strings(unquoted(src["pill"])), [])
    s.expect("unread-words-are-in-14-languages", lambda: catalogue_faults(src["count"] + src["pill"], src["catalogue"]), [])


# J1.54 mutants: per check, the mutation of the app that must turn it red.
# J1.54/thread-row-shows-its-count-pill | a thread row draws no pill, or the wrong number
# J1.54/single-message-shows-no-pill | a single message draws a pill of 1
# J1.54/reply-arrow-matches-the-thread-state | the arrow is drawn on a single message, or the owner's last reply draws none
# J1.54/count-pill-fill-follows-unread-read-and-focus | the pill is one fill for every row: an unread row's pill is quiet, a read row's accent, or the focused row's pill keeps its own fill
# J1.54/unread-count-is-secondary-while-off | the count is drawn in the accent colour while the filter is off
# J1.54/unread-count-is-accent-while-on | the count stays in the secondary colour while the filter is on
# J1.54/unread-control-sits-in-the-count-line | the control stays at the end of the tab row
# J1.54/unread-control-turns-the-filter-on-from-the-count-line | a click on the count does nothing, or the control omits "Show all" while on
# J1.54/unread-control-turns-it-off-from-the-count-line | the second click leaves the filter on
# J1.54/shift-u-and-the-count-line-are-one-switch | the key keeps a switch of its own, apart from the count line's
# J1.54/unread-control-words-come-from-the-catalogue | the tail is typed as "Show all" in the view
# J1.54/unread-words-are-in-14-languages | "Show all" is missing from the catalogue


def header_chevron_sources():
    return {"header": open(os.path.join(SRC, "ListHeader.swift"), encoding="utf-8").read()}


def header_chevron_faults(src):
    """Round 15 (lane reply-actions, doors-list finding): the "All accounts" header drew a chevron with no control behind it (no menu, no click
    door). The header names the filter in words only: no `chevron.down` image beside the filter name. `src` lets --selftest hand a mutated copy."""
    return [l.strip() for l in src["header"].splitlines() if "chevron" in l and "Image(" in l]


def header_chevron_step(j, src=None):
    """J1.57: the list header's account filter name carries no chevron that does nothing (a source check; the driven header row is words only)."""
    src = src or header_chevron_sources()
    s = j.step("J1.57", "Round 15 (reply-actions): the list header's filter name has no dead chevron beside it")
    s.expect("header-filter-name-has-no-dead-chevron", lambda: header_chevron_faults(src), [])


# J1.58/timer-reads-full-when-the-chip-appears | the capsule's clock never starts: the field reads 0 or null at the start
# J1.58/timer-has-drained-by-mid-life | the line never drains: the field stays at 1 for the capsule's whole life
# J1.58/chip-and-timer-are-gone-at-the-end | the capsule has no expiry: it stays up, its timer stuck at 0
# J1.58/no-undo-no-line | a capsule without an Undo button (starred) still carries a timer
# J1.58/second-action-restarts-the-line | a second action keeps the first capsule's clock
# J1.58/timer-reads-full-after-delete | a deleted message's capsule carries no clock (the field stays null)
# J1.58/timer-reads-full-after-pin | a pin's capsule carries no clock (the field stays null)
# J1.58/timer-reads-full-after-snooze | a snooze's capsule carries no clock (the picker's done words are said without the clock)
# J1.58/timer-reads-full-after-label | a label's capsule carries no clock (the label picker's done words are said without the clock)
# J1.58/timer-reads-full-after-reader-delete | the reader's delete shows a capsule with no clock
# J1.58/line-is-one-linear-trim-with-no-second-timer | the line eases (not .linear) in the view text
# J1.58/line-and-expiry-share-one-life | the view carries its own duration number instead of the state's undoChipMs


def undo_timer_sources():
    rd = lambda n: open(os.path.join(SRC, n), encoding="utf-8").read()
    return {"chrome": rd("ListChrome.swift"), "state": rd("InboxListState.swift")}


def undo_life_ms(src):
    m = re.search(r"static let undoChipMs\s*=\s*(\d+)", src["state"])
    return int(m.group(1)) if m else 0


def undo_line_faults(src):
    """The line's view text: one `trim` driven by one `.linear` animation, no Timer / TimelineView / second clock, and the duration comes from
    the state's `undoChipMs` (the number the expiry sleeps for). `src` lets --selftest hand a mutated copy."""
    body = src["chrome"].split("struct UndoTimerLine", 1)[-1]
    faults = []
    if ".trim(" not in body:
        faults.append("the line is not a trim")
    if ".linear(duration:" not in body:
        faults.append("the line does not drain linearly")
    if re.search(r"\bTimer\b|TimelineView|Task\.sleep|DispatchQueue", body):
        faults.append("the line runs a clock of its own")
    if "InboxListState.undoChipMs" not in body:
        faults.append("the line's duration is not the state's undoChipMs")
    if "Task.sleep(for: .milliseconds(Self.undoChipMs))" not in src["state"]:
        faults.append("the expiry does not sleep for undoChipMs")
    return faults


def undo_wait(s, secs):
    ctl = s.j.ctl
    if ctl is not None and hasattr(ctl, "advance"):
        ctl.advance(secs)
    else:
        time.sleep(secs)


UNDO_TIMER_COMMIT = "2e79dd05"   # Undo chip: the 6000 ms clock and the outline timer (AUDIT row 408caa48)


def undo_timer_step(j, src=None):
    """J1.58 (undo-timer): the undo capsule's outline is a timer. The probe row field `undo_timer` (the fraction of the capsule's life left, read
    from the clock that removes the capsule) is about 1 at the start, between 0.3 and 0.7 at mid-life, and the capsule and the field are gone at
    the end; a capsule without an Undo button has no timer; a second action restarts it. Actions covered: archive (E), delete (the trash key),
    snooze and label (the picker's done words), pin, and the reader's delete: every one calls InboxListState.say, which owns the clock."""
    src = src or undo_timer_sources()
    s = j.step("J1.58", "undo-timer: the undo capsule's outline drains over its life and ends as the capsule leaves")
    # the cases that read the app need the commit that brought the timer; a build without it is NOT JUDGED (main63 read these red on a missing feature)
    gate = FC.Gate(UNDO_TIMER_COMMIT)
    ex = lambda name, thunk, want: s.expect(name, gate(thunk), want)
    life = undo_life_ms(src) / 1000.0
    for _ in range(3):
        s.key("Escape")
    s.raw("fixture", "reset")
    s.click("dock.whoosh")
    pause(s)
    timer = lambda: st(s).get("undo_timer")
    s.key("E")
    ex("timer-reads-full-when-the-chip-appears", lambda: (lambda t: t is not None and t >= 0.85)(timer()), True)
    undo_wait(s, life / 2)
    ex("timer-has-drained-by-mid-life", lambda: (lambda t: t is not None and 0.3 <= t <= 0.7)(timer()), True)
    undo_wait(s, life / 2 + 0.3)
    ex("chip-and-timer-are-gone-at-the-end", lambda: (st(s).get("undo_words"), timer()), (None, None))
    s.key("E")
    undo_wait(s, life * 0.6)
    s.key("E")
    ex("second-action-restarts-the-line", lambda: (lambda t: t is not None and t >= 0.85)(timer()), True)
    # S stars the focused row; after the archives above the focus can sit on the refusing account's row (msg-03), where the core refuses the star and the
    # key "does nothing" (main66 log NOTE): click a healthy row first
    healthy = next((r for r in s.ctl.rows("mac-main.list") if r["id"] != "msg-03" and not r["id"].startswith("flow-")), None)
    if healthy:
        s.click(healthy["ax"])
    s.key("S")
    ex("no-undo-no-line", lambda: (st(s)["undo_available"], timer()), (False, None))
    s.expect("line-is-one-linear-trim-with-no-second-timer", lambda: undo_line_faults(src), [])
    s.expect("line-and-expiry-share-one-life", lambda: "undoChipMs" in src["chrome"].split("struct UndoTimerLine", 1)[-1], True)
    # the other actions that show the chip call the same InboxListState.say: delete (#) and pin (shift-P) start the same full line
    # snooze and label finish through their pickers (TriageActions `state.say(p.doneWords(...))`: H, the words, Enter as J1.22; L, a label name, Enter as J1.23) and the reader's
    # delete through ReaderActions.deleteOpen (the reader open on the focused row, then its delete door `mac-main.reader.delete`): each calls the same InboxListState.say
    drives = (("delete", [("key", "#")]), ("pin", [("key", "\u21e7P")]),
              ("snooze", [("key", "H"), ("type", "tomorrow morning"), ("key", "Enter")]),
              ("label", [("key", "L"), ("type", "Suppliers"), ("key", "Enter")]),
              ("reader-delete", [("click", "mac-main.reader.open"), ("click", "mac-main.reader.delete")]))
    for name, acts in drives:
        undo_wait(s, life + 0.3)                                   # the capsule before this one has left
        nxt = next((r for r in s.ctl.rows("mac-main.list") if r["id"] != "msg-03" and not r["id"].startswith("flow-")), None)
        if nxt:
            s.click(nxt["ax"])
        for kind, what in acts:
            {"key": s.key, "type": s.type, "click": s.click}[kind](what)
        ex("timer-reads-full-after-%s" % name, lambda: (lambda w, t: (w is not None, t is not None and t >= 0.85))(st(s).get("undo_words"), timer()), (True, True))


def label_click_sources():
    return {"model": open(os.path.join(ROOT, "Sources/App/AppModel.swift"), encoding="utf-8").read(),
            "label": open(os.path.join(ROOT, "Sources/Features/InboxList/LabelFilter.swift"), encoding="utf-8").read()}


def label_click_step(j, src=None):
    """J1.55 (round 14, sidebar r8, docs/PRD.md A107): a click on a label in the sidebar drives the other two panes. A pinned label opens its
    tab; an unpinned one filters the list to the messages the core says carry it, the reader shows the first, and a label with no message shows
    its empty words and clears the reader. The setup files the focused message under an unpinned owner label through L."""
    src = src or label_click_sources()
    s = j.step("J1.55", "Round 14 (sidebar r8): a label click selects its tab or filters the list; the reader follows; an empty label says so")
    s.raw("fixture", "reset")
    s.click("dock.whoosh")
    pause(s)
    labels = lambda: s.ctl.rows("mac-main.sidebar.labels")
    shown = lambda: s.ctl.rows("mac-main.reader.shown")[0]["shown"]
    first = lambda: (ids(s) or [None])[0]
    pick = {}
    grab(pick, "pinned", lambda: next(r["id"] for r in labels() if r["pinned"] and r["kind"] == "owner"))
    grab(pick, "owners", lambda: [r for r in labels() if not r["pinned"] and r["kind"] == "owner"][:2])
    s.key("J")                                  # the second row is the one filed: the first stays the reader's, so a reader that did not follow is seen
    wait_focus_settled(s)
    grab(pick, "victim", lambda: focus_id(s))
    if len(pick.get("owners") or []) < 2 or not pick.get("victim"):
        s.expect("label-click-needs-two-unpinned-labels", lambda: (_ for _ in ()).throw(Unreadable("the fixture shows fewer than two unpinned owner labels, or no focused row")), True)
        return
    filed, empty = pick["owners"]
    s.key("L")
    s.type(filed["raw_name"])
    s.key("Enter")
    s.key("K")
    wait_focus_settled(s)
    reveal(s, pick["pinned"])
    s.click("mac-main.sidebar.label.%s" % pick["pinned"])
    pause(s)
    wait_focus_settled(s)
    s.expect("label-click-pinned-opens-its-tab", lambda: (s.ctl.uistate().get("tab"), st(s)["label_filter"], shown() == first()), (pick["pinned"], None, True))
    open_view(s, "primary")
    wait_focus_settled(s)
    reveal(s, filed["id"])
    s.click("mac-main.sidebar.label.%s" % filed["id"])
    pause(s)
    wait_focus_settled(s)
    s.expect("label-click-unpinned-lists-only-its-messages", lambda: (st(s)["label_filter"], ids(s)), (filed["id"], [pick["victim"]]))
    s.expect("label-click-reader-shows-the-first-row", lambda: shown(), pick["victim"])
    reveal(s, empty["id"])
    s.click("mac-main.sidebar.label.%s" % empty["id"])
    pause(s)
    wait_focus_settled(s)
    s.expect("label-click-empty-label-says-so-and-clears-the-reader",
             lambda: (ids(s), shown(), empty["name"] in (st(s)["empty_words"] or {}).get("line", "")), ([], None, True))
    s.click("mac-main.sidebar.mailbox.all-inboxes")
    pause(s)
    s.expect("label-click-ends-with-all-inboxes", lambda: (st(s)["label_filter"], len(ids(s)) > 1), (None, True))
    s.expect("label-click-reads-the-core-by-label", lambda: ("labelId: labelId" in src["model"], "labelId: nil" in src["model"]), (True, False))


# Door table (docs/lanes/doors-list.md, control table): every click id ListClicks.swift registers, the control it stands for, and the case that clicks it.
# A pattern is the id with each interpolation written `*`. The "every door" check reads the Swift file and compares: a door in the file that this
# table does not name, or a table row the file does not register, is red, so a control cannot be added or lost without this table saying so.
LIST_DOORS = {
    "mac-main.list.command": "the command field (wide pill and the narrow round button): J1.38/command-field-opens-the-search",
    "mac-main.list.compact": "palette: Compact rows: J1.26",
    "mac-main.list.emblems": "palette / Settings: Show account emblems: J1.29",
    "mac-main.list.filter": "the filter chip's cross: J1.56/filter-chip-door-clears-the-starred-filter",
    "mac-main.list.group.*": "palette: Group by sender / domain / none: J1.26",
    "mac-main.list.labelchip": "the label chip's cross: J1.56/label-chip-door-gives-back-all-inboxes",
    "mac-main.list.mark-all-read": "palette: Mark all as read: J1.25",
    "mac-main.list.perf.start": "dev only (perf probe), no control",
    "mac-main.list.perf.stop": "dev only (perf probe), no control",
    "mac-main.list.row.*": "a row, one click: J1.20 and every case that picks a row",
    "mac-main.list.row.*.act.*": "the hover strip's five buttons: J1.34 (archive, delete, junk, snooze, label)",
    "mac-main.list.row.*.open": "a double click on a row: J1.56/row-open-door-opens-the-thread-with-its-reply",
    "mac-main.list.row.*.create-rule": "the row's context menu and the palette line, Create a rule from this message: J5.9 (journeys/J5.py)",
    "mac-main.list.row.*.why": "the strip's Why? button: J1.28",
    "mac-main.list.selectionbar.*": "the selection bar's five buttons: J1.35 (archive, delete, junk), J1.56 (snooze, label)",
    "mac-main.list.selectionbar.clear": "the bar's cross: J1.35/bar-cross-clears",
    "mac-main.list.undo": "the undo capsule's Undo button: J1.56/undo-door-puts-the-archived-row-back",
    "mac-main.splits.*": "a tab segment: J1.38/tab-segment-N-opens-its-tab",
    "mac-main.splits.more.*": "an item of the +N chip's menu: J1.33",
    "mac-main.splits.pin.*": "the pin door (the sidebar's pin): J1.27",
    "mac-main.splits.unpin.*": "the tab's context menu, Unpin: J1.27",
    "mac-main.splits.unread": "the count line's Unread control: J1.36/chip-click-clears-it",
}


def door_patterns(text):
    return sorted(set(re.sub(r"\\\([^)]*\)", "*", x) for x in re.findall(r'"(mac-main\.[^"]+)"', text)))


def doors_sources():
    return {"clicks": open(os.path.join(SRC, "ListClicks.swift"), encoding="utf-8").read()}


def poll(s, fn, want, tries=30):
    """The last value fn gave, after up to `tries` reads 0.05 s apart (none on a stand-in); stops when it equals `want`."""
    v = fn()
    for _ in range(tries):
        if v == want or s.j.ctl is None or getattr(s.j.ctl, "instant", False):
            break
        time.sleep(0.05)
        v = fn()
    return v


def doors_list_step(j, src=None):
    """J1.56 (doors-list): the controls of the list no case clicked before: the undo capsule's Undo, a double click on a row, the selection bar's
    Snooze and Label, the filter chip's cross, the label chip's cross. Each is clicked through its door and its visible outcome read.
    `fixture reset` does not bring back what earlier steps archived, deleted or junked (it clears the surfaces, not the corpus), so the rows are
    picked from the list as it stands: healthy ones (never msg-03, the refusing account), by position, never by a fixed id."""
    src = src or doors_sources()
    s = j.step("J1.56", "doors-list: the undo button, the double click, the bar's Snooze and Label, the filter chip and the label chip, each clicked once")
    s.expect("every-door-of-the-list-is-in-the-control-table", lambda: (sorted(set(door_patterns(src["clicks"])) - set(LIST_DOORS)),
                                                                          sorted(set(LIST_DOORS) - set(door_patterns(src["clicks"])))), ([], []))
    s.raw("fixture", "reset")
    s.click("dock.whoosh")
    s.click("mac-main.accounts.all")                       # no account filter, no mailbox: the Primary view (earlier steps leave their own)
    s.click("mac-main.sidebar.mailbox.all-inboxes")
    s.click("mac-main.splits.1")                           # All: the view with the most rows left after the earlier steps archived, deleted and junked
    pause(s)
    foot = lambda: s.ctl.rows("mac-main.foot.hints")[0]["surface"]
    h = []
    grab({}, "x", lambda: h.extend(i for i in ids(s) if i != "msg-03" and not i.startswith("flow-")))
    n0 = {}
    grab(n0, "n", lambda: len(ids(s)))
    if len(h) < 5:
        s.expect("list-has-five-healthy-rows", lambda: (_ for _ in ()).throw(Unreadable("the list holds %d healthy rows, five are needed" % len(h))), True)
        return
    tgt, opened, base, starred_id = h[0], h[1], h[2], h[3]
    at = lambda mid: "mac-main.list.row.%d" % row_of(s, mid)
    # Undo button: the capsule that follows an archive
    s.click(at(tgt))
    s.key("E")
    s.expect("capsule-offers-undo-after-an-archive", lambda: (st(s)["undo_available"], tgt in ids(s)), (True, False))
    s.click("mac-main.list.undo")
    s.expect("undo-door-puts-the-archived-row-back", lambda: poll(s, lambda: tgt in ids(s), True), True)
    # a double click on a row: the thread and its reply field
    s.click(at(opened) + ".open")
    s.expect("row-open-door-opens-the-thread-with-its-reply", lambda: poll(s, lambda: (focus_id(s), foot()), (opened, "composer")), (opened, "composer"))
    s.key("Escape")
    s.key("Escape")
    # the selection bar's Snooze and Label
    s.click(at(base))
    s.key("X")
    s.key("⇧J")
    s.expect("bar-is-up-with-two-rows", lambda: st(s)["selected"], 2)
    s.click("mac-main.list.selectionbar.snooze")
    s.expect("bar-snooze-opens-the-picker-on-both-rows", lambda: poll(s, lambda: (st(s)["pending"], len(st(s)["pending_ids"] or [])), ("snooze", 2)), ("snooze", 2))
    s.key("Escape")
    s.click("mac-main.list.selectionbar.label")
    s.expect("bar-label-opens-the-picker-on-both-rows", lambda: poll(s, lambda: (st(s)["pending"], len(st(s)["pending_ids"] or [])), ("label", 2)), ("label", 2))
    s.key("Escape")
    s.key("Escape")
    s.expect("escape-leaves-no-picker-and-no-selection", lambda: (st(s)["pending"], st(s)["selected"]), (None, 0))
    # the filter chip's cross: S stars a row that is not starred yet (a starred row would lose its star), the filter then keeps it
    s.click(at(starred_id))
    if not any(r.get("starred") for r in rows(s) if r["id"] == starred_id):
        s.key("S")
    s.key("⇧S")
    s.expect("starred-filter-keeps-the-starred-row", lambda: (st(s)["filter"], starred_id in ids(s)), ("starred", True))
    s.click("mac-main.list.filter")
    s.expect("filter-chip-door-clears-the-starred-filter", lambda: poll(s, lambda: (st(s)["filter"], len(ids(s)) == n0.get("n")), (None, True)), (None, True))
    # the label chip's cross: an unpinned owner label, its closed parents opened first, holding one row (the setup is J1.55's)
    labels = lambda: s.ctl.rows("mac-main.sidebar.labels")
    pick = {}
    grab(pick, "label", lambda: next(r for r in labels() if not r["pinned"] and r["kind"] == "owner"))
    grab(pick, "victim", lambda: opened)
    if not pick.get("label"):
        s.expect("label-chip-needs-an-unpinned-owner-label", lambda: (_ for _ in ()).throw(Unreadable("the fixture shows no unpinned owner label")), True)
        return
    s.click(at(opened))
    wait_focus_settled(s)
    s.key("L")
    s.type(pick["label"]["raw_name"])
    s.key("Enter")
    reveal(s, pick["label"]["id"])
    s.click("mac-main.sidebar.label.%s" % pick["label"]["id"])
    pause(s)
    wait_focus_settled(s)
    s.expect("label-click-leaves-a-label-filter-with-its-row", lambda: (st(s)["label_filter"], opened in ids(s)), (pick["label"]["id"], True))
    s.click("mac-main.list.labelchip")
    pause(s)
    s.expect("label-chip-door-gives-back-all-inboxes", lambda: poll(s, lambda: (st(s)["label_filter"], len(ids(s)) == n0.get("n")), (None, True)), (None, True))
    s.raw("fixture", "reset")


# J1.56 mutants: per check, the mutation of the app that must turn it red.
# J1.56/every-door-of-the-list-is-in-the-control-table | ListClicks.swift registers a door the control table does not name
# J1.56/capsule-offers-undo-after-an-archive | the capsule offers no Undo after an archive
# J1.56/undo-door-puts-the-archived-row-back | the Undo button runs nothing (ListClicks "mac-main.list.undo" returns false)
# J1.56/row-open-door-opens-the-thread-with-its-reply | the double click focuses the row and presses no Enter
# J1.56/bar-is-up-with-two-rows | X and Shift+J select one row only
# J1.56/bar-snooze-opens-the-picker-on-both-rows | the bar's Snooze runs nothing
# J1.56/bar-label-opens-the-picker-on-both-rows | the bar's Label runs nothing
# J1.56/escape-leaves-no-picker-and-no-selection | Esc leaves the picker open
# J1.56/starred-filter-keeps-the-starred-row | Shift+S opens no starred filter
# J1.56/filter-chip-door-clears-the-starred-filter | a click on the filter chip does nothing
# J1.56/label-click-leaves-a-label-filter-with-its-row | a click on an unpinned label leaves no label filter
# J1.56/label-chip-door-gives-back-all-inboxes | a click on the label chip's cross does nothing



# J1.59 (doors-list round 3): the date words of a row follow the language picked in Settings: General. The list drew weekday and month with a locale
# fixed to en_GB (RowFacts.swift), so a French window showed "Mon" and "4 Nov". The expectation is written here from the CLDR short names.
TIME_WORDS = {
    "fr": ({"Mon": "lun.", "Tue": "mar.", "Wed": "mer.", "Thu": "jeu.", "Fri": "ven.", "Sat": "sam.", "Sun": "dim."},
           {"Jan": "janv.", "Feb": "févr.", "Mar": "mars", "Apr": "avr.", "May": "mai", "Jun": "juin", "Jul": "juil.", "Aug": "août", "Sep": "sept.", "Oct": "oct.", "Nov": "nov.", "Dec": "déc."}),
    "es": ({"Mon": "lun", "Tue": "mar", "Wed": "mié", "Thu": "jue", "Fri": "vie", "Sat": "sáb", "Sun": "dom"},
           {"Jan": "ene", "Feb": "feb", "Apr": "abr", "Aug": "ago", "Dec": "dic"}),   # the months whose Spanish short name is certain; the others are only checked to differ from English
}


def time_in(lang, en):
    """The row's time in `lang`, from its English reading: a clock stays, a weekday or a day and month change words."""
    days, months = TIME_WORDS[lang]
    if re.match(r"^\d\d:\d\d$", en):
        return en
    if en in days:
        return days[en]
    m = re.match(r"^(\d{1,2}) ([A-Z][a-z]{2})$", en)
    return "%s %s" % (m.group(1), months[m.group(2)]) if m and m.group(2) in months else None


def time_faults(lang, en_times, now_times):
    """[(english, expected, got)] for every row whose date words are wrong in `lang`; a row with no certain expectation must at least differ from English."""
    bad = []
    for en, got in zip(en_times, now_times):
        want = time_in(lang, en)
        if (want is not None and got != want) or (want is None and got == en):
            bad.append((en, want, got))
    return bad


def language_dates_step(j):
    s = j.step("J1.59", "doors-list round 3: the date words of the list follow the language picked in Settings: General")
    s.raw("fixture", "reset")
    s.click("dock.whoosh")
    pause(s)
    s.raw("click", "mac-settings-general.appearance.language.en")
    times = lambda: [r["time"] for r in rows(s)]
    base = {}
    grab(base, "en", times)
    s.expect("english-list-has-dated-rows", lambda: any(not re.match(r"^\d\d:\d\d$", t) for t in base.get("en") or []), True)
    for lang in ("fr", "es"):
        s.raw("click", "mac-settings-general.appearance.language." + lang)
        pause(s)
        s.expect("dates-follow-the-language-" + lang, lambda lang=lang: time_faults(lang, base.get("en") or [], times()), [])
    s.raw("click", "mac-settings-general.appearance.language.en")
    pause(s)
    s.expect("dates-come-back-in-english", lambda: times(), base.get("en"))


# J1.59/english-list-has-dated-rows | every row is today's, so no weekday or month is read
# J1.59/dates-follow-the-language-fr | the formatter takes the system or English locale (RowFacts.swift: Locale(identifier: "en_GB"))
# J1.59/dates-follow-the-language-es | the formatter takes the system or English locale
# J1.59/dates-come-back-in-english | the English words are lost after a language round trip


def filter_stamp_sources():
    return {"filter": open(os.path.join(SRC, "ListFilter.swift"), encoding="utf-8").read()}


def filter_stamp_faults(src):
    """The cached answer of the core's filter read is keyed on the rows' unread marks and the starred ids, not on the ids alone."""
    stamp = src["filter"].split("let stamp = FilterStamp(", 1)[-1].split(")\n", 1)[0]
    return [w for w, ok in (("unread marks", "unread" in stamp), ("starred ids", "starred" in stamp)) if not ok]


def filter_stamp_step(j, src=None):
    """J1.60: after "mark all read" under the unread filter the answer was cached on the row ids; the same ids with unread mail again (a reset, a sync)
    kept the empty answer, and the chip then showed no row (main66 J1.51). Found by reading, source check."""
    src = src or filter_stamp_sources()
    s = j.step("J1.60", "doors-list round 4: the filter's cached read follows the unread marks and the starred ids")
    s.expect("filter-cache-follows-read-state", lambda: filter_stamp_faults(src), [])


# ---------------------------------------------------------------------------------------------------------------------
# triage-keys (J1.61): shift M mutes and, on a muted thread, unmutes (the owner's key set has one mute key); the Muted view's bottom bar
# says "Unmute" with the preset's key; shift E returns an archived thread from the Archive view. Each key is judged on the core's mark,
# on the row leaving the view it was in and on the row arriving where it belongs.
TRIAGE_SRC = {"keys": os.path.join(ROOT, "Sources", "Features", "InboxList", "TriageKeys.swift"),
              "foot": os.path.join(ROOT, "Sources", "Features", "MacShell", "MacFootHints.swift")}
TRIAGE_READ = 'message/\\($0)/muted") == "true"'
TRIAGE_TOGGLE = "muted: !unmute"
TRIAGE_BAR = 'Words.text("Unmute")'
TRIAGE_UNARCHIVE = ".unarchive(messageIds: ids)"


def triage_sources():
    return {k: open(v, encoding="utf-8").read() for k, v in TRIAGE_SRC.items()}


def triage_source_faults(src):
    keys, foot = src["keys"], src["foot"]
    mute = keys[keys.index("static func mute("):keys.index("static func markNotDone(")]
    done = keys[keys.index("static func markNotDone("):]
    bar = [l for l in foot.splitlines() if "m.mailbox == .muted" in l]
    bar2 = [l for l in foot.splitlines() if "m.mailbox == .archive" in l]
    return {"mute": [] if TRIAGE_READ in mute and TRIAGE_TOGGLE in mute and "muted: true" not in mute else ["the mute key does not toggle on the core's mark"],
            "bar": [] if len(bar) == 1 and TRIAGE_BAR in bar[0] and 'keyLabel("mute")' in bar[0] and "surface(m) == .list" in bar[0] else ["the Muted view's bar line does not say Unmute with the preset's mute key"],
            "undo": ["the mute key's capsule has no Undo"] if "undoable: false" in mute else [],
            "barArchive": [] if len(bar2) == 1 and 'Words.text("Mark not done")' in bar2[0] and 'keyLabel("mark-not-done")' in bar2[0] and "surface(m) == .list" in bar2[0] else ["the Archive view's bar line does not say Mark not done with the preset's key"],
            "done": [] if TRIAGE_UNARCHIVE in done else ["shift E does not run the core's unarchive on the targets"]}


def triage_keys_step(j, src=None):
    """J1.61. `src` lets --selftest hand the three source checks a mutated copy of the Swift files."""
    src = src or triage_sources()
    faults = triage_source_faults(src)
    s = j.step("J1.61", "triage-keys: shift M mutes and unmutes, the Muted view's bar says Unmute, shift E returns an archived thread")
    for _ in range(3):
        s.key("Escape")
    s.raw("fixture", "reset")
    pause(s)
    hint = lambda: [(r["key"], r["words"]) for r in s.ctl.rows("mac-main.foot.hints") if r["command"] == "mute"]
    target = {}
    grab(target, "id", lambda: rows(s)[0]["id"])
    mid = lambda: target["id"]
    at = lambda: row_of(s, mid())
    s.click("mac-main.list.row.0")
    s.key("⇧M")
    s.expect("shift-m-mutes-the-thread-and-it-leaves-the-inbox", lambda: (core(s, "message/%s/muted" % mid()), mid() in ids(s)), (True, False))
    s.key("G")
    s.key("M")
    s.expect("the-muted-view-holds-the-thread", lambda: mid() in ids(s), True)
    s.expect("the-muted-view-bar-says-unmute-with-its-key", hint, [("⇧M", "Unmute")])
    s.click("mac-main.list.row.%d" % max(at(), 0))
    s.key("⇧M")
    s.expect("shift-m-again-unmutes-the-thread", lambda: core(s, "message/%s/muted" % mid()), False)
    s.expect("the-unmuted-thread-leaves-the-muted-view", lambda: mid() in ids(s), False)
    s.key("G")
    s.key("I")
    s.expect("the-unmuted-thread-is-back-in-the-inbox", lambda: mid() in ids(s), True)
    s.click("mac-main.list.row.%d" % max(at(), 0))
    s.key("E")
    s.key("G")
    s.key("E")
    s.expect("the-archive-view-holds-the-thread", lambda: mid() in ids(s), True)
    s.expect("the-archive-view-bar-says-mark-not-done-with-its-key", lambda: [(r["key"], r["words"]) for r in s.ctl.rows("mac-main.foot.hints") if r["command"] == "mark-not-done"], [("⇧E", "Mark not done")])
    s.click("mac-main.list.row.%d" % max(at(), 0))
    s.key("⇧E")
    s.expect("shift-e-returns-the-archived-thread", lambda: core(s, "message/%s/archived" % mid()), False)
    s.expect("shift-e-says-back-in-the-list", lambda: st(s)["undo_key"], "Back in the list")
    s.expect("the-returned-thread-leaves-the-archive-view", lambda: mid() in ids(s), False)
    s.key("G")
    s.key("I")
    s.expect("the-returned-thread-is-back-in-the-inbox", lambda: mid() in ids(s), True)
    s.click("mac-main.list.row.%d" % max(at(), 0))
    s.key("⇧M")
    s.key("Z")
    s.expect("z-undoes-a-mute-and-the-thread-is-back-in-the-inbox", lambda: (core(s, "message/%s/muted" % mid()), mid() in ids(s)), (False, True))
    s.click("mac-main.list.row.%d" % max(at(), 0))
    s.key("⇧M")
    s.key("G")
    s.key("M")
    s.click("mac-main.list.row.%d" % max(at(), 0))
    s.key("⇧M")
    s.key("Z")
    s.expect("z-undoes-an-unmute-and-the-thread-is-muted-again", lambda: (core(s, "message/%s/muted" % mid()), mid() in ids(s)), (True, True))
    s.key("G")
    s.key("I")
    s.expect("the-mute-key-reads-the-core-mark-and-toggles", lambda: faults["mute"], [])
    s.expect("the-mute-keys-capsule-offers-undo", lambda: faults["undo"], [])
    s.expect("the-archive-view-bar-line-names-mark-not-done-and-the-preset-key", lambda: faults["barArchive"], [])
    s.expect("the-muted-view-bar-line-names-unmute-and-the-preset-key", lambda: faults["bar"], [])
    s.expect("shift-e-runs-the-cores-unarchive", lambda: faults["done"], [])


class TriageCtl:
    """The stand-in for J1.61: three threads, the inbox, the Muted and Archive views, the keys M, E, G and the bottom bar. It models the SPECIFIED
    behaviour; a mutant breaks one thing. (It proves what the checks can tell apart, not the Swift: the three source checks read the Swift.)"""
    instant = True
    ALL = ["m1", "m2", "m3"]

    def __init__(self, mutant=None):
        self.m = mutant
        self.reset()

    def reset(self):
        self.muted, self.archived, self.ghost, self.lost = set(), set(), set(), set()
        self.view, self.focus, self.g, self.said, self.undo = "inbox", 0, False, None, []

    def shown(self):
        if self.view == "muted":
            return [] if self.m == "muted-view-empty" else [i for i in self.ALL if i in self.muted or (i in self.ghost)]
        if self.view == "archive":
            return [] if self.m == "archive-view-empty" else [i for i in self.ALL if i in self.archived or (i in self.ghost)]
        return [i for i in self.ALL if i not in self.muted and i not in self.archived and i not in self.lost]

    def send(self, verb, *a):
        if verb == "fixture":
            self.reset()
        return {"ok": True}

    def click(self, ax):
        if ax.startswith("mac-main.list.row."):
            self.focus = int(ax.rsplit(".", 1)[1])
        return {}

    def key(self, chord):
        if self.g:
            self.g = False
            self.view = {"M": "muted", "E": "archive", "I": "inbox"}.get(chord, self.view)
            self.ghost = set()
            return {}
        if chord == "G":
            self.g = True
        elif chord == "Z":
            if self.undo:
                t, was = self.undo.pop()
                (self.muted.add if was else self.muted.discard)(t)
        elif chord in ("⇧M", "E", "⇧E"):
            vis = self.shown()
            if not vis:
                raise Unreadable("key %s: nothing focused" % chord)
            t = vis[min(self.focus, len(vis) - 1)]
            self.said = None
            if chord == "⇧M":
                if t in self.muted and self.m != "unmute-mutes-again":
                    self.muted.discard(t)
                    if self.m != "unmute-not-undoable":
                        self.undo.append((t, True))
                    if self.m == "unmute-row-stays":
                        self.ghost.add(t)
                    if self.m == "unmute-row-lost":
                        self.lost.add(t)
                elif self.m != "mute-noop":
                    self.muted.add(t)
                    if self.m != "mute-not-undoable":
                        self.undo.append((t, False))
            elif chord == "E":
                self.archived.add(t)
            elif self.view == "archive" or t in self.archived:
                if self.m != "not-done-noop":
                    self.archived.discard(t)
                    if self.m == "not-done-row-stays":
                        self.ghost.add(t)
                    if self.m == "not-done-row-lost":
                        self.lost.add(t)
                    self.said = None if self.m == "not-done-silent" else "Back in the list"
        return {}

    def core(self, path):
        _, i, field = path.split("/")
        return i in (self.muted if field == "muted" else self.archived)

    def rows(self, ax):
        if ax == "mac-main.list":
            return [{"id": i, "ax": "mac-main.list.row.%d" % n} for n, i in enumerate(self.shown())]
        if ax == "mac-main.list.state":
            return [{"undo_key": self.said}]
        if ax == "mac-main.foot.hints":
            hints = [{"command": "archive", "key": "E", "words": "Mark done"}]
            if self.view == "archive" and self.m != "foot-no-not-done":
                hints.insert(0, {"command": "mark-not-done", "key": "⇧E", "words": "Mark not done"})
            if self.view == "muted" and self.m != "foot-no-unmute":
                hints.insert(0, {"command": "mute", "key": "⇧M", "words": "Mute" if self.m == "foot-says-mute" else "Unmute"})
            return hints
        raise Unreadable("the stand-in has no region %s" % ax)

    def uistate(self):
        return {"tab": "primary", "surface": "mac-main"}


TRIAGE_MUT = {   # check name -> the stand-in's mutant tag (report_mutants hands the check's own name to the run)
    "shift-m-mutes-the-thread-and-it-leaves-the-inbox": "mute-noop",
    "the-muted-view-holds-the-thread": "muted-view-empty",
    "the-muted-view-bar-says-unmute-with-its-key": "foot-no-unmute",
    "shift-m-again-unmutes-the-thread": "unmute-mutes-again",
    "the-unmuted-thread-leaves-the-muted-view": "unmute-row-stays",
    "the-unmuted-thread-is-back-in-the-inbox": "unmute-row-lost",
    "the-archive-view-holds-the-thread": "archive-view-empty",
    "shift-e-returns-the-archived-thread": "not-done-noop",
    "shift-e-says-back-in-the-list": "not-done-silent",
    "the-archive-view-bar-says-mark-not-done-with-its-key": "foot-no-not-done",
    "z-undoes-a-mute-and-the-thread-is-back-in-the-inbox": "mute-not-undoable",
    "z-undoes-an-unmute-and-the-thread-is-muted-again": "unmute-not-undoable",
    "the-returned-thread-leaves-the-archive-view": "not-done-row-stays",
    "the-returned-thread-is-back-in-the-inbox": "not-done-row-lost",
}


def triage_mutated(src0):
    k, f = src0["keys"], src0["foot"]
    return {"the-mute-key-reads-the-core-mark-and-toggles": dict(src0, keys=k.replace(TRIAGE_TOGGLE, "muted: true", 1)),
            "the-muted-view-bar-line-names-unmute-and-the-preset-key": dict(src0, foot=f.replace(TRIAGE_BAR, 'Words.text("Mute")', 1)),
            "shift-e-runs-the-cores-unarchive": dict(src0, keys=k.replace(TRIAGE_UNARCHIVE, ".archive(messageIds: ids)", 1)),
            "the-mute-keys-capsule-offers-undo": dict(src0, keys=k.replace('Said("Muted"))   //', 'Said("Muted"), undoable: false)   //', 1)),
            "the-archive-view-bar-line-names-mark-not-done-and-the-preset-key": dict(src0, foot=f.replace('Words.text("Mark not done")', 'Words.text("Archive")', 1))}


def selftest_triage():
    """J1.61 on the stand-in (clean and once per mutant) and on broken copies of the Swift (the three source checks)."""
    src0 = triage_sources()
    assert not any(triage_source_faults(src0).values()), "the Swift is not clean: %s" % triage_source_faults(src0)
    mutated = triage_mutated(src0)
    for name, v in mutated.items():
        assert v != src0, "mutant %s changes nothing: its anchor text is gone from the Swift" % name
    words = mutant_words("J1.61")

    def run(mutant=None):
        j = Journey(ID, TriageCtl(TRIAGE_MUT.get(mutant)))
        triage_keys_step(j, mutated.get(mutant, src0))
        return {c.label.split("/", 1)[1]: (c.state, c.detail) for c in j.checks()}
    return report_mutants(run, words)


# ---------------------------------------------------------------------------------------------------------------------
# triage-keys (J1.62): `fixture reset` puts the two maps this lane added back to launch: the date a list was left (`message/<id>/unsubscribed_at`)
# and a refused filing (`message/<id>/filing`). A mail is unsubscribed through the reader's confirm and a filing refused through `fixture
# filing-refuse`; after the reset both reads are what they were at launch.
def reset_maps_step(j):
    s = j.step("J1.62", "triage-keys: fixture reset clears the unsubscribe date and the refused filing")
    for _ in range(3):
        s.key("Escape")
    s.raw("fixture", "reset")
    pause(s)
    mid = "msg-15"
    launch, dirty = {}, {}
    grab(launch, "filing", lambda: (core(s, "message/%s/filing" % mid) or {}).get("ai"))
    s.click("mac-main.list.row.%d" % max(row_of(s, mid), 0))
    s.click("mac-main.reader.open")
    s.key("\u2318U")
    s.click("mac-main.reader.unsubscribe.confirm")
    s.key("Escape")
    s.raw("fixture", "filing-refuse", mid, "The writing model is not installed on this Mac.")
    grab(dirty, "date", lambda: core(s, "message/%s/unsubscribed_at" % mid))
    grab(dirty, "filing", lambda: (core(s, "message/%s/filing" % mid) or {}).get("ai"))
    s.raw("fixture", "reset")
    pause(s)

    def judged(key, bad):
        def thunk():
            if dirty.get(key) in (None, "", launch.get("filing") if key == "filing" else None):
                raise Unreadable("the case did not get the core into the %s state before the reset, so the reset proves nothing" % key)
            return thunk_read(key)
        return thunk

    def thunk_read(key):
        if key == "date":
            try:
                return core(s, "message/%s/unsubscribed_at" % mid)
            except Unreadable as e:
                # main79: the real socket has no null; for a value the core does not hold it answers ok:false "no core value at <path>" (Verbs.swift
                # core verb; journeys/lib/ctl.py raises Unreadable). That answer, for THIS path, is "no date". Any other failure stays "not judged".
                if "no core value at message/%s/unsubscribed_at" % mid in str(e):
                    return None
                raise
        return (core(s, "message/%s/filing" % mid) or {}).get("ai")
    s.expect("reset-clears-the-unsubscribe-date", judged("date", None), None)
    s.expect("reset-clears-the-refused-filing", judged("filing", None), launch.get("filing"))


class ResetMapsCtl:
    """The stand-in for J1.62: the reader's confirm, the filing verb and `fixture reset`; a mutant leaves one map."""
    instant = True

    def __init__(self, mutant=None):
        self.m, self.at, self.refused, self.msg, self.pending, self.rows_ = mutant, None, False, None, False, ["m1", "msg-15"]

    def send(self, verb, *a):
        if verb == "fixture" and a[0] == "reset":
            if self.m != "reset-keeps-date":
                self.at = None
            if self.m != "reset-keeps-refusal":
                self.refused = False
        elif verb == "fixture" and a[0] == "filing-refuse":
            self.refused = True
        return {"ok": True}

    def key(self, chord):
        if chord == "\u2318U":
            self.pending = True
        return {}

    def click(self, ax):
        if ax.startswith("mac-main.list.row."):
            self.msg = self.rows_[int(ax.rsplit(".", 1)[1])]
        elif ax == "mac-main.reader.unsubscribe.confirm" and self.pending:
            self.at, self.pending = 1762862400000, False
        return {}

    def rows(self, ax):
        if ax == "mac-main.list":
            return [{"id": i} for i in self.rows_]
        raise Unreadable("the stand-in has no region %s" % ax)

    def core(self, path):
        if path.endswith("/unsubscribed_at"):
            if self.at is None:
                raise Unreadable("core %s: no core value at %s" % (path, path))     # the real socket's answer for no value (ok:false), not None
            return str(self.at)
        if path.endswith("/filing"):
            return {"ai": "refused" if self.refused else "configured"}
        raise Unreadable("no path %s" % path)

    def uistate(self):
        return {"surface": "mac-main"}


def selftest_reset_maps():
    words = mutant_words("J1.62")
    tags = {"reset-clears-the-unsubscribe-date": "reset-keeps-date", "reset-clears-the-refused-filing": "reset-keeps-refusal"}

    def run(mutant=None):
        j = Journey(ID, ResetMapsCtl(tags.get(mutant)))
        reset_maps_step(j)
        return {c.label.split("/", 1)[1]: (c.state, c.detail) for c in j.checks()}
    return report_mutants(run, words)


# ---------------------------------------------------------------------------------------------------------------------
# par-inboxlist-sweep: every control of the mail list clicked once, with its visible outcome (owner: "you haven't properly tested every
# button"). The table is docs/lanes/sweep-inboxlist.md. The controls the older cases already click with an outcome (J1.20 to J1.38, J1.56) are named
# there; this step adds the ones none of them clicked with an outcome: every tab segment (J1.38 clicked the first three), every item of the +N
# chip's menu (J1.33 clicked the last), the Group "none" door, the Unread switch turned ON by a click, the Why button's second click.
# Each case puts the list back: the tab row to Primary, the extra pins unpinned, the grouping to none, the filter and the why line closed.
SWEEP = "par-inboxlist-sweep"
SWEEP_WHY = "mac-main.list.row.0.why"


def sweep_labels(s):
    """The labels the sidebar serves: id, pinned, kind (the same rows J1.56 reads)."""
    return s.ctl.rows("mac-main.sidebar.labels")


def sweep_step(j):
    s = j.step(SWEEP, "every control of the mail list clicked once, each with its visible outcome (tab segments, +N chip items, Group none, Unread on, Why twice)")
    s.raw("fixture", "reset")
    s.click("dock.whoosh")
    s.click("mac-main.accounts.all")                       # no account filter, no mailbox: the Primary view
    s.click("mac-main.sidebar.mailbox.all-inboxes")
    s.click("mac-main.splits.0")
    pause(s)
    splits = lambda: s.ctl.rows("mac-main.splits")
    # 1. every tab segment the row draws: its click opens that tab
    def segments():
        shown = [(r["ax"], r["id"]) for r in splits() if r["shown"]]
        got = []
        for ax, tid in shown:
            s.click(ax)
            got.append((tid, poll(s, lambda: s.ctl.uistate()["tab"], tid)))
        return bool(shown) and all(a == b for a, b in got), "segments (tab, tab opened): %r" % got
    s._record("every-tab-segment-opens-its-tab", segments)
    s.click("mac-main.splits.0")
    # 2. every item of the +N chip's menu: pin labels past the cap (the sidebar's pin door), click each folded item, then unpin the extras
    base = {}
    grab(base, "ids", lambda: tab_ids(s))
    added = []

    def pin_past_the_cap():
        need = const("TAB_PINNED_MAX") + 1 - len(label_tab_ids(s))
        for lid in [r["id"] for r in sweep_labels(s) if not r["pinned"] and r["kind"] == "owner"][:max(0, need)]:
            s.click("mac-main.splits.pin.%s" % lid)
            added.append(lid)
    grab({}, "x", pin_past_the_cap)

    def folded_items():
        folded = [r["id"] for r in splits() if not r["shown"]]
        got = []
        for lid in folded:
            s.click("mac-main.splits.more.%s" % lid)
            got.append((lid, poll(s, lambda: s.ctl.uistate()["tab"], lid)))
        return bool(folded) and all(a == b for a, b in got), "chip items (label, tab opened): %r" % got
    s._record("every-item-of-the-plus-n-menu-opens-its-tab", folded_items)
    s.click("mac-main.splits.0")
    for lid in added:
        s.click("mac-main.splits.unpin.%s" % lid)
    s._record("the-extra-pins-are-taken-back-and-the-chip-leaves",
              lambda: (lambda got: (got == (0, base.get("ids")) and bool(added), "chip count, tab row after the unpins: %r against %r" % (got, (0, base.get("ids")))))((st(s)["plus_n"], tab_ids(s))))
    # 3. the Group "none" door: after a grouping, none ends the sections
    s.click("mac-main.list.group.sender")
    s.click("mac-main.list.group.none")
    s._record("group-none-click-ends-the-sections",
              lambda: (lambda got: (got == ("none", [""]), "grouping, group keys of the rows: %r" % (got,)))((st(s)["grouping"], sorted(set(r["group"] for r in rows(s))))))
    # 4. the Unread switch turned ON by a click (J1.36 clicked it to turn the filter off)
    s.key("Escape")
    s._record("unread-switch-click-turns-the-filter-on",
              lambda: (s.click(UNREAD_CHIP), (lambda got: (got == ("unread", True), "filter, chip on: %r" % (got,)))(poll(s, lambda: (st(s)["filter"], chip_on(s)), ("unread", True))))[1])
    s.click(UNREAD_CHIP)                                   # put it back
    # 5. the Why button: its first click opens the line on row 0, its second closes it (J1.28 clicked it once)

    def why_twice():
        mid = rows(s)[0]["id"]
        if st(s)["why_open"] == mid:
            s.click(SWEEP_WHY)                             # an earlier step left it open: start closed
        s.click(SWEEP_WHY)
        first = poll(s, lambda: st(s)["why_open"], mid)
        s.click(SWEEP_WHY)
        second = poll(s, lambda: st(s)["why_open"], None)
        return (first, second) == (mid, None), "why_open after click 1 and click 2: %r against %r" % ((first, second), (mid, None))
    s._record("why-button-clicked-twice-opens-then-closes", why_twice)
    s.raw("fixture", "reset")


# par-inboxlist-sweep mutants: per check, the stand-in with that control disabled.
# par-inboxlist-sweep/every-tab-segment-opens-its-tab | the segments from the fourth on open nothing
# par-inboxlist-sweep/every-item-of-the-plus-n-menu-opens-its-tab | a click on an item of the +N menu opens nothing
# par-inboxlist-sweep/the-extra-pins-are-taken-back-and-the-chip-leaves | the tab's Unpin door does nothing
# par-inboxlist-sweep/group-none-click-ends-the-sections | the Group none door leaves the grouping
# par-inboxlist-sweep/unread-switch-click-turns-the-filter-on | the Unread switch does nothing while the filter is off
# par-inboxlist-sweep/why-button-clicked-twice-opens-then-closes | the Why button's second click does not close the line


class SweepCtl:
    """The stand-in for the sweep: a tab row (two views and the pinned labels, the cap past which the rest fold), twelve owner labels, three rows with
    a sender each, the grouping, the Unread filter and the Why line. It models the SPECIFIED behaviour; a mutant breaks exactly one control."""
    instant = True
    VIEWS = ["primary", "all"]

    def __init__(self, mutants=()):
        self.m = set(mutants)
        self.reset()

    def reset(self):
        self.labels = [{"id": "lbl-%d" % i, "name": "Label %d" % i, "kind": "owner", "pinned": i < 3} for i in range(12)]
        self.tab, self.grouping, self.filter, self.why = "primary", "none", None, None

    def tabs(self):
        return self.VIEWS + [l["id"] for l in self.labels if l["pinned"]]

    def shown(self):
        return self.tabs()[:len(self.VIEWS) + const("TAB_PINNED_MAX")]

    def click(self, ax):
        tail = ax.rsplit(".", 1)[1]
        if ax.startswith("mac-main.splits.more."):
            if "every-item-of-the-plus-n-menu-opens-its-tab" not in self.m:
                self.tab = ax[len("mac-main.splits.more."):]
        elif ax.startswith("mac-main.splits.pin."):
            self.set_pin(ax[len("mac-main.splits.pin."):], True)
        elif ax.startswith("mac-main.splits.unpin."):
            if "the-extra-pins-are-taken-back-and-the-chip-leaves" not in self.m:
                self.set_pin(ax[len("mac-main.splits.unpin."):], False)
        elif ax == "mac-main.splits.unread":
            if self.filter or "unread-switch-click-turns-the-filter-on" not in self.m:
                self.filter = None if self.filter else "unread"
        elif ax.startswith("mac-main.splits.") and tail.isdigit():
            n = int(tail)
            if not (n >= 3 and "every-tab-segment-opens-its-tab" in self.m):
                self.tab = self.tabs()[n]
        elif ax.startswith("mac-main.list.group."):
            if not (tail == "none" and "group-none-click-ends-the-sections" in self.m):
                self.grouping = tail
        elif ax == SWEEP_WHY:
            if self.why and "why-button-clicked-twice-opens-then-closes" in self.m:
                return {}
            self.why = None if self.why else "msg-1"
        return {}

    def set_pin(self, lid, on):
        for l in self.labels:
            if l["id"] == lid:
                l["pinned"] = on

    def key(self, chord):
        if chord == "Escape":
            self.filter = None
        return {}

    def send(self, verb, *a):
        if verb == "fixture" and a[:1] == ("reset",):
            self.reset()
        return {"ok": True}

    def uistate(self):
        return {"tab": self.tab, "surface": "mac-main"}

    def rows(self, ax):
        if ax == "mac-main.splits":
            shown = self.shown()
            return [{"ax": "mac-main.splits.%d" % i, "id": t, "shown": t in shown} for i, t in enumerate(self.tabs())]
        if ax == "mac-main.list.state":
            return [{"grouping": self.grouping, "filter": self.filter, "why_open": self.why, "plus_n": len(self.tabs()) - len(self.shown()),
                     "unread_chip": {"on": self.filter == "unread"}}]
        if ax == "mac-main.list":
            return [{"ax": "mac-main.list.row.%d" % i, "id": "msg-%d" % (i + 1), "group": "" if self.grouping == "none" else "sender-%d" % i} for i in range(3)]
        if ax == "mac-main.sidebar.labels":
            return [dict(l) for l in self.labels]
        return []


def selftest_sweep():
    """The sweep against the stand-in, clean and once per mutant."""
    words = mutant_words(SWEEP)

    def run(mutant=None):
        j = Journey(ID, SweepCtl([mutant] if mutant else []))
        sweep_step(j)
        return {c.label.split("/", 1)[1]: (c.state, c.detail) for c in j.checks()}
    return report_mutants(run, words)


# J1.60/filter-cache-follows-read-state | the cache stamp holds the row ids alone (ListFilters.passing)

# J1.61 mutants (triage-keys): a stand-in mutant, or a broken copy of the Swift for the last three (source copy)
# J1.61/shift-m-mutes-the-thread-and-it-leaves-the-inbox | shift M does nothing on an unmuted thread
# J1.61/the-muted-view-holds-the-thread | the Muted view lists nothing
# J1.61/the-muted-view-bar-says-unmute-with-its-key | the Muted view's bottom bar has no Unmute line, or says Mute
# J1.61/shift-m-again-unmutes-the-thread | shift M on a muted thread mutes it again
# J1.61/the-unmuted-thread-leaves-the-muted-view | the unmuted thread stays in the Muted view
# J1.61/the-unmuted-thread-is-back-in-the-inbox | the unmuted thread never returns to the inbox
# J1.62/reset-clears-the-unsubscribe-date | `fixture reset` leaves the date a list was left
# J1.62/reset-clears-the-refused-filing | `fixture reset` leaves a refused filing
# J1.61/the-archive-view-holds-the-thread | the Archive view lists nothing
# J1.61/the-archive-view-bar-says-mark-not-done-with-its-key | the Archive view's bottom bar has no Mark not done line
# J1.61/z-undoes-a-mute-and-the-thread-is-back-in-the-inbox | a mute leaves no undo entry
# J1.61/z-undoes-an-unmute-and-the-thread-is-muted-again | an unmute leaves no undo entry
# J1.61/the-mute-keys-capsule-offers-undo | TriageKeys.mute says Muted with undoable: false (source copy)
# J1.61/the-archive-view-bar-line-names-mark-not-done-and-the-preset-key | the Archive bar line says Archive (source copy)
# J1.61/shift-e-returns-the-archived-thread | shift E does nothing on an archived thread
# J1.61/shift-e-says-back-in-the-list | shift E says nothing
# J1.61/the-returned-thread-leaves-the-archive-view | the returned thread stays in the Archive view
# J1.61/the-returned-thread-is-back-in-the-inbox | the returned thread never returns to the inbox
# J1.61/the-mute-key-reads-the-core-mark-and-toggles | TriageKeys.mute always passes muted: true (source copy)
# J1.61/the-muted-view-bar-line-names-unmute-and-the-preset-key | the bar line says Mute (source copy)
# J1.61/shift-e-runs-the-cores-unarchive | shift E runs archive instead of unarchive (source copy)

# J1.55 mutants: per check, the mutation of the app that must turn it red.
# J1.57/header-filter-name-has-no-dead-chevron | ListHeader.swift draws the chevron.down image beside the filter name again
# J1.55/label-click-pinned-opens-its-tab | a pinned label's click moves the highlight and leaves the tab
# J1.55/label-click-unpinned-lists-only-its-messages | the list thins one page of the All view instead of reading the core's rows for the label
# J1.55/label-click-reader-shows-the-first-row | the reader keeps the row the focus index pointed at before the click
# J1.55/label-click-empty-label-says-so-and-clears-the-reader | an empty label shows "All done" and the reader keeps its old message
# J1.55/label-click-ends-with-all-inboxes | All inboxes leaves the label filter on
# J1.55/label-click-reads-the-core-by-label | AppModel.refresh() reads with labelId nil again


# ---------------------------------------------------------------------------------------------------------------------
# --selftest: J1.52 against a stand-in app that models the SPECIFIED tag, clean (every check green) and once per mutant (the named
# check red). The stand-in serves what the socket serves (rows, state, core reads, frames); a mutant breaks exactly one behaviour.
ENTRY_READ = {"chain_id": 0, "matched": [{"kind": "body", "words": "Hi Assem"}]}     # step 1's own read of a picked mail (A140)


class WhyCtl:
    instant = True
    ROW_H, TAG_H, CHAR_W = 44, 16, 6.2
    CORPUS = {
        "needs-reply": [
            # A140: every mail of the example label was picked by step 1 (its body opens "Hi Assem"), accepted by the AI and kept by step 3: the exit card,
            # with the AI's answer and step 1's own read (the row that was an entry card before A140 is "m-entry")
            ("m-entry", "Laura Meier", "Print run quote", {"card": "exit", "chain_id": 0, "decided_by": "jev", "ai": "yes", "entry": ENTRY_READ,
                                                           "matched": [{"kind": "fact", "fact": "unanswered", "wanted": True}]}),
            ("m-exit", "Ton Verheyden", "Re: Clarification points", {"card": "exit", "chain_id": 0, "decided_by": "jev", "ai": "yes", "entry": ENTRY_READ,
                                                                    "matched": [{"kind": "fact", "fact": "unanswered", "wanted": True}]}),
            ("m-flip", "Nina Berg", "Workshop agenda draft", {"card": "exit", "chain_id": 0, "decided_by": "laya", "ai": "yes", "entry": ENTRY_READ,
                                                             "matched": [{"kind": "fact", "fact": "unanswered", "wanted": True}]}),
            ("m-long", "Karim Capital", "Term sheet v2", {"card": "exit", "chain_id": 0, "decided_by": "laya", "ai": "yes", "entry": ENTRY_READ,
                                                         "matched": [{"kind": "from", "words": "laura.meier.longlonglong@nordlicht-example.com"}]}),
        ],
        # a label whose AI card is off: step 1 alone files, so its mail carries the entry card
        "lbl-5": [
            ("m-rule", "Linea Supply", "Quote for the Q2 run", {"card": "entry", "chain_id": 0, "decided_by": "rule:r1",
                                                                "matched": [{"kind": "has_attachment"}, {"kind": "from", "words": "linea-supply-example.com"}]}),
        ],
        # as the real corpus stands when J1.52 runs: Primary holds mail no card placed (no read), the Provanta label holds one row the
        # fixture filed by default (a hand read the case did not seed: the case must not take it for its own)
        "primary": [
            ("m-p1", "Marie Atelier", "Final logo files", None),
            ("m-p2", "Linea Supply", "Quote for the Q1 print run", None),
        ],
        "lbl-6": [
            ("m-prox0", "Old Provanta", "Contract draft", {"card": "hand", "chain_id": None, "decided_by": "default", "matched": []}),
        ],
    }
    TITLES = {"primary": "Primary", "all": "All", "needs-reply": "Needs Reply", "lbl-6": "Provanta", "lbl-5": "Suppliers"}
    VIEWS = ("primary", "all", "needs-reply", "lbl-6", "lbl-5")

    def __init__(self, mutants=()):
        self.m = set(mutants)
        self.rows_by_view = {v: list(r) for v, r in self.CORPUS.items()}
        self.view, self.inspector, self.why, self.width = "needs-reply", False, False, 1280
        self.reads, self.core_reads, self.loads = {}, 0, 0
        self.focus, self.picker, self.typed = None, False, ""

    # -- the app's behaviour
    def read_of(self, mid, view):
        for r in self.rows_by_view.get(view, []):
            if r[0] == mid:
                if "tag-picked-mails-wear-the-exit-card" in self.m and mid == "m-entry":      # the pre-A140 read: step 1 filed it, the AI never ruled
                    return {"card": "entry", "chain_id": 0, "decided_by": "rule:r1", "matched": [{"kind": "body", "words": "Hi Assem"}]}
                return r[3]
        return None

    def refresh(self):
        self.reads = {}
        if not (self.inspector and self.why):
            return
        for mid, _, _, _ in self.rows_by_view[self.view]:
            self.core_reads += 1
            rd = self.read_of(mid, self.view)
            if rd:
                self.reads[mid] = rd
        self.loads += 1

    def rows_changed(self):
        if "tag-gone-after-the-reply-flip" not in self.m:      # mutant: the reads are taken once, when the switch goes on, and never again
            self.refresh()

    def words(self, rd):
        card, first = rd["card"], (held_words(rd["matched"][0]) if rd["matched"] else None)
        if "tag-text-entry-card" in self.m and card == "entry":
            first = held_words(rd["matched"][-1])
        word = {"entry": "Rule", "ai": "✦ AI", "exit": "✦ AI", "fallback": "Fallback", "hand": "By hand"}[card]
        if "tag-text-ai-card" in self.m and card == "ai":
            word = "AI"
        if "tag-text-hand-card" in self.m and card == "hand":
            word = "Rule"
        if "tag-never-says-jev" in self.m and rd["decided_by"] == "jev":
            word = "Jev"
        tail = first if card in ("entry", "exit") else self.TITLES[self.view] if card == "fallback" else None
        if "tag-text-exit-card" in self.m and card == "exit":
            tail = None
        if "tag-text-fallback-card" in self.m and card == "fallback":
            tail = None
        return word if tail is None else "%s · %s" % (word, tail)

    def tag_of(self, mid):
        on = self.inspector and self.why
        rd = self.read_of(mid, self.view) if ("tag-hidden-when-the-switch-is-off" in self.m and not on) else self.reads.get(mid)
        if rd is None:
            return None
        if "tag-shows-on-every-row-the-view-holds" in self.m and mid == self.rows_by_view[self.view][-1][0]:
            return None
        return self.words(rd)

    # -- the socket
    def key(self, chord):
        if chord == "⌘I":
            self.inspector = not self.inspector
            self.refresh()
        elif chord == "J" and "tags-are-read-once-per-load" in self.m and self.inspector and self.why:
            self.refresh()                                       # mutant: a scroll re-reads every row
        elif chord == "L" and self.focus:
            self.picker, self.typed = True, ""                   # the label picker on the focused row
        elif chord == "Enter" and self.picker:
            self.picker = False
            if self.typed == "Provanta" and self.focus:          # the owner files the focused row under Provanta: the hand card
                row = next(r for v in self.rows_by_view.values() for r in v if r[0] == self.focus)
                self.rows_by_view["lbl-6"].append((row[0], row[1], row[2], {"card": "hand", "chain_id": None, "decided_by": "owner", "matched": []}))
                self.rows_changed()
        return {}

    def click(self, ax):
        if ax == WHY_SWITCH and self.inspector:
            self.why = not self.why
            self.refresh()
        elif ax.startswith("mac-main.splits."):
            self.view = self.VIEWS[int(ax.rsplit(".", 1)[1])]
            self.rows_changed()
        elif ax.startswith("mac-main.list.row.") and ax.rsplit(".", 1)[1].isdigit():
            self.focus = self.rows_by_view[self.view][int(ax.rsplit(".", 1)[1])][0]
        return {}

    def send(self, verb, *a):
        if verb == "resize":
            self.width = a[0]
        elif verb == "fixture" and a[0] == "reply":
            mid = a[1]
            row = next((r for r in self.rows_by_view["needs-reply"] if r[0] == mid), None)
            if row is None:
                raise Unreadable("fixture reply %s: no such row" % mid)
            self.rows_by_view["needs-reply"].remove(row)
            self.rows_by_view["primary"].append((mid, row[1], row[2], {"card": "fallback", "chain_id": None, "decided_by": "laya", "matched": []}))
            self.rows_changed()
        elif verb == "fixture" and a[0] == "deliver":
            # as the verb reads its file: `arrivals.<name>` with an id and an account, else the verb refuses in words
            try:
                fields = json.load(open(a[2], encoding="utf-8"))["arrivals"][a[1]]
                mid, _acc = fields["id"], fields["account"]
            except (OSError, ValueError, KeyError) as e:
                raise Unreadable("fixture deliver %s: %s" % (a[1], e))
            # the arrival is an inbox row; its words fit the Provanta description and the label's exit filter is off: filed with the AI card
            self.rows_by_view["primary"].insert(0, (mid, fields.get("sender", ""), fields.get("subject", ""), None))
            self.rows_by_view["lbl-6"].append((mid, fields.get("sender", ""), fields.get("subject", ""), {"card": "ai", "chain_id": None, "decided_by": "jev", "matched": []}))
            self.rows_changed()
        return {"ok": True}

    def core(self, path):
        p = path.split("/")
        rd = self.read_of(p[1], p[3]) if p[0] == "message" and p[2] == "why" else None
        return json.dumps(rd) if rd else None

    def uistate(self):
        return {"tab": self.view, "surface": "mac-main"}

    def rows(self, ax):
        if ax == "mac-main.splits":
            return [{"id": v, "title": self.TITLES[v]} for v in self.VIEWS]
        if ax == "mac-main.list.state":
            return [{"why_tags": {"on": self.inspector and self.why, "scope": self.view if self.reads else None, "shown": len(self.reads),
                                  "core_reads": self.core_reads, "loads": self.loads}}]
        if ax == "mac-main.list":
            return [{"ax": "mac-main.list.row.%d" % i, "id": mid, "why_tag": self.tag_of(mid), "why_card": (self.reads.get(mid) or {}).get("card")}
                    for i, (mid, _, _, _) in enumerate(self.rows_by_view[self.view])]
        return []

    def pane(self):
        return 420 if self.width >= 1280 else 330

    def tag_frame(self, mid):
        text = self.tag_of(mid)
        natural = len(text) * self.CHAR_W + 12
        avail = self.pane() - 120
        card = len(text.split(" · ")[0]) * self.CHAR_W
        w, h = min(natural, avail), self.TAG_H
        if "tag-keeps-its-card-word-when-cut" in self.m and natural > avail:
            w = card - 10                                        # mutant: the card word is cut with the condition
        if "tag-fits-one-line-at-960" in self.m and natural > avail:
            h, w = self.TAG_H * 2, avail + 40                    # mutant: the tag wraps and spills
        return {"x": self.pane() - 8 - w, "y": 0, "w": w, "h": h}, card

    def measure(self, ax):
        if ax == "mac-main.list.view":
            return {"frame": {"x": 0, "y": 52, "w": self.pane(), "h": 600}}
        p = ax.split(".")
        if p[:3] == ["mac-main", "list", "row"] and len(p) >= 5:
            mid, what = p[3], ".".join(p[4:])
            order = [r[0] for r in self.rows_by_view[self.view]]
            if mid not in order:
                raise Unreadable("measure %s: nothing drawn" % ax)
            y = 52 + order.index(mid) * self.ROW_H
            if what == "text":
                bump = 4 if ("tag-moves-no-row" in self.m and self.tag_of(mid) is not None) else 0
                return {"frame": {"x": 48, "y": y + 4 + bump, "w": 100, "h": 18}}
            if what in ("whytag", "whytag.card"):
                if self.tag_of(mid) is None:
                    raise Unreadable("measure %s: nothing drawn" % ax)
                t, card = self.tag_frame(mid)
                t["y"] += y + 24
                return {"frame": t if what == "whytag" else {"x": t["x"] + 6, "y": t["y"], "w": card, "h": 14}}
        raise Unreadable("measure %s: no measurement" % ax)

    def shot(self, name):
        return {}

    def type(self, text):
        if self.picker:
            self.typed += text
        return {}


class CollapseCtl:
    """The stand-in for J1.53: a list of four rows, E archives the focused one, Z puts it back. The probe `collapse_glided` is true when the removal
    ran inside the animated transaction (RowCollapse.run); a mutant breaks exactly one behaviour."""
    instant = True

    def __init__(self, mutants=()):
        self.m = set(mutants)
        self.order = ["m1", "m2", "m3", "m4"]
        self.focus, self.glided, self.gone = 0, False, None

    def key(self, chord):
        if chord == "J":
            self.focus = min(self.focus + 1, len(self.order) - 1)
        elif chord == "E":
            self.gone = (self.focus, self.order[self.focus])
            del self.order[self.focus]
            self.glided = "archive-row-leaves-inside-the-collapse" not in self.m
            if "archive-focus-moves-on-the-key-frame" in self.m:
                self.focus = 0
        elif chord == "Z" and self.gone:
            if "undo-inserts-the-row-back" not in self.m:
                self.order.insert(self.gone[0], self.gone[1])
            self.glided = True
        return {}

    def click(self, ax):
        return {}

    def send(self, verb, *a):
        return {"ok": True}

    def rows(self, ax):
        if ax == "mac-main.list.state":
            return [{"collapse_glided": self.glided}]
        if ax == "mac-main.list":
            return [{"ax": "mac-main.list.row.%d" % i, "id": mid, "focused": i == self.focus} for i, mid in enumerate(self.order)]
        return []

    def uistate(self):
        return {"tab": "primary", "surface": "mac-main"}


class ListR8Ctl:
    """The stand-in for J1.54: four rows (a single unread, an unread thread of 3, a read thread of 2 the owner answered last, a single read) and the
    count line's Unread control. It models the SPECIFIED behaviour; a mutant breaks exactly one thing."""
    instant = True
    ROWS = [("s1", 1, True, False), ("t1", 3, True, False), ("t2", 2, False, True), ("s2", 1, False, False)]

    def __init__(self, mutants=()):
        self.m = set(mutants)
        self.filter, self.key_filter = None, None
        self.focus = "s1"

    def fill(self, r):
        if r[1] <= 1:
            return None
        if "count-pill-fill-follows-unread-read-and-focus" in self.m:
            return "quiet" if r[2] else "accent"         # the focus never recolours the pill, and unread and read swap
        return "onaccent" if r[0] == self.focus else ("accent" if r[2] else "quiet")

    def count_style(self):
        on = self.filter is not None
        if "unread-count-is-secondary-while-off" in self.m:
            return "accent"
        if "unread-count-is-accent-while-on" in self.m:
            return "secondary"
        return "accent" if on else "secondary"

    def shown(self):
        return [r for r in self.ROWS if self.filter is None or r[2]]

    def pill(self, r):
        if "thread-row-shows-its-count-pill" in self.m and r[1] > 1:
            return None
        if "single-message-shows-no-pill" in self.m and r[1] == 1:
            return 1
        return r[1] if r[1] > 1 else None

    def arrow(self, r):
        if "reply-arrow-matches-the-thread-state" in self.m:
            return r[0] == "s2" or (r[3] and r[1] > 5)       # on a single message; the owner's thread loses it
        return r[3] and r[1] > 1

    def toggle(self):
        self.filter = None if self.filter else "unread"

    def key(self, chord):
        if chord == "\u21e7U":
            if "shift-u-and-the-count-line-are-one-switch" in self.m:
                self.key_filter = None if self.key_filter else "unread"   # a switch of its own: the count line never hears of it
            else:
                self.toggle()
        return {}

    def click(self, ax):
        p = ax.split(".")
        if p[:3] == ["mac-main", "list", "row"] and len(p) == 4 and p[3].isdigit() and int(p[3]) < len(self.shown()):
            self.focus = self.shown()[int(p[3])][0]
        if ax == UNREAD_CHIP:
            if "unread-control-turns-the-filter-on-from-the-count-line" in self.m and self.filter is None:
                return {}
            if "unread-control-turns-it-off-from-the-count-line" in self.m and self.filter:
                return {}
            self.toggle()
        return {}

    def send(self, verb, *a):
        self.filter = self.key_filter = None
        return {"ok": True}

    def rows(self, ax):
        if ax == "mac-main.list":
            return [{"ax": "mac-main.list.row.%d" % i, "id": r[0], "unread": r[2], "thread_count": r[1], "count_pill": self.pill(r), "replied_last": self.arrow(r),
                     "focused": r[0] == self.focus, "count_pill_fill": self.fill(r)}
                    for i, r in enumerate(self.shown())]
        if ax == "mac-main.list.state":
            on = self.filter is not None
            tail = None if (not on or "unread-control-turns-the-filter-on-from-the-count-line" in self.m) else "\u00b7 Show all"
            return [{"filter": self.filter, "header_unread": 2,
                     "unread_chip": {"ax": UNREAD_CHIP, "on": on, "key": "\u21e7U", "place": "count-line", "count_words": "2 unread", "show_all": tail, "count_style": self.count_style()}}]
        return []

    def measure(self, ax):
        if ax in ("mac-main.splits", "mac-main.splits.0"):       # the app probes the first tab (whooshAx, SplitTabsBar.swift:79); the row has none
            return {"frame": {"x": 8, "y": 12, "w": 300, "h": 32}}
        if ax == "mac-main.list.header":
            return {"frame": {"x": 0, "y": 96, "w": 420, "h": 28}}
        if ax == "mac-main.list.view":
            return {"frame": {"x": 0, "y": 52, "w": 420, "h": 600}}
        if ax == UNREAD_CHIP:
            if "unread-control-sits-in-the-count-line" in self.m:
                return {"frame": {"x": 330, "y": 14, "w": 80, "h": 28}}        # back at the end of the tab row
            return {"frame": {"x": 330, "y": 96, "w": 80, "h": 28}}
        p = ax.split(".")
        if p[:3] == ["mac-main", "list", "row"] and len(p) == 5:
            r = next((r for r in self.shown() if r[0] == p[3]), None)
            if r and p[4] == "count" and self.pill(r) is not None:
                return {"frame": {"x": 100, "y": 60, "w": 16, "h": 16}}
            if r and p[4] == "replied" and self.arrow(r):
                return {"frame": {"x": 100, "y": 80, "w": 12, "h": 12}}
        raise Unreadable("measure %s: nothing drawn" % ax)

    def uistate(self):
        return {"tab": "primary", "surface": "mac-main"}


class LabelListCtl:
    """The stand-in for J1.55: Primary holds four rows, tab lbl-pin two, labels lbl-a (unpinned, takes the L filing) and lbl-b (unpinned, empty).
    A mutant breaks exactly one behaviour."""
    instant = True
    ROWS = {"primary": ["m1", "m2", "m3", "m4"], "lbl-pin": ["m5", "m6"]}
    LABELS = [{"id": "lbl-pin", "name": "Pin", "raw_name": "Pin", "pinned": True, "kind": "owner"},
              {"id": "lbl-a", "name": "Alpha", "raw_name": "Alpha", "pinned": False, "kind": "owner"},
              {"id": "lbl-b", "name": "Beta", "raw_name": "Beta", "pinned": False, "kind": "owner"}]

    def __init__(self, mutants=()):
        self.m = set(mutants)
        self.tab, self.filter, self.reader, self.typed, self.carries, self.focus = "primary", None, "m1", "", {}, 0

    def base(self):
        return list(self.ROWS.get(self.tab, self.ROWS["primary"]))

    def rows_now(self):
        if self.filter:
            if "label-click-unpinned-lists-only-its-messages" in self.m:
                return [r for r in self.base()[:2] if r in self.carries.get(self.filter, [])] + self.base()[1:2]
            return list(self.carries.get(self.filter, []))
        return self.base()

    def follow(self, skip):
        r = self.rows_now()
        self.focus = 0
        if skip not in self.m:
            self.reader = r[0] if r else None

    def key(self, chord):
        if chord in ("J", "K"):
            self.focus = max(0, min(len(self.rows_now()) - 1, self.focus + (1 if chord == "J" else -1)))
            self.reader = self.rows_now()[self.focus]
        elif chord == "Enter" and self.typed:
            lab = next(l["id"] for l in self.LABELS if l["raw_name"] == self.typed)
            self.carries.setdefault(lab, []).append(self.rows_now()[self.focus])
            self.typed = ""
        return {}

    def type(self, text):
        self.typed = text
        return {}

    def click(self, ax):
        if ax.startswith("mac-main.sidebar.label."):
            lid = ax.rsplit(".", 1)[1]
            if lid == "lbl-pin":
                if "label-click-pinned-opens-its-tab" not in self.m:
                    self.tab = lid
                self.filter = None
                self.follow("label-click-reader-shows-the-first-row")
            else:
                self.filter, self.tab = lid, "all"
                self.follow("label-click-empty-label-says-so-and-clears-the-reader" if lid == "lbl-b" else "label-click-reader-shows-the-first-row")
        elif ax == "mac-main.sidebar.mailbox.all-inboxes":
            self.tab = "primary"
            if "label-click-ends-with-all-inboxes" not in self.m:
                self.filter = None
            self.follow("")
        elif ax.startswith("mac-main.splits."):
            self.tab, self.filter = "primary", None
            self.follow("")
        return {}

    def send(self, verb, *a):
        return {"ok": True}

    def uistate(self):
        return {"tab": self.tab, "surface": "mac-main", "focus_message": (self.rows_now() or [None])[self.focus] if self.rows_now() else None, "focus_index": self.focus}

    def rows(self, ax):
        if ax == "mac-main.sidebar.labels":
            return [dict(l) for l in self.LABELS]
        if ax == "mac-main.splits":
            return [{"id": "primary"}, {"id": "all"}, {"id": "lbl-pin"}]
        if ax == "mac-main.list":
            return [{"ax": "mac-main.list.row.%d" % i, "id": mid} for i, mid in enumerate(self.rows_now())]
        if ax == "mac-main.list.state":
            words = None
            if not self.rows_now() and self.filter:
                words = {"title": "Nothing matches", "line": "Nothing in %s." % next(l["name"] for l in self.LABELS if l["id"] == self.filter)}
                if "label-click-empty-label-says-so-and-clears-the-reader" in self.m:
                    words = {"title": "All done", "line": "Nothing in All."}
            return [{"label_filter": self.filter, "empty_words": words}]
        if ax == "mac-main.reader.shown":
            return [{"shown": self.reader}]
        return []


def mutant_words(step="J1.52"):
    """check -> the mutation named in this file's own MUTANTS block (one source for the table and the selftest)."""
    out = {}
    for line in open(os.path.abspath(__file__), encoding="utf-8"):
        m = re.match(r"#\s+" + re.escape(step) + r"/([\w-]+)\s*\|\s*(.+)", line)
        if m:
            out[m.group(1)] = m.group(2).strip()
    return out


class UndoTimerCtl:
    """The stand-in for J1.58: a fake clock (`advance`), one capsule, the probe's `undo_words` and `undo_timer`. It models the SPECIFIED behaviour;
    a mutant breaks exactly one thing."""
    instant = True
    LIFE = 6.0

    def __init__(self, mutants=()):
        self.m = set(mutants)
        self.t, self.at, self.available, self.kind, self.pend = 0.0, None, True, "archive", None

    def advance(self, secs):
        self.t += secs
        if self.at is not None and self.t - self.at >= self.LIFE and "chip-and-timer-are-gone-at-the-end" not in self.m:
            self.at = None

    def say(self, undoable, kind="archive"):
        restart = "second-action-restarts-the-line" not in self.m or self.at is None
        if restart:
            self.at = self.t
        self.available = undoable
        self.kind = kind

    def key(self, chord):
        if chord == "E":
            self.say(True)
        if chord == "S":
            self.say(False)
        if chord == "#":
            self.say(True, "delete")
        if chord == "\u21e7P":
            self.say(True, "pin")
        if chord in ("H", "L"):
            self.pend = {"H": "snooze", "L": "label"}[chord]
        if chord == "Enter" and self.pend:
            self.say(True, self.pend)
            self.pend = None
        return {}

    def type(self, text):
        return {}

    def click(self, ax):
        if ax == "mac-main.reader.delete":
            self.say(True, "reader-delete")
        return {}

    def send(self, verb, *a):
        self.at = None
        return {"ok": True}

    def timer(self):
        if self.at is None or not self.available and "no-undo-no-line" not in self.m:
            return None
        if "timer-reads-full-after-%s" % self.kind in self.m:
            return None
        if "timer-reads-full-when-the-chip-appears" in self.m:
            return 0.0 if self.t == self.at else min(1.0, (self.t - self.at) / self.LIFE)
        if "timer-has-drained-by-mid-life" in self.m:
            return 1.0
        return max(0.0, 1 - (self.t - self.at) / self.LIFE)

    def rows(self, ax):
        if ax == "mac-main.list":
            return [{"id": "m1", "ax": "mac-main.list.row.0"}]
        if ax == "mac-main.list.state":
            return [{"undo_words": None if self.at is None else {"archive": "Archived", "delete": "Deleted", "pin": "Pinned", "snooze": "Snoozed", "label": "Labelled", "reader-delete": "Deleted"}[self.kind], "undo_available": self.available, "undo_timer": self.timer()}]
        return []

    def uistate(self):
        return {"tab": "primary", "surface": "mac-main"}


def selftest_undo_timer():
    """J1.58 against the stand-in, clean and once per mutant. The two source checks read a broken copy of ListChrome.swift."""
    src0 = undo_timer_sources()
    assert undo_life_ms(src0) > 0, "undoChipMs is gone from InboxListState.swift"
    assert undo_line_faults(src0) == [], "the line's source is not clean: %s" % undo_line_faults(src0)
    mutated = {
        "line-is-one-linear-trim-with-no-second-timer": lambda: dict(src0, chrome=src0["chrome"].replace(".linear(duration:", ".easeOut(duration:", 1)),
        "line-and-expiry-share-one-life": lambda: dict(src0, chrome=src0["chrome"].replace("Double(InboxListState.undoChipMs) / 1000", "6.0 / 1000 * 1000", 1)),
    }
    for name, make in mutated.items():
        assert make() != src0, "mutant %s changes nothing: its anchor text is gone from the source" % name
    words = mutant_words("J1.58")

    def run(mutant=None):
        j = Journey(ID, UndoTimerCtl([mutant] if mutant else []))
        undo_timer_step(j, mutated[mutant]() if mutant in mutated else src0)
        return {c.label.split("/", 1)[1]: (c.state, c.detail) for c in j.checks()}
    return report_mutants(run, words)


# ---------------------------------------------------------------------------------------------------------------------
# J1.63 (lane row-marks, item 0b095f58): the four states of board message-card-r1 (design A) the row did not draw: 13 Muted (the mute mark), 14 VIP
# sender (the VIP chip), 10 Snoozed (the clock and the return words in the time's place), 7 Hovered (the action strip on a row that is NOT the focused
# one). Each case reads the probe field (`rows mac-main.list`: muted, vip, snoozed, hovered; ListProbe.swift) AND the drawn mark (the frame probe of
# `mac-main.list.row.<id>.mute|vip|snooze|act.archive`, InboxRowView.swift), reached through the owner's doors: shift M, the palette's
# "Mark as VIP", H and "tomorrow morning", and the rig's pointer rest (`fixture pointer-rest <row ax>` / `off`, FixtureVerbs.swift).
SNOOZE_WORDS = "back %s"      # the catalogue's English "back %@" (Sources/Resources/Localizable.xcstrings) before the core's "HH:mm"


def mark_ax(r, mark):
    return "mac-main.list.row.%s.%s" % (r["id"], mark)


def plain(v):
    """A core state read as text: the test engine answers a JSON string ("\\"08:00\\""), the socket may hand it decoded."""
    return v.strip('"') if isinstance(v, str) else v


def back_to_inbox(s):
    s.click("mac-main.sidebar.command.go-inbox")
    pause(s)


def row_marks_step(j):
    """J1.63."""
    s = j.step("J1.63", "Row marks (board message-card-r1 states 13, 14, 10, 7): the mute mark, the VIP chip, the snooze words in the time's place, the strip on a hovered row off the focus")
    for _ in range(3):
        s.key("Escape")
    s.raw("fixture", "reset")
    pause(s)
    target = {}

    # 13 Muted: the first row muted with the owner's key; in the Muted view it wears the mute mark, and an inbox row wears none
    grab(target, "mute", lambda: rows(s)[0]["id"])
    s.click("mac-main.list.row.0")
    s.key("⇧M")

    def muted_case():
        s.click("mac-main.sidebar.mailbox.muted")
        pause(s)
        r = need([r for r in rows(s) if r["id"] == target.get("mute")], "muted row in the Muted view")[0]
        seen = (r["muted"], onscreen(s, mark_ax(r, "mute"), wait=0.3))
        back_to_inbox(s)
        first = need(rows(s), "row in the inbox")[0]
        return seen, (first["muted"], gone(s, mark_ax(first, "mute"), wait=0.3))
    s.expect("muted-row-wears-the-mute-mark", muted_case, ((True, True), (False, True)))

    # 14 VIP sender: the focused row's sender marked with the palette line; every row of the VIPs view wears the chip, an inbox row of another sender none
    back_to_inbox(s)
    s.click("mac-main.list.row.0")
    s.key("⌘P")
    s.type("mark as vip")
    s.key("Enter")

    def vip_case():
        s.click("mac-main.sidebar.mailbox.vips")
        pause(s)
        seen = sorted({(r["vip"], onscreen(s, mark_ax(r, "vip"), wait=0.3)) for r in need(rows(s), "row in the VIPs view")})
        back_to_inbox(s)
        first = need(rows(s), "row in the inbox")[0]
        return seen, (first["vip"], gone(s, mark_ax(first, "vip"), wait=0.3))
    s.expect("vip-row-wears-the-vip-chip", vip_case, ([(True, True)], (False, True)))

    # 10 Snoozed: the first row snoozed with H and "tomorrow morning"; in the Snoozed view the row says "back" and the core's return time, drawn in the time's place
    back_to_inbox(s)
    grab(target, "snooze", lambda: rows(s)[0]["id"])
    s.click("mac-main.list.row.0")
    s.key("H")
    s.type("tomorrow morning")
    s.key("Enter")
    pause(s)
    # a second snoozed mail takes the focus in the Snoozed view, so the first is read OFF the focus. main194 (lead, 16 Nov): with one snoozed mail the row is
    # the focused one, and a focused row draws the action strip where the words would be (board message-card-r1 `tail`: the strip first), so the words' frame
    # was rightly absent and the case was red on a correct app.
    grab(target, "snooze2", lambda: rows(s)[0]["id"])
    s.click("mac-main.list.row.0")
    s.key("H")
    s.type("tomorrow morning")
    s.key("Enter")

    def snoozed_case():
        s.click("mac-main.sidebar.mailbox.snoozed")
        pause(s)
        mid, mid2 = target.get("snooze"), target.get("snooze2")
        other = need([r for r in rows(s) if r["id"] == mid2], "second snoozed row in the Snoozed view")[0]
        s.click(other["ax"])
        pause(s)
        r = need([r for r in rows(s) if r["id"] == mid], "snoozed row in the Snoozed view")[0]
        other = need([r for r in rows(s) if r["id"] == mid2], "second snoozed row in the Snoozed view")[0]
        when = plain(core(s, "snooze/%s/return_local_time" % mid))
        if not when:
            raise Unreadable("the core serves no return time for %s" % mid)
        # the focused snoozed row: the strip, and no words; the other one: the words in the time's place
        strip = other["focused"] and onscreen(s, mark_ax(other, "act.archive"), wait=0.6) and gone(s, mark_ax(other, "snooze"), wait=0.3)
        return (r["snoozed"] == SNOOZE_WORDS % when, (not r["focused"]) and onscreen(s, mark_ax(r, "snooze"), wait=0.3), strip), r["snoozed"]
    s.expect("snoozed-row-says-when-it-comes-back", lambda: snoozed_case()[0], (True, True, True))

    # 7 Hovered: the focus on the first row, the pointer rests on the second: that row is hovered and draws the strip; off again, no row is hovered
    back_to_inbox(s)
    s.click("mac-main.list.row.0")
    wait_focus_settled(s)

    def hovered_case():
        s.raw("fixture", "pointer-rest", "mac-main.list.row.1")
        pause(s)
        r = need([r for r in rows(s) if r["ax"] == "mac-main.list.row.1"], "second row of the inbox")[0]
        on = (r["hovered"], r["focused"], onscreen(s, mark_ax(r, "act.archive"), wait=0.6), sum(1 for x in rows(s) if x["hovered"]))
        s.raw("fixture", "pointer-rest", "off")
        pause(s)
        return on, (sum(1 for x in rows(s) if x["hovered"]), gone(s, mark_ax(r, "act.archive"), wait=0.6))
    s.expect("hovered-row-shows-the-strip-off-the-focus", hovered_case, ((True, False, True, 1), (0, True)))
    back_to_inbox(s)


# J1.63 mutants (lane row-marks): per check, the mutation of the app that must turn it red.
# J1.63/muted-row-wears-the-mute-mark | the muted row in the Muted view draws no mute mark
# J1.63/vip-row-wears-the-vip-chip | the rows of the VIPs view draw no VIP chip
# J1.63/snoozed-row-says-when-it-comes-back | the snoozed row draws its received time, not "back" and the core's return time
# J1.63/hovered-row-shows-the-strip-off-the-focus | a pointer rest on a row that is not focused draws no strip (the strip follows the focus only)


class RowMarksCtl:
    """The stand-in for J1.63: seven mails of seven senders, the inbox and the Muted, VIPs and Snoozed views, shift M, the palette's line, H with its typed
    words, and the pointer rest. It models the SPECIFIED behaviour; a mutant breaks exactly one mark."""
    instant = True
    ROWS = [("m1", "a@one.example", "09:10"), ("m2", "b@two.example", "09:05"), ("m3", "c@three.example", "08:50"), ("m4", "d@four.example", "08:40"),
            ("m5", "e@five.example", "08:30"), ("m6", "f@six.example", "08:20"), ("m7", "g@seven.example", "08:10")]
    FRAME = {"frame": {"x": 200, "y": 60, "w": 12, "h": 12}}

    def __init__(self, mutants=()):
        self.m = set(mutants)
        self.reset()

    def reset(self):
        self.box, self.focus, self.rest = None, 0, None
        self.muted, self.vips, self.snoozed = set(), set(), {}
        self.palette, self.pending, self.typed = None, False, ""

    def shown(self):
        if self.box == "muted":
            return [r for r in self.ROWS if r[0] in self.muted]
        if self.box == "vips":
            return [r for r in self.ROWS if r[1] in self.vips]
        if self.box == "snoozed":
            return [r for r in self.ROWS if r[0] in self.snoozed]
        return [r for r in self.ROWS if r[0] not in self.muted and r[1] not in self.vips and r[0] not in self.snoozed]

    def cur(self):
        sh = self.shown()
        return sh[min(self.focus, len(sh) - 1)] if sh else None

    def words(self, mid):
        if mid not in self.snoozed or "snoozed-row-says-when-it-comes-back" in self.m:
            return None
        return SNOOZE_WORDS % self.snoozed[mid]

    def send(self, verb, *a):
        if verb == "fixture" and a[:1] == ("reset",):
            self.reset()
        elif verb == "fixture" and a[:1] == ("pointer-rest",):
            t = a[1]
            if t.startswith("mac-main.list.row."):
                n, sh = int(t.rsplit(".", 1)[1]), self.shown()
                if n >= len(sh):
                    raise Unreadable("no row at %s" % t)
                self.rest = sh[n][0]
            else:
                self.rest = None
        return {"ok": True}

    def click(self, ax):
        if ax.startswith("mac-main.list.row.") and ax.rsplit(".", 1)[1].isdigit():
            self.focus = int(ax.rsplit(".", 1)[1])
        elif ax.startswith("mac-main.sidebar.mailbox."):
            self.box, self.focus = ax.rsplit(".", 1)[1], 0
        elif ax == "mac-main.sidebar.command.go-inbox":
            self.box, self.focus = None, 0
        return {}

    def key(self, chord):
        c = self.cur()
        if chord == "⇧M" and c:
            self.muted.add(c[0])
        elif chord == "⌘P":
            self.palette = ""
        elif chord == "H" and c:
            self.pending, self.typed = True, ""
        elif chord == "Enter":
            if self.palette is not None:
                if self.palette and "mark as vip".startswith(self.palette) and c:
                    self.vips.add(c[1])
                self.palette = None
            elif self.pending:
                if c and self.typed == "tomorrow morning":
                    self.snoozed[c[0]] = "08:00"
                self.pending = False
        elif chord == "Escape":
            self.palette, self.pending = None, False
        return {}

    def type(self, text):
        if self.palette is not None:
            self.palette += text
        elif self.pending:
            self.typed += text
        return {}

    def core(self, path):
        p = path.split("/")
        if p[0] == "snooze" and p[2] == "return_local_time":
            return '"%s"' % self.snoozed[p[1]] if p[1] in self.snoozed else None
        if p[0] == "message" and p[2] == "muted":
            return p[1] in self.muted
        return None

    def rows(self, ax):
        if ax == "mac-main.list":
            return [{"ax": "mac-main.list.row.%d" % i, "id": mid, "sender_address": addr, "time": t, "focused": i == self.focus,
                     "muted": mid in self.muted, "vip": addr in self.vips, "snoozed": self.words(mid), "hovered": mid == self.rest, "rested": mid == self.rest}
                    for i, (mid, addr, t) in enumerate(self.shown())]
        if ax == "mac-main.list.state":
            return [{"mailbox": self.box, "pending": "snooze" if self.pending else None}]
        return []

    def measure(self, ax):
        p = ax.split(".")
        sh = self.shown()
        if p[:3] == ["mac-main", "list", "row"] and len(p) >= 5 and p[3] in [r[0] for r in sh]:
            i = [r[0] for r in sh].index(p[3])
            mid, addr, _ = sh[i]
            kind = ".".join(p[4:])
            drawn = {"mute": mid in self.muted and "muted-row-wears-the-mute-mark" not in self.m,
                     "vip": addr in self.vips and "vip-row-wears-the-vip-chip" not in self.m,
                     "snooze": self.words(mid) is not None and i != self.focus and mid != self.rest,   # the strip takes the words' place on a focused or rested row
                     "act.archive": i == self.focus or (mid == self.rest and "hovered-row-shows-the-strip-off-the-focus" not in self.m)}.get(kind, False)
            if drawn:
                return self.FRAME
        raise Unreadable("measure %s: nothing drawn" % ax)

    def uistate(self):
        return {"tab": "primary", "surface": "mac-main"}


def selftest_row_marks():
    """J1.63 against the stand-in, clean and once per mutant."""
    words = mutant_words("J1.63")

    def run(mutant=None):
        j = Journey(ID, RowMarksCtl([mutant] if mutant else []))
        row_marks_step(j)
        return {c.label.split("/", 1)[1]: (c.state, c.detail) for c in j.checks()}
    return report_mutants(run, words)


def selftest():
    os.environ.pop("WHOOSH_BUNDLE", None); os.environ.pop("WHOOSH_BUILD_COMMIT", None)   # the stand-in is not a build: the feature-commit guard must not read the lead's
    code = 0
    code |= selftest_tag()
    code |= selftest_collapse()
    code |= selftest_list_r8()
    code |= selftest_label_click()
    code |= selftest_header_chevron()
    code |= selftest_undo_timer()
    code |= selftest_doors_list()
    code |= selftest_language_dates()
    code |= selftest_filter_stamp()
    code |= selftest_triage()
    code |= selftest_reset_maps()
    code |= selftest_sweep()
    code |= selftest_row_marks()
    return code


def selftest_collapse():
    """J1.53 against the stand-in, clean and once per mutant. The source case reads a broken copy of the view text."""
    src0 = collapse_sources()
    on_change = next(l for l in src0["view"].splitlines() if ".onChange(of: model.rows)" in l)
    mutated = {"side-reads-leave-the-key-frame": lambda: dict(src0, view=src0["view"].replace(on_change, "        .onChange(of: model.rows) { _, _ in ListFilters.refreshStarred(model); WhyTags.refresh(model) }"))}
    for name, make in mutated.items():
        assert make() != src0, "mutant %s changes nothing: its anchor text is gone from the source" % name
    words = mutant_words("J1.53")

    def run(mutant=None):
        j = Journey(ID, CollapseCtl([mutant] if mutant else []))
        collapse_step(j, mutated[mutant]() if mutant in mutated else src0)
        return {c.label.split("/", 1)[1]: (c.state, c.detail) for c in j.checks()}
    code = report_mutants(run, words)
    return code | selftest_collapse_unreadable(src0)


class UnreadableListCtl(CollapseCtl):
    """J1.53 when the list read fails (a refused or empty read): the main76 crash. Every read of the rows raises Unreadable, as the socket's does."""

    def rows(self, ax):
        raise Unreadable("stand-in: the list read was refused")


def selftest_collapse_unreadable(src0):
    """A failed list read must end the three driven cases NOT JUDGED with the reason (never a crash, never a pass); the source case stays judged."""
    j = Journey(ID, UnreadableListCtl())
    try:
        collapse_step(j, src0)
    except Exception as e:      # the main76 TypeError at the line that indexed None
        print("MUTANTS | collapse-unreadable-list | CRASH %s: %s" % (type(e).__name__, e))
        return 1
    got = {c.label.split("/", 1)[1]: c for c in j.checks()}
    want_unjudged = ["archive-row-leaves-inside-the-collapse", "archive-focus-moves-on-the-key-frame", "undo-inserts-the-row-back"]
    bad = [n for n in want_unjudged if n not in got or got[n].state != "unjudged" or "were not read" not in got[n].detail]
    if got.get("side-reads-leave-the-key-frame") is None or got["side-reads-leave-the-key-frame"].state != "pass":
        bad.append("side-reads-leave-the-key-frame")
    print("MUTANTS | collapse-unreadable-list | %s" % ("3 driven cases NOT JUDGED with the reason, source case judged" if not bad else "WRONG " + ", ".join(bad)))
    return 1 if bad else 0


def selftest_list_r8():
    """J1.54 against the stand-in, clean and once per mutant. The two source checks read a broken copy of the sources."""
    src0 = list_r8_sources()
    mutated = {
        "unread-control-words-come-from-the-catalogue": lambda: dict(src0, count=src0["count"].replace('Words.text("Show all")', '"Show all"')),
        "unread-words-are-in-14-languages": lambda: dict(src0, catalogue={k: v for k, v in src0["catalogue"].items() if k != "Show all"}),
    }
    for name, make in mutated.items():
        assert make()["count"] != src0["count"] or make()["catalogue"] != src0["catalogue"], "mutant %s changes nothing: its anchor text is gone from the source" % name
    words = mutant_words("J1.54")

    def run(mutant=None):
        j = Journey(ID, ListR8Ctl([mutant] if mutant else []))
        list_r8_step(j, mutated[mutant]() if mutant in mutated else src0)
        return {c.label.split("/", 1)[1]: (c.state, c.detail) for c in j.checks()}
    return report_mutants(run, words)


def selftest_label_click():
    """J1.55 against the stand-in, clean and once per mutant. The source case reads a broken copy of AppModel.swift."""
    src0 = label_click_sources()
    mutated = {"label-click-reads-the-core-by-label": lambda: dict(src0, model=src0["model"].replace("labelId: labelId", "labelId: nil"))}
    for name, make in mutated.items():
        assert make() != src0, "mutant %s changes nothing: its anchor text is gone from the source" % name
    words = mutant_words("J1.55")

    def run(mutant=None):
        j = Journey(ID, LabelListCtl([mutant] if mutant else []))
        label_click_step(j, mutated[mutant]() if mutant in mutated else src0)
        return {c.label.split("/", 1)[1]: (c.state, c.detail) for c in j.checks()}
    return report_mutants(run, words)


class NoCtl:
    """A stand-in with no socket: the step J1.57 reads source text only."""
    instant = True

    def send(self, verb, *a):
        return {"ok": True}

    def rows(self, ax):
        return []


def selftest_header_chevron():
    """J1.57 against a stand-in, clean and once with the chevron put back in a copy of ListHeader.swift."""
    src0 = header_chevron_sources()
    anchor = 'Text(ListHeaderWords.filterName(model))'
    assert anchor in src0["header"], "the anchor text is gone from ListHeader.swift"
    mutated = {"header-filter-name-has-no-dead-chevron": lambda: dict(src0, header=src0["header"].replace(
        anchor, 'Image(systemName: "chevron.down"); ' + anchor, 1))}
    words = mutant_words("J1.57")

    def run(mutant=None):
        j = Journey(ID, NoCtl())
        header_chevron_step(j, mutated[mutant]() if mutant in mutated else src0)
        return {c.label.split("/", 1)[1]: (c.state, c.detail) for c in j.checks()}
    return report_mutants(run, words)


class DoorsListCtl:
    """The stand-in for J1.56: seven rows, one unpinned owner label. It models the SPECIFIED behaviour of the controls the step clicks; a mutant breaks
    exactly one of them (the one its check names)."""
    instant = True
    ORDER = ["msg-01", "msg-02", "msg-03", "msg-04", "msg-05", "msg-06", "msg-07"]
    LABELS = [{"id": "lbl-a", "name": "Alpha", "raw_name": "Alpha", "pinned": False, "kind": "owner"}]

    def __init__(self, mutants=()):
        self.m = set(mutants)
        self.reset()

    def reset(self):
        self.order, self.focus, self.sel, self.stack, self.undo = list(self.ORDER), 0, [], [], False
        self.stars, self.filter, self.pending, self.pending_ids, self.foot = set(), None, None, [], "list"
        self.label_filter, self.carries, self.typed, self.typing = None, {}, "", False

    def shown(self):
        if self.label_filter:
            return list(self.carries.get(self.label_filter, []))
        if self.filter == "starred":
            return [r for r in self.order if r in self.stars]
        return list(self.order)

    def cur(self):
        sh = self.shown()
        return sh[self.focus] if sh and self.focus < len(sh) else None

    def key(self, chord):
        sh = self.shown()
        if chord == "E" and self.cur():
            mid = self.cur()
            self.stack.append((mid, self.order.index(mid)))
            self.order.remove(mid)
            self.undo = "capsule-offers-undo-after-an-archive" not in self.m
        elif chord == "S" and self.cur():
            self.stars ^= {self.cur()}
        elif chord == "⇧S":
            if "starred-filter-keeps-the-starred-row" not in self.m:
                self.filter, self.focus = "starred", 0
        elif chord == "X" and self.cur():
            self.sel = [self.cur()]
        elif chord == "⇧J" and self.sel:
            if "bar-is-up-with-two-rows" not in self.m and self.focus + 1 < len(sh):
                self.sel.append(sh[self.focus + 1])
            self.focus = min(len(sh) - 1, self.focus + 1)
        elif chord == "J":
            self.focus = min(len(sh) - 1, self.focus + 1)
        elif chord == "K":
            self.focus = max(0, self.focus - 1)
        elif chord == "L" and self.cur():
            self.typing, self.typed = True, ""
        elif chord == "Enter" and self.typing:
            self.carries.setdefault(next(l["id"] for l in self.LABELS if l["raw_name"] == self.typed), []).append(self.cur())
            self.typing = False
        elif chord == "Escape":
            if self.pending:
                if "escape-leaves-no-picker-and-no-selection" not in self.m:
                    self.pending, self.pending_ids = None, []
            elif self.foot == "composer":
                self.foot = "list"
            elif self.sel:
                self.sel = []
            elif self.filter:
                self.filter = None
        return {}

    def type(self, text):
        self.typed = text
        return {}

    def click(self, ax):
        if ax.startswith("mac-main.list.row.") and ax.endswith(".open"):
            self.focus = int(ax.split(".")[-2])
            if "row-open-door-opens-the-thread-with-its-reply" not in self.m:
                self.foot = "composer"
        elif ax.startswith("mac-main.list.row."):
            self.focus = int(ax.rsplit(".", 1)[1])
        elif ax == "mac-main.list.undo":
            if self.undo and self.stack and "undo-door-puts-the-archived-row-back" not in self.m:
                mid, at = self.stack.pop()
                self.order.insert(at, mid)
        elif ax in ("mac-main.list.selectionbar.snooze", "mac-main.list.selectionbar.label") and self.sel:
            if ax.endswith(".snooze") and "bar-snooze-opens-the-picker-on-both-rows" in self.m:
                return {}
            if ax.endswith(".label") and "bar-label-opens-the-picker-on-both-rows" in self.m:
                return {}
            self.pending, self.pending_ids = ax.rsplit(".", 1)[1], list(self.sel)
        elif ax == "mac-main.list.filter" and "filter-chip-door-clears-the-starred-filter" not in self.m:
            self.filter = None
        elif ax.startswith("mac-main.sidebar.label."):
            if "label-click-leaves-a-label-filter-with-its-row" not in self.m:
                self.label_filter = ax.rsplit(".", 1)[1]
        elif ax == "mac-main.list.labelchip" and "label-chip-door-gives-back-all-inboxes" not in self.m:
            self.label_filter = None
        return {}

    def send(self, verb, *a):
        if verb == "fixture" and a[:1] == ("reset",):
            self.reset()
        return {"ok": True}

    def uistate(self):
        return {"tab": "primary", "surface": "mac-main", "focus_message": self.cur(), "focus_index": self.focus}

    def rows(self, ax):
        if ax == "mac-main.list":
            return [{"ax": "mac-main.list.row.%d" % i, "id": mid, "starred": mid in self.stars} for i, mid in enumerate(self.shown())]
        if ax == "mac-main.list.state":
            return [{"undo_available": self.undo, "pending": self.pending, "pending_ids": list(self.pending_ids), "selected": len(self.sel),
                     "filter": self.filter, "label_filter": self.label_filter}]
        if ax == "mac-main.foot.hints":
            return [{"surface": self.foot}]
        if ax == "mac-main.sidebar.labels":
            return [dict(l) for l in self.LABELS]
        return []


def selftest_doors_list():
    """J1.56 against the stand-in, clean and once per mutant. The door-table check reads a copy of ListClicks.swift with one more door."""
    src0 = doors_sources()
    mutated = {"every-door-of-the-list-is-in-the-control-table":
               lambda: dict(src0, clicks=src0["clicks"].replace('"mac-main.list.command":', '"mac-main.list.zzz": { _ in false },\n            "mac-main.list.command":', 1))}
    for name, make in mutated.items():
        assert make() != src0, "mutant %s changes nothing: its anchor text is gone from the source" % name
    words = mutant_words("J1.56")

    def run(mutant=None):
        j = Journey(ID, DoorsListCtl([mutant] if mutant else []))
        doors_list_step(j, mutated[mutant]() if mutant in mutated else src0)
        return {c.label.split("/", 1)[1]: (c.state, c.detail) for c in j.checks()}
    return report_mutants(run, words)


class LanguageDatesCtl:
    """The stand-in for J1.59: rows dated today, this week and older, drawn in the app's language; a mutant draws them with the English locale."""
    instant = True
    EN = ["09:12", "Mon", "Thu", "4 Nov", "12 Apr", "1 Dec"]
    today = ["09:12", "09:10", "08:55"]

    def __init__(self, mutants=()):
        self.m = set(mutants)
        self.lang, self.seen_other = "en", False

    def click(self, ax):
        return {}

    def send(self, verb, *a):
        if verb == "click" and a and a[0].startswith("mac-settings-general.appearance.language."):
            self.lang = a[0].rsplit(".", 1)[1]
            self.seen_other = self.seen_other or self.lang != "en"
        return {"ok": True}

    def word(self, en):
        lang = self.lang
        if lang == "en" and "dates-come-back-in-english" in self.m and self.seen_other:
            lang = "fr"                                       # mutant: the last foreign language sticks
        if lang == "en" or "dates-follow-the-language-%s" % lang in self.m:
            return en                                         # the English locale (or the mutant)
        return time_in(lang, en) or en.lower() + "~"

    def rows(self, ax):
        if ax == "mac-main.list":
            return [{"ax": "mac-main.list.row.%d" % i, "id": "m%d" % i, "time": self.word(t)} for i, t in enumerate(self.today if "english-list-has-dated-rows" in self.m else self.EN)]
        return []


def selftest_language_dates():
    """J1.59 against the stand-in, clean and once per mutant."""
    words = mutant_words("J1.59")

    def run(mutant=None):
        j = Journey(ID, LanguageDatesCtl([mutant] if mutant else []))
        language_dates_step(j)
        return {c.label.split("/", 1)[1]: (c.state, c.detail) for c in j.checks()}
    return report_mutants(run, words)


def selftest_filter_stamp():
    src0 = filter_stamp_sources()
    mutated = {"filter-cache-follows-read-state": lambda: dict(src0, filter=src0["filter"].replace('$0.unread ? $0.messageId + "\\u{1}u" : $0.messageId', '$0.messageId').replace(", starred: state.starred)", ")"))}
    for name, make in mutated.items():
        assert make() != src0, "mutant %s changes nothing: its anchor text is gone from the source" % name
    words = mutant_words("J1.60")

    def run(mutant=None):
        j = Journey(ID, None)
        filter_stamp_step(j, mutated[mutant]() if mutant in mutated else src0)
        return {c.label.split("/", 1)[1]: (c.state, c.detail) for c in j.checks()}
    return report_mutants(run, words)


def report_mutants(run, words):
    clean = run()
    print_model_cases(clean)
    bad = [n for n in clean if n not in words] + [n for n in words if n not in clean]
    for n in bad:
        print("MUTANTS | %s | names no mutant, or the mutant names no check" % n)
    print("MUTANTS | check | clean | mutant | mutant result")
    for name, (state, detail) in clean.items():
        red = run(name)[name]
        ok_red = red[0] == "fail"
        print("MUTANTS | %s | %s | %s | %s" % (name, "green" if state == "pass" else "RED " + detail, words.get(name, "?"),
                                            "red" if ok_red else "STILL " + red[0].upper()))
        bad += [name] if (state != "pass" or not ok_red) else []
    print("%d checks, %d failed, 0 not judged" % (len(clean), len(bad)))
    return 1 if bad else 0


def selftest_tag():
    src0 = read_sources()
    row_anim = "WhooshMotion.animation(WhooshMotion.hover, reduceMotion: reduceMotion), value: whyTag"
    mutated = {                      # the three source checks: the mutant is a broken copy of the file text, the files stay untouched
        "tag-has-no-bare-string": lambda: dict(src0, words=src0["words"].replace('Words.text("Fallback")', '"Fallback"')),
        "tag-words-are-in-14-languages": lambda: dict(src0, catalogue={k: v for k, v in src0["catalogue"].items() if k != "By hand"}),
        "tag-fades-over-hover-tint-ms": lambda: dict(src0, row=src0["row"].replace(row_anim, ".easeOut(duration: 0.4), value: whyTag")),
    }
    for name, make in mutated.items():           # a mutant that changes nothing is no mutant
        assert make() != src0, "mutant %s changes nothing: its anchor text is gone from the source" % name
    words = mutant_words()

    def run(mutant=None):
        ctl = WhyCtl([mutant] if mutant else [])
        j = Journey(ID, ctl)
        why_tag_step(j, mutated[mutant]() if mutant in mutated else src0)
        return {c.label.split("/", 1)[1]: (c.state, c.detail) for c in j.checks()}
    return report_mutants(run, words)


def main():
    if "--selftest" in sys.argv:
        return selftest()
    path = os.path.abspath(__file__)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    print_cases(r)
    for line in r["fail_list"]:
        sys.stderr.write("FAIL %s\n" % line)
    for line in r["no_mutant"]:
        sys.stderr.write("NO MUTANT %s\n" % line)
    for line in r["stray_mutant"]:
        sys.stderr.write("STRAY MUTANT %s\n" % line)
    for line in r["notes"]:
        sys.stderr.write("NOTE %s\n" % line)
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    ok = r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"]
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
