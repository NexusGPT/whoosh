#!/usr/bin/env python3
"""par-quickreply: the GotMail parity row "Emoji in replies" (research/gotmail.md:41): the quick reply's emoji
button opens a row of reactions, a pick lands in the reply text, and the reply with it goes to the outbox.

Drives the RUNNING rig copy over the control socket through journeys/lib, like checks/quickreply-check.py.
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/par-quickreply.py'
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0.
Round 5 (drive_lift): the lifted-glass slab and cards, the header, the picker's key, the failed card, the landing, the pop-out.
Round 9 (lane doors-compose, step par-quickreply-doors): one click on every control the rounds before did not click (the seven other reactions, the From
picker's filter and a row of the narrowed menu, the To chip's x, the Cci and Objet rows, the field after a header row, the pop-out button);
docs/lanes/doors-compose.md holds the control table.
    python3 checks/parity/par-quickreply.py --selftest   the round-9 checks against a model of the quick reply, clean (all green), then once per mutant (each must go red)
"""
import json
import os
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, print_cases, print_model_cases   # noqa: E402
from lib.ctl import Unreadable                                       # noqa: E402
from lib.fixture import const, FX                                   # noqa: E402
sys.path.insert(0, os.path.join(ROOT, "checks", "checklib"))
import resetguard as RG                                              # noqa: E402

ID = "par-quickreply"
TITLE = "Emoji in quick replies"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "the emoji button opens a row of reactions; a pick lands in the reply text and is sent with it; ⌘Enter sends and the send button shows it (⌃Enter and Enter still send, unshown)"
FIELD = "mac-main.quickreply.field"
PICKER = "mac-main.quickreply.emoji.picker"
SEND = "mac-main.quickreply.send"
UNDO_BAR = "mac-main.undo.bar"
UNDO = "mac-main.undo.bar.undo"
FROM = "mac-main.quickreply.from"
FROM_MENU = "mac-main.quickreply.from.menu"
RETRY = "mac-main.reply.retry"
SLAB = "mac-main.quickreply.slab"


def field(s, key):
    return s.ctl.rows(FIELD)[0][key]


def at_end(s, within_ms=400):
    """The thread is scrolled to its end. The reveal lands one main-actor turn after the key (the card must be laid
    out first), so the read polls for up to `within_ms`; a thread that never reaches its end stays False."""
    import time
    t0 = time.perf_counter()
    while True:
        r = s.ctl.rows("mac-main.reader.scroll")[0]
        if r["max_offset"] > 0 and abs(r["offset"] - r["max_offset"]) <= 2:
            return True
        if (time.perf_counter() - t0) * 1000 > within_ms:
            return False
        time.sleep(0.04)


PASTED = "sample-2-pages.pdf"                                       # the fixture file the rig's stage-file door puts on its pasteboard


def drive_paste(j):
    """Owner 2026-11-12 ("copy-paste of file should add attachment", a picture of a reply holding "Nexus-for-Telvora.pdf"): a file pasted
    into the reply is attached, never written as its name. The reply holds no files (QuickSend is text only), so it goes on in the full
    composer with the file on its tile row and the reply's words. The paste is the field's own ⌘V of the rig's PRIVATE pasteboard
    (`mac-main.quickreply.dev.paste`: the system's TextEditor would read the owner's pasteboard), staged as the Finder's Copy puts a file."""
    s = j.step("par-quickreply-paste", "A file pasted into the reply")
    s.key("R")
    s.type("Words before the file. ")
    s.click("mac-main.attachments.dev.stage-file")
    s.click("mac-main.quickreply.dev.paste")

    def landed():
        names = []
        for _ in range(10):                                          # the tile settles a turn after the attach: up to 10 reads over 1 s
            names = [r.get("name") for r in s.ctl.rows("mac-composer.attachments")]
            if PASTED in names:
                break
            time.sleep(0.1)
        text = (s.ctl.rows("mac-composer.body") or [{}])[0].get("text", "")
        return (PASTED in names, "Words before the file." in text, PASTED in text)
    s.expect("pasted-file-opens-the-composer-with-it", landed, (True, True, False))
    s.expect("pasted-file-name-is-not-in-the-reply", lambda: PASTED in ((s.ctl.rows(FIELD) or [{}])[0].get("text") or ""), False)
    s.key("⇧⌘,")                                                     # the composer's draft is thrown away: the steps after start clean
    s.key("Enter")

    # A picture copied as DATA (no file, no text) pasted on the reply: the field's TextEditor would insert nothing; the reply's paste
    # (QuickReplyPasteWatch -> takePastedPicture) goes on in the full composer with the picture IN ITS TEXT (lead ruling, round 7: picture
    # data pastes inline, a copied file is a tile), its file under the app's data folder.
    s.key("R")
    s.click("mac-main.attachments.dev.stage-image")
    s.click("mac-main.quickreply.dev.paste")

    def picture_landed():
        srcs = []
        for _ in range(20):                                          # written off the main thread, then placed: up to 20 reads over 2 s
            head = s.ctl.rows("mac-composer.header")
            srcs = [p for p in ((head[0].get("picture_sources") or []) if head else []) if PASTED_PICTURE in os.path.basename(p)]
            if srcs:
                break
            time.sleep(0.1)
        tile = any(str(r.get("name", "")).startswith(PASTED_PICTURE) for r in s.ctl.rows("mac-composer.attachments"))
        return (bool(srcs), not tile, bool(srcs) and in_pasted_files(srcs[-1]))
    s.expect("pasted-picture-opens-the-composer-with-it", picture_landed, (True, True, True))
    s.key("⇧⌘,")
    s.key("Enter")


PASTED_PICTURE = "Pasted image"                                      # the name a pasted picture is given (AttachmentIntake.writeImage, catalogue word)


def in_pasted_files(path):
    """True when `path` lies in the test copy's own `pasted-files/` folder (AttachmentPlaces.pastedRoot: the run folder on a rig copy)."""
    root = os.environ.get("WHOOSH_SOCKET_DIR", "")
    return bool(root) and os.path.abspath(path).startswith(os.path.join(os.path.abspath(root), "pasted-files") + os.sep)


def r_then_keys(s, name, typed, before=""):
    """Round 20 (owner 07:28: "when hitting R I should have cursor INSIDE the answer ... it stays on list"). A REAL R, then real keys: they
    land in the reply's text. Lead, 13 Nov (owner A162): R CONTINUES the conversation's saved draft (one reply draft per conversation), so
    with a draft card shown the field opens on the card's words and the typed keys follow them; the open thread's surface is the reader's.
    Digits: a key that reached the list instead would do nothing there, so a miss reads as an unchanged field, never as a moved list."""
    s.raw("keyevent", "R")
    s.raw("keyevent", typed)
    want = (1, before + typed, "mac-reader")

    def landed():
        got = (len(s.ctl.rows(FIELD)), field(s, "text"), s.ctl.uistate().get("surface"))
        return got if got == want else {"read (reply open, its text, surface)": got,
                                        "keyevent monitor": read_rows(s, "mac-keyevent.monitor")}
    s.expect(name, landed, want)
    if before:                                                   # only the typed keys go: Escape then keeps the draft as it was, on its card
        for _ in typed:
            s.raw("keyevent", "Delete")
    else:
        s.raw("keyevent", "\u2318A")
        s.raw("keyevent", "Delete")
    s.raw("keyevent", "Escape")                                  # an emptied reply closes and keeps nothing


def drive_keys_first(j):
    """The two "R, then real keys" cases, run FIRST. Lead, 13 Nov (journey record, finding 52): they send real key events through the
    application. In the hidden copy no window is ever the key window, and once a second window (the composer, opened by `drive_paste`) has
    been used the application no longer hands a plain key to the main window: the letters are lost, while the same key sent straight to
    the window lands (walks u4-131, u4-133: app_key_window none before and after). So these cases run before any composer use."""
    s = j.step("par-quickreply", "R, then real keys, land in the reply")
    s.key("Escape")
    s.click(DRAFT + ".0.delete")                                     # start from no draft (dry run: refused, harmless)
    s.key("R")
    s.type("Draft words.")
    s.key("Escape")                                                  # the words are the conversation's draft: its card shows
    try:
        card_words = s.ctl.rows(DRAFT)[0].get("lines") or ""
    except Exception:                                                # noqa: BLE001  dry run
        card_words = ""
    r_then_keys(s, "r-puts-the-cursor-in-the-reply-with-a-draft-card-shown", "123", before=card_words)
    s.click(DRAFT + ".0.delete")
    r_then_keys(s, "r-puts-the-cursor-in-the-reply-with-no-card", "456")
    # R on a LIST row, no mail open (owner 13 Nov 07:29: "it stays on list"): the conversation opens with the reply under it and the cursor in its text.
    s.raw("keyevent", "Escape")                                      # the reply above was emptied and closed: this Escape goes from the open mail back to the list
    s.expect("the-list-holds-the-keys-before-r", lambda: s.ctl.uistate().get("surface"), "mac-main")
    r_then_keys(s, "r-on-a-list-row-puts-the-cursor-in-the-reply", "789")


def drive(j):
    drive_stale_timer(j)                                             # first: its earlier send's timer is spent long before any later send
    drive_ai_open(j)                                                 # reply-thread: the seeded auto draft, then discarded
    drive_keys_first(j)                                              # before the composer window is ever opened (finding 52)
    s = j.step("par-quickreply", "Emoji in quick replies")
    RG.step_reset(s)
    s.key("R")
    s.type("Thanks ")
    s.click("mac-main.quickreply.emoji")
    s.expect("emoji-row-opens", lambda: len(s.ctl.rows(PICKER)) > 0, True)
    first = {}

    def pick():
        rows = s.ctl.rows(PICKER)
        first["emoji"] = rows[0]["emoji"]
        return rows[0]["ax"]
    s.expect("emoji-row-names-reactions", lambda: all(r.get("emoji") for r in s.ctl.rows(PICKER)), True)
    try:
        ax = pick()
    except Exception:                                                # noqa: BLE001  dry run or no picker: the checks below go red/unjudged
        ax = "mac-main.quickreply.emoji.0"
    s.click(ax)
    s.expect("emoji-lands-in-text", lambda: field(s, "text"), "Thanks " + first.get("emoji", "?"))
    s.expect("emoji-row-closes-after-pick", lambda: len(s.ctl.rows(PICKER)), 0)
    before = {}
    try:
        before["n"] = int(s.ctl.core("outbox_entry/count"))
    except Exception:                                                # noqa: BLE001  dry run: the check below is unjudged
        before["n"] = 0
    s.key("⌘Enter")
    s.expect("emoji-reply-queued", lambda: int(s.ctl.core("outbox_entry/count")) - before["n"], 1)
    # Round 3 (owner 2026-11-08 11:45: "the email sender account selector button isn't working"): the From selector.
    s.click(UNDO)                                                    # the text comes back, the field is open again
    st = {}
    s.expect("from-selector-shows-sender", lambda: st.setdefault("orig", s.ctl.rows(FROM)[0]["address"]) == field(s, "from"), True)
    s.click(FROM)
    s.expect("from-menu-opens-with-accounts", lambda: len(s.ctl.rows(FROM_MENU)) > 1, True)
    s.expect("from-menu-checks-current", lambda: [r["address"] for r in s.ctl.rows(FROM_MENU) if r["checked"] == "1"], [st.get("orig")])

    def other():
        rows = [r for r in s.ctl.rows(FROM_MENU) if r["checked"] != "1"]
        st["other"], st["orig_ax"] = rows[0]["address"], [r["ax"] for r in s.ctl.rows(FROM_MENU) if r["checked"] == "1"][0]
        return rows[0]["ax"]
    try:
        ax = other()
    except Exception:                                                # noqa: BLE001  dry run or no menu: the checks below go red/unjudged
        ax = "mac-main.quickreply.from.1"
    s.click(ax)
    s.expect("from-pick-switches-sender", lambda: (s.ctl.rows(FROM)[0]["address"], field(s, "from")), (st.get("other"), st.get("other")))
    s.expect("from-menu-closes-after-pick", lambda: len(s.ctl.rows(FROM_MENU)), 0)
    s.key("\u2303Enter")
    s.expect("send-uses-picked-account", lambda: s.ctl.rows(UNDO_BAR)[0]["sender"], st.get("other"))
    s.click(UNDO)
    s.click(FROM)
    s.click(st.get("orig_ax", "mac-main.quickreply.from.0"))         # back to the thread's own account
    s.expect("from-pick-back-switches-sender", lambda: s.ctl.rows(FROM)[0]["address"], st.get("orig"))
    # The sweep: every control of the quick reply, clicked once, with its visible outcome.
    s.click(SEND)
    s.expect("send-button-click-sends", lambda: len(s.ctl.rows(UNDO_BAR)), 1)
    s.click(UNDO)
    s.expect("undo-button-returns-text", lambda: field(s, "phase") == "editing" and field(s, "text") != "", True)
    s.raw("fixture", "fail-next", "quick-send", "the server refused the message")
    s.click(SEND)
    s.expect("send-failure-offers-retry", lambda: len(s.ctl.rows(RETRY)), 1)
    s.click(RETRY)
    # Lead, 13 Nov (owner picture 206: one object, shown once): after a failed send the box is folded and the Not sent card holds the words; Retry
    # queues them again and the card waits with its own Cancel. The reply is then opened again, with words, for the send keys below.
    s.expect("retry-button-sends", lambda: [r["state"] for r in s.ctl.rows(OUT)][-1:], ["waiting"])
    s.key("Z")
    s.key("R")
    s.type("Thanks again.")
    # Round 3: the send key (owner 2026-11-08 10:50: every bound key shows; owner 2026-11-11, spec A145: the shown key is ⌘↵). ⌃Enter and ⌘Enter both send; the button wears ⌘↵.
    s.expect("send-button-shows-key-cap", lambda: s.ctl.rows(SEND)[0]["cap"], "\u2318\u21b5")
    s.key("\u2303Enter")
    s.expect("ctrl-enter-sends", lambda: len(s.ctl.rows(UNDO_BAR)), 1)
    s.click(UNDO)
    s.key("\u2318Enter")
    s.expect("cmd-enter-still-sends", lambda: len(s.ctl.rows(UNDO_BAR)), 1)
    s.click(UNDO)
    drive_cards(j)


OUT = "mac-main.thread.outgoing"


