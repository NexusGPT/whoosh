#!/usr/bin/env python3
"""Emblem render check (lane emblem). Renders the library at 56, 20 and 16 pt in light and dark on the fixture colours
(Tests/Unit/EmblemCaptureTests.swift writes _captures/emblem), then asserts from the recipe dump AND the pixels:
no glyph clipped; the 16 pt mark has no specular and no inner shadow; the list-row mark uses the account colour at
EMBLEM_LIST_SATURATION; the amber account takes the dark ink; a row's title out-contrasts the mark. Also shoots the
side-by-side (16 fixture rows: emblem beside the tile board's own badge(), light and dark).
Run: python3 checks/emblem-check.py [--no-render]. Last line 'N checks, F failed'; exit 1 when F > 0.
Self-test (no render, no capture): python3 checks/emblem-check.py --selftest
MUTANT seen red: specular left in at 16 pt (EmblemRecipe: `specular = t != .simplified ? ...`) turns C3 and C4 red.
"""
import json, os, subprocess, sys
from PIL import Image

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CAP = os.path.join(ROOT, "_captures", "emblem")
def _tile():
    d = ROOT
    while d != "/":
        p = os.path.join(d, "docs/design/boards/badges/tile/index.html")
        if os.path.exists(p): return p
        d = os.path.dirname(d)
    return ""
TILE = os.environ.get("EMBLEM_TILE_BOARD") or _tile()  # the work home's tile board, found walking up from the repo
BG = {"light": (255, 255, 255), "dark": (0x1E, 0x1E, 0x20)}
TITLE = {"light": (0x1D, 0x1D, 0x1F), "dark": (0xF2, 0xF2, 0xF5)}
results = []

def check(name, ok, detail=""):
    results.append(ok); print(("PASS " if ok else "FAIL ") + name + (": " + detail if detail else ""))

def hx(h): h = h.lstrip("#"); return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))
def mix(a, b, t): return tuple(a[i] + (b[i] - a[i]) * t for i in range(3))
def lum(c):
    f = lambda v: v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4
    r, g, b = (f(x / 255) for x in c[:3]); return 0.2126 * r + 0.7152 * g + 0.0722 * b
def contrast(a, b): x, y = lum(a), lum(b); return (max(x, y) + 0.05) / (min(x, y) + 0.05)
def dist(a, b): return max(abs(a[i] - b[i]) for i in range(3))

def row_mark_report(samples):
    """samples: [(name, scheme, pixel, listColour, rawColour)] -> (marks closer to the list colour, the ones that are not, with the three colours and both distances)."""
    closer, off = 0, []
    for name, scheme, p, lst, raw in samples:
        if dist(p, lst) <= dist(p, raw):
            closer += 1
        else:
            off.append("%s/%s pixel %s list %s raw %s (distance to list %.0f, to raw %.0f)" % (name, scheme, tuple(p), tuple(round(v) for v in lst), tuple(round(v) for v in raw), dist(p, lst), dist(p, raw)))
    return closer, off

if "--selftest" in sys.argv:
    good = [("a", "light", (100, 100, 100), (100, 100, 100), (200, 50, 50)), ("b", "dark", (90, 90, 90), (95, 95, 95), (200, 50, 50))]
    bad = good + [("c", "light", (200, 50, 50), (100, 100, 100), (200, 50, 50))]          # the mark carries the raw colour: not desaturated
    c1, o1 = row_mark_report(good); c2, o2 = row_mark_report(bad)
    ok = (c1, o1) == (2, []) and c2 == 2 and len(o2) == 1 and "c/light" in o2[0] and "distance to list" in o2[0]
    print("%s row marks: two desaturated marks pass; a mark with the raw colour is named with its colours and distances (%s)" % ("PASS" if ok else "FAIL", o2))
    print("1 checks, %d failed" % (0 if ok else 1))
    sys.exit(0 if ok else 1)

