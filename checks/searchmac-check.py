#!/usr/bin/env python3
"""searchmac functional check: the Mac search screen, driven through the control socket.

    python3 checks/searchmac-check.py [--dry]     exit 0 only when every check passed and none was not judged
    python3 checks/searchmac-check.py --selftest  (no app: the dry structure and the helpers on stand-ins)

Covers the searchmac lane's acceptance (typed filter words become removable chips; Enter on a result opens its
thread) plus the failure paths and adversarial inputs of JOURNEYS J2.1-J2.5. Written from docs/JOURNEYS.md,
docs/CONTROL-SOCKET.md and fixtures/tests-j2.json, never from a built screen. Accessibility ids not yet in
docs/CONTROL-SOCKET.md are ASSUMPTIONS, listed in docs/lanes/tests-j2.md.
Step ids reuse the J2 step they cover so the shared harness reads the MUTANTS block; each label names one check.
"""
import json
import os
import re
import sys
import time
import types

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey  # noqa: E402
from lib.ctl import Unreadable  # noqa: E402
from lib.fixture import const  # noqa: E402

CORPUS = json.load(open(os.path.join(ROOT, "fixtures", "tests-j2.json"), encoding="utf-8"))
EXP = CORPUS["expect"]
MSGS = CORPUS["messages"]
RES, CHIPS, BAR = "mac-search.results", "mac-search.chips", "mac-search.bar"


def threads(from_name=None, attach=False, before=None):
    return sorted({m["thread"] for m in MSGS
                   if (not from_name or from_name.lower() in m["from_name"].lower())
                   and (not attach or m["attachments"]) and (not before or m["date"][:10] < before)})


# The dropdown rows the app serves are `mac-search.dropdown` (SearchMacRows.swift:19): kind (search, from, to, label, folder, subject, attachment, filter, recent), title, detail (the address on a From and a To row),
# account (always null now), highlighted, hovered, keyed, chip (the pill kind a Tab makes), group (a number: 0 search, 1 the rest, 2 recents).
# The ids the check read before (suggestions, suggestions.foot, suggestion-highlight, motion) are served by no file under Sources; the cases that need
# a foot, a slide or a fade are not judged, naming the row they looked at and what the app serves instead.
SUG, SKEL = "mac-search.dropdown", "mac-search.skeleton"
FOOT, HIGH, MOTION = "mac-search.suggestions.foot", "mac-search.suggestion-highlight", "mac-search.motion"
CLEAR = "mac-search.bar.clear"
CORPUS_PATH = os.path.join(ROOT, "fixtures", "tests-j2.json")
CATALOGUE = os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings")
# the dropdown's groups by row kind (SearchSuggest.swift): the word order is EXP["dropdown_group_order"]
GROUP_OF = {"search": "Search", "from": "People", "to": "People", "label": "Labels", "folder": "Labels", "subject": "Filter words",
            "attachment": "Filter words", "filter": "Filter words", "recent": "Recent"}


def catalogue_value(key, lang, catalogue=None):
    """The catalogue's sentence for `key` in `lang` (the copy speaks the language it was started in; a check never pins English)."""
    cat = catalogue if catalogue is not None else json.load(open(CATALOGUE, encoding="utf-8"))["strings"]
    v = cat.get(key, {}).get("localizations", {}).get(lang, {}).get("stringUnit", {}).get("value")
    if v is None:
        raise Unreadable("the catalogue has no %r in %r" % (key, lang))
    return v


def mac_languages():
    """The Mac's preferred languages, first choice first (`defaults read -g AppleLanguages`, a global preference, never the app's own data)."""
    import subprocess
    try:
        out = subprocess.run(["defaults", "read", "-g", "AppleLanguages"], capture_output=True, text=True, timeout=10).stdout
    except (OSError, subprocess.SubprocessError):
        return []
    return re.findall(r'"?([A-Za-z]{2,3}(?:-[A-Za-z0-9]+)*)"?\s*[,)]?\s*$', out, re.M)


def catalogue_column(codes, catalogue=None):
    """The first of `codes` the catalogue has a column for: the code itself (`pt-BR`), else its language (`fr-BE` -> `fr`)."""
    cat = catalogue if catalogue is not None else json.load(open(CATALOGUE, encoding="utf-8"))["strings"]
    have = {l for v in cat.values() for l in v.get("localizations", {})}
    for code in codes:
        for c in (code, code.split("-")[0]):
            if c in have:
                return c
    return None


def app_language(s, langs=mac_languages):
    """The language the copy speaks, from the Settings row the app serves (`language`). `system` (the copy follows the Mac) is read from the Mac's
    own preferred languages, the first the catalogue has a column for; with no such column it is not judged."""
    code = s.ctl.rows("mac-settings-general.settings")[0].get("language")
    if code in (None, ""):
        raise Unreadable("the Settings row serves no language")
    if code == "system":
        col = catalogue_column(langs())
        if col is None:
            raise Unreadable("the copy's language is 'system' and none of the Mac's languages %r has a catalogue column" % (langs(),))
        return col
    return code


