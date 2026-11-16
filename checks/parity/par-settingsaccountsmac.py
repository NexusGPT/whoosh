#!/usr/bin/env python3
"""par-settingsaccountsmac: Settings > Accounts in the sidebar-and-cards window: the list, an account's own page, the icon picker,
sign in again, pause, remove with its review, and Add account.

Drives the RUNNING rig copy (docs/CONTROL-SOCKET.md). Expected values come from the design fixture (docs/design/fixtures/mac-main-sample.json:
30 accounts in the groups Work, Personal, Clients and Side; acc-18 Provanta RFP needs a sign-in) and docs/CONSTANTS.md, never from a built screen.
Run (only in a window announced to the owner): sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/par-settingsaccountsmac.py'
python3 checks/parity/par-settingsaccountsmac.py --selftest   the sweep step against a model of the screen, clean, then once per mutant (no app is launched)
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0.
"""
import json
import os
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, print_cases, print_model_cases # noqa: E402
from lib.ctl import Unreadable                                       # noqa: E402

ID = "par-settingsaccountsmac"
TITLE = "Settings > Accounts: needs-you on top, groups, an account's page, picker, sign in again, remove review, Add account"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "the account that needs you is the first card with its button; a click opens its page; each page control changes the account"
W, A = "mac-settings-window", "mac-settings-accounts"
FX = json.load(open(os.path.join(ROOT, "docs", "design", "fixtures", "mac-main-sample.json"), encoding="utf-8"))
ACCOUNTS = FX["accounts"]
FAILING = [a for a in ACCOUNTS if a.get("status") == "sign in again"]
GROUPS = []
for _a in ACCOUNTS:
    if _a.get("group") and _a["group"] not in GROUPS:
        GROUPS.append(_a["group"])
FIRST = ACCOUNTS[0]


def click(s, ax):
    try:
        s.ctl.click(ax)
        return True
    except Unreadable as e:
        return str(e)


def place(s):
    return s.ctl.rows(W + ".place")[0]


def frame(s, ax):
    return s.ctl.measure(ax)["frame"]


def drawn(s, ax):
    ctl = s.ctl                       # raises in a dry run: not judged, not failed
    try:
        return ctl.measure(ax)["frame"]["h"] > 0
    except (Unreadable, KeyError):
        return False


def row(s, acc):
    return [r for r in s.ctl.rows(A + ".accounts") if r["id"] == acc][0]


