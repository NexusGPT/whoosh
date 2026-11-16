#!/usr/bin/env python3
"""The hidden rig's proof (owner 2026-11-08 16:30: "on all tests, EVER, i want you to stop the preview opening EVER").

    python3 checks/rig-hidden-check.py <bundle> [-- <cmd> [args] [';;' <cmd> ...]]
    python3 checks/rig-hidden-check.py --selftest

Launches <bundle> through scripts/launch-one.sh with WHOOSH_RIG_HIDDEN=1 (in the environment and in LAUNCH_ENV) and
--rig-hidden. From OUTSIDE the app it watches the process table every 0.1 s; from the first sighting of the rig copy's
pid until it ends, scripts/rig-windows.swift reads the window server every 0.1 s (CGWindowListCopyWindowInfo,
on-screen windows only). Asserted for the WHOLE run:
  - every window of the pid on the window server has kCGWindowAlpha exactly 0;
  - NSWorkspace.frontmostApplication is never the pid; the pid never holds a regular activation policy (Dock, ⌘-Tab).
Inside the rig, before the first keystroke, the drive waits until the watch has sampled the pid and found it hidden;
then it drives one key (J moves the focus), one click (mac-main.accounts.all answers), takes one shot and asserts it is
not blank. Commands after `--` run inside the same rig, under the same watch, after that drive.

Prints 'N checks, F failed, U not judged'; exit 1 when F > 0, U > 0 or N = 0. Evidence: _captures/appshell/r3/.
--selftest feeds the judge recorded window-server readings (no launch, no activation, nothing on screen): the clean
reading is green, and a window at alpha 1/255, the pid frontmost, the pid with a Dock tile are each red.
"""
import hashlib
import json
import os
import socket
import subprocess
import sys
import tempfile
import threading
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
import importlib.util as _ilu
_sw = _ilu.spec_from_file_location("shotwait", os.path.join(ROOT, "journeys", "lib", "shotwait.py")); shotwait = _ilu.module_from_spec(_sw); _sw.loader.exec_module(shotwait)   # how long a `shot` reply may take (one place)
OUT = os.path.join(ROOT, "_captures", "appshell", "r3")
PS_EVERY_S = 0.1          # process table read, from the launch on
SAMPLE_MS = 100           # window server read once the pid is seen (the brief: at most 0.5 s)

# MUTANTS: per check, the mutation of the app that must turn it red.
# watch-read-the-pid | --selftest: a run with no reading of the pid
# every-window-alpha-0 | WHOOSH_RIG_HIDDEN_MUTANT=alpha-1-255 (the main window at alpha 1/255: nothing drawn, listed 0.0039)
# never-frontmost | --selftest: a recorded reading with the rig pid frontmost (no live activation, ever)
# no-dock-tile | --selftest: a recorded reading with the rig pid holding a regular activation policy
# hidden-before-first-key | the same alpha mutant: the drive refuses to press a key on a visible copy
# key-moves-focus | the J binding removed from BaseFeature
# click-answers | the mac-main.accounts.all target removed from BaseFeature
# shot-not-blank | WHOOSH_SHOT_RENDER path returning an empty bitmap
# system-ui-<kind>-refused | WHOOSH_RIG_HIDDEN_MUTANT=no-system-ui (no hook installed: the check then refuses to trigger the real call)
# drag-moves-the-label | WHOOSH_DRAG_NOSEND=1 and WHOOSH_DROP_OFF=1 (no event, no screen drop)
# interaction-gap-served | the gap answered as 0 (or above the frame): InteractionProbe.answer returning gapMs = 0
# first-frame-served | AppModel.firstFrameMs never set (RootView's main-queue turn removed): measure answers not judged
# shot-frames-match-pixels | WHOOSH_SHOT_MAPPING_MUTANT=title-band (the reply's mapping claims a 28 pt title band)
# hidden-web-text | WHOOSH_SHOT_WEB_MUTANT=off (the hidden shot draws no web view's own picture: the mail text is in the shot right after a load and gone 6 s later); --selftest: a model that blanks the second shot
WEB_WAIT_S = 6.0          # hidden-web-text: the wait between the shot right after the load and the second shot, no key between
WEB_INK_FLOOR_PCT = 0.5   # hidden-web-text: the least share of the band's pixels that differ from its background, in both shots
WEB_INK_KEPT = 0.5        # hidden-web-text: the second shot keeps at least this share of the first shot's ink (native text in the band cannot hide a loss)


def helper():
    src = os.path.join(ROOT, "scripts", "rig-windows.swift")
    tag = hashlib.sha256(open(src, "rb").read()).hexdigest()[:12]
    exe = os.path.join(tempfile.gettempdir(), "whoosh-rig-windows-" + tag)
    if not os.path.exists(exe):
        r = subprocess.run(["swiftc", "-O", src, "-o", exe], capture_output=True, text=True)
        if r.returncode != 0:
            raise RuntimeError("the window reader did not compile: " + r.stderr[-400:])
    return exe


