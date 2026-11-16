"""J7: The same arrival on the iPhone (JOURNEYS J7.1-J7.5). Runs on the iPhone simulator socket
($WHOOSH_SOCKET_DIR); step J7.5 reads the Mac rig copy through $WHOOSH_MAC_SOCKET_DIR.

Written from docs/JOURNEYS.md J7, docs/CAPABILITY-CONTRACT.md C-J7.1-C-J7.4, the design fixture and
fixtures/tests-j7.json, never from a built screen. The iPhone ax ids follow the grammar of
docs/CONTROL-SOCKET.md (`<surface>.<region>[.<element>...][.<index>]`); the iPhone feature lanes register
them, and a delta between these ids and theirs is a finding for the lane, not a reason to bend the journey.
`fixture` events are reserved verbs of the control socket (answer ok:false until the rig serves them).
"""
import contextlib
import io
import json
import os
import re
import subprocess
import sys
import time

from lib.ctl import Ctl, CouldNotDrive, Unreadable
from lib.fixture import FX, MSG, ACCOUNT, const, ROOT, write_arrival
from lib.phonestart import leave_welcome
from lib import phonedictate
from lib import sharedserver

ID = "J7"
TITLE = "The same arrival on the iPhone"
TARGET = "LOCAL"
PLATFORM = "iphone"   # lib/harness.py: on a socket whose app is the Mac app, every check is not judged (COULD-NOT-DRIVE, never RED)
VISIBLE_OUTCOME = "Notification, code copy, swipe triage and a dictated reply on the iPhone; the Mac shows the same result"

with open(os.path.join(ROOT, "fixtures", "tests-j7.json"), encoding="utf-8") as _f:
    T = json.load(_f)
ARR = T["arrival"]
SPOKEN = T["dictation"]["spoken"]


def wait_for(s, read, want, secs=6.0):
    """Reads until the value is `want` or the time is up (the model download and the dictation's end run on their own clock); returns the last
    value. No socket (a dry run) or an instant stand-in: one read."""
    end = time.monotonic() + secs
    got = read()
    if s.j.ctl is None or getattr(s.j.ctl, "instant", False):
        return got
    while got != want and time.monotonic() < end:
        time.sleep(0.15)
        got = read()
    return got
ARR_ACCOUNT = ACCOUNT[ARR["account"]]
REFUSED = [m for m in FX["messages"] if m["account"] == T["refused_account"]][0]
NO_PUSH = [a for a in FX["accounts"] if a["provider"] in T["no_push_providers"]]
LIST = "iphone-inbox.list"


def why_not_driven():
    """The one sentence that says why nothing could be driven here (printed to stderr by drive)."""
    d = os.environ.get("WHOOSH_SOCKET_DIR", "")
    if d and os.path.exists(os.path.join(d, "whoosh.sock")):
        return ""
    try:
        out = subprocess.run(["xcrun", "simctl", "list", "runtimes"], capture_output=True, text=True, timeout=20).stdout
    except (OSError, subprocess.SubprocessError):
        out = ""
    if "iOS" not in out:
        return ("the iPhone simulator cannot run: Xcode reports iOS 26.5 is not installed "
                "(owner step: Xcode > Settings > Components > iOS 26.5)")
    return "no iPhone app socket: launch the iPhone app in the simulator, then set WHOOSH_SOCKET_DIR"


def norm(text):
    return re.sub(r"[^a-z0-9 ]", "", text.lower()).strip()