def fragment(sentence):
    """The first literal run of a catalogue sentence that has a placeholder (`%@`, `%lld`), without quote marks: the words the bar must carry."""
    for piece in re.split(r"%(?:\d+\$)?(?:@|lld|d|s)", sentence):
        piece = piece.strip(" \u201c\u201d\"'\u00ab\u00bb,.:;")
        if len(piece) >= 8:
            return piece
    raise Unreadable("no literal run in %r" % sentence)


def words(s, key):
    return catalogue_value(key, app_language(s))


def check(s, name, got_fn, want, ctx=None):
    """`expect` where the expected value may be a function (read at run time, for words the copy speaks) and a failure line may carry the values that decide it."""
    def run():
        got = got_fn()
        w = want() if callable(want) else want
        s._eg = (w, got)
        ok = got == w
        return ok, "expected %r, got %r" % (w, got) + ("" if ok or ctx is None else " | " + ctx())
    s._record(name, run)


def unserved(s, ax, what):
    """Read an id no file under Sources serves: the read says so with what the app serves instead, so a miss is never read as a wrong value."""
    try:
        rows = s.ctl.rows(ax)
    except Unreadable as e:
        raise Unreadable("%s; %s" % (e, what))
    if not rows:
        raise Unreadable("no rows at %s; %s" % (ax, what))
    return rows


NOT_UNDERSTOOD = "\u201c%@\u201d not understood, searched as words"
NO_EXACT = "no exact match for \u201c%@\u201d, showing close meaning"


def settle():
    time.sleep(0.25)


def enter_unless_running(ctl, text):
    """Return launches the typed words, EXCEPT when the field already holds the words that run: then Return opens the first result (SearchMacModel.fieldMode
    is `!launched || text != launchedText`; SearchMacActs.enter goes to openResult otherwise). A case that retypes the words the screen already ran
    must not press Return, or it lands in the reader, not in the results."""
    try:
        bar = ctl.rows(BAR)[0]
    except Exception:                                    # a stand-in that serves no bar row: the plain Return
        bar = {}
    if not (bar.get("launched") is True and bar.get("dropdown") is False and bar.get("text") == text):
        ctl.key("Enter")


def query(s, text):
    """Replace the bar's text (select all, then type) and launch it with Enter (S1: results come on Enter, not per keystroke)."""
    s.click(BAR)
    s.key("⌘A")
    s.type(text)
    if text.strip():
        s._act(lambda: enter_unless_running(s.ctl, text))
        s._act(settle)


def until(fn, secs=3.0):
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


def press(s, chord):
    """A key the screen refuses is a wrong behaviour to judge, not a read that cannot be made."""
    c = s.ctl                      # raises Unreadable with no socket: not judged, never a false red
    try:
        c.key(chord)
        return True
    except Exception:
        return False


def typed(s, text):
    """Replace the bar's text WITHOUT Enter: the dropdown shows."""
    s.click(BAR)
    s.key("⌘A")
    s.type(text)


def has(rows, *words):
    return [r for r in rows if all(w in str(r) for w in words)]


def groups_of(s):
    """The dropdown's group words in order of appearance, from the rows' kinds (the served `group` is a number, not a word)."""
    out = []
    for r in s.ctl.rows(SUG):
        g = GROUP_OF[r["kind"]]
        if g not in out:
            out.append(g)
    return out


def dropdown_ctx(s):
    return "dropdown rows %r; bar %r" % ([(r["kind"], r["title"], r["highlighted"]) for r in s.ctl.rows(SUG)], s.ctl.rows(BAR)[0]["text"])


def query_raw(s, text):
    s.ctl.click(BAR); s.ctl.key("⌘A"); s.ctl.type(text); enter_unless_running(s.ctl, text)


def corpus_threads(pred):
    return sorted({m["thread"] for m in MSGS if pred(m)})

ESC_CASE = "esc-clears-the-words-and-leaves-the-search"
ESC_WANT = (True, False, "", "mac-main")     # (the dropdown was open, the dropdown is closed, the bar's words, the screen) after ONE Escape


def esc_once(s):
    """The owner's ruling (SearchMacActs.close): one press of Escape clears the words, closes the dropdown and leaves the search for the inbox."""
    typed(s, "price")
    before = len(s.ctl.rows(SUG))                         # positive control: the dropdown WAS open
    s.ctl.key("Escape")
    b = s.ctl.rows(BAR)[0]
    return (before > 0, b["dropdown"], b["text"], s.ctl.uistate().get("surface"))


def run_text(ctl, text):
    """Replace the field's text and launch it with Return (unless those words already run)."""
    ctl.click(BAR); ctl.key("\u2318A"); ctl.type(text)
    if text.strip():
        enter_unless_running(ctl, text)


def pill_pairs(chip_rows):
    return sorted((r["kind"], r["value"] if r["kind"] != "attachment" else "") for r in chip_rows)


