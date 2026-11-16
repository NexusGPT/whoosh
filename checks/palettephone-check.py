#!/usr/bin/env python3
"""palettephone-check: the iPhone command palette, journey J1.9 on the phone (docs/JOURNEYS.md J1.9; brief mob-palettephone).

    python3 checks/palettephone-check.py             drive the RUNNING iPhone app over the control socket ($WHOOSH_SOCKET_DIR);
                                                     exit 1 when F > 0 or U > 0 (a check that could not run is not green)
    python3 checks/palettephone-check.py --steps     no app: each step of the Mac journey J1.9 against the phone's cases, one line per step
    python3 checks/palettephone-check.py --selftest  run every case against a stand-in app (StandIn below), once clean (all must pass)
                                                     and once per MUTANT (the named case must go red); exit 1 when any mutant is not seen red

docs/TESTING.md: a check is green only after it has been seen red on its mutant. StandIn is a model of the SPECIFIED behaviour (the
J1.9 steps, the phone's command list in docs/lanes/palettephone.md, the fixture's label tree), not of the production app: it exists so
that every case can be run red on a deliberate mutation without touching production code.

Expected values come from the fixture (docs/design/fixtures/mac-main-sample.json) and from the spec, never from a built screen.
ax ids (Sources/Features/PalettePhone/PalettePhoneFeature.swift): iphone-palette.{open,close,clear,submit,results.N,state,last,line,field}.
Items the socket contract does not name yet are ASSUMPTIONS: the verb `type` appends to the shared buffer (AppModel.typed), and
`click iphone-palette.clear` empties it; both are written by the screen's own code.
"""
import inspect
import json
import os
import re
import shutil
import socketserver
import subprocess
import sys
import tempfile
import threading
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable  # noqa: E402
from lib.fixture import FX  # noqa: E402
from lib.phonestart import leave_welcome  # noqa: E402

SEP = " › "
LIST = "iphone-inbox.list"

# What the phone lists as commands, by id (docs/lanes/palettephone.md "Command list"), and what it must never list.
PHONE_COMMANDS = {"command:archive", "command:snooze", "command:label", "command:pin", "command:unread", "command:undo",
                  "command:compose", "command:search"}
MAC_ONLY_WORDS = ["window", "notch", "keys", "key list", "sidebar", "split", "pop", "font", "emoji", "attach", "move to another account",
                  "new tab", "trainer", "reply all", "forward"]


def label_paths(nodes=None, prefix=()):
    """Every label of the fixture at any depth, as the palette shows it: names joined by ' › '."""
    out = []
    for n in (FX["labels"] if nodes is None else nodes):
        here = prefix + (n["name"],)
        out.append(SEP.join(here))
        out += label_paths(n.get("children") or [], here)
    return out


PATHS = label_paths()
NESTED = max(PATHS, key=lambda p: p.count(SEP))          # the deepest label of the fixture: Work > Clients > Provanta > RFP 2026 > Q4
assert NESTED.count(SEP) >= 3, "the fixture holds no nested label"


def why_not_driven():
    d = os.environ.get("WHOOSH_SOCKET_DIR", "")
    if d and os.path.exists(os.path.join(d, "whoosh.sock")):
        return ""
    try:
        out = subprocess.run(["xcrun", "simctl", "list", "runtimes"], capture_output=True, text=True, timeout=20).stdout
    except (OSError, subprocess.SubprocessError):
        out = ""
    if "iOS" not in out:
        return "the iPhone simulator cannot run: Xcode reports iOS 26.5 is not installed (owner step: Xcode > Settings > Components > iOS 26.5)"
    return "no iPhone app socket: launch the iPhone app in the simulator, then set WHOOSH_SOCKET_DIR"


# ---------------------------------------------------------------- helpers over the socket
def state(c):
    return c.rows("iphone-palette.state")[0]


def results(c):
    return c.rows("iphone-palette.results")


def last(c):
    r = c.rows("iphone-palette.last")
    return r[0] if r else {}