def drive_stale_timer(j, sleep=None, now=None):
    """Lane send-timer (lead, 16 Nov; the picture check lost `sending` on main193 and main196): a send's undo timer acts for its own send only.
    The test engine numbers its outbox entries from 1 again after `fixture reset`, so the send after a reset can take the id of the send before
    it. The case sends, resets, sends again 3 s into the first window, and reads past the FIRST window's end (10.05 s after the first send,
    QuickReplyModel.send): the new reply is still open and counting, its own window runs to 13 s. Then Cancel. About 12 s of wall clock.
    `sleep` and `now` are the wall clock's; the self-test passes its model's clock."""
    sleep = sleep or time.sleep
    now = now or time.perf_counter
    window = const("UNDO_SEND_DEFAULT_S")
    s = j.step("par-quickreply-timer", "A send's undo timer acts for its own send only")
    st = {}

    def sent(n):
        try:
            st["thread%d" % n], st["id%d" % n] = field(s, "thread"), None
        except Exception:                                            # noqa: BLE001  dry run, or no reply open
            st["thread%d" % n] = None
        s.type("Timer words %d." % n)
        s.key("\u2318Enter")
        st["t%d" % n] = now()
        try:
            st["id%d" % n] = s.ctl.rows(OUT)[-1].get("id")
        except Exception:                                            # noqa: BLE001
            pass
    RG.step_reset(s)
    s.key("R")
    sent(1)
    s.raw("fixture", "reset")                                        # the engine's numbering starts again; the first send's timer is still armed
    sleep(max(0.0, st["t1"] + 3.0 - now()))
    s.key("R")
    sent(2)
    sleep(max(0.0, st["t1"] + window + 1.0 - now()))                # past the first window's end (+0.05 s), 2 s before the second's

    def still_counting():
        rows = s.ctl.rows(HEADER)
        got = (len(rows), rows[0].get("counting") if rows else None, len(s.ctl.rows(UNDO_BAR)))
        return got if got == (1, "1", 1) else {"read (header rows, counting, undo bars)": got, "read at s after send 1": round(now() - st["t1"], 2),
                                               "sends (thread, outbox id)": [(st.get("thread1"), st.get("id1")), (st.get("thread2"), st.get("id2"))]}
    s.expect("reply-keeps-its-window-after-a-reset-and-an-earlier-send", still_counting, (1, "1", 1))
    s.click(UNDO)                                                    # the second send is cancelled: nothing of this case is left in the outbox


def drive_cards(j):
    """Round 4 (owner 2026-11-08 17:19): the send shows up in the conversation at once, with its undo inside it."""
    s = j.step("par-quickreply", "Sent replies in the conversation")
    st = {}

    def outs():
        return s.ctl.rows(OUT)
    try:
        st["n0"] = len(outs())
    except Exception:                                                # noqa: BLE001  dry run: the checks below are unjudged
        st["n0"] = 0
    try:
        st["pre"] = field(s, "text")                                 # the field may still hold earlier words
    except Exception:                                                # noqa: BLE001  dry run
        st["pre"] = ""
    s.raw("resize", "960", "600")                                    # small window: the thread is longer than the pane
    s.type("Card text.")
    # Round 20 (owner 2026-11-13 07:27: "hitting enter actually sends", "not cmd+enter"): a REAL plain Return (the `keyevent` verb, an NSEvent
    # through the monitors and the field, never the socket's `key`) adds a line and does nothing else: the reply stays open with its words and a
    # line break, nothing is queued, no card, no draft saved. Then a real ⌘Return sends. On main115 the field's own Return handler sent.
    def thread_state():
        return {"undo_bars": len(s.ctl.rows(UNDO_BAR)), "cards": len(outs()) - st["n0"], "draft_cards": len(s.ctl.rows(DRAFT)),
                "drafts_count": draft_count(s), "open": len(s.ctl.rows(FIELD)), "text": field(s, "text")}
    try:
        st["before_return"] = thread_state()
    except Exception:                                                # noqa: BLE001  dry run: the case below is unjudged
        st["before_return"] = {}
    s.raw("keyevent", "Return")

    def after_return():
        now, was = thread_state(), st["before_return"]
        got = (now["text"] == st["pre"] + "Card text.\n", now["open"], now["undo_bars"], now["cards"], now["draft_cards"] - was.get("draft_cards", -1),
               now["drafts_count"] == was.get("drafts_count"))
        return got if got == (True, 1, 0, 0, 0, True) else {"read": got, "before": was, "after": now}
    s.expect("return-adds-a-line-and-sends-nothing", after_return, (True, 1, 0, 0, 0, True))
    s.raw("keyevent", "Delete")                                      # the line break goes again: the words sent below are the words typed
    s.raw("keyevent", "\u2318Return")
    s.expect("cmd-return-key-event-sends", lambda: (len(s.ctl.rows(UNDO_BAR)), len(outs()) - st["n0"]), (1, 1))

    s.expect("outgoing-card-scrolled-into-view", lambda: at_end(s), True)
    s.expect("outgoing-card-appears", lambda: len(outs()) - st["n0"], 1)
    s.expect("outgoing-card-waits-with-seconds", lambda: (outs()[-1]["state"], outs()[-1].get("seconds", 0) > 0), ("waiting", True))
    s.expect("outgoing-card-carries-text", lambda: outs()[-1]["text"].startswith(st["pre"] + "Card text."), True)
    s.expect("outgoing-card-undo-shows-key", lambda: outs()[-1].get("undo_cap"), "Z")

    def undo_ax():
        return outs()[-1]["undo"]
    try:
        ax = undo_ax()
    except Exception:                                                # noqa: BLE001
        ax = OUT + ".0.undo"
    s.click(ax)
    s.expect("card-undo-removes-card", lambda: len(outs()) - st["n0"], 0)
    s.expect("card-undo-restores-text", lambda: field(s, "text"), st["pre"] + "Card text.")
    s.key("\u2303Enter")
    s.raw("clock", "advance", const("UNDO_SEND_DEFAULT_S") + 1)
    s.raw("resize", "1280", "820")
    s.expect("card-becomes-sent-with-time", lambda: (outs()[-1]["state"], bool(outs()[-1].get("time"))), ("sent", True))
    drive_drafts(j)


DRAFT = "mac-main.thread.draft"
DRAFT_ADDRESS = DRAFT + ".0.address"                 # the address in the draft card's head (ThreadCardParts.swift ThreadCardHead)
DRAFT_YOU = DRAFT + ".0.you"                          # the "You" beside it: a plain text, so a missing address frame is told from a missing head
ADDRESS_MIN_PT = 48.0                                 # below this a cut address is a stray glyph or two, not a readable part (the app cuts at 96)


def address_fit(frame):
    """'fits' when the address is left out (its chooser 0 wide) or drawn at least ADDRESS_MIN_PT wide; else what was read. The id is on the
    chooser (ThreadCardHead `marked`), so a missing frame is a missing id, never read as hidden (main93 passed 960 on no frame at all)."""
    if frame is None:
        return "no frame for the address: its id is not drawn"
    return "fits" if frame["w"] < 0.5 or frame["w"] >= ADDRESS_MIN_PT else "address drawn %.1f pt wide (a stray glyph)" % frame["w"]
DELETED = "mac-main.thread.draft.deleted"


def read_rows(s, ax):
    """The rows of `ax` for a miss's words; the reason when they cannot be read."""
    try:
        return s.ctl.rows(ax)
    except Exception as e:                                           # noqa: BLE001
        return "unread: %s" % e


def draft_count(s):
    """The engine's count of saved drafts (`core state draft/count`, the number the sidebar's Drafts row shows)."""
    return int(s.ctl.core("draft/count"))


def drive_drafts(j):
    """Round 4 (owner 17:19): a ready draft shows in the conversation, opens on a click, sends, and deletes with undo."""
    s = j.step("par-quickreply", "Drafts in the conversation")

    def draft_rows():
        return s.ctl.rows(DRAFT)

    def list_draft():
        rows = [r for r in s.ctl.rows("mac-main.list") if r.get("focused")]
        return rows[0].get("draft") if rows else None
    s.key("Escape")                                                  # whatever the field holds becomes the draft
    s.click(DRAFT + ".0.delete")                                     # start from no draft (dry run: refused, harmless)
    s.key("R")
    s.type("Draft words.\nSecond line.")
    s.key("Escape")
    s.expect("esc-keeps-words-as-draft-card", lambda: len(draft_rows()), 1)
    s.expect("draft-card-shows-first-lines", lambda: draft_rows()[0]["lines"], "Draft words.\nSecond line.")
    s.expect("draft-card-keys-shown", lambda: (draft_rows()[0]["send_cap"], draft_rows()[0]["delete_cap"]), ("\u2318\u21b5", "\u2318\u232b"))   # owner 13 Nov: "cmd+del not just del it would clash"
    s.expect("draft-card-is-lifted-glass", lambda: draft_rows()[0]["material"], "lifted")   # A147: the draft card and the closed reply field wear one fill, the lifted glass
    s.expect("list-row-shows-draft-mark", list_draft, True)
    # The card's head at the smallest window (lead, main92 picture thread-saved-draft 960x600: "You  [  Draft", the address cut to one
    # glyph): the address is drawn whole, cut to a readable width, or left out whole. Read from the `shot` frame of its text.
    def head(name):
        """The address's frame; on a missing frame, what tells why (main95: none at 960 nor 1280): the frame of the head's You beside it
        (drawn or not) and the address the card is given (rows mac-main.thread.draft `address`, null when the card has none)."""
        fr = s.ctl.send("shot", name, DRAFT_ADDRESS, DRAFT_YOU)["frames"]
        a = fr.get(DRAFT_ADDRESS)
        why = None if a is not None else "no frame for the address; the head's You: %s; the card's address: %r" % (
            "framed %.0f pt wide" % fr[DRAFT_YOU]["w"] if fr.get(DRAFT_YOU) else "no frame", (draft_rows() or [{}])[0].get("address"))
        return a, why
    s.raw("resize", "960", "600")
    s.expect("draft-card-address-whole-cut-or-hidden-at-960", lambda: (lambda a, why: why or address_fit(a))(*head("par-qr-draft-head-960")), "fits")
    s.raw("resize", "1280", "820")
    s.expect("draft-card-address-shown-at-1280", lambda: (lambda a, why: why or (a["w"] >= ADDRESS_MIN_PT or "address %.1f pt wide" % a["w"]))(*head("par-qr-draft-head-1280")), True)
    drive_foot(j, hide_draft=lambda: s.click(DRAFT + ".0.delete"), show_draft=lambda: s.click(DELETED + ".undo"))   # A147: the message card, the draft card and the reply field share their edges
    s.click(DRAFT + ".0")
    s.expect("draft-click-opens-in-field", lambda: field(s, "text"), "Draft words.\nSecond line.")
    ed = {}
    try:
        ed["id"], ed["undo"] = draft_rows()[0]["id"] if draft_rows() else None, s.ctl.core("undo_entry/count")
    except Exception:                                                # noqa: BLE001  dry run, or the card hidden while open
        ed.setdefault("id", None)
        ed.setdefault("undo", None)
    s.type(" Edited.")
    s.key("Escape")
    s.expect("reopened-draft-updates-in-place", lambda: (draft_rows()[0]["lines"].endswith("Edited."), s.ctl.core("undo_entry/count") == ed["undo"]), (True, True))
    s.click(DRAFT + ".0.delete")
    s.expect("delete-removes-draft-card", lambda: len(draft_rows()), 0)
    s.expect("delete-offers-undo", lambda: s.ctl.rows(DELETED)[0]["undo_cap"], "Z")
    s.expect("list-mark-goes-with-draft", list_draft, False)
    s.click(DELETED + ".undo")
    s.expect("undo-brings-draft-back", lambda: len(draft_rows()), 1)
    s.key("\u2318Backspace")
    s.expect("backspace-key-deletes-draft", lambda: len(draft_rows()), 0)
    s.click(DELETED + ".undo")
    # Round 20 (owner 2026-11-13 07:26, "impossible to delete"; the picture: a draft card under an OPEN reply). Lead, 13 Nov: R now CONTINUES the
    # saved draft (its words in the field, its card not drawn), so no card sits under an open reply any more; the way to delete from there is the
    # key the strip shows, Discard (a REAL Shift Cmd comma): the reply closes, the draft is gone (no card, no Draft mark on the list row, one draft
    # fewer in the engine), and the thread offers its Undo.
    dl = {}
    s.key("R")                                                       # the reply opens on the saved draft's words
    seen_count = lambda k: dl.__setitem__(k, draft_count(s))
    try:
        seen_count("d0")
    except Exception:                                                # noqa: BLE001  dry run: the case is unjudged
        dl["d0"] = None
    s.raw("keyevent", "\u21e7\u2318,")

    def gone(key):
        got = (len(draft_rows()), list_draft(), draft_count(s) - dl[key] if dl.get(key) is not None else "no count before")
        return got if got == (0, False, -1) else {"read (draft cards, list mark, drafts more)": got}
    s.expect("discard-key-in-an-open-reply-removes-the-draft", lambda: gone("d0"), (0, False, -1))
    s.expect("discard-key-closes-the-reply", lambda: len(s.ctl.rows(FIELD)), 0)
    s.click(DELETED + ".undo")
    try:
        seen_count("d1")
    except Exception:                                                # noqa: BLE001
        dl["d1"] = None
    s.raw("keyevent", "cmd+Delete")                                  # a REAL ⌘⌫ (an NSEvent through the shell's monitor), not the socket's key
    s.expect("backspace-key-event-removes-the-draft", lambda: gone("d1"), (0, False, -1))
    s.click(DELETED + ".undo")
    # Round 20 (owner 07:29: an empty "You · Draft" card): a reply opened and left with no words leaves nothing: no card, no draft in the engine.
    try:
        dl["d2"] = draft_count(s)
    except Exception:                                                # noqa: BLE001
        dl["d2"] = None
    s.key("R")
    s.key("Escape")
    s.expect("empty-reply-leaves-no-draft", lambda: (len(draft_rows()), draft_count(s) - dl["d2"] if dl.get("d2") is not None else "no count before"), (1, 0))
    st = {}
    try:
        st["n0"] = len(s.ctl.rows(OUT))
    except Exception:                                                # noqa: BLE001  dry run
        st["n0"] = 0
    s.click(DRAFT + ".0.send")
    s.expect("draft-send-makes-outgoing-card", lambda: (len(s.ctl.rows(OUT)) - st["n0"], s.ctl.rows(OUT)[-1]["text"].startswith("Draft words.")), (1, True))
    s.expect("sent-draft-leaves-drafts", lambda: len(draft_rows()), 0)
    s.click(UNDO)
    s.key("Escape")
    s.key("\u2303Enter")
    s.expect("ctrl-enter-key-sends-draft", lambda: len(draft_rows()), 0)
    # A145: the draft card shows ⌘↵ (draft-card-keys-shown), so ⌘↵ with the card shown and no field open sends it too
    s.click(UNDO)                                                    # the ⌃Enter send comes back to the field
    s.key("Escape")                                                  # its words become the draft card again
    s.expect("draft-card-is-back-before-cmd-enter", lambda: len(draft_rows()), 1)
    # Owner, 13 Nov, picture 206 ("when you have a draft you shouldn't have a draft + writing an email no ?"): a conversation holding TWO saved drafts, the
    # state of a mailbox the builds before 13 Nov left (a new draft at every R). One card while the reply is closed (the newest draft), NO card while the
    # reply box is open on that conversation (it holds the newest draft's words), and ⌘⌫ removes the card's draft.
    try:
        s.raw("fixture", "second-draft", "A newer saved draft.")
    except Exception:                                                # noqa: BLE001  an older build: the cases below say so
        pass
    s.expect("one-card-for-a-conversation-with-two-drafts", lambda: [r["lines"] for r in draft_rows()], ["A newer saved draft."])
    s.raw("keyevent", "R")
    s.expect("no-draft-card-beside-an-open-reply", lambda: (len(draft_rows()), len(s.ctl.rows(FIELD)), field(s, "text")), (0, 1, "A newer saved draft."))
    s.key("Escape")
    s.key("\u2318Backspace")                                         # the newer draft goes; the conversation's other draft is the card again
    s.expect("cmd-backspace-removes-the-shown-draft-only", lambda: len(draft_rows()), 1)
    s.key("\u2318Enter")
    s.expect("cmd-enter-key-sends-draft", lambda: len(draft_rows()), 0)
    # The owner's eye, 19:58 shots: a growing sending card was drawn over the draft card. A sending card and the draft
    # card at once, their frames measured right after the send and twice more: no two cards may intersect.
    s.click(UNDO)                                                    # the ⌃Enter send comes back to the field (it was counting: Esc and R wait on a window)
    s.key("Escape")                                                  # its words become the draft card
    s.key("R")
    s.type("A send above the draft, long enough to wrap onto a second line in the thread column at 1280 points.")
    s.key("\u2318Enter")                                              # the sending card grows above it (round 20: plain Enter no longer sends)

    def gaps_drawn_clear():
        """No two stacked cards overlap, judged on the frames the shot reply reads in the SAME main-thread pass as
        its pixels (CONTROL-SOCKET deltas 45-46). A pixel test cannot judge here: under the hidden rig's stand-in the
        lift's fill equals the pane in both themes (main43 shots), so only the shadow tells a card, and a card's own
        shadow falls into the gap by design."""
        ids = [r["ax"] for r in s.ctl.rows(OUT)] + [r["ax"] for r in draft_rows()]
        if len(ids) < 2:
            return "fewer than two cards: %d" % len(ids)
        frames = s.ctl.send("shot", "par-qr-gap", *ids)["frames"]
        missing = [i for i in ids if frames.get(i) is None]
        if missing:
            return "no frame in the shot reply for %r" % missing
        fs = sorted((dict(frames[i], ax=i) for i in ids), key=lambda f: f["y"])
        for a, b in zip(fs, fs[1:]):
            if b["y"] < a["y"] + a["h"] - 0.5:
                return "%s overlaps %s by %.1f pt" % (b["ax"], a["ax"], a["y"] + a["h"] - b["y"])
        return "clear"
    for n in range(3):
        s.expect("cards-never-overlap-%d" % n, gaps_drawn_clear, "clear")
    s.click(UNDO)
    drive_lift(j)