def drive(j):
    s = j.step(ID, "The list: needs you first, then the groups")
    s.click(W + ".open")
    s.click("mac-settings-general.appearance.language.en")      # the words below are English whatever the Mac speaks
    s.click(W + ".pane-row.accounts")
    s.expect("thirty-accounts-are-listed", lambda: len(s.ctl.rows(A + ".accounts")), len(ACCOUNTS))
    s.expect("groups-in-the-fixture-order", lambda: [g for g in dict.fromkeys(r["group"] for r in s.ctl.rows(A + ".accounts") if r["group"])], GROUPS)
    s.expect("the-account-that-needs-you-has-its-own-card", lambda: [(r["name"], r["sign-in-again"]) for r in s.ctl.rows(A + ".failing")],
             [(a["name"], "%s.%s.sign-in-again" % (A, a["id"])) for a in FAILING])
    s.expect("needs-you-card-is-above-the-working-list", lambda: drawn(s, A + ".failing") and frame(s, A + ".failing")["y"] < frame(s, A + ".accounts")["y"], True)
    s.expect("header-has-add-account-with-its-key", lambda: (drawn(s, A + ".add"), drawn(s, A + ".move-to-iphone")), (True, True))
    s.expect("every-account-row-says-address-and-provider", lambda: [r["id"] for r in s.ctl.rows(A + ".accounts")
                                                                    if not (r["address"] and r["provider"])], [])
    s.click(A + ".fixture.find.provanta")
    s.expect("find-narrows-the-list", lambda: [r["id"] for r in s.ctl.rows(A + ".accounts")] != [] and
             all("provanta" in (r["name"] + r["address"]).lower() for r in s.ctl.rows(A + ".accounts")), True)
    s.click(A + ".fixture.find.zzzz")
    s.expect("find-with-no-match-says-so", lambda: (s.ctl.rows(A + ".accounts"), frame(s, A + ".find.empty")["h"] > 0), ([], True))
    s.click(A + ".fixture.find.")
    drive_services(j, ())

    s = j.step(ID, "An account's own page")
    s.click("%s.%s" % (A, FIRST["id"]))
    s.expect("a-row-click-opens-the-page", lambda: (place(s)["pane"], place(s)["account"]), ("accounts", FIRST["id"]))
    s.expect("the-toolbar-names-account-and-page", lambda: place(s)["title"], "Accounts › " + FIRST["name"])
    s.expect("the-page-has-its-head", lambda: drawn(s, "%s.%s.head" % (A, FIRST["id"])), True)
    s.expect("address-and-provider-are-shown", lambda: (drawn(s, "%s.%s.address" % (A, FIRST["id"])), drawn(s, "%s.%s.provider" % (A, FIRST["id"]))), (True, True))
    p = "%s.%s.pause" % (A, FIRST["id"])
    s.click(p)
    s.expect("pause-syncing-pauses", lambda: row(s, FIRST["id"])["paused"], True)
    s.click(p)
    s.expect("pause-syncing-resumes", lambda: row(s, FIRST["id"])["paused"], False)
    n = "%s.%s.notifications" % (A, FIRST["id"])
    s.click(n)
    s.expect("banners-and-sounds-off", lambda: row(s, FIRST["id"])["notifications"], "off")
    s.click("%s.%s.delivery.notch" % (A, FIRST["id"]))
    s.expect("delivery-notch-only-is-saved-through-the-core", lambda: row(s, FIRST["id"])["delivery"], "notch")
    s.click(n)
    s.expect("banners-and-sounds-on", lambda: row(s, FIRST["id"])["notifications"], "on")
    s.click("%s.%s.fixture.rename" % (A, FIRST["id"]))
    s.expect("renaming-changes-the-name-and-the-toolbar", lambda: (row(s, FIRST["id"])["name"], place(s)["title"]), ("Renamed", "Accounts › Renamed"))
    s.click("%s.%s.fixture.signature" % (A, FIRST["id"]))
    s.expect("signature-is-saved-through-the-core", lambda: row(s, FIRST["id"])["signature"], "Best regards")
    s.click("%s.%s.icon" % (A, FIRST["id"]))
    s.expect("icon-and-colour-opens-the-picker", lambda: drawn(s, A + ".picker"), True)
    glyph = lambda: row(s, FIRST["id"])["icon"]
    before = {}
    s.expect("picking-a-glyph-changes-the-mark", lambda: (before.update(g=glyph()),
                                                          [click(s, A + ".icon-library.%s" % g) for g in ("star", "heart", "bolt") if g != before["g"]][:1],
                                                          glyph() != before["g"])[2], True)
    other = [a["colour"] for a in ACCOUNTS if a["colour"].lower() != FIRST["colour"].lower()][0]
    s.click(A + ".colour." + other.lstrip("#"))
    s.expect("picking-a-colour-changes-the-account-colour", lambda: row(s, FIRST["id"])["colour"].lower(), other.lower())
    s.click(A + ".picker.done")
    time.sleep(0.5)
    s.expect("done-closes-the-picker", lambda: drawn(s, A + ".picker"), False)
    s.click(W + ".back")
    s.expect("back-returns-to-the-list", lambda: (place(s)["pane"], place(s)["account"]), ("accounts", None))
    s.click("%s.%s" % (A, FIRST["id"]))
    s.key("⌘[")
    s.expect("cmd-bracket-returns-too", lambda: place(s)["account"], None)

    s = j.step(ID, "Sign in again")
    f = FAILING[0]
    s.click("%s.%s" % (A, f["id"]))
    s.expect("the-failing-page-says-what-is-needed", lambda: drawn(s, "%s.%s.sign-in-again" % (A, f["id"])), True)
    s.click("%s.%s.sign-in-again" % (A, f["id"]))
    s.expect("sign-in-again-clears-the-failure", lambda: (s.ctl.rows(A + ".failing"), s.ctl.rows(W + ".summary")[0]["needs_you"]), ([], 0))
    s.expect("the-sidebar-badge-goes-with-it", lambda: [r["badge"] for r in s.ctl.rows(W + ".panes") if r["id"] == "accounts"], [0])
    s.click(W + ".back")

    s = j.step(ID, "Remove account asks first")
    s.click("%s.%s" % (A, FIRST["id"]))
    s.click("%s.%s.remove" % (A, FIRST["id"]))
    s.expect("remove-opens-a-review-first", lambda: drawn(s, A + ".remove.review"), True)
    s.click(A + ".remove.cancel")
    s.expect("cancel-leaves-the-account", lambda: (click(s, A + ".remove.cancel") is not True, len(s.ctl.rows(A + ".accounts"))), (True, len(ACCOUNTS)))
    s.click("%s.%s.remove" % (A, FIRST["id"]))
    s.expect("the-review-says-what-goes-in-the-cores-words", lambda: [r["text"].startswith("Removes Renamed from this Mac") for r in s.ctl.rows(A + ".remove.words")], [True])   # the account was renamed above
    s.click(A + ".remove.confirm")
    s.expect("confirm-removes-the-account-through-the-core", lambda: ([r["id"] for r in s.ctl.rows(A + ".accounts") if r["id"] == FIRST["id"]], len(s.ctl.rows(A + ".accounts")), place(s)["account"]),
             ([], len(ACCOUNTS) - 1, None))
    s.click(W + ".pane-row.accounts")

    s = j.step(ID, "Add account")
    s.click(A + ".add")
    s.expect("add-account-opens-the-approved-sheet", lambda: place(s)["adding"], True)
    s.click("mac-add-account.cancel")
    s.expect("cancel-closes-the-sheet", lambda: place(s)["adding"], False)
    s.click(W + ".open")                           # the sheet's own click put the surface on the sheet; Settings holds the focus again
    s.key("⌘N")
    s.expect("cmd-n-opens-it-too", lambda: place(s)["adding"], True)
    s.click("mac-add-account.cancel")
    # In hidden rig mode a window opened later is not listed by `winframes`, so the proof here is that the control reaches the shell's opener
    # (the click answers ok only when WindowBridge.open found its opener); the window itself is read in the owner's first look.
    s.expect("move-to-iphone-reaches-the-welcome-opener", lambda: click(s, A + ".move-to-iphone"), True)
    s.click(W + ".open")
    s.click("mac-settings-general.appearance.language.system")
    drive_sweep(j)


