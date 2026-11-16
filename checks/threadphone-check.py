#!/usr/bin/env python3
"""Thread (iPhone): open, code pill, copy, read flag, offline, back, the note, every control once (docs/JOURNEYS.md J7.2, J7.3, J8.4; C-J7.2).

Drives the RUNNING iPhone app in the simulator over the control socket ($WHOOSH_SOCKET_DIR, docs/CONTROL-SOCKET.md)
through journeys/lib, the same harness as checks/parity/par-inboxphone.py. Expected values come from
docs/design/fixtures/mac-main-sample.json, fixtures/tests-j7.json and the journey words, never from a built screen.
iPhone ax ids follow `<surface>.<region>[.<element>...][.<index>]`; the screen registers them (Sources/Features/ThreadPhone/).
Run: sh scripts/launch-one.sh <iphone-bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/threadphone-check.py'
     python3 checks/threadphone-check.py --selftest     the cases against a stand-in socket, then one mutant per case: each must go red
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0 (a check that could not run is not green).
`fixture offline on|off` is served (Sources/ControlSocket/FixtureVerbs.swift:184); the step threadphone-offline uses it.
"""
import json
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, Journey, declared_mutants              # noqa: E402
from lib.ctl import Unreadable                                              # noqa: E402
from lib.fixture import FX, ROWS, ACCOUNT, MSG, write_arrival              # noqa: E402

ID = "threadphone"
TITLE = "Thread (iPhone): open, code pill, copy, read flag, offline, back, note, every control"
TARGET = "LOCAL"
# No `PLATFORM` line: the harness (journeys/lib/harness.py platform_mismatch) turned every case NOT JUDGED on a Mac build ("iPhone journey on a Mac build",
# main88: 46 of 46). The iPhone screens' doors and rows are registered by the app on the Mac test copy too (settingsphone-check drives its doors there:
# `iphone-mailboxes.settings` answered, it did not say "nothing to click"), so this check drives them there as well; a door or row the Mac copy does
# not serve is still NOT JUDGED with the app's own words, and a wrong answer is a FAIL.
VISIBLE_OUTCOME = ("the thread opens from a row; the code pill copies the code and marks read, never archives; offline the body says it is "
                   "not downloaded; back keeps the mail listed; the note saves; every control does something visible")

with open(os.path.join(ROOT, "fixtures", "tests-j7.json"), encoding="utf-8") as _f:
    T = json.load(_f)
R1 = MSG["msg-01"]
R2 = ROWS[1]
THREAD = FX["open_thread"]
ARRIVAL = T["arrival"]
CODE_MAIL = dict(ARRIVAL, sender=ARRIVAL["sender"], **{"from": "no-reply@signin-example.com",
                 "first_lines": "Your sign-in code is %s. It expires in 10 minutes." % ARRIVAL["code"]})
NOTE = "Ask Karen about the cover paper"
LIST = "iphone-inbox.list"
NOT_DOWNLOADED = "not downloaded yet"
COPIED = "Copied"
ATTACHMENT_NAMES = [re.sub(r" \(.*\)$", "", a) for a in THREAD["messages"][-1]["attachments"]]


def norm(text):
    return re.sub(r"[^a-z0-9 ]", "", str(text).lower()).strip()


def why_not_driven():
    d = os.environ.get("WHOOSH_SOCKET_DIR", "")
    if d and os.path.exists(os.path.join(d, "whoosh.sock")):
        return ""
    try:
        out = subprocess.run(["xcrun", "simctl", "list", "runtimes"], capture_output=True, text=True, timeout=20).stdout
    except (OSError, subprocess.SubprocessError):
        out = ""
    if "iOS" not in out:
        return "the iPhone simulator cannot run: Xcode reports iOS 26.5 is not installed (owner step: Xcode > Settings > Components > iOS 26.5)"
    return "no iPhone app socket: launch the iPhone app in the simulator, then set WHOOSH_SOCKET_DIR"


def one(step, ax, key):
    """A thunk: the first row of a region, one field."""
    return lambda: step.ctl.rows(ax)[0][key]


