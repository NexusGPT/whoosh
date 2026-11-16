#!/usr/bin/env python3
"""Parity check: par-sidebar (docs/plan.json). Four parity features of the mac-main sidebar, each with one driven
check of its visible result, plus the owner's two sidebar asks: the Mailboxes picker and the accounts section.

Drives the RUNNING rig copy over the control socket through journeys/lib; expected values come from
docs/design/fixtures/mac-main-sample.json, never from a built screen. Step ids sit under J1.1 (sidebar content)
until Cue adds the parity journey step (plan gap G7).
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/par-sidebar.py'
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0.
"""
import os
import re
import time
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, Journey, print_cases, print_model_cases   # noqa: E402
from lib.fixture import FLOW_EXAMPLE, MSG, FX, ROWS, const, TAB                         # noqa: E402
from lib.ctl import Unreadable                                       # noqa: E402

ID = "par-sidebar"
TITLE = "Sidebar parity: own sidebar, per-address buttons, hide items, light sidebar; inbox picker; accounts menu; label tree; selection motion"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "each sidebar control changes what the sidebar or the list shows, and a second action restores it"
LIST = "mac-main.list"
ACC = {a["id"]: a for a in FX["accounts"]}
FAILING = FX["states"]["sync"]["failing_account"]
SLIDE_MAX = const("SLIDE_MAX_ROWS")
# Round 5 (owner 2026-11-08 09:25): the Mailboxes order, Outbox excepted (it leads only while something waits)
MAILBOX_ORDER = ["All inboxes", "VIPs", "Pinned", "Snoozed", "Drafts", "Sent", "Junk", "Trash", "Archive"]


def label_depth_max():
    """LABEL_DEPTH_MAX from docs/CONSTANTS.md (the owner's "3-5 levels")."""
    return const("LABEL_DEPTH_MAX")


def rollup_holds(rows):
    """Every label's total is its own unread plus its direct children's totals (rows carry own and total)."""
    by = {r["id"]: r for r in rows}
    kids = {}
    for r in rows:
        parent = r.get("parent") or None   # the core's parentId (round 3: ids are the core's, not paths)
        if parent in by:
            kids.setdefault(parent, []).append(r)
    bad = [r["id"] for r in rows if r["has_children"] and r["total"] != r["own"] + sum(k["total"] for k in kids.get(r["id"], []))]
    return bad


# OWNER-ONLY, visible rig (RIG_VISIBLE=1 in a window the lead announces, CONTROL-SOCKET delta 39; also `--step J1.14`): these three cases scan the pixels of
# the real window for the red traffic light. A hidden run never creates them, so it neither judges nor counts them; their mutant rows stay documented below.
VISIBLE = os.environ.get("RIG_VISIBLE") == "1"
VISIBLE_ONLY = ("J1.14/par-toggle-on-the-lights-line-on-screen", "J1.14/par-toggle-on-the-lights-line-on-screen-after-resize",
                "J1.14/par-toggle-on-the-lights-line-on-screen-hidden")


def drive(j):
    s = j.step("J1.1", "Sidebar parity features")
    s.arrive(("click", "dock.whoosh"), "mac-main")
    # the expected words below are the English catalogue values: the check sets the app's language itself, never trusting
    # the rig's (macshell-r8's ms11 run went French and turned 17 rows red on wording alone)
    s.click(LANGUAGE % "en")
    style = lambda: s.ctl.rows("mac-main.sidebar.style")[0]
    accts = lambda: s.ctl.rows("mac-main.sidebar.accounts")
    groups = lambda: {g["name"]: g for g in s.ctl.rows("mac-main.sidebar.groups")}
    unpinned_accounts = lambda: {r["account"] for r in s.ctl.rows(LIST) if not r["pinned"]}

    # per-address rail buttons: one button per address, each at least ACCOUNT_RING_MIN_PX
    s.expect("par-one-button-per-address", lambda: sorted(r["address"] for r in accts()), sorted(a["address"] for a in FX["accounts"]))
    s.expect("par-ring-min-px", lambda: min(r["ring_px"] for r in accts()) >= 20, True)
    # accounts section: four groups, folding Work hides its named items, unfolding restores
    s.expect("par-four-groups", lambda: list(groups()), ["Work", "Personal", "Clients", "Side"])
    s.click("mac-main.sidebar.group.work")
    s.expect("par-fold-work", lambda: groups()["Work"]["open"], False)
    s.click("mac-main.sidebar.group.work")
    s.expect("par-unfold-work", lambda: groups()["Work"]["open"], True)
    s.expect("par-failing-group-dot", lambda: [g for g, v in groups().items() if v["warn"]], [ACC[FAILING]["group"]])
    # one click filters to the account, a second click on it clears
    s.click("mac-main.accounts.acc-06")
    s.expect("par-account-filters", lambda: unpinned_accounts(), {"acc-06"})
    s.click("mac-main.accounts.acc-06")
    s.expect("par-account-click-again-clears", lambda: len(s.ctl.rows(LIST)), len(ROWS))
    s.click("mac-main.accounts.%s" % FAILING)
    s.expect("par-failing-line", lambda: [r.get("line") for r in accts() if r["id"] == FAILING], ["%s needs you to sign in again" % ACC[FAILING]["name"]])
    s.click("mac-main.accounts.all")
    # the Mailboxes picker: tick one inbox filters the list, untick clears
    s.click("mac-main.sidebar.mailboxes.pick")
    s.expect("par-picker-opens", lambda: all(r["open"] for r in s.ctl.rows("mac-main.sidebar.inbox-picker")), True)
    picked = lambda: sorted(r["id"] for r in s.ctl.rows("mac-main.sidebar.inbox-picker") if r["checked"])
    every = sorted(a["id"] for a in FX["accounts"])
    s.expect("par-default-all-checked", lambda: (picked(), s.ctl.rows("mac-main.sidebar.pick-all")[0]["checked"], len(s.ctl.rows(LIST))), (every, True, len(ROWS)))
    s.click("mac-main.sidebar.pick.acc-01")
    s.expect("par-uncheck-one-turns-all-off", lambda: ("acc-01" in picked(), len(picked()), s.ctl.rows("mac-main.sidebar.pick-all")[0]["checked"]), (False, len(every) - 1, False))
    s.expect("par-uncheck-one-chip-counts", lambda: s.ctl.rows("mac-main.sidebar.chip")[0]["label"], "%d inboxes" % (len(every) - 1))
    s.click("mac-main.sidebar.pick.all")
    s.expect("par-all-rechecks-every-account", lambda: (picked(), s.ctl.rows("mac-main.sidebar.pick-all")[0]["checked"], s.ctl.rows("mac-main.sidebar.chip")[0]["label"], len(s.ctl.rows(LIST))),
             (every, True, "All", len(ROWS)))
    s.click("mac-main.accounts.acc-06")
    s.expect("par-one-account-left-checked-filters-the-list", lambda: (picked(), unpinned_accounts()), (["acc-06"], {"acc-06"}))
    s.click("mac-main.sidebar.pick.all")
    s.expect("par-all-after-one-account-restores-the-list", lambda: (len(picked()), len(s.ctl.rows(LIST))), (len(every), len(ROWS)))
    s.click("mac-main.sidebar.mailboxes.pick")
    # hide rail items: hiding Sent removes it from the mailboxes, show all restores
    s.click("mac-main.sidebar.hide.sent")
    s.expect("par-hide-item", lambda: "Sent" in [r["name"] for r in s.ctl.rows("mac-main.sidebar.mailboxes")], False)
    s.click("mac-main.sidebar.show-all")
    s.expect("par-show-all", lambda: [r["name"] for r in s.ctl.rows("mac-main.sidebar.mailboxes")], MAILBOX_ORDER)
    # (the light toggle and the section-up doors had no control: removed in sidebar-r9, finding F-A2)

    # round 2, the accounts menu: one bar at the foot; click, Escape, click outside
    menu = lambda: s.ctl.rows("mac-main.sidebar.accounts-menu")[0]
    s.expect("par-menu-starts-collapsed", lambda: (menu()["open"], menu()["foot_px"]), (False, const("SIDEBAR_FOOT_PX")))
    s.click("mac-main.sidebar.accounts-menu")
    s.expect("par-menu-click-pulls-up", lambda: menu()["open"], True)
    s.expect("par-menu-health", lambda: sorted({r["health"] for r in accts() if r["id"] == FAILING} | {"other" for r in accts() if r["id"] != FAILING and r["health"] == "Needs sign-in"}), ["Needs sign-in"])
    s.click("mac-main.sidebar.accounts-menu.up")
    s.key("Escape")
    s.expect("par-menu-escape-pulls-down", lambda: menu()["open"], False)
    s.click("mac-main.sidebar.accounts-menu.up")
    s.click("mac-main.sidebar.accounts-menu.outside")
    s.expect("par-menu-outside-pulls-down", lambda: menu()["open"], False)
    s.click("mac-main.sidebar.command.accounts")   # the accounts command (⌘J is Gmail and Apple Mail only now)
    s.expect("par-menu-accounts-key", lambda: menu()["open"], True)
    s.click("mac-main.sidebar.accounts-menu.outside")
    # round 4: at 960x600 the pulled-up panel overflows; its edge fades so no mark is sliced
    s.raw("resize", "960", "600")
    s.click("mac-main.sidebar.accounts-menu.up")
    def faded():
        end = time.time() + 2
        while time.time() < end and not menu()["fade_below"]:
            time.sleep(0.2)
        return menu()["fade_below"]
    s.expect("par-menu-overflow-fades", faded, True)
    s.click("mac-main.sidebar.accounts-menu.outside")
    s.raw("resize", "1280", "820")

    # round 2, rule 11: a near selection slides, a far one fades in place
    sel = lambda: s.ctl.rows("mac-main.sidebar.selection")[0]
    s.click("mac-main.sidebar.mailbox.all-inboxes")
    s.click("mac-main.sidebar.mailbox.vips")
    s.expect("par-select-near-slides", lambda: (sel()["motion"], sel()["distance"] <= SLIDE_MAX), ("slide", True))
    lid = lambda path: next(r["id"] for r in s.ctl.rows("mac-main.sidebar.labels") if r["path"] == path)
    s.click("mac-main.sidebar.label.%s" % lid("Finance"))
    s.expect("par-select-far-fades", lambda: (sel()["motion"], sel()["distance"] > SLIDE_MAX), ("fade", True))

    # round 2, labels as a tree: depth to LABEL_DEPTH_MAX, counts rolled up, right expands, left collapses
    labels = lambda: {r["id"]: r for r in s.ctl.rows("mac-main.sidebar.labels")}
    by_path = lambda: {r["path"]: r for r in s.ctl.rows("mac-main.sidebar.labels")}
    # the deepest label that has children (the core's fixture branch, not a name typed here)
    # The deepest label whose children hold unread mail: since contracts delta 13 (11afa8ee) a label's unread is its LIVE rows
    # only (the fixture file's fixed unread is no longer added), so the deepest parent of all (RFP 2026 > Q4) can hold none,
    # and its total then equals its own with the rollup intact. The rollup is shown where there is something to roll up.
    def deep_of():
        rows = by_path()
        best = max((r for r in rows.values() if r["has_children"]
                    and any(c["total"] > 0 for c in rows.values() if c.get("parent") == r["id"])),
                   key=lambda r: r["depth"], default=None)
        if best is None:   # a reset fixture files no mail under an owner label (delta 13: live rows only): judge the rollup on the deepest parent anyway
            best = max((r for r in rows.values() if r["has_children"]), key=lambda r: r["depth"], default=None)
        if best is None:
            raise Unreadable("the fixture has no parent label")   # not judged, with the reason (never a crash)
        return best["path"]

    def rolls_up(path):
        rows = by_path()
        r = rows[path]
        below = sum(c["total"] for c in rows.values() if c.get("parent") == r["id"])
        return r["total"] == r["own"] + below and (below == 0 or r["total"] > r["own"])
    s.expect("par-tree-depth-max", lambda: max(r["depth"] for r in labels().values()) + 1, label_depth_max())
    s.expect("par-tree-rollup", lambda: rollup_holds(list(labels().values())), [])
    s.expect("par-tree-deepest-rolls-up", lambda: rolls_up(deep_of()), True)
    work = lid("Work")
    s.click("mac-main.sidebar.label.%s" % work)
    s.key("Right")
    s.expect("par-key-right-expands", lambda: (by_path()["Work"]["open"], by_path()["Work > Clients"]["visible"]), (True, True))
    s.click("mac-main.sidebar.label.%s.open" % work)
    s.click("mac-main.sidebar.label.%s" % work)
    s.key("Left")
    s.expect("par-key-left-collapses", lambda: (by_path()["Work"]["open"], by_path()["Work > Clients"]["visible"]), (False, False))
    # the rule (lead, round 7): an arrow the sidebar has no use for moves the pane focus (macshell r8 K9)
    pane = lambda: s.ctl.rows("mac-main.shell")[0]["focused_pane"]
    s.click("mac-main.sidebar.label.%s" % work)
    s.key("Right")
    s.key("Right")
    s.expect("par-key-right-on-open-moves-the-pane", lambda: (by_path()["Work"]["open"], pane()), (True, "list"))
    s.click("mac-main.sidebar.label.%s" % work)
    s.key("Left")   # back to the folded tree the later rows start from
    # A label click filters the list (list r7, A107: an unpinned label reads that label's rows, which a reset fixture has none of), and the sidebar
    # now holds the keys. The rows below read and reply on the inbox, so All inboxes gives the list back (main63: the list was empty here, so
    # there was no row to reply to and no unread row to mark).
    s.click("mac-main.sidebar.mailbox.all-inboxes")

    # round 5 (owner 2026-11-08 09:00): the Outbox leads Mailboxes while a send waits and is gone when none does
    mailbox_rows = lambda: s.ctl.rows("mac-main.sidebar.mailboxes")
    outbox_rows = lambda: [r for r in mailbox_rows() if r["ax"] == "mac-main.sidebar.mailbox.outbox"]
    s.expect("par-outbox-absent-when-empty", lambda: (outbox_rows(), mailbox_rows()[0]["ax"]), ([], "mac-main.sidebar.mailbox.all-inboxes"))
    s.click("mac-main.list.row.0")   # the list holds the keys (a label click gave them to the sidebar)
    s.key("Enter")
    s.key("R")
    s.type("A reply that waits in the undo window.")
    s.key("⌘Enter")
    s.expect("par-outbox-first-with-count",
             lambda: (mailbox_rows()[0]["ax"], mailbox_rows()[0]["count"] == int(s.ctl.core("outbox_entry/count"))),
             ("mac-main.sidebar.mailbox.outbox", True))
    s.expect("par-outbox-above-all-inboxes", lambda: [r["ax"] for r in mailbox_rows()][:2],
             ["mac-main.sidebar.mailbox.outbox", "mac-main.sidebar.mailbox.all-inboxes"])
    s.raw("clock", "advance", const("UNDO_SEND_DEFAULT_S") + 1)
    s.expect("par-outbox-gone-when-sent", lambda: outbox_rows(), [])
    # Enter opened the thread (surface mac-reader); Escape returns to the main surface, or every later `type` goes to the reader
    # (round 7 diagnosis: the inspector's name and the rename prompt stayed empty because of this, not because of the app)
    def back_to_main():
        # Escape walks back one layer at a time (the quick reply, the selection, then the thread): at most three presses
        for _ in range(3):
            if s.ctl.uistate()["surface"] == "mac-main":
                break
            s.ctl.key("Escape")
        return s.ctl.uistate()["surface"]
    s.expect("par-outbox-escape-returns-to-main", back_to_main, "mac-main")
    # no blank band: the first header's centre sits on the list column's tab strip (top 12, pill 32: centre 28 below the pane top)
    TAB_STRIP_CENTRE = 12 + 32 / 2   # WhooshMetrics.toolbarTop + glassPillHeight / 2
    def header_centre():
        h = s.ctl.measure("mac-main.sidebar.header.mailboxes")["frame"]
        top = s.ctl.measure("mac-main.sidebar")["frame"]["y"]
        return h["y"] + h["h"] / 2 - top
    s.expect("par-header-on-tab-strip", lambda: abs(header_centre() - TAB_STRIP_CENTRE) <= 1, True)

    # round 5, items 3 to 5: right-click menus per kind, "+" on Labels, every standard mailbox
    names = lambda: [r["name"] for r in mailbox_rows()]
    s.expect("par-mailbox-order", names, MAILBOX_ORDER)
    s.expect("par-mailbox-no-stand-in-count", lambda: [("count" in r) for r in mailbox_rows() if r["name"] in ("Junk", "Trash", "Archive")], [False, False, False])
    menus = lambda: s.ctl.rows("mac-main.sidebar.menus")
    titles = lambda subj: [r["title"] for r in menus() if r["subject"] == subj]
    smart1 = lambda: next(r["subject"] for r in menus() if r["subject"].startswith("smart."))
    s.expect("par-menu-mailbox-items", lambda: titles("mailbox.all-inboxes"), ["Mark all as read", "Hide", "Account info"])
    s.expect("par-menu-smart-items", lambda: titles(smart1()), ["Edit rule…", "Rename", "Delete…"])
    s.expect("par-menu-label-items", lambda: titles("label.%s" % lid("Work")),
             ["New label inside…", "Modify label…", "Rename", "Change colour", "Pin as a tab", "Mark all as read", "Delete…"])
    s.expect("par-menu-header-items", lambda: (titles("header.labels"), titles("header.smart")), (["New label…"], ["New smart folder…"]))
    s.expect("par-menu-missing-doors-named", lambda: [r["ax"] for r in menus() if not r["enabled"] and not r.get("needs")], [])
    s.click("mac-main.sidebar.menu.mailbox.drafts.hide")
    s.expect("par-menu-hide", lambda: "Drafts" in names(), False)
    s.click("mac-main.sidebar.show-all")
    s.click("mac-main.sidebar.menu.mailbox.all-inboxes.account-info")
    s.expect("par-menu-account-info", lambda: s.ctl.rows("mac-main.sidebar.accounts-menu")[0]["open"], True)
    s.click("mac-main.sidebar.accounts-menu.outside")
    unread_rows = lambda: sum(1 for r in s.ctl.rows(LIST) if r["unread"])
    was_unread = unread_rows()
    s.click("mac-main.sidebar.menu.mailbox.all-inboxes.mark-read")
    s.expect("par-menu-mark-all-read", lambda: (was_unread > 0, unread_rows()), (True, 0))
    insp = lambda: s.ctl.rows("mac-labels.inspector")[0]
    s.click("mac-main.sidebar.labels.add")
    s.expect("par-label-plus-opens-inspector", lambda: (insp()["open"], insp()["mode"], insp()["parent"]), (True, "new", None))
    s.click("mac-labels.inspector.cancel")
    s.expect("par-label-inspector-cancels", lambda: insp()["open"], False)
    s.click("mac-main.sidebar.menu.header.labels.new")
    s.expect("par-menu-header-new-opens-inspector", lambda: (insp()["open"], insp()["mode"], insp()["parent"]), (True, "new", None))
    s.click("mac-labels.inspector.cancel")
    label_ids = lambda: {r["id"] for r in s.ctl.rows("mac-main.sidebar.labels")}
    ids_before = {}
    s._act(lambda: ids_before.update(ids=label_ids()))
    s.click("mac-main.sidebar.menu.label.%s.new-inside" % lid("Work"))
    s.expect("par-menu-new-inside-opens-inspector", lambda: (insp()["open"], insp()["mode"], insp()["parent"]), (True, "new", lid("Work")))
    s.type("Zeta")
    s.click("mac-labels.inspector.name")
    s.click("mac-labels.inspector.create")
    # The new label is found by what appeared (an id the sidebar did not list before), not by its name: main63 saw the inspector go to
    # "modify" on lbl-17 while no row was called Zeta, so the name is its own case below and the menu chain never depends on it.
    made = lambda: sorted(label_ids() - ids_before.get("ids", set()))
    parent_of = lambda: [r["parent"] for r in s.ctl.rows("mac-main.sidebar.labels") if r["id"] in made()]
    s.expect("par-menu-new-label-inside", parent_of, [lid("Work")])
    s.expect("par-menu-new-label-takes-the-typed-name", lambda: [r["raw_name"] for r in s.ctl.rows("mac-main.sidebar.labels") if r["id"] in made()], ["Zeta"])
    zeta = lambda: next(iter(made()), "no-zeta")
    s.click("mac-main.sidebar.menu.label.%s.modify" % zeta())
    s.expect("par-menu-modify-opens-inspector", lambda: (insp()["open"], insp()["mode"], insp()["label_id"]), (True, "modify", zeta()))
    s.click("mac-labels.inspector.cancel")
    s.click("mac-main.sidebar.menu.label.%s.rename" % zeta())
    s.type("Eta")
    s.click("mac-main.sidebar.prompt.confirm")
    s.expect("par-menu-rename-label", lambda: [r["raw_name"] for r in s.ctl.rows("mac-main.sidebar.labels") if r["id"] in made()], ["Eta"])
    eta = zeta
    confirm = lambda: s.ctl.rows("mac-main.sidebar.confirm")[0]
    s.click("mac-main.sidebar.menu.label.%s.delete" % eta())
    s.expect("par-menu-delete-asks-first", lambda: (confirm()["open"], confirm()["name"], confirm()["noun"]), (True, "Eta", "label"))
    s.click("mac-main.sidebar.confirm.no")
    s.expect("par-menu-delete-cancel-keeps", lambda: (confirm()["open"], len(made())), (False, 1))
    s.click("mac-main.sidebar.menu.label.%s.delete" % eta())
    s.click("mac-main.sidebar.confirm.yes")
    s.expect("par-menu-delete-removes-label", lambda: len(made()), 0)
    round6(j)
    if VISIBLE:
        step_j114(j)   # the traffic-light cases read a REAL screen: the hidden render draws no light (main63: 3 cases that could neither fail nor pass)
    round7(j)
    round8(j)
    round9(j)
    round11(j)
    round12(j)
    doors_nav(j)


