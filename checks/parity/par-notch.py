#!/usr/bin/env python3
"""par-notch: every GotMail notch row (research/gotmail.md "## Notch feature") driven on the running rig copy.

Inputs the socket's `fixture` verbs will serve (docs/lanes/notch.md N1) come through the notch's own stage clicks
(`mac-notch.stage.*`), which call the same model entries the real inputs call. The clipboard and the window's
sharing state are read from the host (the test copy's private clipboard board, CGWindowList), never from the app's own report.
The Mac's own clipboard is never written or read: only its change count is read, to prove the test copy left it alone.
Step ids J6.11 to J6.38: the harness reads mutant labels only in the J<n>.<m> form (journeys/lib/harness.py:113).
Round 1 of look A (board a, Continuum) adds J6.31 to J6.38: the rest wings, the band, Latest | Unread and the end mark,
the open mail's header, back chevron and keys, the code block's Copy, Open in Whoosh, the shape's widths read from the
shot's own frames (CONTROL-SOCKET delta 46), and the file drop reaching the composer (N4 closed).
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/par-notch.py'
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0.
Round 6 (J6.41 to J6.46: the arrival's time, the pill in the core, Archive in the inbox, the window's sharing type, the
no-notch panel and the dropped file, each driven through the fixture's own verbs):
  python3 checks/parity/par-notch.py --selftest   the round-6 checks against a model of the notch and the core, clean (all
                                                  green), then once per mutant (each must go red on its own check)
"""
import json
import os
import re
import shutil
import socketserver
import subprocess
import sys
import tempfile
import threading
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, print_cases                # noqa: E402
from lib.fixture import const, FX                                    # noqa: E402
from lib.ctl import Ctl                                              # noqa: E402
from lib.harness import Journey                                      # noqa: E402

ID = "par-notch"
TITLE = "The notch, every GotMail row"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "new mail grows out of the notch with its code; every notch act and setting does what GotMail's does"
N, LIST = "mac-notch", "mac-notch.list"
CODE, NEXT = "msg-j6-code", "msg-j6-next"
EVERYTHING = "That's everything"
UNDO_S = const("UNDO_SEND_DEFAULT_S")
HOLD_S = const("NOTCH_DROP_HOLD_S")
J6_FIXTURE = os.path.join(ROOT, "fixtures", "tests-j6.json")
FAILING_LINE = FX["states"]["sync"]["failing_line"]


# The clipboard of a hidden test copy is a private board (Sources/App/HiddenClipboard.swift carries the same name): this file reads and writes
# that board, never the Mac's own clipboard. Of the Mac's own clipboard it reads one number, the change count, to prove no copy landed there.
RIG_CLIP = "io.nexus.whoosh.rig.clipboard"
_JXA_GET = ('function run(a) { ObjC.import("AppKit"); var s = $.NSPasteboard.pasteboardWithName(a[0]).stringForType("public.utf8-plain-text");'
            ' return s.isNil() ? "" : ObjC.unwrap(s); }')
_JXA_SET = ('function run(a) { ObjC.import("AppKit"); var pb = $.NSPasteboard.pasteboardWithName(a[0]); pb.clearContents;'
            ' return pb.setStringForType($(a[1]), "public.utf8-plain-text") ? "ok" : "refused"; }')
_JXA_COUNT = 'function run(a) { ObjC.import("AppKit"); return String($.NSPasteboard.generalPasteboard.changeCount); }'


def _jxa(script, *argv):
    r = subprocess.run(["osascript", "-l", "JavaScript", "-e", script, *argv], capture_output=True, text=True, timeout=10)
    if r.returncode != 0:
        raise OSError("osascript: %s" % r.stderr.strip()[:200])
    return r.stdout[:-1] if r.stdout.endswith("\n") else r.stdout


def _real_clip():
    return _jxa(_JXA_GET, RIG_CLIP)


def _real_setclip(text):
    if _jxa(_JXA_SET, RIG_CLIP, text) != "ok":
        raise OSError("the test copy's clipboard refused the text")


def _real_owner_count():
    """How many times the Mac's own clipboard changed since login: a number, never its content."""
    return int(_jxa(_JXA_COUNT))


def _rig_pids():
    """The process of the test copy under this drive: its program sits in the drive's own folder ($WHOOSH_SOCKET_DIR/rig)."""
    d = os.environ.get("WHOOSH_SOCKET_DIR", "")
    if not d:
        return []
    out = subprocess.run(["pgrep", "-f", os.path.join(d, "rig", "Whoosh.app", "Contents", "MacOS", "Whoosh")], capture_output=True, text=True, timeout=10)
    return [int(x) for x in out.stdout.split() if x.isdigit()]


def _real_cg():
    # The window server lists the windows of every process: the owner's own Whoosh has a window named "Notch" too once a mail reached it
    # (14 Nov 2026, main177: seven cases read [0, 0] for [0], the owner's notch beside the test copy's). Only the test copy's windows count.
    js = ('ObjC.import("CoreGraphics");var l=ObjC.deepUnwrap(ObjC.castRefToObject($.CGWindowListCopyWindowInfo('
          '$.kCGWindowListOptionAll,0)));JSON.stringify(l.filter(function(w){return w.kCGWindowName=="Notch"})'
          '.map(function(w){return {s:w.kCGWindowSharingState,y:w.kCGWindowBounds.Y,pid:w.kCGWindowOwnerPID}}))')
    out = subprocess.run(["osascript", "-l", "JavaScript", "-e", js], capture_output=True, text=True, timeout=20)
    found = json.loads(out.stdout or "[]")
    pids = _rig_pids()
    return [{"s": w["s"], "y": w["y"]} for w in found if not pids or w.get("pid") in pids]


def _real_screen_top():
    """The window server's Y of the top edge of the display the notch targets. The window server counts from the top of the first display;
    a second display set higher or lower than it has another top. (The -551 of 15 Nov 2026 was not such a display: see `_real_lock_scale`.)
    The display is the app's own choice
    (NotchGeometry.screen: the first with a camera housing, else the main one), read here from the host, not from the app."""
    js = ('ObjC.import("AppKit");var s=$.NSScreen.screens;var o=[];for(var i=0;i<s.count;i++){var c=s.objectAtIndex(i);var f=c.frame;'
          'o.push({y:f.origin.y,h:f.size.height,inset:c.safeAreaInsets.top})};JSON.stringify(o)')
    out = subprocess.run(["osascript", "-l", "JavaScript", "-e", js], capture_output=True, text=True, timeout=20)
    screens = json.loads(out.stdout or "[]")
    if not screens:
        raise RuntimeError("the host lists no display")
    target = next((c for c in screens if c["inset"] > 0), screens[0])
    return int(round(screens[0]["h"] - (target["y"] + target["h"])))


_OWN_FRAMES = set()   # (width, height) of every window and panel the app listed during this run


def _note_frames(s):
    """The app's own list of its windows now, each size remembered for `_real_lock_scale`. A copy that lists none gives []."""
    try:
        frames = s.ctl.winframes()
    except Exception as e:                                           # noqa: BLE001  a dry run or a copy that lists no frame: nothing to remember
        return [{"error": "%s: %s" % (type(e).__name__, e)}]
    for f in frames:
        if isinstance(f, dict) and f.get("kind") in ("window", "panel") and f.get("w") and f.get("h"):
            _OWN_FRAMES.add((float(f["w"]), float(f["h"])))
    return frames


def _real_lock_scale(s):
    """-> (k, cy): how the window server lists this copy's windows against where the app put them. Under the LOCK SCREEN the window server lists every
    window scaled about the centre of its display (16 Nov 2026, the screen locked, one 1512x982 display, three windows of the test copy measured: 0.9
    about (756, 491); the panel parked at 6000 read 5449, so J6.25 and J6.46 read -551 for 0; the same -551 on 15 Nov was first taken for a second
    display set higher). k is the window server's width of the copy's main window over the app's own width for it; cy is the window server's Y of the
    centre of the display the notch targets. (1.0, 0.0) when the two widths agree or one of them cannot be read: nothing is undone then."""
    frames = _note_frames(s)
    # every window the app said it has in this run, widest first (the widest gives the most exact factor). 17 Nov 2026: at J6.25 and J6.46 the app
    # lists only the notch's panel (616 x 600, which the window server rounds to 556 x 541: 0.9026, 16 points off at the parked 6000), so the main
    # window's size is kept from the earlier steps (`fresh`), and a window is matched by its SHAPE, not its title (the title is the open mail's subject).
    owns = sorted(_OWN_FRAMES, reverse=True)
    js = ('ObjC.import("CoreGraphics");ObjC.import("AppKit");var l=ObjC.deepUnwrap(ObjC.castRefToObject($.CGWindowListCopyWindowInfo('
          '$.kCGWindowListOptionAll,0)));var s=$.NSScreen.screens;var o=[];for(var i=0;i<s.count;i++){var c=s.objectAtIndex(i);var f=c.frame;'
          'o.push({y:f.origin.y,h:f.size.height,inset:c.safeAreaInsets.top})};JSON.stringify({w:l.filter(function(w){return '
          'w.kCGWindowBounds.Width>=200}).map(function(w){return {w:w.kCGWindowBounds.Width,h:w.kCGWindowBounds.Height,pid:w.kCGWindowOwnerPID}}),s:o})')
    out = subprocess.run(["osascript", "-l", "JavaScript", "-e", js], capture_output=True, text=True, timeout=20)
    got = json.loads(out.stdout or "{}")
    pids = _rig_pids()
    screens = got.get("s") or []
    mine = [(float(w["w"]), float(w["h"])) for w in got.get("w", []) if w.get("w") and w.get("h") and (not pids or w.get("pid") in pids)]
    # a window of the copy whose width and height are one of the app's own windows, both by one same factor: that factor is the scale
    pair = next(((cw, ow) for ow, oh in owns for cw, ch in mine if 0.5 <= cw / ow <= 1.05 and abs(cw / ow - ch / oh) < 0.01), None)
    if os.environ.get("PAR_NOTCH_SCALE_LOG"):   # a lead's probe: what this read saw, one line per call
        with open(os.environ["PAR_NOTCH_SCALE_LOG"], "a", encoding="utf-8") as f:
            f.write(json.dumps({"frames": frames, "pids": pids, "mine": mine, "pair": pair, "screens": screens, "err": out.stderr[:200]}) + "\n")
    if not screens:
        return 1.0, 0.0
    if pair is None:   # said in the run's output: with no window of the app's own shape the read cannot tell whether the window server scales
        print("par-notch: no window of this copy has the shape of one of the app's own windows %s: the copy's windows are %s" % (owns, mine), flush=True)
        return 1.0, 0.0
    seen, own = pair
    if abs(seen / own - 1.0) < 0.002:
        return 1.0, 0.0
    target = next((c for c in screens if c["inset"] > 0), screens[0])
    k, cy = seen / own, (screens[0]["h"] - (target["y"] + target["h"])) + target["h"] / 2.0
    # said in the run's own output: a reader of the log sees that this read was taken under a scaled window server (a locked screen), and by how much
    print("par-notch: the window server lists this copy's windows scaled by %.3f about y=%.0f (main window %s for %s): undone in the panel's read" % (k, cy, seen, own), flush=True)
    return k, cy


