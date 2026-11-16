#!/usr/bin/env python3
"""Composer (iPhone): from-address, dictation by microphone, undo send (docs/JOURNEYS.md J7.4; C-J7.4).

Drives the RUNNING iPhone app in the simulator over the control socket ($WHOOSH_SOCKET_DIR, docs/CONTROL-SOCKET.md)
through journeys/lib. Expected values come from docs/design/fixtures/mac-main-sample.json, fixtures/tests-j7.json
and docs/CONSTANTS.md, never from a built screen. iPhone ax ids follow `<surface>.<region>[.<element>...][.<index>]`
(Sources/Features/ComposerPhone/ComposerPhoneIds.swift). Every control of the screen is clicked once and its visible outcome read.
Run: sh scripts/launch-one.sh <iphone-bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/composerphone-check.py'
Self-test (no app, no simulator): python3 checks/composerphone-check.py --selftest   (each case must be red on its declared mutant)
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0 (a check that could not run is not green).
Needs one line in the shell's FixtureEvents.served (frozen, docs/lanes/composerphone.md "Wiring"): "speech-model": 1...1,
"""
import json
import os
import re
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, Journey, declared_mutants              # noqa: E402
from lib.ctl import Unreadable                                              # noqa: E402
from lib.fixture import FX, ROWS, ACCOUNT, MSG, ACCOUNT_COUNT, const        # noqa: E402
from lib.phonestart import leave_welcome                                    # noqa: E402
from lib import phonedictate                                                # noqa: E402  the dictation sequence J7.4 drives too

ID = "composerphone"
TITLE = "Composer (iPhone): from-address, dictation, undo send"
TARGET = "LOCAL"
VISIBLE_OUTCOME = ("Reply opens from the thread's account and can change it; the microphone with no speech model shows the download, then "
                   "listens and the words arrive; Send shows the undo bar, Undo returns to the draft, the next send says Sent")

with open(os.path.join(ROOT, "fixtures", "tests-j7.json"), encoding="utf-8") as _f:
    T = json.load(_f)
SPOKEN = T["dictation"]["spoken"]
R1 = MSG["msg-01"]
THREAD = FX["open_thread"]
THREAD_ACCOUNT = ACCOUNT[THREAD["account"]]
ACCOUNTS = list(ACCOUNT.values())
PICK = 1                                    # the From list index the check changes to: the second account
WINDOW = const("UNDO_SEND_DEFAULT_S")
C = "iphone-composer"
FROM, BODY, HEAD, DICT, MODEL, UNDO = C + ".from", C + ".body", C + ".header", C + ".dictate", C + ".speech-model", C + ".undo"


def norm(text):
    return re.sub(r"[^a-z0-9 ]", "", text.lower()).strip()


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


def settle(s, read, want, secs=6.0):
    """Reads until the value is `want` or the time is up: the model download and the send window run on their own clock."""
    end, reads = time.monotonic() + secs, 0
    got = read()
    while got != want and time.monotonic() < end and reads < 60:
        if not getattr(s.j.ctl, "instant", False):
            time.sleep(0.15)
        got, reads = read(), reads + 1
    return got


def percents_seen(s, secs=8.0):
    """The percentages the download shows between the tap and its end: it must show at least one between 0 and 100."""
    seen, end = [], time.monotonic() + secs
    while True:
        r = s.ctl.rows(MODEL)[0]
        seen.append((r["state"], r["percent"]))
        if r["state"] != "downloading" or time.monotonic() > end or len(seen) > 60:
            return any(st == "downloading" and 0 < p < 100 for st, p in seen)
        if not getattr(s.j.ctl, "instant", False):
            time.sleep(0.1)


def row(s, ax):
    return s.ctl.rows(ax)[0]


