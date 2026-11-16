#!/usr/bin/env python3
"""sweep-entraguide: the control sweep of the Entra guide (`mac-entra-guide`, « Get your organisation's app ID », 27 steps). Owner, 2026-11-11:
"you haven't properly tested every button/feature".

ONE case per control, clicked or pressed once, its outcome read through the rows the guide serves (`mac-entra-guide.state`, `.step`, `.controls`, `.paste`, `.values`):
the guide's own door Open, the buttons Close, Skip to paste, Back, Next (and its first-step words, Open entra.microsoft.com), Save and sign in, the three Copy buttons, the ID field
(typed text, its focus and its clear button) and the keys the buttons show: Return, Esc, ⌘[, ⌘↓, ⌘1, ⌘2, ⌘3. A last case holds the app's own control list against that inventory. The
rig's `goto.<n>` doors are not a control of the screen (they jump to a step for the shots) and are used only to place the guide.

Nothing leaves this Mac: the guide records the page it would open (`opened_links`) and the pasteboard text it would write under the fixture core; the case that presses Next on step 1
reads `uistate.core` first and is NOT JUDGED unless it says `fixture`, since on another core that press opens the real browser (EntraGuideActions.openEntra). Save and sign in hands the ID to the
add-account sheet only when the guide was opened from it; opened by its own door it has no handler (`has_save_handler`), so no sign-in starts.

    python3 checks/parity/sweep-entraguide.py              drive the running rig copy (hidden, --fixture-core)
    python3 checks/parity/sweep-entraguide.py --dry        the same code with no socket: every case is `unjudged`, the count is exact
    python3 checks/parity/sweep-entraguide.py --selftest   every case against the model of checks/parity/par-entraguide.py (imported, not edited), clean, then once per case against
                                                           its own broken copy (that one control dead): each must go red

Prints one `name  pass|fail|unjudged  detail` line per case (print_cases of journeys/lib/harness.py), the MUTANTS count, then 'N checks, F failed, U not judged'."""
import importlib.util
import json
import os
import shutil
import socketserver
import sys
import tempfile
import threading
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, Unreadable                                     # noqa: E402
from lib.harness import Journey, run_journey, print_cases, declared_mutants   # noqa: E402

_spec = importlib.util.spec_from_file_location("pe", os.path.join(ROOT, "checks", "parity", "par-entraguide.py"))
pe = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(pe)                                            # read for its rows, its helpers and its Model; never edited

ID = "par-entraguide-sweep"
TITLE = "Entra guide: every control clicked once"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "each button, field and key of the guide changes the step, the field, the pasteboard note or the open state the guide draws"
A, M, GOOD_ID, URL = pe.A, pe.M, pe.GOOD_ID, pe.URL
CASES = []      # (name, what it covers, the broken copy's switch: dead:<door> | deadkey:<chord> | extra-control, fn(ctl))


class Red(Exception):
    """The app answered and the answer is wrong: the case is red."""


def case(name, covers, noop, fn):
    CASES.append((name, covers, noop, fn))


def need(cond, words):
    if not cond:
        raise Red(words)


def hit(ctl, door):
    """One click on a door; a refusal ("did nothing") is the app's answer: False, and the outcome read after it shows the dead control."""
    try:
        ctl.click(A + "." + door)
        return True
    except Unreadable as e:
        if "nothing" in str(e):
            return False
        raise


def press(ctl, chord):
    """One key through the same path as the real keyboard. A key that does nothing is `ok:false`: the outcome read after it shows it."""
    try:
        ctl.key(chord)
        return True
    except Unreadable as e:
        if "does nothing" in str(e) or "nothing" in str(e):
            return False
        raise


state, step_row, ctl_row = pe.state, pe.step_row, pe.ctl


def at(ctl, n):
    """The guide open on step n with an empty ID field (setup, with the rig's own `goto` door)."""
    pe.fresh(ctl)
    if n > 1:
        pe.goto(ctl, n)


