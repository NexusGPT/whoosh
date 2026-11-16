#!/usr/bin/env python3
"""Message body check: fn-messagebody (J1.6). Rows come from msg-04 (2 trackers blocked) and the open thread's last message.

Drives the RUNNING rig copy over the control socket (docs/CONTROL-SOCKET.md) through journeys/lib; expected
values come from docs/design/fixtures/mac-main-sample.json and docs/CONSTANTS.md, never from a built screen.
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/messagebody-check.py'
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0 (a check that could not run is not green).
Self-test: `python3 checks/messagebody-check.py --selftest` runs the journey against a stand-in socket (ModelCtl), once clean (all green)
and once per MUTANT below (the check named must go red on that case).
Controls of the body view (each clicked once below): "Show / Hide quoted text" (`mac-main.reader.body.show-quoted`), the "N images blocked"
button (`mac-main.reader.trackers.button`). Links inside the page and the blocked images are drawn by the web view: a link click opens the owner's
browser (a system window), so no case clicks one; an image is blocked and is not a control (see docs/lanes/doors-search.md).
"""
import os
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, Journey, print_cases            # noqa: E402
from lib.ctl import Ctl, Unreadable                                  # noqa: E402
from lib.fixture import FX, ROWS, ACCOUNT, MSG, const               # noqa: E402

ID = "messagebody"
TITLE = "Message body rendering"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "the open message shows its text, older text folded, trackers blocked as a count"
LIST = "mac-main.list"


def ids(s):
    return [r["id"] for r in s.ctl.rows(LIST)]


def accepted(s, *args):
    """True when the app answers ok; ok:false is False so the check after it turns red (dry run: not judged)."""
    ctl = s.ctl
    try:
        ctl.send(*args)
        return True
    except Unreadable:
        return False


def drive(j):
    t = FX["open_thread"]["messages"][-1]
    s = j.step("J1.6", "Body")
    # A129 part 1: the mail is whole the moment it is selected (the list holds the keys, no Enter): never the short preview.
    body_at_rest = lambda: s.ctl.rows("mac-main.reader.body")[0]["paragraphs"]

    def first_body():       # the copy has just started: its first body is built after the first frame (BodyWebGate). The case below reads the body once
        try:                # one is on screen (main148 under load 21: the read came first and the case was not judged, "no message body is on screen")
            return bool(s.ctl.rows("mac-main.reader.body"))
        except Unreadable:
            return False
    s._act(lambda: until(first_body, True, tries=50))
    s.expect("selection-shows-the-whole-body", body_at_rest, t["body"])
    s.expect("selection-body-is-longer-than-the-preview", lambda: len(" ".join(body_at_rest())) > len(ROWS[0]["preview"]), True)
    # A129 B3 and F2 on the selected mail (msg-01: a written mail with a folded earlier message): no sheet, the dots and their key cap, no pictures line.
    body_row = lambda: s.ctl.rows("mac-main.reader.body")[0]
    s.expect("written-mail-has-no-sheet", lambda: (body_row()["layout"], body_row()["sheet"]), ("written", False))
    s.expect("the-dots-pill-shows-its-key", lambda: (body_row()["dots"], body_row()["dots_key"]), (True, "O"))
    s.expect("layout-comes-from-the-engine", lambda: body_row()["layout_source"], "engine")
    s.expect("fold-follows-the-engines-segments", lambda: body_row()["fold_source"], "segments")
    s.expect("no-blocked-no-line", lambda: s.ctl.rows("mac-main.reader.pictures"), [])
    s.key("Enter")
    s.expect("paragraphs-in-order", lambda: s.ctl.rows("mac-main.reader.body")[0]["paragraphs"], t["body"])
    s.expect("no-raw-markup", lambda: any("<" in p for p in s.ctl.rows("mac-main.reader.body")[0]["paragraphs"]), False)
    s.expect("paragraph-probe-control", lambda: len(s.ctl.rows("mac-main.reader.body")[0]["paragraphs"]) > 0, True)
    s.expect("older-quoted-text-folded", lambda: s.ctl.rows("mac-main.reader.body")[0]["quoted_folded"], True)
    quoted = MSG["msg-01"]["quoted"]
    s.expect("thread-has-a-quote", lambda: s.ctl.rows("mac-main.reader.body")[0]["has_quote"], True)
    s.expect("no-trackers-no-button", lambda: accepted(s, "click", "mac-main.reader.trackers.button"), False)   # msg-01 blocked nothing: no button, the door refuses
    s.click("mac-main.reader.body.show-quoted")
    s.expect("show-quoted-click-unfolds", lambda: (s.ctl.rows("mac-main.reader.body")[0]["paragraphs"], s.ctl.rows("mac-main.reader.body")[0]["quoted_folded"]), (t["body"] + quoted, False))
    s.click("mac-main.reader.body.show-quoted")
    s.expect("show-quoted-click-again-folds", lambda: (s.ctl.rows("mac-main.reader.body")[0]["paragraphs"], s.ctl.rows("mac-main.reader.body")[0]["quoted_folded"]), (t["body"], True))
    s.key("Escape")   # A102: Enter in the list opens the reader AND the quick reply, so the first Escape closes the reply field
    s.expect("first-escape-closes-the-reply-field", lambda: (s.ctl.uistate()["surface"], len(s.ctl.rows("mac-main.quickreply.field"))), ("mac-reader", 0))
    # F2 with the keys: the reply field is closed, the reader holds them. O opens everything and the text above stays where it is.
    box = {}
    page_measured(s)                                      # the first read of `text_above_y` is the page's answer to a measure: read the page first (main81: 0 before O, 220 after)
    s._act(lambda: box.update(y=body_row()["text_above_y"]))
    s.key("O")
    page_measured(s)
    s.expect("dots-key-opens-everything", lambda: (body_row()["fold_open"], body_row()["paragraphs"]), (True, t["body"] + quoted))
    # Lead, 13 Nov (pictures of the opened history on main148, light and dark): one quote level drew TWO parallel rules, the folded part's and the quote's own.
    # Read from the page itself: the rules left of the first quote of the folded part (a test copy's page reports it; the web row asks the page to measure again).
    s._act(lambda: s.ctl.rows("mac-main.reader.web"))
    s.expect("an-opened-quote-draws-one-rule", lambda: until(lambda: (body_row().get("drawn") or {}).get("quote_bars"), "1", tries=20), "1")
    def text_above():
        now = body_row()["text_above_y"]
        s._eg = (box["y"], now)
        return abs(now - box["y"]) <= 1, "text_above_y was %r before O and is %r after it (the rule: within 1 point), fold_open %r" % (box["y"], now, body_row()["fold_open"])
    s._record("text-above-does-not-move", text_above)
    s.key("O")
    s.expect("dots-key-again-folds", lambda: (body_row()["fold_open"], body_row()["paragraphs"]), (False, t["body"]))
    s.click("mac-main.reader.body.dots")
    s.expect("dots-click-opens-everything", lambda: body_row()["fold_open"], True)
    s.click("mac-main.reader.body.dots")
    s.expect("dots-click-again-folds", lambda: body_row()["fold_open"], False)
    s.key("Escape")
    s.expect("second-escape-returns-to-list", lambda: s.ctl.uistate()["surface"], "mac-main")
    for _ in range(ROWS.index(MSG["msg-04"])):   # msg-04's place in the served list (the rejected flow row stands before it), not a fixed count
        s.key("J")
    s.key("Enter")
    s.expect("second-thread-is-msg-04", lambda: s.ctl.rows("mac-main.reader.subject")[0]["subject"], MSG["msg-04"]["subject"])
    s.expect("trackers-blocked-count", lambda: s.ctl.rows("mac-main.reader.trackers")[0]["count"], MSG["msg-04"]["trackers_blocked"])
    s.expect("tracker-count-not-zero", lambda: s.ctl.rows("mac-main.reader.trackers")[0]["count"] > 0, True)
    s.expect("no-quote-no-show-quoted-button", lambda: (s.ctl.rows("mac-main.reader.body")[0]["has_quote"], accepted(s, "click", "mac-main.reader.body.show-quoted")), (False, False))
    s.expect("note-starts-closed", lambda: s.ctl.rows("mac-main.reader.trackers")[0]["explain_open"], False)
    s.click("mac-main.reader.trackers.button")
    s.expect("trackers-button-click-opens-the-note", lambda: s.ctl.rows("mac-main.reader.trackers")[0]["explain_open"], True)
    s.click("mac-main.reader.trackers.button")
    s.expect("trackers-button-click-again-closes-the-note", lambda: s.ctl.rows("mac-main.reader.trackers")[0]["explain_open"], False)
    drive_mail(j)
    drive_engine(j)
    drive_wide(j)
    drive_on_open(j)
    drive_local_picture(j)
    drive_open_height(j)


SENDER = MSG["msg-04"]["from"]
# main79 (the note the J1.6b cases cite): msg-04 blocks two tracking pixels and no picture (docs/design/fixtures/mac-main-sample.json: `trackers_blocked` 2, a body with no image; the
# fixture core serves picturesBlocked 0 for every list mail), so the pictures line, Load once and its key cap are asked of the one fixture mail that
# has blocked pictures: `sample_mails.designed` (3 pictures and 2 trackers), shown by the drive seam `mac-main.reader.body.sample.designed`.
# The three mails of the drawings (Sources/Features/MessageBody/MessageBodySamples.swift): the same numbers, written down here.
DESIGNED_PICTURES, DESIGNED_TRACKERS = 3, 2
DESIGNED_ASKABLE = 1               # of its three blocked images, one is a picture with a kept address (the banner); the two others are 1 x 1 pixels, never asked for
FETCHABLE_ON_A_TEST_COPY = 0      # pictures the page asks a server off this Mac for after Load once, on a test copy (main81 read 0; the rig refuses every such address)
WRITTEN_QUOTES = 2
PLAIN_BLOCKS, PLAIN_QUOTES, PLAIN_LINKS = 6, 1, 2


def page_measured(s):
    """Ask the page to measure and wait for its answer (`mac-main.reader.web`: the first read asks again, the second reads the answer, as J1.6c does).
    `text_above_y` is the page's own measure (MessageBodyView.measure): before the page has answered it is 0, which is not where the text is."""
    web = lambda: s.ctl.rows("mac-main.reader.web")[0]
    s._act(web)
    pause(s, 400)
    s._act(web)


