"""J4: Add 30 accounts, move them to the iPhone (JOURNEYS J4.1-J4.12). J4.1-J4.5 rewritten 2026-11-08 for the IMAP add-account flow (fixtures/addaccount/*.json, surface mac-add-account). Fixture accounts and colours.

Written from docs/JOURNEYS.md J4 and the design fixture, never from a built screen.
Surfaces are the ids of docs/lane-dag.json: mac-welcome-accounts, mac-settings-accounts, iphone-welcome-accounts,
iphone-inbox, iphone-settings.

The iPhone half (J4.10, J4.11) is driven on a second socket, $WHOOSH_IPHONE_SOCKET_DIR (the simulator rig of
slice-j7-iphone). Without it those reads raise Unreadable, so the checks are `not judged` (COULD-NOT-DRIVE),
never a pass and never a crash. Nothing here connects a real mail account: the accounts are the fixture's.
"""
import json
import os
import subprocess
import sys
import time

from lib.ctl import Ctl, CouldNotDrive, Unreadable
from lib import theme as _theme
from lib.fixture import FX, ACCOUNT, ACCOUNT_COUNT, FAILING_ACCOUNT, ROOT, const

# Words: a hidden test copy speaks the language Words.swift picks (the stored choice `whoosh.language`, else the language list of the rig identity,
# else the Mac's own list; the hidden run writes `en` into the rig identity), so an expected sentence is read from the catalogue
# (Sources/Resources/Localizable.xcstrings: the key is the English source sentence, the value the one in LANG), never typed in English.
# WHOOSH_JOURNEY_LANG names another catalogue language for a copy started in it. English has no stored value for most keys: the key is the text.
with open(os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8") as _c:
    CATALOGUE = json.load(_c)["strings"]
_CODES = ["en", "fr", "es", "pt", "de", "it", "nl", "pl", "sv", "da", "nb", "fi", "ja", "zh-Hans"]      # Words.languages
_RIG_DOMAIN = "io.nexus.whoosh.rig"


def _defaults(*args):
    try:
        out = subprocess.run(["defaults"] + list(args), capture_output=True, text=True, timeout=10)
        return out.stdout if out.returncode == 0 else ""
    except (OSError, subprocess.SubprocessError):
        return ""


def _mac_language():
    pick = _defaults("read", _RIG_DOMAIN, "whoosh.language").strip()
    if pick in _CODES:
        return pick
    for args in (("read", _RIG_DOMAIN, "AppleLanguages"), ("read", "-g", "AppleLanguages")):
        for line in _defaults(*args).splitlines():
            pref = line.strip().strip(",").strip('"').lower()
            code = next((c for c in sorted(_CODES, key=len, reverse=True) if pref == c.lower() or pref.startswith(c.lower() + "-")), None)
            if code:
                return code
    return "en"


LANG = os.environ.get("WHOOSH_JOURNEY_LANG") or _mac_language()


def say(key, *args):
    """The catalogue sentence for an English source key, in LANG (English: the key itself when the catalogue stores no English value); each %@ or %lld takes the next argument. A key the catalogue lacks is a KeyError (a red, never a guess)."""
    entry = CATALOGUE[key]
    unit = entry.get("localizations", {}).get(LANG, {}).get("stringUnit", {}).get("value")
    if unit is None:
        if LANG != "en":
            raise KeyError("no %s sentence for %r" % (LANG, key))
        unit = key
    out = unit
    for a in args:
        for hole in ("%@", "%lld"):
            if hole in out:
                out = out.replace(hole, str(a), 1)
                break
    return out


def expect_why(s, name, thunk, expected, why):
    """`expect`, whose failure line also carries the values that decide it (`why()`: rows read, window list, state before): an undecided case prints what decides it."""
    def run():
        got = thunk()
        s._eg = (expected, got)
        if got == expected:
            return True, "expected %r, got %r" % (expected, got)
        try:
            more = why()
        except Exception as e:      # noqa: BLE001  a context read never hides the verdict it explains
            more = "unreadable (%s: %s)" % (type(e).__name__, e)
        return False, "expected %r, got %r | deciding values: %s" % (expected, got, more)
    s._record(name, run)


def settle(s, read, want, secs=6.0):
    """An action: reads the state (`read()`) until it equals `want` or `secs` pass; never a blind sleep. The check after it judges the state."""
    def go():
        end = time.monotonic() + secs
        while time.monotonic() < end:
            try:
                if read() == want:
                    return
            except Unreadable:
                pass
            time.sleep(0.1)
    s._act(go)

PICKED_ICON = "briefcase"        # a library glyph that is not A4's fixture icon (star)


def a4_list_rows(s):
    return [r for r in s.ctl.rows("mac-main.list") if r["account"] == A4["id"]]


def open_a4_thread(s):
    """Bring the main window forward, move focus by J/K to a list row of A4 (as J1.goto does) and open it with Enter; no-op in a dry run."""
    if s.j.ctl is None:
        return
    s.click("dock.whoosh")
    for _ in range(40):
        try:
            rows = s.ctl.rows("mac-main.list")
            st = s.ctl.uistate()
        except Unreadable:
            return
        order = [r["id"] for r in rows]
        mine = [r["id"] for r in rows if r["account"] == A4["id"]]
        if not mine or st.get("focus_message") in mine:
            break
        s.key("J" if order.index(mine[0]) > (st.get("focus_index") or 0) else "K")
    s.key("Enter")


ID = "J4"
TITLE = _theme.title("Connect 30 accounts on the Mac and move them to the iPhone")
TARGET = "LOCAL"
VISIBLE_OUTCOME = "30 named, coloured accounts work on the Mac and arrive on the iPhone with no password asked"

W = "mac-welcome-accounts"
S = "mac-settings-accounts"
IW, II, IS = "iphone-welcome-accounts", "iphone-inbox", "iphone-settings"
ACC = W + ".accounts"
rows = lambda s, ax=ACC: s.ctl.rows(ax)
A1, A2, A3, A4 = (FX["accounts"][i] for i in range(4))
IDS = [a["id"] for a in FX["accounts"]]
MUTED = (IDS[1], IDS[2])


def phone(s):
    """The iPhone rig's socket; Unreadable (not judged) while there is none."""
    j = s.j
    if j.ctl is None:
        raise Unreadable("dry run: no socket")
    if getattr(j, "_phone", None) is None:
        d = os.environ.get("WHOOSH_IPHONE_SOCKET_DIR", "")
        if not d:
            raise Unreadable("no iPhone rig: WHOOSH_IPHONE_SOCKET_DIR is not set (slice-j7-iphone)")
        try:
            j._phone = Ctl(d).connect()
        except CouldNotDrive as e:
            raise Unreadable("iPhone rig: %s" % e)
    return j._phone


def pclick(s, ax):
    try:
        phone(s).click(ax)
    except Unreadable as e:
        s.j.notes.append("%s iPhone action %s skipped: %s" % (s.id, ax, e))


def parrive(s, ax, surface):
    pclick(s, ax)
    s.expect("arrives-by-control", lambda: phone(s).uistate().get("surface"), surface)


AA = "mac-add-account"
SW = "mac-settings-window"
MENU = "mac-main.sidebar.accounts-menu"
AAD = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "fixtures", "addaccount")


def _load(name):
    with open(os.path.join(AAD, name + ".json"), encoding="utf-8") as f:
        return json.load(f)


STEPS = _load("steps")
DOM = {d: _load(d) for d in ("gmail.com", "icloud.com", "me.com", "fastmail.com", "srv.example.test", "m365.example.test",
                             "blocked-m365.example.test", "guess-only.example.test", "nothing.example.test")}
FIELDS = STEPS["summary_fields"]
# The step words as the sheet prints them: steps.json writes the autoconfig line with a straight apostrophe, the sheet (and
# par-addaccount's STEP_WORDS) with the typographic one. Same words; only the apostrophe is normalised here.
WORDS = {k: say(v.replace("'", "’")) for k, v in STEPS["progress_words"].items()}
VALIDATION_LINES = [say(v) for v in STEPS["validation_lines"]]
MANUAL, MANUAL_HIDE = say(STEPS["manual_label"]), say("Hide manual setup")
TENANT_GENERIC = say("Microsoft 365 organisation")      # AddAccountWords.tenant: the sheet's one line for a Microsoft 365 organisation
BASIC_AUTH_SENTENCE = say("Microsoft no longer accepts passwords for mail. Sign in with your organisation’s app ID below.")   # AddAccountWords.causeSentence(.basicAuthDisabled)
NO_NOTIFICATIONS = say("no notifications")
# J4.5 (signed 2026-11-08): Fastmail is NOT in the provider database (recorded 404); detection reaches SRV. fastmail.com.json still
# records the older ispdb hit, so the trace below is the signed step's, not the file's.
FASTMAIL_TRACE = [{"id": "ispdb", "outcome": "miss"}, {"id": "autoconfig", "outcome": "miss"}, {"id": "srv", "outcome": "found"}]
# J4.1 "nothing detected": Manual setup with the eight fields editable. The hosts, username and method stay empty; the ports and
# security start on the usual ones the guess step probes (steps.json guess_ports, the PRD owner words), which the sheet pre-fills.
GP = STEPS["guess_ports"]
NOTHING_CFG = dict(DOM["nothing.example.test"]["config"], imapPort=GP["imap"], imapSecurity="ssl", smtpPort=GP["smtp_starttls"], smtpSecurity="starttls")
BEFORE = {}   # counts read just before a press (not checks): the press's effect is judged against them