def drive(j):
    if j.ctl is None and why_not_driven():
        j.notes.append("could-not-drive: " + why_not_driven())
    s = j.step("J7.4", "Reply, change From, dictate, send")
    # A test copy starts its first launch (the welcome), and the composer sheet hangs off the inbox stack the welcome covers (phone round 4:
    # 25 cases red, every row of iphone-composer empty): leave it by the flow's own controls first, as inboxphone-check does.
    s._act(lambda: leave_welcome(s.j.ctl))
    # arrive by the Reply control of the open thread (the thread lane's id; the inbox's Compose is read at the end)
    s.arrive(("click", "iphone-inbox.list.row.0"), "iphone-thread")
    s.arrive(("click", "iphone-thread.reply"), C)
    s.expect("reply-is-addressed-to-the-sender", lambda: (row(s, HEAD)["kind"], row(s, HEAD)["to"]), ("reply", [R1["from"]]))
    s.expect("from-address", lambda: row(s, FROM)["text"], THREAD_ACCOUNT["address"])
    s.expect("dictate-control-present", lambda: (len(s.ctl.rows(DICT)), row(s, DICT)["state"]), (1, "idle"))
    # the From address: the list, a pick, the signature that follows it
    s.click(FROM)
    s.expect("from-list-has-every-account", lambda: len(s.ctl.rows(FROM + ".choices")), ACCOUNT_COUNT)
    s.click("%s.%d" % (FROM, PICK))
    s.expect("from-change", lambda: row(s, FROM)["text"], ACCOUNTS[PICK]["address"])
    s.expect("from-list-closes-on-pick", lambda: row(s, HEAD)["from_open"], False)
    s.expect("signature-follows-from", lambda: row(s, BODY)["signature"], ACCOUNTS[PICK]["signature"])
    # a reply goes out from the account that received the mail: change it back before the send
    back = [i for i, a in enumerate(ACCOUNTS) if a["id"] == THREAD_ACCOUNT["id"]][0]
    s.click("%s.%d" % (FROM, back))
    s.expect("from-restored", lambda: row(s, FROM)["text"], THREAD_ACCOUNT["address"])
    # the other controls, once each
    s.click(C + ".cc.show")
    s.expect("cc-bcc-toggle", lambda: (row(s, HEAD)["cc_visible"], row(s, HEAD)["bcc_visible"]), (True, True))
    s.click(C + ".quoted.toggle")
    s.expect("quoted-toggle", lambda: row(s, C + ".quoted")["open"], True)
    s.click(C + ".attach")
    s.expect("attach-opens-picker", lambda: row(s, HEAD)["attach_open"], True)
    s.click(C + ".attach")
    s.click(C + ".subject")
    s.type(" now")
    s.expect("field-takes-typing", lambda: (row(s, HEAD)["focused"], row(s, HEAD)["subject"].endswith(" now")), ("subject", True))
    # an empty reply is refused in words
    s.click(C + ".send")
    s.expect("empty-reply-is-refused", lambda: (len(s.ctl.rows(UNDO)), row(s, HEAD)["line"] != ""), (0, True))
    # the microphone with no speech model: progress, then listening, then the words
    # The dictation, by the ONE sequence journeys/J7.py J7.4 drives too (journeys/lib/phonedictate.py): start, the progress read here,
    # then finish (the model lands on its own, listening, the words, the second tap, the words arrive).
    s._act(lambda: phonedictate.start(s.j.ctl))
    s.expect("no-model-starts-download", lambda: (row(s, MODEL)["state"], row(s, DICT)["state"]), ("downloading", "downloading"))
    s.expect("download-shows-progress", lambda: percents_seen(s), True)
    dictation = {}
    s._act(lambda: dictation.update(phonedictate.finish(s.j.ctl, SPOKEN)))
    s.expect("model-ready-after-download", lambda: (s.ctl, dictation.get("model"))[1], "ready")       # s.ctl: not judged in a dry run
    s.expect("listening-after-download", lambda: (s.ctl, dictation.get("listening"))[1], "listening")
    s.expect("dictate-state-text", lambda: (s.ctl, dictation.get("after"))[1], "text")
    s.expect("dictated-words", lambda: (norm(row(s, BODY)["dictated"]), norm(SPOKEN) in norm(row(s, BODY)["text"])), (norm(SPOKEN), True))
    # send, the undo bar, Undo back to the draft
    s.click(C + ".send")
    s.expect("undo-send-bar", lambda: len(s.ctl.rows(UNDO)), 1)
    s.expect("undo-bar-says-sending", lambda: (row(s, UNDO)["text"], row(s, UNDO)["undo_offered"]), ("Sending", True))
    s.expect("undo-window-seconds", lambda: row(s, UNDO)["seconds"], WINDOW)
    s.expect("outbox-undoable", lambda: s.ctl.core("outbox_entry/0/state"), "undoable")
    s.click(C + ".undo.cancel")
    s.expect("cancel-stops-the-send", lambda: s.ctl.core("outbox_entry/0/state"), "cancelled")
    s.expect("undo-returns-to-the-draft", lambda: (len(s.ctl.rows(UNDO)), row(s, HEAD)["phase"], norm(SPOKEN) in norm(row(s, BODY)["text"])), (0, "editing", True))
    # send again and let the window run out: the bar says Sent, then the composer goes
    s.click(C + ".send")
    s.raw("clock", "advance", WINDOW + 1)
    s.expect("second-send-leaves-after-window", lambda: s.ctl.core("outbox_entry/1/state"), "sent")
    s.expect("sent-label", lambda: settle(s, lambda: s.ctl.rows(UNDO)[0]["text"] if s.ctl.rows(UNDO) else "", "Sent"), "Sent")
    s.expect("composer-closes-after-sent", lambda: settle(s, lambda: row(s, HEAD)["is_open"], False), False)
    # a new message from the inbox's Compose, and Cancel
    # The composer has gone back to where it came from, the THREAD (ComposerPhoneState.returnSurface); the inbox's Compose control is the
    # inbox's: back to the inbox first (phone round 5: arrives-by-control got 'iphone-thread', then new-message-kind had no row).
    s.click("iphone-thread.back")
    s.arrive(("click", "iphone-inbox.compose"), C)
    s.expect("new-message-kind", lambda: row(s, HEAD)["kind"], "new")
    s.click(C + ".cancel")
    s.expect("cancel-closes-the-composer", lambda: row(s, HEAD)["is_open"], False)


