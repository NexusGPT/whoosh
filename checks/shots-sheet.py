#!/usr/bin/env python3
"""shots-sheet: a picture of every Mac screen in both themes at both window sizes, clipped labels measured, one local page.

Run (the lead, hidden rig, fixture core; never against the owner's installed app):
  sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" perl -e "alarm 600; exec @ARGV" python3 checks/shots-sheet.py <outdir>'
  python3 checks/shots-sheet.py --selftest      (stand-in socket, no app)
Options: --screens a,b,c  (only these screens)   --deadline SECONDS (default 540; screens left are NOT JUDGED)

Writes <outdir>/index.html and <outdir>/pics/*.png. Prints one PASS / FAIL / NOT JUDGED line per screen and state, then
'<n> checks, <f> failed, <u> not judged'. Exit 1 when f > 0 or u > 0. Reach, theme, size and picture use only verbs the app serves:
fixture reset, shot <surface>, shot <name> window:<id> (the id read from winframes), resize, theme, uistate, rows, labels, winframes.
Clipped labels come from `labels window:<id>` (every marked view with text in the pictured window); an app without that verb
answers `unknown verb labels` and the screen's clipped labels stay NOT JUDGED naming it.
Docs: docs/lanes/shots-sheet.md, docs/lanes/labels-verb.md.

Named states (lane gap-shots, docs/lanes/gap-shots.md): a screen is only its first look; STATES drives the hidden copy to a named state with
the socket verbs written beside it (click, key, type, rows, fixture, scroll) and pictures it in both themes at the sizes the state names:
  python3 checks/shots-sheet.py <outdir> --states all          (or --states id1,id2)   run them (same launch line as above; --deadline 1500)
  python3 checks/shots-sheet.py --list-states                   dry: every state, its verbs, sizes, themes and picture files; no socket, exit 0
A state whose step the app refused (or whose rows never said the state was reached) is NOT JUDGED naming the step; its picture is still saved.
"""
import html
import json
import os
import shutil
import sys
import tempfile
import time

sys.dont_write_bytecode = True
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
sys.path.insert(0, os.path.join(ROOT, "checks"))
from lib import ctl as ctlmod                                    # noqa: E402
from lib.ctl import Ctl, CouldNotDrive, Unreadable              # noqa: E402

GEO = os.path.join(ROOT, "docs", "design", "geometry")
THEMES = ("light", "dark")
SIZE_TOL = 1.5          # points
MAX_STEPS = 12          # verbs one state may send: a cap, never a loop without end

# (screen, geometry file, how it is reached)
SCREENS = [
    ("mac-main", "mac-main.json"), ("mac-composer", "mac-main.json"), ("mac-search", "mac-main.json"),
    ("mac-palette", "mac-main.json"), ("mac-trainer", "mac-main.json"), ("mac-notch", "mac-notch.json"),
    ("mac-welcome-accounts", "mac-first-start.json"), ("mac-settings-accounts", "mac-settings.json"),
    ("mac-settings-ai", "mac-settings.json"), ("mac-settings-decisions", "mac-settings.json"), ("mac-settings-general", "mac-settings.json"),
    ("mac-labels-contacts", "mac-main.json"), ("mac-mcp-notes", "mac-settings.json"),
    ("mac-quickreply", "mac-main.json"), ("mac-settings-replies", "mac-settings.json"),
]
# `shot <surface>` opens the surface's window (CONTROL-SOCKET delta 17). mac-quickreply has a fallback: the R key on mac-main
# (checks/parity/par-quickreply.py `s.key("R")`) and a shot of the front window.
FALLBACK = {"mac-quickreply": [("key", "R")]}
# hid-ai round 18 (main108: every sheet-mac-settings-ai picture showed the Decisions pane): `shot mac-settings-ai` opens the Settings window on DECISIONS,
# because the two panes share one surface slot and the slot names Decisions (Sources/Features/SettingsWindow/SettingsPane.swift `pane(for:)`, case
# .macSettingsAI: return .decisions; `slot` gives .macSettingsAI for both .decisions and .ai). So each pane is its own screen: the surface `shot` opens, then
# the sidebar row of the pane (`mac-settings-window.pane-row.<pane>`), read back from `rows mac-settings-window.place` before any picture.
SETTINGS_W = "mac-settings-window"
SCREEN_PANE = {"mac-settings-ai": ("mac-settings-ai", "ai"), "mac-settings-decisions": ("mac-settings-ai", "decisions")}
PANE_WAIT_S = 3.0
# A label is clipped when what its text needs exceeds its box by more than this, in points: SwiftUI lays text out on fractional
# points and snaps frames to the pixel grid (half a point at 2x is one pixel), so a smaller difference is rounding, not a cut glyph.
CLIP_TOL_PT = 0.5
# Read beside `labels`: the status link carries a plain .accessibilityIdentifier("mac-main.status.link") (Sources/Features/MacShell/MacSyncStatus.swift:172),
# not .whooshAx, so `labels` does not serve it. Its row is served under the ROW SOURCE "mac-main.sync.line" (MacShellFeature.swift:9 registers
# "mac-main.sync.line": MacShellActions.syncRows); the row's own `ax` is "mac-main.status.link" (MacShellActions.swift:14), and `rows
# mac-main.status.link` is refused ("no rows at mac-main.status.link": no row source has that name). The key here is the row source.
FIT_REGIONS = {"mac-main": [("mac-main.sync.line", [("text_w", "box_w", "text"), ("line_w", "line_box_w", "line")])]}
RETRIES = 4             # `shot ... window:<id>` attempts: SwiftUI renames a window's identifier after the app tagged it (see on_window)
RETRY_S = 0.3
FIT_MISSING = "the app answered `unknown verb labels` (an older build), so no needed text width and height is served beside a frame here"


class Fail(Exception):
    pass


def geometry(fname):
    with open(os.path.join(GEO, fname), encoding="utf-8") as f:
        g = json.load(f)
    return g["window"], g["min"]


def sizes_for(screen, fname):
    d, m = geometry(fname)
    out = [("default", int(d["w"]), int(d["h"]))]
    if (m["w"], m["h"]) != (d["w"], d["h"]):
        out.append(("min", int(m["w"]), int(m["h"])))
    return out


def inspect(path):
    """Decode one picture: (w, h, one_colour). Raises Fail when it is missing, empty or unreadable."""
    from PIL import Image
    if not os.path.isfile(path):
        raise Fail("picture file is missing: %s" % path)
    if os.path.getsize(path) == 0:
        raise Fail("picture file is empty: %s" % path)
    try:
        with Image.open(path) as im:
            im.load()
            w, h = im.size
            one = im.convert("RGB").getcolors(maxcolors=1) is not None
    except Exception as e:
        raise Fail("picture does not decode (%s): %s" % (type(e).__name__, path))
    return w, h, one


def same_picture(a, b):
    from PIL import Image, ImageChops
    with Image.open(a) as ia, Image.open(b) as ib:
        if ia.size != ib.size:
            return False
        return ImageChops.difference(ia.convert("RGB"), ib.convert("RGB")).getbbox() is None


def resolve_window(c, screen, reply_id):
    """The id of the open window that draws `screen`, read from the app's own `winframes` answer, never guessed and never by title.
    Sources/App/WindowBridge.swift:30-34 (`frames()`): each row carries `id` (the window's identifier now) and `surface` (the screen a
    surface window was opened for, WindowBridge.swift:61 `surfaceId(of:)`). A surface window (search, palette, trainer, notch, welcome,
    labels, mcp notes, quick reply) is found by its `surface`; the `window` of the `shot` reply can be SwiftUI's own name
    (`surface-AppWindow-1`) that the window has no longer, so it is used only for a screen that is drawn in the main or the Settings
    window, and only when `winframes` lists it. Raises Unreadable naming the open windows when none draws the screen."""
    frames = c.winframes()
    rows = [f for f in frames if isinstance(f, dict)]
    by_surface = [f.get("id") for f in rows if f.get("surface") == screen and f.get("id")]
    if by_surface:
        return by_surface[0]
    ids = [f.get("id") for f in rows if f.get("id")]
    if reply_id and reply_id in ids:
        return reply_id
    raise Unreadable("no open window draws %s: `shot` named %r, `winframes` lists (id|surface): %s" % (
        screen, reply_id, ", ".join("%s|%s" % (f.get("id", ""), f.get("surface", "")) for f in rows) or "none"))


def on_window(c, screen, reply_id, act):
    """act(window id) with the id read now from `winframes`. SwiftUI sets a window's identifier to its own name (`surface-AppWindow-1`)
    and the app tags it with the screen id a moment later (ShellScenes.swift:41-43 tags again on the next turn), so an id read a
    moment ago can be refused ("no open window has the id ..."): read again and ask again, RETRIES times, never by title."""
    for attempt in range(RETRIES):
        wid = resolve_window(c, screen, reply_id)
        try:
            return act(wid)
        except Unreadable as e:
            if "no open window has the id" not in str(e) or attempt == RETRIES - 1:
                raise
            time.sleep(RETRY_S)


READY_TRIES, READY_WAIT_S = 3, 1.0   # a screen's first picture can come before its mail page says ready (main200 and main202: the search screen, 3.2 s)


def shot_when_ready(take, wait=None):
    """Takes the picture; while its `source` says `web: not ready`, waits and takes it again: READY_TRIES pictures at most. -> (the last reply, pictures taken).
    A page that is never ready stays NOT JUDGED at the caller: a blank or ghost body is never passed."""
    sr, n = take(), 1
    while "web: not ready" in str(sr.get("source", "")) and n < READY_TRIES:
        time.sleep(READY_WAIT_S if wait is None else wait)
        sr, n = take(), n + 1
    return sr, n


def clipped_labels(c, screen):
    """(names of clipped labels, regions read) or raises Unreadable. A label is clipped when its needed width exceeds its box."""
    names, read = [], []
    for region, pairs in FIT_REGIONS.get(screen, []):
        rows = c.rows(region)
        read.append(region)
        for r in rows:
            for need, box, lab in pairs:
                n, b = r.get(need), r.get(box)
                if isinstance(n, (int, float)) and isinstance(b, (int, float)) and n > b + CLIP_TOL_PT:
                    names.append("%s [%s needs %.0f pt, frame %.0f pt]" % (r.get(lab, r.get("ax", region)), r.get("ax", region), n, b))
    return names, read


MIN_READABLE = 6   # characters of a single-line label that must stay readable before its ellipsis


def ellipsis_cut(text, nw, nh, bw, bh):
    """True when a label's box is narrower than its text but the cut is the NORMAL one: a single line (the box holds the line's height, so nothing
    wraps or is cut at the bottom) in a box the text view itself was given (SwiftUI ends a one-line Text laid out narrower than its ideal width with an
    ellipsis; a word cut by a parent's edge leaves the view's own box at full width), with at least MIN_READABLE characters left before the ellipsis.
    The characters kept are estimated from the share of the width the box has, less one for the ellipsis itself. Anything else stays clipped."""
    if nh > bh + CLIP_TOL_PT or bw <= 0 or nw <= 0:
        return False
    return int(len(text) * min(1.0, bw / nw)) - 1 >= MIN_READABLE


def labels_verb(c, wid):
    """(clipped names, judged, marked, capped, ax ids answered with text null) from `labels window:<wid>`; None when the app has no `labels` verb.
    A row with `text: null` is not counted. Raises Unreadable for any other refusal, a malformed row, or counts that disagree
    with the rows (judged must equal the rows with text, marked the rows)."""
    try:
        r = c.send("labels", "window:%s" % wid)
    except Unreadable as e:
        if "unknown verb labels" in str(e):
            return None
        raise
    rows, judged, marked, capped = r.get("labels"), r.get("judged"), r.get("marked"), r.get("capped")
    if not isinstance(rows, list) or not isinstance(judged, int) or not isinstance(marked, int) or not isinstance(capped, bool):
        raise Unreadable("`labels` answered without labels, judged, marked and capped")
    names, counted, nulls = [], 0, []
    for x in rows:
        if not isinstance(x, dict) or x.get("text") is None:
            if isinstance(x, dict):
                nulls.append(str(x.get("ax")))
            continue
        sz = [x.get(k) for k in ("need_w", "need_h", "box_w", "box_h")]
        if not all(isinstance(v, (int, float)) and not isinstance(v, bool) for v in sz):
            raise Unreadable("`labels` row %s lacks a size: %s" % (x.get("ax"), sz))
        counted += 1
        nw, nh, bw, bh = sz
        if nw > bw + CLIP_TOL_PT and nh <= bh + CLIP_TOL_PT and ellipsis_cut(x["text"], nw, nh, bw, bh):
            continue   # cut with an ellipsis, enough of it readable: a normal cut beside a fixed control, not a clipped word (lead ruling, main92)
        if nw > bw + CLIP_TOL_PT or nh > bh + CLIP_TOL_PT:
            names.append('%s "%s" [needs %.1fx%.1f pt, box %.1fx%.1f pt]' % (x.get("ax"), x["text"], nw, nh, bw, bh))
    if counted != judged or len(rows) != marked:
        raise Unreadable("`labels` counts disagree with its rows: judged %d, marked %d, rows with text %d, rows %d" % (judged, marked, counted, len(rows)))
    return names, judged, marked, capped, nulls


def new_state(screen, theme, label, w, h):
    return {"screen": screen, "theme": theme, "size": label, "req": (w, h), "verdict": None, "why": [], "pic": None,
            "dims": None, "scale": 1, "settled": None, "clipped": [], "clip_note": ""}


def judge(st, verdict, why):
    order = {"PASS": 0, "NOT JUDGED": 1, "FAIL": 2}
    if st["verdict"] is None or order[verdict] > order[st["verdict"]]:
        st["verdict"] = verdict
    st["why"].append((verdict, why))


def run_screen(c, screen, fname, outdir, per_state):
    sizes = sizes_for(screen, fname)
    fixed = len(sizes) == 1
    states = [new_state(screen, t, lab, w, h) for lab, w, h in sizes for t in THEMES]

    def all_nj(why):
        for st in states:
            judge(st, "NOT JUDGED", why)
        return states

    try:
        c.send("fixture", "reset")
    except Unreadable as e:
        return all_nj("verb `fixture reset` failed: %s" % e)
    surface, pane = SCREEN_PANE.get(screen, (screen, None))
    opened = None
    try:
        opened = c.shot(surface)
    except Unreadable as e:
        steps = FALLBACK.get(screen)
        if not steps:
            return all_nj("reach by `shot %s` failed: %s" % (screen, e))
        try:
            for s in steps:
                c.send(*s)
            opened = c.shot("sheet-open-" + screen)
        except Unreadable as e2:
            return all_nj("reach by `shot %s` failed (%s) and by key R then `shot` failed (%s)" % (screen, e, e2))
    reply_id = opened.get("window")   # may be SwiftUI's own name for the window; resolve_window reads the served id
    try:
        resolve_window(c, surface, reply_id)
    except Unreadable as e:
        return all_nj("the window of this screen cannot be named: %s" % e)
    if pane:
        try:
            c.click("%s.pane-row.%s" % (SETTINGS_W, pane))
            end, shown = time.time() + PANE_WAIT_S, None
            while True:
                shown = (c.rows(SETTINGS_W + ".place") or [{}])[0].get("pane")
                if shown == pane or time.time() >= end:
                    break
                time.sleep(POLL_EVERY)
        except Unreadable as e:
            return all_nj("the pane %s could not be opened by its sidebar row: %s" % (pane, e))
        if shown != pane:
            return all_nj("the Settings window shows the pane %r, not %r, %.0f s after the click on %s.pane-row.%s (rows %s.place)" % (shown, pane, PANE_WAIT_S, SETTINGS_W, pane, SETTINGS_W))
    for st in states:
        lab, (w, h) = st["size"], st["req"]
        try:
            if not fixed:
                r = c.send("resize", w, h)
                cont = r.get("content") or {}
                st["settled"] = (float(cont["w"]), float(cont["h"]))
            theme = st["theme"]
            tr = c.send("theme", theme)
            ui = c.uistate()
            name = "sheet-%s-%s-%dx%d" % (screen, theme, w, h)
            sr, _ = shot_when_ready(lambda: on_window(c, surface, reply_id, lambda wid: c.send("capture", "shot", name, "window:%s" % wid)))
        except (Unreadable, KeyError, ValueError, TypeError) as e:
            judge(st, "NOT JUDGED", "a verb for this state failed or answered without its field: %s" % e)
            continue
        if tr.get("theme") != theme or ui.get("theme") != theme:
            judge(st, "FAIL", "theme did not take: asked %s, `theme` replied %s, uistate.theme is %s" % (theme, tr.get("theme"), ui.get("theme")))
        if "web: not ready" in str(sr.get("source", "")):
            judge(st, "NOT JUDGED", "the shot says the page was not ready (%s): a blank or ghost body is never passed" % sr.get("source"))
        src = sr.get("path", "")
        dest_rel = "pics/%s.png" % name
        dest = os.path.join(outdir, dest_rel)
        try:
            os.makedirs(os.path.dirname(dest), exist_ok=True)
            if not src or not os.path.isfile(src):
                raise Fail("picture file is missing: %s" % src)
            shutil.copyfile(src, dest)
            pw, ph, one = inspect(dest)
        except (Fail, OSError) as e:
            judge(st, "FAIL", str(e))
            continue
        st["pic"] = dest_rel
        mp = sr.get("mapping") or {}
        scale = mp.get("scale") or 1
        st["scale"] = scale
        cw, ch = pw / scale, ph / scale - (mp.get("y") or 0)
        st["dims"] = (cw, ch)
        if one:
            judge(st, "FAIL", "picture is one colour")
        if not fixed:
            sw, sh = st["settled"]
            if sw < w - SIZE_TOL or sh < h - SIZE_TOL:
                judge(st, "FAIL", "size not applied: asked %dx%d, the window settled at %.0fx%.0f" % (w, h, sw, sh))
            elif abs(cw - sw) > SIZE_TOL or abs(ch - sh) > SIZE_TOL:
                judge(st, "FAIL", "picture is %.0fx%.0f pt content but `resize` settled at %.0fx%.0f" % (cw, ch, sw, sh))
        if screen == "mac-composer":
            # c7aed2f9 proof 3: the title band of the composer window reads the catalogue word, never the screen id. The band is read from `winbands` (the window's own
            # title and band height) beside the picture; a title that is an id is a FAIL, a band that is not served is said, never a pass.
            try:
                wid_c = resolve_window(c, surface, reply_id)
                band = next((b for b in c.send("winbands").get("bands", []) if b.get("id") == wid_c), None)
                if band is None:
                    st["band_note"] = "winbands lists no band for %s" % wid_c
                    judge(st, "NOT JUDGED", "title band not read: " + st["band_note"])
                elif str(band.get("title", "")).startswith("mac-"):
                    judge(st, "FAIL", "the composer window's title band reads %r, an id and not a word" % band.get("title"))
                else:
                    st["band_note"] = "title band reads %r (band %s pt, picture content offset %s pt)" % (band.get("title"), band.get("band"), mp.get("y") or 0)
            except Unreadable as e:
                judge(st, "NOT JUDGED", "title band not read: `winbands` failed: %s" % e)
        try:
            lab = on_window(c, surface, reply_id, lambda wid: labels_verb(c, wid))
            names, read = clipped_labels(c, screen)
        except Unreadable as e:
            judge(st, "NOT JUDGED", "clipped labels not judged: `labels` or `rows` failed: %s" % e)
            st["clip_note"] = "`labels` or `rows` failed: %s" % e
            lab, names, read = False, None, []
        if lab is None:   # an older build: only the regions served by `rows`, as before the verb
            if not read:
                st["clip_note"] = FIT_MISSING
                judge(st, "NOT JUDGED", "clipped labels not judged: " + FIT_MISSING)
            else:
                st["clipped"] = names
                st["clip_note"] = "read %s only (%s)" % (", ".join(read), FIT_MISSING)
                if names:
                    judge(st, "FAIL", "clipped label%s: %s" % ("s" if len(names) > 1 else "", "; ".join(names)))
                else:
                    judge(st, "PASS", "no clipped label in %s (partial coverage: `labels` is missing)" % ", ".join(read))
        elif lab:
            lnames, judged, marked, capped, nulls = lab
            allnames = lnames + names
            st["clipped"] = allnames
            st["clip_note"] = "labels judged %d of %d marked views (the rest are not leaf texts%s)%s" % (
                judged, marked, "; capped: copies ran out, more were not judged" if capped else "", ("; also read " + ", ".join(read)) if read else "")
            if judged == 0 and marked > 0:
                st["clip_note"] += ("; `labels` answered text null for all %d (%s%s): buttons, fields and containers, none a bare Text the verb can copy,"
                                    " so this is the app's marking, not the check (docs/lanes/shots-sheet.md, 2026-11-11)") % (
                                        marked, ", ".join(nulls[:5]), ", ..." if len(nulls) > 5 else "")
            if allnames:
                judge(st, "FAIL", "clipped label%s: %s" % ("s" if len(allnames) > 1 else "", "; ".join(allnames)))
            elif judged == 0:
                judge(st, "NOT JUDGED", "clipped labels not judged: %s" % st["clip_note"])
            else:
                judge(st, "PASS", "no clipped label: %s%s" % (st["clip_note"], ("; " + st["band_note"]) if st.get("band_note") else ""))
        if st["verdict"] is None:
            judge(st, "PASS", "ok")
        # a state that is only PASS because of the picture checks keeps that verdict
    # pair checks across states of this screen
    by = {(s["theme"], s["size"]): s for s in states}
    for lab, w, h in sizes:
        lt, dk = by[("light", lab)], by[("dark", lab)]
        if lt["pic"] and dk["pic"]:
            try:
                if same_picture(os.path.join(outdir, lt["pic"]), os.path.join(outdir, dk["pic"])):
                    judge(dk, "FAIL", "theme did not change the picture: light and dark are pixel-identical at %s" % lab)
            except Exception as e:
                judge(dk, "NOT JUDGED", "light and dark pictures could not be compared: %s" % e)
    if not fixed:
        for th in THEMES:
            a, b = by[(th, "default")], by[(th, "min")]
            if a["dims"] and b["dims"] and max(abs(a["dims"][0] - b["dims"][0]), abs(a["dims"][1] - b["dims"][1])) <= SIZE_TOL:
                judge(b, "FAIL", "size did not change: default and min pictures are both %.0fx%.0f pt" % a["dims"])
    # a state with no explicit verdict but only PASS items keeps PASS; a state whose last item says PASS after a FAIL keeps FAIL
    return states


def line_for(st):
    d = st["dims"]
    dims = "%.0fx%.0f" % d if d else "no picture"
    head = "%s %s %s %s (asked %dx%d)" % (st["verdict"], st["screen"], st["theme"], st["size"], st["req"][0], st["req"][1])
    reasons = [w for v, w in st["why"] if v == st["verdict"]] or [w for v, w in st["why"]]
    return "%s: picture %s pt; %s" % (head, dims, " | ".join(reasons))