def fixture_core(ctl):
    core = ctl.uistate().get("core")
    if core != "fixture":
        raise Unreadable("the app's core is %r, not the fixture: pressing Next on step 1 would open the real browser; not pressed" % core)


# ---------------------------------------------------------------- the controls
def _open(ctl):
    try:
        ctl.click(pe.SHEET + ".start-over")       # the add-account sheet's own reset: the guide starts closed
    except Unreadable:
        pass
    if pe.rows(ctl, "state") and state(ctl)["open"]:
        pe.click(ctl, "close")
        pe.wait_for(ctl, lambda: not state(ctl)["open"])
    need(not state(ctl)["open"], "setup: the guide is still open")
    hit(ctl, "open")
    pe.wait_for(ctl, lambda: state(ctl)["open"])
    s = state(ctl)
    need(s["open"] and s["step"] == 1 and s["steps"] == M, "Open: open %r, step %r of %r" % (s["open"], s["step"], s["steps"]))
    return "Open: the guide is open at step %d of %d" % (s["step"], s["steps"])


def _next_first(ctl):
    fixture_core(ctl)
    at(ctl, 1)
    b = ctl_row(ctl, "next")["title"]
    hit(ctl, "next")
    s = state(ctl)
    a = ctl_row(ctl, "next")["title"]
    need(b == pe.EN["entra-guide.open-entra"] and s["step"] == 1 and s["opened_links"] == [URL] and s["opened"] and a == pe.EN["Next"],
         "Next on step 1: button %r -> %r, step %r, page opened %r, recorded links %r" % (b, a, s["step"], s["opened"], s["opened_links"]))
    return "Next on step 1: %r opened %s (recorded), the button now reads %r" % (b, s["opened_links"], a)


def _next_step(ctl):
    at(ctl, 3)
    hit(ctl, "next")
    n = state(ctl)["step"]
    need(n == 4, "Next on step 3: the guide is on step %r" % n)
    return "Next: step 3 -> %d" % n


def _back(ctl):
    at(ctl, 6)
    hit(ctl, "back")
    n = state(ctl)["step"]
    need(n == 5, "Back on step 6: the guide is on step %r" % n)
    return "Back: step 6 -> %d" % n


def _skip(ctl):
    at(ctl, 3)
    hit(ctl, "skip")
    n = state(ctl)["step"]
    need(n == M, "Skip to paste on step 3: the guide is on step %r, expected %d" % (n, M))
    return "Skip to paste: step 3 -> %d" % n


def _close(ctl):
    at(ctl, 1)
    hit(ctl, "close")
    gone = pe.wait_for(ctl, lambda: not state(ctl)["open"])
    need(gone, "Close: the guide is still open")
    return "Close: the guide shrank away"


def _copy(n, step, want_text, label):
    def fn(ctl):
        at(ctl, step)
        hit(ctl, "copy.%d" % n)
        s = state(ctl)
        t = ctl_row(ctl, "copy.%d" % n)["title"]
        need(s["clipboard"] == want_text and s["copied"] == n and t == pe.EN["Copied"], "Copy %d on step %d: clipboard %r, copied %r, the button reads %r" % (n, step, s["clipboard"], s["copied"], t))
        return "Copy %d (%s): clipboard %r, the button reads %r" % (n, label, s["clipboard"], t)
    return fn


def _save(ctl):
    at(ctl, M)
    ctl.type(GOOD_ID)
    on = ctl_row(ctl, "save")["enabled"]
    hit(ctl, "save")
    closed = pe.wait_for(ctl, lambda: not state(ctl)["open"])
    s = state(ctl)
    need(on and closed and s["last_saved"] == GOOD_ID, "Save and sign in: enabled %r, closed %r, handed over %r" % (on, closed, s["last_saved"]))
    return "Save and sign in: enabled, the guide closed, handed over %s" % s["last_saved"]


def _typed(ctl):
    at(ctl, M)
    ctl.type(GOOD_ID)
    r = ctl.rows(A + ".paste")[0]
    need(r["text"] == GOOD_ID and r["valid"] is True and r["tick"] is True, "the ID field: text %r, valid %r, tick %r" % (r["text"], r["valid"], r["tick"]))
    return "ID field: typed %s, valid, ticked" % r["text"]


