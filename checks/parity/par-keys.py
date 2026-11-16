#!/usr/bin/env python3
"""par-keys: every LIVE binding of the Superhuman preset is pressed through the socket key verb on the fixture core,
and the visible outcome its command promises is read back. One check per binding, grouped by screen.

    python3 checks/parity/par-keys.py             drive the running rig copy; exit 1 when F > 0 or U > 0
    python3 checks/parity/par-keys.py --selftest  the checks on a model of the preset, clean and with a mutant per group
    python3 checks/parity/par-keys.py --coverage  only the inventory: every binding of the source against its check

Where the bindings come from: Sources/Features/Shortcuts/KeyPreset.swift and Commands.swift, read by the v8 check's own
reader (checks/superhuman-v8-check.py). No key list is typed here. A binding the source holds and this file has no
check for is a failure (coverage), and so is a check for a binding the source no longer holds.

Groups (the screen that owns the outcome): list, reader, composer, sidebar, window, find, accounts, soon.
A coming-soon binding asserts its mark in the key list (mac-shortcuts.map: status coming-soon and the feature named by
Commands.swift) and nothing else.

Expected values come from the design fixture (docs/design/fixtures/mac-main-sample.json) and the sources, never from a
built screen. Run live:
  sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/par-keys.py'
Every check acts on its own message of the fixture (data survives `fixture reset`), so one run holds one pass of each key.
"""
import importlib.util
import os
import re
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, Journey, Check, print_cases, print_model_cases   # noqa: E402
from lib.fixture import FX, ROWS, MSG, const                              # noqa: E402
from lib.ctl import Unreadable                                       # noqa: E402
sys.path.insert(0, os.path.join(ROOT, "checks", "checklib"))
import resetguard as RG                                              # noqa: E402

_spec = importlib.util.spec_from_file_location("superhuman_v8_check", os.path.join(ROOT, "checks", "superhuman-v8-check.py"))
V8 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(V8)

ID = "par-keys"
TITLE = "Superhuman keys: every live binding pressed, its visible outcome read"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "each key of the Superhuman preset does what its command promises on the screen that owns it; a coming-soon key says so in the key list"
GROUPS = ["list", "reader", "composer", "sidebar", "window", "find", "accounts", "aireply", "soon"]
OWNER_GROUP = {"inboxlist": "list", "reader": "reader", "composermac": "composer", "sidebar": "sidebar", "macshell": "window", "appshell": "reader"}
ENGINE_GROUP = {"next": "list", "previous": "list", "archive": "list", "snooze": "list", "label": "list", "pin": "list", "undo": "list",
                "delete": "list", "junk": "list", "move": "list", "star": "list", "unread": "list", "mute": "list", "enter": "list",
                "next-split": "list", "previous-split": "list", "reply": "reader", "reply-all": "reader", "forward": "composer",
                "compose": "composer", "ai-reply": "aireply", "search": "find", "palette": "find", "keys": "find", "open": "list", "accounts": "accounts",
                # live since the reader (round 9) and composer (pop-out) rounds: the screen that owns the outcome, as each promise declares it
                "open-links": "reader", "expand-message": "reader", "expand-all-messages": "reader", "toggle-message-header": "reader",
                "next-message": "reader", "previous-message": "reader", "scroll-down": "reader", "scroll-up": "reader", "jump-top": "reader",
                "toggle-colours": "reader",     # live since A129 (KeyPreset.swift:132, Commands.swift:61, MessageBodyFeature.swift): the open mail's colour switch
                "jump-bottom": "reader", "cycle-links": "reader", "cycle-links-back": "reader", "popout-draft": "composer", "popout-reply-all": "composer", "popout-new": "composer",
                "popout-forward": "composer", "popout-draft-search": "composer", "switch-draft": "composer", "popout-reply": "composer",
                # live since composermac round 4 (their handlers were wired; the registry said stub)
                "attach": "composer", "send-and-done": "composer", "instant-send": "composer", "remind-me": "composer",
                "move-contacts-to-bcc": "composer", "outdent-list": "composer",
                # live since the shell keys round 10 (MacShellCommands.swift provides both)
                "focus-left": "window", "focus-right": "window"}
CORPUS_CANDIDATES = [os.path.join(ROOT, "_captures", "reader", "r5", "links-corpus.json"),
                     os.path.join(ROOT, "..", "..", "_captures", "reader", "r5", "links-corpus.json")]

# fixture facts the promises use
LISTS = [m for m in FX["messages"] if m.get("list_unsubscribe")]
THREAD = FX["messages"][0]                                         # msg-01: three messages, the first row
ACCOUNT_IDS = [a["id"] for a in FX["accounts"]]
MAILBOX_KEY = {"go-sent": "sent", "go-drafts": "drafts", "go-done": "archive", "go-reminders": "snoozed", "go-spam": "junk",
               "go-trash": "trash", "go-starred": "starred", "go-muted": "muted", "go-all-mail": "all mail"}
SHIFT_SYMBOL = {"⇧!", "⇧#"}


# ---------------------------------------------------------------- the inventory (from the source, never typed here)

def label_of(chord):
    """The key as the key list names it: a chord, or 'G then I' for a sequence given as a tuple."""
    if isinstance(chord, tuple):
        a, b = chord
        return "%s then %s" % (a, b[1:] if b in SHIFT_SYMBOL else b)
    return chord


def inventory(p=None, cmds=None):
    """[{chord, label, command, kind, status, owner, group}] for every way into a command of the Superhuman preset."""
    p = p or V8.preset()
    cmds = cmds or V8.commands()
    out = []

    def add(kind, chord, cid):
        title, status, owner = cmds.get(cid, ("", "unknown", ""))
        group = "soon" if status == "soon" else (OWNER_GROUP.get(owner) if status == "stub" else ENGINE_GROUP.get(cid)) or "list"
        out.append({"chord": chord, "label": label_of(chord), "command": cid, "kind": kind, "status": status, "owner": owner, "group": group})
    for c, cid in p["rows"]:
        add("row", c, cid)
    for c, cid in p["also"]:
        add("also", c, cid)
    for c, cid in p["context"]:
        add("context", c, cid)
    for a, b, cid in p["sequences"]:
        add("sequence", (a, b), cid)
    for c, cid in p["fixed"]:
        add("fixed", c, cid)
    for cid in {b["command"] for b in out if b["command"].startswith("switch-account-")}:
        for b in out:
            if b["command"] == cid:
                b["group"] = "accounts"
    return out


def ident(b):
    return (b["label"], b["command"])


def slug_of(b):
    names = {"⇧": "shift-", "⌘": "cmd-", "⌃": "ctrl-", "⌥": "alt-"}
    sym = {"!": "bang", "#": "hash", ",": "comma", ";": "semicolon", "[": "lbracket", "]": "rbracket", "\\": "backslash", "/": "slash",
           "-": "minus", "=": "equals", "?": "question", "+": "-"}
    out = ""
    for ch in b["label"]:
        if ch in names:
            out += names[ch]
        elif ch in sym:
            out += sym[ch]
        elif ch == " ":
            out += "-"
        else:
            out += ch.lower() if ch.isalnum() else "x"
    out = re.sub(r"-+", "-", out).strip("-")
    return "%s-%s" % (out, b["command"])


# ---------------------------------------------------------------- the checks' hands: actions and reads

SLEEP = time.sleep          # the selftest swaps it for a no-op: the model needs no real time


class NoBar(Exception):
    """The composer's undo bar did not appear within the bounded wait."""


MONITOR = "mac-keyevent.monitor"
MONITOR_FIELDS = ("window_id", "window", "focus", "key_before", "key_after", "handled", "route", "undo", "can_undo", "windows", "text_before", "text_after")


def monitor_last(ctl):
    """The key verb's report of its last key (`rows mac-keyevent.monitor`, field `last`): {} when the build serves none. Never raises."""
    try:
        r = ctl.rows(MONITOR)
        last = r[0].get("last") if isinstance(r, list) and r and isinstance(r[0], dict) else None
        return last if isinstance(last, dict) else {}
    except (Unreadable, AttributeError, KeyError, IndexError, TypeError, ValueError):
        return {}


def said_of(last):
    """One report as words; a field an older build does not serve reads "absent"."""
    if not last:
        return "keyevent report absent"
    return "keyevent " + " ".join("%s=%r" % (f, last.get(f, "absent")) for f in MONITOR_FIELDS)


def view_fields(v):
    """The view the digest serves ("surface=mac-main;tab=primary;..."; a dict as well) as {field: value}; None when it is neither."""
    if isinstance(v, dict):
        return {str(k): v[k] for k in v}
    if isinstance(v, str) and "=" in v:
        return dict(p.split("=", 1) if "=" in p else (p, "") for p in v.split(";"))
    return None


def view_diff(launch, now):
    """The fields of the two views that differ, one `field launch=<a> now=<b>` each; both whole views when they are not field lists."""
    a, b = view_fields(launch), view_fields(now)
    if a is None or b is None:
        return "launch view %r, now %r" % (launch, now)
    d = ["%s launch=%r now=%r" % (f, a.get(f, "absent"), b.get(f, "absent")) for f in sorted(set(a) | set(b)) if a.get(f, "absent") != b.get(f, "absent")]
    return "; ".join(d) or "the two views read alike field by field"


def reset_view_detail(ctl):
    """After a reset the guard refused: the two views it compared, only the fields that differ, and where the shell stands. Never raises."""
    try:
        now = RG.digest(ctl)[2]
        out = " | view fields that differ: " + view_diff(RG.LAUNCH.get("v"), now)
    except (Unreadable, AttributeError, KeyError, IndexError, TypeError, ValueError) as e:
        out = " | view not read again (%s)" % e
    try:
        st = ctl.uistate()
        out += " | shell surface=%r focus=%r overlay=%r" % (st.get("surface", "absent"), st.get("first_responder", "absent"), st.get("overlay", "absent"))
    except (Unreadable, AttributeError, KeyError, TypeError, ValueError):
        pass
    return out


def expect_why(s, name, thunk, want, why):
    """`Step.expect`, and a failure also carries `why()`: the app's own words, read after the compare."""
    def run():
        got = thunk()
        s._eg = (want, got)
        return got == want, "expected %r, got %r; %s" % (want, got, why())
    s._record(name, run)


class K:
    """What a promise uses: actions (never checks) and reads (for the thunk). One per binding."""

    def __init__(self, s):
        self.s = s
        self.reports = []       # per REAL key: (text, arm, the key verb's own report read right after it)

    # actions
    def key(self, chord):
        self.s.key(chord)

    def click(self, ax):
        self.s.click(ax)

    def type(self, text):
        self.s.type(text)

    def raw(self, *a):
        self.s.raw(*a)

    def wait(self, ms):
        self.s._act(lambda: SLEEP(ms / 1000.0))

    def keycode(self, code, mods):
        """Press a physical key as an AZERTY keyboard types it (`key code:19 mods:cmd`): (the chord the app resolved, whether the engine ran it).
        The resolved chord comes in the reply, or in the words of the refusal ("the key <chord> (code n) does nothing")."""
        args = ["code:%d" % code] + (["mods:" + "+".join(mods)] if mods else [])
        try:
            return self.s.ctl.send("key", *args).get("chord"), True
        except Unreadable as e:
            m = re.search(r"the key (\S+) \(code \d+\)", str(e))
            if not m:
                raise
            return m.group(1), False

    def real(self, text, arm="a"):
        """A REAL key press (`keyevent`): an NSEvent through the shell's monitor and then the window, as the keyboard sends it. Arm c
        resigns the window's key status first, as the hidden rig and a window that is not key are."""
        def go():
            self.s.ctl.send("keyevent", "arm:" + arm, text)
            self.reports.append((text, arm, monitor_last(self.s.ctl)))
        self.s._act(go)

    def what_the_app_said(self, last=3):
        """For a failed key case: the key verb's report of each of the last REAL keys, and the shell's own focus, read now. Never raises."""
        keys = "; ".join("key %r arm %s -> %s" % (t, a, said_of(r)) for t, a, r in self.reports[-last:]) or "no real key was sent"
        try:
            st = self.c.uistate()
            shell = "surface=%r focus=%r overlay=%r" % (st.get("surface", "absent"), st.get("first_responder", "absent"), st.get("overlay", "absent"))
        except (Unreadable, AttributeError, KeyError, TypeError, ValueError) as e:
            shell = "unreadable (%s)" % e
        try:
            pane = repr(self.c.rows("mac-main.shell")[0].get("focused_pane", "absent"))
        except (Unreadable, AttributeError, KeyError, IndexError, TypeError, ValueError) as e:
            pane = "unreadable (%s)" % e
        return "app said: %s; shell %s focused pane %s" % (keys, shell, pane)

    def refused(self, chord):
        """True when the key passes on (the socket answers ok:false)."""
        try:
            self.s.ctl.key(chord)
            return False
        except Unreadable:
            return True

    def home(self):
        """The plain list of the first tab, nothing open, nothing selected; data stays (fixture reset keeps the core)."""
        s = self.s
        if s.j.ctl is None:      # a dry run: the setup cannot run, and the promise is NOT JUDGED (the drive body would otherwise run it on stand-ins)
            raise Unreadable(s.j.dry_reason or "dry run: no socket")
        try:
            if s.ctl.uistate().get("surface") == "mac-composer":
                s.key("⇧⌘,")
                s.key("Enter")
        except Unreadable:
            pass
        try:
            # The case before may leave the search or a panel open (main81: ctrl-1 read mac-search at launch's mac-main); closing what the last case
            # opened is the check's job, not the reset's, so the two Escapes come first and the digest then compares the state the reset restored.
            s.key("Escape")
            s.key("Escape")
            RG.reset(s.ctl)   # the digest after the reset equals the launch digest, else the case is NOT JUDGED (tests-r58)
        except Unreadable as e:
            raise Unreadable(str(e) + reset_view_detail(s.ctl))
        s.key("Escape")
        s.key("Escape")
        s.click("mac-main.accounts.all")
        s.click("macshell.window.close-others")
        s.click("mac-main.splits.0")
        s.key("⌘0")
        # the ⌘\ check leaves the sidebar hidden and `fixture reset` keeps it so: with no sidebar Left has no pane to go to
        # (docs/lanes/macshell.md "Fix round after main51"). Every check starts with the sidebar showing.
        try:
            if s.ctl.rows("mac-main.sidebar.visibility")[0]["hidden"]:
                s.key("⌘\\")
        except (Unreadable, KeyError, IndexError):
            pass

    def focus(self, mid):
        self.s.click("mac-main.list.row.%d" % self.row_of(mid))

    def reader(self, mid="msg-01"):
        self.home()
        self.focus(mid)
        self.s.click("mac-main.reader.open")

    def composer(self, mid="msg-01"):
        self.reader(mid)
        self.s.click("mac-main.reader.open-full-composer")
        self.wait(250)

    def corpus(self):
        """The reader corpus (links and dates in the newest message); the app keeps it for its life: its promises run last."""
        path = next((p for p in CORPUS_CANDIDATES if os.path.exists(p)), CORPUS_CANDIDATES[0])
        self.home()
        self.s.raw("fixture", "corpus", os.path.abspath(path))
        for _ in range(len(ROWS) + 4):
            self.s.key("K")
        self.s.click("mac-main.reader.open")

    # reads
    @property
    def c(self):
        return self.s.ctl

    def ids(self):
        return [r["id"] for r in self.c.rows("mac-main.list")]

    def _served(self, mid):
        """The served row that says what the fixture message `mid` says (its subject and sender), not the literal id; the id is the fallback."""
        rows = self.c.rows("mac-main.list")
        r = RG.pick(rows, MSG[mid]) if mid in MSG else None
        return r if r is not None else next((x for x in rows if x["id"] == mid), None)

    def row_of(self, mid):
        r = self._served(mid)
        if r is None:
            raise Unreadable("the served list holds no row for %s (%s)" % (mid, MSG.get(mid, {}).get("subject")))
        return self.c.rows("mac-main.list").index(r)

    def row(self, mid):
        r = self._served(mid)
        if r is None:
            raise Unreadable("the served list holds no row for %s" % mid)
        return r

    def lst(self, f):
        return self.c.rows("mac-main.list.state")[0][f]

    def ui(self, f):
        return self.c.uistate()[f]

    def core(self, path):
        return self.c.core(path)

    def sel(self):
        return [r["id"] for r in self.c.rows("mac-main.list.selection")]

    def tabs(self):
        return [r["id"] for r in self.c.rows("mac-main.splits")]

    def rd(self, f):
        return self.c.rows("mac-main.reader.state")[0][f]

    def scroll(self):
        return self.c.rows("mac-main.reader.scroll")[0]

    def header_open(self):
        return [r["open"] for r in self.c.rows("mac-main.reader.header")]

    def collapse_headers(self):
        """The reader keeps a thread's open headers while the thread stays shown, and an earlier key (shift O) leaves them open: start from the default."""
        for r in self.c.rows("mac-main.reader.header"):
            if r.get("open"):
                self.s.click("mac-main.reader.header.%d" % r["index"])

    def ring(self):
        return [r["ring"] for r in self.c.rows("mac-main.reader.links")]

    def hdr(self, f):
        return self.c.rows("mac-composer.header")[0][f]

    def fmt(self, f):
        return self.c.rows("mac-composer.format")[0][f]

    def entries(self):
        return int(self.c.core("outbox_entry/count"))

    def shell(self, f):
        return self.c.rows("mac-main.shell")[0][f]

    def windows(self):
        return len(self.c.winframes())

    def composer_visible(self):
        st = self.c.uistate()
        return "mac-composer" in (st.get("surface"), st.get("overlay"))

    def search_focused(self):
        rows = self.c.rows("mac-search.bar")
        return bool(rows and rows[0].get("focused") and rows[0].get("owner") == "search")

    def dispatch(self):
        d = self.c.rows("mac-shortcuts.dispatch")
        return (d[-1]["command"], d[-1]["outcome"]) if d else None

    def settle(self, fn, want, tries=30):
        """Poll a read until it equals want (windows open on the next run-loop turn); the last value."""
        v = fn()
        for _ in range(tries):
            if v == want:
                return v
            SLEEP(0.05)
            v = fn()
        return v

    # ---- composer reads that wait a bounded time and say what was there on a miss (main79: an undo bar read once, right after the key, read as absent)
    def safe(self, fn):
        try:
            return fn()
        except Exception as e:                                       # noqa: BLE001  evidence never hides the verdict it explains
            return "unread (%s: %s)" % (type(e).__name__, str(e)[:120])

    def composer_state(self):
        """What decides a missing undo bar or a wrong send: the bar's rows, the outbox count, the composer's line, To, Bcc, the draft's text length, the surface."""
        h = self.safe(lambda: self.c.rows("mac-composer.header")[0])
        hh = h if isinstance(h, dict) else {}
        return {"undo bar rows": self.safe(lambda: [{k: v for k, v in r.items() if k != "ax"} for r in self.c.rows("mac-composer.undo.bar")]),
                "outbox entries": self.safe(self.entries), "composer line": hh.get("line", h), "to": hh.get("to"), "bcc": hh.get("bcc"),
                "body chars": self.safe(lambda: len(self.c.rows("mac-composer.body")[0]["text"])), "surface": self.safe(lambda: self.c.uistate().get("surface"))}

    def bar(self, tries=20):
        """The undo bar's row, waiting for it (tries x 0.1 s, bounded). A miss raises NoBar carrying composer_state."""
        for i in range(tries):
            rows = self.c.rows("mac-composer.undo.bar")
            if rows:
                return rows[0]
            if i < tries - 1:
                SLEEP(0.1)
        raise NoBar("no undo bar after %d reads over %.1f s: %s" % (tries, 0.1 * (tries - 1), self.composer_state()))

    def barred(self, fn):
        """fn(), or the evidence when the bar never came."""
        try:
            return fn()
        except NoBar as e:
            return {"no undo bar": str(e)}

    def explain(self, got, want, **evidence):
        """got when it is what was wanted; else the value with the evidence (callables, read now) beside it."""
        if got == want:
            return got
        return {"read": got, **{k: (v() if callable(v) else v) for k, v in evidence.items()}}


