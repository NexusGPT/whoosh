#!/usr/bin/env python3
"""Settings-general check: ix-settingsgeneralmac, the notch and notification controls (J6.6).

Drives the RUNNING rig copy over the control socket (docs/CONTROL-SOCKET.md) through journeys/lib; expected
values come from fixtures/tests-j6.json, docs/design/fixtures/mac-main-sample.json and docs/CONSTANTS.md, never
from a built screen. The `fixture` verbs it uses are listed in docs/lanes/tests-j6.md "Requests to the root".
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/settingsgeneralmac-check.py'
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0 (a check that could not run is not green).
    python3 checks/settingsgeneralmac-check.py --selftest   the J6.7 door cases against a stand-in app: clean, then one mutant per case

J6.7 (doors-settings): every control of the Accounts pane and of the General notch group that no other check clicked, clicked once with
its visible outcome read. The stand-in app (FakeApp) is used ONLY by --selftest; its expected values are literals, never the check's.
"""
import json
import re
import os
import shutil
import socketserver
import subprocess
import sys
import tempfile
import threading
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, print_cases                                  # noqa: E402
from lib.fixture import FX, const                                    # noqa: E402
from lib.harness import Journey                                      # noqa: E402
from lib.ctl import Ctl, Unreadable                                  # noqa: E402
import J6                                                            # noqa: E402
from J6 import F6, FX_PATH, CODE, NEXT, NOTCH, LIST, MAIN, ids   # noqa: E402

TARGET = "LOCAL"
FAILING_LINE = FX["states"]["sync"]["failing_line"]
MUTED, FAIL = F6["arrivals"]["muted-mail"], F6["arrivals"]["failing-account-mail"]

ID = "settingsgeneralmac"
TITLE = "Settings > General: tabs, notch controls, notification mute, refused save"
VISIBLE_OUTCOME = "The notch controls save, change the notch panel and the capture exclusion; a refused save returns the control with its reason"
P = "mac-settings-general"
TABS = ["Shortcuts", "Notch", "Notifications", "Privacy", "Appearance", "Sync"]
MUTE_ACC = "acc-05"
slug = lambda t: t.lower().replace(" ", "-")
STEP_OFF = "delete"   # a served notch button outside the target set (GeneralStore.allButtons)
# core/crates/contracts/src/inbox_views.rs, check_not_a_view(.., Pin): the sentence a pin on a view is refused with
VIEW_PIN_WORDS = "Primary is a view of the inbox, not a label, so it cannot be pinned: the views are always the first tabs."
LANG = "en"   # the expected words are the English catalogue's: the check pins the language, whatever the Mac speaks


# ---- evidence on a red (reader's dossier main77: a failure line that carries no value cannot be sorted into "app wrong" or "check wrong") --------
# When a case below fails or cannot be judged, its line carries the values that decide it, read from the app at that moment through reads the
# socket already serves: `rows` (the row the case read, its neighbours), `measure` (frame and the window that holds the id), `winframes` (which
# windows exist) and `rows mac-keyevent.monitor` (its `last` reply lists each window with its `key` flag after a keyevent). Not served: whether a
# window is the key window without a keyevent (Sources/App/WindowBridge.swift:33, `frames()`, would carry "key": w.isKeyWindow).
def _compact(v, cap=330):
    t = json.dumps(v, sort_keys=True, default=str)
    return t if len(t) <= cap else t[:cap - 3] + "..."


def read_evidence(ctl, reads):
    """`label=value; ...` for each (label, fn(ctl)) of `reads`: a read that fails says why (never raises, never hides the verdict it explains)."""
    if ctl is None:
        return "no socket"
    out = []
    for label, fn, *cap in reads:
        try:
            out.append("%s=%s" % (label, _compact(fn(ctl), *cap)))
        except BaseException as e:      # noqa: BLE001
            if isinstance(e, (KeyboardInterrupt, SystemExit)):
                raise
            out.append("%s=unreadable(%s: %s)" % (label, type(e).__name__, str(e)[:110]))
    return "; ".join(out)


def ev_rows(ax):
    return lambda c: c.rows(ax)


def ev_measure(ax):
    """The frame of `ax` and the window that holds it (the probe reply's `window`), or the socket's words when nothing carries it."""
    return lambda c: {k: v for k, v in c.measure(ax).items() if k in ("frame", "window")}


def ev_windows(c):
    return [(f.get("title"), f.get("id"), f.get("kind"), [round(f.get(k, 0)) for k in ("x", "y", "w", "h")]) for f in c.winframes()]


def ev_monitor(c):
    """The last real key event the socket sent: where it went, whether the window was key, and each window's key flag and focus."""
    r = (c.rows("mac-keyevent.monitor") or [{}])[0]
    last = r.get("last") or {}
    return {"seen": r.get("seen"), "sent": r.get("sent"), "route": last.get("route"), "handled": last.get("handled"),
            "key_before": last.get("key_before"), "key_after": last.get("key_after"), "window": last.get("window_id"),
            "windows": [(w.get("id"), w.get("key"), w.get("focus")) for w in last.get("windows", [])]}


def expect_ev(s, name, thunk, expected, reads):
    """`s.expect`, and the line of a failed case also carries `evidence: <reads>`; a case that cannot be judged says why and carries it too."""
    def run():
        try:
            got = thunk()
        except Unreadable as e:
            raise Unreadable("%s | evidence: %s" % (e, read_evidence(s.j.ctl, reads)))
        except (KeyError, IndexError, TypeError) as e:
            s._eg = (expected, None)
            return False, "reply shape: %s %s | evidence: %s" % (type(e).__name__, e, read_evidence(s.j.ctl, reads))
        s._eg = (expected, got)
        if got == expected:
            return True, "expected %r, got %r" % (expected, got)
        return False, "expected %r, got %r | evidence: %s" % (expected, got, read_evidence(s.j.ctl, reads))
    s._record(name, run)


def screen_h():
    """Height of the main screen: the app reports frames bottom-left (AppKit), the system window list top-left."""
    v = os.environ.get("WHOOSH_SGM_SCREEN_H")
    if v:
        return float(v)
    js = 'ObjC.import("AppKit");$.NSScreen.screens.objectAtIndex(0).frame.size.height'
    out = subprocess.run(["osascript", "-l", "JavaScript", "-e", js], capture_output=True, text=True, timeout=20)
    if out.returncode != 0:
        raise J6.Unreadable("screen height: %s" % out.stderr.strip())
    return float(out.stdout.strip())


def sharing(s, title):
    """System sharing state of the app's `title` window, matched by frame in either coordinate origin."""
    h, out = screen_h(), []
    for f in [f for f in s.ctl.winframes() if f["title"].lower() == title]:
        for w in J6.cg_windows():
            b = w["b"]
            same = all(abs(b[k] - f[v]) <= 1 for k, v in (("X", "x"), ("Width", "w"), ("Height", "h")))
            if same and (abs(b["Y"] - f["y"]) <= 1 or abs(b["Y"] - (h - f["y"] - f["h"])) <= 1):
                out.append(w["s"])
    return out