def typed_pill_bad(ctl):
    """Each typed case of the corpus (`typed_cases`: the field's reading, not the phone's spoken one in `filter_cases`): the pills it makes and their values."""
    bad = []
    for c in CORPUS["typed_cases"]:
        run_text(ctl, c["text"])
        got = pill_pairs(ctl.rows(CHIPS))
        want = sorted((x["kind"], x.get("value", "") if x["kind"] != "attachment" else "") for x in c["chips"])
        if got != want:
            bad.append("%r: pills %r, wanted %r" % (c["text"], got, want))
    return bad


def unknown_words(ctl):
    """A word or a colon form the screen does not read as a filter stays words: no pill, and the engine is asked exactly the typed text."""
    out = []
    for text in ("before Junetember", "before:Junetember"):
        run_text(ctl, text)
        out.append(([r["kind"] for r in ctl.rows(CHIPS)], ctl.rows(BAR)[0]["text"] == text, ctl.core("search/last/query") == text))
    return out


def plain_from_words(ctl):
    run_text(ctl, "from Linea Supply")
    return ([r["kind"] for r in ctl.rows(CHIPS)], ctl.rows(BAR)[0]["text"], ctl.core("search/last/query"))


def tab_pill(ctl, kind, text):
    """Down to the row of this kind (the served `highlighted` marks the row the keys are on), Tab; returns the pills, the narrowed threads and the row Tab was pressed on."""
    ctl.click(BAR); ctl.key("\u2318A"); ctl.type(text)
    for _ in range(10):
        if any(r["highlighted"] and r["kind"] == kind for r in ctl.rows(SUG)):
            break
        try:
            ctl.key("Down")
        except Exception:
            break
    row = [r for r in ctl.rows(SUG) if r["highlighted"] and r["kind"] == kind][:1]
    try:
        ctl.key("Tab")
    except Exception:
        pass
    return [(r["kind"], r["value"]) for r in ctl.rows(CHIPS)], sorted({r["thread"] for r in ctl.rows(RES)}), row


def from_pill_result(got):
    chips, thr_, row = got
    return (chips, thr_) == ([("from", row[0]["detail"])], corpus_threads(lambda m: m["from"] == row[0]["detail"])) if row else "no From row was highlighted"


def subject_pill_result(got):
    chips, thr_, row = got
    return (chips, thr_) == ([("subject", row[0]["title"])], corpus_threads(lambda m: row[0]["title"].lower() in m["subject"].lower())) if row else "no Subject row was highlighted"


