#!/usr/bin/env python3
"""Parity check: par-addaccount (round 2, the sheet). Every state of the Add account sheet (mac-add-account) and every control on it,
driven through the control socket, each with one check of its visible result as docs/PRD.md "Add account by IMAP with provider
autodetection", "Add account: redesign as a popup" and "Add account look: quiet glass, with the provider's mark in colour", docs/JOURNEYS.md
J4.1 to J4.5 and the confirmed board (docs/design/boards/addaccount-r3/mix) state them: the sheet opening over the window, the two fields,
the provider's mark in the Email field, the six steps with "n of 6", the found line and Details, Manual setup and its fields, the check with
"n of 2", Esc asking "Stop checking?", Stop saving nothing, a refusal in the server's words, the added state, Add another, Done, the
inline card, every button with the key it shows, the provider notes (an app password with the provider's own link; why Microsoft takes no password), and bring-your-own sign-in (the organisation's app ID, kept once, then Sign in with its state).

    python3 checks/parity/par-addaccount.py             drive the running rig (sh scripts/launch-one.sh <bundle> -- --fixture-core ...)
    python3 checks/parity/par-addaccount.py --selftest  every check against a model, then against each mutant: each must go red
Prints 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0. Expected values are written here from the PRD, the board and the
recorded fixtures of docs/lanes/contracts.md "## Delta 8", never read from a built screen. No mail account is connected."""
import json
import os
import re
import socketserver
import sys
import tempfile
import threading
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable  # noqa: E402

A = "mac-add-account"
PW = "fixture-password"
WRONG = "fixture-wrong-password"   # fixtures/addaccount/me.com.json: the recorded wrong-password case
N = [0]
CLICKED = set()    # every control id a check clicked
SEEN = set()       # every control id the controls rows ever listed


def uniq(domain):
    """A new address on a domain: the core refuses an address added twice."""
    N[0] += 1
    return "u%d.fixture@%s" % (N[0], domain)


EIGHT = ["imapHost", "imapPort", "imapSecurity", "smtpHost", "smtpPort", "smtpSecurity", "username", "authMethod"]
SRV_CFG = {"imapHost": "imap.srv.example.test", "imapPort": 993, "imapSecurity": "ssl", "smtpHost": "smtp.srv.example.test",
           "smtpPort": 587, "smtpSecurity": "starttls", "username": "assem.fixture@srv.example.test", "authMethod": "password"}
# the step words of fixtures/addaccount/steps.json (progress_words), which the sheet adopts
STEP_WORDS = {"ispdb": "Asking the Thunderbird provider list…", "autoconfig": "Looking for the provider’s own settings file…",
              "srv": "Looking up the mail server in DNS…", "autodiscover": "Asking Microsoft Autodiscover…", "mx": "Reading the domain’s mail record…",
              "guess": "Trying imap., mail. and smtp. names on the usual ports…"}
CHECKS = []


def check(name, mutant, words):
    def deco(fn):
        CHECKS.append((name, mutant, words, fn))
        return fn
    return deco


class Failed(Exception):
    """The app answered that a control or a key did nothing: the check is red, not unjudged."""


# ---- the driver's helpers: every click is recorded, every controls row is noted ----
def click(c, ax):
    CLICKED.add(ax)
    try:
        return c.click(A + "." + ax)
    except Unreadable as e:
        if "did nothing" in str(e) or "nothing to click" in str(e):
            raise Failed("the click on %s did nothing" % ax)
        raise


def key(c, chord):
    try:
        return c.key(chord)
    except Unreadable as e:
        if "does nothing" in str(e):
            raise Failed("the key %s did nothing" % chord)
        raise


def own_id(c):
    """A88: with the shipped default Microsoft ID the app-ID field hides behind 'Use your organisation's own ID'; open it first.
    On a sheet with no default the link is absent and the click is refused, which is fine."""
    try: click(c, "oauth.own-id")
    except Exception: pass

def refused(c, ax):
    """True when the click did nothing (the app answers 'did nothing' for a control that is off or not on screen)."""
    try:
        c.click(A + "." + ax)
        return False
    except Unreadable:
        return True


def release(c):
    """Let a held answer finish; a hold with nothing waiting (the sheet moved on) is not an error."""
    try:
        c.click(A + ".release")
    except Unreadable:
        pass
    CLICKED.add("release")


def rows(c, region):
    r = c.rows("%s.%s" % (A, region))
    if region == "controls":
        SEEN.update(x["id"] for x in r)
    return r


def val(c, region, key):
    """One field of a region's first row; None when the region shows nothing (an absence is a wrong answer, not an unjudged one)."""
    r = rows(c, region)
    return r[0].get(key) if r else None


def ctl(c, cid):
    """One control of the controls rows, or None when it is not on screen."""
    return next((x for x in rows(c, "controls") if x["id"] == cid), None)


def keyof(c, cid):
    x = ctl(c, cid)
    return None if x is None else (x["title"], x["key"], x["enabled"])


def reset(c):
    """A clean sheet: holds off, the sheet style, no fields, no accounts of an earlier check, no links opened."""
    click(c, "fixture.hold-off"); click(c, "fixture.sheet"); click(c, "start-over"); click(c, "cancel"); click(c, "open")


def start(c, address, password=PW):
    reset(c)
    click(c, "address"); c.type(address)
    click(c, "password"); c.type(password)
    click(c, "continue")


def hold_validation(c, address, password=PW):
    start(c, address, password)
    click(c, "fixture.hold-validation"); click(c, "add")


# ================= the sheet and the entry state =================
@check("sheet-opens-on-the-surface", "open-does-nothing", "opening the surface presents no sheet")
def _(c):
    reset(c); click(c, "cancel")
    closed = (c.uistate().get("surface"), val(c, "state", "sheet"))
    click(c, "open")
    return (closed, c.uistate().get("surface"), val(c, "state", "sheet")), (("mac-main", False), A, True)


@check("first-screen-two-fields", "third-field", "a third field is shown on the first screen")
def _(c):
    reset(c)
    return [r["id"] for r in rows(c, "fields")], ["address", "password"]


@check("entry-buttons-show-their-keys", "no-key-caps", "a button of the first screen shows no key")
def _(c):
    reset(c)
    return [keyof(c, i) for i in ("manual", "cancel", "continue")], \
        [("Manual setup", "", True), ("Cancel", "esc", True), ("Continue", "↩", False)]


@check("continue-follows-the-address", "continue-always-on", "Continue is on before there is an address")
def _(c):
    reset(c)
    click(c, "address"); c.type("not-an-address")
    off = (keyof(c, "continue") or (0, 0, None))[2]
    refuse = refused(c, "continue")
    c.type("@x.example.test")
    return (off, refuse, (keyof(c, "continue") or (0, 0, None))[2]), (False, True, True)


@check("entry-says-keychain-and-nothing-is-coming-soon", "entry-line-missing", "the first screen drops the Keychain line, or still says a sign-in is coming soon")
def _(c):
    reset(c)
    return (val(c, "step", "text"), rows(c, "signin.coming-soon")), ("The password stays in this Mac’s Keychain.", [])


@check("reveal-shows-and-hides-the-password", "reveal-dead", "the eye does nothing")
def _(c):
    reset(c)
    click(c, "password"); c.type("zq7-Plum")
    a = (val(c, "password", "revealed"), ctl(c, "reveal")["title"])
    click(c, "reveal")
    b = (val(c, "password", "revealed"), ctl(c, "reveal")["title"], val(c, "password", "text"))
    click(c, "reveal")
    return (a, b, val(c, "password", "revealed")), ((False, "Show password"), (True, "Hide password", "••••••••"), False)


@check("mark-in-the-email-field-follows-the-domain", "wrong-mark-table", "the Email field shows another provider's mark")
def _(c):
    got = {}
    for d in ("gmail.com", "outlook.com", "icloud.com", "yahoo.com", "fastmail.com", "srv.example.test"):
        reset(c); click(c, "address"); c.type("assem.fixture@" + d)
        got[d] = next((r["kind"] for r in rows(c, "marks") if r["placement"] == "field"), None)
    reset(c); click(c, "address"); c.type("assem.fix")
    half = next((r["kind"] for r in rows(c, "marks") if r["placement"] == "field"), None)
    return (got, half), ({"gmail.com": "google", "outlook.com": "microsoft", "icloud.com": "icloud", "yahoo.com": "yahoo",
                          "fastmail.com": "fastmail", "srv.example.test": "unknown"}, None)


@check("head-keeps-the-grey-glyph-until-found", "head-marked-early", "the head shows a mark before the settings are found")
def _(c):
    reset(c); click(c, "address"); c.type("assem.fixture@icloud.com")
    typed = next(r["kind"] for r in rows(c, "marks") if r["placement"] == "head")
    return typed, "mail"


# ================= detection =================
@check("five-steps-in-order", "steps-out-of-order", "the detection steps show in another order")
def _(c):
    start(c, "assem.fixture@guess-only.example.test")
    return [(r["id"], r["outcome"]) for r in rows(c, "detection.step")], [("ispdb", "miss"), ("autoconfig", "miss"), ("srv", "miss"), ("autodiscover", "miss"), ("guess", "found")]


@check("steps-named-in-the-fixture-words", "step-words-empty", "a step shows other words than the fixture's")
def _(c):
    start(c, "assem.fixture@srv.example.test")
    return [r["text"] for r in rows(c, "detection.step")], [STEP_WORDS["ispdb"], STEP_WORDS["autoconfig"], STEP_WORDS["srv"]]


@check("step-line-says-n-of-6", "step-count-wrong", "the running step line does not say '1 of 6' with the first step's words")
def _(c):
    reset(c); click(c, "fixture.hold-detection")
    click(c, "address"); c.type("assem.fixture@srv.example.test"); click(c, "password"); c.type(PW); click(c, "continue")
    r = rows(c, "step")[0]
    locked = (val(c, "address", "locked"), val(c, "password", "locked"))
    controls = [keyof(c, i) for i in ("manual", "continue")]
    got = ((r["text"], r["count"], r["segments"], r["done"]), locked, controls)
    release(c); click(c, "fixture.hold-off")
    return got, ((STEP_WORDS["ispdb"], "1 of 6", 6, 0), (True, True), [("Skip to manual setup", "", True), ("Finding…", "", False)])


@check("detection-held-shows-running", "held-shows-summary", "a detection that has not finished already shows the summary")
def _(c):
    reset(c); click(c, "fixture.hold-detection")
    click(c, "address"); c.type("assem.fixture@srv.example.test"); click(c, "password"); c.type(PW); click(c, "continue")
    s = rows(c, "state")[0]
    held = (s["stage"], s["held"], len(rows(c, "summary.mode")))
    release(c); click(c, "fixture.hold-off")
    return (held, rows(c, "state")[0]["stage"]), (("detecting", True, 0), "summary")


@check("skip-to-manual-leaves-detection", "skip-dead", "'Skip to manual setup' does not open Manual setup")
def _(c):
    reset(c); click(c, "fixture.hold-detection")
    click(c, "address"); c.type("assem.fixture@srv.example.test"); click(c, "password"); c.type(PW); click(c, "continue")
    click(c, "manual")
    got = (val(c, "state", "stage"), val(c, "state", "manual_open"), val(c, "summary.mode", "mode"))
    release(c); click(c, "fixture.hold-off")
    return (got, val(c, "state", "stage")), (("summary", True, "manual"), "summary")


# ================= found, Details and Manual setup =================
@check("found-line-names-both-servers-and-the-step", "found-line-missing", "the found line is missing or names no source")
def _(c):
    start(c, "assem.fixture@gmail.com")
    f = rows(c, "found")
    h = next(r["kind"] for r in rows(c, "marks") if r["placement"] == "head")
    return ((f[0]["imap"], f[0]["smtp"], f[0]["source"], f[0]["mark"]) if f else None, h, val(c, "step", "text")), \
        (("imap.gmail.com:993", "smtp.gmail.com:465", "Thunderbird list", "google"), "google", "found")


@check("found-buttons-show-their-keys", "found-no-keys", "the found state's buttons show no key")
def _(c):
    start(c, "assem.fixture@srv.example.test")
    return [keyof(c, i) for i in ("details", "manual", "cancel", "add")], \
        [("Details", "", True), ("Manual setup", "", True), ("Cancel", "esc", True), ("Add", "↩", True)]