def ids(c):
    return [r["id"] for r in c.rows(LIST)]


def reset(c):
    if state(c)["open"]:
        c.click("iphone-palette.close")
    c.click("iphone-palette.open")
    if state(c)["query"]:
        c.click("iphone-palette.clear")


def index_of(c, row_id):
    for i, r in enumerate(results(c)):
        if r["id"] == row_id:
            return i
    return None


def run_line(c, term, row_id):
    """Open, type `term`, tap the line `row_id`. Returns False when the line is not listed."""
    reset(c)
    c.type(term)
    i = index_of(c, row_id)
    if i is None:
        return False
    try:
        c.click("iphone-palette.results.%d" % i)
    except Unreadable as e:
        # the app answered "did nothing": the row exists and is a command, so the app REFUSED it; its own words say why (PalettePhoneActs.run .refused)
        try:
            words = (state(c).get("line") or last(c).get("words") or "")
        except Exception:      # noqa: BLE001
            words = ""
        raise Unreadable("%s; the app's refusal words for %r (row %d): %r" % (e, row_id, i, words))
    return True


def subsequence(q, text):
    it = iter(text.lower())
    return all(ch in it for ch in q.lower())


# ---------------------------------------------------------------- the cases
CASES = []


def case(name, mutant, expected):
    def deco(fn):
        CASES.append((name, mutant, fn, expected))
        return fn
    return deco


@case("open", "open-noop", (True, "iphone-palette", True))
def _open(c):
    if state(c)["open"]:
        c.click("iphone-palette.close")
    c.click("iphone-palette.open")
    return (state(c)["open"], c.uistate()["surface"], len(results(c)) > 0)


# J1.9 opens the palette on the Unified Inbox and "the palette closes back to the Unified Inbox". The palette closes back to the surface
# it was opened from (PalettePhoneModel.open, close): run last, after `doors` left the search and Settings open under it, this case read
# 'iphone-search' (phone round 7). It runs while nothing but the palette was ever opened over the inbox.
@case("close", "close-noop", (False, "iphone-inbox"))
def _close(c):
    reset(c)
    c.click("iphone-palette.close")
    return (state(c)["open"], c.uistate()["surface"])


# A door picked while the search is already open must land on ITS screen (phone7: after the Search door and then the Settings door the palette's close showed
# `iphone-search`, the screen the first door had opened). The doors are the person's two picks in a row; the second screen is pushed over the open search.
@case("door-after-search-door-lands-on-its-screen", "late-search-reopens", ("iphone-search", "iphone-settings"))
def _door_after_search(c):
    run_line(c, "search", "command:search")
    time.sleep(0.6)                                  # the search model's late `active` change reaches the router after the next door otherwise
    first = c.uistate()["surface"]
    run_line(c, "settings", "settings")
    time.sleep(0.6)
    return (first, c.uistate()["surface"])


@case("suggested-first", "groups-unordered", ["suggested", "commands", "go-to"])
def _groups(c):
    reset(c)
    seen = []
    for r in results(c):
        if not seen or seen[-1] != r["group"]:
            seen.append(r["group"])
    return seen


@case("type-and-match", "no-filter", ("Archive", True, True))
def _match(c):
    reset(c)
    before = len(results(c))
    c.type("arch")
    rows = results(c)
    ok = bool(rows) and all(subsequence("arch", r["title"] + " " + r["detail"]) for r in rows)
    return (rows[0]["title"] if rows else None, ok, len(rows) < before)


@case("empty-words", "no-empty-words", (0, True))
def _empty(c):
    reset(c)
    c.type("zzzqx")
    s = state(c)
    return (s["count"], s["empty"] and "zzzqx" in s["words"])


@case("run-archive", "archive-keeps-row", (False, False, "done", "command:archive", True))
def _archive(c):
    first = ids(c)[0]
    ok = run_line(c, "archive", "command:archive")
    return (first in ids(c), state(c)["open"], last(c).get("outcome"), last(c).get("id"), ok and c.core("message/%s/archived" % first))