def write_page(outdir, rows, sizes_used, notes):
    e = html.escape
    out = ["<!doctype html>", '<html lang="en"><head><meta charset="utf-8"><title>Whoosh Mac screens: pictures to score</title>',
           "<style>",
           ":root{color-scheme:light dark;--bg:#f4f5f7;--fg:#15171a;--mut:#5b6169;--card:#fff;--line:#d9dce1;--ok:#1a7f4b;--bad:#b3261e;--nj:#8a5a00}",
           "@media (prefers-color-scheme:dark){:root{--bg:#141517;--fg:#eceef1;--mut:#9aa1ab;--card:#1e2023;--line:#33363b;--ok:#4cc38a;--bad:#ff8a80;--nj:#e0b050}}",
           "body{margin:0;padding:32px 40px 80px;background:var(--bg);color:var(--fg);font:15px/1.45 -apple-system,system-ui,'Helvetica Neue',sans-serif}",
           "h1{font-size:26px;margin:0 0 4px}h2{font-size:19px;margin:40px 0 4px}p.sub{color:var(--mut);margin:0 0 8px}",
           "section{border-top:1px solid var(--line);padding-top:8px}",
           ".row{display:flex;gap:20px;overflow-x:auto;padding:12px 0 8px;align-items:flex-start}",
           ".tile{flex:0 0 auto;background:var(--card);border:1px solid var(--line);border-radius:10px;padding:12px}",
           ".tile img{display:block;max-width:none;border:1px solid var(--line)}",
           ".cap{margin-top:8px;font-size:13px;max-width:520px}.cap b{font-weight:600}",
           ".PASS{color:var(--ok)}.FAIL{color:var(--bad)}.NJ{color:var(--nj)}",
           ".none{width:320px;height:120px;display:flex;align-items:center;justify-content:center;color:var(--mut);border:1px dashed var(--line)}",
           ".score{color:var(--mut);margin:4px 0 0}",
           "</style></head><body>",
           "<h1>Whoosh Mac screens</h1>",
           '<p class="sub">Each screen in light and dark at its default and its minimum window size. Pictures are shown at their real size (one picture pixel per device pixel on a 2x display), never enlarged. Score each screen out of 10.</p>',
           '<p class="sub">Generated %s. %d screen states.</p>' % (e(time.strftime("%Y-%m-%d %H:%M")), sum(len(v) for v in rows.values()))]
    for screen, states in rows.items():
        out.append('<section><h2>%s</h2><p class="sub">%s</p><div class="row">' % (e(screen), e(sizes_used.get(screen, ""))))
        for st in states:
            cls = {"PASS": "PASS", "FAIL": "FAIL", "NOT JUDGED": "NJ"}[st["verdict"]]
            out.append('<div class="tile">')
            if st["pic"]:
                wpx = (st["dims"][0]) if st["dims"] else None
                full_w = round(wpx) if wpx else ""
                out.append('<img src="%s" alt="%s %s %s" %s>' % (e(st["pic"]), e(screen), e(st["theme"]), e(st["size"]),
                           ('width="%d"' % full_w) if full_w else ""))
            else:
                out.append('<div class="none">no picture</div>')
            if st.get("closeup"):
                out.append('<img src="%s" alt="close-up" style="margin-top:8px">' % e(st["closeup"]))
            cl = st["clipped"]
            if st["clip_note"] and not st["clipped"] and st["verdict"] == "NOT JUDGED":
                clip = "Clipped labels: not judged. " + st["clip_note"]
            else:
                clip = "Clipped labels: %d%s" % (len(cl), (" (" + "; ".join(cl) + ")") if cl else "") + ((" - " + st["clip_note"]) if st["clip_note"] else "")
            dims = ("%.0f x %.0f pt" % st["dims"]) if st["dims"] else "size unknown"
            if st.get("how") is not None:   # a named state: say how it was reached and what the rows answered, not the clipped-label line
                clip = "Reached: " + ("; ".join(st["how"]) or "every step answered and every row read said so")
            out.append('<div class="cap"><b class="%s">%s</b> &middot; %s &middot; %s &middot; asked %d x %d, picture %s<br>%s<br><span class="sub">%s</span></div></div>'
                       % (cls, e(st["verdict"]), e(st["theme"]), e(st["size"]), st["req"][0], st["req"][1], e(dims), e(clip),
                          e(" | ".join(w for v, w in st["why"] if v != "PASS"))))
        out.append('</div><p class="score">Owner score: ____ / 10</p></section>')
    if notes:
        out.append("<h2>Notes</h2>" + "".join("<p class=\"sub\">%s</p>" % e(n) for n in notes))
    out.append("</body></html>")
    with open(os.path.join(outdir, "index.html"), "w", encoding="utf-8") as f:
        f.write("\n".join(out))


def run(c, outdir, only=None, deadline_s=540.0, say=print):
    """Drive c over every screen; returns (lines, rows). `say` gets each result line."""
    os.makedirs(outdir, exist_ok=True)
    t0 = time.time()
    rows, lines, sizes_used = {}, [], {}
    for screen, fname in SCREENS:
        if only and screen not in only:
            continue
        sizes = sizes_for(screen, fname)
        sizes_used[screen] = "Sizes: " + "; ".join("%s %dx%d (%s)" % (lab, w, h, fname) for lab, w, h in sizes) + ("; one size only (default equals minimum)" if len(sizes) == 1 else "")
        say("sizes %s: %s" % (screen, sizes_used[screen][7:]))
        if time.time() - t0 > deadline_s:
            sts = [new_state(screen, t, lab, w, h) for lab, w, h in sizes for t in THEMES]
            for st in sts:
                judge(st, "NOT JUDGED", "deadline of %ds reached before this screen" % deadline_s)
        else:
            try:
                sts = run_screen(c, screen, fname, outdir, None)
            except CouldNotDrive as e:
                sts = [new_state(screen, t, lab, w, h) for lab, w, h in sizes for t in THEMES]
                for st in sts:
                    judge(st, "NOT JUDGED", "no socket: %s" % e)
        for st in sts:
            if st["verdict"] is None:
                judge(st, "NOT JUDGED", "no verdict reached")
            lines.append(line_for(st))
            say(lines[-1])
        rows[screen] = sts
    write_page(outdir, rows, sizes_used, [
        "Clipped labels come from `labels window:<id>`: every leaf text marked .whooshAx in the pictured window (each line names labels judged k of N marked views), clipped when its need exceeds its box by more than %.1f pt (docs/lanes/labels-verb.md says what it does not cover). mac-main's status line is read from `rows mac-main.sync.line` (its row `mac-main.status.link`) beside it. An app without the verb leaves clipped labels not judged." % CLIP_TOL_PT,
        "A hidden-rig picture draws layout, text and placement, not the glass material (CONTROL-SOCKET delta 50)."])
    return lines, rows


# ---------------------------------------------------------------- named states (lane gap-shots)
# A state = (id, item, what it is, the surface `shot` opens, the steps that reach it, its views, its sizes). Per size and theme the tool sends
# fixture reset, shot <screen>, resize, theme, then the reach steps, then each view's steps and one picture of it. Step kinds:
#   ("verb", name, args...)  any socket verb       ("click", ax)  ("key", chord)  ("type", text)
#   ("wait", region, field, want[, secs])   read `rows <region>` until row 0 (field "i:name" = row i, "#len" = how many) equals want, or ("ge", n)
#   ("wait_any", region, field, want) / ("wait_none", region, field, want)   some row / no row has field = want
#   ("progress", fraction)   fixture import-progress <run of rows ...import.progress> <fraction of its total>
#   ("scroll", top|end|page-down|page-up, at)   the `scroll` verb on the window; the reply must say `at` unless nothing overflows
#   ("pick", {field: value}, [queries])   launch each query in turn and move to the first result row holding the fields
#   ("to_more",)   Down until the closed line "All other mail" is selected      ("settle", seconds)  a bounded wait for layout
POLL_S = 6.0
POLL_EVERY = 0.15
SETTLE_S = 0.4
SETTLE_CAP = None       # the self-test caps every settle wait here; a real run never sets it
WAIT_CAP = None         # hid-ai round 21: the self-test caps a step's own longer wait here (the decision model's 30 s waits made its broken copy take 240 s)
REPL = "mac-settings-replies"
WA = "mac-welcome-accounts"
WI = WA + ".import"
SB, SR, SD, SM = "mac-search.bar", "mac-search.results", "mac-search.dropdown", "mac-search.more"
SH, SREF, SNOTE, SSEC = "mac-search.header", "mac-search.refused", "mac-search.index-note", "mac-search.sections"
S1_SENDER = "orders@linea-supply-example.com"   # fixtures/tests-j2.json: the sender of record 1 and of its reply, both holding "quote"
INS = "mac-labels.inspector"
LABEL_BOARD = "needs-reply"      # the built-in label the board draws (par-labelscontactsmac.py BOARD)
LONGER_WORD = "print"            # letters only inside a longer word in the corpus (Printwerk): par-searchmac.py LONGER_WORD
PANES = ["auto", "on-demand", "sources", "retrieval", "voice", "prompt"]


class StepFail(Exception):
    pass


class FitFail(Exception):
    """hid-ai round 24: the state was reached and what it shows is cut (a tile's name, an editor's text): the picture's line is a FAIL, not a NOT JUDGED."""


def _j(*parts):
    with open(os.path.join(ROOT, *parts), encoding="utf-8") as f:
        return json.load(f)


def _fixture_names():
    acc = _j("docs", "design", "fixtures", "mac-main-sample.json")["accounts"]
    bad = next(a for a in acc if a.get("status"))
    ok = [a for a in acc if not a.get("status")][:4]
    return [a["address"] for a in ok] + [bad["address"]], bad["address"]


def _search_inputs():
    corpus = _j("fixtures", "tests-j2.json")
    linea = sorted({m["from"] for m in corpus["messages"] if m["from_name"] == "Linea Supply"})
    domain = linea[0].split("@")[1]
    return os.path.join(ROOT, "fixtures", "tests-j2.json"), domain, corpus["expect"]["meaning_query"]


WELCOME_ADDRESSES, WELCOME_BAD = _fixture_names()
CORPUS_FILE, SEARCH_DOMAIN, MEANING_QUERY = _search_inputs()
# round 20 (main111): the four first queries give no related mail found by its attachment; the last two are about what the one read file of the corpus
# shows (warehouse-photo.jpg: pallets, a delivery note, a forklift) and hold none of msg-s02's letters, so the photo is the piece that relates it
ATTACH_QUERIES = ["unit price", "estimator", "pallets", MEANING_QUERY, "loading dock cartons", "shipping crates"]


def typed(text):
    return [("click", SB), ("key", "⌘A"), ("type", text)]


def launched(text):
    return typed(text) + [("key", "Enter"), ("wait", SR, "#len", ("ge", 1))]


from checklib import related_file as RELATED_FILE   # noqa: E402  the corpus where a related mail found by its attachment is certain (round 23)
RELATED_FILE_DIR = os.path.join(ROOT, "_captures", ".tmp", "searchmac")   # git ignores _captures/.tmp; the J2 corpus under fixtures/ stays as it is
SEARCH_REACH = [("verb", "fixture", "corpus", CORPUS_FILE), ("key", "/"), ("wait", SB, "active", True)]
# item 3bb3d817 (lane every-word): a match holds EVERY typed word, and no mail holds the four words of MEANING_QUERY, so its answer has no match part once a
# model answers; the three parts are reached with the words query of checks/parity/par-searchmac.py: msg-01 and msg-s05 hold all three words (the matches),
# msg-s06 is related, the rest is behind the closed line (_captures/every-word/match-sets-8.out)
WORDS_QUERY = "quote price run"
THREE_PARTS = launched(WORDS_QUERY) + [("wait", "mac-search.header", "related", ("ge", 1)), ("wait", SM, "#len", ("ge", 1))]
SIZES_MAIN = [("1280x820", 1280, 820), ("960x632", 960, 632)]
SIZES_FIRST = [("1280x820", 1280, 820), ("960x600", 960, 600)]
SIZES_SET = [("1280x820", 1280, 820), ("700x480", 700, 480)]
SIZES_SET3 = [("1280x820", 1280, 820), ("940x680", 940, 680), ("700x480", 700, 480)]

# each: id, item, name, screen, reach, views [(suffix, steps, crop)], sizes, design/compare text
STATES = [
    {"id": "replies-panes", "item": "d58d8ad5", "screen": "mac-settings-replies", "sizes": SIZES_SET, "reach": [],
     "what": "Settings > Replies: the six panes and the filter page, to compare with the ai-replies-r7 board",
     # hid-ai round 24 (main114 d58d8ad5 p8): each pane's own fit, judged on the picture's line: the folder tiles' names and paths are never cut (the `labels`
     # verb, strict: no ellipsis allowed), and the text of the prompt and of my voice is never cut inside its card (the editors row's text_cut)
     "views": [(p, [("click", "%s.page.%s" % (REPL, p)), ("wait", REPL + ".place", "page", p), ("settle", SETTLE_S)]
                + {"prompt": [("fit_editor", "prompt")], "voice": [("fit_editor", "voiceText")], "sources": [("fit_labels", REPL + ".sources.")]}.get(p, []), None)
               for p in PANES]
              + [("filter-page", [("click", REPL + ".page.filter"), ("wait", REPL + ".place", "page", "filter"), ("settle", SETTLE_S)], None)]},
    {"id": "replies-filter-page-scroll", "item": "b1d38560", "screen": "mac-settings-replies", "sizes": SIZES_SET3,
     "reach": [("click", REPL + ".page.filter"), ("wait", REPL + ".place", "page", "filter"), ("settle", SETTLE_S)],
     "what": "Settings > Replies > Auto draft > Which messages: the top of the page, one page down (a card passing under the toolbar: the scroll edge), then its end",
     # hid-ai round 21: the middle view shows the column's edge under the toolbar while rows pass under it (SettingsScrollEdge); top shows the cards' left edge
     # against the toolbar's title (1280x820), end shows the footer on one line (700x480)
     "views": [("top", [("scroll", "top", "top"), ("settle", SETTLE_S)], None),
               ("middle", [("scroll", "top", "top"), ("scroll", "page-down", ("middle", "end")), ("settle", SETTLE_S)], None),
               ("end", [("scroll", "end", "end"), ("settle", SETTLE_S)], None)]},
    {"id": "welcome-refused-row", "item": "e47f3f32", "screen": WA, "sizes": SIZES_FIRST,
     "reach": [("click", WA + ".reset"), ("type", " ".join(WELCOME_ADDRESSES)), ("click", WA + ".paste-list"), ("wait", WA + ".accounts", "#len", ("ge", len(WELCOME_ADDRESSES))),
               ("click", WA + ".connect-waiting"), ("wait_none", WA + ".accounts", "phase", "waiting"), ("wait_any", WA + ".accounts", "phase", "refused"),
               ("click", "mac-add-account.address"), ("scroll", "end", "end"), ("settle", SETTLE_S)],   # the Email field is clicked so both themes are pictured with the same focus (main92: the ring showed in dark only, so the two pictures were not taken in one focus state); the refused row is the last of the list: at the minimum size it sits below the fold until the list is scrolled to its end
     "what": "the Welcome list after Connect: %d pasted addresses, one refused (%s) with its reason; the list is scrolled to its end, the close-up crops that row, and a row that is not inside the window is a FAIL" % (len(WELCOME_ADDRESSES), WELCOME_BAD),
     "views": [("window", [], None), ("closeup", [], ("row", WA + ".accounts", "phase", "refused"))]},
    {"id": "thread-saved-draft", "item": "44036170", "screen": "mac-main", "sizes": SIZES_FIRST,
     "reach": [("key", "R"), ("type", "Draft words.\nSecond line."), ("key", "Escape"), ("wait", "mac-main.thread.draft", "#len", ("ge", 1)), ("settle", SETTLE_S)],
     "what": "the open thread with a saved draft: the draft card over the closed reply field, under the received message",
     "views": [("draft-card", [], None)]},
    {"id": "welcome-import-step", "item": "7df97540", "screen": WA, "sizes": SIZES_FIRST,
     "reach": [("click", WA + ".reset"), ("click", WI + ".reset"), ("type", "fine@outlook.com"),   # WA.reset first: the row of the first picture is still there for the next (main95: Continue adds nothing for an address already listed, "did nothing" in 6 of 8 pictures)
                ("click", WA + ".continue"), ("click", WA + ".connect-waiting"),
               ("wait_any", WA + ".accounts", "phase", "healthy"), ("verb", "fixture", "import-sources", "apple-mail=18423"), ("click", WI + ".scan"),
               ("wait", WI + ".panel", "showing", True), ("settle", SETTLE_S)],
     "what": "the first-start import step: the sources the scan found with their count, then a run half done",
     "views": [("sources", [("click", WI + ".select.apple-mail"), ("wait", WI + ".sources", "selected", True), ("settle", SETTLE_S)], None),
               ("progress-half", [("click", WI + ".start"), ("wait", WI + ".progress", "state", "running"), ("progress", 0.5),
                                  ("wait", WI + ".progress", "done", ("ge", 1)), ("settle", SETTLE_S)], None)]},
    {"id": "settings-import-offer", "item": "7df97540", "screen": "mac-settings-accounts", "sizes": SIZES_SET,
     "reach": [("verb", "fixture", "import-sources", "apple-mail=18423")],
     "what": "Settings > Accounts with the Import mail... sheet open (the offer after the first start)",
     "views": [("sheet", [("click", "mac-settings-accounts.import"), ("wait", "mac-settings-accounts.import.sheet", "open", True), ("settle", SETTLE_S)], None)]},
    {"id": "search-typing-domain", "item": "d1ffe36c", "screen": "mac-main", "sizes": SIZES_MAIN, "reach": SEARCH_REACH + typed(SEARCH_DOMAIN[:4]) + [("wait", SD, "1:title", SEARCH_DOMAIN), ("settle", SETTLE_S)],
     "what": "S1: typing the first letters of a sender domain: the dropdown offers `From domain %s` under `Search all mail`; then Down and Tab make the pill" % SEARCH_DOMAIN,
     "views": [("dropdown", [], None), ("pill", [("key", "Down"), ("key", "Tab"), ("wait", "mac-search.chips", "#len", ("ge", 1)), ("settle", SETTLE_S)], None)]},
    {"id": "search-after-enter", "item": "d1ffe36c", "screen": "mac-main", "sizes": SIZES_MAIN, "reach": SEARCH_REACH + THREE_PARTS + [("settle", SETTLE_S)],
     "what": "S1 after Enter: matches, then the Related part, then the closed line `All other mail`", "views": [("results", [], None)]},
    {"id": "search-closed-line-selected", "item": "d1ffe36c", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": SEARCH_REACH + THREE_PARTS + [("to_more",), ("wait", SM, "selected", True), ("settle", SETTLE_S)],
     "what": "S1: Down reaches the closed line `All other mail`; the bottom bar and the line show Enter", "views": [("selected", [], None)]},
    {"id": "search-closed-line-opened", "item": "d1ffe36c", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": SEARCH_REACH + THREE_PARTS + [("to_more",), ("wait", SM, "selected", True), ("key", "Enter"), ("wait", SM, "open", True), ("settle", SETTLE_S)],
     "what": "S1: Enter on the closed line opens the rest of the mail under it", "views": [("opened", [], None)]},
    {"id": "search-related-from-attachment", "item": "d1ffe36c", "screen": "mac-main", "sizes": SIZES_MAIN,
     # round 23: the corpus with a mail related by its file only (checks/checklib/related_file.py) and that file's queries; a miss prints the mail's closeness and the cut-off
     "reach": [("corpus_related_file",), ("key", "/"), ("wait", SB, "active", True),
               ("pick", {"part": "related", "source": "attachment"}, RELATED_FILE.QUERIES, RELATED_FILE.MAIL_ID), ("settle", SETTLE_S)],
     "what": "S1: a related mail that comes from an attachment is selected; the reader opens at its passage", "views": [("selected", [], None)]},
    {"id": "search-nothing-matches", "item": "d1ffe36c", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": SEARCH_REACH + launched("from:@%s zzqxvk" % SEARCH_DOMAIN)[:-1] + [("wait", SM, "selected", True), ("settle", SETTLE_S)],
     "what": "S1: a domain pill and words nothing holds: `No match`, and the closed line still opens every other mail", "views": [("none", [], None)]},
]
# lane fix-rest-app (docs/lanes/fix-rest-app.md): F6 of docs/lanes/gap-search-cases.md (item 98a2ddec) and L1 of docs/lanes/gap-decisions.md (item 7f996166).
# The owner's own query "consultant" exists only in the owner's private mail, so the fixture's forms of it are used: the letters of a sender's name, and
# the letters inside a longer word (LONGER_WORD, the J2.2 case). Each state waits on a row the app serves, so a state that was not reached is NOT JUDGED naming it.
STATES += [
    # round 21 (98a2ddec proof 5, main111 judge: "No picture shows the search results"): the page S1's "After Enter" and its small window: a sender pill
    # typed in its colon form (SPEC A142) and a word, Return pressed, the results on screen at 1280x820 and 960x632, light and dark
    {"id": "search-s1-after-enter", "item": "98a2ddec", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": SEARCH_REACH + launched("from:%s quote" % S1_SENDER) + [("wait", SH, "matches", ("ge", 1)), ("settle", SETTLE_S)],
     "what": "S1 After Enter: the pill From %s and the word `quote`, Return pressed: the count line, the matches with the word in bold, the line before the other mails" % S1_SENDER,
     "views": [("results", [], None)]},
    {"id": "search-bold-letters", "item": "98a2ddec", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": SEARCH_REACH + launched(SEARCH_DOMAIN[:5]) + [("wait", SH, "matches", ("ge", 1)), ("settle", SETTLE_S)],
     "what": "S1: the match group of `%s` (a sender's name): the typed letters in bold in the row, the count line over the rows" % SEARCH_DOMAIN[:5], "views": [("results", [], None)]},
    {"id": "search-letters-inside-a-longer-word", "item": "98a2ddec", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": SEARCH_REACH + launched(LONGER_WORD) + [("wait", SH, "matches", ("ge", 1)), ("settle", SETTLE_S)],
     "what": "S1: `%s` finds the mails that hold it only inside a longer word (Printwerk): they are in the match group and each row says where the letters are" % LONGER_WORD,
     "views": [("results", [], None)]},
    {"id": "search-count-line-and-related-group", "item": "98a2ddec", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": SEARCH_REACH + THREE_PARTS + [("wait", SH, "matches", ("ge", 1)), ("wait", SSEC, "#len", ("ge", 1)), ("settle", SETTLE_S)],
     "what": "S1: the count line (`N matches · M close in meaning`) over the match group, then the line that starts the Related group",
     "views": [("count-and-related", [], None)]},
    {"id": "search-could-not-run", "item": "98a2ddec", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": SEARCH_REACH + [("verb", "fixture", "search-refuse", "The index is closed for this test.")] + typed(MEANING_QUERY)
              + [("key", "Enter"), ("wait", SREF, "#len", ("ge", 1)), ("settle", SETTLE_S)],
     "what": "S1: the engine refuses the search: the one sentence `The search could not run.` and the engine's own words under it, no rows", "views": [("refused", [], None)]},
    {"id": "search-index-line", "item": "98a2ddec", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": SEARCH_REACH + [("verb", "fixture", "index-progress", "40", "60")] + launched(MEANING_QUERY) + [("wait", SNOTE, "#len", ("ge", 1)), ("settle", SETTLE_S)],
     "what": "S1 while the meaning index is built (40 of 100 mails): the one line above the rows says how far it is", "views": [("index-line", [], None)]},
    # item 4d1e8f9c proof 4: the AI card of the label inspector in each state the test engine serves, and the Re-run preview line with the foot (pane 3 of the MAIN window).
    {"id": "label-ai-no-model-preview", "item": "4d1e8f9c", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": [("type", "Shots AI no model"), ("click", INS + ".open.new"), ("click", INS + ".name"), ("type", "supplier quotes"), ("click", INS + ".meaning"),
               ("click", INS + ".rule.0.kind.domain"), ("type", "nothing-matches-this.invalid"), ("click", INS + ".rule.0.value"), ("click", INS + ".rule.0.commit"),
               ("click", INS + ".create"), ("settle", 1.0), ("type", "supplier quotes and invoices"), ("click", INS + ".meaning"), ("click", INS + ".apply"),
               ("wait", INS, "preview_line", "0 picked · 0 by the AI"), ("settle", 1.5)],   # the head line must read its words: the old "the AI rules on each after Save" is NOT JUDGED here, naming the line
     "what": "a saved label with a description and a rule that matches nothing, no model: after Apply the Re-run line counts 0 joins and says `The AI cannot sort yet: no model on this Mac.`, the foot shows the same 0 (the test engine's reason when no decision model is set and the install is the sample)",
     "views": [("preview-and-foot", [], None)]},
    {"id": "label-ai-new-install-no-model", "item": "4d1e8f9c", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": [("verb", "fixture", "install-state", "new"), ("click", "%s.open.modify.%s" % (INS, LABEL_BOARD)), ("wait", INS, "open", True), ("wait", INS, "mode", "modify"), ("settle", SETTLE_S)],
     "what": "ai_no_model: a new installation with no decision model; the AI card says `No model is set for sorting mail: choose one in Settings, Decisions.` and its door is Open Decisions",
     "views": [("ai-card", [], None)]},
    {"id": "label-ai-needs-apple", "item": "4d1e8f9c", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": [("click", "%s.open.modify.updates" % INS), ("wait", INS, "open", True), ("wait", INS, "mode", "modify"), ("settle", SETTLE_S)],
     "what": "ai_needs_apple (the test engine serves it for the built-in Updates): the AI card says `Sorting mail into labels needs Apple Intelligence: turn it on in System Settings, then Apple Intelligence and Siri.` with the door Open System Settings",
     "views": [("ai-card", [], None)]},
    {"id": "label-ai-under-trust", "item": "4d1e8f9c", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": [("click", "%s.open.modify.social" % INS), ("wait", INS, "open", True), ("wait", INS, "mode", "modify"), ("settle", SETTLE_S)],
     "what": "ai_under_trust (the test engine serves it for the built-in Social): the AI card says `The AI judged 42 mails and was sure of none: its best was 58 %, and it files from 80 %.` with the door Open Decisions. ai_failed is not served by the test engine (only the real engine's ai_failed words exist, docs/lanes/fix-aisort.md): no state",
     "views": [("ai-card", [], None)]},
    # item 74ce9fdc proof 4: the designed (newsletter) sample and the plain sample, in the reader of the main window.
    {"id": "reader-designed-and-plain-sample", "item": "74ce9fdc", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": [("click", "mac-main.list.row.0"), ("click", "mac-main.reader.open"), ("settle", SETTLE_S)],
     "what": "the reader with the designed (newsletter) sample selected (its own backgrounds, table and colours kept, the pictures line counting the blocked ones) and with the plain sample",
     "views": [("designed", [("click", "mac-main.reader.body.sample.designed"), ("wait", "mac-main.reader.body", "layout", "designed"), ("settle", SETTLE_S)], None),
               ("plain", [("click", "mac-main.reader.body.sample.plain"), ("wait", "mac-main.reader.body", "format", "plain"), ("settle", SETTLE_S)], None)]},
    {"id": "label-inspector-open", "item": "7f996166", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": [("click", "%s.open.modify.%s" % (INS, LABEL_BOARD)), ("wait", INS, "open", True), ("wait", INS, "mode", "modify"), ("settle", SETTLE_S)],
     "what": "the label inspector on the built-in label `%s` (pane 3 of the MAIN window: the reader swaps to LabelInspectorView, so the shot is of mac-main, not mac-labels-contacts)" % LABEL_BOARD,
     "views": [("inspector", [], None)]},
    # item 5aa79f87 proof 3: step 7 of 10 of the first start (AI), the three lines with their buttons, Skip and Continue. LAST in the list: the flow stays open after it
    # (the next combo's `fixture reset` and this reach's own `first-start off` close it).
    {"id": "welcome-ai-step", "item": "5aa79f87", "screen": "mac-main", "sizes": SIZES_FIRST,
     "reach": [("verb", "fixture", "first-start", "off"), ("verb", "fixture", "first-start", "reduce", "off"), ("verb", "fixture", "first-start", "on"), ("verb", "fixture", "providers", "none"),
               ("click", "mac-first-start.continue"), ("click", "mac-first-start.continue"), ("click", "mac-first-start.continue"), ("click", "mac-first-start.continue"),
               ("click", "mac-first-start.continue"), ("click", "mac-first-start.continue"), ("wait", "mac-first-start.card", "step", 7),
               ("click", "ai-setup.preview.decision.missing"), ("click", "ai-setup.preview.writing.missing"), ("click", "ai-setup.preview.meaning.missing"), ("settle", 1.0)],
     "what": "the first start, step 7 of 10 (AI): one card, three lines (meaning, sorting, writing) each with its button, Skip (Esc) and Continue (Return); every need staged missing",
     "views": [("step-7", [], None), ("card-ink", [], ("cardink", "mac-first-start.card"))]},
]
# item 0b095f58 (the picked message row, docs/design/boards/message-card-r1: states 3, 1 and 5): the list row of a thread with its count pill, the single message
# with none, and the thread whose last message is the owner's with the reply arrow. Each state clicks the row by what the app serves for it (`mac-main.list` rows:
# thread_count, replied_last), so the focused row is the one the tile is about; the row is marked by `.accessibilityIdentifier`, not `.whooshAx`, so there is no
# close-up (the tile is the window). A fixture with no such row is NOT JUDGED naming the field.
LIST_ROWS = "mac-main.list"
LIST_STATE = "mac-main.list.state"     # ListProbe.listState: strip_settled, compact, selected (the count), why_tags
STATES += [
    {"id": "list-row-thread-pill", "item": "0b095f58", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": [("wait_any", LIST_ROWS, "thread_count", 3), ("click_row", LIST_ROWS, "thread_count", 3), ("settle", SETTLE_S)],
     "what": "board message-card-r1 state 3/4: a thread of 3 messages: the count pill after the sender, the unread dot", "views": [("thread-row", [], None)]},
    {"id": "list-row-single-message", "item": "0b095f58", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": [("wait_any", LIST_ROWS, "thread_count", 1), ("click_row", LIST_ROWS, "thread_count", 1), ("settle", SETTLE_S)],
     "what": "board message-card-r1 state 1/2: a single message: no pill", "views": [("single-row", [], None)]},
    {"id": "list-row-reply-arrow", "item": "0b095f58", "screen": "mac-main", "sizes": SIZES_MAIN,
     # the fixture holds no thread whose newest message is the owner's, so the state makes one the way the owner does: Enter on the thread of 3 opens it with the reply field
     # (A102), a line, ⌘Enter sends it, the clock passes the undo window so the reply joins the thread (the test engine's thread count then includes it: FixtureCore.sentCountsLocked,
     # so the list reads the thread again), Escape twice back to the list. main104: with no such step `replied_last` was False on all 24 rows.
     "reach": [("click_row", LIST_ROWS, "thread_count", 3), ("key", "Enter"), ("wait", "mac-main.quickreply.field", "#len", ("ge", 1)), ("type", "Thanks."), ("key", "⌘Enter"),
               ("wait", "mac-main.undo.bar", "#len", ("ge", 1)), ("verb", "clock", "advance", "11"), ("key", "Escape"), ("key", "Escape"),
               ("wait_any", LIST_ROWS, "replied_last", True), ("click_row", LIST_ROWS, "replied_last", True), ("settle", SETTLE_S)],
     "what": "board message-card-r1 state 5: a thread whose last message is the owner's: the small reply arrow before the subject, with the pill", "views": [("reply-row", [], None)]},
]
# item 0b095f58, the other states of the board (docs/design/boards/message-card-r1/index.html lines 110 to 127). Each names its row by what the app serves for it in `rows
# mac-main.list` (ListProbe.rows: thread_count, unread, pinned, starred, selected, focused, draft, attachments, why_tag) and ends on a `wait_any` of the row in that very state,
# never on a sleep alone. A row the picture is about is NOT focused unless the board's state is the focused one (the focus fill turns the pill white), so a state that acts on
# a row (U, S, X, a reply) clicks it, acts, and moves the focus off it with J or K. States 7 (hovered), 10 (snoozed), 13 (muted) and 14 (VIP) follow below (lane
# row-marks, docs/lanes/row-marks.md). `fixture reset` does not clear the list's density (InboxListState) nor the label inspector, so the states that change them put
# them back in a last view.
R_SINGLE_UNREAD = {"thread_count": 1, "unread": True, "attachments": 0, "pinned": False}       # msg-09 in the sample
R_SINGLE_READ = {"thread_count": 1, "unread": False, "attachments": 0, "pinned": False}        # msg-06
R_THREAD3_UNREAD = {"thread_count": 3, "unread": True, "pinned": False}                        # msg-16 (msg-01, the other thread of 3, is pinned)
R_THREAD3 = {"thread_count": 3, "pinned": False}                                               # msg-16 read or unread: opening a thread marks it read
OFF = {"focused": False}
# lane row-marks: the reaches of list-row-thread-draft and list-row-why-tag before the repair, kept for the self-test's broken copies (main193 shots-rows.log:49-56).
# The draft reach waited on an UNREAD thread of 3 with a draft, but Enter opens the thread and the engine marks the opened mail read (FixtureCore.swift
# `case .openThread`: `read.insert(m.id)`), so the wait was never met. The why reach pressed Command I, a toggle (LabelInspectorModel.toggle), on an inspector the
# light pictures had left open (`fixture reset` does not close it), so every dark picture found it closed.
OLD_DRAFT_REACH = [("click_row", LIST_ROWS, R_THREAD3_UNREAD, None), ("key", "Enter"), ("wait", "mac-main.quickreply.field", "#len", ("ge", 1)), ("type", "Two points before I answer."),
                   ("key", "Escape"), ("key", "Escape"), ("key", "K"), ("wait_any", LIST_ROWS, dict(R_THREAD3_UNREAD, draft=True, **OFF), None), ("settle", SETTLE_S)]