def drive(j):
    ids = lambda s: [r["id"] for r in s.ctl.rows(RES)]
    thr = lambda s: sorted(r["thread"] for r in s.ctl.rows(RES))
    kinds = lambda s: sorted(r["kind"] for r in s.ctl.rows(CHIPS))
    says = lambda s: s.ctl.rows(BAR)[0]["says"]

    s = j.step("J2.1", "Garbage in the bar")
    # The corpus the expectations are computed from must be the corpus the app searches (journeys/J2.py:178): without this the app serves its default corpus
    s.raw("fixture", "corpus", CORPUS_PATH)
    s.arrive(("key", "/"), "mac-search")
    s.expect("empty-query-lists-nothing", lambda: (len(s.ctl.rows(RES)), (query(s, EXP["meaning_query"]) or True) and len(s.ctl.rows(RES)) > 0,
                                                    (s.click(CLEAR) or True) and len(s.ctl.rows(RES))), (0, True, 0))

    def battery():
        bad = []
        for case in CORPUS["adversarial_queries"]:
            text = case["repeat"] * case["times"] if "repeat" in case else case["text"]
            query(s, text)
            try:
                if s.ctl.rows(BAR)[0]["text"] != text:      # the garbage must have reached the bar, or the case proves nothing
                    bad.append("%s: never reached the bar (bar text %r)" % (case["name"], s.ctl.rows(BAR)[0]["text"][:40])); continue
                surface = s.ctl.uistate().get("surface")
                alive = surface == "mac-search" and isinstance(s.ctl.rows(RES), list)
                why = "surface %r" % surface
            except Exception as e:                      # a read that fails after garbage is a failed case, not a crash of the check
                alive, why = False, "read failed: %s" % type(e).__name__
            if not alive:
                bad.append("%s: %s" % (case["name"], why))
        query(s, EXP["meaning_query"])
        if s.ctl.rows(BAR)[0]["text"] != EXP["meaning_query"] or EXP["record1_thread"] not in [r["thread"] for r in s.ctl.rows(RES)]:
            bad.append("CONTROL: the plain query no longer finds record 1 after the garbage (bar %r, threads %r)" % (s.ctl.rows(BAR)[0]["text"], [r["thread"] for r in s.ctl.rows(RES)][:6]))
        return bad
    s.expect("adversarial-queries-survive", battery, [])

    s = j.step("J2.2", "Filters: colon forms are pills, typed words are words (SPEC A142, A143)")
    s.expect("each-typed-form-makes-its-pills-and-words-make-none", lambda: typed_pill_bad(s.ctl), [])
    s.expect("unknown-filter-stays-words-and-the-engine-gets-the-words", lambda: unknown_words(s.ctl), [([], True, True)] * 2)
    s.expect("from-colon-form-applies", lambda: (query(s, 'from:"Linea Supply"') or True) and thr(s), threads("Linea Supply"))
    s.expect("plain-from-words-make-no-pill", lambda: plain_from_words(s.ctl), ([], "from Linea Supply", "from Linea Supply"))
    s.expect("attachment-colon-form-applies", lambda: (query(s, "has:attachment") or True) and thr(s), threads(attach=True))
    s.expect("before-colon-form-applies", lambda: (query(s, "before:" + CORPUS["before_june"]) or True) and thr(s), threads(before=CORPUS["before_june"]))
    def after_form():
        query(s, "after:" + CORPUS["before_june"])
        got = sorted(r["thread"] for r in s.ctl.rows(RES))
        return (kinds(s), got == corpus_threads(lambda m: m["date"][:10] >= CORPUS["before_june"]))
    s.expect("after-colon-form-applies", after_form, (["after"], True))

    s = j.step("J2.3", "Exact figure failure paths")
    query(s, EXP["no_exact_query"] + " " + EXP["meaning_query"])
    check(s, "no-exact-match-says-so-keeps-meaning", lambda: (fragment(words(s, NO_EXACT)) in says(s), EXP["record1_thread"] in [r["thread"] for r in s.ctl.rows(RES)]), (True, True),
          lambda: "bar says %r; wanted a run of %r; threads %r" % (says(s), words(s, NO_EXACT), [r["thread"] for r in s.ctl.rows(RES)][:6]))
    query(s, '"84 EUR"')
    check(s, "partial-figure-is-not-exact", lambda: (fragment(words(s, NO_EXACT)) in says(s), [r["id"] for r in s.ctl.rows(RES) if r["highlights"]], s.ctl.rows(BAR)[0]["text"]),
          (True, [], '"84 EUR"'), lambda: "bar says %r; rows with highlights %r" % (says(s), [(r["id"], r["highlights"]) for r in s.ctl.rows(RES) if r["highlights"]]))

    s = j.step("J2.4", "Enter opens exactly the result")
    query(s, EXP["meaning_query"])
    first = []
    s._act(lambda: first.append((s.ctl.rows(RES) or [{"id": None}])[0]["id"]))
    s.arrive(("key", "Enter"), "mac-reader")     # the picked three-pane design opens a thread on mac-reader (J1.6, J2.4; lead fix 2026-11-08)
    s.expect("enter-opens-the-result", lambda: (s.ctl.rows("mac-main.reader.subject")[0]["id"], first[0]), (first[0] if first else None,) * 2)

    s = j.step("J2.5", "Smart folder from the results")
    s.arrive(("key", "Escape"), "mac-search")
    query(s, EXP["mac_filter_text"])
    s.click("mac-search.save-as-smart-folder")
    s.key("Enter")                                    # an empty name
    check(s, "empty-name-refused", lambda: ("Supplier quotes" in [r["title"] for r in s.ctl.rows("mac-search.sidebar.smart-folders")],
                                            len(s.ctl.rows("mac-search.sidebar.smart-folders")), bool(s.ctl.rows("mac-search.save-as-smart-folder.field")[0]["says"])), (False, 0, True),
          lambda: "smart folders %r; name field %r" % ([r["title"] for r in s.ctl.rows("mac-search.sidebar.smart-folders")], s.ctl.rows("mac-search.save-as-smart-folder.field")))
    s.type("Supplier quotes")
    s.key("Enter")
    s.expect("folder-count-equals-result-count", lambda: [r["count"] for r in s.ctl.rows("mac-search.sidebar.smart-folders") if r["title"] == "Supplier quotes"],
             [len(threads("Linea Supply", True, CORPUS["before_june"]))])

    # ---------------- S1 Quiet rows (round 3) ----------------
    s = j.step("J2.1", "The dropdown as you type")
    typed(s, EXP["label_query"])
    # the kind says "folder" for a smart folder (its detail word is the language's, not a key to compare)
    check(s, "dropdown-labels-and-smart-folders-listed", lambda: [(r["title"], r["kind"]) for r in s.ctl.rows(SUG) if r["kind"] in ("label", "folder")
                                                                  and r["title"] == EXP["label_title"]], [(EXP["label_title"], "folder")], lambda: dropdown_ctx(s))
    check(s, "dropdown-shows-only-what-matches", lambda: [r["title"] for r in s.ctl.rows(SUG) if r["kind"] in ("from", "to")], [], lambda: dropdown_ctx(s))
    typed(s, EXP["people_query"].split(" ", 1)[1])      # positive control for the line above: letters that name a person DO list From rows (the engine's people, the address at the right)
    check(s, "dropdown-people-row-has-name-and-address", lambda: sorted((r["title"], r["detail"]) for r in s.ctl.rows(SUG) if r["kind"] == "from"),
          sorted((EXP["people_chip_value"], a) for a in EXP["people_addresses"]), lambda: dropdown_ctx(s))
    typed(s, EXP["filter_words_query"])
    NO_FOOT = "the app serves the dropdown as rows of mac-search.dropdown (kind, title, detail, account, highlighted, hovered, keyed, chip, group) and no foot row"
    s.expect("dropdown-foot-names-the-keys", lambda: [w in " ".join(r["text"] for r in unserved(s, FOOT, NO_FOOT)) for w in EXP["dropdown_foot_words"]]
             + [s.ctl.rows(BAR)[0]["text"] == EXP["filter_words_query"]],
             [True] * (len(EXP["dropdown_foot_words"]) + 1))
    NO_MOTION = "the app serves the highlight only as `highlighted` on the mac-search.dropdown rows (now at index %s): no row carries a slide or fade, a from-index, a duration or a motion constant"
    def moves():
        unserved(s, HIGH, NO_MOTION % [i for i, r in enumerate(s.ctl.rows(SUG)) if r["highlighted"]])
        seq, bad, seen = ["Down", "Down", "Up", "Up", "Up"], [], set()
        for k in seq:
            if not press(s, k):
                bad.append("%s refused" % k); continue
            h = s.ctl.rows(HIGH)[0]
            dist = abs(h["index"] - h["from"])
            want = "slide" if dist <= const("SEARCH_SLIDE_MAX_ROWS") else "fade"
            seen.add(want)
            if h["motion"] != want or h["ms"] > const("MOTION_MAX_MS"):
                bad.append("%s: %d->%d motion %s %sms (want %s)" % (k, h["from"], h["index"], h["motion"], h["ms"], want))
        if seen != {"slide", "fade"}:                       # positive control: the sequence really made one near and one far move
            bad.append("CONTROL: saw only %s" % sorted(seen))
        return bad
    s.expect("highlight-slides-when-near-and-fades-when-far", moves, [])
    s.expect("motion-declares-the-constants", lambda: {r["name"]: r["ms"] for r in unserved(s, MOTION, NO_MOTION % "?")},
             {"dropdown-enter": const("SEARCH_DROPDOWN_ENTER_MS"), "skeleton-crossfade": const("SEARCH_SKELETON_FADE_MS"),
              "hover-tint": const("SEARCH_HOVER_TINT_MS"), "slide-max-rows": const("SEARCH_SLIDE_MAX_ROWS")})
    s.expect("motion-stays-under-the-motion-ceiling", lambda: [r["name"] for r in unserved(s, MOTION, NO_MOTION % "?") if r["name"] != "slide-max-rows" and r["ms"] > const("MOTION_MAX_MS")], [])

    s = j.step("J2.2", "Tab, Esc and the recent searches")
    def tab_row(kind, text):
        """Down to the row of this kind, Tab, then the chips and the narrowed results (the attachment case's shape)."""
        chips, thr_, _ = tab_pill(s.ctl, kind, text)
        return chips, thr_
    # the chip kinds are the ones the app's chips carry (SearchQuery.swift:7): from, subject, attname (the check's old "sender-contains" names are no chip kind)
    # the From row is a person (name, the address at the right); its pill carries the ADDRESS (it names one person) and keeps that address's mail
    check(s, "tab-on-a-from-row-adds-the-pill-with-the-address", lambda: from_pill_result(tab_pill(s.ctl, "from", "supply")), True, lambda: dropdown_ctx(s))
    # the Subject row's title (a subject of a mail on screen that holds the typed words, else the typed words) is the pill's value
    check(s, "tab-on-the-subject-row-adds-that-pill", lambda: subject_pill_result(tab_pill(s.ctl, "subject", "brochure")), True, lambda: dropdown_ctx(s))
    check(s, "tab-on-attachment-name-contains-adds-that-chip", lambda: tab_row("attachment", "warehouse"),
          ([("attname", "warehouse")], corpus_threads(lambda m: any("warehouse" in a.lower() for a in m["attachments"]))), lambda: dropdown_ctx(s))
    check(s, ESC_CASE, lambda: esc_once(s), ESC_WANT,
          lambda: "after one Escape: surface %r, bar %r, dropdown open %r" % (s.ctl.uistate().get("surface"), s.ctl.rows(BAR)[0]["text"], s.ctl.rows(BAR)[0]["dropdown"]))
    def esc_empty():
        press(s, "/")                                          # the screen is open before the clear: a surface that was never search proves nothing
        s.ctl.click(CLEAR)
        opened = (s.ctl.uistate().get("surface"), s.ctl.rows(BAR)[0]["text"])
        press(s, "Escape")
        return opened, s.ctl.uistate().get("surface")
    check(s, "esc-with-an-empty-field-leaves-search", esc_empty, (("mac-search", ""), "mac-main"),
          lambda: "bar row %r; state of the keys: focus %r" % (s.ctl.rows(BAR)[0], s.ctl.uistate().get("focus")))
    s.arrive(("key", "/"), "mac-search")
    flood = EXP["recent_flood_queries"]
    for qq in flood + [flood[2]]:                               # ten searches, then the third again
        query(s, qq)
    cap = const("SEARCH_RECENT_MAX")
    want = ([flood[2]] + [x for x in reversed(flood) if x != flood[2]])[:cap]
    def recents():
        s.ctl.click(CLEAR)
        return [r["title"] for r in s.ctl.rows(SUG) if r["kind"] == "recent"]
    s.expect("recent-rows-newest-first-capped-and-deduplicated", recents, want)
    s.expect("recent-only-when-the-field-is-empty", lambda: groups_of(s), ["Recent"])

    s = j.step("J2.1", "Skeleton rows and the results anatomy")
    def skeleton_at(w, h):
        s.ctl.send("resize", str(w), str(h))
        s.ctl.click("mac-search.skeleton.hold")                # the screen's own hold (journeys/J2.py:178 area): no `fixture search-delay` verb is served
        try:
            query_raw(s, EXP["meaning_query"])
            sk = s.ctl.rows(SKEL)
            fh = s.ctl.measure(SKEL)["frame"]["h"]
        finally:
            s.ctl.click("mac-search.skeleton.release")
        until(lambda: len(s.ctl.rows(RES)) > 0 and not s.ctl.rows(SKEL)[0]["loading"], 4)
        if len(sk) <= 1:
            raise Unreadable("the app serves one skeleton row ({loading, held}), not a row per skeleton line: %d row(s) %r, frame height %s at %dx%d (LIST_ROW_PX %s)"
                             % (len(sk), sk, fh, w, h, const("LIST_ROW_PX")))
        return len(sk), fh
    def fit():
        wide, narrow = skeleton_at(1280, 820), skeleton_at(EXP["narrow_window"]["w"], EXP["narrow_window"]["h"])
        px = const("LIST_ROW_PX")
        return [n > 0 and n * px <= h < (n + 1) * px for n, h in (wide, narrow)] + [wide[0] > narrow[0]]
    s.expect("skeleton-count-equals-the-rows-that-fit-at-two-sizes", fit, [True, True, True])
    def fade():
        rows = s.ctl.rows(RES)
        return sorted({r["fade_ms"] for r in rows if "fade_ms" in r}) , len(rows) > 0
    check(s, "each-result-crossfades-within-the-fade-constant", fade, ([const("SEARCH_SKELETON_FADE_MS")], True),
          lambda: "result rows %d; keys of the first row %r (no fade_ms among them means the app serves no fade)" % (len(s.ctl.rows(RES)), sorted((s.ctl.rows(RES) or [{}])[0])))

    s = j.step("J2.1", "Narrow window truncation")
    def trunc(w, h):
        s.ctl.send("resize", str(w), str(h))
        query_raw(s, EXP["meaning_query"])
        r = next(x for x in until(lambda: s.ctl.rows(RES)) if x["id"] == EXP["record1_message"])
        miss = [k for k in ("subject_truncated", "attachment_truncated") if k not in r]
        if miss:       # the failure line carries what decides: the app serves no truncation flag on its results rows
            return "no %s in the row; keys served %s at %dx%d" % (", ".join(miss), sorted(r), w, h)
        return r["subject_truncated"], r["attachment_truncated"]
    s.expect("wide-window-truncates-nothing-on-record-one", lambda: trunc(1280, 820), (False, False))
    s.expect("narrow-window-truncates-the-subject-first-attachment-name-whole", lambda: trunc(EXP["narrow_window"]["w"], EXP["narrow_window"]["h"]), (True, False))
    s.raw("resize", "1280", "820")