def _field_focus(ctl):
    at(ctl, M)
    before = ctl.rows(A + ".paste")[0]["focused"]
    hit(ctl, "paste.field")
    after = ctl.rows(A + ".paste")[0]["focused"]
    need(after is True, "the ID field: focused %r -> %r after the click" % (before, after))
    return "ID field click: focused %r -> %r" % (before, after)


def _clear(ctl):
    at(ctl, 11)
    ctl.type(pe.WRONG_ID)
    before = ctl.rows(A + ".paste")[0]
    need(before["text"] == pe.WRONG_ID and before["clearable"] is True, "setup: the field holds %r, clearable %r" % (before["text"], before["clearable"]))
    hit(ctl, "paste.clear")
    after = ctl.rows(A + ".paste")[0]["text"]
    need(after == "", "the clear button left %r in the field" % after)
    return "clear button: field %r -> ''" % before["text"]


def _key(chord, setup_step, check, label):
    def fn(ctl):
        at(ctl, setup_step)
        press(ctl, chord)
        return check(ctl, label)
    return fn


def _is_step(n):
    def check(ctl, label):
        got = state(ctl)["step"]
        need(got == n, "%s: the guide is on step %r, expected %d" % (label, got, n))
        return "%s: step %d" % (label, got)
    return check


def _is_closed(ctl, label):
    gone = pe.wait_for(ctl, lambda: not state(ctl)["open"])
    need(gone, "%s: the guide is still open" % label)
    return "%s: the guide shrank away" % label


def _clip(want, n):
    def check(ctl, label):
        s = state(ctl)
        need(s["clipboard"] == want and s["copied"] == n, "%s: clipboard %r, copied %r" % (label, s["clipboard"], s["copied"]))
        return "%s: clipboard %r" % (label, s["clipboard"])
    return check


ALL_CONTROLS = {"close", "skip", "back", "next", "save", "copy.1", "copy.2", "copy.3"}


def _listed(ctl):
    seen = set()
    for n in (1, 5, 20, M):
        at(ctl, n)
        seen |= {x["id"] for x in pe.rows(ctl, "controls")}
    need(seen == ALL_CONTROLS, "the guide lists %s; the sweep drives %s (not driven: %s, not listed: %s)" % (sorted(seen), sorted(ALL_CONTROLS), sorted(seen - ALL_CONTROLS), sorted(ALL_CONTROLS - seen)))
    return "the guide lists %d controls over steps 1, 5, 20 and %d, every one driven by a case: %s" % (len(seen), M, sorted(seen))