def R(s, ax):
    """Rows at an id; an id nobody serves yet reads 'could not drive: <id> not served' (the screen is not built)."""
    try:
        return s.ctl.rows(ax)
    except Unreadable as e:
        if "no rows" in str(e) or "nothing" in str(e):
            raise Unreadable("could not drive: %s not served" % ax)
        raise


def soft(s, ax, key):
    """One field of a row that the screen may leave out. The screen itself must be served (its fields row is read first);
    then a missing row is a wrong answer (None), not an unjudged one."""
    R(s, AA + ".fields")
    try:
        return s.ctl.rows(ax)[0][key]
    except Unreadable as e:
        if "no rows" in str(e):
            return None
        raise
    except IndexError:
        return None


def lines(s, ax):
    """Rows a control press must produce. The screen is anchored by its fields row; an empty answer after the press is a RED."""
    R(s, AA + ".fields")
    try:
        return s.ctl.rows(ax)
    except Unreadable as e:
        if "no rows" in str(e):
            return []
        raise


def core_or_gap(s, path):
    """A core read the signed step needs. A path the core does not serve is a RED naming the gap (the step's result cannot be
    read on this build), never a silent `not judged`."""
    try:
        return s.ctl.core(path)
    except Unreadable as e:
        if "no core value" in str(e):
            return "not served: %s" % path
        raise


def snap(s, key, fn):
    """Read a baseline before a press; None in a dry run (every check reading it is then not judged anyway)."""
    try:
        BEFORE[key] = fn()
    except (Unreadable, KeyError, IndexError, TypeError):
        BEFORE[key] = None


def field(s, name, text):
    s.click("%s.%s" % (AA, name))
    s.type(text)


def enter(s, fx):
    """Address and password only, then Continue (JOURNEYS J4.1: the whole first screen)."""
    field(s, "address", fx["address"])
    field(s, "password", fx["password"])
    s.click(AA + ".continue")


def new_id(s, fx):
    rs = R(s, AA + ".accounts")
    ids = [r["id"] for r in rs if r["address"] == fx["address"]]
    if not ids:
        raise IndexError("no account row for %s; the sheet lists %r" % (fx["address"], [(r["address"], r.get("progress")) for r in rs]))
    return ids[0]


def sheet_state(s):
    """What decides an add that did not land: the sheet's own rows after the press (accounts, validation lines, error, mode, sign-in row, notice) and the window list."""
    def rd(ax, keys):
        try:
            return [{k: r.get(k) for k in keys} for r in s.ctl.rows(ax)]
        except Unreadable as e:
            return "no rows (%s)" % str(e)[-60:]
    try:
        wins = [(f.get("title"), f.get("kind")) for f in s.ctl.winframes()]
    except Unreadable as e:
        wins = "unreadable (%s)" % e
    return ("surface=%r windows=%r accounts=%r validation=%r error=%r mode=%r oauth=%r notice=%r"
            % (s.ctl.uistate().get("surface"), wins, rd(AA + ".accounts", ("address", "progress")), rd(AA + ".validation.line", ("text", "state")),
               rd(AA + ".error", ("header", "text", "cause")), rd(AA + ".summary.mode", ("mode",)), rd(AA + ".oauth", ("kind", "phase", "words")), rd(AA + ".notice", ("text",))))


def not_served_folders(s, fx):
    v = core_or_gap(s, "account/%s/folders" % new_id(s, fx))
    if isinstance(v, str) and v.startswith("not served"):
        raise Unreadable("this build's fixture core serves no folder map (%s): FixtureCore.swift serves account/<id>/ address, colour, order, password_stored and credential only; "
                         "the recorded roles are built in FixtureAddAccount.swift:191 and reach no read door" % v)
    return v


def detected(s, fx, tag, mode="detected", trace=None, config=None):
    trace = trace or fx["trace"]
    s.expect("detection-order" + tag, lambda: [(r["id"], r["outcome"]) for r in lines(s, AA + ".detection.step")],
             [(t["id"], t["outcome"]) for t in trace])
    s.expect("detection-words" + tag, lambda: [r["text"] for r in lines(s, AA + ".detection.step")], [WORDS[t["id"]] for t in trace])
    s.expect("summary-mode" + tag, lambda: R(s, AA + ".summary.mode")[0]["mode"], STEPS["modes"][mode])
    s.expect("summary-eight-fields" + tag, lambda: {f: soft(s, "%s.summary.%s" % (AA, f), "value") for f in FIELDS}, config or fx["config"])


def ops_seen(s, fx):
    """The protocol steps the validation lines report, in order: LOGIN (signed in), LIST (folders read), EHLO (answered), AUTH."""
    c = fx["config"]
    want = [("IMAP LOGIN", "Signed in to %s." % c["imapHost"]), ("IMAP LIST", "Read "), ("SMTP EHLO", "%s answered." % c["smtpHost"]),
            ("SMTP AUTH", "Signed in to %s for sending." % c["smtpHost"])]
    details = [(r["state"], d) for r in lines(s, AA + ".validation.line") for d in r.get("details", [])]
    return [op for op, words in want if any(st == "ok" and d.startswith(words) for st, d in details)]


def validated(s, fx, tag):
    """Add on the summary: IMAP then SMTP, the new account row, folders (JOURNEYS J4.2 rows)."""
    snap(s, "n" + tag, lambda: len(s.ctl.rows(AA + ".accounts")))
    s.click(AA + ".add")
    why = lambda: "before the press %s accounts; after it: %s" % (BEFORE.get("n" + tag), sheet_state(s))
    expect_why(s, "validation-lines" + tag, lambda: [r["text"] for r in lines(s, AA + ".validation.line")], VALIDATION_LINES, why)
    expect_why(s, "validation-ops" + tag, lambda: ops_seen(s, fx), fx["ops"], why)
    expect_why(s, "row-progress" + tag, lambda: R(s, AA + ".accounts")[-1]["progress"], STEPS["row_progress"][0], why)
    s.raw("clock", "advance", 5)
    expect_why(s, "row-progress-next" + tag, lambda: R(s, AA + ".accounts")[-1]["progress"], STEPS["row_progress"][1], why)
    n0 = BEFORE["n" + tag]
    expect_why(s, "account-rows" + tag, lambda: (R(s, AA + ".accounts")[-1]["address"], len(R(s, AA + ".accounts"))),
               (fx["address"], None if n0 is None else n0 + 1), why)
    expect_why(s, "folders-mapped" + tag, lambda: not_served_folders(s, fx), fx["folder_map"], why)


def refused(s, fx, tag):
    """Add on a case the server refuses: the refusal in words (header, sentence, the server's own words behind Details), Manual setup
    reopens editable, and no account is added (absence with its positive control: the sheet already lists the accounts added)."""
    snap(s, "n" + tag, lambda: len(s.ctl.rows(AA + ".accounts")))
    s.click(AA + ".add")
    s.click(AA + ".error.details")
    # the fixture file's "coming soon" is retired (AddAccountFeature.swift:31): a blocked tenant's plain sentence is now the cause's catalogue sentence
    want = [BASIC_AUTH_SENTENCE if w == "coming soon" else w for w in fx["refusal_words_contain"]]
    def words():
        e = R(s, AA + ".error")[0]
        t = " ".join([e.get("header", ""), e.get("text", ""), e.get("details", "")])
        return tuple(w in t for w in want)
    why = lambda: "wanted %r; error rows %r; after the press: %s" % (want, [dict(r) for r in s.ctl.rows(AA + ".error")], sheet_state(s))
    expect_why(s, "refusal-words" + tag, words, tuple(True for _ in want), why)
    s.expect("reopens-manual" + tag, lambda: (R(s, AA + ".summary.mode")[0]["mode"],
             all(R(s, "%s.summary.%s" % (AA, f))[0]["editable"] for f in FIELDS)), (STEPS["modes"]["manual"], True))
    n0 = BEFORE["n" + tag]
    s.expect("not-added" + tag, lambda: (len(R(s, AA + ".accounts")), len(R(s, AA + ".accounts")) >= 1), (n0, True))


def settings_rows(s):
    """The accounts as the Settings pane lists them, by address. A pasted Welcome row carries an address only (WelcomeModel.swift:64-69); the name, colour and
    signature of an account are the core's (the fixture adopts the design fixture's own account for a pasted address: FixtureCore.swift `addAccount`), and
    the Accounts pane is where a person reads them (SettingsAccountsMacFeature.swift `row`)."""
    return {r["address"]: r for r in s.ctl.rows(S + ".accounts")}


def tenant_line(s):
    """The tenant line the sheet shows for a Microsoft 365 organisation. The engine's detection answers a flag, never the organisation's name
    (DetectionResult.microsoft365Tenant, a Bool; Autodiscover returns no display name), so the line is AddAccountWords.tenant, the same sentence for every
    organisation (AddAccountRows.swift:80). The "tenant" name in fixtures/addaccount is a fixture note the app cannot show."""
    return R(s, AA + ".tenant")[0]["text"]