# ---------------------------------------------------------------- the promises: one per live binding

PROMISES = {}      # (label, command) -> (group, does, fn)


def P(chord, command, group, does):
    def deco(fn):
        PROMISES[(label_of(chord), command)] = (group, does, fn)
        return fn
    return deco


# --- list: the focus, the state of a conversation, the selection, the filters, the labels
@P("J", "next", "list", "move the list focus to the next conversation")
def _(k):
    k.home(); k.click("mac-main.list.row.0"); k.key("J")
    return (lambda: k.ui("focus_index")), 1


@P("K", "previous", "list", "move the list focus to the previous conversation")
def _(k):
    k.home(); k.click("mac-main.list.row.1"); k.key("K")
    return (lambda: k.ui("focus_index")), 0


@P("Down", "next", "list", "move the list focus down, in every preset")
def _(k):
    k.home(); k.click("mac-main.list.row.0"); k.key("Down")
    return (lambda: k.ui("focus_index")), 1


@P("Up", "previous", "list", "move the list focus up, in every preset")
def _(k):
    k.home(); k.click("mac-main.list.row.1"); k.key("Up")
    return (lambda: k.ui("focus_index")), 0


@P("E", "archive", "list", "archive the focused conversation")
def _(k):
    k.home(); k.focus("msg-04"); k.key("E")
    return (lambda: (k.core("message/msg-04/archived"), "msg-04" in k.ids())), (True, False)


@P("⇧E", "mark-not-done", "list", "bring an archived conversation back from the Archive view")
def _(k):
    k.home(); k.focus("msg-05"); k.key("E"); k.key("G"); k.key("E")
    pre = {}
    try:
        pre["in_archive_view"] = (k.lst("mailbox"), "msg-05" in k.ids())      # the positive control: the thread is there to bring back
    except Unreadable:
        pass
    k.focus("msg-05"); k.key("⇧E")
    return (lambda: (pre.get("in_archive_view"), "msg-05" in k.ids(), k.core("message/msg-05/archived"))), (("archive", True), False, False)


@P("H", "snooze", "list", "open the snooze picker on the focused conversation")
def _(k):
    k.home(); k.focus("msg-06"); k.key("H")
    return (lambda: k.lst("pending")), "snooze"


@P("S", "star", "list", "star the focused conversation")
def _(k):
    k.home(); k.focus("msg-08"); k.key("S")
    return (lambda: k.core("message/msg-08/starred")), True


@P("U", "unread", "list", "flip the focused conversation between read and unread")
def _(k):
    k.home(); k.focus("msg-12"); k.key("U")
    return (lambda: k.row("msg-12")["unread"]), False


@P("⇧#", "delete", "list", "delete the focused conversation")
def _(k):
    k.home(); k.focus("msg-02"); k.key("⇧#")
    return (lambda: k.core("message/msg-02/deleted")), True


@P("⇧!", "junk", "list", "mark the focused conversation as spam")
def _(k):
    k.home(); k.focus("msg-07"); k.key("⇧!")
    return (lambda: k.core("message/msg-07/spam")), True


@P("⇧M", "mute", "list", "mute the focused conversation")
def _(k):
    k.home(); k.focus("msg-06"); k.key("⇧M")
    return (lambda: k.core("message/msg-06/muted")), True


@P("L", "label", "list", "open the label picker on the focused conversation")
def _(k):
    k.home(); k.focus("msg-13"); k.key("L")
    return (lambda: k.lst("pending")), "label"


@P("V", "move", "list", "open the move picker on the focused conversation")
def _(k):
    k.home(); k.focus("msg-13"); k.key("V")
    return (lambda: k.ui("overlay")), "move"


@P("⇧P", "pin", "list", "pin the focused conversation")
def _(k):
    k.home(); k.focus("msg-11"); k.key("⇧P")
    return (lambda: k.core("pin/msg-11")), True


@P("Z", "undo", "list", "bring back what the last key took away")
def _(k):
    k.home(); k.focus("msg-14"); k.key("E")
    gone = {"archived": None}
    try:
        gone["archived"] = k.core("message/msg-14/archived")
    except Unreadable:
        pass
    k.key("Z")
    return (lambda: (gone["archived"], k.core("message/msg-14/archived"))), (True, False)


@P("X", "select-conversation", "list", "select the focused conversation")
def _(k):
    k.home(); k.focus("msg-09"); k.key("X")
    return (lambda: k.sel()), ["msg-09"]


@P("⇧J", "extend-selection-down", "list", "add the next conversation to the selection")
def _(k):
    k.home(); k.focus("msg-09"); k.key("X"); k.key("⇧J")
    i = {}
    return (lambda: (i.setdefault("ids", k.ids()), k.sel())[1] == i["ids"][k.row_of("msg-09"):k.row_of("msg-09") + 2]), True


@P("⇧K", "extend-selection-up", "list", "add the previous conversation to the selection")
def _(k):
    k.home(); k.click("mac-main.list.row.5"); k.key("⇧K")
    return (lambda: sorted(k.sel()) == sorted(k.ids()[4:6])), True


@P("Escape", "clear-selection", "list", "clear the selection and its bar")
def _(k):
    k.home(); k.focus("msg-09"); k.key("X")
    pre = {}
    try:
        pre["n"] = len(k.sel())
    except Unreadable:
        pass
    k.key("Escape")
    return (lambda: (pre.get("n"), len(k.sel()), k.lst("selection_bar"))), (1, 0, None)


@P("⌘A", "select-from-here", "list", "select every conversation from the focused one down")
def _(k):
    k.home(); k.click("mac-main.list.row.3"); k.key("⌘A")
    return (lambda: k.sel() == k.ids()[3:]), True


@P("⇧⌘A", "select-all", "list", "select every conversation")
def _(k):
    k.home(); k.key("⇧⌘A")
    return (lambda: k.sel() == k.ids()), True


@P("⇧U", "filter-unread", "list", "show only the unread conversations")
def _(k):
    k.home(); k.key("⇧U")
    return (lambda: (k.lst("filter"), all(r["unread"] for r in k.c.rows("mac-main.list")))), ("unread", True)


@P("⇧S", "filter-starred", "list", "show only the starred conversations")
def _(k):
    k.home(); k.key("⇧S")
    return (lambda: (k.lst("filter"), all(k.core("message/%s/starred" % i) is True for i in k.ids()))), ("starred", True)


@P("⇧I", "filter-important", "list", "show only the important conversations")
def _(k):
    k.home(); k.key("⇧I")
    return (lambda: k.lst("filter")), "important"


@P("⇧R", "filter-no-reply", "list", "show only the conversations nobody answered")
def _(k):
    k.home(); k.key("⇧R")
    return (lambda: k.lst("filter")), "no-reply"


def _label_removal(k, mid, chord, step):
    """Label a thread, press the key: the chip leaves and the focus steps (0 stay, +1 next, -1 previous)."""
    k.home(); k.focus(mid); k.key("L"); k.type("Suppliers"); k.key("Enter")
    here = {}
    try:
        here["ids"], here["at"] = k.ids(), k.row_of(mid)
        here["chip"] = "Suppliers" in str(k.core("label_assignment/%s" % mid))
    except Unreadable:
        pass
    k.focus(mid); k.key(chord)

    def read():
        want_focus = here["ids"][here["at"] + step] if step and here.get("ids") else mid
        return (here.get("chip"), "Suppliers" in str(k.core("label_assignment/%s" % mid)), k.ui("focus_message") == want_focus)
    return read, (True, False, True)


@P("Y", "remove-label", "list", "take the label in view off the focused conversation")
def _(k):
    return _label_removal(k, "msg-15", "Y", 0)


@P("[", "remove-label-next", "list", "take the label off and focus the next conversation")
def _(k):
    return _label_removal(k, "msg-12", "[", 1)


@P("]", "remove-label-previous", "list", "take the label off and focus the previous conversation")
def _(k):
    return _label_removal(k, "msg-13", "]", -1)


@P("⇧Y", "remove-all-labels", "list", "take every label off the focused conversation")
def _(k):
    return _label_removal(k, "msg-09", "⇧Y", 0)


@P("Tab", "next-split", "list", "open the next tab of the row")
def _(k):
    k.home(); k.key("Tab")
    return (lambda: k.ui("tab") == k.tabs()[1]), True


@P("Shift+Tab", "previous-split", "list", "open the previous tab of the row, in every preset")
def _(k):
    k.home(); k.key("Tab")
    pre = {}
    try:
        pre["moved"] = k.ui("tab") == k.tabs()[1]
    except Unreadable:
        pass
    k.key("Shift+Tab")
    return (lambda: (pre.get("moved"), k.ui("tab") == k.tabs()[0])), (True, True)


@P("Enter", "enter", "list", "open the focused conversation with its quick reply, the cursor in the reply field, as R does; reply all inside a thread")
def _(k):
    # A102 (owner: "when pressing enter on a message (it should open it and do the same thing as reply R)"): the list's Enter is R's act.
    k.home(); k.click("mac-main.list.row.0"); k.key("Enter")
    first = {}
    try:
        first["surface"] = k.ui("surface")
        # the cursor reaches the reply field on the run-loop turn after the key: read it settled, as r-reply does (main58 read "reader")
        first["focus"] = k.settle(lambda: k.ui("first_responder"), "quick-reply")
        first["field"] = k.settle(lambda: bool(k.c.rows("mac-main.quickreply.field")), True)
        first["resolved"] = k.c.rows("mac-shortcuts.dispatch")[-1]["resolved"]
    except Unreadable:
        pass
    # inside an open thread with no reply under it, Enter keeps Superhuman's meaning (reply all)
    k.reader(); k.key("Enter")
    # The reply box is read from its own row (the real app sets no overlay for it). The cursor in its field is read from first_responder, except on
    # a hidden rig: its windows are never the key window, so the field cannot take the keyboard there (the row is the proof then).
    hidden = bool(k.c.uistate().get("hidden", {}).get("on")) if hasattr(k.c, "uistate") else False
    return (lambda: (first.get("surface"), first.get("field"), hidden or first.get("focus") == "quick-reply", first.get("resolved"), k.dispatch()[0],
                     k.c.rows("mac-shortcuts.dispatch")[-1]["resolved"], bool(k.c.rows("mac-main.quickreply.field")))), \
        ("mac-reader", True, True, "reply", "enter", "reply-all", True)


# --- reader: the thread open on msg-01 (three messages, the last one marked)
@P("Escape", "back", "list", "leave the open thread for the list")   # a live command since the owner's 2026-11-13 ask: it is in the list group like the keys that act on the list
def _(k):
    k.reader(); k.key("Escape")
    return (lambda: k.ui("surface")), "mac-main"


@P("R", "reply", "reader", "mount the quick reply under the open thread")
def _(k):
    k.reader(); k.key("R")
    return (lambda: len(k.c.rows("mac-main.quickreply.field")) > 0), True


@P("N", "next-message", "reader", "mark the next message of the thread")
def _(k):
    k.reader(); k.key("P"); pre = {}
    try:
        pre["at"] = k.rd("focused_message")
    except Unreadable:
        pass
    k.key("N")
    return (lambda: (pre.get("at"), k.rd("focused_message"))), (1, 2)


@P("P", "previous-message", "reader", "mark the message before")
def _(k):
    k.reader(); k.key("P")
    return (lambda: (k.rd("focused_message"), k.rd("marked"))), (1, True)


@P("O", "expand-message", "reader", "unfold the marked message")
def _(k):
    k.reader(); k.key("P"); k.key("O")
    return (lambda: k.rd("expanded")), [1, 2]


@P("⇧O", "expand-all-messages", "reader", "unfold every message of the thread")
def _(k):
    k.reader(); k.key("⇧O")
    return (lambda: k.rd("expanded")), [0, 1, 2]


@P("⇧H", "toggle-message-header", "reader", "expand the marked message's header")
def _(k):
    k.reader(); k.collapse_headers(); k.key("P"); k.key("⇧H")
    return (lambda: k.header_open()), [False, True, False]