MUTANTS = """# MUTANTS
# J2.1/arrives-by-control | the search key opens nothing
# J2.1/empty-query-lists-nothing | an empty bar lists every message
# J2.1/adversarial-queries-survive | an unterminated quote throws the screen back to the inbox
# J2.2/each-typed-form-makes-its-pills-and-words-make-none | a typed word becomes a pill again ("from" before a word, "has attachment", "before June"), a colon form makes no pill, the same pill typed twice gives two, or a pill carries another value (SearchQuery.typed)
# J2.2/unknown-filter-stays-words-and-the-engine-gets-the-words | a word or a colon form that is no filter is dropped from the query, or the engine is asked something else than the typed text
# J2.2/from-colon-form-applies | the from pill is shown but not applied
# J2.2/plain-from-words-make-no-pill | "from Linea Supply" typed as words makes a sender pill (the old `from <word>` branch of SearchQuery.parse back), or the launched query is not the field's text
# J2.2/attachment-colon-form-applies | the has:attachment pill is shown but not applied
# J2.2/before-colon-form-applies | the before pill is shown but not applied
# J2.2/after-colon-form-applies | the after pill is read as before, or not applied (the mail of the day itself is kept, the day before is not)
# J2.3/no-exact-match-says-so-keeps-meaning | a quoted figure with no hit says nothing
# J2.3/partial-figure-is-not-exact | a quoted partial figure matches inside a longer figure
# J2.4/arrives-by-control | Enter on a result opens nothing
# J2.4/enter-opens-the-result | Enter opens another thread than the first result
# J2.5/arrives-by-control | Esc does not return to the results
# J2.5/empty-name-refused | an empty folder name is accepted
# J2.5/folder-count-equals-result-count | the folder count is off by one
# J2.1/dropdown-labels-and-smart-folders-listed | smart folders are left out of the Labels rows
# J2.1/dropdown-shows-only-what-matches | the dropdown lists every person whatever is typed
# J2.1/dropdown-people-row-has-name-and-address | the From row shows the name but no address, or lists one address of the supplier only
# J2.1/dropdown-foot-names-the-keys | the dropdown has no foot hint
# J2.1/highlight-slides-when-near-and-fades-when-far | the highlight always slides, even across the whole list
# J2.1/motion-declares-the-constants | the skeleton crossfade runs 300 ms
# J2.1/motion-stays-under-the-motion-ceiling | the dropdown enter runs past the motion ceiling
# J2.2/arrives-by-control | the search key opens nothing after Esc left search
# J2.2/tab-on-a-from-row-adds-the-pill-with-the-address | Tab on a From row adds the person's name as the pill (the pill must name one address), or a subject pill
# J2.2/tab-on-the-subject-row-adds-that-pill | Tab on the Subject row makes a pill of other words than the row's title, or matches the body too
# J2.2/tab-on-attachment-name-contains-adds-that-chip | Tab on Attachment name contains matches nothing
# J2.2/esc-clears-the-words-and-leaves-the-search | one Esc closes the dropdown only: the words stay in the bar and the search screen stays open
# J2.2/esc-with-an-empty-field-leaves-search | Esc with an empty field does nothing
# J2.2/recent-rows-newest-first-capped-and-deduplicated | recent searches are kept without a cap
# J2.2/recent-only-when-the-field-is-empty | an empty field also lists the Search all mail row
# J2.1/skeleton-count-equals-the-rows-that-fit-at-two-sizes | the skeleton keeps its count when the window shrinks
# J2.1/each-result-crossfades-within-the-fade-constant | a result fades in over 400 ms
# J2.1/wide-window-truncates-nothing-on-record-one | the subject truncates even in a wide window
# J2.1/narrow-window-truncates-the-subject-first-attachment-name-whole | the attachment name truncates before the subject
"""