def core_buttons(ctl):
    v = ctl.core("notch_setting/buttons")
    return [] if v in (None, "none") else list(v)


def drive(j):
    live = j.ctl is not None
    target = F6["notch_buttons_after"]          # titles, as the notch panel shows them
    target_ids = [slug(b) for b in target]       # ids, as the core stores them (NotchSetting.buttons)
    s = j.step("J6.6", "Settings tabs and the notch controls")
    s.raw("fixture", "deliver", "login-code", FX_PATH, "eventsource")
    s.arrive(("click", NOTCH + ".settings"), P)
    before_lang = [r["code"] for r in s.ctl.rows(P + ".languages") if r["current"]] if live else []
    s.click(P + ".appearance.language." + LANG)
    s.expect("language-pinned-en", lambda: [r["code"] for r in s.ctl.rows(P + ".languages") if r["current"]], [LANG])
    s.expect("tabs-in-order", lambda: [r["title"] for r in s.ctl.rows(P + ".tabs")], TABS)
    s.expect("hidden-default-on", lambda: s.ctl.core("notch_setting/hidden_in_screen_share"), True)
    s.expect("notch-window-hidden-with-setting", lambda: sharing(s, "notch"), [0])
    s.click(P + ".notch.hidden")
    s.expect("hidden-off-saved", lambda: s.ctl.core("notch_setting/hidden_in_screen_share"), False)
    s.expect("hidden-off-notch-capturable", lambda: sharing(s, "notch") not in ([], [0]), True)
    s.click(P + ".notch.hidden")
    # going back to hidden builds a fresh window (NotchSharing.applySharing), which the window server lists a moment later: wait for it, up to 3 s
    def hidden_again():
        v = None
        for _ in range(30):
            v = (s.ctl.core("notch_setting/hidden_in_screen_share"), sharing(s, "notch"))
            if v == (True, [0]):
                break
            time.sleep(0.1)
        return v
    s.expect("hidden-on-again-notch-hidden", hidden_again, (True, [0]))

    s = j.step("J6.6", "Buttons, Focus filter, mute")
    current = []
    if live:
        try:
            current = core_buttons(j.ctl)
        except Exception:
            current = []
        if sorted(current) == sorted(target_ids):   # the core starts at the target: step off it first, through the control
            j.ctl.click(P + ".notch.buttons." + STEP_OFF)   # outside the try: a click that fails must not pass as "differs"
            current = core_buttons(j.ctl)
    for b in target_ids:
        if b not in current:
            s.click(P + ".notch.buttons." + b)
    for b in current:
        if b not in target_ids:
            s.click(P + ".notch.buttons." + b)
    s.expect("buttons-differ-before", lambda: (s.ctl is not None) and sorted(current) != sorted(target_ids), True)
    s.expect("buttons-saved", lambda: sorted(core_buttons(s.ctl)), sorted(target_ids))
    s.expect("notch-follows-buttons", lambda: (s.ctl.click(LIST + ".row.1") or True) and
             sorted(r["title"] for r in s.ctl.rows(NOTCH + ".open.actions")), sorted(target))
    s.expect("focus-filter-on-before", lambda: s.ctl.core("notch_setting/follow_focus"), True)   # following Focus is on by default (lead, 2026-11-13)
    s.click(P + ".notch.focus-filter")
    s.expect("focus-filter-saved", lambda: s.ctl.core("notch_setting/follow_focus"), False)
    s.expect("mute-on-before", lambda: s.ctl.core("notification_setting/%s/enabled" % MUTE_ACC), True)
    s.click(P + ".notifications.account." + MUTE_ACC)
    s.expect("mute-saved", lambda: s.ctl.core("notification_setting/%s/enabled" % MUTE_ACC), False)

    s = j.step("J6.6", "A refused save returns the control")
    reason = F6["save_refusal_reason"]
    s.raw("fixture", "fail-next", "notch-setting-save", reason)
    s.click(P + ".notch.focus-filter")
    s.expect("refused-control-returns", lambda: s.ctl.core("notch_setting/follow_focus"), False)
    s.expect("refused-says-why", lambda: [r["text"] for r in s.ctl.rows(P + ".notch.error")], [reason])
    s.click(P + ".notch.focus-filter")
    s.expect("next-save-works-again", lambda: s.ctl.core("notch_setting/follow_focus"), True)
    # Round 11 (docs/PRD.md "A91 shape", owner 2026-11-09 16:30: "just don't make it a Label because it is confusing"): Primary and All are views,
    # so the pinned labels list offers neither, the labels start at Needs Reply, and a pin asked for a view is refused in the contract's words.
    s = j.step("J6.6", "Pinned labels: no Primary, no All")
    listed = lambda: [r["name"] for r in s.ctl.rows(P + ".pinned")]
    served = lambda: s.ctl.rows(P + ".settings")[0]["pinned_labels"]
    def absent(ax):
        try:
            s.ctl.measure(ax)
            return False
        except J6.Unreadable:
            return True
    s.expect("pinned-list-has-no-primary", lambda: [n for n in listed() + served() if n == "Primary"], [])
    s.expect("pinned-list-has-no-all", lambda: [n for n in listed() + served() if n == "All"], [])
    s.expect("pinned-labels-start-at-needs-reply", lambda: served()[:3], ["Needs Reply", "Updates", "Promotions"])
    s.expect("pinned-list-first-row-is-needs-reply", lambda: listed()[0], "Needs Reply")
    s.expect("pinned-list-offers-no-switch-for-a-view", lambda: [absent(P + ".pinned.label.primary"), absent(P + ".pinned.label.all")], [True, True])
    pinned_before = served() if live else []
    s.click(P + ".pinned.ask.pin-view.primary")
    s.expect("view-pin-refused-in-words", lambda: [r["text"] for r in s.ctl.rows(P + ".pinned.error")], [VIEW_PIN_WORDS])
    s.expect("view-pin-refusal-pins-nothing", lambda: served() == pinned_before and len(pinned_before) > 0, True)
    if before_lang and before_lang != [LANG]:
        s.ctl.click(P + ".appearance.language." + before_lang[0])   # leave the rig speaking what it spoke
    drive_doors(j)
    drive_import(j)


