#!/usr/bin/env python3
"""par-relay-tracking: open and click tracking on SENT mail, driven against the running relay.

PRD "Sent-mail tracking": "On, opt-in per message". Off by default; a switch in the composer; the relay's
pixel and redirect routes; the endpoint stores an opaque message token and the event time only, never the
recipient's address; the reader shows "opened · clicked". One driven check per feature below.

    python3 checks/parity/par-relay-tracking.py             run every check once
    python3 checks/parity/par-relay-tracking.py --selftest  clean run, then one rebuilt relay per MUTANT (each must go red)

Prints `N checks, F failed, U not judged` and a MUTANTS table. Exit 1 when F > 0 or U > 0.
HOSTING is gap G17: the routes exist and are tested here on 127.0.0.1; the public host is a later step and is
printed as PENDING, not counted as a check.
"""
import importlib.util, json, os, sys, urllib.parse

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("relay_core_lib", os.path.join(HERE, "..", "relay-core-check.py"))
lib = importlib.util.module_from_spec(spec)
spec.loader.exec_module(lib)
ensure, Fail = lib.ensure, lib.Fail

CHECKS = []


def check(name):
    def deco(fn):
        CHECKS.append((name, fn))
        return fn
    return deco


DEST = "https://example.com/offer?ref=abc"


def new_message(r):
    s, b = r.post("/v1/track/messages", {})
    ensure(s == 201 and len(b.get("token", "")) > 0, "creating a tracked message answered %s %s" % (s, b))
    return b["token"], b["pixel_url"]


def path_of(url):
    p = urllib.parse.urlsplit(url)
    return p.path + ("?" + p.query if p.query else "")


def summary(r, token):
    return r.get("/v1/track/messages/" + token)


@check("open-tracking-pixel-counts-opens")
def _(c):
    r = c.relay()
    token, pixel = new_message(r)
    ensure(pixel.startswith(r.proc.url) and pixel.endswith("/t/o/%s.gif" % token), "pixel_url is %s" % pixel)
    for _ in range(2):
        s, h, raw, _j = r.req("GET", path_of(pixel))
        ensure(s == 200 and h.get("content-type") == "image/gif" and raw.startswith(b"GIF89a"), "pixel answered %s %s" % (s, h.get("content-type")))
    s, sm = summary(r, token)
    ensure(s == 200 and sm["opens"] == 2 and sm["opened"] is True and sm["clicked"] is False, "summary after 2 opens: %s" % sm)
    return "2 opens counted"


@check("pixel-is-never-cached")
def _(c):
    r = c.relay()
    token, pixel = new_message(r)
    h = r.req("GET", path_of(pixel))[1]
    ensure("no-store" in h.get("cache-control", ""), "cache-control is %r: a cached pixel hides the second open" % h.get("cache-control"))


@check("tracking-is-opt-in-per-message")
def _(c):
    r = c.relay()
    a, pixel_a = new_message(r)
    b, _pb = new_message(r)
    stranger = "abcdef0123456789abcdef0123456789"
    s, _, raw, _j = r.req("GET", "/t/o/%s.gif" % stranger)
    ensure(s == 200 and raw.startswith(b"GIF89a"), "an unknown token must still get the pixel, got %s" % s)
    ensure(summary(r, stranger)[0] == 404, "a message nobody opted in has a summary")
    r.req("GET", path_of(pixel_a))
    ensure(summary(r, a)[1]["opens"] == 1 and summary(r, b)[1]["opens"] == 0, "one message's open leaked into another")
    ensure(summary(r, b)[1]["opened"] is False, "a fresh message reads as opened")


@check("click-redirect-counts-the-click")
def _(c):
    r = c.relay()
    token, _p = new_message(r)
    s, j = r.post("/v1/track/links", {"token": token, "url": DEST})
    ensure(s == 200 and j["link_url"].startswith(r.proc.url), "sign link answered %s %s" % (s, j))
    s, h, _raw, _j = r.req("GET", path_of(j["link_url"]))
    ensure(s == 302 and h.get("location") == DEST, "tracked link answered %s -> %s" % (s, h.get("location")))
    sm = summary(r, token)[1]
    ensure(sm["clicks"] == 1 and sm["clicked"] is True and sm["opened"] is False, "summary after one click: %s" % sm)
    ensure(r.post("/v1/track/links", {"token": token, "url": "javascript:alert(1)"})[0] == 400, "a non-http destination was signed")
    ensure(r.post("/v1/track/links", {"token": "abcdef", "url": DEST})[0] == 404, "a link was signed for an unregistered message")


