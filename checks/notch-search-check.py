#!/usr/bin/env python3
"""Search in the notch and the notch's key (owner's picks of 14 Nov 2026, docs/surfaces/mac-notch.md: 3A "in the search field"; the plan's "⌥M to
start. You change it in Settings. The same key closes the notch"), driven in the hidden test copy on the test engine:
   key       the notch's key opens the list with the cursor in the search field and the keyboard taken; the same key closes it and gives the
             keyboard back; the pointer alone never takes the keyboard; a click on the field does
   search    typed letters show in the field, the answer takes the list's place (matches, then related), in under 50 ms; no match says so; the
             clear mark gives the list back; the answer never joins the arrivals list
   keyboard  Down goes to the first mail, Up goes back to the field; a letter on a row goes to the field; Return opens the mail in the notch;
             the read and archive keys act on the row; Escape goes back one step at a time
   alerts    a card taken with the key answers Return, and "/" goes to search from a card and from an open mail
   whoosh    ⌘O opens the mail in Whoosh with the reply that was started, and the notch closes
   settings  the key shows in Settings, a key with two modifiers is refused in words, a new key is kept, Default puts the start key back
A hidden window gets no key press from the keyboard: the notch's key is run through its own act (mac-notch.stage.summon), the recorder through its
own door (mac-notch.stage.key.*). The owner's one press of the key from another app is the proof no test copy can give.
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core --rig-hidden sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/notch-search-check.py'
Mutants (each must turn a named case red, on a build of the broken copy): S1 NotchModel.summon never sets searchFocused ->
the-key-opens-the-list-with-the-cursor-in-the-field; S2 NotchModel.hover takes the keyboard -> the-pointer-alone-never-takes-the-keyboard;
S3 NotchSearch.show keeps the rest of the mail -> the-matches-come-before-the-related-mail; S4 NotchModel.moveRow ignores Up at the first mail ->
up-from-the-first-mail-goes-back-to-the-field; S5 NotchModel.back collapses from an open mail -> escape-goes-back-one-step; S6
NotchModel.openInWhoosh ends the reply -> open-in-whoosh-takes-the-started-reply; S7 NotchHotKey.set takes two modifiers ->
a-key-with-two-modifiers-is-refused; S8 NotchModel.adopt lets a found mail into the arrivals -> the-answer-never-joins-the-arrivals;
S9 NotchModel.keyboardLost returns at once -> a-click-in-another-app-closes-a-notch-taken-with-the-key."""
import os, sys, time
sys.path.insert(0, os.getcwd())
from journeys.lib.ctl import Ctl, Unreadable

c = Ctl().connect()
total = bad = unjudged = 0
WORD = "quote"            # the sample mail "Quote for the Q1 print run: 12,000 units" holds it

def case(name, got_fn, want):
    global total, bad, unjudged
    total += 1
    try:
        got = got_fn()
    except (Unreadable, KeyError, IndexError, TypeError, ValueError, StopIteration) as e:
        unjudged += 1
        print("NOT JUDGED %s: %s %s" % (name, type(e).__name__, str(e)[:220]), flush=True)
        return
    ok = got == want
    bad += not ok
    print("%s %s: got %r, wanted %r" % ("PASS" if ok else "FAIL", name, got, want), flush=True)

def click(ax):
    try:
        c.click(ax); return True
    except Unreadable:
        return False
def key(chord):
    try:
        c.key(chord); return True
    except Unreadable:
        return False
def rows(ax): return c.rows(ax)
def surface(): return rows("mac-notch")[0]
def search(): return rows("mac-notch.search")[0]
def results(): return rows("mac-notch.search.results")
def place(): return rows("mac-notch.hints")[0]["place"]
def reset(): click("mac-notch.stage.reset"); time.sleep(0.12)
def summon(): ok = click("mac-notch.stage.summon"); time.sleep(0.12); return ok
def typed(text):
    """Types, then waits until the answer stands still: the engine answers the same words twice (the word hits, then the full ranking)."""
    c.type(text); time.sleep(0.15)
    last, still = None, 0
    for _ in range(25):
        now = [(x["id"], x["part"]) for x in c.rows("mac-notch.search.results")] if c.rows("mac-notch.search") else None
        still = still + 1 if now == last else 0
        if still >= 3:
            return
        last = now; time.sleep(0.1)
def card(kind="vip"):
    reset(); click("mac-notch.stage.alert.%s.card" % kind); ok = click("mac-notch.stage.arrive.%s" % kind); time.sleep(0.12); return ok