# What the checks read from the host Mac (the clipboard, the window server, the displays). --selftest swaps them for the model's.
HOST = {"clip": _real_clip, "setclip": _real_setclip, "ownercount": _real_owner_count, "cg": _real_cg, "top": _real_screen_top, "scale": _real_lock_scale}
MODEL_SCREEN_TOP = -551      # the model's display sits above the first one: a check that reads the panel against 0 is red on the model
MODEL_LOCK = (0.9, MODEL_SCREEN_TOP + 491.0)   # the model's screen is locked: its window server lists a window scaled by 0.9 about the display's centre,
                                               # so a measure that does not undo the scale is red on the model (49 for 0)
FAST = "--selftest" in sys.argv        # the model answers at once: no beat to wait for
SLEEP = time.sleep


def rows(s, ax):
    return s.ctl.rows(ax)


def texts(s, ax, key="text"):
    return [r.get(key) for r in s.ctl.rows(ax)]


def ids(s):
    return [r["id"] for r in s.ctl.rows(LIST)]


def surface(s, key):
    return s.ctl.rows(N)[0][key]


def clip(s=None):
    if s is not None:
        s.ctl.rows("mac-notch")   # dry run: raises, the check is not judged
    return HOST["clip"]()


def cg_notch(s):
    s.ctl.rows("mac-notch")       # dry run: raises, the check is not judged
    return HOST["cg"]()


def panel_tops(s):
    """How far each notch window's top edge is from the top edge of its display: 0 is at the top. A rig copy draws its panel `parked`
    points below the display (owner's fork, parkedIfTesting, NOTCH_RIG_PARK_PT): the window server's Y less that offset, less the display's
    own top, is where the panel sits on the display it targets."""
    wins = cg_notch(s)
    parked, top = int(s.ctl.rows(N)[0].get("parked", 0)), HOST["top"]()
    k, cy = HOST["scale"](s)             # a locked screen: the window server's Y is scaled about the display's centre; undone here
    return [int(round(cy + (w["y"] - cy) / k)) - parked - top for w in wins]


def fresh(s):
    _note_frames(s)             # the main window's size, for the scale of a sleeping display (`_real_lock_scale`)
    s.click("mac-notch.stage.reset")
    s.click("dock.whoosh")      # the main window holds the keys again: an earlier step that opened Settings left `type` going to its fields (main111: J6.23 typed into Settings)




def shape_w(name):
    """The shape's frame: the stage's width plus its two shoulders, both docs/CONSTANTS.md rows (NOTCH_*). Read inside the
    check, so a missing row fails that check (KeyError) instead of the whole file."""
    return const(name) + 2 * const("NOTCH_SHOULDER_R")


def frame(s, name, ax):
    """One frame read in the same main-thread turn as the shot's pixels (CONTROL-SOCKET delta 46), after the
    shape's growth has settled: a stage change animates the frame for MOTION_MAX_MS, and a read inside that window
    measures the shape mid-motion (measured: list 460 of 480, open 518 of 540)."""
    s.ctl.rows(N)                                     # dry run: raises before the wait, the check is not judged
    time.sleep(const("MOTION_MAX_MS") / 1000 + 0.1)
    r = s.ctl.send("capture", "shot", name, ax)
    f = (r.get("frames") or {}).get(ax)
    if not f:
        raise KeyError("no frame for %s in the shot reply" % ax)
    return f


def owner_writes(s, before):
    """Changes of the Mac's own clipboard since `before`: 0 when the test copy kept its copies to its private board."""
    s.ctl.rows("mac-notch")   # dry run: raises, the check is not judged
    return HOST["ownercount"]() - before


def drive(j):
    steps(j)
    steps_r6(j)
    steps_sweep(j)



# Since the owner's pick of 14 Nov 2026 (docs/surfaces/mac-notch.md "1B") an arrival through the real door shows by the KIND of the mail, and an ordinary
# (Primary) mail shows as one line. The cases of this file that judge the CARD of such an arrival set Primary to the card first with this stage click (it
# saves the setting through the engine; `stage.reset` puts the start values back). The alerts by kind are judged in checks/notch-alerts-check.py.
CARD = "mac-notch.stage.alert.primary.card"