VIEWS = ("primary", "all")
VIEW_PIN_WORDS = {"primary": "Primary is a view of the inbox, not a label, so it cannot be pinned: the views are always the first tabs.",
                  "all": "All is a view of the inbox, not a label, so it cannot be pinned: the views are always the first tabs."}   # core/crates/contracts/src/inbox_views.rs verb(Pin)


def round11(j):
    """Round 11 (docs/PRD.md "A91 shape", owner 2026-11-09 16:30: "just don't make it a Label because it is confusing"): Primary and All are
    views, not labels. The Labels section holds labels only, G then I opens Primary, the All view has its own command, a view is never
    offered a pin, and a refusal reads in the contract's words (field view_not_a_label)."""
    s = j.step("J1.1", "Primary and All are views, not labels")
    s.click("dock.whoosh")
    s.click(LANGUAGE % "en")
    s.raw("fixture", "reset")
    labs = lambda: s.ctl.rows("mac-main.sidebar.labels")
    tab = lambda: s.ctl.uistate().get("tab")
    selected = lambda: s.ctl.rows("mac-main.sidebar.selection")[0]["id"]
    def absent(ax):
        try:
            s.ctl.measure(ax)
            return False
        except Unreadable:
            return True
    # the Labels section: no Primary row, no All row, the pinned labels start at Needs Reply
    s.expect("par-no-primary-row-under-labels", lambda: [r["id"] for r in labs() if r["id"] == "primary" or r["raw_name"] == "Primary"], [])
    s.expect("par-no-all-row-under-labels", lambda: [r["id"] for r in labs() if r["id"] == "all" or r["raw_name"] == "All"], [])
    s.expect("par-labels-start-at-needs-reply", lambda: (labs()[0]["id"], labs()[0]["name"]), ("needs-reply", "Needs Reply"))
    s.expect("par-pinned-labels-start-at-needs-reply", lambda: [r["name"] for r in labs() if r["pinned"]][:3], ["Needs Reply", "Updates", "Promotions"])
    # a pin control never appears for a view
    s.expect("par-pin-never-offered-for-a-view", lambda: [absent("mac-main.sidebar.label.%s.pin" % v) and absent("mac-main.sidebar.label.%s" % v) for v in VIEWS], [True, True])
    # Go to: G then I opens Primary, the All view has its own command, each shows what it opened
    # Primary's row count is read off the Primary tab itself first: round 8 archived, trashed and junked one row each before this
    # round, so the file's count is not the list's (main52: 17 expected, 14 served). G then I must land on the same rows.
    s.click("mac-main.splits.0")
    primary_count = len(s.ctl.rows(LIST))
    primary_ids = {}
    s._act(lambda: primary_ids.update(ids={r["id"] for r in s.ctl.rows(LIST)}))
    s.click("mac-main.sidebar.command.go-all")
    # The All view holds every inbox row, so it holds Primary's rows and no fewer of them. (main55 counted 18 from the file; main63 served 16:
    # the count was the fixture's, not the view's. The view's own rule is core/crates/contracts/src/inbox_views.rs view_contains.)
    s.expect("par-go-all-opens-the-all-view", lambda: (tab(), selected(), primary_ids.get("ids", set()) <= {r["id"] for r in s.ctl.rows(LIST)} and len(s.ctl.rows(LIST)) >= primary_count > 0),
             ("all", "mailbox.all-inboxes", True))
    s.key("G")
    s.key("I")
    s.expect("par-go-i-opens-primary", lambda: (tab(), selected(), len(s.ctl.rows(LIST))), ("primary", "mailbox.all-inboxes", primary_count))
    views = lambda: {r["command"]: r for r in s.ctl.rows("mac-main.sidebar.go-views")}
    s.expect("par-go-views-open-state-follows", lambda: (views()["go-inbox"]["open"], views()["go-all"]["open"]), (True, False))
    s.expect("par-go-i-shows-its-key", lambda: views()["go-inbox"]["key"], "G then I")
    s.expect("par-go-all-key-is-never-a-chord", lambda: sum(views()["go-all"]["key"].count(m) for m in "\u2303\u2325\u21e7\u2318") <= 1, True)
    # a view asked to be pinned: refused in the contract's words, nothing pinned, the failure line says it
    pinned = lambda: sorted(r["name"] for r in labs() if r["pinned"])
    before = pinned() if j.ctl is not None else []
    s.click("mac-main.sidebar.ask.pin-view.primary")
    refusal = lambda: [(r["field"], r["words"]) for r in s.ctl.rows("mac-main.sidebar.refusal")]
    s.expect("par-view-pin-refused-in-words", lambda: refusal(), [("view_not_a_label", VIEW_PIN_WORDS["primary"])])
    s.expect("par-view-pin-refusal-pins-nothing", lambda: pinned() == before and len(before) > 0, True)
    s.expect("par-view-pin-refusal-reaches-the-failure-line", lambda: VIEW_PIN_WORDS["primary"] in str(s.ctl.uistate()), True)
    s.click("mac-main.sidebar.ask.pin-view.all")
    s.expect("par-view-all-pin-refused-in-words", lambda: refusal(), [("view_not_a_label", VIEW_PIN_WORDS["all"])])
    # the views' names follow the app's language
    s.click(LANGUAGE % "fr")
    s.expect("par-view-names-in-french", lambda: [views()[c]["name"] for c in ("go-inbox", "go-all")], [fr("Primary"), fr("All")])
    s.click(LANGUAGE % "en")
    s.raw("fixture", "reset")


def settle(thunk, want, seconds=2.5):
    """A value that lands after an animation: the last read when it matches, else after `seconds`. A match waits out the slide
    (TAB_MORPH_MS twice): the shell's split animation can undo a toggle made while it still runs (docs/lanes/sidebar.md "## Round 6")."""
    end = time.time() + seconds
    v = thunk()
    while v != want and time.time() < end:
        time.sleep(0.1)
        v = thunk()
    if v == want:
        time.sleep(2 * const("TAB_MORPH_MS") / 1000.0)
        v = thunk()
    return v


def external_dy(s, toggle_ax="mac-main.sidebar.toggle"):
    """The toggle's vertical distance from the red traffic light, in points, read from a REAL screen capture of the window (the
    window server's pixels, not the app's own layout numbers): 0 = on the lights' line. The app's frame maths and its own rendering
    agreed with each other and were both wrong once (owner 2026-11-08 12:37), so no number from the app decides this."""
    import subprocess, tempfile, os
    from PIL import Image
    ctl = s.ctl
    if (ctl.uistate().get("hidden") or {}).get("on"):
        return hidden_dy(ctl, toggle_ax)
    tf = ctl.measure(toggle_ax)["frame"]
    path = os.path.join(tempfile.mkdtemp(), "band.png")
    reds = []
    for _ in range(6):   # the rig app is not always allowed to come forward at once: ask again
        ctl.send("activate")
        time.sleep(1.0)
        f = next(w for w in ctl.winframes() if w["title"] == "Whoosh")
        # winframes y counts up from the screen's bottom edge: the window's top is the screen height minus y minus h
        screen_h = int(subprocess.run(["osascript", "-e", 'tell application "Finder" to get bounds of window of desktop'], capture_output=True, text=True).stdout.split(",")[3])
        top = screen_h - f["y"] - f["h"]
        r = subprocess.run(["screencapture", "-x", "-R", "%d,%d,300,110" % (f["x"], top - 20), path], capture_output=True, text=True)
        if r.returncode != 0 or not os.path.exists(path):
            raise Unreadable("screencapture failed: %s" % r.stderr[:80])
        im = Image.open(path).convert("RGB")
        sc = im.width / 300.0
        px = im.load()
        reds = [(x, y) for y in range(im.height) for x in range(int(70 * sc)) if px[x, y][0] > 190 and px[x, y][1] < 120 and px[x, y][2] < 120]
        if len(reds) >= 20:
            break
    if len(reds) < 20:
        raise Unreadable("the traffic lights are not on screen (the window is covered)")
    red_y = sum(y for _, y in reds) / len(reds)
    x0, x1 = int((tf["x"] + 4) * sc), int((tf["x"] + tf["w"] - 4) * sc)
    bg = px[min(int((tf["x"] + tf["w"] + 6) * sc), im.width - 1), int(red_y)]
    rows = [y for y in range(int(red_y - 14 * sc), int(red_y + 14 * sc))   # inside the bar: above is the desktop, below the bar's edge line
            if any(sum(abs(a - b) for a, b in zip(px[x, y], bg)) > 90 for x in range(x0, x1))]
    if not rows:
        raise Unreadable("no toggle pixels near the lights' line")
    return ((min(rows) + max(rows)) / 2.0 - red_y) / sc


