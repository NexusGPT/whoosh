"""J6: A new mail arrives while the owner works elsewhere on the Mac (docs/JOURNEYS.md J6.1-J6.7).
Written from the J6 step table and fixtures/tests-j6.json, never from a built screen. Expected values come from
that fixture, docs/design/fixtures/mac-main-sample.json and docs/CONSTANTS.md.

Drive channel: docs/CONTROL-SOCKET.md (deltas 33, 39, 44 and 46). Round r54 brings the ids and the path to the app of main53,
the way checks/parity/par-notch.py drives the same notch: the notch's own state is read from `rows mac-notch` (its `stage`,
`arrival_ms`), never from `uistate.surface` (that is the main window's surface, which the notch never changes); "Whoosh stays
behind the other app" is read from the hidden rig's own refusal log (`uistate.hidden.refused`: every activation the app asks for
is recorded there and refused), with the composer of J6.7, which must come to the front, as its positive control; the reply is
sent with the open mail's Send control (`mac-notch.open.send`) and its undo window runs on the wall clock (the `clock` verb does
not move it); the outbox and undo counts are read as deltas from a baseline, so the journey holds after J1 to J5 in one instance.
The clipboard and the window sharing state are read from the host (pbpaste, CGWindowList, the window's owner pid matched to the
rig copy), not from the app's own report. WHOOSH_J6_PBOARD and WHOOSH_J6_CGWINDOWS_FILE let a mutation harness point those reads
at a private board and a fake window list; the rig leaves them unset."""
import json
import os
import subprocess
import sys
import time
from lib.ctl import Unreadable
from lib import theme as _theme
from lib.fixture import ROOT, ACCOUNT, const

ID = "J6"
TITLE = _theme.title("A new mail arrives while another app is in front")
TARGET = "LOCAL"
VISIBLE_OUTCOME = ("The owner copies the code, replies and archives from the notch without Whoosh coming to the front; "
                   "the Unified Inbox shows the same state and the notch is absent from screen shares")

FX_PATH = os.path.join(ROOT, "fixtures", "tests-j6.json")
with open(FX_PATH, encoding="utf-8") as _f:
    F6 = json.load(_f)
CODE, NEXT = F6["arrivals"]["login-code"], F6["arrivals"]["next-mail"]
NOTCH, LIST, MAIN = "mac-notch", "mac-notch.list", "mac-main.list"
COMPOSER_HEAD = "mac-composer.header"
PBOARD = os.environ.get("WHOOSH_J6_PBOARD", "general")
SENTINEL = "j6-clipboard-sentinel"
UNDO_S = const("UNDO_SEND_DEFAULT_S")
RIG_PATH_MARK = "Whoosh-rig"
# Mutation harness (r54): WHOOSH_J6_MUTANT names acts to delete (deliver, codepill, pointer-rest, row2, quickreply, type, send,
# archive, settings, buttons, close-settings, drop, whoosh) or app faults to inject through the app's own controls (bring-front:
# Open in Whoosh during the arrival; share-off: the hidden-from-shares setting turned off). Unset on every recorded green run.
MUTANT = set(filter(None, os.environ.get("WHOOSH_J6_MUTANT", "").split(",")))          # scripts/launch-one.sh copies the rig under Application Support/Whoosh-rig*/


def button_id(title):
    """A notch button as the core stores it (notch_setting/buttons): its title, lower case, spaces as hyphens."""
    return title.lower().replace(" ", "-")


def clip_get():
    return subprocess.run(["pbpaste", "-pboard", PBOARD], capture_output=True, text=True, timeout=5).stdout


def clip_set(text):
    subprocess.run(["pbcopy", "-pboard", PBOARD], input=text, text=True, timeout=5)


_OWNER = {}


def _is_rig(pid):
    if pid not in _OWNER:
        out = subprocess.run(["ps", "-o", "comm=", "-p", str(pid)], capture_output=True, text=True, timeout=5)
        _OWNER[pid] = RIG_PATH_MARK in out.stdout
    return _OWNER[pid]