def reader_holds_the_keys(s):
    """The reader, not the list, holds the keys (O and the colour switch act only on the open reader: MessageBodyFeature.readerHoldsKeys, surface `mac-reader`).
    After J1.6c the search is closed and the surface is the list (main81: surface mac-main), so Return opens the reader; its reply field takes the first Escape.
    Reads the surface first; presses nothing when the reader already holds the keys."""
    def state():
        u = s.ctl.uistate()
        return u.get("surface"), u.get("first_responder")
    def to_reader():
        surface, holder = state()
        if surface != "mac-reader":
            s.ctl.send("key", "Enter")
            end = time.time() + 3.0
            while time.time() < end and state()[0] != "mac-reader":
                time.sleep(0.1)
            # the reply the Return opened takes the keyboard a moment after the reader is up: read the holder once it has (main147, run 1 of 4: the read
            # came first, no Escape was pressed, and the case then found the reply holding the keys)
            def reply_open():
                try:
                    return bool(s.ctl.rows("mac-main.quickreply.field"))
                except Exception:                                    # noqa: BLE001  a build with no such row: nothing to wait for
                    return False
            end = time.time() + 1.5
            while time.time() < end and reply_open() and state()[1] != "quick-reply":
                time.sleep(0.05)
            surface, holder = state()
        # The reply's Escape closes one layer per press (QuickReplyFeature "Escape": the From picker, then the AI draft or its writing,
        # then the field: closeFrom || aiEscape || close). Since 865273409 `first_responder` tells the reply editor apart (main92: one
        # Escape left 'quick-reply', the AI draft the open made took it), so Escape is pressed again while the reply still holds the keys, at most 3.
        for _ in range(3):
            if not (surface == "mac-reader" and holder == "quick-reply"):
                break
            s.ctl.send("key", "Escape")
            end = time.time() + 1.0
            while time.time() < end and state()[1] == "quick-reply":
                time.sleep(0.1)
            surface, holder = state()
    def holds():
        got = (s.ctl.uistate().get("surface"), s.ctl.uistate().get("first_responder") != "quick-reply")
        if got == ("mac-reader", True):
            return got
        # main92 and main93: three Escapes left the reply holding the keys. A miss names what the reply says of itself (its phase, text,
        # AI state) and whether the reader shows a search result (the reader's own Escape goes back to the results), so the next log
        # tells "Escape went to another feature" from "the reply's Escape did not close it".
        def rows(ax):
            try:
                return s.ctl.rows(ax)
            except Exception as e:                                   # noqa: BLE001  evidence only
                return "unread (%s)" % e
        field = rows("mac-main.quickreply.field")
        return {"read": got, "reply field": [{k: r.get(k) for k in ("phase", "text", "emoji_open", "thread")} for r in field] if isinstance(field, list) else field,
                "reply ai": rows("mac-main.quickreply.ai"), "reader state": rows("mac-main.reader.state")}
    s._act(to_reader)
    s.expect("reader-holds-the-keys", holds, ("mac-reader", True))


def pause(s, ms):
    """Wait for the app's own page script (a measure after a flip); the stand-in has none."""
    if s.j.ctl is not None and not getattr(s.j.ctl, "instant", False):
        s._act(lambda: time.sleep(ms / 1000.0))


MONITOR_FIELDS = ("window_id", "focus", "handled", "windows", "text_before", "text_after", "window", "key_before", "key_after", "route")


def read_press(ctl, chord):
    """Right after a key: the key verb's own report (`rows mac-keyevent.monitor`, field `last`) and the shell's focus (`uistate`, `rows mac-main.shell`
    focused_pane, which the colour switch asks: MessageBodyFeature.swift:105). A field an older build does not serve reads "absent"; nothing raises."""
    try:
        r = ctl.rows("mac-keyevent.monitor")
        last = r[0].get("last") if r and isinstance(r[0], dict) else None
    except (Unreadable, KeyError, IndexError, TypeError, AttributeError, ValueError):
        last = None
    last = last if isinstance(last, dict) else {}
    said = " ".join("%s=%r" % (f, last.get(f, "absent")) for f in MONITOR_FIELDS) if last else "report absent"
    try:
        u = ctl.uistate()
        shell = "surface=%r focus=%r overlay=%r" % (u.get("surface", "absent"), u.get("first_responder", "absent"), u.get("overlay", "absent"))
    except (Unreadable, KeyError, TypeError, AttributeError, ValueError) as e:
        shell = "unreadable (%s)" % e
    try:
        pane = repr(ctl.rows("mac-main.shell")[0].get("focused_pane", "absent"))
    except (Unreadable, KeyError, IndexError, TypeError, AttributeError, ValueError) as e:
        pane = "unreadable (%s)" % e
    return "app said: key %r, keyevent %s; shell %s focused pane %s" % (chord, said, shell, pane)


def expect_pressed(s, name, thunk, want, pressed):
    """`Step.expect`; a failure also carries what the app said right after the last ⇧I (`pressed`, written by `press`)."""
    def run():
        got = thunk()
        s._eg = (want, got)
        return got == want, "expected %r, got %r; %s" % (want, got, pressed.get("said", "no key was pressed"))
    s._record(name, run)


def drive_mail(j):
    """A129: the pictures line (Load once, Always), B3 (the sheet, ⇧I), a written mail's inline quotes, plain text as paragraphs."""
    s = j.step("J1.6b", "Mail view: pictures line, sheet, colours, quotes, plain text")
    body = lambda: s.ctl.rows("mac-main.reader.body")[0]
    line = lambda: s.ctl.rows("mac-main.reader.pictures")[0]
    pressed = {}

    def press(chord):
        s.key(chord)
        s._act(lambda: pressed.update(said=read_press(s.j.ctl, chord)))
    s.key("Escape")                  # the reply field the open left behind; the reader holds the keys after it
    s.click("mac-main.reader.body.sample.designed")              # the mail with blocked pictures (see the note above DESIGNED_PICTURES): msg-04 has none, only pixels
    s.expect("pictures-line-says-what-was-blocked", lambda: (line()["pictures"], str(DESIGNED_PICTURES) in line()["line"]), (DESIGNED_PICTURES, True))
    s.expect("load-once-shows-its-key-cap", lambda: (line()["load_once_key"], line()["load_once_shown"], line()["always_shown"]), ("I", True, True))
    s.expect("nothing-loads-before-asked", lambda: (line()["loaded"], line()["list"]), (False, "none"))
    s.click("mac-main.reader.pictures.load-once")
    s.expect("load-once-click-loads-the-pictures-of-this-viewing", lambda: (line()["loaded"], line()["list"]), (True, "pictures"))
    s.expect("load-once-keeps-the-page-script-off", lambda: (body()["javascript_allowed"], line()["javascript_allowed"], line()["remote_blocked"]), (False, False, True))
    # Journey finding 63: after the ask the line says what happened. A test copy asks no server off this Mac, so none of the designed mail's pictures comes: the
    # line says how many could not be loaded and offers the load again (the owner's copy says "N pictures loaded" when they come, and then offers Always).
    missing_line = catalogue("%1$@ of %2$@ pictures could not be loaded").replace("%1$@", str(DESIGNED_PICTURES)).replace("%2$@", str(DESIGNED_PICTURES))
    told = ("missing", DESIGNED_PICTURES, True)
    s.expect("after-the-ask-the-line-says-what-did-not-come", lambda: until(lambda: (line()["state"], line()["missing"], line()["line"].startswith(missing_line)), told), told)
    s.expect("pictures-that-did-not-come-offer-try-again", lambda: (line()["retry_shown"], line()["load_once_shown"], line()["always_shown"]), (True, False, False))
    seen = {}
    s._act(lambda: seen.update(loads=line()["loads"]))
    s.click("mac-main.reader.pictures.retry")
    s.expect("try-again-loads-the-pictures-again", lambda: until(lambda: (line()["loads"] > seen.get("loads", 10 ** 9), line()["state"]), (True, "missing")), (True, "missing"))
    s.click("mac-main.reader.body.sample.designed")              # a fresh viewing: Load once used the line up on the first mail
    s.click("mac-main.reader.pictures.always")
    s.expect("always-click-keeps-the-sender", lambda: (line()["always"], line()["loaded"], line()["sender"]), (True, True, "elise@oranta.example"))
    s.click("mac-main.reader.pictures.always.forget")
    s.expect("always-forget-leaves-nothing", lambda: line()["always"], False)
    s.click("mac-main.reader.body.sample.designed")
    s.expect("load-once-does-not-outlast-the-viewing", lambda: line()["loaded"], False)
    s.expect("designed-mail-has-the-sheet", lambda: (body()["layout"], body()["colours"], body()["sheet"]), ("designed", "original", True))
    s.expect("nothing-is-fetched-before-the-click", lambda: line()["fetchable"], 0)
    s.expect("pictures-line-counts-pictures-and-trackers", lambda: (line()["pictures"], line()["trackers"]), (DESIGNED_PICTURES, DESIGNED_TRACKERS))
    s.expect("colours-action-names-the-other-way", lambda: body()["colours_action"], catalogue("App colours"))
    press("⇧I")
    expect_pressed(s, "shift-i-shows-the-app-colours", lambda: (body()["colours"], body()["sheet"]), ("app", False), pressed)
    expect_pressed(s, "colours-action-follows-the-switch", lambda: body()["colours_action"], catalogue("Original colours"), pressed)
    s.click("mac-main.reader.body.colours")
    expect_pressed(s, "colours-door-shows-the-original-colours-again", lambda: (body()["colours"], body()["sheet"]), ("original", True), pressed)
    s.key("I")
    s.expect("key-i-loads-this-mails-pictures", lambda: (line()["loaded"], line()["remote_blocked"], body()["javascript_allowed"]), (True, True, False))
    s._act(lambda: seen.update(loads=until(lambda: line()["state"], "missing") and line()["loads"]))
    s.key("I")
    s.expect("key-i-again-tries-the-missing-pictures-again", lambda: until(lambda: line()["loads"] > seen.get("loads", 10 ** 9), True), True)
    # A test copy never asks a server off this Mac (RigNet.allows, MessageBodyDocument.placeholderImages): the designed mail's pictures keep an https address of
    # the sender's, so after Load once the page asks for none of them. The owner's copy would ask for one (the pictures, never the pixels); a test copy must ask for none.
    s.expect("load-fetches-the-pictures-and-never-a-pixel", lambda: line()["fetchable"], FETCHABLE_ON_A_TEST_COPY)
    s.click("mac-main.reader.body.sample.written")
    s.expect("written-sample-has-no-sheet", lambda: (body()["layout"], body()["colours"], body()["sheet"]), ("written", "app", False))
    s.expect("written-inline-questions-stay-above-as-quotes", lambda: body()["quote_blocks"], WRITTEN_QUOTES)
    s.expect("written-mail-folds-the-history", lambda: (body()["dots"], body()["fold_open"]), (True, False))
    press("⇧I")
    expect_pressed(s, "shift-i-gives-a-written-mail-its-original-colours", lambda: (body()["colours"], body()["sheet"]), ("original", True), pressed)
    s.click("mac-main.reader.body.sample.plain")
    s.expect("plain-text-reads-as-paragraphs", lambda: (body()["format"], body()["blocks"]), ("plain", PLAIN_BLOCKS))
    s.expect("plain-text-links-are-links", lambda: body()["links"], PLAIN_LINKS)
    s.expect("plain-text-quotes-are-quotes", lambda: body()["quote_blocks"], PLAIN_QUOTES)

    # The pane is rebuilt with the same message (a search opens beside it, ⌘K, text, Return): the body is painted again, no blank where the text is.
    s = j.step("J1.6c", "Mail view: the body is painted after the pane is rebuilt")
    s.key("⌘K")
    s.type("invoice")
    s.key("Enter")
    pause(s, 900)
    s.expect("body-is-painted-after-the-pane-is-rebuilt", lambda: s.ctl.rows("mac-main.reader.body")[0]["painted"], True)
    web = lambda: s.ctl.rows("mac-main.reader.web")[0]
    s._act(web)                                           # the first read asks the page to measure again; the second reads its answer
    pause(s, 300)
    s.expect("painted-means-text-in-the-page", lambda: (web()["painted"] == (web()["text_length"] > 0 and web()["in_window"] and web()["layer_filters"] == 0), web()["text_length"] > 0), (True, True))
    s.expect("the-web-view-is-in-a-window-with-no-filter-above-it", lambda: (web()["in_window"], web()["hidden"], web()["layer_filters"], web()["height"] > 0), (True, False, 0, True))


