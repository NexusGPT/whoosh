#!/usr/bin/env python3
"""RETIRED AS A MANIFEST CELL (2026-11-09, tests r4): this drives the Welcome address column that the add-account sheet replaced
(docs/lanes/addaccount-redesign.md); the cell is checks/parity/par-addaccount.py. Kept for its J4.1-J4.6 expectations, not run by the round.

welcomemac: the Welcome and Accounts screen of the Mac (J4.1-J4.7), driven through the control socket.

    python3 checks/welcomemac-check.py [--routes imap]     drive the running rig; exit 1 when F > 0 or U > 0
    python3 checks/welcomemac-check.py --selftest           run every check against a model of the design, then
                                                            against each MUTANT of it: each must go red on its check

Prints `N checks, F failed, U not judged`. `--routes imap` keeps the slice-1 routes (Google app password, iCloud,
unknown domain -> IMAP). The model in this file is the mutation rig only: expected values come from the design
fixture and fixtures/tests-j4.json, never from a built screen. No mail account is connected."""
import json
import os
import socket
import socketserver
import sys
import tempfile
import threading

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
import importlib.util as _ilu
_sw = _ilu.spec_from_file_location("shotwait", os.path.join(ROOT, "journeys", "lib", "shotwait.py")); shotwait = _ilu.module_from_spec(_sw); _sw.loader.exec_module(shotwait)   # how long a `shot` reply may take (one place)
from lib.fixture import FX, ACCOUNT, ACCOUNT_COUNT, FAILING_ACCOUNT, const  # noqa: E402

IDS = [a["id"] for a in FX["accounts"]]
EXTRA = json.load(open(os.path.join(ROOT, "fixtures", "tests-j4.json"), encoding="utf-8"))


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
            self.s.settimeout(shotwait.timeout_for(verb, args, 5))   # `par-welcomemac` sends `shot` through this client
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

    relaunch_hook = None      # the stand-in model's selftest sets it; a real run leaves it None and goes through scripts/launch-one.sh

    def relaunch(self):
        """Quit the app copy and start the SAME copy on the SAME run folder (scripts/launch-one.sh --relaunch, from inside the driven command),
        then connect again. Without the driver's run folder ($D) there is nothing to relaunch: the case is not judged, never passed."""
        if Rig.relaunch_hook is not None:
            return Rig.relaunch_hook(self)
        d = os.environ.get("D") or os.environ.get("WHOOSH_SOCKET_DIR") or ""
        script = os.path.join(ROOT, "scripts", "launch-one.sh")
        if not d or not os.path.exists(script):
            raise Unreadable("no relaunch available (run inside scripts/launch-one.sh, which sets $D)")
        import subprocess
        p = subprocess.run(["sh", script, "--relaunch"], env=dict(os.environ, D=d, WHOOSH_RIG_HIDDEN="1"), capture_output=True, text=True, timeout=240)
        if p.returncode != 0:
            raise Unreadable("launch-one --relaunch exit %d" % p.returncode)
        if self.s is not None:
            self.s.close()
        self.buf = b""
        import time
        t0 = time.time()
        while True:
            try:
                return self.connect()
            except (Unreadable, OSError):
                if time.time() - t0 > 60:
                    raise Unreadable("no socket 60 s after the relaunch")
                time.sleep(0.3)

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
            out.append((name, "fail", "reply shape: %s %s" % (type(e).__name__, e) + (" | evidence: %s" % welcome_state(rig))))
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
        if st != "pass":
            print("  %s %s: %s" % (st.upper(), n, d), file=sys.stderr)
    for n, st, _d in res:           # one line per passing case: a pass is read from its own line, never from the absence of a FAIL
        if st == "pass":
            print("  PASS %s" % n)
    print(line)
    return 1 if (f or u) else 0

W = "mac-welcome-accounts"
A1, A2, A3, A4 = (FX["accounts"][i] for i in range(4))
# The app detects a provider by the DOMAIN alone and only for the hosts it lists (WelcomeRoutes.detect); the sample accounts' `-example` domains are not listed, so the app
# answers IMAP for them (main79). The detection cases type an address on the provider's own public domain; the rest of the file keeps the sample addresses.
PUBLIC = {"Google": "gmail.com", "Microsoft": "outlook.com", "iCloud": "icloud.com", "Fastmail": "fastmail.com"}
DOM = {d: p for p, d in PUBLIC.items()}


def on_public(a):
    """An address on the public domain of the sample account's provider (no real person: the local part is generic)."""
    return "owner@" + PUBLIC[a["provider"]]


