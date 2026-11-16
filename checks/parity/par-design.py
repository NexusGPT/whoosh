#!/usr/bin/env python3
"""Parity check: par-design (the running Mac main window against the design lock, docs/DESIGN.md + docs/CONSTANTS.md).

Drives the RUNNING rig copy over the control socket. Lock numbers come from docs/CONSTANTS.md (const) and the
v2 board's contrast table (docs/design/boards/directions/native-glass/v2); measured numbers come from `measure`
and from the pixels of the real window (screencapture -l, 2x). A number the app does not expose is NOT JUDGED,
never passed. Never adjust a lock here to make a check pass.
Self-test (no app): python3 checks/parity/par-design.py --selftest
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/par-design.py'
Last line: "<n> checks, <f> failed, <u> not judged". Exit 0 only when f == 0 and u == 0.
"""
import colorsys, json, os, statistics, subprocess, sys, time
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, Unreadable          # noqa: E402
from lib.fixture import const, ACCOUNT, MSG   # noqa: E402
from PIL import Image                         # noqa: E402

OUT = os.path.join(ROOT, "_captures", "design-build", "par")
TOL = const("DESIGN_TOLERANCE_PX")
TOL = 2.0  # the lead's brief (2026-11-08) sets 2 pt; the ledger row says 1 (reported in docs/lanes/design-build.md)
LOCK = {  # every number from docs/CONSTANTS.md
    "sidebar": {"1280": const("SIDEBAR_W_PX"), "960": const("SIDEBAR_W_COMPACT_PX")},
    "list": {"1280": const("LIST_W_PX"), "960": const("LIST_W_COMPACT_PX")},
    "foot": const("SIDEBAR_FOOT_PX"), "row": const("LIST_ROW_PX"), "strip": const("FOOT_STRIP_PX"),
    "sat": const("EMBLEM_LIST_SATURATION"),
}
CONTRAST_MIN = 4.5       # v2 board: "every meta line passes 4.5:1"; body and read-subject rows measured 5.1 to 16.8
SAT_TOL = 0.10           # my allowance: the 16 pt mark is a lit gradient, so its median saturation is not exactly the cut
results = []             # (name, status, detail)


def chk(name, ok, detail):
    results.append((name, "PASS" if ok else "FAIL", detail))


def nj(name, why):
    results.append((name, "NOTJUDGED", why))


SELFTEST = "--selftest" in sys.argv
if not SELFTEST:
    c = Ctl(os.environ.get("WHOOSH_SOCKET_DIR") or os.environ["D"]).connect()
    D = c.dir


MAP = {"x": 0, "y": 0, "scale": 2}   # the last shot's mapping (CONTROL-SOCKET delta 45): pixel = (point + mapping) * scale
FRAMES = {}                          # the last shot's frames (delta 46), read in the same turn as the pixels
REPLY = {}                           # what the last shot's reply carried: its keys and the ids it asked for (a missing frame says what it did carry)


def shot(name, *ids):
    """The app's own shot file: `capture shot <name>` writes $WHOOSH_SOCKET_DIR/shots/<name>.png (hidden rig: the
    window's view tree, title band included). No screencapture, no window id: a hidden window has no pixels on screen."""
    r = c.send("capture", "shot", name, *ids)
    MAP.update(r.get("mapping") or {})
    FRAMES.clear(); FRAMES.update(r.get("frames") or {})
    REPLY.clear(); REPLY.update({"keys": sorted(r), "asked": list(ids), "frame_ids": sorted(FRAMES)[:12], "mapping": r.get("mapping")})
    path = r.get("path") or os.path.join(D, "shots", name + ".png")
    im = Image.open(path).convert("RGB")
    if len(set(im.resize((16, 16)).getdata())) < 3:
        raise Unreadable("shot %s is blank (%s)" % (name, path))
    return im


def lum(rgb):
    def f(v):
        v /= 255
        return v / 12.92 if v <= .03928 else ((v + .055) / 1.055) ** 2.4
    r, g, b = rgb
    return .2126 * f(r) + .7152 * f(g) + .0722 * f(b)


def ratio(a, b):
    la, lb = sorted((lum(a), lum(b)), reverse=True)
    return (la + .05) / (lb + .05)


def mode(px):
    return max(set(px), key=px.count)


def ink_clusters(im, x0, x1, y0, y1):
    """Vertical clusters of rows (2x pixels) holding ink in the x range: [(start, end, bg, inkpx)]."""
    out, cur, gap = [], None, 0
    for y in range(y0, y1):
        row = [im.getpixel((x, y)) for x in range(x0, x1)]
        bg = mode(row)
        lb = lum(bg)
        far = max(row, key=lambda p: abs(lum(p) - lb))
        has = abs(lum(far) - lb) > .25 and ratio(far, bg) > 2
        if has:
            if cur is None:
                cur = [y, y, bg, far, 0]
            cur[1] = y
            if ratio(far, bg) > ratio(cur[3], bg):
                cur[3] = far
            gap = 0
        elif cur is not None:
            gap += 1
            if gap > 3:
                out.append(tuple(cur[:4])); cur = None
    if cur:
        out.append(tuple(cur[:4]))
    return out