def judge(samples, pid):
    """(name, ok, detail) for the three whole-run assertions over window-server readings of `pid`."""
    reads = [s for s in samples if "windows" in s]
    lit = [(s.get("n"), w) for s in reads for w in s["windows"] if w.get("alpha") != 0]
    front = [s.get("n") for s in reads if s.get("front_pid") == pid]
    dock = [s.get("n") for s in reads if s.get("dock_tile")]
    taken = max((s.get("n", 0) for s in samples), default=0)
    seen = max((len(s["windows"]) for s in reads), default=0)
    return [
        # The pid was read the whole time. An alpha-0 SwiftUI window is not in the on-screen list at all (as3g), so a
        # run that opens no plain window reads 0 windows: that is hidden, not blind (the mutant at 1/255 lists them).
        ("watch-read-the-pid", len(reads) > 0 and taken >= 10, "%d samples taken, %d distinct readings, up to %d windows of pid %s on screen" % (taken, len(reads), seen, pid)),
        ("every-window-alpha-0", not lit, "windows not at alpha 0: %s" % (lit[:3] or "none")),
        ("never-frontmost", not front, "samples with the pid frontmost: %s" % (front[:5] or "none")),
        ("no-dock-tile", not dock, "samples with a Dock tile: %s" % (dock[:5] or "none")),
    ]


def selftest():
    pid = 4242
    win = lambda a: {"id": 1, "alpha": a, "x": 0, "y": 0, "w": 900, "h": 600, "shown": a > 0}
    clean = [{"n": 12, "windows": [win(0)], "front_pid": 77, "dock_tile": False}]
    cases = [
        ("clean reading is green", clean, set()),
        ("alpha 1/255 is red", [{"n": 12, "windows": [win(1 / 255)], "front_pid": 77, "dock_tile": False}], {"every-window-alpha-0"}),
        ("the pid frontmost is red", clean + [{"n": 13, "windows": [win(0)], "front_pid": pid, "dock_tile": False}], {"never-frontmost"}),
        ("a Dock tile is red", clean + [{"n": 13, "windows": [win(0)], "front_pid": 77, "dock_tile": True}], {"no-dock-tile"}),
        ("no reading at all is red", [{"n": 0, "ended": True}], {"watch-read-the-pid"}),
    ]
    n = f = 0
    for title, samples, want_red in cases:
        red = {name for name, ok, _ in judge(samples, pid) if not ok}
        n += 1
        ok = red == want_red
        f += 0 if ok else 1
        print("  %s  %s (red: %s)" % ("PASS" if ok else "FAIL", title, sorted(red) or "none"))
    # the chip's frame missing from the shot: the failure line carries the frames the app served and the one the case needed
    n += 1
    line = frame_missing_line({"mac-main.attachments.0": None, "mac-main.reader": {"x": 0, "y": 0, "w": 1, "h": 1}})
    need = ("needed frames ['mac-main.attachments.0']", "'mac-main.attachments.0': None", "'mac-main.reader': 'frame'", "7 shots", "reader scroll {'offset': 94", "shell {'surface': 'mac-main'")
    miss = [w for w in need if w not in line]
    ok = line.startswith("shot-frames-match-pixels|FAIL|no frame or mapping") and not miss
    f += 0 if ok else 1
    print("  %s  a shot without the chip's frame is red and says what was served%s" % ("PASS" if ok else "FAIL", "" if ok else " (line lacks %s)" % miss))
    print("    FAIL line: " + line)
    n += 1
    older = frame_missing_line("not-a-dict")                        # an older build: no `frames` at all; nothing raises
    ok = older.startswith("shot-frames-match-pixels|FAIL|") and "absent" in older
    f += 0 if ok else 1
    print("  %s  a shot reply with no frames at all reads as absent and is red" % ("PASS" if ok else "FAIL"))
    # the mail text in the hidden shot: green when both shots carry it, red when the second (6 s later) or the first has lost it
    for title, ink, want in (("the mail text in both hidden shots is green", (True, True), "PASS"),
                             ("the mail text gone from the shot 6 s after the load (the old path) is red", (True, False), "FAIL"),
                             ("the mail text missing from the first shot is red", (False, True), "FAIL"),
                             ("no frame for the reader's mail is red, and says so", None, "FAIL")):
        n += 1
        line = web_text_line(ink)
        ok = line.startswith("hidden-web-text|%s|" % want) and (ink is not None or "no frame" in line)
        f += 0 if ok else 1
        print("  %s  %s" % ("PASS" if ok else "FAIL", title))
        print("    line: " + line)
    for title, line, want in (("a shot saying `web: not ready` is NOT JUDGED", web_ready_line("hidden-web-text", "render (hidden rig; settled 300 ms, web: not ready after 3000 ms)"), "NOT JUDGED"),
                              ("a shot with a ready page is judged", web_ready_line("hidden-web-text", "render (hidden rig; settled 300 ms, web ready after 120 ms; 1 web view drawn)"), None)):
        n += 1
        ok = (line is None) if want is None else (line is not None and "|%s|" % want in line)
        f += 0 if ok else 1
        print("  %s  %s" % ("PASS" if ok else "FAIL", title))
    for title, ok in (("the dark page's light background is red", not theme_follows("dark", 240)), ("the light page's dark background is red", not theme_follows("light", 20)),
                      ("a background that follows the theme is green", theme_follows("dark", 20) and theme_follows("light", 240))):
        n += 1
        f += 0 if ok else 1
        print("  %s  %s" % ("PASS" if ok else "FAIL", title))
    print("%d checks, %d failed, 0 not judged" % (n, f))
    return 0 if f == 0 else 1


