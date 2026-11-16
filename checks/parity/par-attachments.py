#!/usr/bin/env python3
"""par-attachments: an attachment chip in the reader and a pending file in the composer behave as a Finder file.

Space opens the system Quick Look panel (zooming from the chip; arrows step; Space or Escape closes; the panel follows
the selection); double-click, Return and ⌘O open the file in its default app; the chip drags out as a file (or a file
promise before the download); the right-click menu carries Open · Open With · Quick Look · Save to Downloads ·
Save As… · Copy · Share · Show in Finder, each with its key where one exists; images, PDFs and videos show a real
thumbnail; a file still downloading shows its progress and Quick Look and Open wait for it; the composer takes dropped
files and pasted images; the chip shows its keys. Every control on the chip and in the menu is clicked once.

Drives the RUNNING rig copy over the control socket (journeys/lib). Expected names come from
docs/design/fixtures/mac-main-sample.json; the held downloads are the fixture source's (IMG_2051.jpg, the term sheet).
Under the rig, Open records the app it would launch (no app opens on the owner's screen) and saves land in the run dir.

Run:     sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/par-attachments.py'
Mutants: python3 checks/parity/par-attachments.py --mutants <bundle>   (one launch per mutant, WHOOSH_ATTACH_MUTANT=<name>)
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0.
"""
import json
import os
import re
import shutil
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, Journey, declared_mutants, print_cases      # noqa: E402
from lib.fixture import ROWS, const                                  # noqa: E402
from lib.ctl import TIMEOUT_S, Unreadable                            # noqa: E402
from itertools import zip_longest                                    # noqa: E402

ID = "par-attachments"
TITLE = "Attachments open the Mac way: Quick Look, open, drag, menu, thumbnails, download, composer intake, keys"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "Space shows the file in Quick Look, double-click/Return/⌘O open it, the menu's every item acts, thumbnails are real"
R = "mac-main.attachments"
C = "mac-composer.attachment-files"
CAP = os.path.join(ROOT, "_captures", "attachments-mac")
# The poll budget for async lands: the socket's own wait for a frame, stretched by the machine's load over the ledger's
# PERF_LOAD_MAX (a busy Mac lands the same state later; this check judges the state, not its speed).
WAIT_S = const("INTERACTION_MEASURE_WAIT_S") * max(1.0, os.getloadavg()[0] / const("PERF_LOAD_MAX"))
MENU = ["open", "open-with", "quick-look", "save-to-downloads", "save-as", "copy", "share", "show-in-finder"]
READER_ITEMS = ["open", "open-with", "quick-look", "save-to-downloads", "show-in-finder", "copy", "share"]   # the model's menu doors besides Save As and the apps
# The three rows by their id, never by a fixed index: the list serves the flow's rejected row third since delta 17, so msg-08 stands
# at 8 and msg-12 at 12 (main52: PHOTO_ROW 7 read msg-07, no photos, IndexError).
_IDX = {r["id"]: i for i, r in enumerate(ROWS)}
PDF_ROW, PHOTO_ROW, SHEET_ROW = _IDX["msg-01"], _IDX["msg-08"], _IDX["msg-12"]
OPEN_WITH_DOORS = 8                                   # AttachmentSocketPanels.openWithDoors: the app doors the app registers per chip
SHEET_SETTLE_S = 2 * TIMEOUT_S                        # no request is sent while the save sheet connects (see menu_round)              # msg-01 (one PDF), msg-08 (three photos), msg-12 (the term sheet)


FAST = "--selftest" in sys.argv                                      # the selftest's model answers at once: no clock to wait for


def poll(thunk, want, wait=WAIT_S):
    """Reads until the value is `want` or the budget ends; returns the last value read."""
    end = time.time() + wait
    got = thunk()
    while got != want and time.time() < end:
        time.sleep(0.05)
        got = thunk()
    return got


def settle(s, thunk, want):
    """An action's wait outside any check: a read that fails here is noted, and the next check judges the state."""
    if not s.j.ctl:
        return
    try:
        poll(thunk, want)
    except Exception as e:                                           # noqa: BLE001
        s.j.notes.append("%s wait: %s" % (s.id, e))


def chip(s, i, region=R):
    rows = s.ctl.rows(region)
    if i >= len(rows):
        u = s.ctl.uistate()
        raise KeyError("%s has %d chips (surface %s, focus %s, overlay %s)" % (region, len(rows), u.get("surface"), u.get("focus_message"), u.get("overlay")))
    return rows[i]


def panel(s):
    return s.ctl.rows(R + ".panel")[0]


def state(s):
    return s.ctl.rows(R + ".state")[0]


def menu(s):
    return {r["id"]: r for r in s.ctl.rows(R + ".menu")}


def to_row(s, n):
    """Escape out of Quick Look, the selection and the reader, focus the fixture's message n by its id, open its thread."""
    for _ in range(3):
        s.key("Escape")
    s.click("%s.dev.focus.%s" % (R, ROWS[n]["id"]))
    s.click("mac-main.reader.open")


BLANK = []                                           # shots whose pixels are all one colour (or that were never taken)


def keep_shot(s, name, theme, take=True):
    if s.j.ctl and hidden(s):
        # Hidden rig: the app's own shot renders the window's view tree; Quick Look and a menu are drawn by the system
        # outside that tree, so their pictures cannot exist here (CAPTURES.md says so); the chip is rendered.
        if name == "chip":
            gallery_shot(s, "chip", theme)
        return
    if take:
        s.click("%s.dev.shot.%s" % (R, name))
    try:
        shots = state(s)["shots"]
        if int(shots.get(name + ".colours", "0")) <= 1:
            BLANK.append("%s-%s" % (name, theme))
        # Quick Look is in the picture; the menu is not checked here: the system draws it in a window this process does
        # not own, which a rig copy without Screen Recording cannot capture (measured on am5: only the app's own windows)
        if name == "quicklook" and int(shots.get(name + ".windows", "0")) < 2:
            BLANK.append("%s-%s without its panel" % (name, theme))
        src = shots.get(name)
        if src and os.path.exists(src):
            os.makedirs(CAP, exist_ok=True)
            shutil.copy(src, os.path.join(CAP, "%s-%s-1280x820.png" % (name, theme)))
    except Exception as e:                                           # a missing shot is a MISSING row, not a crash
        s.j.notes.append("shot %s/%s: %s" % (name, theme, e))


def hidden(s):
    try:
        return bool(state(s).get("hidden"))
    except Exception:                                                # noqa: BLE001
        return False


def drive(j):
    os.makedirs(CAP, exist_ok=True)
    s = j.step("par-attachments-chip", "The reader's chip: real thumbnail, kind, keys in its tooltip, selection")
    s.raw("resize", "1280", "820")
    to_row(s, PDF_ROW)
    pdf = ROWS[PDF_ROW]["attachments"][0]
    s.expect("chip-names-the-file", lambda: [r["name"] for r in s.ctl.rows(R)], [pdf])
    s.expect("pdf-bytes-land", lambda: poll(lambda: chip(s, 0)["download"], "ready"), "ready")
    s.expect("pdf-has-real-thumbnail", lambda: poll(lambda: chip(s, 0)["thumbnail"], "thumbnail"), "thumbnail")
    s.expect("tooltip-shows-keys", lambda: "⌘O" in chip(s, 0)["tooltip"] and chip(s, 0)["tooltip"].count("·") >= 2, True)   # the key glyph; words are the catalogue's
    s.click(R + ".0")
    s.expect("click-selects-the-chip", lambda: (chip(s, 0)["selected"], state(s)["holds_keys"]), (True, True))
    s.expect("selected-chip-shows-its-keys", lambda: chip(s, 0)["keys_shown"], True)
    s.expect("keys-reveal-within-hover-reveal", lambda: chip(s, 0)["reveal_ms"], const("HOVER_REVEAL_MS"))

    s = j.step("par-attachments-quicklook", "Space opens Quick Look from the chip; Space and Escape close it")
    s.key("Space")
    s.expect("space-opens-the-panel", lambda: poll(lambda: panel(s)["open"], True), True)
    s.expect("panel-shows-the-chip-file", lambda: (panel(s)["item"], chip(s, 0)["preview_open"]), (pdf, True))
    s.expect("panel-is-wired-to-the-chip", lambda: (panel(s)["has_data_source"], panel(s)["has_delegate"]), (True, True))
    s.expect("panel-zooms-from-the-chip", lambda: poll(lambda: "zoom_from" in panel(s) and panel(s).get("zoom_from") == chip(s, 0).get("screen_frame"), True), True)
    s.key("Space")
    s.expect("space-closes-the-panel", lambda: poll(lambda: panel(s)["open"], False), False)
    s.key("Space")
    s.key("Escape")
    s.expect("escape-closes-the-panel", lambda: poll(lambda: panel(s)["open"], False), False)

    s = j.step("par-attachments-open", "Double-click, Return and ⌘O open the file in its default app")
    to_row(s, PDF_ROW)
    s.click(R + ".0.double")
    s.expect("double-click-opens", lambda: (chip(s, 0)["open_count"], chip(s, 0)["opened"].endswith(pdf), chip(s, 0)["opened_with"] != ""), (1, True, True))
    s.key("Enter")
    s.expect("return-opens", lambda: chip(s, 0)["open_count"], 2)
    s.key("⌘O")
    s.expect("command-o-opens", lambda: chip(s, 0)["open_count"], 3)
    s.key("Escape")
    s.expect("escape-lets-go-of-the-chip", lambda: state(s)["selected"], "")
    s.key("⌘O")
    s.expect("command-o-opens-the-first-attachment-unselected", lambda: chip(s, 0)["open_count"], 4)

    menu_round(j, pdf)
    photo_round(j)
    sheet_round(j)
    composer_round(j)
    doors_round(j)
    absent_round(j)
    absent_chip_round(j)
    sweep_round(j)
    shots_round(j)
    gallery_round(j)
    header_round(j)


def pw(thunk, want, wait=None):
    """poll() with the selftest's budget: the model answers at once, so a wrong value is read once, not waited for."""
    return poll(thunk, want, 0 if FAST else (WAIT_S if wait is None else wait))


def app_name(title):
    """A menu title without the app's '(default)' word, which is in the app's language."""
    return re.sub(r" \([^)]*\)$", "", title)


def doors_round(j):
    """doors-read: the controls of the chip, the tile and the menu that no round above clicked, each once with a visible outcome:
    the pending file's remove button, ⌘C and the arrows on a selected chip, the hover, a tile's double-click, menu and drag, each app
    of Open With, and the Save As sheet's Cancel. (The menu's own items, Space, Return, Escape, ⌘O, ⌘V, the gallery's More,
    Quick Look and Save all are clicked by the rounds above.)"""
    photos = ROWS[PHOTO_ROW]["attachments"]
    pdf = ROWS[PDF_ROW]["attachments"][0]
    seen = {}

    s = j.step("par-attachments-doors-remove", "The pending file's own remove button")
    s.click(R + ".dev.stage-file")                                   # a copied FILE makes the pending tile (picture data goes into the text)
    s.key("\u2318V")
    s._act(lambda: seen.update(pending=pw(lambda: len(s.ctl.rows(C)), 2)))
    s.click(C + ".1.remove")
    s.expect("remove-button-removes-the-pending-file", lambda: ([r["name"] for r in s.ctl.rows(C)], seen["pending"]), (["sample-3-seconds.mp4"], 2))

    s = j.step("par-attachments-doors-keys", "A selected chip: ⌘C copies the file, the arrows move the selection")
    to_row(s, PHOTO_ROW)
    s.click(R + ".0")
    s.key("\u2318C")
    s.expect("command-c-copies-the-selected-file", lambda: (state(s)["pasteboard_file"], chip(s, 0)["notice"] != ""), (photos[0], True))
    s.key("Right")
    s.expect("right-moves-the-selection-without-quick-look", lambda: (chip(s, 1)["selected"], chip(s, 0)["selected"]), (True, False))
    s.click(R + ".1")                                                # the Left case starts from a selection of its own, not from where Right left it
    s.key("Left")
    s.expect("left-moves-it-back", lambda: (chip(s, 0)["selected"], chip(s, 1)["selected"]), (True, False))
    s.key("Escape")

    s = j.step("par-attachments-doors-hover", "The pointer on a chip shows its keys; off it, they hide")
    s.click(R + ".hover.none")
    s.click(R + ".1.hover")
    s.expect("hover-shows-the-chip-keys", lambda: chip(s, 1)["keys_shown"], True)
    s.click(R + ".hover.none")
    s.expect("hover-away-hides-them", lambda: chip(s, 1)["keys_shown"], False)

    s = j.step("par-attachments-doors-tile", "A gallery tile is a chip to the keys and the menu: double-click, menu, drag")
    s.expect("the-photos-are-tiles-of-the-gallery", lambda: gallery(s)["media"], photos)
    s._act(lambda: seen.update(opens=chip(s, 1)["open_count"]))
    s.click(R + ".1.double")
    s.expect("tile-double-click-opens-the-file", lambda: chip(s, 1)["open_count"] == seen["opens"] + 1, True)
    s.click(R + ".2.menu")
    s.expect("tile-menu-lists-the-items-in-finder-order", lambda: pw(lambda: list(menu(s).keys()), MENU), MENU)
    s.click(R + ".2.drag")
    s.expect("tile-drag-carries-the-file", lambda: (chip(s, 2)["drag_kind"], chip(s, 2)["drag_file_exists"]), ("file", True))
    s.key("Escape")

    s = j.step("par-attachments-doors-openwith", "Open With: each app of the submenu has its own click")
    to_row(s, PDF_ROW)
    s.click(R + ".0")

    def apps():
        titles = menu(s)["open-with"]["submenu"]
        if len(titles) < 2:
            raise Unreadable("this Mac lists %d app for a PDF in Open With: the second app's door has nothing to open" % len(titles))
        return [app_name(t) for t in titles]
    s.click(R + ".0.menu.open-with.0")
    s.expect("open-with-the-default-app-door", lambda: chip(s, 0)["opened_with"] == apps()[0], True)
    s.click(R + ".0.menu.open-with.1")
    s.expect("open-with-the-second-app-opens-with-it", lambda: chip(s, 0)["opened_with"] == apps()[1], True)

    s = j.step("par-attachments-doors-saveas", "The Save As sheet: Cancel closes it and saves nothing")
    s._act(lambda: seen.update(saved=chip(s, 0)["saved_path"]))
    s.click(R + ".0.menu.save-as")
    if not FAST:
        time.sleep(SHEET_SETTLE_S if s.j.ctl else 0)                 # the sheet connects slowly (see menu_round)
    s.expect("save-as-opens-the-panel-for-the-cancel", lambda: pw(lambda: state(s)["save_panel_open"], True, WAIT_S * 4), True)
    s.click(R + ".save-as.cancel")
    s.expect("save-as-cancel-closes-the-panel-and-saves-nothing", lambda: (pw(lambda: state(s)["save_panel_open"], False), chip(s, 0)["saved_path"] == seen["saved"]), (False, True))
    s.key("Escape")