# MUTANTS: per check, the mutation of the app that must turn it red.
# J6.6/arrives-by-control | the notch settings control is missing
# J6.6/language-pinned-en | the language control does not switch the app to English
# J6.6/tabs-in-order | a tab is missing or out of order
# J6.6/hidden-default-on | the default is off
# J6.6/notch-window-hidden-with-setting | the notch window can be captured while the setting says hidden
# J6.6/hidden-off-saved | the toggle is not saved
# J6.6/hidden-off-notch-capturable | the notch window stays hidden after the setting is turned off
# J6.6/hidden-on-again-notch-hidden | turning the setting back on does not hide the window again
# J6.6/buttons-differ-before | the buttons start at the target and a click on another button does not reach the core
# J6.6/buttons-saved | the buttons are not saved
# J6.6/notch-follows-buttons | the notch keeps its old buttons
# J6.6/focus-filter-on-before | the Focus filter starts on
# J6.6/focus-filter-saved | the Focus filter is not saved
# J6.6/mute-on-before | the account starts muted
# J6.6/mute-saved | the mute is not saved
# J6.6/refused-control-returns | a refused save leaves the control in its new position
# J6.6/refused-says-why | a refused save gives no reason
# J6.6/next-save-works-again | a refused save blocks every later save
# J6.6/pinned-list-has-no-primary | the pinned list shows a Primary row again (the list stops dropping view rows)
# J6.6/pinned-list-has-no-all | the pinned list shows an All row
# J6.6/pinned-labels-start-at-needs-reply | the core's pinned labels start with Primary
# J6.6/pinned-list-first-row-is-needs-reply | the list's first row is Primary
# J6.6/pinned-list-offers-no-switch-for-a-view | the list draws a switch for Primary
# J6.6/view-pin-refused-in-words | a pin on a view shows the core's debug text, or nothing, instead of the contract's sentence
# J6.6/view-pin-refusal-pins-nothing | the refused pin pins another label


# ---- J6.7 (doors-settings): the controls no other check clicked ------------------------------------------------------------------
WELCOME_RESET, WELCOME_PROGRESS = "mac-welcome-accounts.import.reset", "mac-welcome-accounts.import.progress"
A, W, IMPORT = "mac-settings-accounts", "mac-settings-window", "mac-welcome-accounts.import"
NAME_SUFFIX, SIG_SUFFIX = " door", " sig"      # typed after the account's own name and signature: the page's fields append


def drawn(s, ax):
    """True when something on screen carries the id and has a size (raises in a dry run: not judged, not failed)."""
    ctl = s.ctl
    try:
        return ctl.measure(ax)["frame"]["h"] > 0
    except (Unreadable, KeyError):
        return False


def gone(s, ax):
    """True once nothing carries the id: a review or picker closes with a fade, so the view outlives its state by a few frames (waits up to 2 s)."""
    ok = drawn(s, ax)
    for _ in range(20):
        if not ok:
            break
        time.sleep(0.1)
        ok = drawn(s, ax)
    return not ok


def drive_doors(j):
    live = j.ctl is not None
    s = j.step("J6.7", "Doors: Show switch, delivery, name and signature fields, find, scrims, Upload, Import")
    # the General notch group: the Show switch (the sweep of par-settingswindow clicks it but never names it)
    s.click(W + ".pane-row.notifications")

    def show_flips():
        c = s.ctl
        b = c.rows(P + ".settings")[0]["unread_only"]
        c.click(P + ".notch.show")
        a = c.rows(P + ".settings")[0]["unread_only"]
        c.click(P + ".notch.show")
        return (a != b, c.rows(P + ".settings")[0]["unread_only"] == b)
    s.expect("notch-show-door-flips-the-switch", show_flips, (True, True))

    s.click(W + ".pane-row.accounts")
    acc = None
    if live:
        acc = next((r for r in j.ctl.rows(A + ".accounts") if r["healthy"]), None)
        if acc is None:
            raise Unreadable("no healthy account in %s.accounts" % A)
    aid, base_name, base_sig = (acc["id"], acc["name"], acc["signature"]) if acc else ("acc-01", "", "")
    row = lambda: [r for r in s.ctl.rows(A + ".accounts") if r["id"] == aid][0]
    field = lambda: s.ctl.rows(A + ".field")[0]

    # delivery: three choices, each saved through the core (notch was clicked by par-settingsaccountsmac; the other two never)
    for key in ("notifications", "notch", "both"):
        s.click("%s.%s.delivery.%s" % (A, aid, key))
        s.expect("delivery-%s-saved" % key, lambda: row()["delivery"], key)

    # the page's name field: a click focuses it, `type` writes in it, Return saves
    s.click("%s.%s" % (A, aid))
    s.click("%s.%s.name.field" % (A, aid))
    s.expect("name-door-focuses-the-field", lambda: field()["focus"], "name:" + aid)
    s.type(NAME_SUFFIX)
    s.expect("name-field-takes-the-typed-text", lambda: field()["name_draft"], base_name + NAME_SUFFIX)
    s.click("%s.%s.name.field.submit" % (A, aid))
    s.expect("name-return-saves-through-the-core", lambda: row()["name"], base_name + NAME_SUFFIX)
    s.click("%s.%s.signature.field" % (A, aid))
    s.expect("signature-door-focuses-the-field", lambda: field()["focus"], "signature:" + aid)
    s.type(SIG_SUFFIX)
    s.expect("signature-field-takes-the-typed-text", lambda: field()["signature_draft"], base_sig + SIG_SUFFIX)
    s.click("%s.%s.signature.field.submit" % (A, aid))
    s.expect("signature-return-saves-through-the-core", lambda: row()["signature"], base_sig + SIG_SUFFIX)

    # the removal review: a click outside the card closes it, the account stays
    s.click("%s.%s.remove" % (A, aid))
    count = {}
    s.expect("remove-review-is-drawn", lambda: (count.update(n=len(s.ctl.rows(A + ".accounts"))), drawn(s, A + ".remove.review"))[1], True)
    s.click(A + ".remove.scrim")
    s.expect("remove-scrim-closes-the-review-and-keeps-the-account", lambda: (gone(s, A + ".remove.review"), len(s.ctl.rows(A + ".accounts")) == count["n"]), (True, True))
    # Escape is Cancel on the review (the "esc" cap was bound to nothing, lane reply-actions): the review goes, the account stays
    s.click("%s.%s.remove" % (A, aid))
    # a real key event on arm d (KeyEventVerb): the review's cancel is a SwiftUI .cancelAction button, which the `key` verb (AppModel.press) cannot reach, and the
    # hidden rig never makes its window key, so arm a (the default) hands the event to NSApp with no key window and it is not delivered (main79: key_before false, seen 0);
    # arm d makes the window key first, which is what a person's click on the window does
    s.raw("keyevent", "reset")
    s.raw("keyevent", "arm:d", "Escape")

    def escape_cancels():
        # main81: arm d too left the window not key (key_before false, key_after false) and no key monitor saw the event (seen 0, sent 1): the hidden copy never makes
        # a window key (KeyEventVerb.swift:10-13), so the review's cancel button was never offered the key. That says nothing about the app: it is NOT JUDGED here,
        # and RED only when the event did reach a monitor and the review stayed.
        mon = (s.ctl.rows("mac-keyevent.monitor") or [{}])[0]
        if not mon.get("seen"):
            raise Unreadable("the Escape key event reached no key monitor (seen %r, sent %r): the hidden test copy's window cannot become key, so the review's cancel key "
                             "cannot be judged here (KeyEventVerb.swift:10-13)" % (mon.get("seen"), mon.get("sent")))
        return (gone(s, A + ".remove.review"), len(s.ctl.rows(A + ".accounts")) == count["n"])
    expect_ev(s, "remove-escape-cancels-the-review-and-keeps-the-account", escape_cancels, (True, True),
              [("accounts_before_now", lambda c: (count.get("n"), len(c.rows(A + ".accounts")))), ("review", ev_measure(A + ".remove.review")),
               ("review_row_for", lambda c: aid), ("keyevent", ev_monitor), ("windows", ev_windows)])

    # the icon and colour picker: the scrim closes it, the Upload button stores a picture
    s.click("%s.%s.icon" % (A, aid))
    s.expect("picker-is-drawn", lambda: drawn(s, A + ".picker"), True)
    s.click(A + ".picker.scrim")
    s.expect("picker-scrim-closes-the-picker", lambda: gone(s, A + ".picker"), True)
    s.click("%s.%s.icon" % (A, aid))
    s.click(A + ".upload")
    s.expect("upload-door-stores-a-picture", lambda: aid in [r["account"] for r in s.ctl.rows(A + ".icon.stored")], True)
    s.click(A + ".picker.done")

    # the find field (drawn from the seventh account on): a click focuses it, `type` narrows the list
    s.click(W + ".pane-row.accounts")
    s.click(A + ".find")
    s.expect("find-door-focuses-the-field", lambda: field()["focus"], "find")
    s.expect("find-focus-holds-after-the-page-closes", lambda: (time.sleep(0.4), field()["focus"])[1], "find")   # the page's onDisappear runs a frame after the click
    s.type("provanta")
    s.expect("find-field-takes-the-typed-text-and-narrows-the-list",
             lambda: (field()["find_text"], [r for r in s.ctl.rows(A + ".accounts") if "provanta" in (r["name"] + r["address"]).lower()] != [] and
                      all("provanta" in (r["name"] + r["address"]).lower() for r in s.ctl.rows(A + ".accounts"))), ("provanta", True))
    s.click(A + ".fixture.find.")

    # Import mail: the offer opens the Welcome window at its import step (last: the window leaves the Settings window behind it)
    s.click(IMPORT + ".reset")
    s.click(A + ".import")
    s.expect("import-door-opens-the-import-step", lambda: [(r["showing"], r["from_settings"]) for r in s.ctl.rows(IMPORT + ".panel")], [(True, True)])
    s.click(W + ".open")


