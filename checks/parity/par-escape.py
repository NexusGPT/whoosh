#!/usr/bin/env python3
"""par-escape: Esc walks one step back toward the list (owner, 13 Nov 07:28: "impossible to go back to list with esc (you need to fix it and add shortcut)").

REAL key events (`keyevent`, the monitor and the window as the keyboard sends them), on the hidden copy, one PASS / FAIL / NOT JUDGED line per case:
  esc-from-the-reader-gives-the-keys-to-the-list   a mail open in the reader (pane focus on the reader): Esc puts the keyboard on the list (shell focused_pane
                                                   `list`, surface `mac-main`), and J then moves the list's focused row (the list owns the keys again)
  esc-from-the-empty-reply-closes-it               R opens the quick reply under the thread; Esc with nothing typed closes it and the reader stays open
  esc-with-text-keeps-the-draft                    the same with words typed: Esc closes the field, the thread is still open, and R finds the words again
  the-bar-names-the-key-while-a-mail-is-open       the foot strip's reader set holds `back` with the key Escape and the words "Back to list"; at the list it does not
  esc-passes-a-thread-with-a-draft-card            the conversation holds a saved reply draft (its card shows under the mail): Esc from the open mail still puts the
                                                   keyboard on the list, J moves the row, and the draft is still the conversation's (the card is there when it is reopened)
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/par-escape.py'
Self-test (no app, a stand-in whose broken copies each turn one case red): python3 checks/parity/par-escape.py --selftest
Prints 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0."""
import os
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable      # noqa: E402

SCREEN = "par-escape"
CASES = []


def case(name, mutant):
    def deco(fn):
        CASES.append((name, mutant, fn))
        return fn
    return deco


def shell(c):
    return c.rows("mac-main.shell")[0]


def foot(c):
    return c.rows("mac-main.foot.hints")


def reply_open(c):
    return len(c.rows("mac-main.quickreply.field")) > 0


def real(c, text):
    c.send("keyevent", "arm:a", text)


def settle(fn, want, tries=30):
    v = fn()
    for _ in range(tries):
        if v == want:
            return v
        time.sleep(0.1)
        v = fn()
    return v


def open_reader(c):
    c.send("fixture", "reset")
    c.click("mac-main.accounts.all")
    c.click("mac-main.pane.list")
    c.click("mac-main.list.row.0")
    c.click("mac-main.reader.open")
    c.click("mac-main.pane.reader")
    time.sleep(0.3)


@case("esc-from-the-reader-gives-the-keys-to-the-list", "esc-stays-in-the-reader")
def _(c):
    open_reader(c)
    before = (c.uistate().get("surface"), shell(c)["focused_pane"])
    real(c, "escape")
    after = settle(lambda: (c.uistate().get("surface"), shell(c)["focused_pane"]), ("mac-main", "list"))
    i0 = c.uistate().get("focus_index")
    real(c, "j")
    i1 = settle(lambda: c.uistate().get("focus_index"), i0 + 1)
    return (before[1], after, i1 - i0), ("reader", ("mac-main", "list"), 1)


@case("esc-back-to-the-list-draws-no-outline-around-it", "esc-outlines-the-list")
def _(c):
    """Esc out of an open mail leaves the list as it rests: the selected row marks the focus and no outline is drawn around the list column (the shell's
    `focus_by_key`, which draws it, is for a move between panes by the arrow keys). Control: after the Esc the list holds the keys."""
    open_reader(c)
    real(c, "escape")
    after = settle(lambda: shell(c)["focused_pane"], "list")
    return (after, bool(shell(c).get("focus_by_key"))), ("list", False)


@case("esc-from-the-empty-reply-closes-it", "esc-leaves-the-reply-open")
def _(c):
    open_reader(c)
    real(c, "r")
    opened = settle(lambda: reply_open(c), True)
    real(c, "escape")
    closed = settle(lambda: reply_open(c), False)
    return (opened, closed, c.uistate().get("surface")), (True, False, "mac-reader")


@case("esc-with-text-keeps-the-draft", "esc-drops-the-typed-words")
def _(c):
    open_reader(c)
    real(c, "r")
    settle(lambda: reply_open(c), True)
    real(c, "hello")
    real(c, "escape")
    closed = settle(lambda: reply_open(c), False)
    real(c, "r")
    again = settle(lambda: reply_open(c), True)
    text = (c.rows("mac-main.quickreply.field") or [{}])[0].get("text", "")
    return (closed, again, "hello" in str(text)), (False, True, True)


DRAFT = "mac-main.thread.draft"


@case("esc-passes-a-thread-with-a-draft-card", "esc-stuck-behind-a-draft-card")
def _(c):
    open_reader(c)
    real(c, "r")
    settle(lambda: reply_open(c), True)
    real(c, "hello")
    real(c, "escape")                                   # the words are the conversation's draft now: the reply is closed, its card shows
    closed = settle(lambda: reply_open(c), False)
    cards = settle(lambda: len(c.rows(DRAFT)), 1)
    real(c, "escape")
    after = settle(lambda: (c.uistate().get("surface"), shell(c)["focused_pane"]), ("mac-main", "list"))
    i0 = c.uistate().get("focus_index")
    real(c, "j")
    i1 = settle(lambda: c.uistate().get("focus_index"), i0 + 1)
    c.click("mac-main.list.row.0")                      # back to the conversation: its draft card is still there
    c.click("mac-main.reader.open")
    kept = settle(lambda: len(c.rows(DRAFT)), 1)
    c.click(DRAFT + ".0.delete")                        # leave no draft behind
    return (closed, cards, after, i1 - i0, kept), (False, 1, ("mac-main", "list"), 1, 1)


