"""The Mac copy and the iPhone copy on the REAL core against ONE loopback mail server (item f3cafb91).

`WHOOSH_RIG_HIDDEN=1 sh scripts/iphone-rig.sh --with-mac --shared-server <Mac .app> <iPhone .app> -- <check>` starts checks/realcore_mailfake.py
(`--serve --act`: IMAP with MOVE, APPEND and IDLE, SMTP that keeps every message; 127.0.0.1 only), launches BOTH copies without `--fixture-core`
(`--rig-hidden`, `WHOOSH_CORE_PLAIN_LOOPBACK=1`, `WHOOSH_CORE_CHECK_SERVERS=<imap>:<smtp>`) and exports `WHOOSH_SHARED_SERVER=<imap>:<smtp>` to the check.
Without that variable nothing here runs and the callers say "not judged: no shared server".

drive(mac, phone, reply) does what a person would, in the order of J7.5 and the device-parity seam, and returns what both copies showed:
  1. the one seeded account is added on each copy through the Add account model (the same ids the real-core slices drive: mac-add-account.*), the Mac's
     first-start flow is read through with Enter, the phone leaves its welcome (journeys/lib/phonestart.py) now that it holds an account;
  2. both lists show the server's mail; the phone archives its first row; the Mac's list loses that mail by itself (the core's change subscription);
  3. the phone opens its (new) first row, replies with the words, sends; the undo window runs in real time (the clock verb is the fixture core's);
  4. the Mac opens the same mail (Enter on its first row) and reads its thread until the words are there.
Every step the app refuses, and every wait that runs out, is a FAIL that names the step: with the server up there is no "not judged".
"""
import importlib.util
import os
import re
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, os.path.dirname(HERE))          # journeys/: `lib` is a package, one Unreadable class for every caller
from lib.ctl import Unreadable  # noqa: E402
from lib import phonestart  # noqa: E402

ENV = "WHOOSH_SHARED_SERVER"
WHY = ("no shared server: both copies on the real core against one loopback mail server needs `scripts/iphone-rig.sh --with-mac --shared-server` "
       "(it sets %s); without that flag the two copies each own a private fixture mailbox" % ENV)
A = "mac-add-account"
ADD_WAIT_S = 90          # realcore-check.DETECT_WAIT_S: every step of the add has its own timeout
LIST_WAIT_S = 90         # the first sync of the seeded mail
PROPAGATE_WAIT_S = 90    # an archive or a reply reaching the other copy: the server's IDLE tells it, the core re-reads
SEND_WAIT_S = 60         # the undo window (UNDO_SEND_DEFAULT_S, 10 s) plus the send
POLL_S = 1.0
THREAD_EVERY_S = 5.0     # the Mac's thread is opened again this often while the reply has not shown


def enabled():
    return bool(os.environ.get(ENV, "").strip())


def _norm(text):
    return re.sub(r"[^a-z0-9 ]", "", (text or "").lower()).strip()