def cg_windows():
    """The rig copy's windows with the window server's own sharing state (0 none, 1 read only, 2 read write). The owner's
    installed copy also has a "Notch" window: rows are kept only when their owner process runs from the rig's folder.
    Each row: p owner pid, n title, s sharing state, l layer, b bounds {X, Y, Width, Height} in the window server's top-left
    origin (settingsgeneralmac-check matches a window by its frame; a row without b raised KeyError on main55)."""
    f = os.environ.get("WHOOSH_J6_CGWINDOWS_FILE")
    if f:
        return json.load(open(f, encoding="utf-8"))
    js = ('ObjC.import("CoreGraphics");var l=ObjC.deepUnwrap(ObjC.castRefToObject($.CGWindowListCopyWindowInfo('
          '$.kCGWindowListOptionAll,0)));JSON.stringify(l.filter(function(w){return w.kCGWindowOwnerName=="Whoosh"})'
          '.map(function(w){return {p:w.kCGWindowOwnerPID,n:w.kCGWindowName||"",s:w.kCGWindowSharingState,l:w.kCGWindowLayer,b:w.kCGWindowBounds}}))')
    out = subprocess.run(["osascript", "-l", "JavaScript", "-e", js], capture_output=True, text=True, timeout=20)
    if out.returncode != 0:
        raise Unreadable("window list: %s" % out.stderr.strip())
    return [w for w in json.loads(out.stdout or "[]") if _is_rig(w["p"])]


def act(name, fn, *a):
    """One act of the journey, deleted when the mutation harness names it."""
    if name not in MUTANT:
        fn(*a)


def ids(s, ax=LIST):
    return [r["id"] for r in s.ctl.rows(ax)]


def notch(s, key):
    return s.ctl.rows(NOTCH)[0][key]


def clip(s):
    s.ctl  # dry run: raises, the check is not judged
    return clip_get()


def expect_ctx(s, name, thunk, expected, ctx):
    """Like s.expect, and a failing case also prints what decides it: `ctx` is read only when the case fails (a read that fails says why)."""
    def run():
        got = thunk()
        s._eg = (expected, got)
        if got == expected:
            return True, ""
        try:
            more = ctx()
        except Exception as e:      # noqa: BLE001  the context never hides the verdict it explains
            more = "context unreadable (%s: %s)" % (type(e).__name__, e)
        return False, "expected %r, got %r | %s" % (expected, got, more)
    s._record(name, run)


def composer_open(s):
    """True while the composer's header row says it is open. A closed composer serves no header row (ComposerMacFeature.rowSources: `guard model.isOpen else { return [] }`),
    so "no rows" is closed; any other failure stays unreadable."""
    try:
        return any(r["is_open"] for r in s.ctl.rows(COMPOSER_HEAD))
    except Unreadable as e:
        if "no rows" in str(e):
            return False
        raise


def activations(s):
    """How many times the app has asked to come to the front (the hidden rig records each ask and refuses it)."""
    return sum(1 for r in s.ctl.uistate()["hidden"]["refused"] if r == "activate")


def sharing(s, name):
    s.ctl  # dry run: raises, the check is not judged
    return sorted(w["s"] for w in cg_windows() if w["n"] == name)




def outbox_with_attachment(core, ob7, oid, recipient, file):
    """True when the entry the drop's send queued carries `file` (and, when the core serves it, `recipient`); else a dict that decides app against check:
    the outbox count, the baseline, and the fields of every entry from the baseline up (state, to, attachments), each as the core answered or
    "not served". The entries are read from the baseline `ob7` (taken before the send, as the journey's other outbox reads do), never from index 0:
    earlier journeys leave entries below it. The fixture core serves `outbox_entry/<n>/state` only (FixtureCore.swift:1787), so `to` and
    `attachments` by index are unserved there: the files are then read by the entry's id (`outbox/<id>/attachments`, FixtureLabels.swift:735),
    the id the composer's undo bar named while the window was open (`oid`). The recipient is judged only where the core serves `to`."""
    def read(path):
        try:
            return core(path)
        except Unreadable as e:
            return "not served (%s)" % str(e)[:100]
    if ob7 is None:
        raise Unreadable("the outbox count before the send was not read (no baseline)")
    count = core("outbox_entry/count")
    fields = [{"index": i, **{k: read("outbox_entry/%d/%s" % (i, k)) for k in ("state", "to", "attachments")}} for i in range(ob7, count)]
    by_id = read("outbox/%s/attachments" % oid) if oid else "no outbox id from the undo bar"
    names = None
    try:
        # the core's answer arrives parsed (a dict) or as the JSON text; a "not served" or "no outbox id" sentence is neither
        doc = json.loads(by_id) if isinstance(by_id, str) else by_id
        names = [a["file_name"] for a in doc["attachments"]] if isinstance(doc, dict) else None
    except (ValueError, KeyError, TypeError):
        names = None
    to_served = [f["to"] for f in fields if isinstance(f["to"], list)]
    ok = count > ob7 and names == [file] and (not to_served or [recipient] in to_served)
    return True if ok else {"count": count, "baseline": ob7, "entries_from_baseline": fields, "outbox_id": oid, "files_by_id": by_id, "wanted": [recipient, file]}


