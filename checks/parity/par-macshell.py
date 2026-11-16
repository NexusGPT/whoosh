#!/usr/bin/env python3
"""Parity check: par-macshell (menu bar unread count, Dock icon, default mail app, text size, window per message).

Drives the RUNNING rig copy over the control socket (docs/CONTROL-SOCKET.md) through journeys/lib. Expected
values come from the core (`uistate`) or from macOS itself, never from the shell's own rows.
Step ids J0.n: parity rows, not a signed journey (the harness reads mutant rows only as J<n>.<m>).
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/par-macshell.py'
Not driven, on purpose: making the rig the owner's default mail app (it would change this Mac's mailto
handler); the check reads the handler macOS reports instead.
"""
import json
import os
import plistlib
import re
import shutil
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
RESULTS_PICS = os.path.join(os.environ.get("WHOOSH_RESULTS_DIR") or os.path.join(ROOT, "_captures", "results", "par-macshell"), "pics")


def keep_shot(s, name):
    """`shot <name>`, and a copy of the picture in the check's results folder (`_captures/results/par-macshell/pics/<name>.png`, or WHOOSH_RESULTS_DIR/pics): the run folder the
    app writes it to is deleted by the launcher when the run ends (main104: the six r9-foot pictures were gone before anyone read them). No KEEP_RUN_DIR is needed."""
    def go(n):
        r = s.j.ctl.shot(n)
        os.makedirs(RESULTS_PICS, exist_ok=True)
        shutil.copyfile(r["path"], os.path.join(RESULTS_PICS, n + ".png"))
    s._act(go, name)
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, Journey, print_cases, print_model_cases   # noqa: E402
from lib.ctl import Unreadable                                       # noqa: E402
from lib.fixture import const, MSG                                   # noqa: E402
sys.path.insert(0, os.path.join(ROOT, "checks", "checklib"))
import resetguard as RG                                              # noqa: E402
import titlebar as TB                                                # noqa: E402   (probes-r1: the shared title-bar case)

DRAG_STEPS = const("DRAG_STEPS")
LIST_W = const("LIST_W_PX")
LIST_W_COMPACT = const("LIST_W_COMPACT_PX")
LIST_W_WIDE = const("LIST_W_WIDE_PX")
LIST_W_WIDE_COMPACT = const("LIST_W_WIDE_COMPACT_PX")
READER_MIN_W = 400   # MacShellGeometry.readerMinW, macshell's own until the ledger carries it

ID = "par-macshell"
TITLE = "Mac parity: menu bar, Dock, mail handler, text size, message windows"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "the menu bar shows the unread count; the Dock icon hides and returns; text grows and shrinks; a message opens in its own window"


LANGUAGE = "mac-settings-general.appearance.language.%s"