def steps(j):
    s = j.step("J6.11", "That's everything")
    fresh(s)
    s.click("mac-notch.stage.hover")
    s.expect("everything-line", lambda: texts(s, N + ".empty"), [EVERYTHING])

    s = j.step("J6.12", "New mail drops out of the notch")
    fresh(s)
    s.click("mac-notch.stage.sample")
    s.expect("drops-newest-first", lambda: (surface(s, "stage"), ids(s)), ("drop", [NEXT, CODE]))
    s.expect("code-pill-on-code-mail", lambda: {r["id"]: r["code"] for r in rows(s, LIST)}, {NEXT: None, CODE: "482915"})

    s = j.step("J6.13", "New-mail buttons follow the setting")
    s.expect("default-buttons", lambda: texts(s, N + ".buttons", "title"), ["Mark read", "Archive", "Reply"])   # Reply is a start button since the owner's pick of 14 Nov 2026
    s.click("mac-notch.stage.setting.buttons.none")
    s.expect("none-hides-buttons", lambda: texts(s, N + ".buttons", "title"), [])
    s.click("mac-notch.stage.setting.buttons.archive")
    s.expect("archive-only", lambda: texts(s, N + ".buttons", "title"), ["Archive"])
    s.click("mac-notch.stage.setting.buttons.mark-read+archive+reply")
    s.expect("saved-reply-shows-reply", lambda: "Reply" in texts(s, N + ".buttons", "title"), True)

    # J6.13b: the Settings control (not the rig's stage click) reaches the notch: the buttons the card shows come from `notch_setting/buttons` (NotchSettings.read).
    s = j.step("J6.13b", "The Settings buttons choice reaches the new-mail card")
    fresh(s)
    s.click("mac-notch.settings")
    s.click("mac-settings-general.notch.buttons.archive")
    s.expect("settings-archive-off-saved", lambda: "archive" in s.ctl.core("notch_setting/buttons"), False)
    s.click("mac-notch.stage.sample")
    s.expect("settings-archive-off-card-has-no-archive", lambda: "Archive" in texts(s, N + ".buttons", "title"), False)
    s.click("mac-settings-general.notch.buttons.archive")
    s.expect("settings-archive-on-saved", lambda: "archive" in s.ctl.core("notch_setting/buttons"), True)
    s.click("mac-notch.stage.sample")
    s.expect("settings-archive-on-card-has-archive", lambda: "Archive" in texts(s, N + ".buttons", "title"), True)

    s = j.step("J6.14", "Copy the login code")
    HOST["setclip"]("par-notch-sentinel")
    s.click(N + ".codepill")
    s.expect("copied-label", lambda: texts(s, N + ".codepill"), ["Copied"])
    s.expect("clipboard-is-code", lambda: clip(s), "482915")
    s.expect("read-not-archived", lambda: [r["read"] for r in rows(s, LIST) if r["id"] == CODE], [True])

    s = j.step("J6.15", "Fill the code into the field in front")
    asked = lambda: int(s.ctl.core("system-ui/paste-into-front-app") or 0)
    before = asked() if s.ctl is not None else 0
    s.click(N + ".fill")
    s.expect("fill-says-why-without-access", lambda: texts(s, N + ".error"), ["Allow Whoosh in Accessibility to fill codes"])
    s.expect("fill-never-sends-the-paste-key-from-a-test-copy", lambda: asked() - before, 1)

    s = j.step("J6.16", "Latest or only unread on hover")
    s.click("mac-notch.stage.hover")
    s.click("mac-notch.stage.setting.show.unread")
    s.expect("unread-hides-read", lambda: ids(s), [NEXT])
    s.click("mac-notch.stage.setting.show.latest")
    s.expect("latest-shows-all", lambda: ids(s), [NEXT, CODE])

    # J6.16b: the Settings control for "unread only" writes `notch_setting/show` (the word latest|unread), which the notch lists by (NotchSettings.swift:49).
    s = j.step("J6.16b", "The Settings Latest or Unread choice reaches the notch list")
    fresh(s)
    s.click("mac-notch.stage.sample")
    s.click(N + ".codepill")                                         # reads the code mail: one read and one unread mail are listed
    s.click("mac-notch.settings")
    s.click("mac-settings-general.notch.show")
    s.expect("settings-show-unread-saved", lambda: s.ctl.core("notch_setting/show"), "unread")
    s.click("mac-notch.stage.hover")
    s.expect("settings-show-unread-row-says-unread", lambda: surface(s, "show"), "unread")
    s.expect("settings-show-unread-lists-only-unread", lambda: ids(s), [NEXT])
    s.click("mac-settings-general.notch.show")
    s.expect("settings-show-latest-saved", lambda: s.ctl.core("notch_setting/show"), "latest")
    s.click("mac-notch.stage.hover")
    s.expect("settings-show-latest-row-says-latest", lambda: surface(s, "show"), "latest")
    s.expect("settings-show-latest-lists-all", lambda: ids(s), [NEXT, CODE])

    s = j.step("J6.17", "Pointer rest at the top edge opens")
    s.click("mac-notch.stage.leave")
    s.expect("leave-shrinks", lambda: surface(s, "stage"), "rest")
    s.click("mac-notch.stage.edge")
    s.expect("top-edge-opens", lambda: surface(s, "stage"), "list")

    s = j.step("J6.18", "Escape shrinks it back")
    s.click(N + ".collapse")
    s.expect("escape-shrinks", lambda: surface(s, "stage"), "rest")

    s = j.step("J6.19", "A Focus keeps the notch shut")
    s.click("mac-notch.stage.hover")
    pre = {}
    try:
        pre["stage"] = surface(s, "stage")                          # the notch is open before the Focus starts
    except Exception:                                                # noqa: BLE001  dry run: the check below is unjudged
        pre["stage"] = None
    s.click("mac-notch.stage.focus.on")
    s.expect("focus-shuts", lambda: (pre["stage"], surface(s, "stage")), ("list", "rest"))
    s.click("mac-notch.stage.away")
    s.click("mac-notch.stage.edge")
    s.expect("focus-blocks-hover", lambda: surface(s, "stage"), "rest")
    s.click("mac-notch.stage.focus.off")

    # J6.19b: the Settings control for "follow Focus" writes `notch_setting/follow_focus`; off, a Focus no longer keeps the notch shut (NotchSettings.swift:50).
    s = j.step("J6.19b", "The Settings Focus choice reaches the notch")
    fresh(s)
    s.click("mac-notch.settings")

    def follow_to(want):
        """The switch TOGGLES, and the fixture core (like the engine) starts with follow_focus false, so one click from the start turns it ON (main112: off-saved read True).
        Read the saved value now and click only when it is not the wanted one."""
        def go():
            if s.ctl.core("notch_setting/follow_focus") is not want:
                s.ctl.click("mac-settings-general.notch.focus-filter")
        s._act(go)
    follow_to(False)
    s.expect("settings-follow-focus-off-saved", lambda: s.ctl.core("notch_setting/follow_focus"), False)
    s.click("mac-notch.stage.focus.on")
    s.click("mac-notch.stage.sample")
    s.expect("settings-follow-focus-off-opens-in-focus", lambda: (surface(s, "focus"), surface(s, "stage") != "rest"), (True, True))
    s.click("mac-notch.stage.focus.off")
    follow_to(True)
    s.expect("settings-follow-focus-on-saved", lambda: s.ctl.core("notch_setting/follow_focus"), True)
    fresh(s)
    s.click("mac-notch.stage.focus.on")
    s.click("mac-notch.stage.sample")
    s.expect("settings-follow-focus-on-keeps-it-shut", lambda: (surface(s, "focus"), surface(s, "stage")), (True, "rest"))
    s.click("mac-notch.stage.focus.off")

    s = j.step("J6.20", "Notifications only: the notch stays shut")
    fresh(s)
    s.click("mac-notch.stage.setting.delivery.notifications")
    s.click("mac-notch.stage.sample")
    s.expect("notifications-only-no-drop", lambda: surface(s, "stage"), "rest")
    s.click("mac-notch.stage.setting.delivery.both")

    s = j.step("J6.21", "Tracked message opened: a one-time notice")
    fresh(s)
    s.click("mac-notch.stage.tracked")
    s.expect("notice-shows", lambda: texts(s, N + ".notice"), ["Lea Martin opened “Proposal v3”"])
    s.click(N + ".notice")
    s.click("mac-notch.stage.tracked")
    s.expect("notice-only-once", lambda: texts(s, N + ".notice"), [])

    s = j.step("J6.22", "Open in place with first lines, actions and a quick reply")
    fresh(s)
    s.click("mac-notch.stage.sample")
    s.click("mac-notch.stage.hover")
    s.click(LIST + ".row.1")
    s.expect("opens-in-place", lambda: [(r["id"], r["text"][:12]) for r in rows(s, N + ".open.body")], [(NEXT, "Are you free")])
    s.expect("actions-listed", lambda: texts(s, N + ".open.actions", "title"), ["Reply", "Archive", "Mark read", "Snooze"])
    # owner's picks of 14 Nov 2026 (docs/surfaces/mac-notch.md): a mail opens on its words, with no reply and no keyboard taken; Reply starts the reply
    s.expect("opens-with-no-reply-started", lambda: (rows(s, N + ".open.quickreply"), surface(s, "keyed")), ([], False))
    s.click(N + ".open.reply")
    s.expect("quick-reply-ready", lambda: [r["focused"] for r in rows(s, N + ".open.quickreply")], [True])
    # the panel is non-activating: the reply types only while the panel holds the keyboard (owner's fork lesson)
    s.expect("panel-takes-keyboard", lambda: surface(s, "keyed"), True)
    # a hidden rig copy records the ask and never becomes key: a key panel there takes the typing of whoever is at this Mac
    s.expect("rig-never-takes-keyboard", lambda: (surface(s, "parked") > 0, surface(s, "iskey")), (True, False))
    # the reply may only answer the mail open in the notch: no reply row, or one whose target is that mail
    s.expect("reply-targets-open-mail", lambda: [r.get("reply_to") for r in rows(s, N + ".open.quickreply")] in ([], [NEXT]), True)

    s = j.step("J6.23", "Undo-send bar fills, then says Sent")
    s.type("Thursday works.")
    # every reply send control shows the one cap, Command-Return (QuickReplyCopy.sendCap): the row the notch card serves at mac-notch.open.send (QuickReplyFeature.swift:52)
    s.expect("send-cap-is-command-return", lambda: [r["cap"] for r in rows(s, N + ".open.send")], ["⌘↵"])
    s.click(N + ".open.send")
    s.expect("sending-bar", lambda: [(r["text"], r["action"]) for r in rows(s, N + ".undo")], [("Sending", "Undo")])
    s.click(N + ".undo.action")
    # the quick reply opens with the AI draft the reply feature writes (a French greeting and signature): the typed words follow it, and Undo must bring back exactly
    # what was there when Send was pressed (main112: the draft plus the typed words)
    s.expect("undo-restores-text", lambda: [t.endswith("Thursday works.") for t in texts(s, N + ".open.quickreply")], [True])
    s.click(N + ".open.send")
    if j.ctl is not None:
        time.sleep(UNDO_S + 1.5)
    s.expect("sent-after-window", lambda: [(r["text"], r["progress"]) for r in rows(s, N + ".undo")], [("Sent", 1.0)])

    s = j.step("J6.24", "A file dropped on the notch starts a message")
    s.click("mac-notch.stage.dropfile")
    s.expect("drop-named", lambda: texts(s, N + ".drop", "name"), ["notch-sample-brief.pdf"])

    s = j.step("J6.26", "A fixture arrival from the socket drops into the notch")
    fresh(s)
    # an id the core does not hold yet: `stage.sample` (J6.12) filed msg-j6-code and msg-j6-next in the fixture core, and its
    # arrival door refuses a repeat id ("there is already a message msg-j6-code"), so no mail reached the notch (main51)
    ident, path = arrival_file("socket", "Northlight Sign-in", "Your sign-in code", "482915")
    s.raw("fixture", "deliver", "r6", path, "eventsource")
    s.expect("socket-arrival-drops", lambda: (surface(s, "stage"), ids(s)), ("drop", [ident]))
    s.expect("socket-arrival-carries-code", lambda: [r["code"] for r in rows(s, LIST)], ["482915"])
    s.expect("socket-arrival-says-who", lambda: [(r["sender"], r["subject"]) for r in rows(s, LIST)], [("Northlight Sign-in", "Your sign-in code")])

    s = j.step("J6.27", "Delete is a new-mail button, and it deletes")
    fresh(s)
    s.click("mac-notch.stage.setting.buttons.delete")
    s.click(CARD)   # an ordinary mail arrives as one line since 14 Nov 2026; this case judges the card
    # the fixture's arrival door files the mail in the core as it arrives and refuses an id the core already holds, so the
    # arrival is a new id: the core holds exactly that mail, and Delete must take it out of the inbox list too
    ident, path = arrival_file("delete", "Real Sender", "Real subject")
    s.raw("fixture", "deliver", "r6", path, "eventsource")
    s.expect("delete-button-drawn", lambda: texts(s, N + ".buttons", "title"), ["Delete"])
    s.click("mac-notch.stage.hover")
    s.click(LIST + ".row.1.delete")
    s.expect("delete-removes-the-mail", lambda: ids(s), [])
    s.expect("delete-leaves-the-inbox", lambda: ident in inbox(s), False)

    s = j.step("J6.28", "The notch's settings control opens Settings, General")
    s.click("mac-notch.settings")
    s.expect("settings-opens-general", lambda: (lambda u: (u["surface"], u["settings_page"]))(s.ctl.uistate()), ("mac-settings-general", "mac-settings-general"))

    s = j.step("J6.29", "The rig's geometry and mute inputs")
    fresh(s)
    s.raw("fixture", "notch-geometry", "none")
    s.expect("geometry-none", lambda: surface(s, "shape"), "none")
    s.raw("fixture", "notch-geometry", "notch")
    s.raw("fixture", "mute-account", "acc-04")
    _, path = arrival_file("muted", "Northlight Sign-in", "Your sign-in code", "482915")   # a new id, so the mute is what keeps it out
    s.raw("fixture", "deliver", "r6", path, "eventsource")
    s.expect("muted-account-never-drops", lambda: ids(s), [])
    s.raw("fixture", "pointer-rest", "mac-notch")
    s.expect("pointer-rest-opens-list", lambda: surface(s, "stage"), "list")

    s = j.step("J6.30", "A dropped mail shrinks back after NOTCH_DROP_HOLD_S, the mail stays")
    fresh(s)
    s.click("mac-notch.stage.sample")
    s.expect("drop-holds-first", lambda: surface(s, "stage"), "drop")
    if j.ctl is not None:
        time.sleep(HOLD_S + 1.5)
    s.expect("drop-shrinks-after-hold", lambda: (surface(s, "stage"), surface(s, "items")), ("rest", 2))

    s = j.step("J6.31", "Rest: two wings while unread mail waits")
    fresh(s)
    s.click("mac-notch.stage.sample")
    s.click(N + ".collapse")
    s.expect("wings-with-unread", lambda: (surface(s, "stage"), surface(s, "wings"), surface(s, "unread")), ("rest", True, 2))
    s.expect("wings-name-the-account", lambda: [(r["unread"], r["account"] != "") for r in rows(s, N + ".wings")], [(2, True)])
    s.click("mac-notch.stage.hover")
    s.click(LIST + ".row.1.markread")
    s.click(LIST + ".row.2.markread")
    s.click(N + ".collapse")
    s.expect("wings-gone-when-read", lambda: (surface(s, "stage"), surface(s, "wings"), surface(s, "unread")), ("rest", False, 0))

    s = j.step("J6.32", "Arrival: the band says the account and now")
    fresh(s)
    s.click("mac-notch.stage.sample")
    s.expect("arrival-band", lambda: [(r["left"] != "", r["right"]) for r in rows(s, N + ".band")], [(True, "now")])
    s.expect("arrival-buttons-carry-keys", lambda: [(r["title"], r["key"]) for r in rows(s, N + ".buttons")], [("Mark read", "U"), ("Archive", "E"), ("Reply", "R")])

    s = j.step("J6.33", "Hover: Latest | Unread, Open in Whoosh, and the end mark under Unread")
    s.click("mac-notch.stage.hover")
    s.expect("list-band", lambda: [(r["left"], r["right"]) for r in rows(s, N + ".band")], [("Latest", "Open in Whoosh")])
    s.expect("no-end-mark-under-latest", lambda: rows(s, N + ".end"), [])
    s.click(N + ".seg.unread")
    s.expect("unread-segment-filters", lambda: (surface(s, "show"), ids(s)), ("unread", [NEXT, CODE]))
    s.expect("end-mark-under-unread", lambda: [(r["text"], r["detail"]) for r in rows(s, N + ".end")], [(EVERYTHING, "2 unread")])
    s.expect("foot-says-hidden-and-esc", lambda: [(r["sharing"] != "", r["key"]) for r in rows(s, N + ".foot")], [(True, "Escape")])
    s.click(N + ".seg.latest")
    s.expect("latest-segment-back", lambda: (surface(s, "show"), rows(s, N + ".end")), ("latest", []))

    s = j.step("J6.34", "Open: header, From, the back chevron, and the archive and read keys")
    s.click(LIST + ".row.1")
    s.expect("open-header", lambda: [(r["sender"], r["address"], r["subject"], r["back"]) for r in rows(s, N + ".open.header")],
             [("Jonas Weber", "jonas@example.test", "Lunch on Thursday?", "Latest")])
    s.expect("open-actions-carry-keys", lambda: [(r["title"], r["key"]) for r in rows(s, N + ".open.actions")],
             [("Reply", "R"), ("Archive", "E"), ("Mark read", "U"), ("Snooze", "H")])
    s.click(N + ".open.reply")      # the From line belongs to the reply: Reply starts it
    s.expect("from-line-says-address", lambda: [r["address"] != "" for r in rows(s, N + ".open.from")], [True])
    s.click(N + ".open.back")
    s.expect("back-returns-to-list", lambda: surface(s, "stage"), "list")
    s.click(LIST + ".row.1")
    s.key("U")
    s.expect("read-key-marks-read", lambda: [r["read"] for r in rows(s, LIST) if r["id"] == NEXT], [True])
    s.key("E")
    s.expect("archive-key-archives", lambda: (surface(s, "stage"), ids(s)), ("list", [CODE]))

    s = j.step("J6.35", "Arrival of a code: the code block's Copy")
    fresh(s)
    _, path = arrival_file("copy", "Northlight Sign-in", "Your sign-in code", "482915")   # a new id: msg-j6-code is in the core already
    s.raw("fixture", "deliver", "r6", path, "eventsource")
    HOST["setclip"]("par-notch-sentinel-2")
    s.click(N + ".copy")
    s.expect("copy-button-copies", lambda: (texts(s, N + ".codepill"), clip(s)), (["Copied"], "482915"))

    s = j.step("J6.36", "Open in Whoosh brings the main window up and shrinks the notch")
    s.click(N + ".whoosh")
    s.expect("open-in-whoosh", lambda: (surface(s, "stage"), s.ctl.uistate()["surface"]), ("rest", "mac-main"))

    s = j.step("J6.37", "The shape's widths and the row's height, from the shot's own frames")
    fresh(s)
    s.click("mac-notch.stage.sample")
    s.expect("drop-width", lambda: frame(s, "par-drop", N + ".shape")["w"] - shape_w("NOTCH_ARRIVAL_W"), 0)
    s.click("mac-notch.stage.hover")
    s.expect("list-width-and-row-height", lambda: (frame(s, "par-list", N + ".shape")["w"] - shape_w("NOTCH_LIST_W"), frame(s, "par-list-row", LIST + ".row.1")["h"] - const("NOTCH_ROW_H")), (0, 0))
    s.click(LIST + ".row.1")
    s.expect("open-width", lambda: frame(s, "par-open", N + ".shape")["w"] - shape_w("NOTCH_OPEN_W"), 0)

    s = j.step("J6.38", "A file dropped on the notch reaches the composer")
    fresh(s)
    s.click("mac-notch.stage.dropfile")
    s.expect("drop-reaches-composer", lambda: "notch-sample-brief.pdf" in [a["name"] for a in s.ctl.rows("mac-composer.attachments")], True)

    s = j.step("J6.39", "A refusal says whose account it is")
    fresh(s)
    s.raw("fixture", "deliver", "failing-account-mail", J6_FIXTURE, "eventsource")
    s.click("mac-notch.stage.hover")
    s.click(LIST + ".row.1.archive")
    s.expect("refusal-names-account", lambda: texts(s, N + ".error"), [FAILING_LINE])

    s = j.step("J6.40", "Hidden from screen shares again after the setting goes off and back on")
    fresh(s)
    s.click("mac-notch.stage.sample")
    s.click("mac-notch.settings")
    s.click("mac-settings-general.notch.hidden")
    s.click("mac-settings-general.notch.hidden")
    s.expect("hidden-again-after-off-on", lambda: [w["s"] for w in cg_notch(s)], [0])

    # J6.40b: one click on the Settings control turns "hidden from screen shares" off: the window server then lists the notch window as capturable.
    s = j.step("J6.40b", "The Settings hidden-from-screen-shares choice reaches the notch window")
    fresh(s)
    s.click("mac-notch.stage.sample")
    s.click("mac-notch.settings")
    s.click("mac-settings-general.notch.hidden")
    s.expect("settings-hidden-off-saved", lambda: s.ctl.core("notch_setting/hidden_in_screen_share"), False)
    s.expect("settings-hidden-off-window-capturable", lambda: [w["s"] for w in cg_notch(s)] != [0] and len(cg_notch(s)) >= 1, True)
    s.click("mac-settings-general.notch.hidden")
    s.expect("settings-hidden-on-saved", lambda: s.ctl.core("notch_setting/hidden_in_screen_share"), True)
    s.expect("settings-hidden-on-window-none", lambda: [w["s"] for w in cg_notch(s)], [0])

    # J6.40c: the Settings switch "Show the notch" writes `notch_setting/enabled` (NotchSetting.show, the engine command SetNotchSetting). Off, the notch is not
    # a delivery target (surface row `delivery` = notifications) and a dropped mail leaves it at rest; on, it drops again.
    s = j.step("J6.40c", "The Settings notch switch hides and shows the notch")
    fresh(s)
    s.click("mac-notch.settings")
    s.click("mac-settings-general.notch.enabled")
    s.expect("settings-notch-off-saved", lambda: s.ctl.core("notch_setting/enabled"), False)
    s.click("mac-notch.stage.sample")
    s.expect("settings-notch-off-hides-the-notch", lambda: (surface(s, "delivery"), surface(s, "stage")), ("notifications", "rest"))
    s.click("mac-settings-general.notch.enabled")
    s.expect("settings-notch-on-saved", lambda: s.ctl.core("notch_setting/enabled"), True)
    fresh(s)
    s.click("mac-notch.stage.sample")
    s.expect("settings-notch-on-shows-the-notch", lambda: (surface(s, "delivery") != "notifications", surface(s, "stage") != "rest"), (True, True))

    s = j.step("J6.25", "Out of screen shares; a top panel without a notch")
    s.expect("sharing-none", lambda: [w["s"] for w in cg_notch(s)], [0])
    s.click("mac-notch.stage.nonotch")
    s.expect("top-of-screen-without-notch", lambda: panel_tops(s), [0])