def hidden_dy(ctl, toggle_ax):
    """The same distance on a hidden rig, where no window ever reaches the screen (owner 2026-11-08 16:30): the app's own shot of the
    whole window, title band included, with the toggle's frame read in the same main-thread pass and mapped through `mapping`
    (CONTROL-SOCKET deltas 45, 46, 48). The red light and the toggle are still found from PIXELS, never from a layout number."""
    from PIL import Image
    r = ctl.send("shot", "r7-toggle-band", toggle_ax)
    m, tf = r.get("mapping") or {}, (r.get("frames") or {}).get(toggle_ax)
    if not r.get("path") or not tf or not m:
        raise Unreadable("the hidden shot gave no path, mapping or toggle frame: %s" % str(r)[:120])
    sc = float(m["scale"])
    im = Image.open(r["path"]).convert("RGB")
    px = im.load()
    band_h = int(min(im.height, 60 * sc))
    reds = [(x, y) for y in range(band_h) for x in range(int(70 * sc)) if px[x, y][0] > 190 and px[x, y][1] < 120 and px[x, y][2] < 120]
    if len(reds) < 20:
        raise Unreadable("the hidden render draws no traffic light (%d red pixels in the band)" % len(reds))
    red_y = sum(y for _, y in reds) / len(reds)
    x0, x1 = int((tf["x"] + m["x"] + 4) * sc), int((tf["x"] + m["x"] + tf["w"] - 4) * sc)
    bg = px[min(int((tf["x"] + m["x"] + tf["w"] + 6) * sc), im.width - 1), int(red_y)]
    rows = [y for y in range(max(0, int(red_y - 14 * sc)), min(im.height, int(red_y + 14 * sc)))
            if any(sum(abs(a - b) for a, b in zip(px[x, y], bg)) > 90 for x in range(x0, x1))]
    if not rows:
        raise Unreadable("no toggle pixels near the lights' line in the hidden shot")
    return ((min(rows) + max(rows)) / 2.0 - red_y) / sc


def round6_screen(j):
    """Last, because asking the window forward makes it the key window: the app then opens the focused message and marks it read."""
    s = j.step("J1.14", "The toggle is on the lights' line on the real screen")
    s.click("dock.whoosh")
    s.raw("theme", "light")
    time.sleep(1.0)
    s.raw("resize", "1280", "820")
    s.expect("par-toggle-on-the-lights-line-on-screen", lambda: abs(external_dy(s)) <= 2, True)
    s.raw("resize", "960", "632")
    s.expect("par-toggle-on-the-lights-line-on-screen-after-resize", lambda: abs(external_dy(s)) <= 2, True)
    s.raw("resize", "1280", "820")


def step_j114(j):
    """`--step J1.14` (the owner's one visible window, 2026-11-09 09:02: "One window now, about a minute"): only the toggle on the
    lights' line, read from the REAL screen, shown and hidden, then the sidebar shown again. Nothing else runs."""
    round6_screen(j)
    s = j.step("J1.14", "The toggle stays on the lights' line with the sidebar hidden, on the real screen")
    def hidden():
        try:
            return s.ctl.rows("mac-main.sidebar.visibility")[0]["hidden"]
        except Unreadable:
            return None
    if hidden() is False:
        s.click("mac-main.sidebar.toggle.button")
        time.sleep(0.6)
    s.expect("par-toggle-on-the-lights-line-on-screen-hidden", lambda: abs(external_dy(s)) <= 2, True)
    if hidden():
        s.click("mac-main.sidebar.toggle.button")   # back to shown (the run's preferences are restored after it anyway)


def round6(j):
    """Round 6 (owner 2026-11-08 10:25 to 11:14): hide and show, the S-B structure, counts, labels from delta 9, keys shown."""
    s = j.step("J1.14", "Hide and show the sidebar, Apple style")
    vis = lambda: s.ctl.rows("mac-main.sidebar.visibility")[0]
    TOGGLE = "mac-main.sidebar.toggle"   # measured
    BUTTON = "mac-main.sidebar.toggle.button"   # the control's own click (the shell registers the bare id too)
    SIDEBAR_W = const("SIDEBAR_W_PX")
    if j.ctl is not None:   # a remembered state from an earlier run must not decide this run
        if vis()["hidden"]:
            s.click(BUTTON)
            settle(lambda: vis()["pane_w"], SIDEBAR_W)
        for sec in s.ctl.rows("mac-main.sidebar.sections"):
            if sec["collapsed"]:
                s.click(sec["ax"])
        if s.ctl.rows("mac-main.sidebar.all-inboxes")[0]["open"]:
            s.click("mac-main.sidebar.all-inboxes.chevron")
    s.click("mac-main.sidebar.mailbox.all-inboxes")   # the highlight earlier steps left on a label would be the picker's selection
    s.expect("par-toggle-starts-shown", lambda: (vis()["hidden"], vis()["pane_w"]), (False, SIDEBAR_W))
    # lights_dy is read from the rendered pixels of the toggle against the zoom button, so a drawing offset inside its frame shows
    s.expect("par-toggle-beside-lights", lambda: (s.ctl.measure(TOGGLE)["frame"]["x"] > 66, abs(vis()["lights_dy"]) <= 1), (True, True))
    s.raw("resize", "960", "632")
    s.expect("par-toggle-on-the-lights-line-after-resize", lambda: settle(lambda: abs(vis()["lights_dy"]) <= 1, True), True)
    s.raw("resize", "1280", "820")
    s.expect("par-toggle-tooltip-key-shown", lambda: vis()["tooltip"], "Hide sidebar  \u2318\\")
    s.click(BUTTON)
    s.expect("par-toggle-hides", lambda: settle(lambda: (vis()["hidden"], vis()["pane_w"]), (True, 0)), (True, 0))
    s.expect("par-toggle-slide-within-max", lambda: 0 < vis()["slide_ms"] <= const("MOTION_MAX_MS"), True)
    s.expect("par-toggle-remembered", lambda: vis()["remembered"], True)
    s.expect("par-toggle-stays-put-hidden", lambda: (abs(vis()["lights_dy"]) <= 1, s.ctl.measure(TOGGLE)["frame"]["x"] > 66), (True, True))
    s.expect("par-toggle-tooltip-show", lambda: vis()["tooltip"], "Show sidebar  \u2318\\")
    items = lambda: s.ctl.rows("mac-main.sidebar.pick-items")
    s.expect("par-hidden-picker-holds-the-sidebar-items",
             lambda: ([i["title"] for i in items() if i["group"] == "Mailboxes"], sum(1 for i in items() if i["group"] == "Smart folders"),
                      [i["title"] for i in items() if i["selected"]]),
             (MAILBOX_ORDER + ["Starred", "Muted", "All mail"], len(FX["smart_folders"]), ["All inboxes"]))
    s.click(BUTTON)
    s.expect("par-toggle-shows", lambda: settle(lambda: (vis()["hidden"], vis()["pane_w"]), (False, SIDEBAR_W)), (False, SIDEBAR_W))
    s.expect("par-toggle-forgets-when-shown", lambda: vis()["remembered"], False)
    s.click("mac-main.sidebar.command.sidebar")
    s.expect("par-sidebar-command-hides", lambda: settle(lambda: (vis()["hidden"], vis()["pane_w"]), (True, 0)), (True, 0))
    s.click("mac-main.sidebar.command.sidebar")
    s.expect("par-sidebar-command-shows", lambda: settle(lambda: (vis()["hidden"], vis()["pane_w"]), (False, SIDEBAR_W)), (False, SIDEBAR_W))

    # hidden: the list keeps its width and the reader fills to the window edge (owner 2026-11-08 11:58)
    s.click(BUTTON)
    s.expect("par-hidden-list-keeps-its-width", lambda: settle(lambda: vis()["list_w"], const("LIST_W_WIDE_PX")), const("LIST_W_WIDE_PX"))
    s.expect("par-hidden-list-and-reader-fill-the-window", lambda: abs(vis()["list_w"] + vis()["reader_w"] - vis()["window_w"]) <= 2, True)
    s.click(BUTTON)
    settle(lambda: vis()["pane_w"], SIDEBAR_W)
    # Go to: each mailbox command selects its row
    selected = lambda: s.ctl.rows("mac-main.sidebar.selection")[0]["id"]
    def goes():
        out = []
        for cmd in ("go-drafts", "go-sent", "go-done", "go-reminders", "go-spam", "go-trash", "go-inbox"):
            s.ctl.click("mac-main.sidebar.command.%s" % cmd)
            out.append(selected())
        return out
    s.expect("par-go-commands-select-their-mailbox", goes, ["mailbox.drafts", "mailbox.sent", "mailbox.archive", "mailbox.snoozed", "mailbox.junk", "mailbox.trash", "mailbox.all-inboxes"])
    # the pull-up ends with Add account…, which routes to the add-account surface
    menu_rows = lambda: s.ctl.rows("mac-main.sidebar.accounts-menu.rows")
    s.click("mac-main.sidebar.accounts-menu.up")
    s.expect("par-add-account-row-is-last", lambda: (menu_rows()[-1]["title"], len(menu_rows())), ("Add account\u2026", len(FX["accounts"]) + 2))
    s.click("mac-main.sidebar.accounts-menu.add")
    s.expect("par-add-account-routes-to-its-surface", lambda: (s.ctl.uistate()["surface"], s.ctl.rows("mac-main.sidebar.accounts-menu")[0]["open"]), ("mac-add-account", False))
    s.click("dock.whoosh")

    s = j.step("J1.15", "All inboxes per account, folding sections, one account filter")
    inbox = lambda: s.ctl.rows("mac-main.sidebar.inbox-rows")
    chip = lambda: s.ctl.rows("mac-main.sidebar.chip")[0]["label"]
    ticked = lambda: sorted(r["id"] for r in s.ctl.rows("mac-main.sidebar.inbox-picker") if r["checked"])
    EVERY = sorted(a["id"] for a in FX["accounts"])
    listed = lambda: {r["account"] for r in s.ctl.rows(LIST) if not r["pinned"]}
    s.click("mac-main.sidebar.all-inboxes.chevron")
    s.expect("par-all-inboxes-unfolds-one-row-per-account", lambda: ([r["visible"] for r in inbox()].count(True), len(inbox())), (len(FX["accounts"]), len(FX["accounts"])))
    s.expect("par-inbox-rows-under-group-captions", lambda: [g["name"] for g in s.ctl.rows("mac-main.sidebar.inbox-groups")], ["Work", "Personal", "Clients", "Side"])
    s.expect("par-inbox-row-count-is-the-accounts", lambda: sorted((r["id"], r["count"]) for r in inbox()), sorted((a["id"], a["unread"]) for a in FX["accounts"]))
    s.click("mac-main.sidebar.inbox.acc-06")
    s.expect("par-inbox-row-filters-the-list", lambda: listed(), {"acc-06"})
    s.expect("par-filter-chip-names-the-account", chip, ACC["acc-06"]["name"])
    s.expect("par-filter-picker-ticks-the-account", ticked, ["acc-06"])   # one account filtering = only that account checked
    s.click("mac-main.accounts.acc-01")
    s.expect("par-filter-accounts-bar-moves-all-three", lambda: (chip(), ticked(), sorted(r["id"] for r in inbox() if r["filtered"]), listed() <= {"acc-01"}),
             (ACC["acc-01"]["name"], ["acc-01"], ["acc-01"], True))
    s.click("mac-main.sidebar.inbox.acc-01")
    s.expect("par-inbox-row-click-again-clears", lambda: (chip(), ticked(), len(s.ctl.rows(LIST))), ("All", EVERY, len(ROWS)))
    s.click("mac-main.sidebar.pick.acc-01")
    s.expect("par-filter-picker-moves-the-chip", lambda: (chip(), "acc-01" in ticked()), ("%d inboxes" % (len(EVERY) - 1), False))
    s.click("mac-main.sidebar.pick.all")
    s.click("mac-main.accounts.acc-01")
    s.click("mac-main.sidebar.mailbox.all-inboxes")
    s.expect("par-all-inboxes-row-clears-the-filter", lambda: (chip(), ticked()), ("All", EVERY))
    s.click("mac-main.sidebar.all-inboxes.chevron")
    s.expect("par-all-inboxes-folds", lambda: [r["visible"] for r in inbox()].count(True), 0)
    s.expect("par-all-inboxes-remembered", lambda: s.ctl.rows("mac-main.sidebar.all-inboxes")[0]["saved"], False)
    # sections fold from their chevron: the rows go, the header stays, the state is saved
    secs = lambda: {r["id"]: r for r in s.ctl.rows("mac-main.sidebar.sections")}
    def gone(ax):
        c = s.ctl
        try:
            c.measure(ax)
            return False
        except Unreadable:
            return True
    first = {"mailboxes": "mac-main.sidebar.mailbox.vips", "smart-folders": "mac-main.sidebar.smart.%s" % smart1_id(s), "labels": "mac-main.sidebar.label.needs-reply"}   # round 11: Primary is a view, so the first label is Needs Reply
    for sec, ax in first.items():
        s.click("mac-main.sidebar.section.%s.chevron" % sec)
        s.expect("par-section-%s-folds" % sec, lambda sec=sec, ax=ax: (secs()[sec]["collapsed"], settle(lambda: gone(ax), True)), (True, True))
        s.expect("par-section-%s-saved" % sec, lambda sec=sec: secs()[sec]["saved"], True)
        s.click("mac-main.sidebar.section.%s.chevron" % sec)
        s.expect("par-section-%s-unfolds" % sec, lambda sec=sec, ax=ax: (secs()[sec]["collapsed"], settle(lambda: gone(ax), False)), (False, False))

    s = j.step("J1.1", "Counts, pinned and account labels, drag, keys shown")
    mb = lambda: {r["name"]: r for r in s.ctl.rows("mac-main.sidebar.mailboxes")}
    s.expect("par-count-zero-shows-none", lambda: [("count" in mb()[n]) for n in ("Junk", "Trash", "Archive")], [False] * 3)
    def counts_not_as_expected():
        """Every mailbox the fixture gives a count shows it; All inboxes shows the All tab's unread (the inbox's own figure, which moves with the mail)."""
        every = s.ctl.uistate()["counts"].get("all")
        return [(m["name"], mb()[m["name"]].get("count")) for m in FX["mailboxes"] if m.get("count") and mb()[m["name"]].get("count") != (every if m["name"] == "All inboxes" else m["count"])]
    s.expect("par-count-nonzero-shows", counts_not_as_expected, [])
    lab = lambda: {r["id"]: r for r in s.ctl.rows("mac-main.sidebar.labels")}
    pinned_expected = {TAB[t]["name"] for t in ("needs-reply", "updates", "promotions")} | {"Provanta"}   # round 11: Primary is a view, not a pinned label
    s.expect("par-pin-glyph-on-pinned-labels", lambda: {r["name"] for r in lab().values() if r["pinned"]}, pinned_expected)
    s.click("mac-main.sidebar.label.lbl-1.pin")
    s.expect("par-pin-click-pins", lambda: lab()["lbl-1"]["pinned"], True)
    s.expect("par-pin-menu-says-unpin", lambda: [r["title"] for r in s.ctl.rows("mac-main.sidebar.menus") if r["ax"] == "mac-main.sidebar.menu.label.lbl-1.unpin"], ["Unpin tab"])
    s.click("mac-main.sidebar.menu.label.lbl-1.unpin")
    s.expect("par-pin-menu-unpins", lambda: lab()["lbl-1"]["pinned"], False)
    s.click("mac-main.sidebar.menu.label.lbl-1.pin")
    s.expect("par-pin-menu-pins", lambda: lab()["lbl-1"]["pinned"], True)
    s.click("mac-main.sidebar.label.lbl-1.pin")
    s.expect("par-pin-click-unpins", lambda: lab()["lbl-1"]["pinned"], False)
    s.expect("par-pin-hover-shows-the-pin", lambda: (s.ctl.click("mac-main.sidebar.label.lbl-14.hover"), gone("mac-main.sidebar.label.lbl-14.pin"))[1], False)
    s.click("mac-main.sidebar.label.lbl-14.hover")
    # an account's own label: its mark, and only under that account's filter; a shared label always
    SCOPED = next(l for l in FX["labels"] if l["name"] == "Finance")["children"][0]
    s.expect("par-scoped-label-carries-its-account", lambda: (lab()["lbl-15"]["scope"], lab()["lbl-15"]["mark"]), ("account", SCOPED["account"]))
    s.click("mac-main.sidebar.label.lbl-14.toggle")   # the parent open, so only the scope can hide the account's label
    s.expect("par-scoped-label-hidden-without-its-filter", lambda: (lab()["lbl-14"]["open"], lab()["lbl-14"]["visible"], lab()["lbl-15"]["visible"]), (True, True, False))
    s.click("mac-main.accounts.%s" % SCOPED["account"])
    s.expect("par-scoped-label-shows-under-its-filter", lambda: (lab()["lbl-15"]["visible"], lab()["lbl-16"]["visible"]), (True, True))
    s.click("mac-main.accounts.%s" % "acc-06")
    s.expect("par-scoped-label-hidden-under-another-account", lambda: (lab()["lbl-15"]["visible"], lab()["lbl-16"]["visible"]), (False, True))
    s.click("mac-main.accounts.all")
    s.click("mac-main.sidebar.label.lbl-14.toggle")
    # drag: into a label, before a label, refusal in words past the depth limit, a drag held mid-way
    parent = lambda i: lab()[i]["parent"]
    if j.ctl is not None and lab()["lbl-1"]["open"]:   # earlier steps left Work open
        s.click("mac-main.sidebar.label.lbl-1.toggle")
    L = "mac-main.sidebar.label."
    tops = lambda: [r["name"] for r in lab().values() if r["depth"] == 0]
    s.click(L + "lbl-11.lift")
    s.expect("par-drag-held-then-outlined", lambda: (s.ctl.rows("mac-main.sidebar.drag")[0]["held"], s.ctl.rows("mac-main.sidebar.drag")[0]["id"]), (True, "lbl-11"))
    s.click(L + "lbl-14.over-into")
    s.expect("par-drag-over-names-the-drop", lambda: (s.ctl.rows("mac-main.sidebar.drag")[0]["target"], s.ctl.rows("mac-main.sidebar.drag")[0]["zone"]), ("lbl-14", "into"))
    s.click("mac-main.sidebar.drag.cancel")
    s.expect("par-drag-cancel-drops-nothing", lambda: (s.ctl.rows("mac-main.sidebar.drag")[0]["active"], parent("lbl-11")), (False, ""))
    s.click(L + "lbl-11.lift")
    s.click(L + "lbl-14.drop-into")
    s.expect("par-drag-into-reparents", lambda: (parent("lbl-11"), s.ctl.rows("mac-main.sidebar.drag")[0]["active"]), ("lbl-14", False))
    s.click(L + "lbl-14.lift")
    s.click(L + "lbl-1.drop-before")
    s.expect("par-drag-before-reorders-the-core", lambda: tops().index("Finance") < tops().index("Work"), True)
    s.click(L + "lbl-14.lift")
    s.click(L + "lbl-1.drop-after")
    s.expect("par-drag-after-reorders-the-core", lambda: tops().index("Work") + 1 == tops().index("Finance"), True)
    s.click(L + "lbl-1.lift")
    s.click(L + "lbl-12.drop-into")
    s.expect("par-drag-past-depth-limit-refused", lambda: (parent("lbl-1"), "levels deep" in str(s.ctl.uistate())), ("", True))
    # keys shown: a control with a bound key shows it in its tooltip
    s.expect("par-keys-shown-accounts-bar", lambda: s.ctl.rows("mac-main.sidebar.accounts-menu")[0]["tooltip"].endswith("\u2318J"), True)
    # smart folders: rename and delete reach the core
    smart = lambda: {r["ax"].split(".")[-1]: r["name"] for r in s.ctl.rows("mac-main.sidebar.smart-folders")}
    sid = smart1_id(s)
    s.click("mac-main.sidebar.menu.smart.%s.rename" % sid)
    s.type("Trips")
    s.click("mac-main.sidebar.prompt.confirm")
    s.expect("par-smart-rename-reaches-the-core", lambda: smart().get(sid), "Trips")
    s.click("mac-main.sidebar.menu.smart.%s.delete" % sid)
    s.click("mac-main.sidebar.confirm.yes")
    s.expect("par-smart-delete-removes-it", lambda: sid in smart(), False)
    s.raw("fixture", "reset")


