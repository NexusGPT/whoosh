#!/usr/bin/env python3
"""The keys typed right after a key that opens a place to type, with REAL key events and no wait: they are that place's words, never a command.

Run it alone on a fresh hidden copy (scripts/launch-one.sh ... --fixture-core --rig-hidden). Doors, in this order:
  search by its key (the preset's search chord), search by "/", the reply (R), a new message (C), a forward (F).
Judged per door: the letters are in the place's own text, in order; the mail under it is as it was (still there, still focused, not starred).
The pickers (H, L, V) have their own file, checks/pickerkeys-check.py. The key trace (`rows mac-keyevent.monitor`, field `trace`) is printed on a miss.
"""
import os, sys, time
sys.path.insert(0, os.getcwd())
from journeys.lib.ctl import Ctl

c = Ctl().connect()

def rows(ax):
    try: return c.send("rows", ax).get("rows") or []
    except Exception: return []

def first(ax): return (rows(ax) or [{}])[0]
def trace(): return (first("mac-keyevent.monitor").get("trace") or [None])[-1]
def state(): return c.send("uistate").get("state", {})

def mail():
    """The focused mail as a command would change it: how many mails the list holds, which one is focused, its star. Not its unread mark: opening a
    mail to answer it reads it, and that is the reply's own act."""
    rs = rows("mac-main.list")
    i = state().get("focus_index", 0)
    x = rs[i] if i < len(rs) else {}
    return (len(rs), x.get("id"), x.get("starred"))

def close(times=3):
    for _ in range(times):
        c.send("keyevent", "escape"); time.sleep(0.3)

DOORS = [
    ("search-by-its-key", "⌘k", "sup", lambda: first("mac-search.bar").get("text"), lambda: close(2)),
    ("search-by-slash", "/", "sup", lambda: first("mac-search.bar").get("text"), lambda: close(2)),
    ("reply", "r", "456", lambda: first("mac-main.quickreply.field").get("text"), lambda: (c.send("keyevent", "⇧⌘,"), time.sleep(0.4), close(2))),
    # a draft with words is never replaced by another (the composer's one-draft rule): each composer door discards its words before the next one opens
    ("new-message", "c", "ni", lambda: first("mac-composer.header").get("to_input"), lambda: (c.send("keyevent", "⇧⌘,"), time.sleep(0.4), close(2))),
    ("forward", "f", "an", lambda: first("mac-composer.header").get("to_input"), lambda: (c.send("keyevent", "⇧⌘,"), time.sleep(0.4), close(2))),
]
bad = 0
for name, opener, word, read, shut in DOORS:
    before = mail()
    c.send("keyevent", opener)
    lines = ["  open %s: %s" % (opener, trace())]
    for ch in word:
        c.send("keyevent", ch)
        lines.append("  key  %s: %s" % (ch, trace()))
    got = None
    for _ in range(20):
        got = read()
        if got == word: break
        time.sleep(0.1)
    shut()
    after = mail()
    ok = got == word and after == before
    print("%s first-letters-after-%s-are-its-words: words %r (wanted %r); mail before %s, after %s" % ("PASS" if ok else "FAIL", name, got, word, before, after))
    if not ok:
        bad += 1
        print("\n".join(lines))
print("%d checks, %d failed, 0 not judged" % (len(DOORS), bad))
sys.exit(1 if bad else 0)