def arrival_file(name, sender, subject, code=None):
    """One arrival for `fixture deliver`, under an id no earlier step used: the fixture core refuses a repeat id (a mail it
    already holds), and an earlier step may have read or archived the one it holds. Returns (message id, file path)."""
    ident = "msg-j6-r6-%s-%d" % (name, os.getpid())
    path = os.path.join(tempfile.mkdtemp(), "arrival.json")
    arrival = {"id": ident, "thread": "thr-" + ident, "account": "acc-04", "sender": sender, "from": "no-reply@signin-example.com",
               "subject": subject, "first_lines": "Your sign-in code is %s." % code if code else "Are you free on Thursday?", "code": code}
    with open(path, "w") as f:
        json.dump({"arrivals": {"r6": arrival}}, f)
    return ident, path


def inbox(s):
    return {r["id"]: r for r in s.ctl.rows("mac-main.list")}


def arrival_limit_s():
    return 0.2 if FAST else const("NOTCH_ARRIVAL_MS") / 1000


def shown_after_ms(s, t0):
    """Milliseconds from t0 until the notch's surface reads the drop stage; None when NOTCH_ARRIVAL_MS passes first."""
    s.ctl.rows(N)                                     # dry run: raises, the check is not judged
    beat = 0.0 if FAST else const("NOTCH_POINTER_POLL_MS") / 1000
    while time.monotonic() - t0 <= arrival_limit_s():
        if surface(s, "stage") == "drop":
            return round((time.monotonic() - t0) * 1000)
        SLEEP(beat)
    return None