# MUTANTS (J6.7, doors-settings): the label, then the mutation of the app that must turn it red. Each is proven red by --selftest.
# J6.7/notch-show-door-flips-the-switch | the Show switch's door does nothing
# J6.7/delivery-notifications-saved | the Notifications delivery choice is not saved
# J6.7/delivery-notch-saved | the Notch delivery choice is not saved
# J6.7/delivery-both-saved | the Both delivery choice is not saved
# J6.7/name-door-focuses-the-field | a click on the name field focuses nothing
# J6.7/name-field-takes-the-typed-text | typed text never reaches the name field
# J6.7/name-return-saves-through-the-core | Return in the name field saves nothing
# J6.7/signature-door-focuses-the-field | a click on the signature field focuses nothing
# J6.7/signature-field-takes-the-typed-text | typed text never reaches the signature field
# J6.7/signature-return-saves-through-the-core | Return in the signature field saves nothing
# J6.7/remove-review-is-drawn | Remove account opens no review
# J6.7/remove-scrim-closes-the-review-and-keeps-the-account | a click outside the review card leaves it open
# J6.7/remove-escape-cancels-the-review-and-keeps-the-account | Escape leaves the removal review open
# J6.7/picker-is-drawn | the icon button opens no picker
# J6.7/picker-scrim-closes-the-picker | a click outside the picker leaves it open
# J6.7/upload-door-stores-a-picture | the Upload button stores no picture
# J6.7/find-door-focuses-the-field | a click on the find field focuses nothing
# J6.7/find-focus-holds-after-the-page-closes | closing the account page, a frame after the click, takes the focus from the find field
# J6.7/find-field-takes-the-typed-text-and-narrows-the-list | typed text never reaches the find field
# J6.7/import-door-opens-the-import-step | the Import mail button opens nothing
LABEL_MUTANT = {
    "notch-show-door-flips-the-switch": "show-does-nothing", "delivery-notifications-saved": "delivery-notifications-unsaved",
    "delivery-notch-saved": "delivery-notch-unsaved", "delivery-both-saved": "delivery-both-unsaved",
    "name-door-focuses-the-field": "name-door-dead", "name-field-takes-the-typed-text": "name-typing-lost",
    "name-return-saves-through-the-core": "name-return-dead", "signature-door-focuses-the-field": "signature-door-dead",
    "signature-field-takes-the-typed-text": "signature-typing-lost", "signature-return-saves-through-the-core": "signature-return-dead",
    "remove-review-is-drawn": "remove-opens-nothing", "remove-scrim-closes-the-review-and-keeps-the-account": "remove-scrim-dead",
    "remove-escape-cancels-the-review-and-keeps-the-account": "remove-escape-dead",
    "picker-is-drawn": "icon-opens-nothing", "picker-scrim-closes-the-picker": "picker-scrim-dead", "upload-door-stores-a-picture": "upload-dead",
    "find-door-focuses-the-field": "find-door-dead", "find-focus-holds-after-the-page-closes": "page-close-clears-find-focus", "find-field-takes-the-typed-text-and-narrows-the-list": "find-typing-lost",
    "import-door-opens-the-import-step": "import-dead",
}


IMPORT_SPECS = ["apple-mail=12480", "olm:~/Documents/Outlook.olm=2310", "mbox:~/Mail/in.mbox=?"]
FDA_ASK = "Whoosh needs Full Disk Access to read Apple Mail."


def waited(fn, ok, tries=30):
    """The value of `fn` once `ok` holds for it (the scan runs off the main thread): up to 3 s."""
    v = fn()
    for _ in range(tries):
        if ok(v):
            break
        time.sleep(0.1)
        v = fn()
    return v