def drive(j):
    if j.ctl is None:
        reason = why_not_driven()
        if reason:
            j.notes.append("could-not-drive: " + reason)
            sys.stderr.write("J7 could-not-drive: %s\n" % reason)
    ctx = {}
    mac = Ctl(os.environ.get("WHOOSH_MAC_SOCKET_DIR", ""))
    try:
        mac.connect()
    except CouldNotDrive:
        mac = None

    def on_mac():
        if mac is None:
            raise Unreadable("no Mac socket: set WHOOSH_MAC_SOCKET_DIR to the Mac rig copy")
        return mac

    if sharedserver.enabled():
        # scripts/iphone-rig.sh --with-mac --shared-server: both copies are on the REAL core against one loopback mail server, so the fixture steps J7.1 to J7.4
        # (the arrival, the dictation, the clock) have nothing to judge here; J7.5 is the point of this run.
        return drive_shared(j, mac)

    def where(i):
        try:
            return [r["id"] for r in j.ctl.rows(LIST)].index(i)
        except ValueError:
            raise Unreadable("%s is not in the inbox list" % i)
        except (KeyError, TypeError, AttributeError):
            raise Unreadable("the inbox list could not be read")

    # --- J7.1 the notification
    s = j.step("J7.1", "Lock-screen notification")
    # The second pass (dark) runs on the app the first pass (light) left: the composer open, the undo entries, the speech model. `fixture reset` puts the sample mail and the
    # view state back (it clears overlay and surface; the stack follows), the same first step as threadphone-check; the welcome is left through its own flow if it is up.
    s._act(lambda: leave_welcome(s.j.ctl))      # a test copy starts its first launch (the welcome): the inbox rows below need the inbox (first, so a reset cannot hide that it is up)
    s.raw("fixture", "reset")
    # `fixture deliver <arrival> <fixture-file>` reads the arrival from a file (FixtureVerbs.deliver): with the id alone the verb refused (usage), nothing was delivered, and every
    # step after it read another thread (main phone4: the first row msg-01, the reply opened from flow-rejected-8). The mail is the sign-in code of threadphone-check's CODE_MAIL.
    arrival_file = write_arrival(ARR["id"], dict(ARR, **{"from": "no-reply@signin-example.com", "first_lines": "Your sign-in code is %s. It expires in 10 minutes." % ARR["code"]}))
    s.raw("fixture", "deliver", ARR["id"], arrival_file, "eventsource")
    note = lambda: s.ctl.rows("iphone-lock.notifications")
    s.expect("notification-shown", lambda: len(note()), 1)
    s.expect("notification-fields", lambda: (note()[0]["sender"], note()[0]["subject"], note()[0]["account"]),
             (ARR["sender"], ARR["subject"], ARR_ACCOUNT["name"]))
    s.expect_le("arrival-s", lambda: s.ctl.measure("iphone-lock.notifications")["ms"] / 1000.0, const("PUSH_ARRIVAL_P95_S"))
    s.expect("actions-on-long-press", lambda: [a["title"] for a in s.ctl.rows("iphone-lock.notifications.actions")], ["Archive", "Reply"])
    s.expect("fetched-to-phone", lambda: s.ctl.core("message/%s/fetched" % ARR["id"]), True)
    def account_notes():
        """The rows of `iphone-inbox.accounts` (id, name, address, colour, group). The phone draws no push note on its accounts yet (the owner's words are pending),
        so the two note cases are NOT JUDGED with that reason, never red on a field the app does not serve."""
        rows = s.ctl.rows("iphone-inbox.accounts")
        if any("note" not in r for r in rows):
            raise Unreadable("the phone draws no push note on its accounts yet: the owner has not given its words, so `iphone-inbox.accounts` serves no `note` field")
        return rows
    s.expect("no-push-account-says-so", lambda: [("Mac" in r["note"] and "no push" in r["note"].lower())
             for r in account_notes() if r["id"] == NO_PUSH[0]["id"]], [True])
    s.expect("push-account-has-no-note", lambda: [r["note"] for r in account_notes()
             if r["id"] == ARR["account"]], [""])

    # --- J7.2 tap, copy the code
    s = j.step("J7.2", "Tap the notification, copy the code")
    # the notification's tap is the verb the phone has for it (PhoneRouter.notificationTapped): the lock screen itself is the system's, not a surface of the app
    s.raw("fixture", "notification-tap", ARR["id"])
    s.expect("arrives-by-control", lambda: s.ctl.uistate().get("surface"), "iphone-thread")
    s.expect("thread-is-the-arrival", lambda: s.ctl.rows("iphone-thread.subject")[0]["subject"], ARR["subject"])
    s.expect("code-pill-shows-code", lambda: s.ctl.rows("iphone-thread.code-pill")[0]["code"], ARR["code"])
    s.click("iphone-thread.code-pill")
    s.expect("copied-label", lambda: s.ctl.rows("iphone-thread.code-pill")[0]["text"], "Copied")
    s.expect("clipboard-is-the-code", lambda: s.ctl.rows("iphone-thread.code-pill")[0]["clipboard"], ARR["code"])   # the pill row serves what this screen wrote (ThreadClipboard.text); there is no core value `clipboard/text`
    s.expect("marked-read", lambda: s.ctl.core("message/%s/read" % ARR["id"]), True)
    s.expect("copy-never-archives", lambda: s.ctl.core("message/%s/archived" % ARR["id"]), False)
    s.raw("fixture", "offline", "on")
    s.click("iphone-thread.reload")
    s.expect("offline-body-says-so", lambda: s.ctl.rows("iphone-thread.body")[0]["text"], "Not downloaded yet")
    s.raw("fixture", "offline", "off")
    s.click("iphone-thread.reload")
    s.expect("online-body-has-text", lambda: s.ctl.rows("iphone-thread.body")[0]["text"] != "Not downloaded yet", True)

    # --- J7.3 swipe archive and snooze
    s = j.step("J7.3", "Swipe to archive and snooze")
    s.arrive(("click", "iphone-thread.back"), "iphone-inbox")
    ids = lambda: [r["id"] for r in s.ctl.rows(LIST)]
    before = []

    def bef(k):
        if not before:
            raise Unreadable("the rows before the swipes were not read")
        return before[0][k]["id"]

    def pick(n):
        """The n-th row (0 or 1) of an account that is not the refusing one; the list index in the CURRENT list."""
        ks = [k for k, r in enumerate(before[0]) if r.get("account") != T["refused_account"] and r.get("id") != ARR["id"]][:2]
        if len(ks) < 2:
            raise Unreadable("fewer than two swipeable rows in the list")
        return ks[0] if n == 0 else ks[1] - 1       # the second index shifts up by one once the first row has left

    def act(fn):
        def run():
            try:
                if before:
                    fn()
            except (KeyError, IndexError, TypeError):
                raise Unreadable("the list rows are not in the expected shape")
        s._act(run)

    s.expect("rows-before", lambda: before.append(s.ctl.rows(LIST)) or len(before[0]) > 3, True)
    act(lambda: s.ctl.click("iphone-inbox.list.row.%d.swipe-archive" % pick(0)))
    act(lambda: s.ctl.click("iphone-inbox.list.row.%d.swipe-snooze" % pick(1)))
    ARCH = lambda: (bef(0), before[0][pick(0)]["id"])[1]
    SNZ = lambda: (bef(0), before[0][pick(1) + 1]["id"])[1]
    s.expect("archived-row-leaves", lambda: ARCH() in ids(), False)
    s.expect("snoozed-row-leaves", lambda: SNZ() in ids(), False)
    s.expect("other-rows-stay", lambda: ids() == [r["id"] for r in before[0] if r["id"] not in (ARCH(), SNZ())], True)
    s.expect("archived-in-core", lambda: s.ctl.core("message/%s/archived" % ARCH()), True)
    s.expect("snoozed-in-core", lambda: s.ctl.core("message/%s/snoozed" % SNZ()), True)
    s.expect("undo-control-shows", lambda: len(s.ctl.rows("iphone-inbox.undo")), 1)
    s.expect("undo-entries-written", lambda: s.ctl.core("undo_entry/count"), 2)
    act(lambda: s.ctl.click("iphone-inbox.list.row.%d.swipe-archive" % (lambda: ids().index(REFUSED["id"]) if REFUSED["id"] in ids() else (_ for _ in ()).throw(Unreadable("refused row not listed")))()))
    s.expect("refused-row-comes-back", lambda: REFUSED["id"] in ids(), True)
    s.expect("refusal-names-the-account", lambda: ACCOUNT[REFUSED["account"]]["name"] in s.ctl.rows("iphone-inbox.failure")[0]["text"], True)

    # --- J7.4 dictated reply
    s = j.step("J7.4", "Dictated reply")
    s._act(lambda: s.ctl.click("iphone-inbox.list.row.%d" % where(ARR["id"])))
    s.expect("arrives-by-control", lambda: s.ctl.uistate()["surface"], "iphone-thread")
    s.click("iphone-thread.reply")
    s.expect("composer-opens", lambda: s.ctl.uistate()["surface"], "iphone-composer")
    s.expect("from-address", lambda: s.ctl.rows("iphone-composer.from")[0]["text"], ARR_ACCOUNT["address"])
    # The signature is drawn under the text, its own field of the body row (ComposerPhoneProbe.body: `signature`), never inside `text`
    # (phone round 5: the case read `text` and got False on both themes). The account's served signature ends with the mock's name line.
    s.expect("signature-present", lambda: FX["open_thread"]["quick_reply"]["signature"] in (s.ctl.rows("iphone-composer.body")[0].get("signature") or ""), True)
    # The dictation by the ONE sequence composerphone-check drives (journeys/lib/phonedictate.py). Phone round 6: this step set
    # `fixture speech-model ready` mid-download, which CANCELS the download (PhoneSpeech.rigSet), so the session never listened: '' twice.
    s._act(lambda: phonedictate.start(s.j.ctl))
    s.expect("model-downloads-with-progress", lambda: (s.ctl.rows("iphone-composer.speech-model")[0]["state"],
             0 <= s.ctl.rows("iphone-composer.speech-model")[0]["progress"] <= 1), ("downloading", True))
    s._act(lambda: phonedictate.finish(s.j.ctl, SPOKEN))
    s.expect("dictated-text", lambda: norm(s.ctl.rows("iphone-composer.body")[0]["dictated"]), norm(SPOKEN))
    s.click("iphone-composer.send")                  # the composer's Send control (phone round 5: `key send` is no key, nothing was sent)
    s.expect("undo-send-bar", lambda: len(s.ctl.rows("iphone-composer.undo")), 1)
    s.expect("outbox-undoable", lambda: s.ctl.core("outbox_entry/0/state"), "undoable")
    s.raw("clock", "advance", const("UNDO_SEND_DEFAULT_S") + 1)
    s.expect("outbox-sent-after-window", lambda: s.ctl.core("outbox_entry/0/state"), "sent")
    # "Sent" is the composer's own bar after the window (ComposerPhoneProbe.undo), read as composerphone-check reads it, before it closes
    s.expect("sent-label", lambda: wait_for(s, lambda: (s.ctl.rows("iphone-composer.undo") or [{}])[0].get("text", ""), "Sent"), "Sent")

    # --- J7.5 the Mac shows the same
    s = j.step("J7.5", "The Mac shows the same")
    ctx["archived"] = lambda: ARCH()
    mrows = lambda: on_mac().rows("mac-main.list")
    s.expect("mac-probe-works", lambda: len(mrows()) > 0, True)
    # The two copies are fixture cores here, each with a private mailbox: nothing the phone does can reach the Mac. The three cases are judged only on a run with the
    # shared mail server (drive_shared); without it they say so, they do not fail (phone round 8: expected False got True on both themes).
    def no_server():
        raise Unreadable(sharedserver.WHY)
    s.expect("archived-gone-on-mac", lambda: no_server(), False)
    s.expect("reply-in-thread-on-mac", lambda: no_server(), True)
    s.expect("reply-text-on-mac", lambda: no_server(), True)
    if mac:
        mac.close()