def fmt(key, *vals):
    """A catalogue sentence in English with its %lld / %@ holes filled, in order (the source of the expected words, never a built screen)."""
    out = catalogue(key, "en")
    for v in vals:
        out = out.replace("%lld", str(v), 1) if "%lld" in out else out.replace("%@", str(v), 1)
    return out


def catalogue(key, lang):
    with open(os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8") as f:
        return json.load(f)["strings"][key]["localizations"][lang]["stringUnit"]["value"]


SIZE_EN = re.compile(r"^1 file · \d[\d.,]*\s(?:bytes?|kB|MB|GB)$")         # \s: the system's formatter puts a no-break space before the unit, and writes kB (SI) with a small k: measured on this Mac, not assumed
SIZE_FR = re.compile(r"^1 fichier · \d[\d.,\s]*\s(?:octets?|ko|Mo)$")
WHY = "the server refused the download"


def size_read(header, rx):
    """True when the strip's header matches the size pattern; else the sentence that decides app against check: the header string the app served
    (repr, so a no-break space or a capital K shows) with its code points, and the pattern it was matched against. The app's formatter is
    `.byteCount(style: .file)` (AttachmentSize.swift:9-10); measured by lane doors-read (docs/lanes/doors-read.md:128): `12 kB` in English, `12\u202fko` in French."""
    if rx.match(header):
        return True
    return "header %r (code points of its last 6 chars: %s) did not match %r" % (header, " ".join("U+%04X" % ord(c) for c in header[-6:]), rx.pattern)


def saved_count(s, stem):
    """How many files of this stem the rig's Downloads folder holds (the model keeps its own list)."""
    if hasattr(s.ctl, "downloads"):
        return len([f for f in s.ctl.downloads() if f.startswith(stem)])
    down = os.path.join(os.environ.get("WHOOSH_SOCKET_DIR", ""), "downloads")
    return len([f for f in os.listdir(down) if f.startswith(stem)]) if os.path.isdir(down) else 0


def absent_shot(s, name):
    """A picture of the main window in the absent-space round, in light AND dark, named for the theme that is drawn:
    _captures/attachments-mac/absent/<name>-<theme>-1280x820.png (a4a612c7 proof 2; main105: the three files named light were drawn dark, the theme was never set).
    The theme is set before each picture and put back to light after."""
    if s.j.ctl:
        os.makedirs(os.path.join(CAP, "absent"), exist_ok=True)
    for theme in ("light", "dark"):
        s.raw("theme", theme)
        gallery_shot(s, name, theme, sub="absent")
    s.raw("theme", "light")


BLOCK_WIDTH = 500.0      # WhooshMetrics.blockWidth (Sources/Design/WhooshSpacing.swift:49): the widest a lone chip may grow


def frames_apart(a, b):
    return a["x"] + a["w"] <= b["x"] or b["x"] + b["w"] <= a["x"] or a["y"] + a["h"] <= b["y"] or b["y"] + b["h"] <= a["y"]


def ring_and_caps_apart(s):
    """True when the fetch ring and the key caps do not share space. While a file is fetched the ring stands alone: the key caps are not drawn (no measurement of their id), or
    their frame is clear of the ring's. A caps frame that meets the ring's is the defect of main105 (two controls at the chip's right end)."""
    ring = s.ctl.measure(R + ".fetch.ring")["frame"]
    try:
        caps = s.ctl.measure(R + ".0.keys")["frame"]
    except Unreadable as e:
        if "no measurement" in str(e):
            return True
        raise
    return frames_apart(ring, caps)


def name_cut_only_when_full(s):
    """(cut, chip width, limit) read to a verdict: the name is cut (its box narrower than the width it needs in one line) only when the chip is as wide as the limit."""
    need = chip(s, 0)["name_need"]
    name = s.ctl.measure(R + ".0.name")["frame"]
    box = s.ctl.measure(R + ".0")["frame"]
    cut = name["w"] + 0.5 < need
    return (not cut) or box["w"] >= BLOCK_WIDTH - 1


def absent_round(j):
    """doors-read A123 (Space on a file that is not on this Mac does nothing): on the real core a file's bytes are usually absent when the
    mail opens. Space and the Quick Look button must fetch them first (the tile shows the progress), then open the preview; Save all fetches
    then saves; a failed fetch says why and the next Space asks again; the strip says "not on this Mac yet" and prints no size for absent
    files, and sizes are in the app's language. The fixture holds a file absent through the rig door `dev.absent` (no fixture verb does yet)."""
    pdf = ROWS[PDF_ROW]["attachments"][0]
    head = lambda s: gallery(s)["header"]
    seen = {}

    s = j.step("par-attachments-absent-space", "Space on a file that is not on this Mac: fetch, progress, then the preview")
    to_row(s, PDF_ROW)
    s.click(R + ".dev.absent")
    s.click(R + ".0")
    s.expect("absent-file-is-named-absent-and-the-strip-prints-no-size", lambda: (chip(s, 0)["state"], head(s)), ("absent", fmt("1 file · not on this Mac yet")))
    widths = {}
    s._act(lambda: widths.update(absent=s.ctl.measure(R + ".0")["frame"]["w"]))
    absent_shot(s, "1-absent")      # a4a612c7 proof 2: the tile "not on this Mac yet" with its cloud, and the strip
    s.key("Space")
    s.expect("space-on-an-absent-file-starts-the-fetch-and-waits", lambda: pw(lambda: (chip(s, 0)["state"], chip(s, 0)["waiting"], panel(s)["open"]), ("fetching", "quick-look", False)), ("fetching", "quick-look", False))
    s.click(R + ".download.half")
    s.expect("the-tile-shows-the-progress", lambda: (chip(s, 0)["state"], chip(s, 0)["percent"], panel(s)["open"]), ("fetching", 50, False))
    s._act(lambda: widths.update(fetching=s.ctl.measure(R + ".0")["frame"]["w"]))
    s.expect("the-ring-and-the-key-caps-do-not-share-the-chips-end", lambda: ring_and_caps_apart(s), True)
    s.expect("the-name-is-cut-only-when-the-chip-is-full", lambda: name_cut_only_when_full(s), True)
    absent_shot(s, "2-fetching-50")      # the ring at 50 percent
    s.click(R + ".download.finish")
    s.expect("the-preview-opens-when-the-bytes-land", lambda: pw(lambda: (panel(s)["open"], panel(s)["item"], chip(s, 0)["state"]), (True, pdf, "present")), (True, pdf, "present"))
    s._act(lambda: widths.update(present=pw(lambda: s.ctl.measure(R + ".0")["frame"]["w"], widths.get("absent", 0), 0 if FAST else 2.0)))
    def one_width():
        w = dict(widths)
        if len(w) == 3 and max(w.values()) - min(w.values()) <= 0.5:
            return True
        raise IndexError("the chip's width differs between the states: %r" % w)     # the three widths land in the log (main111: expected True, got False said nothing)
    s.expect("the-chip-keeps-one-width-from-absent-to-present", one_width, True)
    s.expect("the-strip-names-no-absent-file-once-it-is-here", lambda: "not on this Mac" not in head(s), True)
    s.key("Space")
    s.expect("space-closes-the-preview", lambda: pw(lambda: panel(s)["open"], False), False)
    absent_shot(s, "3-present")      # the file is here and the preview is closed again (a picture is never taken with a preview open)
    s.click(R + ".dev.present")

    s = j.step("par-attachments-absent-size", "Sizes are in the app's language; a partly-here set counts the files that are here")
    s.click("mac-settings-general.appearance.language.en")
    # main79: the one unwaited read met "1 file · not on this Mac yet", a header that holds until a fetch lands, and no value said whether the fetch was
    # late or never came. The read is waited for (the `pw` budget) and a miss prints the header as it was last served, its code points and the seconds waited.
    waited = {}
    def size_en_waited():
        t0 = time.time()
        got = pw(lambda: size_read(head(s), SIZE_EN), True)
        waited["s"] = round(time.time() - t0, 2)
        return got if got is True else "%s (after waiting %.2f s of a %.1f s budget)" % (got, waited["s"], 0 if FAST else WAIT_S)
    s.expect("size-is-english-in-an-english-app", size_en_waited, True)
    s.click("mac-settings-general.appearance.language.fr")
    s.expect("size-is-french-in-a-french-app", lambda: size_read(head(s), SIZE_FR), True)
    s.click("mac-settings-general.appearance.language.en")
    to_row(s, PHOTO_ROW)
    s.click(R + ".dev.absent")                                         # the three photos are not here; Space on the first fetches that one only
    s.click(R + ".0")
    s.key("Space")
    s.click(R + ".download.finish")
    want = fmt("%lld files · %lld on this Mac", 3, 1)
    s.expect("a-set-with-two-files-missing-counts-the-one-here", lambda: pw(lambda: head(s), want), want)
    s.key("Escape")
    s.key("Escape")
    s.click(R + ".dev.present")

    s = j.step("par-attachments-absent-button", "The Quick Look button on a file that is not on this Mac")
    to_row(s, PDF_ROW)
    s.click(R + ".dev.absent")
    s.click(R + ".gallery.quick-look")
    s.expect("the-button-starts-the-fetch-and-waits", lambda: pw(lambda: (chip(s, 0)["state"], panel(s)["open"]), ("fetching", False)), ("fetching", False))
    s.click(R + ".download.finish")
    s.expect("the-button-opens-the-preview-when-the-bytes-land", lambda: pw(lambda: (panel(s)["open"], panel(s)["item"]), (True, pdf)), (True, pdf))
    s.key("Escape")
    s.expect("escape-closes-it", lambda: pw(lambda: panel(s)["open"], False), False)
    s.click(R + ".dev.present")

    s = j.step("par-attachments-absent-saveall", "Save all on a file that is not on this Mac")
    to_row(s, PDF_ROW)
    s.click(R + ".dev.absent")
    stem = os.path.splitext(pdf)[0]
    # the Downloads folder keeps what the menu steps saved before ("quote-Q1-print-run.pdf"): Save all names its file freely ("... 2.pdf"),
    # so the case counts the files of that stem, never a fixed name (main71: the fixed name read "... 2.pdf" and failed)
    s._act(lambda: seen.update(files=saved_count(s, stem)))
    s.click(R + ".gallery.save-all")
    s.expect("save-all-fetches-the-absent-file", lambda: pw(lambda: chip(s, 0)["state"], "fetching"), "fetching")
    s.click(R + ".download.finish")
    s.expect("save-all-saves-it-once-when-the-bytes-land", lambda: pw(lambda: saved_count(s, stem) - seen["files"], 1), 1)
    s.expect("the-saved-file-is-the-attachment", lambda: os.path.basename(chip(s, 0)["saved_path"]).startswith(stem), True)
    s.click(R + ".dev.present")

    s = j.step("par-attachments-absent-failed", "A failed fetch says why on the strip; Space asks again")
    to_row(s, PDF_ROW)
    s.click(R + ".dev.absent")
    s.click(R + ".dev.fail.on")
    s.click(R + ".0")
    s.key("Space")
    s.expect("a-failed-fetch-is-failed-with-its-reason", lambda: pw(lambda: (chip(s, 0)["state"], chip(s, 0)["why"], panel(s)["open"]), ("failed", WHY, False)), ("failed", WHY, False))
    s.expect("the-chip-says-why-in-one-warn-line", lambda: (chip(s, 0)["subtitle"], chip(s, 0)["failed_line_warn"]), (fmt("Not fetched: %@. Space retries.", WHY), True))
    s.click(R + ".dev.fail.off")
    s.key("Space")
    s.expect("space-asks-again-after-a-failure", lambda: pw(lambda: (chip(s, 0)["state"], chip(s, 0)["waiting"]), ("fetching", "quick-look")), ("fetching", "quick-look"))
    s.click(R + ".download.finish")
    s.expect("the-retry-opens-the-preview-and-clears-the-line", lambda: pw(lambda: (panel(s)["open"], chip(s, 0)["failed_line_warn"]), (True, False)), (True, False))
    s.key("Escape")
    s.click(R + ".dev.present")

    # main71: the rig doors leaked. dev.absent kept the three photo names lazy for the rounds after (the gallery's msg-g1 shares them: 1 of 3
    # tiles had its picture) and Save all started a second fetch of a file already on its way (gallery-30: "…" on a chip). The selftest's
    # model reset itself on every message and had one fetch per file, so it could not see either: it now keeps both.
    s = j.step("par-attachments-absent-leaves-nothing", "After the absent rounds the fixture is as it was: the photos are here, Save all fetches each file once")
    to_row(s, PHOTO_ROW)
    s.expect("the-photos-are-present-tiles-again", lambda: pw(lambda: ([r["state"] for r in s.ctl.rows(R)], [r["draws"] for r in s.ctl.rows(R)]),
                                                              (["present"] * 3, ["tile"] * 3)), (["present"] * 3, ["tile"] * 3))
    # main79: the three photos are here (dev.present reset the counter), so Save all saves each at once and starts no fetch: the counts are the same after
    # it as before (the real socket served [1, 0, 0]: the tile draw may have started one), and no file has been fetched twice. The old [1, 1, 1] asked for a
    # fetch the save path never starts for a file that is here (AttachmentActs+Set.swift saveAll: `if let url = store.peek(ref).url { save; continue }`).
    fetched = lambda: [r["fetches"] for r in s.ctl.rows(R)]
    before = {}
    s._act(lambda: before.update(n=fetched()))
    s.click(R + ".gallery.save-all")
    if not FAST:
        s._act(lambda: time.sleep(0.4))                                 # a fetch that a late task would start has started by now
    s.expect("save-all-starts-no-second-fetch",
             lambda: (lambda after: ([a - b for a, b in zip(after, before["n"])], all(n <= 1 for n in after)))(pw(fetched, before["n"])),
             ([0, 0, 0], True))


def absent_chip_round(j):
    """Owner pick Attachment T1: a file that is not on this Mac draws as the SAME chip as a document, with a cloud, never as the large
    picture tile; its three states have their words; the space cap shows only on the selected chip; when the file arrives an image
    becomes its picture tile in the same place. The photo mail has three JPGs (the chip's kind is an image)."""
    photos = ROWS[PHOTO_ROW]["attachments"]
    draws = lambda s: [r["draws"] for r in s.ctl.rows(R)]
    gh = lambda s: s.ctl.measure(R + ".gallery.frame")["frame"]["h"]
    seen = {}

    s = j.step("par-attachments-chip-absent", "Not on this Mac yet: the chip with a cloud, not the big tile")
    to_row(s, PHOTO_ROW)
    s.click(R + ".dev.absent")
    s._act(lambda: seen.update(h0=gh(s)))
    s.expect("an-absent-image-draws-the-chip-not-the-tile", lambda: (draws(s), gallery(s)["tiles"], gallery(s)["chips"]), (["chip"] * 3, [], photos))
    s.expect("the-absent-chip-has-a-cloud-and-says-not-on-this-mac-yet", lambda: ([r["glyph"] for r in s.ctl.rows(R)], [r["subtitle"] for r in s.ctl.rows(R)]),
             (["cloud"] * 3, [fmt("Not on this Mac yet")] * 3))
    s.click(R + ".hover.none")
    s.click(R + ".1.hover")
    s.expect("the-cap-is-not-shown-on-a-hovered-absent-chip", lambda: [r["keys_shown"] for r in s.ctl.rows(R)], [False, False, False])
    s.click(R + ".0")
    s.expect("the-cap-shows-only-on-the-selected-chip", lambda: [r["keys_shown"] for r in s.ctl.rows(R)], [True, False, False])
    s.click(R + ".hover.none")

    s = j.step("par-attachments-chip-fetching", "Fetching: the type glyph, the line, the ring; no figure, no 0 %")
    s.click(R + ".dev.percent.off")
    s.key("Space")
    s.expect("fetching-without-a-figure-says-fetching-alone", lambda: pw(lambda: (chip(s, 0)["glyph"], chip(s, 0)["subtitle"], chip(s, 0)["percent"]), ("type", fmt("Fetching"), None)), ("type", fmt("Fetching"), None))
    s.click(R + ".dev.percent.on")
    s.click(R + ".download.half")
    s.expect("fetching-with-a-figure-says-it", lambda: pw(lambda: (chip(s, 0)["subtitle"], chip(s, 0)["percent"]), (fmt("Fetching · %@", "50%"), 50)), (fmt("Fetching · %@", "50%"), 50))

    s = j.step("par-attachments-chip-arrives", "Arrival: the image becomes its picture tile in the same place; the strip keeps its height")
    s.click(R + ".download.finish")
    s.expect("the-arrived-image-is-a-picture-tile", lambda: pw(lambda: (draws(s)[0], gallery(s)["tiles"]), ("tile", photos[:1])), ("tile", photos[:1]))
    # what is measured: the gallery's own frame height (mac-main.attachments.gallery.frame) before the arrival, with three chips, and after,
    # with one tile and two chips. The swap may add the difference between a tile and a chip, never more than one tile side.
    s.expect("the-strip-grows-by-at-most-one-tile-at-arrival", lambda: abs(gh(s) - seen["h0"]) <= gallery(s)["tile_side"], True)
    s.key("Escape")
    s.key("Escape")
    s.click(R + ".dev.present")                                         # the later rounds meet the photo mail as the fixture serves it


def file_there(s, path):
    """True when the file is on disk (the selftest's model keeps its own list of what a save wrote)."""
    has = getattr(s.ctl, "has_file", None)
    return has(path) if has else os.path.exists(path)


def unsave(s, path, before):
    """Undo of a sweep save: removes the ONE file the case saved, and only inside the test copy's own downloads folder (never the owner's)."""
    if not s.j.ctl or not path or path == before:
        return
    drop = getattr(s.j.ctl, "drop_file", None)
    if drop:
        drop(path)
        return
    root = os.environ.get("WHOOSH_SOCKET_DIR", "")
    down = os.path.realpath(os.path.join(root, "downloads")) if root else ""
    real = os.path.realpath(path)
    if down and os.path.dirname(real) == down and os.path.isfile(real):
        os.remove(real)
    else:
        s.j.notes.append("sweep: %s is not a file of the test copy's downloads folder, so it was left" % path)


def sheet_wait(s):
    """The Save As sheet of a visible background copy connects slowly (see menu_round); a hidden copy never draws one."""
    if not FAST and s.j.ctl and not hidden(s):
        time.sleep(SHEET_SETTLE_S)


def sweep_round(j):
    """The sweep (owner 2026-11-11: "the email sender account selector button isn't working, you haven't properly tested every button"):
    every control of the attachments screen that no round above clicked, clicked once with its visible outcome: the right-click menu of a
    photo TILE (each item), every app of Open With, and the composer's pending file (its click, hover, right-click menu, each menu item,
    drag). Each case puts the screen back: the Quick Look closed, the saved file removed from the test copy's own folder, the pending
    file removed. A Quick Look item answers on a hidden copy with `system_panel: false` (no real panel opens); that answer is asserted."""
    photos = ROWS[PHOTO_ROW]["attachments"]
    seen = {}
    panel_says = lambda s: panel(s)["system_panel"] != hidden(s)       # hidden copy: no system panel, and it says so; visible copy: the system panel

    s = j.step("par-attachments-sweep", "Every control clicked once: a tile's menu items, every Open With app, the composer's pending file and its menu")
    # the Copy case must see the pasteboard CHANGE (an earlier step left the first photo on it): the PDF chip's Copy puts another file there first
    to_row(s, PDF_ROW)
    s.click(R + ".0.menu.copy")
    to_row(s, PHOTO_ROW)
    s.click(R + ".0")
    s._act(lambda: seen.update(opens=chip(s, 0)["open_count"], saved0=chip(s, 0)["saved_path"]))
    s.click(R + ".0.menu.open")
    s.expect("tile-menu-open-opens-the-file", lambda: chip(s, 0)["open_count"] - seen["opens"], 1)
    s._act(lambda: seen.update(opens=chip(s, 0)["open_count"]))        # a fresh baseline: this case does not depend on the one above
    s.click(R + ".0.menu.open-with")
    s.expect("tile-menu-open-with-opens-with-the-default-app", lambda: (chip(s, 0)["open_count"] - seen["opens"], chip(s, 0)["opened_with"] != ""), (1, True))
    s.click(R + ".0.menu.quick-look")
    s.expect("tile-menu-quick-look-shows-the-photo-without-a-window-on-a-hidden-copy", lambda: pw(lambda: (panel(s)["open"], panel(s)["item"], panel_says(s)), (True, photos[0], True)), (True, photos[0], True))
    s.key("Escape")
    s.click(R + ".0.menu.save-to-downloads")
    s.expect("tile-menu-save-to-downloads-writes-the-file", lambda: pw(lambda: (chip(s, 0)["saved_path"] != seen["saved0"], os.path.basename(chip(s, 0)["saved_path"]).startswith(os.path.splitext(photos[0])[0]),
                                                                            "/downloads/" in chip(s, 0)["saved_path"], file_there(s, chip(s, 0)["saved_path"])), (True,) * 4), (True,) * 4)
    s.click(R + ".0.menu.show-in-finder")
    s.expect("tile-menu-show-in-finder-reveals-the-saved-file", lambda: chip(s, 0)["revealed"] == chip(s, 0)["saved_path"] != "", True)
    s._act(lambda: seen.update(saved1=chip(s, 0)["saved_path"]))
    s.click(R + ".0.menu.save-as")
    sheet_wait(s)
    s.expect("tile-menu-save-as-opens-the-panel-for-the-cancel", lambda: pw(lambda: state(s)["save_panel_open"], True, WAIT_S * 4), True)
    s.click(R + ".save-as.cancel")
    s.expect("tile-menu-save-as-cancel-closes-the-panel-and-saves-nothing", lambda: (pw(lambda: state(s)["save_panel_open"], False), chip(s, 0)["saved_path"] == seen["saved1"]), (False, True))
    s.click(R + ".0.menu.copy")
    s.expect("tile-menu-copy-puts-the-file-on-the-pasteboard", lambda: state(s)["pasteboard_file"], photos[0])
    s.click(R + ".0.menu.share")
    s.expect("tile-menu-share-hands-the-file-to-the-share-sheet", lambda: [os.path.basename(p) for p in chip(s, 0)["shared"]], [photos[0]])
    s._act(lambda: unsave(s, chip(s, 0)["saved_path"], seen["saved0"]))     # undo: the one file this step saved
    s.key("Escape")

    # Open With: every app of the submenu (the cases above click the first two; the model has a third)
    to_row(s, PDF_ROW)
    s._act(lambda: seen.update(ch0=chip(s, 0)["keys_shown"]))           # the document chip's own hover (the doors round hovers the photo tiles)
    s.click(R + ".0.hover")
    s.expect("chip-hover-shows-its-keys", lambda: (seen["ch0"], chip(s, 0)["keys_shown"]), (False, True))
    s.click(R + ".hover.none")
    s.expect("chip-hover-away-hides-its-keys", lambda: chip(s, 0)["keys_shown"], False)
    s.click(R + ".0")
    s.click(R + ".0.menu")
    titles = lambda: menu(s)["open-with"]["submenu"]
    used = []

    def every_app():
        seen["apps"] = [app_name(t) for t in titles()][:OPEN_WITH_DOORS]
        for n in range(len(seen["apps"])):
            s.j.ctl.click("%s.0.menu.open-with.%d" % (R, n))
            used.append(chip(s, 0)["opened_with"])
    s._act(every_app)

    def app_misses():
        if not seen.get("apps"):
            raise Unreadable("this Mac lists no app for a PDF in Open With: no door to click")
        return [(u, a) for u, a in zip_longest(used, seen["apps"]) if u != a]
    s.expect("open-with-every-app-of-the-submenu-opens-with-it", app_misses, [])
    s.key("Escape")

    # The composer's pending file: a pasted FILE (its name is its own), so a removal by name takes only it. A picture pasted as data
    # goes into the text, not on a tile (lead ruling, round 7), so the sweep's tile comes from a copied file.
    cax = lambda door="": "%s.%d%s" % (C, seen.get("pi", 1), "." + door if door else "")
    cp = lambda: chip(s, seen["pi"], C)
    for _ in range(3):
        s.key("Escape")
    s.key("C")
    s._act(lambda: seen.update(before=[r["name"] for r in s.ctl.rows(C)]))
    s.click(R + ".dev.stage-file")
    s.key("\u2318V")

    def pasted():
        pw(lambda: len(s.ctl.rows(C)), len(seen["before"]) + 1)
        seen["pi"] = len(s.ctl.rows(C)) - 1
        seen.update(name=cp()["name"], copens=cp()["open_count"], csaved0=cp()["saved_path"], h0=cp()["keys_shown"])
    s._act(pasted)
    s.click(cax("hover"))
    s.expect("composer-tile-hover-shows-its-keys", lambda: (seen["h0"], cp()["keys_shown"]), (False, True))
    s.click(R + ".hover.none")
    s.click(cax())
    s.expect("composer-tile-click-selects-it", lambda: cp()["selected"], True)
    s.click(cax("menu"))
    s.expect("composer-tile-menu-lists-the-items-with-remove-last", lambda: pw(lambda: list(menu(s).keys()), MENU + ["remove"]), MENU + ["remove"])
    s.click(cax("menu.open"))
    s.expect("composer-menu-open-opens-the-file", lambda: cp()["open_count"] - seen["copens"], 1)
    s._act(lambda: seen.update(copens=cp()["open_count"]))
    s.click(cax("menu.open-with"))
    s.expect("composer-menu-open-with-opens-with-the-app", lambda: (cp()["open_count"] - seen["copens"], cp()["opened_with"] != ""), (1, True))
    s.click(cax("menu.quick-look"))
    s.expect("composer-menu-quick-look-shows-the-file-without-a-window-on-a-hidden-copy", lambda: pw(lambda: (panel(s)["open"], panel(s)["item"], panel_says(s)), (True, seen["name"], True)), (True, seen["name"], True))
    s.key("Escape")
    s.click(cax("menu.save-to-downloads"))
    s.expect("composer-menu-save-to-downloads-writes-the-file", lambda: pw(lambda: (cp()["saved_path"] != seen["csaved0"], os.path.basename(cp()["saved_path"]).endswith(seen["name"]), "/downloads/" in cp()["saved_path"],
                                                                                 file_there(s, cp()["saved_path"])), (True,) * 4), (True,) * 4)
    s._act(lambda: seen.update(csaved1=cp()["saved_path"]))
    s.click(cax("menu.save-as"))
    sheet_wait(s)
    s.expect("composer-menu-save-as-opens-the-panel-for-the-cancel", lambda: pw(lambda: state(s)["save_panel_open"], True, WAIT_S * 4), True)
    s.click(R + ".save-as.cancel")
    s.expect("composer-menu-save-as-cancel-closes-the-panel-and-saves-nothing", lambda: (pw(lambda: state(s)["save_panel_open"], False), cp()["saved_path"] == seen["csaved1"]), (False, True))
    s.click(cax("menu.copy"))
    s.expect("composer-menu-copy-puts-the-file-on-the-pasteboard", lambda: (state(s)["pasteboard_file"], cp()["notice"] != ""), (seen["name"], True))
    s.click(cax("menu.share"))
    s.expect("composer-menu-share-hands-the-file-to-the-share-sheet", lambda: [os.path.basename(p) for p in cp()["shared"]], [seen["name"]])
    s.click(cax("drag"))
    s.expect("composer-tile-drag-carries-the-file", lambda: (cp()["drag_kind"], cp()["drag_file_exists"]), ("file", True))
    s._act(lambda: unsave(s, cp()["saved_path"], seen["csaved0"]))      # undo: the one file this step saved
    s.click(cax("menu.remove"))                                        # undo: the pending file goes, the composer is as it was
    s.expect("composer-menu-remove-leaves-the-composer-as-it-was", lambda: [r["name"] for r in s.ctl.rows(C)], seen.get("before"))
    s.key("Escape")


SWEEP_MUTANTS = [   # (check label, the selftest model's mutant that must turn exactly it red)
    ("par-attachments-sweep/tile-menu-open-opens-the-file", "tile-menu-open-dead"),
    ("par-attachments-sweep/tile-menu-open-with-opens-with-the-default-app", "tile-menu-open-with-dead"),
    ("par-attachments-sweep/tile-menu-quick-look-shows-the-photo-without-a-window-on-a-hidden-copy", "tile-menu-quick-look-dead"),
    ("par-attachments-sweep/tile-menu-save-to-downloads-writes-the-file", "tile-menu-save-to-downloads-dead"),
    ("par-attachments-sweep/tile-menu-show-in-finder-reveals-the-saved-file", "tile-menu-show-in-finder-dead"),
    ("par-attachments-sweep/tile-menu-save-as-opens-the-panel-for-the-cancel", "tile-save-as-dead"),
    ("par-attachments-sweep/tile-menu-save-as-cancel-closes-the-panel-and-saves-nothing", "tile-cancel-saves"),
    ("par-attachments-sweep/tile-menu-copy-puts-the-file-on-the-pasteboard", "tile-menu-copy-dead"),
    ("par-attachments-sweep/tile-menu-share-hands-the-file-to-the-share-sheet", "tile-menu-share-dead"),
    ("par-attachments-sweep/chip-hover-shows-its-keys", "chip-hover-dead"),
    ("par-attachments-sweep/chip-hover-away-hides-its-keys", "chip-hover-sticks"),
    ("par-attachments-sweep/open-with-every-app-of-the-submenu-opens-with-it", "open-with-third-dead"),
    ("par-attachments-sweep/composer-tile-hover-shows-its-keys", "composer-hover-dead"),
    ("par-attachments-sweep/composer-tile-click-selects-it", "composer-click-dead"),
    ("par-attachments-sweep/composer-tile-menu-lists-the-items-with-remove-last", "composer-menu-no-remove"),
    ("par-attachments-sweep/composer-menu-open-opens-the-file", "composer-menu-open-dead"),
    ("par-attachments-sweep/composer-menu-open-with-opens-with-the-app", "composer-menu-open-with-dead"),
    ("par-attachments-sweep/composer-menu-quick-look-shows-the-file-without-a-window-on-a-hidden-copy", "composer-menu-quick-look-dead"),
    ("par-attachments-sweep/composer-menu-save-to-downloads-writes-the-file", "composer-menu-save-to-downloads-dead"),
    ("par-attachments-sweep/composer-menu-save-as-opens-the-panel-for-the-cancel", "composer-menu-save-as-dead"),
    ("par-attachments-sweep/composer-menu-save-as-cancel-closes-the-panel-and-saves-nothing", "composer-cancel-saves"),
    ("par-attachments-sweep/composer-menu-copy-puts-the-file-on-the-pasteboard", "composer-menu-copy-dead"),
    ("par-attachments-sweep/composer-menu-share-hands-the-file-to-the-share-sheet", "composer-menu-share-dead"),
    ("par-attachments-sweep/composer-tile-drag-carries-the-file", "composer-drag-dead"),
    ("par-attachments-sweep/composer-menu-remove-leaves-the-composer-as-it-was", "composer-menu-remove-dead"),
]


ABSENT_CHIP_MUTANTS = [
    ("par-attachments-chip-absent/an-absent-image-draws-the-chip-not-the-tile", "tile-when-absent"),
    ("par-attachments-chip-absent/the-absent-chip-has-a-cloud-and-says-not-on-this-mac-yet", "no-cloud"),
    ("par-attachments-chip-absent/the-cap-is-not-shown-on-a-hovered-absent-chip", "cap-when-hovered"),
    ("par-attachments-chip-absent/the-cap-shows-only-on-the-selected-chip", "no-cap"),
    ("par-attachments-chip-fetching/fetching-without-a-figure-says-fetching-alone", "zero-percent"),
    ("par-attachments-chip-fetching/fetching-with-a-figure-says-it", "no-figure-ever"),
    ("par-attachments-chip-arrives/the-arrived-image-is-a-picture-tile", "stays-chip"),
    ("par-attachments-chip-arrives/the-strip-grows-by-at-most-one-tile-at-arrival", "strip-jumps"),
]


ABSENT_MUTANTS = [   # doors-read A123: (check label, the selftest model's mutant that must turn it red)
    ("par-attachments-absent-space/absent-file-is-named-absent-and-the-strip-prints-no-size", "size-when-absent"),
    ("par-attachments-absent-space/space-on-an-absent-file-starts-the-fetch-and-waits", "space-absent-dead"),
    ("par-attachments-absent-space/the-tile-shows-the-progress", "no-progress"),
    ("par-attachments-absent-space/the-ring-and-the-key-caps-do-not-share-the-chips-end", "ring-and-caps"),
    ("par-attachments-absent-space/the-name-is-cut-only-when-the-chip-is-full", "name-cut"),
    ("par-attachments-absent-space/the-chip-keeps-one-width-from-absent-to-present", "width-jumps"),
    ("par-attachments-absent-space/the-preview-opens-when-the-bytes-land", "preview-never"),
    ("par-attachments-absent-space/the-strip-names-no-absent-file-once-it-is-here", "strip-stale"),
    ("par-attachments-absent-space/space-closes-the-preview", "space-no-close"),
    ("par-attachments-absent-size/size-is-english-in-an-english-app", "size-system-language"),
    ("par-attachments-absent-size/size-is-french-in-a-french-app", "size-system-language"),
    ("par-attachments-absent-size/a-set-with-two-files-missing-counts-the-one-here", "partial-size"),
    ("par-attachments-absent-button/the-button-starts-the-fetch-and-waits", "button-absent-dead"),
    ("par-attachments-absent-button/the-button-opens-the-preview-when-the-bytes-land", "preview-never"),
    ("par-attachments-absent-button/escape-closes-it", "escape-no-close"),
    ("par-attachments-absent-saveall/save-all-fetches-the-absent-file", "saveall-absent-dead"),
    ("par-attachments-absent-saveall/save-all-saves-it-once-when-the-bytes-land", "saveall-twice"),
    ("par-attachments-absent-saveall/the-saved-file-is-the-attachment", "saveall-never-saves"),
    ("par-attachments-absent-failed/a-failed-fetch-is-failed-with-its-reason", "fail-silent"),
    ("par-attachments-absent-failed/the-chip-says-why-in-one-warn-line", "fail-silent"),
    ("par-attachments-absent-failed/space-asks-again-after-a-failure", "no-retry"),
    ("par-attachments-absent-failed/the-retry-opens-the-preview-and-clears-the-line", "line-sticks"),
    ("par-attachments-absent-leaves-nothing/the-photos-are-present-tiles-again", "present-no-reset"),
    ("par-attachments-absent-leaves-nothing/save-all-starts-no-second-fetch", "double-fetch"),
]


DOORS_MUTANTS = [   # doors-read: (check label, the selftest model's mutant that must turn it red)
    ("par-attachments-doors-remove/remove-button-removes-the-pending-file", "remove-button-dead"),
    ("par-attachments-doors-keys/command-c-copies-the-selected-file", "no-copy-key"),
    ("par-attachments-doors-keys/right-moves-the-selection-without-quick-look", "arrow-dead"),
    ("par-attachments-doors-keys/left-moves-it-back", "arrow-dead"),
    ("par-attachments-doors-hover/hover-shows-the-chip-keys", "hover-dead"),
    ("par-attachments-doors-hover/hover-away-hides-them", "hover-sticks"),
    ("par-attachments-doors-tile/the-photos-are-tiles-of-the-gallery", "not-a-gallery"),
    ("par-attachments-doors-tile/tile-double-click-opens-the-file", "tile-double-dead"),
    ("par-attachments-doors-tile/tile-menu-lists-the-items-in-finder-order", "tile-menu-short"),
    ("par-attachments-doors-tile/tile-drag-carries-the-file", "tile-drag-dead"),
    ("par-attachments-doors-openwith/open-with-the-default-app-door", "open-with-dead"),
    ("par-attachments-doors-openwith/open-with-the-second-app-opens-with-it", "open-with-first-only"),
    ("par-attachments-doors-saveas/save-as-opens-the-panel-for-the-cancel", "save-as-dead"),
    ("par-attachments-doors-saveas/save-as-cancel-closes-the-panel-and-saves-nothing", "cancel-saves"),
]


class Model:
    """The chip, tile, menu and composer on the wire for --selftest: what the doors round reads when the app behaves as designed.
    A mutant breaks exactly the behaviour its check names (DOORS_MUTANTS). Speaks the verbs the round uses; anything else is not served."""
    APPS = ["Preview", "Safari", "Chrome"]

    def __init__(self, mutant=None):
        self.m = mutant
        self.msg, self.sel, self.hover, self.menu_for = "pdf", None, None, None
        self.pending, self.staged, self.pasteboard, self.panel = ["sample-3-seconds.mp4"], False, "", False
        self.opens, self.with_app, self.notice, self.saved, self.dragged = {}, {}, {}, {}, {}
        self.leak = False                  # the rig keeps names lazy until dev.present, across messages (the real FixtureAttachmentSource does)
        self.dl = ["quote-Q1-print-run.pdf"]   # what the menu steps saved into Downloads before
        self.fetches = [0, 0, 0]
        # sweep: the menu's own acts on a tile, and the composer's pending files (each keeps its own state by name)
        self.files, self.rev, self.shr, self.nsave = set(), {}, {}, 0
        self.csel, self.chover, self.cs, self.cmenu, self.menu_owner, self.save_owner = None, None, {}, None, "r", None
        self.reset_file()

    def downloads(self):
        return list(self.dl)

    def save_file(self):
        """Save all writes its file under a free name; a mutant writes it twice or not at all."""
        if self.m == "saveall-never-saves":
            return
        name = self.names()[0]
        for _ in range(2 if self.m == "saveall-twice" else 1):
            stem, ext = os.path.splitext(name)
            self.dl.append(name if name not in self.dl else "%s 2%s" % (stem, ext))
        self.saved[0] = "/rig/downloads/" + self.dl[-1]

    def reset_file(self):
        """A message shown: its files are here (the photo mail's third is still downloading), no preview, English."""
        self.fetch, self.pct, self.why, self.waiting = "present", 0, "", ""
        self.fail, self.ql, self.ql_item, self.lang, self.stale = False, False, "", getattr(self, "lang", "en"), False
        self.nofig = False

    def names(self):
        return ROWS[PHOTO_ROW]["attachments"] if self.msg == "photo" else [ROWS[PDF_ROW]["attachments"][0]]

    # -- the socket
    def send(self, verb, *args):
        return {}

    def uistate(self):
        return {"surface": "mac-reader"}

    NAME_NEED = 170.0

    def measure(self, ax):
        """The gallery's frame height: the header, a chip per chip, a tile row when a tile shows (a mutant adds a jump at arrival).
        Also the chip's own parts: the fetch ring at its right end, the key caps (not drawn while fetching; the mutant draws them over the ring) and the name's box
        (a chip as wide as its content; the mutant is a half-width chip that cuts the name)."""
        if ax == R + ".fetch.ring":
            if self.fetch != "fetching":
                raise Unreadable("no measurement at %s: nothing on screen carries that id" % ax)
            return {"frame": {"x": 400.0, "y": 10.0, "w": 20.0, "h": 20.0}}
        if ax == R + ".0.keys":
            if self.fetch == "fetching" and self.m != "ring-and-caps":
                raise Unreadable("no measurement at %s: nothing on screen carries that id" % ax)
            return {"frame": {"x": 390.0 if self.fetch == "fetching" else 420.0, "y": 8.0, "w": 40.0, "h": 24.0}}
        if ax == R + ".0.name":
            return {"frame": {"x": 50.0, "y": 8.0, "w": self.NAME_NEED - (60.0 if self.m == "name-cut" else 0.0), "h": 16.0}}
        if ax == R + ".0":
            w = 245.0 if self.m == "name-cut" else self.NAME_NEED + 150.0
            if self.m == "width-jumps" and self.fetch == "fetching":
                w += 30.0
            return {"frame": {"x": 0.0, "y": 0.0, "w": w, "h": 40.0}}
        rows = [self.file_row(i) for i in range(len(self.names()))]
        tiles = sum(1 for r in rows if r["draws"] == "tile")
        h = 30 + 44 * (len(rows) - tiles) + (88 if tiles else 0) + (300 if (self.m == "strip-jumps" and tiles) else 0)
        return {"frame": {"x": 0, "y": 0, "w": 600, "h": h}}

    def need(self, i):
        if i >= len(self.names()):
            raise Unreadable("no chip %d" % i)
        return i

    def click(self, ax):
        m = self.m
        if ax.startswith(R + ".dev.focus."):
            mid = ax.rsplit(".", 1)[1]
            self.msg = "photo" if mid == ROWS[PHOTO_ROW]["id"] else "pdf"
            self.sel = None
            leak = self.leak
            self.reset_file()
            if leak:
                self.fetch = "absent"            # the names are still lazy: the message opens absent
        elif ax == "mac-main.reader.open":
            pass
        elif ax == R + ".dev.stage-image":
            self.staged = "image"
        elif ax == R + ".dev.stage-file":
            self.staged = "file"
        elif ax == R + ".hover.none":
            self.hover = self.hover if m == "hover-sticks" or (m == "chip-hover-sticks" and self.msg == "pdf") else None
            self.chover = None
        elif self.absent_click(ax):
            pass
        elif ax == R + ".save-as.cancel":
            self.panel = False
            if m == "cancel-saves":
                self.saved[0] = "/rig/save-as/" + self.names()[0]
            elif m == "tile-cancel-saves" and self.save_owner == "r" and self.msg == "photo":
                self.saved[0] = "/rig/save-as/" + self.names()[0]
            elif m == "composer-cancel-saves" and self.save_owner == "c":
                self.cstate(self.cmenu)["saved"] = "/rig/save-as/" + self.cmenu
        elif ax.startswith(C + "."):
            i, _, door = ax[len(C) + 1:].partition(".")
            if door != "remove":
                self.pending_click(int(i), door)
            elif m == "remove-button-dead" or int(i) >= len(self.pending):
                raise Unreadable("click %s: nothing happened" % ax)
            else:
                self.pending.pop(int(i))
        elif ax.startswith(R + "."):
            self.chip_click(ax[len(R) + 1:])
        else:
            raise Unreadable("click %s: nothing is registered there" % ax)
        return {}

    def absent_click(self, ax):
        """doors-read A123: the doors of a file that is not on this Mac; a mutant breaks exactly the behaviour its check names."""
        m = self.m
        if ax.startswith("mac-settings-general.appearance.language."):
            self.lang = ax.rsplit(".", 1)[1]
        elif ax == R + ".dev.absent":
            self.fetch, self.pct, self.why, self.waiting, self.ql = "absent", 0, "", "", False
            self.leak = True
        elif ax == R + ".dev.present":
            if self.m != "present-no-reset":
                self.leak = False
                self.reset_file()
                self.fetches = [1, 1, 1]       # the files appear again: one fetch each
        elif ax == R + ".dev.percent.off":
            self.nofig = True
        elif ax == R + ".dev.percent.on":
            self.nofig = False
        elif ax == R + ".dev.fail.on":
            self.fail = True
        elif ax == R + ".dev.fail.off":
            self.fail = False
        elif ax == R + ".gallery.quick-look":
            if self.fetch == "present":
                self.open_ql()
            elif m == "button-absent-dead":
                raise Unreadable("click %s: nothing happened" % ax)
            else:
                self.begin("quick-look")
        elif ax == R + ".gallery.save-all":
            if self.fetch == "absent" and m == "saveall-absent-dead":
                raise Unreadable("click %s: nothing happened" % ax)
            if self.fetch in ("present", "partial"):
                self.save_file()
                if self.m == "double-fetch":
                    self.fetches = [n + 1 for n in self.fetches]
            else:
                self.begin("save")
        elif ax == R + ".download.half":
            if self.fetch == "fetching":
                self.pct = 50
                self.nofig = self.m == "no-figure-ever"
        elif ax == R + ".download.finish":
            self.land()
        else:
            return False
        return True

    def open_ql(self):
        if self.m != "preview-never":
            self.ql, self.ql_item = True, self.names()[0]

    def begin(self, act):
        """The first act that needs the bytes: a fetch starts (or fails at once with its reason) and the act waits."""
        if self.fail:
            self.fetch, self.why = "failed", "" if self.m == "fail-silent" else WHY
            return
        self.fetch, self.pct = ("absent" if self.m == "no-progress" else "fetching"), 0
        self.why = self.why if self.m == "line-sticks" else ""
        self.waiting = act

    def land(self):
        if self.fetch != "fetching":
            return
        self.fetch, self.pct, act, self.waiting = ("partial" if self.msg == "photo" else "present"), 100, self.waiting, ""
        self.stale = self.m == "strip-stale"
        if act == "quick-look":
            self.open_ql()
        elif act == "save":
            self.save_file()

    def header(self):
        n, m = len(self.names()), self.m
        if self.fetch == "partial":
            return "3 files · 0 bytes on this Mac" if m == "partial-size" else fmt("%lld files · %lld on this Mac", 3, 1)
        if self.fetch in ("absent", "fetching", "failed") or self.stale:
            return "1 file · 0 bytes" if m == "size-when-absent" else fmt("1 file · not on this Mac yet")
        if self.lang == "fr":
            return "1 fichier · 12 kB" if m == "size-system-language" else "1 fichier · 12\u202fko"
        return "1 file · 12 ko" if m == "size-system-language" else "1 file · 12 kB"

    def chip_click(self, tail):
        m = self.m
        head, _, door = tail.partition(".")
        i = self.need(int(head))
        if door == "":
            self.sel = i
        elif door == "double" and m != "tile-double-dead":
            self.sel = i
            self.opens[i], self.with_app[i] = self.opens.get(i, 0) + 1, self.APPS[0]
        elif door == "hover" and m != "hover-dead" and not (m == "chip-hover-dead" and self.msg == "pdf"):
            self.hover = i
        elif door == "menu":
            self.sel, self.menu_for, self.menu_owner = i, i, "r"
        elif door.startswith("menu.open-with.") and m != "open-with-dead":
            n = 0 if m == "open-with-first-only" else int(door.rsplit(".", 1)[1])
            if m == "open-with-third-dead" and n >= 2:
                raise Unreadable("click %s: nothing happened" % tail)
            self.opens[i], self.with_app[i] = self.opens.get(i, 0) + 1, self.APPS[n]
        elif door == "menu.save-as" and m != "save-as-dead" and not (m == "tile-save-as-dead" and self.msg == "photo"):
            self.panel, self.save_owner = True, "r"
        elif door.startswith("menu.") and door[5:] in READER_ITEMS:
            self.reader_item(i, door[5:])
        elif door == "drag" and m != "tile-drag-dead":
            self.dragged[i] = True
        else:
            raise Unreadable("click %s: nothing happened" % tail)

    def cstate(self, name):
        return self.cs.setdefault(name, {"opens": 0, "with": "", "saved": "", "rev": "", "shared": [], "notice": "", "drag": False})

    def crow(self, name):
        st = self.cstate(name)
        return {"name": name, "selected": self.csel == name, "open_count": st["opens"], "opened_with": st["with"], "saved_path": st["saved"],
                "revealed": st["rev"], "shared": st["shared"], "notice": st["notice"], "keys_shown": self.csel == name or self.chover == name,
                "drag_kind": "file" if st["drag"] else "", "drag_file_exists": st["drag"]}

    def has_file(self, path):
        return path in self.files

    def drop_file(self, path):
        self.files.discard(path)

    def reader_item(self, i, item):
        """A reader tile's menu item as a socket click: the same act the item runs. A mutant (tile-menu-<item>-dead) leaves the photo's item dead."""
        name = self.names()[i]
        if self.m == "tile-menu-%s-dead" % item and self.msg == "photo":
            raise Unreadable("click menu.%s: nothing happened" % item)
        if item in ("open", "open-with"):
            self.opens[i], self.with_app[i] = self.opens.get(i, 0) + 1, self.APPS[0]
        elif item == "quick-look":
            self.ql, self.ql_item = True, name
        elif item == "save-to-downloads":
            self.nsave += 1
            stem, ext = os.path.splitext(name)
            self.saved[i] = "/rig/downloads/%s (menu %d)%s" % (stem, self.nsave, ext)
            self.files.add(self.saved[i])
        elif item == "show-in-finder":
            if i not in self.saved:
                raise Unreadable("click menu.show-in-finder: the item is disabled until a save")
            self.rev[i] = self.saved[i]
        elif item == "copy":
            self.pasteboard, self.notice[i] = name, "Copied"
        elif item == "share":
            self.shr[i] = ["/rig/files/" + name]

    def pending_click(self, i, door):
        """A pending file's doors in the composer: select, hover, the right-click menu, drag, and each menu item; a mutant breaks exactly one."""
        m = self.m
        if i >= len(self.pending):
            raise Unreadable("click %s.%d.%s: nothing happened" % (C, i, door))
        name = self.pending[i]
        st = self.cstate(name)
        dead = lambda what: m == "composer-%s-dead" % what
        if door == "":
            if dead("click"):
                raise Unreadable("click: nothing happened")
            self.csel = name
        elif door == "hover" and not dead("hover"):
            self.chover = name
        elif door == "menu":
            self.csel, self.cmenu, self.menu_owner = name, name, "c"
        elif door == "drag" and not dead("drag"):
            st["drag"] = True
        elif door == "menu.open" and not dead("menu-open") or door == "menu.open-with" and not dead("menu-open-with"):
            st["opens"], st["with"] = st["opens"] + 1, self.APPS[0]
        elif door == "menu.quick-look" and not dead("menu-quick-look"):
            self.ql, self.ql_item = True, name
        elif door == "menu.save-to-downloads" and not dead("menu-save-to-downloads"):
            self.nsave += 1
            st["saved"] = "/rig/downloads/%d-%s" % (self.nsave, name)
            self.files.add(st["saved"])
        elif door == "menu.save-as" and not dead("menu-save-as"):
            self.panel, self.save_owner, self.cmenu = True, "c", name
        elif door == "menu.copy" and not dead("menu-copy"):
            self.pasteboard, st["notice"] = name, "Copied"
        elif door == "menu.share" and not dead("menu-share"):
            st["shared"] = ["/rig/pending/" + name]
        elif door == "menu.show-in-finder":
            st["rev"] = "/rig/pending/" + name
        elif door == "menu.remove" and not dead("menu-remove"):
            self.pending.pop(i)
        elif door == "double":
            st["opens"], st["with"] = st["opens"] + 1, self.APPS[0]
        else:
            raise Unreadable("click %s.%d.%s: nothing happened" % (C, i, door))

    def key(self, chord):
        m = self.m
        if chord == "Space" and self.sel is not None:
            if self.ql:
                if m == "space-no-close":
                    raise Unreadable("key Space: not taken")
                self.ql = False
            elif self.fetch == "present":
                self.open_ql()
            elif (self.fetch == "absent" and m == "space-absent-dead") or (self.fetch == "failed" and m == "no-retry"):
                raise Unreadable("key Space: not taken")
            else:
                self.begin("quick-look")
        elif chord == "Escape" and self.ql:
            if m != "escape-no-close":
                self.ql = False
        elif chord == "Escape":
            self.sel = None
        elif chord == "\u2318C" and self.sel is not None and m != "no-copy-key":
            self.pasteboard, self.notice[self.sel] = self.names()[self.sel], "Copied"
        elif chord in ("Right", "Left") and self.sel is not None and m != "arrow-dead":
            to = self.sel + (1 if chord == "Right" else -1)
            if not 0 <= to < len(self.names()):
                raise Unreadable("key %s: not taken" % chord)
            self.sel = to
        elif chord == "\u2318V" and self.staged:
            if self.staged == "file":                    # a copied file is a pending tile; picture data goes into the text (no tile)
                self.pending.append("sample-2-pages.pdf")
            self.staged = False
        else:
            raise Unreadable("key %s: not taken" % chord)
        return {}

    def file_row(self, i):
        m = self.m
        mine = self.msg != "photo" or i == 0
        state = ("present" if i == 0 else "absent") if self.fetch == "partial" else (self.fetch if mine else ("present" if self.fetch == "present" else "absent"))
        if self.m == "stays-chip" and state == "present" and self.msg == "photo":
            draws = "chip"
        else:
            draws = "tile" if self.msg == "photo" and (state == "present" or (m == "tile-when-absent")) else "chip"
        fig = None if (state == "fetching" and self.nofig and m != "zero-percent") else self.pct
        if state == "fetching" and self.nofig:
            sub = fmt("Fetching · %@", "0 %") if m == "zero-percent" else fmt("Fetching")
        elif state == "fetching":
            sub = fmt("Fetching · %@", "%d%%" % self.pct)
        elif state == "absent":
            sub = fmt("Not on this Mac yet")
        elif state == "failed":
            sub = fmt("Not fetched: %@. Space retries.", self.why)
        else:
            sub = "JPG · 12 kB"
        glyph = {"absent": "icon" if m == "no-cloud" else "cloud", "fetching": "type", "failed": "cloud", "present": "picture"}[state]
        keys = (self.sel == i) if state != "present" and m != "cap-when-hovered" else (self.sel == i or self.hover == i)
        keys = False if m == "no-cap" else keys
        return {"state": state, "percent": (100 if state == "present" else fig), "why": self.why if state == "failed" else "", "draws": draws,
                "glyph": glyph, "subtitle": sub, "failed_line_warn": state == "failed" or (m == "line-sticks" and bool(self.why)), "keys_shown": keys,
                "waiting": self.waiting, "saved_path": self.saved.get(i, "/rig/save-as/old"), "fetches": self.fetches[i], "name_need": self.NAME_NEED}

    def base_rows(self):
        return [{"name": n, "selected": self.sel == i, "open_count": self.opens.get(i, 0),
                 "opened_with": self.with_app.get(i, ""), "notice": self.notice.get(i, ""),
                 "drag_kind": "file" if self.dragged.get(i) else "", "drag_file_exists": bool(self.dragged.get(i)),
                 "revealed": self.rev.get(i, ""), "shared": self.shr.get(i, [])} for i, n in enumerate(self.names())]


    def rows(self, ax):
        if ax == C:
            return [self.crow(n) for n in self.pending]
        if ax == R + ".panel":
            return [{"open": self.ql, "item": self.ql_item, "system_panel": True}]
        if ax == R:
            return [dict(r, **self.file_row(i)) for i, r in enumerate(self.base_rows())]
        if ax == R + ".state":
            return [{"pasteboard_file": self.pasteboard, "save_panel_open": self.panel}]
        if ax == R + ".gallery":
            rows = [self.file_row(i) for i in range(len(self.names()))]
            tiles = [n for n, r in zip(self.names(), rows) if r["draws"] == "tile"]
            return [{"media": self.names() if self.m != "not-a-gallery" else [], "header": self.header(), "tiles": tiles,
                     "chips": [n for n in self.names() if n not in tiles], "tile_side": 88}]
        if ax == R + ".menu":
            if self.menu_owner == "c":
                if self.cmenu is None:
                    return []
                return [{"id": i, "submenu": ["%s (default)" % self.APPS[0]] + self.APPS[1:] if i == "open-with" else []}
                        for i in MENU + ([] if self.m == "composer-menu-no-remove" else ["remove"])]
            if self.menu_for is None:
                return []
            items = [i for i in MENU if not (self.m == "tile-menu-short" and i == "share")]
            return [{"id": i, "submenu": ["%s (default)" % self.APPS[0]] + self.APPS[1:] if i == "open-with" else []} for i in items]
        raise Unreadable("the model has no region %s" % ax)


def selftest():
    """The doors round green on the model, then red on each of its mutants. The older rounds drive the app (the socket); their mutants
    are the app's own (--mutants <bundle>)."""
    def run(mutant):
        j = Journey(ID, Model(mutant))
        doors_round(j)
        absent_round(j)
        absent_chip_round(j)
        sweep_round(j)
        return {c.label: c for c in j.checks()}
    every = DOORS_MUTANTS + ABSENT_MUTANTS + ABSENT_CHIP_MUTANTS + SWEEP_MUTANTS
    clean, bad, labels = run(None), 0, [l for l, _ in every]
    declared = {l for l in declared_mutants(os.path.abspath(__file__)) if l.startswith(("par-attachments-doors-", "par-attachments-absent-", "par-attachments-chip-", "par-attachments-sweep/"))}
    missing = sorted((set(clean) - set(labels)) | (set(clean) - declared))
    stray = sorted((set(labels) - set(clean)) | (declared - set(clean)))
    for l in missing:
        print("NO MUTANT %s" % l)
    for l in stray:
        print("STRAY MUTANT %s" % l)
    bad += len(missing) + len(stray)
    print("MUTANTS | check | clean | mutant | mutant result")
    for label, mutant in every:
        c, red = clean.get(label), run(mutant).get(label)
        ok_clean, ok_red = c is not None and c.state == "pass", red is not None and red.state == "fail"
        print("MUTANTS | %s | %s | %s | %s" % (label, "green" if ok_clean else "RED (%s)" % (c.detail if c else "absent"), mutant,
                                               "red" if ok_red else "STILL GREEN (%s)" % (red.state if red else "absent")))
        bad += (not ok_clean) + (not ok_red)
    print("%d checks, %d failed, 0 not judged" % (len(every), bad))
    return 1 if bad else 0


def menu_round(j, pdf):
    s = j.step("par-attachments-menu", "Right-click: every item present, keyed, and acting")
    to_row(s, PDF_ROW)
    s.click(R + ".0.menu")
    # shown on a visible copy; on the hidden rig the menu is built for the socket and never drawn (no window, ever)
    s.expect("menu-opens-at-the-chip", lambda: poll(lambda: state(s)["menu_opens"] >= 1 or (state(s)["hidden"] and state(s)["menus_built_hidden"] >= 1), True), True)
    s.expect("menu-items-in-finder-order", lambda: list(menu(s).keys()), MENU)
    s.expect("menu-keys-shown", lambda: (menu(s)["open"]["key"], menu(s)["quick-look"]["key"], menu(s)["copy"]["key"]), ("⌘O", "Space", "⌘C"))
    s.expect("open-with-lists-the-default-first", lambda: (menu(s)["open-with"]["submenu"] or [""])[0].endswith(")"), True)   # "(default)" in the app's language
    s.expect("share-is-the-system-share-item", lambda: (menu(s)["share"]["enabled"], menu(s)["share"]["title"] != ""), (True, True))  # the system names it in the system's language
    s.expect("show-in-finder-waits-for-a-save", lambda: menu(s)["show-in-finder"]["enabled"], False)
    s.expect("menu-closes", lambda: poll(lambda: state(s)["menu_shown"], False), False)
    try:
        n = chip(s, 0)["open_count"] if s.j.ctl else 0
    except Exception as e:                                           # noqa: BLE001  the checks below judge the state
        s.j.notes.append("menu: open count unread: %s" % e)
        n = 0
    s.click(R + ".0.menu.open")
    s.expect("menu-open-opens", lambda: chip(s, 0)["open_count"], n + 1)
    s.click(R + ".0.menu.open-with")
    s.expect("menu-open-with-opens-with-the-app", lambda: (chip(s, 0)["open_count"], chip(s, 0)["opened_with"] != ""), (n + 2, True))
    s.click(R + ".0.menu.quick-look")
    s.expect("menu-quick-look-opens-the-panel", lambda: poll(lambda: panel(s)["item"], pdf), pdf)
    s.key("Escape")
    settle(s, lambda: panel(s)["on_screen"], False)
    s.click(R + ".0.menu.save-to-downloads")
    s.expect("save-to-downloads-writes-the-file", lambda: (os.path.basename(chip(s, 0)["saved_path"]), "/downloads/" in chip(s, 0)["saved_path"],
                                                            os.path.exists(chip(s, 0)["saved_path"])), (pdf, True, True))
    s.click(R + ".0.menu.show-in-finder")
    s.expect("show-in-finder-reveals-the-saved-file", lambda: chip(s, 0)["revealed"] == chip(s, 0)["saved_path"] != "", True)
    s.click(R + ".0.menu.save-as")                                   # a background copy's save sheet can hold the main thread
    time.sleep(SHEET_SETTLE_S if s.j.ctl else 0)                     # ~7 s while it connects (measured 4.6-6.8 s on the rig;
                                                                     # 0.2 s on an active copy), longer than one reply's wait
    s.expect("save-as-opens-the-save-panel", lambda: poll(lambda: state(s)["save_panel_open"], True, WAIT_S * 4), True)
    s.click(R + ".save-as.save")
    s.expect("save-as-saves-where-the-panel-says", lambda: poll(lambda: "/save-as/" in chip(s, 0)["saved_path"] and os.path.exists(chip(s, 0)["saved_path"]), True), True)
    s.click(R + ".0.menu.copy")
    s.expect("copy-puts-the-file-on-the-pasteboard", lambda: (state(s)["pasteboard_file"], chip(s, 0)["notice"] != ""), (pdf, True))
    s.click(R + ".0.menu.share")
    s.expect("share-hands-the-file-to-the-share-sheet", lambda: [os.path.basename(p) for p in chip(s, 0)["shared"]], [pdf])
    s.click(R + ".0.drag")
    s.expect("drag-carries-the-file", lambda: (chip(s, 0)["drag_kind"], chip(s, 0)["drag_file"], chip(s, 0)["drag_file_exists"]), ("file", pdf, True))
    s.expect("drag-offers-a-file-url", lambda: "public.file-url" in chip(s, 0)["drag_types"], True)


def photo_round(j):
    photos = ROWS[PHOTO_ROW]["attachments"]
    s = j.step("par-attachments-step", "The photo mail: arrows step Quick Look; the panel follows the selection")
    to_row(s, PHOTO_ROW)
    s.expect("three-photo-chips", lambda: [r["name"] for r in s.ctl.rows(R)], photos)
    s.expect("photos-have-real-thumbnails", lambda: poll(lambda: [r["thumbnail"] for r in s.ctl.rows(R)][:2], ["thumbnail"] * 2), ["thumbnail"] * 2)
    s.click(R + ".0")
    s.key("Space")
    s.expect("panel-opens-on-the-first-photo", lambda: poll(lambda: panel(s)["item"], photos[0]), photos[0])
    s.key("Right")
    s.expect("right-steps-to-the-next-photo", lambda: poll(lambda: (panel(s)["item"], chip(s, 1)["selected"]), (photos[1], True)), (photos[1], True))
    s.key("Left")
    s.expect("left-steps-back", lambda: poll(lambda: panel(s)["item"], photos[0]), photos[0])
    s.click(R + ".1")
    s.expect("panel-follows-the-selection", lambda: poll(lambda: panel(s)["item"], photos[1]), photos[1])
    s.key("Escape")

    s = j.step("par-attachments-download", "A photo still downloading: progress in the chip; Quick Look waits, then opens")
    s.expect("third-photo-is-downloading", lambda: (chip(s, 2)["download"], chip(s, 2)["progress"]), ("downloading", 0))
    s.click(R + ".2")
    s.key("Space")
    s.expect("quick-look-waits-for-the-bytes", lambda: (chip(s, 2)["waiting"], panel(s)["open"]), ("quick-look", False))
    s.click(R + ".download.half")
    s.expect("progress-ring-moves", lambda: chip(s, 2)["progress"], 50)
    s.click(R + ".download.finish")
    s.expect("quick-look-opens-when-the-bytes-land", lambda: poll(lambda: panel(s)["item"], photos[2]), photos[2])
    s.expect("landed-photo-gets-its-thumbnail", lambda: poll(lambda: chip(s, 2)["thumbnail"], "thumbnail"), "thumbnail")
    s.key("Escape")


def sheet_round(j):
    sheet = ROWS[SHEET_ROW]["attachments"][0]
    s = j.step("par-attachments-open-waits", "The term sheet still downloading: Return waits, then opens; a drag carries a promise")
    to_row(s, SHEET_ROW)
    s.expect("sheet-is-downloading", lambda: chip(s, 0)["download"], "downloading")
    s.expect("docx-draws-its-document-icon", lambda: chip(s, 0)["thumbnail"], "icon")
    s.click(R + ".0.drag")
    s.expect("drag-before-download-is-a-file-promise", lambda: chip(s, 0)["drag_kind"], "promise")
    s.click(R + ".0")
    s.key("Enter")
    s.expect("open-waits-for-the-bytes", lambda: (chip(s, 0)["waiting"], chip(s, 0)["open_count"]), ("open", 0))
    s.click(R + ".download.finish")
    s.expect("open-runs-when-the-bytes-land", lambda: poll(lambda: chip(s, 0)["open_count"], 1), 1)
    s.expect("opened-file-is-the-sheet", lambda: os.path.basename(chip(s, 0)["opened"]), sheet)


def composer_round(j):
    video = os.path.join(ROOT, "docs", "design", "fixtures", "attachments", "sample-3-seconds.mp4")
    s = j.step("par-attachments-composer", "The composer: a dropped file and a pasted image become pending files with the same proxy")
    for _ in range(3):
        s.key("Escape")
    s.key("C")
    s.raw("fixture", "drop-file", "mac-composer.attachments", video)
    s.expect("dropped-file-is-pending", lambda: poll(lambda: [r["name"] for r in s.ctl.rows(C)], ["sample-3-seconds.mp4"]), ["sample-3-seconds.mp4"])
    s.expect("video-has-real-thumbnail", lambda: poll(lambda: chip(s, 0, C)["thumbnail"], "thumbnail"), "thumbnail")
    s.click(R + ".dev.stage-text")
    s.key("⌘V")
    s.expect("text-paste-is-left-to-the-editor", lambda: len(s.ctl.rows(C)), 1)
    # One rule for a paste (lead ruling, round 7, as mail clients do): a picture copied as DATA goes into the text, a picture in the
    # composer's header row `pictures`, never a tile; a copied FILE (a picture file included) is a pending tile.
    s.click(R + ".dev.stage-image")
    s.key("⌘V")
    s.expect("pasted-image-goes-into-the-text", lambda: poll(lambda: (len(s.ctl.rows("mac-composer.header")[0].get("pictures") or []), [r["name"] for r in s.ctl.rows(C)]),
                                                         (1, ["sample-3-seconds.mp4"])), (1, ["sample-3-seconds.mp4"]))
    s.click(R + ".dev.stage-file")
    s.key("⌘V")
    s.expect("pasted-file-is-pending", lambda: poll(lambda: [r["name"] for r in s.ctl.rows(C)], ["sample-3-seconds.mp4", "sample-2-pages.pdf"]), ["sample-3-seconds.mp4", "sample-2-pages.pdf"])
    s.click(C + ".0")
    s.key("Space")
    s.expect("pending-file-quick-looks", lambda: poll(lambda: panel(s)["item"], "sample-3-seconds.mp4"), "sample-3-seconds.mp4")
    s.key("Escape")
    s.click(C + ".0.double")
    s.expect("pending-file-opens", lambda: chip(s, 0, C)["open_count"], 1)
    s.click(C + ".1.menu.show-in-finder")
    s.expect("pending-file-shows-in-finder", lambda: chip(s, 1, C)["revealed"].endswith(chip(s, 1, C)["name"]) and chip(s, 1, C)["name"].endswith(".pdf"), True)
    s.click(C + ".1.menu.remove")
    s.expect("menu-remove-removes-the-pending-file", lambda: [r["name"] for r in s.ctl.rows(C)], ["sample-3-seconds.mp4"])


def shots_round(j):
    s = j.step("par-attachments-shots", "Shots, light and dark at 1280x820: the chip, Quick Look over it, the menu")
    for theme in ("light", "dark"):
        for _ in range(3):
            s.key("Escape")
        s.raw("theme", theme)
        to_row(s, PHOTO_ROW)
        s.click(R + ".0")
        time.sleep(const("HOVER_REVEAL_MS") / 1000 * 4 if s.j.ctl else 0)  # the key caps finish fading in
        keep_shot(s, "chip", theme)
        s.key("Space")
        settle(s, lambda: panel(s)["open"], True)
        time.sleep(1.5 if s.j.ctl else 0)                            # the zoom settles and the preview draws before the picture
        keep_shot(s, "quicklook", theme)
        s.key("Escape")
        settle(s, lambda: panel(s)["on_screen"], False)              # the system's zoom-out ends before the menu shot
        s.click(R + ".1.menu")                                       # the menu photographs itself, then closes
        settle(s, lambda: state(s)["menu_shown"], False)
        keep_shot(s, "menu", theme, take=False)
    s.expect("no-shot-is-blank", lambda: s.ctl and BLANK, [])
    want = ("chip",) if s.j.ctl and hidden(s) else ("chip", "quicklook", "menu")
    s.expect("six-shots-written", lambda: s.ctl and all(os.path.exists(os.path.join(CAP, "%s-%s-1280x820.png" % (n, t))) for n in want for t in ("light", "dark")), True)


CORPUS = os.path.join(ROOT, "docs", "design", "fixtures", "attachments", "gallery-corpus.json")


def open_message(s, mid):
    for _ in range(3):
        s.key("Escape")
    s.click("%s.dev.focus.%s" % (R, mid))
    s.click("mac-main.reader.open")


def gallery(s):
    return s.ctl.rows(R + ".gallery")[0]


def gallery_round(j):
    """The gallery (owner 15:57): tiles for images and videos, documents below, a cap with "+N more", inline images once."""
    import json
    fx = {m["id"]: m for m in json.load(open(CORPUS, encoding="utf-8"))["messages"]}
    rows = int(const("ATTACHMENT_GALLERY_ROWS"))
    s = j.step("par-attachments-gallery", "Three photos: three tiles with real pictures, no list")
    s.raw("fixture", "corpus", CORPUS)
    s.raw("theme", "light")
    open_message(s, "msg-g1")
    s.expect("three-photos-are-three-tiles", lambda: (gallery(s)["media"], gallery(s)["tiles_shown"], gallery(s)["documents"]), (fx["msg-g1"]["attachments"], 3, []))
    s.expect("tiles-carry-real-pictures", lambda: poll(lambda: gallery(s)["tiles_with_picture"], 3), 3)
    s.expect("tile-side-is-the-ledger-s", lambda: gallery(s)["tile_side"], int(const("ATTACHMENT_GALLERY_TILE_PT")))
    gallery_shot(s, "gallery-3", "light")

    s = j.step("par-attachments-gallery-30", "Thirty files: capped tiles and chips, '+N more' opens the rest in place")
    open_message(s, "msg-g2")
    media = [n for n in fx["msg-g2"]["attachments"] if n.rsplit(".", 1)[-1] in ("jpg", "png", "mp4")]
    docs = [n for n in fx["msg-g2"]["attachments"] if n not in media]
    s.expect("media-and-documents-split", lambda: (len(gallery(s)["media"]), len(gallery(s)["documents"])), (len(media), len(docs)))
    # owner on the 88 pt shot: the collapsed gallery is one full row, its last cell "+N" (cells from the pane width)
    s.expect("collapsed-gallery-is-one-full-row", lambda: gallery(s)["cells"] == gallery(s)["per_row"] * rows and gallery(s)["per_row"] > 1, True)
    s.expect("gallery-collapses-past-the-cap", lambda: (gallery(s)["tiles_shown"], gallery(s)["more"]),
             (gallery(s)["cells"] - 1, "+%d" % (len(media) - gallery(s)["cells"] + 1)) if s.j.ctl else None)
    s.expect("documents-collapse-past-the-cap", lambda: (gallery(s)["documents_shown"], gallery(s)["documents_more"]),
             (gallery(s)["cells"] - 1, len(docs) - gallery(s)["cells"] + 1) if s.j.ctl else None)
    s.expect("header-names-the-count", lambda: str(len(media) + len(docs)) in gallery(s)["header"], True)
    gallery_shot(s, "gallery-30-collapsed", "light")
    s.click(R + ".gallery.more")
    s.expect("more-opens-every-tile", lambda: (gallery(s)["tiles_shown"], gallery(s)["more"], gallery(s)["expanded"]), (len(media), "", True))
    s.expect("video-shows-its-first-frame-and-length", lambda: poll(lambda: [(v["first_frame"], v["duration"]) for v in gallery(s)["videos"]], [(True, "0:03")] * 2, WAIT_S * 2), [(True, "0:03")] * 2)
    s.click(R + ".documents.more")
    s.expect("more-opens-every-document", lambda: gallery(s)["documents_shown"], len(docs))
    gallery_shot(s, "gallery-30-expanded", "light")
    s.click(R + ".gallery.quick-look")
    s.expect("quick-look-over-the-whole-set", lambda: poll(lambda: (panel(s)["open"], panel(s)["count"]), (True, len(media) + len(docs))), (True, len(media) + len(docs)))
    s.key("Escape")
    settle(s, lambda: panel(s)["on_screen"], False)
    s.click(R + ".gallery.save-all")
    down = os.path.join(os.environ.get("WHOOSH_SOCKET_DIR", ""), "downloads")
    s.expect("saved-chips-keep-kind-and-size-and-show-a-check", lambda: poll(lambda: all(
        r["subtitle"].startswith(r["name"].rsplit(".", 1)[-1].upper()) and "…" not in r["subtitle"] and r["badge"] == "checkmark.circle"
        for r in s.ctl.rows(R) if r["group"] == "document"), True), True)
    s.expect("save-all-saves-every-file", lambda: poll(lambda: s.ctl and os.path.isdir(down) and len([f for f in os.listdir(down) if f.split(".")[0] in {n.split(".")[0] for n in media + docs}]) >= len(media + docs), True), True)
    s.click(R + ".%d" % fx["msg-g2"]["attachments"].index(media[-1]))        # a tile's id carries its place in the message
    s.key("Space")
    s.expect("a-tile-quick-looks-like-a-chip", lambda: poll(lambda: panel(s)["item"], media[-1]), media[-1])
    s.key("Escape")

    s = j.step("par-attachments-inline", "An inline image joins the gallery once; a data: image too")
    open_message(s, "msg-g3")
    s.expect("inline-cid-image-counted-once", lambda: gallery(s)["media"].count("floor-plan.png"), 1)
    s.expect("inline-data-image-joins-the-gallery", lambda: len(gallery(s)["inline"]), 1)
    s.expect("inline-tiles-get-pictures", lambda: poll(lambda: gallery(s)["tiles_with_picture"], 2), 2)
    gallery_shot(s, "gallery-inline", "light")
    for theme in ("dark",):
        s.raw("theme", theme)
        open_message(s, "msg-g1"); gallery_shot(s, "gallery-3", theme)
        open_message(s, "msg-g2"); s.click(R + ".dev.collapse"); gallery_shot(s, "gallery-30-collapsed", theme)
        s.click(R + ".gallery.more"); s.click(R + ".documents.more"); gallery_shot(s, "gallery-30-expanded", theme)


def sidebar(s, shown):
    """Shows or hides the sidebar with its own toggle, then reads that it did."""
    if not s.j.ctl:
        return
    try:
        if s.ctl.rows("mac-main.sidebar.visibility")[0]["hidden"] == shown:
            s.click("mac-main.sidebar.toggle.button")
        settle(s, lambda: s.ctl.rows("mac-main.sidebar.visibility")[0]["hidden"], not shown)
    except Exception as e:                                           # noqa: BLE001
        s.j.notes.append("sidebar %s: %s" % (shown, e))


RUNGS = {}                                                           # width label -> the rung the header used


def header_round(j):
    """Owner 23:04 and 23:07 ("should be in one line"): the gallery header is ONE line at every reader width: every frame
    shares one midY, no label is narrower than its text; the rung in use (1 words and cap, 2 words, 3 icons) is reported."""
    s = j.step("par-attachments-header", "The gallery header stays one line at three reader widths, light and dark")
    os.makedirs(os.path.join(CAP, "r3"), exist_ok=True)
    # 1280 first: its labels sit on one line, and their heights there are what "one line" means at every other width
    heights = {}
    for label, (w, h), shown in (("1280", (1280, 820), True), ("960-sidebar", (960, 632), True), ("960-no-sidebar", (960, 632), False)):
        for theme in ("light", "dark"):
            s.raw("theme", theme)
            s.raw("resize", str(w), str(h))
            sidebar(s, shown)
            open_message(s, "msg-g2")
            time.sleep(0.5 if s.j.ctl else 0)
            tag = "%s-%s" % (label, theme)

            def frames(s=s, tag=tag):
                # Frames from the shot reply, read in the same main-thread turn as the pixels (CONTROL-SOCKET delta 46);
                # a separate `measure` can belong to an earlier layout. Content-view points, the unit of text_width.
                row = s.ctl.rows(R + ".header")[0]
                ids = sorted(set(row["frames"] + list(row["text_width"])))
                reply = s.ctl.send("shot", "par-att-header-%s" % tag, *ids)
                return row, {ax: (reply.get("frames") or {}).get(ax) or {} for ax in ids}

            def one_line(s=s, label=label):
                row, f = frames()
                if not all(f.get(ax) for ax in row["text_width"]):     # a label with no frame: say which, and what the shot answered, instead of False
                    raise IndexError("no frame for %s; frames answered: %r; rung %r" % ([ax for ax in row["text_width"] if not f.get(ax)], f, row.get("rung")))
                mids = [v["y"] + v["h"] / 2 for v in f.values() if v]
                tall = {ax: f[ax].get("h", 0) for ax in row["text_width"]}
                if label == "1280":
                    heights.update(tall)
                same_height = all(abs(v - heights.get(ax, v)) <= 1 for ax, v in tall.items())
                return len(mids) >= 3 and max(mids) - min(mids) <= 1 and same_height
            s.expect("header-is-one-line-%s" % tag, one_line, True)

            def full_width(s=s, tag=tag):
                # each offender as (label, its measured width, its text's width): both numbers land in the log
                row, f = frames()
                RUNGS[tag] = row["rung"]
                return [(ax.rsplit(".", 1)[-1], f[ax].get("w"), width) for ax, width in row["text_width"].items()
                        if f[ax].get("w", 0) < width - 1]
            s.expect("no-label-narrower-than-its-text-%s" % tag, full_width, [])
            if label in ("960-sidebar", "1280"):
                gallery_shot(s, "header-%s" % label, theme, "%dx%d" % (w, h), sub="r3")
    sidebar(s, True)
    s.raw("resize", "1280", "820")
    want = ["header-%s-%s-%s.png" % (l, t, sz) for l, sz in (("960-sidebar", "960x632"), ("1280", "1280x820")) for t in ("light", "dark")]
    s.expect("header-shots-written", lambda: s.ctl and all(os.path.exists(os.path.join(CAP, "r3", f)) for f in want), True)
    s.j.notes.append("header rungs: %s" % json.dumps(RUNGS, sort_keys=True))


def gallery_shot(s, name, theme, size="1280x820", sub=""):
    """The main window as the app's own shot verb takes it, into the capture set."""
    if not s.j.ctl:
        return
    time.sleep(0 if FAST else 1.0)                                   # thumbnails land and the tiles fade in (the selftest's model has no thumbnails to wait for)
    try:
        r = s.ctl.shot("attachments-%s-%s" % (name, theme))                 # a flat name: the shot verb writes it in its own folder
        shutil.copy(r["path"], os.path.join(CAP, sub, "%s-%s-%s.png" % (name, theme, size)))
    except Exception as e:                                           # noqa: BLE001
        BLANK.append("%s-%s (%s)" % (name, theme, e))


MUTANTS = """# MUTANTS (WHOOSH_ATTACH_MUTANT=<name> on one build; the label goes red on that mutant)
# par-attachments-chip/chip-names-the-file | (control: no mutant removes the chip; the reader's tile stays)
# par-attachments-chip/pdf-bytes-land | no-source: the fixture source is not chosen (driven by WHOOSH_ATTACHMENT_FIXTURES=/nonexistent)
# par-attachments-chip/pdf-has-real-thumbnail | no-thumbnail: the thumbnail is never generated
# par-attachments-chip/tooltip-shows-keys | no-keys: the tooltip names the file only
# par-attachments-chip/click-selects-the-chip | (control: a click that selects nothing; covered by every later key check)
# par-attachments-chip/selected-chip-shows-its-keys | no-keys: the key caps stay hidden
# par-attachments-chip/keys-reveal-within-hover-reveal | (reads the duration the view uses; a duration change is a code diff)
# par-attachments-quicklook/space-opens-the-panel | no-panel: Space answers but the panel never opens
# par-attachments-quicklook/panel-shows-the-chip-file | no-panel
# par-attachments-quicklook/panel-is-wired-to-the-chip | no-panel
# par-attachments-quicklook/panel-zooms-from-the-chip | no-panel
# par-attachments-quicklook/space-closes-the-panel | (control: reached only when the panel opened)
# par-attachments-quicklook/escape-closes-the-panel | (control: reached only when the panel opened)
# par-attachments-open/double-click-opens | no-open: open answers and does nothing
# par-attachments-open/return-opens | no-open
# par-attachments-open/command-o-opens | no-open
# par-attachments-open/escape-lets-go-of-the-chip | (control)
# par-attachments-open/command-o-opens-the-first-attachment-unselected | no-open
# par-attachments-menu/menu-opens-at-the-chip | (control: the menu's own delegate counts the open)
# par-attachments-menu/menu-items-in-finder-order | menu-without-share: the menu lacks Share
# par-attachments-menu/menu-keys-shown | menu-without-copy: the menu lacks Copy
# par-attachments-menu/open-with-lists-the-default-first | menu-without-open-with
# par-attachments-menu/share-is-the-system-share-item | menu-without-share
# par-attachments-menu/show-in-finder-waits-for-a-save | (control)
# par-attachments-menu/menu-closes | (control: a socket-opened menu closes itself)
# par-attachments-menu/menu-open-opens | no-open
# par-attachments-menu/menu-open-with-opens-with-the-app | menu-without-open-with
# par-attachments-menu/menu-quick-look-opens-the-panel | no-panel
# par-attachments-menu/save-to-downloads-writes-the-file | menu-without-save-to-downloads
# par-attachments-menu/show-in-finder-reveals-the-saved-file | menu-without-show-in-finder
# par-attachments-menu/save-as-opens-the-save-panel | menu-without-save-as
# par-attachments-menu/save-as-saves-where-the-panel-says | menu-without-save-as
# par-attachments-menu/copy-puts-the-file-on-the-pasteboard | menu-without-copy
# par-attachments-menu/share-hands-the-file-to-the-share-sheet | menu-without-share
# par-attachments-menu/drag-carries-the-file | no-drag: the chip carries nothing out
# par-attachments-menu/drag-offers-a-file-url | no-drag
# par-attachments-step/three-photo-chips | (control)
# par-attachments-step/photos-have-real-thumbnails | no-thumbnail
# par-attachments-step/panel-opens-on-the-first-photo | no-panel
# par-attachments-step/right-steps-to-the-next-photo | no-step: arrows answer and stay on the photo
# par-attachments-step/left-steps-back | no-step-back: Left answers and stays on the photo
# par-attachments-step/panel-follows-the-selection | no-follow: the panel stays on the old chip
# par-attachments-download/third-photo-is-downloading | (control: the held download)
# par-attachments-download/quick-look-waits-for-the-bytes | no-wait: Quick Look opens an empty item at once
# par-attachments-download/progress-ring-moves | (control)
# par-attachments-download/quick-look-opens-when-the-bytes-land | no-panel
# par-attachments-download/landed-photo-gets-its-thumbnail | no-thumbnail
# par-attachments-open-waits/sheet-is-downloading | (control)
# par-attachments-open-waits/docx-draws-its-document-icon | (control)
# par-attachments-open-waits/drag-before-download-is-a-file-promise | no-drag
# par-attachments-open-waits/open-waits-for-the-bytes | no-open
# par-attachments-open-waits/open-runs-when-the-bytes-land | no-open
# par-attachments-open-waits/opened-file-is-the-sheet | no-open
# par-attachments-composer/dropped-file-is-pending | no-intake: drops and pastes are ignored
# par-attachments-composer/video-has-real-thumbnail | no-thumbnail
# par-attachments-composer/text-paste-is-left-to-the-editor | (control)
# par-attachments-composer/pasted-image-goes-into-the-text | picture-as-tile
# par-attachments-composer/pasted-file-is-pending | no-intake
# par-attachments-composer/pending-file-quick-looks | no-panel
# par-attachments-composer/pending-file-opens | no-open
# par-attachments-composer/pending-file-shows-in-finder | menu-without-show-in-finder
# par-attachments-composer/menu-remove-removes-the-pending-file | menu-without-remove
# par-attachments-shots/no-shot-is-blank | WHOOSH_SHOT_RENDER-free: a one-colour picture (the wrong base window) fails it
# par-attachments-shots/six-shots-written | (control: the capture set)
# par-attachments-header/header-is-one-line-960-sidebar-light | wrap-label: main's header, labels free to wrap
# par-attachments-header/no-label-narrower-than-its-text-960-sidebar-light | wrap-label: main's header, labels free to wrap
# par-attachments-header/header-is-one-line-960-sidebar-dark | wrap-label: main's header, labels free to wrap
# par-attachments-header/no-label-narrower-than-its-text-960-sidebar-dark | wrap-label: main's header, labels free to wrap
# par-attachments-header/header-is-one-line-960-no-sidebar-light | (control: main's header fits at this width, so no wrap to provoke; the rule is the 960-sidebar check's)
# par-attachments-header/no-label-narrower-than-its-text-960-no-sidebar-light | (control: main's header fits at this width, so no wrap to provoke; the rule is the 960-sidebar check's)
# par-attachments-header/header-is-one-line-960-no-sidebar-dark | (control: main's header fits at this width, so no wrap to provoke; the rule is the 960-sidebar check's)
# par-attachments-header/no-label-narrower-than-its-text-960-no-sidebar-dark | (control: main's header fits at this width, so no wrap to provoke; the rule is the 960-sidebar check's)
# par-attachments-header/header-is-one-line-1280-light | (control: main's header fits at this width, so no wrap to provoke; the rule is the 960-sidebar check's)
# par-attachments-header/no-label-narrower-than-its-text-1280-light | (control: main's header fits at this width, so no wrap to provoke; the rule is the 960-sidebar check's)
# par-attachments-header/header-is-one-line-1280-dark | (control: main's header fits at this width, so no wrap to provoke; the rule is the 960-sidebar check's)
# par-attachments-header/no-label-narrower-than-its-text-1280-dark | (control: main's header fits at this width, so no wrap to provoke; the rule is the 960-sidebar check's)
# par-attachments-header/header-shots-written | (control: the r3 capture set)
# par-attachments-gallery/three-photos-are-three-tiles | (control: the corpus's photo mail)
# par-attachments-gallery/tiles-carry-real-pictures | no-thumbnail: no tile gets its picture
# par-attachments-gallery/tile-side-is-the-ledger-s | (reads the side the view uses)
# par-attachments-gallery-30/media-and-documents-split | (control)
# par-attachments-gallery-30/collapsed-gallery-is-one-full-row | row-overflow: the collapsed gallery holds one cell more than a row
# par-attachments-gallery-30/saved-chips-keep-kind-and-size-and-show-a-check | notice-subtitle: the chip's second line becomes the sentence
# par-attachments-gallery-30/gallery-collapses-past-the-cap | no-collapse: every tile shows, no "+N more"
# par-attachments-gallery-30/documents-collapse-past-the-cap | no-collapse
# par-attachments-gallery-30/header-names-the-count | (control)
# par-attachments-gallery-30/video-shows-its-first-frame-and-length | no-first-frame: the video gets no frame of its own
# par-attachments-gallery-30/more-opens-every-tile | no-collapse
# par-attachments-gallery-30/more-opens-every-document | no-collapse
# par-attachments-gallery-30/quick-look-over-the-whole-set | no-panel
# par-attachments-gallery-30/save-all-saves-every-file | no-source: nothing to save
# par-attachments-gallery-30/a-tile-quick-looks-like-a-chip | no-panel
# par-attachments-inline/inline-cid-image-counted-once | (control: the cid names an attached file)
# par-attachments-inline/inline-data-image-joins-the-gallery | no-inline: inline images are not read from the body
# par-attachments-inline/inline-tiles-get-pictures | no-inline
# par-attachments-doors-remove/remove-button-removes-the-pending-file | remove-button-dead: the xmark on a pending file does nothing (app-side too: WHOOSH_ATTACH_MUTANT)
# par-attachments-doors-keys/command-c-copies-the-selected-file | selftest model: no-copy-key
# par-attachments-doors-keys/right-moves-the-selection-without-quick-look | selftest model: arrow-dead
# par-attachments-doors-keys/left-moves-it-back | selftest model: arrow-dead
# par-attachments-doors-hover/hover-shows-the-chip-keys | selftest model: hover-dead
# par-attachments-doors-hover/hover-away-hides-them | selftest model: hover-sticks
# par-attachments-doors-tile/the-photos-are-tiles-of-the-gallery | selftest model: not-a-gallery
# par-attachments-doors-tile/tile-double-click-opens-the-file | selftest model: tile-double-dead
# par-attachments-doors-tile/tile-menu-lists-the-items-in-finder-order | selftest model: tile-menu-short
# par-attachments-doors-tile/tile-drag-carries-the-file | selftest model: tile-drag-dead
# par-attachments-doors-openwith/open-with-the-default-app-door | selftest model: open-with-dead
# par-attachments-doors-openwith/open-with-the-second-app-opens-with-it | selftest model: open-with-first-only
# par-attachments-doors-saveas/save-as-opens-the-panel-for-the-cancel | selftest model: save-as-dead
# par-attachments-doors-saveas/save-as-cancel-closes-the-panel-and-saves-nothing | selftest model: cancel-saves
# par-attachments-absent-space/absent-file-is-named-absent-and-the-strip-prints-no-size | size-when-absent: the strip prints "0 bytes" for a file that is not here (app-side mutant too: WHOOSH_ATTACH_MUTANT)
# par-attachments-absent-space/space-on-an-absent-file-starts-the-fetch-and-waits | selftest model: space-absent-dead (the A123 defect)
# par-attachments-absent-space/the-tile-shows-the-progress | selftest model: no-progress
# par-attachments-absent-space/the-ring-and-the-key-caps-do-not-share-the-chips-end | the key caps are drawn over the fetch ring at the chip's right end (Swift: the caps overlay is not skipped while fetching)
# par-attachments-absent-space/the-chip-keeps-one-width-from-absent-to-present | the chip's width differs between absent, fetching and present (Swift: the caps' room is reserved only when they are drawn)
# par-attachments-absent-space/the-name-is-cut-only-when-the-chip-is-full | the file name is cut in a chip narrower than the block's width (Swift: the lone chip is a half-width column and does not hug)
# par-attachments-absent-space/the-preview-opens-when-the-bytes-land | selftest model: preview-never
# par-attachments-absent-space/the-strip-names-no-absent-file-once-it-is-here | selftest model: strip-stale
# par-attachments-absent-space/space-closes-the-preview | selftest model: space-no-close
# par-attachments-absent-size/size-is-english-in-an-english-app | selftest model: size-system-language
# par-attachments-absent-size/size-is-french-in-a-french-app | selftest model: size-system-language
# par-attachments-absent-size/a-set-with-two-files-missing-counts-the-one-here | selftest model: partial-size
# par-attachments-absent-button/the-button-starts-the-fetch-and-waits | selftest model: button-absent-dead
# par-attachments-absent-button/the-button-opens-the-preview-when-the-bytes-land | selftest model: preview-never
# par-attachments-absent-button/escape-closes-it | selftest model: escape-no-close
# par-attachments-absent-saveall/save-all-fetches-the-absent-file | selftest model: saveall-absent-dead
# par-attachments-absent-saveall/save-all-saves-it-once-when-the-bytes-land | selftest model: saveall-twice (and saveall-never-saves)
# par-attachments-absent-saveall/the-saved-file-is-the-attachment | selftest model: saveall-never-saves
# par-attachments-absent-failed/a-failed-fetch-is-failed-with-its-reason | selftest model: fail-silent
# par-attachments-absent-failed/the-chip-says-why-in-one-warn-line | selftest model: fail-silent
# par-attachments-absent-failed/space-asks-again-after-a-failure | selftest model: no-retry
# par-attachments-absent-failed/the-retry-opens-the-preview-and-clears-the-line | selftest model: line-sticks
# par-attachments-absent-leaves-nothing/the-photos-are-present-tiles-again | selftest model: present-no-reset (the door does not give the fixture back)
# par-attachments-absent-leaves-nothing/save-all-starts-no-second-fetch | selftest model: double-fetch
# par-attachments-chip-absent/an-absent-image-draws-the-chip-not-the-tile | absent-tile: an image that is not here draws the big tile (app-side too: WHOOSH_ATTACH_MUTANT); model: tile-when-absent
# par-attachments-chip-absent/the-absent-chip-has-a-cloud-and-says-not-on-this-mac-yet | selftest model: no-cloud
# par-attachments-chip-absent/the-cap-is-not-shown-on-a-hovered-absent-chip | cap-when-hovered (app-side too); model: cap-when-hovered
# par-attachments-chip-absent/the-cap-shows-only-on-the-selected-chip | selftest model: no-cap
# par-attachments-chip-fetching/fetching-without-a-figure-says-fetching-alone | selftest model: zero-percent
# par-attachments-chip-fetching/fetching-with-a-figure-says-it | selftest model: no-figure-ever
# par-attachments-chip-arrives/the-arrived-image-is-a-picture-tile | selftest model: stays-chip
# par-attachments-chip-arrives/the-strip-grows-by-at-most-one-tile-at-arrival | selftest model: strip-jumps
# par-attachments-sweep/tile-menu-open-opens-the-file | selftest model: tile-menu-open-dead
# par-attachments-sweep/tile-menu-open-with-opens-with-the-default-app | selftest model: tile-menu-open-with-dead
# par-attachments-sweep/tile-menu-quick-look-shows-the-photo-without-a-window-on-a-hidden-copy | selftest model: tile-menu-quick-look-dead
# par-attachments-sweep/tile-menu-save-to-downloads-writes-the-file | selftest model: tile-menu-save-to-downloads-dead
# par-attachments-sweep/tile-menu-show-in-finder-reveals-the-saved-file | selftest model: tile-menu-show-in-finder-dead
# par-attachments-sweep/tile-menu-save-as-opens-the-panel-for-the-cancel | selftest model: tile-save-as-dead
# par-attachments-sweep/tile-menu-save-as-cancel-closes-the-panel-and-saves-nothing | selftest model: tile-cancel-saves
# par-attachments-sweep/tile-menu-copy-puts-the-file-on-the-pasteboard | selftest model: tile-menu-copy-dead
# par-attachments-sweep/tile-menu-share-hands-the-file-to-the-share-sheet | selftest model: tile-menu-share-dead
# par-attachments-sweep/chip-hover-shows-its-keys | selftest model: chip-hover-dead
# par-attachments-sweep/chip-hover-away-hides-its-keys | selftest model: chip-hover-sticks
# par-attachments-sweep/open-with-every-app-of-the-submenu-opens-with-it | selftest model: open-with-third-dead
# par-attachments-sweep/composer-tile-hover-shows-its-keys | selftest model: composer-hover-dead
# par-attachments-sweep/composer-tile-click-selects-it | selftest model: composer-click-dead
# par-attachments-sweep/composer-tile-menu-lists-the-items-with-remove-last | selftest model: composer-menu-no-remove
# par-attachments-sweep/composer-menu-open-opens-the-file | selftest model: composer-menu-open-dead
# par-attachments-sweep/composer-menu-open-with-opens-with-the-app | selftest model: composer-menu-open-with-dead
# par-attachments-sweep/composer-menu-quick-look-shows-the-file-without-a-window-on-a-hidden-copy | selftest model: composer-menu-quick-look-dead
# par-attachments-sweep/composer-menu-save-to-downloads-writes-the-file | selftest model: composer-menu-save-to-downloads-dead
# par-attachments-sweep/composer-menu-save-as-opens-the-panel-for-the-cancel | selftest model: composer-menu-save-as-dead
# par-attachments-sweep/composer-menu-save-as-cancel-closes-the-panel-and-saves-nothing | selftest model: composer-cancel-saves
# par-attachments-sweep/composer-menu-copy-puts-the-file-on-the-pasteboard | selftest model: composer-menu-copy-dead
# par-attachments-sweep/composer-menu-share-hands-the-file-to-the-share-sheet | selftest model: composer-menu-share-dead
# par-attachments-sweep/composer-tile-drag-carries-the-file | selftest model: composer-drag-dead
# par-attachments-sweep/composer-menu-remove-leaves-the-composer-as-it-was | selftest model: composer-menu-remove-dead
"""

MUTANT_NAMES = ["no-panel", "no-follow", "no-step", "no-step-back", "no-collapse", "no-first-frame", "no-inline", "row-overflow", "notice-subtitle", "wrap-label", "no-open", "no-thumbnail", "no-drag", "no-wait", "no-intake", "no-keys",
                "menu-without-share", "menu-without-copy", "menu-without-open-with", "menu-without-save-to-downloads",
                "menu-without-save-as", "menu-without-show-in-finder", "menu-without-remove", "remove-button-dead", "size-when-absent", "absent-tile", "cap-when-hovered"]


def run_mutants(bundle):
    """One launch per mutant; records which checks went red on each, into _captures/attachments-mac/mutants.md."""
    os.makedirs(CAP, exist_ok=True)
    lines = ["| mutant | exit | red checks |", "|---|---|---|"]
    names = MUTANT_NAMES + ["no-source"]
    if "--only" in sys.argv:
        names = sys.argv[sys.argv.index("--only") + 1].split(",")
    for name in names:
        kv = "WHOOSH_ATTACHMENT_FIXTURES=/nonexistent" if name == "no-source" else "WHOOSH_ATTACH_MUTANT=%s" % name
        # every launch is the hidden rig (owner 16:30: no test window, ever); launch-one refuses any other
        fx = os.environ.get("WHOOSH_ATTACHMENT_FIXTURES")             # an unstamped bundle finds the fixtures through this
        if fx and name != "no-source":
            kv += " WHOOSH_ATTACHMENT_FIXTURES=" + fx
        env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1 " + kv)
        cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--fixture-core", "sh", "-c",
               'WHOOSH_SOCKET_DIR="$D" python3 "%s" --no-shots' % os.path.abspath(__file__)]
        p = subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True, text=True)
        red = sorted({l.split(" ", 2)[1].rstrip(":") for l in p.stderr.splitlines() if l.startswith("FAIL ")})
        # exit 75 is the rig lock held by another lane: the mutant never ran, which is neither red nor a survivor
        verdict = "NOT RUN (rig busy, exit 75)" if p.returncode == 75 else (", ".join(red) or "NONE (mutant survived)")
        lines.append("| %s | %d | %s |" % (name, p.returncode, verdict))
        print(lines[-1])
    out = "mutants.md" if "--only" not in sys.argv else "mutants-rerun.md"
    with open(os.path.join(CAP, out), "w", encoding="utf-8") as f:
        f.write("# par-attachments mutants\n\n" + "\n".join(lines) + "\n")
    survived = [l for l in lines if "survived" in l]
    unrun = [l for l in lines if "NOT RUN" in l]
    print("%d mutants, %d survived, %d not run" % (len(names), len(survived), len(unrun)))
    return 1 if survived or unrun else 0


