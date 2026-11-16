#!/usr/bin/env python3
"""Shortcuts check: fn-shortcuts-engine (J1.2-J1.5, J1.8-J1.10).

Drives the RUNNING rig copy over the control socket (docs/CONTROL-SOCKET.md) through journeys/lib; expected
values come from docs/design/fixtures/mac-main-sample.json and docs/CONSTANTS.md, never from a built screen.
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/shortcuts-check.py'
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0 (a check that could not run is not green).
"""
import importlib.util
import os
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, print_cases                # noqa: E402
from lib.ctl import Unreadable                                       # noqa: E402
from lib.fixture import FX, ROWS, ACCOUNT, MSG, const               # noqa: E402

# The expected key map is read from the v8 JSON by the v8 check's own reader: no hand-typed key list lives here.
_spec = importlib.util.spec_from_file_location("superhuman_v8_check", os.path.join(ROOT, "checks", "superhuman-v8-check.py"))
V8 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(V8)

ID = "shortcuts"
TITLE = "Shortcut engine"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "every key of Superhuman's v8 list is bound, a sequence waits for its second key, Enter reads the state, an unbound chord is refused"
LIST = "mac-main.list"
MAP = "mac-shortcuts.map"
DISPATCH = "mac-shortcuts.dispatch"
EXTRA_SECOND_KEYS = {"/", "⇧?"}        # "/" is a v8 entry; "?" opens the search (owner's card answer)


def ids(s):
    return [r["id"] for r in s.ctl.rows(LIST)]


def accepted(s, *args):
    """True when the app answers ok; ok:false is False so the check after it turns red (dry run: not judged)."""
    ctl = s.ctl
    try:
        ctl.send(*args)
        return True
    except Unreadable:
        return False


def press(s, chord):
    """Press a chord and return the dispatch rows it added (the log holds the last 64)."""
    seen = s.ctl.rows(DISPATCH)
    before = seen[-1]["n"] if seen else 0
    accepted(s, "key", chord)
    return [r for r in s.ctl.rows(DISPATCH) if r["n"] > before]


def overlay(s):
    return s.ctl.uistate().get("overlay")


def search_focused(s):
    """The search is the main window's field (docs/surfaces/mac-search.md): read the field's focus, not an overlay.
    Live (sc4): uistate.surface reads mac-search while the field is open, overlay stays null; the check does not depend on either."""
    rows = s.ctl.rows("mac-search.bar")
    return bool(rows and rows[0].get("focused") and rows[0].get("owner") == "search")


def label(seq):
    first, second = seq
    return "%s then %s" % (first, second[1:] if second.startswith("⇧") and len(second) == 2 else second)


def settle(s):
    """Back to the plain list: Escape until no overlay and the main surface."""
    for _ in range(4):
        st = s.ctl.uistate()
        if st.get("overlay") is None and st.get("surface") == "mac-main":
            return
        accepted(s, "key", "Escape")


LAST_REAL = {}          # the key verb's last reply (what a red case prints beside its values)
REPORTS = []            # every real key of the run so far: (text, arm, the verb's reply, or the refusal's words)
SAID_FIELDS = ("window_id", "focus", "key_after", "handled", "route", "undo", "can_undo", "text_before", "text_after", "window", "key_before", "windows")


def real(s, text, arm="a"):
    """A REAL key press (`keyevent`): the shell's monitor, then the window. True when the app took the verb. The reply (or the words of a refusal)
    is kept in REPORTS, so a red case prints where each of its keys went: window, focus, key window before and after, route."""
    ctl = s.ctl
    try:
        LAST_REAL.clear()
        LAST_REAL.update(ctl.send("keyevent", "arm:" + arm, text))
        REPORTS.append((text, arm, dict(LAST_REAL)))
        return True
    except Unreadable as e:
        REPORTS.append((text, arm, {"refused": str(e)}))
        return False


def one_report(r):
    if "refused" in r:
        return "refused (%s)" % r["refused"]
    return ", ".join("%s=%r" % (f, r.get(f, "absent")) for f in SAID_FIELDS)


def said(last=2):
    """The last real keys' replies, in the fields that decide where a key went (window, focus, key window before and after, route, whether it was
    handled, the undo word, the text length before and after). A refused key shows its refusal; no key sent says so."""
    if not REPORTS:
        return "no real key was sent"
    # newest key first: the red line is cut at a fixed width, and the LAST key (the one that failed to undo or to stay in the field) decides the verdict
    return "keyevent " + " | ".join("key %r arm %s: %s" % (t, a, one_report(r)) for t, a, r in reversed(REPORTS[-last:]))


