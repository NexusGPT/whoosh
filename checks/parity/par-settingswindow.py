#!/usr/bin/env python3
"""par-settingswindow: the Settings window (picks SB and S3, owner 2026-11-08) driven over the control socket.

Drives the RUNNING rig copy (docs/CONTROL-SOCKET.md). Expected values come from docs/design/geometry/mac-settings.json, the
S3 board's pane list (docs/design/boards/settings-r2/settings-sb-s3/notes.md) and the Localizable catalogue, never from a built screen.
Every control the window lists in `rows mac-settings-window.controls` is clicked once with a visible outcome asserted.
Run (only in a window announced to the owner): sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/par-settingswindow.py'
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0.
"""
import json
import os
import struct
import subprocess
import sys
import time
import zlib

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, print_cases                                  # noqa: E402
from lib.ctl import Unreadable                                       # noqa: E402

ID = "par-settingswindow"
TITLE = "Settings window: sidebar and cards, ten panes, search, back and forward, every control"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "a pane switch shows that pane under its coloured icon; a search rings the found row; every control changes what it says it changes"
W, G, A, D, M = "mac-settings-window", "mac-settings-general", "mac-settings-accounts", "mac-settings-ai", "mac-mcp-notes"
GEO = json.load(open(os.path.join(ROOT, "docs", "design", "geometry", "mac-settings.json"), encoding="utf-8"))
CATALOGUE = json.load(open(os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8"))["strings"]
# The geometry board holds the ten panes that have a digit; Calendars (agendas) sits after accounts and has none (settings-integrate round 1).
# Replies (replies) sits after ai (round 3). Its six parts are listed under it in the sidebar (rows mac-settings-window.replies.parts).
PANES = [q for p in GEO["panes"] for q in ([p, "agendas"] if p == "accounts" else [p, "replies"] if p == "ai" else [p])]
KEYLESS = ("agendas", "replies")
KEYED = [p for p in PANES if p not in KEYLESS]      # the panes that answer to a digit, in sidebar order: ⌘1 to ⌘9, then ⌘0
REPLY_PARTS = ["auto", "on-demand", "sources", "retrieval", "voice", "prompt"]     # RepliesPage.panes, in the board's order
REPLY_TITLES = {"auto": "Auto draft", "on-demand": "On demand", "sources": "Sources", "retrieval": "Retrieval", "voice": "Voice", "prompt": "The prompt"}
R = "mac-settings-replies"


def head(p):
    """The pane's header and title ids: Replies draws its own header (mac-settings-replies.header.auto), the rest the window's."""
    return (R + ".header.auto", R + ".title.auto") if p == "replies" else ("%s.header.%s" % (W, p), "%s.title.%s" % (W, p))
TITLES = {"general": "General", "accounts": "Accounts", "agendas": "Calendars", "replies": "Replies", "viewing": "Viewing", "signatures": "Signatures", "decisions": "Decisions", "junk": "Junk",
          "ai": "AI and agents", "notifications": "Notifications", "shortcuts": "Shortcuts", "outside": "Outside apps"}
GROUPS = [("Mail", ["general", "accounts", "agendas", "viewing", "signatures"]), ("Sorting", ["decisions", "junk"]), ("AI", ["ai", "replies"]),
          ("Alerts and keys", ["notifications", "shortcuts", "outside"])]
TYPED_PROBE = "probe@nothing.example.test"      # text typed after the Settings window closed: it belongs to the surface that holds the keys
ACCENT_BLUE = (0, 100, 225)      # WhooshPalette.selection light; dark is (10, 91, 214): both read as "blue" below
PROVIDER_MARKS = ["anthropic", "openai", "gemini", "openrouter", "chatgpt"]   # AISettingsStore.providers ids, then the ChatGPT row
# The logo asset each provider row draws (stems of Sources/Features/SettingsWindow/Marks/); ChatGPT's row is OpenAI's blossom,
# Jev (the Decisions engine) and Laya (the Model row) are the Nexus mark. Mistral and Apple Intelligence have no Settings row.
LOGOS = {"anthropic": "anthropic", "openai": "openai", "gemini": "gemini", "openrouter": "openrouter", "chatgpt": "openai",
         "jev": "nexus-mark", "laya": "nexus-mark"}
LOGOS_ALL = dict(LOGOS, mistral="mistral", apple="apple-intelligence")      # `rows mac-settings-ai.logos`: one row per mark
HUES = {"general": "gray", "accounts": "blue", "agendas": "red", "replies": "purple", "viewing": "indigo", "signatures": "orange", "decisions": "purple", "junk": "brown",
        "ai": "pink", "notifications": "red", "shortcuts": "gray", "outside": "teal"}


def word(key, code):
    return CATALOGUE[key]["localizations"][code]["stringUnit"]["value"]


# ---- reading a shot's pixels (8-bit RGB or RGBA PNG, no interlace) ---------------------------------------------------
def read_png(path):
    data = open(path, "rb").read()
    pos, idat, w, h, ct = 8, b"", 0, 0, 0
    while pos < len(data):
        n, kind = struct.unpack(">I4s", data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + n]
        if kind == b"IHDR":
            w, h, depth, ct = struct.unpack(">IIBB", body[:10])
            if depth != 8 or ct not in (2, 6):
                raise Unreadable("shot %s is not an 8-bit RGB or RGBA PNG" % path)
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


def pixel(img, win_w, x, y):
    """The colour at window point (x, y); the shot may be at 2x."""
    w, h, bpp, rows = img
    k = w / float(win_w)
    px, py = int(x * k), int(y * k)
    r = rows[py]
    return tuple(r[px * bpp:px * bpp + 3])


def is_blue(c):
    return c[2] > 180 and c[0] < 90 and c[1] < 150 and c[2] - c[0] > 100


def hue_ok(c, name):
    r, g, b = c
    return {"blue": b > 200 and r < 90, "gray": abs(r - g) < 24 and abs(g - b) < 24 and 80 < r < 190,
            "indigo": b > 170 and r > 60 and g < 140 and r < 140, "orange": r > 200 and 110 < g < 200 and b < 90,
            "purple": r > 140 and b > 190 and g < 130, "brown": r > 130 and 100 < g < 175 and b < 145 and r > b + 25,
            "pink": r > 200 and g < 100 and b > 60, "teal": b > 170 and g > 150 and r < 110, "red": r > 200 and g < 110 and b < 100}[HUES[name]]


HUES_COLOUR = {}


# ---- driving ----------------------------------------------------------------------------------------------------------
def click(s, ax):
    """True when the click reached something that did something, else the socket's words."""
    try:
        s.ctl.click(ax)
        return True
    except Unreadable as e:
        return str(e)


def place(s):
    return s.ctl.rows(W + ".place")[0]


def settings(s):
    return s.ctl.rows(G + ".settings")[0]


def engine_logo(s, engine):
    """The logo asset the Decisions engine row for `engine` (jev, local, off) serves."""
    return [r["logo"] for r in s.ctl.rows(D + ".decisions.engine") if r["id"] == engine][0]


def frame(s, ax):
    return s.ctl.measure(ax)["frame"]


def settings_window(s):
    return [f for f in s.ctl.winframes() if f["title"] == "Settings"][0]


def drawn(s, ax):
    """True when something on screen carries the id and has a size."""
    ctl = s.ctl                       # raises in a dry run: not judged, not failed
    try:
        return ctl.measure(ax)["frame"]["h"] > 0
    except (Unreadable, KeyError):
        return False


def pane_titles(s):
    return [r["title"] for r in s.ctl.rows(W + ".panes")]


def snapshot(s):
    """Everything a click on a control can change, as one comparable value."""
    out = {"place": place(s), "settings": settings(s), "panes": s.ctl.rows(W + ".panes")}
    for region in (A + ".accounts", A + ".failing", D + ".decisions.engine", D + ".speech", D + ".agents.card", D + ".agents.form", M + ".server",
                   M + ".apps", G + ".notifications", G + ".pinned"):
        try:
            out[region] = s.ctl.rows(region)
        except Unreadable:
            out[region] = None
    return json.dumps(out, sort_keys=True, default=str)

# ---- evidence on a red (reader's dossier main77): a failed or not-judged case below carries the values that decide it ----------------------------
# Read from the app at the moment of the failure through reads the socket already serves (`rows`, `measure` with the window that holds the id,
# `winframes`, `rows mac-keyevent.monitor`) and through the actions the case's own steps were refused (`journey.notes`). Not served: whether a window
# is the key window apart from a keyevent (Sources/App/WindowBridge.swift:33 `frames()` would carry "key": w.isKeyWindow), and the ids the accessibility
# walk found when `measure` says "nothing on screen carries that id" (Sources/App/WindowControl.swift:66 counts them; the reply could list them).
def _compact(v, cap=300):
    t = json.dumps(v, sort_keys=True, default=str)
    return t if len(t) <= cap else t[:cap - 3] + "..."


def read_evidence(ctl, reads):
    """`label=value; ...` for each (label, fn(ctl)[, cap]) of `reads`; a read that fails says why. Never raises: it explains a verdict, it never hides one."""
    if ctl is None:
        return "no socket"
    out = []
    for label, fn, *cap in reads:
        try:
            out.append("%s=%s" % (label, _compact(fn(ctl), *cap)))
        except BaseException as e:      # noqa: BLE001
            if isinstance(e, (KeyboardInterrupt, SystemExit)):
                raise
            out.append("%s=unreadable(%s: %s)" % (label, type(e).__name__, str(e)[:210]))
    return "; ".join(out)


def ev_rows(ax, *keys):
    return lambda c: [({k: r.get(k) for k in keys} if keys else r) for r in c.rows(ax)]


def ev_measure(ax):
    return lambda c: {k: v for k, v in c.measure(ax).items() if k in ("frame", "window")}


def ev_windows(c):
    return [(f.get("title"), f.get("id"), f.get("kind"), [round(f.get(k, 0)) for k in ("x", "y", "w", "h")]) for f in c.winframes()]


def ev_monitor(c):
    r = (c.rows("mac-keyevent.monitor") or [{}])[0]
    last = r.get("last") or {}
    return {"seen": r.get("seen"), "sent": r.get("sent"), "route": last.get("route"), "handled": last.get("handled"), "key_before": last.get("key_before"),
            "key_after": last.get("key_after"), "windows": [(w.get("id"), w.get("key"), w.get("focus")) for w in last.get("windows", [])]}


def expect_ev(s, name, thunk, expected, reads):
    """`s.expect`; a failed case's line also carries `evidence: <reads>`, and a case that cannot be judged says why and carries it too."""
    reads = list(reads) + [("refused_actions", lambda c: s.j.notes[-4:])]
    def run():
        try:
            got = thunk()
        except Unreadable as e:
            raise Unreadable("%s | evidence: %s" % (e, read_evidence(s.j.ctl, reads)))
        except (KeyError, IndexError, TypeError) as e:
            s._eg = (expected, None)
            return False, "reply shape: %s %s | evidence: %s" % (type(e).__name__, e, read_evidence(s.j.ctl, reads))
        s._eg = (expected, got)
        if got == expected:
            return True, "expected %r, got %r" % (expected, got)
        return False, "expected %r, got %r | evidence: %s" % (expected, got, read_evidence(s.j.ctl, reads))
    s._record(name, run)


def replies_open(s):
    return [r["open"] for r in s.ctl.rows(W + ".panes") if r["id"] == "replies"][0]


def replies_parts_drawn(s, wait=2.0):
    """Whether the Replies parts are still drawn. The parts leave the sidebar through an animated removal (SettingsSidebar.swift:41-43), so the state (`open`) is false a few
    frames before the rows are gone: main79 read the measure at once and saw the row still 23 pt high. A parts row still drawn after `wait` seconds is the red case."""
    ok = drawn(s, W + ".pane-row.replies.auto")
    end = time.monotonic() + wait
    while ok and time.monotonic() < end:
        time.sleep(0.1)
        ok = drawn(s, W + ".pane-row.replies.auto")
    return ok


def replies_closed_by_default(s):
    click(s, W + ".pane-row.general")
    return (replies_open(s), replies_parts_drawn(s))


def replies_closed_by_another_pane(s):
    click(s, W + ".pane-row.replies.voice")
    click(s, W + ".pane-row.accounts")
    a = (replies_open(s), replies_parts_drawn(s))
    click(s, W + ".pane-row.replies")
    click(s, W + ".pane-row.general")
    return (a, (replies_open(s), replies_parts_drawn(s)))


def reads_replies_open(s):
    return [("replies_pane_row", lambda c: [{k: r.get(k) for k in ("id", "open", "current")} for r in c.rows(W + ".panes") if r.get("id") == "replies"]), ("parts_row_auto", ev_measure(W + ".pane-row.replies.auto")),
            ("place", ev_rows(W + ".place", "pane", "reply", "title")), ("windows", ev_windows)]


def probe_id(p):
    """The id whose frame shows the pane's header is drawn. The other panes mark the whole header (`mac-settings-window.header.<pane>`); the Replies header carries
    `mac-settings-replies.header.auto` only as a form-fit probe id, which no `measure` reads (RepliesScaffold.swift:40, FormGrid.swift:196-199: main79 "no measurement"),
    while its TITLE, inside that header, is marked (`mac-settings-replies.title.auto`, RepliesScaffold.swift:45): the title's frame is the drawn-header read for Replies."""
    return head(p)[1] if p == "replies" else head(p)[0]


def click_each_pane(s, cur):
    out = {}
    for p in PANES:
        cur["pane"] = p
        ok = click(s, "%s.pane-row.%s" % (W, p))
        out[p] = (ok, place(s)["pane"], place(s)["title"], frame(s, probe_id(p))["h"] > 0)
    return out


def reads_click_each(s, cur):
    return [("pane_being_read", lambda c: cur.get("pane")), ("header_id", lambda c: probe_id(cur.get("pane", "general"))),
            ("header_frame", lambda c: ev_measure(probe_id(cur.get("pane", "general")))(c)),
            ("replies_header_probe_frame", lambda c: ev_measure(head("replies")[0])(c)), ("place", ev_rows(W + ".place", "pane", "reply", "title")),
            ("replies_ids_served", lambda c: [r.get("ax") for r in c.rows(R + ".controls") if any(w in r.get("ax", "") for w in (".header", ".title", ".page"))][:6]),
            ("windows", ev_windows)]


def focused_editor(s):
    """The key of the Replies editor that holds the keys now (`rows mac-settings-replies.editors`, field `focused`), else None."""
    held = [r["key"] for r in s.ctl.rows(R + ".editors") if r.get("focused")]
    return held[0] if held else None


def escape_yields_pane(s):
    click(s, W + ".pane-row.replies.sources")
    s.ctl.click(W + ".fixture.query.show")           # the window's search holds a query
    click(s, W + ".pane-row.replies.voice")
    # the pane's own editor door (RepliesEditorDoors.swift:19): the editor takes the keys, so its Escape must close it first. The page was opened a moment ago and
    # the window's search just changed, so SwiftUI may still place the text view or hand the focus back: ask again (bounded: 20 asks, 0.1 s apart) until the
    # editors row says it holds the keys; main88 read None here after one ask. The last answer is what the case judges, so a door that never holds stays red.
    s.ctl.click(R + ".editor.voiceText")             # the first ask raises when the door is not served: the case is then not judged
    for _ in range(20):
        time.sleep(0.1)
        if focused_editor(s) == "voiceText":
            break
        click(s, R + ".editor.voiceText")
    before = focused_editor(s)
    s.key("Escape")
    after = focused_editor(s)
    q = place(s)["query"]
    s.key("Escape")                                  # nothing left for the pane: the window's Escape clears its search
    return (before, after, q, place(s)["query"])


def reads_escape(s):
    return [("editor_ids_served", lambda c: [r.get("ax") for r in c.rows(R + ".controls") if ".editor." in r.get("ax", "") or ".doc." in r.get("ax", "")][:8]),
            ("editors_rows", ev_rows(R + ".editors")), ("ui", lambda c: c.rows(R + ".state")[0].get("ui")), ("place", ev_rows(W + ".place", "pane", "reply", "query")),
            ("keyevent", ev_monitor), ("windows", ev_windows)]


def search_titles(s):
    return [r["title"] for r in s.ctl.rows(W + ".search")][:1]


def search_first_hit(s):
    return (place(s)["pane"], place(s)["ring"])


def search_first_path(s):
    return s.ctl.rows(W + ".search")[0]["path"]


def reads_search(s):
    return [("search_text", lambda c: place(s).get("query")), ("result_rows", ev_rows(W + ".search", "title", "row", "path", "selected")),
            ("place", ev_rows(W + ".place", "pane", "query", "ring")), ("search_field_frame", ev_measure(W + ".search")),
            ("index_rows", lambda c: len(c.rows(W + ".search.index"))), ("keyevent", ev_monitor), ("windows", ev_windows)]


def engine_chosen(s, e):
    return [r["id"] for r in s.ctl.rows(D + ".decisions.engine") if r["chosen"]]


def jev_refused_without_key_and_consent(s, before):
    """(the core's provider did not move to Jev, the Jev segment says in words why). The words are the core's, so only that they exist is asked.
    Since settingsai round 6 the engine rows' `chosen` is the segment the screen DRAWS (`shownEngine`: Jev while its key or consent is asked for,
    SettingsAIMacFeature.swift:90-95), so it cannot say what is saved: main93 read Jev drawn with the refusal and the core still on `rules`, which is
    the right behaviour. What is saved is `decision_model/provider`."""
    refusal = [r["refusal"] for r in s.ctl.rows(D + ".decisions.engine") if r["id"] == "jev"][0]
    return (s.ctl.core("decision_model/provider") != "jev" and before is not None, refusal != "")


def reads_engine(s):
    return [("engine_rows", ev_rows(D + ".decisions.engine", "id", "chosen", "refusal")), ("core_provider", lambda c: c.core("decision_model/provider")),
            ("jev_key_row", ev_rows(D + ".decisions.jev.key")), ("jev_consent_row", ev_rows(D + ".decisions.jev.consent")), ("place", ev_rows(W + ".place", "pane"))]


def model_row_drawn(s):
    """The Model row (Laya) is on screen. The row is one container the app names only with a plain accessibility identifier
    (ModelSetupRow.swift:52-53), which `measure` cannot read (main81: "nothing on screen carries that id"); the row's own button
    (`.decisions.local.download`, AmberButton `.whooshAx`, ModelSetupRow.swift:95) is drawn exactly while the weights are not here, so that
    stands for the row. A row nothing of which can be measured is not drawn."""
    return drawn(s, D + ".decisions.local") or drawn(s, D + ".decisions.local.download")


def reads_laya_mark(s):
    return [("laya_row_frame", ev_measure(D + ".decisions.local")), ("laya_button_frame", ev_measure(D + ".decisions.local.download")), ("laya_mark_frame", ev_measure(D + ".provider.laya.mark")), ("laya_row", ev_rows(D + ".decisions.local")), ("logos", lambda c: [r for r in c.rows(D + ".logos") if r.get("slug") == "laya"]),
            ("engine_rows", ev_rows(D + ".decisions.engine", "id", "chosen")), ("place", ev_rows(W + ".place", "pane")), ("windows", ev_windows)]


# what each evidence case expects: one place for the drive and for the stand-in of `--selftest`
EXPECT = {"replies-is-closed-while-another-pane-is-selected": (False, False),
          "selecting-another-pane-closes-replies-and-it-is-not-remembered": ((False, False), (False, False)),
          "search-finds-the-emblems-switch-first": ["Show account emblems"], "first-hit-opens-its-pane-and-rings-its-row": ("general", G + ".appearance.emblems"),
          "results-show-their-path": "General › Message list", "provider-mark-laya": (True, False), "engine-jev": (True, True),
          "click-every-pane-row-shows-its-pane": {p: (True, p, REPLY_TITLES["auto"] if p == "replies" else TITLES[p], True) for p in PANES},
          "escape-goes-to-the-open-pane-first-then-the-window": ("voiceText", None, "show", "")}
# the cases above whose red line carries evidence (`--selftest` proves each prints it on a mutant of a stand-in)
EVIDENCE_CASES = ["replies-is-closed-while-another-pane-is-selected", "selecting-another-pane-closes-replies-and-it-is-not-remembered",
                  "search-finds-the-emblems-switch-first", "first-hit-opens-its-pane-and-rings-its-row", "results-show-their-path",
                  "engine-jev", "engine-off", "engine-local", "provider-mark-laya", "click-every-pane-row-shows-its-pane", "escape-goes-to-the-open-pane-first-then-the-window"]


def drive(j):
    # ---- 1 the window ----------------------------------------------------------------------------------------------
    s = j.step(ID, "The window: size, sidebar, toolbar")
    s.click(W + ".open")
    s.click(G + ".appearance.language.en")          # the words below are English whatever the Mac speaks
    s.expect("window-opens-on-the-settings-surface", lambda: s.ctl.uistate()["surface"].startswith("mac-settings"), True)
    s.click(W + ".size.default")
    s.expect("default-size-940x680", lambda: (round(settings_window(s)["w"]), round(settings_window(s)["h"])), (GEO["window"]["w"], GEO["window"]["h"]))
    s.expect("sidebar-250-at-default", lambda: round(frame(s, W + ".sidebar")["w"]), GEO["sidebar"]["w"])
    s.expect("summary-card-is-drawn-at-the-default-size", lambda: drawn(s, W + ".summary"), True)
    s.expect("toolbar-nav-buttons-28", lambda: (round(frame(s, W + ".back")["w"]), round(frame(s, W + ".back")["h"])),
             (GEO["toolbar"]["nav_button"],) * 2)
    s.expect("search-field-28-high", lambda: round(frame(s, W + ".search")["h"]), GEO["sidebar"]["search"]["h"])
    s.expect("pane-row-28-high", lambda: round(frame(s, W + ".pane-row.general")["h"]), GEO["sidebar"]["pane_row"]["h"])
    s.expect("pane-header-icon-40", lambda: round(frame(s, W + ".header.general")["h"] >= GEO["content"]["header_icon"]), 1)
    s.click(W + ".size.min")
    s.expect("min-size-700x480", lambda: (round(settings_window(s)["w"]), round(settings_window(s)["h"])), (GEO["min"]["w"], GEO["min"]["h"]))
    s.expect("sidebar-210-at-min", lambda: round(frame(s, W + ".sidebar")["w"]), GEO["sidebar"]["min_w"])
    s.expect("summary-card-gives-way-at-the-minimum", lambda: drawn(s, W + ".summary"), False)
    # The sidebar is a scroll view and the panes outgrew the 480 pt minimum (the Replies parts sit under their pane only while it is chosen, so how many
    # rows fall below the foot depends on the chosen pane): what must hold is that the column ends inside the window and the rows below the foot are
    # only the tail of the pane order, reached by the sidebar's scroll. A row pushed out of the middle, or a column taller than the window, is red.
    def rows_below_the_foot():
        order = [r["id"] for r in s.ctl.rows(W + ".panes")]
        below = [p for p in order if frame(s, W + ".pane-row." + p)["y"] + 28 > GEO["min"]["h"]]
        sb = frame(s, W + ".sidebar")
        return (sb["y"] + sb["h"] <= GEO["min"]["h"] + 1, below == order[len(order) - len(below):])
    s.expect("every-pane-row-fits-the-min-window", rows_below_the_foot, (True, True))
    s.shot("par-settingswindow-general-700x480")
    s.click(W + ".size.default")
    s.expect("back-at-default-size", lambda: round(frame(s, W + ".sidebar")["w"]), GEO["sidebar"]["w"])

    # ---- 2 the panes -------------------------------------------------------------------------------------------------
    s = j.step(ID, "The ten panes, their groups, icons and keys")
    s.click(W + ".pane-row.general")
    s.expect("ten-panes-in-four-groups", lambda: [(g["title"], g["panes"]) for g in s.ctl.rows(W + ".groups")], GROUPS)
    s.expect("pane-names", lambda: pane_titles(s), [TITLES[p] for p in PANES])
    s.expect("pane-keys-cmd-1-to-0", lambda: [r["key"] for r in s.ctl.rows(W + ".panes")], ["" if p in KEYLESS else "⌘%d" % ((KEYED.index(p) + 1) % 10) for p in PANES])
    s.expect("every-pane-has-a-symbol", lambda: all(r["symbol"] for r in s.ctl.rows(W + ".panes")), True)
    s.expect("summary-card-counts", lambda: [(r["accounts"], r["needs_you"], len(r["marks"]), r["more"]) for r in s.ctl.rows(W + ".summary")], [(30, 1, 5, 25)])
    s.expect("accounts-row-badge-counts-who-needs-you", lambda: [r["badge"] for r in s.ctl.rows(W + ".panes") if r["id"] == "accounts"], [1])

    def icons_and_selection():
        path = s.ctl.send("shot", "mac-settings-general")["path"]
        img, ww = read_png(path), settings_window(s)["w"]
        bad = []
        for p in PANES:
            f = frame(s, W + ".pane-row." + p)
            ic = pixel(img, ww, f["x"] + 9, f["y"] + 14 - 8)      # inside the 20 pt square, above the white symbol
            if p != "general" and not hue_ok(ic, p):
                bad.append((p, ic, (f["x"], f["y"], f["w"], f["h"]), img[:2], ww))
        return bad
    s.expect("pane-icons-are-coloured-squares", icons_and_selection, [])

    def selected_row_blue():
        s.click(W + ".pane-row.accounts")
        time.sleep(0.4)
        path = s.ctl.send("shot", "mac-settings-accounts")["path"]
        img, ww = read_png(path), settings_window(s)["w"]
        f, g = frame(s, W + ".pane-row.accounts"), frame(s, W + ".pane-row.viewing")
        got = (is_blue(pixel(img, ww, f["x"] + 80, f["y"] + 4)), is_blue(pixel(img, ww, g["x"] + 80, g["y"] + 4)))
        return got if got == (True, False) else (got, f, g, pixel(img, ww, f["x"] + 80, f["y"] + 4), img[:2], ww)
    s.expect("selected-row-is-blue-and-others-are-not", selected_row_blue, (True, False))

    # the place's title is the toolbar's words: for Replies the part that shows (Auto draft), not the pane's name
    cur_pane = {}
    expect_ev(s, "click-every-pane-row-shows-its-pane", lambda: click_each_pane(s, cur_pane), EXPECT["click-every-pane-row-shows-its-pane"],
              reads_click_each(s, cur_pane))

    def reply_parts():
        out = {"rows": [(r["id"], r["title"]) for r in s.ctl.rows(W + ".replies.parts")]}
        for part in REPLY_PARTS:
            ok = click(s, "%s.pane-row.replies.%s" % (W, part))
            cur = [r["id"] for r in s.ctl.rows(W + ".replies.parts") if r["current"]]
            pl = place(s)
            out[part] = (ok, pl["pane"], pl["reply"], pl["title"], cur, [r["current"] for r in s.ctl.rows(W + ".panes") if r["id"] == "replies"])
        return out
    s.expect("replies-parts-are-listed-under-replies-and-each-opens", reply_parts,
             dict({"rows": [(p, REPLY_TITLES[p]) for p in REPLY_PARTS]},
                  **{p: (True, "replies", p, REPLY_TITLES[p], [p], [True]) for p in REPLY_PARTS}))

    def opened():
        return [r["open"] for r in s.ctl.rows(W + ".panes") if r["id"] == "replies"][0]

    def parts_drawn():
        return drawn(s, W + ".pane-row.replies.auto")

    expect_ev(s, "replies-is-closed-while-another-pane-is-selected", lambda: replies_closed_by_default(s), EXPECT["replies-is-closed-while-another-pane-is-selected"], reads_replies_open(s))

    def parent_click_opens_first_part():
        click(s, W + ".pane-row.general")
        ok = click(s, W + ".pane-row.replies")
        pl = place(s)
        return (ok, pl["pane"], pl["reply"], opened(), parts_drawn())
    s.expect("a-click-on-the-parent-opens-it-and-selects-its-first-part", parent_click_opens_first_part, (True, "replies", "auto", True, True))

    def chevron_toggles_without_moving():
        click(s, W + ".pane-row.replies.sources")
        click(s, W + ".pane-row.replies.chevron")
        shut = (opened(), place(s)["pane"], place(s)["reply"], parts_drawn())
        click(s, W + ".pane-row.replies.chevron")
        again = (opened(), place(s)["pane"], place(s)["reply"], parts_drawn())
        click(s, W + ".pane-row.general")
        click(s, W + ".pane-row.replies.chevron")           # beside another pane: it opens the parts and the selection stays
        beside = (opened(), place(s)["pane"], parts_drawn())
        click(s, W + ".pane-row.general")
        return (shut, again, beside)
    s.expect("the-chevron-opens-and-closes-without-changing-the-pane", chevron_toggles_without_moving,
             ((False, "replies", "sources", False), (True, "replies", "sources", True), (True, "general", True)))

    expect_ev(s, "selecting-another-pane-closes-replies-and-it-is-not-remembered", lambda: replies_closed_by_another_pane(s), EXPECT["selecting-another-pane-closes-replies-and-it-is-not-remembered"], reads_replies_open(s))

    def default_window_closed_fits():
        s.ctl.click(W + ".size.default")
        click(s, W + ".pane-row.general")
        h = settings_window(s)["h"]
        return ([p for p in PANES if frame(s, W + ".pane-row." + p)["y"] + 28 > h], drawn(s, W + ".summary"))
    s.expect("at-940x680-with-replies-closed-no-pane-row-is-below-the-foot", default_window_closed_fits, ([], True))

    def default_window_open_fits():
        s.ctl.click(W + ".size.default")
        click(s, W + ".pane-row.replies.sources")
        h = settings_window(s)["h"]
        rows = ["%s.pane-row.%s" % (W, p) for p in PANES] + ["%s.pane-row.replies.%s" % (W, p) for p in REPLY_PARTS]
        below = [ax for ax in rows if frame(s, ax)["y"] + 28 > h]
        card_open = drawn(s, W + ".summary")                 # the card yields its height to the six parts while Replies is open
        click(s, W + ".pane-row.general")
        return (below, card_open, drawn(s, W + ".summary"))    # and it is back once Replies closes
    s.expect("at-940x680-with-replies-open-nothing-in-the-sidebar-is-below-the-foot", default_window_open_fits, ([], False, True))

    def reply_history():
        click(s, W + ".pane-row.general")
        click(s, W + ".pane-row.replies.sources")
        click(s, W + ".pane-row.replies.voice")
        s.key("⌘[")
        a = (place(s)["pane"], place(s)["reply"])
        s.key("⌘]")
        b = (place(s)["pane"], place(s)["reply"])
        click(s, W + ".pane-row.general")
        return (a, b)
    s.expect("back-and-forward-walk-the-parts-of-replies", reply_history, (("replies", "sources"), ("replies", "voice")))

    expect_ev(s, "escape-goes-to-the-open-pane-first-then-the-window", lambda: escape_yields_pane(s), EXPECT["escape-goes-to-the-open-pane-first-then-the-window"], reads_escape(s))

    def by_key():
        out = {}
        s.ctl.click(W + ".open")                     # the pane keys answer only while the Settings window holds the keys (SettingsWindowRegistry inSettings): bring it forward, whatever the case before left
        for i, p in enumerate(KEYED):
            s.ctl.key("⌘%d" % ((i + 1) % 10))
            out[p] = place(s)["pane"]
        return out
    s.expect("cmd-1-to-cmd-0-switch-the-panes", by_key, {p: p for p in KEYED})

    # A107: the digit row by PHYSICAL key (ChordKit key codes 18 to 29), as an AZERTY keyboard presses it (& é " ' ( § è ! ç à): the
    # socket's key-code verb (KeyCodeVerb.swift) builds the real event, as par-keys' A99 case does for the main window. The
    # real-event path (the window's local monitor, SettingsDigitKeys) cannot be driven on a hidden rig: its source is held by --selftest.
    DIGIT_CODES = [18, 19, 20, 21, 23, 22, 26, 28, 25, 29]

    def by_key_code():
        out = {}
        s.ctl.click(W + ".open")                     # same: the window holds the keys
        for i, p in enumerate(KEYED):
            s.ctl.send("key", "code:%d" % DIGIT_CODES[i], "mods:cmd", "layout:azerty")
            out[p] = place(s)["pane"]
        return out
    s.expect("cmd-digit-by-key-code-switches-the-panes-on-azerty", by_key_code, {p: p for p in KEYED})
    s.click(W + ".summary")
    s.expect("summary-card-opens-accounts", lambda: place(s)["pane"], "accounts")

    def placeholders():
        out = {}
        for p in ("viewing", "signatures", "junk"):
            s.click("%s.pane-row.%s" % (W, p))
            controls = [r for r in s.ctl.rows(W + ".controls") if r["pane"] == p]
            out[p] = (controls, frame(s, "%s.%s.coming.0" % (W, p))["h"] > 0, frame(s, "%s.%s.note" % (W, p))["h"] > 0)
        return out
    s.expect("placeholder-panes-say-what-comes-and-hold-no-control", placeholders, {p: ([], True, True) for p in ("viewing", "signatures", "junk")})

    # ---- 3 back and forward --------------------------------------------------------------------------------------
    s = j.step(ID, "Back and forward")
    s.click(W + ".pane-row.general")
    s.expect("back-and-forward-start-at-the-pane-last-reached", lambda: (place(s)["pane"], place(s)["can_forward"]), ("general", False))
    s.click(W + ".pane-row.accounts")
    s.expect("back-enabled-after-a-move", lambda: place(s)["can_back"], True)
    s.click(W + ".back")
    s.expect("back-returns-to-the-previous-pane", lambda: (place(s)["pane"], place(s)["can_forward"]), ("general", True))
    s.click(W + ".forward")
    s.expect("forward-returns", lambda: place(s)["pane"], "accounts")
    s.key("⌘[")
    s.expect("cmd-bracket-left-goes-back", lambda: place(s)["pane"], "general")
    s.key("⌘]")
    s.expect("cmd-bracket-right-goes-forward", lambda: place(s)["pane"], "accounts")
    s.expect("back-with-nowhere-to-go-does-nothing-and-says-so", lambda: (click(s, W + ".forward") is not True, place(s)["pane"]), (True, "accounts"))

    # ---- 4 search ----------------------------------------------------------------------------------------------------
    s = j.step(ID, "Search: ⌘F finds a single setting across panes and rings its row")
    s.click(W + ".pane-row.accounts")
    for _ in range(3):      # a query an earlier step left (main79: "show" from the Escape case, so the typed word became "showemblem") is cleared by the window's own Escape; the evidence below names the query typed
        if place(s)["query"] == "":
            break
        s.ctl.key("Escape")
    s.key("⌘F")
    s.type("emblem")
    expect_ev(s, "search-finds-the-emblems-switch-first", lambda: search_titles(s), EXPECT["search-finds-the-emblems-switch-first"], reads_search(s))
    expect_ev(s, "first-hit-opens-its-pane-and-rings-its-row", lambda: search_first_hit(s), EXPECT["first-hit-opens-its-pane-and-rings-its-row"], reads_search(s))
    s.expect("the-ringed-row-is-on-screen", lambda: frame(s, G + ".appearance.emblems")["h"] > 0, True)
    expect_ev(s, "results-show-their-path", lambda: search_first_path(s), EXPECT["results-show-their-path"], reads_search(s))
    s.expect("results-count-says-how-many", lambda: frame(s, W + ".results.count")["h"] > 0, True)
    s.key("Escape")
    s.expect("esc-clears-the-search-and-the-ring", lambda: (place(s)["query"], place(s)["ring"], s.ctl.rows(W + ".search")), ("", None, []))
    s.key("⌘F")
    s.type("qqqq")
    s.expect("no-match-says-so", lambda: (s.ctl.rows(W + ".search"), frame(s, W + ".results.empty")["h"] > 0), ([], True))
    s.key("Escape")

    def finds_every_setting():
        missed = []
        for e in s.ctl.rows(W + ".search.index"):
            s.ctl.key("⌘F")
            s.ctl.type(e["title"])
            hits = [r["row"] for r in s.ctl.rows(W + ".search")]
            if e["row"] not in hits:
                missed.append(e["title"])
            s.ctl.key("Escape")
        return missed
    s.expect("every-setting-finds-itself-by-its-own-name", finds_every_setting, [])

    def french_search():
        s.ctl.click(G + ".appearance.language.fr")
        want = word("Undo send", "fr")
        s.ctl.key("⌘F")
        s.ctl.type(want)
        hit = [r["row"] for r in s.ctl.rows(W + ".search")][:1]
        s.ctl.key("Escape")
        s.ctl.click(G + ".appearance.language.system")
        return hit
    s.expect("search-works-in-french", french_search, [G + ".appearance.undo-send"])
    s.click(G + ".appearance.language.en")
    s.click(W + ".fixture.query.show")
    s.expect("up-and-down-move-the-chosen-result", lambda: (s.ctl.rows(W + ".search")[0]["selected"], s.ctl.rows(W + ".search")[-1]["selected"]), (True, False))
    s.key("Escape")

    # ---- 5 General: the controls the window carries ------------------------------------------------------------------
    s = j.step(ID, "General: appearance, density, motion, emblems, undo send, language, pinned labels")
    s.click(W + ".pane-row.general")
    first_theme = {}
    s.expect("the-theme-the-rig-starts-with-is-read", lambda: (first_theme.update(t=s.ctl.uistate()["theme"]), True)[1], True)
    for a in ("dark", "light", "auto"):
        s.click("%s.appearance.theme.%s" % (G, a))
        s.expect("appearance-" + a, lambda a=a: (settings(s)["appearance"], s.ctl.uistate()["theme"] == (a if a != "auto" else first_theme["t"])),
                 (a, True))
    for d in ("compact", "regular"):
        s.click("%s.appearance.density.%s" % (G, d))
        s.expect("density-" + d, lambda d=d: settings(s)["density"], d)
    for m in ("reduced", "full"):
        s.click("%s.appearance.motion.%s" % (G, m))
        s.expect("motion-" + m, lambda m=m: settings(s)["motion"], m)
    for n in (5, 10, 15, 20, 25, 30):
        s.click("%s.appearance.undo-send.%d" % (G, n))
        s.expect("undo-send-%d" % n, lambda n=n: settings(s)["undo_seconds"], n)
    s.click(G + ".appearance.undo-send.10")

    def searchable_rows_are_drawn():
        gone = []
        for e in s.ctl.rows(W + ".search.index"):
            s.ctl.click("%s.pane-row.%s" % (W, e["pane"]))
            time.sleep(0.5)
            try:
                if not frame(s, e["row"])["h"] > 0:
                    gone.append(e["row"])
            except (Unreadable, KeyError) as ex:
                gone.append((e["row"], str(ex)[:90]))
        s.ctl.click(W + ".pane-row.general")
        return gone
    s.expect("every-searchable-row-is-drawn-in-its-pane", searchable_rows_are_drawn, [])

    def found_settings_are_shown():
        """A found setting is SHOWN: every searchable entry is typed in the search field and chosen from the results, at the minimum window (every pane is taller than
        it); what the search rings must then lie inside the window, at least 28 points of it (a row's height) or all of it. main180: a found GROUP, button or footnote
        was ringed where it stood, below the foot, because only rows carried the id the column scrolls to."""
        s.ctl.click(W + ".size.min")
        hidden = []
        try:
            for e in s.ctl.rows(W + ".search.index"):
                s.ctl.click(W + (".pane-row.general" if e["pane"] == "accounts" else ".pane-row.accounts"))
                for _ in range(3):
                    if place(s)["query"] == "":
                        break
                    s.ctl.key("Escape")
                s.ctl.key("\u2318F")
                s.ctl.type(e["title"])
                time.sleep(0.3)
                res = s.ctl.rows(W + ".search")
                i = next((k for k, r in enumerate(res) if r.get("row") == e["row"]), None)
                if i is None or i > 11:
                    hidden.append((e["row"], "not among the first 12 results for its own title"))
                    continue
                s.ctl.click("%s.result.%d" % (W, i))
                time.sleep(0.7)                                   # the column's scroll (WhooshMotion.readerCrossFade)
                try:
                    f, win = frame(s, e["row"]), settings_window(s)
                    seen = min(f["y"] + f["h"], win["h"]) - max(f["y"], 0)
                    if place(s)["ring"] != e["row"] or seen < min(f["h"], 28) - 1:
                        hidden.append((e["row"], "ring %s, y %d, height %d, window %d" % (place(s)["ring"] == e["row"], round(f["y"]), round(f["h"]), round(win["h"]))))
                except (Unreadable, KeyError) as ex:
                    hidden.append((e["row"], str(ex)[:90]))
        finally:
            s.ctl.click(W + ".size.default")
            s.ctl.click(W + ".pane-row.general")
        return hidden
    s.expect("every-found-setting-is-scrolled-into-view", found_settings_are_shown, [])

    def language_through_the_window():
        s.ctl.click(G + ".appearance.language.fr")
        out = (pane_titles(s)[:2], s.ctl.rows("mac-main.tabs")[0]["title"] == word("Primary", "fr"), place(s)["title"])
        s.ctl.click(G + ".appearance.language.system")
        return out
    s.expect("language-fr-turns-the-window-and-the-main-screen-french", language_through_the_window,
             ([word("General", "fr"), word("Accounts", "fr")], True, word("General", "fr")))
    s.expect("back-to-system-gives-the-macs-pane-names-again", lambda: pane_titles(s)[0] in (word("General", c) for c in ("en", "fr", "de", "es", "nl", "it", "pt", "pl", "sv", "da", "nb", "fi", "ja", "zh-Hans")), True)

    # ---- 6 notifications, shortcuts, decisions, AI, outside apps ------------------------------------------------------
    s = j.step(ID, "Notifications, Shortcuts, Decisions, AI and agents, Outside apps")
    s.click(W + ".pane-row.notifications")
    before = {}
    s.expect("notch-switch-flips", lambda: (before.update(h=settings(s)["hidden_in_screen_share"]), click(s, G + ".notch.hidden"), settings(s)["hidden_in_screen_share"] != before["h"])[2], True)
    s.click(G + ".notch.hidden")
    s.click(W + ".pane-row.shortcuts")
    s.expect("hint-switch-flips", lambda: (before.update(h=settings(s)["hints"]), click(s, G + ".shortcuts.hints"), settings(s)["hints"] != before["h"])[2], True)
    s.click(G + ".shortcuts.hints")
    s.click(W + ".pane-row.decisions")
    for e in ("jev", "off", "local"):
        held = {}
        if e == "jev":      # the engine chosen before the click: a Jev that is refused leaves it as it was
            try:
                held["before"] = engine_chosen(s, e)
            except Unreadable:
                held["before"] = None
        s.click("%s.decisions.engine.%s" % (D, e))
        if e == "jev":
            # main79: the check chose Jev with no key and no consent, and the app refused ("Allow the text of the mail ... or choose the model on this Mac", the core's refusal for a
            # hosted model, contracts.swift:33808) and kept Laya. A key and the consent are what settingsaimac's J5.7 stores through a loopback stub; this window check has no stub
            # and never signs in anywhere, so it asks what the app must do here: refuse in words, choose nothing new.
            expect_ev(s, "engine-jev", lambda: jev_refused_without_key_and_consent(s, held["before"]), EXPECT["engine-jev"], reads_engine(s))
        else:
            expect_ev(s, "engine-" + e, lambda e=e: engine_chosen(s, e), [e], reads_engine(s))
    s.click(D + ".decisions.engine.jev")      # Jev is TypeSafe's API: its row carries TypeSafe's mark (SPEC A81)
    s.expect("provider-mark-jev", lambda: drawn(s, D + ".provider.jev.mark"), True)
    s.expect("provider-logo-jev", lambda: engine_logo(s, "jev"), LOGOS["jev"])
    s.click(D + ".decisions.engine.local")
    # the Model row (Laya, on this Mac) is drawn as the owner picked it (W1, board ai-setup-r1; DecisionsHomeView.swift:55 `LayaRow`: the name, one grey line, one button, NO vendor
    # mark; the Jev row keeps its mark). main79 expected a Nexus mark there and read "no rows at mac-settings-ai.provider.laya": the row is `mac-settings-ai.decisions.local`.
    expect_ev(s, "provider-mark-laya", lambda: (model_row_drawn(s), drawn(s, D + ".provider.laya.mark")), EXPECT["provider-mark-laya"], reads_laya_mark(s))
    s.expect("provider-logo-laya", lambda: engine_logo(s, "local"), LOGOS["laya"])
    s.expect("decisions-lists-what-it-files", lambda: [r["id"] for r in s.ctl.rows(D + ".decisions.pages")],
             ["which-tab", "needs-reply", "sign-in-codes", "vips", "notify-me", "labels"])
    s.click(W + ".pane-row.ai")
    for slug in PROVIDER_MARKS:           # each provider row carries the vendor's own mark beside its name (SPEC A81)
        s.expect("provider-mark-" + slug, lambda slug=slug: drawn(s, "%s.provider.%s.mark" % (D, slug)), True)
    for slug in ("anthropic", "openai", "gemini", "openrouter", "chatgpt"):    # the row's probe names the logo asset it draws (SPEC A81)
        s.expect("provider-logo-" + slug, lambda slug=slug: s.ctl.rows("%s.provider.%s" % (D, slug))[0].get("logo"), LOGOS[slug])
    s.expect("logo-probe-lists-every-vendor", lambda: sorted((r["slug"], r["logo"]) for r in s.ctl.rows(D + ".logos")), sorted(LOGOS_ALL.items()))
    s.click(D + ".speech.local")
    s.expect("speech-on-this-mac", lambda: s.ctl.rows(D + ".speech")[0]["state"], "ready")
    s.click(W + ".pane-row.outside")
    s.expect("outside-apps-lists-the-apps", lambda: len(s.ctl.rows(M + ".apps")) >= 2, True)
    app = lambda: s.ctl.rows(M + ".apps")[0]
    s.expect("allow-an-app", lambda: (click(s, app()["allow"]), [r for r in s.ctl.rows(M + ".apps")][0]["state"])[1], "allowed")
    s.expect("revoke-an-app", lambda: (click(s, s.ctl.rows(M + ".apps")[0]["revoke"]), [r["state"] for r in s.ctl.rows(M + ".apps")][0])[1], "not allowed")

    s.click(W + ".pane-row.accounts")

    def no_coloured_edge():
        time.sleep(0.8)                  # let the pane switch finish before the shot
        path = s.ctl.send("shot", "mac-settings-accounts")["path"]
        img, ww = read_png(path), settings_window(s)["w"]
        f = frame(s, A + ".failing")
        out = []
        for dx in (0.5, 1.5, 2.5):
            c = pixel(img, ww, f["x"] + dx, f["y"] + f["h"] / 2)
            out.append(max(c) - min(c))
        return max(out) < 40            # a card's edge is a grey hairline; an orange strip spreads the channels by over 100
    s.expect("needs-you-card-has-no-coloured-edge", no_coloured_edge, True)
    s.click(W + ".pane-row.outside")
    s.expect("outside-app-icons-are-the-real-app-or-absent", lambda: sorted({r["icon"] for r in s.ctl.rows(M + ".apps")} - {"bundle", "absent"}), [])

    def request_waits_and_is_answered():
        s.ctl.click(M + ".allow.claude-desktop")
        s.ctl.send("fixture", "mcp-request", "Claude Desktop", "send_reply", "send a reply")
        waiting = s.ctl.rows(M + ".requests")
        icon = [r["icon"] for r in waiting]
        ok = click(s, waiting[0]["allow"]) if waiting else "no request waiting"
        return (len(waiting), icon[0] in ("bundle", "absent"), ok, s.ctl.rows(M + ".requests"))
    s.expect("a-request-waits-and-allow-answers-it", request_waits_and_is_answered, (1, True, True, []))
    s.click(M + ".revoke.claude-desktop")
    s.click(G + ".appearance.language.system")

    # ---- 6b the compact width: no label is cut, no title wraps ---------------------------------------------------------------
    s = j.step(ID, "No label narrower than its longest word, a title on one row: 700x480 and 940x680, both themes, every pane")

    def fits():
        bad, seen = [], 0
        for theme in ("light", "dark"):
            s.ctl.send("theme", theme)
            for size in (".size.min", ".size.default"):
                s.ctl.click(W + size)
                for p in PANES + ["account-page"]:
                    if p == "account-page":
                        s.ctl.click(W + ".pane-row.accounts")
                        s.ctl.click(A + ".acc-18")
                    else:
                        s.ctl.click("%s.pane-row.%s" % (W, p))
                    time.sleep(0.7)
                    for r in s.ctl.rows(W + ".rowfit"):
                        seen += 1
                        if r["label_width"] + 0.5 < r["longest_word"]:
                            bad.append((theme, size, p, r["id"], r["label_width"], r["longest_word"]))
                    if p != "account-page":
                        h = frame(s, head(p)[1])["h"]
                        if h > 28:                                   # one line of the 17 pt title is 22 high
                            bad.append((theme, size, p, "title wraps", h))
        s.ctl.send("theme", "light")
        s.ctl.click(W + ".size.default")
        s.ctl.click(W + ".pane-row.general")
        return bad if seen > 100 else bad + ["only %d rows read" % seen]
    s.expect("no-label-is-narrower-than-its-longest-word-and-no-title-wraps", fits, [])

    # ---- 6c the toolbar row is below the title bar band, and the pane starts below the toolbar (owner, main65: "top is not viewable") ------------
    s = j.step(ID, "The toolbar row clears the title bar and the pane clears the toolbar: every pane and every part of Replies, 940x680 and 700x480")

    def toolbar_clears_the_band():
        bad, seen = [], 0
        places = [(p, W + ".pane-row." + p, head(p)[0]) for p in PANES if p != "replies"]
        places += [("replies." + part, "%s.pane-row.replies.%s" % (W, part), "%s.page.%s" % (R, part)) for part in REPLY_PARTS]
        places.append(("replies.filter", R + ".page.filter", R + ".page.filter"))
        for size in (".size.default", ".size.min"):
            s.ctl.click(W + size)
            for name, door, first in places:
                s.ctl.click(door)
                time.sleep(0.4)
                band = s.ctl.rows(W + ".chrome")[0]["titlebar_h"]
                tb, fr = frame(s, W + ".toolbar"), frame(s, first)
                seen += 1
                if tb["y"] + 0.5 < band:
                    bad.append((size, name, "toolbar top %.1f is above the title bar's bottom %.1f" % (tb["y"], band)))
                if fr["y"] + 0.5 < tb["y"] + tb["h"]:
                    bad.append((size, name, "content top %.1f overlaps the toolbar's bottom %.1f" % (fr["y"], tb["y"] + tb["h"])))
        s.ctl.click(W + ".size.default")
        s.ctl.click(W + ".pane-row.general")
        return bad if seen >= 2 * (len(PANES) + 6) else bad + ["only %d places measured" % seen]
    s.expect("toolbar-row-clears-the-title-bar-and-the-pane-clears-the-toolbar", toolbar_clears_the_band, [])

    # ---- 6d the top of the Quels messages page (item b1d38560): the toolbar title and the FIRST card are whole at both sizes -------------------------
    def filter_top_is_whole():
        bad, seen = [], []
        for size, label in ((".size.default", "940x680"), (".size.min", "700x480")):
            s.ctl.click(W + size)
            s.ctl.click(R + ".page.filter")
            time.sleep(0.4)
            win = settings_window(s)
            band = s.ctl.rows(W + ".chrome")[0]["titlebar_h"]
            # the page draws its cards when the core's filter answers (a later turn than the click): read the first card again, bounded (30 reads, 0.1 s
            # apart); the last read's words are the not-judged reason when it never draws (main88: "nothing on screen carries that id" after one 0.4 s wait)
            for tries in range(30):
                try:
                    card = frame(s, R + ".filter.entry.card")   # `filter.entry` is a plain accessibilityIdentifier on the shell (RepliesTemporaryShared.swift:72); the card box carries whooshAx(".card") (:53)
                    break
                except Unreadable:
                    if tries == 29:
                        raise
                    time.sleep(0.1)
            tb = frame(s, W + ".toolbar")
            seen.append("%s: window %dx%d band %.0f toolbar y=%.0f h=%.0f first card x=%.0f y=%.0f w=%.0f h=%.0f" % (
                label, round(win["w"]), round(win["h"]), band, tb["y"], tb["h"], card["x"], card["y"], card["w"], card["h"]))
            if tb["y"] + 0.5 < band or tb["y"] + tb["h"] > win["h"]:
                bad.append("%s: the toolbar title row is cut (y %.1f, h %.1f, band %.1f)" % (label, tb["y"], tb["h"], band))
            if card["y"] + 0.5 < tb["y"] + tb["h"] or card["y"] >= win["h"]:
                bad.append("%s: the first card's top %.1f is under the toolbar's bottom %.1f or below the window's foot %.0f" % (label, card["y"], tb["y"] + tb["h"], win["h"]))
            if card["x"] < -0.5 or card["x"] + card["w"] > win["w"] + 0.5 or card["w"] <= 0 or card["h"] <= 0:
                bad.append("%s: the first card lies outside the window's width (x %.1f, w %.1f of %.0f)" % (label, card["x"], card["w"], win["w"]))
            s.shot("par-settingswindow-filter-page-" + label)
        s.ctl.click(W + ".size.default")
        s.ctl.click(W + ".pane-row.general")
        return bad, seen

    def filter_top_case():
        bad, seen = filter_top_is_whole()
        return not bad, ("; ".join(bad) if bad else "whole") + " | measured: " + " | ".join(seen)
    s._record("filter-page-top-and-first-card-are-whole-at-940x680-and-700x480", filter_top_case)

    # ---- 7 every control, once ----------------------------------------------------------------------------------------
    s = j.step(ID, "Every control the window lists is clicked once and changes something")
    controls = lambda: [r for r in s.ctl.rows(W + ".controls")]
    s.expect("controls-are-listed-for-every-built-pane", lambda: sorted({r["pane"] for r in controls()}),
             sorted({"window", "general", "notifications", "shortcuts", "accounts", "account-page", "decisions", "ai", "outside"}))

    def sweep():
        silent, clicked = [], 0
        groups = {}
        for r in controls():
            # clicked by their own checks above: the sheet and the code screen (accounts check), the language and theme choices, pin and unpin
            # (par-settingsgeneralmac), the search clear and the summary (search and panes steps), back and forward (back and forward step)
            if (r["ax"].endswith((".remove", ".icon", ".connect", ".speech.local", ".add", ".move-to-iphone", ".summary", ".search.clear", ".back", ".forward"))
                    or ".pinned.label." in r["ax"] or r["kind"] == "pane"):
                continue
            groups.setdefault((r["kind"], r["ax"].rsplit(".", 1)[0] if r["kind"] in ("choice", "segment") else r["ax"]), []).append(r["ax"])
        for (kind, _), axes in groups.items():
            axes = axes[-1:] + axes[:-1] if kind in ("choice", "segment") else axes
            for i, ax in enumerate(axes):
                if ".language." in ax or ".theme." in ax:
                    continue
                before = snapshot(s)
                ok = click(s, ax)
                clicked += 1
                changed = snapshot(s) != before
                if ok is not True or (not changed and not (i == 0 and kind in ("choice", "segment"))):
                    silent.append((ax, ok if ok is not True else "no visible change"))
        return silent if clicked > 0 else ["no control was clicked"]
    s.expect("every-listed-control-does-something-when-clicked", sweep, [])

    # ---- 7b doors-settings: the window's own controls the sweep skips ----------------------------------------------------------
    s = j.step(ID, "Doors: the search clear button and a result row")

    def clear_button():
        c = s.ctl
        c.click(W + ".fixture.query.show")
        had = place(s)["query"]
        c.click(W + ".search.clear")
        return (had, place(s)["query"], c.rows(W + ".search"))
    s.expect("search-clear-button-empties-the-query", clear_button, ("show", "", []))

    def result_door():
        c = s.ctl
        c.click(W + ".fixture.query.emblem")      # opens General and rings the Emblems switch
        c.click(W + ".pane-row.accounts")         # leave: the result row must bring us back
        c.click(W + ".result.0")
        p = place(s)
        return (p["pane"], p["ring"], p["query"])
    s.expect("result-row-door-opens-its-pane-and-rings-its-row", result_door, ("general", G + ".appearance.emblems", ""))

    def arrows_and_return():
        c = s.ctl
        c.click(W + ".fixture.query.show")        # several hits: the first is chosen, its pane open and its row rung
        first = place(s)
        c.click(W + ".search.down")
        down = place(s)
        moved = (c.rows(W + ".search")[0]["selected"], c.rows(W + ".search")[1]["selected"], down["ring"] != first["ring"])
        c.click(W + ".search.up")
        back = (c.rows(W + ".search")[0]["selected"], place(s)["ring"] == first["ring"])
        c.click(W + ".pane-row.accounts")         # leave the pane: Return must bring the chosen row back and the panes with it
        c.click(W + ".search.commit")
        p = place(s)
        return (moved, back, (p["pane"] == first["pane"], p["ring"] == first["ring"], p["query"]))
    s.expect("search-arrows-and-return-doors-choose-and-commit", arrows_and_return,
             ((False, True, True), (True, True), (True, True, "")))
    s.click(W + ".pane-row.general")

    # ---- 8 weight ------------------------------------------------------------------------------------------------------
    s = j.step(ID, "Footprint: no idle CPU, views freed when the window closes")

    def ps_rows():
        out = subprocess.run(["ps", "-axo", "pid=,rss=,pcpu=,command="], capture_output=True, text=True).stdout.splitlines()
        rig = os.path.dirname(os.environ.get("WHOOSH_SOCKET_DIR", "").rstrip("/"))
        return [l.split(None, 3) for l in out if "/Contents/MacOS/" in l and rig and rig in l]

    def cpu_seconds():
        out = subprocess.run(["ps", "-axo", "pid=,cputime=,command="], capture_output=True, text=True).stdout.splitlines()
        rig = os.path.dirname(os.environ.get("WHOOSH_SOCKET_DIR", "").rstrip("/"))
        total = 0.0
        for l in out:
            if "/Contents/MacOS/" in l and rig and rig in l:
                t = l.split(None, 2)[1].replace(",", ".").split(":")
                total += sum(float(x) * m for x, m in zip(reversed(t), (1, 60, 3600)))
        return total

    def idle_cpu():
        if not ps_rows():
            raise Unreadable("the rig process was not found by its path")
        s.ctl.click(W + ".pane-row.general")
        time.sleep(2.0)
        a, t0 = cpu_seconds(), time.time()
        time.sleep(12.0)
        return round((cpu_seconds() - a) / (time.time() - t0) * 100, 2)
    s.expect_le("idle-cpu-with-the-window-open", idle_cpu, 1.0)

    def views_after_close():
        s.ctl.click(W + ".open")
        for p in PANES:
            s.ctl.click("%s.pane-row.%s" % (W, p))
        opened = s.ctl.rows(W + ".footprint")[0]["views_alive"]
        s.ctl.click(W + ".close")
        time.sleep(2.0)
        closed = s.ctl.rows(W + ".footprint")[0]["views_alive"]
        s.ctl.click(W + ".open")
        return (opened >= 1, closed)
    s.expect("closing-the-window-frees-its-views", views_after_close, (True, 0))

    def typing_after_settings_closes():
        """typing-lost-r2: the Settings view stays observed after its window closes. The search field was the target (⌘F), the window closes,
        text typed in the add-account sheet afterwards must land in the sheet's address field, and the reopened Settings search stays empty."""
        s.ctl.click(W + ".open")
        s.ctl.key("⌘F")
        s.ctl.click(W + ".close")
        for ax in ("dock.whoosh", "mac-main.sidebar.accounts-menu.up", "mac-main.sidebar.accounts-menu.add", "mac-add-account.address"):
            s.ctl.click(ax)
        s.ctl.type(TYPED_PROBE)
        rows = s.ctl.rows("mac-add-account.address")
        text = rows[0].get("text") if rows else None
        s.ctl.key("Escape")
        s.ctl.click(W + ".open")
        left = s.ctl.rows(W + ".search")
        s.ctl.click(W + ".close")
        return (text, left)
    s.expect("typing-after-settings-closes-reaches-the-next-field", typing_after_settings_closes, (TYPED_PROBE, []))


# MUTANTS: per check, the mutation of the app that must turn it red.
# par-settingswindow/provider-mark-anthropic | the anthropic mark is removed from its provider row
# par-settingswindow/provider-mark-openai | the openai mark is removed from its provider row
# par-settingswindow/provider-mark-gemini | the gemini mark is removed from its provider row
# par-settingswindow/provider-mark-openrouter | the openrouter mark is removed from its provider row
# par-settingswindow/provider-mark-chatgpt | the chatgpt mark is removed from its provider row
# par-settingswindow/provider-mark-jev | the Nexus mark is removed from the Jev row
# par-settingswindow/provider-mark-laya | the Model row (Laya) draws a vendor mark again, against the owner's pick W1, or draws nothing at all
# par-settingswindow/provider-logo-anthropic | the Anthropic row draws the generic mark (its probe serves the logo asset "generic")
# par-settingswindow/provider-logo-openai | the OpenAI row draws the generic mark (its probe serves the logo asset "generic")
# par-settingswindow/provider-logo-gemini | the Gemini row draws the generic mark (its probe serves the logo asset "generic")
# par-settingswindow/provider-logo-openrouter | the OpenRouter row draws the generic mark (its probe serves the logo asset "generic")
# par-settingswindow/provider-logo-chatgpt | the ChatGPT row draws the generic mark (its probe serves the logo asset "generic")
# par-settingswindow/provider-logo-jev | the Jev row draws the generic mark (its probe serves the logo asset "generic")
# par-settingswindow/provider-logo-laya | the Model row draws the generic mark (its probe serves the logo asset "generic")
# par-settingswindow/logo-probe-lists-every-vendor | a vendor's logo asset is renamed away from its file or dropped from the list
# par-settingswindow/window-opens-on-the-settings-surface | the Settings scene mounts the old tab view, not the window
# par-settingswindow/default-size-940x680 | the window opens at another size
# par-settingswindow/sidebar-250-at-default | the sidebar is 260 wide
# par-settingswindow/toolbar-nav-buttons-28 | the back and forward buttons are 24 wide
# par-settingswindow/search-field-28-high | the search field is 32 high
# par-settingswindow/pane-row-28-high | a pane row is 32 high
# par-settingswindow/pane-header-icon-40 | the pane header has no 40 pt square
# par-settingswindow/min-size-700x480 | the minimum is 600x400
# par-settingswindow/sidebar-210-at-min | the sidebar keeps 250 at the minimum
# par-settingswindow/summary-card-is-drawn-at-the-default-size | the summary card is never drawn
# par-settingswindow/summary-card-gives-way-at-the-minimum | the summary card stays at the minimum height
# par-settingswindow/every-pane-row-fits-the-min-window | the sidebar column grows past the window's foot instead of scrolling, or a row in the middle of the order falls below it
# par-settingswindow/back-at-default-size | the window keeps the minimum width after resizing back
# par-settingswindow/ten-panes-in-four-groups | Junk sits under Mail
# par-settingswindow/pane-names | Outside apps is called MCP
# par-settingswindow/pane-keys-cmd-1-to-0 | Outside apps takes ⌘9
# par-settingswindow/every-pane-has-a-symbol | a pane has no symbol
# par-settingswindow/summary-card-counts | the summary card counts 12 accounts
# par-settingswindow/accounts-row-badge-counts-who-needs-you | the badge counts every account
# par-settingswindow/pane-icons-are-coloured-squares | the pane squares are grey
# par-settingswindow/selected-row-is-blue-and-others-are-not | the selected row is not filled
# par-settingswindow/click-every-pane-row-shows-its-pane | Viewing opens Signatures
# par-settingswindow/cmd-1-to-cmd-0-switch-the-panes | ⌘3 does nothing
# par-settingswindow/cmd-digit-by-key-code-switches-the-panes-on-azerty | a pane key is read by the character the layout types, so ⌘é (the 2 key) selects nothing
# par-settingswindow/summary-card-opens-accounts | the summary card is not a button
# par-settingswindow/placeholder-panes-say-what-comes-and-hold-no-control | Viewing shows a button that does nothing
# par-settingswindow/back-and-forward-start-at-the-pane-last-reached | forward is enabled with no history
# par-settingswindow/back-enabled-after-a-move | a move does not record history
# par-settingswindow/back-returns-to-the-previous-pane | back goes home
# par-settingswindow/forward-returns | forward does nothing
# par-settingswindow/cmd-bracket-left-goes-back | ⌘[ is not bound
# par-settingswindow/cmd-bracket-right-goes-forward | ⌘] is not bound
# par-settingswindow/replies-parts-are-listed-under-replies-and-each-opens | a part of Replies is missing from the sidebar, or a click opens another part
# par-settingswindow/back-and-forward-walk-the-parts-of-replies | back leaves Replies instead of returning to the part before
# par-settingswindow/escape-goes-to-the-open-pane-first-then-the-window | Escape clears the window's search while a Replies editor is open and the editor stays open
# par-settingswindow/replies-is-closed-while-another-pane-is-selected | the six parts are listed under Replies while General is selected
# par-settingswindow/a-click-on-the-parent-opens-it-and-selects-its-first-part | a click on the closed parent selects Replies and leaves it closed
# par-settingswindow/the-chevron-opens-and-closes-without-changing-the-pane | the chevron selects Replies, or does nothing
# par-settingswindow/selecting-another-pane-closes-replies-and-it-is-not-remembered | Replies stays open under Accounts
# par-settingswindow/at-940x680-with-replies-closed-no-pane-row-is-below-the-foot | the six parts are listed at 940 x 680 and Notifications falls below the foot
# par-settingswindow/at-940x680-with-replies-open-nothing-in-the-sidebar-is-below-the-foot | the card stays while Replies is open and Shortcuts or Outside apps fall below the foot
# par-settingswindow/toolbar-row-clears-the-title-bar-and-the-pane-clears-the-toolbar | the toolbar row sits at the top of the window under the title bar band, so only its lowest points show
# par-settingswindow/filter-page-top-and-first-card-are-whole-at-940x680-and-700x480 | the Quels messages page body has a large negative top padding (FilterPage.swift .padding(.top, -400)) so its first card is cut under the toolbar
# par-settingswindow/back-with-nowhere-to-go-does-nothing-and-says-so | forward moves with no forward history
# par-settingswindow/search-finds-the-emblems-switch-first | search is case sensitive
# par-settingswindow/first-hit-opens-its-pane-and-rings-its-row | the first hit opens no pane
# par-settingswindow/the-ringed-row-is-on-screen | the ring marks a row that is not drawn
# par-settingswindow/every-found-setting-is-scrolled-into-view | a found group, button or footnote carries no scroll id: it is ringed where it stands, below the foot
# par-settingswindow/results-show-their-path | a result shows no path
# par-settingswindow/results-count-says-how-many | the result count line is missing
# par-settingswindow/esc-clears-the-search-and-the-ring | Esc leaves the query
# par-settingswindow/no-match-says-so | a query with no match shows an empty list and no words
# par-settingswindow/every-setting-finds-itself-by-its-own-name | the index leaves one setting out
# par-settingswindow/search-works-in-french | the index reads English words under French
# par-settingswindow/up-and-down-move-the-chosen-result | the chosen result never moves
# par-settingswindow/the-theme-the-rig-starts-with-is-read | the rig theme cannot be read
# par-settingswindow/appearance-dark | the dark choice sets light
# par-settingswindow/appearance-light | the light choice does nothing
# par-settingswindow/appearance-auto | auto keeps the last choice
# par-settingswindow/density-compact | the density choice is not saved
# par-settingswindow/density-regular | the density choice is not saved
# par-settingswindow/motion-reduced | the motion choice is not saved
# par-settingswindow/motion-full | the motion choice is not saved
# par-settingswindow/undo-send-5 | the undo choice is not saved
# par-settingswindow/undo-send-10 | the undo choice is not saved
# par-settingswindow/undo-send-15 | the undo choice is not saved
# par-settingswindow/undo-send-20 | the undo choice is not saved
# par-settingswindow/undo-send-25 | the undo choice is not saved
# par-settingswindow/undo-send-30 | the undo choice is not saved
# par-settingswindow/every-searchable-row-is-drawn-in-its-pane | the index lists a row no pane draws
# par-settingswindow/language-fr-turns-the-window-and-the-main-screen-french | the window keeps English after the choice
# par-settingswindow/back-to-system-gives-the-macs-pane-names-again | System keeps French
# par-settingswindow/notch-switch-flips | the notch switch does nothing
# par-settingswindow/hint-switch-flips | the hint switch does nothing
# par-settingswindow/engine-jev | Jev is chosen with no key and no consent, or is refused with no words
# par-settingswindow/engine-off | the Off segment does nothing
# par-settingswindow/engine-local | the Laya segment does nothing
# par-settingswindow/decisions-lists-what-it-files | Decisions lists no page
# par-settingswindow/speech-on-this-mac | the speech button does nothing
# par-settingswindow/outside-apps-lists-the-apps | the apps list is empty
# par-settingswindow/allow-an-app | Allow does nothing
# par-settingswindow/revoke-an-app | Revoke does nothing
# par-settingswindow/needs-you-card-has-no-coloured-edge | the needs-you card draws an orange strip on its left edge
# par-settingswindow/outside-app-icons-are-the-real-app-or-absent | an outside app is drawn as a coloured letter square
# par-settingswindow/a-request-waits-and-allow-answers-it | Allow leaves the request waiting
# par-settingswindow/no-label-is-narrower-than-its-longest-word-and-no-title-wraps | the buttons stay in the header row and the form row never stacks
# par-settingswindow/controls-are-listed-for-every-built-pane | a pane lists no control
# par-settingswindow/every-listed-control-does-something-when-clicked | one switch is wired to nothing
# par-settingswindow/search-clear-button-empties-the-query | the clear button leaves the query in the field
# par-settingswindow/result-row-door-opens-its-pane-and-rings-its-row | a click on a result opens no pane
# par-settingswindow/search-arrows-and-return-doors-choose-and-commit | Down does not move the chosen result, Up does not move it back, or Return opens no pane
# par-settingswindow/idle-cpu-with-the-window-open | the window runs a timer
# par-settingswindow/closing-the-window-frees-its-views | the window keeps its views after it closes (the content is not dropped on disappear)
# par-settingswindow/typing-after-settings-closes-reaches-the-next-field | closed-settings-keeps-taking-the-typed-text: the typed observer has no focus guard, so the closed window's search field takes the shared buffer and the sheet's address reads ""

# ---- --selftest: the logo wiring read from the Swift sources (no app, no window) ------------------------------------------
SRC = {"kit": "Sources/Features/SettingsWindow/SettingsKit.swift", "scaffold": "Sources/Features/SettingsWindow/SettingsPaneScaffold.swift",
       "marks": "Sources/Features/SettingsWindow/ProviderMarks.swift", "providers": "Sources/Features/SettingsAIMac/ProvidersView.swift",
       "decisions": "Sources/Features/SettingsAIMac/DecisionsHomeView.swift", "store": "Sources/Features/SettingsAIMac/AISettingsStore.swift",
       "view": "Sources/Features/SettingsWindow/SettingsWindowView.swift", "model": "Sources/Features/SettingsWindow/SettingsWindowModel.swift",
       "metrics": "Sources/Features/SettingsWindow/SettingsMetrics.swift", "registry": "Sources/Features/SettingsWindow/SettingsWindowRegistry.swift", "sidebar": "Sources/Features/SettingsWindow/SettingsSidebar.swift", "pane": "Sources/Features/SettingsWindow/SettingsPane.swift"}
MARKS_DIR = os.path.join("Sources", "Features", "SettingsWindow", "Marks")


def asset_table(marks_swift):
    """{mark case: asset stem} from the `var asset` switch of ProviderMarks.swift."""
    import re
    m = re.search(r"var asset: String \{(.*?)\n    \}", marks_swift, re.S)
    out = {}
    for line in (m.group(1) if m else "").splitlines():
        c = re.match(r'\s*case ((?:\.\w+(?:, )?)+): "([^"]*)"', line)
        if c:
            for case in re.findall(r"\.(\w+)", c.group(1)):
                out[case] = c.group(2)
    return out


def served_logos(src):
    """{row slug: the logo asset the row draws, or None when the row draws no ProviderMarkView}, from the sources alone."""
    import re
    assets = asset_table(src["marks"])
    cases = re.search(r"enum ProviderMark: String, CaseIterable \{\s*case ([^\n]*)", src["marks"])
    cases = [c.strip() for c in cases.group(1).split(",")] if cases else []
    block = re.search(r"shownProviders = \[(.*?)\]\n", src["store"], re.S)      # the rows, as ("id", "Name") pairs
    ids = re.findall(r'\("(\w+)",', block.group(1) if block else "")
    generic_row = "ProviderMark.of(provider.id).map { AnyView(ProviderMarkView(mark: $0)) }" in src["providers"]
    out = {i: (assets.get(i) if generic_row and i in cases else None) for i in ids}

    def wired(text, anchor, span=r"[^\n]{0,300}?"):      # a row's mark sits on the row's own line unless `span` says it continues
        m = re.search(re.escape(anchor) + span + r"ProviderMarkView\(mark: \.(\w+)", text, re.S)
        return assets.get(m.group(1)) if m else None
    out["chatgpt"] = wired(src["providers"], 'provider.chatgpt"', r".{0,200}?")
    out["jev"] = wired(src["decisions"], 'decisions.jev.key"', r".{0,300}?")     # the mark sits on the `leading:` line below the row's id
    out["laya"] = wired(src["decisions"], 'decisions.local"', r".{0,200}?")
    return out


def read_sources(root):
    return {k: open(os.path.join(root, v), encoding="utf-8").read() for k, v in SRC.items()}


def logo_mutants():
    """(label, source key, find, replace): each makes ONE row draw the generic mark."""
    rows = [("anthropic", "marks", 'case .anthropic: "anthropic"'), ("openai", "marks", 'case .openai, .chatgpt: "openai"'),
            ("gemini", "marks", 'case .gemini: "gemini"'), ("openrouter", "marks", 'case .openrouter: "openrouter"')]
    out = [(slug, key, find, find.rsplit(":", 1)[0] + ': "generic"') for slug, key, find in rows]
    out.append(("chatgpt", "providers", "ProviderMarkView(mark: .chatgpt)", 'Image(systemName: "envelope")'))
    out.append(("jev", "decisions", "ProviderMarkView(mark: .jev)", 'Image(systemName: "envelope")'))
    return out


def typed_observer_guarded(src):
    """typing-lost-r2, from the sources alone: the Settings view's typed observer takes the buffer only while the window holds the keys,
    and `holdsKeys` is the surface test (a closed window's surface is another one)."""
    import re
    m = re.search(r"\.onChange\(of: app\.typed\)(.*?)\n        \.onAppear", src["view"], re.S)
    h = re.search(r"static func holdsKeys\(_ m: AppModel\) -> Bool \{ (.*?) \}", src["model"])
    ok_view = bool(m) and "guard SettingsWindowModel.holdsKeys(app)" in m.group(1) and m.group(1).index("holdsKeys") < m.group(1).index("absorbTyped")
    return ok_view and bool(h) and h.group(1) == 'm.surface.hasPrefix("mac-settings")'


def typed_mutants():
    """(label, source key, find, replace): the closed window's view takes the shared buffer again."""
    return [("closed-settings-keeps-taking-the-typed-text", "view", "guard SettingsWindowModel.holdsKeys(app) else { model.releaseTyped(); return }\n            ", ""),
            ("holds-keys-is-always-true", "model", 'm.surface.hasPrefix("mac-settings")', "true")]


def found_has_scroll_target(src):
    """From the sources alone: every element that can wear the found ring, and every element that takes an optional socket id (a group's card, a footnote),
    carries the id the pane's column scrolls to; the column scrolls to the ring."""
    import re
    kit, scaffold = src["kit"], src["scaffold"]
    ring = re.search(r"struct SettingsRing: ViewModifier \{(.*?)\n\}\n", kit, re.S)
    target = re.search(r"struct SettingsScrollTarget: ViewModifier \{(.*?)\n\}\n", kit, re.S)
    optional = re.search(r"struct SettingsOptionalAx: ViewModifier \{(.*?)\n\}\n", kit, re.S)
    return (bool(ring) and ".modifier(SettingsScrollTarget(ax: ax))" in ring.group(1)
            and bool(target) and "content.background { Color.clear.id(ax) }" in target.group(1)      # inside the element: a root id is lost in a card of rows
            and bool(optional) and ".modifier(SettingsScrollTarget(ax: ax))" in optional.group(1)
            and ".task(id: model.ring)" in scaffold                              # at each new ring, and when a pane opens already ringed
            and scaffold.count("proxy.scrollTo(ring, anchor: .center)") == 2)      # once animated after the layout, once more without animation


def found_scroll_mutants():
    """(label, source key, find, replace): a ringed element, or a group's card, loses the id the column scrolls to; or the column no longer scrolls."""
    return [("a-ringed-element-has-no-scroll-target", "kit", '            .accessibilityValue(on ? Words.text("Found") : "")\n            .modifier(SettingsScrollTarget(ax: ax))', '            .accessibilityValue(on ? Words.text("Found") : "")'),
            ("a-group-card-has-no-scroll-target", "kit", "content.whooshAx(ax).modifier(SettingsScrollTarget(ax: ax))", "content.whooshAx(ax)"),
            ("the-scroll-target-sets-no-id", "kit", "content.background { Color.clear.id(ax) } }", "content }"),
            ("the-scroll-target-sits-on-the-root", "kit", "content.background { Color.clear.id(ax) } }", "content.id(ax) }"),
            ("the-column-does-not-scroll-to-the-ring", "scaffold", "withAnimation(.easeInOut(duration: WhooshMotion.readerCrossFade)) { proxy.scrollTo(ring, anchor: .center) }", ""),
            ("the-scroll-is-asked-only-when-the-ring-changes", "scaffold", ".task(id: model.ring) {", ".onChange(of: model.ring) { _, _ in Task {"),
            ("the-second-ask-is-gone", "scaffold", "                guard !Task.isCancelled, model.ring == ring else { return }\n                proxy.scrollTo(ring, anchor: .center)", "")]


def hover_tint_follows_theme(src):
    """From the source alone: a Settings button's hover tint is the theme's text colour (a dark tint on the light theme, a light tint on the dark one). A black
    tint showed on the light theme only (the lead's sweep of fixed colours, 14 Nov 2026)."""
    kit = src["kit"]
    return "WhooshPalette.label.opacity(hover ? " in kit and ".black.opacity(hover" not in kit


def hover_tint_mutants():
    return [("the-hover-tint-is-black", "kit", "Capsule().fill(WhooshPalette.label.opacity(hover ? 0.05 : 0))", "Capsule().fill(Color.black.opacity(hover ? 0.05 : 0))")]


def digit_keys_by_code(src):
    """A107, from the sources alone: the Settings window reads ⌘1 to ⌘0 through ChordKit.chord (key codes) in a local key monitor of its own
    window, and no hidden button binds a pane by the typed character."""
    v = src["view"]
    mon = "struct SettingsDigitKeys" in v and "addLocalMonitorForEvents(matching: .keyDown)" in v and "e.window === w" in v
    by_code = "let chord = ChordKit.chord(e)" in v and "$0.chord == chord" in v
    by_char = "keyboardShortcut(KeyEquivalent(Character(pane.key))" in v
    return mon and by_code and not by_char


def digit_mutants():
    """(label, source key, find, replace): the panes go back to the typed character, or the monitor reads the character."""
    return [("the-monitor-reads-the-typed-character", "view", "let chord = ChordKit.chord(e)", 'let chord = "⌘" + (e.charactersIgnoringModifiers ?? "")'),
            ("a-hidden-button-binds-each-pane-by-character", "view", "        #if os(macOS)\n        .background(SettingsDigitKeys(model: model))",
             "        .background(ForEach(SettingsPane.allCases) { pane in Button(\"\") {}.keyboardShortcut(KeyEquivalent(Character(pane.key)), modifiers: .command) })\n        #if os(macOS)\n        .background(SettingsDigitKeys(model: model))"),
            ("no-monitor-on-the-window", "view", "e.window === w", "e.window != nil")]


def escape_yields(src):
    """Round 3, from the sources alone: the window's Escape offers the key to the open Replies pane first (close a popover, leave an editor)
    and clears its own search only when the pane did not take it."""
    r = src["registry"]
    pane = 'SettingsRepliesMacFeature.keyBindings()["Escape"]?(m) == true { return true }'
    own = "guard model.searching else { return false }"
    return pane in r and own in r and r.index(pane) < r.rindex(own)


def escape_mutants():
    pane = '            if model.pane == .replies, SettingsRepliesMacFeature.keyBindings()["Escape"]?(m) == true { return true }\n'
    return [("the-window-escape-does-not-ask-the-pane", "registry", pane, ""),
            ("the-window-escape-asks-the-pane-last", "registry", pane + "            guard model.searching else { return false }",
             "            guard model.searching else { return false }\n" + pane.replace("            if model.pane", "            if model.pane"))]


def replies_open_rule(src):
    """Round 5, from the sources: the parts are listed only while Replies is open; opening is selecting Replies or a part, or the chevron;
    any move to another pane closes it; the sidebar draws the parts under that condition only."""
    m, sb = src["model"], src["sidebar"]
    return ('if p.pane != .replies { repliesOpen = false }' in m and "if pane == .replies { repliesOpen = true }" in m
            and "func toggleReplies() { repliesOpen.toggle() }" in m and "if pane == .replies && model.repliesOpen { repliesChildren }" in sb
            and "WhooshMotion.unfold" in sb)


def replies_open_mutants():
    return [("the-parts-are-always-listed", "sidebar", "if pane == .replies && model.repliesOpen { repliesChildren }", "if pane == .replies { repliesChildren }"),
            ("another-pane-keeps-it-open", "model", "if p.pane != .replies { repliesOpen = false }", "if p.pane != .replies { }"),
            ("the-chevron-does-nothing", "model", "func toggleReplies() { repliesOpen.toggle() }", "func toggleReplies() { }")]


def card_yields(src):
    """Round 5, from the sources: the summary card's fit counts the six parts while Replies is open, and the window passes the open state."""
    return ("repliesOpen ? CGFloat(RepliesPage.panes.count) * childRowHeight : 0" in src["metrics"]
            and "showsSummary(forHeight: geo.size.height, repliesOpen: model.repliesOpen)" in src["view"])


def card_mutants():
    return [("the-card-ignores-the-open-parts", "view", "showsSummary(forHeight: geo.size.height, repliesOpen: model.repliesOpen)", "showsSummary(forHeight: geo.size.height)"),
            ("the-fit-does-not-count-the-parts", "metrics", "(repliesOpen ? CGFloat(RepliesPage.panes.count) * childRowHeight : 0)", "0")]


def toolbar_clears_titlebar(src):
    """Round 5c, from the sources: the window reports its title bar band and the detail column starts below it."""
    return ("SettingsWindowModel.shared.setTitlebar(max(0, w.frame.height - w.contentLayoutRect.height))" in src["view"]
            and ".padding(.top, model.titlebarHeight)" in src["view"] and "func setTitlebar(" in src["model"])


def toolbar_mutants():
    return [("the-detail-column-starts-under-the-band", "view", ".padding(.top, model.titlebarHeight)", ".padding(.top, 0)"),
            ("the-band-height-is-never-read", "view", "SettingsWindowModel.shared.setTitlebar(max(0, w.frame.height - w.contentLayoutRect.height))", "")]


def replies_has_no_digit(src):
    """Replies and Calendars carry no ⌘ digit: the digits of the other ten stay where they were."""
    p = src["pane"]
    return "static let keyless: Set<SettingsPane> = [.agendas, .replies]" in p and "if Self.keyless.contains(self) { return \"\" }" in p


def keyless_mutants():
    return [("replies-takes-a-digit", "pane", "[.agendas, .replies]", "[.agendas]")]


# doors-settings: every control of the three screens, its click door and the control it stands for, both read from the Swift sources.
# (label, door file, door text, control file, control text); the label is the control's name in docs/COVERAGE.md "Settings: General, Accounts, window".
DS = "Sources/Features/"
DOORS = [
    ("window search clear", "SettingsWindow/SettingsWindowRegistry.swift", '"\\(W).search.clear": { _ in guard model.searching', "SettingsWindow/SettingsSidebar.swift", 'whooshAx("mac-settings-window.search.clear")'),
    ("window result row", "SettingsWindow/SettingsWindowRegistry.swift", 'for i in 0..<12 { out["\\(W).result.\\(i)"]', "SettingsWindow/SettingsSidebar.swift", 'whooshAx("mac-settings-window.result.\\(i)")'),
    ("window search Down", "SettingsWindow/SettingsWindowRegistry.swift", '"\\(W).search.down": { _ in guard !model.results.isEmpty', "SettingsWindow/SettingsSidebar.swift", '.onKeyPress(.downArrow) { model.moveSelection(1)'),
    ("window search Up", "SettingsWindow/SettingsWindowRegistry.swift", '"\\(W).search.up": { _ in guard !model.results.isEmpty', "SettingsWindow/SettingsSidebar.swift", '.onKeyPress(.upArrow) { model.moveSelection(-1)'),
    ("window search Return", "SettingsWindow/SettingsWindowRegistry.swift", '"\\(W).search.commit": { _ in guard !model.results.isEmpty', "SettingsWindow/SettingsSidebar.swift", '.onSubmit { model.commitSearch() }'),
    ("import sheet source rows", "SettingsAccountsMac/SettingsAccountsMacFeature.swift", 'out["\\(S).import.source.\\(kind)"] = { _ in', "SettingsAccountsMac/AccountsImportSheet.swift", 'SettingsRow(ax: "\\(P).source.\\(s.kind)"'),
    ("import sheet Choose a file", "SettingsAccountsMac/SettingsAccountsMacFeature.swift", 'out["\\(S).import.\\(name)"] = { _ in', "SettingsAccountsMac/AccountsImportSheet.swift", 'SettingsButton(title: ImportWords.choose, ax: "\\(P).choose")'),
    ("import sheet Open Full Disk Access", "SettingsAccountsMac/SettingsAccountsMacFeature.swift", 'out["\\(S).import.open-settings"] = { _ in', "SettingsAccountsMac/AccountsImportSheet.swift", 'ax: "\\(P).open-settings"'),
    ("import sheet Start", "SettingsAccountsMac/SettingsAccountsMacFeature.swift", '"\\(S).import.start": { _ in guard s.importing', "SettingsAccountsMac/AccountsImportSheet.swift", 'ax: "\\(P).start"'),
    ("import sheet Cancel", "SettingsAccountsMac/SettingsAccountsMacFeature.swift", '"\\(S).import.cancel": { _ in guard s.importing', "SettingsAccountsMac/AccountsImportSheet.swift", 'ax: "\\(P).cancel"'),
    ("import sheet Close", "SettingsAccountsMac/SettingsAccountsMacFeature.swift", '"\\(S).import.close": { _ in guard s.importing', "SettingsAccountsMac/AccountsImportSheet.swift", 'key: "esc", ax: "\\(P).close"'),
    ("import sheet scrim", "SettingsAccountsMac/SettingsAccountsMacFeature.swift", '"\\(S).import.scrim": { _ in guard s.importing', "SettingsAccountsMac/AccountsImportSheet.swift", 'Rectangle().fill(WhooshPalette.windowScrim).onTapGesture { if !model.running { session.closeImport() } }'),
    ("window back", "SettingsWindow/SettingsWindowRegistry.swift", '"\\(W).back": { _ in model.goBack() }', "SettingsWindow/SettingsWindowView.swift", 'navButton("chevron.left", ax: "mac-settings-window.back"'),
    ("window forward", "SettingsWindow/SettingsWindowRegistry.swift", '"\\(W).forward": { _ in model.goForward() }', "SettingsWindow/SettingsWindowView.swift", 'navButton("chevron.right", ax: "mac-settings-window.forward"'),
    ("window summary card", "SettingsWindow/SettingsWindowRegistry.swift", '"\\(W).summary": { _ in model.select(.accounts)', "SettingsWindow/SettingsSidebar.swift", 'whooshAx("mac-settings-window.summary")'),
    ("window replies part row", "SettingsWindow/SettingsWindowRegistry.swift", 'out["\\(W).pane-row.replies.\\(p.rawValue)"] = { _ in _ = model.openReply(p)', "SettingsWindow/SettingsSidebar.swift", 'whooshAx("mac-settings-window.pane-row.replies.\\(p.rawValue)")'),
    ("window pane row", "SettingsWindow/SettingsWindowRegistry.swift", 'out["\\(W).pane-row.\\(p.id)"] = { _ in _ = model.select(p)', "SettingsWindow/SettingsSidebar.swift", 'whooshAx("mac-settings-window.pane-row.\\(pane.id)")'),
    ("general notch Show switch", "SettingsGeneralMac/SettingsGeneralMacFeature.swift", '"\\(P).notch.show": act { s.setUnreadOnly', "SettingsGeneralMac/NotificationsPaneView.swift", 'SettingsRow(ax: "\\(P).notch.show"'),
    ("general notch Focus filter switch", "SettingsGeneralMac/SettingsGeneralMacFeature.swift", '"\\(P).notch.focus-filter": act { s.setFollowFocus', "SettingsGeneralMac/NotificationsPaneView.swift", 'SettingsRow(ax: "\\(P).notch.focus-filter"'),
    ("accounts Import mail button", "SettingsAccountsMac/SettingsAccountsMacFeature.swift", '"\\(S).import": { _ in s.openImport(); return true }', "SettingsAccountsMac/AccountsPaneView.swift", 'ax: "\\(P).import"'),
    ("accounts find field", "SettingsAccountsMac/SettingsAccountsMacFeature.swift", 'out["\\(S).find"] = { _ in guard s.accounts.count > 6', "SettingsAccountsMac/AccountsPaneView.swift", '.whooshAx("\\(P).find")'),
    ("account page name field", "SettingsAccountsMac/SettingsAccountsMacFeature.swift", 'out["\\(S).\\(a.id).\\(field).field"] = { _ in', "SettingsAccountsMac/AccountPageView.swift", 'SettingsField(ax: "\\(P).\\(a.id).name.field"'),
    ("account page name Return", "SettingsAccountsMac/SettingsAccountsMacFeature.swift", 'out["\\(S).\\(a.id).name.field.submit"] = { _ in s.submitName(a.id) }', "SettingsAccountsMac/AccountPageView.swift", '_ = session.submitName(a.id)'),
    ("account page signature field", "SettingsAccountsMac/SettingsAccountsMacFeature.swift", '[("name", AccountsSession.Field.name(a.id)), ("signature", AccountsSession.Field.signature(a.id))]', "SettingsAccountsMac/AccountPageView.swift", 'SettingsField(ax: "\\(P).\\(a.id).signature.field"'),
    ("account page signature Return", "SettingsAccountsMac/SettingsAccountsMacFeature.swift", 'out["\\(S).\\(a.id).signature.field.submit"] = { _ in s.submitSignature(a.id) }', "SettingsAccountsMac/AccountPageView.swift", '_ = session.submitSignature(a.id)'),
    ("account page delivery choices", "SettingsWindow/SettingsWindowRegistry.swift", 'out["mac-settings-accounts.\\(a.id).delivery.\\(key)"] = { _ in s.setDelivery(d, for: a.id) }', "SettingsAccountsMac/AccountPageView.swift", 'SettingsPopup(ax: "\\(P).\\(a.id).delivery.popup"'),
    ("account page Remove account", "SettingsWindow/SettingsWindowRegistry.swift", 'out["mac-settings-accounts.\\(a.id).remove"] = { _ in s.askRemove(a.id) }', "SettingsAccountsMac/AccountPageView.swift", 'ax: "\\(P).\\(a.id).remove"'),
    ("remove review scrim", "SettingsAccountsMac/SettingsAccountsMacFeature.swift", 'out["\\(S).remove.scrim"] = { _ in guard s.removing != nil', "SettingsAccountsMac/AccountPageView.swift", 'Rectangle().fill(WhooshPalette.windowScrim).onTapGesture { session.removing = nil }'),
    ("remove review Cancel", "SettingsWindow/SettingsWindowRegistry.swift", 'out["mac-settings-accounts.remove.cancel"] = { _ in guard s.removing != nil', "SettingsAccountsMac/AccountPageView.swift", 'ax: "\\(P).remove.cancel"'),
    ("remove review Remove", "SettingsWindow/SettingsWindowRegistry.swift", 'out["mac-settings-accounts.remove.confirm"] = { _ in s.confirmRemove() }', "SettingsAccountsMac/AccountPageView.swift", 'ax: "\\(P).remove.confirm"'),
    ("picker scrim", "SettingsAccountsMac/SettingsAccountsMacFeature.swift", 'out["\\(S).picker.scrim"] = { _ in guard s.editing != nil', "SettingsAccountsMac/AccountIconPicker.swift", 'Rectangle().fill(WhooshPalette.windowScrim).ignoresSafeArea().onTapGesture { session.editing = nil }'),
    ("picker Upload button", "SettingsAccountsMac/SettingsAccountsMacFeature.swift", 'out[name == "button" ? "\\(S).upload"', "SettingsAccountsMac/AccountIconPicker.swift", 'id: "mac-settings-accounts.upload"'),
    ("picker Done", "SettingsAccountsMac/SettingsAccountsMacFeature.swift", '"\\(S).picker.done": { _ in s.editing = nil', "SettingsAccountsMac/AccountIconPicker.swift", 'id: "mac-settings-accounts.picker.done"'),
]


def door_missing(files, entry):
    """Which of an entry's two halves is absent from the sources: [] when the door and its control are both there."""
    label, dfile, dtext, cfile, ctext = entry
    return [w for w, f, t in (("door", dfile, dtext), ("control", cfile, ctext)) if t not in files[f]]


def selftest(root=ROOT):
    src = read_sources(root)
    failed = 0
    files = {f: open(os.path.join(root, DS + f), encoding="utf-8").read() for e in DOORS for f in (e[1], e[3])}

    def report(ok, label, detail=""):
        nonlocal failed
        failed += 0 if ok else 1
        print("%s %s %s" % ("ok  " if ok else "FAIL", label, detail))
    clean = served_logos(src)
    # Laya's row draws no mark since the owner's pick W1 (board ai-setup-r1: the name, one grey line, one button): the source says so, the live case stays
    report(clean == dict(LOGOS, laya=None), "clean tree: every provider row draws its own logo asset", str(clean))
    for slug, asset in LOGOS_ALL.items():
        have = [f for f in os.listdir(os.path.join(root, MARKS_DIR)) if f.startswith(asset) and f.endswith(".svg")]
        if have:
            print("ok   asset %s: %s" % (slug, ", ".join(sorted(have))))
        else:
            print("FINDING %s: no asset in the tree; expected %s/%s.svg" % (slug, MARKS_DIR, asset))
    report(set(LOGOS) - {"laya"} <= {s for s, a in clean.items() if a and any(f.startswith(a) for f in os.listdir(os.path.join(root, MARKS_DIR)))},
           "every row's asset has a file in Marks/")
    for slug, key, find, repl in logo_mutants():
        assert find in src[key], (slug, find)
        got = served_logos(dict(src, **{key: src[key].replace(find, repl)}))
        shared = ("openai", "chatgpt")      # one asset-table line serves both rows
        report(got[slug] != LOGOS[slug] and all(got[o] == dict(LOGOS, laya=None)[o] for o in LOGOS if o != slug and not (slug in shared and o in shared)),
               "mutant: the %s row draws the generic mark -> goes red" % slug, "row served %r" % got[slug])
    report(typed_observer_guarded(src), "clean tree: the typed observer is guarded by holdsKeys and holdsKeys tests the settings surface")
    for label, key, find, repl in typed_mutants():
        assert find in src[key], (label, find)
        report(not typed_observer_guarded(dict(src, **{key: src[key].replace(find, repl)})), "mutant: %s -> goes red" % label)
    report(found_has_scroll_target(src), "clean tree: every element the search can ring carries the id the pane's column scrolls to")
    for label, key, find, repl in found_scroll_mutants():
        assert find in src[key], (label, find)
        report(not found_has_scroll_target(dict(src, **{key: src[key].replace(find, repl)})), "mutant: %s -> goes red" % label)
    report(hover_tint_follows_theme(src), "clean tree: a Settings button's hover tint follows the theme")
    for label, key, find, repl in hover_tint_mutants():
        assert find in src[key], (label, find)
        report(not hover_tint_follows_theme(dict(src, **{key: src[key].replace(find, repl)})), "mutant: %s -> goes red" % label)
    report(digit_keys_by_code(src), "clean tree: the panes' digit keys are read by key code in the window's own monitor (A107)")
    for label, key, find, repl in digit_mutants():
        assert find in src[key], (label, find)
        report(not digit_keys_by_code(dict(src, **{key: src[key].replace(find, repl)})), "mutant: %s -> goes red" % label)
    report(escape_yields(src), "clean tree: the window's Escape asks the open pane first, then clears its own search (round 3)")
    for label, key, find, repl in escape_mutants():
        assert find in src[key], (label, find)
        report(not escape_yields(dict(src, **{key: src[key].replace(find, repl)})), "mutant: %s -> goes red" % label)
    report(replies_open_rule(src), "clean tree: the parts of Replies are listed only while it is open; a move to another pane closes it (round 5)")
    for label, key, find, repl in replies_open_mutants():
        assert find in src[key], (label, find)
        report(not replies_open_rule(dict(src, **{key: src[key].replace(find, repl)})), "mutant: %s -> goes red" % label)
    report(card_yields(src), "clean tree: the accounts card yields its height to the open Replies parts at the default window (round 5)")
    for label, key, find, repl in card_mutants():
        assert find in src[key], (label, find)
        report(not card_yields(dict(src, **{key: src[key].replace(find, repl)})), "mutant: %s -> goes red" % label)
    report(toolbar_clears_titlebar(src), "clean tree: the detail column starts below the title bar band (round 5c)")
    for label, key, find, repl in toolbar_mutants():
        assert find in src[key], (label, find)
        report(not toolbar_clears_titlebar(dict(src, **{key: src[key].replace(find, repl)})), "mutant: %s -> goes red" % label)
    report(replies_has_no_digit(src), "clean tree: Replies and Calendars take no digit")
    for label, key, find, repl in keyless_mutants():
        assert find in src[key], (label, find)
        report(not replies_has_no_digit(dict(src, **{key: src[key].replace(find, repl)})), "mutant: %s -> goes red" % label)
    # doors-settings: each control has its click door and its drawn control in the sources; a mutant deletes the door (or the control) and goes red
    report(all(not door_missing(files, e) for e in DOORS), "clean tree: %d controls each have a click door and a drawn control" % len(DOORS),
           str([(e[0], door_missing(files, e)) for e in DOORS if door_missing(files, e)]))
    for e in DOORS:
        label, dfile, dtext, cfile, ctext = e
        for half, f, t in (("door", dfile, dtext), ("control", cfile, ctext)):
            mutated = dict(files, **{f: files[f].replace(t, "/* removed */")})
            report(half in door_missing(mutated, e), "mutant: the %s of '%s' is removed -> goes red" % (half, label))
    evidence_selftest(report)
    print("%d failed" % failed)
    return 1 if failed else 0


# ---- --selftest, the evidence cases: a stand-in for the socket's reads (the REAL case functions above run against it) --------------------------
class _FakeCtl:
    """Serves just the rows, measures and verbs the evidence cases read, with one switch per mutant. Used ONLY by --selftest."""
    def __init__(self, mut=()):
        self.m, self.pane, self.reply, self.open, self.query, self.typing = set(mut), "general", "auto", False, "", False
        self.engine, self.refusal, self.editing = "local", "", None
        if "stale-query-sticks" in self.m:      # an earlier step left a query and the window's Escape does not clear it
            self.query = "show"

    def hits(self):
        return [{"title": "Show account emblems", "row": G + ".appearance.emblems", "path": "General › Message list", "selected": True}] if self.query.lower() == "emblem" and "search-empty" not in self.m else []

    def click(self, ax):
        pre = W + ".pane-row."
        if ax.startswith(pre):
            rest = ax[len(pre):]
            if rest in PANES:
                keep = "replies-stays-open" in self.m and self.open and rest != "replies"
                self.pane, self.open = rest, rest == "replies" or keep
                self.reply = "auto" if rest == "replies" else self.reply
                return {}
            if rest.startswith("replies.") and rest.split(".")[1] in REPLY_PARTS:
                self.pane, self.reply, self.open = "replies", rest.split(".")[1], True
                return {}
        if ax == W + ".fixture.query.show":
            self.query = "show"
            return {}
        if ax == R + ".editor.voiceText" and "doc-door-unserved" not in self.m:      # the app's own editor door (RepliesEditorDoors.swift:19)
            self.editing = "voiceText"
            return {}
        if ax.startswith(D + ".decisions.engine."):
            e = ax.rsplit(".", 1)[1]
            if e == "jev" and "jev-chosen-without-key" not in self.m:      # the app refuses a hosted model with no key and no consent, in the core's words
                self.refusal = "Allow the text of the mail to go to Jev first, or choose the model on this Mac."
            else:
                self.engine, self.refusal = e, ""
            return {}
        raise Unreadable("click ('%s',): nothing to click at %s" % (ax, ax))

    def key(self, chord):
        if chord == "\u2318F":
            self.typing = "cmdf-dead" not in self.m
        elif chord == "Escape":
            if self.editing:
                self.editing = None
            elif "stale-query-sticks" not in self.m:
                self.query, self.typing = "", False

    def type(self, text):
        if self.typing:
            self.query += text

    def title(self):
        return REPLY_TITLES[self.reply] if self.pane == "replies" else TITLES[self.pane]

    def rows(self, ax):
        if ax == W + ".panes":
            return [{"id": p, "open": self.open if p == "replies" else None, "current": p == self.pane, "title": TITLES[p]} for p in PANES]
        if ax == W + ".place":
            hit = self.hits()
            return [{"pane": "general" if hit else self.pane, "reply": self.reply, "title": self.title(), "query": self.query, "ring": G + ".appearance.emblems" if hit else None}]
        if ax == W + ".search":
            return self.hits()
        if ax == W + ".search.index":
            return [{"title": "Show account emblems", "row": G + ".appearance.emblems", "pane": "general"}]
        if ax == R + ".controls":
            return [{"ax": R + ".header.auto", "kind": "header"}, {"ax": R + ".editor.voiceText", "kind": "text"}, {"ax": R + ".editor.voiceText.save", "kind": "button"}]
        if ax == R + ".editors":
            return [{"key": "voiceText", "focused": self.editing == "voiceText"}]
        if ax == R + ".state":
            return [{"ui": {"editing": self.editing, "page": self.reply}}]
        if ax == D + ".decisions.engine":
            return [{"id": e, "chosen": self.engine == e, "refusal": self.refusal if e == "jev" else ""} for e in ("jev", "off", "local")]
        if ax in (D + ".decisions.jev.key", D + ".decisions.jev.consent"):
            return [{"given": False, "state": "not connected"}]
        if ax == D + ".decisions.local":
            return [{"state": "absent"}]
        if ax == D + ".logos":
            return [{"slug": "laya", "logo": "nexus-mark"}]
        if ax == "mac-keyevent.monitor":
            return [{"seen": 2, "sent": 2, "last": {"route": "window", "handled": True, "key_before": True, "key_after": True,
                                                    "windows": [{"id": "mac-settings-window", "key": True, "focus": "NSTextView" if self.typing else "NSView"}]}}]
        raise Unreadable("no rows at %s" % ax)

    def measure(self, ax):
        if ax == R + ".header.auto":      # the app never marks this id (a form-fit probe only): no measure reads it, whatever a mutant does
            raise Unreadable("no measurement at %s: nothing on screen carries that id (15 accessibility elements walked)" % ax)
        if ax == R + ".title.auto" and "replies-title-unserved" in self.m:
            raise Unreadable("no measurement at %s: nothing on screen carries that id (15 accessibility elements walked)" % ax)
        if ax == W + ".pane-row.replies.auto" and not self.open:
            raise Unreadable("no measurement at %s" % ax)
        if ax == D + ".decisions.local":      # the row's container is a plain accessibility id: not measurable on the real app either (main81)
            raise Unreadable("no measurement at %s: nothing on screen carries that id (15 accessibility elements walked)" % ax)
        if ax == D + ".decisions.local.download" and "laya-row-undrawn" in self.m:
            raise Unreadable("no measurement at %s: nothing on screen carries that id (15 accessibility elements walked)" % ax)
        if ax == D + ".provider.laya.mark" and "laya-draws-a-mark" not in self.m:      # the Model row draws no vendor mark (W1)
            raise Unreadable("no measurement at %s: nothing on screen carries that id (15 accessibility elements walked)" % ax)
        return {"ok": True, "frame": {"x": 10, "y": 10, "w": 100, "h": 28}, "window": "mac-settings-window"}

    def winframes(self):
        return [{"title": "Settings", "x": 100, "y": 80, "w": 940, "h": 680, "kind": "window", "id": "mac-settings-window"}]

    def uistate(self):
        return {"surface": "mac-settings-" + self.pane, "first_responder": "search-field" if self.typing else "list", "tab": "primary"}

    def core(self, path):
        return "rules" if self.engine == "off" else self.engine

    def send(self, verb, *a):
        return getattr(self, verb)(*a)


def _flow(name, s):
    """The case `name` as drive() reaches it: the same actions, then the same real thunk and reads."""
    ctl = s.j.ctl
    if name in ("replies-is-closed-while-another-pane-is-selected", "selecting-another-pane-closes-replies-and-it-is-not-remembered"):
        click(s, W + ".pane-row.replies.voice")
        thunk = (lambda: replies_closed_by_default(s)) if name.startswith("replies-is-closed") else (lambda: replies_closed_by_another_pane(s))
        expect_ev(s, name, thunk, EXPECT[name], reads_replies_open(s))
    elif name in ("search-finds-the-emblems-switch-first", "first-hit-opens-its-pane-and-rings-its-row", "results-show-their-path"):
        s.click(W + ".pane-row.accounts")
        for _ in range(3):      # the same hygiene as drive(): a query an earlier step left is cleared by the window's own Escape
            if place(s)["query"] == "":
                break
            s.ctl.key("Escape")
        s.key("\u2318F")
        s.type("emblem")
        thunk = {"search-finds-the-emblems-switch-first": lambda: search_titles(s), "first-hit-opens-its-pane-and-rings-its-row": lambda: search_first_hit(s),
                 "results-show-their-path": lambda: search_first_path(s)}[name]
        expect_ev(s, name, thunk, EXPECT[name], reads_search(s))
    elif name.startswith("engine-"):
        e = name[len("engine-"):]
        before = engine_chosen(s, e) if e == "jev" else None
        s.click("%s.decisions.engine.%s" % (D, e))
        if e == "jev":
            expect_ev(s, name, lambda: jev_refused_without_key_and_consent(s, before), EXPECT[name], reads_engine(s))
        else:
            expect_ev(s, name, lambda: engine_chosen(s, e), [e], reads_engine(s))
    elif name == "provider-mark-laya":
        s.click(D + ".decisions.engine.local")
        expect_ev(s, name, lambda: (model_row_drawn(s), drawn(s, D + ".provider.laya.mark")), EXPECT[name], reads_laya_mark(s))
    elif name == "click-every-pane-row-shows-its-pane":
        cur = {}
        expect_ev(s, name, lambda: click_each_pane(s, cur), EXPECT[name], reads_click_each(s, cur))
    elif name == "escape-goes-to-the-open-pane-first-then-the-window":
        expect_ev(s, name, lambda: escape_yields_pane(s), EXPECT[name], reads_escape(s))
    else:
        raise KeyError(name)
    return ctl


# (case, mutant, state on the mutant, labels the line must carry, words the line must carry)
EVIDENCE_MUTANTS = [
    ("replies-is-closed-while-another-pane-is-selected", "replies-stays-open", "fail", ["replies_pane_row", "parts_row_auto", "place"], '"open": true'),
    ("selecting-another-pane-closes-replies-and-it-is-not-remembered", "replies-stays-open", "fail", ["replies_pane_row", "parts_row_auto", "windows"], '"window": "mac-settings-window"'),
    ("search-finds-the-emblems-switch-first", "search-empty", "fail", ["search_text", "result_rows", "keyevent"], 'search_text="emblem"'),
    ("first-hit-opens-its-pane-and-rings-its-row", "search-empty", "fail", ["search_text", "place", "search_field_frame"], '"pane": "accounts"'),
    ("search-finds-the-emblems-switch-first", "stale-query-sticks", "fail", ["search_text", "result_rows", "place"], 'search_text="showemblem"'),
    ("results-show-their-path", "search-empty", "fail", ["search_text", "result_rows", "index_rows"], "reply shape: IndexError"),
    ("engine-jev", "jev-chosen-without-key", "fail", ["engine_rows", "core_provider", "jev_key_row"], '"chosen": true'),
    ("provider-mark-laya", "laya-draws-a-mark", "fail", ["laya_row_frame", "laya_mark_frame", "laya_row", "logos"], '"frame"'),
    ("provider-mark-laya", "laya-row-undrawn", "fail", ["laya_row_frame", "laya_button_frame", "laya_row", "logos"], '"state": "absent"'),
    ("click-every-pane-row-shows-its-pane", "replies-title-unserved", "unjudged", ["pane_being_read", "header_frame", "replies_ids_served"], 'pane_being_read="replies"'),
    ("escape-goes-to-the-open-pane-first-then-the-window", "doc-door-unserved", "unjudged", ["editor_ids_served", "editors_rows", "keyevent"], "mac-settings-replies.editor.voiceText"),
]


def evidence_selftest(report):
    from lib.harness import Journey
    clean_names = [c for c in EVIDENCE_CASES if c not in ("engine-off", "engine-local")] + ["engine-off", "engine-local"]
    for name in clean_names:
        j = Journey(ID, _FakeCtl())
        _flow(name, j.step(ID, "stand-in"))
        c = [x for x in j.checks() if x.label.endswith("/" + name)][0]
        report(c.state == "pass", "stand-in, clean: %s passes" % name, "%s %s" % (c.state, c.detail[:200]))
    for name, mut, state, labels, words in EVIDENCE_MUTANTS:
        j = Journey(ID, _FakeCtl((mut,)))
        _flow(name, j.step(ID, "stand-in"))
        c = [x for x in j.checks() if x.label.endswith("/" + name)][0]
        has = c.state == state and "| evidence: " in c.detail and all(l + "=" in c.detail for l in labels) and words in c.detail
        report(has, "mutant %s: %s goes %s and its line carries %s" % (mut, name, state, ", ".join(labels)))
        print("     FAIL TEXT: %s" % c.detail[:1500])


def main():
    if "--selftest" in sys.argv:
        return selftest()
    path = os.path.abspath(__file__)
    import lib.harness as H                       # keep the journeys, to say why a check was not judged
    made, orig = [], H.Journey.__init__
    def init(self, *a, **k):
        orig(self, *a, **k)
        made.append(self)
    H.Journey.__init__ = init
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    rows_ = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# par-") and " | " in l]
    print("MUTANTS (a check is green only after it has been seen red on its mutant)")
    print("| check | mutant |")
    print("|---|---|")
    for k, v in rows_:
        print("| %s | %s |" % (k, v))
    for line in r["fail_list"]:
        sys.stderr.write("FAIL %s\n" % line)
    for j in made:
        for c in j.checks():
            if c.state == "unjudged":
                sys.stderr.write("NOT JUDGED %s: %s\n" % (c.label, c.detail))
    for line in r["no_mutant"]:
        sys.stderr.write("NO MUTANT %s\n" % line)
    for line in r["stray_mutant"]:
        sys.stderr.write("STRAY MUTANT %s\n" % line)
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print_cases(r)                                # ONE line per case: "<case> pass|fail|unjudged <detail>", a pass is never read from silence
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    ok = r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"]
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