def row_pitch(cl, S):
    """Median pitch in points between consecutive rows' first-line clusters, or None when fewer than 8 clusters."""
    if len(cl) < 8:
        return None, 0
    starts = [cl[i][0] for i in range(0, len(cl) - 2, 2)]
    pitches = [(starts[i + 1] - starts[i]) / S for i in range(len(starts) - 1)]
    return statistics.median(pitches), len(pitches)


def sat_of(hexs):
    h = hexs.lstrip("#"); r, g, b = (int(h[i:i + 2], 16) / 255 for i in (0, 2, 4))
    return colorsys.rgb_to_hsv(r, g, b)[1]


def frame(i):
    try:
        return c.send("measure", i)["frame"]
    except Unreadable:
        return None


def foot_steps(col, S, limit=6):
    """The first `limit` changes of the column's mean brightness above 2 levels, from the window foot upwards: [(points from the foot, value above, value below)]."""
    out = []
    for y in range(len(col) - 2, len(col) // 2, -1):
        if abs(col[y] - col[y + 1]) > 2:
            out.append((round((len(col) - y) / S, 1), round(col[y], 1), round(col[y + 1], 1)))
            if len(out) == limit:
                break
    return out


def collapse_verdict(n0, n1, dt, load, load_max, lock):
    """'pass' | 'fail' | 'not judged': a row that left the model in the same frame within the lock; a slow reply is not judged while the machine load is over PERF_LOAD_MAX."""
    if n1 != n0 - 1:
        return "fail"
    if dt < lock:
        return "pass"
    return "not judged" if load > load_max else "fail"


def _planted(pitch, S=2):
    """A synthetic list shot: 16 rows of dark ink on white, `pitch` points apart, two ink lines per row (sender, subject)."""
    im = Image.new("RGB", (800, 2000), (255, 255, 255))
    for r in range(16):
        y = int(r * pitch * S) + 200
        for dy in (0, 14 * S // 2 * 2):
            for yy in range(y + dy, y + dy + 8):
                for x in range(546, 700):
                    im.putpixel((x, yy), (20, 20, 20))
    return im


if SELFTEST:
    S = 2
    ok_med, _ = row_pitch(ink_clusters(_planted(LOCK["row"]), 546, 800, 100, 1900), S)
    bad_med, _ = row_pitch(ink_clusters(_planted(LOCK["row"] + 10), 546, 800, 100, 1900), S)
    good = ok_med is not None and abs(ok_med - LOCK["row"]) <= TOL
    bad = bad_med is not None and abs(bad_med - LOCK["row"]) > TOL
    print("%s planted shot at the lock pitch %s: measured %s" % ("PASS" if good else "FAIL", LOCK["row"], ok_med))
    print("%s planted shot 10 pt off: measured %s, judged red" % ("PASS" if bad else "FAIL", bad_med))
    col = [240.0] * 700 + [200.0] * 3 + [240.0] * 3 + [250.0] * 20           # a strip of 20 px (10 pt) at the foot, then a dark hairline of 3 px
    steps = foot_steps(col, 2)
    steps_ok = bool(steps) and steps[0][0] == 10.5 and steps[1][0] == 12.0 and len(steps) == 3
    print("%s planted column: the steps from the foot are listed (%s)" % ("PASS" if steps_ok else "FAIL", steps))
    lock = float(const("INTERACTION_COMMIT_MS")); lmax = float(const("PERF_LOAD_MAX"))
    cv = (collapse_verdict(17, 16, 84.9, lmax + 6, lmax, lock), collapse_verdict(17, 16, 84.9, lmax - 6, lmax, lock),
          collapse_verdict(17, 16, 5.0, lmax + 6, lmax, lock), collapse_verdict(17, 17, 5.0, lmax - 6, lmax, lock))
    cv_ok = cv == ("not judged", "fail", "pass", "fail")
    print("%s collapse verdict: slow under load not judged, slow on a quiet machine red, fast green, row kept red (%s)" % ("PASS" if cv_ok else "FAIL", cv))
    allok = good and bad and steps_ok and cv_ok
    print("4 checks, %d failed, 0 not judged" % (4 - good - bad - steps_ok - cv_ok))
    sys.exit(0 if allok else 1)

for (W, H) in ((1280, 820), (960, 632)):
    c.send("resize", str(W), str(H))
    for theme in ("light", "dark"):
        c.send("theme", theme); time.sleep(1.2)
        k = "%dx%d-%s" % (W, H, theme)
        sb, ls, ft = frame("mac-main.sidebar"), frame("mac-main.list.view"), frame("mac-main.sidebar.accounts-menu")
        sh = c.send("rows", "mac-main.shell")["rows"][0]
        wf = c.send("winframes")["frames"][0]
        sw, lw = LOCK["sidebar"][str(W)], LOCK["list"][str(W)]
        chk("%s/sidebar-width" % k, sb and abs(sb["w"] - sw) <= TOL, "lock %s measured %s (mac-main.sidebar)" % (sw, sb and round(sb["w"], 1)))
        chk("%s/list-width" % k, ls and abs(ls["w"] - lw) <= TOL, "lock %s measured %s (mac-main.list.view)" % (lw, ls and round(ls["w"], 1)))
        chk("%s/sidebar-foot-height" % k, ft and abs(ft["h"] - LOCK["foot"]) <= TOL, "lock %s measured %s (mac-main.sidebar.accounts-menu)" % (LOCK["foot"], ft and round(ft["h"], 1)))
        rw = W - sw - lw
        chk("%s/reader-width" % k, abs(sh["reader_w"] - rw) <= TOL, "lock %s (window - sidebar - list) measured %s (rows mac-main.shell)" % (rw, sh["reader_w"]))
        chk("%s/window-size" % k, abs(wf["w"] - W) <= TOL and abs(wf["h"] - H) <= TOL, "asked %sx%s window is %sx%s" % (W, H, wf["w"], wf["h"]))
        try:
            im = shot("list-" + k, "mac-main.list.row.1")
        except Unreadable as e:
            nj("%s/row-height" % k, str(e)); nj("%s/emblem-saturation" % k, str(e))
            continue
        S = MAP.get("scale") or im.width / wf["w"]   # pixels per point, from the shot's own mapping
        pt = lambda v, a="x": int(round((v + MAP.get(a, 0)) * S))   # content-view point -> shot pixel
        # row height: pitch of first-line clusters in the sender column
        cl = ink_clusters(im, pt(273), pt(400), pt(100, "y"), pt(wf["h"] - 40, "y"))
        med, npitch = row_pitch(cl, S)
        if W == 1280 and med is not None:
            chk("%s/row-height" % k, abs(med - LOCK["row"]) <= TOL, "lock %s measured pitch %.1f (median of %d, pixels)" % (LOCK["row"], med, npitch))
        elif W == 1280:
            nj("%s/row-height" % k, "ink clusters found: %d" % len(cl))
        # contrast on 1280 only (the same rows at 960)
        if W == 1280 and len(cl) >= 8:
            sender = cl[4]   # Ton Verheyden: unread, label colour on the row background
            sub = cl[3]      # Marie Lefevre subject (read): secondary label
            sel = cl[0]      # Linea Supply: white on the selection fill
            for nm, cc in (("body-sender", sender), ("secondary-read-subject", sub), ("selected-row-text", sel)):
                r = ratio(cc[3], cc[2])
                chk("%s/contrast-%s" % (k, nm), r >= CONTRAST_MIN, "%.1f:1 (lock >= %.1f)" % (r, CONTRAST_MIN))
        # emblem quietness: row 1 (Marie, account of msg-02)
        if W == 1280:
            raw = sat_of(ACCOUNT[MSG["msg-02"]["account"]]["colour"])
            fr = FRAMES.get("mac-main.list.row.1")   # row 1 is Marie's: centre the 28 pt box on its frame, not on a fixed y
            if not fr:
                nj("%s/emblem-saturation" % k, "the shot reply carried no frame for mac-main.list.row.1 (shot reply keys %s, ids asked %s, frames it carried %s, mapping %s)" % (
                    REPLY.get("keys"), REPLY.get("asked"), REPLY.get("frame_ids"), REPLY.get("mapping"))); raw = None
            cy = (fr["y"] + fr["h"] / 2) if fr else 151
            box = [im.getpixel((x, y)) for x in range(pt(246), pt(274)) for y in range(pt(cy - 14, "y"), pt(cy + 14, "y"))] if fr else []
            sats = [colorsys.rgb_to_hsv(*(v / 255 for v in p))[1] for p in box]
            disc = [s for s in sats if s > .2]
            if raw is None:
                pass
            elif len(disc) > 20:
                share = statistics.median(disc) / raw
                chk("%s/emblem-saturation" % k, share <= LOCK["sat"] + SAT_TOL, "median disc saturation / account colour saturation = %.2f (lock %.2f, allowance %.2f)" % (share, LOCK["sat"], SAT_TOL))
            else:
                nj("%s/emblem-saturation" % k, "no disc pixels found in the box")
        # thread open: foot strip height from the hairline
        c.send("click", "mac-main.pane.reader"); c.send("key", "Enter"); time.sleep(1.5)
        try:
            im2 = shot("thread-" + k)
        except Unreadable as e:
            nj("%s/foot-strip-height" % k, str(e)); c.send("key", "Escape"); continue
        x0, x1 = pt(sw + lw + 30), pt(sw + lw + 90)   # x has mapping.x = 0
        col = [statistics.mean(sum(im2.getpixel((x, y))) / 3 for x in range(x0, x1)) for y in range(im2.height)]
        base = col[-3]
        hy = None
        for y in range(im2.height - 6, im2.height // 2, -1):
            if abs(col[y] - base) > 2:
                hy = y; break
        if hy is not None:
            h = (im2.height - hy) / S
            chk("%s/foot-strip-height" % k, abs(h - LOCK["strip"]) <= TOL, "lock %s measured %.1f (hairline to window foot, pixels); shot %dx%d px at scale %s, column x %d..%d, "
                "foot value %.1f, first steps from the foot (pt, value above, value below) %s" % (LOCK["strip"], h, im2.width, im2.height, S, x0, x1, base, foot_steps(col, S)))
        else:
            nj("%s/foot-strip-height" % k, "hairline not found")
        chk("%s/reader-opens" % k, c.send("uistate")["state"]["surface"] == "mac-reader", "surface after Enter on the reader pane")
        c.send("key", "Escape"); c.send("click", "mac-main.pane.list"); time.sleep(.3)

# type sizes: not exposed
nj("type-sizes", "no verb exposes a font size; the pixel cap heights would measure the rasteriser, not the token")

# motion (1280x820 light)
c.send("resize", "1280", "820"); c.send("theme", "light"); time.sleep(1)
st = lambda: c.send("rows", "mac-main.list.state")["rows"][0]
fi = lambda: c.send("uistate")["state"]["focus_index"]
f0 = fi()
t0 = time.time(); c.send("key", "J"); dt = (time.time() - t0) * 1000
f1 = fi()
chk("motion/J-state-on-input", f1 == f0 + 1 and dt < const("INTERACTION_COMMIT_MS"), "focus %s -> %s, key round trip %.1f ms (lock < %s)" % (f0, f1, dt, const("INTERACTION_COMMIT_MS")))
s = st()
chk("motion/rule11-near-slides", s["selection_motion"] == "slide" and s["selection_distance"] <= s["slide_max_rows"], "motion %s distance %s (max %s)" % (s["selection_motion"], s["selection_distance"], s["slide_max_rows"]))
rows = c.send("rows", "mac-main.list")["rows"]
try:
    c.send("click", rows[9]["ax"]); time.sleep(.3)
    s = st()
    chk("motion/rule11-far-fades", s["selection_motion"] == "fade" and s["selection_distance"] > s["slide_max_rows"], "motion %s distance %s" % (s["selection_motion"], s["selection_distance"]))
except Unreadable as e:
    nj("motion/rule11-far-fades", "click on row 9 refused: %s" % e)
n0 = len(c.send("rows", "mac-main.list")["rows"])
load = os.getloadavg()[0]; load_max = float(const("PERF_LOAD_MAX"))
t0 = time.time(); c.send("key", "E"); n1 = len(c.send("rows", "mac-main.list")["rows"]); dt = (time.time() - t0) * 1000
verdict = collapse_verdict(n0, n1, dt, load, load_max, float(const("INTERACTION_COMMIT_MS")))
line = "rows %d -> %d, %.1f ms over two socket round trips (the key, then the list read) (lock < %s); load %.1f (PERF_LOAD_MAX %g)" % (n0, n1, dt, const("INTERACTION_COMMIT_MS"), load, load_max)
if verdict == "not judged":
    nj("motion/rule3-collapse-model-same-frame", "not judged: the machine load is over the budget; " + line)
else:
    chk("motion/rule3-collapse-model-same-frame", verdict == "pass", line)
c.send("key", "Z")
try:
    c.send("measure", "mac-main.interaction.frame")
    nj("motion/interaction-gate", "served: run perf/interaction.py")
except Unreadable as e:
    nj("motion/interaction-gate", "perf/interaction.py cannot judge: measure mac-main.interaction.frame is not served (%s)" % str(e)[:80])
nj("motion/collapse-duration", "a window capture takes longer than ROW_COLLAPSE_MS (160): two shots cannot resolve it; no frame verb")

fails = [r for r in results if r[1] == "FAIL"]
unj = [r for r in results if r[1] == "NOTJUDGED"]
for n, s_, d in results:
    print("%-9s %s | %s" % (s_, n, d))
print("%d checks, %d failed, %d not judged" % (len(results), len(fails), len(unj)))
sys.exit(0 if not fails and not unj else 1)
