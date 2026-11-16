#!/usr/bin/env python3
"""settingscalendars-check: J10.2 (calendars as a source), the Settings > Agendas pane and the read-only macOS calendars.

    python3 checks/settingscalendars-check.py              drive the running rig copy ($WHOOSH_SOCKET_DIR); exit 1 when F>0 or U>0
    python3 checks/settingscalendars-check.py --selftest   every check against a stand-in app, clean, then one mutant per check

Two kinds of case. DRIVEN cases (the pane, one per control and per access state) read rows and click doors over the control socket;
the access state is set with `fixture calendar-access <not-asked|allowed|refused|will-refuse>` (or, until the lead serves that verb,
the door `mac-settings-calendars.fixture.<word>`). STATIC cases read Sources/Features/SettingsCalendarsMac/*.swift and the lane note
for what a rig cannot show: that nothing writes an event, that macOS is asked only by the button, that EventKit is never waited on,
that every sentence is listed with its French text. Expected sentences are literals below, taken from the lane note; the static
case "words-are-the-words-in-the-source" ties each literal to the Swift source, so the stand-in cannot drift from the app.

The stand-in (Fake) speaks the served shapes and is used ONLY by --selftest. Nothing here asks macOS for anything.
"""
import json
import os
import re
import shutil
import socketserver
import sys
import tempfile
import threading

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable  # noqa: E402

SRC_DIR = os.path.join(ROOT, "Sources", "Features", "SettingsCalendarsMac")
NOTE = os.path.join(ROOT, "docs", "lanes", "settingscalendars-mac.md")
CAPDIR = os.path.join(ROOT, "_captures", "settingscalendars-mac")
P = "mac-settings-calendars"
PANE_ROW = "mac-settings-window.pane-row.agendas"
PLACE = "mac-settings-window.place"

