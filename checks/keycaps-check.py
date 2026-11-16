#!/usr/bin/env python3
"""keycaps-check: every control a screen serves in its probe region shows its key (a key cap or a tooltip carrying the chord).

    python3 checks/keycaps-check.py             drive the running rig copy; exit 1 when F > 0 or U > 0
    python3 checks/keycaps-check.py --selftest  the check on a model screen set, clean and with a mutant per screen

The owner: "every control shows its key". A control "has a binding" when the Superhuman preset binds its command
(KeyPreset.swift, read by checks/superhuman-v8-check.py; nothing typed here). Per screen the check reads the rows the
screen's probe region serves, finds each control's command, and requires the preset's chord in the text the control
shows (key, cap, chord, tooltip or help). A screen that serves no such region is a red of its own: the screen lane adds
the region (the sweep list in docs/lanes/keys-r4.md). Every absence carries its positive control: a region that serves
no row, or no control with a binding, is red, never a vacuous green.

Expected values come from the preset source, never from a built screen.
Run live: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/keycaps-check.py'
"""
import importlib.util
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, Journey               # noqa: E402
from lib.ctl import Unreadable                              # noqa: E402


def _load(name, rel):
    spec = importlib.util.spec_from_file_location(name, os.path.join(ROOT, rel))
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


PK = _load("par_keys", "checks/parity/par-keys.py")
V8 = PK.V8

ID = "keycaps"
TITLE = "Every control shows its key"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "each control of each screen that the Superhuman preset binds carries its chord as a key cap or in its tooltip"

# the composer's controls and the command each runs (the owner's list in docs/lanes/composermac.md, par-composermac "every control shows its key")
COMPOSER_AX = {"mac-composer.attach": "attach", "mac-composer.from": "compose-from", "mac-composer.send": "send",
               "mac-composer.send-later": "send-later", "mac-composer.cc.show": "compose-cc", "mac-composer.bcc.show": "compose-bcc",
               "mac-composer.discard": "discard-draft", "mac-composer.snippets": "use-snippet",
               "mac-composer.format-bar.bold": "bold", "mac-composer.format-bar.italic": "italic", "mac-composer.format-bar.underline": "underline",
               "mac-composer.format-bar.strike": "strikethrough", "mac-composer.format-bar.link": "hyperlink", "mac-composer.format-bar.color": "color",
               "mac-composer.format-bar.list": "bullet-list", "mac-composer.format-bar.numbers": "numbered-list", "mac-composer.format-bar.quote": "quote"}

# screens the Superhuman preset binds controls on, whose probe region this check cannot read today: the sweep list.
# (screen, what it has that a key reaches, why no row is read)
UNREAD_SCREENS = [
    ("search", "Enter opens the result, Tab narrows, Esc leaves", "mac-search serves bar and results rows, no control rows with a key or tooltip"),
    ("labels move and label pickers", "L, V, Y, [, ] and the picker rows", "mac-labels.move serves targets only, no key text per control"),
    ("sidebar mailboxes", "G then a letter for each mailbox row", "mac-main.sidebar.mailboxes serves no key or tooltip per row"),
    ("trainer key list", "each row is its own key", "read by shortcuts-check; not a control"),
]

SYMBOL = {"Enter": ["↵", "Return", "Enter"], "Escape": ["⎋", "Esc", "Escape"], "Tab": ["⇥", "Tab"], "Up": ["↑", "Up"], "Down": ["↓", "Down"], "Left": ["←", "Left"], "Right": ["→", "Right"],   # the preset itself spells ⌘Up, so the word is the preset's own spelling (as Enter, Escape and Tab already allow)
          "Space": ["Space", "␣"], "Backspace": ["⌫"]}


def chord_of(command, preset):
    """The chord (or 'G then X') the Superhuman preset shows for a command, rows first; None when it binds none."""
    for c, cid in preset["rows"] + preset["also"]:
        if cid == command:
            return c
    for a, b, cid in preset["sequences"]:
        if cid == command:
            return PK.label_of((a, b))
    for c, cid in preset["fixed"]:
        if cid == command:
            return c
    return None


def spellings(chord):
    """Every way a screen may spell a chord: modifiers in either order, the key as a glyph or a word."""
    if " then " in chord:
        a, b = chord.split(" then ")
        return {chord, "%s %s" % (a, b), a + b}
    if chord.startswith("Shift+"):
        key = chord[len("Shift+"):]
        return {"⇧" + k for k in SYMBOL.get(key, [key])} | {"Shift+" + key}
    mods = "".join(m for m in chord if m in "⌃⌥⇧⌘")
    key = chord[len(mods):]
    out = set()
    for k in SYMBOL.get(key, [key]):
        out.add(mods + k)
        out.add("".join(sorted(mods, key="⌃⌥⌘⇧".index)) + k)          # ⌘⇧ for ⇧⌘
        if mods:
            out.add("".join(sorted(mods, key="⌃⌥⇧⌘".index)) + k)
    return out