def snap(step, mid):
    """What the app says about one mail and the open thread, as text for a failure line (the next log decides: nothing here is judged)."""
    out = []
    for label, fn in (("open", lambda: step.ctl.rows("iphone-thread.open")), ("code-pill", lambda: step.ctl.rows("iphone-thread.code-pill")),
                      ("subject", lambda: step.ctl.rows("iphone-thread.subject")), ("body", lambda: step.ctl.rows("iphone-thread.body")),
                      ("messages", lambda: [(m.get("from"), m.get("folded")) for m in step.ctl.rows("iphone-thread.messages")]),
                      ("list row", lambda: [r for r in step.ctl.rows(LIST) if r.get("id") == mid]),
                      ("list ids", lambda: [r.get("id") for r in step.ctl.rows(LIST)][:6]),
                      ("core read", lambda: step.ctl.core("message/%s/read" % mid)), ("core snoozed", lambda: step.ctl.core("message/%s/snoozed" % mid)),
                      ("core archived", lambda: step.ctl.core("message/%s/archived" % mid)), ("surface", lambda: step.ctl.uistate().get("surface"))):
        try:
            out.append("%s=%r" % (label, fn()))
        except Exception as e:               # a read that fails is part of the picture, never hidden
            out.append("%s unreadable (%s: %s)" % (label, type(e).__name__, e))
    return "; ".join(out)


def showing(step, read, want, mid):
    """`read()` unchanged when it equals `want`; else an IndexError whose text carries the snapshot, so the failure line names the rows behind it."""
    def f():
        try:
            v = read()
        except (IndexError, KeyError) as e:
            raise IndexError("%s: %s | %s" % (type(e).__name__, e, snap(step, mid)))
        if v != want:
            raise IndexError("got %r, expected %r | %s" % (v, want, snap(step, mid)))
        return v
    return f


def listed(step):
    return lambda: [r["id"] for r in step.ctl.rows(LIST)]


def row_of(step, message_id):
    """The list index of a message at the moment of the act (the drive body, not a check); 0 when it cannot be read."""
    try:
        return [r["id"] for r in step.ctl.rows(LIST)].index(message_id)
    except (Unreadable, ValueError):
        return 0


def open_row(step, message_id):
    step.click("iphone-inbox.list.row.%d" % row_of(step, message_id))