def searching():
    """A fresh notch taken with its key, with the word typed: the answer is on screen."""
    reset(); summon(); typed(WORD)

# ---- the key
def key_opens():
    reset(); ok = summon(); s, f = surface(), search()
    return (ok, s["stage"], s["keyed"], f["focused"], f["keyboard"], f["text"], f["row_focus"], place())
case("the-key-opens-the-list-with-the-cursor-in-the-field", key_opens, (True, "list", True, True, True, "", None, "field"))
def key_closes():
    ok = summon(); s = surface()
    return (ok, s["stage"], s["keyed"])
case("the-same-key-closes-it-and-gives-the-keyboard-back", key_closes, (True, "rest", False))
def pointer_alone():
    reset(); click("mac-notch.stage.hover"); time.sleep(0.12); s, f = surface(), search()
    return (s["stage"], s["keyed"], f["focused"], f["keyboard"], place())
case("the-pointer-alone-never-takes-the-keyboard", pointer_alone, ("list", False, False, False, "pointer"))
def field_click():
    ok = click("mac-notch.search.field"); time.sleep(0.12); s, f = surface(), search()
    return (ok, s["keyed"], f["focused"], f["keyboard"])
case("a-click-on-the-field-takes-the-keyboard", field_click, (True, True, True, True))
def pointer_leaves():
    click("mac-notch.stage.leave"); time.sleep(0.15)       # the pointer leaves: the notch the person took with the keyboard stays
    return surface()["stage"]
case("a-notch-taken-with-the-keyboard-stays-when-the-pointer-leaves", pointer_leaves, "list")

def clicks_away():
    reset(); summon(); f1 = search()["keyboard"]
    ok = click("mac-notch.stage.resign"); time.sleep(0.12); s = surface()      # the panel stops being the key window: a click in another app
    return (f1, ok, s["stage"], s["keyed"])
case("a-click-in-another-app-closes-a-notch-taken-with-the-key", clicks_away, (True, True, "rest", False))

# ---- the search
def letters_show():
    searching(); f, r = search(), results()
    return (f["text"], f["active"], f["waiting"], f["results"] >= 1, f["matches"] >= 1, len(r) == f["results"], r[0]["part"], WORD in r[0]["subject"].lower(), place())
case("typed-letters-show-and-the-answer-takes-the-lists-place", letters_show, (WORD, True, False, True, True, True, "match", True, "answer"))
def matches_first():
    parts = [x["part"] for x in results()]
    return (parts == sorted(parts, key=lambda p: p != "match"), set(parts) <= {"match", "related"}, search()["matches"] == parts.count("match"))
case("the-matches-come-before-the-related-mail", matches_first, (True, True, True))
def speed():
    ms = search()["ms"]
    print("   the answer came %.1f ms after the last letter (load %.1f)" % (ms, os.getloadavg()[0]), flush=True)
    return ms < 50
case("the-answer-comes-in-under-50-ms", speed, True)
def never_joins():
    reset(); click("mac-notch.stage.arrive.needs_reply"); time.sleep(0.12)     # one arrival, a mail that does not hold the word
    before = [x["id"] for x in rows("mac-notch.list")]
    summon(); key("/"); typed(WORD); first = results()[0]["id"]   # the key takes the new mail's card; "/" goes to the search field
    key("Down"); key("Enter"); time.sleep(0.15)             # a found mail is open in the notch
    opened = surface()["stage"]
    key("Escape"); key("Escape"); time.sleep(0.15)          # back to the answer, then the words are emptied: the list is back
    after = [x["id"] for x in rows("mac-notch.list")]
    return (opened, first not in before, after == before, surface()["items"] == len(before), surface()["unread"])
case("the-answer-never-joins-the-arrivals", never_joins, ("open", True, True, True, 1))
def no_match():
    # no mail holds these letters: no match is listed (the engine may still list a mail it finds related by meaning, under its own word)
    reset(); summon(); typed("zzqqxxvv"); f = search()
    return (f["active"], f["waiting"], f["matches"], [x["part"] for x in results() if x["part"] != "related"])
case("letters-no-mail-holds-list-no-match", no_match, (True, False, 0, []))
def third_line():
    searching(); r = results()[0]
    return (bool(r["detail"].strip()), WORD in (r["subject"] + " " + r["detail"]).lower())