@check("summary-eight-fields", "summary-drops-field", "the summary drops a field")
def _(c):
    start(c, "assem.fixture@srv.example.test")
    got = {f: val(c, "summary." + f, "value") for f in EIGHT}
    return (got, val(c, "summary.mode", "mode"), any(val(c, "summary." + f, "editable") for f in EIGHT)), (SRV_CFG, "detected", False)


@check("details-unfolds-manual-setup-and-folds-it-back", "details-dead", "Details does not unfold the server settings")
def _(c):
    start(c, "assem.fixture@srv.example.test")
    click(c, "details")
    a = (val(c, "state", "manual_open"), val(c, "summary.mode", "mode"), all(val(c, "summary." + f, "editable") for f in EIGHT), ctl(c, "manual")["title"], ctl(c, "details"))
    click(c, "manual")
    b = (val(c, "state", "manual_open"), val(c, "summary.mode", "mode"), ctl(c, "manual")["title"])
    return (a, b), ((True, "manual", True, "Hide manual setup", None), (False, "detected", "Manual setup"))


@check("manual-setup-button-edits", "manual-button-dead", "Manual setup does nothing")
def _(c):
    start(c, "assem.fixture@srv.example.test")
    click(c, "manual")
    return (val(c, "summary.mode", "mode"), all(val(c, "summary." + f, "editable") for f in EIGHT)), ("manual", True)


@check("manual-setup-opens-when-nothing-found", "nothing-opens-detected", "a failed detection opens as a detected summary")
def _(c):
    start(c, "assem.fixture@nothing.example.test")
    return (val(c, "summary.mode", "mode"), val(c, "state", "manual_open"), all(val(c, "summary." + f, "editable") for f in EIGHT),
            val(c, "step", "text"), ctl(c, "manual")["title"]), \
        ("manual", True, True, "No settings found for nothing.example.test. The Thunderbird list, the domain’s file, DNS, Autodiscover and the usual names all missed.", "Hide manual setup")


@check("manual-fields-take-typed-settings", "manual-type-lost", "text typed into a server field is lost")
def _(c):
    start(c, "assem.fixture@nothing.example.test")
    before = (val(c, "summary.imapPort", "value"), val(c, "summary.imapSecurity", "value"), val(c, "summary.smtpPort", "value"), val(c, "summary.smtpSecurity", "value"))
    click(c, "summary.imapHost"); c.type("imap.nothing.example.test")
    click(c, "summary.imapSecurity.starttls")
    return (before, val(c, "summary.imapHost", "value"), val(c, "summary.imapSecurity", "value")), \
        (((993, "ssl", 587, "starttls")), "imap.nothing.example.test", "starttls")


@check("manual-security-segments-switch-each-server", "security-seg-dead", "a Security segment of Manual setup does nothing")
def _(c):
    start(c, "assem.fixture@nothing.example.test")
    got = []
    for f, v in (("imapSecurity", "starttls"), ("imapSecurity", "ssl"), ("smtpSecurity", "ssl"), ("smtpSecurity", "starttls")):
        click(c, "summary.%s.%s" % (f, v))
        got.append(val(c, "summary." + f, "value"))
    return got, ["starttls", "ssl", "ssl", "starttls"]


@check("manual-port-and-username-fields-take-typing", "field-focus-dead", "a Manual setup field other than the hosts takes no typing")
def _(c):
    start(c, "assem.fixture@nothing.example.test")
    got = []
    for f, t in (("imapPort", "4"), ("smtpPort", "5"), ("username", "me@x.test")):
        before = val(c, "summary." + f, "value")
        click(c, "summary." + f); c.type(t)
        got.append(str(val(c, "summary." + f, "value")) == str(before) + t)   # the socket's `type` appends to what the field holds
    return got, [True, True, True]


@check("scrim-click-leaves-the-sheet-open", "scrim-closes", "a click on the scrim outside the sheet closes it")
def _(c):
    start(c, "assem.fixture@nothing.example.test")
    before = (val(c, "state", "sheet"), val(c, "state", "stage"))
    click(c, "scrim")
    return (before, (val(c, "state", "sheet"), val(c, "state", "stage"))), (((True, "summary")), (True, "summary"))


@check("add-waits-for-complete-settings", "add-on-when-incomplete", "Add is on while a server setting is missing")
def _(c):
    start(c, "assem.fixture@nothing.example.test")
    off = (ctl(c, "add") or {}).get("enabled")
    refuse = refused(c, "add")
    for f, v in (("imapHost", "imap.nothing.example.test"), ("smtpHost", "smtp.nothing.example.test")):
        click(c, "summary." + f); c.type(v)
    return (off, refuse, (ctl(c, "add") or {}).get("enabled")), (False, True, True)


@check("app-password-link-opens-its-page", "app-password-link-dead", "the app-password link opens nothing")
def _(c):
    start(c, "assem.fixture@gmail.com")
    click(c, "app-password.link")
    return [l for l in val(c, "state", "opened_links")], ["https://myaccount.google.com/apppasswords"]


@check("microsoft-365-tenant", "tenant-not-named", "a Microsoft 365 address does not say it is a tenant")
def _(c):
    start(c, "assem.fixture@m365.example.test")
    return (len(rows(c, "tenant")), rows(c, "manual")[0]["visible"], rows(c, "signin.coming-soon")), (1, True, [])


# ================= round 3: provider notes and bring-your-own sign-in (PRD 2026-11-09 "as easy as Thunderbird") =================
MS_NOTE = "Microsoft turned off password sign-in for IMAP in 2022, so a password cannot work here. Paste your organisation’s app ID to sign in."


GUID = "11111111-2222-3333-4444-555555555555"
MS_BAD_ID = "a Microsoft application (client) ID looks like 8-4-4-4-12 letters and digits; copy it from the app's Overview page"


def ms(c):
    """A fresh Microsoft 365 address: the sign-in belongs to the address (its draft id), so each check takes its own."""
    a = uniq("m365.example.test")
    start(c, a)
    return a


def note(c, address):
    start(c, address)
    r = rows(c, "provider-note")
    return (r[0]["note"], r[0]["text"], r[0].get("link", "")) if r else None


def wait_for(c, region, key, want, tries=12):
    """The sheet polls the core's sign-in state; a read waits for it to move."""
    for _ in range(tries):
        if val(c, region, key) == want:
            return want
        time.sleep(0.4)
    return val(c, region, key)


@check("note-gmail-takes-an-app-password", "gmail-no-note", "Gmail shows no app-password line with Google's link")
def _(c):
    return note(c, "assem.fixture@gmail.com"), ("app-password", "Google wants an app password for mail apps.", "https://myaccount.google.com/apppasswords")


@check("note-icloud-takes-an-app-password", "icloud-no-note", "iCloud shows no app-password line with Apple's link")
def _(c):
    return note(c, uniq("icloud.com")), ("app-password", "iCloud wants an app password for mail apps.", "https://account.apple.com/account/manage")


@check("note-fastmail-takes-an-app-password", "fastmail-no-note", "Fastmail shows no app-password line with its link")
def _(c):
    return note(c, uniq("fastmail.com")), ("app-password", "Fastmail wants an app password for mail apps.", "https://app.fastmail.com/settings/security/integrations")


@check("note-plain-imap-host-says-nothing", "plain-gets-note", "a plain IMAP host is given a provider note")
def _(c):
    return (note(c, uniq("srv.example.test")), rows(c, "oauth")), (None, [])


@check("microsoft-without-an-id-says-why", "ms-note-missing", "Microsoft 365 with no app ID does not say why a password cannot work")
def _(c):
    ms(c)
    n = rows(c, "provider-note")
    o = rows(c, "oauth")[0]
    return ((n[0]["note"], n[0]["text"], n[0].get("guide")) if n else None, o["kind"], o["client_saved"]), \
        (("microsoft-no-password", MS_NOTE, "bring-your-own-sign-in.md"), "microsoft", False)


@check("guide-link-opens-the-bundled-guide", "guide-link-dead", "How to get your app ID opens nothing")
def _(c):
    ms(c)
    click(c, "app-id-guide")
    return val(c, "state", "opened_links"), ["guide:bring-your-own-sign-in.md"]


@check("sign-in-needs-the-id-first", "signin-without-id", "Sign in goes ahead with no app ID pasted")
def _(c):
    # A88 ships a default Microsoft ID, so this check clears it first (the A88 checks below set and clear it the same way)
    c.send("fixture", "default-client", "clear")
    try:
        ms(c)
        refuse = refused(c, "oauth.sign-in")
        return (refuse, val(c, "state", "stage"), rows(c, "error")[0]["text"] if rows(c, "error") else ""), (True, "summary", "Paste your app ID first.")
    finally:
        c.send("fixture", "default-client", "set")


@check("wrong-app-id-refused-in-words", "bad-id-accepted", "a pasted app ID of the wrong shape is accepted")
def _(c):
    ms(c)
    own_id(c); click(c, "oauth.field"); c.type("refused-app-id")
    click(c, "oauth.sign-in")
    return (val(c, "oauth", "phase"), val(c, "oauth", "client_saved"), MS_BAD_ID in (val(c, "error", "text") or ""), val(c, "state", "added")), ("idle", False, True, 0)


@check("microsoft-with-an-id-signs-in", "signin-skips-waiting", "Sign in shows no waiting state before the provider answers")
def _(c):
    a = ms(c)
    own_id(c); click(c, "oauth.field"); c.type(GUID)
    click(c, "oauth.sign-in")
    wait = (val(c, "oauth", "phase"), val(c, "oauth", "words"), val(c, "oauth", "client_saved"), (val(c, "oauth", "url") or "").startswith("http"),
            len(rows(c, "accounts")))
    try:
        c.send("fixture", "oauth", "approve", a)
    except Unreadable:
        pass                                     # nothing was waiting: the check below is already red on the waiting state
    out = (wait, wait_for(c, "state", "stage", "added"), len(rows(c, "accounts")) >= 1)
    return out, (("waiting", "Waiting for you in the browser…", True, True, 0), "added", True)


@check("provider-refusal-shown-in-words", "refusal-hidden", "the provider's refusal is not shown, or adds an account")
def _(c):
    a = ms(c)
    own_id(c); click(c, "oauth.field"); c.type(GUID)
    click(c, "oauth.sign-in")
    c.send("fixture", "oauth", "refuse", a, "access_denied")
    got = wait_for(c, "oauth", "phase", "refused")
    return (got, val(c, "oauth", "words"), val(c, "state", "stage"), val(c, "state", "added")), \
        ("refused", "the provider refused the sign-in: access_denied", "summary", 0)


@check("app-id-kept-for-the-address-and-changeable", "change-dead", "Change does not bring the app ID field back")
def _(c):
    a = ms(c)
    own_id(c); click(c, "oauth.field"); c.type(GUID)
    click(c, "oauth.sign-in")
    c.send("fixture", "oauth", "refuse", a, "x")
    wait_for(c, "oauth", "phase", "refused")
    start(c, a)                                  # the same address on a new sheet: the core still holds its ID
    again = val(c, "oauth", "client_saved")
    click(c, "oauth.change")
    return (again, val(c, "oauth", "client_saved")), (True, False)


@check("app-id-is-in-no-row", "app-id-in-row", "the pasted app ID can be read from a row")
def _(c):
    ms(c)
    secret = "zz-app-id-4f19-secret-shape"
    own_id(c); click(c, "oauth.field"); c.type(secret)
    seen = json.dumps([rows(c, r) for r in ("oauth", "provider-note", "state", "step", "error", "controls")])
    return (secret in seen, val(c, "oauth", "length")), (False, len(secret))


@check("personal-gmail-has-no-app-id-field", "gmail-shows-oauth", "a personal Gmail address is offered an app ID field")
def _(c):
    start(c, "assem.fixture@gmail.com")
    return (rows(c, "oauth"), val(c, "provider-note", "note")), ([], "app-password")


@check("guide-copy-matches-the-doc", "guide-copy-differs", "the guide bundled in the app differs from docs/guides")
def _(c):
    a = os.path.join(ROOT, "docs", "guides", "bring-your-own-sign-in.md")
    b = os.path.join(ROOT, "Sources", "Resources", "guides", "bring-your-own-sign-in.md")
    return open(a, encoding="utf-8").read() == open(b, encoding="utf-8").read(), True


