"""J2: Find a message by meaning or exact words, across every account and year (JOURNEYS J2.1-J2.8; round 3 = the one search, picked look S1 Quiet rows).
Record 1 = the Linea Supply quote thread (msg-01, acc-04, 0.84 EUR, quote-Q1-print-run.pdf).

Written from docs/JOURNEYS.md J2 and docs/CONTROL-SOCKET.md only, never from a built screen.
Expected values come from fixtures/tests-j2.json (the search corpus: it dates record 1 in spring and adds the
decoys each check needs to be able to fail) and docs/CONSTANTS.md. Accessibility ids and the `core state` paths
that docs/CONTROL-SOCKET.md does not serve yet are ASSUMPTIONS, listed in docs/lanes/tests-j2.md; the producing
lanes (searchmac, labelscontactsmac, core search) register them. Surfaces: mac-search, mac-main, mac-labels-contacts.
"""
import json
import os
import re
import subprocess
import sys
import time

from lib.ctl import Unreadable
from lib import theme as _theme
from lib.fixture import MSG, const

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

ID = "J2"
TITLE = _theme.title("Find a message by meaning or by exact words")
TARGET = "LOCAL"
VISIBLE_OUTCOME = "The supplier thread opens from the first screen of results; the quote is saved as a smart folder"

_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CORPUS = json.load(open(os.path.join(_ROOT, "fixtures", "tests-j2.json"), encoding="utf-8"))
EXP = CORPUS["expect"]
BY_ID = {m["id"]: m for m in CORPUS["messages"]}
# Item 3bb3d817 (lane every-word): a match holds EVERY typed word (search-text query.rs; FixtureSearchBook.holdsEvery). No mail holds the four words of
# the meaning query, so the J2.1 cases built on its one-word matches (the count line's matches, the tinted words, the paperclip of record 1's match) read the
# answer of WORDS_QUERY: msg-01 (record 1) and msg-s05 hold all three words, thr-s06 is close in meaning. The same query as checks/parity/par-searchmac.py
# WORDS_QUERY; the sets: _captures/every-word/match-sets-5.out.
WORDS_QUERY = "quote price run"
# and the grouping case reads GROUP_QUERY: msg-01, msg-01a and msg-01b (thr-01) all hold its words, so a list of one row per message shows thr-01 three times
GROUP_QUERY = "print run"
# and the J2.6 rule edit adds the colon form has:attachment: the edited folder holds exactly the figure's mails with a file, msg-01 and msg-s02
EDITED_RULE_COUNT = 2
WORDS_TINT = {t: sorted({w for w in WORDS_QUERY.split() if re.search(r"\b%s\b" % w, m["subject"].lower())}) for m in CORPUS["messages"]
              for t in [m["thread"]] if t in EXP["meaning_query_must_include_threads"]
              and all(w in " ".join([m["subject"], m["body"], m["from_name"], m["from"]] + list(m.get("attachments") or [])).lower() for w in WORDS_QUERY.split())}
R1 = MSG["msg-01"]                      # the design fixture's record 1 (sender, account, attachment name)
assert R1["account"] == EXP["record1_account"] and R1["attachments"][0] == EXP["record1_attachment"]
# The corpus must be able to make the grouping check fail: record 1's thread holds several messages that match the meaning words.
assert sum(1 for m in CORPUS["messages"] if m["thread"] == EXP["thread_with_several_matches"]
           and "quote" in (m["subject"] + m["body"]).lower()) >= 2


def _filtered(from_name=None, attach=False, before=None):
    """Threads of the corpus that pass the typed filters (the filter semantics of JOURNEYS J2.2), sorted."""
    out = set()
    for m in CORPUS["messages"]:
        if from_name and from_name.lower() not in m["from_name"].lower():
            continue
        if attach and not m["attachments"]:
            continue
        if before and not m["date"][:10] < before:
            continue
        out.add(m["thread"])
    return sorted(out)


NAME_ALL = _filtered("Linea Supply")                                          # the person chip alone (the chip filters by the name)
NARROWED = _filtered("Linea Supply", True, CORPUS["before_june"])            # with all three chips
WITHOUT_BEFORE = _filtered("Linea Supply", True, None)                       # after the "before" chip is removed
assert NARROWED == EXP["filter_result_threads_with_meaning_words"] and NARROWED != WITHOUT_BEFORE != NAME_ALL

RES = "mac-search.results"
res = lambda s: s.ctl.rows(RES)
kinds = lambda s: sorted(r["kind"] for r in s.ctl.rows("mac-search.chips"))


def remove_id(s, kind):
    """The remove control of the chip of this kind, as the chip row publishes it in its `remove` field
    (the screen indexes chips, `mac-search.chips.<n>.remove`; a guessed `chips.<kind>.remove` clicks nothing)."""
    return [r["remove"] for r in s.ctl.rows("mac-search.chips") if r["kind"] == kind][0]