def drive(j):
    live = j.ctl is not None
    saved = clip_get() if live else None
    try:
        steps(j)
    finally:
        if live:
            clip_set(saved)


def steps(j):
    live = j.ctl is not None

    def ctl_try(fn):
        try:
            return fn() if live else None
        except Unreadable:
            return None

    def wait(sec):
        if live:
            time.sleep(sec)

    # --- J6.1 the mail drops out of the notch while another app is in front
    s = j.step("J6.1", "Mail appears in the notch")
    if live:
        clip_set(SENTINEL)
    s.click(NOTCH + ".stage.reset")              # the notch's own empty state (the rig's stage control); the core is untouched
    s.raw("fixture", "notch-geometry", "notch")
    s.raw("fixture", "front-app", F6["front_app"])
    _theme.apply(s)
    act0 = ctl_try(lambda: activations(s))
    s.expect("notch-empty-before", lambda: ids(s), [])
    act("deliver", s.raw, "fixture", "deliver", "login-code", FX_PATH, "idle")
    s.raw("clock", "advance", 1)
    act("deliver", s.raw, "fixture", "deliver", "next-mail", FX_PATH, "idle")
    if "bring-front" in MUTANT:
        s.click(NOTCH + ".whoosh")
    wait(0.6)
    s.expect("notch-shows-mail", lambda: ids(s), [NEXT["id"], CODE["id"]])
    s.expect("sender-subject-account", lambda: [(r["sender"], r["subject"], r["account"]) for r in s.ctl.rows(LIST)],
             [(m["sender"], m["subject"], m["account"]) for m in (NEXT, CODE)])
    s.expect("code-pill", lambda: {r["id"]: r["code"] for r in s.ctl.rows(LIST)}, {NEXT["id"]: None, CODE["id"]: CODE["code"]})
    s.expect("arrival-measured", lambda: (notch(s, "arrival_ms") or 0) > 0, True)
    s.expect_le("arrival-ms", lambda: notch(s, "arrival_ms"), const("NOTCH_ARRIVAL_MS"))
    s.expect("other-app-still-front", lambda: activations(s) - act0, 0)

    # --- J6.2 copy the code from the pill
    s = j.step("J6.2", "Copy the code")
    s.expect("clipboard-before", lambda: clip(s), SENTINEL)
    s.expect("unread-before", lambda: s.ctl.core("message/%s/read" % CODE["id"]), False)
    stage0 = ctl_try(lambda: notch(s, "stage"))
    act("codepill", s.click, NOTCH + ".codepill")
    s.expect("arrives-by-control", lambda: (stage0, notch(s, "stage"), s.ctl.rows(NOTCH + ".codepill")[0]["text"]), ("drop", "drop", "Copied"))
    s.expect("copied-label", lambda: s.ctl.rows(NOTCH + ".codepill")[0]["text"], "Copied")
    s.expect("clipboard-is-the-code", lambda: clip(s), CODE["code"])
    s.expect("read-not-archived", lambda: (s.ctl.core("message/%s/read" % CODE["id"]),
                                           s.ctl.core("message/%s/archived" % CODE["id"])), (True, False))

    # --- J6.3 the pointer rests on the notch; open one in place
    s = j.step("J6.3", "Open one in place")
    s.expect("list-collapsed-before", lambda: notch(s, "stage"), "drop")
    act("pointer-rest", s.raw, "fixture", "pointer-rest", NOTCH)
    s.expect("list-opens-on-rest", lambda: notch(s, "stage"), "list")
    s.expect("nothing-open-before", lambda: s.ctl.rows(NOTCH + ".open.body"), [])
    act("row2", s.click, LIST + ".row.2")
    s.expect("row-opens-in-place", lambda: [r["id"] for r in s.ctl.rows(NOTCH + ".open.body")], [CODE["id"]])
    s.expect("first-lines", lambda: s.ctl.rows(NOTCH + ".open.body")[0]["text"], CODE["first_lines"])
    s.expect_in("actions-shown", lambda: [r["title"] for r in s.ctl.rows(NOTCH + ".open.actions")], "Archive")
    # owner's picks of 14 Nov 2026 (docs/surfaces/mac-notch.md, "its word by default"; plan "Reply folds the mail to one line"): a mail opens on its
    # words and Reply starts the reply, so the reply field is one press away
    act("reply", s.click, NOTCH + ".open.reply")
    s.expect("quick-reply-field", lambda: len(s.ctl.rows(NOTCH + ".open.quickreply")), 1)

    # --- J6.4 quick reply from the notch
    s = j.step("J6.4", "Quick reply from the notch")
    act("quickreply", s.click, NOTCH + ".open.quickreply")
    s.expect("reply-field-focused-on-the-open-mail", lambda: [(r["focused"], r["reply_to"]) for r in s.ctl.rows(NOTCH + ".open.quickreply")],
             [(True, CODE["id"])])
    s.expect("from-address", lambda: [r["address"] for r in s.ctl.rows(NOTCH + ".open.from")], [ACCOUNT[CODE["account"]]["address"]])
    act("type", s.type, F6["reply"])
    s.expect("reply-typed", lambda: [r["text"] for r in s.ctl.rows(NOTCH + ".open.quickreply")], [F6["reply"]])
    ob0 = ctl_try(lambda: j.ctl.core("outbox_entry/count"))
    act("send", s.click, NOTCH + ".open.send")
    s.expect("outbox-entry", lambda: s.ctl.core("outbox_entry/count") - ob0, 1)
    entry = lambda f: s.ctl.core("outbox_entry/%d/%s" % (ob0, f)) if s.ctl.core("outbox_entry/count") > ob0 else None   # noqa: E731  no entry: the check fails, it is not unjudged
    s.expect("outbox-undoable", lambda: entry("state"), "undoable")
    s.expect("reply-on-the-open-mails-thread", lambda: s.ctl.core("outbox_thread/" + CODE["thread"]), ["undoable"])
    wait(UNDO_S / 2)
    s.expect("undo-bar-fills", lambda: [(r["text"], r["action"], 0.3 <= r["progress"] <= 0.7) for r in s.ctl.rows(NOTCH + ".undo")],
             [("Sending", "Undo", True)])
    wait(UNDO_S / 2 + 1.5)
    s.expect("sent-after-window", lambda: ([r["text"] for r in s.ctl.rows(NOTCH + ".undo")], entry("state")),
             (["Sent"], "sent"))

    # --- J6.5 archive the next mail from the notch
    s = j.step("J6.5", "Archive the next mail")
    undo0 = ctl_try(lambda: j.ctl.core("undo_entry/count"))
    s.expect("in-notch-and-inbox-before", lambda: (NEXT["id"] in ids(s), NEXT["id"] in ids(s, MAIN)), (True, True))
    act("archive", s.click, LIST + ".row.1.archive")
    s.expect("leaves-notch", lambda: ids(s), [CODE["id"]])
    s.expect("archived-in-core", lambda: s.ctl.core("message/%s/archived" % NEXT["id"]), True)
    s.expect("undo-entry-written", lambda: s.ctl.core("undo_entry/count") - undo0, 1)
    s.expect("gone-from-inbox", lambda: NEXT["id"] in ids(s, MAIN), False)

    # --- J6.6 notch settings
    s = j.step("J6.6", "Notch settings")
    target = sorted(button_id(b) for b in F6["notch_buttons_after"])
    act("settings", s.click, NOTCH + ".settings")
    s.expect("arrives-by-control", lambda: s.ctl.uistate().get("surface"), "mac-settings-general")
    if "share-off" in MUTANT:
        s.click("mac-settings-general.notch.hidden")
    s.expect("hidden-from-shares-on", lambda: s.ctl.core("notch_setting/hidden_in_screen_share"), True)
    # the start set picked on 14 Nov 2026 (mark read, archive, reply) IS this journey's target: one button goes off first, so the save below changes something
    if sorted(ctl_try(lambda: j.ctl.core("notch_setting/buttons")) or []) == target:
        act("buttons", s.click, "mac-settings-general.notch.buttons." + target[-1])
    s.expect("buttons-differ-before", lambda: sorted(s.ctl.core("notch_setting/buttons")) != target, True)
    now = ctl_try(lambda: j.ctl.core("notch_setting/buttons")) or []
    for b in target:          # a checkbox per button: click the ones that differ from the target
        if b not in now:
            act("buttons", s.click, "mac-settings-general.notch.buttons." + b)
    for b in now:
        if b not in target:
            act("buttons", s.click, "mac-settings-general.notch.buttons." + b)
    s.expect("buttons-saved", lambda: sorted(s.ctl.core("notch_setting/buttons")), target)
    s.expect("notch-window-sharing-none", lambda: sharing(s, NOTCH_TITLE), [0])
    s.expect("main-window-shareable-control", lambda: (lambda w: len(w) >= 1 and 0 not in w)(
        [x["s"] for x in cg_windows() if x["n"] not in (NOTCH_TITLE, "") and x["l"] == 0]) if live else s.ctl, True)

    # --- J6.7 drop a file on the notch
    s = j.step("J6.7", "Drop a file on the notch")
    d = F6["drop"]
    s.expect("settings-open-before", lambda: [(f.get("title"), f.get("kind")) for f in s.ctl.winframes()].count(("Settings", "window")), 1)
    act("close-settings", s.click, "mac-settings-window.close")
    # the Settings window's frame row (winframes title "Settings", kind window): present before the close, gone after it
    s.expect("settings-closed", lambda: [(f.get("title"), f.get("kind")) for f in s.ctl.winframes()].count(("Settings", "window")), 0)
    s.expect("notch-still-present", lambda: ids(s), [CODE["id"]])
    act7 = ctl_try(lambda: activations(s))
    act("drop", s.raw, "fixture", "drop-file", NOTCH, d["file"])
    wait(0.6)
    s.expect("composer-opens-in-front", lambda: s.ctl.uistate()["surface"], "mac-composer")
    # "in front" while another app is in front needs the app to ask macOS to come forward: the ask is what the refusal log counts
    s.expect("composer-asks-to-come-front", lambda: activations(s) - act7 >= 1, True)
    s.expect("file-attached", lambda: [a["name"] for a in s.ctl.rows("mac-composer.attachments")], [d["file"]])
    ob7 = ctl_try(lambda: j.ctl.core("outbox_entry/count"))
    s.key("⇧⌘O")
    s.type(d["recipient"] + ",")
    s.expect("recipient-typed", lambda: s.ctl.rows(COMPOSER_HEAD)[0]["to"], [d["recipient"]])
    s.key("⇧⌘M")
    s.type(d["line"])
    s.key("⌘Enter")
    # The composer shows its undo bar while the send can still be taken back (par-composermac send-key-reaches-composer); it closes when the window ends.
    s.expect("send-opens-the-undo-window", lambda: s.ctl.rows("mac-composer.undo.bar")[0]["undo_offered"], True)
    oid7 = ctl_try(lambda: j.ctl.rows("mac-composer.undo.bar")[0]["outbox_id"])      # the entry's id while its window is open: the entry is read by it after the window ends
    act("window-ends", s.raw, "clock", "advance", 60)
    # The composer closes when the undo window ends, and that window runs on the wall clock (UNDO_SEND_DEFAULT_S, docs/CONSTANTS.md; the `clock` verb does not move it):
    # the header row is read until it says closed, for at most the window and a margin, never a fixed wait.
    if live:
        end = time.monotonic() + UNDO_S + 6
        while time.monotonic() < end and ctl_try(lambda: composer_open(s)) is not False:
            time.sleep(0.2)
    # docs/JOURNEYS.md (Composer (Mac), exits): "none: send closes it (J3.7)". A miss prints what decides it: the bar's own words and whether it still
    # offers undo, the entry's state, the header row's flags, and the surface (main81: the composer still open UNDO_S + 6 s after the window ended).
    def why_open():
        bar = ctl_try(lambda: s.ctl.rows("mac-composer.undo.bar"))
        head = ctl_try(lambda: [{k: r.get(k) for k in ("is_open", "has_draft", "draft_id", "files_held")} for r in s.ctl.rows(COMPOSER_HEAD)])
        state = ctl_try(lambda: s.ctl.core("outbox_entry/%d/state" % ob7)) if isinstance(ob7, int) else None
        return "undo bar rows %r; header %r; outbox entry %r (picked by baseline index %r) state %r; surface %r (None: not readable)" % (bar, head, oid7, ob7, state, ctl_try(lambda: s.ctl.uistate().get("surface")))
    expect_ctx(s, "send-closes-the-composer", lambda: composer_open(s), False, why_open)
    s.expect("outbox-entry-queued", lambda: s.ctl.core("outbox_entry/count") - ob7, 1)
    s.expect("outbox-with-attachment", lambda: outbox_with_attachment(s.ctl.core, ob7, oid7, d["recipient"], d["file"]), True)
    # positive control for every "activations" delta above: the one act that asks to bring Whoosh forward (NotchActs.openInWhoosh)
    actw = ctl_try(lambda: activations(s))
    act("whoosh", s.click, NOTCH + ".whoosh")
    s.expect("activation-probe-control", lambda: activations(s) - actw >= 1, True)