@check("manual-stays-closed-when-something-is-found", "manual-open-on-found", "the manual fields open although a step found the servers")
def _(c):
    start(c, uniq("srv.example.test"))
    return (val(c, "state", "manual_open"), val(c, "summary.mode", "mode")), (False, "detected")


# ================= the check, Esc, Stop =================
@check("validation-lines", "validation-lines-reordered", "the validation lines show in the wrong order")
def _(c):
    start(c, uniq("srv.example.test"))
    click(c, "add")
    return [r["text"] for r in rows(c, "validation.line")], ["Checking the mail server…", "Checking sending…"]


@check("validating-line-says-1-of-2", "validating-no-count", "the check line does not say '1 of 2'")
def _(c):
    hold_validation(c, uniq("srv.example.test"))
    r = rows(c, "step")[0]
    got = ((r["text"], r["count"], r["segments"]), val(c, "address", "locked"), [keyof(c, i) for i in ("manual", "cancel", "add")],
           [(x["text"], x["state"]) for x in rows(c, "validation.line")])
    release(c); click(c, "fixture.hold-off")
    return got, (("Checking the mail server…", "1 of 2", 2), True, [("Manual setup", "", False), ("Cancel", "esc", True), ("Adding…", "", False)],
                 [("Checking the mail server…", "running"), ("Checking sending…", "pending")])


@check("esc-asks-before-stopping", "esc-stops-at-once", "Esc stops a running check without asking")
def _(c):
    hold_validation(c, uniq("srv.example.test"))
    key(c, "Escape")
    got = (val(c, "state", "stage"), val(c, "state", "confirming_stop"), [keyof(c, i) for i in ("keep-checking", "stop", "cancel")])
    release(c); click(c, "fixture.hold-off")
    return got, ("validating", True, [("Keep checking", "↩", True), ("Stop", "esc", True), ("Cancel", "esc", False)])


@check("cancel-mid-check-asks-too", "cancel-closes-mid-check", "Cancel closes the sheet while a check runs")
def _(c):
    hold_validation(c, uniq("srv.example.test"))
    click(c, "cancel")
    got = (c.uistate().get("surface"), val(c, "state", "confirming_stop"), val(c, "state", "stage"))
    release(c); click(c, "fixture.hold-off")
    return got, (A, True, "validating")


@check("keep-checking-resumes", "keep-dead", "Keep checking does not dismiss the question")
def _(c):
    hold_validation(c, uniq("srv.example.test"))
    key(c, "Escape"); click(c, "keep-checking")
    got = (val(c, "state", "confirming_stop"), val(c, "state", "stage"), ctl(c, "keep-checking"))
    release(c); click(c, "fixture.hold-off")
    return (got, val(c, "state", "stage")), ((False, "validating", None), "added")


@check("stop-saves-nothing", "stop-adds-account", "Stop leaves an account behind")
def _(c):
    reset(c)
    before = val(c, "state", "added")
    hold_validation(c, uniq("srv.example.test"))
    key(c, "Escape"); click(c, "stop")
    after_stop = (val(c, "state", "stage"), val(c, "state", "confirming_stop"), len(rows(c, "validation.line")), val(c, "state", "added"))
    release(c)
    return (after_stop, val(c, "state", "added"), before), (("summary", False, 0, before), before, before)


@check("second-esc-is-stop", "second-esc-ignored", "a second Esc does not stop the check")
def _(c):
    hold_validation(c, uniq("srv.example.test"))
    key(c, "Escape"); key(c, "Escape")
    got = (val(c, "state", "stage"), val(c, "state", "confirming_stop"))
    release(c)
    return got, ("summary", False)


# ================= close, add, added =================
@check("cancel-closes-the-sheet", "cancel-dead", "Cancel leaves the sheet open")
def _(c):
    start(c, "assem.fixture@srv.example.test")
    click(c, "cancel")
    return (c.uistate().get("surface"), val(c, "state", "sheet"), val(c, "state", "stage")), ("mac-main", False, "entry")


@check("esc-closes-the-first-screen", "esc-dead-at-entry", "Esc does not close the first screen")
def _(c):
    reset(c)
    key(c, "Escape")
    return (c.uistate().get("surface"), val(c, "state", "sheet")), ("mac-main", False)


@check("return-continues", "return-dead", "Return does not run Continue")
def _(c):
    reset(c); click(c, "address"); c.type("assem.fixture@srv.example.test"); click(c, "password"); c.type(PW)
    key(c, "Enter")
    return val(c, "state", "stage"), "summary"


@check("account-row-after-add", "no-account-row", "Add creates no account row")
def _(c):
    a = uniq("srv.example.test")
    start(c, a)
    click(c, "add")
    r = rows(c, "accounts")
    return (len(r) >= 1 and r[-1]["address"], rows(c, "state")[0]["stage"]), (a, "added")


@check("added-stays-added", "added-falls-back", "the screen drops back to the empty form once the account is added")
def _(c):
    start(c, uniq("srv.example.test"))
    click(c, "add")
    time.sleep(0.8)   # a field that leaves the screen commits its text late: the stage must hold
    return (rows(c, "state")[0]["stage"], len(rows(c, "accounts")) >= 1), ("added", True)


@check("added-state-buttons-marks-and-words", "added-no-keys", "the added state shows no keys, no mark or no sync words")
def _(c):
    start(c, uniq("fastmail.com"))
    click(c, "add")
    marks = [(r["placement"], r["kind"]) for r in rows(c, "marks")]
    return ([keyof(c, i) for i in ("add-another", "done")], marks, val(c, "step", "text"), ctl(c, "cancel")), \
        ([("Add another", "⌘N", True), ("Done", "↩", True)], [("head", "fastmail"), ("added", "fastmail")], "Connecting…", None)


@check("row-progress-connecting-then-fetching", "progress-stuck", "the account row stays on connecting")
def _(c):
    start(c, uniq("fastmail.com"))
    click(c, "add")
    first = rows(c, "accounts")[-1]["progress"]
    c.send("clock", "advance", 5)
    return (first, rows(c, "accounts")[-1]["progress"]), ("connecting", "fetching the inbox")


@check("add-another-clears-and-stays", "add-another-leaves", "Add another leaves the screen")
def _(c):
    start(c, uniq("srv.example.test"))
    click(c, "add"); click(c, "add-another")
    return (rows(c, "address")[0]["text"], c.uistate().get("surface"), len(rows(c, "accounts")) >= 1, val(c, "state", "stage")), ("", A, True, "entry")


@check("done-closes-on-the-new-account", "done-leaves-open", "Done leaves the sheet open or selects no account")
def _(c):
    start(c, uniq("srv.example.test"))
    click(c, "add")
    new = rows(c, "accounts")[-1]["id"]
    click(c, "done")
    return (c.uistate().get("surface"), val(c, "state", "sheet"), c.uistate().get("account_filter")), ("mac-main", False, new)


@check("keys-add-another-and-done", "added-keys-dead", "⌘N or ⌘↩ do nothing on the added state")
def _(c):
    start(c, uniq("srv.example.test")); click(c, "add"); key(c, "⌘N")
    a = (val(c, "state", "stage"), c.uistate().get("surface"))
    start(c, uniq("srv.example.test")); click(c, "add"); key(c, "⌘Enter")
    return (a, c.uistate().get("surface")), (("entry", A), "mac-main")


# ================= refusals =================
@check("wrong-password-in-words-with-link", "no-link", "the wrong-password block carries no Apple page link")
def _(c):
    start(c, "assem.fixture@me.com", WRONG)
    click(c, "add")
    e = rows(c, "error")[0]
    click(c, "error.details")   # SPEC A87: the server's own words (host, "wrong password") sit behind Details
    d = rows(c, "error")[0]["details"]
    return ((e["text"], "imap.mail.me.com" in d, "wrong password" in d), e["link"].startswith("https://"), val(c, "summary.mode", "mode")), \
        (("The password was refused.", True, True), True, "manual")


@check("refusal-block-says-who-and-offers-try-again", "refusal-header-missing", "the refusal names no server or offers no Try again")
def _(c):
    start(c, "assem.fixture@me.com", WRONG)
    click(c, "add")
    e = rows(c, "error")[0]
    got = (e["header"], e["host"], [keyof(c, i) for i in ("manual", "retry", "cancel")], val(c, "state", "manual_open"), val(c, "state", "stage"))
    click(c, "retry")   # Try again asks the server again; the same wrong password is refused the same way
    again = (val(c, "state", "stage"), len(rows(c, "error")), val(c, "state", "added"))
    return (got, again), (("imap.mail.me.com refused the password", "imap.mail.me.com", [("Server settings", "", True), ("Try again", "↩", True), ("Cancel", "esc", True)], False, "summary"),
                          ("summary", 1, 0))


@check("error-link-opens-its-page", "error-link-dead", "the refusal's page link opens nothing")
def _(c):
    start(c, "assem.fixture@me.com", WRONG)
    click(c, "add"); click(c, "error.link")
    return val(c, "state", "opened_links"), ["https://appleid.apple.com"]


@check("blocked-tenant-says-so", "blocked-tenant-silent", "a tenant that blocks password IMAP is not told why")
def _(c):
    start(c, "assem.fixture@blocked-m365.example.test")
    click(c, "add")
    t = rows(c, "error")[0]["text"]
    return ("password" in t, "app ID" in t, val(c, "summary.mode", "mode")), (True, True, "manual")


# ================= SPEC A87: one plain sentence per known cause, the server's words behind Details =================
# The sentences are the catalogue's English source (Sources/Features/AddAccount/AddAccountWords.causeSentence); the raw
# words are the fixture's recorded server answers (Sources/CoreBridge/FixtureAddAccount.swift rawWords).
CAUSES = {
    "basic_auth_disabled": ("Microsoft no longer accepts passwords for mail. Sign in with your organisation’s app ID below.", "Basic authentication is disabled."),
    "bad_password": ("The password was refused.", "Invalid credentials"),
    "host_unreachable": ("The server did not answer.", "Connection refused"),
    "tls_failed": ("The secure connection failed.", "invalid peer certificate"),
    "smtp_refused": ("The sending server refused the sign-in.", "535 5.7.8"),
    "unknown": ("The server refused the account. Details shows its own words.", "SERVERBUG"),
}


def refuse_with(c, cause):
    start(c, uniq("srv.example.test"))
    c.send("fixture", "refuse", cause)
    click(c, "add")
    return rows(c, "error")[0]


def _cause_check(cause):
    @check("a87-%s-reads-as-its-sentence" % cause.replace("_", "-"), "raw-as-sentence", "the server's raw text is shown as the sentence")
    def _(c):
        e = refuse_with(c, cause)
        sentence, raw = CAUSES[cause]
        return (e["text"], e.get("cause"), raw in e["text"], e.get("details_available"), e.get("details_open"), e.get("details")), \
            (sentence, cause, False, True, False, "")


for _c in CAUSES:
    _cause_check(_c)


@check("a87-details-opens-the-servers-words-and-closes", "details-dead", "Details does not show the server's own words")
def _(c):
    e = refuse_with(c, "basic_auth_disabled")
    click(c, "error.details")
    opened = rows(c, "error")[0]
    click(c, "error.details")
    closed = rows(c, "error")[0]
    return ((e["details_label"], opened["details_open"], CAUSES["basic_auth_disabled"][1] in opened["details"], opened["details_label"]),
            (closed["details_open"], closed["details"])), (("Details", True, True, "Hide details"), (False, ""))


@check("a87-how-to-get-one-beside-the-app-id-field", "field-guide-missing", "no guide link beside the app-ID field")
def _(c):
    ms(c)
    link = val(c, "oauth", "guide_link")
    click(c, "oauth.guide")
    return (link, val(c, "oauth", "guide"), val(c, "state", "opened_links")), ("How to get one", "bring-your-own-sign-in.md", ["guide:bring-your-own-sign-in.md"])


