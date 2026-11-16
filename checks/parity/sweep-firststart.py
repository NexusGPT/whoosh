#!/usr/bin/env python3
"""sweep-firststart: the control sweep of the first-start flow `mac-first-start` (the ten steps over the main window: Continue, Back, Skip, Add account,
the try-it field of step 2, the try-it keys of step 3, the three keys Enter, Esc and cmd-[, and the AI step's doors).

Owner, 2026-11-11: "the email sender account selector button isn't working btw, you haven't properly tested every button/feature".
Every control of the flow, clicked or pressed ONCE through the door the app registers, its visible outcome read through the control socket.
The import step, the Reponses step and the AI setup marks are the Welcome window's own screens: their controls are swept by
checks/parity/par-welcomemac.py (`welcomemac-sweep-*`, docs/lanes/sweep-welcomemac.md); this file does not click them twice, and
docs/COVERAGE.md "Control sweep: First start" lists them with their case or finding.

    sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/sweep-firststart.py'
    python3 checks/parity/sweep-firststart.py --selftest     a model of the flow: every case green, then one broken copy per case (no app, no socket)

Prints one line per case `name  pass|fail|unjudged  detail` (print_cases of journeys/lib/harness.py), then 'N checks, F failed, U not judged'.
The flow is opened with `fixture first-start on` and closed with `fixture first-start off` (docs/CONTROL-SOCKET.md delta 58); the account of the
Add account case is never created (only the sheet opening is read); the AI doors run on the fixture core (`fixture providers none`): no network.
"""
import os
import re
import shutil
import sys
import importlib.util
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable      # noqa: E402
from lib.harness import print_cases, declared_mutants   # noqa: E402

# The loopback stub of the decision model's five files (class Stub("laya"), 127.0.0.1 only): imported from the Settings AI check, never copied.
_sspec = importlib.util.spec_from_file_location("sai", os.path.join(ROOT, "checks", "settingsaimac-check.py"))
sai = importlib.util.module_from_spec(_sspec)
_argv, sys.argv = sys.argv, [sys.argv[0]]
_sspec.loader.exec_module(sai)
sys.argv = _argv

SCREEN = "sweep-firststart"
CASES = []                                                # (name, door, words, fn)
DEPENDENT = {}                                            # case -> cases whose set-up clicks that case's control (red on its broken copy too, named here)


def case(name, door, words):
    def deco(fn):
        CASES.append((name, door, words, fn))
        return fn
    return deco


class BaseModel:
    """The self-test's stand-in for the app. A broken copy `sw-<case>` makes the control of that case do nothing: `dead(door)` is true."""
    def __init__(self, m=()):
        self.m = set(m)
        self.door_case = {}
        for name, door, _w, _f in CASES:
            self.door_case.setdefault(door, []).append(name)
        self.reset_state()

    def dead(self, door):
        return any("sw-" + n in self.m for n in self.door_case.get(door, ()))

    def reset_state(self):
        pass

    def send(self, verb, *a):
        if verb == "click":
            return self.click(a[0])
        if verb == "key":
            return self.key(a[0])
        if verb == "type":
            return self.type(a[0])
        if verb == "fixture":
            return self.fixture(*a)
        raise Unreadable("model: verb %s not served" % verb)

    def ok(self, **kw):
        return dict({"ok": True}, **kw)

    def fixture(self, *a):
        raise Unreadable("model: fixture %s not served" % (a[0],))

    def type(self, text):
        raise Unreadable("model: type not served")

    def key(self, chord):
        raise Unreadable("model: key %s: no binding" % chord)

    def uistate(self):
        raise Unreadable("model: uistate not served")

    def core(self, path):
        raise Unreadable("model: core %s not served" % path)


class Shim:
    """The Ctl calls the cases use, over a model (the real Ctl is used on the app)."""
    def __init__(self, model):
        self.m = model

    def send(self, verb, *a):
        return self.m.send(verb, *a)

    def click(self, ax):
        return self.m.click(ax)

    def key(self, chord):
        return self.m.key(chord)

    def type(self, text):
        return self.m.type(text)

    def uistate(self):
        return self.m.uistate()

    def rows(self, ax):
        return self.m.rows(ax)

    def core(self, path):
        return self.m.core(path)


X = "mac-first-start"
ADD = "mac-add-account"


def st(c):
    return c.rows(X + ".state")[0]