def drive_shared(j, mac):
    """J7.5 on the shared loopback mail server (journeys/lib/sharedserver.py): pass or fail, never not judged."""
    s = j.step("J7.5", "The Mac shows the same")
    box = {}

    def result():
        if "r" not in box and "why" not in box:
            if mac is None:
                raise Unreadable("no Mac socket: set WHOOSH_MAC_SOCKET_DIR to the Mac rig copy")
            try:
                # the harness leaves j.ctl None when the phone's surface word names no platform (the real engine starts on 'mac-main'): the phone socket is opened here
                phone = j.ctl or Ctl(os.environ.get("WHOOSH_IPHONE_SOCKET_DIR") or os.environ.get("WHOOSH_SOCKET_DIR", "")).connect()
                box["r"] = sharedserver.drive(mac, phone, SPOKEN)
            except sharedserver.Failed as e:
                box["why"] = str(e)
            except CouldNotDrive as e:
                raise Unreadable("the phone socket cannot be opened: %s" % e)
        return box

    def val(f):
        b = result()
        return b["why"] if "why" in b else f(b["r"])

    s.expect("mac-probe-works", lambda: val(lambda r: len(r["accounts"]["mac"]) > 0), True)
    s.expect("archived-gone-on-mac", lambda: val(lambda r: r["gone_on_mac"]), True)
    s.expect("reply-in-thread-on-mac", lambda: val(lambda r: r["reply_in_thread"] or r.get("reply_note", False)), True)
    s.expect("reply-text-on-mac", lambda: val(lambda r: bool(r["thread_texts"]) and norm(r["thread_texts"][-1]).startswith(norm(SPOKEN))), True)
    if mac:
        mac.close()