case("open-door-opens-the-guide", "mac-entra-guide.open", "dead:open", _open)
case("next-on-step-one-opens-the-page", "mac-entra-guide.next (step 1, Open entra.microsoft.com)", "dead:next", _next_first)
case("next-goes-one-step-on", "mac-entra-guide.next", "dead:next", _next_step)
case("back-goes-one-step-back", "mac-entra-guide.back", "dead:back", _back)
case("skip-to-paste-lands-on-the-last-step", "mac-entra-guide.skip", "dead:skip", _skip)
case("close-shrinks-the-guide-away", "mac-entra-guide.close", "dead:close", _close)
case("copy-1-puts-the-name-on-the-clipboard", "mac-entra-guide.copy.1", "dead:copy.1", _copy(1, 5, "Whoosh", "Name"))
case("copy-2-puts-smtp-send-on-the-clipboard", "mac-entra-guide.copy.2", "dead:copy.2", _copy(2, 20, "SMTP.Send", "permission"))
case("copy-3-puts-offline-access-on-the-clipboard", "mac-entra-guide.copy.3", "dead:copy.3", _copy(3, 20, "offline_access", "permission"))
case("save-and-sign-in-hands-the-id-over", "mac-entra-guide.save", "dead:save", _save)
case("id-field-takes-the-typed-text", "the ID field (type)", "dead:type", _typed)
case("id-field-click-focuses-it", "mac-entra-guide.paste.field", "dead:paste.field", _field_focus)
case("id-field-clear-button-empties-it", "mac-entra-guide.paste.clear", "dead:paste.clear", _clear)
case("key-return-goes-one-step-on", "key Return", "deadkey:Enter", _key("Enter", 3, _is_step(4), "Return"))
case("key-escape-closes-the-guide", "key Esc", "deadkey:Escape", _key("Escape", 1, _is_closed, "Esc"))
case("key-command-bracket-goes-one-step-back", "key ⌘[", "deadkey:⌘[", _key("⌘[", 6, _is_step(5), "⌘["))
case("key-command-down-skips-to-paste", "key ⌘↓", "deadkey:⌘Down", _key("⌘Down", 3, _is_step(M), "⌘↓"))
case("key-command-1-copies-the-name", "key ⌘1", "deadkey:⌘1", _key("⌘1", 5, _clip("Whoosh", 1), "⌘1"))
case("key-command-2-copies-the-second-value", "key ⌘2", "deadkey:⌘2", _key("⌘2", 20, _clip("SMTP.Send", 2), "⌘2"))
case("key-command-3-copies-the-third-value", "key ⌘3", "deadkey:⌘3", _key("⌘3", 20, _clip("offline_access", 3), "⌘3"))
case("every-listed-control-has-a-case", "mac-entra-guide.controls", "extra-control", _listed)

# the lines the generator cannot match by their text
SRC = {
    "open-door-opens-the-guide": "EntraGuideFeature.swift:\"\\(A).open\":",
    "next-on-step-one-opens-the-page": "EntraGuideActions.swift:func openEntra()",
    "next-goes-one-step-on": "EntraGuideModel.swift:func next()",
    "back-goes-one-step-back": "EntraGuideModel.swift:func back()",
    "skip-to-paste-lands-on-the-last-step": "EntraGuideModel.swift:func skip()",
    "close-shrinks-the-guide-away": "EntraGuideModel.swift:func close()",
    "copy-1-puts-the-name-on-the-clipboard": "EntraGuideActions.swift:func copy(_ n: Int)",
    "copy-2-puts-smtp-send-on-the-clipboard": "EntraGuideActions.swift:func copy(_ n: Int)",
    "copy-3-puts-offline-access-on-the-clipboard": "EntraGuideActions.swift:func copy(_ n: Int)",
    "save-and-sign-in-hands-the-id-over": "EntraGuideActions.swift:func save()",
    "id-field-takes-the-typed-text": "EntraGuideActions.swift:func type(_ text: String)",
    "id-field-click-focuses-it": "EntraGuideActions.swift:func focusPaste()",
    "id-field-clear-button-empties-it": "EntraGuideActions.swift:func clearPasted()",
    "key-return-goes-one-step-on": "EntraGuideFeature.swift:\"Enter\":",
    "key-escape-closes-the-guide": "EntraGuideFeature.swift:\"Enter\":",
    "key-command-bracket-goes-one-step-back": "EntraGuideFeature.swift:\"⌘[\":",
    "key-command-down-skips-to-paste": "EntraGuideFeature.swift:\"⌘Down\":",
    "key-command-1-copies-the-name": "EntraGuideFeature.swift:\"⌘\\(n)\"",
    "key-command-2-copies-the-second-value": "EntraGuideFeature.swift:\"⌘\\(n)\"",
    "key-command-3-copies-the-third-value": "EntraGuideFeature.swift:\"⌘\\(n)\"",
    "every-listed-control-has-a-case": "EntraGuideRows.swift:var out: [ControlRow] = [control(\"close\"",
}
CHANGE = {n: "the control does nothing: the body of this function becomes `return false`" for n in SRC}
CHANGE["open-door-opens-the-guide"] = "the door does nothing: its closure becomes `{ _ in true }`"
for _n in ("key-return-goes-one-step-on", "key-escape-closes-the-guide", "key-command-bracket-goes-one-step-back", "key-command-down-skips-to-paste",
           "key-command-1-copies-the-name", "key-command-2-copies-the-second-value", "key-command-3-copies-the-third-value"):
    CHANGE[_n] = "the key's binding does nothing: its closure becomes `{ _ in false }` (the key is then `ok:false`, as a key bound to nothing is)"