@case("run-snooze", "snooze-not-in-core", (False, False, "done", True))
def _snooze(c):
    first = ids(c)[0]
    ok = run_line(c, "snooze", "command:snooze")
    return (first in ids(c), state(c)["open"], last(c).get("outcome"), ok and c.core("message/%s/snoozed" % first))


@case("undo-entry-written", "no-undo-entry", (True, True))
def _undo(c):
    # J1.9 "undo-entry-written": a command that acted in the core leaves one undo entry (the Mac journey reads undo_entry/count)
    before = c.core("undo_entry/count")
    ok = run_line(c, "archive", "command:archive")
    after = c.core("undo_entry/count")
    return (ok, isinstance(before, int) and after == before + 1)


@case("nested-label", "no-nested-labels", (True, False, NESTED, "done", True))
def _nested(c):
    before = ids(c)
    reset(c)
    c.type(NESTED.split(SEP)[-1])
    i = next((k for k, r in enumerate(results(c)) if r["title"] == NESTED), None)
    if i is None:
        return (False, None, None, None, None)
    c.click("iphone-palette.results.%d" % i)
    return (True, state(c)["open"], last(c).get("title"), last(c).get("outcome"), ids(c) != before)


@case("mailbox-jump", "jump-stays-open", (True, False, "done"))
def _mailbox(c):
    ok = run_line(c, "starred", "mailbox:starred")
    return (ok, state(c)["open"], last(c).get("outcome"))


@case("submit-runs-first", "submit-noop", ("mailbox:trash", False))
def _submit(c):
    reset(c)
    c.type("trash")
    c.click("iphone-palette.submit")
    return (last(c).get("id"), state(c)["open"])


@case("doors", "settings-missing", [(True, False, "command:compose"), (True, False, "command:search"), (True, False, "settings")])
def _doors(c):
    out = []
    for term, rid in (("compose", "command:compose"), ("search", "command:search"), ("settings", "settings")):
        ok = run_line(c, term, rid)
        out.append((ok, state(c)["open"], last(c).get("id")))
    return out


@case("no-mac-only-command", "lists-window-command", (True, True, True))
def _no_mac(c):
    reset(c)
    rows = results(c)
    cmds = [r for r in rows if r["id"].startswith("command:")]
    low = [(r["title"] + " " + r["detail"]).lower() for r in rows]
    return (all(r["id"] in PHONE_COMMANDS for r in cmds),
            not any(w in t for w in MAC_ONLY_WORDS for t in low),
            not any("key" in r for r in rows))


@case("titles-unique", "duplicate-title", True)
def _unique(c):
    reset(c)
    # a command title is listed once; the mailbox "Archive" beside the command "Archive" differs by where it leads (the detail line)
    titles = [(r["title"].lower(), r["detail"].lower()) for r in results(c)]
    return len(titles) == len(set(titles)) and len(titles) > 0


@case("clear", "clear-noop", ("", True))
def _clear(c):
    reset(c)
    full = state(c)["count"]
    c.type("arch")
    c.click("iphone-palette.clear")
    return (state(c)["query"], state(c)["count"] == full)