# MUTANTS: per check, the mutation of the app that must turn it red.
# J7.4/arrives-by-control | Reply opens nothing, or Compose opens nothing
# J7.4/reply-is-addressed-to-the-sender | the reply has no recipient or another one
# J7.4/from-address | the reply opens from another account
# J7.4/dictate-control-present | the microphone control is missing
# J7.4/from-list-has-every-account | the From list leaves an account out
# J7.4/from-change | picking an account in the list does not change From
# J7.4/from-list-closes-on-pick | the list stays open after a pick
# J7.4/signature-follows-from | the signature stays the old account's after From changes
# J7.4/from-restored | the From address cannot be put back
# J7.4/cc-bcc-toggle | the Cc and Bcc control shows nothing
# J7.4/quoted-toggle | the folded quote does not open
# J7.4/attach-opens-picker | the paperclip does nothing
# J7.4/field-takes-typing | typed text does not reach the field that has the cursor
# J7.4/empty-reply-is-refused | an empty reply goes to the undo bar, or says nothing
# J7.4/no-model-starts-download | with no speech model the tap does nothing
# J7.4/download-shows-progress | the download shows no percentage between 0 and 100
# J7.4/model-ready-after-download | the download never ends
# J7.4/listening-after-download | after the download the microphone does not listen
# J7.4/dictate-state-text | the microphone never says the words arrived
# J7.4/dictated-words | dictation inserts nothing or other words
# J7.4/undo-send-bar | no undo bar after Send
# J7.4/undo-bar-says-sending | the bar has no Sending word or offers no Undo
# J7.4/undo-window-seconds | the undo window is not the default
# J7.4/outbox-undoable | Send writes no outbox entry in its undo window
# J7.4/cancel-stops-the-send | Undo leaves the entry to be sent
# J7.4/undo-returns-to-the-draft | Undo loses the text or leaves the composer sending
# J7.4/second-send-leaves-after-window | the reply never leaves the outbox
# J7.4/sent-label | the bar never says Sent
# J7.4/composer-closes-after-sent | the composer stays open after Sent
# J7.4/new-message-kind | Compose opens a reply
# J7.4/cancel-closes-the-composer | Cancel does nothing