OLD_WHY_REACH = [("key", "\u2318I"), ("wait", INS, "open", True), ("click", INS + ".switch.mailbox"), ("click", INS + ".why"),
                 ("wait_any", LIST_ROWS, dict({"why_tag": ("set",)}, **OFF), None), ("settle", SETTLE_S)]
OLD_WHY_VIEWS = [("why-tag-row", [], None)]
STATES += [
    {"id": "list-row-single-read", "item": "0b095f58", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": [("wait_any", LIST_ROWS, dict(R_SINGLE_READ, **OFF), None), ("settle", SETTLE_S)],
     "what": "board message-card-r1 state 2 `Single, read`: a read single message off the focus: no pill, no unread dot", "views": [("single-read-row", [], None)]},
    {"id": "list-row-thread-read", "item": "0b095f58", "screen": "mac-main", "sizes": SIZES_MAIN,
     # the sample holds no read thread of 3: the owner reads the unread one with U, then moves off it (K), so the pill is grey and not on the focus fill
     "reach": [("click_row", LIST_ROWS, R_THREAD3_UNREAD, None), ("key", "U"), ("key", "K"), ("wait_any", LIST_ROWS, dict(R_THREAD3_UNREAD, unread=False, **OFF), None), ("settle", SETTLE_S)],
     "what": "board message-card-r1 state 4 `Thread of 3, read`: a thread of 3 marked read with U and off the focus: the grey count pill, no unread dot", "views": [("thread-read-row", [], None)]},
    {"id": "list-row-thread-focused", "item": "0b095f58", "screen": "mac-main", "sizes": SIZES_MAIN,
     # focused by the keyboard: the click lands on the row, K and J leave and come back, then the strip waits for the focus to settle (strip_settled)
     "reach": [("click_row", LIST_ROWS, R_THREAD3_UNREAD, None), ("key", "K"), ("key", "J"), ("wait", LIST_STATE, "strip_settled", True),
               ("wait_any", LIST_ROWS, dict(R_THREAD3_UNREAD, focused=True), None), ("settle", SETTLE_S)],
     "what": "board message-card-r1 state 6 `Focused (keyboard)`: an unread thread of 3 focused with K then J: the focus fill, the white pill and the action strip once the focus settles",
     "views": [("focused-row", [], None)]},
    {"id": "list-row-selected", "item": "0b095f58", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": [("click_row", LIST_ROWS, R_SINGLE_UNREAD, None), ("key", "X"), ("key", "K"), ("wait", LIST_STATE, "selected", ("ge", 1)),
               ("wait_any", LIST_ROWS, dict(R_SINGLE_UNREAD, selected=True, **OFF), None), ("settle", SETTLE_S)],
     "what": "board message-card-r1 state 8 `Selected (X)`: an unread single message selected with X and off the focus: the check mark and the selection bar",
     "views": [("selected-row", [], None), ("selection-cleared", [("key", "Escape"), ("wait", LIST_STATE, "selected", 0), ("settle", SETTLE_S)], None)]},
    {"id": "list-row-pinned", "item": "0b095f58", "screen": "mac-main", "sizes": SIZES_MAIN,
     # pinned is the sample's own (msg-01 is pinned and unread); the focus is first moved to a read single message so the pinned row is not on the focus fill
     "reach": [("click_row", LIST_ROWS, R_SINGLE_READ, None), ("wait_any", LIST_ROWS, {"pinned": True, "unread": True, "focused": False}, None), ("settle", SETTLE_S)],
     "what": "board message-card-r1 state 9 `Pinned`: an unread pinned message off the focus: the pin mark", "views": [("pinned-row", [], None)]},
    {"id": "list-row-starred", "item": "0b095f58", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": [("click_row", LIST_ROWS, R_SINGLE_UNREAD, None), ("key", "S"), ("key", "K"), ("wait_any", LIST_ROWS, dict(R_SINGLE_UNREAD, starred=True, **OFF), None), ("settle", SETTLE_S)],
     "what": "board message-card-r1 state 11 `Starred`: an unread single message starred with S and off the focus: the star", "views": [("starred-row", [], None)]},
    {"id": "list-row-attachment", "item": "0b095f58", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": [("wait_any", LIST_ROWS, {"attachments": ("ge", 1), "thread_count": 2, "pinned": False, "focused": False}, None), ("settle", SETTLE_S)],
     "what": "board message-card-r1 state 12 `Attachment`: a thread of 2 with a file, off the focus: the paperclip beside the time", "views": [("attachment-row", [], None)]},
    {"id": "list-row-thread-draft", "item": "0b095f58", "screen": "mac-main", "sizes": SIZES_MAIN,
     # the owner's way to a draft: Enter opens the reply on the thread, a line is typed, Escape twice leaves with the reply kept as a draft (CONTROL-SOCKET 40: a reply survives Esc)
     # lane row-marks: the opened thread is read (the engine marks the mail Enter opens), so the last wait names the thread of 3 without its unread mark (the board's
     # state 15 is a read thread too); main193 waited on unread = true and was never reached
     "reach": [("click_row", LIST_ROWS, R_THREAD3_UNREAD, None), ("key", "Enter"), ("wait", "mac-main.quickreply.field", "#len", ("ge", 1)), ("type", "Two points before I answer."),
               ("key", "Escape"), ("key", "Escape"), ("key", "K"), ("wait_any", LIST_ROWS, dict(R_THREAD3, draft=True, **OFF), None), ("settle", SETTLE_S)],
     "what": "board message-card-r1 state 15 `Thread with a draft`: a thread whose reply was typed and left with Escape (opened, so read), off the focus: the word Draft before the subject",
     "views": [("draft-row", [], None)]},
    {"id": "list-row-why-tag", "item": "0b095f58", "screen": "mac-main", "sizes": SIZES_MAIN,
     # the owner's way: Command I opens the inspector, its list switch, then its `Why here?` switch (par-inboxlist.py J1.52); every row then wears the tag. Command I and the
     # switch are toggles and the inspector outlives `fixture reset`, so each is pressed only when its state is not the wanted one (lane row-marks: main193's dark pictures found
     # the inspector closed by their own Command I), and the last view closes both again so the next picture starts from a closed inspector
     "reach": [("ensure", INS, "open", True, [("key", "\u2318I")]), ("click", INS + ".switch.mailbox"), ("ensure", LIST_STATE, "why_tags.on", True, [("click", INS + ".why")]),
               ("wait_any", LIST_ROWS, dict({"why_tag": ("set",)}, **OFF), None), ("settle", SETTLE_S)],
     "what": "board message-card-r1 state 16 `With a why here tag`: the inspector's Why here? switch on: the grey tag naming the card that placed the row, on rows off the focus; the second picture closes the switch and the inspector",
     "views": [("why-tag-row", [], None),
               ("inspector-closed", [("ensure", LIST_STATE, "why_tags.on", False, [("click", INS + ".why")]), ("ensure", INS, "open", False, [("key", "\u2318I")]),
                                     ("wait_none", LIST_ROWS, {"why_tag": ("set",)}, None), ("settle", SETTLE_S)], None)]},
    {"id": "list-row-compact", "item": "0b095f58", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": [("wait", LIST_STATE, "compact", False), ("click", "mac-main.list.compact"), ("wait", LIST_STATE, "compact", True), ("click_row", LIST_ROWS, R_THREAD3_UNREAD, None),
               ("key", "K"), ("wait_any", LIST_ROWS, dict(R_THREAD3_UNREAD, **OFF), None), ("settle", SETTLE_S)],
     "what": "board message-card-r1 state 17 `Compact density`: the compact switch on: one line per row, the thread of 3 keeps its pill; the second picture puts the rows back",
     "views": [("compact-rows", [], None), ("comfortable-again", [("click", "mac-main.list.compact"), ("wait", LIST_STATE, "compact", False), ("settle", SETTLE_S)], None)]},
]
# lane row-marks (item 0b095f58): board states 7, 10, 13 and 14, reached through the owner's doors and read from the row fields ListProbe.rows serves for them (muted, vip,
# snoozed, hovered, rested). A snoozed, muted or VIP sender's mail leaves the inbox, so each is pictured in its own view (the sidebar's Snoozed, Muted, VIPs); a view of one
# row would put that row on the focus fill, so each marks two mails and puts the focus on the other one before the last wait.
# the second VIP mark: an unread thread of an account that can act. main194 (lead, 15 Nov): the first unread thread of the sample is of the account that needs
# sign-in, the engine refuses every act on its mail (FixtureCore.refuseFailing), and "the key Enter does nothing on mac-main" left the state unreached.
R_VIP_THREAD = {"unread": True, "pinned": False, "vip": False, "thread_count": ("ge", 2), "account_attention": False}
R_VIP_THREAD_OLD = {"unread": True, "pinned": False, "vip": False, "thread_count": ("ge", 2)}
R_THREAD4_READ = {"thread_count": 4, "unread": False, "pinned": False}                          # msg-10 LHG Procurement (the board's state 13 is a read thread of 4)
STATES += [
    {"id": "list-row-hovered", "item": "0b095f58", "screen": "mac-main", "sizes": SIZES_MAIN,
     # the focus on a read single message, the pointer rests on the unread single one (the rig's `fixture pointer-rest <row ax>`, the notch's verb); the second picture takes it off
     "reach": [("click_row", LIST_ROWS, R_SINGLE_READ, None), ("rest_row", LIST_ROWS, dict(R_SINGLE_UNREAD, **OFF)),
               ("wait_any", LIST_ROWS, dict(R_SINGLE_UNREAD, hovered=True, rested=True, **OFF), None), ("settle", SETTLE_S)],
     "what": "board message-card-r1 state 7 `Hovered, action strip`: the pointer rests on an unread single message that is not the focused row: the hover fill and the action strip in the time's place; the second picture takes the pointer off",
     "views": [("hovered-row", [], None), ("pointer-off", [("verb", "fixture", "pointer-rest", "off"), ("wait_none", LIST_ROWS, {"hovered": True}, None), ("settle", SETTLE_S)], None)]},
    {"id": "list-row-snoozed", "item": "0b095f58", "screen": "mac-main", "sizes": SIZES_MAIN,
     # H, "tomorrow morning", Enter on a read single message and on the read thread of 4; the Snoozed view; the focus on the thread
     "reach": [("click_row", LIST_ROWS, R_SINGLE_READ, None), ("key", "H"), ("wait", LIST_STATE, "pending", "snooze"), ("type", "tomorrow morning"), ("key", "Enter"),
               ("wait", LIST_STATE, "pending", None), ("click_row", LIST_ROWS, R_THREAD4_READ, None), ("key", "H"), ("wait", LIST_STATE, "pending", "snooze"),
               ("type", "tomorrow morning"), ("key", "Enter"), ("wait", LIST_STATE, "pending", None),
               ("click", "mac-main.sidebar.mailbox.snoozed"), ("wait", LIST_STATE, "mailbox", "snoozed"), ("wait", LIST_ROWS, "#len", ("ge", 2)),
               ("click_row", LIST_ROWS, {"thread_count": 4}, None), ("wait_any", LIST_ROWS, dict(R_SINGLE_READ, snoozed=("set",), **OFF), None), ("settle", SETTLE_S)],
     "what": "board message-card-r1 state 10 `Snoozed`: a read single message snoozed with H until tomorrow morning, in the Snoozed view off the focus: the clock and `back <time>` in the warning colour where the time was",
     "views": [("snoozed-row", [], None)]},
    {"id": "list-row-muted", "item": "0b095f58", "screen": "mac-main", "sizes": SIZES_MAIN,
     # shift M on the read thread of 4 and on a read single message; the Muted view; the focus on the single one
     "reach": [("click_row", LIST_ROWS, R_THREAD4_READ, None), ("key", "\u21e7M"), ("wait_none", LIST_ROWS, R_THREAD4_READ, None),
               ("click_row", LIST_ROWS, R_SINGLE_READ, None), ("key", "\u21e7M"),
               ("click", "mac-main.sidebar.mailbox.muted"), ("wait", LIST_STATE, "mailbox", "muted"), ("wait", LIST_ROWS, "#len", ("ge", 2)),
               ("click_row", LIST_ROWS, {"thread_count": 1}, None), ("wait_any", LIST_ROWS, dict(R_THREAD4_READ, muted=True, **OFF), None), ("settle", SETTLE_S)],
     "what": "board message-card-r1 state 13 `Muted`: a read thread of 4 muted with shift M, in the Muted view off the focus: the mute mark after the count, sender and subject in the tertiary ink",
     "views": [("muted-row", [], None)]},
    {"id": "list-row-vip", "item": "0b095f58", "screen": "mac-main", "sizes": SIZES_MAIN,
     # the palette's `Mark as VIP` on the unread single message, then on an unread thread of another sender (the VIPs view must hold the first before the second is
     # marked); the VIPs view; the focus on the thread
     "reach": [("click_row", LIST_ROWS, R_SINGLE_UNREAD, None), ("key", "\u2318P"), ("type", "mark as vip"), ("wait", "mac-palette.results", "0:title", "Mark as VIP"), ("key", "Enter"),
               ("click", "mac-main.sidebar.mailbox.vips"), ("wait", LIST_STATE, "mailbox", "vips"), ("wait", LIST_ROWS, "#len", ("ge", 1)),
               ("click", "mac-main.sidebar.command.go-inbox"), ("wait", LIST_STATE, "mailbox", None),
               ("click_row", LIST_ROWS, R_VIP_THREAD, None), ("key", "\u2318P"), ("type", "mark as vip"),
               ("wait", "mac-palette.results", "0:title", "Mark as VIP"), ("key", "Enter"),
               ("click", "mac-main.sidebar.mailbox.vips"), ("wait", LIST_STATE, "mailbox", "vips"), ("wait", LIST_ROWS, "#len", ("ge", 2)),
               ("click_row", LIST_ROWS, {"vip": True, "thread_count": ("ge", 2)}, None), ("wait_any", LIST_ROWS, dict(R_SINGLE_UNREAD, vip=True, **OFF), None), ("settle", SETTLE_S)],
     "what": "board message-card-r1 state 14 `VIP sender`: an unread single message whose sender was marked with the palette's Mark as VIP, in the VIPs view off the focus: the VIP chip after the sender, before the other marks",
     "views": [("vip-row", [], None)]},
]
# item 637ae165 (the Lifted look of the quick reply; board docs/design/boards/thread-cards-r2 cards-s3 and the sendwave Aurora landing): the
# quick reply WHERE THE OWNER MEETS IT, docked under the reader of the main window on the fixture's Linea Supply thread, in the board's language
# (French), not the separate empty "Quick reply" window of `--screens mac-quickreply` (main107 judge: no thread behind, no recipient, an empty
# From, no signature, English). One combo per size and theme walks the board's states in order; each view reads the header's words, the From
# row's address and the signature line into its check line (`read` steps), and waits on the row that says the state is there.
QR_FIELD, QR_HEADER, QR_FROM = "mac-main.quickreply.field", "mac-main.quickreply.header", "mac-main.quickreply.from"
QR_OUT, QR_UNDO = "mac-main.thread.outgoing", "mac-main.undo.bar"
QR_LANG = "fr"                                       # the board's language
READER_SCROLL = "mac-main.reader.scroll"   # the reader's scroll offset (ReaderProbe.swift: offset, max_offset, viewport, content)
STILL_GAP_S = 0.1                          # the gap between the two reads that must agree
# The board's reply, as cards-s3 draws it (shots expanded-1280-light, then sending-960-dark and sent-1280-light): typed in two parts, the
# first before the header opens, the question before the send. The board's closing "Best, Assem" is the signature the app adds.
QR_TEXT = ("Hi Karen,\n\nThanks for the quote. Please go ahead with 12,000 units and the 170 g cover; we will skip the spot varnish. "
           "Week 4 to Düsseldorf works for us.",
           "\n\nCould you send the updated quote for signature?")