# --- --selftest: J7's own steps and checks against the stand-in phone and Mac (journeys/lib/fakephone.py), no app, no simulator ---------------------------------------
# One broken copy per declared mutant: label -> (the copy, what it breaks). The copy is ("fixture", mut) (the fixture-core run J7.1 to J7.5, fakephone.Phone.mut: see
# journeys/lib/fakephone.py) or ("shared", name) (the run on the shared mail server, J7.5 only: journeys/lib/sharedserver.py's stand-in world mutant `name`).
# A declared mutant with no row here, or a copy that leaves its own check green, makes the self-test exit 1.
LOCK, THREAD, COMPOSER, INBOX = "iphone-lock.notifications", "iphone-thread", "iphone-composer", "iphone-inbox"
BROKEN_COPIES = {
    "J7.1/notification-shown": (("fixture", ("read", ("rows", LOCK), "empty")), "the lock-screen rows are empty"),
    "J7.1/notification-fields": (("fixture", ("read", ("rows", LOCK), "change")), "the notification's sender, subject and account are other words"),
    "J7.1/arrival-s": (("fixture", ("read", ("measure", LOCK), "change")), "the notification took 10000 s to show"),
    "J7.1/actions-on-long-press": (("fixture", ("read", ("rows", LOCK + ".actions"), "empty")), "no Archive and Reply actions"),
    "J7.1/fetched-to-phone": (("fixture", ("read", ("core", "message/%s/fetched" % ARR["id"]), "change")), "the core says the mail was not fetched"),
    "J7.1/no-push-account-says-so": (("fixture", ("flag", "no-push-silent")), "the no-push accounts carry no note"),
    "J7.1/push-account-has-no-note": (("fixture", ("flag", "every-account-noted")), "every account carries the no-push note"),
    "J7.2/arrives-by-control": (("fixture", ("act", ("fixture", "notification-tap:%s" % ARR["id"]))), "the notification tap does nothing"),
    "J7.2/thread-is-the-arrival": (("fixture", ("read", ("rows", THREAD + ".subject"), "change")), "the thread's subject is another one"),
    "J7.2/code-pill-shows-code": (("fixture", ("read", ("rows", THREAD + ".code-pill"), "change")), "the pill shows another code"),
    "J7.2/copied-label": (("fixture", ("act", ("click", THREAD + ".code-pill"))), "the pill tap does nothing: no Copied"),
    "J7.2/clipboard-is-the-code": (("fixture", ("flag", "wrong-clipboard")), "the clipboard holds 000000"),
    "J7.2/marked-read": (("fixture", ("read", ("core", "message/%s/read" % ARR["id"]), "change")), "the core says the mail is unread"),
    "J7.2/copy-never-archives": (("fixture", ("read", ("core", "message/%s/archived" % ARR["id"]), "change")), "the core says the mail is archived"),
    "J7.2/offline-body-says-so": (("fixture", ("act", ("fixture", "offline:on"))), "offline mode is never turned on: the body stays"),
    "J7.2/online-body-has-text": (("fixture", ("act", ("fixture", "offline:off"))), "offline mode is never turned off: the body stays missing"),
    "J7.3/arrives-by-control": (("fixture", ("act", ("click", THREAD + ".back"))), "Back does nothing"),
    "J7.3/rows-before": (("fixture", ("read", ("rows", INBOX + ".list"), "empty")), "the inbox list cannot be read"),
    "J7.3/archived-row-leaves": (("fixture", ("flag", "archive-keeps-row")), "the archive swipe leaves the row"),
    "J7.3/snoozed-row-leaves": (("fixture", ("flag", "snooze-keeps-row")), "the snooze swipe leaves the row"),
    "J7.3/other-rows-stay": (("fixture", ("flag", "snooze-takes-row-above")), "the snooze swipe removes the row above it too"),
    "J7.3/archived-in-core": (("fixture", ("flag", "archive-hides-only")), "the row is hidden but the core holds no archive"),
    "J7.3/snoozed-in-core": (("fixture", ("flag", "snooze-hides-only")), "the row is hidden but the core holds no snooze"),
    "J7.3/undo-control-shows": (("fixture", ("read", ("rows", INBOX + ".undo"), "empty")), "no undo control"),
    "J7.3/undo-entries-written": (("fixture", ("read", ("core", "undo_entry/count"), "dec")), "one undo entry fewer than the swipes"),
    "J7.3/refused-row-comes-back": (("fixture", ("flag", "refused-drops-row")), "a refused archive drops the row anyway"),
    "J7.3/refusal-names-the-account": (("fixture", ("read", ("rows", INBOX + ".failure"), "empty")), "the refusal shows no reason"),
    "J7.4/arrives-by-control": (("fixture", ("act", ("click", INBOX + ".list.row.0"))), "the row tap opens nothing"),
    "J7.4/composer-opens": (("fixture", ("act", ("click", THREAD + ".reply"))), "Reply opens nothing"),
    "J7.4/from-address": (("fixture", ("read", ("rows", COMPOSER + ".from"), "change")), "the reply is from another address"),
    "J7.4/signature-present": (("fixture", ("flag", "no-signature")), "the signature is missing"),
    "J7.4/model-downloads-with-progress": (("fixture", ("act", ("fixture", "speech-model:missing"))), "the speech model is never missing: no download, no progress"),
    "J7.4/dictated-text": (("fixture", ("read", ("rows", COMPOSER + ".body"), "change")), "the dictation inserts other words"),
    "J7.4/undo-send-bar": (("fixture", ("read", ("rows", COMPOSER + ".undo"), "empty")), "no undo bar"),
    "J7.4/outbox-undoable": (("fixture", ("read", ("core", "outbox_entry/0/state"), "change")), "the outbox entry is in no undoable state"),
    "J7.4/outbox-sent-after-window": (("fixture", ("act", ("clock", "advance"))), "the clock never moves: the reply never leaves the outbox"),
    "J7.4/sent-label": (("fixture", ("read", ("rows", COMPOSER + ".undo"), "change")), "the bar never says Sent"),
    "J7.5/mac-probe-works": (("fixture", ("read", ("rows", "mac-main.list"), "empty")), "the Mac rows probe answers no rows"),
    "J7.5/archived-gone-on-mac": (("shared", "archive-not-propagated"), "the phone's archive never reaches the Mac"),
    "J7.5/reply-in-thread-on-mac": (("shared", "reply-not-propagated"), "the reply never reaches the Mac's thread"),
    "J7.5/reply-text-on-mac": (("shared", "reply-other-text"), "the Mac's thread shows the reply under another body"),
}