def write_png(path, w, h, ink_rows):
    """A white w x h RGB PNG with black pixels on the rows in `ink_rows` (every 3rd pixel): the model's shot."""
    import struct
    import zlib
    raw = b"".join(b"\x00" + b"".join((b"\x00\x00\x00" if (y in ink_rows and x % 3 == 0) else b"\xff\xff\xff") for x in range(w)) for y in range(h))
    chunk = lambda t, d: struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    open(path, "wb").write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))


class FakeWebCtl:
    """The socket of the stand-in for the mail-text case: a 400 x 200 pt window at scale 1, the reader's mail block at y 100..200 whose last
    84 pt are the page, and shots that carry the page's text rows or not (`ink` per shot)."""
    def __init__(self, dirpath, ink, frame=True):
        self.dir, self.ink, self.frame, self.shots = dirpath, list(ink), frame, 0

    def send(self, verb, *args):
        if verb == "key":
            return {"ok": True}
        if verb == "rows" and args[0] == "mac-main.reader.web":
            return {"rows": [{"text_length": 140, "height": 84}]}
        if verb == "measure":
            if args[0] == "mac-main.reader.message.0" and self.frame:
                return {"frame": {"x": 0, "y": 100, "w": 400, "h": 100}}
            return {"ok": False, "error": "no measurement at %s" % args[0]}
        if verb == "shot":
            path = os.path.join(self.dir, "%s.png" % args[0])
            write_png(path, 400, 200, set(range(130, 190, 6)) if self.ink[self.shots] else set())
            self.shots += 1
            return {"ok": True, "path": path, "source": "render (hidden rig; 1 web view drawn)", "mapping": {"x": 0, "y": 0, "scale": 1}}
        return {"ok": True}


def web_text_line(ink):
    """Run `hidden_web_text` on the stand-in (no wait) and return the line it printed; `ink` is (first shot has text, second has text), or None for no frame."""
    import contextlib
    import io
    buf = io.StringIO()
    with tempfile.TemporaryDirectory() as d, contextlib.redirect_stdout(buf):
        hidden_web_text(FakeWebCtl(d, ink or (True, True), frame=ink is not None), sleep=lambda s: None)
    return buf.getvalue().strip()


class FakeShotCtl:
    """The socket of the stand-in for the shot case: a list with one attachment row, a thread that scrolls, a shot that serves `frames`."""
    def __init__(self, frames):
        self.frames = frames

    def send(self, verb, *args):
        if verb == "rows" and args[0] == "mac-main.list":
            return {"rows": [{"id": "msg-01", "attachments": 1, "thread_count": 3}]}
        if verb == "uistate":
            return {"state": {"focus_message": "msg-01", "focus_index": 0, "surface": "mac-main", "first_responder": "reader"}}
        if verb == "rows" and args[0] == "mac-main.reader.scroll":
            return {"rows": [{"offset": 94, "max_offset": 400}]}
        if verb == "rows":
            return {"ok": False, "error": "no rows at %s" % args[0]}
        if verb == "shot":
            r = {"ok": True, "path": "/nonexistent.png", "mapping": None}
            if self.frames != "not-a-dict":
                r["frames"] = self.frames
            return r
        return {"ok": True}


def frame_missing_line(frames):
    """Run `frame_vs_pixels` on the stand-in and return the line it printed."""
    import contextlib
    import io
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        frame_vs_pixels(FakeShotCtl(frames))
    return buf.getvalue().strip()


class Ctl:
    def __init__(self, path):
        self.s = socket.socket(socket.AF_UNIX)
        self.s.settimeout(30)
        self.s.connect(path)
        self.f = self.s.makefile("rwb")

    def send(self, verb, *args):
        self.s.settimeout(shotwait.timeout_for(verb, args, 30))
        self.f.write((json.dumps({"verb": verb, "args": [str(a) for a in args]}) + "\n").encode())
        self.f.flush()
        return json.loads(self.f.readline().decode() or "{}")


def colours(path):
    small = path + ".small.bmp"
    if subprocess.run(["sips", "-Z", "64", "-s", "format", "bmp", path, "--out", small], capture_output=True).returncode != 0:
        return 0
    data = open(small, "rb").read()
    off = int.from_bytes(data[10:14], "little")
    return len({data[i:i + 3] for i in range(off, len(data) - 3, 4)})


