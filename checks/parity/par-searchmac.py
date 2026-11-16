#!/usr/bin/env python3
"""Parity check: par-searchmac (docs/plan.json ix-searchmac). Five features of the Mac search screen, each with one
driven check of its visible result. Expected values come from fixtures/tests-j2.json, never from a built screen.

Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/par-searchmac.py'
(after `fixture corpus fixtures/tests-j2.json`, which the check sends itself).
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0.
`--selftest` judges the round 11 smart-folder checks and the round 12 checks (Enter takes the typed words, chips narrow as typed, the view's rows,
the picker's date words) on a model (green) and on mutants (each red on its own case); it launches nothing.
"""
import inspect
import json
import os
import re
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, print_cases                    # noqa: E402
from lib.fixture import const                                        # noqa: E402
from lib.ctl import Unreadable                                       # noqa: E402

ID = "par-searchmac"
TITLE = "Mac search parity (S1 Quiet rows): dropdown, Tab chip, Enter skeleton, grouped results, attachment named, recents, thread, folder"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "typing opens the dropdown, Tab adds a chip, Enter swaps the list to skeleton rows then results, each control changes what shows and a second action restores it"
CORPUS = json.load(open(os.path.join(ROOT, "fixtures", "tests-j2.json"), encoding="utf-8"))
MSGS = CORPUS["messages"]
EXP = CORPUS["expect"]
ACC = {m["id"]: m["account"] for m in MSGS}
ATT = {m["id"]: m["attachments"] for m in MSGS}
LINEA_ADDR = sorted({m["from"] for m in MSGS if m["from_name"] == "Linea Supply"})
LINEA_THREADS = sorted({m["thread"] for m in MSGS if m["from_name"] == "Linea Supply"})
RES, CHIPS, BAR, DD = "mac-search.results", "mac-search.chips", "mac-search.bar", "mac-search.dropdown"
ALLOWED = {"search", "domain", "from", "to", "label", "folder", "subject", "attachment", "filter", "recent"}   # no action rows, no menus (SearchSuggest.Kind); `domain`: SPEC A153


# round 8 inputs: the photo whose content (not its name, not the subject) holds the word; the fixture's own accounts
CONTENT_QUERY = "pallets"
PHOTO_MSG = CORPUS["attachment_content"][EXP["photo_attachment"]]["message"]
SAMPLE_FILE = json.load(open(os.path.join(ROOT, "docs", "design", "fixtures", "mac-main-sample.json"), encoding="utf-8"))["accounts"]
SAMPLE_ACCOUNTS = [a["id"] for a in SAMPLE_FILE]
COLD_WORD = "quote"   # a plain word of record 1's subject: a text hit with no meaning index at all
# Item 3bb3d817 (lane every-word): a match holds EVERY typed word (search-text query.rs; FixtureSearchBook.holdsEvery). The corpus's meaning query
# ("supplier price quote spring") is held whole by no mail, so the cases designed on its one-word matches (the tinted words, the count line, the evidence,
# the three parts, the ranking line, the first answer from the words) launch WORDS_QUERY: msg-01 (thr-01) and msg-s05 (thr-s05), the two threads the
# corpus names in meaning_query_must_include_threads, hold all three words; thr-s06 holds none and is the mail close in meaning (the `near` table's
# quote and price). Sets: _captures/every-word/match-sets-5.out.
WORDS_QUERY = "quote price run"
GROUP_QUERY = "print run"     # msg-01, msg-01a and msg-01b (thr-01) hold both words: a list of one row per message would show thr-01 three times


def words_tint():
    """The typed words each match thread's subject holds (the tint of its row): read from the fixture file, the mails of the thread that hold every word."""
    words = WORDS_QUERY.split()
    out = {}
    for m in MSGS:
        hay = " ".join([m["subject"], m["body"], m["from_name"], m["from"]] + list(m.get("attachments") or [])).lower()
        if all(w in hay for w in words) and m["thread"] in EXP["meaning_query_must_include_threads"]:
            out[m["thread"]] = sorted({w for w in words if re.search(r"\b%s\b" % re.escape(w), m["subject"].lower())})
    return out


WORDS_TINT = words_tint()   # {"thr-01": ["quote", "run"], "thr-s05": ["run"]}
TWO_BATCHES = os.path.join(ROOT, "_captures", "searchmac", "meaning-two-batches.json")   # 51 messages: two EMBED_BATCH steps


def write_two_batches():
    """The corpus of the re-ask case, made here when it is absent: the J2 corpus plus 40 more mails of new ids, so more than EMBED_BATCH mails have no vector and the
    meaning index builds them in the background (two steps). main92: the file existed in no copy, `fixture corpus` refused it, the J2 corpus (already embedded)
    stayed, no background step ran and no re-ask came. Written under _captures (git ignores it), never under fixtures/."""
    if os.path.exists(TWO_BATCHES):
        return TWO_BATCHES
    base = json.load(open(os.path.join(ROOT, "fixtures", "tests-j2.json"), encoding="utf-8"))
    topics = ["the print run invoice", "paper samples for the brochure", "the delivery week", "the warehouse pallets", "a new quote for flyers",
              "the spring catalogue", "payment terms", "the bank details", "a meeting next Tuesday", "the lake photos"]
    extra = [{"id": "mb-%03d" % n, "thread": "thr-mb-%03d" % n, "account": "acc-04", "from_name": "Batch Sender", "from": "batch%02d@two-batches-example.com" % (n % 7),
              "date": "2026-03-%02dT10:00:00" % (1 + n % 27), "subject": "Note %d about %s" % (n, topics[n % len(topics)]),
              "body": "A short note number %d about %s, written for the second embedding step." % (n, topics[n % len(topics)]), "attachments": [], "role": "two-batches filler"}
             for n in range(40)]
    out = dict(base)
    out["messages"] = base["messages"] + extra
    os.makedirs(os.path.dirname(TWO_BATCHES), exist_ok=True)
    with open(TWO_BATCHES, "w", encoding="utf-8") as f:
        json.dump(out, f, ensure_ascii=False, indent=1)
    return TWO_BATCHES
REASK_WAIT_S = 60   # the model's first load plus two background steps; a check bound, not a product number


def has_const(name):
    try:
        const(name)
        return True
    except Exception:
        return False


INDEXED_FIRST = 14   # `fixture indexing 14`: the first 14 accounts, in the fixture's listed order, are indexed
# round 9 (main46): 14 of 30 indexed leaves 16 not indexed, but the fixture's failing account (status "sign in again") is among
# them and is not loading: it needs the owner to sign in, which the sign-in screen says (SearchIndexing.swift). So 15 are named.
# Read from the fixture file, never from the core or the screen.
LOADING_EXPECTED = len([a for a in SAMPLE_FILE[INDEXED_FIRST:] if not a.get("status")])


def model_installed():
    """Whether the rig can build a meaning index at all: the weights GemmaWeights.locate() would find (env, installer folder,
    Hugging Face cache). Without them MeaningSearch has no builder and `onUpdate` has no source."""
    rev = re.search(r'revision = "([0-9a-f]+)"', open(os.path.join(ROOT, "Sources", "Features", "Embedding", "GemmaWeights.swift"), encoding="utf-8").read())
    rev = rev.group(1) if rev else ""
    dirs = []
    for src in (os.environ.get("WHOOSH_EMBED_WEIGHTS", ""), dict(kv.split("=", 1) for kv in os.environ.get("LAUNCH_ENV", "").split() if "=" in kv).get("WHOOSH_EMBED_WEIGHTS", "")):
        if src:
            dirs.append(src)
    home = os.path.expanduser("~")
    dirs.append(os.path.join(home, "Library", "Application Support", "Whoosh", "Models", "embeddinggemma-2", rev))
    snaps = os.path.join(home, ".cache", "huggingface", "hub", "models--google--embeddinggemma-2", "snapshots")
    if os.path.isdir(snaps):
        dirs += [os.path.join(snaps, n) for n in sorted(os.listdir(snaps))]
    return any(os.path.exists(os.path.join(d, "model.safetensors")) for d in dirs)


def still_loading(s):
    """The accounts the core says are still read in (history not fetched, not failing), read from the core, never the screen."""
    ids = [a for a in SAMPLE_ACCOUNTS if s.ctl.core("sync_state/%s/history_fetched" % a) is False and s.ctl.core("sync_state/%s/healthy" % a) is True]
    return sorted(ids), len(ids)


