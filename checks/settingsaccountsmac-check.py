#!/usr/bin/env python3
"""settingsaccountsmac: Settings: Accounts on the Mac (J4.8, J4.9, J4.12 and the 31st account), through the control socket.

    python3 checks/settingsaccountsmac-check.py             drive the running rig; exit 1 when F > 0 or U > 0
    python3 checks/settingsaccountsmac-check.py --selftest  run every check against a model of the design, then
                                                            against each MUTANT of it: each must go red on its check

Prints `N checks, F failed, U not judged`. The model is the mutation rig only; expected values come from the design
fixture, docs/CONSTANTS.md and fixtures/tests-j4.json, never from a built screen."""
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
from lib.fixture import FX, ACCOUNT, ACCOUNT_COUNT, FAILING_ACCOUNT, const  # noqa: E402

IDS = [a["id"] for a in FX["accounts"]]
EXTRA = json.load(open(os.path.join(ROOT, "fixtures", "tests-j4.json"), encoding="utf-8"))


CATALOGUE = json.load(open(os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8"))["strings"]
CODES = sorted({c for v in CATALOGUE.values() for c in v.get("localizations", {})})
GEN_SETTINGS, LANG_DOOR = "mac-settings-general.settings", "mac-settings-general.appearance.language.%s"
PINNED = {"was": None}      # the stored language choice this run replaced through the General pane's own door (None: nothing replaced)


def word(key, code, *fill):
    """The catalogue's sentence for `key` in `code` (the key itself when the language has none, as the app answers); `%@` is filled in order."""
    s = CATALOGUE[key]["localizations"].get(code, {}).get("stringUnit", {}).get("value", key)
    for f in fill:
        s = s.replace("%@", str(f), 1)
    return s


class Unreadable(Exception):
    """A read or action failed: the check that needed it is not judged."""


class Rig:
    """Control-socket client (docs/CONTROL-SOCKET.md). One JSON line out, one back."""

    def __init__(self, sock_dir):
        self.path = os.path.join(sock_dir, "whoosh.sock") if sock_dir else ""
        self.s, self.buf = None, b""

    def connect(self):
        if not self.path or not os.path.exists(self.path):
            raise Unreadable("no socket at %r (set WHOOSH_SOCKET_DIR, run under scripts/launch-one.sh)" % self.path)
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

    def act(self, verb, *args):
        """An action; a dead one is not a check, the read after it shows the effect."""
        try:
            self.send(verb, *args)
        except Unreadable:
            pass

    def rows(self, ax):
        return self.send("rows", ax).get("rows", [])

    def core(self, path):
        return self.send("core", "state", path).get("value")

    def surface(self):
        return self.send("uistate").get("state", {}).get("surface")

    def speak(self, key, *fill):
        """`key` in the language the app speaks now (main79: the rig spoke French and these cases compared with English). A stored catalogue code is used as it is;
        `system` hides the code the Mac resolves it to, so the check chooses English through the General pane's own language door and remembers what it replaced
        (`put_back` restores it at the end of the run)."""
        code = (self.rows(GEN_SETTINGS) or [{}])[0].get("language")
        if code not in CODES:
            self.send("click", "mac-settings-window.pane-row.general")
            self.send("click", LANG_DOOR % "en")
            PINNED["was"] = PINNED["was"] or code or "system"
            code = "en"
        return word(key, code, *fill)

    def put_back(self):
        was, PINNED["was"] = PINNED["was"], None
        if was is not None:
            self.send("click", "mac-settings-window.pane-row.general")
            self.send("click", LANG_DOOR % was)
        return was


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
            got, want, *ev = fn(rig)
            out.append((name, "pass" if got == want else "fail", "expected %r, got %r" % (want, got) + ("" if got == want or not ev else " | evidence: %s" % ev[0]())))
        except Unreadable as e:
            out.append((name, "unjudged", str(e)))
        except (KeyError, IndexError, TypeError) as e:
            out.append((name, "fail", "reply shape: %s %s" % (type(e).__name__, e)))
    try:
        PINNED["put_back"] = rig.put_back()
    except Unreadable as e:
        out.append(("language-put-back", "unjudged", "could not put the stored language choice %r back: %s" % (PINNED["was"], e)))
    return out


def summary(res):
    f = sum(1 for r in res if r[1] == "fail")
    u = sum(1 for r in res if r[1] == "unjudged")
    return "%d checks, %d failed, %d not judged" % (len(res), f, u), f, u


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
    d = tempfile.mkdtemp(prefix="j4fake")
    srv = _S(os.path.join(d, "whoosh.sock"), _H)
    srv.model = model
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    r = None
    try:
        r = Rig(d).connect()
        return fn(r)
    finally:
        if r is not None and r.s is not None:
            r.s.close()      # on every path: an open connection keeps its handler thread, and server_close() joins it for ever
        srv.shutdown()
        srv.server_close()


def selftest(make_model, only=None):
    """Base model: every check passes. Each mutant: its own check goes red. Exit 0 only then."""
    bad = 0
    base = with_fake(make_model(set()), lambda r: run_checks(r, only))
    line, f, u = summary(base)
    print("base model: " + line)
    if f or u:
        bad += 1
        for n, st, d in base:
            if st != "pass":
                print("  BASE %s %s: %s" % (st.upper(), n, d))
    # the stored choice is "system" (the stand-in speaks French): the check chooses English through the door, passes, and puts "system" back
    m = make_model({"stored-system"})
    sysres = with_fake(m, lambda r: run_checks(r, only))
    sline, sf, su = summary(sysres)
    ok_sys = sf == 0 and su == 0 and m.stored == "system"
    print("stored language 'system': %s; stored after the run: %r: %s" % (sline, m.stored, "OK" if ok_sys else "WRONG"))
    bad += 0 if ok_sys else 1
    for n, st, d in sysres:
        if st != "pass":
            print("  SYSTEM-RUN %s %s: %s" % (st.upper(), n, d))
    print("| check | mutant | mutation | seen red |")
    print("|---|---|---|---|")
    for name, mut, words, _fn in CHECKS:
        if only and not only(name):
            continue
        res = with_fake(make_model({mut}), lambda r: run_checks(r, only))
        red = [n for n, st, _d in res if st == "fail"]
        ok = name in red
        bad += 0 if ok else 1
        print("| %s | %s | %s | %s |" % (name, mut, words, ("red on %s" % name) if ok else "NOT RED (red: %s)" % red))
    print("selftest: %s" % ("every check seen red on its mutant" if not bad else "%d problem(s)" % bad))
    return 1 if bad else 0


def main(make_model, only_from_argv=None):
    only = only_from_argv(sys.argv[1:]) if only_from_argv else None
    if "--selftest" in sys.argv:
        return selftest(make_model, only)
    try:
        rig = Rig(os.environ.get("WHOOSH_SOCKET_DIR") or os.environ.get("D") or "").connect()
    except (Unreadable, OSError) as e:
        print("could not drive: %s" % e, file=sys.stderr)
        res = [(n, "unjudged", str(e)) for n, *_ in CHECKS if not only or only(n)]
    else:
        res = run_checks(rig, only)
    line, f, u = summary(res)
    for n, st, d in res:
        if st == "pass":
            print("  PASS %s: %s" % (n, d))
        else:
            print("  %s %s: %s" % (st.upper(), n, d), file=sys.stderr)
    print(line)
    return 1 if (f or u) else 0

S, W = "mac-settings-accounts", "mac-welcome-accounts"
MUTED = (IDS[1], IDS[2])
MAXB = const("ACCOUNT_ICON_MAX_BYTES")
QR = const("QR_EXPIRY_S")
T31 = EXTRA["thirty_first"]
# a glyph other than the one the design fixture already gives acc-01 (its own icon is the fallback the core answers, so picking it again proves no save)
PICK_ICON = next(a["icon_id"] for a in FX["accounts"] if a.get("icon_id") and a["icon_id"] != FX["accounts"][0].get("icon_id"))
T31_ID = "acc-%02d" % (ACCOUNT_COUNT + 1)      # the id the fixture core gives an added account (FixtureCore: acc-NN, NN = accounts held + 1)
FORBID = EXTRA["qr_payload_forbidden_keys"]


def open_s(r):
    r.act("fixture", "reset")
    r.act("click", "mac-main.status")


@check("broken-account-named", "failing-unnamed", "the broken account is not named")
def _(r):
    open_s(r)
    return r.rows(S + ".failing")[0]["name"], ACCOUNT[FAILING_ACCOUNT]["name"]


@check("sign-in-again-heals", "signin-dead", "sign in again does not clear the error")
def _(r):
    open_s(r)
    before = r.core("sync_state/%s/healthy" % FAILING_ACCOUNT)      # control: it starts broken
    r.act("click", S + ".%s.sign-in-again" % FAILING_ACCOUNT)
    return (before, r.core("sync_state/%s/healthy" % FAILING_ACCOUNT)), (False, True)


@check("two-muted-exactly", "mute-all", "muting one account mutes every account")
def _(r):
    open_s(r)
    for i in MUTED: r.act("click", S + ".%s.notifications" % i)
    return sorted(r.core("notification_setting/off")), sorted(MUTED)


@check("muted-row-says-so", "mute-silent", "muted rows look the same")
def _(r):
    open_s(r)
    want = r.speak("no notifications")      # the catalogue's sentence in the app's language, read (and the language pinned if need be) BEFORE the row is drawn
    for i in MUTED: r.act("click", S + ".%s.notifications" % i)
    unmuted = IDS[3]                                             # control: an unmuted row says nothing
    return (r.rows(S + "." + MUTED[0])[0]["note"], r.rows(S + "." + unmuted)[0]["note"]), (want, "")


@check("icon-library-saved", "icon-lost", "the picked library icon is not saved")
def _(r):
    open_s(r)
    r.act("click", S + ".%s.icon" % IDS[0]); r.act("click", S + ".icon-library.%s" % PICK_ICON)
    return r.core("account_icon/%s" % IDS[0]), PICK_ICON


@check("icon-colour-saved", "colour-lost", "the picked colour is not saved")
def _(r):
    open_s(r)
    r.act("click", S + ".%s.icon" % IDS[0]); r.act("click", S + ".colour.%s" % T31["colour"].lstrip("#"))
    return r.core("account/%s/colour" % IDS[0]), T31["colour"]


@check("icon-upload-at-limit-accepted", "limit-refused", "an image of exactly the limit is refused")
def _(r):
    open_s(r)
    r.act("click", S + ".%s.icon" % IDS[0]); r.act("fixture", "upload-icon", MAXB)
    return r.core("account_icon/%s" % IDS[0]), "uploaded"


@check("icon-upload-over-limit-refused", "over-accepted", "an image one byte over the limit is accepted")
def _(r):
    open_s(r)
    # the refusal's sentence is the catalogue's, in the app's language, with the limit in MB filled in (AccountsWords.tooBig); read before the upload so it is drawn in that language
    want = r.speak("That image is too big. The limit is %@ MB.", MAXB // 1048576)
    r.act("click", S + ".%s.icon" % IDS[0])
    before = r.core("account_icon/%s" % IDS[0])      # control: the account's own icon (the design fixture's, never "no icon"); a refused upload leaves it as it was
    r.act("fixture", "upload-icon", MAXB + 1)
    return (r.core("account_icon/%s" % IDS[0]), r.rows(S + "." + IDS[0])[0]["note"]), (before, want)


@check("thirty-first-added-last", "t31-reorders", "adding a 31st account reorders the first 30")
def _(r):
    open_s(r)
    r.act("fixture", "add-account", T31["address"], T31["name"], T31["colour"])
    names = [x["name"] for x in r.rows(S + ".accounts")]

    def evidence():
        rs = r.rows(S + ".accounts")
        ids = [x.get("id") for x in rs]
        reads = {"rows": len(rs), "last_ids": ids[-3:], "acc-31_in_rows": T31_ID in ids, "row_keys": sorted(rs[-1].keys()) if rs else None}
        for label, path in (("core_acc31_colour", "account/%s/colour" % T31_ID), ("core_acc31_credential", "account_credential/%s/stored" % T31_ID),
                            ("core_acc31_healthy", "sync_state/%s/healthy" % T31_ID)):
            try:
                reads[label] = r.core(path)
            except Unreadable as e:
                reads[label] = "unreadable(%s)" % str(e)[:80]
        try:
            reads["find_text"] = (r.rows(S + ".field") or [{}])[0].get("find_text")
        except Unreadable as e:
            reads["find_text"] = "unreadable(%s)" % str(e)[:80]
        return json.dumps(reads, sort_keys=True)
    return names, [a["name"] for a in FX["accounts"]] + [T31["name"]], evidence


@check("thirty-first-keeps-credentials", "t31-drops", "adding a 31st account drops an existing credential")
def _(r):
    open_s(r)
    r.act("fixture", "add-account", T31["address"], T31["name"], T31["colour"])
    return sum(1 for i in IDS if r.core("account_credential/%s/stored" % i) is True), ACCOUNT_COUNT


@check("move-to-iphone-shows-qr", "qr-missing", "the move control shows no QR")
def _(r):
    open_s(r); r.act("click", S + ".move-to-iphone")
    # the row is served only while the QR is open and carries the payload it draws (WelcomeMacFeature.qr: ax, payload, expires_in_s, issued): no `visible` key exists
    qr = r.rows(W + ".qr")
    return (r.surface(), bool(qr and qr[0].get("payload"))), (W, True)


@check("qr-expiry-from-constants", "qr-never", "the QR never expires")
def _(r):
    open_s(r); r.act("click", S + ".move-to-iphone")
    return r.core("device_link/expires_in_s"), QR


@check("qr-carries-token-only", "qr-leaks", "the QR payload carries a password")
def _(r):
    open_s(r); r.act("click", S + ".move-to-iphone")
    keys = r.core("device_link/payload_keys")
    return (sorted(keys), [k for k in keys if k in FORBID]), (["token"], [])


@check("qr-row-payload-is-the-token", "qr-payload-leaks", "the QR the owner scans carries an address, not only a token")
def _(r):
    open_s(r); r.act("click", S + ".move-to-iphone")
    # the text the QR draws (row W.qr, field payload: WelcomeMacFeature.swift:104) is `whoosh-pair:v1:` and a hex token: no address, no password, no link
    qr = r.rows(W + ".qr")
    payload = qr[0].get("payload") if qr else None
    if not isinstance(payload, str):
        raise Unreadable("the QR row serves no payload text (rows %s.qr: %r)" % (W, qr))
    return (bool(re.fullmatch(r"whoosh-pair:v1:[0-9a-f]+", payload)), "@" in payload), (True, False), lambda: "payload %r" % payload


@check("expired-qr-replaced", "qr-stays", "an expired QR stays on screen")
def _(r):
    open_s(r); r.act("click", S + ".move-to-iphone")
    first = r.core("device_link/id")                              # control: a link exists to expire
    r.act("clock", "advance", QR + 1)
    second = r.core("device_link/id")
    return (first is not None, second not in (None, first)), (True, True)


class Model:
    def __init__(self, mut):
        self.m = mut
        if "stored-system" in mut:
            self.stored = "system"
        self.reset()

    stored = "fr"      # the stored language choice of the stand-in: "fr", or "system" (which the stand-in speaks as French, like the main79 rig)

    def spoken(self):
        return "fr" if self.stored == "system" else self.stored

    def reset(self):
        self.healthy = {i: i != FAILING_ACCOUNT for i in IDS}
        self.off, self.icon, self.colour = [], {}, {}
        self.accts = [dict(a) for a in FX["accounts"]]
        self.surf, self.link, self.clock, self.notes = "mac-main", None, 0, {}

    def mklink(self): self.link = {"id": "link-%d" % (self.clock + 1), "at": self.clock}

    def handle(self, verb, args):
        ok = {"ok": True}
        if verb == "fixture":
            a = args[0]
            if a == "reset": self.reset()
            elif a == "upload-icon":
                n = int(args[1])
                if n > MAXB and "over-accepted" not in self.m:
                    self.notes[IDS[0]] = word("That image is too big. The limit is %@ MB.", self.spoken(), MAXB // 1048576)
                elif n == MAXB and "limit-refused" in self.m:
                    self.notes[IDS[0]] = word("That image is too big. The limit is %@ MB.", self.spoken(), MAXB // 1048576)
                else:
                    self.icon[IDS[0]] = "uploaded"
            elif a == "add-account":
                if "t31-reorders" in self.m: self.accts.reverse()
                self.accts.append({"id": "acc-31", "name": args[2], "colour": args[3]})
                if "t31-drops" in self.m: self.healthy["acc-31"] = True; self.dropped = True
            return ok
        if verb == "click":
            ax = args[0]
            if ax.startswith(LANG_DOOR % ""): self.stored = ax.rsplit(".", 1)[1]; return ok
            if ax.startswith("mac-settings-window.pane-row."): return ok
            if ax == "mac-main.status": self.surf = S; return ok
            if ax == S + ".move-to-iphone":
                self.surf = W
                if "qr-missing" not in self.m: self.mklink()
                return ok
            if ax.endswith(".sign-in-again"):
                if "signin-dead" not in self.m: self.healthy[FAILING_ACCOUNT] = True
                return ok
            if ax.endswith(".notifications"):
                i = ax.split(".")[1]
                self.off = list(IDS) if "mute-all" in self.m else self.off + [i]
                return ok
            if ax.startswith(S + ".icon-library."):
                if "icon-lost" not in self.m: self.icon[IDS[0]] = ax.rsplit(".", 1)[1]
                return ok
            if ax.startswith(S + ".colour."):
                if "colour-lost" not in self.m: self.colour[IDS[0]] = "#" + ax.rsplit(".", 1)[1]
                return ok
            if ax.endswith(".icon"): return ok
            return {"ok": False, "error": "nothing at %s" % ax}
        if verb == "clock":
            self.clock += int(args[1])
            if self.link and self.clock - self.link["at"] >= QR and "qr-stays" not in self.m: self.mklink()
            return ok
        if verb == "uistate": return {"ok": True, "state": {"surface": self.surf}}
        if verb == "rows":
            ax = args[0]
            if ax == S + ".failing":
                return {"ok": True, "rows": [{"name": "" if "failing-unnamed" in self.m else ACCOUNT[FAILING_ACCOUNT]["name"]}]}
            if ax == S + ".accounts": return {"ok": True, "rows": self.accts}
            if ax == GEN_SETTINGS: return {"ok": True, "rows": [{"language": self.stored}]}
            if ax == W + ".qr": return {"ok": True, "rows": [] if self.link is None else [{"ax": W + ".qr", "payload": "whoosh-pair:v1:" + self.link["id"].encode().hex() + (":owner@example.test" if "qr-payload-leaks" in self.m else ""), "expires_in_s": QR, "issued": 0}]}
            if ax.startswith(S + "."):
                i = ax.split(".", 1)[1]
                note = self.notes.get(i, "")
                if i in self.off and "mute-silent" not in self.m: note = word("no notifications", self.spoken())
                return {"ok": True, "rows": [{"note": note}]}
            return {"ok": False, "error": "no rows at %s" % ax}
        if verb == "core":
            p = args[1].replace(".", "/").split("/")
            if p[0] == "sync_state" and p[-1] == "healthy": return {"ok": True, "value": self.healthy.get(p[1])}
            if p[0] == "notification_setting": return {"ok": True, "value": sorted(set(self.off))}
            if p[0] == "account_icon": return {"ok": True, "value": self.icon.get(p[1], "briefcase")}      # the design fixture gives acc-01 the briefcase
            if p[0] == "account" and p[-1] == "colour": return {"ok": True, "value": self.colour.get(p[1])}
            if p[0] == "account_credential":
                gone = "t31-drops" in self.m and getattr(self, "dropped", False) and p[1] == IDS[0]
                return {"ok": True, "value": None if gone else True}
            if p[0] == "device_link":
                if self.link is None: return {"ok": True, "value": None}
                if p[1] == "expires_in_s": return {"ok": True, "value": 99999 if "qr-never" in self.m else QR}
                if p[1] == "payload_keys": return {"ok": True, "value": ["token", "password"] if "qr-leaks" in self.m else ["token"]}
                if p[1] == "id": return {"ok": True, "value": self.link["id"]}
            return {"ok": True, "value": None}
        return {"ok": False, "error": "unknown verb %s" % verb}


if __name__ == "__main__":
    sys.exit(main(Model))
