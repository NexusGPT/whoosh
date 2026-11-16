"""J1: Triage the day's mail across 30+ accounts with keys (docs/JOURNEYS.md J1.1-J1.12), one pass per theme.

Written from the step table, docs/design/fixtures/mac-main-sample.json and docs/CONSTANTS.md, never from a
built screen. The live wire contract is docs/CONTROL-SOCKET.md (see its "Deltas from the runner's assumption").
Run once per theme: WHOOSH_THEME=light|dark python3 journeys/run.py J1  (one app launch per theme: the fixture
core cannot be reset, so the two passes never share state).

Fixture trace (identity, not position; every row is named by its fixture id):
  msg-01 archived (J1.2) . msg-02 snoozed tomorrow morning (J1.3) . msg-04 pinned and labelled Suppliers, opened,
  answered (J1.4-J1.7) . msg-05 archived by mistake then undone then moved to acc-01 (J1.8-J1.9) . msg-06 archived
  with the mouse (hint) . msg-07 archived with E (third key use) . msg-08 archived with the mouse (no hint).
  acc-18 (msg-03) is the failing account: the fixture core refuses archive/snooze/label/move/send on it, so the
  journey never acts on msg-03 until the fixture event `fixture reauth acc-18` (J1.12) brings it back.
"""
import json
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__))))
from lib.ctl import Unreadable                                                          # noqa: E402
from lib.fixture import PRESET_ANCHORS, PRESET_KEY_ROWS, TABS_NOW, TAB_IDS_NOW, FX, ROWS, ACCOUNT, PRIMARY_UNREAD, ROW_COUNT, served_unread, const             # noqa: E402


# ---- the words the app shows, read from its catalogue (never typed in English here) -------------------------------------------------------------
# A hidden test copy speaks the language of the Mac (French on this one), so a sentence a check expects is the catalogue's text for the language
# the app speaks now (Sources/Resources/Localizable.xcstrings, the same file Sources/Localization/Words.swift reads; a key the catalogue lacks
# shows as written, as it does in the app). The language is read the way Words.swift picks it: the stored choice `whoosh.language`, else the
# language list of the rig identity, else the Mac's own list.
CATALOGUE_PATH = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "Sources", "Resources", "Localizable.xcstrings")
CATALOGUE = json.load(open(CATALOGUE_PATH, encoding="utf-8"))["strings"]
CODES = ["en", "fr", "es", "pt", "de", "it", "nl", "pl", "sv", "da", "nb", "fi", "ja", "zh-Hans"]      # Words.languages
RIG_DOMAIN = "io.nexus.whoosh.rig"
MISSING_KEYS = set()


def _defaults(*args):
    try:
        out = subprocess.run(["defaults"] + list(args), capture_output=True, text=True, timeout=10)
        return out.stdout if out.returncode == 0 else ""
    except (OSError, subprocess.SubprocessError):
        return ""


def mac_language():
    """(code, where it was read): what Words.current gives a test copy that was never told another language."""
    pick = _defaults("read", RIG_DOMAIN, "whoosh.language").strip()
    if pick in CODES:
        return pick, "whoosh.language of the rig identity"
    for args, where in ((("read", RIG_DOMAIN, "AppleLanguages"), "AppleLanguages of the rig identity"), (("read", "-g", "AppleLanguages"), "AppleLanguages of the Mac")):
        for line in _defaults(*args).splitlines():
            pref = line.strip().strip(",").strip('"').lower()
            code = next((c for c in sorted(CODES, key=len, reverse=True) if pref == c.lower() or pref.startswith(c.lower() + "-")), None)
            if code:
                return code, where
    return "en", "no language list read: English"


LANG = {"code": None, "where": ""}


def app_language():
    if LANG["code"] is None:
        LANG["code"], LANG["where"] = mac_language()
    return LANG["code"]


def learn_language(s, shown, key):
    """Settle the language from a text the APP shows, not from the Mac's preferences: the codes whose catalogue text for `key` is inside `shown`.
    The guess from the preferences stays when it is one of them. Nothing is read in a dry run, and a text no language matches keeps the guess."""
    if s.j.ctl is None or not isinstance(shown, str):
        return
    loc = CATALOGUE.get(key, {}).get("localizations", {})
    hits = [c for c in CODES if (loc.get(c, {}).get("stringUnit", {}).get("value") or (key if c == "en" else "")) and
            (loc.get(c, {}).get("stringUnit", {}).get("value") or key) in shown]
    guess = app_language()
    if hits:
        LANG["code"], LANG["where"] = (guess if guess in hits else hits[0]), "read from the app's own text %r" % shown[:70]
    s.j.notes.append("the app's language: %s (%s); the Mac's preferences said %s%s" % (LANG["code"], LANG["where"], guess, "" if hits else ", and no language matched the app's text"))