def drive_import(j):
    """J6.8 (doors-settings round 3): Settings > Accounts > Import mail…, every control of the sheet clicked once, the outcome read."""
    s = j.step("J6.8", "Import mail sheet: sources, Full Disk Access, run, Cancel, result, Close")
    I, WI = A + ".import", "mac-welcome-accounts.import"
    if j.ctl is not None:
        j.ctl.click(W + ".pane-row.accounts")
        if j.ctl.rows(I + ".sheet") and j.ctl.rows(I + ".sheet")[0]["open"]:
            j.ctl.click(I + ".close")
    s.raw("fixture", "import-sources", *IMPORT_SPECS)
    s.click(WI + ".reset")
    s.click(I)
    sheet = lambda: s.ctl.rows(I + ".sheet")[0]
    trail = {}      # what the sheet, its sources and the run read at the named moments of the run (evidence of the cases that follow it)

    def snap(tag):
        if j.ctl is not None:
            try:
                sh, res = sheet(), j.ctl.rows(I + ".result")
                trail[tag] = "state=%s running=%s selected=%s fda=%s | result=%s | run_ids=%s | sources(kind,count,imported)=%s" % (
                    sh.get("state"), sh.get("running"), sh.get("selected"), sh.get("needs_full_disk_access"),
                    [(r.get("progress"), r.get("line"), r.get("done"), r.get("stored")) for r in res], [r.get("run_id") for r in j.ctl.rows(WI + ".progress")],
                    [(r.get("kind"), r.get("count"), r.get("imported")) for r in j.ctl.rows(I + ".sources")])
            except Exception as e:      # noqa: BLE001  a read for the evidence never stops the drive
                trail[tag] = "unreadable(%s: %s)" % (type(e).__name__, str(e)[:90])

    def again(c):
        """The result row and the sheet once more after 1.0 s: tells a finish that landed late from one that never landed."""
        time.sleep(1.0)
        return {"sheet": {k: v for k, v in c.rows(I + ".sheet")[0].items() if k in ("open", "running", "state")}, "result": [(r.get("progress"), r.get("line")) for r in c.rows(I + ".result")]}
    IMP = [("sheet", ev_rows(I + ".sheet")), ("sources", ev_rows(I + ".sources")), ("result", ev_rows(I + ".result")), ("run_progress", ev_rows(WI + ".progress")),
           ("sheet_frame", ev_measure(I + ".sheet")), ("windows", ev_windows), ("trail", lambda c: list(trail.items()), 1400)]
    IMP_LATE = IMP + [("again_after_1s", again)]
    kinds = lambda: [(r["kind"], r["count"]) for r in s.ctl.rows(I + ".sources")]
    s.expect("import-button-opens-the-sheet", lambda: sheet()["open"], True)
    s.expect("sheet-lists-the-three-sources-with-their-counts", lambda: waited(kinds, lambda v: len(v) == 3),
             [("apple-mail", 12480), ("olm", 2310), ("mbox", None)])
    s.expect("sheet-is-drawn", lambda: drawn(s, I + ".sheet"), True)
    for kind in ("olm", "mbox", "apple-mail"):
        s.click("%s.source.%s" % (I, kind))
        s.expect("source-%s-door-chooses-it" % kind, lambda kind=kind: sheet()["selected"], kind)
    s.click(I + ".choose-bad")
    s.expect("choose-a-file-door-refuses-a-bad-file-in-words", lambda: sheet()["refusal"],
             "Pick an Outlook archive (.olm) or a Thunderbird or Unix mailbox file (.mbox).")
    s.click(I + ".choose-mbox")
    s.expect("choose-a-file-door-keeps-the-sources", lambda: len(s.ctl.rows(I + ".sources")), 3)

    def choose_apple_mail():
        """Choose the Apple Mail row once the scan listed it (main79: the check never chose it, so every later figure was the mailbox file's)."""
        if j.ctl is not None:
            waited(lambda: [r["kind"] for r in s.ctl.rows(I + ".sources")], lambda v: "apple-mail" in v)
        s.click("%s.source.apple-mail" % I)

    # Full Disk Access: the ask in one sentence, the button that opens the right System Settings page (the door records the ask; a test never opens it)
    s.click(I + ".close")
    s.raw("fixture", "import-sources", "apple-mail=fda")
    s.click(I)
    choose_apple_mail()      # the sheet keeps the source chosen last (the mailbox file above): the Full Disk Access ask belongs to the CHOSEN source
    snap("after-fda-open")
    expect_ev(s, "fda-source-asks-for-access", lambda: waited(lambda: sheet()["needs_full_disk_access"], lambda v: v), True, IMP)
    s.click(I + ".open-settings")
    expect_ev(s, "fda-button-asks-for-the-right-page", lambda: sheet()["fda_opened"], True, IMP)
    s.click(I + ".close")

    # a run: progress with a count, Cancel, then a run to its result
    s.raw("fixture", "import-sources", "apple-mail=12480")
    s.click(I)
    waited(lambda: len(s.ctl.rows(I + ".sources")), lambda v: v == 1) if j.ctl else None
    choose_apple_mail()      # the run, its "of 12480" count and its history are Apple Mail's: the chosen source decides which one starts
    s.click(I + ".start")
    s.expect("start-door-begins-a-run", lambda: sheet()["running"], True)
    snap("after-first-start")
    run_id = lambda: s.ctl.rows(WI + ".progress")[0]["run_id"]
    s.raw("fixture", "import-progress", run_id() if j.ctl else "run", "5000")
    # the sheet's progress line follows the run a moment after the verb answers (main81: "0 of 12480" at once, "5000 of 12480" one second later): read until it moves, up to 3 s
    expect_ev(s, "progress-says-how-many-of-how-many",
              lambda: waited(lambda: s.ctl.rows(I + ".result")[0]["progress"], lambda v: v.startswith("5000 of"), 30).startswith("5000 of 12480"), True, IMP_LATE)
    s.click(I + ".cancel")
    s.expect("cancel-ends-the-run-in-words", lambda: (sheet()["state"], s.ctl.rows(I + ".result")[0]["line"]), ("cancelled", "Import cancelled. What was stored stays."))
    snap("after-cancel")
    stored = {}     # what each run put in, read from the result row's own `stored` (the source row adds the runs up)
    stored["cancelled"] = s.ctl.rows(I + ".result")[0]["stored"] if j.ctl is not None else 0
    s.click(I + ".start")
    snap("after-second-start")
    s.raw("fixture", "import-progress", run_id() if j.ctl else "run", "finish")
    snap("after-finish")
    # the figure is written with the Mac's grouping mark (main81: "11.981" on a Mac whose region groups with a dot, "11,981" elsewhere): the digits are compared, not the mark
    ungroup = lambda t: re.sub(r"(?<=\d)[.,\u202f\u00a0 ](?=\d{3}\b)", "", t)
    expect_ev(s, "result-says-how-many-went-into-which-account", lambda: ungroup(s.ctl.rows(I + ".result")[0]["line"]), "11981 messages imported into Imported", IMP_LATE)
    stored["finished"] = s.ctl.rows(I + ".result")[0]["stored"] if j.ctl is not None else 0
    # the source row adds up every run on that source: the cancelled run's stored mail stays (main81: 4800 + 11981 = 16781), 
    expect_ev(s, "source-row-says-what-is-already-imported",
              lambda: [(r["kind"], r["imported"]) for r in s.ctl.rows(I + ".sources") if r["kind"] == "apple-mail"],
              [("apple-mail", stored["cancelled"] + stored["finished"])], IMP_LATE)
    s.click(I + ".close")
    # the sheet leaves with a fade, so its view outlives its state by a few frames: the drawn half waits for the fade (gone, up to 2 s) like the review and picker cases;
    # the state half is read at once. A sheet that is still drawn after the wait, or still open, is red; the evidence carries the one-second reread
    expect_ev(s, "close-door-closes-the-sheet", lambda: (sheet()["open"], gone(s, I + ".sheet")), (False, True), IMP_LATE)
    s.click(I)
    s.click(I + ".scrim")
    s.expect("scrim-door-closes-the-sheet", lambda: sheet()["open"], False)


