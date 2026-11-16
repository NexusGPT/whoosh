#!/usr/bin/env python3
"""Notch alerts by kind of mail (owner's picks of 14 Nov 2026, docs/surfaces/mac-notch.md "1B 2A 3A"), driven in the hidden test copy on the test engine:
   start    the engine's start values are the picked set 1B, and the notch holds the same
   arrive   one arrival of each of the 6 kinds, through the real delivery door, shows the style its setting says: the 6 start styles, then each
            kind in each of the 5 values (30 arrivals)
   shapes   the light carries the account's colour and the unread wings; the line says who and what and a click opens the mail; a smaller alert
            under a card joins the list and leaves the card
   stay     an alert set to 5 seconds is gone after 6; one set to "until I act" is still there after 6
   sound    with Sound on, an alert asks for its sound once (the hidden copy plays nothing)
   settings every alert control of Settings > Notifications is clicked once and its value is read back from the engine
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core --rig-hidden sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/notch-alerts-check.py'
Mutants (each must turn a named case red; run on a build of the broken copy): A1 NotchFeature.deliver passes `style: .card` whatever the setting ->
arrive-primary-start; A2 NotchModel.show lets a smaller alert replace a larger one (`guard size >= now` removed) -> a-line-under-a-card-joins-the-list;
A3 NotchRoot's hold task sleeps NOTCH_DROP_HOLD_S whatever the stay -> stay-5s-gone-after-6s; A4 GeneralStore.setAlert saves the start values ->
settings-primary-opened."""
import os, sys, time
sys.path.insert(0, os.getcwd())
from journeys.lib.ctl import Ctl, Unreadable

c = Ctl().connect()
P = "mac-settings-general"
KINDS = ["sign_in_codes", "vip", "needs_reply", "updates", "promotions", "primary"]
STYLES = ["nothing", "light", "line", "card", "opened"]
STAGE = {"nothing": "rest", "light": "light", "line": "line", "card": "drop", "opened": "open"}
START = {"sign_in_codes": "card", "vip": "card", "needs_reply": "card", "updates": "light", "promotions": "nothing", "primary": "line"}
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
def reset(): c.click("mac-notch.stage.reset"); time.sleep(0.12)
def engine(field): return c.core("notch_alerts/%s" % field)
def arrive(kind): ok = click("mac-notch.stage.arrive.%s" % kind); time.sleep(0.12); return ok

# ---- start: the engine's values are the picked set, and the notch's copy agrees after a reset
reset()
case("start-values-of-the-engine", lambda: {k: engine(k) for k in KINDS}, START)
case("start-stay-sound-pictures", lambda: (engine("stay"), engine("sound"), engine("full_mail_loads_pictures")), ("10s", False, True))
case("start-the-notch-holds-the-same", lambda: {k: c.rows("mac-notch.alerts")[0][k] for k in KINDS}, START)

# ---- arrive: each kind in its start style, through the real delivery door
for k in KINDS:
    reset()
    def got(k=k):
        delivered = arrive(k); s = surface()
        return (delivered, s["stage"], s["items"], s["alert_kind"] if STAGE[START[k]] in ("light", "line", "drop") else None)
    case("arrive-%s-start" % k, got, (True, STAGE[START[k]], 1, k if STAGE[START[k]] in ("light", "line", "drop") else None))

# ---- arrive: every kind in every value (the setting is saved through the engine by the stage click, then read back)
for k in KINDS:
    for st in STYLES:
        reset()
        def got(k=k, st=st):
            saved = click("mac-notch.stage.alert.%s.%s" % (k, st))
            delivered = arrive(k); s = surface()
            return (saved, engine(k), delivered, s["stage"], s["items"])
        case("arrive-%s-as-%s" % (k, st), got, (True, st, True, STAGE[st], 1))

# ---- shapes
reset()
def light():
    click("mac-notch.stage.alert.primary.light"); arrive("primary")
    s = surface(); row = c.rows("mac-notch.light")
    return (s["stage"], s["wings"], s["unread"], len(row), bool(row and str(row[0].get("colour", "")).startswith("#")), c.rows("mac-notch.band"))
case("a-light-shows-the-colour-and-the-unread-wings", light, ("light", True, 1, 1, True, []))

reset()
def line():
    arrive("primary")            # the start style of Primary is one line
    s = surface(); row = c.rows("mac-notch.line")[0]; band = c.rows("mac-notch.band")[0]; first = c.rows("mac-notch.list")[0]
    return (s["stage"], s["width"], row["sender"] == first["sender"], row["subject"] == first["subject"], (band["left"], band["right"]) == (row["sender"], row["subject"]), row["key"])
