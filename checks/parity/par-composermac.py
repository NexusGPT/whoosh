#!/usr/bin/env python3
"""par-composermac: the parity rows of the Mac composer (research/superhuman.md and gotmail.md rows): templates with files,
snippets (⌘;), instant send (⌘⇧Z), send and mark done (⌘⇧Enter), Instant Reply (three options), the Bcc key, the optional
"Sent with Whoosh" footer, and a file handed to the composer from outside (the notch's drop calls acceptFile).

Drives the RUNNING rig copy over the control socket through journeys/lib, like checks/parity/par-quickreply.py.
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/par-composermac.py'
Round 2 (the Superhuman composer keys, step par-composermac-r2): the field keys and their focus ring, the discard question, the
attach panel, the From selector, Move to Bcc, the `;` snippet and `:` emoji lists, the format keys read back from the editor,
the link and colour panels, the key on every control, and one click on every control of the format bar.
Round 3 (step par-composermac-r3, plan K10): the pop-out windows. ⇧R, ⇧⏎, ⇧F, ⇧C open a reply, a reply to all, a forward or a new message
in the composer window (the socket's winframes lists it by its `surface` field); ⇧⌘P pops a reply in to its thread and out again with the same words,
recipients and sender; a draft with files stays out and says why; closing the window keeps the draft; a second draft never replaces the first;
⌘/ pops the draft out and focuses the search; ⌘D goes between the draft and the thread; the window opens with opacity and a 6 pt rise.
Round 6 (lane doors-compose, step par-composermac-r6): one click on every control the rounds before did not click (chip x, list under To, Cc and
Bcc buttons and Return, quoted text, AI draft with Run and Accept, Rewrite and its three choices, Snippets Save and rows, Instant Reply rows, link
Cancel, colour swatches, `;` and `:` rows, Remind and Send later with their date lines, Send, Undo, the chevron menu, the From list, a recent file,
Discard); docs/lanes/doors-compose.md holds the control table, and its two findings (the dictate button, the system file panel).
Round 4 (step par-composermac-r4): a message in its undo window does not hold the composer window (the next draft opens at once), and the
remind-me-if-no-reply key (⇧⌘H): its time list, a pick, and the send handing the time to the core.
    python3 checks/parity/par-composermac.py --selftest   the round-3 to round-6 checks against a model of the app, clean (all green), then once per mutant (each must go red)
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0.
Nothing leaves this Mac: a send reaches the fixture core's outbox only.
"""
import json
import os
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, print_cases, print_model_cases   # noqa: E402
from lib.ctl import Unreadable                                       # noqa: E402
from lib.fixture import ACCOUNT, MSG                                 # noqa: E402

ID = "par-composermac"
TITLE = "Mac composer parity"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "templates, snippets, instant send, send and mark done, Instant Reply, the Bcc key and the footer each show their result in the composer"
M = MSG["msg-01"]
HEAD = "mac-composer.header"
BODY = "mac-composer.body"
FOOTER = "Sent with Whoosh"


def header(s, key):
    return s.ctl.rows(HEAD)[0][key]


def body(s):
    return s.ctl.rows(BODY)[0]["text"]


def settle(secs):
    time.sleep(secs * SPEED["settle"])                              # the selftest sets SPEED to 0: a model has no frames to wait for


def entries(s):
    try:
        return int(s.ctl.core("outbox_entry/count"))
    except Unreadable:
        raise                                                        # dry run: the check is not judged, never failed on a made-up 0
    except Exception:                                                # noqa: BLE001  dry run: the checks below go unjudged
        return 0


def reopen(s):
    s.key("Escape")
    s.click("mac-main.reader.open-full-composer")


class NoBar(Exception):
    """The undo bar did not appear within the bounded wait; the message says what was there instead."""


def read_safe(fn):
    """A read for evidence: never raises, a failed read says so."""
    try:
        return fn()
    except Exception as e:                                           # noqa: BLE001  evidence never hides the verdict it explains
        return "unread (%s: %s)" % (type(e).__name__, str(e)[:120])


def list_context(s):
    """Why the main list holds what it holds, for the evidence of a case that looks a message up in it (main88: the list read empty from round 4
    on, ids []): the list's own state row (filter, label filter, search, tab: InboxList ListProbe.listState) and the shell's surface and mailbox."""
    st = read_safe(lambda: s.ctl.rows("mac-main.list.state")[0])
    ui = read_safe(lambda: s.ctl.uistate())
    pick = lambda d, keys: {k: d.get(k) for k in keys} if isinstance(d, dict) else d
    return {"list state": pick(st, ("filter", "label_filter", "search_active", "tab", "filter_empty", "last_failure")),
            "ui": pick(ui, ("surface", "mailbox", "account_filter", "tab", "first_responder"))}


def bar_state(s):
    """What decides a missing undo bar: the bar's rows, the outbox count, the composer's line, To, text length and draft id, the surface."""
    h = read_safe(lambda: hrow(s))
    hh = h if isinstance(h, dict) else {}
    return "undo bar rows %s; outbox entries %s; composer line %r; to %r; is_open %r; draft id %r; body chars %s; surface %s" % (
        read_safe(lambda: [{k: v for k, v in r.items() if k != "ax"} for r in s.ctl.rows(UB)]), read_safe(lambda: entries(s)), hh.get("line", h),
        hh.get("to"), hh.get("is_open"), hh.get("draft_id"), read_safe(lambda: len(body(s))), read_safe(lambda: surface_of(s)))


def bar_row(s, tries=10):
    """The undo bar's row, waiting for it a bounded time (tries x 0.2 s): a bar the app raises a turn after the click is not absent. A miss raises NoBar with the state."""
    for i in range(tries):
        rows = s.ctl.rows(UB)
        if rows:
            return rows[0]
        if i < tries - 1:
            settle(0.2 * SPEED["settle"])
    raise NoBar("no undo bar after %d reads over %.1f s: %s" % (tries, 0.2 * (tries - 1) * SPEED["settle"], bar_state(s)))


def barred(s, fn):
    """fn(), or the evidence when the bar never came (a miss is a value the case compares, never an IndexError)."""
    try:
        return fn()
    except NoBar as e:
        return {"no undo bar": str(e)}


def drive(j):
    drive_r1(j)
    drive_r2(j)
    drive_r3(j)
    drive_r4(j)
    drive_r5(j)   # typing-lost: the live driver skipped round 5, so its two declared mutant rows read STRAY and the check exited 1 on 134 green (main55)
    drive_r6(j)   # doors-compose: one click on every control the rounds before did not click
    drive_r7(j)   # doors-compose, delta 22: files on the draft go through the core


def drive_r1(j):
    s = j.step("par-composermac", "Mac composer parity")
    mem = {}

    # window slot, From, keys by focus
    s.click("mac-main.reader.open-full-composer")
    s.expect("reply-opens-with-the-cursor-in-the-text", lambda: (settle(0.25), hrow(s)["focused"])[1], "body")
    s.expect("composer-in-window-slot", lambda: s.ctl.uistate().get("surface"), "mac-composer")
    s.expect("from-is-receiving-account", lambda: header(s, "from"), ACCOUNT[M["account"]]["address"])
    s.expect("reply-goes-to-sender", lambda: header(s, "to"), [M["from"]])
    s.type("Thanks Karen.")
    s.expect("outgoing-text-ends-with-the-served-signature", lambda: (lambda h: bool(h["signature"]) and h["outgoing"] == "Thanks Karen.\n\n" + h["signature"])(hrow(s)), True)
    s.expect("typed-text-in-body", lambda: body(s), "Thanks Karen.")
    s.key("Escape")
    s.expect("escape-closes-window", lambda: s.ctl.uistate().get("surface"), "mac-main")
    s.click("mac-main.reader.open-full-composer")
    s.expect("escape-kept-the-draft", lambda: body(s), "Thanks Karen.")

    # snippets
    s.key("⌘;")
    s.expect("snippets-key-opens-list", lambda: len(s.ctl.rows("mac-composer.snippets")) >= 2, True)
    s.click("mac-composer.snippets.0")
    s.expect("snippet-lands-in-text", lambda: "Thanks, received" in body(s), True)
    s.expect("snippet-list-closes", lambda: len(s.ctl.rows("mac-composer.snippets")), 0)

    # templates with files
    s.click("mac-composer.dev.sample-file")
    s.expect("file-attached", lambda: [r["name"] for r in s.ctl.rows("mac-composer.attachments")], ["whoosh-sample-quote.txt"])
    s.click("mac-composer.templates")
    s.click("mac-composer.templates.save")
    s.click("mac-composer.templates")
    s.click("mac-composer.templates")
    s.expect("saved-template-keeps-its-file", lambda: s.ctl.rows("mac-composer.templates")[-1]["files"], 1)
    s.click("mac-composer.templates.0")
    s.expect("template-fills-subject", lambda: header(s, "subject"), "Delivery confirmation")
    s.expect("template-fills-text", lambda: "Please confirm the delivery date" in body(s), True)

    # the Bcc and Cc keys, the footer
    s.expect("bcc-hidden-first", lambda: header(s, "bcc_visible"), False)
    s.key("⌘⇧B")
    s.expect("bcc-key-shows-the-row", lambda: header(s, "bcc_visible"), True)
    s.key("⌘⇧C")
    s.expect("cc-key-shows-the-row", lambda: header(s, "cc_visible"), True)
    s.expect("footer-off-by-default", lambda: FOOTER in header(s, "outgoing"), False)
    s.click("mac-composer.footer")
    s.expect("footer-on-ends-the-text", lambda: header(s, "outgoing").rstrip().endswith(FOOTER), True)
    s.click("mac-composer.footer")
    s.expect("footer-off-again", lambda: FOOTER in header(s, "outgoing"), False)

    # Instant Reply: three options, a pick fills the text
    s.click("mac-composer.dev.fixture-writer")
    s.click("mac-composer.instant-reply")
    settle(0.6)
    s.expect("instant-reply-offers-three", lambda: len(s.ctl.rows("mac-composer.instant-reply.options")), 3)

    def pick():
        mem["pick"] = s.ctl.rows("mac-composer.instant-reply.options")[1]["text"]
        return mem["pick"]
    s.expect("instant-reply-options-are-sentences", lambda: len(pick()) > 0, True)
    s.click("mac-composer.instant-reply.1")
    s.expect("instant-reply-pick-fills-text", lambda: body(s), mem.get("pick", "?"))

    # send: the normal window, then instant, then send and mark done
    try:
        n0 = entries(s)
    except Unreadable:                                               # dry run: the check below is not judged
        n0 = 0
    s.key("⌘Enter")
    s.expect("send-key-reaches-composer", lambda: s.ctl.rows("mac-composer.undo.bar")[0]["undo_offered"], True)
    mem["sending"] = (s.ctl.rows("mac-composer.undo.bar") or [{}])[0].get("text", "") if s.j.ctl else ""      # no bar: the cases above say so; the run goes on
    s.expect("send-queued-once", lambda: entries(s) - n0, 1)
    s.key("Z")
    s.expect("undo-key-brings-text-back", lambda: body(s), mem.get("pick", "?"))
    s.key("⌘⇧Z")
    s.expect("instant-send-says-sent-at-once", lambda: s.ctl.rows("mac-composer.undo.bar")[0]["text"] != mem.get("sending", "Sending"), True)
    s.expect("instant-send-offers-no-undo", lambda: s.ctl.rows("mac-composer.undo.bar")[0]["undo_offered"], False)
    settle(1.6)
    s.click("mac-main.reader.open-full-composer")
    s.type("Done.")
    ids = {}

    def listed():
        ids["before"] = [r["id"] for r in s.ctl.rows("mac-main.list")]
        return M["id"] in ids["before"]
    s.expect("answered-message-listed-before", listed, True)
    s.key("⌘⇧Enter")
    s.expect("send-and-done-queues-a-send", lambda: s.ctl.rows("mac-composer.undo.bar")[0]["undo_offered"], True)
    s.expect("send-and-done-archives-the-answered-message", lambda: M["id"] in [r["id"] for r in s.ctl.rows("mac-main.list")], False)

    # a file handed over from outside opens the composer (the notch's file sink)
    s.raw("clock", "advance", 60)
    settle(1.8)
    s.click("mac-composer.dev.sample-file")
    s.expect("outside-file-opens-the-composer", lambda: s.ctl.uistate().get("surface"), "mac-composer")
    s.expect("outside-file-is-attached", lambda: [r["name"] for r in s.ctl.rows("mac-composer.attachments")], ["whoosh-sample-quote.txt"])


def hrow(s):
    return s.ctl.rows(HEAD)[0]


def fmt(s):
    return s.ctl.rows("mac-composer.format")[0]


def runs(s):
    return s.ctl.rows("mac-composer.runs")


def last_run(s):
    r = runs(s)
    return r[-1] if r else {}


def drive_r2(j):
    s = j.step("par-composermac-r2", "Mac composer: the Superhuman composer keys")
    mem = {}

    # a draft with a subject and text, thrown away: the question first, Esc keeps, Return discards, a new message comes back empty
    s.key("⇧⌘S")
    s.type("Old subject")
    s.key("⇧⌘M")
    settle(0.25)
    s.type("Old body")
    s.key("⇧⌘,")
    s.expect("discard-key-asks-first", lambda: hrow(s)["discard_ask"], True)
    s.key("Escape")
    s.expect("esc-answers-the-question-with-keep", lambda: (hrow(s)["discard_ask"], s.ctl.uistate().get("surface"), body(s).endswith("Old body")), (False, "mac-composer", True))
    s.key("⇧⌘,")
    s.key("Enter")
    s.expect("enter-discards-and-closes-the-composer", lambda: s.ctl.uistate().get("surface"), "mac-main")
    s.expect("a-discarded-draft-is-not-kept", lambda: (lambda r: (r["kept"], r["body_chars"], r["subject_chars"]))(s.ctl.rows("mac-composer.draft")[0]), (False, 0, 0))
    s.click("mac-composer.dev.sample-file")

    # the field keys: the focus ring moves on the key's frame, near rows slide, far rows fade
    # A new message opens with the cursor in To (ComposerModel.openNew: `focus(to.isEmpty ? .to : .body)`), so the To key below would read "to" with no
    # key pressed. The cursor goes to the text first, so the case can only pass when the To key moves it (the selftest model is red on its mutant only so).
    s.key("⇧⌘M")
    s.key("⇧⌘O")
    s.expect("to-key-focuses-to", lambda: hrow(s)["focused"], "to")
    s.key("⇧⌘S")
    s.expect("subject-key-focuses-subject", lambda: hrow(s)["focused"], "subject")
    s.expect("near-rows-slide", lambda: hrow(s)["ring_motion"], "slide")
    s.key("⇧⌘C")
    s.expect("cc-key-focuses-cc", lambda: (hrow(s)["focused"], hrow(s)["cc_visible"]), ("cc", True))
    s.key("⇧⌘B")
    s.expect("bcc-key-focuses-bcc", lambda: (hrow(s)["focused"], hrow(s)["bcc_visible"]), ("bcc", True))
    s.key("⇧⌘F")
    s.expect("from-key-opens-the-sender-list", lambda: (hrow(s)["focused"], hrow(s)["from_open"]), ("from", True))
    s.key("⇧⌘M")
    s.expect("message-key-focuses-the-body", lambda: hrow(s)["focused"], "body")
    s.expect("far-rows-fade", lambda: hrow(s)["ring_motion"], "fade")
    s.expect("message-key-puts-the-cursor-in-the-text", lambda: (settle(0.25), fmt(s)["body_focused"])[1], True)
    s.expect("from-list-closes-when-the-cursor-leaves", lambda: hrow(s)["from_open"], False)

    # the From selector by hand (owner 11:45: it did nothing)
    s.click("mac-composer.from")
    s.expect("from-click-opens-the-list", lambda: hrow(s)["from_open"], True)

    def pick_other():
        mem["before"] = hrow(s)["from"]
        s.click("mac-composer.from.1")
        return hrow(s)["from"] != mem["before"] and not hrow(s)["from_open"]
    s.expect("from-choice-changes-the-sender", pick_other, True)
    s.key("⇧⌘F")
    s.key("Down")
    s.key("Enter")
    s.expect("from-list-keys-pick-with-return", lambda: (hrow(s)["from_open"], hrow(s)["focused"]), (False, "body"))

    # Move to Bcc
    s.key("⇧⌘O")
    s.type("lea@example.com,")
    s.expect("to-takes-typed-recipient", lambda: hrow(s)["to"], ["lea@example.com"])
    s.key("⇧⌘I")
    s.expect("move-key-sends-the-contacts-to-bcc", lambda: (hrow(s)["to"], hrow(s)["bcc"]), ([], ["lea@example.com"]))

    # attach panel
    s.key("⇧⌘A")
    s.expect("attach-key-opens-the-panel", lambda: (hrow(s)["attach_open"], len(s.ctl.rows("mac-composer.attach.panel")) >= 1), (True, True))
    s.key("Escape")
    s.expect("esc-closes-the-attach-panel", lambda: hrow(s)["attach_open"], False)
    s.click("mac-composer.attach")
    s.expect("attach-button-opens-the-panel", lambda: hrow(s)["attach_open"], True)
    s.key("Escape")

    # the cc and bcc buttons and the discard button, once each
    s.click("mac-composer.discard")
    s.expect("discard-button-asks-first", lambda: hrow(s)["discard_ask"], True)
    s.click("mac-composer.discard.keep")
    s.expect("keep-button-keeps-the-draft", lambda: (hrow(s)["discard_ask"], hrow(s)["bcc"]), (False, ["lea@example.com"]))

    # snippets inline and emoji inline
    s.key("⇧⌘M")
    settle(0.25)
    s.key(";")
    s.expect("semicolon-opens-the-snippet-list", lambda: [r["kind"] for r in s.ctl.rows("mac-composer.inline")][:1], ["snippet"])
    s.expect("snippet-list-offers-every-snippet", lambda: len(s.ctl.rows("mac-composer.inline")) >= 2, True)
    s.type("thank")
    s.expect("snippet-list-filters-by-name", lambda: [r["title"] for r in s.ctl.rows("mac-composer.inline")], ["Thanks, received"])
    s.key("Enter")
    s.expect("return-inserts-the-snippet-and-drops-the-trigger", lambda: ("Thanks, received" in body(s), ";" in body(s)), (True, False))
    s.expect("snippet-list-closes-after-the-pick", lambda: len(s.ctl.rows("mac-composer.inline")), 0)
    s.type(" :tad")
    s.expect("colon-name-opens-the-emoji-list", lambda: [r["title"] for r in s.ctl.rows("mac-composer.inline")][:1], [":tada"])
    s.key("Escape")
    s.expect("esc-closes-the-emoji-list-not-the-composer", lambda: (len(s.ctl.rows("mac-composer.inline")), s.ctl.uistate().get("surface")), (0, "mac-composer"))
    s.type("a")
    s.key("Enter")
    s.expect("return-inserts-the-emoji", lambda: "\U0001F389" in body(s), True)
    s.key("⌘;")
    s.expect("snippet-picker-key-opens-the-picker", lambda: len(s.ctl.rows("mac-composer.snippets")) >= 2, True)
    s.key("Escape")

    # format keys: each changes the editor and is read back from it
    s.key("⌘B")
    s.expect("bold-key-turns-bold-on", lambda: fmt(s)["bold"], True)
    s.type("B")
    s.expect("bold-key-marks-the-typed-text", lambda: last_run(s).get("format"), "bold")
    s.key("⌘I")
    s.type("I")
    s.expect("italic-key-adds-italic", lambda: (fmt(s)["italic"], last_run(s).get("format")), (True, "bold+italic"))
    s.key("⌘B")
    s.key("⌘U")
    s.type("U")
    s.expect("underline-key-adds-underline", lambda: (fmt(s)["underline"], last_run(s).get("format")), (True, "italic+underline"))
    s.key("⇧⌘X")
    s.type("S")
    s.expect("strikethrough-key-adds-strike", lambda: (fmt(s)["strike"], last_run(s).get("format")), (True, "italic+underline+strike"))
    s.key("⌘I")
    s.key("⌘U")
    s.key("⇧⌘X")
    s.expect("format-keys-toggle-off", lambda: (fmt(s)["bold"], fmt(s)["italic"], fmt(s)["underline"], fmt(s)["strike"]), (False, False, False, False))

    s.key("⌘O")
    s.expect("color-key-opens-the-colour-panel-in-the-composer", lambda: (hrow(s)["color_open"], len(s.ctl.rows("mac-composer.color.options"))), (True, 4))
    s.click("mac-composer.color.accent")
    s.expect("colour-pick-sets-the-colour", lambda: (hrow(s)["color_open"], fmt(s)["color"]), (False, "accent"))
    s.type("C")
    s.expect("colour-marks-the-typed-text", lambda: last_run(s).get("format"), "color:accent")

    s.key("⌘K")
    s.expect("link-key-opens-the-link-panel", lambda: hrow(s)["link_open"], True)
    settle(0.3)
    s.type("example.com")
    s.click("mac-composer.link.apply")
    s.expect("link-applies-https-to-the-address", lambda: any(r.get("href") == "https://example.com" for r in runs(s)), True)
    s.expect("link-panel-closes", lambda: hrow(s)["link_open"], False)

    # lists, quote and indent on a fresh message
    s.key("⇧⌘,")
    s.key("Enter")
    s.click("mac-composer.dev.sample-file")
    s.key("⇧⌘M")
    settle(0.25)
    s.type("item")
    s.key("⇧⌘8")
    s.expect("bullets-key-makes-a-bullet-item", lambda: (fmt(s)["list"], last_run(s).get("format")), ("bullet", "list-item"))
    s.key("Tab")
    s.expect("tab-indents-a-list-item", lambda: (fmt(s)["indent"], last_run(s).get("indent")), (1, 1))
    s.key("Shift+Tab")
    s.expect("shift-tab-outdents-a-list-item", lambda: fmt(s)["indent"], 0)
    s.key("⇧⌘7")
    s.expect("numbers-key-makes-a-numbered-item", lambda: (fmt(s)["list"], last_run(s).get("format")), ("number", "numbered-item"))
    s.key("⇧⌘9")
    s.expect("quote-key-makes-a-quote", lambda: (fmt(s)["quote"], fmt(s)["list"], last_run(s).get("block")), (True, "none", "quote"))
    s.key("⌘]")
    s.expect("increase-indent-key-indents-the-paragraph", lambda: (fmt(s)["indent"], last_run(s).get("indent")), (1, 1))
    s.key("⌘[")
    s.expect("decrease-indent-key-outdents-the-paragraph", lambda: fmt(s)["indent"], 0)
    s.key("⇧⌘9")

    # one click on every format-bar button, with its visible outcome
    for name, field, want in (("bold", "bold", True), ("italic", "italic", True), ("underline", "underline", True), ("strike", "strike", True),
                              ("list", "list", "bullet"), ("numbers", "list", "number"), ("quote", "quote", True)):
        s.click("mac-composer.format-bar." + name)
        s.expect("format-bar-%s-button-acts" % name, (lambda f=field: fmt(s)[f]), want)
    s.click("mac-composer.format-bar.link")
    s.expect("format-bar-link-button-opens-the-panel", lambda: hrow(s)["link_open"], True)
    s.key("Escape")
    s.click("mac-composer.format-bar.color")
    s.expect("format-bar-color-button-opens-the-panel", lambda: hrow(s)["color_open"], True)
    s.key("Escape")

    # every control shows its key
    keys = {"mac-composer.attach": "⌘⇧A", "mac-composer.from": "⌘⇧F", "mac-composer.send": "⌘↵", "mac-composer.send-later": "⌘⇧L",
            "mac-composer.cc.show": "⌘⇧C", "mac-composer.bcc.show": "⌘⇧B", "mac-composer.discard": "⌘⇧,", "mac-composer.snippets": "⌘;",
            "mac-composer.format-bar.bold": "⌘B", "mac-composer.format-bar.italic": "⌘I", "mac-composer.format-bar.underline": "⌘U",
            "mac-composer.format-bar.strike": "⇧⌘X", "mac-composer.format-bar.link": "⌘K", "mac-composer.format-bar.color": "⌘O",
            "mac-composer.format-bar.list": "⇧⌘8", "mac-composer.format-bar.numbers": "⇧⌘7", "mac-composer.format-bar.quote": "⇧⌘9"}

    def shown():
        rows = {r["ax"]: r for r in s.ctl.rows("mac-composer.controls")}
        norm = lambda k: k.replace("⌘⇧", "⇧⌘")
        return sorted(a for a, k in keys.items() if a not in rows or norm(k) not in norm(rows[a]["tooltip"]))
    s.expect("every-control-shows-its-key", shown, [])
    s.expect("send-and-from-show-a-key-cap", lambda: sorted(a for a in ("mac-composer.send", "mac-composer.from", "mac-composer.send-later")
                                                          if not {r["ax"]: r for r in s.ctl.rows("mac-composer.controls")}[a]["cap"]), [])
    s.key("⇧⌘,")
    s.key("Enter")