def inner(watch):
    """Inside launch-one: hidden first, then one key, one click, one shot. Lines 'name|PASS|detail' for the outer."""
    pid = int(os.environ["RIG_PID"])
    deadline = time.time() + 20
    last = None
    while time.time() < deadline:   # the watch must have read this pid, and found it hidden, before any key
        lines = [json.loads(x) for x in open(watch) if x.strip()] if os.path.exists(watch) else []
        reads = [s for s in lines if s.get("pid") == pid and "windows" in s]
        if reads and (reads[-1]["windows"] or time.time() > deadline - 10):   # a window, or 10 s of none on screen
            last = reads[-1]
            break
        time.sleep(0.1)
    hidden = last is not None and all(w["alpha"] == 0 for w in last["windows"]) and last.get("front_pid") != pid
    print("hidden-before-first-key|%s|%s" % ("PASS" if hidden else "FAIL", last and [w["alpha"] for w in last["windows"]]))
    if not hidden:
        return
    c = Ctl(os.path.join(os.environ["D"], "whoosh.sock"))
    before = c.send("uistate").get("state", {}).get("focus_index")
    c.send("key", "J")
    after = c.send("uistate").get("state", {}).get("focus_index")
    print("key-moves-focus|%s|focus %s -> %s" % ("PASS" if after != before else "FAIL", before, after))
    # Perf gates' measures (docs/PERF-FOOTPRINT.md Round 2): the gap of the J just pressed, and the first frame.
    gap, frame = c.send("measure", "mac-main.interaction.gap"), c.send("measure", "mac-main.interaction.frame")
    g, fr = gap.get("ms"), frame.get("ms")
    ok = isinstance(g, (int, float)) and isinstance(fr, (int, float)) and 0 < g <= fr + 0.5
    print("interaction-gap-served|%s|gap %s ms, frame %s ms (a gap lies inside arrival to frame) %s" % ("PASS" if ok else "FAIL", g, fr, gap.get("error") or ""))
    ff, launch = c.send("measure", "mac-main.first-frame"), c.send("measure", "mac-main.launch")
    a, b = ff.get("ms"), launch.get("ms")
    ok = isinstance(a, (int, float)) and isinstance(b, (int, float)) and 0 < b <= a
    print("first-frame-served|%s|first frame %s ms, launch %s ms (the first frame follows the first appearance) %s" % ("PASS" if ok else "FAIL", a, b, ff.get("error") or ""))
    r = c.send("click", "mac-main.accounts.all")
    print("click-answers|%s|%s" % ("PASS" if r.get("ok") else "FAIL", r))
    s = c.send("shot", "rig-hidden-check")
    n = colours(s.get("path", "")) if s.get("ok") else 0
    print("shot-not-blank|%s|%s %s, %d colours" % ("PASS" if n > 16 else "FAIL", s.get("source"), s.get("path"), n))
    # From here every step runs whatever the step before it did (main43: a timed-out save-panel request left four steps
    # "not reached"): a step that throws prints its own FAIL, and the next one gets a fresh connection, because the late
    # reply of a timed-out request would otherwise answer the next request on the old one.
    path = os.path.join(os.environ["D"], "whoosh.sock")
    box = {"c": c}

    def guarded(name, fn):
        try:
            fn(box["c"])
        except Exception as e:   # noqa: BLE001 (a socket timeout, a reply of the wrong shape: the step is red, the run goes on)
            print("%s|FAIL|%s: %s" % (name, type(e).__name__, e))
            try:
                box["c"].s.close()
            except OSError:
                pass
            box["c"] = Ctl(path)

    guarded("hidden-web-text", hidden_web_text)   # before the thread is opened and scrolled below: the reader pane as J left it
    guarded("hidden-web-theme", hidden_web_theme)
    guarded("shot-frames-match-pixels", frame_vs_pixels)
    # System UI (lead 18:17): every kind is refused on a hidden rig; the whole-run watch must still read no window.
    hooks = []

    def read_hooks(cc):
        hooks.extend(cc.send("fixture", "system-ui", "hooks").get("hooks") or [])
    guarded("system-ui-hooks", read_hooks)
    need = {"quick-look": "QLPreviewPanel", "menu": "NSMenu", "save-panel": "NSSavePanel", "alert": "NSAlert", "share": "NSSharingServicePicker"}
    for kind, cls in need.items():
        if not any(h.startswith(cls + " ") for h in hooks):   # never make the real call while its hook is missing
            print("system-ui-%s-refused|FAIL|no hook on %s; not triggered" % (kind, cls))
            continue

        def refuse(cc, kind=kind):
            before = cc.send("core", "state", "system-ui/" + kind).get("value", 0)
            r = cc.send("fixture", "system-ui", kind)
            after = cc.send("core", "state", "system-ui/" + kind).get("value", 0)
            print("system-ui-%s-refused|%s|asks %s -> %s, %s" % (kind, "PASS" if r.get("ok") and after > before else "FAIL", before, after, r.get("error") or ""))
        guarded("system-ui-%s-refused" % kind, refuse)
    guarded("drag-moves-the-label", drag_label)