if "--no-render" not in sys.argv:
    env = dict(os.environ, WHOOSH_EMBLEM_CAPTURE=CAP)
    r = subprocess.run(["swift", "test", "--filter", "EmblemCaptureTests", "-j", "2"], cwd=ROOT, env=env, capture_output=True, text=True)
    check("render ran", r.returncode == 0, "exit %d %s" % (r.returncode, r.stdout[-400:] if r.returncode else ""))

doc = json.load(open(os.path.join(CAP, "render.json")))
S = doc["scale"]
check("C1 library size", doc["glyphCount"] >= doc["ICON_LIBRARY_MIN"], "%d glyphs, minimum %d" % (doc["glyphCount"], doc["ICON_LIBRARY_MIN"]))
clipped = [(c["glyph"], c["size"]) for c in doc["cells"] if not c["clipFree"]]
check("C2 no glyph clipped", not clipped, "%d of %d cells clipped %s" % (len(clipped), len(doc["cells"]), clipped[:5]))
tiny = [c for c in doc["cells"] if c["size"] == 16]
check("C3 16 pt recipe has no specular, no inner shadow", all(c["specular"] is None and c["innerShadow"] is None for c in tiny),
      "%d of %d cells carry a light layer" % (sum(c["specular"] is not None or c["innerShadow"] is not None for c in tiny), len(tiny)))

def probe(cell):
    """Pixel at the top of the disc, above the glyph box, against the bare base gradient there."""
    im = SHEETS[(cell["scheme"], cell["size"])]; d = cell["size"]
    p = im.getpixel((int((cell["x"] + d / 2) * S), int((cell["y"] + 0.10 * d) * S)))
    return dist(p, mix(hx(cell["baseTop"]), hx(cell["baseBottom"]), 0.10))
SHEETS = {(s, z): Image.open(os.path.join(CAP, "library-%s-%d.png" % (s, z))).convert("RGB") for s in ("light", "dark") for z in (56, 20, 16)}
dev16 = [probe(c) for c in tiny]
dev56 = [probe(c) for c in doc["cells"] if c["size"] == 56]
check("C4 16 pt pixels show no specular", max(dev16) <= 18, "worst deviation from the bare base %.0f/255 (limit 18)" % max(dev16))
check("C4b the probe sees a specular where one is drawn (56 pt)", min(dev56) > 18, "least 56 pt deviation %.0f/255" % min(dev56))

