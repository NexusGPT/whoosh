#!/usr/bin/env python3
"""Parity check: par-entraguide (the in-app guide "Get your organisation's app ID", mac-entra-guide). Every control of the guide, driven
through the control socket, each with one check of its visible result as docs/surfaces/mac-entra-guide.md and the picked board
(docs/design/boards/entra-guide-r1/eg-screenshot-stage) state them: the guide opening as the grown add-account sheet, 27 steps (one per
click, the owner's amendment of 2026-11-09 16:14), "Step N of 27", the ring and its caption per step, Copy with its key and the Copied
state, Back, Next, Skip to paste, Close, the ID field with its check, "Save and sign in", the compact size, and the words from the
catalogue in 14 languages.

    python3 checks/parity/par-entraguide.py             drive the running rig (sh scripts/launch-one.sh <bundle> -- --fixture-core ...)
    python3 checks/parity/par-entraguide.py --selftest  every check against a model, then against each mutant: each must go red
Prints 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0. Expected values are written here from the surface file, the board
and docs/lanes/entra-guide-build.md, or read from the repo's own data files (steps.json, the catalogue), never from a built screen.
The rig runs in English. This check never opens a window of its own: it only talks to the app the lead started."""
import json
import os
import re
import shutil
import socketserver
import sys
import tempfile
import threading
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable  # noqa: E402

A = "mac-entra-guide"
GOOD_ID = "1c6f2a90-7d3e-4b8a-9e51-0f4c2d7b8a63"
WRONG_ID = "1c6f2a90-7d3e-4b8a-9e51"
LANGS = ["da", "de", "en", "es", "fi", "fr", "it", "ja", "nb", "nl", "pl", "pt", "sv", "zh-Hans"]
URL = "https://entra.microsoft.com"
CHECKS = []
CLICKED = set()   # every control id a check clicked
SEEN = set()      # every control id the controls rows ever listed


def data_path(*p): return os.path.join(ROOT, *p)


def load_steps():
    return json.load(open(data_path("Sources", "Resources", "EntraGuide", "entraguide-steps.json"), encoding="utf-8"))