# MUTANTS (J6.8, doors-settings round 3): each is proven red by --selftest.
# J6.8/import-button-opens-the-sheet | the Import mail button opens no sheet
# J6.8/sheet-lists-the-three-sources-with-their-counts | the sheet lists one source or shows no count
# J6.8/sheet-is-drawn | the sheet is open in the state but never drawn
# J6.8/source-olm-door-chooses-it | the Outlook row does not choose itself
# J6.8/source-mbox-door-chooses-it | the mailbox row does not choose itself
# J6.8/source-apple-mail-door-chooses-it | the Apple Mail row does not choose itself
# J6.8/choose-a-file-door-refuses-a-bad-file-in-words | Choose a file takes a text file without a word
# J6.8/choose-a-file-door-keeps-the-sources | choosing a file empties the list of sources
# J6.8/fda-source-asks-for-access | an unreadable Apple Mail shows no ask
# J6.8/fda-button-asks-for-the-right-page | the Open Full Disk Access button does nothing
# J6.8/start-door-begins-a-run | Start begins no run
# J6.8/progress-says-how-many-of-how-many | the progress line shows no count
# J6.8/cancel-ends-the-run-in-words | Cancel leaves the run going
# J6.8/result-says-how-many-went-into-which-account | the result line gives no count or no account
# J6.8/source-row-says-what-is-already-imported | the source row forgets what a run stored
# J6.8/close-door-closes-the-sheet | Close leaves the sheet up
# J6.8/scrim-door-closes-the-sheet | a click outside the sheet leaves it up
IMPORT_MUTANT = {
    "import-button-opens-the-sheet": "imp-open-dead", "sheet-lists-the-three-sources-with-their-counts": "imp-one-source", "sheet-is-drawn": "imp-undrawn",
    "source-olm-door-chooses-it": "imp-select-olm-dead", "source-mbox-door-chooses-it": "imp-select-mbox-dead", "source-apple-mail-door-chooses-it": "imp-select-apple-dead",
    "choose-a-file-door-refuses-a-bad-file-in-words": "imp-bad-file-silent", "choose-a-file-door-keeps-the-sources": "imp-choose-empties",
    "fda-source-asks-for-access": "imp-fda-silent", "fda-button-asks-for-the-right-page": "imp-fda-button-dead", "start-door-begins-a-run": "imp-start-dead",
    "progress-says-how-many-of-how-many": "imp-progress-bare", "cancel-ends-the-run-in-words": "imp-cancel-dead",
    "result-says-how-many-went-into-which-account": "imp-result-bare", "source-row-says-what-is-already-imported": "imp-history-lost",
    "close-door-closes-the-sheet": "imp-close-dead", "scrim-door-closes-the-sheet": "imp-scrim-dead",
}
LABEL_MUTANT.update(IMPORT_MUTANT)
# the cases whose red line carries the values that decide it (expect_ev); --selftest proves each prints them on its mutant
EVIDENCE_CASES = {"remove-escape-cancels-the-review-and-keeps-the-account", "fda-source-asks-for-access", "fda-button-asks-for-the-right-page",
                  "progress-says-how-many-of-how-many", "result-says-how-many-went-into-which-account", "source-row-says-what-is-already-imported",
                  "close-door-closes-the-sheet"}