def shows(texts, chord):
    blob = " ".join(str(t) for t in texts if t)
    return any(s in blob for s in spellings(chord))


# ---------------------------------------------------------------- the screens: what each region serves, read as (control, command, texts)

def read_list(k):
    st = k.c.rows("mac-main.list.state")[0]
    out = [(i["id"], i["id"], [i.get("key"), i.get("help")]) for i in st["strip"]]
    out.append(("unread-chip", "filter-unread", [st["unread_chip"].get("key")]))
    return out


def read_reader_strip(k):
    return [(r["ax"], r["command"], [r.get("key"), r.get("tooltip")]) for r in k.c.rows("mac-main.reader.strip")]


def read_reader_header(k):
    return [(r.get("ax", "header"), "toggle-message-header", [r.get("tooltip")]) for r in k.c.rows("mac-main.reader.header")]


def read_toolbar(k):
    return [(r["ax"], r["command"], [r.get("tooltip")]) for r in k.c.rows("mac-main.toolbar")]


def read_composer(k):
    rows = {r["ax"]: r for r in k.c.rows("mac-composer.controls")}
    return [(ax, cmd, [rows[ax].get("tooltip"), rows[ax].get("cap") if isinstance(rows[ax].get("cap"), str) else None] if ax in rows else None)
            for ax, cmd in COMPOSER_AX.items()]


def read_sidebar(k):
    r = k.c.rows("mac-main.sidebar.visibility")[0]
    return [("sidebar-toggle", "sidebar", [r.get("chord"), r.get("tooltip")])]


def read_quick(k):
    send = k.c.rows("mac-main.quickreply.send")
    frm = k.c.rows("mac-main.quickreply.from")
    head = k.c.rows("mac-main.quickreply.header")
    return ([("quickreply.send", "send", [r.get("cap"), r.get("help")]) for r in send]
            + [("quickreply.from", "compose-from", [r.get("cap")]) for r in frm]
            + [("quickreply.header", "toggle-message-header", [r.get("cap")]) for r in head])


def _command_by_title():
    """Commands.swift's own registry: display title (first letter capitalised, as PaletteCatalog.display draws it) -> command id."""
    src = open(os.path.join(ROOT, "Sources", "Features", "Shortcuts", "Commands.swift"), encoding="utf-8").read()
    out = {}
    for cid, title in re.findall(r'(?:live|stub)\("([^"]+)",\s*"([^"]+)"', src):
        out.setdefault(title[:1].upper() + title[1:], cid)
    return out


def read_palette(k):
    """A palette line's id is `command:<Title>` (PaletteCatalog.commands); the command the preset binds is the registry's id for that title."""
    by_title = _command_by_title()
    out = []
    for r in k.c.rows("mac-palette.results"):
        rid = r.get("id", "")
        cmd = by_title.get(rid[len("command:"):], rid) if rid.startswith("command:") else rid
        out.append((r["ax"], cmd, [r.get("key")]))
    return out


SCREENS = [   # (check name, setup, reader)
    ("list-strip-and-chip", "list", read_list),
    ("reader-strip", "reader", read_reader_strip),
    ("reader-header-chevron", "reader", read_reader_header),
    ("window-toolbar", "list", read_toolbar),
    ("composer-controls", "composer", read_composer),
    ("sidebar-toggle", "list", read_sidebar),
    ("quick-reply", "quick", read_quick),
    ("palette-lines", "palette", read_palette),
]


def setup(k, name):
    if name == "list":
        k.home()
    elif name == "reader":
        k.reader()
    elif name == "composer":
        k.composer()
    elif name == "quick":
        k.reader(); k.key("R")
    elif name == "palette":
        k.home(); k.key("⌘P"); k.type("")


def offenders(k, reader, preset):
    """(missing, bound): the controls whose command the preset binds but whose text lacks the chord, and how many are bound."""
    missing, bound = [], 0
    for ctl, command, texts in reader(k):
        chord = chord_of(command, preset)
        if chord is None:
            continue
        bound += 1
        if texts is None:
            missing.append("%s: not served" % ctl)
        elif not shows(texts, chord):
            missing.append("%s (%s): shows %s, wants %s" % (ctl, command, [t for t in texts if t], chord))
    return missing, bound