def opened(c, step=1):
    """The flow open at `step`, reached with the Continue door (the case's own control is clicked after this)."""
    c.send("fixture", "first-start", "on")
    for _ in range(step - 1):
        c.click(X + ".continue")


def closed(c):
    c.send("fixture", "first-start", "off")


@case("continue-goes-to-the-next-step", X + ".continue", "Continue does nothing: the flow stays on step 1")
def _(c):
    opened(c)
    before = st(c)["step"]
    c.click(X + ".continue")
    after = st(c)["step"]
    closed(c)
    return (before, after), (1, 2)


@case("back-returns-to-the-step-before", X + ".back", "Back does nothing: the flow stays on step 2")
def _(c):
    opened(c, 2)
    before = st(c)["step"]
    c.click(X + ".back")
    after = st(c)["step"]
    closed(c)
    return (before, after), (2, 1)


@case("continue-at-the-add-step-needs-an-account", X + ".continue (no account)", "Continue at step 6 goes on with no account: the flow leaves the add-account step empty-handed")
def _(c):
    c.send("fixture", "accounts", "none")          # the core lists no account (F1: the door the sweep lacked; the flow counts the core's list, then the ones the add card saved since it opened)
    try:
        opened(c, 6)
        enabled = [x["enabled"] for x in c.rows(X + ".controls") if x["id"] == "continue"]
        try:
            reply = c.click(X + ".continue")
            pressed = not (isinstance(reply, dict) and reply.get("ok") is False)
        except Unreadable:
            pressed = False                         # the door answers ok:false: the press did nothing
        step = st(c)["step"]
        closed(c)
    finally:
        c.send("fixture", "accounts", "all")
    return (enabled, pressed, step), ([False], False, 6)


@case("skip-jumps-to-the-add-account-step", X + ".skip", "Skip does nothing: the flow stays on step 1")
def _(c):
    opened(c)
    before = (st(c)["step"], st(c)["skipped"])
    c.click(X + ".skip")
    after = (st(c)["step"], st(c)["skipped"])
    closed(c)
    return (before, after), ((1, False), (6, False))


@case("skip-at-the-add-step-leaves-the-empty-inbox", X + ".skip", "Skip at step 6 does nothing: the flow stays on the add-account step")
def _(c):
    opened(c, 6)
    before = (st(c)["step"], st(c)["skipped"])
    c.click(X + ".skip")
    after = (st(c)["skipped"], st(c)["lifted"])
    closed(c)
    return (before, after), ((6, False), (True, True))


@case("add-account-opens-the-sheet", X + ".add-account", "Add account does nothing: no sheet opens from the empty inbox")
def _(c):
    opened(c, 6)
    c.click(X + ".skip")
    before = c.uistate().get("surface")
    c.click(X + ".add-account")
    after = c.uistate().get("surface")
    c.click(ADD + ".cancel")
    closed(c)
    return (before, after), ("mac-main", ADD)


@case("try-it-field-finds-by-meaning", "type", "typing in the step-2 field reaches no field: the match line stays empty")
def _(c):
    opened(c, 2)
    before = c.rows(X + ".search")[0]["match"]
    c.type("boat")
    row = c.rows(X + ".search")[0]
    closed(c)
    return (before, row["typed"], row["match"]), ("", "boat", "Sailing club")


def key_case(k, setup, want, words):
    @case("try-it-key-%s-%s" % (k.lower(), words[0]), "key " + k, words[1])
    def _(c):
        opened(c, 3)
        for s in setup:
            c.key(s)
        before = c.rows(X + ".keys")[0]
        c.key(k)
        after = c.rows(X + ".keys")[0]
        closed(c)
        return want(before, after)


key_case("J", (), lambda b, a: ((b["focus"], a["focus"]), (0, 1)), ("moves-the-focus-down", "J does nothing: the focus stays on the first sample line"))
key_case("K", ("J",), lambda b, a: ((b["focus"], a["focus"]), (1, 0)), ("moves-the-focus-up", "K does nothing: the focus stays on the second sample line"))
key_case("E", (), lambda b, a: ((len(b["rows"]) - len(a["rows"]), a["last"]), (1, "done")), ("marks-the-line-done", "E does nothing: the sample line stays and nothing is marked done"))
key_case("H", (), lambda b, a: ((len(b["rows"]) - len(a["rows"]), a["last"]), (1, "remind")), ("reminds-about-the-line", "H does nothing: the sample line stays and nothing is marked as reminded"))


