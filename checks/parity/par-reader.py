#!/usr/bin/env python3
"""par-reader (round 3): the per-message facts from message(id), the sign-in code chip, the sender card, Delete and
the quick reply mounted under the open thread.

Drives the RUNNING rig copy over the control socket through journeys/lib, like checks/reader-check.py. Expected
values come from docs/design/fixtures/mac-main-sample.json, never from a built screen.
Rounds 9 and 10 (the reader keys as live commands, Shift Tab back, no body re-read; the header toggle by click, key, N and P, shift O) also run on a model:
  python3 checks/parity/par-reader.py --selftest   every round-9 and round-10 check green on the model, red on its own mutant
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/par-reader.py'
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0.
"""
import json
import os
import re
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, Journey, declared_mutants, print_cases            # noqa: E402
from lib.fixture import FX, ROWS, ROW_COUNT, const                   # noqa: E402
from lib.ctl import Unreadable                                       # noqa: E402

ID = "par-reader"
TITLE = "Reader rounds 3, 5, 6 and 9: message facts, code, sender card, Delete with undo, quick reply, the strip and header icons, reader keys, scroll, light focus, links"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "the reader names each message's sender, shows the code chip, opens the sender card, deletes on Delete and mounts the quick reply"
THREAD = "mac-main.reader.thread"
CODE_ROW = next(i for i, m in enumerate(ROWS) if m["id"] == "msg-09")   # msg-09, the fixture's sign-in mail (GitHub, code 482913), at its place in the served list
CODE_MSG = ROWS[CODE_ROW]


def focus_row(s, n):
    """Move the list focus to row n with J (the list holds the focus after Escape)."""
    for _ in range(n):
        s.key("J")


def drive(j):
    first = FX["open_thread"]["messages"]
    first_count = len(first)
    s = j.step("par-reader-rest", "The focused thread shows at rest, with its actions in the header")
    s.expect("thread-shows-before-enter", lambda: (s.ctl.uistate()["surface"], s.ctl.rows("mac-main.reader.subject")[0]["subject"]),
             ("mac-main", ROWS[0]["subject"]))
    s.expect("actions-sit-in-the-header", lambda: [(r["label"], r["where"]) for r in s.ctl.rows("mac-main.reader.actions")],
             [(l, "header") for l in ("Archive", "Snooze", "Label", "Pin", "Reply", "More")])
    s.expect("summary-callout-shows-the-core-summary", lambda: [r["text"] for r in s.ctl.rows("mac-main.reader.summary-callout")],
             [first[-1]["agent_summary"]])
    s.expect("attachment-card-kind", lambda: [r["kind"] for r in s.ctl.rows("mac-main.reader.attachments")], ["PDF"])

    s = j.step("par-reader-foot", "The window's foot strip carries the keys; the reader draws none of its own")
    s.key("Escape")
    s.expect("no-reader-foot-strip-control", lambda: [refuses_click(s, "mac-main.reader.key.%s" % k)() for k in ("next", "previous", "open")], [True, True, True])
    def reader_gap_above_the_strip():
        """Points between the bottom of the reader's content and the top of the foot strip (the list's own foot marks it). Measured on main184's picture
        (walk-main184/05): 36.5 pt, the shell's room and the pane's own padding both kept."""
        c, f = s.ctl.measure("mac-main.reader.content").get("frame"), s.ctl.measure("mac-main.list.foot").get("frame")
        if not c or not f:
            raise Unreadable("no frame for the reader's content or the list's foot: %r, %r" % (c, f))
        return round(f["y"] - (c["y"] + c["h"]))
    s.expect("reader-content-ends-at-the-foot-strip", reader_gap_above_the_strip, 0)
    def under_text_shown_too_early(moves=9):
        """For each of `moves` moves down the list: the blocks under the open mail's text (its files, the thread's cards, the reply line) are drawn only once
        the text has its height. Read every few ms after the key: the text's box (`measure mac-main.reader.body`) and the mail whose under-text blocks are
        shown (`mac-main.reader.scroll` under_text_shown_for). A move is bad when the blocks are shown while the box is under 90 % of its final height, or
        when they are never shown. Measured on main188 before the fix (the lead's probe): a 1300 pt page mail kept one line of room from 45 to 155 ms
        after the key with the blocks drawn right under the header."""
        bad = []
        try:
            for i in range(moves):
                s.ctl.send("keyevent", "j")
                t0, seen = time.monotonic(), []
                while time.monotonic() - t0 < 1.5:
                    try:
                        h = float((s.ctl.measure("mac-main.reader.body").get("frame") or {}).get("h") or 0)
                    except Unreadable:
                        h = 0.0                                   # no text box on screen yet
                    row = s.ctl.rows("mac-main.reader.scroll")[0]
                    if "under_text_shown_for" not in row:
                        raise Unreadable("the build serves no under_text_shown_for (mac-main.reader.scroll)")
                    seen.append((h, row["under_text_shown_for"] == s.ctl.uistate().get("focus_message")))
                    if len(seen) > 6 and seen[-1][1] and all(abs(x[0] - seen[-1][0]) < 0.5 for x in seen[-4:]):
                        break
                final = seen[-1][0]
                early = [round(h) for h, shown in seen if shown and h < 0.9 * final]
                if early or not seen[-1][1]:
                    bad.append((i + 1, early[:3], round(final), seen[-1][1]))
        finally:
            for _ in range(moves):
                s.ctl.send("keyevent", "k")
        return bad
    s.expect("blocks-under-the-text-wait-for-the-text", under_text_shown_too_early, [])

    s = j.step("par-reader-facts", "Per-message facts from the core")
    s.arrive(("click", "mac-main.reader.open"), "mac-reader")
    s.click("mac-main.reader.earlier")
    s.expect("headers-start-collapsed-on-first-open", lambda: [r["open"] for r in s.ctl.rows("mac-main.reader.header")], [False] * first_count)
    s.expect("earlier-authors-from-core", lambda: [r["sender"] for r in s.ctl.rows(THREAD)], [m["from"] for m in first])
    s.expect("every-message-has-a-time", lambda: all(r["time"] for r in s.ctl.rows(THREAD)), True)
    s.expect("attachment-names", lambda: [r["name"] for r in s.ctl.rows("mac-main.reader.attachments")], ROWS[0]["attachments"])
    s.expect("sender-address-shown", lambda: s.ctl.rows("mac-main.reader.sender")[0]["address"], ROWS[0]["from"])

    s = j.step("par-reader-quick", "Quick reply mounted under the thread")
    s.key("R")
    s.expect("quick-reply-mounted", lambda: [r["thread"] for r in s.ctl.rows("mac-main.quickreply.field")] != [], True)
    s.key("Escape")
    s.expect("quick-reply-closes-on-escape", lambda: len(s.ctl.rows("mac-main.quickreply.field")), 0)
    s.key("Escape")
    s.expect("escape-returns-to-list", lambda: s.ctl.uistate()["surface"], "mac-main")

    focus_row(s, CODE_ROW)
    s = j.step("par-reader-code", "Sign-in code chip")
    s.arrive(("click", "mac-main.reader.open"), "mac-reader")
    s.expect("code-chip-shows-code", lambda: s.ctl.rows("mac-main.reader.codepill")[0]["code"], CODE_MSG["login_code"])
    s.click("mac-main.reader.codepill")
    s.expect("code-on-clipboard", lambda: s.ctl.rows("mac-main.reader.codepill")[0]["clipboard"], CODE_MSG["login_code"])
    s.expect("chip-says-copied", lambda: s.ctl.rows("mac-main.reader.codepill")[0]["copied"], True)
    s.expect("message-marked-read", lambda: s.ctl.core("message/%s/read" % CODE_MSG["id"]), True)

    s = j.step("par-reader-sender", "Sender card")
    s.expect("sender-card-counts-messages", lambda: s.ctl.rows("mac-main.reader.sender")[0]["contact_messages"] >= 1, True)
    s.arrive(("click", "mac-main.reader.sender"), "mac-labels-contacts")
    s.expect("card-is-the-sender", lambda: s.ctl.rows("mac-labels-contacts.contact.header")[0]["address"], CODE_MSG["from"])
    s.key("Escape")

    s = j.step("par-reader-delete", "Delete key")
    s.click("mac-main.reader.open")
    s.key("⇧#")                                    # the one key that deletes a mail, as in the list (13 Nov, f4d2c58ea: plain Backspace deletes no mail)
    s.expect("message-deleted-in-core", lambda: s.ctl.core("message/%s/deleted" % CODE_MSG["id"]), True)
    s.expect("delete-returns-to-list", lambda: s.ctl.uistate()["surface"], "mac-main")
    s.expect("delete-offers-undo", lambda: (s.ctl.rows("mac-main.list.state")[0]["undo_available"], s.ctl.rows("mac-main.list.state")[0]["undo_words"]), (True, "Deleted"))
    s.click("mac-settings-general.appearance.language.fr")
    s.expect("undo-words-follow-a-language-switch", lambda: (s.ctl.rows("mac-main.list.state")[0]["undo_key"], s.ctl.rows("mac-main.list.state")[0]["undo_words"]), ("Deleted", catalogue("Deleted", "fr")))
    s.click("mac-settings-general.appearance.language.en")
    s.key("Z")
    s.expect("undo-brings-the-message-back", lambda: s.ctl.core("message/%s/deleted" % CODE_MSG["id"]), False)

    strip_round(j)
    keys_round(j)
    scroll_round(j)
    focus_round(j)
    nav_round(j)
    o_round(j)
    colours_round(j)
    width_round(j)
    header_round(j)
    r10_round(j)
    hold_round(j)
    notdone_round(j)
    unsubscribe_round(j)
    doors_round(j)              # doors-read: on the served list, before links_round replaces it with its corpus (a one-message thread, no msg-01, no msg-11)
    unsub_twice_round(j)        # triage-keys: after doors_round (it still needs msg-11 unconfirmed), before links_round replaces the list
    r9_round(j)
    sweep_round(j)              # the sweep: before links_round, which replaces the list with its corpus
    links_round(j)              # the corpus it loads stays for the life of the app
    r9_ring(j)                  # needs that corpus: the newest message carries links and dates
    doors_links(j)              # doors-read: the link chip click needs that corpus
    search_escape_round(j)      # last: it loads the J2 search corpus, which replaces the list for good


ACTIONS = ["delete", "reply", "reply-all", "forward", "archive"]       # the strip's engine commands, left to right
STRIP_IDS = ["delete", "reply", "replyall", "forward", "archive"]
THREAD_MSG = FX["messages"][0]                                          # msg-01: three messages, two folded, one attachment


FAST = "--selftest" in sys.argv                 # the model has no clock to wait for


def wait(s, ms):
    """Pause the driver; the app's own quiet times (FOCUS_SLIDE_MS and the like) are real time."""
    if not FAST:
        s._act(lambda: time.sleep(ms / 1000.0))


def refuses(s, chord):
    """True when the key passes on (the socket answers ok:false)."""
    def thunk():
        try:
            s.ctl.key(chord)
            return False
        except Unreadable:
            return True
    return thunk