SUG, SKEL, SECT, HEAD, FOOT = ("mac-search.dropdown", "mac-search.skeleton", "mac-search.sections", "mac-search.header", "mac-search.footer")
ORDERS = EXP["people_chip_address"]
BY_ADDR_ALL = sorted({m["thread"] for m in CORPUS["messages"] if m["from"] == ORDERS})
BY_ADDR_NARROWED = sorted({m["thread"] for m in CORPUS["messages"] if m["from"] == ORDERS and m["attachments"] and m["date"][:10] < CORPUS["before_june"]})
BY_ADDR_NO_BEFORE = sorted({m["thread"] for m in CORPUS["messages"] if m["from"] == ORDERS and m["attachments"]})
assert BY_ADDR_ALL != BY_ADDR_NO_BEFORE != BY_ADDR_NARROWED          # each chip step can be told apart from the one before


def until(fn, secs=4.0):
    """Poll a read until it is truthy (results arrive after the skeleton); the last read is returned either way."""
    end = time.time() + secs
    while time.time() < end:
        try:
            v = fn()
            if v:
                return v
        except Exception:
            pass
        time.sleep(0.1)
    return fn()


def launch(s, text):
    """Replace the bar's words and press Enter: the dropdown's first row, `Search all mail`, launches the search."""
    s.click("mac-search.bar")
    s.key("⌘A")
    s.type(text)
    s.key("Enter")


def peek(fn, default=None):
    """A read for navigation or for naming the app's language: an unreadable probe gives the default, never a verdict."""
    try:
        return fn()
    except (Unreadable, KeyError, IndexError, TypeError):
        return default


def divider_follows_stage(s):
    """The one line the pane draws after the matches (SPEC A150, docs/surfaces/mac-search.md round 3): its words follow the stage the engine ended on
    (`mac-search.header` stage): ranked "Related" when related mail is listed (rows of part `related`) and no line when none is; no model "Other mails, by
    date" when other mail stands behind the closed line (header `rest`); the second answer pending "Ranking the other mails by meaning…"."""
    head, rows = s.ctl.rows(HEAD), res(s)
    stage = head[0].get("stage") if head else None
    key = EXP["mac_divider_keys"].get(stage)
    related = any(r["part"] == "related" for r in rows)
    rest = bool(head and head[0].get("rest"))
    show = {"ranked": related, "words": True, "no_model": rest}.get(stage, False)
    if related and stage != "words":                     # related mail listed always stands under "Related" (SearchResultsList.titles, round 8)
        key, show = EXP["mac_divider_keys"]["ranked"], True
    want = [words(key, strict=False)] if key and show else []      # the stage sentences: a catalogue without one shows the key as written, as the app does
    got = [r["title"] for r in s.ctl.rows(SECT)]
    return got == want, "stage %r: the divider says %r, wanted %r (related rows %s, rest behind the closed line %s)" % (stage, got, want, related, rest)


def count_line(n, m):
    """SPEC A150's count line in the app's language: "N matches · M related", the matches alone when nothing is related, "No match · M related"."""
    if m == 0:
        return words("1 match") if n == 1 else words("%lld matches", n)
    if n == 0:
        return words("No match · 1 related") if m == 1 else words("No match · %lld related", m)
    if n == 1:
        return words("1 match · 1 related") if m == 1 else words("1 match · %lld related", m)
    return words("%lld matches · 1 related", n) if m == 1 else words("%lld matches · %lld related", n, m)


def header_counts(s):
    """The count line (SPEC A150): "N matches" counts the matches (the leading run of rows of part `match`), "· M related" the related mail listed after
    them; the mail behind the closed line is never counted there. The title stays "Results". This search must list both (record 1 matches, thr-s06 is related)."""
    rows = res(s)
    lead = next((i for i, r in enumerate(rows) if r["part"] != "match"), len(rows))
    related = sum(1 for r in rows if r["part"] == "related")
    want = [(words("Results"), count_line(lead, related), lead, related)]
    got = [(h["title"], h["count"], h["matches"], h["related"]) for h in s.ctl.rows(HEAD)]
    return got == want and lead > 0 and related > 0, "expected %r (matches %d, related %d), got %r" % (want, lead, related, got)


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


def slug(text):
    """The id part the labels screen gives a name (LabelsContactsSource.slug): lower case, runs of letters and digits joined by dashes."""
    return "-".join(w for w in re.split(r"[^\w]+", text.lower()) if w)


def editor_row(s):
    """The folder editor's own row: its rule, name, parent and whether it is new, as the screen holds them now."""
    r = s.ctl.rows("mac-labels-contacts.rule.matches")[0]
    return {k: r.get(k) for k in ("rule", "name", "parent", "new", "folder_id")}


def groups(s):
    out = []
    for r in s.ctl.rows(SUG):
        if r["group"] not in out:
            out.append(r["group"])
    return out


def row_of(s, message_id):
    return next((r for r in res(s) if r["id"] == message_id), None)


