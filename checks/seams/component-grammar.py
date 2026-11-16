#!/usr/bin/env python3
"""Seam vis-components/component-grammar: one control keeps one height and one corner radius on every surface
that draws it (docs/lanes/components.md:18). Controls: the code pill (reader vs notch), the label chip (list
row vs reader meta), the key hint (foot hints vs palette) and the account mark (sidebar vs reader meta).

Expected numbers come from the app's own design numbers, never typed here: the account-mark sizes from
docs/CONSTANTS.md (ACCOUNT_RING_MIN_PX, ACCOUNT_MARK_ROW_PX); the chip, pill and key-cap height and radius from
Sources/Design/WhooshSpacing.swift (keyCap) and WhooshRadius.swift (item, keyCap), because docs/DESIGN.md and
docs/CONSTANTS.md hold no chip, pill or key-cap number (finding in docs/lanes/gap-seams.md).

Reads: `measure <ax-id>` -> frame.h; the corner radius from the same reply's `radius` field. A view says its radius with
`whooshAx(id, radius:)` (C1, built); a view that does not reads NOT JUDGED on its radius, never PASS. A surface whose
control registers no measurable id is NOT JUDGED with the reason named. Heights that differ are FAIL with both
values, whatever the radius says. Prints one line per control: PASS|FAIL|NOT JUDGED <control> <detail>."""
import sys, os, re
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from seamlib import *  # noqa: E402,F401,F403
from seamlib import NJ, nj, constant_int, run, ROOT  # noqa: E402


def token(file, name):
    """`public static let <name>: CGFloat = <n>` in Sources/Design/<file>."""
    with open(os.path.join(ROOT, "Sources/Design", file), encoding="utf-8") as f:
        m = re.search(r"static let %s: CGFloat = (\d+(?:\.\d+)?)" % re.escape(name), f.read())
    if not m:
        raise KeyError("Sources/Design/%s has no CGFloat %s" % (file, name))
    v = float(m.group(1))
    return int(v) if v == int(v) else v


NO_RADIUS = "the measure reply has no `radius`: the marked view does not say its radius (`whooshAx(id, radius:)`, FrameProbe.swift; built for the notch code pill)"


def expected():
    cap, item, kcr = token("WhooshSpacing.swift", "keyCap"), token("WhooshRadius.swift", "item"), token("WhooshRadius.swift", "keyCap")
    tile, field = token("WhooshSpacing.swift", "fileTile"), token("WhooshRadius.swift", "field")
    ring, row = constant_int("ACCOUNT_RING_MIN_PX"), constant_int("ACCOUNT_MARK_ROW_PX")
    # (control, [(surface, ax id or None, why not measurable, expected height, expected radius)])
    return {"cases": [
        # the reader's code pill keeps its drawn look (lead ruling 2026-11-12): its own component, judged against its own row (ledger rows proposed in docs/lanes/hid-keys.md)
        ("reader-code-pill", [("reader-codepill", "mac-main.reader.codepill", "", tile, field)]),
        ("code-pill", [("notch-codepill", "mac-notch.codepill", "", cap, item)]),
        # C3 (hid-keys, main95): each id is one line in the file that draws the control (docs/lanes/hid-keys.md lists them). LabelChip is drawn only in the component
        # gallery (no list row and no reader meta draws one), so the gallery is its surface; the key hint is judged on its CAP (20 pt, 5 pt radius), the hint around it holds its words.
        ("label-chip", [("gallery", "mac-components.gallery.label-chip", "", cap, item)]),
        ("key-hint", [("foot-hints", "mac-main.foot.cap.archive", "", cap, kcr),
                      ("gallery", "mac-components.gallery.key-hint.cap", "", cap, kcr)]),
        ("account-mark", [("sidebar-ring", "SIDEBAR", "", ring, ring / 2),
                          ("reader-meta", "mac-main.reader.account-mark", "", row, row / 2)]),
    ]}