def pixel_reader(path):
    """(width, height, pixel(x, y) -> (r, g, b)) of a PNG, read through sips as `colours` and `column_runs` do."""
    small = path + ".ink.bmp"
    if subprocess.run(["sips", "-s", "format", "bmp", path, "--out", small], capture_output=True).returncode != 0:
        raise OSError("sips could not read %s" % path)
    data = open(small, "rb").read()
    off, bpp = int.from_bytes(data[10:14], "little"), int.from_bytes(data[28:30], "little") // 8
    w, h = int.from_bytes(data[18:22], "little"), int.from_bytes(data[22:26], "little", signed=True)
    top_down, h = h < 0, abs(h)
    stride = (w * bpp + 3) // 4 * 4

    def pixel(x, y):
        i = off + (y if top_down else h - 1 - y) * stride + x * bpp
        return (data[i + 2], data[i + 1], data[i])
    return w, h, pixel


def band_ink(path, box, scale):
    """The share (percent) of the pixels in `box` = (x, y, w, h) shot points that differ from the band's commonest colour by more than 24 on a channel."""
    w, h, pixel = pixel_reader(path)
    x0, x1 = max(0, int(box[0] * scale)), min(w, int((box[0] + box[2]) * scale))
    y0, y1 = max(0, int(box[1] * scale)), min(h, int((box[1] + box[3]) * scale))
    if x1 <= x0 or y1 <= y0:
        raise ValueError("the band %s lies outside the %dx%d picture" % (box, w, h))
    px = [pixel(x, y) for y in range(y0, y1) for x in range(x0, x1)]
    base = max(set(px), key=px.count)
    return 100.0 * sum(1 for p in px if max(abs(p[k] - base[k]) for k in range(3)) > 24) / len(px)


def web_ready_line(name, source):
    """A shot whose `source` says `web: not ready` (the verb waited its bound and the page was not loaded and painted) is NOT JUDGED, never a pass or a fail of the page."""
    if "web: not ready" in str(source):
        return "%s|NOT JUDGED|the shot says the page was not ready: %s" % (name, source)
    return None


def band_luminance(path, box, scale):
    """The commonest colour's luminance (0 to 255) in `box` = (x, y, w, h) shot points: the page's own background."""
    w, h, pixel = pixel_reader(path)
    x0, x1 = max(0, int(box[0] * scale)), min(w, int((box[0] + box[2]) * scale))
    y0, y1 = max(0, int(box[1] * scale)), min(h, int((box[1] + box[3]) * scale))
    px = [pixel(x, y) for y in range(y0, y1) for x in range(x0, x1)]
    base = max(set(px), key=px.count)
    return 0.299 * base[0] + 0.587 * base[1] + 0.114 * base[2]


def theme_follows(theme, luminance):
    """The pasted page's background follows the theme just set: light above 128, dark below."""
    return luminance > 128 if theme == "light" else luminance < 128


def hidden_web_theme(c):
    """After `theme dark` the pasted body's background is dark, after `theme light` light (main108: the dark 1280 body came out empty, the light one ghost text)."""
    name = "hidden-web-theme"
    c.send("key", "J")
    block = None
    for k in range(12):
        f = (c.send("measure", "mac-main.reader.message.%d" % k) or {}).get("frame")
        if f:
            block = f
    if not block:
        print("%s|FAIL|no frame for the reader's mail" % name)
        return
    try:
        for theme in ("dark", "light"):
            c.send("theme", theme)
            s = c.send("shot", "%s-%s" % (name, theme))
            nr = web_ready_line(name, s.get("source"))
            if nr:
                print(nr)
                return
            mp = s.get("mapping") or {}
            box = (block["x"] + mp["x"], block["y"] + mp["y"], block["w"], block["h"])
            lum = band_luminance(s["path"], box, float(mp["scale"]))
            if not theme_follows(theme, lum):
                print("%s|FAIL|after `theme %s` the pasted page's background has luminance %.0f; source %r" % (name, theme, lum, s.get("source")))
                return
        print("%s|PASS|the page background followed the theme (dark then light)" % name)
    finally:
        c.send("theme", "light")