def words(key, *args, strict=True):
    """The app's text for a catalogue key in the language it speaks now; `%@` and `%lld` take the arguments in order."""
    entry = CATALOGUE.get(key)
    if entry is None and strict:
        MISSING_KEYS.add(key)
    loc = (entry or {}).get("localizations", {})
    value = loc.get(app_language(), {}).get("stringUnit", {}).get("value") or loc.get("en", {}).get("stringUnit", {}).get("value") or key
    for a in args:
        value = value.replace("%@", str(a), 1) if "%@" in value else value.replace("%lld", str(a), 1)
    return value

THEME = os.environ.get("WHOOSH_THEME", "light")
ID = "J1"
TITLE = "Triage the day's mail across 30+ accounts with keys (%s theme)" % THEME
TARGET = "LOCAL"
VISIBLE_OUTCOME = ("Primary holds no unhandled mail; each message shows in its new place "
                   "(archived, snoozed, labelled, pinned or answered), all done with keys")

LIST = "mac-main.list"
M = {int(m["id"].split("-")[1]): m for m in FX["messages"]}          # M[4] is msg-04
SNOOZED0 = next(x["count"] for x in FX["mailboxes"] if x["name"] == "Snoozed")
FAIL_ACC = FX["states"]["sync"]["failing_account"]
TARGET_ACC = "acc-01"
FAILING_REASON = "needs you to sign in again"             # the catalogue key of the reason the sync line gives (FixtureCore.swift:2024)
assert FAILING_REASON in FX["states"]["sync"]["failing_line"]
assert M[3]["account"] == FAIL_ACC and M[4]["account"] != FAIL_ACC


def rd(fn, default=None):
    """A precondition read for navigation; an unreadable probe gives the default, never a verdict."""
    try:
        return fn()
    except (Unreadable, KeyError, IndexError, TypeError):
        return default


def served(s, *args):
    """True when the app answers ok to a verb; an ok:false is a False, so the check after it turns red."""
    ctl = s.ctl                       # a dry run raises Unreadable here: the check is not judged, never failed
    try:
        ctl.send(*args)
        return True
    except Unreadable:
        return False


def ids(s):
    return [r["id"] for r in s.ctl.rows(LIST)]


def idx(s, mid):
    """Position of a row by id, or None when it is not listed (a miss is a failed check, never a crash)."""
    order = ids(s)
    return order.index(mid) if mid in order else None


def under_load(value, budget, strict=True):
    """The shared load rule (journeys/lib/loadgate.py, lead 2026-11-13): at or below PERF_LOAD_MAX the timing is judged as always; above
    it a timing inside its budget still passes (the load is printed) and one over its budget is NOT JUDGED with its number."""
    from lib import loadgate
    return loadgate.under_load(value, budget, strict, Unreadable)


def over_budget_with(s, phases_ax, value, limit):
    """`value` when it is within `limit`; else an IndexError that carries the phases the app serves under `phases_ax` (docs/lanes/perf-archive.md (c) and (d):
    `measure mac-main.interaction.phases`, `mac-main.launch.phases`), so the failure line says which phase holds the time. The budget is not touched."""
    if value <= limit:
        return value
    try:
        got = s.ctl.measure(phases_ax)
        shown = {k: v for k, v in (got.get("phases") or {}).items() if v} if isinstance(got, dict) and isinstance(got.get("phases"), dict) else got
    except Exception as e:           # a build that serves no phases says so
        shown = "not served (%s: %s)" % (type(e).__name__, e)
    raise IndexError("got %r, expected <= %r | %s %s" % (value, limit, phases_ax, shown))


def label_path(name, tree=None, up=()):
    """The label's path in the design fixture's tree ("Work > Suppliers"), the way the sidebar names it."""
    for lab in (FX["labels"] if tree is None else tree):
        here = up + (lab["name"],)
        if lab["name"] == name:
            return " > ".join(here)
        found = label_path(name, lab.get("children") or [], here)
        if found:
            return found
    return None


def sidebar_label(s, name):
    """(path, own count) of the sidebar label row named `name`; no row is (None, None), a plain value the check compares."""
    row = next((r for r in s.ctl.rows("mac-main.sidebar.labels") if r.get("name") == name), None)
    return (row.get("path"), row.get("own")) if row else (None, None)


def goto(s, mid):
    """Move focus to a row by J/K, as a person would; no-op in a dry run."""
    if s.j.ctl is None:
        return
    for _ in range(30):
        st = rd(lambda: s.ctl.uistate())
        order = rd(lambda: ids(s))
        if not st or not order or mid not in order or st.get("focus_message") == mid:
            return
        s.key("J" if order.index(mid) > (st.get("focus_index") or 0) else "K")