def catalogue_fr(key):
    cat = json.load(open(os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8"))
    return cat["strings"].get(key, {}).get("localizations", {}).get("fr", {}).get("stringUnit", {}).get("value")


def query(s, text):
    s.click(BAR)
    s.key("⌘A")
    s.type(text)


def launch(s, text):
    """Type the words and press Enter: the search runs."""
    query(s, text)
    try:
        bar = s.rows(BAR)[0]
    except Exception:                                    # a stand-in that serves no bar row: the rule is the plain one
        bar = {}
    if bar.get("launched") is True and bar.get("dropdown") is False and bar.get("text") == text:
        return                                           # these words already run: with the field's text equal to the launched text Return opens the first result (SearchMacModel.fieldMode, SearchMacActs.enter), never a second launch
    s.key("Enter")


FOLDERS, MAIN_LIST, LIST_STATE = "mac-search.sidebar.smart-folders", "mac-main.list", "mac-main.list.state"
FOLDER_CASE = ("par-folder-click-lists-core-rows", "par-folder-header-names-it", "par-folder-count-follows-an-archive")
FOLDER_EXPECT = {FOLDER_CASE[0]: (True, True), FOLDER_CASE[1]: (True, True), FOLDER_CASE[2]: (True, True, True, True)}


def folder_row(ctl, name):
    """The sidebar's row of the saved folder. A folder that is not listed is a FAILED case naming what the sidebar lists (main88: a bare `next` raised
    StopIteration, which the harness does not catch, so the run stopped and no case of it was judged)."""
    rows = ctl.rows(FOLDERS)
    row = next((r for r in rows if r["title"] == name), None)
    if row is None:
        raise KeyError("no smart folder %r in %s: it lists %r" % (name, FOLDERS, [r.get("title") for r in rows]))
    return row


def folder_core(ctl, fid):
    """The core's own answer for the folder: the message ids its saved words find, archived ones gone (`smart_folder/<id>/threads`)."""
    return sorted(ctl.core("smart_folder/%s/threads" % fid) or [])


def folder_opens(ctl, name):
    """Click the saved folder in the main sidebar: the list shows exactly the rows the core says the folder holds (and holds some)."""
    fid = folder_row(ctl, name)["id"]
    want = folder_core(ctl, fid)
    ctl.click("mac-main.sidebar.smart.%s" % fid)
    return (len(want) > 0, sorted(r["id"] for r in ctl.rows(MAIN_LIST)) == want)


def folder_header(ctl, name):
    """The list header names the folder the sidebar opened: the list's own state says which folder, its header line says the name."""
    st = ctl.rows(LIST_STATE)[0]
    return (st.get("smart_folder") == folder_row(ctl, name)["id"], name in (st.get("header_line") or ""))


def folder_count(ctl, name):
    """The folder's count equals the core's row count, before and after an archive from its own list; the archived row leaves both."""
    fid = folder_row(ctl, name)["id"]
    before, held = folder_row(ctl, name)["count"], len(folder_core(ctl, fid))
    ctl.key("E")                                       # archive the focused result of the folder's list
    after, rest = folder_row(ctl, name)["count"], len(folder_core(ctl, fid))
    return (before == held, after == rest, after == before - 1, len(ctl.rows(MAIN_LIST)) == rest)


class FolderModel:
    """The self-test's stand-in for the app: the same four calls as the socket (rows, core, click, key), answering what the built
    screen must answer. `mutant` names one defect; with none it is the correct behaviour. No app is launched."""
    def __init__(self, mutant=None):
        self.mutant, self.held, self.opened = mutant, ["msg-01", "msg-02", "msg-03"], False
        self.saved_count = len(self.held)

    def core(self, path):
        return list(self.held) if path == "smart_folder/sf-4/threads" else None

    def click(self, ax):
        self.opened = ax == "mac-main.sidebar.smart.sf-4" and self.mutant != "click-opens-nothing"

    def key(self, k):
        if k == "E" and self.held:
            self.held.pop(0)

    def rows(self, region):
        if region == FOLDERS:
            return [{"title": "Parity folder", "id": "sf-4", "count": self.saved_count if self.mutant == "count-cached" else len(self.held)}]
        if region == MAIN_LIST:
            return [{"id": i} for i in (self.held if self.opened else ["msg-p%d" % n for n in range(11)])]
        if region == LIST_STATE:
            named = self.opened and self.mutant != "header-keeps-filter"
            return [{"smart_folder": "sf-4" if named else None, "header_line": "Parity folder \u00b7 %d conversations" % len(self.held) if named else "All accounts \u00b7 11 unread"}]
        return []


SELFTEST_MUTANTS = {"click-opens-nothing": FOLDER_CASE[0], "header-keeps-filter": FOLDER_CASE[1], "count-cached": FOLDER_CASE[2]}


def selftest():
    """Each new check green on the model of the right behaviour, and red on its own mutant (and only that one)."""
    judge = {FOLDER_CASE[0]: folder_opens, FOLDER_CASE[1]: folder_header, FOLDER_CASE[2]: folder_count}
    declared = set(re.findall(r"^# J2\.5/(par-folder-[\w-]+) \|", MUTANTS, re.M))
    bad = []
    def run(mutant):
        m = FolderModel(mutant)
        out = {}
        for label in FOLDER_CASE:                      # the same order the journey runs them in: open, header, archive
            out[label] = judge[label](m, "Parity folder") == FOLDER_EXPECT[label]
        return out
    base = run(None)
    print("model, correct behaviour: %s" % base)
    bad += ["%s not green on the correct model" % k for k, v in base.items() if not v]
    for mutant, target in SELFTEST_MUTANTS.items():
        got = run(mutant)
        print("mutant %-20s target %-42s red=%s" % (mutant, target, not got[target]))
        if got[target]:
            bad.append("mutant %s stays green on %s" % (mutant, target))
    bad += ["no mutant line declared for %s" % k for k in FOLDER_CASE if k not in declared]
    # round 12: Enter takes the typed words, chips narrow as typed, the view's rows, the picker's date words
    base12 = round12_run(None)
    print("model, round 12 correct behaviour: %s" % base12)
    bad += ["%s not green on the correct model" % k for k, v in base12.items() if not v]
    for mutant, target in ROUND12_MUTANTS.items():
        got = round12_run(mutant)
        print("mutant %-22s target %-42s red=%s" % (mutant, target, not got[target]))
        if got[target]:
            bad.append("mutant %s stays green on %s" % (mutant, target))
    declared12 = set(re.findall(r"^# J2\.\d/(par-[\w-]+) \|", MUTANTS, re.M))
    bad += ["no mutant line declared for %s" % k for k in base12 if k not in declared12]
    print("%d new checks, %d mutants, %d problems" % (len(FOLDER_CASE) + len(base12), len(SELFTEST_MUTANTS) + len(ROUND12_MUTANTS), len(bad)))
    bad += doors_selftest()
    bad += return_selftest()
    bad += bar_selftest()
    bad += result_selftest()
    bad += passage_selftest()
    bad += related_file_selftest()
    bad += screen_selftest()
    bad += perf_selftest()
    bad += parts_selftest()
    bad += gap_selftest()
    for b in bad:
        sys.stderr.write("SELFTEST FAIL %s\n" % b)
    return 0 if not bad else 1


# ---- round 12 (searchmac-r12): Enter takes what was typed, chips narrow as typed, the view's own rows, the picker's date words ----
STALE_ENTER = "par-enter-launches-what-was-typed"
LIVE_CASES = ("par-chip-narrows-without-enter", "par-filters-alone-narrow-without-enter")
VIEW_CASES = ("par-results-view-served", "par-matched-words-tinted")
DATE_WORDS = ("later today", "tomorrow", "next week")        # the snooze picker's own lines, whole or by their leading words
DATE_NOW = (2026, 5, 11, 12, 0)                               # a Monday noon: tomorrow and next week cut the corpus at different threads
FILTER_FIELD = ' from:"Linea Supply"'                         # the form Tab writes: a settled chip, never half a word


LIVE_WAIT_S = 2.0   # a check bound, not a product number: the field's text reaches the model on the view's next update, then the list is rebuilt


def threads_shown(c):
    return sorted({r["thread"] for r in c.rows(RES)})


def threads_when(c, done):
    """The threads on screen once `done(threads)` holds, polled for LIVE_WAIT_S (the socket can read between a key and the view's update)."""
    end, got = time.time() + LIVE_WAIT_S, threads_shown(c)
    while not done(got) and time.time() < end:
        time.sleep(0.05)
        got = threads_shown(c)
    return got


def enter_takes_typed(c, words):
    """Select the field's words, type new ones and press Enter in the same breath: the bar holds the new words and the core was asked them
    (a stale highlighted dropdown row, a recent search, must not be accepted in their place: main54 J2.8)."""
    c.click(BAR)
    c.key("⌘A")
    c.type(words)
    c.key("Enter")
    return (c.rows(BAR)[0]["text"], c.core("search/last/query"))


def chip_narrows_live(c, before):
    """A settled chip typed after a launched search narrows the list with no Enter: fewer rows, all of them the supplier's; the recent
    searches are as they were (nothing was launched)."""
    recent = [r["text"] for r in c.rows("mac-search.recent")][:1]
    if not c.rows(BAR)[0].get("focused"):                # after the launch the keys are on the results (lead, 13 Nov): the search key puts the cursor back after the words
        c.key("⌘K")
    c.type(FILTER_FIELD)
    after = threads_when(c, lambda t: len(t) < len(before))
    return (len(after) > 0, set(after) <= set(LINEA_THREADS), len(after) < len(before), [r["text"] for r in c.rows("mac-search.recent")][:1] == recent)


def filters_alone_live(c):
    """The field emptied and a chip typed alone: the list is every thread of that sender, with no Enter."""
    if not c.rows(BAR)[0].get("focused"):                # after a Return the keys are on the results (lead, 13 Nov): the search key puts the cursor back in the words
        c.key("⌘K")
    c.key("⌘A")
    c.type(FILTER_FIELD.strip())
    return threads_when(c, lambda t: t == sorted(LINEA_THREADS))


def tinted(c):
    """The words tinted in each result's subject, per thread (a meaning-only row tints nothing)."""
    return {r["thread"]: sorted({w.lower() for w in r["highlights"]}) for r in c.rows(RES) if r["thread"] in WORDS_TINT}


def picker_time(c, word):
    """The time the snooze picker's own line resolves to (mac-main.snooze.options, served from TimeInWords): the line named by these words."""
    rows = c.rows("mac-main.snooze.options")
    hit = next((r for r in rows if r["title"].lower().startswith(word)), None)
    if hit is None:      # a failed case naming the lines, never a StopIteration that stops the run (main88, folder_row)
        raise KeyError("no snooze line starting %r: the picker lists %r" % (word, [r.get("title") for r in rows]))
    return hit["resolves_to"]


def threads_before(iso):
    """The corpus threads with a message received before the time: the fixture's dates are local times."""
    import datetime
    cut = datetime.datetime.strptime(iso, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=datetime.timezone.utc)
    return sorted({m["thread"] for m in MSGS if datetime.datetime.strptime(m["date"], "%Y-%m-%dT%H:%M:%S").astimezone() < cut})


# ---- lane search-checks (SPEC A141 to A143): the suggestions like Apple Mail, the pills, the ranked list with its count line and divider ----
RANK = {"search": 0, "domain": 1, "from": 1, "to": 1, "subject": 2, "attachment": 3, "filter": 4, "label": 5, "folder": 5, "recent": 6}   # the drawn order (SearchSuggest.build)
ORDER_CASE = "par-suggestions-in-the-drawn-order"
TABPILL_CASE = "par-tab-on-a-from-row-makes-the-pill-with-the-address"
NOPILL_CASE = "par-typed-words-make-no-pill"
FROMROW_CASE = "par-from-row-replaces-the-typed-from"
SENT_CASE = "par-person-only-in-sent-mail-is-offered-as-to"
COUNT_CASE = "par-count-line-says-matches-and-close-in-meaning"
DIVIDER_CASE = "par-divider-starts-the-close-group-and-follows-the-stage"
HOLD_CASE = "par-ranking-line-shows-while-the-second-answer-is-held"
REFUSE_CASE = "par-could-not-run-note-says-the-engines-words"
READER_CASE = "par-reader-follows-the-selected-result"
PILLQ_CASE = "par-pill-is-sent-to-the-engine-as-its-colon-form"
SEARCH_CASES = (ORDER_CASE, TABPILL_CASE, NOPILL_CASE, FROMROW_CASE, SENT_CASE, COUNT_CASE, DIVIDER_CASE, HOLD_CASE, REFUSE_CASE, READER_CASE, PILLQ_CASE)
READER_WAIT_S = 2.0   # a check bound, not a product number: the reader follows the selected result after its own quiet time (SPEC A138)


def jval(v):
    """A state value as the socket gives it: JSON text or already decoded."""
    if isinstance(v, str):
        try:
            return json.loads(v)
        except ValueError:
            return v
    return v


_SPOKEN = {"reading": False}


def spoken_code(c, cat, guess):
    """The language the copy SPEAKS, read from a sentence it serves now: the results header's title ("Results" in the catalogue). The guess from the
    settings row and the language lists stays when it is among the languages that give the served title. main93: the settings and the lists named one
    language, the screen spoke another, and the count line and the divider were judged against the wrong words. A copy that serves no header keeps the guess."""
    if _SPOKEN["reading"]:                                # a stand-in that builds its header from app_words asks back: the guess
        return guess
    _SPOKEN["reading"] = True
    try:
        head = c.rows("mac-search.header")
    except Exception:                                     # noqa: BLE001  a stand-in or a screen with no such region: the guess
        return guess
    finally:
        _SPOKEN["reading"] = False
    title = head[0].get("title") if head and isinstance(head[0], dict) else None
    if not isinstance(title, str):
        return guess
    loc = cat.get("Results", {}).get("localizations", {})
    hits = [k for k, v in loc.items() if v.get("stringUnit", {}).get("value") == title] + (["en"] if title == "Results" else [])
    if not hits or guess in hits:
        return guess
    return "en" if "en" in hits else sorted(hits)[0]


def app_words(c, key, *args):
    """The catalogue's sentence for `key` in the language the copy speaks (the Settings row says it; `system` is the Mac's first language the catalogue has);
    `%@` and `%lld` take the arguments in order. A key the catalogue lacks shows as written, as it does in the app (Words.swift)."""
    import subprocess
    cat = json.load(open(os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8"))["strings"]
    code = (c.rows("mac-settings-general.settings") or [{}])[0].get("language") or "en"
    if code == "system":
        have = {l for v in cat.values() for l in v.get("localizations", {})}
        # the hidden copy's own identity first (the lead's harness starts it with `AppleLanguages en` in io.nexus.whoosh.rig, and the app's Words reads
        # that list), the Mac's list only when the copy has none (main92: the Mac said fr, the copy spoke en, every catalogue sentence read wrong)
        out = ""
        for cmd in (["defaults", "read", "io.nexus.whoosh.rig", "AppleLanguages"], ["defaults", "read", "-g", "AppleLanguages"]):   # never `args`: the sentence's own
            try:                                                                                                                   # arguments (main100: "defaults matches")
                r = subprocess.run(cmd, capture_output=True, text=True, timeout=10)
            except (OSError, subprocess.SubprocessError):
                continue
            if r.returncode == 0 and r.stdout.strip():
                out = r.stdout
                break
        langs = re.findall(r'"?([A-Za-z]{2,3}(?:-[A-Za-z0-9]+)*)"?\s*[,)]?\s*$', out, re.M)
        code = next((x for l in langs for x in (l, l.split("-")[0]) if x in have), "en")
    code = spoken_code(c, cat, code)
    loc = cat.get(key, {}).get("localizations", {})
    value = loc.get(code, {}).get("stringUnit", {}).get("value") or loc.get("en", {}).get("stringUnit", {}).get("value") or key
    for a in args:
        value = value.replace("%@", str(a), 1) if "%@" in value else value.replace("%lld", str(a), 1)
    return value


def matched_words(c, n):
    return app_words(c, "1 match") if n == 1 else app_words(c, "%lld matches", n)


def count_words(c, n):
    """The results header's count line (SPEC A149, SearchMacModel.countLine): "No match", "1 match", "N matches"; nothing about the other mails."""
    return app_words(c, "No match") if n == 0 else matched_words(c, n)


def is_close(r):
    """A row of the close-in-meaning group: the row's own `close` (lane match-evidence), else its `matched_in` as before."""
    return r["close"] if "close" in r else r["matched_in"] == "meaning"


def people_core(c, letters):
    """The engine's own answer for the letters (people/prefix/<letters>), as a list."""
    v = jval(c.core("people/prefix/" + letters))
    return v if isinstance(v, list) else []


def rows_of_people(people):
    """The From and To rows the field must offer for the engine's people: From when mail came from them, To when mail was sent to them, From first; at most 4 people."""
    return [(k, p["address"]) for p in people[:4] for k, n in (("from", p.get("from_count", 0)), ("to", p.get("to_count", 0))) if n > 0]


def suggestions_in_drawn_order(c):
    """"Linea" typed: the Search row first, then the people as From and To (the engine's own list, best first, From before To), then one Subject row, one
    Attachment row, the rest in the drawn order; the `group` numbers never go back."""
    query(c, "Linea")
    rows = c.rows(DD)
    ranks = [RANK[r["kind"]] for r in rows]
    want = rows_of_people(people_core(c, "linea"))
    got = [(r["kind"], r["detail"]) for r in rows if r["kind"] in ("from", "to")]
    kinds = [r["kind"] for r in rows]
    groups = [r["group"] for r in rows]
    return (kinds[:1], ranks == sorted(ranks), groups == sorted(groups), len(want) > 0 and got == want, kinds.count("subject"), kinds.count("attachment"))


ORDER_EXPECT = (["search"], True, True, True, 1, 1)


def first_of(c, kind, default=1):
    """The index of the first dropdown row of this kind. SPEC A153 (docs/surfaces/mac-search.md round 3): letters that start a sender's domain
    ("Linea" starts linea-supply-example.com) put "From domain" right under the Search row, so the first person row is no longer row 1.
    A stand-in that serves no kinds keeps the old row 1."""
    try:
        rows = c.rows(DD)
    except (Unreadable, KeyError, IndexError, TypeError, AttributeError):
        return default
    return next((i for i, r in enumerate(rows) if isinstance(r, dict) and r.get("kind") == kind), default)


def down_to(c, kind, text, match=None):
    """The words typed (no Enter), Down until the highlighted row is the first of this kind; returns that row."""
    query(c, text)
    rows = c.rows(DD)
    i = next((i for i, r in enumerate(rows) if r["kind"] == kind and (match is None or match(r))), None)
    if i is None:
        raise IndexError("no %s row in the dropdown (kinds %r)" % (kind, [r["kind"] for r in rows]))
    for _ in range(i):
        c.key("Down")
    return c.rows(DD)[i]


def pill_taken(c, row):
    """After Tab on a From row: ONE from pill whose value is that row's address (a pill from a person names exactly one person), the draft empty."""
    chips = c.rows(CHIPS)
    return ([k["kind"] for k in chips], [k["value"] for k in chips] == [row["detail"]], c.rows(BAR)[0]["draft"])


def pill_query(c, address):
    """After Return on that pill: the engine was asked the pill in its colon form with the address, and the list keeps only that address's mail."""
    q = jval(c.core("search/last/query")) or ""
    return (q.startswith('from:"%s"' % address), threads_shown(c) == sorted({m["thread"] for m in MSGS if m["from"] == address}))


def tab_on_from_row(c):
    """"Linea", Down to the first From row, Tab: ONE from pill whose value is that row's address, the draft empty."""
    row = down_to(c, "from", "Linea")
    c.key("Tab")
    return pill_taken(c, row)


TABPILL_EXPECT = (["from"], True, "")


def words_stay_words(c):
    """The owner's picture ("email from consultant", a pill "From c"): typed words never become a pill, and the query launched is exactly what the field shows."""
    out = []
    for text in ("email from consultant", "from Linea Supply has attachment", "price before June"):
        launch(c, text)
        out.append(([k["kind"] for k in c.rows(CHIPS)], c.rows(BAR)[0]["text"] == text, jval(c.core("search/last/query")) == text))
    return out


NOPILL_EXPECT = [([], True, True)] * 3


def from_row_replaces_typed_from(c):
    """"from Linea" typed: no pill yet (the word stays a word); Down, Tab on the From row: one from pill and no leftover "from" or letters."""
    query(c, EXP["people_query"])
    before = [k["kind"] for k in c.rows(CHIPS)]
    for _ in range(first_of(c, "from")):              # down to the first person row (SPEC A153: "From domain" stands above it for "Linea")
        c.key("Down")
    c.key("Tab")
    return (before, [k["kind"] for k in c.rows(CHIPS)], c.rows(BAR)[0]["draft"])


FROMROW_EXPECT = ([], ["from"], "")


def sent_only_person(c):
    """A person who appears only as the recipient of the owner's sent mail (from_count 0, to_count above 0 in the engine's people list) is offered as To and never as From.
    The letters tried are the first letters of every name the corpus holds and of the sent mail's recipients; none in the corpus is NOT JUDGED with the reason."""
    letters = sorted({(m["from_name"] or "?")[:3].lower() for m in MSGS} | {"kar", "fay"})
    found = None
    for L in letters:
        found = next((p for p in people_core(c, L) if p.get("from_count") == 0 and p.get("to_count", 0) > 0), None)
        if found:
            break
    if found is None:
        raise Unreadable("no person in this corpus appears only in sent mail (the engine's people list for %d letter groups has none with from_count 0 and to_count above 0): "
                         "the fixture corpus rows carry no `outgoing` mail and no `to` (FixtureFile.swift CorpusFile.asMessages); the hook needed: corpus rows with `outgoing` and `to`" % len(letters))
    rows = [(r["kind"], r["detail"]) for r in (query(c, found["name"].split()[0]) or c.rows(DD)) if r["kind"] in ("from", "to")]
    return ("to", found["address"]) in rows and ("from", found["address"]) not in rows


def line_title(head, stage, old_close=False):
    """The one line above the closed line, by the stage (SPEC A150; lane list-p1): the screen of round 3 serves `related` and `rest` in its header and says "Related"
    only above related mail, the date line only when other mail is listed by date; the screen before it (no `related` in its header) said "Close in meaning" above
    every mail found by meaning. None when no line stands there."""
    new = bool(head) and "related" in head[0]
    if stage == "words":
        return "Ranking the other mails by meaning\u2026"
    if new:
        if stage in ("no_model", "model_failed"):   # related mail listed stands under "Related" whatever the stage (round 8: the test engine's stand-in)
            return "Related" if head[0].get("related") else "Other mails, by date" if head[0].get("rest") else None
        return "Related" if head[0].get("related") else None
    return {"ranked": "Close in meaning", "no_model": "Other mails, by date", "model_failed": "Other mails, by date"}.get(stage) if old_close else None


def count_line(c):
    """The count line (SPEC A149, owner 2026-11-11: "makes no sense to have 1 match every time"): `count` says the exact matches alone ("N matches",
    "1 match", "No match") with nothing about the other mails, `matches` is that number, `close` counts the rest, the old `close_line` is gone, and
    the close rows are all after the exact ones. Read after the meaning query ran. (Before lane match-evidence: "N matches" then "· M close in meaning".)"""
    rows = c.rows(RES)
    lead = next((i for i, r in enumerate(rows) if is_close(r)), len(rows))
    close = len(rows) - lead
    tail_ok = all(is_close(r) for r in rows[lead:])
    head = c.rows("mac-search.header")
    # A150: "N matches · M related" when related mail is listed (the header says how many), "N matches" alone otherwise; the old `close` number is the listed rest
    # a count line that differs says both texts (main93 to main95 printed only False three builds running)
    same = lambda h: True if h["count"] == count_expect(c, lead, h.get("related", 0)) else "served %r, expected %r" % (h["count"], count_expect(c, lead, h.get("related", 0)))
    return ([(same(h), h.get("matches") == lead, h.get("close", close) == close, "close_line" not in h) for h in head],
            tail_ok, lead > 0 and (close > 0 or "related" in head[0]))


COUNT_EXPECT = ([(True, True, True, True)], True, True)
EVIDENCE_CASE = "par-every-match-shows-its-evidence"


def match_evidence(c):
    """Every row of the match group (before the close-in-meaning rows) says where the typed letters are (`evidence_in` not `none`) and serves them
    bold (`evidence_bold` not empty). Read after the meaning query ran; a list with no match row judges nothing and is red."""
    rows = c.rows(RES)
    lead = [r for r in rows[:next((i for i, r in enumerate(rows) if is_close(r)), len(rows))]]
    bad = [r["id"] for r in lead if r.get("evidence_in") in (None, "none") or not r.get("evidence_bold")]
    return (len(lead) > 0, bad)


EVIDENCE_EXPECT = (True, [])


def view_served(c):
    """What the results pane draws around its rows is on the socket: the header's title, the divider's words for the stage (a Close in meaning group, or the ranking line,
    or the by-date line when no model answered), and the footer line; the words are the catalogue's in the language the copy speaks.
    The three reads are made once the answer is in (main197: the header was read while the search was loading and the footer after it: `([], False, True)`)."""
    end = time.time() + 4.0
    while time.time() < end and c.rows(BAR)[0].get("loading") is True:
        time.sleep(0.1)
    head, secs, foot = c.rows("mac-search.header"), [r["title"] for r in c.rows("mac-search.sections")], [r["text"] for r in c.rows("mac-search.footer")]
    stage = head[0].get("stage") if head else None
    title = line_title(head, stage, old_close=True)
    close = any(r["matched_in"] == "meaning" for r in c.rows(RES))
    new = bool(head) and "related" in head[0]
    return ([h["title"] == app_words(c, "Results") for h in head], secs == ([app_words(c, title)] if title and (close or stage == "words" or new) else []), foot == [app_words(c, EXP["results_footer"])])


VIEW_EXPECT = ([True], True, True)


def divider_and_stage(c, installed):
    """The divider's words follow the stage the engine ended on: ranked (a model answered) "Close in meaning", no model "Other mails, by date"; the header, the core
    and the divider agree, and the rows before the divider are the exact ones. With no weights on the rig the stage must be no_model."""
    want = "ranked" if installed else "no_model"
    h = c.rows("mac-search.header")[0]
    title = line_title([h], want, old_close=True)
    return (h.get("stage"), jval(c.core("search/last/stage")), [r["title"] for r in c.rows("mac-search.sections")] == ([app_words(c, title)] if title else []), want)


def divider_expect(installed):
    want = "ranked" if installed else "no_model"
    return (want, want, True, want)


def fresh_words(n):
    """The words query (WORDS_QUERY, item 3bb3d817: the meaning question is held whole by no mail) in another word order (n-th rotation): NEW words for the engine, the same mails. A case that needs the engine to answer
    again cannot relaunch the words on screen: unchanged words are not launched (launch() presses no Enter) and the same engine words are not asked
    again (SearchMacModel.runSearch asks only when the words differ). main93: the held, refused and index-line cases relaunched the same question
    and read the earlier answer."""
    w = WORDS_QUERY.split()
    n %= len(w)
    return " ".join(w[n:] + w[:n]) if n else " ".join(reversed(w))


def ranking_in_progress(c, installed):
    """The two answers: with the second held, the first list shows its exact matches, ONE line "Ranking the other mails by meaning…" and no tail; released, the ranked list
    replaces it in place and the selection stays on its mail. The hold is `fixture search-hold on|off` (the build without it: NOT JUDGED)."""
    c.send("fixture", "search-hold", "on")
    try:
        launch(c, fresh_words(0))
        c.key("Down")
        rows, h = c.rows(RES), c.rows("mac-search.header")[0]
        mid = next((r["id"] for r in rows if r.get("selected")), None)
        held = (h.get("stage"), h.get("tail_pending"), [r["title"] for r in c.rows("mac-search.sections")] == [app_words(c, "Ranking the other mails by meaning…")],
                all(r["matched_in"] != "meaning" for r in rows) and len(rows) > 0)
    finally:
        c.send("fixture", "search-hold", "off")
    end = time.time() + LIVE_WAIT_S
    while c.rows("mac-search.header")[0].get("stage") == "words" and time.time() < end:
        time.sleep(0.05)
    rows2, h2 = c.rows(RES), c.rows("mac-search.header")[0]
    after_title = line_title([h2], h2.get("stage"), old_close=True)      # with no model the second answer never comes: the list ends by date
    return (held, (h2.get("stage"), [r["title"] for r in c.rows("mac-search.sections")] == ([app_words(c, after_title)] if after_title else []),
                   next((r["id"] for r in rows2 if r.get("selected")), None) == mid, len(rows2) + (h2.get("rest") or 0) > len(rows)))


def hold_expect(installed):
    return (("words", True, True, True), ("ranked" if installed else "no_model", True, True, True))


def could_not_run(c):
    """A search the engine refuses: the pane says the one sentence and the engine's words under it, lists nothing. The refusal is `fixture search-refuse on|off`."""
    c.send("fixture", "search-refuse", "on")
    try:
        launch(c, fresh_words(1))
        end = time.time() + 4.0                          # the refusal is said once the skeleton gives way (`refused` is served only then)
        while time.time() < end and c.rows(BAR)[0].get("loading") is True:
            time.sleep(0.1)
        ref, rows = c.rows("mac-search.refused"), c.rows(RES)
    finally:
        c.send("fixture", "search-refuse", "off")
    return ([r["text"] == app_words(c, "The search could not run.") for r in ref], [bool((r.get("words") or "").strip()) for r in ref], len(rows))


REFUSE_EXPECT = ([True], [True], 0)


def reader_follows_result(c):
    """Down, Down, Up on the results: after the reader's quiet time the reader shows the selected result, no Return pressed (SPEC A138)."""
    launch(c, EXP["meaning_query"])
    out = []
    for k in ("Down", "Down", "Up"):
        c.key(k)
        end = time.time() + READER_WAIT_S
        while True:
            sel = [r["id"] for r in c.rows(RES) if r.get("selected")]
            shown = [r["id"] for r in c.rows("mac-main.reader.subject")]
            if (len(sel) == 1 and shown == sel) or time.time() > end:
                break
            time.sleep(0.05)
        out.append(len(sel) == 1 and shown == sel)
    return out


READER_EXPECT = [True, True, True]


def pill_goes_to_the_engine(c):
    """Tab on a From row, Return: the engine was asked the pill in its colon form with the address, and the list keeps only that address's mail."""
    row = down_to(c, "from", "Linea")
    c.key("Tab")
    c.key("Enter")
    return pill_query(c, row["detail"])


PILLQ_EXPECT = (True, True)

# the date pills: typed words make none; the row the words name is offered, Tab makes the pill with the picker's day, and the list keeps the mail before that day
DATE_PILL_CASE = "par-date-words-are-offered-as-a-pill-with-the-pickers-day"
BARE_CASE = "par-bare-snooze-word-stays-a-word"
FROM_CASE = "par-from-prefix-lists-from-rows"


def picker_day(c, word):
    """The local day the snooze picker's own line resolves to (YYYY-MM-DD)."""
    import datetime
    iso = picker_time(c, word)
    return datetime.datetime.strptime(iso, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=datetime.timezone.utc).astimezone().strftime("%Y-%m-%d")


def threads_before_day(day):
    """The corpus threads with a message received before that day's start (the engine keeps `before:` strictly earlier than the day)."""
    return sorted({m["thread"] for m in MSGS if m["date"][:10] < day})


def date_words(c):
    """Per picker word: `before <word>` typed makes NO pill; the dropdown offers a filter row for it; Tab makes the before pill whose value is the picker's day; Return lists
    exactly the threads before that day."""
    out = {}
    for w in DATE_WORDS:
        day = picker_day(c, w)
        query(c, "before " + w)
        none = [k["kind"] for k in c.rows(CHIPS)] == []
        rows = c.rows(DD)
        i = next((i for i, r in enumerate(rows) if r["kind"] == "filter" and r.get("chip") == "before"), None)
        if i is not None:
            for _ in range(i):
                c.key("Down")
            c.key("Tab")
        chips = [(k["kind"], k["value"]) for k in c.rows(CHIPS)]
        c.key("Enter")
        out[w] = (none, i is not None, chips == [("before", day)], threads_shown(c) == threads_before_day(day))
    return out


DATE_EXPECT = {w: (True, True, True, True) for w in DATE_WORDS}


def bare_date_words(c):
    """A snooze word typed alone, or beside another word, is plain words: no pill, and the engine is asked exactly those words."""
    out = {}
    for w in DATE_WORDS:
        launch(c, w)
        alone = ([k["kind"] for k in c.rows(CHIPS)], jval(c.core("search/last/query")) == w)
        launch(c, "quote " + w)
        out[w] = (alone, [k["kind"] for k in c.rows(CHIPS)])
    return out


BARE_EXPECT = {w: (([], True), []) for w in DATE_WORDS}


def from_prefix_people(c):
    """"from Linea" typed: the word "from" stays a word (no pill); the dropdown lists a From row per address of the supplier and no To row (the corpus holds no sent mail)."""
    query(c, EXP["people_query"])
    rows = c.rows(DD)
    return ([k["kind"] for k in c.rows(CHIPS)], sorted(r["detail"] for r in rows if r["kind"] == "from"), sorted(r["detail"] for r in rows if r["kind"] == "to"))


FROM_EXPECT = ([], sorted(EXP["people_addresses"]), [])
PERF_CASES = ("par-suggestions-under-50-ms-after-a-typed-letter", "par-first-result-rows-under-50-ms-after-return")
PERF_BUDGET_KEY = "INTERACTION_COMMIT_MS"      # owner 2026-11-10: "The objective is <50ms for any operation"; docs/CONSTANTS.md INTERACTION_COMMIT_MS = 50


def under_load(value, budget, strict=True):
    """The shared load rule (journeys/lib/loadgate.py, lead 2026-11-13): at or below PERF_LOAD_MAX the timing is judged as always; above
    it a timing inside its budget still passes (the load is printed) and one over its budget is NOT JUDGED with its number."""
    from lib import loadgate
    return loadgate.under_load(value, budget, strict, Unreadable)


def judge_timing(fn, c):
    """One timing case: (under the budget, the measure in ms, the detail line). The detail carries the NUMBER on a pass as on a fail, so the printed case
    line shows the measure (owner: "The objective is <50ms for any operation"); a measure that cannot be taken raises Unreadable (NOT JUDGED)."""
    got = fn(c)
    limit = const(PERF_BUDGET_KEY)
    return got < limit, got, "%.1f ms, expected < %s ms" % (got, limit)


def letter_to_suggestions_ms(c):
    """The time from a typed letter to the suggestions under the field: the letter goes in as the `type` verb, which the app's interaction probe measures
    (ControlSocket/Verbs.swift "type": `InteractionProbe.run { Registrations.type(text, m) }`). The `key` verb is a chord for the commands
    (AppModel.press): a plain letter never reaches the field that way (main105: bar text 'Lin' after 'Lin', NOT JUDGED twice). The probe's `commit` is
    the milliseconds from the letter's arrival to the commit of the screen it changed, and the suggestions must be on that screen. A letter that did not
    reach the field, or no suggestions, is NOT JUDGED naming what was read."""
    c.key("/") if c.uistate().get("surface") != "mac-search" else None
    c.click(BAR)
    c.key("⌘A")
    c.type("Lin")                                  # the field holds "Lin" already: the measured letter is the next one
    before = c.rows(BAR)[0]["text"]
    c.type("e")
    b = c.rows(BAR)[0]
    if b["text"] != before + "e":
        raise Unreadable("the typed letter did not reach the field (bar text %r after %r)" % (b["text"], before))
    ms = float(c.measure("mac-main.interaction.commit")["ms"])
    if not c.rows(DD):
        raise Unreadable("no suggestion rows after the letter (bar %r)" % b["text"])
    return under_load(ms, const(PERF_BUDGET_KEY))


def return_to_first_rows_ms(c):
    """The time from Return to the first result rows: the stopwatch starts as the key is sent and stops at the first read that finds result rows (a poll, so a few ms coarse),
    beside the probe's own `commit` for the key; the larger is judged."""
    c.click(BAR)
    c.key("⌘A")
    c.type(EXP["meaning_query"])
    c.rows(DD)
    t0 = time.monotonic()
    c.key("Enter")
    polled = None
    while time.monotonic() - t0 < 5.0:
        if c.rows(RES):
            polled = (time.monotonic() - t0) * 1000.0
            break
    if polled is None:
        raise Unreadable("no result rows within 5 s of Return")
    commit = float(c.measure("mac-main.interaction.commit")["ms"])
    return under_load(max(polled, commit), const(PERF_BUDGET_KEY))


class ScreenModel:
    """The self-test's stand-in for the search screen on the test engine, for the lane search-checks cases: the socket's calls (rows, core, click, key, type, send,
    measure, uistate), answering what the built screen must answer under SPEC A141 to A143. `mutant` names one defect; with none it is the correct behaviour.
    `hooks=False` is a build without the two fixture hooks (search-hold, search-refuse); `sent` adds a person the owner only wrote to. No app is launched."""
    def __init__(self, mutant=None, hooks=True, sent=False, model=True):
        import datetime
        self.m, self.hooks, self.sent, self.model = mutant, hooks, sent, model
        self.text, self.shown, self.launched, self.dropdown, self.hl, self.sel = "", "", False, False, 0, 0
        self.recents, self.hold, self.refuse, self.release_at = ["quote"], False, False, None
        self.now = datetime.datetime(*DATE_NOW)
        self.snooze = {"later today": self.now + datetime.timedelta(hours=3), "tomorrow": datetime.datetime(2026, 5, 12, 8, 0), "next week": datetime.datetime(2026, 5, 18, 8, 0)}
        self.last_q = None
        self.commit_ms = 12.0

    # ---- the engine's people (people/prefix/<letters>)
    def people(self, letters):
        letters = letters.lower()
        out = {}
        for m in MSGS:
            p = out.setdefault(m["from"], {"name": m["from_name"], "address": m["from"], "from_count": 0, "to_count": 0})
            p["from_count"] += 1
        if self.sent:
            out["orders@linea-supply-example.com"]["to_count"] = 1      # the owner once wrote to the supplier too: both rows for one person
            out["karen@linea.example"] = {"name": "Karen Bosch", "address": "karen@linea.example", "from_count": 0, "to_count": 2}
        hit = [p for p in out.values() if any(w.lower().startswith(letters) for w in p["name"].replace("@", " ").replace(".", " ").split()) or p["address"].lower().startswith(letters)]
        return sorted(hit, key=lambda p: (-(p["from_count"] + p["to_count"]), p["name"]))[:8]

    # ---- the field's reading of its text: pills only from the colon forms (SearchQuery.typed)
    @staticmethod
    def parse(text):
        chips, rest = [], text
        for m in re.finditer(r'\b(from|to|subject|attachment|label|before|after):("[^"]*"|[^\s"]+)', text, re.I):
            kind, val = m.group(1).lower(), m.group(2).strip('"')
            if kind in ("before", "after") and not re.fullmatch(r"\d{4}-\d{2}-\d{2}", val):
                continue                                     # not a day: the words stay words
            if any(k == kind for k, _ in chips):
                continue                                     # one pill per kind
            chips.append((kind, val))
            rest = rest.replace(m.group(0), " ", 1)
        if re.search(r"\bhas:attachments?\b", rest, re.I):
            chips.append(("has", "attachment"))
            rest = re.sub(r"\bhas:attachments?\b", " ", rest, flags=re.I)
        return chips, rest.split()

    def draft(self, text):
        return " ".join(self.parse(text)[1])

    def passes(self, chips, m):
        for kind, v in chips:
            if kind == "from" and v.lower() not in (m["from"] + " " + m["from_name"]).lower():
                return False
            if kind == "before" and not m["date"][:10] < v:
                return False
            if kind == "after" and not m["date"][:10] >= v:
                return False
            if kind == "subject" and v.lower() not in m["subject"].lower():
                return False
            if kind == "attachment" and not any(v.lower() in a.lower() for a in m["attachments"]):
                return False
            if kind == "to":
                return False                                 # the corpus holds no sent mail: nobody was written to
            if kind == "has" and not m["attachments"]:
                return False
        return True

    # ---- the socket's calls
    def click(self, ax):
        if ax == BAR:
            self.dropdown = bool(self.text)
        elif ax.startswith("mac-search.dropdown.") and ax.endswith(".tab"):
            self.hl = int(ax.split(".")[2]); self.tab()
        elif ax == "mac-search.skeleton.hold" or ax == "mac-search.skeleton.release":
            pass

    def type(self, t):
        self.text += t
        self.dropdown, self.hl = True, 0

    def key(self, k):
        if k == "⌘A":
            self.text = ""
        elif k == "Down":
            if self.dropdown:
                self.hl = min(self.hl + 1, len(self.suggestions()) - 1)
            elif self.launched:
                self.sel = min(self.sel + 1, max(len(self.results()) - 1, 0))
        elif k == "Up":
            if self.dropdown:
                self.hl = max(self.hl - 1, 0)
            elif self.launched:
                self.sel = max(self.sel - 1, 0)
        elif k == "Tab":
            self.tab()
        elif k == "Enter":
            if self.launched and not self.dropdown and self.text == self.shown:
                return
            self.shown, self.launched, self.dropdown, self.hl, self.sel = self.text, True, False, 0, 0
            self.recents.insert(0, self.text)
            chips, words = self.parse(self.text)
            self.last_q = self.engine_query()
            self.release_at = None

    def engine_query(self):
        chips, words = self.parse(self.shown or self.text)
        if self.m == "pill-sent-as-words":
            return " ".join(words)
        forms = ['%s:"%s"' % (k, v) if k in ("from", "to", "subject", "attachment", "label") else ("has:attachment" if k == "has" else "%s:%s" % (k, v)) for k, v in chips]
        return " ".join(forms + [w for w in words if any(ch.isalnum() for ch in w)])

    def send(self, verb, *args):
        if not self.hooks or verb != "fixture":
            raise Unreadable("send %s %s: unknown fixture verb (the build has no such hook)" % (verb, args))
        if args[0] == "search-hold":
            self.hold = args[1] == "on"
        elif args[0] == "search-refuse":
            self.refuse = args[1] == "on"
        return {}

    def measure(self, ax):
        return {"ms": self.commit_ms}

    def uistate(self):
        return {"surface": "mac-search"}

    # ---- the suggestions (SearchSuggest.build): Search, From and To, Subject, Attachment, filter rows, recents
    def suggestions(self):
        draft = self.draft(self.text)
        words = self.text.split()
        rows = []
        if self.text.strip():
            rows.append({"kind": "search", "title": self.text, "detail": "", "chip": None, "group": 0, "highlighted": False})
        if not words:
            return [dict(kind="recent", title=r, detail="", chip=None, group=2, highlighted=False) for r in self.recents[:1]]
        last = words[-1]
        if self.m != "no-people-rows":
            for p in self.people(last)[:4]:
                kinds = [("from", p["from_count"]), ("to", p["to_count"])]
                if self.m == "to-before-from":
                    kinds.reverse()
                for k, n in kinds:
                    if n > 0 or (self.m == "from-row-for-sent-only" and k == "from"):
                        rows.append({"kind": k, "title": p["name"], "detail": p["address"], "chip": k, "group": 1, "highlighted": False, "replaces": ("%s %s" % (k, last)) if len(words) > 1 and words[-2].lower() == k else last})
        rows.append({"kind": "subject", "title": draft, "detail": "", "chip": "subject", "group": 1, "highlighted": False, "replaces": draft})
        rows.append({"kind": "attachment", "title": draft, "detail": "", "chip": "attname", "group": 1, "highlighted": False, "replaces": draft})
        low = [w.lower() for w in words]
        for i in range(len(low) - 1):
            for phrase in self.snooze:
                if low[i] == "before" and low[i + 1:i + 1 + len(phrase.split())] == phrase.split():
                    day = self.snooze[phrase].astimezone().strftime("%Y-%m-%d")
                    rows.append({"kind": "filter", "title": "Before " + day, "detail": "", "chip": "before", "group": 1, "highlighted": False, "replaces": "before " + phrase, "value": day})
            if low[i] == "has" and low[i + 1] in ("attachment", "attachments"):
                rows.append({"kind": "filter", "title": "Has attachment", "detail": "", "chip": "attachment", "group": 1, "highlighted": False, "replaces": "has attachment", "value": "attachment"})
        if self.m == "recent-before-people":
            rows.insert(1, {"kind": "recent", "title": self.recents[0], "detail": "", "chip": None, "group": 2, "highlighted": False})
        rows += [{"kind": "recent", "title": r, "detail": "", "chip": None, "group": 2, "highlighted": False} for r in self.recents[:2]]
        return rows

    def tab(self):
        rows = self.suggestions()
        if not self.dropdown or self.hl >= len(rows) or rows[self.hl]["kind"] == "search" or rows[self.hl]["chip"] is None:
            return
        r = rows[self.hl]
        draft = self.text
        rep = r.get("replaces")
        if rep and rep.lower() in draft.lower() and not (self.m == "from-row-keeps-from" and r["kind"] == "from" and rep.lower().startswith("from ")):
            i = draft.lower().rfind(rep.lower()); draft = draft[:i] + draft[i + len(rep):]
        if self.m == "from-row-keeps-from" and r["kind"] == "from" and rep and rep.lower().startswith("from "):
            draft = draft.lower().replace("linea", "").strip()
        if r["kind"] in ("from", "to"):
            value = r["detail"] if self.m != "pill-carries-name" else r["title"]
            form = '%s:"%s"' % (r["kind"], value)
        elif r["kind"] == "filter":
            form = "has:attachment" if r["chip"] == "attachment" else "before:%s" % r["value"]
        else:
            form = '%s:"%s"' % (r["chip"].replace("attname", "attachment"), r["title"] if self.m != "subject-pill-other-words" else "something else")
        self.text = (form + " " + " ".join(draft.split())).strip()
        self.hl = 0
        if self.launched:
            self.shown = self.text                          # a settled pill narrows a launched list at once, with no Return (narrowLive)

    # ---- the results: every mail, exact first, then the others (the test engine's own rule, A141)
    def results(self):
        if not self.launched or self.refuse:
            return []
        chips, words = self.parse(self.shown)
        words = [w for w in words if any(ch.isalnum() for ch in w)]
        kept = [m for m in MSGS if self.passes(chips, m)]
        if self.m == "from-pill-ignored":
            kept = [m for m in MSGS if self.passes([c for c in chips if c[0] != "from"], m)]
        if not words:
            return [dict(m, close=False) for m in sorted(kept, key=lambda m: m["date"], reverse=True)]
        exact = [m for m in kept if any(w.lower() in (m["subject"] + " " + m["body"]).lower() for w in words)]   # the test engine ORs the typed words (tier 0)
        tail = [m for m in kept if m not in exact]
        if self.m == "list-cut":
            tail = tail[:2]
        if self.held():
            return [dict(m, close=False) for m in exact]
        rows = [dict(m, close=False) for m in exact] + [dict(m, close=True) for m in tail]
        if self.m == "exact-after-close" and len(rows) > 2:
            rows[0], rows[-1] = rows[-1], rows[0]
        return rows

    def held(self):
        return self.hold and self.hooks and self.m != "hold-ignored"

    def stage(self):
        chips, words = self.parse(self.shown)
        if not [w for w in words if any(ch.isalnum() for ch in w)]:
            return "filtered"
        if self.held():
            return "words"
        return "ranked" if self.model else "no_model"

    def core(self, path):
        if path == "search/last/query":
            return self.last_q
        if path == "search/last/stage":
            return json.dumps(self.stage()) if self.m != "stage-unread" else json.dumps("ranked")
        if path.startswith("people/prefix/"):
            return json.dumps(self.people(path[len("people/prefix/"):]))
        return None

    def rows(self, region):
        res = self.results()
        if region == BAR:
            return [{"text": self.text, "launched": self.launched, "dropdown": self.dropdown, "draft": self.draft(self.text), "focused": True, "owner": "search", "says": ""}]
        if region == DD:
            rows = self.suggestions() if self.dropdown else []
            return [dict(r, highlighted=(i == self.hl), keyed=False, hovered=False, account=None) for i, r in enumerate(rows)]
        if region == CHIPS:
            chips, _ = self.parse(self.text)
            return [{"kind": {"has": "attachment", "attachment": "attname"}.get(k, k), "value": ("" if k == "has" else v), "title": k, "resolves_to": None} for k, v in chips]
        if region == RES:
            words = [w for w in self.parse(self.shown)[1] if any(ch.isalnum() for ch in w)]
            def evidence(m):
                if m["close"] or (self.m == "evidence-missing" and m is res[0]):
                    return "none", []
                for place, text in (("subject", m["subject"]), ("text", m["body"])):
                    hit = [text[text.lower().index(w.lower()):][:len(w)] for w in words if w.lower() in text.lower()]
                    if hit:
                        return place, hit
                return "none", []
            return [dict({"id": m["id"], "thread": m["thread"], "matched_in": "meaning" if m["close"] else "subject", "selected": i == self.sel, "close": m["close"]},
                         evidence_in=evidence(m)[0], evidence_bold=evidence(m)[1]) for i, m in enumerate(res)]
        if region == "mac-search.recent":
            return [{"text": r} for r in self.recents]
        if region == "mac-search.refused":
            return [{"text": app_words(self, "The search could not run.") if self.m != "refusal-unsaid" else "", "words": "type something to search for" if self.m != "refusal-no-words" else ""}] if self.refuse and self.launched else []
        if region == "mac-search.header":
            if not self.launched or self.refuse or self.m == "view-unserved":
                return []
            lead = sum(1 for m in res if not m["close"])
            close = len(res) - lead
            tail_pending = self.stage() == "words"
            count = count_words(self, lead if self.m != "count-counts-all" else len(res))
            row = {"title": "Results", "count": count, "matches": lead, "close": close if self.m != "close-uncounted" else 0, "stage": self.stage(), "tail_pending": tail_pending}
            if self.m == "close-line-kept":                   # the header before A149: "N matches · M close in meaning", with its `close_line` row
                row["close_line"] = app_words(self, "· %lld close in meaning", close) if close else None
                row["count"] = count + (" " + row["close_line"] if close else "")
            return [row]
        if region == "mac-search.sections":
            if not self.launched or self.refuse:
                return []
            st = self.stage()
            if st == "words":
                return [{"title": app_words(self, "Ranking the other mails by meaning…") if self.m != "ranking-line-missing" else ""}]
            if any(m["close"] for m in res):
                title = {"ranked": "Close in meaning", "no_model": "Other mails, by date"}.get(st, "Close in meaning")
                if self.m == "divider-always-close":
                    title = "Close in meaning"
                return [{"title": app_words(self, title)}]
            return []
        if region == "mac-search.footer":
            return [] if self.m == "view-unserved" or not res else [{"text": app_words(self, EXP["results_footer"])}]
        if region == "mac-main.reader.subject":
            if not res:
                return []
            return [{"id": res[0]["id"] if self.m == "reader-stays" else res[self.sel]["id"]}]
        if region == "mac-main.snooze.options":
            iso = lambda d: d.astimezone(__import__("datetime").timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
            names = {"later today": "Later today", "tomorrow": "Tomorrow morning", "next week": "Next week"}
            return [{"title": names[w], "resolves_to": iso(d)} for w, d in self.snooze.items()]
        if region == "mac-settings-general.settings":
            return [{"language": "en"}]
        return []


def screen_run(mutant=None, hooks=True, sent=False, model=True, only=None):
    """The lane's cases in the order the journey runs them, each on a fresh model; returns {case: green?}. A case that cannot be judged is None; `only` limits the cases."""
    out = {}
    def judge(name, fn, want):
        if only is not None and name not in only:
            return
        m = ScreenModel(mutant, hooks, sent, model)
        try:
            out[name] = fn(m) == (want() if callable(want) else want)
        except Unreadable:
            out[name] = None
        except (KeyError, IndexError, TypeError):
            out[name] = False
    judge(ORDER_CASE, suggestions_in_drawn_order, ORDER_EXPECT)
    judge(TABPILL_CASE, tab_on_from_row, TABPILL_EXPECT)
    judge(NOPILL_CASE, words_stay_words, NOPILL_EXPECT)
    judge(FROMROW_CASE, from_row_replaces_typed_from, FROMROW_EXPECT)
    judge(SENT_CASE, sent_only_person, True)
    judge(COUNT_CASE, lambda c: (launch(c, WORDS_QUERY) or count_line(c)), COUNT_EXPECT)
    judge(EVIDENCE_CASE, lambda c: (launch(c, WORDS_QUERY) or match_evidence(c)), EVIDENCE_EXPECT)
    judge(VIEW_CASES[0], lambda c: (launch(c, WORDS_QUERY) or view_served(c)), VIEW_EXPECT)
    judge(DIVIDER_CASE, lambda c: (launch(c, WORDS_QUERY) or divider_and_stage(c, model)), divider_expect(model))
    judge(HOLD_CASE, lambda c: ranking_in_progress(c, model), hold_expect(model))
    judge(REFUSE_CASE, could_not_run, REFUSE_EXPECT)
    judge(READER_CASE, reader_follows_result, READER_EXPECT)
    judge(PILLQ_CASE, pill_goes_to_the_engine, PILLQ_EXPECT)
    judge(DATE_PILL_CASE, date_words, DATE_EXPECT)
    judge(BARE_CASE, bare_date_words, BARE_EXPECT)
    judge(FROM_CASE, from_prefix_people, FROM_EXPECT)
    return out


class eager_parse:
    """Context: the stand-in reads typed words as pills again ("from <word>", "before <word>", a lone snooze word), the defect SPEC A142 removed."""
    def __enter__(self):
        self.old = ScreenModel.__dict__["parse"]
        old = self.old

        def eager(text):
            chips, words = old.__func__(text)
            ws = text.split()
            for i, w in enumerate(ws[:-1]):
                if w.lower() == "from" and not any(k == "from" for k, _ in chips):
                    chips.append(("from", ws[i + 1]))
                if w.lower() == "before" and not any(k == "before" for k, _ in chips):
                    chips.append(("before", ws[i + 1]))
            if ws and ws[0].lower() in ("later", "tomorrow", "next"):
                chips.append(("before", ws[0]))
            return chips, words
        ScreenModel.parse = staticmethod(eager)
        return self

    def __exit__(self, *a):
        ScreenModel.parse = self.old


SCREEN_MUTANTS = {
    "no-people-rows": (ORDER_CASE, TABPILL_CASE, FROMROW_CASE, PILLQ_CASE, FROM_CASE), "to-before-from": (ORDER_CASE,), "recent-before-people": (ORDER_CASE,),   # the order case catches it; the from-row case now goes down to the From row itself (SPEC A153, main93)
    "pill-carries-name": (TABPILL_CASE, PILLQ_CASE), "from-row-keeps-from": (FROMROW_CASE,),
    "from-row-for-sent-only": (SENT_CASE,),
    "pill-sent-as-words": (PILLQ_CASE,), "from-pill-ignored": (PILLQ_CASE,),
    "count-counts-all": (COUNT_CASE,), "close-uncounted": (COUNT_CASE,), "exact-after-close": (COUNT_CASE, EVIDENCE_CASE), "close-line-kept": (COUNT_CASE,),
    "evidence-missing": (EVIDENCE_CASE,),
    "divider-always-close": (DIVIDER_CASE, VIEW_CASES[0], HOLD_CASE), "stage-unread": (DIVIDER_CASE,),
    "ranking-line-missing": (HOLD_CASE,), "hold-ignored": (HOLD_CASE,),
    "view-unserved": (VIEW_CASES[0], COUNT_CASE, DIVIDER_CASE, HOLD_CASE), "refusal-unsaid": (REFUSE_CASE,), "refusal-no-words": (REFUSE_CASE,),
    "reader-stays": (READER_CASE,),
}
SENT_MUTANTS = {"to-before-from", "from-row-for-sent-only"}      # judged on a model whose corpus holds sent mail (the ordinary corpus holds none)
NOMODEL_MUTANTS = {"divider-always-close", "stage-unread"}        # judged on a copy with no model: the divider must say "by date" there
PARSE_MUTANTS = {"words-make-pills": (NOPILL_CASE, FROM_CASE, BARE_CASE, DATE_PILL_CASE)}


def screen_selftest():
    """Each lane case green on the model of the right behaviour, NOT JUDGED where the build lacks a hook (never green), red on its own mutants and on no other case."""
    bad = []
    base = screen_run()
    print("model, search-checks cases: %s" % {k: {True: "green", False: "RED", None: "not judged"}[v] for k, v in base.items()})
    bad += ["%s not green on the correct model" % k for k, v in base.items() if v is not True and k != SENT_CASE]
    if base[SENT_CASE] is not None:
        bad.append("%s judged on a corpus with no sent mail" % SENT_CASE)
    withsent = screen_run(sent=True)
    if withsent[SENT_CASE] is not True:
        bad.append("%s not green with a sent-only person" % SENT_CASE)
    nohooks = screen_run(hooks=False)
    for k in (HOLD_CASE, REFUSE_CASE):
        if nohooks[k] is not None:
            bad.append("%s was judged by a build with no fixture hook" % k)
    nomodel = screen_run(model=False)
    if nomodel[DIVIDER_CASE] is not True and nomodel[DIVIDER_CASE] is not None:
        bad.append("%s red on a copy with no model" % DIVIDER_CASE)
    covered = set()
    for mutant, targets in SCREEN_MUTANTS.items():
        sent_mode = mutant in SENT_MUTANTS
        got = screen_run(mutant, sent=sent_mode, model=mutant not in NOMODEL_MUTANTS, only=(ORDER_CASE, SENT_CASE) if sent_mode else None)
        covered |= set(targets)
        reds = {k for k, v in got.items() if v is False}
        print("mutant %-24s targets %-60s red=%s" % (mutant, ",".join(t[4:20] for t in targets), reds >= set(targets)))
        bad += ["mutant %s stays green on %s" % (mutant, t) for t in targets if got[t] is not False]
        bad += ["mutant %s also reddens %s" % (mutant, k) for k in reds - set(targets)]
    # the date and bare-word cases: a model that reads words as pills must go red on them
    for mutant, targets in PARSE_MUTANTS.items():
        with eager_parse():
            got = screen_run(mutant)
        covered |= set(targets)
        print("mutant %-24s targets %-60s red=%s" % (mutant, ",".join(t[4:20] for t in targets), all(got[t] is False for t in targets)))
        bad += ["mutant %s stays green on %s" % (mutant, t) for t in targets if got[t] is not False]
    required = set(SEARCH_CASES) | {DATE_PILL_CASE, BARE_CASE, FROM_CASE, VIEW_CASES[0], EVIDENCE_CASE}
    bad += ["%s has no mutant that turns it red" % k for k in sorted(required - covered - {SENT_CASE})]
    declared = set(re.findall(r"^# J2\.\d/(par-[\w-]+) \|", MUTANTS, re.M))
    bad += ["no mutant line declared for %s" % k for k in sorted(required | set(PERF_CASES)) if k not in declared]
    print("%d search-checks cases, %d mutants, %d problems" % (len(required), len(SCREEN_MUTANTS) + len(PARSE_MUTANTS), len(bad)))
    return bad


def perf_selftest():
    """The two timing cases: under the budget on a quiet model, over it red, a key that did not type NOT JUDGED, load above the limit NOT JUDGED."""
    bad = []
    class Typing(ScreenModel):
        def __init__(self, ms, types=True):
            super().__init__(); self.ms, self.types = ms, types
        def key(self, k):
            if len(k) == 1 and k.isalpha():
                return                    # as the app: the `key` verb is a command chord, a plain letter never lands in the field
            return super().key(k)
        def type(self, t):
            if self.types:
                super().type(t)
        def measure(self, ax):
            return {"ms": self.ms}
        def uistate(self):
            return {"surface": "mac-search"}
    limit = const(PERF_BUDGET_KEY)
    def run(fn, ms, types=True, load=0.5):
        saved = os.getloadavg
        os.getloadavg = lambda: (load, load, load)
        try:
            return fn(Typing(ms, types)) < limit
        except Unreadable:
            return None
        finally:
            os.getloadavg = saved
    for name, fn in (("letter", letter_to_suggestions_ms), ("return", return_to_first_rows_ms)):
        if run(fn, 12.0) is not True:
            bad.append("%s timing not green under the budget" % name)
        if run(fn, 80.0) is not False:
            bad.append("%s timing over the budget not red" % name)
        if run(fn, 12.0, load=99.0) is not True:
            bad.append("%s timing inside the budget on a loaded Mac not a PASS (journeys/lib/loadgate.py)" % name)
        if run(fn, 80.0, load=99.0) is not None:
            bad.append("%s timing over the budget on a loaded Mac not NOT JUDGED" % name)
    if run(letter_to_suggestions_ms, 12.0, types=False) is not None:
        bad.append("a letter that did not reach the field was judged")
    # the printed measure: a pass and a fail each carry the number the model measured, and main prints every case line
    for ms, want_ok in ((12.0, True), (80.0, False)):
        saved = os.getloadavg
        os.getloadavg = lambda: (0.5, 0.5, 0.5)
        try:
            ok, got, detail = judge_timing(letter_to_suggestions_ms, Typing(ms))
        finally:
            os.getloadavg = saved
        if ok is not want_ok or ("%.1f ms" % ms) not in detail or got != ms:
            bad.append("the timing case line for a %.1f ms model reads %r (judged %r)" % (ms, detail, ok))
    import inspect
    if "print_cases(r)" not in inspect.getsource(main):
        bad.append("main prints no line per judged case (print_cases(r) missing)")
    print("selftest timing cases: %s" % ("both green under %s ms (also on a loaded Mac), red over it, NOT JUDGED over it on a loaded Mac and when the key did not type" % limit if not bad else bad))
    return bad


# ---- doors round (doors-search): every control of the search screen is clicked once and its visible outcome read ----
DOOR_CASES = {"open": "par-door-open-opens-the-search", "clear": "par-door-clear-button-empties-the-field", "bar": "par-door-bar-click-takes-the-keys",
              "tab": "par-door-dropdown-tab-adds-the-chip", "hover": "par-door-dropdown-hover-tints-the-row",
              "recent": "par-door-recent-row-reruns-it", "result": "par-door-result-click-opens-the-thread",
              "cancel": "par-door-save-name-escape-closes-the-field",
              "owner": "par-door-return-after-typing-launches-the-search", "arrows": "par-door-arrows-and-tab-in-the-field-go-to-the-search",
              "esc": "par-door-escape-in-the-field-closes-the-search", "cursor": "par-door-letting-go-gives-up-the-cursor"}


def chip_attachment_or_state(c):
    """True when the bar shows the attachment chip. When it does not, the value is the screen's own state (the bar row, the surface, the chips),
    so the failure line names which state it was in: the save-name field open (`naming`), the search closed (`active`), the words not in the
    field (`text`), the search not launched. A judged result either way, never a crash of the check."""
    kinds = [k["kind"] for k in c.rows(CHIPS)]
    if "attachment" in kinds:
        return True
    b = c.rows(BAR)[0]
    return {"chips": kinds, "naming": b.get("naming"), "active": b.get("active"), "launched": b.get("launched"), "loading": b.get("loading"),
            "focused": b.get("focused"), "owner": b.get("owner"), "text": b.get("text"), "surface": c.uistate().get("surface")}


def open_door(c):
    """The search door (`mac-search.open`, the same act as ⌘K and `/`) from the inbox: the search screen is open with its field live."""
    c.click("mac-search.open")
    return (c.uistate().get("surface"), c.rows(BAR)[0]["active"])


def clear_button(c):
    """The field's clear button after a search ran: the words and the results go, the search stays open."""
    ensure_open(c)                                       # a door judges its own control, never the state the case before it left (item 9bfd333a)
    launch(c, "quote")
    c.click("mac-search.bar.clear")
    b = c.rows(BAR)[0]
    return (b["text"], b["launched"], b["active"], len(c.rows(RES)))


def bar_click_takes_keys(c):
    """A click on a list row releases the field (its words stay); a click on the field takes the keys back with its words."""
    ensure_open(c)
    query(c, "Linea")
    c.click("mac-main.list.row.2")
    released = c.rows(BAR)[0]["focused"]
    c.click(BAR)
    b = c.rows(BAR)[0]
    return (released, b["focused"], b["owner"], b["text"])


def ensure_open(c):
    """The search is open before a door's first click on the field (main92 on: a click on the bar also opens a closed search, SearchMacFeature.swift `mac-search.bar`). The door cases that
    follow the save-name case start here: a door judges its own control, never the state the case before it left (main79: the three doors after the save-name case found
    the search closed). Opens it by its own door (`mac-search.open`) only when the bar says it is not active; the state the case began with stays in the trace."""
    try:
        active = c.rows(BAR)[0].get("active")
    except (Unreadable, IndexError, KeyError, TypeError, AttributeError):
        return
    if active is not True:
        c.click("mac-search.open")


def tab_door(c):
    """The dropdown row's Tab door (the act the Tab key runs on the highlighted row): the person becomes the from chip, the draft empties."""
    ensure_open(c)
    query(c, "Linea")
    c.click("mac-search.dropdown.%d.tab" % first_of(c, "from"))   # the first person row (A153: the domain row may stand above it)
    return ([k["kind"] for k in c.rows(CHIPS)], c.rows(BAR)[0]["draft"])


def hover_door(c):
    """The pointer resting on the third dropdown row gives that row its own tint; the keyboard highlight stays on the first row (owner: Return is
    for the typed words, a pointer resting on a row never changes it). Returns (highlighted row, hovered row)."""
    ensure_open(c)
    query(c, "Linea")
    c.click("mac-search.dropdown.2.hover")
    rows = c.rows(DD)
    first = lambda key: next((i for i, r in enumerate(rows) if r.get(key)), None)
    return (first("highlighted"), first("hovered"))


def recent_door(c):
    """A recent search row: its words fill the field and the search runs again."""
    ensure_open(c)
    launch(c, "quote")
    launch(c, "bravo")
    texts = [r["text"] for r in c.rows("mac-search.recent")]
    c.click("mac-search.recent.%d" % (texts.index("quote") if "quote" in texts else 0))
    b = c.rows(BAR)[0]
    return (b["text"], b["launched"], len(c.rows(RES)) > 0)


def result_click(c):
    """A click on the second result opens that thread in the reader; Escape brings the results back."""
    ensure_open(c)
    ids = [r["id"] for r in c.rows(RES)]
    c.click("mac-search.results.1")
    state = c.uistate().get("surface")
    opened = c.rows("mac-main.reader.subject")[0]["id"]
    c.key("Escape")
    return (state, opened == (ids[1] if len(ids) > 1 else None), c.uistate().get("surface"))


def save_name_escape(c):
    """Save as smart folder opens the name field; Escape closes it and the field shows the saved query again."""
    ensure_open(c)                                       # item 9bfd333a finding 2: the case began on whatever the case before it left
    launch(c, "quote")
    c.click("mac-search.save-as-smart-folder")
    opened = c.rows("mac-search.save-as-smart-folder.field")[0]["open"]
    c.key("Escape")
    b = c.rows(BAR)[0]
    return (opened, c.rows("mac-search.save-as-smart-folder.field")[0]["open"], b["text"], b["launched"])


def leave_search(c):
    """Back to the inbox with the search closed. The owner's ruling: ONE Escape clears the words and leaves the search, so one press is made and the
    surface is read; only a thread opened from a result (the reader) takes a second one, to come back to the results first."""
    for _ in range(2):
        try:
            c.key("Escape")
        except Unreadable:
            pass
        read = getattr(c, "uistate", None)               # a self-test stand-in that serves no surface is one press, the rule
        if read is None or read().get("surface") == "mac-main":
            return


def owner_sequence(c):
    """The owner's sequence (main61, real mail): the list's last row has the focus, ⌘K, the letters one by one, the list's own rows change under the
    open field (a sync removes the focused last row: AppModel.refresh clamps `focusIndex`, no pointer and no key near the list), then Return.
    The field still holds the keys: no quick reply opens, the surface is the search, the typed words were launched.
    What the socket cannot give: the real first responder. `key` calls AppModel.press directly (ControlSocket/Verbs.swift:23), never the shell's
    monitor (ChordKit.handle) that hands Return to a text field with the cursor. So this proves the model keeps the keys through the list's
    change; the field's own Return handler (SearchField.onKeyPress(.return)) is reached only by a real key event."""
    leave_search(c)
    n = len(c.rows(MAIN_LIST))
    c.click("mac-main.list.row.%d" % (n - 1))
    c.key("\u2318K")
    for ch in "hello":
        c.type(ch)
    c.click("mac-search.rig.archive-focused-row")
    time.sleep(0.5)                                  # the view's next update: the onChange of focusIndex runs here
    c.key("Enter")
    b = c.rows(BAR)[0]
    return (c.uistate().get("surface"), len(c.rows("mac-main.quickreply.field")), b["launched"], b["text"], b["focused"])


def field_arrows_and_tab(c):
    """After ⌘K and the letters of "Linea" typed one by one: Down moves the dropdown's highlight, Up moves it back, Down then Tab makes the
    highlighted person the from chip. Never the list's focus, never a reply. The surface stays the search's."""
    leave_search(c)
    c.key("\u2318K")
    for ch in "Linea":
        c.type(ch)
    hl = lambda: [r["highlighted"] for r in c.rows(DD)].index(True)
    c.key("Down")
    down = hl()
    c.key("Up")
    up = hl()
    for _ in range(first_of(c, "from")):              # down to the first person row (A153: the domain row may stand above it)
        c.key("Down")
    c.key("Tab")
    return (down, up, [k["kind"] for k in c.rows(CHIPS)], c.uistate().get("surface"), len(c.rows("mac-main.quickreply.field")))


def field_escape(c):
    """⌘K, letters one by one, Esc: the words and the dropdown go and the search closes on the inbox."""
    leave_search(c)
    c.key("\u2318K")
    for ch in "Linea":
        c.type(ch)
    c.key("Escape")
    b = c.rows(BAR)[0]
    return (c.uistate().get("surface"), b["text"], b["active"])


def letting_go_gives_up_the_cursor(c):
    """When the search lets go of the keys (a click on a list row) the text field gives up its cursor too, so a later Return cannot land in a field the
    model no longer owns. Read from the bar row's `cursor` (the field's own focus), polled for the view's next update."""
    leave_search(c)
    c.key("\u2318K")
    c.type("he")
    c.click("mac-main.list.row.2")
    end, b = time.time() + 2.0, c.rows(BAR)[0]
    while b["cursor"] and time.time() < end:
        time.sleep(0.05)
        b = c.rows(BAR)[0]
    return (b["focused"], b["cursor"])


DOOR_EXPECT = {"open": ("mac-search", True), "clear": ("", False, True, 0), "bar": (False, True, "search", "Linea"), "tab": (["from"], ""),
               "hover": (0, 2), "recent": ("quote", True, True), "result": ("mac-reader", True, "mac-search"), "cancel": (True, False, "quote", True),
               "owner": ("mac-search", 0, True, "hello", False), "arrows": (1, 0, ["from"], "mac-search", 0), "esc": ("mac-main", "", False), "cursor": (False, False)}
DOOR_JUDGE = {"open": open_door, "clear": clear_button, "bar": bar_click_takes_keys, "tab": tab_door, "hover": hover_door, "recent": recent_door,
              "result": result_click, "cancel": save_name_escape, "owner": owner_sequence, "arrows": field_arrows_and_tab, "esc": field_escape, "cursor": letting_go_gives_up_the_cursor}
DOOR_ORDER = ("open", "clear", "recent", "result", "cancel", "tab", "hover", "bar", "owner", "arrows", "esc", "cursor")      # the order the journey runs them in


SEARCH_FIELDS = ("active", "text", "launched", "naming", "focused", "owner", "surface")


def search_snapshot(c):
    """The search state a refused click needs explained: the bar row (active, text, launched, naming, focused, owner) and the surface. Never raises;
    a field the build does not serve reads "absent"."""
    out = {}
    try:
        b = c.rows(BAR)
        b = b[0] if b else {}
    except (Unreadable, IndexError, KeyError, TypeError, AttributeError, ValueError) as e:
        b = {}
        out["bar"] = "unreadable (%s)" % e
    for f in SEARCH_FIELDS[:-1]:
        out[f] = b.get(f, "absent") if isinstance(b, dict) else "absent"
    try:
        out["surface"] = c.uistate().get("surface", "absent")
    except (Unreadable, KeyError, TypeError, AttributeError, ValueError) as e:
        out["surface"] = "unreadable (%s)" % e
    return out


class DoorTrace:
    """Wraps the socket for the doors step: each click, key and type is recorded with the search state before and after, so a door the app answers
    "did nothing" (the search is inactive) names the state it met and the earlier action that last changed `active`. Reads pass through."""

    def __init__(self, ctl):
        self.c, self.case, self.log = ctl, "start", []
        self.start = search_snapshot(ctl)
        self.entered = self.start

    def __getattr__(self, name):
        return getattr(self.c, name)

    def begin(self, case):
        self.case = case
        self.entered = search_snapshot(self.c)      # main79: the surface and the search owner as the case begins, before its first query() click

    def _do(self, verb, arg):
        before = search_snapshot(self.c)
        try:
            r = getattr(self.c, verb)(arg)
        except Unreadable as e:
            self.log.append((self.case, verb, arg, before, None, str(e)))
            raise Unreadable("%s | the search before this %s: %s | %s | %s | %s" % (e, verb, before, self.last_change(), self.began(), self.trail()))
        self.log.append((self.case, verb, arg, before, search_snapshot(self.c), None))
        return r

    def click(self, ax):
        return self._do("click", ax)

    def key(self, chord):
        return self._do("key", chord)

    def type(self, text):
        return self._do("type", text)

    def last_change(self):
        """Which earlier action last changed `active` (case, action, before -> after), or that none did since the step began."""
        for case, verb, arg, before, after, refused in reversed(self.log):
            if after is not None and after.get("active") != before.get("active"):
                return "active last changed by %s %r in case %s: %r -> %r" % (verb, arg, case, before.get("active"), after.get("active"))
        return "active never changed since the doors step began (it started %r; search at the start %s)" % (self.start.get("active"), self.start)

    def began(self):
        return "the case began with: %s" % (self.entered,)

    def trail(self):
        """Each action of this case so far with the surface, the search owner and `active` it left behind: where the first Enter or click went."""
        steps = ["%s %r -> surface %r, owner %r, active %r" % (verb, arg, (after or {}).get("surface"), (after or {}).get("owner"), (after or {}).get("active"))
                 for case, verb, arg, before, after, refused in self.log if case == self.case and after is not None]
        return "this case so far: %s" % ("; ".join(steps) if steps else "no action yet")

    def why(self):
        now = search_snapshot(self.c)
        return "search now %s | %s | %s | %s" % (now, self.last_change(), self.began(), self.trail())


def door_result(tr, k):
    """One door case through the tracer: (ok, got, detail). A refused click re-raises with the search state it met (the harness records it not judged)."""
    tr.begin(DOOR_CASES[k])
    got = DOOR_JUDGE[k](tr)
    return got == DOOR_EXPECT[k], got, "expected %r, got %r; %s" % (DOOR_EXPECT[k], got, tr.why())


class DoorsModel:
    """The self-test's stand-in for the search screen's controls: the calls of the socket (rows, click, key, type, uistate). `mutant` names the
    control that does nothing; with none it is the correct behaviour."""
    IDS = ["msg-a", "msg-b", "msg-c"]

    def __init__(self, mutant=None):
        self.m, self.surface, self.active, self.text, self.launched = mutant, "mac-main", False, "", False
        self.recents, self.focused, self.naming, self.saved, self.hl, self.chips, self.reader = [], False, False, "", 0, [], None
        self.focus, self.nrows, self.claimed_at, self.claimed_rows, self.reply, self.cursor = 1, 6, 0, 0, False, False
        self.hv = None

    def type(self, t):
        self.text += t

    def launch(self):
        if self.text:
            self.recents = [self.text] + [r for r in self.recents if r != self.text]
            self.launched, self.naming, self.hl = True, False, 0

    def release(self):
        self.focused = False
        self.surface = "mac-main"
        if self.m != "cursor-kept":
            self.cursor = False

    def moved(self):
        """SearchMacModel.focusMoved: the list's focus changed while the field holds the keys."""
        if not (self.focused and self.surface == "mac-search") or self.naming or self.focus == self.claimed_at:
            return
        if self.nrows != self.claimed_rows and self.m != "focus-guard-removed":
            self.claimed_rows, self.claimed_at = self.nrows, self.focus      # the list's own data moved: re-anchor, keep the keys
            return
        self.release()

    def key(self, k):
        if k == "⌘A":
            self.text, self.chips = "", []
        elif k == "⌘K":
            self.active, self.surface, self.focused, self.cursor = True, "mac-search", True, True
            self.claimed_at, self.claimed_rows = self.focus, self.nrows
        elif k in ("Down", "Up", "Tab"):
            if self.m == "field-keys-to-list":
                self.focus += 1                          # the list took the key
            elif k == "Down":
                self.hl = min(self.hl + 1, 4)
            elif k == "Up":
                self.hl = max(self.hl - 1, 0)
            elif self.hl > 0:
                self.chips, self.text = ["from"], 'from:"Linea Supply"'
        elif k == "Enter":
            if self.surface == "mac-search":
                self.launch()
                self.focused = self.cursor = False     # lead, 13 Nov (owner A162): after Return the keys are on the results (SearchFocus.toResults)
            else:
                self.reply = True                      # Commands.enter on mac-main: the list's Enter opens the quick reply
        elif k == "Escape":
            if self.naming:
                if self.m == "cancel-closes-search":
                    self.naming, self.active, self.surface, self.launched, self.focused, self.cursor, self.text = False, False, "mac-main", False, False, False, ""
                elif self.m != "cancel-dead":
                    self.naming, self.text = False, self.saved
            elif self.surface == "mac-reader":
                self.surface = "mac-search"
            else:
                self.active, self.surface, self.launched, self.focused, self.cursor, self.reply = False, "mac-main", False, False, False, False
                if self.m != "escape-keeps-text":
                    self.text = ""

    def click(self, ax):
        m = self.m
        if ax == "mac-search.open":
            if m != "open-dead":
                self.active, self.surface, self.focused = True, "mac-search", True
                self.claimed_at, self.claimed_rows = self.focus, self.nrows
        elif self.m in ("search-closed", "cancel-closes-search") and ax in (BAR, "mac-search.save-as-smart-folder", "mac-search.dropdown.1.tab", "mac-search.dropdown.2.hover") and not self.active:
            raise Unreadable("click %s: %s did nothing" % (ax, ax))        # the app's own answer: the handler refuses while the search is inactive
        elif ax == BAR:
            if self.active:
                self.surface, self.focused = "mac-search", True if m != "bar-dead" else self.focused
                self.claimed_at, self.claimed_rows = self.focus, self.nrows
        elif ax.startswith("mac-main.list.row."):
            was = self.focused and self.surface == "mac-search"
            self.focus = int(ax.rsplit(".", 1)[1])
            if was:
                self.moved()
            else:
                self.surface, self.focused = "mac-main", False
        elif ax == "mac-search.rig.archive-focused-row":
            if self.surface != "mac-search" or self.nrows < 2 or self.focus != self.nrows - 1:
                raise Unreadable("click %s: refused" % ax)
            self.nrows -= 1
            self.focus = min(self.focus, self.nrows - 1)
            self.moved()
        elif ax == "mac-search.bar.clear":
            if m != "clear-dead":
                self.text, self.launched, self.chips = "", False, []
        elif ax == "mac-search.dropdown.1.tab":
            if m != "tab-dead":
                self.chips, self.text = ["from"], 'from:"Linea Supply"'
        elif ax == "mac-search.dropdown.2.hover":
            if m != "hover-dead":
                self.hv = 2
        elif ax.startswith("mac-search.recent."):
            if m != "recent-dead":
                self.text, self.launched = self.recents[int(ax.rsplit(".", 1)[1])], True
        elif ax == "mac-search.results.1":
            if m != "result-dead":
                self.surface, self.reader = "mac-reader", self.IDS[1]
                if m == "search-closed":
                    self.active = False                    # SearchMacActs.openResult calls leave()
        elif ax == "mac-search.save-as-smart-folder":
            if self.launched and m != "save-dead":
                self.naming, self.saved = True, self.text
        else:
            raise Unreadable("click %s: no such control" % ax)

    def uistate(self):
        return {"surface": self.surface}

    def rows(self, region):
        draft = "" if self.chips else self.text
        if region == BAR:
            return [{"text": self.saved if self.naming else self.text, "launched": self.launched, "active": self.active, "focused": self.focused,
                     "owner": "search" if self.focused else "other", "draft": draft, "naming": self.naming, "cursor": self.cursor}]
        if region == MAIN_LIST:
            return [{"id": "row-%d" % i} for i in range(self.nrows)]
        if region == "mac-main.quickreply.field":
            return [{}] if self.reply else []
        if region == CHIPS:
            return [{"kind": k} for k in self.chips]
        if region == RES:
            return [{"id": i} for i in self.IDS] if self.launched else []
        if region == DD:
            return [{"highlighted": i == self.hl, "hovered": i == self.hv} for i in range(5)]
        if region == "mac-search.recent":
            return [{"text": r} for r in self.recents]
        if region == "mac-search.save-as-smart-folder.field":
            return [{"open": self.naming}]
        if region == "mac-main.reader.subject":
            return [{"id": self.reader}]
        return []


def doors_run(mutant):
    m = DoorsModel(mutant)
    tr = DoorTrace(m)
    out, said = {}, {}
    for k in DOOR_ORDER:
        try:
            ok, _got, said[DOOR_CASES[k]] = door_result(tr, k)
            out[DOOR_CASES[k]] = ok
        except Unreadable as e:
            out[DOOR_CASES[k]], said[DOOR_CASES[k]] = False, "not judged: %s" % e
        except IndexError as e:
            out[DOOR_CASES[k]], said[DOOR_CASES[k]] = False, "IndexError %s" % e                 # a row that is not there is a red case
    doors_run.said = said
    return out


DOORS_MUTANTS = {"open-dead": "open", "clear-dead": "clear", "bar-dead": "bar", "tab-dead": "tab", "hover-dead": "hover", "recent-dead": "recent",
                 "result-dead": "result", "cancel-dead": "cancel", "cancel-closes-search": "cancel", "focus-guard-removed": "owner", "field-keys-to-list": "arrows", "escape-keeps-text": "esc", "cursor-kept": "cursor"}


def doors_selftest():
    bad = []
    base = doors_run(None)
    print("model, doors, correct behaviour: %s" % base)
    bad += ["%s not green on the correct model" % k for k, v in base.items() if not v]
    for mutant, key in DOORS_MUTANTS.items():
        target = DOOR_CASES[key]
        got = doors_run(mutant)
        print("mutant %-14s target %-48s red=%s" % (mutant, target, not got[target]))
        if got[target]:
            bad.append("mutant %s stays green on %s" % (mutant, target))
    declared = set(re.findall(r"^# J2\.9/(par-door-[\w-]+) \|", MUTANTS, re.M))
    bad += ["no mutant line declared for %s" % k for k in base if k not in declared]
    # the evidence a refused click carries: the search state it met and the earlier action that last changed `active`
    got = doors_run("search-closed")
    said = doors_run.said
    refused = [k for k in DOOR_CASES.values() if not got[k] and said[k].startswith("not judged")]
    for k in refused:
        print("NOT JUDGED %s: %s" % (k, said[k]))
        for need in ("did nothing", "the search before this click: {'active': False", "active last changed by click 'mac-search.results.1' in case par-door-result-click-opens-the-thread: True -> False",
                     "the case began with: {", "this case so far: "):
            if need not in said[k]:
                bad.append("the refusal text of %s lacks %r" % (k, need))
    if not refused and not got[DOOR_CASES["cancel"]]:
        # round 9 (item 9bfd333a): every door that clicks the field opens the search first (ensure_open), so a search closed by the case before is no
        # longer a refused click; the evidence text above is judged when a door is refused, and the save-name door must now recover
        bad.append("the search-closed mutant: the save-name door neither recovered by ensure_open nor was refused with its evidence")
    for k in ("par-door-clear-button-empties-the-field", "par-door-open-opens-the-search"):
        if not got[k]:
            bad.append("the search-closed mutant reddened %s, a door that runs before the result click" % k)
    # the doors after the save-name case judge their own control: with the save-name Escape closing the whole search, the case itself is red and the three doors
    # that follow it (tab, hover, bar) stay green because each opens the search first
    got = doors_run("cancel-closes-search")
    for k in ("tab", "hover", "bar"):
        if not got[DOOR_CASES[k]]:
            bad.append("with the search closed by the save-name Escape, %s is red: a door must not inherit the state of the case before it" % DOOR_CASES[k])
    if got[DOOR_CASES["cancel"]]:
        bad.append("the save-name case stays green when its Escape closes the search")
    got = doors_run("tab-dead")
    print("FAIL %s: %s" % (DOOR_CASES["tab"], doors_run.said[DOOR_CASES["tab"]]))
    if "search now {" not in doors_run.said[DOOR_CASES["tab"]]:
        bad.append("a compare failure does not carry the search state")
    print("%d door checks, %d mutants, %d problems" % (len(base), len(DOORS_MUTANTS), len(bad)))
    for b in bad:
        sys.stderr.write("SELFTEST FAIL %s\n" % b)
    return bad


class SearchModel:
    """The self-test's stand-in for the search screen: the socket's calls (rows, core, click, key, type), answering what the built screen
    must answer. `mutant` names one defect; with none it is the correct behaviour. No app is launched."""
    WORDS = {"quote": ["thr-01", "thr-s05", "thr-s06"], "pallets": ["thr-s02"]}

    def __init__(self, mutant=None):
        import datetime
        self.mutant, self.text, self.shown, self.launched, self.last_q = mutant, "", "", False, None
        self.recents, self.typed_now = ['from:"Linea Supply" has:attachment'], False

    def click(self, ax):
        pass

    def type(self, t):
        self.text += t
        self.typed_now = True
        if self.launched and self.mutant != "chips-wait-for-enter" and ("from:" in t) and not (self.mutant == "filters-alone-wait" and not self.words(self.text)):
            # narrowLive before round 13: the list was rebuilt only when the chips differed from the shown ones, so a filter typed over the words
            # (the same chip, the words gone) changed nothing
            if self.mutant == "filters-alone-same-chips" and not self.words(self.text) and self.chips_of(self.text) == self.chips_of(self.shown):
                return
            self.shown = self.text

    @staticmethod
    def chips_of(text):
        return re.findall(r'from:"[^"]*"|before:\S+|has:attachment', text)

    def key(self, k):
        if k == "⌘A":
            self.text = ""
        elif k == "Enter":
            if self.mutant == "highlight-stale" and self.typed_now:
                self.text = self.recents[0]               # the old text's highlighted recent row is accepted, the typed words are lost
            self.typed_now = False
            self.launch()

    @staticmethod
    def words(text):
        return " ".join(w for w in text.split() if ":" not in w and w not in ("Linea", "Supply\"")).strip()

    def launch(self):
        self.shown = self.text
        self.launched = True
        if self.words(self.text):
            self.last_q = self.words(self.text)
        self.recents.insert(0, self.text)

    def threads(self):
        t = self.shown
        words = self.words(t)
        base = self.WORDS.get(words, sorted({m["thread"] for m in MSGS})) if words or not t.strip() else sorted({m["thread"] for m in MSGS})
        if 'from:"Linea Supply"' in t:
            base = [x for x in (self.WORDS.get(words, LINEA_THREADS) if words else LINEA_THREADS) if x in LINEA_THREADS]
        return base

    def core(self, path):
        return self.last_q if path == "search/last/query" else None

    def rows(self, region):
        t = self.shown
        if region == BAR:
            return [{"text": self.text}]
        if region == "mac-search.recent":
            return [{"text": x} for x in self.recents]
        if region == RES:
            tints = {"thr-01": ["Quote", "run"], "thr-s05": ["run"]} if self.mutant != "tint-empty" else {}
            return [{"thread": x, "highlights": tints.get(x, [])} for x in self.threads()]
        return []


ROUND12_MUTANTS = {"highlight-stale": STALE_ENTER, "chips-wait-for-enter": LIVE_CASES[0], "filters-alone-wait": LIVE_CASES[1],
                   "tint-empty": VIEW_CASES[1], "filters-alone-same-chips": LIVE_CASES[1]}


def round12_run(mutant):
    """The new cases in the order the journey runs them, on one model."""
    m = SearchModel(mutant)
    out = {}
    launch(m, "quote")
    before = threads_shown(m)
    out[LIVE_CASES[0]] = chip_narrows_live(m, before) == (True, True, True, True)
    out[LIVE_CASES[1]] = filters_alone_live(m) == sorted(LINEA_THREADS)
    out[STALE_ENTER] = enter_takes_typed(m, "pallets") == ("pallets", "pallets")
    m.text = WORDS_QUERY; m.launch()
    out[VIEW_CASES[1]] = tinted(m) == WORDS_TINT
    return out


# ---- return-in-the-field (lane search-return): Return runs the typed words; only an arrow key or a click on a row makes Return take a row ----
RETURN_CASES = ("par-return-pointer-over-row-runs-typed", "par-return-after-down-takes-the-row", "par-return-down-then-letter-runs-typed", "par-click-on-row-takes-it")
RETURN_WORDS = "Linea"


def fresh_field(c):
    """The search closed, then opened by ⌘K with the letters typed one by one."""
    leave_search(c)
    c.key("\u2318K")
    for ch in RETURN_WORDS:
        c.type(ch)


def after_launch(c):
    """(bar text, launched, first recent search) once the search ran."""
    b = c.rows(BAR)[0]
    rec = c.rows("mac-search.recent")
    return (b["text"], b["launched"], rec[0]["text"] if rec else None)


def return_pointer_over_row(c):
    """The pointer rests on the third row (a Sender contains / person row), then Return: the typed words run and the recent list keeps them.
    Read before Return too: the highlight is still the first row and the pointer's row has its own mark."""
    fresh_field(c)
    c.click("mac-search.dropdown.2.hover")
    rows = c.rows(DD)
    first = lambda key: next((i for i, r in enumerate(rows) if r.get(key)), None)
    before = (first("highlighted"), first("hovered"))
    c.key("Enter")
    return before + after_launch(c)


def return_after_down(c):
    """Down moves to the second row (a person), Return takes it: the from chip is the launched search."""
    fresh_field(c)
    for _ in range(first_of(c, "from")):              # down to the first person row (A153: the domain row may stand above it)
        c.key("Down")
    c.key("Enter")
    text, launched, _ = after_launch(c)
    return (text.startswith('from:"'), launched, [k["kind"] for k in c.rows(CHIPS)])


def return_down_then_letter(c):
    """Down, then one more letter, then Return: the typed words (with that letter) run; the arrow's row is forgotten."""
    fresh_field(c)
    c.key("Down")
    c.type("x")
    c.key("Enter")
    return after_launch(c)


def click_on_row(c):
    """A click on the second row takes that row, as Return would after an arrow."""
    fresh_field(c)
    c.click("mac-search.dropdown.%d" % first_of(c, "from"))   # the first person row (A153: the domain row may stand above it)
    text, launched, _ = after_launch(c)
    return (text.startswith('from:"'), launched, [k["kind"] for k in c.rows(CHIPS)])


RETURN_EXPECT = {RETURN_CASES[0]: (0, 2, RETURN_WORDS, True, RETURN_WORDS), RETURN_CASES[1]: (True, True, ["from"]),
                 RETURN_CASES[2]: (RETURN_WORDS + "x", True, RETURN_WORDS + "x"), RETURN_CASES[3]: (True, True, ["from"])}
RETURN_JUDGE = {RETURN_CASES[0]: return_pointer_over_row, RETURN_CASES[1]: return_after_down, RETURN_CASES[2]: return_down_then_letter, RETURN_CASES[3]: click_on_row}


class ReturnModel:
    """The self-test's stand-in for the field and its dropdown (5 rows: Search all mail, two people, Sender contains, Subject contains). `mutant`
    names one defect; with none it is the rule: Return takes a row only after an arrow key or a click on it, never for a pointer resting on it."""
    CHIP = 'from:"Linea Supply"'

    def __init__(self, mutant=None):
        self.m, self.text, self.hl, self.hv, self.keyed, self.launched, self.recents, self.chips = mutant, "", 0, None, False, False, [], []

    def type(self, t):
        self.text += t
        if self.m != "keyed-survives-typing":             # the mutant: the arrow's row stays chosen for the new text
            self.hl, self.keyed = 0, False

    def launch(self):
        self.recents.insert(0, self.text)
        self.launched = True

    def take(self, i):
        if 1 <= i <= 2:                                   # a person row: the from chip replaces the draft
            self.text, self.chips = self.CHIP, ["from"]
        self.launch()

    def key(self, k):
        if k == "\u2318K":
            self.text, self.hl, self.hv, self.keyed, self.launched, self.recents, self.chips = "", 0, None, False, False, [], []
        elif k == "Down":
            self.hl, self.keyed = min(self.hl + 1, 4), True
        elif k == "Enter":
            if self.m == "return-ignores-arrow":
                self.launch()
            elif (self.keyed or self.m == "hover-moves-highlight") and self.hl > 0:
                self.take(self.hl)
            else:
                self.launch()
        elif k == "Escape":
            self.text, self.hl, self.hv, self.keyed, self.launched, self.chips = "", 0, None, False, False, []

    def click(self, ax):
        if ax == "mac-search.dropdown.2.hover":
            self.hv = 2
            if self.m == "hover-moves-highlight":
                self.hl = 2
        elif ax == "mac-search.dropdown.1":
            if self.m != "click-row-dead":
                self.hl, self.keyed = 1, True
                self.take(1)
        else:
            raise Unreadable("click %s: no such control" % ax)

    def rows(self, region):
        if region == BAR:
            return [{"text": self.text, "launched": self.launched}]
        if region == DD:
            return [{"highlighted": i == self.hl, "hovered": i == self.hv} for i in range(5)]
        if region == "mac-search.recent":
            return [{"text": r} for r in self.recents]
        if region == CHIPS:
            return [{"kind": k} for k in self.chips]
        return []


def return_run(mutant):
    m = ReturnModel(mutant)
    m.key("\u2318K")
    out = {}
    for k in RETURN_CASES:
        out[k] = RETURN_JUDGE[k](m) == RETURN_EXPECT[k]
    return out


RETURN_MUTANTS = {"hover-moves-highlight": RETURN_CASES[0], "return-ignores-arrow": RETURN_CASES[1], "keyed-survives-typing": RETURN_CASES[2], "click-row-dead": RETURN_CASES[3]}


def return_selftest():
    bad = []
    base = return_run(None)
    print("model, return in the field, correct behaviour: %s" % base)
    bad += ["%s not green on the correct model" % k for k, v in base.items() if not v]
    for mutant, target in RETURN_MUTANTS.items():
        got = return_run(mutant)
        print("mutant %-22s target %-46s red=%s" % (mutant, target, not got[target]))
        if got[target]:
            bad.append("mutant %s stays green on %s" % (mutant, target))
    declared = set(re.findall(r"^# J2\.1/(par-(?:return|click)-[\w-]+) \|", MUTANTS, re.M))
    bad += ["no mutant line declared for %s" % k for k in RETURN_CASES if k not in declared]
    print("%d return checks, %d mutants, %d problems" % (len(RETURN_CASES), len(RETURN_MUTANTS), len(bad)))
    for b in bad:
        sys.stderr.write("SELFTEST FAIL %s\n" % b)
    return bad


# lane miss-search-bar (owner: "every control with a bound key shows it", "shortcuts are contextual"): the bottom key bar during a search shows the search's keys
FOOT = "mac-main.foot.hints"
BAR_CASES = ("par-bar-shows-search-keys-during-search", "par-bar-is-the-list-set-after-search-closes")
BAR_COMPOSER = {"send", "ai-reply", "discard-draft", "popout-draft"}   # the composer set's commands (MacFootHints.sets[.composer])
BAR_SEARCH = ["enter", "archive", "reply", "snooze", "search", "back"]   # after Return the keys are on the results (lead, 13 Nov, owner A162): open, the result's own acts, back into the words, close
BAR_SEARCH_OWN = {"enter", "back"}                                     # the two commands only the search set shows


def bar_items(c):
    """(surface of the bar's items, its commands without the last 'hold-cmd' item)."""
    rows = c.rows(FOOT)
    return (sorted({r["surface"] for r in rows}), [r["command"] for r in rows if r["command"] != "hold-cmd"])


def say_trace(line):
    """One TRACE line on the run's output (stdout), for a case whose end state alone does not show its cause."""
    print("TRACE      " + line, flush=True)


def bar_during_search(c):
    """The item's sequence: Enter, Escape, Cmd-K, a typed word, Return (a search validated). The bar holds the search's keys and none of the composer's."""
    leave_search(c)
    def trace(after):                                    # each step's state on the log (main93 to main95: three builds red with only the end state)
        try:
            st, b = c.uistate(), c.rows(BAR)[0]
            taker = (c.rows("mac-keyevent.monitor") or [{}])[0].get("taker")
            say_trace("bar-during-search after %s: surface %r, focus %r, overlay %r, field text %r, launched %r, holds %r, active %r, key taken by %r, keys last let go by %r" % (
                after, st.get("surface"), st.get("first_responder"), st.get("overlay"), b.get("text"), b.get("launched"), b.get("focused"), b.get("active"), taker,
                b.get("released_by")))
        except Exception as e:                           # noqa: BLE001  a trace never stops the case
            say_trace("bar-during-search after %s: unreadable (%s)" % (after, type(e).__name__))
    for k in ("Enter", "Escape", "\u2318K"):
        c.key(k)
        trace(k)
    for ch in RETURN_WORDS:
        c.type(ch)
    trace("typing")
    c.key("Enter")
    trace("Return")
    surfaces, cmds = bar_items(c)
    return (surfaces, cmds, bool(BAR_COMPOSER & set(cmds)))


def bar_after_close(c):
    """The search closed (one Escape): the bar is the list's set again, with none of the search's own commands."""
    leave_search(c)
    surfaces, cmds = bar_items(c)
    return (surfaces, bool(BAR_SEARCH_OWN & set(cmds)), "archive" in cmds)


BAR_EXPECT = {BAR_CASES[0]: (["search"], BAR_SEARCH, False), BAR_CASES[1]: (["list"], False, True)}
BAR_JUDGE = {BAR_CASES[0]: bar_during_search, BAR_CASES[1]: bar_after_close}
SEARCH_SET, LIST_SET, COMPOSER_SET = list(BAR_SEARCH), ["archive", "reply", "reply-all", "forward", "snooze", "label", "search"], ["send", "ai-reply", "discard-draft", "popout-draft"]


class BarModel:
    """The self-test's stand-in for the key bar and the search (the keys of MacFootHints.surface). `mutant` names one defect; with none it is the rule:
    the search's set while the field holds the keys or a validated search shows, the list's set once it closes."""
    def __init__(self, mutant=None):
        self.m, self.holds, self.active, self.launched = mutant, False, False, False

    def key(self, k):
        if k == "\u2318K":
            self.holds = self.active = True
            self.launched = False
        elif k == "Enter" and self.holds:
            self.launched = True
        elif k == "Escape":
            self.holds = self.active = self.launched = False

    def type(self, ch):
        pass

    def rows(self, region):
        if region != FOOT:
            raise Unreadable("rows %s: no such region" % region)
        searching = self.holds or (self.active and self.launched)
        if searching:
            self.seen = True
        if searching and self.m == "composer-in-search":
            surface, cmds = "composer", COMPOSER_SET
        elif searching or (self.m == "search-stays-after-close" and getattr(self, "seen", False)):
            surface, cmds = "search", SEARCH_SET
        else:
            surface, cmds = "list", LIST_SET
        return [{"surface": surface, "command": x} for x in cmds + ["hold-cmd"]]


BAR_MUTANTS = {"composer-in-search": BAR_CASES[0], "search-stays-after-close": BAR_CASES[1]}


def bar_run(mutant):
    m = BarModel(mutant)
    return {k: BAR_JUDGE[k](m) == BAR_EXPECT[k] for k in BAR_CASES}


def bar_selftest():
    bad = []
    base = bar_run(None)
    print("model, key bar during a search, correct behaviour: %s" % base)
    bad += ["%s not green on the correct model" % k for k, v in base.items() if not v]
    for mutant, target in BAR_MUTANTS.items():
        got = bar_run(mutant)
        print("mutant %-24s target %-46s red=%s" % (mutant, target, not got[target]))
        if got[target]:
            bad.append("mutant %s stays green on %s" % (mutant, target))
    declared = set(re.findall(r"^# J2\.1/(par-bar-[\w-]+) \|", MUTANTS, re.M))
    bad += ["no mutant line declared for %s" % k for k in BAR_CASES if k not in declared]
    print("%d key bar checks, %d mutants, %d problems" % (len(BAR_CASES), len(BAR_MUTANTS), len(bad)))
    for b in bad:
        sys.stderr.write("SELFTEST FAIL %s\n" % b)
    return bad


# ---- lane reader-result (SPEC A148): the reader shows the selected result wherever the mail is; Enter opens it with the results kept ----
# Owner 2026-11-11: "when I go enter on this email , it doesn't open it, it just goes straight to no search"; "we don't even see email".
# `to:fay` on the test engine lists its sent seed (FixtureSearchBook.sentSeed): three mails in NO inbox tab, so no result is a loaded inbox row.
# Only the launch corpus carries the seed (FixtureCore.resetMessages), so the step runs after `fixture reset`.
FAY_QUERY = "to:fay"
FAY_IDS = ["sent-seed-1", "sent-seed-2", "sent-seed-3"]
RESULT_CASES = ("par-reader-shows-a-result-outside-the-inbox-rows", "par-enter-opens-the-result-and-keeps-the-results",
                "par-arrows-walk-the-results-and-escape-returns-to-them")


def reader_id(c):
    """The reader's mail (`mac-main.reader.subject` row id); None when the reader has no conversation (the probe refuses)."""
    try:
        rows = c.rows("mac-main.reader.subject")
    except Unreadable:
        return None
    return rows[0].get("id") if rows else None


def selected_id(c):
    sel = [r["id"] for r in c.rows(RES) if r.get("selected")]
    return sel[0] if len(sel) == 1 else None


def reader_after_quiet(c, want):
    """The reader's mail once it settled on `want`, or what it shows when READER_WAIT_S has gone by (the light-focus quiet time, ReaderFocus)."""
    end = time.time() + READER_WAIT_S
    while True:
        got = reader_id(c)
        if got == want or time.time() > end:
            return got
        time.sleep(0.05)


def result_outside_inbox(c):
    """`to:fay` launched: three results, all sent-seed mail, and the reader's mail id is the selected result's (not nil, not an inbox row)."""
    launch(c, FAY_QUERY)
    ids = [r["id"] for r in c.rows(RES)]
    sel = selected_id(c)
    return (sorted(ids), sel is not None and reader_after_quiet(c, sel) == sel)


RESULT_OUTSIDE_EXPECT = (FAY_IDS, True)


def enter_opens_result(c):
    """Enter on the selected result: the reader surface, the reader's mail is that result, the three results stay listed with the same selection."""
    sel = selected_id(c)
    c.key("Enter")
    surface = c.uistate().get("surface")
    opened = reader_after_quiet(c, sel)
    return (surface, sel is not None and opened == sel, len(c.rows(RES)), selected_id(c) == sel)


ENTER_OPENS_EXPECT = ("mac-reader", True, 3, True)


def arrows_and_escape(c):
    """In the reader on a result: Down selects the next result and the reader follows; Esc gives the results back (search surface, the field's
    words as they were, the same selection); a second Esc closes the search."""
    words, sel = c.rows(BAR)[0].get("text"), selected_id(c)
    c.key("Down")
    nxt = selected_id(c)
    follows = nxt is not None and nxt != sel and reader_after_quiet(c, nxt) == nxt
    c.key("Escape")
    back = (c.uistate().get("surface"), c.rows(BAR)[0].get("text") == words, selected_id(c) == nxt)
    c.key("Escape")
    return (follows, back, c.uistate().get("surface"))


ARROWS_ESCAPE_EXPECT = (True, ("mac-search", True, True), "mac-main")
RESULT_JUDGE = {RESULT_CASES[0]: (result_outside_inbox, RESULT_OUTSIDE_EXPECT), RESULT_CASES[1]: (enter_opens_result, ENTER_OPENS_EXPECT),
                RESULT_CASES[2]: (arrows_and_escape, ARROWS_ESCAPE_EXPECT)}


class ResultModel:
    """The self-test's stand-in for the search screen and the reader with a result outside the inbox rows: the socket's calls (rows, click, key,
    type, uistate), answering what the built app must answer. `mutant` names one defect; with none it is the correct behaviour. No app is launched."""
    INBOX = ["msg-inbox-0", "msg-inbox-1"]

    def __init__(self, mutant=None):
        self.m, self.surface, self.active, self.launched = mutant, "mac-main", False, False
        self.text, self.launched_text, self.sel, self.focus, self.focused = "", "", 0, 0, False

    def type(self, t):
        self.text += t

    def click(self, ax):
        if ax == BAR:
            self.active, self.surface, self.focused = True, "mac-search", True

    def close(self):
        self.active, self.launched, self.text, self.launched_text, self.surface, self.focused = False, False, "", "", "mac-main", False

    def key(self, k):
        if k == "⌘A":
            self.text = ""
        elif k == "Enter" and self.surface == "mac-search":
            if not self.launched or self.text != self.launched_text:
                self.launched, self.launched_text, self.sel = True, self.text, 0
            elif self.m == "enter-leaves-search":
                self.close()
                self.surface = "mac-reader"                  # the old openResult: leave(), the reader takes the focused inbox row
            else:
                self.surface, self.focused = "mac-reader", False
        elif k in ("Down", "Up"):
            d = 1 if k == "Down" else -1
            if self.surface == "mac-reader" and self.m == "keys-walk-inbox":
                self.focus = min(max(0, self.focus + d), len(self.INBOX) - 1)
            elif self.launched and self.active:
                self.sel = min(max(0, self.sel + d), len(FAY_IDS) - 1)
        elif k == "Escape":
            if self.surface == "mac-reader" and self.active:
                if self.m == "escape-to-inbox":
                    self.surface = "mac-main"                # the list's Esc: the reader closes, the field holds no keys
                else:
                    self.surface, self.focused = "mac-search", True
            else:
                self.close()

    def uistate(self):
        return {"surface": self.surface}

    def rows(self, region):
        shows = self.active and self.launched
        if region == BAR:
            return [{"text": self.text, "launched": self.launched, "dropdown": False, "focused": self.focused}]
        if region == RES:
            return [{"id": i, "selected": n == self.sel} for n, i in enumerate(FAY_IDS)] if shows else []
        if region == "mac-main.reader.subject":
            mail = FAY_IDS[self.sel] if shows else self.INBOX[self.focus]
            if self.m == "reader-from-inbox-rows" and mail not in self.INBOX:
                raise Unreadable("rows mac-main.reader.subject: no conversation is open in the reader")
            return [{"id": mail}]
        return []


RESULT_MUTANTS = {"reader-from-inbox-rows": RESULT_CASES[0], "enter-leaves-search": RESULT_CASES[1], "keys-walk-inbox": RESULT_CASES[2],
                  "escape-to-inbox": RESULT_CASES[2]}


def result_run(mutant):
    """The three cases in the journey's order on one model (each case starts where the last one left the screen); {case: green?}."""
    m, out = ResultModel(mutant), {}
    for k in RESULT_CASES:
        fn, want = RESULT_JUDGE[k]
        try:
            out[k] = fn(m) == want
        except (Unreadable, KeyError, IndexError, TypeError):
            out[k] = False
    return out


def result_selftest():
    bad = []
    base = result_run(None)
    print("model, a result outside the inbox rows, correct behaviour: %s" % base)
    bad += ["%s not green on the correct model" % k for k, v in base.items() if not v]
    for mutant, target in RESULT_MUTANTS.items():
        got = result_run(mutant)
        print("mutant %-22s target %-52s red=%s" % (mutant, target, not got[target]))
        if got[target]:
            bad.append("mutant %s stays green on %s" % (mutant, target))
    declared = set(re.findall(r"^# J2\.4/(par-[\w-]+) \|", MUTANTS, re.M))
    bad += ["no mutant line declared for %s" % k for k in RESULT_CASES if k not in declared]
    print("%d reader-result checks, %d mutants, %d problems" % (len(RESULT_CASES), len(RESULT_MUTANTS), len(bad)))
    for b in bad:
        sys.stderr.write("SELFTEST FAIL %s\n" % b)
    return bad


# ---- lane reader-passage (SPEC A152, owner 2026-11-11: "it's subject+body+attachment and they should be all related"; "+thread too btw"; card "P1 The passage on the
# row"): the reader opens at the mail that holds the selected result's passage and marks it. The reader serves `mac-main.reader.passage` (one row): `marked`, `text`,
# `source` (text, attachment, thread), `mail_id` (the mail that holds the passage), `file`/`ring` (an attachment's file name and its ringed tile), `pieces` (the marked
# text), `earlier_index` (the earlier mail of the thread that holds it, else null). The expected values are the RESULT ROW's own (`passage_line`, `source`, `hit_id`),
# never typed here. A kind of result the corpus does not hold is reported as absent (None), which is red: the case is judged on the fixture's three kinds.
PASSAGE_CASE = "par-reader-marks-the-passage-of-the-selected-result"
PASSAGE_KINDS = ("text", "attachment", "thread")
PASSAGE_ROW = "mac-main.reader.passage"


def passage_queries():
    # round 20 (main111): the last two make a related mail found by its attachment (the warehouse photo of msg-s02: none of its letters, its meaning)
    return ("unit price", "estimator", CONTENT_QUERY, EXP["meaning_query"], "loading dock cartons", "shipping crates")


def go_to(c, i):
    """Down or Up until result i is the selected one; True when it is."""
    rows = c.rows(RES)
    sel = [n for n, r in enumerate(rows) if r.get("selected")]
    if not sel or not 0 <= i < len(rows):
        return False
    for _ in range(abs(i - sel[0])):
        c.key("Down" if i > sel[0] else "Up")
    return selected_id(c) == rows[i]["id"]


def passage_of_kind(c, kind):
    """Select a result whose passage is of `kind` and read the reader's passage row: (marked, the passage is the row's, the source is the row's, the mail is the
    row's hit mail, what the kind needs: the marked text / the file's ring / an earlier mail). None when no query's results hold a result of that kind."""
    for q in passage_queries():
        launch(c, q)
        rows = c.rows(RES)
        i = next((n for n, r in enumerate(rows) if r.get("source") == kind and r.get("passage_line")), None)
        if i is None:
            continue
        if not go_to(c, i):
            return ("not-selected",)
        want = rows[i]
        reader_after_quiet(c, want["id"])
        got = c.rows(PASSAGE_ROW)[0]
        extra = {"text": bool(got.get("pieces")), "attachment": got.get("file") == want.get("source_name") and bool(got.get("ring")),
                 "thread": got.get("earlier_index") is not None and got.get("in_open_mail") is False}[kind]
        return (got.get("marked") is True, got.get("text") == want["passage_line"], got.get("source") == kind, got.get("mail_id") == want.get("hit_id"), extra)
    return None


def reader_marks_passage(c):
    """The three kinds in turn, then the search closed: nothing is marked (the reader shows the focused inbox mail again)."""
    out = [passage_of_kind(c, k) for k in PASSAGE_KINDS]
    c.key("Escape")
    surface = c.uistate().get("surface")
    try:
        after = c.rows(PASSAGE_ROW)[0].get("marked") is False
    except Unreadable:
        after = False
    return (out, surface == "mac-main", after)


PASSAGE_EXPECT = ([(True, True, True, True, True)] * 3, True, True)


class PassageModel:
    """The self-test's stand-in for the search screen and the reader: three results (a text one, an attachment one, one whose passage is in an EARLIER mail of the
    conversation), the reader's `mac-main.reader.passage` row for the selected one. `mutant` names one defect; with none it is the correct behaviour."""
    RESULTS = [{"id": "row-a", "hit_id": "row-a", "source": "text", "source_name": None, "passage_line": "the unit price at 12,000", "pieces": ["unit", "price"]},
               {"id": "row-b", "hit_id": "row-b", "source": "attachment", "source_name": "warehouse-photo.jpg", "passage_line": "pallets of brochures", "pieces": []},
               {"id": "row-c", "hit_id": "msg-early", "source": "thread", "source_name": "Marc Petit", "passage_line": "as agreed we lowered the amount", "pieces": ["as agreed we lowered the amount"]}]

    def __init__(self, mutant=None):
        self.m, self.active, self.launched, self.sel, self.text = mutant, False, False, 0, ""

    def click(self, ax):
        if ax == BAR:
            self.active = True

    def type(self, t):
        self.text += t

    def key(self, k):
        if k == "⌘A":
            self.text = ""
        elif k == "Enter":
            self.launched, self.sel = True, 0
        elif k in ("Down", "Up") and self.launched:
            self.sel = min(max(0, self.sel + (1 if k == "Down" else -1)), len(self.RESULTS) - 1)
        elif k == "Escape":
            self.active, self.launched, self.text = False, False, ""

    def uistate(self):
        return {"surface": "mac-search" if self.active else "mac-main"}

    def measure(self, ax):
        if ax == "ai-setup.mark.meaning" and self.m == "empty-mark-keeps-its-band":
            return {"frame": {"x": 380.0, "y": 760.0, "w": 400.0, "h": 0.0}}
        raise Unreadable("measure %s: not on screen" % ax)

    def rows(self, region):
        if region == BAR:
            return [{"text": self.text, "launched": self.launched, "dropdown": False}]
        if region == RES:
            return [{k: r[k] for k in ("id", "hit_id", "source", "source_name", "passage_line")} | {"selected": n == self.sel} for n, r in enumerate(self.RESULTS)] if self.launched else []
        if region == "mac-main.reader.subject":
            return [{"id": self.RESULTS[self.sel]["id"] if self.launched else "inbox-0"}]
        if region == READER_SCROLL:
            return [{"offset": 84.0 if self.m == "scrolls-to-a-passage-in-view" else 0.0, "max_offset": 400.0, "viewport": 590.0, "content": 990.0}]
        if region == "ai-setup.status":
            return [{"need": "meaning", "usable": True}]
        if region == PASSAGE_ROW:
            none = {"marked": False, "text": "", "source": "", "file": None, "mail_id": None, "in_open_mail": False, "earlier_index": None, "pieces": [], "ring": None}
            if not self.launched and self.m != "mark-stays-after-close":
                return [none]
            r = self.RESULTS[self.sel]
            early = r["source"] == "thread"
            mail = r["id"] if self.m == "hands-the-row-mail" else r["hit_id"]
            file = r["source_name"] if r["source"] == "attachment" else None
            return [{"marked": self.m != "marks-nothing", "text": r["passage_line"], "source": r["source"], "file": file, "mail_id": mail,
                     "in_open_mail": not early, "earlier_index": 0 if early else None, "pieces": r["pieces"],
                     "ring": None if self.m == "no-ring" else file}]
        return []


PASSAGE_MUTANTS = ("marks-nothing", "hands-the-row-mail", "no-ring", "mark-stays-after-close")

INVIEW_CASE = "par-a-passage-already-in-view-does-not-move-the-reader"
READER_SCROLL = "mac-main.reader.scroll"


def passage_in_view_stays(c):
    """The lead's walk on main182 (the word "quote"): the marked sentence stood in the middle of the pane and the reader still scrolled, which cut the summary's
    top. The first result whose passage is in the mail's own text (the first lines of the newest mail: in view at the default window) is selected; the reader
    marks it and stays where it was: offset 0. A passage that is NOT in view is still brought there: the earlier-mail and attachment kinds of the case above."""
    for q in passage_queries():
        launch(c, q)
        rows = c.rows(RES)
        i = next((n for n, r in enumerate(rows) if r.get("source") == "text" and r.get("passage_line")), None)
        if i is None:
            continue
        if not go_to(c, i):
            return ("not-selected",)
        reader_after_quiet(c, rows[i]["id"])
        time.sleep(0.8)                                 # the page marks its passage after its load, then says where the mark stands
        got, sc = c.rows(PASSAGE_ROW)[0], c.rows(READER_SCROLL)[0]
        c.key("Escape")
        return (got.get("marked") is True, round(sc.get("offset", -1)))
    return None


INVIEW_EXPECT = (True, 0)

BAND_CASE = "par-the-results-keep-no-blank-band-at-their-foot"
EVERY_CASE = "par-a-match-holds-every-typed-word"
# fixtures/tests-j2.json: the conversation of msg-01 holds "quote" and "signed"; msg-s05 (another conversation) holds one of the two
EVERY_QUERY, EVERY_BOTH, EVERY_ONE = "quote signed", {"msg-01", "msg-01a", "msg-01b"}, "msg-s05"


def every_word_match(c):
    """Several typed words: a match holds every one, as the real engine's (search-text query.rs). The test engine listed a mail with one of the words among the
    matches (the lead's walk on main183, "print run": three matches, two of them with one word). Answers (the matches are mails of the conversation that holds
    both words, the mail with one word is not among the matches)."""
    launch(c, EVERY_QUERY)
    rows = [r for r in c.rows(RES) if r.get("part") == "match"]
    ids = {r.get("hit_id") for r in rows} | {r.get("id") for r in rows}
    ids.discard(None)
    return (bool(rows) and ids <= EVERY_BOTH, EVERY_ONE not in ids)


class EveryWordModel(PassageModel):
    """The stand-in for the case above: the conversation with both words is the match; the mail with one word is related (a match in the broken copy)."""
    def rows(self, region):
        if region == RES and self.launched:
            return [{"id": "msg-01b", "hit_id": "msg-01", "part": "match"},
                    {"id": EVERY_ONE, "hit_id": EVERY_ONE, "part": "match" if self.m == "one-word-is-a-match" else "related"}]
        return super().rows(region)


def results_foot_band(c):
    """With the meaning model set up, the search list's missing-model line says nothing and nothing of it is mounted. Its wrapper used to stay: 10 pt of padding
    on the pane's ground, a blank band over the foot of the results (walk-main183/08: the results ended 18.5 pt above the foot strip, the inbox list 3.5).
    True = the line is mounted while it says nothing."""
    launch(c, passage_queries()[0])
    try:
        usable = next((r.get("usable") for r in c.rows("ai-setup.status") if r.get("need") == "meaning"), None)
        if usable is not True:
            raise Unreadable("the meaning model is not set up in this copy (usable %r): the line is right to show" % usable)
        try:
            frame = c.measure("ai-setup.mark.meaning").get("frame")
        except Unreadable:
            frame = None                                # not on screen: the socket refuses the measure
        return bool(frame)
    finally:
        c.key("Escape")


def passage_run(mutant):
    try:
        return reader_marks_passage(PassageModel(mutant)) == PASSAGE_EXPECT
    except (Unreadable, KeyError, IndexError, TypeError):
        return False


def passage_selftest():
    bad = []
    base = passage_run(None)
    print("model, the reader marks the selected result's passage, correct behaviour: green=%s" % base)
    if not base:
        bad.append("%s not green on the correct model" % PASSAGE_CASE)
    for mutant in PASSAGE_MUTANTS:
        red = not passage_run(mutant)
        print("mutant %-22s target %-52s red=%s" % (mutant, PASSAGE_CASE, red))
        if not red:
            bad.append("mutant %s stays green on %s" % (mutant, PASSAGE_CASE))
    if not re.search(r"^# J2\.4/%s \|" % PASSAGE_CASE, MUTANTS, re.M):
        bad.append("no mutant line declared for %s" % PASSAGE_CASE)
    stays = passage_in_view_stays(PassageModel(None)) == INVIEW_EXPECT
    moved = passage_in_view_stays(PassageModel("scrolls-to-a-passage-in-view")) != INVIEW_EXPECT
    print("model, a passage already in view does not move the reader: green=%s; mutant scrolls-to-a-passage-in-view red=%s" % (stays, moved))
    if not stays:
        bad.append("%s not green on the correct model" % INVIEW_CASE)
    if not moved:
        bad.append("mutant scrolls-to-a-passage-in-view stays green on %s" % INVIEW_CASE)
    if not re.search(r"^# J2\.4/%s \|" % INVIEW_CASE, MUTANTS, re.M):
        bad.append("no mutant line declared for %s" % INVIEW_CASE)
    clear = results_foot_band(PassageModel(None)) is False
    kept = results_foot_band(PassageModel("empty-mark-keeps-its-band")) is True
    print("model, the results keep no blank band at their foot: green=%s; mutant empty-mark-keeps-its-band red=%s" % (clear, kept))
    if not clear:
        bad.append("%s not green on the correct model" % BAND_CASE)
    if not kept:
        bad.append("mutant empty-mark-keeps-its-band stays green on %s" % BAND_CASE)
    if not re.search(r"^# J2\.4/%s \|" % BAND_CASE, MUTANTS, re.M):
        bad.append("no mutant line declared for %s" % BAND_CASE)
    whole = every_word_match(EveryWordModel(None)) == (True, True)
    part = every_word_match(EveryWordModel("one-word-is-a-match")) != (True, True)
    print("model, a match holds every typed word: green=%s; mutant one-word-is-a-match red=%s" % (whole, part))
    if not whole:
        bad.append("%s not green on the correct model" % EVERY_CASE)
    if not part:
        bad.append("mutant one-word-is-a-match stays green on %s" % EVERY_CASE)
    if not re.search(r"^# J2\.9/%s \|" % EVERY_CASE, MUTANTS, re.M):
        bad.append("no mutant line declared for %s" % EVERY_CASE)
    if not re.search(r"^    s\.expect\(EVERY_CASE, lambda: every_word_match\(s\.ctl\), \(True, True\)\)", inspect.getsource(drive), re.M):
        bad.append("%s is not driven in the journey's J2.9 step" % EVERY_CASE)
    print("4 reader-passage checks, %d mutants, %d problems" % (len(PASSAGE_MUTANTS) + 3, len(bad)))
    for b in bad:
        sys.stderr.write("SELFTEST FAIL %s\n" % b)
    return bad


sys.path.insert(0, os.path.join(ROOT, "checks"))
from checklib import related_file as RELATED_FILE                   # noqa: E402  round 23: the corpus where a related mail found by its attachment is certain
RELATED_FILE_DIR = os.path.join(ROOT, "_captures", ".tmp", "searchmac")   # git ignores _captures/.tmp


# ---- hid-search round 18 (main107, item d1ffe36c): selecting a related mail found by its attachment ended the test copy (AppKit raised in a layout pass:
# `+[NSApplication _crashOnException:]` under `_NSViewLayout`, five crash reports). The case selects such a result, lets the reader settle, and asks the copy
# twice for its state: a copy that died or stopped answering fails the case by name, never a pipe error in the next case.
RELATED_FILE_CASE = "par-a-related-mail-from-an-attachment-opens-and-the-app-keeps-answering"


def related_file_stays(c):
    """(the result was selected, the reader shows it, the first uistate answered, the second answered); a dead copy answers ('the copy stopped answering', ...).
    Round 23 (item bad881fa): on the corpus where a related mail found by its attachment is certain (checks/checklib/related_file.py: one mail that says "See
    attached." and one file on bee keeping, queried in words in none of the corpus), the J2 corpus loaded again after; a miss names each query's closeness of
    that mail and the related cut-off (the test engine's `search/last/closeness/<id>`, `search/last/related_cut`)."""
    c.send("fixture", "corpus", RELATED_FILE.write(RELATED_FILE_DIR))
    try:
        return related_file_on(c)
    finally:
        c.send("fixture", "corpus", os.path.join(ROOT, "fixtures", "tests-j2.json"))


def related_file_on(c):
    missed = []
    for q in RELATED_FILE.QUERIES:
        launch(c, q)
        rows = c.rows(RES)
        i = next((n for n, r in enumerate(rows) if r.get("part") == "related" and r.get("source") == "attachment"), None)
        if i is None:
            try:
                missed.append("%r: closeness of %s %s, cut-off %s" % (q, RELATED_FILE.MAIL_ID, c.core("search/last/closeness/%s" % RELATED_FILE.MAIL_ID), c.core("search/last/related_cut")))
            except Unreadable as e:
                missed.append("%r: closeness unread (%s)" % (q, str(e)[:80]))
            continue
        if not go_to(c, i):
            return ("not-selected", q, i)
        want = rows[i]["id"]
        try:
            shown = reader_after_quiet(c, want) == want
            first = c.uistate().get("surface") is not None
            time.sleep(1.0)
            second = c.uistate().get("surface") is not None
        except Unreadable as e:
            return ("the copy stopped answering after a related mail from an attachment was selected", q, str(e)[:160])
        c.key("Escape")                                  # the search closes, as the passage case before it leaves the screen for the next step
        return (True, shown, first, second)
    raise Unreadable("no related mail found by its attachment in the results of %s: the case judges nothing" % "; ".join(missed))


RELATED_FILE_EXPECT = (True, True, True, True)


class RelatedFileModel(PassageModel):
    """PassageModel with one more result: a related mail found by its attachment (the fourth row). Mutant `dies-on-related-file`: the copy stops answering once it
    is selected (main107)."""
    RESULTS = PassageModel.RESULTS + [{"id": "row-d", "hit_id": "row-d", "source": "attachment", "source_name": "offer-v3.pdf", "passage_line": "we lowered the amount", "pieces": []}]

    def rows(self, region):
        out = super().rows(region)
        if region == RES:
            out = [dict(r, part="related" if r["id"] == "row-d" else "match") for r in out]
        return out

    def send(self, verb, *a):
        return {"ok": True}

    def core(self, path):
        return None

    def uistate(self):
        if self.m == "dies-on-related-file" and self.launched and self.RESULTS[self.sel]["id"] == "row-d":
            raise Unreadable("uistate (): [Errno 32] Broken pipe")
        return super().uistate()


def related_file_selftest():
    bad = []
    try:
        base = related_file_stays(RelatedFileModel()) == RELATED_FILE_EXPECT
    except Unreadable:
        base = False
    dead = related_file_stays(RelatedFileModel("dies-on-related-file"))
    red = dead != RELATED_FILE_EXPECT and "stopped answering" in str(dead[0])
    print("model, a related mail from an attachment, correct behaviour: green=%s; mutant dies-on-related-file red=%s (%s)" % (base, red, dead[0]))
    try:
        related_file_on(type('NoCore', (PassageModel,), {'core': lambda self, path: None})())
        bad.append("%s judged on results with no related mail from an attachment" % RELATED_FILE_CASE)
    except Unreadable:
        pass
    if not base:
        bad.append("%s not green on the correct model" % RELATED_FILE_CASE)
    if not red:
        bad.append("mutant dies-on-related-file stays green on %s" % RELATED_FILE_CASE)
    if not re.search(r"^# J2\.4/%s \|" % RELATED_FILE_CASE, MUTANTS, re.M):
        bad.append("no mutant line declared for %s" % RELATED_FILE_CASE)
    print("1 related-file check, 1 mutant, %d problems" % len(bad))
    for b in bad:
        sys.stderr.write("SELFTEST FAIL %s\n" % b)
    return bad


# ---- round 3 (lane list-p1, SPEC A150, A152, A153): the list in three parts, the closed line, the passage on the row, the sender-domain filter ----
PARTS_CASES = ("par-list-in-three-parts-in-order", "par-closed-line-opens-and-closes-by-enter", "par-closed-line-click-and-new-search-closes-it",
               "par-count-line-says-matches-and-related", "par-attachment-result-shows-its-passage-line", "par-domain-suggestion-tab-pill-and-engine-query",
               "par-no-match-state-keeps-the-closed-line")
PART_ORDER = {"match": 0, "related": 1, "rest": 2}
MORE = "mac-search.more"
FOOT = "mac-main.foot.hints"


def count_expect(c, m, r):
    """The count line the header must serve for m matches and r related (SPEC A150): both, matches alone, 'No match · M related', or nothing at all."""
    if m == 0 and r == 0:
        return ""
    if r == 0:
        return matched_words(c, m)
    if m == 0:
        return app_words(c, "No match · 1 related") if r == 1 else app_words(c, "No match · %lld related", r)
    key = ("1 match" if m == 1 else "%lld matches") + " · " + ("1 related" if r == 1 else "%lld related")
    return app_words(c, key, *([m] if m != 1 else []) + ([r] if r != 1 else []))


def parts_in_order(c):
    """After a search whose answer has related mail: the rows come match, related, in that order, none of the rest while the line is closed, and the closed
    line is there with its count (the rest of the answer), closed."""
    rows, more = c.rows(RES), c.rows(MORE)
    parts = [r.get("part") for r in rows]
    ranks = [PART_ORDER.get(p, 9) for p in parts]
    return (ranks == sorted(ranks), "rest" not in parts, "match" in parts and "related" in parts, len(more) == 1 and more[0]["open"] is False and more[0]["count"] > 0)


PARTS_ORDER_EXPECT = (True, True, True, True)


def down_to_line(c):
    for _ in range(60):
        m = c.rows(MORE)
        if m and m[0]["selected"]:
            return True
        c.key("Down")
    return False


def foot_enter_words(c):
    """The words of the bottom bar's Enter hint while the closed line is selected (FOOT_MORE of the drawing)."""
    return next((h["words"] for h in c.rows(FOOT) if h.get("command") == "enter"), None)


def line_by_enter(c):
    """Down reaches the closed line (the bar says Show all other mail); Enter opens it (the rest is listed under it, the bar says Hide); Enter closes it."""
    reached = down_to_line(c)
    show = foot_enter_words(c)
    n = c.rows(MORE)[0]["count"]
    c.key("Enter")
    after = c.rows(RES)
    opened = (c.rows(MORE)[0]["open"], sum(1 for r in after if r.get("part") == "rest") == n, foot_enter_words(c))
    c.key("Enter")
    closed = (c.rows(MORE)[0]["open"], all(r.get("part") != "rest" for r in c.rows(RES)))
    return (reached, show, opened, closed)


def line_by_enter_expect(c):
    return (True, app_words(c, "Show all other mail"), (True, True, app_words(c, "Hide all other mail")), (False, True))


def line_by_click(c, again):
    """A click opens the line and a click closes it; a new search closes it again."""
    c.click(MORE)
    first = c.rows(MORE)[0]["open"]
    c.click(MORE)
    second = c.rows(MORE)[0]["open"]
    c.click(MORE)
    again()                                              # a new search with the line open
    end = time.time() + 4.0                              # its answer lands after Enter; a search with no other mail has no closed line at all (closed)
    while time.time() < end and c.rows(BAR)[0].get("loading") is True:
        time.sleep(0.1)
    m = c.rows(MORE)
    return (first, second, bool(m) and m[0]["open"])


def count_line_parts(c):
    rows = c.rows(RES)
    m = sum(1 for r in rows if r.get("part") == "match")
    r = sum(1 for x in rows if x.get("part") == "related")
    h = c.rows("mac-search.header")[0]
    want = count_expect(c, m, r)
    return (True if h["count"] == want else "served %r, expected %r" % (h["count"], want), h.get("matches") == m, h.get("related") == r)


def attachment_passage(c, file_name, word):
    """The result found in an attachment's text: its source is the attachment with the file's name, and the passage line holds the typed word, in bold."""
    r = next((x for x in c.rows(RES) if x.get("source") == "attachment"), None)
    if r is None:
        return None
    return (r["source_name"] == file_name, word in (r["passage_line"] or "").lower(), word in [b.lower() for b in (r.get("passage_bold") or [])], r["part"])


def domain_flow(c, domain):
    """The first letters of the domain offer 'From domain <domain>' right under the Search row; Tab makes the pill; Enter asks the engine for it."""
    query(c, domain[:4])
    dd = c.rows(DD)
    kinds = [r["kind"] for r in dd[:2]]
    offered = dd[1]["title"] == domain and bool(dd[1]["detail"]) if len(dd) > 1 else False
    c.key("Down")
    c.key("Tab")
    chips = c.rows(CHIPS)
    pill = ([k["kind"] for k in chips], [k["value"] for k in chips], c.rows(BAR)[0]["text"])
    c.key("Enter")
    q = jval(c.core("search/last/query")) or ""
    return (kinds, offered, pill, q.startswith("from:@" + domain))


def no_match_state(c, domain):
    """A domain pill and words nothing holds or is close to: no row, no count line, the closed line selected and counting every other mail of the pill."""
    launch(c, "from:@%s zzqxvk" % domain)
    end = time.time() + 4.0                              # the answer lands after Enter: read once it is in (main92 read the header while it was loading)
    while time.time() < end and c.rows(BAR)[0].get("loading") is True:
        time.sleep(0.1)
    h = c.rows("mac-search.header")
    m = c.rows(MORE)
    return (c.rows(RES) == [], h[0]["count"] == "" if h else None, len(m) == 1 and m[0]["selected"] is True and m[0]["count"] > 0)


class PartsModel:
    """The self-test's stand-in for the three-part list: the socket's calls, answering what the built screen must answer. `mutant` names one defect."""
    def __init__(self, mutant=None):
        self.m, self.text, self.launched, self.open, self.sel, self.on_more = mutant, "", False, False, 0, False
        self.matches, self.related, self.rest = ["m1", "m2"], ["r1", "r2"], ["x1", "x2", "x3"]
        self.domain, self.dd_open, self.chips = "lorvan-hotels.example", False, []
        self.core_q = None
        self.launched_text = ""

    def click(self, ax):
        if ax == MORE and self.m != "no-click-door":
            self.open = not self.open
            self.on_more = True

    def type(self, t):
        self.text += t
        self.dd_open = True

    def uses_domain(self):
        return "from:@" in self.text

    def key(self, k):
        if k == "⌘A":
            self.text, self.chips = "", []
        elif k == "Enter":
            if self.launched and self.on_more and self.text == self.launched_text and self.m != "enter-ignored-on-line":   # changed words launch, as the field does
                self.open = not self.open
            else:
                self.launch()
        elif k == "Down":
            if self.dd_open:
                self.hl = getattr(self, "hl", 0) + 1
            elif self.launched:
                n = len(self.matches) + len(self.related)
                if self.on_more:
                    self.on_more = False if self.open else True
                    self.sel = n if self.open else self.sel
                elif self.sel == n - 1:
                    self.on_more = True
                else:
                    self.sel += 1
        elif k == "Tab" and self.dd_open and getattr(self, "hl", 0) == 1:
            self.chips = [("from" if self.m == "domain-is-a-person" else "domain", self.domain)]
            self.text = "from:@" + self.domain
            self.dd_open = False

    def launch(self):
        self.launched, self.dd_open, self.sel, self.hl = True, False, 0, 0
        if self.m != "open-survives-new-search":
            self.open = False
        self.on_more = False
        nothing = "zzqxvk" in self.text
        self.cur = ([], [], self.rest + ["x4"]) if nothing else (self.matches, self.related, self.rest)
        if nothing and self.m != "nothing-no-line":
            self.on_more = True
        self.core_q = self.launched_text = self.text

    def core(self, path):
        return json.dumps(self.core_q) if path == "search/last/query" else None

    def rows(self, region):
        if region == BAR:
            return [{"text": self.text, "draft": "", "launched": self.launched}]
        if region == CHIPS:
            return [{"kind": k, "value": v} for k, v in self.chips]
        if region == DD:
            if not self.dd_open:
                return []
            return [{"kind": "search", "title": self.text, "detail": ""},
                    {"kind": "domain" if self.m != "domain-after-people" else "from", "title": self.domain, "detail": "3 people"}]
        if region == "mac-settings-general.settings":
            return [{"language": "en"}]
        if not self.launched:
            return []
        ms, rl, rs = self.cur
        att = "pallets" in self.text
        if region == RES:
            out = [{"part": "match", "source": "attachment" if att and i == 0 and self.m != "no-passage" else None, "source_name": "warehouse-photo.jpg",
                    "passage_line": "pallets in the warehouse", "passage_bold": ["pallets"], "selected": False} for i, _ in enumerate(ms)]
            out += [{"part": "related", "source": "text", "source_name": None, "passage_line": "x", "passage_bold": [], "selected": False} for _ in rl]
            if self.open or self.m == "rest-listed-closed":
                out += [{"part": "rest", "source": None, "source_name": None, "passage_line": None, "passage_bold": None, "selected": False} for _ in rs]
            return out
        if region == MORE:
            if not rs or (not ms and not rl and self.m == "nothing-no-line"):
                return []
            return [{"count": len(rs), "open": self.open, "selected": self.on_more}]
        if region == "mac-search.header":
            r = len(rl)
            count = count_expect(self, len(ms), r) if self.m != "count-counts-only-matches" else matched_words(self, len(ms))
            return [{"count": count, "matches": len(ms), "related": r}]
        if region == FOOT:
            if not self.on_more:
                return []
            words = app_words(self, "Hide all other mail" if self.open and self.m != "foot-stale" else "Show all other mail")
            return [{"command": "enter", "words": words}]
        return []


PARTS_MUTANTS = {"rest-listed-closed": PARTS_CASES[0], "enter-ignored-on-line": PARTS_CASES[1], "foot-stale": PARTS_CASES[1], "no-click-door": PARTS_CASES[2],
                 "open-survives-new-search": PARTS_CASES[2], "count-counts-only-matches": PARTS_CASES[3], "no-passage": PARTS_CASES[4],
                 "domain-is-a-person": PARTS_CASES[5], "domain-after-people": PARTS_CASES[5], "nothing-no-line": PARTS_CASES[6]}


def parts_run(mutant):
    """The seven cases in the journey's order on one model; {case: green?}."""
    m, out = PartsModel(mutant), {}
    def judge(case, fn, want):
        try:
            out[case] = fn() == (want() if callable(want) else want)
        except (Unreadable, KeyError, IndexError, TypeError, StopIteration):
            out[case] = False
    launch(m, "revised price")
    judge(PARTS_CASES[0], lambda: parts_in_order(m), PARTS_ORDER_EXPECT)
    judge(PARTS_CASES[3], lambda: count_line_parts(m), (True, True, True))
    judge(PARTS_CASES[1], lambda: line_by_enter(m), lambda: line_by_enter_expect(m))
    judge(PARTS_CASES[2], lambda: line_by_click(m, lambda: launch(m, "revised price again")), (True, False, False))
    launch(m, "pallets")
    judge(PARTS_CASES[4], lambda: attachment_passage(m, "warehouse-photo.jpg", "pallets"), ("warehouse-photo.jpg" == "warehouse-photo.jpg", True, True, "match"))
    judge(PARTS_CASES[5], lambda: domain_flow(m, m.domain), lambda: (["search", "domain"], True, (["domain"], [m.domain], "from:@" + m.domain), True))
    judge(PARTS_CASES[6], lambda: no_match_state(m, m.domain), (True, True, True))
    return out


def parts_selftest():
    bad = []
    base = parts_run(None)
    print("model, the list in three parts, correct behaviour: %s" % base)
    bad += ["%s not green on the correct model" % k for k, v in base.items() if not v]
    for mutant, target in PARTS_MUTANTS.items():
        got = parts_run(mutant)
        print("mutant %-26s target %-52s red=%s" % (mutant, target, not got[target]))
        if got[target]:
            bad.append("mutant %s stays green on %s" % (mutant, target))
    declared = set(re.findall(r"^# J2\.\d/(par-[\w-]+) \|", MUTANTS, re.M))
    bad += ["no mutant line declared for %s" % k for k in PARTS_CASES if k not in declared]
    print("%d three-part checks, %d mutants, %d problems" % (len(PARTS_CASES), len(PARTS_MUTANTS), len(bad)))
    for b in bad:
        sys.stderr.write("SELFTEST FAIL %s\n" % b)
    return bad


# ---- lane gap-search-cases: a REAL Return in the field, the index line, a shorter word finding its longer form, the reader never off the results ----
REAL_CASES = ("par-real-return-in-the-field-launches-the-typed-words", "par-real-return-from-the-list-opens-the-reply")
REAL_WORDS = "hello"
REAL_WAIT_S = 2.0          # a check bound, not a product number: the field takes the real letters, then the real Return
QUICK = "mac-main.quickreply.field"
INDEX_CASE = "par-index-line-shows-while-the-meaning-index-builds"
INDEX_NOTE = "mac-search.index-note"
INDEX_FIGURES = (40, 60)   # indexed, queued: under 1,000 so no separator depends on the language
LONGER_CASE = "par-a-shorter-word-finds-the-mail-that-holds-its-longer-form"
LONGER_WORD = "print"
READER_IN_CASE = "par-reader-never-shows-a-mail-outside-the-results"


def hay(m):
    """Everything the typed letters may be found in, lower case (A149: the subject, the sender's name and address, the text, the attachments' names)."""
    return " ".join([m["subject"], m["from_name"], m["from"], m["body"], " ".join(m["attachments"])]).lower()


# the mails that hold LONGER_WORD only inside a longer word (Printwerk), read from the fixture file: the corpus must hold one, else the case judges nothing
LONGER_THREADS = sorted({m["thread"] for m in MSGS if LONGER_WORD in hay(m) and not re.search(r"\b%s\b" % LONGER_WORD, hay(m))})


def poll(done, wait=REAL_WAIT_S):
    """True once `done()` holds, polled for `wait` seconds (a key event lands on the next turn of the app's run loop)."""
    end = time.time() + wait
    while True:
        if done():
            return True
        if time.time() > end:
            return False
        time.sleep(0.05)


def real(c, text):
    """One REAL key press (`keyevent`, KeyEventVerb.swift): an NSEvent through the shell's monitor and then the window that holds the cursor, as the keyboard sends it.
    The socket's `key` verb calls AppModel.press and never meets the field's own handler."""
    c.send("keyevent", text)


def to_list_row(c):
    """The search closed, the list's last row holding the keys."""
    leave_search(c)
    c.click("mac-main.list.row.%d" % (len(c.rows(MAIN_LIST)) - 1))


def real_return_in_field(c):
    """The owner's A113 ("when pressing cmd+k enter is to validate the search") with REAL key events: the list's last row has the keys, ⌘K opens the search, the letters of
    REAL_WORDS and Return are real key events typed into the field. The field's own handler (SearchField.onSubmit / onKeyPress(.return)) runs: the bar says the words
    were launched, and no quick reply opened under the list. NOT JUDGED when the field has no cursor or the real letters did not land (the Return would prove nothing)."""
    to_list_row(c)
    c.key("⌘K")
    if not poll(lambda: c.rows(BAR)[0].get("cursor") is True):
        raise Unreadable("the field has no cursor after the search key (bar %s): a real key would not reach it" % search_snapshot(c))
    real(c, REAL_WORDS)
    if not poll(lambda: c.rows(BAR)[0].get("text") == REAL_WORDS):
        raise Unreadable("the real letters did not reach the field (bar text %r, wanted %r)" % (c.rows(BAR)[0].get("text"), REAL_WORDS))
    real(c, "return")
    poll(lambda: c.rows(BAR)[0].get("launched") is True or len(c.rows(QUICK)) > 0)
    b = c.rows(BAR)[0]
    return (c.uistate().get("surface"), b.get("launched"), b.get("text"), len(c.rows(QUICK)))


REAL_EXPECT = ("mac-search", True, REAL_WORDS, 0)


def real_return_from_list(c):
    """The positive control of the case above: the SAME real Return with the LIST holding the keys (the search closed) does open the quick reply (Commands.enter: in the
    plain list Enter does what R does), so the absence of a reply in the case above means the field took the key. Closes what it opened."""
    to_list_row(c)
    real(c, "return")
    poll(lambda: len(c.rows(QUICK)) > 0)
    b = c.rows(BAR)[0]
    got = (len(c.rows(QUICK)), b.get("launched"), b.get("active"))
    for _ in range(3):                                   # the reply, the reader, then the list: back to the inbox
        if not c.rows(QUICK) and c.uistate().get("surface") == "mac-main":
            break
        c.key("Escape")
    return got


REAL_CONTROL_EXPECT = (1, False, False)
REAL_JUDGE = {REAL_CASES[0]: (real_return_in_field, REAL_EXPECT), REAL_CASES[1]: (real_return_from_list, REAL_CONTROL_EXPECT)}


def index_number(c, n):
    return "{:,}".format(n) if (c.rows("mac-settings-general.settings") or [{}])[0].get("language", "en") in ("en", "system") else str(n)


def index_line(c):
    """While the meaning index is being built the results carry ONE line above the rows, with the figures of the builder (`mac-search.index-note`); when the build is done
    the line is gone. The builder's state is set by `fixture index-progress <indexed> <queued>` (a build that does not serve the verb: NOT JUDGED naming it)."""
    done, queued = INDEX_FIGURES
    c.send("fixture", "index-progress", str(done), str(queued))
    try:
        launch(c, fresh_words(2))
        want = app_words(c, "The meaning index is still being built: %@ of %@ mails.", index_number(c, done), index_number(c, done + queued))
        shown = poll(lambda: [r.get("text") for r in c.rows(INDEX_NOTE)] == [want])
        said = [r.get("text") for r in c.rows(INDEX_NOTE)]
    finally:
        c.send("fixture", "index-progress", str(done + queued), "0")
    gone = poll(lambda: c.rows(INDEX_NOTE) == [])
    return (True if shown and said == [want] else "said %r, expected %r" % (said, [want]), gone)


INDEX_EXPECT = (True, True)


def shorter_word_finds_longer_form(c):
    """A word typed in its shorter form finds the mail that holds it only inside a longer word (A149: the letters inside words: `print` in the attachment name
    printwerk-offer.pdf of msg-s05): that mail's thread is in the match group, and its row says where the letters are. The mails come from the fixture file (LONGER_THREADS), never from the screen."""
    if not LONGER_THREADS:
        raise Unreadable("the fixture corpus holds no mail with %r only inside a longer word: the case judges nothing" % LONGER_WORD)
    launch(c, LONGER_WORD)
    rows = c.rows(RES)
    match = [r for r in rows if r.get("part") == "match"]
    missing = sorted(set(LONGER_THREADS) - {r["thread"] for r in match})
    # the mail's row says where the letters are (here the attachment's name: no letter to bold in a name the row's own attachment line shows)
    says = all(r.get("evidence_in") not in (None, "none") for r in match if r["thread"] in LONGER_THREADS)
    return (missing, says and len(match) > 0)


LONGER_EXPECT = ([], True)


def reader_in_results(c):
    """The reader never shows a mail that is not among the listed results: after each of Down, Down, Up its mail id is one of the ids the list shows."""
    launch(c, EXP["meaning_query"])
    out = []
    for k in ("Down", "Down", "Up"):
        c.key(k)
        ids = {r["id"] for r in c.rows(RES)}
        poll(lambda: [r["id"] for r in c.rows("mac-main.reader.subject")] != [] and {r["id"] for r in c.rows("mac-main.reader.subject")} <= ids, READER_WAIT_S)
        shown = {r["id"] for r in c.rows("mac-main.reader.subject")}
        out.append(len(shown) == 1 and shown <= ids)
    return out


class GapModel:
    """The self-test's stand-in for the four cases above: the socket's calls (rows, click, key, type, send, uistate, core). `mutant` names one defect."""
    IDS = ["m-1", "m-2", "m-3"]

    def __init__(self, mutant=None):
        self.m, self.surface, self.active, self.cursor, self.text, self.launched, self.reply = mutant, "mac-main", False, False, "", False, False
        self.progress, self.sel, self.shown = None, 0, "m-1"

    def send(self, verb, *args):
        if verb == "keyevent":
            key = args[-1]
            if key == "return":
                if self.cursor and self.active:
                    if self.m == "return-ignored-in-field":
                        return
                    if self.m == "return-reaches-the-list":
                        self.reply = True
                    else:
                        self.launched = True
                elif self.m != "list-return-dead":
                    self.reply = True
            elif self.cursor:
                self.text += key
            return
        if verb == "fixture" and args[0] == "index-progress":
            if self.m == "verb-not-served":
                raise Unreadable("fixture index-progress: unknown verb")
            self.progress = (int(args[1]), int(args[2]))
            return
        raise Unreadable("no such verb %s" % verb)

    def key(self, k):
        if k == "⌘K":
            self.active, self.surface, self.cursor = True, "mac-search", True
        elif k == "⌘A":
            self.text = ""
        elif k == "Enter":
            if self.active:
                self.launched, self.sel, self.shown = True, 0, self.IDS[0]
        elif k == "Escape":
            if self.reply:
                self.reply = False
            else:
                self.active, self.surface, self.cursor, self.launched, self.text = False, "mac-main", False, False, ""
        elif k in ("Down", "Up") and self.launched:
            self.sel = min(max(0, self.sel + (1 if k == "Down" else -1)), len(self.IDS) - 1)
            self.shown = "outside" if self.m == "reader-off-the-results" else self.IDS[self.sel]

    def click(self, ax):
        if ax.startswith("mac-main.list.row."):
            self.surface, self.cursor = "mac-main", False
        elif ax == BAR:
            self.active, self.surface, self.cursor = True, "mac-search", True

    def type(self, t):
        self.text += t

    def uistate(self):
        return {"surface": self.surface}

    def rows(self, region):
        if region == BAR:
            return [{"text": self.text, "launched": self.launched, "active": self.active, "cursor": self.cursor, "dropdown": False, "focused": self.cursor, "owner": "search"}]
        if region == MAIN_LIST:
            return [{"id": "row-%d" % i} for i in range(6)]
        if region == QUICK:
            return [{}] if self.reply else []
        if region == "mac-settings-general.settings":
            return [{"language": "en"}]
        if region == INDEX_NOTE:
            if self.progress and self.progress[1] > 0 and self.launched and self.m != "index-line-never":
                return [{"text": app_words(self, "The meaning index is still being built: %@ of %@ mails.", index_number(self, self.progress[0]), index_number(self, sum(self.progress)))}]
            if self.m == "index-line-always" and self.launched:
                return [{"text": "x"}]
            return []
        if region == RES:
            if not self.launched:
                return []
            if self.text == LONGER_WORD:
                return [{"id": "p-1", "thread": t, "part": "match", "evidence_in": "none" if self.m == "longer-form-says-nothing" else "attachment", "evidence_bold": []} for t in (LONGER_THREADS if self.m != "longer-form-missed" else [])] + \
                       [{"id": "p-0", "thread": "other", "part": "match", "evidence_in": "subject", "evidence_bold": [LONGER_WORD]}]
            return [{"id": i, "thread": i, "part": "match", "selected": n == self.sel} for n, i in enumerate(self.IDS)]
        if region == "mac-main.reader.subject":
            return [{"id": self.shown}]
        return []


def gap_run(mutant):
    """The four cases in the journey's order on one model: {case: green?}."""
    m, out = GapModel(mutant), {}
    def judge(case, fn, want):
        try:
            out[case] = fn() == want
        except (Unreadable, KeyError, IndexError, TypeError):
            out[case] = False
    for case in REAL_CASES:
        judge(case, lambda case=case: REAL_JUDGE[case][0](m), REAL_JUDGE[case][1])
    judge(INDEX_CASE, lambda: index_line(m), INDEX_EXPECT)
    judge(LONGER_CASE, lambda: shorter_word_finds_longer_form(m), LONGER_EXPECT)
    judge(READER_IN_CASE, lambda: reader_in_results(m), [True, True, True])
    return out


GAP_MUTANTS = {"return-ignored-in-field": REAL_CASES[0], "return-reaches-the-list": REAL_CASES[0], "list-return-dead": REAL_CASES[1],
               "verb-not-served": INDEX_CASE, "index-line-never": INDEX_CASE, "index-line-always": INDEX_CASE, "longer-form-missed": LONGER_CASE, "longer-form-says-nothing": LONGER_CASE,
               "reader-off-the-results": READER_IN_CASE}


def gap_selftest():
    bad = []
    base = gap_run(None)
    print("model, real Return, index line, longer form, reader in results, correct behaviour: %s" % base)
    bad += ["%s not green on the correct model" % k for k, v in base.items() if not v]
    for mutant, target in GAP_MUTANTS.items():
        got = gap_run(mutant)
        print("mutant %-26s target %-58s red=%s" % (mutant, target, not got[target]))
        if got[target]:
            bad.append("mutant %s stays green on %s" % (mutant, target))
    declared = set(re.findall(r"^# J2\.\d/(par-[\w-]+) \|", MUTANTS, re.M))
    bad += ["no mutant line declared for %s" % k for k in base if k not in declared]
    if not LONGER_THREADS:
        bad.append("the fixture corpus holds no mail with %r only inside a longer word" % LONGER_WORD)
    print("%d gap checks, %d mutants, %d problems" % (len(base), len(GAP_MUTANTS), len(bad)))
    for b in bad:
        sys.stderr.write("SELFTEST FAIL %s\n" % b)
    return bad


def drive(j):
    s = j.step("J2.1", "Type, suggestions, Tab, Enter")
    s.raw("fixture", "corpus", os.path.join(ROOT, "fixtures", "tests-j2.json"))
    wins = []
    s._act(lambda: wins.append(len(s.ctl.winframes())))
    s.arrive(("key", "/"), "mac-search")
    s.expect("par-no-new-window", lambda: (len(s.ctl.winframes()), len(s.ctl.winframes()) == (wins[0] if wins else -1)), (wins[0] if wins else None, True))
    rows = lambda: s.ctl.rows(RES)
    dd = lambda: s.ctl.rows(DD)
    bar = lambda: s.ctl.rows(BAR)[0]
    s.expect("par-field-focused", lambda: s.ctl.rows(BAR)[0]["focused"], True)   # ⌘K and / put the cursor in the field
    query(s, "Linea")
    s.expect("par-dropdown-people", lambda: (dd()[0]["kind"], sorted(r["detail"] for r in dd() if r["kind"] == "from")), ("search", LINEA_ADDR))
    s.expect("par-dropdown-only-suggestions", lambda: sorted({r["kind"] for r in dd()} - ALLOWED), [])
    s.expect("par-dropdown-subject-and-attachment-rows", lambda: ([r["kind"] for r in dd()].count("subject"), [r["kind"] for r in dd()].count("attachment"),
                                                                [r["title"] for r in dd() if r["kind"] == "attachment"], any(r["title"].lower().find("linea") >= 0 for r in dd() if r["kind"] == "subject")),
             (1, 1, ["Linea"], True))
    s.expect(ORDER_CASE, lambda: suggestions_in_drawn_order(s.ctl), ORDER_EXPECT)          # retypes "Linea": the field is as it was for the next lines
    picked = []
    s.key("Down")
    s.expect("par-highlight-moves", lambda: [r["highlighted"] for r in dd()].index(True), 1)
    def to_person():   # down from row 1 to the first person row (SPEC A153: "From domain" stands right under the Search row when the letters start a domain)
        for _ in range(first_of(s.ctl, "from") - 1):
            s.ctl.key("Down")
        picked.append(dd()[first_of(s.ctl, "from")]["detail"])
    s._act(to_person)
    s.key("Tab")
    s.expect("par-tab-adds-chip", lambda: ([c["kind"] for c in s.ctl.rows(CHIPS)], bar()["draft"]), (["from"], ""))
    s.expect(TABPILL_CASE, lambda: pill_taken(s.ctl, {"detail": picked[0] if picked else None}), TABPILL_EXPECT)
    s.click("mac-search.skeleton.hold")             # the shot verb's hold: the skeleton stays until released
    s.key("Enter")
    s.expect("par-enter-swaps-to-skeleton", lambda: (s.ctl.rows("mac-search.skeleton")[0]["loading"], bar()["launched"]), (True, True))
    s.expect("par-skeleton-fills-the-pane", lambda: (lambda sk, lst: (sk["h"] >= const("LIST_ROW_PX"), sk["y"] >= lst["y"], sk["y"] + sk["h"] <= lst["y"] + lst["h"]))(
        s.ctl.measure("mac-search.skeleton")["frame"], s.ctl.measure("mac-main.list.view")["frame"]), (True, True, True))
    s.click("mac-search.skeleton.release")
    s.expect("par-skeleton-gives-way", lambda: (s.ctl.rows("mac-search.skeleton")[0]["loading"], sorted({r["thread"] for r in rows()}) == sorted({m["thread"] for m in MSGS if m["from"] == (picked[0] if picked else None)})), (False, True))
    s.expect(PILLQ_CASE, lambda: pill_query(s.ctl, picked[0] if picked else None), PILLQ_EXPECT)
    def inside_list():
        lst = s.ctl.measure("mac-main.list.view")["frame"]
        res = s.ctl.measure("mac-search.results")["frame"]
        return (res["x"] >= lst["x"] - 1 and res["x"] + res["w"] <= lst["x"] + lst["w"] + 1 and res["y"] >= lst["y"] - 1 and len(rows()) > 0, len(s.ctl.winframes()))
    s.expect("par-results-in-list-pane", inside_list, (True, wins[0] if wins else None))   # no second window, the rows sit inside the list pane
    # round 9 (embedding round 8): over a corpus the meaning index has not built yet, the first answer comes at once from the words;
    # as the index grows in the background the screen asks again by itself and its list becomes the core's newest answer
    s = j.step("J2.1", "Meaning hits join while the index builds")
    s.raw("fixture", "corpus", write_two_batches())
    s.key("/")
    launch(s, WORDS_QUERY)                            # item 3bb3d817: the first answer from the words needs a mail that holds every word
    first = []
    s._act(lambda: first.append((len(rows()), bar()["reasks"])))
    def grew():
        deadline = time.time() + REASK_WAIT_S
        while time.time() < deadline and bar()["reasks"] == 0:
            time.sleep(const("EMBED_NOTIFY_MS") / 1000.0 if has_const("EMBED_NOTIFY_MS") else 1.0)
        time.sleep(1.0)                               # the re-ask's answer lands on the same turn on the fixture core
        n = int(s.ctl.core("search/last/count") or 0)
        core_ids = [s.ctl.core("search/last/hit/%d/message_id" % i) for i in range(n)]
        screen = {r["id"] for r in rows()}
        return (first[0][0] > 0 if first else False, bar()["reasks"] >= 1, screen <= set(core_ids) and len(screen) > 0)
    # round 9 (main46): the re-ask has one source, the meaning index builder's update (MeaningSearch.onUpdate). With no weights on the
    # rig there is no builder, so the right screen asks once and never again: a re-ask there would be a loop the product invented.
    s.expect("par-meaning-reask-while-indexing", grew, (True, model_installed(), True))
    # round 10 (main44): an unquoted search must not wait for the meaning index: its word hits show at once and it is a recent
    launch(s, COLD_WORD)
    s.expect("par-unquoted-cold-index-answers-and-is-recent", lambda: (len(rows()) > 0, bar()["loading"], [r["text"] for r in s.ctl.rows("mac-search.recent")][:1]),
             (True, False, [COLD_WORD]))
    s.raw("fixture", "corpus", os.path.join(ROOT, "fixtures", "tests-j2.json"))
    s.key("Escape")

    # round 7 (owner 2026-11-08 11:30): the field takes the keys only from /, ⌘K or a click on it; Esc and a click elsewhere release them
    # round 10 (shortcuts lane's trace): a row click's focus change delivered after `/` claimed the field must not release it
    s = j.step("J2.7", "A late row change does not release a field just claimed")
    def race():
        try:
            s.ctl.key("Escape")                         # the list holds the keys; an Escape with nothing to close is no failure here
        except Exception:
            pass
        s.ctl.click("mac-main.list.row.1"); s.ctl.key("/")   # back to back: no read between the click and the key
        time.sleep(0.5)                                 # let SwiftUI deliver the click's focusIndex change
        b = s.ctl.rows(BAR)[0]
        return (s.ctl.uistate().get("surface"), b["focused"], b["owner"])
    s.expect("par-late-row-change-keeps-field", race, ("mac-search", True, "search"))
    s.key("Escape")

    s = j.step("J2.7", "Focus discipline: Esc returns to the list, a click releases the field, a letter from the list is a command")
    s.key("Escape")                                 # leave the search, then the list row is the only focus
    s._act(lambda: s.ctl.click("mac-main.list.row.1"))
    row0 = []
    s._act(lambda: row0.append(s.ctl.uistate().get("focus_message")))
    s.arrive(("key", "/"), "mac-search")
    s.expect("par-field-focused-by-key", lambda: (bar()["focused"], bar()["owner"]), (True, "search"))
    query(s, "Linea")
    s.key("Escape")
    s.expect("par-esc-returns-focus-to-list", lambda: (bar()["text"], bar()["dropdown"], bar()["focused"], bar()["active"], s.ctl.uistate().get("surface"), s.ctl.uistate().get("focus_message")),
             ("", False, False, False, "mac-main", row0[0] if row0 else None))
    # click away: the row takes the focus, the field lets go, its words stay
    s.key("/")
    query(s, "Linea")
    s.click("mac-main.list.row.2")
    s.expect("par-click-away-releases-field", lambda: (bar()["focused"], bar()["dropdown"], bar()["text"], s.ctl.uistate().get("surface"), s.ctl.uistate().get("focus_index")),
             (False, False, "Linea", "mac-main", 2))
    # the field let go but still shows its words: Esc, with the list holding the keys, clears them
    s.key("Escape")
    s.expect("par-esc-clears-released-field", lambda: (bar()["text"], bar()["active"], s.ctl.uistate().get("surface")), ("", False, "mac-main"))
    s.key("/")
    query(s, "Linea")
    s.click("mac-main.list.row.2")
    # a letter typed with the list holding the keys is a command: R opens the quick reply, the search text is untouched
    s.key("R")
    s.expect("par-letter-from-list-is-a-command", lambda: (s.ctl.uistate().get("surface"), len(s.ctl.rows("mac-main.quickreply.field")), bar()["text"]), ("mac-reader", 1, "Linea"))
    s.type("hello")
    s.expect("par-typed-text-lands-once", lambda: (s.ctl.rows("mac-main.quickreply.field")[0]["text"], bar()["text"], bar()["focused"]), ("hello", "Linea", False))
    for _ in range(3):                              # the reply, the reader, then the field's words: back to a clean list
        s._act(lambda: s.ctl.key("Escape"))

    s = j.step("J2.2", "Chips, grouping, account mark, attachment named, recents")
    s.key("/")
    launch(s, 'from:"Linea Supply" has:attachment')            # the colon forms make the pills (A142); plain words make none (NOPILL_CASE below)
    narrow = []
    s._act(lambda: narrow.append(len(rows())))
    idx = []
    s.expect("par-chip-attachment-shown", lambda: chip_attachment_or_state(s.ctl), True)
    s._act(lambda: idx.extend(i for i, c in enumerate(s.ctl.rows(CHIPS)) if c["kind"] == "attachment"))
    s.click("mac-search.chips.%d.remove" % (idx[0] if idx else 0))
    s.expect("par-chip-remove-widens", lambda: (len(rows()) > (narrow[0] if narrow else 0), [c["kind"] for c in s.ctl.rows(CHIPS)]), (True, ["from"]))
    launch(s, GROUP_QUERY)                            # item 3bb3d817: thr-01 holds three matches of these words (the meaning query's are gone)
    def grouped():
        seen, last = set(), None
        for r in rows():
            if r["thread"] != last and r["thread"] in seen:
                return False
            seen.add(r["thread"]); last = r["thread"]
        return len(rows()) > 0 and len({r["thread"] for r in rows()}) == len(rows())
    s.expect("par-grouped-by-thread", grouped, True)
    launch(s, WORDS_QUERY)                            # item 3bb3d817: the cases below read matches, and the meaning query is held whole by no mail
    s.expect(VIEW_CASES[0], lambda: view_served(s.ctl), VIEW_EXPECT)
    s.expect(VIEW_CASES[1], lambda: tinted(s.ctl), WORDS_TINT)
    s.expect(COUNT_CASE, lambda: count_line(s.ctl), COUNT_EXPECT)                    # "N matches" alone (A149), the rows after the divider all meaning
    s.expect(EVIDENCE_CASE, lambda: match_evidence(s.ctl), EVIDENCE_EXPECT)          # every match row says where the letters are, in bold (A149)
    s.expect(DIVIDER_CASE, lambda: divider_and_stage(s.ctl, model_installed()), divider_expect(model_installed()))
    s.expect("par-account-mark-on-result", lambda: len(rows()) > 0 and all(r["account"] == ACC[r["id"]] and bool(r["account_name"]) and r["colour"].startswith("#") for r in rows()), True)
    launch(s, "quote")
    s.expect("par-attachment-named-in-row", lambda: ({r["thread"]: r["attachment"] for r in rows()}.get(EXP["record1_thread"]),
                                                    all(r["attachment"] is None or r["attachment"] in ATT[r["id"]] for r in rows())), (EXP["record1_attachment"], True))
    # round 12: a chip typed after a launched search narrows the list with no Enter; filters alone do too
    before = []
    s._act(lambda: before.append(threads_shown(s.ctl)))
    s.expect(LIVE_CASES[0], lambda: chip_narrows_live(s.ctl, before[0] if before else []), (True, True, True, True))
    s.expect(LIVE_CASES[1], lambda: filters_alone_live(s.ctl), sorted(LINEA_THREADS))
    s.expect("par-recent-kept", lambda: [r["text"] for r in s.ctl.rows("mac-search.recent")][:2], ["quote", WORDS_QUERY])
    query(s, "")
    s.expect("par-recents-in-dropdown", lambda: (bar()["dropdown"], [r["kind"] for r in dd()][:1], dd()[0]["title"]), (True, ["recent"], "quote"))
    s.click("mac-search.dropdown.1")
    s.expect("par-recent-reuses", lambda: (bar()["text"], bar()["launched"], len(rows()) > 0), (WORDS_QUERY, True, True))
    for w in ("alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "india", "juliet"):
        launch(s, w)                                  # ten more searches: the kept list stays at its cap
    s.expect("par-recents-capped", lambda: len(s.ctl.rows("mac-search.recent")), int(const("SEARCH_RECENT_MAX")))
    s.expect(NOPILL_CASE, lambda: words_stay_words(s.ctl), NOPILL_EXPECT)            # the owner's picture: "email from consultant" makes no "From c" pill
    s.expect(LONGER_CASE, lambda: shorter_word_finds_longer_form(s.ctl), LONGER_EXPECT)   # "print" finds the mail that holds it only in "Printwerk" (A149)

    # round 3 (lane list-p1): the list in three parts, the closed line, the passage on the row, the sender-domain filter (SPEC A150, A152, A153)
    s = j.step("J2.9", "Three parts, the closed line, the passage, the domain filter")
    s.raw("fixture", "corpus", os.path.join(ROOT, "fixtures", "tests-j2.json"))
    s.key("/")
    launch(s, WORDS_QUERY)                            # item 3bb3d817: three parts need a match, and the meaning query is held whole by no mail
    s.expect(PARTS_CASES[0], lambda: parts_in_order(s.ctl), PARTS_ORDER_EXPECT)
    s.expect(PARTS_CASES[3], lambda: count_line_parts(s.ctl), (True, True, True))
    s.expect(PARTS_CASES[1], lambda: line_by_enter(s.ctl), line_by_enter_expect(s.ctl))
    # the new search must be NEW words: the same words with the field unchanged are not launched again (launch() presses no Enter, and Return there opens
    # the first result, SearchMacModel.fieldMode); main92 relaunched the same words and read the line still open
    s.expect(PARTS_CASES[2], lambda: line_by_click(s.ctl, lambda: launch(s.ctl, "quote")), (True, False, False))
    s.expect(EVERY_CASE, lambda: every_word_match(s.ctl), (True, True))   # item 3bb3d817: "quote signed" lists msg-01's conversation, msg-s05 (one word) is no match
    launch(s, CONTENT_QUERY)
    s.expect(PARTS_CASES[4], lambda: attachment_passage(s.ctl, EXP["photo_attachment"], CONTENT_QUERY), (True, True, True, "match"))
    s.expect(PARTS_CASES[5], lambda: domain_flow(s.ctl, LINEA_ADDR[0].split("@")[1]), (["search", "domain"], True, (["domain"], [LINEA_ADDR[0].split("@")[1]], "from:@" + LINEA_ADDR[0].split("@")[1]), True))
    s.expect(PARTS_CASES[6], lambda: no_match_state(s.ctl, LINEA_ADDR[0].split("@")[1]), (True, True, True))
    s.key("Escape")

    # lane reader-passage (SPEC A152): the reader opens at the mail that holds the selected result's passage and marks it: a text result, an attachment result, a
    # result whose passage is in an earlier mail of the conversation; then the search closes and nothing is marked
    s = j.step("J2.4", "The reader marks the passage of the selected result")
    s.raw("fixture", "corpus", os.path.join(ROOT, "fixtures", "tests-j2.json"))
    s.key("/")
    s.expect(PASSAGE_CASE, lambda: reader_marks_passage(s.ctl), PASSAGE_EXPECT)
    s.expect(INVIEW_CASE, lambda: passage_in_view_stays(s.ctl), INVIEW_EXPECT)
    s.expect(BAND_CASE, lambda: results_foot_band(s.ctl), False)
    s.expect(RELATED_FILE_CASE, lambda: related_file_stays(s.ctl), RELATED_FILE_EXPECT)   # round 18: main107 quit here

    # Enter on a result opens the thread on the reader; Escape brings the results back with the typed field and its chips kept
    s = j.step("J2.4", "Open a result, come back")
    launch(s, 'from:"Linea Supply" has:attachment')
    held = []
    s._act(lambda: held.append((bar()["text"], sorted(c["kind"] for c in s.ctl.rows(CHIPS)), len(rows()))))
    first = []
    s.key("Down")                                     # the second result: not the message the inbox already had focused
    s._act(lambda: first.append(rows()[1]["id"] if len(rows()) > 1 else None))
    # the search must hold at least two results here, else the case judges nothing (main52: one result, the reader opened it, "None" expected)
    def two_results():
        ids = [r["id"] for r in rows()]
        return (len(ids) > 1, ids if len(ids) <= 1 else "ids ok")
    s.expect("par-two-results-to-step-through", two_results, (True, "ids ok"))
    s.arrive(("key", "Enter"), "mac-reader")
    s.expect("par-enter-opens-reader", lambda: (s.ctl.rows("mac-main.reader.subject")[0]["id"], s.ctl.rows("mac-main.reader.subject")[0]["resting"]), (first[0] if first else None, False))
    s.arrive(("key", "Escape"), "mac-search")
    s.expect("par-escape-keeps-results", lambda: (bar()["text"], sorted(c["kind"] for c in s.ctl.rows(CHIPS)), len(rows())), held[0] if held else None)

    # a smart folder is saved from the results header; the column lists it with its thread count
    s = j.step("J2.5", "Save as smart folder")
    cnt = []
    s._act(lambda: cnt.append(len({r["thread"] for r in rows()})))
    s.click("mac-search.save-as-smart-folder")
    s.type("Parity folder")
    s.click("mac-search.save-as-smart-folder.confirm")
    # round 11: `threads` is the field's reading of the rule (one id per thread); `count` is the core's row count for the folder's query
    s.expect("par-folder-in-column", lambda: [(r["title"], len(set(r["threads"])), isinstance(r["count"], int)) for r in s.ctl.rows("mac-search.sidebar.smart-folders") if r["title"] == "Parity folder"],
             [("Parity folder", cnt[0] if cnt else 0, True)])
    # the core refuses a name that exists, in its own words, in the field
    s.click("mac-search.save-as-smart-folder")
    s.type("Parity folder")                           # saved a step ago: the corpus file defines no folders of its own
    s.click("mac-search.save-as-smart-folder.confirm")
    s.expect("par-duplicate-refused", lambda: ("exists already" in s.ctl.rows("mac-search.save-as-smart-folder.field")[0]["says"],
                                               [r["title"] for r in s.ctl.rows("mac-search.sidebar.smart-folders")].count("Parity folder")), (True, 1))
    s.key("Escape")                                   # the name field closes, the field is as it was

    # round 8 (fn-search-hybrid): a saved folder lists exactly the threads its query lists, plain words (the core's fused hits) included
    s = j.step("J2.5", "A smart folder lists the same threads as its query")
    launch(s, EXP["meaning_query"])
    shown = []
    s._act(lambda: shown.append(sorted({r["thread"] for r in rows()})))
    s.click("mac-search.save-as-smart-folder")
    s.type("Meaning folder")
    s.click("mac-search.save-as-smart-folder.confirm")
    s.expect("par-folder-same-threads", lambda: ([sorted(r["threads"]) for r in s.ctl.rows("mac-search.sidebar.smart-folders") if r["title"] == "Meaning folder"],
                                                 EXP["meaning_query_semantic_only_thread"] in (shown[0] if shown else [])),
             ([shown[0] if shown else None], True))

    # round 8 (row b190aaad): a hit read out of an attachment names the file and says it is in the attachment
    s = j.step("J2.8", "A match inside an attachment")
    # round 12 (main54): the Enter lands in the same breath as the typed words; the screen must launch those words, not a row the old text highlighted
    s.expect(STALE_ENTER, lambda: enter_takes_typed(s.ctl, CONTENT_QUERY), (CONTENT_QUERY, CONTENT_QUERY))
    s.expect("par-attachment-content-named", lambda: [(r["attachment"], r["matched_in"], r["attachment_says"]) for r in rows() if r["id"] == PHOTO_MSG],
             [(EXP["photo_attachment"], "attachment-content", "in the attachment")])
    s.expect("par-attachment-content-french", lambda: catalogue_fr("in the attachment"), "dans la pièce jointe")

    # round 12 (board row 8bf73cf8): the snooze picker's own words after "before" narrow the list to the threads before the picker's time
    import datetime
    s = j.step("J2.2", "The snooze picker's date words")
    secs = "%.0f" % (datetime.datetime(*DATE_NOW).timestamp() - time.time())
    s.raw("clock", "advance", secs)                   # the fixture's clock stands at DATE_NOW: the three cut points fall between the corpus's threads
    s.expect(DATE_PILL_CASE, lambda: date_words(s.ctl), DATE_EXPECT)       # A142: typed date words make no pill; the dropdown offers it, Tab makes it
    s.expect(BARE_CASE, lambda: bare_date_words(s.ctl), BARE_EXPECT)       # a snooze word typed alone stays a word
    s.raw("clock", "advance", "%.0f" % -float(secs))
    s.expect("par-date-words-in-french", lambda: all(catalogue_fr(k) for k in ("Later today", "Tomorrow morning", "Next week", "Before %@")), True)
    s.expect(FROM_CASE, lambda: from_prefix_people(s.ctl), FROM_EXPECT)       # "from Linea": no pill, a From row per address
    s.expect(FROMROW_CASE, lambda: from_row_replaces_typed_from(s.ctl), FROMROW_EXPECT)

    # round 8 (fn-search-hybrid): while accounts are still read in, the results say which ones (14 of 30 indexed: 15 named, the failing account is not loading)
    s = j.step("J2.1", "Partial results name the accounts still loading")
    s.raw("fixture", "indexing", "14")
    launch(s, EXP["meaning_query"])
    def loading_named():
        core_ids, n = still_loading(s)
        screen = sorted(r["id"] for r in s.ctl.rows("mac-search.loading-accounts"))
        return (screen == core_ids, n, ("%d accounts" % n) in bar()["says"])
    s.expect("par-loading-accounts-named", loading_named, (True, LOADING_EXPECTED, True))
    s.raw("fixture", "indexing", "all")

    # round 11: a saved folder opens from the main sidebar: its results fill the list, the header names it, its count follows an archive
    s = j.step("J2.5", "A saved smart folder opens in the main list")
    s.key("Escape")                                   # leave the search: the main window's list and sidebar take the clicks
    s.expect(FOLDER_CASE[0], lambda: folder_opens(s.ctl, "Parity folder"), FOLDER_EXPECT[FOLDER_CASE[0]])
    s.expect(FOLDER_CASE[1], lambda: folder_header(s.ctl, "Parity folder"), FOLDER_EXPECT[FOLDER_CASE[1]])
    s.expect(FOLDER_CASE[2], lambda: folder_count(s.ctl, "Parity folder"), FOLDER_EXPECT[FOLDER_CASE[2]])
    s.click("mac-main.sidebar.mailbox.all-inboxes")   # the inbox again: the folder gives way

    # lane search-checks (A141): the second answer, the refusal in words, the reader beside the list, the speed
    s = j.step("J2.1", "The ranking line, the could-not-run note, the reader and the speed")
    s.key("/")
    s.expect(HOLD_CASE, lambda: ranking_in_progress(s.ctl, model_installed()), hold_expect(model_installed()))                # needs `fixture search-hold on|off`: NOT JUDGED without it
    s.expect(REFUSE_CASE, lambda: could_not_run(s.ctl), REFUSE_EXPECT)                 # needs `fixture search-refuse on|off`: NOT JUDGED without it
    s.expect(READER_CASE, lambda: reader_follows_result(s.ctl), READER_EXPECT)
    s.expect(READER_IN_CASE, lambda: reader_in_results(s.ctl), [True, True, True])
    s.expect(INDEX_CASE, lambda: index_line(s.ctl), INDEX_EXPECT)                      # needs `fixture index-progress <indexed> <queued>`: NOT JUDGED without it
    for name, fn in zip(PERF_CASES, (letter_to_suggestions_ms, return_to_first_rows_ms)):
        def timing(fn=fn):
            ok, got, detail = judge_timing(fn, s.ctl)
            s._eg = ("< %s ms" % const(PERF_BUDGET_KEY), "%.1f ms" % got)
            return ok, detail
        s._record(name, timing)
    s.key("Escape")

    # doors (doors-search): every control of the search screen is clicked once, its visible outcome read
    s = j.step("J2.9", "Doors: every control of the search screen")
    s.key("Escape")                                   # the list holds the keys, the search is closed
    trace = []                                        # one DoorTrace for the step: an earlier door's last act is what a later door met
    for k in DOOR_ORDER:
        def door(k=k):
            if not trace or trace[0].c is not s.ctl:
                trace[:] = [DoorTrace(s.ctl)]
            ok, got, detail = door_result(trace[0], k)
            s._eg = (DOOR_EXPECT[k], got)
            return ok, detail
        s._record(DOOR_CASES[k], door)
    s.key("Escape")                                   # the search closes: the inbox again
    s._act(lambda: s.ctl.key("Escape"))

    # lane search-return (owner: "WHEN pressing cmd+k enter is to validate the search"): Return runs the typed words; a pointer resting on a row never changes it
    s = j.step("J2.1", "Return in the field runs the typed words")
    for k in RETURN_CASES:
        s.expect(k, lambda k=k: RETURN_JUDGE[k](s.ctl), RETURN_EXPECT[k])
    # lane miss-search-bar: the bottom key bar during the search is the search's set (never the composer's), the list's set once it closes
    for k in BAR_CASES:
        s.expect(k, lambda k=k: BAR_JUDGE[k](s.ctl), BAR_EXPECT[k])
    # lane gap-search-cases: the same Return as a REAL key event, which reaches the field's own handler (the socket's `key` never does), and its positive control
    for k in REAL_CASES:
        s.expect(k, lambda k=k: REAL_JUDGE[k][0](s.ctl), REAL_JUDGE[k][1])
    leave_search(s.ctl)

    # each smart folder carries an Edit control that opens the rule editor
    s = j.step("J2.6", "Edit a saved folder")
    s.arrive(("click", "mac-search.sidebar.smart.0.edit"), "mac-labels-contacts")

    # lane reader-result (SPEC A148): a result in no inbox tab (the test engine's sent seed, launch corpus only): the reader shows it, Enter opens it
    # with the results kept, Down walks the results, Esc gives them back, a second Esc closes the search
    s = j.step("J2.4", "A result outside the inbox rows: the reader shows it and Enter opens it")
    s.raw("fixture", "reset")                         # the launch corpus again: only it carries the sent seed (FixtureCore.resetMessages)
    s.key("/")                                        # the search opens (its field's click door answers only while it is open)
    for k in RESULT_CASES:
        s.expect(k, lambda k=k: RESULT_JUDGE[k][0](s.ctl), RESULT_JUDGE[k][1])

    # J2.2's sent-only person on the same launch corpus: its sent seed is the one mail of the test engine written TO someone who never wrote
    # (fay.lund@fixture-example.com, FixtureSearchBook.sentSeed). main105 ran this case on tests-j2.json, which holds no sent mail: NOT JUDGED on every run.
    s = j.step("J2.2", "A person only in sent mail is offered as To (the launch corpus's sent seed)")
    s.expect(SENT_CASE, lambda: sent_only_person(s.ctl), True)               # NOT JUDGED only on a corpus with no sent mail


MUTANTS = """# J2.1/arrives-by-control | the search key opens nothing
# J2.1/par-no-new-window | the search key opens a second window
# J2.1/par-field-focused | the search key leaves the cursor out of the field
# J2.1/par-results-in-list-pane | the results draw outside the list pane
# J2.1/par-dropdown-people | the dropdown lists no From row for a person, or one address of Linea Supply (the people loop of SearchSuggest.build deleted, or it reads the rows on screen and not the engine's `people/prefix/<letters>`)
# J2.1/par-dropdown-only-suggestions | an action row (a command by name) shows in the dropdown
# J2.1/par-dropdown-subject-and-attachment-rows | the Subject row or the Attachment row is missing, or the Attachment row does not carry the typed words (the two `add(.subject` / `add(.attachment` lines of SearchSuggest.build)
# J2.1/par-suggestions-in-the-drawn-order | the suggestions come in another order (Recent before the people, To before From, a Label above the Subject row), the `group` numbers go back, or the People rows are not the engine's list
# J2.1/par-tab-on-a-from-row-makes-the-pill-with-the-address | Tab on a From row makes a pill with the person's name, not the address (`let value = p.address` in SearchSuggest.build), or leaves the typed letters in the draft
# J2.1/par-pill-is-sent-to-the-engine-as-its-colon-form | the engine is asked the words without the pill (SearchEngine.engineQuery drops the chips), or the pill does not filter the list
# J2.1/par-highlight-moves | Down leaves the highlight on the first row
# J2.1/par-tab-adds-chip | Tab does nothing
# J2.1/par-enter-swaps-to-skeleton | Enter shows no skeleton rows
# J2.1/par-skeleton-gives-way | the skeleton never gives way to the results
# J2.7/arrives-by-control | the search key opens nothing
# J2.7/par-field-focused-by-key | the search key leaves the field without the keys
# J2.7/par-esc-returns-focus-to-list | Escape leaves the text, the dropdown or the field's hold in place, or the list row loses its focus
# J2.7/par-click-away-releases-field | a click on a list row leaves the field holding the keys
# J2.7/par-letter-from-list-is-a-command | R typed with the list holding the keys goes to the search field
# J2.7/par-typed-text-lands-once | the reply's words also show in the search field
# J2.7/par-esc-clears-released-field | Escape with the field released leaves its words on screen
# J2.2/par-chip-attachment-shown | the typed form has:attachment makes no pill (the `has` colon form of SearchQuery.typed deleted), or the chip rows are served empty (a `naming` that stays true in SearchMacRows `mac-search.chips`); the failure line names the screen's state
# J2.2/par-chip-remove-widens | the chip's remove button does nothing
# J2.2/par-grouped-by-thread | results sit in score order, one thread split apart
# J2.2/par-account-mark-on-result | the account mark shows no account
# J2.2/par-attachment-named-in-row | a result never names the attachment the words matched
# J2.2/par-recent-kept | a launched search is not remembered
# J2.2/par-recents-capped | recent searches grow past the cap
# J2.2/par-recents-in-dropdown | an empty field opens no recent searches
# J2.2/par-recent-reuses | a recent search does not refill the field and run
# J2.4/arrives-by-control | Enter on a result opens nothing
# J2.4/par-two-results-to-step-through | the search returns one result for the two-word query
# J2.4/par-enter-opens-reader | Enter opens the first result, not the selected one
# J2.4/par-escape-keeps-results | Escape from the thread opens the results with an empty field
# J2.4/arrives-by-control | Escape does not return to the results
# J2.5/par-folder-in-column | a saved folder is not listed in the column
# J2.5/par-duplicate-refused | a second folder with the same name is saved silently
# J2.5/par-folder-same-threads | a saved folder counts its rule on the local engine, not the field's fused hits
# J2.5/par-folder-click-lists-core-rows | a click on a saved folder in the main sidebar leaves the list on its tab (AppModel.smartFolderId never written)
# J2.5/par-folder-header-names-it | the list header keeps the account filter's name and unread count, not the folder's
# J2.5/par-folder-count-follows-an-archive | the folder's count is kept from the save: an archived row stays in the count (the `s.syncFolders()` call in SearchMacRows.folderRows deleted)
# J2.1/par-skeleton-fills-the-pane | the skeleton rows cannot be measured or draw outside the list pane (`.whooshAx("mac-search.skeleton")` back to `.accessibilityIdentifier` in SearchSkeleton.swift)
# J2.2/par-results-view-served | the header, the divider's words for the stage or the footer row of the results pane is not served (the `mac-search.header` or `mac-search.sections` entry of SearchMacRows deleted)
# J2.2/par-count-line-says-matches-and-close-in-meaning | the count line counts every row as a match (`matchedConversations` read as `results.count`), or says "N matches · M close in meaning" again with a `close_line` row (the owner's A149: "makes no sense to have 1 match every time"), or `close` is dropped from `mac-search.header`, or an exact hit sits after the first close one
# J2.2/par-every-match-shows-its-evidence | a row of the match group serves `evidence_in` "none" or no `evidence_bold` (the evidence of SearchHits.fromHits not computed, or the `evidence_in`/`evidence_bold` fields of SearchMacRows `mac-search.results` dropped)
# J2.2/par-divider-starts-the-close-group-and-follows-the-stage | the divider says "Close in meaning" when no model answered (SearchResultsList.dividerTitle ignores the stage), or the stage is not read from `search/last/stage`
# J2.2/par-typed-words-make-no-pill | typed words become a pill again ("from" before a word, "has attachment", "before June": the word branches of SearchQuery.typed), or the launched query differs from the field's text
# J2.2/par-matched-words-tinted | a core hit's row serves no tinted word (the subjectMarks fallback of `highlights` in SearchMacRows deleted)
# J2.2/par-chip-narrows-without-enter | the list waits for Enter after a chip is typed (the `narrowLive()` call in SearchMacModel.typedChanged deleted)
# J2.2/par-filters-alone-narrow-without-enter | filters typed alone wait for Enter (`filtersAlone` taken out of narrowLive's guard), or a filter typed over the words (same chip, words gone) changes nothing (`|| (filtersAlone && !sameWords)` taken out of narrowLive's first guard)
# J2.8/par-enter-launches-what-was-typed | Enter accepts the row the old text highlighted (`highlight` kept as a plain stored value again): a recent search takes the typed words' place
# J2.2/par-date-words-are-offered-as-a-pill-with-the-pickers-day | typed `before <picker word>` makes a pill by itself, the dropdown offers no filter row for it, Tab makes a pill of another day (not the picker line's day, SearchDateWords.match), or the list ignores the day (SearchEngine.passes)
# J2.2/par-bare-snooze-word-stays-a-word | a snooze word typed alone, or beside another word, becomes a before pill (`bareDateWord()` called again from SearchQuery.typed), or the engine is asked something else than the typed words
# J2.2/par-from-prefix-lists-from-rows | "from Linea" makes a pill by itself, or the dropdown lists no From row per address of the supplier (the `previous == "from"` words of SearchSuggest.build, or the people read)
# J2.2/par-from-row-replaces-the-typed-from | Tab on a From row keeps the typed word "from" or the letters in the draft (the `replaces` of the From row not "from <letters>")
# J2.2/par-person-only-in-sent-mail-is-offered-as-to | a person only in sent mail is offered as From, or not offered at all (the `toCount > 0` test of SearchSuggest.build, or the people read ignoring sent mail); NOT JUDGED while the corpus holds no sent mail
# J2.2/par-date-words-in-french | the catalogue lacks one of the picker's words, or "Before %@", in French
# J2.8/par-attachment-content-named | a match read out of an attachment shows no file name or no "in the attachment"
# J2.8/par-attachment-content-french | the catalogue lacks "in the attachment" in French
# J2.1/par-loading-accounts-named | the results name no account still loading, or name a failing one (`&& $0.lastError == nil` deleted in SearchIndexing.swift names acc-18: 16, red)
# J2.1/par-meaning-reask-while-indexing | with the weights on the rig: the screen never asks again when the meaning index grows (the `MeaningSearch.onUpdate = ` line in SearchMeaningRefresh.swift deleted); without them: every answer counts as a re-ask (`reaskCount += 1` moved into askCore)
# J2.1/par-ranking-line-shows-while-the-second-answer-is-held | with the second answer held the list shows the date-ordered tail, no ranking line, or the selection jumps when the ranked list replaces it (`tailPending` not drawn; `receive` not keeping the selected id); NOT JUDGED without `fixture search-hold`
# J2.1/par-could-not-run-note-says-the-engines-words | a refused search lists rows, says no sentence or no engine words (SearchRefusedState, the `mac-search.refused` entry of SearchMacRows); NOT JUDGED without `fixture search-refuse`
# J2.1/par-reader-follows-the-selected-result | the reader keeps the first result while Down moves the selection (`shownMessageId` not following `selectedResultId`)
# J2.1/par-real-return-in-the-field-launches-the-typed-words | a REAL Return typed in the field does not launch the search (SearchField.swift: `.onSubmit { model.claim(); _ = model.enter() }` -> `.onSubmit { }` and `.onKeyPress(.return) { model.claim(); return model.enter() ? .handled : .ignored }` -> `.onKeyPress(.return) { .ignored }`), so the key reaches the list's Enter and opens the quick reply
# J2.1/par-real-return-from-the-list-opens-the-reply | the control: a real Return with the list holding the keys opens no quick reply (Commands.enter), so the case before it would prove nothing
# J2.1/par-reader-never-shows-a-mail-outside-the-results | the reader keeps a mail that is not among the listed results while Down and Up move the selection (`shownRow` not following `selectedResultId`, AppModel.swift)
# J2.1/par-index-line-shows-while-the-meaning-index-builds | no line while the index builds, or the line stays after it is done, or it carries other figures (SearchResultsList.indexLine without its `guard IndexProgress.isBuilding(s) else { return nil }`, the `mac-search.index-note` entry of SearchMacRows); NOT JUDGED without `fixture index-progress`
# J2.2/par-a-shorter-word-finds-the-mail-that-holds-its-longer-form | the typed letters match only whole words or prefixes: `print` finds no mail whose only hit is the attachment name printwerk-offer.pdf (FixtureSearch.swift: `r.attachments.first(where: { $0.lowercased().contains(t) })` -> `r.attachments.first(where: { $0.lowercased() == t })`; the engine's SUBSTRING_SCAN_MAX loop of core/crates/search-text/src/query.rs)
# J2.1/par-suggestions-under-50-ms-after-a-typed-letter | the suggestions are rebuilt from a scan of every mail on each letter (the people read or `suggestions` made slow); NOT JUDGED above PERF_LOAD_MAX, or while `type` is not measured
# J2.1/par-first-result-rows-under-50-ms-after-return | the first rows wait for the full ranking before they show (the first answer not drawn at once); NOT JUDGED above PERF_LOAD_MAX
# J2.7/par-late-row-change-keeps-field | every focusIndex change releases the field, even one that lands after the claim
# J2.1/par-unquoted-cold-index-answers-and-is-recent | an unquoted search waits for the meaning index: no rows, no recent until it answers
# J2.9/par-door-open-opens-the-search | the search door (`mac-search.open`) opens nothing (the `mac-search.open` entry of SearchMacFeature.clickTargets deleted)
# J2.9/par-door-clear-button-empties-the-field | the field's clear button leaves the words or the results on screen (SearchField.swift: `model.clearField()` removed from the button)
# J2.9/par-door-bar-click-takes-the-keys | a click on the field after a click-away does not give the keys back (the `mac-search.bar` door or `claim()` deleted)
# J2.9/par-door-dropdown-tab-adds-the-chip | the dropdown row's Tab door adds no chip (the `.tab` door removed, or `tab()` returns false)
# J2.9/par-door-dropdown-hover-tints-the-row | the pointer on a dropdown row gives it no tint of its own, or moves the keyboard highlight to it (the `.hover` door removed or setting `highlight` again, or SearchDropdown's `onHover` not setting `model.hovered`)
# J2.1/par-return-pointer-over-row-runs-typed | the pointer resting on a suggestion row changes what Return does: the row is accepted and the typed words are lost from the list and the recents (SearchDropdown's `onHover` or the `.hover` door writing `highlight` again, or `enter()` not asking `keyed`)
# J2.1/par-return-after-down-takes-the-row | Down then Return runs the typed words instead of the row Down moved to (`move()` not marking the row `keyed`, or `enter()` ignoring it)
# J2.1/par-return-down-then-letter-runs-typed | Down, a typed letter, then Return takes the row the old text had highlighted (`keyed` not tied to the text it was set for)
# J2.1/par-click-on-row-takes-it | a click on a dropdown row launches the typed words, not the row (the `mac-search.dropdown.<i>` door or onTapGesture not calling `choose(i)`)
# J2.1/par-bar-shows-search-keys-during-search | the bottom bar shows the composer's keys (Send, Draft, Discard, Pop out) or no search set while the search holds the keyboard or shows a validated search (`MacFootHints.surface` without its search branch)
# J2.1/par-bar-is-the-list-set-after-search-closes | the bottom bar keeps the search's set after the search closed (the search branch of `MacFootHints.surface` reading a stale flag)
# J2.9/par-door-recent-row-reruns-it | a recent search row refills nothing (`useRecent` not called by the `mac-search.recent.<i>` door)
# J2.9/par-door-result-click-opens-the-thread | a click on a result opens no thread, or Escape does not bring the results back (`openResult(i)` not called by the `mac-search.results.<i>` door)
# J2.9/par-door-return-after-typing-launches-the-search | Return typed after the list's own rows changed opens the quick reply and leaves the field's words unlaunched (the `app.rows.count != claimedRows` re-anchor of SearchMacModel.focusMoved deleted, the old `if holds, !naming, app.focusIndex != claimedAt { release() }` restored; or SearchField's Return handler and onSubmit without `model.claim()`)
# J2.9/par-door-letting-go-gives-up-the-cursor | the search lets go of the keys and the text field keeps its cursor (SearchField's `.onChange(of: model.holds)` that sets `focused` deleted)
# J2.9/par-door-arrows-and-tab-in-the-field-go-to-the-search | Down, Up or Tab typed in the field move the list's focus or do nothing (SearchField's `.onKeyPress(.downArrow/.upArrow/.tab)` deleted)\n# J2.9/par-door-escape-in-the-field-closes-the-search | Esc typed in the field leaves the words on screen or the search open (the field's `.onKeyPress(.escape)` or SearchMacModel.close deleted)\n# J2.9/par-door-save-name-escape-closes-the-field | Escape leaves the name field open, or the bar shows another text after it (the `if naming` branch of SearchMacModel.close deleted)
# J2.4/par-reader-shows-a-result-outside-the-inbox-rows | the reader says "No conversation open" (or shows another mail) for a selected result in no inbox tab (ReaderThread.make looks the id up in `m.rows` only, not `AppModel.knownRow`)
# J2.4/par-enter-opens-the-result-and-keeps-the-results | Enter on a result closes the search and the reader takes the focused inbox row (SearchMacActs.openResult calls `leave()` again)
# J2.4/par-arrows-walk-the-results-and-escape-returns-to-them | in the reader on a result Down moves the inbox focus and not the results (the reader's `next`/`previous` taken out of ReaderKeys), or Esc lands on the inbox list with the field holding no keys (`backToResults` not called)
# J2.4/par-reader-marks-the-passage-of-the-selected-result | the reader serves no mark for the selected result (`mac-main.reader.passage` says marked false), names the row's mail and not the mail that holds the passage (the earlier mail of the thread), serves no ringed file for an attachment result, or keeps the mark after the search closed (`ReaderFocus.syncPassage`, `ReaderPassage.of`, `ReaderPassage.reading`)
# J2.9/par-a-match-holds-every-typed-word | a mail that holds one of two typed words is listed among the matches (FixtureSearchBook.run without `holdsEvery`: any typed word makes a match)
# J2.4/par-the-results-keep-no-blank-band-at-their-foot | the search list keeps its empty missing-model line mounted: 10 pt of blank pane over the foot of the results
# J2.4/par-a-passage-already-in-view-does-not-move-the-reader | the reader scrolls to a marked passage that was already in view (the thread moves and the summary's top is cut)
# J2.4/par-a-related-mail-from-an-attachment-opens-and-the-app-keeps-answering | selecting a related mail found by its attachment ends the app or leaves it not answering (main107: AppKit raised in the reader's layout pass); the case fails by name, never as a pipe error in the next case
# J2.6/arrives-by-control | Edit opens nothing
# J2.9/par-list-in-three-parts-in-order | the rows are not match, related, rest in that order, the rest is listed while the line is closed, or the closed line is not served (the `part` field or the `mac-search.more` row of SearchMacRows; `results` of SearchMacModel returning every conversation)
# J2.9/par-closed-line-opens-and-closes-by-enter | Down does not reach the closed line, Enter on it opens no mail but does not toggle the rest, or the bottom bar keeps "Show all other mail" when it is open (SearchMacActs.move / enter, MacFootHints.searchSet)
# J2.9/par-closed-line-click-and-new-search-closes-it | a click on the closed line does nothing (no `mac-search.more` click target), or a new search leaves it open (`restOpen = false` missing in SearchMacActs.launch)
# J2.9/par-count-line-says-matches-and-related | the count line says only the matches when related mail is listed, or counts the rest (SearchMacModel.countLine(matches:related:))
# J2.9/par-attachment-result-shows-its-passage-line | a result found in an attachment serves no passage line, no source or not the file's name (SearchHits.found, the `source` fields of SearchMacRows)
# J2.9/par-domain-suggestion-tab-pill-and-engine-query | typing the first letters of a sender domain offers no "From domain" row under Search all mail, Tab makes a person's pill, or the engine is not asked `from:@<domain>` (SearchSuggest.build, SearchChip.canonical)
# J2.9/par-no-match-state-keeps-the-closed-line | with no match and nothing related the list shows rows, a count line, or no closed line selected (SearchMacModel.settleLine, countLine(matches:related:) for (0, 0))
"""


def main():
    if "--selftest" in sys.argv:
        return selftest()
    path = os.path.join(ROOT, "_captures", ".tmp", "par-searchmac-mutants.txt")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    open(path, "w", encoding="utf-8").write(MUTANTS)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    print_cases(r)                                    # one line per judged case, pass or fail: a pass is never read from the absence of a failure
    print("MUTANTS (a check is green only after it has been seen red on its mutant)")
    print("| check | mutant |")
    print("|---|---|")
    for l in MUTANTS.splitlines():
        k, v = l[1:].strip().split(" | ", 1)
        print("| %s | %s |" % (k, v))
    for line in r["fail_list"]:
        sys.stderr.write("FAIL %s\n" % line)
    for line in r["no_mutant"]:
        sys.stderr.write("NO MUTANT %s\n" % line)
    for line in r["stray_mutant"]:
        sys.stderr.write("STRAY MUTANT %s\n" % line)
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    return 0 if r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"] else 1


if __name__ == "__main__":
    sys.exit(main())