FOLDED_ABOVE = ["Hi Assem,", "Can you confirm the price?", "Yes: 18,400 EUR."]
LINK_MAX_CHARS = 40                                # docs/CONSTANTS.md BODY_LINK_MAX_CHARS
PAPER_PAD_PX = 16                                 # docs/CONSTANTS.md BODY_PAPER_PAD_PX: the inner margin of a designed mail's page
TILE_FILL = "rgb(240, 240, 243)"                  # the tile of a blocked picture on the sender's page (MessageBodyDocument `html.original .blocked`)
# the invented tracking addresses of the sample (MessageBodyEngineSamples.trackingStory, trackingPrefs, trackingPlain): the full address is what a link opens
TRACKING = ["https://click.news-digest.example/track/L2g5a3Jhd2xlci9yZWRpcmVjdC9hYmNkZWZnaGlqa2xtbm9wcXJzdHV2d3h5ejAxMjM0NTY3ODk/redirect?u=aHR0cHM6Ly93d3cubmV3cy1kaWdlc3QuZXhhbXBsZS9zdG9yeS8xMjM0NQ&c=0a1b2c3d4e5f",
            "https://click.news-digest.example/prefs/MDEyMzQ1Njc4OWFiY2RlZmdoaWprbG1ub3BxcnN0dXZ3eHl6/manage?t=ZGlnZXN0LXdlZWtseQ&sig=9f8e7d6c5b4a3210fedcba9876543210",
            "https://www.tmcnet.example/articles/2026/11/13/the-week-in-print/index.html?utm_source=digest&utm_medium=email&utm_campaign=weekly&id=88231977"]
TRACKING_TEXTS = ["Read the full story", "To adjust your preferences", "tmcnet.example link"]
PLACEHOLDER_TEXT_MIN_PX = 130                      # docs/CONSTANTS.md BODY_BLOCKED_TEXT_MIN_PX
FOLDED_BEHIND = ["Earlier message from Assem", "Claire Martin, Head of Purchasing", "Confidential."]


def drive_engine(j):
    """Contract delta 26: layout, documents, segments and fold_from, the pictures counts and the warnings come from the engine."""
    s = j.step("J1.6d", "Mail view: the engine's prepared fields")
    body = lambda: s.ctl.rows("mac-main.reader.body")[0]
    line = lambda: s.ctl.rows("mac-main.reader.pictures")[0]
    # main79: J1.6c left the search open (⌘K, text, Enter), and while the search surface is shown its field holds the keys, so O and ⇧I below went to
    # the field. Close it the way a person does (Escape) and read the surface before the first key.
    s.key("Escape")
    s.expect("search-is-closed-before-the-keys", lambda: (s.ctl.uistate().get("surface") != "mac-search", s.ctl.uistate().get("first_responder") != "search-field"), (True, True))
    reader_holds_the_keys(s)
    s.click("mac-main.reader.body.sample.written")
    s.expect("unprepared-mail-falls-back-to-written", lambda: (body()["layout"], body()["layout_source"], body()["fold_source"]), ("written", "fallback", "quote_offset"))
    s.click("mac-main.reader.body.sample.trackers")
    s.expect("trackers-only-says-trackers-and-no-pictures", lambda: (line()["pictures"], line()["trackers"]), (0, 2))
    s.expect("trackers-only-offers-no-load-once", lambda: (line()["load_once_shown"], line()["always_shown"], line()["fetchable"]), (False, False, 0))
    s.click("mac-main.reader.body.sample.folded")
    s.expect("fold-follows-fold-from", lambda: (body()["has_quote"], body()["folded_text"]), (True, FOLDED_BEHIND))
    s.expect("inline-quote-and-text-after-it-stay-above-the-dots", lambda: body()["paragraphs"], FOLDED_ABOVE)
    s.key("O")
    s.expect("dots-open-everything-the-engine-folded", lambda: body()["paragraphs"], FOLDED_ABOVE + FOLDED_BEHIND)
    # owner 2026-11-13: a short mail a person wrote, its signature a small layout table with pictures, was drawn on a white page (designed); a newsletter keeps its paper.
    # The decision is the engine's (core/crates/parse/src/display.rs, the corpus case written-with-picture-signature.case); the app draws what it says, and every blocked picture
    # keeps the size it declared: a mark with no words when narrower than the words need, the words only where they fit on one line.
    placeholders = lambda: [(r["mark"], r["width"], r["height"], bool(r["words"])) for r in s.ctl.rows("mac-main.reader.body.placeholders")]
    s.click("mac-main.reader.body.sample.signature")
    s.expect("a-written-mail-with-a-picture-signature-is-written", lambda: (body()["layout"], body()["colours"]), ("written", "app"))
    s.expect("signature-pictures-are-marks-at-their-declared-size", placeholders, [(True, 100, 100, False)] + [(True, 24, 24, False)] * 3 + [(True, 16, 16, False)] * 4)
    s.click("mac-main.reader.body.sample.newsletter")
    s.expect("a-newsletter-keeps-its-paper", lambda: (body()["layout"], body()["colours"]), ("designed", "original"))
    s.expect("a-wide-blocked-picture-keeps-its-words", placeholders, [(False, 600, 200, True), (False, 280, 180, True), (False, 280, 180, True)])
    # Lead, 13 Nov (owner picture 198): what the PAGE computed, read back from the web view (the checks above read the generated text, and the rules of the
    # sender's-colours mode matched nothing for weeks: `html.orig` against the class `original`). The page's class, its inner margin, the tile of a blocked picture.
    def drawn(settled):
        """The page's own facts, read once `settled(facts)` holds, or after 2 s as they are. The page reads them back a moment after a load or
        a colour switch (an asynchronous script), and the samples reuse one page model: right after a sample is opened or the colours are
        switched the row still holds the facts of the page before it (walk q3-134)."""
        import time
        d = {}
        for _ in range(20):
            d = body().get("drawn") or {}
            if settled(d):
                break
            time.sleep(0.1)
        return d
    paper = ("original", "%dpx" % PAPER_PAD_PX, TILE_FILL)
    paper_read = lambda: (lambda d: (d.get("class"), d.get("pad_left"), d.get("tile_fill")))(drawn(lambda d: (d.get("class"), d.get("pad_left"), d.get("tile_fill")) == paper))
    s.expect("the-designed-page-draws-its-own-rules", paper_read, paper)
    # owner 2026-11-13 07:29: a plain text mail showed a tracking address as about ten lines of letters and digits. An address in angle brackets after its own words folds into
    # them; a long web address is one short link naming its site; the full address is what the link opens and its tooltip.
    links = lambda: s.ctl.rows("mac-main.reader.body.links")
    s.click("mac-main.reader.body.sample.trackinglinks")
    s.expect("a-long-address-is-one-short-link", lambda: ([r["text"] for r in links()], all(len(r["text"]) <= LINK_MAX_CHARS and all(len(w) <= LINK_MAX_CHARS for w in r["text"].split()) for r in links())), (TRACKING_TEXTS, True))
    s.expect("a-link-opens-the-full-address", lambda: ([r["href"] for r in links()], [r["title"] for r in links()]), (TRACKING, TRACKING))
    s.click("mac-main.reader.body.sample.degraded")
    s.expect("degraded-mail-says-it-is-plain-text", lambda: body()["degraded"], True)
    s.click("mac-main.reader.body.sample.written")
    s.expect("a-normal-mail-says-nothing-of-it", lambda: body()["degraded"], False)
    s.click("mac-main.reader.body.sample.twodocs")
    s.expect("two-documents-for-two-colours", lambda: (body()["documents"], body()["colours"], body()["drawn_document"]), (2, "app", "written"))
    s.key("⇧I")
    s.expect("the-switch-changes-the-document", lambda: (body()["colours"], body()["drawn_document"]), ("original", "designed"))
    s.expect("only-the-senders-document-shows-in-the-senders-colours",
             lambda: (lambda d: (d.get("app_document"), d.get("sender_document")))(drawn(lambda d: (d.get("app_document"), d.get("sender_document")) == ("none", "block"))), ("none", "block"))
    s.key("⇧I")
    s.expect("the-switch-back-restores-the-written-document", lambda: body()["drawn_document"], "written")