# The sentences of the pane, read from the catalogue (Sources/Resources/Localizable.xcstrings) in the language the app speaks: the catalogue's keys are the
# English source sentences, the check never compares with an English literal of its own. The language is read from the app (`speak`, `ensure_language`).
CATALOGUE = json.load(open(os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8"))["strings"]
CODES = sorted({c for v in CATALOGUE.values() for c in v.get("localizations", {})})
GEN_SETTINGS = "mac-settings-general.settings"
LANG_DOOR = "mac-settings-general.appearance.language.%s"
PINNED = {"was": None}      # the stored choice before this run pinned a language through the General pane's own door (None: it pinned nothing)


def word(key, code):
    """The catalogue's sentence for `key` in `code`; a key with no entry in that language answers the key itself (the source language), as the app does."""
    return CATALOGUE[key]["localizations"].get(code, {}).get("stringUnit", {}).get("value", key)


def build(code):
    w = lambda k: word(k, code)
    return {
        "access": w("Calendar access"),
        "chip": {"not-asked": w("Not asked yet"), "allowed": w("Allowed"), "refused": w("Refused")},
        "line": {"not-asked": w("Whoosh has not asked yet. It reads your events and never writes one."),
                 "allowed": w("Whoosh reads your events on this Mac. It never writes one."),
                 "refused": w("macOS refused access. Allow it in System Settings.")},
        "line_no_prompt": w("macOS did not ask. Allow Calendars for Whoosh in System Settings."),
        "line_asking": w("Answer the macOS window."),
        "steps": [w("Open Privacy and Security in System Settings"), w("Choose Calendars"), w("Switch Whoosh on")],
        "list_title": w("Calendars"), "read_only": w("Read only"),
        "footnote": w("Use a calendar as a source for replies in Settings › Replies › Sources."),
    }


W = build("en")      # replaced in place by `speak` once the app's language is read
EN = build("en")


def speak(code):
    W.clear()
    W.update(build(code))
CALS = [("cal-work", "Travail", "assem@nexus-example.com", "#007AFF"), ("cal-personal", "Personnel", "assem@icloud.com", "#34C759")]
EVENTS = [("ev-1", "cal-work"), ("ev-2", "cal-work"), ("ev-3", "cal-work"), ("ev-4", "cal-personal")]
# the keys the Swift source must hold (static cases: the source is English, whatever language the rig speaks)
SOURCE_WORDS = ["Untitled calendar", "Open Calendars", EN["access"], "Allow access", EN["chip"]["not-asked"], EN["chip"]["allowed"], EN["chip"]["refused"], *EN["line"].values(),
                "Answer the macOS window.", "macOS did not ask. Allow Calendars for Whoosh in System Settings.", "To allow access", "Open System Settings", EN["list_title"], EN["read_only"], "Reading calendars",
                "No calendar on this Mac", "Add one in the Calendar app and it appears here.", "On this Mac", EN["footnote"], *EN["steps"]]


# ---------------------------------------------------------------- runner
class Refused(Exception):
    """An action the check performed was refused by the app: the check is red, not unjudged."""


class Suite:
    def __init__(self):
        self.items = []

    def check(self, label, mutant, desc):
        def deco(fn):
            self.items.append((label, fn, mutant, desc))
            return fn
        return deco


S = Suite()


def act(ctl, verb, *args):
    try:
        return ctl.send(verb, *args)
    except Unreadable as e:
        raise Refused("action refused: %s" % e)


def refused(ctl, verb, *args):
    """True when the app refuses the action (a door that is not there answers ok:false)."""
    try:
        ctl.send(verb, *args)
        return False
    except Unreadable:
        return True


def eq(got, want):
    return got == want, "expected %r, got %r" % (want, got)


def set_access(ctl, word):
    try:
        ctl.send("fixture", "calendar-access", word)
    except Unreadable as e:
        if "not served" not in str(e):
            raise Refused("fixture calendar-access %s refused: %s" % (word, e))
        act(ctl, "click", "%s.fixture.%s" % (P, word))


def ensure_language(ctl):
    """Read the language the app speaks (`rows mac-settings-general.settings`, field `language`) and speak it. A stored choice that is a catalogue code is used as it is.
    `system` hides the code the Mac resolves it to (main79: the rig spoke French and the pane's words were compared with English), so then the check chooses English
    through the General pane's own language door and remembers the choice it replaced; `drive` puts it back. Nothing is assumed about the Mac's language."""
    code = ctl.rows(GEN_SETTINGS)[0].get("language")
    if code not in CODES:
        act(ctl, "click", "mac-settings-window.pane-row.general")
        act(ctl, "click", LANG_DOOR % "en")
        PINNED["was"] = PINNED["was"] or code or "system"      # the first replaced choice is the one to put back
        code = "en"
    speak(code)


def restore_language(ctl):
    """Puts back the stored language choice `ensure_language` replaced (the General pane's own door), and says what it did."""
    was, PINNED["was"] = PINNED["was"], None
    if was is not None:
        act(ctl, "click", "mac-settings-window.pane-row.general")
        act(ctl, "click", LANG_DOOR % was)
    return was


def opened(ctl, word="not-asked"):
    """Every case starts from a reset app, on the pane, with the access in the state it needs."""
    try:
        ctl.send("fixture", "reset")
    except Unreadable:
        pass
    ensure_language(ctl)      # after the reset: a reset that changed the language is read, not assumed
    set_access(ctl, word)
    act(ctl, "click", PANE_ROW)


def access(ctl):
    return ctl.rows(P + ".access")[0]


def calendars(ctl):
    return ctl.rows(P + ".list")


def steps(ctl):
    return ctl.rows(P + ".steps")


def feed(ctl):
    return ctl.rows(P + ".feed")[0]


# ---------------------------------------------------------------- driven cases: one per control, one per access state
@S.check("J10.2c/arrives-by-the-pane-row", "no-pane-row", "the Agendas row of the Settings sidebar opens nothing")
def c_arrive(ctl, env):
    opened(ctl)
    return eq(ctl.rows(PLACE)[0]["pane"], "agendas")


@S.check("J10.2c/not-asked-shows-one-button-and-no-list", "not-asked-shows-list", "before access the pane already lists calendars")
def c_not_asked(ctl, env):
    opened(ctl, "not-asked")
    a = access(ctl)
    got = (a["state"], a["chip"], a["line"], a["ask"], a["open_settings"], calendars(ctl), steps(ctl), ctl.rows(P + ".list.state")[0]["state"])
    want = ("not-asked", W["chip"]["not-asked"], W["line"]["not-asked"], P + ".access.ask", None, [], [], "hidden")
    return eq(got, want)


@S.check("J10.2c/ask-door-asks-once", "ask-door-stays-when-allowed", "the ask door still answers once access is allowed")
def c_ask_once(ctl, env):
    opened(ctl, "not-asked")
    act(ctl, "click", P + ".access.ask")
    a = access(ctl)
    second = refused(ctl, "click", P + ".access.ask")
    return (a["state"], a["ask"], second) == ("allowed", None, True), "after the ask %r, the second ask refused: %r" % (a["state"], second)


@S.check("J10.2c/ask-refused-by-the-owner", "ask-always-allows", "an owner who refuses in the macOS window still ends as allowed")
def c_ask_refused(ctl, env):
    opened(ctl, "will-refuse")
    act(ctl, "click", P + ".access.ask")
    a = access(ctl)
    return (a["state"], a["chip"]) == ("refused", W["chip"]["refused"]), "state %r chip %r" % (a["state"], a["chip"])


@S.check("J10.2c/allowed-lists-name-and-account", "list-missing-account", "the account of a calendar is left out")
def c_allowed_list(ctl, env):
    opened(ctl, "allowed")
    a = access(ctl)
    rows = [(r["name"], r["account"]) for r in calendars(ctl)]
    ls = ctl.rows(P + ".list.state")[0]
    ok = (a["state"], a["chip"], a["line"], a["ask"]) == ("allowed", W["chip"]["allowed"], W["line"]["allowed"], None) \
        and rows == [(n, acc) for _i, n, acc, _c in CALS] and (ls["state"], ls["count"], ls["title"], ls["note"]) == ("list", 2, W["list_title"], W["read_only"])
    return ok, "access %r, rows %r, list state %r" % (a, rows, ls)


@S.check("J10.2c/calendar-rows-carry-their-colour", "colour-lost", "every calendar dot is the neutral grey")
def c_colour(ctl, env):
    opened(ctl, "allowed")
    return eq([r["colour"] for r in calendars(ctl)], [c for _i, _n, _a, c in CALS])


@S.check("J10.2c/refused-shows-the-three-steps-and-a-door", "steps-missing", "a refused access shows no steps")
def c_refused(ctl, env):
    opened(ctl, "refused")
    a = access(ctl)
    got = (a["state"], a["chip"], a["line"], a["open_settings"], [r["text"] for r in steps(ctl)], [r["n"] for r in steps(ctl)], calendars(ctl))
    want = ("refused", W["chip"]["refused"], W["line"]["refused"], P + ".access.open-settings", W["steps"], [1, 2, 3], [])
    return eq(got, want)


@S.check("J10.2c/steps-only-while-refused", "steps-always", "the steps stay on screen after access is allowed")
def c_steps_hidden(ctl, env):
    opened(ctl, "not-asked")
    first = steps(ctl)
    set_access(ctl, "allowed")
    return (first, steps(ctl)) == ([], []), "steps while not asked %r, while allowed %r" % (first, steps(ctl))


@S.check("J10.2c/no-prompt-says-so-and-offers-settings", "no-prompt-silent", "macOS shows no prompt and the row changes nothing")
def c_no_prompt(ctl, env):
    opened(ctl, "no-prompt")
    act(ctl, "click", P + ".access.ask")
    a = access(ctl)
    st = [r["text"] for r in steps(ctl)]
    act(ctl, "click", P + ".access.open-settings")
    got = (a["state"], a["no_prompt"], a["line"], a["open_settings"], a["ask"], st, access(ctl)["opened_settings"])
    want = ("not-asked", True, W["line_no_prompt"], P + ".access.open-settings", P + ".access.ask", W["steps"], 1)
    return eq(got, want)


@S.check("J10.2c/an-answer-that-never-comes-stops-asking", "asking-forever", "the row says asking for ever when macOS never calls back")
def c_no_answer(ctl, env):
    import time
    opened(ctl, "no-answer")
    act(ctl, "click", P + ".access.ask")
    first = (access(ctl)["asking"], access(ctl)["line"])
    time.sleep(1.0)
    a = access(ctl)
    got = (first, a["asking"], a["no_prompt"], a["line"], a["open_settings"])
    want = ((True, W["line_asking"]), False, True, W["line_no_prompt"], P + ".access.open-settings")
    return eq(got, want)


@S.check("J10.2c/open-settings-door", "open-door-missing", "Open System Settings does nothing")
def c_open_settings(ctl, env):
    opened(ctl, "refused")
    act(ctl, "click", P + ".access.open-settings")
    n = access(ctl)["opened_settings"]
    set_access(ctl, "allowed")
    gone = refused(ctl, "click", P + ".access.open-settings")
    return (n, gone) == (1, True), "pressed %r time(s); door refused once allowed: %r" % (n, gone)


@S.check("J10.2c/calendar-list-is-read-only", "list-has-toggle", "a calendar row carries a switch that changes the replies' sources")
def c_read_only(ctl, env):
    opened(ctl, "allowed")
    before = feed(ctl)["enabled"]
    tries = [refused(ctl, "click", P + ".list.cal-work.toggle"), refused(ctl, "click", P + ".list.cal-personal.toggle"), refused(ctl, "click", P + ".list.cal-work")]
    after = feed(ctl)["enabled"]
    return (tries, before, after) == ([True, True, True], ["cal-work"], ["cal-work"]), "doors refused %r, enabled before %r after %r" % (tries, before, after)


@S.check("J10.2c/footnote-points-to-the-sources-pane", "footnote-wrong-place", "the line sends the owner to another pane")
def c_footnote(ctl, env):
    opened(ctl, "allowed")
    return eq(ctl.rows(P + ".footnote")[0]["text"], W["footnote"])


@S.check("J10.2c/a-change-made-in-the-system-reaches-the-pane", "state-change-stale", "the pane keeps the access it read first")
def c_system_change(ctl, env):
    opened(ctl, "refused")
    first = (access(ctl)["state"], len(steps(ctl)), len(calendars(ctl)))
    set_access(ctl, "allowed")
    second = (access(ctl)["state"], len(steps(ctl)), len(calendars(ctl)))
    return (first, second) == (("refused", 3, 0), ("allowed", 0, 2)), "refused: %r, then allowed: %r" % (first, second)


@S.check("J10.2c/the-core-gets-the-calendar-list", "feed-empty", "the calendar list is never handed to the replies")
def c_feed_calendars(ctl, env):
    opened(ctl, "allowed")
    return eq(feed(ctl)["calendars"], [i for i, _n, _a, _c in CALS])


@S.check("J10.2c/events-of-enabled-calendars-only", "feed-all-events", "the events of a calendar switched off are read as well")
def c_feed_events(ctl, env):
    opened(ctl, "allowed")
    f = feed(ctl)
    return (f["events"] > 0, f["event_calendars"], f["enabled"]) == (True, ["cal-work"], ["cal-work"]), "feed %r" % f


@S.check("J10.2c/refused-hands-nothing-to-the-core", "feed-kept-when-refused", "events stay in the replies after access is taken away")
def c_feed_refused(ctl, env):
    opened(ctl, "allowed")
    had = feed(ctl)["events"] > 0
    set_access(ctl, "refused")
    f = feed(ctl)
    return (had, f["calendars"], f["events"]) == (True, [], 0), "had events %r; after refusal %r" % (had, f)


@S.check("J10.2c/reset-returns-to-not-asked", "reset-keeps-state", "fixture reset leaves the access where it was")
def c_reset(ctl, env):
    opened(ctl, "refused")
    act(ctl, "click", P + ".access.open-settings")
    act(ctl, "fixture", "reset")
    a, f = access(ctl), feed(ctl)
    return (a["state"], a["opened_settings"], calendars(ctl), f["events"]) == ("not-asked", 0, [], 0), "access %r feed %r" % (a, f)


SRC_ROWS = "mac-settings-replies.sources"
SRC_LINK = "mac-settings-replies.sources.calendars.open"


def source_calendars(ctl):
    """The calendar tiles Settings > Replies > Sources draws: (name, enabled), as its own rows give them."""
    return [(r["name"], r["enabled"]) for r in ctl.rows(SRC_ROWS) if r.get("kind") == "calendar"]


def until(fn, want, secs=3.0):
    """The events follow a switch in the background (never on the click): read until the answer is the wanted one, or time is up."""
    import time
    end, got = time.time() + secs, None
    while time.time() < end:
        got = fn()
        if got == want:
            break
        time.sleep(0.05)
    return got


@S.check("J10.2c/sources-link-opens-the-pane", "sources-link-dead", "the Calendars link on the Sources page opens nothing")
def c_sources_link(ctl, env):
    opened(ctl)
    act(ctl, "click", "mac-settings-window.pane-row.general")
    before = ctl.rows(PLACE)[0]["pane"]
    act(ctl, "click", SRC_LINK)
    return (before, ctl.rows(PLACE)[0]["pane"]) == ("general", "agendas"), "pane before %r, after the link %r" % (before, ctl.rows(PLACE)[0]["pane"])


@S.check("J10.2c/allowed-calendars-appear-on-sources-with-their-switches", "sources-not-fed", "the calendars macOS gives never reach the Sources page")
def c_on_sources(ctl, env):
    opened(ctl, "allowed")
    return eq(source_calendars(ctl), [(n, n == "Travail") for _i, n, _a, _c in CALS])


@S.check("J10.2c/refused-takes-the-calendars-off-sources", "sources-kept-when-refused", "Sources keeps listing calendars after access is refused")
def c_off_sources(ctl, env):
    opened(ctl, "allowed")
    had = len(source_calendars(ctl))
    set_access(ctl, "refused")
    return (had, source_calendars(ctl)) == (2, []), "calendars on Sources while allowed: %r; after the refusal %r" % (had, source_calendars(ctl))


@S.check("J10.2c/switching-a-calendar-on-sends-its-events", "switch-sends-nothing", "a calendar switched on in Sources gets no events")
def c_switch_on(ctl, env):
    opened(ctl, "allowed")
    source_calendars(ctl)   # the Sources rows load the page's doors
    before = feed(ctl)["event_calendars"]
    act(ctl, "click", SRC_ROWS + ".calendar.cal-personal")
    after = until(lambda: feed(ctl)["event_calendars"], ["cal-personal", "cal-work"])
    return (before, after) == (["cal-work"], ["cal-personal", "cal-work"]), "events from %r, then from %r after the switch" % (before, after)


# ---------------------------------------------------------------- static cases: the Swift source and the lane note
def swift_files(env):
    return env["src"]()


def all_text(env):
    return "\n".join(swift_files(env).values())


@S.check("J10.2c/source-never-writes", "src-writes-event", "a file of the folder saves an event")
def c_no_writes(ctl, env):
    forbidden = [r"\.save\(", r"\.remove\(", r"\.commit\(", r"requestWriteOnlyAccessToEvents", r"requestAccess\(to:", r"EKEvent\(eventStore",
                 r"EKCalendar\(for", r"EKReminder", r"\bsaveEvent", r"\bremoveEvent", r"\bsaveCalendar", r"\bremoveCalendar",
                 r"func\s+(save|delete|remove|create|add|write|update)\w*"]
    text = all_text(env)
    hits = [p for p in forbidden if re.search(p, text)]
    asks_full = "requestFullAccessToEvents" in text
    return (hits, asks_full) == ([], True), "write calls found %r; full read access asked: %r" % (hits, asks_full)


@S.check("J10.2c/macos-is-asked-only-by-the-button", "src-asks-in-refresh", "reading the state also asks macOS")
def c_asked_by_button(ctl, env):
    files = swift_files(env)
    where = lambda pat: sorted(n for n, t in files.items() if re.search(pat, t))
    full = [m.start() for t in files.values() for m in re.finditer(r"requestFullAccessToEvents", t)]
    got = (len(full), where(r"requestFullAccessToEvents"), where(r"access\.requestAccess"), where(r"\.ask\(\)"))
    want = (1, ["EventKitCalendarAccess.swift"], ["CalendarsModel.swift"], ["CalendarsPaneView.swift", "SettingsCalendarsMacFeature.swift"])
    return eq(got, want)


@S.check("J10.2c/eventkit-is-never-waited-on", "src-blocks-main", "a function reads the event store without the queue")
def c_no_wait(ctl, env):
    files = swift_files(env)
    text = all_text(env)
    waits = [p for p in [r"\.sync\s*[\{\(]", r"DispatchSemaphore", r"\.wait\(", r"Thread\.sleep", r"usleep\("] if re.search(p, text)]
    elsewhere = sorted(n for n, t in files.items() if n != "EventKitCalendarAccess.swift" and re.search(r"EKEventStore\(|events\(matching|calendars\(for", t))
    ek = files.get("EventKitCalendarAccess.swift", "")
    loose = []
    for m in re.finditer(r"store\.(calendars\(for|events\(matching|requestFullAccessToEvents)", ek):
        before = ek[:m.start()]
        last_async, last_func = before.rfind("queue.async"), max((x.start() for x in re.finditer(r"\bfunc\s", before)), default=-1)
        if last_async < last_func:
            loose.append(m.group(0))
    return (waits, elsewhere, loose) == ([], [], []), "waits %r, event store used outside the queue file %r, calls outside queue.async %r" % (waits, elsewhere, loose)


@S.check("J10.2c/no-notification-crosses-into-the-main-actor", "src-passes-notification", "an observer hands its Notification to assumeIsolated")
def c_swift6(ctl, env):
    bad = [m.group(0)[:60] for m in re.finditer(r"assumeIsolated\s*\{[^}]*\b(n|note|notification)\b[^}]*\}", all_text(env))]
    return eq(bad, [])


@S.check("J10.2c/sources-heading-carries-the-link", "src-link-removed", "the Sources heading is plain text again")
def c_link_source(ctl, env):
    t = env["pane"]()
    ok = "SettingsCalendarsMacFeature.openPane()" in t and "calendars.open" in t and "calendarsNote" not in t
    return ok, "SourcesPane.swift calendar heading: link %r, plain note kept %r" % ("openPane()" in t, "calendarsNote" in t)


@S.check("J10.2c/log-lines-never-carry-event-text", "src-logs-title", "a log line names an event's title")
def c_log(ctl, env):
    bad = [l.strip()[:80] for l in all_text(env).splitlines() if "CalendarLog.line(" in l and re.search(r"\.(title|name|location|attendees|account)\b|e\.title", l)]
    steps_logged = all(w in all_text(env) for w in ["calendar access asked", "calendar access answer", "calendars read", "events sent"])
    return (bad, steps_logged) == ([], True), "lines with event or calendar text %r; every step logs %r" % (bad, steps_logged)


@S.check("J10.2c/an-empty-colour-or-name-never-reaches-the-core", "src-colour-raw", "a calendar with no colour is sent as is, and the core refuses the whole sync")
def c_sanitised(ctl, env):
    t = env["src"]().get("CoreReplyCalendarFeed.swift", "")
    return ("static func colour" in t and "colour: Self.colour(" in t and "CalendarWords.untitled" in t), "adapter sends the colour through Self.colour and a name through untitled"


def note_rows(env):
    """The rows `| English | French | ... |` of the note's "## New strings" section."""
    rows, inside = {}, False
    for line in env["note"]().splitlines():
        if line.startswith("## "):
            inside = line.startswith("## New strings")
            continue
        cells = [c.strip() for c in line.strip().strip("|").split(" | ")]
        if inside and line.startswith("|") and len(cells) >= 2 and not set(cells[0]) <= set("-: "):
            rows[cells[0]] = cells[1]
    return rows


def source_words(env):
    return [m.group(1).replace('\\"', '"') for m in re.finditer(r'Words\.text\("((?:[^"\\]|\\.)*)"', all_text(env))]


@S.check("J10.2c/every-word-is-listed-with-its-french", "note-missing-a-word", "a sentence of the pane is not in the note's table")
def c_words_listed(ctl, env):
    rows = note_rows(env)
    missing = sorted({w for w in source_words(env) if not rows.get(w)})
    return eq(missing, [])


@S.check("J10.2c/words-are-the-words-in-the-source", "src-word-changed", "a sentence of the Swift source differs from the one the check expects")
def c_words_match(ctl, env):
    have = set(source_words(env))
    return eq(sorted(w for w in SOURCE_WORDS if w not in have), [])


@S.check("J10.2c/no-dash-in-the-copy", "src-dash-in-copy", "a sentence carries a dash")
def c_no_dash(ctl, env):
    rows = note_rows(env)
    texts = source_words(env) + [rows[w] for w in source_words(env) if rows.get(w)]
    return eq([t for t in texts if re.search(r"[-–—]", t)], [])


# ---------------------------------------------------------------- the stand-in app (--selftest only)
class Fake:
    """`stored` is the language choice the app holds ("fr", or "system" which the stand-in resolves to French like the main79 rig); the app speaks it."""

    def __init__(self, mutants=(), stored="fr"):
        self.m = set(mutants)
        self.stored = stored
        self.reset()

    def spoken(self):
        return build("fr" if self.stored == "system" else self.stored)

    def reset(self):
        self.pane, self.state, self.shown, self.will_refuse, self.opened = "general", "not-asked", "not-asked", False, 0
        self.mode, self.no_prompt, self.deadline = "", False, 0.0
        self.enabled = ["cal-work"]
        self.cals, self.events = [c[0] for c in CALS], []   # what the fixture core holds before any access
        self.sync()

    def sync(self, from_fixture=False):
        if not ("state-change-stale" in self.m and from_fixture):
            self.shown = self.state
        if self.shown == "not-asked":
            return   # nothing is sent while macOS was not asked
        if self.shown != "allowed":
            if "feed-kept-when-refused" not in self.m or not self.events:
                self.cals, self.events = [], []
            return
        self.cals = [] if "feed-empty" in self.m else [c[0] for c in CALS]
        self.events = [e for e in EVENTS if "feed-all-events" in self.m or e[1] in self.enabled]

    def handle(self, verb, args):
        fn = getattr(self, "v_" + verb, None)
        if fn is None:
            return {"ok": False, "error": "unknown verb %s" % verb}
        try:
            r = fn(*args)
        except KeyError as e:
            return {"ok": False, "error": "no control %s" % e}
        return r if isinstance(r, dict) else {"ok": True}

    def v_uistate(self):
        return {"ok": True, "state": {"surface": "mac-settings-" + self.pane}}

    def v_fixture(self, name, *a):
        if name == "reset":
            if "reset-keeps-state" not in self.m:
                self.state, self.will_refuse, self.shown = "not-asked", False, "not-asked"
            self.opened = 0
            self.sync()
            return {"ok": True}
        if name == "calendar-access":
            self._set(a[0])
            return {"ok": True}
        return {"ok": False, "error": "fixture %s is not served" % name}

    def _set(self, word):
        if word not in ("not-asked", "allowed", "refused", "will-refuse", "no-prompt", "no-answer"):
            raise KeyError(word)
        self.mode, self.no_prompt, self.deadline = (word if word in ("no-prompt", "no-answer") else ""), False, 0.0
        self.state, self.will_refuse = ("not-asked" if word in ("will-refuse", "no-prompt", "no-answer") else word), word == "will-refuse"
        self.sync(from_fixture=True)

    def v_click(self, ax):
        shown = self.shown
        if ax.startswith(LANG_DOOR % ""):
            self.stored = ax.rsplit(".", 1)[1]
            return {"ok": True}
        if ax.startswith("mac-settings-window.pane-row.") and ax != "mac-settings-window.pane-row.agendas":
            self.pane = ax.rsplit(".", 1)[1]
            return {"ok": True}
        if ax == SRC_LINK:
            if "sources-link-dead" not in self.m:
                self.pane = "agendas"
            return {"ok": True}
        if ax == SRC_ROWS + ".calendar.cal-personal" or ax == SRC_ROWS + ".calendar.cal-work":
            who = ax.rsplit(".", 1)[1]
            self.enabled = [e for e in self.enabled if e != who] if who in self.enabled else self.enabled + [who]
            if "switch-sends-nothing" not in self.m:
                self.sync()
            return {"ok": True}
        if ax == "mac-settings-window.pane-row.agendas":
            if "no-pane-row" not in self.m:
                self.pane = "agendas"
            return {"ok": True}
        if ax.startswith(P + ".fixture."):
            self._set(ax[len(P + ".fixture."):])
            return {"ok": True}
        if ax == P + ".access.ask" and (shown == "not-asked" or "ask-door-stays-when-allowed" in self.m):
            if self.state == "not-asked" and self.mode == "no-prompt":
                self.no_prompt = "no-prompt-silent" not in self.m
            elif self.state == "not-asked" and self.mode == "no-answer":
                self.deadline = __import__("time").time() + 0.5
            elif self.state == "not-asked":
                self.state = "refused" if (self.will_refuse and "ask-always-allows" not in self.m) else "allowed"
                self.sync()
            return {"ok": True}
        if ax == P + ".access.open-settings" and (shown == "refused" or self.silent_now()) and "open-door-missing" not in self.m:
            self.opened += 1
            return {"ok": True}
        if ax in (P + ".list.cal-work.toggle", P + ".list.cal-personal.toggle") and "list-has-toggle" in self.m:
            who = ax.split(".")[-2]
            who = "cal-work" if who == "cal-work" else "cal-personal"
            self.enabled = [e for e in self.enabled if e != who] if who in self.enabled else self.enabled + [who]
            return {"ok": True}
        raise KeyError(ax)

    def asking_now(self):
        t = __import__("time").time()
        return "asking-forever" in self.m and self.deadline > 0 or t < self.deadline

    def silent_now(self):
        if self.deadline and not self.asking_now():
            self.no_prompt = True
        return self.no_prompt and self.shown == "not-asked"

    def v_rows(self, ax):
        return {"ok": True, "rows": self.rows_(ax)}

    def rows_(self, ax):
        shown, m = self.shown, self.m
        W = self.spoken()      # the stand-in speaks its own language, never the check's
        if ax == GEN_SETTINGS:
            return [{"language": self.stored}]
        if ax == PLACE:
            return [{"pane": self.pane}]
        if ax == P + ".access":
            asking, quiet = self.asking_now(), self.silent_now()
            line = W["line_asking"] if asking else (W["line_no_prompt"] if quiet else W["line"][shown])
            return [{"state": shown, "asking": asking, "no_prompt": quiet and not asking, "chip": W["chip"][shown], "line": line, "opened_settings": self.opened,
                     "ask": P + ".access.ask" if shown == "not-asked" else None,
                     "open_settings": P + ".access.open-settings" if shown == "refused" or (quiet and not asking) else None}]
        if ax == P + ".steps":
            if "steps-missing" in m:
                return []
            if shown == "refused" or "steps-always" in m or (self.silent_now() and not self.asking_now()):
                return [{"n": i + 1, "text": t, "ax": "%s.steps.%d" % (P, i + 1)} for i, t in enumerate(W["steps"])]
            return []
        if ax == P + ".list":
            if shown != "allowed" and "not-asked-shows-list" not in m:
                return []
            return [{"id": i, "name": n, "account": "" if "list-missing-account" in m else a, "colour": "" if "colour-lost" in m else c,
                     "ax": "%s.list.%s" % (P, i)} for i, n, a, c in CALS]
        if ax == P + ".list.state":
            return [{"state": "list" if shown == "allowed" else "hidden", "count": len(CALS) if shown == "allowed" else 0,
                     "title": W["list_title"], "note": W["read_only"]}]
        if ax == P + ".footnote":
            return [{"text": W["footnote"] + " Accounts" if "footnote-wrong-place" in m else W["footnote"]}]
        if ax == SRC_ROWS:
            if "sources-not-fed" in m and shown == "allowed":
                return []
            held = self.cals if "sources-kept-when-refused" not in m or shown == "allowed" else [c[0] for c in CALS]
            if shown == "refused" and "sources-kept-when-refused" in m:
                held = [c[0] for c in CALS]
            return [{"kind": "calendar", "name": n, "enabled": i in self.enabled} for i, n, _a, _c in CALS if i in held]
        if ax == P + ".feed":
            return [{"calendars": self.cals, "events": len(self.events), "event_calendars": sorted({e[1] for e in self.events}), "enabled": self.enabled}]
        raise KeyError(ax)


class _Handler(socketserver.StreamRequestHandler):
    def handle(self):
        for line in self.rfile:
            req = json.loads(line.decode())
            self.wfile.write((json.dumps(self.server.app.handle(req["verb"], req.get("args", []))) + "\n").encode())


class FakeServer:
    def __init__(self, mutants=(), stored="fr"):
        self.dir = tempfile.mkdtemp(prefix="wcal", dir="/tmp")
        self.srv = socketserver.ThreadingUnixStreamServer(os.path.join(self.dir, "whoosh.sock"), _Handler)
        self.srv.app = Fake(mutants, stored)
        threading.Thread(target=self.srv.serve_forever, daemon=True).start()

    def close(self):
        self.srv.shutdown()
        self.srv.server_close()
        shutil.rmtree(self.dir, ignore_errors=True)


def mutate_sources(files, mutants):
    f = dict(files)
    if "src-writes-event" in mutants:
        f["EventKitCalendarAccess.swift"] += "\nextension EventKitCalendarAccess { func note(_ e: EKEvent, _ s: EKEventStore) throws { try s.save(e, span: .thisEvent) } }\n"
    if "src-asks-in-refresh" in mutants:
        f["CalendarsPaneView.swift"] += "\nlet quiet: Void = CalendarsModel.shared.access.requestAccess { _ in }\n"
    if "src-blocks-main" in mutants:
        f["EventKitCalendarAccess.swift"] += "\nextension EventKitCalendarAccess { func hot() -> Int { store.calendars(for: .event).count } }\n"
    if "src-passes-notification" in mutants:
        f["CalendarsFixture.swift"] += "\nlet bad = { (n: Notification) in MainActor.assumeIsolated { print(n.name) } }\n"
    if "src-word-changed" in mutants:
        f["CalendarWords.swift"] = f["CalendarWords.swift"].replace('"Choose Calendars"', '"Pick Calendars"')
    if "src-logs-title" in mutants:
        f["CalendarsModel.swift"] += '\nextension CalendarsModel { func leak(_ e: CalendarEventInfo) { CalendarLog.line("event \\(e.title)") } }\n'
    if "src-colour-raw" in mutants:
        f["CoreReplyCalendarFeed.swift"] = f["CoreReplyCalendarFeed.swift"].replace("colour: Self.colour($0.colourHex)", "colour: $0.colourHex")
    if "src-dash-in-copy" in mutants:
        f["CalendarWords.swift"] = f["CalendarWords.swift"].replace('"Read only"', '"Read-only"')
    return f


def mutate_note(text, mutants):
    if "note-missing-a-word" in mutants:
        return "\n".join(l for l in text.splitlines() if not l.startswith("| Choose Calendars |"))
    return text


def read_pane():
    try:
        return open(os.path.join(ROOT, "Sources", "Features", "SettingsRepliesMac", "SourcesPane.swift"), encoding="utf-8").read()
    except OSError:
        return ""


def read_sources():
    out = {}
    for n in sorted(os.listdir(SRC_DIR)):
        if n.endswith(".swift"):
            out[n] = open(os.path.join(SRC_DIR, n), encoding="utf-8").read()
    return out


def read_note():
    try:
        return open(NOTE, encoding="utf-8").read()
    except OSError:
        return ""


def make_env(mutants):
    base, note, pane = read_sources(), read_note(), read_pane()
    return {"src": lambda: mutate_sources(base, mutants), "note": lambda: mutate_note(note, mutants),
            "pane": lambda: pane.replace("SettingsCalendarsMacFeature.openPane()", "{}").replace("calendars.open", "x") + "\nnote: RepliesWords.calendarsNote" if "src-link-removed" in mutants else pane}


def run_items(items, ctl, env):
    out = []
    for label, fn, _m, _d in items:
        try:
            ok, detail = fn(ctl, env)
            out.append((label, "pass" if ok else "fail", detail))
        except Refused as e:
            out.append((label, "fail", str(e)))
        except Unreadable as e:
            out.append((label, "unjudged", str(e)))
        except (KeyError, IndexError, TypeError, ValueError) as e:
            out.append((label, "fail", "reply shape: %s %s" % (type(e).__name__, e)))
    return out


def mutants_table(observed):
    lines = ["MUTANTS", "| check | mutant | mutation | observed |", "|---|---|---|---|"]
    for label, _fn, m, d in S.items:
        lines.append("| %s | %s | %s | %s |" % (label, m, d, observed.get(label, "declared; run --selftest to see it red")))
    return "\n".join(lines)


def against_fake(mutants, stored="fr"):
    srv = FakeServer(mutants, stored)
    ctl = Ctl(srv.dir).connect()
    try:
        out = run_items(S.items, ctl, make_env(mutants))
        was = restore_language(ctl)
        against_fake.after = (stored, was, srv.srv.app.stored)      # (stored before, choice put back, stored after)
        return out
    finally:
        ctl.close()
        srv.close()


def selftest():
    clean = against_fake(())
    n, f, u = len(clean), sum(r[1] == "fail" for r in clean), sum(r[1] == "unjudged" for r in clean)
    print("%d checks, %d failed, %d not judged" % (n, f, u))
    for l, st, d in clean:
        if st != "pass":
            sys.stderr.write("CLEAN-RUN %s %s: %s\n" % (st.upper(), l, d))
    # the stored choice is "system": the check pins English through the door, runs, and puts "system" back
    sysrun = against_fake((), "system")
    sys_bad = [(l, st) for l, st, _d in sysrun if st != "pass"]
    sys_ok = not sys_bad and against_fake.after == ("system", "system", "system")
    print("stored language 'system' (the stand-in speaks French): %d checks, not passing %r; (before, put back, after) = %r: %s" % (len(sysrun), sys_bad, against_fake.after, "OK" if sys_ok else "WRONG"))
    cd = against_fake((), "fr")
    print("stored language 'fr': pinned nothing, (before, put back, after) = %r" % (against_fake.after,))
    observed, bad, strays = {}, 0, {}
    for m in sorted({m for _l, _f, m, _d in S.items}):
        res = {l: (st, d) for l, st, d in against_fake((m,))}
        for l, _fn, mm, _d in S.items:
            if mm == m:
                st, d = res[l]
                observed[l] = "RED on this mutant: %s" % d if st == "fail" else "NOT RED (%s)" % st
                bad += st != "fail"
        # a mutant must turn red ONLY the checks that name it: another check going red on it is a finding
        stray = [l for l, (st, _d) in res.items() if st == "fail" and next(mm for ll, _f, mm, _dd in S.items if ll == l) != m]
        for l in stray:
            strays.setdefault(l, []).append(m)
    for l, ms in strays.items():
        observed[l] += " | also red on the mutant of another check: %s" % ", ".join(ms)
    print(mutants_table(observed))
    try:
        os.makedirs(CAPDIR, exist_ok=True)
        json.dump(observed, open(os.path.join(CAPDIR, "selftest-mutants.json"), "w", encoding="utf-8"), indent=1)
    except OSError:
        pass
    ok = f == 0 and u == 0 and bad == 0 and sys_ok and against_fake.after == ("fr", None, "fr")
    print("selftest: %s (%d mutants not red)" % ("OK" if ok else "FAILED", bad))
    return 0 if ok else 1


def drive():
    try:
        ctl = Ctl().connect()
    except CouldNotDrive as e:
        res = [(l, "unjudged", str(e)) for l, *_ in S.items]
    else:
        try:
            res = run_items(S.items, ctl, make_env(()))
        finally:
            try:
                put_back = restore_language(ctl)
                sys.stderr.write("language: %s\n" % ("the stored choice %r was put back" % put_back if put_back else "the app already held a catalogue language; nothing was changed"))
            except (Unreadable, CouldNotDrive, OSError) as e:
                sys.stderr.write("language: could not put the stored choice %r back (%s)\n" % (PINNED["was"], e))
            ctl.close()
    n, f, u = len(res), sum(r[1] == "fail" for r in res), sum(r[1] == "unjudged" for r in res)
    for l, st, d in res:
        if st != "pass":
            sys.stderr.write("%s %s: %s\n" % (st.upper(), l, d))
        else:
            print("PASS %s" % l)                                  # ONE line per passing case
    print("%d checks, %d failed, %d not judged" % (n, f, u))
    try:
        recorded = json.load(open(os.path.join(CAPDIR, "selftest-mutants.json"), encoding="utf-8"))
    except (OSError, ValueError):
        recorded = {}
    print(mutants_table(recorded))
    return 1 if f or u else 0


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv[1:] else drive())