def wait_until(fn, tries=20, gap=0.1):
    """Poll a read until it is true (tries x gap seconds, bounded); the last value. A read that fails counts as not yet."""
    v = None
    for i in range(tries):
        try:
            v = fn()
        except Unreadable:
            v = None
        if v:
            return v
        if i < tries - 1:
            time.sleep(gap)
    return v


def settings_up(s):
    """Wait for the Settings WINDOW after the open key: `winframes` lists a window whose id starts with `mac-settings` (the id the app gives it,
    WindowBridge.settingsWindowId; never its title, which is in the app's language). The `mac-settings-window.place` row is served by the model
    whether or not the window exists yet, so it is no proof (main81: it answered while the window list held only the main window and the status item,
    and the letter went to the main window). None when the window is listed; else the sentence with the window list."""
    up = wait_until(lambda: any(str(f.get("id", "")).startswith("mac-settings") for f in s.ctl.winframes()), tries=40)
    if up:
        return None
    def read(fn):
        try:
            return fn()
        except Exception as e:                                       # noqa: BLE001  evidence never hides the verdict it explains
            return "unread (%s)" % e
    return "Settings did not open: place row %s; windows %s" % (read(lambda: s.ctl.rows("mac-settings-window.place")),
                                                                read(lambda: [(f.get("id"), f.get("title"), f.get("kind")) for f in s.ctl.winframes()]))


def composer_undo(s):
    """lead F18: ⌘Z reaches the composer body's own undo, not the engine's."""
    settle(s)
    accepted(s, "key", "C")
    time.sleep(0.3)
    opened = wait_until(lambda: s.ctl.rows("mac-composer.body"))        # the body row exists once the composer is open: wait for it, bounded
    accepted(s, "key", "⇧⌘M")
    time.sleep(0.25)
    real(s, "q")
    real(s, "cmd+z")
    # The verb's own reply (text_after) can say the text view is empty while the body row still serves the old text: the row is the session's copy,
    # refreshed after the view tells it (main81: the row read 'q' with text_after=0). Read until it says empty, bounded (20 x 0.1 s), then judge.
    wait_until(lambda: (lambda r: bool(r) and r[0]["text"] == "")(s.ctl.rows("mac-composer.body")))
    rows = s.ctl.rows("mac-composer.body")
    out = rows[0]["text"] if rows else "no composer body row (opened=%r; surface %r)" % (bool(opened), s.ctl.uistate().get("surface"))
    if out != "":
        out = "%r; %s" % (out, said())                                 # the red line says where the key went and what the verb did with it
    accepted(s, "key", "⇧⌘,")
    accepted(s, "key", "Enter")
    settle(s)
    return out


def settings_field_return(s):
    """lead F18: Return in a Settings field (window not key, as the hidden rig) stays in the field: no dispatch row, no reply."""
    settle(s)
    accepted(s, "key", "⌘,")
    time.sleep(0.4)
    why = settings_up(s)
    if why:
        settle(s)
        return why
    accepted(s, "key", "⌘F")
    time.sleep(0.2)
    real(s, "q", "c")
    seen = s.ctl.rows(DISPATCH)
    n0 = seen[-1]["n"] if seen else 0
    real(s, "return", "c")
    after = s.ctl.rows(DISPATCH)
    out = (s.ctl.rows("mac-settings-window.place")[0]["query"], (after[-1]["n"] if after else 0) - n0, overlay(s))
    if out != SETTINGS_RETURN_WANT:
        out = out + (said(),)                                        # the red line says where the Return went: window, focus, route, whether it was handled
    accepted(s, "key", "Escape")
    settle(s)
    return out


COMPOSER_UNDO_WANT = ""                       # the body text after ⌘Z: the typed letter is gone
SETTINGS_RETURN_WANT = ("q", 0, None)         # (the Settings field's text, dispatch rows the Return added, overlay): it stays in the field


