#!/usr/bin/env python3
"""phone-shots: a picture of each iPhone screen in its states, light and dark, from the iPhone app on the simulator, one local page. No window.

Run (the lead; the hidden phone rig, test engine; never against a real phone):
  WHOOSH_RIG_HIDDEN=1 sh scripts/iphone-rig.sh "$IPHONE_APP" -- python3 checks/phone-shots.py <outdir>
  python3 checks/phone-shots.py --selftest         (a fake socket and a fake xcrun, no app, no simulator)
Options: --screens a,b   only the states of these screens (default all)    --list    print every state and its steps, no socket, exit 0

It runs INSIDE the rig command: the socket folder is $WHOOSH_SOCKET_DIR (the rig exports it), the device is $RIG_UDID. It opens no window: the pictures are
`xcrun simctl io "$RIG_UDID" screenshot <file>` (a PNG of the device screen, no Simulator.app) and the theme is `xcrun simctl ui "$RIG_UDID" appearance
light|dark` (a simctl call, no window). The socket's own `shot` verb is not used: it is refused on iOS (Sources/App/WindowShot.swift:90-92).

Writes <outdir>/index.html and <outdir>/pics/<screen>-<state>-<theme>.png. One line per picture: PASS / FAIL / NOT JUDGED, then the totals line
'<n> checks, <f> failed, <u> not judged'; exit 1 when f > 0 or u > 0. A picture PASSES when the screen the state names was reached (uistate surface),
the PNG exists and has the size of the geometry file times its density (docs/design/geometry/iphone-inbox.json: 402x874 at 3 = 1206x2622). A state whose
step the app refused is NOT JUDGED naming the step (its picture is still taken); the wrong screen or the wrong size is FAIL. That no row is clipped is read by
a person from index.html: a number cannot say it.

States are driven only through verbs the app serves on the phone (click, fixture) and ids that the phone checks use (checks/settingsphone-check.py:179-263,
checks/threadphone-check.py:115-216, checks/palettephone-check.py, checks/inboxphone-check.py:52). Screens with no door id in those checks (labels and contacts,
the welcome accounts) are not pictured here: not invented.
"""
import html
import json
import os
import struct
import subprocess
import sys
import tempfile
import time
import zlib

sys.dont_write_bytecode = True
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable      # noqa: E402
from lib import phonestart                              # noqa: E402

GEO = os.path.join(ROOT, "docs", "design", "geometry", "iphone-inbox.json")
THEMES = ("light", "dark")
XCRUN_LIMIT_S = 60
SURFACE_WAIT_S = 6.0     # a sheet shows a moment after its door, and `fixture reset` leaves the surface at another word for a moment (phone4)

# (screen, state, caption, steps, surface the state must reach).  A step is ("click", ax) or ("fixture", word, ...).  Every state starts from
# `fixture reset` and the inbox (the app launches on it, PhoneRoutes.swift), then runs its steps.
STATES = [
    ("iphone-inbox", "list", "The inbox list, the fixture mail with account marks", [], "iphone-inbox"),
    ("iphone-mailboxes", "home", "Mailboxes, reached by the inbox Back door", [("click", "iphone-inbox.back")], "iphone-mailboxes"),
    ("iphone-settings", "home", "Settings home", [("click", "iphone-inbox.back"), ("click", "iphone-mailboxes.settings")], "iphone-settings"),
    ("iphone-settings", "sync-ok", "Settings, Sync healthy", [("fixture", "sync", "ok"), ("click", "iphone-inbox.back"), ("click", "iphone-mailboxes.settings"), ("click", "iphone-settings.row.sync")], "iphone-settings"),
    ("iphone-settings", "sync-broken", "Settings, Sync broken, with its reason and Retry", [("fixture", "sync", "broken"), ("click", "iphone-inbox.back"), ("click", "iphone-mailboxes.settings"), ("click", "iphone-settings.row.sync")], "iphone-settings"),
    ("iphone-thread", "open", "A thread opened from the first inbox row", [("click", "iphone-inbox.list.row.0")], "iphone-thread"),
    ("iphone-thread", "more-menu", "The thread's More menu", [("click", "iphone-inbox.list.row.0"), ("click", "iphone-thread.more")], "iphone-thread"),
    ("iphone-composer", "reply", "The reply composer opened from a thread", [("click", "iphone-inbox.list.row.0"), ("click", "iphone-thread.reply")], "iphone-composer"),
    ("iphone-palette", "open", "The command palette sheet", [("click", "iphone-palette.open")], "iphone-palette"),
    ("iphone-search", "open", "The search, opened in place in the inbox", [("click", "iphone-search.open")], "iphone-search"),
]


def geometry():
    with open(GEO, encoding="utf-8") as f:
        g = json.load(f)
    scale = g.get("density", 1)
    return int(round(g["window"]["w"] * scale)), int(round(g["window"]["h"] * scale)), g