def another(s):
    """Add another, then the next address. After a refusal the sheet stays on the summary, where Add another is not offered: the
    check goes red there, and Start over (a control on the same sheet) clears it so the walk goes on."""
    s.arrive(("click", AA + ".add-another"), AA)
    expect_why(s, "address-cleared", lambda: R(s, AA + ".address")[0]["text"], "", lambda: sheet_state(s))   # Add another keeps the screen: only a cleared field proves the click
    s.click(AA + ".start-over")   # a no-op after a working Add another; after a refused Add (Add another not offered) it clears the sheet


def restart(s):
    """After a refusal the sheet stays on Manual setup, where Add another is not offered: Start over clears it for the next address."""
    s.click(AA + ".start-over")


def drive(j):
    nf, sv = DOM["nothing.example.test"], DOM["srv.example.test"]
    s = j.step("J4.1", "Type an address and password, detect the provider")
    s.raw("fixture", "reset")   # the sample as launched: in the run of every journey an earlier one has healed the failing account (J4.6 and J4.8 need it failing) and left its own state
    # main53: no first-launch Welcome on the fixture core (the first-start flow is main54). The sheet's door is the sidebar's
    # accounts menu, "Add account…" (par-sidebar par-add-account-routes-to-its-surface).
    s.click("dock.whoosh")
    _theme.apply(s)
    s.click(MENU + ".up")
    s.arrive(("click", MENU + ".add"), AA)
    s.expect("two-fields-only", lambda: [r["id"] for r in R(s, AA + ".fields")], STEPS["first_screen_fields"])
    enter(s, nf)
    detected(s, nf, "-nothing", "manual", config=NOTHING_CFG)
    # nothing found: Manual setup is open, so its control offers to hide it (par-addaccount manual-setup-opens-when-nothing-found)
    s.expect("manual-label", lambda: (R(s, AA + ".manual")[0]["open"], R(s, AA + ".manual")[0]["text"]),
             (True, MANUAL_HIDE))
    s.expect("manual-fields-editable", lambda: {f: R(s, "%s.summary.%s" % (AA, f))[0]["editable"] for f in FIELDS},
             {f: True for f in FIELDS})
    s.expect("typed-control", lambda: R(s, AA + ".address")[0]["text"], nf["address"])
    s.click(AA + ".start-over")
    s.expect("start-over-clears", lambda: (R(s, AA + ".address")[0]["text"], R(s, AA + ".password")[0]["text"]), ("", ""))
    enter(s, sv)
    detected(s, sv, "-srv")
    s.expect("manual-offered", lambda: (R(s, AA + ".manual")[0]["visible"], R(s, AA + ".manual")[0]["text"]), (True, MANUAL))
    s.expect("no-tenant-line-srv", lambda: soft(s, AA + ".tenant", "text"), None)   # the positive control of tenant-named: a plain IMAP domain shows no organisation line

    s = j.step("J4.2", "Add: check the mail server, then sending")
    validated(s, sv, "")
    sid = lambda: new_id(s, sv)
    s.expect("credential-in-keychain", lambda: s.ctl.core("account_credential/%s/stored" % sid()), True)
    s.expect("account-exists-control", lambda: s.ctl.core("account/%s/address" % sid()), sv["address"])
    # absence with its positive control: the same account's address reads (above), its password does not
    s.expect("no-password-in-state", lambda: (core_or_gap(s, "account/%s/password" % sid()).startswith("not served"),
             s.ctl.core("account/%s/password_stored" % sid())), (True, True))

    s = j.step("J4.3", "Microsoft 365: tenant named, sign-in coming soon")
    mt, bt = DOM["m365.example.test"], DOM["blocked-m365.example.test"]
    another(s)
    enter(s, mt)
    detected(s, mt, "-tenant")
    s.expect("tenant-named", lambda: tenant_line(s), TENANT_GENERIC)
    # retired 2026-11-09 (AddAccountFeature.swift:31): nothing is "coming soon" now; the screen is anchored (its fields row is read first), the line must be absent
    s.expect("coming-soon", lambda: soft(s, AA + ".signin.coming-soon", "text"), None)
    s.expect("manual-offered", lambda: R(s, AA + ".manual")[0]["visible"], True)
    validated(s, mt, "-tenant")
    another(s)
    enter(s, bt)
    s.expect("blocked-tenant-named", lambda: tenant_line(s), TENANT_GENERIC)
    refused(s, bt, "-blocked")

    s = j.step("J4.4", "iCloud with an app-specific password")
    ic, me = DOM["icloud.com"], DOM["me.com"]
    restart(s)
    enter(s, ic)
    detected(s, ic, "-icloud")
    s.expect("app-password-steps-link", lambda: (len(R(s, AA + ".app-password.steps")) > 0,
             R(s, AA + ".app-password.steps")[0]["link"].startswith(ic["steps_link_prefix"])), (True, True))
    validated(s, ic, "-icloud")
    another(s)
    enter(s, me)
    refused(s, me, "-wrong-password")
    s.expect("apple-link-in-refusal", lambda: R(s, AA + ".error")[0]["link"].startswith(me["link_prefix"]), True)

    s = j.step("J4.5", "Fastmail, Gmail with the app-password route, a host nobody lists")
    fm, gm, go = DOM["fastmail.com"], DOM["gmail.com"], DOM["guess-only.example.test"]
    restart(s)
    enter(s, fm)
    detected(s, fm, "-fastmail", trace=FASTMAIL_TRACE)
    validated(s, fm, "-fastmail")
    another(s)
    enter(s, gm)
    detected(s, gm, "-gmail")
    s.expect("gmail-coming-soon", lambda: soft(s, AA + ".signin.coming-soon", "text"), None)   # retired 2026-11-09, as above
    s.expect("gmail-app-password-route", lambda: (R(s, AA + ".password")[0]["visible"], len(R(s, AA + ".app-password.steps")) > 0,
             R(s, AA + ".app-password.steps")[0]["link"].startswith(gm["steps_link_prefix"])), (True, True, True))
    validated(s, gm, "-gmail")
    another(s)
    enter(s, go)
    detected(s, go, "-guess")
    validated(s, go, "-guess")
    s.click(AA + ".cancel")

    s = j.step("J4.6", "Name, colour, order and sign 30 accounts")
    # main53: the Welcome window opens only at its import step or its QR step. At this build "Import mail…" opens an import sheet inside the one Settings
    # window (SettingsAccountsMacFeature.swift:81 `openImport`, AccountsSession.swift:49), not the Welcome window; the Welcome list is reached by its other
    # door, Settings > Accounts "Move accounts to iPhone" (WelcomeMacFeature.swift:63), which opens the same window. The pasted list goes through the
    # screen's own Paste list control.
    s.click(SW + ".open")
    s.click(SW + ".pane-row.accounts")
    s.click(S + ".import")
    s.expect("import-opens-its-sheet", lambda: (s.ctl.uistate().get("surface"), s.ctl.rows(S + ".import.sheet")[0]["open"]), (S, True))
    s.click(S + ".import.close")      # the sheet is put away before the next door
    s.arrive(("click", S + ".move-to-iphone"), W)
    s.click(W + ".reset")
    s.expect("list-starts-empty", lambda: s.ctl.rows(W + ".summary")[0]["total"], 0)   # so the 30 below come from the paste
    s.type(" ".join(a["address"] for a in FX["accounts"]))
    s.click(W + ".paste-list")
    s.expect("thirty-rows", lambda: len(rows(s)), ACCOUNT_COUNT)
    s.expect("order-on-screen", lambda: [r["address"] for r in rows(s)], [a["address"] for a in FX["accounts"]])
    s.expect("names", lambda: [settings_rows(s)[a["address"]]["name"] for a in FX["accounts"]], [a["name"] for a in FX["accounts"]])
    s.expect("colours", lambda: [settings_rows(s)[a["address"]]["colour"] for a in FX["accounts"]], [a["colour"] for a in FX["accounts"]])
    s.expect("signature-per-address", lambda: sum(1 for a in FX["accounts"] if settings_rows(s)[a["address"]]["signature"] == a["signature"]), ACCOUNT_COUNT)
    s.expect("shortcut-preset-chosen", lambda: s.ctl.rows(W + ".summary")[0]["preset"] in ("Superhuman", "GotEmail"), True)
    s.expect("first-screen-before-history", lambda: s.ctl.core("sync_state/%s/first_screen_at" % A1["id"]) is not None, True)
    s.click(W + ".connect-waiting")
    # failure branch: one account fails, its row says why, the other 29 carry on. The connecting runs on its own tasks: read until no row waits (bounded).
    bad_addr = ACCOUNT[FAILING_ACCOUNT]["address"]
    settle(s, lambda: all(r["phase"] not in ("waiting", "connecting", "fetching") for r in rows(s)), True)   # each row has its end: healthy or refused (a row still fetching has no status yet)
    def why6():
        rs = rows(s)
        ph = {}
        for r in rs:
            ph[r["phase"]] = ph.get(r["phase"], 0) + 1
        return "phases %r; the failing row %r; surface %r; windows %r" % (ph, [(r["phase"], r["status"]) for r in rs if r["address"] == bad_addr], s.ctl.uistate().get("surface"),
                                                                       [(f.get("title"), f.get("kind")) for f in s.ctl.winframes()])
    # The row shows the core's own words for the failing account's sync error (WelcomeModel.swift:90 `status = w`; FixtureCore.swift:2143 "needs you to sign in again"),
    # raw, in the core's language: the design fixture's "sign in again" (ACCOUNT[...]["status"]) is the phrase it contains, never the whole row text.
    why_words = "needs you to sign in again"
    assert ACCOUNT[FAILING_ACCOUNT]["status"] in why_words
    expect_why(s, "failing-row-says-why", lambda: [r for r in rows(s) if r["address"] == bad_addr][0]["status"], why_words, why6)
    expect_why(s, "other-29-carry-on", lambda: sum(1 for r in rows(s) if r["address"] != bad_addr and r["phase"] in ("connecting", "healthy")), ACCOUNT_COUNT - 1, why6)

    s = j.step("J4.7", "Open the inbox while history loads")
    s.expect("starts-on-welcome", lambda: s.ctl.uistate().get("surface"), W)   # so a dead Open inbox cannot pass by being on mac-main already
    s.raw("fixture", "indexing", "10")   # history still loading: 10 of the 30 indexed
    s.arrive(("click", W + ".open-inbox"), "mac-main")
    s.click("dock.whoosh")   # the status line is read on the main window even when Open inbox did nothing (its arrival check is red then)
    line = lambda: s.ctl.rows("mac-main.sync.line")[0]["line"]   # the status line's summary (MacShellFeature: mac-main.sync.line)
    # The line says "<healthy> of <all> accounts synced" (MacSyncLine.read, MacSyncStatus.swift:93), with every account the core lists, the ones added in J4.2-J4.5 included:
    # it carries no indexed count. The indexed count is the core's own `sync_state/<id>/history_fetched`; the fixture's failing account never has its history (FixtureCore.swift:2023).
    def indexed():
        return sum(1 for a in IDS if s.ctl.core("sync_state/%s/history_fetched" % a) is True)
    def counts():
        ids = s.ctl.core("account/order")
        return len(ids), sum(1 for a in ids if s.ctl.core("sync_state/%s/healthy" % a) is True)
    def line_ok():
        total, healthy = counts()
        return line().startswith(say("%@ of %lld accounts synced", healthy, total))
    def why7():
        return "line %r; indexed %r; (accounts, healthy) %r; surface %r" % (line(), indexed(), counts(), s.ctl.uistate().get("surface"))
    first10 = sum(1 for a in IDS[:10] if a != FAILING_ACCOUNT)
    # mail is there while history is still loading (not yet every account indexed); a populated list alone is true of any fixture
    s.expect("inbox-works-now", lambda: (len(s.ctl.rows("mac-main.list")) > 0, indexed() < ACCOUNT_COUNT - 1), (True, True))
    expect_why(s, "status-counts-indexed", lambda: (indexed(), line_ok()), (first10, True), why7)
    s.raw("fixture", "indexing", "all")
    expect_why(s, "indexed-reaches-thirty", lambda: (indexed(), line_ok()), (ACCOUNT_COUNT - 1, True), why7)

    s = j.step("J4.8", "Fix a broken account, mute two")
    s.arrive(("click", "mac-main.status"), S)
    s.expect("broken-named", lambda: s.ctl.rows(S + ".failing")[0]["name"], ACCOUNT[FAILING_ACCOUNT]["name"])
    s.click(S + ".%s.sign-in-again" % FAILING_ACCOUNT)
    s.click(S + ".%s.notifications" % MUTED[0])
    s.click(S + ".%s.notifications" % MUTED[1])
    s.expect("account-healthy", lambda: s.ctl.core("sync_state/%s/healthy" % FAILING_ACCOUNT), True)
    s.expect("two-muted", lambda: sorted(s.ctl.core("notification_setting/off")), sorted(MUTED))
    s.expect("muted-rows-say-so", lambda: [r["note"] for r in s.ctl.rows(S + ".accounts") if r["id"] in MUTED], [NO_NOTIFICATIONS] * 2)

    s = j.step("J4.9", "QR to move accounts to the iPhone")
    s.arrive(("click", S + ".move-to-iphone"), W)
    qr = lambda: s.ctl.rows(W + ".qr")
    s.expect("qr-shown", lambda: (len(qr()), qr()[0]["payload"].startswith("whoosh-pair:")), (1, True))
    s.expect("qr-expires", lambda: s.ctl.core("device_link/expires_in_s"), const("QR_EXPIRY_S"))
    s.expect("token-only", lambda: sorted(s.ctl.core("device_link/payload_keys")), ["token"])
    # positive control for the J4.10 absence check: before the scan the link token exists
    first = {}
    s.expect("token-live-before-scan", lambda: first.setdefault("id", s.ctl.core("device_link/id")) is not None and
             first.setdefault("qr", qr()[0]["payload"]) is not None, True)
    s.raw("clock", "advance", const("QR_EXPIRY_S") + 1)
    s.expect("expired-qr-replaced", lambda: qr()[0]["payload"] not in (None, first.get("qr")), True)
    s = j.step("J4.10", "iPhone scans the code")
    # the welcome's own flow (docs/JOURNEYS.md J9; a test copy lands the accounts at once): scan and the typed code reach the arrival step, Continue and Not now leave for the inbox
    pclick(s, "iphone-welcome.scan")     # the control id is `iphone-welcome.scan` (WelcomeControls.scan), not the surface id `iphone-welcome-accounts` + ".scan": that id is no control, the click was skipped and the step stayed `first` (main phone4)
    s._act(lambda: phone(s).type("fixture:valid"))     # the code as if the camera read it: a test copy takes `fixture:` words (WelcomeMacSide.testWord)
    s.expect("arrives-by-control", lambda: (phone(s).rows("iphone-welcome.step")[0]["step"], len(phone(s).rows("iphone-welcome.accounts"))), ("arriving", ACCOUNT_COUNT))
    pclick(s, "iphone-welcome.continue")
    pclick(s, "iphone-welcome.not-now")
    s.expect("welcome-leaves-for-the-inbox", lambda: phone(s).uistate().get("surface"), II)
    s.raw("fixture", "device-link", "scan")        # the phone's scan reaches THIS copy only through the verb (two processes): the Mac's token is used
    s.expect("same-thirty", lambda: len(phone(s).rows(II + ".accounts")), ACCOUNT_COUNT)
    s.expect("same-names-colours-order", lambda: [(r["name"], r["colour"]) for r in phone(s).rows(II + ".accounts")],
             [(a["name"], a["colour"]) for a in FX["accounts"]])
    s.expect("no-password-asked", lambda: phone(s).core("iphone/password_prompts"), 0)
    def token_now():
        """The Mac's live link token, None once it is used: the state path answers NO VALUE for a used token (`DeviceLinkModel.token` is nil: main phone4, after
        `fixture device-link scan`, the read said "no core value at device_link/token" and the case was not judged). The path exists: `token-live-before-scan` read a value
        from it earlier in this journey, so that answer is the cleared token, not a missing path."""
        phone(s)
        try:
            return s.ctl.core("device_link/token")
        except Unreadable as e:
            if "no core value" in str(e):
                return None
            raise
    s.expect("link-token-cleared", token_now, None)

    s = j.step("J4.11", "iPhone sync state")
    # The settings are reached the way the owner reaches them: the inbox's Back to the mailboxes, the Settings row there, then the Sync page (`iphone-inbox.settings-sync` is no
    # control of the app: the click was refused, the phone stayed on the inbox, main phone6 `expected 'iphone-settings', got 'iphone-inbox'`).
    pclick(s, "iphone-inbox.back")
    parrive(s, "iphone-mailboxes.settings", IS)
    pclick(s, "iphone-settings.row.sync")
    s.expect("names-the-mac", lambda: phone(s).rows(IS + ".sync")[0]["device"], "Mac")
    s.expect("recent-time", lambda: phone(s).rows(IS + ".sync")[0]["age_s"] < 300, True)

    s = j.step("J4.12", "Account icon and colour")
    open_a4_thread(s)                                # the reader must show a thread of A4 before the pick, so its meta line can be read after it
    s.click(SW + ".open")
    s.click(SW + ".pane-row.accounts")
    s.click("%s.%s" % (S, A4["id"]))                 # the account's own page holds the icon control
    s.arrive(("click", S + ".%s.icon" % A4["id"]), S)
    s.click(S + ".icon-library." + PICKED_ICON)      # not A4's fixture icon, so a dead picker cannot pass
    s.click(S + ".colour.%s" % A1["colour"].lstrip("#"))
    s.click(S + ".picker.done")
    s.click("dock.whoosh")                           # only brings the main window forward: no read below waits for the list (RowFacts.swift keys its cache by AccountMarkRevision)
    s.expect("icon-saved", lambda: s.ctl.core("account_icon/%s" % A4["id"]), PICKED_ICON)
    s.expect("colour-saved", lambda: s.ctl.core("account/%s/colour" % A4["id"]), A1["colour"])
    expect_why(s, "icon-on-list-row", lambda: [r["account_icon"] for r in a4_list_rows(s)][:1], [PICKED_ICON],
               lambda: "list rows of %s: %r; core account_icon %r; the list's first rows %r; surface %r" % (
                   A4["id"], [(r["id"], r["account_icon"]) for r in a4_list_rows(s)], s.ctl.core("account_icon/%s" % A4["id"]),
                   [(r["id"], r["account"], r["account_icon"]) for r in s.ctl.rows("mac-main.list")[:3]], s.ctl.uistate().get("surface")))
    s.expect("icon-in-sidebar", lambda: [r["icon"] for r in s.ctl.rows("mac-main.sidebar.accounts") if r["id"] == A4["id"]], [PICKED_ICON])
    expect_why(s, "icon-on-reader-meta", lambda: [r["account_icon"] for r in s.ctl.rows("mac-main.reader.header")][:1], [PICKED_ICON],
               lambda: "reader header rows %r; focus %r" % ([(r.get("account"), r.get("account_icon")) for r in s.ctl.rows("mac-main.reader.header")], s.ctl.uistate().get("focus_message")))
    # the colour, read the same way: the three places, back to back, nothing waited for
    want = A1["colour"].lower()
    expect_why(s, "colour-on-list-row", lambda: [r["account_colour"].lower() for r in a4_list_rows(s)][:1], [want],
               lambda: "list rows of %s: %r; core colour %r" % (A4["id"], [(r["id"], r["account_colour"]) for r in a4_list_rows(s)], s.ctl.core("account/%s/colour" % A4["id"])))
    s.expect("colour-in-sidebar", lambda: [r["colour"].lower() for r in s.ctl.rows("mac-main.sidebar.accounts") if r["id"] == A4["id"]], [want])
    expect_why(s, "colour-on-reader-meta", lambda: [r["account_colour"].lower() for r in s.ctl.rows("mac-main.reader.header")][:1], [want],
               lambda: "reader header rows %r; focus %r" % ([(r.get("account"), r.get("account_colour")) for r in s.ctl.rows("mac-main.reader.header")], s.ctl.uistate().get("focus_message")))