def hidden_web_text(c, sleep=time.sleep, wait_s=WEB_WAIT_S):
    """The mail's text in a hidden shot whatever the time since the load: J loads a mail; one shot as soon as the page holds text, one
    after `wait_s` with no key between; the band of the page (the last `height` points of the reader's last message block, from `rows
    mac-main.reader.web` and `measure mac-main.reader.message.N`) must carry ink above the floor in both, and the second must keep the first's."""
    name = "hidden-web-text"
    c.send("key", "J")
    web, t0 = {}, time.time()
    while time.time() - t0 < 5:
        rows = c.send("rows", "mac-main.reader.web").get("rows") or []
        web = rows[0] if rows else {}
        # the view's height is read once it is the height the model reserved: read while it still moved, the band was the last 12 pt of a block and held
        # no text (main143, run r015: 0.00 % in both shots with the text drawn above the band)
        if (web.get("text_length") or 0) > 0 and abs((web.get("height") or 0) - (web.get("height_reserved") or web.get("height") or 0)) < 1:
            break
        sleep(0.1)
    block = None
    for k in range(12):   # the last message block of the thread that answers a frame
        f = (c.send("measure", "mac-main.reader.message.%d" % k) or {}).get("frame")
        if f:
            block = f
    if not block or not web.get("height") or not (web.get("text_length") or 0) > 0:
        print("%s|FAIL|no frame or no page text to judge: block frame %s, web row %s" % (name, block, {k: web.get(k) for k in ("text_length", "height", "painted")}))
        return
    band = (block["x"], block["y"] + block["h"] - min(web["height"], block["h"]), block["w"], min(web["height"], block["h"]))
    reads = []
    for i, label in enumerate(("right after the load", "%g s later, no key" % wait_s)):
        if i:
            sleep(wait_s)
        s = c.send("shot", "%s-%d" % (name, i + 1))
        mp = s.get("mapping") or {}
        if not (s.get("ok") and mp):
            print("%s|FAIL|no shot or mapping %s: %s" % (name, label, {k: s.get(k) for k in ("ok", "error", "mapping")}))
            return
        nr = web_ready_line(name, s.get("source"))
        if nr:
            print(nr)
            return
        reads.append((label, band_ink(s["path"], (band[0] + mp["x"], band[1] + mp["y"], band[2], band[3]), float(mp["scale"])), s.get("source")))
    (l1, a, src1), (l2, b, src2) = reads
    ok = a > WEB_INK_FLOOR_PCT and b > WEB_INK_FLOOR_PCT and b >= WEB_INK_KEPT * a
    print("%s|%s|mail text ink in the page band %s: %.2f %% %s, %.2f %% %s (floor %g %%, the second keeps at least %g of the first); source %r then %r; page text_length %s" % (
        name, "PASS" if ok else "FAIL", tuple(round(v, 1) for v in band), a, l1, b, l2, WEB_INK_FLOOR_PCT, WEB_INK_KEPT, src1, src2, web.get("text_length")))


def drag_label(c):
    """One drag of the labels screen's gesture (tap + DragGesture), so the whole-run watch covers SwiftUI's own
    activation path."""
    lab = "mac-labels-contacts"
    c.send("shot", lab)
    c.send("click", lab + ".show.labels")
    rows = c.send("rows", lab + ".labels").get("rows", [])
    # user labels only: the core's built-in categories (primary, needs-reply, ...) are top-level rows that never move
    user = [r for r in rows if r["id"].startswith("lbl-")]
    parents = {r.get("parent") for r in rows}
    leaves = [r for r in user if r.get("parent") is not None and r["id"] not in parents]
    tops = [r for r in user if r.get("parent") is None]
    pair = next(((x, t) for x in leaves for t in tops if t["id"] != x.get("parent")), None)
    if pair is None:
        print("drag-moves-the-label|FAIL|no user leaf and top-level label pair in %s" % [r["id"] for r in rows])
        return
    a, b = pair
    r = c.send("drag", a["ax"], b["ax"])
    moved = {x["id"]: x.get("parent") for x in c.send("rows", lab + ".labels").get("rows", [])}.get(a["id"]) == b["id"]
    # moved with drop None = the gesture committed (as3i with WHOOSH_DROP_OFF=1); drop "screen" = the screen's drop
    # function took the end (it also answers for a label the gesture already moved, so it cannot tell the two apart).
    print("drag-moves-the-label|%s|moved %s, drop %s by %s, received %s of %s" % (
        "PASS" if moved else "FAIL", moved, r.get("drop"), r.get("drop_by"), r.get("received"), r.get("delivered")))


def column_runs(path, x):
    """Runs of one colour (within 3 per channel) down pixel column x of a PNG: [(top_px, bottom_px, (r, g, b))]."""
    small = path + ".col.bmp"
    w = int(subprocess.run(["sips", "-g", "pixelWidth", path], capture_output=True, text=True).stdout.split()[-1])
    h = int(subprocess.run(["sips", "-g", "pixelHeight", path], capture_output=True, text=True).stdout.split()[-1])
    if subprocess.run(["sips", "-s", "format", "bmp", path, "--out", small], capture_output=True).returncode != 0:
        return []
    data = open(small, "rb").read()
    off, bpp = int.from_bytes(data[10:14], "little"), int.from_bytes(data[28:30], "little") // 8
    top_down = int.from_bytes(data[22:26], "little", signed=True) < 0   # a negative height: rows stored top-down
    stride = (w * bpp + 3) // 4 * 4
    col = []
    for y in range(h):
        i = off + (y if top_down else h - 1 - y) * stride + x * bpp
        col.append((data[i + 2], data[i + 1], data[i]))
    runs, start = [], 0
    for y in range(1, h + 1):
        if y == h or max(abs(col[y][k] - col[start][k]) for k in range(3)) > 3:
            runs.append((start, y - 1, col[start]))
            start = y
    return runs


def served_frames(s):
    """What a shot reply served: {frame name: "frame" or None} (a name served with no frame is None); "absent" when the reply has no `frames`."""
    fr = s.get("frames") if isinstance(s, dict) else None
    return {str(k): (None if v is None else "frame") for k, v in fr.items()} if isinstance(fr, dict) else "absent"