def wide_natural_width():
    """The width the wide fixture mail is laid out at: its table's own `width` attribute, read from the design fixture (docs/design/fixtures/mac-main-sample.json sample_mails.wide)."""
    import re
    m = re.search(r'<table[^>]*\bwidth="(\d+)"', FX["sample_mails"]["wide"]["html"])
    return int(m.group(1)) if m else None


FIT_TOLERANCE = 0.5       # WideMailFit's half point (docs/lanes/wide-mail.md): a mail at most this much wider than the pane is not scaled


def drive_wide(j):
    """A146: a mail wider than the reading pane is scaled to the pane in the page, nothing is cut off at the right, and the fit alone does not scroll sideways.
    The zoomed half (pinch, then sideways scroll) has no door on the socket: it stays a unit test (Tests/Unit/WideMailFitTests.swift)."""
    s = j.step("J1.6e", "Mail view: a wide mail is fitted to the reading pane")
    row = lambda: s.ctl.rows("mac-main.reader.body")[0]
    s.click("mac-main.reader.body.sample.wide")
    page_measured(s)
    natural = wide_natural_width()
    s.expect("wide-fixture-mail-is-wider-than-the-pane", lambda: (natural, row()["view_width"] > 0, natural > row()["view_width"] + FIT_TOLERANCE), (natural, True, True))
    # Lead, 13 Nov: the wide sample is a designed mail on its paper, so its own width is its widest element plus the paper's inner margin on
    # BOTH sides (BODY_PAPER_PAD_PX): fitted to the pane, it touches neither edge of the paper.
    s.expect("wide-mail-is-laid-out-at-its-own-width", lambda: row()["natural_width"], float(natural) + 2 * PAPER_PAD_PX)

    def scaled():
        r = row()
        want = r["view_width"] / r["natural_width"] if r["natural_width"] else None
        s._eg = (want, r["scale"])
        ok = want is not None and r["scale"] < 1 and abs(r["scale"] - want) <= 0.01
        return ok, "natural_width %r, view_width %r: scale %r (the fit %r, the pane over the page)" % (r["natural_width"], r["view_width"], r["scale"], want)
    s._record("wide-mail-is-scaled-to-the-pane", scaled)

    def not_cut():
        r = row()
        s._eg = (r["view_width"] + FIT_TOLERANCE, r["scaled_width"])
        return 0 < r["scaled_width"] <= r["view_width"] + FIT_TOLERANCE, "scaled_width %r against view_width %r (the rule: at most the pane plus %r)" % (r["scaled_width"], r["view_width"], FIT_TOLERANCE)
    s._record("wide-mail-is-not-cut-off-at-the-right", not_cut)

    def no_sideways():
        r = row()
        s._eg = ((False, 1.0), (r["scrolls_horizontally"], r["zoom"]))
        return r["scrolls_horizontally"] is False and r["zoom"] == 1.0, "scrolls_horizontally %r, zoom %r (the fit alone: no sideways scroll, no pinch)" % (r["scrolls_horizontally"], r["zoom"])
    s._record("the-fit-alone-does-not-scroll-sideways", no_sideways)


def drive_on_open(j):
    """Owner 13 Nov, pictures 213 and 214 ("lots of picture not loaded or difficult to load"): web pictures load when a mail opens, unless the owner turned the
    rule off in Settings > General > Privacy or the mail is read in Junk. A test copy starts with the rule off (it may ask no server off this Mac); the drive
    turns it on through the Settings switch's own door and leaves it off. The test copy asks for none of the designed mail's pictures, so no picture is fetched here."""
    s = j.step("J1.6f", "Mail view: pictures load when a mail opens")
    line = lambda: s.ctl.rows("mac-main.reader.pictures")[0]
    rule = lambda: s.ctl.rows("mac-settings-general.settings")[0]["pictures_on_open"]
    switch = "mac-settings-general.privacy.pictures"

    def start_off():      # a run that died with the rule on leaves it on in the test copy's own settings: start from ask first
        if rule():
            s.ctl.click(switch)
    s._act(start_off)
    s.click("mac-main.reader.body.sample.designed")
    s.expect("with-the-rule-off-a-mail-asks-first", lambda: (rule(), line()["loaded"], line()["load_once_shown"]), (False, False, True))
    s.click(switch)
    s.click("mac-main.reader.body.sample.designed")
    s.expect("with-the-rule-on-a-mail-opens-with-its-pictures-asked", lambda: (rule(), line()["loaded"], line()["by_rule"], line()["load_once_shown"]), (True, True, True, False))
    s.expect("the-rule-never-asks-for-a-pixel", lambda: (line()["askable"], line()["trackers"], line()["fetchable"]), (DESIGNED_ASKABLE, DESIGNED_TRACKERS, FETCHABLE_ON_A_TEST_COPY))
    s.expect("the-note-says-pictures-load-on-open", lambda: line()["note"], catalogue("Pictures load when you open a mail. Tracking pixels are never loaded."))
    s.click("mac-main.reader.body.sample.designed-junk")
    s.expect("a-mail-read-in-junk-asks-first", lambda: (line()["quarantined"], line()["loaded"], line()["load_once_shown"]), (True, False, True))
    s.click(switch)
    s.click("mac-main.reader.body.sample.designed")
    s.expect("the-switch-off-asks-first-again", lambda: (rule(), line()["loaded"], line()["load_once_shown"]), (False, False, True))


