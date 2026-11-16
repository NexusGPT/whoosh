#!/usr/bin/env python3
"""relay-core check: drives the RUNNING push relay (relay/target/debug/relay) on 127.0.0.1.

    python3 checks/relay-core-check.py              run every check once against the built relay
    python3 checks/relay-core-check.py --selftest   run once clean (must be green), then once per MUTANT:
                                                    a copy of relay/ with one defect planted, rebuilt, re-driven;
                                                    the named check must go red on it
Prints `N checks, F failed, U not judged` and a MUTANTS table. Exit 1 when F > 0 or U > 0, or when a
selftest mutant stays green. docs/TESTING.md: a check is green only after it has been seen red on its mutant.

Nothing leaves this Mac: the relay binds 127.0.0.1 and talks to a LOCAL APNs stand-in (`relay apns-stub`)
started here; no real APNs host is contacted and no real key is used (a throwaway ES256 key per run).
Env: RELAY_BIN (use this binary instead of building), WHOOSH_REPO (repo root).
Other scripts reuse this file as a library: `import_lib()` then `lib.main(name, checks, mutants)`.
"""
import http.client, json, os, re, shutil, sqlite3, subprocess, sys, tempfile, threading, time

ROOT = os.environ.get("WHOOSH_REPO") or os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RELAY_DIR = os.path.join(ROOT, "relay")
CARGO = os.path.expanduser("~/.cargo/bin/cargo")
CAP = os.path.join(ROOT, "_captures", "relay-core")
MUTANT_TARGET = os.path.join(ROOT, "_captures", ".tmp", "relay-core-mutants", "target")
FIXTURE_TOKEN = "00fc13adff785122b4ad28809a3420982341241421348097878e577c991de8f0"
GONE_TOKEN = "f" * 64
SIGNAL_TOKEN = "check-signal-token"
CHECKS = []   # (name, fn)
STATE = {"keys": None}


class Fail(Exception):
    pass


class Unjudged(Exception):
    pass


def ensure(cond, msg):
    if not cond:
        raise Fail(msg)


def check(name):
    def deco(fn):
        CHECKS.append((name, fn))
        return fn
    return deco


def cargo_version():
    m = re.search(r'^version = "([^"]+)"', open(os.path.join(RELAY_DIR, "Cargo.toml")).read(), re.M)
    return m.group(1)


# ---------------------------------------------------------------- the rig
class Proc:
    def __init__(self, argv, env, log_path):
        self.log = open(log_path, "w+")
        self.p = subprocess.Popen(argv, env=env, stdout=self.log, stderr=subprocess.STDOUT)
        self.url = None
        end = time.time() + 20
        while time.time() < end and self.url is None:
            self.log.flush()
            m = re.search(r"listening on (http://\S+)", open(log_path).read())
            if m:
                self.url = m.group(1)
            elif self.p.poll() is not None:
                raise Fail("process exited at boot: " + open(log_path).read()[-400:])
            else:
                time.sleep(0.05)
        if self.url is None:
            raise Fail("process never printed 'listening on'")

    def stop(self):
        if self.p.poll() is None:
            self.p.terminate()
            try:
                self.p.wait(5)
            except subprocess.TimeoutExpired:
                self.p.kill()
        self.log.close()