def main():
    if "--selftest" in sys.argv:
        return selftest()
    if "--mutants" in sys.argv:
        return run_mutants(sys.argv[sys.argv.index("--mutants") + 1])
    if "--no-shots" in sys.argv:
        globals()["shots_round"] = lambda j: None
    path = os.path.abspath(__file__)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    rows = [l[1:].strip().split(" | ", 1) for l in MUTANTS.splitlines() if l.startswith("# par-attachments") and " | " in l]
    print("MUTANTS (a check is green only after it has been seen red on its mutant)")
    print("| check | mutant |")
    print("|---|---|")
    for k, v in rows:
        print("| %s | %s |" % (k, v))
    print_cases(r)                   # one line per judged case, the passing ones too: `<step>/<case>  pass|fail|unjudged  <the values>`
    for line in r["fail_list"]:
        sys.stderr.write("FAIL %s\n" % line)
    for line in r["no_mutant"]:
        sys.stderr.write("NO MUTANT %s\n" % line)
    for line in r["stray_mutant"]:
        sys.stderr.write("STRAY MUTANT %s\n" % line)
    for line in r["notes"]:
        sys.stderr.write("NOTE %s\n" % line)
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    stray_ok = "--no-shots" in sys.argv                              # the shots step is skipped on mutant runs
    ok = r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and (stray_ok or not r["stray_mutant"])
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