@case("enter-key-continues", "key Enter", "Enter does nothing: the flow stays on step 1")
def _(c):
    opened(c)
    c.key("Enter")
    after = st(c)["step"]
    closed(c)
    return after, 2


@case("escape-key-skips", "key Escape", "Esc does nothing: the flow stays on step 1")
def _(c):
    opened(c)
    c.key("Escape")
    after = st(c)["step"]
    closed(c)
    return after, 6


@case("command-bracket-key-goes-back", "key ⌘[", "cmd-[ does nothing: the flow stays on step 2")
def _(c):
    opened(c, 2)
    c.key("⌘[")
    after = st(c)["step"]
    closed(c)
    return after, 1


def need(c, name):
    return [r for r in c.rows("ai-setup.status") if r["need"] == name][0]


def ai_open(c):
    opened(c, 7)
    c.send("fixture", "providers", "none")
    c.click("ai-setup.reset")


def sorting(c):
    return [r for r in c.rows(X + ".ai") if r["id"] == "sorting"][0]


@case("ai-writing-key-connects-a-provider", X + ".ai.writing.key", "the pasted-key door does nothing: the writing model stays 'no provider'")
def _(c):
    ai_open(c)
    before = need(c, "writing")["reason"]
    c.click(X + ".ai.writing.key")
    c.send("clock", "advance", "1")
    after = need(c, "writing")["reason"]
    c.send("fixture", "providers", "default")
    closed(c)
    return (before, after != "provider-none"), ("provider-none", True)


def meaning_missing(c):
    """The search model not on this Mac, as a new install has it: the rig run folder's weights emptied (never the owner's folder) and the engine's
    embedder unloaded (`fixture embedder unloaded`). Without this the rig copy reads 'ready' (main88: expected 'missing', got 'ready') and the
    Download door has nothing to start. The stand-in model has no run folder (`dir`), so only the verb is sent there."""
    run = getattr(c, "dir", None)
    if run is not None:
        if not run or not os.path.exists(os.path.join(run, "whoosh.sock")):
            raise Unreadable("no rig run folder (WHOOSH_SOCKET_DIR): the models folder to empty is unknown")
        owner = os.path.realpath(os.path.expanduser("~/Library/Application Support/Whoosh"))
        folder = os.path.realpath(os.path.join(run, "models", "embeddinggemma-2"))
        if folder == owner or folder.startswith(owner + os.sep):
            raise Unreadable("the models folder %s is the owner's, never emptied by a check" % folder)
        shutil.rmtree(folder, ignore_errors=True)
        if os.path.exists(folder):
            raise Unreadable("the rig's models folder %s could not be emptied" % folder)
    c.send("fixture", "embedder", "unloaded")


@case("ai-meaning-download-starts", X + ".ai.meaning.download", "the search model's Download does nothing: its line keeps saying it is missing")
def _(c):
    ai_open(c)
    meaning_missing(c)
    before = need(c, "meaning")["state"]
    c.click(X + ".ai.meaning.download")
    after = need(c, "meaning")["state"]
    c.send("fixture", "embedder", "loaded")
    c.send("fixture", "providers", "default")
    closed(c)
    return (before, after != "missing"), ("missing", True)


def laya_fresh(c):
    """The decision model not on this Mac: its weights live on the disk (`fixture reset` keeps them), so the rig copy's OWN `<run folder>/Models/laya` is emptied
    (the owner's folder is refused; the same step as settingsaimac-check.py `_laya_fresh`). The stand-in model has no run folder (`dir`): nothing to empty."""
    run = getattr(c, "dir", None)
    if run is None:
        return
    if not run or not os.path.exists(os.path.join(run, "whoosh.sock")):
        raise Unreadable("no rig run folder (WHOOSH_SOCKET_DIR): the models folder to empty is unknown")
    owner = os.path.realpath(os.path.expanduser("~/Library/Application Support/Whoosh"))
    folder = os.path.realpath(os.path.join(run, "Models", "laya"))
    if folder == owner or folder.startswith(owner + os.sep):
        raise Unreadable("the models folder %s is the owner's, never emptied by a check" % folder)
    shutil.rmtree(folder, ignore_errors=True)
    if os.path.exists(folder):
        raise Unreadable("the rig's models folder %s could not be emptied" % folder)