class Relay:
    def __init__(self, proc, db, stub_log):
        self.proc, self.db, self.stub_log = proc, db, stub_log
        self.host, self.port = proc.url[len("http://"):].split(":")

    def req(self, method, path, body=None, headers=None):
        c = http.client.HTTPConnection(self.host, int(self.port), timeout=10)
        h = dict(headers or {})
        data = None
        if body is not None:
            data = json.dumps(body).encode() if not isinstance(body, bytes) else body
            h.setdefault("content-type", "application/json")
        c.request(method, path, body=data, headers=h)
        r = c.getresponse()
        raw = r.read()
        c.close()
        try:
            parsed = json.loads(raw)
        except ValueError:
            parsed = None
        return r.status, {k.lower(): v for k, v in r.getheaders()}, raw, parsed

    def get(self, path):
        s, _, _, j = self.req("GET", path)
        return s, j

    def post(self, path, body, headers=None):
        s, _, _, j = self.req("POST", path, body, headers)
        return s, j

    def register(self, token, routing, env="sandbox"):
        return self.post("/v1/registrations", {"device_token": token, "routing_id": routing, "environment": env})

    def unregister(self, token, routing):
        return self.req("DELETE", "/v1/registrations/%s/%s" % (routing, token))[0]

    def signal(self, signal_id, routing, ref="history-42"):
        return self.post("/v1/inbound/loopback", {"signal_id": signal_id, "routing_id": routing, "opaque_ref": ref},
                         {"authorization": "Bearer " + SIGNAL_TOKEN})

    def health(self):
        return self.get("/v1/health")[1]

    def wait_jobs(self, state, at_least, secs=15):
        end = time.time() + secs
        while time.time() < end:
            h = self.health()
            if h["jobs"][state] >= at_least:
                return h
            time.sleep(0.05)
        raise Fail("jobs.%s never reached %d; health %s" % (state, at_least, json.dumps(self.health())))

    def stub_lines(self):
        if not self.stub_log or not os.path.exists(self.stub_log):
            return []
        return [json.loads(l) for l in open(self.stub_log) if l.strip()]

    def sql(self, query):
        c = sqlite3.connect("file:%s?mode=ro" % self.db, uri=True)
        try:
            return c.execute(query).fetchall()
        finally:
            c.close()


class Ctx:
    """One check's world: temp dir, its relays and stubs, torn down when the check ends."""
    def __init__(self, relay_bin, stub_bin):
        self.relay_bin, self.stub_bin = relay_bin, stub_bin
        self.dir = tempfile.mkdtemp(prefix="relay-core-check-")
        self.procs = []

    def keys(self):
        if STATE["keys"] is None:
            if not shutil.which("openssl"):
                raise Unjudged("openssl is not installed: no throwaway APNs key can be made")
            d = tempfile.mkdtemp(prefix="relay-core-keys-")
            ec, p8, pub = (os.path.join(d, n) for n in ("ec.pem", "AuthKey_CHECK.p8", "pub.pem"))
            for argv in (["openssl", "ecparam", "-name", "prime256v1", "-genkey", "-noout", "-out", ec],
                         ["openssl", "pkcs8", "-topk8", "-nocrypt", "-in", ec, "-out", p8],
                         ["openssl", "ec", "-in", ec, "-pubout", "-out", pub]):
                r = subprocess.run(argv, capture_output=True, text=True)
                if r.returncode != 0:
                    raise Unjudged("openssl failed: " + r.stderr.strip())
            STATE["keys"] = (p8, pub)
        return STATE["keys"]

    def stub(self):
        p8, pub = self.keys()
        log = os.path.join(self.dir, "stub-%d.log" % len(self.procs))
        proc = Proc([self.stub_bin, "apns-stub", "--bind", "127.0.0.1:0", "--pub-key", pub, "--ok-token", FIXTURE_TOKEN,
                     "--gone-token", GONE_TOKEN, "--log", os.path.join(self.dir, "stub-events.jsonl")],
                    dict(os.environ), log)
        self.procs.append(proc)
        return proc.url, os.path.join(self.dir, "stub-events.jsonl")

    def relay(self, apns=False, apns_url=None, extra_env=None):
        env = {k: v for k, v in os.environ.items() if not k.startswith("RELAY_")}
        db = os.path.join(self.dir, "relay-%d.sqlite3" % len(self.procs))
        env.update({"RELAY_DB": db, "RELAY_BIND": "127.0.0.1:0", "RELAY_SIGNAL_TOKEN": SIGNAL_TOKEN})
        stub_log = None
        if apns:
            url, stub_log = (apns_url, None) if apns_url else self.stub()
            p8, _ = self.keys()
            env.update({"RELAY_APNS_KEY_PATH": p8, "RELAY_APNS_KEY_ID": "CHECKKEY01", "RELAY_APNS_TEAM_ID": "CHECKTEAM1",
                        "RELAY_APNS_TOPIC": "app.whoosh.check", "RELAY_APNS_SANDBOX_URL": url, "RELAY_APNS_PRODUCTION_URL": url})
        env.update(extra_env or {})
        proc = Proc([self.relay_bin], env, os.path.join(self.dir, "relay-%d.log" % len(self.procs)))
        self.procs.append(proc)
        return Relay(proc, db, stub_log)

    def close(self):
        for p in self.procs:
            p.stop()
        shutil.rmtree(self.dir, ignore_errors=True)


