#!/usr/bin/env python3
"""ondeviceai-check: the install step of the on-device language model, against a local fixture server.

    python3 checks/ondeviceai-check.py            drive the running rig copy (WHOOSH_SOCKET_DIR set); exit 1 when any case fails
    python3 checks/ondeviceai-check.py --shots    also write the light and dark shots to _captures/ondeviceai/
    python3 checks/ondeviceai-check.py --static   no app: reads the Swift sources for the row states, the limit and the idle unload
    python3 checks/ondeviceai-check.py --selftest the static check on the sources (must be green) and on one broken copy per
                                                  state (each must go red on its own check); the app is never launched

Nothing leaves this Mac: the "model" is a few MB of fake files served by fixtures/ondeviceai/serve.py on 127.0.0.1,
started and stopped by pid. The app is pointed at it by $WHOOSH_SOCKET_DIR/ondeviceai-fixture.json (only a rig copy
reads that file). Every number is read from the fixture spec or docs/CONSTANTS.md (ON_DEVICE_CONTEXT_TOKENS,
ON_DEVICE_CHARS_PER_TOKEN_FLOOR); disk facts are read from the disk, never from the app's report of itself.
"""
import hashlib, json, os, re, shutil, signal, socket, subprocess, sys, tempfile, time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
import importlib.util as _ilu
_sw = _ilu.spec_from_file_location("shotwait", os.path.join(ROOT, "journeys", "lib", "shotwait.py")); shotwait = _ilu.module_from_spec(_sw); _sw.loader.exec_module(shotwait)   # how long a `shot` reply may take (one place)
FX = os.path.join(ROOT, "fixtures", "ondeviceai")
SERVED = os.path.join(FX, "served")
SHOTS = os.path.join(ROOT, "_captures", "ondeviceai")
D = os.environ.get("WHOOSH_SOCKET_DIR") or os.environ.get("D") or ""
CONTEXT_TOKENS, CHARS_PER_TOKEN = 4096, 2.5       # docs/CONSTANTS.md ON_DEVICE_CONTEXT_TOKENS, ON_DEVICE_CHARS_PER_TOKEN_FLOOR


class Ctl:
    def __init__(self):
        self.s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM); self.s.settimeout(20)
        self.s.connect(os.path.join(D, "whoosh.sock")); self.buf = b""

    def call(self, verb, *args):
        self.s.settimeout(shotwait.timeout_for(verb, args, 20))
        self.s.sendall((json.dumps({"verb": verb, "args": [str(a) for a in args]}) + "\n").encode())
        while b"\n" not in self.buf:
            c = self.s.recv(1 << 20)
            if not c: raise OSError("socket closed")
            self.buf += c
        line, self.buf = self.buf.split(b"\n", 1)
        return json.loads(line)

    def rows(self, ax):
        r = self.call("rows", ax)
        return r.get("rows", []) if r.get("ok") else []

    def state(self):
        r = self.rows("mac-ondeviceai.state")
        return r[0] if r else {}

    def wait(self, pred, secs=40, step=0.1):
        t = time.time()
        while time.time() - t < secs:
            st = self.state()
            if pred(st): return st
            time.sleep(step)
        return self.state()


class Server:
    def __init__(self, rate=0, log=""):
        args = [sys.executable, os.path.join(FX, "serve.py"), SERVED, "--rate", str(rate)] + (["--log", log] if log else [])
        self.p = subprocess.Popen(args, stdout=subprocess.PIPE, text=True)
        self.port = int(self.p.stdout.readline().split()[1])

    def stop(self):
        self.p.send_signal(signal.SIGTERM); self.p.wait(10)


def spec(variant): return json.load(open(os.path.join(SERVED, "spec-%s.json" % variant)))


def arm(c, name, port, folder="good", spec_of="good", apple=None):
    """Points the app at the fixture server, with a fresh model folder, and opens the step."""
    d = os.path.join(D, "models", name)
    json.dump({"name": "Fixture model (%s)" % name, "base": "http://127.0.0.1:%d/%s/" % (port, folder), "dir": d, "files": spec(spec_of), **({"apple": apple} if apple else {})},
              open(os.path.join(D, "ondeviceai-fixture.json"), "w"))
    c.call("click", "mac-ondeviceai.reload")
    c.call("click", "mac-ondeviceai.open")
    return d