def put_on_screen(c):
    """Bring the two code pills on screen through the doors the notch check uses (par-notch.py J6.12/J6.42): the notch's stage drops the sample mails
    (one carries the code 482915, id msg-j6-code) into the notch and the inbox; the code mail's row (found by its id in `mac-main.list`, not by a position)
    opens in the reader. Every step is best effort: a door the app refuses leaves the pill off screen, and the measure below then says so (NOT JUDGED with
    the app's words), never a pass. (main99: the row was taken by position 1 and the reader pill was never on screen.)"""
    c.call("click", "mac-main.accounts.all")
    # The reader pill needs the fixture's sign-in mail, msg-09 (code 482913), the one par-reader.py opens (CODE_ROW): it is a row of the Mac list. The sample mail
    # of the notch stage (msg-j6-code) is delivered to the notch, never a row of `mac-main.list` (main103: the lookup by that id found nothing and the reader stayed
    # shut: "11 accessibility elements walked"). Focus goes to row 0, then J moves down to msg-09 as par-reader does, and the reader opens on it.
    rows = c.call("rows", "mac-main.list").get("rows") or []
    idx = next((i for i, r in enumerate(rows) if r.get("id") == "msg-09"), None)
    if idx is None:
        return
    for verb, ax in (("click", "mac-main.pane.list"), ("click", "mac-main.list.row.0")):
        c.call(verb, ax)
    for _ in range(idx):
        c.call("key", "J")
    c.call("click", "mac-main.reader.open")
    # `measure` mounts its readers only for views built after it switched on (WhooshAxModifier reads the gate when its body runs). Reopening the SAME mail did not rebuild
    # the reader's header (main104/main105: the pill, which has state of its own, was found; `mac-main.reader.account-mark` was not: "6 accessibility elements walked"):
    # a header is rebuilt when its thread changes. So one measure switches the gate on, another mail is opened and closed (row 0), and the code mail is opened again:
    # its header is a new thread's, built with the readers.
    c.call("measure", "mac-main.reader.codepill")      # a view carrying whooshAx (the subject carries a plain identifier: "nothing on screen carries that id", main111)
    c.call("key", "Escape")
    c.call("click", "mac-main.list.row.0")
    c.call("click", "mac-main.reader.open")
    c.call("key", "Escape")
    for _ in range(idx):
        c.call("key", "J")
    c.call("click", "mac-main.reader.open")
    # LAST: the notch's sample arrival and its list. The Escape presses above collapse the notch (main111: the pill was gone), so the notch is raised after them and nothing
    # that closes it is pressed afterwards; the reader stays open under it.
    for verb, ax in (("click", "mac-notch.stage.reset"), ("click", "mac-notch.stage.sample"), ("click", "mac-notch.stage.hover")):
        c.call(verb, ax)


GALLERY_PAGE = {"mac-components.gallery.label-chip": "chips", "mac-components.gallery.key-hint.cap": "keys"}


def resolve(c, ax):
    """`SIDEBAR` is the first sidebar account's mark (`mac-main.sidebar.account-mark.<account id>`); the gallery ids need their page shown first."""
    if ax == "SIDEBAR":
        rows = c.rows("sidebar_accounts")      # the seam library maps the region key to its ax id (a raw `rows sidebar_accounts` is refused: main99 read "account-mark.none")
        return "mac-main.sidebar.account-mark.%s" % rows[0]["id"] if not nj(rows) and rows else "mac-main.sidebar.account-mark.none"
    if ax in GALLERY_PAGE:
        c.call("click", "components.gallery.show." + GALLERY_PAGE[ax])
    return ax