QR_ACCOUNT = next(a for a in _j("docs", "design", "fixtures", "mac-main-sample.json")["accounts"] if a["id"] == "acc-04")   # Linea Supply's thread, received at Nordlicht


def _undo_window_s():
    with open(os.path.join(ROOT, "docs", "CONSTANTS.md"), encoding="utf-8") as f:
        for line in f:
            if line.startswith("| UNDO_SEND_DEFAULT_S"):
                return int(line.split("|")[2].split("·")[0])
    raise RuntimeError("docs/CONSTANTS.md has no UNDO_SEND_DEFAULT_S row")


QR_READS = [("read", QR_HEADER, ("title", "open")), ("read", QR_FROM, ("address", "open")), ("read", QR_FIELD, ("signature", "placeholder", "text"))]
STATES += [
    {"id": "quickreply-in-thread", "item": "637ae165", "screen": "mac-main", "sizes": SIZES_MAIN, "lang": QR_LANG,
     "reach": [("click", "mac-settings-general.appearance.language." + QR_LANG), ("settle", SETTLE_S),
               ("click_row", LIST_ROWS, "sender", "Linea Supply"), ("key", "R"), ("wait", QR_FIELD, "#len", ("ge", 1)),
               ("wait", QR_FROM, "address", QR_ACCOUNT["address"]), ("settle", SETTLE_S)],
     "what": "the quick reply docked under the reader of `Quote for the Q1 print run` (Linea Supply), French, as board cards-s3 draws it: idle, the header open, the From picker with its caps, sending (the lifted card), the Aurora landing after the send wave, sent (flat)",
     # round 19 (hid-list's read of the main111 pictures): R scrolls the reader to its end (main79, par-reader `r-scrolls-to-the-new-reply-card`),
     # which hid the folded earlier messages and the summary card the board draws at rest. `after-r` pictures the thread as R really leaves it;
     # `idle` then takes the reader to its top (⌘Up, the reader's jump-top) and pictures it at rest, as the board's Idle. Every view ends on a
     # still reader (two equal reads of its offset), so no picture is taken mid-scroll.
     "views": [("after-r", list(QR_READS) + [("still",)], None),
               # round 20 (main115 judge: idle byte-identical to after-r at 3 of 4 combos): the reader's ⌘Up moved only the offset the app keeps
               # (rows mac-main.reader.scroll read 0 at once) while the drawn thread stayed at its end. The window's scroll view is moved itself
               # (`scroll top`, whose reply is the drawn offset), then the reader holds still.
               ("idle", [("scroll", "top", "top"), ("settle", SETTLE_S), ("still",)] + QR_READS, None),
               # round 18 (main111 judge, clause 1): the board's "Header open" is a reply being written, "Brouillon à Linea Supply" over the typed
               # words with Envoyer enabled; the state typed nothing and showed the empty "Répondre à". The board's words go in first.
               ("header-open", [("type", QR_TEXT[0]), ("settle", SETTLE_S), ("click", QR_HEADER + ".toggle"), ("wait", QR_HEADER, "open", "1"),
                                ("settle", SETTLE_S)] + QR_READS + [("still",)], None),
               ("from-picker", [("click", QR_FROM), ("wait", QR_FROM, "open", "1"), ("settle", SETTLE_S)] + QR_READS + [("still",)], None),
               # the rest of the board's reply ("Sending": the same words and the question after them); the app adds the signature itself
               ("sending", [("key", "Escape"), ("wait", QR_FROM, "open", "0"), ("type", QR_TEXT[1]), ("key", "⌘Enter"),
                            ("wait", QR_UNDO, "#len", ("ge", 1)), ("wait", QR_OUT, "#len", ("ge", 1)), ("settle", SETTLE_S),
                            ("read", QR_OUT, ("state", "material")), ("read", QR_HEADER, ("title", "open")), ("read", QR_FIELD, ("signature",)), ("still",)], None),
               ("landing", [("verb", "clock", "advance", str(_undo_window_s() + 1)), ("wait", QR_OUT, "0:state", "sent"), ("settle", 0.15),
                            # the reply box folds once its words are sent (owner, 13 Nov, picture 206: one object is shown once): the sent card is what is read,
                            # and the box's header answers no row (until 14 Nov these two views read the header's title and were never reached)
                            ("read", QR_OUT, ("state", "material")), ("wait", QR_HEADER, "#len", 0), ("still",)], None),
               ("sent", [("settle", 1.0), ("read", QR_OUT, ("state", "material", "time")), ("wait", QR_HEADER, "#len", 0), ("still",)], None)]},
]
# hid-ai round 18 (item 0aea8b37 proof 4): the decision model's row on the Decisions pane while it downloads (bytes and a bar), ready, and after a failure
# (the sentence and Retry), on the rig's loopback download stand-in: the stub settingsaimac-check's J5.7 cases drive (its Stub("laya"), loaded from that
# file), named to the copy by `fixture provider-endpoint laya <stub>` (AIFixture.swift: LayaOnDevice.useRigBase, rig only, loopback only) at each start.
# No launch variable is needed beyond the hidden test copy's own (WHOOSH_RIG_HIDDEN=1, --fixture-core); WHOOSH_RIG_LAYA_BASE is the launch-time way to the
# same address and is not read here. Each combo empties the copy's own models folder (<run folder>/Models/laya, never the owner's) first.
LAYA_ROW = "mac-settings-ai.decisions.local"
LAYA_READ = ["state", "words", "received", "total", "chip", "retry", "button"]
DECISIONS_REACH = [("click", SETTINGS_W + ".pane-row.decisions"), ("wait", SETTINGS_W + ".place", "pane", "decisions"),
                   ("laya", "fresh"), ("wait", LAYA_ROW, "state", "absent")]
STATES += [
    {"id": "decision-model-download", "item": "0aea8b37", "screen": "mac-settings-ai", "sizes": SIZES_SET, "reach": DECISIONS_REACH,
     "what": "Settings > Decisions: the decision model's row while it downloads (bytes and a bar), when it is ready, and after a failure (the sentence and Retry)",
     "views": [("downloading", [("laya", "slow"), ("click", LAYA_ROW + ".download"), ("wait", LAYA_ROW, "state", "downloading"),
                                ("wait", LAYA_ROW, "received", ("ge", 1)), ("wait", LAYA_ROW, "total", ("ge", 1)), ("read", LAYA_ROW, LAYA_READ)], None),
               ("ready", [("wait", LAYA_ROW, "state", "ready", 30), ("read", LAYA_ROW, LAYA_READ)], None),
               ("failed", [("laya", "fresh"), ("wait", LAYA_ROW, "state", "absent"), ("laya", "missing"), ("click", LAYA_ROW + ".download"),
                           ("wait", LAYA_ROW, "state", "failed", 30), ("read", LAYA_ROW, LAYA_READ)], None)]},
]
# hid-ai round 20 (item 9e4826fb proof 4, main111: the four sheet-mac-settings-ai pictures stop at Search by meaning and at Providers): the AI and agents pane
# scrolled to its end, where the agent cards are (AgentsView is the pane's last group, AIPaneView.swift AIAgentsPaneView); the line names the cards it found.
AGENT_CARDS = "mac-settings-ai.agents.card"
STATES += [
    {"id": "settings-ai-agent-cards", "item": "9e4826fb", "screen": "mac-settings-ai", "sizes": SIZES_SET,
     "reach": [("click", SETTINGS_W + ".pane-row.ai"), ("wait", SETTINGS_W + ".place", "pane", "ai"),
               ("wait_any", AGENT_CARDS, "name", "Ask my inbox"), ("wait_any", AGENT_CARDS, "name", "Writer")],
     "what": "Settings > AI and agents scrolled to its end: the Ask my inbox and Writer cards, each with its provider and model",
     "views": [("cards", [("scroll", "end", "end"), ("settle", SETTLE_S), ("read", AGENT_CARDS, ["name", "provider", "model"])], None)]},
]
# owner 2026-11-13 07:24 ("some emails not super well displayed"): a short written mail with a picture signature, and a designed newsletter beside it, pictures blocked.
# The mails are the fixture core's drive samples (MessageBodyEngineSamples `signature` and `newsletter`): the open thread's body is replaced in the view only.
# Enter on a list row opens the thread with the reply's text holding the keys; since 13 Nov a further Enter there adds a line (plain Enter never
# sends), so a state reached four times (two sizes, two themes) piled empty lines into the reply and drew a tall empty reply box over the mail
# (lead, main117 pictures). Escape puts the empty reply away and leaves the mail open: the pictures are of the mail.
MAIL_OPEN = [("click_row", LIST_ROWS, "thread_count", 3), ("key", "Enter"), ("key", "Escape"), ("wait", "mac-main.reader.pictures", "#len", ("ge", 0))]
PLACEHOLDERS = "mac-main.reader.body.placeholders"
STATES += [
    {"id": "mail-written-picture-signature", "item": "owner-2026-11-13", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": MAIL_OPEN + [("click", "mac-main.reader.body.sample.signature"), ("wait", PLACEHOLDERS, "#len", ("ge", 8)), ("settle", SETTLE_S)],
     "what": "a short mail a person wrote: drawn in the app's colours (no white page), its signature's 8 blocked pictures as small marks at their declared sizes (never words stacked one letter per line)",
     "views": [("written", [], None)]},
    {"id": "mail-designed-newsletter", "item": "owner-2026-11-13", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": MAIL_OPEN + [("click", "mac-main.reader.body.sample.newsletter"), ("wait", PLACEHOLDERS, "#len", ("ge", 3)), ("settle", SETTLE_S)],
     "what": "a designed newsletter keeps its white paper, with the page's inner margin on all four sides; its 3 wide blocked pictures keep their words on one line",
     "views": [("designed", [], None)]},
]
STATES += [
    {"id": "mail-plain-long-addresses", "item": "owner-2026-11-13", "screen": "mac-main", "sizes": SIZES_MAIN,
     "reach": MAIL_OPEN + [("click", "mac-main.reader.body.sample.trackinglinks"), ("wait", "mac-main.reader.body.links", "#len", ("ge", 3)), ("settle", SETTLE_S)],
     "what": "a plain text mail with two long tracking addresses in angle brackets and one in the open: each is one short link (the words before it, or the site's name and 'link'), never ten lines of letters",
     "views": [("plain-links", [], None)]},
]
STATE_IDS = [s["id"] for s in STATES]


def sel_words(field, want):
    return " and ".join("%s = %s" % (k, json.dumps(v) if v != ("set",) else "set") for k, v in field.items()) if isinstance(field, dict) else "%s = %s" % (field, want)


def step_words(st):
    k = st[0]
    if k == "verb":
        return " ".join(str(x) for x in st[1:])
    if k in ("click", "key", "type"):
        return "%s %s" % (k, json.dumps(st[1], ensure_ascii=False))
    if k == "click_row":
        return "click the first row of %s with %s" % (st[1], sel_words(st[2], st[3]))
    if k == "rest_row":
        return "rest the pointer on the first row of %s with %s (fixture pointer-rest <its ax>)" % (st[1], sel_words(st[2], None))
    if k == "ensure":
        return "read rows %s: unless %s = %s, %s; then until %s = %s" % (st[1], st[2], json.dumps(st[3]), "; ".join(step_words(x) for x in st[4]), st[2], json.dumps(st[3]))
    if k == "wait":
        return "read rows %s until %s = %s" % (st[1], st[2], st[3])
    if k in ("wait_any", "wait_none"):
        return "read rows %s until %s row has %s" % (st[1], "some" if k == "wait_any" else "no", sel_words(st[2], st[3]))
    if k == "progress":
        return "fixture import-progress <run> <%d%% of total>" % round(st[1] * 100)
    if k == "scroll":
        return "scroll %s window:<id> (reply must say at=%s)" % (st[1], " or ".join(st[2]) if isinstance(st[2], tuple) else st[2])
    if k == "pick":
        return "launch %s in turn; Down/Up to the first result with %s" % (" | ".join(json.dumps(q) for q in st[2]), st[1]) + (
            "; on a miss read search/last/closeness/%s and search/last/related_cut" % st[3] if len(st) > 3 else "")
    if k == "corpus_related_file":
        return "fixture corpus <%s/%s> (the J2 corpus plus %s, related by its file only)" % (RELATED_FILE_DIR, RELATED_FILE.NAME, RELATED_FILE.MAIL_ID)
    if k == "to_more":
        return "Down until rows %s says selected" % SM
    if k == "settle":
        return "wait %.1f s" % st[1]
    if k == "read":
        return "read rows %s: %s" % (st[1], ", ".join(st[2]))
    if k == "still":
        return "read rows %s until two reads %.1f s apart give the same offset" % (READER_SCROLL, STILL_GAP_S)
    if k == "fit_editor":
        return "read rows %s.editors: the %s text is not cut inside its card (text_cut false)" % (REPL, st[1])
    if k == "fit_labels":
        return "labels window:<id>: no text under %s is cut (need within box; no ellipsis allowed)" % st[1]
    if k == "laya":
        return {"fresh": "empty the copy's <run folder>/Models/laya and name the loopback download stand-in to it (fixture provider-endpoint laya <stub>)",
                "slow": "the stand-in sends the big file in ten pieces, %.1f s apart" % LAYA_PIECE_S,
                "missing": "the stand-in answers 404 for the last file"}.get(st[1], repr(st))
    return repr(st)


LAYA = [None]            # (the settingsaimac-check module, its Stub("laya")), made on the first use and kept for the run
LAYA_PIECE_S = 0.5       # the J5.7 progress cases' piece delay (settingsaimac-check LAYA_PIECE_S): ten pieces, five seconds for the big file


def laya_stub():
    if LAYA[0] is None:
        import importlib.util
        spec = importlib.util.spec_from_file_location("j5harness_shots", os.path.join(ROOT, "checks", "settingsaimac-check.py"))
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        LAYA[0] = (mod, mod.Stub("laya"))
    return LAYA[0]


def laya_step(c, what):
    mod, stub = laya_stub()
    if what == "fresh":
        run = getattr(c, "dir", "") or ""
        if not run or not os.path.exists(os.path.join(run, "whoosh.sock")):
            raise StepFail("no rig run folder (WHOOSH_SOCKET_DIR): the copy's models folder to empty is unknown")
        owner = os.path.realpath(os.path.expanduser("~/Library/Application Support/Whoosh"))
        folder = os.path.realpath(os.path.join(run, "Models", "laya"))
        if folder == owner or folder.startswith(owner + os.sep):
            raise StepFail("the models folder %s is the owner's, never emptied by a check" % folder)
        shutil.rmtree(folder, ignore_errors=True)
        if os.path.exists(folder):
            raise StepFail("the copy's models folder %s could not be emptied" % folder)
        stub.notfound, stub.piece_delay, stub.delay = set(), 0, 0
        c.send("fixture", "provider-endpoint", "laya", stub.url)   # the copy reads its disk again (AIFixture: layaStateChanged)
    elif what == "slow":
        stub.piece_delay = LAYA_PIECE_S
    elif what == "missing":
        stub.notfound = {mod.LAYA_FILES[-1]}
    else:
        raise StepFail("unknown laya step %r" % what)


def pic_name(sd, view, theme, w, h):
    return "state-%s%s-%s-%dx%d" % (sd["id"], "-" + view if view else "", theme, w, h)


def list_states(say=print):
    n = 0
    for sd in STATES:
        say("state %s (item %s): %s" % (sd["id"], sd["item"], sd["what"]))
        say("  opens: fixture reset; shot %s; then per size and theme: resize <w> <h>; theme light|dark" % sd["screen"])
        for stp in sd["reach"]:
            say("  reach: " + step_words(stp))
        for view, steps, crop in sd["views"]:
            for stp in steps:
                say("  view %s: %s" % (view or "-", step_words(stp)))
            say("  view %s: capture shot %s%s window:<id>" % (view or "-", pic_name(sd, view, "<theme>", 0, 0).replace("-0x0", "-<w>x<h>"),
                                                                (" <row ax>  (a crop of that row)" if crop else "")))
            for lab, w, h in sd["sizes"]:
                for th in THEMES:
                    say("  file: pics/%s.png" % pic_name(sd, view, th, w, h) + ("  and pics/%s-closeup.png" % pic_name(sd, view, th, w, h) if crop else ""))
                    n += 1
    say("%d states, %d pictures" % (len(STATES), n))
    return n


def _ok(got, want):
    if isinstance(want, tuple) and want and want[0] == "ge":
        return isinstance(got, (int, float)) and not isinstance(got, bool) and got >= want[1]
    if want == ("set",):                 # item 0b095f58: the field holds a value (not null), whatever it is
        return got is not None
    return got == want


def row_has(r, field, want):
    """A row matches a selector: one field and its wanted value, or (field a dict) every field of the dict at its value, so a state names the one row it is about
    (a single unread mail, a thread of 3 that is not pinned) by what the app serves for it, never by its place in the list."""
    if isinstance(field, dict):
        return all(_ok(r.get(k), v) for k, v in field.items())
    return _ok(r.get(field), want)


def row_seen(r, field):
    return {k: r.get(k) for k in field} if isinstance(field, dict) else r.get(field)


def _poll(read, what, secs):
    end = time.time() + secs
    last = "nothing read"
    while True:
        try:
            ok, last = read()
            if ok:
                return last
        except Unreadable as e:
            last = "the app refused: %s" % e
        if time.time() >= end:
            raise StepFail("%s: not reached in %.0f s; last %s" % (what, secs, last))
        time.sleep(POLL_EVERY)


def field_of(row, field):
    """A field of a row, a dotted name reading inside a nested answer (`why_tags.on`: the `on` of the row's `why_tags`)."""
    v = row
    for part in field.split("."):
        v = v.get(part) if isinstance(v, dict) else None
    return v


def ensure_state(c, wid, stp, obs):
    """("ensure", region, field, want, steps): a control that toggles is pressed only when the state it sets is not already the wanted one, then the state is
    waited for. Lane row-marks: Command I and the Why here? switch are toggles, and the label inspector outlives `fixture reset`, so a reach that pressed them
    each time undid in one picture what the picture before had left (main193: list-row-why-tag dark, `open = False`)."""
    _, region, field, want, steps = stp
    rows = c.rows(region)
    got = field_of(rows[0], field) if rows else None
    if _ok(got, want):
        obs.append("%s %s already %s" % (region, field, json.dumps(got)))
        return
    for x in steps:
        run_step(c, wid, x, obs)

    def read():
        rs = c.rows(region)
        g = field_of(rs[0], field) if rs else None
        return _ok(g, want), "%s = %r" % (field, g)
    _poll(read, "rows %s %s should be %s" % (region, field, want), POLL_S)
    obs.append("%s %s set to %s" % (region, field, json.dumps(want)))


def wait_rows(c, region, field, want, secs=None):
    secs = POLL_S if secs is None else (secs if WAIT_CAP is None else min(secs, WAIT_CAP))

    def read():
        rows = c.rows(region)
        if field == "#len":
            got = len(rows)
        else:
            i, name = (int(field.split(":")[0]), field.split(":", 1)[1]) if ":" in field else (0, field)
            if len(rows) <= i:
                return False, "%d row(s) in %s, none at index %d" % (len(rows), region, i)
            got = rows[i].get(name)
        return _ok(got, want), "%s = %r" % (field, got)
    return _poll(read, "rows %s %s should be %s" % (region, field, want), secs)