case("a-row-of-the-answer-says-its-passage", third_line, (True, True))
def band_back():
    searching(); before = rows("mac-notch.band")[0]["left"]
    ok = click("mac-notch.search.back"); time.sleep(0.12); f = search()
    return (before, ok, f["text"], f["active"], surface()["stage"])
case("the-band-leads-back-to-the-list", band_back, ("Latest", True, "", False, "list"))
def clear_mark():
    searching()
    ok = click("mac-notch.search.clear"); time.sleep(0.12); f = search()
    return (ok, f["text"], f["active"], f["focused"], surface()["stage"])
case("the-clear-mark-gives-the-list-back", clear_mark, (True, "", False, True, "list"))

# ---- the keyboard
def down():
    searching(); first = results()[0]["id"]
    ok = key("Down"); time.sleep(0.1); f = search()
    return (ok, f["focused"], f["row_focus"] == first, results()[0]["focused"], place())
case("down-goes-to-the-first-mail", down, (True, False, True, True, "row"))
def up():
    ok = key("Up"); time.sleep(0.1); f = search()
    return (ok, f["focused"], f["row_focus"], place())
case("up-from-the-first-mail-goes-back-to-the-field", up, (True, True, None, "answer"))
def letter_on_row():
    key("Down"); typed("s"); f = search()
    return (f["text"], f["focused"], f["row_focus"])
case("a-letter-on-a-row-goes-to-the-field", letter_on_row, (WORD + "s", True, None))
def enter_opens():
    searching(); first = results()[0]
    key("Down"); ok = key("Enter"); time.sleep(0.15)
    h = rows("mac-notch.open.header")[0]
    return (ok, surface()["stage"], h["sender"] == first["sender"], h["back"], len(rows("mac-notch.open.body")), len(rows("mac-notch.open.quickreply")))
case("return-opens-the-mail-in-the-notch", enter_opens, (True, "open", True, "Search", 1, 0))
def escape_steps():
    a = key("Escape"); time.sleep(0.12); s1, f1 = surface()["stage"], search()
    b = key("Escape"); time.sleep(0.12); s2, f2 = surface()["stage"], search()
    d = key("Escape"); time.sleep(0.12); s3 = surface()
    return (a, s1, f1["text"], f1["row_focus"] is not None, b, s2, f2["text"], f2["focused"], d, s3["stage"], s3["keyed"])
case("escape-goes-back-one-step", escape_steps, (True, "list", WORD, True, True, "list", "", True, True, "rest", False))
def return_in_field():
    searching(); first = results()[0]["id"]
    ok = key("Enter"); time.sleep(0.15)                      # the cursor is in the field: Return opens the answer's first mail
    return (ok, surface()["stage"], rows("mac-notch.open.body")[0]["id"] == first)
case("return-in-the-field-opens-the-first-mail", return_in_field, (True, "open", True))
def read_key():
    searching(); key("Down"); first = results()[0]
    ok = key("U"); time.sleep(0.12); now = [x for x in results() if x["id"] == first["id"]]
    return (ok, first["read"], [x["read"] for x in now], surface()["stage"])
case("the-read-key-acts-on-the-row", read_key, (True, False, [True], "list"))
def archive_key():
    searching(); key("Down"); before = results(); first = before[0]["id"]
    ok = key("E"); time.sleep(0.2); after = results(); f = search()
    inbox = [x["id"] for x in rows("mac-main.list")]
    # the keyboard is never on nothing: on the row that took the archived one's place, or back in the field when the answer is empty
    keeps = (f["row_focus"] == after[0]["id"] and not f["focused"]) if after else (f["row_focus"] is None and f["focused"])
    if not keeps:
        print("   after E: search %r, answer %r" % ({k: f[k] for k in ("focused", "row_focus", "results")}, [(x["id"], x["focused"]) for x in after]), flush=True)
    return (ok, first in [x["id"] for x in after], len(after) == len(before) - 1, keeps, first in inbox, surface()["stage"])
case("the-archive-key-archives-the-row-and-the-keyboard-stays-on-the-list", archive_key, (True, False, True, True, False, "list"))
def reply_key_on_row():
    searching(); key("Down"); first = results()[0]["id"]
    ok = key("R"); time.sleep(0.2)
    return (ok, surface()["stage"], [x.get("reply_to") for x in rows("mac-notch.open.quickreply")] == [first])