@case("ai-sorting-download-starts-the-decision-model", X + ".ai.sorting.download", "the decision model's Download does nothing: its line keeps saying it is not on this Mac")
def _(c):
    ai_open(c)
    laya_fresh(c)
    stub = sai.Stub("laya")
    try:
        c.send("fixture", "provider-endpoint", "laya", stub.url)      # `fixture provider-endpoint laya <loopback url>` (AIFixture.swift): the weights come from the stub, never from outside this Mac
        before = sorting(c)
        if before["state"] != "missing":
            raise Unreadable("the sorting line reads %r after the models folder was emptied, not 'missing': the Download cannot be tried from a known state" % (before["state"],))
        c.click(X + ".ai.sorting.download")
        state = before["state"]
        for _ in range(30):                  # the line leaves 'missing' (moving figures, then ready) once the download runs
            state = sorting(c)["state"]
            if state != "missing":
                break
            time.sleep(0.2)
        for _ in range(50):                  # let a running download end before the stub goes
            if sorting(c)["state"] != "working":
                break
            time.sleep(0.2)
        reached = any(m == "GET" for m, _p in stub.gets)
    finally:
        try:
            c.send("fixture", "provider-endpoint", "laya", "")
        except Unreadable:
            pass
        stub.close()
        c.send("fixture", "providers", "default")
        closed(c)
    return (before["state"], state in ("working", "ready"), reached), ("missing", True, True)


@case("ai-use-jev-asks-for-the-key", X + ".ai.sorting.use-jev", "Use Jev does nothing: the sorting line still offers Download and Use Jev")
def _(c):
    ai_open(c)
    laya_fresh(c)                            # the line offers Use Jev only while the model is not on this Mac; an earlier case's download may have left it there
    before = sorting(c)["actions"]
    c.click(X + ".ai.sorting.use-jev")
    after = sorting(c)["actions"]
    c.send("fixture", "providers", "default")
    closed(c)
    return (before, after), (["use-jev", "download"], ["jev-key"])


@case("ai-jev-key-asks-for-the-consent", X + ".ai.sorting.jev-key", "the Jev key door does nothing: the line keeps asking for the key")
def _(c):
    ai_open(c)
    laya_fresh(c)
    c.click(X + ".ai.sorting.use-jev")
    before = sorting(c)["actions"]
    c.click(X + ".ai.sorting.jev-key")
    after = sorting(c)["actions"]
    c.send("fixture", "providers", "default")
    closed(c)
    return (before, after), (["jev-key"], ["jev-allow"])


@case("ai-jev-allow-chooses-jev", X + ".ai.sorting.jev-allow", "Allow does nothing: the line keeps asking for the consent")
def _(c):
    ai_open(c)
    laya_fresh(c)
    c.click(X + ".ai.sorting.use-jev")
    c.click(X + ".ai.sorting.jev-key")
    before = sorting(c)["actions"]
    c.click(X + ".ai.sorting.jev-allow")
    after = sorting(c)["actions"]
    c.send("fixture", "providers", "default")
    closed(c)
    return (before, after), (["jev-allow"], [])


DEPENDENT.update({
    "continue-goes-to-the-next-step": ["continue-at-the-add-step-needs-an-account", "back-returns-to-the-step-before", "skip-at-the-add-step-leaves-the-empty-inbox", "add-account-opens-the-sheet", "try-it-field-finds-by-meaning",
                                       "try-it-key-j-moves-the-focus-down", "try-it-key-k-moves-the-focus-up", "try-it-key-e-marks-the-line-done", "try-it-key-h-reminds-about-the-line",
                                       "command-bracket-key-goes-back", "ai-writing-key-connects-a-provider", "ai-meaning-download-starts", "ai-sorting-download-starts-the-decision-model", "ai-use-jev-asks-for-the-key",
                                       "ai-jev-key-asks-for-the-consent", "ai-jev-allow-chooses-jev"],
    "skip-jumps-to-the-add-account-step": ["skip-at-the-add-step-leaves-the-empty-inbox", "add-account-opens-the-sheet"],
    "skip-at-the-add-step-leaves-the-empty-inbox": ["add-account-opens-the-sheet", "skip-jumps-to-the-add-account-step"],
    "try-it-key-j-moves-the-focus-down": ["try-it-key-k-moves-the-focus-up"],
    "ai-use-jev-asks-for-the-key": ["ai-jev-key-asks-for-the-consent", "ai-jev-allow-chooses-jev"],
    "ai-jev-key-asks-for-the-consent": ["ai-jev-allow-chooses-jev"],
})