CHANGE["key-escape-closes-the-guide"] = "the Esc binding (on the next line of the same dictionary) does nothing: its closure becomes `{ _ in false }`"
CHANGE["every-listed-control-has-a-case"] = "one more control is listed that no case drives: `out.append(control(\"help\", \"Help\", enabled: true))` is added after this line"


# ---------------------------------------------------------------- the run
def run_case(s, name, fn):
    def run():
        ctl = s.ctl
        try:
            return True, fn(ctl)
        except Red as e:
            return False, str(e)
        except pe.Failed as e:
            return False, "a control did nothing: %s" % e
    s._record(name, run)


def drive(j, only=None):
    s = j.step(ID, TITLE)
    for name, _covers, _noop, fn in CASES:
        if only is None or name == only:
            run_case(s, name, fn)


# MUTANTS: per case, the one-line change of the app that must turn it red; the selftest makes the same change in its model (a dead door).
# Format: # <step>/<case> | <file>:<line>: the change
# par-entraguide-sweep/open-door-opens-the-guide | Sources/Features/EntraGuide/EntraGuideFeature.swift:27: the door does nothing: its closure becomes `{ _ in true }` (line: "\(A).open": { m in m.surface = A; if !model().isOpen { model().open() }; return true },)
# par-entraguide-sweep/next-on-step-one-opens-the-page | Sources/Features/EntraGuide/EntraGuideActions.swift:9: the control does nothing: the body of this function becomes `return false` (line: func openEntra() -> Bool {)
# par-entraguide-sweep/next-goes-one-step-on | Sources/Features/EntraGuide/EntraGuideModel.swift:107: the control does nothing: the body of this function becomes `return false` (line: func next() -> Bool {)
# par-entraguide-sweep/back-goes-one-step-back | Sources/Features/EntraGuide/EntraGuideModel.swift:115: the control does nothing: the body of this function becomes `return false` (line: func back() -> Bool { live && index > 0 ? go(index - 1) : false })
# par-entraguide-sweep/skip-to-paste-lands-on-the-last-step | Sources/Features/EntraGuide/EntraGuideModel.swift:119: the control does nothing: the body of this function becomes `return false` (line: func skip() -> Bool {)
# par-entraguide-sweep/close-shrinks-the-guide-away | Sources/Features/EntraGuide/EntraGuideModel.swift:89: the control does nothing: the body of this function becomes `return false` (line: func close() -> Bool { leave { } })
# par-entraguide-sweep/copy-1-puts-the-name-on-the-clipboard | Sources/Features/EntraGuide/EntraGuideActions.swift:21: the control does nothing: the body of this function becomes `return false` (line: func copy(_ n: Int) -> Bool {)
# par-entraguide-sweep/copy-2-puts-smtp-send-on-the-clipboard | Sources/Features/EntraGuide/EntraGuideActions.swift:21: the control does nothing: the body of this function becomes `return false` (line: func copy(_ n: Int) -> Bool {)
# par-entraguide-sweep/copy-3-puts-offline-access-on-the-clipboard | Sources/Features/EntraGuide/EntraGuideActions.swift:21: the control does nothing: the body of this function becomes `return false` (line: func copy(_ n: Int) -> Bool {)
# par-entraguide-sweep/save-and-sign-in-hands-the-id-over | Sources/Features/EntraGuide/EntraGuideActions.swift:40: the control does nothing: the body of this function becomes `return false` (line: func save() -> Bool {)
# par-entraguide-sweep/id-field-takes-the-typed-text | Sources/Features/EntraGuide/EntraGuideActions.swift:48: the control does nothing: the body of this function becomes `return false` (line: func type(_ text: String) -> Bool {)
# par-entraguide-sweep/id-field-click-focuses-it | Sources/Features/EntraGuide/EntraGuideActions.swift:55: the control does nothing: the body of this function becomes `return false` (line: func focusPaste() -> Bool {)
# par-entraguide-sweep/id-field-clear-button-empties-it | Sources/Features/EntraGuide/EntraGuideActions.swift:62: the control does nothing: the body of this function becomes `return false` (line: func clearPasted() -> Bool {)
# par-entraguide-sweep/key-return-goes-one-step-on | Sources/Features/EntraGuide/EntraGuideFeature.swift:46: the key's binding does nothing: its closure becomes `{ _ in false }` (the key is then `ok:false`, as a key bound to nothing is) (line: "Enter": { _ in model().leaving ? true : model().next() }, "Escape": { _ in model().leaving ? true : model(...)
# par-entraguide-sweep/key-escape-closes-the-guide | Sources/Features/EntraGuide/EntraGuideFeature.swift:46: the Esc binding (on the next line of the same dictionary) does nothing: its closure becomes `{ _ in false }` (line: "Enter": { _ in model().leaving ? true : model().next() }, "Escape": { _ in model().leaving ? true : model(...)
# par-entraguide-sweep/key-command-bracket-goes-one-step-back | Sources/Features/EntraGuide/EntraGuideFeature.swift:47: the key's binding does nothing: its closure becomes `{ _ in false }` (the key is then `ok:false`, as a key bound to nothing is) (line: "⌘[": { _ in model().back() }, "⌘Down": { _ in model().skip() },)
# par-entraguide-sweep/key-command-down-skips-to-paste | Sources/Features/EntraGuide/EntraGuideFeature.swift:47: the key's binding does nothing: its closure becomes `{ _ in false }` (the key is then `ok:false`, as a key bound to nothing is) (line: "⌘[": { _ in model().back() }, "⌘Down": { _ in model().skip() },)
# par-entraguide-sweep/key-command-1-copies-the-name | Sources/Features/EntraGuide/EntraGuideFeature.swift:49: the key's binding does nothing: its closure becomes `{ _ in false }` (the key is then `ok:false`, as a key bound to nothing is) (line: for n in 1...3 { out["⌘\(n)"] = { _ in model().copy(n) } })
# par-entraguide-sweep/key-command-2-copies-the-second-value | Sources/Features/EntraGuide/EntraGuideFeature.swift:49: the key's binding does nothing: its closure becomes `{ _ in false }` (the key is then `ok:false`, as a key bound to nothing is) (line: for n in 1...3 { out["⌘\(n)"] = { _ in model().copy(n) } })
# par-entraguide-sweep/key-command-3-copies-the-third-value | Sources/Features/EntraGuide/EntraGuideFeature.swift:49: the key's binding does nothing: its closure becomes `{ _ in false }` (the key is then `ok:false`, as a key bound to nothing is) (line: for n in 1...3 { out["⌘\(n)"] = { _ in model().copy(n) } })
# par-entraguide-sweep/every-listed-control-has-a-case | Sources/Features/EntraGuide/EntraGuideRows.swift:35: one more control is listed that no case drives: `out.append(control("help", "Help", enabled: true))` is added after this line (line: var out: [ControlRow] = [control("close", W.close, enabled: true)])
# MUTANTS-END