def drive(j):
    if j.ctl is None and why_not_driven():
        j.notes.append("could-not-drive: " + why_not_driven())
    arrival_file = write_arrival(ARRIVAL["id"], CODE_MAIL)

    # ---- J7.2 the thread of a row
    s = j.step("J7.2", "Open the thread, read it")
    s.expect("mail-unread-before-open", lambda: s.ctl.core("message/%s/read" % R1["id"]), False)
    s.arrive(("click", "iphone-inbox.list.row.0"), "iphone-thread")
    s.expect("open-thread-is-the-tapped-one", showing(s, one(s, "iphone-thread.open", "thread"), THREAD["id"], R1["id"]), THREAD["id"])
    s.expect("opened-message", one(s, "iphone-thread.open", "message"), R1["id"])
    s.expect("subject", one(s, "iphone-thread.subject", "subject"), THREAD["subject"])
    s.expect("participants", one(s, "iphone-thread.participants", "names"), THREAD["participants"])
    s.expect("account-name-shown", one(s, "iphone-thread.account", "name"), ACCOUNT[THREAD["account"]]["name"])
    s.expect("older-messages-folded", lambda: [m["folded"] for m in s.ctl.rows("iphone-thread.messages")], [m["folded"] for m in THREAD["messages"]])
    s.expect("senders-in-order", lambda: [m["from"] for m in s.ctl.rows("iphone-thread.messages")], [m["from"] for m in THREAD["messages"]])
    s.expect("attachment-listed", lambda: [a["name"] for a in s.ctl.rows("iphone-thread.attachments")], ATTACHMENT_NAMES)
    s.expect("opening-marks-read", showing(s, one(s, "iphone-thread.open", "read"), True, R1["id"]), True)
    s.expect("opening-keeps-it-in-the-inbox", lambda: (s.ctl.rows("iphone-thread.open")[0]["in_inbox"], s.ctl.rows("iphone-thread.open")[0]["archived"]), (True, False))
    s.expect("no-pill-without-a-code", lambda: s.ctl.rows("iphone-thread.code-pill"), [])
    s.expect("body-shown-online", one(s, "iphone-thread.body", "state"), "shown")

    # ---- J7.3 back to the inbox
    b = j.step("J7.3", "Back to the inbox")
    b.arrive(("click", "iphone-thread.back"), "iphone-inbox")
    b.expect("mail-still-listed-after-back", lambda: R1["id"] in [r["id"] for r in b.ctl.rows(LIST)], True)

    # ---- J7.2 the code mail: the pill
    p = j.step("threadphone-pill", "The code pill copies, marks read, never archives")
    p.raw("fixture", "deliver", ARRIVAL["id"], arrival_file, "eventsource")
    open_row(p, ARRIVAL["id"])
    p.expect("opens-from-the-list", one(p, "iphone-thread.open", "message"), ARRIVAL["id"])
    p.expect("code-pill-shows-the-code", showing(p, one(p, "iphone-thread.code-pill", "code"), ARRIVAL["code"], ARRIVAL["id"]), ARRIVAL["code"])
    p.click("iphone-thread.more")
    p.click("iphone-thread.more.unread")
    p.expect("more-panel-closes-on-pick", one(p, "iphone-thread.more", "open"), False)
    p.expect("unread-before-the-tap", showing(p, one(p, "iphone-thread.code-pill", "read"), False, ARRIVAL["id"]), False)
    p.expect("not-copied-before-the-tap", showing(p, lambda: (p.ctl.rows("iphone-thread.code-pill")[0]["copied"], p.ctl.rows("iphone-thread.code-pill")[0]["text"]), (False, "Copy"), ARRIVAL["id"]), (False, "Copy"))
    p.click("iphone-thread.code-pill")
    p.expect("copied-label", showing(p, one(p, "iphone-thread.code-pill", "text"), COPIED, ARRIVAL["id"]), COPIED)
    p.expect("clipboard-is-the-code", showing(p, one(p, "iphone-thread.code-pill", "clipboard"), ARRIVAL["code"], ARRIVAL["id"]), ARRIVAL["code"])
    p.expect("code-mail-marked-read", showing(p, lambda: p.ctl.core("message/%s/read" % ARRIVAL["id"]), True, ARRIVAL["id"]), True)
    p.expect("copying-never-archives", lambda: p.ctl.core("message/%s/archived" % ARRIVAL["id"]), False)
    p.expect("copied-mail-stays-in-the-inbox", showing(p, lambda: p.ctl.rows("iphone-thread.code-pill")[0]["in_inbox"], True, ARRIVAL["id"]), True)

    # ---- J7.2 offline: the notification text shows, the body says it is not downloaded
    o = j.step("threadphone-offline", "Offline: a body not on the phone says so")
    o.raw("fixture", "offline", "on")
    o.click("iphone-thread.reload")
    o.expect("offline-body-says-so", one(o, "iphone-thread.body", "state"), "not-downloaded")
    o.expect("offline-sentence-in-words", lambda: norm(o.ctl.rows("iphone-thread.body")[0]["text"]), NOT_DOWNLOADED)
    o.raw("fixture", "offline", "off")
    o.click("iphone-thread.reload")
    o.expect("online-body-has-text", one(o, "iphone-thread.body", "state"), "shown")

    # ---- J8.4 the thread's private note under "..."
    n = j.step("J8.4", "The thread's note, under More")
    n.click("iphone-thread.more")
    n.expect("more-opens-the-panel", one(n, "iphone-thread.more", "open"), True)
    n.click("iphone-thread.more.note")
    n.expect("note-opens-from-more", one(n, "iphone-thread.note", "open"), True)
    n.type(NOTE)
    n.expect("typed-text-is-in-the-note", one(n, "iphone-thread.note", "draft"), NOTE)
    n.click("iphone-thread.note.save")
    n.expect("note-sheet-closes-on-save", one(n, "iphone-thread.note", "open"), False)
    n.expect("note-saved-text", one(n, "iphone-thread.note", "text"), NOTE)
    n.click("iphone-thread.more")
    n.click("iphone-thread.more.note")
    n.expect("note-read-back", one(n, "iphone-thread.note", "draft"), NOTE)
    n.click("iphone-thread.note.close")
    n.expect("note-close-closes-the-sheet", one(n, "iphone-thread.note", "open"), False)
    n.click("iphone-thread.back")

    # ---- every control once, with its visible outcome
    c = j.step("threadphone-controls", "Every control of the thread, tapped once")
    open_row(c, R1["id"])
    c.click("iphone-thread.messages.0")
    c.expect("unfold-earlier-message", lambda: c.ctl.rows("iphone-thread.messages")[0]["folded"], False)
    pinned0 = {}                    # Pin in More TOGGLES: the design file's msg-01 is pinned already (docs/design/fixtures/mac-main-sample.json), so the case reads the state before the tap
    c._act(lambda: pinned0.update(v=c.ctl.rows("iphone-thread.open")[0]["pinned"]))
    c.click("iphone-thread.more")
    c.click("iphone-thread.more.pin")
    c.expect("pin-from-more", showing(c, lambda: one(c, "iphone-thread.open", "pinned")() != pinned0.get("v", False), True, R1["id"]), True)
    c.click("iphone-thread.more")
    c.click("iphone-thread.more.unread")
    c.expect("mark-unread-from-more", one(c, "iphone-thread.open", "unread"), True)
    c.click("iphone-thread.reload")
    c.expect("reload-keeps-the-body", showing(c, lambda: (c.ctl.rows("iphone-thread.body")[0]["state"], len(c.ctl.rows("iphone-thread.messages"))), ("shown", len(THREAD["messages"])), R1["id"]), ("shown", len(THREAD["messages"])))
    c.click("iphone-thread.more")
    c.click("iphone-thread.more.snooze")
    c.expect("snooze-leaves-the-thread", lambda: c.ctl.uistate().get("surface"), "iphone-inbox")
    c.expect("snooze-snoozes-the-mail", showing(c, lambda: c.ctl.core("message/%s/snoozed" % R1["id"]), True, R1["id"]), True)
    open_row(c, ARRIVAL["id"])
    c.click("iphone-thread.archive")
    c.expect("archive-leaves-the-thread", lambda: c.ctl.uistate().get("surface"), "iphone-inbox")
    c.expect("archive-archives-the-mail", lambda: c.ctl.core("message/%s/archived" % ARRIVAL["id"]), True)
    c.expect("archived-mail-leaves-the-list", lambda: ARRIVAL["id"] in [r["id"] for r in c.ctl.rows(LIST)], False)
    open_row(c, R2["id"])
    c.click("iphone-thread.reply")
    c.expect("reply-opens-the-composer", lambda: c.ctl.uistate().get("overlay"), "iphone-composer")