# ================= round 5: the engine's refusals in the app's language =================
# The engine sends a code beside its English words. The sheet shows the catalogue sentence for the code (14 languages) and keeps the
# engine's words behind Details. The sentences are read from the catalogue file (Sources/Resources/Localizable.xcstrings, keys
# `add-account.engine.*`), never from a built screen. The engine's words below are the ones `fixture refuse <code>` must throw
# (ActionFailed.reasonWords with `code` set): the patch is written in docs/lanes/addaccount.md "round 5".
CAT = json.load(open(os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8"))["strings"]
LANG_ID = "mac-settings-general.appearance.language."
CONSENT_KEY = "The admin of %@ must approve Whoosh once. Then you can sign in."   # A98: the sentence names the mailbox's organisation (%@ = the address's domain)
CONSENT_OLD_KEY = "add-account.engine.oauth_consent_required"   # the sentence with no domain (kept for a sync error that has none)
LINK_KEY = "Approval link: %@"
CONSENT_HOST = "srv.example.test"   # the domain every consent drive uses
CONSENT_URL = "https://login.microsoftonline.com/%s/adminconsent?client_id=%s"
STAND_IN_ID = "-".join("c" * n for n in (8, 4, 4, 4, 12))   # DefaultSignInClients.microsoftInUse under `fixture default-client set`
ENGINE = {   # code: (catalogue key, the engine's own English words)
    "engine.not_built_yet": ("add-account.engine.not_built_yet", "The local engine does not do this yet."),
    "oauth.no_client_id": ("add-account.engine.oauth_no_client_id", "no client id is set for this sign-in"),
    "oauth.sign_in_not_done": ("add-account.engine.oauth_sign_in_not_done", "the sign-in in the browser was not finished"),
    "oauth.refused": ("add-account.engine.oauth_refused", "the provider refused the sign-in"),
    "oauth.no_refresh_token": ("add-account.engine.oauth_no_refresh_token", "the provider gave no refresh token"),
    "oauth.browser": ("add-account.engine.oauth_browser", "the browser could not be opened"),
    # round 6 (A97): Microsoft wants the organisation's admin to approve the app once. `fixture refuse oauth.consent_required` must throw these words
    # (FixtureAddAccountBook.engineWords gains the code: docs/lanes/addaccount-r6.md FINDING)
    "oauth.consent_required": (CONSENT_KEY, "the provider refused the sign-in: AADSTS65001: The user or administrator has not consented to use the application."),
}
PATH_KEYS = ("add-account.engine.oauth_consent_path_page", "add-account.engine.oauth_consent_path_entra")


def word(key, lang):
    return CAT[key]["localizations"][lang]["stringUnit"]["value"]


def sentence_of(code, lang, domain=CONSENT_HOST):
    """The catalogue sentence for an engine code; the consent sentence carries the mailbox's domain in place of its %@."""
    w = word(ENGINE[code][0], lang)
    return w.replace("%@", domain) if code == "oauth.consent_required" else w


def engine_case(c, code, lang):
    """One engine refusal in one language: what the panel shows closed, then what Details shows open."""
    key, raw = ENGINE[code]
    start_domain = CONSENT_HOST
    c.click(LANG_ID + lang)
    try:
        start(c, uniq(start_domain))
        try:
            c.send("fixture", "refuse", code)
        except Exception as e:   # the fixture core refuses a code it does not know yet: not judged, never passed
            raise Unreadable("needs fixture verb (docs/lanes/addaccount.md round 5): %s" % e)
        click(c, "add")
        closed = rows(c, "error")[0]
        click(c, "error.details")
        opened = rows(c, "error")[0]
    finally:
        c.click(LANG_ID + "system")
    sentence = sentence_of(code, lang, start_domain)
    if lang != "en":
        assert sentence != sentence_of(code, "en", start_domain), "the %s sentence for %s is the English one" % (lang, code)
    got = (closed["text"], closed.get("cause"), raw in closed["text"], closed["header"], closed.get("details_available"), closed.get("details_open"),
           closed.get("details"), closed["details_label"], opened["details_open"], opened["details"], opened["details_label"])
    want = (sentence, code, False, word("The sign-in failed", lang), True, False, "", word("Details", lang), True, raw, word("Hide details", lang))
    return got, want


def _engine_check(code, lang):
    tag = code.replace(".", "-").replace("_", "-")
    mutant = "engine-words-alone" if lang == "fr" else "engine-wrong-sentence"
    @check("r5-%s-%s-shows-its-sentence-and-the-engines-words-under-details" % (tag, lang), mutant,
           "the %s engine refusal %s shows the engine's English words or another sentence" % (lang, code))
    def _(c):
        return engine_case(c, code, lang)


for _code in ENGINE:
    for _lang in ("fr", "en"):
        _engine_check(_code, _lang)


@check("r5-an-engine-code-without-a-sentence-reads-as-the-unknown-sentence", "engine-words-alone", "an unknown engine code shows the engine's English words alone")
def _(c):
    c.click(LANG_ID + "fr")
    try:
        start(c, uniq("srv.example.test"))
        try:
            c.send("fixture", "refuse", "engine.some_new_code")
        except Exception as e:
            raise Unreadable("needs fixture verb (docs/lanes/addaccount.md round 5): %s" % e)
        click(c, "add")
        e = rows(c, "error")[0]
    finally:
        c.click(LANG_ID + "system")
    return (e["text"], e.get("details_available")), (word("The server refused the account. Details shows its own words.", "fr"), True)


# ================= round 6 (SPEC A97): admin consent, the two admin paths said in words =================
def paths_case(c, code, lang):
    """One engine refusal in one language: the lines under the sentence (the two admin paths for consent, none for any other code)."""
    c.click(LANG_ID + lang)
    try:
        start(c, uniq("srv.example.test"))
        try:
            c.send("fixture", "refuse", code)
        except Exception as e:
            raise Unreadable("needs fixture verb (docs/lanes/addaccount-r6.md FINDING): %s" % e)
        click(c, "add")
        e = rows(c, "error")[0]
    finally:
        c.click(LANG_ID + "system")
    want = [word(k, lang) for k in PATH_KEYS] if code == "oauth.consent_required" else []
    if lang != "en" and want:
        assert all(word(k, lang) != word(k, "en") for k in PATH_KEYS), "a %s path line is the English one" % lang
    return (e.get("paths"), e["text"]), (want, sentence_of(code, lang))


for _lang in ("fr", "en"):
    @check("a97-consent-shows-the-two-admin-paths-%s" % _lang, "consent-no-paths", "the %s consent refusal shows no admin path under its sentence" % _lang)
    def _(c, _l=_lang):
        return paths_case(c, "oauth.consent_required", _l)


@check("a97-another-refusal-shows-no-admin-path", "consent-paths-always", "an ordinary provider refusal shows the admin paths")
def _(c):
    return paths_case(c, "oauth.refused", "fr")


@check("a97-the-consent-sentence-names-the-admin-and-once-in-every-language", "consent-sentence-generic", "a language's consent sentence is the English one, lacks the domain placeholder or does not say once")
def _(c):
    loc = json.load(open(os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8"))["strings"][CONSENT_KEY]["localizations"]
    en = loc["en"]["stringUnit"]["value"]
    bad = [l for l, v in loc.items() if l != "en" and v["stringUnit"]["value"] == en]
    holes = [l for l, v in loc.items() if v["stringUnit"]["value"].count("%@") != 1]
    return (len(loc), bad, holes, "admin" in en and "once" in en), (14, [], [], True)


# ================= round 7 (SPEC A98): the sentence names the mailbox's organisation, its admin-consent link =================
def consent_drive(c, lang, click_link=True):
    """The consent refusal on an address of CONSENT_HOST with the default registration set: the error row, then the link clicked and Copy clicked."""
    c.click(LANG_ID + lang)
    c.send("fixture", "default-client", "set")
    try:
        start(c, uniq(CONSENT_HOST))
        try:
            c.send("fixture", "refuse", "oauth.consent_required")
        except Exception as e:
            raise Unreadable("needs fixture verb (docs/lanes/addaccount-r6.md FINDING): %s" % e)
        click(c, "add")
        e = rows(c, "error")[0]
        click(c, "error.consent-link")
        click(c, "error.consent-copy")
        st = rows(c, "state")[0]
    finally:
        c.send("fixture", "default-client", "set")
        c.click(LANG_ID + "system")
    return e, st


for _lang in ("fr", "en"):
    @check("a98-the-consent-sentence-names-the-mailbox-domain-%s" % _lang, "consent-sentence-no-domain", "the %s consent sentence does not name the mailbox's domain" % _lang)
    def _(c, _l=_lang):
        e, _ = consent_drive(c, _l)
        return (e["text"], CONSENT_HOST in e["text"]), (sentence_of("oauth.consent_required", _l), True)

    @check("a98-the-consent-link-line-and-its-click-%s" % _lang, "consent-link-wrong-tenant" if _lang == "fr" else "consent-link-dead",
           "the %s consent link names another tenant or does not open, or its line is not the catalogue's" % _lang)
    def _(c, _l=_lang):
        e, st = consent_drive(c, _l)
        url = CONSENT_URL % (CONSENT_HOST, STAND_IN_ID)
        got = (e.get("consent_link"), e.get("consent_link_text"), e.get("consent_copy_label"), st["opened_links"], st["copied_links"])
        want = (url, word(LINK_KEY, _l).replace("%@", url), word("Copy", _l), [url], [url])
        if _l != "en":
            assert word(LINK_KEY, _l) != word(LINK_KEY, "en"), "the %s link line is the English one" % _l
        return got, want


@check("a98-another-refusal-offers-no-consent-link", "consent-link-always", "an ordinary provider refusal offers the admin-consent link")
def _(c):
    c.send("fixture", "default-client", "set")
    start(c, uniq(CONSENT_HOST))
    c.send("fixture", "refuse", "oauth.refused")
    click(c, "add")
    e = rows(c, "error")[0]
    return (e.get("consent_link"), e.get("consent_link_text")), ("", "")


@check("a98-the-link-line-and-copy-words-exist-in-every-language", "consent-link-words-english", "a language's link line is the English one or holds no link placeholder")
def _(c):
    loc = json.load(open(os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8"))["strings"][LINK_KEY]["localizations"]
    en = loc["en"]["stringUnit"]["value"]
    return (len(loc), [l for l, v in loc.items() if l != "en" and v["stringUnit"]["value"] == en], [l for l, v in loc.items() if v["stringUnit"]["value"].count("%@") != 1]), (14, [], [])


def guide_consent_step(root):
    """The Entra guide's consent step (the one whose sentence says Grant admin consent), as entraguide-r3 shaped it: a hint saying the approval is the
    mailbox organisation's admin's and not the registration owner's, with the two paths in words; the step flagged consent; and the admin-consent link
    as its own copyable value row (the typed URL left the hint, par-entraguide judges the link itself)."""
    d = json.load(open(os.path.join(root, "Sources", "Resources", "EntraGuide", "entraguide-steps.json"), encoding="utf-8"))
    steps = d["steps"]
    cat = json.load(open(os.path.join(root, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8"))["strings"]
    n = next(s["n"] for s in steps if "Grant admin consent" in cat["entra-guide.s%02d.say" % s["n"]]["localizations"]["en"]["stringUnit"]["value"])
    step = next(s for s in steps if s["n"] == n)
    hint = cat.get("entra-guide.s%02d.hint" % n, {}).get("localizations", {})
    en = hint.get("en", {}).get("stringUnit", {}).get("value", "")
    link_row = any(v.get("dynamic") == "consent-link" and n in v.get("steps", []) for g in d.get("groups", {}).values() for v in g)
    return (bool(step.get("hint")), len(hint), "not the registration's owner" in en.lower() and "Consent on behalf of your organisation" in en and "Grant admin consent" in en,
            bool(step.get("consent")), link_row, [l for l, v in hint.items() if l != "en" and v["stringUnit"]["value"] == en]), (True, 14, True, True, True, [])


@check("a97-the-guides-consent-step-says-the-mailbox-admin-approves-once-with-the-link", "guide-consent-silent", "the guide's consent step does not say the mailbox organisation's admin approves once, with the link pattern")
def _(c):
    return guide_consent_step(ROOT)


# ================= SPEC A88: Whoosh's default Microsoft registration, the own ID as the override =================
OWN_ID = "Use your organisation’s own ID"


@check("a88-default-client-signs-in-without-an-id", "default-ignored", "with the default shipped, a Microsoft address still asks for an ID")
def _(c):
    c.send("fixture", "default-client", "set")
    try:
        ms(c)
        o = rows(c, "oauth")[0]
        field_refused = refused(c, "oauth.field")
        click(c, "oauth.sign-in")
        phase = wait_for(c, "oauth", "phase", "waiting")
        return (o["default_client"], o["field_shown"], o["own_id_link"], field_refused, phase), (True, False, OWN_ID, True, "waiting")
    finally:
        c.send("fixture", "default-client", "clear")


@check("a88-own-id-link-brings-the-field-back", "own-id-link-dead", "Use your organisation's own ID does not bring the field back")
def _(c):
    c.send("fixture", "default-client", "set")
    try:
        ms(c)
        click(c, "oauth.own-id")
        o = rows(c, "oauth")[0]
        asked = refused(c, "oauth.sign-in")   # the override path as today: no ID pasted, Sign in does nothing but ask for one
        e = rows(c, "error")[0]["text"] if rows(c, "error") else ""
        return (o["default_client"], o["field_shown"], o["own_id_link"], asked, e), (False, True, "", True, "Paste your app ID first.")
    finally:
        c.send("fixture", "default-client", "clear")


@check("a88-no-default-shows-the-field-as-before", "default-always-on", "with no default shipped, the app-ID field is hidden")
def _(c):
    c.send("fixture", "default-client", "clear")
    ms(c)
    o = rows(c, "oauth")[0]
    return (o["default_client"], o["field_shown"], o["own_id_link"]), (False, True, "")


@check("refusal-adds-nothing", "refusal-adds-account", "a refused account is added anyway")
def _(c):
    reset(c)
    before = rows(c, "state")[0]["added"]
    start(c, "assem.fixture@me.com", WRONG)
    click(c, "add")
    return rows(c, "state")[0]["added"], before


@check("validation-held-shows-running", "held-validation-ok", "a check that has not finished shows as done")
def _(c):
    hold_validation(c, uniq("srv.example.test"))
    held = [(r["text"], r["state"]) for r in rows(c, "validation.line")]
    release(c); click(c, "fixture.hold-off")
    return held, [("Checking the mail server…", "running"), ("Checking sending…", "pending")]


# ================= the card inline, the password, start over =================
@check("inline-card-has-no-cancel-and-lists-what-was-added", "inline-has-cancel", "Welcome's card shows Cancel or loses the Added list")
def _(c):
    reset(c); click(c, "fixture.inline")
    a = [x["id"] for x in rows(c, "controls")]
    click(c, "address"); c.type(uniq("srv.example.test")); click(c, "password"); c.type(PW); click(c, "continue"); click(c, "add")
    got = (a, val(c, "state", "style"), val(c, "state", "stage"), len(rows(c, "accounts")) >= 1)
    reset(c)
    return got, (["reveal", "manual", "continue"], "inline", "entry", True)


@check("password-in-no-row", "password-in-row", "the password is readable in a row")
def _(c):
    secret = "zq7-Plum-Harbour-93"
    start(c, "assem.fixture@srv.example.test", secret)
    seen = json.dumps([rows(c, r) for r in ("password", "state", "fields", "summary.mode", "error", "notice", "manual", "detection.step", "validation.line", "accounts", "controls", "step")])
    return (secret in seen, rows(c, "password")[0]["length"]), (False, len(secret))


@check("start-over-clears", "start-over-keeps", "Start over does not clear the fields")
def _(c):
    start(c, "assem.fixture@srv.example.test")
    click(c, "start-over")
    return (rows(c, "address")[0]["text"], rows(c, "password")[0]["length"], rows(c, "state")[0]["stage"]), ("", 0, "entry")


@check("no-bare-string-in-the-sheet", "bare-string", "a string is drawn without Words.text")
def _(c):
    d = os.path.join(ROOT, "Sources", "Features", "AddAccount")
    bad = []
    for f in sorted(os.listdir(d)):
        if not f.endswith(".swift"):
            continue
        for i, line in enumerate(open(os.path.join(d, f), encoding="utf-8"), 1):
            code = line.split("//")[0]
            for m in re.finditer(r'(?:Text|Label|\.help|Button)\(\s*"([^"\\]*[A-Za-z]{3,}[^"]*)"', code):
                if "Words.text" not in code[:m.start()]:
                    bad.append("%s:%d %s" % (f, i, m.group(1)))
    return bad, []


@check("every-control-listed-was-clicked", "unclicked-control", "a control of the sheet is never clicked by a check")
def _(c):
    # the rows of the controls and the clicks recorded while the checks above ran
    want = {"reveal", "details", "manual", "cancel", "continue", "add", "retry", "add-another", "done", "keep-checking", "stop"}
    return sorted((SEEN | want) - CLICKED), []


def drive(c):
    CLICKED.clear(); SEEN.clear()
    out = []
    for name, mutant, words, fn in CHECKS:
        try:
            got, want = fn(c)
            out.append((name, got == want, "" if got == want else "got %r want %r" % (got, want)))
        except Failed as e:
            out.append((name, False, str(e)))
        except (Unreadable, IndexError, KeyError, TypeError, StopIteration) as e:
            out.append((name, None, "not judged: %s %s" % (type(e).__name__, e)))
    return out


def report(res):
    for n, ok, why in res:
        print("%s %s %s" % ("PASS" if ok else ("----" if ok is None else "FAIL"), n, why))
    f, u = sum(1 for _, ok, _ in res if ok is False), sum(1 for _, ok, _ in res if ok is None)
    print("%d checks, %d failed, %d not judged" % (len(res), f, u))
    return 1 if f or u else 0


# ---- the mutation rig: a model of the sheet, one mutant switched on at a time ----
DOM = {   # trace, config, coming-soon, tenant, mark, source
    "srv.example.test": ([("ispdb", 0), ("autoconfig", 0), ("srv", 1)], SRV_CFG, None, False, "unknown", "DNS"),
    "guess-only.example.test": ([("ispdb", 0), ("autoconfig", 0), ("srv", 0), ("autodiscover", 0), ("guess", 1)], dict(SRV_CFG, imapHost="imap.guess-only.example.test", smtpHost="smtp.guess-only.example.test"), None, False, "unknown", "the usual names"),
    "nothing.example.test": ([("ispdb", 0), ("autoconfig", 0), ("srv", 0), ("autodiscover", 0), ("guess", 0)], None, None, False, "unknown", ""),
    "gmail.com": ([("ispdb", 1)], dict(SRV_CFG, imapHost="imap.gmail.com", smtpHost="smtp.gmail.com", smtpPort=465, smtpSecurity="ssl", authMethod="oauth2"), "Sign in with Google: coming soon", False, "google", "Thunderbird list"),
    "m365.example.test": ([("ispdb", 0), ("autoconfig", 0), ("srv", 0), ("autodiscover", 1)], dict(SRV_CFG, imapHost="outlook.office365.com"), "Sign in with Microsoft: coming soon", True, "microsoft", "Microsoft Autodiscover"),
    "blocked-m365.example.test": ([("ispdb", 0), ("autoconfig", 0), ("srv", 0), ("autodiscover", 1)], dict(SRV_CFG, imapHost="outlook.office365.com"), "Sign in with Microsoft: coming soon", True, "microsoft", "Microsoft Autodiscover"),
    "me.com": ([("ispdb", 1)], dict(SRV_CFG, imapHost="imap.mail.me.com", smtpHost="smtp.mail.me.com"), None, False, "icloud", "Thunderbird list"),
    "icloud.com": ([("ispdb", 1)], dict(SRV_CFG, imapHost="imap.mail.me.com", smtpHost="smtp.mail.me.com"), None, False, "icloud", "Thunderbird list"),
    "fastmail.com": ([("ispdb", 0), ("autoconfig", 0), ("srv", 1)], dict(SRV_CFG, imapHost="imap.fastmail.com", smtpHost="smtp.fastmail.com"), None, False, "fastmail", "DNS"),
}
MARK_TABLE = {"gmail.com": "google", "outlook.com": "microsoft", "icloud.com": "icloud", "me.com": "icloud", "yahoo.com": "yahoo",
              "fastmail.com": "fastmail", "m365.example.test": "microsoft", "blocked-m365.example.test": "microsoft"}
KEYS = {"manual": "", "cancel": "esc", "continue": "↩", "add": "↩", "retry": "↩", "add-another": "⌘N", "done": "↩", "keep-checking": "↩", "stop": "esc", "details": "", "reveal": ""}
NOTE_NOTHING = ("No settings found for %s. The Thunderbird list, the domain’s file, DNS, Autodiscover and the usual names all missed.")


class Model:
    def __init__(self, mut):
        self.m = mut
        self.surface = "mac-main"
        self.account_filter = None
        self.hold_d = self.hold_v = False
        self.drafts = {}            # address -> {state, words, url}: the core holds each sign-in by its draft id; it outlives a sheet
        self.changing = False
        self.held = None
        self.clock = 0
        self.added = []
        self.links = []
        self.copied = []
        self.consent_url = ""
        self.style = "sheet"
        self.default_client = False   # SPEC A88: `fixture default-client set|clear`
        self.lang = "en"
        self.reset()

    def reset(self):
        self.links = []
        self.copied = []
        self.consent_url = ""
        self.addr = self.pw = ""
        self.focus = None
        self.stage, self.mode, self.steps, self.cfg, self.soon, self.tenant, self.src = "entry", "detected", [], None, None, False, ""
        self.lines, self.err, self.link, self.errhost, self.errhead = [], "", "", "", ""
        self.cause, self.details, self.details_open, self.armed, self.engine = "", "", False, None, False
        self.paths = []
        self.manual_open = self.confirm = self.revealed = self.edited = False
        self.fields = {}
        self.had_result = False
        self.app_id = ""
        self.changing = False
        self.own_id = False

    # ---- helpers ----
    def domain(self): return self.addr.split("@")[-1]
    def valid(self): return self.addr.count("@") == 1 and "." in self.domain() and not self.addr.startswith("@")
    def kind_of_domain(self):
        d = self.domain()
        if "wrong-mark-table" in self.m and d == "gmail.com": return "microsoft"
        return MARK_TABLE.get(d, "unknown")
    def field_kind(self):
        return self.kind_of_domain() if self.valid() else None
    def head_kind(self):
        if "head-marked-early" in self.m and self.field_kind(): return self.field_kind()
        if self.stage in ("summary", "validating") and self.field_kind():
            return self.field_kind()
        return None
    def complete(self):
        v = self.fields
        return bool(v.get("imapHost") and v.get("imapPort") and v.get("imapSecurity") and v.get("smtpHost") and v.get("smtpPort") and v.get("smtpSecurity"))
    def working(self): return self.stage in ("detecting", "validating")
    def found_line(self):
        return self.stage == "summary" and self.mode == "detected" and not self.manual_open and not self.err and self.complete() and "found-line-missing" not in self.m

    def controls(self):
        out = []
        def ctl(i, title, enabled=True, busy=False):
            k = "" if busy else KEYS[i]
            if "no-key-caps" in self.m and i in ("cancel", "continue"): k = ""
            if "found-no-keys" in self.m and i in ("cancel", "add") and self.stage == "summary": k = ""
            if "added-no-keys" in self.m and i in ("add-another", "done"): k = ""
            out.append({"id": i, "title": title, "key": k, "enabled": enabled, "busy": busy})
        if self.stage != "added": ctl("reveal", "Hide password" if self.revealed else "Show password", not self.working())
        if self.found_line(): ctl("details", "Details")
        cancel = lambda: ctl("cancel", "Cancel", not self.confirm) if self.style == "sheet" or "inline-has-cancel" in self.m else None
        st = self.stage
        if st == "entry":
            ctl("manual", "Manual setup"); cancel(); ctl("continue", "Continue", self.valid() or "continue-always-on" in self.m)
        elif st == "detecting":
            ctl("manual", "Skip to manual setup"); cancel(); ctl("continue", "Finding…", False, True)
        elif st == "summary":
            title = "Hide manual setup" if self.manual_open else ("Server settings" if self.err else "Manual setup")
            ctl("manual", title); cancel()
            if self.err: ctl("retry", "Try again")
            else: ctl("add", "Add", (not self.manual_open) or self.complete() or "add-on-when-incomplete" in self.m)
        elif st == "validating":
            ctl("manual", "Manual setup", False); cancel(); ctl("add", "Adding…", False, True)
        elif st == "added":
            ctl("add-another", "Add another"); ctl("done", "Done")
        if self.confirm:
            ctl("keep-checking", "Keep checking"); ctl("stop", "Stop")
        return out

    # ---- the verbs ----
    def handle(self, verb, args):
        ok = {"ok": True}
        if verb == "uistate":
            return {"ok": True, "state": {"surface": self.surface, "account_filter": self.account_filter}}
        if verb == "clock":
            self.clock += float(args[1]); return ok
        if verb == "fixture" and args[0] == "default-client":
            if args[1] not in ("set", "clear"): return {"ok": False, "error": "usage"}
            self.default_client = args[1] == "set"; return ok
        if verb == "fixture" and args[0] == "refuse":
            if args[1] not in CAUSES and args[1] not in ENGINE and args[1] != "engine.some_new_code": return {"ok": False, "error": "no refusal cause " + args[1]}
            self.armed = args[1]; return ok
        if verb == "click" and args[0].startswith(LANG_ID):
            code = args[0][len(LANG_ID):]
            self.lang = "en" if code == "system" else code; return ok
        if verb == "fixture":
            _, which, addr = args[:3]
            d = self.drafts.get(addr)
            if not d or d["state"] != "waiting": return {"ok": False, "error": "no sign-in is waiting for " + addr}
            if which == "approve":
                d["state"] = "done"
                self.added.append({"address": addr, "t": self.clock, "id": "acc-%d" % len(self.added), "kind": "microsoft"})
                if addr == self.addr: self.stage = "added"
            else:
                d["state"] = "refused"
                d["words"] = "" if "refusal-hidden" in self.m else "the provider refused the sign-in: " + (args[3] if len(args) > 3 else "")
            return ok
        if verb == "type":
            if self.focus == "address": self.addr += args[0]
            elif self.focus == "password": self.pw += args[0]
            elif self.focus == "appId": self.app_id += args[0]
            elif self.focus and "manual-type-lost" not in self.m: self.fields[self.focus] = str(self.fields.get(self.focus, "")) + args[0]
            return ok
        if verb == "key":
            return ok if self.key(args[0]) else {"ok": False, "error": "key does nothing"}
        if verb == "click":
            if "open-does-nothing" in self.m and args[0] == A + ".open": return ok
            self.surface = A if not args[0].endswith(".cancel") else self.surface
            if args[0] == A + ".open": self.surface = A
            return self.click(args[0][len(A) + 1:])
        if verb == "rows":
            return {"ok": True, "rows": self.rows(args[0][len(A) + 1:])}
        return {"ok": False, "error": "nothing"}

    def key(self, chord):
        if self.surface != A: return False
        if chord == "Enter":
            if "return-dead" in self.m: return False
            return self.primary()
        if chord == "Escape":
            if self.stage == "validating":
                if self.confirm:
                    if "second-esc-ignored" in self.m: return True
                    return self.stop()
                if "esc-stops-at-once" in self.m: return self.stop()
                self.confirm = True; return True
            if "esc-dead-at-entry" in self.m: return False
            return self.close()
        if chord == "⌘N": return False if "added-keys-dead" in self.m else self.add_another()
        if chord == "⌘Enter": return False if "added-keys-dead" in self.m else self.done()
        return False

    def primary(self):
        if self.stage == "entry": return self.detect()
        if self.stage == "summary": return self.add()
        if self.stage == "validating" and self.confirm: self.confirm = False; return True
        if self.stage == "added": return self.done()
        return False

    def close(self):
        a = self.added
        self.reset(); self.added = []
        self.surface = "mac-main"; return True

    def stop(self):
        if "stop-adds-account" in self.m: self.added.append({"address": self.addr, "t": self.clock, "id": "acc-x", "kind": self.kind_of_domain()})
        self.held = None
        self.lines = []; self.confirm = False; self.stage = "summary"; return True

    def add_another(self):
        if not self.added: return False
        a = self.added; self.reset(); self.added = a
        if "add-another-leaves" in self.m: self.surface = "mac-main"
        return True

    def done(self):
        if self.stage != "added": return False
        if "done-leaves-open" in self.m: return True
        self.account_filter = self.added[-1]["id"]
        return self.close()

    def click(self, id):
        ok = {"ok": True}
        err = {"ok": False, "error": "%s did nothing" % id}
        if id in ("address", "password"): self.focus = id
        elif id.startswith("summary."):
            if id.count(".") == 2 and id.split(".")[2] in ("ssl", "starttls"):
                if not (self.manual_open and self.mode == "manual"): return err
                if "security-seg-dead" in self.m and id.split(".")[1] == "smtpSecurity": return ok
                self.fields[id.split(".")[1]] = id.split(".")[2]
            else:
                if not (self.manual_open and self.mode == "manual"): return err
                if "field-focus-dead" in self.m and id.split(".")[1] == "username": return ok
                self.focus = id.split(".")[1]
        elif id == "start-over":
            if "start-over-keeps" in self.m: self.stage = "entry"; return ok
            a = self.added; self.reset(); self.added = a
        elif id == "fixture.hold-detection": self.hold_d = True
        elif id == "fixture.hold-validation": self.hold_v = True
        elif id == "fixture.hold-off": self.hold_d = self.hold_v = False
        elif id == "app-id-guide":
            if "guide-link-dead" in self.m or self.stage != "summary": return err
            self.links.append("guide:bring-your-own-sign-in.md")
        elif id == "oauth.field":
            if not self.oauth_kind() or self.uses_default(): return err
            self.focus = "appId"
        elif id == "oauth.own-id":
            if not self.uses_default() or "own-id-link-dead" in self.m: return err
            self.own_id = True
        elif id == "oauth.change":
            k = self.oauth_kind()
            if "change-dead" in self.m or not k or not self.saved(): return err
            self.changing = True; self.app_id = ""
        elif id == "oauth.sign-in": return ok if self.sign_in() else err
        elif id == "fixture.inline": self.style = "inline"
        elif id == "fixture.sheet": self.style = "sheet"
        elif id == "release":
            if not self.held: return err
            self.held(); self.held = None
        elif id == "open": pass
        elif id == "scrim":
            if "scrim-closes" in self.m: self.close()
        elif id == "signin.coming-soon":
            if "coming-soon-acts" in self.m: self.revealed = not self.revealed
        elif id == "app-password.link":
            if "app-password-link-dead" in self.m: return err
            self.links.append("https://myaccount.google.com/apppasswords")
        elif id == "error.link":
            if "error-link-dead" in self.m or not self.link: return err
            self.links.append(self.link)
        elif id == "error.consent-link":
            if "consent-link-dead" in self.m or not self.consent_url: return err
            self.links.append(self.consent_url)
        elif id == "error.consent-copy":
            if not self.consent_url: return err
            self.copied.append(self.consent_url)
        elif id == "error.details":
            if not (self.err and self.details) or "details-dead" in self.m: return err
            self.details_open = not self.details_open
        elif id == "oauth.guide":
            if "field-guide-missing" in self.m or not (self.oauth_kind() and self.stage == "summary"): return err
            self.links.append("guide:bring-your-own-sign-in.md")
        else:
            c = next((x for x in self.controls() if x["id"] == id or (id in ("add", "retry") and x["id"] in ("add", "retry"))), None)
            if c is None or not c["enabled"]: return err
            if id == "continue": self.detect()
            elif id in ("add", "retry"): self.add()
            elif id == "cancel":
                if self.stage == "validating":
                    if "cancel-closes-mid-check" in self.m: self.close()
                    else: self.confirm = True
                else: self.close() if "cancel-dead" not in self.m else None
            elif id in ("manual", "details"):
                if "details-dead" in self.m and id == "details": return ok
                if "skip-dead" in self.m and self.stage == "detecting": return ok
                self.toggle_manual()
            elif id == "add-another": self.add_another()
            elif id == "done": self.done()
            elif id == "keep-checking":
                if "keep-dead" not in self.m: self.confirm = False
            elif id == "stop": self.stop()
            elif id == "reveal":
                if "reveal-dead" not in self.m: self.revealed = not self.revealed
            else: return err
        return ok

    # ---- bring your own sign-in ----
    def oauth_kind(self):
        k = self.head_kind() if self.stage == "summary" else None
        if k == "google" and self.domain() in ("gmail.com", "googlemail.com") and "gmail-shows-oauth" not in self.m: return None
        return k if k in ("microsoft", "google") else None

    def draft(self): return self.drafts.get(self.addr)
    def saved(self): return self.draft() is not None and not self.changing
    def phase(self):
        d = self.draft()
        return d["state"] if d and d["state"] in ("waiting", "done", "refused") else "idle"

    def uses_default(self):
        """SPEC A88: a Microsoft address, no own ID saved, the owner's default present, the own-ID link not taken."""
        if "default-always-on" in self.m: return self.oauth_kind() == "microsoft" and not self.saved()
        return self.default_client and self.oauth_kind() == "microsoft" and not self.saved() and not self.own_id and "default-ignored" not in self.m

    def sign_in(self):
        k = self.oauth_kind()
        if not k or self.phase() in ("waiting", "done"): return False
        if self.uses_default():
            self.err = ""
            self.drafts[self.addr] = {"state": "waiting", "words": "", "url": "http://127.0.0.1/fixture-authority/authorize?x=1"}
            self.links.append(self.drafts[self.addr]["url"])
            return True
        setting = not self.saved() or bool(self.app_id)
        if setting and not self.app_id and "signin-without-id" not in self.m:
            self.err = "Paste your app ID first."; return False
        if setting:
            if "bad-id-accepted" not in self.m and not re.fullmatch(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}", self.app_id.lower()):
                self.err = MS_BAD_ID; return True
        self.err = ""
        self.drafts[self.addr] = {"state": "ready" if "signin-skips-waiting" in self.m else "waiting", "words": "", "url": "http://127.0.0.1/fixture-authority/authorize?x=1"}
        self.changing = False; self.app_id = ""
        self.links.append(self.drafts[self.addr]["url"])
        return True

    def provider_note(self):
        if self.stage not in ("summary", "validating"): return []
        k = self.kind_of_domain()
        if k == "microsoft" and not self.saved() and "ms-note-missing" not in self.m:
            return [{"kind": "microsoft", "note": "microsoft-no-password", "text": MS_NOTE, "link": "", "guide": "bring-your-own-sign-in.md"}]
        table = {"google": ("Google", "https://myaccount.google.com/apppasswords"), "icloud": ("iCloud", "https://account.apple.com/account/manage"),
                 "fastmail": ("Fastmail", "https://app.fastmail.com/settings/security/integrations")}
        if "plain-gets-note" in self.m and k == "unknown": k = "google"
        if "gmail-no-note" in self.m and k == "google": return []
        if "icloud-no-note" in self.m and k == "icloud": return []
        if "fastmail-no-note" in self.m and k == "fastmail": return []
        if k in table: return [{"kind": table[k][0].lower(), "note": "app-password", "text": "%s wants an app password for mail apps." % table[k][0], "link": table[k][1]}]
        return []

    def toggle_manual(self):
        if self.manual_open:
            self.manual_open = False
            if not self.had_result and not self.edited: self.stage = "entry"; self.mode = "detected"
            elif not self.edited and self.cfg: self.mode = "detected"
        else:
            if self.stage == "detecting": self.held = None
            if self.stage in ("entry", "detecting"): self.stage = "summary"
            if "manual-button-dead" in self.m and self.stage == "summary" and self.had_result: return
            self.mode = "manual"; self.manual_open = True

    def detect(self):
        if not self.valid(): return False
        d = self.domain()
        trace, cfg, soon, tenant, mark, src = DOM[d]
        trace = list(reversed(trace)) if "steps-out-of-order" in self.m and len(trace) > 2 else trace
        def done():
            self.steps = [{"id": i, "outcome": "found" if f else "miss", "text": "" if "step-words-empty" in self.m else STEP_WORDS[i]} for i, f in trace]
            self.stage = "summary"; self.had_result = True
            self.cfg = cfg; self.src = src
            self.fields = dict(cfg) if cfg else {"imapPort": 993, "imapSecurity": "ssl", "smtpPort": 587, "smtpSecurity": "starttls"}
            if "summary-drops-field" in self.m: self.fields.pop("username", None)
            self.mode = "detected" if cfg else "manual"; self.manual_open = (not cfg) or "manual-open-on-found" in self.m
            if "nothing-opens-detected" in self.m and not cfg: self.mode = "detected"
            self.soon = None if "no-coming-soon" in self.m else soon
            self.tenant = bool(tenant) and "tenant-not-named" not in self.m
        self.stage = "detecting"
        if self.hold_d and "held-shows-summary" not in self.m: self.held = done
        else: done()
        return True

    def refusal(self, cause, raw, host):
        """SPEC A87: the plain sentence for the cause, the raw words behind Details (closed)."""
        self.mode = "manual"; self.stage = "summary"
        self.cause, self.details, self.details_open, self.engine = cause, raw, False, False
        self.paths = []
        self.err = raw if "raw-as-sentence" in self.m else CAUSES[cause][0]
        self.errhost = host; self.errhead = host + " refused the password"
        self.lines = [{"text": "Checking the mail server…", "state": "failed"}]

    def engine_refusal(self, code):
        """Round 5: the catalogue sentence for the engine's code in the chosen language, the engine's words behind Details (closed)."""
        raw = ENGINE[code][1] if code in ENGINE else "engine words for an unknown code"
        known = code in ENGINE and "engine-wrong-code" not in self.m
        key = ENGINE[code][0] if known else None
        self.mode = "manual"; self.stage = "summary"
        self.engine, self.cause, self.details, self.details_open = True, code, raw, False
        consent = code == "oauth.consent_required" and "consent-no-paths" not in self.m
        self.paths = [word(k, self.lang) for k in PATH_KEYS] if consent or (code != "oauth.consent_required" and "consent-paths-always" in self.m) else []
        sentence = word(key, self.lang) if key else word("The server refused the account. Details shows its own words.", self.lang)
        self.consent_url = ""
        if code == "oauth.consent_required":
            sentence = word(CONSENT_OLD_KEY, self.lang) if "consent-sentence-no-domain" in self.m else sentence.replace("%@", self.domain())
        if self.default_client and (code == "oauth.consent_required" or "consent-link-always" in self.m):
            self.consent_url = CONSENT_URL % ("common" if "consent-link-wrong-tenant" in self.m else self.domain(), STAND_IN_ID)
        if "engine-wrong-sentence" in self.m and self.lang == "en": sentence = word("The server refused the account. Details shows its own words.", "en")
        self.err = raw if "engine-words-alone" in self.m else sentence
        self.errhost, self.errhead = "", word("The sign-in failed", self.lang)
        self.lines = [{"text": "Checking the mail server…", "state": "failed"}]
        return True

    def add(self):
        if self.stage != "summary": return False
        if self.armed:
            cause, self.armed = self.armed, None
            self.link = ""
            if cause not in CAUSES: return self.engine_refusal(cause)
            self.refusal(cause, "fixture words: " + CAUSES[cause][1], "smtp.srv.example.test" if cause == "smtp_refused" else "imap.srv.example.test")
            return True
        d = self.domain()
        if d in ("me.com", "blocked-m365.example.test"):
            self.mode = "manual"; self.stage = "summary"
            if d == "me.com":
                self.refusal("bad_password", "imap.mail.me.com refused the sign-in: wrong password. If this account uses an app-specific password, make one at https://appleid.apple.com and paste it here.", "imap.mail.me.com")
                self.link = "" if "no-link" in self.m else "https://appleid.apple.com"
                if "refusal-header-missing" in self.m: self.errhead = ""
            else:
                self.refusal("basic_auth_disabled", "outlook.office365.com: this organisation blocks password sign-in for IMAP (Basic authentication is disabled.)", "outlook.office365.com")
                if "blocked-tenant-silent" in self.m: self.err = "outlook.office365.com refused the sign-in."
                self.link = ""
            self.lines = [{"text": "Checking the mail server…", "state": "failed"}]
            if "refusal-adds-account" in self.m: self.added.append({"address": self.addr, "t": self.clock, "id": "acc-r", "kind": self.kind_of_domain()})
            return True
        def done():
            self.lines = [{"text": "Checking the mail server…", "state": "ok"}, {"text": "Checking sending…", "state": "ok"}]
            if "validation-lines-reordered" in self.m: self.lines.reverse()
            if "no-account-row" not in self.m:
                N[0] += 1
                self.added.append({"address": self.addr, "t": self.clock, "id": "acc-%d" % len(self.added), "kind": self.kind_of_domain()})
            self.stage = "added"
            if self.style == "inline":
                a = self.added; st = self.style; self.reset(); self.added = a; self.style = st
            elif "added-falls-back" in self.m:
                pass
        self.stage = "validating"; self.confirm = False
        self.lines = [{"text": "Checking the mail server…", "state": "running"}, {"text": "Checking sending…", "state": "pending"}]
        if self.hold_v and "held-validation-ok" not in self.m: self.held = done
        else: done()
        return True

    def rows(self, r):
        e = []
        sm = self.stage in ("summary", "validating", "added")
        if r == "fields": e = [{"id": "address"}, {"id": "password"}] + ([{"id": "notes"}] if "third-field" in self.m else [])
        elif r == "address": e = [{"text": self.addr, "locked": self.working()}]
        elif r == "password":
            e = [{"text": self.pw if "password-in-row" in self.m else "•" * len(self.pw), "length": len(self.pw), "visible": True, "revealed": self.revealed, "locked": self.working()}]
        elif r == "state":
            e = [{"stage": "entry" if ("added-falls-back" in self.m and self.stage == "added") else self.stage, "mode": self.mode, "held": self.held is not None,
                  "added": len(self.added), "sheet": self.surface == A, "style": self.style, "manual_open": self.manual_open,
                  "confirming_stop": self.confirm, "revealed": self.revealed, "opened_links": list(self.links), "copied_links": list(self.copied)}]
        elif r == "detection.step": e = self.steps
        elif r == "step":
            if self.stage == "detecting":
                n = min(len(self.steps) + 1, 6)
                n = 2 if "step-count-wrong" in self.m else n
                e = [{"text": STEP_WORDS[["ispdb", "autoconfig", "srv", "autodiscover", "mx", "guess"][n - 1]], "n": n, "of": 6, "count": "%d of 6" % n, "segments": 6, "done": n - 1}]
            elif self.stage == "validating":
                e = [{"text": "Checking the mail server…", "n": 1, "of": 2, "count": "" if "validating-no-count" in self.m else "1 of 2", "segments": 2, "done": 0}]
            elif self.stage == "added": e = [{"text": "Connecting…", "n": 0, "of": 0, "count": "", "segments": 0, "done": 0}]
            elif self.stage == "entry":
                e = [{"text": "" if "entry-line-missing" in self.m else "The password stays in this Mac’s Keychain.", "n": 0, "of": 0, "count": "", "segments": 0, "done": 0}]
            else:
                t = "" if self.err else ("found" if self.found_line() else (NOTE_NOTHING % self.domain() if self.steps else "Type the incoming and the sending server below."))
                e = [{"text": t, "n": 0, "of": 0, "count": "", "segments": 0, "done": 0}]
        elif r == "found":
            if self.found_line(): e = [{"imap": "%s:%s" % (self.fields["imapHost"], self.fields["imapPort"]), "smtp": "%s:%s" % (self.fields["smtpHost"], self.fields["smtpPort"]),
                                         "source": self.src, "mark": self.kind_of_domain()}]
        elif r == "marks":
            if self.field_kind() and self.stage != "added": e.append({"placement": "field", "kind": self.field_kind()})
            hk = self.added[-1]["kind"] if self.stage == "added" and self.added else self.head_kind()
            e.append({"placement": "head", "kind": hk or "mail"})
            if self.found_line(): e.append({"placement": "found", "kind": self.kind_of_domain()})
            e += [{"placement": "added", "kind": a["kind"]} for a in self.added]
        elif r == "controls": e = self.controls()
        elif r == "summary.mode": e = [{"mode": self.mode}] if sm else []
        elif r.startswith("summary.") and sm:
            f = r[8:]
            if f in EIGHT:
                v = self.fields.get(f, "")
                if f == "imapPort" or f == "smtpPort":
                    try: v = int(v)
                    except (ValueError, TypeError): pass
                e = [{"value": v, "editable": self.mode == "manual"}]
        elif r == "manual":
            m = next((x for x in self.controls() if x["id"] == "manual"), None)
            e = [{"text": m["title"] if m else "Manual setup", "visible": m is not None and self.mode == "detected" and self.stage == "summary", "open": self.manual_open}] if sm or self.stage == "entry" else []
        elif r == "signin.coming-soon": e = [{"text": "Sign in with Google, Microsoft or Yahoo coming soon", "enabled": False}] if "entry-line-missing" in self.m and self.stage == "entry" else []
        elif r == "provider-note": e = self.provider_note()
        elif r == "oauth":
            k = self.oauth_kind()
            if k:
                d = self.draft() or {}
                ph = self.phase()
                e = [{"kind": k, "client_saved": self.saved(), "phase": ph, "words": "Waiting for you in the browser…" if ph == "waiting" else d.get("words", ""),
                      "url": d.get("url", "") if ph == "waiting" else "", "caption": "Your organisation’s app ID",
                      "guide_link": "" if "field-guide-missing" in self.m else "How to get one", "guide": "bring-your-own-sign-in.md",
                      "default_client": self.uses_default(), "field_shown": not self.uses_default() and not self.saved(),
                      "own_id_link": "Use your organisation’s own ID" if self.uses_default() else "",
                      "text": self.app_id if "app-id-in-row" in self.m else "", "length": len(self.app_id)}]
        elif r == "tenant": e = [{"text": "Microsoft 365 organisation"}] if sm and self.tenant else []
        elif r == "app-password.steps": e = [{"link": "https://myaccount.google.com/apppasswords"}] if sm and self.kind_of_domain() == "google" else []
        elif r == "validation.line": e = self.lines
        elif r == "error":
            has = bool((self.errhost or self.engine) and self.cause and self.details)
            e = [{"text": self.err, "link": self.link, "host": self.errhost, "header": self.errhead, "cause": self.cause if has else "",
                  "details_available": has, "details_open": has and self.details_open, "details": self.details if has and self.details_open else "",
                  "details_label": word("Hide details" if self.details_open else "Details", self.lang), "paths": self.paths if has else [],
                  "consent_link": self.consent_url, "consent_link_text": word(LINK_KEY, self.lang).replace("%@", self.consent_url) if self.consent_url else "",
                  "consent_copy_label": word("Copy", self.lang) if self.consent_url else ""}] if self.err else []
        elif r == "accounts":
            e = [{"id": a["id"], "address": a["address"], "mark": a["kind"], "progress": "connecting" if (self.clock - a["t"] < 3 or "progress-stuck" in self.m) else "fetching the inbox"} for a in self.added]
        return e



# ---- doors-labels: every control of the sheet, its click door and the control it stands for, both read from the Swift sources (--selftest).
# (label, door file, door text, control file, control text); the label is the control's name in docs/COVERAGE.md "Labels, contacts and Add account (lane doors-labels)".
AA = "Sources/Features/AddAccount/"
DOORS = [
    ("Email field", "AddAccountFeature.swift", r'"\(A).address": go { model().setFocus(.address)', "AddAccountFieldsView.swift", r'accessibilityIdentifier("\(AddAccountWords.surface).address")'),
    ("Password field", "AddAccountFeature.swift", r'"\(A).password": go { model().setFocus(.password)', "AddAccountFieldsView.swift", r'accessibilityIdentifier("\(AddAccountWords.surface).password")'),
    ("Eye (show or hide the password)", "AddAccountFeature.swift", '"stop", "reveal"]', "AddAccountFieldsView.swift", r'accessibilityIdentifier("\(AddAccountWords.surface).reveal")'),
    ("Footer and Esc-question buttons (10)", "AddAccountFeature.swift", '"keep-checking", "stop", "reveal"]', "AddAccountButtons.swift", r'accessibilityIdentifier("\(AddAccountWords.surface).\(control.id)")'),
    ("Details on the found line", "AddAccountFeature.swift", '"manual", "details", "add-another"', "AddAccountStepView.swift", r'accessibilityIdentifier("\(AddAccountWords.surface).details")'),
    ("Manual setup text fields (7)", "AddAccountFeature.swift", 'for f in AddAccountDraft.fieldIds where f != "authMethod"', "AddAccountManualView.swift", r'accessibilityIdentifier("\(AddAccountWords.surface).summary.\(id)")'),
    ("Security segments (4)", "AddAccountFeature.swift", 'for f in ["imapSecurity", "smtpSecurity"] { for v in ["ssl", "starttls"]', "AddAccountManualView.swift", r'accessibilityIdentifier("\(AddAccountWords.surface).summary.\(field).\(value)")'),
    ("App-password link", "AddAccountFeature.swift", r'"\(A).app-password.link"', "AddAccountStepView.swift", r'accessibilityIdentifier("\(W.surface).app-password.link")'),
    ("How to get your app ID link", "AddAccountFeature.swift", r'"\(A).app-id-guide"', "AddAccountOAuthView.swift", r'accessibilityIdentifier("\(AddAccountWords.surface).app-id-guide")'),
    ("How to get one link", "AddAccountFeature.swift", r'"\(A).oauth.guide"', "AddAccountOAuthView.swift", r'accessibilityIdentifier("\(AddAccountWords.surface).oauth.guide")'),
    ("App ID field", "AddAccountFeature.swift", r'"\(A).oauth.field"', "AddAccountOAuthView.swift", r'accessibilityIdentifier("\(AddAccountWords.surface).oauth.field")'),
    ("Sign in", "AddAccountFeature.swift", r'"\(A).oauth.sign-in"', "AddAccountOAuthView.swift", r'accessibilityIdentifier("\(AddAccountWords.surface).oauth.sign-in")'),
    ("Change (the app ID)", "AddAccountFeature.swift", r'"\(A).oauth.change"', "AddAccountOAuthView.swift", r'accessibilityIdentifier("\(AddAccountWords.surface).oauth.change")'),
    ("Use your organisation's own ID", "AddAccountFeature.swift", r'"\(A).oauth.own-id"', "AddAccountOAuthView.swift", r'accessibilityIdentifier("\(AddAccountWords.surface).oauth.own-id")'),
    ("Details under a refusal", "AddAccountFeature.swift", r'"\(A).error.details"', "AddAccountErrorView.swift", r'accessibilityIdentifier("\(AddAccountWords.surface).error.details")'),
    ("Refusal page link", "AddAccountFeature.swift", r'"\(A).error.link"', "AddAccountErrorView.swift", r'accessibilityIdentifier("\(AddAccountWords.surface).error.link")'),
    ("Admin-consent link", "AddAccountFeature.swift", r'"\(A).error.consent-link"', "AddAccountErrorView.swift", r'accessibilityIdentifier("\(AddAccountWords.surface).error.consent-link")'),
    ("Admin-consent Copy", "AddAccountFeature.swift", r'"\(A).error.consent-copy"', "AddAccountErrorView.swift", r'accessibilityIdentifier("\(AddAccountWords.surface).error.consent-copy")'),
    ("Details under a failed added step", "AddAccountFeature.swift", r'"\(A).step.details": go { model().toggleStepDetails() }', "AddAccountStepView.swift", r'accessibilityIdentifier("\(AddAccountWords.surface).step.details")'),
    ("Scrim (a tap is swallowed)", "AddAccountFeature.swift", r'"\(A).scrim": go { true }', "AddAccountSheetView.swift", r'accessibilityIdentifier("\(AddAccountWords.surface).scrim")'),
    ("Key Return", "AddAccountFeature.swift", '"Enter": { m in up(m) && model().primaryAction() }', "AddAccountControls.swift", 'key: "↩"'),
    ("Key Esc", "AddAccountFeature.swift", '"Escape": { m in up(m) && model().escapePressed() }', "AddAccountControls.swift", 'key: "esc"'),
    ("Key Cmd N", "AddAccountFeature.swift", '"⌘N": { m in up(m) && model().addAnother() }', "AddAccountControls.swift", 'key: "⌘N"'),
    ("Key Cmd Return (hidden button)", "AddAccountFeature.swift", '"⌘Enter": { m in up(m) && model().doneTapped() }', "AddAccountCardView.swift", ".keyboardShortcut(.return, modifiers: .command)"),
]


def door_gap(root, entry, drop=None):
    """Which half of an entry is absent from the sources under `root`: [] when the door and its control are both there. `drop` removes one half first (the mutant)."""
    _, dfile, dtext, cfile, ctext = entry
    rd = lambda f: open(os.path.join(root, AA, f), encoding="utf-8").read()
    d, c = rd(dfile), rd(cfile)
    if drop == "door":
        d = d.replace(dtext, "")
    if drop == "control":
        c = c.replace(ctext, "")
    return [w for w, src, t in (("door", d, dtext), ("control", c, ctext)) if t not in src]


def selftest():
    def serve(mut):
        d = tempfile.mkdtemp(prefix="par-aa-")
        model = Model(mut)
        if "unclicked-control" in mut:
            _orig = model.controls
            model.controls = lambda: _orig() + [{"id": "help", "title": "Help", "key": "", "enabled": True, "busy": False}]

        class H(socketserver.StreamRequestHandler):
            def handle(self):
                for line in self.rfile:
                    q = json.loads(line.decode())
                    self.wfile.write((json.dumps(model.handle(q["verb"], q["args"])) + "\n").encode())
        srv = socketserver.ThreadingUnixStreamServer(os.path.join(d, "whoosh.sock"), H)
        threading.Thread(target=srv.serve_forever, daemon=True).start()
        return d, srv
    bad = 0
    d, srv = serve(set())
    base = drive(Ctl(d).connect())
    srv.shutdown()
    print("model (no mutant): %d of %d checks pass" % (sum(1 for _, ok, _ in base if ok), len(base)))
    for n, ok, why in base:
        if not ok: print("  RED on the model: %s %s" % (n, why)); bad += 1
    for name, mutant, words, fn in CHECKS:
        if mutant in ("bare-string", "guide-copy-differs", "guide-consent-silent", "consent-sentence-generic", "consent-link-words-english"):
            continue   # the source scan is mutated on a copy of the tree below
        d, srv = serve({mutant})
        res = {n: ok for n, ok, _ in drive(Ctl(d).connect())}
        srv.shutdown()
        red = [n for n, ok in res.items() if ok is False]
        verdict = "RED on %s" % name if res.get(name) is False else "NOT RED (red: %s)" % red
        print("mutant %-30s %-62s -> %s" % (mutant, words, verdict))
        if res.get(name) is not False: bad += 1
    # the source scan: a bare string put into a copy of a file must turn it red
    global ROOT
    real = ROOT
    tmp = tempfile.mkdtemp(prefix="par-aa-src-")
    sdir = os.path.join(tmp, "Sources", "Features", "AddAccount"); os.makedirs(sdir)
    for f in os.listdir(os.path.join(real, "Sources", "Features", "AddAccount")):
        open(os.path.join(sdir, f), "w", encoding="utf-8").write(open(os.path.join(real, "Sources", "Features", "AddAccount", f), encoding="utf-8").read())
    open(os.path.join(sdir, "AddAccountHeadView.swift"), "a", encoding="utf-8").write('\nlet bare = Text("Add your account")\n')
    ROOT = tmp
    got, want = next(fn for n, m, w, fn in CHECKS if n == "no-bare-string-in-the-sheet")(None)
    ROOT = real
    print("mutant %-30s %-62s -> %s" % ("bare-string", "a string is drawn without Words.text", "RED on no-bare-string-in-the-sheet" if got != want else "NOT RED"))
    if got == want: bad += 1
    # the guide copy: a differing bundled copy in a scratch tree must turn its check red
    g = tempfile.mkdtemp(prefix="par-aa-guide-")
    for sub_ in (("docs", "guides"), ("Sources", "Resources", "guides")): os.makedirs(os.path.join(g, *sub_))
    open(os.path.join(g, "docs", "guides", "bring-your-own-sign-in.md"), "w").write("one\n")
    open(os.path.join(g, "Sources", "Resources", "guides", "bring-your-own-sign-in.md"), "w").write("two\n")
    ROOT = g
    got, want = next(fn for n, m, w, fn in CHECKS if n == "guide-copy-matches-the-doc")(None)
    ROOT = real
    print("mutant %-30s %-62s -> %s" % ("guide-copy-differs", "the bundled guide differs from docs/guides", "RED on guide-copy-matches-the-doc" if got != want else "NOT RED"))
    if got == want: bad += 1
    # round 6: the two catalogue-reading checks, each on a scratch copy of the files it reads, with one edit
    def scratch(edit):
        t = tempfile.mkdtemp(prefix="par-aa-a97-")
        for rel in (("Sources", "Resources", "EntraGuide", "entraguide-steps.json"), ("Sources", "Resources", "Localizable.xcstrings")):
            os.makedirs(os.path.join(t, *rel[:-1]), exist_ok=True)
            open(os.path.join(t, *rel), "w", encoding="utf-8").write(open(os.path.join(real, *rel), encoding="utf-8").read())
        edit(t)
        return t
    def edit_json(t, rel, fn):
        f = os.path.join(t, *rel); d = json.load(open(f, encoding="utf-8")); fn(d); json.dump(d, open(f, "w", encoding="utf-8"), ensure_ascii=False)
    steps_rel, cat_rel = ("Sources", "Resources", "EntraGuide", "entraguide-steps.json"), ("Sources", "Resources", "Localizable.xcstrings")
    for mutant, chk, edit in (
        ("guide-consent-silent", "a97-the-guides-consent-step-says-the-mailbox-admin-approves-once-with-the-link",
         lambda t: edit_json(t, steps_rel, lambda d: [s_.pop("hint", None) for s_ in d["steps"] if s_["n"] == 26])),
        ("guide-consent-flag-dropped", "a97-the-guides-consent-step-says-the-mailbox-admin-approves-once-with-the-link",
         lambda t: edit_json(t, steps_rel, lambda d: [s_.pop("consent", None) for s_ in d["steps"] if s_["n"] == 26])),
        ("guide-consent-link-row-dropped", "a97-the-guides-consent-step-says-the-mailbox-admin-approves-once-with-the-link",
         lambda t: edit_json(t, steps_rel, lambda d: d["groups"].pop("consent", None))),
        ("consent-sentence-generic", "a97-the-consent-sentence-names-the-admin-and-once-in-every-language",
         lambda t: edit_json(t, cat_rel, lambda d: d["strings"][CONSENT_KEY]["localizations"]["fr"]["stringUnit"].update(
             value=d["strings"][CONSENT_KEY]["localizations"]["en"]["stringUnit"]["value"]))),
        ("consent-link-words-english", "a98-the-link-line-and-copy-words-exist-in-every-language",
         lambda t: edit_json(t, cat_rel, lambda d: d["strings"][LINK_KEY]["localizations"]["fr"]["stringUnit"].update(
             value=d["strings"][LINK_KEY]["localizations"]["en"]["stringUnit"]["value"]))),
    ):
        ROOT = scratch(edit)
        got, want = next(fn for n, m, w, fn in CHECKS if n == chk)(None)
        ROOT = real
        print("mutant %-30s %-62s -> %s" % (mutant, "scratch copy of the files, one edit", "RED on %s" % chk if got != want else "NOT RED"))
        if got == want: bad += 1
    # doors-labels: each control's door and its drawn control are in the sources; a mutant removes one half and goes red
    gaps = [(e[0], door_gap(ROOT, e)) for e in DOORS if door_gap(ROOT, e)]
    print("door table: %d controls each have a click door and a drawn control%s" % (len(DOORS), (" -> RED " + str(gaps)) if gaps else ""))
    bad += bool(gaps)
    for e in DOORS:
        for half in ("door", "control"):
            red = half in door_gap(ROOT, e, drop=half)
            print("mutant %-30s %-62s -> %s" % ("%s-removed" % half, "'%s'" % e[0], "RED on the door table" if red else "NOT RED"))
            bad += not red
    print("%d checks, %d mutants not red" % (len(CHECKS), bad))
    return 1 if bad else 0


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        sys.exit(selftest())
    try:
        sys.exit(report(drive(Ctl().connect())))
    except CouldNotDrive as e:
        print("0 checks, 0 failed, %d not judged  (%s)" % (len(CHECKS), e)); sys.exit(3)
