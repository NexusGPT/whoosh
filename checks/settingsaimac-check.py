#!/usr/bin/env python3
"""settingsaimac-check: J5.1-J5.3, AI providers, speech engine, agent cards (docs/JOURNEYS.md J5).

    python3 checks/settingsaimac-check.py              drive the running rig copy ($WHOOSH_SOCKET_DIR); exit 1 when F>0 or U>0
    python3 checks/settingsaimac-check.py --selftest   run every check against a stand-in app (clean, then one mutant per check)

This file is also the lane's shared harness: mcpnotesmac-check.py and decision-check.py load it (askmac-check.py retired in round 2 with the Ask panel).
It holds (1) the check runner, (2) two 127.0.0.1 stub providers, (3) FakeApp, a stand-in for the app on the
control-socket wire, used ONLY by --selftest to prove each check can go red on its mutant. Expected values are
literals taken from docs/JOURNEYS.md, docs/DECISION-MODEL.md v2 and docs/CONSTANTS.md, never from FakeApp.
Nothing here touches a real key or host: every provider is a stub on 127.0.0.1.
"""
import copy
import hashlib
import http.server
import importlib.util
import json
import os
import shutil
import socketserver
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request

LAYA_FILES = ["typed-decisions/rl_agent_config.json", "typed-decisions/encoder/config.json", "typed-decisions/tokenizer/tokenizer.json",
              "typed-decisions/tokenizer/tokenizer_config.json", "typed-decisions/model.safetensors"]
LAYA_SIZE = 150000
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable  # noqa: E402
from lib.fixture import const, MSG, ACCOUNT  # noqa: E402

FX = json.load(open(os.path.join(ROOT, "fixtures", "tests-j5.json"), encoding="utf-8"))
A = FX["ax"]
# hid-ai round 6: the list's row doors count from 0 (InboxList/ListClicks.swift `mac-main.list.row.\(i)`), and the message the prefill names
# (decision.prefill.msg, msg-01) is the FIRST row; fixtures/tests-j5.json's "row.1" opened the second row (main92: msg-01a, assem@northlight-example.de).
A["list_row_1_create_rule"] = "mac-main.list.row.0.create-rule"
D = FX["decision"]
CAPDIR = os.path.join(ROOT, "_captures", "tests-j5")
NETPROBE = {"fn": None}      # None: `lsof` on the rig copy's pid (the run folder's rig.pid); a self-test sets the stand-in's (FakeServer.__init__): the text `lsof -a -i -n -P -p <pid>` would print
RELAUNCH = {"fn": None}      # None: the real `sh scripts/launch-one.sh --relaunch`; a self-test sets the stand-in's (FakeServer.__init__), so no test launches anything


class Refused(Exception):
    """An action the check performed was refused by the app: the check is red, not unjudged."""


# ---------------------------------------------------------------- runner
class Excluded(Exception):
    """A case that can never be judged on this run's engine, by design (lead's ruling, round 16): its words start with the engine that does judge it
    ("real engine only: <why>"). It prints `ok <case> excluded, <words>` and counts as neither failed nor not judged; that engine's run judges it."""


class Suite:
    def __init__(self, name):
        self.name, self.items = name, []

    def check(self, label, mutant, desc):
        def deco(fn):
            self.items.append((label, fn, mutant, desc))
            return fn
        return deco


def act(ctl, verb, *args):
    try:
        return ctl.send(verb, *args)
    except Unreadable as e:
        raise Refused("action refused: %s" % e)


def soft(ctl, verb, *args):
    """A setup call whose refusal is not the check's business (the observable decides)."""
    try:
        return ctl.send(verb, *args)
    except Unreadable:
        return None


def reset(ctl, env):
    soft(ctl, "fixture", "reset")
    for name, stub in (("anthropic", env["prov"]), ("jev", env["jev"]), ("laya", env["laya"])):
        stub.requests.clear()
        stub.answer = None
        stub.delay = 0
        stub.piece_delay = 0
        stub.notfound = set()
        stub.gets.clear()
        env.setdefault("endpoints", {})[name] = soft_why(ctl, "fixture", "provider-endpoint", name, stub.url)


def soft_why(ctl, verb, *args):
    """`soft`, and the answer: "ok", or the socket's own words when the call was refused (the staging of a stub is read back through this)."""
    try:
        ctl.send(verb, *args)
        return "ok"
    except Unreadable as e:
        return "refused: %s" % e


def run_suite(suite, ctl, env):
    out = []
    for label, fn, _m, _d in suite.items:
        try:
            ok, detail = fn(ctl, env)
            out.append((label, "pass" if ok else "fail", detail if ok else detail + evidence(label, ctl, env)))
        except Refused as e:
            out.append((label, "fail", str(e) + evidence(label, ctl, env)))
        except Excluded as e:
            out.append((label, "excluded", str(e)))
        except Unreadable as e:
            out.append((label, "unjudged", str(e) + evidence(label, ctl, env)))
        except (KeyError, IndexError, TypeError, ValueError) as e:
            out.append((label, "fail", "reply shape: %s %s" % (type(e).__name__, e) + evidence(label, ctl, env)))
    return out


# ---- evidence on a red (reader's dossier main77): the values that decide a case are printed on its failed or not-judged line ----------------------
# EVIDENCE maps a case label to a function of `env` returning [(label, fn(ctl)), ...]: each read goes through a read the socket already serves (`rows`,
# `uistate`, `winframes`, `measure`, `core`) or the check's own loopback stubs. A read that fails says why; none can change a verdict. Not served (named,
# not invented): whether a window is the key window outside a keyevent reply (Sources/App/WindowBridge.swift:33 would carry "key": w.isKeyWindow), and the
# inspector's own Laya state (LabelInspectorSocket.swift:31 serves `empty` only while the label's card shows a refusal; a row for `layaState`, `layaWords`
# and `aiRefusal` would go beside it). Sibling suites (mcpnotesmac-check.py) add their own labels to this table.
EVIDENCE = {}


def _compact(v, cap=330):
    s = json.dumps(v, sort_keys=True, default=str)
    return s if len(s) <= cap else s[:cap - 3] + "..."


def read_evidence(ctl, reads):
    out = []
    for label, fn, *cap in reads:
        try:
            out.append("%s=%s" % (label, _compact(fn(ctl), *cap)))
        except BaseException as e:      # noqa: BLE001
            if isinstance(e, (KeyboardInterrupt, SystemExit)):
                raise
            out.append("%s=unreadable(%s: %s)" % (label, type(e).__name__, str(e)[:200]))
    return "; ".join(out)


def evidence(label, ctl, env):
    """` | evidence: ...` for a case that has reads declared, else nothing."""
    make = EVIDENCE.get(label)
    if make is None:
        return ""
    try:
        return " | evidence: " + read_evidence(ctl, make(env))
    except BaseException as e:      # noqa: BLE001
        if isinstance(e, (KeyboardInterrupt, SystemExit)):
            raise
        return " | evidence: unreadable (%s: %s)" % (type(e).__name__, e)


def stub_seen(stub):
    """What the loopback stub was asked: counts by method and the paths (an app that never reached it shows zero)."""
    g = list(stub.gets)
    return {"HEAD": sum(m == "HEAD" for m, _ in g), "GET": sum(m == "GET" for m, _ in g), "paths": sorted({p for _, p in g})[:6], "POST": len(stub.requests)}


def ev_rows(ax, *keys):
    return lambda c: [({k: r.get(k) for k in keys} if keys else r) for r in c.rows(ax)]


def ev_measure(ax):
    return lambda c: {k: v for k, v in c.measure(ax).items() if k in ("frame", "window")}


def ev_windows(c):
    return [(f.get("title"), f.get("id"), f.get("kind")) for f in c.winframes()]


def ev_core(path):
    return lambda c: core_or_none(c, path)


def eq(got, want):
    return got == want, "expected %r, got %r" % (want, got)


def rows(ctl, ax):
    return ctl.rows(ax)


def core(ctl, path):
    return ctl.core(path)


def core_or_none(ctl, path):
    """A path the core deliberately holds no value for (a key is never read back): the socket's `core` verb refuses it with
    "no core value at <path>", which for that path IS the answer, None. Any other refusal still means not judged."""
    try:
        return ctl.core(path)
    except Unreadable as e:
        if "no core value at" in str(e):
            return None
        raise


# ---------------------------------------------------------------- stub providers on 127.0.0.1
class Stub:
    def __init__(self, kind):
        self.kind, self.requests, self.answer, self.delay, self.notfound = kind, [], None, 0, set()
        self.gets = []            # (method, path) of every HEAD and GET the app sent (the evidence of the Laya cases: did the app reach this stub at all)
        self.piece_delay = 0      # seconds between the ten pieces of the LAST Laya file (the big one): a download slow enough for the app's 250 ms progress tick
        stub = self

        class H(http.server.BaseHTTPRequestHandler):
            def log_message(self, *a):
                pass

            def _file(self, head):
                path = self.path.lstrip("/")
                if path in stub.notfound or path not in LAYA_FILES:
                    self.send_response(404)
                    self.send_header("Content-Length", "0")
                    self.end_headers()
                    return
                self.send_response(200)
                self.send_header("Content-Length", str(LAYA_SIZE))
                self.end_headers()
                if not head:
                    try:
                        for _ in range(10):
                            self.wfile.write(b"A" * (LAYA_SIZE // 10))
                            self.wfile.flush()
                            time.sleep(stub.delay + (stub.piece_delay if path == LAYA_FILES[-1] else 0))
                    except (BrokenPipeError, ConnectionResetError):      # the app cancelled: the client went away mid-file
                        pass

            def do_HEAD(self):
                stub.gets.append(("HEAD", self.path))
                self._file(True)

            def do_GET(self):
                stub.gets.append(("GET", self.path))
                self._file(False)

            def do_POST(self):
                n = int(self.headers.get("Content-Length", "0"))
                raw = self.rfile.read(n).decode("utf-8", "replace")
                try:
                    body = json.loads(raw) if raw else {}
                except ValueError:
                    body = {"_raw": raw}
                auth = self.headers.get("Authorization", "")
                stub.requests.append({"path": self.path, "auth": auth, "body": body, "raw": raw})
                code, reply = 200, {}
                if self.path == "/validate":
                    if stub.delay:
                        time.sleep(stub.delay)   # a slow provider: the app must not wait for it on its main thread
                    if auth == "Bearer " + D["jev_key"]:
                        reply = {"ok": True}
                    elif auth == "Bearer " + FX["providers"]["anthropic"]["key"]:
                        reply = {"ok": True, "models": ["claude-sonnet-5-5", "claude-haiku-5-5"]}
                    else:
                        code, reply = 401, {"error": "This key is not valid for the provider."}
                elif self.path == "/oauth/token":      # round 9: the ChatGPT sign-in's token address (fixture chatgpt): a token set, as the provider answers
                    reply = {"access_token": "rig-chatgpt-access", "token_type": "Bearer", "expires_in": 3600, "refresh_token": "rig-chatgpt-refresh"}
                elif stub.answer is not None:
                    reply = stub.answer(body)
                elif self.path == "/v1/systemone":      # Jev unscripted (the key Test's sample): an answer in the real shape to what was asked
                    reply = jev_reply(body)
                else:
                    reply = {"text": "stub answer"}
                data = json.dumps(reply).encode()
                self.send_response(code)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)

        # The search model's address is read by the app from its LAUNCH environment (WHOOSH_RIG_EMBED_BASE, Sources/Features/Embedding/MeaningModelRig.swift:30), before this
        # stub exists. scripts/launch-one.sh turns WHOOSH_RIG_EMBED_PORT into that address; the stub that serves the model's files (named "laya" here) then listens on
        # that very port (loopback only). Unset: any free port, as before.
        port = int(os.environ["WHOOSH_RIG_EMBED_PORT"]) if kind == "laya" and os.environ.get("WHOOSH_RIG_EMBED_PORT", "").isdigit() else 0
        self.srv = http.server.ThreadingHTTPServer(("127.0.0.1", port), H)
        self.url = "http://127.0.0.1:%d" % self.srv.server_address[1]
        threading.Thread(target=self.srv.serve_forever, daemon=True).start()

    def close(self):
        self.srv.shutdown()
        self.srv.server_close()


# ---------------------------------------------------------------- the model hub on 127.0.0.1 (model-door: the line "Other model")
HUB_CORPUS = os.path.join(ROOT, "checks", "corpora", "model-hub")
# the address each real answer of the hub is served at (checks/corpora/model-hub, read unchanged). KnowLine is served twice: at its real address, which is
# the catalogue's line knowline08 (its page), so the app refuses it as already in the list; and at a second address, where it is a model the app can add.
HUB_ANSWERS = {"PelaAI/KnowLine-0.8B-Gen1": "knowline-0.8b", "rig/knowline-copy": "knowline-0.8b",
               "Valen-Team/Valen-Decision-1.5-2B": "valen-2b", "fastino/GLiDE-0.8B": "glide-0.8b"}


class HubStub:
    """The hub's two reads as the door asks them (Sources/Features/OnDeviceAI/Core/ModelHub.swift): GET /api/models/<owner>/<name>?blobs=true answers
    `<corpus>.api.json`, GET /<owner>/<name>/resolve/<sha>/config.json answers `<corpus>.config.json` when <sha> is that answer's revision; anything else 404."""
    def __init__(self):
        self.gets = []
        stub = self

        class H(http.server.BaseHTTPRequestHandler):
            def log_message(self, *a):
                pass

            def do_GET(self):
                stub.gets.append(self.path)
                path = self.path.split("?", 1)[0].strip("/")
                parts, body = path.split("/"), None
                if len(parts) == 4 and parts[0] == "api" and parts[1] == "models" and "/".join(parts[2:]) in HUB_ANSWERS:
                    body = open(os.path.join(HUB_CORPUS, HUB_ANSWERS["/".join(parts[2:])] + ".api.json"), "rb").read()
                elif len(parts) == 5 and parts[2] == "resolve" and parts[4] == "config.json" and "/".join(parts[:2]) in HUB_ANSWERS:
                    name = HUB_ANSWERS["/".join(parts[:2])]
                    if json.load(open(os.path.join(HUB_CORPUS, name + ".api.json"), encoding="utf-8")).get("sha") == parts[3]:
                        body = open(os.path.join(HUB_CORPUS, name + ".config.json"), "rb").read()
                self.send_response(200 if body is not None else 404)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(body or b"")))
                self.end_headers()
                if body:
                    self.wfile.write(body)

        self.srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
        self.url = "http://127.0.0.1:%d" % self.srv.server_address[1]
        threading.Thread(target=self.srv.serve_forever, daemon=True).start()

    def close(self):
        self.srv.shutdown()
        self.srv.server_close()


# ---------------------------------------------------------------- Jev's real wire shape (hid-ai round 5; fix-jevrig findings 1-4)
# Request (decision/src/adapters/jev.rs `decide`, wire.rs `request_body`): {"state": <text>, "model": "jev-latest", "questions": {<id>: {...}}}; the state is
# "from: ..\nto: ..\ncc: ..\nsubject: ..\nlist header: yes|no\n\n<body>". Answer (wire.rs `read_answers`): {"model": .., "answers": {<every id asked>:
# {"choice": .., "confidence": ..} | {"noul": p} | {"score": v}}}.
JEV_TABS = ("Primary", "Updates", "Promotions", "Social")


def jev_state_text(sender, subject, list_header, body_text):
    return "from: %s\nto: \ncc: \nsubject: %s\nlist header: %s\n\n%s" % (sender, subject, "yes" if list_header else "no", body_text)


def jev_body(state, question):
    return {"state": state, "model": "jev-latest", "questions": {"which_tab": {"type": "choice", "instructions": question, "criteria": {t: t for t in JEV_TABS}}}}


def jev_reply(body, choice=None, conf=None):
    """Jev's answer to the questions the body asked, in the real shape; `choice`/`conf` script it (a check's scripted model)."""
    qs = body.get("questions") if isinstance(body, dict) else None
    out = {}
    for qid, q in (qs.items() if isinstance(qs, dict) else []):
        kind = q.get("type") if isinstance(q, dict) else None
        if kind == "noul":
            out[qid] = {"noul": 0.9 if conf is None else conf}
        elif kind == "score":
            out[qid] = {"score": 0.5 if conf is None else conf}
        else:
            crit = (q.get("criteria") if isinstance(q, dict) else None) or {"Primary": "Primary"}
            out[qid] = {"choice": next(iter(crit)) if choice is None else choice, "confidence": 0.9 if conf is None else conf}
    return {"model": "jev-latest", "answers": out}


def jev_answer(r):
    """(choice, confidence) of the one question asked, from an answer in the real shape; None when the shape is not Jev's."""
    a = next(iter((r.get("answers") or {}).values()), None) if isinstance(r, dict) else None
    return (a["choice"], float(a["confidence"])) if isinstance(a, dict) and "choice" in a and "confidence" in a else None


def post(url, path, body, key=None):
    req = urllib.request.Request(url + path, data=json.dumps(body).encode(), method="POST",
                                 headers={"Content-Type": "application/json", **({"Authorization": "Bearer " + key} if key else {})})
    try:
        with urllib.request.urlopen(req, timeout=3) as r:
            return r.status, json.loads(r.read().decode())
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read().decode() or "{}")


# ---------------------------------------------------------------- FakeApp: stand-in on the wire (selftest only)
WORDS = {"readMail": "read mail", "search": "search", "draft": "draft", "label": "label", "sendWithConfirmation": "send with your confirmation"}
ALLOW = {"read": "readMail", "search": "search", "draft": "draft", "label": "label", "send": "sendWithConfirmation"}
TAB_TITLE = {"primary": "Primary", "updates": "Updates", "promotions": "Promotions", "social": "Social"}