def selftest():
    """No app: J7's own steps run against fakephone's phone and Mac copies. Two runs of the journey per copy: the fixture-core run (J7.1 to J7.5; the three J7.5 cases that
    need the shared server are NOT JUDGED there, as on the rig) and the shared-server run (J7.5 only, on sharedserver.py's stand-in copies). The clean copy must be
    green on every check, each broken copy must turn its own check red, and every declared mutant must have a broken copy. Writes no results file."""
    import shutil
    import tempfile
    from lib import fakephone, harness

    here = os.path.abspath(__file__)
    me = sys.modules[__name__]
    declared = harness.declared_mutants(here)
    out = {"bad": 0}
    keep = {k: os.environ.get(k) for k in ("WHOOSH_SOCKET_DIR", "WHOOSH_MAC_SOCKET_DIR", "WHOOSH_IPHONE_SOCKET_DIR", "WHOOSH_RESULTS_DIR", sharedserver.ENV)}
    results_dir = tempfile.mkdtemp(prefix="j7-selftest-results-")
    old_instant = getattr(Ctl, "instant", None)
    old_sleep_consts = (sharedserver.POLL_S, sharedserver.THREAD_EVERY_S, sharedserver.ADD_WAIT_S, sharedserver.LIST_WAIT_S, sharedserver.PROPAGATE_WAIT_S, sharedserver.SEND_WAIT_S)
    servers = []
    ph = fakephone.Phone()

    def run_through(phone_srv, mac_srv, shared):
        """The journey through journeys/run.py's own path (harness.run_journey over the two sockets); returns its result dict."""
        os.environ["WHOOSH_SOCKET_DIR"], os.environ["WHOOSH_MAC_SOCKET_DIR"], os.environ["WHOOSH_RESULTS_DIR"] = phone_srv.dir, mac_srv.dir, results_dir
        os.environ.pop("WHOOSH_IPHONE_SOCKET_DIR", None)
        if shared:
            os.environ[sharedserver.ENV] = "127.0.0.1:1"
        else:
            os.environ.pop(sharedserver.ENV, None)
        noise = io.StringIO()
        with contextlib.redirect_stderr(noise):
            return harness.run_journey(me, here)

    def fixture_run(mut):
        ph.reset()
        ph.mut, ph.reads, ph.acts = mut, [], []
        return run_through(phone_srv, mac_srv, False)

    def shared_run(mutant):
        world = sharedserver._World(mutant)
        phone_copy, mac_copy = _SharedCopy(world, "phone"), _SharedCopy(world, "mac")
        a, b = fakephone.CopyServer(phone_copy), fakephone.CopyServer(mac_copy)
        servers.extend([a, b])
        try:
            return run_through(a, b, True)
        finally:
            a.close()
            b.close()

    class _SharedCopy(sharedserver._Copy):
        def type(self, text):
            if self.composer:
                self.typed = [text]             # the stand-in keeps every typed string (the account's address and password too): the reply is what the composer was typed
            else:
                sharedserver._Copy.type(self, text)

        def rows(self, ax):
            got = sharedserver._Copy.rows(self, ax)
            if ax == "mac-main.reader.thread" and self.w.mutant == "reply-other-text" and self.w.reply and len(got) > 1:
                got = got[:-1] + [{"text": "On Monday you wrote: " + got[-1]["text"]}]
            return got

    try:
        Ctl.instant = True                      # the stand-in answers at once: wait_for and phonedictate read once
        sharedserver.POLL_S, sharedserver.THREAD_EVERY_S = 0.05, 0.1
        sharedserver.ADD_WAIT_S, sharedserver.LIST_WAIT_S, sharedserver.PROPAGATE_WAIT_S, sharedserver.SEND_WAIT_S = 3, 3, 2, 3
        phone_srv, mac_srv = fakephone.Server(ph), fakephone.Server(ph, mac=True)
        servers.extend([phone_srv, mac_srv])

        def show(tag, r):
            green = sum(c[1] == "pass" for c in r["cases"])
            print("%s: %s, %d steps, %d checks, %d green, %d red, %d not judged" % (tag, r["verdict"], r["steps"], r["checks"], green, r["failed"], r["unjudged"]))

        r = fixture_run(None)
        show("clean stand-in phone and Mac (fixture core, J7.1 to J7.5)", r)
        labels = [c[0] for c in r["cases"]]
        need_shared = [c[0] for c in r["cases"] if c[1] == "unjudged"]
        for label, state, detail in r["cases"]:
            if state == "fail" or (state == "unjudged" and not detail.startswith(sharedserver.WHY)):
                print("  NOT GREEN on the clean stand-in: %s %s: %s" % (label, state, detail))
                out["bad"] += 1
        for lab in labels:
            if lab not in declared:
                print("NO MUTANT  %s" % lab)
                out["bad"] += 1
        for lab in sorted(set(declared) - set(labels)):
            print("STRAY MUTANT  %s" % lab)
            out["bad"] += 1
        rs = shared_run(None)
        show("clean stand-in phone and Mac (shared mail server, J7.5)", rs)
        shared_labels = [c[0] for c in rs["cases"]]
        still = [c[0] for c in rs["cases"] if c[1] != "pass"]
        for lab in still:
            print("  NOT GREEN on the clean shared-server stand-in: %s" % lab)
        out["bad"] += len(still)
        rest = sorted(set(need_shared) - {c[0] for c in rs["cases"] if c[1] == "pass"})
        print("clean total: %d checks judged green in one run or the other, %d not judged in either" % (len({c[0] for c in r["cases"] if c[1] == "pass"} | {c[0] for c in rs["cases"] if c[1] == "pass"}), len(rest)))
        out["bad"] += len(rest)
        for lab in sorted(set(BROKEN_COPIES) - set(declared)):
            print("STRAY BROKEN COPY  %s names no declared mutant" % lab)
            out["bad"] += 1
        reds = {}
        for lab in declared:
            if lab not in BROKEN_COPIES:
                print("NO BROKEN COPY  %s" % lab)
                out["bad"] += 1
                continue
            (kind, mut), what = BROKEN_COPIES[lab]
            if kind == "shared":
                if lab not in shared_labels:
                    print("NO SHARED-SERVER CHECK  %s" % lab)
                    out["bad"] += 1
                    continue
                got = {c[0]: c for c in shared_run(mut)["cases"]}
            else:
                got = {c[0]: c for c in fixture_run(mut)["cases"]}
            state = got[lab][1] if lab in got else "missing"
            also = sorted(k for k, c in got.items() if c[1] == "fail" and k != lab)
            print("mutant %-38s -> %s  (%s)%s" % (lab, "RED" if state == "fail" else "NOT RED (%s)" % state, what, ("  also red: " + ", ".join(also)) if also else ""))
            out["bad"] += state != "fail"
            reds[lab] = state == "fail"
        # a normal run of J7 on the clean stand-in is the same journey: no mutant row missing, none stray
        print("through run_journey on the clean stand-in (fixture core; its not judged are the J7.5 cases that need the shared server): %s, %d steps, %d checks, %d failed, %d not judged, no mutant row for %s, stray rows %s"
              % (r["verdict"], r["steps"], r["checks"], r["failed"], r["unjudged"], r["no_mutant"], r["stray_mutant"]))
        out["bad"] += bool(r["no_mutant"]) or bool(r["stray_mutant"]) or r["failed"] != 0
    finally:
        for sv in servers:
            sv.close()
            shutil.rmtree(sv.dir, ignore_errors=True)
        shutil.rmtree(results_dir, ignore_errors=True)
        sharedserver.POLL_S, sharedserver.THREAD_EVERY_S, sharedserver.ADD_WAIT_S, sharedserver.LIST_WAIT_S, sharedserver.PROPAGATE_WAIT_S, sharedserver.SEND_WAIT_S = old_sleep_consts
        if old_instant is None:
            if "instant" in Ctl.__dict__:
                del Ctl.instant
        else:
            Ctl.instant = old_instant
        for k, v in keep.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
    print("%s selftest: %d checks, %d mutants, %d not red or missing" % (ID, len(labels), len(declared), out["bad"]))
    return 1 if out["bad"] else 0


