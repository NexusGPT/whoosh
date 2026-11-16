#!/usr/bin/env python3
"""
settingsphone: the iPhone Settings screen (docs/JOURNEYS.md J4.11; docs/CAPABILITY-CONTRACT.md C-J4.11), driven through the control socket.

    python3 checks/settingsphone-check.py             drive the running iPhone app ($WHOOSH_SOCKET_DIR); exit 1 when F > 0 or U > 0
    python3 checks/settingsphone-check.py --selftest  run every check against a model of the design, clean, then once per MUTANT of it:
                                                      each mutant must turn its own check red; exit 0 only then

Prints `N checks, F failed, U not judged`. Expected values come from the design fixture (docs/design/fixtures/mac-main-sample.json),
the catalogue (Sources/Resources/Localizable.xcstrings), the language list in Sources/Localization/Words.swift and the acceptance of
todo mob-settingsphone, never from a built screen. The model in this file is the mutation rig only. No mail account is connected and
nothing leaves this machine: the only transport is the local control socket.

The screen's ids (`<surface>.<region>[.<element>...][.<index>]`):
  rows   iphone-settings.nav | .title | .pages | .summary | .sync | .accounts | .language | .appearance | .ai
  clicks iphone-settings.row.<page> | .summary | .back | .done | .sync.retry | .language.<code> | .appearance.<id>
  fixture verbs the check needs and the fixture core must serve:  `fixture sync broken|ok` (the sync line's failure) and
  `click ai-setup.preview.<need>.<state>` (the AI setup's drawn states). A verb the rig refuses leaves its check not judged.
  `click iphone-mailboxes.settings` opens the screen from the Mailboxes foot.
"""
import json
import os
import re
import socket
import socketserver
import sys
import tempfile
import threading

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.fixture import FX, FAILING_ACCOUNT  # noqa: E402

# docs/CONSTANTS.md has no SYNC_RECENT_S yet (docs/lanes/devicesync.md names the gap). The acceptance of mob-settingsphone says
# "a time inside SYNC_RECENT_S"; until the ledger holds it this private figure stands, 300 s, the one the old check and the
# Swift test already used. It is listed in docs/lanes/settingsphone.md for the ledger.
SYNC_RECENT_S = 300                    # FALLBACK, not a ledger value: used only while docs/CONSTANTS.md has no SYNC_RECENT_S row
_FALLBACK = SYNC_RECENT_S
SYNC_RECENT_SOURCE = "private fallback (docs/CONSTANTS.md has no SYNC_RECENT_S row)"
_FALLBACK_SOURCE = SYNC_RECENT_SOURCE   # kept apart: SYNC_RECENT_SOURCE is reassigned below once the ledger has the row


def read_sync_recent(path=None):
    """(seconds, where it came from): the ledger row SYNC_RECENT_S by name when there is one, else the fallback above, said as such."""
    import lib.fixture as fixture
    old = fixture.CONSTANTS_PATH
    if path:
        fixture.CONSTANTS_PATH = path
    try:
        return fixture.const("SYNC_RECENT_S"), "docs/CONSTANTS.md row SYNC_RECENT_S"
    except KeyError:
        return _FALLBACK, _FALLBACK_SOURCE
    finally:
        fixture.CONSTANTS_PATH = old


SYNC_RECENT_S, SYNC_RECENT_SOURCE = read_sync_recent()

PAGES = ["accounts", "sync", "ai", "language", "appearance"]
APPEARANCES = ["auto", "light", "dark"]
MAC = "Mac"   # the paired Mac's name: Tests/Unit/WelcomePhoneTests.swift names `settings_sync/devices` == ["Mac"]