# ---- --selftest: a stand-in app on the control-socket wire (never used against the real app) ------------------------------------
class FakeApp:
    """Just enough of the app for J6.7: the verbs the cases use, the rows they read, and one switch per mutant."""

    def __init__(self, mutants=()):
        self.m = set(mutants)
        self.keys_sent = 0
        self.accounts = [{"id": "acc-%02d" % i, "name": n, "address": a, "healthy": True, "signature": "", "delivery": "both"}
                         for i, (n, a) in enumerate([("Work", "me@work.example"), ("Home", "me@home.example"), ("Provanta RFP", "rfp@provanta.example"),
                                                     ("Club", "me@club.example"), ("Shop", "me@shop.example"), ("Blog", "me@blog.example"),
                                                     ("Travel", "me@travel.example")], 1)]
        self.page, self.focus, self.query, self.removing, self.editing, self.stored = None, None, "", None, None, set()
        self.name_draft, self.sig_draft, self.unread_only, self.import_open, self.import_from = {}, {}, False, False, False
        self.shown = "general"
        self.imp_init()

    def acc(self, i):
        return next(a for a in self.accounts if a["id"] == i)

    # ---- J6.8: the Import mail sheet
    def imp_init(self):
        self.found, self.scanned, self.isel, self.irun, self.isheet, self.fda_opened, self.irefusal, self.istate = [], [], None, None, False, False, "", "none"
        self.idone, self.istored, self.hist = 0, 0, 0

    def imp_specs(self, specs):
        out = []
        for sp in specs:
            head, _, val = sp.partition("=")
            kind = head.split(":")[0]
            out.append({"kind": kind, "name": kind, "count": int(val) if val.isdigit() else None, "access": "needs_full_disk_access" if val == "fda" else "readable", "imported": 0})
        return out

    def v_fixture(self, name, *a):
        if name == "import-sources":
            self.found = self.imp_specs(a)
        elif name == "import-progress":
            if a[1] == "finish":
                total = self.scanned[0]["count"]
                self.istate, self.idone, self.istored = "finished", total, total - total // 25
                self.scanned[0]["imported"] += 0 if "imp-history-lost" in self.m else self.istored
            else:
                self.idone = int(a[1])
        else:
            raise KeyError(name)

    def imp_click(self, ax):
        m, I = self.m, A + ".import"
        if ax == WELCOME_RESET:
            self.scanned, self.isel, self.irun, self.istate, self.irefusal = [], None, None, "none", ""
        elif ax == I:
            if "imp-open-dead" not in m:
                self.isheet, self.scanned, self.irefusal = True, [dict(x) for x in self.found][:1 if "imp-one-source" in m else 3], ""
                if self.isel is None:      # the real sheet keeps the chosen source across a close and a reopen (main79: still the mailbox file)
                    self.isel = next((x["kind"] for x in self.scanned if x["access"] == "readable"), None)
        elif not self.isheet:
            raise KeyError(ax)
        elif ax.startswith(I + ".source."):
            k = ax.rsplit(".", 1)[1]
            if "imp-select-%s-dead" % ("apple" if k == "apple-mail" else k) not in m:
                self.isel = k
        elif ax == I + ".choose-bad":
            if "imp-bad-file-silent" not in m:
                self.irefusal = "Pick an Outlook archive (.olm) or a Thunderbird or Unix mailbox file (.mbox)."
        elif ax in (I + ".choose-mbox", I + ".choose-olm"):
            if "imp-choose-empties" in m:
                self.scanned = []
        elif ax == I + ".open-settings":
            if "imp-fda-button-dead" not in m:
                self.fda_opened = True
        elif ax == I + ".start":
            if "imp-start-dead" not in m:
                self.irun, self.istate, self.idone = "run-7", "running", 0
        elif ax == I + ".cancel":
            if "imp-cancel-dead" not in m:
                self.istate = "cancelled"
                self.istored = self.idone - self.idone // 25       # the cancelled run keeps what it stored
                self.scanned[0]["imported"] += 0 if "imp-history-lost" in m else self.istored
        elif ax in (I + ".close", I + ".scrim"):
            if ax.endswith("close") and "imp-close-dead" in m or ax.endswith("scrim") and "imp-scrim-dead" in m:
                return
            self.isheet = False
        else:
            raise KeyError(ax)

    def imp_rows(self, ax):
        I = A + ".import"
        if ax == I + ".sheet":
            fda = any(x["kind"] == self.isel and x["access"] == "needs_full_disk_access" for x in self.scanned) and "imp-fda-silent" not in self.m
            return [{"open": self.isheet, "running": self.istate == "running", "selected": self.isel or "", "needs_full_disk_access": fda,
                     "fda_opened": self.fda_opened, "refusal": self.irefusal, "state": self.istate}]
        if ax == I + ".sources":
            return [{"kind": x["kind"], "count": None if "imp-one-source" in self.m and False else x["count"], "imported": x["imported"], "selected": x["kind"] == self.isel}
                    for x in self.scanned]
        if ax == I + ".result":
            line = ""
            if self.istate == "cancelled":
                line = "Import cancelled. What was stored stays."
            elif self.istate == "finished":
                line = "%s messages imported into Imported" % ("{:,}".format(self.istored).replace(",", ".") if "imp-result-bare" not in self.m else "some")
            prog = "" if self.istate != "running" else ("%d of %d · INBOX" % (self.idone, self.scanned[0]["count"]) if "imp-progress-bare" not in self.m else "reading")
            return [{"line": line, "progress": prog, "done": self.idone, "stored": self.istored}]
        if ax == WELCOME_PROGRESS:
            return [{"run_id": self.irun or "run-7"}]
        raise KeyError(ax)

    def handle(self, verb, args):
        if getattr(self, "late", 0):
            self.late -= 1
            if self.late == 0:
                self.focus = None      # the mutant: the closed page's onDisappear lands after the door
        try:
            r = getattr(self, "v_" + verb)(*args)
            return r if isinstance(r, dict) else {"ok": True}
        except KeyError as e:
            return {"ok": False, "error": "no rows at %s" % e}
        except Exception as e:
            return {"ok": False, "error": "%s: %s" % (type(e).__name__, e)}

    def v_winframes(self):
        return {"ok": True, "frames": [{"title": "Settings", "x": 100, "y": 80, "w": 940, "h": 680, "kind": "window", "id": "mac-settings-window", "surface": ""}]}

    def v_uistate(self):
        return {"ok": True, "state": {"surface": "mac-settings-accounts", "first_responder": "other:field" if self.focus else "list", "overlay": None, "tab": "primary"}}

    def v_rows(self, ax):
        if ax == "mac-keyevent.monitor":
            return {"ok": True, "rows": [{"seen": 0 if "key-undelivered" in self.m else self.keys_sent, "sent": self.keys_sent, "last": {"route": "window", "handled": True, "key_before": True, "key_after": True, "window_id": "mac-settings-window",
                                                                         "windows": [{"id": "mac-settings-window", "key": True, "focus": "NSView"}]}}]}
        if ax.startswith(A + ".import.") or ax == WELCOME_PROGRESS:
            return {"ok": True, "rows": self.imp_rows(ax)}
        if ax == "mac-settings-general.settings":
            return {"ok": True, "rows": [{"unread_only": self.unread_only}]}
        if ax == A + ".accounts":
            vis = [a for a in self.accounts if self.query.lower() in (a["name"] + a["address"]).lower()]
            return {"ok": True, "rows": [dict(a, provider="IMAP") for a in vis]}
        if ax == A + ".field":
            f = self.focus
            i = f.partition(":")[2] if f else ""
            return {"ok": True, "rows": [{"focus": f or "none", "name_draft": self.name_draft.get(i, ""), "find_text": self.query,
                                           "signature_draft": self.sig_draft.get(i, "")}]}
        if ax == A + ".icon.stored":
            return {"ok": True, "rows": [{"account": i, "px": 128, "bytes": 1000} for i in sorted(self.stored)]}
        if ax == IMPORT + ".panel":
            return {"ok": True, "rows": [{"showing": self.import_open, "from_settings": self.import_from}]}
        raise KeyError(ax)

    def v_measure(self, ax):
        if ax == A + ".import.sheet":
            if self.isheet and "imp-undrawn" not in self.m:
                return {"ok": True, "frame": {"x": 0, "y": 0, "w": 400, "h": 300}, "window": "mac-settings-window"}
            return {"ok": False, "error": "no measurement at %s" % ax}
        if (ax == A + ".remove.review" and self.removing) or (ax == A + ".picker" and self.editing):
            return {"ok": True, "frame": {"x": 0, "y": 0, "w": 300, "h": 200}, "window": "mac-settings-window"}
        return {"ok": False, "error": "no measurement at %s" % ax}

    def v_type(self, text):
        f, m = self.focus, self.m
        if f is None:
            return
        kind, _, i = f.partition(":")
        if kind == "name" and "name-typing-lost" not in m:
            self.name_draft[i] = self.name_draft.get(i, self.acc(i)["name"]) + text
        elif kind == "signature" and "signature-typing-lost" not in m:
            self.sig_draft[i] = self.sig_draft.get(i, self.acc(i)["signature"]) + text
        elif f == "find" and "find-typing-lost" not in m:
            self.query += text

    def v_key(self, chord):
        pass       # the `key` verb answers the shortcut registry only (AppModel.press): the review's Escape is a SwiftUI cancel button it cannot reach

    def v_keyevent(self, *args):
        """A real key event (KeyEventVerb): it reaches the review's cancel button."""
        # like the hidden rig: the window is never key, so only arm d (which makes it key first) reaches the cancel button
        if args == ("reset",):
            self.keys_sent = 0
            return
        self.keys_sent += 1
        if "key-undelivered" in self.m:       # the hidden rig as it was on main81: the window never becomes key, no monitor sees the event, nothing closes
            return
        if args and args[-1] == "Escape" and "arm:d" in args and self.removing and "remove-escape-dead" not in self.m:
            self.removing = None

    def v_click(self, ax):
        m = self.m
        if ax in ("mac-settings-window.pane-row.notifications", "mac-settings-window.pane-row.accounts"):
            if self.page and "page-close-clears-find-focus" in m:
                self.late = 2
            self.page, self.focus = None, None
            return
        if ax == "mac-settings-window.open":
            return
        if ax == "mac-settings-general.notch.show":
            if "show-does-nothing" not in m:
                self.unread_only = not self.unread_only
            return
        if ax == A + ".find":
            if "find-door-dead" not in m:
                self.focus = "find"
            return
        if ax == A + ".fixture.find.":
            self.query = ""
            return
        if ax == IMPORT + ".reset":
            self.import_open = self.import_from = False
            return self.imp_click(WELCOME_RESET)
        if ax == A + ".import":
            if "import-dead" not in m:
                self.import_open = self.import_from = True
            return self.imp_click(ax)
        if ax.startswith(A + ".import."):
            return self.imp_click(ax)
        if ax == A + ".remove.scrim":
            if self.removing and "remove-scrim-dead" not in m:
                self.removing = None
            return
        if ax == A + ".picker.scrim":
            if self.editing and "picker-scrim-dead" not in m:
                self.editing = None
            return
        if ax == A + ".picker.done":
            self.editing = None
            return
        if ax == A + ".upload":
            if self.editing and "upload-dead" not in m:
                self.stored.add(self.editing)
            return
        if ax.startswith(A + ".acc-"):
            i, _, rest = ax[len(A) + 1:].partition(".")
            a = self.acc(i)
            if rest == "":
                self.page = i
            elif rest.startswith("delivery."):
                k = rest.split(".")[1]
                if "delivery-%s-unsaved" % k not in m:
                    a["delivery"] = k
            elif rest in ("name.field", "signature.field"):
                if self.page == i and ("%s-door-dead" % rest.split(".")[0]) not in m:
                    self.focus = "%s:%s" % (rest.split(".")[0], i)
            elif rest == "name.field.submit":
                if i in self.name_draft and "name-return-dead" not in m:
                    a["name"] = self.name_draft[i]
            elif rest == "signature.field.submit":
                if i in self.sig_draft and "signature-return-dead" not in m:
                    a["signature"] = self.sig_draft[i]
            elif rest == "remove":
                if "remove-opens-nothing" not in m:
                    self.removing = i
            elif rest == "icon":
                if "icon-opens-nothing" not in m:
                    self.editing = i
            else:
                raise KeyError(ax)
            return
        raise KeyError(ax)