# MUTANTS: per check, the mutation of the app that must turn it red.
# J7.2/mail-unread-before-open | the mail is already read before the thread opens (control for opening-marks-read)
# J7.2/arrives-by-control | a tap on the row opens another surface
# J7.2/open-thread-is-the-tapped-one | the thread shows another mail's conversation
# J7.2/opened-message | the thread is open on another message
# J7.2/subject | the thread shows another subject
# J7.2/participants | the participants are missing or reordered
# J7.2/account-name-shown | the thread does not name its account
# J7.2/older-messages-folded | every message is expanded or every one folded
# J7.2/senders-in-order | messages are listed newest first
# J7.2/attachment-listed | the attachment is not listed
# J7.2/opening-marks-read | opening leaves the mail unread
# J7.2/opening-keeps-it-in-the-inbox | opening archives the mail
# J7.2/no-pill-without-a-code | the pill shows on a mail that has no code
# J7.2/body-shown-online | the body stays "not downloaded" while online
# J7.3/arrives-by-control | Back leaves the thread up
# J7.3/mail-still-listed-after-back | Back takes the mail out of the inbox
# threadphone-pill/opens-from-the-list | the row of the new mail opens another mail
# threadphone-pill/code-pill-shows-the-code | the pill shows no code or a wrong one
# threadphone-pill/more-panel-closes-on-pick | the panel stays up after a pick
# threadphone-pill/unread-before-the-tap | Mark unread does not mark the mail unread (control for the copy case)
# threadphone-pill/not-copied-before-the-tap | the pill says Copied before the tap (control for copied-label)
# threadphone-pill/copied-label | no Copied confirmation
# threadphone-pill/clipboard-is-the-code | the clipboard gets another string
# threadphone-pill/code-mail-marked-read | the copy leaves the mail unread
# threadphone-pill/copying-never-archives | copying archives the message
# threadphone-pill/copied-mail-stays-in-the-inbox | copying takes the message out of the inbox
# threadphone-offline/offline-body-says-so | offline shows the old body or nothing
# threadphone-offline/offline-sentence-in-words | the offline sentence is blank or another text
# threadphone-offline/online-body-has-text | the body stays "not downloaded" once online (control for the two cases above)
# J8.4/more-opens-the-panel | the More control opens nothing
# J8.4/note-opens-from-more | the Note item opens nothing
# J8.4/typed-text-is-in-the-note | typing does not reach the note
# J8.4/note-sheet-closes-on-save | Save leaves the sheet up
# J8.4/note-saved-text | Save does not keep the text
# J8.4/note-read-back | the saved note does not come back in the sheet
# J8.4/note-close-closes-the-sheet | the close control does nothing
# threadphone-controls/unfold-earlier-message | a tap on an earlier message does nothing
# threadphone-controls/pin-from-more | the Pin item does nothing
# threadphone-controls/mark-unread-from-more | the Mark unread item does nothing
# threadphone-controls/reload-keeps-the-body | reload blanks the body or the messages
# threadphone-controls/snooze-leaves-the-thread | Snooze leaves the thread up
# threadphone-controls/snooze-snoozes-the-mail | Snooze leaves the mail where it was
# threadphone-controls/archive-leaves-the-thread | Archive leaves the thread up
# threadphone-controls/archive-archives-the-mail | Archive does nothing to the mail
# threadphone-controls/archived-mail-leaves-the-list | the archived mail stays in the list
# threadphone-controls/reply-opens-the-composer | Reply opens nothing