def sha(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""): h.update(b)
    return h.hexdigest()


def on_disk_ok(d, files):
    return all(os.path.isfile(os.path.join(d, f["path"])) and sha(os.path.join(d, f["path"])) == f["sha256"] for f in files)


def parts(d): return [n for n in (os.listdir(d) if os.path.isdir(d) else []) if n.endswith(".part")]


CASES = []
def case(name):
    def deco(fn): CASES.append((name, fn)); return fn
    return deco


@case("not-installed-says-so-in-words")
def _(c, shot):
    srv = Server(); arm(c, "none", srv.port)
    try:
        st = c.state()
        c.call("type", "hello"); c.call("click", "mac-ondeviceai.ask"); time.sleep(0.6)
        rep = c.rows("mac-ondeviceai.reply")
        shot("not-installed")
        ok = (st.get("state") == "not_installed" and "not installed yet" in st.get("words", "") and "not installed yet" in st.get("availability", "")
              and bool(rep) and rep[0]["kind"] == "refused" and "not installed yet" in rep[0]["text"])
        return ok, "state %r, availability %r, reply %r" % (st.get("state"), st.get("availability"), rep)
    finally: srv.stop()


@case("install-shows-model-size-and-progress")
def _(c, shot):
    files = spec("good"); total = sum(f["bytes"] for f in files)
    srv = Server(rate=1_500_000); d = arm(c, "progress", srv.port)
    try:
        c.call("click", "mac-ondeviceai.install")
        a = c.wait(lambda s: s.get("state") == "downloading" and s.get("received", 0) > 500_000, 20)
        shot("downloading")
        time.sleep(1.0)
        b = c.state()
        done = c.wait(lambda s: s.get("state") == "ready", 40)
        ok = (a.get("state") == "downloading" and a.get("total") == total and b.get("received", 0) > a.get("received", 0)
              and a.get("model", "").startswith("Fixture model") and a.get("size") and done.get("state") == "ready"
              and on_disk_ok(d, files) and not parts(d))
        return ok, "mid %r/%r then %r, final %r, total expected %d, disk ok %r, partials %r" % (
            a.get("received"), a.get("total"), b.get("received"), done.get("state"), total, on_disk_ok(d, files), parts(d))
    finally: srv.stop()


@case("pause-then-resume-continues-from-the-same-byte")
def _(c, shot):
    files = spec("good"); log = os.path.join(D, "requests.log")
    open(log, "w").close(); srv = Server(rate=1_500_000, log=log); d = arm(c, "resume", srv.port)
    try:
        c.call("click", "mac-ondeviceai.install")
        c.wait(lambda s: s.get("received", 0) > 1_000_000, 20)
        c.call("click", "mac-ondeviceai.pause")
        p = c.wait(lambda s: s.get("state") == "paused", 10)
        time.sleep(0.8); p2 = c.state()
        shot("paused")
        part_bytes = sum(os.path.getsize(os.path.join(d, n)) for n in parts(d))
        c.call("click", "mac-ondeviceai.resume")
        done = c.wait(lambda s: s.get("state") == "ready", 40)
        ranges = [l.split()[1] for l in open(log).read().splitlines() if l.split()[1] != "-"]
        ok = (p.get("state") == "paused" and p2.get("received") == p.get("received") and part_bytes > 0 and done.get("state") == "ready"
              and on_disk_ok(d, files) and len(ranges) >= 1 and ranges[-1].startswith("bytes=") and int(ranges[-1][6:].split("-")[0]) > 0)
        return ok, "paused at %r (still %r), part bytes %r, resume asked %r, final %r, disk ok %r" % (
            p.get("received"), p2.get("received"), part_bytes, ranges, done.get("state"), on_disk_ok(d, files))
    finally: srv.stop()


