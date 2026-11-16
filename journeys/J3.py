"""J3: Write and send with dictation or an AI draft, then undo (docs/JOURNEYS.md J3.1-J3.8), on record 1's thread.

Written from the J3 step table, docs/CONSTANTS.md and fixtures/tests-j3.json; never from a built screen.
Brought to the app as served on main53 (tests-r54): the ids and reply fields are the ones the composer and the
dictation feature serve today (Sources/Features/ComposerMac/ComposerWords.swift, ComposerMacFeature.swift,
Sources/Features/Dictation/DictationFeature.swift), the same ones checks/parity/par-composermac.py and
checks/dictation-check.py drive:
  - dictation is the row `mac-composer.dictation` (state, live, line, audio_files); `fixture speak <words>` takes the
    words to say (the raw utterance), never an utterance id;
  - the signature is the receiving account's own (the design fixture's account row), "Best,\n<name> · <account>";
  - the AI draft and the rewrite need a writer: the rig's `mac-composer.dev.fixture-writer` stands in for the
    provider (the app's default writer has no provider: ComposerModel+AI.swift NoProviderWriter);
  - Enter after a rewrite is gone: the composer does not take Enter outside a list or the AI field, the key falls
    through to the inbox and opens a thread (that is what put the surface on mac-reader in the main53 run);
  - the undo bar counts on the wall clock, so "waits one second" is a real second; the core's window runs on the
    fixture clock (`clock advance`);
  - the send-later key (⌘⇧L) opens the composer on the thread with the picker, as the step's "reply key, then
    the send-later picker" says;
  - outbox entries are read relative to the count at the step's start, so the journey does not depend on what an
    earlier journey sent.
A core path or a row the app does not serve is a FAILED check (the step's acceptance has nothing to show it), never
a not-judged one: `served()` turns "no core value" into the words NOT_SERVED. Each such red is a product gap named in
docs/lanes/tests-r54.md. A send goes to the fixture core's outbox only: nothing leaves this Mac.

Themes: one pass per theme, as J1 does (journeys/run.py documents `--themes` but never reads it): WHOOSH_THEME=light|dark python3 journeys/run.py J3,
one app launch per theme; J3.1 sets the theme through `fixture theme` and reads it back (theme-set, theme-applied).

Mutation drive: J3_MUTATE="<label>,<label>" drops the named acts (labels in ACTS below); each dropped act must turn the
check after it red. Unset in every normal run.
"""
import datetime
import json
import os
import subprocess
import sys
import time
from lib.ctl import Unreadable
from lib.fixture import ACCOUNT, MSG, FX, const

# ---- the words the app shows, read from its catalogue (never typed in English here) -------------------------------------------------------------
# A hidden test copy speaks the language of the Mac (French on this one), so a sentence a check expects is the catalogue's text for the language
# the app speaks now (Sources/Resources/Localizable.xcstrings, the same file Sources/Localization/Words.swift reads; a key the catalogue lacks
# shows as written, as it does in the app). The language is read the way Words.swift picks it: the stored choice `whoosh.language`, else the
# language list of the rig identity, else the Mac's own list.
CATALOGUE_PATH = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "Sources", "Resources", "Localizable.xcstrings")
CATALOGUE = json.load(open(CATALOGUE_PATH, encoding="utf-8"))["strings"]
CODES = ["en", "fr", "es", "pt", "de", "it", "nl", "pl", "sv", "da", "nb", "fi", "ja", "zh-Hans"]      # Words.languages
RIG_DOMAIN = "io.nexus.whoosh.rig"
MISSING_KEYS = set()


def _defaults(*args):
    try:
        out = subprocess.run(["defaults"] + list(args), capture_output=True, text=True, timeout=10)
        return out.stdout if out.returncode == 0 else ""
    except (OSError, subprocess.SubprocessError):
        return ""


def mac_language():
    """(code, where it was read): what Words.current gives a test copy that was never told another language."""
    pick = _defaults("read", RIG_DOMAIN, "whoosh.language").strip()
    if pick in CODES:
        return pick, "whoosh.language of the rig identity"
    for args, where in ((("read", RIG_DOMAIN, "AppleLanguages"), "AppleLanguages of the rig identity"), (("read", "-g", "AppleLanguages"), "AppleLanguages of the Mac")):
        for line in _defaults(*args).splitlines():
            pref = line.strip().strip(",").strip('"').lower()
            code = next((c for c in sorted(CODES, key=len, reverse=True) if pref == c.lower() or pref.startswith(c.lower() + "-")), None)
            if code:
                return code, where
    return "en", "no language list read: English"