def png_size(path):
    with open(path, "rb") as f:
        head = f.read(24)
    if len(head) < 24 or head[:8] != b"\x89PNG\r\n\x1a\n" or head[12:16] != b"IHDR":
        raise ValueError("not a PNG")
    return struct.unpack(">II", head[16:24])


def xcrun(args):
    """(ok, text). Under a time limit; the answer is kept, never thrown away."""
    try:
        r = subprocess.run(["xcrun", "simctl"] + args, capture_output=True, text=True, timeout=XCRUN_LIMIT_S)
    except (OSError, subprocess.TimeoutExpired) as e:
        return False, "xcrun simctl %s: %s" % (" ".join(args[:2]), e)
    return r.returncode == 0, (r.stdout + r.stderr).strip()


def surface_of(ctl):
    return ctl.send("uistate").get("state", {}).get("surface")


def drive(ctl, steps, want_surface):
    """Run one state. Returns (verdict, why). The picture is taken by the caller whatever this says."""
    try:
        ctl.send("fixture", "reset")
        # a test copy starts its first launch on the welcome: leave it through its own controls (journeys/lib/phonestart.py), then wait for the inbox
        end = time.time() + SURFACE_WAIT_S
        got = surface_of(ctl)
        while got != "iphone-inbox" and time.time() < end:
            if got == phonestart.WELCOME:
                phonestart.leave_welcome(ctl)
            time.sleep(0.2)
            got = surface_of(ctl)
        for st in steps:
            if st[0] == "click":
                ctl.send("click", st[1])
            else:
                ctl.send(*st)
        end = time.time() + SURFACE_WAIT_S
        got = surface_of(ctl)
        while got != want_surface and time.time() < end:
            time.sleep(0.2)
            got = surface_of(ctl)
    except Unreadable as e:
        return "NOT JUDGED", "the app refused a step: %s" % e
    if got != want_surface:
        try:
            more = ctl.send("uistate").get("state", {})
            seen = ", ".join("%s=%s" % (k, more.get(k)) for k in ("overlay", "settings_page", "search") if k in more)
        except Unreadable:
            seen = ""
        return "FAIL", "the screen is %r, the state names %r%s" % (got, want_surface, (" (" + seen + ")") if seen else "")
    return "PASS", "reached %s" % want_surface


def take(udid, path, want):
    ok, text = xcrun(["io", udid, "screenshot", path])
    if not ok:
        return "NOT JUDGED", "screenshot failed: %s" % text[:160]
    try:
        size = png_size(path)
    except (OSError, ValueError) as e:
        return "FAIL", "the picture is unreadable: %s" % e
    if size != want:
        return "FAIL", "the picture is %dx%d, the geometry says %dx%d" % (size[0], size[1], want[0], want[1])
    return "PASS", "%dx%d" % size


def page(out, rows, g):
    tiles = []
    for r in rows:
        img = '<img src="pics/%s" alt="%s" width="201">' % (html.escape(r["file"]), html.escape(r["caption"])) if r["pic"] else "<p>no picture</p>"
        tiles.append('<figure><div class="v %s">%s</div>%s<figcaption><b>%s / %s / %s</b><br>%s<br>%s</figcaption></figure>' % (
            r["verdict"].replace(" ", "-"), html.escape(r["verdict"]), img, html.escape(r["screen"]), html.escape(r["state"]), r["theme"],
            html.escape(r["caption"]), html.escape(r["why"])))
    body = "\n".join(tiles)
    with open(os.path.join(out, "index.html"), "w", encoding="utf-8") as f:
        f.write('<!doctype html><meta charset="utf-8"><title>iPhone shots</title><style>body{font:13px system-ui;margin:16px}figure{display:inline-block;'
                'vertical-align:top;margin:8px;width:215px}.v{font-weight:600}.PASS{color:#0a7d2c}.FAIL{color:#c00}.NOT-JUDGED{color:#a60}</style>'
                '<h1>iPhone shots, %dx%d points at %sx (read each picture for clipped text)</h1>\n%s\n' % (g["window"]["w"], g["window"]["h"], g.get("density"), body))