class LocalPictures:
    """A picture server on this Mac only (127.0.0.1, a free port), for the one case that fetches a real picture: it answers a small PNG, or 404 while `fail`
    is set, and keeps the list of the paths it was asked for. Nothing here leaves this Mac."""
    PNG = bytes.fromhex("89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c4890000000d49444154789c6360f8cfc0f01f0005000201a0a5f5a30000000049454e44ae426082")

    def __init__(self):
        import http.server, threading
        outer = self
        self.asked, self.fail = [], False

        class H(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                outer.asked.append(self.path)
                if outer.fail:
                    self.send_response(404); self.send_header("Content-Length", "0"); self.end_headers(); return
                self.send_response(200); self.send_header("Content-Type", "image/png"); self.send_header("Content-Length", str(len(outer.PNG)))
                self.send_header("Cache-Control", "no-store"); self.end_headers(); self.wfile.write(outer.PNG)

            def log_message(self, *a):
                pass
        self.httpd = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
        self.port = self.httpd.server_address[1]
        threading.Thread(target=self.httpd.serve_forever, daemon=True).start()

    def url(self, path):
        return "http://127.0.0.1:%d%s" % (self.port, path)

    def count(self, path):
        return sum(1 for a in self.asked if a == path)

    def stop(self):
        self.httpd.shutdown(); self.httpd.server_close()


def drive_local_picture(j):
    """The whole load path with a REAL fetch that never leaves this Mac: with the rule on, a mail whose one picture is kept at an address of this check's own
    local server opens with the picture asked for and drawn; the tracking pixel kept beside it is never asked for (the server's own request list says so);
    a picture the server refuses is said with Try again, and Try again brings it once the server answers. (A test copy may ask no server off this Mac, so
    the fixture's designed mail could only ever prove the refusal.)"""
    s = j.step("J1.6g", "Mail view: a picture is fetched from a server on this Mac, a pixel never")
    line = lambda: s.ctl.rows("mac-main.reader.pictures")[0]
    rule = lambda: s.ctl.rows("mac-settings-general.settings")[0]["pictures_on_open"]
    switch = "mac-settings-general.privacy.pictures"
    box = {}

    def start():
        box["srv"] = LocalPictures()
        if hasattr(s.ctl, "local_pictures"):
            s.ctl.local_pictures(box["srv"])           # the stand-in asks the same server, the way the app's page does
        if not rule():
            s.ctl.click(switch)
    s._act(start)
    srv = lambda: box.get("srv")
    asked = lambda path: srv().count(path) if srv() else None
    s._act(lambda: s.ctl.send("fixture", "mail-picture", srv().url("/p.png")))
    arrived = ("loaded", 0, True)
    s.expect("the-picture-is-asked-for-and-arrives", lambda: until(lambda: (line()["state"], line()["missing"], (asked("/p.png") or 0) >= 1), arrived), arrived)
    s.expect("a-pixel-is-never-asked-for", lambda: (asked("/p.png-pixel"), line()["trackers"]), (0, 1))
    s.expect("loaded-pictures-leave-the-line-quiet", lambda: (line()["line"], line()["retry_shown"], line()["load_once_shown"]), (catalogue("1 tracker blocked"), False, False))
    s._act(lambda: setattr(srv(), "fail", True))
    s._act(lambda: s.ctl.send("fixture", "mail-picture", srv().url("/q.png")))
    said = ("missing", True, True)
    s.expect("a-picture-that-does-not-come-is-said", lambda: until(lambda: (line()["state"], line()["retry_shown"], (asked("/q.png") or 0) >= 1), said), said)
    s._act(lambda: setattr(srv(), "fail", False))
    s.click("mac-main.reader.pictures.retry")
    s.expect("try-again-brings-the-picture", lambda: until(lambda: (line()["state"], line()["missing"], (asked("/q.png") or 0) >= 2), ("loaded", 0, True)), ("loaded", 0, True))

    def end():
        if rule():
            s.ctl.click(switch)
        if srv():
            srv().stop()
    s._act(end)


OPEN_SAMPLE_S = 1.4     # J1.6h: how long the view's height is sampled after the key
OPEN_ASK_S = 0.4        # J1.6h: for this long after the key the check also asks the page for a measure (a measure during the load)
OPEN_GRACE_S = 0.2      # J1.6h: one layout pass after the model reserved the final height (a spring would still be moving for 0.5 s and more)


def opens_at_once(samples, grace=OPEN_GRACE_S):
    """samples: [(seconds after the key, the view's height on screen, the height the model reserved)], one mail. The mail opens at once when, from `grace`
    after the first sample whose reserved height is the final one, the view is never at another height. Answers "at once", or the words of what was seen."""
    if not samples:
        return "no sample of the body's frame"
    final = samples[-1][2]
    if final <= 40:
        return "the mail is %d pt high: too short to tell an unfolding height from a set one (%d samples)" % (final, len(samples))
    first = next(t for t, _, r in samples if abs(r - final) <= 0.5)
    late = [(round(t, 2), round(v, 1)) for t, v, _ in samples if t > first + grace and abs(v - final) > 0.5]
    if not late:
        return "at once"
    return "final height %d pt reserved %.2f s after the key; the view was still at other heights more than %.2f s later: %s (%d samples)" % (final, first, grace, late[:8], len(samples))


def drive_open_height(j):
    """Lead, 13 Nov (hidden probe on main145: the view's height grew from 32 to 166 pt over 0.7 s after a load): a mail's text is given its full height in one
    step when the mail opens. The unfold motion is for a height that changes under the reader's eyes, never for the open. Sampled from
    `mac-main.reader.body.frame`, which asks for no measure (the web row asks for one at each read, and that changes what is measured)."""
    s = j.step("J1.6h", "Mail view: a mail opens at its full height at once")
    box = {}

    def sample():
        fast = getattr(s.ctl, "instant", False)
        s.ctl.key("Escape")
        s.ctl.key("K")
        out, t0, i = [], time.monotonic(), 0
        s.ctl.key("J")
        while (i < 40) if fast else (time.monotonic() - t0 < OPEN_SAMPLE_S):
            if not fast and time.monotonic() - t0 < OPEN_ASK_S:
                # a measure is asked while the page is still on its way (the web row asks for one at each read): the case that made the height unfold on
                # main145. A plain open asks for none before its page has loaded, so without this the broken copy stays green on the app (main148).
                try:
                    s.ctl.rows("mac-main.reader.web")
                except (Unreadable, IndexError):
                    pass
            try:
                r = s.ctl.rows("mac-main.reader.body.frame")[0]
                out.append((i * 0.03 if fast else time.monotonic() - t0, float(r["view_height"]), float(r["reserved"]), r.get("id")))
            except (Unreadable, IndexError, KeyError):
                pass
            i += 1
            if not fast:
                time.sleep(0.02)
        mail = out[-1][3] if out else None
        box["samples"] = [(a, b, c) for a, b, c, d in out if d == mail]
    s._act(sample)
    s.expect("a-mail-opens-at-its-full-height-at-once", lambda: opens_at_once(box.get("samples") or []), "at once")


def until(fn, want, tries=60, gap=0.1):
    """Polls fn until it answers want (a page finishes its load a moment after the click); answers the last value read."""
    v = fn()
    for _ in range(tries):
        if v == want:
            return v
        time.sleep(gap)
        v = fn()
    return v


def catalogue(key):
    """The English word of a catalogue key (Sources/Resources/Localizable.xcstrings): the one source of the app's words."""
    import json
    with open(os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8") as f:
        return json.load(f)["strings"][key]["localizations"]["en"]["stringUnit"]["value"]


# MUTANTS: per check, the mutation of the app that must turn it red.
# J1.6/selection-shows-the-whole-body | the reader shows no body until Enter (the resting state is back)
# J1.6/selection-body-is-longer-than-the-preview | a selected mail shows only its preview sentence
# J1.6/written-mail-has-no-sheet | a written mail is drawn on the white sheet
# J1.6/the-dots-pill-shows-its-key | the three-dot pill drops its key cap
# J1.6/no-blocked-no-line | the pictures line shows on a mail that blocked nothing
# J1.6/dots-key-opens-everything | the key O does nothing
# J1.6/an-opened-quote-draws-one-rule | the opened history draws the folded part's rule and the quote's own: two rules for one quote level
# J1.6/text-above-does-not-move | opening the dots pushes the written text down
# J1.6/dots-key-again-folds | the key O only opens, never folds
# J1.6/dots-click-opens-everything | a click on the dots does nothing
# J1.6/dots-click-again-folds | a click on the dots only opens
# J1.6b/pictures-line-says-what-was-blocked | the pictures line names no count
# J1.6b/load-once-shows-its-key-cap | Load once shows no key cap
# J1.6b/nothing-loads-before-asked | the pictures load before the owner asks
# J1.6b/load-once-click-loads-the-pictures-of-this-viewing | Load once does nothing when clicked
# J1.6b/load-once-keeps-the-page-script-off | loading the pictures turns page script on
# J1.6b/after-the-ask-the-line-says-what-did-not-come | after Load once the line still says the pictures are not loaded
# J1.6b/pictures-that-did-not-come-offer-try-again | pictures that did not come offer nothing
# J1.6b/try-again-loads-the-pictures-again | Try again does nothing
# J1.6b/key-i-again-tries-the-missing-pictures-again | a second I does nothing
# J1.6b/always-click-keeps-the-sender | Always does not remember the sender
# J1.6b/always-forget-leaves-nothing | the sender stays remembered after the drive forgets it
# J1.6b/load-once-does-not-outlast-the-viewing | the pictures of one viewing stay loaded for the next
# J1.6b/designed-mail-has-the-sheet | a designed mail loses its white sheet
# J1.6b/pictures-line-counts-pictures-and-trackers | every blocked picture is called a tracker
# J1.6b/nothing-is-fetched-before-the-click | the page asks the sender's server for a picture before the owner clicks
# J1.6b/load-fetches-the-pictures-and-never-a-pixel | Load once on a test copy asks the sender's server for a picture (the copy must ask no server off this Mac)
# J1.6b/colours-action-names-the-other-way | the foot word does not name what the key does now
# J1.6b/shift-i-shows-the-app-colours | shift I does nothing on a designed mail
# J1.6b/colours-action-follows-the-switch | the foot word stays the same after the switch
# J1.6b/colours-door-shows-the-original-colours-again | the colours door only switches one way
# J1.6b/key-i-loads-this-mails-pictures | the key I does nothing
# J1.6b/written-sample-has-no-sheet | a written mail starts on the sheet
# J1.6b/written-inline-questions-stay-above-as-quotes | inline questions lose their quote rule
# J1.6b/written-mail-folds-the-history | the earlier message is shown open with no dots
# J1.6b/shift-i-gives-a-written-mail-its-original-colours | shift I does nothing on a written mail
# J1.6b/plain-text-reads-as-paragraphs | plain text is one block of raw text
# J1.6b/plain-text-links-are-links | web and mail addresses in plain text are not links
# J1.6b/plain-text-quotes-are-quotes | lines starting with > are not drawn as a quote
# J1.6e/wide-fixture-mail-is-wider-than-the-pane | the page's width is never measured (0), so the fit has no pane to compare with
# J1.6e/wide-mail-is-laid-out-at-its-own-width | the page reports no natural width for the wide mail
# J1.6f/with-the-rule-off-a-mail-asks-first | with the rule off a mail still opens with its pictures asked
# J1.6f/with-the-rule-on-a-mail-opens-with-its-pictures-asked | the rule is on and a mail still waits for Load once
# J1.6f/the-rule-never-asks-for-a-pixel | the rule counts the tracking pixels among the pictures it can ask for
# J1.6f/the-note-says-pictures-load-on-open | the note under the line still says that pictures are never loaded
# J1.6f/a-mail-read-in-junk-asks-first | a mail read in Junk opens with its pictures asked
# J1.6f/the-switch-off-asks-first-again | the switch only turns the rule on
# J1.6g/the-picture-is-asked-for-and-arrives | with the rule on the page never asks the server for the picture
# J1.6g/a-pixel-is-never-asked-for | the page asks the server for the tracking pixel too
# J1.6g/loaded-pictures-leave-the-line-quiet | the line keeps a sentence about pictures after they loaded
# J1.6g/a-picture-that-does-not-come-is-said | a picture the server refuses is called loaded
# J1.6g/try-again-brings-the-picture | Try again does not ask the server again
# J1.6e/wide-mail-is-scaled-to-the-pane | a wide mail is shown at scale 1, no fit
# J1.6e/wide-mail-is-not-cut-off-at-the-right | the fit scale is too big: the scaled mail is wider than the pane
# J1.6e/the-fit-alone-does-not-scroll-sideways | the fit alone opens the sideways scroll
# J1.6c/body-is-painted-after-the-pane-is-rebuilt | the web view stays hidden after the pane is rebuilt with the same message
# J1.6/layout-comes-from-the-engine | the layout is read from the old local rule, not the engine
# J1.6/fold-follows-the-engines-segments | the fold still uses the single quote offset on a prepared mail
# J1.6h/a-mail-opens-at-its-full-height-at-once | the first height of an opened mail unfolds (a measure of a page still on its way counts as the first measure)
# J1.6d/unprepared-mail-falls-back-to-written | a mail with no prepared fields claims the engine's layout
# J1.6d/trackers-only-says-trackers-and-no-pictures | a trackers-only mail is counted as pictures
# J1.6d/trackers-only-offers-no-load-once | a trackers-only mail offers Load once
# J1.6d/fold-follows-fold-from | the fold ignores fold_from and cuts at the old quote offset
# J1.6d/inline-quote-and-text-after-it-stay-above-the-dots | an inline quote or the text after it is folded behind the dots
# J1.6d/reader-holds-the-keys | the reader does not hold the keys after the search closes (O and the colour switch go to the list)
# J1.6d/search-is-closed-before-the-keys | Escape leaves the search open, so the keys after it go to the search field
# J1.6d/dots-open-everything-the-engine-folded | the dots open only part of what the engine folded
# J1.6d/degraded-mail-says-it-is-plain-text | a degraded mail shows no warning line
# J1.6d/a-normal-mail-says-nothing-of-it | every mail shows the plain-text warning
# J1.6d/two-documents-for-two-colours | the page holds one document for both colours
# J1.6d/the-switch-changes-the-document | shift I recolours the same document instead of switching documents
# J1.6d/the-switch-back-restores-the-written-document | shift I switches one way only
# J1.6d/a-written-mail-with-a-picture-signature-is-written | the mail's layout is designed because its signature holds pictures and a table: it is drawn on a white page
# J1.6d/signature-pictures-are-marks-at-their-declared-size | a blocked icon shows the words stacked inside its width, or is taller than it declared
# J1.6d/a-long-address-is-one-short-link | a long tracking address is drawn in full, wrapped over many lines (or its words are not the link)
# J1.6d/a-link-opens-the-full-address | the short link opens a cut address, or its tooltip is not the address
# J1.6d/a-newsletter-keeps-its-paper | a designed newsletter is drawn in the app's colours
# J1.6d/the-designed-page-draws-its-own-rules | the rules of the sender's-colours mode match nothing (written for another class than the page carries): no inner margin, no tile
# J1.6d/only-the-senders-document-shows-in-the-senders-colours | both documents of a two-version mail are drawn, one under the other
# J1.6d/a-wide-blocked-picture-keeps-its-words | a wide blocked picture loses its words
# J1.6c/painted-means-text-in-the-page | painted reads true on a web view whose page holds no text
# J1.6c/the-web-view-is-in-a-window-with-no-filter-above-it | the web view sits under a filtered or hidden layer
# J1.6/paragraphs-in-order | paragraphs are merged or reordered
# J1.6/no-raw-markup | markup is shown as text
# J1.6/paragraph-probe-control | the body probe returns no paragraphs
# J1.6/older-quoted-text-folded | quoted text is shown open
# J1.6/first-escape-closes-the-reply-field | Esc leaves the reply field mounted, or closes the reader at once
# J1.6/second-escape-returns-to-list | the second Esc leaves the reader open
# J1.6/second-thread-is-msg-04 | J does not move the list focus, so the reader shows the first thread again
# J1.6/trackers-blocked-count | blocked trackers are not counted
# J1.6/tracker-count-not-zero | the tracker count is hard-coded to 0
# J1.6/thread-has-a-quote | the body view reports no quote for a message that has one
# J1.6/no-trackers-no-button | the trackers door answers ok on a message with no blocked image
# J1.6/show-quoted-click-unfolds | the Show quoted text button does nothing when clicked
# J1.6/show-quoted-click-again-folds | the button unfolds the quote and cannot fold it again
# J1.6/no-quote-no-show-quoted-button | the Show quoted text door answers ok on a message with no quote
# J1.6/note-starts-closed | the images-blocked note is open before any click
# J1.6/trackers-button-click-opens-the-note | the "N images blocked" button does nothing when clicked
# J1.6/trackers-button-click-again-closes-the-note | the note stays open on the second click


WIDE_PANE = 560.0          # the stand-in's reading pane width (a stand-in value, never read from a build)


class ModelCtl(Ctl):
    """A stand-in for the control socket: the list, the reader, the body view and its two buttons, with MUTANT labels applied."""
    instant = True

    def __init__(self, mutants=()):
        super().__init__("")
        self.mut, self.cursor, self.open, self.reply, self.quoted_open, self.explain = set(mutants), 0, None, False, False, False
        self.sample, self.colours, self.asked, self.always, self.script, self.rebuilt = None, None, False, set(), False, False
        self.loads = 0                                  # page loads so far (Load once, Try again): the stand-in fetches nothing, like a test copy
        self.rule, self.junk, self.by_owner = False, False, False   # J1.6f: the Settings rule (a test copy starts with ask first), a mail read in Junk, who asked
        self.srv, self.local, self.local_ok = None, None, False      # J1.6g: the check's local picture server, the address of the shown mail's picture, whether it came
        self.searching = False
        self.left_search = False

    def local_pictures(self, srv):
        self.srv = srv

    def _fetch_local(self):
        """What the app's page does when the mail's pictures are asked for: one GET for the picture, none for the pixel."""
        import urllib.request, urllib.error
        if self.m("the-picture-is-asked-for-and-arrives"):
            self.local_ok = False
            return
        got = False
        for u in [self.local] + ([self.local + "-pixel"] if self.m("a-pixel-is-never-asked-for") else []):
            try:
                with urllib.request.urlopen(u, timeout=5) as r:
                    ok = r.status == 200
            except urllib.error.URLError:
                ok = False
            if u == self.local:
                got = ok
        self.local_ok = got or self.m("a-picture-that-does-not-come-is-said")

    def m(self, name):
        return any(("%s/%s" % (st, name)) in self.mut for st in ("J1.6", "J1.6b", "J1.6c", "J1.6d", "J1.6e", "J1.6f", "J1.6g", "J1.6h"))

    def _msg(self):
        return ROWS[self.open]

    def _quote(self):
        q = self._msg().get("quoted")
        return list(q) if q and not self.m("thread-has-a-quote") else []

    def _trackers(self):
        n = self._msg().get("trackers_blocked") or 0
        return 0 if self.m("tracker-count-not-zero") or self.m("trackers-blocked-count") else n

    # -- A129: the mail the body shows (a fixture row, or one of the drawings' samples), its colours, its pictures
    def _layout(self):
        if self.sample == "signature":
            return "designed" if self.m("a-written-mail-with-a-picture-signature-is-written") else "written"      # a written mail with a picture signature stays written
        if self.sample == "newsletter" and self.m("a-newsletter-keeps-its-paper"):
            return "written"
        return "designed" if self.sample in ("designed", "newsletter") else "written"

    def _colours(self):
        return self.colours or ("original" if self._layout() == "designed" else "app")

    def _has_fold(self):
        if self.sample == "folded":
            return True
        if self.sample:
            return self.sample == "written" and not self.m("written-mail-folds-the-history")
        return bool(ROWS[self.open if self.open is not None else self.cursor].get("quoted")) and not self.m("thread-has-a-quote")

    def _blocked(self):
        if self.sample:
            return DESIGNED_PICTURES if self.sample == "designed" else 0
        return ROWS[self.open if self.open is not None else self.cursor].get("trackers_blocked") or 0

    def _sender(self):
        if self.sample:
            return {"trackers": "news@shop.example", "degraded": "big@sender.example", "twodocs": "claire.martin@lorvan-hotels.example", "folded": "claire.martin@lorvan-hotels.example",
                    "designed": "elise@oranta.example", "written": "claire.martin@lorvan-hotels.example", "plain": "karen@linea-supply.example",
                    "wide": "news@industry-weekly.example", "signature": "robin.vale@workshop.example", "newsletter": "letters@garden.example", "trackinglinks": "digest@news.example"}[self.sample]
        return ROWS[self.open if self.open is not None else self.cursor]["from"]

    def _fresh(self):
        """A mail comes on screen: its own colours, nothing asked, nothing open."""
        self.colours, self.quoted_open, self.explain, self.script = None, False, False, False
        self.asked = self.asked and self.m("load-once-does-not-outlast-the-viewing")
        self.asked = self.asked or self._sender() in self.always
        self.by_owner = False
        on = (self.rule and not self.m("with-the-rule-on-a-mail-opens-with-its-pictures-asked")) or (not self.rule and self.m("with-the-rule-off-a-mail-asks-first"))
        if on and self._blocked() and (not self.junk or self.m("a-mail-read-in-junk-asks-first")):
            self.asked = True

    def _fetchable(self, asked):
        if self.m("nothing-is-fetched-before-the-click"):
            return 3                                    # fetched with no click
        if not asked or self.sample != "designed":
            return 0
        return 1 if self.m("load-fetches-the-pictures-and-never-a-pixel") else FETCHABLE_ON_A_TEST_COPY

    def _links(self):
        """The links of the tracking sample: the words (or the site and "link") are drawn, the full address is the target. Mutants: the full address drawn; a cut target."""
        out = []
        for text, full in zip(TRACKING_TEXTS, TRACKING):
            out.append({"text": full if self.m("a-long-address-is-one-short-link") else text,
                        "href": full[:30] if self.m("a-link-opens-the-full-address") else full, "title": full})
        return out

    def _placeholders(self):
        """The blocked pictures of the signature and newsletter samples as the document draws them: a picture declared narrower than PLACEHOLDER_TEXT_MIN_PX is a mark
        with no words at its declared size; a wider one carries its words. Mutant `icon-shows-words`: the words stack inside an icon's width."""
        decl = {"signature": [(100, 100)] + [(24, 24)] * 3 + [(16, 16)] * 4, "newsletter": [(600, 200), (280, 180), (280, 180)]}.get(self.sample, [])
        out = []
        for i, (w, h) in enumerate(decl):
            mark = w < PLACEHOLDER_TEXT_MIN_PX and not self.m("signature-pictures-are-marks-at-their-declared-size")
            if w >= PLACEHOLDER_TEXT_MIN_PX and self.m("a-wide-blocked-picture-keeps-its-words"):
                mark = True
            out.append({"ax": "mac-main.reader.body.placeholder.%d" % i, "mark": mark, "width": w, "height": h,
                        "words": "" if mark else "Picture not loaded"})
        return out

    def _pictures_row(self):
        if self.sample == "local":
            ok, quiet = self.local_ok, not self.m("loaded-pictures-leave-the-line-quiet")
            words = "1 tracker blocked" if ok and quiet else "The picture could not be loaded, 1 tracker blocked"
            return [{"line": words, "pictures": 1, "trackers": 1, "loaded": True, "by_rule": True, "quarantined": False, "askable": 1,
                     "state": "loaded" if ok else "missing", "missing": 0 if ok else 1, "loads": self.loads, "retry_shown": not ok or not quiet,
                     "always": False, "sender": "pictures@this-mac.example", "load_once_key": "I", "load_once_shown": False, "always_shown": False,
                     "fetchable": 0, "list": "pictures", "remote_blocked": True, "javascript_allowed": False, "note": ""}]
        if self.sample == "trackers":
            shown = self.m("trackers-only-offers-no-load-once")
            return [{"line": "2 trackers blocked", "pictures": 3 if self.m("trackers-only-says-trackers-and-no-pictures") else 0, "trackers": 2, "loaded": False, "always": False,
                     "sender": self._sender(), "load_once_key": "I", "load_once_shown": shown, "always_shown": shown, "fetchable": 0, "list": "none",
                     "remote_blocked": True, "javascript_allowed": False}]
        n = self._blocked()
        if n == 0 and not self.m("no-blocked-no-line"):
            return []
        asked = self.asked or self.m("nothing-loads-before-asked")
        trackers = (n if self.m("pictures-line-counts-pictures-and-trackers") else DESIGNED_TRACKERS) if self.sample == "designed" else 0
        pictures = 0 if self.m("pictures-line-says-what-was-blocked") else n
        told = asked and not self.m("after-the-ask-the-line-says-what-did-not-come")
        words = catalogue("%1$@ of %2$@ pictures could not be loaded").replace("%1$@", str(pictures)).replace("%2$@", str(pictures)) if told else "%s pictures not loaded" % ("" if pictures == 0 else pictures)
        note = "Pictures load when you open a mail. Tracking pixels are never loaded." if self.rule and not self.junk and not self.m("the-note-says-pictures-load-on-open") else "Images from the web are never loaded. Nothing leaves this Mac."
        return [{"line": words, "pictures": pictures, "trackers": trackers, "loaded": asked,
                 "by_rule": asked and not self.by_owner, "quarantined": self.junk, "note": catalogue(note),
                 "askable": DESIGNED_PICTURES if self.m("the-rule-never-asks-for-a-pixel") else (DESIGNED_ASKABLE if self.sample == "designed" else 0),
                 "state": "missing" if told else "blocked", "missing": pictures if told else None, "loads": self.loads,
                 "retry_shown": asked and not self.m("pictures-that-did-not-come-offer-try-again"),
                 "always": self._sender() in self.always, "sender": self._sender(), "load_once_key": "" if self.m("load-once-shows-its-key-cap") else "I",
                 "fetchable": self._fetchable(asked),
                 "load_once_shown": not asked, "always_shown": not asked, "list": "pictures" if asked else "none", "remote_blocked": True,
                 "javascript_allowed": self.script}]

    def _body_row(self, paragraphs, quoted_folded):
        sheet = self._colours() == "original" or (self._layout() == "written" and self.m("written-mail-has-no-sheet")) or (self._layout() == "designed" and False)
        if self.m("designed-mail-has-the-sheet") and self._layout() == "designed":
            sheet = False
        if self.m("written-sample-has-no-sheet") and self.sample == "written":
            sheet = True
        colours_action = "App colours" if (sheet or self.m("colours-action-follows-the-switch")) and not self.m("colours-action-names-the-other-way") else "Original colours"
        if self.m("colours-action-names-the-other-way"):
            colours_action = "Original colours"
        plain = self.sample == "plain"
        quotes = {"written": 0 if self.m("written-inline-questions-stay-above-as-quotes") else WRITTEN_QUOTES,
                  "plain": 0 if self.m("plain-text-quotes-are-quotes") else PLAIN_QUOTES}.get(self.sample, 1 if self._has_fold() else 0)
        folded_text = []
        if self.sample == "folded":
            above = FOLDED_ABOVE[:2] if self.m("inline-quote-and-text-after-it-stay-above-the-dots") else FOLDED_ABOVE
            behind = FOLDED_ABOVE[1:] + FOLDED_BEHIND if self.m("fold-follows-fold-from") else FOLDED_BEHIND
            if self.m("dots-open-everything-the-engine-folded"):
                behind = FOLDED_BEHIND[:1]
            paragraphs = above + (behind if self.quoted_open else [])
            if self.m("inline-quote-and-text-after-it-stay-above-the-dots") and self.quoted_open:
                paragraphs = FOLDED_ABOVE + FOLDED_BEHIND
            folded_text = behind
            if self.m("dots-open-everything-the-engine-folded") and self.quoted_open:
                paragraphs = FOLDED_ABOVE + FOLDED_BEHIND[:1]
        prepared = self.sample not in ("written", "plain") and not self.m("layout-comes-from-the-engine")
        if self.sample in ("written", "plain") and self.m("unprepared-mail-falls-back-to-written"):
            prepared = True
        two = self.sample == "twodocs" and not self.m("two-documents-for-two-colours")
        colours_now = self._colours()
        drawn = "written" if (colours_now == "app" or self.m("the-switch-changes-the-document")) and not (self.m("the-switch-back-restores-the-written-document") and self.colours == "app" and self.sample == "twodocs") else "designed"
        if self.sample != "twodocs":
            drawn = "written" if colours_now == "app" else "designed"
        wide = self.sample == "wide"
        natural = 0.0 if (wide and self.m("wide-mail-is-laid-out-at-its-own-width")) else (float(wide_natural_width()) + 2 * PAPER_PAD_PX if wide else 0.0)
        fit = 0.9 if self.m("wide-mail-is-not-cut-off-at-the-right") else (1.0 if self.m("wide-mail-is-scaled-to-the-pane") else WIDE_PANE / (700.0 + 2 * PAPER_PAD_PX))
        scale = fit if wide else 1.0
        return {"paragraphs": paragraphs, "has_quote": self._has_fold(), "quoted_folded": quoted_folded,
                "natural_width": natural, "view_width": 0.0 if self.m("wide-fixture-mail-is-wider-than-the-pane") else WIDE_PANE, "scale": scale, "fit_scale": scale, "scaled_width": natural * scale, "zoom": 1.0,
                "scrolls_horizontally": wide and self.m("the-fit-alone-does-not-scroll-sideways"),
                "layout_source": "engine" if prepared else "fallback", "folded_text": folded_text,
                "fold_source": "segments" if (prepared and not self.m("fold-follows-the-engines-segments")) else "quote_offset",
                "degraded": (self.sample == "degraded" and not self.m("degraded-mail-says-it-is-plain-text")) or self.m("a-normal-mail-says-nothing-of-it"),
                "documents": 2 if two else 1, "drawn_document": drawn,
                "drawn": {"class": colours_now, "quote_bars": ("2" if self.m("an-opened-quote-draws-one-rule") else "1") if self._has_fold() else "0",
                          "pad_left": ("%dpx" % PAPER_PAD_PX) if colours_now == "original" and not self.m("the-designed-page-draws-its-own-rules") else "0px",
                          "tile_fill": TILE_FILL if colours_now == "original" and not self.m("the-designed-page-draws-its-own-rules") else "rgba(255, 255, 255, 0.1)",
                          "app_document": ("none" if colours_now == "original" and not self.m("only-the-senders-document-shows-in-the-senders-colours") else "block") if two else "",
                          "sender_document": ("block" if colours_now == "original" else "none") if two else ""},
                "javascript_allowed": self.script, "format": "plain" if plain else "html",
                "painted": not (self.rebuilt and self.m("body-is-painted-after-the-pane-is-rebuilt")),
                "layout": self._layout(), "colours": self._colours(), "sheet": sheet, "dots": self._has_fold(),
                "dots_key": "" if self.m("the-dots-pill-shows-its-key") else "O", "fold_open": self.quoted_open,
                "colours_action": colours_action, "colours_applies": not plain,
                "text_above_y": 100 + (20 if self.quoted_open and self.m("text-above-does-not-move") else 0),
                "blocks": (1 if self.m("plain-text-reads-as-paragraphs") else PLAIN_BLOCKS) if plain else 6,
                "links": 0 if (plain and self.m("plain-text-links-are-links")) else (PLAIN_LINKS if plain else 0), "quote_blocks": quotes}

    def _flip(self):
        self.colours = "app" if self._colours() == "original" else "original"

    def _key_a129(self, k):
        """O, I, shift I: the reader holds the keys (no reply field)."""
        if self.open is None or self.reply:
            return
        if k == "O" and self._has_fold():
            if self.m("dots-key-opens-everything") or (self.quoted_open and self.m("dots-key-again-folds")):
                return
            self.quoted_open = not self.quoted_open
        elif k == "I" and self._blocked() and not self.asked:
            if not self.m("key-i-loads-this-mails-pictures"):
                self._ask()
        elif k == "I" and self._blocked() and self.asked:
            if not self.m("key-i-again-tries-the-missing-pictures-again"):
                self.loads += 1                         # the same document loads again
        elif k == "⇧I" and self.sample != "plain":
            if self.m("shift-i-shows-the-app-colours") and self._layout() == "designed":
                return
            if self.m("shift-i-gives-a-written-mail-its-original-colours") and self._layout() == "written":
                return
            self._flip()

    def _ask(self):
        self.asked = True
        self.by_owner = True
        self.loads += 1
        self.script = self.m("load-once-keeps-the-page-script-off")

    def send(self, verb, *args):
        if verb == "key":
            k = args[0]
            if k == "⌘K":
                self.rebuilt, self.searching = True, True
                return {"ok": True}
            if self.searching and k == "Escape":
                self.searching = self.m("search-is-closed-before-the-keys")          # the field's Escape closes the search
                self.left_search = True
                return {"ok": True}
            if k in ("O", "I", "⇧I"):
                if not self.searching:                                                  # the search field holds the keys while its surface is shown
                    self._key_a129(k)
                self.last = {"window_id": 10, "focus": "reader", "handled": True, "windows": 1, "text_before": "", "text_after": ""}
                return {"ok": True}
            if k == "Enter" and self.open is None:
                self.open, self.reply, self.quoted_open, self.explain = self.cursor, not self.m("first-escape-closes-the-reply-field"), False, False
                self.sample = None
                self._fresh()
            elif k == "Escape":
                if self.reply:
                    self.reply = False
                elif self.open is not None and not self.m("second-escape-returns-to-list"):
                    self.open = None
            elif k == "J" and not self.m("second-thread-is-msg-04"):
                self.cursor = min(self.cursor + 1, len(ROWS) - 1)
            if k == "J":
                self.frame_reads = 0                     # J1.6h: a new mail is shown; its frame is read from here
            return {"ok": True}
        if verb == "click":
            ax = args[0]
            if self.open is None:
                raise Unreadable("click %s: no body on screen" % ax)
            if ax == "mac-main.reader.body.show-quoted":
                if not self._quote() and not self.m("no-quote-no-show-quoted-button"):
                    raise Unreadable("click %s: refused" % ax)
                if not self.m("show-quoted-click-unfolds") and not (self.quoted_open and self.m("show-quoted-click-again-folds")):
                    self.quoted_open = not self.quoted_open
                return {"ok": True}
            if ax == "mac-main.reader.trackers.button":
                if not self._msg().get("trackers_blocked") and not self.m("no-trackers-no-button"):
                    raise Unreadable("click %s: refused" % ax)
                if not self.m("trackers-button-click-opens-the-note") and not (self.explain and self.m("trackers-button-click-again-closes-the-note")):
                    self.explain = not self.explain
                return {"ok": True}
            if ax == "mac-settings-general.privacy.pictures":
                if not (self.rule and self.m("the-switch-off-asks-first-again")):
                    self.rule = not self.rule
                return {"ok": True}
            if ax.startswith("mac-main.reader.body.sample."):
                name = ax.rsplit(".", 1)[1]
                self.junk = name == "designed-junk"            # the same mail as read in Junk
                self.sample = "designed" if self.junk else name
                self._fresh()
                return {"ok": True}
            if ax == "mac-main.reader.pictures.load-once":
                if self._blocked() and not self.asked and not self.m("load-once-click-loads-the-pictures-of-this-viewing"):
                    self._ask()
                return {"ok": True}
            if ax == "mac-main.reader.pictures.retry" and self.sample == "local":
                self.loads += 1
                if not self.m("try-again-brings-the-picture"):
                    self._fetch_local()
                return {"ok": True}
            if ax == "mac-main.reader.pictures.retry":
                if not self.asked:
                    raise Unreadable("click %s: refused" % ax)
                if not self.m("try-again-loads-the-pictures-again"):
                    self.loads += 1
                return {"ok": True}
            if ax == "mac-main.reader.pictures.always":
                if self._blocked() and self._sender() not in self.always:
                    if not self.m("always-click-keeps-the-sender"):
                        self.always.add(self._sender())
                    self.asked = True
                return {"ok": True}
            if ax == "mac-main.reader.pictures.always.forget":
                if not self.m("always-forget-leaves-nothing"):
                    self.always.discard(self._sender())
                return {"ok": True}
            if ax == "mac-main.reader.body.colours":
                if not self.m("colours-door-shows-the-original-colours-again"):
                    self._flip()
                return {"ok": True}
            if ax == "mac-main.reader.body.dots":
                if self._has_fold() and not self.m("dots-click-opens-everything") and not (self.quoted_open and self.m("dots-click-again-folds")):
                    self.quoted_open = not self.quoted_open
                return {"ok": True}
            raise Unreadable("click %s: no such control" % ax)
        if verb == "fixture" and args[:1] == ("mail-picture",):
            # a mail whose one picture is kept at the check's own server: with the rule on it is asked for as the mail opens
            self.sample, self.junk, self.local, self.local_ok = "local", False, args[1], False
            self.loads += 1
            if self.rule:
                self._fetch_local()
            return {"ok": True}
        if verb == "uistate":
            if self.searching:
                return {"ok": True, "state": {"surface": "mac-search", "first_responder": "search-field", "overlay": None}}
            if self.left_search and self.m("reader-holds-the-keys"):             # the mutant: the closed search leaves the list holding the keys
                return {"ok": True, "state": {"surface": "mac-main", "first_responder": "list", "overlay": None}}
            return {"ok": True, "state": {"surface": "mac-main" if self.open is None else "mac-reader", "first_responder": "reader" if self.open is not None else "list", "overlay": None}}
        if verb == "rows":
            ax = args[0]
            if ax == "mac-keyevent.monitor":
                return {"ok": True, "rows": [{"last": self.last}] if getattr(self, "last", None) else []}
            if ax == "mac-main.shell":
                return {"ok": True, "rows": [{"focused_pane": "reader" if self.open is not None else "list"}]}
            if ax == "mac-main.quickreply.field":
                return {"ok": True, "rows": [{}] if self.reply else []}
            if ax == "mac-main.list":
                return {"ok": True, "rows": [{"id": r["id"]} for r in ROWS]}
            if ax == "mac-main.reader.body.frame":       # J1.6h: the view is at the reserved height at once; the mutant lets it grow over 0.5 s
                n = self.frame_reads = getattr(self, "frame_reads", 0) + 1
                grown = min(166.0, 20.0 + 9.0 * n) if self.m("a-mail-opens-at-its-full-height-at-once") else 166.0
                return {"ok": True, "rows": [{"id": ROWS[self.cursor]["id"], "view_height": grown, "reserved": 166.0, "measured": True, "page_loaded": True}]}
            if ax == "mac-main.reader.web":
                blank = self.rebuilt and self.m("painted-means-text-in-the-page")
                return {"ok": True, "rows": [{"painted": True, "text_length": 0 if blank else 120, "in_window": True, "hidden": False,
                                              "layer_filters": 1 if self.m("the-web-view-is-in-a-window-with-no-filter-above-it") else 0, "height": 300}]}
            if ax == "mac-main.reader.body.links" and self.open is not None:
                return {"ok": True, "rows": self._links()}
            if ax == "mac-main.reader.body.placeholders" and self.open is not None:
                return {"ok": True, "rows": self._placeholders()}
            if ax == "mac-settings-general.settings":
                return {"ok": True, "rows": [{"pictures_on_open": self.rule}]}
            if ax == "mac-main.reader.pictures" and not (self.open is None and False):
                return {"ok": True, "rows": self._pictures_row()}
            if self.open is None and ax == "mac-main.reader.body":
                body = [ROWS[self.cursor]["preview"]] if self.m("selection-body-is-longer-than-the-preview") else list(ROWS[self.cursor]["body"])
                body = [] if self.m("selection-shows-the-whole-body") else body   # the resting state: the app answers no body (a stand-in for its refusal)
                return {"ok": True, "rows": [self._body_row(body, True)]}
            if self.open is None:
                raise Unreadable("rows %s: no message body is on screen" % ax)
            m = self._msg()
            if ax == "mac-main.reader.subject":
                return {"ok": True, "rows": [{"subject": m["subject"]}]}
            if ax == "mac-main.reader.trackers":
                return {"ok": True, "rows": [{"count": self._trackers(), "explain_open": self.explain or self.m("note-starts-closed")}]}
            if ax == "mac-main.reader.body":
                body = list(m["body"])
                if self.m("paragraphs-in-order"):
                    body = body[::-1]
                if self.m("paragraph-probe-control"):
                    body = []
                if self.m("no-raw-markup"):
                    body = ["<p>" + body[0] + "</p>"] + body[1:] if body else body
                folded = not self.quoted_open
                if self.m("older-quoted-text-folded"):
                    folded = False
                row = self._body_row(body + (self._quote() if self.quoted_open else []), folded)
                if not self.sample:
                    row["has_quote"] = bool(self._quote())
                return {"ok": True, "rows": [row]}
        raise Unreadable("%s: not modelled" % verb)


def model_run(mutants):
    j = Journey(ID, ModelCtl(mutants))
    drive(j)
    cs = j.checks()
    return {c.label for c in cs if c.state == "fail"}, {c.label for c in cs if c.state == "unjudged"}


def selftest(path):
    labels = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# J") and " | " in l]
    base_f, base_u = model_run(())
    rows, missed = [], []
    for label, what in labels:
        f, _ = model_run((label,))
        rows.append((label, what, label in f))
        if label not in f:
            missed.append(label)
    print("clean model: failed=%s unjudged=%s" % (sorted(base_f), sorted(base_u)))
    print("MUTANTS (a check is green only after it has been seen red on its mutant)")
    print("| check | mutant | seen red |")
    print("|---|---|---|")
    for label, what, red in rows:
        print("| %s | %s | %s |" % (label, what, "RED on this case" if red else "NOT RED"))
    print("%d mutants, %d not seen red, %d clean failures, %d clean not judged" % (len(labels), len(missed), len(base_f), len(base_u)))
    # the evidence a failed ⇧I case carries: the key verb's report and the shell's focus (an older build serves neither and the case does not crash)
    bad = 0
    for case in ("J1.6b/shift-i-shows-the-app-colours", "J1.6b/colours-action-follows-the-switch", "J1.6b/colours-door-shows-the-original-colours-again", "J1.6b/shift-i-gives-a-written-mail-its-original-colours"):
        mutant = case if case != "J1.6b/colours-action-follows-the-switch" else "J1.6b/shift-i-shows-the-app-colours"
        mutant = "J1.6b/shift-i-shows-the-app-colours" if case.endswith("colours-door-shows-the-original-colours-again") else mutant
        j = Journey(ID, ModelCtl((mutant,)))
        drive(j)
        c = next((c for c in j.checks() if c.label == case), None)
        print("FAIL %s: %s" % (case, c.detail if c else "case missing"))
        for need in ("app said: key '⇧I', keyevent window_id=10 focus='reader' handled=True windows=1 text_before='' text_after=''", "shell surface='mac-reader' focus='reader'", "focused pane 'reader'"):
            if not c or c.state != "fail" or need not in c.detail:
                print("EVIDENCE MISSING in %s: %s" % (case, need))
                bad += 1
    old = ModelCtl()
    old.rows = lambda ax, _r=ModelCtl.rows: (_ for _ in ()).throw(Unreadable("no rows at " + ax)) if ax in ("mac-keyevent.monitor", "mac-main.shell") else _r(old, ax)
    old.uistate = lambda: (_ for _ in ()).throw(Unreadable("no uistate"))
    line = read_press(old, "⇧I")
    print("older build: " + line)
    if "report absent" not in line or "unreadable" not in line:
        print("EVIDENCE MISSING: an older build is not read as absent")
        bad += 1
    return 0 if not (base_f or base_u or missed or bad) else 1


def main():
    path = os.path.abspath(__file__)
    if "--selftest" in sys.argv:
        return selftest(path)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    rows = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# J") and " | " in l]
    print("MUTANTS (a check is green only after it has been seen red on its mutant)")
    print("| check | mutant |")
    print("|---|---|")
    for k, v in rows:
        print("| %s | %s |" % (k, v))
    print_cases(r)                   # one line per judged case (pass lines too): `<step>/<case>  pass|fail|unjudged  <the values>`
    # journeys/lib/harness.py declared_mutants reads a step id as `J<n>.<n>` only, so every J1.6b..e row of the MUTANTS block was invisible to it and the run listed
    # them all under NO MUTANT (exit 1 with 0 failed). The accounting is redone here with the step ids this file uses; the harness file is not this check's to change.
    import re
    declared = {m.group(1) for l in open(path, encoding="utf-8") for m in [re.match(r"#\s+(J\d+\.\d+[a-z]?/[\w-]+)\s*\|\s*\S", l)] if m}
    labels = {l for l, _st, _d in r["cases"]}
    r["no_mutant"], r["stray_mutant"] = sorted(labels - declared), sorted(declared - labels)
    for line in r["fail_list"]:
        sys.stderr.write("FAIL %s\n" % line)
    for line in r["no_mutant"]:
        sys.stderr.write("NO MUTANT %s\n" % line)
    for line in r["stray_mutant"]:
        sys.stderr.write("STRAY MUTANT %s\n" % line)
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    ok = r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"]
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