def catalogue(key, lang):
    """A key's text in one language, from the string catalogue (the source, never a built screen)."""
    with open(os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8") as f:
        return json.load(f)["strings"][key]["localizations"][lang]["stringUnit"]["value"]


def press_any(s, chord):
    """Press a key whether or not anything takes it (the socket answers ok:false for a key that passes on)."""
    try:
        s.ctl.key(chord)
    except Unreadable:
        pass


def preview_is(s, want, wait_s=3.0):
    """The Quick Look panel's `open`, read until it is `want` or `wait_s` passed (a file that is not on this Mac is fetched first). A build or a
    stand-in with no panel row reads closed."""
    def thunk():
        end = time.monotonic() + (0 if FAST else wait_s)
        while True:
            try:
                v = bool(s.ctl.rows("mac-main.attachments.panel")[0]["open"])
            except (Unreadable, IndexError, KeyError):
                v = False
            if v == want or time.monotonic() >= end:
                return v
            time.sleep(0.1)
    return thunk


def space_at_the_bottom(s):
    """Space with the thread at its end. A mail with no file lets the key pass on (the socket answers ok:false). A mail with files opens Quick Look
    on its first file: the files' header draws the Space key for the whole set (lead, 13 Nov, 53880c2ef), so the key must act."""
    def thunk():
        try:
            files = s.ctl.rows("mac-main.attachments")
        except Unreadable:
            files = []
        passed = refuses(s, "Space")()
        return passed if not files else ((not passed) and preview_is(s, True)())
    return thunk


def refuses_click(s, ax):
    """True when the app has no such control (the socket answers ok:false)."""
    def thunk():
        try:
            s.ctl.click(ax)
            return False
        except Unreadable:
            return True
    return thunk


def preset_key(s, command):
    """The key the active preset binds to a command, read from the shortcuts lane's own rows."""
    for r in s.ctl.rows("mac-shortcuts.preset"):
        if r["command"] == command:
            return r["key"]
    return None


def to_top(s):
    for _ in range(ROW_COUNT):
        s.key("K")


def strip_round(j):
    """Round 5: the strip on the message header (delete, reply, reply all, forward, archive), every button clicked."""
    s = j.step("par-reader-strip", "The strip on the message header")
    s.key("Escape")
    s.click("mac-main.reader.open")
    s.expect("strip-has-five-controls-in-order", lambda: [r["ax"].rsplit(".", 1)[1] for r in s.ctl.rows("mac-main.reader.strip")], STRIP_IDS)
    s.expect("strip-keys-come-from-the-preset", lambda: [r["key"] for r in s.ctl.rows("mac-main.reader.strip")] == [preset_key(s, c) or preset_key(s, "enter") for c in ACTIONS], True)
    s.expect("strip-keys-in-tooltips", lambda: [r["key"] in r["tooltip"] for r in s.ctl.rows("mac-main.reader.strip")], [True] * 5)
    s.expect("strip-runs-engine-commands", lambda: [r["command"] for r in s.ctl.rows("mac-main.reader.strip")], ACTIONS)
    s.expect("strip-all-apply-to-an-open-message", lambda: [r["enabled"] for r in s.ctl.rows("mac-main.reader.strip")], [True, True, True, True, True])
    s.expect("reply-all-and-forward-are-live-doors", lambda: [(r["door"], "composer round" in r["tooltip"]) for r in s.ctl.rows("mac-main.reader.strip")][2:4], [("quickreply", False), ("composer", False)])
    s.expect("strip-reveals-within-the-reveal-time", lambda: s.ctl.rows("mac-main.reader.strip")[0]["reveal_ms"], const("HOVER_REVEAL_MS"))
    s.click("mac-main.pane.list")                     # the open door puts the pane focus on the reader (one-state rule), so the list takes it back for this case
    s.expect("strip-hidden-while-the-list-holds-the-keys", lambda: s.ctl.rows("mac-main.reader.strip")[0]["visible"], False)
    s.click("mac-main.pane.reader")
    s.expect("strip-shows-when-the-pane-holds-the-keys", lambda: s.ctl.rows("mac-main.reader.strip")[0]["visible"], True)

    s.click("mac-main.reader.strip.reply")
    s.expect("strip-reply-opens-the-quick-reply", lambda: [r["thread"] for r in s.ctl.rows("mac-main.quickreply.field")] != [], True)
    s.expect("strip-greys-delete-and-reply-under-a-reply", lambda: [r["enabled"] for r in s.ctl.rows("mac-main.reader.strip")], [False, False, True, True, True])
    s.click("mac-main.reader.strip.delete")
    s.expect("greyed-delete-does-nothing", lambda: s.ctl.core("message/%s/deleted" % CODE_MSG["id"]), False)
    s.key("Escape")

    s.click("mac-main.reader.strip.archive")
    s.expect("strip-archive-archives-in-core", lambda: s.ctl.core("message/%s/archived" % CODE_MSG["id"]), True)
    s.key("Z")

    s.key("Escape")
    s.click("mac-main.reader.open")
    s.click("mac-main.reader.strip.delete")
    s.expect("strip-delete-deletes-in-core", lambda: s.ctl.core("message/%s/deleted" % CODE_MSG["id"]), True)
    s.expect("strip-delete-returns-to-list", lambda: s.ctl.uistate()["surface"], "mac-main")
    s.expect("strip-delete-offers-undo", lambda: (s.ctl.rows("mac-main.list.state")[0]["undo_available"], s.ctl.rows("mac-main.list.state")[0]["undo_words"]), (True, "Deleted"))
    s.key("Z")

    s = j.step("par-reader-header", "The icons in the header band")
    s.key("Escape")
    s.click("mac-main.reader.open")
    s.click("mac-main.reader.key.archive")
    s.expect("header-archive-archives", lambda: s.ctl.core("message/%s/archived" % CODE_MSG["id"]), True)
    s.key("Z")
    s.key("Escape")
    s.click("mac-main.reader.open")
    s.click("mac-main.reader.key.pin")
    s.expect("header-pin-pins", lambda: s.ctl.core("pin/%s" % CODE_MSG["id"]), True)
    s.click("mac-main.reader.key.pin")
    s.click("mac-main.reader.key.reply")
    s.expect("header-reply-opens-the-quick-reply", lambda: [r["thread"] for r in s.ctl.rows("mac-main.quickreply.field")] != [], True)
    s.key("Escape")
    s.click("mac-main.reader.key.snooze")
    s.expect("header-snooze-opens-the-picker", lambda: s.ctl.uistate()["overlay"], "mac-main.snooze")
    s.key("Escape")
    s.click("mac-main.reader.key.label")
    s.expect("header-label-opens-the-picker", lambda: s.ctl.uistate()["overlay"], "mac-main.label")
    s.key("Escape")
    s.click("mac-main.reader.key.more")
    s.expect("header-more-opens-the-palette", lambda: s.ctl.uistate()["overlay"] in ("mac-palette", "mac-search"), True)
    s.key("Escape")


def keys_round(j):
    """Round 5: the reader keys on the fixture's three-message thread (msg-01)."""
    s = j.step("par-reader-keys", "N, P, O, shift O step through and open the messages of a thread")
    s.key("Escape")
    to_top(s)
    s.click("mac-main.reader.open")
    st = lambda: s.ctl.rows("mac-main.reader.state")[0]
    s.expect("opened-on-the-three-message-thread", lambda: (st()["message_count"], st()["marked"]), (3, False))
    s.key("N")
    s.expect("n-at-the-last-message-stays", lambda: (st()["focused_message"], st()["marked"]), (2, False))
    s.key("P")
    s.expect("p-marks-the-message-before", lambda: (st()["focused_message"], st()["marked"], st()["earlier_open"]), (1, True, True))
    s.expect("only-the-marked-message-is-marked", lambda: [r["focused"] for r in s.ctl.rows("mac-main.reader.thread")], [False, True, False])
    s.key("P")
    s.key("P")
    s.expect("p-at-the-first-message-stays", lambda: (st()["focused_message"], s.ctl.core("pin/%s" % THREAD_MSG["id"])), (0, THREAD_MSG["pinned"]))
    s.key("N")
    s.expect("n-moves-forward", lambda: st()["focused_message"], 1)
    s.key("O")
    s.expect("o-expands-the-marked-message", lambda: st()["expanded"], [1, 2])
    s.expect("expanded-flag-on-the-row", lambda: [r["expanded"] for r in s.ctl.rows("mac-main.reader.thread")], [False, True, True])
    s.key("O")
    s.expect("o-again-folds-it", lambda: st()["expanded"], [2])
    s.key("N")
    s.key("⇧O")
    s.expect("shift-o-expands-every-message", lambda: st()["expanded"], [0, 1, 2])
    s.key("⇧O")
    s.expect("shift-o-again-folds-them-and-the-rule", lambda: (st()["expanded"], st()["earlier_open"]), ([2], False))
    s.key("Escape")
    s.expect("keys-do-nothing-in-the-list", refuses(s, "N"), True)


def wait_quiet(secs=240):
    """Poll the machine load (every 5 s) until it is at or under PERF_LOAD_MAX, for at most `secs`: the cost cases are not judged above it (main103: loads of 24 and
    28 left 7 cases not judged). A quiet machine costs nothing; a loaded one that does not calm down in `secs` still reads not judged, as before. Never in --selftest."""
    if FAST:
        return
    limit = float(os.environ.get("PERF_LOAD_MAX") or const("PERF_LOAD_MAX"))
    end = time.time() + secs
    while os.getloadavg()[0] > limit and time.time() < end:
        time.sleep(5)


def scroll_evidence(s, stamp, first, first_at):
    """For a thread that does not open at the top: the scroll offset 0.2 s (the check's own read), 0.6 s and 1.5 s after the R key, and the web view's row
    (`mac-main.reader.web`: height_reserved, measures when served). Never raises; a read the build refuses says so. Real time, except in --selftest."""
    reads = ["offset %r at %.2f s after the key (the check's read)" % (first, first_at)]
    t0 = stamp[0] if stamp else None
    for due in (0.6, 1.5):
        if t0 is not None and not FAST:
            time.sleep(max(0.0, t0 + due - time.monotonic()))
        try:
            v = s.j.ctl.rows("mac-main.reader.scroll")[0].get("offset", "absent")
        except (Unreadable, KeyError, IndexError, TypeError, AttributeError, ValueError) as e:
            v = "unreadable (%s)" % e
        reads.append("offset %r at %.2f s" % (v, (time.monotonic() - t0) if t0 is not None else 0.0))
    try:
        w = s.j.ctl.rows("mac-main.reader.web")
        w = w[0] if w else {}
        web = {f: w.get(f, "absent") for f in ("height_reserved", "measures")}
    except (Unreadable, KeyError, IndexError, TypeError, AttributeError, ValueError) as e:
        web = "unreadable (%s)" % e
    # where the pieces stand when the thread is not at its end (main92 to main100: 87 of 336, 311 of 423, 318 of 423 steady): the frames of the slab, the thread
    # column and the layout row, so the next log says whether the content the scroll row counts reaches below the slab, and whether the end is reachable
    frames = {}
    for ax in ("mac-main.quickreply.slab", "mac-main.reader.thread", "mac-main.reader.reply"):
        try:
            fr = s.j.ctl.measure(ax).get("frame")
            frames[ax] = {k: round(fr[k]) for k in ("x", "y", "w", "h")} if isinstance(fr, dict) else fr
        except Exception as e:
            frames[ax] = "unreadable (%s)" % type(e).__name__
    try:
        lay = s.j.ctl.rows("mac-main.reader.layout")
        lay = {k: lay[0].get(k) for k in ("pane_width", "summary_gap", "message_gap")} if lay else "no row"
    except Exception as e:
        lay = "unreadable (%s)" % type(e).__name__
    return "scroll after R: %s; mac-main.reader.web %s; frames %s; layout %s" % ("; ".join(reads), web, frames, lay)


def scroll_round(j):
    """Round 5: space, shift space, command up and down by a page."""
    s = j.step("par-reader-scroll", "Scroll by a page and jump to the ends")
    s.key("Escape")
    to_top(s)
    s.click("mac-main.reader.open")
    s.key("⇧O")
    sc = lambda: s.ctl.rows("mac-main.reader.scroll")[0]
    before = {}
    s._act(lambda: before.update(row=dict(sc())))         # the scroll row before R: what R moved is read against it, not assumed
    s.key("R")
    stamp = []
    s._act(lambda: stamp.append(time.monotonic()))        # the clock of the key, for the late reads of a failed "scrolls to the new reply card"
    wait(s, 200)
    s.expect("content-taller-than-the-pane", lambda: sc()["max_offset"] > 0, True)

    # main79: R opens the reply under the thread and the app scrolls the reader so the new reply card is in view, on purpose: while the reply slab
    # floats the END is the only right place (QuickReplyThreadCards.reveal: "it leaves the room clear under the last card, so every card sits above
    # the slab"). The old case asked for offset 0, the opposite of that (offset 48, 94, 53 on r76, r77, r79, steady over 1.5 s). The card's place is
    # read as the offset against max_offset (the row serves no card frame): in view means the offset has reached the end of what scrolls.
    def scrolls_to_the_new_reply_card():
        # the end is asked again after the layout settles (ReaderScroll.retryEnd: looks at 150, 400 and 800 ms), so the offset reaches it within about a second: read until
        # it does (up to 2 s), never judge the first read (main112: 323 of 423 at 0.20 s, 423 at 0.61 s and 1.51 s, red on the early read)
        end_by = time.monotonic() + (0 if FAST else 2.0)
        row = sc()
        while abs(row["offset"] - row["max_offset"]) > 1 and time.monotonic() < end_by:
            time.sleep(0.1)
            row = sc()
        got, end = row["offset"], row["max_offset"]
        at = (time.monotonic() - stamp[0]) if stamp else 0.0
        s._eg = ("offset == max_offset", "offset %r, max_offset %r" % (got, end))
        if abs(got - end) <= 1:
            return True, "offset %r at the end %r" % (got, end)
        return False, "expected the reader at the end of the thread (offset == max_offset) with the new reply card in view, got offset %r of max_offset %r (viewport %r, content %r); before R: %s; %s" % (
            got, end, row.get("viewport"), row.get("content"), before.get("row", "not read"), scroll_evidence(s, stamp, got, at))
    s._record("r-scrolls-to-the-new-reply-card", scrolls_to_the_new_reply_card)
    s.key("Escape")
    # from the top: R left the thread at its end, where Space no longer scrolls (on a mail with files it previews the first one: main146 and main147, the
    # case below then read the preview closed by the second Space)
    s.key("⌘Up")
    wait(s, 150)
    s.key("Space")
    wait(s, 150)
    s.expect("space-scrolls-down-a-page", lambda: 0 < sc()["offset"] <= sc()["viewport"], True)
    s.key("⇧Space")
    wait(s, 150)
    s.expect("shift-space-scrolls-back", lambda: sc()["offset"], 0)
    s.key("⌘Down")
    wait(s, 150)
    s.expect("command-down-jumps-to-the-bottom", lambda: sc()["offset"] == sc()["max_offset"], True)
    s.expect("space-at-the-bottom-passes-on-or-previews-the-first-file", space_at_the_bottom(s), True)
    s._act(lambda: preview_is(s, False, 0)() and press_any(s, "Escape"))   # a preview the key opened is closed: the next steps start from the mail
    s.key("⌘Up")
    wait(s, 150)
    s.expect("command-up-jumps-to-the-top", lambda: sc()["offset"], 0)
    s.expect("shift-space-at-the-top-passes-on", refuses(s, "⇧Space"), True)
    s.key("Escape")


# A worktree is a sparse checkout without `_captures` (main88: hshard-0 held no corpus): the file is the one of the main checkout two folders up.
CORPUS = next((p for p in (os.path.join(ROOT, "_captures", "reader", "r5", "links-corpus.json"),
                           os.path.join(ROOT, "..", "..", "_captures", "reader", "r5", "links-corpus.json")) if os.path.exists(p)),
              os.path.join(ROOT, "_captures", "reader", "r5", "links-corpus.json"))
LINK_RE = re.compile(r"https?://[^\s<>\"')\]]+")
MONTHS = "January|February|March|April|May|June|July|August|September|October|November|December|Jan|Feb|Mar|Apr|Jun|Jul|Aug|Sept|Sep|Oct|Nov|Dec"
DATE_RE = re.compile(r"\b(?:\d{1,2}(?:st|nd|rd|th)?\s+(?:%s)(?:\s+\d{4})?|Monday|Tuesday|Wednesday|Thursday|Friday|Saturday|Sunday)\b" % MONTHS)


def expected_items(body):
    """The links and dates of a body in reading order: the spec of the ring, written down independently of the app."""
    found = [(m.start(), "link", m.group(0).rstrip(".,;:!?")) for m in LINK_RE.finditer(body)]
    found += [(m.start(), "date", m.group(0)) for m in DATE_RE.finditer(body)]
    return [(k, t) for _, k, t in sorted(found)]


SIDE = 32           # WhooshMetrics.readerSide: the pane's trailing inset
OFFSET = 32         # the far-side offset of other people's blocks (s32, aligned by author): kept by design, measured outside the block


def gaps(s):
    """(summary callout, open message block) pane-right minus content-right, points, from the reader's layout probe."""
    r = s.ctl.rows("mac-main.reader.layout")[0]
    return [r["summary_gap"], r["message_gap"]]


def width_round(j):
    """Owner 11:58: the right part takes the full width. No dead band right of the content at any pane width."""
    s = j.step("par-reader-width", "The reader's content fills the pane at every width")
    s.key("Escape")
    to_top(s)
    s.click("mac-main.reader.open")
    for name, w, h in (("1280", "1280", "820"), ("960", "960", "600")):
        s.raw("resize", w, h)
        wait(s, 400)
        s.expect("summary-fills-to-the-inset-at-%s" % name, lambda: gaps(s)[0] <= SIDE + 1, True)
        s.expect("message-fills-to-the-offset-at-%s" % name, lambda: gaps(s)[1] <= SIDE + OFFSET + 1, True)
    s.raw("resize", "1280", "820")
    s.click("mac-main.sidebar.toggle")
    wait(s, 600)
    s.expect("summary-fills-to-the-inset-sidebar-hidden", lambda: gaps(s)[0] <= SIDE + 1, True)
    s.expect("message-fills-to-the-offset-sidebar-hidden", lambda: gaps(s)[1] <= SIDE + OFFSET + 1, True)
    s.expect("pane-is-wider-with-the-sidebar-hidden", lambda: s.ctl.rows("mac-main.reader.layout")[0]["pane_width"] > 700, True)
    s.click("mac-main.sidebar.toggle")
    wait(s, 300)


def links_round(j):
    """Round 5: Tab through the links and dates, Enter opens, command O opens the links and attachments."""
    msgs = json.load(open(CORPUS, encoding="utf-8"))["messages"]
    last = sorted(msgs, key=lambda m: m["date"])[-1]                    # the newest message is row 0
    items = expected_items(last["body"])
    first_link = [t for k, t in items if k == "link"][0]
    s = j.step("par-reader-links", "Tab rings the links and dates of the open message")
    s.key("Escape")
    s.raw("fixture", "corpus", CORPUS)
    to_top(s)
    s.click("mac-main.reader.open")
    ring = lambda: [(r["kind"], r["text"], r["ring"]) for r in s.ctl.rows("mac-main.reader.links")]
    s.expect("no-ring-before-tab", lambda: [r["ring"] for r in s.ctl.rows("mac-main.reader.links")], [])
    s.key("Tab")
    s.expect("tab-lists-the-links-and-dates-in-reading-order", lambda: [(k, t) for k, t, _ in ring()], items)
    s.expect("tab-puts-the-ring-on-the-first", lambda: [r for _, _, r in ring()], [i == 0 for i in range(len(items))])
    s.key("Tab")
    s.expect("tab-moves-the-ring-on", lambda: [r for _, _, r in ring()], [i == 1 for i in range(len(items))])
    for _ in range(len(items) - 2):
        s.key("Tab")
    s.key("Tab")
    s.expect("tab-after-the-last-takes-the-ring-off", lambda: [r for _, _, r in ring()], [False] * len(items))
    s.key("Tab")
    s.expect("tab-then-starts-again", lambda: ring()[0][2], True)
    s.key("Enter")
    s.expect("enter-opens-the-ringed-link", lambda: [(r["kind"], r["target"]) for r in s.ctl.rows("mac-main.reader.opened")], [("browser", first_link)])
    s.expect("the-fixture-logs-and-starts-no-browser", lambda: [r["done"] for r in s.ctl.rows("mac-main.reader.opened")], [False])
    s.key("Tab")
    s.key("Enter")
    s.expect("enter-on-a-date-is-logged", lambda: s.ctl.rows("mac-main.reader.opened")[-1]["kind"], "calendar")
    s.key("Escape")
    s.click("mac-main.reader.open")
    s.key("⌘O")
    s.expect("command-o-opens-the-first-link-and-the-attachments",
             lambda: [(r["kind"], r["target"]) for r in s.ctl.rows("mac-main.reader.opened")][2:],
             [("browser", first_link)] + [("quicklook", a) for a in last.get("attachments", [])])
    s.key("Tab")
    s.click("mac-main.reader.link.1")
    s.expect("a-chip-click-opens-that-item", lambda: s.ctl.rows("mac-main.reader.opened")[-1]["kind"], "calendar")
    s.key("Escape")
    s.key("Escape")


def focus_round(j):
    """Round 5: light focus. A passing focus costs the reader nothing; the settled one follows and cross-fades."""
    settle = const("FOCUS_SLIDE_MS")
    s = j.step("par-reader-focus", "The reader follows the list focus once the keys settle")
    s.key("Escape")
    to_top(s)
    wait(s, settle + 100)
    shown = lambda: s.ctl.rows("mac-main.reader.shown")[0]
    s.key("J")
    s.expect("a-move-leaves-the-reader-on-the-old-thread", lambda: (shown()["pending"], shown()["shown"] == ROWS[0]["id"], shown()["focused"] == ROWS[1]["id"]), (True, True, True))
    s.expect("the-old-thread-is-still-on-screen", lambda: s.ctl.rows("mac-main.reader.subject")[0]["subject"], ROWS[0]["subject"])
    wait(s, settle + 100)
    s.expect("the-settled-move-is-followed", lambda: (shown()["pending"], shown()["shown"]), (False, ROWS[1]["id"]))
    s.expect("and-draws-the-new-thread", lambda: s.ctl.rows("mac-main.reader.subject")[0]["subject"], ROWS[1]["subject"])
    s.expect("settle-and-cross-fade-times-are-the-constants", lambda: (shown()["settle_ms"], shown()["crossfade_ms"]), (settle, const("READER_CROSSFADE_MS")))

    s = j.step("par-reader-focus-cost", "Thirty fast presses build one thread")
    s._act(wait_quiet)                              # a timing is judged only on a quiet machine: wait (measured, up to 4 minutes) until the load is under the limit
    load_before = os.getloadavg()[0]
    s.click("mac-main.reader.perf.start")
    sent25 = []                                     # when each press was sent: the counts are judged only if the presses stayed inside one settle
    for k in ["J"] * 15 + ["K"] * 10:
        s._act(lambda c: (sent25.append(time.monotonic()), s.ctl.key(c)), k)
        wait(s, 10)
    wait(s, settle + 250)
    s.click("mac-main.reader.perf.stop")
    load25 = max(load_before, os.getloadavg()[0])
    cost = lambda: s.ctl.rows("mac-main.reader.perf")[0]

    def fast25(read):
        """`read()` of the cost meter, only when the presses really came faster than FOCUS_SLIDE_MS and the machine load is at or under PERF_LOAD_MAX
        (main92: load 17 stretched the gaps past the settle, so every press settled and built its thread); else not judged, with the reason."""
        def f():
            limit = float(os.environ.get("PERF_LOAD_MAX") or const("PERF_LOAD_MAX"))
            if load25 > limit:
                raise Unreadable("machine load %.2f is above PERF_LOAD_MAX %s: the settle timing cannot be judged" % (load25, limit))
            gaps = [(b - a) * 1000 for a, b in zip(sent25, sent25[1:])]
            if gaps and max(gaps) >= settle:
                raise Unreadable("a press gap of %.0f ms reached FOCUS_SLIDE_MS %s: the presses did not stay inside one settle" % (max(gaps), settle))
            return read()
        return f
    s.expect("25-presses-build-at-most-two-threads", fast25(lambda: cost()["makes"] <= 2), True)
    s.expect("25-presses-cross-fade-once", fast25(lambda: cost()["swaps"]), 1)
    s.expect("25-presses-load-no-body", lambda: cost()["body_loads"], 0)
    s.expect("25-presses-evaluate-the-pane-at-most-twice", fast25(lambda: cost()["pane_evals"] <= 2), True)

    s = j.step("par-reader-focus-open", "In an open reader the body loads once, after the keys settle")
    s.click("mac-main.reader.open")
    wait(s, settle + 200)
    s._act(wait_quiet)                              # a timing is judged only on a quiet machine: wait (measured, up to 4 minutes) until the load is under the limit
    load_before = os.getloadavg()[0]
    s.click("mac-main.reader.perf.start")
    sent = []                                       # when each press was sent: the step is judged only if they stayed fast
    for k in ["J"] * 8 + ["K"] * 3:
        s._act(lambda c: (sent.append(time.monotonic()), s.ctl.key(c)), k)
        wait(s, 10)
    wait(s, settle + 250)
    # The counters are read BEFORE the stop click: any socket click builds the click tables, and the attachments lane's
    # table reads the reader's thread with its body (AttachmentSocket.clicks, AttachmentSet.reader), one load the presses
    # did not cause (seen on main43: 1 load after the settle, 2 after the stop click).
    window = {}
    s._act(lambda: window.update(cost()))
    s.click("mac-main.reader.perf.stop")
    load = max(load_before, os.getloadavg()[0])

    def gated_loads():
        """A settle count is judged only when the presses really came faster than the settle time (FOCUS_SLIDE_MS) and the
        machine load is at or under PERF_LOAD_MAX (as perf/interaction.py); else it is not judged, with the reason."""
        limit = float(os.environ.get("PERF_LOAD_MAX") or const("PERF_LOAD_MAX"))
        if load > limit:
            raise Unreadable("machine load %.2f is above PERF_LOAD_MAX %s: the settle timing cannot be judged" % (load, limit))
        gaps = [(b - a) * 1000 for a, b in zip(sent, sent[1:])]
        if not gaps or max(gaps) >= settle:
            raise Unreadable("a press gap of %.0f ms reached FOCUS_SLIDE_MS %s: the presses did not stay inside one settle" % (max(gaps or [0]), settle))
        if "body_loads" not in window:
            raise Unreadable("the cost meter was not read before the stop click")
        return window["body_loads"]
    s.expect_le("11-presses-in-an-open-reader-load-one-body", gated_loads, 1)
    s.expect("the-open-reader-follows-the-settled-focus", lambda: s.ctl.rows("mac-main.reader.shown")[0]["shown"], ROWS[11]["id"])   # 11 presses from the top land on the 12th served row (the rejected flow row stands third)
    s.key("Escape")


def nav_round(j):
    """A129 part 1: a selected mail shows its whole body, so the settled thread reads one body (by the card: body_reads, never by a build:
    body_loads) and a passing press reads none; then twenty J and K moves, each given time to settle, keep every main-thread turn under
    the frame budget (the key frame carries nothing heavy). Timing is NOT JUDGED above PERF_LOAD_MAX."""
    settle = const("FOCUS_SLIDE_MS")
    s = j.step("par-reader-a129-nav", "Twenty J and K moves read their bodies off the key frame")
    s.key("Escape")
    to_top(s)
    wait(s, settle + 100)
    s.click("mac-main.reader.perf.start")
    for k in ["J"] * 15 + ["K"] * 10:
        s.key(k)
        wait(s, 10)
    wait(s, settle + 250)
    burst = {}
    s._act(lambda: burst.update(s.ctl.rows("mac-main.reader.perf")[0]))      # read BEFORE the stop click (a click builds the click tables)
    s.click("mac-main.reader.perf.stop")

    def burst_reads():
        if "body_reads" not in burst:
            raise Unreadable("the cost meter was not read before the stop click")
        return burst["body_reads"]
    s.expect_le("25-presses-read-at-most-one-body", burst_reads, 1)
    s.expect("25-presses-build-no-body", lambda: burst["body_loads"], 0)

    s._act(wait_quiet)                              # a timing is judged only on a quiet machine: wait (measured, up to 4 minutes) until the load is under the limit
    load_before = os.getloadavg()[0]
    s.click("mac-main.reader.perf.start")
    phases = []                                               # per move: the key frame's phases (`measure mac-main.interaction.phases`, docs/lanes/perf-archive.md (c))

    def read_phases(k):
        try:
            phases.append((k, dict((s.ctl.measure("mac-main.interaction.phases") or {}).get("phases") or {})))
        except Exception as e:                               # a build that does not serve the measure: said in the line, never hidden
            phases.append((k, "not served (%s: %s)" % (type(e).__name__, e)))
    for k in ["J"] * 12 + ["K"] * 8:                          # twenty moves, each one allowed to settle: twenty bodies to show
        s.key(k)
        wait(s, settle + 80)
        s._act(read_phases, k)
    paced = {}
    s._act(lambda: paced.update(s.ctl.rows("mac-main.reader.perf")[0]))
    s.click("mac-main.reader.perf.stop")
    load = max(load_before, os.getloadavg()[0]) if not FAST else 0

    def judged(field):
        limit = float(os.environ.get("PERF_LOAD_MAX") or const("PERF_LOAD_MAX"))
        if load > limit:
            raise Unreadable("machine load %.2f is above PERF_LOAD_MAX %s: the frame budget cannot be judged" % (load, limit))
        if field not in paced:
            raise Unreadable("the cost meter was not read before the stop click")
        return paced[field]
    def over_frame_with_phases():
        n = judged("over_frame")
        if n == 0:
            return n
        # where the time goes, printed with the failure (the budget is not touched): the meter's own totals, the key frame's phases of every move
        # and their median per phase. A turn the meter counts that the phases do not hold is time AFTER the key frame (the settle: the body's build).
        got = [p for _, p in phases if isinstance(p, dict)]
        names = sorted({n2 for p in got for n2 in p})
        med = {n2: sorted(p.get(n2, 0) for p in got)[len(got) // 2] for n2 in names} if got else "no phases read"
        meter = {k: paced.get(k) for k in ("turns", "over_frame", "p90_ms", "max_ms", "slow_ms", "slow_tags", "busy_ms", "frame_ms", "makes", "make_ms", "body_reads", "body_loads", "pane_evals", "swaps")}
        raise IndexError("got %r, expected 0 | meter %s | median phase ms over %d moves %s | per move %s" % (
            n, meter, len(got), med, "; ".join("%s %s" % (k, p if not isinstance(p, dict) else {a: b for a, b in p.items() if b}) for k, p in phases)))
    s.expect("20-moves-keep-every-turn-under-the-frame-budget", over_frame_with_phases, 0)
    s.expect_le("20-moves-read-one-body-each-at-most", lambda: judged("body_reads"), 20)
    s.expect_le("20-moves-evaluate-the-pane-once-each-at-most", lambda: judged("pane_evals"), 20)

    def rows_rebuilt():
        # Owner, 8 Nov: "navigation with arrows is WAYYYY TOO SLOW AND HEAVY". A one-row move changes two rows (the one giving the focus, the one taking it); each
        # may be built twice before the frame (the move, then its settle) and the scroll may draw one or two rows that were off screen: 6 at most. main180 read 33,
        # every row on screen, because the highlight's animation kind sat in every row's key. A count, not a timing: judged at any machine load.
        got = [p.get("InboxRowView.body (count)") for _, p in phases if isinstance(p, dict)]
        counted = [v for v in got if isinstance(v, (int, float))]
        if len(counted) < 20:
            raise Unreadable("the rows built in the key frame were read for %d of 20 moves" % len(counted))
        return max(counted)
    s.expect_le("20-moves-rebuild-only-the-rows-that-change", rows_rebuilt, 6)


class NavModel:
    """The reader's cost meter on the wire for --selftest (nav_round): the first metered run is the burst of twenty-five presses (one
    settle), the second the twenty paced moves (a settle each)."""
    def __init__(self, mutant=None):
        self.m, self.runs, self.on, self.presses = mutant, 0, False, 0

    def send(self, verb, *args):
        return {}

    def key(self, chord):
        if chord in ("J", "K") and self.on:
            self.presses += 1
        return {}

    def click(self, ax):
        if ax == "mac-main.reader.perf.start":
            self.runs, self.on, self.presses = self.runs + 1, True, 0
        elif ax == "mac-main.reader.perf.stop":
            self.on = False
        return {}

    def rows(self, ax):
        if ax != "mac-main.reader.perf":
            raise Unreadable("the model has no region %s" % ax)
        settles = 1 if self.runs == 1 else self.presses
        reads = {"press-reads-a-body": self.presses, "press-reads-a-body-twice": 2 * self.presses + 1}.get(self.m, settles)
        return [{"body_reads": reads,
                 "body_loads": 3 if self.m == "build-loads-the-body" else 0,
                 "over_frame": self.presses if self.m == "key-frame-builds-the-thread" else 0,
                 "frame_ms": 16, "pane_evals": 2 * self.presses + 1 if self.m == "pane-evaluates-twice" else settles}]

    def measure(self, what):
        if what != "mac-main.interaction.phases":
            raise Unreadable("the model has no measure %s" % what)
        return {"phases": {"InboxRowView.body (count)": 33 if self.m == "move-rebuilds-every-row" else 2}}

    def uistate(self):
        return {"surface": "mac-main"}


class ScrollModel:
    """The reader's scroll on the wire for --selftest (scroll_round): R opens the reply and scrolls to the end that the reply's room makes (OPEN_MAX),
    a page per Space, the ends by command; mutant `opens-scrolled` leaves the thread scrolled by 94 after R (short of the end), `stays-on-top` leaves
    it at 0 (the old expectation, now a fault), `settles-back` shows the end at the check's read and 0 on the later reads. The stand-in drops the
    offset when the reply closes so the later steps start from the top as before (the real app keeps it; they pass from 53 as well: r79 log)."""
    MAX, VIEW, OPEN_MAX = 400, 100, 40

    def __init__(self, mutant=None):
        self.m, self.off, self.reads, self.reply = mutant, 0, 0, False

    def send(self, verb, *args):
        return {}

    def key(self, chord):
        if chord == "R":
            self.off = 94 if self.m == "opens-scrolled" else (0 if self.m == "stays-on-top" else self.OPEN_MAX)
            self.reads, self.reply = 0, True
        elif chord == "Escape":
            if self.reply:
                self.reply, self.off = False, 0
        elif chord == "Space":
            if self.off >= self.MAX:
                raise Unreadable("key Space: the key Space does nothing on mac-main")
            self.off = min(self.off + self.VIEW, self.MAX)
        elif chord == "⇧Space":
            if self.off <= 0:
                raise Unreadable("key ⇧Space: the key ⇧Space does nothing on mac-main")
            self.off = max(self.off - self.VIEW, 0)
        elif chord == "⌘Down":
            self.off = self.MAX
        elif chord == "⌘Up":
            self.off = 0
        return {}

    def click(self, ax):
        return {}

    def rows(self, ax):
        if ax == "mac-main.reader.scroll":
            self.reads += 1
            off = (self.OPEN_MAX if self.reads > 2 else 94) if (self.m == "settles-back" and self.reply) else self.off
            return [{"offset": off, "max_offset": self.OPEN_MAX if self.reply else self.MAX, "viewport": self.VIEW, "content": 500}]
        if ax == "mac-main.reader.web":
            return [{"height_reserved": 640, "measures": 2}]
        raise Unreadable("the model has no region %s" % ax)

    def uistate(self):
        return {"surface": "mac-main"}


def selftest_scroll():
    """scroll_round on the stand-in: green clean, `r-scrolls-to-the-new-reply-card` red on its mutants with the late reads and the web row in its failure
    line, and the same line on a build that serves no web row (nothing raises). Returns the number of problems."""
    bad = 0
    def run(mutant):
        j = Journey(ID, ScrollModel(mutant))
        scroll_round(j)
        return {c.label.split("/", 1)[1]: c for c in j.checks()}
    clean = run(None)
    red = [n for n, c in clean.items() if c.state != "pass"]
    print("scroll stand-in, clean: %d checks, not green %s" % (len(clean), red))
    bad += len(red)
    for mutant in ("opens-scrolled", "stays-on-top", "settles-back"):
        got = run(mutant)
        c = got["r-scrolls-to-the-new-reply-card"]
        print("scroll stand-in, mutant %-14s r-scrolls-to-the-new-reply-card %s%s" % (mutant, c.state, ("" if c.state == "pass" else ": " + c.detail)))
        if c.state != "fail":
            bad += 1
        if c.state == "fail" and ("max_offset 40" not in c.detail or "before R: {'offset': 0, 'max_offset': 400" not in c.detail):
            print("EVIDENCE MISSING: the end and the scroll row before R are not in the line")
            bad += 1
        if mutant == "settles-back" and "offset 40 at " not in c.detail:
            print("EVIDENCE MISSING: the late reads are not in the line")
            bad += 1
        if c.state == "fail":
            for need in ("scroll after R: offset 94 at ", "s after the key (the check's read)", "mac-main.reader.web {'height_reserved': 640, 'measures': 2}"):
                if need not in c.detail and mutant == "opens-scrolled":
                    print("EVIDENCE MISSING: %s" % need)
                    bad += 1
    # a failing read on a build that serves no web row: the line says so and nothing raises
    m = ScrollModel("opens-scrolled")
    m.rows = lambda ax, _r=ScrollModel.rows: (_ for _ in ()).throw(Unreadable("no rows at " + ax)) if ax == "mac-main.reader.web" else _r(m, ax)
    j = Journey(ID, m)
    scroll_round(j)
    c = next(c for c in j.checks() if c.label.endswith("/r-scrolls-to-the-new-reply-card"))
    print("scroll stand-in, no web row: %s" % c.detail)
    if c.state != "fail" or "mac-main.reader.web unreadable" not in c.detail:
        print("EVIDENCE MISSING: a build with no web row is not read as unreadable")
        bad += 1
    return bad


def selftest_o_evidence():
    """`o-on-the-open-message-passes-on` on the mutant where the app takes O on a mail with no fold: red, and the line carries the key's reply and the
    reader's state before and after. Clean it passes. Prints the `evidence on red` line; returns the number of problems."""
    bad, ev = 0, 0
    for mutant in (None, "o-taken-without-a-fold"):
        j = Journey(ID, OModel(mutant))
        o_round(j)
        c = next(c for c in j.checks() if c.label.endswith("/o-on-the-open-message-passes-on"))
        if mutant is None:
            bad += c.state != "pass"
            continue
        ok = c.state == "fail" and all(w in c.detail for w in ("the app took O, reply ", "before: ", "after: ", "mac-main.reader.state {'message_count': 1", "'earlier_open': True", "mac-main.reader.body {'fold_open': True", "shell focus pane 'reader'"))
        ev += ok
        bad += not ok
        print("O stand-in, mutant %s: %s%s" % (mutant, c.state, "" if ok else " EVIDENCE MISSING: " + c.detail))
    print("evidence on red (par-reader): %d of 1 evidence case prints the key's reply and the reader state before and after on its mutant" % ev)
    return bad


def colours_round(j):
    """A129 ruling: shift I is the list's "important" filter while the LIST holds the focus and the colour switch of the open mail while the
    READER pane holds it (keys are contextual). Each state takes its own command and leaves the other's effect alone."""
    s = j.step("par-reader-a129-colours", "Shift I filters in the list and switches colours in the reader")
    colours = lambda: s.ctl.rows("mac-main.reader.body")[0]["colours"]
    box = {}
    s.key("Escape")
    s._act(lambda: box.update(start=colours()))
    s.key("⇧I")
    s.expect("list-focus-shift-i-filters", lambda: last_dispatch(s), ("filter-important", "ran"))
    s.expect("list-focus-shift-i-leaves-the-colours", lambda: colours() == box["start"], True)
    s.key("⇧I")                                       # the filter toggles back
    s.click("mac-main.reader.open")                   # READER: the mail is open, the reply field is not focused (the click gives it no keys)
    s.expect("foot-strip-in-the-open-mail-is-the-reader-set", lambda: s.ctl.rows("mac-main.foot.hints")[0]["surface"], "reader")
    s.click("mac-main.pane.reader")
    s.key("⇧I")
    s.expect("reader-focus-shift-i-switches", lambda: last_dispatch(s), ("toggle-colours", "ran"))
    s.expect("reader-focus-shift-i-changes-the-colours", lambda: colours() != box["start"], True)
    s.key("⇧I")
    s.expect("reader-focus-shift-i-again-restores-them", lambda: colours() == box["start"], True)
    s.key("Escape")
    s.key("Escape")
    s.key("Enter")                                    # Return opens the reader with the pane focus still on the list (found in A129d)
    s.key("Escape")
    s.key("⇧I")
    s.expect("return-opened-reader-shift-i-switches", lambda: last_dispatch(s), ("toggle-colours", "ran"))
    s.key("Escape")
    s.key("Enter")                                    # a stale reader pane focus (set by an earlier click) must not make the list's shift I switch colours
    s.key("Escape")
    s.key("Escape")
    s.click("mac-main.pane.reader")
    s.key("Escape")                                   # back to the list: the pane focus is the reader's no longer, the surface is the list
    s.key("⇧I")
    s.expect("shift-i-in-the-list-filters-whatever-the-pane-focus-was", lambda: last_dispatch(s), ("filter-important", "ran"))
    s.expect("foot-strip-after-return-and-escape-is-the-list-set", lambda: s.ctl.rows("mac-main.foot.hints")[0]["surface"], "list")


class ColoursModel:
    """The key chain's answer to shift I for --selftest (colours_round)."""
    def __init__(self, mutant=None):
        self.m, self.pane, self.colours, self.log, self.opened, self.reply = mutant, "list", "app", [], False, False

    def send(self, verb, *args):
        return {}

    def click(self, ax):
        if ax == "mac-main.reader.open":
            self.opened, self.reply = True, self.m == "click-focuses-the-reply-field"   # the door enters READER and does not touch the pane focus
        if ax == "mac-main.pane.reader":
            self.pane = "reader"
        return {}

    def key(self, chord):
        if chord == "Enter" and not self.opened:
            self.opened, self.reply = True, True
            return {}
        if chord == "Escape":
            if self.reply:
                self.reply = False
            else:
                self.opened = False                    # READER goes to LIST and the pane focus goes with it (the mutant leaves it where the last click put it)
                if self.m != "stale-pane-focus":
                    self.pane = "list"
            return {}
        if chord == "⇧I":
            if self.reply:
                return {}                              # the reply field holds the keyboard: the letter types
            reader = self.opened and self.m != "needs-the-pane-focus"
            if self.m == "stale-pane-focus":
                reader = self.pane == "reader"
            if (reader and self.m != "reader-filters" and not (self.m == "reader-one-way" and self.colours == "original")) or (not reader and self.m == "list-switches-colours"):
                self.colours = "original" if self.colours == "app" else "app"
                self.log.append(("toggle-colours", "ran"))
            else:
                self.log.append(("filter-important", "ran"))
        return {}

    def rows(self, ax):
        if ax == "mac-shortcuts.dispatch":
            return [{"command": c, "outcome": o} for c, o in self.log] or [{"command": "-", "outcome": "-"}]
        if ax == "mac-main.reader.body":
            return [{"colours": self.colours}]
        if ax == "mac-main.foot.hints":
            if self.m == "field-focus-stays" and not self.opened:
                return [{"surface": "composer"}]
            if self.m == "bar-reads-the-pane":
                return [{"surface": "reader" if self.pane == "reader" else "list"}]
            return [{"surface": "composer" if self.reply else ("reader" if self.opened else "list")}]
        raise Unreadable("the model has no region %s" % ax)


NO_FOLD_ROW = next(i for i, m in enumerate(ROWS) if m["id"] == "msg-04")    # one message, no quoted part: nothing to fold
FOLD_ROW = 0                                                                   # msg-01: the dots hold its quoted part


SRC_FEATURE = os.path.join(ROOT, "Sources", "Features", "MessageBody", "MessageBodyFeature.swift")
SRC_KEYS = os.path.join(ROOT, "Sources", "Features", "Reader", "ReaderKeys.swift")


def dots_sources():
    """(the mail view's key table, ReaderKeys.expand's text): the dots path. A stand-in replaces this in --selftest."""
    feature = open(SRC_FEATURE, encoding="utf-8").read()
    keys = open(SRC_KEYS, encoding="utf-8").read()
    a = keys.index("static func expand(")
    return feature, keys[a:keys.index("static func expandAll", a)]


def dots_follow_the_preset(feature, expand):
    """No literal O chord in the dots path, and the dots are reached from the `expand-message` handler."""
    table = feature[feature.index("keyBindings()"):feature.index("static func", feature.index("keyBindings()") + 20)]
    return ('"O"' not in table and '"O"' not in expand, "toggleFold" in expand)


def reader_reads(s):
    """Where the reader stands, in the fields that decide an O: the thread state (message count, focused, marked, expanded) and the body's dots
    (fold_open, dots_key). A read the build refuses says so; never raises."""
    out = []
    for ax, fields in (("mac-main.reader.state", ("message_count", "focused_message", "marked", "expanded", "earlier_open")),
                       ("mac-main.reader.body", ("fold_open", "dots_key", "text_above_y"))):
        try:
            r = s.j.ctl.rows(ax)[0]
            out.append("%s %s" % (ax, {f: r.get(f, "absent") for f in fields}))
        except (Unreadable, KeyError, IndexError, TypeError, AttributeError, ValueError) as e:
            out.append("%s unreadable (%s)" % (ax, e))
    try:
        out.append("shell focus pane %r" % (s.j.ctl.rows("mac-main.shell")[0].get("focused_pane", "absent"),))
    except (Unreadable, KeyError, IndexError, TypeError, AttributeError, ValueError) as e:
        out.append("shell unreadable (%s)" % e)
    return "; ".join(out)


def o_passes_on(s):
    """A129e: O on an open mail WITHOUT a fold passes on (the socket answers ok:false). (ok, detail); on a red the detail carries the key's reply and
    the reader's state before and after, so the two specs the dossier set against each other (the reader check's "passes on", the mail view's
    "O is the open message's dots") are told apart by what the app did: a reply with `ran`, and a fold that opened or not."""
    if s.j.ctl is None:
        raise Unreadable(s.j.dry_reason or "dry run: no socket")
    before = reader_reads(s)
    try:
        reply = s.j.ctl.key("O")
    except Unreadable as e:
        s._eg = (True, True)
        return True, "the socket refused O: %s" % e
    s._eg = (True, False)
    return False, "expected True (O passes on), got False: the app took O, reply %r | before: %s | after: %s" % (reply, before, reader_reads(s))


def o_round(j):
    """A129e (lead's ruling): O on an open mail that HAS a fold toggles the three dots and the written text does not move; on an open mail WITHOUT
    a fold it passes on (the key falls to 'expand message', which refuses the open message). The pill carries the preset's key."""
    s = j.step("par-reader-o", "O is the three dots on a folded mail and passes on on a mail without a fold")
    body = lambda: s.ctl.rows("mac-main.reader.body")[0]
    box = {}
    s.key("Escape")
    to_top(s)
    focus_row(s, NO_FOLD_ROW)
    s.click("mac-main.reader.open")
    s._record("o-on-the-open-message-passes-on", lambda: o_passes_on(s))
    s.key("Escape")
    to_top(s)
    focus_row(s, FOLD_ROW)
    s.click("mac-main.reader.open")
    wait(s, 600)                                      # the page loads and measures where the written text ends; the Escape that used to sit here gave it this time
    s._act(lambda: box.update(y=body()["text_above_y"]))
    s.expect("the-dots-key-is-the-presets-expand-message-key", lambda: body()["dots_key"], preset_key(s, "expand-message"))
    s.key("O")
    wait(s, 200)
    s.expect("o-on-a-folded-mail-toggles-the-dots", lambda: body()["fold_open"], True)
    s.expect("o-on-a-folded-mail-leaves-the-text-where-it-is", lambda: 0 if abs(body()["text_above_y"] - box["y"]) <= 1 else "text_above_y before O %s, after O %s" % (box["y"], body()["text_above_y"]), 0)
    s.key("O")
    s.expect("o-again-folds-the-dots", lambda: body()["fold_open"], False)
    s.key("Escape")
    s.key("Escape")
    s.expect("the-dots-path-has-no-literal-o-chord", lambda: dots_follow_the_preset(*dots_sources()), (True, True))


class OModel:
    """The reader's O on the wire for --selftest (o_round)."""
    def __init__(self, mutant=None):
        self.m, self.row, self.opened, self.reply, self.fold = mutant, 0, False, False, False

    def send(self, verb, *args):
        return {}

    def click(self, ax):
        if ax == "mac-main.reader.open":
            self.opened, self.reply, self.fold = True, self.m == "click-focuses-the-reply-field", False
        return {}

    def key(self, chord):
        if chord == "J":
            self.row += 1
        elif chord == "K":
            self.row = max(0, self.row - 1)
        elif chord == "Escape":
            if self.reply:
                self.reply = False
            else:
                self.opened = False
        elif chord == "O":
            has_fold = self.row == FOLD_ROW
            if self.m == "o-taken-without-a-fold":
                has_fold = True
            if not (self.opened and has_fold) or self.reply or self.m == "o-never-toggles":   # the reply field holds the keyboard: the letter types
                raise Unreadable("key O: not taken")
            if self.fold and self.m == "o-only-opens":
                return {}
            self.fold = not self.fold
        return {}

    def rows(self, ax):
        if ax == "mac-main.reader.body":
            moved = 20 if (self.fold and self.m == "o-moves-the-text") else 0
            return [{"fold_open": self.fold, "text_above_y": 100 + moved, "dots_key": "" if self.m == "pill-key-literal" else "O"}]
        if ax == "mac-shortcuts.preset":
            return [{"command": "expand-message", "key": "O"}]
        if ax == "mac-main.reader.state":
            return [{"message_count": 1 if self.row != FOLD_ROW else 3, "focused_message": 0, "marked": False, "expanded": [0], "earlier_open": self.fold}]
        if ax == "mac-main.shell":
            return [{"focused_pane": "reader" if self.opened else "list"}]
        raise Unreadable("the model has no region %s" % ax)

    def uistate(self):
        return {"surface": "mac-reader" if self.opened else "mac-main"}


HEADER = "mac-main.reader.header"
HEADER_KEY_COMMAND = "toggle-message-header"
DATE_WORDS = re.compile(r"^\w+, \w+ \d{1,2}, \d{4} at \d{2}:\d{2} (?:[A-Z]{2,5}|GMT[+-]\d{1,2}(?::\d{2})?)$")
LISTS = {m["id"]: (i, m) for i, m in enumerate(ROWS) if m.get("list_unsubscribe")}


def account_colour(m):
    """The fixture's colour for a message's account (accounts[].colour), never a built screen."""
    return next(a["colour"] for a in FX["accounts"] if a["id"] == m["account"])


def header_rows(s):
    return s.ctl.rows(HEADER)


def has_frame(s, ax):
    """True when the socket's measure finds a frame for the id (a .whooshAx id); a refusal is False, not 'not judged'."""
    def thunk():
        try:
            return bool(s.ctl.measure(ax))
        except Unreadable:
            return False
    return thunk


def collapse_headers(s):
    """Start a header step from the app's default: the reader keeps a thread's open headers while the thread stays shown (ReaderState.headerThread,
    SPEC A70: only ANOTHER thread starts collapsed), and the scroll step's shift O leaves every header of msg-01 open. The default itself is checked
    once, on the first open of a thread no step has touched (par-reader-facts/headers-start-collapsed-on-first-open)."""
    try:
        for r in header_rows(s):
            if r.get("open"):
                s.click("mac-main.reader.header.%d" % r["index"])
    except Exception:                                                # noqa: BLE001  dry run: nothing is served
        pass


def open_row(s, n):
    """The reader opened on list row n: Escape to the list, K to the top, J n times, then the reader's own open."""
    s.key("Escape")
    to_top(s)
    focus_row(s, n)
    s.click("mac-main.reader.open")


def header_round(j):
    """Round 6 (SPEC A70): each message's header expands and collapses, by its chevron or by the preset's key."""
    s = j.step("par-reader-header-toggle", "Each message's header expands and collapses")
    m = THREAD_MSG
    open_row(s, 0)
    collapse_headers(s)
    s.expect("header-account-colour-is-the-list-rows", lambda: (set(r["account_colour"] for r in header_rows(s)), [r["account_colour"] for r in s.ctl.rows("mac-main.list") if r["id"] == m["id"]]), ({account_colour(m)}, [account_colour(m)]))
    s.expect("chevron-on-every-message", lambda: [r["chevron"] for r in header_rows(s)], [True] * m["thread_count"])
    s.expect("every-header-starts-collapsed", lambda: [r["open"] for r in header_rows(s)], [False] * m["thread_count"])   # after collapse_headers: guards the probe and the collapse path
    s.expect("the-preset-binds-shift-h", lambda: preset_key(s, HEADER_KEY_COMMAND), "⇧H")
    s.expect("tooltip-says-expand-with-the-key", lambda: header_rows(s)[-1]["tooltip"] == "Expand message header  " + str(preset_key(s, HEADER_KEY_COMMAND)), True)   # the expected text is read inside the thunk: a dry run (no socket) counts the check instead of crashing here
    s.click("mac-main.reader.header.%d" % (m["thread_count"] - 1))
    s.expect("chevron-expands-only-its-message", lambda: [r["open"] for r in header_rows(s)], [False] * (m["thread_count"] - 1) + [True])
    s.expect("expanded-rows-are-from-to-date", lambda: [r["label"] for r in header_rows(s)[-1]["rows"]], ["From", "To", "Date", "Attachments"])
    s.expect("from-is-name-and-address", lambda: header_rows(s)[-1]["rows"][0]["value"], "%s <%s>" % (m["sender"], m["from"]))
    s.expect("to-lists-every-recipient", lambda: header_rows(s)[-1]["rows"][1]["value"], ", ".join(m["to"]))
    s.expect("cc-slot-waits-for-its-field", lambda: (header_rows(s)[-1]["cc_served"], header_rows(s)[-1]["cc_reason"], "Cc" in [r["label"] for r in header_rows(s)[-1]["rows"]]), (False, "no cc field yet (contract delta 12)", False))
    s.expect("date-is-full-with-time-and-zone", lambda: bool(DATE_WORDS.match(header_rows(s)[-1]["rows"][2]["value"])), True)
    s.expect("tooltip-says-collapse-with-the-key", lambda: header_rows(s)[-1]["tooltip"], "Collapse message header  ⇧H")
    s.key("P")
    s.key("⇧H")
    s.expect("shift-h-expands-the-marked-message", lambda: [r["open"] for r in header_rows(s)], [False] * (m["thread_count"] - 2) + [True, True])
    s.key("⇧H")
    s.expect("shift-h-again-collapses-it-and-keeps-the-other", lambda: [r["open"] for r in header_rows(s)], [False] * (m["thread_count"] - 1) + [True])
    s.click("mac-main.reader.header.%d" % (m["thread_count"] - 1))
    s.expect("chevron-again-collapses", lambda: [r["open"] for r in header_rows(s)], [False] * m["thread_count"])
    s.expect("chevron-has-a-frame-for-the-key-overlay", has_frame(s, "mac-main.reader.header.%d" % (m["thread_count"] - 1)), True)
    s.expect("strip-control-has-a-frame-for-the-key-overlay", has_frame(s, "mac-main.reader.strip.reply"), True)
    s.key("Escape")
    s.expect("shift-h-passes-on-in-the-list", refuses(s, "⇧H"), True)


def attachments_words(n):
    """The header's attachments row, as the app words it (catalogue keys, never a literal)."""
    return catalogue("1 attachment", "en") if n == 1 else catalogue("%@ attachments", "en").replace("%@", str(n))


R10_LIVE_KEY = ("⇧H", "live")


def r10_round(j):
    """Round 10 (SPEC A70): a click on the header line toggles it, the key toggles the marked one, the state survives N and P,
    shift O opens every header and closes them when all are open, the toggle reads no body and moves in under 180 ms."""
    n = THREAD_MSG["thread_count"]
    last = n - 1
    hdr = lambda s: header_rows(s)
    opens = lambda s: (lambda: [r["open"] for r in hdr(s)])
    line = lambda i: "mac-main.reader.header.line.%d" % i
    s = j.step("par-reader-r10-live", "The header command is live with its key")
    s.expect("toggle-message-header-is-live-on-shift-h", lambda: [(r["key"], r["status"]) for r in s.ctl.rows("mac-shortcuts.map") if r["command"] == HEADER_KEY_COMMAND], [R10_LIVE_KEY])

    s = j.step("par-reader-r10-header", "Click, key, N and P, shift O on the headers of a thread")
    open_row(s, 0)
    collapse_headers(s)
    s.click(line(last))
    s.expect("click-on-the-header-line-expands-its-message", opens(s), [False] * last + [True])
    s.expect("expanded-header-states-the-attachment-count", lambda: [(r["label"], r["value"]) for r in hdr(s)[last]["rows"]][-1], ("Attachments", attachments_words(len(THREAD_MSG["attachments"]))))
    s.click(line(last))
    s.expect("click-on-the-header-line-again-collapses-it", opens(s), [False] * n)
    s.expect("collapsed-header-shows-sender-and-time-only", lambda: (hdr(s)[last]["rows"], bool(hdr(s)[last]["time"])), ([], True))
    s.key("P")
    s.key("⇧H")
    s.expect("key-expands-the-marked-header", opens(s), [False] * (last - 1) + [True, False])
    s.key("⇧H")
    s.expect("key-again-collapses-the-marked-header", opens(s), [False] * n)
    s.key("⇧H")
    s.key("N")
    s.key("P")
    s.key("N")
    s.expect("state-is-kept-across-n-and-p", opens(s), [False] * (last - 1) + [True, False])
    s.click(line(0))
    s.expect("state-is-per-message", opens(s), [True, True] + [False] * (n - 2))
    s.click(line(0))
    s.click(line(1))
    s.expect("every-header-is-collapsed-again", opens(s), [False] * n)
    s.key("⇧O")
    s.expect("shift-o-opens-every-header", opens(s), [True] * n)
    s.key("⇧O")
    s.expect("shift-o-again-closes-every-header", opens(s), [False] * n)
    s.click(line(last))
    s.key("⇧O")
    s.expect("shift-o-with-one-header-open-opens-the-rest", opens(s), [True] * n)
    s.key("⇧O")
    s.expect("shift-o-closes-them-all-again", opens(s), [False] * n)
    s.expect("header-motion-is-opacity-and-transform-under-180-ms", lambda: (hdr(s)[last]["motion"], hdr(s)[last]["motion_ms"] < 180, hdr(s)[last]["motion_ms"] == const("ROW_COLLAPSE_MS")), ("opacity+transform", True, True))

    s = j.step("par-reader-r10-cost", "A header toggle reads no body from the core")
    s.click("mac-main.reader.perf.start")
    for k in ("⇧H", "⇧H", "N", "⇧H", "P", "⇧O", "⇧O"):
        s.key(k)
        wait(s, 20)
    s.click(line(last))
    s.click(line(last))
    wait(s, 300)
    window = {}
    s._act(lambda: window.update(s.ctl.rows("mac-main.reader.perf")[0]))        # read BEFORE the stop click (round 8: a click builds the tables)
    s.click("mac-main.reader.perf.stop")

    def reads():
        if "body_reads" not in window:
            raise Unreadable("the cost meter was not read before the stop click")
        return window["body_reads"]
    s.expect("toggling-headers-reads-no-body-from-the-core", reads, 0)
    s.key("Escape")
    s.key("Escape")


HEADER_ICONS = [("archive", "archive"), ("snooze", "snooze"), ("label", "label"), ("pin", "pin"), ("reply", "reply"), ("more", "palette")]


def hold_round(j):
    """Round 6: while the shell shows its hold-⌘ badges, each icon of the reader's header band shows its key under it."""
    s = j.step("par-reader-hold", "Hold command: one cap under each header icon")
    open_row(s, 0)
    s.expect("no-caps-before-the-hold", lambda: [r["visible"] for r in s.ctl.rows("mac-main.reader.keycaps")], [False] * len(HEADER_ICONS))
    s.click("macshell.hint.hold")           # the socket cannot hold ⌘: the same show the shell's timer calls
    wait(s, 200)
    s.expect("hold-shows-a-cap-under-every-icon", lambda: [r["visible"] for r in s.ctl.rows("mac-main.reader.keycaps")], [True] * len(HEADER_ICONS))
    s.expect("each-cap-is-its-icons-preset-key", lambda: [r["key"] for r in s.ctl.rows("mac-main.reader.keycaps")] == [preset_key(s, c) for _, c in HEADER_ICONS], True)   # the expected keys are read inside the thunk: a dry run counts the check
    s.click("macshell.hint.release")
    wait(s, const("POPOVER_MS") + 100)
    s.expect("release-hides-the-caps", lambda: [r["visible"] for r in s.ctl.rows("mac-main.reader.keycaps")], [False] * len(HEADER_ICONS))
    s.key("Escape")


def notdone_round(j):
    """Shift E from the reader: a thread archived from the reader comes back from the Archive view (G then E).
    The command is the inbox list's alone (TriageKeys.markNotDone, "Back in the list"); the reader provides none."""
    s = j.step("par-reader-not-done", "Shift E brings an archived thread back from the Archive view")
    m = THREAD_MSG
    open_row(s, 0)
    s.click("mac-main.reader.strip.archive")
    s.expect("archived-in-core", lambda: s.ctl.core("message/%s/archived" % m["id"]), True)
    s.key("Escape")
    s.key("G")
    s.key("E")                                   # go to done: the list shows the Archive mailbox (AppModel.mailbox = .archive)
    s.expect("g-then-e-lists-the-archived-thread", lambda: m["id"] in [r["id"] for r in s.ctl.rows("mac-main.list")], True)

    def shift_e_from_the_archive(read):
        def thunk():
            ids = [r["id"] for r in s.ctl.rows("mac-main.list")]
            if m["id"] not in ids:
                raise Unreadable("the Archive view does not list the archived thread: the list shows %r" % ids[:3])
            s.ctl.click("mac-main.list.row.%d" % ids.index(m["id"]))
            s.ctl.click("mac-main.reader.open")
            s.ctl.key("⇧E")
            return read()
        return thunk
    s.expect("shift-e-unarchives-in-core", shift_e_from_the_archive(lambda: s.ctl.core("message/%s/archived" % m["id"])), False)
    s.expect("the-list-says-back-in-the-list", lambda: s.ctl.rows("mac-main.list.state")[0]["undo_key"], "Back in the list")
    s.key("Escape")
    s.key("G")
    s.key("I")                                   # back to the inbox for the rounds after this one


def unsubscribe_round(j):
    """Round 6: command U asks first (what will happen, Cancel, Unsubscribe), then leaves the list through the core."""
    s = j.step("par-reader-unsubscribe", "Command U unsubscribes after a confirm")
    i, m = LISTS["msg-09"]
    open_row(s, 0)
    s.expect("command-u-passes-on-for-a-non-list-message", refuses(s, "⌘U"), True)
    open_row(s, i)
    s.key("⌘U")
    s.expect("command-u-asks-first", lambda: s.ctl.rows("mac-main.reader.unsubscribe")[0]["pending"], True)
    s.expect("nothing-left-before-the-confirm", lambda: s.ctl.core("message/%s/unsubscribed" % m["id"]), False)
    s.expect("the-confirm-names-the-method-and-host", lambda: (s.ctl.rows("mac-main.reader.unsubscribe")[0]["method"], "github-example.com" in s.ctl.rows("mac-main.reader.unsubscribe")[0]["says"]), ("one_click", True))
    s.click("mac-main.reader.unsubscribe.cancel")
    s.expect("cancel-leaves-nothing", lambda: (s.ctl.rows("mac-main.reader.unsubscribe")[0]["pending"], s.ctl.core("message/%s/unsubscribed" % m["id"])), (False, False))
    s.key("⌘U")
    s.click("mac-main.reader.unsubscribe.confirm")
    s.expect("confirm-unsubscribes-in-core", lambda: s.ctl.core("message/%s/unsubscribed" % m["id"]), True)
    s.expect("the-pane-says-unsubscribed", lambda: s.ctl.rows("mac-main.reader.unsubscribe")[0]["note"], "Unsubscribed")
    i2, m2 = LISTS["msg-11"]
    open_row(s, i2)
    s.key("⌘U")
    s.expect("a-web-link-list-says-the-browser-opens", lambda: s.ctl.rows("mac-main.reader.unsubscribe")[0].get("method", "no method in the row: the confirm is not armed"), "link")
    s.key("Escape")
    s.expect("escape-cancels-the-confirm", lambda: s.ctl.rows("mac-main.reader.unsubscribe")[0]["pending"], False)
    s.key("Escape")


R9_KEYS = {"next-message": "N", "previous-message": "P", "expand-message": "O", "expand-all-messages": "⇧O", "open-links": "⌘O",
           "cycle-links": "Tab", "scroll-down": "Space", "scroll-up": "⇧Space", "jump-top": "⌘Up", "jump-bottom": "⌘Down",
           "mark-not-done": "⇧E"}        # the v8 keys of round 9, as the map spells them (docs/reference/superhuman-shortcuts-v8.json)


def last_dispatch(s):
    """(command, outcome) of the last command the engine tried: what the key reached and what came of it."""
    row = s.ctl.rows("mac-shortcuts.dispatch")[-1]
    return row["command"], row["outcome"]


def r9_round(j):
    """Round 9 (the owner's rule: 'are you sure ALL superhuman controls are replicated exactly in the app?'): the reader keys are
    live commands with their v8 keys, the controls the keys also reach show them, Tab outside the reader is the split, and a
    key press reads no body from the core."""
    s = j.step("par-reader-r9-keys", "The reader keys are live commands and the controls they reach show their key")

    def ways():
        out = {}
        for r in s.ctl.rows("mac-shortcuts.map"):
            if r["command"] in R9_KEYS:
                out.setdefault(r["command"], []).append((r["key"], r["status"]))
        return out
    s.expect("reader-commands-are-live-with-their-v8-key", ways, {c: [(k, "live")] for c, k in R9_KEYS.items()})
    tip = lambda i: {r["id"]: r["tooltip"] for r in s.ctl.rows("mac-main.reader.keytips")}[i]
    s.expect("earlier-rule-tooltip-carries-its-key", lambda: tip("earlier"), catalogue("Expand all messages", "en") + "  " + R9_KEYS["expand-all-messages"])
    s.expect("link-chip-tooltip-carries-its-key", lambda: tip("links"), catalogue("Next link or date", "en") + "  " + R9_KEYS["cycle-links"])

    s = j.step("par-reader-r9-list", "Tab and Shift Tab outside the reader stay next and previous split")
    s.key("Escape")
    s.key("Escape")
    s.key("Tab")
    s.expect("tab-in-the-list-is-next-split", lambda: last_dispatch(s), ("next-split", "ran"))
    s.key("Shift+Tab")
    s.expect("shift-tab-in-the-list-is-previous-split", lambda: last_dispatch(s), ("previous-split", "ran"))

    s = j.step("par-reader-r9-cost", "A key press reads no body from the core")
    open_row(s, 0)
    s.key("⇧O")
    wait(s, 300)
    s.click("mac-main.reader.perf.start")
    for k in ("N", "P", "N", "P", "Tab", "Shift+Tab", "⌘O", "Space", "⇧Space", "⌘Down", "⌘Up", "O", "⇧O"):
        s.key(k)
        wait(s, 20)
    wait(s, 300)
    window = {}
    s._act(lambda: window.update(s.ctl.rows("mac-main.reader.perf")[0]))        # read BEFORE the stop click (round 8: a click builds the tables)
    s.click("mac-main.reader.perf.stop")

    def reads():
        if "body_reads" not in window:
            raise Unreadable("the cost meter was not read before the stop click")
        return window["body_reads"]
    s.expect("keys-make-the-reader-read-no-body-from-the-core", reads, 0)
    s.key("Escape")
    s.key("Escape")


def r9_ring(j):
    """Round 9: Shift Tab walks the ring back, as Tab walks it on. Needs a message with links and dates (links_round's corpus)."""
    s = j.step("par-reader-r9-ring", "Shift Tab walks the ring of links and dates back")
    open_row(s, 0)
    ring = lambda: [r["ring"] for r in s.ctl.rows("mac-main.reader.links")]
    on = lambda i: (lambda r: len(r) >= 2 and r[i] and sum(r) == 1)(ring())
    s.key("Shift+Tab")
    s.expect("shift-tab-from-off-lands-on-the-last", lambda: on(-1), True)
    s.key("Shift+Tab")
    s.expect("shift-tab-walks-back-one", lambda: on(-2), True)
    s.key("Tab")
    s.expect("tab-after-shift-tab-moves-forward", lambda: on(-1), True)
    s._act(lambda: [s.ctl.key("Shift+Tab") for _ in range(len(ring()) - 1)])
    s.expect("shift-tab-reaches-the-first", lambda: on(0), True)
    s.key("Shift+Tab")
    s.expect("shift-tab-before-the-first-takes-the-ring-off", lambda: sum(ring()), 0)
    s.key("Escape")
    s.key("Escape")


def corpus_first_link():
    """The first link of the newest corpus message (the one links_round loads), from the corpus file. The selftest's model has no
    corpus on a sparse worktree, so it gets a stand-in; a live run with the file missing is a crash, as links_round's is."""
    try:
        msgs = json.load(open(CORPUS, encoding="utf-8"))["messages"]
    except FileNotFoundError:
        if FAST:
            return "https://selftest.invalid/first"
        raise
    return [t for k, t in expected_items(sorted(msgs, key=lambda m: m["date"])[-1]["body"]) if k == "link"][0]


STATE = "mac-main.reader.state"
QUICK = "mac-main.quickreply.field"
INSPECTOR = "mac-labels.inspector"


def doors_round(j):
    """doors-read: the controls of the reader that no round above clicks, each once, with a visible outcome: the label
    inspector's button, the reply block, the delete and header doors, the earlier-messages rule, Backspace, Enter on the
    focused row, a link chip, the unsubscribe doors, and the two strip buttons that have no live door yet (a finding)."""
    n = THREAD_MSG["thread_count"]
    last = n - 1
    mid = THREAD_MSG["id"]
    quick_open = lambda s: (lambda: s.ctl.rows(QUICK) != [])
    i11, _ = LISTS["msg-11"]

    s = j.step("par-reader-doors-header", "The label inspector's button and the two strip buttons that have no live door")
    open_row(s, 0)
    s.click("mac-labels.inspector.toolbar")
    s.expect("inspector-button-opens-the-inspector", lambda: s.ctl.rows(INSPECTOR)[0]["open"], True)
    s.click("mac-labels.inspector.toolbar")
    s.expect("inspector-button-closes-it-again", lambda: s.ctl.rows(INSPECTOR)[0]["open"], False)
    # Reply all and Forward are live (ReplyDoors in Commands.swift): each click is read by its visible outcome, not by the door's name.
    qh = lambda: (s.ctl.rows("mac-main.quickreply.header") or [{}])[0]
    s.click("mac-main.reader.strip.replyall")
    s.expect("strip-reply-all-opens-the-reply-box-with-the-header-filled", lambda: (qh().get("open"), bool(qh().get("to"))), ("1", True))
    s.key("Escape")
    fw = lambda: (s.ctl.rows("mac-composer.popout") or [{}])[0]
    s.click("mac-main.reader.strip.forward")
    s.expect("strip-forward-opens-the-forward-window", lambda: (fw().get("open"), fw().get("kind")), (True, "forward"))
    s.key("Escape")

    s = j.step("par-reader-doors-reply", "The reply block, the header and delete doors, the earlier rule, Backspace, Enter on the focused row")
    open_row(s, 0)
    s.click("mac-main.reader.reply")
    s.expect("reply-block-opens-the-quick-reply", quick_open(s), True)
    s.key("Escape")
    collapse_headers(s)
    s.click("mac-main.reader.header")
    s.expect("header-door-expands-the-open-message-header", lambda: [r["open"] for r in header_rows(s)], [False] * last + [True])
    s.click("mac-main.reader.header")
    s.expect("header-door-collapses-it-again", lambda: [r["open"] for r in header_rows(s)], [False] * n)
    fold = {}
    s._act(lambda: fold.update(was=s.ctl.rows(STATE)[0]["earlier_open"]))
    s.click("mac-main.reader.earlier")
    s.expect("earlier-rule-click-flips-the-fold", lambda: s.ctl.rows(STATE)[0]["earlier_open"] == (not fold["was"]), True)
    s.click("mac-main.reader.earlier")
    s.expect("earlier-rule-click-again-restores-it", lambda: s.ctl.rows(STATE)[0]["earlier_open"] == fold["was"], True)
    s.click("mac-main.reader.delete")
    s.expect("delete-door-deletes-in-core", lambda: s.ctl.core("message/%s/deleted" % mid), True)
    s.expect("delete-door-returns-to-the-list", lambda: s.ctl.uistate()["surface"], "mac-main")
    s.key("Z")
    s.expect("delete-door-undo-restores-the-message", lambda: s.ctl.core("message/%s/deleted" % mid), False)
    open_row(s, 0)
    # plain Backspace deletes no mail since 13 Nov (owner: "cmd+del not just del it would clash"; f4d2c58ea): the mail stays, open
    s.expect("backspace-deletes-no-mail", lambda: (press_any(s, "Backspace"), s.ctl.core("message/%s/deleted" % mid), s.ctl.uistate()["surface"])[1:], (False, "mac-reader"))
    s.key("⇧#")
    s.expect("shift-hash-deletes-the-open-message", lambda: s.ctl.core("message/%s/deleted" % mid), True)
    s.key("Z")
    s.key("Escape")
    s.click("mac-main.pane.reader")
    s.key("Enter")
    s.expect("enter-on-the-focused-row-opens-the-quick-reply", quick_open(s), True)
    s.key("Escape")
    s.key("Escape")

    s = j.step("par-reader-doors-unsubscribe", "The unsubscribe doors: ask, then cancel; a message with no list is refused")
    open_row(s, i11)
    s.click("mac-main.reader.unsubscribe.ask")
    s.expect("ask-door-arms-the-confirm", lambda: s.ctl.rows("mac-main.reader.unsubscribe")[0]["pending"], True)
    s.expect("ask-door-names-the-method", lambda: s.ctl.rows("mac-main.reader.unsubscribe")[0]["method"], "link")
    s.click("mac-main.reader.unsubscribe.cancel")
    s.expect("cancel-door-clears-the-confirm", lambda: s.ctl.rows("mac-main.reader.unsubscribe")[0]["pending"], False)
    open_row(s, 0)
    s.expect("ask-door-refuses-a-message-with-no-list", refuses_click(s, "mac-main.reader.unsubscribe.ask"), True)
    s.key("Escape")


def doors_links(j):
    """doors-read: a link chip, clicked, opens its link. Runs after links_round: the chips need the corpus it loads."""
    first_link = corpus_first_link()
    s = j.step("par-reader-doors-links", "A link chip, clicked, opens its link")
    open_row(s, 0)
    s.key("Tab")
    s.click("mac-main.reader.link.0")
    s.expect("a-link-chip-click-opens-the-link", lambda: (lambda r: (r["kind"], r["target"]))(s.ctl.rows("mac-main.reader.opened")[-1]), ("browser", first_link))
    s.key("Escape")
    s.key("Escape")


R9_MUTANTS = [   # (check label, the model's mutant that must turn it red)
    ("par-reader-r9-keys/reader-commands-are-live-with-their-v8-key", "reader-stub"),
    ("par-reader-r9-keys/earlier-rule-tooltip-carries-its-key", "tip-no-key-earlier"),
    ("par-reader-r9-keys/link-chip-tooltip-carries-its-key", "tip-no-key-link"),
    ("par-reader-r9-list/tab-in-the-list-is-next-split", "list-tab-cycles"),
    ("par-reader-r9-list/shift-tab-in-the-list-is-previous-split", "list-back-cycles"),
    ("par-reader-r9-cost/keys-make-the-reader-read-no-body-from-the-core", "no-body-memo"),
    ("par-reader-r9-ring/shift-tab-from-off-lands-on-the-last", "back-forward"),
    ("par-reader-r9-ring/shift-tab-walks-back-one", "back-forward"),
    ("par-reader-r9-ring/tab-after-shift-tab-moves-forward", "tab-restarts"),
    ("par-reader-r9-ring/shift-tab-reaches-the-first", "back-stops-early"),
    ("par-reader-r9-ring/shift-tab-before-the-first-takes-the-ring-off", "back-wraps"),
]


R10_MUTANTS = [   # (check label, the model's mutant that must turn it red)
    ("par-reader-r10-live/toggle-message-header-is-live-on-shift-h", "header-stub"),
    ("par-reader-r10-header/click-on-the-header-line-expands-its-message", "line-click-dead"),
    ("par-reader-r10-header/expanded-header-states-the-attachment-count", "no-attachments-row"),
    ("par-reader-r10-header/click-on-the-header-line-again-collapses-it", "line-click-open-only"),
    ("par-reader-r10-header/collapsed-header-shows-sender-and-time-only", "collapsed-keeps-rows"),
    ("par-reader-r10-header/key-expands-the-marked-header", "key-dead"),
    ("par-reader-r10-header/key-again-collapses-the-marked-header", "key-open-only"),
    ("par-reader-r10-header/state-is-kept-across-n-and-p", "np-resets"),
    ("par-reader-r10-header/state-is-per-message", "click-opens-all"),
    ("par-reader-r10-header/every-header-is-collapsed-again", "line-click-open-only"),
    ("par-reader-r10-header/shift-o-opens-every-header", "so-no-headers"),
    ("par-reader-r10-header/shift-o-again-closes-every-header", "so-no-collapse"),
    ("par-reader-r10-header/shift-o-with-one-header-open-opens-the-rest", "so-collapses-when-any-open"),
    ("par-reader-r10-header/shift-o-closes-them-all-again", "so-no-collapse"),
    ("par-reader-r10-header/header-motion-is-opacity-and-transform-under-180-ms", "slow-motion"),
    ("par-reader-r10-cost/toggling-headers-reads-no-body-from-the-core", "toggle-reads-body"),
]


R12_MUTANTS = [   # A129 nav_round: (check label, the NavModel's mutant that must turn it red)
    ("par-reader-a129-nav/25-presses-read-at-most-one-body", "press-reads-a-body"),
    ("par-reader-a129-nav/25-presses-build-no-body", "build-loads-the-body"),
    ("par-reader-a129-nav/20-moves-read-one-body-each-at-most", "press-reads-a-body-twice"),
    ("par-reader-a129-nav/20-moves-evaluate-the-pane-once-each-at-most", "pane-evaluates-twice"),
    ("par-reader-a129-nav/20-moves-keep-every-turn-under-the-frame-budget", "key-frame-builds-the-thread"),
    ("par-reader-a129-nav/20-moves-rebuild-only-the-rows-that-change", "move-rebuilds-every-row"),
]
R13_MUTANTS = [   # A129 colours_round: (check label, the ColoursModel's mutant)
    ("par-reader-a129-colours/list-focus-shift-i-filters", "list-switches-colours"),
    ("par-reader-a129-colours/list-focus-shift-i-leaves-the-colours", "list-switches-colours"),
    ("par-reader-a129-colours/reader-focus-shift-i-switches", "reader-filters"),
    ("par-reader-a129-colours/reader-focus-shift-i-changes-the-colours", "reader-filters"),
    ("par-reader-a129-colours/reader-focus-shift-i-again-restores-them", "reader-one-way"),
]
R13_MUTANTS += [("par-reader-a129-colours/shift-i-in-the-list-filters-whatever-the-pane-focus-was", "stale-pane-focus"),
    ("par-reader-a129-colours/foot-strip-after-return-and-escape-is-the-list-set", "field-focus-stays"),
    ("par-reader-a129-colours/return-opened-reader-shift-i-switches", "needs-the-pane-focus"),
    ("par-reader-a129-colours/reader-focus-shift-i-switches", "click-focuses-the-reply-field"),
    ("par-reader-a129-colours/foot-strip-in-the-open-mail-is-the-reader-set", "bar-reads-the-pane")]
R15_MUTANTS = [   # A129e o_round (renamed at the merge: triage-keys also named a list R14_MUTANTS, and the later one replaced this one)
    ("par-reader-o/o-on-the-open-message-passes-on", "o-taken-without-a-fold"),
    ("par-reader-o/the-dots-key-is-the-presets-expand-message-key", "pill-key-literal"),
    ("par-reader-o/o-on-a-folded-mail-toggles-the-dots", "o-never-toggles"),
    ("par-reader-o/o-on-a-folded-mail-leaves-the-text-where-it-is", "o-moves-the-text"),
    ("par-reader-o/o-again-folds-the-dots", "o-only-opens"),
    ("par-reader-o/o-on-a-folded-mail-toggles-the-dots", "click-focuses-the-reply-field"),
    ("par-reader-o/the-dots-path-has-no-literal-o-chord", "literal-o-back"),
]
R11_MUTANTS = [   # doors-read: (check label, the model's mutant that must turn it red)
    ("par-reader-doors-header/inspector-button-opens-the-inspector", "inspector-dead"),
    ("par-reader-doors-header/inspector-button-closes-it-again", "inspector-stuck-open"),
    ("par-reader-doors-header/strip-reply-all-opens-the-reply-box-with-the-header-filled", "strip-replyall-dead"),
    ("par-reader-doors-header/strip-forward-opens-the-forward-window", "strip-forward-dead"),
    ("par-reader-doors-reply/reply-block-opens-the-quick-reply", "reply-block-dead"),
    ("par-reader-doors-reply/header-door-expands-the-open-message-header", "header-door-dead"),
    ("par-reader-doors-reply/header-door-collapses-it-again", "header-door-open-only"),
    ("par-reader-doors-reply/earlier-rule-click-flips-the-fold", "earlier-dead"),
    ("par-reader-doors-reply/earlier-rule-click-again-restores-it", "earlier-open-only"),
    ("par-reader-doors-reply/delete-door-deletes-in-core", "delete-door-dead"),
    ("par-reader-doors-reply/delete-door-returns-to-the-list", "delete-stays"),
    ("par-reader-doors-reply/delete-door-undo-restores-the-message", "undo-dead"),
    ("par-reader-doors-reply/backspace-deletes-no-mail", "backspace-deletes"),
    ("par-reader-doors-reply/shift-hash-deletes-the-open-message", "hash-dead"),
    ("par-reader-doors-reply/enter-on-the-focused-row-opens-the-quick-reply", "enter-dead"),
    ("par-reader-doors-links/a-link-chip-click-opens-the-link", "chip-link-dead"),
    ("par-reader-doors-unsubscribe/ask-door-arms-the-confirm", "ask-dead"),
    ("par-reader-doors-unsubscribe/ask-door-names-the-method", "ask-wrong-method"),
    ("par-reader-doors-unsubscribe/cancel-door-clears-the-confirm", "cancel-dead"),
    ("par-reader-doors-unsubscribe/ask-door-refuses-a-message-with-no-list", "ask-always"),
]


# ---------------------------------------------------------------------------------------------------------------------
# triage-keys: command U asks once, sends only on the confirm, a second command U sends nothing and says when the list was left,
# and a web-link list opens its page after the confirm. The one-click request runs off the main actor (the key answers at once).
UNSUB_ACTIONS = os.path.join(ROOT, "Sources", "Features", "Reader", "ReaderListActions.swift")
UNSUB_ASK = 'message/\\(t.messageId)/unsubscribed") == "true"'
UNSUB_ARM = "ReaderState.shared.unsubscribe = (t.messageId, how)"


def unsub_sources():
    return {"actions": open(UNSUB_ACTIONS, encoding="utf-8").read()}


def unsub_source_faults(src):
    """What the Swift source must hold for the three promises the drive cannot count (pure, so --selftest hands it a mutated copy)."""
    t = src["actions"]
    a, b = t.index("static func unsubscribe("), t.index("static func leftWords")
    ask = t[a:b]
    c = t.index("static func confirmUnsubscribe(")
    confirm = t[c:t.index("private static func finish(")]
    return {"reads-the-mark": [] if UNSUB_ASK in ask and ask.index(UNSUB_ASK) < ask.index(UNSUB_ARM) else ["the ask arms the confirm before it reads the core's mark"],
            "ask-never-sends": [".unsubscribe(messageId" for _ in [0] if ".unsubscribe(messageId" in ask or "core.run(" in ask],
            "off-main": [] if "await Task.detached" in confirm and ".oneClick" in confirm else ["the one-click request is not run off the main actor"]}


def outside_opens(s):
    """How many web addresses the hidden rig refused to open so far (`uistate.hidden.system_ui.open`, HiddenSystemUI.recordRefusedOpen): an attempt to leave this Mac."""
    return int((s.ctl.uistate().get("hidden", {}).get("system_ui", {}) or {}).get("open", 0))


def unsub_twice_round(j, src=None):
    """Command U on a one-click list: one confirm, one engine call; a second command U sends nothing and says when it was left (the core's
    `unsubscribed_at`; a core without it, the fixture, is NOT JUDGED on the date); on a web-link list the page opens after the confirm only."""
    src = src or unsub_sources()
    faults = unsub_source_faults(src)
    s = j.step("par-reader-unsub2", "Command U: one confirm, one engine call, a second press sends nothing and says when; a web link opens after the confirm")
    i15, m15 = LISTS["msg-15"]
    i11, m11 = LISTS["msg-11"]
    page = next(u.strip("<>") for u in m11["list_unsubscribe"] if u.startswith("<https:"))
    row = lambda: s.ctl.rows("mac-main.reader.unsubscribe")[0]
    left = lambda m: s.ctl.core("message/%s/unsubscribed" % m["id"])
    base = {}
    open_row(s, i15)
    s._act(lambda: base.update(n=row()["calls"]))
    s.key("⌘U")
    s.expect("command-u-with-no-confirm-sends-nothing", lambda: (row()["pending"], row()["calls"] - base["n"], left(m15)), (True, 0, False))
    s.click("mac-main.reader.unsubscribe.confirm")
    s.expect("the-confirm-calls-the-engine-once", lambda: (row()["calls"] - base["n"], left(m15)), (1, True))
    s.key("⌘U")
    s.expect("a-second-command-u-sends-nothing", lambda: (row()["pending"], row()["calls"] - base["n"]), (False, 1))

    def date_said():
        at = s.ctl.core("message/%s/unsubscribed_at" % m15["id"])
        if at in (None, ""):
            raise Unreadable("this core serves no message/<id>/unsubscribed_at (the fixture core does not): the date is judged on the real core")
        import datetime
        d = datetime.datetime.fromtimestamp(int(at) / 1000.0)
        note = row()["note"] or ""
        return note.startswith("Unsubscribed on ") and str(d.year) in note and str(d.day) in note
    s.expect("a-second-command-u-says-the-date", date_said, True)
    open_row(s, i11)
    s._act(lambda: base.update(page=row()["opened"].count(page), out=outside_opens(s)))
    s.key("⌘U")
    s.expect("a-web-link-opens-nothing-before-the-confirm", lambda: row()["opened"].count(page) - base["page"], 0)
    s.click("mac-main.reader.unsubscribe.confirm")
    s.expect("a-web-link-opens-its-page-after-the-confirm", lambda: (row()["opened"].count(page) - base["page"], left(m11)), (1, True))
    # d66b30dd (a test copy never reaches outside this Mac): the page is a web address off this Mac. On the fixture core the app logs it and starts no browser
    # (ReaderListActions.swift:88, ReaderLinks.swift:94 skip NSWorkspace); if a call to the workspace were made the rig would refuse it and COUNT it in
    # `uistate.hidden.system_ui.open`. The peer read: no web address was even attempted.
    s.expect("a-web-link-reaches-nothing-outside-this-mac", lambda: outside_opens(s) - base["out"], 0)
    s.key("Escape")
    s.expect("command-u-reads-the-core-mark-before-it-arms", lambda: faults["reads-the-mark"], [])
    s.expect("command-u-itself-never-asks-the-engine", lambda: faults["ask-never-sends"], [])
    s.expect("the-one-click-request-runs-off-the-main-actor", lambda: faults["off-main"], [])


class UnsubCtl:
    """The stand-in for unsub_twice_round: the screen as designed, over the socket's verbs the round uses; a mutant breaks one thing."""
    def __init__(self, mutant=None):
        self.m, self.row, self.msg = mutant, 0, None
        self.pending, self.note, self.calls, self.opened, self.done = False, None, 0, [], set()
        self.methods = {i: ("msg-15", "one_click") for i, _ in [LISTS["msg-15"]]}
        self.methods[LISTS["msg-11"][0]] = ("msg-11", "link")

    def uistate(self):
        return {"surface": "mac-reader" if self.msg else "mac-main", "hidden": {"system_ui": {"open": getattr(self, "outside", 0)}}}

    def send(self, *a):
        return {}

    def click(self, ax):
        if ax == "mac-main.reader.open":
            self.msg = self.methods.get(self.row)
        elif ax == "mac-main.reader.unsubscribe.confirm":
            if not self.pending:
                raise Unreadable("click: nothing armed")
            self.pending = False
            self.ask_engine(2 if self.m == "confirm-sends-twice" else 1)
            self.note = "Unsubscribed"
            if self.msg[1] == "link" and self.m != "link-never-opens":
                self.opened.append(UNSUB_PAGE)
                if self.m == "link-asks-the-real-browser":
                    self.outside = getattr(self, "outside", 0) + 1       # the broken copy: the workspace is called, the rig refuses and counts it
        return {}

    def ask_engine(self, n):
        self.calls += n
        self.done.add(self.msg[0])

    def key(self, chord):
        if chord == "K":
            self.row = max(0, self.row - 1)
        elif chord == "J":
            self.row += 1
        elif chord == "Escape":
            self.msg, self.pending = None, False
        elif chord == "⌘U":
            if self.msg is None:
                raise Unreadable("key ⌘U: not taken")
            self.note = None
            if self.msg[0] in self.done and self.m != "second-u-asks-again":
                self.pending, self.note = False, ("Unsubscribed" if self.m == "second-u-no-date" else "Unsubscribed on %s" % UNSUB_DATE)
                return {}
            self.pending = True
            if self.m == "u-sends-at-once":
                self.ask_engine(1)
            if self.m == "link-opens-at-once" and self.msg[1] == "link":
                self.opened.append(UNSUB_PAGE)
        else:
            raise Unreadable("key %s: not taken" % chord)
        return {}

    def core(self, path):
        parts = path.split("/")
        if parts[-1] == "unsubscribed":
            return parts[1] in self.done
        if parts[-1] == "unsubscribed_at":
            return str(UNSUB_MS) if parts[1] in self.done else None
        raise Unreadable("the stand-in has no path %s" % path)

    def rows(self, ax):
        if ax == "mac-main.reader.unsubscribe":
            return [{"pending": self.pending, "note": self.note, "calls": self.calls, "opened": list(self.opened), "sending": None}]
        raise Unreadable("the stand-in has no region %s" % ax)


import datetime as _dt
UNSUB_MS = 1762862400000     # 2025-11-11 12:00 UTC: far from a day edge in every zone
UNSUB_DATE = _dt.datetime.fromtimestamp(UNSUB_MS / 1000.0).strftime("%B %d, %Y").replace(" 0", " ")
UNSUB_PAGE = "https://billing-example.com/email-preferences"

R14_MUTANTS = [   # triage-keys: (check label, the stand-in's mutant, or "source:<name>" for a broken copy of ReaderListActions.swift)
    ("par-reader-unsub2/command-u-with-no-confirm-sends-nothing", "u-sends-at-once"),
    ("par-reader-unsub2/the-confirm-calls-the-engine-once", "confirm-sends-twice"),
    ("par-reader-unsub2/a-second-command-u-sends-nothing", "second-u-asks-again"),
    ("par-reader-unsub2/a-second-command-u-says-the-date", "second-u-no-date"),
    ("par-reader-unsub2/a-web-link-opens-nothing-before-the-confirm", "link-opens-at-once"),
    ("par-reader-unsub2/a-web-link-opens-its-page-after-the-confirm", "link-never-opens"),
    ("par-reader-unsub2/a-web-link-reaches-nothing-outside-this-mac", "link-asks-the-real-browser"),
    ("par-reader-unsub2/command-u-reads-the-core-mark-before-it-arms", "source:ask-skips-the-mark"),
    ("par-reader-unsub2/command-u-itself-never-asks-the-engine", "source:ask-sends"),
    ("par-reader-unsub2/the-one-click-request-runs-off-the-main-actor", "source:on-main"),
]


def unsub_mutated(src0):
    t = src0["actions"]
    return {"source:ask-skips-the-mark": dict(src0, actions=t.replace(UNSUB_ASK, 'message/\\(t.messageId)/unsubscribed") == "never"', 1)),
            "source:ask-sends": dict(src0, actions=t.replace(UNSUB_ARM, "_ = m.perform(.unsubscribe(messageId: t.messageId)); " + UNSUB_ARM, 1)),
            "source:on-main": dict(src0, actions=t.replace("await Task.detached", "await Task", 1))}


def selftest_unsub():
    """unsub_twice_round on the stand-in and on broken copies of the source: every check green clean, each red on its mutant."""
    src0 = unsub_sources()
    mutated = unsub_mutated(src0)
    for name, v in mutated.items():
        assert v != src0, "mutant %s changes nothing: its anchor text is gone from ReaderListActions.swift" % name
    def run(mutant=None):
        j = Journey(ID, UnsubCtl(None if mutant in mutated else mutant))
        unsub_twice_round(j, mutated.get(mutant, src0))
        return {c.label: c for c in j.checks()}
    return run, [l for l, _ in R14_MUTANTS]


class Model:
    """The reader on the wire for --selftest: what the round-9 rows and keys answer when the app behaves as designed. A mutant
    breaks exactly the behaviour its check names (R9_MUTANTS). It speaks the control socket's verbs the round uses."""
    ITEMS = 4                                   # links and dates of the open message

    def __init__(self, mutant=None):
        self.m = mutant
        self.surface, self.cursor, self.back = "mac-main", None, False
        self.log, self.reads = [], 0
        self.hdr, self.mark = [False] * THREAD_MSG["thread_count"], None      # round 10: the headers' state and the message N and P stand on
        self.row, self.pane, self.quick, self.inspector = 0, False, False, False   # doors-read: the list row, the pane holding the keys, the quick reply, the inspector
        self.deleted, self.earlier, self.unsub = False, False, False
        self.qhdr, self.fwd = False, False
        self.opened = [("calendar", "Monday")]

    # -- the socket
    def send(self, verb, *args):
        return {}

    def dead(self):
        raise Unreadable("click: nothing happened")

    def door_click(self, ax):
        """doors-read: the doors of the controls no earlier round clicked; a dead mutant answers as the app does (ok:false)."""
        m = self.m
        reader = self.surface == "mac-reader"
        if ax == "mac-labels.inspector.toolbar":
            if m == "inspector-dead" or (m == "inspector-stuck-open" and self.inspector):
                self.dead()
            self.inspector = not self.inspector
        elif ax == "mac-main.reader.reply":
            if m == "reply-block-dead" or not reader:
                self.dead()
            self.quick = True
        elif ax == "mac-main.reader.header":
            if m == "header-door-dead" or not reader:
                self.dead()
            at = len(self.hdr) - 1 if self.mark is None else self.mark
            self.hdr[at] = True if m == "header-door-open-only" else not self.hdr[at]
        elif ax == "mac-main.reader.earlier":
            if m == "earlier-dead" or not reader:
                self.dead()
            self.earlier = True if m == "earlier-open-only" else not self.earlier
        elif ax == "mac-main.reader.delete":
            if m == "delete-door-dead" or not reader:
                self.dead()
            self.deleted = True
            self.surface = self.surface if m == "delete-stays" else "mac-main"
        elif ax == "mac-main.pane.reader":
            self.pane = True
        elif ax == "mac-main.reader.strip.replyall":
            if m == "strip-replyall-dead" or not reader:
                self.dead()
            self.quick, self.qhdr = True, True
        elif ax == "mac-main.reader.strip.forward":
            if m == "strip-forward-dead" or not reader:
                self.dead()
            self.fwd = True
        elif ax == "mac-main.reader.link.0":
            if m == "chip-link-dead" or self.cursor is None:
                self.dead()
            self.opened.append(("browser", corpus_first_link()))
        elif ax == "mac-main.reader.unsubscribe.ask":
            on_a_list = self.row in {i for i, _ in LISTS.values()}
            if m == "ask-dead" or not reader or not (on_a_list or m == "ask-always"):
                self.dead()
            self.unsub = True
        elif ax == "mac-main.reader.unsubscribe.cancel":
            if m == "cancel-dead" or not self.unsub:
                self.dead()
            self.unsub = False
        else:
            return False
        return True

    def door_key(self, chord):
        """doors-read: Escape closes the quick reply first, Z brings a deleted message back, Backspace and Enter."""
        m = self.m
        if chord == "Escape":
            if self.quick or self.fwd:
                self.quick, self.qhdr, self.fwd = False, False, False
                return True
            self.pane = False
            return False
        if chord == "Z":
            self.deleted = self.deleted and m == "undo-dead"
            return True
        if chord in ("\x7f", "Backspace") and self.surface == "mac-reader" and not self.quick:
            if m == "backspace-deletes":
                self.deleted, self.surface = True, "mac-main"
                return True
            raise Unreadable("key: not taken")
        if chord == "⇧#" and self.surface == "mac-reader" and not self.quick:
            if m == "hash-dead":
                raise Unreadable("key: not taken")
            self.deleted, self.surface = True, "mac-main"
            return True
        if chord == "Enter" and self.pane and self.surface == "mac-main":
            if m == "enter-dead":
                raise Unreadable("key: not taken")
            self.quick, self.surface, self.pane = True, "mac-reader", False
            return True
        return False

    def click(self, ax):
        if self.door_click(ax):
            return {}
        if ax == "mac-main.reader.open":
            self.surface, self.cursor = "mac-reader", None
            self.hdr, self.mark = [False] * len(self.hdr), None
        elif ax.startswith("mac-main.reader.header."):
            self.header_click(ax.rsplit(".", 1)[1], line=".line." in ax)
        elif ax == "mac-main.reader.perf.start":
            self.reads = 0
        return {}

    def key(self, chord):
        if self.door_key(chord):
            return {}
        reader = self.surface == "mac-reader"
        if chord == "Escape":
            self.surface, self.cursor = "mac-main", None
        elif chord in ("Tab", "Shift+Tab"):
            self.tab(chord, reader)
        elif chord in ("J", "K"):
            self.row = max(0, self.row + (1 if chord == "J" else -1))
        elif reader and chord in ("N", "P", "O", "⇧O", "⇧H", "⌘O", "Space", "⇧Space", "⌘Down", "⌘Up"):
            if self.m == "no-body-memo" and chord in ("N", "P", "O", "⇧O", "⌘O"):
                self.reads += 1
            if self.m == "toggle-reads-body" and chord in ("⇧H", "⇧O"):
                self.reads += 1
            if chord in ("N", "P"):
                at = len(self.hdr) - 1 if self.mark is None else self.mark
                self.mark = min(max(at + (1 if chord == "N" else -1), 0), len(self.hdr) - 1)
                if self.m == "np-resets":
                    self.hdr = [False] * len(self.hdr)
            elif chord == "⇧H" and self.m != "key-dead":
                at = len(self.hdr) - 1 if self.mark is None else self.mark
                self.hdr[at] = True if self.m == "key-open-only" else not self.hdr[at]
            elif chord == "⇧O":
                self.shift_o()
        elif chord == "⇧H" and not reader:
            raise Unreadable("key ⇧H: not taken")
        else:
            raise Unreadable("key %s: not taken" % chord)
        return {}

    def header_click(self, tail, line):
        """A click on a header's line (and its chevron): that message's header flips; the mutants break one way."""
        if self.m == "toggle-reads-body":
            self.reads += 1
        i = int(tail)
        if line and self.m == "line-click-dead":
            return
        if line and self.m == "click-opens-all":
            self.hdr = [True] * len(self.hdr)
        elif self.m == "line-click-open-only" and line:
            self.hdr[i] = True
        else:
            self.hdr[i] = not self.hdr[i]

    def shift_o(self):
        """Shift O on the headers: all open, or all closed when every one is open."""
        if self.m == "so-no-headers":
            return
        shut = (any(self.hdr) if self.m == "so-collapses-when-any-open" else all(self.hdr)) and self.m != "so-no-collapse"
        self.hdr = [not shut] * len(self.hdr)

    def header_rows(self):
        last = len(self.hdr) - 1
        out = []
        for i, o in enumerate(self.hdr):
            rows = [{"label": "From", "value": "x"}, {"label": "To", "value": "y"}, {"label": "Date", "value": "z"}]
            if i == last and self.m != "no-attachments-row":
                rows.append({"label": "Attachments", "value": attachments_words(len(THREAD_MSG["attachments"]))})
            out.append({"index": i, "open": o, "rows": rows if (o or self.m == "collapsed-keeps-rows") else [], "time": "09:12",
                        "motion": "opacity+transform", "motion_ms": 260 if self.m == "slow-motion" else const("ROW_COLLAPSE_MS")})
        return out

    def tab(self, chord, reader):
        forward = chord == "Tab"
        if reader or (self.m == "list-tab-cycles" and forward) or (self.m == "list-back-cycles" and not forward):
            self.log.append(("cycle-links" if forward or not reader else "previous-split", "ran"))
            self.step_ring(forward if self.m != "back-forward" else True)
            return
        self.log += [("cycle-links", "passed"), ("indent-list", "passed"), ("next-split", "ran")] if forward \
            else [("outdent-list", "passed"), ("previous-split", "ran")]

    def step_ring(self, forward):
        n, c = self.ITEMS, self.cursor
        if forward:
            if self.m == "tab-restarts" and self.back:
                c = 0
            else:
                c = (c + 1 if c + 1 < n else None) if c is not None else 0
        else:
            floor = 1 if self.m == "back-stops-early" else 0
            if c is None:
                c = n - 1
            elif c > floor:
                c -= 1
            elif c == 0:
                c = n - 1 if self.m == "back-wraps" else None
        self.cursor, self.back = c, not forward

    def core(self, path):
        if path.endswith("/deleted"):
            return self.deleted
        raise Unreadable("the model has no core state %s" % path)

    def uistate(self):
        return {"surface": self.surface}

    def rows(self, ax):
        if ax == INSPECTOR:
            return [{"open": self.inspector}]
        if ax == QUICK:
            return [{"thread": "t"}] if self.quick else []
        if ax == "mac-main.quickreply.header":
            return [{"open": "1", "to": "karen@x.example, ben@x.example"}] if self.qhdr else []
        if ax == "mac-composer.popout":
            return [{"open": True, "kind": "forward"}] if self.fwd else []
        if ax == STATE:
            return [{"open": self.surface == "mac-reader", "earlier_open": self.earlier, "message_count": len(self.hdr)}]
        if ax == "mac-main.reader.unsubscribe":
            return [{"pending": self.unsub, "method": "one_click" if self.m == "ask-wrong-method" else "link", "note": None}]
        if ax == "mac-main.reader.opened":
            return [{"kind": k, "target": t} for k, t in self.opened]
        if ax == "mac-main.reader.header":
            return self.header_rows()
        if ax == "mac-shortcuts.map":
            return [{"command": c, "key": k, "status": "stub" if (self.m == "reader-stub" and c == "scroll-down") else "live", "kind": "row"}
                    for c, k in R9_KEYS.items()] + [{"command": HEADER_KEY_COMMAND, "key": "⇧H", "status": "stub" if self.m == "header-stub" else "live", "kind": "row"}] + [{"command": "next-split", "key": "Tab", "status": "live", "kind": "row"}]
        if ax == "mac-main.reader.keytips":
            return [{"id": "earlier", "tooltip": catalogue("Expand all messages", "en") + ("" if self.m == "tip-no-key-earlier" else "  ⇧O")},
                    {"id": "links", "tooltip": catalogue("Next link or date", "en") + ("" if self.m == "tip-no-key-link" else "  Tab")}]
        if ax == "mac-shortcuts.dispatch":
            return [{"command": c, "outcome": o} for c, o in self.log] or [{"command": "-", "outcome": "-"}]
        if ax == "mac-main.reader.links":
            if self.surface != "mac-reader":
                raise Unreadable("no thread open")
            return [{"ring": self.cursor == i} for i in range(self.ITEMS)] if (self.cursor is not None or self.log) else []
        if ax == "mac-main.reader.perf":
            return [{"body_reads": self.reads, "body_loads": 0}]
        raise Unreadable("the model has no region %s" % ax)


def run_unsub(mutant):
    """The triage-keys round on its stand-in or its broken source copy (only the mutants it declares are applied to it)."""
    run, _ = selftest_unsub()
    names = {m for _, m in R14_MUTANTS}
    return run(mutant if mutant in names else None)


ATT = "mac-main.attachments"
BODY_DOOR = "mac-main.reader.body"


def sweep_round(j):
    """The sweep: every control of the reader that no round above clicks, each clicked once, with the outcome the person sees.
    The header icons, the strip, the chevrons and header lines, the sender, the code chip, the earlier rule, the reply block, the unsubscribe
    doors and the link chips are clicked in the rounds above; this step adds the body's three dots, its older show-quoted door, the
    attachment tile, the pictures line (the sentence, Load once, Always), and the colours door. Findings (no door or no row): see docs/lanes/sweep-reader.md."""
    s = j.step("par-reader-sweep", "Every control of the reader clicked once: the three dots, the attachment tile, the pictures line, the colours door")
    body = lambda: s.ctl.rows(BODY_DOOR)[0]
    line = lambda: s.ctl.rows("mac-main.reader.pictures")[0]
    note = lambda: s.ctl.rows("mac-main.reader.trackers")[0]["explain_open"]
    open_row(s, FOLD_ROW)
    s.click("mac-main.reader.body.dots")
    s.expect("dots-pill-click-opens-the-folded-part", lambda: body()["fold_open"], True)
    s.click("mac-main.reader.body.dots")
    s.expect("dots-pill-click-again-folds-it", lambda: body()["fold_open"], False)
    open_row(s, FOLD_ROW)                                            # a fresh viewing: a pill that left the fold open does not carry into the next case
    s.click("mac-main.reader.body.show-quoted")
    s.expect("show-quoted-button-opens-the-folded-part", lambda: body()["fold_open"], True)
    s.click("mac-main.reader.body.show-quoted")
    s.expect("show-quoted-button-again-folds-it", lambda: body()["fold_open"], False)
    s.click(ATT + ".0")
    s.expect("attachment-tile-click-selects-the-file", lambda: s.ctl.rows(ATT)[0]["selected"], True)
    # a file that holds the keys (selected, or its preview open) takes Escape before the reader pane does, pane focused (lead, 13 Nov: the pane's
    # Escape left the mail with the preview still open behind the list)
    s.click("mac-main.pane.reader")
    s.key("Space")
    s.expect("space-on-the-selected-file-opens-its-preview", preview_is(s, True), True)
    s.key("Escape")
    s.expect("escape-closes-the-preview-and-keeps-the-mail", lambda: (preview_is(s, False)(), s.ctl.uistate()["surface"]), (False, "mac-reader"))
    s.key("Escape")
    s.expect("a-second-escape-lets-go-of-the-file-and-keeps-the-mail", lambda: (s.ctl.rows(ATT)[0]["selected"], s.ctl.uistate()["surface"]), (False, "mac-reader"))
    s.click("mac-main.reader.body.sample.designed")                  # the one fixture mail with blocked pictures (the fixture core's drive seam)
    s.click("mac-main.reader.trackers.button")
    s.expect("pictures-sentence-click-opens-its-note", note, True)
    s.click("mac-main.reader.trackers.button")
    s.expect("pictures-sentence-click-again-closes-its-note", note, False)
    s.click("mac-main.reader.pictures.load-once")
    s.expect("load-once-click-loads-this-viewings-pictures", lambda: (line()["loaded"], line()["list"]), (True, "pictures"))
    s.click("mac-main.reader.body.sample.designed")                  # a fresh viewing: Load once used the line up
    s.click("mac-main.reader.pictures.always")
    s.expect("always-click-remembers-the-sender-and-loads", lambda: (line()["always"], line()["loaded"]), (True, True))
    s.click("mac-main.reader.pictures.always.forget")                # put it back: no Always left behind
    s.click("mac-main.reader.body.sample.designed")
    s.click("mac-main.reader.body.colours")
    s.expect("colours-door-switches-to-the-app-colours", lambda: (body()["colours"], body()["sheet"]), ("app", False))
    s.click("mac-main.reader.body.colours")
    s.expect("colours-door-again-restores-the-original", lambda: (body()["colours"], body()["sheet"]), ("original", True))
    s.key("Escape")
    s.key("Escape")


class SweepModel:
    """The reader's body controls on the wire for --selftest (sweep_round). A dead click does nothing, as a button wired to no door."""
    def __init__(self, mutant=None):
        self.m = mutant
        self.surface, self.fold, self.sel, self.ql = "mac-main", False, False, False
        self.explain, self.loaded, self.always, self.colours = False, False, False, "original"

    def send(self, verb, *args):
        return {}

    def key(self, chord):
        m = self.m
        if chord == "Space" and self.sel and m != "space-dead":
            self.ql = True
        elif chord == "Escape":
            if self.ql:                      # the preview takes it; the mutant is the reader pane taking it first (the mail closes, the preview stays)
                if m == "escape-leaves-the-mail":
                    self.surface = "mac-main"
                else:
                    self.ql = False
            elif self.sel:                   # then the selected file lets go; the mutant leaves the mail instead
                if m == "second-escape-leaves":
                    self.surface = "mac-main"
                else:
                    self.sel = False
            else:
                self.surface = "mac-main"
        return {}

    def uistate(self):
        return {"surface": self.surface}

    def click(self, ax):
        m = self.m
        if ax == "mac-main.reader.open":
            self.surface, self.fold, self.sel, self.ql = "mac-reader", False, False, False
        elif ax == "mac-main.reader.body.dots" and m != "dots-dead":
            self.fold = True if (m == "dots-open-only") else not self.fold
        elif ax == "mac-main.reader.body.show-quoted" and m != "show-quoted-dead":
            self.fold = True if (m == "show-quoted-open-only") else not self.fold
        elif ax == ATT + ".0" and m != "tile-dead":
            self.sel = True
        elif ax == "mac-main.reader.body.sample.designed":
            self.explain, self.loaded, self.always, self.colours = False, False, False, "original"
        elif ax == "mac-main.reader.trackers.button" and m != "sentence-dead":
            self.explain = True if (m == "sentence-open-only") else not self.explain
        elif ax == "mac-main.reader.pictures.load-once" and m != "load-once-dead":
            self.loaded = True
        elif ax == "mac-main.reader.pictures.always" and m != "always-dead":
            self.always, self.loaded = True, True
        elif ax == "mac-main.reader.pictures.always.forget":
            self.always = False
        elif ax == "mac-main.reader.body.colours" and m != "colours-dead":
            self.colours = "app" if (self.colours == "original" or m == "colours-app-only") else "original"
        return {}

    def rows(self, ax):
        if ax == BODY_DOOR:
            return [{"fold_open": self.fold, "colours": self.colours, "sheet": self.colours == "original"}]
        if ax == ATT:
            return [{"selected": self.sel}]
        if ax == ATT + ".panel":
            return [{"open": self.ql}]
        if ax == "mac-main.reader.trackers":
            return [{"explain_open": self.explain}]
        if ax == "mac-main.reader.pictures":
            return [{"loaded": self.loaded, "list": "pictures" if self.loaded else "none", "always": self.always}]
        raise Unreadable("the model has no region %s" % ax)


R16_MUTANTS = [   # sweep_round: (check label, the SweepModel's mutant that must turn it red)
    ("par-reader-sweep/dots-pill-click-opens-the-folded-part", "dots-dead"),
    ("par-reader-sweep/dots-pill-click-again-folds-it", "dots-open-only"),
    ("par-reader-sweep/show-quoted-button-opens-the-folded-part", "show-quoted-dead"),
    ("par-reader-sweep/show-quoted-button-again-folds-it", "show-quoted-open-only"),
    ("par-reader-sweep/attachment-tile-click-selects-the-file", "tile-dead"),
    ("par-reader-sweep/space-on-the-selected-file-opens-its-preview", "space-dead"),
    ("par-reader-sweep/escape-closes-the-preview-and-keeps-the-mail", "escape-leaves-the-mail"),
    ("par-reader-sweep/a-second-escape-lets-go-of-the-file-and-keeps-the-mail", "second-escape-leaves"),
    ("par-reader-sweep/pictures-sentence-click-opens-its-note", "sentence-dead"),
    ("par-reader-sweep/pictures-sentence-click-again-closes-its-note", "sentence-open-only"),
    ("par-reader-sweep/load-once-click-loads-this-viewings-pictures", "load-once-dead"),
    ("par-reader-sweep/always-click-remembers-the-sender-and-loads", "always-dead"),
    ("par-reader-sweep/colours-door-switches-to-the-app-colours", "colours-dead"),
    ("par-reader-sweep/colours-door-again-restores-the-original", "colours-app-only"),
]


SEARCH_J2 = os.path.join(ROOT, "fixtures", "tests-j2.json")
SEARCH_QUERY = 'from:"Linea Supply" has:attachment'


def search_escape_round(j):
    """A mail opened from a search: Escape reaches the open reply field FIRST (it closes the reply and keeps the reader), and only the second Escape goes back to
    the results. Every focused feature is asked for a key in registry order and the reader comes before the quick reply; the reader's Escape (back to the
    results) used to take the key while the reply field held the keys, so the typed reply stayed open behind the results (hid-compose, J1.6d)."""
    s = j.step("par-reader-search-escape", "Open a result, close the reply with Escape, then go back to the results")
    s.raw("fixture", "corpus", SEARCH_J2)
    s.click("mac-search.bar")
    s.key("⌘A")
    s.type(SEARCH_QUERY)
    s.key("Enter")
    s.key("Down")
    s.arrive(("key", "Enter"), "mac-reader")
    reply = lambda: len(s.ctl.rows("mac-main.quickreply.field"))
    def reply_open():
        # Enter on a result opens the reply field focused (A102); if this build left it closed, R opens it (the field takes no R while it is closed)
        if reply() == 0:
            s.ctl.key("R")
        return reply()
    s._act(reply_open)
    s.expect("reply-open-on-a-result", reply, 1)
    s.key("Escape")
    s.expect("escape-closes-the-reply-and-keeps-the-reader", lambda: (s.ctl.uistate().get("surface"), reply()), ("mac-reader", 0))
    s.key("Escape")
    s.expect("second-escape-goes-back-to-the-results", lambda: s.ctl.uistate().get("surface"), "mac-search")


class SearchEscapeModel:
    """The search and the reader on the wire for --selftest (search_escape_round). Enter on a result opens the reader with the reply open; Escape with the reply
    open closes it, with it closed returns to the results. Mutants: `reader-first` the reader takes Escape first (back to the results with the reply still
    open, the defect), `enter-dead` Enter opens nothing, `no-reply` the reply never opens, `second-escape-stays` the Escape after the reply closed does nothing."""
    def __init__(self, mutant=None):
        self.m, self.surface, self.reply = mutant, "mac-main", False

    def send(self, verb, *args):
        return {}

    def click(self, ax):
        return {}

    def type(self, text):
        return {}

    def key(self, chord):
        if chord == "⌘A":
            self.surface = "mac-search"
        elif chord == "Enter" and self.surface == "mac-search" and self.m != "enter-dead":
            self.surface, self.reply = "mac-reader", self.m != "no-reply"
        elif chord == "Escape" and self.surface == "mac-reader":
            if self.m == "reader-first":
                self.surface = "mac-search"
            elif self.reply:
                self.reply = False
            elif self.m != "second-escape-stays":
                self.surface = "mac-search"
        return {}

    def rows(self, ax):
        if ax == "mac-main.quickreply.field":
            return [{"ax": ax}] if self.reply else []
        raise Unreadable("the model has no region %s" % ax)

    def uistate(self):
        return {"surface": self.surface}


R17_MUTANTS = [   # search_escape_round: (check label, the SearchEscapeModel's mutant that must turn it red)
    ("par-reader-search-escape/arrives-by-control", "enter-dead"),
    ("par-reader-search-escape/reply-open-on-a-result", "no-reply"),
    ("par-reader-search-escape/escape-closes-the-reply-and-keeps-the-reader", "reader-first"),
    ("par-reader-search-escape/second-escape-goes-back-to-the-results", "second-escape-stays"),
]


def selftest():
    """Every round-9 check green on the model, then red on its own mutant. The model covers the round-9 checks only; the
    older rounds drive the app (the socket) and have their mutants listed below."""
    def run(mutant):
        j = Journey(ID, Model(mutant))
        r9_round(j)
        r9_ring(j)
        r10_round(j)
        doors_round(j)
        doors_links(j)
        out = {c.label: c for c in j.checks()}
        jn = Journey(ID, NavModel(mutant))
        nav_round(jn)
        out.update({c.label: c for c in jn.checks()})
        jc = Journey(ID, ColoursModel(mutant))
        colours_round(jc)
        out.update({c.label: c for c in jc.checks()})
        out.update(run_unsub(mutant))
        jw = Journey(ID, SweepModel(mutant))
        sweep_round(jw)
        out.update({c.label: c for c in jw.checks()})
        global dots_sources
        real = dots_sources
        if mutant == "literal-o-back":
            dots_sources = lambda: (real()[0].replace("keyBindings() -> [String: (AppModel) -> Bool] {\n        [", "keyBindings() -> [String: (AppModel) -> Bool] {\n        [\n            \"O\": { _ in true },", 1), real()[1])
        jo = Journey(ID, OModel(mutant))
        o_round(jo)
        dots_sources = real
        out.update({c.label: c for c in jo.checks()})
        je = Journey(ID, SearchEscapeModel(mutant))
        search_escape_round(je)
        out.update({c.label: c for c in je.checks()})
        return out
    every = R9_MUTANTS + R10_MUTANTS + R11_MUTANTS + R12_MUTANTS + R13_MUTANTS + R14_MUTANTS + R15_MUTANTS + R16_MUTANTS + R17_MUTANTS
    clean, bad, labels = run(None), 0, [l for l, _ in every]
    declared = {l for l in declared_mutants(os.path.abspath(__file__)) if l.startswith(("par-reader-r9-", "par-reader-r10-", "par-reader-doors-", "par-reader-a129-", "par-reader-unsub2/", "par-reader-o/", "par-reader-sweep/", "par-reader-search-escape/"))}     # the live run's own accounting
    missing = sorted((set(clean) - set(labels)) | (set(clean) - declared))
    stray = sorted((set(labels) - set(clean)) | (declared - set(clean)))
    for l in missing:
        print("NO MUTANT %s" % l)
    for l in stray:
        print("STRAY MUTANT %s" % l)
    bad += len(missing) + len(stray)
    print("MUTANTS | check | clean | mutant | mutant result")
    for label, mutant in every:
        c = clean.get(label)
        red = run(mutant).get(label)
        ok_clean = c is not None and c.state == "pass"
        ok_red = red is not None and red.state == "fail"
        print("MUTANTS | %s | %s | %s | %s" % (label, "green" if ok_clean else "RED (%s)" % (c.detail if c else "absent"), mutant,
                                               "red" if ok_red else "STILL GREEN (%s)" % (red.state if red else "absent")))
        bad += (not ok_clean) + (not ok_red)
    bad += selftest_scroll()
    bad += selftest_o_evidence()
    print("%d checks, %d failed, 0 not judged" % (len(every), bad))
    return 1 if bad else 0


MUTANTS = """# MUTANTS
# par-reader-unsub2/command-u-with-no-confirm-sends-nothing | command U asks the engine to unsubscribe before the confirm
# par-reader-unsub2/the-confirm-calls-the-engine-once | the confirm asks the engine twice
# par-reader-unsub2/a-second-command-u-sends-nothing | a second command U on a list already left asks for the confirm again
# par-reader-unsub2/a-second-command-u-says-the-date | a second command U says "Unsubscribed" without the date
# par-reader-unsub2/a-web-link-opens-nothing-before-the-confirm | the web page opens when command U is pressed
# par-reader-unsub2/a-web-link-opens-its-page-after-the-confirm | the confirm marks the list but never opens the page
# par-reader-unsub2/a-web-link-reaches-nothing-outside-this-mac | the confirm calls the workspace for the web address in a test copy (the rig refuses it and counts it)
# par-reader-unsub2/command-u-reads-the-core-mark-before-it-arms | the ask never reads the core's mark (source copy)
# par-reader-unsub2/command-u-itself-never-asks-the-engine | the ask itself sends the unsubscribe (source copy)
# par-reader-unsub2/the-one-click-request-runs-off-the-main-actor | the one-click request runs on the main actor (source copy)
# par-reader-doors-header/inspector-button-opens-the-inspector | the label inspector's button does nothing
# par-reader-doors-header/inspector-button-closes-it-again | the button only opens the inspector, never closes it
# par-reader-doors-header/strip-reply-all-opens-the-reply-box-with-the-header-filled | the strip's Reply all button opens no reply box
# par-reader-doors-header/strip-forward-opens-the-forward-window | the strip's Forward button opens no forward window
# par-reader-doors-reply/reply-block-opens-the-quick-reply | a click on the reply block does nothing
# par-reader-doors-reply/header-door-expands-the-open-message-header | the header door (the key's act) does nothing
# par-reader-doors-reply/header-door-collapses-it-again | the header door only expands
# par-reader-doors-reply/earlier-rule-click-flips-the-fold | a click on the earlier-messages rule does nothing
# par-reader-doors-reply/earlier-rule-click-again-restores-it | the rule only unfolds, never folds
# par-reader-doors-reply/delete-door-deletes-in-core | the delete door does nothing
# par-reader-doors-reply/delete-door-returns-to-the-list | the reader stays open after the delete door
# par-reader-doors-reply/delete-door-undo-restores-the-message | Z does not bring the deleted message back
# par-reader-doors-reply/backspace-deletes-no-mail | plain Backspace deletes the open mail
# par-reader-doors-reply/shift-hash-deletes-the-open-message | Shift+# is not bound in the reader
# par-reader-doors-reply/enter-on-the-focused-row-opens-the-quick-reply | Enter with the pane holding the keys does nothing
# par-reader-doors-links/a-link-chip-click-opens-the-link | a click on a link chip does nothing
# par-reader-doors-unsubscribe/ask-door-arms-the-confirm | the ask door does nothing on a list message
# par-reader-doors-unsubscribe/ask-door-names-the-method | the confirm names another method than the list's
# par-reader-doors-unsubscribe/cancel-door-clears-the-confirm | the cancel door leaves the confirm up
# par-reader-doors-unsubscribe/ask-door-refuses-a-message-with-no-list | the ask door arms a confirm for a message with no list
# par-reader-r10-live/toggle-message-header-is-live-on-shift-h | the header command is a stub again, or its key moved
# par-reader-r10-header/click-on-the-header-line-expands-its-message | a click on the header line does nothing
# par-reader-r10-header/expanded-header-states-the-attachment-count | the expanded header drops the attachments row
# par-reader-r10-header/click-on-the-header-line-again-collapses-it | a click on the line only expands
# par-reader-r10-header/collapsed-header-shows-sender-and-time-only | a collapsed header keeps its rows
# par-reader-r10-header/key-expands-the-marked-header | the key does nothing
# par-reader-r10-header/key-again-collapses-the-marked-header | the key only expands
# par-reader-r10-header/state-is-kept-across-n-and-p | N or P closes the open headers
# par-reader-r10-header/state-is-per-message | a click on one header opens every header
# par-reader-r10-header/every-header-is-collapsed-again | a click on the line only expands
# par-reader-r10-header/shift-o-opens-every-header | shift O leaves the headers alone
# par-reader-r10-header/shift-o-again-closes-every-header | shift O only opens, never closes
# par-reader-r10-header/shift-o-with-one-header-open-opens-the-rest | shift O closes the headers as soon as one is open
# par-reader-r10-header/shift-o-closes-them-all-again | shift O only opens, never closes
# par-reader-r10-header/header-motion-is-opacity-and-transform-under-180-ms | the header moves over another duration or another property
# par-reader-r10-cost/toggling-headers-reads-no-body-from-the-core | a header toggle reads the body from the core again
# par-reader-r9-keys/reader-commands-are-live-with-their-v8-key | a reader command is a stub again, or its key moved
# par-reader-r9-keys/earlier-rule-tooltip-carries-its-key | the tooltip of the earlier-messages rule drops its key
# par-reader-r9-keys/link-chip-tooltip-carries-its-key | the tooltip of a link chip drops its key
# par-reader-r9-list/tab-in-the-list-is-next-split | the reader takes Tab while the list shows
# par-reader-r9-list/shift-tab-in-the-list-is-previous-split | the reader takes Shift Tab while the list shows
# par-reader-r9-cost/keys-make-the-reader-read-no-body-from-the-core | a key press reads the body from the core again
# par-reader-r9-ring/shift-tab-from-off-lands-on-the-last | Shift Tab walks forward
# par-reader-r9-ring/shift-tab-walks-back-one | Shift Tab walks forward
# par-reader-r9-ring/tab-after-shift-tab-moves-forward | Tab after Shift Tab starts again at the first item
# par-reader-r9-ring/shift-tab-reaches-the-first | Shift Tab stops one short of the first item
# par-reader-r9-ring/shift-tab-before-the-first-takes-the-ring-off | Shift Tab before the first item wraps to the last
# par-reader-header-toggle/header-account-colour-is-the-list-rows | the header serves another account's colour (a literal, or the filter's first account)
# par-reader-header-toggle/chevron-on-every-message | an earlier message has no chevron
# par-reader-header-toggle/every-header-starts-collapsed | the open message starts expanded
# par-reader-facts/headers-start-collapsed-on-first-open | the open message of a thread opened for the first time starts expanded
# par-reader-header-toggle/the-preset-binds-shift-h | the preset row is missing
# par-reader-header-toggle/tooltip-says-expand-with-the-key | the tooltip drops the key
# par-reader-header-toggle/chevron-expands-only-its-message | the chevron does nothing
# par-reader-header-toggle/expanded-rows-are-from-to-date | the To row is dropped
# par-reader-header-toggle/from-is-name-and-address | From shows the name only
# par-reader-header-toggle/to-lists-every-recipient | To keeps the first recipient only
# par-reader-header-toggle/cc-slot-waits-for-its-field | a Cc row is drawn from another field, or the reason is dropped
# par-reader-header-toggle/date-is-full-with-time-and-zone | the date has no zone
# par-reader-header-toggle/tooltip-says-collapse-with-the-key | the tooltip still says expand when open
# par-reader-header-toggle/shift-h-expands-the-marked-message | shift H expands the open message, not the marked one
# par-reader-header-toggle/shift-h-again-collapses-it-and-keeps-the-other | shift H collapses every header
# par-reader-header-toggle/chevron-again-collapses | the chevron only expands
# par-reader-header-toggle/chevron-has-a-frame-for-the-key-overlay | the chevron has no whooshAx id
# par-reader-header-toggle/strip-control-has-a-frame-for-the-key-overlay | the strip buttons have no whooshAx id
# par-reader-header-toggle/shift-h-passes-on-in-the-list | shift H is taken with no thread open
# par-reader-hold/no-caps-before-the-hold | the caps show with no hold
# par-reader-hold/hold-shows-a-cap-under-every-icon | the hold shows no cap in the reader
# par-reader-hold/each-cap-is-its-icons-preset-key | a cap shows a fixed key, not the preset's
# par-reader-hold/release-hides-the-caps | the caps stay after the release
# par-reader-not-done/archived-in-core | the archive button does nothing
# par-reader-not-done/g-then-e-lists-the-archived-thread | the Archive view leaves the archived thread out (list lane's)
# par-reader-not-done/shift-e-unarchives-in-core | shift E does nothing
# par-reader-not-done/the-list-says-back-in-the-list | the capsule says nothing after shift E
# par-reader-unsubscribe/command-u-passes-on-for-a-non-list-message | command U is taken on a message with no list
# par-reader-unsubscribe/command-u-asks-first | command U unsubscribes with no confirm
# par-reader-unsubscribe/nothing-left-before-the-confirm | command U leaves the list before the confirm
# par-reader-unsubscribe/the-confirm-names-the-method-and-host | the confirm does not say what happens
# par-reader-unsubscribe/cancel-leaves-nothing | cancel still unsubscribes
# par-reader-unsubscribe/confirm-unsubscribes-in-core | the confirm does nothing
# par-reader-unsubscribe/the-pane-says-unsubscribed | the note is missing
# par-reader-unsubscribe/a-web-link-list-says-the-browser-opens | every list is called one-click
# par-reader-unsubscribe/escape-cancels-the-confirm | Escape leaves the confirm up
# par-reader-width/summary-fills-to-the-inset-at-1280 | the summary callout is capped at a fixed width
# par-reader-width/message-fills-to-the-offset-at-1280 | the message block is capped at a fixed width
# par-reader-width/summary-fills-to-the-inset-at-960 | as above, narrow pane
# par-reader-width/message-fills-to-the-offset-at-960 | as above, narrow pane
# par-reader-width/summary-fills-to-the-inset-sidebar-hidden | the blank band returns when the sidebar hides
# par-reader-width/message-fills-to-the-offset-sidebar-hidden | the blank band returns when the sidebar hides
# par-reader-width/pane-is-wider-with-the-sidebar-hidden | hiding the sidebar does not widen the reader
# par-reader-foot/no-reader-foot-strip-control | the reader's own key strip is back under the empty state
# par-reader-foot/blocks-under-the-text-wait-for-the-text | the blocks under the text are drawn before the text has its height: they stand under the header, then jump down
# par-reader-foot/reader-content-ends-at-the-foot-strip | the reader keeps a second foot strip's room: its content ends 36 pt above the strip
# par-reader-rest/thread-shows-before-enter | the reader stays empty until Enter
# par-reader-rest/actions-sit-in-the-header | the actions stay in the foot strip
# par-reader-rest/summary-callout-shows-the-core-summary | the summary callout is missing or shows other words than the core's
# par-reader-rest/attachment-card-kind | the attachment is a bare chip with no kind
# par-reader-facts/arrives-by-control | the open click does nothing
# par-reader-facts/earlier-authors-from-core | earlier messages show no author
# par-reader-facts/every-message-has-a-time | the time is left empty
# par-reader-facts/attachment-names | the attachment tile shows a count, not the names
# par-reader-facts/sender-address-shown | the address under the sender is dropped
# par-reader-quick/quick-reply-mounted | the bar is not mounted under the thread
# par-reader-quick/quick-reply-closes-on-escape | Escape leaves the bar open
# par-reader-quick/escape-returns-to-list | the second Escape keeps the reader
# par-reader-code/arrives-by-control | the open click does nothing
# par-reader-code/code-chip-shows-code | the chip is missing
# par-reader-code/code-on-clipboard | the click copies nothing
# par-reader-code/chip-says-copied | the chip keeps saying Copy
# par-reader-code/message-marked-read | the click does not mark read
# par-reader-sender/sender-card-counts-messages | the contact has no messages
# par-reader-sender/arrives-by-control | the sender is not a control
# par-reader-sender/card-is-the-sender | the card opens for another address
# par-reader-delete/message-deleted-in-core | Delete does nothing
# par-reader-delete/delete-returns-to-list | the reader stays open after Delete
# par-reader-delete/undo-words-follow-a-language-switch | the reader passes translated text to the capsule (the words stay English)
# par-reader-delete/delete-offers-undo | Delete leaves no undo capsule
# par-reader-delete/undo-brings-the-message-back | Z does not restore the message
# par-reader-strip/strip-has-five-controls-in-order | the archive button is missing or the order changes
# par-reader-strip/strip-keys-come-from-the-preset | the strip shows a fixed key, not the preset's
# par-reader-strip/strip-runs-engine-commands | a button runs another command than its key
# par-reader-strip/strip-all-apply-to-an-open-message | a button is drawn greyed on an open message
# par-reader-strip/strip-reveals-within-the-reveal-time | the reveal uses another duration
# par-reader-strip/strip-greys-delete-and-reply-under-a-reply | the strip stays live under an open quick reply
# par-reader-strip/greyed-delete-does-nothing | a greyed delete still deletes
# par-reader-strip/reply-all-and-forward-are-live-doors | reply all or forward is drawn greyed again, or names no live door
# par-reader-strip/strip-archive-archives-in-core | the archive button does nothing
# par-reader-header/header-archive-archives | the header archive icon does nothing
# par-reader-header/header-pin-pins | the header pin icon presses P and steps to the previous message
# par-reader-header/header-reply-opens-the-quick-reply | the header reply icon does nothing
# par-reader-header/header-snooze-opens-the-picker | the header snooze icon does nothing
# par-reader-header/header-label-opens-the-picker | the header label icon does nothing
# par-reader-header/header-more-opens-the-palette | the header more icon does nothing
# par-reader-keys/opened-on-the-three-message-thread | the reader opens with a mark already set
# par-reader-keys/n-at-the-last-message-stays | N past the last message moves the mark
# par-reader-keys/p-marks-the-message-before | P does not move the mark or leaves the rule folded
# par-reader-keys/only-the-marked-message-is-marked | every message is marked
# par-reader-keys/p-at-the-first-message-stays | P at the first message pins the message
# par-reader-keys/n-moves-forward | N does not move the mark
# par-reader-keys/o-expands-the-marked-message | O does nothing
# par-reader-keys/expanded-flag-on-the-row | the thread rows do not say which message is expanded
# par-reader-keys/o-again-folds-it | O only expands, never folds
# par-reader-o/o-on-the-open-message-passes-on | O on an open mail without a fold is taken
# par-reader-o/the-dots-key-is-the-presets-expand-message-key | the pill shows a literal key, not the preset's
# par-reader-o/o-on-a-folded-mail-toggles-the-dots | O on a mail with a fold does nothing
# par-reader-o/o-on-a-folded-mail-leaves-the-text-where-it-is | opening the dots moves the written text
# par-reader-o/the-dots-path-has-no-literal-o-chord | the dots are bound to the literal chord O again, or expand-message no longer reaches them
# par-reader-o/o-again-folds-the-dots | O only opens the dots
# par-reader-keys/shift-o-expands-every-message | shift O does nothing
# par-reader-keys/shift-o-again-folds-them-and-the-rule | shift O only expands, never folds
# par-reader-keys/keys-do-nothing-in-the-list | N is taken while no thread is open
# par-reader-scroll/content-taller-than-the-pane | the scroll region reports no content height
# par-reader-scroll/r-scrolls-to-the-new-reply-card | R leaves the thread where it was, or short of the end, so the new reply card is not in view
# par-reader-scroll/space-scrolls-down-a-page | space does not scroll
# par-reader-scroll/shift-space-scrolls-back | shift space does not scroll back
# par-reader-scroll/command-down-jumps-to-the-bottom | command down does not reach the bottom
# par-reader-scroll/space-at-the-bottom-passes-on-or-previews-the-first-file | space at the bottom is taken on a mail with no file, or opens no preview on a mail with files
# par-reader-scroll/command-up-jumps-to-the-top | command up does not reach the top
# par-reader-scroll/shift-space-at-the-top-passes-on | shift space at the top is taken
# par-reader-focus/a-move-leaves-the-reader-on-the-old-thread | the reader follows a move on the same frame
# par-reader-focus/the-old-thread-is-still-on-screen | the reader follows a move on the same frame
# par-reader-focus/the-settled-move-is-followed | the reader never follows a settled move
# par-reader-focus/and-draws-the-new-thread | the reader keeps the old thread after the settle
# par-reader-focus/settle-and-cross-fade-times-are-the-constants | the settle time is another number
# par-reader-focus-cost/25-presses-build-at-most-two-threads | every press builds a thread
# par-reader-focus-cost/25-presses-cross-fade-once | every press cross-fades
# par-reader-focus-cost/25-presses-load-no-body | the list focus loads bodies
# par-reader-a129-colours/list-focus-shift-i-filters | shift I in the list switches colours instead of filtering
# par-reader-a129-colours/list-focus-shift-i-leaves-the-colours | shift I in the list changes the open mail's colours
# par-reader-a129-colours/reader-focus-shift-i-switches | shift I in the reader runs the list's filter
# par-reader-a129-colours/reader-focus-shift-i-changes-the-colours | shift I in the reader leaves the colours
# par-reader-a129-colours/reader-focus-shift-i-again-restores-them | shift I in the reader only switches one way
# par-reader-a129-colours/return-opened-reader-shift-i-switches | shift I after Return needs a click on the reader pane first
# par-reader-a129-colours/shift-i-in-the-list-filters-whatever-the-pane-focus-was | the reader pane's stale focus makes shift I switch colours in the list
# par-reader-a129-colours/foot-strip-after-return-and-escape-is-the-list-set | the foot strip shows the composer's keys after the reply closed
# par-reader-a129-colours/foot-strip-in-the-open-mail-is-the-reader-set | the foot strip reads the pane focus, so a mail opened by a click on its row shows the list's keys while shift I and O take the reader's
# par-reader-a129-nav/25-presses-read-at-most-one-body | every press reads a body from the core
# par-reader-a129-nav/25-presses-build-no-body | a thread build reads the body again
# par-reader-a129-nav/20-moves-keep-every-turn-under-the-frame-budget | the key frame carries the build of the thread
# par-reader-a129-nav/20-moves-read-one-body-each-at-most | a move reads more than one body
# par-reader-a129-nav/20-moves-evaluate-the-pane-once-each-at-most | a move evaluates the pane more than once
# par-reader-a129-nav/20-moves-rebuild-only-the-rows-that-change | every row on screen is rebuilt on a one-row move
# par-reader-focus-cost/25-presses-evaluate-the-pane-at-most-twice | every press evaluates the pane
# par-reader-focus-open/11-presses-in-an-open-reader-load-one-body | every press loads a body
# par-reader-focus-open/the-open-reader-follows-the-settled-focus | the open reader stays on the first thread
# par-reader-links/no-ring-before-tab | the ring is on before Tab
# par-reader-links/tab-lists-the-links-and-dates-in-reading-order | Tab lists dates before links
# par-reader-links/tab-puts-the-ring-on-the-first | Tab starts at the second item
# par-reader-links/tab-moves-the-ring-on | Tab keeps the ring on the first item
# par-reader-links/tab-after-the-last-takes-the-ring-off | Tab after the last stays on the last
# par-reader-links/tab-then-starts-again | the ring never starts again
# par-reader-links/enter-opens-the-ringed-link | Enter with the ring on replies all
# par-reader-links/the-fixture-logs-and-starts-no-browser | the fixture starts a browser
# par-reader-links/enter-on-a-date-is-logged | Enter on a date is logged as a browser open
# par-reader-links/command-o-opens-the-first-link-and-the-attachments | command O logs no attachments
# par-reader-links/a-chip-click-opens-that-item | a chip click does nothing
# par-reader-strip/strip-keys-in-tooltips | a tooltip lacks its key
# par-reader-strip/strip-hidden-while-the-list-holds-the-keys | the strip shows with no hover and no focus
# par-reader-strip/strip-shows-when-the-pane-holds-the-keys | the strip stays hidden under keyboard focus
# par-reader-strip/strip-reply-opens-the-quick-reply | the reply button does nothing
# par-reader-strip/strip-delete-deletes-in-core | the trash button does nothing
# par-reader-strip/strip-delete-returns-to-list | the reader stays open after the trash button
# par-reader-strip/strip-delete-offers-undo | the trash button leaves no undo
# par-reader-sweep/dots-pill-click-opens-the-folded-part | a click on the three-dot pill does nothing
# par-reader-sweep/dots-pill-click-again-folds-it | the pill only opens, never folds
# par-reader-search-escape/arrives-by-control | Enter on a result opens no reader
# par-reader-search-escape/reply-open-on-a-result | the reply field is not open under a mail opened from a search
# par-reader-search-escape/escape-closes-the-reply-and-keeps-the-reader | Escape goes back to the results while the reply field is open, the typed reply stays open behind them
# par-reader-search-escape/second-escape-goes-back-to-the-results | the Escape after the reply closed leaves the reader where it is
# par-reader-sweep/show-quoted-button-opens-the-folded-part | the show-quoted door does nothing
# par-reader-sweep/show-quoted-button-again-folds-it | the show-quoted door only opens
# par-reader-sweep/attachment-tile-click-selects-the-file | a click on the attachment tile selects nothing
# par-reader-sweep/space-on-the-selected-file-opens-its-preview | Space on a selected file opens no preview
# par-reader-sweep/escape-closes-the-preview-and-keeps-the-mail | the reader pane takes Escape before the open preview (the mail closes, the preview stays)
# par-reader-sweep/a-second-escape-lets-go-of-the-file-and-keeps-the-mail | the reader pane takes Escape before the selected file
# par-reader-sweep/pictures-sentence-click-opens-its-note | a click on the pictures sentence opens no note
# par-reader-sweep/pictures-sentence-click-again-closes-its-note | the sentence only opens its note
# par-reader-sweep/load-once-click-loads-this-viewings-pictures | Load once does nothing
# par-reader-sweep/always-click-remembers-the-sender-and-loads | Always does nothing
# par-reader-sweep/colours-door-switches-to-the-app-colours | the colours door does nothing
# par-reader-sweep/colours-door-again-restores-the-original | the colours door only switches one way
"""


def main():
    if "--selftest" in sys.argv:
        return selftest()
    path = os.path.abspath(__file__)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    rows = [l[1:].strip().split(" | ", 1) for l in MUTANTS.splitlines() if l.startswith("# par-reader") and " | " in l]
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
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    ok = r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"]
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