# ---------------------------------------------------------------- the self-test: the model is par-entraguide.py's Model; a broken copy makes one control dead
class Model(pe.Model):
    def handle(self, verb, args):
        m = self.m
        self.expire()
        if verb == "uistate":
            return {"ok": True, "state": {"surface": self.surface, "core": "fixture"}}
        if verb == "click" and args[0].startswith(A + ".") and "dead:" + args[0][len(A) + 1:] in m:
            return {"ok": True}
        if verb == "key" and "deadkey:" + args[0] in m:
            return {"ok": False, "error": "key does nothing"}      # a key bound to nothing answers ok:false (docs/CONTROL-SOCKET.md delta 4)
        if verb == "type" and "dead:type" in m:
            return {"ok": True}
        r = super().handle(verb, args)
        if verb == "rows" and args[0] == A + ".paste":
            for row in r["rows"]:
                row["clearable"] = bool(row["text"])       # the real row serves it (EntraGuideRows.paste); the older model did not
        if verb == "rows" and args[0] == A + ".controls" and "extra-control" in m and self.open:
            r["rows"].append({"id": "help", "title": "Help", "key": "", "enabled": True})
        return r


def serve(mutants):
    d = tempfile.mkdtemp(prefix="sw-eg-")
    model = Model(set(mutants))

    class H(socketserver.StreamRequestHandler):
        def handle(self):
            for line in self.rfile:
                q = json.loads(line.decode())
                self.wfile.write((json.dumps(model.handle(q["verb"], q["args"])) + "\n").encode())
    srv = socketserver.ThreadingUnixStreamServer(os.path.join(d, "whoosh.sock"), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return d, srv


def model_run(mutants, only=None):
    d, srv = serve(mutants)
    c = Ctl(d).connect()
    try:
        j = Journey(ID, c)
        pe.CLICKED.clear()
        pe.SEEN.clear()
        drive(j, only)
        return {x.label.split("/", 1)[1]: (x.state, x.detail) for x in j.checks()}
    finally:
        c.close()
        srv.shutdown()
        srv.server_close()
        shutil.rmtree(d, ignore_errors=True)


def _selftest():
    bad = 0
    clean = model_run(())
    for name, (st, why) in clean.items():
        print("model %-48s %-8s %s" % (name, st, " ".join(why.split())[:150]))
        bad += st != "pass"
    declared = {l.split(" | ")[0][2:].strip().split("/", 1)[1] for l in open(os.path.abspath(__file__), encoding="utf-8") if l.startswith("# " + ID + "/")}
    for name, _cv, noop, _fn in CASES:
        if name not in declared:
            print("NO MUTANT declared for %s" % name)
            bad += 1
        st = model_run((noop,), only=name)[name][0]
        print("mutant %-48s %-22s -> %s" % (name, noop, "RED" if st == "fail" else "NOT RED (%s)" % st))
        bad += st != "fail"
    for l in sorted(declared - {c[0] for c in CASES}):
        print("STRAY MUTANT %s" % l)
        bad += 1
    dry = Journey(ID, None)
    drive(dry)
    ok = len(dry.checks()) == len(CASES) and all(c.state == "unjudged" for c in dry.checks())
    print("whole check, undriven: %d cases (all unjudged: %s)" % (len(dry.checks()), ok))
    bad += not ok
    print("selftest: %d cases, %d on the clean model green, %d mutants, %d not red or missing" % (len(CASES), sum(1 for v in clean.values() if v[0] == "pass"), len(declared), bad))
    if not bad:
        print("selftest: every check was seen red on its broken model")
    return 1 if bad else 0


def selftest():
    """On a virtual wall clock like par-entraguide's: a sleep of s seconds moves the clock the model reads by s and costs 2 ms of real time."""
    real_sleep, real_time, skew = time.sleep, time.time, [0.0]

    def virtual_sleep(secs):
        skew[0] += secs
        real_sleep(min(secs, 0.002))
    time.sleep, time.time = virtual_sleep, lambda: real_time() + skew[0]
    try:
        return _selftest()
    finally:
        time.sleep, time.time = real_sleep, real_time


def main():
    path = os.path.abspath(__file__)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    print_cases(r)
    print("MUTANTS: %d declared in %s (a check is green only after it was seen red on its broken copy)" % (len(declared_mutants(path)), os.path.basename(path)))
    for line in r["fail_list"]:
        sys.stderr.write("FAIL %s\n" % line)
    for line in r["no_mutant"]:
        sys.stderr.write("NO MUTANT %s\n" % line)
    for line in r["stray_mutant"]:
        sys.stderr.write("STRAY MUTANT %s\n" % line)
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    return 0 if r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"] else 1


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else main())