SV = ID + "-services"
# The five mail services in the order the pane lists them, the route the fixture names for each account's provider (the app keeps the same
# route: core `MailProvider`, SettingsAccountsMac/ServiceStatus.swift), and the words of each state. Nothing here is read from a built screen.
SERVICES = [("gmail", "Gmail"), ("outlook", "Outlook.com"), ("icloud", "iCloud"), ("fastmail", "Fastmail"), ("zoho", "Zoho")]
ROUTE = {"Google": "gmail", "Microsoft": "outlook", "iCloud": "icloud", "Fastmail": "fastmail"}
STATE_WORDS = {"no-account": "No account on this Mac", "connected": "Connected", "needs-sign-in": "Needs sign-in"}


def expected_service(sid, gone=()):
    """The state the fixture's accounts of one service call for: none, a sign-in needed (acc-18 Provanta RFP, a Microsoft account), else connected."""
    mine = [a for a in ACCOUNTS if a["id"] not in gone and ROUTE.get(a["provider"]) == sid]
    if not mine:
        return "no-account"
    return "needs-sign-in" if any(a.get("status") == "sign in again" for a in mine) else "connected"


def service_rows(s):
    return s.ctl.rows(A + ".service-status")


def drive_services(j, gone):
    """Mail service status: Settings > Accounts names Gmail, Outlook.com, iCloud, Fastmail and Zoho, each with what THIS Mac's accounts of that
    service report, in words (the state is this Mac's own; nothing is fetched from a service). `gone` are the fixture accounts already removed."""
    s = j.step(SV, "Mail service status")
    s.click(W + ".open")
    s.click("mac-settings-general.appearance.language.en")
    s.click(W + ".pane-row.accounts")
    s.expect("service-status-rows-come-in-the-fixed-order", lambda: [r["name"] for r in service_rows(s)], [n for _i, n in SERVICES])
    for sid, name in SERVICES:
        want = STATE_WORDS[expected_service(sid, gone)]
        s.expect("service-status-%s-is-named-with-its-state" % sid,
                 lambda sid=sid: next(((r["name"], r["words"]) for r in service_rows(s) if r["id"] == sid), None), (name, want))
    s.expect("an-account-that-needs-a-sign-in-is-not-shown-connected",
             lambda: [r["words"] for r in service_rows(s) if r["id"] == "outlook"] not in ([], [STATE_WORDS["connected"]]), True)
    s.expect("a-service-with-no-account-says-so-and-is-not-shown-connected",
             lambda: [r["words"] for r in service_rows(s) if r["id"] == "zoho"], [STATE_WORDS["no-account"]])


SW = ID + "-sweep"
SECOND = [a for a in ACCOUNTS if a["id"] != FIRST["id"] and a not in FAILING][0]
# The picker offers the colours of the accounts that are still listed (AccountIconPicker.swift:13-17) and registers one door per such colour
# (SettingsAccountsMacFeature.swift:106-124): the first account is removed in the cases above, so a colour only it carried has no swatch left.
DIST_COLOURS = list(dict.fromkeys(a["colour"] for a in ACCOUNTS if a["id"] != FIRST["id"]))
# The picker's glyph doors are the emblem ids (EmblemLibrary.all); the fixture stores the ring library's 20 ids, which the app maps through
# EmblemLibrary.aliases (Sources/Design/Emblem/EmblemFamily.swift:39-42). A door exists under the emblem id only.
ALIASES = {"chart": "bar-chart", "pen": "pencil", "sail": "sailboat", "mail": "envelope", "cup": "coffee", "plane": "airplane", "code": "toolbox", "phone": "chat"}
DIST_ICONS = list(dict.fromkeys(ALIASES.get(a["icon_id"], a["icon_id"]) for a in ACCOUNTS if a.get("icon_id")))


def fld(s):
    return s.ctl.rows(A + ".field")[0]


def sheet(s):
    return s.ctl.rows(A + ".import.sheet")[0]


def settle():
    time.sleep(0.4)                                   # the sheet, the review and the picker fade out before they leave the frame