# MUTANTS
# J4.1/theme-applied | the app ignores the theme and stays light
# J4.1/arrives-by-control | first launch skips the Welcome screen
# J4.3/arrives-by-control | Add another leaves the Welcome screen
# J4.3/address-cleared | Add another does nothing
# J4.4/arrives-by-control | Add another leaves the Welcome screen
# J4.4/address-cleared | Add another does nothing
# J4.5/arrives-by-control | Add another leaves the Welcome screen
# J4.5/address-cleared | Add another does nothing
# J4.1/detection-order-nothing | detection steps are shown in another order
# J4.1/detection-order-srv | detection steps are shown in another order
# J4.1/detection-words-nothing | the progress line names no step in words
# J4.1/detection-words-srv | the progress line names no step in words
# J4.1/manual-fields-editable | Manual setup fields are read-only
# J4.1/manual-label | the fallback is not called Manual setup
# J4.1/manual-offered | no Manual setup control on the summary
# J4.1/no-tenant-line-srv | a plain IMAP domain shows a Microsoft 365 organisation line
# J4.1/start-over-clears | Start over does not clear the fields
# J4.1/summary-eight-fields-nothing | the summary drops a field
# J4.1/summary-eight-fields-srv | the summary drops a field
# J4.1/summary-mode-nothing | a failed detection opens as a detected summary, not Manual setup
# J4.1/summary-mode-srv | a detected summary opens as Manual setup
# J4.1/two-fields-only | a third field is shown on the first screen
# J4.1/typed-control | typing does not reach the address field
# J4.2/account-rows | Add creates no account row
# J4.2/folders-mapped | folders ignore SPECIAL-USE
# J4.2/row-progress-next | the row stays on connecting
# J4.2/validation-lines | the validation lines show in the wrong order or words
# J4.2/validation-ops | SMTP is never checked
# J4.3/account-rows-tenant | Add creates no account row
# J4.3/blocked-tenant-named | the blocked tenant is not named
# J4.3/coming-soon | the sheet says Microsoft sign-in is coming soon again (a retired line comes back)
# J4.3/detection-order-tenant | detection steps are shown in another order
# J4.3/detection-words-tenant | the progress line names no step in words
# J4.3/folders-mapped-tenant | folders ignore SPECIAL-USE
# J4.3/manual-offered | no Manual setup control on the summary
# J4.3/not-added-blocked | a refused account is added anyway
# J4.3/refusal-words-blocked | the refusal line names no server or reason
# J4.3/reopens-manual-blocked | Manual setup fields are read-only
# J4.3/row-progress-next-tenant | the row stays on connecting
# J4.3/row-progress-tenant | the account row shows no progress
# J4.3/summary-eight-fields-tenant | the summary drops a field
# J4.3/summary-mode-tenant | a detected summary opens as Manual setup
# J4.3/tenant-named | the tenant is not named
# J4.3/validation-lines-tenant | the validation lines show in the wrong order or words
# J4.3/validation-ops-tenant | SMTP is never checked
# J4.4/account-rows-icloud | Add creates no account row
# J4.4/app-password-steps-link | the app-password steps have no link
# J4.4/apple-link-in-refusal | the refusal has no Apple link
# J4.4/detection-order-icloud | detection steps are shown in another order
# J4.4/detection-words-icloud | the progress line names no step in words
# J4.4/folders-mapped-icloud | folders ignore SPECIAL-USE
# J4.4/not-added-wrong-password | a refused account is added anyway
# J4.4/refusal-words-wrong-password | the refusal line names no server or reason
# J4.4/reopens-manual-wrong-password | Manual setup fields are read-only
# J4.4/row-progress-icloud | the account row shows no progress
# J4.4/row-progress-next-icloud | the row stays on connecting
# J4.4/summary-eight-fields-icloud | the summary drops a field
# J4.4/summary-mode-icloud | a detected summary opens as Manual setup
# J4.4/validation-lines-icloud | the validation lines show in the wrong order or words
# J4.4/validation-ops-icloud | SMTP is never checked
# J4.5/account-rows-fastmail | Add creates no account row
# J4.5/account-rows-gmail | Add creates no account row
# J4.5/account-rows-guess | Add creates no account row
# J4.5/detection-order-fastmail | detection steps are shown in another order
# J4.5/detection-order-gmail | detection steps are shown in another order
# J4.5/detection-order-guess | detection steps are shown in another order
# J4.5/detection-words-fastmail | the progress line names no step in words
# J4.5/detection-words-gmail | the progress line names no step in words
# J4.5/detection-words-guess | the progress line names no step in words
# J4.5/folders-mapped-fastmail | folders ignore SPECIAL-USE
# J4.5/folders-mapped-gmail | folders ignore SPECIAL-USE
# J4.5/folders-mapped-guess | folders ignore SPECIAL-USE
# J4.5/gmail-app-password-route | the password route is hidden beside coming soon
# J4.5/gmail-coming-soon | the sheet says Google sign-in is coming soon again (a retired line comes back)
# J4.5/row-progress-fastmail | the account row shows no progress
# J4.5/row-progress-gmail | the account row shows no progress
# J4.5/row-progress-guess | the account row shows no progress
# J4.5/row-progress-next-fastmail | the row stays on connecting
# J4.5/row-progress-next-gmail | the row stays on connecting
# J4.5/row-progress-next-guess | the row stays on connecting
# J4.5/summary-eight-fields-fastmail | the summary drops a field
# J4.5/summary-eight-fields-gmail | the summary drops a field
# J4.5/summary-eight-fields-guess | the summary drops a field
# J4.5/summary-mode-fastmail | a detected summary opens as Manual setup
# J4.5/summary-mode-gmail | a detected summary opens as Manual setup
# J4.5/summary-mode-guess | a detected summary opens as Manual setup
# J4.5/validation-lines-fastmail | the validation lines show in the wrong order or words
# J4.5/validation-lines-gmail | the validation lines show in the wrong order or words
# J4.5/validation-lines-guess | the validation lines show in the wrong order or words
# J4.5/validation-ops-fastmail | SMTP is never checked
# J4.5/validation-ops-gmail | SMTP is never checked
# J4.5/validation-ops-guess | SMTP is never checked
# J4.2/credential-in-keychain | the password is not stored in the Keychain
# J4.2/account-exists-control | the account is never created in the core
# J4.2/no-password-in-state | the password is written to the core state
# J4.2/row-progress | the account row shows no progress
# J4.6/import-opens-its-sheet | Import mail does not open its sheet in the Settings window
# J4.6/arrives-by-control | Move accounts to iPhone does not open the Welcome window
# J4.6/list-starts-empty | the Welcome list is pre-filled before the paste
# J4.6/order-on-screen | the pasted addresses are listed in another order
# J4.6/thirty-rows | rows are capped below 30
# J4.6/names | names are lost
# J4.6/colours | colours reset to one
# J4.6/signature-per-address | signatures are saved for one address only
# J4.6/shortcut-preset-chosen | no preset is stored
# J4.6/first-screen-before-history | the inbox waits for the whole history
# J4.6/failing-row-says-why | the failing row shows no reason
# J4.6/other-29-carry-on | one failing account stops the other 29
# J4.7/starts-on-welcome | the Welcome screen is gone before Open inbox is pressed
# J4.7/arrives-by-control | Open inbox does nothing
# J4.7/inbox-works-now | the inbox is empty until the history is done
# J4.7/status-counts-indexed | the core's indexed count or the status line's synced count is wrong while history loads
# J4.7/indexed-reaches-thirty | the indexing stalls before every account has its history
# J4.8/arrives-by-control | the status line is not clickable
# J4.8/broken-named | the broken account is not named
# J4.8/account-healthy | sign in again does not clear the error
# J4.8/two-muted | notification settings are not saved
# J4.8/muted-rows-say-so | muted rows look the same
# J4.9/arrives-by-control | the move control is missing
# J4.9/qr-shown | no QR
# J4.9/qr-expires | the QR never expires
# J4.9/token-only | the QR carries a password
# J4.9/token-live-before-scan | no pairing token is ever created
# J4.9/expired-qr-replaced | an expired QR stays on screen
# J4.10/arrives-by-control | scan opens nothing
# J4.10/welcome-leaves-for-the-inbox | Continue and Not now leave the welcome open, the iPhone never reaches its inbox
# J4.10/same-thirty | accounts are dropped on the iPhone
# J4.10/same-names-colours-order | names, colours or order differ on the iPhone
# J4.10/no-password-asked | the iPhone asks for passwords
# J4.10/link-token-cleared | the token stays valid after use (the engine does not clear it on the first scan, or `fixture device-link scan` does nothing)
# J4.11/arrives-by-control | the sync row is unreachable
# J4.11/names-the-mac | the device is not named
# J4.11/recent-time | the time is stale or missing
# J4.12/arrives-by-control | the icon control is missing
# J4.12/icon-saved | the icon is not saved
# J4.12/colour-saved | the colour is not saved
# J4.12/icon-on-list-row | the list row ignores the icon
# J4.12/icon-in-sidebar | the sidebar ignores the icon
# J4.12/icon-on-reader-meta | the reader's meta line ignores the icon
# J4.12/colour-on-list-row | the list row ignores the colour
# J4.12/colour-in-sidebar | the sidebar ignores the colour
# J4.12/colour-on-reader-meta | the reader's meta line ignores the colour