def welcome_state(r):
    """What decides a Welcome case that read nothing sensible: the surface, the account rows and the summary, each read on its own (a failed read says why)."""
    out = {}
    for label, fn in (("surface", lambda: r.surface()), ("account_rows", lambda: len(acc_rows(r))),
                      ("first_rows", lambda: [(x.get("id"), x.get("name"), x.get("progress")) for x in acc_rows(r)[:2]]),
                      ("summary", lambda: r.rows(W + ".summary")), ("core_order_len", lambda: len(r.core("account/order") or []))):
        try:
            out[label] = fn()
        except Unreadable as e:
            out[label] = "unreadable(%s)" % str(e)[:90]
        except (KeyError, IndexError, TypeError) as e:
            out[label] = "reply shape: %s %s" % (type(e).__name__, e)
    return json.dumps(out, sort_keys=True, default=str)
import re  # noqa: E402
VALID = re.compile(r"^[^@\s]+@[^@\s]+\.[^@\s]+$")


def go_welcome(r):
    r.act("fixture", "reset")
    r.act("click", "dock.whoosh")
    r.act("click", W + ".reset")      # the Welcome list survives `fixture reset` (rows persist): a row an earlier case added made the next one read 1 (main81: garbage addresses, wrong password)


def add(r, address, via=W + ".continue"):
    r.act("type", address)
    r.act("click", via)


def acc_rows(r):
    return r.rows(W + ".accounts")


@check("provider-google", "g-none", "a Google address is detected as nothing")
def _(r):
    go_welcome(r); r.act("type", on_public(A1))
    return r.rows(W + ".provider")[0]["name"], A1["provider"]


@check("provider-microsoft", "ms-as-google", "a Microsoft address is detected as Google")
def _(r):
    go_welcome(r); r.act("type", on_public(A2))
    return r.rows(W + ".provider")[0]["name"], A2["provider"]


@check("provider-icloud", "icloud-as-imap", "an iCloud address is detected as plain IMAP")
def _(r):
    go_welcome(r); r.act("type", on_public(A3))
    return r.rows(W + ".provider")[0]["name"], A3["provider"]


@check("provider-fastmail", "fastmail-as-imap", "a Fastmail address is detected as plain IMAP")
def _(r):
    go_welcome(r); r.act("type", on_public(A4))
    return r.rows(W + ".provider")[0]["name"], A4["provider"]


@check("unknown-domain-is-imap", "unknown-as-google", "an unknown domain is guessed to be Google")
def _(r):
    go_welcome(r); r.act("type", EXTRA["unknown_domain_address"])
    return r.rows(W + ".provider")[0]["name"], EXTRA["unknown_domain_provider"]


@check("unknown-domain-server-line", "unknown-no-server", "an unknown domain shows no server settings line")
def _(r):
    go_welcome(r); r.act("type", EXTRA["unknown_domain_address"])
    return len(r.rows(W + ".server")) > 0, True


@check("app-password-steps-link", "steps-no-link", "the app-password steps carry no link")
def _(r):
    go_welcome(r); r.act("type", on_public(A1))      # the Google route carries the link; the IMAP route the sample domain gets has none
    return r.rows(W + ".steps")[0]["link"].startswith(EXTRA["app_password_link_prefix"]), True


@check("valid-address-accepted", "continue-dead", "Continue does nothing")
def _(r):
    go_welcome(r); add(r, A1["address"])
    return len(acc_rows(r)), 1


@check("garbage-addresses-refused", "garbage-accepted", "Continue accepts a malformed address")
def _(r):
    cases = EXTRA["garbage_addresses"] + ["x" * EXTRA["long_local_part_chars"] + "@example.test"]
    got, seen = [], {}
    for g in cases:
        go_welcome(r); add(r, g)
        got.append(len(acc_rows(r)))
        seen[g[:20]] = [(x.get("id"), x.get("address"), x.get("progress"), x.get("status")) for x in acc_rows(r)][:2]

    def evidence():      # what the 1 is: an account row (its id, address, progress) or something else, per input, and where the app is
        return json.dumps({"rows_per_input": seen, "state_now": json.loads(welcome_state(r))}, default=str)
    return got, [0] * len(cases), evidence


@check("duplicate-address-once", "dup-added", "the same address added twice makes two rows")
def _(r):
    go_welcome(r); add(r, A1["address"])
    r.act("click", W + ".add-another"); add(r, A1["address"])
    return len(acc_rows(r)), 1