# ---------------------------------------------------------------- the selftest: a stand-in socket written from the journey and the fixture
# The model follows the journey's words (docs/JOURNEYS.md J7.2, J7.3, J8.4), never the Swift screen. Each declared mutant is one flaw in it; the
# check that names it must go red. A second table deletes one ACT (the `skip` hook drops it) and demands the check that reads its effect goes red,
# because a check already true before its act is decoration.
class ModelCtl:
    instant = True

    def __init__(self, mut=(), skip=None):
        self.mut, self.skip = set(mut), skip
        self.surface, self.overlay, self.cur = "iphone-inbox", None, None
        self.order = [r["id"] for r in ROWS]
        self.read = {r["id"] for r in ROWS if not r["unread"]}
        self.archived, self.snoozed, self.pinned, self.expanded = set(), set(), set(), set()
        self.copied, self.clip, self.more, self.note_open = False, None, False, False
        self.note, self.draft, self.offline, self.was_offline, self.blank = "", "", False, False, False
        if self.m("J7.2/mail-unread-before-open"):
            self.read.add(R1["id"])

    def m(self, label):
        return label in self.mut

    def acted(self, kind, arg):
        return self.skip is not None and self.skip(kind, arg, self)

    # what the inbox lists: neither archived nor snoozed mail
    def listed(self):
        keep = self.m("threadphone-controls/archived-mail-leaves-the-list")
        return [i for i in self.order if (i not in self.archived or (keep and i == ARRIVAL["id"])) and i not in self.snoozed]

    # the open conversation, from the fixture: msg-01 has the fixture thread, the code mail one message, the others one message
    def thread(self):
        if self.cur == R1["id"]:
            msgs = [{"id": x["id"], "from": x["from"], "folded": x["folded"], "attachments": x.get("attachments", [])} for x in THREAD["messages"]]
            return {"id": THREAD["id"], "subject": THREAD["subject"], "participants": list(THREAD["participants"]), "account": THREAD["account"], "messages": msgs, "code": None}
        if self.cur == ARRIVAL["id"]:
            return {"id": ARRIVAL["thread"], "subject": ARRIVAL["subject"], "participants": ["%s <%s>" % (ARRIVAL["sender"], CODE_MAIL["from"])],
                    "account": ARRIVAL["account"], "messages": [{"id": self.cur, "from": ARRIVAL["sender"], "folded": False, "attachments": []}], "code": ARRIVAL["code"]}
        r = MSG[self.cur]
        return {"id": "thr-" + self.cur, "subject": r["subject"], "participants": [r["sender"]], "account": r["account"],
                "messages": [{"id": self.cur, "from": r["sender"], "folded": False, "attachments": []}], "code": None}

    def open_it(self, message_id):
        self.cur, self.overlay = message_id, "iphone-thread"
        self.surface = "iphone-search" if self.m("J7.2/arrives-by-control") else "iphone-thread"
        if self.m("threadphone-pill/opens-from-the-list") and message_id == ARRIVAL["id"]:
            self.cur = R1["id"]
        if not self.m("J7.2/opening-marks-read"):
            self.read.add(self.cur)
        if self.m("J7.2/opening-keeps-it-in-the-inbox") and self.cur == R1["id"]:
            self.archived.add(self.cur)

    def leave(self, verb):
        i = self.cur
        if verb == "snooze" and not self.m("threadphone-controls/snooze-snoozes-the-mail"):
            self.snoozed.add(i)
        if verb == "archive" and not self.m("threadphone-controls/archive-archives-the-mail"):
            self.archived.add(i)
        stay = self.m("threadphone-controls/%s-leaves-the-thread" % verb)
        if not stay:
            self.cur = None
        self.surface = "iphone-thread" if stay else "iphone-inbox"
        self.overlay = "iphone-thread" if stay else None
        self.more = False

    def send(self, verb, *args):
        if verb == "type":
            if self.note_open and not self.m("J8.4/typed-text-is-in-the-note"):
                self.draft += args[0]
            return {"ok": True}
        if verb == "fixture" and args[0] == "deliver":
            self.order.insert(0, ARRIVAL["id"])
            return {"ok": True}
        if verb == "fixture" and args[0] == "offline":
            self.offline = args[1] == "on"
            self.was_offline = self.was_offline or self.offline
            return {"ok": True}
        raise Unreadable("send %s: not modelled" % verb)

    def key(self, chord):
        raise Unreadable("key %s: not modelled" % chord)

    def type(self, text):
        return self.send("type", text)

    def click(self, ax):
        if self.acted("click", ax):
            return {"ok": True}
        if ax.startswith("iphone-inbox.list.row."):
            ids = self.listed()
            self.open_it(ids[int(ax.rsplit(".", 1)[1])])
            return {"ok": True}
        if self.cur is None:
            raise Unreadable("click %s: no thread is open" % ax)
        t = self.thread()
        if ax == "iphone-thread.back":
            self.surface, self.overlay, self.cur = ("iphone-thread" if self.m("J7.3/arrives-by-control") else "iphone-inbox"), None, None
            if self.m("J7.3/mail-still-listed-after-back"):
                self.archived.add(R1["id"])
            return {"ok": True}
        if ax == "iphone-thread.code-pill":
            if not t["code"]:
                raise Unreadable("click %s: did nothing" % ax)
            self.copied = not self.m("threadphone-pill/copied-label")
            self.clip = "000000" if self.m("threadphone-pill/clipboard-is-the-code") else t["code"]
            if not self.m("threadphone-pill/code-mail-marked-read"):
                self.read.add(self.cur)
            if self.m("threadphone-pill/copying-never-archives"):
                self.archived.add(self.cur)
            if self.m("threadphone-pill/copied-mail-stays-in-the-inbox"):
                self.order.remove(self.cur)
            return {"ok": True}
        if ax == "iphone-thread.more":
            self.more = not self.more and not self.m("J8.4/more-opens-the-panel")
            return {"ok": True}
        if ax.startswith("iphone-thread.more."):
            if not self.more:
                raise Unreadable("click %s: the panel is closed" % ax)
            self.more = self.m("threadphone-pill/more-panel-closes-on-pick")
            item = ax.rsplit(".", 1)[1]
            if item == "note":
                self.note_open = not self.m("J8.4/note-opens-from-more")
                self.draft = self.note if not self.m("J8.4/note-read-back") else ""
            elif item == "unread":
                if not (self.m("threadphone-controls/mark-unread-from-more") or self.m("threadphone-pill/unread-before-the-tap")):
                    self.read.discard(self.cur)
            elif item == "pin":
                if not self.m("threadphone-controls/pin-from-more"):
                    self.pinned.add(self.cur)
            elif item == "snooze":
                self.leave("snooze")
            return {"ok": True}
        if ax == "iphone-thread.note.save":
            if not self.note_open:
                raise Unreadable("click %s: no sheet" % ax)
            if not self.m("J8.4/note-saved-text"):
                self.note = self.draft
            self.note_open = self.m("J8.4/note-sheet-closes-on-save")
            return {"ok": True}
        if ax == "iphone-thread.note.close":
            if not self.note_open:
                raise Unreadable("click %s: no sheet" % ax)
            self.note_open = self.m("J8.4/note-close-closes-the-sheet")
            return {"ok": True}
        if ax == "iphone-thread.reload":
            self.blank = self.m("threadphone-controls/reload-keeps-the-body")
            return {"ok": True}
        if ax == "iphone-thread.archive":
            self.leave("archive")
            return {"ok": True}
        if ax == "iphone-thread.reply":
            self.overlay = None if self.m("threadphone-controls/reply-opens-the-composer") else "iphone-composer"
            return {"ok": True}
        if ax.startswith("iphone-thread.messages."):
            i = int(ax.rsplit(".", 1)[1])
            if i >= len(t["messages"]) - 1:
                raise Unreadable("click %s: not an earlier message" % ax)
            if not self.m("threadphone-controls/unfold-earlier-message"):
                self.expanded.add(i)
            return {"ok": True}
        raise Unreadable("click %s: nothing to click" % ax)

    def _body(self):
        if self.offline:
            if self.m("threadphone-offline/offline-body-says-so"):
                return {"state": "shown", "text": "the old body"}
            return {"state": "not-downloaded", "text": "" if self.m("threadphone-offline/offline-sentence-in-words") else "Not downloaded yet"}
        if self.blank or self.m("J7.2/body-shown-online") or (self.m("threadphone-offline/online-body-has-text") and self.was_offline):
            return {"state": "not-downloaded", "text": "Not downloaded yet"}
        return {"state": "shown", "text": "Hi Assem"}

    def rows(self, ax):
        if ax == LIST:
            return [{"ax": "iphone-inbox.list.row.%d" % i, "id": x, "unread": x not in self.read} for i, x in enumerate(self.listed())]
        if self.cur is None:
            return []
        t = self.thread()
        r = {"ax": ax}
        if ax == "iphone-thread.open":
            r.update(thread=t["id"], message=self.cur, read=self.cur in self.read, archived=self.cur in self.archived, in_inbox=self.cur in self.listed(),
                     pinned=self.cur in self.pinned, unread=self.cur not in self.read)
            if self.m("J7.2/open-thread-is-the-tapped-one"):
                r["thread"] = "thr-other"
            if self.m("J7.2/opened-message") and self.cur == R1["id"]:
                r["message"] = R2["id"]
        elif ax == "iphone-thread.subject":
            r.update(subject="Another subject" if self.m("J7.2/subject") else t["subject"])
        elif ax == "iphone-thread.participants":
            names = list(t["participants"])
            if self.m("J7.2/participants"):
                names = names[::-1]
            r.update(names=names)
        elif ax == "iphone-thread.account":
            r.update(name="" if self.m("J7.2/account-name-shown") else ACCOUNT[t["account"]]["name"])
        elif ax == "iphone-thread.messages":
            msgs = t["messages"][::-1] if self.m("J7.2/senders-in-order") else t["messages"]
            out = []
            for i, x in enumerate(msgs):
                folded = x["folded"]
                if self.m("J7.2/older-messages-folded"):
                    folded = not folded
                if i in self.expanded:
                    folded = False
                out.append({"ax": "iphone-thread.messages.%d" % i, "from": x["from"], "folded": folded})
            return out
        elif ax == "iphone-thread.attachments":
            names = [] if self.m("J7.2/attachment-listed") else ATTACHMENT_NAMES if self.cur == R1["id"] else []
            return [{"ax": "iphone-thread.attachments.%d" % i, "name": n} for i, n in enumerate(names)]
        elif ax == "iphone-thread.code-pill":
            show = bool(t["code"]) or self.m("J7.2/no-pill-without-a-code")
            if not show:
                return []
            copied = self.copied or self.m("threadphone-pill/not-copied-before-the-tap")
            r.update(code=("" if self.m("threadphone-pill/code-pill-shows-the-code") else t["code"] or "000000"), copied=copied,
                     text=COPIED if copied else "Copy", clipboard=self.clip, read=self.cur in self.read, archived=self.cur in self.archived,
                     in_inbox=self.cur in self.order)
        elif ax == "iphone-thread.body":
            r.update(self._body())
        elif ax == "iphone-thread.more":
            r.update(open=self.more)
        elif ax == "iphone-thread.note":
            r.update(open=self.note_open, text=self.note, draft=self.draft)
        else:
            raise Unreadable("rows %s: not modelled" % ax)
        return [r]

    def core(self, path):
        _, mid, what = path.split("/")
        if what == "read":
            return mid in self.read
        if what == "archived":
            return mid in self.archived
        if what == "snoozed":
            return mid in self.snoozed
        raise Unreadable("core %s: not modelled" % path)

    def uistate(self):
        return {"surface": self.surface, "overlay": self.overlay}

    def close(self):
        pass