NOTCH_TITLE = "Notch"      # the panel's window title (NotchIds.windowTitle, English rig)


# MUTANTS
# J6.1/theme-applied | the app ignores the theme and stays light
# J6.1/notch-empty-before | the notch already lists a mail before any arrival
# J6.1/notch-shows-mail | the arrival event never reaches the notch
# J6.1/sender-subject-account | the row shows another sender, subject or account
# J6.1/code-pill | the code is not extracted from the mail
# J6.1/arrival-measured | the arrival measurement reads zero
# J6.1/arrival-ms | the notch waits for the next poll
# J6.1/other-app-still-front | the notch brings Whoosh to the front
# J6.2/clipboard-before | the arrival copies the code before any click
# J6.2/unread-before | the mail is already read before the click
# J6.2/arrives-by-control | the code pill click collapses the notch or opens Whoosh instead
# J6.2/copied-label | no Copied confirmation
# J6.2/clipboard-is-the-code | the clipboard gets another string
# J6.2/read-not-archived | copying a code archives the mail
# J6.3/list-collapsed-before | the list is already open before the pointer rests
# J6.3/list-opens-on-rest | a pointer rest opens nothing
# J6.3/nothing-open-before | a mail is already open before the click
# J6.3/row-opens-in-place | the row does not open in place
# J6.3/first-lines | the body is empty
# J6.3/actions-shown | the actions are missing
# J6.3/quick-reply-field | Reply on the open mail starts no quick reply field
# J6.4/reply-field-focused-on-the-open-mail | the reply field is not focused, or answers another mail
# J6.4/from-address | the reply shows no from-address, or another account's
# J6.4/reply-typed | typing never reaches the reply field
# J6.4/outbox-entry | send writes no outbox entry
# J6.4/outbox-undoable | the entry starts as sent, with no undo window
# J6.4/reply-on-the-open-mails-thread | the reply is filed on another mail's thread
# J6.4/undo-bar-fills | the bar does not fill during the window, or has no Undo
# J6.4/sent-after-window | the reply never reports Sent
# J6.5/in-notch-and-inbox-before | the next mail is missing from the notch or the inbox before the click
# J6.5/leaves-notch | the mail stays in the notch
# J6.5/archived-in-core | the archive is not written
# J6.5/undo-entry-written | the archive writes no undo entry
# J6.5/gone-from-inbox | the inbox still lists it
# J6.6/arrives-by-control | the settings control is missing
# J6.6/hidden-from-shares-on | the default is off
# J6.6/buttons-differ-before | the buttons already are the target
# J6.6/buttons-saved | the buttons are not saved
# J6.6/notch-window-sharing-none | the notch window can be captured
# J6.6/main-window-shareable-control | the window probe cannot tell shareable from hidden windows
# J6.7/settings-open-before | the Settings window is not listed while it is open (the probe is blind)
# J6.7/settings-closed | the Settings close control does nothing
# J6.7/notch-still-present | the notch vanishes when Settings closes
# J6.7/composer-opens-in-front | the drop does nothing
# J6.7/composer-asks-to-come-front | the composer opens behind the app in front (no ask to come forward)
# J6.7/file-attached | the file is not attached
# J6.7/recipient-typed | the typed recipient is not taken
# J6.7/send-opens-the-undo-window | the send queues nothing, so no undo bar shows
# J6.7/send-closes-the-composer | the composer stays open after the undo window ends
# J6.7/outbox-entry-queued | send queues nothing
# J6.7/outbox-with-attachment | the sent mail lacks the file
# J6.7/activation-probe-control | the refusal log never counts an ask to come forward (the probe is blind)


