#!/usr/bin/env python3
"""The reply box's Cc line: people offered by name, and a word that is not an address stops the send with its sentence.

Run it alone on a fresh hidden copy (scripts/launch-one.sh ... --fixture-core --rig-hidden). Judged:
  1. R, the header rows opened, "karen" on the Cc line, two letters in the reply, the send key: nothing is sent (the reply still edits), the rows
     say "karen is not an address." and the words of the reply are kept;
  2. a fresh reply, "n" on the Cc line: the list has two lines or more, none an address nobody answers; a click on its first line puts that
     person's address on the line, with a comma after it, and the list closes;
  3. the send key then starts the send (the reply leaves the editing state) and no refusal shows.
Red first (lead, 13 Nov 2026, read in code on main159): the Cc line offered nobody, and a word with no '@' was dropped at the send without a word.
The Cc line takes its letters through the socket's `type` here (the test door gives the line no cursor); the reply's own letters and the send key are real keys.
"""
import os, re, sys, time
sys.path.insert(0, os.getcwd())
from journeys.lib.ctl import Ctl

c = Ctl().connect()

def rows(ax):
    try: return c.send("rows", ax).get("rows") or []
    except Exception: return []

def first(ax): return (rows(ax) or [{}])[0]
def head(): return first("mac-main.quickreply.header")
def field(): return first("mac-main.quickreply.field")
def offers(): return rows("mac-main.quickreply.header.cc.completions")
def keys(text):
    for ch in text: c.send("keyevent", ch)
def wait(fn, tries=25):
    v = fn()
    for _ in range(tries):
        if v: break
        time.sleep(0.1); v = fn()
    return v

bad = total = 0
def judge(name, ok, facts):
    global bad, total
    total += 1; bad += 0 if ok else 1
    print("%s %s: %s" % ("PASS" if ok else "FAIL", name, facts))

def open_reply_with_cc():
    c.send("keyevent", "r")
    wait(lambda: field().get("phase") == "editing")
    c.send("click", "mac-main.quickreply.header.toggle"); time.sleep(0.3)
    c.send("click", "mac-main.quickreply.header.cc"); time.sleep(0.2)

open_reply_with_cc()
c.send("type", "karen"); time.sleep(0.3)
keys("ok"); time.sleep(0.3)
c.send("keyevent", "⌘return"); time.sleep(0.6)
h, f = head(), field()
judge("a-word-that-is-not-an-address-stops-the-send", f.get("phase") == "editing" and "karen" in (h.get("refusal") or "") and f.get("text") == "ok",
      "phase %r, refusal %r, reply words %r, cc %r" % (f.get("phase"), h.get("refusal"), f.get("text"), h.get("cc")))

c.send("keyevent", "⇧⌘,"); time.sleep(0.5)
for _ in range(2): c.send("keyevent", "escape"); time.sleep(0.3)
open_reply_with_cc()
c.send("type", "n"); time.sleep(0.4)
lst = wait(offers)
want = lst[0].get("address") if lst else None
nobody = [r.get("address") for r in lst if re.match(r"(no[-_.]?reply|mailer[-_.]?daemon)", (r.get("address") or "").lower())]
if lst: c.send("click", "mac-main.quickreply.header.cc.completions.0")
time.sleep(0.4)
h = head()
judge("cc-offers-people-and-a-click-takes-the-line", len(lst) >= 2 and not nobody and h.get("cc") == "%s, " % want and not offers(),
      "%d lines %s; cc %r; lines left %d" % (len(lst), [r.get("address") for r in lst], h.get("cc"), len(offers())))

keys("ok"); time.sleep(0.3)
c.send("keyevent", "⌘return"); time.sleep(0.8)
f = field()
sent = (not f) or f.get("phase") not in ("editing", None)
judge("the-send-starts-with-the-chosen-person-in-cc", sent and not (head().get("refusal") or ""),
      "phase %r, refusal %r" % (f.get("phase") if f else "closed", head().get("refusal")))
c.send("keyevent", "z"); time.sleep(0.5)
for _ in range(2): c.send("keyevent", "escape"); time.sleep(0.3)
print("%d checks, %d failed, 0 not judged" % (total, bad))
sys.exit(1 if bad else 0)