LANG = {"code": None, "where": ""}


def app_language():
    if LANG["code"] is None:
        LANG["code"], LANG["where"] = mac_language()
    return LANG["code"]


def learn_language(s, shown, key):
    """Settle the language from a text the APP shows, not from the Mac's preferences: the codes whose catalogue text for `key` is inside `shown`.
    The guess from the preferences stays when it is one of them. Nothing is read in a dry run, and a text no language matches keeps the guess."""
    if s.j.ctl is None or not isinstance(shown, str):
        return
    loc = CATALOGUE.get(key, {}).get("localizations", {})
    hits = [c for c in CODES if (loc.get(c, {}).get("stringUnit", {}).get("value") or (key if c == "en" else "")) and
            (loc.get(c, {}).get("stringUnit", {}).get("value") or key) in shown]
    guess = app_language()
    if hits:
        LANG["code"], LANG["where"] = (guess if guess in hits else hits[0]), "read from the app's own text %r" % shown[:70]
    s.j.notes.append("the app's language: %s (%s); the Mac's preferences said %s%s" % (LANG["code"], LANG["where"], guess, "" if hits else ", and no language matched the app's text"))


def words(key, *args, strict=True):
    """The app's text for a catalogue key in the language it speaks now; `%@` and `%lld` take the arguments in order."""
    entry = CATALOGUE.get(key)
    if entry is None and strict:
        MISSING_KEYS.add(key)
    loc = (entry or {}).get("localizations", {})
    value = loc.get(app_language(), {}).get("stringUnit", {}).get("value") or loc.get("en", {}).get("stringUnit", {}).get("value") or key
    for a in args:
        value = value.replace("%@", str(a), 1) if "%@" in value else value.replace("%lld", str(a), 1)
    return value

ID = "J3"
THEME = os.environ.get("WHOOSH_THEME", "light")        # one pass per theme, as J1 does: WHOOSH_THEME=light|dark python3 journeys/run.py J3 (one app launch per theme)
TITLE = "Write and send with dictation or an AI draft, then undo"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "The reply goes out from the receiving address with its signature after the undo window; an undo inside the window sends nothing; the scheduled copy waits in Scheduled"

T = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "fixtures", "tests-j3.json"), encoding="utf-8"))
R1 = MSG[T["record"]["message"]]
ACC = ACCOUNT[R1["account"]]
SIG = ACC["signature"]                                  # the receiving address's own signature (design fixture account row)
REPLYER = [m["from"] for m in FX["open_thread"]["messages"] if m["from"] != "Linea Supply"][0]
U1 = T["dictation"]["utterances"][0]
FIX = T["dictation"]["fixing_word"]["typed"]
UNDO = const("UNDO_SEND_DEFAULT_S")
BODY = "mac-composer.body"
HEAD = "mac-composer.header"
BAR = "mac-composer.undo.bar"
CARDS = "mac-main.thread.outgoing"                     # the thread's sends as cards (Sources/Features/QuickReply/ThreadCardsRegistry.swift)
# The fixture core has no mail server: no `server/...` path is served (FixtureCore.swift:1713 on). "Delivered" is read from what the core serves, the
# Sent mailbox's count (`counts/sent`: the outbox entries that left, FixtureLabels.swift:445-446) and the Outbox's (`counts/outbox`: the entries still
# waiting, undoable or scheduled, FixtureLabels.swift:446 `waitingEntriesLocked`), each against its count at the start of the step.
SENT = "counts/sent"
WAITING = "counts/outbox"
LANG_DOOR = "mac-settings-general.appearance.language.%s"      # the language pick of Settings > General, a click door that needs no window (par-settingsgeneralmac)
RECIP = "server/%s/inbox/count"                         # filled with the address the composer addresses (read from its header at J3.1): the fixture's "test_recipient" is a placeholder name, no address of the thread
FIRST_NAME = REPLYER.split()[0]                          # the voice: "signs off with the first name"
NOT_SERVED = "NOT_SERVED"
MUTATE = {x for x in os.environ.get("J3_MUTATE", "").split(",") if x}
ACTS = ["fn-down", "fn-up", "type-fix", "ai-run", "ai-edit", "rewrite-pick", "cmd-z", "send-1", "wait-1s", "undo-z", "send-2", "expire", "send-later-key", "pick-tomorrow"]


