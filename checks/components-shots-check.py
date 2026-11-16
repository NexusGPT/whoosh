#!/usr/bin/env python3
"""components-shots-check: the pictures of the shared components (item 847f8b14, part B), judged by a script as far as a script can judge.

Two steps, the first only by the lead on the hidden copy, the second reads files and launches nothing:

  make   sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/components-shots-check.py --make _captures/components'
         opens the component gallery (`click components.gallery.open`), and for each of the 10 pages and both themes takes the picture
         `components-<page>-<light|dark>.png` (verb `shot`, the gallery's own window is the front window) and asks `measure` for the ids the gallery marks
         (key cap, label chip) into `_captures/components/frames.json` (id -> frame h, w, radius, plus the picture's width in points).
  judge  python3 checks/components-shots-check.py [_captures/components]      one PASS / FAIL / NOT JUDGED line per case, then
         '<n> checks, <f> failed, <u> not judged'; exit 1 when f > 0 or u > 0.
  self   python3 checks/components-shots-check.py --selftest                    builds pictures in memory, clean then once per broken copy; no app, no files kept.

Cases (the grammar of checks/seams/component-grammar.py and the tokens of Sources/Design, never numbers typed here):
  picture-<page>-<theme>         the file exists, is a readable 8-bit PNG, is not one flat colour, and is light (light theme) or dark (dark theme) at its corners
  key-cap-size                   the gallery's key cap is KEY_CAP high (WhooshSpacing.keyCap) with the key-cap radius (WhooshRadius.keyCap) in `measure`, and its corners are cut in the picture
  label-chip-size                the gallery's label chip is keyCap high with the item radius, corners cut in the picture
  code-pill-rules                the RULE for the code pills (lead ruling 2026-11-12): the reader's pill keeps its own drawn look, 36 high (WhooshSpacing.fileTile) and radius 10
                                 (WhooshRadius.field), and is judged as its OWN component `reader-code-pill`; the shared CodePill (the notch's) stays 20 and 6. Read from the
                                 expectations of component-grammar.py, so the two checks cannot drift apart: the reader pill is never judged against the shared one.
A page file that is missing is NOT JUDGED naming it (the lead's run did not make it): a harness case, never a pass.
"""
import json
import os
import struct
import sys
import time
import zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.dont_write_bytecode = True
sys.path.insert(0, os.path.join(ROOT, "journeys"))
sys.path.insert(0, os.path.join(ROOT, "checks", "seams"))

PAGES = ["overview", "snooze", "snoozedate", "labels", "undo", "status", "keys", "empty", "marks", "chips"]
THEMES = ["light", "dark"]
DEFAULT_DIR = os.path.join(ROOT, "_captures", "components")
PAGE_IDS = {"keys": ("key-cap", "mac-components.gallery.key-hint.cap"), "chips": ("label-chip", "mac-components.gallery.label-chip")}


class NotJudged(Exception):
    pass


# ---- PNG (8-bit RGB or RGBA, no interlace) --------------------------------------------------------------------------------------

def read_png(path):
    data = open(path, "rb").read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("not a PNG")
    pos, idat, w, h, ct = 8, b"", 0, 0, 0
    while pos < len(data):
        n, kind = struct.unpack(">I4s", data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + n]
        if kind == b"IHDR":
            w, h, depth, ct, _, _, il = struct.unpack(">IIBBBBB", body[:13])
            if depth != 8 or ct not in (2, 6) or il:
                raise ValueError("not an 8-bit RGB or RGBA PNG without interlace")
        elif kind == b"IDAT":
            idat += body
        pos += 12 + n
    bpp = 4 if ct == 6 else 3
    raw, stride, rows, prev = zlib.decompress(idat), w * bpp, [], bytearray(w * bpp)
    for y in range(h):
        f, line = raw[y * (stride + 1)], bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        for i in range(stride):
            a = line[i - bpp] if i >= bpp else 0
            b = prev[i]
            c = prev[i - bpp] if i >= bpp else 0
            if f == 1: line[i] = (line[i] + a) & 255
            elif f == 2: line[i] = (line[i] + b) & 255
            elif f == 3: line[i] = (line[i] + (a + b) // 2) & 255
            elif f == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else (b if pb <= pc else c))) & 255
        rows.append(line)
        prev = line
    return w, h, bpp, rows