@check("wrong-password-says-so", "wrong-pw-silent", "a wrong app password gives no line saying so")
def _(r):
    go_welcome(r); add(r, on_public(A1)); r.act("fixture", "paste", "wrong-password")      # a Google address: the route that offers the app-password link
    row = acc_rows(r)[0]
    return (EXTRA["wrong_password_words"] in row["status"], row["status_link"].startswith("https://")), (True, True), \
        lambda: json.dumps({"status": row.get("status"), "status_link": row.get("status_link"), "provider": row.get("provider"), "address": row.get("address")})


@check("credential-in-keychain", "pw-not-stored", "the pasted password is not stored")
def _(r):
    go_welcome(r); add(r, A1["address"]); r.act("fixture", "paste", "app-password")
    return r.core("account_credential/%s/stored" % A1["id"]), True


@check("account-exists-control", "no-account", "the account is never created")
def _(r):
    go_welcome(r); add(r, A1["address"]); r.act("fixture", "paste", "app-password")
    return r.core("account/%s/address" % A1["id"]), A1["address"]


@check("no-password-in-state", "pw-in-state", "the password is written next to the account")
def _(r):
    go_welcome(r); add(r, A1["address"]); r.act("fixture", "paste", "app-password")
    served = r.core("account/%s/address" % A1["id"])      # control: the account's own path IS served, so a path that is not served says "no password here"
    try:
        value = r.core("account/%s/password" % A1["id"])
    except Unreadable as e:
        if "no core value" not in str(e):      # only the core's "no value at this path" answer counts as absent; any other refusal stays not judged
            raise
        value = None
    return (served == A1["address"], value), (True, None)


@check("thirty-names-colours-order", "order-lost", "the drag order is not saved")
def _(r):
    go_welcome(r); r.act("fixture", "paste-accounts", "mac-main-sample")
    return ([(x["name"], x["colour"]) for x in acc_rows(r)], r.core("account/order")), \
           ([(a["name"], a["colour"]) for a in FX["accounts"]], IDS), lambda: welcome_state(r)


@check("failing-row-says-why", "fail-no-reason", "the failing row shows no reason")
def _(r):
    go_welcome(r); r.act("fixture", "paste-accounts", "mac-main-sample")
    rows = [x for x in acc_rows(r) if x["id"] == FAILING_ACCOUNT]
    if not rows:      # no such row: say what the list holds instead of an index error
        return None, ACCOUNT[FAILING_ACCOUNT]["status"], lambda: "no row with id %s among %s | %s" % (FAILING_ACCOUNT, [x.get("id") for x in acc_rows(r)], welcome_state(r))
    return rows[0]["status"], ACCOUNT[FAILING_ACCOUNT]["status"]


@check("other-29-carry-on", "fail-stops-all", "one failing account stops the other 29")
def _(r):
    go_welcome(r); r.act("fixture", "paste-accounts", "mac-main-sample")
    return sum(1 for i in IDS if i != FAILING_ACCOUNT and r.core("sync_state/%s/healthy" % i) is True), ACCOUNT_COUNT - 1


@check("shortcut-preset-stored", "no-preset", "no shortcut preset is stored")
def _(r):
    go_welcome(r); r.act("fixture", "paste-accounts", "mac-main-sample")
    return r.core("shortcut_preset/name") in ("Superhuman", "GotEmail"), True


@check("first-screen-before-history", "waits-for-history", "the first screen waits for the whole history")
def _(r):
    go_welcome(r); r.act("fixture", "paste-accounts", "mac-main-sample")
    return r.core("sync_state/%s/first_screen_at" % A1["id"]) is not None, True


@check("open-inbox-arrives", "open-dead", "Open inbox does nothing")
def _(r):
    go_welcome(r); r.act("fixture", "paste-accounts", "mac-main-sample")
    before = r.surface()                                  # control: the press must be what leaves the Welcome screen
    at = {"before_press": before, "state_before_press": welcome_state(r)}
    r.act("click", W + ".open-inbox")
    return (before, r.surface()), (W, "mac-main"), lambda: json.dumps(dict(at, after_press=r.surface()), default=str)