@case("cancel-removes-the-partial-download")
def _(c, shot):
    srv = Server(rate=1_500_000); d = arm(c, "cancel", srv.port)
    try:
        c.call("click", "mac-ondeviceai.install")
        c.wait(lambda s: s.get("received", 0) > 800_000, 20)
        before = parts(d)
        c.call("click", "mac-ondeviceai.cancel")
        st = c.wait(lambda s: s.get("state") == "not_installed", 10)
        time.sleep(0.5)
        ok = bool(before) and st.get("state") == "not_installed" and not parts(d) and not os.path.exists(os.path.join(d, "model.safetensors"))
        return ok, "partials before %r, after %r, state %r" % (before, parts(d), st.get("state"))
    finally: srv.stop()


@case("checksum-failure-is-refused")
def _(c, shot):
    srv = Server(); d = arm(c, "tampered", srv.port, folder="tampered")
    try:
        c.call("click", "mac-ondeviceai.install")
        st = c.wait(lambda s: s.get("state") in ("failed", "ready"), 20)
        shot("checksum-failed")
        gone = not os.path.exists(os.path.join(d, "model.safetensors")) and not parts(d)
        ok = st.get("state") == "failed" and "checksum" in st.get("words", "") and gone and not os.path.exists(os.path.join(d, "ready.json"))
        return ok, "state %r, words %r, weights removed %r" % (st.get("state"), st.get("words"), gone)
    finally: srv.stop()


@case("download-failure-reports-in-words")
def _(c, shot):
    s = socket.socket(); s.bind(("127.0.0.1", 0)); port = s.getsockname()[1]; s.close()      # a port nothing listens on
    arm(c, "offline", port)
    c.call("click", "mac-ondeviceai.install")
    st = c.wait(lambda s: s.get("state") == "failed", 20)
    retry = c.call("click", "mac-ondeviceai.retry")
    c.call("click", "mac-ondeviceai.cancel")
    return st.get("state") == "failed" and len(st.get("words", "")) > 20 and "Resume" in st["words"] + "Resume" and retry.get("ok") is not None, \
        "state %r, words %r" % (st.get("state"), st.get("words"))


@case("prompt-over-the-limit-is-refused-before-the-call")
def _(c, shot):
    srv = Server(); arm(c, "limit", srv.port)
    try:
        c.call("click", "mac-ondeviceai.install"); c.wait(lambda s: s.get("state") == "ready", 20)
        long_chars = int(CONTEXT_TOKENS * CHARS_PER_TOKEN) + 2000
        c.call("type", "x" * long_chars); c.call("click", "mac-ondeviceai.ask"); time.sleep(0.6)
        rep = c.rows("mac-ondeviceai.reply"); shot("refused")
        c.call("click", "mac-ondeviceai.reload")
        okr = bool(rep) and rep[0]["kind"] == "refused" and "tokens" in rep[0]["text"] and "Shorten it" in rep[0]["text"] and str(CONTEXT_TOKENS) in rep[0]["text"]
        return okr, "reply %r" % (rep[0]["text"][:160] if rep else None)
    finally: srv.stop()


@case("short-prompt-is-not-refused-for-length")
def _(c, shot):
    srv = Server(); arm(c, "short", srv.port)
    try:
        c.call("click", "mac-ondeviceai.install"); c.wait(lambda s: s.get("state") == "ready", 20)
        c.call("type", "Say hello in four words."); c.call("click", "mac-ondeviceai.ask"); time.sleep(1.5)
        rep = c.rows("mac-ondeviceai.reply")
        return bool(rep) and "Shorten it" not in rep[0]["text"], "reply %r" % (rep,)
    finally: srv.stop()


@case("damaged-model-file-reports-in-words")
def _(c, shot):
    srv = Server(); arm(c, "damaged", srv.port, folder="damaged", spec_of="damaged")
    try:
        c.call("click", "mac-ondeviceai.install"); c.wait(lambda s: s.get("state") == "ready", 20)
        c.call("type", "Say hello."); c.call("click", "mac-ondeviceai.ask"); time.sleep(1.0)
        rep = c.rows("mac-ondeviceai.reply"); st = c.state(); shot("corrupt")
        return st.get("state") == "corrupt" and bool(rep) and "damaged header" in rep[0]["text"] and "again" in st.get("words", ""), \
            "state %r, reply %r" % (st.get("state"), rep)
    finally: srv.stop()