@P("⇧I", "toggle-colours", "reader", "switch the open mail between the app's colours and the sender's while the reader holds the focus; the list's ⇧I filters and leaves them")
def _(k):
    """A129 ruling (keys are contextual): ⇧I is the list's important filter with the LIST focused and the open mail's colour switch with the READER pane
    focused and the quick reply closed; the list state is no mail open (Escape from the reader), not a click on the list pane. Read from the body's `colours` value (mac-main.reader.body, as par-reader's colours_round does)."""
    colours = lambda: k.c.rows("mac-main.reader.body")[0]["colours"]
    k.reader()
    k.key("Escape")                                       # Escape in READER goes to LIST: with no mail open ⇧I is the filter
    before = colours()
    k.key("⇧I")
    in_list = (k.dispatch(), colours() == before)         # the filter ran and the colours did not move
    k.key("⇧I")                                           # the filter toggles back
    k.reader()                                            # the mail opens again: READER
    start = colours()
    k.click("mac-main.pane.reader")
    if k.c.rows("mac-main.quickreply.field"):             # the reply field the open leaves behind takes the key: close it, as par-reader does
        k.key("Escape")
    k.key("⇧I")
    return (lambda: (in_list, k.dispatch(), colours() != start)), ((("filter-important", "ran"), True), ("toggle-colours", "ran"), True)


def _tall_reader(k):
    k.reader(); k.key("⇧O"); k.key("R"); k.wait(200); k.key("Escape")


@P("Space", "scroll-down", "reader", "scroll the thread down a page")
def _(k):
    _tall_reader(k); k.key("Space"); k.wait(150)
    return (lambda: 0 < k.scroll()["offset"] <= k.scroll()["viewport"]), True


@P("⇧Space", "scroll-up", "reader", "scroll the thread back up a page")
def _(k):
    _tall_reader(k); k.key("Space"); k.wait(150)
    pre = {}
    try:
        pre["moved"] = k.scroll()["offset"] > 0
    except Unreadable:
        pass
    k.key("⇧Space"); k.wait(150)
    return (lambda: (pre.get("moved"), k.scroll()["offset"])), (True, 0)


@P("⌘Down", "jump-bottom", "reader", "jump to the end of the thread")
def _(k):
    _tall_reader(k); k.key("⌘Down"); k.wait(150)
    return (lambda: (k.scroll()["max_offset"] > 0, k.scroll()["offset"] == k.scroll()["max_offset"])), (True, True)


@P("⌘Up", "jump-top", "reader", "jump to the top of the thread")
def _(k):
    _tall_reader(k); k.key("⌘Down"); k.wait(150)
    pre = {}
    try:
        pre["off"] = k.scroll()["offset"] > 0
    except Unreadable:
        pass
    k.key("⌘Up"); k.wait(150)
    return (lambda: (pre.get("off"), k.scroll()["offset"])), (True, 0)


@P("⌘U", "unsubscribe", "reader", "ask before unsubscribing from a list message")
def _(k):
    mid = LISTS[0]["id"] if LISTS else "msg-09"
    k.reader(mid); k.key("⌘U")
    return (lambda: k.c.rows("mac-main.reader.unsubscribe")[0]["pending"]), True


@P("⌘O", "open-links", "reader", "open the first link of the message")
def _(k):
    k.corpus(); n0 = {}
    try:
        n0["n"] = len(k.c.rows("mac-main.reader.opened"))
    except Unreadable:
        pass
    k.click("mac-main.pane.reader")   # the reader holds the key: with the list focused the attachments' ⌘O answers first (it opens the chip, nothing is logged as a link)
    k.key("⌘O")
    return (lambda: [r["kind"] for r in k.c.rows("mac-main.reader.opened")][n0["n"]:][:1]), ["browser"]


@P("Tab", "cycle-links", "reader", "put the ring on the first link or date of the message")
def _(k):
    k.corpus(); k.key("Tab")
    return (lambda: (len(k.ring()) > 1, k.ring()[:1] == [True])), (True, True)


@P("Shift+Tab", "cycle-links-back", "reader", "move the ring back one link or date")
def _(k):
    k.corpus(); k.key("Tab"); k.key("Tab"); k.key("Shift+Tab")
    return (lambda: (len(k.ring()) > 1, k.ring()[:2] == [True, False])), (True, True)


def _popout(k, chord):
    k.reader(); base = {}
    try:
        base["n"] = k.windows()
    except Unreadable:
        pass
    k.key(chord)
    return (lambda: k.settle(lambda: k.windows() - base["n"], 1)), 1


@P("⇧R", "popout-reply", "composer", "pop the reply out into its own window")
def _(k):
    return _popout(k, "⇧R")


@P("⇧Enter", "popout-reply-all", "composer", "pop the reply-all out into its own window")
def _(k):
    return _popout(k, "⇧Enter")


@P("⇧C", "popout-new", "composer", "pop a new message out into its own window")
def _(k):
    return _popout(k, "⇧C")


@P("⇧F", "popout-forward", "composer", "pop the forward out into its own window")
def _(k):
    return _popout(k, "⇧F")


def _reader_with_draft(k):
    """The reader of msg-01 with words in the quick reply: open-full-composer IS the window, so a pop-out check starts from a draft in the reply box."""
    k.reader(); k.key("R"); k.wait(200); k.type("thanks, will do")


@P("⌘/", "popout-draft-search", "composer", "pop a draft out and open the search")
def _(k):
    _reader_with_draft(k); base = {}
    try:
        base["n"] = k.windows()
    except Unreadable:
        pass
    pre = {"frames": k.safe(lambda: [(f.get("id"), f.get("surface"), f.get("title")) for f in k.c.winframes()])}
    k.key("⌘/")
    want = (1, True)
    return (lambda: k.explain((k.settle(lambda: k.windows() - base.get("n", 0), 1), k.search_focused()), want, **{
        "windows before the key": base.get("n"), "windows now": lambda: k.safe(k.windows), "window frames before": pre["frames"],
        "window frames now": lambda: k.safe(lambda: [(f.get("id"), f.get("surface"), f.get("title")) for f in k.c.winframes()]),
        "surface": lambda: k.safe(lambda: k.c.uistate().get("surface")), "composer open": lambda: k.safe(lambda: k.c.rows("mac-composer.header")[0].get("is_open"))})), want


# --- composer: opened over msg-01 by the reader's full-composer control
@P("C", "compose", "composer", "open a new message")
def _(k):
    k.home(); k.key("C"); k.wait(250)
    return (lambda: k.composer_visible()), True


@P("F", "forward", "composer", "open a forward of the focused conversation")
def _(k):
    k.reader(); k.key("F"); k.wait(250)
    return (lambda: k.composer_visible()), True


@P("⇧⌘O", "compose-to", "composer", "put the cursor in To")
def _(k):
    k.composer(); k.key("⇧⌘O")
    return (lambda: k.hdr("focused")), "to"


@P("⇧⌘C", "compose-cc", "composer", "show Cc and put the cursor in it")
def _(k):
    k.composer(); k.key("⇧⌘C")
    return (lambda: (k.hdr("focused"), k.hdr("cc_visible"))), ("cc", True)


@P("⇧⌘B", "compose-bcc", "composer", "show Bcc and put the cursor in it")
def _(k):
    k.composer(); k.key("⇧⌘B")
    return (lambda: (k.hdr("focused"), k.hdr("bcc_visible"))), ("bcc", True)


@P("⇧⌘F", "compose-from", "composer", "open the sender list")
def _(k):
    k.composer(); k.key("⇧⌘F")
    return (lambda: (k.hdr("focused"), k.hdr("from_open"))), ("from", True)


@P("⇧⌘S", "edit-subject", "composer", "put the cursor in the subject")
def _(k):
    k.composer(); k.key("⇧⌘S")
    return (lambda: k.hdr("focused")), "subject"


@P("⇧⌘M", "edit-message", "composer", "put the cursor in the text")
def _(k):
    k.composer(); k.key("⇧⌘S"); pre = {}
    try:
        pre["at"] = k.hdr("focused")
    except Unreadable:
        pass
    k.key("⇧⌘M")
    return (lambda: (pre.get("at"), k.hdr("focused"))), ("subject", "body")


@P("⇧⌘,", "discard-draft", "composer", "ask before throwing the draft away")
def _(k):
    k.composer(); k.key("⇧⌘,")
    return (lambda: k.hdr("discard_ask")), True


@P("⇧⌘I", "move-contacts-to-bcc", "composer", "move the To contacts to Bcc")
def _(k):
    # a reply opens with the sender in To (docs/lanes/composermac.md round 4): the typed address joins it, and both move to Bcc
    k.composer(); k.key("⇧⌘O"); k.type("lea@example.com,"); pre = {}
    try:
        pre["to"], pre["bcc"] = k.hdr("to"), k.hdr("bcc")             # what the key moves: the sender is in To only if the reply opened with it
    except Unreadable:
        pass
    k.key("⇧⌘I")
    want = ([], [THREAD["from"], "lea@example.com"])
    return (lambda: k.explain((k.hdr("to"), k.hdr("bcc")), want, **{"To before the key": pre.get("to"), "Bcc before the key": pre.get("bcc"), "composer": k.composer_state})), want


@P("⇧⌘A", "attach", "composer", "open the attach panel")
def _(k):
    k.composer(); k.key("⇧⌘A")
    return (lambda: k.hdr("attach_open")), True


@P("⌘;", "use-snippet", "composer", "open the snippet picker")
def _(k):
    k.composer(); k.key("⌘;")
    return (lambda: len(k.c.rows("mac-composer.snippets")) >= 2), True


@P(";", "snippet-inline", "composer", "open the inline snippet list in the text")
def _(k):
    k.composer(); k.key("⇧⌘M"); k.wait(250); k.key(";")
    return (lambda: [r["kind"] for r in k.c.rows("mac-composer.inline")][:1]), ["snippet"]


def _format_key(k, chord, field, want):
    k.composer(); k.key("⇧⌘M"); k.wait(250); k.key(chord)
    return (lambda: k.fmt(field)), want


@P("⌘B", "bold", "composer", "turn bold on at the cursor")
def _(k):
    return _format_key(k, "⌘B", "bold", True)


@P("⌘I", "italic", "composer", "turn italic on at the cursor")
def _(k):
    return _format_key(k, "⌘I", "italic", True)


@P("⌘U", "underline", "composer", "turn underline on at the cursor")
def _(k):
    return _format_key(k, "⌘U", "underline", True)


@P("⇧⌘X", "strikethrough", "composer", "turn strikethrough on at the cursor")
def _(k):
    return _format_key(k, "⇧⌘X", "strike", True)


@P("⇧⌘8", "bullet-list", "composer", "make the line a bullet item")
def _(k):
    k.composer(); k.key("⇧⌘M"); k.wait(250); k.type("item"); k.key("⇧⌘8")
    return (lambda: k.fmt("list")), "bullet"


@P("⇧⌘7", "numbered-list", "composer", "make the line a numbered item")
def _(k):
    k.composer(); k.key("⇧⌘M"); k.wait(250); k.type("item"); k.key("⇧⌘7")
    return (lambda: k.fmt("list")), "number"


@P("⇧⌘9", "quote", "composer", "make the paragraph a quote")
def _(k):
    k.composer(); k.key("⇧⌘M"); k.wait(250); k.type("item"); k.key("⇧⌘9")
    return (lambda: k.fmt("quote")), True


@P("⌘]", "increase-indent", "composer", "indent the paragraph one level")
def _(k):
    k.composer(); k.key("⇧⌘M"); k.wait(250); k.type("item"); k.key("⌘]")
    return (lambda: k.fmt("indent")), 1


@P("⌘[", "decrease-indent", "composer", "take one indent level off")
def _(k):
    k.composer(); k.key("⇧⌘M"); k.wait(250); k.type("item"); k.key("⌘]")
    pre = {}
    try:
        pre["i"] = k.fmt("indent")
    except Unreadable:
        pass
    k.key("⌘[")
    return (lambda: (pre.get("i"), k.fmt("indent"))), (1, 0)


@P("Tab", "indent-list", "composer", "indent a list item")
def _(k):
    k.composer(); k.key("⇧⌘M"); k.wait(250); k.type("item"); k.key("⇧⌘8"); k.key("Tab")
    return (lambda: k.fmt("indent")), 1


@P("Shift+Tab", "outdent-list", "composer", "outdent a list item")
def _(k):
    k.composer(); k.key("⇧⌘M"); k.wait(250); k.type("item"); k.key("⇧⌘8"); k.key("Tab")
    pre = {}
    try:
        pre["i"] = k.fmt("indent")
    except Unreadable:
        pass
    k.key("Shift+Tab")
    return (lambda: (pre.get("i"), k.fmt("indent"))), (1, 0)


@P("⌘O", "color", "composer", "open the colour panel")
def _(k):
    k.composer(); k.key("⇧⌘M"); k.wait(250); k.key("⌘O")
    return (lambda: k.hdr("color_open")), True


@P("⌘K", "hyperlink", "composer", "open the link panel")
def _(k):
    k.composer(); k.key("⇧⌘M"); k.wait(250); k.key("⌘K")
    return (lambda: k.hdr("link_open")), True


@P("⇧⌘L", "send-later", "composer", "open the send-later choices")
def _(k):
    k.composer(); k.key("⇧⌘L")
    return (lambda: len(k.c.rows("mac-composer.send-later.options")) >= 2), True


@P("⇧⌘H", "remind-me", "composer", "ask to be reminded when nobody replies")
def _(k):
    k.composer(); k.key("⇧⌘H")
    return (lambda: k.hdr("remind_open")), True


@P("⌘Enter", "send", "composer", "send the message with an undo window")
def _(k):
    k.composer(); k.key("⇧⌘M"); k.wait(250); k.type("Sent by the key check."); n0 = {}
    try:
        n0["n"] = k.entries()
    except Unreadable:
        pass
    k.key("⌘Enter")
    return (lambda: k.barred(lambda: (k.bar()["undo_offered"], k.settle(lambda: k.entries() - n0["n"], 1)))), (True, 1)


@P("⇧⌘Enter", "send-and-done", "composer", "send the message and archive the conversation it answers")
def _(k):
    k.composer("msg-10"); k.key("⇧⌘M"); k.wait(250); k.type("Done."); n0 = {}
    try:
        n0["n"] = k.entries()
        n0["there"] = "msg-10" in k.ids()
    except Unreadable:
        pass
    k.key("⇧⌘Enter")
    want = (True, 1, False)
    # the bar, the outbox count and the list each settle (bounded) before they are read; a miss says which send the bar belongs to
    return (lambda: k.barred(lambda: k.explain((k.bar()["undo_offered"], k.settle(lambda: k.entries() - n0["n"], 1), k.settle(lambda: "msg-10" in k.ids(), False)), want,
                                               **{"msg-10 listed before the key": n0.get("there"), "outbox entries before the key": n0.get("n"), "composer": k.composer_state}))), want


@P("⇧⌘Z", "instant-send", "composer", "send now, with no undo window")
def _(k):
    k.composer("msg-13"); k.key("⇧⌘M"); k.wait(250); k.type("Now."); n0 = {}
    try:
        n0["n"] = k.entries()
    except Unreadable:
        pass
    k.key("⇧⌘Z")
    return (lambda: k.barred(lambda: (k.bar()["undo_offered"], k.settle(lambda: k.entries() - n0["n"], 1)))), (False, 1)


@P("⇧⌘P", "popout-draft", "composer", "pop the open draft out into its own window")
def _(k):
    _reader_with_draft(k); base = {}
    try:
        base["n"] = k.windows()
    except Unreadable:
        pass
    k.key("⇧⌘P")
    return (lambda: k.settle(lambda: k.windows() - base["n"], 1)), 1


@P("⌘D", "switch-draft", "composer", "leave the open draft for the list, and come back to it")
def _(k):
    k.composer(); k.key("⌘D")
    first = {}
    try:
        first["surface"] = k.ui("surface")
    except Unreadable:
        pass
    k.key("⌘D")
    return (lambda: (first.get("surface"), k.ui("surface"))), ("mac-main", "mac-composer")


# --- sidebar: go to a mailbox by two keys, and hide the sidebar
def _go(k, command, second):
    k.home(); k.key("G"); k.key(second)
    return (lambda: k.lst("mailbox")), MAILBOX_KEY[command]