# ---------------------------------------------------------------- the Mac journey's steps, one row each (J1.9 in journeys/J1.py)
# Every `s.expect("<name>"` and every `s.arrive(..., "<surface>")` inside the J1.9 step of the Mac journey is a key here, with what the phone
# does with it: "same" (the phone case expects the same kind of value: the Mac literal and the phone literal are both quoted), "substituted"
# (the phone repeats the step on its own command or surface, the reason says why the value differs), or "excluded" (the phone has no such
# thing, the reason says why; `proved_by` names the phone case that shows the absence). A Mac step with no row, a row whose Mac step is gone,
# a Mac literal that moved, a phone case or phone literal that is not there, or an exclusion with no reason: each is a red row.
MAC_JOURNEY = os.path.join(ROOT, "journeys", "J1.py")
MAC_STEP = "J1.9"
# key: (kind, phone case, Mac literal on the Mac line, phone literal in the phone case, reason)
STEP_TABLE = {
    "arrive:mac-palette": ("same", "open", "mac-palette", "iphone-palette", "the Mac key opens the palette surface; the phone's open control opens its own, iphone-palette"),
    "palette-result": ("substituted", "type-and-match", "Move to another account", "Archive",
                       "Move to another account is a Mac-only command (MAC_ONLY_WORDS); the phone types its own command and expects the first result to be it"),
    "key-hint-column-exists": ("excluded", "no-mac-only-command", "\"key\"", "\"key\" in r",
                               "the phone has no keyboard, so no result row carries a key field; the case asserts that no row does"),
    "palette-closes-to-inbox": ("same", "close", "mac-main", "iphone-inbox", "after the palette the screen is the inbox: the Mac surface is mac-main, the phone's is iphone-inbox"),
    "moved-in-core": ("substituted", "run-archive", "message/msg-05/account", "message/%s/archived",
                      "the account move is Mac only; the phone repeats the step 'the command acted in the core' on its Archive command, read at message/<id>/archived"),
    "row-shows-under-target-account": ("excluded", None, "mac-main.accounts.", None,
                                       "the Mac sidebar lists accounts and the row is read under the target; the phone's palette has no account sidebar (the account strip is the inbox's, InboxPhone)"),
    "undo-entry-written": ("same", "undo-entry-written", "undo_entry/count", "undo_entry/count", "both read the undo entry count: the Mac expects 7 in its journey, the phone expects one more than before its command"),
}
_PATCH = set()          # selftest only: names of journey mutants applied to the text of the Mac journey


def mac_journey_text():
    text = open(MAC_JOURNEY, encoding="utf-8").read()
    if "journey-gains-a-step" in _PATCH:
        i = text.index('s = j.step("%s"' % MAC_STEP)
        j = text.index("\n", i)
        text = text[: j + 1] + '    s.expect("palette-new-step", lambda: 1, 1)\n' + text[j + 1:]
    return text


def mac_steps(text, step=MAC_STEP):
    i = text.index('j.step("%s"' % step)
    j = text.find('j.step("', i + 1)
    out = {}
    for line in text[i: j if j > 0 else len(text)].splitlines():
        m = re.search(r's\.expect\("([^"]+)",(.*)$', line)
        if m:
            out[m.group(1)] = m.group(2)
        m = re.search(r's\.arrive\(.*,\s*"([^"]+)"\)', line)
        if m:
            out["arrive:" + m.group(1)] = line
    return out


def judge_steps(text):
    """(row, ok, why) for each Mac step and each table row."""
    mac = mac_steps(text)
    by_name = {n: (f, e) for n, _m, f, e in CASES}
    rows = []
    for name, line in mac.items():
        row = STEP_TABLE.get(name)
        if row is None:
            rows.append((name, False, "the Mac journey step has no phone case and no named exclusion"))
            continue
        kind, case_name, mac_lit, phone_lit, why = row
        if mac_lit not in line:
            rows.append((name, False, "the Mac step no longer holds %r" % mac_lit))
            continue
        if kind in ("same", "substituted") or case_name:
            if case_name not in by_name:
                rows.append((name, False, "phone case %s does not exist" % case_name))
                continue
            fn, expected = by_name[case_name]
            held = inspect.getsource(fn) + " " + repr(expected)
            if phone_lit and phone_lit not in held:
                rows.append((name, False, "phone case %s no longer holds %r" % (case_name, phone_lit)))
                continue
        if kind == "excluded" and len(why) < 30:
            rows.append((name, False, "an exclusion needs its reason"))
            continue
        rows.append((name, True, "%s -> %s: %s" % (kind, case_name or "no phone case", why)))
    for name in STEP_TABLE:
        if name not in mac:
            rows.append((name, False, "the table row names a Mac step that is not in %s any more" % MAC_STEP))
    return rows


VERBOSE = False