def drive(j):
    s = j.step("J1.2", "Bound keys")
    s.expect("J-accepted", lambda: accepted(s, "key", "J"), True)
    s.expect("K-accepted", lambda: accepted(s, "key", "K"), True)
    s.expect("lowercase-j-is-the-same-key", lambda: (s.key("K") or True) and (s.key("j") or True) and s.ctl.uistate()["focus_index"], 1)
    s.expect("tab-accepted", lambda: accepted(s, "key", "Tab") and accepted(s, "key", "Shift+Tab"), True)
    s.expect("palette-key-accepted", lambda: accepted(s, "key", "⌘P") and accepted(s, "key", "Escape"), True)
    s.expect("search-key-accepted", lambda: accepted(s, "key", "⌘K") and accepted(s, "key", "Escape"), True)
    s.expect("slash-and-question-open-the-search", lambda: [(accepted(s, "key", k), search_focused(s), accepted(s, "key", "Escape")) for k in ("/", "⇧?")],
             [(True, True, True)] * 2)
    s.expect("snooze-key-accepted", lambda: accepted(s, "key", "H") and accepted(s, "key", "Escape"), True)
    s.expect("label-key-accepted", lambda: accepted(s, "key", "L") and accepted(s, "key", "Escape"), True)
    s.expect("pin-key-accepted", lambda: accepted(s, "key", "⇧P"), True)
    s.expect("unbound-chord-refused", lambda: accepted(s, "key", "Q"), False)
    # A and M stay gone. ⌘J is NOT gone any more: the owner approved AI replies with ⌘J in a thread (docs/JOURNEYS.md J10; Sources/Features/Shortcuts/Commands.swift:59 binds "ai-reply"; docs/lanes/shortcuts.md, dated line under "Gone"), so it left this list.
    s.expect("keys-v8-removed-are-refused", lambda: [accepted(s, "key", k) for k in ("A", "M")], [False, False])

    s = j.step("J1.3", "The v8 map")
    s.expect("preset-map-matches-fixture", lambda: {r["key"]: r["action"] for r in s.ctl.rows("mac-shortcuts.preset")}, FX["shortcuts"])
    s.expect("map-equals-v8", lambda: map_diff(s), {"missing": [], "extra": []})
    s.expect("status-matches-commands", lambda: status_diff(s), [])
    s.expect("audit-clean", lambda: [r["preset"] for r in s.ctl.rows("mac-shortcuts.audit")
                                     if r["duplicates"] or r["unregistered"] or r["no_handler"]], [])
    s.expect("gotemail-palette-key", lambda: s.ctl.rows("mac-shortcuts.preset.gotemail")[0]["palette"], "⇧⌘P")
    s.expect("stub-key-reaches-its-command", lambda: stub_diff(s), [])

    s = j.step("J1.4", "Two-key sequences")
    s.expect("first-key-waits", lambda: first_key(s), ("armed", False))
    s.expect("second-key-resolves", lambda: second_key(s), ("go-inbox", "G then I"))
    s.expect("every-sequence-resolves", lambda: sequence_diff(s), [])
    s.expect("late-second-key-is-dropped", lambda: late_second_key(s), (True, False))
    s.expect("wrong-second-key-cancels", lambda: wrong_second_key(s), (True, False))
    s.expect("second-key-never-leaks", lambda: leak(s), (True, True))

    s = j.step("J1.5", "Enter by state, digits, coming soon")
    s.expect("enter-opens-then-replies-all", lambda: enter_by_state(s), ("reply", "mac-reader", "reply-all", True))
    s.expect("control-digits-switch-accounts", lambda: digits(s), [FX["accounts"][1]["id"], FX["accounts"][0]["id"]])
    s.expect("coming-soon-notice", lambda: notices(s), ["Tabs is coming soon.", "Calendar is coming soon.", []])
    s.expect("palette-show-all-keys-opens-the-key-list", lambda: show_all_keys(s), ("Show all keys", "", "mac-trainer.keys"))
    s.expect("gotemail-keys-work", lambda: gotemail(s), (True, True, False))

    s = j.step("J1.6", "A focused text view keeps its editing keys")
    s.expect("cmd-z-undoes-typed-text-in-the-composer", lambda: composer_undo(s), COMPOSER_UNDO_WANT)
    s.expect("return-in-a-settings-field-stays-in-the-field", lambda: settings_field_return(s), SETTINGS_RETURN_WANT)

    s = j.step("J1.8", "First paint")
    s.expect("archive-key-accepted", lambda: accepted(s, "key", "E"), True)
    s.expect("undo-key-accepted", lambda: accepted(s, "key", "Z"), True)