# ---------------------------------------------------------------------------------------------------------------- self-test kit
# python3 journeys/J4.py --selftest (and J5.py, J6.py, which import this kit): the whole journey against a stand-in app served on a private Unix socket in a
# temporary folder (never the real app, never a window, never the network). Four runs per journey, the house form of journeys/J10.py:
#   ideal   the stand-in answers as the contract says: every check green, except the ones the build cannot stage (declared, each not judged with its reason)
#   inert   the stand-in serves nothing: no check may be green
#   mutant  one run per check: the stand-in answers that one check's own reads wrongly (a value bent, a row dropped, an absent row made present): that check must go red
#   runner  the same journey through lib.harness.run_journey: verdict, and a mutant row both ways (NO MUTANT / STRAY MUTANT)
# A mutant is a read-level fault (the answer is wrong while the check reads it); it proves the check can fail on that read. What a click does is the ideal run's job.
NOT_SERVED = object()


class Gone(Exception):
    """A read the stand-in does not serve: the verb's own refusal words."""


def bend(v):
    """A wrong answer for a right one, whatever its shape: flags flip, numbers move, text changes, a list loses its first row, an empty one gains a stranger."""
    if isinstance(v, bool):
        return not v
    if isinstance(v, (int, float)):
        return v + 7
    if isinstance(v, str):
        return v + "~"
    if v is None:
        return "x"
    if isinstance(v, list):
        return [bend(x) for x in v[1:]] or [{"x": 1}]
    if isinstance(v, dict):
        return {k: bend(x) for k, x in v.items()}
    return "x"