# Controls with no door, no drawn control or no readable outcome, and the screens the Welcome sweep owns: (title, file, line, a snippet that must be on that line, why).
FINDINGS = [
    ("F2 sorting cancel (the decision model)", "Sources/Features/FirstStart/FirstStartAIRows.swift", 56, 'door("sorting.cancel") { AISetupDoors.cancel(.decision) },',
     "Cancel needs a download that runs long enough to be stopped (the stub's slow pieces); it is clicked only by par-firststart.py (`ai-cancel-stops-the-decision-download`), not in this file. The Download itself has its case here (`ai-sorting-download-starts-the-decision-model`)"),
    ("F3 the step-5 optional switches (Jev key, Push relay)", "Sources/Features/FirstStart/FirstStartRows.swift", 76, 'row["optional"] = [["name": FirstStartWords.jevKey, "on": false], ["name": FirstStartWords.pushRelay, "on": false]]',
     "step 5 draws two switches that are always off and read from a constant row: no control, no door"),
    ("F4 the step dots", "Sources/Features/FirstStart/FirstStartControls.swift", 80, ".accessibilityLabel(Text(FirstStartWords.stepOf(current)))",
     "the eight dots (`mac-first-start.dots.<n>`) are a display, not buttons: no click moves the flow"),
    ("F5 the import step's panel controls", "Sources/Features/FirstStart/FirstStartRows.swift", 53, "t.merge(AISetupRows.clickTargets()) { a, _ in a }",
     "the import panel (scan, Choose, source rows, Into, Start, Cancel, Skip, Go to inbox, Open Full Disk Access), the Reponses step choices and the AI setup marks are swept in par-welcomemac.py: welcomemac-sweep-* (docs/lanes/sweep-welcomemac.md: 65 controls, 44 covered, 21 findings F-a to F-m); not repeated here"),
    ("F6 the stage keys C, / and the other swallowed keys", "Sources/Features/FirstStart/FirstStartFeature.swift", 21, 'static let swallowed = ["C", "/", "Z"',
     "the flow swallows 17 keys so they do not act behind the veil; a swallowed key has no outcome to read except that the inbox rows stay (par-firststart.py `keys-stay-out`), and is not a control"),
]