LANGUAGE = "mac-settings-general.appearance.language.%s"
GO_KEYS = [("I", "mailbox.all-inboxes"), ("S", "mailbox.starred"), ("D", "mailbox.drafts"), ("T", "mailbox.sent"), ("E", "mailbox.archive"),
           ("H", "mailbox.snoozed"), ("M", "mailbox.muted"), ("\u21e7!", "mailbox.junk"), ("\u21e7#", "mailbox.trash"), ("A", "mailbox.all-mail")]
CATALOGUE_PATH = os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings")


def fr(key):
    import json
    if not isinstance(key, str):      # a dry run's drive body hands over a stand-in key (journeys/lib/harness.py Dry): the case is not judged anyway
        return "dry"
    with open(CATALOGUE_PATH, encoding="utf-8") as f:
        return json.load(f)["strings"][key]["localizations"]["fr"]["stringUnit"]["value"]


def round7(j):
    """Round 7: the hidden rig answers menus and confirms through the socket; G then a key reaches every view."""
    s = j.step("J1.1", "Go-to keys, built-in views, catalogue names, hidden-rig asks")
    selected = lambda: s.ctl.rows("mac-main.sidebar.selection")[0]["id"]
    s.expect("par-views-built-in", lambda: [r["name"] for r in s.ctl.rows("mac-main.sidebar.views")], ["Starred", "Muted", "All mail"])

    def go(second):
        s.ctl.key("G")
        s.ctl.key(second)
        return selected()
    for key, want in GO_KEYS:
        s.expect("par-go-key-%s" % want.split(".")[1], lambda key=key: go(key), want)
    s.key("G")
    s.key("L")
    s.expect("par-go-key-label", lambda: (selected().startswith("label."), s.ctl.rows("mac-main.sidebar.style")[0].get("sections", [""])[0] != ""), (True, True))
    notice = lambda: [r["text"] for r in s.ctl.rows("mac-shortcuts.notice")]
    for key, name in (("O", "other"), (";", "snippets")):
        s.key("G")
        s.key(key)
        s.expect("par-go-key-%s-coming-soon" % name, lambda: any("coming soon" in t.lower() for t in notice()), True)
    # built-in names follow the app's language (no English literal in a French UI)
    s.click(LANGUAGE % "fr")
    s.expect("par-views-in-french", lambda: [r["title"] for r in s.ctl.rows("mac-main.sidebar.views")], [fr("Starred"), fr("Muted"), fr("All mail")])
    s.expect("par-mailboxes-in-french", lambda: [r["title"] for r in s.ctl.rows("mac-main.sidebar.mailboxes") if r["name"] in ("Snoozed", "Trash")], [fr("Snoozed"), fr("Trash")])
    builtin = lambda: [(r["raw_name"], r["name"]) for r in s.ctl.rows("mac-main.sidebar.labels") if r.get("kind") == "built-in"]
    s.expect("par-builtin-labels-in-french", lambda: [n for _, n in builtin()], [fr(raw) for raw, _ in builtin()])
    s.click(LANGUAGE % "en")
    # hidden rig: a delete ask is STATE the socket reads and answers; no alert shows, the ask is recorded instead
    alerts = lambda: int(s.ctl.core("system-ui/alert") or 0)
    try:
        before = alerts()
        sid = smart1_id(s)
        name = next(r["name"] for r in s.ctl.rows("mac-main.sidebar.smart-folders") if r["ax"].endswith(sid))
    except Unreadable:
        before, sid, name = 0, "smart-1", "?"
    s.click("mac-main.sidebar.menu.smart.%s.delete" % sid)
    confirm = lambda: s.ctl.rows("mac-main.sidebar.confirm")[0]
    s.expect("par-hidden-confirm-is-state", lambda: (confirm()["open"], confirm()["name"], confirm()["noun"]), (True, name, "smart folder"))
    s.expect("par-hidden-confirm-recorded-not-shown", lambda: alerts() > before, True)
    s.click("mac-main.sidebar.confirm.no")
    s.expect("par-hidden-confirm-no-keeps", lambda: (confirm()["open"], sid in [r["ax"].split(".")[-1] for r in s.ctl.rows("mac-main.sidebar.smart-folders")]), (False, True))
    s.raw("fixture", "reset")


# the core's own rule for each message-backed mailbox (FixtureLabels.mailboxRowsLocked), over every fixture message's stored state
MAILBOX_RULES = {
    "starred": lambda st: st["starred"] and not st["deleted"] and not st["spam"],
    "muted": lambda st: st["muted"] and not st["deleted"] and not st["spam"],
    "archive": lambda st: st["archived"] and not st["deleted"] and not st["spam"],
    "trash": lambda st: st["deleted"],
    "junk": lambda st: st["spam"] and not st["deleted"],
    "snoozed": lambda st: st["snoozed"] and not st["deleted"] and not st["archived"],
    "all-mail": lambda st: not st["deleted"] and not st["spam"],
}


def round8(j):
    """Round 8: the list shows what the sidebar selects (AppModel.mailbox, written by the row click and the G keys)."""
    s = j.step("J1.1", "The list follows the selected mailbox")
    s.click("dock.whoosh")
    ids = lambda: [r["id"] for r in s.ctl.rows(LIST)]
    # put one message in each acted mailbox, with the list's own keys on the focused row
    for key in ("S", "\u21e7M", "E", "\u21e7#", "\u21e7!"):
        s.key(key)
    def stored(mid):
        return {k: bool(s.ctl.core("message/%s/%s" % (mid, k))) for k in ("starred", "muted", "archived", "deleted", "spam", "snoozed")}
    # every stored message the list can serve: the file's rows plus the flow's entry and kept rows (they live in the Needs Reply
    # label, so they are not Primary rows, but All mail holds them; main52: 4 flow rows in the list, none in the expectation)
    stored_ids = [m["id"] for m in ROWS] + [r["id"] for r in FLOW_EXAMPLE if r["id"] not in MSG]
    def expected(slug):
        return sorted(i for i in stored_ids if MAILBOX_RULES[slug](stored(i)))
    for slug in ("starred", "muted", "archive", "trash", "junk", "snoozed", "all-mail"):
        def same(slug=slug):
            want = expected(slug)   # read before the click: the core's state, not the list
            s.ctl.click("mac-main.sidebar.mailbox.%s" % slug)
            got = sorted(ids())
            if slug == "all-mail":
                # contracts delta 15: a sent reply's copy (`sent-<outboxId>`, the core's own naming) joins its thread. All mail
                # holds every message not deleted or junk, sent copies included; the file's rows cannot name them, so each sent
                # copy the list shows is judged by the same rule on its stored state.
                want = sorted(want + [i for i in got if i.startswith("sent-") and MAILBOX_RULES[slug](stored(i))])
            return "same rows" if got == want else "list %s, core %s" % (got, want)
        s.expect("par-list-shows-%s" % slug, same, "same rows")
    # Drafts and Sent hold drafts and sent entries, not inbox messages, and no socket verb reads their rows: no inbox message in
    # the list, and the list's own state names the mailbox. (The fixture core counts 3 drafts but serves 0 draft rows: a core
    # data mismatch, reported, not asserted here.)
    for slug in ("drafts", "sent"):
        def leaves_inbox(slug=slug):
            s.ctl.click("mac-main.sidebar.mailbox.%s" % slug)
            return (sorted(set(ids()) & set(m["id"] for m in ROWS)), s.ctl.rows("mac-main.list.state")[0].get("mailbox"))
        s.expect("par-list-shows-%s" % slug, leaves_inbox, ([], slug))
    s.click("mac-main.sidebar.mailbox.all-inboxes")
    s.expect("par-all-inboxes-brings-the-inbox-back", lambda: (s.ctl.uistate().get("tab"), ids()[:1] != [] and set(ids()) <= set(m["id"] for m in ROWS)), ("primary", True))
    s.raw("fixture", "reset")


def round9(j):
    """Round 9: a saved smart folder opens its results in the list (carrier: InboxQuery.smart_folder_id + AppModel.smartFolderId,
    the same shape as the mailbox). Rows and count both come from the core's query; the core's own answer is
    `core state smart_folder/<id>/threads` (the message ids the saved rule matches), read before the click."""
    s = j.step("J1.1", "A smart folder opens its results in the list")
    s.click("dock.whoosh")
    ids = lambda: [r["id"] for r in s.ctl.rows(LIST)]
    state = lambda: s.ctl.rows("mac-main.list.state")[0]
    folder = lambda: s.ctl.rows("mac-main.sidebar.smart-folders")[0]
    def threads(fid):
        v = s.ctl.core("smart_folder/%s/threads" % fid)
        if v is None:
            raise Unreadable("the core answers nothing at smart_folder/%s/threads (contracts gap)" % fid)
        return sorted(v)
    def opens():
        f = folder(); fid = f["ax"].split(".")[-1]
        want = threads(fid)
        s.ctl.click(f["ax"])
        return "same rows" if sorted(ids()) == want else "list %s, core %s" % (sorted(ids()), want)
    s.expect("par-smart-folder-opens-its-results", opens, "same rows")
    s.expect("par-smart-folder-header-names-it",
             lambda: (state().get("smart_folder") == folder()["ax"].split(".")[-1], folder()["name"] in (state().get("header_line") or "")), (True, True))
    def count_follows():
        before = folder().get("count")
        s.ctl.key("E")   # archive the focused result
        f = folder(); fid = f["ax"].split(".")[-1]
        return (f.get("count") or 0, len(ids())) == (len(threads(fid)), len(threads(fid))) and before != f.get("count")
    s.expect("par-smart-folder-count-follows-an-archive", count_follows, True)
    s.click("mac-main.sidebar.mailbox.all-inboxes")
    s.expect("par-smart-folder-left-for-the-inbox", lambda: (state().get("smart_folder"), state().get("mailbox")), (None, None))
    s.raw("fixture", "reset")