# ---------------------------------------------------------------- build
def build(src_dir, target_dir=None):
    env = dict(os.environ)
    if target_dir:
        env["CARGO_TARGET_DIR"] = target_dir
    r = subprocess.run([CARGO, "build", "-j", "2"], cwd=src_dir, env=env, capture_output=True, text=True)
    if r.returncode != 0:
        raise Fail("cargo build failed:\n" + r.stderr[-800:])
    return os.path.join(target_dir or os.path.join(src_dir, "target"), "debug", "relay")


# ---------------------------------------------------------------- running
def run_checks(checks, relay_bin, stub_bin):
    out = []
    for name, fn in checks:
        ctx = Ctx(relay_bin, stub_bin)
        try:
            detail = fn(ctx) or ""
            out.append((name, "pass", detail))
        except Fail as e:
            out.append((name, "fail", str(e)))
        except Unjudged as e:
            out.append((name, "unjudged", str(e)))
        except Exception as e:  # a check that crashed did not judge anything
            out.append((name, "unjudged", "check crashed: %r" % e))
        finally:
            ctx.close()
    return out


def summary(results):
    n = len(results)
    f = sum(1 for r in results if r[1] == "fail")
    u = sum(1 for r in results if r[1] == "unjudged")
    return n, f, u


def print_results(results):
    for name, state, detail in results:
        print("  %-8s %s%s" % (state.upper(), name, ("  - " + detail) if detail and state != "pass" else ""))
    n, f, u = summary(results)
    print("%d checks, %d failed, %d not judged" % (n, f, u))


def apply_mutant(src_copy, m):
    path = os.path.join(src_copy, m["file"][len("relay/"):])
    text = open(path).read()
    if text.count(m["old"]) != 1:
        raise Fail("mutant %s: expected one occurrence of the original text in %s, found %d" % (m["id"], m["file"], text.count(m["old"])))
    open(path, "w").write(text.replace(m["old"], m["new"]))


def selftest(name, checks, mutants, clean_bin):
    clean = run_checks(checks, clean_bin, clean_bin)
    print("CLEAN RUN")
    print_results(clean)
    rows, ok = [], summary(clean)[1:] == (0, 0)
    for m in mutants:
        tmp = tempfile.mkdtemp(prefix="relay-core-mutant-")
        try:
            dst = os.path.join(tmp, "relay")
            shutil.copytree(RELAY_DIR, dst, ignore=shutil.ignore_patterns("target"))
            apply_mutant(dst, m)
            mbin = build(dst, MUTANT_TARGET)
            res = {r[0]: r for r in run_checks([c for c in checks if c[0] == m["check"]], mbin, clean_bin)}
            state = res[m["check"]][1]
            rows.append((m["check"], m["what"], state == "fail", res[m["check"]][2][:90]))
        except Fail as e:
            rows.append((m["check"], m["what"], False, "mutant did not build or apply: " + str(e)[:90]))
        finally:
            shutil.rmtree(tmp, ignore_errors=True)
    print("MUTANTS\n| check | mutation made | seen red |")
    print("|---|---|---|")
    for check_name, what, red, why in rows:
        print("| %s | %s | %s |" % (check_name, what, "yes" if red else "NO (%s)" % why))
    os.makedirs(CAP, exist_ok=True)
    json.dump([{"check": r[0], "mutation": r[1], "seen_red": r[2]} for r in rows], open(os.path.join(CAP, name + ".mutants.json"), "w"), indent=1)
    return ok and all(r[2] for r in rows)


def print_last_mutants(name, mutants):
    p = os.path.join(CAP, name + ".mutants.json")
    last = {r["check"] + r["mutation"]: r["seen_red"] for r in json.load(open(p))} if os.path.exists(p) else {}
    print("MUTANTS\n| check | mutation made | seen red (last --selftest) |")
    print("|---|---|---|")
    for m in mutants:
        v = last.get(m["check"] + m["what"])
        print("| %s | %s | %s |" % (m["check"], m["what"], "not yet run" if v is None else ("yes" if v else "NO")))