def selftest():
    """No app: the dry structure and the helpers on stand-ins, each with a mutant. Exit 0 when all hold."""
    bad = []

    def report(ok, what):
        print(("ok   " if ok else "FAIL ") + what)
        if not ok:
            bad.append(what)

    mod = types.SimpleNamespace(ID="searchmac-check", TITLE="selftest", TARGET="LOCAL", VISIBLE_OUTCOME="", drive=drive)
    path = os.path.join(ROOT, "_captures", ".tmp", "searchmac-mutants.txt")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    open(path, "w", encoding="utf-8").write(MUTANTS)
    r = run_journey(mod, path, force_dry=True)
    report(r["failed"] == 0 and not r["no_mutant"] and not r["stray_mutant"] and r["checks"] > 0,
           "dry run: %d checks, %d failed, no check without a mutant, no stray mutant" % (r["checks"], r["failed"]))
    en, fr = catalogue_value(NOT_UNDERSTOOD, "en"), catalogue_value(NOT_UNDERSTOOD, "fr")
    report(fragment(en) == "not understood, searched as words" and fragment(fr) == "non compris, recherch\u00e9 comme mots", "the bar's words are the catalogue's, per language (%r, %r)" % (fragment(en), fragment(fr)))
    report(fragment(en) not in fr, "mutant: the English words are not found in a French bar")
    report(fragment(catalogue_value(NO_EXACT, "en")) == "no exact match for", "the no-exact words come from the catalogue (%r)" % fragment(catalogue_value(NO_EXACT, "en")))
    stub = lambda lang: types.SimpleNamespace(ctl=types.SimpleNamespace(rows=lambda ax: [{"language": lang}]))
    report(app_language(stub("fr")) == "fr", "the language is read from the Settings row")
    report(app_language(stub("system"), langs=lambda: ["fr-BE", "en-BE"]) == "fr", "language 'system' is the Mac's first language with a catalogue column (fr-BE gives fr)")
    report(app_language(stub("system"), langs=lambda: ["xx-YY", "en-US"]) == "en", "language 'system': a language the catalogue lacks is passed over (en-US gives en)")
    try:
        app_language(stub("system"), langs=lambda: ["xx-YY"])
        report(False, "language 'system' with no catalogue column is not judged (it answered)")
    except Unreadable:
        report(True, "language 'system' with no catalogue column is not judged")
    seen = {}
    rec = types.SimpleNamespace(_eg=None, _record=lambda name, fn: seen.__setitem__(name, fn()))
    check(rec, "same", lambda: [1], lambda: [1], lambda: "ctx")
    check(rec, "other", lambda: [1], [2], lambda: "ctx values")
    report(seen["same"][0] is True and seen["other"][0] is False and seen["other"][1] == "expected [2], got [1] | ctx values", "check passes equal values and fails different ones with the deciding values in the line")
    none = types.SimpleNamespace(ctl=types.SimpleNamespace(rows=lambda ax: []))
    try:
        unserved(none, FOOT, "what is served")
        report(False, "an id no row serves is not judged (it answered)")
    except Unreadable as e:
        report("what is served" in str(e) and FOOT in str(e), "an id no row serves is not judged and names what the app serves (%s)" % str(e)[:60])
    rows = [{"kind": "from"}, {"kind": "to"}, {"kind": "folder"}, {"kind": "filter"}, {"kind": "recent"}]
    report([GROUP_OF[r["kind"]] for r in rows] == ["People", "People", "Labels", "Filter words", "Recent"] and set(GROUP_OF.values()) == set(EXP["dropdown_group_order"]),
           "every dropdown kind maps to one of the fixture's group words")
    class _Search:                                          # a stand-in search screen: `keeps` is the mutant where Escape keeps the words (the owner's ruling is one press that clears and leaves)
        def __init__(self, keeps):
            self.keeps, self.text, self.surface, self.dropdown = keeps, "", "mac-search", False
            self.ctl = self
        def click(self, ax): pass
        def type(self, t): self.text, self.dropdown = t, True
        def key(self, k):
            if k == "Escape":
                self.dropdown = False
                if not self.keeps:
                    self.text, self.surface = "", "mac-main"
        def rows(self, ax): return [{"kind": "recent"}] * 3 if ax == SUG and self.dropdown else ([] if ax == SUG else [{"text": self.text, "dropdown": self.dropdown}])
        def uistate(self): return {"surface": self.surface}
    report(esc_once(_Search(False)) == ESC_WANT, "one Escape clears the words and leaves the search on the clean stand-in")
    report(esc_once(_Search(True)) != ESC_WANT, "mutant: an Escape that keeps the words fails the case (%r)" % (esc_once(_Search(True)),))
    # the changed cases on the search-checks stand-in of checks/parity/par-searchmac.py (the test engine's behaviour under SPEC A141 to A143), green and one mutant each
    import importlib.util
    spec = importlib.util.spec_from_file_location("par_searchmac", os.path.join(ROOT, "checks", "parity", "par-searchmac.py"))
    par = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(par)
    def on(mutant, fn, *a):
        m = par.ScreenModel(mutant)
        run_text(m, "quote")                                # a search was launched before: a settled pill narrows it at once
        return fn(m, *a)
    report(on(None, typed_pill_bad) == [], "typed cases: every colon form makes its pills, words make none, on the clean stand-in")
    with par.eager_parse():
        eager_bad = on(None, typed_pill_bad)
    report(eager_bad != [], "mutant: words that become pills turn each-typed-form-makes-its-pills-and-words-make-none red (%d cases)" % len(eager_bad))
    report(on(None, unknown_words) == [([], True, True)] * 2, "unknown filter words stay words and reach the engine as typed, on the clean stand-in")
    report(on("pill-sent-as-words", unknown_words) == [([], True, True)] * 2, "(control) a stand-in that drops pills from the query still passes the words-only cases")
    report(on(None, plain_from_words) == ([], "from Linea Supply", "from Linea Supply"), "plain from words make no pill, on the clean stand-in")
    base_from = from_pill_result(on(None, tab_pill, "from", "supply"))
    report(base_from is True, "Tab on a From row: the pill carries the address and the list keeps that address's mail, on the clean stand-in (%s)" % (base_from,))
    report(from_pill_result(on("pill-carries-name", tab_pill, "from", "supply")) is not True, "mutant: a pill with the person's name turns tab-on-a-from-row-adds-the-pill-with-the-address red")
    base_subject = subject_pill_result(on(None, tab_pill, "subject", "brochure"))
    report(base_subject is True, "Tab on the Subject row: a subject pill with the row's title, on the clean stand-in (%s)" % (base_subject,))
    report(subject_pill_result(on("subject-pill-other-words", tab_pill, "subject", "brochure")) is not True, "mutant: a subject pill of other words turns tab-on-the-subject-row-adds-that-pill red")
    print("searchmac selftest: %d problems" % len(bad))
    return 1 if bad else 0


def main(argv):
    if "--selftest" in argv:
        return selftest()
    mod = types.SimpleNamespace(ID="searchmac-check", TITLE="Mac search screen", TARGET="LOCAL", VISIBLE_OUTCOME="", drive=drive)
    path = os.path.join(ROOT, "_captures", ".tmp", "searchmac-mutants.txt")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    open(path, "w", encoding="utf-8").write(MUTANTS)
    r = run_journey(mod, path, force_dry="--dry" in argv)
    for line in r["fail_list"]:
        sys.stderr.write("FAIL %s\n" % line)
    for lab in r["no_mutant"]:
        sys.stderr.write("NO MUTANT %s\n" % lab)
    for lab in r["stray_mutant"]:
        sys.stderr.write("STRAY MUTANT %s\n" % lab)
    sys.stderr.write(r["verdict"] + ": " + r["evidence"] + "\n")
    sys.stdout.write(MUTANTS)
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    ok = r["verdict"] == "GREEN" and not r["no_mutant"] and not r["stray_mutant"]
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