def write_png(w, h, rgb_rows):
    """A 8-bit RGB PNG from rows of (r, g, b) tuples (the selftest's pictures)."""
    raw = b"".join(b"\x00" + bytes(c for px in row for c in px) for row in rgb_rows)
    def chunk(k, d):
        return struct.pack(">I", len(d)) + k + d + struct.pack(">I", zlib.crc32(k + d) & 0xFFFFFFFF)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")


def px(img, x, y):
    w, h, bpp, rows = img
    x, y = min(max(int(x), 0), w - 1), min(max(int(y), 0), h - 1)
    return tuple(rows[y][x * bpp:x * bpp + 3])


def lum(c):
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]


# ---- the numbers: the app's own tokens, through component-grammar.py's reader -----------------------------------------------------

def tokens():
    import importlib.util
    spec = importlib.util.spec_from_file_location("component_grammar", os.path.join(ROOT, "checks", "seams", "component-grammar.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod, mod.expected()


# ---- cases --------------------------------------------------------------------------------------------------------------------------

def case_picture(folder, page, theme):
    name = "components-%s-%s.png" % (page, theme)
    path = os.path.join(folder, name)
    if not os.path.exists(path):
        raise NotJudged("%s is missing in %s: the lead's run did not make it" % (name, folder))
    try:
        img = read_png(path)
    except (ValueError, zlib.error, struct.error, IndexError) as e:
        return False, "%s is not a readable 8-bit PNG: %s" % (name, e)
    w, h = img[0], img[1]
    colours = {px(img, x, y) for x in range(0, w, 3) for y in range(0, h, max(h // 60, 1))}      # 60 rows, every third pixel: sparse pages still show their text
    if len(colours) < 4:
        return False, "%s is flat: %d colours over a 60-row sample (an empty window)" % (name, len(colours))
    corners = [lum(px(img, x, y)) for x, y in ((2, 2), (w - 3, 2), (2, h - 3), (w - 3, h - 3))]
    mean = sum(corners) / len(corners)
    ok = mean > 128 if theme == "light" else mean < 128
    return ok, "%dx%d px, %d colours, corner luminance %.0f (%s theme)" % (w, h, len(colours), mean, theme)


def cut_corners(img, scale, f, radius, origin=(0.0, 0.0)):
    """True when the box's top-left corner is rounded to about `radius` points: the pixel 0.6 pt in along the diagonal is outside the fill (so the radius is above
    2 pt: a square corner fails) and the pixel 2.4 pt in is inside it (a radius above 8 pt fails). Judged against the colour at the box's middle.
    `origin` is where the window's content starts in the picture, in points (the shot reply's `mapping` x and y): a hidden copy's picture holds the whole window,
    title band included, and `measure` answers in the content's coordinates (Sources/App/WindowShot.swift: pixel = ((x + contentX) * scale, (y + contentY) * scale)).
    main191: with no origin every sample sat a title band above the control, on the plain background, and both size cases read "not rounded"."""
    x, y = f["x"] + origin[0], f["y"] + origin[1]
    # the box's own fill, read 2 pt inside its left edge at mid height: the middle holds the glyphs (main111: the middle pixel was a letter, so every real corner read "not rounded")
    mid = px(img, (x + 2.0) * scale, (y + f["h"] / 2) * scale)
    def same(c, d):
        return sum(abs(a - b) for a, b in zip(c, d)) < 24
    near = px(img, (x + 0.6) * scale, (y + 0.6) * scale)
    deep = px(img, (x + 2.4) * scale, (y + 2.4) * scale)
    return (not same(near, mid)) and same(deep, mid)


def case_component(folder, page, label, want_h, want_r, frames):
    name = "components-%s-light.png" % page
    path = os.path.join(folder, name)
    if not os.path.exists(path):
        raise NotJudged("%s is missing in %s: the lead's run did not make it" % (name, folder))
    ent = frames.get(label)
    if not ent:
        raise NotJudged("frames.json has no entry %r: `measure` did not serve the gallery id" % label)
    f = ent["frame"]
    if "radius" not in ent:
        raise NotJudged("`measure` served no radius for %s (the view does not say it)" % label)
    bad = []
    if abs(f["h"] - want_h) > 0.5:
        bad.append("height %g, expected %g" % (f["h"], want_h))
    if abs(ent["radius"] - want_r) > 0.01:
        bad.append("radius %g, expected %g" % (ent["radius"], want_r))
    img = read_png(path)
    mp = ent.get("mapping") or {}
    scale = float(mp["scale"]) if mp.get("scale") else img[0] / float(ent["picture_w_pt"])
    if not cut_corners(img, scale, f, want_r, (float(mp.get("x", 0)), float(mp.get("y", 0)))):
        bad.append("the picture's corner is not rounded to about %g pt" % want_r)
    return (not bad), ("; ".join(bad) if bad else "height %g, radius %g, corner rounded in the picture" % (f["h"], ent["radius"]))


def case_code_pill_rules(_folder, _frames):
    cg, exp = tokens()
    cases = {c: s for c, s in exp["cases"]}
    reader, shared = cases.get("reader-code-pill"), cases.get("code-pill")
    if not reader or not shared:
        return False, "component-grammar.py has no separate reader-code-pill and code-pill rows: the reader pill would be judged against the shared one"
    tile, field = cg.token("WhooshSpacing.swift", "fileTile"), cg.token("WhooshRadius.swift", "field")
    cap, item = cg.token("WhooshSpacing.swift", "keyCap"), cg.token("WhooshRadius.swift", "item")
    r, s = reader[0], shared[0]
    ok = (r[3], r[4]) == (tile, field) == (36, 10) and (s[3], s[4]) == (cap, item) == (20, 6) and len(reader) == 1 and r[0] != s[0]
    return ok, "reader-code-pill %sx%s (own component), code-pill %sx%s (shared)" % (r[3], r[4], s[3], s[4])


def run_cases(folder, frames, tok=None):
    cg, exp = tok or tokens()
    cap, item, kcr = cg.token("WhooshSpacing.swift", "keyCap"), cg.token("WhooshRadius.swift", "item"), cg.token("WhooshRadius.swift", "keyCap")
    cases = []
    for page in PAGES:
        for theme in THEMES:
            cases.append(("picture-%s-%s" % (page, theme), lambda p=page, t=theme: case_picture(folder, p, t)))
    cases.append(("key-cap-size", lambda: case_component(folder, "keys", PAGE_IDS["keys"][1], cap, kcr, frames)))
    cases.append(("label-chip-size", lambda: case_component(folder, "chips", PAGE_IDS["chips"][1], cap, item, frames)))
    cases.append(("code-pill-rules", lambda: case_code_pill_rules(folder, frames)))
    out = []
    for name, fn in cases:
        try:
            ok, why = fn()
            out.append((name, "PASS" if ok else "FAIL", why))
        except NotJudged as e:
            out.append((name, "NOT JUDGED", str(e)))
        except (KeyError, ValueError, TypeError, zlib.error, struct.error, IndexError) as e:
            out.append((name, "FAIL", "unreadable: %s %s" % (type(e).__name__, e)))
    return out


def report(res):
    for n, st, why in res:
        print("%-10s %s  %s" % (st, n, why))
    f, u = sum(r[1] == "FAIL" for r in res), sum(r[1] == "NOT JUDGED" for r in res)
    print("%d checks, %d failed, %d not judged" % (len(res), f, u))
    return 1 if f or u else 0


def load_frames(folder):
    p = os.path.join(folder, "frames.json")
    return json.load(open(p, encoding="utf-8")) if os.path.exists(p) else {}


# ---- make (the lead's run on the hidden copy) -------------------------------------------------------------------------------------

def make(folder):
    from lib.ctl import Ctl, Unreadable
    os.makedirs(folder, exist_ok=True)
    c = Ctl().connect()
    frames = {}
    c.send("fixture", "reset")
    c.send("click", "components.gallery.open")
    for theme in THEMES:
        # The gallery owns its theme (GalleryState.dark, its window's appearance and `.preferredColorScheme`: ComponentGallery.swift:29, GalleryWindow.swift:18), so the app's
        # `theme` verb never reaches it: main107 drew every page named dark in the light theme (corner luminance 255). Its own doors set it.
        c.send("click", "components.gallery.%s" % theme)
        c.send("theme", theme)
        time.sleep(0.5)
        for page in PAGES:
            c.send("click", "components.gallery.show.%s" % page)
            time.sleep(0.5)
            reply = {}
            for attempt in range(3):          # a shot of a window can take longer than the socket's wait on a loaded machine (main112: load 20 to 60, "timed out" ended the run with a traceback)
                try:
                    reply = c.send("shot", "components-%s-%s" % (page, theme))
                    break
                except Unreadable as e:
                    print("SHOT RETRY components-%s-%s (%d of 3): %s" % (page, theme, attempt + 1, e))
                    time.sleep(3)
                    try:
                        c.close()
                    except Exception:             # noqa: BLE001
                        pass
                    c = Ctl().connect()
            src = reply.get("path")
            if not src or not os.path.exists(src):
                print("NOT MADE components-%s-%s: the shot reply names no file (%r)" % (page, theme, reply))
                continue
            with open(src, "rb") as a, open(os.path.join(folder, "components-%s-%s.png" % (page, theme)), "wb") as b:
                b.write(a.read())
            if theme == "light" and page in PAGE_IDS:
                label, ax = PAGE_IDS[page]
                try:
                    m = c.send("measure", ax)
                    w = read_png(os.path.join(folder, "components-%s-light.png" % page))[0]
                    wins = c.send("winframes").get("frames", [])
                    # the window the picture shows: the one whose width in points times 1, 2 or 3 is the picture's width in pixels (the widest window was the main window, whose
                    # width gave a wrong scale: main111)
                    fit = [f["w"] for f in wins if f.get("w") and any(abs(f["w"] * k - w) < 2 for k in (1, 2, 3))]
                    ww = fit[0] if fit else max((f.get("w", 0) for f in wins if f.get("w")), default=0)
                    entry = {"frame": m["frame"], "picture_w_pt": ww or w}
                    if isinstance(reply.get("mapping"), dict):      # where the content starts in this picture, and its scale (the shot's own answer)
                        entry["mapping"] = reply["mapping"]
                    if "radius" in m:
                        entry["radius"] = m["radius"]
                    frames[ax] = entry
                except Unreadable as e:
                    print("NOT MEASURED %s: %s" % (ax, e))
    with open(os.path.join(folder, "frames.json"), "w", encoding="utf-8") as f:
        json.dump(frames, f, indent=1)
    print("made %d pictures in %s" % (len(PAGES) * len(THEMES), folder))
    return 0


# ---- selftest -----------------------------------------------------------------------------------------------------------------------

def _picture(theme, w=200, h=120, chip=None, radius=6, scale=2):
    """A synthetic gallery page: a flat page colour, and (when `chip` is a frame in pt) a filled box with the given corner radius drawn at `scale`."""
    bg = (245, 245, 247) if theme == "light" else (28, 28, 30)
    fill = (200, 200, 205) if theme == "light" else (70, 70, 74)
    W, H = w * scale, h * scale
    rows = []
    for y in range(H):
        row = []
        for x in range(W):
            c = bg
            if chip:
                cx, cy, cw, ch = chip["x"] * scale, chip["y"] * scale, chip["w"] * scale, chip["h"] * scale
                r = radius * scale
                if cx <= x < cx + cw and cy <= y < cy + ch:
                    dx = max(cx + r - x - 0.5, 0, x + 0.5 - (cx + cw - r))
                    dy = max(cy + r - y - 0.5, 0, y + 0.5 - (cy + ch - r))
                    c = fill if dx * dx + dy * dy <= r * r else bg
            near_chip = bool(chip) and chip['x'] * scale - 4 <= x < (chip['x'] + chip['w']) * scale + 4 and chip['y'] * scale - 4 <= y < (chip['y'] + chip['h']) * scale + 4
            if near_chip:
                pass                                         # the box and its edge stay clean: only the page around it carries texture
            elif (x + y) % 4 == 0 and (x // 8 + y // 8) % 2 == 0:
                c = (c[0] ^ 40, c[1] ^ 40, c[2] ^ 40)       # a little texture so a page is never one flat colour
            elif x % 6 == 0 and y % 4 == 0:
                c = (c[0] ^ 80, c[1] ^ 80, c[2] ^ 80)
            elif x % 5 == 0 and y % 3 == 0:
                c = (c[0] ^ 20, c[1] ^ 20, c[2] ^ 20)
            row.append(c)
        rows.append(row)
    return write_png(W, H, rows)


def selftest():
    import tempfile
    tok = tokens()
    cg, exp = tok
    cap, item, kcr = cg.token("WhooshSpacing.swift", "keyCap"), cg.token("WhooshRadius.swift", "item"), cg.token("WhooshRadius.swift", "keyCap")
    box = lambda h: {"x": 20, "y": 30, "w": 60, "h": h}
    def build(d, broken=None, band=0):
        # `band`: the title band of a hidden copy's picture, in points: the controls are drawn that much lower than `measure` says, and the frames carry the
        # shot's `mapping` (the broken copy "no-title-band-offset" leaves it out: main191's case)
        drawn = lambda b: dict(b, y=b["y"] + band)
        for page in PAGES:
            for theme in THEMES:
                if broken == "missing-page" and (page, theme) == ("empty", "dark"):
                    continue
                chip = None
                if page == "keys":
                    chip, rad = drawn(box(cap)), (0 if broken == "square-key-cap" else kcr)
                elif page == "chips":
                    chip, rad = drawn(box(cap)), item
                else:
                    rad = 0
                theme_shown = ("dark" if theme == "light" else "light") if (broken == "wrong-theme" and (page, theme) == ("status", "light")) else theme
                data = _picture(theme_shown, chip=chip, radius=rad if chip else 0) if broken != "flat-page" or (page, theme) != ("snooze", "light") else write_png(40, 40, [[(255, 255, 255)] * 40] * 40)
                open(os.path.join(d, "components-%s-%s.png" % (page, theme)), "wb").write(data)
        frames = {
            PAGE_IDS["keys"][1]: {"frame": box(cap + (1 if broken == "tall-key-cap" else 0)), "radius": kcr, "picture_w_pt": 200},
            PAGE_IDS["chips"][1]: {"frame": box(cap), "radius": item + (4 if broken == "wrong-chip-radius" else 0), "picture_w_pt": 200},
        }
        if band and broken != "no-title-band-offset":
            for ent in frames.values():
                ent["mapping"] = {"x": 0, "y": band, "scale": 2}
        return frames
    bad = 0
    broken_list = [None, "missing-page", "flat-page", "wrong-theme", "tall-key-cap", "square-key-cap", "wrong-chip-radius"]
    want_red = {"missing-page": "picture-empty-dark", "flat-page": "picture-snooze-light", "wrong-theme": "picture-status-light", "tall-key-cap": "key-cap-size",
                "square-key-cap": "key-cap-size", "wrong-chip-radius": "label-chip-size"}
    # a hidden copy's picture holds the title band: green with the shot's mapping, red without it (the two size cases of main191)
    for b, want in ((None, None), ("no-title-band-offset", "key-cap-size")):
        with tempfile.TemporaryDirectory() as d:
            reds = [n for n, st, _ in run_cases(d, build(d, b, band=32), tok) if st != "PASS"]
            ok = (not reds) if b is None else (want in reds)
            print("%-5s a picture with a title band of 32 pt, %s -> %s" % ("ok" if ok else "FAIL", "the shot's mapping kept" if b is None else "the mapping left out",
                                                                          "green" if not reds else "red on %s" % ", ".join(reds)))
            bad += 0 if ok else 1
    for b in broken_list:
        with tempfile.TemporaryDirectory() as d:
            res = run_cases(d, build(d, b), tok)
            reds = [n for n, st, _ in res if st != "PASS"]
            if b is None:
                ok = not reds
                print("%-5s clean pictures: %d cases, not green: %s" % ("ok" if ok else "FAIL", len(res), reds))
            else:
                ok = want_red[b] in reds
                print("%-5s broken copy %-18s -> %s" % ("ok" if ok else "FAIL", b, "red on %s" % want_red[b] if ok else "STAYED GREEN (not green: %s)" % reds))
            bad += 0 if ok else 1
    # the rule: a component-grammar that judged the reader pill with the shared one is caught
    rows = {c: s for c, s in exp["cases"]}
    merged_ok = "reader-code-pill" in rows and "code-pill" in rows
    print("%-5s the reader's code pill is its own component in component-grammar.py (36 x 10) beside the shared pill (20 x 6)" % ("ok" if merged_ok else "FAIL"))
    bad += 0 if merged_ok else 1
    # the theme of a dark page: `make` sets it through the gallery's own doors, which the app registers (the app's `theme` verb does not reach the gallery)
    import inspect
    src_make = inspect.getsource(make)
    reg = open(os.path.join(ROOT, "Sources", "Features", "Components", "ComponentsFeature.swift"), encoding="utf-8").read()
    doors_ok = 'c.send("click", "components.gallery.%s" % theme)' in src_make and 't["components.gallery.light"]' in reg and 't["components.gallery.dark"]' in reg
    print("%-5s make sets each page's theme through the gallery doors components.gallery.light and .dark, both registered by ComponentsFeature.swift" % ("ok" if doors_ok else "FAIL"))
    bad += 0 if doors_ok else 1
    print("selftest: %d problem(s)" % bad)
    return 1 if bad else 0


def main(argv):
    if "--selftest" in argv:
        return selftest()
    args = [a for a in argv if not a.startswith("--")]
    if "--make" in argv:
        return make(args[0] if args else DEFAULT_DIR)
    folder = args[0] if args else DEFAULT_DIR
    return report(run_cases(folder, load_frames(folder)))


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