def smart1_id(s):
    try:
        return next(r["ax"].split(".")[-1] for r in s.ctl.rows("mac-main.sidebar.smart-folders"))
    except Unreadable:
        return "smart-1"


def reveal_label(s, lid):
    """Unfold every ancestor of a label so its row is drawn: the click door refuses a row the sidebar does not draw
    (SidebarTargets.swift: `guard st.labelVisible(l)`), and a refused click is only a note in the run, never a failure."""
    try:
        byid = {r["id"]: r for r in s.ctl.rows("mac-main.sidebar.labels")}
    except Unreadable:
        return
    chain, p = [], byid.get(lid, {}).get("parent")
    while p and p in byid and p not in chain:
        chain.append(p)
        p = byid[p].get("parent")
    for a in reversed(chain):
        if not byid[a].get("open", True):
            s.click("mac-main.sidebar.label.%s.open" % a)


def round12(j):
    """Round 12 (list r7, docs/PRD.md A107, owner 2026-11-10: "when clicking on a label I should have the 2 other window adjusting (right
    filter on window 2 and right messages)"): a click on a label in the sidebar drives the other two panes. A pinned label selects its tab,
    an unpinned one filters the list by it, and the reader shows the first row of the new list."""
    s = j.step("J1.1", "A label click selects its tab or filters the list, and the reader follows")
    s.click("dock.whoosh")
    s.raw("fixture", "reset")
    labels = lambda: s.ctl.rows("mac-main.sidebar.labels")
    ids = lambda: [r["id"] for r in s.ctl.rows(LIST)]
    state = lambda: s.ctl.rows("mac-main.list.state")[0]
    shown = lambda: s.ctl.rows("mac-main.reader.shown")[0]["shown"]
    settle = lambda: None if getattr(s.j.ctl, "instant", False) else time.sleep(const("FOCUS_SLIDE_MS") / 1000.0 + 0.25)
    # an OWNER pinned label: the built-in ones (Needs Reply, Updates, Promotions) are tabs of the core's own categories, which a reset fixture holds
    # no row for (main63: the tab was empty, so the reader had nothing to follow); an owner label is filled below by the L key
    pinned = next((r for r in labels() if r.get("pinned") and r.get("kind", "owner") == "owner"), None) or next((r for r in labels() if r.get("pinned")), None)
    free = next((r for r in labels() if not r.get("pinned") and r.get("kind", "owner") == "owner"), None)
    if pinned is None or free is None:
        # a reset fixture files nothing under any owner label (FixtureCore.resetMessages: assigned = [:]), so the "has mail" read below
        # needs a filing first; a fixture with no such label is not judged, never a crash that ends the whole run (main58: no result line)
        s.expect("par-label-click-needs-a-pinned-and-an-unpinned-label", lambda: (_ for _ in ()).throw(Unreadable("the Labels section shows no pinned label and no unpinned owner label (%d rows)" % len(labels()))), True)
        return
    filed = {}
    s.click("mac-main.sidebar.mailbox.all-inboxes")
    s.click("mac-main.list.row.0")                # the pinned label's tab holds the first message too, so the reader has a row to follow
    s.key("L")
    s.type(pinned.get("raw_name", pinned["name"]))
    s.key("Enter")
    s._act(lambda: filed.update(ids=ids()[1:2]))
    s.click("mac-main.list.row.1")                # the list holds the keys; L files the focused SECOND message under the unpinned label
    s.key("L")
    s.type(free.get("raw_name", free["name"]))
    s.key("Enter")
    s.click("mac-main.list.row.0")                # the reader goes back to the first message, so a reader that did not follow the filter is seen
    s.click("mac-main.sidebar.mailbox.all-inboxes")
    settle()
    base = {}
    base["ids"] = ids()
    base["tab"] = None
    s._act(lambda: base.update(tab=s.ctl.uistate().get("tab")))
    reveal_label(s, pinned["id"])                 # the fixture's pinned Provanta sits under Work > Clients, folded at start: an undrawn row has no click door
    s.click("mac-main.sidebar.label.%s" % pinned["id"])
    settle()
    s.expect("par-label-click-selects-its-tab", lambda: (s.ctl.uistate().get("tab"), state().get("label_filter")), (pinned["id"], None))
    s.expect("par-label-click-moves-the-reader", lambda: (shown() == (ids() or [None])[0], bool(ids())), (True, True))
    s.click("mac-main.sidebar.mailbox.all-inboxes")
    reveal_label(s, free["id"])
    s.click("mac-main.sidebar.label.%s" % free["id"])
    settle()
    s.expect("par-label-click-filters-the-list", lambda: (state().get("label_filter"), 0 < len(ids()) < len(base["ids"])), (free["id"], True))
    s.expect("par-label-click-reader-shows-the-first-row", lambda: shown() == (ids() or [None])[0], True)
    s.expect("par-label-click-list-is-the-labels-messages", lambda: ids(), filed.get("ids"))
    s.click("mac-main.sidebar.mailbox.all-inboxes")
    s.expect("par-label-click-all-inboxes-gives-the-tab-back", lambda: s.ctl.uistate().get("tab"), base["tab"])
    s.expect("par-label-click-ends-with-all-inboxes", lambda: (state().get("label_filter"), ids()), (None, base["ids"]))
    s.raw("fixture", "reset")


# ---- doors-nav (docs/lanes/doors-nav.md): every control of the sidebar has a door, and each door's control is clicked once --------------------
# Door table: every click id the Sidebar folder registers, the control it stands for, and the case that clicks it. A pattern is the id with each
# interpolation written `*`. The first check reads the Swift files and compares both ways, so a door cannot be added or lost without this table saying so.
SIDEBAR_DIR = os.path.join(ROOT, "Sources", "Features", "Sidebar")
INTERP = re.compile(r"\\\((?:[^()]|\((?:[^()]|\([^()]*\))*\))*\)")
SIDEBAR_DOORS = {
    "mac-main.accounts.*": "an account's ring (folded group) or row (open group), pulled-up menu: par-account-filters, J1.15",
    "mac-main.accounts.all": "the menu's All accounts row: par-account-click-again-clears's restore, J1.15",
    "mac-main.sidebar.accounts-menu": "the bar at the foot (marks, +N, chevron are one control): par-menu-click-pulls-up",
    "mac-main.sidebar.accounts-menu.add": "the pull-up's Add account row: par-add-account-routes-to-its-surface",
    "mac-main.sidebar.accounts-menu.outside": "a click outside the pulled-up panel: par-menu-outside-pulls-down",
    "mac-main.sidebar.accounts-menu.up": "drive door for the bar's drag up / the Escape start state; no control of its own",
    "mac-main.sidebar.all-inboxes.chevron": "the chevron in the All inboxes row's gutter: par-all-inboxes-unfolds-one-row-per-account",
    "mac-main.sidebar.ask.pin-view.*": "drive door (the sidebar never offers a pin on a view): par-view-pin-refused-in-words",
    "mac-main.sidebar.command.*": "a Go-to command: par-go-commands-select-their-mailbox, par-go-i-opens-primary",
    "mac-main.sidebar.command.accounts": "the accounts command (the key): par-menu-accounts-key",
    "mac-main.sidebar.command.sidebar": "the sidebar command (the key): par-sidebar-command-hides",
    "mac-main.sidebar.confirm.no": "the delete alert's Cancel: par-menu-delete-cancel-keeps",
    "mac-main.sidebar.confirm.yes": "the delete alert's Delete: par-menu-delete-removes-label",
    "mac-main.sidebar.drag.cancel": "Escape in a label drag: par-drag-cancel-drops-nothing",
    "mac-main.sidebar.empty.add": "the empty sidebar's Add account button: J1.57/empty-add-door-opens-the-add-account-surface",
    "mac-main.sidebar.group.*": "a group header in the pulled-up menu: par-fold-work, par-unfold-work",
    "mac-main.sidebar.hide.*": "drive door for Hide (the control is the right-click item): par-hide-item",
    "mac-main.sidebar.inbox.*": "an account row under All inboxes: par-inbox-row-filters-the-list",
    "mac-main.sidebar.label.*": "a label row: par-select-far-fades, par-label-click-selects-its-tab",
    "mac-main.sidebar.label.*.drop-*": "the drop zones of a label row: par-drag-into-reparents, -before, -after",
    "mac-main.sidebar.label.*.jump": "drive door for a jump that names a label (the palette): opens the parents, then selects; par-palettemac label-jump-opens-the-parents-first",
    "mac-main.sidebar.label.*.hover": "drive door (the drive cannot hover): par-pin-hover-shows-the-pin",
    "mac-main.sidebar.label.*.lift": "drive door for starting a label drag: par-drag-held-then-outlined",
    "mac-main.sidebar.label.*.open": "drive door for opening a label: par-key-right-expands",
    "mac-main.sidebar.label.*.over-*": "drive door for a drag over a zone: par-drag-over-names-the-drop",
    "mac-main.sidebar.label.*.pin": "the pin on a label row: par-pin-click-pins, par-pin-click-unpins",
    "mac-main.sidebar.label.*.toggle": "the chevron on a label row: par-scoped-label-hidden-without-its-filter",
    "mac-main.sidebar.labels.add": "the + on the Labels header: par-label-plus-opens-inspector",
    "mac-main.sidebar.labels.hover": "drive door (the drive cannot hover): par-label-plus-opens-inspector's precursor",
    "mac-main.sidebar.mailbox.*": "a mailbox or view row: par-select-near-slides, par-list-shows-*",
    "mac-main.sidebar.mailbox.outbox": "the Outbox row: J1.57/outbox-door-selects-the-outbox",
    "mac-main.sidebar.mailboxes.pick": "the Mailboxes chip: par-picker-opens",
    "mac-main.sidebar.menu.*.*": "every item of every right-click menu: par-menu-*, J1.57/each-mailbox-and-view-hide-item-hides-its-row, -account-info-, label-mark-all-read-",
    "mac-main.sidebar.pick-item.*": "an item of the hidden sidebar's picker: J1.57/hidden-picker-door-*",
    "mac-main.sidebar.pick.*": "an account's tick in the Mailboxes picker: J1.57/each-account-tick-unticks-and-reticks",
    "mac-main.sidebar.pick.all": "the picker's All tick: par-all-rechecks-every-account",
    "mac-main.sidebar.prompt.cancel": "the prompt's Cancel: J1.57/prompt-cancel-closes-it-and-keeps-the-folders",
    "mac-main.sidebar.prompt.confirm": "the prompt's Create / Rename: par-menu-rename-label",
    "mac-main.sidebar.section.*.chevron": "a section header's fold chevron: par-section-*-folds",
    "mac-main.sidebar.show-all": "drive door that restores the hidden set (the control is the menu item Show hidden mailboxes): par-show-all",
    "mac-main.sidebar.smart.*": "a smart folder row: par-smart-folder-opens-its-results",
    "mac-main.sidebar.toggle.button": "the sidebar toggle beside the lights: par-toggle-hides",
}
# Ids the views draw that are not controls, or that a door of another name answers.
SIDEBAR_DISPLAY = {
    "mac-main.sidebar": "the pane",
    "mac-main.sidebar.header.mailboxes": "the Mailboxes header box (measured by par-header-on-tab-strip)",
    "mac-main.sidebar.accounts-menu.panel": "the pulled-up panel itself; its rows are the account doors",
    "mac-main.sidebar.label.*.drop-before": "a 4 pt drop mark, hit testing off; the drop-* doors act for it",
    "mac-main.sidebar.label.*.drop-after": "a 4 pt drop mark, hit testing off; the drop-* doors act for it",
    "mac-main.sidebar.empty": "the empty state's container",
    "mac-main.sidebar.inbox-group.*": "the group caption over an account's rows",
}
SIDEBAR_ALIAS = {
    "mac-main.sidebar.*.add": "mac-main.sidebar.labels.add",      # SidebarHeader draws the + from its title; only Labels passes onAdd
    "mac-main.sidebar.toggle": "mac-main.sidebar.toggle.button",  # the toggle's measurable id; the click door is .button (the shell registers the bare id too)
}


def norm(text):
    return INTERP.sub("*", text)


def sidebar_sources():
    out = {}
    for name in sorted(os.listdir(SIDEBAR_DIR)):
        if name.endswith(".swift"):
            with open(os.path.join(SIDEBAR_DIR, name), encoding="utf-8") as f:
                out[name] = f.read()
    return out


def door_ids(files):
    """The click ids the Swift files register: keys written `t["id"] =` or `"id": {` inside a function whose name holds `lick` or `Targets`, and the
    empty state's constant key."""
    out = set()
    for text in files.values():
        fn = ""
        for line in text.splitlines():
            m = re.search(r"\bfunc (\w+)", line)
            if m:
                fn = m.group(1)
            if "lick" not in fn and "Targets" not in fn:
                continue
            m = re.search(r'\bt\["([^"]+)"\]\s*=', line) or re.match(r'\s*"(mac-[^"]*)":\s*\{', line)
            if m:
                out.add(norm(m.group(1)))
    empty = files.get("SidebarEmptyState.swift", "")
    key = re.search(r'static let addAx = "([^"]+)"', empty)
    if key and re.search(r"\bSidebarEmptyState\.addAx:\s*\{", "\n".join(files.values())):
        out.add(key.group(1))
    return out


def control_ids(files):
    """The ids the views give their controls: whooshAx("id"), accessibilityIdentifier("id") and pinAx: "id" literals."""
    out = set()
    for text in files.values():
        for m in re.finditer(r'(?:whooshAx\(|accessibilityIdentifier\(|pinAx:\s*)"(mac-[^"]+)"', text):
            out.add(norm(m.group(1)))
    return out


def wait(s, thunk, want, tries=30):
    """The last value thunk gave, after up to `tries` reads 0.1 s apart (none on a stand-in); stops when it equals `want`."""
    v = thunk()
    for _ in range(tries):
        if v == want or getattr(s.j.ctl, "instant", False):
            break
        time.sleep(0.1)
        v = thunk()
    return v


def back_to_main(s):
    for _ in range(3):   # Escape walks back one layer at a time: the quick reply, the selection, the thread
        if s.ctl.uistate()["surface"] == "mac-main":
            break
        s.ctl.key("Escape")
    return s.ctl.uistate()["surface"]


def door(s, ax):
    """Click a door; False when the app refuses it (ok:false), which is what a dead control looks like."""
    ctl = s.ctl   # no socket raises here: that is "not judged", not a dead control
    try:
        ctl.click(ax)
        return True
    except Unreadable:
        return False