case("a-line-says-who-and-what", line, ("line", 520.0, True, True, True, "Enter"))
def line_open():
    opened = click("mac-notch.line.open"); s = surface()
    return (opened, s["stage"], c.rows("mac-notch.open.header")[0]["sender"] == c.rows("mac-notch.list")[0]["sender"])
case("a-click-on-the-line-opens-the-mail", line_open, (True, "open", True))

reset()
def under():
    arrive("vip"); first = surface()        # a card
    arrive("primary"); second = surface()   # a line arrives under it
    return (first["stage"], first["alert"] is not None, second["stage"], second["alert"] == first["alert"], second["items"])
case("a-line-under-a-card-joins-the-list", under, ("drop", True, "drop", True, 2))

reset()
def over():
    arrive("primary"); first = surface()    # a line
    arrive("vip"); second = surface()       # a card arrives over it
    return (first["stage"], second["stage"], second["alert"] != first["alert"], second["alert_kind"], second["items"])
case("a-card-over-a-line-takes-its-place", over, ("line", "drop", True, "vip", 2))

reset()
def list_stays():
    arrive("updates"); click("mac-notch.stage.hover"); before = surface()["stage"]
    arrive("vip"); s = surface()
    return (before, s["stage"], s["items"])
case("an-alert-leaves-the-open-list-alone", list_stays, ("list", "list", 2))

# ---- stay
reset()
def stay5():
    saved = click("mac-notch.stage.stay.5s"); arrive("vip"); at_once = surface()["stage"]
    time.sleep(6.0)
    return (saved, engine("stay"), at_once, surface()["stage"])
case("stay-5s-gone-after-6s", stay5, (True, "5s", "drop", "rest"))
reset()
def stay_until():
    saved = click("mac-notch.stage.stay.until-acted"); arrive("vip"); time.sleep(6.0)
    return (saved, engine("stay"), surface()["stage"])
case("stay-until-i-act-still-there-after-6s", stay_until, (True, "until-acted", "drop"))

# ---- sound: the hidden copy counts the ask
reset()
def sound():
    silent = (arrive("vip"), surface()["sounds"])
    click("mac-notch.stage.reset"); time.sleep(0.12)
    click("%s.alerts.sound" % P); on = engine("sound")
    arrive("vip"); one = surface()["sounds"]
    click("mac-notch.stage.alert.updates.nothing"); arrive("updates")     # an alert that shows nothing asks for no sound
    return (silent, on, one, surface()["sounds"])
case("sound-on-asks-once-per-alert-shown", sound, ((True, 0), True, 1, 1))

# ---- settings: every alert control of Settings > Notifications, clicked once, its value read back from the engine
reset()
for k in KINDS:
    for st in STYLES:
        def got(k=k, st=st):
            clicked = click("%s.alerts.%s.%s" % (P, k, st))
            row = [r for r in c.rows("%s.alerts" % P) if r["kind"] == k][0]
            return (clicked, engine(k), row["style"])
        case("settings-%s-%s" % (k, st), got, (True, st, st))
for word in ("5s", "10s", "until-acted"):
    case("settings-stay-%s" % word, lambda word=word: (click("%s.alerts.stay.%s" % (P, word)), engine("stay"), c.rows("%s.settings" % P)[0]["alert_stay"]), (True, word, word))
def switch(field, ax, key):
    before = engine(field); clicked = click(ax); after = engine(field); shown = c.rows("%s.settings" % P)[0][key]
    click(ax)
    return (clicked, after == (not before), shown == after, engine(field) == before)
case("settings-sound-switch", lambda: switch("sound", "%s.alerts.sound" % P, "alert_sound"), (True, True, True, True))
case("settings-pictures-switch", lambda: switch("full_mail_loads_pictures", "%s.alerts.pictures" % P, "alert_pictures"), (True, True, True, True))
case("settings-legend-shows-the-four-styles", lambda: [r["ax"].rsplit(".", 1)[1] for r in c.rows("%s.alerts.legend" % P)], ["light", "line", "card", "opened"])
case("settings-a-kind-set-here-reaches-the-next-arrival", lambda: (click("%s.alerts.primary.card" % P), click("mac-notch.stage.arrive.primary"), time.sleep(0.12), surface()["stage"])[3], "drop")
reset()

print("%d checks, %d failed, %d not judged" % (total, bad, unjudged))
sys.exit(1 if bad or unjudged else 0)