def perf_rows(lines):
    """The whole JSON rows of the perf file and the count of lines that are not one. The app appends each row with a seek and a write
    (an older build: seek then write) so two processes on one file could tear a line: main76 read `Expecting value: line 1 column 1` here and the
    whole run stopped. The app now writes one whole line in one O_APPEND write (SendWavePerfLog.swift:27-31); a torn line in a file is an older build's."""
    rows, torn = [], 0
    for l in lines:
        if not l.strip():
            continue
        try:
            r = json.loads(l)
        except ValueError:
            torn += 1
            continue
        if isinstance(r, dict):
            rows.append(r)
        else:
            torn += 1
    return rows, torn


def landing_of_run(lines, since, where):
    """The last landing row written by THIS run. A torn line is skipped; when no whole row of this run is left and a line was torn, the run may
    have lost its own row to the tear, so the case is NOT JUDGED (Unreadable) with the count, never red and never a pass."""
    rows, torn = perf_rows(lines)
    mine = [r for r in rows if r.get("kind") == "landing" and str(r.get("at", "")) >= since]
    if mine:
        return mine[-1]
    last = next((l for l in reversed(lines) if l.strip()), "")
    lastrow = rows[-1] if rows else {}
    # FIRST what decides it (main92 and main93 printed the file name first and the line was cut): this run's rows, each kind/state/reason/
    # frames/windows/snapshot source, so a landing that never started is told from one that was interrupted, and why
    this_run = ["%s/%s(%s) frames=%s windows=%s snap=%s" % (r.get("kind"), r.get("state"), r.get("reason") or "-", r.get("frames"), r.get("windows"), r.get("snapshot_source"))
                for r in rows if str(r.get("at", "")) >= since]
    why = ("this run's rows %s; file %s (%d line(s), %d whole row(s), %d landing row(s), none at or after %s); last line %r; last whole row at=%r kind=%r state=%r"
           % (this_run, where, len(lines), len(rows), sum(1 for r in rows if r.get("kind") == "landing"), since, last[:200], lastrow.get("at"), lastrow.get("kind"), lastrow.get("state")))
    if torn:
        raise Unreadable("no whole landing row of this run: %d line(s) of the file are torn (not JSON), so the row may be one of them; %s" % (torn, why))
    raise KeyError("no landing row of this run (refused: a row counts only when kind is landing and at >= the run's start): %s" % why)


def perf_log_path(env=None):
    """The perf file of the TEST COPY, never the owner's. A rig copy (launched with WHOOSH_SOCKET_DIR=$D) writes `$D/perf/sendwave.jsonl`
    (ModelsFolder.whooshRoot: the run folder on a rig copy, CoreHost.swift CorePlace: dataDir = $D/core; SendWavePerfLog.swift:11-14); a test that
    gives the app WHOOSH_SENDWAVE_PERF gets that file. The owner's `~/Library/Application Support/Whoosh/perf/sendwave.jsonl` belongs to the owner's
    installed copy and is read by perf/sendwave-report.py only: the check never reads it. None when neither is set (the case is then not judged)."""
    env = os.environ if env is None else env
    if env.get("WHOOSH_SENDWAVE_PERF"):
        return env["WHOOSH_SENDWAVE_PERF"]
    if env.get("WHOOSH_SOCKET_DIR"):
        return os.path.join(env["WHOOSH_SOCKET_DIR"], "perf", "sendwave.jsonl")
    return None


HEADER = "mac-main.quickreply.header"


def thread_of(message_id):
    """The thread a list row's message belongs to, by the test engine's rule (FixtureCore.threadIdOf): the fixture's `thread` field, else
    the open thread's id for the fixture's first message, else the message's own id. The open-by-thread doors are per THREAD
    (QuickReplyFeature: one per `rows.threadId`); main95 clicked `open.msg-01.stay`, which does not exist: msg-01 is thread thr-01."""
    msgs = FX.get("messages", [])
    m = next((x for x in msgs if x.get("id") == message_id), None)
    if m is None:
        return message_id
    if m.get("thread"):
        return m["thread"]
    if msgs and msgs[0].get("id") == message_id and FX.get("open_thread"):
        return FX["open_thread"]["id"]
    return message_id
HEADER_TOGGLE = "mac-main.quickreply.header.toggle"
HEADER_TO = "mac-main.quickreply.header.to"
FOOT_STATE = "mac-main.reader.state"               # one row; message_count (ReaderProbe.swift "mac-main.reader.state")


def foot_message(s):
    """The open message's card: `mac-main.reader.message.<earlierCount>` (ReaderBlocks.swift:97), earlierCount = messageCount - 1
    (ReaderThread.swift:36). The fixture thread holds 3 messages (msg-01 thread_count 3), so the open card is message.2, not message.0
    (main88: message.0 is the first earlier message, folded, with no frame)."""
    return "mac-main.reader.message.%d" % (int(s.ctl.rows(FOOT_STATE)[0]["message_count"]) - 1)
FOOT_DRAFT = DRAFT + ".0"
FOOT_REPLY = "mac-main.reader.reply"                # the closed reply field (ReaderReply.swift)
FOOT_TOL = 0.5                                      # points
FOOT_DOTS = "mac-main.reader.body.dots"             # the quoted-text pill under the written text (MessageBodyView.swift DotsPill, `.whooshAx`)
FOOT_BODY = "mac-main.reader.body"                  # the body probe row: `has_quote` says whether a pill is drawn at all


def foot_frames(s, ids):
    """Left and right edge (x, x + w) of each id, from one `shot` reply's frames (read in the same main-thread pass as the pixels).
    A missing frame is named, never read as aligned."""
    frames = s.ctl.send("shot", "par-qr-foot", *ids)["frames"]
    missing = [i for i in ids if frames.get(i) is None]
    if missing:
        raise KeyError("no frame in the shot reply for %r" % missing)
    return {i: (frames[i]["x"], frames[i]["x"] + frames[i]["w"]) for i in ids}


def foot_edges(s, st, hide_draft=None, show_draft=None):
    """The edges of the open message's card, the draft card and the closed reply field, read once and kept in `st`.
    Owner, 13 Nov, picture 206: a saved draft and the closed reply field never stand together, so on the app the draft card's edges are read
    first, `hide_draft` then removes the draft (the caller's own step), the reply field's edges and material are read, and `show_draft`
    puts the draft back. The selftest's model serves the three frames at once and passes no callback."""
    if st.get("e") is None:
        import time
        msg = foot_message(s)
        if hide_draft is None:
            e = foot_frames(s, [msg, FOOT_DRAFT, FOOT_REPLY])
            material = s.ctl.rows(FOOT_REPLY)[0]["material"]
        else:
            e = foot_frames(s, [msg, FOOT_DRAFT])
            beside = len(s.ctl.rows(FOOT_REPLY))
            if beside:
                raise KeyError("the closed reply field is drawn beside a draft card (%d row)" % beside)
            hide_draft()
            try:
                time.sleep(0.25)                                     # the thread lays out again without the card
                e.update(foot_frames(s, [FOOT_REPLY]))
                material = s.ctl.rows(FOOT_REPLY)[0]["material"]
            finally:
                show_draft()
        e["message"] = e[msg]
        st["e"], st["material"] = e, material
    return st["e"]


FOOT_OFFSET = 32.0                                  # ThreadFootLayout.authorOffset (the reader's "aligned by author" s32)


def drive_foot(j, hide_draft=None, show_draft=None):
    """SPEC A147 (owner 2026-11-11, "weird visual bug"): the draft card and the reply field (both the owner's own) start at one x and end at
    one x; the received message keeps the author offset: it starts FOOT_OFFSET left of them and ends FOOT_OFFSET short of their right edge.
    Called from drive_drafts, where the draft card stands under the open message with the slab closed. Since 13 Nov the closed reply field
    is not drawn beside a draft card: its edges are read with the draft removed for a moment (`foot_edges`); a model of the three frames
    serves the selftest."""
    s = j.step("par-quickreply-foot", "The thread's blocks with a draft share their edges")
    st = {}
    edges = lambda: foot_edges(s, st, hide_draft, show_draft)
    s.expect("foot-own-blocks-share-left-edge", lambda: abs(edges()[FOOT_DRAFT][0] - edges()[FOOT_REPLY][0]) <= FOOT_TOL, True)
    s.expect("foot-own-blocks-share-right-edge", lambda: abs(edges()[FOOT_DRAFT][1] - edges()[FOOT_REPLY][1]) <= FOOT_TOL, True)
    def offset_kept():
        e = edges()
        got = (round(e[FOOT_DRAFT][0] - e["message"][0] - FOOT_OFFSET) == 0, round(e[FOOT_DRAFT][1] - e["message"][1] - FOOT_OFFSET) == 0)
        # a miss names the three left and right edges read (main92 printed (False, False) alone)
        return got if got == (True, True) else {"read": got, "edges (left, right)": {"draft": e[FOOT_DRAFT], "message": e["message"], "reply": e[FOOT_REPLY]}}
    s.expect("foot-message-keeps-the-author-offset", offset_kept, (True, True))
    # docs/lanes/fix-mailviews.md, the two cases it left to this file (item 44036170): the closed reply field's material, read from its own row
    # (`rows mac-main.reader.reply` serves one row {ax, material} while the quick reply is NOT open on the thread: the slab is closed here), and the
    # quoted-text pill's frame inside its card.
    s.expect("reply-field-material-is-lifted", lambda: (edges(), st["material"])[1], "lifted")
    s.expect("dots-pill-lies-inside-its-card", lambda: dots_inside_its_card(s), True)


def dots_inside_its_card(s):
    """True when the quoted-text pill's frame lies inside the frame of the message card that holds it (both read from ONE `shot` reply, in the
    same main-thread pass: CONTROL-SOCKET delta 46, a separate `measure` of each can belong to another layout). A message with no quote draws no
    pill: NOT JUDGED, never a pass. A quote with no frame for the pill (the `.whooshAx` lost) is the string 'no frame', a failure."""
    from lib.ctl import Unreadable as _U
    if not s.ctl.rows(FOOT_BODY)[0]["has_quote"]:
        raise _U("the open message has no quoted text: the pill is not drawn, nothing to measure")
    msg = foot_message(s)
    frames = s.ctl.send("shot", "par-qr-dots", msg, FOOT_DOTS)["frames"]
    if frames.get(msg) is None:
        raise KeyError("no frame in the shot reply for %r" % msg)
    if frames.get(FOOT_DOTS) is None:
        return "no frame"
    c, p = frames[msg], frames[FOOT_DOTS]
    return (p["x"] >= c["x"] - FOOT_TOL and p["x"] + p["w"] <= c["x"] + c["w"] + FOOT_TOL
            and p["y"] >= c["y"] - FOOT_TOL and p["y"] + p["h"] <= c["y"] + c["h"] + FOOT_TOL)


HEADER_CC = "mac-main.quickreply.header.cc"
POPOUT = "mac-main.quickreply.popout"


def shot_frames(s, name, ids):
    """One `shot` reply: the frames of `ids` read in the same main-thread pass as the pixels (CONTROL-SOCKET deltas
    45-48), each moved into PNG points by `mapping`; plus the pixel reader and the scale."""
    reply = s.ctl.send("shot", name, *ids)
    frames, mp = reply["frames"], reply["mapping"]
    missing = [i for i in ids if frames.get(i) is None]
    if missing:
        raise KeyError("no frame in the shot reply for %r" % missing)
    fs = {i: dict(f, x=f["x"] + mp["x"], y=f["y"] + mp["y"]) for i, f in frames.items() if f}
    w, h, px = png_rgba(reply["path"])
    return fs, px, float(mp["scale"]), reply["path"]


ROOM_WINDOW_H = 600                                 # drive_lift resizes the window to 960 x 600 before its sends