def load_en():
    cat = json.load(open(data_path("Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8"))["strings"]
    return {k: v["localizations"]["en"]["stringUnit"]["value"] for k, v in cat.items() if "en" in v.get("localizations", {})}


DATA = load_steps()
EN = load_en()
STEPS = DATA["steps"]
M = len(STEPS)
PLATES = {p["id"]: p for p in DATA["plates"]}


def say(n, domain=""):
    """The step's sentence; the consent step's one %@ is the mailbox's domain, or the plain words when the sheet had no address (A98)."""
    return EN["entra-guide.s%02d.say" % n].replace("%@", domain or EN["entra-guide.consent.nodomain"])


def shipped_id():
    """The client ID Whoosh ships, read from its one home (never typed here)."""
    t = open(data_path("Sources", "Features", "AddAccount", "DefaultSignInClients.swift"), encoding="utf-8").read()
    return re.search(r'static let microsoft: String\? = "([0-9a-fA-F-]{36})"', t).group(1)


def consent_link(domain): return "https://login.microsoftonline.com/%s/adminconsent?client_id=%s" % (domain or "organizations", shipped_id())
def cap(n): return EN["entra-guide.s%02d.cap" % n]
def count(n): return EN["Step %@ of %@"].replace("%@", str(n), 1).replace("%@", str(M), 1)


def check(name, mutant, words):
    def deco(fn):
        CHECKS.append((name, mutant, words, fn))
        return fn
    return deco


class Failed(Exception):
    """The app answered that a control or a key did nothing: the check is red, not unjudged."""


# ---- the driver's helpers ----
def click(c, ax):
    CLICKED.add(ax)
    try:
        return c.click(A + "." + ax)
    except Unreadable as e:
        if "did nothing" in str(e) or "nothing to click" in str(e):
            raise Failed("the click on %s did nothing" % ax)
        raise


def key(c, chord):
    try:
        return c.key(chord)
    except Unreadable as e:
        if "does nothing" in str(e):
            raise Failed("the key %s did nothing" % chord)
        raise


def refused(c, ax):
    """True when the click did nothing (a control that is off or not on screen)."""
    try:
        c.click(A + "." + ax)
        return False
    except Unreadable:
        return True


def rows(c, region):
    r = c.rows("%s.%s" % (A, region))
    if region == "controls":
        SEEN.update(x["id"] for x in r)
    return r


def state(c): return rows(c, "state")[0]
def step_row(c): return rows(c, "step")[0]
def ctl(c, cid): return next((x for x in rows(c, "controls") if x["id"] == cid), None)


def wait_for(c, fn, tries=40, pause=0.1):
    for _ in range(tries):
        if fn():
            return True
        time.sleep(pause)
    return fn()


def fresh(c):
    """The guide open at step 1 with an empty ID field."""
    wait_for(c, lambda: not state(c)["leaving"])
    if rows(c, "state") and state(c)["open"]:
        if state(c)["step"] != M:
            click(c, "skip")
        if state(c)["pasted"]:
            click(c, "paste.clear")
        click(c, "close")
        wait_for(c, lambda: not state(c)["open"])
    click(c, "open")
    wait_for(c, lambda: state(c)["open"])
    if state(c)["pasted"]:
        goto(c, M)
        click(c, "paste.clear")
        goto(c, 1)


SHEET = "mac-add-account"


def sheet_address(c, address):
    """The add-account sheet holding `address` (empty = a fresh sheet); the guide reads the domain from it when it opens."""
    if rows(c, "state") and state(c)["open"]:   # the guide is shut first: the typing must land in the sheet's field
        click(c, "close")
        wait_for(c, lambda: not state(c)["open"])
    c.click(SHEET + ".start-over"); c.click(SHEET + ".open")
    if address:
        c.click(SHEET + ".address"); c.type(address)


def goto(c, n):
    click(c, "goto.%d" % n)
    return wait_for(c, lambda: state(c)["step"] == n)


def number_ok(r): return isinstance(r, list) and len(r) == 4 and all(isinstance(x, (int, float)) and 0 <= x <= 1 for x in r) and r[0] < r[2] and r[1] < r[3]


# ================= the checks =================
@check("opens-as-the-grown-sheet", "open-does-nothing", "Open opens the guide at step 1 of 27 over 7 chapters")
def _(c):
    fresh(c)
    s = state(c)
    return [s["open"], s["step"], s["steps"], s["chapters"]], [True, 1, M, 7]


@check("one-step-per-click", "few-steps", "the owner's amendment: far more than seven steps, every one with its own sentence and ring")
def _(c):
    fresh(c)
    r = rows(c, "steps")
    good = [x["n"] for x in r if x["say"] and not x["say"].startswith("entra-guide.") and x["cap"] and not x["cap"].startswith("entra-guide.") and number_ok(x["ring"])]
    return [len(r), len(good), r[0]["of"] if r else 0], [M, M, M]


@check("counter-reads-step-n-of-m", "counter-says-of-7", "the head says 'Step 1 of 27', never 'of 7'")
def _(c):
    fresh(c)
    return step_row(c)["count"], count(1)


@check("step-words-come-from-the-catalogue", "raw-key-shown", "the sentence on screen is the catalogue's English, not a raw key")
def _(c):
    fresh(c)
    out = []
    for n in (1, 11, 20, 27):
        goto(c, n)
        r = step_row(c)
        out.append((r["say"], r["cap"]))
    return out, [(say(n), cap(n)) for n in (1, 11, 20, 27)]


@check("walk-every-step", "step-skipped", "Next walks all 27 steps in order, one at a time, each with its words, picture and ring")
def _(c):
    fresh(c)
    seen = []
    first = ctl(c, "next")
    click(c, "next")                     # step 1: opens the browser, stays on step 1
    for n in range(2, M + 1):
        click(c, "next")
        r = step_row(c)
        seen.append((r["n"], r["count"], r["say"], r["plate"], bool(r["location"]), number_ok(r["ring"])))
    want = [(n, count(n), say(n), STEPS[n - 1]["plate"], True, True) for n in range(2, M + 1)]
    return [first["title"], seen], [EN["entra-guide.open-entra"], want]


@check("step-one-opens-the-browser", "browser-not-opened", "Open entra.microsoft.com opens the page, then the button reads Next and the note says it opened")
def _(c):
    fresh(c)
    before = ctl(c, "next")["title"]
    click(c, "next")
    s = state(c)
    return [before, s["step"], s["opened_links"], s["opened"], ctl(c, "next")["title"]], [EN["entra-guide.open-entra"], 1, [URL], True, EN["Next"]]


@check("close-by-control", "close-does-nothing", "Close (esc) shrinks the guide away")
def _(c):
    fresh(c)
    click(c, "close")
    return wait_for(c, lambda: not state(c)["open"]), True


@check("close-by-escape", "escape-does-nothing", "the Escape key closes the guide")
def _(c):
    fresh(c)
    key(c, "Escape")
    return wait_for(c, lambda: not state(c)["open"]), True


@check("next-by-key", "enter-does-nothing", "Return does what Next does")
def _(c):
    fresh(c)
    goto(c, 3)
    key(c, "Enter")
    return state(c)["step"], 4


@check("back-by-control-and-key", "back-goes-forward", "Back (⌘[) goes one step back, by click and by key")
def _(c):
    fresh(c)
    goto(c, 6)
    click(c, "back")
    a = state(c)["step"]
    key(c, "⌘[")
    return [a, state(c)["step"]], [5, 4]


@check("back-is-off-at-the-start", "back-enabled-first", "Back is off at step 1")
def _(c):
    fresh(c)
    return [ctl(c, "back")["enabled"], refused(c, "back"), state(c)["step"]], [False, True, 1]


@check("skip-by-control-and-key", "skip-lands-short", "Skip to paste (⌘↓) lands on the last step, by click and by key")
def _(c):
    fresh(c)
    goto(c, 3)
    click(c, "skip")
    a = state(c)["step"]
    goto(c, 5)
    key(c, "⌘Down")
    return [a, state(c)["step"]], [M, M]


@check("no-skip-on-the-last-step", "skip-visible-last", "Skip to paste is gone where the paste field already is")
def _(c):
    fresh(c)
    goto(c, M)
    return [ctl(c, "skip"), refused(c, "skip")], [None, True]


@check("controls-show-their-keys", "key-cap-missing", "every control shows its key: Close esc, Skip ⌘↓, Back ⌘[, Next ↩, Copy ⌘1 and ⌘2")
def _(c):
    fresh(c)
    out = {x["id"]: x["key"] for x in rows(c, "controls")}
    goto(c, 5)
    out.update({x["id"]: x["key"] for x in rows(c, "controls")})
    return [out.get(k) for k in ("close", "skip", "back", "next", "copy.1", "copy.2")], ["esc", "⌘↓", "⌘[", "↩", "⌘1", "⌘2"]


@check("copy-shows-copied", "copy-wrong-value", "Copy ⌘1 puts the Name on the clipboard and reads Copied")
def _(c):
    fresh(c)
    goto(c, 5)
    click(c, "copy.1")
    s = state(c)
    return [s["clipboard"], s["copied"], ctl(c, "copy.1")["title"], ctl(c, "copy.2")["title"]], ["Whoosh", 1, EN["Copied"], EN["Copy"]]


@check("copy-by-key-two", "copy-key-two-missing", "⌘2 copies the redirect address")
def _(c):
    fresh(c)
    goto(c, 8)
    key(c, "⌘2")
    s = state(c)
    return [s["clipboard"], s["copied"]], ["http://localhost/", 2]


@check("copied-goes-back-to-copy", "copied-stays", "Copied lasts 1.6 s, then the button reads Copy again")
def _(c):
    fresh(c)
    goto(c, 5)
    click(c, "copy.1")
    now = ctl(c, "copy.1")["title"]
    time.sleep(2.0)
    return [now, ctl(c, "copy.1")["title"], state(c)["copied"]], [EN["Copied"], EN["Copy"], 0]


@check("copy-three-permissions", "permission-copy-wrong", "step 20 to 24 offer IMAP.AccessAsUser.All, SMTP.Send and offline_access to copy")
def _(c):
    fresh(c)
    goto(c, 20)
    click(c, "copy.1")
    a = state(c)["clipboard"]
    click(c, "copy.2")
    b = state(c)["clipboard"]
    click(c, "copy.3")
    return [a, b, state(c)["clipboard"]], ["IMAP.AccessAsUser.All", "SMTP.Send", "offline_access"]


@check("value-glows-on-its-step", "no-current-value", "the value row of the current step glows, the others stay as a reminder")
def _(c):
    fresh(c)
    out = []
    for n in (5, 6, 7, 8):
        goto(c, n)
        out.append([v["label"] for v in rows(c, "values") if v["current"]])
    names = ["Name", "Who can use it", "Platform", "Redirect address"]
    return out, [[x] for x in names]


@check("pick-values-carry-a-tag", "pick-tag-missing", "a value picked in Entra's list is tagged 'Pick it in the list', not given a Copy button")
def _(c):
    fresh(c)
    goto(c, 6)
    v = {x["label"]: x for x in rows(c, "values")}
    return [v["Who can use it"]["copy"], v["Who can use it"]["tag"], v["Platform"]["copy"]], [False, EN["entra-guide.pick-tag"], False]


@check("id-check-wrong-then-right", "id-accepts-anything", "a wrong ID is refused in words, then the right one gets the tick")
def _(c):
    fresh(c)
    goto(c, 11)
    c.type(WRONG_ID)
    a = rows(c, "paste")[0]
    click(c, "paste.clear")
    b = rows(c, "paste")[0]
    c.type(GOOD_ID)
    d = rows(c, "paste")[0]
    return [a["kind"], a["valid"], a["wrong"], b["text"], d["valid"], d["tick"]], ["slot", False, True, "", True, True]


@check("slot-text-reaches-the-last-step", "slot-text-lost", "the ID pasted on step 11 is still in the field on the last step")
def _(c):
    fresh(c)
    goto(c, 11)
    c.type(GOOD_ID)
    goto(c, M)
    r = rows(c, "paste")[0]
    return [r["kind"], r["text"], r["valid"]], ["paste", GOOD_ID, True]


@check("last-step-note-and-field", "keychain-note-missing", "the last step has the field, 'Application ID', and the Keychain note; a wrong ID swaps the note for a refusal")
def _(c):
    fresh(c)
    goto(c, M)
    a = rows(c, "paste")[0]
    c.type(WRONG_ID)
    b = rows(c, "paste")[0]
    return [a["label"], a["note"], b["note"]], [EN["entra-guide.paste-label"], EN["entra-guide.keychain"], EN["entra-guide.id-wrong"]]


@check("paste-field-takes-focus", "paste-focus-missing", "clicking the field focuses it; Skip to paste focuses it too")
def _(c):
    fresh(c)
    goto(c, M)
    click(c, "paste.field")
    a = rows(c, "paste")[0]["focused"]
    fresh(c)
    click(c, "skip")
    return [a, rows(c, "paste")[0]["focused"]], [True, True]


@check("save-needs-a-valid-id", "save-without-valid", "Save and sign in is off until the text is an app ID, and a click does nothing")
def _(c):
    fresh(c)
    goto(c, M)
    c.type(WRONG_ID)
    s = ctl(c, "save")
    return [s["title"], s["key"], s["enabled"], refused(c, "save"), state(c)["open"]], [EN["entra-guide.save"], "↩", False, True, True]


@check("save-hands-the-id-over", "save-loses-id", "Save and sign in closes the guide and hands the pasted ID back")
def _(c):
    fresh(c)
    goto(c, M)
    c.type(GOOD_ID)
    ok = ctl(c, "save")["enabled"]
    click(c, "save")
    closed = wait_for(c, lambda: not state(c)["open"])
    return [ok, closed, state(c)["last_saved"]], [True, True, GOOD_ID]


@check("aurora-on-a-valid-id", "aurora-always", "the hairline has 27 segments, and turns to the aurora colours only for a valid ID on the last step")
def _(c):
    fresh(c)
    a = rows(c, "progress")[0]
    goto(c, M)
    b = rows(c, "progress")[0]
    c.type(GOOD_ID)
    d = rows(c, "progress")[0]
    return [a["segments"], a["filled"], a["aurora"], b["filled"], b["aurora"], d["aurora"]], [M, 1, False, M, False, True]


@check("every-plate-is-a-real-shot", "stand-in-kept", "no plate is a drawn stand-in: the sign-in page and the IMAP and SMTP ticks are the owner's shots (2026-11-09)")
def _(c):
    fresh(c)
    out = []
    for n in (1, 2, 21, 23):
        goto(c, n)
        p = rows(c, "plate")[0]
        out.append((p["drawn"], p["flag"]))
    return out, [(False, ""), (False, ""), (False, ""), (False, "")]


@check("plate-shows-ring-and-caption", "ring-outside-picture", "every step's plate has a ring inside the picture and its caption")
def _(c):
    fresh(c)
    bad = []
    for n in range(1, M + 1):
        goto(c, n)
        p = rows(c, "plate")[0]
        if not (number_ok(p["ring"]) and p["caption"] == cap(n) and p["plate"] == STEPS[n - 1]["plate"]):
            bad.append(n)
    return bad, []


def sheet_frame(c):
    """The sheet's frame. The first `measure` of a cold probe only mounts the readers (FrameProbe.request, PROBE_SETTLE_S 0.15 s); on a hidden rig the
    overlay's reader may not have published when the reply is built ("nothing on screen carries that id", not judged on main50, 56, 57 and 58). One
    retry after the readers had time to publish; a second refusal is still not judged."""
    try:
        return c.measure(A + ".sheet")["frame"]
    except Unreadable:
        time.sleep(0.6)
        return c.measure(A + ".sheet")["frame"]


@check("compact-size", "compact-same-top", "at 960 x 632 the sheet is 880 wide and sits 16 pt from the top (36 pt at 1280 x 820)")
def _(c):
    fresh(c)
    c.send("resize", "1280", "820"); time.sleep(0.4)
    wide = sheet_frame(c)
    c.send("resize", "960", "632"); time.sleep(0.4)
    small = sheet_frame(c)
    return [wide["w"], small["w"], round(wide["h"] - small["h"])], [880, 880, 188 - 2 * (36 - 16)]


@check("every-control-was-driven", "unclicked-control", "every control the rows ever listed was clicked by a check above")
def _(c):
    want = {"close", "skip", "back", "next", "save", "copy.1", "copy.2", "copy.3"}
    return sorted((SEEN | want) - CLICKED), []


@check("consent-step-names-the-mailbox-organisation", "consent-domain-missing", "A98: opened from a sheet whose address is <user>@srv.example.test, the consent step's words carry srv.example.test; with no address they say 'your mailbox's domain'")
def _(c):
    sheet_address(c, "someone@srv.example.test")
    fresh(c); goto(c, 26)
    r = step_row(c)
    with_domain = [r["say"], "srv.example.test" in (r["say"] + " " + r["cap"]), "<domain>" in r["say"] + r["hint"]]
    sheet_address(c, "")
    fresh(c); goto(c, 26)
    without = step_row(c)["say"]
    return [with_domain, without], [[say(26, "srv.example.test"), True, False], say(26)]


@check("consent-step-offers-the-admin-consent-link", "consent-link-wrong", "A98: step 26 offers a copyable row whose text is the exact admin-consent URL for the address's domain and the shipped client id; its copy control copies it")
def _(c):
    sheet_address(c, "someone@srv.example.test")
    fresh(c); goto(c, 26)
    v = rows(c, "values")
    want = consent_link("srv.example.test")
    row = v[0] if v else {}
    click(c, "copy.1")
    copied = state(c)["clipboard"]
    sheet_address(c, "")
    fresh(c); goto(c, 26)
    bare = rows(c, "values")
    return [len(v), row.get("value"), row.get("copy"), row.get("number"), copied, [x["value"] for x in bare]], [1, want, True, 1, want, [consent_link("")]]


# ---- checks that read the repo, not the app ----
def source_files(root):
    d = os.path.join(root, "Sources", "Features", "EntraGuide")
    return [os.path.join(d, f) for f in sorted(os.listdir(d)) if f.endswith(".swift")]


@check("catalogue-has-14-languages", "language-missing", "every entra-guide string is in the catalogue in all 14 languages, with the same placeholders")
def _(c):
    cat = json.load(open(os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8"))["strings"]
    keys = {k for k in cat if k.startswith("entra-guide.")}
    need = {"entra-guide.c%d.title" % i for i in range(1, DATA["chapters"] + 1)} | {s["label"] for g in DATA["groups"].values() for s in g}
    for s in STEPS:
        need |= {"entra-guide.s%02d.say" % s["n"], "entra-guide.s%02d.cap" % s["n"]} | ({"entra-guide.s%02d.hint" % s["n"]} if s.get("hint") else set())
    for f in source_files(ROOT):
        need |= set(re.findall(r'"(entra-guide\.[a-z0-9.\-]+)"', open(f, encoding="utf-8").read()))
    need = {k for k in need if not k.endswith(".%@")}
    bad = []
    for k in sorted(need):
        loc = cat.get(k, {}).get("localizations", {})
        enp = (loc.get("en", {}).get("stringUnit", {}).get("value", "")).count("%@")
        for l in LANGS:
            v = loc.get(l, {}).get("stringUnit", {}).get("value", "")
            if not v or v.count("%@") != enp:
                bad.append("%s:%s" % (k, l))
    return [bad, sorted(need - keys)], [[], []]


@check("no-bare-string-in-the-guide", "bare-string", "every string the guide draws goes through Words.text (a catalogue key)")
def _(c):
    hit = []
    for f in source_files(ROOT):
        for i, line in enumerate(open(f, encoding="utf-8"), 1):
            if re.search(r'\b(Text|Label|TextField|Button)\(\s*"[A-Za-z][^"]*"', line) and "Words." not in line and "Image(" not in line:
                hit.append("%s:%d" % (os.path.basename(f), i))
    return hit, []


@check("pictures-are-bundled", "picture-missing", "every picture the steps use is in Sources/Resources/EntraGuide")
def _(c):
    d = os.path.join(ROOT, "Sources", "Resources", "EntraGuide")
    miss = [p["file"] for p in DATA["plates"] if p.get("file") and not os.path.exists(os.path.join(d, p["file"] + ".png"))]
    unused = [s["plate"] for s in STEPS if s["plate"] not in PLATES]
    return [miss, unused], [[], []]


# ---- run ----
def drive(c):
    CLICKED.clear(); SEEN.clear()
    out = []
    for name, mutant, words, fn in CHECKS:
        try:
            got, want = fn(c)
            out.append((name, got == want, "" if got == want else "got %r want %r" % (got, want)))
        except Failed as e:
            out.append((name, False, str(e)))
        except (Unreadable, IndexError, KeyError, TypeError, StopIteration) as e:
            out.append((name, None, "not judged: %s %s" % (type(e).__name__, e)))
    return out


def report(res):
    for n, ok, why in res:
        print("%s %s %s" % ("PASS" if ok else ("----" if ok is None else "FAIL"), n, why))
    f, u = sum(1 for _, ok, _ in res if ok is False), sum(1 for _, ok, _ in res if ok is None)
    print("%d checks, %d failed, %d not judged" % (len(res), f, u))
    return 1 if f or u else 0


# ---- the mutation rig: a model of the guide, one mutant switched on at a time ----
class Model:
    def __init__(self, mut):
        self.m = mut
        self.surface = "mac-main"
        self.win = (1280, 820)
        self.open = False
        self.leaving = False
        self.leave_at = 0.0
        self.idx = 0
        self.opened = False
        self.copied = 0
        self.copied_at = 0.0
        self.clip = ""
        self.pasted = ""
        self.links = []
        self.focus = False
        self.last_saved = ""
        self.addr = ""
        self.addr_focus = False
        self.domain = ""
        self.steps = STEPS[:3] + STEPS[4:] if "few-steps" in mut else STEPS

    # -- the screen's words (the real app reads the catalogue; the model reads the same file) --
    def n(self): return self.idx + 1
    def total(self): return len(self.steps)
    def step(self): return self.steps[self.idx]
    def last(self): return self.idx == self.total() - 1
    def valid(self):
        if "id-accepts-anything" in self.m: return len(self.pasted) > 0
        return bool(re.fullmatch(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}", self.pasted.strip()))
    def say(self, s):
        if "raw-key-shown" in self.m: return "entra-guide.s%02d.say" % s["n"]
        if s["n"] == 26 and "consent-domain-missing" in self.m: return say(26)
        return say(s["n"], self.domain)
    def count(self, n):
        return count(n).replace("of %d" % M, "of 7") if "counter-says-of-7" in self.m else count(n)
    def open_is_next(self): return self.step().get("primary") and not self.opened
    def expire(self):
        if self.leaving and time.time() - self.leave_at >= 0.24: self._gone()     # the close animation, on the clock the self-test makes virtual
        if self.copied and "copied-stays" not in self.m and time.time() - self.copied_at > 1.6: self.copied = 0

    def values(self):
        out, k = [], 0
        for v in DATA["groups"].get(self.step().get("group", ""), []):
            if v["copy"]: k += 1
            if v.get("dynamic") == "consent-link":
                v = dict(v, value=("https://login.microsoftonline.com/<domain>/adminconsent?client_id=<id>" if "consent-link-wrong" in self.m else consent_link(self.domain)))
            out.append({"def": v, "number": k if v["copy"] else 0, "current": self.step()["n"] in v["steps"] and "no-current-value" not in self.m})
        return out

    # -- doors --
    def go(self, i):
        if 0 <= i < self.total():
            if "slot-text-lost" in self.m and self.step().get("slot") and i > self.idx: self.pasted = ""
            self.idx = i; return True
        return False

    def do_next(self):
        if not self.open or self.leaving: return False
        if self.open_is_next():
            if "browser-not-opened" not in self.m: self.links.append(URL)
            self.opened = True; return True
        if self.last(): return self.do_save()
        if "step-skipped" in self.m and self.step()["n"] == 13: return self.go(self.idx + 2)
        return self.go(self.idx + 1)

    def do_back(self):
        if not self.open or self.leaving: return False
        if "back-goes-forward" in self.m: return self.go(self.idx + 1)
        if self.idx == 0 and "back-enabled-first" not in self.m: return False
        return self.go(max(self.idx - 1, 0)) if self.idx else True

    def do_skip(self):
        if not self.open or (self.last() and "skip-visible-last" not in self.m): return False
        self.go(self.total() - 2 if "skip-lands-short" in self.m else self.total() - 1); self.focus = True; return True

    def do_close(self, key=False):
        if not self.open or self.leaving: return False
        if ("escape-does-nothing" if key else "close-does-nothing") in self.m: return True
        self.leaving = True
        self.leave_at = time.time(); return True

    def _gone(self):
        self.open = False; self.leaving = False; self.focus = False

    def do_copy(self, n):
        if not self.open: return False
        if n == 2 and "copy-key-two-missing" in self.m and self._via_key: return False
        vs = [v for v in self.values() if v["number"] == n and n > 0]
        if not vs: return False
        v = vs[0]["def"]["value"]
        if "copy-wrong-value" in self.m and n == 1: v = [x for x in self.values() if x["number"] == 2][0]["def"]["value"] if len([x for x in self.values() if x["number"]]) > 1 else v + "!"
        if "permission-copy-wrong" in self.m and v == "SMTP.Send": v = "IMAP.AccessAsUser.All"
        self.clip = v; self.copied = n; self.copied_at = time.time(); return True

    def do_save(self):
        if not self.open or not self.last(): return False
        if not self.valid() and "save-without-valid" not in self.m: return False
        self.last_saved = "" if "save-loses-id" in self.m else self.pasted.strip()
        self.leaving = True
        self.leave_at = time.time(); return True

    def key(self, chord):
        self._via_key = True
        try:
            if chord == "Enter": return False if "enter-does-nothing" in self.m else self.do_next()
            if chord == "Escape": return self.do_close(key=True)
            if chord == "⌘[": return self.do_back()
            if chord == "⌘Down": return self.do_skip()
            if chord in ("⌘1", "⌘2", "⌘3"): return self.do_copy(int(chord[1]))
            return False
        finally:
            self._via_key = False
    _via_key = False

    def click(self, ax):
        if ax == "open":
            self.surface = A
            if "open-does-nothing" in self.m: return True
            if not self.open: self.domain = self.addr.split("@")[-1].lower() if re.fullmatch(r"[^@\s]+@[^@\s]+\.[^@\s]+", self.addr) else ""; self.open = True; self.leaving = False; self.idx = 0; self.opened = False; self.copied = 0; self.focus = False; self.links = []
            return True
        if ax == "next": return (not self.last()) and self.do_next()
        if ax == "save": return self.last() and self.do_save()
        if ax == "back": return self.do_back()
        if ax == "skip": return self.do_skip()
        if ax == "close": return self.do_close()
        if ax == "paste.field": self.focus = True if "paste-focus-missing" not in self.m else self.focus; return self.open and bool(self.step().get("slot") or self.step().get("paste"))
        if ax == "paste.clear":
            if not self.pasted: return False
            self.pasted = ""; return True
        if ax.startswith("copy."): return self.do_copy(int(ax[5:]))
        if ax.startswith("goto."): return self.open and not self.leaving and self.go(int(ax[5:]) - 1)
        return False

    # -- the rows --
    def drawn(self, s, p):
        # a plate is drawn when the data says so; the mutant stand-in-kept keeps the sign-in page as a stand-in
        return "drawn" in p or ("stand-in-kept" in self.m and s["plate"] == "signin")

    def srow(self, s, current):
        p = PLATES[s["plate"]]
        return {"n": s["n"], "of": self.total() if "few-steps" not in self.m else M, "count": self.count(s["n"]), "chapter": s["chapter"], "say": self.say(s), "cap": cap(s["n"]),
                "hint": EN["entra-guide.s%02d.hint" % s["n"]] if s.get("hint") else "", "plate": s["plate"], "drawn": self.drawn(s, p), "location": p["location"], "ring": s["ring"], "current": current}

    def rows(self, r):
        self.expire()
        e = []
        if r == "state":
            e = [{"open": self.open, "leaving": self.leaving, "steps": self.total(), "chapters": DATA["chapters"], "step": self.n(), "opened": self.opened, "copied": self.copied,
                  "clipboard": self.clip, "pasted": self.pasted, "valid_id": self.valid(), "opened_links": list(self.links), "last_saved": self.last_saved}]
        elif r == "step": e = [self.srow(self.step(), True)] if self.open else []
        elif r == "steps": e = [self.srow(s, s["n"] == self.step()["n"]) for s in self.steps]
        elif r == "controls" and self.open:
            e = [{"id": "close", "title": EN["Close"], "key": "esc", "enabled": True}]
            if not self.last() or "skip-visible-last" in self.m: e.append({"id": "skip", "title": EN["entra-guide.skip"], "key": "⌘↓" if "key-cap-missing" not in self.m else "", "enabled": True})
            e.append({"id": "back", "title": EN["Back"], "key": "⌘[", "enabled": self.idx > 0 or "back-enabled-first" in self.m})
            for v in self.values():
                if v["number"]: e.append({"id": "copy.%d" % v["number"], "title": EN["Copied"] if self.copied == v["number"] else EN["Copy"], "key": "⌘%d" % v["number"], "enabled": True})
            if self.last(): e.append({"id": "save", "title": EN["entra-guide.save"], "key": "↩", "enabled": self.valid()})
            else: e.append({"id": "next", "title": EN["entra-guide.open-entra"] if self.open_is_next() else EN["Next"], "key": "↩", "enabled": True})
        elif r == "values" and self.open:
            e = [{"label": EN[v["def"]["label"]], "value": v["def"]["value"], "copy": v["def"]["copy"], "number": v["number"], "current": v["current"], "copied": bool(v["number"]) and self.copied == v["number"],
                  "tag": "" if v["def"]["copy"] or "pick-tag-missing" in self.m else EN["entra-guide.pick-tag"]} for v in self.values()]
        elif r == "paste" and self.open and (self.step().get("slot") or self.step().get("paste")):
            wrong = bool(self.pasted) and not self.valid()
            note = "" if self.step().get("slot") else (EN["entra-guide.id-wrong"] if wrong else ("" if "keychain-note-missing" in self.m else EN["entra-guide.keychain"]))
            e = [{"kind": "slot" if self.step().get("slot") else "paste", "text": self.pasted, "valid": self.valid(), "tick": self.valid(), "focused": self.focus, "wrong": wrong, "note": note,
                  "label": EN["entra-guide.slot-label"] if self.step().get("slot") else EN["entra-guide.paste-label"]}]
        elif r == "progress" and self.open:
            e = [{"segments": self.total(), "filled": self.n(), "aurora": "aurora-always" in self.m or (self.last() and self.valid())}]
        elif r == "plate" and self.open:
            s = self.step(); p = PLATES[s["plate"]]
            ring = [1.4, s["ring"][1], 1.6, s["ring"][3]] if "ring-outside-picture" in self.m else s["ring"]
            flag = EN["entra-guide.drawn"] if self.drawn(s, p) else ""
            e = [{"plate": s["plate"], "drawn": self.drawn(s, p), "flag": flag, "location": p["location"], "ring": ring, "caption": cap(s["n"])}]
        return e

    def handle(self, verb, args):
        self.expire()
        ok = {"ok": True}
        if verb == "type":
            if self.addr_focus and not self.open:
                self.addr += args[0]; return ok
            if self.open and (self.step().get("slot") or self.step().get("paste")):
                self.pasted += args[0]
            return ok
        if verb == "key": return ok if self.key(args[0]) else {"ok": False, "error": "key does nothing"}
        if verb == "click" and args[0].startswith(SHEET + "."):
            ax = args[0][len(SHEET) + 1:]
            if ax == "start-over": self.addr = ""; self.addr_focus = False
            if ax == "address": self.addr_focus = True
            return ok
        if verb == "click":
            if "unclicked-control" in self.m and args[0].endswith(".help"): return ok
            return ok if self.click(args[0][len(A) + 1:]) else {"ok": False, "error": "did nothing"}
        if verb == "rows": return {"ok": True, "rows": self.rows(args[0][len(A) + 1:])}
        if verb == "resize": self.win = (float(args[0]), float(args[1])); return ok
        if verb == "measure":
            w, h = self.win
            top = 36 if (w >= 1100 or "compact-same-top" in self.m) else 16
            return {"ok": True, "frame": {"x": (w - 880) / 2, "y": top, "w": min(880, w - 32), "h": h - 2 * top}}
        return {"ok": False, "error": "nothing"}


def _selftest():
    def serve(mut):
        d = tempfile.mkdtemp(prefix="par-eg-")
        model = Model(mut)
        if "unclicked-control" in mut:
            _orig = model.rows
            model.rows = lambda r: _orig(r) + ([{"id": "help", "title": "Help", "key": "", "enabled": True}] if r == "controls" and model.open else [])

        class H(socketserver.StreamRequestHandler):
            def handle(self):
                for line in self.rfile:
                    q = json.loads(line.decode())
                    self.wfile.write((json.dumps(model.handle(q["verb"], q["args"])) + "\n").encode())
        srv = socketserver.ThreadingUnixStreamServer(os.path.join(d, "whoosh.sock"), H)
        threading.Thread(target=srv.serve_forever, daemon=True).start()
        return d, srv
    bad = 0
    d, srv = serve(set())
    base = drive(Ctl(d).connect())
    srv.shutdown()
    print("model (no mutant): %d of %d checks pass" % (sum(1 for _, ok, _ in base if ok), len(base)))
    for n, ok, why in base:
        if not ok: print("  RED on the model: %s %s" % (n, why)); bad += 1
    static = ("bare-string", "language-missing", "picture-missing")
    for name, mutant, words, fn in CHECKS:
        if mutant in static:
            continue
        d, srv = serve({mutant})
        res = {n: ok for n, ok, _ in drive(Ctl(d).connect())}
        srv.shutdown()
        red = [n for n, ok in res.items() if ok is False]
        verdict = "RED on %s" % name if res.get(name) is False else "NOT RED (red: %s)" % red
        print("mutant %-26s %-70s -> %s" % (mutant, words[:70], verdict))
        if res.get(name) is not False: bad += 1
    # the repo-reading checks: each is mutated on a copy of the tree
    global ROOT
    real = ROOT

    def scratch():
        t = tempfile.mkdtemp(prefix="par-eg-tree-")
        shutil.copytree(os.path.join(real, "Sources", "Features", "EntraGuide"), os.path.join(t, "Sources", "Features", "EntraGuide"))
        shutil.copytree(os.path.join(real, "Sources", "Resources", "EntraGuide"), os.path.join(t, "Sources", "Resources", "EntraGuide"))
        shutil.copy(os.path.join(real, "Sources", "Resources", "Localizable.xcstrings"), os.path.join(t, "Sources", "Resources", "Localizable.xcstrings"))
        return t

    def run(name, tree):
        global ROOT
        ROOT = tree
        try:
            got, want = next(fn for n, m, w, fn in CHECKS if n == name)(None)
        finally:
            ROOT = real
        return got != want
    t = scratch()
    open(os.path.join(t, "Sources", "Features", "EntraGuide", "EntraGuideHeadView.swift"), "w", encoding="utf-8").write('let bare = Text("Get the app ID")\n')
    red = run("no-bare-string-in-the-guide", t)
    print("mutant %-26s %-70s -> %s" % ("bare-string", "a string is drawn without Words.text", "RED on no-bare-string-in-the-guide" if red else "NOT RED")); bad += 0 if red else 1
    t = scratch()
    p = os.path.join(t, "Sources", "Resources", "Localizable.xcstrings")
    cat = json.load(open(p, encoding="utf-8")); del cat["strings"]["entra-guide.s13.say"]["localizations"]["ja"]
    json.dump(cat, open(p, "w", encoding="utf-8"), ensure_ascii=False)
    red = run("catalogue-has-14-languages", t)
    print("mutant %-26s %-70s -> %s" % ("language-missing", "one string lacks Japanese", "RED on catalogue-has-14-languages" if red else "NOT RED")); bad += 0 if red else 1
    t = scratch()
    os.remove(os.path.join(t, "Sources", "Resources", "EntraGuide", "entraguide-flow-07.png"))
    red = run("pictures-are-bundled", t)
    print("mutant %-26s %-70s -> %s" % ("picture-missing", "a step's picture is not in the bundle", "RED on pictures-are-bundled" if red else "NOT RED")); bad += 0 if red else 1
    print("%d checks, %d mutants not red" % (len(CHECKS), bad))
    return 1 if bad else 0


def selftest():
    """The stand-in run on a virtual wall clock: a sleep of s seconds moves the clock the model reads by s and costs 2 ms of real time
    (the checks sleep 2.0 s for Copied to lapse and poll up to 4 s per wait, about 36 times over for the mutants: that took over 420 s)."""
    real_sleep, real_time, skew = time.sleep, time.time, [0.0]

    def virtual_sleep(secs):
        skew[0] += secs
        real_sleep(min(secs, 0.002))
    time.sleep, time.time = virtual_sleep, lambda: real_time() + skew[0]
    try:
        return _selftest()
    finally:
        time.sleep, time.time = real_sleep, real_time


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        sys.exit(selftest())
    try:
        sys.exit(report(drive(Ctl().connect())))
    except CouldNotDrive as e:
        print("0 checks, 0 failed, %d not judged  (%s)" % (len(CHECKS), e)); sys.exit(3)