class _Handler(socketserver.StreamRequestHandler):
    def handle(self):
        for line in self.rfile:
            req = json.loads(line.decode())
            self.wfile.write((json.dumps(self.server.app.handle(req["verb"], req.get("args", []))) + "\n").encode())


def _run_against(mutants):
    d = tempfile.mkdtemp(prefix="wsg", dir="/tmp")
    srv = socketserver.ThreadingUnixStreamServer(os.path.join(d, "whoosh.sock"), _Handler)
    srv.app = FakeApp(mutants)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    ctl = Ctl(d).connect()
    try:
        j = Journey(ID, ctl)
        drive_doors(j)
        drive_import(j)
        return j.checks()
    finally:
        ctl.close()
        srv.shutdown()
        srv.server_close()
        shutil.rmtree(d, ignore_errors=True)


def selftest():
    clean = _run_against(())
    bad_clean = [c for c in clean if c.state != "pass"]
    print("clean run: %d checks, %d not passing" % (len(clean), len(bad_clean)))
    for c in bad_clean:
        print("  CLEAN %s %s: %s" % (c.state.upper(), c.label, c.detail))
    labels = {c.label.split("/", 1)[1] for c in clean}
    missing = sorted(labels - set(LABEL_MUTANT))
    stray = sorted(set(LABEL_MUTANT) - labels)
    print("labels without a mutant: %s; mutants without a label: %s" % (missing or "none", stray or "none"))
    not_red, with_ev, no_ev = 0, 0, []
    print("| check | mutant | observed |")
    print("|---|---|---|")
    for m in sorted(set(LABEL_MUTANT.values())):
        res = {c.label.split("/", 1)[1]: c for c in _run_against((m,))}
        for label, mm in LABEL_MUTANT.items():
            if mm != m:
                continue
            c = res.get(label)
            red = c is not None and c.state == "fail"
            not_red += 0 if red else 1
            print("| J6.7/%s | %s | %s |" % (label, m, "RED: " + c.detail if red else "NOT RED (%s)" % (c.state if c else "missing")))
            if red and label in EVIDENCE_CASES:
                with_ev += 1
                if "| evidence: " not in c.detail:
                    no_ev.append(label)
    # a key the rig never delivered is not an app verdict: the case says so (not judged), it is neither green nor red
    und = {c.label.split("/", 1)[1]: c for c in _run_against(("key-undelivered",))}.get("remove-escape-cancels-the-review-and-keeps-the-account")
    und_ok = und is not None and und.state == "unjudged" and "reached no key monitor" in und.detail
    print("undelivered Escape: %s" % ("not judged, with its reason" if und_ok else "WRONG: %s" % (und.state if und else "missing")))
    not_red += 0 if und_ok else 1
    print("evidence on red: %d of %d evidence cases print an `evidence:` part on their mutant (missing: %s)" % (with_ev - len(no_ev), len(EVIDENCE_CASES), no_ev or "none"))
    ok = not bad_clean and not missing and not stray and not_red == 0 and not no_ev and with_ev == len(EVIDENCE_CASES)
    print("selftest: %s (%d mutants not red)" % ("OK" if ok else "FAILED", not_red))
    return 0 if ok else 1


def main():
    if "--selftest" in sys.argv:
        return selftest()
    path = os.path.abspath(__file__)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    rows = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# J") and " | " in l]
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
    print_cases(r)                                # ONE line per case: "<case> pass|fail|unjudged <detail>"
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    ok = r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"]
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
