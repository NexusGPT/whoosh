#!/usr/bin/env python3
"""The three pickers over the list (L label, H snooze, V move) with REAL key events, from the first key of a launch.

Run it alone on a fresh hidden copy (scripts/launch-one.sh ... --fixture-core --rig-hidden): round 1 is the first keys the app ever gets.
Each picker, twice: the opening key, then letters at once (no wait). Judged per picker and round:
  - every letter is in the picker's words, none ran as a command;
  - the mail under the picker is as it was (not starred, not marked read);
  - Esc with the cursor in the picker's field closes the picker WITH its action (no overlay, no picker row);
  - after Esc the list has its keys back (J moves the selection).
Red first (lead, 13 Nov 2026): on main155 and main156 round 1 of the label picker typed 'p' of 'supp' and the mail came out starred and read (the key
hook was installed only after a key no command takes), and Esc in the label field left the action behind: the next keys were taken by a picker nobody
saw (typed 'phnext'). The key trace (`rows mac-keyevent.monitor`, field `trace`) is printed with each key.
"""
import os, sys, time
sys.path.insert(0, os.getcwd())
from journeys.lib.ctl import Ctl

c = Ctl().connect()

def mon():
    r = (c.send("rows", "mac-keyevent.monitor").get("rows") or [{}])[0]
    t = r.get("trace") or []
    return "taker=%s | trace=%s" % (r.get("taker"), t[-1] if t else None)

def picker():
    r = c.send("rows", "mac-main.picker").get("rows") or []
    return r[0] if r else None

def move_query():
    r = c.send("rows", "mac-labels.move.targets").get("rows") or []
    return len(r)

def state():
    return c.send("uistate").get("state", {})

def row():
    rows = c.send("rows", "mac-main.list").get("rows") or []
    i = state().get("focus_index", 0)
    x = rows[i] if i < len(rows) else {}
    return {k: x.get(k) for k in ("id", "unread", "starred", "pinned")}

bad = total = 0
for rnd in (1, 2):
    for key, kind, word in (("l", "label", "supp"), ("h", "snooze", "next"), ("v", "move", "supp")):
        total += 1
        before, all_targets = row(), None
        c.send("keyevent", key)
        lines = ["  open %s: %s" % (key, mon())]
        if kind == "move": all_targets = move_query()
        for ch in word:
            c.send("keyevent", ch)
            lines.append("  key  %s: %s" % (ch, mon()))
        time.sleep(0.5)
        if kind == "move":
            narrowed = move_query()
            typed_ok, got = (narrowed < (all_targets or 0)), "%s targets of %s" % (narrowed, all_targets)
        else:
            p = picker()
            got = p.get("typed") if p else None
            typed_ok = got == word and (p or {}).get("kind") == kind
        untouched = row() == before
        c.send("keyevent", "escape"); time.sleep(0.4)
        lines.append("  esc   : %s" % mon())
        closed = state().get("overlay") in (None, "") and picker() is None
        at = state().get("focus_index", 0)
        c.send("keyevent", "j"); time.sleep(0.25)
        moved = state().get("focus_index", 0) == at + 1
        lines.append("  j     : %s" % mon())
        c.send("keyevent", "k"); time.sleep(0.25)
        ok = typed_ok and untouched and closed and moved
        print("%s round-%d-%s-picker-owns-its-keys: words %r (wanted %r); mail untouched %s; closed by Esc %s; J moves the list after %s" % (
            "PASS" if ok else "FAIL", rnd, kind, got, word, untouched, closed, moved))
        if not ok:
            bad += 1
            print("\n".join(lines))
print("%d checks, %d failed, 0 not judged" % (total, bad))
sys.exit(1 if bad else 0)