# ---------------------------------------------------------------- the selftest: a stand-in socket that behaves as the contract says
# Written from the fixture, docs/JOURNEYS.md J7.4 and the screen's contract, never from the app's reply. Its mutants are the declared ones above
# (one per label); a second table deletes the ACT of a case (the `skip` hook drops one act of the journey) and demands the check goes red,
# because a check already true before its act is decoration.
class ModelCtl:
    instant = True

    def __init__(self, mut=(), skip=None):
        self.mut, self.skip = set(mut), skip
        self.surface, self.open, self.kind, self.cc, self.quoted_open, self.attach = "iphone-inbox", False, "reply", False, False, False
        self.from_i = [i for i, a in enumerate(ACCOUNTS) if a["id"] == THREAD_ACCOUNT["id"]][0]
        self.from_open, self.focus, self.subject, self.body, self.line = False, "", "Re: " + THREAD["subject"], "", ""
        self.model, self.dl, self.mic, self.dictated, self.heard = "missing", 0, "idle", "", ""
        self.phase, self.entries, self.sent_seen, self.window = "editing", [], False, WINDOW

    # -- helpers
    def has(self, label):
        return label in self.mut

    def _acted(self, verb, arg):
        return bool(self.skip and self.skip(verb, arg, self))

    def _tick(self):
        """One poll of a clock-driven part: the download counts up in steps, then the microphone listens."""
        if self.model == "downloading":
            if self.has("J7.4/model-ready-after-download"):
                return
            self.dl += 20
            if self.dl >= 100:
                self.model = "ready"
                if not self.has("J7.4/listening-after-download"):
                    self.mic = "listening"

    def _sign(self):
        return ACCOUNTS[0]["signature"] if self.has("J7.4/signature-follows-from") else ACCOUNTS[self.from_i]["signature"]

    # -- the socket verbs
    def send(self, verb, *args):
        if self._acted(verb, args):
            return {"ok": True}
        if verb == "fixture" and args[0] == "speech-model":
            self.model, self.dl = args[1], 0
        elif verb == "fixture" and args[0] == "speak":
            self.heard = " ".join(args[1:])
        elif verb == "clock" and args[0] == "advance":
            if self.phase == "sending" and float(args[1]) >= self.window and not self.has("J7.4/second-send-leaves-after-window"):
                self.entries[-1] = "sent"
                self.phase = "sent"
                if self.has("J7.4/sent-label"):
                    self.phase = "closed-quiet"
        return {"ok": True}

    def key(self, chord):
        raise Unreadable("key %s: not modelled" % chord)

    def type(self, text):
        if self._acted("type", text):
            return {"ok": True}
        if self.focus == "subject" and not self.has("J7.4/field-takes-typing"):
            self.subject += text
        return {"ok": True}

    def click(self, ax):
        if self._acted("click", ax):
            return {"ok": True}
        if ax == "iphone-inbox.list.row.0":
            self.surface = "iphone-thread"
        elif ax == "iphone-thread.reply":
            if not self.has("J7.4/arrives-by-control"):
                self.surface, self.open, self.kind = C, True, "reply"
        elif ax == "iphone-inbox.compose":
            if not self.has("J7.4/arrives-by-control"):
                self.surface, self.open, self.kind = C, True, "reply" if self.has("J7.4/new-message-kind") else "new"
        elif ax == FROM:
            self.from_open = not self.from_open
        elif ax.startswith(FROM + "."):
            if not self.has("J7.4/from-change"):
                self.from_i = int(ax.rsplit(".", 1)[1])
            if not self.has("J7.4/from-list-closes-on-pick"):
                self.from_open = False
            if self.has("J7.4/from-restored") and int(ax.rsplit(".", 1)[1]) != PICK:
                self.from_i = PICK
        elif ax == C + ".cc.show":
            self.cc = not self.has("J7.4/cc-bcc-toggle")
        elif ax == C + ".quoted.toggle":
            self.quoted_open = not self.has("J7.4/quoted-toggle")
        elif ax == C + ".attach":
            self.attach = (not self.attach) and not self.has("J7.4/attach-opens-picker")
        elif ax == C + ".subject":
            self.focus = "subject"
        elif ax == C + ".send":
            self._send()
        elif ax == DICT:
            self._dictate()
        elif ax == C + ".undo.cancel":
            if self.phase == "sending":
                if not self.has("J7.4/cancel-stops-the-send"):
                    self.entries[-1] = "cancelled"
                self.phase = "sending" if self.has("J7.4/undo-returns-to-the-draft") else "editing"
                if self.has("J7.4/undo-returns-to-the-draft"):
                    self.body = ""
        elif ax == C + ".cancel":
            if not self.has("J7.4/cancel-closes-the-composer"):
                self.open = False
                self.surface = "iphone-inbox"
        else:
            raise Unreadable("click %s: nothing to click" % ax)
        return {"ok": True}

    def _send(self):
        if not self.body.strip():
            if self.has("J7.4/empty-reply-is-refused"):
                self.phase, self.entries = "sending", self.entries + ["undoable"]
            else:
                self.line = "Add a recipient and some text first."
            return
        if self.phase == "sending":
            return
        self.entries.append("undoable" if not self.has("J7.4/outbox-undoable") else "sent")
        self.phase = "sending"
        self.window = WINDOW + 3 if self.has("J7.4/undo-window-seconds") else WINDOW
        if self.has("J7.4/undo-send-bar"):
            self.phase = "sending-hidden"

    def _dictate(self):
        if self.mic == "listening":
            if self.has("J7.4/dictated-words"):
                self.heard = "other words"
            words = self.heard
            self.body = (self.body + " " + words).strip()
            self.dictated = words
            self.mic = "idle" if self.has("J7.4/dictate-state-text") else "text"
            return
        if self.model == "missing":
            if not self.has("J7.4/no-model-starts-download"):
                self.model, self.dl = "downloading", 0
        elif self.model == "ready":
            self.mic = "listening"

    # -- the reads
    def rows(self, ax):
        self._tick()
        if ax == HEAD:
            kind = self.kind
            to = [] if self.has("J7.4/reply-is-addressed-to-the-sender") else [R1["from"]]
            if self.sent_seen and not self.has("J7.4/composer-closes-after-sent"):
                self.open, self.surface, self.phase, self.sent_seen = False, "iphone-thread", "editing", False
            if not self.open:
                return [{"ax": HEAD, "is_open": False}]
            return [{"ax": HEAD, "is_open": True, "kind": kind, "to": to if kind == "reply" else [], "cc_visible": self.cc, "bcc_visible": self.cc,
                     "from_open": self.from_open, "attach_open": self.attach, "focused": self.focus, "subject": self.subject, "line": self.line,
                     "phase": "editing" if self.phase in ("editing", "closed-quiet") else "sending" if self.phase.startswith("sending") else self.phase}]
        if ax == FROM:
            i = 0 if self.has("J7.4/from-address") and self.from_i == [k for k, a in enumerate(ACCOUNTS) if a["id"] == THREAD_ACCOUNT["id"]][0] else self.from_i
            return [{"ax": FROM, "text": ACCOUNTS[i]["address"]}]
        if ax == FROM + ".choices":
            n = ACCOUNT_COUNT - 1 if self.has("J7.4/from-list-has-every-account") else ACCOUNT_COUNT
            return [{"ax": "%s.%d" % (FROM, i)} for i in range(n)]
        if ax == BODY:
            return [{"ax": BODY, "text": self.body, "signature": self._sign(), "dictated": self.dictated}]
        if ax == C + ".quoted":
            return [{"ax": ax, "open": self.quoted_open}]
        if ax == DICT:
            if self.has("J7.4/dictate-control-present"):
                return []
            state = "downloading" if self.model == "downloading" else self.mic
            return [{"ax": DICT, "state": state, "percent": self.dl if self.model == "downloading" else 0}]
        if ax == MODEL:
            pct = 0 if self.has("J7.4/download-shows-progress") else self.dl
            if self.has("J7.4/download-shows-progress") and self.model == "downloading":
                pct = 0
            return [{"ax": MODEL, "state": self.model, "progress": pct / 100.0, "percent": pct}]
        if ax == UNDO:
            if self.phase == "sending":
                return [{"ax": UNDO, "text": "Sending…" if self.has("J7.4/undo-bar-says-sending") else "Sending", "seconds": int(self.window),
                         "undo_offered": not self.has("J7.4/undo-bar-says-sending")}]
            if self.phase == "sent":
                self.sent_seen = True          # the beat: the bar's last word has been read, the composer goes after it
                return [{"ax": UNDO, "text": "Sent", "seconds": 0, "undo_offered": False}]
            return []
        raise Unreadable("rows %s: not modelled" % ax)

    def uistate(self):
        return {"surface": self.surface}

    def core(self, path):
        m = re.match(r"outbox_entry/(\d+)/state", path)
        if m and int(m.group(1)) < len(self.entries):
            return self.entries[int(m.group(1))]
        raise Unreadable("core %s: not modelled" % path)

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
    "J7.4/from-change": ("the click that picks the second account", lambda v, a, m: v == "click" and a == "%s.%d" % (FROM, PICK)),
    "J7.4/cc-bcc-toggle": ("the click on Cc and Bcc", lambda v, a, m: v == "click" and a == C + ".cc.show"),
    "J7.4/quoted-toggle": ("the click on the folded quote", lambda v, a, m: v == "click" and a == C + ".quoted.toggle"),
    "J7.4/attach-opens-picker": ("the first click on the paperclip", lambda v, a, m: v == "click" and a == C + ".attach" and not m.attach),
    "J7.4/field-takes-typing": ("the typing into Subject", lambda v, a, m: v == "type"),
    "J7.4/no-model-starts-download": ("the tap on the microphone with no model", lambda v, a, m: v == "click" and a == DICT and m.model == "missing"),
    "J7.4/dictated-words": ("the second tap that ends the dictation", lambda v, a, m: v == "click" and a == DICT and m.mic == "listening"),
    "J7.4/undo-send-bar": ("the click on Send with words in the draft", lambda v, a, m: v == "click" and a == C + ".send" and bool(m.body)),
    "J7.4/cancel-stops-the-send": ("the tap on Undo", lambda v, a, m: v == "click" and a == C + ".undo.cancel"),
    "J7.4/second-send-leaves-after-window": ("the clock advance past the window", lambda v, a, m: v == "clock"),
    "J7.4/cancel-closes-the-composer": ("the tap on Cancel", lambda v, a, m: v == "click" and a == C + ".cancel"),
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
    if r["unjudged"] and r["notes"]:
        sys.stderr.write("%s\n" % r["notes"][0])
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    ok = r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"]
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