class Model(BaseModel):
    def reset_state(self):
        self.active, self.step, self.skipped, self.lifted = False, 1, False, False
        self.typed, self.rows_, self.focus, self.last, self.surface = "", ["Linea Supply", "Marie Lefevre", "Ton Verheyden"], 0, "", "mac-main"
        self.jev, self.writing, self.meaning = "none", "provider-none", "missing"
        self.sorting, self.endpoint = "missing", None

    def fixture(self, *a):
        if a[0] == "accounts":
            self.noacc = a[1] == "none"
            return self.ok()
        if a[0] == "first-start":
            if a[1] == "on":
                self.reset_state()
                self.active = True
            elif a[1] == "off":
                self.reset_state()
            return self.ok()
        if a[0] == "providers":
            self.writing = "provider-none" if a[1] == "none" else "ok"
            return self.ok()
        if a[0] == "embedder":
            return self.ok()
        if a[0] == "provider-endpoint" and a[1] == "laya":
            self.endpoint = a[2] or None
            return self.ok()
        return super().fixture(*a)

    def send(self, verb, *a):
        if verb == "clock":
            return self.ok()
        return super().send(verb, *a)

    def go(self, n):
        self.step = n
        if n == 3:
            self.focus, self.rows_, self.last = 0, ["Linea Supply", "Marie Lefevre", "Ton Verheyden"], ""

    def nxt(self):
        if not self.active or self.skipped:
            return False
        if self.step == 6 and getattr(self, "noacc", False) and "sw-continue-at-the-add-step-needs-an-account" not in self.m:
            return False                      # step 6 goes on only once an account exists (FirstStartModel.next)
        if self.step < 9:
            self.go(self.step + 1)
            return True
        return False

    def back(self):
        if not self.active or self.skipped or self.step <= 1 or self.step > 9:
            return False
        self.go(self.step - 1)
        return True

    def skip(self):
        if not self.active or self.skipped:
            return False
        if self.step <= 5:
            self.go(6)
        elif self.step == 6:
            self.skipped = self.lifted = True
        else:
            self.go(self.step + 1)
        return True

    def reply(self, ok):
        return {"ok": True} if ok else {"ok": False, "error": "did nothing"}

    def click(self, ax):
        d = self.dead(ax)
        if ax == X + ".continue":
            return self.reply(False if d else self.nxt())
        if ax == X + ".back":
            return self.reply(False if d else self.back())
        if ax == X + ".skip":
            return self.reply(False if d else self.skip())
        if ax == X + ".add-account":
            if d or not self.skipped:
                return self.reply(False)
            self.surface = ADD
            return self.reply(True)
        if ax == ADD + ".cancel":
            self.surface = "mac-main"
            return self.reply(True)
        if ax == "ai-setup.reset":
            self.jev, self.meaning = "none", "missing"
            return self.reply(True)
        if ax.startswith(X + ".ai."):
            if not (self.active and self.step == 7) or d:
                return self.reply(False)
            k = ax[len(X) + 4:]
            if k == "writing.key":
                self.writing = "ok"
            elif k == "meaning.download":
                self.meaning = "working"
            elif k == "sorting.download":
                self.fetch()
            elif k == "sorting.use-jev":
                self.jev = "key"
            elif k == "sorting.jev-key" and self.jev == "key":
                self.jev = "consent"
            elif k == "sorting.jev-allow" and self.jev == "consent":
                self.jev = "chosen"
            return self.reply(True)
        raise Unreadable("click %s: nothing registered" % ax)

    def fetch(self):
        """The Download: each of the model's five files is asked of the staged address (a loopback stub only: any other address is refused here)."""
        import urllib.request
        if not self.endpoint or not self.endpoint.startswith("http://127.0.0.1:"):
            return
        try:
            for f in sai.LAYA_FILES:
                urllib.request.urlopen(self.endpoint + "/" + f, timeout=5).read()
            self.sorting = "ready"
        except OSError:
            self.sorting = "missing"

    def key(self, chord):
        d = self.dead("key " + chord)
        if chord == "Enter":
            return self.reply(False if d else self.nxt())
        if chord == "Escape":
            return self.reply(False if d else (self.active and self.skip()))
        if chord == "⌘[":
            return self.reply(False if d else self.back())
        if chord in ("J", "K", "E", "H") and self.active and self.step == 3:
            if d:
                return self.reply(True)       # the key reaches a handler that does nothing
            if chord == "J":
                self.focus = min(len(self.rows_) - 1, self.focus + 1)
            elif chord == "K":
                self.focus = max(0, self.focus - 1)
            else:
                self.rows_.pop(self.focus)
                self.last = "done" if chord == "E" else "remind"
                self.focus = min(self.focus, max(0, len(self.rows_) - 1))
            return self.reply(True)
        return self.reply(False)

    def type(self, text):
        if not self.dead("type") and self.active and self.step == 2:
            self.typed += text
        return self.ok()

    def uistate(self):
        return {"surface": self.surface}

    def rows(self, ax):
        if ax == X + ".state":
            return [{"step": self.step, "skipped": self.skipped, "lifted": self.lifted, "active": self.active}]
        if ax == X + ".controls":
            return [{"id": "continue", "enabled": not (self.step == 6 and getattr(self, "noacc", False))}]
        if ax == X + ".search":
            return [{"typed": self.typed, "match": "Sailing club" if "boat" in self.typed else ""}]
        if ax == X + ".keys":
            return [{"rows": list(self.rows_), "focus": self.focus, "last": self.last}]
        if ax == "ai-setup.status":
            return [{"need": "writing", "state": "ready" if self.writing == "ok" else "missing", "reason": self.writing},
                    {"need": "meaning", "state": self.meaning, "reason": "meaning-missing"}]
        if ax == X + ".ai":
            acts = {"none": ["use-jev", "download"], "key": ["jev-key"], "consent": ["jev-allow"], "chosen": []}[self.jev]
            return [{"id": "sorting", "state": self.sorting, "actions": acts, "jev": self.jev}]
        raise Unreadable("no rows at %s" % ax)