for _cmd, _second, _cap in (("go-sent", "T", "Sent"), ("go-drafts", "D", "Drafts"), ("go-done", "E", "Done (Archive)"),
                            ("go-reminders", "H", "Reminders (Snoozed)"), ("go-spam", "⇧!", "Spam (Junk)"), ("go-trash", "⇧#", "Trash"),
                            ("go-starred", "S", "Starred"), ("go-muted", "M", "Muted"), ("go-all-mail", "A", "All Mail")):
    PROMISES[(label_of(("G", _second)), _cmd)] = ("sidebar", "show the %s mailbox in the list" % _cap, (lambda c, sec: lambda k: _go(k, c, sec))(_cmd, _second))


@P(("G", "I"), "go-inbox", "sidebar", "show the Primary view of the inbox")
def _(k):
    k.home(); k.key("G"); k.key("T"); pre = {}
    try:
        pre["box"] = k.lst("mailbox")
    except Unreadable:
        pass
    k.key("G"); k.key("I")
    return (lambda: (pre.get("box"), k.lst("mailbox"), k.ui("tab"))), ("sent", None, "primary")


@P(("G", "L"), "go-label", "sidebar", "focus the first label of the sidebar")
def _(k):
    k.home(); k.key("G"); k.key("L")
    return (lambda: str(k.c.rows("mac-main.sidebar.selection")[0]["id"]).endswith(str(k.c.rows("mac-main.sidebar.labels")[0]["id"]))), True


@P("⌘\\", "sidebar", "sidebar", "hide the sidebar, and show it again")
def _(k):
    k.home()
    pre = {}
    try:
        pre["h"] = k.c.rows("mac-main.sidebar.visibility")[0]["hidden"]
    except Unreadable:
        pass
    k.key("⌘\\")
    return (lambda: (pre.get("h"), k.c.rows("mac-main.sidebar.visibility")[0]["hidden"])), (False, True)


# --- window: the panes, new window, text size, the page link
@P("Left", "focus-left", "window", "move the keyboard focus one pane to the left")
def _(k):
    k.home(); k.raw("activate"); k.click("mac-main.pane.list"); k.key("Left")
    return (lambda: k.settle(lambda: k.shell("focused_pane"), "sidebar")), "sidebar"


@P("Right", "focus-right", "window", "move the keyboard focus one pane to the right")
def _(k):
    k.home(); k.raw("activate"); k.click("mac-main.pane.list"); k.key("Left"); pre = {}
    try:
        pre["pane"] = k.settle(lambda: k.shell("focused_pane"), "sidebar")
    except Unreadable:
        pass
    k.key("Right")
    return (lambda: (pre.get("pane"), k.settle(lambda: k.shell("focused_pane"), "list"))), ("sidebar", "list")


@P("⌘N", "new-window", "window", "open a second main window")
def _(k):
    k.home(); base = {}
    try:
        base["n"] = k.shell("main_windows")
    except Unreadable:
        pass
    k.key("⌘N")
    return (lambda: k.settle(lambda: k.shell("main_windows") - base["n"], 1)), 1


@P("⌘=", "font-bigger", "window", "make the text bigger")
def _(k):
    k.home(); k.key("⌘=")
    return (lambda: k.shell("text_scale") > 1), True


@P("⌘-", "font-smaller", "window", "make the text smaller")
def _(k):
    k.home(); k.key("⌘-")
    return (lambda: k.shell("text_scale") < 1), True


@P("⌘0", "font-reset", "window", "return the text to its own size")
def _(k):
    k.home(); k.key("⌘="); pre = {}
    try:
        pre["big"] = k.shell("text_scale") > 1
    except Unreadable:
        pass
    k.key("⌘0")
    return (lambda: (pre.get("big"), k.shell("text_scale"))), (True, 1)


@P("⌃/", "copy-page-link", "window", "copy the link of the focused conversation")
def _(k):
    k.home(); k.focus("msg-08"); k.key("⌃/")
    return (lambda: k.shell("last_link") == "whoosh://message/msg-08"), True


# --- find: the search, the palette
def _search(k, chord):
    k.home(); k.key(chord)
    return (lambda: k.search_focused()), True


@P("⌘K", "search", "find", "focus the search field")
def _(k):
    return _search(k, "⌘K")


@P("/", "search", "find", "focus the search field (second way in)")
def _(k):
    return _search(k, "/")


@P("⇧?", "search", "find", "focus the search field (second way in)")
def _(k):
    return _search(k, "⇧?")


@P("⌘P", "palette", "find", "open the command palette")
def _(k):
    k.home(); k.key("⌘P")
    return (lambda: k.ui("overlay")), "mac-palette"


# --- the AI reply: ⌘J is contextual. With a thread open it opens the reply under it and writes the draft (or asks for the instruction first);
# anywhere else the ai-reply handler says "not here" (QuickReplyModel.aiKey) and no reply opens.
@P("⌘J", "ai-reply", "aireply", "write the AI reply draft in an open thread, and open no reply from the plain list")
def _(k):
    ai = lambda: (k.c.rows("mac-main.quickreply.ai") or [{}])[0]
    k.home(); k.key("⌘J")
    outside = {}
    try:
        outside["reply"] = len(k.c.rows("mac-main.quickreply.field")) > 0
    except Unreadable:
        outside["reply"] = None
    k.reader(); k.key("⌘J")
    return (lambda: (outside.get("reply"), int(ai().get("writes", 0)) + (1 if ai().get("instruction_field") == "1" else 0) > 0)), (False, True)


# --- accounts: ⌃1 to ⌃9
def _digit(n):
    def fn(k):
        k.home(); k.key("⌃%d" % n)
        return (lambda: k.ui("account_filter")), ACCOUNT_IDS[n - 1]
    return fn


for _n in range(1, 10):
    PROMISES[("⌃%d" % _n, "switch-account-%d" % _n)] = ("accounts", "filter the list to account %d" % _n, _digit(_n))


# --- coming soon: the mark in the key list, nothing else
def soon_promise(b, feature):
    def fn(k):
        k.home()
        rows = k.c.rows("mac-shortcuts.map")
        hit = [r for r in rows if r["key"] == b["label"] and r["command"] == b["command"]]
        return (lambda: [(r["status"], r["feature"]) for r in hit]), [("coming-soon", feature)]
    return fn


# --- the digit row, pressed as an AZERTY keyboard presses it (SPEC A99): one check per bound digit chord
DIGIT_GROUP = "digits"
DIGIT_CODE = {"1": 18, "2": 19, "3": 20, "4": 21, "5": 23, "6": 22, "7": 26, "8": 28, "9": 25, "0": 29}   # the digit row's key codes, every layout
MOD_WORD = {"⌃": "ctrl", "⌥": "opt", "⇧": "shift", "⌘": "cmd"}


def digit_chords(inv):
    """The bindings whose chord is modifiers plus one digit, except Shift alone (that chord keeps the typed character)."""
    out = []
    for b in inv:
        if b["kind"] in ("sequence",):
            continue
        m = re.match(r"^([⌃⌥⇧⌘]+)([0-9])$", b["label"])
        if m and m.group(1) != "⇧":
            out.append(b)
    return out


def digit_slug(b):
    return "azerty-" + slug_of(b)


def digit_promise(b):
    """Press the digit as the physical key: the app must resolve the chord by the key (⌘ plus the "é" key is ⌘2), whatever the command then does.
    The outcome of a live digit chord (an account filter, the text size) is the owner group's check, pressed by chord text."""
    mods = [MOD_WORD[c] for c in b["label"][:-1]]
    code = DIGIT_CODE[b["label"][-1]]

    def fn(k):
        k.home()
        chord, _ran = k.keycode(code, mods)
        return (lambda: chord), b["label"]
    return fn


def drive_digits(j, inv):
    s = j.step("par-keys-%s" % DIGIT_GROUP, "Superhuman digit chords, pressed by key code as an AZERTY keyboard does")
    for b in digit_chords(inv):
        name = digit_slug(b)
        try:
            thunk, want = digit_promise(b)(K(s))
        except Unreadable as e:
            s.checks.append(Check("%s/%s" % (s.id, name), "unjudged", str(e)))
            continue
        except (KeyError, IndexError, TypeError, ValueError) as e:
            s.checks.append(Check("%s/%s" % (s.id, name), "fail", "reply shape: %s %s" % (type(e).__name__, e)))
            continue
        s.expect(name, thunk, want)


TEXT_GROUP = "text"
TEXT_CHECKS = [
    ("cmd-z-undoes-typed-text-in-the-composer", "⌘Z is taken by the engine before the composer body sees it, so the typed text stays"),
    ("cmd-a-selects-the-field-text-not-the-list", "⌘A is taken by the engine before the search field sees it: it selects the list's rows and the typed text is not replaced"),
    ("return-in-a-settings-field-does-not-reply", "Return in the Settings search field reaches the engine and replies to the selected mail"),
    ("a-text-view-in-a-non-key-window-keeps-its-keys", "a text view in a window that is not key loses Return and its letters to the engine (the monitor reads the key window)"),
]


def drive_text(j):
    """lead F18 (main74): a text view holding the focus takes the editing keys first. Every case presses REAL keys (`keyevent`:
    monitor, then window) and reads what the field holds; the engine's own reply to the same key is the mutant."""
    s = j.step("par-keys-%s" % TEXT_GROUP, "A focused text view takes its editing keys before the engine does")
    k = K(s)
    last = lambda: (k.c.rows("mac-shortcuts.dispatch") or [{"n": 0}])[-1]["n"]
    body = lambda: k.c.rows("mac-composer.body")[0]["text"]
    bar = lambda: k.c.rows("mac-search.bar")[0]["text"]
    query = lambda: k.c.rows("mac-settings-window.place")[0]["query"]

    def case(name, setup, read, want):
        k.reports.clear()
        try:
            thunk = setup()
        except Unreadable as e:
            s.checks.append(Check("%s/%s" % (s.id, name), "unjudged", str(e)))
            return
        expect_why(s, name, thunk, want, k.what_the_app_said)

    def undo():
        k.composer(); k.key("⇧⌘M"); k.wait(250); k.real("q"); k.real("cmd+z")
        return body
    case(TEXT_CHECKS[0][0], undo, None, "")

    def select_all():
        k.home(); k.key("⌘K"); k.wait(250); k.real("abc"); k.real("cmd+a"); k.real("x")
        return lambda: (bar(), len(k.sel()))
    case(TEXT_CHECKS[1][0], select_all, None, ("x", 0))

    def settings_open():
        """Wait (bounded: 40 reads, 0.1 s apart) for the Settings window after ⌘, : `winframes` lists a window whose id starts with `mac-settings`.
        None when it is open; else the sentence and the window list (main79 and main81: no Settings window existed when the key arrived, and the typed
        letter went to the main window)."""
        def open_now():
            # the WINDOW, by the id the app gives it (WindowBridge.settingsWindowId), never by its title (the app's language): the place row is
            # served by the model whether or not the window exists, so main81 judged Return in a field that was not there
            return any(str(f.get("id", "")).startswith("mac-settings") for f in k.c.winframes())
        for i in range(40):
            if k.safe(open_now) is True:
                return None
            if i < 39:
                SLEEP(0.1)
        return "Settings did not open: place row %s; windows %s" % (k.safe(lambda: k.c.rows("mac-settings-window.place")),
                                                                    k.safe(lambda: [(f.get("id"), f.get("title"), f.get("kind")) for f in k.c.winframes()]))

    def settings_focus():
        """Open Settings and focus its search field; the reason it did not open, or None."""
        k.home(); k.focus("msg-01"); k.key("⌘,"); k.wait(400)
        why = settings_open()
        if why is None:
            k.key("⌘F"); k.wait(200)
        return why

    def settings_return():
        why = settings_focus()
        if why:
            return lambda: why
        k.real("q"); n0 = last(); k.real("return")
        return lambda: (query(), last() - n0, k.ui("overlay"))
    case(TEXT_CHECKS[2][0], settings_return, None, ("q", 0, None))

    def non_key():
        why = settings_focus()
        if why:
            return lambda: why
        k.real("q"); n0 = last(); k.real("y", "c"); k.real("return", "c")
        return lambda: (query(), last() - n0, k.ui("overlay"))
    case(TEXT_CHECKS[3][0], non_key, None, ("qy", 0, None))
    try:
        k.key("Escape"); k.key("Escape")
    except Unreadable:
        pass


# ---------------------------------------------------------------- the run

def promise_for(b, cmds):
    if b["status"] == "soon":
        return ("soon", "show the coming-soon mark for %s" % cmds[b["command"]][2], soon_promise(b, cmds[b["command"]][2]))
    return PROMISES.get(ident(b))


def coverage(inv=None, cmds=None):
    """(no_check, stale): bindings of the source with no check, and checks for bindings the source no longer holds."""
    cmds = cmds or V8.commands()
    inv = inv if inv is not None else inventory(cmds=cmds)
    no_check = [b["label"] + " " + b["command"] for b in inv if promise_for(b, cmds) is None]
    held = {ident(b) for b in inv}
    stale = sorted("%s %s" % k for k in PROMISES if k not in held)
    wrong_group = [b["label"] + " " + b["command"] for b in inv if promise_for(b, cmds) and promise_for(b, cmds)[0] != b["group"]]
    return no_check, stale, wrong_group


def order(inv):
    """Corpus-loading checks run last: the app keeps the corpus for its life."""
    last = {("Tab", "cycle-links"), ("Shift+Tab", "cycle-links-back"), ("⌘O", "open-links")}
    return sorted(inv, key=lambda b: (ident(b) in last, GROUPS.index(b["group"])))


def drive(j):
    cmds = V8.commands()
    inv = inventory(cmds=cmds)
    steps = {g: j.step("par-keys-%s" % g, "Superhuman keys, group %s" % g) for g in GROUPS}
    for b in order(inv):
        got = promise_for(b, cmds)
        s = steps[b["group"]]
        name = slug_of(b)
        if got is None:
            s.checks.append(Check("%s/%s" % (s.id, name), "fail", "no check for this binding of the source"))
            continue
        _, _does, fn = got
        try:
            thunk, want = fn(K(s))
        except Unreadable as e:
            s.checks.append(Check("%s/%s" % (s.id, name), "unjudged", str(e)))
            continue
        except (KeyError, IndexError, TypeError, ValueError) as e:
            s.checks.append(Check("%s/%s" % (s.id, name), "fail", "reply shape: %s %s" % (type(e).__name__, e)))
            continue
        s.expect(name, thunk, want)
    drive_digits(j, inv)
    drive_text(j)


def mutant_lines():
    """The `# MUTANTS` block, one line per binding of the source: the file keeps it in step (see --write-mutants)."""
    cmds = V8.commands()
    out = []
    for b in order(inventory(cmds=cmds)):
        got = promise_for(b, cmds)
        if got is None:
            continue
        group, does, _ = got
        text = ("the key is dead or the list does not carry the mark" if group == "soon"
                else "%s does nothing, or does not %s" % (b["label"], does))
        out.append("# par-keys-%s/%s | %s" % (group, slug_of(b), text))
    for b in digit_chords(inventory(cmds=cmds)):
        out.append("# par-keys-%s/%s | the key code is read by the character the layout types (\"é\" for the 2 key), so %s resolves to another chord or none"
                   % (DIGIT_GROUP, digit_slug(b), b["label"]))
    for name, why in TEXT_CHECKS:
        out.append("# par-keys-%s/%s | %s" % (TEXT_GROUP, name, why))
    return out


def report(r, rows, extra=()):
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
    for line in extra:
        sys.stderr.write(line + "\n")
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))


