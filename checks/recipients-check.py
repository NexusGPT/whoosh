#!/usr/bin/env python3
"""The recipient fields of the composer offer the people of all mail by the first letters of a name (To, Cc, Bcc), with REAL keys.

Run it alone on a fresh hidden copy (scripts/launch-one.sh ... --fixture-core --rig-hidden). Judged:
  1. C then "n": the list under To has two lines or more, each line's name or address has a word that starts with "n", the first line is
     highlighted, and no line is an address nobody answers (noreply@);
  2. Down moves the highlight (when the list has two lines); Return makes the highlighted person a chip, empties the field and closes the list;
  3. "fa": a person the owner only wrote to (never a sender) is offered; Tab takes the highlighted line and the cursor stays in To;
  4. a whole address nobody has ("zed@nowhere-example.org"): no line is highlighted and Return takes it as typed;
  5. Cc (its key) then "ka": the list under Cc has a line; a click on its first line makes that person a Cc chip.
Red first (lead, 13 Nov 2026, picture of a new message on main157): "ni" in To offered nobody; only the person a reply answers and a whole address were completed.
"""
import os, re, sys, time
sys.path.insert(0, os.getcwd())
from journeys.lib.ctl import Ctl

c = Ctl().connect()

def rows(ax):
    try: return c.send("rows", ax).get("rows") or []
    except Exception: return []

def head(): return (rows("mac-composer.header") or [{}])[0]
def offered(f="to"): return rows("mac-composer.%s.completions" % f)
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

starts = lambda r, p: any(w.startswith(p) for w in re.split(r"[ .\-_@]+", ((r.get("name") or "") + " " + (r.get("address") or "")).lower()))

c.send("keyevent", "c"); keys("n")
lst = wait(offered)
judge("letters-of-a-name-list-people-under-to", len(lst) >= 2 and all(starts(r, "n") for r in lst) and lst[0].get("highlighted") is True,
      "%d lines %s" % (len(lst), [(r.get("name"), r.get("address"), r.get("highlighted")) for r in lst][:5]))
# an address nobody answers is not offered (probe on main159: "n" listed noreply@github-example.com and noreply@hostcloud-example.net among five)
judge("an-address-nobody-answers-is-not-offered", bool(lst) and not any(re.match(r"(no[-_.]?reply|mailer[-_.]?daemon)", (r.get("address") or "").lower()) for r in lst),
      "%s" % [r.get("address") for r in lst])

want = None
if len(lst) >= 2:
    c.send("keyevent", "down"); time.sleep(0.2)
    now = offered()
    want = next((r.get("address") for r in now if r.get("highlighted")), None)
    moved = [r.get("highlighted") for r in now][:2] == [False, True]
else:
    want, moved = (lst[0].get("address") if lst else None), None
c.send("keyevent", "return"); time.sleep(0.4)
h = head()
judge("return-takes-the-highlighted-person", want is not None and h.get("to") == [want] and h.get("to_input") == "" and not offered() and moved is not False,
      "wanted %r; to %r, field %r, lines left %d, down moved the highlight %s" % (want, h.get("to"), h.get("to_input"), len(offered()), moved))

keys("fa")
lst = wait(offered)
fay = next((r.get("address") for r in lst if "fay" in (r.get("address") or "")), None)
c.send("keyevent", "tab"); time.sleep(0.4)
h = head()
judge("a-person-only-written-to-is-offered-and-tab-takes-the-line", fay is not None and len(h.get("to") or []) == 2 and h.get("focused") == "to" and h.get("to_input") == "",
      "fay offered %r; to %r, cursor in %r, field %r" % (fay, h.get("to"), h.get("focused"), h.get("to_input")))

c.send("type", "zed@nowhere-example.org"); time.sleep(0.4)
lit = [r.get("highlighted") for r in offered()]
c.send("keyevent", "return"); time.sleep(0.4)
h = head()
judge("a-whole-address-is-taken-as-typed", not any(lit) and "zed@nowhere-example.org" in (h.get("to") or []),
      "highlighted lines %s; to %r, field %r" % (lit, h.get("to"), h.get("to_input")))

c.send("keyevent", "⇧⌘c"); time.sleep(0.4)
keys("ka")
lst = wait(lambda: offered("cc"))
first = lst[0].get("address") if lst else None
if lst: c.send("click", "mac-composer.cc.completions.0")
time.sleep(0.4)
h = head()
cc = (rows("mac-composer.header") or [{}])[0].get("cc")
judge("cc-offers-people-and-a-click-takes-the-line", first is not None and cc == [first],
      "first line %r; cc %r" % (first, cc))

c.send("keyevent", "⇧⌘,"); time.sleep(0.5)
for _ in range(2): c.send("keyevent", "escape"); time.sleep(0.3)
print("%d checks, %d failed, 0 not judged" % (total, bad))
sys.exit(1 if bad else 0)