def run(out, screens=None, deadline_s=900.0):
    udid = os.environ.get("RIG_UDID", "")
    wp, hp, g = geometry()
    states = [s for s in STATES if not screens or s[0] in screens]
    os.makedirs(os.path.join(out, "pics"), exist_ok=True)
    rows = []
    ctl = Ctl()
    try:
        ctl.connect()
    except CouldNotDrive as e:
        print("NOT JUDGED all: %s" % e)
        print("0 checks, 0 failed, 1 not judged")
        return 1
    if not udid:
        print("NOT JUDGED all: RIG_UDID is not set (run this inside scripts/iphone-rig.sh)")
        print("0 checks, 0 failed, 1 not judged")
        return 1
    t0 = time.time()
    try:
        for theme in THEMES:
            ok, text = xcrun(["ui", udid, "appearance", theme])
            if not ok:
                print("NOT JUDGED theme %s: appearance not set: %s" % (theme, text[:160]))
            for screen, state, caption, steps, want in states:
                row = {"screen": screen, "state": state, "theme": theme, "caption": caption, "pic": False, "file": "%s-%s-%s.png" % (screen, state, theme)}
                if time.time() - t0 > deadline_s:
                    row["verdict"], row["why"] = "NOT JUDGED", "the deadline of %ds passed before this state" % deadline_s
                else:
                    v, why = drive(ctl, steps, want)
                    path = os.path.join(out, "pics", row["file"])
                    v2, why2 = take(udid, path, (wp, hp))
                    row["pic"] = os.path.exists(path)
                    row["verdict"] = "FAIL" if "FAIL" in (v, v2) else ("NOT JUDGED" if "NOT JUDGED" in (v, v2) else "PASS")
                    row["why"] = "; ".join(x for x in (why, why2) if x)
                    if not ok:
                        row["verdict"] = "NOT JUDGED" if row["verdict"] == "PASS" else row["verdict"]
                        row["why"] += "; the theme could not be set"
                rows.append(row)
                print("%s %s/%s/%s %s" % (row["verdict"], screen, state, theme, row["why"]))
    finally:
        xcrun(["ui", udid, "appearance", "light"])
        ctl.close()
    page(out, rows, g)
    n = len(rows); f = sum(1 for r in rows if r["verdict"] == "FAIL"); u = sum(1 for r in rows if r["verdict"] == "NOT JUDGED")
    print("%d checks, %d failed, %d not judged" % (n, f, u))
    return 0 if f == 0 and u == 0 and n > 0 else 1


# ---------------------------------------------------------------------------------------------- the selftest: a fake socket and a fake xcrun
def _png(w, h):
    raw = b"".join(b"\x00" + b"\xff" * w for _ in range(h))
    def chunk(t, d): return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 0, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 1)) + chunk(b"IEND", b"")