# MUTANTS
# J7.1/notification-shown | the relay never pushes
# J7.1/notification-fields | the notification shows another sender, subject or account
# J7.1/arrival-s | the push waits for the next poll
# J7.1/actions-on-long-press | Archive and Reply are missing
# J7.1/fetched-to-phone | the notification arrives with no message stored
# J7.1/no-push-account-says-so | an account without push opens with no word about it
# J7.1/push-account-has-no-note | every account carries the no-push note (control for the line above)
# J7.2/arrives-by-control | a tap opens the app but not the thread
# J7.2/thread-is-the-arrival | the tap opens another thread
# J7.2/code-pill-shows-code | the pill shows no code or a wrong one
# J7.2/copied-label | no Copied confirmation
# J7.2/clipboard-is-the-code | the clipboard gets another string
# J7.2/marked-read | the thread is not marked read
# J7.2/copy-never-archives | copying archives the message
# J7.2/offline-body-says-so | offline shows a blank or stale body
# J7.2/online-body-has-text | the body stays "not downloaded yet" once online (control for the line above)
# J7.3/arrives-by-control | back leaves the thread open
# J7.3/rows-before | the list cannot be read (baseline reading)
# J7.3/archived-row-leaves | the archive swipe leaves the row
# J7.3/snoozed-row-leaves | the snooze swipe leaves the row
# J7.3/other-rows-stay | a swipe removes a neighbouring row too
# J7.3/archived-in-core | the row is hidden but not archived
# J7.3/snoozed-in-core | the row is hidden but not snoozed
# J7.3/undo-control-shows | no undo control
# J7.3/undo-entries-written | swipes write no undo entries
# J7.3/refused-row-comes-back | a refused archive drops the row anyway
# J7.3/refusal-names-the-account | the refusal shows no reason
# J7.4/arrives-by-control | the row does not open
# J7.4/composer-opens | Reply opens nothing
# J7.4/from-address | the reply goes out from another account
# J7.4/signature-present | the signature is missing
# J7.4/model-downloads-with-progress | a missing speech model fails silently, no progress
# J7.4/dictated-text | dictation inserts nothing or other words
# J7.4/undo-send-bar | no undo bar
# J7.4/outbox-undoable | send writes no outbox entry in its undo window
# J7.4/outbox-sent-after-window | the reply never leaves the outbox
# J7.4/sent-label | the reply never reports Sent
# J7.5/mac-probe-works | the Mac rows probe is broken (control for the absence below)
# J7.5/archived-gone-on-mac | the iPhone archive does not sync to the Mac
# J7.5/reply-in-thread-on-mac | the reply is missing on the Mac
# J7.5/reply-text-on-mac | the Mac shows another body


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else 0)