@case("the-bar-names-the-key-while-a-mail-is-open", "bar-has-no-back-hint")
def _(c):
    open_reader(c)
    with_mail = [(r["command"], r["key"], r["words"]) for r in foot(c) if r["command"] == "back" and r["shown"]]
    real(c, "escape")
    settle(lambda: c.uistate().get("surface"), "mac-main")
    at_list = [r["command"] for r in foot(c) if r["command"] == "back" and r["shown"]]
    return (with_mail, at_list), ([("back", "Escape", "Back to list")], [])


def run(c):
    out = []
    for name, _m, fn in CASES:
        label = "%s/%s" % (SCREEN, name)
        try:
            got, want = fn(c)
            out.append((label, "PASS" if got == want else "FAIL", "expected %r, got %r" % (want, got)))
        except Unreadable as e:
            out.append((label, "NOT JUDGED", str(e)))
        except (KeyError, IndexError, TypeError) as e:
            out.append((label, "FAIL", "reply shape: %s %s" % (type(e).__name__, e)))
    return out


# ---------------------------------------------------------------- the stand-in (self-test only)
class Model:
    def __init__(self, mut=None):
        self.m = mut
        self.reset()

    def reset(self):
        self.surface, self.pane, self.reply, self.text, self.idx, self.draft = "mac-main", "list", False, "", 0, ""
        self.by_key = False

    def send(self, verb, *a):
        if verb == "fixture":
            self.reset()
        elif verb == "keyevent":
            self.key(a[-1])
        return {}

    def click(self, ax):
        if ax == "mac-main.reader.open":
            self.surface = "mac-reader"
        elif ax == "mac-main.thread.draft.0.delete":
            self.draft = ""
        elif ax == "mac-main.pane.reader":
            self.pane = "reader"
        elif ax == "mac-main.pane.list":
            self.pane = "list"

    def key(self, k):
        if k == "escape":
            if self.reply:
                if self.m != "esc-leaves-the-reply-open":
                    self.reply = False
                    if self.m != "esc-drops-the-typed-words":
                        self.draft = self.text
                    self.text = ""
            elif self.surface == "mac-reader" and self.m != "esc-stays-in-the-reader" and not (self.draft and self.m == "esc-stuck-behind-a-draft-card"):
                self.surface, self.pane = "mac-main", "list"
                self.by_key = self.m == "esc-outlines-the-list"
        elif k == "r" and self.surface == "mac-reader":
            self.reply, self.text = True, self.draft
        elif k == "j" and self.pane == "list":
            self.idx += 1
        elif self.reply and len(k) > 1 and k not in ("escape",):
            self.text += k

    def uistate(self):
        return {"surface": self.surface, "focus_index": self.idx}

    def rows(self, ax):
        if ax == "mac-main.shell":
            return [{"focused_pane": self.pane, "focus_by_key": self.by_key}]
        if ax == "mac-main.quickreply.field":
            return [{"text": self.text}] if self.reply else []
        if ax == "mac-main.thread.draft":
            return [{"lines": self.draft}] if self.draft and not self.reply and self.surface == "mac-reader" else []
        if ax == "mac-main.foot.hints":
            on = self.surface == "mac-reader" and self.m != "bar-has-no-back-hint"
            return [{"command": "back", "key": "Escape", "words": "Back to list", "shown": on}]
        raise Unreadable("no rows at %s" % ax)


def selftest():
    bad = []
    for label, st, d in run(Model()):
        print("model %-62s %s %s" % (label, st, d[:90]))
        bad += [] if st == "PASS" else ["clean model: %s %s" % (label, d)]
    for name, mut, _fn in CASES:
        res = run(Model(mut))
        red = [l for l, st, _ in res if st == "FAIL"]
        ok = "%s/%s" % (SCREEN, name) in red          # a broken copy turns its own case red; cases that start from the same Esc may follow it
        print("broken %-44s red: %s" % (mut, ", ".join(l.split("/")[1] for l in red) or "NONE"))
        bad += [] if ok else ["broken copy %s: red %s" % (mut, red)]
    for b in bad:
        print("PROBLEM " + b)
    print("selftest: %d problem(s)" % len(bad))
    return 1 if bad else 0


def main():
    if "--selftest" in sys.argv:
        return selftest()
    try:
        c = Ctl().connect()
    except CouldNotDrive as e:
        res = [("%s/%s" % (SCREEN, n), "NOT JUDGED", str(e)) for n, _m, _f in CASES]
    else:
        try:
            res = run(c)
        finally:
            c.close()
    for label, st, d in res:
        print("%-10s %s  %s" % (st, label, d))
    f, u = sum(r[1] == "FAIL" for r in res), sum(r[1] == "NOT JUDGED" for r in res)
    print("%d checks, %d failed, %d not judged" % (len(res), f, u))
    return 1 if f or u else 0


if __name__ == "__main__":
    sys.exit(main())