def act(label, fn, *a):
    """One act of the journey; J3_MUTATE drops it (the mutation drive)."""
    assert label in ACTS, label
    if label not in MUTATE:
        fn(*a)


def until(fn, ok, timeout=4.0):
    """Poll a read until ok(value) or the timeout; returns the last value (the check judges it)."""
    end = time.time() + timeout
    while True:
        v = fn()
        if ok(v) or time.time() > end:
            return v
        time.sleep(0.1)


def verb_ok(s, *args):
    """True when the app answers ok to a verb; an ok:false is a False, so the check after it turns red (a dry run raises Unreadable: not judged)."""
    try:
        s.ctl.send(*args)
        return True
    except Unreadable as e:
        if s.j.ctl is None:
            raise
        s.j.notes.append("verb %s refused: %s" % (" ".join(args), e))
        return False


def served(s, path):
    """A core value, or NOT_SERVED when the core answers that it has no such path (a failed check, not an unread one)."""
    c = s.ctl
    try:
        return c.core(path)
    except Unreadable as e:
        if "no core value" in str(e):
            return "%s %s" % (NOT_SERVED, path)
        raise


def rows_or_gap(s, ax):
    c = s.ctl
    try:
        return c.rows(ax)
    except Unreadable as e:
        if "no rows" in str(e):
            return []
        raise


def peek(fn, default=None):
    try:
        return fn()
    except (Unreadable, KeyError, IndexError, TypeError):
        return default


def expect_ctx(s, name, thunk, expected, ctx):
    """Like s.expect, and a failing case also prints what decides it: `ctx` is read only when the case fails (a read that fails says why)."""
    def run():
        got = thunk()
        s._eg = (expected, got)
        if got == expected:
            return True, ""
        try:
            more = ctx()
        except Exception as e:      # noqa: BLE001  the context never hides the verdict it explains
            more = "context unreadable (%s: %s)" % (type(e).__name__, e)
        return False, "expected %r, got %r | %s" % (expected, got, more)
    s._record(name, run)


def count_of(s, path):
    """A mailbox count the core serves, as a number (0 on a dry run or an unread core), for a step's starting figure."""
    if s.j.ctl is None:
        return 0
    try:
        return int(s.j.ctl.core(path))
    except (Unreadable, TypeError, ValueError):
        return 0


def since(s, path, base):
    """The core's count at `path` minus its figure at the step's start; the words NOT_SERVED when the core serves none."""
    v = served(s, path)
    return v - base if isinstance(v, int) and not isinstance(v, bool) else v


def seconds_to_pick(s):
    """Seconds from now to the instant the tomorrow-8:00 picker row resolves to (its own `resolves_to`), read before the pick. The clock verb takes
    `advance <seconds>` only (Verbs.swift:83); a margin of ten minutes covers what the fixture clock already moved (undo windows advanced above).
    None when no such row was read: the case that follows then says the mail did not leave."""
    if s.j.ctl is None:
        return 1
    row = peek(lambda: tomorrow_0800(s.j.ctl.rows("mac-composer.send-later.options")))
    if not row:
        s.j.notes.append("J3.8: no picker row resolves to tomorrow %s, so the clock was not advanced" % T["schedule"]["local_time"])
        return None
    at = datetime.datetime.strptime(row["resolves_to"], "%Y-%m-%d %H:%M:%S %z")
    return int((at - datetime.datetime.now(at.tzinfo)).total_seconds()) + 600


def body(s):
    return s.ctl.rows(BODY)[0]["text"]


def drow(s):
    return s.ctl.rows("mac-composer.dictation")[0]


def bar(s):
    return s.ctl.rows(BAR)[0]


def count_now(s):
    """The outbox's entry count before a step's act (0 on a dry run or an unread core): the step reads its entries from there."""
    if s.j.ctl is None:
        return 0
    try:
        return int(s.j.ctl.core("outbox_entry/count"))
    except (Unreadable, TypeError, ValueError):
        return 0


def nth_state(s, i):
    return served(s, "outbox_entry/%d/state" % i)


def tomorrow_0800(rows):
    """The picker row that resolves to tomorrow at the fixture's local time, by its title (the words the owner reads)."""
    want = T["schedule"]["local_time"]
    tomorrow = (datetime.datetime.now() + datetime.timedelta(days=1)).date()
    for r in rows:
        try:
            d = datetime.datetime.strptime(r.get("resolves_to", ""), "%Y-%m-%d %H:%M:%S %z").astimezone()
        except ValueError:
            continue
        if d.date() == tomorrow and d.strftime("%H:%M") == want:
            return r
    return None