class FakeApp:
    def __init__(self, mutants=()):
        self.m = set(mutants)
        self.models_dir = None      # the rig's run folder (FakeServer sets it): the app keeps the downloaded model under <dir>/Models/laya
        self.laya_disk = False      # LayaModelStore reads the DISK (`ready.json`): a `fixture reset` does not remove a downloaded model
        self.reset()

    def _laya_manifest(self):
        return os.path.join(self.models_dir, "Models", "laya", "ready.json") if self.models_dir else ""

    def _laya_sync(self):
        """The row's state is read from the disk each time (LayaModelStore.refresh): a removed folder means not downloaded, a finished download means ready."""
        if self.laya_disk and self.models_dir and not os.path.exists(self._laya_manifest()):
            self.laya_disk = False
        if self.laya_state == "ready" and not self.laya_disk:
            self.laya_state = "absent"
        elif self.laya_state == "absent" and self.laya_disk:
            self.laya_state = "ready"

    TICK_S = 0.25       # LayaModelStore.progressTickNs: the row's figure is the bytes read at the last tick

    def _laya_start(self):
        """A download as the app does it: the bytes arrive at the server's pace (a real GET of each file), the row shows them at 250 ms ticks."""
        self.laya_gen = getattr(self, "laya_gen", 0) + 1
        gen = self.laya_gen
        self.laya_state, self.laya_bytes, self.laya_seen = "downloading", 0, 0
        threading.Thread(target=self._laya_run, args=(gen,), daemon=True).start()
        threading.Thread(target=self._laya_ticker, args=(gen,), daemon=True).start()

    def _laya_run(self, gen):
        try:
            for f in LAYA_FILES:
                with urllib.request.urlopen(self.laya_url + "/" + f, timeout=30) as r:
                    while gen == self.laya_gen:
                        chunk = r.read(LAYA_SIZE // 10)
                        if not chunk:
                            break
                        self.laya_bytes += len(chunk)
                if gen != self.laya_gen:
                    return
            if gen == self.laya_gen:
                self._laya_download_done()
        except (OSError, ValueError):
            if gen == self.laya_gen:
                self.laya_state, self.laya_why = "failed", "Could not download the decision model. The download stopped. Click Retry."

    def _laya_ticker(self, gen):
        while gen == self.laya_gen and self.laya_state == "downloading":
            time.sleep(self.TICK_S)
            if "laya-progress-frozen" not in self.m:
                self.laya_seen = self.laya_bytes

    def _kl_manifest(self):
        return os.path.join(self.models_dir, "Models", "knowline-0.8b-gen1", "01952e55", "ready.json") if self.models_dir else ""

    def _kl_sync(self):
        """The second line reads the disk too (ModelInstaller.refresh): a removed folder is not downloaded, a finished download is ready."""
        here = bool(self.models_dir) and os.path.exists(self._kl_manifest())
        if self.kl_state == "ready" and not here:
            self.kl_state = "absent"
        elif self.kl_state == "absent" and here:
            self.kl_state = "ready"

    def _kl_start(self):
        self.kl_gen += 1
        gen = self.kl_gen
        self.kl_state = "downloading"
        if self.chosen_model != KL:
            self.wanted_model = KL

        def run():
            try:
                for f in LAYA_FILES:
                    with urllib.request.urlopen(self.kl_url + "/" + f, timeout=30) as r:
                        while gen == self.kl_gen and r.read(LAYA_SIZE // 10):
                            pass
                    if gen != self.kl_gen:
                        return
            except (OSError, ValueError):
                if gen == self.kl_gen:
                    self.kl_state, self.kl_why = "failed", "Could not download the decision model. The download stopped. Click Retry."
                return
            if gen != self.kl_gen:
                return
            if self.models_dir:
                os.makedirs(os.path.dirname(self._kl_manifest()), exist_ok=True)
                with open(self._kl_manifest(), "w", encoding="utf-8") as f:
                    f.write("{}")
            self.kl_state = "ready"
            if self.wanted_model == KL:
                self.wanted_model = None
                if "second-model-never-in-use" not in self.m:
                    self._use(KL)
        threading.Thread(target=run, daemon=True).start()

    def _use(self, model):
        self.chosen_model = model
        self.core_["decision_model/model"] = model

    def _kl_row(self):
        self._kl_sync()
        in_use, wanted = self.chosen_model == KL, self.wanted_model == KL
        row = {"model": KL, "name": "KnowLine 0.8B", "facts": model_facts_wanted("knowline08"), "in_use": in_use, "wanted": wanted}
        if self.kl_state == "downloading":
            row.update({"state": "downloading", "words": "Downloading", "download": False, "cancel": True, "key": "esc" if in_use or wanted else ""})
        elif self.kl_state == "failed":
            row.update({"state": "failed", "words": self.kl_why, "download": True, "retry": True, "button": "Retry", "key": "\u21a9" if in_use or wanted else ""})
        elif self.kl_state == "ready":
            row.update({"state": "ready", "words": "", "chip": "Ready", "download": False})
        else:
            row.update({"state": "absent", "words": "Not on this Mac yet. 1.73 GB, downloaded once." if in_use else "", "download": True, "button": "Download model",
                        "key": "\u21a9" if in_use else ""})
        return row

    def _laya_download_done(self):
        if self.wanted_model == "laya":
            self.wanted_model = None
            self._use("laya")
        self.laya_state, self.laya_disk = "ready", True
        if self.models_dir:
            os.makedirs(os.path.dirname(self._laya_manifest()), exist_ok=True)
            with open(self._laya_manifest(), "w", encoding="utf-8") as f:
                f.write("{}")

    # ---- the line "Other model" (model-door): a hub address checked, a model the app runs added as a line; the stand-in of OtherModelLine, ModelHub,
    # DecisionModels.lines and AISettingsStore's door. The added models live on the disk (<run>/Models/added-models.json) as in the app.
    BUILT_IN_PAGES = ("https://huggingface.co/convaiinnovations/laya", "https://huggingface.co/PelaAI/KnowLine-0.8B-Gen1")
    OTHER_FIELD = "other-model"

    def _added_file(self):
        return os.path.join(self.models_dir, "Models", "added-models.json") if self.models_dir else ""

    def _door_reset(self):
        """A reset closes the door and re-reads the added models from the disk (they outlive it, as a downloaded model does)."""
        self.other_open, self.other_refusal = False, ""
        self.keys.pop(self.OTHER_FIELD, None)
        if self.key_focus == self.OTHER_FIELD:
            self.key_focus = None
        self.hub_url = getattr(self, "hub_url", "")                 # the staged hub outlives a reset (ModelHub's rig base)
        self.added_urls = getattr(self, "added_urls", {})
        self.added_gen = getattr(self, "added_gen", 0) + 1
        self.added_state, self.added_why = {}, {}
        path = self._added_file()
        self.added = json.load(open(path, encoding="utf-8")) if path and os.path.exists(path) else []

    def _added_save(self):
        os.makedirs(os.path.dirname(self._added_file()), exist_ok=True)
        with open(self._added_file(), "w", encoding="utf-8") as f:
            json.dump(self.added, f)

    def _added_manifest(self, a):
        return os.path.join(self.models_dir, "Models", "hub", a["owner"], a["name"], a["revision"][:8], "ready.json")

    def _hub_get(self, path):
        try:
            with urllib.request.urlopen(self.hub_url + path, timeout=10) as r:
                return r.status, r.read()
        except urllib.error.HTTPError as e:
            return e.code, b""
        except (OSError, ValueError):
            return None, b""

    def _door_check(self):
        """Check: the address, the duplicate, the hub's card and config, the family's files; one sentence when refused."""
        import re
        typed = self.keys.get(self.OTHER_FIELD, "").strip().split("?")[0].split("#")[0]
        if typed.lower().startswith("https://"):
            typed = typed[8:] if typed[8:].lower().startswith("huggingface.co/") else "!"
        if typed.lower().startswith("huggingface.co/"):
            typed = typed[len("huggingface.co/"):]
        parts = typed.split("/")
        ok = len(parts) >= 2 and all(re.fullmatch(r"[A-Za-z0-9._-]{1,96}", p) and set(p) != {"."} for p in parts[:2])
        if not ok and "not-an-address-asks-the-hub" not in self.m:
            self.other_refusal = "This is not a Hugging Face address. Write it as owner/model."
            return
        owner, name = (parts + ["", ""])[:2]
        aid, page = "hub:%s/%s" % (owner, name), "https://huggingface.co/%s/%s" % (owner, name)
        if "duplicate-added" not in self.m and (page.lower() in [x.lower() for x in self.BUILT_IN_PAGES] or aid.lower() in [a["id"].lower() for a in self.added]):
            self.other_refusal = "This model is already in the list."
            return
        code, raw = self._hub_get("/api/models/%s/%s?blobs=true" % (owner, name))
        if code in (404, 401):
            self.other_refusal = "Could not reach Hugging Face. Connect, then check again." if "404-said-unreachable" in self.m else "Hugging Face has no model at this address."
            return
        if code != 200:
            self.other_refusal = "Could not reach Hugging Face. Connect, then check again."
            return
        card = json.loads(raw)
        if card.get("gated") is not False:
            self.other_refusal = "This model needs a sign-in on Hugging Face. Whoosh downloads public models only."
            return
        sib = {x["rfilename"]: x for x in card.get("siblings", [])}
        code, raw = self._hub_get("/%s/%s/resolve/%s/config.json" % (owner, name, card["sha"]))
        kind = json.loads(raw).get("model_type", "") if code == 200 else ""
        if kind not in ("qwen3_5", "qwen3_5_text") and "kind-refusal-silent" not in self.m:
            self.other_refusal = "Whoosh cannot run this model: its kind (%s) is not one it knows." % kind
            return
        if "decision_config.json" in sib and "own-head-accepted" not in self.m:
            self.other_refusal = "Whoosh cannot run this model yet: it reads decisions through its own head."
            return
        weights = ["model.safetensors"] if "model.safetensors" in sib else sorted(n for n in sib if re.fullmatch(r"model-\d{5}-of-\d{5}\.safetensors", n))
        names = ["config.json", "tokenizer.json", "tokenizer_config.json"] + [n for n in ("vocab.json", "merges.txt", "chat_template.jinja", "temperatures.json") if n in sib] + weights
        files = [{"path": n, "bytes": sib[n]["size"], "sha256": (sib[n].get("lfs") or {}).get("sha256", "")} for n in names if n in sib]
        lic = next((t[len("license:"):] for t in card.get("tags", []) if t.startswith("license:")), "")
        found = {"id": aid, "owner": owner, "name": name, "revision": card["sha"], "licence": lic, "files": files}
        if "other-added-line-missing" not in self.m:
            self.added.append(found)
            self._added_save()
        self.other_open, self.other_refusal = False, ""
        self.keys.pop(self.OTHER_FIELD, None)
        self.key_focus = None

    def _added_row(self, k, a):
        aid = a["id"]
        self._added_sync(a)
        st = self.added_state.get(aid, "absent")
        mb = round(sum(f["bytes"] for f in a["files"]) / 1e6)
        size = "%d MB" % mb if mb < 1000 else "%.2f GB" % (mb / 1000)
        lic = a["licence"] or "no licence stated"
        lic = "own licence, see its page" if lic.lower() == "other" else lic
        in_use, wanted = self.chosen_model == aid, self.wanted_model == aid
        row = {"model": aid, "name": a["name"], "facts": "%s \u00b7 %s" % (size, lic), "in_use": in_use, "wanted": wanted,
               "ax": "mac-settings-ai.decisions.added.%d" % k, "added": True, "remove": not in_use and st != "downloading"}
        if st == "downloading":
            row.update({"state": "downloading", "words": "Downloading", "download": False, "cancel": True})
        elif st == "ready":
            row.update({"state": "ready", "words": "", "chip": "Ready", "download": False})
        elif st == "failed":
            row.update({"state": "failed", "words": self.added_why.get(aid, ""), "download": True, "retry": True, "button": "Retry"})
        else:
            row.update({"state": "absent", "words": "", "download": True, "button": "Download model"})
        return row

    def _added_sync(self, a):
        here = bool(self.models_dir) and os.path.exists(self._added_manifest(a))
        st = self.added_state.get(a["id"], "absent")
        if st == "ready" and not here:
            self.added_state[a["id"]] = "absent"
        elif st == "absent" and here:
            self.added_state[a["id"]] = "ready"

    def _added_start(self, a):
        aid, gen, url = a["id"], self.added_gen, self.added_urls.get(a["id"], "")
        self.added_state[aid] = "downloading"
        if self.chosen_model != aid:
            self.wanted_model = aid

        def run():
            try:
                for f in LAYA_FILES:
                    with urllib.request.urlopen(url + "/" + f, timeout=30) as r:
                        while gen == self.added_gen and r.read(LAYA_SIZE // 10):
                            pass
            except (OSError, ValueError):
                if gen == self.added_gen:
                    self.added_state[aid], self.added_why[aid] = "failed", "Could not download the decision model. The download stopped. Click Retry."
                return
            if gen != self.added_gen:
                return
            os.makedirs(os.path.dirname(self._added_manifest(a)), exist_ok=True)
            with open(self._added_manifest(a), "w", encoding="utf-8") as f:
                f.write("{}")
            self.added_state[aid] = "ready"
            if self.wanted_model == aid:
                self.wanted_model = None
                if "added-never-in-use" not in self.m:
                    self._use(aid)
        threading.Thread(target=run, daemon=True).start()

    def _door_click(self, s):
        P = "mac-settings-ai.decisions."
        no = {"ok": False, "error": "nothing registered at %s" % s}
        if s == P + "other.add":
            if self.engine != "local" or self.other_open:
                return no
            if "other-add-ignored" not in self.m:
                self.other_open, self.other_refusal, self.key_focus = True, "", self.OTHER_FIELD
            return {"ok": True}
        if s == P + "other.field":
            if not self.other_open:
                return no
            self.key_focus = self.OTHER_FIELD
            return {"ok": True}
        if s == P + "other.check":
            if not self.other_open:
                return no
            self.other_refusal = ""
            self._door_check()
            return {"ok": True}
        if s == P + "other.leave":
            if not self.other_open:
                return no
            self.other_open, self.other_refusal = False, ""
            self.keys.pop(self.OTHER_FIELD, None)
            self.key_focus = None
            return {"ok": True}
        parts = s[len(P):].split(".")          # added.<k>.<door>
        if len(parts) != 3 or parts[0] != "added" or not parts[1].isdigit() or not 1 <= int(parts[1]) <= len(self.added):
            return no
        a, door = self.added[int(parts[1]) - 1], parts[2]
        self._added_sync(a)
        st = self.added_state.get(a["id"], "absent")
        if door in ("download", "choose"):
            if st == "ready":
                if self.chosen_model == a["id"]:
                    return no
                self._use(a["id"])
            elif st != "downloading":
                self._added_start(a)
            return {"ok": True}
        if door == "cancel":
            if st != "downloading":
                return no
            self.added_gen += 1
            self.added_state[a["id"]] = "absent"
            if self.wanted_model == a["id"]:
                self.wanted_model = None
            return {"ok": True}
        if door == "remove":
            if self.chosen_model == a["id"] or st == "downloading":
                return no
            if "remove-keeps-the-line" not in self.m:
                self.added = [x for x in self.added if x["id"] != a["id"]]
                self._added_save()
                shutil.rmtree(os.path.join(self.models_dir, "Models", "hub", a["owner"], a["name"]), ignore_errors=True)
            return {"ok": True}
        return no

    def _door_rows(self, ax):
        P = "mac-settings-ai.decisions."
        if ax == P + "lines":
            if self.engine != "local":
                return []
            ids = ["laya", KL] + [a["id"] for a in self.added]
            return [{"line": i} for i in ids] + ([] if "other-line-missing" in self.m else [{"line": "other"}])
        if ax == P + "other":
            if self.engine != "local":
                return []
            o = self.other_open
            return [{"name": "Other model", "facts": "" if o else "A Hugging Face address of a model Whoosh can run.", "in_use": False, "open": o,
                     "address": self.keys.get(self.OTHER_FIELD, "") if o else "", "focused": self.key_focus == self.OTHER_FIELD, "checking": False,
                     "refusal": self.other_refusal if o else "", "button": "Check" if o else "Add", "key": "\u21a9" if o else ""}]
        k = int(ax[len(P + "added."):])
        return [self._added_row(k, self.added[k - 1])] if 1 <= k <= len(self.added) else []

    def reset(self):
        self.surface, self.core_, self.prov_url, self.jev_url = "mac-main", {}, "", ""
        self.providers = {p: {"state": "not connected", "models": [], "note": ""} for p in ("anthropic", "openai", "gemini", "openrouter")}
        self.providers["chatgpt"] = {"state": "not connected", "models": [], "note": "" if "no-chatgpt-note" in self.m else "Signing in with ChatGPT can stop working at any time."}
        self.speech, self.speech_fail, self.speech_retry = "none", False, []
        self.form, self.agent_name, self.agent_allowed, self.agent_open = [], "", [], False
        self.section, self.keys, self.key_focus, self.pending = "decisions", {}, None, {}
        self.clock_evt = threading.Event()      # `clock advance` lets a key check's answer land (the fixture core lands it by its own clock)
        self.saved = [{"name": "Ask my inbox", "provider": "anthropic", "model": "claude-sonnet-5-5", "allowed": ["readMail", "search"], "builtin": True},
                      {"name": "Writer", "provider": "anthropic", "model": "claude-sonnet-5-5", "allowed": ["draft"], "builtin": True}]
        self.jev_wanted, self.engine_note, self.jev_note, self.laya_ready = False, "", "", False
        self.laya_url, self.laya_state, self.laya_got, self.laya_why, self.laya_own, self.laya_pending = "", "absent", 0, "", "absent", False
        self.laya_gen, self.laya_bytes, self.laya_seen = getattr(self, "laya_gen", 0) + 1, 0, 0      # a download of the last run stops with the reset
        # the list of on-device models (SPEC A133): the model in use, the one asked for, and the second line's download
        self.chosen_model, self.wanted_model, self.kl_url, self.kl_state, self.kl_why = "laya", None, "", "absent", ""
        self.kl_gen = getattr(self, "kl_gen", 0) + 1
        if self.laya_disk:                                  # the model on the disk outlives the reset (the real store re-reads it)
            self.laya_state = "ready"
        self.meaning_url, self.meaning_state, self.meaning_got = "", "notDownloaded", 0
        self.peers = []                                              # the sockets this stand-in holds off this Mac (what its `lsof` text shows)
        self.index_fig, self.index_last_line = (0, 0), None          # the builder's figures (indexed, queued) as `fixture index-progress` set them
        self.draft_model, self.speech_engine, self.sw_pane = ("anthropic", "claude-sonnet-5-5"), "local", "general"
        self.buf = ""
        self.sheet, self.addr_focus, self.addr = False, False, ""      # typing-lost-r2: the add-account sheet beside a closed Settings window
        self.citations, self.proposal, self.items, self.notice, self.error = [], [], [], [], []
        self.fail_provider, self.deleted = False, set()
        # search (round 2): the one field; results by meaning, never an answer
        self.corpus, self.hold, self.index, self.q_shown, self.results, self.skeleton = None, False, "ready", None, [], 0
        self.focus, self.sel, self.lf, self.labels, self.undo_stack, self.reader, self.snotice = None, set(), None, {}, [], None, []
        self.core_["provider_request/count"], self.core_["undo_entry/count"] = 0, 0
        self.core_["label/%s/exists" % FX["search"]["label"]] = False
        self.mcp_on, self.grants, self.mcp_prompt, self.mcp_results = True, set(), [], []
        self.mcp_pending = None
        self.engine, self.dec_ready = "local", {"local": "ready", "jev": "needs key", "off": "ready"}
        self.lists = copy.deepcopy(D["shipped"])
        for p, es in self.lists.items():
            for e in es:
                e["outcome"] = None
        self.jev_consent, self.last = False, []
        self.corr, self.examples, self.preview, self.editor = {}, 0, [], []
        self.q_focus = None
        self.vips = list(D["vip_list"])
        self._door_reset()
        if "model-entry-missing" in self.m:
            self.lists["which-tab"] = [e for e in self.lists["which-tab"] if e["kind"] != "model"]
        if "no-default" in self.m:
            self.lists["which-tab"] = [e for e in self.lists["which-tab"] if e["kind"] != "default"]

    # ---- wire
    def handle(self, verb, args):
        try:
            fn = getattr(self, "v_" + verb, None)
            if fn is None:
                return {"ok": False, "error": "unknown verb %s" % verb}
            r = fn(*args)
            return r if isinstance(r, dict) else {"ok": True}
        except KeyError as e:
            return {"ok": False, "error": "no rows at %s" % e}
        except Exception as e:  # a stand-in must never crash the selftest silently
            return {"ok": False, "error": "%s: %s" % (type(e).__name__, e)}

    def v_selftestmutants(self):
        """Selftest only: the mutants this stand-in runs under, for the static cases that read the Swift sources (a real app answers 'unknown verb')."""
        return {"ok": True, "mutants": sorted(self.m)}

    def v_uistate(self):
        return {"ok": True, "state": {"surface": self.surface, "core": "fixture" if "core-fixture" in self.m else "rust"}}

    def v_type(self, text):
        if self.lf is not None:
            self.lf += text
            return
        if self.sheet and self.addr_focus:
            # the sheet holds the keys: its address field takes the text, unless the closed pane's draft still listens to the shared buffer
            if "closed-ai-pane-keeps-taking-the-typed-text" in self.m and self.agent_open:
                self.agent_name += text
            else:
                self.addr += text
            return
        if self.q_focus and not self.key_focus and self.surface == "mac-settings-ai":
            # a model entry's question was clicked last: the words are added to it word for word (SettingsAIMac typeIntoDraft)
            self.q_focus["question"] = (self.q_focus.get("question") or "") + text
            return
        self.buf += text
        if self.key_focus and self.surface == "mac-settings-ai" and "typed-key-lost" not in self.m:
            self.keys[self.key_focus] = self.keys.get(self.key_focus, "") + text
            return
        if self.key_focus and "typed-key-lost" in self.m:
            return
        if self.agent_open and self.surface == "mac-settings-ai":
            self.agent_name += text
            self._save_agent()

    def v_clock(self, verb, secs):
        self.clock_evt.set()
        self._settle()      # the core lands what is due before `clock advance` answers
        return {"ok": True}

    def v_core(self, kind, path):
        key = path.replace(".", "/")
        if key == "decision_consent/jev":      # the real core answers the record {"needed","given"} (contracts decision_consent.rs:42), never a bare yes/no
            return {"ok": True, "value": {"given": bool(self.core_.get(key)), "needed": True}}
        return {"ok": True, "value": self.core_.get(key)}

    def v_capture(self, *a):
        return {"ok": True, "path": "/tmp/x.png"}

    def v_rows(self, ax):
        if ax == "mac-main.sync.line":
            return {"ok": True, "rows": [dict({"summary": "Up to date"}, **self._sync_index_fields())]}
        rs = self.rows_(ax)
        if ax == "mac-settings-ai.meaning":
            rs = [dict(r, **self._index_fields(r.get("state") == "ready")) for r in rs]
        return {"ok": True, "rows": rs}

    def v_key(self, chord):
        dead = lambda name: "key-%s-dead" % name in self.m
        if chord == "⌘K" and self.surface == "mac-main" and not dead("cmdk"):
            self.surface, self.buf = "mac-search", ""
            return {"ok": True}
        if chord == "Enter" and self.surface == "mac-search":
            if self.lf is not None:
                return {"ok": True} if dead("label-enter") else self._label_apply()
            if self.buf:
                return {"ok": True} if dead("enter-search") else self._search()
            return {"ok": True} if dead("enter-open") else self._open()
        if chord == "Escape" and self.surface == "mac-reader" and not dead("esc"):
            self.surface = "mac-search"
            if "esc-clears-results" in self.m:
                self.results = []
            return {"ok": True}
        if chord == "⌘A" and self.surface == "mac-search":
            if self.results and not dead("cmda"):
                self.sel = set(r["thread"] for r in self.results[:1 if "select-first-only" in self.m else None])
            elif not self.results:
                self.buf = ""            # select-all in the field, then typing replaces the text
            return {"ok": True}
        if chord == "L" and self.surface == "mac-search" and self.sel and not dead("l"):
            self.lf = ""
            return {"ok": True}
        if chord == "Z" and not dead("z"):
            return self._undo()
        if chord == "⌘W":
            self.surface = "mac-main"
            return {"ok": True}
        return {"ok": False, "error": "no binding"}

    def v_fixture(self, name, *a):
        fn = getattr(self, "f_" + name.replace("-", "_"), None)
        if fn is None:
            return {"ok": False, "error": "unknown fixture event %s" % name}
        fn(*a)
        return {"ok": True}

    # ---- fixture events
    def f_reset(self):
        self.reset()

    def f_chatgpt(self, verdict, base):
        """The stand-in of `fixture chatgpt`: the token is asked of the stub on loopback; the row says connected when it answered (the mutant
        `chatgpt-row-never-connected` is SettingsAIMacFeature's row word fixed to "not connected")."""
        if verdict != "approve":
            return
        code, r = post(base, "/oauth/token", {"grant_type": "authorization_code", "code": "rig-code"})
        if code == 200 and r.get("access_token") and "chatgpt-row-never-connected" not in self.m:
            self.providers["chatgpt"]["state"] = "connected"

    def f_provider_endpoint(self, name, url):
        if name == "model-hub":
            self.hub_url = url
        elif name.startswith("model:hub:"):
            self.added_urls[name[len("model:"):]] = url
        elif name == "model:" + KL:
            self.kl_url = url
        elif name == "laya":
            self.laya_url = url
        elif name == "meaning":
            self.meaning_url = "" if "meaning-address-ignored" in self.m else url      # the real app reads this address from its launch environment only
        elif name == "jev":
            self.jev_url = url
        else:
            self.prov_url = url

    def lsof_text(self):
        """What `lsof -a -i -n -P -p <pid>` prints for this stand-in: a line per socket it holds off this Mac (none unless a mutant opened one)."""
        return "".join("Whoosh 4242 u IPv4 0x1 0t0 TCP 192.168.1.5:50000->%s (SYN_SENT)\n" % p for p in self.peers)

    def f_paste(self, what):
        if what in ("anthropic-key", "bad-key"):
            key = FX["providers"]["anthropic"]["key"] if what == "anthropic-key" else "sk-bad"
            t = threading.Thread(target=self._check_key, args=("anthropic", key), daemon=True)      # the app asks off the main thread
            self.pending["anthropic"] = t
            t.start()
        elif what == "jev-key":
            self.dec_ready["jev"] = "ready"
            self.core_["provider_credential/jev/stored"] = True
            self.core_["decision_model/key"] = D["jev_key"] if "jev-key-in-db" in self.m else None

    def _check_key(self, name, key):
        """Asks the stub provider about the key (the app's core does this off the main thread) and applies the answer."""
        if name != "anthropic":      # round 6: the other providers have key doors too (the real core's table); the test engine takes a key of 8 or more characters
            if "unsupported-silent" in self.m or len(key) < 8:
                self.providers[name].update(state="refused", note="")
            else:
                self.providers[name].update(state="connected", note="")
            return
        two = "one-step-answer" not in self.m
        if two:
            self._paths("checking")
        if "key-not-validated" in self.m:
            code, r = (200, {"models": ["claude-sonnet-5-5", "claude-haiku-5-5"]}) if key == FX["providers"]["anthropic"]["key"] else (401, {"error": "This key is not valid for the provider."})
        else:
            code, r = post(self.prov_url, "/validate", {}, key)
            self.core_["provider_request/count"] += 1
        if two:
            self.clock_evt.wait(0.1)      # the fixture core lands the check 600 ms of ITS clock after the save; the stand-in waits less
        p = self.providers["anthropic"]
        if code == 200 and "key-never-connects" not in self.m:
            p.update(state="connected", models=[] if "no-models" in self.m else r["models"], note="")
            self._paths("connected", models=p["models"])
            self.core_["provider_credential/anthropic/stored"] = True
            self.core_["ai_provider/anthropic/key"] = key if "key-in-db" in self.m else None
        else:
            p.update(state="refused", note="" if "refusal-words-lost" in self.m else ("401" if "reason-code-only" in self.m else r.get("error", "")))
            self._paths("refused", words=r.get("error", ""))

    def _paths(self, state, words="", models=()):
        """The core's three state paths for the provider (the real core and the fixture core serve the same ones)."""
        if "state-path-not-served" in self.m:
            return
        self.core_["ai_provider/anthropic/state"] = state
        self.core_["ai_provider/anthropic/refusal"] = (words or None) if state == "refused" and "refusal-path-not-served" not in self.m else None
        self.core_["ai_provider/anthropic/models"] = list(models)

    def _connect(self, name):
        """The Connect button: the typed key goes to the provider; the answer comes later unless the mutant waits for it."""
        key = "" if "connect-ignores-key" in self.m else self.keys.pop(name, "")
        self.keys.pop(name, None)
        self.key_focus = None
        if "connect-blocks" in self.m:
            self._check_key(name, key)
            return
        t = threading.Thread(target=self._check_key, args=(name, key), daemon=True)
        self.pending[name] = t
        t.start()

    def _settle(self):
        for t in list(self.pending.values()):
            t.join()
        self.pending.clear()

    def f_corpus(self, path):
        self.corpus = json.load(open(path, encoding="utf-8"))["messages"]

    def f_hold_search(self):
        self.hold = True

    def f_release_search(self):
        self.hold = False
        if self.skeleton:
            self.skeleton = 0
            self._show()

    def f_index_state(self, state):
        if "never-merges" in self.m and state == "ready":
            return
        self.index = state
        if state == "ready" and self.q_shown and not self.hold:
            self._show()

    def f_speech_fail(self):
        self.speech_fail = True

    def f_index_progress(self, indexed, queued):
        """`fixture index-progress <indexed> <queued>` (finding F1 of docs/lanes/gap-search-cases.md, Sources/ControlSocket/FixtureVerbs.swift): a build that serves it."""
        if "index-progress-not-served" in self.m:
            raise AttributeError("unknown fixture event index-progress")
        self.index_fig = (int(indexed), int(queued))

    def relaunch(self):
        """The copy quits and starts again on the same run folder (scripts/launch-one.sh --relaunch). What survives is the disk: a search model download that was
        running is found again by the store, which announces it, and the line follows it (MeaningLineModel); the surfaces start closed."""
        self.surface, self.section = "mac-main", "decisions"
        self.index_fig = (0, 0)
        if self.meaning_state == "downloading":
            if "meaning-lost-on-relaunch" in self.m:
                self.meaning_state = "notDownloaded"
            else:
                self.meaning_got = 0

    def _index_fields(self, ready):
        done, queued = self.index_fig
        building = queued > 0
        if building:
            line = "Indexing your mail: {:,} of {:,}".format(done, done + queued)
        elif done > 0:
            line = "1 mail indexed." if done == 1 else "{:,} mails indexed.".format(done)
        else:
            line = None
        if "meaning-index-line-never" in self.m:
            line = None
        if "meaning-index-line-stuck" in self.m:
            if building:
                self.index_last_line = line
            elif self.index_last_line:
                line = self.index_last_line
        return {"indexing": building, "index_done": done, "index_total": done + queued, "index_paused": False, "index_failed": 0,
                "index_text": line if ready else None}

    def _sync_index_fields(self):
        done, queued = self.index_fig
        building = queued > 0 and "status-index-never" not in self.m
        text = "indexing %d %%" % int(done * 100 / (done + queued)) if building else None
        if "status-index-stuck" in self.m and not building and self.index_last_line:
            text = self.index_last_line
        if building:
            self.index_last_line = text
        return {"indexing": building, "index_done": done, "index_total": done + queued, "index_paused": False, "index_text": text}

    def f_provider_fails(self, kind):
        self.fail_provider = True

    def f_delete_message(self, mid):
        self.deleted.add(mid)

    def f_agent_allowed(self, csv):
        self.agent_allowed = csv.split(",")
        self._save_agent()

    def f_mcp_server(self, state):
        self.mcp_on = state == "on"

    def f_mcp_search(self, app, query):
        granted = app in self.grants or "ungranted-gets-mail" in self.m
        hits = [m for m in MSG.values() if query.lower() in m["subject"].lower()]
        self.mcp_prompt, self.mcp_results, self.mcp_pending = [], [], None
        if not granted:
            return
        if "no-prompt" in self.m:
            self.mcp_results = [{"id": h["id"], "subject": h["subject"]} for h in hits]
            return
        self.mcp_prompt = [{"state": "waiting"}]
        self.mcp_pending = hits

    def f_consent(self, who, yes):
        self.jev_consent = yes == "yes"
        self.core_["decision_consent/jev"] = self.jev_consent
        self._jev_try()

    def f_corrections(self, page, n, who):
        self.corr = {"page": page, "n": int(n), "who": who}

    def f_examples(self, n):
        self.examples = int(n)

    def f_add_rule(self, page, words, outcome):
        e = {"kind": "rule", "word": words, "outcome": outcome, "custom": True}
        es = self.lists[page]
        es.insert(len(es) - 1 if "rule-appended-at-end" in self.m else 0, e)
        self._renumber(page)

    def f_decide(self, page, msgk, label, conf):
        self._decide(page, msgk, label, float(conf))

    # ---- clicks
    def v_click(self, ax):
        s = ax
        if "dead:" + ax in self.m:
            return {"ok": False, "error": "nothing registered at %s" % s}
        if s.startswith("mac-settings-ai.decisions.other.") or s.startswith("mac-settings-ai.decisions.added."):
            return self._door_click(s)
        if s == A["palette_ai_settings"]:
            if "no-ai-entry" in self.m:
                return {"ok": False, "error": "nothing registered at %s" % s}
            self.surface = "mac-settings-ai"
        elif s == A["palette_decisions"]:
            if "no-decisions-entry" in self.m:
                return {"ok": False, "error": "nothing registered at %s" % s}
            self.surface = "mac-settings-ai"
        elif s == A["palette_mcp"]:
            if "no-mcp-entry" in self.m:
                return {"ok": False, "error": "nothing registered at %s" % s}
            self.surface = "mac-mcp-notes"
        elif s == A["speech_local"]:
            if self.speech_fail:
                self.speech, self.speech_retry = "failed", [{"reason": "" if "retry-without-reason" in self.m else "The download stopped: the network dropped."}]
            else:
                self.speech = "downloading" if "speech-never-ready" in self.m else "ready"
                self.core_["speech_engine/chosen/state"] = None if "engine-not-saved" in self.m else self.speech
        elif s.startswith("mac-settings-ai.provider.") and s.endswith(".key"):
            self.surface, self.section, self.key_focus = "mac-settings-ai", "providers", s.split(".")[2]
        elif s.startswith("mac-settings-ai.provider.") and s.endswith(".connect"):
            self._connect(s.split(".")[2])
        elif s == "mac-settings-ai.agents.name":
            if not self.agent_open:
                return {"ok": False, "error": "nothing registered at %s" % s}
            if "name-door-keeps-key-focus" not in self.m:
                self.key_focus = None
        elif s == "mac-settings-ai.agents.form.link":
            if not self.form:
                return {"ok": False, "error": "nothing registered at %s" % s}
            self.section = "agents" if "form-link-stays" in self.m else "providers"
        elif s in ("mac-settings-ai.nav.providers", "mac-settings-ai.nav.agents", "mac-settings-ai.nav.decisions"):
            to = s.rsplit(".", 1)[1]
            self.section = "providers" if (to == "agents" and "nav-agents-goes-providers" in self.m) else to
        elif s == "mac-settings-ai.agents.save":
            if self.agent_open:
                if "save-keeps-draft-open" not in self.m:
                    self.saved.append({"name": self.agent_name or "Untitled agent", "allowed": list(self.agent_allowed), "builtin": False,
                                       "provider": self.draft_model[0], "model": self.draft_model[1]})
                    self.agent_open = False
        elif s == "mac-settings-ai.agents.duplicate":
            if "duplicate-noop" not in self.m and self.saved:
                src = self.saved[-1]
                self.saved.append(dict(src, name=src["name"] + " copy", builtin=False))
                self.core_["agent_config/%s/allowed" % (src["name"] + " copy")] = list(src["allowed"])
        elif s.startswith("mac-settings-ai.agents.model.") and ".choose." in s:
            n, k = (int(x) for x in s[len("mac-settings-ai.agents.model."):].split(".choose."))
            cards = self._cards()
            choices = self._choices()
            if n > len(cards) or k > len(choices) or (not choices[k - 1][2] and "greyed-entry-clickable" not in self.m):
                return {"ok": False, "error": "nothing registered at %s" % s}
            if "model-choice-not-saved" not in self.m and choices[k - 1][2]:
                cards[n - 1]["provider"], cards[n - 1]["model"] = choices[k - 1][0], choices[k - 1][1]
                if cards[n - 1].get("draft"):
                    self.draft_model = (choices[k - 1][0], choices[k - 1][1])
        elif s == "mac-settings-ai.speech.shortcuts":
            self.sw_pane = "shortcuts"
        elif s.startswith("mac-settings-ai.speech.engine."):
            e = s.rsplit(".", 1)[1]
            if e != "local" and self.providers.get(e, {}).get("state") != "connected":
                return {"ok": False, "error": "nothing registered at %s" % s}
            if "speech-pill-ignores-choice" not in self.m:
                self.speech_engine, self.speech = e, "ready"
        elif s == A["agents_new"]:
            self.section, self.key_focus = "agents", None
            self.agent_open, self.agent_name, self.agent_allowed = True, "", []
            self.form = [] if any(p["state"] == "connected" for p in self.providers.values()) else [
                {"missing": "" if "form-silent" in self.m else "Connect a provider first: this agent needs one.", "link": "mac-settings-ai.provider"}]
        elif s.startswith("mac-settings-ai.agents.allow."):
            perm = ALLOW[s.rsplit(".", 1)[1]]
            if perm in self.agent_allowed and "allow-toggle-stuck" not in self.m:
                self.agent_allowed.remove(perm)
            elif perm not in self.agent_allowed:
                self.agent_allowed.append(perm)
            self._save_agent()
        elif s == "mac-settings-ai.fixture.paste.anthropic-key":
            self.f_paste("anthropic-key")
        elif s == "mac-settings-window.close":
            self.surface = "mac-main"
        elif s == "dock.whoosh" or s == "mac-main.sidebar.accounts-menu.up":
            self.surface = "mac-main"
        elif s == "mac-main.sidebar.accounts-menu.add":
            self.sheet, self.addr_focus = True, False
        elif s == "mac-add-account.address":
            self.addr_focus = self.sheet
        elif s.startswith("mac-search.results.row."):
            self.focus = self.results[int(s.rsplit(".", 1)[1]) - 1]["thread"]
        elif s == A["mcp_start"]:
            self.mcp_on = True
        elif s == A["mcp_allow_claude"]:
            if "grant-not-listed" not in self.m:
                self.grants.add("Claude Desktop")
            self.core_["mcp_grant/Claude Desktop"] = None if "grant-not-saved" in self.m else True
        elif s == A["mcp_revoke_claude"]:
            if "revoke-keeps-access" not in self.m:
                self.grants.discard("Claude Desktop")
            self.core_["mcp_grant/Claude Desktop"] = False
        elif s == A["mcp_prompt_approve"]:
            self.mcp_results = [] if "approve-returns-nothing" in self.m else [{"id": h["id"], "subject": h["subject"]} for h in (self.mcp_pending or [])]
            self.mcp_prompt = [{"state": "approved"}]
        elif s in (KLX + ".download", KLX + ".choose"):
            self._kl_sync()
            if s.endswith(".choose") and self.chosen_model == KL:
                return {"ok": False, "error": "nothing registered at %s" % s}
            if self.kl_state == "ready":
                if s.endswith(".download") and self.chosen_model == KL:
                    return {"ok": False, "error": "nothing registered at %s" % s}
                if "circle-click-ignored" not in self.m:
                    self._use(KL)
                return
            if self.kl_state == "downloading":
                return
            if not self.kl_url:
                self.kl_state, self.kl_why = "failed", "Could not download the decision model: Could not connect to the server. Click Retry."
                return
            self._kl_start()
        elif s == KLX + ".cancel":
            if self.kl_state != "downloading":
                return {"ok": False, "error": "nothing registered at %s" % s}
            self.kl_gen += 1
            self.kl_state = "absent"
            if "cancel-leaves-it-wanted" not in self.m:
                self.wanted_model = None
        elif s == "mac-settings-ai.decisions.local.choose":
            self._laya_sync()
            if self.chosen_model == "laya" or self.laya_state != "ready":
                return {"ok": False, "error": "nothing registered at %s" % s}
            if "circle-click-ignored" not in self.m:
                self._use("laya")
        elif s == "mac-settings-ai.decisions.local.cancel":
            if self.laya_state != "downloading":
                return {"ok": False, "error": "nothing registered at %s" % s}
            if "laya-cancel-ignored" not in self.m:
                self.laya_gen = getattr(self, "laya_gen", 0) + 1       # the running download stops
                self.laya_state, self.laya_bytes, self.laya_seen = "absent", 0, 0
        elif s == "mac-settings-ai.meaning.download":
            if self.meaning_state in ("downloading", "ready"):
                return {"ok": False, "error": "nothing registered at %s" % s}
            if not self.meaning_url:
                self.meaning_state = "failed"
            elif "meaning-click-silent" in self.m:
                self.meaning_state = "notDownloaded"
            else:
                self.meaning_state, self.meaning_got = "downloading", 0
        elif s in ("mac-settings-ai.decisions.local.download", "mac-labels.inspector.laya.download"):
            self._laya_sync()
            if self.laya_state == "ready":
                return {"ok": False, "error": "nothing registered at %s" % s}
            if s.startswith("mac-labels") and "laya-state-not-shared" in self.m:
                self.laya_own = "downloading"          # the inspector keeps a state of its own
                return
            if self.laya_state == "downloading":
                if "laya-second-click-restarts" in self.m:
                    self.laya_bytes = self.laya_seen = 0
                return
            if self.laya_state == "failed" and "laya-failed-no-retry" in self.m:
                return
            if not self.laya_url:                     # no loopback base staged: the rig copy asks RigNet.refusing (127.0.0.1:1), never the real host
                if "laya-download-asks-outside" in self.m:
                    self.peers.append("198.51.100.7:443")      # mutant: RigNet.safe lets the real host through
                self.laya_state, self.laya_why = "failed", "Could not download the decision model: Could not connect to the server. Click Retry."
                return
            bad = ""
            for f in LAYA_FILES:                      # the sizes come first, as the app asks them
                req = urllib.request.Request(self.laya_url + "/" + f, method="HEAD")
                try:
                    urllib.request.urlopen(req, timeout=3).read()
                except urllib.error.HTTPError:
                    bad = f
                    break
            if bad:
                self.laya_state, self.laya_why = "failed", "Could not download the decision model. The download server answered 404 for %s. Click Retry." % bad
            elif "laya-click-silent" in self.m:
                self.laya_state, self.laya_pending = "absent", True
            else:
                self._laya_start()
        elif s == "mac-settings-ai.decisions.jev.consent.allow":
            self.core_["decision_consent/jev"] = True
            self._jev_try()
        elif s == "mac-settings-ai.decisions.jev.consent.withdraw":
            self.core_["decision_consent/jev"] = True if "withdraw-keeps-consent" in self.m else False
        elif s == "mac-settings-ai.decisions.jev.key.field":
            self.surface, self.key_focus = "mac-settings-ai", "jev"
        elif s == "mac-settings-ai.decisions.jev.test":
            return self._jev_test()
        elif s == "mac-settings-ai.decisions.jev.connect":
            key = self.keys.pop("jev", "")
            self.key_focus = None
            if len(key) >= 8:
                self.core_["provider_credential/jev/stored"] = True
                self.core_["decision_model/key"] = key if "jev-key-in-db" in self.m else None
                self.jev_state = "connected" if "saved-shown-as-connected" in self.m else "not checked"
                self.jev_reads = 0      # the provider's check answers a moment later: the second read of the key row sees it (`jev_row_state`)
                self._jev_try()
                self.jev_note = ""
            else:
                self.jev_note = "Jev did not accept that key"
        elif s.startswith("mac-settings-ai.decisions.engine."):
            e = s.rsplit(".", 1)[1]
            if e == "jev" and not self.core_.get("provider_credential/jev/stored") and "jev-without-key-accepted" not in self.m:
                self.jev_wanted, self.engine_note = True, "Add your Jev key first; Jev is not used without one."
                return
            if e == "jev" and not self.core_.get("decision_consent/jev") and "jev-without-consent-accepted" not in self.m and "model-before-consent" not in self.m:
                self.jev_wanted, self.engine_note = True, "Allow sending mail text to Jev first; Jev is not used without it."
                return
            self.jev_wanted, self.engine_note = False, ""
            self.engine = e
            # the core stores the Off choice as the `rules` provider (AISettingsStore.chooseEngine: `case .off: m.provider = .rules`)
            self.core_["decision_model/provider"] = None if ("engine-not-saved" in self.m or "engine-choice-local-only" in self.m) else ("rules" if e == "off" else e)
        elif ".decisions.page." in s and ".entry." in s and s.endswith(".enabled"):
            page, n = s.split(".page.")[1].split(".")[0], int(s.split(".entry.")[1].split(".")[0])
            es = self.lists[page]
            if n >= len(es):      # the closing default has no switch (DecisionEntryRow)
                return {"ok": False, "error": "nothing registered at %s" % s}
            if "untick-ignored" not in self.m:
                es[n - 1]["enabled"] = not es[n - 1].get("enabled", True)
        elif ".decisions.page." in s and ".entry." in s and s.endswith(".question"):
            page, n = s.split(".page.")[1].split(".")[0], int(s.split(".entry.")[1].split(".")[0])
            es = self.lists[page]
            if n > len(es) or es[n - 1]["kind"] != "model":
                return {"ok": False, "error": "nothing registered at %s" % s}
            self.q_focus = es[n - 1]
        elif ".decisions.page." in s and ".move-up" in s:
            page = s.split(".page.")[1].split(".")[0]
            n = int(s.split(".entry.")[1].split(".")[0])
            if "reorder-ignored" not in self.m and n > 1:
                es = self.lists[page]
                es[n - 1], es[n - 2] = es[n - 2], es[n - 1]
                self._renumber(page)
        elif ".decisions.page." in s and s.endswith(".preview"):
            msgs = list(D["messages"].keys())
            n = const("DECISION_PREVIEW_N") - (1 if "preview-n-wrong" in self.m else 0)
            page = s.split(".page.")[1].split(".")[0]
            self.preview = [self._preview_row(page, i) for i in range(n)]
            self.core_["decision_preview/applied"] = True if "preview-writes" in self.m else False
        elif ".decisions.page." in s and s.endswith(".apply"):
            self.core_["decision_preview/applied"] = True
        elif s == A["list_row_1_create_rule"]:
            self.surface = "mac-settings-ai"
            m = MSG[FX["decision"]["prefill"]["msg"]]
            self.editor = [{"from": "" if "prefill-empty" in self.m else m["from"], "domain": "" if "prefill-empty" in self.m else m["from"].split("@")[1], "page": "which-tab"}]
        else:
            return {"ok": False, "error": "nothing registered at %s" % s}

    # ---- helpers: agent / ask
    def _jev_try(self):
        """Jev was chosen first: it is saved once the key and the consent are both there."""
        if self.jev_wanted and self.core_.get("provider_credential/jev/stored") and self.core_.get("decision_consent/jev"):
            self.engine, self.jev_wanted, self.engine_note = "jev", False, ""
            self.core_["decision_model/provider"] = "jev"

    def _cards(self):
        draft = [{"name": self.agent_name, "provider": self.draft_model[0], "model": self.draft_model[1], "draft": True}] if self.agent_open else []
        return draft + self.saved

    def _choices(self):
        """(provider, model, enabled) in the order the menu lists them: connected providers' models, else one greyed entry."""
        out = []
        for p in ("anthropic", "openai", "gemini", "openrouter"):
            pr = self.providers[p]
            if pr["state"] == "connected" and pr["models"]:
                out += [(p, m, True) for m in pr["models"]]
            elif p == "openai" and "menu-lists-unconnected-models" in self.m:
                out.append((p, "gpt-5-mini", True))
            else:
                out.append((p, "", False))
        return out

    def _save_agent(self):
        if "agent-not-saved" in self.m:
            return
        allowed = list(self.agent_allowed) + (["sendWithConfirmation"] if "agent-gets-send" in self.m else [])
        self.core_["agent_config/%s/allowed" % self.agent_name] = allowed

    def _search(self):
        q, self.buf = self.buf, ""
        self.q_shown, self.sel, self.snotice = q, set(), []
        hist = self.core_.setdefault("search_history/recent", [])
        if "history-not-written" not in self.m:
            hist.append(q)
        if "search-calls-provider" in self.m:
            self.core_["provider_request/count"] += 1
        if "ask-panel-opens" in self.m:
            self.surface = "mac-ask"
        if self.hold:
            self.skeleton = 0 if "no-skeleton" in self.m else 5
            self.results = list(self.results) if "results-while-loading" in self.m else []
            if "results-while-loading" in self.m:
                self._show()
            return {"ok": True}
        self._show()
        return {"ok": True}

    def _show(self):
        word, meaning = ["thr-01", "thr-s05"], ["thr-01", "thr-s05", "thr-s06"]
        if "record1-missing" in self.m:
            word, meaning = ["thr-s05"], ["thr-s05", "thr-s06"]
        building = self.index == "building"
        if "meaning-off" in self.m:
            meaning = word
        if "returns-everything" in self.m:
            meaning = sorted({m["thread"] for m in self.corpus})
        ths = word if building and "never-merges" not in self.m else meaning
        if building and "never-merges" in self.m:
            ths = word
        if building and "building-shows-nothing" in self.m:
            ths = []
        self.skeleton = 5 if "skeleton-stays" in self.m else 0
        rows = []
        for th in ths:
            ms = sorted([m for m in self.corpus if m["thread"] == th], key=lambda m: m["date"])
            head = next((m for m in ms if m["id"] in ("msg-01", "msg-s05", "msg-s06", "msg-s08")), ms[0])
            rows.append({"thread": th, "id": head["id"], "account": head["account"], "chips": [], "selected": False})
        self.results = rows
        if building:
            self.snotice = [] if "no-quiet-line" in self.m else [{"text": "Meaning results are coming."}]
        else:
            self.snotice = [{"text": "Meaning results are coming."}] if "quiet-line-stays" in self.m else []
        if "agent-notice" in self.m:
            self.snotice.append({"text": "Configure an agent to get an answer."})

    def _open(self):
        row = next((r for r in self.results if r["thread"] == self.focus), None)
        if row is None:
            return {"ok": False, "error": "nothing to open"}
        if row["id"] in self.deleted:
            if "deleted-silent" not in self.m:
                self.snotice = [{"text": "No longer in your mail."}]
            if "deleted-opens-anyway" in self.m:
                self.surface, self.reader = "mac-reader", row["id"]
            elif "deleted-stays" not in self.m:
                self.results = [r for r in self.results if r is not row]
            return {"ok": True}
        ms = sorted([m for m in self.corpus if m["thread"] == row["thread"]], key=lambda m: m["date"])
        self.surface = "mac-reader"
        self.reader = ms[0]["id"] if "opens-first-message" in self.m else row["id"]
        return {"ok": True}

    def _label_apply(self):
        name, self.lf = self.lf, None
        ths = {r["thread"] for r in self.results if r["thread"] in self.sel}
        msgs = sorted(m["id"] for m in self.corpus if m["thread"] in ths and m["id"] not in self.deleted)
        if "label-thread-head-only" in self.m:
            msgs = sorted({r["id"] for r in self.results if r["thread"] in ths})
        for r in self.results:
            if r["thread"] in ths and not ("chips-only-first" in self.m and r is not self.results[0]):
                r["chips"] = [name]
        self.labels[name] = msgs
        n = len(msgs) if "undo-per-message" in self.m else 1
        self.undo_stack.append((name, msgs))
        self.core_["undo_entry/count"] += n
        self.core_["label_assignment/%s/count" % name] = len(msgs)
        self.core_["label_assignment/%s/messages" % name] = msgs
        self.core_["label/%s/exists" % name] = True
        return {"ok": True}

    def _undo(self):
        if not self.undo_stack:
            return {"ok": False, "error": "Nothing to undo"}
        name, msgs = self.undo_stack.pop()
        keep = msgs[:1] if "undo-one-message-only" in self.m else []
        self.labels[name] = keep
        self.core_["label_assignment/%s/count" % name] = len(keep)
        self.core_["label_assignment/%s/messages" % name] = keep
        if not keep:
            for r in self.results:
                r["chips"] = []
        return {"ok": True}

    def _jev_row_state(self):
        """Jev's key row: Connect saves the key with no network call (DECISION-MODEL.md:63), so the row says saved, not checked, until the Test button
        sends one fixed sample (`_jev_test`)."""
        return getattr(self, "jev_state", "")

    def _jev_test(self):
        """The Test button: one fixed sample to the Jev endpoint (the loopback stub's /v1/systemone) with the saved key; connected on a 200.
        The mutant `jev-check-never-answers` asks nothing, so the row stays saved, not checked."""
        if getattr(self, "jev_state", "") not in ("not checked", "refused"):
            return {"ok": False, "error": "did nothing"}
        if "jev-check-never-answers" not in self.m:
            code, r = post(self.jev_url, "/v1/systemone", jev_body(jev_state_text("sample@example.com", "Whoosh key test", False, ""), "Which tab?"), D["jev_key"])
            self.jev_state = "connected" if code == 200 and jev_answer(r) is not None else "refused"
        return {"ok": True}

    # ---- helpers: decisions
    def _preview_row(self, page, i):
        """Message i would be decided by entry 1 + i % 4 of the shipped list; an entry whose switch is off passes it on to the next one that is on (the closing default has no switch)."""
        es = self.lists[page]
        k = min(1 + i % 4, len(es))
        while k < len(es) and not es[k - 1].get("enabled", True):
            k += 1
        e = es[k - 1]
        if e["kind"] == "rule":
            out, by = self._rule_outcome(page, e), "entry %d · rule" % k
        elif e["kind"] == "model":
            out, by = "Promotions", "entry %d · the model said Promotions (0.80)" % k
        else:
            out, by = self._default_outcome(page, e), "entry %d · otherwise" % k
        return {"message": "msg-%02d" % (i + 1), "entry": None if "preview-no-entry" in self.m else k, "outcome": "" if "preview-row-without-tab" in self.m else out,
                "decided_by": by, "differs": i % 3 == 0}

    def _renumber(self, page):
        for i, e in enumerate(self.lists[page], 1):
            e["n"] = i

    def _rule_match(self, page, e, msg):
        w = e["word"]
        if e.get("custom"):
            return w.split("contains ", 1)[1].lower() in msg["subject"].lower()
        if w == "VIP" or w == "VIP list":
            return msg["from"] in self.vips
        if w == "list header":
            return msg["list_header"]
        if w == "from me":
            return msg["from_me"]
        if w == "code":
            return any(k in msg["subject"].lower() for k in ("code", "verification", "sign in"))
        if w == "needs my reply":
            return msg["needs_reply"]
        return False

    def _rule_outcome(self, page, e):
        if e.get("custom"):
            return e["outcome"]
        return {"which-tab": {"VIP": "Primary", "list header": "Updates"}, "needs-reply": {"from me": "no", "list header": "no"},
                "sign-in-codes": {"code": "yes"}, "vips": {"VIP list": "yes"},
                "notify-me": {"VIP": "notify", "needs my reply": "notify", "list header": "silent"}}[page][e["word"]]

    def _default_outcome(self, page, e):
        return {"Primary": "Primary", "no": "no", "silent": "silent", "none": "none"}[e["word"]]

    def _jev(self, msg, question):
        text = "x" * (msg["body_len"] if "body-not-truncated" in self.m else min(msg["body_len"], const("DECISION_BODY_CHARS")))
        body = jev_body(jev_state_text(msg["from"], msg["subject"], msg["list_header"], text), question)
        if "key-in-body" in self.m:
            body["api_key"] = D["jev_key"]
        if "attachments-sent" in self.m:
            body["attachments"] = ["file.pdf"]
        code, r = post(self.jev_url, "/v1/systemone", body, D["jev_key"])
        return jev_answer(r)

    def _decide(self, page, msgk, label, conf):
        msg = D["messages"][msgk]
        title = next(p["title"] for p in D["pages"] if p["id"] == page)
        called, res = "model-asked-anyway" in self.m, None
        for i, e in enumerate(self.lists[page], 1):
            hit = None
            if e["kind"] == "rule" and self._rule_match(page, e, msg):
                hit = (self._rule_outcome(page, e), "%s · entry %d · %s" % (title, i, e["word"]))
            elif e["kind"] == "model":
                if self.engine == "off" and "off-still-asks" not in self.m:
                    continue
                if self.engine == "jev" and not self.jev_consent and "model-before-consent" not in self.m:
                    continue
                if self.engine == "jev":
                    label, conf = self._jev(msg, e.get("question", "Which tab?"))
                called = True
                trust = e["trust"] / 100.0
                if "trust-ignored" in self.m or (conf > trust if "trust-exclusive" in self.m else conf >= trust):
                    out = TAB_TITLE.get(label, label.title()) if page == "which-tab" else label
                    hit = (out, ("%s · the model said %s (%.2f)" % (title, out, conf)) if "why-without-entry" in self.m else "%s · entry %d · the model said %s (%.2f)" % (title, i, out, conf))
            elif e["kind"] == "default":
                hit = (self._default_outcome(page, e), "%s · entry %d · otherwise %s" % (title, i, e["word"]))
            if hit:
                res = {"page": page, "entry": i, "outcome": hit[0], "why": hit[1], "model_called": called}
                if "first-match-ignored" not in self.m:
                    break
        self.last = [res]

    def _model_line(self):
        """What every state of the model's line also says (pick M1): which model it is, its facts as drawn, and that it is the one in use."""
        facts = model_facts_wanted()
        if "model-line-without-its-score" in self.m:
            facts = facts.rsplit(" \u00b7 ", 1)[0]
        line = {"model": "laya", "name": "Laya", "facts": facts, "in_use": self.chosen_model == "laya" and "model-line-not-marked-in-use" not in self.m,
                "wanted": self.wanted_model == "laya"}
        # only a model that is on this Mac sorts mail, so only its line says "in use until"
        here = self.laya_state == "ready" or "until-line-when-absent" in self.m
        if self.chosen_model == "laya" and self.wanted_model == KL and here and "until-line-missing" not in self.m:
            line["until"] = "In use until KnowLine 0.8B is ready."
        return line

    def _laya_rows(self):
        self._laya_sync()
        if self.laya_pending:
            self.laya_pending = False
            self._laya_start()
            return [{"state": "absent", "words": "Not downloaded.", "download": True}]
        if self.laya_state == "downloading":
            got = self.laya_seen
            return [{"state": "downloading", "received": got, "total": 5 * LAYA_SIZE, "words": "Downloading %d KB of 843 MB" % (got // 1000), "download": False,
                     "cancel": "laya-no-cancel" not in self.m, "key": "esc"}]
        if self.laya_state == "failed":
            return [{"state": "failed", "words": self.laya_why, "download": True, "retry": "laya-failed-no-retry" not in self.m, "button": "Retry", "key": "\u21a9"}]
        if self.laya_state == "ready":
            return [{"state": "ready", "words": "", "download": False}]
        return [{"state": "absent", "words": "" if "laya-absent-silent" in self.m else "Not on this Mac yet. 843 MB, downloaded once.", "download": True,
                 "button": "Download", "key": "" if "laya-button-no-key" in self.m else "\u21a9"}]

    # ---- rows
    def rows_(self, ax):
        if ax in ("mac-settings-ai.decisions.lines", "mac-settings-ai.decisions.other") or ax.startswith("mac-settings-ai.decisions.added."):
            return self._door_rows(ax)
        if ax == "mac-settings-ai.decisions.local" and "laya-first-read-slow" in self.m and self.laya_state == "downloading":
            time.sleep(0.08)      # mutant: the row takes longer than one frame to answer
        P = "mac-settings-ai.provider."
        if ax == "mac-settings-ai.section":
            return [{"section": self.section}]
        if ax == "mac-settings-ai.decisions.jev.consent":
            return [{"given": bool(self.core_.get("decision_consent/jev")), "refusal": self.engine_note if self.jev_wanted else ""}]
        if ax == "mac-settings-ai.decisions.jev.key":
            st = self._jev_row_state()
            return [{"connected": bool(self.core_.get("provider_credential/jev/stored")), "state": st, "shown": {"connected": "connected", "not checked": "Saved, not checked"}.get(st, "not connected") if "raw-word-printed" not in self.m else (st or "not connected")}]
        if ax == "mac-settings-ai.decisions.local":
            return [dict(r, **self._model_line()) for r in self._laya_rows()]
        if ax == KLX:
            return [self._kl_row()]
        if ax == "mac-settings-ai.decisions.models":
            lines = [dict(r, **self._model_line()) for r in self._laya_rows()]
            lines = lines if "second-model-line-missing" in self.m else lines + [self._kl_row()]
            return lines + [self._added_row(k, a) for k, a in enumerate(self.added, 1)]
        if ax == "mac-settings-ai.meaning":
            st = self.meaning_state
            if st == "downloading":
                got = self.meaning_got
                self.meaning_got += 400000000
                if self.meaning_got >= 1200000000:
                    self.meaning_state = "ready"
                return [{"ax": ax, "state": "downloading", "words": "Downloading.", "button": "", "received": got, "total": 1525000000}]
            if st == "failed":
                return [{"ax": ax, "state": "failed", "words": "This test copy downloads only from a loopback address (WHOOSH_RIG_EMBED_BASE). Nothing was asked.",
                         "button": "" if "meaning-failed-no-retry" in self.m else "Retry", "button_ax": "mac-settings-ai.meaning.download"}]
            if st == "ready":
                return [{"ax": ax, "state": "ready", "words": "Ready on this Mac.", "button": "", "bytes": 1525000000}]
            return [{"ax": ax, "state": "notDownloaded", "words": "The search model is not on this Mac.", "button": "" if "meaning-absent-no-button" in self.m else "Download",
                     "button_ax": "mac-settings-ai.meaning.download"}]
        if ax == "mac-settings-ai.decisions.local.next":
            return [] if "laya-line-missing" in self.m or self.engine != "local" or self.laya_state != "ready" else [{"key": "R", "text": "New mail is sorted as it arrives. Mail already in the inbox is sorted when you press R on a label."}]
        if ax == "mac-settings-ai.agents.new.row":
            return [{"title": "New agent" if "row-title-repeats-button" in self.m else "Your agents", "button": "New agent"}]
        if ax == "mac-settings-ai.speech.line":
            return [{"key": "" if "speech-line-no-key" in self.m else "Fn", "link": False}]
        if ax == "mac-settings-ai.speech.link":
            return [{"pane": self.sw_pane}]
        if ax == "mac-settings-ai.speech.engines":
            ids = ["local"] + [p for p in ("anthropic", "openai", "gemini", "openrouter") if self.providers[p]["state"] == "connected" or "speech-offers-unconnected" in self.m]
            return [{"engine": e, "chosen": e == self.speech_engine} for e in ids]
        if ax.startswith("mac-settings-ai.agents.model."):
            n = int(ax.rsplit(".", 1)[1])
            cards = self._cards()
            if n > len(cards):
                return []
            c = cards[n - 1]
            return [{"provider": p, "model": m, "enabled": en, "current": en and (p, m) == (c["provider"], c["model"]), "note": "" if en else "Add a key above"} for p, m, en in self._choices()]
        if ax.startswith(P) and ax.endswith(".check"):
            n = ax[len(P):-6]
            return [{"state": "checking" if n in self.pending and self.pending[n].is_alive() else "idle"}]
        if ax.startswith(P) and ax.endswith(".key"):
            n = ax[len(P):-4]
            return [{"length": len(self.keys.get(n, "")), "focused": self.key_focus == n}]
        if ax.startswith(P):
            self._settle()
            rest = ax[len(P):]
            if rest.endswith(".models"):
                return [{"model": m} for m in self.providers[rest[:-7]]["models"]]
            return [dict(self.providers[rest])]
        if ax == A["speech"]:
            return [{"state": self.speech, "engine": self.speech_engine, "title": "Speech" if "speech-card-titled-speech" in self.m else "Dictation", "row_title": "Engine"}]
        if ax == "mac-settings-ai.speech.retry":
            return self.speech_retry
        if ax == A["agents_form"]:
            return self.form
        if ax == "mac-add-account.address":
            if not self.sheet:
                raise KeyError(ax)
            return [{"text": self.addr}]
        if ax == A["agents_card"]:
            if "rows-read-eats-typed" in self.m and self.surface != "mac-settings-ai" and self.agent_open and self.addr:
                self.agent_name, self.addr = self.agent_name + self.addr, ""      # a probe read takes what another surface was typed
            allowed = self.agent_allowed + (["sendWithConfirmation"] if "agent-gets-send" in self.m else [])
            draft = [{"name": self.agent_name, "allowed": allowed if "card-codes" in self.m else [WORDS[a] for a in allowed], "editing": True,
                      "provider": self.draft_model[0], "model": self.draft_model[1], "refusal": ""}] if self.agent_open else []
            return draft + [{"name": c["name"], "allowed": [WORDS.get(a, a) for a in c["allowed"]], "editing": False,
                             "provider": c["provider"], "model": c["model"], "refusal": ""} for c in self.saved]
        if ax == "mac-main.reader.subject":
            return [{"id": self.reader, "subject": next(m["subject"] for m in self.corpus if m["id"] == self.reader)}]
        if ax == A["search_results"]:
            return [dict(r, ax="mac-search.results.row.%d" % i, selected=r["thread"] in self.sel) for i, r in enumerate(self.results, 1)]
        if ax == A["search_skeleton"]:
            return [{"ax": "mac-search.skeleton.%d" % i} for i in range(1, self.skeleton + 1)]
        if ax == A["search_notice"]:
            return list(self.snotice)
        if ax == A["search_label_field"]:
            if self.lf is None:
                raise KeyError(ax)
            return [{"name": self.lf, "offers_create": False if "no-create-offer" in self.m else not self.core_.get("label/%s/exists" % self.lf)}]
        if ax == A["search_undo"]:
            return [] if ("no-undo-line" in self.m or not self.undo_stack) else [{"text": "Labelled. Z to undo."}]
        if ax == "mac-search.answer" and "answer-line" in self.m:
            return [{"text": "Suppliers quoted 0.84 EUR per unit."}]
        if ax == "mac-search.proposal" and "proposal-row" in self.m:
            return [{"label": "Supplier quotes", "approve": "mac-search.proposal.approve"}]
        if ax == A["mcp_server"]:
            return [{"state": "on", "line": ""}] if self.mcp_on else [{"state": "off", "line": "The local MCP server is off.", "start": "" if "no-start-control" in self.m else A["mcp_start"]}]
        if ax == A["mcp_apps"]:
            return [{"app": a, "state": "allowed" if a in self.grants else "not allowed"} for a in ("Claude Desktop", "Cursor")]
        if ax == A["mcp_prompt"]:
            return self.mcp_prompt
        if ax == A["mcp_results"]:
            return self.mcp_results
        if ax == A["dec_pages"]:
            return [{"id": "rules", "title": "Rules"}, {"id": "decision-model", "title": "Decision model"}] if "pages-by-method" in self.m else list(D["pages"])
        if ax == A["dec_engine"]:
            return [{"id": e, "state": self.dec_ready[e], "chosen": (e == "jev" and self.jev_wanted) or (e == self.engine and not self.jev_wanted),
                     "refusal": self.engine_note if (e == "jev" and self.jev_wanted) else ""} for e in (("local", "jev") if "engine-no-off" in self.m else ("local", "jev", "off"))]
        if ax == A["dec_result"]:
            return [r for r in self.last if r]
        if ax == A["dec_consent"]:
            return [{"provider": "jev", "state": "given" if self.jev_consent else "asked"}]
        if ax == A["dec_rule_editor"]:
            return self.editor
        if ax.startswith("mac-settings-ai.decisions.page."):
            page, rest = ax[len("mac-settings-ai.decisions.page."):].split(".", 1)
            if rest == "entries":
                return [{"n": e["n"] if "n" in e else i, "kind": e["kind"], "text": "" if "model-text-empty" in self.m and e["kind"] == "model" else (e.get("question") or e["word"]), "trust": e.get("trust"),
                         "enabled": e.get("enabled", True)} for i, e in enumerate(self.lists[page], 1)]
            if rest == "preview":
                return self.preview
            if rest == "teaching":
                offers = []
                c = self.corr
                if c and (c["who"] != "mixed" or "make-rule-for-mixed" in self.m):
                    offers.append("make-rule")
                limit = D["teaching"]["local_refit_at"] - (1 if "refit-at-34" in self.m else 0)
                if self.engine == "local" and self.examples >= limit or (self.engine == "jev" and "refit-for-jev" in self.m and self.examples >= limit):
                    offers.append("refit")
                return [{"count": "%d corrections" % c.get("n", 0) if c else "0 corrections", "offers": offers}]
        raise KeyError(ax)


class _Handler(socketserver.StreamRequestHandler):
    def handle(self):
        for line in self.rfile:
            req = json.loads(line.decode())
            reply = self.server.app.handle(req["verb"], req.get("args", []))
            self.wfile.write((json.dumps(reply) + "\n").encode())


class FakeServer:
    def __init__(self, mutants=()):
        self.dir = tempfile.mkdtemp(prefix="wj5", dir="/tmp")
        self.srv = socketserver.ThreadingUnixStreamServer(os.path.join(self.dir, "whoosh.sock"), _Handler)
        self.srv.app = FakeApp(mutants)
        self.srv.app.models_dir = self.dir
        threading.Thread(target=self.srv.serve_forever, daemon=True).start()
        NETPROBE["fn"] = lambda ctl: self.srv.app.lsof_text()
        RELAUNCH["fn"] = self.relaunch      # the cases that relaunch the copy do it through this stand-in on a self-test, never through launch-one.sh

    def relaunch(self, ctl):
        ctl.close()
        self.srv.app.relaunch()
        ctl.connect()

    def close(self):
        self.srv.shutdown()
        self.srv.server_close()
        shutil.rmtree(self.dir, ignore_errors=True)


# ---------------------------------------------------------------- main
def mutants_table(suite, observed):
    lines = ["MUTANTS", "| check | mutant | mutation | observed |", "|---|---|---|---|"]
    for label, _fn, m, d in suite.items:
        lines.append("| %s | %s | %s | %s |" % (label, m, d, observed.get(label, "declared; run --selftest to see it red")))
    return "\n".join(lines)


def _record(suite, observed):
    try:
        os.makedirs(CAPDIR, exist_ok=True)
        with open(os.path.join(CAPDIR, "%s-mutants.json" % suite.name), "w", encoding="utf-8") as f:
            json.dump(observed, f, indent=1)
    except OSError:
        pass


def _recorded(suite):
    try:
        return json.load(open(os.path.join(CAPDIR, "%s-mutants.json" % suite.name), encoding="utf-8"))
    except (OSError, ValueError):
        return {}


def main(suite, argv=None):
    argv = sys.argv[1:] if argv is None else argv
    env = {"prov": Stub("provider"), "jev": Stub("jev"), "laya": Stub("laya"), "hub": HubStub()}
    try:
        if "--selftest" in argv:
            return selftest(suite, env)
        try:
            ctl = Ctl().connect()
        except CouldNotDrive as e:
            res = [(l, "unjudged", str(e)) for l, *_ in suite.items]
        else:
            try:
                reset_all = lambda: reset(ctl, env)
                for it in suite.items:  # each check starts from a reset app
                    pass
                res = []
                for item in suite.items:
                    reset_all()
                    s2 = Suite(suite.name)
                    s2.items = [item]
                    res += run_suite(s2, ctl, env)
            finally:
                ctl.close()
        n, f, u = len(res), sum(r[1] == "fail" for r in res), sum(r[1] == "unjudged" for r in res)
        for l, st, d in res:
            if st == "excluded":
                print("ok %s excluded, %s" % (l, d))   # never counted as not judged: judged on the engine its words name
            elif st != "pass":
                sys.stderr.write("%s %s: %s\n" % (st.upper(), l, d))
            else:
                print(("PASS %s: %s" % (l, " ".join(str(d).split())[:200])).rstrip(": "))   # ONE line per passing case
        print("%d checks, %d failed, %d not judged" % (n, f, u))
        print(mutants_table(suite, _recorded(suite)))
        return 1 if f or u else 0
    finally:
        env["prov"].close()
        env["jev"].close()
        env["laya"].close()
        env["hub"].close()


def _run_against(suite, env, mutants, only=None):
    """Every check against the stand-in with `mutants` on; `only` (a mutant name) runs just the checks that declare it (the mutant run reads nothing else:
    running all 63 per mutant made the self-test longer than the 300 s a run is given, since the Laya cases wait on a slow stub)."""
    srv = FakeServer(mutants)
    ctl = Ctl(srv.dir).connect()
    try:
        res = []
        for item in suite.items:
            if only is not None and item[2] != only:
                continue
            reset(ctl, env)
            s2 = Suite(suite.name)
            s2.items = [item]
            res += run_suite(s2, ctl, env)
        return res
    finally:
        ctl.close()
        srv.close()


def selftest(suite, env):
    clean = _run_against(suite, env, ())
    n, f, u = len(clean), sum(r[1] == "fail" for r in clean), sum(r[1] == "unjudged" for r in clean)
    print("%d checks, %d failed, %d not judged" % (n, f, u))
    for l, st, d in clean:
        if st != "pass":
            sys.stderr.write("CLEAN-RUN %s %s: %s\n" % (st.upper(), l, d))
    observed, bad = {}, 0
    for m in sorted({m for _l, _f, m, _d in suite.items}):
        res = {l: (st, d) for l, st, d in _run_against(suite, env, (m,), only=m)}
        for l, _fn, mm, _d in suite.items:
            if mm != m:
                continue
            st, d = res[l]
            observed[l] = "RED on this mutant: %s" % d if st == "fail" else "NOT RED (%s)" % st
            bad += st != "fail"
    print(mutants_table(suite, observed))
    _record(suite, observed)
    no_ev = [l for l in EVIDENCE if l in {i[0] for i in suite.items} and "| evidence: " not in observed.get(l, "")]
    unj = Suite(suite.name)
    unj.items = [(l, (lambda ctl, env: (_ for _ in ()).throw(Unreadable("a read the case could not make"))), "x", "x") for l in EVIDENCE if l in {i[0] for i in suite.items}]
    srv = FakeServer(())
    uctl = Ctl(srv.dir).connect()
    try:
        un = run_suite(unj, uctl, env)
    finally:
        uctl.close()
        srv.close()
    no_ev_unj = [l for l, st, d in un if st != "unjudged" or "| evidence: " not in d]
    print("evidence on red: %d of %d evidence cases print an `evidence:` part on their mutant (missing: %s); on a case that cannot be judged: %d of %d (missing: %s)" % (
        len([l for l in EVIDENCE if l in {i[0] for i in suite.items}]) - len(no_ev), len([l for l in EVIDENCE if l in {i[0] for i in suite.items}]), no_ev or "none",
        len(un) - len(no_ev_unj), len(un), no_ev_unj or "none"))
    # the search model case on a copy that ignores the staged address (what main77 read): not judged, in the row's own words, with the stub's request count
    item = next((i for i in suite.items if i[0] == "J5.9/meaning-downloads-with-moving-figures-then-is-ready"), None)
    meaning_ok = True
    if item is not None:      # only the suite that holds the case (a sibling suite loads this harness too)
        srv = FakeServer(("meaning-address-ignored",))
        mctl = Ctl(srv.dir).connect()
        try:
            reset(mctl, env)
            one = Suite(suite.name)
            one.items = [item]
            (ml, mst, md), = run_suite(one, mctl, env)
        finally:
            mctl.close()
            srv.close()
        meaning_ok = mst == "unjudged" and "reads 'failed' right after the click" in md and "WHOOSH_RIG_EMBED_BASE" in md and "the stub was asked {'HEAD': 0, 'GET': 0" in md and "| evidence: " in md
        print("search model on a copy that ignores the staged address: %s: %s" % (mst.upper(), md))
        print("search model case says what it read, names the row's own words and the stub's request counts: %s" % ("OK" if meaning_ok else "FAILED"))
    # the index cases on a build that does not serve `fixture index-progress` (finding F1 of gap-search-cases): NOT JUDGED naming the verb and the file, never a pass
    index_ok = True
    for lab in ("J5.9/meaning-index-line-gives-the-figures-while-building", "J5.9/meaning-index-line-says-how-many-mails-are-indexed-when-the-build-is-done",
                "J5.9/status-line-says-the-share-while-the-index-builds"):
        item = next((i for i in suite.items if i[0] == lab), None)
        if item is None:
            continue
        srv = FakeServer(("index-progress-not-served",))
        ictl = Ctl(srv.dir).connect()
        try:
            reset(ictl, env)
            one = Suite(suite.name)
            one.items = [item]
            (il, ist, idt), = run_suite(one, ictl, env)
        finally:
            ictl.close()
            srv.close()
        good = ist == "unjudged" and "fixture index-progress" in idt and "FixtureVerbs.swift" in idt
        index_ok = index_ok and good
        print("index case on a build without `fixture index-progress`: %s %s: %s" % (il, ist.upper(), idt[:160]))
    print("index cases say which verb and which file are missing, and are never a pass: %s" % ("OK" if index_ok else "FAILED"))
    # round 16: a case that can never be judged on the test engine is excluded there (its own `ok ... excluded` line), never counted as not judged
    excl_ok = True
    item = next((i for i in suite.items if i[0] == "J5.7/laya-download-started-in-the-inspector-shows-in-the-pane"), None)
    if item is not None:
        srv = FakeServer(("core-fixture",))
        xctl = Ctl(srv.dir).connect()
        try:
            reset(xctl, env)
            one = Suite(suite.name)
            one.items = [item]
            (xl, xst, xd), = run_suite(one, xctl, env)
        finally:
            xctl.close()
            srv.close()
        excl_ok = xst == "excluded" and xd.startswith("real engine only: ")
        print("inspector case on the test engine: %s: ok %s excluded, %s" % ("OK" if excl_ok else "WRONG (%s)" % xst, xl, xd[:120]))
    bad += len(no_ev) + len(no_ev_unj) + (0 if meaning_ok else 1) + (0 if index_ok else 1) + (0 if excl_ok else 1)
    ok = f == 0 and u == 0 and bad == 0
    print("selftest: %s (%d mutants not red)" % ("OK" if ok else "FAILED", bad))
    return 0 if ok else 1


def load_harness():
    """For the sibling checks: this module under an importable name."""
    spec = importlib.util.spec_from_file_location("j5harness", os.path.join(ROOT, "checks", "settingsaimac-check.py"))
    mod = importlib.util.module_from_spec(spec)
    sys.modules["j5harness"] = mod
    spec.loader.exec_module(mod)
    return mod


# ---------------------------------------------------------------- the checks (J5.1-J5.3)
S = Suite("settingsaimac")
AP = A["providers"]
JEV_CHECK_WAIT = 4.0      # seconds the provider's check may take to answer on the loopback stub (a bound, never a sleep that waits it out)


def _open(ctl):
    act(ctl, "click", A["palette_ai_settings"])


def _paste_landed(ctl):
    """`fixture paste anthropic-key` and wait for the answer: the core lands it by its own clock (a move of 1 s), the pane's row read waits for it."""
    act(ctl, "fixture", "paste", "anthropic-key")
    act(ctl, "clock", "advance", "1")
    return rows(ctl, AP + ".anthropic")[0]["state"]


@S.check("J5.1/arrives-by-palette", "no-ai-entry", "the palette has no AI settings entry")
def c_arrive(ctl, env):
    soft(ctl, "click", A["palette_ai_settings"])
    return eq(ctl.uistate().get("surface"), "mac-settings-ai")


@S.check("J5.1/key-connects", "key-never-connects", "a valid pasted key never connects")
def c_connect(ctl, env):
    _open(ctl)
    _paste_landed(ctl)
    ok1, d1 = eq(rows(ctl, AP + ".anthropic")[0]["state"], "connected")
    ok2, d2 = eq(rows(ctl, AP + ".openai")[0]["state"], "not connected")  # control: a provider nobody gave a key stays off
    return ok1 and ok2, "anthropic: %s; openai control: %s" % (d1, d2)


@S.check("J5.1/models-listed", "no-models", "the connected provider lists no models")
def c_models(ctl, env):
    _open(ctl)
    _paste_landed(ctl)
    got = [r["model"] for r in rows(ctl, AP + ".anthropic.models")]
    ctrl = rows(ctl, AP + ".openai.models")
    return got == ["claude-sonnet-5-5", "claude-haiku-5-5"] and ctrl == [], "models %r, unconnected provider %r" % (got, ctrl)


@S.check("J5.1/refused-key-reason-in-words", "reason-code-only", "a refused key shows a bare code")
def c_refused(ctl, env):
    _open(ctl)
    act(ctl, "fixture", "paste", "bad-key")
    r = rows(ctl, AP + ".anthropic")[0]
    ok = r["state"] == "refused" and r["note"] == "This key is not valid for the provider."
    return ok, "row %r" % r


@S.check("J5.1/chatgpt-shows-connected", "chatgpt-row-never-connected", "after a ChatGPT sign-in the stand-in approved, the row still says not connected")
def c_chatgpt_connected(ctl, env):
    """Item 348b2ea0 (round 9): `fixture chatgpt approve <stub>` runs the ChatGPT sign-in route on this test copy against the loopback stub (its token
    address `/oauth/token`; nothing leaves this Mac); the row `mac-settings-ai.provider.chatgpt` says connected, and not connected again after a reset.
    Controls: the row starts not connected; the stub was asked for the token once."""
    _open(ctl)
    before = rows(ctl, AP + ".chatgpt")[0]["state"]
    act(ctl, "fixture", "chatgpt", "approve", env["prov"].url)
    state, end = before, time.monotonic() + 4
    while state != "connected" and time.monotonic() < end:
        time.sleep(0.1)
        state = rows(ctl, AP + ".chatgpt")[0]["state"]
    asked = [r for r in env["prov"].requests if r["path"] == "/oauth/token"]
    reset(ctl, env)
    _open(ctl)
    after = rows(ctl, AP + ".chatgpt")[0]["state"]
    return (before, state, len(asked), after) == ("not connected", "connected", 1, "not connected"), \
        "row %r, after the sign-in %r (the stub was asked for a token %d time(s)), after a reset %r" % (before, state, len(asked), after)


@S.check("J5.1/chatgpt-stop-note", "no-chatgpt-note", "the ChatGPT option loses its warning")
def c_chatgpt(ctl, env):
    _open(ctl)
    note = rows(ctl, AP + ".chatgpt")[0]["note"]
    return FX["providers"]["chatgpt_note_contains"] in note, "note %r" % note


@S.check("J5.1/key-in-keychain-not-db", "key-in-db", "the key is stored in the database")
def c_keychain(ctl, env):
    _open(ctl)
    _paste_landed(ctl)
    got = (core(ctl, "provider_credential/anthropic/stored"), core_or_none(ctl, "ai_provider/anthropic/key"))
    return eq(got, (True, None))


@S.check("J5.1/key-validated-against-provider", "key-not-validated", "the key is accepted without asking the provider")
def c_validated(ctl, env):
    _open(ctl)
    env["prov"].requests.clear()
    _paste_landed(ctl)
    got = [(r["path"], r["auth"]) for r in env["prov"].requests]
    return eq(got, [("/validate", "Bearer " + FX["providers"]["anthropic"]["key"])])


@S.check("J5.2/engine-ready", "speech-never-ready", "the local speech engine never reports ready")
def c_speech(ctl, env):
    _open(ctl)
    act(ctl, "click", A["speech_local"])
    return eq(rows(ctl, A["speech"])[0]["state"], "ready")


@S.check("J5.2/engine-in-core", "engine-not-saved", "the speech choice is not saved")
def c_speech_core(ctl, env):
    _open(ctl)
    act(ctl, "click", A["speech_local"])
    return eq(core(ctl, "speech_engine/chosen/state"), "ready")


@S.check("J5.2/download-fails-retry-with-reason", "retry-without-reason", "Retry shows no reason")
def c_speech_fail(ctl, env):
    _open(ctl)
    act(ctl, "fixture", "speech-fail")
    act(ctl, "click", A["speech_local"])
    st = rows(ctl, A["speech"])[0]["state"]
    rt = rows(ctl, "mac-settings-ai.speech.retry")
    return st == "failed" and [r["reason"] for r in rt] == ["The download stopped: the network dropped."], "state %r, retry %r" % (st, rt)


def _new_agent(ctl):
    _open(ctl)
    _paste_landed(ctl)
    act(ctl, "click", A["agents_new"])
    act(ctl, "type", FX["agent"]["name"])
    for w in ("read", "search", "label"):
        act(ctl, "click", "mac-settings-ai.agents.allow." + w)


@S.check("J5.3/card-lists-actions-in-words", "card-codes", "the card shows codes, not words")
def c_card(ctl, env):
    _new_agent(ctl)
    return eq(rows(ctl, A["agents_card"])[0]["allowed"], FX["agent"]["words"])


@S.check("J5.3/config-in-core", "agent-not-saved", "the agent is not saved")
def c_agent_core(ctl, env):
    _new_agent(ctl)
    return eq(core(ctl, "agent_config/%s/allowed" % FX["agent"]["name"]), FX["agent"]["allow"])


@S.check("J5.3/no-send-permission", "agent-gets-send", "the agent gets a send permission nobody gave")
def c_nosend(ctl, env):
    _new_agent(ctl)
    allowed = core(ctl, "agent_config/%s/allowed" % FX["agent"]["name"])
    card = rows(ctl, A["agents_card"])[0]["allowed"]
    # positive control: the same probe sees the three permissions that were given
    ok = allowed is not None and "label" in allowed and FX["agent"]["no_send"] not in allowed and "send with your confirmation" not in card
    return ok, "core %r, card %r" % (allowed, card)


SHEET_PATH = ("dock.whoosh", "mac-main.sidebar.accounts-menu.up", "mac-main.sidebar.accounts-menu.add", "mac-add-account.address")
TYPED_PROBE = "probe@nothing.example.test"


def _draft_then_sheet(ctl):
    """A draft agent is open in the AI pane, Settings closes, the add-account sheet opens with its address field focused.
    The key lands first (`_paste_landed`: the core answers it on its own clock, 1 s; main76 clicked the paste door and went on, so no provider was
    connected yet, `newAgent` (AISettingsStore.swift:250-255) found none and opened no draft: the first card read was the built-in 'Ask my inbox').
    A draft that is not open here is the setup failing, so the case is NOT JUDGED naming it, never a verdict on the typing."""
    _open(ctl)
    _paste_landed(ctl)
    act(ctl, "click", A["agents_new"])
    if _draft_card(ctl) is None:
        raise Unreadable("no draft agent card is open after the new-agent click (cards read: %r): the case cannot start" % [(c.get("name"), c.get("editing")) for c in rows(ctl, A["agents_card"])])
    act(ctl, "click", "mac-settings-window.close")
    for ax in SHEET_PATH:
        act(ctl, "click", ax)


def _draft_card(ctl):
    """The open draft's card (`editing` true), None when no draft is open. The draft is listed first, but the built-ins are cards too: a read of the
    first card names a built-in whenever the draft is gone (a draft's own name starts empty, AISettingsStore.swift:254)."""
    return next((c for c in rows(ctl, A["agents_card"]) if c.get("editing")), None)


@S.check("J5.3/typing-after-settings-closes-reaches-the-next-field", "closed-ai-pane-keeps-taking-the-typed-text",
         "a draft agent stays open after Settings closes and takes the text typed in the next surface")
def c_typing_after_close(ctl, env):
    _draft_then_sheet(ctl)
    act(ctl, "type", TYPED_PROBE)
    rows_ = rows(ctl, "mac-add-account.address")
    text = rows_[0].get("text") if rows_ else None
    card = _draft_card(ctl)
    name = card["name"] if card else None          # None: the draft is gone (red), never a built-in card's name
    soft(ctl, "key", "Escape")
    return (text, name) == (TYPED_PROBE, ""), "the address field holds %r, the draft's name %r" % (text, name)


@S.check("J5.3/rows-read-does-not-eat-typed", "rows-read-eats-typed",
         "reading the agent card rows takes the text another surface was typed")
def c_rows_pure(ctl, env):
    _draft_then_sheet(ctl)
    rows(ctl, A["agents_card"])
    act(ctl, "type", TYPED_PROBE)
    first = (_draft_card(ctl) or {}).get("name")
    second = (_draft_card(ctl) or {}).get("name")
    rows_ = rows(ctl, "mac-add-account.address")
    text = rows_[0].get("text") if rows_ else None
    soft(ctl, "key", "Escape")
    return (text, first, second) == (TYPED_PROBE, "", ""), "the address field holds %r after two row reads naming %r then %r" % (text, first, second)


@S.check("J5.3/no-provider-form-names-it", "form-silent", "with no provider the form does not say so")
def c_noprov(ctl, env):
    _open(ctl)
    act(ctl, "click", A["agents_new"])
    f = rows(ctl, A["agents_form"])
    ok = len(f) == 1 and "provider" in f[0]["missing"].lower() and f[0]["link"] == "mac-settings-ai.provider"
    return ok, "form %r" % f


# ---------------------------------------------------------------- round 4: every control of the pane has a click door and a visible outcome
PK = "mac-settings-ai.provider."
NAV = "mac-settings-ai.section"
NEW_KEY = FX["providers"]["anthropic"]["key"]


def _key_door(ctl, name, key):
    """The key field and the Connect button, by their click doors: focus the field, type, press Connect."""
    _open(ctl)
    act(ctl, "click", PK + name + ".key")
    act(ctl, "type", key)


@S.check("J5.4/key-field-door-takes-typed-text", "typed-key-lost", "the key field door is clicked, the typed key never reaches it")
def c_keyfield(ctl, env):
    _key_door(ctl, "anthropic", NEW_KEY)
    r = rows(ctl, PK + "anthropic.key")[0]
    return eq((r["length"], r["focused"]), (len(NEW_KEY), True))


@S.check("J5.4/connect-door-sends-the-typed-key", "connect-ignores-key", "Connect asks the provider with an empty key, not the typed one")
def c_connect_door(ctl, env):
    _key_door(ctl, "anthropic", NEW_KEY)
    env["prov"].requests.clear()
    act(ctl, "click", PK + "anthropic.connect")
    state = rows(ctl, PK + "anthropic")[0]["state"]
    seen = [(r["path"], r["auth"]) for r in env["prov"].requests]
    left = rows(ctl, PK + "anthropic.key")[0]["length"]
    ok = state == "connected" and seen == [("/validate", "Bearer " + NEW_KEY)] and left == 0
    return ok, "state %r, provider saw %r, key field left with %d characters" % (state, seen, left)


@S.check("J5.4/connect-door-refused-key-says-why", "refusal-words-lost", "a refused key through the Connect door shows no reason")
def c_connect_refused(ctl, env):
    _key_door(ctl, "anthropic", "sk-bad")
    act(ctl, "click", PK + "anthropic.connect")
    r = rows(ctl, PK + "anthropic")[0]
    return (r["state"], r["note"]) == ("refused", "This key is not valid for the provider."), "row %r" % r


@S.check("J5.4/provider-without-key-door-says-so", "unsupported-silent", "a pasted Gemini key is refused without a word, as if Gemini had no key door")
def c_unsupported(ctl, env):
    """Round 6 (hid-ai): Gemini HAS a key door: the real core's provider table holds it (core/crates/ai-providers/src/table.rs slug "gemini", and
    "openrouter"), the pane shows four provider rows, and the test engine takes their keys since round 1 (FixtureSettings.credentialIds). Before: the
    case wanted Gemini refused with "Whoosh cannot connect this provider yet." (written when only Anthropic and OpenAI had doors); main92 read it
    connected. Now: a Gemini key connects with no refusal line, and the provider nobody touched stays off (the control)."""
    _key_door(ctl, "gemini", "sk-any-key-0001")
    act(ctl, "click", PK + "gemini.connect")
    act(ctl, "clock", "advance", "1")      # the test engine answers a key in two steps (FixtureProviderKeys.swift)
    r = rows(ctl, PK + "gemini")[0]
    ctrl = rows(ctl, PK + "anthropic")[0]["state"]      # control: the provider nobody touched stays off
    return (r["state"], r["note"], ctrl) == ("connected", "", "not connected"), "gemini %r, anthropic %r" % (r, ctrl)


@S.check("J5.4/provider-check-does-not-wait-on-the-ui", "connect-blocks", "Connect waits for the provider before it returns")
def c_off_main(ctl, env):
    env["prov"].delay = 1.5
    try:
        _key_door(ctl, "anthropic", NEW_KEY)
        t0 = time.monotonic()
        act(ctl, "click", PK + "anthropic.connect")
        took = time.monotonic() - t0
        mid = rows(ctl, PK + "anthropic.check")[0]["state"]
        end = rows(ctl, PK + "anthropic")[0]["state"]      # a read of the answer waits; the click did not
        ok = took < 0.5 and mid == "checking" and end == "connected"
        return ok, "the click returned in %.2f s (provider takes 1.5 s), the row said %r meanwhile, then %r" % (took, mid, end)
    finally:
        env["prov"].delay = 0


def _wait_core(ctl, path, not_values, tries=60):
    """The pasted key reaches the core a moment after the verb answers (the pane asks off the main thread): reads until the path leaves `not_values`."""
    got = None
    for _ in range(tries):
        got = core_or_none(ctl, path)
        if got not in not_values:
            return got
        time.sleep(0.02)
    return got


def _advance(ctl):
    act(ctl, "clock", "advance", "1")


@S.check("J5.1/key-answer-lands-in-two-steps", "one-step-answer", "the core answers a pasted key in one step: the row never says checking, the real core's first answer is only 'checking'")
def c_two_steps(ctl, env):
    _open(ctl)
    act(ctl, "fixture", "paste", "anthropic-key")
    first = _wait_core(ctl, "ai_provider/anthropic/state", (None, "not connected"))
    row_mid = rows(ctl, AP + ".anthropic.check")[0]["state"]      # the pane's row says checking while the core does
    _advance(ctl)
    last = core_or_none(ctl, "ai_provider/anthropic/state")
    models = core_or_none(ctl, "ai_provider/anthropic/models")
    refusal = core_or_none(ctl, "ai_provider/anthropic/refusal")
    row_end = rows(ctl, AP + ".anthropic")[0]["state"]
    ok = (first, row_mid, last, models, refusal, row_end) == ("checking", "checking", "connected", ["claude-sonnet-5-5", "claude-haiku-5-5"], None, "connected")
    return ok, "after the paste the core %r and the row %r, after the clock moved the core %r and the row %r, models %r, refusal %r" % (first, row_mid, last, row_end, models, refusal)


@S.check("J5.1/refused-key-lands-in-two-steps-with-words", "refusal-path-not-served", "a refused key lands refused but the provider's words are not on the refusal path")
def c_two_steps_refused(ctl, env):
    _open(ctl)
    act(ctl, "fixture", "paste", "bad-key")
    first = _wait_core(ctl, "ai_provider/anthropic/state", (None, "not connected"))
    _advance(ctl)
    last = core_or_none(ctl, "ai_provider/anthropic/state")
    words = core_or_none(ctl, "ai_provider/anthropic/refusal")
    models = core_or_none(ctl, "ai_provider/anthropic/models")
    ok = (first, last, words, models) == ("checking", "refused", "This key is not valid for the provider.", [])
    return ok, "after the paste %r, after the clock moved %r, the provider's words %r, models %r" % (first, last, words, models)


@S.check("J5.1/state-paths-are-served", "state-path-not-served", "the core serves no ai_provider/<id>/state, refusal or models path")
def c_paths_served(ctl, env):
    _open(ctl)
    act(ctl, "fixture", "paste", "anthropic-key")
    _wait_core(ctl, "ai_provider/anthropic/state", (None, "not connected"))
    _advance(ctl)
    got = [core_or_none(ctl, "ai_provider/anthropic/" + leaf) is not None for leaf in ("state", "models")]
    ctrl = core_or_none(ctl, "ai_provider/openai/state")      # control: a provider nobody gave a key answers "not connected"
    return got == [True, True] and ctrl in ("not connected", None), "state and models served: %r, untouched provider %r" % (got, ctrl)


@S.check("J5.5/key-check-waits-for-the-core-state-path", "state-path-skipped", "Connect takes the core's first Ok as final: a real core answers SaveProviderKey at once, the row reads not connected and the answer never arrives")
def c_state_path(ctl, env):
    """From the Swift source alone (the fixture core now answers in two steps too: J5.1/key-answer-lands-in-two-steps drives them): after Ok the pane re-reads
    `ai_provider/<id>/state` while it says "checking", bounded by twice AI_HTTP_TIMEOUT_MS, and a "refused" carries the provider's own words."""
    try:
        muts = ctl.send("selftestmutants").get("mutants", [])
    except Unreadable:
        muts = []
    src = open(os.path.join(ROOT, "Sources", "Features", "SettingsAIMac", "AISettingsStore.swift"), encoding="utf-8").read()
    if "state-path-skipped" in muts:
        src = src.replace("check.finish(AISettingsStore.landed(core, id))", "check.finish(nil)")
    waits = "check.finish(AISettingsStore.landed(core, id))" in src
    polls = 'let path = "ai_provider/\\(id)/state"' in src and 'core.state(path: path) == "\\"checking\\""' in src
    bounded = "Date() < until" in src and "2 * WhooshConstants.AI_HTTP_TIMEOUT_MS" in src and "Thread.sleep" in src
    words = 'core.state(path: "ai_provider/\\(id)/refusal")' in src and "CoreError.Invalid(field: \"key\"" in src
    return waits and polls and bounded and words, "finish waits for landed: %s; polls the state path: %s; bounded: %s; refusal words read: %s" % (waits, polls, bounded, words)


@S.check("J5.4/agent-form-link-goes-to-providers", "form-link-stays", "the form's Go to Providers link leaves the pane where it is")
def c_form_link(ctl, env):
    _open(ctl)
    act(ctl, "click", A["agents_new"])
    before = rows(ctl, NAV)[0]["section"]
    act(ctl, "click", "mac-settings-ai.agents.form.link")
    after = rows(ctl, NAV)[0]["section"]
    return (before, after) == ("agents", "providers"), "section %r then %r" % (before, after)


@S.check("J5.4/section-doors-switch-the-section", "nav-agents-goes-providers", "the Agents door opens Providers")
def c_nav(ctl, env):
    _open(ctl)
    got = []
    for to in ("providers", "agents", "decisions"):
        act(ctl, "click", "mac-settings-ai.nav." + to)
        got.append(rows(ctl, NAV)[0]["section"])
    return eq(got, ["providers", "agents", "decisions"])


@S.check("J5.4/name-door-takes-the-typing-back", "name-door-keeps-key-focus", "after a key field was clicked, the name door leaves the typing in the key field")
def c_name_door(ctl, env):
    _open(ctl)
    _paste_landed(ctl)
    act(ctl, "click", A["agents_new"])
    act(ctl, "click", PK + "openai.key")
    act(ctl, "click", "mac-settings-ai.agents.name")
    act(ctl, "type", FX["agent"]["name"])
    name = rows(ctl, A["agents_card"])[0]["name"]
    key = rows(ctl, PK + "openai.key")[0]["length"]
    return (name, key) == (FX["agent"]["name"], 0), "the draft's name %r, the key field holds %d characters" % (name, key)


@S.check("J5.4/permission-door-switches-off-again", "allow-toggle-stuck", "a second click on a permission leaves it on")
def c_allow_off(ctl, env):
    _open(ctl)
    _paste_landed(ctl)
    act(ctl, "click", A["agents_new"])
    act(ctl, "type", FX["agent"]["name"])
    got = []
    for _ in range(2):
        act(ctl, "click", "mac-settings-ai.agents.allow.draft")
        got.append(rows(ctl, A["agents_card"])[0]["allowed"])
    return eq(got, [["draft"], []])


@S.check("J5.4/save-door-closes-the-draft", "save-keeps-draft-open", "Save leaves the agent open as a draft")
def c_save(ctl, env):
    _new_agent(ctl)
    act(ctl, "click", "mac-settings-ai.agents.save")
    # the fixture core ships built-in agents (Ask my inbox, Writer: FixtureAgents.swift): the case reads this agent's own card
    cards = [(c["name"], c["editing"]) for c in rows(ctl, A["agents_card"]) if c["name"] == FX["agent"]["name"]]
    saved = core(ctl, "agent_config/%s/allowed" % FX["agent"]["name"])
    return (cards, saved) == ([(FX["agent"]["name"], False)], FX["agent"]["allow"]), "cards %r, core %r" % (cards, saved)


@S.check("J5.4/duplicate-door-copies-the-agent", "duplicate-noop", "Duplicate does nothing")
def c_duplicate(ctl, env):
    _new_agent(ctl)
    act(ctl, "click", "mac-settings-ai.agents.save")
    act(ctl, "click", "mac-settings-ai.agents.duplicate")
    names = [c["name"] for c in rows(ctl, A["agents_card"]) if c["name"].startswith(FX["agent"]["name"])]   # built-ins aside (FixtureAgents.swift)
    copy = core(ctl, "agent_config/%s copy/allowed" % FX["agent"]["name"])
    return (names, copy) == ([FX["agent"]["name"], FX["agent"]["name"] + " copy"], FX["agent"]["allow"]), "cards %r, the copy in the core %r" % (names, copy)


# ---------------------------------------------------------------- round 4b: the model of every agent, and the Dictation card
AM = "mac-settings-ai.agents.model."


def _slot(ctl, name):
    """The card number (1 = first card) of the agent called `name`."""
    return [c["name"] for c in rows(ctl, A["agents_card"])].index(name) + 1


def _connected(ctl):
    _open(ctl)
    _paste_landed(ctl)


@S.check("J5.6/model-menu-lists-only-connected-models", "menu-lists-unconnected-models", "the model menu offers a model of a provider that has no key")
def c_model_menu(ctl, env):
    _open(ctl)
    before = rows(ctl, AM + "1")
    _paste_landed(ctl)
    after = rows(ctl, AM + "1")
    live = [(r["provider"], r["model"]) for r in after if r["enabled"]]
    grey = [(r["provider"], r["note"]) for r in after if not r["enabled"]]
    ok = (not any(r["enabled"] for r in before)
          and live == [("anthropic", "claude-sonnet-5-5"), ("anthropic", "claude-haiku-5-5")]
          and grey == [(p, "Add a key above") for p in ("openai", "gemini", "openrouter")])
    return ok, "before any key: %d enabled entries; with the anthropic key: enabled %r, greyed %r" % (sum(r["enabled"] for r in before), live, grey)


@S.check("J5.6/built-in-agent-model-changes", "model-choice-not-saved", "a choice in the model menu is not saved")
def c_model_builtin(ctl, env):
    _connected(ctl)
    n = _slot(ctl, "Ask my inbox")
    act(ctl, "click", AM + "%d.choose.2" % n)
    card = rows(ctl, A["agents_card"])[n - 1]
    cur = [r["model"] for r in rows(ctl, AM + str(n)) if r["current"]]
    return (card["model"], cur) == ("claude-haiku-5-5", ["claude-haiku-5-5"]), "card %r, ticked in the menu %r" % (card["model"], cur)


@S.check("J5.6/made-agent-model-changes", "model-choice-not-saved", "a choice in the model menu is not saved")
def c_model_made(ctl, env):
    _new_agent(ctl)
    act(ctl, "click", "mac-settings-ai.agents.save")
    n = _slot(ctl, FX["agent"]["name"])
    act(ctl, "click", AM + "%d.choose.2" % n)
    card = rows(ctl, A["agents_card"])[n - 1]
    return (card["name"], card["provider"], card["model"]) == (FX["agent"]["name"], "anthropic", "claude-haiku-5-5"), "card %r" % card


@S.check("J5.6/provider-without-key-cannot-be-chosen", "greyed-entry-clickable", "a greyed entry of the model menu can be chosen")
def c_model_grey(ctl, env):
    _connected(ctl)
    n = _slot(ctl, "Ask my inbox")
    out = soft(ctl, "click", AM + "%d.choose.3" % n)        # entry 3 is OpenAI, which has no key
    card = rows(ctl, A["agents_card"])[n - 1]
    return (out, card["provider"], card["model"]) == (None, "anthropic", "claude-sonnet-5-5"), "click answered %r, card %r" % (out, (card["provider"], card["model"]))


AGENTS_NEW_ROW = "mac-settings-ai.agents.new.row"      # the agents group's first row (AgentsView.swift:11), the id the app draws; the case reads it as rows when the app serves it


@S.check("J5.6/new-agent-row-and-button-have-two-names", "row-title-repeats-button", "the row says New agent and so does its button")
def c_heading(ctl, env):
    _open(ctl)
    try:
        r = rows(ctl, AGENTS_NEW_ROW)[0]
    except Unreadable as e:
        # main79: `mac-settings-ai.agents.heading` is in no file under Sources. The row the app draws is the accessibility id `agents.new.row` (AgentsView.swift:11) and its
        # button `agents.new` (:12); neither is a rows source, and no served read gives a drawn title, so the two names cannot be read here. Not judged, never a pass.
        raise Unreadable("no served read of the row title and the button title: %s (the app draws %s and %s, AgentsView.swift:11-12, as ids only; a rows source for the row with its `title` and `button` words is what this case needs)" % (e, AGENTS_NEW_ROW, A["agents_new"]))
    # no sentence is compared with a literal (the rig's language is not assumed): the two names are both there and differ
    return bool(r["title"]) and bool(r["button"]) and r["title"] != r["button"], "row %r, button %r" % (r["title"], r["button"])


@S.check("J5.6/speech-pill-switches-the-engine", "speech-pill-ignores-choice", "the Dictation pill keeps the old engine after a choice")
def c_speech_pill(ctl, env):
    _connected(ctl)
    act(ctl, "click", "mac-settings-ai.speech.engine.anthropic")
    r1 = rows(ctl, A["speech"])[0]
    act(ctl, "click", "mac-settings-ai.speech.engine.local")
    r2 = rows(ctl, A["speech"])[0]
    return ((r1["engine"], r1["state"]), (r2["engine"], r2["state"])) == (("anthropic", "ready"), ("local", "ready")), "after the provider: %r, back on this Mac: %r" % (r1, r2)


@S.check("J5.6/speech-pill-offers-connected-providers-only", "speech-offers-unconnected", "the Dictation pill offers providers with no key")
def c_speech_offers(ctl, env):
    _open(ctl)
    before = [r["engine"] for r in rows(ctl, "mac-settings-ai.speech.engines")]
    _paste_landed(ctl)
    after = [r["engine"] for r in rows(ctl, "mac-settings-ai.speech.engines")]
    return (before, after) == (["local"], ["local", "anthropic"]), "no key: %r, with the anthropic key: %r" % (before, after)


@S.check("J5.6/speech-line-names-the-key", "speech-line-no-key", "the Dictation line shows no key")
def c_speech_line(ctl, env):
    _open(ctl)
    r = rows(ctl, "mac-settings-ai.speech.line")[0]
    return (r["key"], r["link"]) == ("Fn", False), "line %r" % r


@S.check("J5.6/speech-card-is-titled-dictation-not-speech", "speech-card-titled-speech", "the card still reads Speech, and its row repeats the word")
def c_speech_title(ctl, env):
    _open(ctl)
    r = rows(ctl, A["speech"])[0]
    got = (r.get("title"), r.get("row_title"))
    return got == ("Dictation", "Engine"), "the card title and its row title read %r (expected ('Dictation', 'Engine')); 'Speech' is not the title: %s" % (got, "Speech" not in got)


@S.check("J5.6/shortcuts-link-opens-shortcuts", "dead:mac-settings-ai.speech.shortcuts", "the link to Settings > Shortcuts has no door")
def c_speech_link(ctl, env):
    _open(ctl)
    before = rows(ctl, "mac-settings-ai.speech.link")[0]["pane"]
    act(ctl, "click", "mac-settings-ai.speech.shortcuts")
    after = rows(ctl, "mac-settings-ai.speech.link")[0]["pane"]
    return (before != "shortcuts", after) == (True, "shortcuts"), "pane %r then %r" % (before, after)


# ---------------------------------------------------------------- round 6: the Decisions pane talks to the core; Laya is handed over
DE = A["dec_engine"]
SRC = os.path.join(ROOT, "Sources", "Features")


def _src(ctl, *rel):
    """The Swift sources under Sources/Features, as the app is built from them; a mutant of this stand-in rewrites them in memory."""
    try:
        muts = ctl.send("selftestmutants").get("mutants", [])
    except Unreadable:
        muts = []
    out = {}
    for r in rel:
        out[r] = open(os.path.join(SRC, r), encoding="utf-8").read()
    if "interim-door-back" in muts:
        out["SettingsAIMac/AISettingsStore.swift"] += '\nenum InterimProviderDoor { static func validateJev(_ key: String) -> Bool { key == "tsa-test-jev-key-0001" } }\n'
    if "rows-drawn-twice" in muts:
        out["SettingsAIMac/DecisionModelRow.swift"] = out["SettingsAIMac/DecisionModelRow.swift"].replace("AmberButton(", "OwnButton(")
    if "rig-models-in-owner-folder" in muts:
        out["Embedding/GemmaWeights.swift"] += "\nlet _o = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)\n"
    if "state-word-printed-raw" in muts:
        out["SettingsAIMac/DecisionsHomeView.swift"] += "\nlet _x = Text(aiProviderStateKey(s: .notChecked))\n"
    if "second-hand-over-back" in muts:
        out["OnDeviceAI/OnDeviceSetup.swift"] += "\nextension OnDeviceSetup { func again(_ c: WhooshCore) { c.setLocalModel(model: nil, maxLen: 0, headMaxLen: 0) } }\n"
    return out


def _chosen(ctl):
    return [r["id"] for r in rows(ctl, DE) if r["chosen"]]


@S.check("J5.7/engine-choice-is-saved-in-the-core-and-read-back", "engine-choice-local-only",
         "the pane keeps the engine choice to itself: the core still holds the old one")
def c_engine_saved(ctl, env):
    _open(ctl)
    got = []
    for e in ("off", "local"):
        act(ctl, "click", "mac-settings-ai.decisions.engine." + e)
        got.append((e, core(ctl, "decision_model/provider"), _chosen(ctl)))
    # Off is stored as the `rules` provider: Sources/Features/SettingsAIMac/AISettingsStore.swift:113 (`case .off: m.provider = .rules`) and the core's
    # DecisionProviderKind::Rules (core/crates/engine/src/decision_model.rs:73); the pane's own control still says `off`.
    return got == [("off", "rules", ["off"]), ("local", "local", ["local"])], "choice, the core holds, the pane shows: %r" % got


@S.check("J5.7/jev-needs-its-key-then-is-saved", "jev-without-key-accepted",
         "Jev is chosen without a key and the pane says it is saved")
def c_jev_key(ctl, env):
    _open(ctl)
    # main79: with no key AND no consent the refusal is the core's consent refusal (it does not say "key"), so the consent is given first and the missing KEY is then the only
    # thing in the way: Jev is refused in words and not saved; the key is stored; Jev is chosen again and is saved
    act(ctl, "click", "mac-settings-ai.decisions.jev.consent.allow")
    act(ctl, "click", "mac-settings-ai.decisions.engine.jev")
    refusal = [r["refusal"] for r in rows(ctl, DE) if r["id"] == "jev"][0]
    held = core_or_none(ctl, "decision_model/provider")      # main77: the core holds no provider before any engine was saved ("no core value at decision_model/provider"), which is not Jev
    act(ctl, "click", "mac-settings-ai.decisions.jev.key.field")
    act(ctl, "type", D["jev_key"])
    act(ctl, "click", "mac-settings-ai.decisions.jev.connect")
    stored = core(ctl, "provider_credential/jev/stored")
    act(ctl, "click", "mac-settings-ai.decisions.engine.jev")
    after = (core(ctl, "decision_model/provider"), _chosen(ctl))
    ok = refusal != "" and held != "jev" and stored is True and after == ("jev", ["jev"])
    return ok, "refusal with consent and no key %r, the core held %r before the key; key stored %r; Jev chosen again with key and consent %r" % (refusal, held, stored, after)


@S.check("J5.7/jev-without-consent-is-refused-in-words", "jev-without-consent-accepted",
         "Jev is chosen with a key but without the owner's consent and the core takes it")
def c_jev_noconsent(ctl, env):
    _open(ctl)
    act(ctl, "click", "mac-settings-ai.decisions.jev.key.field")
    act(ctl, "type", D["jev_key"])
    act(ctl, "click", "mac-settings-ai.decisions.jev.connect")
    act(ctl, "click", "mac-settings-ai.decisions.engine.jev")
    refusal = [r["refusal"] for r in rows(ctl, DE) if r["id"] == "jev"][0]
    held = core_or_none(ctl, "decision_model/provider")      # as above: no value yet is None, and None is not Jev
    return refusal != "" and held != "jev", "refusal words %r, the core holds %r" % (refusal, held)


@S.check("J5.7/the-literal-test-key-is-gone-from-product-code", "interim-door-back",
         "the pane accepts Jev's key by comparing it with a test key")
def c_no_literal(ctl, env):
    src = _src(ctl, "SettingsAIMac/AISettingsStore.swift", "SettingsAIMac/DecisionsHomeView.swift", "SettingsAIMac/SettingsAIMacFeature.swift")
    text = "\n".join(src.values())
    bad = [t for t in ("tsa-test-jev-key", "InterimProviderDoor", "validateJev") if t in text]
    through_core = 'paste(key, into: "jev")' in text and "jevConnected = core.state(path:" in text
    return not bad and through_core, "test-key traces left %r; Jev's key goes through SaveProviderKey and the connected state is read from the core: %s" % (bad, through_core)


@S.check("J5.7/laya-has-one-hand-over-the-typed-decision-port", "second-hand-over-back",
         "the pane's own LocalModel hand-over is back beside the decision port")
def c_one_handover(ctl, env):
    src = _src(ctl, "../CoreBridge/CoreHost.swift", "OnDeviceAI/OnDeviceSetup.swift")
    host = src["../CoreBridge/CoreHost.swift"]
    port = host.count("openRustCoreWithDecision(") == 1 and "LayaOnDevice.shared.port" in host
    allswift = []
    for root, _d, files in os.walk(SRC):
        allswift += [open(os.path.join(root, f), encoding="utf-8").read() for f in files if f.endswith(".swift")]
    allswift += list(src.values())
    dup = [t for t in ("LayaHandOver", "setLocalModel(") if any(t in x for x in allswift)]
    gone = not os.path.exists(os.path.join(SRC, "OnDeviceAI/Contract/LayaHandOver.swift")) and not os.path.exists(os.path.join(SRC, "OnDeviceAI/Contract/LayaHandOver+Core.swift"))
    return port and not dup and gone, "core opened with the decision port once: %s; a second hand-over left %r; its files gone: %s" % (port, dup, gone)


def _laya_row(ctl):
    return rows(ctl, "mac-settings-ai.decisions.local")[0]


def model_facts_wanted(line="laya"):
    """The facts line of a model as the screen must draw it in English: "<size> \u00b7 <licence> \u00b7 index <score, one decimal>", each part read from its
    source: the size from the ledger (docs/CONSTANTS.md, the constant the catalogue's line names), written as the file system writes it (MB under
    1,000 MB, else GB with two decimals), the licence and the score from the catalogue's line (`static let <line> = DecisionModelEntry(`)."""
    import re
    ledger = open(os.path.join(ROOT, "docs", "CONSTANTS.md"), encoding="utf-8").read()
    cat = open(os.path.join(ROOT, "Sources", "Features", "OnDeviceAI", "Core", "DecisionModelCatalogue.swift"), encoding="utf-8").read()
    at = cat.find("static let %s = DecisionModelEntry(" % line)
    if at < 0:
        raise Unreadable("the catalogue has no line %r" % line)
    body = cat[at:]
    const = re.search(r"downloadMB: WhooshConstants\.(\w+),", body)
    mb = re.search(r"^\| %s \| (\d+) \|" % const.group(1), ledger, re.M) if const else None
    licence = re.search(r'licence: "([^"]+)"', body)
    score = re.search(r"score: ([\d.]+),", body)
    if not (mb and licence and score):
        raise Unreadable("the sources of the model's facts line could not be read (ledger %s, licence %s, score %s)" % (bool(mb), bool(licence), bool(score)))
    n = int(mb.group(1))
    size = "%d MB" % n if n < 1000 else "%.2f GB" % (n / 1000)
    return "%s \u00b7 %s \u00b7 index %.1f" % (size, licence.group(1), float(score.group(1)))


KL = "knowline-0.8b"                                 # the second line of the catalogue (DecisionModelCatalogue.knowline08)
KLX = "mac-settings-ai.decisions.model." + KL        # its row and its three doors (.download, .cancel, .choose)


def _kl_fresh(ctl, env, slow=0.0):
    """Both models off this Mac and the pane open; the second model's download staged on the loopback stub with a small file list
    (`decision-model-<id>.json` in the run folder, read when the address is staged: DecisionModels.useRigBase). `slow` is the pause between the
    ten pieces of the last file, so a case can read the line while it downloads. NOT JUDGED with what was read when the line is not absent."""
    run = getattr(ctl, "dir", "")
    if not run or not os.path.exists(os.path.join(run, "whoosh.sock")):
        raise Unreadable("no rig run folder (WHOOSH_SOCKET_DIR): the models folder to empty is unknown")
    owner = os.path.realpath(os.path.expanduser("~/Library/Application Support/Whoosh"))
    folder = os.path.realpath(os.path.join(run, "Models", "knowline-0.8b-gen1"))
    if folder == owner or folder.startswith(owner + os.sep):
        raise Unreadable("the models folder %s is the owner's, never emptied by a check" % folder)
    shutil.rmtree(folder, ignore_errors=True)
    digest = hashlib.sha256(b"A" * LAYA_SIZE).hexdigest()
    with open(os.path.join(run, "decision-model-%s.json" % KL), "w", encoding="utf-8") as f:
        json.dump({"files": [{"path": p, "bytes": LAYA_SIZE, "sha256": digest} for p in LAYA_FILES]}, f)
    env["laya"].piece_delay = slow
    staged = soft_why(ctl, "fixture", "provider-endpoint", "model:" + KL, env["laya"].url)
    _laya_fresh(ctl)
    row = (rows(ctl, KLX) or [{}])[0]
    if row.get("state") != "absent" or row.get("in_use"):
        raise Unreadable("the second model's line reads %r after its folder %s was emptied (staging answered %r): the case cannot start from a known state" % (row, folder, staged))


def _kl_wait(ctl, pred, seconds=20):
    """Reads the two lines until `pred(laya, second)` holds; returns the last read."""
    end = time.monotonic() + seconds
    while True:
        a, b = _laya_row(ctl), rows(ctl, KLX)[0]
        if pred(a, b) or time.monotonic() >= end:
            return a, b
        time.sleep(0.2)


def _brief(r):
    return {k: r.get(k) for k in ("model", "state", "in_use", "wanted", "until") if k in r}


def _laya_wait(ctl, seconds=2.4):
    """Reads the Laya row until the download ends (the app moves it on its own); gives up after `seconds`."""
    r = _laya_row(ctl)
    end = time.monotonic() + seconds
    while r["state"] not in ("ready", "failed") and time.monotonic() < end:
        time.sleep(0.2)
        r = _laya_row(ctl)
    return r


LAYA_PIECE_S = 0.5      # seconds between the pieces of the big file: ten pieces, five seconds in all
def _fig(r, since):
    """One read of the Laya row as the evidence of a download case: state, bytes received, the row's words, seconds since the click."""
    return (r.get("state"), r.get("received"), str(r.get("words", ""))[:60], round(time.monotonic() - since, 2))


LAYA_TICKS_S = 0.6      # a wait of at least two of the app's 250 ms progress ticks (LayaModelStore.progressTickNs)


class _slow_laya:
    """The download setup of the progress and Cancel cases: the stub serves the big file at LAYA_PIECE_S per piece, so the app's progress tick has
    pieces to show and Cancel has pieces left to stop (a stub that sends 150 KB at once ends the download before the first read: main76/main77 read
    'downloading, 0' twice then 'ready', or 'ready' after Cancel). The delay is restored and a running download cancelled on the way out."""
    def __init__(self, ctl, env):
        self.ctl, self.env = ctl, env

    def __enter__(self):
        self.env["laya"].piece_delay = LAYA_PIECE_S
        return self

    def __exit__(self, *a):
        self.env["laya"].piece_delay = 0
        try:
            if _laya_row(self.ctl).get("state") == "downloading":
                soft(self.ctl, "click", "mac-settings-ai.decisions.local.cancel")
                _laya_wait_not_downloading(self.ctl)
        except (Unreadable, KeyError, IndexError):
            pass
        return False


def _laya_wait_not_downloading(ctl, seconds=8):
    end = time.monotonic() + seconds
    while time.monotonic() < end and _laya_row(ctl).get("state") == "downloading":
        time.sleep(0.2)


def _laya_fresh(ctl):
    """Opens the AI pane with the on-device model NOT on this Mac, a known state every Laya case starts from.
    Why: the model's state is the DISK's (LayaModelStore.refresh reads `<models folder>/laya/ready.json`) and `fixture reset` does not touch it, so one
    case that downloads the model (J5.7/laya-row-says-why-not...) leaves it ready for the next: main76 read `ready` and every download click "did nothing".
    The folder is the rig copy's own, `<run folder>/Models/laya` (ModelsFolder.root(): a rig copy keeps its models under its run folder, the one
    WHOOSH_SOCKET_DIR names), emptied here; the owner's `~/Library/Application Support/Whoosh` is refused. Where the row still is not absent afterwards
    the case is NOT JUDGED, with the state it read."""
    run = getattr(ctl, "dir", "")
    if not run or not os.path.exists(os.path.join(run, "whoosh.sock")):
        raise Unreadable("no rig run folder (WHOOSH_SOCKET_DIR): the models folder to empty is unknown")
    owner = os.path.realpath(os.path.expanduser("~/Library/Application Support/Whoosh"))
    folder = os.path.realpath(os.path.join(run, "Models", "laya"))
    if folder == owner or folder.startswith(owner + os.sep):
        raise Unreadable("the models folder %s is the owner's, never emptied by a check" % folder)
    shutil.rmtree(folder, ignore_errors=True)
    if os.path.exists(folder):
        raise Unreadable("the rig's models folder %s could not be emptied" % folder)
    _open(ctl)
    state = _laya_row(ctl).get("state")
    if state != "absent":
        raise Unreadable("the Laya row reads %r after its models folder %s was emptied, not 'absent': the case cannot start from a known state" % (state, folder))


@S.check("J5.7/laya-row-says-why-not-and-the-download-door-works", "laya-absent-silent",
         "the Laya row is silent when the weights are not on this Mac")
def c_laya_row(ctl, env):
    _laya_fresh(ctl)
    before = rows(ctl, "mac-settings-ai.decisions.local")[0]
    act(ctl, "click", "mac-settings-ai.decisions.local.download")
    after = _laya_wait(ctl)
    ok = before["state"] == "absent" and "Not on this Mac yet" in before["words"] and before["download"] and after["state"] == "ready" and not after["download"]
    return ok, "before the download %r, after %r" % (before, after)


@S.check("J5.7/the-model-line-says-size-licence-and-index-score", "model-line-without-its-score",
         "the model's line says its size and licence and leaves the index score out")
def c_model_line_facts(ctl, env):
    """SPEC A133 and the owner's pick M1 (board decision-models-r1): one line per model, "size \u00b7 licence \u00b7 index N". The line is read in two states
    (not downloaded, ready): the facts are the model's, never the download's."""
    _laya_fresh(ctl)
    want = model_facts_wanted()
    absent = _laya_row(ctl)
    act(ctl, "click", "mac-settings-ai.decisions.local.download")
    ready = _laya_wait(ctl)
    ok = absent.get("state") == "absent" and ready.get("state") == "ready" and absent.get("facts") == want and ready.get("facts") == want and absent.get("model") == "laya"
    return ok, "wanted %r; not downloaded: %r; ready: %r" % (want, {k: absent.get(k) for k in ("state", "model", "facts")}, {k: ready.get(k) for k in ("state", "model", "facts")})


@S.check("J5.7/the-model-in-use-carries-the-mark", "model-line-not-marked-in-use",
         "the line of the model that sorts the mail does not carry the mark of the model in use")
def c_model_line_mark(ctl, env):
    """Pick M1: the model in use is marked at the left of its line. Laya is the one on-device model today, so with the provider on this Mac its line is marked,
    downloaded or not."""
    _laya_fresh(ctl)
    absent = _laya_row(ctl)
    act(ctl, "click", "mac-settings-ai.decisions.local.download")
    ready = _laya_wait(ctl)
    ok = absent.get("in_use") is True and ready.get("in_use") is True and ready.get("state") == "ready"
    return ok, "not downloaded: %r; ready: %r" % ({k: absent.get(k) for k in ("state", "in_use")}, {k: ready.get(k) for k in ("state", "in_use")})


@S.check("J5.7/the-list-holds-every-model-of-the-catalogue", "second-model-line-missing",
         "the list shows Laya alone: the catalogue's second model has no line")
def c_model_list(ctl, env):
    """SPEC A133 and the owner's pick M1: one line per model the app can run, each with its facts, the filled circle on exactly one."""
    import re
    _kl_fresh(ctl, env)
    cat = open(os.path.join(ROOT, "Sources", "Features", "OnDeviceAI", "Core", "DecisionModelCatalogue.swift"), encoding="utf-8").read()
    names = [x.strip() for x in re.search(r"public static let all: \[DecisionModelEntry\] = \[([^\]]*)\]", cat).group(1).split(",") if x.strip()]
    want = [(re.search(r'static let %s = DecisionModelEntry\(\s*id: "([^"]+)"' % n, cat).group(1), model_facts_wanted(n)) for n in names]
    got = rows(ctl, "mac-settings-ai.decisions.models")
    ok = [(r.get("model"), r.get("facts")) for r in got] == want and [r.get("in_use") for r in got].count(True) == 1 and got[0].get("in_use") is True
    return ok, "wanted %r; the list %r" % (want, [(r.get("model"), r.get("facts"), r.get("in_use")) for r in got])


@S.check("J5.7/a-second-model-downloads-then-becomes-the-model-in-use", "second-model-never-in-use",
         "the second model downloads and Laya stays the model in use for ever")
def c_second_model(ctl, env):
    """Pick M1, "A second model downloads: the model in use keeps sorting until the new one is ready": Download on a line that is not in use fetches
    it, and when its files are here it becomes the model in use; the core keeps the choice (`decision_model/model`)."""
    _kl_fresh(ctl, env, slow=0.25)
    act(ctl, "click", KLX + ".download")
    during = _kl_wait(ctl, lambda a, b: b.get("state") == "downloading", 5)
    after = _kl_wait(ctl, lambda a, b: b.get("in_use") is True, 30)
    kept = core_or_none(ctl, "decision_model/model")
    ok = (during[1].get("state") == "downloading" and during[1].get("wanted") is True and during[0].get("in_use") is True
          and after[1].get("in_use") is True and after[1].get("state") == "ready" and after[0].get("in_use") is False and kept == KL)
    return ok, "while it downloads: Laya %r, the second %r; after: Laya %r, the second %r; the core's model %r" % (_brief(during[0]), _brief(during[1]), _brief(after[0]), _brief(after[1]), kept)


@S.check("J5.7/the-model-in-use-says-until-when-another-downloads", "until-line-missing",
         "the model in use says nothing while the model asked for downloads")
def c_until_line(ctl, env):
    """Pick M1: the line of the model in use, on this Mac, says "In use until <the other> is ready." while that one downloads, and stops saying
    it after."""
    _kl_fresh(ctl, env)
    act(ctl, "click", "mac-settings-ai.decisions.local.download")
    before = _laya_wait(ctl, 10)
    env["laya"].piece_delay = 0.25
    act(ctl, "click", KLX + ".download")
    during = _kl_wait(ctl, lambda a, b: b.get("state") == "downloading", 5)
    after = _kl_wait(ctl, lambda a, b: b.get("state") == "ready", 30)
    ok = (before.get("state") == "ready" and "until" not in before and during[0].get("until") == "In use until KnowLine 0.8B is ready."
          and "until" not in after[0])
    return ok, "before %r; while it downloads %r; after %r" % (_brief(before), _brief(during[0]), _brief(after[0]))


@S.check("J5.7/a-model-that-is-not-on-this-mac-never-says-in-use-until", "until-line-when-absent",
         "a model that is not on this Mac says it is in use until the other one is ready")
def c_until_absent(ctl, env):
    """Found on the pictures of main191: with the chosen model not on this Mac, its line said "In use until KnowLine 0.8B is ready." above
    "Not on this Mac yet." A model that is not here sorts nothing: its line keeps its amber sentence and its Download button, no "until"."""
    _kl_fresh(ctl, env, slow=0.25)
    act(ctl, "click", KLX + ".download")
    during = _kl_wait(ctl, lambda a, b: b.get("state") == "downloading", 5)
    ok = (during[1].get("state") == "downloading" and during[0].get("state") == "absent" and during[0].get("download") is True
          and "until" not in during[0])
    act(ctl, "click", KLX + ".cancel")
    _kl_wait(ctl, lambda a, b: b.get("state") == "absent" and not b.get("wanted"), 5)
    return ok, "while the second downloads: Laya %r (download %r), the second %r" % (_brief(during[0]), during[0].get("download"), _brief(during[1]))


@S.check("J5.7/cancel-keeps-the-model-in-use", "cancel-leaves-it-wanted",
         "after Cancel the model that was asked for is still waited for")
def c_cancel_second(ctl, env):
    """Cancel on the downloading line stops it: the line is not downloaded again, nothing is waited for, the model in use is unchanged."""
    _kl_fresh(ctl, env, slow=0.4)
    act(ctl, "click", KLX + ".download")
    during = _kl_wait(ctl, lambda a, b: b.get("state") == "downloading", 5)
    act(ctl, "click", KLX + ".cancel")
    after = _kl_wait(ctl, lambda a, b: b.get("state") == "absent" and not b.get("wanted"), 5)
    ok = (during[1].get("state") == "downloading" and after[1].get("state") == "absent" and after[1].get("wanted") is False
          and after[0].get("in_use") is True and "until" not in after[0])
    return ok, "while it downloads %r; after Cancel: Laya %r, the second %r" % (_brief(during[1]), _brief(after[0]), _brief(after[1]))


@S.check("J5.7/the-circle-of-a-model-on-this-mac-switches-at-once", "circle-click-ignored",
         "a click on the circle of a downloaded model changes nothing")
def c_circle(ctl, env):
    """Pick M1: "The filled circle is the model in use". With both models on this Mac, the circle of the other one switches at once, no download."""
    _kl_fresh(ctl, env)
    act(ctl, "click", "mac-settings-ai.decisions.local.download")
    _laya_wait(ctl)
    act(ctl, "click", KLX + ".download")
    second = _kl_wait(ctl, lambda a, b: b.get("in_use") is True, 30)
    act(ctl, "click", "mac-settings-ai.decisions.local.choose")
    back = _kl_wait(ctl, lambda a, b: a.get("in_use") is True, 3)
    ok = (second[0].get("state") == "ready" and second[1].get("in_use") is True and back[0].get("in_use") is True and back[1].get("in_use") is False
          and back[1].get("state") == "ready" and core_or_none(ctl, "decision_model/model") == "laya")
    return ok, "after the second model's download: Laya %r, the second %r; after Laya's circle: Laya %r, the second %r" % (_brief(second[0]), _brief(second[1]), _brief(back[0]), _brief(back[1]))


# ---- the line "Other model" (model-door, item e959af26; the owner's pick M1, board decision-models-r1 `other` and "Another model, refused") ------------
OTHER = "mac-settings-ai.decisions.other"            # the line's row and its doors (.add, .field, .check, .leave)
LINES = "mac-settings-ai.decisions.lines"            # the card's lines in the order they are drawn
HUB_COPY = "rig/knowline-copy"                       # where the stub serves KnowLine's real answer as a model that is not a line yet (HUB_ANSWERS)
OTHER_FACT = "A Hugging Face address of a model Whoosh can run."      # the drawing's fact line (designs.js, the constant `other`)


def _catalogue_ids():
    """The ids of the catalogue's lines in the order of `all` (DecisionModelCatalogue.swift), as c_model_list reads them."""
    import re
    cat = open(os.path.join(ROOT, "Sources", "Features", "OnDeviceAI", "Core", "DecisionModelCatalogue.swift"), encoding="utf-8").read()
    names = [x.strip() for x in re.search(r"public static let all: \[DecisionModelEntry\] = \[([^\]]*)\]", cat).group(1).split(",") if x.strip()]
    return [re.search(r'static let %s = DecisionModelEntry\(\s*id: "([^"]+)"' % n, cat).group(1) for n in names]


def _door_fresh(ctl, env):
    """A clean list: the added models' file and folders of the rig's run folder removed, the hub staged on the loopback stub, then `fixture reset` (a reset
    re-reads added-models.json: the added lines live on the disk and outlive a reset, as a downloaded model does), the pane open. NOT JUDGED when the
    list then holds more than the catalogue's lines and "other", or the door is not closed."""
    run = getattr(ctl, "dir", "")
    if not run or not os.path.exists(os.path.join(run, "whoosh.sock")):
        raise Unreadable("no rig run folder (WHOOSH_SOCKET_DIR): the models folder to empty is unknown")
    owner = os.path.realpath(os.path.expanduser("~/Library/Application Support/Whoosh"))
    models = os.path.realpath(os.path.join(run, "Models"))
    if models == owner or models.startswith(owner + os.sep):
        raise Unreadable("the models folder %s is the owner's, never emptied by a check" % models)
    for f in (os.path.join(models, "added-models.json"),):
        if os.path.exists(f):
            os.remove(f)
    shutil.rmtree(os.path.join(models, "hub"), ignore_errors=True)
    env["hub"].gets.clear()
    staged = soft_why(ctl, "fixture", "provider-endpoint", "model-hub", env["hub"].url)
    soft(ctl, "fixture", "reset")
    _open(ctl)
    lines, other = [r.get("line") for r in rows(ctl, LINES)], (rows(ctl, OTHER) or [{}])[0]
    if [l for l in lines if l != "other"] != _catalogue_ids() or other.get("open") is True:
        raise Unreadable("after the added models' file was removed and the copy reset, the list reads %r and the door %r (hub staging answered %r): "
                         "the case cannot start from a clean list" % (lines, other, staged))


def _door_check(ctl, address, seconds=10):
    """Add, the address typed into the field, Check; reads the door until the check has answered (the field closed, or a refusal shown)."""
    act(ctl, "click", OTHER + ".add")
    act(ctl, "type", address)
    act(ctl, "click", OTHER + ".check")
    end = time.monotonic() + seconds
    while True:
        o = rows(ctl, OTHER)[0]
        if not o.get("open") or o.get("refusal") or time.monotonic() >= end:
            return o
        time.sleep(0.1)


def _hub_asked(env):
    return list(env["hub"].gets)


def _refusal_case(ctl, env, address, want):
    _door_fresh(ctl, env)
    before = [r.get("model") for r in rows(ctl, "mac-settings-ai.decisions.models")]
    o = _door_check(ctl, address)
    after = [r.get("model") for r in rows(ctl, "mac-settings-ai.decisions.models")]
    ok = o.get("open") is True and o.get("refusal") == want and after == before
    return ok, o, before, after


@S.check("J5.7/other-model-is-the-last-line", "other-line-missing",
         "the list ends with the last model: the line that adds another model is not drawn")
def c_other_last(ctl, env):
    """Pick M1 (board decision-models-r1, the constant `other`): the last line of the card is "Other model", an empty circle, one grey fact line, Add."""
    _door_fresh(ctl, env)
    lines = [r.get("line") for r in rows(ctl, LINES)]
    o = rows(ctl, OTHER)[0]
    ok = (lines == _catalogue_ids() + ["other"] and o.get("name") == "Other model" and o.get("facts") == OTHER_FACT and o.get("in_use") is False
          and o.get("button") == "Add" and o.get("key") == "" and o.get("open") is False)
    return ok, "the card's lines %r; the last line %r" % (lines, o)


@S.check("J5.7/other-model-add-shows-the-field-and-escape-leaves-it", "other-add-ignored",
         "Add changes nothing: no field takes the address")
def c_other_add(ctl, env):
    """Add turns the line into a field holding the address and Check with its Return cap (the drawing "Another model, refused"); the field takes what is typed;
    Escape (the door `.leave`) leaves the field and drops what was typed."""
    _door_fresh(ctl, env)
    act(ctl, "click", OTHER + ".add")
    opened = rows(ctl, OTHER)[0]
    if opened.get("open") is not True:
        return False, "after Add the line reads %r: no field" % opened
    act(ctl, "type", "owner/model")
    typed = rows(ctl, OTHER)[0]
    act(ctl, "click", OTHER + ".leave")
    left = rows(ctl, OTHER)[0]
    ok = (opened.get("open") is True and opened.get("button") == "Check" and opened.get("key") == "↩" and opened.get("focused") is True
          and typed.get("address") == "owner/model" and left.get("open") is False and left.get("address") == "" and left.get("button") == "Add")
    return ok, "after Add %r; after typing %r; after Escape %r" % (opened, typed, left)


@S.check("J5.7/other-model-adds-a-model-the-app-runs-with-its-size-and-licence", "other-added-line-missing",
         "a model the app runs passes the check and no line is added")
def c_other_adds(ctl, env):
    """Design 1, 2, 3 and 5: a full page address (anything after the name dropped) of a model the app runs (KnowLine's real hub answer, served at a second
    address) becomes the last model line: id hub:<owner>/<name>, the name part, "<size> · apache-2.0". The size and the files are the catalogue's KnowLine
    line (the same eight files at the same revision: DecisionModelCatalogue.knowline08), the checksums the hub's lfs.sha256 or none; the line is kept in
    <run>/Models/added-models.json; the hub was asked its two reads."""
    import re
    _door_fresh(ctl, env)
    o = _door_check(ctl, "https://huggingface.co/%s/tree/main" % HUB_COPY)
    models = rows(ctl, "mac-settings-ai.decisions.models")
    lines = [r.get("line") for r in rows(ctl, LINES)]
    last = models[-1] if models else {}
    size = model_facts_wanted("knowline08").split(" · ")[0]
    card = json.load(open(os.path.join(HUB_CORPUS, "knowline-0.8b.api.json"), encoding="utf-8"))
    lfs = {x["rfilename"]: (x.get("lfs") or {}).get("sha256", "") for x in card["siblings"]}
    cat = open(os.path.join(ROOT, "Sources", "Features", "OnDeviceAI", "Core", "DecisionModelCatalogue.swift"), encoding="utf-8").read()
    body = cat[cat.find("static let knowline08 = DecisionModelEntry("):]
    want_files = [{"path": p, "bytes": int(b.replace("_", "")), "sha256": lfs.get(p, "")} for p, b in re.findall(r'ModelFile\(path: "([^"]+)", bytes: ([\d_]+),', body.split("folder:")[0])]
    path = os.path.join(getattr(ctl, "dir", ""), "Models", "added-models.json")
    kept = json.load(open(path, encoding="utf-8")) if os.path.exists(path) else None
    rec = (kept or [{}])[-1] if isinstance(kept, list) else {}
    asked = _hub_asked(env)
    ok = (o.get("open") is False and o.get("refusal") == "" and last.get("model") == "hub:" + HUB_COPY and last.get("name") == "knowline-copy"
          and last.get("facts") == "%s · apache-2.0" % size and last.get("in_use") is False and last.get("added") is True
          and lines == _catalogue_ids() + ["hub:" + HUB_COPY, "other"]
          and rec.get("id") == "hub:" + HUB_COPY and rec.get("revision") == card["sha"] and rec.get("licence") == "apache-2.0" and rec.get("files") == want_files
          and asked == ["/api/models/%s?blobs=true" % HUB_COPY, "/%s/resolve/%s/config.json" % (HUB_COPY, card["sha"])])
    return ok, "the door after Check %r; the last model line %r; the card's lines %r; wanted facts %r; kept on disk %r (wanted files %r); the hub stub was asked %r" % (
        o, last, lines, "%s · apache-2.0" % size, rec, want_files, asked)


@S.check("J5.7/other-model-of-another-kind-is-refused-in-one-sentence", "kind-refusal-silent",
         "a model of a kind the app does not run is not refused for its kind")
def c_other_kind(ctl, env):
    """Design 4, with Valen's real hub answer (model_type valen_qwen)."""
    want = "Whoosh cannot run this model: its kind (valen_qwen) is not one it knows."
    ok, o, before, after = _refusal_case(ctl, env, "Valen-Team/Valen-Decision-1.5-2B", want)
    return ok, "wanted %r; the door %r; the model lines before %r, after %r; the hub stub was asked %r" % (want, o, before, after, _hub_asked(env))


@S.check("J5.7/other-model-with-its-own-head-is-refused-in-one-sentence", "own-head-accepted",
         "a model that reads decisions through its own head is added")
def c_other_head(ctl, env):
    """Design 3 and 4, with GLiDE's real hub answer (a qwen3_5 model that holds decision_config.json)."""
    want = "Whoosh cannot run this model yet: it reads decisions through its own head."
    ok, o, before, after = _refusal_case(ctl, env, "huggingface.co/fastino/GLiDE-0.8B", want)
    return ok, "wanted %r; the door %r; the model lines before %r, after %r; the hub stub was asked %r" % (want, o, before, after, _hub_asked(env))


@S.check("J5.7/other-model-at-an-empty-address-is-refused-in-one-sentence", "404-said-unreachable",
         "a 404 of the hub is shown as a hub that could not be reached")
def c_other_404(ctl, env):
    """Design 4: the hub answers 404 (the stub holds no model at the address)."""
    want = "Hugging Face has no model at this address."
    ok, o, before, after = _refusal_case(ctl, env, "nobody/no-such-model", want)
    asked = _hub_asked(env)
    ok = ok and asked == ["/api/models/nobody/no-such-model?blobs=true"]
    return ok, "wanted %r; the door %r; the model lines before %r, after %r; the hub stub was asked %r" % (want, o, before, after, asked)


@S.check("J5.7/other-model-not-an-address-is-refused-and-asks-nothing", "not-an-address-asks-the-hub",
         "text that is not an address is sent to the hub")
def c_other_not_address(ctl, env):
    """Design 1: text that is not owner/model (nor a huggingface.co address) is refused in its sentence, and the hub is asked nothing."""
    want = "This is not a Hugging Face address. Write it as owner/model."
    ok, o, before, after = _refusal_case(ctl, env, "https://example.org/owner/model", want)
    asked = _hub_asked(env)
    ok = ok and asked == []
    return ok, "wanted %r; the door %r; the model lines before %r, after %r; the hub stub was asked %r" % (want, o, before, after, asked)


@S.check("J5.7/other-model-already-a-line-is-refused-and-asks-nothing", "duplicate-added",
         "the address of a model that is already a line adds it a second time")
def c_other_already(ctl, env):
    """Design 4: KnowLine's real address is the catalogue's line knowline08 (its page): refused as already in the list, before the hub is asked."""
    want = "This model is already in the list."
    ok, o, before, after = _refusal_case(ctl, env, "PelaAI/KnowLine-0.8B-Gen1", want)
    asked = _hub_asked(env)
    ok = ok and asked == []
    return ok, "wanted %r; the door %r; the model lines before %r, after %r; the hub stub was asked %r" % (want, o, before, after, asked)


def _added_fresh(ctl, env, slow=0.0):
    """A clean list, then the model at HUB_COPY added (its line `added.1`), its download staged on the loopback stub with small stand-in files (the file
    `decision-model-<id, ':' as '-' and '/' as '--'>.json` in the run folder, read when the address is staged: DecisionModels.useRigBase)."""
    _door_fresh(ctl, env)
    o = _door_check(ctl, HUB_COPY)
    run = getattr(ctl, "dir", "")
    digest = hashlib.sha256(b"A" * LAYA_SIZE).hexdigest()
    with open(os.path.join(run, "decision-model-hub-%s.json" % HUB_COPY.replace("/", "--")), "w", encoding="utf-8") as f:
        json.dump({"files": [{"path": p, "bytes": LAYA_SIZE, "sha256": digest} for p in LAYA_FILES]}, f)
    env["laya"].piece_delay = slow
    staged = soft_why(ctl, "fixture", "provider-endpoint", "model:hub:" + HUB_COPY, env["laya"].url)
    row = (rows(ctl, "mac-settings-ai.decisions.added.1") or [{}])[0]
    if row.get("model") != "hub:" + HUB_COPY or row.get("state") != "absent":
        raise Unreadable("the added line reads %r after the check %r (download staging answered %r): the case cannot start from a known state" % (row, o, staged))
    return row


def _added_wait(ctl, pred, seconds=30):
    end = time.monotonic() + seconds
    while True:
        r = (rows(ctl, "mac-settings-ai.decisions.added.1") or [{}])[0]
        if pred(r) or time.monotonic() >= end:
            return r
        time.sleep(0.2)


@S.check("J5.7/an-added-model-downloads-then-becomes-the-model-in-use", "added-never-in-use",
         "the added model downloads and never becomes the model in use")
def c_added_download(ctl, env):
    """Design 5: the added line downloads, through the same row and store doors as a catalogue line, from the loopback stub, and when its files are here it
    becomes the model in use; the core keeps the choice (`decision_model/model`)."""
    first = _added_fresh(ctl, env, slow=0.25)
    act(ctl, "click", "mac-settings-ai.decisions.added.1.download")
    during = _added_wait(ctl, lambda r: r.get("state") == "downloading", 5)
    after = _added_wait(ctl, lambda r: r.get("in_use") is True, 30)
    env["laya"].piece_delay = 0
    kept = core_or_none(ctl, "decision_model/model")
    laya = _laya_row(ctl)
    ok = (first.get("remove") is True and during.get("state") == "downloading" and during.get("wanted") is True and during.get("remove") is False
          and after.get("state") == "ready" and after.get("in_use") is True and after.get("remove") is False and kept == "hub:" + HUB_COPY and laya.get("in_use") is False)
    return ok, "before %r; while it downloads %r; after %r; the core's model %r; Laya in use %r" % (_brief(first), _brief(during), _brief(after), kept, laya.get("in_use"))


@S.check("J5.7/remove-takes-an-added-model-away", "remove-keeps-the-line",
         "Remove leaves the added model in the list")
def c_added_remove(ctl, env):
    """Design 5: an added line that is not in use and not downloading carries Remove; Remove takes its line and its entry in added-models.json away."""
    first = _added_fresh(ctl, env)
    act(ctl, "click", "mac-settings-ai.decisions.added.1.remove")
    lines = [r.get("line") for r in rows(ctl, LINES)]
    models = [r.get("model") for r in rows(ctl, "mac-settings-ai.decisions.models")]
    path = os.path.join(getattr(ctl, "dir", ""), "Models", "added-models.json")
    kept = json.load(open(path, encoding="utf-8")) if os.path.exists(path) else []
    ok = (first.get("remove") is True and lines == _catalogue_ids() + ["other"] and "hub:" + HUB_COPY not in models
          and all(k.get("id") != "hub:" + HUB_COPY for k in kept))
    return ok, "before Remove %r; after: the card's lines %r, the model lines %r, added-models.json %r" % (_brief(first), lines, models, kept)



@S.check("J5.7/jev-key-saved-is-not-shown-as-connected", "saved-shown-as-connected",
         "a Jev key the core only saved is shown as connected")
def c_jev_saved(ctl, env):
    _open(ctl)
    act(ctl, "click", "mac-settings-ai.decisions.jev.key.field")
    act(ctl, "type", D["jev_key"])
    act(ctl, "click", "mac-settings-ai.decisions.jev.connect")
    r = rows(ctl, "mac-settings-ai.decisions.jev.key")[0]
    return (r["state"], r["shown"]) == ("not checked", "Saved, not checked"), "row %r" % r


@S.check("J5.7/jev-with-the-fixture-key-says-connected", "jev-check-never-answers",
         "the Test button asks Jev nothing: the key row never says connected")
def c_jev_connected(ctl, env):
    """docs/DECISION-MODEL.md:63 (kept by v2 §4): Jev's key goes to the Keychain, and "a Test button that sends one fixed sample" says whether Jev takes it.
    Before (hid-ai round 3): the case waited for a key check at save time against the stub's /validate, which the core does not make
    (core/crates/engine/src/ai_settings.rs:236 "No request is made"). Now: Connect asks nothing (the control), the Test click sends one sample to
    the loopback stub's /v1/systemone with the key in the header, and the row says connected."""
    _open(ctl)
    act(ctl, "click", "mac-settings-ai.decisions.jev.key.field")
    act(ctl, "type", D["jev_key"])
    act(ctl, "click", "mac-settings-ai.decisions.jev.connect")
    first = rows(ctl, "mac-settings-ai.decisions.jev.key")[0]["state"]      # the control: the key is saved and not yet connected (J5.7/jev-key-saved-is-not-shown-as-connected)
    at_save = len(env["jev"].requests)                                       # the control: saving the key asks Jev nothing
    act(ctl, "click", "mac-settings-ai.decisions.jev.test")
    state, end = rows(ctl, "mac-settings-ai.decisions.jev.key")[0]["state"], time.monotonic() + JEV_CHECK_WAIT
    while state != "connected" and time.monotonic() < end:
        time.sleep(0.1)
        state = rows(ctl, "mac-settings-ai.decisions.jev.key")[0]["state"]
    asked = [r for r in env["jev"].requests if r["path"] == "/v1/systemone" and r["auth"] == "Bearer " + D["jev_key"]]      # the loopback stub, never a real provider
    shown = rows(ctl, "mac-settings-ai.decisions.jev.key")[0]["shown"]
    return state == "connected" and first != "connected" and at_save == 0 and len(asked) == 1, \
        "right after Connect the row says %r and Jev was asked %d time(s); after Test, within %.0f s it says %r (shown %r); the stub on 127.0.0.1 got %d sample(s) with the fixture key" % (
            first, at_save, JEV_CHECK_WAIT, state, shown, len(asked))


@S.check("J5.7/consent-is-given-and-withdrawn-in-the-core", "withdraw-keeps-consent",
         "Withdraw leaves the consent in the core")
def c_consent(ctl, env):
    _open(ctl)
    start = (rows(ctl, "mac-settings-ai.decisions.jev.consent")[0]["given"], core(ctl, "decision_consent/jev"))
    act(ctl, "click", "mac-settings-ai.decisions.jev.consent.allow")
    given = (rows(ctl, "mac-settings-ai.decisions.jev.consent")[0]["given"], core(ctl, "decision_consent/jev"))
    act(ctl, "click", "mac-settings-ai.decisions.jev.consent.withdraw")
    gone = (rows(ctl, "mac-settings-ai.decisions.jev.consent")[0]["given"], core(ctl, "decision_consent/jev"))
    # The core answers the consent path with a record, `{"needed": bool, "given": bool}` (core/crates/contracts/src/decision_consent.rs:42; the engine test
    # core/crates/engine/tests/engine_label_model_r5.rs:354): Jev is hosted, so needed is true, and given is what the owner said (false until Allow).
    rec = lambda g: {"given": g, "needed": True}
    return (start, given, gone) == ((False, rec(False)), (True, rec(True)), (False, rec(False))), "before %r, after Allow %r, after Withdraw %r (row, core)" % (start, given, gone)


@S.check("J5.7/provider-state-words-come-from-one-mapping", "state-word-printed-raw",
         "a view prints the core's raw state word instead of the sentence")
def c_state_map(ctl, env):
    src = _src(ctl, "SettingsAIMac/AIWords.swift", "SettingsAIMac/DecisionsHomeView.swift", "SettingsAIMac/ProvidersView.swift", "SettingsAIMac/AISettingsStore.swift")
    words = src["SettingsAIMac/AIWords.swift"]
    cases = all(("case .%s:" % c) in words for c in ("notConnected", "checking", "connected", "refused", "notChecked"))
    sentence = 'Words.text("Saved, not checked")' in words
    raw = [f for f in ("SettingsAIMac/DecisionsHomeView.swift", "SettingsAIMac/ProvidersView.swift") if "aiProviderStateKey" in src[f]]
    usable = "providerStateIsUsable(s: s)" in src["SettingsAIMac/AISettingsStore.swift"]
    return cases and sentence and not raw and usable, "all five states mapped in AIWords.state: %s; the sentence for not checked: %s; views printing a raw word: %r; not checked counts as usable: %s" % (cases, sentence, raw, usable)


@S.check("J5.7/laya-chosen-says-what-happens-next", "laya-line-missing",
         "with Laya chosen and ready the pane does not say when mail is sorted")
def c_laya_next(ctl, env):
    _laya_fresh(ctl)
    act(ctl, "click", "mac-settings-ai.decisions.local.download")
    _laya_wait(ctl)
    r = rows(ctl, "mac-settings-ai.decisions.local.next")
    return len(r) == 1 and r[0]["key"] == "R" and "sorted as it arrives" in r[0]["text"], "rows %r" % r


FIRST_READ_MS = 50   # the owner: "<50ms for any operation"; one frame at 60 Hz is 16.7 ms, so a state shown on the first read is inside it


@S.check("J5.7/laya-download-click-shows-downloading-then-ready", "laya-click-silent",
         "the row keeps showing the Download button for a read after the click")
def c_laya_progress(ctl, env):
    _laya_fresh(ctl)
    with _slow_laya(ctl, env):
        act(ctl, "click", "mac-settings-ai.decisions.local.download")
        t0 = time.monotonic()
        r1 = _laya_row(ctl)
        f1 = _fig(r1, t0)
        time.sleep(LAYA_TICKS_S)                      # two progress ticks: the figure the row shows has had the time to move
        r2 = _laya_row(ctl)
        f2 = _fig(r2, t0)
        end = _laya_wait(ctl, 15)
        f3 = _fig(end, t0)
    ok = r1["state"] == "downloading" and r2["state"] == "downloading" and r2["received"] > r1["received"] and end["state"] == "ready"
    return ok, "right after the click %r, next read %r, end %r (each: state, bytes received, words, seconds since the click)" % (f1, f2, f3)


@S.check("J5.7/laya-download-click-shows-downloading-within-one-frame", "laya-first-read-slow",
         "the row says Downloading only after more than 50 ms (the owner's rule), not within one frame of the click")
def c_laya_one_frame(ctl, env):
    # item 0aea8b37 ("within one frame"): the click returns once the app has run its door; the row is read at once and must already say Downloading, and that
    # read must answer inside FIRST_READ_MS. 16.7 ms is one frame at 60 Hz; the owner's bound is 50 ms for any operation.
    _laya_fresh(ctl)
    with _slow_laya(ctl, env):
        act(ctl, "click", "mac-settings-ai.decisions.local.download")
        t0 = time.monotonic()
        r1 = _laya_row(ctl)
        first_ms = (time.monotonic() - t0) * 1000
        _laya_wait(ctl, 15)
    return r1["state"] == "downloading" and first_ms <= FIRST_READ_MS, "the first read after the click returned says %r (%r) and answered in %.1f ms (limit %d ms)" % (
        r1["state"], r1.get("words"), first_ms, FIRST_READ_MS)


@S.check("J5.7/laya-second-click-while-downloading-changes-only-the-progress", "laya-second-click-restarts",
         "a second click restarts the download from zero")
def c_laya_second(ctl, env):
    _laya_fresh(ctl)
    with _slow_laya(ctl, env):
        act(ctl, "click", "mac-settings-ai.decisions.local.download")
        t0 = time.monotonic()
        # the download starts a moment after the click: until the row says so it has no `received` (main81: KeyError 'received' on the very first read)
        r1 = _laya_row(ctl)
        for _ in range(20):
            if r1.get("state") == "downloading" and "received" in r1:
                break
            time.sleep(0.1)
            r1 = _laya_row(ctl)
        f1 = _fig(r1, t0)
        time.sleep(LAYA_TICKS_S)
        r1b = _laya_row(ctl)
        f1b = _fig(r1b, t0)
        act(ctl, "click", "mac-settings-ai.decisions.local.download")
        # round 8: read until the figure moves past the one before the second click (a bound of 3 s), never one fixed sleep: the stub writes a
        # piece every LAYA_PIECE_S, and one read LAYA_TICKS_S later could fall between two pieces (main92, and this check's own self-test once:
        # 630000 twice). A restart reads 0 or less than before, at every read.
        r2, end = _laya_row(ctl), time.monotonic() + 3.0
        restarted = r2.get("received", -1) < r1b.get("received", -1)
        while r2.get("state") == "downloading" and r2.get("received", -1) <= r1b.get("received", -1) and time.monotonic() < end:
            time.sleep(0.1)
            r2 = _laya_row(ctl)
            restarted = restarted or r2.get("received", -1) < r1b.get("received", -1)
        f2 = _fig(r2, t0)
    ok = not restarted and r2.get("state") == "downloading" and r2.get("received", -1) > r1b.get("received", -1) > r1.get("received", -2)
    return ok, "figures %r, %r, then after the second click %r (each: state, bytes received, words, seconds since the first click)" % (f1[1], f1b[1], f2[1]) + \
        "; reads %r, %r, %r" % (f1, f1b, f2)


@S.check("J5.7/laya-figures-keep-moving-while-downloading", "laya-progress-frozen",
         "the downloading row's figure stays where it was while the file keeps arriving")
def c_laya_moving(ctl, env):
    _laya_fresh(ctl)
    figs = []
    with _slow_laya(ctl, env):
        act(ctl, "click", "mac-settings-ai.decisions.local.download")
        for _ in range(3):
            time.sleep(LAYA_TICKS_S)
            r = _laya_row(ctl)
            figs.append((r["state"], r.get("received")))
    ok = all(s == "downloading" for s, _ in figs) and all(b > a for (_, a), (_, b) in zip(figs, figs[1:]))
    return ok, "the row after each of three waits of two progress ticks: %r" % figs


@S.check("J5.7/laya-download-failure-names-the-file-and-retry-works", "laya-failed-no-retry",
         "a failed download offers no Retry")
def c_laya_failed(ctl, env):
    _laya_fresh(ctl)
    bad = LAYA_FILES[-1]
    env["laya"].notfound = {bad}
    act(ctl, "click", "mac-settings-ai.decisions.local.download")
    f = _laya_wait(ctl)
    env["laya"].notfound = set()
    act(ctl, "click", "mac-settings-ai.decisions.local.download")
    end = _laya_wait(ctl)
    ok = f["state"] == "failed" and bad in f["words"] and f.get("retry") is True and f.get("key") == "\u21a9" and end["state"] == "ready"
    return ok, "failed row %r, after Retry %r" % ({k: f.get(k) for k in ("state", "retry")}, end["state"])


THIS_MAC = ("127.0.0.1", "::1", "[::1]", "localhost")
NO_BASE_WAIT_S = 5      # the refused download ends 'failed' this soon (the address refuses at once)


def peers_of(lsof_text):
    """(peers off this Mac, peers on this Mac) of `lsof -a -i -n -P -p <pid>` text: the part after `->` of each connection line (the same reading as
    offenders_of in checks/firstrun-check.py:931, which this check does not import)."""
    import re
    off, here = [], []
    for line in lsof_text.splitlines():
        m = re.search(r"->(\[?[0-9a-fA-F:.]+\]?):(\d+)", line)
        if m:
            (here if m.group(1) in THIS_MAC else off).append("%s:%s" % (m.group(1), m.group(2)))
    return off, here


def _net_sample(ctl):
    """One look at the app's sockets from outside it. A stand-in probe (self-test) answers the text; the real one runs lsof on the pid in <run folder>/rig.pid.
    Anything that stops the look (no pid, no lsof) is NOT JUDGED, never a pass."""
    if NETPROBE["fn"] is not None:
        return NETPROBE["fn"](ctl)
    try:
        pid = int(open(os.path.join(getattr(ctl, "dir", ""), "rig.pid"), encoding="utf-8").read().strip())
    except (OSError, ValueError) as e:
        raise Unreadable("the copy's pid is unknown (<run folder>/rig.pid: %s): its sockets cannot be sampled" % e)
    try:
        r = subprocess.run(["lsof", "-a", "-i", "-n", "-P", "-p", str(pid)], capture_output=True, text=True, timeout=10)
    except (OSError, subprocess.TimeoutExpired) as e:
        raise Unreadable("lsof could not be run on pid %d (%s: %s)" % (pid, type(e).__name__, e))
    if r.returncode not in (0, 1):      # 1 is lsof's answer for "no such sockets"
        raise Unreadable("lsof on pid %d exited %d: %s" % (pid, r.returncode, " ".join(r.stderr.split())[:160]))
    return r.stdout


@S.check("J5.7/laya-download-with-no-loopback-base-asks-nothing-outside-this-mac", "laya-download-asks-outside",
         "with no loopback address for the decision model a click on Download makes the copy ask a host outside this Mac")
def c_laya_no_base(ctl, env):
    # item ab99d16d: the copy has NO loopback base for the weights (`fixture provider-endpoint laya ""` puts the real address back, as the owner's copy has it; a
    # rig copy then asks RigNet.refusing, 127.0.0.1:1, Sources/CoreBridge/CoreHost.swift:87). Download must end 'failed' in words, the stubs must hear nothing, and the
    # sockets read from outside (lsof) must show no peer off this Mac. The pass line prints both values.
    _laya_fresh(ctl)
    staged = soft_why(ctl, "fixture", "provider-endpoint", "laya", "")
    if staged != "ok":
        raise Unreadable("the loopback base could not be taken away (`fixture provider-endpoint laya \"\"` answered %r)" % staged)
    _net_sample(ctl)                                   # not judged here when the sockets cannot be read: before any click
    off, here, samples = [], [], 0
    act(ctl, "click", "mac-settings-ai.decisions.local.download")
    t0 = time.monotonic()
    r = _laya_row(ctl)
    while True:
        o, h = peers_of(_net_sample(ctl))
        samples += 1
        off += [p for p in o if p not in off]
        here += [p for p in h if p not in here]
        if r.get("state") in ("failed", "ready") or time.monotonic() - t0 > NO_BASE_WAIT_S:
            break
        time.sleep(0.2)
        r = _laya_row(ctl)
    secs = time.monotonic() - t0
    for _ in range(2):                                 # two looks after the end: a late connect would show
        time.sleep(0.25)
        o, h = peers_of(_net_sample(ctl))
        samples += 1
        off += [p for p in o if p not in off]
        here += [p for p in h if p not in here]
    asked = {k: stub_seen(env[k]) for k in ("laya", "prov", "jev")}
    n_asked = sum(v["HEAD"] + v["GET"] + v["POST"] for v in asked.values())
    words = str(r.get("words", ""))
    ok = (r.get("state") == "failed" and secs <= NO_BASE_WAIT_S and words.startswith("Could not download the decision model") and "Click Retry." in words
          and n_asked == 0 and not off)
    return ok, "row %s in %.1f s, words %r; stub requests: %d; peers off this Mac: %d %s; lsof peers read: %d (this Mac) over %d samples" % (
        r.get("state"), secs, words[:40], n_asked, len(off), off, len(here), samples)


@S.check("J5.7/laya-download-started-in-the-inspector-shows-in-the-pane", "laya-state-not-shared",
         "the label inspector's download is invisible to the Settings row")
def c_laya_shared(ctl, env):
    _laya_fresh(ctl)
    core_kind = ctl.uistate().get("core")
    if core_kind != "rust":
        # LabelFlowModel.layaWords (Sources/Features/LabelsContactsMac/LabelFlowModel.swift:37) answers nil unless CoreHost.shared.kind == .rust, and the door
        # (`downloadLaya`, :60) refuses without it: on the fixture core the inspector's download door never exists. Not the model being ready.
        raise Excluded("real engine only: this run's core is %r; the label inspector's download door exists only on the real core (LabelFlowModel.layaWords and downloadLaya), "
                       "so the test engine can never judge it" % core_kind)
    with _slow_laya(ctl, env):
        act(ctl, "click", "mac-labels.inspector.laya.download")
        r = _laya_row(ctl)
    return r["state"] == "downloading", "the Settings row reads %r after the download was started from the inspector" % r["state"]


@S.check("J5.7/rig-copy-keeps-models-out-of-the-owners-folder", "rig-models-in-owner-folder",
         "the Laya store of a rig copy writes under the owner's Application Support")
def c_rig_folder(ctl, env):
    files = ("OnDeviceAI/Core/LayaModelStore.swift", "OnDeviceAI/Core/ModelInstaller.swift", "Embedding/GemmaWeights.swift", "Dictation/DictationFeature.swift",
             "SettingsAccountsMac/AccountIconStore.swift", "../Design/SendWave/SendWavePerfLog.swift", "OnDeviceAI/Core/ModelsFolder.swift",
             "../CoreBridge/CoreHost.swift")
    src = _src(ctl, *files)
    stores = files[:-2]
    own = [f for f in stores if "applicationSupportDirectory" in src[f]]
    through = [f for f in stores if "ModelsFolder." not in src[f]]
    mf = src["OnDeviceAI/Core/ModelsFolder.swift"]
    rig = "CorePlace.current(env: env)" in mf and "if place.rig" in mf
    # the loopback rule moved into one definition (RigNet, CoreHost.swift): the Laya base goes through `ModelsFolder.loopbackOnly`, which asks `RigNet.isThisMac`, and
    # the rig-only base is read behind `CorePlace.current(env: env).rig` (the older literal list in ModelsFolder.swift is gone since 1542f4b1)
    host = src["../CoreBridge/CoreHost.swift"]
    loop = ('static let thisMacHosts: Set<String> = ["127.0.0.1", "::1", "localhost"]' in host and "RigNet.isThisMac(u.host)" in mf
            and "guard CorePlace.current(env: env).rig" in mf)
    ok = not own and not through and rig and loop
    return ok, "stores reading Application Support directly %r; stores not asking ModelsFolder %r; a rig copy gets its run folder: %s; the Laya base is loopback only and rig only: %s (the run of the real paths: _captures/settingsaimac-r4/laya-download/run.log)" % (own, through, rig, loop)


# ---------------------------------------------------------------- round 9: the look the owner picked (W1): amber dot, one line, one button
@S.check("J5.9/laya-missing-says-what-and-offers-one-button-with-its-key", "laya-button-no-key",
         "the Download button of the Laya row has no key cap")
def c_laya_w1(ctl, env):
    _laya_fresh(ctl)
    r = _laya_row(ctl)
    ok = r["state"] == "absent" and r["words"] == "Not on this Mac yet. 843 MB, downloaded once." and r["button"] == "Download" and r["key"] == "\u21a9"
    return ok, "row %r" % {k: r.get(k) for k in ("state", "words", "button", "key")}


@S.check("J5.9/laya-downloading-shows-figures-and-a-cancel-with-esc", "laya-no-cancel",
         "the downloading row offers no Cancel")
def c_laya_cancel_shown(ctl, env):
    _laya_fresh(ctl)
    with _slow_laya(ctl, env):
        act(ctl, "click", "mac-settings-ai.decisions.local.download")
        r = _laya_row(ctl)
    ok = r["state"] == "downloading" and r["words"].startswith("Downloading ") and " of 843 MB" in r["words"] and r.get("cancel") is True and r.get("key") == "esc"
    return ok, "row %r" % {k: r.get(k) for k in ("state", "words", "cancel", "key")}


@S.check("J5.9/laya-cancel-stops-the-download-and-offers-download-again", "laya-cancel-ignored",
         "Cancel leaves the download running")
def c_laya_cancel(ctl, env):
    _laya_fresh(ctl)
    with _slow_laya(ctl, env):
        act(ctl, "click", "mac-settings-ai.decisions.local.download")
        t0 = time.monotonic()
        first = _fig(_laya_row(ctl), t0)
        time.sleep(LAYA_TICKS_S)                      # pieces are still to come when Cancel is clicked
        pre = _fig(_laya_row(ctl), t0)                # the row just before Cancel: still downloading, or already ready (then Cancel had nothing to stop)
        act(ctl, "click", "mac-settings-ai.decisions.local.cancel")
        r = _laya_row(ctl)
        fr = _fig(r, t0)
        time.sleep(LAYA_TICKS_S)
        r2 = _laya_row(ctl)
        fr2 = _fig(r2, t0)
    return (r["state"], r2["state"], r.get("button")) == ("absent", "absent", "Download"), \
        "after Cancel: %r then %r, button %r; reads (state, bytes received, words, seconds since the click): after the click %r, just before Cancel %r, after Cancel %r, then %r" % (
            r["state"], r2["state"], r.get("button"), first, pre, fr, fr2)


MR = "mac-settings-ai.meaning"


def _meaning(ctl):
    return rows(ctl, MR)[0]


@S.check("J5.9/meaning-missing-has-one-download-button", "meaning-absent-no-button",
         "the search model row has no Download button when the model is missing")
def c_meaning_absent(ctl, env):
    _open(ctl)
    r = _meaning(ctl)
    return (r["state"], r["button"], r.get("button_ax")) == ("notDownloaded", "Download", MR + ".download"), "row %r" % {k: r.get(k) for k in ("state", "button", "button_ax")}


@S.check("J5.9/meaning-failed-says-why-and-offers-retry", "meaning-failed-no-retry",
         "a failed search model download offers no Retry")
def c_meaning_failed(ctl, env):
    _open(ctl)
    # round 8: the download fails because the stub answers 404 for one of its files (before: an empty `provider-endpoint meaning`, which no code
    # reads, so the copy downloaded and main95 read 'notDownloaded'); the line is read until it leaves "notDownloaded" and "downloading"
    _small_list_once(ctl, env)
    _open(ctl)
    env["laya"].notfound = {LAYA_FILES[0]}
    try:
        act(ctl, "click", MR + ".download")
        r = _after_click(ctl, ("notDownloaded", "downloading"), secs=6.0)
    finally:
        env["laya"].notfound = set()
    return (r["state"], r["button"]) == ("failed", "Retry") and len(r["words"]) > 0, "row %r" % {k: r.get(k) for k in ("state", "button", "words")}


@S.check("J5.9/meaning-downloads-with-moving-figures-then-is-ready", "meaning-click-silent",
         "the search model row shows no progress after the click")
def c_meaning_progress(ctl, env):
    _small_list_once(ctl, env)
    _open(ctl)
    staged = soft_why(ctl, "fixture", "provider-endpoint", "meaning", env["laya"].url)
    # round 13: the small list (750 kB from this Mac's stub) arrived inside one progress tick on main104 ((downloading, 0) twice, then ready), so
    # nothing could move on the line; the stub sends its last file in ten pieces MEANING_PROGRESS_PIECE_S apart, so the download lasts several ticks
    env["laya"].piece_delay = MEANING_PROGRESS_PIECE_S
    act(ctl, "click", MR + ".download")
    r1 = _after_click(ctl, ("notDownloaded", "failed"), secs=3.0)      # round 8: the line answers the click a moment later
    if r1["state"] == "failed":
        # what the code tested: the row reads 'failed' straight after the click. The address of the search model is read only from the launch environment
        # (Sources/Features/Embedding/MeaningModelRig.swift:30, WHOOSH_RIG_EMBED_BASE); `fixture provider-endpoint meaning <url>` is staged above but no code reads it,
        # so no socket verb can set it (the verb that is missing: a `meaning` endpoint honoured at MeaningModelRig.swift:30).
        words = str(r1.get("words", ""))
        named = " The row names WHOOSH_RIG_EMBED_BASE: this rig copy was launched without a loopback address for it." if "WHOOSH_RIG_EMBED_BASE" in words else ""
        raise Unreadable("the search model row reads 'failed' right after the click; its words %r; the staging of the stub at %s was answered %r; the stub was asked %r.%s" % (
            words, env["laya"].url, staged, stub_seen(env["laya"]), named))
    try:
        # reads until the figure moves past the one after the click (a bound), then until the download ends (a bound)
        r2, t_end = _meaning(ctl), time.monotonic() + 6
        while r2["state"] == "downloading" and r2.get("received", 0) <= r1.get("received", 0) and time.monotonic() < t_end:
            time.sleep(0.15)
            r2 = _meaning(ctl)
        end, t_end = r2, time.monotonic() + 15
        while end["state"] not in ("ready", "failed") and time.monotonic() < t_end:
            time.sleep(0.2)
            end = _meaning(ctl)
    finally:
        env["laya"].piece_delay = 0
    ok = r1["state"] == "downloading" and r2["state"] == "downloading" and r2.get("received", -1) > r1.get("received", 1 << 60) and end["state"] == "ready"
    return ok, "after the click %r, next %r, end %r" % ((r1["state"], r1.get("received")), (r2["state"], r2.get("received")), end["state"])


# ---- round 10 (lane fix-checks): the search model's download across a relaunch, and the index figures on the two screens that say them ------------------
MEANING_PROGRESS_PIECE_S = 0.4   # round 13: the progress case's pieces: ten of them make the last file last 4 s (16 ticks of 250 ms)
MEANING_PIECE_S = 1.5       # seconds between the ten pieces of the last file: the download outlives a relaunch (15 s), so there is something to find again
MEANING_RELAUNCH_WAIT_S = 40
MEANING_SHOW_WITHIN_S = 3   # the relaunched line must show the running download this soon after the page is opened (it reads the store on appear)
INDEX_DONE, INDEX_QUEUED = 40, 60      # the figures the index cases set: 40 of 100


def real_relaunch(ctl):
    """`sh scripts/launch-one.sh --relaunch` from inside the driven command: the copy is quit by its pid and started again on the same run folder with the same
    launch environment (WHOOSH_RIG_EMBED_BASE included); the socket comes back. Outside a driven command ($D unset) or without WHOOSH_RIG_HIDDEN=1 the script
    refuses and the case is NOT JUDGED with the script's words."""
    if not os.environ.get("D"):
        raise Unreadable("this check is not run inside scripts/launch-one.sh ($D is unset): the copy cannot be relaunched")
    ctl.close()
    try:
        r = subprocess.run(["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), "--relaunch"], capture_output=True, text=True, timeout=180, cwd=ROOT)
    except subprocess.TimeoutExpired:
        raise Unreadable("`launch-one.sh --relaunch` did not finish within 180 s")
    if r.returncode != 0:
        raise Unreadable("`launch-one.sh --relaunch` refused or failed (exit %d): %s" % (r.returncode, " ".join((r.stdout + r.stderr).split())[-300:]))
    try:
        ctl.connect()
    except CouldNotDrive as e:
        raise Unreadable("the copy did not come back after the relaunch: %s" % e)


def relaunch(ctl):
    (RELAUNCH["fn"] or real_relaunch)(ctl)


def _meaning_folder(ctl):
    """The rig copy's own folder of the search model (MeaningModelRig.runModelsFolder: <run folder>/models/embeddinggemma-2); never the owner's."""
    run = getattr(ctl, "dir", "")
    if not run or not os.path.exists(os.path.join(run, "whoosh.sock")):
        raise Unreadable("no rig run folder (WHOOSH_SOCKET_DIR): the search model's folder is unknown")
    owner = os.path.realpath(os.path.expanduser("~/Library/Application Support/Whoosh"))
    folder = os.path.realpath(os.path.join(run, "models", "embeddinggemma-2"))
    if folder == owner or folder.startswith(owner + os.sep):
        raise Unreadable("the models folder %s is the owner's, never emptied by a check" % folder)
    return run, folder


def _small_model_list(run):
    """`embed-fixture.json` in the run folder swaps the search model's file list for a small one (MeaningModelRig.current): the five files of the loopback stub,
    each LAYA_SIZE bytes of 'A'. The copy reads it when it starts, which is why the case relaunches once before it clicks."""
    digest = hashlib.sha256(b"A" * LAYA_SIZE).hexdigest()
    with open(os.path.join(run, "embed-fixture.json"), "w", encoding="utf-8") as f:
        json.dump({"files": [{"path": p, "bytes": LAYA_SIZE, "sha256": digest} for p in LAYA_FILES]}, f)


def _small_list_once(ctl, env):
    """Round 8: the search model's small file list (the stub's files) is what every search-model case downloads; the copy reads it only at launch
    (MeaningModelRig.current), so it is written and the copy relaunched ONCE, before the first such case, then the endpoints are staged again.
    Before, only the relaunch case wrote it: the cases before it asked the pinned list (config.json ...) of the stub, which serves only its five
    files, and read that 404 (main93, main95: "404 for config.json" while the stub's log, cleared by each case's reset, showed no request)."""
    if env.get("small_list"):
        return
    run, _folder = _meaning_folder(ctl)
    _small_model_list(run)
    relaunch(ctl)
    reset(ctl, env)
    env["small_list"] = True


def _after_click(ctl, leave, secs=4.0):
    """The line answers the click a moment later (e97a73ef9: it follows the store's announcements): read until it leaves the states in `leave`."""
    r, end = _meaning(ctl), time.monotonic() + secs
    while r["state"] in leave and time.monotonic() < end:
        time.sleep(0.15)
        r = _meaning(ctl)
    return r


def _failed_without_base(r, env, staged):
    words = str(r.get("words", ""))
    named = " The row names WHOOSH_RIG_EMBED_BASE: this rig copy was launched without a loopback address for it." if "WHOOSH_RIG_EMBED_BASE" in words else ""
    return Unreadable("the search model row reads 'failed' right after the click; its words %r; this check's WHOOSH_RIG_EMBED_PORT is %r (launch with WHOOSH_RIG_EMBED_PORT=<free port>: "
                      "scripts/launch-one.sh turns it into the copy's WHOOSH_RIG_EMBED_BASE and the stub listens there); the stub at %s was staged %r and asked %r.%s" % (
                          words, os.environ.get("WHOOSH_RIG_EMBED_PORT"), env["laya"].url, staged, stub_seen(env["laya"]), named))


@S.check("J5.9/meaning-download-survives-a-relaunch", "meaning-lost-on-relaunch",
         "a search model download running when the copy quits is not on the line after the relaunch")
def c_meaning_relaunch(ctl, env):
    run, folder = _meaning_folder(ctl)
    shutil.rmtree(folder, ignore_errors=True)
    if os.path.exists(folder):
        raise Unreadable("the rig's search model folder %s could not be emptied" % folder)
    _small_model_list(run)
    env["laya"].piece_delay = MEANING_PIECE_S
    try:
        relaunch(ctl)                                   # the copy reads the small file list when it starts
        _open(ctl)
        start = _meaning(ctl)["state"]
        if start != "notDownloaded":
            raise Unreadable("the search model row reads %r on a copy whose models folder %s was emptied, not 'notDownloaded': no known start" % (start, folder))
        staged = soft_why(ctl, "fixture", "provider-endpoint", "meaning", env["laya"].url)
        act(ctl, "click", MR + ".download")
        # round 8: the copy quits only once the download is really under way (the stub got a GET); main95 relaunched before any request
        end = time.monotonic() + 5
        while not [g for g in env["laya"].gets if g[0] == "GET"] and time.monotonic() < end:
            time.sleep(0.1)
        r1 = _meaning(ctl)
        if r1["state"] == "failed":
            raise _failed_without_base(r1, env, staged)
        relaunch(ctl)                                   # the copy quits in the middle of the download and starts again
        _open(ctl)
        t0 = time.monotonic()
        seen = []
        while time.monotonic() - t0 < MEANING_RELAUNCH_WAIT_S:
            r = _meaning(ctl)
            seen.append((r["state"], r.get("received")))
            if r["state"] in ("ready", "failed"):
                break
            if r["state"] == "notDownloaded" and time.monotonic() - t0 > MEANING_SHOW_WITHIN_S:
                break                                   # the line shows nothing of a download the store holds: no need to wait for the rest
            time.sleep(0.3)
    finally:
        env["laya"].piece_delay = 0
    figs = [b for s, b in seen if s == "downloading" and isinstance(b, (int, float))]
    rising = len(set(figs)) >= 2 and figs == sorted(figs)
    ok = r1["state"] == "downloading" and seen[-1][0] == "ready" and rising and seen[0][0] in ("downloading", "ready")
    return ok, "before the quit %r; after the relaunch the line read %d times, first %r, last %r, figures %r (rising %s)" % (
        (r1["state"], r1.get("received")), len(seen), seen[0], seen[-1], figs[:6], rising)


def _meaning_ready(ctl, env):
    """The search model on this Mac, for the index line (it is drawn only then): a model left ready by the case before is used as it is; else the small list is
    downloaded from the loopback stub, as the progress case does. A copy that cannot download (no loopback address) makes the case NOT JUDGED, with its words."""
    _small_list_once(ctl, env)
    _open(ctl)
    r = _meaning(ctl)
    if r["state"] == "ready":
        return r
    staged = soft_why(ctl, "fixture", "provider-endpoint", "meaning", env["laya"].url)
    if r["state"] in ("notDownloaded", "failed"):
        act(ctl, "click", MR + ".download")
        _after_click(ctl, ("notDownloaded", "failed"), secs=3.0)
    end = time.monotonic() + 30
    r = _meaning(ctl)
    while r["state"] == "downloading" and time.monotonic() < end:
        time.sleep(0.3)
        r = _meaning(ctl)
    if r["state"] != "ready":
        raise Unreadable("the search model is not on this Mac (row reads %r, words %r; staging of the stub %r): the index line is drawn only when it is, so the case cannot be judged" % (
            r["state"], r.get("words"), staged))
    return r


def _set_index(ctl, indexed, queued):
    try:
        ctl.send("fixture", "index-progress", str(indexed), str(queued))
    except Unreadable as e:
        raise Unreadable("this build does not serve `fixture index-progress <indexed> <queued>` (finding F1 of docs/lanes/gap-search-cases.md: add the case to Sources/ControlSocket/FixtureVerbs.swift, "
                         "next to `search-hold`): %s" % e)


@S.check("J5.9/meaning-index-line-gives-the-figures-while-building", "meaning-index-line-never",
         "the search model row shows no line with the builder's figures while the index is built")
def c_index_settings(ctl, env):
    _meaning_ready(ctl, env)
    _set_index(ctl, INDEX_DONE, INDEX_QUEUED)
    try:
        r = _meaning(ctl)
    finally:
        _set_index(ctl, 0, 0)
    got = (r.get("indexing"), r.get("index_done"), r.get("index_total"), r.get("index_text"))
    return got == (True, INDEX_DONE, INDEX_DONE + INDEX_QUEUED, "Indexing your mail: 40 of 100"), "row index fields %r, expected %r" % (
        got, (True, INDEX_DONE, INDEX_DONE + INDEX_QUEUED, "Indexing your mail: 40 of 100"))


@S.check("J5.9/meaning-index-line-says-how-many-mails-are-indexed-when-the-build-is-done", "meaning-index-line-stuck",
         "the search model row keeps its building line, or says nothing, after the build is done")
def c_index_settings_done(ctl, env):
    _meaning_ready(ctl, env)
    try:
        _set_index(ctl, INDEX_DONE, INDEX_QUEUED)
        building = _meaning(ctl).get("index_text")               # the line is on screen while the build runs, then the build ends
        _set_index(ctl, INDEX_DONE + INDEX_QUEUED, 0)
        r = _meaning(ctl)
    finally:
        soft(ctl, "fixture", "index-progress", "0", "0")         # `fixture reset` does not clear the builder's figures (docs/lanes/fix-rest-app.md): leave none behind
    got = (building, r.get("indexing"), r.get("index_text"))
    want = ("Indexing your mail: 40 of 100", False, "100 mails indexed.")
    return got == want, "row index text while building, indexing flag and text when done %r, expected %r" % (got, want)


@S.check("J5.9/status-line-says-the-share-while-the-index-builds", "status-index-never",
         "the main window's sync line says nothing of the index while it is built")
def c_index_status(ctl, env):
    _set_index(ctl, INDEX_DONE, INDEX_QUEUED)
    try:
        r = rows(ctl, "mac-main.sync.line")[0]
        building = (r.get("indexing"), r.get("index_done"), r.get("index_total"), r.get("index_text"))
    finally:
        _set_index(ctl, INDEX_DONE + INDEX_QUEUED, 0)
    try:
        r = rows(ctl, "mac-main.sync.line")[0]
        done = (r.get("indexing"), r.get("index_text"))
    finally:
        soft(ctl, "fixture", "index-progress", "0", "0")         # leave no figures behind (`fixture reset` does not clear them)
    want = ((True, INDEX_DONE, INDEX_DONE + INDEX_QUEUED, "indexing 40 %"), (False, None))
    return (building, done) == want, "while building %r, when done %r, expected %r" % (building, done, want)


@S.check("J5.9/both-model-rows-are-the-one-picked-drawing", "rows-drawn-twice",
         "a decision model's line draws its own amber button, not the picked W1 one")
def c_one_drawing(ctl, env):
    """The owner's pick W1 (one drawing for a model that is not here: the amber dot, one line, the amber button with Return, the bar, Cancel with
    Esc). The search model's row uses the shared row; since the list of decision models (pick M1, SPEC A133) each of its lines is a
    DecisionModelRow, which must be made of the same parts."""
    src = _src(ctl, "SettingsAIMac/DecisionsHomeView.swift", "SettingsAIMac/MeaningSetupRow.swift", "SettingsAIMac/ModelSetupRow.swift", "SettingsAIMac/AIPaneView.swift",
               "SettingsAIMac/DecisionModelRow.swift")
    W1 = ("AmberDot()", "AmberButton(", "ProgressBar(", ".keyboardShortcut(.defaultAction)", ".keyboardShortcut(.cancelAction)", 'key: "esc"', '"\\u{21A9}"')
    row = src["SettingsAIMac/ModelSetupRow.swift"]
    parts = all(t in row for t in W1)
    laya = "DecisionModelRow(" in src["SettingsAIMac/DecisionsHomeView.swift"] and all(t in src["SettingsAIMac/DecisionModelRow.swift"] for t in W1)
    meaning = "ModelSetupRow(" in src["SettingsAIMac/MeaningSetupRow.swift"] and "MeaningSetupRow()" in src["SettingsAIMac/AIPaneView.swift"] and "MeaningModelDoor.downloadId" in src["SettingsAIMac/MeaningSetupRow.swift"]
    return parts and laya and meaning, "the row has the amber dot, the amber button with return, the bar and Cancel with esc: %s; each decision model's line is made of the same parts: %s; the search model row uses it, sits in the AI and agents pane and keeps the store's ids: %s" % (parts, laya, meaning)


# ---- the evidence each case prints on a red (see EVIDENCE above) ------------------------------------------------------------------------------
def _ev_laya(env):
    return [("laya_row", lambda c: _laya_row(c)), ("laya_stub_asked", lambda c: stub_seen(env["laya"])), ("endpoint_staging", lambda c: env.get("endpoints")),
            ("core_kind", lambda c: c.uistate().get("core")), ("windows", ev_windows)]


def _ev_inspector(env):
    return _ev_laya(env) + [("decision_provider", ev_core("decision_model/provider")), ("inspector_empty_row", ev_rows("mac-labels.inspector.empty")),
                            ("door_frame", ev_measure("mac-labels.inspector.laya.download")), ("surface", lambda c: c.uistate().get("surface"))]


def _ev_jev(env):
    return [("engine_rows", ev_rows(DE, "id", "chosen", "refusal")), ("core_provider", ev_core("decision_model/provider")),
            ("jev_key_row", ev_rows("mac-settings-ai.decisions.jev.key")), ("jev_consent_row", ev_rows("mac-settings-ai.decisions.jev.consent")),
            ("surface", lambda c: c.uistate().get("surface")), ("windows", ev_windows)]


def _ev_heading(env):
    return [("heading_row", ev_rows(AGENTS_NEW_ROW)), ("new_agent_button_frame", ev_measure(A["agents_new"])),
            ("heading_frame", ev_measure(AGENTS_NEW_ROW)), ("agent_cards", lambda c: len(c.rows(A["agents_card"]))),
            ("engine_rows_drawn", lambda c: len(c.rows(DE))), ("surface", lambda c: c.uistate().get("surface")), ("windows", ev_windows)]


def _ev_meaning(env):
    return [("meaning_row", lambda c: _meaning(c)), ("stub_asked", lambda c: stub_seen(env["laya"])), ("endpoint_staging", lambda c: env.get("endpoints"))]


EVIDENCE.update({
    "J5.6/new-agent-row-and-button-have-two-names": _ev_heading,
    "J5.7/jev-needs-its-key-then-is-saved": _ev_jev, "J5.7/jev-without-consent-is-refused-in-words": _ev_jev,
    "J5.7/laya-download-click-shows-downloading-then-ready": _ev_laya, "J5.7/laya-second-click-while-downloading-changes-only-the-progress": _ev_laya,
    "J5.9/laya-cancel-stops-the-download-and-offers-download-again": _ev_laya,
    "J5.7/laya-download-click-shows-downloading-within-one-frame": _ev_laya,
    "J5.7/laya-download-started-in-the-inspector-shows-in-the-pane": _ev_inspector,
    "J5.7/laya-download-with-no-loopback-base-asks-nothing-outside-this-mac": _ev_laya,
    "J5.9/meaning-downloads-with-moving-figures-then-is-ready": _ev_meaning,
    "J5.9/meaning-download-survives-a-relaunch": _ev_meaning,
    "J5.9/meaning-index-line-gives-the-figures-while-building": _ev_meaning,
    "J5.9/meaning-index-line-says-how-many-mails-are-indexed-when-the-build-is-done": _ev_meaning,
    "J5.9/status-line-says-the-share-while-the-index-builds": lambda env: [("sync_line", ev_rows("mac-main.sync.line")), ("meaning_row", lambda c: _meaning(c))],
})


if __name__ == "__main__":
    sys.exit(main(S))