SLUGS = ["all-inboxes", "vips", "pinned", "snoozed", "drafts", "sent", "junk", "trash", "archive", "starred", "muted", "all-mail"]
NAMES = {"all-inboxes": "All inboxes", "vips": "VIPs", "pinned": "Pinned", "snoozed": "Snoozed", "drafts": "Drafts", "sent": "Sent", "junk": "Junk",
         "trash": "Trash", "archive": "Archive", "starred": "Starred", "muted": "Muted", "all-mail": "All mail"}


def doors_nav(j, src=None):
    """J1.57 (doors-nav): the controls of the sidebar no case clicked before. Each is clicked through its door and its visible outcome read."""
    src = src or sidebar_sources()
    s = j.step("J1.57", "doors-nav: the sidebar's controls, each clicked once: smart folder prompt, hide, account info, mark all read, ticks, outbox, picker, empty state")
    doors, views = door_ids(src), control_ids(src)
    s.expect("every-door-of-the-sidebar-is-in-the-control-table", lambda: (sorted(doors - set(SIDEBAR_DOORS)), sorted(set(SIDEBAR_DOORS) - doors)), ([], []))
    known = set(SIDEBAR_DOORS) | set(SIDEBAR_DISPLAY) | set(SIDEBAR_ALIAS)
    s.expect("every-control-the-sidebar-draws-has-a-door", lambda: (sorted(views - known), sorted(v for v in SIDEBAR_ALIAS.values() if v not in doors)), ([], []))
    # The label rows' redraw after a mail change is asked with a timer of SIDEBAR_COUNTS_FOLLOW_MS that a newer change replaces, never with a main-queue
    # block: such a block runs before the key's frame is shown and put the whole redraw into every archive (real engine, archive median 34.4 ms on
    # main184, 51.6 ms on main187: checks/realcore-perf.py --archive 34, the case that measures it). From the source: what `mailChanged` does.
    def redraw_waits_for_quiet_mail():
        t = src.get("SidebarEmptyState.swift", "")
        m = re.search(r"func mailChanged\(\) \{(.*?)\n    \}\n", t, re.S)
        body = m.group(1) if m else ""
        return ("mailRedraw?.cancel()" in body, "asyncAfter(deadline: .now() + .milliseconds(WhooshConstants.SIDEBAR_COUNTS_FOLLOW_MS)" in body,
                "DispatchQueue.main.async {" not in body, "labelsRevision += 1" in body and "self.revision += 1" not in body)
    s.expect("label-redraw-waits-for-quiet-mail", redraw_waits_for_quiet_mail, (True, True, True, True))
    s.raw("fixture", "reset")
    s.click("dock.whoosh")
    s.click(LANGUAGE % "en")
    s.click("mac-main.sidebar.mailbox.all-inboxes")   # the inbox on its own tab: the cases below read and reply on it
    rows = lambda ax: s.ctl.rows(ax)
    prompt = lambda: rows("mac-main.sidebar.prompt")[0]
    smart_ids = lambda: sorted(r["ax"] for r in rows("mac-main.sidebar.smart-folders"))
    selection = lambda: rows("mac-main.sidebar.selection")[0]["id"]
    # New smart folder… on the Smart folders header menu, and the prompt's Cancel
    before = {}
    try:
        before["smart"] = smart_ids()
    except Unreadable:
        before["smart"] = None
    s.click("mac-main.sidebar.menu.header.smart.new")
    s.expect("header-new-smart-folder-opens-its-prompt", lambda: (prompt()["open"], prompt()["title"], prompt()["has_rule"]), (True, "New smart folder", True))
    s.click("mac-main.sidebar.prompt.cancel")
    s.expect("prompt-cancel-closes-it-and-keeps-the-folders", lambda: (prompt()["open"], smart_ids() == before["smart"]), (False, True))
    # the right-click menus of every mailbox and view: Hide, and Account info
    titles = lambda: [r["title"] for r in rows("mac-main.sidebar.pick-items") if r["group"] == "Mailboxes"]

    def hide_all():
        bad = []
        for slug in SLUGS:
            if not door(s, "mac-main.sidebar.menu.mailbox.%s.hide" % slug) or NAMES[slug] in titles():
                bad.append(slug)
            door(s, "mac-main.sidebar.show-all")
        return bad
    s.expect("each-mailbox-and-view-hide-item-hides-its-row", hide_all, [])

    def info_all():
        bad = []
        for slug in SLUGS:
            if not door(s, "mac-main.sidebar.menu.mailbox.%s.account-info" % slug) or not rows("mac-main.sidebar.accounts-menu")[0]["open"]:
                bad.append(slug)
            door(s, "mac-main.sidebar.accounts-menu.outside")
        return bad
    s.expect("each-mailbox-and-view-account-info-item-pulls-the-menu-up", info_all, [])
    # F-A1: the way back from Hide is a menu item, drawn only while something is hidden
    shown_items = lambda: [r["title"] for r in rows("mac-main.sidebar.menus") if r["item"] == "show-hidden"]
    door(s, "mac-main.sidebar.show-all")
    none_before = shown_items()
    door(s, "mac-main.sidebar.menu.mailbox.sent.hide")
    while_hidden = shown_items()
    door(s, "mac-main.sidebar.show-all")
    none_after = shown_items()
    s.expect("show-hidden-item-is-drawn-only-while-something-is-hidden", lambda: (none_before, bool(while_hidden) and set(while_hidden) == {"Show hidden mailboxes"}, none_after),
             ([], True, []))

    def show_hidden_all():
        bad = []
        for slug in ("sent", "vips", "starred"):
            door(s, "mac-main.sidebar.menu.mailbox.%s.hide" % slug)
            door(s, "mac-main.sidebar.menu.mailbox.drafts.hide")
            if not door(s, "mac-main.sidebar.menu.mailbox.all-inboxes.show-hidden") or len(titles()) != len(SLUGS):
                bad.append(slug)
            door(s, "mac-main.sidebar.show-all")
        return bad
    s.expect("show-hidden-item-brings-the-hidden-mailboxes-back", show_hidden_all, [])
    # a label's Mark all as read
    labels = lambda: {r["id"]: r for r in rows("mac-main.sidebar.labels")}
    victim = {}
    try:
        victim["id"] = next(i for i, r in labels().items() if r["own"] > 0)
    except (Unreadable, StopIteration):
        victim["id"] = None
    # What the owner sees is the label's mail turning read, so the check reads the label's rows. The row's own figure is the core's: a built-in
    # label (Needs Reply) is a tab whose figure is the fixture file's number (FixtureCore.countsLocked, tabCount: `t.unread - left + into - out +
    # arrived`), which reading a fixture row never lowers; only an owner label's figure is live (FixtureLabels.labelsLocked, `live`).
    listed = lambda: sum(1 for r in s.ctl.rows(LIST) if r.get("unread"))
    first = {}
    if victim["id"]:
        reveal_label(s, victim["id"])
        s.click("mac-main.sidebar.label.%s" % victim["id"])
        None if getattr(s.j.ctl, "instant", False) else time.sleep(0.5)
        s._act(lambda: first.update(rows=listed(), kind=labels()[victim["id"]].get("kind", "owner")))
    # A mail opened in the LIST lowers the count the label's row draws. `own` and `count` are read from the core when the socket is asked; `drawn` is what the
    # row drew at its last draw. They differed on main183: the tab strip said "Needs Reply 29", the sidebar row still drew 30 (the lead's walk, 14 Nov 2026).
    def a_read_in_the_list_redraws_the_count():
        vid = victim["id"]
        if not vid or not first.get("rows"):
            raise Unreadable("no label with unread mail to open (own figure %s)" % (labels()[vid]["own"] if vid else None))
        before = labels()[vid]["own"]
        s.click("mac-main.list.row.%d" % next(i for i, r in enumerate(s.ctl.rows(LIST)) if r.get("unread")))
        s.key("Enter")
        try:
            if not wait(s, lambda: labels()[vid]["own"] != before, True):
                raise Unreadable("opening an unread mail of the label left the core's figure at %s: nothing to compare" % before)
            return wait(s, lambda: (lambda r: r.get("drawn") == r.get("count", r["own"]))(labels()[vid]), True)
        finally:
            s.key("Escape")
    s.expect("a-mail-read-in-the-list-redraws-the-label-count", a_read_in_the_list_redraws_the_count, True)
    s.click("mac-main.sidebar.menu.label.%s.mark-read" % victim["id"])
    s.expect("label-mark-all-read-item-clears-its-unread",
             lambda: wait(s, lambda: (listed(), labels()[victim["id"]]["own"] if first.get("kind") == "owner" else 0) if first.get("rows") else (_ for _ in ()).throw(Unreadable("the label shows no unread row to clear (own figure %s)" % (labels()[victim["id"]]["own"] if victim["id"] else None))), (0, 0)), (0, 0))
    # every account's tick in the Mailboxes picker
    ids = sorted(a["id"] for a in FX["accounts"])
    ticked = lambda: {r["id"]: r["checked"] for r in rows("mac-main.sidebar.inbox-picker")}

    def ticks():
        bad = []
        for a in (ids[0], ids[1], ids[-1]):
            off = door(s, "mac-main.sidebar.pick.%s" % a) and ticked()[a] is False
            on = door(s, "mac-main.sidebar.pick.%s" % a) and ticked()[a] is True
            if not (off and on):
                bad.append(a)
        door(s, "mac-main.sidebar.pick.all")
        return bad
    s.expect("each-account-tick-unticks-and-reticks", ticks, [])
    # the Outbox row: a queued send puts it first; its door selects it; the send leaving gives the highlight back
    s.raw("fixture", "reset")
    s.click("dock.whoosh")
    s.click("mac-main.list.row.0")
    s.key("Enter")
    s.key("R")
    s.type("A reply that waits in the undo window.")
    s.key("⌘Enter")
    s._act(lambda: back_to_main(s))
    s.click("mac-main.sidebar.mailbox.outbox")
    s.expect("outbox-door-selects-the-outbox", lambda: selection(), "mailbox.outbox")
    s.raw("clock", "advance", const("UNDO_SEND_DEFAULT_S") + 1)
    s.expect("outbox-selection-falls-back-when-the-send-leaves", lambda: wait(s, selection, "mailbox.all-inboxes"), "mailbox.all-inboxes")
    # the hidden sidebar's picker: choosing an item does what a sidebar click does
    s.raw("fixture", "reset")
    s.click("mac-main.sidebar.pick-item.mailbox.sent")
    s.expect("hidden-picker-door-selects-its-mailbox", lambda: selection(), "mailbox.sent")
    smart = {}
    try:
        smart["id"] = next(r["id"] for r in rows("mac-main.sidebar.pick-items") if r["group"] == "Smart folders")
    except (Unreadable, StopIteration):
        smart["id"] = None
    s.click("mac-main.sidebar.pick-item.%s" % smart["id"])
    s.expect("hidden-picker-door-selects-its-smart-folder", lambda: (smart["id"] is not None, selection() == smart["id"]), (True, True))
    s.click("mac-main.accounts.acc-01")
    s.click("mac-main.sidebar.pick-item.mailbox.all-inboxes")
    s.expect("hidden-picker-all-inboxes-clears-the-account-filter", lambda: (selection(), s.ctl.uistate().get("account_filter")), ("mailbox.all-inboxes", None))
    # the empty sidebar (no account): its Add account button
    s.raw("fixture", "accounts", "none")
    s.expect("empty-sidebar-offers-add-account", lambda: [(r["ax"], r["route"]) for r in rows("mac-main.sidebar.empty")],
             [("mac-main.sidebar.empty.add", "mac-add-account")])
    s.click("mac-main.sidebar.empty.add")
    s.expect("empty-add-door-opens-the-add-account-surface", lambda: s.ctl.uistate()["surface"], "mac-add-account")
    s.raw("fixture", "accounts", "all")
    s.click("dock.whoosh")
    s.raw("fixture", "reset")


class LabelCtl:
    """The stand-in for round 12: Primary holds four rows, Provanta is a pinned label (a tab), Nordlicht an unpinned one that filters."""
    instant = True
    ROWS = {"primary": ["m1", "m2", "m3", "m4"], "lbl-pinned": ["m5", "m6"]}

    def __init__(self, mutants=()):
        self.m = set(mutants)
        self.tab, self.filter, self.reader, self.typed, self.TAGGED, self.focus = "primary", None, "m1", "", {}, 0
        self.open_top = False   # lbl-pinned sits under the folded lbl-top, as the fixture's Provanta sits under Work > Clients

    def rows_now(self):
        if self.filter and "par-label-click-filters-the-list" not in self.m:
            tagged = list(self.TAGGED.get(self.filter, []))
            return tagged + ["m3"] if "par-label-click-list-is-the-labels-messages" in self.m else tagged
        return list(self.ROWS.get(self.tab, self.ROWS["primary"]))

    def click(self, ax):
        if ax.startswith("mac-main.list.row."):
            self.focus = int(ax.rsplit(".", 1)[1])
            self.reader = self.rows_now()[self.focus]
        elif ax == "mac-main.sidebar.label.lbl-top.open":
            self.open_top = True
        elif ax.startswith("mac-main.sidebar.label."):
            lid = ax.rsplit(".", 1)[1]
            if lid == "lbl-pinned" and not self.open_top:
                raise Unreadable("click %s: refused" % ax)   # the app's door: a row that is not drawn has no click
            if lid == "lbl-pinned":
                if "par-label-click-selects-its-tab" not in self.m:
                    self.tab, self.filter = lid, None
            else:
                self.filter = lid
            skip = "par-label-click-moves-the-reader" if lid == "lbl-pinned" else "par-label-click-reader-shows-the-first-row"
            if skip not in self.m:
                self.reader = self.rows_now()[0]
        elif ax == "mac-main.sidebar.mailbox.all-inboxes":
            if "par-label-click-all-inboxes-gives-the-tab-back" not in self.m:
                self.tab = "primary"
            if "par-label-click-ends-with-all-inboxes" not in self.m:
                self.filter = None
            self.reader = self.rows_now()[0]
        return {}

    def key(self, chord):
        if chord == "Enter" and self.typed:
            lbl = "lbl-pinned" if self.typed == "Pinned" else "lbl-free"
            self.TAGGED.setdefault(lbl, []).append(self.rows_now()[self.focus])
            self.typed = ""
        return {}

    def type(self, text):
        self.typed = text
        return {}

    def send(self, verb, *a):
        return {"ok": True}

    def uistate(self):
        return {"tab": self.tab, "surface": "mac-main"}

    def rows(self, ax):
        if ax == "mac-main.sidebar.labels":
            return [{"id": "lbl-free", "pinned": False, "own": 0, "kind": "owner", "name": "Free", "raw_name": "Free", "parent": ""},
                    {"id": "lbl-top", "pinned": False, "own": 0, "kind": "owner", "name": "Top", "raw_name": "Top", "parent": "", "open": self.open_top},
                    {"id": "lbl-pinned", "pinned": True, "own": 2, "kind": "owner", "name": "Pinned", "raw_name": "Pinned", "parent": "lbl-top"}]
        if ax == LIST:
            return [{"ax": "mac-main.list.row.%d" % i, "id": mid} for i, mid in enumerate(self.rows_now())]
        if ax == "mac-main.list.state":
            return [{"label_filter": self.filter}]
        if ax == "mac-main.reader.shown":
            return [{"shown": self.reader}]
        return []