POP = "mac-composer.popout"
POP_LINES = {"finish": "Finish or discard this draft before starting another.",
             "only-reply": "Only a reply can go back into its thread.",
             "files": "Files stay in the pop-out window. Remove them to put the reply back in the thread."}
SPEED = {"settle": 1.0}                                              # the selftest sets it to 0: the model has no frames to wait for


def pop(s):
    return s.ctl.rows(POP)[0]


def wins(s):
    """How many windows the socket lists for the composer surface (the pop-out window): by the window's `surface` field, never by its title (the title is the screen's name in words)."""
    return sum(1 for f in s.ctl.winframes() if f.get("surface") == "mac-composer")


def surface(s):
    return s.ctl.uistate().get("surface")


def keys_missing(s, want):
    rows = {r["ax"]: r for r in s.ctl.rows("mac-composer.controls")}
    norm = lambda k: k.replace("⌘⇧", "⇧⌘")
    return sorted(a for a, k in want.items() if a not in rows or norm(k) not in norm(rows[a]["tooltip"]))


def discard_now(s):
    s.key("⇧⌘,")
    s.key("Enter")


def in_thread(s):
    """A thread is open: the reader surface, or the thread overlay (what the shell's ⇧R split reads: ShortcutEngine.threadIsOpen)."""
    try:
        st = s.ctl.uistate()
    except Unreadable:                                               # dry run: nothing to read, nothing to open
        return True
    return st.get("surface") == "mac-reader" or st.get("overlay") == "mac-thread"


def open_thread(s):
    """⇧R is two keys in one chord (shell-keys-r10, MacShellLinks.shiftR): from the plain list the No Reply filter, from an open thread
    the reply pop-out. The pop-out checks press it from an open thread, the way an owner who has a reply to pop out does."""
    if surface_of(s) != "mac-composer" and not in_thread(s):
        s.click("mac-main.list.row.0")
        s.click("mac-main.reader.open")


def surface_of(s):
    try:
        return s.ctl.uistate().get("surface")
    except Unreadable:
        return ""


def shift_r(s):
    """⇧R only where it is the reply pop-out: a thread open, or the composer in front. From the plain list it is the No Reply filter
    (MacShellLinks.shiftR), and main92 showed the check had left it on: `list state` filter 'no-reply', filter_empty True, the list []
    for every later list read (6 cases), and each later ⇧R with no thread to open turned it off and on again. With no thread open after
    open_thread the key is not pressed: the cases after it fail on the composer that did not open, which names the real miss."""
    open_thread(s)
    try:
        st = s.ctl.uistate()
    except Unreadable:                                               # dry run: the key is pressed as before, nothing is read
        st = None
    if st is None or st.get("surface") == "mac-composer" or st.get("surface") == "mac-reader" or st.get("overlay") == "mac-thread":
        s.key("⇧R")
    else:
        s.j.notes.append("shift_r: no thread open and no composer in front (surface %r, overlay %r): ⇧R not pressed, it would be the No Reply filter"
                         % (st.get("surface"), st.get("overlay")))


def drive_r3(j):
    s = j.step("par-composermac-r3", "Mac composer: pop-out windows (⇧R ⇧⏎ ⇧F ⇧C ⇧⌘P ⌘/ ⌘D)")
    mem = {}
    s.expect("composer-window-is-closed-before-the-keys", lambda: wins(s), 0)

    # ⇧R from an open thread: a reply in its own window, the cursor in the text (from the plain list it is the No Reply filter)
    open_thread(s)
    s.expect("thread-is-open-before-shift-r", lambda: in_thread(s), True)
    s.key("⇧R")
    s.expect("shift-r-opens-a-composer-window", lambda: (wins(s), pop(s)["open"]), (1, True))

    def reply_row():
        r = pop(s)
        mem["sender"] = (r["to"] or [""])[0]
        return (r["kind"], bool(mem["sender"]), r["title"])
    s.expect("shift-r-opens-a-reply", reply_row, ("reply", True, "Reply"))
    s.expect("thread-and-pop-in-controls-show-their-keys", lambda: keys_missing(s, {"mac-composer.pop-in": "⇧⌘P", "mac-composer.thread": "⌘D"}), [])
    s.type("Pop text")
    s.expect("popout-typed-text-in-body", lambda: body(s), "Pop text")
    s.key("⇧⌘O")
    s.type("lea@example.com,")

    def two_recipients():
        mem["to"] = hrow(s)["to"]
        return len(mem["to"])
    s.expect("popout-takes-a-typed-recipient", two_recipients, 2)

    # ⇧⌘P: in to the thread's reply box, with the words and the recipients; out again, the same
    s.key("⇧⌘P")
    s.expect("pop-in-closes-the-window", lambda: (wins(s), pop(s)["open"]), (0, False))
    s.expect("pop-in-carries-the-text-to-the-thread-reply", lambda: (pop(s)["slab_open"], pop(s)["slab_text"]), (True, "Pop text"))
    s.expect("pop-in-carries-the-recipients", lambda: pop(s)["slab_to"], mem.get("to", ["?"]))
    s.key("⇧⌘P")
    s.expect("pop-out-opens-the-window-again", lambda: (wins(s), pop(s)["open"], pop(s)["slab_open"]), (1, True, False))
    s.expect("pop-out-brings-the-same-text-back", lambda: body(s), "Pop text")
    s.expect("pop-out-brings-the-same-recipients-back", lambda: hrow(s)["to"], mem.get("to", ["?"]))

    # a draft with a file stays out and says why; closing keeps it; the same key brings it back whole
    s.click("mac-composer.dev.sample-file")
    s.key("⇧⌘P")
    s.expect("pop-in-with-files-is-refused-with-the-reason", lambda: (wins(s), pop(s)["files"], pop(s)["line"]), (1, 1, POP_LINES["files"]))
    s.key("Escape")
    s.expect("closing-the-pop-out-keeps-the-draft", lambda: (wins(s), s.ctl.rows("mac-composer.draft")[0]["kept"], s.ctl.rows("mac-composer.draft")[0]["body_chars"]), (0, True, len("Pop text")))
    shift_r(s)
    s.expect("reopening-brings-text-recipients-and-files-back", lambda: (body(s), hrow(s)["to"], pop(s)["files"]), ("Pop text", mem.get("to", ["?"]), 1))

    # the two window controls, by a click: Thread goes to the main window, Pop in puts the reply back
    discard_now(s)
    shift_r(s)
    s.type("Button text")
    s.click("mac-composer.thread")
    s.expect("thread-button-goes-to-the-thread", lambda: (surface(s), wins(s)), ("mac-main", 1))
    s.key("⌘D")
    s.click("mac-composer.pop-in")
    s.expect("pop-in-button-puts-the-reply-back", lambda: (wins(s), pop(s)["slab_open"], pop(s)["slab_text"]), (0, True, "Button text"))
    s.key("⇧⌘P")
    discard_now(s)

    # ⇧⏎, ⇧F, ⇧C
    s.key("⇧Enter")
    s.expect("shift-enter-opens-reply-all", lambda: (wins(s), pop(s)["kind"], pop(s)["title"]), (1, "reply-all", "Reply all"))
    s.expect("reply-all-keeps-the-sender-first", lambda: (pop(s)["to"] or [""])[0], mem.get("sender", "?"))
    discard_now(s)
    s.key("⇧F")
    s.expect("shift-f-opens-a-forward", lambda: (wins(s), pop(s)["kind"], pop(s)["title"]), (1, "forward", "Forward"))
    s.expect("forward-has-no-recipient-and-a-fwd-subject", lambda: (pop(s)["to"], pop(s)["subject"].startswith("Fwd: "), hrow(s)["focused"]), ([], True, "to"))
    s.expect("forward-carries-the-message", lambda: "Forwarded message" in body(s), True)
    discard_now(s)
    s.key("⇧C")
    s.expect("shift-c-opens-a-new-message", lambda: (wins(s), pop(s)["kind"], pop(s)["to"], pop(s)["subject"]), (1, "new", [], ""))
    s.key("⇧⌘M")
    settle(0.25 * SPEED["settle"])
    s.type("First draft")

    # a new message has no thread to go back to; a second draft never replaces the first
    s.key("⇧⌘P")
    s.expect("a-new-message-stays-out-with-the-reason", lambda: (wins(s), pop(s)["line"], body(s)), (1, POP_LINES["only-reply"], "First draft"))
    shift_r(s)
    s.expect("a-second-draft-never-replaces-the-first", lambda: (pop(s)["kind"], body(s), pop(s)["line"]), ("new", "First draft", POP_LINES["finish"]))

    # ⌘D: to the main window and back, the draft kept
    s.key("⌘D")
    s.expect("switch-draft-key-goes-to-the-main-window", lambda: surface(s), "mac-main")
    s.expect("switch-draft-key-keeps-the-draft-window-up", lambda: (wins(s), s.ctl.rows("mac-composer.draft")[0]["kept"]), (1, True))
    s.key("⌘D")
    s.expect("switch-draft-key-comes-back-to-the-draft", lambda: (surface(s), body(s)), ("mac-composer", "First draft"))
    discard_now(s)

    # ⌘/: the draft goes out and the search takes the keys
    shift_r(s)
    s.type("Search words")
    s.key("⇧⌘P")
    s.key("⌘/")
    s.expect("popout-search-pops-the-draft-out", lambda: (wins(s), pop(s)["slab_open"], body(s)), (1, False, "Search words"))
    s.expect("popout-search-focuses-the-search", lambda: surface(s), "mac-search")
    s.click("mac-composer.discard")
    s.click("mac-composer.discard.confirm")
    s.key("Escape")

    # a window opens with opacity and a short rise
    s.expect("a-window-opens-with-opacity-and-a-short-rise", lambda: (pop(s)["motion"], pop(s)["rise_pt"], pop(s)["ms"] <= 220), ("rise+opacity", 6.0, True))


def drive_r4(j):
    s = j.step("par-composermac-r4", "Mac composer: a sent message does not hold the window; remind me if no reply (⇧⌘H)")
    mem = {}

    # a reply in its undo window, then the next message: the window is free at once (a draft in the outbox is not at risk)
    shift_r(s)
    s.type("Sent at once")
    s.key("⌘Enter")
    s.expect("send-starts-the-undo-window", lambda: s.ctl.rows("mac-composer.undo.bar")[0]["undo_offered"], True)
    s.key("⇧C")
    s.expect("a-message-in-its-undo-window-does-not-hold-the-window", lambda: (pop(s)["kind"], body(s), pop(s)["line"]), ("new", "", ""))
    s.key("Escape")

    # ⇧⌘H: the time list, a pick, and the send hands the time to the core
    shift_r(s)
    s.type("Remind me words")
    s.key("⇧⌘H")
    s.expect("remind-key-opens-the-time-list", lambda: (hrow(s)["remind_open"], len(s.ctl.rows("mac-composer.remind.options")) >= 3), (True, True))
    s.click("mac-composer.remind." + first_line())

    def picked():
        mem["ms"] = hrow(s)["reminder_ms"]
        return (hrow(s)["remind_open"], mem["ms"] > 0)
    s.expect("remind-pick-sets-the-reminder", picked, (False, True))
    s.key("⌘Enter")

    def in_core():
        entry = s.ctl.rows("mac-composer.undo.bar")[0]["outbox_id"]
        return s.ctl.core("reply_reminder/%s/at_ms" % entry) == mem.get("ms", -1)
    s.expect("send-gives-the-reminder-to-the-core", in_core, True)
    # the reminder at its time: no reply came, the clock passes the picked time, the core reads it fired and the thread reads fired by its draft
    fired(s, mem, "reminder-fires-when-no-reply-arrives", reply=False)
    # a reply arrived before the time: the same send, a later time (a thread holds one reminder; the clock already stands past tomorrow morning)
    s.key("⇧C")
    s.key("Escape")
    shift_r(s)
    s.type("Remind me, but they answer")
    s.key("⇧⌘H")
    s.click("mac-composer.remind.nextWeek")
    s.key("⌘Enter")
    fired(s, mem, "reminder-stays-quiet-when-a-reply-arrived", reply=True)
    # "Next week" on a Monday before the morning hour is the Monday after, never the same day (main197 and main198, Monday 16 Nov 06:3x: the line
    # gave today 08:00, a time the core's clock had passed, and the send above carried no reminder). The clock is moved to such a Monday.
    s.expect("next-week-on-a-monday-morning-is-the-monday-after", lambda: next_week_on_monday(s), (7, 8, True))


def first_line():
    """The first reminder's line. The second reminder (Next week) must be later than the first plus the hour the clock is moved past it. On a Sunday
    "Tomorrow morning" and "Next week" name the same Monday 08:00, so there the first is "Later today" (refused after 21:00: the pick case says so)."""
    return "laterToday" if time.localtime().tm_wday == 6 else "tomorrowMorning"


def when_of(rows, kind):
    """The time a line of the snooze picker resolves to (`mac-main.snooze.options`, the time words read on the core's clock)."""
    import datetime as _dt
    row = next((r for r in rows if r.get("id") == kind), None)
    if row is None or not row.get("resolves_to"):
        raise Unreadable("the snooze picker serves no time for %s: %s" % (kind, [(r.get("id"), r.get("resolves_to")) for r in rows]))
    return _dt.datetime.fromisoformat(row["resolves_to"].replace("Z", "+00:00")).astimezone()


def next_week_on_monday(s):
    """Moves the core's clock to the coming Monday at about 03:00 (before the morning hour), then reads the picker's Next week line: the days from
    that Monday to the line's day (7), the line's hour (the morning hour, 8), and that it is later than Tomorrow morning."""
    import datetime as _dt
    now = when_of(s.ctl.rows("mac-main.snooze.options"), "laterToday") - _dt.timedelta(hours=3)        # Later today is the clock plus 3 h, to the half hour
    monday = (now + _dt.timedelta(days=(7 - now.weekday()) % 7 or 7)).replace(hour=3, minute=0, second=0, microsecond=0)
    s.ctl.send("clock", "advance", int((monday - now).total_seconds()))
    rows = s.ctl.rows("mac-main.snooze.options")
    nw, tm = when_of(rows, "nextWeek"), when_of(rows, "tomorrowMorning")
    return ((nw.date() - monday.date()).days, nw.hour, nw > tm)


def core_value(s, path):
    """The core's value at a path, parsed when it comes back as JSON text."""
    v = s.ctl.core(path)
    return json.loads(v) if isinstance(v, str) and v[:1] in "{[" else v


def fired(s, mem, name, reply):
    """A sent reminder at its time. Reads before the move (not fired: the control), then, with `reply`, a reply reaches the thread
    (`fixture reply-arrived`, refused in words where the app does not serve it: the case is then not judged, never red on a missing door),
    then the clock passes the picked time. Judged: `reply_reminder/<outbox id>/fired` (true when no reply came, false when one did) and
    `thread/<message>/reminder` (the state word, naming the draft); then the Reminders view's rows (one more for a fired reminder)."""
    entry = {}
    try:
        entry["id"] = s.ctl.rows("mac-composer.undo.bar")[0]["outbox_id"]
        entry["ms"] = hrow_ms(s, entry["id"])
        entry["mid"] = s.ctl.uistate().get("focus_message")             # the message the reply answers: its thread is the reminder's
    except (Unreadable, IndexError, KeyError) as e:
        # the read that failed is named in the case's own words (main197 and main198: the case said only "KeyError 'mid'")
        entry["id"], entry["mid"], entry["unread"] = None, None, "%s %s after %s" % (type(e).__name__, str(e)[:120], sorted(entry))
    secs = int(max(60, (entry.get("ms") or 0) / 1000 - time.time()) + 3600)     # past the picked time whatever the fixture clock stood at
    before = {}
    for key, path in (("fired", "reply_reminder/%s/fired" % entry["id"]), ("view", "mailbox/reminders/rows")):
        try:
            before[key] = s.ctl.core(path)
        except Unreadable as e:
            before[key] = "unreadable: %s" % e

    def after():
        if entry.get("unread"):
            raise KeyError("the sent reminder could not be read before the move: %s; undo bar rows %d, state %s" % (entry["unread"], len(s.ctl.rows("mac-composer.undo.bar")), {k: s.ctl.uistate().get(k) for k in ("surface", "focus", "editing", "focus_message")}))
        if reply:
            try:
                s.ctl.send("fixture", "reply-arrived", entry["mid"])
            except Unreadable as e:
                raise Unreadable("fixture reply-arrived is not served here, so the reply cannot be sent: %s" % e)
        s.ctl.send("clock", "advance", secs)
        got = s.ctl.core("reply_reminder/%s/fired" % entry["id"])
        thread = core_value(s, "thread/%s/reminder" % entry["mid"])
        thread = thread if isinstance(thread, dict) else {}
        return (before["fired"], got, thread.get("state"), thread.get("draft_id") == entry["id"])
    want = (False, False, "answered", True) if reply else (False, True, "fired", True)
    s.expect(name, after, want)
    if not reply:
        # the Reminders view holds the thread: one row more than before the move. No sidebar door opens that view and no core read lists it
        # (FixtureLabels.mailboxNamed has no `reminders`), so the count is read where the contract names it: `mailbox/reminders/rows`
        s.expect("fired-reminder-is-a-row-of-the-reminders-view", lambda: (before["view"], int(core_value(s, "mailbox/reminders/rows"))), (0, 1))
        # the inbox row of the thread carries the mark: `rows mac-main.list` serves `reminder` on the row of a thread whose reminder fired
        s.expect("fired-reminder-marks-the-inbox-row", lambda: inbox_mark(s, entry.get("mid")), ROW_MARK)


ROW_MARK = True                                                      # the row's `reminder` field is a bool, always present: True for a fired thread, False for every other row (ListProbe.swift `"reminder": r.reminderFired`, gap-remind)


def inbox_mark(s, mid):
    """The `reminder` field of the list row of message `mid` (the thread whose reminder fired); a miss says which ids the list held and what the row served."""
    rows = s.ctl.rows("mac-main.list")
    for r in rows:
        if r.get("id") == mid:
            return r.get("reminder", "no reminder field on the row")
    return "no row for %r in the list; ids %s; %s" % (mid, [r.get("id") for r in rows][:20], list_context(s))


def hrow_ms(s, entry):
    """The time the core holds for the reminder of an outbox entry."""
    v = s.ctl.core("reply_reminder/%s/at_ms" % entry)
    return int(v) if v is not None else None               # no reminder in the core: the case reads fired None and goes red on it


def address_text(s):
    rows = s.ctl.rows("mac-add-account.address")
    return rows[0]["text"] if rows else None


def drive_r5(j):
    s = j.step("par-composermac-r5", "Mac composer: after it closes, typed text reaches the next field (typing-lost)")
    # round 4 ends on a send: Escape does not close a composer whose undo window still counts (ComposerModel.close), so the window is let go first
    s.raw("clock", "advance", 60)
    settle(0.5 * SPEED["settle"])
    s.key("Escape")
    shift_r(s)
    s.type("Words typed in the composer")
    s.key("Escape")
    s.expect("the-composer-is-closed-before-the-sheet", lambda: s.ctl.uistate().get("surface"), "mac-main")   # an unreadable socket = not judged, never a false red (the --dry stand-in)
    s.click("dock.whoosh")
    s.click("mac-main.sidebar.accounts-menu.up")
    s.click("mac-main.sidebar.accounts-menu.add")
    s.click("mac-add-account.address")
    s.type("probe@nothing.example.test")
    s.expect("typing-after-the-composer-closes-reaches-the-next-field", lambda: address_text(s), "probe@nothing.example.test")
    s.key("Escape")


REM = "mac-composer.remind"
SL = "mac-composer.send-later"
COMPL = "mac-composer.to.completions"
NOTHING_TO_SEND = "Add a recipient and some text first."
REWRITES = (("shorter", "The price works for us."), ("warmer", "Thank you so much for the quote."), ("more-formal", "We confirm that the proposed price"))


def chip(field, i):
    return "mac-composer.%s.chip.%d.remove" % (field, i)


def fresh_panel(s, ax):
    """Closes the panel that is open (Escape), then opens the one `ax` opens: a case never starts from the last case's panel."""
    try:
        if hrow(s)["panel"]:
            s.key("Escape")
    except Exception:                                                # noqa: BLE001  dry run: nothing to read
        pass
    s.click(ax)


def seen(mem, key, thunk, default="?"):
    """A value the next click needs, read at drive time. A dry run (nothing to read) keeps `default`."""
    try:
        mem[key] = thunk()
    except Exception:                                                # noqa: BLE001
        mem[key] = default


def acted(s):
    """A Send later line did something: the panel closed with a sentence in the line (the core's answer, either way), or it said in words why not."""
    r = hrow(s)
    return (r["panel"] != "send-later" and r["line"] != NOTHING_TO_SEND) or r["date_refusal"] != ""


