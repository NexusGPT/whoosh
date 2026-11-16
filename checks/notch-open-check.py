#!/usr/bin/env python3
"""The notch's open mail (owner's picks of 14 Nov 2026, docs/surfaces/mac-notch.md: "its word by default an a button to display full mail", "Full email
load pictures by default"), driven in the hidden test copy on the test engine:
   words     a mail opens on its words; no reply starts and the notch takes no keyboard; the folded history is one named line
   reply     Reply starts the reply and folds the mail to one line; a card's Reply button opens with the reply started
   full mail a written mail has no Full mail button; a designed one has it with the picture count; the button shows the page, the line says what
             happened to the pictures and the tracking pixel, Its words goes back; the pictures switch of Settings changes the line; the choice
             lasts for that mail only
   snooze    Snooze shows three choices; one press snoozes the mail and the notch goes back to its list
   more      Mark unread and Delete act on the open mail
   keys      a key is the notch's only with the pointer on it or the keyboard held; a reply the notch started ends when its mail leaves the notch
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core --rig-hidden sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/notch-open-check.py'
Mutants (each must turn a named case red, on a build of the broken copy): O1 NotchModel.open starts the reply whatever `reply` says ->
a-mail-opens-on-its-words; O2 NotchOpenView's `full` ignores `hasPage` (and the row serves the button for every mail) -> a-written-mail-has-no-full-mail-button;
O3 toggleFullMail never clears `fullMailId` -> its-words-goes-back; O4 NotchCopy.pageFacts ignores `loaded` -> the-pictures-switch-changes-the-line;
O5 NotchModel.snooze leaves the mail in the notch -> one-choice-snoozes-the-mail; O6 NotchFeature.takesKeys always true ->
a-key-with-the-pointer-away-is-not-the-notchs; O7 NotchModel.endOpenReply returns at once -> a-reply-ends-when-its-mail-leaves-the-notch."""
import os, sys, time
sys.path.insert(0, os.getcwd())
from journeys.lib.ctl import Ctl, Unreadable

c = Ctl().connect()
P = "mac-settings-general"
total = bad = unjudged = 0

def case(name, got_fn, want):
    global total, bad, unjudged
    total += 1
    try:
        got = got_fn()
    except (Unreadable, KeyError, IndexError, TypeError, ValueError) as e:
        unjudged += 1
        print("NOT JUDGED %s: %s %s" % (name, type(e).__name__, str(e)[:200]), flush=True)
        return
    ok = got == want
    bad += not ok
    print("%s %s: got %r, wanted %r" % ("PASS" if ok else "FAIL", name, got, want), flush=True)

def click(ax):
    try:
        c.click(ax); return True
    except Unreadable:
        return False

def surface(): return c.rows("mac-notch")[0]
def rows(ax): return c.rows(ax)
def reset(): c.click("mac-notch.stage.reset"); time.sleep(0.12)
def card(kind="vip"):
    """A fresh notch with one card: an arrival of that kind through the real door (a VIP mail is a card with the start values)."""
    reset(); ok = click("mac-notch.stage.arrive.%s" % kind); time.sleep(0.12); return ok
def open_card(): ok = click("mac-notch.arrival.open"); time.sleep(0.15); return ok

# ---- words
def opens_on_words():
    delivered = card(); opened = open_card(); s = surface(); body = rows("mac-notch.open.body")
    return (delivered, opened, s["stage"], len(rows("mac-notch.open.quickreply")), s["keyed"], len(body), bool(body and body[0]["text"].strip()), body[0]["paragraphs"] >= 1 if body else None)
case("a-mail-opens-on-its-words", opens_on_words, (True, True, "open", 0, False, 1, True, True))
def own_text():
    # the words are the mail's own text: the arrival's preview line starts inside them (the words keep the mail's line breaks, the preview has none)
    r = rows("mac-notch.open.body")[0]
    flat = lambda t: " ".join(t.lower().split())
    return " ".join(flat(r["preview"]).split()[:4]) in flat(r["text"])
case("the-words-are-the-mails-own-text", own_text, True)
case("a-written-mail-has-no-full-mail-button", lambda: (rows("mac-notch.open.fullmail"), click("mac-notch.open.fullmail")), ([], False))

def folded_history():
    # the sample mails with quoted history are among the first of the inbox: open primary mails until one names a folded part
    seen = []
    for _ in range(8):
        reset(); click("mac-notch.stage.alert.primary.card")
        if not click("mac-notch.stage.arrive.primary"): break
        time.sleep(0.12); open_card()
        f = rows("mac-notch.open.folds")
        if f: return (f[0]["count"], f[0]["text"])
        seen.append(surface()["alert"])
        click("mac-notch.open.archive")       # this mail has no history: it leaves, the next primary mail takes its place
    return ("no sample mail with history among", len(seen))
case("folded-history-is-one-named-line", folded_history, (1, "Folded: 1 earlier message"))

# ---- reply
def reply_starts():
    card(); open_card()
    started = click("mac-notch.open.reply"); time.sleep(0.2)
    q = rows("mac-notch.open.quickreply"); folded = rows("mac-notch.open.folded")
    return (started, len(q), len(folded), bool(folded and folded[0]["sender"]), rows("mac-notch.open.body"), surface()["stage"])
case("reply-starts-the-reply-and-folds-the-mail", reply_starts, (True, 1, 1, True, [], "open"))
def card_reply():
    card(); ok = click("mac-notch.arrival.reply"); time.sleep(0.2)
    return (ok, surface()["stage"], len(rows("mac-notch.open.quickreply")))
case("a-cards-reply-opens-with-the-reply-started", card_reply, (True, "open", 1))