def drive_sweep(j):
    """The sweep: every control of Settings > Accounts that no case above clicks with its outcome, clicked once through the socket door the screen
    registers (SettingsAccountsMacFeature.clickTargets, SettingsWindowRegistry.clickTargets), the outcome read from a row the app already serves.
    It runs on the fixture copy, on the second account, and puts back what a door can put back: the language, the sheet, the picker, the colour,
    the mark, the delivery. A typed name or signature has no door that restores it (finding in docs/lanes/sweep-settingsaccounts.md)."""
    X = SECOND["id"]
    ax = lambda tail: "%s.%s.%s" % (A, X, tail)
    st = {}
    s = j.step(SW, "Every control clicked once")
    s.click(W + ".open")
    s.click("mac-settings-general.appearance.language.en")
    s.click(W + ".pane-row.accounts")
    # -- the header: Import mail and its sheet
    s.click(A + ".import")
    s.expect("import-button-opens-the-import-sheet", lambda: sheet(s)["open"], True)
    s.click(A + ".import.choose-olm")
    s.click(A + ".import.choose-mbox")
    s.expect("choose-a-file-adds-its-source-and-selects-it",
             lambda: ({"olm", "mbox"} <= {r["kind"] for r in s.ctl.rows(A + ".import.sources")}, sheet(s)["selected"]), (True, "mbox"))
    for kind in ("olm", "apple-mail", "mbox"):
        s.click("%s.import.source.%s" % (A, kind))
        s.expect("source-row-%s-selects-it" % kind, lambda kind=kind: sheet(s)["selected"], kind)
    s.click(A + ".import.choose-bad")
    s.expect("choose-a-bad-file-is-refused-in-words", lambda: sheet(s)["refusal"] != "", True)
    s.click(A + ".import.close")
    s.expect("import-close-button-closes-the-sheet", lambda: sheet(s)["open"], False)
    s.click(A + ".import")
    s.click(A + ".import.scrim")
    s.expect("import-scrim-closes-the-sheet", lambda: sheet(s)["open"], False)
    # -- the find field
    s.click(A + ".find")
    s.expect("find-field-click-focuses-the-field", lambda: fld(s)["focus"], "find")
    s.type("prov")
    s.expect("typing-in-the-find-field-narrows-the-list",
             lambda: (fld(s)["find_text"], s.ctl.rows(A + ".accounts") != [] and all("prov" in (r["name"] + r["address"]).lower() for r in s.ctl.rows(A + ".accounts"))),
             ("prov", True))
    s.click(A + ".find.clear")
    s.expect("find-clear-empties-the-find-field", lambda: (fld(s)["find_text"], len(s.ctl.rows(A + ".accounts"))), ("", len(ACCOUNTS) - 1))
    # -- another account's page: its two text fields, the delivery choices
    s.click("%s.%s" % (A, X))
    s.expect("second-account-row-opens-its-page",
             lambda: (st.update(icon=row(s, X)["icon"], colour=row(s, X)["colour"]), place(s)["pane"], place(s)["account"])[1:], ("accounts", X))
    s.click(ax("name.field"))
    s.expect("name-field-click-focuses-it", lambda: fld(s)["focus"], "name:" + X)
    s.type("!")
    s.expect("typing-in-the-name-field-fills-the-draft", lambda: fld(s)["name_draft"], SECOND["name"] + "!")
    s.click(ax("name.field.submit"))
    s.expect("name-return-saves-the-name", lambda: row(s, X)["name"], SECOND["name"] + "!")
    s.click(ax("signature.field"))
    s.expect("signature-field-click-focuses-it", lambda: fld(s)["focus"], "signature:" + X)
    s.type("!")
    s.expect("typing-in-the-signature-field-fills-the-draft", lambda: fld(s)["signature_draft"], SECOND["signature"] + "!")
    s.click(ax("signature.field.submit"))
    s.expect("signature-return-saves-the-signature", lambda: row(s, X)["signature"], SECOND["signature"] + "!")
    s.click(ax("delivery.notifications"))
    s.expect("delivery-notifications-only-is-saved", lambda: row(s, X)["delivery"], "notifications")
    s.click(ax("delivery.both"))
    s.expect("delivery-both-is-saved", lambda: row(s, X)["delivery"], "both")
    # -- the review of a removal, closed by its scrim
    s.click(ax("remove"))
    s.expect("remove-opens-the-review-on-another-account", lambda: drawn(s, A + ".remove.review"), True)
    s.click(A + ".remove.scrim")
    settle()
    s.expect("remove-scrim-closes-the-review-and-keeps-the-account", lambda: (drawn(s, A + ".remove.review"), len(s.ctl.rows(A + ".accounts"))), (False, len(ACCOUNTS) - 1))
    # -- the picker: swatches, glyphs, the upload, then the mark and the colour put back
    s.click(ax("icon"))
    s.expect("icon-button-opens-the-picker-on-another-account", lambda: drawn(s, A + ".picker"), True)

    def swatches():
        bad = []
        for hx in DIST_COLOURS:
            r = click(s, A + ".colour." + hx.lstrip("#"))
            if r is not True or row(s, X)["colour"].lower() != hx.lower():
                bad.append(hx)
        return bad
    s.expect("every-colour-swatch-sets-the-colour", swatches, [])

    def glyphs():
        bad, prev = [], row(s, X)["icon"]
        for g in DIST_ICONS:
            if g == prev:
                continue
            r = click(s, A + ".icon-library." + g)
            now = row(s, X)["icon"]
            if r is not True or now == prev:
                bad.append(g)
            prev = now
        return bad
    s.expect("every-fixture-glyph-changes-the-mark", glyphs, [])
    s.click(A + ".upload")
    s.expect("upload-button-stores-the-picture-and-marks-uploaded",
             lambda: (row(s, X)["icon"], X in [r["account"] for r in s.ctl.rows(A + ".icon.stored")]), ("uploaded", True))
    s.click("%s.icon-library.%s" % (A, st.get("icon", "")))
    s.expect("picking-a-glyph-after-an-upload-restores-the-mark", lambda: row(s, X)["icon"], st.get("icon"))
    # The second account's own first colour has no swatch once the swatch loop moved it away: the palette and its doors are the colours of the
    # listed accounts (AccountIconPicker.swift:13-17, SettingsAccountsMacFeature.swift:106-124), and no other account carries it. So the
    # case picks a swatch again that other accounts carry (the first colour of the first other account) and reads that the second pick is kept.
    first_again = [a["colour"] for a in ACCOUNTS if a["id"] not in (FIRST["id"], X)][0]
    s.click("%s.colour.%s" % (A, first_again.lstrip("#")))
    s.expect("picking-the-first-colour-back-restores-it", lambda: row(s, X)["colour"].lower(), first_again.lower())
    s.click(A + ".picker.scrim")
    settle()
    s.expect("picker-scrim-closes-the-picker", lambda: drawn(s, A + ".picker"), False)
    # -- back to the list, the language back to the Mac's
    s.click(W + ".back")
    s.expect("sweep-leaves-the-screen-as-it-found-it",
             lambda: (place(s)["pane"], place(s)["account"], place(s)["adding"], sheet(s)["open"]), ("accounts", None, False, False))
    s.click(W + ".open")
    s.click("mac-settings-general.appearance.language.system")