def drive_r6(j):
    """Round 6 (lane doors-compose, owner 2026-11-08: "the email sender account selector button isn't working btw, you haven't properly
    tested every button/feature"): one click on every control of the composer that the rounds before did not click, each with its visible
    outcome read back from the header row, the body, or the region the control changes. docs/lanes/doors-compose.md holds the control table."""
    s = j.step("par-composermac-r6", "Mac composer: every control clicked once, with its visible outcome")
    mem = {}
    shift_r(s)
    discard_now(s)                                                   # an earlier round's kept draft goes; the next reply opens clean
    s.click("mac-main.list.row.0")                                   # a message still in the list: an earlier send and mark done archived its own
    s.click("mac-main.reader.open")
    shift_r(s)
    s.type("Doors text.")

    def start():
        r = hrow(s)
        mem["sender"] = (r["to"] or [""])[0]
        return (len(r["to"]), r["cc_visible"], r["bcc_visible"], r["quoted_folded"], body(s))
    s.expect("a-fresh-reply-starts-with-cc-bcc-hidden-and-quoted-folded", start, (1, False, False, True, "Doors text."))

    # the To chip and the list under To
    s.click(chip("to", 0))
    s.expect("to-chip-x-takes-the-recipient-away", lambda: hrow(s)["to"], [])
    s.key("⇧⌘O")
    s.type(mem.get("sender", "kar@x").split("@")[0][:3])
    # the person answered comes first; since 13 Nov the people of all mail with these letters follow (checks/recipients-check.py)
    s.expect("typed-letters-list-the-person-answered", lambda: [r["address"] for r in s.ctl.rows(COMPL)][:1], [mem.get("sender", "?")])
    s.click(COMPL + ".0")
    s.expect("completion-click-adds-the-recipient", lambda: (hrow(s)["to"], len(s.ctl.rows(COMPL))), ([mem.get("sender", "?")], 0))

    # the Cc and Bcc buttons, the typed address, the field's Return, the chip's x
    for f in ("cc", "bcc"):
        addr = "%s-one@example.com" % f
        s.click("mac-composer.%s.show" % f)
        settle(0.3 * SPEED["settle"])
        s.expect("%s-button-shows-the-row-and-takes-the-cursor" % f, lambda f=f: (hrow(s)["%s_visible" % f], hrow(s)["focused"]), (True, f))
        s.type(addr)
        s.expect("%s-takes-the-typed-address" % f, lambda f=f, a=addr: hrow(s)["%s_input" % f], addr)
        s.click("mac-composer.%s.submit" % f)
        s.expect("%s-submit-makes-a-chip" % f, lambda f=f, a=addr: hrow(s)[f], [addr])
        s.click(chip(f, 0))
        s.expect("%s-chip-x-takes-the-recipient-away" % f, lambda f=f: hrow(s)[f], [])

    # the subject, the quoted text
    s.key("⇧⌘S")
    s.type("Doors subject")
    s.expect("subject-takes-the-typed-text", lambda: "Doors subject" in hrow(s)["subject"], True)
    s.click("mac-composer.quoted.toggle")
    s.expect("quoted-button-unfolds-the-quoted-text", lambda: hrow(s)["quoted_folded"], False)
    s.click("mac-composer.quoted.toggle")
    s.expect("quoted-button-folds-it-again", lambda: hrow(s)["quoted_folded"], True)

    # AI draft: the button, its field, Run, Accept; then Rewrite and its three choices
    s.key("⇧⌘M")
    settle(0.25 * SPEED["settle"])
    s.click("mac-composer.dev.fixture-writer")
    s.click("mac-composer.ai-draft")
    s.expect("ai-draft-button-opens-the-field", lambda: hrow(s)["panel"], "ai")
    s.type("Say yes.")
    s.click("mac-composer.ai-draft.run")
    settle(0.6 * SPEED["settle"])
    s.expect("ai-run-button-writes-the-draft", lambda: (body(s).startswith("Hi Karen"), len(s.ctl.rows("mac-composer.ai-marks"))), (True, 1))
    s.click("mac-composer.ai-accept")
    s.expect("ai-accept-button-clears-the-mark", lambda: len(s.ctl.rows("mac-composer.ai-marks")), 0)
    s.click("mac-composer.rewrite")
    s.expect("rewrite-button-opens-the-three-choices", lambda: (hrow(s)["panel"], len(s.ctl.rows("mac-composer.rewrite.choices"))), ("rewrite", 3))
    for n, (slug, want) in enumerate(REWRITES):
        s.click("mac-composer.rewrite." + slug)
        settle(0.5 * SPEED["settle"])
        s.expect("rewrite-%s-replaces-the-paragraph" % slug, lambda w=want: w in body(s), True)
        if n < len(REWRITES) - 1:
            s.click("mac-composer.rewrite")

    # snippets: the button, Save, a row; Instant Reply: the rows the first round did not pick
    s.key("⇧⌘M")
    settle(0.25 * SPEED["settle"])
    s.click("mac-composer.snippets")
    s.expect("snippets-button-opens-the-list", lambda: (hrow(s)["panel"], len(s.ctl.rows("mac-composer.snippets")) >= 2), ("snippets", True))
    s.click("mac-composer.snippets.save")
    s.expect("snippets-save-says-it-saved", lambda: hrow(s)["line"], "Saved as a snippet")
    seen(mem, "sn", lambda: s.ctl.rows("mac-composer.snippets")[1]["text"])
    s.click("mac-composer.snippets.1")
    s.expect("snippet-1-lands-in-the-text", lambda: mem["sn"] in body(s), True)
    for i in (0, 2):
        s.click("mac-composer.instant-reply")
        settle(0.6 * SPEED["settle"])
        seen(mem, "ir%d" % i, lambda i=i: s.ctl.rows("mac-composer.instant-reply.options")[i]["text"])
        s.click("mac-composer.instant-reply.%d" % i)
        s.expect("instant-reply-choice-%d-fills-the-text" % i, lambda i=i: body(s) == mem["ir%d" % i], True)

    # Dictate (doors-compose F1, lane reply-actions): a click starts a session, the second ends it; read from the dictation row, not the button
    drs = lambda: s.ctl.rows("mac-composer.dictation")[0]["state"]

    def listening():
        # the click starts the session in a Task (ComposerMacFeature.toggleDictation -> DictationController.begin, which awaits the
        # recorder's authorize() off the main actor): the state turns a turn or two after the click answers. Read up to 10 times over
        # 0.5 s (main88 read once, at once: 'idle'); a miss names the state and the dictation line (a refusal says why there).
        for _ in range(10):
            if drs() == "listening":
                return "listening"
            settle(0.05)
        row = s.ctl.rows("mac-composer.dictation")[0]
        return "state %r after 0.5 s; line %r; engine %r" % (row.get("state"), row.get("line"), row.get("engine"))
    s.click("mac-composer.dictate")
    s.expect("dictate-button-starts-listening", listening, "listening")
    s.click("mac-composer.dictate")
    settle(0.4 * SPEED["settle"])
    s.expect("dictate-button-again-ends-the-session", lambda: drs(), "idle")

    # the link panel's Cancel, every colour swatch, the rows of the `;` and `:` lists
    s.key("⇧⌘M")
    settle(0.25 * SPEED["settle"])
    s.key("⌘K")
    s.click("mac-composer.link.cancel")
    s.expect("link-cancel-closes-the-panel", lambda: (hrow(s)["link_open"], hrow(s)["panel"]), (False, ""))
    for slug in ("default", "secondary", "accent", "warning"):
        s.key("⌘O")
        s.click("mac-composer.color." + slug)
        s.expect("colour-swatch-%s-applies" % slug, lambda: (hrow(s)["color_open"], fmt(s)["color"]), (False, "" if slug == "default" else slug))
    s.key("⇧⌘M")
    settle(0.25 * SPEED["settle"])
    s.key(";")
    seen(mem, "inl", lambda: s.ctl.rows("mac-composer.inline")[0]["text"])
    s.click("mac-composer.inline.0")
    s.expect("inline-snippet-click-inserts-the-row", lambda: (mem["inl"] in body(s), len(s.ctl.rows("mac-composer.inline"))), (True, 0))
    s.type(" :tad")
    seen(mem, "gl", lambda: s.ctl.rows("mac-composer.inline")[0]["glyph"])
    s.click("mac-composer.inline.0")
    s.expect("inline-emoji-click-inserts-the-glyph", lambda: (mem["gl"] in body(s), len(s.ctl.rows("mac-composer.inline"))), (True, 0))

    # Remind me if no reply: the bell, a time line, the date line and its button, the row that takes it away
    s.click(REM)
    s.expect("remind-button-opens-the-time-list", lambda: (hrow(s)["panel"], len(s.ctl.rows("mac-composer.remind.options")) >= 3), ("remind", True))
    seen(mem, "lt", lambda: [r["refused"] for r in s.ctl.rows("mac-composer.remind.options") if r["ax"].endswith(".laterToday")][0], False)
    s.click(REM + ".laterToday")
    s.expect("remind-later-today-sets-or-says-why", lambda: (hrow(s)["reminder_ms"] > 0) == (not mem["lt"]), True)
    seen(mem, "ms0", lambda: hrow(s)["reminder_ms"], 0)
    fresh_panel(s, REM)
    s.click(REM + ".pickDate")
    s.expect("remind-pick-date-opens-the-picker", lambda: hrow(s)["date_picking"], True)
    s.click(REM + ".set")
    s.expect("remind-set-button-sets-the-picked-time", lambda: (hrow(s)["reminder_ms"] > 0 and hrow(s)["reminder_ms"] != mem["ms0"], hrow(s)["panel"]), (True, ""))
    s.click(REM + ".clear")
    s.expect("remind-clear-removes-the-reminder", lambda: (hrow(s)["reminder_ms"], hrow(s)["reminder_words"]), (0, ""))

    # Send later: the button, its three time lines, the date line and its Schedule button (the fixture core refuses a schedule in words: the line says so)
    s.click(SL)
    s.expect("send-later-button-opens-the-time-list", lambda: (hrow(s)["panel"], len(s.ctl.rows("mac-composer.send-later.options")) >= 3), ("send-later", True))
    for kind in ("laterToday", "tomorrowMorning", "nextWeek"):
        fresh_panel(s, SL)
        s.click(SL + "." + kind)
        s.expect("send-later-%s-acts" % kind, lambda: acted(s), True)
    fresh_panel(s, SL)
    s.click(SL + ".pickDate")
    s.expect("send-later-pick-date-opens-the-picker", lambda: hrow(s)["date_picking"], True)
    s.click(SL + ".schedule")
    s.expect("send-later-schedule-button-acts", lambda: acted(s), True)
    # a new message has no draft in the core: Send later saves it first, then schedules the id the core returned (probes-r2)
    fresh_panel(s, SL)
    s.click(SL + ".pickDate")
    seen(mem, "dr0", lambda: int(s.ctl.core("draft/count")), 0)
    seen(mem, "en0", lambda: entries(s), 0)
    s.click(SL + ".schedule")

    def saved_first():
        # The core schedules only a draft it holds (FixtureCore.scheduleLocked: an unknown id is NotFound); a schedule it takes moves that
        # draft out of the drafts into a `scheduled` outbox entry (core/crates/outbox schedule_send.rs, "the draft leaves the drafts"), one it
        # refuses (a time not in the future on the core's clock) leaves the saved draft. So a save shows as ONE more draft or ONE more
        # scheduled entry, never both, never none (main88 read draft/count alone: 0, the schedule had been taken).
        dd, de = int(s.ctl.core("draft/count")) - mem["dr0"], entries(s) - mem["en0"]
        if (dd, de) == (0, 1):
            st = s.ctl.core("outbox_entry/%d/state" % (mem["en0"],))
            return 1 if st == "scheduled" else "the new outbox entry reads %r, not scheduled" % st
        if (dd, de) == (1, 0):
            return 1
        # A refused schedule (main92: 'pick a time in the future', the core's clock stood past the picked time after round 4's clock moves)
        # leaves the saved draft; when the reply's text was autosaved before, that draft is not a new one (+0). The save is then proven by the
        # composer naming a draft id the core holds.
        did = read_safe(lambda: hrow(s)["draft_id"])
        held = read_safe(lambda: s.ctl.core("draft/%s/from_account_id" % did)) if did else None
        if de == 0 and dd == 0 and did and held and not str(held).startswith("unread ("):
            return 1
        return "drafts %+d, outbox entries %+d; line %r; composer draft id %r, core holds it: %r" % (dd, de, read_safe(lambda: hrow(s)["line"]), did, held)
    s.expect("send-later-saves-the-new-message-first", saved_first, 1)

    # Send, its Undo, the chevron's menu: Send and mark done, Send now
    UB = "mac-composer.undo.bar"
    seen(mem, "n0", lambda: entries(s), 0)
    seen(mem, "body0", lambda: body(s))
    s.click("mac-composer.send")
    s.expect("send-button-queues-one-send", lambda: barred(s, lambda: (bar_row(s)["undo_offered"], entries(s) - mem["n0"])), (True, 1))
    s.click("mac-composer.undo")
    s.expect("undo-button-brings-the-text-back", lambda: (len(s.ctl.rows(UB)), body(s)), (0, mem.get("body0", "?")))
    s.click("mac-composer.send-more")
    s.expect("more-button-opens-the-menu", lambda: hrow(s)["panel"], "more")
    seen(mem, "mid", lambda: hrow(s)["reply_message_id"])           # the message THIS reply answers, read from the app
    seen(mem, "listed", lambda: mem["mid"] in [r["id"] for r in s.ctl.rows("mac-main.list")], False)
    seen(mem, "archived0", lambda: s.ctl.core("message/%s/archived" % mem["mid"]), "unread")   # a miss says whether the core already held it archived before the click
    s.click("mac-composer.send-done")
    def done_read():
        got = (bar_row(s)["undo_offered"], mem["listed"], mem["mid"] in [r["id"] for r in s.ctl.rows("mac-main.list")])
        if got == (True, True, False):
            return got
        ids = [r["id"] for r in s.ctl.rows("mac-main.list")]       # a miss says which message the reply answers, whether the list held it before, the list now and the core's archived flag
        return {"read": got, "reply_message_id": mem["mid"], "listed before the click": mem["listed"], "list ids now": ids[:20],
                "core archived flag of that message": read_safe(lambda: s.ctl.core("message/%s/archived" % mem["mid"])),
                "core archived flag before the click": mem.get("archived0"), "list context": list_context(s), "composer": read_safe(lambda: bar_state(s))}
    s.expect("send-and-done-button-sends-and-archives", lambda: barred(s, done_read), (True, True, False))
    s.click("mac-composer.undo")
    s.click("mac-composer.send-more")
    s.click("mac-composer.send-instant")
    s.expect("send-now-button-sends-without-undo", lambda: barred(s, lambda: bar_row(s)["undo_offered"]), False)
    settle(1.8 * SPEED["settle"])

    # a new reply: every row of the From list, a recent file, Discard and its question's Discard button
    shift_r(s)
    s.type("Doors again.")
    mem["froms"] = []
    seen(mem, "n_acc", lambda: hrow(s)["accounts"], 3)
    for i in range(mem["n_acc"]):
        s.click("mac-composer.from")
        s.click("mac-composer.from.%d" % i)
        try:
            mem["froms"].append(hrow(s)["from"])
        except Exception:                                            # noqa: BLE001  dry run
            pass
    s.expect("every-choice-of-the-from-list-sets-its-sender", lambda: (s.ctl, (len(set(mem["froms"])), len(mem["froms"])) == (mem["n_acc"], mem["n_acc"]))[1], True)   # s.ctl: unjudged in a dry run
    s.click("mac-composer.dev.sample-file")
    s.click("mac-composer.attachment-files.0.menu.remove")           # AttachmentsMac's chip menu (par-attachments clicks it too): the file goes, its name stays among the recent files
    seen(mem, "att0", lambda: len(s.ctl.rows("mac-composer.attachments")), -1)
    s.click("mac-composer.attach")
    s.click("mac-composer.attach.recent.0")
    s.expect("attach-recent-row-attaches-the-file", lambda: (mem["att0"], [r["name"] for r in s.ctl.rows("mac-composer.attachments")]), (0, ["whoosh-sample-quote.txt"]))
    s.click("mac-composer.discard")
    s.click("mac-composer.discard.confirm")
    s.expect("discard-confirm-button-throws-the-draft-away", lambda: (surface(s), s.ctl.rows("mac-composer.draft")[0]["kept"]), ("mac-main", False))


ATT = "mac-composer.attachments"
NOTES = "mac-composer.attachment-notes"
SAMPLE_NAME = "whoosh-sample-quote.txt"
DEV = "mac-composer.dev."
UB = "mac-composer.undo.bar"


def tiles(s):
    return [(r["name"], r["state"]) for r in s.ctl.rows(ATT)]


def tile_names(s):
    return [n for n, _ in tiles(s)]


def core_files(s, path):
    """The core's read of a draft's or an outbox entry's files: (name, size, state) for each."""
    v = s.ctl.core(path)
    d = json.loads(v) if isinstance(v, str) else (v or {"attachments": []})
    return [(a["file_name"], a["size_bytes"], a["state"]) for a in d["attachments"]]


def draft_names(s):
    return [n for n, _, _ in core_files(s, "draft/%s/attachments" % hrow(s)["draft_id"])]


def outbox_files(s):
    return core_files(s, "outbox/%s/attachments" % bar_row(s)["outbox_id"])


def files_state(s):
    """What decides a files case when it is red (never raises: a read that fails says so): the composer's own line (the sentence it shows), the draft
    id the composer holds (`rows mac-composer.header` draft_id: empty = no core draft), the draft's attachment list as the core serves it
    (`core state draft/<id>/attachments`), the tiles and the note rows. NOT served by the socket today: the core's answer to an attach or to a
    forward (an attach error only sets the line, ComposerModel+Files.swift:73-75; a refused forward returns without a trace, :154-156)."""
    def read(fn):
        try:
            return fn()
        except Exception as e:                                           # noqa: BLE001  evidence never hides the verdict it explains
            return "unread (%s: %s)" % (type(e).__name__, str(e)[:120])
    h = read(lambda: hrow(s))
    did = h.get("draft_id") if isinstance(h, dict) else None
    line = h.get("line") if isinstance(h, dict) else h
    raw = read(lambda: s.ctl.core("draft/%s/attachments" % did)) if did else "no draft id, not read"
    return "composer line %r; draft id %r; core draft files %s; tiles %s; notes %s; undo bar rows %s" % (
        line, did, (str(raw)[:300] if isinstance(raw, str) else repr(raw)[:300]), read(lambda: tiles(s)), read(lambda: [r["words"] for r in s.ctl.rows(NOTES)]),
        read(lambda: len(s.ctl.rows(UB))))


def expect_files(s, name, thunk, expected):
    """s.expect, and on a miss the value carries files_state beside what the thunk read."""
    def run():
        try:
            got = thunk()
        except Unreadable as e:                                          # still not judged, with the same evidence beside the reason
            raise Unreadable("%s | %s" % (e, files_state(s)))
        except (IndexError, KeyError) as e:                              # a row the case reads is missing (main83: 'reply shape: IndexError' alone): say which read, with the evidence
            got = "no row to read (%s: %s)" % (type(e).__name__, e)
        return got if got == expected else {"read": got, "evidence": files_state(s)}
    s.expect(name, run, expected)


def wait_composer(s, tries=10):
    """True once the composer's body row and header answer (waits tries x 0.2 s, bounded); else the state that says why not."""
    why = "no read"
    for i in range(tries):
        try:
            if s.ctl.rows(BODY):
                return True
            why = "no body row; surface %s; composer windows %s" % (surface_of(s), read_safe(lambda: wins(s)))
        except Unreadable:
            return "dry"
        except Exception as e:                                       # noqa: BLE001
            why = "unread (%s: %s)" % (type(e).__name__, str(e)[:120])
        settle(0.2 * SPEED["settle"])
    return why


def wait_tile(s, name, state, tries=10):
    """True once the tile `name` reads `state` (a read of the tiles settles the core's answer); else the tiles, the composer's line and its draft id."""
    for i in range(tries):
        try:
            if (name, state) in tiles(s):
                return True
        except Unreadable:
            return "dry"
        settle(0.2 * SPEED["settle"])
    return "tiles %s; line %r; draft id %r" % (read_safe(lambda: tiles(s)), read_safe(lambda: hrow(s)["line"]), read_safe(lambda: hrow(s)["draft_id"]))


def forward_source(s, mid):
    """Click the list row of message `mid` (found by id, never by a default row) and check the focus followed. None when the row was opened;
    else the evidence a case compares: the list's ids with their file counts, the focused message and the core's archived flag of `mid`."""
    try:
        rows = s.ctl.rows("mac-main.list")
    except Unreadable:
        return None                                                  # dry run: the body goes on, the checks are not judged
    ids = [r["id"] for r in rows]
    if mid in ids:
        s.click("mac-main.list.row.%d" % ids.index(mid))
        focus = read_safe(lambda: s.ctl.uistate().get("focus_message"))
        if focus == mid or focus is None:                            # None: a model that does not serve the focused message (the real app serves it)
            return None
        return {"forward source": "%s is row %d but the focus is %r after the click" % (mid, ids.index(mid), focus), "list ids": ids[:20]}
    return {"forward source": "%s is not in the list" % mid, "list ids with file counts": [(r["id"], r.get("attachments")) for r in rows][:20],
            "focused message": read_safe(lambda: s.ctl.uistate().get("focus_message")), "core archived flag of %s" % mid: read_safe(lambda: s.ctl.core("message/%s/archived" % mid)),
            "list context": list_context(s)}


def remove_tile(i):
    return "mac-composer.attachment-files.%d.menu.remove" % i