def main():
    path = os.path.abspath(__file__)
    if "--write-mutants" in sys.argv:
        return write_mutants(path)
    if "--selftest" in sys.argv:
        return selftest(sys.modules[__name__], path)
    no_check, stale, wrong_group = coverage()
    for t, rows in (("NO CHECK", no_check), ("STALE CHECK", stale), ("WRONG GROUP", wrong_group)):
        for x in rows:
            sys.stderr.write("%s: %s\n" % (t, x))
    if "--coverage" in sys.argv:
        inv = inventory()
        by = {g: sum(1 for b in inv if b["group"] == g) for g in GROUPS}
        print("par-keys coverage: bindings=%d %s no_check=%d stale=%d wrong_group=%d" % (
            len(inv), " ".join("%s=%d" % kv for kv in by.items()), len(no_check), len(stale), len(wrong_group)))
        return 0 if not (no_check or stale or wrong_group) else 1
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    print_cases(r)
    rows = [l[1:].strip().split(" | ", 1) for l in MUTANTS.splitlines() if l.startswith("# par-keys") and " | " in l]
    report(r, rows)
    ok = (r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"]
          and not (no_check or stale or wrong_group))
    return 0 if ok else 1


def write_mutants(path):
    """Rewrite the MUTANTS block between its markers from the source's bindings."""
    text = open(path, encoding="utf-8").read()
    a = text.index('\nMUTANTS = r"""') + len('\nMUTANTS = r"""')
    b = text.index('"""', a)
    block = "\n# MUTANTS: per check, the mutation of the app that must turn it red.\n" + "\n".join(mutant_lines()) + "\n"
    open(path, "w", encoding="utf-8").write(text[:a] + block + text[b:])
    print("mutants written: %d" % len(mutant_lines()))
    return 0


MUTANTS = r"""
# MUTANTS: per check, the mutation of the app that must turn it red.
# par-keys-list/z-undo | Z does nothing, or does not bring back what the last key took away
# par-keys-list/x-select-conversation | X does nothing, or does not select the focused conversation
# par-keys-list/shift-j-extend-selection-down | ⇧J does nothing, or does not add the next conversation to the selection
# par-keys-list/shift-k-extend-selection-up | ⇧K does nothing, or does not add the previous conversation to the selection
# par-keys-list/cmd-a-select-from-here | ⌘A does nothing, or does not select every conversation from the focused one down
# par-keys-list/shift-cmd-a-select-all | ⇧⌘A does nothing, or does not select every conversation
# par-keys-list/e-archive | E does nothing, or does not archive the focused conversation
# par-keys-list/shift-e-mark-not-done | ⇧E does nothing, or does not bring an archived conversation back from the Archive view
# par-keys-list/h-snooze | H does nothing, or does not open the snooze picker on the focused conversation
# par-keys-list/s-star | S does nothing, or does not star the focused conversation
# par-keys-list/u-unread | U does nothing, or does not flip the focused conversation between read and unread
# par-keys-list/shift-hash-delete | ⇧# does nothing, or does not delete the focused conversation
# par-keys-list/shift-bang-junk | ⇧! does nothing, or does not mark the focused conversation as spam
# par-keys-list/shift-m-mute | ⇧M does nothing, or does not mute the focused conversation
# par-keys-list/enter-enter | Enter does nothing, or does not open the focused conversation with its quick reply, the cursor in the reply field, as R does; reply all inside a thread
# par-keys-list/l-label | L does nothing, or does not open the label picker on the focused conversation
# par-keys-list/y-remove-label | Y does nothing, or does not take the label in view off the focused conversation
# par-keys-list/lbracket-remove-label-next | [ does nothing, or does not take the label off and focus the next conversation
# par-keys-list/rbracket-remove-label-previous | ] does nothing, or does not take the label off and focus the previous conversation
# par-keys-list/shift-y-remove-all-labels | ⇧Y does nothing, or does not take every label off the focused conversation
# par-keys-list/v-move | V does nothing, or does not open the move picker on the focused conversation
# par-keys-list/shift-u-filter-unread | ⇧U does nothing, or does not show only the unread conversations
# par-keys-list/shift-s-filter-starred | ⇧S does nothing, or does not show only the starred conversations
# par-keys-list/shift-i-filter-important | ⇧I does nothing, or does not show only the important conversations
# par-keys-reader/shift-i-toggle-colours | ⇧I does nothing, or ⇧I filters while a mail is open, or the reader's ⇧I does not change the open mail's colours
# par-keys-list/shift-r-filter-no-reply | ⇧R does nothing, or does not show only the conversations nobody answered
# par-keys-list/tab-next-split | Tab does nothing, or does not open the next tab of the row
# par-keys-list/j-next | J does nothing, or does not move the list focus to the next conversation
# par-keys-list/k-previous | K does nothing, or does not move the list focus to the previous conversation
# par-keys-list/shift-p-pin | ⇧P does nothing, or does not pin the focused conversation
# par-keys-list/escape-clear-selection | Escape does nothing, or does not clear the selection and its bar
# par-keys-list/shift-tab-previous-split | Shift+Tab does nothing, or does not open the previous tab of the row, in every preset
# par-keys-list/down-next | Down does nothing, or does not move the list focus down, in every preset
# par-keys-list/up-previous | Up does nothing, or does not move the list focus up, in every preset
# par-keys-list/escape-back | Escape does nothing, or does not leave the open thread for the list
# par-keys-reader/cmd-u-unsubscribe | ⌘U does nothing, or does not ask before unsubscribing from a list message
# par-keys-reader/r-reply | R does nothing, or does not mount the quick reply under the open thread
# par-keys-reader/o-expand-message | O does nothing, or does not unfold the marked message
# par-keys-reader/shift-o-expand-all-messages | ⇧O does nothing, or does not unfold every message of the thread
# par-keys-reader/shift-h-toggle-message-header | ⇧H does nothing, or does not expand the marked message's header
# par-keys-reader/n-next-message | N does nothing, or does not mark the next message of the thread
# par-keys-reader/p-previous-message | P does nothing, or does not mark the message before
# par-keys-reader/space-scroll-down | Space does nothing, or does not scroll the thread down a page
# par-keys-reader/shift-space-scroll-up | ⇧Space does nothing, or does not scroll the thread back up a page
# par-keys-reader/cmd-up-jump-top | ⌘Up does nothing, or does not jump to the top of the thread
# par-keys-reader/cmd-down-jump-bottom | ⌘Down does nothing, or does not jump to the end of the thread
# par-keys-composer/c-compose | C does nothing, or does not open a new message
# par-keys-composer/f-forward | F does nothing, or does not open a forward of the focused conversation
# par-keys-composer/cmd-b-bold | ⌘B does nothing, or does not turn bold on at the cursor
# par-keys-composer/cmd-i-italic | ⌘I does nothing, or does not turn italic on at the cursor
# par-keys-composer/shift-cmd-x-strikethrough | ⇧⌘X does nothing, or does not turn strikethrough on at the cursor
# par-keys-composer/shift-cmd-7-numbered-list | ⇧⌘7 does nothing, or does not make the line a numbered item
# par-keys-composer/shift-cmd-8-bullet-list | ⇧⌘8 does nothing, or does not make the line a bullet item
# par-keys-composer/shift-cmd-9-quote | ⇧⌘9 does nothing, or does not make the paragraph a quote
# par-keys-composer/cmd-rbracket-increase-indent | ⌘] does nothing, or does not indent the paragraph one level
# par-keys-composer/cmd-lbracket-decrease-indent | ⌘[ does nothing, or does not take one indent level off
# par-keys-composer/shift-cmd-o-compose-to | ⇧⌘O does nothing, or does not put the cursor in To
# par-keys-composer/shift-cmd-c-compose-cc | ⇧⌘C does nothing, or does not show Cc and put the cursor in it
# par-keys-composer/shift-cmd-b-compose-bcc | ⇧⌘B does nothing, or does not show Bcc and put the cursor in it
# par-keys-composer/shift-cmd-f-compose-from | ⇧⌘F does nothing, or does not open the sender list
# par-keys-composer/shift-cmd-s-edit-subject | ⇧⌘S does nothing, or does not put the cursor in the subject
# par-keys-composer/shift-cmd-m-edit-message | ⇧⌘M does nothing, or does not put the cursor in the text
# par-keys-composer/shift-cmd-comma-discard-draft | ⇧⌘, does nothing, or does not ask before throwing the draft away
# par-keys-composer/shift-cmd-i-move-contacts-to-bcc | ⇧⌘I does nothing, or does not move the To contacts to Bcc
# par-keys-composer/shift-cmd-h-remind-me | ⇧⌘H does nothing, or does not ask to be reminded when nobody replies
# par-keys-composer/shift-cmd-l-send-later | ⇧⌘L does nothing, or does not open the send-later choices
# par-keys-composer/semicolon-snippet-inline | ; does nothing, or does not open the inline snippet list in the text
# par-keys-composer/cmd-semicolon-use-snippet | ⌘; does nothing, or does not open the snippet picker
# par-keys-composer/cmd-enter-send | ⌘Enter does nothing, or does not send the message with an undo window
# par-keys-composer/shift-cmd-enter-send-and-done | ⇧⌘Enter does nothing, or does not send the message and archive the conversation it answers
# par-keys-composer/shift-cmd-z-instant-send | ⇧⌘Z does nothing, or does not send now, with no undo window
# par-keys-composer/shift-cmd-p-popout-draft | ⇧⌘P does nothing, or does not pop the open draft out into its own window
# par-keys-composer/shift-enter-popout-reply-all | ⇧Enter does nothing, or does not pop the reply-all out into its own window
# par-keys-composer/shift-c-popout-new | ⇧C does nothing, or does not pop a new message out into its own window
# par-keys-composer/shift-f-popout-forward | ⇧F does nothing, or does not pop the forward out into its own window
# par-keys-composer/cmd-slash-popout-draft-search | ⌘/ does nothing, or does not pop a draft out and open the search
# par-keys-composer/cmd-d-switch-draft | ⌘D does nothing, or does not leave the open draft for the list, and come back to it
# par-keys-composer/shift-cmd-a-attach | ⇧⌘A does nothing, or does not open the attach panel
# par-keys-composer/tab-indent-list | Tab does nothing, or does not indent a list item
# par-keys-composer/shift-tab-outdent-list | Shift+Tab does nothing, or does not outdent a list item
# par-keys-composer/cmd-k-hyperlink | ⌘K does nothing, or does not open the link panel
# par-keys-composer/cmd-o-color | ⌘O does nothing, or does not open the colour panel
# par-keys-composer/cmd-u-underline | ⌘U does nothing, or does not turn underline on at the cursor
# par-keys-composer/shift-r-popout-reply | ⇧R does nothing, or does not pop the reply out into its own window
# par-keys-sidebar/cmd-backslash-sidebar | ⌘\ does nothing, or does not hide the sidebar, and show it again
# par-keys-sidebar/g-then-i-go-inbox | G then I does nothing, or does not show the Primary view of the inbox
# par-keys-sidebar/g-then-s-go-starred | G then S does nothing, or does not show the Starred mailbox in the list
# par-keys-sidebar/g-then-d-go-drafts | G then D does nothing, or does not show the Drafts mailbox in the list
# par-keys-sidebar/g-then-t-go-sent | G then T does nothing, or does not show the Sent mailbox in the list
# par-keys-sidebar/g-then-e-go-done | G then E does nothing, or does not show the Done (Archive) mailbox in the list
# par-keys-sidebar/g-then-h-go-reminders | G then H does nothing, or does not show the Reminders (Snoozed) mailbox in the list
# par-keys-sidebar/g-then-m-go-muted | G then M does nothing, or does not show the Muted mailbox in the list
# par-keys-sidebar/g-then-bang-go-spam | G then ! does nothing, or does not show the Spam (Junk) mailbox in the list
# par-keys-sidebar/g-then-hash-go-trash | G then # does nothing, or does not show the Trash mailbox in the list
# par-keys-sidebar/g-then-a-go-all-mail | G then A does nothing, or does not show the All Mail mailbox in the list
# par-keys-sidebar/g-then-l-go-label | G then L does nothing, or does not focus the first label of the sidebar
# par-keys-window/left-focus-left | Left does nothing, or does not move the keyboard focus one pane to the left
# par-keys-window/right-focus-right | Right does nothing, or does not move the keyboard focus one pane to the right
# par-keys-window/cmd-n-new-window | ⌘N does nothing, or does not open a second main window
# par-keys-window/cmd-equals-font-bigger | ⌘= does nothing, or does not make the text bigger
# par-keys-window/cmd-minus-font-smaller | ⌘- does nothing, or does not make the text smaller
# par-keys-window/cmd-0-font-reset | ⌘0 does nothing, or does not return the text to its own size
# par-keys-window/ctrl-slash-copy-page-link | ⌃/ does nothing, or does not copy the link of the focused conversation
# par-keys-find/cmd-k-search | ⌘K does nothing, or does not focus the search field
# par-keys-find/cmd-p-palette | ⌘P does nothing, or does not open the command palette
# par-keys-find/slash-search | / does nothing, or does not focus the search field (second way in)
# par-keys-find/shift-question-search | ⇧? does nothing, or does not focus the search field (second way in)
# par-keys-accounts/ctrl-1-switch-account-1 | ⌃1 does nothing, or does not filter the list to account 1
# par-keys-accounts/ctrl-2-switch-account-2 | ⌃2 does nothing, or does not filter the list to account 2
# par-keys-accounts/ctrl-3-switch-account-3 | ⌃3 does nothing, or does not filter the list to account 3
# par-keys-accounts/ctrl-4-switch-account-4 | ⌃4 does nothing, or does not filter the list to account 4
# par-keys-accounts/ctrl-5-switch-account-5 | ⌃5 does nothing, or does not filter the list to account 5
# par-keys-accounts/ctrl-6-switch-account-6 | ⌃6 does nothing, or does not filter the list to account 6
# par-keys-accounts/ctrl-7-switch-account-7 | ⌃7 does nothing, or does not filter the list to account 7
# par-keys-accounts/ctrl-8-switch-account-8 | ⌃8 does nothing, or does not filter the list to account 8
# par-keys-accounts/ctrl-9-switch-account-9 | ⌃9 does nothing, or does not filter the list to account 9
# par-keys-aireply/cmd-j-ai-reply | ⌘J does nothing, or does not write the AI reply draft in an open thread, and open no reply from the plain list
# par-keys-soon/0-cal-day | the key is dead or the list does not carry the mark
# par-keys-soon/t-cal-today | the key is dead or the list does not carry the mark
# par-keys-soon/minus-cal-previous | the key is dead or the list does not carry the mark
# par-keys-soon/equals-cal-next | the key is dead or the list does not carry the mark
# par-keys-soon/b-cal-create-event | the key is dead or the list does not carry the mark
# par-keys-soon/cmd-t-new-tab | the key is dead or the list does not carry the mark
# par-keys-soon/shift-cmd-rbracket-next-tab | the key is dead or the list does not carry the mark
# par-keys-soon/shift-cmd-lbracket-previous-tab | the key is dead or the list does not carry the mark
# par-keys-soon/cmd-w-close-tab | the key is dead or the list does not carry the mark
# par-keys-soon/cmd-1-switch-tab-1 | the key is dead or the list does not carry the mark
# par-keys-soon/cmd-2-switch-tab-2 | the key is dead or the list does not carry the mark
# par-keys-soon/cmd-3-switch-tab-3 | the key is dead or the list does not carry the mark
# par-keys-soon/cmd-4-switch-tab-4 | the key is dead or the list does not carry the mark
# par-keys-soon/cmd-5-switch-tab-5 | the key is dead or the list does not carry the mark
# par-keys-soon/cmd-6-switch-tab-6 | the key is dead or the list does not carry the mark
# par-keys-soon/cmd-7-switch-tab-7 | the key is dead or the list does not carry the mark
# par-keys-soon/cmd-8-switch-tab-8 | the key is dead or the list does not carry the mark
# par-keys-soon/cmd-9-switch-tab-9 | the key is dead or the list does not carry the mark
# par-keys-soon/p-cal-previous | the key is dead or the list does not carry the mark
# par-keys-soon/n-cal-next | the key is dead or the list does not carry the mark
# par-keys-soon/g-then-o-go-other | the key is dead or the list does not carry the mark
# par-keys-soon/g-then-semicolon-go-snippets | the key is dead or the list does not carry the mark
# par-keys-soon/0-then-0-cal-week | the key is dead or the list does not carry the mark
# par-keys-reader/cmd-o-open-links | ⌘O does nothing, or does not open the first link of the message
# par-keys-reader/tab-cycle-links | Tab does nothing, or does not put the ring on the first link or date of the message
# par-keys-reader/shift-tab-cycle-links-back | Shift+Tab does nothing, or does not move the ring back one link or date
# par-keys-digits/azerty-shift-cmd-7-numbered-list | the key code is read by the character the layout types ("é" for the 2 key), so ⇧⌘7 resolves to another chord or none
# par-keys-digits/azerty-shift-cmd-8-bullet-list | the key code is read by the character the layout types ("é" for the 2 key), so ⇧⌘8 resolves to another chord or none
# par-keys-digits/azerty-shift-cmd-9-quote | the key code is read by the character the layout types ("é" for the 2 key), so ⇧⌘9 resolves to another chord or none
# par-keys-digits/azerty-cmd-0-font-reset | the key code is read by the character the layout types ("é" for the 2 key), so ⌘0 resolves to another chord or none
# par-keys-digits/azerty-ctrl-1-switch-account-1 | the key code is read by the character the layout types ("é" for the 2 key), so ⌃1 resolves to another chord or none
# par-keys-digits/azerty-ctrl-2-switch-account-2 | the key code is read by the character the layout types ("é" for the 2 key), so ⌃2 resolves to another chord or none
# par-keys-digits/azerty-ctrl-3-switch-account-3 | the key code is read by the character the layout types ("é" for the 2 key), so ⌃3 resolves to another chord or none
# par-keys-digits/azerty-ctrl-4-switch-account-4 | the key code is read by the character the layout types ("é" for the 2 key), so ⌃4 resolves to another chord or none
# par-keys-digits/azerty-ctrl-5-switch-account-5 | the key code is read by the character the layout types ("é" for the 2 key), so ⌃5 resolves to another chord or none
# par-keys-digits/azerty-ctrl-6-switch-account-6 | the key code is read by the character the layout types ("é" for the 2 key), so ⌃6 resolves to another chord or none
# par-keys-digits/azerty-ctrl-7-switch-account-7 | the key code is read by the character the layout types ("é" for the 2 key), so ⌃7 resolves to another chord or none
# par-keys-digits/azerty-ctrl-8-switch-account-8 | the key code is read by the character the layout types ("é" for the 2 key), so ⌃8 resolves to another chord or none
# par-keys-digits/azerty-ctrl-9-switch-account-9 | the key code is read by the character the layout types ("é" for the 2 key), so ⌃9 resolves to another chord or none
# par-keys-digits/azerty-cmd-1-switch-tab-1 | the key code is read by the character the layout types ("é" for the 2 key), so ⌘1 resolves to another chord or none
# par-keys-digits/azerty-cmd-2-switch-tab-2 | the key code is read by the character the layout types ("é" for the 2 key), so ⌘2 resolves to another chord or none
# par-keys-digits/azerty-cmd-3-switch-tab-3 | the key code is read by the character the layout types ("é" for the 2 key), so ⌘3 resolves to another chord or none
# par-keys-digits/azerty-cmd-4-switch-tab-4 | the key code is read by the character the layout types ("é" for the 2 key), so ⌘4 resolves to another chord or none
# par-keys-digits/azerty-cmd-5-switch-tab-5 | the key code is read by the character the layout types ("é" for the 2 key), so ⌘5 resolves to another chord or none
# par-keys-digits/azerty-cmd-6-switch-tab-6 | the key code is read by the character the layout types ("é" for the 2 key), so ⌘6 resolves to another chord or none
# par-keys-digits/azerty-cmd-7-switch-tab-7 | the key code is read by the character the layout types ("é" for the 2 key), so ⌘7 resolves to another chord or none
# par-keys-digits/azerty-cmd-8-switch-tab-8 | the key code is read by the character the layout types ("é" for the 2 key), so ⌘8 resolves to another chord or none
# par-keys-digits/azerty-cmd-9-switch-tab-9 | the key code is read by the character the layout types ("é" for the 2 key), so ⌘9 resolves to another chord or none
# par-keys-text/cmd-z-undoes-typed-text-in-the-composer | ⌘Z is taken by the engine before the composer body sees it, so the typed text stays
# par-keys-text/cmd-a-selects-the-field-text-not-the-list | ⌘A is taken by the engine before the search field sees it: it selects the list's rows and the typed text is not replaced
# par-keys-text/return-in-a-settings-field-does-not-reply | Return in the Settings search field reaches the engine and replies to the selected mail
# par-keys-text/a-text-view-in-a-non-key-window-keeps-its-keys | a text view in a window that is not key loses Return and its letters to the engine (the monitor reads the key window)
"""