@case("steps-equal-the-mac-journey", "journey-gains-a-step", [])
def _steps(_c):
    rows = judge_steps(mac_journey_text())
    if VERBOSE:
        for name, ok, why in rows:
            print("%s %s/%s %s" % ("ok  " if ok else "FAIL", MAC_STEP, name, why))
    return [n for n, ok, _w in rows if not ok]


STATIC = {"steps-equal-the-mac-journey"}     # judged without the app


# ---------------------------------------------------------------- the stand-in app (selftest only)
class Err(Exception):
    pass


class StandIn:
    COMMANDS = [("archive", "Archive", "suggested"), ("snooze", "Snooze", "suggested"), ("label", "Label", "suggested"),
                ("compose", "Compose", "suggested"), ("pin", "Pin", "commands"), ("unread", "Mark read or unread", "commands"),
                ("undo", "Undo", "commands"), ("search", "Search", "commands")]
    MAILBOXES = ["All Inboxes", "VIPs", "Pinned", "Snoozed", "Drafts", "Sent", "Junk", "Trash", "Archive", "Starred", "Muted", "All mail"]

    def __init__(self, mutants=()):
        self.m = set(mutants)
        self.lock = threading.Lock()
        self.rows = ["msg-%02d" % n for n in range(1, 8)]
        self.core_state = {}
        self.is_open, self.typed, self.surface, self.last = False, "", "iphone-inbox", None
        self.back_to = "iphone-inbox"   # as PalettePhoneModel.open: the palette closes back to the surface it was opened from
        self.label_filter = None
        self.undo = 0

    def pool(self):
        out = []
        for cid, title, group in self.COMMANDS:
            out.append({"id": "command:" + cid, "title": title, "detail": "Mail", "group": group})
        out.append({"id": "settings", "title": "Settings", "detail": "", "group": "commands"})
        if "lists-window-command" in self.m:
            out.append({"id": "command:new-window", "title": "New window", "detail": "", "group": "commands"})
        if "duplicate-title" in self.m:
            out.append({"id": "command:archive-2", "title": "archive", "detail": "Mail", "group": "commands"})
        if "settings-missing" in self.m:
            out = [o for o in out if o["id"] != "settings"]
        for b in self.MAILBOXES:
            out.append({"id": "mailbox:" + b.lower().replace(" ", "-"), "title": b, "detail": "Mailbox", "group": "go-to"})
        for p in PATHS:
            if "no-nested-labels" in self.m and p.count(SEP) >= 2:
                continue
            out.append({"id": "label:" + p, "title": p, "detail": "Label", "group": "go-to"})
        return out

    def flat(self):
        pool = self.pool()
        if "no-filter" not in self.m and self.typed.strip():
            def hay(o):
                return o["title"] + (" " + o["detail"] if o["group"] == "go-to" else "")
            pool = [o for o in pool if subsequence(self.typed.strip(), hay(o))]
        order = ["suggested", "commands", "go-to"]
        if "groups-unordered" in self.m:
            order = ["go-to", "suggested", "commands"]
        return [o for g in order for o in pool if o["group"] == g]

    def handle(self, verb, args):
        with self.lock:
            fn = getattr(self, "v_" + verb, None)
            if fn is None:
                raise Err("model: no verb %s" % verb)
            return fn(*args) or {}

    def v_uistate(self):
        return {"state": {"surface": self.surface, "overlay": "iphone-palette" if self.is_open else None}}

    def v_type(self, text):
        if self.is_open:
            self.typed += text
        return {}

    def v_core(self, kind, path):
        if path == "undo_entry/count":
            return {"value": self.undo}
        return {"value": self.core_state.get(path, False)}

    def v_rows(self, ax):
        if ax == LIST:
            return {"rows": [{"ax": "%s.row.%d" % (LIST, i), "id": r} for i, r in enumerate(self.rows)]}
        if ax == "iphone-palette.state":
            n = len(self.flat()) if self.is_open else 0
            words = "No command matches “%s”" % self.typed if n == 0 and "no-empty-words" not in self.m else ""
            return {"rows": [{"open": self.is_open, "query": self.typed, "count": n, "empty": n == 0, "words": words, "line": ""}]}
        if ax == "iphone-palette.results":
            rows = [dict(ax="iphone-palette.results.%d" % i, id=o["id"], title=o["title"], detail=o["detail"], group=o["group"],
                         symbol="x", account_mark=False) for i, o in enumerate(self.flat())] if self.is_open else []
            return {"rows": rows}
        if ax == "iphone-palette.last":
            return {"rows": [self.last] if self.last else []}
        raise Err("no rows at %s" % ax)

    def v_click(self, ax):
        if ax == "iphone-palette.open":
            if "open-noop" not in self.m and not self.is_open:
                self.back_to = self.surface
                self.is_open, self.typed, self.surface = True, "", "iphone-palette"
            return {}
        if not self.is_open:
            raise Err("not open")
        if ax == "iphone-palette.close":
            if "close-noop" not in self.m:
                self._close()
            return {}
        if ax == "iphone-palette.clear":
            if "clear-noop" not in self.m:
                self.typed = ""
            return {}
        if ax == "iphone-palette.submit":
            if "submit-noop" not in self.m:
                self._run(0)
            return {}
        if ax.startswith("iphone-palette.results."):
            self._run(int(ax.rsplit(".", 1)[1]))
            return {}
        raise Err("no click target %s" % ax)

    def _close(self):
        self.is_open, self.typed, self.surface = False, "", self.back_to

    # The screens the doors open stay under the next palette, as in the app: the Search door opens the search over the inbox
    # (PhoneRouter.overlayChanged -> openSearch), Settings is pushed; Compose is a sheet that names no surface.
    DOOR_SURFACES = {"command:search": "iphone-search", "settings": "iphone-settings"}

    def _run(self, i):
        flat = self.flat()
        if not 0 <= i < len(flat):
            raise Err("no line %d" % i)
        o = flat[i]
        first = self.rows[0] if self.rows else None
        if o["id"] in ("command:archive", "command:snooze") and first and "no-undo-entry" not in self.m:
            self.undo += 1
        if o["id"] == "command:archive" and first:
            if "archive-keeps-row" not in self.m:
                self.rows.remove(first)
            self.core_state["message/%s/archived" % first] = True
        elif o["id"] == "command:snooze" and first:
            self.rows.remove(first)
            if "snooze-not-in-core" not in self.m:
                self.core_state["message/%s/snoozed" % first] = True
        elif o["id"].startswith("label:"):
            self.rows = self.rows[: max(1, len(self.rows) // 2)]
        self.last = {"id": o["id"], "title": o["title"], "outcome": "done", "words": ""}
        if "jump-stays-open" in self.m and o["id"].startswith("mailbox:"):
            return
        self._close()
        searching = self.surface == "iphone-search"
        self.surface = self.DOOR_SURFACES.get(o["id"], self.surface)
        if "late-search-reopens" in self.m and searching and o["id"] == "settings":
            self.surface = "iphone-search"            # the broken copy: the first door's late `active` change opens the search again over the pushed Settings


def serve(app):
    d = tempfile.mkdtemp(prefix="pp", dir="/tmp")

    class H(socketserver.StreamRequestHandler):
        def handle(self):
            for line in self.rfile:
                try:
                    req = json.loads(line.decode())
                    out = {"ok": True, **app.handle(req["verb"], req.get("args", []))}
                except Err as e:
                    out = {"ok": False, "error": str(e)}
                except Exception as e:  # a model bug must never read as a pass
                    out = {"ok": False, "error": "model: %r" % e}
                self.wfile.write((json.dumps(out) + "\n").encode())

    class S(socketserver.ThreadingUnixStreamServer):
        daemon_threads = True

    srv = S(os.path.join(d, "whoosh.sock"), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv, d


def stop(srv, d):
    srv.shutdown()
    srv.server_close()
    for name in os.listdir(d):
        os.unlink(os.path.join(d, name))
    os.rmdir(d)


# ---------------------------------------------------------------- the runner
def run_cases(c, why=""):
    res = []
    if c is not None:
        leave_welcome(c)    # a test copy starts its first launch on the welcome: the palette returns to the surface it was opened from (phone4: `close` got iphone-welcome-accounts), so leave it by the flow's own controls first
    for name, _mutant, fn, expected in CASES:
        if c is None and name not in STATIC:
            res.append((name, "unjudged", why))
            continue
        try:
            got = fn(c)
            res.append((name, "pass" if got == expected else "fail", "expected %r, got %r" % (expected, got)))
        except Unreadable as e:
            res.append((name, "unjudged", str(e)))
        except (IndexError, KeyError, TypeError) as e:
            res.append((name, "fail", "reply shape: %s %s" % (type(e).__name__, e)))
    return res


def run_against(mutants):
    _PATCH.clear()
    _PATCH.update(mutants)
    app = StandIn(mutants)
    srv, d = serve(app)
    try:
        c = Ctl(d).connect()
        try:
            return run_cases(c)
        finally:
            c.close()
    finally:
        stop(srv, d)


def selftest():
    bad = []
    clean = run_against(())
    for name, st, detail in clean:
        if st != "pass":
            bad.append("clean run: %s %s (%s)" % (name, st, detail))
    mutant_of = {name: m for name, m, _f, _e in CASES}
    seen_red = 0
    print("| case | mutant | red on its mutant |")
    print("|---|---|---|")
    for name, m in mutant_of.items():
        res = dict((n, s) for n, s, _d in run_against((m,)))
        red = res.get(name) == "fail"
        seen_red += red
        print("| %s | %s | %s |" % (name, m, "yes" if red else "NO"))
        if not red:
            bad.append("mutant %s did not turn %s red (got %s)" % (m, name, res.get(name)))
    for b in bad:
        sys.stderr.write("SELFTEST FAIL %s\n" % b)
    print("selftest: %d cases green on the clean stand-in, %d of %d mutants seen red, %d problems"
          % (sum(1 for _n, s, _d in clean if s == "pass"), seen_red, len(mutant_of), len(bad)))
    return 1 if bad else 0


def steps_only():
    """The static half: the Mac journey's J1.9 steps against the phone's cases, one printed line per step. No app, no socket."""
    rows = judge_steps(mac_journey_text())
    for name, ok, why in rows:
        print("%s %s/%s %s" % ("ok  " if ok else "FAIL", MAC_STEP, name, why))
    bad = sum(1 for _n, ok, _w in rows if not ok)
    print("%d steps, %d failed, 0 not judged" % (len(rows), bad))
    return 1 if bad else 0


def main():
    if "--selftest" in sys.argv:
        return selftest()
    if "--steps" in sys.argv:
        return steps_only()
    reason = why_not_driven()
    c = None
    if not reason:
        try:
            c = Ctl().connect()
        except CouldNotDrive as e:
            reason = str(e)
    global VERBOSE
    VERBOSE = True
    res = run_cases(c, reason)
    for name, st, _detail in res:
        if st == "pass":
            print("ok   %s" % name)
    print("MUTANTS (a check is green only after it has been seen red on its mutant)")
    print("| check | mutant |")
    print("|---|---|")
    for name, m, _f, _e in CASES:
        print("| %s | %s |" % (name, m))
    failed = unjudged = 0
    for name, st, detail in res:
        if st == "fail":
            failed += 1
            sys.stderr.write("FAIL %s: %s\n" % (name, detail))
        elif st == "unjudged":
            unjudged += 1
            sys.stderr.write("NOT JUDGED %s: %s\n" % (name, detail))
    if c is not None:
        c.close()
    print("%d checks, %d failed, %d not judged" % (len(res), failed, unjudged))
    return 0 if failed == 0 and unjudged == 0 and res else 1


if __name__ == "__main__":
    sys.exit(main())