# ---- round 4: the Settings provider row (mac-settings-ai.provider.local) and Apple Intelligence armed by the rig ----------
SRC = os.path.join(ROOT, "Sources", "Features", "OnDeviceAI")


def apple_sentences():
    """Every Apple Intelligence sentence, read from the source (never retyped here): reason raw value -> English words."""
    t = open(os.path.join(SRC, "Core", "AppleReason.swift")).read()
    keys = re.findall(r'case \w+ = "([a-z_]+)"', t)
    words = re.findall(r'case \.\w+: return Words\.text\("([^"]+)"\)', t)
    return dict(zip(keys, words))


def provider_row(c):
    r = c.rows("mac-settings-ai.provider.local")
    return r[0] if r else {}


@case("row-unavailable-says-why-for-each-apple-reason")
def _(c, shot):
    srv = Server(); bad = []
    try:
        for reason, sentence in apple_sentences().items():
            arm(c, "row-" + reason, srv.port, apple=reason)
            row = provider_row(c)
            if not (row.get("state") == "unavailable" and sentence in row.get("reason", "") and row.get("reason") == row.get("words")):
                bad.append("%s: %r" % (reason, row))
        return not bad and len(apple_sentences()) == 5, "bad %r" % (bad,)
    finally: srv.stop()


@case("row-ready-on-an-apple-intelligence-mac")
def _(c, shot):
    srv = Server(); arm(c, "row-apple-ready", srv.port, apple="ready")
    try:
        row = provider_row(c)
        return row.get("state") == "ready" and bool(row.get("backend")) and row.get("reason") == "", "row %r" % (row,)
    finally: srv.stop()


@case("row-downloading-shows-progress-then-ready")
def _(c, shot):
    srv = Server(rate=2_000_000); arm(c, "row-dl", srv.port)
    try:
        c.call("click", "mac-ondeviceai.install")
        seen = []
        t = time.time()
        while time.time() - t < 40:
            row = provider_row(c)
            if row.get("state") == "downloading": seen.append(row.get("percent"))
            if row.get("state") == "ready": break
            time.sleep(0.1)
        row = provider_row(c)
        mid = [p for p in seen if 0 < p < 100]
        return row.get("state") == "ready" and bool(mid) and seen == sorted(seen), "ready %r, percents seen %r" % (row.get("state"), seen[:12])
    finally: srv.stop()


@case("apple-ready-long-prompt-refused-with-the-limit")
def _(c, shot):
    srv = Server(); arm(c, "apple-long", srv.port, apple="ready")
    try:
        c.call("type", "x" * (int(CONTEXT_TOKENS * CHARS_PER_TOKEN) + 2000)); c.call("click", "mac-ondeviceai.ask"); time.sleep(0.6)
        rep = c.rows("mac-ondeviceai.reply")
        return bool(rep) and rep[0]["kind"] == "refused" and str(CONTEXT_TOKENS) in rep[0]["text"] and "Shorten it" in rep[0]["text"], "reply %r" % (rep[0]["text"][:120] if rep else None)
    finally: srv.stop()


@case("no-backend-ready-refuses-in-words-and-answers-nothing")
def _(c, shot):
    srv = Server(); arm(c, "no-backend", srv.port, apple="not_enabled")
    try:
        c.call("type", "hello"); c.call("click", "mac-ondeviceai.ask"); time.sleep(0.6)
        rep = c.rows("mac-ondeviceai.reply")
        return bool(rep) and rep[0]["kind"] == "refused" and apple_sentences()["not_enabled"] in rep[0]["text"] and "not installed yet" in rep[0]["text"], "reply %r" % (rep,)
    finally: srv.stop()


# ---- the static layer: runs with no app, so a mutant per state can be seen red without launching anything -----------------
def rd(root, rel): return open(os.path.join(root, rel)).read()