# ---------------------------------------------------------------- the model of the preset, for --selftest
# A stand-in app that reads the REAL preset source (contexts first, then the row, then the fixed keys; a sequence waits for
# its second key) and runs each command the way its screen promises. Its handlers are written apart from the promises above:
# a check and the model that agree are two descriptions that agree. A mutant breaks one screen group (a dead key) or makes
# one command do the wrong thing; the checks of that group, and only that group, must turn red.

class Model:
    FIRST_LABEL = "label-1"
    TABS = ["primary", "all", "provanta"]

    def __init__(self, mutant=None, preset=None, commands=None, inv=None):
        self.mut = mutant                      # None | ("dead", group) | ("wrong", name)
        self.p = preset or V8.preset()
        self.cmds = commands or V8.commands()
        self.cgroup = {b["command"]: b["group"] for b in (inv if inv is not None else inventory(self.p, self.cmds))}
        self.msgs = {m["id"]: dict(archived=False, deleted=False, spam=False, muted=False, starred=False, unread=bool(m["unread"]),
                                   pinned=bool(m["pinned"]), labels=set(), thread=m["thread_count"], lists=bool(m.get("list_unsubscribe")))
                     for m in ROWS}
        self.order = [m["id"] for m in ROWS if m["pinned"]] + [m["id"] for m in ROWS if not m["pinned"]]
        self.surface, self.overlay, self.mailbox, self.tab, self.account = "mac-main", None, None, 0, None
        self.focus_id, self.sel, self.filter, self.pending, self.typed = self.order[0], [], None, None, ""
        self.armed, self.log, self.notice, self.n = None, [], None, 0
        self.rd = None
        self.corpus = False
        self.c = None
        self.entries, self.popouts, self.main_windows, self.scale = 0, 0, 1, 1.0
        self.qr_text = ""
        self.pane, self.sidebar_hidden, self.search, self.last_link, self.opened, self.undo_stack = "list", False, False, None, [], []
        self.sidebar_sel = None
        self.settings_open, self.settings_field, self.settings_query = False, False, ""
        self.bar, self.select_all, self.hist = "", False, []

    # -- real keys (the `keyevent` verb): the shell's monitor, then the window
    def text_focus(self):
        if self.settings_open:
            return "settings" if self.settings_field else None
        if self.composer_open() and self.c["focused"] == "body":
            return "body"
        return "search" if self.search else None

    @staticmethod
    def owned_by_text(chord):
        """The model's own wording of the table in TextKeys.swift (written apart from it): typing and the field's own commands, the six editing chords."""
        if chord == "Escape":
            return False
        return not any(m in chord for m in "⌃⌥⌘") or chord in ("⌘Z", "⇧⌘Z", "⌘A", "⌘X", "⌘C", "⌘V")

    def real_key(self, chord, key_window):
        where = self.text_focus()
        # mutant text-key-window: the monitor asks the KEY window (ChordKit.isTyping before F18), so a field in another window is not typing
        typing = where is not None and (key_window or not self.wrong("text-key-window"))
        # mutants: text-keys-to-engine = the old monitor, every ⌘ chord goes to the engine first; text-return-to-engine = Return does too
        mod = any(m in chord for m in "⌃⌥⌘")
        if typing and self.owned_by_text(chord) and not (mod and self.wrong("text-keys-to-engine")) and not (chord == "Enter" and self.wrong("text-return-to-engine")):
            self.text_edit(where, chord)
            return
        self.press(chord)

    def text_edit(self, where, chord):
        if where == "body":
            t = self.c["text"]
            if chord == "⌘Z":
                self.c["text"] = self.hist.pop() if self.hist else t
            elif len(chord) == 1:
                self.hist.append(t)
                self.c["text"] = t + chord.lower()
            return
        if where == "search":
            if chord == "⌘A":
                self.select_all = True
            elif len(chord) == 1:
                self.bar = chord.lower() if self.select_all else self.bar + chord.lower()
                self.select_all = False
            return
        if len(chord) == 1:                       # the Settings field
            self.settings_query += chord.lower()

    # -- the engine
    def visible(self):
        out = []
        for i in self.order:
            m = self.msgs[i]
            if self.mailbox == "archive":
                ok = m["archived"] and not m["deleted"]
            elif self.mailbox in ("sent", "drafts", "snoozed", "junk", "trash", "starred", "muted", "all mail"):
                ok = {"junk": m["spam"], "trash": m["deleted"], "starred": m["starred"], "muted": m["muted"]}.get(self.mailbox, False) or self.mailbox == "all mail"
            else:
                ok = not (m["archived"] or m["deleted"] or m["spam"] or m["muted"])
            if ok and self.filter == "unread" and not m["unread"]:
                ok = False
            if ok and self.filter == "starred" and not m["starred"]:
                ok = False
            if ok and self.filter == "important" and ROWS[[r["id"] for r in ROWS].index(i)].get("tab") not in ("needs-reply", "vips"):
                ok = False
            if ok and self.filter == "no-reply" and ROWS[[r["id"] for r in ROWS].index(i)].get("tab") != "needs-reply":
                ok = False
            if ok:
                out.append(i)
        return out

    def commands_for(self, chord):
        ctx = [cid for c, cid in self.p["context"] if c == chord]
        row = next((cid for c, cid in self.p["rows"] + self.p["also"] + self.p["fixed"] if c == chord), None)
        allc = ctx + ([row] if row else [])
        live = [c for c in allc if self.cmds.get(c, ("", "unknown", ""))[1] != "soon"]
        return live + [c for c in allc if c not in live]

    def press(self, chord):
        self.n += 1
        self.notice = None
        if chord == "⌘," and not self.settings_open:
            self.settings_open, self.settings_field = True, False
            return True
        if self.settings_open and chord == "⌘F":
            self.settings_field = True
            return True
        if self.settings_open and chord == "Escape":
            self.settings_open, self.settings_field, self.settings_query = False, False, ""
            return True
        if self.armed:
            first, self.armed = self.armed, None
            q = next((cid for a, b, cid in self.p["sequences"] if a == first and b == chord), None)
            if q is None:
                self.log.append(dict(n=self.n, key=chord, command="", resolved="", outcome="cancelled"))
                return False
            return self.run(q, "%s then %s" % (first, chord[1:] if chord in SHIFT_SYMBOL else chord))
        if chord == "Escape" and self.early_escape():
            return True
        if chord == "Enter" and self.early_enter():
            return True
        for cid in self.commands_for(chord):
            if self.cmds.get(cid, ("", "", ""))[1] == "soon":
                self.notice = "%s is coming soon." % self.cmds[cid][2]
                continue
            if self.run(cid, chord):
                return True
        if any(a == chord for a, b, _ in self.p["sequences"]) and not self.composer_open():
            self.armed = chord
            self.log.append(dict(n=self.n, key=chord, command="", resolved="", outcome="armed"))
            return True
        return False

    def run(self, cid, key):
        group = self.cgroup.get(cid, "list")
        if self.mut and self.mut[0] == "dead" and group == self.mut[1]:
            return False
        h = getattr(self, "h_" + cid.replace("-", "_"), None)
        self.resolved = cid
        ok = bool(h and h())
        self.log.append(dict(n=self.n, key=key, command=cid, resolved=self.resolved, outcome="ran" if ok else "passed"))
        return ok

    def wrong(self, name):
        return self.mut == ("wrong", name)

    # -- state shortcuts
    def composer_open(self):
        return self.c is not None and self.c["open"]

    def reader_open(self):
        return self.surface == "mac-reader"

    def new_composer(self):
        self.c = dict(open=True, focused="body", cc=False, bcc=False, from_open=False, discard_ask=False, attach=False, color=False, link=False,
                      to=[], bcc_list=[], text="", bold=False, italic=False, underline=False, strike=False, list="none", quote=False, indent=0,
                      snippets=False, inline=[], send_later=False, remind=False, undo=False, prev_surface=self.surface)
        self.surface = "mac-composer"
        return True

    def early_escape(self):
        if self.pending:
            self.pending = None
            return True
        if self.overlay:
            self.overlay = None
            return True
        if self.search:
            self.search = False
            return True
        if self.composer_open():
            c = self.c
            if c["discard_ask"]:
                c["discard_ask"] = False
                return True
            for f in ("attach", "color", "link", "snippets", "from_open", "send_later", "remind"):
                if c[f]:
                    c[f] = False
                    return True
            if c["inline"]:
                c["inline"] = []
                return True
            self.surface, self.c["open"] = "mac-main", False        # Escape closes the window and keeps the draft
            return True
        return False

    def early_enter(self):
        if self.pending == "label":
            self.msgs[self.focus_id]["labels"].add(self.typed or "Suppliers")
            self.pending, self.typed = None, ""
            return True
        if self.composer_open() and self.c["discard_ask"]:
            self.surface, self.c = "mac-main", None
            return True
        return False

    # -- list handlers
    def move(self, d):
        v = self.visible()
        if self.focus_id not in v:
            return False
        i = v.index(self.focus_id)
        if self.wrong("list-jk") and d == 1:
            d = -1
        if not 0 <= i + d < len(v):
            return False
        self.focus_id = v[i + d]
        return True

    def h_next(self):
        return self.move(1)

    def h_previous(self):
        return self.move(-1)

    def flag(self, name, value=True):
        if self.focus_id is None:
            return False
        self.msgs[self.focus_id][name] = value
        self.undo_stack.append((self.focus_id, name))
        self.after_remove()
        return True

    def after_remove(self):
        v = self.visible()
        if self.focus_id not in v and v:
            self.focus_id = v[0]

    def h_archive(self):
        if self.wrong("list-archive"):
            return self.flag("deleted")
        return self.flag("archived")

    def h_delete(self):
        return self.flag("deleted")

    def h_junk(self):
        return self.flag("spam")

    def h_mute(self):
        return self.flag("muted")

    def h_star(self):
        self.msgs[self.focus_id]["starred"] = True
        return True

    def h_pin(self):
        self.msgs[self.focus_id]["pinned"] = True
        return True

    def h_unread(self):
        m = self.msgs[self.focus_id]
        m["unread"] = not m["unread"]
        return True

    def h_snooze(self):
        self.pending = "snooze"
        return True

    def h_label(self):
        self.pending = "label"
        return True

    def h_move(self):
        self.overlay = "move"
        return True

    def h_undo(self):
        if not self.undo_stack:
            return False
        i, name = self.undo_stack.pop()
        self.msgs[i][name] = False
        return True

    def h_mark_not_done(self):
        m = self.msgs.get(self.focus_id)
        if self.mailbox == "archive" and m and m["archived"]:
            m["archived"] = False
            return True
        return False

    def h_select_conversation(self):
        self.sel = [self.focus_id]
        return True

    def h_extend_selection_down(self):
        v = self.visible()
        base = self.sel[0] if self.sel else self.focus_id
        i = v.index(base)
        self.sel = v[i:i + 2]
        return True

    def h_extend_selection_up(self):
        v = self.visible()
        i = v.index(self.focus_id)
        self.sel = v[max(0, i - 1):i + 1]
        return True

    def h_clear_selection(self):
        if self.sel:
            self.sel = []
            return True
        if self.filter:
            self.filter = None
            return True
        return False

    def h_select_from_here(self):
        v = self.visible()
        self.sel = v[v.index(self.focus_id):]
        return True

    def h_select_all(self):
        if self.surface != "mac-main":
            return False
        self.sel = self.visible()
        return True

    def filt(self, name):
        self.filter = None if self.filter == name else name
        return True

    def h_filter_unread(self):
        return self.filt("unread")

    def h_filter_starred(self):
        return self.filt("starred")

    def h_filter_important(self):
        return self.filt("important")

    def h_filter_no_reply(self):
        return self.filt("no-reply")

    def unlabel(self, step):
        m = self.msgs[self.focus_id]
        if not m["labels"]:
            return False
        v = self.visible()
        i = v.index(self.focus_id)
        m["labels"] = set()
        if step:
            self.focus_id = v[min(max(i + step, 0), len(v) - 1)]
        return True

    def h_remove_label(self):
        return self.unlabel(0)

    def h_remove_all_labels(self):
        return self.unlabel(0)

    def h_remove_label_next(self):
        return self.unlabel(1)

    def h_remove_label_previous(self):
        return self.unlabel(-1)

    def h_next_split(self):
        self.tab = (self.tab + 1) % len(self.TABS)
        self.mailbox = None
        return True

    def h_previous_split(self):
        self.tab = (self.tab - 1) % len(self.TABS)
        return True

    def h_enter(self):
        if self.reader_open():
            self.resolved = "reply-all"
            self.overlay = "quick-reply"
            return True
        if self.wrong("enter-opens-only"):          # the mutant: the Superhuman "open" the list had before A102, no reply under the thread
            self.resolved = "open"
        else:
            self.resolved = "reply"                 # the list's Enter is R's act: the thread and the quick reply, the cursor in its field
            self.overlay, self.fr = "quick-reply", True
        self.surface = "mac-reader"
        self.rd = dict(focused=2, marked=False, expanded={2}, header=set(), offset=0, ring=None, unsub=False)
        return True

    def run_enter_log(self):
        pass

    # -- reader handlers
    def h_back(self):
        if self.reader_open():
            self.surface, self.rd = "mac-main", None
            return True
        return False

    def h_reply(self):
        if self.reader_open() or self.focus_id:
            self.overlay = "quick-reply"
            return True
        return False

    def h_ai_reply(self):
        if not self.reader_open() and not self.wrong("cmd-j-everywhere"):
            return False
        self.overlay, self.ai = "quick-reply", True
        return True

    def h_reply_all(self):
        self.overlay = "quick-reply"
        return True

    def h_next_message(self):
        if not self.reader_open():
            return False
        r = self.rd
        if self.wrong("reader-np"):
            r["focused"] = max(0, r["focused"] - 1)
            return True
        r["focused"] = min(2, r["focused"] + 1)
        return True

    def h_previous_message(self):
        if not self.reader_open():
            return False
        r = self.rd
        if self.wrong("reader-np"):
            r["focused"] = min(2, r["focused"] + 1)
            return True
        r["focused"] = max(0, r["focused"] - 1)
        r["marked"] = True
        return True

    def h_expand_message(self):
        if not self.reader_open():
            return False
        r = self.rd
        r["expanded"] ^= {r["focused"]}
        return True

    def h_expand_all_messages(self):
        if not self.reader_open():
            return False
        self.rd["expanded"] = {0, 1, 2} if self.rd["expanded"] != {0, 1, 2} else {2}
        return True

    def h_toggle_colours(self):
        """KeyPreset.swift:132: ⇧I is the colour switch while a mail is open (the READER state, whatever pane has the focus); with none open it is the important filter.
        Mutant colours-from-list: ⇧I filters while a mail is open."""
        if not self.reader_open() or self.wrong("colours-from-list"):
            return False
        self.rd["colours"] = "original" if self.rd["colours"] == "app" else "app"
        return True

    def h_toggle_message_header(self):
        if not self.reader_open():
            return False
        self.rd["header"] ^= {self.rd["focused"]}
        return True

    VIEWPORT, MAXOFF = 600, 1500

    def h_scroll_down(self):
        if not self.reader_open() or self.rd["offset"] >= self.MAXOFF:
            return False
        self.rd["offset"] = min(self.MAXOFF, self.rd["offset"] + self.VIEWPORT)
        return True

    def h_scroll_up(self):
        if not self.reader_open() or self.rd["offset"] <= 0:
            return False
        self.rd["offset"] = max(0, self.rd["offset"] - self.VIEWPORT)
        return True

    def h_jump_bottom(self):
        if not self.reader_open():
            return False
        self.rd["offset"] = self.MAXOFF
        return True

    def h_jump_top(self):
        if not self.reader_open():
            return False
        self.rd["offset"] = 0
        return True

    def h_unsubscribe(self):
        if not self.reader_open() or not self.msgs[self.focus_id]["lists"]:
            return False
        self.rd["unsub"] = True
        return True

    def h_open_links(self):
        if not self.reader_open() or not self.corpus:
            return False
        self.opened.append("browser")
        return True

    def h_cycle_links(self):
        if not self.reader_open() or not self.corpus:
            return False
        r = self.rd
        r["ring"] = 0 if r["ring"] is None else (r["ring"] + 1 if r["ring"] + 1 < 5 else None)
        return True

    def h_cycle_links_back(self):
        if not self.reader_open() or not self.corpus or self.rd["ring"] is None:
            return False
        self.rd["ring"] = max(0, self.rd["ring"] - 1)
        return True

    def popout(self):
        self.popouts += 1
        return True

    def h_popout_reply(self):
        return self.reader_open() and self.popout()

    def h_popout_reply_all(self):
        return self.reader_open() and self.popout()

    def h_popout_new(self):
        return self.popout()

    def h_popout_forward(self):
        return self.reader_open() and self.popout()

    def h_popout_draft_search(self):
        if not self.reader_open() or not self.qr_text:        # nothing to pop out without a draft in the reply box
            return False
        self.popout()
        self.search = True
        return True

    # -- composer handlers
    def h_compose(self):
        return self.new_composer()

    def h_forward(self):
        return self.new_composer() if self.focus_id else False

    def when(self, fn):
        if not self.composer_open():
            return False
        fn(self.c)
        return True

    def focus_field(self, name):
        def f(c):
            c["focused"] = name
            c["from_open"] = name == "from"
            if name == "cc":
                c["cc"] = True
            if name == "bcc":
                c["bcc"] = True
        return self.when(f)

    def h_compose_to(self):
        return self.focus_field("to")

    def h_compose_cc(self):
        return self.focus_field("cc")

    def h_compose_bcc(self):
        return self.focus_field("bcc")

    def h_compose_from(self):
        return self.focus_field("from")

    def h_edit_subject(self):
        return self.focus_field("subject")

    def h_edit_message(self):
        return self.focus_field("body")

    def h_discard_draft(self):
        return self.when(lambda c: c.update(discard_ask=True))

    def h_move_contacts_to_bcc(self):
        return self.when(lambda c: c.update(bcc_list=c["bcc_list"] + c["to"], to=[]))

    def h_attach(self):
        return self.when(lambda c: c.update(attach=True))

    def h_use_snippet(self):
        return self.when(lambda c: c.update(snippets=True))

    def h_snippet_inline(self):
        return self.when(lambda c: c.update(inline=["snippet", "snippet"]) if c["focused"] == "body" else None)

    def fmt_flag(self, name):
        if self.wrong("composer-bold") and name == "bold":
            name = "italic"
        return self.when(lambda c: c.update({name: not c[name]}))

    def h_bold(self):
        return self.fmt_flag("bold")

    def h_italic(self):
        return self.fmt_flag("italic")

    def h_underline(self):
        return self.fmt_flag("underline")

    def h_strikethrough(self):
        return self.fmt_flag("strike")

    def h_bullet_list(self):
        return self.when(lambda c: c.update({"list": "bullet"}))

    def h_numbered_list(self):
        return self.when(lambda c: c.update({"list": "number"}))

    def h_quote(self):
        return self.when(lambda c: c.update({"quote": not c["quote"], "list": "none"}))

    def h_increase_indent(self):
        return self.when(lambda c: c.update(indent=c["indent"] + 1))

    def h_decrease_indent(self):
        return self.when(lambda c: c.update(indent=max(0, c["indent"] - 1)))

    def h_indent_list(self):
        return self.composer_open() and self.c["list"] != "none" and self.when(lambda c: c.update(indent=c["indent"] + 1))

    def h_outdent_list(self):
        return self.composer_open() and self.c["list"] != "none" and self.when(lambda c: c.update(indent=max(0, c["indent"] - 1)))

    def h_color(self):
        return self.when(lambda c: c.update(color=True))

    def h_hyperlink(self):
        return self.when(lambda c: c.update(link=True))

    def h_send_later(self):
        return self.when(lambda c: c.update(send_later=True))

    def h_remind_me(self):
        return self.when(lambda c: c.update(remind=True))

    def send_now(self, undo, archive=False):
        if not self.composer_open():
            return False
        self.entries += 1
        if archive:
            self.msgs[self.focus_id]["archived"] = True
        self.surface = "mac-main"
        self.c.update(open=False, undo=undo)
        self.after_remove()
        return True

    def h_send(self):
        return self.send_now(True)

    def h_send_and_done(self):
        return self.send_now(True, archive=True)

    def h_instant_send(self):
        return self.send_now(False)

    def h_popout_draft(self):
        return self.overlay == "quick-reply" and bool(self.qr_text) and self.popout()

    def h_switch_draft(self):
        if self.composer_open():
            self.surface, self.c["open"] = "mac-main", False
            self.c["parked"] = True
            return True
        if self.c and self.c.get("parked"):
            self.surface, self.c["open"], self.c["parked"] = "mac-composer", True, False
            return True
        return False

    # -- sidebar / window / find / accounts handlers
    def go(self, box):
        if self.wrong("sidebar-go") and box == "sent":
            box = "archive"
        self.mailbox, self.filter, self.sel = box, None, []
        self.after_remove()
        return True

    def h_go_sent(self):
        return self.go("sent")

    def h_go_drafts(self):
        return self.go("drafts")

    def h_go_done(self):
        return self.go("archive")

    def h_go_reminders(self):
        return self.go("snoozed")

    def h_go_spam(self):
        return self.go("junk")

    def h_go_trash(self):
        return self.go("trash")

    def h_go_starred(self):
        return self.go("starred")

    def h_go_muted(self):
        return self.go("muted")

    def h_go_all_mail(self):
        return self.go("all mail")

    def h_go_inbox(self):
        self.mailbox, self.tab = None, 0
        return True

    def h_go_label(self):
        self.sidebar_sel = "mac-main.sidebar.label." + self.FIRST_LABEL
        return True

    def h_sidebar(self):
        self.sidebar_hidden = not self.sidebar_hidden
        return True

    def h_focus_left(self):
        if self.sidebar_hidden and self.pane == "list":      # with the sidebar hidden there is no pane to the left: the key does nothing (read on main51)
            return False
        self.pane = {"list": "sidebar", "reader": "list"}.get(self.pane, self.pane)
        return True

    def h_focus_right(self):
        self.pane = {"sidebar": "list", "list": "reader"}.get(self.pane, self.pane)
        return True

    def h_new_window(self):
        self.main_windows += 1
        return True

    def h_font_bigger(self):
        self.scale = round(self.scale + (-0.1 if self.wrong("window-font") else 0.1), 2)
        return True

    def h_font_smaller(self):
        self.scale = round(self.scale - 0.1, 2)
        return True

    def h_font_reset(self):
        self.scale = 1.0
        return True

    def h_copy_page_link(self):
        if not self.focus_id:
            return False
        self.last_link = "whoosh://message/" + self.focus_id
        return True

    def h_search(self):
        self.search = True
        if self.wrong("find-palette"):
            self.overlay = "mac-palette"
            self.search = False
        return True

    def h_palette(self):
        self.overlay = "mac-palette"
        return True

    def digit(self, n):
        accounts = ACCOUNT_IDS
        self.account = accounts[n if self.wrong("accounts-off-by-one") else n - 1]
        return True

    def __getattr__(self, name):
        m = re.match(r"h_switch_account_(\d)$", name)
        if m:
            n = int(m.group(1))
            return lambda: self.digit(n)
        raise AttributeError(name)

    # -- the socket's verbs
    def row_dicts(self):
        return [dict(id=i, unread=self.msgs[i]["unread"], pinned=self.msgs[i]["pinned"], chips=sorted(self.msgs[i]["labels"])) for i in self.visible()]