def reader_still(c, obs):
    """hid-compose round 19 (main111, 637ae165 at 960): a picture taken while the reader still scrolled drew the reply slab see-through over the
    text. The picture waits until two reads of the reader's offset, STILL_GAP_S apart, give the same number."""
    end = time.time() + POLL_S
    seen = []
    while True:
        a = c.rows(READER_SCROLL)
        time.sleep(STILL_GAP_S if SETTLE_CAP is None else min(STILL_GAP_S, POLL_EVERY))
        b = c.rows(READER_SCROLL)
        if not a or not b:
            raise StepFail("rows %s answered no row, so the reader's scroll cannot be read" % READER_SCROLL)
        seen.append(b[0].get("offset"))
        if a[0].get("offset") == b[0].get("offset"):
            obs.append("reader scroll still at %s of %s" % (b[0].get("offset"), b[0].get("max_offset")))
            return
        if time.time() > end:
            raise StepFail("rows %s: the reader's offset never held still (last reads %s)" % (READER_SCROLL, seen[-6:]))


def wait_some(c, region, field, want, any_row):
    def read():
        rows = c.rows(region)
        seen = [row_seen(r, field) for r in rows]
        hit = any(row_has(r, field, want) for r in rows)
        more = ""
        if field == "replied_last":      # the reply arrow also needs the thread's count to have grown with the sent reply (FixtureCore.sentCountsLocked): print both
            more = "; thread_count of the same rows = %r" % [r.get("thread_count") for r in rows][:8]
        return (hit if any_row else not hit), "%s of %d row(s) = %r%s" % (field, len(rows), seen[:8], more)
    return _poll(read, "rows %s: %s row with %s" % (region, "some" if any_row else "no", sel_words(field, want)), POLL_S)


# round 18 (d1ffe36c, main107): the copy stopped answering after the related-from-attachment state's steps, and every later tile said
# "verb `fixture reset` failed: ... Broken pipe". A tile names the state and the step after which the copy went quiet, never only the pipe.
LAST_STEP = [None, None]   # (state id, the words of the last step sent)
GONE = [None]              # once the copy stopped answering: the sentence every later tile says
GONE_WORDS = ("Broken pipe", "socket closed", "timed out", "Connection reset", "not connected")


def copy_gone(e):
    """True when a verb's refusal says the copy is no longer answering (it died, or its main thread is blocked past the socket's wait)."""
    return any(w in str(e) for w in GONE_WORDS)


def stack_sample(state_id, outdir):
    """When the copy is alive but does not answer: three seconds of its threads' stacks by macOS `sample` (the hidden copy's pid from the run
    folder's rig.pid, scripts/launch-one.sh), written beside the pictures; the file's name, or why there is none. Nothing is shown on screen."""
    d = os.environ.get("D") or os.environ.get("WHOOSH_SOCKET_DIR") or ""
    pidf = os.path.join(d, "rig.pid")
    if not d or not os.path.isfile(pidf):
        return "no stack sample: no rig.pid in the run folder"
    try:
        pid = int(open(pidf).read().strip())
        os.kill(pid, 0)
    except (ValueError, OSError) as e:
        return "no stack sample: the copy's process is gone (%s)" % type(e).__name__
    dest = os.path.join(outdir, "hang-%s.txt" % state_id)
    import subprocess
    with open(dest + ".log", "w") as log:
        r = subprocess.run(["/usr/bin/sample", str(pid), "3", "-file", dest], stdout=log, stderr=log, timeout=60)
    return "the copy's stacks: %s (sample exit %d)" % (dest, r.returncode) if os.path.isfile(dest) else "no stack sample: sample exit %d (%s.log)" % (r.returncode, dest)


SCROLLED = [0.0]     # the offset the last `scroll` of this combo reported (points from the top of the scrolled page)


def do_scroll(c, wid, cmd, at, obs):
    try:
        r = c.send("scroll", cmd, "window:%s" % wid)
        SCROLLED[0] = float(r.get("offset") or 0.0) if r.get("scrollable") is True else 0.0
    except Unreadable as e:
        raise StepFail("`scroll %s` failed: %s" % (cmd, e))
    obs.append("scroll %s: at %s, offset %s of %s, content %s, view %s, scrollable %s" % (
        cmd, r.get("at"), r.get("offset"), r.get("max"), r.get("content_h"), r.get("view_h"), r.get("scrollable")))
    if r.get("scrollable") is True and not (r.get("at") in at if isinstance(at, tuple) else r.get("at") == at):
        raise StepFail("`scroll %s` left the page at %r, not %r (offset %s of %s)" % (cmd, r.get("at"), at, r.get("offset"), r.get("max")))
    if r.get("scrollable") is False:
        obs.append("the page fits the window: nothing to scroll, its end is its top")


def go_to_more(c):
    for _ in range(60):
        m = c.rows(SM)
        if m and m[0].get("selected"):
            return
        c.key("Down")
    raise StepFail("Down 60 times never reached the closed line %s" % SM)


PICK_WAIT_S = 6.0            # how long one query may take to list a result that holds the condition, before the next query is tried
PICK_STOPS_AT_NO_ROWS = False   # the self-test's broken copy: the way before 16 Nov (a query with no result ended the whole pick)


def pick_result(c, cond, queries, obs, mail=None):
    """Launches the queries in turn and selects the first result that holds `cond`. A query that lists no such result inside PICK_WAIT_S is a miss, and the
    next one is tried: the related-file queries are meaning-only, and a mail under the related cut-off for one of them lists NO row at all (main196, lead,
    16 Nov: closeness 0.65, 0.55, 0.67 under the cut-off 0.7 for the first three, 0.72 for the fourth; the pick stopped at the first empty answer)."""
    seen = []
    for q in queries:
        for stp in launched(q):
            if stp[0] == "wait" and not PICK_STOPS_AT_NO_ROWS:
                continue                     # the answer is waited for below, and an empty one is a miss, not the end
            run_step(c, None, stp, obs)
        t0, rows, idx = time.time(), [], None
        while True:
            rows = c.rows(SR)
            idx = next((i for i, r in enumerate(rows) if all(r.get(k) == v for k, v in cond.items())), None)
            if idx is not None or time.time() - t0 >= (0 if getattr(c, "instant", False) else PICK_WAIT_S):
                break
            time.sleep(0.2)
        seen.append("%r: %s" % (q, [(r.get("part"), r.get("source")) for r in rows][:6]))
        if mail:   # round 23: by how much the mail missed the related part (the test engine's closeness and cut-off, numbers only)
            try:
                seen[-1] += " (closeness of %s %s, related cut-off %s)" % (mail, c.core("search/last/closeness/%s" % mail), c.core("search/last/related_cut"))
            except Unreadable as e:
                seen[-1] += " (closeness unread: %s)" % str(e)[:80]
        if idx is None:
            continue
        sel = next((i for i, r in enumerate(rows) if r.get("selected")), None)
        if sel is None:
            raise StepFail("no result of %r is selected" % q)
        for _ in range(abs(idx - sel)):
            c.key("Down" if idx > sel else "Up")
        now = c.rows(SR)
        if not (len(now) > idx and now[idx].get("selected")):
            raise StepFail("the result %d of %r did not become the selected one" % (idx, q))
        obs.append("selected result %d of %r (%s)" % (idx, q, ", ".join("%s=%s" % (k, v) for k, v in cond.items())))
        return
    raise StepFail("no result holding %s in the results of any query tried (part, source of the first six rows: %s)" % (cond, "; ".join(seen)))


def run_step(c, wid, stp, obs):
    k = stp[0]
    LAST_STEP[1] = step_words(stp)
    try:
        if k == "verb":
            c.send(*stp[1:])
        elif k == "click":
            c.click(stp[1])
        elif k == "key":
            c.key(stp[1])
        elif k == "type":
            c.type(stp[1])
        elif k == "wait":
            wait_rows(c, stp[1], stp[2], stp[3], stp[4] if len(stp) > 4 else None)
        elif k == "click_row":
            ax = next((r.get("ax") for r in c.rows(stp[1]) if row_has(r, stp[2], stp[3]) and r.get("ax")), None)
            if not ax:
                raise StepFail("no row of %s has %s = %r, so there is no row to click" % (stp[1], stp[2], stp[3] if not isinstance(stp[2], dict) else None))
            c.click(ax)
        elif k == "rest_row":
            ax = next((r.get("ax") for r in c.rows(stp[1]) if row_has(r, stp[2], None) and r.get("ax")), None)
            if not ax:
                raise StepFail("no row of %s has %s, so there is no row to rest the pointer on" % (stp[1], sel_words(stp[2], None)))
            rep = c.send("fixture", "pointer-rest", ax)
            if not rep.get("list_row"):
                raise StepFail("`fixture pointer-rest %s` rested on no list row (reply %r): a build whose verb serves the notch only" % (ax, rep))
            obs.append("pointer rests on %s (%s)" % (ax, rep.get("list_row")))
        elif k == "ensure":
            ensure_state(c, wid, stp, obs)
        elif k == "wait_any":
            wait_some(c, stp[1], stp[2], stp[3], True)
        elif k == "wait_none":
            wait_some(c, stp[1], stp[2], stp[3], False)
        elif k == "progress":
            row = c.rows(WI + ".progress")[0]
            total = row.get("total")
            if not isinstance(total, (int, float)) or isinstance(total, bool) or total <= 0 or not row.get("run_id"):
                raise StepFail("rows %s.progress holds no run with a total: %r" % (WI, row))
            c.send("fixture", "import-progress", row["run_id"], str(int(total * stp[1])))
        elif k == "scroll":
            do_scroll(c, wid, stp[1], stp[2], obs)
        elif k == "pick":
            pick_result(c, stp[1], stp[2], obs, stp[3] if len(stp) > 3 else None)
        elif k == "corpus_related_file":
            c.send("fixture", "corpus", RELATED_FILE.write(RELATED_FILE_DIR))
        elif k == "to_more":
            go_to_more(c)
        elif k == "settle":
            time.sleep(stp[1] if SETTLE_CAP is None else min(stp[1], SETTLE_CAP))
        elif k == "laya":
            laya_step(c, stp[1])
        elif k == "fit_editor":
            row = next((r for r in c.rows(REPL + ".editors") if r.get("key") == stp[1]), None)
            if row is None or row.get("text_cut") is None:
                raise StepFail("the editors row has no %s editor on screen (text_cut %r)" % (stp[1], None if row is None else row.get("text_cut")))
            obs.append("%s: text_cut %s" % (stp[1], json.dumps(row.get("text_cut"))))
            if row.get("text_cut") is True:
                raise FitFail("the %s text is cut inside its card (rows %s.editors: text_cut true)" % (stp[1], REPL))
        elif k == "fit_labels":
            r = c.send("labels", "window:%s" % wid)
            rows = [x for x in (r.get("labels") or []) if isinstance(x, dict) and str(x.get("ax", "")).startswith(stp[1]) and x.get("text") is not None]
            if not rows:
                raise StepFail("`labels` served no text under %s (a build without the marks of round 24)" % stp[1])
            cut = ['%s "%s" [needs %.1fx%.1f pt, box %.1fx%.1f pt]' % (x["ax"], x["text"], x["need_w"], x["need_h"], x["box_w"], x["box_h"])
                   for x in rows if x["need_w"] > x["box_w"] + CLIP_TOL_PT or x["need_h"] > x["box_h"] + CLIP_TOL_PT]
            obs.append("labels under %s: %d texts, %d cut" % (stp[1], len(rows), len(cut)))
            if cut:
                raise FitFail("cut, where no name may be cut: %s" % "; ".join(cut))
        elif k == "still":
            reader_still(c, obs)
        elif k == "read":
            rows = c.rows(stp[1])
            if not rows:
                raise StepFail("rows %s answered no row, so %s cannot be read" % (stp[1], ", ".join(stp[2])))
            obs.append("%s: %s" % (stp[1], "; ".join("%s=%s" % (f, json.dumps(rows[0].get(f), ensure_ascii=False)) for f in stp[2])))
        else:
            raise StepFail("unknown step %r" % (stp,))
    except Unreadable as e:
        raise StepFail("%s: %s" % (step_words(stp), e))
    except (IndexError, KeyError, TypeError) as e:
        raise StepFail("%s: the answer lacked a field (%s: %s)" % (step_words(stp), type(e).__name__, e))


class RowOutside(Exception):
    """The row a close-up is cut from is not inside the window: the picture does not show it (a FAIL, never a PASS)."""


class CardInk(Exception):
    """The card's rectangle holds ink that is not the card's: the page behind it shows through (item 5aa79f87)."""


CARD_INK_MAX_PCT = 0.2   # the share of the card's pixels that may differ between two pictures that differ only in the mail behind the card (a changed pixel: > 24 on a channel)


def card_ink(c, sd, theme, w, h, crop, outdir, st, reply_id, name):
    """The card is opaque: take the picture three times with a different mail body behind the card (the thread, the designed sample, the plain sample: the doors
    `mac-main.reader.body.sample.*`), cut the card's own frame (read with each picture, in the same turn as its pixels) out of each and compare them. The card draws the
    same in all three, so any pixel that differs is mail text drawn through or over it. Seen red on the unfixed code (the page pasted over the card) by the selftest model
    `body-through-card`; a shot that says `web: not ready` makes it NOT JUDGED (StepFail), never a pass."""
    from PIL import Image, ImageChops
    ax = crop[1]
    shots = []
    for tag, door in (("a", None), ("b", "mac-main.reader.body.sample.designed"), ("c", "mac-main.reader.body.sample.plain")):
        if door:
            c.click(door)
            time.sleep(SETTLE_S if SETTLE_CAP is None else min(SETTLE_S, SETTLE_CAP))
        sr = on_window(c, sd["screen"], reply_id, lambda wid: c.send("capture", "shot", "%s-ink-%s" % (name, tag), ax, "window:%s" % wid))
        if "web: not ready" in str(sr.get("source", "")):
            raise StepFail("the shot says the page was not ready (%s): the card cannot be compared" % sr.get("source"))
        fr = ((sr.get("frames") or {})).get(ax)
        if not fr:
            raise StepFail("`shot` answered no frame for %s (frames: %r)" % (ax, sr.get("frames")))
        shots.append((sr, fr))
    sr0, fr = shots[0]
    mp = sr0.get("mapping") or {}
    sc = mp.get("scale") or 1
    box = [int(round(v)) for v in ((fr["x"] + (mp.get("x") or 0)) * sc, (fr["y"] + (mp.get("y") or 0)) * sc, (fr["x"] + fr["w"] + (mp.get("x") or 0)) * sc, (fr["y"] + fr["h"] + (mp.get("y") or 0)) * sc)]
    pieces = []
    for sr, _ in shots:
        with Image.open(sr["path"]) as im:
            b = [max(0, box[0]), max(0, box[1]), min(im.size[0], box[2]), min(im.size[1], box[3])]
            if b[2] - b[0] < 8 or b[3] - b[1] < 8:
                raise StepFail("the card's frame %r lies outside the picture %dx%d" % (fr, im.size[0], im.size[1]))
            pieces.append(im.convert("RGB").crop(tuple(b)))
    worst = 0.0
    for i, j in ((0, 1), (1, 2), (0, 2)):
        d = ImageChops.difference(pieces[i], pieces[j]).convert("L").point(lambda v: 255 if v > 24 else 0)
        pct = 100.0 * sum(1 for v in d.getdata() if v) / max(1, d.size[0] * d.size[1])
        worst = max(worst, pct)
    dest_rel = "pics/%s-closeup.png" % name
    os.makedirs(os.path.dirname(os.path.join(outdir, dest_rel)), exist_ok=True)
    pieces[1].save(os.path.join(outdir, dest_rel))
    st["closeup"] = dest_rel
    st["how"].append("card ink: the card's frame %.0f,%.0f %.0fx%.0f pt differs by at most %.2f %% of its pixels between three mail bodies behind it (limit %.1f %%)" % (fr["x"], fr["y"], fr["w"], fr["h"], worst, CARD_INK_MAX_PCT))
    if worst > CARD_INK_MAX_PCT:
        raise CardInk("mail ink inside the card: %.2f %% of the card's pixels change with the mail behind it (limit %.1f %%); frame %.0f,%.0f %.0fx%.0f pt" % (worst, CARD_INK_MAX_PCT, fr["x"], fr["y"], fr["w"], fr["h"]))


def crop_row(c, sd, view, theme, w, h, crop, outdir, st, reply_id, name):
    """The close-up: the frame of the row `crop` names, from the shot reply's frames (read in the pass that drew the pixels), cut from the picture."""
    from PIL import Image
    _, region, field, want = crop
    rows = c.rows(region)
    ax = next((r.get("ax") for r in rows if r.get(field) == want and r.get("ax")), None)
    if not ax:
        raise StepFail("no row of %s has %s = %r, so there is no row to crop" % (region, field, want))
    sr = on_window(c, sd["screen"], reply_id, lambda wid: c.send("capture", "shot", name + "-closeup", ax, "window:%s" % wid))
    fr = (sr.get("frames") or {}).get(ax)
    if not fr:
        raise StepFail("`shot` answered no frame for %s (frames: %r): the row is not marked .whooshAx, so it cannot be cut out; WelcomeRowView.swift:34 carries a plain .accessibilityIdentifier" % (ax, sr.get("frames")))
    mp = sr.get("mapping") or {}
    sc = mp.get("scale") or 1
    cw, ch = st["dims"]       # the window's content in points, from the picture of this size
    # main95: at 960x600 `scroll end` moved the list by 79 pt (reply: offset 79 of 79, the row is then fully drawn) but the frame the app answered for the row, 389,552 882x75 (light) and
    # 349,552 602x75 (dark), is the row's place BEFORE the scroll (y 552 + 75 = 627, below the 600 pt window): the frame of a view inside the scrolled list does not follow the scroll. So
    # a frame that lies below the window is read again minus the offset the `scroll` reported; the row counts as inside only if that frame is, and the picture is cut there.
    off = SCROLLED[0]
    if off > 0 and fr["y"] + fr["h"] > ch + SIZE_TOL and fr["y"] - off >= -SIZE_TOL and fr["y"] - off + fr["h"] <= ch + SIZE_TOL:
        st["how"].append("the frame answered y %.0f lies below the window, the list was scrolled %.0f pt: read as y %.0f" % (fr["y"], off, fr["y"] - off))
        fr = dict(fr, y=fr["y"] - off)
    if fr["x"] < -SIZE_TOL or fr["y"] < -SIZE_TOL or fr["x"] + fr["w"] > cw + SIZE_TOL or fr["y"] + fr["h"] > ch + SIZE_TOL:
        raise RowOutside("the row %s is not inside the window: its frame is %.0f,%.0f %.0fx%.0f pt and the window shows %.0fx%.0f pt (main92: at 960x600 the refused row lay below the fold behind the Keys bar and the tile read PASS)" % (
            ax, fr["x"], fr["y"], fr["w"], fr["h"], cw, ch))
    m = 12.0
    box = [int(round(v)) for v in (((fr["x"] - m) + (mp.get("x") or 0)) * sc, ((fr["y"] - m) + (mp.get("y") or 0)) * sc,
                                   ((fr["x"] + fr["w"] + m) + (mp.get("x") or 0)) * sc, ((fr["y"] + fr["h"] + m) + (mp.get("y") or 0)) * sc)]
    dest_rel = "pics/%s-closeup.png" % name
    dest = os.path.join(outdir, dest_rel)
    with Image.open(sr["path"]) as im:
        box = [max(0, box[0]), max(0, box[1]), min(im.size[0], box[2]), min(im.size[1], box[3])]
        if box[2] - box[0] < 8 or box[3] - box[1] < 8:
            raise StepFail("the frame of %s (%r) lies outside the picture %dx%d" % (ax, fr, im.size[0], im.size[1]))
        im.crop(tuple(box)).save(dest)
    st["closeup"] = dest_rel
    st["how"].append("close-up of %s: frame %.0f,%.0f %.0fx%.0f pt cut with a %.0f pt margin" % (ax, fr["x"], fr["y"], fr["w"], fr["h"], m))