# ---------------------------------------------------------------------------------------------------------------- self-test (the kit is journeys/J4.py)
# python3 journeys/J6.py --selftest: the whole journey against a stand-in app on a private socket in a temporary folder, with the host reads (the clipboard, the window
# list) served by the same stand-in in memory: no pasteboard is touched, no window opens, and the stand-in's clock is moved by the journey's own waits.
def _kit():
    import importlib.util
    spec = importlib.util.spec_from_file_location("journey_selftest_kit", os.path.join(os.path.dirname(os.path.abspath(__file__)), "J4.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def _app6(kit):
    BUTTONS0 = ["archive", "mark-read", "snooze"]
    CODE_ROW = lambda m: {"id": m["id"], "sender": m["sender"], "subject": m["subject"], "account": m["account"], "code": m["code"]}

    class App6(kit.Sim):
        # a count read only against its baseline needs the wrong count itself, not a bent list
        FAULTS = {"J6.1/other-app-still-front": lambda out: _with_refused(out, ["activate"]),
                  "J6.7/composer-asks-to-come-front": lambda out: _with_refused(out, []),
                  "J6.7/activation-probe-control": lambda out: _with_refused(out, []),
                  "J6.3/quick-reply-field": lambda out: dict(out, rows=[]),
                  "J6.6/buttons-differ-before": lambda out: dict(out, value=sorted(button_id(b) for b in F6["notch_buttons_after"])),
                  "J6.7/settings-closed": lambda out: dict(out, frames=[{"title": "Settings", "kind": "window"}])}

        def setup(self):
            self.stage, self.arrived, self.clip, self.opened, self.focused, self.typed = "idle", [], SENTINEL, None, False, ""
            self.replying = False
            self.entries, self.read, self.archived, self.undo_n, self.buttons = [], set(), set(), 0, list(BUTTONS0)
            self.composer = None
            self.frames = [{"title": "Settings", "kind": "window"}]

        def host_clip(self):
            if self.inert:
                return ""
            return kit.bend(self.clip) if self.mutant and self.mutant == self.current else self.clip

        def host_windows(self):
            if self.inert:
                return []
            w = [{"p": 1, "n": "Notch", "s": 0, "l": 25, "b": {}}, {"p": 1, "n": "Whoosh", "s": 1, "l": 0, "b": {}}]
            return kit.bend(w) if self.mutant and self.mutant == self.current else w

        def entry_state(self, e):
            return "sent" if self.now >= e["at"] + UNDO_S else "undoable"

        def rows_for(self, ax):
            if ax == NOTCH:
                return [{"stage": self.stage, "arrival_ms": 300}]
            if ax == LIST:
                return [CODE_ROW(m) for m in self.arrived]
            if ax == MAIN:
                return [{"id": m["id"]} for m in [NEXT, CODE] if m["id"] not in self.archived]
            if ax == NOTCH + ".codepill":
                return [{"text": "Copied" if self.clip == CODE["code"] else CODE["code"]}]
            if ax == NOTCH + ".open.body":
                return [{"id": self.opened["id"], "text": self.opened["first_lines"]}] if self.opened else []
            if ax == NOTCH + ".open.actions":
                return [{"title": "Archive"}, {"title": "Reply"}] if self.opened else []
            if ax == NOTCH + ".open.quickreply":
                return [{"focused": self.focused, "reply_to": self.opened["id"], "text": self.typed}] if self.opened and self.replying else []
            if ax == NOTCH + ".open.from":
                return [{"address": ACCOUNT[self.opened["account"]]["address"]}] if self.opened else []
            if ax == NOTCH + ".undo":
                if not self.entries or self.composer:
                    return []
                e = self.entries[0]
                done = self.entry_state(e) == "sent"
                return [{"text": "Sent" if done else "Sending", "action": "Undo", "progress": 1.0 if done else (self.now - e["at"]) / UNDO_S}]
            if ax == COMPOSER_HEAD:
                c = self.composer
                if not c or (c["sent_at"] is not None and self.now >= c["sent_at"] + UNDO_S):
                    return None
                return [{"is_open": True, "to": list(c["to"])}]
            if ax == "mac-composer.attachments":
                return [{"name": self.composer["file"]}] if self.composer else None
            if ax == "mac-composer.undo.bar":
                c = self.composer
                return [{"undo_offered": c["sent_at"] is not None, "outbox_id": "outbox-7"}] if c else None
            return None

        def core_for(self, path):
            p = path.split("/")
            if path == "outbox_entry/count":
                return len(self.entries)
            if p[0] == "outbox_entry" and len(p) == 3 and p[2] == "state" and int(p[1]) < len(self.entries):
                return self.entry_state(self.entries[int(p[1])])
            if path == "outbox_thread/" + CODE["thread"]:
                return [self.entry_state(e) for e in self.entries if e["thread"] == CODE["thread"]] if self.entries and self.entries[0]["thread"] == CODE["thread"] else []
            if path == "outbox/outbox-7/attachments":
                return {"attachments": [{"file_name": F6["drop"]["file"]}]} if self.composer and self.composer["sent_at"] is not None else kit.NOT_SERVED
            if p[0] == "message" and len(p) == 3:
                return (p[1] in self.read) if p[2] == "read" else (p[1] in self.archived)
            if path == "undo_entry/count":
                return self.undo_n
            if path == "notch_setting/hidden_in_screen_share":
                return True
            if path == "notch_setting/buttons":
                return list(self.buttons)
            return kit.NOT_SERVED

        def answer(self, verb, args):
            if self.inert or verb in ("rows", "core", "uistate", "winframes"):
                return super().answer(verb, args)
            if verb in ("key", "capture", "type"):
                return self.keys(verb, args)
            if verb == "clock":
                return {"ok": True}      # the wall clock runs the undo window: the clock verb does not move it
            if verb == "fixture":
                return self.fixture(args)
            return self.click(args[0])

        def fixture(self, a):
            v = a[0]
            if v == "deliver":
                m = {"login-code": CODE, "next-mail": NEXT}[a[1]]
                self.arrived.insert(0, m)
                self.stage = "drop"
            elif v == "pointer-rest":
                self.stage = "list"
            elif v == "drop-file":
                self.composer = {"to": [], "file": a[2], "sent_at": None}
                self.state["surface"] = "mac-composer"
                self.state["hidden"]["refused"].append("activate")
            elif v not in ("notch-geometry", "front-app"):
                raise kit.Gone("fixture %s is not served" % v)
            return {"ok": True}

        def keys(self, verb, args):
            if verb == "type":
                if self.composer is not None:
                    self.composer["to"] = [args[0].rstrip(",")] if args[0].endswith(",") else self.composer["to"]
                else:
                    self.typed = args[0]
            elif verb == "key" and args[0] == "⌘Enter" and self.composer is not None:
                self.composer["sent_at"] = self.now
                self.entries.append({"at": self.now, "thread": "thr-composer"})
            return {"ok": True}

        def click(self, ax):
            if ax == NOTCH + ".stage.reset":
                self.stage, self.arrived, self.opened, self.replying = "idle", [], None, False
            elif ax == NOTCH + ".codepill":
                self.clip = CODE["code"]
                self.read.add(CODE["id"])
            elif ax == LIST + ".row.2":
                self.opened = CODE
            elif ax == NOTCH + ".open.reply":
                self.replying = self.opened is not None
            elif ax == NOTCH + ".open.quickreply":
                self.focused = True
            elif ax == NOTCH + ".open.send":
                self.entries.append({"at": self.now, "thread": CODE["thread"]})
            elif ax == LIST + ".row.1.archive":
                self.arrived = [m for m in self.arrived if m["id"] != NEXT["id"]]
                self.archived.add(NEXT["id"])
                self.undo_n += 1
            elif ax == NOTCH + ".settings":
                self.state["surface"] = "mac-settings-general"
            elif ax.startswith("mac-settings-general.notch.buttons."):
                b = ax.rsplit(".", 1)[1]
                self.buttons.remove(b) if b in self.buttons else self.buttons.append(b)
            elif ax == "mac-settings-window.close":
                self.frames = []
            elif ax == NOTCH + ".whoosh":
                self.state["hidden"]["refused"].append("activate")
            else:
                raise kit.Gone("click %s: nothing to click at %s" % ((ax,), ax))
            return {"ok": True}
    return App6


def _with_refused(out, refused):
    st = dict(out["state"])
    st["hidden"] = {"refused": list(refused)}
    return dict(out, state=st)


def selftest():
    kit = _kit()
    App6 = _app6(kit)
    mod = sys.modules[__name__]
    saved = (mod.clip_get, mod.clip_set, mod.cg_windows)

    def make(mutant, inert):
        sim = App6(mutant, inert)
        mod.clip_get = sim.host_clip
        mod.clip_set = lambda text: setattr(sim, "clip", text)
        mod.cg_windows = sim.host_windows
        return sim

    def put_back():
        mod.clip_get, mod.clip_set, mod.cg_windows = saved
    return kit.run_selftest(mod, os.path.abspath(__file__), make, {}, after=put_back)


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else 0)