class ModelCtl:
    """The control socket of the model: the verbs the checks use, answering as the app does (ok:false is Unreadable)."""

    def __init__(self, app):
        self.a = app
        self.core_clear = {}
        self.last = None

    def key(self, chord):
        if not self.a.press(chord):
            raise Unreadable("key %s: the key %s does nothing on %s" % (chord, chord, self.a.surface))
        return {}

    def type(self, text):
        a = self.a
        if a.pending == "label":
            a.typed = text
        elif a.overlay == "quick-reply" and not a.composer_open():
            a.qr_text += text
        elif a.composer_open() and a.c["focused"] == "to":
            a.c["to"] = a.c["to"] + [x for x in text.split(",") if x]
        elif a.composer_open():
            a.c["text"] += text
        return {}

    def click(self, ax):
        a = self.a
        m = re.match(r"mac-main\.list\.row\.(\d+)$", ax)
        if m:
            v = a.visible()
            if int(m.group(1)) >= len(v):
                raise Unreadable("click %s: nothing to click" % ax)
            a.focus_id = v[int(m.group(1))]
            return {}
        if ax == "mac-main.reader.open":
            if not a.focus_id:
                raise Unreadable("click: no focus")
            a.surface = "mac-reader"
            a.rd = dict(focused=2, marked=False, expanded={2}, header=set(), offset=0, ring=None, unsub=False, colours="app")
            return {}
        if ax == "mac-main.reader.open-full-composer":
            a.new_composer()
            a.c["to"] = [MSG[a.focus_id]["from"]]            # a reply opens with the sender in To
            return {}
        if ax == "mac-main.accounts.all":
            a.account = None
            return {}
        if ax == "macshell.window.close-others":
            a.main_windows, a.popouts = 1, 0
            return {}
        if ax in ("mac-main.pane.list", "mac-main.pane.reader"):
            a.pane = ax.rsplit(".", 1)[1]
            return {}
        m = re.match(r"mac-main\.splits\.(\d)$", ax)
        if m:
            a.tab, a.mailbox = int(m.group(1)), None
            return {}
        raise Unreadable("click %s: nothing to click" % ax)

    def uistate(self):
        a = self.a
        v = a.visible()
        return {"surface": a.surface, "tab": a.TABS[a.tab], "focus_message": a.focus_id, "focus_index": v.index(a.focus_id) if a.focus_id in v else 0,
                "overlay": a.overlay, "account_filter": a.account,
                "first_responder": "quick-reply" if getattr(a, "fr", False) and a.overlay == "quick-reply" else "none"}

    def core(self, path):
        a = self.a
        m = re.match(r"message/([\w-]+)/(\w+)$", path)
        if m:
            return {"archived": 0, "deleted": 0, "spam": 0, "muted": 0, "starred": 0}.get(m.group(2)) is not None and a.msgs[m.group(1)][m.group(2)]
        m = re.match(r"pin/([\w-]+)$", path)
        if m:
            return a.msgs[m.group(1)]["pinned"]
        m = re.match(r"label_assignment/([\w-]+)$", path)
        if m:
            return str(sorted(a.msgs[m.group(1)]["labels"]))
        if path == "outbox_entry/count":
            return str(a.entries)
        raise Unreadable("no core value at %s" % path)

    def winframes(self):
        frames = [{"title": "w"}] * (a_windows(self.a))
        if self.a.settings_open:                                     # the Settings window, listed by the id the app gives it (WindowBridge.settingsWindowId)
            frames = frames + [{"id": "mac-settings", "title": "Settings", "kind": "window"}]
        return frames

    def send(self, verb, *args):
        a = self.a
        if verb == "fixture" and args == ("digest",):
            return {"digest": "model", "parts": {}, "view": "v"}
        if verb == "fixture" and args == ("reset",):
            a.overlay, a.surface, a.typed, a.search, a.pending, a.popouts, a.main_windows = None, "mac-main", "", False, None, 0, 1
            a.qr_text = ""
            a.settings_open, a.settings_field, a.settings_query, a.bar, a.select_all, a.hist = False, False, "", "", False, []
            a.rd = None
            if a.c:
                a.c["open"] = False
            return {}
        if verb == "fixture" and args and args[0] == "corpus":
            a.corpus = True
            return {}
        if verb in ("activate", "clock"):
            return {}
        if verb == "keyevent":
            return self.keyevent(args)
        if verb == "key" and args and str(args[0]).startswith("code:"):
            return self.key_code(args)
        raise Unreadable("unknown verb %s" % verb)

    NAMED = {"return": "Enter", "enter": "Enter", "escape": "Escape", "esc": "Escape", "tab": "Tab", "space": "Space"}

    def keyevent(self, args):
        """`keyevent [arm:x] <chord|text>` as KeyEventVerb.swift parses it: one chord, or text typed a character at a time; arm c = window not key."""
        rest = list(args)
        arm = rest.pop(0)[4:] if rest and rest[0].startswith("arm:") else "a"
        text = " ".join(rest)
        mods, low = "", text.lower()
        for word, sym in (("ctrl+", "⌃"), ("alt+", "⌥"), ("opt+", "⌥"), ("shift+", "⇧"), ("cmd+", "⌘")):
            if low.startswith(word):
                mods, low = mods + sym, low[len(word):]
        order = "".join(m for m in "⌃⌥⇧⌘" if m in mods)
        chords = [order + self.NAMED.get(low, low.upper())] if (mods or low in self.NAMED) else list(text)
        for c in chords:
            self.a.real_key(c, arm != "c")
        self.last = {"window_id": 11 if arm == "c" else 10, "window": "Settings", "focus": self.a.text_focus() or "none", "key_before": arm != "c", "key_after": arm != "c",
                     "route": "sendEvent", "handled": arm != "c", "windows": 2,
                     "text_before": self.a.settings_query, "text_after": self.a.settings_query}
        return {"arm": arm, "sent": len(chords)}

    AZERTY = {18: "&", 19: "é", 20: '"', 21: "'", 23: "(", 22: "§", 26: "è", 28: "!", 25: "ç", 29: "à"}

    def key_code(self, args):
        """What Verbs/KeyCodeVerb.swift and ChordKit.chord do for `key code:n mods:...` on the AZERTY layout, by hand."""
        code = int(args[0][5:])
        mods = next((x[5:].split("+") for x in args[1:] if x.startswith("mods:")), [])
        prefix = "".join(sym for sym, word in (("⌃", "ctrl"), ("⌥", "opt"), ("⇧", "shift"), ("⌘", "cmd")) if word in mods)
        digit = next(d for d, c in DIGIT_CODE.items() if c == code)
        by_key = bool(mods) and mods != ["shift"]
        # the by-character mutant is the old ChordKit: it read the character the layout types, so the "2" key gave "É"
        chord = prefix + (digit if by_key and not self.a.wrong("digits-by-character") else (digit if mods == ["shift"] else self.AZERTY[code].upper()))
        if not self.a.press(chord):
            raise Unreadable("key %s: the key %s (code %d) does nothing on %s" % (args, chord, code, self.a.surface))
        return {"chord": chord}

    def rows(self, ax):
        a = self.a
        c = a.c or {}
        r = a.rd or {}
        simple = {
            "mac-main.list": lambda: a.row_dicts(),
            "mac-main.list.state": lambda: [{"pending": a.pending, "filter": a.filter, "mailbox": a.mailbox,
                                             "selection_bar": {"count": len(a.sel)} if a.sel else None}],
            "mac-main.list.selection": lambda: [{"id": i} for i in a.sel],
            "mac-main.splits": lambda: [{"id": t} for t in a.TABS],
            "mac-main.reader.state": lambda: [{"focused_message": r.get("focused"), "marked": r.get("marked"), "expanded": sorted(r.get("expanded", ())), "message_count": 3}],
            "mac-main.reader.body": lambda: [{"colours": r.get("colours")}],
            "mac-main.reader.scroll": lambda: [{"offset": r.get("offset", 0), "max_offset": a.MAXOFF, "viewport": a.VIEWPORT}],
            "mac-main.reader.header": lambda: [{"open": i in r.get("header", ())} for i in range(3)],
            "mac-main.reader.links": lambda: [] if r.get("ring") is None else [{"ring": i == r["ring"]} for i in range(5)],
            "mac-main.reader.unsubscribe": lambda: [{"pending": bool(r.get("unsub"))}],
            "mac-main.reader.opened": lambda: [{"kind": k} for k in a.opened],
            "mac-main.quickreply.ai": lambda: [{"phase": "writing", "writes": 1, "instruction_field": "0"}] if getattr(a, "ai", False) and a.overlay == "quick-reply" else [],
            "mac-main.quickreply.field": lambda: [{"thread": a.focus_id}] if a.overlay == "quick-reply" else [],
            "mac-composer.header": lambda: [{"focused": c["focused"], "cc_visible": c["cc"], "bcc_visible": c["bcc"], "from_open": c["from_open"],
                                             "discard_ask": c["discard_ask"], "attach_open": c["attach"], "color_open": c["color"], "link_open": c["link"],
                                             "to": c["to"], "bcc": c["bcc_list"], "remind_open": c["remind"]}],
            "mac-composer.format": lambda: [{"bold": c["bold"], "italic": c["italic"], "underline": c["underline"], "strike": c["strike"],
                                             "list": c["list"], "quote": c["quote"], "indent": c["indent"]}],
            "mac-composer.snippets": lambda: [{"id": 0}, {"id": 1}] if c.get("snippets") else [],
            "mac-composer.inline": lambda: [{"kind": k} for k in c.get("inline", [])],
            "mac-composer.send-later.options": lambda: [{"title": "08:00"}, {"title": "13:00"}] if c.get("send_later") else [],
            "mac-composer.undo.bar": lambda: [{"undo_offered": bool(c.get("undo")), "text": "Sending"}],
            "mac-keyevent.monitor": lambda: [{"last": self.last}] if self.last else [],
            "mac-search.bar": lambda: [{"focused": a.search, "owner": "search", "text": a.bar}],
            "mac-composer.body": lambda: [{"text": c["text"]}],
            "mac-settings-window.place": lambda: [{"pane": "general", "query": a.settings_query}],
            "mac-main.shell": lambda: [{"focused_pane": a.pane, "main_windows": a.main_windows, "text_scale": a.scale, "last_link": a.last_link}],
            "mac-main.sidebar.visibility": lambda: [{"hidden": a.sidebar_hidden}],
            "mac-main.sidebar.selection": lambda: [{"id": a.sidebar_sel}],
            "mac-main.sidebar.labels": lambda: [{"id": Model.FIRST_LABEL}],
            "mac-shortcuts.dispatch": lambda: list(a.log[-64:]),
            "mac-shortcuts.map": lambda: self.map_rows(),
        }
        if ax not in simple:
            raise Unreadable("no rows at %s" % ax)
        try:
            return simple[ax]()
        except (KeyError, TypeError):
            raise Unreadable("no rows at %s (the screen is not open)" % ax)

    def map_rows(self):
        a = self.a
        out = []
        for kind, chord, cid in ([("row", c, i) for c, i in a.p["rows"]] + [("also", c, i) for c, i in a.p["also"]]
                                 + [("context", c, i) for c, i in a.p["context"]]
                                 + [("sequence", label_of((x, y)), i) for x, y, i in a.p["sequences"]]
                                 + [("fixed", c, i) for c, i in a.p["fixed"]]):
            title, status, owner = a.cmds[cid]
            st = {"soon": "coming-soon", "stub": "stub"}.get(status, "live")
            if a.mut == ("wrong", "soon-live") and status == "soon":
                st = "live"
            out.append({"key": chord, "kind": kind, "command": cid, "status": st, "feature": owner if status == "soon" else ""})
        return out