# MUTANTS: per check, the mutation of the app that must turn it red.
# par-settingsaccountsmac/thirty-accounts-are-listed | the list stops at 12 accounts
# par-settingsaccountsmac/groups-in-the-fixture-order | the groups are sorted by name
# par-settingsaccountsmac/the-account-that-needs-you-has-its-own-card | the failing account stays in the working list
# par-settingsaccountsmac/needs-you-card-is-above-the-working-list | the needs-you card sits under the list
# par-settingsaccountsmac/header-has-add-account-with-its-key | the header has no Add account button
# par-settingsaccountsmac/every-account-row-says-address-and-provider | a row shows the name only
# par-settingsaccountsmac/find-narrows-the-list | the find field changes nothing
# par-settingsaccountsmac/find-with-no-match-says-so | no match shows an empty list and no words
# par-settingsaccountsmac/a-row-click-opens-the-page | a row is not a button
# par-settingsaccountsmac/the-toolbar-names-account-and-page | the toolbar keeps the pane name only
# par-settingsaccountsmac/the-page-has-its-head | the page has no head card
# par-settingsaccountsmac/address-and-provider-are-shown | the page hides the address
# par-settingsaccountsmac/pause-syncing-pauses | the pause switch does nothing
# par-settingsaccountsmac/pause-syncing-resumes | a second click does not resume
# par-settingsaccountsmac/picking-a-colour-changes-the-account-colour | a colour pick is not sent to the core
# par-settingsaccountsmac/delivery-notch-only-is-saved-through-the-core | the delivery choice is not sent to the core
# par-settingsaccountsmac/banners-and-sounds-off | the switch does not mute
# par-settingsaccountsmac/banners-and-sounds-on | the switch does not unmute
# par-settingsaccountsmac/renaming-changes-the-name-and-the-toolbar | the new name is not kept
# par-settingsaccountsmac/icon-and-colour-opens-the-picker | the button does nothing
# par-settingsaccountsmac/picking-a-glyph-changes-the-mark | a pick does not change the mark
# par-settingsaccountsmac/done-closes-the-picker | Done leaves the picker open
# par-settingsaccountsmac/back-returns-to-the-list | back leaves the account page up
# par-settingsaccountsmac/cmd-bracket-returns-too | ⌘[ is not bound
# par-settingsaccountsmac/the-failing-page-says-what-is-needed | the failing account has no Sign in again button
# par-settingsaccountsmac/sign-in-again-clears-the-failure | Sign in again leaves the account failing
# par-settingsaccountsmac/the-sidebar-badge-goes-with-it | the badge keeps counting
# par-settingsaccountsmac/remove-opens-a-review-first | Remove acts at once
# par-settingsaccountsmac/signature-is-saved-through-the-core | the signature is kept for the session and never sent to the core
# par-settingsaccountsmac/the-review-says-what-goes-in-the-cores-words | the review shows a fixed sentence, not the core's
# par-settingsaccountsmac/confirm-removes-the-account-through-the-core | Confirm closes the review and removes nothing
# par-settingsaccountsmac/cancel-leaves-the-account | Cancel removes the account
# par-settingsaccountsmac/add-account-opens-the-approved-sheet | the button opens nothing
# par-settingsaccountsmac/cancel-closes-the-sheet | Cancel leaves the sheet up
# par-settingsaccountsmac/cmd-n-opens-it-too | ⌘N is not bound
# par-settingsaccountsmac/move-to-iphone-reaches-the-welcome-opener | the button does nothing
# par-settingsaccountsmac-services/service-status-rows-come-in-the-fixed-order | the services are listed sorted by name
# par-settingsaccountsmac-services/service-status-gmail-is-named-with-its-state | the Gmail row is missing from the list
# par-settingsaccountsmac-services/service-status-outlook-is-named-with-its-state | the Outlook.com row is missing from the list
# par-settingsaccountsmac-services/service-status-icloud-is-named-with-its-state | the iCloud row is missing from the list
# par-settingsaccountsmac-services/service-status-fastmail-is-named-with-its-state | the Fastmail row is missing from the list
# par-settingsaccountsmac-services/service-status-zoho-is-named-with-its-state | the Zoho row is missing from the list
# par-settingsaccountsmac-services/an-account-that-needs-a-sign-in-is-not-shown-connected | every service is shown connected
# par-settingsaccountsmac-services/a-service-with-no-account-says-so-and-is-not-shown-connected | every service is shown connected
# par-settingsaccountsmac-sweep/import-button-opens-the-import-sheet | the Import mail button opens no sheet
# par-settingsaccountsmac-sweep/choose-a-file-adds-its-source-and-selects-it | Choose a file adds no source and selects nothing
# par-settingsaccountsmac-sweep/source-row-olm-selects-it | a click on the Outlook archive row does not select it
# par-settingsaccountsmac-sweep/source-row-apple-mail-selects-it | a click on the Apple Mail row does not select it
# par-settingsaccountsmac-sweep/source-row-mbox-selects-it | a click on the mbox row does not select it
# par-settingsaccountsmac-sweep/choose-a-bad-file-is-refused-in-words | a file that is not a mail archive is taken without a word
# par-settingsaccountsmac-sweep/import-close-button-closes-the-sheet | Close leaves the sheet open
# par-settingsaccountsmac-sweep/import-scrim-closes-the-sheet | a click outside the sheet leaves it open
# par-settingsaccountsmac-sweep/find-field-click-focuses-the-field | a click on the find field focuses nothing
# par-settingsaccountsmac-sweep/typing-in-the-find-field-narrows-the-list | the find field takes no typed text
# par-settingsaccountsmac-sweep/find-clear-empties-the-find-field | the clear door leaves the words in the find field
# par-settingsaccountsmac-sweep/second-account-row-opens-its-page | a row on the second account is not a button
# par-settingsaccountsmac-sweep/name-field-click-focuses-it | a click on the name field focuses nothing
# par-settingsaccountsmac-sweep/typing-in-the-name-field-fills-the-draft | the name field takes no typed text
# par-settingsaccountsmac-sweep/name-return-saves-the-name | Return in the name field saves nothing
# par-settingsaccountsmac-sweep/signature-field-click-focuses-it | a click on the signature field focuses nothing
# par-settingsaccountsmac-sweep/typing-in-the-signature-field-fills-the-draft | the signature field takes no typed text
# par-settingsaccountsmac-sweep/signature-return-saves-the-signature | Return in the signature field saves nothing
# par-settingsaccountsmac-sweep/delivery-notifications-only-is-saved | the Notifications only choice is not sent to the core
# par-settingsaccountsmac-sweep/delivery-both-is-saved | the Both choice is not sent to the core
# par-settingsaccountsmac-sweep/remove-opens-the-review-on-another-account | Remove account on the second account opens no review
# par-settingsaccountsmac-sweep/remove-scrim-closes-the-review-and-keeps-the-account | a click outside the review confirms the removal
# par-settingsaccountsmac-sweep/icon-button-opens-the-picker-on-another-account | Icon and colour on the second account opens no picker
# par-settingsaccountsmac-sweep/every-colour-swatch-sets-the-colour | a swatch does not change the account colour
# par-settingsaccountsmac-sweep/every-fixture-glyph-changes-the-mark | a glyph pick does not change the mark
# par-settingsaccountsmac-sweep/upload-button-stores-the-picture-and-marks-uploaded | Choose an image stores nothing
# par-settingsaccountsmac-sweep/picking-a-glyph-after-an-upload-restores-the-mark | a glyph pick after an upload leaves the uploaded picture
# par-settingsaccountsmac-sweep/picking-the-first-colour-back-restores-it | a second colour pick is not kept
# par-settingsaccountsmac-sweep/picker-scrim-closes-the-picker | a click outside the picker leaves it open
# par-settingsaccountsmac-sweep/sweep-leaves-the-screen-as-it-found-it | back leaves the account page up