class Sim:
    """The stand-in app's wire: rows, core, uistate, winframes, the actions and the fixture/clock verbs. A subclass fills `rows`, `core`, `on` and `state`.
    rows[ax] is a list or a function returning one (None = the region is not served: "no rows at"); core[path] a value or a function (a missing path = "no core value");
    on[("click", ax)], on[("key", chord)], on[("type",)], on[("fixture", verb)], on[("clock", verb)] are functions (sim, args) -> reply fields or None."""
    FAULTS = {}      # label -> function(reply) -> reply: the wrong answer a check needs where bending the right one is not enough (a value read only for "is it there")

    def __init__(self, mutant=None, inert=False):
        self.mutant, self.inert, self.current, self.now = mutant, inert, None, 0.0
        self.rows, self.core, self.on, self.frames = {}, {}, {}, []
        self.state = {"surface": "mac-main", "first_responder": "list", "focus_message": "msg-01", "tab": "primary", "overlay": None, "theme": "light", "hidden": {"refused": []}}
        self.setup()

    def setup(self):
        pass

    def tick(self, secs):
        self.now += secs

    def rows_for(self, ax):
        v = self.rows.get(ax)
        return v(self) if callable(v) else v

    def core_for(self, path):
        if path not in self.core:
            return NOT_SERVED
        v = self.core[path]
        return v(self) if callable(v) else v

    def handle(self, verb, args):
        if verb == "fixture" and args[:1] == ["theme"] and len(args) == 2 and not self.inert:      # served for every journey's stand-in: the theme the journey asked for
            self.state["theme"] = args[1]
            return {"ok": True}
        try:
            out = self.answer(verb, args)
        except (KeyError, IndexError, TypeError, ValueError) as e:      # a gap in the stand-in is an answer, never a dead socket
            return {"ok": False, "error": "stand-in fault %s: %s" % (type(e).__name__, e)}
        except Gone as e:
            if self.mutant and self.mutant == self.current and verb in ("rows", "core"):
                return {"ok": True, "rows": [{"x": 1}]} if verb == "rows" else {"ok": True, "value": "x"}
            return {"ok": False, "error": str(e)}
        if self.mutant and self.mutant == self.current and verb in ("rows", "core", "uistate", "winframes"):
            if self.mutant in self.FAULTS:
                return self.FAULTS[self.mutant](out)
            out = {k: (bend(v) if k in ("rows", "value", "state", "frames") else v) for k, v in out.items()}
        return out

    def answer(self, verb, args):
        if self.inert:
            raise Gone("%s %s: nothing is served" % (verb, tuple(args)))
        if verb == "rows":
            v = self.rows_for(args[0])
            if v is None:
                raise Gone("rows %s: no rows at %s" % (tuple(args), args[0]))
            return {"ok": True, "rows": v}
        if verb == "core":
            v = self.core_for(args[1])
            if v is NOT_SERVED:
                raise Gone("core %s: no core value at %s" % (tuple(args), args[1]))
            return {"ok": True, "value": v}
        if verb == "uistate":
            return {"ok": True, "state": dict(self.state)}
        if verb == "winframes":
            return {"ok": True, "frames": list(self.frames)}
        key = ("click", args[0]) if verb == "click" else ("key", args[0]) if verb == "key" else (verb,) if verb == "type" else (verb, args[0]) if args else (verb,)
        fn = self.on.get(key)
        if fn is None:
            if verb == "click":
                raise Gone("click %s: nothing to click at %s" % (tuple(args), args[0]))
            if verb in ("fixture",):
                raise Gone("fixture %s is not served" % args[0])
            if verb in ("type", "key", "clock", "capture"):
                return {"ok": True}
            raise Gone("%s: unknown verb" % verb)
        out = fn(self, args[1:] if verb in ("fixture", "clock") else args)
        return dict({"ok": True}, **(out or {}))


def serve_sim(sim, fn, mod_id):
    """Runs fn(journey) over a socket served by `sim`; returns the journey's checks. Closed on every path. `time.sleep` moves the stand-in's clock and waits for nothing."""
    import shutil
    import socketserver
    import tempfile
    import threading
    from lib.harness import Journey, Step
    from lib.ctl import Ctl

    class H(socketserver.StreamRequestHandler):
        def handle(self):
            for line in self.rfile:
                q = json.loads(line.decode())
                self.wfile.write((json.dumps(sim.handle(q["verb"], q["args"])) + "\n").encode())
    d = tempfile.mkdtemp(prefix="journey-standin-")
    srv = socketserver.ThreadingUnixStreamServer(os.path.join(d, "whoosh.sock"), H)
    srv.daemon_threads = True
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    ctl, old_rec, old_sleep = None, Step._record, time.sleep

    def rec(self, name, f):
        sim.current = "%s/%s" % (self.id, name)
        try:
            return old_rec(self, name, f)
        finally:
            sim.current = None
    try:
        ctl = Ctl(d).connect()
        Step._record = rec
        time.sleep = sim.tick
        j = Journey(mod_id, ctl)
        fn(j)
        return j.checks()
    finally:
        Step._record, time.sleep = old_rec, old_sleep
        if ctl is not None:
            ctl.close()
        srv.shutdown()
        srv.server_close()
        shutil.rmtree(d, ignore_errors=True)