def drive_r7(j):
    """Round 7 (lane doors-compose, contract delta 22): a file attached in the composer is on the core's draft, the tiles are drawn from the core's
    read, the x removes it there, the send carries it, a forward opens with the original's files, each refusal says its sentence."""
    s = j.step("par-composermac-r7", "Mac composer: files on the draft go through the core")
    mem = {}
    shift_r(s)
    discard_now(s)
    shift_r(s)
    # The reply must be open, with its To and its body row, before the text goes in. A key typed with no composer open goes to the list, and the
    # file doors then open a NEW message (ComposerMacFeature.acceptFile) with no recipient: every send below is refused ("Add a recipient and some
    # text first.") and no undo bar comes (main79, log lines 178 to 183). The wait is bounded; a miss is carried into the first case.
    mem["open"] = wait_composer(s)
    # Round 17 (lead ruling, main111): the socket code never ends or begins an undo group by hand; the rig boundary only breaks the typing run.
    # On the hidden copy no event ends the window undo manager's group (round 10's table), so text the socket's `type` puts in and keys typed after
    # it stay one group whatever the app does, and "one Undo takes only the typed keys" cannot hold there. The case is now what a real keyboard
    # makes: ONE run of real key presses, and one ⌘Z takes the whole typed run back (AppKit's typing coalescing, as TextEdit and Mail do;
    # round 11 notes, point 4). The Edit menu's Redo brings it back, so the file cases after it have their text.
    # Cmd-Z in the body undoes typed text through `keyevent` (the real key path). Judged again: lane keys-r6's fix (c11ffbca, "the surface's window
    # wins when it also holds a text focus") is an ancestor of the build under test. On a miss the line carries the text before and after the key,
    # the key verb's reply and `rows mac-keyevent.monitor` (seen, sent, arm, last: Sources/ControlSocket/KeyEventVerb.swift:16-34).
    # The words to undo are typed as REAL keys (`keyevent`, one key press per character), as the owner types them: the socket's `type` puts text
    # in from outside (RichTextEditor.swift RichStorageSwap.replace drops the view's typing entries on purpose, main81 crash), so text it put in
    # has no undo step and ⌘Z then falls through to the app's undo (main88: `can_undo` False, the composer closed).
    s.raw("keyevent", TYPED_RUN)

    def cmd_z():
        if mem["open"] is not True:
            return "the reply composer did not open after the key: %s" % mem["open"]
        def read_body():
            """The body text, or the words that say why there is none (main83: the composer was gone, `rows mac-composer.body` answered no row, and the case
            printed only 'reply shape: IndexError'): the surface and the header's flags decide it."""
            try:
                return body(s)
            except (IndexError, KeyError) as e:
                st = read_safe(lambda: s.ctl.uistate().get("surface"))
                hd = read_safe(lambda: {k: s.ctl.rows(HEAD)[0].get(k) for k in ("is_open", "has_draft", "draft_id")})
                return "no body row (%s: %s); surface %r; header %r" % (type(e).__name__, e, st, hd)
        before = read_body()
        if before != TYPED_RUN:                                          # the keys did not all land: the Undo would prove nothing
            return "the typed run is not in the body before the key: %r" % before
        n_out = read_safe(lambda: entries(s))
        reply = s.ctl.send("keyevent", "⌘Z")                             # a refusal (an older build: unknown verb) is Unreadable: not judged, with the build's own words
        after = read_body()
        for _ in range(4):                                               # the undo may land a frame later: up to 4 more reads, capped
            if after == "":
                break
            settle(0.2 * SPEED["settle"])
            after = read_body()
        if after == "":
            return after
        try:
            mon = s.ctl.rows("mac-keyevent.monitor")
        except Unreadable as e:
            mon = "unread (%s)" % e
        # what decides it FIRST (main93 printed only a cut text): which route took the key, what the undo step said, whether the key sent
        # the reply (an outbox entry more: ⇧⌘Z is the instant send of ComposerKeyLayer) and the composer's phase line; then the texts
        n_after = read_safe(lambda: entries(s))
        sent = (n_after - n_out) if isinstance(n_out, int) and isinstance(n_after, int) else "unread"
        lead = "route %r, undo %r, handled %r, can_undo %r; outbox entries more %s; composer line %r" % (
            reply.get("route"), reply.get("undo"), reply.get("handled"), reply.get("can_undo"), sent, read_safe(lambda: hrow(s)["line"]))
        return "%s | text before the key %r, after %r; keyevent reply %r; rows mac-keyevent.monitor %r" % (lead, before, after, reply, mon)
    s.expect("cmd-z-in-the-body-undoes-the-typed-text", cmd_z, "")

    def editor_and_copy():
        # One source of truth (round 10): the editor's own text (`editor_text`, the NSTextView's string) and the composer's copy (`text`,
        # the session's document) read the same after the undo. main95 to main103: the editor read '' while the copy kept the words.
        got = None
        for _ in range(5):                                               # the editor reloads from the document a frame later: up to 5 reads
            r = s.ctl.rows(BODY)[0]
            got = ((r.get("editor_text") or "").strip(), (r.get("text") or "").strip())
            if got == ("", ""):
                break
            settle(0.2 * SPEED["settle"])
        return got
    s.expect("cmd-z-leaves-the-editor-and-the-copy-equal", editor_and_copy, ("", ""))
    # Redo by the Edit menu's Redo (`mac-composer.dev.redo` sends its `redo:`; ⇧⌘Z in the text is instant send by design): the words come
    # back in the editor AND in the composer's copy (the copy follows any change of the text storage, undo and redo included).
    s.click("mac-composer.dev.redo")

    def redone():
        got = None
        for _ in range(5):
            r = s.ctl.rows(BODY)[0]
            got = ((r.get("editor_text") or "").strip(), (r.get("text") or "").strip())
            if got == (TYPED_RUN, TYPED_RUN):
                break
            settle(0.2 * SPEED["settle"])
        return got
    s.expect("edit-menu-redo-brings-the-words-back-in-the-editor-and-the-copy", redone, (TYPED_RUN, TYPED_RUN))

    # The file cases need the reply composer of this step (with its recipient): when the Cmd-Z case left none open (main88 and main92: the
    # composer was gone after the key, and the file door then opened a NEW message with no recipient, 16 cases red in a chain), it is opened
    # again here, so those cases judge files and not the Cmd-Z case's outcome.
    try:
        reopen = not s.ctl.rows(BODY)
    except Unreadable:
        reopen = False
    if reopen:
        s.j.notes.append("r7: no composer after the Cmd-Z case: the reply is opened again for the file cases")
        shift_r(s)
        mem["open"] = wait_composer(s)
        s.type("Files text.")
    # attach one; attach a second while the core has not answered (the tile is there at once, in the being added state)
    s.click(DEV + "sample-file")
    s.expect("attached-file-is-on-the-core-draft", lambda: (tiles(s), draft_names(s)), ([(SAMPLE_NAME, "ready")], [SAMPLE_NAME]))
    s.click(DEV + "hold-files")
    s.click(DEV + "mid-file")
    s.expect("tile-shows-being-added-before-the-core-answers", lambda: (tiles(s)[-1][1], [r["words"] for r in s.ctl.rows(NOTES)]), ("adding", ["Adding"]))
    s.click(DEV + "hold-files")
    seen(mem, "mid", lambda: tiles(s)[-1][0])
    s.expect("tile-settles-to-ready-on-the-core-read", lambda: (tiles(s)[-1][1], len(draft_names(s))), ("ready", 2))
    s.expect("second-file-joins-the-first", lambda: (tile_names(s), draft_names(s)), ([SAMPLE_NAME, mem["mid"]], [SAMPLE_NAME, mem["mid"]]))

    # remove one, send, read what the outbox entry carries, undo gives the files back
    s.click(remove_tile(0))
    s.expect("x-removes-the-file-from-the-core-draft", lambda: (tile_names(s), draft_names(s)), ([mem["mid"]], [mem["mid"]]))
    s.click("mac-composer.send")
    s.expect("send-carries-the-files-to-the-outbox-entry", lambda: barred(s, lambda: outbox_files(s)), [(mem["mid"], 8 * 1048576, "ready")])
    s.click("mac-composer.undo")
    s.expect("undo-gives-the-files-back", lambda: (len(s.ctl.rows(UB)), draft_names(s)), (0, [mem["mid"]]))
    # the core does not keep the draft a send took (FixtureCore.swift `.send`: draftRows[draftId] = nil; its cancel gives nothing back): the composer
    # must send the text again anyway, and the second undo gives the files back once more
    s.click("mac-composer.send")
    s.expect("send-after-the-undo-goes-again", lambda: barred(s, lambda: (bar_row(s), len(s.ctl.rows(UB)), outbox_files(s))[1:]), (1, [(mem["mid"], 8 * 1048576, "ready")]))
    s.click("mac-composer.undo")
    s.expect("second-undo-gives-the-files-back", lambda: (len(s.ctl.rows(UB)), draft_names(s)), (0, [mem["mid"]]))

    # each refusal says its sentence on the line; the refused file has no tile
    for name, dev, sentence in (("too-large-file-is-refused-with-its-sentence", "big-file", "This file is too large to send."),
                                ("files-too-large-together-are-refused-with-their-sentence", "mid-file", "These files are too large to send together."),
                                ("missing-file-is-refused-with-its-sentence", "gone-file", "This file is no longer on this Mac."),
                                ("unusable-file-is-refused-with-its-sentence", "bad-file", "This file cannot be attached.")):
        s.click(DEV + dev)
        s.expect(name, lambda sentence=sentence: (tile_names(s), hrow(s)["line"]), ([mem["mid"]], sentence))

    # a file deleted after it was attached reads gone; the send is refused until it is removed
    s.click(DEV + "sample-file")
    # The core answers an attach a turn after the click (`scheduleSettle`; a read of the tiles settles it at once). The file is deleted only once its
    # tile reads ready: deleted first, the attach is refused as gone and there is no tile to read gone (main79, log line 181).
    seen(mem, "sample_ready", lambda: wait_tile(s, SAMPLE_NAME, "ready"), "dry")
    s.click(DEV + "delete-sample")
    expect_files(s, "deleted-file-reads-gone-on-its-tile", lambda: ([st for n, st in tiles(s) if n == SAMPLE_NAME], [r["words"] for r in s.ctl.rows(NOTES)]) if mem["sample_ready"] in (True, "dry") else {"the sample tile never read ready before the file was deleted": mem["sample_ready"]}, (["gone"], ["Not on this Mac"]))
    s.click("mac-composer.send")
    expect_files(s, "send-with-a-gone-file-is-refused-with-its-sentence", lambda: (len(s.ctl.rows(UB)), hrow(s)["line"]), (0, "This file is no longer on this Mac."))
    seen(mem, "gi", lambda: tile_names(s).index(SAMPLE_NAME), 1)
    s.click(remove_tile(mem["gi"]))
    s.click("mac-composer.send")
    s.expect("send-goes-once-the-gone-file-is-removed", lambda: barred(s, lambda: bar_row(s)["undo_offered"]), True)
    s.click("mac-composer.undo")
    discard_now(s)

    # a forward opens with the original's files as tiles, each removable, and its send carries what is left
    # msg-02 carries two files. It is found by id: a default row (the old `.index(...)` fell back to row 1) opened the forward of reply-35, a message
    # with no files, so no core draft was made (main79, log lines 184 to 186). A message the list does not hold is said, with the list and the core's archived flag.
    mem["fwd_miss"] = forward_source(s, "msg-02")
    if not mem["fwd_miss"]:
        s.key("⇧F")
    expect_files(s, "forward-opens-with-the-originals-files-as-tiles", lambda: mem["fwd_miss"] or (tile_names(s), draft_names(s)), (["brand-book-v3.pdf", "wordmark.svg"], ["brand-book-v3.pdf", "wordmark.svg"]))
    if not mem["fwd_miss"]:
        s.click(remove_tile(0))
    expect_files(s, "forward-file-removed-leaves-the-other", lambda: mem["fwd_miss"] or (tile_names(s), draft_names(s)), (["wordmark.svg"], ["wordmark.svg"]))
    s.key("⇧⌘O")
    s.type("lea@example.com,")
    s.click("mac-composer.send")
    expect_files(s, "forward-send-carries-the-remaining-file", lambda: barred(s, lambda: [n for n, _, _ in outbox_files(s)]), ["wordmark.svg"])
    s.click("mac-composer.undo")
    discard_now(s)

    # a forward the core refuses says why on the composer's line (the line a refused send uses); the draft opens with no files.
    # On the real app the test door `fixture forward-refuse on` (start-fixes a107a507) makes the fixture core refuse the forward in its own
    # words, and the composer's line shows them; the stand-in refuses the forward of msg-03 itself and shows the catalogue sentence.
    live = not REFUSED_FORWARD["staged"]
    if live:
        s.raw("fixture", "forward-refuse", "on")
    mem["refuse_miss"] = forward_source(s, "msg-03")
    if not mem["refuse_miss"]:
        s.key("⇧F")
    want = "The server refused to forward this message." if live else "This message could not be forwarded."
    s.expect("forward-the-core-refuses-says-its-sentence", lambda: mem["refuse_miss"] or (tile_names(s), hrow(s)["line"]), ([], want))
    if live:
        s.raw("fixture", "forward-refuse", "off")
    discard_now(s)

    # ⌘V of a copied FILE in the text (owner 2026-11-12: "copy-paste of file should add attachment"): a real key press (`keyevent`) reaches
    # the text view's paste (WhooshTextView.paste); the file is on the rig's PRIVATE pasteboard as the Finder's Copy puts it (its URL and
    # its name as text: `mac-main.attachments.dev.stage-file`), never on the owner's. The file becomes a tile; its name never enters the text.
    shift_r(s)
    wait_composer(s)
    s.type("Paste words.")
    s.click("mac-main.attachments.dev.stage-file")
    s.raw("keyevent", "⌘V")
    expect_files(s, "pasted-file-becomes-a-tile-not-text", lambda: (wait_tile(s, PASTED, "ready"), PASTED in body(s)), (True, False))
    # A picture copied as DATA pasted in the text becomes a file the engine reads at send: it lives under the app's own data folder
    # (`pasted-files/<uuid>/`, the test copy's run folder), never the system's temporary folder a cleanup empties (lead ruling, round 7).
    # One rule for a paste (lead ruling, round 7, as mail clients do): picture DATA goes into the text as a picture (insertPicture, as a
    # drop on the text), never a tile; a copied FILE is a tile (the case above).
    seen(mem, "pics0", lambda: len(hrow(s).get("pictures") or []), 0)
    s.click("mac-main.attachments.dev.stage-image")
    s.raw("keyevent", "⌘V")
    expect_files(s, "pasted-picture-goes-into-the-text", lambda: pasted_picture_inline(s, mem["pics0"]), True)
    expect_files(s, "pasted-picture-lives-in-the-app-data-folder", lambda: pasted_picture_home(s), True)
    discard_now(s)

    # Round 20 (owner 2026-11-13 07:27, "hitting enter actually sends", "not cmd+enter"): in the composer's body a REAL plain Return (`keyevent`)
    # adds a line and nothing else: the composer stays open, no undo bar, no outbox entry; a real ⌘Return sends.
    shift_r(s)
    wait_composer(s)
    s.raw("keyevent", RETURN_WORDS)
    seen(mem, "ret_out0", lambda: entries(s), 0)
    s.raw("keyevent", "Return")

    def return_in_body():
        # the editor's own text: the draft's text field drops a last empty line until a letter follows it (walk compret-143: editor 'Line one.' + a new line, draft 'Line one.')
        editor = lambda: (lambda r: r.get("editor_text") if r.get("editor_text") is not None else r.get("text"))(s.ctl.rows(BODY)[0])
        got = (read_safe(editor), len(s.ctl.rows(UB)), read_safe(lambda: entries(s) - mem["ret_out0"]), read_safe(lambda: s.ctl.uistate().get("surface")))
        want = (RETURN_WORDS + "\n", 0, 0, "mac-composer")
        return "same" if got == want else {"read (body, undo bars, outbox entries more, surface)": got, "want": want}
    s.expect("return-in-the-body-adds-a-line-and-sends-nothing", return_in_body, "same")
    s.raw("keyevent", "\u2318Return")
    s.expect("cmd-return-in-the-body-sends", lambda: barred(s, lambda: bar_row(s)["undo_offered"]), True)
    s.click("mac-composer.undo")
    discard_now(s)

    # The rig's undo boundary must never end the test copy (main107 to main109: with the search field focused, `fixture reset` ended the copy:
    # "NSCellUndoManager ... must begin a group before registering undo", the field editor's event group ended by hand, then SwiftUI wrote the field
    # in the same pass). The search field takes the keys, `fixture reset` runs (a verb that inserts from outside an event), and the copy still
    # answers two `uistate` reads.
    s.key("/")
    s.raw("fixture", "reset")

    def copy_answers():
        got = []
        for _ in range(2):
            try:
                got.append(bool(s.ctl.uistate().get("surface")))
            except Unreadable as e:                                      # the copy is gone: a FAIL with its words, never "not judged"
                got.append("no answer: %s" % e)
            settle(0.2 * SPEED["settle"])
        return got
    s.expect("fixture-reset-with-the-search-field-focused-keeps-the-copy-alive", copy_answers, [True, True])


RETURN_WORDS = "Line one."                                            # real key presses before the plain Return (round 20)
TYPED_RUN = "Files text. Undo me."                                     # one run of real key presses in the reply's body (r7 Cmd-Z cases)
PASTED = "sample-2-pages.pdf"                                         # the fixture file the rig's stage-file door puts on its pasteboard
PASTED_PICTURE = "Pasted image"                                       # the name a pasted picture is given (AttachmentIntake.writeImage)
RIG_DIR = {"path": os.environ.get("WHOOSH_SOCKET_DIR", "")}           # the test copy's run folder (the selftest sets its model's)


def pasted_picture_inline(s, before):
    """True when the paste put one picture more in the text and no "Pasted image" tile; else what was read. Up to 20 reads over 2 s (the
    picture is written off the main thread, then placed)."""
    for _ in range(20):
        if len(hrow(s).get("pictures") or []) > before:
            break
        settle(0.1 * SPEED["settle"])
    pics, tiles_now = len(hrow(s).get("pictures") or []), read_safe(lambda: tile_names(s))
    as_tile = isinstance(tiles_now, list) and any(str(n).startswith(PASTED_PICTURE) for n in tiles_now)
    return True if pics == before + 1 and not as_tile else "pictures in the text %d (were %d); tiles %s" % (pics, before, tiles_now)


def pasted_picture_home(s):
    """True when the pasted picture in the text has its file under `<run folder>/pasted-files/` (header row `picture_sources`); else what
    was read."""
    srcs = [p for p in (hrow(s).get("picture_sources") or []) if PASTED_PICTURE in os.path.basename(p)]
    if not srcs:
        return "no pasted picture among the text's picture sources %s" % read_safe(lambda: hrow(s).get("picture_sources"))
    path, root = srcs[-1], RIG_DIR["path"]
    inside = bool(root) and os.path.abspath(path).startswith(os.path.join(os.path.abspath(root), "pasted-files") + os.sep)
    return True if inside else "source %r is not under %r" % (path, os.path.join(root, "pasted-files"))
REFUSED_FORWARD = {"staged": False}                                  # the selftest sets it True: the model refuses the forward of msg-03