# MUTANTS: per case, the one-line change in the app that turns it red: `label | file | find (occurs once in the file) | replace`.
# sweep-firststart/continue-goes-to-the-next-step | Sources/Features/FirstStart/FirstStartRows.swift | "\(X).continue": { _ in m.next() }, | "\(X).continue": { _ in true },
# sweep-firststart/continue-at-the-add-step-needs-an-account | Sources/Features/FirstStart/FirstStartModel.swift | guard accountCount > 0 else { return false } | _ = accountCount
# sweep-firststart/back-returns-to-the-step-before | Sources/Features/FirstStart/FirstStartRows.swift | "\(X).back": { _ in m.back() }, | "\(X).back": { _ in true },
# sweep-firststart/skip-jumps-to-the-add-account-step | Sources/Features/FirstStart/FirstStartRows.swift | "\(X).skip": { _ in m.skip() }, | "\(X).skip": { _ in true },
# sweep-firststart/skip-at-the-add-step-leaves-the-empty-inbox | Sources/Features/FirstStart/FirstStartRows.swift | "\(X).skip": { _ in m.skip() }, | "\(X).skip": { _ in m.step == FirstStartModel.addStep ? true : m.skip() },
# sweep-firststart/add-account-opens-the-sheet | Sources/Features/FirstStart/FirstStartRows.swift | "\(X).add-account": { _ in m.addAccountFromEmptyInbox() }, | "\(X).add-account": { _ in true },
# sweep-firststart/try-it-field-finds-by-meaning | Sources/Features/FirstStart/FirstStartFeature.swift | public static func typeText(_ text: String, _ app: AppModel) -> Bool { m.tryType(text) } | public static func typeText(_ text: String, _ app: AppModel) -> Bool { false }
# sweep-firststart/try-it-key-j-moves-the-focus-down | Sources/Features/FirstStart/FirstStartFeature.swift | for k in ["J", "K", "E", "H"] { out[k] = { _ in m.tryKey(k) } } | for k in ["J", "K", "E", "H"] { out[k] = { _ in k == "J" ? true : m.tryKey(k) } }
# sweep-firststart/try-it-key-k-moves-the-focus-up | Sources/Features/FirstStart/FirstStartFeature.swift | for k in ["J", "K", "E", "H"] { out[k] = { _ in m.tryKey(k) } } | for k in ["J", "K", "E", "H"] { out[k] = { _ in k == "K" ? true : m.tryKey(k) } }
# sweep-firststart/try-it-key-e-marks-the-line-done | Sources/Features/FirstStart/FirstStartFeature.swift | for k in ["J", "K", "E", "H"] { out[k] = { _ in m.tryKey(k) } } | for k in ["J", "K", "E", "H"] { out[k] = { _ in k == "E" ? true : m.tryKey(k) } }
# sweep-firststart/try-it-key-h-reminds-about-the-line | Sources/Features/FirstStart/FirstStartFeature.swift | for k in ["J", "K", "E", "H"] { out[k] = { _ in m.tryKey(k) } } | for k in ["J", "K", "E", "H"] { out[k] = { _ in k == "H" ? true : m.tryKey(k) } }
# sweep-firststart/enter-key-continues | Sources/Features/FirstStart/FirstStartFeature.swift | "Enter": { _ in enter() }, | "Enter": { _ in false },
# sweep-firststart/escape-key-skips | Sources/Features/FirstStart/FirstStartFeature.swift | "Escape": { _ in m.active && m.skip() }, | "Escape": { _ in false },
# sweep-firststart/command-bracket-key-goes-back | Sources/Features/FirstStart/FirstStartFeature.swift | "⌘[": { _ in m.back() }, | "⌘[": { _ in false },
# sweep-firststart/ai-writing-key-connects-a-provider | Sources/Features/FirstStart/FirstStartAIRows.swift | door("writing.key", fixtureOnly: true) { step.pasteWritingKey(key) }, | door("writing.key", fixtureOnly: true) { },
# sweep-firststart/ai-meaning-download-starts | Sources/Features/FirstStart/FirstStartAIRows.swift | door("meaning.download") { AISetupDoors.fix(.meaning) }, | door("meaning.download") { },
# sweep-firststart/ai-sorting-download-starts-the-decision-model | Sources/Features/FirstStart/FirstStartAIRows.swift | door("sorting.download") { step.download() }, | door("sorting.download") { },
# sweep-firststart/ai-use-jev-asks-for-the-key | Sources/Features/FirstStart/FirstStartAIRows.swift | door("sorting.use-jev") { step.openJev() }, | door("sorting.use-jev") { },
# sweep-firststart/ai-jev-key-asks-for-the-consent | Sources/Features/FirstStart/FirstStartAIRows.swift | door("sorting.jev-key", fixtureOnly: true) { step.submitJevKey(key) }, | door("sorting.jev-key", fixtureOnly: true) { },
# sweep-firststart/ai-jev-allow-chooses-jev | Sources/Features/FirstStart/FirstStartAIRows.swift | door("sorting.jev-allow") { step.allowJev() }, | door("sorting.jev-allow") { },