def steps_r6(j):
    """Round 6. Every input is the fixture's own verb (deliver, notch-geometry, drop-file); every outcome is read from the rows
    the owner's eyes would follow: the notch's, the inbox's, the composer's, the clipboard and the window server."""
    limit_ms = lambda: const("NOTCH_ARRIVAL_MS")      # noqa: E731  read inside the check: a missing row fails that check only
    s = j.step("J6.41", "A fixture arrival shows the panel within NOTCH_ARRIVAL_MS of the core event")
    s.raw("fixture", "notch-geometry", "notch")       # an earlier step may have left the no-notch shape on
    fresh(s)
    _, path = arrival_file("arrive", "Northlight Sign-in", "Your sign-in code", "482915")
    t0 = time.monotonic()
    s.raw("fixture", "deliver", "r6", path, "eventsource")
    s.expect("arrival-shows-within-bound", lambda: (lambda ms: ms is not None and ms <= limit_ms())(shown_after_ms(s, t0)), True)
    s.expect("arrival-measured-by-the-app", lambda: (lambda v: v is not None and v <= limit_ms())(surface(s, "arrival_ms")), True)
    s.expect("arrival-window-is-listed", lambda: len(cg_notch(s)), 1)

    s = j.step("J6.42", "The code pill puts the code on the clipboard and reads the mail in the core, never archiving it")
    fresh(s)
    ident, path = arrival_file("pill", "Northlight Sign-in", "Your sign-in code", "482915")
    s.raw("fixture", "deliver", "r6", path, "eventsource")
    HOST["setclip"]("par-notch-sentinel-r6")
    owner = HOST["ownercount"]() if s.ctl is not None else 0
    s.click(N + ".codepill")
    s.expect("pill-puts-code-on-clipboard", lambda: clip(s), "482915")
    s.expect("the-macs-own-clipboard-is-not-written", lambda: owner_writes(s, owner), 0)
    s.expect("pill-reads-it-in-the-core", lambda: [r["unread"] for r in inbox(s).values() if r["id"] == ident], [False])
    s.expect("pill-leaves-it-in-the-inbox", lambda: ident in inbox(s), True)
    s.expect("pill-leaves-it-in-the-notch", lambda: ids(s), [ident])

    s = j.step("J6.43", "Archive removes the mail from the notch and from the inbox list")
    fresh(s)
    ident, path = arrival_file("archive", "Jonas Weber", "Lunch on Thursday?")
    s.raw("fixture", "deliver", "r6", path, "eventsource")
    s.click("mac-notch.stage.hover")
    s.click(LIST + ".row.1.archive")
    s.expect("archive-leaves-the-notch", lambda: ids(s), [])
    s.expect("archive-leaves-the-inbox", lambda: ident in inbox(s), False)

    s = j.step("J6.44", "The panel window's own sharing type reads none")
    fresh(s)
    _, path = arrival_file("sharing", "Jonas Weber", "Lunch on Thursday?")
    s.raw("fixture", "deliver", "r6", path, "eventsource")
    s.expect("window-sharing-type-none", lambda: surface(s, "sharing_type"), 0)

    s = j.step("J6.45", "A file dropped on the notch through the rig's verb opens a composer with it attached")
    fresh(s)
    name = json.load(open(J6_FIXTURE))["drop"]["file"]
    s.raw("fixture", "drop-file", N, name)
    s.expect("verb-drop-names-the-file", lambda: texts(s, N + ".drop", "name"), [name])
    s.expect("verb-drop-attaches-in-composer", lambda: name in [a["name"] for a in s.ctl.rows("mac-composer.attachments")], True)

    s = j.step("J6.46", "Without a notch the panel is a top-of-screen panel")
    fresh(s)
    s.raw("fixture", "notch-geometry", "none")
    _, path = arrival_file("nonotch", "Jonas Weber", "Lunch on Thursday?")
    s.raw("fixture", "deliver", "r6", path, "eventsource")
    s.expect("no-notch-shape-by-verb", lambda: surface(s, "shape"), "none")
    s.expect("no-notch-panel-at-screen-top", lambda: panel_tops(s), [0])
    s.raw("fixture", "notch-geometry", "notch")

    s = j.step("J6.47", "The quick reply opened from the notch answers the notch's open mail, not the mail the main window focuses")
    s.raw("fixture", "notch-geometry", "notch")
    fresh(s)
    ident, path = arrival_file("reply", "Jonas Weber", "Lunch on Thursday?")
    s.raw("fixture", "deliver", "r6", path, "eventsource")
    s.click("mac-notch.stage.hover")
    s.click(LIST + ".row.1")
    s.click(N + ".open.quickreply")
    s.expect("quick-reply-answers-the-open-mail", lambda: [r["reply_to"] for r in s.ctl.rows(N + ".open.quickreply")], [ident])

    s = j.step("J6.48", "A file dropped on the notch asks to bring the composer to the front")
    fresh(s)
    ask0 = activations(s)
    s.raw("fixture", "drop-file", N, json.load(open(J6_FIXTURE))["drop"]["file"])
    s.expect("drop-asks-to-come-front", lambda: activations(s) - ask0, 1)


def activations(s):
    """How many times the app asked macOS to come to the front (the hidden rig records each ask and refuses it)."""
    return sum(1 for r in s.ctl.uistate()["hidden"]["refused"] if r == "activate")