def drive(j):
    """The language the journey pins (English: the clean-up's filler and capital rules run only when the app speaks English, LocalCleaner.swift:6-8)
    is put back to the Mac's own (`system`) at the end, a failed run included, so a later journey starts from what the test copy started with."""
    try:
        _drive(j)
    finally:
        if j.ctl is not None:
            try:
                j.ctl.click(LANG_DOOR % "system")
            except Unreadable as e:
                j.notes.append("J3: the language was not put back: %s" % e)


def _drive(j):
    mem = {}

    s = j.step("J3.1", "Open the full composer, hold Globe and speak")
    s.expect("theme-set", lambda: verb_ok(s, "fixture", "theme", THEME), True)
    s.expect("theme-applied", lambda: s.ctl.uistate()["theme"], THEME)
    # The dictation clean-up removes fillers and capitalises months only when the app speaks English; a test copy speaks the Mac's language
    # (French here), so the journey picks English through the language door, reads the pick back, and reads every sentence below from the catalogue in English.
    s.click(LANG_DOOR % "en")
    LANG["code"], LANG["where"] = "en", "picked by the journey through %s" % (LANG_DOOR % "en")
    j.notes.append("J3 picks English for the dictation and puts the language back to system at its end")
    s.expect("app-language-pinned", lambda: s.ctl.rows("mac-settings-general.settings")[0]["language"], "en")
    s.key("Escape")
    s.key("R")
    s.arrive(("click", "mac-main.reader.open-full-composer"), "mac-composer")
    s.expect("from-is-receiving-account", lambda: s.ctl.rows(HEAD)[0]["from"], ACC["address"])
    s.expect("signature-of-that-address", lambda: s.ctl.rows(HEAD)[0]["signature"], SIG)
    s.expect("reply-addressed-to-one-recipient", lambda: (mem.__setitem__("recip", RECIP % s.ctl.rows(HEAD)[0]["to"][0]) or len(s.ctl.rows(HEAD)[0]["to"])), 1)
    s.expect("quoted-text-folded", lambda: s.ctl.rows(HEAD)[0]["quoted_folded"], True)
    s.expect("dictation-idle-first", lambda: (mem.__setitem__("a0", drow(s)["audio_files"]) or drow(s)["state"]), "idle")
    act("fn-down", s.key, "Fn:down")
    s.expect("state-listening", lambda: until(lambda: drow(s)["state"], lambda v: v == "listening"), "listening")
    s.raw("fixture", "speak", U1["raw"])
    s.expect("live-words", lambda: (mem.__setitem__("live", until(lambda: drow(s)["live"] or "", lambda v: "accept" in v.lower()).lower()) or "accept" in mem["live"]), True)
    s.expect("audio-held-while-listening", lambda: drow(s)["audio_files"] - mem["a0"], 1)   # relative: a recording an earlier run left on disk is not this one

    s = j.step("J3.2", "Release the key, fix one word by typing")
    act("fn-up", s.key, "Fn:up")
    s.expect("cleaned-text-at-cursor", lambda: until(lambda: body(s), lambda v: v == U1["cleaned"], 6.0), U1["cleaned"])
    s.expect("raw-had-fillers", lambda: (s.ctl.uistate(), all(f in mem.get("live", "").split() for f in U1["fillers_in_raw"]))[1], True)
    s.expect("fillers-removed", lambda: [f for f in U1["fillers_in_raw"] if (" %s " % f) in (" %s " % body(s).lower())], [])
    s.expect("audio-deleted-at-text", lambda: drow(s)["audio_files"] - mem["a0"], 0)
    s.expect("dictation-idle-after", lambda: until(lambda: drow(s)["state"], lambda v: v == "idle"), "idle")
    act("type-fix", s.type, FIX)
    s.expect("typed-fix-appended-not-replaced", lambda: body(s), U1["cleaned"] + FIX)
    # The composer saves its draft in the background once the typing has rested (ComposerModel+Send.swift draftPauseMs, 1000 ms; abeb8c83): the count is
    # read until it says 1, for at most 4 s, never right after the key (main81 read 0 before that build existed, and would read 0 here inside the pause).
    s.expect("draft-saved-in-core", lambda: until(lambda: served(s, "draft/count"), lambda v: v == 1, 4.0), 1)

    s = j.step("J3.3", "AI draft in the owner's voice")
    s.click("mac-composer.dev.fixture-writer")
    s.arrive(("click", "mac-composer.ai-draft"), "mac-composer")
    s.type(T["ai"]["instruction"])
    if s.j.ctl:
        try:
            mem["pre_ai"] = body(s)             # the text the draft must replace, read before the act (not the text the fixture expected)
        except (Unreadable, IndexError, KeyError):
            mem["pre_ai"] = None
    act("ai-run", s.key, "Enter")
    s.expect("draft-written", lambda: (mem.__setitem__("ai", until(lambda: body(s), lambda v: v != mem.get("pre_ai"), 4.0)) or mem["ai"]) not in ("", mem.get("pre_ai")), True)
    s.expect("draft-in-owners-voice", lambda: (lambda b: "March" in b and b.rstrip().endswith(FIRST_NAME) and "!" not in b)(body(s)), True)
    s.expect("draft-uses-writer-agent", lambda: served(s, "ai_job/last/agent"), T["ai"]["writer_agent"])
    s.expect("draft-uses-voice-profile-of-address", lambda: served(s, "ai_job/last/voice_profile_address"), ACC["address"])
    s.expect("marked-as-ai", lambda: len(rows_or_gap(s, "mac-composer.ai-marks")) > 0, True)
    s.expect("mark-covers-the-draft", lambda: s.ctl.rows("mac-composer.ai-marks")[0]["words"] == mem.get("ai"), True)
    s.expect("focus-back-in-the-text-after-draft", lambda: s.ctl.rows(HEAD)[0]["focused"], "body")
    act("ai-edit", s.type, ".")
    deciding = lambda: "body ends %r; the header says focused=%r; first responder %r; AI marks %d; the draft ended %r before the edit" % (
        body(s)[-40:], s.ctl.rows(HEAD)[0].get("focused"), s.ctl.uistate().get("first_responder"), len(rows_or_gap(s, "mac-composer.ai-marks")), (mem.get("ai") or "")[-20:])
    expect_ctx(s, "edit-lands-in-the-draft", lambda: body(s), (mem.get("ai") or "") + ".", deciding)
    expect_ctx(s, "mark-clears-on-edit", lambda: len(rows_or_gap(s, "mac-composer.ai-marks")), 0, deciding)

    s = j.step("J3.4", "Rewrite a paragraph shorter, undo with Cmd-Z")
    s.arrive(("click", "mac-composer.rewrite"), "mac-composer")
    s.expect("choices-in-words", lambda: [r["title"].lower() for r in s.ctl.rows("mac-composer.rewrite.choices")], [words(c, strict=False).lower() for c in T["ai"]["rewrite_choices"]])      # a choice the catalogue lacks shows as written, as in the app
    s.expect("old-text-on-screen-before-rewrite", lambda: (mem.__setitem__("before", body(s)) or len(mem["before"])) > 0, True)
    act("rewrite-pick", s.click, "mac-composer.rewrite." + T["ai"]["rewrite_chosen"])
    s.expect("paragraph-replaced-shorter", lambda: (lambda b: 0 < len(b) < len(mem.get("before", "")))(until(lambda: body(s), lambda v: v != mem.get("before"))), True)
    act("cmd-z", s.key, "⌘Z")
    s.expect("cmd-z-restores-exact-old-text", lambda: until(lambda: body(s), lambda v: v == mem.get("before")), mem.get("before"))
    s.expect("still-in-the-composer", lambda: s.ctl.uistate().get("surface"), "mac-composer")

    s = j.step("J3.5", "Send, the undo window counts down")
    mem["n0"] = count_now(s)
    mem["sent0"] = count_of(s, SENT)
    act("send-1", s.key, "⌘Enter")
    s.expect("arrives-by-control", lambda: (s.ctl.uistate().get("surface"), bar(s)["undo_offered"]), ("mac-composer", True))
    s.expect("bar-starts-at-the-window", lambda: (mem.__setitem__("s0", bar(s)["seconds_left"]) or mem["s0"]), UNDO)
    act("wait-1s", time.sleep, 1.0)
    s.expect("bar-counting-down", lambda: (lambda left: 1 <= mem.get("s0", -99) - left <= 2)(bar(s)["seconds_left"]), True)   # one second of wall clock, one read of latency
    s.expect("outbox-entry-undoable", lambda: nth_state(s, mem["n0"]), "undoable")
    s.expect("outbox-has-one-entry", lambda: s.ctl.core("outbox_entry/count") - mem["n0"], 1)
    s.expect("nothing-delivered-during-window", lambda: since(s, SENT, mem["sent0"]), 0)

    s = j.step("J3.6", "Undo send inside the window")
    act("undo-z", s.key, "Z")
    s.expect("arrives-by-control", lambda: (s.ctl.uistate().get("surface"), len(rows_or_gap(s, BAR))), ("mac-composer", 0))
    s.expect("composer-reopens-with-same-text", lambda: body(s), mem.get("before", ""))
    s.expect("outbox-entry-cancelled", lambda: nth_state(s, mem["n0"]), "cancelled")
    s.expect("server-sent-folder-holds-no-copy", lambda: since(s, SENT, mem["sent0"]), 0)
    s.expect("recipient-received-nothing", lambda: served(s, mem.get("recip", RECIP % T["record"]["test_recipient"])), 0)
    s.raw("clock", "advance", UNDO + 1)
    s.expect("still-cancelled-after-window-passes", lambda: nth_state(s, mem["n0"]), "cancelled")

    s = j.step("J3.7", "Send again and let it run out")
    act("send-2", s.key, "⌘Enter")
    s.expect("arrives-by-control", lambda: (s.ctl.uistate().get("surface"), bar(s)["undo_offered"]), ("mac-composer", True))
    act("expire", lambda: (time.sleep(UNDO + 1), s.raw("clock", "advance", UNDO + 1)))   # the owner's wall clock and the core's clock both run out
    s.expect("bar-says-sent", lambda: until(lambda: bar(s)["text"], lambda v: v == words("Sent")), words("Sent"))
    s.expect("undo-no-longer-offered", lambda: bar(s)["undo_offered"], False)
    s.expect("outbox-entry-sent", lambda: nth_state(s, mem["n0"] + 1), "sent")
    s.expect("composer-text-spent-after-sent", lambda: rows_or_gap(s, BODY)[:1] in ([], [{"ax": BODY, "text": ""}]), True)
    s.expect("reply-shows-in-the-thread-as-sent", lambda: (lambda c: (c[-1]["state"], c[-1]["account"]) if c else None)(until(lambda: rows_or_gap(s, CARDS), lambda v: bool(v) and v[-1]["state"] == "sent")), ("sent", R1["account"]))
    s.expect("reply-text-in-thread", lambda: rows_or_gap(s, CARDS)[-1]["text"].startswith(mem.get("before", "?")), True)
    s.expect("one-copy-in-sent-folder", lambda: since(s, SENT, mem["sent0"]), 1)
    s.expect("recipient-got-exactly-one", lambda: served(s, mem.get("recip", RECIP % T["record"]["test_recipient"])), 1)
    s.expect("sent-body-ends-with-signature", lambda: rows_or_gap(s, CARDS)[-1]["text"].rstrip().endswith(SIG), True)

    # setup for J3.8, not an act of the step: a composer the window left open is discarded so the reply starts clean
    if s.j.ctl and rows_or_gap(s, HEAD):
        s.click("mac-composer.discard")
        s.click("mac-composer.discard.confirm")

    s = j.step("J3.8", "Schedule a reply for tomorrow 8:00")
    s.expect("composer-closed-to-the-thread", lambda: len(until(lambda: rows_or_gap(s, HEAD), lambda v: not v, 4.0)), 0)
    s.expect("composer-closed-positive-control", lambda: len(s.ctl.rows("mac-main.list")) > 0, True)
    mem["n1"] = count_now(s)
    mem["waiting1"] = count_of(s, WAITING)
    s.key("R")
    act("send-later-key", s.key, "⌘⇧L")
    s.expect("arrives-by-control", lambda: (s.ctl.uistate().get("surface"), len(rows_or_gap(s, "mac-composer.send-later.options")) > 0), ("mac-composer", True))
    s.type("Confirming March.")
    s.expect("picker-in-words", lambda: (lambda r: bool(r) and not any(ch.isdigit() for ch in r["title"]))(tomorrow_0800(s.ctl.rows("mac-composer.send-later.options"))), True)
    mem["to_8"] = seconds_to_pick(s)
    act("pick-tomorrow", s.click, "mac-composer.send-later.tomorrow-0800")
    s.expect("composer-says-scheduled", lambda: (lambda l: "OK" if l.startswith(words("Scheduled for %@", "").rstrip()) and "8:00" in l else l)(s.ctl.rows("mac-composer.line")[0]["text"]), "OK")
    s.expect("outbox-entry-scheduled", lambda: (s.ctl.core("outbox_entry/count") - mem["n1"], nth_state(s, mem["n1"])), (1, "scheduled"))
    s.expect("listed-in-scheduled", lambda: since(s, WAITING, mem["waiting1"]), 1)
    s.expect("not-delivered-before-its-time", lambda: since(s, SENT, mem["sent0"]), 1)
    if mem["to_8"] is not None:      # `clock advance_to <words>` is not a verb (Verbs.swift:83 takes `advance <seconds>`); the seconds come from the picker's own row
        s.raw("clock", "advance", mem["to_8"])
    s.expect("leaves-at-its-time", lambda: nth_state(s, mem["n1"]), "sent")
    s.expect("delivered-once-more", lambda: since(s, SENT, mem["sent0"]), 2)
    if s.j.ctl and rows_or_gap(s, HEAD):      # leave no draft behind for the next journey
        s.click("mac-composer.discard")
        s.click("mac-composer.discard.confirm")