class DoorsNavCtl:
    """The stand-in for J1.57: the sidebar's controls as the step clicks them, modelled as SPECIFIED. A mutant breaks exactly one control (the one its
    check names). A door the app refuses raises Unreadable, as the real socket does (ok:false)."""
    instant = True
    ACCOUNTS = sorted(a["id"] for a in FX["accounts"])
    SMART = ["smart-1", "smart-2"]

    def __init__(self, mutants=()):
        self.m = set(mutants)
        self.reset()

    def reset(self):
        self.hidden, self.menu, self.waiting, self.surface, self.filter, self.none = set(), False, 0, "mac-main", None, False
        self.prompt = {"open": False, "title": "", "has_rule": False}
        self.smart, self.own, self.unchecked, self.sel = list(self.SMART), {"needs-reply": 2, "lbl-a": 3, "lbl-b": 0}, set(), "mailbox.all-inboxes"
        self.unread_rows, self.shown = {"needs-reply": 2, "lbl-a": 3, "lbl-b": 0}, None
        self.drawn = dict(self.own)

    def mut(self, name):
        return name in self.m

    def click(self, ax):
        m = re.match(r"mac-main\.sidebar\.menu\.mailbox\.([\w-]+)\.(hide|account-info)$", ax)
        if m:
            slug, item = m.groups()
            if item == "hide" and not (slug == "vips" and self.mut("each-mailbox-and-view-hide-item-hides-its-row")):
                self.hidden.add(slug)
            if item == "account-info" and not (slug == "sent" and self.mut("each-mailbox-and-view-account-info-item-pulls-the-menu-up")):
                self.menu = True
            return {}
        m = re.match(r"mac-main\.sidebar\.menu\.(?:mailbox|header)\.([\w-]+)\.show-hidden$", ax)
        if m:
            if not self.hidden:
                raise Unreadable("click %s: refused" % ax)
            if not self.mut("show-hidden-item-brings-the-hidden-mailboxes-back"):
                self.hidden = set()
            return {}
        m = re.match(r"mac-main\.sidebar\.menu\.label\.([\w-]+)\.mark-read$", ax)
        if m:
            if not self.mut("label-mark-all-read-item-clears-its-unread"):
                self.unread_rows[m.group(1)] = 0
                if m.group(1) != "needs-reply":   # a built-in label's figure is the fixture file's: reading its rows never lowers it
                    self.own[m.group(1)] = 0
                self.drawn = dict(self.own)       # a sidebar action redraws the rows
            return {}
        m = re.match(r"mac-main\.sidebar\.label\.([\w-]+)$", ax)
        if m:
            self.shown = m.group(1)
            return {}
        m = re.match(r"mac-main\.sidebar\.pick\.([\w-]+)$", ax)
        if m and m.group(1) != "all":
            if not (m.group(1) == self.ACCOUNTS[1] and self.mut("each-account-tick-unticks-and-reticks")):
                self.unchecked ^= {m.group(1)}
            return {}
        m = re.match(r"mac-main\.sidebar\.pick-item\.(.+)$", ax)
        if m:
            item = m.group(1)
            if item == "mailbox.sent" and self.mut("hidden-picker-door-selects-its-mailbox"):
                return {}
            if item.startswith("smart.") and self.mut("hidden-picker-door-selects-its-smart-folder"):
                return {}
            self.sel = item
            if item == "mailbox.all-inboxes" and not self.mut("hidden-picker-all-inboxes-clears-the-account-filter"):
                self.filter = None
            return {}
        if ax == "mac-main.sidebar.menu.header.smart.new":
            if not self.mut("header-new-smart-folder-opens-its-prompt"):
                self.prompt = {"open": True, "title": "New smart folder", "has_rule": True}
        elif ax == "mac-main.sidebar.prompt.cancel":
            if self.mut("prompt-cancel-closes-it-and-keeps-the-folders"):
                self.smart.append("smart-3")
            self.prompt = {"open": False, "title": "", "has_rule": False}
        elif ax == "mac-main.sidebar.accounts-menu.outside":
            self.menu = False
        elif ax == "mac-main.sidebar.show-all":
            if not self.hidden:
                raise Unreadable("click %s: refused" % ax)
            self.hidden = set()
        elif ax == "mac-main.sidebar.pick.all":
            self.unchecked = set()
        elif ax == "mac-main.sidebar.mailbox.outbox":
            if not self.waiting:
                raise Unreadable("click %s: refused" % ax)
            if not self.mut("outbox-door-selects-the-outbox"):
                self.sel = "mailbox.outbox"
        elif ax == "mac-main.accounts.acc-01":
            self.filter = "acc-01"
        elif ax == "mac-main.sidebar.empty.add":
            if not self.none:
                raise Unreadable("click %s: refused" % ax)
            if not self.mut("empty-add-door-opens-the-add-account-surface"):
                self.surface = "mac-add-account"
        elif ax == "dock.whoosh":
            self.surface = "mac-main"
        return {}

    def key(self, chord):
        if chord == "Enter" and self.surface == "mac-main":
            self.surface = "mac-reader"
            if self.shown and self.unread_rows.get(self.shown, 0) > 0:      # the opened mail turns read: the core's figure drops, and the row draws it again
                self.unread_rows[self.shown] -= 1
                self.own[self.shown] -= 1
                if not self.mut("a-mail-read-in-the-list-redraws-the-label-count"):
                    self.drawn = dict(self.own)
        elif chord == "⌘Enter":
            self.waiting = 1
        elif chord == "Escape" and self.surface != "mac-main":
            self.surface = "mac-main"
        return {}

    def type(self, text):
        return {}

    def send(self, verb, *a):
        if verb == "fixture" and a[:1] == ("reset",):
            self.reset()
        elif verb == "fixture" and a[:2] == ("accounts", "none"):
            self.none = True
        elif verb == "fixture" and a[:2] == ("accounts", "all"):
            self.none = False
        elif verb == "clock":
            self.waiting = 0
            if self.sel == "mailbox.outbox" and not self.mut("outbox-selection-falls-back-when-the-send-leaves"):
                self.sel = "mailbox.all-inboxes"
        return {"ok": True}

    def uistate(self):
        return {"surface": self.surface, "account_filter": self.filter}

    def rows(self, ax):
        if ax == "mac-main.sidebar.prompt":
            return [dict(self.prompt, text="")]
        if ax == "mac-main.sidebar.menus":
            drawn = bool(self.hidden) != self.mut("show-hidden-item-is-drawn-only-while-something-is-hidden")
            return [{"ax": "mac-main.sidebar.menu.mailbox.all-inboxes.show-hidden", "subject": "mailbox.all-inboxes", "item": "show-hidden", "title": "Show hidden mailboxes"}] if drawn else []
        if ax == "mac-main.sidebar.smart-folders":
            return [{"ax": "mac-main.sidebar.smart.%s" % x} for x in self.smart]
        if ax == "mac-main.sidebar.pick-items":
            names = ["All inboxes", "VIPs", "Pinned", "Snoozed", "Drafts", "Sent", "Junk", "Trash", "Archive", "Starred", "Muted", "All mail"]
            return ([{"id": "mailbox.%s" % n.lower().replace(" ", "-"), "group": "Mailboxes", "title": n} for n in names
                     if n.lower().replace(" ", "-") not in self.hidden]
                    + [{"id": "smart.%s" % x, "group": "Smart folders", "title": x} for x in self.smart])
        if ax == "mac-main.sidebar.selection":
            return [{"id": self.sel}]
        if ax == "mac-main.sidebar.accounts-menu":
            return [{"open": self.menu}]
        if ax == "mac-main.sidebar.labels":
            return [{"id": k, "own": v, "count": v, "drawn": self.drawn[k], "kind": "built-in" if k == "needs-reply" else "owner", "parent": "", "pinned": k == "needs-reply"} for k, v in self.own.items()]
        if ax == LIST:
            return [{"ax": "mac-main.list.row.%d" % i, "id": "m%d" % i, "unread": True} for i in range(self.unread_rows.get(self.shown, 0))]
        if ax == "mac-main.sidebar.inbox-picker":
            return [{"id": a, "checked": a not in self.unchecked} for a in self.ACCOUNTS]
        if ax == "mac-main.sidebar.empty":
            return [{"ax": "mac-main.sidebar.empty.add", "title": "Add account", "route": "mac-add-account"}] if self.none and not self.mut("empty-sidebar-offers-add-account") else []
        return []


def mutant_words(step):
    out = {}
    for line in open(os.path.abspath(__file__), encoding="utf-8"):
        m = re.match(r"#\s+" + re.escape(step) + r"/([\w-]+)\s*\|\s*(.+)", line)
        if m:
            out[m.group(1)] = m.group(2).strip()
    return out


def selftest_doors_nav():
    """J1.57 against the stand-in, clean and once per mutant. The two source checks read copies of the Swift files with one more door or one more control."""
    src0 = sidebar_sources()
    mutated = {
        "every-door-of-the-sidebar-is-in-the-control-table": lambda: dict(src0, **{"SidebarFeature.swift": src0["SidebarFeature.swift"].replace(
            '"mac-main.accounts.all": {', '"mac-main.sidebar.zzz": { _ in false },\n            "mac-main.accounts.all": {', 1)}),
        "every-control-the-sidebar-draws-has-a-door": lambda: dict(src0, **{"SidebarView.swift": src0["SidebarView.swift"].replace(
            '.whooshAx("mac-main.sidebar")', '.whooshAx("mac-main.sidebar").whooshAx("mac-main.sidebar.zzz")', 1)}),
        "label-redraw-waits-for-quiet-mail": lambda: dict(src0, **{"SidebarEmptyState.swift": src0["SidebarEmptyState.swift"].replace(
            "DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(WhooshConstants.SIDEBAR_COUNTS_FOLLOW_MS), execute: work)", "DispatchQueue.main.async { work.perform() }", 1)}),
    }
    for name, make in mutated.items():
        assert make() != src0, "mutant %s changes nothing: its anchor text is gone from the source" % name
    words = mutant_words("J1.57")

    def run(mutant=None):
        j = Journey(ID, DoorsNavCtl([mutant] if mutant else []))
        doors_nav(j, mutated[mutant]() if mutant in mutated else src0)
        return {c.label.split("/", 1)[1]: (c.state, c.detail) for c in j.checks()}
    clean = run()
    print_model_cases(clean)
    bad = [n for n in clean if n not in words] + [n for n in words if n not in clean]
    for n in bad:
        print("MUTANTS | %s | names no mutant, or the mutant names no check" % n)
    print("MUTANTS | check | clean | mutant | mutant result")
    for name, (state, detail) in clean.items():
        red = run(name)[name]
        ok_red = red[0] == "fail"
        print("MUTANTS | %s | %s | %s | %s" % (name, "green" if state == "pass" else "RED " + detail, words.get(name, "?"), "red" if ok_red else "STILL " + red[0].upper()))
        bad += [name] if (state != "pass" or not ok_red) else []
    print("J1.57: %d checks, %d failed, 0 not judged" % (len(clean), len(bad)))
    return 1 if bad else 0


def selftest():
    words = {}
    for line in open(os.path.abspath(__file__), encoding="utf-8"):
        m = re.match(r"#\s+J1\.1/(par-label-click-[\w-]+)\s*\|\s*(.+)", line)
        if m:
            words[m.group(1)] = m.group(2).strip()

    def run(mutant=None):
        j = Journey(ID, LabelCtl([mutant] if mutant else []))
        round12(j)
        return {c.label.split("/", 1)[1]: (c.state, c.detail) for c in j.checks()}
    clean = run()
    print_model_cases(clean)
    bad = [n for n in clean if n not in words] + [n for n in words if n not in clean]
    print("MUTANTS | check | clean | mutant | mutant result")
    for name, (state, detail) in clean.items():
        red = run(name)[name]
        ok_red = red[0] == "fail"
        print("MUTANTS | %s | %s | %s | %s" % (name, "green" if state == "pass" else "RED " + detail, words.get(name, "?"), "red" if ok_red else "STILL " + red[0].upper()))
        bad += [name] if (state != "pass" or not ok_red) else []
    print("%d checks, %d failed, 0 not judged" % (len(clean), len(bad)))
    return (1 if bad else 0) | selftest_doors_nav()