def main(name, checks, mutants, pending=()):
    relay_bin = os.environ.get("RELAY_BIN")
    try:
        relay_bin = relay_bin or build(RELAY_DIR)
    except Fail as e:
        print(e)
        print("%d checks, 0 failed, %d not judged" % (len(checks), len(checks)))
        return 1
    if "--selftest" in sys.argv:
        good = selftest(name, checks, mutants, relay_bin)
        return 0 if good else 1
    results = run_checks(checks, relay_bin, relay_bin)
    print_results(results)
    for line in pending:
        print("PENDING (not a check): " + line)
    print_last_mutants(name, mutants)
    n, f, u = summary(results)
    return 1 if (f or u) else 0


# ---------------------------------------------------------------- the checks (fn-relay-core)
ALLOWED = {"push_job": {"apns_reason", "apns_status", "device_token", "enqueued_at", "environment", "finished_at", "job_id", "opaque_ref", "routing_id", "signal_id", "state"},
           "registration": {"device_token", "environment", "registered_at", "routing_id"},
           "tracked_message": {"created_at", "token"}, "tracking_event": {"event_id", "kind", "occurred_at", "token"}}
FORBIDDEN = ("password", "passwd", "secret", "credential", "oauth", "refresh", "access", "auth", "subject", "body", "snippet",
             "preview", "sender", "from", "recipient", "address", "email", "content", "text", "html", "url", "ip_", "agent")


def schema_of(relay):
    tables = [r[0] for r in relay.sql("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")]
    return {t: {r[1] for r in relay.sql("PRAGMA table_info(%s)" % t)} for t in tables}


@check("health-is-self-describing")
def _(c):
    r = c.relay()
    s, h = r.get("/v1/health")
    ensure(s == 200, "health answered %s" % s)
    ensure(h.get("service") == "whoosh-relay", "service name is %r" % h.get("service"))
    ensure(h.get("version") == cargo_version(), "health says version %r, Cargo.toml says %r" % (h.get("version"), cargo_version()))
    ensure(isinstance(h.get("queue_depth"), int), "queue_depth is %r" % h.get("queue_depth"))
    ensure("GET /v1/health" in h.get("routes", []), "the route list does not describe the service")
    return "version %s" % h["version"]


@check("register-unregister-round-trip")
def _(c):
    r = c.relay()
    s, b = r.register(FIXTURE_TOKEN, "acct-1")
    ensure(s == 201 and b["created"] is True, "first register: %s %s" % (s, b))
    s, b = r.register(FIXTURE_TOKEN, "acct-1")
    ensure(s == 200 and b["created"] is False, "second register is not idempotent: %s %s" % (s, b))
    ensure(r.get("/v1/registrations/acct-1")[1]["count"] == 1, "lookup after register is not 1")
    ensure(r.unregister(FIXTURE_TOKEN, "acct-1") == 204, "unregister did not answer 204")
    ensure(r.get("/v1/registrations/acct-1")[1]["count"] == 0, "lookup after unregister is not 0")
    ensure(r.unregister(FIXTURE_TOKEN, "acct-1") == 404, "unregistering twice did not answer 404")
    return "register, repeat, lookup, unregister, repeat"


@check("unregister-removes-only-its-own-pair")
def _(c):
    r = c.relay()
    r.register(FIXTURE_TOKEN, "acct-1"); r.register(GONE_TOKEN, "acct-1"); r.register(FIXTURE_TOKEN, "acct-2")
    ensure(r.unregister(FIXTURE_TOKEN, "acct-1") == 204, "unregister failed")
    ensure(r.get("/v1/registrations/acct-1")[1]["count"] == 1, "the other device on acct-1 was removed")
    ensure(r.get("/v1/registrations/acct-2")[1]["count"] == 1, "the same device on acct-2 was removed")


@check("routing-id-lookup-is-scoped")
def _(c):
    r = c.relay()
    r.register(FIXTURE_TOKEN, "acct-1"); r.register(GONE_TOKEN, "acct-2"); r.register("abcdef", "acct-2")
    got = [r.get("/v1/registrations/%s" % a)[1]["count"] for a in ("acct-1", "acct-2", "acct-none")]
    ensure(got == [1, 2, 0], "lookup counts for acct-1, acct-2, acct-none are %s, want [1, 2, 0]" % got)
    ensure(FIXTURE_TOKEN not in json.dumps(r.get("/v1/registrations/acct-1")[1]), "lookup hands a device token back")