# ---- full mail, on a designed sample mail (the test engine serves one page with two pictures and one tracking pixel)
def designed():
    card(); marked = click("mac-notch.stage.designed"); open_card()
    r = rows("mac-notch.open.fullmail")
    return (marked, [(x["title"], x["key"], x["full"], x["pictures"], x["trackers"], x["facts"]) for x in r], len(rows("mac-notch.open.body")))
case("a-designed-mail-has-the-full-mail-button", designed, (True, [("Full mail", "O", False, 2, 1, "2 pictures not loaded")], 1))
def full():
    ok = click("mac-notch.open.fullmail"); time.sleep(0.2)
    r = rows("mac-notch.open.fullmail")[0]
    return (ok, r["title"], r["full"], r["loads_pictures"], r["facts"], rows("mac-notch.open.body"), surface()["stage"])
case("full-mail-shows-the-page-with-its-pictures", full, (True, "Its words", True, True, "2 pictures loaded · 1 tracker blocked", [], "open"))
def switch_off():
    off = click("%s.alerts.pictures" % P); time.sleep(0.15)
    r = rows("mac-notch.open.fullmail")[0]
    back = click("%s.alerts.pictures" % P); time.sleep(0.15)
    return (off, c.core("notch_alerts/full_mail_loads_pictures") is True, r["loads_pictures"], r["facts"], back, rows("mac-notch.open.fullmail")[0]["facts"])
case("the-pictures-switch-changes-the-line", switch_off, (True, True, False, "2 pictures not loaded · 1 tracker blocked", True, "2 pictures loaded · 1 tracker blocked"))
def words_back():
    ok = click("mac-notch.open.fullmail"); time.sleep(0.2)
    r = rows("mac-notch.open.fullmail")[0]
    return (ok, r["title"], r["full"], len(rows("mac-notch.open.body")))
case("its-words-goes-back", words_back, (True, "Full mail", False, 1))
def only_that_mail():
    click("mac-notch.open.fullmail"); time.sleep(0.15)                      # the page again, on the designed mail
    first = surface()["stage"]
    click("mac-notch.open.back"); click("mac-notch.stage.arrive.needs_reply"); time.sleep(0.12)   # another mail joins the list
    click("mac-notch.stage.hover"); click("mac-notch.list.row.1"); time.sleep(0.15)             # the newest: the other mail
    return (first, rows("mac-notch.open.fullmail"), len(rows("mac-notch.open.body")))
case("full-mail-lasts-for-that-mail-only", only_that_mail, ("open", [], 1))

# ---- snooze
def snooze_choices():
    card(); open_card()
    shown = click("mac-notch.open.snooze"); time.sleep(0.15)
    r = rows("mac-notch.open.snooze")[0]
    return (shown, r["open"], r["key"], [(x["title"], x["key"]) for x in r["choices"]])
case("snooze-shows-three-choices", snooze_choices, (True, True, "H", [("Later today", "1"), ("Tomorrow morning", "2"), ("Next week", "3")]))
def snooze_one():
    ident = [x["id"] for x in rows("mac-notch.list")][0]
    ok = click("mac-notch.open.snooze.tomorrow"); time.sleep(0.2)
    s = surface()
    return (ok, s["stage"], s["items"], ident in [x["id"] for x in rows("mac-main.list")])
case("one-choice-snoozes-the-mail", snooze_one, (True, "rest", 0, False))

# ---- more
def more():
    card(); open_card()
    before = rows("mac-notch.list")[0]["read"]
    click("mac-notch.open.markread"); time.sleep(0.12); read = rows("mac-notch.list")[0]["read"]
    unread = click("mac-notch.open.more.unread"); time.sleep(0.12); after = rows("mac-notch.list")[0]["read"]
    deleted = click("mac-notch.open.more.delete"); time.sleep(0.15)
    return (before, read, unread, after, deleted, surface()["items"])
case("more-marks-unread-and-deletes", more, (False, True, True, False, True, 0))

# ---- whose key it is: the notch takes a key only when the pointer is on it or it holds the keyboard
def taker():
    return rows("mac-keyevent.monitor")[0].get("taker", "")
def key(chord):
    try:
        c.key(chord); return True
    except Unreadable:
        return False
def key_not_the_notchs():
    card(); open_card()                    # opened by a click on the card: the pointer was never on the notch and no reply holds the keyboard
    takes = surface()["takes_keys"]
    key("U")                               # the main window's key: whatever it does there, the mail open in the notch stays unread
    return (takes, "by notch" in taker(), rows("mac-notch.list")[0]["read"], surface()["stage"])
case("a-key-with-the-pointer-away-is-not-the-notchs", key_not_the_notchs, (False, False, False, "open"))
def key_the_notchs():
    card(); open_card(); click("mac-notch.stage.hover")     # the pointer is on the notch
    takes = surface()["takes_keys"]; pressed = key("U")
    return (takes, pressed, "by notch" in taker(), rows("mac-notch.list")[0]["read"])
case("the-read-key-acts-with-the-pointer-on-the-notch", key_the_notchs, (True, True, True, True))
def leaving_ends_the_reply():
    card(); open_card(); click("mac-notch.open.reply"); time.sleep(0.15)
    started = len(rows("mac-notch.open.quickreply"))
    click("mac-notch.open.back"); time.sleep(0.15)          # the mail leaves the open state: the reply the notch started ends with it
    click("mac-notch.stage.hover"); click("mac-notch.list.row.1"); time.sleep(0.15)
    return (started, surface()["stage"], len(rows("mac-notch.open.quickreply")), len(rows("mac-notch.open.body")))
case("a-reply-ends-when-its-mail-leaves-the-notch", leaving_ends_the_reply, (1, "open", 0, 1))
reset()

print("%d checks, %d failed, %d not judged" % (total, bad, unjudged))
sys.exit(1 if bad or unjudged else 0)