def missing_frame_evidence(c, chip, s, shots, scroll):
    """The values that decide a missing chip frame: the frame the case needed against the frames the app served on each of the shots it took (the
    first, then one per page-up), the mapping, the reader's scroll row and the shell's focus. Never raises; a read the build refuses says so."""
    def read(fn):
        try:
            return fn()
        except (OSError, ValueError, KeyError, IndexError, TypeError, AttributeError) as e:
            return "unreadable (%s)" % e
    st = read(lambda: c.send("uistate").get("state", {}))
    shell = {f: st.get(f, "absent") for f in ("surface", "focus", "first_responder", "focus_message", "overlay")} if isinstance(st, dict) else st
    atts = read(lambda: [r.get("id", "absent") for r in (c.send("rows", "mac-main.attachments").get("rows") or [])])
    return ("; needed frames [%r]; served on the %d shots taken (the first, then one after each page-up): %s; mapping %r; reader scroll %s; shell %s; "
            "attachment rows %s" % (chip, len(shots), shots, s.get("mapping", "absent") if isinstance(s, dict) else "absent", scroll, shell, atts))


def frame_vs_pixels(c):
    """On a scrolled thread: the attachment chip's frame from the shot reply, mapped through its `mapping`, against
    where the chip's own fill is found in the PNG (a column of pixels, searched without the frame's y)."""
    name = "shot-frames-match-pixels"
    rows = c.send("rows", "mac-main.list").get("rows") or []
    with_att = [r for r in rows if r.get("attachments", 0) > 0]
    if not with_att:
        print("%s|FAIL|no row with an attachment in the fixture list" % name)
        return
    target = max(with_att, key=lambda r: r.get("thread_count", 0))
    for _ in range(len(rows) + 2):
        st = c.send("uistate").get("state", {})
        if st.get("focus_message") == target["id"]:
            break
        c.send("key", "J" if st.get("focus_index", 0) < rows.index(target) else "K")
    c.send("resize", "960", "600")                         # small enough that the opened thread scrolls
    c.send("key", "Enter")
    c.send("click", "mac-main.reader.earlier")
    c.send("key", "R")
    c.send("key", "Escape")
    for _ in range(6):
        c.send("key", "Space")
    scroll = (c.send("rows", "mac-main.reader.scroll").get("rows") or [{}])[0]
    chip = "mac-main.attachments.0"
    s = c.send("shot", "mac-main", chip)
    shots = [served_frames(s)]       # what each shot of the chip answered, for the line that says the frame is missing
    for _ in range(6):   # six pages can carry the chip past the top: page back up until it is drawn again (main43 local run)
        if (s.get("frames") or {}).get(chip):
            break
        c.send("key", "⇧Space")
        s = c.send("shot", "mac-main", chip)
        shots.append(served_frames(s))
    scroll = (c.send("rows", "mac-main.reader.scroll").get("rows") or [{}])[0]
    f, mp = (s.get("frames") or {}).get(chip), s.get("mapping")
    if not (s.get("ok") and f and mp):
        print("%s|FAIL|no frame or mapping in the shot reply: %s%s" % (name, {k: s.get(k) for k in ("ok", "error", "frames", "mapping")},
                                                                      missing_frame_evidence(c, chip, s, shots, scroll)))
        return
    sc = float(mp["scale"])
    x = int((f["x"] + mp["x"] + f["w"] - 12) * sc)                 # inside the chip's right end, clear of its text
    runs = column_runs(s["path"], x)
    base = max(runs, key=lambda r: r[1] - r[0])[2] if runs else None   # the column's commonest colour: the pane
    # every colour that also fills a stretch at least twice the chip's height is a pane colour, never the chip's fill
    # (main43: a stretch of the reader's own background, (30, 30, 32), had the chip's height and counted as a second fill)
    panes = [r[2] for r in runs if (r[1] - r[0] + 1) / sc >= 2 * f["h"]] + ([base] if base else [])
    near = lambda a, b: max(abs(a[k] - b[k]) for k in range(3)) <= 8
    fits = [r for r in runs if abs((r[1] - r[0] + 1) / sc - f["h"]) <= 3 and not any(near(r[2], p) for p in panes)]
    if not fits:
        print("%s|FAIL|no run of the chip's height in column %d (scroll %s): %s" % (name, x, scroll, runs[:8]))
        return
    # several stretches of the chip's height can share a column (main43: a 52 pt stretch of the reader's background 250 pt
    # above the chip). The nearest to the frame's centre is judged; the title-band mutant moves that centre by 28 pt, so
    # it still fails the 1 pt delta. Every candidate is printed.
    claimed_px = (f["y"] + mp["y"] + f["h"] / 2) * sc
    top, bottom, colour = min(fits, key=lambda r: abs((r[0] + r[1] + 1) / 2 - claimed_px))
    drawn = (top + bottom + 1) / 2 / sc                               # the fill's centre, in shot points
    claimed = f["y"] + mp["y"] + f["h"] / 2
    delta = drawn - claimed
    print("%s|%s|delta %.2f pt: drawn centre %.2f, frame centre %.2f (frame y %s h %s, mapping y %s, scale %s), scroll offset %s of %s, fill %s, candidates %d" % (
        name, "PASS" if abs(delta) < 1 and scroll.get("offset", 0) > 0 else "FAIL", delta, drawn, claimed, f["y"], f["h"], mp["y"], sc,
        scroll.get("offset"), scroll.get("max_offset"), colour, len(fits)))