# MUTANTS
# J3.1/theme-set | the app does not serve the theme verb
# J3.1/theme-applied | the app ignores the theme and stays light
# J3.1/app-language-pinned | the language door does nothing, so the dictation clean-up never runs its English rules
# J3.1/arrives-by-control | "open full composer" opens quick reply again
# J3.1/from-is-receiving-account | the From field takes the first account instead of the receiving one
# J3.1/reply-addressed-to-one-recipient | the reply opens with no recipient or two, so the delivery counts below read an address nobody got mail at
# J3.1/signature-of-that-address | the signature of another address is inserted
# J3.1/quoted-text-folded | the quoted text opens unfolded
# J3.1/dictation-idle-first | the dictation state starts as listening
# J3.1/state-listening | holding Fn starts nothing (J3_MUTATE=fn-down)
# J3.1/live-words | no live text while speaking (J3_MUTATE=fn-down)
# J3.1/audio-held-while-listening | the recording is never written, so the delete check below is vacuous
# J3.2/cleaned-text-at-cursor | the raw transcript is inserted unpunctuated (J3_MUTATE=fn-up: nothing inserted)
# J3.2/raw-had-fillers | the live words are already cleaned, so the filler check below proves nothing
# J3.2/fillers-removed | the clean-up pass keeps um and uh
# J3.2/audio-deleted-at-text | the audio file stays after the text appears
# J3.2/dictation-idle-after | the state stays at listening (J3_MUTATE=fn-up)
# J3.2/typed-fix-appended-not-replaced | typing after dictation replaces the whole text (J3_MUTATE=type-fix)
# J3.2/draft-saved-in-core | the draft is not saved in the core
# J3.3/arrives-by-control | AI draft control missing
# J3.3/draft-written | the AI returns an empty draft (J3_MUTATE=ai-run)
# J3.3/draft-in-owners-voice | the draft ignores the voice: no first-name sign-off, or an exclamation mark
# J3.3/draft-uses-writer-agent | the draft runs without the Writer agent
# J3.3/draft-uses-voice-profile-of-address | the draft ignores the voice profile
# J3.3/marked-as-ai | AI text carries no mark (J3_MUTATE=ai-run)
# J3.3/mark-covers-the-draft | the mark covers other words than the draft
# J3.3/focus-back-in-the-text-after-draft | the AI field closes and leaves the keys nowhere, so the owner's next key edits nothing
# J3.3/edit-lands-in-the-draft | the typed character goes elsewhere, so the mark check below proves nothing
# J3.3/mark-clears-on-edit | the AI mark stays after an edit (J3_MUTATE=ai-edit)
# J3.4/arrives-by-control | rewrite control missing
# J3.4/choices-in-words | the menu offers shorter and warmer only
# J3.4/old-text-on-screen-before-rewrite | the composer shows no text before the rewrite, so the checks below compare nothing
# J3.4/paragraph-replaced-shorter | rewrite changes nothing (J3_MUTATE=rewrite-pick)
# J3.4/cmd-z-restores-exact-old-text | Cmd-Z restores a trimmed copy of the old text (J3_MUTATE=cmd-z)
# J3.4/still-in-the-composer | a key the composer does not take falls through and opens a thread over it
# J3.5/arrives-by-control | Cmd-Enter does nothing (J3_MUTATE=send-1)
# J3.5/bar-starts-at-the-window | the bar starts at another figure than UNDO_SEND_DEFAULT_S
# J3.5/bar-counting-down | the bar does not count down (J3_MUTATE=wait-1s)
# J3.5/outbox-entry-undoable | send writes the entry as already sent
# J3.5/outbox-has-one-entry | send writes two entries
# J3.5/nothing-delivered-during-window | the mail leaves at once and the bar is cosmetic
# J3.6/arrives-by-control | Z does not undo: the bar stays (J3_MUTATE=undo-z)
# J3.6/composer-reopens-with-same-text | the composer reopens empty
# J3.6/outbox-entry-cancelled | undo leaves the entry pending (J3_MUTATE=undo-z)
# J3.6/server-sent-folder-holds-no-copy | undo-send still sent the mail
# J3.6/recipient-received-nothing | the cancelled mail still reaches the recipient
# J3.6/still-cancelled-after-window-passes | undo only pauses the timer and the mail leaves later
# J3.7/arrives-by-control | second send does nothing (J3_MUTATE=send-2)
# J3.7/bar-says-sent | no Sent confirmation (J3_MUTATE=expire)
# J3.7/undo-no-longer-offered | undo stays offered after the window (J3_MUTATE=expire)
# J3.7/outbox-entry-sent | the entry never reaches sent (J3_MUTATE=expire)
# J3.7/composer-text-spent-after-sent | the composer keeps the sent text, as if the send were cancelled
# J3.7/reply-shows-in-the-thread-as-sent | the reply is missing from the thread, or from another account (J3_MUTATE=send-2)
# J3.7/reply-text-in-thread | the thread shows other text than was sent
# J3.7/one-copy-in-sent-folder | no copy, or two copies, in the Sent folder
# J3.7/recipient-got-exactly-one | the recipient gets nothing, or the cancelled send as well
# J3.7/sent-body-ends-with-signature | the signature is dropped on send (the sent card is the body that left)
# J3.8/composer-closed-to-the-thread | the composer stays open after Sent
# J3.8/composer-closed-positive-control | the probe reads nothing at all, so the empty composer read above means nothing
# J3.8/arrives-by-control | Cmd-Shift-L opens nothing (J3_MUTATE=send-later-key)
# J3.8/picker-in-words | the picker lists clock times only, or no row resolves to tomorrow 8:00
# J3.8/composer-says-scheduled | the composer does not say when it will leave (J3_MUTATE=pick-tomorrow)
# J3.8/outbox-entry-scheduled | the entry is stored as undoable, or not stored (J3_MUTATE=pick-tomorrow)
# J3.8/listed-in-scheduled | the message is not listed in Scheduled
# J3.8/not-delivered-before-its-time | the scheduled mail goes at once
# J3.8/leaves-at-its-time | the scheduled mail never leaves
# J3.8/delivered-once-more | the scheduled mail leaves twice