@check("only-relay-signed-links-redirect")
def _(c):
    r = c.relay()
    token, _p = new_message(r)
    link = r.post("/v1/track/links", {"token": token, "url": DEST})[1]["link_url"]
    q = urllib.parse.parse_qs(urllib.parse.urlsplit(link).query)
    evil = urllib.parse.quote(__import__("base64").urlsafe_b64encode(b"https://evil.example/phish").decode().rstrip("="))
    forged = "/t/c/%s?u=%s&s=%s" % (token, evil, q["s"][0])
    s, h, _raw, _j = r.req("GET", forged)
    ensure(s == 400 and "location" not in h, "a link with a swapped destination answered %s %s" % (s, h.get("location")))
    s, h, _raw, _j = r.req("GET", "/t/c/%s?u=%s&s=00" % (token, q["u"][0]))
    ensure(s == 400 and "location" not in h, "a link with a bad signature answered %s" % s)
    ensure(summary(r, token)[1]["clicks"] == 0, "a refused link was still counted as a click")


@check("tracking-stores-token-and-time-only")
def _(c):
    r = c.relay()
    token, pixel = new_message(r)
    link = r.post("/v1/track/links", {"token": token, "url": DEST})[1]["link_url"]
    r.req("GET", path_of(pixel), headers={"x-forwarded-for": "203.0.113.9", "user-agent": "Mail/1.0 bob@example.com"})
    r.req("GET", path_of(link), headers={"x-forwarded-for": "203.0.113.9", "user-agent": "Mail/1.0 bob@example.com"})
    cols = {t: lib.schema_of(r)[t] for t in ("tracked_message", "tracking_event")}
    ensure(cols == {t: lib.ALLOWED[t] for t in cols}, "tracking tables hold %s" % {t: sorted(v) for t, v in cols.items()})
    dump = json.dumps([r.sql("SELECT * FROM tracked_message"), r.sql("SELECT * FROM tracking_event")])
    for needle in ("@", "203.0.113.9", "example.com", "http", "Mail/1.0"):
        ensure(needle not in dump, "tracking rows contain %r: %s" % (needle, dump))
    return "token + time + kind only"


@check("reader-summary-says-opened-and-clicked")
def _(c):
    r = c.relay()
    token, pixel = new_message(r)
    link = r.post("/v1/track/links", {"token": token, "url": DEST})[1]["link_url"]
    r.req("GET", path_of(pixel)); r.req("GET", path_of(link))
    s, sm = summary(r, token)
    ensure(s == 200 and sm["opened"] is True and sm["clicked"] is True, "summary: %s" % sm)
    ensure(all(isinstance(sm[k], int) for k in ("first_open_at", "last_open_at", "last_click_at")), "event times missing: %s" % sm)


T = "relay/src/"
MUTANTS = [
    {"id": "P1", "check": "open-tracking-pixel-counts-opens", "file": T + "routes/tracking_routes.rs",
     "old": 'tracking::record(&st.store, token, "open", now_unix())', "new": "Ok::<bool, crate::registry::StoreError>(false)", "what": "the pixel route stops recording opens"},
    {"id": "P2", "check": "pixel-is-never-cached", "file": T + "routes/tracking_routes.rs",
     "old": '(header::CACHE_CONTROL, "no-store")', "new": '(header::CACHE_CONTROL, "public")', "what": "the pixel is served cacheable"},
    {"id": "P3", "check": "tracking-is-opt-in-per-message", "file": T + "tracking/mod.rs",
     "old": ".optional()?.is_none() {", "new": ".optional()?.is_none() && false {", "what": "an unregistered token gets a summary"},
    {"id": "P4", "check": "click-redirect-counts-the-click", "file": T + "routes/tracking_routes.rs",
     "old": 'tracking::record(&st.store, &token, "click", now_unix())', "new": "Ok::<bool, crate::registry::StoreError>(false)", "what": "the redirect stops recording clicks"},
    {"id": "P5", "check": "only-relay-signed-links-redirect", "file": T + "routes/tracking_routes.rs",
     "old": "|| !st.signer.verify(&token, &url, &q.s)", "new": "|| false", "what": "the redirect skips signature verification"},
    {"id": "P6", "check": "tracking-stores-token-and-time-only", "file": T + "registry/schema.rs",
     "old": "    occurred_at INTEGER NOT NULL\n);", "new": "    occurred_at INTEGER NOT NULL,\n    recipient_address TEXT\n);", "what": "tracking_event gains a recipient_address column"},
    {"id": "P7", "check": "reader-summary-says-opened-and-clicked", "file": T + "routes/tracking_routes.rs",
     "old": '"clicked": s.clicks > 0,', "new": '"clicked": false,', "what": "the summary never reports clicked"},
]
PENDING = ["hosting the pixel and redirect on a public host is gap G17; the routes are exercised on 127.0.0.1 only"]

if __name__ == "__main__":
    sys.exit(lib.main("par-relay-tracking", CHECKS, MUTANTS, PENDING))