def steps_sweep(j):
    """The sweep: every control of the notch that has a click door, clicked once, with the outcome a person sees. Controls the
    earlier steps already click with an assertion (code pill, Copy, Fill, Open in Whoosh, the segments, the back chevron,
    the rows and their Archive, Mark read and Delete, Send, Undo, the settings door, Escape's collapse) are not repeated; see
    docs/lanes/sweep-notch.md for the table. Each case lays out its own fresh arrival and puts the notch back as it found it."""
    s = j.step("par-notch-sweep", "Every control of the notch clicked once")
    EXTRA_OPEN, FROM = N + ".open", N + ".open.from"
    s.raw("fixture", "notch-geometry", "notch")

    def open_first(tag):
        fresh(s)
        ident, path = arrival_file(tag, "Jonas Weber", "Lunch on Thursday?")
        s.raw("fixture", "deliver", "r6", path, "eventsource")
        s.click("mac-notch.stage.hover")
        s.click(LIST + ".row.1")
        return ident

    # the open mail's close mark (the X in the band)
    open_first("sw-close")
    s.expect("open-mail-is-open-before-close", lambda: surface(s, "stage"), "open")
    s.click(N + ".open.close")
    s.expect("open-close-mark-shrinks-the-notch", lambda: surface(s, "stage"), "rest")

    # Archive on the open mail
    ident = open_first("sw-archive")
    s.click(N + ".open.archive")
    s.expect("open-archive-leaves-the-notch", lambda: (surface(s, "stage"), ident in ids(s)), ("rest", False))
    s.expect("open-archive-leaves-the-inbox", lambda: ident in inbox(s), False)

    # Mark read on the open mail
    ident = open_first("sw-read")
    s.expect("open-mail-unread-before-mark-read", lambda: [r["read"] for r in rows(s, LIST) if r["id"] == ident], [False])
    s.click(N + ".open.markread")
    s.expect("open-mark-read-marks-the-row-read", lambda: [r["read"] for r in rows(s, LIST) if r["id"] == ident], [True])
    s.expect("open-mark-read-reads-it-in-the-core", lambda: [r["unread"] for r in inbox(s).values() if r["id"] == ident], [False])

    # the From selector of the quick reply: open it, pick another account, open it again and pick the first back
    open_first("sw-from")
    s.click(N + ".open.reply")      # the From selector belongs to the reply: Reply starts it (a mail opens on its words)
    st = {}
    try:
        picks = s.ctl.rows(FROM)[0]["picks"]
        st["orig"] = next(p for p in picks if p["chosen"])
        st["other"] = next((p for p in picks if not p["chosen"]), None)
    except Exception:                                                # noqa: BLE001  dry run: the checks below are unjudged
        st["orig"], st["other"] = {"ax": "?", "address": "?"}, {"ax": "?", "address": "?"}
    s.expect("from-has-a-second-account-to-pick", lambda: st["other"] is not None and st["other"]["address"] != st["orig"]["address"], True)
    s.click(FROM)
    s.expect("from-selector-click-opens-the-list", lambda: s.ctl.rows(FROM)[0]["open"], True)
    s.click(st["other"]["ax"] if st["other"] else FROM + ".none")
    s.expect("from-pick-switches-the-sender", lambda: (s.ctl.rows(FROM)[0]["address"], s.ctl.rows(FROM)[0]["open"]), (st["other"]["address"] if st["other"] else None, False))
    s.click(FROM)
    s.click(st["orig"]["ax"])
    s.expect("from-pick-back-restores-the-sender", lambda: (s.ctl.rows(FROM)[0]["address"], s.ctl.rows(FROM)[0]["open"]), (st["orig"]["address"], False))

    # the tracked-open notice: its own click dismisses it
    fresh(s)
    s.click("mac-notch.stage.tracked")
    s.expect("notice-is-up-before-its-click", lambda: len(texts(s, N + ".notice")), 1)
    s.click(N + ".notice")
    s.expect("notice-click-dismisses-it", lambda: texts(s, N + ".notice"), [])

    # the arrival card's own controls and the list row's Reply (sweep-notch F1 to F3, the doors NotchFeature.clickTargets now registers): each is clicked once on a fresh
    # arrival that carries no sign-in code, with all four buttons switched on, and the outcome a person sees is asserted; the setting is put back at the end
    ALL4 = "mac-notch.stage.setting.buttons.mark-read+archive+reply+delete"

    def card(tag):
        fresh(s)
        s.click(CARD)          # the card is what these cases judge: an ordinary mail arrives as one line since 14 Nov 2026
        s.click(ALL4)          # AFTER the reset: `stage.reset` puts the settings back to the defaults (main111: Delete and Reply were not on the card)
        ident, path = arrival_file(tag, "Jonas Weber", "Lunch on Thursday?")
        s.raw("fixture", "deliver", "r6", path, "eventsource")
        return ident
    ident = card("sw-card-read")
    s.click(N + ".arrival.markread")
    s.expect("arrival-card-mark-read-reads-it", lambda: [r["read"] for r in rows(s, LIST) if r["id"] == ident], [True])
    ident = card("sw-card-archive")
    s.click(N + ".arrival.archive")
    s.expect("arrival-card-archive-leaves-the-inbox", lambda: ident in inbox(s), False)
    ident = card("sw-card-delete")
    s.click(N + ".arrival.delete")
    s.expect("arrival-card-delete-leaves-the-notch", lambda: ident in ids(s), False)
    card("sw-card-reply")
    s.click(N + ".arrival.reply")
    s.expect("arrival-card-reply-opens-the-mail", lambda: surface(s, "stage"), "open")
    card("sw-card-tap")
    s.click(N + ".arrival.open")
    s.expect("arrival-card-tap-opens-the-mail", lambda: surface(s, "stage"), "open")
    card("sw-row-reply")
    s.click("mac-notch.stage.hover")
    s.click(LIST + ".row.1.reply")
    s.expect("list-row-reply-opens-the-mail", lambda: surface(s, "stage"), "open")
    s.click("mac-notch.stage.setting.buttons.mark-read+archive+reply")

    # the option-drag along the top edge (NotchDrag, sweep-notch F4) as two doors, a tenth of the screen either way, and the place served as `fraction` on the surface
    # row. It moves only a Mac without a housing, so the shape is set to none first. The place is read after each move, so a rig default left by an earlier run
    # (clamped at an end) cannot change the step: left, read, right, read, left, read, right puts the notch back.
    fresh(s)      # the row's Reply above left a mail open with the reply holding the keys: the notch refuses a drag while it holds them (main149: both moves read 0.0)
    s.raw("fixture", "notch-geometry", "none")
    place = {}
    s.click(N + ".drag.left")
    s._act(lambda: place.update(a=surface(s, "fraction")))
    s.click(N + ".drag.right")
    s._act(lambda: place.update(b=surface(s, "fraction")))
    s.click(N + ".drag.left")
    s._act(lambda: place.update(c=surface(s, "fraction")))
    s.click(N + ".drag.right")
    s.expect("drag-right-moves-the-notch-a-tenth-right", lambda: round(place["b"] - place["a"], 6), 0.1)
    s.expect("drag-left-moves-the-notch-a-tenth-left", lambda: round(place["b"] - place["c"], 6), 0.1)
    fresh(s)
    s.raw("fixture", "notch-geometry", "notch")