def run(c):
    out = []
    for name, _door, _w, fn in CASES:
        label = "%s/%s" % (SCREEN, name)
        try:
            got, want = fn(c)[:2]
            out.append((label, "pass" if got == want else "fail", "expected %r, got %r" % (want, got)))
        except Unreadable as e:
            out.append((label, "unjudged", str(e)))
        except (KeyError, IndexError, TypeError) as e:
            out.append((label, "fail", "reply shape: %s %s" % (type(e).__name__, e)))
    return out


def mutant_lines():
    rows = []
    for line in open(os.path.abspath(__file__), encoding="utf-8"):
        m = re.match(r"# (" + SCREEN + r"/[\w-]+) \| (\S+) \| (.+?) \| (.+)$", line.rstrip("\n"))
        if m:
            rows.append(m.groups())
    return rows


def selftest():
    bad = []
    names = [n for n, *_ in CASES]
    clean = run(Shim(Model()))
    for label, st, d in clean:
        print("model %-44s %-8s %s" % (label.split("/", 1)[1][:44], st, d[:100]))
        bad += [] if st == "pass" else ["clean model: %s is %s: %s" % (label, st, d)]
    seen = 0
    for n in names:
        res = run(Shim(Model({"sw-" + n})))
        red = sorted([l.split("/", 1)[1] for l, st, _ in res if st == "fail"], key=lambda x: x != n)
        ok = red[:1] == [n] and set(red) <= {n} | set(DEPENDENT.get(n, ()))
        seen += ok
        print("broken %-46s red: %s" % (n, ", ".join(red) or "NONE"))
        bad += [] if ok else ["broken copy of %s: red cases %s" % (n, red)]
    declared = set(declared_mutants(os.path.abspath(__file__)))
    want = {"%s/%s" % (SCREEN, n) for n in names}
    lines = mutant_lines()
    if declared != want or len(lines) != len(names):
        bad.append("MUTANTS block: declared %d, cases %d, missing %s, stray %s" % (len(declared), len(want), sorted(want - declared), sorted(declared - want)))
    for label, path, find, repl in lines:
        text = open(os.path.join(ROOT, path), encoding="utf-8").read()
        if text.count(find) != 1:
            bad.append("%s: the find text occurs %d times in %s (must be once)" % (label, text.count(find), path))
        if find == repl:
            bad.append("%s: replace equals find" % label)
    for title, path, line, snippet, _why in FINDINGS:
        got = open(os.path.join(ROOT, path), encoding="utf-8").read().split("\n")[line - 1]
        if snippet.strip() not in got:
            bad.append("finding %s: line %d of %s reads %r, not the snippet" % (title, line, path, got.strip()[:80]))
    print("%d cases, %d seen red on their broken model, %d MUTANTS rows checked against the files (find occurs once), %d findings checked at their line" % (len(names), seen, len(lines), len(FINDINGS)))
    for b in bad:
        print("PROBLEM " + b)
    print("selftest: every check seen red on its broken model" if not bad else "selftest: %d problem(s)" % len(bad))
    return 1 if bad else 0


def main():
    if "--selftest" in sys.argv:
        return selftest()
    try:
        c = Ctl().connect()
    except CouldNotDrive as e:
        res = [("%s/%s" % (SCREEN, n), "unjudged", str(e)) for n, *_ in CASES]
    else:
        try:
            pre = globals().get("PRE_RUN")
            if pre:
                pre(c)
            res = run(c)
        finally:
            post = globals().get("POST_RUN")
            if post:
                post()
            c.close()
    print_cases({"cases": res})
    f, u = sum(r[1] == "fail" for r in res), sum(r[1] == "unjudged" for r in res)
    print("%d checks, %d failed, %d not judged" % (len(res), f, u))
    return 1 if f or u else 0


if __name__ == "__main__":
    sys.exit(main())