def holder(s):
    """Where the next key goes (`uistate.first_responder`: list, reader, quick-reply, ...); None when it cannot be read (a dry run)."""
    if s.j.ctl is None:
        return None
    return rd(lambda: s.ctl.uistate().get("first_responder"))


def where(s):
    """(surface, first_responder) read now: the two things a key changes."""
    return rd(lambda: (lambda u: (u.get("surface"), u.get("first_responder")))(s.ctl.uistate()))


def settle(s, changed_from, secs=3.0):
    """Read the state until it differs from `changed_from` (a read, never a blind sleep); gives up after `secs` and returns what it last read."""
    end = time.time() + secs
    now = where(s)
    while now == changed_from and time.time() < end:
        time.sleep(0.1)
        now = where(s)
    return now


def holder_after(s, want, secs=3.0):
    """`uistate.first_responder`, read again until it is `want` or `secs` have passed; the last read is returned (a read, never a blind sleep)."""
    end = time.time() + secs
    now = s.ctl.uistate().get("first_responder")
    while now != want and time.time() < end and not getattr(s.j.ctl, "instant", False):
        time.sleep(0.1)
        now = s.ctl.uistate().get("first_responder")
    return now


def to_reader(s):
    """REPLY -> READER. A Return in the list opens the mail AND focuses the reply field (REPLY); Escape in REPLY goes to READER. A step that
    starts from READER (R, the reply key) first puts the keys back there; from READER already it presses nothing."""
    if s.j.ctl is None or holder(s) != "quick-reply":
        return
    before = where(s)
    s.key("Escape")
    settle(s, before)


def to_list(s):
    """READER or REPLY -> LIST: Escape in REPLY goes to READER, Escape in READER goes to LIST. Presses Escape up to three times, each one only
    after reading that the surface is not the list yet, and waits for each to change the state."""
    if s.j.ctl is None:
        return
    for _ in range(3):
        before = where(s)
        if not before or before[0] == "mac-main":
            return
        s.key("Escape")
        settle(s, before)


def hint_text(s):
    """The text of the one `mac-trainer.hint` row, polled up to 3 s (the hint is drawn after the click returns). No row is a plain value the
    check compares, never an IndexError (the trainer lane's patch for this file, docs/lanes/trainer.md round 3)."""
    import time
    end = time.time() + 3.0
    rows = s.ctl.rows("mac-trainer.hint")
    while not rows and time.time() < end:
        time.sleep(0.1)
        rows = s.ctl.rows("mac-trainer.hint")
    return rows[0].get("text", "<hint row without text>") if rows else "<no mac-trainer.hint row served>"


def archive_with_mouse(s, mid):
    order = rd(lambda: ids(s), [])
    if mid in order:
        s.click("%s.row.%d.archive" % (LIST, order.index(mid)))