@check("schema-has-no-credential-or-content-column")
def _(c):
    r = c.relay()
    r.health()
    schema = schema_of(r)
    ensure(schema == ALLOWED, "tables/columns differ from the allowlist: extra %s" % {t: sorted(schema.get(t, set()) - ALLOWED.get(t, set())) for t in schema if schema.get(t, set()) - ALLOWED.get(t, set())} or schema)
    bad = [(t, col) for t, cols in schema.items() for col in cols if any(w in col for w in FORBIDDEN)]
    ensure(not bad, "credential or content-looking columns: %s" % bad)
    return "%d tables, %d columns" % (len(schema), sum(len(v) for v in schema.values()))


@check("queue-depth-counts-queued-jobs-and-dedups-signals")
def _(c):
    r = c.relay()
    r.register(FIXTURE_TOKEN, "acct-1"); r.register(GONE_TOKEN, "acct-1")
    ensure(r.signal("sig-1", "acct-1")[1]["queued"] == 2, "a signal for 2 devices did not queue 2")
    ensure(r.health()["queue_depth"] == 2, "queue_depth after one signal is %s, want 2" % r.health()["queue_depth"])
    ensure(r.signal("sig-1", "acct-1")[1]["queued"] == 0, "the same signal id queued again")
    ensure(r.health()["queue_depth"] == 2, "queue_depth moved on a repeated signal")
    ensure(r.signal("sig-2", "acct-1")[1]["queued"] == 2 and r.health()["queue_depth"] == 4, "a new signal did not add 2")
    return "depth 2 -> 2 -> 4"


@check("relay-boots-degraded-without-apns")
def _(c):
    r = c.relay(apns=False)
    h = r.health()
    ensure(h["status"] == "degraded" and h["apns"]["state"] == "unconfigured", "health: %s" % {k: h[k] for k in ("status", "apns")})
    r2 = c.relay(apns=True, apns_url="http://127.0.0.1:1")   # configured, nothing listening
    ensure(r2.get("/v1/health")[0] == 200, "relay with an unreachable APNs did not serve health")
    r2.register(FIXTURE_TOKEN, "acct-1"); r2.signal("s", "acct-1")
    h = r2.wait_jobs("failed", 1)
    ensure(h["queue_depth"] == 0, "a failed send stayed in the queue")


@check("apns-sandbox-send-returns-200-for-the-fixture-token")
def _(c):
    r = c.relay(apns=True)
    r.register(FIXTURE_TOKEN, "acct-1")
    ensure(r.signal("history-42", "acct-1")[1]["queued"] == 1, "signal not queued")
    h = r.wait_jobs("sent", 1)
    ensure(h["queue_depth"] == 0 and h["status"] == "ok", "after send: depth %s status %s" % (h["queue_depth"], h["status"]))
    lines = r.stub_lines()
    ensure(len(lines) == 1, "stub saw %d requests, want 1" % len(lines))
    l = lines[0]
    ensure(l["token"] == FIXTURE_TOKEN and l["jwt_ok"] is True, "token/jwt: %s %s" % (l["token"], l["jwt_ok"]))
    ensure(l["topic"] == "app.whoosh.check" and l["push_type"] == "alert", "headers: %s %s" % (l["topic"], l["push_type"]))
    return "stub got 1 request, ES256 verified"


@check("apns-payload-is-a-signal-not-mail-content")
def _(c):
    r = c.relay(apns=True)
    r.register(FIXTURE_TOKEN, "acct-1")
    r.signal("history-42", "acct-1", ref="history-42")
    r.wait_jobs("sent", 1)
    body = r.stub_lines()[0]["body"]
    ensure(sorted(body.keys()) == ["aps", "whoosh"], "payload keys are %s" % sorted(body.keys()))
    ensure(sorted(body["whoosh"].keys()) == ["ref", "routing_id"], "whoosh keys are %s" % sorted(body["whoosh"].keys()))
    flat = json.dumps(body).lower()
    ensure("subject" not in flat and "@" not in flat, "payload carries mail-looking content: %s" % flat)


@check("apns-gone-token-drops-the-registration")
def _(c):
    r = c.relay(apns=True)
    r.register(GONE_TOKEN, "acct-1"); r.register("abcdef0123", "acct-1")
    r.signal("history-1", "acct-1")
    h = r.wait_jobs("rejected", 2)
    ensure(h["jobs"]["sent"] == 0, "a refused token counted as sent")
    ensure(r.get("/v1/registrations/acct-1")[1]["count"] == 0, "410 / BadDeviceToken left the registration in place")