# MUTANTS: per check, the mutation of the app that must turn it red.
# par-composermac/composer-in-window-slot | the composer opens no window and leaves the surface on the main window
# par-composermac/from-is-receiving-account | From takes the first account instead of the one the message names
# par-composermac/escape-kept-the-draft | Escape discards the draft
# par-composermac/snippets-key-opens-list | the snippets key does nothing
# par-composermac/snippet-lands-in-text | a snippet pick inserts nothing
# par-composermac/saved-template-keeps-its-file | a saved template forgets its files
# par-composermac/template-fills-subject | applying a template leaves the subject empty
# par-composermac/bcc-key-shows-the-row | the Bcc key does nothing
# par-composermac/footer-on-ends-the-text | the footer switch adds no line to the outgoing text
# par-composermac/footer-off-again | the footer stays after it is switched off
# par-composermac/instant-reply-offers-three | Instant Reply offers one option
# par-composermac/instant-reply-pick-fills-text | a pick leaves the text unchanged
# par-composermac/send-key-reaches-composer | the quick reply takes Cmd-Enter before the composer
# par-composermac/instant-send-offers-no-undo | instant send keeps the undo window open
# par-composermac/send-and-done-archives-the-answered-message | send and mark done only sends
# par-composermac/outside-file-opens-the-composer | a file from outside attaches to nothing when the composer is closed
# par-composermac/answered-message-listed-before | the list does not hold the answered message, so the archive check below proves nothing
# par-composermac/bcc-hidden-first | the Bcc row starts visible
# par-composermac/cc-key-shows-the-row | the Cc key does nothing
# par-composermac/escape-closes-window | Escape leaves the composer on screen
# par-composermac/file-attached | an attached file is not listed
# par-composermac/footer-off-by-default | the footer starts switched on
# par-composermac/instant-reply-options-are-sentences | the options are empty strings
# par-composermac/instant-send-says-sent-at-once | instant send shows the counting bar
# par-composermac/outside-file-is-attached | a file from outside opens the composer without the file
# par-composermac/reply-goes-to-sender | a reply opens with no recipient
# par-composermac/send-and-done-queues-a-send | send and mark done archives without sending
# par-composermac/send-queued-once | a send queues two outbox entries
# par-composermac/outgoing-text-ends-with-the-served-signature | the composer reads no signature from the core, so the outgoing text has none
# par-composermac/snippet-list-closes | the snippet list stays open after a pick
# par-composermac/template-fills-text | applying a template leaves the text unchanged
# par-composermac/typed-text-in-body | typed text never reaches the body
# par-composermac/undo-key-brings-text-back | Z cancels the send and clears the text
# par-composermac-r2/discard-key-asks-first | the discard key throws the draft away with no question
# par-composermac-r2/esc-answers-the-question-with-keep | Escape on the question discards the draft
# par-composermac-r2/enter-discards-and-closes-the-composer | Return on the question does nothing
# par-composermac-r2/a-discarded-draft-is-not-kept | discard closes the composer but keeps the draft for the next open
# par-composermac-r2/to-key-focuses-to | the To key does nothing
# par-composermac/reply-opens-with-the-cursor-in-the-text | a reply opens with the cursor nowhere
# par-composermac-r2/subject-key-focuses-subject | the Subject key focuses the body
# par-composermac-r2/near-rows-slide | the ring always fades
# par-composermac-r2/cc-key-focuses-cc | the Cc key shows the row and leaves the focus in To
# par-composermac-r2/bcc-key-focuses-bcc | the Bcc key shows the row and leaves the focus in To
# par-composermac-r2/from-key-opens-the-sender-list | the From key does nothing
# par-composermac-r2/message-key-focuses-the-body | the Message key does nothing
# par-composermac-r2/far-rows-fade | the ring always slides
# par-composermac-r2/message-key-puts-the-cursor-in-the-text | the Message key moves the ring but not the cursor
# par-composermac-r2/from-list-closes-when-the-cursor-leaves | the sender list stays open when the cursor goes to the body
# par-composermac-r2/from-click-opens-the-list | the From button does nothing
# par-composermac-r2/from-choice-changes-the-sender | a choice in the sender list leaves From unchanged
# par-composermac-r2/from-list-keys-pick-with-return | Return in the sender list does nothing
# par-composermac-r2/to-takes-typed-recipient | text typed in To never becomes a recipient
# par-composermac-r2/move-key-sends-the-contacts-to-bcc | the Move to Bcc key does nothing
# par-composermac-r2/attach-key-opens-the-panel | the Attach key does nothing
# par-composermac-r2/esc-closes-the-attach-panel | Escape leaves the attach panel open
# par-composermac-r2/attach-button-opens-the-panel | the paperclip opens the system panel only and shows no panel
# par-composermac-r2/discard-button-asks-first | the trash button discards at once
# par-composermac-r2/keep-button-keeps-the-draft | the Keep button discards
# par-composermac-r2/semicolon-opens-the-snippet-list | the semicolon opens no list
# par-composermac-r2/snippet-list-offers-every-snippet | the list holds one snippet
# par-composermac-r2/snippet-list-filters-by-name | the list ignores what is typed
# par-composermac-r2/return-inserts-the-snippet-and-drops-the-trigger | Return inserts the snippet and leaves the typed trigger
# par-composermac-r2/snippet-list-closes-after-the-pick | the list stays open after the pick
# par-composermac-r2/colon-name-opens-the-emoji-list | a colon and a name open no list
# par-composermac-r2/esc-closes-the-emoji-list-not-the-composer | Escape closes the composer with the list open
# par-composermac-r2/return-inserts-the-emoji | Return inserts nothing for an emoji
# par-composermac-r2/snippet-picker-key-opens-the-picker | the snippet picker key does nothing
# par-composermac-r2/bold-key-turns-bold-on | the bold key does nothing
# par-composermac-r2/bold-key-marks-the-typed-text | bold is on but the typed text stays plain
# par-composermac-r2/italic-key-adds-italic | the italic key does nothing
# par-composermac-r2/underline-key-adds-underline | the underline key does nothing
# par-composermac-r2/strikethrough-key-adds-strike | the strikethrough key does nothing
# par-composermac-r2/format-keys-toggle-off | a second press leaves the mark on
# par-composermac-r2/color-key-opens-the-colour-panel-in-the-composer | the colour key opens the reader's links instead
# par-composermac-r2/colour-pick-sets-the-colour | a colour pick changes nothing
# par-composermac-r2/colour-marks-the-typed-text | the colour is set but the typed text keeps the default
# par-composermac-r2/link-key-opens-the-link-panel | the link key opens the search
# par-composermac-r2/link-applies-https-to-the-address | the link panel applies nothing
# par-composermac-r2/link-panel-closes | the link panel stays open after Apply
# par-composermac-r2/bullets-key-makes-a-bullet-item | the bullets key does nothing
# par-composermac-r2/tab-indents-a-list-item | Tab in a list switches the split tab
# par-composermac-r2/shift-tab-outdents-a-list-item | Shift Tab in a list switches the split tab back
# par-composermac-r2/numbers-key-makes-a-numbered-item | the numbers key makes a bullet
# par-composermac-r2/quote-key-makes-a-quote | the quote key does nothing
# par-composermac-r2/increase-indent-key-indents-the-paragraph | the increase indent key does nothing
# par-composermac-r2/decrease-indent-key-outdents-the-paragraph | the decrease indent key does nothing
# par-composermac-r2/format-bar-bold-button-acts | the bold button does nothing
# par-composermac-r2/format-bar-italic-button-acts | the italic button does nothing
# par-composermac-r2/format-bar-underline-button-acts | the underline button does nothing
# par-composermac-r2/format-bar-strike-button-acts | the strike button does nothing
# par-composermac-r2/format-bar-list-button-acts | the bullets button does nothing
# par-composermac-r2/format-bar-numbers-button-acts | the numbers button does nothing
# par-composermac-r2/format-bar-quote-button-acts | the quote button does nothing
# par-composermac-r2/format-bar-link-button-opens-the-panel | the link button does nothing
# par-composermac-r2/format-bar-color-button-opens-the-panel | the colour button does nothing
# par-composermac-r2/every-control-shows-its-key | one control drops its key from the tooltip
# par-composermac-r2/send-and-from-show-a-key-cap | the From row shows no key cap
# par-composermac-r3/composer-window-is-closed-before-the-keys | a discard closes the composer but leaves its window on the screen
# par-composermac-r3/thread-is-open-before-shift-r | the open control opens no thread, so ⇧R would be the No Reply filter
# par-composermac-r3/shift-r-opens-a-composer-window | ⇧R opens no window
# par-composermac-r3/shift-r-opens-a-reply | ⇧R opens a new message with no recipient
# par-composermac-r3/thread-and-pop-in-controls-show-their-keys | the Pop in control shows no key in its tooltip
# par-composermac-r3/popout-typed-text-in-body | text typed in the pop-out never reaches the body
# par-composermac-r3/popout-takes-a-typed-recipient | an address typed in To never becomes a recipient
# par-composermac-r3/pop-in-closes-the-window | ⇧⌘P puts the draft in the thread and leaves the window up
# par-composermac-r3/pop-in-carries-the-text-to-the-thread-reply | ⇧⌘P closes the window and leaves the reply box empty
# par-composermac-r3/pop-in-carries-the-recipients | the reply box gets the words but only the original recipient
# par-composermac-r3/pop-out-opens-the-window-again | ⇧⌘P in the reply box opens no window
# par-composermac-r3/pop-out-brings-the-same-text-back | the window opens empty and the words are lost
# par-composermac-r3/pop-out-brings-the-same-recipients-back | the window opens with the original recipient only
# par-composermac-r3/pop-in-with-files-is-refused-with-the-reason | ⇧⌘P puts a draft with files in the thread and drops the files without a word
# par-composermac-r3/closing-the-pop-out-keeps-the-draft | Escape on the window throws the draft away
# par-composermac-r3/reopening-brings-text-recipients-and-files-back | closing the window forgets the attached files
# par-composermac-r3/thread-button-goes-to-the-thread | the Thread button does nothing
# par-composermac-r3/pop-in-button-puts-the-reply-back | the Pop in button does nothing
# par-composermac-r3/shift-enter-opens-reply-all | ⇧⏎ opens a plain reply
# par-composermac-r3/reply-all-keeps-the-sender-first | reply all leaves the sender out of To
# par-composermac-r3/shift-f-opens-a-forward | ⇧F opens a reply
# par-composermac-r3/forward-has-no-recipient-and-a-fwd-subject | a forward keeps "Re:" and the sender as the recipient
# par-composermac-r3/forward-carries-the-message | a forward opens with an empty body
# par-composermac-r3/shift-c-opens-a-new-message | ⇧C opens nothing
# par-composermac-r3/a-new-message-stays-out-with-the-reason | ⇧⌘P on a new message closes the window and the draft is lost
# par-composermac-r3/a-second-draft-never-replaces-the-first | ⇧R replaces the draft the window holds with a reply
# par-composermac-r3/switch-draft-key-goes-to-the-main-window | ⌘D does nothing
# par-composermac-r3/switch-draft-key-keeps-the-draft-window-up | ⌘D closes the draft window
# par-composermac-r3/switch-draft-key-comes-back-to-the-draft | the second ⌘D does nothing
# par-composermac-r3/popout-search-pops-the-draft-out | ⌘/ opens the search and leaves the draft in the thread
# par-composermac-r3/popout-search-focuses-the-search | ⌘/ pops the draft out and leaves the search closed
# par-composermac-r3/a-window-opens-with-opacity-and-a-short-rise | the window opens with a scale instead of a rise
# par-composermac-r4/send-starts-the-undo-window | the send key queues nothing, so the undo bar never appears
# par-composermac-r4/a-message-in-its-undo-window-does-not-hold-the-window | a message in its undo window still blocks the next draft
# par-composermac-r4/remind-key-opens-the-time-list | the remind key does nothing
# par-composermac-r4/remind-pick-sets-the-reminder | a time pick sets no reminder
# par-composermac-r4/send-gives-the-reminder-to-the-core | the send forgets the reminder
# par-composermac-r4/reminder-fires-when-no-reply-arrives | the reminder never fires: the clock passing its time changes nothing
# par-composermac-r4/fired-reminder-is-a-row-of-the-reminders-view | the Reminders view lists no row for a fired reminder
# par-composermac-r4/reminder-stays-quiet-when-a-reply-arrived | the reminder fires although a reply arrived before its time
# par-composermac-r4/next-week-on-a-monday-morning-is-the-monday-after | on a Monday before 08:00 the Next week line names the same day (TimeInWords.options without its same-day test)
# par-composermac-r4/fired-reminder-marks-the-inbox-row | the inbox row of a thread whose reminder fired carries no reminder mark
# par-composermac-r5/the-composer-is-closed-before-the-sheet | Escape leaves the composer window up on its surface
# par-composermac-r5/typing-after-the-composer-closes-reaches-the-next-field | the closed composer's view still takes the shared typed buffer and inserts it into its body
# par-composermac-r6/a-fresh-reply-starts-with-cc-bcc-hidden-and-quoted-folded | the Cc and Bcc rows start visible
# par-composermac-r6/to-chip-x-takes-the-recipient-away | the x on a To chip does nothing
# par-composermac-r6/typed-letters-list-the-person-answered | the list under To offers nobody
# par-composermac-r6/completion-click-adds-the-recipient | a click on a row of the list under To adds nobody
# par-composermac-r6/cc-button-shows-the-row-and-takes-the-cursor | the Cc button does nothing
# par-composermac-r6/cc-takes-the-typed-address | text typed in Cc goes nowhere
# par-composermac-r6/cc-submit-makes-a-chip | Return in Cc makes no chip
# par-composermac-r6/cc-chip-x-takes-the-recipient-away | the x on a Cc chip does nothing
# par-composermac-r6/bcc-button-shows-the-row-and-takes-the-cursor | the Bcc button does nothing
# par-composermac-r6/bcc-takes-the-typed-address | text typed in Bcc goes nowhere
# par-composermac-r6/bcc-submit-makes-a-chip | Return in Bcc makes no chip
# par-composermac-r6/bcc-chip-x-takes-the-recipient-away | the x on a Bcc chip does nothing
# par-composermac-r6/subject-takes-the-typed-text | text typed after the Subject key lands in the message
# par-composermac-r6/quoted-button-unfolds-the-quoted-text | the quoted-text button does nothing
# par-composermac-r6/quoted-button-folds-it-again | the quoted-text button only unfolds
# par-composermac-r6/ai-draft-button-opens-the-field | the AI draft button opens no field
# par-composermac-r6/ai-run-button-writes-the-draft | the Run button of the AI field writes nothing
# par-composermac-r6/ai-accept-button-clears-the-mark | the Accept button leaves the AI mark
# par-composermac-r6/rewrite-button-opens-the-three-choices | the Rewrite button opens no choices
# par-composermac-r6/rewrite-shorter-replaces-the-paragraph | the Shorter choice changes nothing
# par-composermac-r6/rewrite-warmer-replaces-the-paragraph | the Warmer choice changes nothing
# par-composermac-r6/rewrite-more-formal-replaces-the-paragraph | the More formal choice changes nothing
# par-composermac-r6/snippets-button-opens-the-list | the Snippets button opens no list
# par-composermac-r6/snippets-save-says-it-saved | Save the text as a snippet says nothing
# par-composermac-r6/snippet-1-lands-in-the-text | the second row of the snippet list inserts nothing
# par-composermac-r6/instant-reply-choice-0-fills-the-text | the first Instant Reply choice leaves the text
# par-composermac-r6/instant-reply-choice-2-fills-the-text | the third Instant Reply choice leaves the text
# par-composermac-r6/dictate-button-starts-listening | the Dictate button does nothing on a click
# par-composermac-r6/dictate-button-again-ends-the-session | the second click on Dictate leaves the session listening
# par-composermac-r6/link-cancel-closes-the-panel | the Cancel button leaves the link panel open
# par-composermac-r6/colour-swatch-default-applies | the Default swatch changes nothing
# par-composermac-r6/colour-swatch-secondary-applies | the Grey swatch changes nothing
# par-composermac-r6/colour-swatch-accent-applies | the Blue swatch changes nothing
# par-composermac-r6/colour-swatch-warning-applies | the Orange swatch changes nothing
# par-composermac-r6/inline-snippet-click-inserts-the-row | a click on a row of the semicolon list inserts nothing
# par-composermac-r6/inline-emoji-click-inserts-the-glyph | a click on a row of the colon list inserts nothing
# par-composermac-r6/remind-button-opens-the-time-list | the bell opens no time list
# par-composermac-r6/remind-later-today-sets-or-says-why | the Later today line sets nothing
# par-composermac-r6/remind-pick-date-opens-the-picker | the Pick a date line of Remind opens no picker
# par-composermac-r6/remind-set-button-sets-the-picked-time | the Set the reminder button sets nothing
# par-composermac-r6/remind-clear-removes-the-reminder | the Remove the reminder row keeps the reminder
# par-composermac-r6/send-later-button-opens-the-time-list | the Send later button opens no time list
# par-composermac-r6/send-later-laterToday-acts | the Later today line of Send later does nothing
# par-composermac-r6/send-later-tomorrowMorning-acts | the Tomorrow morning line of Send later does nothing
# par-composermac-r6/send-later-nextWeek-acts | the Next week line of Send later does nothing
# par-composermac-r6/send-later-pick-date-opens-the-picker | the Pick a date line of Send later opens no picker
# par-composermac-r6/send-later-schedule-button-acts | the Schedule button does nothing
# par-composermac-r6/send-later-saves-the-new-message-first | Send later schedules the id "draft-new" without saving the message
# par-composermac-r6/send-button-queues-one-send | the Send button queues nothing
# par-composermac-r6/undo-button-brings-the-text-back | the Undo button leaves the undo bar up
# par-composermac-r6/more-button-opens-the-menu | the chevron beside Send opens no menu
# par-composermac-r6/send-and-done-button-sends-and-archives | the Send and mark done button sends but leaves the message in the inbox
# par-composermac-r6/send-now-button-sends-without-undo | the Send now button keeps the undo window
# par-composermac-r6/every-choice-of-the-from-list-sets-its-sender | every choice of the From list picks the first account
# par-composermac-r6/attach-recent-row-attaches-the-file | a recent file in the Attach panel attaches nothing
# par-composermac-r6/discard-confirm-button-throws-the-draft-away | the Discard button of the question closes the composer but keeps the draft
# par-composermac-r7/cmd-z-in-the-body-undoes-the-typed-text | Cmd-Z through keyevent leaves the typed run in the body
# par-composermac-r7/cmd-z-leaves-the-editor-and-the-copy-equal | after the Undo the composer's copy keeps the words the editor took back (the copy written only from textDidChange)
# par-composermac-r7/edit-menu-redo-brings-the-words-back-in-the-editor-and-the-copy | after the Redo the copy stays without the words the editor brought back
# par-composermac-r7/return-in-the-body-adds-a-line-and-sends-nothing | a plain Return in the body sends the message (a Return key bound to send)
# par-composermac-r7/cmd-return-in-the-body-sends | a real ⌘Return in the body queues nothing (the "⌘Enter" binding answers without sending)
# par-composermac-r7/fixture-reset-with-the-search-field-focused-keeps-the-copy-alive | the boundary ends a field editor's event group by hand and the copy dies on the next write (main107 to main109)
# par-composermac-r7/attached-file-is-on-the-core-draft | the tile is drawn from a local list and the core never gets the file
# par-composermac-r7/tile-shows-being-added-before-the-core-answers | a tile appears only after the core has answered
# par-composermac-r7/tile-settles-to-ready-on-the-core-read | the tile stays in the being added state
# par-composermac-r7/second-file-joins-the-first | a second file replaces the first
# par-composermac-r7/x-removes-the-file-from-the-core-draft | the x on a tile does nothing
# par-composermac-r7/send-carries-the-files-to-the-outbox-entry | the send leaves the files behind
# par-composermac-r7/undo-gives-the-files-back | the undo leaves the files on the outbox entry
# par-composermac-r7/send-after-the-undo-goes-again | after an undo the composer sends with the id of the draft the first send took, and the core answers NotFound
# par-composermac-r7/second-undo-gives-the-files-back | the second undo leaves the files on the outbox entry
# par-composermac-r7/too-large-file-is-refused-with-its-sentence | a file over the cap keeps its tile and says nothing
# par-composermac-r7/files-too-large-together-are-refused-with-their-sentence | files over the total cap are all accepted
# par-composermac-r7/missing-file-is-refused-with-its-sentence | a missing file keeps its tile and says nothing
# par-composermac-r7/unusable-file-is-refused-with-its-sentence | a path the core cannot use is accepted
# par-composermac-r7/deleted-file-reads-gone-on-its-tile | the tile of a deleted file still reads ready
# par-composermac-r7/send-with-a-gone-file-is-refused-with-its-sentence | the send goes with a file that is gone
# par-composermac-r7/send-goes-once-the-gone-file-is-removed | the send stays refused after the gone file is removed
# par-composermac-r7/forward-opens-with-the-originals-files-as-tiles | the forward opens with no files
# par-composermac-r7/forward-file-removed-leaves-the-other | removing a forward file removes both
# par-composermac-r7/forward-send-carries-the-remaining-file | the forward goes without its files
# par-composermac-r7/forward-the-core-refuses-says-its-sentence | the refused forward is swallowed: the line stays empty
# par-composermac-r7/pasted-file-becomes-a-tile-not-text | ⌘V of a copied file writes its name into the text (WhooshTextView.paste without session.pasteFiles)
# par-composermac-r7/pasted-picture-goes-into-the-text | picture data pasted becomes a tile, not a picture in the text (WHOOSH_ATTACH_MUTANT=picture-as-tile)
# par-composermac-r7/pasted-picture-lives-in-the-app-data-folder | a pasted picture is written to the system's temporary folder (WHOOSH_ATTACH_MUTANT=paste-to-temp)


def main():
    path = os.path.abspath(__file__)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    print_cases(r)
    rows = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# par-composermac") and " | " in l]
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