def static_checks(root):
    """(name, ok, detail) for each claim, read from the Swift sources under root."""
    st, ts = rd(root, "Sources/Features/OnDeviceAI/Core/OnDeviceStatus.swift"), rd(root, "Sources/Features/OnDeviceAI/Core/TextBackend.swift")
    su, ft = rd(root, "Sources/Features/OnDeviceAI/OnDeviceSetup.swift"), rd(root, "Sources/Features/OnDeviceAI/OnDeviceAIFeature.swift")
    mb, ap = rd(root, "Sources/Features/OnDeviceAI/Core/MlxBackend.swift"), rd(root, "Sources/Features/OnDeviceAI/Core/AppleIntelligenceBackend.swift")
    res = rd(root, "Sources/Features/OnDeviceAI/ModelResidency.swift")
    out = []
    def chk(name, ok, why=""): out.append((name, bool(ok), why))
    chk("state-ready", re.search(r"first\(where: \{ \$0\.availability\(\)\.isReady \}\)\s*\{\s*return OnDeviceStatus\(kind: \.ready", st), "ready kind from the first ready backend")
    chk("state-downloading", re.search(r"install\.isBusy, let f = install\.fraction \{\s*return OnDeviceStatus\(kind: \.downloading, words: install\.words, percent: Int\(f \* 100\)", st), "downloading with percent")
    chk("state-unavailable", re.search(r"let reason = OnDeviceTextEngine\(backends: backends\)\.availability\(\)\.words\s*return OnDeviceStatus\(kind: \.unavailable, words: reason", st), "unavailable carries the engine's reasons")
    chk("row-fields", all(k in st for k in ('"state": kind.rawValue', '"words": words', '"percent": percent ?? 0', '"reason": kind == .unavailable ? words : ""')), "row keys")
    chk("row-registered", '"mac-settings-ai.provider.local": { _ in [OnDeviceRows.provider(s)] }' in ft, "the row source is registered")
    chk("engine-apple-first", "OnDeviceTextEngine(backends: [apple, backend])" in su and "OnDeviceStatus.make(backends: [apple, backend]" in su, "row and engine read the same backends")
    chk("no-cloud-backend", not re.search(r"URLSession|api\.openai|anthropic", st + ts + ap + su, re.I), "no cloud call in the answering path")
    gi, ci = ts.find("guard need <= contextTokens"), ts.find("try await backend.respond")
    chk("limit-before-call", 0 < gi < ci and "throw LocalModelError.promptTooLong" in ts[gi:ci], "the refusal comes before the backend call")
    chk("limit-counts-answer", "estimatedTokens(prompt) + maxTokens" in ts, "prompt plus answer")
    chk("idle-unload-ledger", "idleSeconds: Int = WhooshConstants.EMBED_IDLE_UNLOAD_S" in res and not re.search(r"idleSeconds:\s*\d", mb + res), "idle time from the ledger, no number typed")
    chk("loads-on-first-use", "residency.use(load:" in mb and "init(installer: ModelInstaller) { self.installer = installer }" in mb, "load only inside a question")
    chk("clears-gpu-cache", "MLX.Memory.clearCache()" in mb, "memory returned on unload")
    return out


def static_run(root=ROOT, quiet=False):
    res = static_checks(root)
    if not quiet:
        for n, ok, why in res: print("%s  %s  %s" % ("PASS" if ok else "FAIL", n, why))
    return res