def selftest():
    """No app: the journey runs dry (no socket) and every check must name its mutant above, every mutant a check, and every catalogue key the
    journey reads must be in the catalogue. Writes no results file. The dry run's one line per check it could not judge is counted, not listed."""
    import contextlib
    import io
    from lib import harness
    harness.write_results = lambda *a, **k: None
    noise = io.StringIO()
    with contextlib.redirect_stderr(noise):
        r = harness.run_journey(sys.modules[__name__], os.path.abspath(__file__), force_dry=True)
    declared = harness.declared_mutants(os.path.abspath(__file__))
    bad = len(r["no_mutant"]) + len(r["stray_mutant"]) + len(MISSING_KEYS) + (0 if r["checks"] else 1)
    for label in r["no_mutant"]:
        print("NO MUTANT  %s" % label)
    for label in r["stray_mutant"]:
        print("STRAY      %s names no check" % label)
    for key in sorted(MISSING_KEYS):
        print("NOT IN THE CATALOGUE  %r" % key)
    print("%s: %d checks run dry, %d not judged (no app), the app's language read as %s (%s)" % (ID, r["checks"], r["unjudged"], app_language(), LANG["where"]))
    print("%s selftest: %d checks, %d mutants, %d not red or missing" % (ID, r["checks"], len(declared), bad))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else 0)