def phases(s):
    """The app's own split of the last input's time (`measure mac-main.interaction.phases`, InteractionProbe.Phase), the non-zero phases only, so a
    timing over its budget says where the time went (main105: first-screen-ms 480 ms and one-interaction-budget-ms 61.8 ms, no split printed)."""
    try:
        got = s.ctl.measure("mac-main.interaction.phases")
        return {k: round(v, 1) for k, v in (got.get("phases") or {}).items() if v}
    except Exception as e:           # a build that serves no phases says so
        return "not served (%s: %s)" % (type(e).__name__, e)


def lt(s, name, thunk, limit):
    """Strictly under a budget: SEARCH_FIRST_SCREEN_MS is the line a screen must stay UNDER (perf/search.py fails at >=). Over it, the line carries the phases."""
    def run():
        got = thunk()
        return got < limit, "expected < %r, got %r" % (limit, got) + ("" if got < limit else " | mac-main.interaction.phases %s" % (phases(s),))
    s._record(name, run)


def le(s, name, thunk, limit):
    """At most a budget (the harness's expect_le), with the phases on the line when it is over."""
    def run():
        got = thunk()
        return got <= limit, "expected <= %r, got %r" % (limit, got) + ("" if got <= limit else " | mac-main.interaction.phases %s" % (phases(s),))
    s._record(name, run)

DROP = set(filter(None, os.environ.get("J2_DROP", "").split(",")))   # mutation (a): the listed steps skip their own act


def arrive(s, via, surface):
    """The arrival by the named control; under mutation (a) the control is not pressed at all, and the arrival check must go red."""
    if acts(s):
        s.arrive(via, surface)
    else:
        s.expect("arrives-by-control", lambda: s.ctl.uistate().get("surface"), surface)


def acts(s):
    """False when this step's act is deleted for the mutation run (J2_DROP=J2.1,J2.3): its checks must then go red."""
    return s.id not in DROP


def need(s, thunk):
    """A read the step needs but the app serves no reader for (no rows, no measure, no core value): live, that is a red
    named as a product gap, never a silent not-judged. Dry (no socket) it stays not judged."""
    def run():
        try:
            return thunk()
        except Unreadable as e:
            if s.j.ctl is None:
                raise
            return "PRODUCT GAP, no reader: %s" % e
    return run


def under_load(value, budget, strict=True):
    """The shared load rule (journeys/lib/loadgate.py, lead 2026-11-13): at or below PERF_LOAD_MAX the timing is judged as always; above
    it a timing inside its budget still passes (the load is printed) and one over its budget is NOT JUDGED with its number."""
    from lib import loadgate
    return loadgate.under_load(value, budget, strict, Unreadable)


def same(s, name, got, want):
    """Two live reads that must agree (the screen's figure and the core's own): both are read when the check runs."""
    def run():
        a, b = got(), want()
        return a == b, "screen %r, core %r" % (a, b)
    s._record(name, run)


def folder_id(s, name):
    return next((r["id"] for r in s.ctl.rows("mac-search.sidebar.smart-folders") if r["title"] == name), None)


def in_folder(s, name, field, f=lambda v: v):
    """The core's value for the folder the sidebar names; no such folder is a red (it was never saved), not a not-judged."""
    fid = folder_id(s, name)
    return f(s.ctl.core("smart_folder/%s/%s" % (fid, field))) if fid else "no folder %r in the sidebar" % name


def threads_of(ids):
    """Message ids (what the core's smart_folder/<id>/threads answers) as their corpus threads."""
    return sorted({BY_ID[i]["thread"] if i in BY_ID else i for i in (ids or [])})