def run_selftest(mod, path, make, unjudged_ideal, before=None, after=None):
    """The four runs for one journey module. `make(mutant, inert)` builds the stand-in; `unjudged_ideal` {label: why} the ideal run may not judge (the build cannot stage it);
    `before()`/`after()` set up and put back what a run touches outside the socket. Prints the proof lines; returns the exit code."""
    import shutil
    import socketserver
    import tempfile
    import threading
    from lib.harness import run_journey, declared_mutants
    declared = set(declared_mutants(path))
    bad = 0

    def run(**kw):
        if before:
            before()
        try:
            return serve_sim(make(**kw), mod.drive, mod.ID)
        finally:
            if after:
                after()
    ideal = run(mutant=None, inert=False)
    labels = [c.label for c in ideal]
    green = sum(c.state == "pass" for c in ideal)
    print("ideal stand-in: %d checks, %d green, %d red, %d not judged" % (len(ideal), green, sum(c.state == "fail" for c in ideal), sum(c.state == "unjudged" for c in ideal)))
    for c in ideal:
        if c.state != "pass" and not (c.state == "unjudged" and c.label in unjudged_ideal):
            print("  NOT GREEN on the ideal stand-in: %s %s: %s" % (c.label, c.state, c.detail))
            bad += 1
    for lab in sorted(unjudged_ideal):
        got = [c for c in ideal if c.label == lab]
        if not got or got[0].state != "unjudged":
            print("  EXPECTED NOT JUDGED but %s: %s" % (got[0].state if got else "missing", lab))
            bad += 1
    print("  not judged on the ideal stand-in, each for its reason: %d" % len(unjudged_ideal))
    inert = run(mutant=None, inert=True)
    up = [c.label for c in inert if c.state == "pass"]
    print("inert stand-in (serves nothing): %d checks, %d red, %d not judged, %d green%s" % (
        len(inert), sum(c.state == "fail" for c in inert), sum(c.state == "unjudged" for c in inert), len(up), (": " + ", ".join(up)) if up else ""))
    bad += len(up)
    for lab in labels:
        if lab not in declared:
            print("NO MUTANT %s" % lab)
            bad += 1
    for lab in sorted(declared - set(labels)):
        print("STRAY MUTANT %s" % lab)
        bad += 1
    # the runner path: the same drive through run_journey over $WHOOSH_SOCKET_DIR on the ideal stand-in
    sim = make(mutant=None, inert=False)

    class H(socketserver.StreamRequestHandler):
        def handle(self):
            for line in self.rfile:
                q = json.loads(line.decode())
                self.wfile.write((json.dumps(sim.handle(q["verb"], q["args"])) + "\n").encode())
    d = tempfile.mkdtemp(prefix="journey-runner-")
    srv = socketserver.ThreadingUnixStreamServer(os.path.join(d, "whoosh.sock"), H)
    srv.daemon_threads = True
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    env, old_sleep = os.environ.get("WHOOSH_SOCKET_DIR"), time.sleep
    os.environ["WHOOSH_SOCKET_DIR"] = d
    os.environ.setdefault("WHOOSH_RESULTS_DIR", tempfile.mkdtemp(prefix="journey-results-"))
    results_dir = os.environ["WHOOSH_RESULTS_DIR"]
    if before:
        before()
    try:
        time.sleep = sim.tick
        r = run_journey(mod, path)
    finally:
        time.sleep = old_sleep
        if after:
            after()
        if env is None:
            os.environ.pop("WHOOSH_SOCKET_DIR", None)
        else:
            os.environ["WHOOSH_SOCKET_DIR"] = env
        srv.shutdown()
        srv.server_close()
        shutil.rmtree(d, ignore_errors=True)
        shutil.rmtree(results_dir, ignore_errors=True)
        os.environ.pop("WHOOSH_RESULTS_DIR", None)
    print("through run_journey on the ideal stand-in: %s, %d steps, %d checks, %d failed, %d not judged, no mutant row for %s, stray rows %s"
          % (r["verdict"], r["steps"], r["checks"], r["failed"], r["unjudged"], r["no_mutant"], r["stray_mutant"]))
    bad += r["failed"] != 0 or r["checks"] != len(labels) or bool(r["no_mutant"]) or bool(r["stray_mutant"]) or r["unjudged"] != len(unjudged_ideal)
    red = skipped = 0
    ideal_bad = {c.label for c in ideal if c.state != "pass"}
    for lab in labels:
        if lab in unjudged_ideal:
            print("mutant %-52s -> not run (not judged on the ideal stand-in: %s)" % (lab, unjudged_ideal[lab]))
            skipped += 1
            continue
        got = {c.label: c for c in run(mutant=lab, inert=False)}
        st = got[lab].state
        also = sorted(k for k, c in got.items() if c.state != "pass" and k != lab and k not in ideal_bad)
        print("mutant %-52s -> %s%s" % (lab, "RED" if st == "fail" else "NOT RED (%s)" % st, ("  also: " + ", ".join(also[:6]) + (" ..." if len(also) > 6 else "")) if also else ""))
        red += st == "fail"
        bad += st != "fail"
    print("selftest: %d checks, %d mutants red, %d not run (not judged on the ideal), %d declared rows, %d not red or missing" % (len(labels), red, skipped, len(declared), bad))
    return 1 if bad else 0