# MUTANTS: per check, the mutation of the app that must turn it red.
# J6.11/everything-line | the empty list says nothing
# J6.12/drops-newest-first | an arrival never grows the notch
# J6.12/code-pill-on-code-mail | the login code is not carried into the notch
# J6.13/default-buttons | the new mail shows no buttons
# J6.13/none-hides-buttons | the None setting still shows the buttons
# J6.13/archive-only | the setting is ignored: both buttons always show
# J6.13/saved-reply-shows-reply | a saved reply button is dropped and the new mail shows no Reply
# J6.13b/settings-archive-off-saved | the Archive checkbox does not save
# J6.13b/settings-archive-off-card-has-no-archive | the card keeps the Archive button after Settings switched it off
# J6.13b/settings-archive-on-saved | the Archive checkbox does not save when switched on again
# J6.13b/settings-archive-on-card-has-archive | the card never gets the Archive button back
# J6.14/copied-label | the pill click does nothing
# J6.14/clipboard-is-code | the pill click leaves the clipboard alone
# J6.14/read-not-archived | copying the code leaves the mail unread
# J6.15/fill-says-why-without-access | Fill without the permission is silent
# J6.15/fill-never-sends-the-paste-key-from-a-test-copy | Fill reads the Mac's Accessibility permission and sends Command-V from a test copy
# J6.16/unread-hides-read | the unread setting still lists read mail
# J6.16/latest-shows-all | the latest setting drops read mail
# J6.16b/settings-show-unread-saved | the Unread switch does not save the word unread
# J6.16b/settings-show-unread-row-says-unread | the notch never reads the saved word (reads it as a bool and keeps latest)
# J6.16b/settings-show-unread-lists-only-unread | the notch keeps listing read mail after Settings chose unread
# J6.16b/settings-show-latest-saved | the Unread switch does not save the word latest when switched off
# J6.16b/settings-show-latest-row-says-latest | the notch keeps the word unread after Settings went back to latest
# J6.16b/settings-show-latest-lists-all | the notch keeps hiding read mail after Settings went back to latest
# J6.17/leave-shrinks | the pointer leaving keeps the list open
# J6.17/top-edge-opens | a rest on the top edge does nothing
# J6.18/escape-shrinks | Escape does nothing
# J6.19/focus-shuts | a Focus leaves the notch open
# J6.19/focus-blocks-hover | the pointer opens the notch during a Focus
# J6.19b/settings-follow-focus-off-saved | the Focus switch does not save
# J6.19b/settings-follow-focus-off-opens-in-focus | the notch stays shut in a Focus after Settings turned following off
# J6.19b/settings-follow-focus-on-saved | the Focus switch does not save when switched on again
# J6.19b/settings-follow-focus-on-keeps-it-shut | the notch opens in a Focus although Settings follows it
# J6.20/notifications-only-no-drop | notifications only still drops mail into the notch
# J6.21/notice-shows | a tracked open shows nothing
# J6.21/notice-only-once | the notice shows again for the same message
# J6.22/opens-in-place | a row click does not open the mail in place
# J6.22/actions-listed | the open mail lists no actions
# J6.22/opens-with-no-reply-started | a mail opens with the reply already started and the keyboard taken
# J6.22/quick-reply-ready | Reply on the open mail starts no quick reply
# J6.22/panel-takes-keyboard | opening a mail never makes the notch panel key, so typing goes nowhere
# J6.22/reply-targets-open-mail | the reply opens on whatever row the main window had focused and answers another mail
# J6.22/rig-never-takes-keyboard | a hidden rig copy makes its panel key and takes the typing of whoever is at this Mac
# J6.39/refusal-names-account | a refusal shows the core's reason without the account it belongs to
# J6.40/hidden-again-after-off-on | turning the hidden setting off then on leaves the window capturable
# J6.23/send-cap-is-command-return | the notch card names Control-Return as its send key
# J6.23/sending-bar | a quick send shows no undo bar
# J6.23/undo-restores-text | Undo loses the typed text
# J6.23/sent-after-window | the bar never says Sent
# J6.24/drop-named | a dropped file is ignored
# J6.26/socket-arrival-drops | a fixture arrival never reaches the notch
# J6.26/socket-arrival-carries-code | the arrival's code is dropped on the way
# J6.26/socket-arrival-says-who | the arrival loses its sender or subject
# J6.27/delete-button-drawn | the Delete button is never drawn
# J6.27/delete-removes-the-mail | the Delete button leaves the mail in the notch
# J6.27/delete-leaves-the-inbox | Delete only hides the mail in the notch and never reaches the core
# J6.28/settings-opens-general | the settings control does nothing
# J6.29/geometry-none | the notch-geometry verb is ignored
# J6.29/muted-account-never-drops | a muted account still drops its mail
# J6.29/pointer-rest-opens-list | a rest on the notch opens nothing
# J6.30/drop-holds-first | a drop never opens the shape
# J6.30/drop-shrinks-after-hold | the dropped mail holds the shape open for ever (or the hold also deletes the mail)
# J6.31/wings-with-unread | at rest the housing never grows its wings
# J6.31/wings-name-the-account | the wings carry no account or no count
# J6.31/wings-gone-when-read | the wings stay with nothing unread
# J6.32/arrival-band | the arrival's band is empty (no account, no "now")
# J6.32/arrival-buttons-carry-keys | the new-mail buttons show no key
# J6.33/list-band | the list's band has no Latest or no Open in Whoosh
# J6.33/no-end-mark-under-latest | the end mark shows under Latest too
# J6.33/unread-segment-filters | the Unread segment does nothing
# J6.33/end-mark-under-unread | the Unread list has no end mark, or it names the wrong count
# J6.33/foot-says-hidden-and-esc | the foot is missing its sharing note or its Esc key
# J6.33/latest-segment-back | the Latest segment does nothing
# J6.34/open-header | the open mail's header is missing its sender, address, subject or back word
# J6.34/open-actions-carry-keys | the open mail's actions show no keys
# J6.34/from-line-says-address | the From line is empty
# J6.34/back-returns-to-list | the back chevron does nothing
# J6.34/read-key-marks-read | the read key does nothing on the open mail
# J6.34/archive-key-archives | the archive key does nothing on the open mail
# J6.35/copy-button-copies | the code block's Copy does nothing
# J6.36/open-in-whoosh | Open in Whoosh does nothing
# J6.37/drop-width | the arrival card is not NOTCH_ARRIVAL_W wide
# J6.37/list-width-and-row-height | the list is not NOTCH_LIST_W wide, or a row is not NOTCH_ROW_H tall
# J6.37/open-width | the open mail is not NOTCH_OPEN_W wide
# J6.38/drop-reaches-composer | the dropped file never reaches the composer
# J6.25/sharing-none | the panel shows in screen shares
# J6.25/top-of-screen-without-notch | without a notch the panel sits below the top edge
# J6.40b/settings-hidden-off-saved | the hidden switch does not save
# J6.40b/settings-hidden-off-window-capturable | the notch window stays hidden from screen shares after Settings turned it off
# J6.40b/settings-hidden-on-saved | the hidden switch does not save when switched on again
# J6.40b/settings-hidden-on-window-none | the notch window stays capturable after Settings turned hiding back on
# J6.40c/settings-notch-off-saved | the Show the notch switch does not save
# J6.40c/settings-notch-off-hides-the-notch | the notch still takes mail after Settings switched it off
# J6.40c/settings-notch-on-saved | the Show the notch switch does not save when switched on again
# J6.40c/settings-notch-on-shows-the-notch | the notch stays hidden after Settings switched it back on
# J6.41/arrival-shows-within-bound | a fixture arrival never grows the notch within NOTCH_ARRIVAL_MS
# J6.41/arrival-measured-by-the-app | the arrival's own time (drop received to first frame shown) is never reported
# J6.41/arrival-window-is-listed | an arrival leaves the panel window out of the window server's list
# J6.42/pill-puts-code-on-clipboard | the pill click leaves the clipboard alone
# J6.42/the-macs-own-clipboard-is-not-written | a hidden test copy writes the Mac's own clipboard
# J6.42/pill-reads-it-in-the-core | the pill never tells the core the mail is read
# J6.42/pill-leaves-it-in-the-inbox | the pill archives the mail in the core instead of reading it
# J6.42/pill-leaves-it-in-the-notch | the pill removes the mail from the notch
# J6.43/archive-leaves-the-notch | Archive keeps the mail in the notch's list
# J6.43/archive-leaves-the-inbox | Archive hides the mail in the notch and never reaches the core
# J6.44/window-sharing-type-none | the panel window's own sharing type is readOnly, not none
# J6.46/no-notch-shape-by-verb | the notch-geometry verb is ignored
# J6.46/no-notch-panel-at-screen-top | without a notch the panel sits below the top edge
# J6.45/verb-drop-names-the-file | a file dropped through the rig's verb is ignored by the notch
# J6.45/verb-drop-attaches-in-composer | the verb's dropped file never reaches the composer
# J6.47/quick-reply-answers-the-open-mail | the notch's quick reply opens on the main window's focused mail, not the open one
# J6.48/drop-asks-to-come-front | the dropped file opens the composer without asking to come to the front
# par-notch-sweep/open-mail-is-open-before-close | a row click does not open the mail
# par-notch-sweep/open-close-mark-shrinks-the-notch | the open mail's close mark does nothing
# par-notch-sweep/open-archive-leaves-the-notch | the open mail's Archive button leaves the mail and the open stage in the notch
# par-notch-sweep/open-archive-leaves-the-inbox | the open mail's Archive button never reaches the core
# par-notch-sweep/open-mail-unread-before-mark-read | the mail opens already read
# par-notch-sweep/open-mark-read-marks-the-row-read | the open mail's Mark read button does nothing
# par-notch-sweep/open-mark-read-reads-it-in-the-core | the open mail's Mark read never tells the core
# par-notch-sweep/from-has-a-second-account-to-pick | the From selector offers one account only
# par-notch-sweep/from-selector-click-opens-the-list | the From selector click does nothing
# par-notch-sweep/from-pick-switches-the-sender | picking an account leaves From on the old address, or leaves the list open
# par-notch-sweep/from-pick-back-restores-the-sender | picking the first account back does nothing
# par-notch-sweep/notice-is-up-before-its-click | a tracked open shows no notice
# par-notch-sweep/notice-click-dismisses-it | the notice's own click leaves it up
# par-notch-sweep/drag-right-moves-the-notch-a-tenth-right | the right drag door does nothing
# par-notch-sweep/drag-left-moves-the-notch-a-tenth-left | the left drag door does nothing
# par-notch-sweep/arrival-card-mark-read-reads-it | the arrival card's Mark read does nothing
# par-notch-sweep/arrival-card-archive-leaves-the-inbox | the arrival card's Archive never reaches the core
# par-notch-sweep/arrival-card-delete-leaves-the-notch | the arrival card's Delete leaves the mail in the notch
# par-notch-sweep/arrival-card-reply-opens-the-mail | the arrival card's Reply opens nothing
# par-notch-sweep/arrival-card-tap-opens-the-mail | a tap on the arrival card opens nothing
# par-notch-sweep/list-row-reply-opens-the-mail | the list row's Reply opens nothing