def drive(j):
    # ------------------------------------------------------------------ J1.1 launch
    s = j.step("J1.1", "Unified Inbox paints inside the launch budget")
    s.arrive(("click", "dock.whoosh"), "mac-main")
    learn_language(s, rd(lambda: s.ctl.rows("mac-main.sync.line")[0]["text"]), FAILING_REASON)      # every sentence below is the catalogue's text in THIS language
    failing_line = FX["states"]["sync"]["failing_line"].replace(FAILING_REASON, words(FAILING_REASON))
    s.expect("theme-set", lambda: served(s, "fixture", "theme", THEME), True)
    s.expect("theme-applied", lambda: s.ctl.uistate()["theme"], THEME)
    s.expect("row-count", lambda: len(s.ctl.rows(LIST)), ROW_COUNT)
    s.expect("row-ids-in-fixture-order", lambda: ids(s), [m["id"] for m in ROWS])
    s.expect("pinned-rows-on-top", lambda: [r["pinned"] for r in s.ctl.rows(LIST)][:2], [True, True])
    s.expect("primary-unread", lambda: s.ctl.uistate()["counts"]["primary"], served_unread("primary", PRIMARY_UNREAD))   # the file's 23 plus the flow's rejected row
    s.expect("primary-waiting", lambda: s.ctl.uistate()["waiting"]["primary"], FX["tabs"][0]["waiting"])
    s.expect("first-row-focus", lambda: s.ctl.uistate()["focus_message"], FX["focused_message"])
    s.expect("account-colour-per-row", lambda: [r["account"] for r in s.ctl.rows(LIST)], [m["account"] for m in ROWS])
    # the fixture's line is the account name and the reason in English; the reason is a catalogue key, shown in the language the app speaks
    s.expect("failing-account-line", lambda: s.ctl.rows("mac-main.sync.line")[0]["text"], failing_line)
    # launch-r3: J1 runs on one launch of a new rig copy, its first (the OS scans the new file first): the cold budget, as perf/launch.py's
    # fresh-copy line; the owner's daily launch (the second of one copy) is perf/launch.py's LAUNCH_SAME_COPY_MS line
    s.expect_le("launch-budget-ms", lambda: over_budget_with(s, "mac-main.launch.phases", under_load(s.ctl.measure("mac-main.launch")["ms"], const("LAUNCH_COLD_MS"), strict=False), const("LAUNCH_COLD_MS")), const("LAUNCH_COLD_MS"))
    s.shot("J1.1-%s" % THEME)

    # ------------------------------------------------------------------ J1.2 archive
    s = j.step("J1.2", "Archive with E")
    s.arrive(("key", "J"), "mac-main")
    s.key("K")
    s.key("E")
    s.expect("row-leaves", lambda: s.ctl.rows(LIST)[0]["id"], "msg-02")
    s.expect("row-gone-from-list", lambda: "msg-01" in ids(s), False)
    s.expect("list-probe-control", lambda: len(ids(s)), ROW_COUNT - 1)
    s.expect("next-row-focus", lambda: s.ctl.uistate()["focus_message"], "msg-02")
    s.expect("count-drops", lambda: s.ctl.uistate()["counts"]["primary"], served_unread("primary", PRIMARY_UNREAD) - 1)
    s.expect("archived-in-core", lambda: s.ctl.core("message/msg-01/archived"), True)
    s.expect("undo-entry-written", lambda: s.ctl.core("undo_entry/count"), 1)

    # ------------------------------------------------------------------ J1.3 snooze
    s = j.step("J1.3", "Snooze with H, tomorrow morning")
    s.arrive(("key", "H"), "mac-main")
    s.expect("picker-lists-times-in-words", lambda: [r["title"].lower() for r in s.ctl.rows("mac-main.snooze.options")][:3],
             ["later today", "tomorrow morning", "next week"])
    s.type("tomorrow morning")      # the picker finishes on typed words plus Enter (inboxlist lane); its option rows carry no click door
    s.key("Enter")
    s.expect("row-leaves", lambda: "msg-02" in ids(s), False)
    s.expect("picker-closed", lambda: s.ctl.uistate()["overlay"], None)
    s.expect("snoozed-count-up", lambda: s.ctl.uistate()["counts"]["snoozed"], SNOOZED0 + 1)
    s.expect("snoozed-in-core", lambda: s.ctl.core("message/msg-02/snoozed"), True)
    s.expect("snooze-time-0800", lambda: s.ctl.core("snooze/msg-02/return_local_time"), "08:00")
    s.expect("undo-entry-written", lambda: s.ctl.core("undo_entry/count"), 2)

    # ------------------------------------------------------------------ J1.4 pin
    s = j.step("J1.4", "Pin with P")
    goto(s, "msg-04")
    s.arrive(("key", "⇧P"), "mac-main")   # pin is ⇧P in the Superhuman preset; plain P is previous-message
    s.expect("row-on-top", lambda: s.ctl.rows(LIST)[0]["id"], "msg-04")
    s.expect("pin-mark", lambda: s.ctl.rows(LIST)[0]["pinned"], True)
    s.expect("pinned-in-core", lambda: s.ctl.core("pin/msg-04"), True)
    s.expect("pinned-row-keeps-focus", lambda: s.ctl.uistate()["focus_message"], "msg-04")
    s.click("mac-main.accounts.%s" % M[3]["account"])      # another account filter than msg-04's own
    s.expect("on-every-account-filter", lambda: s.ctl.rows(LIST)[0]["id"], "msg-04")
    s.click("mac-main.accounts.all")
    s.expect("filter-reset", lambda: s.ctl.uistate()["account_filter"], None)
    s.expect("undo-entry-written", lambda: s.ctl.core("undo_entry/count"), 3)

    # ------------------------------------------------------------------ J1.5 label
    s = j.step("J1.5", "Label with L, Suppliers")
    goto(s, "msg-04")
    own0 = rd(lambda: sidebar_label(s, "Suppliers")[1], 0) or 0   # the label's own count before this step (0 on a fresh core)
    s.arrive(("key", "L"), "mac-main")
    s.expect("picker-lists-nested-labels", lambda: "Suppliers" in [r["name"] for r in s.ctl.rows("mac-main.label.picker")], True)
    s.type("Suppliers")
    s.key("Enter")
    s.expect("chip-on-row", lambda: s.ctl.rows(LIST)[0]["chips"], ["Suppliers"])
    s.expect("assignment-in-core", lambda: s.ctl.core("label_assignment/msg-04"), ["Suppliers"])
    # "the message shows under that label in the sidebar": the sidebar's label rows (mac-main.sidebar.labels, par-sidebar) carry the label's
    # path in the tree and its own count; the one message labelled here raises that count by exactly one. A click on the row cannot read
    # the label's list: its parent Work is folded, and a click on a row that is not visible does nothing (main53, probe of 2026-11-10).
    s.expect("sidebar-lists-label", lambda: (lambda p_n: (p_n[0], (p_n[1] or 0) - own0))(sidebar_label(s, "Suppliers")),
             (label_path("Suppliers"), 1))
    s.expect("other-row-unlabelled", lambda: s.ctl.core("label_assignment/msg-05"), [])

    # ------------------------------------------------------------------ J1.6 thread
    s = j.step("J1.6", "Enter opens the thread")
    s.arrive(("key", "Enter"), "mac-reader")
    s.expect("subject", lambda: s.ctl.rows("mac-main.reader.subject")[0]["subject"], M[4]["subject"])
    s.expect("tracker-count", lambda: s.ctl.rows("mac-main.reader.trackers")[0]["count"], M[4]["trackers_blocked"])
    s.expect("marked-read", lambda: s.ctl.core("message/msg-04/read"), True)
    # The budget is the commit of the Enter that opened the thread (docs/CONSTANTS.md INTERACTION_COMMIT_MS: `mac-main.interaction.commit`
    # is under it); there is no `mac-main.reader` measure. Judged only on a quiet machine, like J1.1.
    s.expect_le("one-interaction-budget-ms", lambda: over_budget_with(s, "mac-main.interaction.phases", under_load(float(s.ctl.measure("mac-main.interaction.commit")["ms"]), const("INTERACTION_COMMIT_MS"), strict=False), const("INTERACTION_COMMIT_MS")), const("INTERACTION_COMMIT_MS"))

    # ------------------------------------------------------------------ J1.7 quick reply
    s = j.step("J1.7", "Reply with R, two lines, send with Cmd-Enter")
    to_reader(s)                     # J1.6's Return focused the reply field (REPLY): Escape puts the keys on the reader, so R is the reply key and is not typed
    s.arrive(("key", "R"), "mac-reader")
    # REPLY: the typed lines below go to the field. R opens the field and the focus follows on the next turn, so the holder is read until it is the field
    # (up to 3 s; main81 read `reader` right after the surface changed, and the lines typed next still reached the field).
    s.expect("reply-field-holds-the-keys", lambda: holder_after(s, "quick-reply"), "quick-reply")
    s.expect("sent-from-receiving-address", lambda: s.ctl.rows("mac-main.quickreply.field")[0]["from"], ACCOUNT[M[4]["account"]]["address"])
    s.type("Thanks.\nNoted for section 4.2.")
    s.key("⌘Enter")
    s.expect("undo-send-bar", lambda: (s.ctl.rows("mac-main.undo.bar")[0]["message"], bool(s.ctl.rows("mac-main.undo.bar")[0].get("undo"))), (words("Sending"), True))   # the bar says Sending (in the app's language) and carries its Undo door
    s.expect("undo-send-counts-from-default", lambda: s.ctl.rows("mac-main.undo.bar")[0]["seconds"], const("UNDO_SEND_DEFAULT_S"))
    s.expect("outbox-entry", lambda: s.ctl.core("outbox_entry/count"), 1)
    s.expect("not-sent-inside-window", lambda: s.ctl.core("outbox_entry/0/state"), "undoable")
    s.raw("clock", "advance", const("UNDO_SEND_DEFAULT_S") + 1)
    s.expect("sent-after-window", lambda: s.ctl.core("outbox_entry/0/state"), "sent")
    # The reader thread lists the earlier messages and then the OPEN message (ReaderThread.entries: "the latest is open"), and the open one is
    # the row that was opened (msg-04, the host's message), so the reply is never the LAST entry: it is one of the entries (main48 read
    # [-1] and got the host). The reply shows in the thread when an entry is sent by the owner; the control is the original, still listed.
    s.expect("thread-lists-the-original", lambda: [r["sender"] for r in s.ctl.rows("mac-main.reader.thread")].count(M[4]["sender"]), 1)
    # The sent reply is the thread entry FROM the receiving address that carries the typed text (main53: sender and from are the address,
    # the text is the two lines then the signature). Exactly one such entry: missing or listed twice is red.
    s.expect("reply-in-thread", lambda: sum(1 for r in s.ctl.rows("mac-main.reader.thread")
                                            if r.get("from") == ACCOUNT[M[4]["account"]]["address"]
                                            and (r.get("text") or "").startswith("Thanks.\nNoted for section 4.2.")), 1)

    # ------------------------------------------------------------------ J1.8 undo an archive by mistake
    s = j.step("J1.8", "Esc back, archive by mistake, Z undoes")
    to_list(s)                       # REPLY -> READER -> LIST, one Escape per read of the state (not a fixed two: after a send the field may not hold the keys)
    s.expect("arrives-by-control", lambda: s.ctl.uistate().get("surface"), "mac-main")
    goto(s, "msg-05")
    n0 = rd(lambda: len(ids(s)), -1)
    i0 = rd(lambda: idx(s, "msg-05"), -1)
    s.key("E")
    s.expect("mistake-archived", lambda: ("msg-05" in ids(s), len(ids(s)) - n0), (False, -1))
    s.key("Z")
    s.expect("row-returns", lambda: len(ids(s)) - n0, 0)
    s.expect("row-returns-to-its-place", lambda: idx(s, "msg-05"), i0)
    s.expect("undo-entry-consumed", lambda: s.ctl.core("undo_entry/consumed_last"), True)
    s.expect("archived-flag-cleared", lambda: s.ctl.core("message/msg-05/archived"), False)

    # ------------------------------------------------------------------ J1.9 palette: move to another account
    s = j.step("J1.9", "Palette: Move to another account")
    goto(s, "msg-05")
    s.arrive(("key", "⌘P"), "mac-palette")      # ⌘K is the search everywhere (owner 2026-11-08); the palette key is ⌘P
    s.type(words("Move to another account"))     # the palette ranks the title it shows, in the app's language
    s.expect("palette-result", lambda: s.ctl.rows("mac-palette.results")[0]["title"], words("Move to another account"))
    s.expect("key-hint-column-exists", lambda: "key" in s.ctl.rows("mac-palette.results")[0], True)
    s.key("Enter")
    s.type(ACCOUNT[TARGET_ACC]["name"])
    s.key("Enter")
    s.expect("palette-closes-to-inbox", lambda: s.ctl.uistate()["surface"], "mac-main")
    s.expect("moved-in-core", lambda: s.ctl.core("message/msg-05/account"), TARGET_ACC)
    s.expect("row-shows-under-target-account", lambda: ((s.click("mac-main.accounts.%s" % TARGET_ACC) or True) and "msg-05" in ids(s)), True)
    s.click("mac-main.accounts.all")
    s.expect("undo-entry-written", lambda: s.ctl.core("undo_entry/count"), 7)

    # ------------------------------------------------------------------ J1.10 trainer hint
    s = j.step("J1.10", "Trainer hint after a mouse archive, gone after three key uses")
    # The hints switch is a preference of the rig identity and survives between runs (main48: whoosh.trainer.hints = 0, no hint served):
    # the journey turns it on and proves it before it relies on a hint (same as checks/trainer-check.py).
    s.click("mac-trainer.hints.on")
    s.expect("hints-switch-is-on", lambda: s.ctl.rows("mac-trainer.settings")[0]["hints"], "on")
    archive_with_mouse(s, "msg-06")                          # key uses of E so far: J1.2, J1.8 = 2
    s.expect("hint-text", lambda: hint_text(s), FX["states"]["trainer_hint"])
    s.expect("mouse-archive-ran", lambda: s.ctl.core("message/msg-06/archived"), True)
    goto(s, "msg-07")
    s.arrive(("key", "E"), "mac-main")                       # the third key use of E
    s.expect("key-archive-ran", lambda: s.ctl.core("message/msg-07/archived"), True)
    archive_with_mouse(s, "msg-08")
    s.expect("mouse-archive-ran-again", lambda: s.ctl.core("message/msg-08/archived"), True)
    s.expect("hint-stops-after-three-key-uses", lambda: len(s.ctl.rows("mac-trainer.hint")), 0)

    # ------------------------------------------------------------------ J1.11 show-keys overlay
    s = j.step("J1.11", "Show-keys overlay marks the keys already used")
    s.key("⌘P")                      # ⇧? is a second key of the search in the Superhuman preset; the key list opens from the palette
    s.type(words("Show all keys"))   # the palette line that opens the key list (ShortcutsFeature.paletteCommands), by its title in the app's language
    s.arrive(("key", "Enter"), "mac-trainer")
    s.expect("overlay-open", lambda: s.ctl.uistate()["overlay"], "mac-trainer.keys")
    s.expect("overlay-lists-every-key", lambda: len(s.ctl.rows("mac-trainer.overlay.keys")), PRESET_KEY_ROWS)
    s.expect("overlay-names-the-anchor-actions", lambda: {r["key"]: r["action"] for r in s.ctl.rows("mac-trainer.overlay.keys") if r["key"] in PRESET_ANCHORS}, PRESET_ANCHORS)
    s.expect("used-key-marked", lambda: [r["used"] for r in s.ctl.rows("mac-trainer.overlay.keys") if r["key"] == "E"], [True])
    s.expect("unused-key-unmarked", lambda: [r["used"] for r in s.ctl.rows("mac-trainer.overlay.keys") if r["key"] == "X"], [False])   # X (select conversation) is never pressed in this journey
    s.key("Escape")
    s.expect("overlay-closes", lambda: s.ctl.uistate()["overlay"], None)

    # ------------------------------------------------------------------ J1.12 empty Primary
    s = j.step("J1.12", "All handled: Primary says All done; Tab opens the next split")
    s.expect("list-probe-control-has-rows", lambda: len(ids(s)) > 0, True)
    s.raw("fixture", "reauth", FAIL_ACC)                     # the failing account signs in again (J4.8), so msg-03 can be handled
    if j.ctl is not None:
        for _ in range(40):
            if rd(lambda: len(ids(s)), 0) == 0:
                break
            s.key("E")
    s.expect("no-rows-primary", lambda: len(s.ctl.rows(LIST)), 0)
    s.expect("empty-title", lambda: s.ctl.uistate()["empty_state"]["title"], words(FX["states"]["empty_primary"]["title"]))
    s.expect("empty-line-names-next-snooze", lambda: bool(re.search(r"\d\d:\d\d", s.ctl.uistate()["empty_state"]["line"])), True)   # JOURNEYS J1.12: the line names the next snooze return time (the fixture: "Next snooze returns at 14:00 ...")
    s.expect("tabs-probe-control", lambda: len(s.ctl.rows("mac-main.tabs")), len(TABS_NOW))
    s.arrive(("key", "Tab"), "mac-main")
    s.expect("next-tab-opens", lambda: s.ctl.uistate()["tab"], TAB_IDS_NOW[1])      # delta 16: the strip is Primary, All, ...: Tab from Primary opens the All view
    s.shot("J1.12-%s" % THEME)