# ---- the selftest: the round-3 checks against a model of the pop-out windows, clean and once per mutant ----
# The model answers the socket verbs the round-3 checks use, the way the Swift in Sources/Features/ComposerMac/ComposerPopOut.swift
# is written to answer them. One mutant (named by its check) is switched on at a time and that check must go red.
class PopModel:
    SENDER, OTHER = "karen@linea-example.com", "assem@northlight-example.de"

    def __init__(self, mut):
        self.m = mut
        self.surface = "mac-main"
        self.thread = False                                               # a thread is open in the main window (the overlay the shell reads)
        self.win = "composer-window-is-closed-before-the-keys" in mut      # the mutant: a window left up by an earlier discard
        self.open = False
        self.d = None                                                     # the draft: key, kind, to, cc, subject, body, files, seed_to, seed_subject, focused
        self.slab = None                                                  # the thread's reply box when it holds words: text, to
        self.line = ""
        self.ask = False
        self.sent = None                                                  # the outbox id of a message in its undo window
        self.remind_open = False
        self.rem_ms = 0
        self.core_rem = {}
        self.now = int(time.time() * 1000)                                # the core's clock: `clock advance` moves it
        self.n_sent = 0
        self.reply_at = None                                              # when a reply reached the thread (`fixture reply-arrived`)
        self.thread_rem = None                                            # a thread holds one reminder: the latest draft id
        self.sheet = False                                                # the add-account sheet
        self.addr_focus = False
        self.addr = ""

    def on(self, name): return name in self.m

    # -- the draft
    def work(self):
        d = self.d
        if self.sent and not self.on("a-message-in-its-undo-window-does-not-hold-the-window"): return False
        return bool(d["body"] or d["files"] or d["cc"] or d["to"] != d["seed_to"] or d["subject"] != d["seed_subject"])

    def present(self):
        self.open = True; self.win = True; self.surface = "mac-composer"

    def can_replace(self):
        if self.d is None or not self.work() or self.on("a-second-draft-never-replaces-the-first"): return True
        self.line = POP_LINES["finish"]; self.present(); return False

    def new_draft(self, key, kind, to, subject, body=""):
        self.d = {"key": key, "kind": kind, "to": list(to), "cc": [], "subject": subject, "body": body, "files": 0,
                  "seed_to": list(to), "seed_subject": subject, "focused": ""}
        self.line = ""
        self.sent = None; self.remind_open = False; self.rem_ms = 0

    def open_reply(self, kind="reply"):
        if self.d is None or self.d["key"] != "t1":
            if not self.can_replace(): return False
            if self.on("shift-r-opens-a-reply") and kind == "reply": self.new_draft("t1", "new", [], "")
            else: self.new_draft("t1", kind, [self.SENDER], "Re: Delivery")
        if self.on("shift-r-opens-a-composer-window") and kind == "reply" and self.d["kind"] == "reply":
            self.open = True; return True
        self.present()
        self.d["focused"] = "body"
        return True

    def slab_pop_out(self):
        sl = self.slab
        if self.on("pop-out-opens-the-window-again") or sl is None: return False
        if not self.open_reply(): return False
        d = self.d
        d["to"] = list(sl["to"])
        d["body"] = d["body"] + sl["text"] if d["body"] else sl["text"]
        if self.on("pop-out-brings-the-same-text-back"): d["body"] = ""
        if self.on("pop-out-brings-the-same-recipients-back"): d["to"] = [self.SENDER]
        self.slab = None
        return True

    def pop_reply(self):
        return self.slab_pop_out() if self.slab else self.open_reply()

    def pop_reply_all(self):
        if not self.pop_reply(): return False
        self.d["kind"] = "reply-all" if not self.on("shift-enter-opens-reply-all") else self.d["kind"]
        if self.on("reply-all-keeps-the-sender-first"): self.d["to"] = [a for a in self.d["to"] if a != self.SENDER] + ["third@example.com"]
        return True

    def pop_forward(self):
        if self.on("shift-f-opens-a-forward"): return self.open_reply()
        if self.d is None or self.d["key"] != "fwd:m1":
            if not self.can_replace(): return False
            if self.on("forward-has-no-recipient-and-a-fwd-subject"): self.new_draft("fwd:m1", "forward", [self.SENDER], "Re: Delivery")
            else: self.new_draft("fwd:m1", "forward", [], "Fwd: Delivery")
            self.d["body"] = "" if self.on("forward-carries-the-message") else "\n\n---------- Forwarded message ----------\nFrom: Karen <%s>\n\nText" % self.SENDER
            self.d["seed_subject"] = self.d["subject"]
        self.present(); self.d["focused"] = "to"
        return True

    def pop_new(self):
        if self.on("shift-c-opens-a-new-message"): return False
        if self.d is None or self.d["key"] != "new":
            if not self.can_replace(): return False
            self.new_draft("new", "new", [], "")
        self.present(); self.d["focused"] = "to"
        return True

    def pop_in(self):
        d = self.d
        if not self.open or d is None: return False
        if d["kind"] not in ("reply", "reply-all"):
            if self.on("a-new-message-stays-out-with-the-reason"):
                self.open = False; self.win = False; self.d = None; self.surface = "mac-main"; return True
            self.line = POP_LINES["only-reply"]; return True
        if d["files"]:
            if not self.on("pop-in-with-files-is-refused-with-the-reason"):
                self.line = POP_LINES["files"]; return True
            d["files"] = 0
        text = "" if self.on("pop-in-carries-the-text-to-the-thread-reply") else d["body"]
        to = [self.SENDER] if self.on("pop-in-carries-the-recipients") else list(d["to"])
        self.slab = {"text": text, "to": to}
        self.open = False
        if not self.on("pop-in-closes-the-window"): self.win = False
        self.d = None
        self.surface = "mac-reader"
        return True

    def toggle_pop(self):
        if self.open: return self.pop_in()
        return self.slab_pop_out() if self.slab else False

    def pop_search(self):
        out = self.open
        if not out and self.slab and not self.on("popout-search-pops-the-draft-out"): out = self.slab_pop_out()
        if not self.on("popout-search-focuses-the-search"): self.surface = "mac-search"
        return True

    def switch_draft(self):
        if self.on("switch-draft-key-goes-to-the-main-window") and self.surface == "mac-composer": return True      # the mutant: the key does nothing
        if not self.open: return False
        if self.surface == "mac-composer":
            self.surface = "mac-main"
            if self.on("switch-draft-key-keeps-the-draft-window-up"): self.win = False
            return True
        if self.on("switch-draft-key-comes-back-to-the-draft"): return True
        self.present(); return True

    # -- discard
    def discard_ask(self):
        if not self.open: return False
        if self.d["body"] or self.d["files"] or self.d["to"] != self.d["seed_to"]: self.ask = True; return True
        return self.discard_confirm()

    def discard_confirm(self):
        if not self.open: return False
        self.ask = False; self.d = None; self.open = False; self.win = self.on("composer-window-is-closed-before-the-keys"); self.surface = "mac-main"
        return True

    def escape(self):
        if not self.open: return False
        if self.ask: self.ask = False; return True
        self.open = False; self.win = self.on("the-composer-is-closed-before-the-sheet"); self.surface = "mac-composer" if self.on("the-composer-is-closed-before-the-sheet") else "mac-main"
        if self.on("closing-the-pop-out-keeps-the-draft"): self.d = None
        if self.on("reopening-brings-text-recipients-and-files-back") and self.d: self.d["files"] = 0
        return True

    # -- the socket
    def key(self, chord):
        if self.sheet and chord == "Escape":
            self.sheet = False; self.addr_focus = False; self.surface = "mac-main"; return True
        if chord == "⇧R":
            if self.surface == "mac-main" and not self.thread and self.slab is None: return True      # the shell: from the plain list ⇧R is the No Reply filter
            return self.pop_reply()
        if chord == "⇧Enter" and not self.ask: return self.pop_reply_all()
        if chord == "⇧F": return self.pop_forward()
        if chord == "⇧C": return self.pop_new()
        if chord == "⇧⌘P": return self.toggle_pop()
        if chord == "⌘/": return self.pop_search()
        if chord == "⌘D": return self.switch_draft()
        if chord == "⇧⌘,": return self.discard_ask()
        if chord == "Enter": return self.discard_confirm() if self.ask else False
        if chord == "Escape": return self.escape()
        if chord == "⌘Enter": return self.send()
        if chord == "⇧⌘H": return self.remind_key()
        if chord == "⇧⌘O": return self.open and self._focus("to")
        if chord == "⇧⌘M": return self.open and self._focus("body")
        return False

    def send(self):
        if not self.open or not self.d or self.sent: return False
        if self.on("send-starts-the-undo-window"): return True
        self.n_sent += 1
        self.sent = "ob%d" % self.n_sent
        if self.rem_ms and not self.on("send-gives-the-reminder-to-the-core"): self.thread_rem = self.sent
        if self.rem_ms and not self.on("send-gives-the-reminder-to-the-core"): self.core_rem[self.sent] = self.rem_ms
        return True

    def remind_key(self):
        if not self.open: return False
        if not self.on("remind-key-opens-the-time-list"): self.remind_open = not self.remind_open
        return True

    def _focus(self, f):
        self.d["focused"] = f; return True

    def type(self, text):
        if self.sheet and self.addr_focus:
            if self.d and self.on("typing-after-the-composer-closes-reaches-the-next-field"): self.d["body"] += text   # the closed composer's view eats the buffer
            else: self.addr += text
            return
        if not (self.open and self.d): return
        if self.on("popout-typed-text-in-body") and self.d["focused"] == "body": return
        if self.d["focused"] == "to":
            for a in text.split(","):
                if a.strip() and not self.on("popout-takes-a-typed-recipient"): self.d["to"].append(a.strip())
        else:
            self.d["body"] += text

    def click(self, ax):
        if ax in ("dock.whoosh", "mac-main.sidebar.accounts-menu.up"): return True
        if ax == "mac-main.sidebar.accounts-menu.add": self.sheet = True; self.surface = "mac-add-account"; return True
        if ax == "mac-add-account.address": self.addr_focus = self.sheet; return self.sheet
        if ax == "mac-main.list.row.0": return True
        if ax == "mac-main.reader.open":
            if not self.on("thread-is-open-before-shift-r"): self.thread = True
            return True
        if ax == "mac-composer.dev.sample-file":
            if not self.open: self.new_draft("new", "new", [], ""); self.present()
            self.d["files"] += 1; return True
        if ax in ("mac-composer.remind.tomorrowMorning", "mac-composer.remind.laterToday"):
            if not (self.open and self.remind_open): return False
            self.remind_open = False
            if not self.on("remind-pick-sets-the-reminder"):
                self.rem_ms = self.now + (86_400_000 if ax.endswith("tomorrowMorning") else 3 * 3_600_000)   # tomorrow morning: within two days of the clock; later today: 3 h
                if self.sent: self.core_rem[self.sent] = self.rem_ms; self.thread_rem = self.sent
            return True
        if ax == "mac-composer.remind.nextWeek":
            if not (self.open and self.remind_open): return False
            self.remind_open = False
            self.rem_ms = self.now + 6 * 86_400_000
            if self.sent: self.core_rem[self.sent] = self.rem_ms; self.thread_rem = self.sent
            return True
        if ax == "mac-composer.discard": return self.discard_ask()
        if ax == "mac-composer.discard.confirm": return self.discard_confirm()
        if ax == "mac-composer.pop-in": return False if self.on("pop-in-button-puts-the-reply-back") else self.pop_in()
        if ax == "mac-composer.thread": return True if self.on("thread-button-goes-to-the-thread") else self.switch_draft_force()
        return False

    def switch_draft_force(self):
        if not self.open: return False
        self.surface = "mac-main"; return True

    def tooltip(self, title, key, mut):
        return title if self.on(mut) else title + "  " + key

    def rows(self, ax):
        d = self.d if self.open else None
        if ax == "mac-main.list":
            # the inbox row of the thread: `reminder` is a bool, True once its reminder fired
            fired_now = self.thread_rem in self.core_rem and self.reminder_state(self.thread_rem) == "fired"
            return [{"ax": "mac-main.list.row.0", "id": "msg-01", "reminder": bool(fired_now and not self.on("fired-reminder-marks-the-inbox-row"))}]
        if ax == POP:
            sl = self.slab or {"text": "", "to": []}
            return [{"ax": POP, "open": self.open, "window": self.win, "kind": d["kind"] if d else "new", "title": {"new": "New message", "reply": "Reply", "reply-all": "Reply all", "forward": "Forward"}[d["kind"]] if d else "New message",
                     "to": d["to"] if d else [], "cc": [], "subject": d["subject"] if d else "", "files": d["files"] if d else 0, "line": self.line,
                     "can_pop_in": True, "slab_open": self.slab is not None, "slab_text": sl["text"], "slab_to": sl["to"], "slab_cc": [], "slab_bcc": [], "slab_subject": "",
                     "motion": "scale+opacity" if self.on("a-window-opens-with-opacity-and-a-short-rise") else "rise+opacity",
                     "rise_pt": 6.0, "ms": 160}]
        if ax == "mac-add-account.address": return [{"ax": ax, "focused": self.addr_focus, "text": self.addr, "visible": True}] if self.sheet else []
        if ax == "mac-composer.body": return [{"ax": ax, "text": d["body"]}] if d else []
        if ax == "mac-composer.header":
            return [{"ax": ax, "to": d["to"], "cc": d["cc"], "subject": d["subject"], "focused": d["focused"], "discard_ask": self.ask,
                     "remind_open": self.remind_open, "reminder_ms": self.rem_ms}] if d else []
        if ax == "mac-main.snooze.options":
            # the time words on the model's clock: Later today +3 h to the half hour, Tomorrow morning 08:00, Next week the coming Monday 08:00 and never
            # today. The mutant keeps the same day on a Monday before 08:00.
            import datetime as _dt
            now = _dt.datetime.fromtimestamp(self.now / 1000).astimezone()
            later = now + _dt.timedelta(hours=3)
            later = later.replace(minute=30 * (later.minute // 30), second=0, microsecond=0)
            at8 = now.replace(hour=8, minute=0, second=0, microsecond=0)
            ahead = (7 - now.weekday()) % 7
            if ahead == 0 and not (self.on("next-week-on-a-monday-morning-is-the-monday-after") and now < at8): ahead = 7
            iso = lambda t: t.astimezone(_dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
            return [{"ax": "mac-main.snooze.options.%d" % i, "id": k, "resolves_to": iso(t) if t else None}
                    for i, (k, t) in enumerate((("laterToday", later), ("tomorrowMorning", at8 + _dt.timedelta(days=1)), ("nextWeek", at8 + _dt.timedelta(days=ahead)), ("pickDate", None)))]
        if ax == "mac-composer.remind.options":
            return [{"ax": "mac-composer.remind." + k} for k in ("laterToday", "tomorrowMorning", "nextWeek", "pickDate")] if self.remind_open else []
        if ax == "mac-composer.undo.bar":
            return [{"ax": ax, "undo_offered": True, "outbox_id": self.sent}] if self.sent and d else []
        if ax == "mac-composer.draft":
            dd = self.d
            return [{"ax": ax, "kept": dd is not None, "body_chars": len(dd["body"]) if dd else 0, "subject_chars": len(dd["subject"]) if dd else 0}]
        if ax == "mac-composer.controls" and d:
            out = []
            if d["kind"] in ("reply", "reply-all"): out.append({"ax": "mac-composer.pop-in", "tooltip": self.tooltip("Pop in", "⇧⌘P", "thread-and-pop-in-controls-show-their-keys")})
            if d["kind"] != "new": out.append({"ax": "mac-composer.thread", "tooltip": self.tooltip("Thread", "⌘D", "none")})
            return out
        return []

    def reminder_state(self, rid):
        """The contract's reading (reply_reminder_state): a reply at or before the time answers it, else the clock decides."""
        at = self.core_rem[rid]
        if self.reply_at is not None and self.reply_at <= at and not self.on("reminder-stays-quiet-when-a-reply-arrived"): return "answered"
        if self.now >= at and not self.on("reminder-fires-when-no-reply-arrives"): return "fired"
        return "waiting"

    def handle(self, verb, args):
        if verb == "key": return {"ok": True} if self.key(args[0]) else {"ok": False, "error": "key does nothing"}
        if verb == "type": self.type(args[0]); return {"ok": True}
        if verb == "click": return {"ok": True} if self.click(args[0]) else {"ok": False, "error": "did nothing"}
        if verb == "rows": return {"ok": True, "rows": self.rows(args[0])}
        if verb == "core":
            parts = args[1].split("/")
            if parts[0] == "reply_reminder" and parts[2] == "at_ms": return {"ok": True, "value": self.core_rem.get(parts[1])}
            if parts[0] == "reply_reminder" and parts[2] == "fired":
                return {"ok": True, "value": self.reminder_state(parts[1]) == "fired" if parts[1] in self.core_rem else None}
            if parts[0] == "thread" and parts[2] == "reminder":
                return {"ok": True, "value": json.dumps({"state": self.reminder_state(self.thread_rem) if self.thread_rem in self.core_rem else "none",
                                                          "draft_id": self.thread_rem if self.thread_rem in self.core_rem else None})}
            if parts[0] == "mailbox" and parts[1] == "reminders" and parts[2] == "rows":
                if self.on("fired-reminder-is-a-row-of-the-reminders-view"): return {"ok": True, "value": 0}      # mutant: the view lists nothing
                return {"ok": True, "value": sum(1 for i in self.core_rem if i == self.thread_rem and self.reminder_state(i) == "fired")}
            return {"ok": True, "value": None}
        if verb == "clock":
            if args[0] != "advance": return {"ok": False, "error": "usage"}
            self.now += int(args[1]) * 1000
            return {"ok": True}
        if verb == "fixture" and args[0] == "reply-arrived":
            if self.reply_at is None: self.reply_at = self.now
            return {"ok": True}
        if verb == "uistate": return {"ok": True, "state": {"surface": self.surface, "overlay": "mac-thread" if self.thread else None, "focus_message": "msg-01"}}
        if verb == "winframes":
            return {"ok": True, "frames": [{"title": "Whoosh", "kind": "window"}] + ([{"title": "New message", "surface": "mac-composer", "kind": "window"}] if self.win else [])}
        return {"ok": False, "error": "nothing"}


def model_run(mut):
    """Drives the round-3 checks over a socket served by PopModel with the mutants `mut` on; returns {check label: state}."""
    import json as _json
    import shutil
    import socketserver
    import tempfile
    import threading
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl                                              # noqa: E402
    from lib.harness import Journey                                      # noqa: E402
    model = PopModel(mut)
    d = tempfile.mkdtemp(prefix="par-cm3-")

    class H(socketserver.StreamRequestHandler):
        def handle(self):
            for line in self.rfile:
                q = _json.loads(line.decode())
                self.wfile.write((_json.dumps(model.handle(q["verb"], q["args"])) + "\n").encode())
    srv = socketserver.ThreadingUnixStreamServer(os.path.join(d, "whoosh.sock"), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    SPEED["settle"] = 0.0
    ctl = None
    try:
        ctl = Ctl(d).connect()
        j = Journey("par-composermac-r3", ctl)
        drive_r3(j)
        drive_r4(j)
        drive_r5(j)
        return {c.label: (c.state, c.detail) for c in j.checks()}
    finally:
        if ctl is not None:
            ctl.close()      # on every path: an open connection keeps its handler thread, and server_close() joins it for ever
        srv.shutdown(); srv.server_close()
        shutil.rmtree(d, ignore_errors=True)


# ---- the selftest of round 6: a model of the composer's controls, clean and once per mutant ----
# DoorsModel answers the socket verbs drive_r6 uses the way the Swift in Sources/Features/ComposerMac (ComposerMacFeature.clickTargets and
# rowSources, ComposerModel+Doors.swift) is written to answer them. One mutant (named by its check) is switched on at a time.
class DoorsModel:
    SENDER = "karen@linea-example.com"
    ADDR = ["amy@one.example", "ben@two.example", "cat@three.example"]
    SNIPS = [("Thanks, received", "Thanks, received. I will come back to you."), ("Meeting", "Can we meet on Thursday?")]
    AI = ("Hi Karen,\n\nThank you for the quote. The price works for us, and we would like delivery confirmed for March. "
          "Please also confirm the signed quote reaches our accounts team before the end of the week.\n\nAssem")
    RW = {"shorter": "The price works for us. Please confirm delivery in March.", "warmer": "Thank you so much for the quote. The price works well for us.",
          "more-formal": "We confirm that the proposed price is acceptable."}
    INSTANT = ["Yes, that works for me.", "I need a little more time.", "I cannot do this, sorry."]
    FIXTURE_REFUSAL = "the fixture core does not serve this command"

    def __init__(self, mut):
        self.m = mut
        self.surface = "mac-main"
        self.thread = False
        self.open = False
        self.d = None
        self.snips = list(self.SNIPS)
        self.recent = []
        self.writer = False
        self.n_entries = 0
        self.n_drafts = 0
        self.archived = set()
        self.bar = None                                              # the undo bar: offered, text, outbox id
        self.sent_kind = None
        self.pending_close = False                                   # an instant send: the composer closes after a beat
        self.now = int(time.time() * 1000)

    def on(self, name): return name in self.m

    def new_draft(self):
        vis = self.on("a-fresh-reply-starts-with-cc-bcc-hidden-and-quoted-folded")
        self.d = {"to": [self.SENDER], "cc": [], "bcc": [], "showcc": vis, "showbcc": vis, "subject": "Re: Delivery", "body": "", "folded": True,
                  "focused": "body", "panel": "", "line": "", "ai_input": "", "ai_text": None, "color": "", "rem": 0, "picking": False, "refusal": "",
                  "files": [], "toin": "", "ccin": "", "bccin": "", "frm": 0, "inline": None, "asked": False, "dict": "idle"}
        self.bar = None

    def finish(self):
        """The beat after an instant send: the window closes."""
        if self.pending_close:
            self.pending_close = False
            self.open = False; self.d = None; self.bar = None; self.surface = "mac-main"

    def can_send(self):
        d = self.d
        return bool(d["body"].strip()) and bool(d["to"] or d["toin"])

    def close_panels(self):
        d = self.d
        d["panel"] = ""; d["picking"] = False; d["refusal"] = ""; d["asked"] = False

    def focus(self, f):
        d = self.d
        self.close_panels()
        d["focused"] = f
        if f == "cc": d["showcc"] = True
        if f == "bcc": d["showbcc"] = True

    def inline_items(self):
        d = self.d
        if d["inline"] == "snippet": return [("snippet", n, "", t) for n, t in self.snips]
        if d["inline"] == "emoji": return [("emoji", ":tada", "\U0001F389", "")]
        return []

    def paragraphs_replace(self, text):
        ps = self.d["body"].split("\n\n")
        i = max(range(len(ps)), key=lambda k: len(ps[k]))
        ps[i] = text
        self.d["body"] = "\n\n".join(ps)

    # -- keys and typing
    def key(self, chord):
        self.finish()
        if chord == "⇧R":
            if self.open: return True
            if not self.thread: return True                          # the plain list: the No Reply filter
            self.new_draft() if self.d is None else None
            self.open = True; self.surface = "mac-composer"
            return True
        d = self.d if self.open else None
        if d is None: return False
        if chord == "⇧⌘,":
            if d["body"] or d["files"]: d["asked"] = True; d["panel"] = "discard"; return True
            return self.confirm_discard()
        if chord == "Enter": return self.confirm_discard() if d["asked"] else False
        if chord == "Escape":
            if d["inline"]: d["inline"] = None; return True
            if d["panel"]: self.close_panels(); return True
            self.open = False; self.surface = "mac-main"; return True
        if chord == "⇧⌘O": self.focus("to"); return True
        if chord == "⇧⌘S": self.focus("subject"); return True
        if chord == "⇧⌘M": self.focus("body"); return True
        if chord == "⌘K": self.close_panels(); d["panel"] = "link"; return True
        if chord == "⌘O": self.close_panels(); d["panel"] = "color"; return True
        if chord == ";":
            d["body"] += ";"; d["inline"] = "snippet"; return True
        return False

    def type(self, text):
        self.finish()
        d = self.d if self.open else None
        if d is None: return
        if d["panel"] == "ai": d["ai_input"] += text; return
        f = d["focused"]
        if f == "subject" and self.on("subject-takes-the-typed-text"): f = "body"
        if f == "to": d["toin"] += text
        elif f == "cc":
            if not self.on("cc-takes-the-typed-address"): d["ccin"] += text
        elif f == "bcc":
            if not self.on("bcc-takes-the-typed-address"): d["bccin"] += text
        elif f == "subject": d["subject"] += text
        else:
            d["body"] += text
            d["ai_text"] = d["ai_text"] if d["body"] == d["ai_text"] else None
            if d["body"].endswith(":tad"): d["inline"] = "emoji"

    def confirm_discard(self):
        d = self.d
        if self.on("discard-confirm-button-throws-the-draft-away"):
            d["asked"] = False; d["panel"] = ""
        else:
            self.d = None
        self.open = False; self.surface = "mac-main"
        return True

    # -- clicks
    def click(self, ax):
        self.finish()
        if ax == "mac-main.list.row.0": return True
        if ax == "mac-main.reader.open": self.thread = True; return True
        d = self.d if self.open else None
        if d is None: return False
        import re
        m = re.fullmatch(r"mac-composer\.(to|cc|bcc)\.chip\.(\d+)\.remove", ax)
        if m:
            f, i = m.group(1), int(m.group(2))
            if self.on("%s-chip-x-takes-the-recipient-away" % f) or i >= len(d[f]): return False
            d[f].pop(i); return True
        if ax == "mac-composer.to.completions.0":
            if self.on("completion-click-adds-the-recipient") or not self.completions(): return False
            d["to"].append(self.completions()[0]); d["toin"] = ""; return True
        m = re.fullmatch(r"mac-composer\.(cc|bcc)\.show", ax)
        if m:
            f = m.group(1)
            if self.on("%s-button-shows-the-row-and-takes-the-cursor" % f): return False
            self.focus(f); return True
        m = re.fullmatch(r"mac-composer\.(cc|bcc)\.submit", ax)
        if m:
            f = m.group(1)
            if self.on("%s-submit-makes-a-chip" % f) or "@" not in d[f + "in"]: return False
            d[f].append(d[f + "in"]); d[f + "in"] = ""; return True
        if ax == "mac-composer.quoted.toggle":
            if self.on("quoted-button-unfolds-the-quoted-text") or (self.on("quoted-button-folds-it-again") and not d["folded"]): return False
            d["folded"] = not d["folded"]; return True
        if ax == "mac-composer.dictate":
            if d["dict"] == "idle":
                if self.on("dictate-button-starts-listening"): return False
                d["dict"] = "listening"; return True
            if d["dict"] == "listening":
                if self.on("dictate-button-again-ends-the-session"): return False
                d["dict"] = "idle"; return True
            return False
        if ax == "mac-composer.dev.fixture-writer": self.writer = True; return True
        if ax == "mac-composer.dev.sample-file":
            if "whoosh-sample-quote.txt" not in d["files"]: d["files"].append("whoosh-sample-quote.txt")
            if "whoosh-sample-quote.txt" not in self.recent: self.recent.append("whoosh-sample-quote.txt")
            return True
        if ax == "mac-composer.attachment-files.0.menu.remove":
            if not d["files"]: return False
            d["files"].pop(0); return True
        # panels opened by a button: one at a time
        toggles = {"mac-composer.ai-draft": ("ai", "ai-draft-button-opens-the-field"), "mac-composer.rewrite": ("rewrite", "rewrite-button-opens-the-three-choices"),
                   "mac-composer.snippets": ("snippets", "snippets-button-opens-the-list"), "mac-composer.instant-reply": ("instant", None),
                   "mac-composer.remind": ("remind", "remind-button-opens-the-time-list"), "mac-composer.send-later": ("send-later", "send-later-button-opens-the-time-list"),
                   "mac-composer.send-more": ("more", "more-button-opens-the-menu"), "mac-composer.attach": ("attach", None), "mac-composer.from": ("from", None)}
        if ax in toggles:
            word, mut = toggles[ax]
            if mut and self.on(mut): return False
            if word == "rewrite" and not d["body"]: return False
            was = d["panel"] == word
            self.close_panels()
            if not was: d["panel"] = word
            return True
        if ax == "mac-composer.ai-draft.run":
            if d["panel"] != "ai" or not d["ai_input"].strip() or self.on("ai-run-button-writes-the-draft"): return False
            d["body"] = self.AI; d["ai_text"] = self.AI; d["panel"] = ""; return True
        if ax == "mac-composer.ai-accept":
            if self.on("ai-accept-button-clears-the-mark") or d["ai_text"] is None: return False
            d["ai_text"] = None; return True
        m = re.fullmatch(r"mac-composer\.rewrite\.(shorter|warmer|more-formal)", ax)
        if m:
            slug = m.group(1)
            if d["panel"] != "rewrite" or self.on("rewrite-%s-replaces-the-paragraph" % slug): return False
            self.paragraphs_replace(self.RW[slug]); d["panel"] = ""; d["ai_text"] = None; return True
        if ax == "mac-composer.snippets.save":
            if self.on("snippets-save-says-it-saved") or not d["body"]: return False
            self.snips.append(("Saved", d["body"])); d["line"] = "Saved as a snippet"; return True
        m = re.fullmatch(r"mac-composer\.snippets\.(\d+)", ax)
        if m:
            i = int(m.group(1))
            if d["panel"] != "snippets" or i >= len(self.snips) or (i == 1 and self.on("snippet-1-lands-in-the-text")): return False
            d["body"] += self.snips[i][1]; d["panel"] = ""; return True
        m = re.fullmatch(r"mac-composer\.instant-reply\.(\d+)", ax)
        if m:
            i = int(m.group(1))
            if d["panel"] != "instant" or i >= 3 or self.on("instant-reply-choice-%d-fills-the-text" % i): return False
            d["body"] = self.INSTANT[i]; d["panel"] = ""; d["ai_text"] = None; return True
        if ax == "mac-composer.link.cancel":
            if d["panel"] != "link" or self.on("link-cancel-closes-the-panel"): return False
            d["panel"] = ""; return True
        m = re.fullmatch(r"mac-composer\.color\.(default|secondary|accent|warning)", ax)
        if m:
            slug = m.group(1)
            if d["panel"] != "color" or self.on("colour-swatch-%s-applies" % slug): return False
            d["color"] = "" if slug == "default" else slug; d["panel"] = ""; return True
        m = re.fullmatch(r"mac-composer\.inline\.(\d+)", ax)
        if m:
            items = self.inline_items()
            if not items: return False
            kind, _, glyph, text = items[0]
            if self.on("inline-%s-click-inserts-the-%s" % (("snippet", "row") if kind == "snippet" else ("emoji", "glyph"))): return False
            if kind == "snippet": d["body"] = d["body"][:-1] + text
            else: d["body"] = d["body"][:-len(":tad")] + glyph
            d["inline"] = None; return True
        # remind
        m = re.fullmatch(r"mac-composer\.remind\.(laterToday|tomorrowMorning|nextWeek|pickDate|set|clear)", ax)
        if m:
            k = m.group(1)
            if k == "clear":
                if self.on("remind-clear-removes-the-reminder") or not d["rem"]: return False
                d["rem"] = 0; return True
            if d["panel"] != "remind": return False
            if k == "pickDate":
                if self.on("remind-pick-date-opens-the-picker"): return False
                d["picking"] = not d["picking"]; return True
            if k == "set":
                if not d["picking"] or self.on("remind-set-button-sets-the-picked-time"): return False
                d["rem"] = self.now + 86_400_000; d["panel"] = ""; d["picking"] = False; return True
            if k == "laterToday" and self.on("remind-later-today-sets-or-says-why"): return False
            d["rem"] = self.now + (3 * 3_600_000 if k == "laterToday" else 86_400_000 if k == "tomorrowMorning" else 6 * 86_400_000); d["panel"] = ""; return True
        # send later
        m = re.fullmatch(r"mac-composer\.send-later\.(laterToday|tomorrowMorning|nextWeek|pickDate|schedule)", ax)
        if m:
            k = m.group(1)
            if d["panel"] != "send-later": return False
            if k == "pickDate":
                if self.on("send-later-pick-date-opens-the-picker"): return False
                d["picking"] = not d["picking"]; return True
            if k == "schedule":
                if not d["picking"] or self.on("send-later-schedule-button-acts"): return False
            elif self.on("send-later-%s-acts" % k): return False
            if not self.can_send(): d["line"] = "Add a recipient and some text first."; return True
            if not self.on("send-later-saves-the-new-message-first"): self.n_drafts += 1       # saveDraft before schedule
            d["panel"] = ""; d["picking"] = False; d["line"] = self.FIXTURE_REFUSAL; return True
        # send
        if ax == "mac-composer.send": return self.send(False, False)
        if ax == "mac-composer.send-done": return self.send(True, False)
        if ax == "mac-composer.send-instant": return self.send(False, True)
        if ax == "mac-composer.undo":
            if not self.bar or self.on("undo-button-brings-the-text-back"): return False
            self.bar = None
            for mid in list(self.archived): self.archived.discard(mid)
            return True
        # From list and Attach panel
        m = re.fullmatch(r"mac-composer\.from\.(\d+)", ax)
        if m:
            i = int(m.group(1))
            if d["panel"] != "from" or i >= len(self.ADDR): return False
            d["frm"] = 0 if self.on("every-choice-of-the-from-list-sets-its-sender") else i; d["panel"] = ""; return True
        if ax == "mac-composer.attach.recent.0":
            if d["panel"] != "attach" or not self.recent or self.on("attach-recent-row-attaches-the-file"): return False
            if self.recent[0] not in d["files"]: d["files"].append(self.recent[0])
            d["panel"] = ""; return True
        if ax == "mac-composer.discard":
            if d["body"] or d["files"]: d["asked"] = True; d["panel"] = "discard"; return True
            return self.confirm_discard()
        if ax == "mac-composer.discard.confirm": return self.confirm_discard() if d["asked"] else False
        return False

    def completions(self):
        d = self.d
        t = d["toin"].strip().lower()
        if self.on("typed-letters-list-the-person-answered") or not t or self.SENDER in d["to"]: return []
        return [self.SENDER] if t in self.SENDER else []

    def send(self, done, instant):
        d = self.d
        if self.bar or not self.can_send(): return False
        if self.on("send-button-queues-one-send") and not done and not instant: return False
        self.n_entries += 1
        self.close_panels()
        if instant and self.on("send-now-button-sends-without-undo"): instant = False
        self.bar = {"offered": not instant, "text": "Sent" if instant else "Sending", "id": "ob%d" % self.n_entries}
        if done and not self.on("send-and-done-button-sends-and-archives"): self.archived.add("msg-01")
        if instant: self.pending_close = True
        return True

    # -- reads
    def rows(self, ax):
        d = self.d if self.open else None
        if ax == "mac-main.list": return [{"id": i} for i in ("msg-01", "msg-02", "msg-03") if i not in self.archived]
        if ax == "mac-composer.undo.bar":
            return [{"ax": ax, "undo_offered": self.bar["offered"], "text": self.bar["text"], "outbox_id": self.bar["id"]}] if self.bar else []
        if ax == "mac-composer.draft":
            return [{"ax": ax, "kept": self.d is not None, "body_chars": len(self.d["body"]) if self.d else 0, "subject_chars": len(self.d["subject"]) if self.d else 0}]
        if d is None: return []
        if ax == HEAD:
            return [{"ax": ax, "reply_message_id": "msg-01", "to": d["to"], "cc": d["cc"], "bcc": d["bcc"], "cc_visible": d["showcc"], "bcc_visible": d["showbcc"], "quoted_folded": d["folded"],
                     "focused": d["focused"], "panel": d["panel"], "line": d["line"], "accounts": len(self.ADDR), "from": self.ADDR[d["frm"]], "subject": d["subject"],
                     "to_input": d["toin"], "cc_input": d["ccin"], "bcc_input": d["bccin"], "date_picking": d["picking"], "date_refusal": d["refusal"],
                     "reminder_ms": d["rem"], "reminder_words": "Reminder set" if d["rem"] else "", "link_open": d["panel"] == "link", "color_open": d["panel"] == "color"}]
        if ax == BODY: return [{"ax": ax, "text": d["body"], "editor_text": d.get("editor", d["body"])}]
        if ax == "mac-composer.format": return [{"ax": ax, "color": d["color"]}]
        if ax == "mac-composer.to.completions": return [{"ax": "mac-composer.to.completions.%d" % i, "address": a} for i, a in enumerate(self.completions())]
        if ax == "mac-composer.ai-marks": return [{"ax": ax, "words": d["ai_text"]}] if d["ai_text"] is not None and d["body"] == d["ai_text"] else []
        if ax == "mac-composer.rewrite.choices": return [{"ax": "mac-composer.rewrite." + s, "title": s} for s in ("shorter", "warmer", "more-formal")] if d["panel"] == "rewrite" else []
        if ax == "mac-composer.snippets": return [{"ax": "mac-composer.snippets.%d" % i, "title": n, "text": t} for i, (n, t) in enumerate(self.snips)] if d["panel"] == "snippets" else []
        if ax == "mac-composer.instant-reply.options": return [{"ax": "mac-composer.instant-reply.%d" % i, "text": t} for i, t in enumerate(self.INSTANT)] if d["panel"] == "instant" else []
        if ax == "mac-composer.inline": return [{"ax": "mac-composer.inline.%d" % i, "kind": k, "title": n, "glyph": g, "text": t} for i, (k, n, g, t) in enumerate(self.inline_items())]
        if ax == "mac-composer.remind.options":
            return [{"ax": "mac-composer.remind." + k, "title": k, "refused": False} for k in ("laterToday", "tomorrowMorning", "nextWeek", "pickDate")] if d["panel"] == "remind" else []
        if ax == "mac-composer.send-later.options":
            return [{"ax": "mac-composer.send-later." + k, "title": k, "refused": False} for k in ("laterToday", "tomorrowMorning", "nextWeek", "pickDate")] if d["panel"] == "send-later" else []
        if ax == "mac-composer.attachments": return [{"ax": ax, "name": f} for f in d["files"]]
        if ax == "mac-composer.dictation": return [{"state": d["dict"], "live": "", "line": "", "audio_files": 0}]
        return []

    def handle(self, verb, args):
        if verb == "key": return {"ok": True} if self.key(args[0]) else {"ok": False, "error": "key does nothing"}
        if verb == "type": self.type(args[0]); return {"ok": True}
        if verb == "click": return {"ok": True} if self.click(args[0]) else {"ok": False, "error": "did nothing"}
        if verb == "rows": return {"ok": True, "rows": self.rows(args[0])}
        if verb == "core": return {"ok": True, "value": self.n_entries if args[1] == "outbox_entry/count" else self.n_drafts if args[1] == "draft/count" else None}
        if verb == "uistate": return {"ok": True, "state": {"surface": self.surface, "overlay": "mac-thread" if self.thread else None, "focus_message": "msg-01"}}
        return {"ok": False, "error": "nothing"}


def serve_model(model, name):
    """A Unix socket answered by `model.handle`; returns (ctl, close)."""
    import json as _json
    import shutil
    import socketserver
    import tempfile
    import threading
    from lib.ctl import Ctl                                              # noqa: E402
    d = tempfile.mkdtemp(prefix=name)

    class H(socketserver.StreamRequestHandler):
        def handle(self):
            for line in self.rfile:
                q = _json.loads(line.decode())
                self.wfile.write((_json.dumps(model.handle(q["verb"], q["args"])) + "\n").encode())
    srv = socketserver.ThreadingUnixStreamServer(os.path.join(d, "whoosh.sock"), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    ctl = Ctl(d).connect()

    def close():
        ctl.close(); srv.shutdown(); srv.server_close(); shutil.rmtree(d, ignore_errors=True)
    return ctl, close


# ---- the selftest of round 7: a model of the composer's files, clean and once per mutant ----
# FilesModel answers the socket verbs drive_r7 uses the way ComposerModel+Files.swift (AddDraftAttachment, RemoveDraftAttachment, the read
# `draft/<id>/attachments`, the fixture core's refusals and its send moving the files to the outbox entry) is written to answer them.
class FilesModel:
    SENDER = "karen@linea-example.com"
    MIB = 1048576
    SAY = {"too_large": "This file is too large to send.", "total": "These files are too large to send together.",
           "gone": "This file is no longer on this Mac.", "bad": "This file cannot be attached."}
    FWD = {"msg-02": [("brand-book-v3.pdf", 40000), ("wordmark.svg", 3000)]}
    FWD_REFUSED = {"msg-03": "This message could not be forwarded."}      # the forward of this message is refused by the core; the app says the sentence

    def __init__(self, mut):
        self.m = mut
        self.thread, self.open, self.d = False, False, None
        self.surface = "mac-main"
        self.mid_n, self.sample_exists, self.n = 0, False, 0
        self.out, self.bar, self.parked = {}, None, []
        self.stale = False
        self.undos = 0
        self.nrow = {}

    def on(self, name): return name in self.m

    def new_draft(self, kind, to, body, files=()):
        self.d = {"kind": kind, "to": list(to), "body": body, "core": [{"id": "att%d" % i, "name": n, "size": z, "state": "will_download"} for i, (n, z) in enumerate(files)] if not self.on("forward-opens-with-the-originals-files-as-tiles") else [],
                  "adding": [], "id": "draft-1" if files and not self.on("forward-opens-with-the-originals-files-as-tiles") else "", "line": "", "held": False, "focus": "body", "asked": False}
        self.bar = None

    # -- files
    def add(self, name, size, exists, relative=False):
        d = self.d
        d["adding"].append({"name": name, "size": size, "exists": exists, "relative": relative})
        if not d["held"]: self.settle()

    def settle(self):
        d = self.d
        if self.on("tile-settles-to-ready-on-the-core-read") and d["adding"] and not d["held"]: return
        adds, d["adding"] = d["adding"], []
        for a in adds:
            code = None
            if a["relative"]: code = "bad"
            elif not a["exists"]: code = "gone"
            elif a["size"] > 10 * self.MIB: code = "too_large"
            elif sum(f["size"] for f in d["core"]) + a["size"] > 14 * self.MIB: code = "total"
            mut = {"bad": "unusable-file-is-refused-with-its-sentence", "gone": "missing-file-is-refused-with-its-sentence",
                   "too_large": "too-large-file-is-refused-with-its-sentence", "total": "files-too-large-together-are-refused-with-their-sentence"}.get(code)
            if code and not self.on(mut):
                d["line"] = self.SAY[code]; continue
            if self.on("attached-file-is-on-the-core-draft"): d["local"] = d.get("local", []) + [a["name"]]; continue
            if any(f["name"] == a["name"] for f in d["core"]) and self.on("second-file-joins-the-first"): continue
            if not d["id"]: d["id"] = "draft-1"
            if self.on("second-file-joins-the-first") and d["core"]: d["core"] = []
            d["core"].append({"id": "att%d" % self.n, "name": a["name"], "size": a["size"], "state": "ready" if a["exists"] else "gone"}); self.n += 1

    def tiles(self):
        d = self.d
        if self.on("attached-file-is-on-the-core-draft"): return [{"name": n, "state": "ready", "id": n, "size": 0} for n in d.get("local", [])]
        core = []
        for f in d["core"]:
            st = f["state"]
            if f["name"] == "whoosh-sample-quote.txt" and not self.sample_exists and st == "ready" and not self.on("deleted-file-reads-gone-on-its-tile"): st = "gone"
            core.append(dict(f, state=st))
        ad = [] if self.on("tile-shows-being-added-before-the-core-answers") else [{"id": "adding:" + a["name"], "name": a["name"], "size": a["size"], "state": "adding"} for a in d["adding"]]
        return core + ad

    # -- keys, typing, clicks
    def key(self, chord):
        if chord == "/": self.search_focused = True; return True       # the search field takes the keys
        if chord == "⇧R":
            if self.open: return True
            if not self.thread: return True
            if self.d is None: self.new_draft("reply", [self.SENDER], "")
            self.open = True; self.surface = "mac-composer"; return True
        if chord == "⇧F":
            if self.open: return True
            self.new_draft("forward", [], "Forwarded text", self.FWD.get(self.nrow.get("focus", ""), []))
            if self.nrow.get("focus", "") in self.FWD_REFUSED and not self.on("forward-the-core-refuses-says-its-sentence"): self.d["line"] = self.FWD_REFUSED[self.nrow["focus"]]
            self.open = True; self.surface = "mac-composer"; self.d["focus"] = "to"; return True
        d = self.d if self.open else None
        if d is None: return False
        if chord == "⇧⌘,":
            if d["body"]: d["asked"] = True; return True
            return self.discard()
        if chord == "Enter": return self.discard() if d["asked"] else False
        if chord == "⇧⌘O": d["focus"] = "to"; return True
        return False

    def discard(self):
        self.d = None; self.open = False; self.surface = "mac-main"; self.bar = None; return True

    def type(self, text):
        d = self.d if self.open else None
        if d is None: return
        if d["focus"] == "to":
            for a in text.split(","):
                if a.strip(): d["to"].append(a.strip())
        else: d["hist"] = []; d["body"] += text          # as the app: text from outside drops the view's typing steps (RichStorageSwap)

    def click(self, ax):
        if ax == "mac-main.reader.open": self.thread = True; return True
        if ax.startswith("mac-main.list.row."): self.nrow["focus"] = ("msg-01", "msg-02", "msg-03")[int(ax.rsplit(".", 1)[1])]; return True
        d = self.d if self.open else None
        if d is None: return False
        if ax == "mac-composer.dev.sample-file": self.sample_exists = True; self.add("whoosh-sample-quote.txt", 12, True); return True
        if ax == "mac-composer.dev.mid-file": self.mid_n += 1; self.add("whoosh-mid-file-%d.bin" % self.mid_n, 8 * self.MIB, True); return True
        if ax == "mac-composer.dev.big-file": self.add("whoosh-big-file.bin", 11 * self.MIB, True); return True
        if ax == "mac-composer.dev.gone-file": self.add("whoosh-gone-file.txt", 0, False); return True
        if ax == "mac-composer.dev.bad-file": self.add("relative-file.txt", 0, True, relative=True); return True
        if ax == "mac-composer.dev.delete-sample": self.sample_exists = False; return True
        if ax == "mac-main.attachments.dev.stage-file": self.staged = True; return True
        if ax == "mac-main.attachments.dev.stage-image": self.staged = "image"; return True
        if ax == "mac-composer.dev.redo":               # the Edit menu's Redo: the editor gets the words back; the copy follows (the mutant: it does not)
            d = self.d if self.open else None
            if d is None or not d.get("redo"): return False
            words = d["redo"].pop()
            if self.on("edit-menu-redo-brings-the-words-back-in-the-editor-and-the-copy"): d["editor"] = words
            else: d["body"] = words; d.pop("editor", None)
            return True
        if ax == "mac-composer.dev.hold-files":
            d["held"] = not d["held"]
            if not d["held"]: self.settle()
            return True
        if ax.startswith("mac-composer.attachment-files.") and ax.endswith(".menu.remove"):
            i = int(ax.split(".")[2]); t = self.tiles()
            if i >= len(t) or self.on("x-removes-the-file-from-the-core-draft"): return False
            gone = t[i]["id"]
            if self.on("forward-file-removed-leaves-the-other") and d["kind"] == "forward": d["core"] = []
            else: d["core"] = [f for f in d["core"] if f["id"] != gone]
            return True
        if ax == "mac-composer.send": return self.send()
        if ax == "mac-composer.undo":
            if not self.bar: return False
            self.bar = None
            self.stale = True                                        # the core's cancel does not give the draft back
            if not self.on("undo-gives-the-files-back") and not (self.on("second-undo-gives-the-files-back") and self.undos): d["core"] = self.parked
            else: d["core"] = []
            self.undos += 1
            return True
        return False

    def send(self):
        d = self.d
        if self.bar or not d["body"] or not d["to"]: return False
        if self.stale and self.on("send-after-the-undo-goes-again"): d["line"] = 'NotFound(kind: "draft", id: "%s")' % d["id"]; return True   # the app's stale id
        self.stale = False
        self.settle()
        t = self.tiles()
        if any(f["state"] == "gone" for f in t) and not self.on("send-with-a-gone-file-is-refused-with-its-sentence"):
            d["line"] = self.SAY["gone"]; d["refused_once"] = True; return True
        if self.on("send-goes-once-the-gone-file-is-removed") and d.get("refused_once"): d["line"] = self.SAY["gone"]; return True
        self.bar = {"id": "ob%d" % (len(self.out) + 1)}
        self.parked = list(d["core"])
        self.out[self.bar["id"]] = [] if self.on("send-carries-the-files-to-the-outbox-entry") or (self.on("forward-send-carries-the-remaining-file") and d["kind"] == "forward") else list(d["core"])
        d["core"] = []
        return True

    # -- reads
    def rows(self, ax):
        d = self.d if self.open else None
        if ax == "mac-main.list": return [{"id": i} for i in ("msg-01", "msg-02", "msg-03")]
        if ax == UB: return [{"ax": ax, "undo_offered": True, "outbox_id": self.bar["id"]}] if self.bar else []
        if d is None: return []
        if ax == HEAD: return [{"ax": ax, "draft_id": d["id"], "line": d["line"], "to": d["to"], "pictures": ["cid%d" % i for i, _ in enumerate(d.get("pictures", []))],
                                "picture_sources": list(d.get("pictures", []))}]
        if ax == BODY: return [{"ax": ax, "text": d["body"], "editor_text": d.get("editor", d["body"])}]
        if ax == ATT: return [{"ax": ax, "name": t["name"], "state": t["state"], "id": t["id"], "source_path": t.get("source_path", "")} for t in self.tiles()]
        if ax == NOTES: return [{"ax": ax, "name": t["name"], "words": {"adding": "Adding", "gone": "Not on this Mac", "will_download": "Will be fetched when sent", "too_large": "Too large"}[t["state"]]} for t in self.tiles() if t["state"] != "ready"]
        return []

    def read(self, path):
        parts = path.split("/")
        if parts[0] == "draft" and self.d and parts[1] == self.d["id"]:
            files = [{"id": f["id"], "file_name": f["name"], "size_bytes": f["size"], "state": f["state"]} for f in self.d["core"]]
        elif parts[0] == "outbox" and parts[1] in self.out:
            files = [{"id": f["id"], "file_name": f["name"], "size_bytes": f["size"], "state": "ready" if f["state"] == "gone" else f["state"]} for f in self.out[parts[1]]]
        else: return None
        return json.dumps({"total_bytes": sum(f["size_bytes"] for f in files), "attachments": files})

    def handle(self, verb, args):
        if verb == "key": return {"ok": True} if self.key(args[0]) else {"ok": False, "error": "key does nothing"}
        if verb == "type": self.type(args[0]); return {"ok": True}
        if verb == "click": return {"ok": True} if self.click(args[0]) else {"ok": False, "error": "did nothing"}
        if verb == "rows": return {"ok": True, "rows": self.rows(args[0])}
        if verb == "keyevent":
            d = self.d if self.open else None
            if d is None: return {"ok": False, "error": "nothing"}
            if args[0] == "⌘V":                              # the text view's paste: a staged file is attached, its name stays out of the text
                if not getattr(self, "staged", False): return {"ok": True}
                if self.staged == "image":                   # picture data: into the text, its file under the app's data folder (mutants: a tile; the temporary folder)
                    home = "/tmp/whoosh-pasted" if self.on("pasted-picture-lives-in-the-app-data-folder") else os.path.join(RIG_DIR["path"], "pasted-files", "u1")
                    path = os.path.join(home, PASTED_PICTURE + ".png")
                    if self.on("pasted-picture-goes-into-the-text"):
                        d["core"].append({"id": "attP", "name": PASTED_PICTURE + ".png", "size": 70, "state": "ready", "source_path": path})
                    else:
                        d.setdefault("pictures", []).append(path)
                    return {"ok": True}
                if self.on("pasted-file-becomes-a-tile-not-text"): d["body"] += PASTED
                else: self.add(PASTED, 40000, True)
                return {"ok": True}
            if args[0] == "Return":                          # round 20: a line, nothing else (the mutant: Return sends)
                if self.on("return-in-the-body-adds-a-line-and-sends-nothing"): self.send(); return {"ok": True}
                d.setdefault("hist", []).append(d["body"]); d["body"] += "\n"; return {"ok": True}
            if args[0] == "\u2318Return":                    # round 20: the send key (the mutant: it answers and queues nothing)
                if not self.on("cmd-return-in-the-body-sends"): self.send()
                return {"ok": True}
            if args[0] != "⌘Z":                              # real key presses into the text: one typing step the view can undo
                d.setdefault("hist", []).append(d["body"]); d["body"] += args[0]
                return {"ok": True}
            if self.on("cmd-z-leaves-the-editor-and-the-copy-equal"):     # the view's own undo emptied it; the copy kept the words
                d["editor"] = d["hist"][-1] if d.get("hist") else ""; d.setdefault("redo", []).append(d["body"]); return {"ok": True}
            if d.get("hist") and not self.on("cmd-z-in-the-body-undoes-the-typed-text"):
                d.setdefault("redo", []).append(d["body"]); d["body"] = d["hist"].pop()
            return {"ok": True}
        if verb == "core": return {"ok": True, "value": self.read(args[1])}
        if getattr(self, "dead", False): return {"ok": False, "error": "socket closed"}       # the copy ended
        if verb == "fixture" and args[:1] == ["reset"]:
            if getattr(self, "search_focused", False) and self.on("fixture-reset-with-the-search-field-focused-keeps-the-copy-alive"): self.dead = True
            return {"ok": True}
        if verb == "uistate": return {"ok": True, "state": {"surface": self.surface, "overlay": "mac-thread" if self.thread else None, "focus_message": self.nrow.get("focus", "msg-01")}}
        return {"ok": False, "error": "nothing"}


def model_run_files(mut):
    """Drives round 7 over a socket served by FilesModel with the mutants `mut` on; returns {check label: (state, detail)}."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.harness import Journey                                      # noqa: E402
    SPEED["settle"] = 0.0
    REFUSED_FORWARD["staged"] = True
    RIG_DIR["path"] = "/model-run"                                      # the model's run folder: its pasted pictures lie under /model-run/pasted-files
    ctl, close = serve_model(FilesModel(mut), "par-cm7-")
    try:
        j = Journey("par-composermac-r7", ctl)
        drive_r7(j)
        return {c.label: (c.state, c.detail) for c in j.checks()}
    finally:
        close()


def model_run_doors(mut):
    """Drives round 6 over a socket served by DoorsModel with the mutants `mut` on; returns {check label: (state, detail)}."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.harness import Journey                                      # noqa: E402
    SPEED["settle"] = 0.0
    ctl, close = serve_model(DoorsModel(mut), "par-cm6-")
    try:
        j = Journey("par-composermac-r6", ctl)
        drive_r6(j)
        return {c.label: (c.state, c.detail) for c in j.checks()}
    finally:
        close()


# ---- the selftest of rounds 1 and 2: a model of the composer's lists, keys and format bar, clean and once per mutant ----
# MailModel answers the socket verbs drive_r1 and drive_r2 use, the way ComposerModel (+Send, +Doors, +Remind, the footer, the template and snippet
# stores, the format bar) is written to answer them. One mutant (named by its check) is switched on at a time and that check must go red.
# Round 1 and round 2 run on two models: round 2 starts where round 1 leaves the app (a new message holding the sample file), so a round-1 mutant
# never reaches a round-2 case. Where a mutant is narrower than its declared words, SELFTEST_NARROWED says how and why (docs/lanes/gap-composer-models.md).
import re as _re

R12_UNMODELLED = {}                                                  # label -> the reason a case has no model (none today: the count printed says so)


class MailModel:
    ACCT = ACCOUNT[M["account"]]["address"]
    ADDRS = ["first@account-example.test", ACCT, "third@account-example.test"]      # the receiving account is NOT the first: "takes the first account" must be red
    SIG = "Best regards,\nAssem"
    SNIPS = [("Thanks, received", "Thanks, received. I will come back to you."), ("Meeting", "Can we meet on Thursday?")]
    EMOJI = [(":tada", "\U0001F389"), (":thumbsup", "\U0001F44D")]
    INSTANT = ["Yes, that works for me.", "I need a little more time.", "I cannot do this, sorry."]
    TEMPLATES = [{"title": "Delivery confirmation", "subject": "Delivery confirmation", "body": "Please confirm the delivery date for this order.", "files": 0}]
    ROWS_ORDER = ["from", "to", "cc", "bcc", "subject", "body"]
    KEYS = {"mac-composer.attach": "⌘⇧A", "mac-composer.from": "⌘⇧F", "mac-composer.send": "⌘↵", "mac-composer.send-later": "⌘⇧L",
            "mac-composer.cc.show": "⌘⇧C", "mac-composer.bcc.show": "⌘⇧B", "mac-composer.discard": "⌘⇧,", "mac-composer.snippets": "⌘;",
            "mac-composer.format-bar.bold": "⌘B", "mac-composer.format-bar.italic": "⌘I", "mac-composer.format-bar.underline": "⌘U",
            "mac-composer.format-bar.strike": "⇧⌘X", "mac-composer.format-bar.link": "⌘K", "mac-composer.format-bar.color": "⌘O",
            "mac-composer.format-bar.list": "⇧⌘8", "mac-composer.format-bar.numbers": "⇧⌘7", "mac-composer.format-bar.quote": "⇧⌘9"}
    CAPS = ("mac-composer.send", "mac-composer.from", "mac-composer.send-later")

    def __init__(self, mut, round_no):
        self.m = mut
        self.r2 = round_no == 2                                      # round 2 never presses Escape to close the window: Escape with nothing to close does nothing
        self.surface = "mac-main"
        self.open = False
        self.d = None
        self.kept = None                                             # the draft Escape left behind (the window closed, the words kept)
        self.ghost = None                                            # the mutant that keeps a discarded draft for the draft row
        self.snips = list(self.SNIPS)
        self.templates = [dict(t) for t in self.TEMPLATES]
        self.writer = False
        self.n_entries = 0
        self.archived = set()
        self.bar = None                                              # the undo bar: offered, text, outbox id
        self.sending = False                                         # the composer's own send is in its undo window
        self.pending_close = False
        self.presents = 0
        self.spent = set()
        self.plain_once = self.on("bold-key-marks-the-typed-text")        # narrowed: the first run typed in bold stays plain
        if round_no == 2:                                            # what round 1 leaves: a new message holding the sample file
            self.new_draft("new")
            self.d["focused"] = "to"
            self.d["files"] = ["whoosh-sample-quote.txt"]
            self.open = True; self.surface = "mac-composer"

    def on(self, name): return name in self.m

    def first(self, name):
        """True the first time `name` is asked while its mutant is on (a key that fails once, then works: a key pressed again later by another case is not the case's)."""
        if name not in self.m or name in self.spent: return False
        self.spent.add(name); return True

    # -- the draft
    def new_draft(self, kind):
        reply = kind == "reply"
        self.d = {"kind": kind, "to": [M["from"]] if reply else [], "cc": [], "bcc": [], "subject": "Re: " + M["subject"] if reply else "", "body": "", "files": [],
                  "focused": "", "frm": 1 if reply else 0, "from_open": False, "from_hi": 0, "attach_open": False, "color_open": False, "link_open": False,
                  "ask": False, "ring": "", "showcc": False, "showbcc": self.on("bcc-hidden-first"), "toin": "", "linkin": "", "inline": None, "inline_q": "",
                  "snip_open": False, "tpl_open": False, "ir_open": False, "footer": self.on("footer-off-by-default"), "footer_stuck": False,
                  "marks": [], "color": "", "list": "none", "indent": 0, "quote": False, "runs": [], "cursor": True, "pre_send": ""}
        if reply:
            self.d["focused"] = "" if self.on("reply-opens-with-the-cursor-in-the-text") else "body"
            if self.on("reply-goes-to-sender"): self.d["to"] = []
            if self.on("from-is-receiving-account"): self.d["frm"] = 0

    def present(self):
        self.open = True
        self.presents += 1
        self.surface = "mac-main" if self.on("composer-in-window-slot") and self.presents == 1 else "mac-composer"       # narrowed: the first window only

    def finish(self):
        """The beat after an instant send: the window closes."""
        if self.pending_close:
            self.pending_close = False
            self.open = False; self.d = None; self.bar = None; self.sending = False; self.surface = "mac-main"

    def can_send(self):
        d = self.d
        return bool(d["body"].strip()) and (bool(d["to"]) or d["kind"] == "reply")      # a reply with no To goes to its thread's sender (the quick-send)

    def panels_closed(self):
        d = self.d
        d["from_open"] = False; d["attach_open"] = False; d["color_open"] = False; d["link_open"] = False
        d["snip_open"] = False; d["tpl_open"] = False; d["ir_open"] = False

    def any_panel(self):
        d = self.d
        return d["from_open"] or d["attach_open"] or d["color_open"] or d["link_open"] or d["snip_open"] or d["tpl_open"] or d["ir_open"]

    def visible_rows(self):
        d = self.d
        return [r for r in self.ROWS_ORDER if (r != "cc" or d["showcc"]) and (r != "bcc" or d["showbcc"])]

    def focus(self, f, keep_from=False):
        d = self.d
        vis = self.visible_rows()
        was = d["focused"]
        d["focused"] = f
        if f in ("cc", "bcc"): d["showcc" if f == "cc" else "showbcc"] = True
        if was in vis and f in vis:
            near = abs(vis.index(was) - vis.index(f)) == 1
            d["ring"] = "slide" if near else "fade"
            if self.on("near-rows-slide"): d["ring"] = "fade"
            if self.on("far-rows-fade"): d["ring"] = "slide"
        elif f in vis:
            d["ring"] = "fade"
        if f != "from" and not self.on("from-list-closes-when-the-cursor-leaves"): d["from_open"] = False
        if f == "body" and self.on("message-key-puts-the-cursor-in-the-text"): d["cursor"] = False

    def outgoing(self):
        d = self.d
        sig = "" if self.on("outgoing-text-ends-with-the-served-signature") else self.SIG
        text = d["body"] + ("\n\n" + sig if sig else "")
        if (d["footer"] or d["footer_stuck"]) and not self.on("footer-on-ends-the-text"): text += "\n\n" + FOOTER
        return text

    # -- inline lists
    def inline_items(self):
        d = self.d
        if d["inline"] == "snippet":
            snips = self.snips[:1] if self.on("snippet-list-offers-every-snippet") else self.snips
            q = d["inline_q"].lower()
            if q and not self.on("snippet-list-filters-by-name"): snips = [(n, t) for n, t in snips if q in n.lower()]
            return [("snippet", n, "", t) for n, t in snips]
        if d["inline"] == "emoji":
            return [("emoji", n, g, "") for n, g in self.EMOJI if n.startswith(":" + d["inline_q"])]
        return []

    def after_text(self):
        d = self.d
        mt = _re.search(r":([a-z]+)$", d["body"])
        if mt and d["inline"] != "snippet" and not self.on("colon-name-opens-the-emoji-list"):
            d["inline"] = "emoji"; d["inline_q"] = mt.group(1)
        elif d["inline"] == "emoji": d["inline"] = None

    def inline_pick(self):
        d = self.d
        items = self.inline_items()
        if not items: return False
        kind, name, glyph, text = items[0]
        if kind == "snippet":
            trig = ";" + d["inline_q"]
            if not self.on("return-inserts-the-snippet-and-drops-the-trigger"): d["body"] = d["body"][:-len(trig)] + text
            else: d["body"] = d["body"] + text
        elif not self.on("return-inserts-the-emoji"): d["body"] = d["body"][:-len(name)] + glyph
        if not self.on("snippet-list-closes-after-the-pick") or kind != "snippet": d["inline"] = None; d["inline_q"] = ""
        return True

    # -- the format state
    def add_run(self, text, href=""):
        d = self.d
        marks = [x for x in ("bold", "italic", "underline", "strike") if x in d["marks"]]
        d["marks"] = marks
        run = {"text": text, "format": "+".join(marks + (["color:" + d["color"]] if d["color"] and not self.on("colour-marks-the-typed-text") else [])) or "plain",
               "indent": d["indent"], "block": "quote" if d["quote"] else "none"}
        if href: run["href"] = href
        if self.plain_once and "bold" in marks: run["format"] = "plain"; self.plain_once = False
        d["runs"].append(run)

    def mark(self, name, mut):
        d = self.d
        if self.on(mut): return True
        if name in d["marks"] and name == "strike" and self.on("format-keys-toggle-off"): return True      # narrowed: the last second press leaves its mark on
        if name in d["marks"]: d["marks"].remove(name)
        else: d["marks"].append(name)
        d["marks"] = [x for x in ("bold", "italic", "underline", "strike") if x in d["marks"]]
        return True

    def block(self, **kw):
        d = self.d
        if d["runs"]: d["runs"][-1].update(kw)

    # -- keys
    def key(self, chord):
        self.finish()
        chord = chord.replace("⌘⇧", "⇧⌘")
        if self.sending_quick_key(chord): return True
        d = self.d if self.open else None
        if d is None: return False
        if chord == "⇧⌘,":
            if self.on("discard-key-asks-first"): return self.discard_now()
            if d["body"] or d["files"] or d["subject"]: d["ask"] = True; return True
            return self.discard_now()
        if chord == "Enter":
            if d["ask"]: return False if self.on("enter-discards-and-closes-the-composer") else self.discard_now()
            if d["inline"]: return self.inline_pick()
            if d["from_open"]:
                if self.on("from-list-keys-pick-with-return"): return False
                d["frm"] = d["from_hi"]; d["from_open"] = False; self.focus("body"); return True
            return False
        if chord == "Down" and d["from_open"]: d["from_hi"] = (d["from_hi"] + 1) % len(self.ADDRS); return True
        if chord == "Escape": return self.escape()
        if chord == "⌘Enter": return self.send(False, False, False)
        if chord == "⌘⇧Z" or chord == "⇧⌘Z": return self.send(False, True, False)
        if chord == "⇧⌘Enter": return self.send(True, False, False)
        if chord == "Z":
            if not self.sending: return False
            self.bar = None; self.sending = False
            d["body"] = "" if self.on("undo-key-brings-text-back") else d["pre_send"]
            for mid in list(self.archived): self.archived.discard(mid)
            return True
        if chord == "⌘;":
            if self.on("snippets-key-opens-list") or self.on("snippet-picker-key-opens-the-picker"): return False
            self.panels_closed(); d["snip_open"] = True; return True
        if chord == "⇧⌘B":
            if self.on("bcc-key-shows-the-row"): return False
            if self.on("bcc-key-focuses-bcc"): d["showbcc"] = True; self.focus("to"); return True
            self.focus("bcc"); return True
        if chord == "⇧⌘C":
            if self.on("cc-key-shows-the-row"): return False
            if self.on("cc-key-focuses-cc"): d["showcc"] = True; self.focus("to"); return True
            self.focus("cc"); return True
        if chord == "⇧⌘O": return False if self.first("to-key-focuses-to") else (self.focus("to") or True)
        if chord == "⇧⌘S": return False if self.on("subject-key-focuses-subject") else (self.focus("subject") or True)
        if chord == "⇧⌘F":
            if self.first("from-key-opens-the-sender-list"): return False
            self.panels_closed(); self.focus("from"); d["from_open"] = True; d["from_hi"] = d["frm"]; return True
        if chord == "⇧⌘M":
            if d["focused"] == "from" and self.on("message-key-focuses-the-body"): return False       # narrowed: the press that follows the From key (the case's own)
            self.focus("body"); return True
        if chord == "⇧⌘I":
            if self.on("move-key-sends-the-contacts-to-bcc"): return False
            d["bcc"] += d["to"]; d["to"] = []; d["showbcc"] = True; return True
        if chord == "⇧⌘A":
            if self.on("attach-key-opens-the-panel"): return False
            self.panels_closed(); d["attach_open"] = True; return True
        if chord == ";":
            d["body"] += ";"
            if not self.on("semicolon-opens-the-snippet-list"): d["inline"] = "snippet"; d["inline_q"] = ""
            return True
        if chord == "⌘B": return self.mark("bold", "bold-key-turns-bold-on")
        if chord == "⌘I": return self.mark("italic", "italic-key-adds-italic")
        if chord == "⌘U": return self.mark("underline", "underline-key-adds-underline")
        if chord == "⇧⌘X": return self.mark("strike", "strikethrough-key-adds-strike")
        if chord == "⌘O":
            if self.on("color-key-opens-the-colour-panel-in-the-composer"): return False
            self.panels_closed(); d["color_open"] = True; return True
        if chord == "⌘K":
            if self.on("link-key-opens-the-link-panel"): return False
            self.panels_closed(); d["link_open"] = True; d["linkin"] = ""; return True
        if chord == "⇧⌘8": return False if self.on("bullets-key-makes-a-bullet-item") else self.set_list("bullet", "list-item")
        if chord == "⇧⌘7": return self.set_list("bullet" if self.on("numbers-key-makes-a-numbered-item") else "number", "list-item" if self.on("numbers-key-makes-a-numbered-item") else "numbered-item")
        if chord == "⇧⌘9":
            if self.on("quote-key-makes-a-quote"): return False
            d["quote"] = not d["quote"]; d["list"] = "none"; self.block(block="quote" if d["quote"] else "none"); return True
        if chord == "Tab": return self.indent(1, "tab-indents-a-list-item", True)
        if chord == "Shift+Tab": return self.indent(-1, "shift-tab-outdents-a-list-item", True)
        if chord == "⌘]": return self.indent(1, "increase-indent-key-indents-the-paragraph", False)
        if chord == "⌘[": return self.indent(-1, "decrease-indent-key-outdents-the-paragraph", False)
        return False

    def set_list(self, kind, word):
        d = self.d
        d["list"] = kind; d["quote"] = False; self.block(format=word, block="none"); return True

    def indent(self, step, mut, only_list):
        d = self.d
        if self.on(mut): return False
        d["indent"] = max(0, d["indent"] + step); self.block(indent=d["indent"]); return True

    def sending_quick_key(self, chord):
        """The quick reply that takes Cmd-Enter before the composer (send-key-reaches-composer): a bar with no undo, the composer never told."""
        if chord == "⌘Enter" and self.on("send-key-reaches-composer") and self.open and self.d and self.can_send():
            self.n_entries += 1
            self.bar = {"offered": False, "text": "Sending", "id": "ob%d" % self.n_entries}
            return True
        return False

    def discard_now(self):
        if self.on("a-discarded-draft-is-not-kept") and self.d: self.ghost = {"body": len(self.d["body"]) or 1, "subject": len(self.d["subject"]) or 1}
        self.d = None; self.open = False; self.surface = "mac-main"; self.kept = None; self.bar = None; self.sending = False
        return True

    def escape(self):
        d = self.d
        if d["ask"]:
            if self.on("esc-answers-the-question-with-keep"): return self.discard_now()
            d["ask"] = False; return True
        if d["inline"]:
            d["inline"] = None; d["inline_q"] = ""
            if self.on("esc-closes-the-emoji-list-not-the-composer"): self.surface = "mac-main"       # narrowed: the window reads closed, the draft rows still answer
            return True
        if d["attach_open"] and self.on("esc-closes-the-attach-panel"): return True          # the panel stays
        if self.any_panel(): self.panels_closed(); return True
        if self.r2: return True                                      # round 2 never closes the window by Escape (see __init__)
        if self.on("escape-closes-window"): return True
        self.open = False; self.surface = "mac-main"
        self.kept = None if self.on("escape-kept-the-draft") else d
        if self.kept is None: self.d = None
        return True

    # -- typing
    def type(self, text):
        self.finish()
        d = self.d if self.open else None
        if d is None: return
        if d["link_open"]: d["linkin"] += text; return
        f = d["focused"]
        if f == "to":
            parts = text.split(",")
            for a in parts[:-1]:
                if a.strip() and not self.on("to-takes-typed-recipient"): d["to"].append(a.strip())
            d["toin"] += parts[-1]
        elif f == "subject": d["subject"] += text
        elif f in ("cc", "bcc"): d["toin"] += text
        else:
            if self.first("typed-text-in-body"): return
            d["body"] += text
            if d["inline"] == "snippet": d["inline_q"] += text
            self.add_run(text)
            self.after_text()

    # -- clicks
    def click(self, ax):
        self.finish()
        if ax == "mac-main.reader.open-full-composer":
            if self.open: return True
            if self.kept is not None: self.d = self.kept; self.kept = None
            else: self.new_draft("reply")
            self.present(); return True
        if ax == "mac-composer.dev.sample-file":
            if not self.open:                                        # a file handed over from outside (the notch's sink) opens a new message
                if self.on("outside-file-opens-the-composer"): return False
                self.new_draft("new"); self.d["focused"] = "to"; self.present()
                if self.on("outside-file-is-attached"): return True
            if "whoosh-sample-quote.txt" not in self.d["files"]: self.d["files"].append("whoosh-sample-quote.txt")
            return True
        d = self.d if self.open else None
        if d is None: return False
        if ax == "mac-composer.dev.fixture-writer": self.writer = True; return True
        if ax == "mac-composer.snippets.0":
            if not d["snip_open"]: return False
            if not self.on("snippet-lands-in-text"): d["body"] += self.snips[0][1]
            if not self.on("snippet-list-closes"): d["snip_open"] = False
            return True
        if ax == "mac-composer.templates":
            was = d["tpl_open"]; self.panels_closed(); d["tpl_open"] = not was; return True
        if ax == "mac-composer.templates.save":
            if not d["tpl_open"]: return False
            self.templates.append({"title": d["subject"] or "Saved", "subject": d["subject"], "body": d["body"], "files": 0 if self.on("saved-template-keeps-its-file") else len(d["files"])})
            return True
        m = _re.fullmatch(r"mac-composer\.templates\.(\d+)", ax)
        if m:
            i = int(m.group(1))
            if not d["tpl_open"] or i >= len(self.templates): return False
            t = self.templates[i]
            if not self.on("template-fills-subject"): d["subject"] = t["subject"]
            if not self.on("template-fills-text"): d["body"] += t["body"]
            d["tpl_open"] = False; return True
        if ax == "mac-composer.footer":
            if d["footer"]:
                d["footer"] = False
                if self.on("footer-off-again"): d["footer_stuck"] = True
            else: d["footer"] = True
            return True
        if ax == "mac-composer.instant-reply":
            if not self.writer: return False
            self.panels_closed(); d["ir_open"] = True; return True
        m = _re.fullmatch(r"mac-composer\.instant-reply\.(\d+)", ax)
        if m:
            i = int(m.group(1))
            if not d["ir_open"]: return False
            n = 1 if self.on("instant-reply-offers-three") else 3
            if i >= n: return False
            if not self.on("instant-reply-pick-fills-text"): d["body"] = self.INSTANT[i]
            d["ir_open"] = False; return True
        if ax == "mac-composer.from":
            if self.on("from-click-opens-the-list"): return False
            self.panels_closed(); d["from_open"] = True; d["from_hi"] = d["frm"]; return True
        m = _re.fullmatch(r"mac-composer\.from\.(\d+)", ax)
        if m:
            i = int(m.group(1))
            if not d["from_open"] or i >= len(self.ADDRS): return False
            if not self.on("from-choice-changes-the-sender"): d["frm"] = i
            d["from_open"] = False; return True
        if ax == "mac-composer.attach":
            if self.on("attach-button-opens-the-panel"): return False
            self.panels_closed(); d["attach_open"] = True; return True
        if ax == "mac-composer.discard":
            if self.on("discard-button-asks-first"): return False           # narrowed: no question and nothing thrown away (declared: discards at once; the window would go and every later case read no row)
            if d["body"] or d["files"] or d["subject"]: d["ask"] = True; return True
            return self.discard_now()
        if ax == "mac-composer.discard.keep":
            if not d["ask"]: return False
            d["ask"] = False
            if self.on("keep-button-keeps-the-draft"): d["bcc"] = []       # narrowed: the draft's recipients go, the window stays
            return True
        if ax == "mac-composer.discard.confirm": return self.discard_now() if d["ask"] else False
        if ax == "mac-composer.link.apply":
            if not d["link_open"]: return False
            if not self.on("link-applies-https-to-the-address"): self.add_run(d["linkin"], href="https://" + d["linkin"])
            if not self.on("link-panel-closes"): d["link_open"] = False
            return True
        m = _re.fullmatch(r"mac-composer\.color\.(default|secondary|accent|warning)", ax)
        if m:
            if not d["color_open"]: return False
            if not self.on("colour-pick-sets-the-colour"): d["color"] = "" if m.group(1) == "default" else m.group(1)
            d["color_open"] = False; return True
        m = _re.fullmatch(r"mac-composer\.format-bar\.(bold|italic|underline|strike|list|numbers|quote|link|color)", ax)
        if m:
            k = m.group(1)
            if self.on("format-bar-%s-button-acts" % k) or self.on("format-bar-%s-button-opens-the-panel" % k): return False
            if k in ("bold", "italic", "underline", "strike"): return self.mark(k, "none")
            if k == "list": d["list"] = "none" if d["list"] == "bullet" else "bullet"; return True
            if k == "numbers": d["list"] = "none" if d["list"] == "number" else "number"; return True
            if k == "quote": d["quote"] = not d["quote"]; return True
            self.panels_closed()
            if k == "link": d["link_open"] = True
            else: d["color_open"] = True
            return True
        return False

    def send(self, done, instant, _unused):
        d = self.d
        if self.sending or not self.can_send(): return False
        d["pre_send"] = d["body"]
        self.n_entries += 2 if self.on("send-queued-once") else 1
        offered = not instant or self.on("instant-send-offers-no-undo")
        text = "Sending" if (not instant or self.on("instant-send-says-sent-at-once")) else "Sent"
        if done and self.on("send-and-done-queues-a-send"):
            self.n_entries -= 1                                      # archives without sending
            self.archived.add(M["id"]); return True
        self.bar = {"offered": offered, "text": text, "id": "ob%d" % self.n_entries}
        self.sending = not instant
        if done and not self.on("send-and-done-archives-the-answered-message"): self.archived.add(M["id"])
        if instant: self.pending_close = True
        return True

    def advance(self, secs):
        """`clock advance`: an undo window that ran out is sent, and the composer of that send closes."""
        if self.sending and secs >= 10:
            self.sending = False; self.bar = None; self.open = False; self.d = None; self.surface = "mac-main"

    # -- reads
    def rows(self, ax):
        d = self.d if self.open else None
        if ax == "mac-main.list":
            ids = ["msg-01", "msg-02", "msg-03"]
            if self.on("answered-message-listed-before"): ids = [i for i in ids if i != M["id"]]
            return [{"ax": "mac-main.list.row.%d" % i, "id": x} for i, x in enumerate(i for i in ids if i not in self.archived)]
        if ax == UB:
            return [{"ax": ax, "undo_offered": self.bar["offered"], "text": self.bar["text"], "outbox_id": self.bar["id"]}] if self.bar else []
        if ax == "mac-composer.draft":
            if self.ghost and not self.d: return [{"ax": ax, "kept": True, "body_chars": self.ghost["body"], "subject_chars": self.ghost["subject"]}]
            dd = self.d
            return [{"ax": ax, "kept": dd is not None, "body_chars": len(dd["body"]) if dd else 0, "subject_chars": len(dd["subject"]) if dd else 0}]
        if d is None: return []
        if ax == HEAD:
            return [{"ax": ax, "from": self.ADDRS[d["frm"]], "to": list(d["to"]), "cc": list(d["cc"]), "bcc": list(d["bcc"]), "subject": d["subject"], "focused": d["focused"],
                     "signature": "" if self.on("outgoing-text-ends-with-the-served-signature") else self.SIG, "outgoing": self.outgoing(),
                     "cc_visible": d["showcc"], "bcc_visible": d["showbcc"], "discard_ask": d["ask"], "ring_motion": d["ring"], "from_open": d["from_open"],
                     "attach_open": d["attach_open"], "color_open": d["color_open"], "link_open": d["link_open"], "is_open": True, "draft_id": "", "line": ""}]
        if ax == BODY: return [{"ax": ax, "text": d["body"], "editor_text": d.get("editor", d["body"])}]
        if ax == "mac-composer.format":
            return [{"ax": ax, "bold": "bold" in d["marks"], "italic": "italic" in d["marks"], "underline": "underline" in d["marks"], "strike": "strike" in d["marks"],
                     "color": d["color"], "list": d["list"], "indent": d["indent"], "quote": d["quote"], "body_focused": d["focused"] == "body" and d["cursor"]}]
        if ax == "mac-composer.runs": return [dict(r, ax=ax) for r in d["runs"]]
        if ax == "mac-composer.attachments": return [] if self.on("file-attached") and d["kind"] == "reply" else [{"ax": ax, "name": f} for f in d["files"]]
        if ax == "mac-composer.attach.panel": return [{"ax": ax}] if d["attach_open"] else []
        if ax == "mac-composer.color.options": return [{"ax": "mac-composer.color." + s} for s in ("default", "secondary", "accent", "warning")] if d["color_open"] else []
        if ax == "mac-composer.templates":
            return [{"ax": "mac-composer.templates.%d" % i, "title": t["title"], "files": t["files"]} for i, t in enumerate(self.templates)] if d["tpl_open"] else []
        if ax == "mac-composer.snippets": return [{"ax": "mac-composer.snippets.%d" % i, "title": n, "text": t} for i, (n, t) in enumerate(self.snips)] if d["snip_open"] else []
        if ax == "mac-composer.instant-reply.options":
            if not d["ir_open"]: return []
            n = 1 if self.on("instant-reply-offers-three") else 3
            return [{"ax": "mac-composer.instant-reply.%d" % i, "text": "" if self.on("instant-reply-options-are-sentences") else self.INSTANT[i]} for i in range(n)]
        if ax == "mac-composer.inline":
            return [{"ax": "mac-composer.inline.%d" % i, "kind": k, "title": n, "glyph": g, "text": t} for i, (k, n, g, t) in enumerate(self.inline_items())]
        if ax == "mac-composer.controls":
            out = []
            for a, k in self.KEYS.items():
                tip = "Control" if a == "mac-composer.attach" and self.on("every-control-shows-its-key") else "Control  " + k
                out.append({"ax": a, "tooltip": tip, "cap": a in self.CAPS and not (a == "mac-composer.from" and self.on("send-and-from-show-a-key-cap"))})
            return out
        return []

    def handle(self, verb, args):
        if verb == "key": return {"ok": True} if self.key(args[0]) else {"ok": False, "error": "key does nothing"}
        if verb == "type": self.type(args[0]); return {"ok": True}
        if verb == "click": return {"ok": True} if self.click(args[0]) else {"ok": False, "error": "did nothing"}
        if verb == "rows": return {"ok": True, "rows": self.rows(args[0])}
        if verb == "core": return {"ok": True, "value": self.n_entries if args[1] == "outbox_entry/count" else None}
        if verb == "clock":
            self.advance(int(args[1])); return {"ok": True}
        if verb == "uistate": return {"ok": True, "state": {"surface": self.surface, "overlay": None, "focus_message": M["id"]}}
        return {"ok": False, "error": "nothing"}


def model_run_mail(mut, round_no):
    """Drives round 1 or round 2 over a socket served by MailModel with the mutants `mut` on; returns {check label: (state, detail)}."""
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.harness import Journey                                      # noqa: E402
    SPEED["settle"] = 0.0
    ctl, close = serve_model(MailModel(mut, round_no), "par-cm%d-" % round_no)
    try:
        j = Journey("par-composermac" if round_no == 1 else "par-composermac-r2", ctl)
        (drive_r1 if round_no == 1 else drive_r2)(j)
        return {c.label: (c.state, c.detail) for c in j.checks()}
    finally:
        close()


def model_run_r1(mut): return model_run_mail(mut, 1)
def model_run_r2(mut): return model_run_mail(mut, 2)


def selftest():
    bad = 0
    total = 0
    suites = (("round-1", model_run_r1, ("# par-composermac/",)),
              ("round-2", model_run_r2, ("# par-composermac-r2/",)),
              ("round-3, round-4 and round-5", model_run, ("# par-composermac-r3/", "# par-composermac-r4/", "# par-composermac-r5/")),
              ("round-6", model_run_doors, ("# par-composermac-r6/",)),
              ("round-7", model_run_files, ("# par-composermac-r7/",)))
    for title, runner, prefixes in suites:
        clean = runner(set())
        print_model_cases(clean)
        for label, (state, why) in clean.items():
            if state != "pass":
                print("RED on the model: %s %s" % (label, why)); bad += 1
        print("%s model, no mutant: %d of %d checks green" % (title, sum(1 for st, _ in clean.values() if st == "pass"), len(clean)))
        declared = {l.split(" | ")[0][2:].strip(): l.split(" | ", 1)[1].strip() for l in open(os.path.abspath(__file__), encoding="utf-8") if l.startswith(prefixes)}
        for label in clean:
            name = label.split("/", 1)[1]
            if label not in declared:
                print("NO MUTANT %s" % label); bad += 1; continue
            got = runner({name})
            state = got[label][0]
            others = sorted(k.split("/", 1)[1] for k, (st, _) in got.items() if st == "fail" and k != label)
            print("mutant %-62s -> %s%s" % (name, "RED" if state == "fail" else "NOT RED (%s)" % state, ("  also red: " + ", ".join(others)) if others else ""))
            if state != "fail": bad += 1
        listed = 0
        for l in sorted(set(declared) - set(clean)):
            if l in R12_UNMODELLED:
                print("NOT MODELLED %s: %s" % (l, R12_UNMODELLED[l])); listed += 1
            else:
                print("STRAY MUTANT %s" % l); bad += 1
        total += len(clean)
        print("%s: %d checks green on the model, %d mutants declared" % (title, len(clean), len(declared)))
        print("%s: cases modelled of cases declared: %d of %d (listed with a reason: %d)" % (title, len(set(declared) & set(clean)), len(declared), listed))
    print("selftest: %d checks, %d mutants not red or missing" % (total, bad))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else main())