def map_diff(s):
    rows = s.ctl.rows(MAP)
    have = {r["key"] for r in rows}
    want = set()
    for e in V8.expected():
        want.update(e["chords"])
        if e["sequence"]:
            want.add(label(e["sequence"]))
    want |= set(V8.EXTRAS) | EXTRA_SECOND_KEYS
    return {"missing": sorted(want - have), "extra": sorted(have - want)}


def status_diff(s):
    cmds = V8.commands()
    names = {"live": "live", "stub": "stub", "soon": "coming-soon"}
    return [r["key"] for r in s.ctl.rows(MAP) if names[cmds[r["command"]][1]] != r["status"]]


def stub_diff(s):
    """Press one chord of each owner lane that has no handler yet: the engine must resolve it to its command and pass."""
    picks = {"⌘B": "bold", "⇧⌘O": "compose-to", "⌘D": "switch-draft", "⇧Y": "remove-all-labels", "O": "expand-message"}
    bad = []
    for chord, command in picks.items():
        got = press(s, chord)
        if not got or got[-1]["command"] != command or got[-1]["outcome"] != "passed":
            bad.append((chord, got[-1:] if got else None))
    settle(s)
    return bad


def first_key(s):
    rows = press(s, "G")
    return (rows[-1]["outcome"], any(r["command"] == "go-inbox" for r in rows))


def second_key(s):
    rows = press(s, "I")
    return (rows[-1]["command"], rows[-1]["key"])


def sequence_diff(s):
    bad = []
    for r in s.ctl.rows(MAP):
        if r["kind"] != "sequence":
            continue
        first, second = r["key"].split(" then ")
        second = "⇧" + second if second in ("!", "#") else second
        accepted(s, "key", first)
        rows = press(s, second)
        if not rows or rows[-1]["key"] != r["key"] or rows[-1]["command"] != r["command"]:
            bad.append((r["key"], rows[-1:] if rows else None))
        settle(s)
    home(s)
    return bad


def late_second_key(s):
    accepted(s, "key", "G")
    time.sleep(const("SEQUENCE_WAIT_MS") / 1000.0 + 0.3)
    rows = press(s, "I")
    return (any(r["outcome"] == "lapsed" for r in rows), any(r["command"] == "go-inbox" for r in rows))


def wrong_second_key(s):
    accepted(s, "key", "G")
    rows = press(s, "Q")
    cancelled = any(r["outcome"] == "cancelled" for r in rows)
    again = press(s, "I")
    return (cancelled, any(r["command"] == "go-inbox" for r in again))


def home(s):
    """Back to the inbox: G then I. Since sidebar r8 every G sequence changes the mailbox the list shows, and Escape does not leave it."""
    settle(s)
    accepted(s, "key", "G")
    accepted(s, "key", "I")


def leak(s):
    """G then E is 'go to done'. The E must not also archive the focused conversation: no archive is dispatched, and the inbox is unchanged once we are back."""
    home(s)
    before = ids(s)
    accepted(s, "key", "G")
    rows = press(s, "E")
    archived = any(r["command"] == "archive" for r in rows)
    home(s)
    return (any(r["command"] == "go-done" for r in rows), not archived and ids(s) == before)


def enter_by_state(s):
    settle(s)
    first = press(s, "Enter")
    surface = s.ctl.uistate().get("surface")
    # Enter from the list opens the mail with the reply under it and the cursor in its text (owner 13 Nov 07:29): there a second Enter is a new line of the
    # reply, by the owner's rule (07:27), and the engine never sees it. Enter is "reply all" in an open mail whose reply does not hold the keyboard: Escape
    # closes the empty reply and the mail stays open (par-escape).
    accepted(s, "key", "Escape")
    second = press(s, "Enter")
    # the reply box is the quick reply's own row (mac-main.quickreply.field), not an overlay name: reply all opens it with the header filled
    out = (first[-1]["resolved"], surface, second[-1]["resolved"], bool(s.ctl.rows("mac-main.quickreply.field")))
    settle(s)
    return out


def digits(s):
    got = []
    for key in ("⌃2", "⌃1"):
        accepted(s, "key", key)
        got.append(s.ctl.uistate().get("account_filter"))
    accepted(s, "click", "mac-main.accounts.all")
    return got


def notices(s):
    got = []
    for key in ("⌘T", "T"):
        accepted(s, "key", key)
        rows = s.ctl.rows("mac-shortcuts.notice")
        got.append(rows[0]["text"] if rows else None)
    accepted(s, "key", "J")
    got.append(s.ctl.rows("mac-shortcuts.notice"))
    return got