def drive(j):
    q = EXP["meaning_query"]
    s = j.step("J2.1", "Search by meaning")
    # The corpus the expectations are computed from must be the corpus the app searches (lead fix, 2026-11-08).
    s.raw("fixture", "corpus", os.path.join(_ROOT, "fixtures", "tests-j2.json"))
    _theme.apply(s)
    arrive(s, ("key", "/"), "mac-search")
    if acts(s):
        s.type(q)
    # the dropdown (`mac-search.dropdown`): row 0 is the search row (kind `search`, the typed words; the view draws "Search all mail"
    # beside it, the reply does not carry that caption), then People, Labels, the three "contains" rows, recents
    learn_language(s, peek(lambda: next((r["title"] for r in s.ctl.rows(SUG) if r["kind"] == "recent"), None)), "Close in meaning")   # no dropdown row speaks a catalogue sentence now: the language is learned from the divider below
    s.expect("dropdown-first-row-is-search-all-mail", lambda: [(r["kind"], r["title"]) for r in s.ctl.rows(SUG)[:1]], [("search", q)])
    s.expect("dropdown-subject-and-attachment-rows", lambda: ([r["kind"] for r in s.ctl.rows(SUG) if r["kind"] in ("subject", "attachment")], [r["title"] for r in s.ctl.rows(SUG) if r["kind"] == "attachment"]),
             (["subject", "attachment"], [q]))      # A143: a Subject row and an Attachment row beside the people; the Attachment row carries the typed words
    s.click("mac-search.skeleton.hold")             # the screen's own hold: the skeleton stays until released (par-searchmac uses it)
    if acts(s):
        s.key("Enter")
    lt(s, "first-screen-ms", lambda: under_load(s.ctl.measure("mac-main.interaction.frame")["ms"], const("SEARCH_FIRST_SCREEN_MS")), const("SEARCH_FIRST_SCREEN_MS"))
    s.expect("skeleton-rows-on-the-same-frame", lambda: (s.ctl.rows(SKEL)[0]["loading"], s.ctl.rows("mac-search.bar")[0]["launched"]), (True, True))
    s.expect("skeleton-rows-fill-the-pane", need(s, lambda: (lambda sk, lst: sk["h"] >= const("LIST_ROW_PX") and sk["y"] >= lst["y"] and sk["y"] + sk["h"] <= lst["y"] + lst["h"])(
        s.ctl.measure(SKEL)["frame"], s.ctl.measure("mac-main.list.view")["frame"])), True)
    s.click("mac-search.skeleton.release")
    s.expect("skeleton-gives-way-to-results", lambda: (until(lambda: len(res(s)) > 0), s.ctl.rows(SKEL)[0]["loading"]), (True, False))
    s.expect("record1-in-first-screen", lambda: EXP["record1_thread"] in [r["thread"] for r in res(s)], True)
    s.expect("meaning-match-without-shared-words", lambda: EXP["meaning_query_semantic_only_thread"] in [r["thread"] for r in res(s)], True)
    # the view draws the count line ("N matches · M close in meaning"), ONE divider before the close group and the footer line (SearchResultsList,
    # SearchListPane); each is read from its own rows (`mac-search.header`, `mac-search.sections`, `mac-search.footer`)
    s._record("results-divider-follows-the-stage", lambda: divider_follows_stage(s))          # replaces the Top results / All results rows (A141, S1: one divider)
    s.expect("results-footer-line", need(s, lambda: [r["text"] for r in s.ctl.rows(FOOT)]), [words(EXP["results_footer"])])
    s.expect("meaning-only-row-tints-nothing", lambda: [r["highlights"] for r in res(s) if r["thread"] == EXP["meaning_query_semantic_only_thread"]], [[]])
    s.expect("account-colour-per-result", lambda: {r["thread"]: (r["account"], r["colour"].startswith("#")) for r in res(s)
                                                  if r["thread"] in (EXP["record1_thread"], EXP["meaning_query_semantic_only_thread"])},
             {EXP["record1_thread"]: (EXP["record1_account"], True), EXP["meaning_query_semantic_only_thread"]: (BY_ID["msg-s06"]["account"], True)})
    s.expect("history-written", lambda: q in s.ctl.core("search/recent"), True)
    # item 3bb3d817: the cases that read the matches, on the words query (record 1 and msg-s05 hold every word; thr-s06 is related)
    if acts(s):
        launch(s, WORDS_QUERY)
    s._record("results-header-counts-exact-and-close", lambda: header_counts(s))                # replaces "N conversations": "N matches · M close in meaning"
    s.expect("matched-words-tinted", lambda: {r["thread"]: sorted({w.lower() for w in r["highlights"]}) for r in res(s)
                                              if r["thread"] in WORDS_TINT}, WORDS_TINT)
    # no paperclip = the row's `attachment` is null
    s.expect("paperclip-only-where-the-match-is-in-the-attachment", lambda: {r["thread"]: r["attachment"] for r in res(s)
                                                                             if r["thread"] in (EXP["record1_thread"], "thr-s05")},
             {EXP["record1_thread"]: EXP["record1_attachment"], "thr-s05": None})
    if acts(s):
        launch(s, GROUP_QUERY)                     # item 3bb3d817: the three mails of thr-01 hold every word of this one
    s.expect("grouped-by-thread", lambda: [r["thread"] for r in res(s)].count(EXP["thread_with_several_matches"]), 1)

    s = j.step("J2.2", "Filters in words")
    s.click("mac-search.bar")                       # same bar, same screen: no navigation, so no arrival check (it could not fail)
    s.key("⌘A")
    if acts(s):
        s.type(EXP["people_query"])                 # the signed words: "from " and the supplier's first letters
    s.expect("typed-from-makes-no-pill", lambda: kinds(s), [])             # SPEC A142: the word "from" typed before letters stays a word, whatever the dropdown offers
    s.expect("people-rows-list-both-addresses", lambda: sorted(r["detail"] for r in s.ctl.rows(SUG) if r["kind"] == "from"),
             sorted(EXP["people_addresses"]))
    s.expect("dropdown-groups-people-filters-recent-in-order", lambda: (
        lambda g, k: (g == sorted(g), "from" in k, "subject" in k, "recent" in k))([r["group"] for r in s.ctl.rows(SUG)], [r["kind"] for r in s.ctl.rows(SUG)]),
        (True, True, True, True))
    s.expect("people-rows-show-name-and-address", lambda: sorted((r["title"], r["detail"]) for r in s.ctl.rows(SUG) if r["kind"] == "from"),
             sorted((EXP["people_chip_value"], a) for a in EXP["people_addresses"]))
    # main53: "from <letters>" turns into a sender-contains chip at once and lists no People rows (the red above). The person chip is
    # then reached the way the screen offers it: the supplier's first letters alone list the People rows, Down to the address, Tab.
    s.key("⌘A")
    s.type(EXP["people_query"].split(" ", 1)[1])
    if acts(s):
        s._act(lambda: [s.ctl.key("Down") for _ in range(next((i for i, r in enumerate(s.ctl.rows(SUG)) if r["detail"] == ORDERS), 0))])
        s.key("Tab")
    s.expect("tab-adds-the-person-as-a-chip", lambda: [(r["kind"], r["value"]) for r in s.ctl.rows("mac-search.chips")],
             [("from", ORDERS)])                                        # the pill carries the ADDRESS (it names one person), the row drew the name
    s.expect("list-narrows-on-the-chip-without-enter", lambda: sorted({r["thread"] for r in res(s)}), BY_ADDR_ALL)
    if acts(s):
        s.type(" before:%s has:attachment" % CORPUS["before_june"])       # the colon forms make the pills (A142); the words "before June has attachment" make none
    s.expect("filter-chips", lambda: kinds(s), EXP["filter_chip_kinds"])
    s.expect("list-narrows", lambda: sorted({r["thread"] for r in res(s)}), BY_ADDR_NARROWED)
    s.expect("chip-removable", lambda: (s.ctl.click(remove_id(s, "before")) or True) and kinds(s), ["attachment", "from"])
    s.expect("chip-removal-widens", lambda: sorted({r["thread"] for r in res(s)}), BY_ADDR_NO_BEFORE)

    s = j.step("J2.3", "Exact figure in quotes")
    s.click("mac-search.bar")                       # same bar, same screen: no navigation, so no arrival check (it could not fail)
    s.key("⌘A")
    if acts(s):
        s.type(EXP["exact_query"])
        s.key("Enter")
    s.expect("bar-shows-only-the-exact-query", lambda: s.ctl.rows("mac-search.bar")[0]["text"], EXP["exact_query"])
    s.expect("exact-message-ranks-first", lambda: until(lambda: res(s) and res(s)[0]["id"] == EXP["exact_first_message"]) and res(s)[0]["id"], EXP["exact_first_message"])
    # a result row carries no section (`group` is on the dropdown rows only, SearchMacRows.swift:22, :36-43). The screen draws the first
    # `topCount` rows under the first section (SearchResultsList.swift:26 `rs.indices.prefix(topCount)`), and `mac-search.sections` names that
    # first section: the exact hit is result 0, so it sits under whatever the first section is called.
    s.expect("exact-message-is-listed-before-the-divider", lambda: (res(s)[0]["id"], res(s)[0]["matched_in"] != "meaning"), (EXP["exact_first_message"], True))
    s.expect("figure-highlighted", lambda: "0.84 EUR" in res(s)[0]["highlights"], True)
    # absence ("not the near figures") carries its positive control: the line above shows the real hit IS highlighted
    s.expect("only-the-exact-figure-highlighted", lambda: [r["id"] for r in res(s) if "0.84 EUR" in r["highlights"]], EXP["exact_matching_messages"])

    # J2.8 arrives from the J2.3 field (its signed `from`), so it runs here; J2.4 then opens J2.3's result again
    s = j.step("J2.8", "Search inside an attachment")   # same field, same screen (its signed `from` is J2.3): no arrival check, it could not fail
    if acts(s):
        launch(s, EXP["photo_query"])
    s.expect("bar-holds-exactly-the-query", lambda: s.ctl.rows("mac-search.bar")[0]["text"], EXP["photo_query"])
    s.expect("photo-message-ranks-first", lambda: until(lambda: res(s) and res(s)[0]["id"] == EXP["photo_message"]) and res(s)[0]["id"], EXP["photo_message"])
    s.expect("photo-attachment-named-in-the-row", lambda: (lambda r: (r["attachment"], r["matched_in"].startswith("attachment")))(row_of(s, EXP["photo_message"])),
             (EXP["photo_attachment"], True))
    if acts(s):
        launch(s, EXP["photo_content_query"])
    s.expect("photo-found-by-what-it-shows", lambda: until(lambda: res(s) and res(s)[0]["id"] == EXP["photo_message"]) and
             (res(s)[0]["id"], res(s)[0]["matched_in"], res(s)[0]["attachment"], s.ctl.rows("mac-search.bar")[0]["text"]),
             (EXP["photo_message"], "attachment-content", EXP["photo_attachment"], EXP["photo_content_query"]))
    if acts(s):
        launch(s, EXP["not_downloaded_query"])
    s.expect("not-downloaded-row-says-so", lambda: until(lambda: row_of(s, EXP["not_downloaded_message"])) and
             (lambda r: (r["attachment"], r["attachment_says"]))(row_of(s, EXP["not_downloaded_message"])),
             (EXP["not_downloaded_attachment"], EXP["not_downloaded_note"]))
    if acts(s):
        launch(s, EXP["unreadable_query"])
    s.expect("unreadable-kind-searched-by-name-and-says-so", lambda: until(lambda: row_of(s, EXP["unreadable_message"])) and
             (lambda r: (r["attachment"], EXP["unreadable_note_words"] in (r["attachment_says"] or ""), r["matched_in"]))(row_of(s, EXP["unreadable_message"])),
             (EXP["unreadable_attachment"], True, "attachment-name"))
    launch(s, EXP["exact_query"])                  # back to J2.3's results: J2.4 opens its result

    s = j.step("J2.4", "Open the thread, drag the attachment out")
    arrive(s, ("key", "Enter"), "mac-reader")   # the picked three-pane design opens a thread on mac-reader
    le(s, "one-interaction-budget-ms", lambda: under_load(s.ctl.measure("mac-main.interaction.commit")["ms"], const("INTERACTION_COMMIT_MS"), strict=False), const("INTERACTION_COMMIT_MS"))
    # record 1 is also the inbox's focused message at launch, so every read here also asks that the SEARCH opened it: the reader is
    # not resting (an opened thread) and the search's passage names the message (`mac-search.passage`, set by openResult)
    opened = lambda: ([r["id"] for r in s.ctl.rows("mac-search.passage")], [r["resting"] for r in s.ctl.rows("mac-main.reader.subject")])
    s.expect("thread-subject", lambda: ([(r["id"], r["subject"]) for r in s.ctl.rows("mac-main.reader.subject")], opened()),
             ([(EXP["record1_message"], BY_ID["msg-01"]["subject"])], ([EXP["record1_message"]], [False])))
    s.expect("marked-read", lambda: (s.ctl.core("message/%s/read" % EXP["record1_message"]), opened()), (True, ([EXP["record1_message"]], [False])))
    s.expect("attachment-shown", lambda: ([r["name"] for r in s.ctl.rows("mac-main.reader.attachments")], opened()),
             ([EXP["record1_attachment"]], ([EXP["record1_message"]], [False])))
    if acts(s):
        s.raw("drag", "mac-main.reader.attachment.0", "desktop")
    # main53 serves no drag-out of an attachment and no read of the desktop: red as a product gap until it does
    s.expect("file-on-desktop", need(s, lambda: s.ctl.core("desktop/files")), [EXP["record1_attachment"]])

    name = EXP["smart_folder_name"]
    s = j.step("J2.5", "Save as smart folder")
    arrive(s, ("key", "Escape"), "mac-search")
    if acts(s):
        s.click("mac-search.save-as-smart-folder")
        s.type(name)
        s.key("Enter")
    s.expect("folder-in-sidebar", lambda: name in [r["title"] for r in s.ctl.rows("mac-search.sidebar.smart-folders")], True)
    # "lists the same threads": the field's reading of the rule AND the core's own answer for the folder (message ids, as threads)
    s.expect("folder-lists-same-threads", lambda: (sorted(next((r["threads"] for r in s.ctl.rows("mac-search.sidebar.smart-folders") if r["title"] == name), [])),
                                                   in_folder(s, name, "threads", threads_of)),
             (EXP["smart_folder_threads_after_save"], EXP["smart_folder_threads_after_save"]))
    s.expect("smart-folder-in-core", lambda: in_folder(s, name, "rule"), EXP["exact_query"])
    s.click("mac-search.save-as-smart-folder")
    s.type(name)
    s.key("Enter")
    s.expect("duplicate-name-refused", lambda: ("exists already" in s.ctl.rows("mac-search.save-as-smart-folder.field")[0]["says"],
                                                [r["title"] for r in s.ctl.rows("mac-search.sidebar.smart-folders")].count(name)), (True, 1))
    s.key("Escape")                                 # the name field closes

    s = j.step("J2.6", "Nest the folder under Work, edit the rule")
    # The socket's typed text is ONE buffer shared by the screens (FeatureModule.swift:89; the search field mirrors its words into it,
    # SearchFocus.swift:20) and the folder editor drains it into the rule (LabelsContactsModel.takeTyped :66-70). What the search bar held when
    # the editor was asked for is read here, so a doubled rule prints the words that were pending.
    bar_before = []
    s._act(lambda: bar_before.append({k: v for k, v in s.ctl.rows("mac-search.bar")[0].items() if k in ("text", "focused", "owner", "active")}))
    arrive(s, ("click", "mac-search.sidebar.smart-folders.supplier-quotes.edit"), "mac-labels-contacts")
    expect_ctx(s, "rule-in-words", lambda: s.ctl.rows("mac-labels-contacts.rule.matches")[0]["rule"], EXP["exact_query"],
               lambda: "search bar before the editor opened: %r; editor now: %r" % (bar_before, editor_row(s)))
    target = []   # (id, name) of the "Work" label when the tree has one (J1.5 makes labels), else of the tree's first top-level label: the nesting is the act
    s._act(lambda: target.append(next(((r["id"], r["name"]) for r in s.ctl.rows("mac-labels-contacts.labels") if r["name"] == EXP["smart_folder_parent"]),
                                      next(((r["id"], r["name"]) for r in s.ctl.rows("mac-labels-contacts.labels") if r["depth"] == 1), None))))
    fid = []
    s._act(lambda: fid.append(next((r["id"] for r in s.ctl.rows("mac-labels-contacts.sidebar") if r["title"] == name), None)))
    fv = lambda: (s.ctl, fid[0] if fid else None)[1]   # the folder's id as the labels screen lists it (a read: dry, not judged)
    if acts(s):
        # the screen's own gesture: a folder row carries no drag (LabelsSidebar.swift:55-58, :136-158); the editor lists the top-level labels as
        # parent chips (SmartFolderEditor.swift:68-73, id `mac-labels-contacts.parent.<name>`) and Save keeps the pick (:75-87)
        s._act(lambda: target and s.ctl.click("mac-labels-contacts.parent.%s" % slug(target[0][1])))
        s._act(lambda: target and s.ctl.click("mac-labels-contacts.rule.save"))
    sidebar_parent = lambda: [r["parent"] for r in s.ctl.rows("mac-labels-contacts.sidebar") if r["title"] == name]
    expect_ctx(s, "nested-under-work", need(s, lambda: s.ctl.core("smart_folder/%s/parent" % fv())), target[0][0] if target else EXP["smart_folder_parent"],
               lambda: "the sidebar row says parent %r; editor now: %r" % (sidebar_parent(), editor_row(s)))
    # the sidebar publishes the parent the chip held, which is the label's NAME (LabelsContactsMacFeature.swift:122 `setParent(n.name)`)
    s.expect("sidebar-shows-nesting", sidebar_parent, [target[0][1] if target else EXP["smart_folder_parent"]])
    if acts(s):
        s.type(" has:attachment")                   # item 3bb3d817: the words "and has attachment" are held by no mail (every typed word must be): the colon form
        s.click("mac-labels-contacts.rule.save")
    expect_ctx(s, "rule-edited", lambda: (fv() and s.ctl.core("smart_folder/%s/rule" % fv()) or "").endswith(" has:attachment"), True,
               lambda: "the core's rule: %r; the editor's: %r; the sidebar row's: %r" % (s.ctl.core("smart_folder/%s/rule" % fv()), editor_row(s),
                                                                                     [r.get("rule") for r in s.ctl.rows("mac-search.sidebar.smart-folders") if r["title"] == name]))
    # the screen's figure and the core's own, both the exact figure of the edited rule: the figure's mails with a file, msg-01 and msg-s02 (item 3bb3d817,
    # _captures/every-word/match-sets-8.out '"0.84 EUR" has:attachment'); before, 0 against 0 could not fail
    s.expect("count-updates", lambda: ([r["count"] for r in s.ctl.rows("mac-labels-contacts.sidebar") if r["title"] == name],
                                       [len(s.ctl.core("smart_folder/%s/threads" % fv()) or [])] if fv() else "no folder %r on the screen" % name), ([EDITED_RULE_COUNT], [EDITED_RULE_COUNT]))

    s = j.step("J2.7", "Everything from one sender")
    # bridge back to the thread: / shows the search with J2.3's results; a click on the first result reopens record 1 (after the labels
    # editor, main53's keys no longer reach the field: Enter there opens nothing, so the result's own click carries the hop)
    s.key("/")
    s.click("mac-search.results.0")
    arrive(s, ("click", "mac-main.reader.sender"), "mac-labels-contacts")
    s.expect("sender-messages-newest-first", lambda: [r["id"] for r in s.ctl.rows("mac-labels-contacts.contact.messages")],
             EXP["sender_messages_newest_first"])
    s.expect("each-row-has-account", lambda: [r["account"] for r in s.ctl.rows("mac-labels-contacts.contact.messages")], EXP["sender_accounts"])
    s.expect("sender-is-linea", lambda: s.ctl.rows("mac-labels-contacts.contact.header")[0]["address"], EXP["sender_address"])