def main():
    path = os.path.abspath(__file__)
    import lib.harness as H                       # keep the journeys, to say why a check was not judged
    made, orig = [], H.Journey.__init__
    def init(self, *a, **k):
        orig(self, *a, **k)
        made.append(self)
    H.Journey.__init__ = init
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    rows_ = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# par-") and " | " in l]
    print("MUTANTS (a check is green only after it has been seen red on its mutant)")
    print("| check | mutant |")
    print("|---|---|")
    for k, v in rows_:
        print("| %s | %s |" % (k, v))
    for line in r["fail_list"]:
        sys.stderr.write("FAIL %s\n" % line)
    for j in made:
        for c in j.checks():
            if c.state == "unjudged":
                sys.stderr.write("NOT JUDGED %s: %s\n" % (c.label, c.detail))
    for line in r["no_mutant"]:
        sys.stderr.write("NO MUTANT %s\n" % line)
    for line in r["stray_mutant"]:
        sys.stderr.write("STRAY MUTANT %s\n" % line)
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print_cases(r)                                # ONE line per case: "<case> pass|fail|unjudged <detail>", a pass is never read from silence
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    ok = r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"]
    return 0 if ok else 1


# ---- the selftest: the sweep against a model of the screen, clean (all green), then once per mutant (each must turn its own case red) ----
# SAModel answers the verbs drive_sweep uses, the way the Swift is written to answer them: SettingsAccountsMacFeature.clickTargets and rowSources
# (the sheet, the find field, the two text fields, the picker doors, the scrims), SettingsWindowRegistry.clickTargets (rows, delivery, Remove, Back)
# and AccountsSession (what each act changes). It starts where the cases above leave the app: the first account removed, the list showing.
# One mutant (named by its case) is switched on at a time. It judges the sweep step only; the cases above run on the app.
class SAModel:
    def __init__(self, mut):
        self.m = mut
        self.acc = {a["id"]: {"name": a["name"], "address": a["address"], "colour": a["colour"], "icon": a.get("icon_id") or "paper-plane",
                              "sig": a.get("signature", ""), "delivery": "both", "image": False} for a in ACCOUNTS if a["id"] != FIRST["id"]}
        self.page, self.adding, self.importing = None, False, False
        self.sources, self.sel, self.refusal = [], "", ""
        self.focus, self.drafts, self.query = None, {}, ""
        self.editing, self.removing = None, None

    def on(self, name): return name in self.m

    def visible(self):
        return [(i, a) for i, a in self.acc.items() if self.query.lower() in (a["name"] + a["address"]).lower()]

    def click(self, ax):
        on = self.on
        if ax == W + ".open" or ax.startswith("mac-settings-general."): return True
        if ax == W + ".pane-row.accounts": self.page = None; return True
        if ax == W + ".back":
            if self.page is None: return False
            if not on("sweep-leaves-the-screen-as-it-found-it"): self.page = None
            return True
        if ax == A + ".import":
            if not on("import-button-opens-the-import-sheet"):
                self.importing, self.sources, self.sel, self.refusal = True, ["apple-mail"], "apple-mail", ""
            return True
        if ax.startswith(A + ".import.choose-"):
            if not self.importing: return False
            kind = ax.rsplit("-", 1)[1]
            if kind == "bad":
                if not on("choose-a-bad-file-is-refused-in-words"): self.refusal = "That file is not a mail archive."
                return True
            if not on("choose-a-file-adds-its-source-and-selects-it"):
                self.sources.append(kind); self.sel = kind
            return True
        if ax.startswith(A + ".import.source."):
            kind = ax.rsplit(".", 1)[1]
            if not self.importing or kind not in self.sources: return False
            if not on("source-row-%s-selects-it" % kind): self.sel = kind
            return self.sel == kind
        if ax in (A + ".import.close", A + ".import.scrim"):
            if not self.importing: return False
            if not on(("import-close-button-closes-the-sheet" if ax.endswith("close") else "import-scrim-closes-the-sheet")): self.importing = False
            return True
        if ax == A + ".find":
            if not on("find-field-click-focuses-the-field"): self.focus = ("find",)
            return True
        if ax == A + ".find.clear":
            if not on("find-clear-empties-the-find-field"): self.query = ""
            return True
        if ax.startswith(A + ".acc-") or ax.startswith(A + ".remove") or ax.startswith(A + ".colour.") or ax.startswith(A + ".icon-library.") or ax.startswith(A + ".picker") or ax == A + ".upload":
            return self.click_account(ax)
        return False

    def click_account(self, ax):
        on = self.on
        tail = ax[len(A) + 1:]
        if tail == "remove.scrim":
            if self.removing is None: return False
            if on("remove-scrim-closes-the-review-and-keeps-the-account"): self.acc.pop(self.removing, None); self.page = None
            self.removing = None
            return True
        if tail == "picker.scrim":
            if self.editing is None: return False
            if not on("picker-scrim-closes-the-picker"): self.editing = None
            return True
        if tail == "upload":
            if self.editing is None: return False
            if not on("upload-button-stores-the-picture-and-marks-uploaded"): self.acc[self.editing]["image"] = True
            return True
        if tail.startswith("colour."):
            if self.editing is None: return False
            hx = "#" + tail.split(".", 1)[1]
            if hx not in {b["colour"] for b in self.acc.values()}: return False          # a door per colour of a listed account only
            if on("every-colour-swatch-sets-the-colour") or (on("picking-the-first-colour-back-restores-it") and hx.lower() == [a["colour"] for a in ACCOUNTS if a["id"] not in (FIRST["id"], SECOND["id"])][0].lower() and self.acc[self.editing]["colour"].lower() != hx.lower()): return True
            self.acc[self.editing]["colour"] = hx
            return True
        if tail.startswith("icon-library."):
            if self.editing is None: return False
            a = self.acc[self.editing]
            if tail.split(".", 1)[1] in ALIASES: return False                           # a door per emblem id: a ring alias has none
            if on("every-fixture-glyph-changes-the-mark"): return True
            if a["image"] and on("picking-a-glyph-after-an-upload-restores-the-mark"): return True
            a["icon"], a["image"] = tail.split(".", 1)[1], False
            return True
        parts = tail.split(".")
        if parts[0] not in self.acc: return False
        i, rest, a = parts[0], ".".join(parts[1:]), self.acc[parts[0]]
        if rest == "":
            if not on("second-account-row-opens-its-page"): self.page = i
            return True
        if rest == "remove":
            if not on("remove-opens-the-review-on-another-account"): self.removing = i
            return True
        if rest == "icon":
            if not on("icon-button-opens-the-picker-on-another-account"): self.editing = i
            return True
        if rest in ("name.field", "signature.field"):
            if self.page != i: return False
            f = rest.split(".")[0]
            if not on("%s-field-click-focuses-it" % f): self.focus = (f, i)
            return True
        if rest == "name.field.submit":
            d = self.drafts.get(("name", i))
            if d is None: return False
            if not on("name-return-saves-the-name"): a["name"] = d
            return True
        if rest == "signature.field.submit":
            d = self.drafts.get(("signature", i))
            if d is None: return False
            if not on("signature-return-saves-the-signature"): a["sig"] = d
            return True
        if rest.startswith("delivery."):
            k = rest.split(".")[1]
            if not on({"notifications": "delivery-notifications-only-is-saved", "both": "delivery-both-is-saved"}.get(k, "")): a["delivery"] = k
            return True
        return False

    def type(self, text):
        f = self.focus
        if f is None: return
        if f[0] == "find":
            if not self.on("typing-in-the-find-field-narrows-the-list"): self.query += text
        elif f[0] == "name":
            if not self.on("typing-in-the-name-field-fills-the-draft"): self.drafts[f] = self.drafts.get(f, self.acc[f[1]]["name"]) + text
        else:
            if not self.on("typing-in-the-signature-field-fills-the-draft"): self.drafts[f] = self.drafts.get(f, self.acc[f[1]]["sig"]) + text

    def rows(self, ax):
        if ax == W + ".place": return [{"pane": "accounts", "account": self.page, "adding": self.adding}]
        if ax == A + ".field":
            f = self.focus
            return [{"focus": "none" if f is None else (f[0] if f[0] == "find" else "%s:%s" % f), "find_text": self.query,
                     "name_draft": self.drafts.get(f, "") if f and f[0] == "name" else "", "signature_draft": self.drafts.get(f, "") if f and f[0] == "signature" else ""}]
        if ax == A + ".import.sheet": return [{"open": self.importing, "selected": self.sel, "refusal": self.refusal}]
        if ax == A + ".import.sources": return [{"kind": k} for k in self.sources]
        if ax == A + ".accounts": return [self.row(i, a) for i, a in self.visible()]
        if ax == A + ".service-status": return self.service_rows()
        if ax == A + ".icon.stored": return [{"account": i, "px": 64} for i, a in self.acc.items() if a["image"]]
        raise KeyError(ax)

    def service_rows(self):
        """The five rows the way ServiceStatus.swift builds them: this Mac's own accounts per service (the route the fixture names), a sign-in needed first."""
        out = []
        for sid, name in SERVICES:
            if self.on("service-status-%s-is-named-with-its-state" % sid):
                continue                                                                   # the broken copy drops the service's row
            mine = [a for a in ACCOUNTS if a["id"] in self.acc and ROUTE.get(a["provider"]) == sid]
            st = "no-account" if not mine else ("needs-sign-in" if any(a.get("status") == "sign in again" for a in mine) else "connected")
            if self.on("an-account-that-needs-a-sign-in-is-not-shown-connected") or self.on("a-service-with-no-account-says-so-and-is-not-shown-connected"):
                st = "connected"                                                           # the broken copy shows every service connected
            out.append({"id": sid, "name": name, "state": st, "words": STATE_WORDS[st], "accounts": len(mine)})
        if self.on("service-status-rows-come-in-the-fixed-order"):
            out.sort(key=lambda r: r["name"])
        return out

    @staticmethod
    def row(i, a):
        return {"id": i, "name": a["name"], "address": a["address"], "colour": a["colour"], "icon": "uploaded" if a["image"] else a["icon"],
                "signature": a["sig"], "delivery": a["delivery"]}

    def measure(self, ax):
        shown = {A + ".picker": self.editing is not None, A + ".remove.review": self.removing is not None}
        if not shown.get(ax): return None
        return {"ok": True, "frame": {"x": 0, "y": 0, "w": 100, "h": 40}}

    def handle(self, verb, args):
        try:
            if verb == "key": return {"ok": False, "error": "key does nothing"}
            if verb == "type": self.type(args[0]); return {"ok": True}
            if verb == "click": return {"ok": True} if self.click(args[0]) else {"ok": False, "error": "did nothing"}
            if verb == "rows": return {"ok": True, "rows": self.rows(args[0])}
            if verb == "measure":
                m = self.measure(args[0])
                return m if m else {"ok": False, "error": "not drawn"}
        except KeyError as e:
            return {"ok": False, "error": "no rows at %s" % e}
        return {"ok": False, "error": "nothing"}