def a_windows(app):
    return app.main_windows + app.popouts


def run_model(mod, path, mutant=None, preset=None, commands=None):
    """Drive the checks against the model; the same code path as run_journey, with the model for the socket."""
    app = Model(mutant, preset, commands)
    ctl = ModelCtl(app)
    j = Journey(ID, ctl)
    drive(j)
    cs = j.checks()
    return j, cs, app


def selftest(mod, path):
    global SLEEP
    SLEEP = lambda _s: None
    try:
        return selftest_run(mod, path)
    finally:
        SLEEP = time.sleep


def selftest_run(mod, path):
    """Clean model: every check green. One mutant per screen group (a dead key) plus a wrong result per group: that group
    turns red and no other does. A binding left without a check, and a mutant left undeclared, fail too."""
    bad = []
    inv = inventory()
    by_group = {g: [slug_of(b) for b in inv if b["group"] == g] for g in GROUPS}
    by_group[DIGIT_GROUP] = [digit_slug(b) for b in digit_chords(inv)]
    by_group[TEXT_GROUP] = [n for n, _ in TEXT_CHECKS]
    declared = {l[2:].split(" | ")[0] for l in MUTANTS.splitlines() if l.startswith("# par-keys-")}

    def red(cs, g):
        return sorted(c.label.split("/")[1] for c in cs if c.state != "pass" and c.label.startswith("par-keys-%s/" % g))

    j, cs, _ = run_model(mod, path)
    print_model_cases(cs)
    fails = [c for c in cs if c.state != "pass"]
    print("clean model: %d checks, %d not green" % (len(cs), len(fails)))
    for c in fails[:int(os.environ.get("SHOW", "12"))]:
        print("  NOT GREEN %s: %s %s" % (c.label, c.state, c.detail))
    if fails:
        bad.append("the clean model is not green")
    labels = {c.label for c in cs}
    for x in sorted(labels - declared):
        bad.append("no mutant declared for " + x)
    for x in sorted(declared - labels):
        bad.append("stray mutant " + x)
    print("coverage: bindings=%d checks=%d declared mutants=%d" % (len(inv), len(cs), len(declared)))

    mutants = [(("dead", g), g) for g in GROUPS if g != "soon"] + [
        (("wrong", "list-jk"), "list"), (("wrong", "list-archive"), "list"), (("wrong", "reader-np"), "reader"), (("wrong", "colours-from-list"), "reader"), (("wrong", "composer-bold"), "composer"),
        (("wrong", "sidebar-go"), "sidebar"), (("wrong", "window-font"), "window"), (("wrong", "find-palette"), "find"),
        (("wrong", "accounts-off-by-one"), "accounts"), (("wrong", "enter-opens-only"), "list"), (("wrong", "soon-live"), "soon"), (("wrong", "digits-by-character"), DIGIT_GROUP),
        (("wrong", "cmd-j-everywhere"), "aireply"), (("wrong", "text-keys-to-engine"), TEXT_GROUP), (("wrong", "text-key-window"), TEXT_GROUP), (("wrong", "text-return-to-engine"), TEXT_GROUP)]
    print("| mutant | group | checks red in the group | checks red elsewhere |")
    print("|---|---|---|---|")
    for m, g in mutants:
        j2, cs2, _ = run_model(mod, path, mutant=m)
        inside = red(cs2, g)
        # the text group's ⌘A case opens the search with ⌘K: a mutant of the find group legitimately reddens it
        outside = sorted(c.label for c in cs2 if c.state != "pass" and not c.label.startswith("par-keys-%s/" % g)
                         and not (g == "find" and c.label.startswith("par-keys-%s/cmd-a" % TEXT_GROUP)))
        print("| %s | %s | %d of %d | %d |" % ("%s:%s" % m, g, len(inside), len(by_group[g]), len(outside)))
        if not inside:
            bad.append("mutant %s:%s turned no check red in %s" % (m[0], m[1], g))
        if m[0] == "dead" and len(inside) < max(1, len(by_group[g]) // 2):
            bad.append("dead group %s: only %d of %d checks red" % (g, len(inside), len(by_group[g])))
        # a dead group may break the setup of another group's check that presses a key of it (the sends, G sequences): list them, fail on none of the soon group
        if m[0] == "wrong" and outside:
            bad.append("wrong-result mutant %s:%s turned %d checks outside %s red: %s" % (m[0], m[1], len(outside), g, outside[:3]))

    # the evidence a failed key case prints: the key verb's report and the shell's focus (a mutant makes the settings cases red)
    shown = []
    for mm in (("wrong", "text-key-window"), ("wrong", "text-return-to-engine")):
        j3, cs3, _ = run_model(mod, path, mutant=mm)
        shown += [c for c in cs3 if c.state == "fail" and c.label.startswith("par-keys-%s/" % TEXT_GROUP) and ("settings" in c.label or "non-key-window" in c.label)]
    for c in shown:
        print("FAIL %s: %s" % (c.label, c.detail))
        for need in ("app said: key ", "window_id=", "key_before=", "key_after=", "route='sendEvent'", "handled=", "text_before=", "text_after=", "shell surface=", "focused pane"):
            if need not in c.detail:
                bad.append("a failed text case does not carry %r: %s" % (need, c.label))
    if not shown:
        bad.append("the text-key-window mutant made no settings case fail, so the evidence text was not seen")
    seen_cases = sorted({c.label for c in shown})
    print("evidence on red (par-keys): %d of 2 settings-field cases print the key verb's report (window, focus, key window before and after, route) on their mutants: %s; "
          "the 2 reset cases (composer/cmd-d-switch-draft, accounts/ctrl-1-switch-account-1) share home(), whose refusal prints the view fields that differ (above) and, from the guard, both view strings" % (len(seen_cases), seen_cases))
    if len(seen_cases) != 2:
        bad.append("not both settings-field cases failed on a mutant: %s" % seen_cases)
    #   an older build serves no monitor row, or one without the new fields: the words say "absent" and nothing raises
    for rows_reply, want in (([], "keyevent report absent"), ([{"last": {"window_id": 3}}], "focus='absent'")):
        class Old:
            def rows(self, ax, _r=rows_reply):
                return _r
        if want not in said_of(monitor_last(Old())):
            bad.append("an older build's monitor row (%r) is not read as absent" % (rows_reply,))
    class Raises:
        def rows(self, ax):
            raise Unreadable("no rows at " + ax)
    if monitor_last(Raises()) != {}:
        bad.append("a refused monitor read raised")
    #   the reset guard's view refusal names the fields that differ
    class ViewCtl:
        def __init__(self, v):
            self.v = v
        def send(self, *a):
            return {"digest": "d", "parts": {}, "view": self.v}
        def uistate(self):
            return {"surface": "mac-reader", "first_responder": "list"}
    saved_launch = dict(RG.LAUNCH)
    RG.LAUNCH.clear()
    RG.LAUNCH.update(d="d", parts={}, v="surface=mac-main;tab=primary;typed=")
    try:
        detail = reset_view_detail(ViewCtl("surface=mac-reader;tab=primary;typed=q"))
        print("view refusal text:" + detail)
        for need in ("surface launch='mac-main' now='mac-reader'", "typed launch='' now='q'", "surface='mac-reader'"):
            if need not in detail:
                bad.append("the reset view detail lacks %r" % need)
        if "tab" in view_diff(RG.LAUNCH["v"], "surface=mac-reader;tab=primary;typed=q"):
            bad.append("the view diff names a field that does not differ")
    finally:
        RG.LAUNCH.clear()
        RG.LAUNCH.update(saved_launch)

    # the coverage mutants: a binding without a check, a check without a binding
    saved = dict(PROMISES)
    victim = next(iter(PROMISES))
    del PROMISES[victim]
    nc, st, wg = coverage()
    PROMISES.update(saved)
    print("coverage mutant (check for %s removed): no_check=%d" % (" ".join(victim), len(nc)))
    if not nc:
        bad.append("a binding with no check was not reported")
    PROMISES[("Q", "no-such-command")] = ("list", "x", lambda k: ((lambda: 1), 1))
    nc2, st2, wg2 = coverage()
    del PROMISES[("Q", "no-such-command")]
    print("coverage mutant (check for a binding the source lacks): stale=%d" % len(st2))
    if not st2:
        bad.append("a stale check was not reported")
    nc3, st3, wg3 = coverage()
    if nc3 or st3 or wg3:
        bad.append("the real inventory has a coverage gap: %s %s %s" % (nc3, st3, wg3))
    for line in bad:
        print("SELFTEST FAIL: " + line)
    print("par-keys selftest: %s (%d problems)" % ("green" if not bad else "RED", len(bad)))
    return 0 if not bad else 1


if __name__ == "__main__":
    sys.exit(main())