# MUTANTS
# J2.1/theme-applied | the app ignores the theme and stays light
# J2.1/arrives-by-control | the search key opens nothing
# J2.1/dropdown-first-row-is-search-all-mail | the filter-word rows come above Search all mail
# J2.2/dropdown-groups-people-filters-recent-in-order | Recent searches come before the From rows, or the group numbers go back
# J2.1/dropdown-subject-and-attachment-rows | the Subject row or the Attachment row is missing, or the Attachment row does not carry the typed words
# J2.1/skeleton-rows-on-the-same-frame | Enter shows an empty list until the results arrive
# J2.1/skeleton-rows-fill-the-pane | the skeleton shows a fixed 3 rows however tall the pane
# J2.1/skeleton-gives-way-to-results | the skeleton rows stay on screen after the results land
# J2.1/record1-in-first-screen | the account that received record 1 is left out of the results
# J2.1/meaning-match-without-shared-words | semantic search is off, only literal matches rank
# J2.1/grouped-by-thread | results list one row per message
# J2.1/results-divider-follows-the-stage | the line says Related when no model answered or nothing is related, or no line stands above the related rows, or it says the old Close in meaning (SearchResultsList.dividerTitle ignores the stage, SPEC A150)
# J2.1/results-header-counts-exact-and-close | the count line counts every row as a match, or leaves out the related number, or counts the mail behind the closed line (SPEC A150)
# J2.1/results-footer-line | the footer line is missing
# J2.1/matched-words-tinted | no word is tinted
# J2.1/meaning-only-row-tints-nothing | the meaning-only row tints a word it does not hold
# J2.1/paperclip-only-where-the-match-is-in-the-attachment | every row with an attachment draws it
# J2.1/account-colour-per-result | results lose the account colour
# J2.1/first-screen-ms | the first screen waits for the full index scan
# J2.1/history-written | the query is not saved to search history
# J2.2/people-rows-list-both-addresses | the People rows list one address per person
# J2.2/typed-from-makes-no-pill | the typed word "from" before letters becomes a sender pill again (the `from <word>` branch of SearchQuery back)
# J2.2/people-rows-show-name-and-address | a From row shows the name but no address
# J2.2/tab-adds-the-person-as-a-chip | Tab does nothing on a person row
# J2.2/list-narrows-on-the-chip-without-enter | the chip is added but the list waits for Enter
# J2.2/filter-chips | the typed colon forms before:<day> and has:attachment make no pills
# J2.2/list-narrows | the filters are parsed but not applied
# J2.2/chip-removable | the chip has no remove control
# J2.2/chip-removal-widens | removing the chip leaves the list as narrow as before
# J2.3/exact-message-ranks-first | the quoted figure is treated as a loose term
# J2.3/exact-message-is-listed-before-the-divider | the exact hit is listed after the divider, among the close ones
# J2.3/bar-shows-only-the-exact-query | select-all does nothing, the figure is typed after the old words
# J2.3/figure-highlighted | the figure is not highlighted
# J2.3/only-the-exact-figure-highlighted | the quoted figure also matches inside 110.84 EUR
# J2.4/arrives-by-control | Enter on a result opens nothing
# J2.4/thread-subject | the wrong thread opens
# J2.4/marked-read | opening from search does not mark read
# J2.4/one-interaction-budget-ms | the thread loads after the budget
# J2.4/attachment-shown | the opened thread lists no attachment, or another file
# J2.4/file-on-desktop | the drag drops no file
# J2.5/arrives-by-control | Esc does not return to the results
# J2.5/folder-in-sidebar | the folder is saved but not shown
# J2.5/folder-lists-same-threads | the folder rule drops threads the search found
# J2.5/smart-folder-in-core | the folder lives in the view only
# J2.5/duplicate-name-refused | a second folder with the same name is saved silently
# J2.6/arrives-by-control | Edit opens nothing
# J2.6/rule-in-words | the editor shows another rule than the saved folder's, or shows it twice
# J2.6/nested-under-work | nesting is not saved
# J2.6/sidebar-shows-nesting | the sidebar keeps the folder at the top level
# J2.6/rule-edited | the rule edit is discarded
# J2.6/count-updates | the folder count does not update
# J2.7/arrives-by-control | the sender name is not clickable
# J2.7/sender-messages-newest-first | the list is oldest first
# J2.7/each-row-has-account | rows omit the account
# J2.7/sender-is-linea | the contact page shows another sender
# J2.8/bar-holds-exactly-the-query | select-all does nothing, the new words are typed after the old ones
# J2.8/photo-message-ranks-first | image attachments are not read, only the subject and body
# J2.8/photo-attachment-named-in-the-row | the row does not name the attachment
# J2.8/photo-found-by-what-it-shows | the scene labels are not in the index, only the file name
# J2.8/not-downloaded-row-says-so | a not-downloaded attachment shows no note
# J2.8/unreadable-kind-searched-by-name-and-says-so | an unreadable kind is dropped from the search


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
    from lib import loadgate                      # round 8 of perf-archive: the shared load rule, through this journey's own wrapper
    from lib.harness import verdict_cases
    for name, ok in loadgate.wrapper_cases(lambda v, b: under_load(v, b), const("INTERACTION_COMMIT_MS"), Unreadable, ID + " under_load") + [(ID + " verdict: " + n, o) for n, o in verdict_cases()]:
        print("%-4s %s" % ("ok" if ok else "BAD", name)); bad += not ok
    print("%s selftest: %d checks, %d mutants, %d not red or missing" % (ID, r["checks"], len(declared), bad))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else 0)