def show_all_keys(s):
    """The palette line 'Show all keys' shows the key that opens the list (none in this preset) and runs the list, not the search."""
    settle(s)
    accepted(s, "key", "⌘P")
    accepted(s, "type", "show all keys")
    line = s.ctl.rows("mac-palette.results")[0]
    accepted(s, "key", "Enter")
    out = (line["title"], line["key"], overlay(s))
    settle(s)
    return out


def gotemail(s):
    accepted(s, "click", "mac-shortcuts.preset.gotemail")
    palette = accepted(s, "key", "⇧⌘P") and overlay(s) == "mac-palette"
    settle(s)
    search = accepted(s, "key", "⌘K") and search_focused(s)
    settle(s)
    slash = accepted(s, "key", "/")
    accepted(s, "click", "mac-shortcuts.preset.superhuman")
    return (palette, search, slash)


# MUTANTS: per check, the mutation of the app that must turn it red.
# J1.2/J-accepted | J is unbound
# J1.2/K-accepted | K is unbound
# J1.2/lowercase-j-is-the-same-key | lowercase and uppercase differ
# J1.2/tab-accepted | Tab or Shift+Tab is unbound
# J1.2/palette-key-accepted | the palette key is unbound
# J1.2/search-key-accepted | the search key is unbound
# J1.2/slash-and-question-open-the-search | / or ? is unbound, or opens something else
# J1.2/snooze-key-accepted | H is unbound
# J1.2/label-key-accepted | L is unbound
# J1.2/pin-key-accepted | the pin key is unbound
# J1.2/unbound-chord-refused | every chord is accepted
# J1.2/keys-v8-removed-are-refused | A, M or the accounts-menu key is bound again
# J1.3/preset-map-matches-fixture | the preset maps a key to another action
# J1.3/map-equals-v8 | a v8 chord is left out of the preset, or an extra chord is bound
# J1.3/status-matches-commands | a command marked live while its handler is a stub, or a stub marked live
# J1.3/audit-clean | a chord or a command is bound twice, or a command has no title or handler
# J1.3/gotemail-palette-key | the GotEmail preset keeps the Superhuman palette key
# J1.3/stub-key-reaches-its-command | a stub chord resolves to another command or is not recorded
# J1.4/first-key-waits | the first key of a sequence fires the sequence
# J1.4/second-key-resolves | the second key is ignored
# J1.4/every-sequence-resolves | one sequence is dropped from the preset
# J1.4/late-second-key-is-dropped | the sequence has no wait limit
# J1.4/wrong-second-key-cancels | a wrong second key leaves the sequence armed
# J1.4/second-key-never-leaks | a matched sequence lets its second key run the single-key command
# J1.5/enter-opens-then-replies-all | Enter from the list always opens without the reply (A102: it replies, as R does), or in a thread always replies
# J1.5/control-digits-switch-accounts | a control digit switches to the first account
# J1.5/coming-soon-notice | a coming-soon key shows no notice, or the notice stays
# J1.5/palette-show-all-keys-opens-the-key-list | the palette line runs its hard-coded ⇧? (the search) or shows ⇧? as its key
# J1.5/gotemail-keys-work | the preset switch does not store the preset
# J1.6/cmd-z-undoes-typed-text-in-the-composer | the shell's monitor gives ⌘Z to the engine before the composer body, so the typed text stays
# J1.6/return-in-a-settings-field-stays-in-the-field | the monitor reads the KEY window's focus, so Return in a Settings field of a window that is not key replies to the selected mail
# J1.8/archive-key-accepted | E is unbound
# J1.8/undo-key-accepted | Z is unbound