with open(os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8") as _f:
    CATALOGUE = json.load(_f)["strings"]
with open(os.path.join(ROOT, "Sources", "Localization", "Words.swift"), encoding="utf-8") as _f:
    LANGS = re.findall(r'\("([A-Za-z-]+)", "([^"]+)"\)', _f.read().split("public static let languages", 1)[1].split("= [", 1)[1].split("]", 1)[0])
CHOICES = [("system", "System")] + LANGS


def word(key, code):
    """The catalogue's text for `key` in language `code` (the English key itself for `system` and `en`)."""
    if code in ("system", "en"):
        return key
    return CATALOGUE[key]["localizations"][code]["stringUnit"]["value"]


def mac_order(accounts):
    """The Mac's Accounts pane order: groups in order of first appearance, the ungrouped last, the file's order inside each."""
    groups = []
    for a in accounts:
        if a.get("group") and a["group"] not in groups:
            groups.append(a["group"])
    return [a for g in groups for a in accounts if a.get("group") == g] + [a for a in accounts if not a.get("group")]


EXPECTED = mac_order(FX["accounts"])


class Unreadable(Exception):
    """A read or an action failed: the check that needed it is not judged."""


class Rig:
    """Control-socket client (docs/CONTROL-SOCKET.md). One JSON line out, one back."""

    def __init__(self, sock_dir):
        self.path = os.path.join(sock_dir, "whoosh.sock") if sock_dir else ""
        self.s, self.buf = None, b""

    def connect(self):
        if not self.path or not os.path.exists(self.path):
            raise Unreadable("no socket at %r (launch the iPhone app under scripts/launch-one.sh, set WHOOSH_SOCKET_DIR; "
                             "without an iOS 26.5 simulator runtime there is none)" % self.path)
        self.s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.s.settimeout(5)
        self.s.connect(self.path)
        return self

    def send(self, verb, *args):
        try:
            self.s.sendall((json.dumps({"verb": verb, "args": list(args)}) + "\n").encode())
            while b"\n" not in self.buf:
                chunk = self.s.recv(65536)
                if not chunk:
                    raise OSError("socket closed")
                self.buf += chunk
            line, self.buf = self.buf.split(b"\n", 1)
            r = json.loads(line.decode())
        except (OSError, ValueError) as e:
            raise Unreadable("%s %s: %s" % (verb, args, e))
        if not isinstance(r, dict) or r.get("ok") is not True:
            raise Unreadable("%s %s: %s" % (verb, args, r.get("error") if isinstance(r, dict) else r))
        return r

    def click(self, ax):
        return self.send("click", ax)

    def fixture(self, *args):
        return self.send("fixture", *args)

    def rows(self, ax):
        return self.send("rows", ax).get("rows", [])

    def one(self, ax):
        got = self.rows(ax)
        if len(got) != 1:
            raise Unreadable("rows %s: wanted one row, got %d" % (ax, len(got)))
        return got[0]


CHECKS = []   # (name, mutant key, mutation words, fn)


def check(name, mutant, words):
    def deco(fn):
        CHECKS.append((name, mutant, words, fn))
        return fn
    return deco


def run_checks(rig, only=None):
    out = []
    for name, _m, _w, fn in CHECKS:
        if only and not only(name):
            continue
        try:
            got, want = fn(rig)
            out.append((name, "pass" if got == want else "fail", "expected %r, got %r" % (want, got)))
        except Unreadable as e:
            out.append((name, "unjudged", str(e)))
        except (KeyError, IndexError, TypeError, ValueError) as e:
            out.append((name, "fail", "reply shape: %s %s" % (type(e).__name__, e)))
    return out


def summary(res):
    f = sum(1 for r in res if r[1] == "fail")
    u = sum(1 for r in res if r[1] == "unjudged")
    return "%d checks, %d failed, %d not judged" % (len(res), f, u), f, u


# ---- helpers every check starts from -------------------------------------------------------------------------------------------

def open_from_mailboxes(r):
    """Settings from the Mailboxes foot. The app launches on the inbox (PhoneNav.path = [.inbox], PhoneRoutes.swift), where the Settings door answers
    "did nothing" (it lives on Mailboxes only, InboxPhoneFeature.swift): main88 read all 20 cases not judged for that. Go back to Mailboxes with the
    inbox's own Back door (`iphone-inbox.back`) and ask again; the first ask stays the first try, so a build that opens it at once is not changed."""
    try:
        r.click("iphone-mailboxes.settings")
    except Unreadable:
        r.click("iphone-inbox.back")
        r.click("iphone-mailboxes.settings")


def home(r, strict=True):
    """The screen open on its home list, every page closed. `strict=False` is for the check whose subject is Back itself:
    a Back that does nothing must turn that check red, not leave it not judged."""
    if not r.one("iphone-settings.nav").get("open"):
        open_from_mailboxes(r)
    for _ in range(len(PAGES) + 1):
        if r.one("iphone-settings.nav")["depth"] == 0:
            break
        r.click("iphone-settings.back")
    if strict and r.one("iphone-settings.nav")["depth"] != 0:
        raise Unreadable("Back does not return to the home list")


def restore(r):
    for ax in ("iphone-settings.language.system", "iphone-settings.appearance.auto"):
        try:
            r.click(ax)
        except Unreadable:
            pass
    try:
        r.fixture("sync", "ok")
    except Unreadable:
        pass


def on_page(r, page):
    r.click("iphone-settings.row.%s" % page)
    return r.one("iphone-settings.nav")["page"]


# ---- the checks ----------------------------------------------------------------------------------------------------------------

@check("pages-listed", "extra-pane", "a Shortcuts row appears: the phone has no Shortcuts and no Notch pane")
def _(r):
    home(r)
    return [p["id"] for p in r.rows("iphone-settings.pages")], PAGES


@check("sync-names-the-mac", "no-mac-name", "the Sync line says 'In sync' and no Mac's name")
def _(r):
    home(r); r.fixture("sync", "ok"); r.click("iphone-settings.row.sync")
    s = r.one("iphone-settings.sync")
    return (s["line"], s["device"]), ("In sync with %s" % s["device"], MAC)


@check("sync-time-recent", "stale-time", "the last sync time is older than SYNC_RECENT_S")
def _(r):
    home(r); r.fixture("sync", "ok"); r.click("iphone-settings.row.sync")
    age = r.one("iphone-settings.sync")["age_s"]
    return 0 <= age < SYNC_RECENT_S, True


@check("sync-ok-has-no-retry", "retry-in-sync", "Retry shows while the sync is fine")
def _(r):
    home(r); r.fixture("sync", "ok"); r.click("iphone-settings.row.sync")
    return r.one("iphone-settings.sync")["retry"], False


@check("sync-failure-says-why", "no-why", "a failed sync gives no reason")
def _(r):
    home(r); r.fixture("sync", "broken"); r.click("iphone-settings.row.sync")
    s = r.one("iphone-settings.sync")
    r.fixture("sync", "ok")
    return len(s["reason"]) > 0 and not s["line"].startswith("In sync with "), True


@check("sync-failure-offers-retry", "no-retry-failing", "a failed sync offers no Retry")
def _(r):
    home(r); r.fixture("sync", "broken"); r.click("iphone-settings.row.sync")
    got = r.one("iphone-settings.sync")["retry"]
    r.fixture("sync", "ok")
    return got, True


@check("retry-moves-the-state", "retry-inert", "the Retry button does nothing")
def _(r):
    home(r); r.fixture("sync", "broken"); r.click("iphone-settings.row.sync")
    before = r.one("iphone-settings.sync")["retries"]
    r.click("iphone-settings.sync.retry")
    after = r.one("iphone-settings.sync")["retries"]
    r.fixture("sync", "ok")
    return after, before + 1


@check("language-rows", "lang-missing-pt", "Português is missing from the language list")
def _(r):
    home(r); r.click("iphone-settings.row.language")
    got = r.rows("iphone-settings.language")
    return [(g["code"], g["name"]) for g in got], CHOICES


@check("language-changes-the-words", "lang-inert", "picking Français leaves the title in English")
def _(r):
    home(r); r.click("iphone-settings.row.language")
    try:
        r.click("iphone-settings.language.fr")
        got = (r.one("iphone-settings.title")["text"], [g["code"] for g in r.rows("iphone-settings.language") if g["on"]])
    finally:
        restore(r)
    return got, (word("Settings", "fr"), ["fr"])


@check("appearance-pick-kept", "appearance-inert", "the Dark pick is not kept")
def _(r):
    home(r); r.click("iphone-settings.row.appearance")
    try:
        r.click("iphone-settings.appearance.dark")
        got = [a["id"] for a in r.rows("iphone-settings.appearance") if a["on"]]
    finally:
        restore(r)
    return got, ["dark"]


@check("accounts-equal-the-fixture", "accounts-missing", "the last account is missing from the list")
def _(r):
    home(r); r.click("iphone-settings.row.accounts")
    got = r.rows("iphone-settings.accounts")
    return sorted((g["id"], g["name"], g["address"], g["mark_colour"]) for g in got), \
        sorted((a["id"], a["name"], a["address"], a["colour"]) for a in FX["accounts"])


@check("accounts-in-the-macs-order", "accounts-order", "the list is in the file's order, not the Mac's grouped order")
def _(r):
    home(r); r.click("iphone-settings.row.accounts")
    return [g["id"] for g in r.rows("iphone-settings.accounts")], [a["id"] for a in EXPECTED]


@check("failing-account-marked", "failing-unmarked", "the account that needs a sign-in carries no attention mark")
def _(r):
    home(r); r.click("iphone-settings.row.accounts")
    return [g["id"] for g in r.rows("iphone-settings.accounts") if g["mark_attention"]], [FAILING_ACCOUNT]


@check("ai-missing-mark", "ai-no-mark", "a missing model shows no missing-setup mark")
def _(r):
    home(r); r.click("iphone-settings.row.ai")
    try:
        r.click("ai-setup.preview.writing.missing")
        miss = {g["need"]: g for g in r.rows("iphone-settings.ai")}["writing"]
        r.click("ai-setup.preview.writing.ready")
        ready = {g["need"]: g for g in r.rows("iphone-settings.ai")}["writing"]
    finally:
        try:
            r.click("ai-setup.preview.writing.clear")
        except Unreadable:
            pass
    return (miss["missing"], len(miss["mark"]) > 0, ready["missing"], ready["mark"]), (True, True, False, "")


@check("every-row-opens-its-page", "row-inert", "the Sync row opens nothing")
def _(r):
    got = []
    for p in PAGES:
        home(r)
        got.append((p, on_page(r, p)))
    return got, [(p, p) for p in PAGES]


@check("summary-opens-accounts", "summary-inert", "the accounts card opens nothing")
def _(r):
    home(r); r.click("iphone-settings.summary")
    return r.one("iphone-settings.nav")["page"], "accounts"


@check("back-returns", "back-inert", "Back stays on the page")
def _(r):
    home(r, strict=False); r.click("iphone-settings.row.language"); r.click("iphone-settings.back")
    return r.one("iphone-settings.nav")["page"], "home"


@check("done-closes", "done-inert", "Done leaves the screen open")
def _(r):
    home(r); r.click("iphone-settings.done")
    got = r.one("iphone-settings.nav")["open"]
    open_from_mailboxes(r)   # leave the screen as the next check finds it
    return got, False


@check("every-language-tapped", "lang-one-inert", "the Japanese row does nothing")
def _(r):
    home(r); r.click("iphone-settings.row.language")
    got = []
    try:
        for code, _name in CHOICES:
            r.click("iphone-settings.language.%s" % code)
            got.append((code, [g["code"] for g in r.rows("iphone-settings.language") if g["on"]]))
    finally:
        restore(r)
    return got, [(c, [c]) for c, _n in CHOICES]


@check("every-appearance-tapped", "appearance-dark-inert", "the Dark row does nothing")
def _(r):
    home(r); r.click("iphone-settings.row.appearance")
    got = []
    try:
        for a in APPEARANCES:
            r.click("iphone-settings.appearance.%s" % a)
            got.append((a, [x["id"] for x in r.rows("iphone-settings.appearance") if x["on"]]))
    finally:
        restore(r)
    return got, [(a, [a]) for a in APPEARANCES]


# ---- the mutation rig: a model of the design, and each way it can be wrong ----------------------------------------------------------

class Model:
    """What the screen must do, as the socket sees it. `mut` is the set of mutations in force (empty = the design)."""

    def __init__(self, mut):
        self.mut = mut
        self.open, self.stack = True, []
        self.lang, self.look = "system", "auto"
        self.broken, self.retries = False, 0
        self.ai = {"writing": "missing", "decision": "ready"}

    def has(self, k):
        return k in self.mut

    def accounts(self):
        a = list(FX["accounts"]) if self.has("accounts-order") else mac_order(FX["accounts"])
        return a[:-1] if self.has("accounts-missing") else a

    def handle(self, verb, args):
        if verb == "rows":
            return {"ok": True, "rows": getattr(self, "rows_" + args[0].replace("iphone-settings.", "").replace(".", "_"))()}
        if verb == "click":
            return self.click(args[0])
        if verb == "fixture" and args[:1] == ["sync"] and args[1:2] in (["broken"], ["ok"]):
            self.broken = args[1] == "broken"
            return {"ok": True}
        if verb == "uistate":
            return {"ok": True, "state": {"surface": "iphone-settings" if self.open else "iphone-mailboxes"}}
        return {"ok": False, "error": "unknown verb %s %s" % (verb, args)}

    def click(self, ax):
        m, p = self.mut, ax.replace("iphone-settings.", "")
        if ax == "iphone-mailboxes.settings":
            self.open, self.stack = True, []
        elif p.startswith("row."):
            if not (p == "row.sync" and "row-inert" in m):
                self.stack.append(p[4:])
        elif p == "summary":
            if "summary-inert" not in m:
                self.stack.append("accounts")
        elif p == "back":
            if "back-inert" not in m and self.stack:
                self.stack.pop()
        elif p == "done":
            if "done-inert" not in m:
                self.open, self.stack = False, []
        elif p == "sync.retry":
            if not self.broken:
                return {"ok": False, "error": "no such control"}
            if "retry-inert" not in m:
                self.retries += 1
        elif p.startswith("language."):
            code = p[9:]
            if code not in [c for c, _n in CHOICES]:
                return {"ok": False, "error": "no such control"}
            if "lang-inert" not in m and not (code == "ja" and "lang-one-inert" in m):
                self.lang = code
        elif p.startswith("appearance."):
            a = p[11:]
            if a not in APPEARANCES:
                return {"ok": False, "error": "no such control"}
            if "appearance-inert" not in m and not (a == "dark" and "appearance-dark-inert" in m):
                self.look = a
        elif ax.startswith("ai-setup.preview.writing."):
            self.ai["writing"] = {"missing": "missing", "ready": "ready", "clear": "missing"}[ax.rsplit(".", 1)[1]]
        else:
            return {"ok": False, "error": "no such control %s" % ax}
        return {"ok": True}

    def rows_nav(self):
        return [{"ax": "iphone-settings.nav", "page": self.stack[-1] if self.stack else "home", "depth": len(self.stack), "open": self.open}]

    def rows_title(self):
        code = "en" if self.lang == "system" else self.lang
        return [{"ax": "iphone-settings.title", "text": "Settings" if self.has("lang-inert") else word("Settings", code)}]

    def rows_pages(self):
        return [{"ax": "iphone-settings.row." + p, "id": p} for p in (PAGES + ["shortcuts"] if self.has("extra-pane") else PAGES)]

    def rows_sync(self):
        line = "In sync" if self.has("no-mac-name") else "In sync with %s" % MAC
        if self.broken:
            reason = "" if self.has("no-why") else "iCloud did not answer."
            return [{"ax": "iphone-settings.sync", "line": "Not in sync with %s" % MAC, "device": MAC, "age_s": 900.0, "reason": reason,
                     "retry": "no-retry-failing" not in self.mut, "retries": self.retries}]
        return [{"ax": "iphone-settings.sync", "line": line, "device": MAC, "age_s": 99999.0 if self.has("stale-time") else 42.0, "reason": "",
                 "retry": self.has("retry-in-sync"), "retries": self.retries}]

    def rows_accounts(self):
        out = []
        for i, a in enumerate(self.accounts()):
            attention = a["id"] == FAILING_ACCOUNT and not self.has("failing-unmarked")
            out.append({"ax": "iphone-settings.accounts.row.%d" % i, "id": a["id"], "name": a["name"], "address": a["address"],
                        "mark_colour": a["colour"], "mark_attention": attention})
        return out

    def rows_language(self):
        return [{"ax": "iphone-settings.language." + c, "code": c, "name": n, "on": c == self.lang}
                for c, n in CHOICES if not (c == "pt" and self.has("lang-missing-pt"))]

    def rows_appearance(self):
        return [{"ax": "iphone-settings.appearance." + a, "id": a, "on": a == self.look} for a in APPEARANCES]

    def rows_ai(self):
        out = []
        for need, st in self.ai.items():
            miss = st != "ready"
            out.append({"ax": "iphone-settings.ai." + need, "need": need, "missing": miss,
                        "mark": "No provider is connected. Paste a key." if miss and "ai-no-mark" not in self.mut else ""})
        return out


class _H(socketserver.StreamRequestHandler):
    def handle(self):
        for line in self.rfile:
            try:
                req = json.loads(line.decode())
                rep = self.server.model.handle(req["verb"], req.get("args", []))
            except Exception as e:  # a model bug must show as a refused request
                rep = {"ok": False, "error": "model: %s" % e}
            self.wfile.write((json.dumps(rep) + "\n").encode())


class _S(socketserver.ThreadingMixIn, socketserver.UnixStreamServer):
    daemon_threads = True


def with_fake(model, fn):
    d = tempfile.mkdtemp(prefix="sphfake")
    srv = _S(os.path.join(d, "whoosh.sock"), _H)
    srv.model = model
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    r = None
    try:
        r = Rig(d).connect()
        return fn(r)
    finally:
        if r is not None and r.s is not None:
            r.s.close()   # on every path: an open connection keeps its handler thread, and server_close() joins it for ever
        srv.shutdown()
        srv.server_close()
        for n in os.listdir(d):      # the stand-in's folder goes with it (it used to stay behind, one per run)
            os.unlink(os.path.join(d, n))
        os.rmdir(d)


def selftest():
    """The base model: every check passes. Each mutant: its own check goes red. Exit 0 only then."""
    bad = 0
    base = with_fake(Model(set()), lambda r: run_checks(r))
    line, f, u = summary(base)
    print("base model: " + line)
    if f or u:
        bad += 1
        for n, st, d in base:
            if st != "pass":
                print("  BASE %s %s: %s" % (st.upper(), n, d))
    # the constant is read by name: a ledger row of a temporary copy is found, and without a row the fallback is named as such
    d = tempfile.mkdtemp(prefix="sphconst")
    try:
        with open(os.path.join(d, "C.md"), "w", encoding="utf-8") as fh:
            fh.write("| Name | Value | Why |\n|---|---|---|\n| SYNC_RECENT_S | 45 | test row |\n")
        got_row = read_sync_recent(os.path.join(d, "C.md"))
        with open(os.path.join(d, "N.md"), "w", encoding="utf-8") as fh:
            fh.write("| Name | Value | Why |\n|---|---|---|\n| OTHER | 7 | none |\n")
        got_none = read_sync_recent(os.path.join(d, "N.md"))
    finally:
        for n in os.listdir(d):
            os.unlink(os.path.join(d, n))
        os.rmdir(d)
    ok_row = got_row == (45, "docs/CONSTANTS.md row SYNC_RECENT_S")
    ok_none = got_none == (_FALLBACK, _FALLBACK_SOURCE) and "fallback" in got_none[1]
    bad += (0 if ok_row else 1) + (0 if ok_none else 1)
    print("constant by name: a row SYNC_RECENT_S = 45 reads %r (%s); no row reads %r (%s)" % (got_row, "ok" if ok_row else "WRONG", got_none, "ok" if ok_none else "WRONG"))
    print("constant in use: SYNC_RECENT_S = %s from %s" % (SYNC_RECENT_S, SYNC_RECENT_SOURCE))
    print("| check | mutant | mutation | seen red |")
    print("|---|---|---|---|")
    for name, mut, words, _fn in CHECKS:
        res = with_fake(Model({mut}), lambda r: run_checks(r))
        red = [n for n, st, _d in res if st == "fail"]
        ok = name in red
        bad += 0 if ok else 1
        print("| %s | %s | %s | %s |" % (name, mut, words, ("red on %s" % name) if ok else "NOT RED (red: %s)" % red))
    print("selftest: %s" % ("every check seen red on its mutant" if not bad else "%d problem(s)" % bad))
    return 1 if bad else 0


def main():
    if "--selftest" in sys.argv:
        return selftest()
    try:
        rig = Rig(os.environ.get("WHOOSH_SOCKET_DIR") or os.environ.get("D") or "").connect()
    except (Unreadable, OSError) as e:
        print("could not drive: %s" % e, file=sys.stderr)
        res = [(n, "unjudged", str(e)) for n, *_ in CHECKS]
    else:
        res = run_checks(rig)
    line, f, u = summary(res)
    for n, st, d in res:
        if st != "pass":
            print("  %s %s: %s" % (st.upper(), n, d), file=sys.stderr)
    print(line)
    return 1 if (f or u) else 0


if __name__ == "__main__":
    sys.exit(main())