def observe(c, exp):
    out = {}
    put_on_screen(c)
    for control, surfaces in exp["cases"]:
        out[control] = {}
        for surface, ax, why, _h, _r in surfaces:
            if ax is None:
                out[control][surface] = {"h": NJ(why), "r": NJ(why)}
                continue
            ax = resolve(c, ax)
            r = c.call("measure", ax)
            if not r.get("ok"):
                nj_ = NJ("measure %s: %s" % (ax, r.get("error")))
                out[control][surface] = {"h": nj_, "r": nj_}
                continue
            fr = r.get("frame") or {}
            out[control][surface] = {"h": fr["h"] if "h" in fr else NJ("measure %s: no frame.h" % ax),
                                     "r": r["radius"] if "radius" in r else NJ(NO_RADIUS)}
    return out


def fmt(v):
    return ("%g" % v) if isinstance(v, (int, float)) else str(v)


def judge(o, exp):
    res = []
    for control, surfaces in exp["cases"]:
        seen = o[control]
        names = [s[0] for s in surfaces]
        hs = {n: seen[n]["h"] for n in names}
        rs = {n: seen[n]["r"] for n in names}
        want = {s[0]: (s[3], s[4]) for s in surfaces}
        tail = " and ".join([", ".join(names[:-1]), names[-1]]) if len(names) > 2 else " and ".join(names)
        bad = [("height", n, hs[n], want[n][0]) for n in names if not nj(hs[n]) and hs[n] != want[n][0]]
        bad += [("radius", n, rs[n], want[n][1]) for n in names if not nj(rs[n]) and rs[n] != want[n][1]]
        if bad:
            res.append((control, False, "; ".join("%s on %s is %s, expected %s" % (k, n, fmt(g), fmt(w)) for k, n, g, w in bad)))
            continue
        unread = [(n, k, v) for n in names for k, v in (("height", hs[n]), ("radius", rs[n])) if nj(v)]
        if unread:
            res.append((control, None, "; ".join("%s on %s: %s" % (k, n, v.why) for n, k, v in unread[:2]) + (" (+%d more)" % (len(unread) - 2) if len(unread) > 2 else "")))
            continue
        h0, r0 = want[names[0]]
        res.append((control, True, "height %s radius %s on %s" % (fmt(h0), fmt(r0), tail) if len({want[n] for n in names}) == 1
                    else "; ".join("%s height %s radius %s" % (n, fmt(want[n][0]), fmt(want[n][1])) for n in names)))
    return res


def oracle(exp):
    return {control: {s[0]: {"h": s[3], "r": s[4]} for s in surfaces} for control, surfaces in exp["cases"]}


def setk(control, surface, key, val):
    def f(o):
        o[control][surface][key] = val
        return o
    return f


_E = expected()
_CAP, _ITEM = _E["cases"][1][1][0][3], _E["cases"][1][1][0][4]
_FILE_TILE = token("WhooshSpacing.swift", "fileTile")
_FIELD = token("WhooshRadius.swift", "field")
# MUTANTS (one per control, height and radius for the code pill, where the app already differs)
# code-pill-height  | the notch's code pill is drawn at the file-tile height                          | code-pill
# code-pill-radius  | the notch's code pill takes the field radius (ReaderCodePill.swift:22)                | code-pill
# chip-height       | the reader-meta label chip is one point taller                                          | label-chip
# hint-radius       | the palette key hint rounds its cap at the item radius                                  | key-hint
# mark-size         | the reader-meta account mark is drawn at the sidebar ring size                          | account-mark
MUTANTS = {
    "code-pill-height": ("notch pill at file-tile height", "code-pill", setk("code-pill", "notch-codepill", "h", _FILE_TILE)),
    "code-pill-radius": ("notch pill at field radius", "code-pill", setk("code-pill", "notch-codepill", "r", _FIELD)),
    "chip-height": ("chip one point taller", "label-chip", setk("label-chip", "gallery", "h", _CAP + 1)),
    "hint-radius": ("palette cap at item radius", "key-hint", setk("key-hint", "gallery", "r", _ITEM)),
    "mark-size": ("reader mark at ring size", "account-mark", setk("account-mark", "reader-meta", "h", _E["cases"][4][1][0][3])),
}
if __name__ == "__main__":
    sys.exit(run("component-grammar", expected, observe, judge, oracle, MUTANTS))