k = doc["EMBLEM_LIST_SATURATION"]
bad = [r["name"] for r in doc["rows"] if abs(r["listSaturation"] - r["rawSaturation"] * k) > 0.01]
check("C5 list-row colour at EMBLEM_LIST_SATURATION (recipe)", not bad, "k=%.2f, off: %s" % (k, bad))
samples = []
for r in doc["rows"]:
    im = Image.open(os.path.join(CAP, "rows", "%s-%s.png" % (r["account"], r["scheme"]))).convert("RGB")
    p = im.getpixel((im.width // 2, int(im.height * 0.10)))
    lst = mix(hx(r["list"]["baseTop"]), hx(r["list"]["baseBottom"]), 0.10); raw = mix(hx(r["raw"]["baseTop"]), hx(r["raw"]["baseBottom"]), 0.10)
    samples.append((r["name"], r["scheme"], p, lst, raw))
closer, off = row_mark_report(samples)
check("C5b list-row pixels match the desaturated colour", closer == len(doc["rows"]), "%d of %d row marks%s" % (closer, len(doc["rows"]), "; not closer to the list colour: " + " | ".join(off) if off else ""))

amber = [r for r in doc["rows"] if r["name"] == "Nexus billing"]
check("C6 amber account takes the dark ink (recipe)", bool(amber) and all(r["list"]["darkInk"] and r["raw"]["darkInk"] for r in amber))
cell = next(c for c in doc["cells"] if c["size"] == 56 and c["scheme"] == "light" and c["colour"] == "#F59F00")
im = SHEETS[("light", 56)]
box = [im.getpixel((int((cell["x"] + x) * S), int((cell["y"] + y) * S))) for x in range(18, 38) for y in range(18, 38)]
check("C6b amber pixels carry dark ink", min(lum(p) for p in box) < 0.15, "darkest glyph pixel luminance %.3f" % min(lum(p) for p in box))

for scheme in ("light", "dark"):
    means = []
    for r in [r for r in doc["rows"] if r["scheme"] == scheme]:
        im = Image.open(os.path.join(CAP, "rows", "%s-%s.png" % (r["account"], scheme))).convert("RGBA")
        px = [p for p in im.getdata() if p[3] > 127]
        comp = [mix(BG[scheme], p[:3], p[3] / 255) for p in px]
        means.append((sum(contrast(c, BG[scheme]) for c in comp) / len(comp), r["name"]))
    title = contrast(TITLE[scheme], BG[scheme]); top = max(means)
    check("C7 %s: row title out-contrasts every mark" % scheme, title > top[0],
          "title %.2f:1; mark mean %.2f:1 (highest %s %.2f:1, lowest %.2f:1)" % (title, sum(m for m, _ in means) / len(means), top[1], top[0], min(means)[0]))

def side_by_side():
    fx = json.load(open(os.path.join(ROOT, "docs/design/fixtures/mac-main-sample.json")))
    rows = [{"a": m["account"], "s": m["sender"], "t": m["time"], "j": m["subject"], "u": m.get("unread", False)} for m in fx["messages"]]
    html = open(TILE).read()
    script = """<script>window.addEventListener('load',()=>{const R=%s;const defs=document.getElementById('defs').closest('svg').outerHTML;
const col=(th,emb)=>`<div class="col"><div class="h">${emb?'Emblem (this lane)':'Tile board badge'} · ${th}</div>`+R.map(r=>`<div class="rw"><span class="ud">${r.u?'<i></i>':''}</span><span class="bg">${emb?`<img src="rows/${r.a}-${th}.png" width="16" height="16">`:badge(A(r.a),16,{theme:th})}</span><div class="tx"><div class="l1"><b>${r.s}</b><span>${r.t}</span></div><div class="l2">${r.j}</div></div></div>`).join('')+'</div>';
document.body.innerHTML=defs+['light','dark'].map(th=>`<div class="pane ${th}">${col(th,true)}${col(th,false)}</div>`).join('');
const st=document.createElement('style');st.textContent=`body{margin:0;width:1000px;background:#E9E9EC;font-family:"Helvetica Neue",Helvetica,Arial,sans-serif}.pane{display:flex;gap:24px;padding:16px}.light{background:#fff;color:#1D1D1F}.dark{background:#1E1E20;color:#F2F2F5}.col{width:460px}.h{font:600 11px/24px "Helvetica Neue",Helvetica;opacity:.6}.rw{display:flex;align-items:center;height:44px;border-bottom:.5px solid rgba(128,128,128,.25)}.ud{width:20px;display:flex;justify-content:center}.ud i{width:8px;height:8px;border-radius:4px;background:#007AFF}.bg{width:24px;display:flex}.tx{flex:1;min-width:0}.l1{display:flex;justify-content:space-between;font:400 13px/16px "Helvetica Neue",Helvetica}.l1 span{font-size:12px;opacity:.6}.l2{font:400 12px/16px "Helvetica Neue",Helvetica;opacity:.6;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}`;document.head.appendChild(st);});</script>""" % json.dumps(rows)
    out = os.path.join(CAP, "side-by-side.html")
    open(out, "w").write(html.replace("</body>", script + "</body>"))
    # C8 used to render this page with a headless Chrome. Owner 2026-11-10 22:13: "don't do any test on chrome" (headless included).
    # No browser is launched here any more: the page is written for a reader, and the shot is NOT JUDGED until an app-side render exists.
    print("NOT JUDGED C8 side-by-side shot (16 rows, light and dark): no test runs in Chrome (owner's rule); the page is at " + out)

side_by_side()
print("%d checks, %d failed" % (len(results), results.count(False)))
sys.exit(1 if False in results else 0)