@check("bad-input-uses-one-error-vocabulary")
def _(c):
    r = c.relay()
    for label, status, body in (("bad token", 400, {"device_token": "no hex!", "routing_id": "a", "environment": "sandbox"}),
                                ("bad env", 400, {"device_token": "ab", "routing_id": "a", "environment": "moon"})):
        s, j = r.post("/v1/registrations", body)
        ensure(s == status and j["error"]["code"] == "bad_request", "%s: %s %s" % (label, s, j))
    s, _, _, j = r.req("POST", "/v1/registrations", b"{nope")
    ensure(s == 400 and j and j["error"]["code"] == "bad_request", "malformed body: %s %s" % (s, j))
    s, _, _, j = r.req("POST", "/v1/inbound/loopback", {"signal_id": "s", "routing_id": "a"}, {"authorization": "Bearer wrong"})
    ensure(s == 401 and j["error"]["code"] == "unauthorized", "wrong signal token: %s %s" % (s, j))


R = "relay/src/"
MUTANTS = [
    {"id": "M1", "check": "schema-has-no-credential-or-content-column", "file": R + "registry/schema.rs",
     "old": "    registered_at INTEGER NOT NULL,\n    PRIMARY KEY", "new": "    registered_at INTEGER NOT NULL,\n    mailbox_password TEXT,\n    PRIMARY KEY",
     "what": "registration table gains a mailbox_password column"},
    {"id": "M2", "check": "unregister-removes-only-its-own-pair", "file": R + "registry/devices.rs",
     "old": "WHERE device_token = ?1 AND routing_id = ?2\",\n            params![token, routing_id],\n        )?;\n        Ok(n > 0)",
     "new": "WHERE device_token = ?1 AND ?2 = ?2\",\n            params![token, routing_id],\n        )?;\n        Ok(n > 0)",
     "what": "unregister drops the routing-id condition"},
    {"id": "M3", "check": "routing-id-lookup-is-scoped", "file": R + "registry/devices.rs",
     "old": "FROM registration WHERE routing_id = ?1 ORDER BY", "new": "FROM registration WHERE ?1 = ?1 ORDER BY",
     "what": "lookup returns every routing id's devices"},
    {"id": "M4", "check": "queue-depth-counts-queued-jobs-and-dedups-signals", "file": R + "routes/health.rs",
     "old": '"queue_depth": of("queued"),', "new": '"queue_depth": 0,', "what": "health reports queue_depth 0 always"},
    {"id": "M5", "check": "queue-depth-counts-queued-jobs-and-dedups-signals", "file": R + "registry/schema.rs",
     "old": "UNIQUE (signal_id, device_token)", "new": "UNIQUE (job_id)", "what": "the (signal, device) uniqueness is removed"},
    {"id": "M6", "check": "apns-gone-token-drops-the-registration", "file": R + "apns/worker.rs",
     "old": "devices::unregister(&state.store, &job.device_token, &job.routing_id)", "new": "devices::unregister(&state.store, \"\", &job.routing_id)",
     "what": "a 410 no longer removes the registration"},
    {"id": "M7", "check": "apns-payload-is-a-signal-not-mail-content", "file": R + "apns/mod.rs",
     "old": '"mutable-content": 1, "sound": "default" },', "new": '"mutable-content": 1, "sound": "default" },\n        "subject": "Re: invoice",', "what": "the push payload gains a subject"},
    {"id": "M8", "check": "health-is-self-describing", "file": R + "routes/health.rs",
     "old": '"version": env!("CARGO_PKG_VERSION"),', "new": '"version": "0.0.0",', "what": "health reports a hard-coded version"},
    {"id": "M9", "check": "bad-input-uses-one-error-vocabulary", "file": R + "routes/mod.rs",
     "old": "Err(rejection) => Err(ApiError::bad_request(rejection.body_text())),", "new": "Err(rejection) => Err(ApiError::bad_request(rejection.body_text())).map_err(|e: ApiError| ApiError { code: \"invalid\", ..e }),",
     "what": "a malformed body answers with a different error code"},
]

if __name__ == "__main__":
    sys.exit(main("relay-core", CHECKS, MUTANTS))