# ---- the selftest: the round-6 checks against a model of the notch and the core, clean and once per mutant ----
# The model answers the socket verbs steps_r6 uses the way Sources/Features/Notch is written to answer them (NotchModel,
# NotchActs, NotchRows, NotchRowFacts, NotchEvents). One mutant (named by its check) is switched on at a time and that
# check must go red. It proves the checks can fail; the lead's mutate.py proves the app's source does what the model does.
class R6Model:
    def __init__(self, mut):
        self.m = mut
        self.core = {}                       # message id -> {"unread", "archived"}
        self.clipboard = "before"
        self.owner_count = 0                 # the Mac's own clipboard: how many times this stand-in wrote it
        self.attachments = []
        self.geo_none = False
        self.reset()

    def on(self, name):
        return name in self.m

    def reset(self):
        self.fraction = 0.5
        self.items, self.stage, self.arrival_ms, self.dropped = [], "rest", None, None
        self.open_id, self.reply_to, self.activations = None, None, []
        self.notice, self.from_open, self.chosen = False, False, "acc-a"

    def arrive(self, f):
        self.core[f["id"]] = {"unread": True, "archived": False}
        if self.on("arrival-shows-within-bound"):
            return                           # the notch never grows
        self.items.insert(0, {"id": f["id"], "code": f.get("code"), "read": self.on("open-mail-unread-before-mark-read")})
        self.stage = "drop"
        self.arrival_ms = None if self.on("arrival-measured-by-the-app") else 12

    def pill(self):
        it = next((i for i in self.items if i["code"]), None)
        if it is None:
            return
        if not self.on("pill-puts-code-on-clipboard"):
            self.clipboard = it["code"]
        if self.on("the-macs-own-clipboard-is-not-written"):
            self.owner_count += 1
        it["read"] = True
        row = self.core[it["id"]]
        if not self.on("pill-reads-it-in-the-core"):
            row["unread"] = False
        if self.on("pill-leaves-it-in-the-inbox"):
            row["archived"] = True
        if self.on("pill-leaves-it-in-the-notch"):
            self.items.remove(it)

    def archive_first(self):
        if not self.items:
            return
        it = self.items[0]
        if not self.on("archive-leaves-the-notch"):
            self.items.remove(it)
        if not self.on("archive-leaves-the-inbox"):
            self.core[it["id"]]["archived"] = True

    ACCOUNTS = {"acc-a": "a@example.test", "acc-b": "b@example.test"}

    def sweep_click(self, ax):
        """The doors par-notch-sweep clicks, answering as NotchFeature.clickTargets does (open.close, open.archive, open.markread,
        open.from and its picks, the notice, stage.tracked)."""
        if ax == "mac-notch.stage.tracked":
            self.notice = not self.on("notice-is-up-before-its-click")
        elif ax == "mac-notch.notice" and self.notice:
            self.notice = self.on("notice-click-dismisses-it")
        elif ax == "mac-notch.open.close" and not self.on("open-close-mark-shrinks-the-notch"):
            self.stage, self.open_id = "rest", None
        elif ax == "mac-notch.open.archive" and self.open_id:
            it = next(i for i in self.items if i["id"] == self.open_id)
            if not self.on("open-archive-leaves-the-notch"):
                self.items.remove(it)
                self.stage, self.open_id = ("list" if self.items else "rest"), None
            if not self.on("open-archive-leaves-the-inbox"):
                self.core[it["id"]]["archived"] = True
        elif ax == "mac-notch.open.markread" and self.open_id:
            it = next(i for i in self.items if i["id"] == self.open_id)
            if not self.on("open-mark-read-marks-the-row-read"):
                it["read"] = True
            if not self.on("open-mark-read-reads-it-in-the-core"):
                self.core[it["id"]]["unread"] = False
        elif ax.startswith("mac-notch.arrival.") and self.stage == "drop" and self.items and not self.items[0]["code"]:
            it, act = self.items[0], ax.rsplit(".", 1)[1]
            if act == "markread" and not self.on("arrival-card-mark-read-reads-it"):
                it["read"] = True
                self.core[it["id"]]["unread"] = False
            elif act == "archive" and not self.on("arrival-card-archive-leaves-the-inbox"):
                self.items.remove(it)
                self.core[it["id"]]["archived"] = True
            elif act == "delete" and not self.on("arrival-card-delete-leaves-the-notch"):
                self.items.remove(it)
            elif act in ("reply", "open") and not self.on("arrival-card-%s-opens-the-mail" % ("reply" if act == "reply" else "tap")):
                self.open_id, self.stage = it["id"], "open"
        elif ax in ("mac-notch.drag.left", "mac-notch.drag.right") and self.geo_none:
            if not self.on("drag-%s-moves-the-notch-a-tenth-%s" % (("right", "right") if ax.endswith("right") else ("left", "left"))):
                self.fraction = min(1.0, max(0.0, self.fraction + (0.1 if ax.endswith("right") else -0.1)))
        elif ax == "mac-notch.list.row.1.reply" and self.stage != "rest" and self.items and not self.on("list-row-reply-opens-the-mail"):
            self.open_id, self.stage = self.items[0]["id"], "open"
        elif ax == "mac-notch.open.from" and self.open_id and not self.on("from-selector-click-opens-the-list"):
            self.from_open = not self.from_open
        elif ax.startswith("mac-notch.open.from.") and self.from_open:
            acc = ax.rsplit(".", 1)[1]
            if acc in self.ACCOUNTS and not (acc == "acc-a" and self.on("from-pick-back-restores-the-sender")):
                if not self.on("from-pick-switches-the-sender"):
                    self.chosen, self.from_open = acc, False

    def window_server(self):
        if self.on("arrival-window-is-listed") and self.stage != "rest":
            return []
        low = self.on("no-notch-panel-at-screen-top") and self.geo_none
        k, cy = MODEL_LOCK                # where the panel is, as a locked screen's window server lists it
        return [{"s": 1 if self.on("window-sharing-type-none") else 0, "y": cy + (MODEL_SCREEN_TOP + (30 if low else 0) - cy) * k}]

    def handle(self, verb, a):
        if verb == "click":
            ax = a[0]
            if ax == "mac-notch.stage.reset":
                self.reset()
            elif ax == "mac-notch.stage.hover":
                self.stage = "list"
            elif ax == "mac-notch.codepill":
                self.pill()
            elif ax == "mac-notch.list.row.1.archive":
                self.archive_first()
            elif ax == "mac-notch.list.row.1" and self.items:
                self.open_id = self.items[0]["id"]
                self.stage = "open" if not self.on("open-mail-is-open-before-close") else self.stage
                # the quick reply's door answers the open mail by its own id; the fault leaves it on the main window's focus
                self.reply_to = "msg-01" if self.on("quick-reply-answers-the-open-mail") else self.open_id
            else:
                self.sweep_click(ax)
            return {"ok": True}
        if verb == "fixture":
            if a[0] == "deliver":
                self.arrive(json.load(open(a[2]))["arrivals"][a[1]])
            elif a[0] == "notch-geometry":
                self.geo_none = a[1] == "none" and not self.on("no-notch-shape-by-verb")
            elif a[0] == "drop-file":
                if not self.on("drop-asks-to-come-front"):
                    self.activations.append("activate")
                if not self.on("verb-drop-names-the-file"):
                    self.dropped = a[2]
                    if not self.on("verb-drop-attaches-in-composer"):
                        self.attachments.append({"name": a[2]})
            return {"ok": True}
        if verb == "rows":
            ax = a[0]
            if ax == "mac-notch":
                return {"ok": True, "rows": [{"stage": self.stage, "shape": "none" if self.geo_none else "screen", "parked": 0,
                                              "arrival_ms": self.arrival_ms, "fraction": self.fraction, "sharing_type": 1 if self.on("window-sharing-type-none") else 0}]}
            if ax == "mac-notch.list":
                return {"ok": True, "rows": [{"id": i["id"], "read": i["read"]} for i in self.items] if self.stage != "rest" else []}
            if ax == "mac-main.list":
                return {"ok": True, "rows": [{"id": k, "unread": v["unread"]} for k, v in self.core.items() if not v["archived"]]}
            if ax == "mac-notch.notice":
                return {"ok": True, "rows": [{"text": "Lea Martin opened"}] if self.notice else []}
            if ax == "mac-notch.open.from":
                accs = self.ACCOUNTS if not self.on("from-has-a-second-account-to-pick") else {"acc-a": self.ACCOUNTS["acc-a"]}
                if not self.open_id:
                    return {"ok": True, "rows": []}
                picks = [{"ax": "mac-notch.open.from." + k, "address": v, "chosen": k == self.chosen} for k, v in accs.items()]
                return {"ok": True, "rows": [{"address": self.ACCOUNTS[self.chosen], "open": self.from_open, "picks": picks}]}
            if ax == "mac-notch.open.quickreply":
                return {"ok": True, "rows": [{"reply_to": self.reply_to}] if self.open_id else []}
            if ax == "mac-notch.drop":
                return {"ok": True, "rows": [{"name": self.dropped}] if self.dropped else []}
            if ax == "mac-composer.attachments":
                return {"ok": True, "rows": self.attachments}
            return {"ok": True, "rows": []}
        if verb == "uistate":
            return {"ok": True, "state": {"hidden": {"refused": list(self.activations)}}}
        return {"ok": False, "error": "model: %s not served" % verb}


def model_run(mut):
    model = R6Model(mut)
    d = tempfile.mkdtemp(prefix="pn6")

    class H(socketserver.StreamRequestHandler):
        def handle(self):
            for line in self.rfile:
                q = json.loads(line.decode())
                self.wfile.write((json.dumps(model.handle(q["verb"], q["args"])) + "\n").encode())
    srv = socketserver.ThreadingUnixStreamServer(os.path.join(d, "whoosh.sock"), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    real = dict(HOST)
    HOST.update(clip=lambda: model.clipboard, setclip=lambda t: setattr(model, "clipboard", t), ownercount=lambda: model.owner_count, cg=model.window_server, top=lambda: MODEL_SCREEN_TOP, scale=lambda _s: MODEL_LOCK)
    ctl = None
    try:
        ctl = Ctl(d).connect()
        j = Journey("par-notch-r6", ctl)
        steps_r6(j)
        steps_sweep(j)
        return {c.label: (c.state, c.detail) for c in j.checks()}
    finally:
        if ctl is not None:
            ctl.close()      # on every path: an open connection keeps its handler thread, and server_close() joins it for ever
        HOST.update(real)
        srv.shutdown()
        srv.server_close()
        shutil.rmtree(d, ignore_errors=True)


def selftest():
    clean = model_run(set())
    bad = 0
    for label, (state, why) in clean.items():
        if state != "pass":
            print("RED on the model: %s %s" % (label, why))
            bad += 1
    print("model, no mutant: %d of %d checks green" % (sum(1 for st, _ in clean.values() if st == "pass"), len(clean)))
    declared = {}
    for line in open(os.path.abspath(__file__), encoding="utf-8"):
        if re.match(r"# (J6\.4[1-8]|par-notch-sweep)/", line) and " | " in line:
            k, v = line[2:].split(" | ", 1)
            declared[k.strip()] = v.strip()
    for label in clean:
        name = label.split("/", 1)[1]
        if label not in declared:
            print("NO MUTANT %s" % label)
            bad += 1
            continue
        got = model_run({name})
        state = got[label][0]
        others = sorted(k.split("/", 1)[1] for k, (st, _) in got.items() if st == "fail" and k != label)
        print("mutant %-40s -> %s%s" % (name, "RED" if state == "fail" else "NOT RED (%s)" % state, ("  also red: " + ", ".join(others)) if others else ""))
        if state != "fail":
            bad += 1
    for label in sorted(set(declared) - set(clean)):
        print("STRAY MUTANT %s" % label)
        bad += 1
    print("%d round-6 checks green on the model, %d mutants, %d not red" % (len(clean), len(declared), bad))
    return 1 if bad else 0


def main():
    path = os.path.abspath(__file__)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    print_cases(r)       # one line per case, the passing ones too
    rows_ = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# J6.") and " | " in l]
    print("MUTANTS (a check is green only after it has been seen red on its mutant)")
    print("| check | mutant |")
    print("|---|---|")
    for k, v in rows_:
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


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else main())