def drive(j):
    preset = V8.preset()
    for name, setup_name, reader in SCREENS:
        s = j.step("keycaps-%s" % name, name)
        k = PK.K(s)
        try:
            setup(k, setup_name)
            result = offenders(k, reader, preset)
        except Unreadable as e:
            from lib.harness import Check
            s.checks.append(Check("%s/every-bound-control-shows-its-key" % s.id, "unjudged", str(e)))
            continue
        except (KeyError, IndexError, TypeError) as e:
            from lib.harness import Check
            s.checks.append(Check("%s/every-bound-control-shows-its-key" % s.id, "fail", "no probe region or reply shape: %s %s" % (type(e).__name__, e)))
            continue
        s.expect("every-bound-control-shows-its-key", lambda r=result: (r[0], r[1] > 0), ([], True))


# MUTANTS: per check, the mutation of the app that must turn it red.
# keycaps-list-strip-and-chip/every-bound-control-shows-its-key | a row action (archive, snooze, label, pin) draws no key, or the Unread chip loses ⇧U
# keycaps-reader-strip/every-bound-control-shows-its-key | a strip control's tooltip drops its key
# keycaps-reader-header-chevron/every-bound-control-shows-its-key | the header chevron's tooltip drops ⇧H
# keycaps-window-toolbar/every-bound-control-shows-its-key | a toolbar control's tooltip drops its key
# keycaps-composer-controls/every-bound-control-shows-its-key | one composer control's tooltip drops its key, or the control is not served
# keycaps-sidebar-toggle/every-bound-control-shows-its-key | the sidebar toggle's tooltip drops ⌘\
# keycaps-quick-reply/every-bound-control-shows-its-key | the quick reply's send, from or header cap is empty
# keycaps-palette-lines/every-bound-control-shows-its-key | a palette line shows no key


# ---------------------------------------------------------------- the selftest: a model of the screens above

class Screens:
    """The probe regions of the screens as the lanes should serve them, built from the preset; a mutant drops one key."""

    def __init__(self, mutant=None):
        self.m = mutant
        self.p = V8.preset()

    def tip(self, command, title, screen):
        c = chord_of(command, self.p)
        if self.m == screen:
            return title
        return "%s  %s" % (title, c) if c else title

    def rows(self, ax):
        p = self.p
        if ax == "mac-main.list.state":
            ids = ["archive", "snooze", "label", "pin"]
            strip = [{"id": i, "key": (None if self.m == "list-strip-and-chip" and i == "snooze" else chord_of(i, p)), "help": (i if self.m == "list-strip-and-chip" and i == "snooze" else self.tip(i, i, "x"))} for i in ids]
            return [{"strip": strip, "unread_chip": {"key": chord_of("filter-unread", p)}}]
        if ax == "mac-main.reader.strip":
            return [{"ax": "mac-main.reader.strip." + c, "command": c, "key": (None if self.m == "reader-strip" else chord_of(c, p)), "tooltip": self.tip(c, c, "reader-strip")}
                    for c in ("delete", "reply", "reply-all", "forward", "archive")]
        if ax == "mac-main.reader.header":
            return [{"ax": "mac-main.reader.header.0", "tooltip": self.tip("toggle-message-header", "Expand message header", "reader-header-chevron")}]
        if ax == "mac-main.toolbar":
            return [{"ax": "mac-main.toolbar." + c, "command": c, "tooltip": self.tip(c, c, "window-toolbar")} for c in ("compose", "reply", "archive", "snooze", "label")]
        if ax == "mac-composer.controls":
            rows = [{"ax": a, "tooltip": self.tip(c, c, "composer-controls"), "cap": False} for a, c in COMPOSER_AX.items()]
            return [r for r in rows if not (self.m == "composer-controls-gone" and r["ax"] == "mac-composer.send-later")]
        if ax == "mac-main.sidebar.visibility":
            return [{"chord": "", "tooltip": self.tip("sidebar", "Hide the sidebar", "sidebar-toggle")}]
        if ax == "mac-main.quickreply.send":
            return [{"cap": "" if self.m == "quick-reply" else "⌘↵", "help": "Send" if self.m == "quick-reply" else "Send (⌘↵)"}]
        if ax == "mac-main.quickreply.from":
            return [{"cap": chord_of("compose-from", p)}]
        if ax == "mac-main.quickreply.header":
            return [{"cap": chord_of("toggle-message-header", p)}]
        if ax == "mac-palette.results":
            return [{"ax": "mac-palette.results.%d" % i, "id": c, "key": ("" if self.m == "palette-lines" and c == "archive" else chord_of(c, p) or "")}
                    for i, c in enumerate(("archive", "snooze", "label", "palette"))]
        raise Unreadable("no rows at %s" % ax)