def mailfake():
    spec = importlib.util.spec_from_file_location("realcore_mailfake", os.path.join(ROOT, "checks", "realcore_mailfake.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


class Failed(Exception):
    """A step that did not happen, in words naming it. The caller records it as a FAIL."""


def until(fn, secs, what, every=POLL_S):
    """The first truthy value of fn() within secs; Failed(what) otherwise. An Unreadable from fn counts as 'not yet' until the time is up."""
    end = time.time() + secs
    last = None
    while True:
        try:
            got = fn()
            if got:
                return got
            last = "answered %r" % (got,)
        except Unreadable as e:
            last = str(e)
        if time.time() >= end:
            raise Failed("%s (waited %ds; last: %s)" % (what, secs, last))
        time.sleep(every)


def step(label, fn):
    """Run one app action; an app that refuses it fails the drive, naming the step."""
    try:
        return fn()
    except Unreadable as e:
        raise Failed("%s: the app refused it: %s" % (label, e))


def rows(ctl, ax):
    try:
        return ctl.rows(ax)
    except Unreadable:
        return []


def add_account(ctl, who, address, password):
    """The Add account flow of the real-core slices (checks/realcore-check.py drive5), by the same ids. Idempotent: a copy that already lists an account keeps it."""
    have = rows(ctl, "mac-main.sidebar.accounts") if who == "mac" else rows(ctl, "iphone-mailboxes.accounts")
    if have:
        return "already added"
    step("%s: open Add account" % who, lambda: ctl.click(A + ".open"))
    step("%s: address field" % who, lambda: ctl.click(A + ".address"))
    step("%s: type the address" % who, lambda: ctl.type(address))
    step("%s: password field" % who, lambda: ctl.click(A + ".password"))
    step("%s: type the password" % who, lambda: ctl.type(password))
    step("%s: continue" % who, lambda: ctl.click(A + ".continue"))
    until(lambda: (rows(ctl, A + ".state") or [{}])[0].get("stage") not in (None, "detecting", "entry"), ADD_WAIT_S, "%s: the server was never detected" % who)
    step("%s: add" % who, lambda: ctl.click(A + ".add"))
    stage = until(lambda: (rows(ctl, A + ".state") or [{}])[0].get("stage") in ("added", "summary") and "ok", ADD_WAIT_S,
                  "%s: the account was never added (stage %r, error %r)" % (who, (rows(ctl, A + ".state") or [{}])[0].get("stage"), (rows(ctl, A + ".error") or [{}])[0].get("text")))
    step("%s: done" % who, lambda: ctl.click(A + ".done"))
    return stage


def read_first_start(mac):
    """The Mac's first launch shows the first-start flow over the inbox and swallows the mail keys: read through it with Enter (as realcore-check does)."""
    for _ in range(10):
        st = rows(mac, "mac-first-start.state")
        if not (st and st[0].get("active")):
            return
        step("mac: first-start Enter", lambda: mac.key("Enter"))
        time.sleep(0.5)
    time.sleep(1.5)


def subjects(ctl, ax):
    return [r.get("subject") for r in rows(ctl, ax)]


def drive(mac, phone, reply, waits=None):
    """Returns {"gone_on_mac": bool, "archived": subject, "reply_in_thread": bool, "reply_note": why it is False when the Mac could not open the thread, "thread_texts": [..], "accounts": {"mac": [[name, colour]], "iphone": [[name, colour]]}}.
    Raises Failed naming the step that did not happen."""
    w = dict(add=ADD_WAIT_S, lists=LIST_WAIT_S, propagate=PROPAGATE_WAIT_S, send=SEND_WAIT_S)
    w.update(waits or {})
    fake = mailfake()
    out = {}
    add_account(mac, "mac", fake.ADDRESS, fake.PASSWORD)
    read_first_start(mac)
    add_account(phone, "phone", fake.ADDRESS, fake.PASSWORD)
    left = step("phone: leave the welcome", lambda: phonestart.leave_welcome(phone))
    # On the real engine the model's surface word is not reliable before the first door (phone9: 'mac-main' with the inbox on screen, as after `fixture reset`):
    # what is judged is that the phone's inbox LIST serves the server's mail, which the next line waits for; only the welcome itself is a stop.
    until(lambda: phone.uistate().get("surface") != phonestart.WELCOME, 15, "phone: the app is still on the welcome (%r) after the account was added" % (left,))
    until(lambda: len(subjects(mac, "mac-main.list")) >= 2, w["lists"], "mac: the server's mail never reached the Mac's list")
    until(lambda: len(subjects(phone, "iphone-inbox.list")) >= 2, w["lists"], "phone: the server's mail never reached the phone's list")
    tab = lambda ctl, ax, f: [[r.get("name"), r.get(f)] for r in rows(ctl, ax)]
    out["accounts"] = {"mac": tab(mac, "mac-main.sidebar.accounts", "colour"), "iphone": tab(phone, "iphone-mailboxes.accounts", "mark_colour")}
    mac_before = subjects(mac, "mac-main.list")
    archived = (subjects(phone, "iphone-inbox.list") or [None])[0]
    out["archived"] = archived
    if archived not in mac_before:
        raise Failed("the control failed: the phone's first mail %r is not in the Mac's list %r before the archive" % (archived, mac_before))
    step("phone: archive row 0", lambda: phone.click("iphone-inbox.list.row.0.swipe-archive"))
    until(lambda: archived not in subjects(phone, "iphone-inbox.list"), 15, "phone: the archived mail %r is still in the phone's list" % archived)
    try:
        until(lambda: archived not in subjects(mac, "mac-main.list"), w["propagate"], "mac: the archived mail %r is still in the Mac's list" % archived)
        out["gone_on_mac"] = True
    except Failed:
        out["gone_on_mac"] = False
    # the reply: the phone's first row now is the mail the Mac's first row is
    replied = until(lambda: (subjects(phone, "iphone-inbox.list") or [None])[0], 15, "phone: no mail left to reply to")
    step("phone: open row 0", lambda: phone.click("iphone-inbox.list.row.0"))
    until(lambda: phone.uistate().get("surface") == "iphone-thread", 15, "phone: the thread did not open")
    step("phone: reply", lambda: phone.click("iphone-thread.reply"))
    until(lambda: (rows(phone, "iphone-composer.header") or [{}])[0].get("is_open"), 20, "phone: the composer did not open")
    step("phone: type the reply", lambda: phone.type(reply))
    step("phone: send", lambda: phone.click("iphone-composer.send"))
    until(lambda: (rows(phone, "iphone-composer.undo") or [{}])[0].get("text") == "Sent", w["send"], "phone: the bar never said Sent")
    top = until(lambda: (subjects(mac, "mac-main.list") or [None])[0], 15, "mac: no first row")
    out["replied"] = replied
    if top != replied:
        out["reply_in_thread"], out["thread_texts"] = False, []
        out["reply_note"] = "the Mac's first row is %r, not %r, the mail the phone replied to (its archive did not reach the Mac: the Mac cannot open the same thread)" % (top, replied)
        return out
    texts = []

    def read_thread():
        try:
            mac.key("Escape")
        except Unreadable:
            pass
        step("mac: open the first row", lambda: mac.key("Enter"))
        time.sleep(0.8)
        texts[:] = [r.get("text", "") for r in rows(mac, "mac-main.reader.thread")]
        return any(_norm(reply) in _norm(t) for t in texts)

    try:
        until(read_thread, w["propagate"], "mac: the reply %r is not in the Mac's thread" % reply, every=THREAD_EVERY_S)
        out["reply_in_thread"] = True
    except Failed:
        out["reply_in_thread"] = False
    out["thread_texts"] = texts
    return out


# ---------------------------------------------------------------------------------------------- --selftest: two stand-in copies and one stand-in server
class _World:
    """One mailbox both stand-ins read: what the server holds. `mutant` breaks one thing."""
    def __init__(self, mutant=None):
        self.mutant = mutant
        self.subjects = ["Check message %d" % n for n in range(6, 0, -1)]
        self.archived, self.reply = set(), None      # (subject, text) once the phone has sent
        self.t_archive = self.t_send = None


class _Copy:
    def __init__(self, world, who):
        self.w, self.who = world, who
        self.stage, self.added, self.t_added, self.open_thread, self.surface = None, False, None, False, "iphone-welcome-accounts" if who == "phone" else "mac-main"
        self.typed, self.composer, self.sent_at, self.row0_thread = [], False, None, None

    # the journeys.lib.ctl.Ctl surface
    def click(self, ax):
        w = self.w
        if w.mutant == "phone-refuses-archive" and self.who == "phone" and ax.endswith("swipe-archive"):
            raise Unreadable("%s did nothing" % ax)
        if w.mutant == "phone-add-refused" and self.who == "phone" and ax == A + ".open":
            raise Unreadable("%s did nothing" % ax)
        if ax == A + ".open": self.stage = "entry"
        elif ax == A + ".continue": self.stage = "ready"
        elif ax == A + ".add": self.stage = "added"
        elif ax == A + ".done": self.added, self.t_added = True, time.time()
        elif ax == "iphone-welcome.scan": self.step = "scanning"
        elif ax == "iphone-welcome.continue": self.step = "notify"
        elif ax == "iphone-welcome.not-now":
            if w.mutant != "phone-stuck-on-welcome": self.surface = "mac-main"
        elif ax == "iphone-inbox.list.row.0.swipe-archive":
            w.archived.add(self.listed()[0]); w.t_archive = time.time()
        elif ax == "iphone-inbox.list.row.0": self.surface = "iphone-thread"; self.row0_thread = self.listed()[0]
        elif ax == "iphone-thread.reply": self.composer = True
        elif ax == "iphone-composer.send":
            w.t_send = time.time()
            subj = self.row0_thread if w.mutant != "reply-in-wrong-thread" else "Check message 99"
            w.reply = (subj, " ".join(self.typed))
        return {"ok": True}

    def type(self, text):
        self.typed.append(text)

    def key(self, chord):
        if chord == "Enter" and self.who == "mac": self.open_thread = True
        if chord == "Escape" and self.who == "mac": self.open_thread = False

    def uistate(self):
        return {"surface": self.surface}

    def close(self):
        pass

    def listed(self):
        w = self.w
        out = [s for s in w.subjects if s not in w.archived]
        if self.who == "mac" and w.mutant == "archive-not-propagated":
            out = list(w.subjects)
        if self.who == "mac" and w.t_archive and time.time() - w.t_archive < 0.3:
            out = list(w.subjects)
        return out

    def rows(self, ax):
        w = self.w
        if ax in ("mac-main.sidebar.accounts", "iphone-mailboxes.accounts"):
            return [{"name": "owner", "colour": "#2F6FED", "mark_colour": "#2F6FED"}] if self.added else []
        if ax == A + ".state": return [{"stage": self.stage}] if self.stage else []
        if ax == A + ".error": return [{"text": ""}]
        if ax == "mac-first-start.state": return [{"active": False}]
        if not self.added or time.time() - self.t_added < 0.2:
            return []
        if ax in ("mac-main.list", "iphone-inbox.list"):
            return [{"subject": s, "id": s} for s in self.listed()]
        if ax == "iphone-composer.header": return [{"is_open": self.composer}]
        if ax == "iphone-composer.undo":
            return [{"text": "Sent"}] if w.t_send and time.time() - w.t_send > 0.2 else ([{"text": "Sending"}] if w.t_send else [])
        if ax == "mac-main.reader.thread" and self.open_thread:
            top = self.listed()[0]
            rows = [{"text": "Body of " + top}]
            if w.reply and w.mutant != "reply-not-propagated" and w.reply[0] == top and time.time() - w.t_send > 0.4:
                rows.append({"text": w.reply[1]})
            return rows
        return []


def _selftest():
    global POLL_S, THREAD_EVERY_S
    POLL_S, THREAD_EVERY_S = 0.05, 0.1
    waits = dict(add=3, lists=3, propagate=2, send=3)
    n = bad = 0
    def ok(name, cond):
        nonlocal n, bad
        n += 1; bad += not cond
        print(("PASS " if cond else "FAIL ") + name)
    def run(mutant=None):
        w = _World(mutant)
        try:
            return drive(_Copy(w, "mac"), _Copy(w, "phone"), "seam parity reply", waits), None
        except Failed as e:
            return None, str(e)
    r, why = run()
    ok("clean: the archive is gone on the Mac and the reply is in the Mac's thread (%s)" % (why or "no failure"), bool(r) and r["gone_on_mac"] is True and r["reply_in_thread"] is True and r["archived"] == "Check message 6")
    ok("clean: both copies list the account with the same name and colour", bool(r) and r["accounts"]["mac"] == r["accounts"]["iphone"] and len(r["accounts"]["mac"]) == 1)
    r, why = run("archive-not-propagated")
    ok("broken copy: the archive never reaches the server's other reader -> gone_on_mac False", bool(r) and r["gone_on_mac"] is False)
    r, why = run("reply-not-propagated")
    ok("broken copy: the reply never reaches the Mac -> reply_in_thread False", bool(r) and r["reply_in_thread"] is False)
    r, why = run("reply-in-wrong-thread")
    ok("broken copy: the reply lands in another thread -> reply_in_thread False", bool(r) and r["reply_in_thread"] is False)
    r, why = run("phone-refuses-archive")
    ok("a refused archive is a FAIL naming the step, never not judged", r is None and "phone: archive row 0: the app refused it" in (why or ""))
    r, why = run("phone-add-refused")
    ok("a refused Add account on the phone is a FAIL naming the step", r is None and "phone: open Add account: the app refused it" in (why or ""))
    r, why = run("phone-stuck-on-welcome")
    ok("a phone that cannot leave its welcome is a FAIL naming the surface", r is None and "iphone-welcome-accounts" in (why or ""))
    saved = os.environ.pop(ENV, None)
    ok("without the variable the shared server is not enabled", enabled() is False)
    os.environ[ENV] = "1:2"
    ok("with the variable it is", enabled() is True)
    if saved is None: os.environ.pop(ENV, None)
    else: os.environ[ENV] = saved
    print("selftest: %d cases, %d failed" % (n, bad))
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(_selftest() if "--selftest" in sys.argv else 2)