def run_combo(c, sd, lab, w, h, theme, outdir):
    recs = []
    SCROLLED[0] = 0.0
    for view, _steps, _crop in sd["views"]:
        st = new_state("%s%s" % (sd["id"], "/" + view if view else ""), theme, lab, w, h)
        st["how"] = []
        recs.append((view, st, _steps, _crop))

    def all_nj(why):
        for _, st, _, _ in recs:
            judge(st, "NOT JUDGED", why)
        return [r[1] for r in recs]
    LAST_STEP[:] = [sd["id"], None]
    if GONE[0]:
        return all_nj(GONE[0])
    try:
        c.send("fixture", "reset")
    except Unreadable as e:
        if copy_gone(e):
            GONE[0] = "not reached: the hidden copy stopped answering before this state, at its first verb `fixture reset` (%s); no later state can be reached on it" % e
            return all_nj(GONE[0])
        return all_nj("verb `fixture reset` failed: %s" % e)
    try:
        c.send("fixture", "index-progress", "0", "0")   # `fixture reset` does not clear the index builder's figures: the index line of one state must not reach the next
    except Unreadable:
        pass                                            # a build without the verb has no figures to clear
    # `fixture reset` does not close the first-start flow either (it is the app's own, not the test engine's): a state that ran before with the flow open
    # left its card over the next state's screen, and that state passed with a picture of the card (main182, 14 Nov: list-row-thread-pill drawn under
    # "What the AI may use"). Every state starts with the flow closed; a state that wants it opens it in its own reach.
    try:
        c.send("fixture", "first-start", "off")
    except Unreadable:
        pass                                            # a build without the flow has no card to close
    if not any(len(stp) > 3 and stp[:3] == ("verb", "fixture", "first-start") for stp in sd.get("reach", [])):
        try:
            if c.rows("mac-first-start.card"):
                return all_nj("the first-start card is on the screen at this state's start, after `fixture first-start off`: the picture would show the card, not the state")
        except Unreadable:
            pass                                        # no such region while the flow is closed: nothing covers the screen
    try:
        opened = c.shot(sd["screen"])
    except Unreadable as e:
        return all_nj("reach by `shot %s` failed: %s" % (sd["screen"], e))
    reply_id = opened.get("window")
    try:
        resolve_window(c, sd["screen"], reply_id)
        r = c.send("resize", w, h)
        cont = r.get("content") or {}
        settled = (float(cont["w"]), float(cont["h"]))
        tr = c.send("theme", theme)
    except (Unreadable, KeyError, ValueError, TypeError) as e:
        return all_nj("opening %s at %dx%d in %s failed: %s" % (sd["screen"], w, h, theme, e))
    obs = []
    err = None
    wid_now = lambda: resolve_window(c, sd["screen"], reply_id)   # noqa: E731
    try:
        for stp in sd["reach"]:
            run_step(c, wid_now() if stp[0] in ("scroll", "fit_labels") else None, stp, obs)
    except (StepFail, Unreadable, FitFail) as e:
        err = str(e)
        if copy_gone(e):
            GONE[0] = "not reached: the hidden copy stopped answering during state %s at its step `%s` (%s); no later state can be reached on it" % (sd["id"], LAST_STEP[1] or "none", e)
    for view, st, steps, crop in recs:
        verr, ferr = err, None
        if verr is None:
            try:
                for stp in steps:
                    run_step(c, wid_now() if stp[0] in ("scroll", "fit_labels") else None, stp, obs)
            except (StepFail, Unreadable) as e:
                verr = str(e)
            except FitFail as e:
                ferr = str(e)
        st["how"] = list(obs)
        obs[:] = []
        name = pic_name(sd, view, theme, w, h)
        try:
            ui = c.uistate()
            sr = on_window(c, sd["screen"], reply_id, lambda wid: c.send("capture", "shot", name, "window:%s" % wid))
        except (Unreadable, KeyError, ValueError, TypeError) as e:
            if isinstance(e, Unreadable) and copy_gone(e):
                after = "every step of the state answered" if verr is None else "a step failed (%s)" % verr
                why = ("not reached: %s, the last one `%s`; then the hidden copy stopped answering (%s). %s"
                       % (after, LAST_STEP[1] or "none", e, stack_sample(sd["id"], outdir) if "timed out" in str(e) else "The copy died or closed its socket."))
                judge(st, "NOT JUDGED", why)
                GONE[0] = "not reached: the hidden copy stopped answering during state %s after its step `%s` (%s); no later state can be reached on it" % (sd["id"], LAST_STEP[1] or "none", e)
                continue
            judge(st, "NOT JUDGED", "a verb for this picture failed or answered without its field: %s" % e)
            continue
        if tr.get("theme") != theme or ui.get("theme") != theme:
            judge(st, "FAIL", "theme did not take: asked %s, `theme` replied %s, uistate.theme is %s" % (theme, tr.get("theme"), ui.get("theme")))
        if "web: not ready" in str(sr.get("source", "")):
            judge(st, "NOT JUDGED", "the shot says the page was not ready (%s): a blank or ghost body is never passed" % sr.get("source"))
        dest_rel = "pics/%s.png" % name
        dest = os.path.join(outdir, dest_rel)
        try:
            os.makedirs(os.path.dirname(dest), exist_ok=True)
            src = sr.get("path", "")
            if not src or not os.path.isfile(src):
                raise Fail("picture file is missing: %s" % src)
            shutil.copyfile(src, dest)
            pw, ph, one = inspect(dest)
        except (Fail, OSError) as e:
            judge(st, "FAIL", str(e))
            continue
        st["pic"], st["settled"] = dest_rel, settled
        mp = sr.get("mapping") or {}
        scale = mp.get("scale") or 1
        st["dims"] = (pw / scale, ph / scale - (mp.get("y") or 0))
        if one:
            judge(st, "FAIL", "picture is one colour")
        if settled[0] < w - SIZE_TOL or settled[1] < h - SIZE_TOL:
            judge(st, "FAIL", "size not applied: asked %dx%d, the window settled at %.0fx%.0f" % (w, h, settled[0], settled[1]))
        elif abs(st["dims"][0] - settled[0]) > SIZE_TOL or abs(st["dims"][1] - settled[1]) > SIZE_TOL:
            judge(st, "FAIL", "picture is %.0fx%.0f pt content but `resize` settled at %.0fx%.0f" % (st["dims"][0], st["dims"][1], settled[0], settled[1]))
        if verr is None and crop:
            try:
                if crop[0] == "cardink":
                    card_ink(c, sd, theme, w, h, crop, outdir, st, reply_id, name)
                else:
                    crop_row(c, sd, view, theme, w, h, crop, outdir, st, reply_id, name)
            except (RowOutside, CardInk) as e:
                judge(st, "FAIL", str(e))
            except (StepFail, Unreadable, OSError) as e:
                verr = "close-up: %s" % e
        if verr is None and ferr is not None:
            judge(st, "FAIL", ferr)
        if verr is not None:
            judge(st, "NOT JUDGED", "the state was not reached, the picture shows what the app drew: %s" % verr)
        elif st["verdict"] is None:
            judge(st, "PASS", "state reached (%s)" % ("; ".join(st["how"]) or "every step answered and every row read said so"))
    return [r[1] for r in recs]


def run_states(c, outdir, wanted, deadline_s=1500.0, say=print):
    """Drive the named states; returns (lines, rows by state id)."""
    os.makedirs(outdir, exist_ok=True)
    t0 = time.time()
    lines, rows = [], {}
    GONE[0] = None
    for sd in STATES:
        if wanted and sd["id"] not in wanted:
            continue
        sts = []
        for lab, w, h in sd["sizes"]:
            for theme in THEMES:
                if time.time() - t0 > deadline_s:
                    for view, _, _ in sd["views"]:
                        st = new_state("%s/%s" % (sd["id"], view), theme, lab, w, h)
                        st["how"] = []
                        judge(st, "NOT JUDGED", "deadline of %ds reached before this state" % deadline_s)
                        sts.append(st)
                    continue
                try:
                    sts += run_combo(c, sd, lab, w, h, theme, outdir)
                except CouldNotDrive as e:
                    for view, _, _ in sd["views"]:
                        st = new_state("%s/%s" % (sd["id"], view), theme, lab, w, h)
                        st["how"] = []
                        judge(st, "NOT JUDGED", "no socket: %s" % e)
                        sts.append(st)
        if sd.get("lang"):
            try:
                c.click("mac-settings-general.appearance.language.en")   # the copy speaks English again for the states and checks after this one
            except (Unreadable, CouldNotDrive):
                pass
        by = {}
        for st in sts:
            by.setdefault((st["screen"], st["size"]), {})[st["theme"]] = st
        for pair in by.values():
            lt, dk = pair.get("light"), pair.get("dark")
            if lt and dk and lt["pic"] and dk["pic"]:
                try:
                    if same_picture(os.path.join(outdir, lt["pic"]), os.path.join(outdir, dk["pic"])):
                        judge(dk, "FAIL", "theme did not change the picture: light and dark are pixel-identical")
                except Exception as e:
                    judge(dk, "NOT JUDGED", "light and dark pictures could not be compared: %s" % e)
        for st in sts:
            if st["verdict"] is None:
                judge(st, "NOT JUDGED", "no verdict reached")
            lines.append(line_for(st))
            say(lines[-1])
        rows["%s: %s" % (sd["id"], sd["what"])] = sts
    write_page(outdir, rows, {k: "item %s" % next(s["item"] for s in STATES if k.startswith(s["id"] + ":")) for k in rows},
               ["States are named in checks/shots-sheet.py STATES; each tile says how it was reached (the verbs it sent and what the rows answered). A tile marked NOT JUDGED is a picture of what the app drew when the state was not reached.",
                "A hidden-rig picture draws layout, text and placement, not the glass material (CONTROL-SOCKET delta 50)."])
    return lines, rows


def summarize(lines, say=print):
    n = len(lines)
    f = sum(1 for l in lines if l.startswith("FAIL"))
    u = sum(1 for l in lines if l.startswith("NOT JUDGED"))
    say("%d checks, %d failed, %d not judged" % (n, f, u))
    return f, u


# ---------------------------------------------------------------- self-test
def real_standin_class():
    """The stand-in of checks/checklib/shots_standin.py (not owned by this lane, so it is extended here) with the answers the real app gave on
    the hidden run of build main80 (lane-inputs-main81/shots-sheet.log), copied word for word:
      - `shot <surface>` replies `window: surface-AppWindow-1` for a surface window, while the window is open as <surface> (its identifier
        is the surface id once tagged: WindowBridge.swift:56-57), and `shot <name> window:surface-AppWindow-1` is refused with
        `no open window has the id surface-AppWindow-1; open windows (id|title): mac-main-AppWindow-1|Whoosh, mac-search|Rechercher`;
      - `rows mac-main.status.link` is refused `no rows at mac-main.status.link`; the row is served by `rows mac-main.sync.line`;
      - `labels` on the composer marks views that are all buttons, fields and containers: every row answers `text: null`.
    Extra modes: late-tag (the window's identifier is SwiftUI's own name for the first `winframes` read, then the screen id)."""
    from checklib.shots_standin import Standin, SURFACES, START
    MAIN, SETTINGS = "mac-main-AppWindow-1", "mac-settings-AppWindow-1"
    TITLES = {"mac-search": "Rechercher", "mac-palette": "Palette de commandes", "mac-trainer": "Coach Whoosh", "mac-notch": "",
              "mac-welcome-accounts": "", "mac-labels-contacts": "\u00c9tiquettes", "mac-mcp-notes": "Apps d\u2019IA externes", "mac-quickreply": "R\u00e9ponse rapide"}
    COMPOSER_IDS = ["mac-composer.attach", "mac-composer.body", "mac-composer.cc", "mac-composer.footer", "mac-composer.send", "mac-composer.surface"]

    class Real(Standin):
        def __init__(self, sock_dir, mode="good"):
            super().__init__(sock_dir, mode)
            self.reset_windows()

        def reset_windows(self):
            self.wins, self.cur, self.surface, self.untagged = {MAIN: dict(START)}, None, "mac-main", {}
            self.pane = "decisions"   # what `shot mac-settings-ai` shows on the app (SettingsPane.pane(for: .macSettingsAI))

        def ident(self, key):
            return "surface-AppWindow-1" if key in self.untagged else key

        def listing(self):
            return ", ".join("%s|%s" % (self.ident(k), TITLES.get(k, "Whoosh" if k == MAIN else "R\u00e9glages")) for k in self.wins)

        def key_of(self, wid):
            return next((k for k in self.wins if self.ident(k) == wid), None)

        def handle(self, verb, a):
            if verb == "fixture" and a[:1] == ["reset"]:
                self.reset_windows()
                return {"ok": True}
            if verb == "winframes":
                rows = [{"title": TITLES.get(k, ""), "x": 0, "y": 0, "w": v["w"], "h": v["h"], "kind": "panel" if k == "mac-notch" else "window",
                         "id": self.ident(k), "surface": k if k in TITLES else ""} for k, v in self.wins.items()]
                self.untagged = {k: n - 1 for k, n in self.untagged.items() if n > 1}   # tagged a moment later
                return {"ok": True, "frames": rows}
            if verb in ("shot", "capture"):
                args = a[1:] if verb == "capture" else a
                name, rest = args[0], args[1:]
                asked = [x[len("window:"):] for x in rest if x.startswith("window:")]
                if asked:
                    k = self.key_of(asked[0])
                    if k is None:
                        return {"ok": False, "error": "no open window has the id %s; open windows (id|title): %s" % (asked[0], self.listing())}
                    self.cur = k
                    return self.shot(name)
                if name in SURFACES:
                    k = name if name in TITLES else (SETTINGS if name.startswith("mac-settings") else MAIN)
                    if k not in self.wins:
                        self.wins[k] = dict(START)
                        if k in TITLES and self.mode == "late-tag":
                            self.untagged[k] = 2   # reported under SwiftUI's name by the next two `winframes` reads
                    self.cur, self.surface = k, name
                    r = self.shot(name)
                    r["window"] = "surface-AppWindow-1" if k in TITLES else k   # the name SwiftUI gave it, read before the tag
                    return r
                self.cur = self.cur or MAIN
                return self.shot(name)
            if verb == "click" and a and a[0].startswith(SETTINGS_W + ".pane-row."):
                if self.mode != "pane-ignored":
                    self.pane = a[0].rsplit(".", 1)[1]
                return {"ok": True}
            if verb == "rows" and a[0] == SETTINGS_W + ".place":
                return {"ok": True, "rows": [{"pane": self.pane}]}
            if verb == "rows":
                if a[0] == "mac-main.sync.line":
                    r = super().handle("rows", ["mac-main.status.link"])
                    return r
                return {"ok": False, "error": "no rows at %s" % a[0]}
            if verb == "labels":
                if self.mode == "no-labels-verb":
                    return super().handle(verb, a)
                win = [x[len("window:"):] for x in a if x.startswith("window:")]
                k = self.key_of(win[0]) if win else None
                if k is None:
                    return {"ok": False, "error": "no open window has the id %s" % (win[0] if win else "")}
                if k == MAIN and self.surface == "mac-composer":
                    rows = [{"ax": i, "text": None, "need_w": 30.0, "need_h": 20.0, "box_w": 30.0, "box_h": 20.0} for i in COMPOSER_IDS]
                    return {"ok": True, "labels": rows, "judged": 0, "marked": len(rows), "capped": False, "window": win[0]}
                keep, self.wins = self.wins, {win[0]: dict(START)}   # the base stand-in knows windows by the id it is asked for
                try:
                    return super().handle(verb, a)
                finally:
                    self.wins = keep
            return super().handle(verb, a)
    return Real