def main():
    if "--selftest" in sys.argv:
        return selftest()
    if "--inner" in sys.argv:
        inner(sys.argv[sys.argv.index("--inner") + 1])
        for cmd in extras(sys.argv):
            r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
            print("---- %s: exit %d\n%s" % (" ".join(cmd), r.returncode, (r.stdout + r.stderr).rstrip()[-6000:]))
        return 0
    if len(sys.argv) < 2 or not os.path.isdir(sys.argv[1]):
        print("usage: rig-hidden-check.py <bundle> [-- cmd ...] | --selftest")
        return 2
    bundle = os.path.abspath(sys.argv[1])
    os.makedirs(OUT, exist_ok=True)
    exe = helper()
    watch = os.path.join(OUT, "rig-hidden-watch.jsonl")
    open(watch, "w").close()
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1",
               LAUNCH_ENV=" ".join(["WHOOSH_RIG_HIDDEN=1"] + os.environ.get("LAUNCH_ENV", "").split()))
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--fixture-core", "--rig-hidden",
           "python3", os.path.abspath(__file__), "--inner", watch] + (["--"] + sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else [])
    launcher = subprocess.Popen(cmd, cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    marker = "run-%d/rig/" % launcher.pid
    readers = []

    def watch_ps():   # from the first sighting of the pid: a reader per rig pid, appending to one file
        seen = set()
        while launcher.poll() is None:
            ps = subprocess.run(["ps", "-axo", "pid=,command="], capture_output=True, text=True).stdout
            for line in ps.splitlines():
                p, _, c = line.strip().partition(" ")
                if marker in c and "/Contents/MacOS/" in c and int(p) not in seen:
                    seen.add(int(p))
                    readers.append(subprocess.Popen([sys.executable, "-c", TAG, exe, p, watch, str(SAMPLE_MS)]))
            time.sleep(PS_EVERY_S)

    t = threading.Thread(target=watch_ps, daemon=True)
    t.start()
    out = launcher.communicate()[0]
    t.join(2)
    for r in readers:
        r.wait(5)
    print(out.rstrip())
    samples = [json.loads(x) for x in open(watch) if x.strip()]
    pids = sorted({s["pid"] for s in samples if "pid" in s})
    results = [(n, ok, d) for pid in pids for n, ok, d in judge([s for s in samples if s.get("pid") == pid], pid)] or \
              [("watch-read-the-pid", False, "no rig pid was ever seen")]
    for line in out.splitlines():
        if line.count("|") >= 2 and line.split("|")[1] in ("PASS", "FAIL"):
            name, verdict, detail = line.split("|", 2)
            results.append((name, verdict == "PASS", detail))
    names = {n for n, _, _ in results}
    for want in ("hidden-before-first-key", "key-moves-focus", "interaction-gap-served", "first-frame-served", "click-answers", "shot-not-blank", "hidden-web-text", "hidden-web-theme", "shot-frames-match-pixels", "drag-moves-the-label") + tuple(
            "system-ui-%s-refused" % k for k in ("quick-look", "menu", "save-panel", "alert", "share")):
        if want not in names:
            results.append((want, None, "not reached"))
    lines = ["%-10s %s  %s" % ("PASS" if ok else ("NOT JUDGED" if ok is None else "FAIL"), n, d) for n, ok, d in results]
    failed = sum(1 for _, ok, _ in results if ok is False)
    unjudged = sum(1 for _, ok, _ in results if ok is None)
    lines.append("%d checks, %d failed, %d not judged" % (len(results), failed, unjudged))
    open(os.path.join(OUT, "rig-hidden-check.txt"), "w").write("launcher exit %s\n" % launcher.returncode + "\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0 if failed == 0 and unjudged == 0 and results else 1


def extras(argv):
    words, cmds, cmd = (argv[argv.index("--") + 1:] if "--" in argv else []), [], []
    for x in words + [";;"]:
        if x == ";;":
            if cmd:
                cmds.append(cmd)
            cmd = []
        else:
            cmd.append(x)
    return cmds


# Runs the window reader on one pid and tags each line with the pid, appending to the shared watch file.
TAG = r"""
import json, subprocess, sys
exe, pid, watch, every = sys.argv[1:5]
p = subprocess.Popen([exe, pid, "--watch", "-", "--every", every], stdout=subprocess.PIPE, text=True)
with open(watch, "a") as f:
    for line in iter(p.stdout.readline, ""):
        d = json.loads(line); d["pid"] = int(pid); f.write(json.dumps(d) + "\n"); f.flush()
"""

if __name__ == "__main__":
    sys.exit(main())