def word(key, lang="fr"):
    with open(os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8") as f:
        return json.load(f)["strings"][key]["localizations"][lang]["stringUnit"]["value"]


def fr(key):
    return word(key, "fr")


def shell(s):
    return s.ctl.rows("mac-main.shell")[0]


BY_DESIGN = []   # checks not judged on purpose: the Dock tile is refused on every hidden rig copy (appshell round 3, uistate.hidden)


def dock_asked(s):
    """On a hidden rig the Dock tile can never be shown (it would sit on the owner's screen), but what the app does on a click CAN be judged: the copy runs as an accessory
    app (its Dock icon is hidden), `macshell.dock.toggle` therefore asks macOS for the tile (`setActivationPolicy(.regular)`, MacSystemBridges.toggleDock), and the rig
    refuses and counts it (`uistate.hidden.refused` gets one `dock-tile`, HiddenRigHooks.swift:27) and changes nothing. True: exactly one new `dock-tile` refusal and the
    `dock_hidden` flag as it was. A switch that does nothing leaves the count where it was (main105: both cases were not judged by design for four rounds)."""
    def refused():
        return (s.ctl.uistate().get("hidden", {}).get("refused") or []).count("dock-tile")
    n0, was = refused(), shell(s)["dock_hidden"]
    try:
        s.ctl.click("macshell.dock.toggle")
    except Unreadable as e:
        # the app's own answer to a refused tile: setActivationPolicy(.regular) returns false, the door says "did nothing" (main111): that IS the refusal, judged by the count below
        if "did nothing" not in str(e):
            raise
    return refused() - n0 == 1 and shell(s)["dock_hidden"] == was


def dock_judged(s, name, thunk):
    """A hidden rig: the Dock switch is judged by what it asks of macOS (`dock_asked`); a rig shown on screen: by the real icon (`thunk`)."""
    def run():
        if s.ctl.uistate().get("hidden", {}).get("on"):
            return dock_asked(s)
        return thunk()
    return run


def settle(fn, want, tries=40):
    """Polls fn until it equals want (the split animates for TAB_MORPH_MS); returns the last value."""
    import time
    v = fn()
    for _ in range(tries):
        if v == want:
            return v
        time.sleep(0.05)
        v = fn()
    return v


def win_w(s):
    return next(f["w"] for f in s.ctl.winframes() if f["title"] == "Whoosh" or f["w"] > 900)


def widths(s):
    r = shell(s)
    return round(r["list_w"]), round(r["reader_w"])


def all_inboxes(s):
    return s.ctl.uistate()["counts"]["all inboxes"]


def os_mail_handler():
    """The mailto handler macOS has on record (Launch Services); Apple Mail when none is set."""
    p = os.path.expanduser("~/Library/Preferences/com.apple.LaunchServices/com.apple.launchservices.secure.plist")
    try:
        out = subprocess.run(["plutil", "-convert", "xml1", "-o", "-", p], capture_output=True, check=True).stdout
        for h in plistlib.loads(out).get("LSHandlers", []):
            if h.get("LSHandlerURLScheme") == "mailto":
                return h.get("LSHandlerRoleAll", "").lower()
    except (OSError, subprocess.CalledProcessError, plistlib.InvalidFileException):
        return "com.apple.mail"
    return "com.apple.mail"


def account_digits(s):
    """J0.10's last two cases: Ctrl-1 and Ctrl-2 filter the list to the first and second account."""
    accounts = lambda: [a["id"] for a in s.ctl.rows("mac-main.sidebar.accounts")]
    for n in (1, 2):
        s.key("⌃%d" % n)
        # the account list is read INSIDE the check: read outside it (main76), a refused read escaped as Unreadable and stopped the whole run
        s.expect("ctrl-%d-filters-to-account-%d" % (n, n), lambda n=n: (lambda got, want: got == want or "filter %r, account %d is %r" % (got, n, want))(
            s.ctl.uistate()["account_filter"], accounts()[n - 1]), True)


def drive(j):
    s = j.step("J0.1", "Menu bar item with the unread count")
    s.expect("menubar-visible", lambda: shell(s)["menubar_visible"], True)
    # the menu bar's number is All inboxes, and All inboxes is the All tab's unread (the inbox's own figure): three places, one number. On the real core the
    # "all inboxes" key was not filled and the menu bar summed every tab (the lead's read of counts.rs, 14 Nov 2026)
    s.expect("menubar-count", lambda: (shell(s)["menubar_title"], str(all_inboxes(s))) == (str(s.ctl.uistate()["counts"]["all"]),) * 2, True)

    s = j.step("J0.2", "Hide the Dock icon, then show it again")
    before = {}
    s.expect("dock-toggles", lambda: dock_judged(s, "dock-toggles", lambda: (before.setdefault("v", shell(s)["dock_hidden"]), s.ctl.click("macshell.dock.toggle"),
                                      shell(s)["dock_hidden"])[2] != before["v"])(), True)
    s.expect("dock-returns", lambda: dock_judged(s, "dock-returns", lambda: (s.ctl.click("macshell.dock.toggle"), shell(s)["dock_hidden"])[1] == before["v"])(), True)

    s = j.step("J0.3", "Default mail app as macOS reports it")
    s.expect("mail-handler-matches-os", lambda: shell(s)["default_mail_app"].lower() == os_mail_handler(), True)

    s = j.step("J0.4", "Text size: ⌘+ grows, ⌘− shrinks, ⌘0 resets")
    s.key("⌘+")
    s.expect("bigger", lambda: shell(s)["text_scale"] > 1, True)
    s.key("⌘−")
    s.key("⌘−")
    s.expect("smaller", lambda: shell(s)["text_scale"] < 1, True)
    s.key("⌘0")
    s.expect("reset", lambda: shell(s)["text_scale"], 1)

    s = j.step("J0.5", "Window per message")
    s.click("mac-main.message.window")
    subject = lambda: s.ctl.rows("mac-main.list")[s.ctl.uistate()["focus_index"]]["subject"]
    s.expect("window-titled-with-subject", lambda: subject() in [f["title"] for f in s.ctl.winframes()], True)

    s = j.step("J0.6", "Drag: a label dragged onto another label in the labels screen moves under it")
    LAB = "mac-labels-contacts"
    s.raw("shot", LAB)
    s.raw("activate")   # the app comes forward once the verb has returned; the click below gives it that turn
    s.click(LAB + ".show.labels")
    # user labels only: the core lists its five built-in categories (primary, needs-reply, ...) first, and they never move
    tops = lambda: [r for r in s.ctl.rows(LAB + ".labels") if r["parent"] is None and r["id"].startswith("lbl-")]
    moved = {}

    def drag_moves_label():
        a, b = tops()[1], tops()[2]   # not the first: its subtree is 5 deep and the screen refuses a 6th level (LABEL_DEPTH_MAX)
        reply = s.ctl.send("drag", a["ax"], b["ax"])
        moved["reply"] = reply
        now = {r["id"]: r["parent"] for r in s.ctl.rows(LAB + ".labels")}
        return now[a["id"]] == b["id"]
    s.expect("drag-reparents-the-label", drag_moves_label, True)
    s.expect("drag-delivers-every-event", lambda: moved["reply"]["received"] if "reply" in moved else (_ for _ in ()).throw(Unreadable("the drag case before this one sent no drag")), DRAG_STEPS + 2)

    s = j.step("J0.7", "The status line names the failing account and opens Settings: Accounts")
    s.expect("status-names-failing-account", lambda: "Provanta RFP" in s.ctl.rows("mac-main.status")[0]["text"], True)
    # the core's reason is English words; the line draws them in the app's language, the account's name stays the owner's
    s.click(LANGUAGE % "fr")
    s.expect("status-reason-in-french", lambda: s.ctl.rows("mac-main.status")[0]["text"], "Provanta RFP " + fr("needs you to sign in again"))
    s.click(LANGUAGE % "en")
    s.click("mac-main.status")
    s.expect("status-opens-settings-accounts", lambda: s.ctl.uistate()["surface"], "mac-settings-accounts")
    s.expect("settings-window-is-up", lambda: "Settings" in [f["title"] for f in s.ctl.winframes()], True)
    RG.step_reset(s)
    s.expect("reset-returns-to-main", lambda: s.ctl.uistate()["surface"], "mac-main")


    for (w, h, list_open, wide, tag) in ((1280, 820, LIST_W, LIST_W_WIDE, "1280"), (960, 632, LIST_W_COMPACT, LIST_W_WIDE_COMPACT, "960")):
        s = j.step("J0.8" if tag == "1280" else "J0.13", "Hidden sidebar at %sx%s: the list takes the wide width, the reader the rest" % (w, h))
        s.raw("resize", str(w), str(h))
        s.expect("shown-list-width-" + tag, lambda: settle(lambda: widths(s)[0], list_open), list_open)
        s.click("mac-main.sidebar.toggle")
        s.expect("hidden-list-width-" + tag, lambda: settle(lambda: widths(s)[0], wide), wide)
        s.expect("hidden-reader-takes-rest-" + tag, lambda: settle(lambda: abs(sum(widths(s)) - win_w(s)) <= 2 and widths(s)[1] >= READER_MIN_W, True), True)
        s.shot("r8-hidden-%s" % tag)
        s.click("mac-main.sidebar.toggle")
        s.expect("shown-again-restores-list-" + tag, lambda: settle(lambda: widths(s)[0], list_open), list_open)
    s.raw("resize", "1280", "820")

    s = j.step("J0.9", "Arrow keys move the keyboard focus between sidebar, list and reader")
    s.raw("activate")   # the keyboard focus is read from the key window: the app comes forward first
    s.click("mac-main.pane.list")
    s.key("Left")
    s.expect("left-focuses-sidebar", lambda: settle(lambda: (s.ctl.uistate()["first_responder"], shell(s)["focused_pane"]), ("sidebar", "sidebar")), ("sidebar", "sidebar"))
    s.expect("focus-ring-shown", lambda: shell(s)["focus_by_key"], True)
    s.key("Right")
    s.expect("right-focuses-list", lambda: shell(s)["focused_pane"], "list")
    s.key("Right")
    s.expect("right-again-focuses-reader", lambda: settle(lambda: (s.ctl.uistate()["first_responder"], shell(s)["focused_pane"]), ("reader", "reader")), ("reader", "reader"))
    s.click("mac-main.pane.list")
    s.expect("click-clears-key-ring", lambda: shell(s)["focus_by_key"], False)

    s = j.step("J0.10", "New window, text size, message link, account digits")
    try:
        base = shell(s)["main_windows"]
    except Unreadable:      # no socket (dry run): the checks below read through s.ctl and are not judged; the count stays exact
        base = 0
    s.key("⌘N")
    s.expect("new-window-opens-a-second-main-window", lambda: settle(lambda: shell(s)["main_windows"], base + 1) - base, 1)
    s.click("macshell.window.close-others")   # the copy carries the same row ids: later frame reads must see one main window
    s.expect("extra-window-closes", lambda: settle(lambda: shell(s)["main_windows"], base), base)
    s.key("⌘=")
    s.expect("cmd-equals-grows", lambda: shell(s)["text_scale"] > 1, True)
    s.key("⌘-")
    s.key("⌘-")
    s.expect("cmd-minus-shrinks", lambda: shell(s)["text_scale"] < 1, True)
    s.key("⌘0")
    s.expect("cmd-zero-resets", lambda: shell(s)["text_scale"], 1)
    s.key("⌃/")
    s.expect("ctrl-slash-copies-the-message-link", lambda: shell(s)["last_link"] == "whoosh://message/" + s.ctl.uistate()["focus_message"], True)
    account_digits(s)

    s = j.step("J0.11", "Window toolbar: ten controls, key in the tooltip, greyed with no selection, running the command")
    tb = lambda: {r["command"]: r for r in s.ctl.rows("mac-main.toolbar")}
    cmds = ["compose", "reply", "reply-all", "forward", "archive", "delete", "junk", "snooze", "label"]
    s.expect("toolbar-has-compose-to-more", lambda: [c for c in cmds if c in tb()] + (["more"] if tb().get("star", {}).get("in_menu") else []), cmds + ["more"])
    s.expect("tooltips-carry-their-key", lambda: all(len(tb()[c]["tooltip"].split("  ")) == 2 for c in ("reply", "archive", "snooze", "label")), True)
    s.expect("enabled-with-a-selection", lambda: tb()["archive"]["enabled"], True)
    n0 = len(s.ctl.rows("mac-main.list"))
    s.click("mac-main.toolbar.archive")
    s.expect("archive-button-archives", lambda: n0 - settle(lambda: len(s.ctl.rows("mac-main.list")), n0 - 1), 1)
    RG.step_reset(s)

    s = j.step("J0.12", "Hold ⌘: the key badges appear across the screen, release hides them")
    hints = lambda: s.ctl.rows("mac-main.hints")
    s.expect("no-badges-while-not-held", lambda: len(hints()), 0)
    s.click("macshell.hint.hold")
    s.expect("hold-shows-badges", lambda: settle(lambda: len(hints()) > 0, True), True)
    s.expect("row-action-badge-archive-E", lambda: [h["label"] for h in hints() if h["ax"].endswith(".act.archive")][:1], ["E"])
    s.expect("tab-badge-cmd-1", lambda: [h["label"] for h in hints() if h["ax"] == "mac-main.splits.0"], ["⌘1"])
    # the sidebar is its own pane: its rows report their frames a moment after the list's (seen at machine load 30+), and the hints follow them at the next tick
    s.expect("sidebar-badge-sequence", lambda: True if settle(lambda: any(h["label"] == "G I" for h in hints()), True) else "no G I badge; the sidebar badges are %s; all badges %d" % (
        [(h["ax"], h["label"]) for h in hints() if "sidebar" in h["ax"]][:8], len(hints())), True)
    s.shot("r8-hold-cmd")
    s.click("macshell.hint.release")
    s.expect("release-hides-badges", lambda: settle(lambda: len(hints()), 0), 0)

    for (w, h, step) in ((1280, 820, "J0.14"), (960, 632, "J0.15")):
        s = j.step(step, "Hold ⌘ at %sx%s: no badge touches another badge, a tab's count, the toolbar icons" % (w, h))
        s.raw("resize", str(w), str(h))
        s.click("mac-main.accounts.all")   # earlier steps leave one account filtered; the row-above check needs a full list
        s.key("K"); s.key("K"); s.key("K"); s.key("J")   # the second row: a row sits above the focused row's caps
        s.key("Enter")                                     # a thread open, as the owner looks at it; the focused row shows its strip
        time.sleep(0.5)                                    # the strip draws after the focus move; the caps are read once, at hold
        s.click("macshell.hint.hold")
        hs = lambda: s.ctl.rows("mac-main.hints")
        frame = lambda ax: s.ctl.send("measure", ax)["frame"]
        rect = lambda r: (r["x"], r["y"], r["x"] + r["w"], r["y"] + r["h"])
        hit = lambda a, b: a[0] < b[2] - 0.5 and b[0] < a[2] - 0.5 and a[1] < b[3] - 0.5 and b[1] < a[3] - 0.5   # half a point of slack for float edges
        flat = lambda: [h2 for h2 in hs() if not h2["ax"].endswith(".strip")]   # a pill holds its own pairs by design
        s.expect("badges-up-" + w.__str__(), lambda: settle(lambda: len(hs()) > 8, True), True)
        s.expect("no-two-badges-touch-" + str(w), lambda: [(a["ax"], b["ax"]) for i, a in enumerate(flat()) for b in flat()[i + 1:] if hit(rect(a), rect(b))], [])
        s.expect("tab-badges-clear-of-the-tab-count-" + str(w), lambda: [h2["ax"] for h2 in hs() if h2["ax"].startswith("mac-main.splits.")
                 and h2["y"] + h2["h"] > frame(h2["ax"])["y"] + 4], [])
        s.expect("toolbar-caps-under-the-icons-" + str(w), lambda: [h2["ax"] for h2 in hs() if h2["ax"].startswith("mac-main.toolbar.cap.")
                 and h2["y"] < frame("mac-main.toolbar")["y"] + frame("mac-main.toolbar")["h"]], [])
        def covered():
            ctrl = {h2["ax"]: frame(h2["ax"]) for h2 in hs() if h2["ax"].startswith("mac-main.splits.")}
            bar = frame("mac-main.toolbar")
            ctrl["mac-main.toolbar"] = bar
            box = lambda f: (f["x"], f["y"], f["x"] + f["w"], f["y"] + f["h"])
            return [(h2["ax"], c) for h2 in hs() for c, f in ctrl.items() if hit(rect(h2), box(f))]
        s.expect("no-badge-covers-a-control-" + str(w), covered, [])
        def row_text(rid):
            try:
                return frame("mac-main.list.row.%s.text" % rid)
            except Exception:
                return None
        rowids = lambda: [r["id"] for r in s.ctl.rows("mac-main.list")]
        # Row caps while ⌘ is held (owner 23:08, "floating better"): each cap floats directly above its own icon, no pill.
        acts = lambda: [h2 for h2 in hs() if ".act." in h2["ax"] and not h2["ax"].endswith(".icon")]   # a pill's icon copies are not caps
        own_id = lambda ax: ax[len("mac-main.list.row."):].split(".act.")[0]
        bands = lambda: s.ctl.rows("mac-main.hints.bands")
        box = lambda f: (f["x"], f["y"], f["x"] + f["w"], f["y"] + f["h"])
        def float_above():
            return caps_off_their_icons(acts(), frame, s.ctl.rows("mac-main.hints.glyphs"))
        def clear_of(own):
            hits = []
            if not own:   # the check is vacuous unless some row with caps has a row drawn above it
                tops = sorted(b["sender"]["y"] for b in bands())
                if not any(b["id"] == own_id(c["ax"]) and b["sender"]["y"] > tops[0] for c in acts() for b in bands()):
                    hits.append(("no row above the caps", len(tops)))
            for c in acts():
                for bd in bands():
                    if (bd["id"] == own_id(c["ax"])) != own:
                        continue
                    cr = rect(c) if own else (c["x"], c["y"] - 1.5, c["x"] + c["w"], c["y"] + c["h"] + 1.5)   # another row's text: 1 pt clear (+ hit()'s 0.5 slack)
                    for part in (("sender",) if own else ("sender", "subject")):
                        if bd[part]["w"] > 0 and hit(cr, box(bd[part])):
                            hits.append((c["ax"], bd["id"], part))
            return hits
        s.expect("row-caps-shown-" + str(w), lambda: len(acts()) >= 1, True)
        s.expect("no-pill-" + str(w), lambda: [h2["ax"] for h2 in hs() if h2["ax"].endswith(".strip")], [])
        s.expect("caps-float-above-their-own-icons-" + str(w), lambda: settle(float_above, [], tries=10), [])   # the list may still be moving
        s.expect("caps-clear-of-own-sender-" + str(w), lambda: clear_of(True), [])
        s.expect("caps-clear-of-other-rows-text-" + str(w), lambda: clear_of(False), [])
        def glyph_hits():
            g = {r["ax"]: r["glyph"] for r in s.ctl.rows("mac-main.hints.glyphs")}
            return [c["ax"] for c in acts() if c["ax"] not in g or hit(rect(c), box(g[c["ax"]]))]
        s.expect("caps-clear-of-own-icon-glyph-" + str(w), glyph_hits, [])
        s.expect("one-cap-per-toolbar-icon-" + str(w), lambda: len([h2 for h2 in hs() if h2["ax"].startswith("mac-main.toolbar.cap.")]) == len([r for r in s.ctl.rows("mac-main.toolbar") if not r["in_menu"] and len(r["tooltip"].split("  ")) == 2]) > 0, True)
        s.shot("r8-hold-%s" % w)
        s.click("macshell.hint.release")
        s.key("Escape")
    s.raw("resize", "1280", "820")

    s = j.step("J0.16", "Reply all: no cap and a disabled icon with no thread open; Enter as its cap with one open")
    tbr = lambda: {r["command"]: r for r in s.ctl.rows("mac-main.toolbar")}
    # the earlier steps may leave a thread (or its reply box, which Escape closes first) open: three Escapes bring the window to the plain list
    for _ in range(3):
        s.key("Escape")
    s.expect("reply-all-disabled-with-no-thread", lambda: tbr()["reply-all"]["enabled"], False)
    s.key("Enter")
    s.expect("thread-open", lambda: settle(lambda: s.ctl.uistate()["overlay"] is not None or s.ctl.uistate()["surface"] != "mac-main", True), True)
    s.expect("reply-all-enabled-in-a-thread", lambda: settle(lambda: tbr()["reply-all"]["enabled"], True), True)
    s.click("macshell.hint.hold")
    s.expect("reply-all-cap-is-enter", lambda: settle(lambda: [h["label"] for h in s.ctl.rows("mac-main.hints") if h["ax"] == "mac-main.toolbar.cap.reply-all"], ["Enter"]), ["Enter"])
    s.expect("all-nine-toolbar-icons-have-a-cap-in-a-thread", lambda: settle(lambda: len([h["ax"] for h in s.ctl.rows("mac-main.hints") if h["ax"].startswith("mac-main.toolbar.cap.")]), 9), 9)
    s.shot("r8-hold-thread")
    s.click("macshell.hint.release")
    s.key("Escape")   # A102: Enter in the list opened the thread AND the quick reply, so this Escape closes the reply field only
    s.key("Escape")   # and this one leaves the thread: J0.17 starts on the list, not on the composer's foot set (main58: foot-list-set read 'composer')
    s.expect("escape-twice-leaves-the-thread-for-the-list", lambda: settle(lambda: s.ctl.uistate()["surface"], "mac-main"), "mac-main")

    s = j.step("J0.17", "Foot strip: the focused surface's keys, from the live preset (owner 08:58, SPEC A78)")
    foot = lambda: s.ctl.rows("mac-main.foot.hints")
    cmds = lambda: [r["command"] for r in foot()]
    s.raw("resize", "1280", "820")
    s.click("mac-main.accounts.all")
    s.click("mac-main.pane.list")
    # A105/A106: the list and the reader show E Mark done, R, Reply all (only where the preset has a key: Enter in an open thread), F, H, L, ⌘K,
    # then the last item of every state, the ⌘ cap with "Hold for all shortcuts". Words from the catalogue (rig language en), keys from the preset.
    LIST_SET = [("archive", "E", "Mark done"), ("reply", "R", "Reply"), ("forward", "F", "Forward"), ("snooze", "H", "Snooze"),
                ("label", "L", "Label"), ("search", "⌘K", "Search")]
    HOLD = ("hold-cmd", "⌘", "Hold for all shortcuts")
    shown = lambda: [(r["command"], r["key"], r["words"]) for r in foot() if r["shown"]]
    want = lambda rows: [(c, k, word(w, "en")) for c, k, w in rows]
    s.expect("foot-list-set", lambda: settle(lambda: (foot()[0]["surface"], shown()), ("list", want(LIST_SET + [HOLD]))),
             ("list", want(LIST_SET + [HOLD])))
    s.click("mac-main.list.row.0")
    s.click("mac-main.reader.open")                  # a thread open, no reply under it yet: the reader has its own keys
    s.click("mac-main.pane.reader")
    READER_SET = [("back", "Escape", "Back to list")] + LIST_SET[:2] + [("reply-all", "Enter", "Reply all")] + LIST_SET[2:]   # Esc goes back to the list while a mail is open (owner 2026-11-13)
    s.expect("foot-reader-set", lambda: settle(lambda: (foot()[0]["surface"], shown()), ("reader", want(READER_SET + [HOLD]))),
             ("reader", want(READER_SET + [HOLD])))
    s.expect("foot-hold-item-last", lambda: settle(lambda: foot()[-1]["command"], "hold-cmd"), "hold-cmd")
    s.key("R")                                       # quick reply open: the composer's keys, the hold item last
    # the composer set holds ai-reply "Draft" since 7674d245: Sources/Features/MacShell/MacFootHints.swift:29
    s.expect("foot-composer-set", lambda: settle(lambda: (foot()[0]["surface"], cmds()), ("composer", ["send", "ai-reply", "discard-draft", "popout-draft", "hold-cmd"])),
             ("composer", ["send", "ai-reply", "discard-draft", "popout-draft", "hold-cmd"]))
    s.key("Escape")
    s.key("Escape")
    s.click("mac-main.pane.list")
    def keys_follow(preset):
        s.ctl.click("mac-shortcuts.preset." + preset)
        want = {}
        for r in s.ctl.rows("mac-shortcuts.preset"):
            want.setdefault(r["command"], r["key"])
        got = {r["command"]: r["key"] for r in foot()}
        return [(c, k, want.get(c)) for c, k in got.items() if c in want and want[c] != k]
    s.expect("foot-keys-follow-preset", lambda: keys_follow("gmail"), [])
    s.click("mac-shortcuts.preset.superhuman")
    keep_shot(s, "r9-foot-list-1280")
    # The finish line's "fewer at 960 pt": the strip keeps the hints that fit the list and reader panes (MacFootHints.fitting), in order, so the narrow
    # window shows a shorter prefix of the same row list, which still ends on the hold item.
    memo = {}
    s.expect("foot-shows-all-seven-at-1280", lambda: memo.setdefault("n", settle(lambda: len([r for r in foot() if r["shown"]]), len(LIST_SET) + 1)), len(LIST_SET) + 1)
    s.raw("resize", "960", "632")
    # the strip fits the list and reader panes (MacFootHints.width): the narrowing is only meant with the sidebar and the list drawn, so ask for them and say what
    # the shell holds when it still shows as many (main92: 7 at 960 as at 1280, no width in the line)
    def fits_at_960():
        for _ in range(2):
            sh = shell(s)
            if sh.get("sidebar_collapsed"):
                s.click("mac-main.sidebar.toggle")
        settle(lambda: len([r for r in foot() if r["shown"]]), len([r for r in foot() if r["shown"]]))
        sh = shell(s)
        shown = len([r for r in foot() if r["shown"]])
        # main93 read: 7 of 7 shown at 960 with panes sidebar 200 + list 320 + reader 438: the strip is list + reader less its insets = 726 pt, and the seven
        # hints of the list set need less than that, so showing all of them IS the fit (MacFootHints.fitting keeps every hint that fits, in order). The old
        # claim "fewer at 960" assumed hints wider than the panes; no SPEC line asks the strip to drop a hint that fits. What must hold at the smallest window:
        # never more than at 1280, at least the first hint, and the strip is a prefix that ends on the hold item (the two cases after this one).
        strip_w = (sh.get("list_w") or 0) + (sh.get("reader_w") or 0) - 32
        ok = 0 < shown <= memo.get("n", 0)
        return True if ok else "shown %d of %d at 960 (1280: %s); strip %.0f pt" % (shown, len(foot()), memo.get("n"), strip_w)
    s.expect("foot-keeps-the-hints-that-fit-at-960", fits_at_960, True)
    s.expect("foot-hold-item-is-still-the-last-row-at-960", lambda: foot()[-1]["command"], "hold-cmd")
    # Lead, 13 Nov: the hold item is the strip's last item and always shown (the case above); the hints before it are a prefix of their order.
    # The old form compared the whole list, hold item included, so it held only while every hint fitted (main136: the sixth hint no longer fits).
    def prefix_at_960():
        shown = [r["shown"] for r in foot()]
        return (shown[:-1] == sorted(shown[:-1], reverse=True), shown[-1])
    s.expect("foot-shown-hints-are-a-prefix-at-960", prefix_at_960, (True, True))
    s.raw("resize", "1280", "820")
    # Light and dark pictures of the three states (the list, the reader, the composer) at 1280x820, for the lead to read.
    for theme in ("light", "dark"):
        s.raw("theme", theme)
        s.click("mac-main.pane.list")
        keep_shot(s, "r9-foot-list-1280-" + theme)
        s.click("mac-main.list.row.0")
        s.click("mac-main.reader.open")
        s.click("mac-main.pane.reader")
        keep_shot(s, "r9-foot-reader-1280-" + theme)
        s.key("R")
        keep_shot(s, "r9-foot-composer-1280-" + theme)
        s.key("Escape")
        s.key("Escape")
    s.raw("theme", "light")
    s.click("mac-main.pane.list")

    # ⇧R is in Superhuman's v8 list twice (docs/reference/superhuman-shortcuts-v8.json: Filters "No Reply" and Pop Out "Pop Out Reply"):
    # one chord, one meaning per place. The list filters; an open thread pops the reply out (last: it leaves a window behind).
    s = j.step("J0.18", "⇧R: the No Reply filter from the list, the reply pop-out from an open thread")
    resolved = lambda: s.ctl.rows("mac-shortcuts.dispatch")[-1]["resolved"]
    RG.step_reset(s)
    s.key("Escape")
    s.click("mac-main.accounts.all")
    s.click("macshell.window.close-others")
    s.key("⇧R")
    s.expect("shift-r-from-the-list-is-the-no-reply-filter", lambda: resolved(), "filter-no-reply")
    s.key("⇧R")                                      # the same key again takes the filter off
    s.click("mac-main.list.row.0")
    s.click("mac-main.reader.open")
    s.key("⇧R")
    s.expect("shift-r-in-an-open-thread-pops-out-the-reply", lambda: resolved(), "popout-reply")

    # A106: the ⌘ key-up is lost when the app or a window loses the focus, so the caps stayed. `app-active off|on` runs the handlers a real
    # focus change runs (the hidden rig never moves the real focus); with no ⌘ down on the live flags the caps go either way.
    s = j.step("J0.19", "Held-⌘ caps never stay stuck: the app losing or regaining the focus hides them")
    hints = lambda: s.ctl.rows("mac-main.hints")
    RG.step_reset(s)
    s.key("Escape")
    s.click("macshell.window.close-others")
    s.click("macshell.hint.hold")
    s.expect("caps-up-while-held", lambda: settle(lambda: len(hints()) > 0, True), True)
    s.raw("app-active", "off")
    s.expect("caps-hide-when-the-app-resigns-active", lambda: settle(lambda: len(hints()), 0), 0)
    s.click("macshell.hint.hold")
    s.expect("caps-up-again", lambda: settle(lambda: len(hints()) > 0, True), True)
    s.raw("app-active", "on")
    s.expect("caps-hide-when-the-app-returns-without-cmd", lambda: settle(lambda: len(hints()), 0), 0)
    # The three exits `app-active` does not reach, each through the code a real event runs (MacHintMonitor.simulateHold/Release/WindowResign/EventWithoutCommand,
    # Sources/Features/MacShell/MacHintOverlay.swift): the flags route that installs the key monitor, then ⌘ up, a window losing the key, a key-up without ⌘.
    s.click("macshell.hint.hold-by-flags")
    s.expect("caps-up-by-the-flags-route", lambda: settle(lambda: len(hints()) > 0, True), True)
    s.click("macshell.hint.release-by-flags")
    s.expect("caps-hide-when-cmd-comes-up-in-the-flags", lambda: settle(lambda: len(hints()), 0), 0)
    s.click("macshell.hint.hold-by-flags")
    s.expect("caps-up-before-a-window-resigns", lambda: settle(lambda: len(hints()) > 0, True), True)
    s.click("macshell.hint.window-resign")
    s.expect("caps-hide-when-a-window-resigns-key", lambda: settle(lambda: len(hints()), 0), 0)
    s.click("macshell.hint.hold-by-flags")
    s.expect("caps-up-before-an-event-without-cmd", lambda: settle(lambda: len(hints()) > 0, True), True)
    s.click("macshell.hint.event-without-cmd")
    s.expect("caps-hide-on-an-event-without-cmd", lambda: settle(lambda: len(hints()), 0), 0)

    doors_shell_step(j)
    sweep_step(j)
    titlebar_step(j)


# ---------------------------------------------------------------------------------------------------------------------
# doors-list: the window toolbar and the foot of the window (the sidebar's sync line, the foot strip), every control clicked once through its door.
# The door table names each id MacShellFeature.swift registers, the control it stands for and the case that clicks it. The "every door" check reads
# the file and compares, so a control cannot be added or lost without the table saying so.
SHELL_DOORS = {
    "mac-main.status": "the sync line at the sidebar foot, whole line: J0.7/status-opens-settings-accounts",
    "mac-main.status.link": "the failing account's link in that line: J0.20/status-link-door-opens-settings-accounts",
    "mac-main.sidebar.toggle": "hide or show the sidebar (the key and the palette row): J0.8",
    "mac-main.list.toggle": "hide or show the message list (the key and the palette row): J0.20/list-toggle-door-hides-and-shows-the-list",
    "mac-main.pane.*": "a pane's click (it takes the focus): J0.20/pane-door-<pane>-gives-it-the-focus, J0.9 and J0.17",
    "mac-main.toolbar.*": "the toolbar's nine buttons and the five items of its More menu: J0.11 (archive), J0.20 (the other thirteen)",
    "mac-main.message.window": "a message in its own window: J0.5",
    "macshell.window.close-others": "closes the extra main windows (a rig door, no control)",
    "macshell.dock.toggle": "the Dock icon switch (Settings: General): J0.2",
    "macshell.mail.make-default": "make Whoosh the default mail app: NOT clicked, by design (it changes the owner's mailto handler); J0.3 reads the handler",
    "macshell.menubar.toggle": "the menu bar item switch (Settings: General): J0.20/menubar-toggle-door-hides-and-shows-the-item",
    "macshell.hint.hold": "hold the Command key (the socket cannot): J0.12",
    "macshell.hint.release": "release it: J0.12",
    "macshell.hint.hold-by-flags": "hold ⌘ through the flags route that installs the key monitor (a rig door): J0.19",
    "macshell.hint.release-by-flags": "⌘ up through the flags route (a rig door): J0.19/caps-hide-when-cmd-comes-up-in-the-flags",
    "macshell.hint.window-resign": "a window losing the key (a rig door): J0.19/caps-hide-when-a-window-resigns-key",
    "macshell.hint.event-without-cmd": "a key-up with no ⌘ in its flags (a rig door): J0.19/caps-hide-on-an-event-without-cmd",
    "macshell.dev.shots": "dev only (a contact-sheet run), no control",
}


def shell_door_patterns(text):
    a = text.index("static func clickTargets()")
    b = text.index("static func paletteCommands()")
    return sorted(set(re.sub(r"\\\([^)]*\)", "*", x) for x in re.findall(r'"((?:mac-main|macshell)\.[^"]+)"', text[a:b])))


def shell_sources():
    return {"feature": open(os.path.join(ROOT, "Sources", "Features", "MacShell", "MacShellFeature.swift"), encoding="utf-8").read()}


def lpoll(fn, want, instant, tries=30):
    v = fn()
    for _ in range(tries):
        if v == want or instant:
            break
        time.sleep(0.05)
        v = fn()
    return v


def doors_shell_step(j, src=None):
    """J0.20 (doors-list): the toolbar's controls no case clicked before (thirteen of fourteen), the sync line's link, the list toggle, the panes, the
    menu bar switch. Each is clicked through its door and its visible outcome read. The foot strip's key hints are display only: a click on one is refused."""
    src = src or shell_sources()
    s = j.step("J0.20", "doors-list: every control of the toolbar and the foot of the window clicked once")
    inst = getattr(j.ctl, "instant", False)
    s.expect("every-door-of-the-shell-is-in-the-control-table", lambda: (sorted(set(shell_door_patterns(src["feature"])) - set(SHELL_DOORS)),
                                                                          sorted(set(SHELL_DOORS) - set(shell_door_patterns(src["feature"])))), ([], []))
    RG.step_reset(s)
    s.key("Escape")
    s.click("mac-main.accounts.all")
    s.click("macshell.window.close-others")
    lids = lambda: [r["id"] for r in s.ctl.rows("mac-main.list")]
    # rows are found by what the served list says (subject, sender), not by a literal id an earlier step may have removed (tests-r58 prune)
    lrow = lambda mid: RG.pick(s.ctl.rows("mac-main.list"), MSG[mid]) or next(r for r in s.ctl.rows("mac-main.list") if r["id"] == mid)
    def pick(mid):
        try:
            i = RG.row_index(s.ctl.rows("mac-main.list"), MSG[mid])
        except Unreadable as e:           # an action never checks: the case after it reads the missing effect and is not judged / red on it
            s.j.notes.append("%s pick %s: %s" % (s.id, mid, e)); return
        if i is None:                      # rows that carry no subject (the stand-in) are found by id, the old way
            i = next((k for k, r in enumerate(s.ctl.rows("mac-main.list")) if "subject" not in r and r["id"] == mid), None)
        if i is None:
            s.j.notes.append("%s pick %s: the served list holds no row saying %r" % (s.id, mid, MSG[mid]["subject"])); return
        s.click("mac-main.list.row.%d" % i)
    core = lambda path: s.ctl.core(path)
    ui = lambda: s.ctl.uistate()
    foot = lambda: s.ctl.rows("mac-main.foot.hints")[0]["surface"]
    pending = lambda: s.ctl.rows("mac-main.list.state")[0]["pending"]

    def tool(cmd, mid):
        pick(mid)
        s.click("mac-main.toolbar." + cmd)

    tool("compose", "msg-06")
    s.expect("toolbar-compose-opens-the-composer", lambda: lpoll(lambda: "mac-composer" in (ui()["surface"], ui()["overlay"]), True, inst), True)
    RG.step_reset(s)
    tool("reply", "msg-06")
    s.expect("toolbar-reply-opens-the-reply-field", lambda: lpoll(foot, "composer", inst), "composer")
    RG.step_reset(s)
    pick("msg-06")
    s.key("Enter")                                                    # a thread open: Reply all is enabled
    s.click("mac-main.toolbar.reply-all")
    qrh = lambda: (s.ctl.rows("mac-main.quickreply.header") or [{}])[0]
    s.expect("toolbar-reply-all-opens-the-reply-box-with-everyone", lambda: lpoll(lambda: (qrh().get("open"), bool(qrh().get("to"))), ("1", True), inst), ("1", True))   # the visible outcome: the box is open with its header filled
    RG.step_reset(s)
    tool("forward", "msg-06")
    fwd = lambda: (s.ctl.rows("mac-composer.popout") or [{}])[0]
    s.expect("toolbar-forward-opens-the-forward-window", lambda: lpoll(lambda: (fwd().get("open"), fwd().get("kind"), str(fwd().get("subject", "")).startswith("Fwd: ")), (True, "forward", True), inst), (True, "forward", True))   # the visible outcome: the forward window is up
    RG.step_reset(s)
    tool("delete", "msg-09")
    s.expect("toolbar-delete-deletes-the-message", lambda: lpoll(lambda: core("message/msg-09/deleted"), True, inst), True)
    RG.step_reset(s)
    tool("junk", "msg-07")
    s.expect("toolbar-junk-moves-the-message-to-junk", lambda: lpoll(lambda: core("message/msg-07/spam"), True, inst), True)
    RG.step_reset(s)
    tool("snooze", "msg-04")
    s.expect("toolbar-snooze-opens-the-snooze-picker", lambda: lpoll(pending, "snooze", inst), "snooze")
    s.key("Escape")
    tool("label", "msg-04")
    s.expect("toolbar-label-opens-the-label-picker", lambda: lpoll(pending, "label", inst), "label")
    s.key("Escape")
    RG.step_reset(s)
    pick("msg-05")
    was = {}
    s.expect("fixture-row-is-unread", lambda: was.setdefault("unread", lrow("msg-05")["unread"]), True)
    s.click("mac-main.toolbar.unread")
    s.expect("toolbar-more-unread-flips-the-read-state", lambda: lpoll(lambda: lrow("msg-05")["unread"], False, inst), False)
    tool("star", "msg-06")
    s.expect("toolbar-more-star-stars-the-message", lambda: lpoll(lambda: lrow("msg-06")["starred"], True, inst), True)
    tool("mute", "msg-08")
    s.expect("toolbar-more-mute-mutes-the-message", lambda: lpoll(lambda: core("message/msg-08/muted"), True, inst), True)
    tool("move", "msg-02")
    s.expect("toolbar-more-move-opens-the-move-picker", lambda: lpoll(lambda: ui()["overlay"], "move", inst), "move")
    s.key("Escape")
    tool("pin", "msg-04")
    s.expect("toolbar-more-pin-pins-the-message", lambda: lpoll(lambda: lrow("msg-04")["pinned"], True, inst), True)
    RG.step_reset(s)
    # the foot of the window: the sync line's link, the foot strip, the panes, the list toggle, the menu bar item
    s.click("mac-main.status.link")
    s.expect("status-link-door-opens-settings-accounts", lambda: lpoll(lambda: ui()["surface"], "mac-settings-accounts", inst), "mac-settings-accounts")
    RG.step_reset(s)
    hint = lambda: s.ctl.rows("mac-main.foot.hints")[0]["command"]
    refused = lambda: not accepted_click(s, "mac-main.foot.hint." + hint())
    s.expect("foot-hints-are-display-only-and-say-so", refused, True)
    shell = lambda: s.ctl.rows("mac-main.shell")[0]
    s.click("mac-main.list.toggle")
    s.expect("list-toggle-door-hides-the-list", lambda: lpoll(lambda: shell()["list_collapsed"], True, inst), True)
    s.click("mac-main.list.toggle")
    s.expect("list-toggle-door-shows-it-again", lambda: lpoll(lambda: shell()["list_collapsed"], False, inst), False)
    for p in ("sidebar", "reader", "list"):
        s.click("mac-main.pane." + p)
        s.expect("pane-door-%s-gives-it-the-focus" % p, lambda p=p: lpoll(lambda: shell()["focused_pane"], p, inst), p)
    s.click("macshell.menubar.toggle")
    s.expect("menubar-toggle-door-hides-the-item", lambda: lpoll(lambda: shell()["menubar_visible"], False, inst), False)
    s.click("macshell.menubar.toggle")
    s.expect("menubar-toggle-door-shows-it-again", lambda: lpoll(lambda: shell()["menubar_visible"], True, inst), True)
    RG.step_reset(s)


def accepted_click(s, ax):
    """True when the app answers the click ok; a refusal ("nothing registered at the id") is False."""
    try:
        s.ctl.send("click", ax)
        return True
    except Unreadable:
        return False


# doors-list mutants (J0.20): per check, the mutation of the app that must turn it red. The toolbar's cases all turn red under dead-toolbar.
# J0.20/every-door-of-the-shell-is-in-the-control-table | MacShellFeature.swift registers a door the control table does not name
# J0.20/toolbar-compose-opens-the-composer | the Compose button runs nothing (WHOOSH_MUTANT_R8=dead-toolbar)
# J0.20/toolbar-reply-opens-the-reply-field | the Reply button runs nothing (WHOOSH_MUTANT_R8=dead-toolbar)
# J0.20/toolbar-reply-all-opens-the-reply-box-with-everyone | the Reply all button opens no reply box with the recipients (WHOOSH_MUTANT_R8=dead-toolbar)
# J0.20/toolbar-forward-opens-the-forward-window | the Forward button opens no forward window (WHOOSH_MUTANT_R8=dead-toolbar)
# J0.20/toolbar-delete-deletes-the-message | the Delete button runs nothing (WHOOSH_MUTANT_R8=dead-toolbar)
# J0.20/toolbar-junk-moves-the-message-to-junk | the Junk button runs nothing (WHOOSH_MUTANT_R8=dead-toolbar)
# J0.20/toolbar-snooze-opens-the-snooze-picker | the Snooze button runs nothing (WHOOSH_MUTANT_R8=dead-toolbar)
# J0.20/toolbar-label-opens-the-label-picker | the Label button runs nothing (WHOOSH_MUTANT_R8=dead-toolbar)
# J0.20/fixture-row-is-unread | the fixture row msg-05 is read before the click
# J0.20/toolbar-more-unread-flips-the-read-state | the Mark read or unread item runs nothing (WHOOSH_MUTANT_R8=dead-toolbar)
# J0.20/toolbar-more-star-stars-the-message | the Star item runs nothing (WHOOSH_MUTANT_R8=dead-toolbar)
# J0.20/toolbar-more-mute-mutes-the-message | the Mute item runs nothing (WHOOSH_MUTANT_R8=dead-toolbar)
# J0.20/toolbar-more-move-opens-the-move-picker | the Move item runs nothing (WHOOSH_MUTANT_R8=dead-toolbar)
# J0.20/toolbar-more-pin-pins-the-message | the Pin item runs nothing (WHOOSH_MUTANT_R8=dead-toolbar)
# J0.20/status-link-door-opens-settings-accounts | the status link's click does nothing
# J0.20/foot-hints-are-display-only-and-say-so | a door answers on a foot hint and does nothing (the click is accepted, the strip unchanged)
# J0.20/list-toggle-door-hides-the-list | the list toggle door flips nothing
# J0.20/list-toggle-door-shows-it-again | the second click leaves the list hidden
# J0.20/pane-door-sidebar-gives-it-the-focus | the sidebar pane's click leaves the focus where it was
# J0.20/pane-door-reader-gives-it-the-focus | the reader pane's click leaves the focus where it was
# J0.20/pane-door-list-gives-it-the-focus | the list pane's click leaves the focus where it was
# J0.20/menubar-toggle-door-hides-the-item | the menu bar switch door flips nothing
# J0.20/menubar-toggle-door-shows-it-again | the second click leaves the item hidden


# MUTANTS: per check, the mutation of the app that must turn it red.
# J0.16/escape-twice-leaves-the-thread-for-the-list | the second Escape leaves the thread open, so the next step starts inside the reader
# J0.17/foot-list-set | the strip shows the old list set, Archive Snooze Compose Search Palette, and no last item (WHOOSH_MUTANT_R8=old-set)
# J0.17/foot-hold-item-last | the strip ends on its last key, no Hold for all shortcuts item (WHOOSH_MUTANT_R8=no-hold-hint)
# J0.19/caps-up-while-held | holding shows no caps (WHOOSH_MUTANT_R8=no-hints)
# J0.19/caps-hide-when-the-app-resigns-active | the caps stay after the app loses the focus (WHOOSH_MUTANT_R8=caps-stuck)
# J0.19/caps-up-again | holding shows no caps the second time (WHOOSH_MUTANT_R8=no-hints)
# J0.19/caps-hide-when-the-app-returns-without-cmd | the caps stay when the app is active again and ⌘ is not down (WHOOSH_MUTANT_R8=caps-stuck)
# J0.19/caps-up-by-the-flags-route | holding through the flags route shows no caps (WHOOSH_MUTANT_R8=no-hints)
# J0.19/caps-up-before-a-window-resigns | holding through the flags route shows no caps (WHOOSH_MUTANT_R8=no-hints)
# J0.19/caps-up-before-an-event-without-cmd | holding through the flags route shows no caps (WHOOSH_MUTANT_R8=no-hints)
# J0.19/caps-hide-when-cmd-comes-up-in-the-flags | the ⌘-up flags change no longer ends the hold (the else branch of MacHintMonitor.flagsChanged emptied: the proof file's mutant 3)
# J0.19/caps-hide-when-a-window-resigns-key | the caps stay after a window loses the key (WHOOSH_MUTANT_R8=caps-stuck)
# J0.19/caps-hide-on-an-event-without-cmd | the caps stay when an event without ⌘ arrives (WHOOSH_MUTANT_R8=caps-stuck)
# J0.17/foot-shows-all-seven-at-1280 | the strip shows fewer than the list's six keys and the hold item at 1280 (the width rule too strict)
# J0.17/foot-keeps-the-hints-that-fit-at-960 | the strip shows more hints at 960 than at 1280, or none
# J0.17/foot-hold-item-is-still-the-last-row-at-960 | the strip ends on its last key at 960, no Hold for all shortcuts item (WHOOSH_MUTANT_R8=no-hold-hint)
# J0.17/foot-shown-hints-are-a-prefix-at-960 | a hint is shown after one that does not fit (the order of the fit broken)
# J0.17/foot-reader-set | the strip keeps the list's keys when the reader takes focus (WHOOSH_MUTANT_R8=stale-set)
# J0.17/foot-composer-set | the strip keeps the list's keys while quick reply is open (WHOOSH_MUTANT_R8=stale-set)
# J0.17/foot-keys-follow-preset | the strip's keys are typed in and stay Superhuman's after a preset switch (WHOOSH_MUTANT_R8=literal-key)
# J0.1/menubar-visible | the menu bar item is never created
# J0.1/menubar-count | the menu bar shows another number than All inboxes
# J0.2/dock-toggles | the Dock switch does nothing
# J0.2/dock-returns | the Dock icon cannot come back
# J0.3/mail-handler-matches-os | the shell reports a handler macOS does not have
# J0.4/bigger | ⌘+ does not grow the text
# J0.4/smaller | ⌘− does not shrink the text
# J0.4/reset | ⌘0 does not return to the locked size
# J0.5/window-titled-with-subject | the message window does not open, or carries another title
# J0.6/drag-reparents-the-label | drag sends no event (WHOOSH_DRAG_NOSEND=1)
# J0.6/drag-delivers-every-event | drag sends no event (WHOOSH_DRAG_NOSEND=1)
# J0.7/status-names-failing-account | the status line shows the sync summary only
# J0.7/status-reason-in-french | the status line draws the core's English reason in a French UI (MacSyncLine.read without Words.text)
# J0.18/shift-r-from-the-list-is-the-no-reply-filter | ⇧R from the plain list pops a reply out (the engine tries popout-reply first and nothing refuses it)
# J0.18/shift-r-in-an-open-thread-pops-out-the-reply | ⇧R in an open thread runs the filter (the list context swallows the key)
# J0.7/status-opens-settings-accounts | the status click does nothing
# J0.7/settings-window-is-up | the status click sets the surface but opens no window
# J0.7/reset-returns-to-main | fixture reset leaves the Settings window and surface in place
# J0.8/shown-list-width-1280 | the list is not at LIST_W_PX before the hide (a stale width from a drag)
# J0.13/shown-list-width-960 | the list is not at LIST_W_COMPACT_PX before the hide
# J0.8/hidden-list-width-1280 | the list keeps LIST_W_PX while the sidebar is hidden (WHOOSH_MUTANT_R8=keep-list-width)
# J0.13/hidden-list-width-960 | the list keeps LIST_W_COMPACT_PX while the sidebar is hidden
# J0.8/hidden-reader-takes-rest-1280 | the reader is capped and leaves a blank band (WHOOSH_MUTANT_R8=reader-cap)
# J0.13/hidden-reader-takes-rest-960 | the reader is capped and leaves a blank band (WHOOSH_MUTANT_R8=reader-cap)
# J0.8/shown-again-restores-list-1280 | showing the sidebar leaves the list wide
# J0.13/shown-again-restores-list-960 | showing the sidebar leaves the list wide
# J0.9/left-focuses-sidebar | focus-left does nothing (WHOOSH_MUTANT_R8=no-focus-keys)
# J0.9/focus-ring-shown | a key-moved focus draws no ring state
# J0.9/right-focuses-list | focus-right is not provided
# J0.9/right-again-focuses-reader | the reader pane never takes the first responder
# J0.9/click-clears-key-ring | a click leaves the keyboard ring on
# J0.10/new-window-opens-a-second-main-window | new-window returns true without opening a window (WHOOSH_MUTANT_R8=no-new-window)
# J0.10/extra-window-closes | the ⌘N copy stays open (WHOOSH_MUTANT_R8=keep-extra)
# J0.10/cmd-equals-grows | font-bigger is not provided
# J0.10/cmd-minus-shrinks | font-smaller is not provided
# J0.10/cmd-zero-resets | font-reset is not provided
# J0.10/ctrl-slash-copies-the-message-link | copy-page-link copies nothing or another id
# J0.10/ctrl-1-filters-to-account-1 | switch-account-1 picks another account
# J0.10/ctrl-2-filters-to-account-2 | switch-account-2 picks another account
# J0.11/toolbar-has-compose-to-more | a toolbar control is missing
# J0.11/tooltips-carry-their-key | a tooltip drops its key
# J0.11/enabled-with-a-selection | the controls stay greyed with a selection
# J0.12/hold-shows-badges | holding ⌘ shows no badge (WHOOSH_MUTANT_R8=no-hints)
# J0.12/row-action-badge-archive-E | the row action carries no key
# J0.12/tab-badge-cmd-1 | the tab row shows no ⌘1
# J0.12/sidebar-badge-sequence | the sidebar rows show no G-then key
# J0.12/release-hides-badges | the badges stay after release
# J0.12/no-badges-while-not-held | the badges are up without a hold
# J0.14/no-two-badges-touch-1280 | the badges sit at the old corner offsets and collide (WHOOSH_MUTANT_R8=no-offset)
# J0.14/tab-badges-clear-of-the-tab-count-1280 | a tab badge covers the tab's count (WHOOSH_MUTANT_R8=no-offset)
# J0.14/toolbar-caps-under-the-icons-1280 | the toolbar caps sit on the icons (WHOOSH_MUTANT_R8=no-offset)
# J0.14/one-cap-per-toolbar-icon-1280 | a toolbar icon shows no cap (WHOOSH_MUTANT_R8=no-hints)
# J0.14/badges-up-1280 | holding shows too few badges (WHOOSH_MUTANT_R8=no-hints)
# J0.15/no-two-badges-touch-960 | the badges sit at the old corner offsets and collide (WHOOSH_MUTANT_R8=no-offset)
# J0.15/tab-badges-clear-of-the-tab-count-960 | a tab badge covers the tab's count (WHOOSH_MUTANT_R8=no-offset)
# J0.15/toolbar-caps-under-the-icons-960 | the toolbar caps sit on the icons (WHOOSH_MUTANT_R8=no-offset)
# J0.15/one-cap-per-toolbar-icon-960 | a toolbar icon shows no cap (WHOOSH_MUTANT_R8=no-hints)
# J0.15/badges-up-960 | holding shows too few badges (WHOOSH_MUTANT_R8=no-hints)
# J0.14/no-badge-covers-a-control-1280 | a badge is centred on the control it labels and hides it (WHOOSH_MUTANT_R8=no-offset)
# J0.15/no-badge-covers-a-control-960 | a badge is centred on the control it labels and hides it (WHOOSH_MUTANT_R8=no-offset)
# J0.14/row-caps-shown-1280 | holding ⌘ shows no row cap (WHOOSH_MUTANT_R8=no-hints)
# J0.14/no-pill-1280 | the row caps sit in one pill of icon and cap pairs (WHOOSH_MUTANT_R8=pill)
# J0.14/caps-float-above-their-own-icons-1280 | the row caps sit beside their icons in the pill (WHOOSH_MUTANT_R8=pill)
# J0.14/caps-clear-of-own-sender-1280 | the pill covers the end of the row's own sender (WHOOSH_MUTANT_R8=pill)
# J0.14/caps-clear-of-other-rows-text-1280 | the caps float a full frame above the icons and land on the row above's subject (WHOOSH_MUTANT_R8=high-caps)
# J0.14/caps-clear-of-own-icon-glyph-1280 | a cap is centred on its icon and covers the glyph (WHOOSH_MUTANT_R8=no-offset)
# J0.15/row-caps-shown-960 | holding ⌘ shows no row cap (WHOOSH_MUTANT_R8=no-hints)
# J0.15/no-pill-960 | the row caps sit in one pill of icon and cap pairs (WHOOSH_MUTANT_R8=pill)
# J0.15/caps-float-above-their-own-icons-960 | the row caps sit beside their icons in the pill (WHOOSH_MUTANT_R8=pill)
# J0.15/caps-clear-of-own-sender-960 | the pill covers the end of the row's own sender (WHOOSH_MUTANT_R8=pill)
# J0.15/caps-clear-of-other-rows-text-960 | the caps float a full frame above the icons and land on the row above's subject (WHOOSH_MUTANT_R8=high-caps)
# J0.15/caps-clear-of-own-icon-glyph-960 | a cap is centred on its icon and covers the glyph (WHOOSH_MUTANT_R8=no-offset)
# J0.16/reply-all-cap-is-enter | the reply-all icon shows no cap in a thread (WHOOSH_MUTANT_R8=no-replyall-cap)
# J0.16/reply-all-disabled-with-no-thread | the reply-all icon is enabled with no thread open
# J0.16/all-nine-toolbar-icons-have-a-cap-in-a-thread | a toolbar icon (reply all) shows no cap in a thread (WHOOSH_MUTANT_R8=no-replyall-cap)
# J0.16/thread-open | Enter opens no thread
# J0.16/reply-all-enabled-in-a-thread | the reply-all icon stays disabled inside a thread
# J0.11/archive-button-archives | the archive control runs no command (WHOOSH_MUTANT_R8=dead-toolbar)



# ---------------------------------------------------------------------------------------------------------------------
# --selftest: J0.20 against a stand-in app, clean (every check green) and once per mutant (the named check red). The stand-in serves what the
# socket serves; a mutant breaks exactly one control. The checks that read macOS itself (J0.1 to J0.19) are not part of it.
class DoorsShellCtl:
    instant = True
    ORDER = ["msg-%02d" % i for i in range(1, 10)]
    CMD = {"toolbar-compose-opens-the-composer": "compose", "toolbar-reply-opens-the-reply-field": "reply", "toolbar-reply-all-opens-the-reply-box-with-everyone": "reply-all",
           "toolbar-forward-opens-the-forward-window": "forward", "toolbar-delete-deletes-the-message": "delete", "toolbar-junk-moves-the-message-to-junk": "junk",
           "toolbar-snooze-opens-the-snooze-picker": "snooze", "toolbar-label-opens-the-label-picker": "label",
           "toolbar-more-unread-flips-the-read-state": "unread", "toolbar-more-star-stars-the-message": "star", "toolbar-more-mute-mutes-the-message": "mute",
           "toolbar-more-move-opens-the-move-picker": "move", "toolbar-more-pin-pins-the-message": "pin"}

    def __init__(self, mutants=()):
        self.m = set(mutants)
        self.reset()

    def reset(self):
        self.order = list(self.ORDER)
        self.unread = {"msg-05", "msg-07", "msg-09"} if "fixture-row-is-unread" not in self.m else {"msg-07", "msg-09"}
        self.starred, self.pinned, self.deleted, self.spam, self.muted = set(), set(), set(), set(), set()
        self.focus, self.overlay, self.surface, self.pending, self.foot, self.thread = 0, None, "mac-main", None, "list", False
        self.collapsed, self.menubar, self.pane = False, True, "list"
        self.qr, self.popout = None, None

    def cur(self):
        return self.order[self.focus] if self.order and self.focus < len(self.order) else None

    def key(self, chord):
        if chord == "Escape":
            self.overlay, self.surface, self.pending, self.foot, self.thread = None, "mac-main", None, "list", False
        elif chord == "Enter" and self.cur():
            self.thread, self.foot = True, "composer"
        return {}

    def toolbar(self, cmd):
        mid = self.cur()
        if mid is None or (cmd == "reply-all" and not self.thread) or any(self.CMD.get(n) == cmd for n in self.m):
            return
        if cmd == "compose":
            self.surface = "mac-composer"
        elif cmd == "reply":
            self.foot = "composer"
        elif cmd == "reply-all":
            self.qr = {"ax": "mac-main.quickreply.header", "open": "1", "to": "karen@x.example, ben@x.example", "cc": ""}
        elif cmd == "forward":
            self.popout = {"ax": "mac-composer.popout", "open": True, "kind": "forward", "subject": "Fwd: Quarterly plan"}
        elif cmd in ("delete", "junk"):
            (self.deleted if cmd == "delete" else self.spam).add(mid)
            self.order.remove(mid)
        elif cmd in ("snooze", "label"):
            self.pending = cmd
        elif cmd == "unread":
            self.unread ^= {mid}
        elif cmd == "star":
            self.starred ^= {mid}
        elif cmd == "mute":
            self.muted.add(mid)
        elif cmd == "move":
            self.overlay = "move"
        elif cmd == "pin":
            self.pinned ^= {mid}

    def click(self, ax):
        if ax.startswith("mac-main.toolbar."):
            self.toolbar(ax.rsplit(".", 1)[1])
        elif ax.startswith("mac-main.list.row."):
            self.focus = int(ax.rsplit(".", 1)[1])
        elif ax == "mac-main.status.link":
            if "status-link-door-opens-settings-accounts" not in self.m:
                self.surface = "mac-settings-accounts"
        elif ax == "mac-main.list.toggle":
            bad = "list-toggle-door-shows-it-again" if self.collapsed else "list-toggle-door-hides-the-list"
            if bad not in self.m:
                self.collapsed = not self.collapsed
        elif ax.startswith("mac-main.pane."):
            p = ax.rsplit(".", 1)[1]
            if "pane-door-%s-gives-it-the-focus" % p not in self.m:
                self.pane = p
        elif ax == "macshell.menubar.toggle":
            bad = "menubar-toggle-door-shows-it-again" if not self.menubar else "menubar-toggle-door-hides-the-item"
            if bad not in self.m:
                self.menubar = not self.menubar
        return {}

    def send(self, verb, *a):
        if verb == "fixture" and a[:1] == ("digest",):
            return {"digest": "stand-in", "parts": {}, "view": "v"}
        if verb == "fixture" and a[:1] == ("reset",):
            self.reset()
        elif verb == "click":
            if a[0].startswith("mac-main.foot.hint.") and "foot-hints-are-display-only-and-say-so" not in self.m:
                raise Unreadable("no door at %s" % a[0])
        return {"ok": True}

    def core(self, path):
        _, mid, field = path.split("/")
        return mid in {"deleted": self.deleted, "spam": self.spam, "muted": self.muted}[field]

    def uistate(self):
        return {"surface": self.surface, "overlay": self.overlay}

    def rows(self, ax):
        if ax == "mac-main.list":
            return [{"ax": "mac-main.list.row.%d" % i, "id": mid, "unread": mid in self.unread, "starred": mid in self.starred, "pinned": mid in self.pinned}
                    for i, mid in enumerate(self.order)]
        if ax == "mac-main.quickreply.header":
            return [self.qr] if self.qr else []
        if ax == "mac-composer.popout":
            return [self.popout] if self.popout else []
        if ax == "mac-main.list.state":
            return [{"pending": self.pending}]
        if ax == "mac-main.foot.hints":
            return [{"surface": self.foot, "command": "archive"}]
        if ax == "mac-main.shell":
            return [{"list_collapsed": self.collapsed, "menubar_visible": self.menubar, "focused_pane": self.pane}]
        return []


def mutant_words(step):
    """check -> the mutation named in this file's own mutants block (one source for the table and the selftest)."""
    out = {}
    for line in open(os.path.abspath(__file__), encoding="utf-8"):
        m = re.match(r"#\s+" + re.escape(step) + r"/([\w-]+)\s*\|\s*(.+)", line)
        if m:
            out[m.group(1)] = m.group(2).strip()
    return out


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
        print("MUTANTS | %s | %s | %s | %s" % (name, "green" if state == "pass" else "RED " + detail, words.get(name, "?"), "red" if ok_red else "STILL " + red[0].upper()))
        bad += [name] if (state != "pass" or not ok_red) else []
    print("%d checks, %d failed, 0 not judged" % (len(clean), len(bad)))
    return 1 if bad else 0


def caps_off_their_icons(acts, frame, glyph_rows):
    """The J0.14/J0.15 float-above rule (lead ruling 23:20): each cap centred on its icon (1 pt), its top above the icon's frame, its bottom
    no lower than the glyph's ink (the icon's top padding may be overlapped). One function so the driven step and the selftest share it."""
    bad = []
    g = {r["ax"]: r["glyph"] for r in glyph_rows}
    for c in acts:
        i = frame(c["ax"])
        ink = g.get(c["ax"], {"y": i["y"]})["y"]
        if abs((c["x"] + c["w"] / 2) - (i["x"] + i["w"] / 2)) > 1 or not (c["y"] < i["y"] and c["y"] + c["h"] <= ink + 0.5):
            bad.append("%s cap=%s icon=%s ink_top=%s" % (c["ax"], [c[k] for k in ("x", "y", "w", "h")], [i[k] for k in ("x", "y", "w", "h")], ink))
    return bad


def selftest_caps():
    """The caps rule on the measured main79 960 frames (one row, five icons): caps over their icons are green; every cap 100 pt left of its icon
    (the main79 shift) is red, and so is a cap that does not rise above the icon's frame."""
    icons = {"mac-main.list.row.msg-02.act." + n: {"x": x, "y": 196, "w": 20, "h": 18} for n, x in (("archive", 471), ("delete", 493), ("junk", 515), ("snooze", 537), ("label", 559))}
    glyphs = [{"ax": a, "glyph": {"x": f["x"] + 5, "y": 199.5, "w": 10, "h": 10}} for a, f in icons.items()]
    def caps(dx, dy):
        return [{"ax": a, "x": f["x"] + 10 - 4.5 + dx, "y": 188.625 + dy, "w": 9, "h": 9} for a, f in icons.items()]
    out = []
    for name, dx, dy, want in (("over-their-icons", 0, 0, 0), ("100-left-of-their-icons", -100, 0, 5), ("not-above-the-frame", 0, 8, 5)):
        got = len(caps_off_their_icons(caps(dx, dy), lambda ax: icons[ax], glyphs))
        out.append(got == want)
        print("MUTANTS | caps %s | %s | %d bad" % (name, "ok" if got == want else "WRONG", got))
    return 0 if all(out) else 1


def selftest():
    """J0.20 clean and once per mutant. The door-table check reads a copy of MacShellFeature.swift with one more door."""
    src0 = shell_sources()
    anchor = '"mac-main.sidebar.toggle":'
    mutated = {"every-door-of-the-shell-is-in-the-control-table": lambda: dict(src0, feature=src0["feature"].replace(anchor, '"mac-main.zzz": { _ in false },\n            ' + anchor, 1))}
    for name, make in mutated.items():
        assert make() != src0, "mutant %s changes nothing: its anchor text is gone from the source" % name
    words = mutant_words("J0.20")

    def run(mutant=None):
        j = Journey(ID, DoorsShellCtl([mutant] if mutant else []))
        doors_shell_step(j, mutated[mutant]() if mutant in mutated else src0)
        return {c.label.split("/", 1)[1]: (c.state, c.detail) for c in j.checks()}
    return report_mutants(run, words) | selftest_sweep() | selftest_titlebar() | selftest_account_digits() | selftest_caps()


# ---------------------------------------------------------------------------------------------------------------------
# par-macshell-sweep (J0.22): every control of the window shell no other case clicks, clicked once with its visible outcome read. What the other steps
# click is listed in docs/lanes/sweep-macshell.md (the inventory). Here: the sidebar's own toggle button and the account switcher at the sidebar's foot
# (the bar, the scrim, the All accounts row, each account row, Add account). Not clicked, no door or row for them (findings, same file): the toolbar's
# More button, the foot strip's ai-mark, the two pane dividers. Not clicked by design: macshell.mail.make-default.
SW_BAR = "mac-main.sidebar.accounts-menu"
SW_OUT = SW_BAR + ".outside"
SW_ALL = "mac-main.accounts.all"
SW_ADD = SW_BAR + ".add"
SW_BTN = "mac-main.sidebar.toggle.button"


def sweep_step(j):
    s = j.step("J0.22", "par-macshell-sweep: the sidebar button and the account switcher, every control clicked once")
    inst = getattr(j.ctl, "instant", False)
    ui = lambda: s.ctl.uistate()
    menu = lambda: s.ctl.rows(SW_BAR)[0]["open"]
    hidden = lambda: s.ctl.rows("mac-main.sidebar.visibility")[0]["hidden"]
    RG.step_reset(s)
    s.key("Escape")
    s.click(SW_ALL)
    s.click("macshell.window.close-others")
    s.click(SW_OUT)                                   # the menu starts pulled down
    s.click(SW_BTN)
    s.expect("sidebar-button-hides-the-sidebar", lambda: lpoll(hidden, True, inst), True)
    s.click(SW_BTN)
    s.expect("sidebar-button-shows-it-again", lambda: lpoll(hidden, False, inst), False)
    s.click(SW_BAR)
    s.expect("account-bar-click-pulls-the-menu-up", lambda: lpoll(menu, True, inst), True)
    s.click(SW_BAR)
    s.expect("account-bar-click-again-pulls-it-down", lambda: lpoll(menu, False, inst), False)
    s.click(SW_BAR + ".up")                           # the rig's opener: the bar's own click was judged above
    rows = lambda: s.ctl.rows(SW_BAR + ".rows")
    s.expect("account-menu-rows-are-all-accounts-every-account-and-add", lambda: (lambda r: (r[0]["ax"], len(r) > 2, r[-1]["ax"]))(rows()), (SW_ALL, True, SW_ADD))
    done = {}

    def one_by_one():
        """Each account row of the menu, clicked once: the filter names that account; then All accounts: no filter."""
        if "run" not in done:
            accts = [r["ax"] for r in rows() if r["ax"].startswith("mac-main.accounts.") and r["ax"] != SW_ALL]
            done["run"] = {"picked": [], "cleared": [], "n": len(accts)}
            for ax in accts:
                s.ctl.click(ax)
                done["run"]["picked"].append((ax, lpoll(lambda: ui()["account_filter"], ax[len("mac-main.accounts."):], inst)))
                s.ctl.click(SW_ALL)
                done["run"]["cleared"].append((ax, lpoll(lambda: ui()["account_filter"], None, inst)))
        return done["run"]
    s.expect("account-row-click-filters-the-list-to-that-account", lambda: [a for a, f in one_by_one()["picked"] if f != a[len("mac-main.accounts."):]] + (["no account row"] * (one_by_one()["n"] == 0)), [])
    s.expect("all-accounts-row-click-clears-the-filter", lambda: [a for a, f in one_by_one()["cleared"] if f is not None], [])
    s.click(SW_OUT)
    s.expect("account-scrim-click-pulls-the-menu-down", lambda: lpoll(menu, False, inst), False)
    s.click(SW_BAR + ".up")
    s.click(SW_ADD)
    s.expect("add-account-row-opens-the-add-account-surface", lambda: lpoll(lambda: (ui()["surface"], menu()), ("mac-add-account", False), inst), ("mac-add-account", False))
    RG.step_reset(s)


# par-macshell-sweep mutants (J0.22): per check, the stand-in with that control dead.
# J0.22/sidebar-button-hides-the-sidebar | the sidebar toggle button runs nothing
# J0.22/sidebar-button-shows-it-again | the second click on the button leaves the sidebar hidden
# J0.22/account-bar-click-pulls-the-menu-up | a click on the account bar does not pull the menu up
# J0.22/account-bar-click-again-pulls-it-down | a second click on the bar leaves the menu up
# J0.22/account-menu-rows-are-all-accounts-every-account-and-add | the menu lacks its Add account row
# J0.22/account-row-click-filters-the-list-to-that-account | a click on an account row filters nothing
# J0.22/all-accounts-row-click-clears-the-filter | the All accounts row leaves the filter on
# J0.22/account-scrim-click-pulls-the-menu-down | a click outside the menu leaves it up
# J0.22/add-account-row-opens-the-add-account-surface | the Add account row opens no surface


class SweepCtl:
    instant = True

    def __init__(self, mutants=()):
        self.m = set(mutants)
        self.reset()

    def reset(self):
        self.hidden, self.open, self.filter, self.surface = False, False, None, "mac-main"
        self.accts = ["acc-1", "acc-2"]

    def key(self, chord):
        return {}

    def click(self, ax):
        dead = lambda n: n in self.m
        if ax == SW_BTN:
            if not dead("sidebar-button-shows-it-again" if self.hidden else "sidebar-button-hides-the-sidebar"):
                self.hidden = not self.hidden
        elif ax == SW_BAR:
            if not dead("account-bar-click-again-pulls-it-down" if self.open else "account-bar-click-pulls-the-menu-up"):
                self.open = not self.open
        elif ax == SW_BAR + ".up":
            self.open = True
        elif ax == SW_OUT:
            if not dead("account-scrim-click-pulls-the-menu-down"):
                self.open = False
        elif ax == SW_ALL:
            if not dead("all-accounts-row-click-clears-the-filter"):
                self.filter = None
        elif ax == SW_ADD:
            if not dead("add-account-row-opens-the-add-account-surface"):
                self.surface, self.open = "mac-add-account", False
        elif ax.startswith("mac-main.accounts."):
            if not dead("account-row-click-filters-the-list-to-that-account"):
                self.filter = ax[len("mac-main.accounts."):]
        return {}

    def send(self, verb, *a):
        if verb == "fixture" and a[:1] == ("digest",):
            return {"digest": "stand-in", "parts": {}, "view": "v"}
        if verb == "fixture" and a[:1] == ("reset",):
            self.reset()
        return {"ok": True}

    def uistate(self):
        return {"surface": self.surface, "account_filter": self.filter}

    def rows(self, ax):
        if ax == SW_BAR:
            return [{"open": self.open}]
        if ax == "mac-main.sidebar.visibility":
            return [{"hidden": self.hidden}]
        if ax == SW_BAR + ".rows":
            r = [{"ax": SW_ALL}] + [{"ax": "mac-main.accounts." + a} for a in self.accts]
            return r + ([] if "account-menu-rows-are-all-accounts-every-account-and-add" in self.m else [{"ax": SW_ADD}])
        return []


def selftest_sweep():
    """J0.22 against the stand-in, clean and once per mutant; the same report as J0.20's."""
    words = mutant_words("J0.22")

    def run(mutant=None):
        RG.LAUNCH.clear()
        j = Journey(ID, SweepCtl([mutant] if mutant else []))
        sweep_step(j)
        return {c.label.split("/", 1)[1]: (c.state, c.detail) for c in j.checks()}
    return report_mutants(run, words)


# ---------------------------------------------------------------------------------------------------------------------
# doors-nav round 2 (J0.21): no control of the main window sits under the title bar. The shared helper is probes-r1's checks/checklib/titlebar.py
# (`winbands` gives the window's REAL band; `mac-main.controls` lists the controls; `measure` gives each frame). The band is whatever the window says:
# no number is typed here. The toolbar and the sidebar toggle sit ON the bar's line by design (frame-view subviews above it) and are not listed.
FULL_SCREEN_MAX_TOP = 60.0   # pt: the lowest first-row control (the list's command field, 48 pt) plus its own margin
TB_SIZES = (("1280", 1280, 820), ("960", 960, 632))


def main_band(s):
    """-> (offenders, unjudged) of the main window alone (the helper judges every full-size window the app has open)."""
    ctl = s.ctl
    wins = [w for w in ctl.send("winbands").get("bands", []) if TB.surface_of(w) == "mac-main"]
    if not wins:
        raise Unreadable("winbands lists no main window")

    def controls_of(surface):
        try:
            return ctl.rows(surface + ".controls")
        except Unreadable:
            return None

    def measure(ax):
        try:
            return ctl.measure(ax).get("frame")
        except Unreadable:
            return None
    return TB.offenders(wins, controls_of, measure)


def under_band(s):
    bad, unj = main_band(s)
    return ([(a, p) for _, a, p in bad], [w for w, _ in unj])


def full_screen_band(s):
    """The band in full screen is 0. A hidden rig refuses full screen (a Space of its own): that is not judged by design, named once."""
    bands = [w for w in s.ctl.send("winbands").get("bands", []) if TB.surface_of(w) == "mac-main"]
    if not bands or bands[0].get("band", 1) != 0:
        name = "J0.21/main-controls-clear-the-band-in-full-screen"
        if name not in BY_DESIGN:
            BY_DESIGN.append(name)
        raise Unreadable("the main window is not in full screen (a hidden rig refuses it)")
    # band 0: nothing may be pushed down for a bar that is gone. The lowest first row (the list's command field) sits 48 pt down by design.
    low = [a for a in [r["ax"] for r in s.ctl.rows("mac-main.controls")] if (s.ctl.measure(a).get("frame") or {"y": 0})["y"] > FULL_SCREEN_MAX_TOP]
    return under_band(s) + (low,)


def main_title_shown(s):
    """Whether the main window draws its title text (`winbands` title_shown, WindowControl.bands). A build that serves no such field is not judged."""
    bands = [w for w in s.ctl.send("winbands").get("bands", []) if TB.surface_of(w) == "mac-main"]
    if not bands or "title_shown" not in bands[0]:
        raise Unreadable("winbands serves no title_shown for the main window (an older build)")
    return bands[0]["title_shown"]


def titlebar_step(j):
    s = j.step("J0.21", "No control of the main window is under the title bar: default and compact size, sidebar hidden, full screen")
    # the title text (owner's picture, 16 Nov, the installed main195: the word "Whoosh" drawn over the sidebar button): hidden at launch, and hidden again
    # after something turns it on (`window title show`: the rig's way to do what the system did on the owner's copy)
    s.expect("main-title-text-is-hidden", lambda: main_title_shown(s), False)
    s.raw("window", "title", "show")
    s.expect("main-title-text-is-hidden-again-after-it-was-turned-on", lambda: settle(lambda: main_title_shown(s), False), False)
    for tag, w, h in TB_SIZES:
        s.raw("resize", str(w), str(h))
        s.expect("main-controls-clear-the-band-at-" + tag, lambda: settle(lambda: under_band(s), ([], [])), ([], []))
    s.raw("resize", "1280", "820")
    s.click("mac-main.sidebar.toggle")
    s.expect("main-controls-clear-the-band-with-the-sidebar-hidden", lambda: settle(lambda: under_band(s), ([], [])), ([], []))
    s.click("mac-main.sidebar.toggle")
    s.raw("window", "fullscreen", "on")
    s.expect("main-controls-clear-the-band-in-full-screen", lambda: full_screen_band(s), ([], [], []))
    s.raw("window", "fullscreen", "off")
    s.raw("resize", "1280", "820")


# J0.21 mutants: per check, the mutation of the app that must turn it red (each puts the controls back where a typed top offset leaves them).
# J0.21/main-title-text-is-hidden | the main window never hides its title text: the word "Whoosh" is drawn in the title band from the launch
# J0.21/main-title-text-is-hidden-again-after-it-was-turned-on | the main window hides its title text once, at launch, and never again (no watch): turned on later, it stays on the sidebar button
# J0.21/main-controls-clear-the-band-at-1280 | the panes take no extra top room at 1280 x 820: the first control starts 12 pt from the window top
# J0.21/main-controls-clear-the-band-at-960 | the panes take no extra top room at 960 x 632
# J0.21/main-controls-clear-the-band-with-the-sidebar-hidden | the list takes no extra top room while the sidebar is hidden
# J0.21/main-controls-clear-the-band-in-full-screen | the panes keep the windowed top room in full screen, where the band is 0 (a control is pushed down 52 pt for a bar that is gone: the check reads a first control lower than 60 pt)


class TitleBandCtl:
    """The stand-in for J0.21: a main window whose panes begin at the window top (the worse case) and place their first control `FIRST` below the pane
    top plus the room the app adds. The fixed app adds max(0, band - FIRST); a mutant adds nothing in the state its check names. In full screen the band is 0."""
    instant = True
    BAND = 52.0
    FIRST = {"mac-main.sidebar.header.mailboxes": 16, "mac-main.sidebar.mailboxes.pick": 16, "mac-main.list.command": 48, "mac-main.splits.0": 12}

    def __init__(self, mutants=()):
        self.m = set(mutants)
        self.size, self.hidden, self.full = "1280", False, False
        self.title = "main-title-text-is-hidden" in self.m          # the title text drawn: never on the fixed app

    def band(self):
        return 0.0 if self.full else self.BAND

    def room(self, first):
        name = {"1280": "main-controls-clear-the-band-at-1280", "960": "main-controls-clear-the-band-at-960"}[self.size]
        if self.hidden:
            name = "main-controls-clear-the-band-with-the-sidebar-hidden"
        if self.full:
            name = "main-controls-clear-the-band-in-full-screen"
        if name in self.m:
            return 0.0 if not self.full else 52.0   # the full-screen mutant leaves the windowed room in place
        return max(0.0, self.band() - first)

    def send(self, verb, *a):
        if verb == "resize":
            self.size = "960" if a[0] == "960" else "1280"
        elif verb == "window" and a[:2] == ("fullscreen", "on"):
            self.full = True
        elif verb == "window" and a[:2] == ("fullscreen", "off"):
            self.full = False
        elif verb == "window" and a[:2] == ("title", "show"):
            self.title = "main-title-text-is-hidden-again-after-it-was-turned-on" in self.m or "main-title-text-is-hidden" in self.m   # the fixed app hides it again at once
        elif verb == "winbands":
            return {"ok": True, "bands": [{"id": "mac-main-AppWindow-1", "title": "Whoosh", "band": self.band(), "fullsize": True, "lights": [{"x": 12, "y": 18, "w": 14, "h": 16}],
                                           "title_shown": self.title}]}
        return {"ok": True}

    def click(self, ax):
        if ax == "mac-main.sidebar.toggle":
            self.hidden = not self.hidden

    def rows(self, ax):
        if ax == "mac-main.controls":
            return [{"ax": k} for k in self.FIRST if not (self.hidden and "sidebar" in k)]
        return []

    def measure(self, ax):
        first = self.FIRST[ax]
        return {"ok": True, "frame": {"x": 20, "y": first + self.room(first), "w": 100, "h": 24}}

    def uistate(self):
        return {"surface": "mac-main"}


def selftest_titlebar():
    """J0.21 against the stand-in, clean and once per mutant."""
    words = mutant_words("J0.21")

    def run(mutant=None):
        j = Journey(ID, TitleBandCtl([mutant] if mutant else []))
        titlebar_step(j)
        return {c.label.split("/", 1)[1]: (c.state, c.detail) for c in j.checks()}
    return report_mutants(run, words)



class AccountDigitsCtl:
    """Stand-in for J0.10's account digits: Ctrl-N sets the filter to account N. `refuse` makes the account list read fail (the main76 crash);
    `wrong` makes the key leave the filter unchanged."""
    instant = True

    def __init__(self, refuse=False, wrong=False):
        self.refuse, self.wrong, self.filter = refuse, wrong, None

    def key(self, chord):
        if not self.wrong:
            self.filter = {"⌃1": "acc-1", "⌃2": "acc-2"}[chord]
        return {}

    def uistate(self):
        return {"account_filter": self.filter}

    def rows(self, ax):
        if self.refuse:
            raise Unreadable("stand-in: the account list read was refused")
        return [{"id": "acc-1"}, {"id": "acc-2"}]


def selftest_account_digits():
    """A refused account-list read is NOT JUDGED for both cases (never a crash); a filter that does not follow the key is red; the normal run is green."""
    out = []
    for name, ctl, want in (("normal", AccountDigitsCtl(), "pass"), ("refused-read", AccountDigitsCtl(refuse=True), "unjudged"), ("filter-ignores-the-key", AccountDigitsCtl(wrong=True), "fail")):
        j = Journey(ID, ctl)
        try:
            account_digits(j.step("J0.10", "digits"))
            got = sorted({c.state for c in j.checks()})
        except Exception as e:
            got = ["CRASH %s: %s" % (type(e).__name__, e)]
        ok = got == [want] and len(j.checks()) == 2
        out.append(ok)
        print("MUTANTS | account-digits %s | %s | %s" % (name, "ok" if ok else "WRONG", ", ".join(got)))
    return 0 if all(out) else 1


def main():
    if "--selftest" in sys.argv:
        return selftest()
    path = os.path.abspath(__file__)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    print_cases(r)
    for line in r["fail_list"]:
        sys.stderr.write("FAIL %s\n" % line)
    for line in r["no_mutant"] + r["stray_mutant"]:
        sys.stderr.write("MUTANT ROW %s\n" % line)
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    for name in BY_DESIGN:
        sys.stderr.write("NOT JUDGED BY DESIGN %s: the Dock tile is refused on a hidden rig\n" % name)
    ok = r["failed"] == 0 and r["unjudged"] == len(BY_DESIGN) and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"]
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