def selftest():
    import socket
    import threading
    base = tempfile.mkdtemp(prefix="pshot.", dir="/tmp")           # short: a unix socket path is limited to 104 bytes
    wp, hp, _ = geometry()
    bindir = os.path.join(base, "bin"); os.makedirs(bindir)
    png = os.path.join(base, "good.png"); open(png, "wb").write(_png(wp, hp))
    bad = os.path.join(base, "bad.png"); open(bad, "wb").write(_png(wp // 2, hp // 2))
    log = os.path.join(base, "xcrun.log")
    with open(os.path.join(bindir, "xcrun"), "w") as f:
        f.write('#!/bin/sh\necho "xcrun $*" >> "%s"\nif [ "$3" = screenshot ] || [ "$4" = screenshot ]; then\n  src="%s"; [ -n "${FAKE_BAD_PNG:-}" ] && src="%s"\n'
                '  cp "$src" "$5"\nfi\n' % (log, png, bad))
    os.chmod(os.path.join(bindir, "xcrun"), 0o755)
    state = {"surface": "iphone-inbox", "refuse": None, "clicks": [], "reads": 0, "start": "iphone-welcome-accounts", "stuck": None}
    sock_path = os.path.join(base, "whoosh.sock")
    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM); srv.bind(sock_path); srv.listen(4)
    NAV = {"iphone-inbox.back": "iphone-mailboxes", "iphone-mailboxes.settings": "iphone-settings", "iphone-inbox.list.row.0": "iphone-thread",
           "iphone-thread.reply": "iphone-composer", "iphone-palette.open": "iphone-palette", "iphone-search.open": "iphone-search"}

    def serve():
        while True:
            try:
                c, _ = srv.accept()
            except OSError:
                return
            buf = b""
            while True:
                try:
                    d = c.recv(65536)
                except OSError:
                    break
                if not d:
                    break
                buf += d
                while b"\n" in buf:
                    line, buf = buf.split(b"\n", 1)
                    q = json.loads(line.decode()); v, a = q["verb"], q.get("args", [])
                    if v == "fixture" and a[:1] == ["reset"]:
                        state["surface"] = "mac-main"; state["reads"] = 0; r = {"ok": True}
                    elif v == "type":
                        r = {"ok": True}
                    elif v == "click":
                        if a[0] == state["refuse"]:
                            r = {"ok": False, "error": "no such control %s" % a[0]}
                        else:
                            state["clicks"].append(a[0])
                            if a[0] == "iphone-welcome.not-now": state["start"] = state["surface"] = "iphone-inbox"
                            elif a[0] != state["stuck"] and state["surface"] != "iphone-welcome-accounts": state["surface"] = NAV.get(a[0], state["surface"])
                            r = {"ok": True}
                    elif v == "fixture":
                        r = {"ok": True}
                    elif v == "uistate":
                        state["reads"] += 1
                        if state["surface"] == "mac-main" and state["reads"] > 2: state["surface"] = state["start"]
                        r = {"ok": True, "state": {"surface": state["surface"]}}
                    else:
                        r = {"ok": False, "error": "unknown verb"}
                    c.sendall((json.dumps(r) + "\n").encode())
            c.close()
    threading.Thread(target=serve, daemon=True).start()

    def once(env_extra=None, refuse=None, screens=None, udid="AAAA", stuck=None):
        out = os.path.join(base, "out-%d" % len(os.listdir(base)))
        state["refuse"] = refuse; state["stuck"] = stuck; state["clicks"] = []
        open(log, "w").close()
        env = dict(os.environ, PATH=bindir + os.pathsep + os.environ["PATH"], WHOOSH_SOCKET_DIR=base, RIG_UDID=udid)
        env.update(env_extra or {})
        saved = dict(os.environ); os.environ.clear(); os.environ.update(env)
        try:
            import io, contextlib
            buf = io.StringIO()
            with contextlib.redirect_stdout(buf):
                rc = run(out, screens)
            return rc, buf.getvalue(), out
        finally:
            os.environ.clear(); os.environ.update(saved)

    n = bad_n = 0
    def ok(name, cond):
        nonlocal n, bad_n
        n += 1
        print(("PASS " if cond else "FAIL ") + name)
        if not cond: bad_n += 1

    total = len(STATES) * 2
    rc, text, out = once()
    ok("clean-run-every-state-in-both-themes-passes (%d pictures)" % total, rc == 0 and ("%d checks, 0 failed, 0 not judged" % total) in text)
    ok("pictures-and-index-written", len(os.listdir(os.path.join(out, "pics"))) == total and "iphone-settings / sync-broken / dark" in open(os.path.join(out, "index.html")).read())
    calls = open(log).read().splitlines()
    ok("theme-set-light-then-dark-then-light-back-and-device-named", [c for c in calls if " appearance " in c] == ["xcrun simctl ui AAAA appearance light", "xcrun simctl ui AAAA appearance dark", "xcrun simctl ui AAAA appearance light"])
    ok("no-call-of-open-or-Simulator", not any(c.split()[1:2] not in (["simctl"],) for c in calls) and all(" io AAAA screenshot " in c or " ui AAAA appearance " in c for c in calls))
    rc, text, out = once(refuse="iphone-settings.row.sync")
    ok("a-refused-step-is-NOT-JUDGED-naming-the-step-and-the-picture-is-still-taken", rc == 1 and "NOT JUDGED iphone-settings/sync-ok/light the app refused a step" in text and "iphone-settings.row.sync" in text and os.path.exists(os.path.join(out, "pics", "iphone-settings-sync-ok-light.png")))
    rc, text, out = once(env_extra={"FAKE_BAD_PNG": "1"})
    ok("a-picture-of-the-wrong-size-is-FAIL", rc == 1 and "FAIL iphone-inbox/list/light reached iphone-inbox; the picture is %dx%d" % (wp // 2, hp // 2) in text and ("%d checks, %d failed" % (total, total)) in text)
    rc, text, out = once(udid="")
    ok("no-RIG_UDID-is-NOT-JUDGED-and-exit-1", rc == 1 and "RIG_UDID is not set" in text)
    rc, text, out = once(screens=["iphone-thread"])
    ok("--screens-picks-only-those-screens", rc == 0 and "4 checks, 0 failed, 0 not judged" in text)
    global SURFACE_WAIT_S
    SURFACE_WAIT_S = 1.0
    rc, text, out = once(stuck="iphone-search.open", screens=["iphone-search"])
    ok("a-door-that-does-nothing-is-FAIL-naming-both-screens", rc == 1 and "FAIL iphone-search/open/light the screen is 'iphone-inbox', the state names 'iphone-search'" in text)
    state["surface"] = "x"
    srv.close()
    print("selftest: %d cases, %d failed" % (n, bad_n))
    import shutil
    shutil.rmtree(base, ignore_errors=True)
    return 0 if bad_n == 0 else 1


def main(argv):
    if "--selftest" in argv:
        return selftest()
    if "--list" in argv:
        for screen, state, caption, steps, want in STATES:
            print("%s/%s: %s -> %s  [%s]" % (screen, state, steps, want, caption))
        return 0
    args = [a for a in argv if not a.startswith("--")]
    screens = None
    if "--screens" in argv:
        i = argv.index("--screens"); screens = argv[i + 1].split(","); args = [a for a in args if a != argv[i + 1]]
    if not args:
        print("usage: phone-shots.py <outdir> [--screens a,b] | --selftest | --list"); return 2
    return run(args[0], screens)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