def model_run(mutants=(), skip=None):
    """The journey on the stand-in: (labels failed, labels not judged, labels run)."""
    j = Journey(ID, ModelCtl(mutants, skip))
    drive(j)
    cs = j.checks()
    return {c.label for c in cs if c.state == "fail"}, {c.label for c in cs if c.state == "unjudged"}, {c.label for c in cs}


# Deleting one act must turn the check that reads its effect red: label -> (what is deleted, the drop rule)
ACT_DELETED = {
    "threadphone-pill/copied-label": ("the tap on the pill", lambda k, a, m: k == "click" and a == "iphone-thread.code-pill"),
    "threadphone-pill/clipboard-is-the-code": ("the tap on the pill", lambda k, a, m: k == "click" and a == "iphone-thread.code-pill"),
    "threadphone-pill/code-mail-marked-read": ("the tap on the pill", lambda k, a, m: k == "click" and a == "iphone-thread.code-pill"),
    "threadphone-controls/archive-archives-the-mail": ("the tap on Archive", lambda k, a, m: k == "click" and a == "iphone-thread.archive"),
    "threadphone-controls/pin-from-more": ("the tap on Pin", lambda k, a, m: k == "click" and a == "iphone-thread.more.pin"),
    "threadphone-controls/unfold-earlier-message": ("the tap on the earlier message", lambda k, a, m: k == "click" and a == "iphone-thread.messages.0"),
    "J8.4/note-saved-text": ("the tap on Save", lambda k, a, m: k == "click" and a == "iphone-thread.note.save"),
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
    unrun = sorted(set(labels) - base_run)
    print("clean model: failed=%s unjudged=%s; labels without a mutant=%s; declared mutants whose check does not run=%s" % (sorted(base_f), sorted(base_u), stray, unrun))
    print("| check | mutant (declared) | seen red |")
    print("|---|---|---|")
    for label, red in rows:
        print("| %s | the model carries this mutant | %s |" % (label, "RED on this case" if red else "NOT RED"))
    print("| check | act deleted | seen red |")
    print("|---|---|---|")
    for label, what, red in acts:
        print("| %s | %s | %s |" % (label, what, "RED on this case" if red else "NOT RED"))
    bad = len(base_f) + len(base_u) + len(missed) + len(act_missed) + len(stray) + len(unrun)
    print("%d mutants, %d not seen red; %d acts deleted, %d not seen red; %d clean failures, %d clean not judged" % (
        len(rows), len(missed), len(acts), len(act_missed), len(base_f), len(base_u)))
    return 0 if bad == 0 else 1


def main():
    path = os.path.abspath(__file__)
    if "--selftest" in sys.argv:
        return selftest(path)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    rows = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if re.match(r"# (J\d+\.\d+|threadphone-[a-z]+)/", l) and " | " in l]
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
    if r["unjudged"] and r["notes"]:
        sys.stderr.write("%s\n" % r["notes"][0])
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    ok = r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"]
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
