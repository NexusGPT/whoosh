#!/usr/bin/env python3
"""sweep-trainer: the control sweep of the trainer `mac-trainer` (the hint after a mouse action, the keys overlay, the hints switch).

Owner, 2026-11-11: "the email sender account selector button isn't working btw, you haven't properly tested every button/feature".
Every control of the screen, clicked ONCE through the door the app registers, its visible outcome read through the control socket.
The table is docs/COVERAGE.md "Control sweep: Trainer"; the notes are docs/lanes/gap-sweep-a.md.

    sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/sweep-trainer.py'
    python3 checks/parity/sweep-trainer.py --selftest     a model of the screen: every case green, then one broken copy per case (no app, no socket)

Prints one line per case `name  pass|fail|unjudged  detail` (print_cases of journeys/lib/harness.py), then 'N checks, F failed, U not judged'.
Assumption named, not hidden: the trainer counts key uses in the copy's own preferences (the rig identity keeps them between runs); a row action
case reads a hint only while that command has fewer than TRAINER_HINT_STOP_AFTER key uses, so a copy that already learned a key shows no hint
(the case then fails on 'no hint'; the lead's hidden run starts from a fresh run folder). Expected words come from TrainerFeature.rowActions
and the hint row's own `command` field, never from a built screen.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable      # noqa: E402
from lib.harness import print_cases, declared_mutants   # noqa: E402

SCREEN = "sweep-trainer"
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

    def measure(self, ax):
        return self.m.measure(ax)

    def core(self, path):
        return self.m.core(path)


T = "mac-trainer"
OVERLAY = "mac-trainer.keys"
ACTIONS = ("archive", "snooze", "label", "pin")
ROW = 4                                                   # a list row well inside the fixture's first page (trainer-check.py uses row 4 too)


def hint(c):
    return c.rows(T + ".hint")


def hints_state(c):
    return c.rows(T + ".settings")[0]["hints"]


def reset(c):
    """The screen as a person finds it: nothing open, hints on, no hint showing, the first rows listed."""
    c.send("fixture", "reset")
    c.click(T + ".hints.on")
    if c.uistate().get("overlay") is not None:
        c.key("Escape")


@case("hints-on-turns-the-hints-on", T + ".hints.on", "the hints-on control does nothing: the switch stays off")
def _(c):
    reset(c)
    c.click(T + ".hints.off")
    before = hints_state(c)
    c.click(T + ".hints.on")
    after = hints_state(c)
    reset(c)
    return (before, after), ("off", "on")


@case("hints-off-turns-the-hints-off", T + ".hints.off", "the hints-off control does nothing: the switch stays on")
def _(c):
    reset(c)
    before = hints_state(c)
    c.click(T + ".hints.off")
    after = hints_state(c)
    reset(c)
    return (before, after), ("on", "off")


def action_case(a):
    @case("row-%s-shows-the-hint" % a, "mac-main.list.row.%d.%s" % (ROW, a), "the %s button of a list row does nothing: no hint follows" % a)
    def _(c):
        reset(c)
        before = len(hint(c))
        c.click("mac-main.list.row.%d.%s" % (ROW, a))
        rows = hint(c)
        got = (before, rows[0]["command"] if rows else None, bool(rows) and rows[0]["text"].startswith("Next time, press "))
        if c.uistate().get("overlay") is not None:         # snooze and label open a picker: put it away
            c.key("Escape")
        reset(c)
        return got, (0, a, True)


for _a in ACTIONS:
    action_case(_a)


@case("palette-turn-shortcut-hints-off-turns-the-hints-off", "mac-palette.result.turn-shortcut-hints-off", "the palette line 'Turn shortcut hints off' does nothing: the switch stays on")
def _(c):
    reset(c)
    before = hints_state(c)
    c.click("mac-palette.result.turn-shortcut-hints-off")
    after = hints_state(c)
    reset(c)
    return (before, after), ("on", "off")


@case("palette-turn-shortcut-hints-on-turns-the-hints-on", "mac-palette.result.turn-shortcut-hints-on", "the palette line 'Turn shortcut hints on' does nothing: the switch stays off")
def _(c):
    reset(c)
    c.click(T + ".hints.off")
    before = hints_state(c)
    c.click("mac-palette.result.turn-shortcut-hints-on")
    after = hints_state(c)
    reset(c)
    return (before, after), ("off", "on")


@case("hint-dismiss-takes-the-hint-away", T + ".hint.dismiss", "the hint's x button does nothing: the hint stays on screen")
def _(c):
    reset(c)
    c.click("mac-main.list.row.%d.archive" % ROW)
    before = len(hint(c))
    c.click(T + ".hint.dismiss")
    after = len(hint(c))
    reset(c)
    return (before, after), (1, 0)


@case("palette-show-all-keys-opens-the-overlay", "mac-palette.result.show-all-keys", "the palette line 'Show all keys' opens nothing")
def _(c):
    reset(c)
    before = c.uistate().get("overlay")
    c.click("mac-palette.result.show-all-keys")
    st = c.uistate()
    got = (before, st.get("overlay"), st.get("surface"), len(c.rows(T + ".overlay.keys")) > 0)
    c.key("Escape")
    reset(c)
    return got, (None, OVERLAY, T, True)


@case("show-keys-command-opens-then-closes-the-overlay", "mac-shortcuts.action.keys", "the show-keys command does nothing: the overlay never opens, or a second press never closes it")
def _(c):
    reset(c)
    c.click("mac-shortcuts.action.keys")
    opened = c.uistate().get("overlay")
    c.click("mac-shortcuts.action.keys")
    closed = (c.uistate().get("overlay"), c.uistate().get("surface"))
    reset(c)
    return (opened, closed), (OVERLAY, (None, "mac-main"))


@case("overlay-scrim-closes-the-overlay", T + ".overlay.close", "a click on the dimmed window (the scrim) leaves the keys overlay open")
def _(c):
    reset(c)
    c.click("mac-palette.result.show-all-keys")
    before = c.uistate().get("overlay")
    c.click(T + ".overlay.close")
    st = c.uistate()
    got = (before, st.get("overlay"), st.get("surface"))
    reset(c)
    return got, (OVERLAY, None, "mac-main")


@case("overlay-escape-closes-the-overlay", "key Escape", "Escape leaves the keys overlay open")
def _(c):
    reset(c)
    c.click("mac-palette.result.show-all-keys")
    before = c.uistate().get("overlay")
    c.key("Escape")
    st = c.uistate()
    got = (before, st.get("overlay"), st.get("surface"))
    reset(c)
    return got, (OVERLAY, None, "mac-main")


def drawn(c, ax):
    """True when `measure <ax>` answers a frame with a size; False when nothing on screen carries the id (the reply says so)."""
    try:
        f = c.measure(ax).get("frame") or {}
    except Unreadable as e:
        if "no measurement" in str(e):
            return False
        raise
    return f.get("w", 0) > 0 and f.get("h", 0) > 0


@case("hint-dismiss-button-is-drawn-with-its-own-id", "measure " + T + ".hint.dismiss", "the hint's x button carries no id the socket can measure")
def _(c):
    reset(c)
    c.click("mac-main.list.row.%d.archive" % ROW)
    got = drawn(c, T + ".hint.dismiss")
    reset(c)
    return got, True


@case("overlay-scrim-is-drawn-with-its-own-id", "measure " + T + ".overlay.close", "the dimmed window (the scrim) carries no id the socket can measure")
def _(c):
    reset(c)
    c.click("mac-palette.result.show-all-keys")
    got = drawn(c, T + ".overlay.close")
    c.key("Escape")
    reset(c)
    return got, True


# A case whose set-up needs another control of the same screen is red on that control's broken copy too.
DEPENDENT.update({"palette-show-all-keys-opens-the-overlay": ["overlay-scrim-closes-the-overlay", "overlay-escape-closes-the-overlay"],
                  "hints-off-turns-the-hints-off": ["hints-on-turns-the-hints-on", "palette-turn-shortcut-hints-on-turns-the-hints-on"],
                  "palette-turn-shortcut-hints-off-turns-the-hints-off": ["hints-on-turns-the-hints-on"],
                  "palette-turn-shortcut-hints-on-turns-the-hints-on": ["hints-off-turns-the-hints-off", "hints-on-turns-the-hints-on"],
                  "row-archive-shows-the-hint": ["hint-dismiss-takes-the-hint-away", "hint-dismiss-button-is-drawn-with-its-own-id"]})
DEPENDENT["palette-show-all-keys-opens-the-overlay"] += ["overlay-scrim-is-drawn-with-its-own-id"]

# Controls with no door, no drawn id or no readable outcome: findings (title, file, line, a snippet that must be on that line, why).
# F1 (the palette lines 'Turn shortcut hints off|on' had no door) is two driven cases now: palette-turn-shortcut-hints-off-turns-the-hints-off and palette-turn-shortcut-hints-on-turns-the-hints-on.
# F2 (the hint x has no drawn id) and F3 (the scrim has no id) are driven cases now: hint-dismiss-button-is-drawn-with-its-own-id, overlay-scrim-is-drawn-with-its-own-id.
FINDINGS = [
    ("F4 stage doors are rig doors", "Sources/Features/Trainer/TrainerFeature.swift", 59, 't["mac-trainer.stage.light"] = { _ in TrainerStage.shared.show(dark: false) }',
     "`mac-trainer.stage.light|dark|wide|narrow` open the design-shot stage window: a rig door, no control a person has; not counted"),
]


class Model(BaseModel):
    def reset_state(self):
        self.hints, self.shown, self.overlay, self.surface = True, None, None, "mac-main"

    def fixture(self, *a):
        if a[0] == "reset":
            self.reset_state()
            return self.ok()
        return super().fixture(*a)

    def click(self, ax):
        if ax == T + ".hints.on":
            if not self.dead(ax):
                self.hints = True
        elif ax == T + ".hints.off":
            if not self.dead(ax):
                self.hints, self.shown = False, None
        elif ax == "mac-palette.result.turn-shortcut-hints-off":
            if not self.dead(ax):
                self.hints, self.shown = False, None
        elif ax == "mac-palette.result.turn-shortcut-hints-on":
            if not self.dead(ax):
                self.hints = True
        elif ax.startswith("mac-main.list.row.%d." % ROW):
            a = ax.rsplit(".", 1)[1]
            if a in ACTIONS and not self.dead(ax):
                self.shown = {"command": a, "text": "Next time, press E to %s" % a} if self.hints else None
        elif ax == T + ".hint.dismiss":
            if self.shown is None:
                return {"ok": False, "error": "did nothing"}
            if not self.dead(ax):
                self.shown = None
        elif ax == "mac-palette.result.show-all-keys":
            if not self.dead(ax):
                self.overlay, self.surface, self.shown = OVERLAY, T, None
        elif ax == "mac-shortcuts.action.keys":
            if not self.dead(ax):
                if self.overlay:
                    self.overlay, self.surface = None, "mac-main"
                else:
                    self.overlay, self.surface, self.shown = OVERLAY, T, None
        elif ax == T + ".overlay.close":
            if self.overlay is None:
                return {"ok": False, "error": "did nothing"}
            if not self.dead(ax):
                self.overlay, self.surface = None, "mac-main"
        else:
            raise Unreadable("click %s: nothing registered" % ax)
        return {"ok": True}

    def key(self, chord):
        if chord == "Escape":
            if self.overlay and not self.dead("key Escape"):
                self.overlay, self.surface = None, "mac-main"
            return {"ok": True}
        return super().key(chord)

    def uistate(self):
        return {"surface": self.surface, "overlay": self.overlay}

    def measure(self, ax):
        if ax == T + ".hint.dismiss":
            shown = self.shown is not None and self.hints and not self.overlay
            ok = shown and not self.dead("measure " + ax)
        elif ax == T + ".overlay.close":
            ok = self.overlay is not None and not self.dead("measure " + ax)
        else:
            raise Unreadable("measure %s: not served" % ax)
        if not ok:
            raise Unreadable("measure ('%s',): no measurement at %s: nothing on screen carries that id" % (ax, ax))
        return {"ok": True, "frame": {"x": 1, "y": 1, "w": 20, "h": 20}}

    def rows(self, ax):
        if ax == T + ".hint":
            return [self.shown] if self.shown and self.hints and not self.overlay else []
        if ax == T + ".settings":
            return [{"hints": "on" if self.hints else "off"}]
        if ax == T + ".overlay.keys":
            return [{"key": "E", "action": "Archive"}] if self.overlay else []
        raise Unreadable("no rows at %s" % ax)


# MUTANTS: per case, the one-line change in the app that turns it red: `label | file | find (occurs once in the file) | replace`.
# sweep-trainer/hints-on-turns-the-hints-on | Sources/Features/Trainer/TrainerFeature.swift | "mac-trainer.hints.on": { _ in model.hintsOn = true; return true }, | "mac-trainer.hints.on": { _ in return true },
# sweep-trainer/hints-off-turns-the-hints-off | Sources/Features/Trainer/TrainerFeature.swift | "mac-trainer.hints.off": { _ in model.hintsOn = false; return true }, | "mac-trainer.hints.off": { _ in return true },
# sweep-trainer/palette-turn-shortcut-hints-off-turns-the-hints-off | Sources/Features/Trainer/TrainerFeature.swift | "mac-palette.result.turn-shortcut-hints-off": { _ in model.hintsOn = false; return true }, | "mac-palette.result.turn-shortcut-hints-off": { _ in return true },
# sweep-trainer/palette-turn-shortcut-hints-on-turns-the-hints-on | Sources/Features/Trainer/TrainerFeature.swift | "mac-palette.result.turn-shortcut-hints-on": { _ in model.hintsOn = true; return true }, | "mac-palette.result.turn-shortcut-hints-on": { _ in return true },
# sweep-trainer/row-archive-shows-the-hint | Sources/Features/Trainer/TrainerFeature.swift | guard m.rows.indices.contains(i) else { return false } | guard m.rows.indices.contains(i), a != "archive" else { return false }
# sweep-trainer/row-snooze-shows-the-hint | Sources/Features/Trainer/TrainerFeature.swift | guard m.rows.indices.contains(i) else { return false } | guard m.rows.indices.contains(i), a != "snooze" else { return false }
# sweep-trainer/row-label-shows-the-hint | Sources/Features/Trainer/TrainerFeature.swift | guard m.rows.indices.contains(i) else { return false } | guard m.rows.indices.contains(i), a != "label" else { return false }
# sweep-trainer/row-pin-shows-the-hint | Sources/Features/Trainer/TrainerFeature.swift | guard m.rows.indices.contains(i) else { return false } | guard m.rows.indices.contains(i), a != "pin" else { return false }
# sweep-trainer/hint-dismiss-takes-the-hint-away | Sources/Features/Trainer/TrainerFeature.swift | model.dismiss(); return true | return true
# sweep-trainer/palette-show-all-keys-opens-the-overlay | Sources/Features/Shortcuts/ShortcutsFeature.swift | out["mac-palette.result.show-all-keys"] = { m in ShortcutEngine.shared.click("keys", m) } | out["mac-palette.result.show-all-keys"] = { m in false }
# sweep-trainer/show-keys-command-opens-then-closes-the-overlay | Sources/Features/Shortcuts/ShortcutsFeature.swift | out["mac-shortcuts.action.\(id)"] = { m in ShortcutEngine.shared.click(id, m) } | out["mac-shortcuts.action.\(id)"] = { m in id == "keys" ? false : ShortcutEngine.shared.click(id, m) }
# sweep-trainer/overlay-scrim-closes-the-overlay | Sources/Features/Trainer/TrainerFeature.swift | "mac-trainer.overlay.close": { model.close($0) }, | "mac-trainer.overlay.close": { _ in false },
# sweep-trainer/overlay-escape-closes-the-overlay | Sources/App/BaseFeature.swift | "Escape": { m in m.overlay = nil; m.surface = "mac-main"; return true }, | "Escape": { m in if m.overlay == "mac-trainer.keys" { return false }; m.overlay = nil; m.surface = "mac-main"; return true },
# sweep-trainer/hint-dismiss-button-is-drawn-with-its-own-id | Sources/Features/Trainer/TrainerHintView.swift | .whooshAx("mac-trainer.hint.dismiss")   // the x has its own id and a frame `measure` reads (the door of the same id runs the same dismiss) | .accessibilityIdentifier("mac-trainer.hint.dismiss")
# sweep-trainer/overlay-scrim-is-drawn-with-its-own-id | Sources/Features/Trainer/TrainerOverlayView.swift | .whooshAx("mac-trainer.overlay.close")   // the scrim has its own id and a frame (the door of the same id runs the same close) | .accessibilityIdentifier("mac-trainer.overlay.close")


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