class App4(Sim):
    """J4's stand-in: the Add account sheet (detection, summary, validation, refusal), the Welcome list, the sync line, the Settings pages, as the journey's contract reads them.
    Where this build cannot be staged (no tenant name, no folder map, no per-row name, colour or signature door, no iPhone rig) it serves what the build serves."""
    FAULTS = {"J4.6/first-screen-before-history": lambda out: {"ok": True, "value": None},
              "J4.7/inbox-works-now": lambda out: dict(out, value=True) if "value" in out else dict(out, rows=[])}

    def setup(self):
        self.typed, self.field, self.stage, self.dom, self.manual, self.details = {"address": "", "password": ""}, None, None, None, False, False
        self.added, self.lines, self.w_rows, self.w_text, self.indexing = [], [], [], "", None
        self.signed, self.muted, self.qr, self.qr_open, self.import_open = False, [], 0, False, False
        self.icon, self.colour = {}, {}

    def domain(self):
        return self.typed["address"].split("@")[-1]

    def trace(self):
        return FASTMAIL_TRACE if self.domain() == "fastmail.com" else DOM[self.domain()]["trace"]

    def cfg(self):
        return DOM[self.domain()]["config"] if self.dom else None

    def ids(self):
        return IDS + ["acc-%d" % (ACCOUNT_COUNT + 1 + i) for i in range(len(self.added))]

    def counts(self):
        ok = [i for i in self.ids() if not (i == FAILING_ACCOUNT and not self.signed)]
        return len(self.ids()), len(ok)

    def fetched(self, i):
        if i == FAILING_ACCOUNT:
            return self.signed and (self.indexing is None or IDS.index(i) < self.indexing)
        if i not in IDS:
            return False
        return self.indexing is None or IDS.index(i) < self.indexing

    def rows_for(self, ax):
        st, d, c = self.state["surface"], self.dom, self.dom and DOM[self.dom].get("config")
        mode_manual = self.stage in ("summary", "added", "error") and (self.manual)
        if ax.startswith(AA + "."):
            leaf = ax[len(AA) + 1:]
            if leaf == "fields":
                return [{"id": i} for i in STEPS["first_screen_fields"]]
            if leaf == "address":
                return [{"text": self.typed["address"]}]
            if leaf == "password":
                return [{"text": self.typed["password"], "visible": True}]
            if leaf == "accounts":
                return [dict(a) for a in self.added]
            if leaf == "detection.step":
                return [{"id": t["id"], "outcome": t["outcome"], "text": WORDS[t["id"]]} for t in self.trace()] if self.stage else None
            if not self.stage:
                return None
            if leaf == "summary.mode":
                return [{"mode": STEPS["modes"]["manual" if mode_manual else "detected"]}]
            if leaf.startswith("summary."):
                f = leaf.split(".", 1)[1]
                base = c if c and c.get("imapHost") else NOTHING_CFG
                return [{"value": base[f], "editable": bool(mode_manual)}] if f in FIELDS else None
            if leaf == "manual":
                return [{"open": bool(mode_manual), "visible": True, "text": MANUAL_HIDE if mode_manual else MANUAL}]
            if leaf == "tenant":
                return [{"text": TENANT_GENERIC}] if d and DOM[d].get("tenant") else None
            if leaf == "app-password.steps":
                return [{"link": "https://support.example/app-password"}] if d in ("icloud.com", "gmail.com") else None
            if leaf == "validation.line":
                return list(self.lines) if self.lines else None
            if leaf == "error":
                return self.error_rows() if self.stage == "error" else None
            return None
        if ax == S + ".failing":
            return [{"name": ACCOUNT[FAILING_ACCOUNT]["name"]}] if not self.signed else None
        if ax == S + ".accounts":
            return [{"id": i, "address": ACCOUNT[i]["address"], "name": ACCOUNT[i]["name"], "colour": self.colour.get(i, ACCOUNT[i]["colour"]), "signature": ACCOUNT[i]["signature"],
                     "note": NO_NOTIFICATIONS if i in self.muted else ""} for i in IDS]
        if ax == S + ".import.sheet":
            return [{"open": self.import_open}]
        if ax == W + ".summary":
            return [{"total": len(self.w_rows), "preset": "Superhuman"}]
        if ax == W + ".accounts":
            return [dict(r) for r in self.w_rows]
        if ax == W + ".qr":
            return [{"payload": "whoosh-pair:%d" % self.qr}] if self.qr_open else None
        if ax == "mac-main.sync.line":
            total, ok = self.counts()
            return [{"line": say("%@ of %lld accounts synced", ok, total) + " · " + say(" · just now").lstrip(" ·")}]
        if ax == "mac-main.list":
            return [{"id": "msg-%02d" % (n + 1), "account": i, "account_icon": self.icon.get(i, "star"), "account_colour": self.colour.get(i, ACCOUNT[i]["colour"])} for n, i in enumerate(IDS[:6])]
        if ax == "mac-main.sidebar.accounts":
            return [{"id": i, "icon": self.icon.get(i, "star"), "colour": self.colour.get(i, ACCOUNT[i]["colour"])} for i in IDS]
        if ax == "mac-main.reader.header":
            i = IDS[int(self.state["focus_message"].split("-")[1]) - 1]
            return [{"index": 0, "account": i, "account_icon": self.icon.get(i, "star"), "account_colour": self.colour.get(i, ACCOUNT[i]["colour"])}]
        return None

    def error_rows(self):
        me = self.domain() == "me.com"
        t = say("The password was refused.") if me else BASIC_AUTH_SENTENCE
        det = ("imap.mail.me.com refused the sign-in: wrong password." if me else "outlook.office365.com: this organisation blocks password sign-in for IMAP (Basic authentication is disabled.)")
        return [{"header": t, "text": t, "cause": "x", "link": "https://appleid.apple.com" if me else "", "details": det if self.details else ""}]

    def core_for(self, path):
        p = path.split("/")
        ids = self.ids()
        if p[0] == "account" and len(p) == 3 and p[1] in ids:
            if p[2] == "address":
                return next((a["address"] for a in self.added if a["id"] == p[1]), None)
            if p[2] == "password_stored":
                return True
            if p[2] == "colour":
                return self.colour.get(p[1], ACCOUNT[p[1]]["colour"])
            if p[2] == "folders":      # the role -> folder map the validation recorded (FixtureAddAccount.swift `configValue`), for an account added by the sheet
                a = next((a for a in self.added if a["id"] == p[1]), None)
                return DOM[a["address"].split("@")[1]]["folder_map"] if a else NOT_SERVED
            return NOT_SERVED
        if p[0] == "account_credential" and len(p) == 3 and p[1] in ids:
            return True
        if path == "account/order":
            return ids
        if p[0] == "sync_state" and len(p) == 3:
            return {"healthy": not (p[1] == FAILING_ACCOUNT and not self.signed), "history_fetched": self.fetched(p[1]), "first_screen_at": 1}.get(p[2], NOT_SERVED)
        if path == "notification_setting/off":
            return list(self.muted)
        if path == "device_link/expires_in_s":
            return const("QR_EXPIRY_S")
        if path == "device_link/payload_keys":
            return ["token"]
        if path == "device_link/id":
            return "link-%d" % self.qr
        if p[0] == "account_icon" and len(p) == 2:
            return self.icon.get(p[1], "star")
        return NOT_SERVED

    def answer(self, verb, args):
        if self.inert or verb in ("rows", "core", "uistate", "winframes"):
            return super().answer(verb, args)
        if verb == "type":
            if self.state["surface"] == W:
                self.w_text = args[0]
            elif self.field:
                self.typed[self.field] = args[0]
            return {"ok": True}
        if verb == "key" and args[0] in ("J", "K"):
            n = max(0, min(5, int(self.state.get("focus_index") or 0) + (1 if args[0] == "J" else -1)))
            self.state["focus_index"], self.state["focus_message"] = n, "msg-%02d" % (n + 1)
            return {"ok": True}
        if verb in ("key", "capture"):
            return {"ok": True}
        if verb == "clock":
            if args[0] == "advance" and args[1] >= const("QR_EXPIRY_S"):
                self.qr += 1
            elif args[0] == "advance":
                for a in self.added:
                    a["progress"] = STEPS["row_progress"][1]
            return {"ok": True}
        if verb == "fixture":
            if args[0] == "indexing":
                self.indexing = None if args[1] == "all" else int(args[1])
                return {"ok": True}
            raise Gone("fixture %s is not served" % args[0])
        return self.click(args[0])

    def go(self, surface):
        self.state["surface"] = surface

    def click(self, ax):
        n = len(self.added)
        if ax == "dock.whoosh":
            self.go("mac-main")
        elif ax == MENU + ".add":
            self.go(AA)
            self.stage, self.dom, self.manual, self.lines, self.details = None, None, False, [], False
        elif ax in (MENU + ".up", AA + ".cancel"):
            pass
        elif ax in (AA + ".address", AA + ".password"):
            self.field = ax.rsplit(".", 1)[1]
        elif ax == AA + ".continue":
            self.dom = self.domain()
            self.stage = "summary"
            self.manual = not DOM[self.dom]["config"].get("imapHost")
        elif ax == AA + ".start-over":
            self.typed, self.stage, self.dom, self.manual, self.lines, self.details = {"address": "", "password": ""}, None, None, False, [], False
        elif ax == AA + ".add-another":
            if self.stage == "added":
                self.typed, self.stage, self.dom, self.lines = {"address": "", "password": ""}, None, None, []
        elif ax == AA + ".add":
            if self.stage != "summary":
                raise Gone("click %s: nothing to click at %s" % ((ax,), ax))
            d = DOM[self.dom]
            if d["validates"]:
                c = d["config"]
                self.lines = [{"text": VALIDATION_LINES[0], "state": "ok", "details": ["Signed in to %s." % c["imapHost"], "Read %d folders." % len(d["folders"])]},
                              {"text": VALIDATION_LINES[1], "state": "ok", "details": ["%s answered." % c["smtpHost"], "Signed in to %s for sending." % c["smtpHost"]]}]
                self.added.append({"id": "acc-%d" % (ACCOUNT_COUNT + 1 + n), "address": self.typed["address"], "progress": STEPS["row_progress"][0]})
                self.stage = "added"
            else:
                self.stage, self.manual, self.details = "error", True, False
        elif ax == AA + ".error.details":
            self.details = True
        elif ax == SW + ".open":
            self.frames = [{"title": "Settings", "kind": "window"}]
        elif ax == SW + ".pane-row.accounts" or ax == "mac-main.status":
            self.go(S)
        elif ax == S + ".import":
            self.import_open = True
        elif ax == S + ".import.close":
            self.import_open = False
        elif ax == S + ".move-to-iphone":
            self.go(W)
            self.qr_open = True
        elif ax == W + ".reset":
            self.w_rows = []
        elif ax == W + ".paste-list":
            self.w_rows = [{"address": a, "name": a.split("@")[0], "colour": "#000000", "signature": "", "phase": "waiting", "status": ""} for a in self.w_text.split()]
            self.w_text = ""
        elif ax == W + ".connect-waiting":
            for r in self.w_rows:
                bad = r["address"] == ACCOUNT[FAILING_ACCOUNT]["address"]
                r["phase"], r["status"] = ("refused", "needs you to sign in again") if bad else ("healthy", "")
        elif ax == W + ".open-inbox":
            self.go("mac-main")
        elif ax == S + ".%s.sign-in-again" % FAILING_ACCOUNT:
            self.signed = True
        elif ax.startswith(S + ".") and ax.endswith(".notifications"):
            self.muted.append(ax.split(".")[-2])
        elif ax == S + ".icon-library." + PICKED_ICON:
            self.icon[A4["id"]] = PICKED_ICON
        elif ax.startswith(S + ".colour."):
            self.colour[A4["id"]] = "#" + ax.rsplit(".", 1)[1]
        elif ax.startswith(S + ".") or ax == AA + ".manual":
            pass
        else:
            raise Gone("click %s: nothing to click at %s" % ((ax,), ax))
        return {"ok": True}


NOT_JUDGED_4 = {
    "J4.10/arrives-by-control": "no iPhone rig", "J4.10/welcome-leaves-for-the-inbox": "no iPhone rig", "J4.10/same-thirty": "no iPhone rig", "J4.10/same-names-colours-order": "no iPhone rig", "J4.10/no-password-asked": "no iPhone rig",
    "J4.10/link-token-cleared": "no iPhone rig", "J4.11/arrives-by-control": "no iPhone rig", "J4.11/names-the-mac": "no iPhone rig", "J4.11/recent-time": "no iPhone rig"}


def selftest():
    saved = os.environ.pop("WHOOSH_IPHONE_SOCKET_DIR", None)
    try:
        return run_selftest(sys.modules[__name__], os.path.abspath(__file__), lambda mutant, inert: App4(mutant, inert), NOT_JUDGED_4)
    finally:
        if saved is not None:
            os.environ["WHOOSH_IPHONE_SOCKET_DIR"] = saved


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else 0)