class SelfCtl:
    """A stand-in for the socket, only as far as the two J1.6 cases read it (the composer's ⌘Z and Return in a Settings field), clean and with the
    app's own break each case names in its MUTANTS row. `mutant`: engine-takes-cmd-z | monitor-reads-key-window."""

    def __init__(self, mutant=None):
        self.mutant, self.surface, self.body, self.settings, self.query, self.n, self.field = mutant, "mac-main", None, False, "", 0, False

    def uistate(self):
        return {"overlay": None, "surface": self.surface}

    def winframes(self):
        return [{"id": "mac-main"}] + ([{"id": "mac-settings-window", "title": "Settings", "kind": "window"}] if self.settings else [])

    def rows(self, ax):
        if ax == "mac-composer.body":
            return [] if self.body is None else [{"text": self.body}]
        if ax == DISPATCH:
            return [{"n": self.n}]
        if ax == "mac-settings-window.place":
            return [{"query": self.query}]
        return []

    def send(self, verb, *args):
        if verb == "key":
            k = args[0]
            if k == "C":
                self.surface, self.body = "mac-composer", ""
            elif k == "⇧⌘,":
                self.surface, self.body = "mac-main", None
            elif k == "⌘,":
                self.settings = True
            elif k == "⌘F":
                self.field = self.settings
            elif k == "Escape":
                self.settings, self.field = False, False
            return {}
        if verb == "keyevent":
            text = args[-1]
            if text == "q":
                if self.field:
                    self.query += "q"
                elif self.body is not None:
                    self.body += "q"
            elif text == "cmd+z" and self.body is not None and self.mutant != "engine-takes-cmd-z":
                self.body = self.body[:-1]
            elif text == "return" and self.field and self.mutant == "monitor-reads-key-window":
                self.n += 1                           # the engine took the Return: a dispatch row (a reply to the selected mail)
            if self.mutant == "refuses-the-key":
                raise Unreadable("keyevent: no window shows mac-main")
            return {"handled": True, "route": "sendEvent", "window_id": "stand-in", "window": "Stand-in", "focus": "stand-in", "key_before": False, "key_after": False}
        return {}


class SelfStep:
    def __init__(self, ctl):
        self.ctl = ctl


def selftest():
    """The two J1.6 cases that decide on the key verb's reply: each is green on the stand-in and red on its own mutant, and a red value
    carries the verb's reply fields (`said()`), never a bare mismatch."""
    cases = (("cmd-z-undoes-typed-text-in-the-composer", composer_undo, COMPOSER_UNDO_WANT, "engine-takes-cmd-z"),
             ("return-in-a-settings-field-stays-in-the-field", settings_field_return, SETTINGS_RETURN_WANT, "monitor-reads-key-window"))
    declared = {l[1:].strip().split(" | ", 1)[0] for l in open(os.path.abspath(__file__), encoding="utf-8") if l.startswith("# J") and " | " in l}
    bad, ev_missing = 0, 0
    for name, fn, want, mutant in cases:
        for mut in (None, mutant):
            LAST_REAL.clear()
            REPORTS.clear()
            got = fn(SelfStep(SelfCtl(mut)))
            green = got == want
            ok = green if mut is None else not green
            line = got if isinstance(got, str) else got[3] if isinstance(got, tuple) and len(got) == 4 else ""
            printed = mut is None or all(w in line for w in ("keyevent", "key_before=False", "key_after=False", "route='sendEvent'", "focus='stand-in'"))
            print("%-5s J1.6/%s | %s | got %r" % ("PASS" if ok else "FAIL", name, "clean" if mut is None else "mutant " + mut + " is RED" if not green else "mutant " + mut + " NOT RED", got))
            bad += (not ok) + (not printed)
            ev_missing += not printed
        bad += "J1.6/" + name not in declared
    print("evidence on red (shortcuts-check): %d of %d J1.6 cases print the key verb's reply (window, focus, key window before and after, route) on their mutant" % (len(cases) - ev_missing, len(cases)))
    # a key the verb refuses: the red line carries the refusal's words, not "absent"
    REPORTS.clear()
    real(SelfStep(SelfCtl("refuses-the-key")), "q", "c")
    refusal = said()
    print("%-5s J1.6/refused-key-is-quoted | %s" % ("PASS" if "refused (keyevent: no window shows mac-main)" in refusal else "FAIL", refusal))
    bad += "refused (keyevent: no window shows mac-main)" not in refusal
    print("shortcuts-check selftest: %s (%d problems)" % ("green" if not bad else "RED", bad))
    return 1 if bad else 0


def main():
    if "--selftest" in sys.argv:
        return selftest()
    path = os.path.abspath(__file__)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    rows = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# J") and " | " in l]
    print("MUTANTS (a check is green only after it has been seen red on its mutant)")
    print("| check | mutant |")
    print("|---|---|")
    for k, v in rows:
        print("| %s | %s |" % (k, v))
    print_cases(r)       # one line per case, the passing ones too
    for line in r["fail_list"]:
        sys.stderr.write("FAIL %s\n" % line)
    for line in r["no_mutant"]:
        sys.stderr.write("NO MUTANT %s\n" % line)
    for line in r["stray_mutant"]:
        sys.stderr.write("STRAY MUTANT %s\n" % line)
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    ok = r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"]
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