# MUTANTS: per check, the mutation of the app (or of the fixture it renders) that must turn it red.
# J1.1/arrives-by-control | the Dock launch opens a different surface or none
# J1.1/theme-set | the app does not serve the theme verb
# J1.1/theme-applied | the app ignores the theme and stays light
# J1.1/row-count | the list drops or duplicates a row
# J1.1/row-ids-in-fixture-order | the list reorders rows or swaps two ids
# J1.1/pinned-rows-on-top | pinned rows are not sorted to the top
# J1.1/primary-unread | the Primary badge counts read mail or omits an account
# J1.1/primary-waiting | the waiting count is missing or stale
# J1.1/first-row-focus | focus starts on no row or on the last row
# J1.1/account-colour-per-row | every row shows the same account
# J1.1/failing-account-line | the failing account is not named in a line
# J1.1/launch-budget-ms | the first paint waits on a full sync
# J1.2/arrives-by-control | the J/K keys switch surface
# J1.2/row-leaves | E does nothing, or removes a different row
# J1.2/row-gone-from-list | the archived row stays below the new top
# J1.2/list-probe-control | the list reads fewer or more rows than the fixture minus one
# J1.2/next-row-focus | focus is lost after the archive
# J1.2/count-drops | the Primary count is not recomputed after the archive
# J1.2/archived-in-core | the archive is drawn but never written to the core
# J1.2/undo-entry-written | the archive writes no undo entry
# J1.3/arrives-by-control | H jumps to another surface
# J1.3/picker-lists-times-in-words | the time picker lists no times, or times not in words
# J1.3/row-leaves | the snooze leaves the row in the list
# J1.3/picker-closed | the time picker stays open after the pick
# J1.3/snoozed-count-up | the sidebar Snoozed count stays unchanged
# J1.3/snoozed-in-core | the snooze is drawn but never written
# J1.3/snooze-time-0800 | tomorrow morning uses now plus 24 hours instead of 08:00
# J1.3/undo-entry-written | the snooze writes no undo entry
# J1.4/arrives-by-control | P is bound to another action
# J1.4/row-on-top | the pinned row stays where it was
# J1.4/pin-mark | the row moves but shows no pin mark
# J1.4/pinned-in-core | the pin lives in the view only
# J1.4/pinned-row-keeps-focus | focus stays on the old index after the row moves to the top
# J1.4/on-every-account-filter | the pin shows only on the All accounts filter
# J1.4/filter-reset | the All accounts click leaves a filter on
# J1.4/undo-entry-written | the pin writes no undo entry
# J1.5/arrives-by-control | L jumps to another surface
# J1.5/picker-lists-nested-labels | the picker lists only the top level or no label
# J1.5/chip-on-row | the chip is missing after Enter
# J1.5/assignment-in-core | the label is drawn but never assigned
# J1.5/sidebar-lists-label | the sidebar label lists no message
# J1.5/other-row-unlabelled | the label lands on every row or on a neighbour
# J1.6/arrives-by-control | Enter opens nothing
# J1.6/subject | the reader shows the previous thread
# J1.6/tracker-count | the tracker count is hard-coded to 0
# J1.6/marked-read | opening does not mark the message read
# J1.6/one-interaction-budget-ms | the thread body renders after a network fetch
# J1.7/arrives-by-control | R opens the full composer instead of quick reply
# J1.7/reply-field-holds-the-keys | R does not give the reply field the keyboard, so the typed lines go to the list
# J1.7/sent-from-receiving-address | the reply is sent from the first account
# J1.7/undo-send-bar | no undo bar is shown after send
# J1.7/undo-send-counts-from-default | the bar counts from another number than the default window
# J1.7/outbox-entry | send writes no outbox entry
# J1.7/not-sent-inside-window | the message leaves at once, with no undo window
# J1.7/sent-after-window | the message never leaves the outbox
# J1.7/thread-lists-the-original | the thread probe lists nothing, or the original twice (the control of reply-in-thread)
# J1.7/reply-in-thread | the sent reply is missing from the thread, or listed twice
# J1.8/arrives-by-control | Esc leaves the reader open
# J1.8/mistake-archived | E on the list does nothing
# J1.8/row-returns | Z undoes the wrong action or nothing
# J1.8/row-returns-to-its-place | the row comes back at the bottom
# J1.8/undo-entry-consumed | the undo entry is not consumed
# J1.8/archived-flag-cleared | the row returns in the view but the core keeps it archived
# J1.9/arrives-by-control | the palette key opens nothing
# J1.9/palette-result | typing move finds no matching action
# J1.9/key-hint-column-exists | palette rows omit the key hint column
# J1.9/palette-closes-to-inbox | the palette stays open after the pick
# J1.9/moved-in-core | the row is hidden but the account is unchanged
# J1.9/row-shows-under-target-account | the row vanishes instead of showing under the target
# J1.9/undo-entry-written | the move writes no undo entry
# J1.10/hints-switch-is-on | the hints-on door does nothing, or the switch stays off
# J1.10/hint-text | no hint after a mouse archive
# J1.10/mouse-archive-ran | the archive button does nothing
# J1.10/arrives-by-control | E on the next row jumps to another surface
# J1.10/key-archive-ran | E archives a different row
# J1.10/mouse-archive-ran-again | the second archive button does nothing
# J1.10/hint-stops-after-three-key-uses | the hint keeps showing after three key uses
# J1.11/arrives-by-control | the show-keys key opens nothing
# J1.11/overlay-open | the overlay does not set the overlay state
# J1.11/overlay-names-the-anchor-actions | an anchor key names another action
# J1.11/overlay-lists-every-key | the overlay lists fewer keys than the preset
# J1.11/used-key-marked | no key is marked used
# J1.11/unused-key-unmarked | every key is marked used
# J1.11/overlay-closes | Esc leaves the overlay open
# J1.12/list-probe-control-has-rows | the list probe returns nothing even with mail present
# J1.12/no-rows-primary | handled rows stay in Primary
# J1.12/empty-title | the zero state has no All done title
# J1.12/empty-line-names-next-snooze | the zero state omits the next snooze return time
# J1.12/tabs-probe-control | the tabs region lists fewer tabs than the fixture
# J1.12/arrives-by-control | Tab jumps to another surface
# J1.12/next-tab-opens | Tab opens the wrong split