def driven_standin_class():
    """The stand-in of `real_standin_class` extended with the verbs the named states send: click, key, type, fixture, rows, scroll and `capture shot <name> <ax> window:<id>`
    (frames). Its rows are what the app's own rows would say once the state is reached (the shapes read from par-searchmac.py, par-welcomemac.py, settingsreplies-check.py,
    J4.py). A stand-in, never proof of the real app. Modes: good | page-stuck | no-scroll-verb | scroll-short | page-fits | click-refused | no-frames | no-related
    | no-matches | refuse-ignored | no-index-verb | index-note-never | inspector-stuck (the last five: the states of lane fix-rest-app)."""
    Real = real_standin_class()

    class Driven(Real):
        def __init__(self, sock_dir, mode="good"):
            super().__init__(sock_dir, mode)
            self.log = []
            self.world()

        def world(self):
            self.page, self.text, self.accts, self.connected = "auto", "", [], False
            self.scanned = self.sel_src = self.started = self.sheet = self.search = self.launched = self.more_sel = self.more_open = self.draft = False
            self.done, self.sel, self.chips = 0, 0, 0
            self.refuse, self.queued = None, 0
            # the label inspector and its Why here? switch are the app's own: `fixture reset` leaves them as they are (main193: the light pictures left the inspector open)
            self.inspector, self.why_on = getattr(self, "inspector", False), getattr(self, "why_on", False)
            self.mail = None
            self.focus, self.read_ids, self.star_ids, self.sel_ids, self.draft_ids = 0, set(), set(), set(), set()   # item 0b095f58: the list's own state (focus, read, star, X, drafts)
            self.rtext, self.rtarget, self.compact = "", None, False
            # lane row-marks: the mailbox view shown, the snoozed, muted and VIP marks (the test engine's, so the reset clears them), the snooze picker, the palette and the
            # pointer's rest (ListPointerRest clears it on the reset)
            self.box, self.snz_ids, self.mute_ids, self.vip_senders = None, set(), set(), set()
            self.pending, self.ptyped, self.palette, self.rest = None, "", None, None
            self.reply_open = self.sent = self.advanced = False   # the reply-arrow state: the reply field, the send, the clock past the undo window
            self.qr_header_open = self.qr_from_open = False          # item 637ae165: the quick reply's header and From picker
            self.reader_offset, self.reader_reads = 0.0, 0                # round 19: the reader's scroll (R takes it to the end, ⌘Up to the top)
            self.sample, self.fs_step = "written", 1
            self.fs_open = getattr(self, "fs_open", False)                # the first-start flow is the app's own: `fixture reset` leaves it as it is (as the app does)
            self.laya_base, self.laya, self.laya_t0, self.laya_why = None, "absent", 0.0, ""   # hid-ai round 18: the decision model's row

        # item 0b095f58: the list the row states read. The facts are the sample mails' own (docs/design/fixtures/mac-main-sample.json: msg-01 pinned unread thread of 3 with a
        # file, msg-02 pinned read, msg-05 unread thread of 2 with a file, msg-06 read single, msg-09 unread single, msg-16 unread thread of 3, msg-04 unread single with a file).
        LIST_BASE = [("msg-01", "Linea Supply", 3, True, True, 1), ("msg-02", "Marie Lef\u00e8vre", 2, False, True, 2), ("msg-04", "Hetzner-style host", 1, True, False, 1),
                     ("msg-03", "Ton Verheyden", 5, True, False, 0), ("msg-05", "Nina Okafor", 2, True, False, 1), ("msg-06", "Apple Developer", 1, False, False, 0), ("msg-09", "GitHub", 1, True, False, 0),
                     ("msg-16", "Bart De Smet", 3, True, False, 0), ("msg-10", "LHG Procurement", 4, False, False, 1)]   # lane row-marks: msg-10, the read thread of 4

        ATTENTION_ID = "msg-03"   # the sample's mail of the account that needs sign-in: the engine refuses every act on it

        def in_box(self, mid, who):
            """The mailbox view holds the mail: the inbox keeps out the snoozed, the muted and the VIP senders' mail (they leave it), each view holds its own."""
            if self.box == "snoozed":
                return mid in self.snz_ids
            if self.box == "muted":
                return mid in self.mute_ids
            if self.box == "vips":
                return who in self.vip_senders
            return mid not in self.snz_ids and mid not in self.mute_ids and who not in self.vip_senders

        def mark_read(self, mid):
            """Opening a thread marks its mail read (FixtureCore.swift `case .openThread`: `read.insert(m.id)`)."""
            base = next(b[3] for b in self.LIST_BASE if b[0] == mid)
            if base and mid not in self.read_ids:
                self.read_ids.add(mid)
            elif not base and mid in self.read_ids:
                self.read_ids.discard(mid)

        def list_rows(self):
            m, out = self.mode, []
            base = [b for b in self.LIST_BASE if not (m == "no-pill-row" and b[2] == 3) and not (m == "no-single-row" and b[2] == 1) and self.in_box(b[0], b[1])]
            for i, (mid, who, tc, unread, pinned, att) in enumerate(base):
                if m == "no-read-single" and tc == 1:
                    unread = True                                     # the broken copy: no single message is ever read
                if m == "no-pinned":
                    pinned = False
                if m == "no-attachment":
                    att = 0
                out.append({"ax": "mac-main.list.row.%d" % i, "id": mid, "sender": who, "thread_count": tc, "unread": unread != (mid in self.read_ids), "pinned": pinned,
                            "attachments": att, "focused": i == self.focus, "selected": mid in self.sel_ids, "starred": mid in self.star_ids,
                            "draft": mid in self.draft_ids, "why_tag": "\u2726 IA \u00b7 Sans r\u00e9ponse : non" if (self.inspector and self.why_on) else None,
                            "replied_last": mid == "msg-05" and m != "no-reply-row" and self.sent and self.advanced,
                            # lane row-marks: the marks ListProbe.rows serves; each broken copy serves one of them dead
                            "muted": mid in self.mute_ids and m != "muted-field-false", "vip": who in self.vip_senders and m != "vip-field-false",
                            "snoozed": "back 08:00" if (mid in self.snz_ids and m != "snooze-words-null") else None,
                            "hovered": mid == self.rest, "rested": mid == self.rest,
                            "account_attention": mid == self.ATTENTION_ID})
            return out

        def list_key(self, k):
            """What the list does with a key (the app's own: J and K move the focus, U toggles read, S the star, X the selection, Escape closes the reply and keeps a typed
            reply as a draft, then clears the selection; Command I opens the label inspector). Each mode is the same copy with one of them dead."""
            m, rows = self.mode, self.list_rows()
            if not rows or self.search:
                return False
            cur = rows[min(self.focus, len(rows) - 1)]["id"]
            who = rows[min(self.focus, len(rows) - 1)]["sender"]
            if k == "Enter" and self.palette is not None:      # the palette's highlighted line runs on the focused mail (AppModel.shownMessageId)
                if self.palette and "mark as vip".startswith(self.palette.lower()):
                    self.vip_senders.add(who)
                self.palette = None
                return True
            if k == "Enter" and self.pending == "snooze":     # the picker's typed line ("tomorrow morning") snoozes the focused mail
                if "tomorrow morning" in self.ptyped:
                    self.snz_ids.add(cur)
                self.pending, self.ptyped = None, ""
                return True
            if k == "\u2318P":
                self.palette = ""
                return True
            if k == "H":
                self.pending, self.ptyped = "snooze", ""
                return True
            if k == "\u21e7M":
                self.mute_ids ^= {cur}
                return True
            if k == "J":
                self.focus = min(self.focus + 1, len(rows) - 1)
            elif k == "K":
                self.focus = max(self.focus - 1, 0)
            elif k == "U" and m != "u-dead":
                self.read_ids ^= {cur}
            elif k == "S" and m != "s-dead":
                self.star_ids ^= {cur}
            elif k == "X" and m != "x-dead":
                self.sel_ids ^= {cur}
            elif k == "Enter":
                self.rtarget = cur
                self.mark_read(cur)
            elif k == "\u2318I":
                self.inspector = not self.inspector     # a toggle (LabelInspectorModel.toggle); opening or closing turns the Why here? switch off
                self.why_on = False
            elif k == "Escape":
                if self.reply_open:
                    if self.rtext and not self.sent and self.rtarget and m != "draft-lost":
                        self.draft_ids.add(self.rtarget)
                elif self.sel_ids:
                    self.sel_ids = set()
            return False

        def results(self):
            if not self.launched or "zzqxvk" in self.text or self.refuse:
                return []
            if self.text.strip() in RELATED_FILE.QUERIES[:-1]:   # meaning-only queries the related mail is not close enough to: no row at all (main196)
                return []
            if self.mode == "no-related":   # the meaning index is not there: every row is a word match, no related part
                return [{"part": "match", "source": "text"}] * 4
            return [{"part": "match", "source": "text"}, {"part": "match", "source": "text"}, {"part": "related", "source": "attachment"}, {"part": "related", "source": "text"}]

        def matches(self):
            return 0 if self.mode == "no-matches" else sum(1 for x in self.results() if x["part"] == "match")

        def handle(self, verb, a):
            if getattr(self, "dead", False):
                raise SystemExit   # mode dies-on-related: the stand-in's serve thread ends silently and its socket closes, as a copy that died
            rep = self.answer(verb, a)
            if getattr(self, "dying", False) and verb == "rows":
                self.dying, self.dead = False, True   # the selection was read back; the next verb meets a dead copy
            return rep

        def answer(self, verb, a):
            self.log.append((verb, list(a)))
            m = self.mode
            if verb == "fixture":
                if a[:1] == ["reset"]:
                    self.world()
                    return super().handle(verb, a)
                if a[:2] == ["first-start", "on"]:
                    self.fs_open, self.fs_step = True, 1
                if a[:2] == ["first-start", "off"]:
                    self.fs_open = m == "first-start-stays-open"          # the broken copy: a flow an earlier state opened stays open, its card covers this state
                if a[:1] == ["import-progress"]:
                    self.done = int(a[2])
                if a[:1] == ["search-refuse"]:
                    self.refuse = None if a[1:] == ["off"] else " ".join(a[1:])
                if a[:2] == ["provider-endpoint", "laya"]:
                    self.laya_base, self.laya = a[2], "absent"   # the copy re-reads its (just emptied) disk
                    return {"ok": True}
                if a[:1] == ["pointer-rest"]:                  # lane row-marks: the notch's verb, also on a list row (ListPointerRest)
                    t = a[1] if len(a) > 1 else ""
                    if t.startswith("mac-main.list.row."):
                        rows = self.list_rows()
                        n = int(t.rsplit(".", 1)[1]) if t.rsplit(".", 1)[1].isdigit() else -1
                        if not 0 <= n < len(rows):
                            return {"ok": False, "error": "no row at %s: the list holds %d rows" % (t, len(rows))}
                        if m == "rest-dead":                    # the broken copy: the verb serves the notch only
                            return {"ok": True, "rested_on": t}
                        self.rest = rows[n]["id"]
                        return {"ok": True, "rested_on": t, "list_row": self.rest}
                    self.rest = None
                    return {"ok": True, "rested_on": t, "list_row": None}
                if a[:1] == ["index-progress"]:
                    if m == "no-index-verb":
                        return {"ok": False, "error": "unknown fixture verb index-progress"}
                    self.queued = int(a[2])
                return {"ok": True}
            if verb == "clock":
                self.advanced = self.advanced or (a[:1] == ["advance"] and self.sent)
                return {"ok": True}
            if verb == "scroll":
                if m == "no-scroll-verb":
                    return {"ok": False, "error": "unknown verb scroll"}
                end = a[0] == "end"
                if m == "page-fits":
                    return {"ok": True, "command": a[0], "scrollable": False, "offset": 0.0, "max": 0.0, "content_h": 300.0, "view_h": 400.0, "at": "top", "window": a[1][7:]}
                at = ("middle" if m == "scroll-short" else "end") if end else ("middle" if a[0] == "page-down" else "top")
                if a[0] == "top" and self.reply_open:          # round 20: the reader the quick reply's state pictures at rest
                    if m == "reader-stays-at-end":
                        at = "end"                               # the mutant: the window's scroll leaves the thread at its end
                    else:
                        self.reader_offset = 0.0
                return {"ok": True, "command": a[0], "scrollable": True, "offset": 600.0 if end else 0.0, "max": 600.0, "content_h": 1000.0, "view_h": 400.0, "at": at, "window": a[1][7:]}
            if verb == "click":
                ax = a[0]
                if m == "click-refused":
                    return {"ok": False, "error": "nothing to click at %s" % ax}
                if ".page." in ax and m != "page-stuck":
                    self.page = ax.rsplit(".", 1)[1]
                elif ax == WA + ".reset":
                    self.accts = []
                elif ax == WA + ".paste-list":
                    self.accts = self.text.split()
                elif ax == WA + ".continue":
                    self.accts = self.accts + [self.text]
                elif ax == WA + ".connect-waiting":
                    self.connected = True
                elif ax == WI + ".scan":
                    self.scanned = True
                elif ax == WI + ".select.apple-mail":
                    self.sel_src = True
                elif ax == WI + ".start":
                    self.started = True
                elif ax == "mac-settings-accounts.import":
                    self.sheet = True
                elif ax.startswith("mac-main.list.row.") and ax.rsplit(".", 1)[1].isdigit():
                    self.focus = int(ax.rsplit(".", 1)[1])
                elif ax == "mac-main.list.compact" and m != "compact-dead":
                    self.compact = not self.compact
                elif ax == "mac-labels.inspector.why" and self.inspector and m != "why-dead":
                    self.why_on = not self.why_on             # a toggle (LabelInspectorModel.toggleWhy)
                elif ax in ("mac-main.sidebar.mailbox.snoozed", "mac-main.sidebar.mailbox.muted", "mac-main.sidebar.mailbox.vips"):
                    self.box, self.focus = ax.rsplit(".", 1)[1], 0
                elif ax == "mac-main.sidebar.command.go-inbox":
                    self.box, self.focus = None, 0
                elif ax.startswith("mac-main.reader.body.sample.") and m != "sample-dead":
                    self.sample = ax.rsplit(".", 1)[1]
                    self.mail = self.sample
                elif ax == "mac-main.quickreply.header.toggle" and self.reply_open:
                    self.qr_header_open = not self.qr_header_open
                elif ax == "mac-main.quickreply.from" and self.reply_open and m != "reply-picker-dead":
                    self.qr_from_open = True
                elif ax.startswith(SETTINGS_W + ".pane-row."):
                    return Real.handle(self, verb, a)   # the Settings window's sidebar row (the screen-level stand-in keeps the pane)
                elif ax == LAYA_ROW + ".download" and m != "laya-stuck":
                    import urllib.request, urllib.error
                    files = laya_stub()[0].LAYA_FILES   # the stand-in asks the stub as the app does: HEAD each file
                    missing = []
                    for f in files:
                        try:
                            urllib.request.urlopen(urllib.request.Request("%s/%s" % (self.laya_base, f), method="HEAD"), timeout=5).close()
                        except urllib.error.HTTPError:
                            missing.append(f)
                    if missing:
                        self.laya, self.laya_why = "failed", "Could not download the decision model: %s answered 404. Click Retry." % missing[0]
                    else:
                        self.laya, self.laya_t0 = "downloading", time.time()
                elif ax == "mac-first-start.continue" and m != "ai-step-stuck":
                    self.fs_step += 1
                elif ax.startswith(INS + ".open.modify.") and ax.rsplit(".", 1)[1] in (LABEL_BOARD, "updates", "social") and m != "inspector-stuck":
                    self.inspector, self.why_on = True, False
                return {"ok": True}
            if verb == "type":
                if self.palette is not None:
                    self.palette += a[0]
                    return {"ok": True}
                if self.pending:
                    self.ptyped += a[0]
                    return {"ok": True}
                self.text += a[0]
                if self.reply_open:
                    self.rtext += a[0]
                return {"ok": True}
            if verb == "key":
                k, res = a[0], self.results()
                lr = self.list_rows() if (k == "Enter" and self.palette is not None and not self.search) else []
                if lr and lr[min(self.focus, len(lr) - 1)]["id"] == self.ATTENTION_ID and self.palette and "mark as vip".startswith(self.palette.lower()):
                    self.palette = None          # the palette closes, the engine refuses the mark (the account needs sign-in): the app answers that the key did nothing
                    return {"ok": False, "error": "the key Enter does nothing on mac-main"}
                if self.list_key(k):
                    return {"ok": True}
                if k == "Enter" and not self.search:
                    self.reply_open = True
                elif k == "R" and not self.search:
                    self.reply_open = True
                    self.reader_offset = 600.0                # R scrolls the reader to its end (main79)
                elif k == "⌘Up" and m != "reader-stays-at-end":
                    self.reader_offset = 0.0
                elif k == "⌘Enter" and self.reply_open:
                    self.sent = True                          # the slab stays open while the send counts (QuickReplyModel: phase counting is open)
                elif k == "Escape" and self.qr_from_open:
                    self.qr_from_open = False                 # Esc closes the picker first
                elif k == "Escape":
                    self.reply_open = False
                if k == "/":
                    self.search = True
                elif k == "⌘A":
                    self.text = ""
                elif k == "Enter":
                    if self.more_sel:
                        self.more_open = not self.more_open
                    else:
                        self.launched, self.sel = True, 0
                elif k == "Down" and self.launched:
                    self.sel = min(self.sel + 1, len(res))
                    self.more_sel = self.sel == len(res)
                    if m == "dies-on-related" and self.sel < len(res) and res[self.sel] == {"part": "related", "source": "attachment"}:
                        self.dying = True   # main107: the selection is read back, then the copy stops answering
                elif k == "Up" and self.launched:
                    self.sel = max(self.sel - 1, 0)
                    self.more_sel = self.sel == len(res)
                elif k == "Tab":
                    self.chips += 1
                elif k == "Escape" and "Draft" in self.text:
                    self.draft = True
                return {"ok": True}
            if verb == "rows":
                r = a[0]
                res = self.results()
                if r == REPL + ".place":
                    return {"ok": True, "rows": [{"page": self.page}]}
                if r == AGENT_CARDS:   # hid-ai round 20: the two shipped agents (mode agents-missing: no card)
                    return {"ok": True, "rows": [] if m == "agents-missing" else [{"name": "Ask my inbox", "provider": "anthropic", "model": "claude-sonnet-5-5"},
                                                                                {"name": "Writer", "provider": "anthropic", "model": "claude-sonnet-5-5"}]}
                if r == REPL + ".editors":   # hid-ai round 20/24: the prompt's and my voice's editors (RepliesEditorDoors.places), text_cut while on screen
                    return {"ok": True, "rows": [{"key": "prompt", "text_cut": m == "prompt-cut" if self.page == "prompt" else None},
                                                 {"key": "voiceText", "text_cut": m == "voice-cut" if self.page == "voice" else None}]}
                if r == WA + ".accounts":
                    rows = [{"ax": "%s.accounts.%d" % (WA, i), "address": ad, "phase": ("waiting" if not self.connected else "refused" if ad == WELCOME_BAD else "healthy")} for i, ad in enumerate(self.accts)]
                    return {"ok": True, "rows": rows}
                if r == WI + ".panel":
                    return {"ok": True, "rows": [{"showing": self.connected and self.scanned}]}
                if r == WI + ".sources":
                    return {"ok": True, "rows": [{"kind": "apple-mail", "selected": self.sel_src}]}
                if r == WI + ".progress":
                    return {"ok": True, "rows": [{"run_id": "run-1", "state": "running", "total": 18423, "done": self.done}] if self.started else []}
                if r == "mac-settings-accounts.import.sheet":
                    return {"ok": True, "rows": [{"open": self.sheet}]}
                if r == "mac-main.thread.draft":
                    return {"ok": True, "rows": [{"lines": "Draft words."}] if self.draft else []}
                if r == SB and self.search:
                    return {"ok": True, "rows": [{"active": True, "text": self.text}]}
                if r == SD and self.search and self.text and not self.launched:
                    return {"ok": True, "rows": [{"kind": "search", "title": "Search all mail"}, {"kind": "domain", "title": SEARCH_DOMAIN}]}
                if r == "mac-search.chips" and self.search:
                    return {"ok": True, "rows": [{"kind": "domain"}] * self.chips}
                if r == SH and self.search:
                    return {"ok": True, "rows": [{"related": sum(1 for x in res if x["part"] == "related"), "matches": self.matches()}]}
                if r == SSEC and self.search:
                    return {"ok": True, "rows": [{"title": "Close in meaning"}] if any(x["part"] == "related" for x in res) else []}
                if r == SREF and self.search:
                    return {"ok": True, "rows": [{"text": "The search could not run.", "words": self.refuse}] if self.refuse and m != "refuse-ignored" else []}
                if r == SNOTE and self.search:
                    return {"ok": True, "rows": [{"text": "The meaning index is still being built: 40 of 100 mails."}] if self.queued > 0 and res and m != "index-note-never" else []}
                if r == READER_SCROLL:
                    self.reader_reads += 1
                    moving = self.reader_reads * 7.0 if m == "reader-scroll-running" else 0.0   # a reader that never stops scrolling
                    return {"ok": True, "rows": [{"offset": self.reader_offset + moving, "max_offset": 600.0, "viewport": 400.0, "content": 1000.0}]}
                if r == "mac-main.quickreply.field":
                    return {"ok": True, "rows": [{"ax": r, "signature": QR_ACCOUNT.get("signature", ""), "placeholder": "Répondre à Linea Supply…", "text": ""}] if self.reply_open else []}
                if r == "mac-main.quickreply.header":
                    # the reply box folds once its words are sent and the undo window has passed: its header answers no row (owner, 13 Nov, picture 206)
                    return {"ok": True, "rows": [{"title": "Répondre à Linea Supply", "open": "1" if self.qr_header_open else "0"}] if self.reply_open and not (self.sent and self.advanced) else []}
                if r == "mac-main.quickreply.from":
                    addr = "" if m == "reply-from-empty" else QR_ACCOUNT["address"]   # main107's lone window: an empty From
                    return {"ok": True, "rows": [{"address": addr, "open": "1" if self.qr_from_open else "0"}] if self.reply_open else []}
                if r == "mac-main.thread.outgoing":
                    return {"ok": True, "rows": [{"state": "sent" if self.advanced else "waiting", "material": "flat" if self.advanced else "lifted",
                                                  "time": "23:19" if self.advanced else ""}] if self.sent else []}
                if r == "mac-main.undo.bar":
                    return {"ok": True, "rows": [{"ax": r, "seconds": 5}] if self.sent else []}
                if r == "mac-main.reader.body.links":
                    return {"ok": True, "rows": [{"text": "x"}] * (3 if self.mail == "trackinglinks" else 0)}
                if r == PLACEHOLDERS:
                    return {"ok": True, "rows": [{"mark": True}] * {"signature": 8, "newsletter": 3}.get(self.mail, 0)}
                if r == "mac-main.reader.pictures":
                    return {"ok": True, "rows": []}
                if r == LIST_ROWS:
                    return {"ok": True, "rows": self.list_rows()}
                if r == LIST_STATE:
                    return {"ok": True, "rows": [{"strip_settled": m != "strip-never", "compact": self.compact, "selected": len(self.sel_ids), "mailbox": self.box,
                                                  "pending": self.pending, "why_tags": {"on": self.inspector and self.why_on}}]}
                if r == "mac-palette.results":
                    return {"ok": True, "rows": [{"title": "Mark as VIP", "key": ""}] if (self.palette and "mark as vip".startswith(self.palette.lower())) else []}
                if r == "mac-main.reader.body":
                    return {"ok": True, "rows": [{"layout": "designed" if self.sample == "designed" else "written", "format": "plain" if self.sample == "plain" else "html"}]}
                if r == LAYA_ROW:
                    if self.laya == "downloading" and time.time() - self.laya_t0 > 0.6:
                        self.laya = "ready"
                    if self.laya == "downloading":
                        got = min(750000, int((time.time() - self.laya_t0) * 1000000) + 1)
                        return {"ok": True, "rows": [{"state": "downloading", "words": "Downloading %d kB of 750 kB" % (got // 1000), "received": got, "total": 750000, "cancel": True}]}
                    if self.laya == "ready":
                        return {"ok": True, "rows": [{"state": "ready", "words": "", "chip": "Ready", "download": False}]}
                    if self.laya == "failed":
                        return {"ok": True, "rows": [{"state": "failed", "words": self.laya_why, "retry": True, "button": "Retry", "download": True}]}
                    return {"ok": True, "rows": [{"state": "absent", "words": "Not on this Mac yet. 843 MB, downloaded once.", "button": "Download", "download": True}]}
                if r == "mac-first-start.card":
                    return {"ok": True, "rows": [{"step": self.fs_step}] if self.fs_open else []}
                if r == INS:
                    return {"ok": True, "rows": [{"open": self.inspector, "mode": "modify" if self.inspector else "closed", "preview_line": "0 picked · 0 by the AI"}]}
                if r == SR and self.search:
                    return {"ok": True, "rows": [dict(x, selected=(i == self.sel and not self.more_sel)) for i, x in enumerate(res)]}
                if r == SM and self.search and self.launched:
                    return {"ok": True, "rows": [{"selected": self.more_sel or "zzqxvk" in self.text, "open": self.more_open, "count": 9}]}
                return super().handle(verb, a)
            if verb == "labels" and getattr(self, "page", None) == "sources" and self.surface == REPL:   # round 24: the folder tiles' names (mode tile-cut: one cut)
                names = [("acc-1.folder.0.name", "Inbox"), ("acc-1.folder.2.name", "Clients"), ("acc-1.folder.3.name", "Clients / Lorvan Hotels")]
                rows = [{"ax": "%s.sources.account.%s" % (REPL, ax), "text": t, "need_w": 150.0 if (m == "tile-cut" and i == 2) else 90.0, "need_h": 16.0,
                         "box_w": 120.0, "box_h": 16.0} for i, (ax, t) in enumerate(names)]
                return {"ok": True, "labels": rows, "judged": len(rows), "marked": len(rows), "capped": False, "window": a[-1][7:]}
            if verb in ("capture", "shot"):
                args = a[1:] if verb == "capture" else a
                extra = [x for x in args[1:] if not x.startswith("window:")]
                rep = super().handle(verb, a)
                if m == "body-through-card" and rep.get("ok") and extra and getattr(self, "sample", "written") != "written":
                    from PIL import Image as _I, ImageDraw as _D
                    with _I.open(rep["path"]) as _im:
                        _im = _im.convert("RGB"); _d = _D.Draw(_im)
                        for _k in range(5):   # the mail body behind the card pasted over it: a line per row, placed by the sample
                            _y = 210 + _k * 9 + (3 if self.sample == "plain" else 0)
                            _d.rectangle([60, _y, 400 + 30 * _k, _y + 4], fill=(5, 5, 5))
                        _im.save(rep["path"])
                if extra and rep.get("ok"):
                    rep["frames"] = {x: (None if m == "no-frames" else {"x": 40.0, "y": 1500.0, "w": 600.0, "h": 75.0} if m == "row-offscreen" else {"x": 40.0, "y": 800.0, "w": 600.0, "h": 75.0} if m == "row-before-scroll" else {"x": 40.0, "y": 200.0, "w": 600.0, "h": 60.0}) for x in extra}
                return rep
            return super().handle(verb, a)
    return Driven


def selftest(outdir):
    Standin = real_standin_class()
    global RETRY_S
    RETRY_S = 0.0
    root = outdir or tempfile.mkdtemp(prefix="ss-")
    made_here = outdir is None
    checks = [0]
    bad = []

    def expect(ok, what):
        checks[0] += 1
        if not ok:
            bad.append(what)
            print("selftest FAIL: %s" % what)

    # the clipped-text rule, both sides (lead ruling main92): an ellipsis cut that leaves 6 or more characters is accepted; fewer, a cut at the bottom, a cut
    # that is not an ellipsis cut (a wrapped line) and a box of no width stay clipped
    expect(ellipsis_cut("Linea Supply", 81.0, 16.0, 73.5, 16.0), "ellipsis-cut: Linea Supply, 9 characters of 12 kept, is a normal cut")
    expect(ellipsis_cut("Northlight Desk 05", 120.0, 16.0, 96.5, 16.0), "ellipsis-cut: Northlight Desk 05, 13 characters of 18 kept, is a normal cut")
    expect(not ellipsis_cut("Orbital Desk 04", 99.0, 16.0, 30.0, 16.0), "ellipsis-cut: 3 characters kept is clipped")
    expect(not ellipsis_cut("Linea Supply", 81.0, 34.0, 73.5, 17.0), "ellipsis-cut: a wrapped line cut at the bottom is clipped")
    expect(not ellipsis_cut("Linea Supply", 81.0, 16.0, 0.0, 16.0), "ellipsis-cut: a box of no width is clipped")

    def drive(mode, tag, only=None, patch=None):
        """patch: module globals replaced for this run only (a mutant that puts back the old behaviour of one cause)."""
        d = os.path.join(root, tag)
        sd = os.path.join(root, "s%d" % abs(hash(tag) % 1000))
        si = Standin(sd, mode).start()
        c = Ctl(sd)
        ctlmod.TIMEOUT_S = 15
        g = globals()
        saved = {k: g[k] for k in (patch or {})}
        g.update(patch or {})
        try:
            c.connect()
            lines, _ = run(c, os.path.join(d, "out"), only=only, say=lambda s: None)
        finally:
            g.update(saved)
            c.close()
            si.stop()
        return lines

    quick = {"mac-main", "mac-settings-general", "mac-search", "mac-composer"}
    # the good stand-in: every state pictured, every screen's labels served by `labels`, nothing failed
    lines = drive("good", "good")
    f = sum(1 for l in lines if l.startswith("FAIL"))
    expect(len(lines) == sum(len(sizes_for(s, g)) * 2 for s, g in SCREENS), "good: one line per screen and state (%d)" % len(lines))
    expect(f == 0, "good: no FAIL line (%d)" % f)
    expect(all(l.startswith("PASS") and "labels judged 2 of 3 marked views" in l for l in lines if " mac-composer " not in l),
           "good: every state but the composer's PASS naming labels judged 2 of 3 marked views")
    expect(all("also read mac-main.sync.line" in l for l in lines if " mac-main " in l), "good: mac-main also reads its status line by `rows mac-main.sync.line`")
    comp = [l for l in lines if " mac-composer " in l]
    expect(len(comp) == 4 and all(l.startswith("NOT JUDGED mac-composer ") and "picture 1280x820 pt" in l.replace("picture 960x600", "picture 1280x820") and
                                  "labels judged 0 of 6 marked views" in l and "text null for all 6 (mac-composer.attach" in l for l in comp),
           "good: mac-composer is pictured and its labels are NOT JUDGED naming the text-null answer for all 6 marked views")
    surf = ["mac-search", "mac-palette", "mac-trainer", "mac-notch", "mac-welcome-accounts", "mac-labels-contacts", "mac-mcp-notes", "mac-quickreply"]
    expect(all(l.startswith("PASS") and "picture no picture" not in l for l in lines if any((" %s " % x) in l for x in surf)),
           "good: every surface window is pictured through the id its `winframes` row serves (the reply names surface-AppWindow-1)")
    # a need over its box by less than CLIP_TOL_PT is rounding, not a clipped label
    tl = drive("labels-tolerance", "labels-tolerance", only=quick)
    expect(tl and all(l.startswith("PASS") for l in tl if " mac-composer " not in l), "labels-tolerance: a need %.1f pt over its box stays PASS" % (CLIP_TOL_PT - 0.1))
    expect(os.path.isfile(os.path.join(root, "good", "out", "index.html")), "good: index.html written")
    page = open(os.path.join(root, "good", "out", "index.html"), encoding="utf-8").read()
    expect("http://" not in page and "https://" not in page and "<script" not in page, "good: page holds no network address and no script")
    expect(page.count("<img ") == sum(len(sizes_for(s, g)) * 2 for s, g in SCREENS), "good: one picture per state on the page")
    for s, g in SCREENS:
        expect(sum(1 for l in lines if (" %s " % s) in l) == len(sizes_for(s, g)) * 2, "good: %s has a line per state" % s)
    # mutants: (mode, expected verdict, text the expected line holds, screen)
    mutants = [("missing-picture", "FAIL", "picture file is missing", "mac-main"),
               ("one-colour", "FAIL", "one colour", "mac-main"),
               ("clipped", "FAIL", "clipped label", "mac-main"),
               ("theme-same", "FAIL", "theme", "mac-main"),
               ("size-same", "FAIL", "size did not change","mac-settings-general"),
               ("no-theme-verb", "NOT JUDGED", "theme", "mac-main"),
               ("no-resize-verb", "NOT JUDGED", "resize", "mac-settings-general"),
               ("labels-clipped", "FAIL", 'clipped label: settings.row.title "Notifications for every account" [needs 1800.0x17.0 pt, box 180.0x17.0 pt]', "mac-settings-general"),
               ("labels-wrap-clipped", "FAIL", 'settings.row.note "A long note that wraps" [needs 180.0x34.0 pt, box 180.0x17.0 pt]', "mac-settings-general"),
               ("no-labels-verb", "NOT JUDGED", "unknown verb labels", "mac-settings-general"),
               ("labels-null", "NOT JUDGED", "labels judged 0 of 1 marked views", "mac-settings-general"),
               ("labels-null", "NOT JUDGED", "labels judged 0 of 1 marked views", "mac-main"),
               ("labels-capped", "PASS", "capped: copies ran out", "mac-settings-general"),
               ("labels-bad-count", "NOT JUDGED", "counts disagree", "mac-settings-general")]
    # one mutant per cause of the first hidden run: each puts back the old behaviour and must show the NOT JUDGED line of that run
    old_id = lambda c, screen, reply_id: reply_id   # noqa: E731  the id of the `shot` reply, taken as is
    mutants += [("old-window-id", "NOT JUDGED", "no open window has the id surface-AppWindow-1; open windows (id|title): mac-main-AppWindow-1|Whoosh, mac-search|Rechercher", "mac-search"),
                ("old-status-link", "NOT JUDGED", "no rows at mac-main.status.link", "mac-main"),
                ("no-retry", "NOT JUDGED", "no open window has the id surface-AppWindow-1; open windows (id|title): mac-main-AppWindow-1|Whoosh, mac-search|Rechercher", "mac-search"),
                ("old-composer-note", "NOT JUDGED", "labels judged 0 of 6 marked views (the rest are not leaf texts)", "mac-composer")]
    patches = {"old-window-id": {"resolve_window": old_id}, "no-retry": {"RETRIES": 1},
               "old-status-link": {"FIT_REGIONS": {"mac-main": [("mac-main.status.link", FIT_REGIONS["mac-main"][0][1])]}}}
    # the identifier is SwiftUI's own name on the first read and the screen id after: the second attempt gets through
    late = drive("late-tag", "late-tag", only=quick)
    expect(late and all(l.startswith("PASS") for l in late if " mac-composer " not in l), "late-tag: a window named SwiftUI's way at first is still pictured (read again after the refusal)")
    expect(any(l.startswith("PASS mac-search ") for l in late), "late-tag: mac-search PASS")
    notred = 0
    # hid-ai round 18 (main108): the AI and agents screen is pictured on its own pane, Decisions on its; a sidebar row that does nothing is NOT JUDGED naming both
    pl = drive("good", "panes-good", only={"mac-settings-ai", "mac-settings-decisions"})
    expect(len(pl) == 8 and all(l.startswith("PASS") for l in pl), "good: mac-settings-ai and mac-settings-decisions each pictured on their pane (%s)" % [l[:160] for l in pl if not l.startswith("PASS")][:1])
    pm = drive("pane-ignored", "mut-pane-ignored", only={"mac-settings-ai"})
    red = bool(pm) and all(l.startswith("NOT JUDGED mac-settings-ai ") and "shows the pane 'decisions', not 'ai'" in l for l in pm)
    notred += 0 if red else 1
    expect(red, "mutant pane-ignored is red (NOT JUDGED mac-settings-ai naming the pane it shows): %s" % [l[:200] for l in pm][:1])
    print("selftest mutant %-16s %s" % ("pane-ignored", "red" if red else "NOT RED"))
    for mode, verdict, text, screen in mutants:
        if mode in patches:
            ml = drive("late-tag" if mode == "no-retry" else "good", "mut-" + mode, only=quick, patch=patches[mode])
            hit = [l for l in ml if l.startswith(verdict + " " + screen + " ") and text in l]
            red = bool(hit)
            notred += 0 if red else 1
            expect(red, "mutant %s is red (a %s line for %s naming %r)" % (mode, verdict, screen, text))
            print("selftest mutant %-16s %s" % (mode, "red" if red else "NOT RED"))
            continue
        if mode == "old-composer-note":   # the old note held no word of the cause: the good run's composer line must hold more than it
            ml = drive("good", "mut-" + mode, only=quick)
            red = any(l.startswith("NOT JUDGED mac-composer ") and text in l and "text null for all" in l for l in ml)
            notred += 0 if red else 1
            expect(red, "mutant %s is red (the composer line says more than the old note %r)" % (mode, text))
            print("selftest mutant %-16s %s" % (mode, "red" if red else "NOT RED"))
            continue
        ml = drive(mode, "mut-" + mode, only=quick)
        if mode == "labels-null":   # a text:null row whose need exceeds its box is not counted, so never a FAIL
            expect(not any(l.startswith("FAIL") for l in ml), "labels-null: a row with text null is not counted as clipped")
        if mode == "no-labels-verb":   # an older build keeps mac-main's status-line judgement, as before the verb
            expect(all(l.startswith("PASS") for l in ml if " mac-main " in l), "no-labels-verb: mac-main still PASS by `rows`")
        hit = [l for l in ml if l.startswith(verdict + " " + screen + " ") and text in l]
        red = bool(hit)
        if not red:
            notred += 1
        expect(red, "mutant %s is red (a %s line for %s naming %r)" % (mode, verdict, screen, text))
        print("selftest mutant %-16s %s" % (mode, "red" if red else "NOT RED"))

    # ---- the named states (lane gap-shots): every state pictured on the stand-in, each mutant seen red
    Driven = driven_standin_class()
    global POLL_S, POLL_EVERY, SETTLE_CAP, WAIT_CAP, PICK_WAIT_S
    POLL_S, POLL_EVERY, SETTLE_CAP, WAIT_CAP = 0.3, 0.02, 0.0, 2.0   # the stand-in's download ends in 0.6 s
    PICK_WAIT_S = 0.3   # the stand-in answers at once: the 6 s a real query may take made every stand-in run of the related-file state wait 18 s per picture (16 Nov: the self-test passed 300 s)

    def drive_states(mode, tag, wanted=None):
        d = os.path.join(root, tag)
        sd = os.path.join(root, "t%d" % abs(hash(tag) % 1000))
        si = Driven(sd, mode).start()
        c = Ctl(sd)
        ctlmod.TIMEOUT_S = 15
        try:
            c.connect()
            lines, rows = run_states(c, os.path.join(d, "out"), wanted, say=lambda x: None)
        finally:
            c.close()
            si.stop()
        return lines, si.log, os.path.join(d, "out")

    slines, slog, sout = drive_states("good", "states-good")
    want_n = sum(len(sd["views"]) * len(sd["sizes"]) * 2 for sd in STATES)
    expect(len(slines) == want_n, "states good: one line per view, size and theme (%d of %d)" % (len(slines), want_n))
    expect(all(l.startswith("PASS") for l in slines), "states good: every line PASS (%s)" % [l[:420] for l in slines if not l.startswith("PASS")][:2])
    for sd in STATES:
        n = sum(1 for l in slines if (" %s/" % sd["id"]) in l)
        expect(n == len(sd["views"]) * len(sd["sizes"]) * 2, "states good: %s has a line per view, size and theme (%d)" % (sd["id"], n))
        print("selftest state %-32s item %s  %d pictures" % (sd["id"], sd["item"], n))
    expect(all(os.path.isfile(os.path.join(sout, "pics", "%s.png" % pic_name(sd, v, t, w, h)))
               for sd in STATES for v, _, _ in sd["views"] for _, w, h in sd["sizes"] for t in THEMES), "states good: every picture file listed by --list-states is written")
    expect(all(os.path.isfile(os.path.join(sout, "pics", "%s-closeup.png" % pic_name(sd, v, t, w, h)))
               for sd in STATES for v, _, cr in sd["views"] if cr for _, w, h in sd["sizes"] for t in THEMES), "states good: the close-up of the refused row is cut and written")
    sent = [(v, a) for v, a in slog]
    expect(any(v == "scroll" and a[0] == "end" and a[1].startswith("window:") for v, a in sent), "states good: `scroll end window:<id>` was sent")
    expect(any(v == "scroll" and a[0] == "top" for v, a in sent), "states good: `scroll top` was sent")
    expect(any(v == "click" and a[0] == WA + ".connect-waiting" for v, a in sent), "states good: the Welcome refused-row state connected its rows before the scroll to the end")
    expect(("click", [REPL + ".page.sources"]) in sent and ("click", [REPL + ".page.prompt"]) in sent, "states good: the Replies panes are reached by their page doors")
    expect(("fixture", ["import-progress", "run-1", "9211"]) in sent, "states good: the import run is put at half of its total (9211 of 18423)")
    expect(("key", ["Tab"]) in sent and ("fixture", ["corpus", CORPUS_FILE]) in sent, "states good: the search pill by Tab and the corpus are sent")
    expect(("fixture", ["import-sources", "apple-mail=18423"]) in sent, "states good: the import sources are set by the fixture verb")
    expect(sum(1 for v, a in sent if v == "key" and a == ["Down"]) >= 2, "states good: Down moves to the related mail and to the closed line")
    expect(any(v == "fixture" and a[:1] == ["search-refuse"] and len(a) > 1 for v, a in sent), "states good: the engine is told to refuse the search by `fixture search-refuse <words>`")
    expect(("fixture", ["index-progress", "40", "60"]) in sent, "states good: the index builder is put at 40 indexed, 60 queued by `fixture index-progress 40 60`")
    expect(("click", [INS + ".open.modify." + LABEL_BOARD]) in sent, "states good: the label inspector is opened by its door on the built-in label")
    combos = sum(len(sd["sizes"]) * 2 for sd in STATES)
    expect(sum(1 for v, a in sent if v == "fixture" and a == ["index-progress", "0", "0"]) == combos, "states good: the index figures are cleared once at the start of each of the %d combos" % combos)
    for sid in ("search-bold-letters", "search-letters-inside-a-longer-word", "search-count-line-and-related-group", "search-could-not-run", "search-index-line", "label-inspector-open"):
        expect(sid in STATE_IDS, "state %s is listed" % sid)
    shown = []
    for sd in STATES:
        list_states(say=shown.append)
    expect(all(any(("state %s " % x) in l for l in shown) for x in STATE_IDS), "--list-states names every state")
    smut = [("page-stuck", "replies-panes", "NOT JUDGED", REPL + ".place"), ("no-scroll-verb", "replies-filter-page-scroll", "NOT JUDGED", "unknown verb scroll"),
            ("scroll-short", "replies-filter-page-scroll", "NOT JUDGED", "left the page at 'middle'"), ("click-refused", "welcome-refused-row", "NOT JUDGED", "nothing to click"),
            ("no-frames", "welcome-refused-row", "NOT JUDGED", "answered no frame"), ("row-offscreen", "welcome-refused-row", "FAIL", "not inside the window"), ("row-before-scroll", "welcome-refused-row", "PASS", "read as y"), ("no-related", "search-after-enter", "NOT JUDGED", "mac-search.header"),
            ("no-related", "search-related-from-attachment", "NOT JUDGED", "no result holding"), ("page-fits", "replies-filter-page-scroll", "PASS", "nothing to scroll"),
            # lane fix-rest-app: one broken model per new state (a build that does not reach the state is NOT JUDGED naming the row or verb it missed)
            ("no-matches", "search-bold-letters", "NOT JUDGED", SH), ("no-matches", "search-s1-after-enter", "NOT JUDGED", SH), ("no-matches", "search-letters-inside-a-longer-word", "NOT JUDGED", SH),
            ("no-related", "search-count-line-and-related-group", "NOT JUDGED", SH), ("refuse-ignored", "search-could-not-run", "NOT JUDGED", SREF),
            ("no-index-verb", "search-index-line", "NOT JUDGED", "index-progress"), ("index-note-never", "search-index-line", "NOT JUDGED", SNOTE),
            ("inspector-stuck", "label-inspector-open", "NOT JUDGED", INS), ("inspector-stuck", "label-ai-needs-apple", "NOT JUDGED", INS), ("inspector-stuck", "label-ai-under-trust", "NOT JUDGED", INS),
            ("inspector-stuck", "label-ai-new-install-no-model", "NOT JUDGED", INS), ("sample-dead", "reader-designed-and-plain-sample", "NOT JUDGED", "layout"),
            ("ai-step-stuck", "welcome-ai-step", "NOT JUDGED", "step"), ("body-through-card", "welcome-ai-step", "FAIL", "mail ink inside the card"), ("click-refused", "label-ai-no-model-preview", "NOT JUDGED", "nothing to click"), ("click-refused", "label-inspector-open", "NOT JUDGED", "nothing to click"),
            # item 0b095f58: a fixture with no thread row, no single row or no row of the owner's own last reply is NOT JUDGED naming the field
            ("no-pill-row", "list-row-thread-pill", "NOT JUDGED", "thread_count"), ("no-single-row", "list-row-single-message", "NOT JUDGED", "thread_count"),
            ("no-reply-row", "list-row-reply-arrow", "NOT JUDGED", "replied_last"),
            # item 0b095f58, the other states of the board: each broken copy lacks the one thing the state's last wait reads, and the tile names the field
            ("no-read-single", "list-row-single-read", "NOT JUDGED", "unread = false"), ("u-dead", "list-row-thread-read", "NOT JUDGED", "unread = false"),
            ("strip-never", "list-row-thread-focused", "NOT JUDGED", "strip_settled"), ("x-dead", "list-row-selected", "NOT JUDGED", "selected"),
            ("no-pinned", "list-row-pinned", "NOT JUDGED", "pinned = true"), ("s-dead", "list-row-starred", "NOT JUDGED", "starred = true"),
            ("no-attachment", "list-row-attachment", "NOT JUDGED", "attachments"), ("draft-lost", "list-row-thread-draft", "NOT JUDGED", "draft = true"),
            ("why-dead", "list-row-why-tag", "NOT JUDGED", "why_tag"), ("compact-dead", "list-row-compact", "NOT JUDGED", "compact"),
            # item 637ae165: the quick reply in its thread: an empty From (main107's lone window) and a dead From button are NOT JUDGED naming the row
            ("reply-from-empty", "quickreply-in-thread", "NOT JUDGED", "address"), ("reply-picker-dead", "quickreply-in-thread", "NOT JUDGED", "mac-main.quickreply.from"),
            # round 19: a reader still scrolling, and a ⌘Up that leaves the thread at its end, are NOT JUDGED naming the scroll row
            ("reader-scroll-running", "quickreply-in-thread", "NOT JUDGED", "never held still"), ("reader-stays-at-end", "quickreply-in-thread", "NOT JUDGED", "left the page at 'end'"),
            # round 18 (main107): the copy dies once the related mail from an attachment is selected: the tile names the step, never only the pipe
            ("dies-on-related", "search-related-from-attachment", "NOT JUDGED", "then the hidden copy stopped answering"),
            # hid-ai round 18 (item 0aea8b37): a Download that starts nothing leaves the row absent: NOT JUDGED naming the row
            ("laya-stuck", "decision-model-download", "NOT JUDGED", LAYA_ROW),
            # hid-ai round 20 (main111 d58d8ad5 p8): a prompt card that cuts its text is NOT JUDGED naming the row's text_cut
            ("prompt-cut", "replies-panes", "FAIL", "text_cut"),
            # hid-ai round 24 (main114 d58d8ad5 p8): a cut tile name and a cut voice text are FAILs naming them
            ("tile-cut", "replies-panes", "FAIL", "Clients / Lorvan Hotels"), ("voice-cut", "replies-panes", "FAIL", "voiceText"),
            ("agents-missing", "settings-ai-agent-cards", "NOT JUDGED", AGENT_CARDS),
            # 14 Nov (main182): a first-start card left open by an earlier state covers this state's screen: NOT JUDGED naming the card, never a pass
            ("first-start-stays-open", "list-row-thread-pill", "NOT JUDGED", "first-start card is on the screen"),
            # lane row-marks: one broken copy per new state, each lacking the one thing the state's reach reads, and the tile names it
            ("rest-dead", "list-row-hovered", "NOT JUDGED", "rested on no list row"), ("snooze-words-null", "list-row-snoozed", "NOT JUDGED", "snoozed = set"),
            ("muted-field-false", "list-row-muted", "NOT JUDGED", "muted = true"), ("vip-field-false", "list-row-vip", "NOT JUDGED", "'vip': True")]
    for mode, sid, verdict, text in smut:
        ml, _, _ = drive_states(mode, "states-" + mode + "-" + sid, {sid})
        hit = [l for l in ml if l.startswith(verdict + " " + sid + "/") and text in l]
        red = bool(hit)
        notred += 0 if red else 1
        expect(red, "states mutant %s is red (a %s line for %s naming %r)" % (mode, verdict, sid, text))
        print("selftest state mutant %-14s %-32s %s" % (mode, sid, "red" if red else "NOT RED"))
    # lane row-marks: the two reaches main193 never met (shots-rows.log:49-56), put back on the stand-in that models the app's toggles and the read mark of an opened
    # thread: each must be NOT JUDGED with main193's own words (the draft in every picture, the why tag in the dark ones)
    vip_now = next(x for x in STATES if x["id"] == "list-row-vip")["reach"]
    OLD_VIP_REACH = [(st[0], st[1], R_VIP_THREAD_OLD, st[3]) if (st[0] == "click_row" and st[2] is R_VIP_THREAD) else st for st in vip_now]
    expect(OLD_VIP_REACH != vip_now, "the old VIP reach differs from today's")
    olds = [("list-row-vip", OLD_VIP_REACH, None, "the key Enter does nothing on mac-main", None),
            ("list-row-thread-draft", OLD_DRAFT_REACH, None, "thread_count = 3 and unread = true and pinned = false and draft = true and focused = false: not reached", None),
            ("list-row-why-tag", OLD_WHY_REACH, OLD_WHY_VIEWS, "rows mac-labels.inspector open should be True: not reached", "dark")]
    for sid, old, old_views, text, theme in olds:
        sd_old = next(x for x in STATES if x["id"] == sid)
        keep = (sd_old["reach"], sd_old["views"])
        sd_old["reach"], sd_old["views"] = old, old_views or sd_old["views"]
        try:
            ml, _, _ = drive_states("good", "states-old-reach-" + sid, {sid})
        finally:
            sd_old["reach"], sd_old["views"] = keep
        hit = [l for l in ml if l.startswith("NOT JUDGED " + sid + "/") and text in l and (theme is None or (" %s " % theme) in l)]
        red = bool(hit) and (theme is not None or all(l.startswith("NOT JUDGED") for l in ml))
        notred += 0 if red else 1
        expect(red, "states old reach of %s is red (NOT JUDGED naming %r%s): %s" % (sid, text, "" if theme is None else " in the %s pictures" % theme, [l[:300] for l in ml][:2]))
        print("selftest state mutant %-14s %-32s %s" % ("old-reach", sid, "red" if red else "NOT RED"))
    # the pick's way before 16 Nov (a query with no result ended it): on the stand-in whose first related-file queries list nothing, the state is not reached
    global PICK_STOPS_AT_NO_ROWS
    PICK_STOPS_AT_NO_ROWS = True
    try:
        pl, _, _ = drive_states("good", "states-old-pick", {"search-related-from-attachment"})
    finally:
        PICK_STOPS_AT_NO_ROWS = False
    red = bool(pl) and all(l.startswith("NOT JUDGED") and "not reached" in l for l in pl)
    notred += 0 if red else 1
    expect(red, "states old pick of search-related-from-attachment is red (NOT JUDGED, not reached): %s" % [l[:200] for l in pl][:1])
    print("selftest state mutant %-14s %-32s %s" % ("old-pick", "search-related-from-attachment", "red" if red else "NOT RED"))
    # the state after a dead copy says where the copy died, not "fixture reset failed: Broken pipe"
    dl, _, _ = drive_states("dies-on-related", "states-dies-two", {"search-related-from-attachment", "search-nothing-matches"})
    after = [l for l in dl if " search-nothing-matches/" in l]
    expect(len(after) == 4 and all(l.startswith("NOT JUDGED") and "stopped answering during state search-related-from-attachment after its step" in l
                                    and "Broken pipe" not in l.split("(")[0] for l in after),
           "states dies-on-related: each later tile names the state and step after which the copy stopped answering (%s)" % [l[:300] for l in after][:1])
    replies = iter([{"source": "render (web: not ready after 3220 ms)"}, {"source": "render (1 web view drawn)"}])
    expect(shot_when_ready(lambda: next(replies), wait=0) == ({"source": "render (1 web view drawn)"}, 2), "a page ready at the second picture is taken twice and the ready one is kept")
    expect(shot_when_ready(lambda: {"source": "render (web: not ready after 3220 ms)"}, wait=0)[1] == READY_TRIES, "a page that is never ready is pictured READY_TRIES times and stays not ready (the caller's NOT JUDGED)")
    expect(shot_when_ready(lambda: {"source": "render (1 web view drawn)"}, wait=0)[1] == 1, "a ready page is pictured once")
    expect(all(l.startswith("PASS") for l in drive_states("good", "states-good-two", {"search-related-from-attachment", "search-nothing-matches"})[0]),
           "states good: the two states after the change still PASS")
    POLL_S, POLL_EVERY, SETTLE_CAP, WAIT_CAP = 6.0, 0.15, None, None
    PICK_WAIT_S = 6.0
    print("selftest: %d state mutants" % len(smut))
    print("selftest: %d checks, %d mutants, %d not red" % (checks[0], len(mutants) + len(smut) + len(olds) + 1, notred))
    if made_here:
        shutil.rmtree(root, ignore_errors=True)
    return 0 if not bad and notred == 0 else 1


def main(argv):
    args = argv[1:]
    only, deadline, out, st, states, ls = None, 540.0, None, False, None, False
    i = 0
    while i < len(args):
        a = args[i]
        if a == "--selftest":
            st = True
        elif a == "--screens":
            i += 1
            only = set(args[i].split(","))
        elif a == "--deadline":
            i += 1
            deadline = float(args[i])
        elif a == "--states":
            i += 1
            states = set() if args[i] == "all" else set(args[i].split(","))
        elif a == "--list-states":
            ls = True
        elif a.startswith("-"):
            print("unknown option %s" % a)
            return 2
        else:
            out = a
        i += 1
    if st:
        return selftest(out)
    if ls:
        list_states()
        return 0
    if states is not None:
        unknown = sorted(states - set(STATE_IDS))
        if unknown or not out:
            print("usage: python3 checks/shots-sheet.py <outdir> --states all|id1,id2 [--deadline S]; unknown: %s; states: %s" % (", ".join(unknown) or "none", ", ".join(STATE_IDS)))
            return 2
    if not out:
        print("usage: python3 checks/shots-sheet.py <outdir> [--screens a,b | --states all|id1,id2] [--deadline S] | --list-states | --selftest")
        return 2
    out = os.path.abspath(out)
    ctlmod.TIMEOUT_S = 30
    c = Ctl()
    try:
        c.connect()
    except CouldNotDrive as e:
        lines = []
        os.makedirs(out, exist_ok=True)
        rows = {}
        if states is not None:
            class Dead:
                def send(self, *a):
                    raise CouldNotDrive(str(e))
            lines, _ = run_states(Dead(), out, states, deadline_s=1500.0)
            summarize(lines)
            return 1
        for screen, fname in SCREENS:
            if only and screen not in only:
                continue
            sts = [new_state(screen, t, lab, w, h) for lab, w, h in sizes_for(screen, fname) for t in THEMES]
            for s in sts:
                judge(s, "NOT JUDGED", "no socket: %s" % e)
                lines.append(line_for(s))
                print(lines[-1])
            rows[screen] = sts
        write_page(out, rows, {}, ["The app could not be driven: %s" % e])
        summarize(lines)
        return 1
    try:
        if states is not None:
            lines, _ = run_states(c, out, states, deadline_s=max(deadline, 1500.0) if deadline == 540.0 else deadline)
        else:
            lines, _ = run(c, out, only=only, deadline_s=deadline)
    finally:
        c.close()
    f, u = summarize(lines)
    print("page: %s" % os.path.join(out, "index.html"))
    return 1 if (f or u) else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