case("the-reply-key-opens-the-row-with-the-reply-started", reply_key_on_row, (True, "open", True))
def snooze_key_on_row():
    searching(); key("Down"); first = results()[0]["id"]
    ok = key("H"); time.sleep(0.2); s = rows("mac-notch.open.snooze")
    return (ok, surface()["stage"], rows("mac-notch.open.body")[0]["id"] == first, s[0]["open"] if s else None)
case("the-snooze-key-opens-the-row-with-its-choices", snooze_key_on_row, (True, "open", True, True))
def row_click():
    searching(); first = results()[0]["id"]
    ok = click("mac-notch.search.row.1"); time.sleep(0.15)
    return (ok, surface()["stage"], rows("mac-notch.open.body")[0]["id"] == first)
case("a-click-on-a-result-opens-it", row_click, (True, "open", True))

# ---- alerts taken with the key
def card_keys():
    delivered = card(); ident = surface()["alert"]
    took = summon(); s = surface()                          # the card stays; the keyboard is the notch's
    opened = key("Enter"); time.sleep(0.15)
    return (delivered, took, s["stage"], s["keyed"], opened, surface()["stage"], rows("mac-notch.open.body")[0]["id"] == ident)
case("a-card-taken-with-the-key-answers-return", card_keys, (True, True, "drop", True, True, "open", True))
def slash_from_mail():
    ok = key("/"); time.sleep(0.12); s, f = surface(), search()
    return (ok, s["stage"], f["focused"], f["keyboard"])
case("slash-goes-to-search-from-an-open-mail", slash_from_mail, (True, "list", True, True))
def slash_from_card():
    card(); click("mac-notch.stage.hover")                  # the pointer on the notch: the list; a card needs the key
    card(); summon()
    ok = key("/"); time.sleep(0.12); s, f = surface(), search()
    return (ok, s["stage"], f["focused"])
case("slash-goes-to-search-from-a-card", slash_from_card, (True, "list", True))
def card_archive_closes():
    card(); ident = surface()["alert"]; summon()
    ok = key("E"); time.sleep(0.15); s = surface()
    return (ok, s["stage"], s["keyed"], ident in [x["id"] for x in rows("mac-main.list")])
case("a-card-taken-with-the-key-closes-once-its-mail-is-archived", card_archive_closes, (True, "rest", False, False))

# ---- Open in Whoosh
def hand_over():
    card(); ident = surface()["alert"]
    click("mac-notch.arrival.open"); click("mac-notch.open.reply"); time.sleep(0.15); typed("Thursday works.")
    click("mac-notch.stage.hover")                          # the pointer is on the notch: ⌘O is the notch's
    ok = key("⌘O"); time.sleep(0.25); s = surface()
    field = rows("mac-main.quickreply.field")
    text = (field[0].get("text") or field[0].get("draft") or "") if field else ""
    return (ok, s["stage"], s["handed_over"] == ident, c.uistate().get("surface"), text.endswith("Thursday works."))
case("open-in-whoosh-takes-the-started-reply", hand_over, (True, "rest", True, "mac-reader", True))

# ---- the key in Settings
KEY = "mac-settings-general.notch.key"
def key_row(): return rows(KEY)[0]
case("the-start-key-is-option-m", lambda: (key_row()["key"], key_row()["recording"]), ("⌥M", False))
def two_modifiers():
    click(KEY + ".change"); rec = key_row()["recording"]
    click("mac-notch.stage.key.shift-command-n"); r = key_row()
    return (rec, r["key"], r["recording"], r["refusal"])
case("a-key-with-two-modifiers-is-refused", two_modifiers, (True, "⌥M", True, "Use Command, Option or Control with one other key."))
def new_key():
    click("mac-notch.stage.key.control-n"); r = key_row()
    return (r["key"], r["recording"], r["refusal"])
case("a-new-key-is-kept", new_key, ("⌃N", False, None))
def default_back():
    ok = click(KEY + ".default"); r = key_row()
    return (ok, r["key"], r["recording"])
case("default-puts-the-start-key-back", default_back, (True, "⌥M", False))
def change_then_cancel():
    click(KEY + ".change"); a = key_row()["recording"]; click(KEY + ".change"); r = key_row()
    return (a, r["recording"], r["key"])
case("change-pressed-again-leaves-the-key", change_then_cancel, (True, False, "⌥M"))
reset()

print("%d checks, %d failed, %d not judged" % (total, bad, unjudged))
sys.exit(1 if bad or unjudged else 0)