def selftest():
    """No app: the journey runs dry (no socket) and every check must name its mutant above, every mutant a check, and every catalogue key the
    journey reads must be in the catalogue. Writes no results file. The dry run's one line per check it could not judge is counted, not listed."""
    import contextlib
    import io
    from lib import harness
    harness.write_results = lambda *a, **k: None
    noise = io.StringIO()
    with contextlib.redirect_stderr(noise):
        r = harness.run_journey(sys.modules[__name__], os.path.abspath(__file__), force_dry=True)
    declared = harness.declared_mutants(os.path.abspath(__file__))
    bad = len(r["no_mutant"]) + len(r["stray_mutant"]) + len(MISSING_KEYS) + (0 if r["checks"] else 1)
    for label in r["no_mutant"]:
        print("NO MUTANT  %s" % label)
    for label in r["stray_mutant"]:
        print("STRAY      %s names no check" % label)
    for key in sorted(MISSING_KEYS):
        print("NOT IN THE CATALOGUE  %r" % key)
    print("%s: %d checks run dry, %d not judged (no app), the app's language read as %s (%s)" % (ID, r["checks"], r["unjudged"], app_language(), LANG["where"]))
    from lib import loadgate                      # round 8 of perf-archive: the shared load rule, through this journey's own wrapper
    from lib.harness import verdict_cases
    for name, ok in loadgate.wrapper_cases(lambda v, b: under_load(v, b), const("INTERACTION_COMMIT_MS"), Unreadable, ID + " under_load") + [(ID + " verdict: " + n, o) for n, o in verdict_cases()]:
        print("%-4s %s" % ("ok" if ok else "BAD", name)); bad += not ok
    print("%s selftest: %d checks, %d mutants, %d not red or missing" % (ID, r["checks"], len(declared), bad))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else 0)