# MUTANTS: per check, the mutation of the app that must turn it red.
# J1.1/arrives-by-control | the Dock launch opens another surface
# J1.1/par-one-button-per-address | the accounts rows drop the failing account
# J1.1/par-ring-min-px | the sidebar draws rings at the 18 pt compact size
# J1.1/par-four-groups | the groups are listed alphabetically
# J1.1/par-fold-work | the Work chevron does nothing
# J1.1/par-unfold-work | a folded group cannot be opened again
# J1.1/par-failing-group-dot | the warning dot sits on every group
# J1.1/par-account-filters | an account click leaves the list unfiltered
# J1.1/par-account-click-again-clears | a second click keeps the filter
# J1.1/par-failing-line | the failing account's line is missing
# J1.1/par-picker-opens | the Mailboxes button does not open the picker
# J1.1/par-default-all-checked | the picker starts with every account unchecked
# J1.1/par-uncheck-one-turns-all-off | unchecking an account leaves All on
# J1.1/par-uncheck-one-chip-counts | the chip ignores the unchecked accounts
# J1.1/par-all-rechecks-every-account | All checks nothing
# J1.1/par-one-account-left-checked-filters-the-list | the account filter leaves the other accounts checked
# J1.1/par-all-after-one-account-restores-the-list | All leaves the account filter on
# J1.14/par-hidden-list-keeps-its-width | the hidden sidebar's width is not handed to the list (it stays LIST_W_PX)
# J1.14/par-hidden-list-and-reader-fill-the-window | the reader pane row reads 100 pt short
# J1.14/par-go-commands-select-their-mailbox | go-sent selects Drafts
# J1.14/par-add-account-row-is-last | the pull-up has no Add account row
# J1.14/par-add-account-routes-to-its-surface | the row opens nothing
# J1.1/par-hide-item | hide does nothing
# J1.1/par-show-all | show all leaves an item hidden
# J1.1/par-menu-starts-collapsed | the foot reserves a height other than SIDEBAR_FOOT_PX
# J1.1/par-menu-click-pulls-up | a click on the bar does nothing
# J1.1/par-menu-health | every account reads Live
# J1.1/par-menu-escape-pulls-down | Escape leaves the menu up
# J1.1/par-menu-outside-pulls-down | a click outside leaves the menu up
# J1.1/par-menu-accounts-key | the A key does nothing
# J1.1/par-menu-overflow-fades | the panel's last group is sliced at its edge (no fade)
# J1.1/par-select-near-slides | the rule is inverted: near fades, far slides
# J1.1/par-select-far-fades | the rule is inverted: near fades, far slides
# J1.1/par-tree-depth-max | the tree stops one level short (depth 4)
# J1.1/par-tree-rollup | a parent shows only its own unread
# J1.1/par-tree-deepest-rolls-up | a parent shows only its own unread
# J1.1/par-key-right-expands | Right does nothing on a label
# J1.1/par-key-left-collapses | Left does nothing on a label
# J1.1/par-key-right-on-open-moves-the-pane | the sidebar keeps Right on an open label (moves to its child) and the pane focus stays
# J1.1/par-outbox-absent-when-empty | the Outbox row is always drawn, even with a count of 0
# J1.1/par-outbox-first-with-count | the Outbox row shows a stand-in count of 1 instead of the core's
# J1.1/par-outbox-above-all-inboxes | the Outbox row sits after the mailboxes
# J1.1/par-outbox-gone-when-sent | the Outbox row stays after the send left
# J1.1/par-outbox-escape-returns-to-main | Escape leaves the thread open (the surface stays mac-reader)
# J1.1/par-mailbox-order | Sent comes before Drafts
# J1.1/par-mailbox-no-stand-in-count | Junk shows a stand-in count of 0
# J1.1/par-menu-mailbox-items | the mailbox menu has no Account info
# J1.1/par-menu-smart-items | the smart folder menu has no Delete
# J1.1/par-menu-label-items | the label menu has no Change colour
# J1.1/par-menu-header-items | the Labels header has no menu
# J1.1/par-menu-missing-doors-named | an item is disabled without saying which door it needs
# J1.1/par-menu-hide | Hide in the menu does nothing
# J1.1/par-menu-account-info | Account info does nothing
# J1.1/par-menu-mark-all-read | Mark all as read marks nothing
# J1.1/par-label-plus-opens-inspector | the + on Labels opens the old name prompt
# J1.1/par-label-inspector-cancels | Cancel leaves the inspector open
# J1.1/par-menu-header-new-opens-inspector | New label… on the header opens the old name prompt
# J1.1/par-menu-new-inside-opens-inspector | New label inside… opens the old name prompt
# J1.1/par-menu-modify-opens-inspector | Modify label… stays disabled
# J1.1/par-menu-new-label-takes-the-typed-name | the new label is created with the inspector's default name, not the typed one
# J1.1/par-menu-new-label-inside | the new label lands at the top level
# J1.1/par-menu-rename-label | Rename keeps the old name
# J1.1/par-menu-delete-asks-first | Delete acts without asking
# J1.1/par-menu-delete-cancel-keeps | No deletes the label anyway
# J1.1/par-header-on-tab-strip | the sidebar content starts 52 pt below the pane top again
# J1.14/par-toggle-starts-shown | the pane width row reads the list width
# J1.14/par-toggle-beside-lights | the toggle draws 11 pt below the lights (no safe-area fix)
# J1.14/par-toggle-tooltip-key-shown | the tooltip drops the key
# J1.14/par-toggle-hides | the toggle click does nothing
# J1.14/par-toggle-slide-within-max | the row reports a slide of MOTION_MAX_MS plus 100
# J1.14/par-toggle-remembered | the hidden state is not saved
# J1.14/par-toggle-on-the-lights-line-after-resize | the toggle draws 11 pt below the lights (no safe-area fix)
# J1.14/par-toggle-on-the-lights-line-on-screen | the toggle is placed from the zoom button frame (a row below the lights)
# J1.14/par-toggle-on-the-lights-line-on-screen-after-resize | the toggle is placed from the zoom button frame (a row below the lights)
# J1.14/par-toggle-on-the-lights-line-on-screen-hidden | the hidden-state toggle is placed from the list's top, a row below the lights
# J1.14/par-toggle-stays-put-hidden | the toggle is placed 17 pt below the lights
# J1.14/par-toggle-tooltip-show | the tooltip drops the key
# J1.14/par-hidden-picker-holds-the-sidebar-items | the picker lists no smart folders
# J1.14/par-toggle-shows | the second click does not bring the sidebar back
# J1.14/par-toggle-forgets-when-shown | the saved state is always hidden
# J1.14/par-sidebar-command-hides | the sidebar command does nothing
# J1.14/par-sidebar-command-shows | the sidebar command only hides
# J1.15/par-all-inboxes-unfolds-one-row-per-account | the chevron saves the state but does not unfold
# J1.15/par-inbox-rows-under-group-captions | the captions are missing
# J1.15/par-inbox-row-count-is-the-accounts | an account row shows its count plus 1
# J1.15/par-inbox-row-filters-the-list | an account row click leaves the list unfiltered
# J1.15/par-filter-chip-names-the-account | the chip ignores the account filter
# J1.15/par-filter-picker-ticks-the-account | a filter click leaves the picker ticks empty
# J1.15/par-filter-accounts-bar-moves-all-three | the chip ignores the filter and the ticks stay empty
# J1.15/par-inbox-row-click-again-clears | a second click sets the filter again
# J1.15/par-filter-picker-moves-the-chip | the chip ignores the account filter
# J1.15/par-all-inboxes-row-clears-the-filter | the All inboxes click leaves the account filter on
# J1.15/par-all-inboxes-folds | the chevron only opens
# J1.15/par-all-inboxes-remembered | the saved state is always unfolded
# J1.15/par-section-mailboxes-folds | the section keeps its rows when folded
# J1.15/par-section-mailboxes-saved | a folded section is not saved
# J1.15/par-section-mailboxes-unfolds | a folded section cannot unfold
# J1.15/par-section-smart-folders-folds | the section keeps its rows when folded
# J1.15/par-section-smart-folders-saved | a folded section is not saved
# J1.15/par-section-smart-folders-unfolds | a folded section cannot unfold
# J1.15/par-section-labels-folds | the section keeps its rows when folded
# J1.15/par-section-labels-saved | a folded section is not saved
# J1.15/par-section-labels-unfolds | a folded section cannot unfold
# J1.1/par-count-zero-shows-none | Junk shows a count of 0
# J1.1/par-count-nonzero-shows | no count is ever shown
# J1.1/par-pin-glyph-on-pinned-labels | the rows report no label as pinned
# J1.1/par-pin-click-pins | the pin click does nothing
# J1.1/par-pin-menu-says-unpin | the menu always says Pin as a tab
# J1.1/par-pin-menu-unpins | the pin click does nothing
# J1.1/par-pin-menu-pins | the pin click does nothing
# J1.1/par-pin-click-unpins | the pin click does nothing
# J1.1/par-pin-hover-shows-the-pin | the pin is hidden while the pointer is over the row
# J1.1/par-scoped-label-carries-its-account | an account's label carries no account mark
# J1.1/par-scoped-label-hidden-without-its-filter | an account label shows under every filter
# J1.1/par-scoped-label-shows-under-its-filter | an account label never shows
# J1.1/par-scoped-label-hidden-under-another-account | an account label shows under every filter
# J1.1/par-drag-into-reparents | a drop on a row's middle does nothing
# J1.1/par-drag-before-reorders-the-core | a drop before a row lands after it
# J1.1/par-drag-after-reorders-the-core | a drop after a row lands two places later
# J1.1/par-drag-past-depth-limit-refused | the drop clears the refusal words
# J1.1/par-drag-held-then-outlined | lift does not hold the label
# J1.1/par-drag-over-names-the-drop | over names no row
# J1.1/par-drag-cancel-drops-nothing | cancel does not clear the held drag
# J1.1/par-keys-shown-accounts-bar | the accounts bar tooltip drops its key
# J1.1/par-smart-rename-reaches-the-core | Rename on a smart folder keeps the old name
# J1.1/par-smart-delete-removes-it | Delete on a smart folder deletes nothing
# J1.1/par-menu-delete-removes-label | Yes does not delete the label
# J1.1/par-views-built-in | the built-in views region is missing
# J1.1/par-go-key-all-inboxes | G then I selects nothing
# J1.1/par-go-key-starred | go-starred stays a stub
# J1.1/par-go-key-drafts | G then D selects Sent
# J1.1/par-go-key-sent | G then T selects Drafts
# J1.1/par-go-key-archive | G then E archives instead
# J1.1/par-go-key-snoozed | G then H selects nothing
# J1.1/par-go-key-muted | go-muted stays a stub
# J1.1/par-go-key-junk | G then ! selects nothing
# J1.1/par-go-key-trash | G then # selects nothing
# J1.1/par-go-key-all-mail | go-all-mail stays a stub
# J1.1/par-go-key-label | go-label stays a stub
# J1.1/par-go-key-other-coming-soon | G then O says nothing
# J1.1/par-go-key-snippets-coming-soon | G then ; says nothing
# J1.1/par-views-in-french | the views draw their English names in French
# J1.1/par-mailboxes-in-french | a mailbox title skips the catalogue
# J1.1/par-builtin-labels-in-french | a built-in label row carries its stored English name in a French UI
# J1.1/par-hidden-confirm-is-state | the hidden rig presents an alert whose refusal clears the ask
# J1.1/par-hidden-confirm-recorded-not-shown | the ask is neither shown nor recorded
# J1.1/par-hidden-confirm-no-keeps | No deletes the smart folder anyway
# J1.1/par-list-shows-starred | the sidebar selects Starred and leaves AppModel.mailbox unwritten (the list keeps the inbox)
# J1.1/par-list-shows-muted | as above, Muted
# J1.1/par-list-shows-archive | as above, Archive
# J1.1/par-list-shows-trash | as above, Trash
# J1.1/par-list-shows-junk | as above, Junk
# J1.1/par-list-shows-snoozed | as above, Snoozed
# J1.1/par-list-shows-all-mail | as above, All mail
# J1.1/par-list-shows-drafts | as above, Drafts
# J1.1/par-list-shows-sent | as above, Sent
# J1.1/par-all-inboxes-brings-the-inbox-back | All inboxes leaves the last mailbox in the list
# J1.1/par-smart-folder-opens-its-results | a smart-folder click leaves AppModel.smartFolderId unwritten (the list keeps Primary)
# J1.1/par-smart-folder-header-names-it | as above: the header keeps the account filter
# J1.1/par-smart-folder-count-follows-an-archive | the folder's count is a stored number, not the query's row count
# J1.1/par-smart-folder-left-for-the-inbox | All inboxes leaves the smart folder in the list
# J1.1/par-no-primary-row-under-labels | the Labels section draws a Primary row first (the legacy built-in label row is no longer filtered)
# J1.1/par-no-all-row-under-labels | the Labels section draws an All row after Primary
# J1.1/par-labels-start-at-needs-reply | the Labels section starts with a Primary row
# J1.1/par-pinned-labels-start-at-needs-reply | the Primary row is drawn pinned and leads the pinned labels
# J1.1/par-pin-never-offered-for-a-view | a Primary row draws a pin control
# J1.1/par-go-all-opens-the-all-view | go-all opens nothing (the tab stays where it was)
# J1.1/par-go-i-opens-primary | go-inbox leaves the tab on All
# J1.1/par-go-views-open-state-follows | the go-views row reports the view as closed
# J1.1/par-go-i-shows-its-key | the go-views row for go-inbox shows no key
# J1.1/par-go-all-key-is-never-a-chord | the preset binds go-all to the two-modifier chord ⇧⌘A
# J1.1/par-view-pin-refused-in-words | a pin on a view is let through to the core and shows its debug text, not the contract's words
# J1.1/par-view-pin-refusal-pins-nothing | the refused pin on a view pins another label
# J1.1/par-view-pin-refusal-reaches-the-failure-line | the refusal is kept in the sidebar and never reaches the failure line
# J1.1/par-view-all-pin-refused-in-words | a pin on All is refused with the Primary sentence
# J1.1/par-view-names-in-french | the go-views rows draw their English names in French
# J1.1/par-label-click-selects-its-tab | a click on a pinned label moves the highlight and leaves the strip on its tab
# J1.1/par-label-click-moves-the-reader | a click on a pinned label leaves the reader on the message it showed before
# J1.1/par-label-click-filters-the-list | a click on an unpinned label leaves the list on every message
# J1.1/par-label-click-reader-shows-the-first-row | a click on an unpinned label leaves the reader on the message it showed before
# J1.1/par-label-click-list-is-the-labels-messages | the list thins one page of the All view instead of reading the core's rows for the label (a row the label does not carry stays)
# J1.1/par-label-click-all-inboxes-gives-the-tab-back | All inboxes leaves the list on the pinned label's tab
# J1.1/par-label-click-ends-with-all-inboxes | All inboxes leaves the label filter on

# J1.57/every-door-of-the-sidebar-is-in-the-control-table | a Sidebar file registers a click door the control table does not name
# J1.57/label-redraw-waits-for-quiet-mail | the label rows' redraw is asked with a main-queue block, which runs before the key's frame is shown
# J1.57/every-control-the-sidebar-draws-has-a-door | a sidebar view gives a control an id that no door and no display-only line names
# J1.57/header-new-smart-folder-opens-its-prompt | New smart folder… on the Smart folders menu opens nothing
# J1.57/prompt-cancel-closes-it-and-keeps-the-folders | Cancel on the prompt creates a smart folder
# J1.57/each-mailbox-and-view-hide-item-hides-its-row | Hide on the VIPs row's menu does nothing
# J1.57/each-mailbox-and-view-account-info-item-pulls-the-menu-up | Account info on the Sent row's menu does nothing
# J1.57/show-hidden-item-is-drawn-only-while-something-is-hidden | the Show hidden mailboxes item is drawn when nothing is hidden
# J1.57/show-hidden-item-brings-the-hidden-mailboxes-back | the Show hidden mailboxes item leaves the mailboxes hidden
# J1.57/a-mail-read-in-the-list-redraws-the-label-count | a mail opened in the list leaves the label's row drawing its old count
# J1.57/label-mark-all-read-item-clears-its-unread | Mark all as read on a label's menu marks nothing (the stand-in's first label is Needs Reply, a built-in that is a tab: the live run's first label with unread)
# J1.57/each-account-tick-unticks-and-reticks | one account's tick in the Mailboxes picker does nothing
# J1.57/outbox-door-selects-the-outbox | a click on the Outbox row leaves the highlight where it was
# J1.57/outbox-selection-falls-back-when-the-send-leaves | the highlight stays on the Outbox row after its row is gone
# J1.57/hidden-picker-door-selects-its-mailbox | choosing Sent in the hidden sidebar's picker selects nothing
# J1.57/hidden-picker-door-selects-its-smart-folder | choosing a smart folder in the picker selects nothing
# J1.57/hidden-picker-all-inboxes-clears-the-account-filter | choosing All inboxes in the picker leaves the account filter on
# J1.57/empty-sidebar-offers-add-account | the empty sidebar draws no Add account button
# J1.57/empty-add-door-opens-the-add-account-surface | the empty state's Add account button opens nothing


def main():
    if "--selftest" in sys.argv:
        return selftest()
    path = os.path.abspath(__file__)
    step = sys.argv[sys.argv.index("--step") + 1] if "--step" in sys.argv else None
    if step is not None:
        if step != "J1.14":
            print("only --step J1.14 exists"); return 2
        sys.modules[__name__].drive = step_j114
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    print_cases(r)
    if step is not None:
        r["stray_mutant"] = []   # the other steps' mutants are not run in a one-step run
    elif not VISIBLE:
        r["stray_mutant"] = [m for m in r["stray_mutant"] if m not in VISIBLE_ONLY]   # owner-only cases: documented, not run here
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