def drive_lift(j):
    """Round 5 (owner pick 00:35 "Lifted until it lands", board cards-s3): the slab's header, the From picker's key, the
    lifted and landed materials, the room under the last card, the failed card, the landing's cost, and the pop-out."""
    import json
    import time
    s = j.step("par-quickreply", "Lifted until it lands")
    from datetime import datetime, timezone
    st = {"t_start": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")}   # the perf log's `at` is UTC ISO 8601

    def outs():
        return s.ctl.rows(OUT)

    def header():
        return s.ctl.rows(HEADER)[0]
    s.key("Escape")                                                  # the field closes; its words are the thread's draft
    s.click(DRAFT + ".0.delete")                                     # start the round with no draft card
    s.raw("resize", "960", "600")                                    # the thread is longer than the pane: the slab floats over it
    s.key("R")
    # The header line: "Répondre à <x>" while the field is empty, "Brouillon à <x>" once there are words (owner 23:04).
    try:
        st["t0"] = header()["title"]
    except Exception:                                                # noqa: BLE001  dry run
        st["t0"] = ""
    s.type("Header words.")
    s.expect("header-title-changes-with-words", lambda: (header()["title"] != st["t0"], bool(st["t0"]), bool(header()["title"]), header()["open"]), (True, True, True, "0"))
    s.expect("header-shows-keys", lambda: (header()["cap"], header()["popout_cap"]), ("\u21e7H", "\u21e7\u2318P"))
    s.click(HEADER_TOGGLE)
    s.expect("header-toggle-opens-rows", lambda: (header()["open"], len(s.ctl.rows(HEADER_TO)), "@" in s.ctl.rows(HEADER_TO)[0]["address"]), ("1", 1, True))
    s.expect("header-rows-carry-subject", lambda: bool(header()["subject"]), True)
    s.key("\u21e7H")
    s.expect("shift-h-collapses-header", lambda: header()["open"], "0")
    s.key("\u21e7H")
    s.click(HEADER_CC)
    s.type("cc@example.com")
    s.expect("typed-cc-lands-in-row", lambda: (header()["cc"], field(s, "text")), ("cc@example.com", "Header words."))
    try:
        st["n0"] = len(outs())
    except Exception:                                                # noqa: BLE001
        st["n0"] = 0
    s.key("\u2303Enter")
    # An edited header sends through a saved draft and Send (contract delta 12): the card carries the Cc.
    s.expect("expanded-reply-sends-via-draft", lambda: (len(outs()) - st["n0"], outs()[-1]["state"], outs()[-1].get("cc")), (1, "waiting", "cc@example.com"))
    s.expect("header-folds-while-counting", lambda: (header()["counting"], len(s.ctl.rows(UNDO_BAR))), ("1", 1))
    # The materials: a waiting card floats (its shadow falls on the room under it); the landed card lies flat (`mine`).

    def last_card_fill():
        """(interior pixel, pixel just under the card, the pane's background beside it) of the newest card."""
        ax = outs()[-1]["ax"]
        fs, px, sc, _ = shot_frames(s, "par-qr-material", [ax])
        f = fs[ax]
        settled = const("LAND_SETTLE_PT") if outs()[-1].get("material") == "flat" else 0   # a landed card is drawn this far lower
        inside = px(int((f["x"] + f["w"] - 10) * sc), int((f["y"] + settled + f["h"] / 2) * sc))
        under = px(int((f["x"] + f["w"] / 2) * sc), int((f["y"] + settled + f["h"] + 3) * sc))
        beside = px(int((f["x"] - 4) * sc), int((f["y"] + f["h"] / 2) * sc))
        return inside, under, beside

    def differs(a, b, by):
        return max(abs(a[i] - b[i]) for i in range(3)) > by
    try:
        at_end(s)                                                    # the reveal has settled: the card is not under the slab
        st["waiting"] = last_card_fill()
    except Exception:                                                # noqa: BLE001  dry run or no frame
        st["waiting"] = None
    def lifted_read():
        got = (outs()[-1]["material"], differs(st["waiting"][1], st["waiting"][2], 3))
        # a miss names the three pixels it compared (inside the card, under it, beside it) and the card's row (main92/93: (lifted, False) alone)
        return got if got == ("lifted", True) else {"read": got, "pixels (inside, under, beside)": st["waiting"], "card": outs()[-1]}
    s.expect("sending-card-is-lifted", lifted_read, ("lifted", True))
    # The room: the newest card sits wholly in the window, three sends running. Lead, 13 Nov (owner's picture 206): the reply box folds at
    # the send and the sending card carries the countdown, so no slab floats over the thread; the card must still be scrolled into view.

    def visible_above_slab():
        at_end(s)
        ax = outs()[-1]["ax"]
        fs, _, _, _ = shot_frames(s, "par-qr-room", [ax])
        card = fs[ax]
        if card["y"] < 0:
            return "card top above the window: %r" % card
        if card["y"] + card["h"] > ROOM_WINDOW_H:
            return "card below the window: card bottom %.0f, window %d" % (card["y"] + card["h"], ROOM_WINDOW_H)
        if s.ctl.rows(UNDO_BAR) and field(s, "phase") == "editing":
            return "the reply box is open over a sending card"
        return "visible"
    s.expect("visible-after-send-0", visible_above_slab, "visible")
    s.raw("clock", "advance", const("UNDO_SEND_DEFAULT_S") + 1)     # the landing frame
    t0 = time.perf_counter()
    s.key("R")                                                       # a key on the landing frame: it must not wait for the light. The box folded at the send (13 Nov): R opens the next reply
    st["key_ms"] = (time.perf_counter() - t0) * 1000
    s.expect("landing-never-holds-a-key", lambda: (len(s.ctl.rows(FIELD)), field(s, "phase"), st["key_ms"] < 300), (1, "editing", True))
    s.expect("header-returns-after-landing", lambda: (header()["counting"], header()["open"]), ("0", "0"))
    s.key("Escape")                                                  # the empty reply closes and keeps nothing
    time.sleep(0.8)                                                  # the light's 480 ms and the row's collapse; no shot inside them (the shot is main-thread work)
    def landed():
        """flat material; the interior is the mine tint now, not the glass; the shadow under the card is gone (the pixel
        under it is lighter than while the card floated; a lifted draft card below may still cast its own)."""
        now = last_card_fill()
        return (outs()[-1]["material"], differs(now[0], st["waiting"][0], 6), sum(now[1][:3]) > sum(st["waiting"][1][:3]) + 3)
    s.expect("sent-card-lands-flat", landed, ("flat", True, True))

    def landing_row():
        """The landing row THIS run's landing wrote: rows older than this drive's start are other runs' (a light that
        never played must not pass on a stale row; the landing-light-ran mutant found it)."""
        _ = s.ctl                                                    # no socket (dry run): unjudged, not red
        path = perf_log_path()
        if path is None:
            raise Unreadable("no test copy folder known (neither WHOOSH_SOCKET_DIR nor WHOOSH_SENDWAVE_PERF is set): the owner's perf file is never read")
        lines = open(path, encoding="utf-8", errors="replace").read().splitlines() if os.path.exists(path) else []
        return landing_of_run(lines, st["t_start"], path)
    def landing_state():
        # The miss is a value, not a crash of the read (main95: "reply shape: KeyError"): the state word, or why there is no row
        # (this run's send-wave rows lead it). A landing is logged only when it was measured in a window (AuroraLanding.swift
        # `guard let w = window ... unlogged`), so send rows reading windows=0 say the wave found no window to draw in.
        try:
            return landing_row()["state"]
        except KeyError as e:
            return str(e.args[0] if e.args else e)
    s.expect("landing-light-ran", landing_state, "done")
    try:
        lr = landing_row()
        print("LANDING ROW frames=%s dropped=%s max_frame_ms=%s max_busy_ms=%s over_busy=%s hidden=%s (the frame verdict is the owner's copy's: perf/sendwave-report.py)"
              % (lr.get("frames"), lr.get("dropped"), lr.get("max_frame_ms"), lr.get("max_busy_ms"), lr.get("over_busy"), lr.get("hidden")))
    except Exception as e:                                           # noqa: BLE001
        print("LANDING ROW none: %s" % e)
    for n in (1, 2):
        s.key("R") if n == 1 else None                               # closed since the landing; after the Cancel below the words are back in the open box
        s.type("Send %d." % n)
        s.key("\u2318Enter")                                          # round 20: plain Enter no longer sends
        s.expect("visible-after-send-%d" % n, visible_above_slab, "visible")
        s.click(UNDO)
    # The failed card: the server refuses the send with a code; the thread shows the card, Réessayer queues it again.
    s.raw("fixture", "fail-next", "quick-send", "the server refused the message 451 4.7.1")
    s.key("\u2318Enter")
    s.expect("failed-card-shows-code", lambda: (outs()[-1]["state"], outs()[-1]["code"], outs()[-1]["material"], outs()[-1]["retry_cap"], outs()[-1]["discard_cap"]), ("failed", "451 4.7.1", "lifted", "\u2318\u21b5", "\u2318\u232b"))   # owner 13 Nov: "cmd+del not just del it would clash"
    try:
        st["nf"], st["retry"] = len(outs()), outs()[-1]["retry"]
    except Exception:                                                # noqa: BLE001
        st["nf"], st["retry"] = 0, OUT + ".0.retry"
    s.click(st["retry"])
    s.expect("failed-card-retries", lambda: (len(outs()) - st["nf"], outs()[-1]["state"]), (0, "waiting"))   # the card carries its own countdown; the reply box is not counting
    s.key("Z")                                                       # Cancel is the card's key
    s.key("R")                                                       # the test engine gives no draft back on a cancel, the real one does: either way the box opens
    s.type("Fail again.")
    s.raw("fixture", "fail-next", "quick-send", "the server refused the message 451 4.7.1")
    s.key("\u2318Enter")
    try:
        st["discard"] = outs()[-1]["discard"]
    except Exception:                                                # noqa: BLE001
        st["discard"] = OUT + ".0.discard"
    s.click(st["discard"])
    s.expect("failed-card-discard-removes-it", lambda: len(outs()) - st["nf"], -1)
    # Round 20 (owner 07:28: "after send, I still see cancel with z"; the entry was failed). A send the engine refused says so: "Not sent" with
    # the engine's words, Retry ⌘↵ and Edit ⌘E, no Cancel; a REAL ⌘E puts its words back in the reply's text and the card goes.
    s.raw("fixture", "fail-next", "quick-send", "the server refused the message 451 4.7.1")
    s.key("R")                                                       # the box folded when the send before it failed
    s.type("Edit me.")
    s.key("\u2318Enter")

    def failed_offers():
        r = outs()[-1]
        got = (r["state"], bool(r.get("words")), r.get("retry_cap"), r.get("edit_cap"), "undo" in r)
        return got if got == ("failed", True, "\u2318\u21b5", "\u2318E", False) else {"read": got, "card": r}
    s.expect("failed-card-offers-retry-and-edit-with-keys", failed_offers, ("failed", True, "\u2318\u21b5", "\u2318E", False))
    r_then_keys(s, "r-puts-the-cursor-in-the-reply-with-a-failed-send-shown", "789")   # the same real R beside a "Not sent" card; the helper empties and closes the reply after
    try:
        st["ne"] = len(outs())
    except Exception:                                                # noqa: BLE001
        st["ne"] = 0
    s.raw("keyevent", "\u2318E")
    s.expect("cmd-e-puts-the-failed-words-back-in-the-reply", lambda: (field(s, "text"), len(outs()) - st["ne"]), ("Edit me.", -1))
    s.raw("keyevent", "\u2318A")
    s.raw("keyevent", "Delete")
    # The undo window is the wall clock's: past it, the card's Cancel is gone even while the engine still reads the entry undoable (the test
    # engine's clock is not moved here, as the owner's engine had not moved on). Read from the frames the shot draws.
    s.type("Window words.")
    s.key("\u2318Enter")
    time.sleep(const("UNDO_SEND_DEFAULT_S") + 1)

    def cancel_drawn():
        ax = outs()[-1].get("undo") or OUT + ".0.undo"
        fs = s.ctl.send("shot", "par-qr-window-over", ax)["frames"]
        return "drawn at %r" % fs[ax] if fs.get(ax) else "gone"
    s.expect("cancel-leaves-when-the-window-ends", cancel_drawn, "gone")
    s.raw("clock", "advance", const("UNDO_SEND_DEFAULT_S") + 1)    # the engine's own window ends too: the send goes, the field is free
    # The From picker: ⇧⌘F opens it with the sender checked, its key line and filter; Esc closes it.
    s.key("R")                                                       # the reply closed when its send landed
    s.key("\u21e7\u2318F")
    s.expect("from-key-opens-picker", lambda: (s.ctl.rows(FROM)[0]["open"], [r["checked"] for r in s.ctl.rows(FROM_MENU)].count("1")), ("1", 1))
    def highlighted():
        """The highlighted row's index, -1 for none (a ValueError inside a check crashed a mutant run, hiding its red)."""
        rows = s.ctl.rows(FROM_MENU)
        return next((i for i, r in enumerate(rows) if r.get("highlighted") == "1"), -1)
    try:
        st["hl"], st["n"] = highlighted(), max(1, len(s.ctl.rows(FROM_MENU)))   # opens on the sender's row
    except Exception:                                                # noqa: BLE001
        st["hl"], st["n"] = 0, 1
    s.key("Down")
    s.expect("picker-arrow-moves-highlight", highlighted, (st["hl"] + 1) % st["n"])
    s.key("Escape")
    s.expect("picker-esc-closes-it", lambda: (s.ctl.rows(FROM)[0]["open"], field(s, "phase")), ("0", "editing"))
    # Round 6 (the notch replied to the wrong person): open the reply BY THREAD while another row has the focus.
    ob = {}
    try:
        rows = s.ctl.rows("mac-main.list")
        focused = next(r for r in rows if r.get("focused"))
        other = next(r for r in rows if r["sender"] != focused["sender"] and r["id"] != focused["id"] and r.get("account_address"))
        ob.update(thread=other["id"], sender=other["sender"], address=other["account_address"])   # the fixture keys a thread by its row's message id
    except Exception:                                                # noqa: BLE001  dry run
        ob.update(thread="msg-04", sender="?", address="?")
    s.key("Escape")                                                  # the field closes
    s.key("Escape")                                                  # the main window back on mac-main
    s.click("mac-main.quickreply.open.%s.stay" % ob["thread"])      # moveSurface: false (the notch's call)
    s.expect("open-by-thread-stay-keeps-surface", lambda: (field(s, "thread"), s.ctl.uistate().get("surface")), (ob["thread"], "mac-main"))
    s.click("mac-main.quickreply.open.%s" % ob["thread"])
    s.expect("open-by-thread-answers-that-thread", lambda: (field(s, "thread"), ob["sender"] in s.ctl.rows(HEADER)[0]["title"], field(s, "from"), s.ctl.uistate().get("surface")),
             (ob["thread"], True, ob["address"], "mac-reader"))
    # Round 7 (verdict sweep on main41: From read Nordlicht on 5 of 5 threads): From is the account that RECEIVED the
    # thread, on three threads of three different accounts.
    trio = []
    try:
        if s.j.ctl is None:                                          # a dry run hands the body stand-ins (journeys/lib/harness.py Dry): take the dry trio
            raise Unreadable("dry run")
        for r in s.ctl.rows("mac-main.list"):
            if r.get("account_address") and not r.get("account_attention") and r["account"] not in [t["account"] for t in trio]:
                trio.append({"id": r["id"], "account": r["account"], "address": r["account_address"]})
            if len(trio) == 3:
                break
    except Exception:                                                # noqa: BLE001  dry run
        trio = [{"id": "msg-0%d" % i, "account": "?", "address": "?"} for i in (1, 2, 3)]
    for n, t in enumerate(trio[:3]):
        t["thread"] = thread_of(t["id"])
        clicked = {}
        try:
            clicked["reply"] = s.ctl.send("click", "mac-main.quickreply.open.%s.stay" % t["thread"]).get("ok")
        except Unreadable as e:                                      # a refused click is evidence for the case below, not a crash
            clicked["reply"] = "refused: %s" % e
        except Exception as e:                                       # noqa: BLE001  dry run
            clicked["reply"] = "unread (%s)" % type(e).__name__

        def from_read(t=t, clicked=clicked):
            got = (field(s, "thread"), field(s, "from"))
            # a miss names the click's answer and the list row of that id (main93: the field stayed on the thread opened before)
            return got if got == (t["thread"], t["address"]) else {"read": got, "click": clicked.get("reply"),
                                                               "list row": [{k: r.get(k) for k in ("id", "account", "account_address")} for r in s.ctl.rows("mac-main.list") if r.get("id") == t["id"]]}
        s.expect("from-is-receiving-account-%d" % n, from_read, (t.get("thread", t["id"]), t["address"]))
    s.expect("from-check-spans-three-accounts", lambda: (s.ctl, len({t["account"] for t in trio}))[1], 3)   # s.ctl: unjudged in a dry run
    # Round 8: the signature the core serves for the From address (signature(address:)) ends the sent reply,
    # after one blank line; with none served, the reply is the typed words alone.
    sig = {}
    s.type("Signature test.")
    try:
        sig["s"] = field(s, "signature")                             # read while the box is open: it folds at the send
    except Exception:                                                # noqa: BLE001  dry run
        sig["s"] = ""
    s.key("\u2318Enter")                                              # round 20: plain Enter no longer sends

    def sent_text():
        return s.ctl.rows(OUT)[-1]["text"]
    s.expect("reply-ends-with-served-signature",
             lambda: (lambda t: t == "Signature test." + ("\n\n" + sig["s"] if sig["s"] else ""))(sent_text()), True)
    s.click(UNDO)
    s.key("R")                                                       # back to the focused row's thread for the pop-out
    s.raw("resize", "1280", "820")
    drive_doors(j)                                                   # doors-compose: every control the rounds before did not click
    # LAST: the pop-out takes the words to the composer window, which then holds the keys.
    s.type("Pop these words out.")
    s.key("\u21e7\u2318P")
    s.expect("popout-key-opens-composer", lambda: ("Pop these words out." in s.ctl.rows("mac-composer.body")[0]["text"], bool(s.ctl.rows("mac-composer.header")[0]["to"]), len(s.ctl.rows(FIELD))), (True, True, 0))
    drive_doors_tail(j)
    drive_ai(j)                                                      # reply-thread: ⌘J, the mark, the change field
    drive_paste(j)                                                   # LAST of all: it opens the composer window, after which the hidden copy loses real plain keys meant for the main window (finding 52)


HEADER_BCC = "mac-main.quickreply.header.bcc"
HEADER_SUBJECT = "mac-main.quickreply.header.subject"
FILTER = "mac-main.quickreply.from.filter"
EMOJI = "mac-main.quickreply.emoji"


def head(s):
    return s.ctl.rows(HEADER)[0]


def drive_doors(j):
    """Round 9 (lane doors-compose, owner 2026-11-08: "you haven't properly tested every button/feature"): one click on every control of the quick
    reply the rounds before did not click: the seven reactions after the first, the From picker's filter and a row of the narrowed menu, the
    To chip's x, the Cci and Objet rows, a click on the field after a header row. docs/lanes/doors-compose.md holds the control table."""
    s = j.step("par-quickreply-doors", "Quick reply: every control clicked once, with its visible outcome")
    em, st = {}, {}
    for i in range(1, 8):
        s.click(EMOJI)
        try:
            em[i] = s.ctl.rows(PICKER)[i]["emoji"]
        except Exception:                                            # noqa: BLE001  dry run, or no picker
            em[i] = "?"
        s.click("mac-main.quickreply.emoji.%d" % i)
        s.expect("emoji-item-%d-lands-in-text" % i, lambda i=i: field(s, "text").endswith(em[i]), True)
    # the From picker's filter: the typing goes to the filter, the menu narrows to the account typed, the row picks it
    s.click(FROM)
    try:
        st["target"] = s.ctl.rows(FROM_MENU)[-1]["address"]
    except Exception:                                                # noqa: BLE001
        st["target"] = "?"
    s.type(st["target"])
    s.expect("from-filter-field-holds-the-typing", lambda: s.ctl.rows(FILTER)[0]["text"], st["target"])
    s.expect("from-filter-narrows-the-menu", lambda: [r["address"] for r in s.ctl.rows(FROM_MENU)], [st["target"]])
    s.click("mac-main.quickreply.from.0")
    s.expect("from-filtered-row-picks-its-account", lambda: (s.ctl.rows(FROM)[0]["address"], len(s.ctl.rows(FROM_MENU))), (st["target"], 0))
    # the header rows: the chip's x, a click on Cci and on Objet takes the typing, a click on the field takes it back
    try:
        opened = head(s)["open"] == "1"
    except Exception:                                                # noqa: BLE001  dry run
        opened = False
    if not opened:
        s.click(HEADER_TOGGLE)
    try:
        st["to0"] = len(s.ctl.rows(HEADER_TO))
        st["text0"] = field(s, "text")
    except Exception:                                                # noqa: BLE001
        st["to0"], st["text0"] = -1, "?"
    s.click(HEADER_TO + ".0.remove")
    s.expect("header-to-chip-x-takes-the-person-off", lambda: (st["to0"], len(s.ctl.rows(HEADER_TO))), (1, 0))
    s.click(HEADER_BCC)
    s.type("bcc@example.com")
    s.expect("bcc-row-click-takes-the-typing", lambda: (head(s)["bcc"], field(s, "text")), ("bcc@example.com", st["text0"]))
    s.click(HEADER_SUBJECT)
    s.type(" Re-edited")
    s.expect("subject-row-click-takes-the-typing", lambda: head(s)["subject"].endswith(" Re-edited"), True)
    s.click(FIELD)
    s.type("Back in the field.")
    s.expect("field-click-takes-the-typing-back", lambda: (field(s, "text").endswith("Back in the field."), head(s)["subject"].endswith("Back in the field.")), (True, False))
    s.key("Escape")                                                  # the field closes (its words become the draft card)
    s.key("R")                                                       # and opens again with the thread's own To, so the pop-out below reads a recipient


def drive_doors_tail(j):
    """After the pop-out KEY's case: the composer window is discarded, the reply reopens, and the pop-out BUTTON does the same."""
    s = j.step("par-quickreply-doors", "Quick reply: the pop-out button")
    s.key("⇧⌘,")
    s.key("Enter")
    s.key("R")
    s.type("Click pop words.")
    s.click(POPOUT)
    s.expect("popout-button-opens-composer", lambda: ("Click pop words." in s.ctl.rows("mac-composer.body")[0]["text"], bool(s.ctl.rows("mac-composer.header")[0]["to"]), len(s.ctl.rows(FIELD))), (True, True, 0))
    s.key("⇧⌘,")
    s.key("Enter")



# ---- reply-thread: the AI reply in the thread (contracts delta 20; docs/lanes/reply-thread.md holds the control table) ----
AI = "mac-main.quickreply.ai"
AI_INSTR = AI + ".instruction"
AI_INSTR_SUBMIT = AI_INSTR + ".submit"
AI_CHANGE = AI + ".change"
AI_CHANGE_SUBMIT = AI_CHANGE + ".submit"
AI_DISCARD = AI + ".discard"
# The fixture core seeds a live AUTO draft on msg-16's thread (Sources/CoreBridge/FixtureReplies.swift, J10.8): the design
# fixture's last row, which no older check opens with R, so R on the first row keeps an empty field.
SEEDED_THREAD = FX["messages"][-1]["id"]
# The two Sur demande switches are set through the core command SetOnDemandOption, by the quick reply's own fixture-only door
# (QuickReplyAIRegistry.optionDoors): the value is in the id, so the cases depend neither on the Settings window nor on its doors.
def option_door(key, on):
    return "mac-main.quickreply.ai.option.%s.%s" % (key.replace("_", "-"), "on" if on else "off")


def ai(s):
    return s.ctl.rows(AI)[0]


def served_draft(s, thread):
    """The thread's live draft as the core serves it (`reply/draft/<thread id>`), the expected values of every case."""
    return s.ctl.core("reply/draft/%s" % thread)


def set_on_demand(s, key, on):
    """Sets one Sur demande switch to `on` (SetOnDemandOption); the cases read `reply/on_demand` back."""
    s.click(option_door(key, on))


def drive_ai_open(j):
    """First of the run: the reply opens on the thread that holds the seeded AUTO draft (by its door, the list's focus stays).
    The draft shows in the field with its mark, as the owner approved (no panel); Escape discards it and closes the slab."""
    s = j.step("par-quickreply-ai", "AI reply in the thread: the auto draft on open")
    RG.step_reset(s)       # the group starts from the launch state, proven by the digest (tests-r58 prune)
    s.click("mac-main.quickreply.open.%s.stay" % SEEDED_THREAD)   # .stay: the main window keeps its surface for the older cases
    seed = {}

    def opened():
        seed["d"] = served_draft(s, field(s, "thread"))
        a = ai(s)
        return (field(s, "thread"), a["mark"], a["made_by"], a["draft_id"] == seed["d"]["id"], field(s, "text") == seed["d"]["text"])
    s.expect("auto-draft-shows-on-open", opened, (SEEDED_THREAD, "1", "auto", True, True))
    s.expect("pill-shows-its-key", lambda: (ai(s)["cap"], ai(s)["label"] != ""), ("⌘J", True))
    s.key("Escape")
    s.expect("escape-discards-the-auto-draft", lambda: (ai(s)["mark"], ai(s)["phase"], field(s, "text"), field(s, "phase")), ("0", "idle", "", "editing"))
    s.key("Escape")                                                  # the slab closes; the older rounds start from R


def drive_ai(j):
    """Last of the run: ⌘J with and without the instruction field, the draft with its mark, a change replacing the text in
    place, typing turning it ordinary, the discard control, the owner's words given back, a refused write, nothing sent alone."""
    s = j.step("par-quickreply-ai", "AI reply in the thread: ⌘J, the mark, the change field")
    st = {}
    s.key("Escape")
    s.key("R")
    set_on_demand(s, "instruction_before", True)
    set_on_demand(s, "change_after", True)
    try:
        st["out0"] = int(s.ctl.core("outbox_entry/count"))
    except Exception:                                                # noqa: BLE001  dry run
        st["out0"] = 0
    s.key("⌘J")
    s.expect("cmd-j-shows-the-instruction-field", lambda: (ai(s)["phase"], ai(s)["instruction_field"], ai(s)["focus"], bool(s.ctl.rows(AI_INSTR)[0]["placeholder"]), field(s, "text")),
             ("asking", "1", "instruction", True, ""))
    s.type("accepte, propose jeudi")
    s.expect("instruction-field-takes-the-typing", lambda: (s.ctl.rows(AI_INSTR)[0]["text"], field(s, "text")), ("accepte, propose jeudi", ""))
    try:
        st["w0"] = ai(s)["writes"]
    except Exception:                                                # noqa: BLE001
        st["w0"] = -1
    s.key("Enter")

    def landed():
        d = served_draft(s, field(s, "thread"))
        a = ai(s)
        return (a["writes"] - st["w0"], a["phase"], a["mark"], a["made_by"], d["instruction"], field(s, "text") == d["text"], a["instruction_field"])
    s.expect("return-writes-the-draft-with-its-mark", landed, (1, "ready", "1", "on_demand", "accepte, propose jeudi", True, "0"))
    s.expect("writing-state-went-before-the-draft", lambda: (ai(s)["writes"] - st["w0"], ai(s)["label"] != "", ai(s)["phase"] != "writing"), (1, True, True))
    s.expect("change-field-shows-under-the-draft", lambda: (ai(s)["change_field"], len(s.ctl.rows(AI_CHANGE)), bool(s.ctl.rows(AI_CHANGE)[0]["placeholder"])), ("1", 1, True))
    try:
        st["id"], st["rev"] = ai(s)["draft_id"], ai(s)["revision"]
    except Exception:                                                # noqa: BLE001
        st["id"], st["rev"] = "?", -1
    s.click(AI_CHANGE)
    s.type("plus court")
    s.key("Enter")

    def changed():
        d = served_draft(s, field(s, "thread"))
        a = ai(s)
        return (a["draft_id"] == st["id"], a["revision"] - st["rev"], field(s, "text") == d["text"], "plus court" in d["text"], a["mark"], s.ctl.rows(AI_CHANGE)[0]["text"])
    s.expect("change-request-replaces-the-text-in-place", changed, (True, 1, True, True, "1", ""))
    s.click(FIELD)
    s.type(" Merci.")
    s.expect("typing-makes-it-an-ordinary-draft",
             lambda: (ai(s)["mark"], ai(s)["change_field"], field(s, "text").endswith(" Merci."), served_draft(s, field(s, "thread"))["state"]), ("0", "0", True, "edited"))
    s.key("Escape")                                                  # its words are the thread's ordinary draft now
    s.click(DRAFT + ".0.delete")
    s.key("R")
    s.expect("an-edited-draft-does-not-come-back-marked", lambda: (ai(s)["mark"], field(s, "text")), ("0", ""))
    # without the instruction field: ⌘J writes at once
    set_on_demand(s, "instruction_before", False)
    s.key("⌘J")
    s.expect("cmd-j-without-the-field-writes-at-once",
             lambda: (s.ctl.core("reply/on_demand")["instruction_before"], ai(s)["instruction_field"], ai(s)["phase"], ai(s)["mark"], served_draft(s, field(s, "thread"))["instruction"]),
             (False, "0", "ready", "1", None))
    s.click(AI_DISCARD)
    s.expect("discard-control-removes-the-draft", lambda: (ai(s)["mark"], field(s, "text"), ai(s)["phase"]), ("0", "", "idle"))
    s.type("Mes mots.")
    s.click(AI)                                                      # the pill is ⌘J
    s.expect("pill-click-writes-over-the-owner-words", lambda: (ai(s)["mark"], field(s, "text") == served_draft(s, field(s, "thread"))["text"]), ("1", True))
    s.key("Escape")
    s.expect("escape-gives-the-owner-words-back", lambda: (ai(s)["mark"], field(s, "text")), ("0", "Mes mots."))
    s.raw("fixture", "fail-next", "draft-reply-on-demand", "the model is not reachable")
    s.key("⌘J")
    s.expect("refused-write-says-why-and-keeps-the-words", lambda: (ai(s)["phase"], ai(s)["words"], field(s, "text")), ("failed", "the model is not reachable", "Mes mots."))
    s.expect("nothing-is-sent-alone", lambda: int(s.ctl.core("outbox_entry/count")) - st["out0"], 0)
    s.key("⌘J")                                                 # tries again: the draft lands over the words
    s.key("⌘Enter")
    s.expect("send-makes-it-ordinary-and-queues-one", lambda: (int(s.ctl.core("outbox_entry/count")) - st["out0"], ai(s)["mark"], len(s.ctl.rows(UNDO_BAR))), (1, "0", 1))
    s.click(UNDO)
    set_on_demand(s, "instruction_before", True)
    s.key("Escape")


def png_rgba(path):
    """A PNG from the app's shot as (width, height, pixel(x, y) -> (r, g, b, a)); standard library only."""
    import struct
    import zlib
    data = open(path, "rb").read()
    pos, chunks = 8, []
    w = h = None
    ctype = 6
    while pos < len(data):
        n, kind = struct.unpack(">I4s", data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + n]
        if kind == b"IHDR":
            w, h, depth, ctype = struct.unpack(">IIBB", body[:10])
            if depth != 8 or ctype not in (2, 6):
                raise ValueError("unsupported PNG: depth %d colour type %d" % (depth, ctype))
        elif kind == b"IDAT":
            chunks.append(body)
        pos += 12 + n
    bpp = 4 if ctype == 6 else 3
    raw = zlib.decompress(b"".join(chunks))
    stride = w * bpp
    rows, prev, i = [], bytearray(stride), 0
    for _ in range(h):
        f, line = raw[i], bytearray(raw[i + 1:i + 1 + stride])
        i += 1 + stride
        for x in range(stride):
            a = line[x - bpp] if x >= bpp else 0
            b, c = prev[x], (prev[x - bpp] if x >= bpp else 0)
            if f == 1: line[x] = (line[x] + a) & 255
            elif f == 2: line[x] = (line[x] + b) & 255
            elif f == 3: line[x] = (line[x] + (a + b) // 2) & 255
            elif f == 4:
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                line[x] = (line[x] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(line)
        prev = line
    def px(x, y):
        o = x * bpp
        r = rows[y]
        return (r[o], r[o + 1], r[o + 2], r[o + 3] if bpp == 4 else 255)
    return w, h, px


# MUTANTS: per check, the mutation of the app that must turn it red.
# par-quickreply/from-is-receiving-account-0 | From reads the thread's fallback account instead of the account that received the message
# par-quickreply/from-is-receiving-account-1 | the same, second account
# par-quickreply/from-is-receiving-account-2 | the same, third account
# par-quickreply/from-check-spans-three-accounts | the list offers fewer than three receiving accounts (the check would prove one account only)
# par-quickreply/reply-ends-with-served-signature | the signature read is the stub (empty), or the reply does not end with the served signature after one blank line
# par-quickreply/open-by-thread-stay-keeps-surface | open-by-thread with moveSurface false still switches the main window to the reader
# par-quickreply/open-by-thread-answers-that-thread | the open-by-thread door ignores the id and answers the focused row
# par-quickreply/emoji-row-opens | the emoji button toggles nothing
# par-quickreply/emoji-row-names-reactions | the picker rows carry no emoji
# par-quickreply/emoji-lands-in-text | a pick inserts nothing into the reply
# par-quickreply/emoji-row-closes-after-pick | the row stays open after a pick
# par-quickreply/emoji-reply-queued | a reply holding an emoji is refused
# par-quickreply/from-selector-shows-sender | the From line shows no address
# par-quickreply/from-menu-opens-with-accounts | the From selector opens nothing
# par-quickreply/from-menu-checks-current | the menu checks no account, or a different one
# par-quickreply/from-pick-switches-sender | a pick in the menu leaves the sender as it was
# par-quickreply/from-menu-closes-after-pick | the menu stays open after a pick
# par-quickreply/send-uses-picked-account | the send leaves from the thread's account, not the picked one
# par-quickreply/from-pick-back-switches-sender | picking the first account again changes nothing
# par-quickreply/send-button-click-sends | a click on the send button queues nothing
# par-quickreply/undo-button-returns-text | the Undo button leaves the field empty
# par-quickreply/send-failure-offers-retry | a refused send shows no Retry
# par-quickreply/retry-button-sends | Retry queues nothing
# par-quickreply/send-button-shows-key-cap | the send button's key cap text reads another key
# par-quickreply/ctrl-enter-sends | the quick reply does not bind ⌃Enter
# par-quickreply/cmd-enter-still-sends | the quick reply does not bind ⌘Enter
# par-quickreply/return-adds-a-line-and-sends-nothing | plain Return sends the reply (the field's `.onKeyPress(.return)` calling model.send, main115), or the engine's "Enter" sends an open reply
# par-quickreply/cmd-return-key-event-sends | a real ⌘Return in the reply's text queues nothing
# par-quickreply/outgoing-card-scrolled-into-view | a new send leaves the thread scrolled where it was, the card out of view
# par-quickreply/outgoing-card-appears | the thread shows no card for a send in its window
# par-quickreply/outgoing-card-waits-with-seconds | the card shows no countdown while the send waits
# par-quickreply/outgoing-card-carries-text | the card shows another text than the one sent
# par-quickreply/outgoing-card-undo-shows-key | the card's Undo has no key cap
# par-quickreply/card-undo-removes-card | Undo on the card leaves the card in the thread
# par-quickreply/card-undo-restores-text | Undo on the card does not put the text back in the field
# par-quickreply/card-becomes-sent-with-time | after the window the card does not become the sent message with its time
# par-quickreply-foot/foot-own-blocks-share-left-edge | the draft card starts flush left while the reply field under it is inset 32 pt (or the reverse)
# par-quickreply-foot/foot-own-blocks-share-right-edge | the draft card and the reply field end at different x (one of them 32 pt short)
# par-quickreply-foot/foot-message-keeps-the-author-offset | a received message is drawn full width (the author offset removed) or the offset is not 32 pt
# par-quickreply-foot/reply-field-material-is-lifted | the closed reply field is drawn flat: its row reads another material than the draft card's lifted glass (app: ReplyBlock.material answers "flat")
# par-quickreply-foot/dots-pill-lies-inside-its-card | the quoted-text pill is placed outside the card that holds it (app: an offset on the pill before its .whooshAx)
# par-quickreply/esc-keeps-words-as-draft-card | Esc with words in the field saves no draft, or the thread shows no draft card
# par-quickreply/draft-card-shows-first-lines | the draft card shows other lines than the draft's first lines
# par-quickreply/draft-card-keys-shown | the draft card's Send or Delete has no key cap
# par-quickreply/list-row-shows-draft-mark | the list row of a thread with a draft shows no draft mark
# par-quickreply/discard-key-in-an-open-reply-removes-the-draft | the Discard key in an open reply leaves the draft (QuickReplyModel.deleteDraft deletes nothing, or the reply does not answer the key)
# par-quickreply/discard-key-closes-the-reply | the discard key deletes the draft and leaves the reply box open (case added 13 Nov; this mutant not yet seen red)
# par-quickreply/no-draft-card-beside-an-open-reply | a draft card is drawn while the reply box is open on the same conversation (ThreadCardsRegistry.cardDraft without its `q.showsBox` line)
# par-quickreply/one-card-for-a-conversation-with-two-drafts | a conversation with two saved drafts draws the older one, or two cards
# par-quickreply/cmd-backspace-removes-the-shown-draft-only | ⌘⌫ removes nothing, or both drafts
# par-quickreply/backspace-key-event-removes-the-draft | a real ⌘⌫ with the card shown and no text holding the keys leaves the draft (the "⌘Backspace" binding removed)
# par-quickreply-paste/pasted-file-opens-the-composer-with-it | a pasted file stays in the reply as its name (QuickReplyModel.takePastedFiles not called)
# par-quickreply-paste/pasted-file-name-is-not-in-the-reply | the name is left in the reply's words after the file went on (draft not set back)
# par-quickreply-paste/pasted-picture-opens-the-composer-with-it | a picture copied as data pasted on the reply is dropped (WHOOSH_ATTACH_MUTANT=reply-picture-dropped), lands as a tile (=picture-as-tile), or is written to the system's temporary folder (=paste-to-temp)
# par-quickreply/draft-card-address-whole-cut-or-hidden-at-960 | at 960x600 the head's address is cut to one stray glyph (ThreadCardHead without its ViewThatFits)
# par-quickreply/draft-card-address-shown-at-1280 | the address is left out at 1280 too (the whole option dropped)
# par-quickreply/draft-click-opens-in-field | a click on the draft card leaves the field without its text
# par-quickreply/reopened-draft-updates-in-place | saving a reopened, edited draft deletes and re-saves it (an extra undo entry) or keeps the old words
# par-quickreply/cards-never-overlap-0 | the thread's stack lays its cards over each other (a negative VStack spacing: a LAYOUT mutant)
# par-quickreply/cards-never-overlap-1 | the same, on the second sample
# par-quickreply/cards-never-overlap-2 | the same, on the third sample
# par-quickreply/delete-removes-draft-card | Delete on the draft card leaves the card
# par-quickreply/delete-offers-undo | Delete on a draft with words offers no Undo
# par-quickreply/list-mark-goes-with-draft | the list row keeps its draft mark after the draft is deleted
# par-quickreply/undo-brings-draft-back | Undo after Delete does not bring the draft back
# par-quickreply/backspace-key-deletes-draft | ⌫ does not delete the thread's draft
# par-quickreply/draft-send-makes-outgoing-card | Send on the draft card makes no outgoing card with its text
# par-quickreply/sent-draft-leaves-drafts | a sent draft stays in the drafts
# par-quickreply/ctrl-enter-key-sends-draft | ⌃↵ does not send the thread's draft
# par-quickreply/draft-card-is-lifted-glass | the draft card's material reads flat while the closed reply field is the lifted glass
# par-quickreply/draft-card-is-back-before-cmd-enter | Esc after the undo leaves no draft card, so the next key proves nothing
# par-quickreply/cmd-enter-key-sends-draft | ⌘↵ does not send the thread's draft (ThreadCardsRegistry.sendShownDraft is bound to ⌃Enter only)
# par-quickreply/header-title-changes-with-words | the header says "Répondre à" with words in the field too
# par-quickreply/header-shows-keys | the header line shows no ⇧H or ⇧⌘P cap
# par-quickreply/header-toggle-opens-rows | the chevron opens no To / Cc / Cci / Objet rows
# par-quickreply/header-rows-carry-subject | the Objet row is empty
# par-quickreply/shift-h-collapses-header | ⇧H does not fold the header
# par-quickreply/typed-cc-lands-in-row | typing after a click on Cc lands in the field, not the Cc row
# par-quickreply/expanded-reply-sends-via-draft | a reply with a Cc sends as a plain quick reply (the Cc is lost)
# par-quickreply/header-folds-while-counting | the header line is not the countdown while the send waits
# par-quickreply/sending-card-is-lifted | a waiting card casts no shadow (it is drawn flat)
# par-quickreply/visible-after-send-0 | the thread does not scroll to its newest card after a send (13 Nov: the check reads the window, the box folds at the send; this mutant not yet seen red)
# par-quickreply/visible-after-send-1 | the same, second send
# par-quickreply/visible-after-send-2 | the same, third send
# par-quickreply/landing-never-holds-a-key | a key on the landing frame waits for the light
# par-quickreply/sent-card-lands-flat | a sent card keeps its glass (never lands)
# par-quickreply/header-returns-after-landing | the header keeps the countdown after the window closes
# par-quickreply/landing-light-ran | the landing light never plays (no perf row of kind landing)
# par-quickreply/failed-card-shows-code | a refused send shows no code (or no failed card)
# par-quickreply/failed-card-retries | Réessayer on the failed card queues nothing
# par-quickreply/failed-card-discard-removes-it | Supprimer leaves the failed card
# par-quickreply/failed-card-offers-retry-and-edit-with-keys | a refused send shows no Edit or no key on it, or still offers Cancel
# par-quickreply/cmd-e-puts-the-failed-words-back-in-the-reply | ⌘E leaves the failed card and an empty reply (QuickReplyModel.editFailed not reached)
# par-quickreply/cancel-leaves-when-the-window-ends | past the undo window the card still draws "Cancel Z" (the pill not gated on the countdown)
# par-quickreply/r-puts-the-cursor-in-the-reply-with-a-draft-card-shown | after a real R the keys go to the list, not the reply's text (the bar never takes the focus)
# par-quickreply/r-puts-the-cursor-in-the-reply-with-no-card | as above, with no draft card in the thread
# par-quickreply/the-list-holds-the-keys-before-r | Escape leaves the open mail holding the keys
# par-quickreply/r-on-a-list-row-puts-the-cursor-in-the-reply | a real R on a list row opens the reply and the keys stay on the list
# par-quickreply/r-puts-the-cursor-in-the-reply-with-a-failed-send-shown | as above, beside a failed send in the thread
# par-quickreply/empty-reply-leaves-no-draft | a reply opened and closed with no words saves an empty draft
# par-quickreply/from-key-opens-picker | ⇧⌘F opens no picker
# par-quickreply/picker-arrow-moves-highlight | ↓ moves no highlight in the picker
# par-quickreply/picker-esc-closes-it | Esc in the picker closes the reply instead of the picker
# par-quickreply-timer/reply-keeps-its-window-after-a-reset-and-an-earlier-send | the undo timer of a send made before `fixture reset` folds the new send that took the same outbox id (the timer compares the id only, not its send's own number)
# par-quickreply/popout-key-opens-composer | ⇧⌘P opens no composer with the words
# par-quickreply-doors/emoji-item-1-lands-in-text | the second reaction of the row inserts nothing
# par-quickreply-doors/emoji-item-2-lands-in-text | the third reaction of the row inserts nothing
# par-quickreply-doors/emoji-item-3-lands-in-text | the fourth reaction of the row inserts nothing
# par-quickreply-doors/emoji-item-4-lands-in-text | the fifth reaction of the row inserts nothing
# par-quickreply-doors/emoji-item-5-lands-in-text | the sixth reaction of the row inserts nothing
# par-quickreply-doors/emoji-item-6-lands-in-text | the seventh reaction of the row inserts nothing
# par-quickreply-doors/emoji-item-7-lands-in-text | the eighth reaction of the row inserts nothing
# par-quickreply-doors/from-filter-field-holds-the-typing | text typed while the From picker is open goes to the reply, not the filter
# par-quickreply-doors/from-filter-narrows-the-menu | the filter leaves every account in the menu
# par-quickreply-doors/from-filtered-row-picks-its-account | a row of the narrowed menu picks the first account instead of its own
# par-quickreply-doors/header-to-chip-x-takes-the-person-off | the x on the To chip does nothing
# par-quickreply-doors/bcc-row-click-takes-the-typing | after a click on the Cci row the typing lands in the reply field
# par-quickreply-doors/subject-row-click-takes-the-typing | after a click on the Objet row the typing lands in the reply field
# par-quickreply-doors/field-click-takes-the-typing-back | a click on the reply field leaves the typing in the header row
# par-quickreply-doors/popout-button-opens-composer | the pop-out button opens no composer with the words
# par-quickreply-ai/auto-draft-shows-on-open | the thread opens with an empty field although the core holds its live auto draft
# par-quickreply-ai/pill-shows-its-key | the AI pill carries no ⌘J cap (or another key)
# par-quickreply-ai/escape-discards-the-auto-draft | Escape closes the slab and leaves the auto draft in place
# par-quickreply-ai/cmd-j-shows-the-instruction-field | ⌘J writes at once although "Donner une consigne avant" is on
# par-quickreply-ai/instruction-field-takes-the-typing | typing while the instruction field is up lands in the reply field
# par-quickreply-ai/return-writes-the-draft-with-its-mark | Return in the instruction field writes without the instruction
# par-quickreply-ai/writing-state-went-before-the-draft | the draft is put in the field without the write going through the writing state
# par-quickreply-ai/change-field-shows-under-the-draft | no change field under the AI draft although "Demander un changement après" is on
# par-quickreply-ai/change-request-replaces-the-text-in-place | a change request makes a new draft instead of rewriting the shown one
# par-quickreply-ai/typing-makes-it-an-ordinary-draft | typing over the AI's words keeps the mark (no AcceptReplyDraft)
# par-quickreply-ai/an-edited-draft-does-not-come-back-marked | a thread whose AI draft was taken over reopens with that text marked as the AI's
# par-quickreply-ai/cmd-j-without-the-field-writes-at-once | ⌘J shows the instruction field although the switch is off
# par-quickreply-ai/discard-control-removes-the-draft | the × beside the mark does nothing
# par-quickreply-ai/pill-click-writes-over-the-owner-words | a click on the AI pill does nothing
# par-quickreply-ai/escape-gives-the-owner-words-back | discarding the draft leaves the field empty, the owner's words lost
# par-quickreply-ai/refused-write-says-why-and-keeps-the-words | a refused write says nothing or clears the owner's words
# par-quickreply-ai/nothing-is-sent-alone | a landed AI draft goes to the outbox by itself
# par-quickreply-ai/send-makes-it-ordinary-and-queues-one | a send of the AI's words leaves the mark on (no AcceptReplyDraft first)


def main():
    path = os.path.abspath(__file__)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    print_cases(r)
    rows = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# par-quickreply") and " | " in l]
    print("MUTANTS (a check is green only after it has been seen red on its mutant)")
    print("| check | mutant |")
    print("|---|---|")
    for k, v in rows:
        print("| %s | %s |" % (k, v))
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


# ---- the selftest: the round-9 checks against a model of the quick reply, clean and once per mutant ----
# QRModel answers the socket verbs drive_doors and drive_doors_tail use, the way the Swift in Sources/Features/QuickReply
# (QuickReplyFeature.clickTargets and rowSources, QuickReplyHeaderActions.swift) is written to answer them. One mutant (named by its check) is switched on at a time.
class QRModel:
    SENDER = "karen@linea-example.com"
    ACC = ["acc%d@mail%d.example" % (i, i) for i in range(5)]
    EMO = ["\U0001F44D", "\U0001F64F", "\U0001F389", "\U0001F60A", "\U0001F440", "✅", "❤️", "\U0001F680"]

    def __init__(self, mut):
        self.m = mut
        self.open, self.draft = True, "Signature test."
        self.hopen, self.to, self.bcc, self.subject, self.focus = False, [self.SENDER], "", "Re: Delivery", "field"
        self.emoji, self.fromopen, self.filter, self.frm = False, False, "", 0
        self.composer, self.ask = None, False
        # lane send-timer: the engine's outbox numbering (from 1 again at a reset), the wall clock, the armed timers and the send's own number
        self.now, self.next_id, self.serial, self.counting, self.timers, self.sent = 0.0, 1, 0, None, [], []

    def on(self, name): return name in self.m

    WINDOW = 10.0

    def advance(self, seconds):
        self.now += seconds

    def tick(self):
        """QuickReplyModel.send's timer: at window + 0.05 s it expires the reply when its own number is still the newest and the id matches
        (the broken copy: the id only). The undo bar's own countdown ends the window at WINDOW (UndoBarView onChange isExpired)."""
        for t in [t for t in self.timers if t[0] <= self.now]:
            self.timers.remove(t)
            _, created, serial = t
            own = self.on("reply-keeps-its-window-after-a-reset-and-an-earlier-send") or serial == self.serial
            if self.counting and own and self.counting[0] == created:
                self.expire()
        if self.counting and self.now >= self.counting[1] + self.WINDOW:
            self.expire()

    def expire(self):
        self.counting = None; self.open = False; self.draft = ""; self.reset_header()

    def send(self):
        if not self.open or self.counting or not self.draft.strip(): return False
        created = "outbox-%d" % self.next_id; self.next_id += 2           # the outbox entry and its undo entry (FixtureCore.queueSendLocked)
        self.serial += 1
        self.counting = (created, self.now, self.serial, self.draft)
        self.timers.append((self.now + self.WINDOW + 0.05, created, self.serial))
        self.sent.append(created); self.draft = ""
        return True

    def reset(self):
        """`fixture reset`: the engine to its seed (numbering from 1, no entry), the reply as at launch (resetToLaunch moves the number); the
        armed DispatchQueue timers are not the engine's and stay."""
        self.next_id, self.sent, self.counting, self.open, self.draft = 1, [], None, False, ""
        self.serial += 1; self.reset_header()

    def reset_header(self):
        self.hopen, self.to, self.bcc, self.subject, self.focus = False, [self.SENDER], "", "Re: Delivery", "field"

    def menu(self):
        t = self.filter.strip().lower()
        if self.on("from-filter-narrows-the-menu"): return list(enumerate(self.ACC))
        return [(i, a) for i, a in enumerate(self.ACC) if not t or t in a.lower()]

    def pop_out(self):
        if not self.open: return False
        self.composer = {"body": self.draft, "to": list(self.to)}
        self.open = False; self.reset_header()
        return True

    def key(self, chord):
        if chord == "Escape":
            if self.fromopen: self.fromopen = False; return True
            if self.open: self.open = False; self.reset_header(); return True
            return False
        if chord == "R":
            if self.open: return True
            self.open = True; self.draft = ""; self.reset_header(); return True
        if chord == "⇧⌘P": return self.pop_out()
        if chord == "\u2318Enter": return self.send()
        if chord == "⇧⌘,":
            if not self.composer: return False
            self.ask = True; return True
        if chord == "Enter":
            if self.composer and self.ask: self.composer = None; self.ask = False; return True
            return False
        return False

    def type(self, text):
        if not self.open: return
        if self.fromopen:
            if self.on("from-filter-field-holds-the-typing"): self.draft += text
            else: self.filter += text
            return
        if self.hopen and self.focus == "bcc" and not self.on("bcc-row-click-takes-the-typing"): self.bcc += text
        elif self.hopen and self.focus == "subject" and not self.on("subject-row-click-takes-the-typing"): self.subject += text
        else: self.draft += text

    def click(self, ax):
        p = "mac-main.quickreply."
        if ax == POPOUT:
            return False if self.on("popout-button-opens-composer") else self.pop_out()
        if not self.open: return False
        if ax == UNDO:
            if not self.counting: return False
            self.draft = self.counting[3]; self.sent.remove(self.counting[0]); self.counting = None; return True
        if ax == EMOJI: self.emoji = not self.emoji; self.fromopen = False; return True
        if ax.startswith(EMOJI + ".") and ax[len(EMOJI) + 1:].isdigit():
            i = int(ax[len(EMOJI) + 1:])
            if not self.emoji or i >= len(self.EMO) or self.on("emoji-item-%d-lands-in-text" % i): return False
            self.draft += self.EMO[i]; self.emoji = False; return True
        if ax == FROM:
            self.fromopen = not self.fromopen
            if self.fromopen: self.emoji = False; self.filter = ""
            return True
        if ax.startswith(FROM + ".") and ax[len(FROM) + 1:].isdigit():
            i = int(ax[len(FROM) + 1:])
            rows = self.menu()
            if not self.fromopen or i >= len(rows): return False
            self.frm = 0 if self.on("from-filtered-row-picks-its-account") else rows[i][0]; self.fromopen = False; return True
        if ax == HEADER_TOGGLE: self.hopen = not self.hopen; return True
        if ax == HEADER_TO + ".0.remove":
            if not self.hopen or not self.to or self.on("header-to-chip-x-takes-the-person-off"): return False
            self.to.pop(0); return True
        if ax in (HEADER_BCC, HEADER_SUBJECT):
            if not self.hopen: return False
            self.focus = "bcc" if ax == HEADER_BCC else "subject"; return True
        if ax == FIELD:
            if self.on("field-click-takes-the-typing-back"): return True
            self.focus = "field"; return True
        return False

    def rows(self, ax):
        if ax == "mac-composer.body": return [{"text": self.composer["body"]}] if self.composer else []
        if ax == "mac-composer.header": return [{"to": self.composer["to"]}] if self.composer else []
        if not self.open: return []
        if ax == FIELD: return [{"ax": ax, "text": self.draft, "phase": "editing", "from": self.ACC[self.frm]}]
        if ax == PICKER: return [{"ax": "%s.%d" % (EMOJI, i), "emoji": e} for i, e in enumerate(self.EMO)] if self.emoji else []
        if ax == FROM: return [{"ax": ax, "address": self.ACC[self.frm], "open": "1" if self.fromopen else "0"}]
        if ax == FROM_MENU: return [{"ax": "%s.%d" % (FROM, n), "address": a, "checked": "1" if i == self.frm else "0"} for n, (i, a) in enumerate(self.menu())] if self.fromopen else []
        if ax == FILTER: return [{"ax": ax, "text": self.filter}] if self.fromopen else []
        if ax == HEADER:
            r = {"ax": ax, "open": "1" if self.hopen else "0", "counting": "1" if self.counting else "0"}
            if self.hopen: r.update({"to": ", ".join(self.to), "cc": "", "bcc": self.bcc, "subject": self.subject})
            return [r]
        if ax == HEADER_TO: return [{"ax": "%s.%d" % (HEADER_TO, i), "address": a} for i, a in enumerate(self.to)] if self.hopen else []
        if ax == UNDO_BAR: return [{"ax": ax}] if self.counting else []
        if ax == OUT: return [{"ax": "%s.%d" % (OUT, i), "id": e} for i, e in enumerate(self.sent)]
        return []

    def handle(self, verb, args):
        self.tick()
        if verb == "fixture" and args[:1] == ["digest"]: return {"ok": True, "digest": "model", "parts": {}, "view": "v"}
        if verb == "fixture" and args[:1] == ["reset"]: self.reset(); return {"ok": True}
        if verb == "key": return {"ok": True} if self.key(args[0]) else {"ok": False, "error": "key does nothing"}
        if verb == "type": self.type(args[0]); return {"ok": True}
        if verb == "click": return {"ok": True} if self.click(args[0]) else {"ok": False, "error": "did nothing"}
        if verb == "rows": return {"ok": True, "rows": self.rows(args[0])}
        return {"ok": False, "error": "nothing"}


def model_run(mut):
    """Drives round 9 over a socket served by QRModel with the mutants `mut` on; returns {check label: (state, detail)}."""
    import json as _json
    import shutil
    import socketserver
    import tempfile
    import threading
    from lib.ctl import Ctl                                              # noqa: E402
    from lib.harness import Journey                                      # noqa: E402
    model = QRModel(mut)
    d = tempfile.mkdtemp(prefix="par-qr9-")

    class H(socketserver.StreamRequestHandler):
        def handle(self):
            for line in self.rfile:
                q = _json.loads(line.decode())
                self.wfile.write((_json.dumps(model.handle(q["verb"], q["args"])) + "\n").encode())
    srv = socketserver.ThreadingUnixStreamServer(os.path.join(d, "whoosh.sock"), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    ctl = None
    try:
        ctl = Ctl(d).connect()
        j = Journey("par-quickreply-doors", ctl)
        drive_doors(j)
        ctl.type("Pop these words out.")                                 # the last case of drive_lift: the key pops the words out
        ctl.key("⇧⌘P")
        drive_doors_tail(j)
        return {c.label: (c.state, c.detail) for c in j.checks()}
    finally:
        if ctl is not None:
            ctl.close()      # on every path: an open connection keeps its handler thread, and server_close() joins it for ever
        srv.shutdown(); srv.server_close()
        shutil.rmtree(d, ignore_errors=True)



def timer_model_run(mut):
    """drive_stale_timer against QRModel on the model's own clock (no wall-clock wait): the reply opens from a reset, as in the app."""
    model = QRModel(mut)
    model.open = False
    return serve_model(model, lambda j: drive_stale_timer(j, sleep=model.advance, now=lambda: model.now))


# The AI reply's model: answers the verbs drive_ai_open and drive_ai use, the way Sources/Features/QuickReply/QuickReplyAI*.swift
# and the fixture core (Sources/CoreBridge/FixtureReplies.swift: the seeded auto draft, the scripted texts) are written to answer them.
class AIModel:
    SEED = "Bonjour,\n\nMerci pour votre message. Je vous envoie la proposition mise à jour avant jeudi.\n\nBien à vous,\nAssem"

    def __init__(self, mut):
        self.m = mut
        self.opts = {"instruction_before": True, "change_after": True}
        self.drafts = [{"id": "reply-draft-seed", "thread": SEEDED_THREAD, "text": self.SEED, "made_by": "auto", "instruction": None, "state": "proposed", "revision": 0}]
        self.ordinary, self.open_draft, self.outbox, self.undo_text, self.armed, self.n = [], None, 0, None, None, 0
        self.open, self.thread, self.text = False, "msg-01", ""
        self.clear()
        self.writes = 0

    def on(self, name): return name in self.m

    def clear(self):
        self.phase, self.ai, self.instr, self.change, self.focus, self.held, self.words = "idle", None, "", "", "reply", "", ""

    def live(self):
        rows = [d for d in self.drafts if d["thread"] == self.thread and d["state"] in ("proposed", "edited")]
        return rows[-1] if rows else None

    def show(self, d):
        self.ai, self.phase, self.change, self.text = dict(d), "ready", "", d["text"]
        if self.focus == "instruction": self.focus = "reply"

    # -- the acts (QuickReplyAIActions.swift)
    def open_reply(self):
        self.open, self.text = True, ""
        self.clear()
        d = self.live()
        if d and self.open_draft is None and (d["state"] == "proposed" or self.on("an-edited-draft-does-not-come-back-marked")) and not self.on("auto-draft-shows-on-open"):
            self.show(d)

    def ai_key(self):
        if not self.open: return False
        if self.phase == "asking": return self.write(None if self.on("return-writes-the-draft-with-its-mark") else self.instr)
        own = self.text.strip() if self.ai is None else ""
        if own: self.held, self.text = self.text, ""
        ask = self.opts["instruction_before"]
        if self.on("cmd-j-shows-the-instruction-field"): ask = False
        if self.on("cmd-j-without-the-field-writes-at-once"): ask = True
        if ask:
            self.instr = own or (self.ai or {}).get("instruction") or ""
            self.phase, self.focus = "asking", "instruction"
            return True
        return self.write(None)

    def write(self, instruction):
        replaceable = self.text
        if not self.on("writing-state-went-before-the-draft"): self.writes += 1
        self.phase, self.focus = "writing", "reply"
        if self.armed:
            self.phase, self.words, self.armed = "failed", self.armed, None
            if self.on("refused-write-says-why-and-keeps-the-words"): self.words = ""
            elif not self.text: self.text, self.held = self.held, ""      # the owner's words come back with the refusal
            return True
        for d in self.drafts:
            if d["thread"] == self.thread and d["state"] == "proposed": d["state"] = "discarded"
        self.n += 1
        w = (instruction or "").strip() or None
        d = {"id": "reply-draft-%d" % self.n, "thread": self.thread, "made_by": "on_demand", "instruction": w, "state": "proposed", "revision": 0,
             "text": "Bonjour,\n\n" + ("Suite à votre message : %s.\n\n" % w if w else "Merci pour votre message.\n\n") + "Bien à vous,\nAssem"}
        self.drafts.append(d)
        self.instr = ""
        if self.text in ("", replaceable): self.show(d)
        if self.on("nothing-is-sent-alone"): self.outbox += 1
        return True

    def request_change(self):
        if self.ai is None or not self.opts["change_after"]: return False
        t = self.change.strip()
        if not t: return True
        self.writes += 1
        cur = [d for d in self.drafts if d["id"] == self.ai["id"]][0]
        if self.on("change-request-replaces-the-text-in-place"):
            self.n += 1
            new = dict(cur, id="reply-draft-%d" % self.n, revision=0)
            cur["state"] = "discarded"; self.drafts.append(new); cur = new
        cur["text"] += "\n\n(Modifié : %s)" % t
        cur["revision"] += 1
        self.show(cur)
        return True

    def accept(self):
        if self.ai is None: return
        d = [x for x in self.drafts if x["id"] == self.ai["id"]][0]
        d["state"] = "edited"
        self.clear()
        self.open_draft = "draft-%d" % len(self.ordinary)
        self.ordinary.append({"id": self.open_draft, "text": d["text"]})

    def discard(self):
        if self.ai is None or self.on("discard-control-removes-the-draft"): return False
        [x for x in self.drafts if x["id"] == self.ai["id"]][0]["state"] = "discarded"
        held = self.held
        self.clear()
        self.text = "" if self.on("escape-gives-the-owner-words-back") else held
        return True

    def escape(self):
        if not self.open: return False
        if self.phase == "asking":
            if self.ai is not None: self.phase, self.instr, self.focus = "ready", "", "reply"; return True
            held = self.held; self.clear()
            if not self.text: self.text = held
            return True
        if self.ai is not None and not self.on("escape-discards-the-auto-draft"): return self.discard()
        # the slab's own Escape: keepAsDraft (an untouched AI draft stays the core's proposal), then closed
        if self.ai is not None and self.ai["text"] == self.text: self.text = self.held
        self.clear()
        if self.text.strip():
            if self.open_draft: [o for o in self.ordinary if o["id"] == self.open_draft][0]["text"] = self.text
            else: self.ordinary.append({"id": "draft-%d" % len(self.ordinary), "text": self.text})
        self.open, self.open_draft = False, None
        return True

    def edited(self):
        if self.ai is not None and self.text != self.ai["text"] and self.phase != "asking" and not self.on("typing-makes-it-an-ordinary-draft"): self.accept()

    def send(self):
        if not self.open or not self.text.strip(): return False
        if not self.on("send-makes-it-ordinary-and-queues-one"): self.accept()
        self.outbox += 1; self.undo_text, self.text = self.text, ""
        if self.open_draft: self.ordinary = [o for o in self.ordinary if o["id"] != self.open_draft]; self.open_draft = None
        return True

    # -- the socket
    def key(self, chord):
        if chord == "R": self.thread = "msg-01"; self.open_reply(); return True   # the focused first row's own thread
        if chord == "⌘J": return self.ai_key()
        if chord == "Escape": return self.escape()
        if chord == "⌘Enter": return self.send()
        if chord == "Enter":
            if self.focus == "instruction" and self.phase == "asking": return self.ai_key()
            if self.focus == "change" and self.ai is not None: return self.request_change()
            return self.send()
        return False

    def type(self, text):
        if not self.open: return
        if self.focus == "instruction" and self.phase == "asking" and not self.on("instruction-field-takes-the-typing"): self.instr += text; return
        if self.focus == "change" and self.ai is not None: self.change += text; return
        self.text += text
        self.edited()

    def click(self, ax):
        if ax == "mac-main.quickreply.open.%s.stay" % SEEDED_THREAD: self.thread = SEEDED_THREAD; self.open_reply(); return True
        if ax == AI: return False if self.on("pill-click-writes-over-the-owner-words") else self.ai_key()
        if ax == AI_DISCARD: return self.discard()
        if ax == AI_CHANGE:
            if self.ai is None or not self.opts["change_after"]: return False
            self.focus = "change"; return True
        if ax == FIELD: self.focus = "reply"; return self.open
        if ax == DRAFT + ".0.delete":
            if not self.ordinary: return False
            self.ordinary.pop(0); return True
        if ax == UNDO:
            if self.undo_text is None: return False
            self.outbox -= 1; self.text, self.undo_text = self.undo_text, None; return True
        for k in self.opts:
            for on in (True, False):
                if ax == option_door(k, on): self.opts[k] = on; return True
        return False

    def ai_row(self):
        d = self.ai
        writing = self.phase == "writing"
        cap = "⌘K" if self.on("pill-shows-its-key") else "⌘J"
        change = d is not None and self.opts["change_after"] and self.phase != "asking" and not self.on("change-field-shows-under-the-draft")
        return {"ax": AI, "phase": self.phase, "mark": "1" if d else "0", "label": "Écriture…" if writing else "Brouillon", "cap": cap,
                "draft_id": d["id"] if d else "", "made_by": d["made_by"] if d else "", "revision": d["revision"] if d else 0, "focus": self.focus,
                "writes": self.writes, "instruction_field": "1" if self.phase == "asking" else "0", "change_field": "1" if change else "0", "words": self.words}

    def rows(self, ax):
        if ax == UNDO_BAR: return [{"ax": ax}] if self.undo_text is not None else []
        if not self.open: return []
        if ax == FIELD: return [{"ax": ax, "thread": self.thread, "text": self.text, "phase": "editing"}]
        if ax == AI: return [self.ai_row()]
        if ax == AI_INSTR: return [{"ax": ax, "text": self.instr, "placeholder": "accepte, propose jeudi"}] if self.phase == "asking" else []
        if ax == AI_CHANGE: return [{"ax": ax, "text": self.change, "placeholder": "Dites ce qu’il faut changer"}] if self.ai_row()["change_field"] == "1" else []
        return []

    def core(self, path):
        if path == "reply/on_demand": return dict(self.opts)
        if path == "outbox_entry/count": return self.outbox
        if path.startswith("reply/draft/"):
            d = [x for x in self.drafts if x["thread"] == path[len("reply/draft/"):] and x["state"] in ("proposed", "edited")]
            if not d: raise KeyError(path)
            d = d[-1]
            return {"id": d["id"], "thread_id": d["thread"], "text": d["text"], "made_by": d["made_by"], "instruction": d["instruction"], "state": d["state"], "revision": d["revision"]}
        raise KeyError(path)

    def handle(self, verb, args):
        try:
            if verb == "key": return {"ok": True} if self.key(args[0]) else {"ok": False, "error": "key does nothing"}
            if verb == "type": self.type(args[0]); return {"ok": True}
            if verb == "click": return {"ok": True} if self.click(args[0]) else {"ok": False, "error": "did nothing"}
            if verb == "rows": return {"ok": True, "rows": self.rows(args[0])}
            if verb == "core": return {"ok": True, "value": self.core(args[1])}
            if verb == "fixture" and args[:1] == ["digest"]: return {"ok": True, "digest": "model", "parts": {}, "view": "v"}
            if verb == "fixture" and args[:1] == ["reset"]: return {"ok": True}
            if verb == "fixture" and args[:1] == ["fail-next"]: self.armed = " ".join(args[2:]); return {"ok": True}
        except KeyError as e:
            return {"ok": False, "error": "no core value at %s" % e}
        return {"ok": False, "error": "nothing"}


def serve_model(model, drive):
    """Drives `drive(j)` over a socket served by `model`; returns {check label: (state, detail)}."""
    import json as _json
    import shutil
    import socketserver
    import tempfile
    import threading
    from lib.ctl import Ctl                                              # noqa: E402
    from lib.harness import Journey                                      # noqa: E402
    d = tempfile.mkdtemp(prefix="par-qrai-")

    class H(socketserver.StreamRequestHandler):
        def handle(self):
            for line in self.rfile:
                q = _json.loads(line.decode())
                self.wfile.write((_json.dumps(model.handle(q["verb"], q["args"])) + "\n").encode())
    srv = socketserver.ThreadingUnixStreamServer(os.path.join(d, "whoosh.sock"), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    ctl = None
    try:
        ctl = Ctl(d).connect()
        j = Journey("par-quickreply-ai", ctl)
        drive(j)
        return {c.label: (c.state, c.detail) for c in j.checks()}
    finally:
        if ctl is not None:
            ctl.close()      # on every path: an open connection keeps its handler thread, and server_close() joins it for ever
        srv.shutdown(); srv.server_close()
        shutil.rmtree(d, ignore_errors=True)


def ai_model_run(mut):
    """drive_ai_open then drive_ai against one AIModel (one run, as in the app: the second starts where the first left)."""
    m = AIModel(mut)
    return serve_model(m, lambda j: (drive_ai_open(j), drive_ai(j)))


class FootModel:
    """The three frames of the thread with a draft, as Sources/Features/Reader/ReaderBlocks.swift and QuickReply/ThreadFootLayout.swift
    lay them out: the owner's blocks share (authorOffset, 0) insets, a received message has the mirror (0, authorOffset)."""
    LEFT, WIDTH = 803.0, 1101.0
    OPEN = "mac-main.reader.message.2"            # the fixture thread's 3 messages: the open card is the third


    def __init__(self, mut):
        self.m = mut

    def frame(self, ax):
        x, w = self.LEFT, self.WIDTH
        if ax == FOOT_DOTS:
            return {"x": x + (w + 10.0 if "dots-pill-lies-inside-its-card" in self.m else 40.0), "y": 150.0, "w": 60.0, "h": 22.0}
        if ax.startswith("mac-main.reader.message."):
            if ax != self.OPEN:
                return None                       # an earlier message, folded: no frame (as on main88)
            w -= 0 if "foot-message-keeps-the-author-offset" in self.m else FOOT_OFFSET
            return {"x": x, "y": 100.0, "w": w, "h": 80.0}
        x, w = x + FOOT_OFFSET, w - FOOT_OFFSET
        if ax == FOOT_DRAFT and "foot-own-blocks-share-left-edge" in self.m:
            x, w = x - FOOT_OFFSET, w + FOOT_OFFSET
        if ax == FOOT_REPLY and "foot-own-blocks-share-right-edge" in self.m:
            w -= FOOT_OFFSET
        return {"x": x, "y": 100.0, "w": w, "h": 80.0}

    def handle(self, verb, args):
        if verb == "shot":
            return {"ok": True, "frames": {i: self.frame(i) for i in args[1:]}}
        if verb == "rows" and args[0] == FOOT_REPLY:
            return {"ok": True, "rows": [{"ax": FOOT_REPLY, "material": "flat" if "reply-field-material-is-lifted" in self.m else "lifted"}]}
        if verb == "rows" and args[0] == FOOT_BODY:
            return {"ok": True, "rows": [{"has_quote": True}]}
        if verb == "rows" and args[0] == FOOT_STATE:
            return {"ok": True, "rows": [{"open": True, "message_count": 3}]}
        return {"ok": False, "error": "nothing"}


def foot_model_run(mut):
    return serve_model(FootModel(mut), drive_foot)


def mutant_suite(prefix, run, title):
    """One suite: clean (all green), then once per declared mutant of `prefix` (each must turn its own check red)."""
    clean = run(set())
    print_model_cases(clean)
    bad = 0
    for label, (state, why) in clean.items():
        if state != "pass":
            print("RED on the model: %s %s" % (label, why)); bad += 1
    print("%s, no mutant: %d of %d checks green" % (title, sum(1 for st, _ in clean.values() if st == "pass"), len(clean)))
    declared = {l.split(" | ")[0][2:].strip(): l.split(" | ", 1)[1].strip() for l in open(os.path.abspath(__file__), encoding="utf-8") if l.startswith("# %s/" % prefix)}
    for label in clean:
        name = label.split("/", 1)[1]
        if label not in declared:
            print("NO MUTANT %s" % label); bad += 1; continue
        got = run({name})
        state = got[label][0]
        others = sorted(k.split("/", 1)[1] for k, (st, _) in got.items() if st == "fail" and k != label)
        print("mutant %-62s -> %s%s" % (name, "RED" if state == "fail" else "NOT RED (%s)" % state, ("  also red: " + ", ".join(others)) if others else ""))
        if state != "fail": bad += 1
    for l in sorted(set(declared) - set(clean)):
        print("STRAY MUTANT %s" % l); bad += 1
    return len(clean), len(declared), bad

def selftest():
    clean = model_run(set())
    print_model_cases(clean)
    bad = 0
    for label, (state, why) in clean.items():
        if state != "pass":
            print("RED on the model: %s %s" % (label, why)); bad += 1
    print("model, no mutant: %d of %d checks green" % (sum(1 for st, _ in clean.values() if st == "pass"), len(clean)))
    declared = {l.split(" | ")[0][2:].strip(): l.split(" | ", 1)[1].strip() for l in open(os.path.abspath(__file__), encoding="utf-8") if l.startswith("# par-quickreply-doors/")}
    for label in clean:
        name = label.split("/", 1)[1]
        if label not in declared:
            print("NO MUTANT %s" % label); bad += 1; continue
        got = model_run({name})
        state = got[label][0]
        others = sorted(k.split("/", 1)[1] for k, (st, _) in got.items() if st == "fail" and k != label)
        print("mutant %-62s -> %s%s" % (name, "RED" if state == "fail" else "NOT RED (%s)" % state, ("  also red: " + ", ".join(others)) if others else ""))
        if state != "fail": bad += 1
    for l in sorted(set(declared) - set(clean)):
        print("STRAY MUTANT %s" % l); bad += 1
    print("%d round-9 checks green on the model, %d mutants, %d not red or missing" % (len(clean), len(declared), bad))
    n, k, b = mutant_suite("par-quickreply-ai", ai_model_run, "AI reply model")
    print("%d AI reply checks green on the model, %d mutants, %d not red or missing" % (n, k, b))
    print("selftest: %d checks, %d mutants, %d not red or missing" % (len(clean) + n, len(declared) + k, bad + b))
    nf, kf, bf = mutant_suite("par-quickreply-foot", foot_model_run, "thread foot model")
    print("%d thread-foot checks green on the model, %d mutants, %d not red or missing" % (nf, kf, bf))
    print("selftest incl. thread foot: %d checks, %d mutants, %d not red or missing" % (len(clean) + n + nf, len(declared) + k + kf, bad + b + bf))
    nt, kt, bt = mutant_suite("par-quickreply-timer", timer_model_run, "send timer model")
    print("%d send-timer checks green on the model, %d mutants, %d not red or missing" % (nt, kt, bt))
    pb = selftest_perf_file()
    # the address judgement on its own: no frame and a readable width pass, a one-glyph width does not
    for frame, want in ((None, False), ({"w": 0.0}, True), ({"w": 96.0}, True), ({"w": 7.0}, False)):
        ok = (address_fit(frame) == "fits") == want
        print("address_fit %r -> %s" % (frame, "ok" if ok else "WRONG"))
        pb = pb or (0 if ok else 1)
    return 1 if bad or b or bf or bt or pb else 0


def selftest_perf_file(read=None, path_of=None):
    """The perf file read (main76 crash): a torn line never stops the run. `read` is the function under test (a mutant passes the old one)."""
    read = read or landing_of_run
    path_of = path_of or perf_log_path
    owner = os.path.expanduser("~/Library/Application Support/Whoosh/perf/sendwave.jsonl")
    whole = '{"at": "2026-11-01T10:00:00Z", "kind": "landing", "state": "done"}'
    since = "2026-11-01T09:00:00Z"
    cases = [("whole row among torn lines", None, lambda: read(["\x00\x00", '{"at": "2026-11-01T0', whole, "{"], since, "f")["state"], "done"),
             ("only torn lines: NOT JUDGED", None, lambda: _outcome(read, ['{"at": "2026', "\x00\x00"], since), "Unreadable"),
             ("no row and no torn line: a miss (red)", None, lambda: _outcome(read, [], since), "KeyError"),
             ("an older run's row is not this run's", None, lambda: _outcome(read, [whole], "2026-11-02T00:00:00Z"), "KeyError"),
             ("a rig copy's file is in its own run folder", None, lambda: path_of({"WHOOSH_SOCKET_DIR": "/run/d1"}), "/run/d1/perf/sendwave.jsonl"),
             ("a test's own file wins", None, lambda: path_of({"WHOOSH_SOCKET_DIR": "/run/d1", "WHOOSH_SENDWAVE_PERF": "/run/own.jsonl"}), "/run/own.jsonl"),
             ("no test folder: no path, never the owner's file", None, lambda: path_of({}), None),
             ("the owner's file is never the answer", None, lambda: path_of({"HOME": os.path.expanduser("~")}) == owner, False)]
    bad = 0
    for name, _, fn, want in cases:
        try:
            got = fn()
        except Exception as e:      # noqa: BLE001
            got = "CRASH " + type(e).__name__
        ok = got == want
        bad += not ok
        print("perf file | %s | %s%s" % (name, "ok" if ok else "WRONG", "" if ok else " got %r want %r" % (got, want)))
    print("perf file: %d cases, %d wrong" % (len(cases), bad))
    return bad


def _outcome(read, lines, since):
    try:
        read(lines, since, "f")
        return "row"
    except Exception as e:      # noqa: BLE001
        return type(e).__name__


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else main())