class Model:
    """Stands in for the app so each check can be seen red on its mutant; it serves the design behaviour of J4."""

    def __init__(self, mut):
        self.m = mut
        self.reset()

    def reset(self):
        self.typed, self.accts, self.surf, self.last = "", [], "mac-main", None

    def prov(self, a):
        p = DOM.get(a.split("@")[-1], "IMAP")
        if self.m & {"g-none"} and p == "Google": return ""
        if "ms-as-google" in self.m and p == "Microsoft": return "Google"
        if "icloud-as-imap" in self.m and p == "iCloud": return "IMAP"
        if "fastmail-as-imap" in self.m and p == "Fastmail": return "IMAP"
        if "unknown-as-google" in self.m and p == "IMAP": return "Google"
        return p

    def new(self, addr, i=None):
        a = {"id": "acc-%02d" % (len(self.accts) + 1), "address": addr, "name": addr, "provider": self.prov(addr),
             "colour": "#2F6FED", "progress": "connecting", "status": "", "status_link": ""}
        self.accts.append(a)

    def handle(self, verb, args):
        ok = {"ok": True}
        if verb == "fixture":
            if args[0] == "reset": self.reset()
            elif args[0] == "paste" and self.accts:
                if args[1] == "wrong-password" and "wrong-pw-silent" not in self.m:
                    self.accts[-1].update(status="wrong password: check the app password", status_link="https://myaccount.example.test/apppasswords", progress="")
                elif args[1] != "wrong-password":
                    self.last = "stored"
            elif args[0] == "paste-accounts":
                self.accts = []
                for a in FX["accounts"]:
                    x = dict(a); x.update(provider=a["provider"], progress="fetching the inbox", status=a.get("status", ""))
                    if "colour" in a: x["colour"] = a["colour"] if "colour-reset" not in self.m else "#2F6FED"
                    if "fail-no-reason" in self.m: x["status"] = ""
                    self.accts.append(x)
                self.surf = W
            return ok
        if verb == "click":
            ax = args[0]
            if ax == "dock.whoosh": self.surf = W; return ok
            if ax == W + ".reset": self.accts, self.typed = [], ""; return ok
            if ax == W + ".continue" or ax == W + ".add-another":
                if ax.endswith("continue") and "continue-dead" not in self.m:
                    if (VALID.match(self.typed) and len(self.typed) <= 254) or "garbage-accepted" in self.m:
                        if not any(a["address"] == self.typed for a in self.accts) or "dup-added" in self.m:
                            self.new(self.typed)
                    self.typed = ""
                elif ax.endswith("add-another"):
                    self.typed = ""
                return ok
            if ax == W + ".open-inbox":
                if "open-dead" not in self.m: self.surf = "mac-main"
                return ok
            return {"ok": False, "error": "nothing at %s" % ax}
        if verb == "type":
            self.typed += args[0]; return ok
        if verb == "uistate":
            return {"ok": True, "state": {"surface": self.surf}}
        if verb == "rows":
            ax = args[0]
            if ax == W + ".provider":
                return {"ok": True, "rows": [{"name": self.prov(self.typed)}] if self.typed else []}
            if ax == W + ".steps":
                p = self.prov(self.typed)
                link = "https://myaccount.example.test/apppasswords" if "steps-no-link" not in self.m else ""
                return {"ok": True, "rows": [{"text": "Make an app password", "link": link}] if p in ("Google", "iCloud") else []}
            if ax == W + ".server":
                return {"ok": True, "rows": [] if "unknown-no-server" in self.m else [{"host": "imap." + self.typed.split("@")[-1]}]}
            if ax == W + ".accounts":
                return {"ok": True, "rows": self.accts}
            return {"ok": False, "error": "no rows at %s" % ax}
        if verb == "core":
            p = args[1].replace(".", "/").split("/")
            if p[0] == "account_credential": return {"ok": True, "value": (self.last == "stored" and "pw-not-stored" not in self.m) or None}
            if p[0] == "account" and p[-1] == "address":
                return {"ok": True, "value": None if "no-account" in self.m else (self.accts[0]["address"] if self.accts else None)}
            if p[0] == "account" and p[-1] == "password":
                if "pw-in-state" in self.m: return {"ok": True, "value": "secret"}
                return {"ok": False, "error": "no core value at state/account/%s/password" % p[1]}      # the core serves no such path (main79)
            if p[0] == "account" and p[-1] == "order":
                ids = [a["id"] for a in self.accts]
                return {"ok": True, "value": (ids[::-1] if "order-lost" in self.m else ids)}
            if p[0] == "sync_state":
                if p[-1] == "healthy":
                    bad = p[1] == FAILING_ACCOUNT or ("fail-stops-all" in self.m)
                    return {"ok": True, "value": not bad}
                if p[-1] == "first_screen_at": return {"ok": True, "value": None if "waits-for-history" in self.m else 1}
            if p[0] == "shortcut_preset": return {"ok": True, "value": None if "no-preset" in self.m else "Superhuman"}
            return {"ok": True, "value": None}
        return {"ok": False, "error": "unknown verb %s" % verb}


def only(argv):
    if "--routes" in argv and argv[argv.index("--routes") + 1] == "imap":
        skip = {"provider-microsoft", "provider-fastmail"}
        return lambda n: n not in skip
    return None


if __name__ == "__main__":
    sys.exit(main(Model, only))