def model_run(mut):
    """drive_sweep against one SAModel; returns {check label: (state, detail)}."""
    import json as _json
    import shutil
    import socketserver
    import tempfile
    import threading
    from lib.ctl import Ctl                                              # noqa: E402
    from lib.harness import Journey                                      # noqa: E402
    model = SAModel(mut)
    d = tempfile.mkdtemp(prefix="par-sa-sweep-")

    class H(socketserver.StreamRequestHandler):
        def handle(self):
            for line in self.rfile:
                q = _json.loads(line.decode())
                self.wfile.write((_json.dumps(model.handle(q["verb"], q["args"])) + "\n").encode())
    srv = socketserver.ThreadingUnixStreamServer(os.path.join(d, "whoosh.sock"), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    ctl = None
    try:
        ctl = Ctl(d).connect()
        j = Journey(ID, ctl)
        drive_services(j, (FIRST["id"],))      # the model starts where the cases above leave the app: the first account removed
        drive_sweep(j)
        return {c.label: (c.state, c.detail) for c in j.checks()}
    finally:
        if ctl is not None:
            ctl.close()
        srv.shutdown(); srv.server_close()
        shutil.rmtree(d, ignore_errors=True)


def selftest():
    clean = model_run(set())
    bad = 0
    for label, (state, why) in clean.items():
        if state != "pass":
            print("RED on the model: %s %s" % (label, why)); bad += 1
    print_model_cases(clean)
    print("model, no mutant: %d of %d sweep checks green" % (sum(1 for st, _ in clean.values() if st == "pass"), len(clean)))
    declared = {l.split(" | ")[0][2:].strip(): l.split(" | ", 1)[1].strip() for l in open(os.path.abspath(__file__), encoding="utf-8") if l.startswith(("# " + SW + "/", "# " + SV + "/"))}
    for label in clean:
        name = label.split("/", 1)[1]
        if label not in declared:
            print("NO MUTANT %s" % label); bad += 1; continue
        got = model_run({name})
        state = got[label][0]
        others = sorted(k.split("/", 1)[1] for k, (st, _) in got.items() if st == "fail" and k != label)
        print("mutant %-62s -> %s%s" % (name, "RED" if state == "fail" else "NOT RED (%s)" % state, ("  also red: " + ", ".join(others)) if others else ""))
        if state != "fail": bad += 1
    for l in sorted(set(declared) - set(clean)):
        print("STRAY MUTANT %s" % l); bad += 1
    print("selftest: %d sweep checks, %d mutants, %d not red or missing" % (len(clean), len(declared), bad))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else main())