class ScreensCtl:
    def __init__(self, screens):
        self.s = screens
        self.surface = "mac-main"

    def rows(self, ax):
        return self.s.rows(ax)

    def uistate(self):
        return {"surface": self.surface}

    def key(self, chord):
        return {}

    def click(self, ax):
        return {}

    def type(self, text):
        return {}

    def send(self, *a):
        return {}

    def winframes(self):
        return []


class StubK(PK.K):
    """The setups of the live run, with nothing to drive: the model screens are always in the state the region needs."""
    def home(self):
        pass

    def reader(self, mid="msg-01"):
        pass

    def composer(self, mid="msg-01"):
        pass


def run_model(mutant):
    j = Journey(ID, ScreensCtl(Screens(mutant)))
    global PK_K
    saved = PK.K
    PK.K = StubK
    try:
        drive(j)
    finally:
        PK.K = saved
    return j.checks()


def selftest():
    bad = []
    cs = run_model(None)
    print("clean model: %d checks, %d not green" % (len(cs), sum(1 for c in cs if c.state != "pass")))
    for c in cs:
        if c.state != "pass":
            print("  NOT GREEN %s: %s" % (c.label, c.detail))
            bad.append("clean model not green: " + c.label)
    mutants = [("list-strip-and-chip", "keycaps-list-strip-and-chip"), ("reader-strip", "keycaps-reader-strip"),
               ("reader-header-chevron", "keycaps-reader-header-chevron"), ("window-toolbar", "keycaps-window-toolbar"),
               ("composer-controls", "keycaps-composer-controls"), ("composer-controls-gone", "keycaps-composer-controls"),
               ("sidebar-toggle", "keycaps-sidebar-toggle"), ("quick-reply", "keycaps-quick-reply"), ("palette-lines", "keycaps-palette-lines")]
    print("| mutant | step | red there | red elsewhere |")
    print("|---|---|---|---|")
    for m, step in mutants:
        cs = run_model(m)
        there = [c for c in cs if c.state != "pass" and c.label.startswith(step + "/")]
        other = [c for c in cs if c.state != "pass" and not c.label.startswith(step + "/")]
        print("| %s | %s | %d | %d |" % (m, step, len(there), len(other)))
        if not there:
            bad.append("mutant %s turned %s no red" % (m, step))
        if other:
            bad.append("mutant %s turned other steps red: %s" % (m, [c.label for c in other]))
    # the chord matcher: a spelling in either modifier order matches; a missing or different chord does not
    for text, chord, want in (("Archive  E", "E", True), ("Reply all  ⌘⇧F", "⇧⌘F", True), ("Send  ⌘↵", "⌘Enter", True),
                              ("Send", "⌘Enter", False), ("Bold  ⌘I", "⌘B", False), ("Pin  ⇧P", "⇧P", True), ("Previous  ⇧⇥", "Shift+Tab", True)):
        if shows([text], chord) != want:
            bad.append("matcher: %r vs %s should be %s" % (text, chord, want))
    # the positive control: a region with no bound control is red, not green
    j = Journey(ID, ScreensCtl(Screens(None)))
    ok = offenders(StubK(j.step("x", "x")), lambda k: [("c", "no-such-command", ["x"])], V8.preset())
    if ok[1] != 0:
        bad.append("a control with no binding was counted as bound")
    for line in bad:
        print("SELFTEST FAIL: " + line)
    print("keycaps selftest: %s (%d problems)" % ("green" if not bad else "RED", len(bad)))
    return 0 if not bad else 1


def main():
    if "--selftest" in sys.argv:
        return selftest()
    path = os.path.abspath(__file__)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    rows = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# keycaps") and " | " in l]
    print("MUTANTS (a check is green only after it has been seen red on its mutant)")
    print("| check | mutant |")
    print("|---|---|")
    for kk, v in rows:
        print("| %s | %s |" % (kk, v))
    for line in r["fail_list"]:
        sys.stderr.write("FAIL %s\n" % line)
    for line in r["no_mutant"]:
        sys.stderr.write("NO MUTANT %s\n" % line)
    for line in r["stray_mutant"]:
        sys.stderr.write("STRAY MUTANT %s\n" % line)
    print("screens the check cannot read yet (the sweep list):")
    for name, what, why in UNREAD_SCREENS:
        print("  - %s: %s; %s" % (name, what, why))
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    ok = r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"]
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