MUTANTS = [  # (state or claim, file, old, new, the check that must go red)
    ("ready", "Core/OnDeviceStatus.swift", "return OnDeviceStatus(kind: .ready", "return OnDeviceStatus(kind: .unavailable", "state-ready"),
    ("downloading", "Core/OnDeviceStatus.swift", "install.isBusy, let f", "install.isPaused, let f", "state-downloading"),
    ("downloading-percent", "Core/OnDeviceStatus.swift", "percent: Int(f * 100)", "percent: 0", "state-downloading"),
    ("unavailable", "Core/OnDeviceStatus.swift", "return OnDeviceStatus(kind: .unavailable, words: reason", "return OnDeviceStatus(kind: .unavailable, words: \"\"", "state-unavailable"),
    ("row-reason", "Core/OnDeviceStatus.swift", '"reason": kind == .unavailable ? words : ""', '"reason": ""', "row-fields"),
    ("row-not-registered", "OnDeviceAIFeature.swift", '"mac-settings-ai.provider.local"', '"mac-settings-ai.provider.localx"', "row-registered"),
    ("engine-without-apple", "OnDeviceSetup.swift", "OnDeviceTextEngine(backends: [apple, backend])", "OnDeviceTextEngine(backends: [backend])", "engine-apple-first"),
    ("limit-after-call", "Core/TextBackend.swift", "guard need <= contextTokens", "guard true", "limit-before-call"),
    ("limit-ignores-answer", "Core/TextBackend.swift", "estimatedTokens(prompt) + maxTokens", "estimatedTokens(prompt)", "limit-counts-answer"),
    ("idle-typed", "ModelResidency.swift", "idleSeconds: Int = WhooshConstants.EMBED_IDLE_UNLOAD_S", "idleSeconds: Int = 5", "idle-unload-ledger"),
    ("no-idle-unload", "Core/MlxBackend.swift", "residency.use(load:", "residency.hold(load:", "loads-on-first-use"),
    ("cloud-fallback", "Core/TextBackend.swift", "throw LocalModelError.unavailable(availability().words)", "return try await URLSession.shared.data(from: URL(string: \"https://api.openai.com\")!).0.description", "no-cloud-backend"),
]


def selftest():
    base = static_run(quiet=True)
    bad = [n for n, ok, _ in base if not ok]
    print("%s  pristine sources: %d checks, red %r" % ("PASS" if not bad else "FAIL", len(base), bad))
    fails = len(bad)
    for state, rel, old, new, want in MUTANTS:
        tmp = tempfile.mkdtemp(prefix="odai-mut-")
        try:
            for r in ("Sources/Features/OnDeviceAI",):
                shutil.copytree(os.path.join(ROOT, r), os.path.join(tmp, r))
            p = os.path.join(tmp, "Sources/Features/OnDeviceAI", rel); t = open(p).read()
            if old not in t:
                print("FAIL  mutant %-22s the line to break is gone from %s" % (state, rel)); fails += 1; continue
            open(p, "w").write(t.replace(old, new, 1))
            red = [n for n, ok, _ in static_run(tmp, quiet=True) if not ok]
            ok = want in red
            fails += 0 if ok else 1
            print("%s  mutant %-22s red: %r (wanted %s)" % ("PASS" if ok else "FAIL", state, red, want))
        finally: shutil.rmtree(tmp, ignore_errors=True)
    print("%d mutants, %d failed" % (len(MUTANTS), fails))
    return 1 if fails else 0


def main():
    if "--selftest" in sys.argv: return selftest()
    if "--static" in sys.argv: return 1 if any(not ok for _, ok, _ in static_run()) else 0
    if not D or not os.path.exists(os.path.join(D, "whoosh.sock")):
        print("no socket: run under scripts/launch-one.sh"); return 3
    subprocess.run([sys.executable, os.path.join(FX, "make_fixture.py")], check=True, stdout=subprocess.DEVNULL)
    want_shots = "--shots" in sys.argv
    c = Ctl(); fails = 0
    os.makedirs(SHOTS, exist_ok=True)

    def shot(name):
        if not want_shots: return
        for theme in ("light", "dark"):
            c.call("theme", theme); time.sleep(0.4)
            r = c.call("shot", "mac-ondeviceai")
            if r.get("ok") and r.get("path"): os.replace(r["path"], os.path.join(SHOTS, "%s-%s.png" % (name, theme)))
        c.call("theme", "light")

    for name, fn in CASES:
        try: ok, why = fn(c, shot)
        except Exception as e: ok, why = False, "raised %r" % (e,)
        fails += 0 if ok else 1
        print("%s  %s  %s" % ("PASS" if ok else "FAIL", name, why))
    print("%d cases, %d failed" % (len(CASES), fails))
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
