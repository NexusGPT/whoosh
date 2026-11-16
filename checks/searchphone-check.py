#!/usr/bin/env python3
"""iPhone search (lane searchphone): the phone twin of journey J2 (docs/JOURNEYS.md J2.1 to J2.5, J2.8 for the meaning model's line).

  python3 checks/searchphone-check.py            judges the SOURCES of Sources/Features/SearchPhone: no literal text, no network, nothing
                                                 waits, every control has its id and its click, every sentence is in the catalogue
  python3 checks/searchphone-check.py --live     drives the RUNNING iPhone app over the control socket ($WHOOSH_SOCKET_DIR, docs/CONTROL-SOCKET.md):
                                                 sh scripts/launch-one.sh <iphone-bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/searchphone-check.py --live'
  python3 checks/searchphone-check.py --selftest the real sources must pass, the live cases must pass against a stand-in of the intended
                                                 screen, and each mutant (a source edit, or a stand-in that breaks one behaviour) must go
                                                 red on the case it breaks

Expected values come from fixtures/tests-j2.json (the search corpus) and docs/CONSTANTS.md, never from a built screen.
Prints 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0 (a check that could not run is not green).
The stand-in in --selftest repeats the intended rules in Python so the cases can be proven able to fail; it is not evidence about the app.
"""
import inspect
import json
import os
import re
import socket
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DIR = "Sources/Features/SearchPhone/"
NOTES = "docs/lanes/searchphone.md"
CATALOGUE = "Sources/Resources/Localizable.xcstrings"
FILES = ["Feature", "Model", "Acts", "Search", "Save", "Rows", "Words", "Filters", "Row", "Results", "Bar", "Cards", "Screen", "Parts", "More", "Domain", "Found", "Divider"]
CORPUS = os.path.join(ROOT, "fixtures", "tests-j2.json")
EXP = json.load(open(CORPUS, encoding="utf-8"))["expect"]
MSGS = json.load(open(CORPUS, encoding="utf-8"))["messages"]
AC = json.load(open(CORPUS, encoding="utf-8"))["attachment_content"]
CONSTANTS = os.path.join(ROOT, "docs", "CONSTANTS.md")
MONTHS = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"]
QUERY = EXP["meaning_query"]
TYPED_FIRST = "supplier price quote"          # what is on the field before the last word is typed
# Item 3bb3d817 (lane every-word): a match holds EVERY typed word (search-text query.rs; FixtureSearchBook.holdsEvery). No mail holds the four words of
# QUERY, so the two cases built on its one-word matches (the marked words, record 1's paperclip) read the answer of WORDS_QUERY, as journeys/J2.py does:
# msg-01 (record 1) and msg-s05 hold all three words. The sets: _captures/every-word/match-sets-5.out.
WORDS_QUERY = "quote price run"
# and the single filters narrow the answer of FILTER_QUERY: its matches hold every word whatever the model (msg-01 and its thread, msg-s02 after June on the
# same sender and account, msg-s05 with "print" inside printwerk-offer.pdf); QUERY's answer lists no mail after June once a model answers (thr-01 and thr-s05
# are related, msg-s02 is in the rest), so "before June" could not narrow it
FILTER_QUERY = "print run"     # also the grouping case's: msg-01, msg-01a and msg-01b (thr-01) hold both words
WORDS_TINT = {t: sorted({w for w in WORDS_QUERY.split() if re.search(r"\b%s\b" % w, m["subject"].lower())}) for m in MSGS
              for t in [m["thread"]] if t in EXP["meaning_query_must_include_threads"]
              and all(w in " ".join([m["subject"], m["body"], m["from_name"], m["from"]] + list(m.get("attachments") or [])).lower() for w in WORDS_QUERY.split())}
SENDER = EXP["people_chip_value"]
ACCOUNT = EXP["record1_account"]
NEW_SENTENCE = "Search by meaning is off: the meaning model is not on this iPhone."
LONG = 4000

# every control the screen has; the check taps each one once (docs/CONSTRAINTS.md: "every button and control is clicked once")
CONTROLS = ["open", "close", "field.clear", "field.submit", "filter.account", "filter.sender", "filter.date", "filter.attachment",
            "filter.account.clear", "filter.sender.clear", "filter.date.clear", "filter.attachment.clear", "filter.menu.close",
            "filter.account.all", "filter.account.%s" % ACCOUNT, "filter.sender.0", "filter.date.june", "results.0", "recent.0",
            "save", "save.confirm", "save.cancel", "meaning.download", "domain.0", "filter.domain.clear", "skeleton.hold", "skeleton.release"]


def const(name):
    """The first number of a constant's cell in docs/CONSTANTS.md (the Mac journey reads its limits the same way), never typed here."""
    for line in open(CONSTANTS, encoding="utf-8"):
        cells = [c.strip() for c in line.strip().strip("|").split("|")] if line.startswith("|") else []
        if len(cells) >= 2 and name in [n.strip().split(" (")[0] for n in re.split(r"[\u00b7/]", cells[0])]:
            return float(re.search(r"\d+(?:\.\d+)?", cells[1]).group(0))
    raise RuntimeError("no constant %s in docs/CONSTANTS.md" % name)


def read(rel):
    with open(os.path.join(ROOT, rel), encoding="utf-8") as fh:
        return fh.read()


def strip_comments(text):
    return re.sub(r"//[^\n]*", "", text)


def body_of(text, anchor):
    i = text.find(anchor)
    j = text.find("{", i) if i >= 0 else -1
    if j < 0:
        return None
    depth = 0
    for k in range(j, len(text)):
        depth += {"{": 1, "}": -1}.get(text[k], 0)
        if depth == 0:
            return text[j:k + 1]
    return None


# ---------------------------------------------------------------- the sources
def count_line(matches, related):
    """The count line in the Mac's words (SearchMacModel.countLine(matches:related:), the catalogue keys '%lld matches · %lld related' and kin)."""
    def m(n):
        return "1 match" if n == 1 else "%d matches" % n
    if matches == 0 and related == 0:
        return "No match"
    if related == 0:
        return m(matches)
    return ("No match" if matches == 0 else m(matches)) + " \u00b7 " + ("1 related" if related == 1 else "%d related" % related)


def load_sources():
    t = {f: read(DIR + "SearchPhone%s.swift" % f) for f in FILES}
    t["notes"] = read(NOTES)
    t["catalogue"] = read(CATALOGUE)
    return t


def judge_sources(t):
    """(name, ok, why) rows over a {key: text} dict."""
    code = {k: v for k, v in t.items() if k in FILES}
    bare = {k: strip_comments(v) for k, v in code.items()}
    allcode = "\n".join(bare.values())
    rows = []

    def add(name, ok, why):
        rows.append((name, bool(ok), why))

    lit = [(k, m.group(0)) for k, v in bare.items() for m in re.finditer(r'(?<![A-Za-z.])(?:Text|TextField|Button|Label)\("(?!\\\()[^"]*[A-Za-z]', v)]
    add("no-literal-text", not lit, "every user-facing string goes through Words.text; literal: %s" % (lit[:2] or "none"))

    net = re.findall(r"URLSession|URL\(string|https?://|NWConnection|Network\.|WKWebView", allcode)
    add("no-network", not net, "the search reads the local index and the local model; found %s" % (net[:2] or "nothing"))
    priv = re.findall(r"Application Support|SecItem|Keychain|NSHomeDirectory", allcode)
    add("no-private-reads", not priv, "no read of the app's own folder or the Keychain; found %s" % (priv[:2] or "nothing"))

    wait = re.findall(r"Task\.sleep|asyncAfter|usleep|sleep\(|DispatchSemaphore|\.wait\(", allcode)
    add("nothing-waits", not wait, "no timer or wait on the typing path; found %s" % (wait[:2] or "nothing"))

    typed = body_of(bare["Acts"], "func typedChanged()") or ""
    add("typing-never-waits", typed and "refresh(" not in typed and "run(command" not in typed and "build(" not in typed,
        "typedChanged only marks the text dirty; the search runs on the next turn (settle), never inside the key's frame")
    settle = body_of(bare["Acts"], "func settle()") or ""
    add("read-settles-first", "dirty" in settle and "refresh()" in settle, "a read of the screen runs a pending search first")

    ids = sorted(set(re.findall(r'accessibilityIdentifier\("(iphone-search\.[^"]+)"\)', allcode)))
    need = ["iphone-search.close", "iphone-search.field", "iphone-search.field.clear", "iphone-search.save", "iphone-search.save.confirm",
            "iphone-search.save.cancel", "iphone-search.meaning.download", "iphone-search.list", "iphone-search.header", "iphone-search.footer",
            "iphone-search.skeleton", "iphone-search.empty", "iphone-search.filters", "iphone-search.filter.menu", "iphone-search.divider", "iphone-search.domains"]
    missing = [i for i in need if i not in ids]
    dyn = ['iphone-search.results.\\(i)', 'iphone-search.recent.\\(i)', 'iphone-search.filter.\\(c.kind)', 'iphone-search.filter.\\(c.kind).clear']
    missing += [d for d in dyn if d not in allcode]
    add("every-control-has-id", not missing, "each control carries its iphone-search.<part> id; missing %s" % (missing or "none"))

    clicks = bare["Feature"]
    want = ["iphone-search.open", "iphone-search.close", "iphone-search.field.clear", "iphone-search.field.submit", "iphone-search.filter.menu.close",
            "iphone-search.filter.attachment", "iphone-search.save", "iphone-search.save.confirm", "iphone-search.save.cancel",
            "iphone-search.meaning.download", "iphone-search.filter.account.all", "iphone-search.filter.account.\\(a.id)",
            "iphone-search.filter.sender.\\(i)", "iphone-search.filter.date.\\(month.lowercased())", "iphone-search.results.\\(i)",
            "iphone-search.recent.\\(i)", "iphone-search.filter.\\(m.rawValue)", "iphone-search.filter.\\(k).clear",
            "iphone-search.domain.\\(i)", "iphone-search.skeleton.hold", "iphone-search.skeleton.release", "iphone-search.more"]
    gap = [w for w in want if w not in clicks]
    add("every-control-has-click", not gap, "each control is a socket click target; missing %s" % (gap or "none"))

    sizes = {k: v.count("\n") + 1 for k, v in code.items()}
    big = {k: n for k, n in sizes.items() if n > 200}
    add("file-size", not big, "components stay under 200 lines; over: %s" % (big or "none"))

    edge = re.findall(r"\.border\(|\.strokeBorder\(|Rectangle\(\)\.fill\([^)]*\)\.frame\(width:", allcode)
    add("no-edge-strip", not edge, "no coloured strip or stroke on a row's edge; found %s" % (edge[:2] or "nothing"))

    cat = json.loads(t["catalogue"])["strings"]
    keys = set(re.findall(r'Words\.text\("([^"]+)"', allcode))
    absent = sorted(k for k in keys if k not in cat)
    unlisted = [k for k in absent if k not in t["notes"]]
    add("strings-in-catalogue", not unlisted, "every sentence is in the catalogue or listed in the notes with its English text; unlisted %s" % (unlisted or "none"))

    consts = re.findall(r"static (?:let|var) (\w+)", "\n".join(re.findall(r"enum SearchPhoneMetrics \{.*?\n\}", bare["Row"], re.S)))
    nolist = [c for c in consts if "`%s`" % c not in t["notes"]]
    add("new-numbers-listed", consts and not nolist, "each number this screen adds is a private constant listed in the notes; unlisted %s" % (nolist or "none"))

    add("meaning-door-is-ai-setup", "AISetupDoors.fix(.meaning)" in bare["Save"] and "AISetupStatus.shared.status(.meaning)" in bare["Save"],
        "the missing-model line reads the AI setup's one status and its door is the AI setup's fix")
    add("domain-offers-are-the-macs", "SearchSuggest.build" in bare["Domain"] and "DomainRead.domains" in bare["Domain"],
        "the sender-domain offers are the Mac's own rows (SearchSuggest.build on DomainRead.domains), not worked out again on the phone")
    add("related-line-is-the-macs", "SearchResultsList.titles" in bare["Parts"] and "SearchMacModel.countLine" in bare["Parts"],
        "the thin line and the count line come from the Mac's functions (SearchResultsList.titles, SearchMacModel.countLine)")
    add("passage-is-the-engines", "r.found" in bare["Found"] and "r.senderMarks" in bare["Found"] and "wordRanges" not in bare["Found"],
        "the passage, its source and the bold letters are read from the engine's SearchResult (found, senderMarks), never found again by the phone")
    add("hit-mail-is-read", "hitMessageId" in bare["Acts"] and "openedHitMessageId" in bare["Acts"], "opening a result names the mail that holds the passage (the engine's hitMessageId)")
    add("same-search-as-mac", all(x in bare["Search"] for x in [".search(query:", "SearchEngine.fromHits", "SearchEngine.run", "SearchHitSink"]),
        "plain words go to the core's search, filters alone to the Mac's local engine, answers come on the Search stream")
    return rows


# ---------------------------------------------------------------- the live driver: the socket
class Socket:
    def __init__(self):
        d = os.environ.get("WHOOSH_SOCKET_DIR", "")
        self.path = os.path.join(d, "whoosh.sock")
        self.s, self.buf = None, b""
        self.tapped = set()
        self.refused = []

    def raw(self, verb, *args):
        if self.s is None:
            if not os.path.exists(self.path):
                raise RuntimeError("no socket at %s (the iPhone app is not running)" % self.path)
            self.s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            self.s.settimeout(5)
            self.s.connect(self.path)
        self.s.sendall((json.dumps({"verb": verb, "args": [str(a) for a in args]}) + "\n").encode())
        while b"\n" not in self.buf:
            chunk = self.s.recv(65536)
            if not chunk:
                raise RuntimeError("socket closed")
            self.buf += chunk
        line, self.buf = self.buf.split(b"\n", 1)
        return json.loads(line.decode())

    def rows(self, ax):
        r = self.raw("rows", ax)
        if r.get("ok") is not True:
            raise RuntimeError("rows %s: %s" % (ax, r.get("error", r)))
        return r.get("rows", [])

    def row(self, ax):
        rs = self.rows(ax)
        if len(rs) != 1:
            raise RuntimeError("rows %s: %d rows, one expected" % (ax, len(rs)))
        return rs[0]

    def click(self, ax):
        r = self.raw("click", ax)
        if r.get("ok") is not True:
            raise RuntimeError("click %s: %s" % (ax, r.get("error", r)))
        self.tapped.add(ax)

    def tap(self, ax):
        """A click whose refusal is a finding (the control is not wired), not the end of the run."""
        try:
            self.click(ax)
            return True
        except RuntimeError as e:
            self.refused.append(str(e))
            return False

    def type(self, text):
        r = self.raw("type", text)
        if r.get("ok") is not True:
            raise RuntimeError("type: %s" % r.get("error", r))

    def uistate(self):
        r = self.raw("uistate")
        return r.get("state", {})

    def core(self, path):
        return self.raw("core", "state", path).get("value")

    def corpus(self, path):
        r = self.raw("fixture", "corpus", path)
        if r.get("ok") is not True:
            raise RuntimeError("fixture corpus: %s" % r.get("error", r))

    def measure(self, ax):
        """`measure <ax>`: {ms} for an interaction, {frame} for a view. A refusal is a RuntimeError the case records as NOT JUDGED."""
        r = self.raw("measure", ax)
        if r.get("ok") is not True:
            raise RuntimeError("measure %s: %s" % (ax, r.get("error", r)))
        return r

    def settle(self):
        time.sleep(0.3)


# ---------------------------------------------------------------- the Mac journey's steps, one row each (journeys/J2.py)
# Every check call of the Mac journey's steps J2.1, J2.2, J2.3, J2.8, J2.4 and J2.5 (`s.expect`, `expect_le`, `expect_ctx`, `same`, `lt`, `_record`)
# is a key here, with what the phone does with it:
#   same         the phone case checks the same fixture value (the Mac literal and the phone literal are both quoted and both are in the code)
#   substituted  the phone repeats the step on its own control or reading (the reason says how the value differs)
#   not-driven   the phone has the thing but no live case reads it yet (named, so it is not hidden; it is not a pass of that step)
#   excluded     the phone has no such thing (the reason says why)
# A Mac step with no row, a row whose Mac step is gone or whose Mac literal moved, a phone case or literal that is not in cases(), or a row
# with no reason: each is a red row. Mac literals are read from the journey's own text, phone literals from cases() in this file.
MAC_JOURNEY = os.path.join(ROOT, "journeys", "J2.py")
MAC_STEPS = ["J2.1", "J2.2", "J2.3", "J2.8", "J2.4", "J2.5"]
# (Mac step/check, kind, phone case or None, Mac literal, phone literal, reason)
STEP_ROWS = [
    ("J2.1/dropdown-first-row-is-search-all-mail", "excluded", "results-while-typing", '"search"', "no Return pressed", "the Mac dropdown under the bar has no phone twin: the phone lists results as the words are typed"),
    ("J2.1/dropdown-subject-and-attachment-rows", "excluded", "filters-listed", '"attachment"', '"attachment"', "the Mac dropdown rows have no phone twin; the phone offers its Attachment filter as a chip under the field"),
    ("J2.1/first-screen-ms", "excluded", None, "SEARCH_FIRST_SCREEN_MS", None, "Mac only: the interaction probe arms its commit observer and display link on macOS alone (Sources/App/InteractionProbe.swift arm, #if os(macOS)), so the phone serves no mac-main.interaction measure and has no measure id of its own; phone round 6 could not judge it"),
    ("J2.1/skeleton-rows-on-the-same-frame", "substituted", "skeleton-on-the-same-frame", '"loading"', '"loading"', "the Mac reads (loading, launched); the phone reads (loading, state searching) while its own hold keeps the answer back"),
    ("J2.1/skeleton-rows-fill-the-pane", "substituted", "skeleton-fills-the-pane", "LIST_ROW_PX", 'const("LIST_ROW_PX")', "both measure the skeleton's frame against the list's and LIST_ROW_PX; the phone's ids are iphone-search.skeleton and iphone-search.list"),
    ("J2.1/skeleton-gives-way-to-results", "substituted", "results-while-typing", '"loading"', '"results"', "the Mac reads the skeleton's loading flag; the phone reads the state 'results' once its rows have landed"),
    ("J2.1/record1-in-first-screen", "same", "row-names-its-account", 'EXP["record1_thread"]', 'EXP["record1_thread"]', "record 1's thread is in the results"),
    ("J2.1/meaning-match-without-shared-words", "same", "meaning-and-words-found", 'EXP["meaning_query_semantic_only_thread"]', 'EXP["meaning_query_semantic_only_thread"]', "the meaning-only thread is found"),
    ("J2.1/grouped-by-thread", "same", "grouped-by-thread", 'EXP["thread_with_several_matches"]', 'EXP["thread_with_several_matches"]', "each thread is listed once, record 1's thread among them"),
    ("J2.1/results-divider-follows-the-stage", "substituted", "sections-header-footer", "divider_follows_stage", 'EXP["result_sections"]', "the Mac judges one divider by stage; the phone compares its section titles with the fixture's"),
    ("J2.1/results-header-counts-exact-and-close", "substituted", "sections-header-footer", "header_counts", "count_line", "the Mac header reads 'N matches, M close in meaning'; the phone header carries one 'N conversations' line"),
    ("J2.1/results-footer-line", "same", "sections-header-footer", 'EXP["results_footer"]', 'EXP["results_footer"]', "the footer line is the fixture's sentence"),
    ("J2.1/matched-words-tinted", "same", "matched-words-marked", 'WORDS_TINT', 'WORDS_TINT', "the tinted words per thread, on the words query (item 3bb3d817)"),
    ("J2.1/meaning-only-row-tints-nothing", "same", "meaning-only-row-tints-nothing", "[[]]", "[[]]", "the meaning-only thread's row marks no word"),
    ("J2.1/paperclip-only-where-the-match-is-in-the-attachment", "same", "paperclip-only-where-the-match-is-in-the-attachment", 'EXP["record1_attachment"]', 'EXP["record1_attachment"]', "the row names its attachment only where the match is in it"),
    ("J2.1/account-colour-per-result", "substituted", "row-names-its-account", "colour", "account_name", "the Mac reads the account colour; the phone reads the account name each row carries"),
    ("J2.1/history-written", "substituted", "recent-searches-listed", "search/recent", 'rec[0]["text"] == QUERY', "the Mac reads the core's recent list; the phone reads its own recent rows"),
    ("J2.2/typed-from-makes-no-pill", "substituted", "typed-from-makes-no-pill", "kinds(s)", 'EXP["people_query"]', "the Mac reads the pills of its bar, the phone the chips that are on; both are none after the words are typed"),
    ("J2.2/people-rows-list-both-addresses", "substituted", "picker-opens", "people_addresses", "filter.menu", "the Mac lists the People rows in its dropdown; the phone lists the senders in its sender picker"),
    ("J2.2/dropdown-groups-people-filters-recent-in-order", "substituted", "filters-listed", '"recent"', '["account", "sender", "date", "attachment"]', "the Mac orders its dropdown groups; the phone orders its four filter chips"),
    ("J2.2/people-rows-show-name-and-address", "substituted", "picker-opens", "people_chip_value", "SENDER", "the Mac row shows name and address; the phone's picker line is checked for the name only"),
    ("J2.2/tab-adds-the-person-as-a-chip", "substituted", "chip-shows-its-value", '"from"', '"From " + SENDER', "the Mac adds the chip with Tab; the phone adds it by tapping the picker line"),
    ("J2.2/list-narrows-on-the-chip-without-enter", "substituted", "sender-narrows", "res(s)", "len(got) < len(base)", "both narrow the list at once, no Return"),
    ("J2.2/filter-chips", "substituted", "filters-combine", 'EXP["filter_chip_kinds"]', 'EXP["filter_result_threads_with_meaning_words"]', "the Mac names the chip kinds; the phone checks the threads left by the three chips together"),
    ("J2.2/list-narrows", "substituted", "date-narrows", "BY_ADDR_NARROWED", "Before June", "both narrow on the date chip"),
    ("J2.2/chip-removable", "substituted", "clear-restores", "remove_id", "filter.sender.clear", "both take the chip off by its own control"),
    ("J2.2/chip-removal-widens", "substituted", "clear-restores", "BY_ADDR_NO_BEFORE", "base", "both bring the list back after the chip goes"),
    ("J2.3/bar-shows-only-the-exact-query", "same", "field-shows-only-the-exact-query", 'EXP["exact_query"]', 'EXP["exact_query"]', "the field holds only the exact query"),
    ("J2.3/exact-message-ranks-first", "same", "exact-figure-first", 'EXP["exact_first_message"]', 'EXP["exact_first_message"]', "the quoted figure ranks its message first"),
    ("J2.3/exact-message-is-listed-before-the-divider", "substituted", "exact-figure-first", "matched_in", "got[0]", "the Mac reads where the first row sits; the phone reads that the exact message is the first row"),
    ("J2.3/figure-highlighted", "substituted", "figure-highlighted", "0.84 EUR", '"0.84"', "the Mac reads the figure as one highlight; the phone marks its words (0.84, EUR) one by one"),
    ("J2.3/only-the-exact-figure-highlighted", "substituted", "exact-figure-first", 'EXP["exact_matching_messages"]', 'EXP["exact_must_not_match"]', "the Mac lists the messages that carry the figure; the phone lists that none of the near figures is a result"),
    ("J2.8/bar-holds-exactly-the-query", "same", "photo-query-in-the-field", 'EXP["photo_query"]', 'EXP["photo_query"]', "the field holds exactly the photo query"),
    ("J2.8/photo-message-ranks-first", "same", "photo-message-ranks-first", 'EXP["photo_message"]', 'EXP["photo_message"]', "the message with the photo is first"),
    ("J2.8/photo-attachment-named-in-the-row", "same", "photo-attachment-named-in-the-row", 'EXP["photo_attachment"]', 'EXP["photo_attachment"]', "the row names the photo attachment, found in the attachment"),
    ("J2.8/photo-found-by-what-it-shows", "same", "photo-found-by-what-it-shows", "attachment-content", "attachment-content", "found by what the photo shows"),
    ("J2.8/not-downloaded-row-says-so", "same", "not-downloaded-row-says-so", 'EXP["not_downloaded_note"]', 'EXP["not_downloaded_note"]', "the row says the file is not downloaded"),
    ("J2.8/unreadable-kind-searched-by-name-and-says-so", "same", "unreadable-kind-searched-by-name-and-says-so", "attachment-name", "attachment-name", "found by name only and says so"),
    ("J2.4/one-interaction-budget-ms", "excluded", None, "INTERACTION_COMMIT_MS", None, "Mac only: the interaction probe arms its commit observer and display link on macOS alone (Sources/App/InteractionProbe.swift arm, #if os(macOS)), so the phone serves no mac-main.interaction measure and has no measure id of its own; phone round 6 could not judge it"),
    ("J2.4/thread-subject", "substituted", "opens-a-result", 'EXP["record1_message"]', 'EXP["record1_thread"]', "the Mac reads the reader's subject row; the phone reads that its thread overlay opened the tapped result"),
    ("J2.4/marked-read", "same", "open-marks-read", "/read", "/read", "the core says the opened message is read"),
    ("J2.4/attachment-shown", "substituted", "open-shows-the-attachment", "reader.attachments", "iphone-thread.attachments", "the Mac reads its reader's attachment rows, the phone its thread screen's"),
    ("J2.4/file-on-desktop", "excluded", None, "desktop/files", None, "dragging an attachment out to the desktop is a Mac gesture; the phone has no desktop"),
    ("J2.5/folder-in-sidebar", "substituted", "save-keeps-the-search", "smart-folders", 'sv["saved"]', "the Mac lists the folder in its sidebar; the phone has no sidebar, its save card says the saved name"),
    ("J2.5/folder-lists-same-threads", "same", "save-keeps-the-search", 'EXP["smart_folder_threads_after_save"]', 'EXP["smart_folder_threads_after_save"]', "the saved folder lists the same threads"),
    ("J2.5/smart-folder-in-core", "substituted", "save-keeps-the-search", '"rule"', "/rule", "both read the saved folder's rule in the core; the Mac through its helper, the phone at smart_folder/<id>/rule"),
    ("J2.5/duplicate-name-refused", "substituted", "save-refuses-a-taken-name", "exists already", '"exist"', "a second folder of the same name is refused in words; the phone matches the stem 'exist', the Mac the sentence 'exists already'"),
]
# the journey's check calls: the harness's own (`s.expect`, `s.expect_le`, `s._record`) and J2.py's helpers that record a check (`expect_ctx`, `same`, `lt`,
# and `le`, the at-most budget that prints the phases since hid-search round 16: phone round 9 read J2.4/one-interaction-budget-ms as gone)
_CALL = re.compile(r'(?:\bs\.expect_le\(|\bs\.expect\(|\bs\._record\(|\bexpect_ctx\(s, |\bsame\(s, |\blt\(s, |\ble\(s, )"([a-z0-9][a-z0-9-]*)"')


def mac_steps(text):
    """{'J2.1/name': the text of that call up to the next call} for the steps of MAC_STEPS."""
    out = {}
    for step in MAC_STEPS:
        i = text.index('j.step("%s"' % step)
        j = text.find('j.step("', i + 1)
        k = text.find("# MUTANTS", i)
        end = min(x for x in (j, k, len(text)) if x > i)
        sec = text[i:end]
        hits = list(_CALL.finditer(sec))
        for n, m in enumerate(hits):
            out["%s/%s" % (step, m.group(1))] = sec[m.start(): hits[n + 1].start() if n + 1 < len(hits) else len(sec)]
    return out


def phone_case_text(name):
    """The code that judges one phone case: from the add before it to the add after it."""
    src = inspect.getsource(cases)
    parts = re.split(r"(?m)^    (?=add\()", src)
    for n, part in enumerate(parts):
        if part.startswith('add("%s"' % name):
            return (parts[n - 1] if n else "") + part + (parts[n + 1] if n + 1 < len(parts) else "")
    return None


def judge_steps(text):
    """[(row name, ok, why)]: one row per Mac step call of the journey and one per table row without one."""
    mac = mac_steps(text)
    table = {r[0]: r for r in STEP_ROWS}
    rows = []
    for key, seg in mac.items():
        r = table.get(key)
        if r is None:
            rows.append(("step %s" % key, False, "the Mac journey step has no phone case and no named exclusion"))
            continue
        _k, kind, case, mac_lit, phone_lit, why = r
        if mac_lit not in seg:
            rows.append(("step %s" % key, False, "the Mac step no longer holds %r" % mac_lit))
        elif len(why) < 25:
            rows.append(("step %s" % key, False, "a row needs its reason"))
        elif kind in ("same", "substituted") or case:
            code = phone_case_text(case) if case else None
            if code is None:
                rows.append(("step %s" % key, False, "phone case %s is not in cases()" % case))
            elif phone_lit not in code:
                rows.append(("step %s" % key, False, "phone case %s no longer holds %r" % (case, phone_lit)))
            elif kind == "same" and mac_lit != phone_lit:
                rows.append(("step %s" % key, False, "kind same needs the same literal on both sides: %r and %r" % (mac_lit, phone_lit)))
            else:
                rows.append(("step %s" % key, True, "%s -> %s: %s" % (kind, case, why)))
        else:
            rows.append(("step %s" % key, True, "%s, no phone case: %s" % (kind, why)))
    # a name the journey still declares in its MUTANTS block (one line per check it records) is not gone: the call that records it is one this table does
    # not read yet, said so, so the fix is the call pattern above and never the table row
    declared = set("%s/%s" % m for m in re.findall(r"^# (J2\.\d+)/([a-z0-9][a-z0-9-]*) \|", text, re.M))
    for key in table:
        if key not in mac:
            if key in declared:
                rows.append(("step %s" % key, False, "the Mac journey still declares this step, but records it through a call this table does not read (_CALL)"))
            else:
                rows.append(("step %s" % key, False, "the table row names a Mac step that is not in journeys/J2.py any more"))
    return rows


def journey_text(mutant=None):
    text = open(MAC_JOURNEY, encoding="utf-8").read()
    if mutant == "journey-gains-a-step":
        i = text.index('s = j.step("J2.3"')
        j = text.index("\n", i)
        text = text[: j + 1] + '    s.expect("exact-new-step", lambda: 1, 1)\n' + text[j + 1:]
    elif mutant == "journey-drops-a-step":   # the step's call and its mutant line both gone: a real mismatch with the table
        text = re.sub(r'(?m)^    le\(s, "one-interaction-budget-ms".*\n', "", text, count=1)
        text = re.sub(r"(?m)^# J2\.4/one-interaction-budget-ms \|.*\n", "", text, count=1)
    elif mutant == "journey-hides-a-step":   # recorded through a helper the table does not read: named as such, not as gone
        text = re.sub(r'(?m)^    le\(s, "one-interaction-budget-ms"', '    budget(s, "one-interaction-budget-ms"', text, count=1)
    elif mutant == "journey-moves-a-value":
        assert text.count('EXP["exact_first_message"]') > 1
        text = text.replace('"exact-message-ranks-first", lambda: until(lambda: res(s) and res(s)[0]["id"] == EXP["exact_first_message"]) and res(s)[0]["id"], EXP["exact_first_message"]',
                            '"exact-message-ranks-first", lambda: until(lambda: res(s) and res(s)[0]["id"] == EXP["exact_other"]) and res(s)[0]["id"], EXP["exact_other"]', 1)
    return text


JOURNEY_MUTANTS = {"journey-gains-a-step": "step J2.3/exact-new-step", "journey-moves-a-value": "step J2.3/exact-message-ranks-first",
                   "journey-drops-a-step": "step J2.4/one-interaction-budget-ms", "journey-hides-a-step": "step J2.4/one-interaction-budget-ms"}


# ---------------------------------------------------------------- the live cases, one driver interface (the socket or the stand-in)
BY_ID = {m["id"]: m for m in MSGS}
THREAD_OF = {m["id"]: m["thread"] for m in MSGS}


def norm(ax):
    """The control a tapped id stands for: any result, recent or sender index is the first of its kind."""
    return re.sub(r"^iphone-search\.", "", re.sub(r"^((?:iphone-search\.)?(?:results|recent|filter\.sender|domain))\.\d+$", r"\1.0", ax))


def until(fn, secs=3.0):
    end = time.time() + secs
    while time.time() < end:
        v = fn()
        if v:
            return v
        time.sleep(0.05)
    return fn()


HEAD_OF = {}
for _m in MSGS:
    if _m["thread"] not in HEAD_OF or _m["date"] > HEAD_OF[_m["thread"]]["date"]:
        HEAD_OF[_m["thread"]] = _m          # the mail a conversation's row stands for: its newest
PASSAGE_WORDS = ["iban", "warehouse", "estimator"]   # a word held by ONE mail of the corpus, in its body, in an attachment's name, in an earlier mail of a thread
LETTERS = SENDER.split()[0].lower()                  # the first letters of the sender the people chip names
DOMAIN = EXP["sender_address"].split("@")[1]         # the domain those letters start


def holder(word):
    """(the one mail whose body or attachment name holds the word, its conversation's head mail), read from the corpus."""
    hits = [m for m in MSGS if word in m["body"].lower() or any(word in a.lower() for a in m["attachments"])]
    assert len(hits) == 1, "%r must be held by one mail, got %r" % (word, [m["id"] for m in hits])
    return hits[0], HEAD_OF[hits[0]["thread"]]


def cases(d):
    """Each case: (name, ok, why). A driver exception is not judged (the caller counts it)."""
    out = []

    def add(name, ok, why):
        out.append((name, None if str(why).startswith("NOT JUDGED") else bool(ok), why))

    def reading(ax, key):
        """(value, None) or (None, the refusal): a measure the app cannot answer makes the case NOT JUDGED, never a pass or a fail."""
        try:
            return d.measure(ax)[key], None
        except RuntimeError as e:
            return None, str(e)

    S = "iphone-search."
    state = lambda: d.row(S + "state")
    res = lambda: d.rows(S + "results")
    threads = lambda: [r["thread"] for r in res()]
    chip = lambda kind: [c for c in d.rows(S + "filters") if c["kind"] == kind][0]
    d.corpus(CORPUS)
    d.tap(S + "open")
    d.settle()
    add("opens-on-its-surface", d.uistate().get("surface") == "iphone-search", "the open control lands on iphone-search, got %r" % d.uistate().get("surface"))
    add("idle-before-typing", state()["state"] == "idle" and res() == [] and state()["text"] == "", "an empty field lists no result: %r" % state()["state"])

    # J2.1: the first answer. The screen's own hold keeps it unlisted, so the skeleton and its frame can be read (the Mac's `mac-search.skeleton.hold`)
    d.tap(S + "skeleton.hold")
    d.type(TYPED_FIRST)
    # first-screen-ms and open-commits-within-the-budget are Mac only (STEP_ROWS J2.1/first-screen-ms, J2.4/one-interaction-budget-ms say why)
    sk = d.rows(S + "skeleton")
    add("skeleton-on-the-same-frame", len(sk) == 1 and (sk[0]["loading"], state()["state"]) == (True, "searching") and res() == [],
        "held: the skeleton is loading and no row is listed: %r, state %r" % (sk, state()["state"]))
    frame, why_f = reading(S + "skeleton", "frame")
    lst, why_l = reading(S + "list", "frame")
    add("skeleton-fills-the-pane", frame is not None and lst is not None and frame["h"] >= const("LIST_ROW_PX") and frame["y"] >= lst["y"] and frame["y"] + frame["h"] <= lst["y"] + lst["h"],
        "NOT JUDGED: %s" % (why_f or why_l) if (why_f or why_l) else "the skeleton's frame %r is at least LIST_ROW_PX (%.0f) high and inside the list %r" % (frame, const("LIST_ROW_PX"), lst))
    d.tap(S + "skeleton.release")
    d.tap(S + "field.clear")

    # J2.1: results appear as the words are typed, with no Return
    d.type(TYPED_FIRST)
    first = until(res)
    add("results-while-typing", len(first) > 0 and state()["state"] == "results", "%d rows after typing %r, state %r, no Return pressed" % (len(first), TYPED_FIRST, state()["state"]))
    d.type(" spring")
    rows = until(res)
    th = [r["thread"] for r in rows]
    add("meaning-and-words-found", all(t in th for t in EXP["meaning_query_must_include_threads"] + [EXP["meaning_query_semantic_only_thread"]]),
        "record 1, the spring offer and the meaning-only thread are in %r" % th)
    add("meaning-only-row-tints-nothing", [r["highlights"] for r in rows if r["thread"] == EXP["meaning_query_semantic_only_thread"]] == [[]],
        "the meaning-only thread %s has no marked word: %r" % (EXP["meaning_query_semantic_only_thread"], [r["highlights"] for r in rows if r["thread"] == EXP["meaning_query_semantic_only_thread"]]))
    rel = d.rows(S + "divider")
    add("related-line-above-related-mail", (len(rel) == 1 and rel[0]["title"] == "Related" and rel[0]["before"] == len([r for r in rows if r.get("part") == "match"])) if any(r.get("part") == "related" for r in rows) else rel == [],
        "the thin line 'Related' stands after the %d matches when related mail is listed, and is absent when none is: %r" % (len([r for r in rows if r.get("part") == "match"]), rel))
    secs = [r["title"] for r in d.rows(S + "sections")]
    head = d.rows(S + "header")
    foot = d.rows(S + "footer")
    n_match, n_rel = len([r for r in rows if r.get("part") == "match"]), len([r for r in rows if r.get("part") == "related"])
    upper = n_match + n_rel
    add("sections-header-footer", secs == EXP["result_sections"][:len(secs)] and (len(secs) == 2) == (upper > 3) and head and head[0]["count"] == count_line(n_match, n_rel)
        and foot and foot[0]["text"] == EXP["results_footer"], "sections %r, header %r (want %r), footer %r" % (secs, head, count_line(n_match, n_rel), foot))
    # SPEC A150 on the phone: the list is the matches, then the related mail; the rest waits behind the closed line "All other mail", which opens and closes by a tap
    more = d.rows(S + "more")
    rest = head[0].get("rest", 0) if head else 0
    parts_ok = (head and head[0].get("matches") == n_match and head[0].get("related") == n_rel and len(rows) == upper and [r.get("part") for r in rows] == ["match"] * n_match + ["related"] * n_rel
                and bool(more) == (rest > 0) and (not more or (more[0]["count"] == rest and more[0]["open"] is False and more[0]["listed"] == 0)))
    opened = shut = True
    if more and parts_ok:
        d.tap(S + "more")
        opened = d.rows(S + "more")[0]["open"] is True and len(d.rows(S + "results")) == upper + rest and d.rows(S + "header")[0]["count"] == count_line(n_match, n_rel)
        d.tap(S + "more")
        shut = d.rows(S + "more")[0]["open"] is False and len(d.rows(S + "results")) == upper
    add("matches-related-and-the-closed-line", bool(parts_ok and opened and shut),
        "matches %d, related %d, rest %d behind the closed line (open adds them, a second tap closes): listed %d rows, closed line %r; parts in order %r, opened %r, closed again %r, parts %r"
        % (n_match, n_rel, rest, len(rows), more, bool(parts_ok), opened, shut, [r.get("part") for r in rows]))
    add("row-names-its-account", all(r["account_name"] and r["sender"] and r["subject"] for r in rows) and
        [r["account"] for r in rows if r["thread"] == EXP["record1_thread"]] == [EXP["record1_account"]], "each row names sender, subject and account; record 1 on %s" % EXP["record1_account"])
    # item 3bb3d817: the two cases that read the matches, on the words query (record 1 and msg-s05 hold every word); then QUERY again for the steps after
    d.tap(S + "field.clear")
    d.type(WORDS_QUERY)
    wrows = until(lambda: [r for r in res() if r.get("part") == "match"] and res())
    marks = {r["thread"]: [w.lower() for w in r["highlights"]] for r in wrows}
    add("matched-words-marked", all(w in marks.get(t, []) for t, ws in WORDS_TINT.items() for w in ws), "marked words per thread: %r" % marks)
    add("paperclip-only-where-the-match-is-in-the-attachment", {r["thread"]: r["attachment"] for r in wrows if r["thread"] in (EXP["record1_thread"], "thr-s05")} ==
        {EXP["record1_thread"]: EXP["record1_attachment"], "thr-s05": None}, "the row names an attachment only where the match is in it: %r" % {r["thread"]: r["attachment"] for r in wrows if r["thread"] in (EXP["record1_thread"], "thr-s05")})
    d.tap(S + "field.clear")
    d.type(QUERY)
    until(res)
    d.tap(S + "field.submit")                                   # QUERY is the newest recent search (J2.1/history-written below)
    d.tap(S + "field.clear")
    d.type(FILTER_QUERY)                                        # item 3bb3d817: the single filters narrow this answer (FILTER_QUERY above)
    th = [r["thread"] for r in until(res)]
    add("grouped-by-thread", len(th) == len(set(th)) and EXP["thread_with_several_matches"] in th, "one row per thread, record 1's thread once: %r" % th)
    base = sorted(threads())

    # J2.2: filters narrow, each shows as a chip, each can be taken off
    add("filters-listed", [c["kind"] for c in d.rows(S + "filters")] == ["account", "sender", "date", "attachment"], "four chips under the field")
    d.tap(S + "filter.sender")
    menu = d.rows(S + "filter.menu")
    add("picker-opens", state()["menu"] == "sender" and SENDER in [o["title"] for o in menu], "the sender picker lists %r" % [o["title"] for o in menu])
    pick = [o for o in menu if o["title"] == SENDER]
    if pick:
        d.tap(S + "filter.sender.%s" % pick[0]["id"])
    got = res()
    add("sender-narrows", got and all(SENDER.lower() in r["sender"].lower() for r in got) and len(got) < len(base), "%d of %d rows, all from %s" % (len(got), len(base), SENDER))
    add("chip-shows-its-value", chip("sender")["on"] is True and chip("sender")["label"] == "From " + SENDER and state()["menu"] == "", "chip %r" % chip("sender"))
    d.tap(S + "filter.sender.clear")
    add("clear-restores", sorted(threads()) == base and chip("sender")["on"] is False, "taking the sender off brings back the %d threads" % len(base))
    d.tap(S + "filter.account")
    d.tap(S + "filter.account.%s" % ACCOUNT)
    got = res()
    add("account-narrows", got and all(r["account"] == ACCOUNT for r in got) and len(got) < len(base), "%d of %d rows, all on %s" % (len(got), len(base), ACCOUNT))
    d.tap(S + "filter.account.clear")
    d.tap(S + "filter.account")
    d.tap(S + "filter.account.all")
    add("account-all-restores", sorted(threads()) == base, "All accounts brings back the %d threads" % len(base))
    d.tap(S + "filter.date")
    d.tap(S + "filter.date.june")
    got = res()
    add("date-narrows", got and len(got) < len(base) and chip("date")["label"] == "Before June", "%d of %d rows before June" % (len(got), len(base)))
    d.tap(S + "filter.date.clear")
    d.tap(S + "filter.attachment")
    got = res()
    # the switch keeps exactly the threads that hold a mail with an attachment (main97: every thread of this query holds one, so "fewer than before" could not hold)
    with_att = sorted(t for t in base if any(m["attachments"] for m in MSGS if m["thread"] == t))
    add("attachment-narrows", got and sorted(threads()) == with_att and chip("attachment")["on"] is True and (len(got) < len(base) or with_att == base),
        "%d of %d rows with an attachment; the threads holding one: %r" % (len(got), len(base), with_att))
    d.tap(S + "filter.attachment.clear")
    d.tap(S + "filter.date")
    d.tap(S + "filter.menu.close")
    add("picker-closes-without-a-pick", state()["menu"] == "" and chip("date")["on"] is False and sorted(threads()) == base, "the picker closes, no filter was set")
    d.tap(S + "field.clear")
    d.type(QUERY)                                               # the three filters together and the save run on QUERY, as before item 3bb3d817
    until(res)
    d.tap(S + "filter.sender")
    o = [o for o in d.rows(S + "filter.menu") if o["title"] == SENDER]
    if o:
        d.tap(S + "filter.sender.%s" % o[0]["id"])
    d.tap(S + "filter.date")
    d.tap(S + "filter.date.june")
    d.tap(S + "filter.attachment")
    add("filters-combine", sorted(threads()) == EXP["filter_result_threads_with_meaning_words"], "sender, June and attachment together leave %r, expected %r" % (threads(), EXP["filter_result_threads_with_meaning_words"]))

    # J2.5: keep the search as a smart folder
    rule = d.row(S + "save")["rule"]
    before = d.core("smart_folder/count") or 0
    d.tap(S + "save")
    add("save-opens-the-name", d.row(S + "save")["open"] is True, "Save opens the name card")
    d.tap(S + "save.confirm")
    add("save-asks-for-a-name", d.row(S + "save")["open"] is True and d.row(S + "save")["says"] != "", "an empty name stays open and says why: %r" % d.row(S + "save")["says"])
    d.type(EXP["smart_folder_name"])
    add("name-goes-to-the-name", d.row(S + "save")["text"] == EXP["smart_folder_name"] and state()["text"] == QUERY, "typed text lands in the name, the search keeps %r" % state()["text"])
    d.tap(S + "save.confirm")
    after = d.core("smart_folder/count") or 0
    fid = "sf-%d" % after
    sv = d.row(S + "save")
    add("save-keeps-the-search", after == before + 1 and sv["open"] is False and sv["saved"] == EXP["smart_folder_name"] and d.core("smart_folder/%s/rule" % fid) == rule
        and sorted({THREAD_OF.get(i, i) for i in (d.core("smart_folder/%s/threads" % fid) or [])}) == EXP["smart_folder_threads_after_save"],
        "folder %s holds the rule %r and lists %r" % (fid, rule, d.core("smart_folder/%s/threads" % fid)))
    d.tap(S + "save")
    d.type(EXP["smart_folder_name"])
    d.tap(S + "save.confirm")
    sv = d.row(S + "save")
    add("save-refuses-a-taken-name", (d.core("smart_folder/count") or 0) == after and sv["open"] is True and "exist" in sv["says"].lower(), "a second folder of that name is refused in words: %r" % sv["says"])
    d.tap(S + "save.cancel")
    add("cancel-closes-the-name", d.row(S + "save")["open"] is False and (d.core("smart_folder/count") or 0) == after, "Cancel closes the card and saves nothing")

    # J2.4: a result opens its thread
    for k in ("sender", "date", "attachment"):
        d.tap(S + "filter.%s.clear" % k)
    rows = until(res)
    i = [n for n, r in enumerate(rows) if r["thread"] == EXP["record1_thread"]]
    if i:
        d.tap(S + "results.%d" % i[0])
    st, ui = state(), d.uistate()
    add("opens-a-result", bool(i) and ui.get("overlay") == "iphone-thread" and st["opened"] == rows[i[0]]["id"] if i else False,
        "the result opens its thread: overlay %r, opened %r" % (ui.get("overlay"), st["opened"]))
    add("open-marks-read", d.core("message/%s/read" % EXP["record1_message"]) is True and st["opened"] == EXP["record1_message"], "message/%s/read is true and the state names it: %r" % (EXP["record1_message"], st["opened"]))
    shown = [r["name"] for r in d.rows("iphone-thread.attachments")]
    add("open-shows-the-attachment", shown == [EXP["record1_attachment"]], "the opened thread lists %r (iphone-thread.attachments)" % shown)
    # Back from the thread, as a person does: the search's own controls are under the thread until then (phone round 7: the close tapped
    # through the thread left it on top, surface 'iphone-thread'). J2.4 opens the thread from the search; nothing in the journeys closes both at once.
    d.tap("iphone-thread.back")
    add("text-kept-after-open", st["text"] == QUERY and len(res()) == len(rows), "closing the thread lands on the same search: %r, %d rows" % (st["text"], len(res())))

    # recent searches and Return
    d.tap(S + "field.clear")
    add("clear-empties-the-field", state()["text"] == "" and state()["state"] == "idle" and res() == [], "the clear control empties the field and the rows go")
    rec = d.rows(S + "recent")
    add("recent-searches-listed", rec and rec[0]["text"] == QUERY, "the search just opened is the newest recent: %r" % rec[:1])
    d.tap(S + "recent.0")
    add("recent-searches-fill-the-field", state()["text"] == QUERY and len(until(res)) > 0, "tapping a recent search fills the field and runs it")
    d.tap(S + "field.clear")
    d.type(EXP["exact_query"])
    got = until(res)
    add("exact-figure-first", got and got[0]["id"] == EXP["exact_first_message"] and [r["id"] for r in got if r["id"] in EXP["exact_must_not_match"]] == [],
        "the quoted figure ranks %s first: %r" % (EXP["exact_first_message"], [r["id"] for r in got]))
    add("field-shows-only-the-exact-query", d.row(S + "field")["text"] == EXP["exact_query"], "after the field was cleared the exact query is all it holds: %r" % d.row(S + "field")["text"])
    add("figure-highlighted", bool(got) and {"0.84", "eur"} <= {h.lower() for h in got[0]["highlights"]}, "the figure's words are marked in the first row: %r" % (got[0]["highlights"] if got else None))
    d.tap(S + "field.submit")
    add("submit-keeps-a-recent", (d.rows(S + "recent") or [{}])[0].get("text") == EXP["exact_query"], "Return keeps the typed words as the newest recent")

    # SPEC A149, A152, A153 on the phone: the passage line and where it is from, the typed letters in bold, the sender-domain filter
    d.tap(S + "field.clear")
    d.type(EXP["people_query"])
    add("typed-from-makes-no-pill", [c["kind"] for c in d.rows(S + "filters") if c["on"]] == [] and state()["text"] == EXP["people_query"],
        "typing %r makes no pill, the words stay words: chips on %r" % (EXP["people_query"], [c["kind"] for c in d.rows(S + "filters") if c["on"]]))
    seen = {}
    for word in PASSAGE_WORDS:
        d.tap(S + "field.clear")
        d.type(word)
        hit, head = holder(word)
        got = until(lambda: [x for x in res() if x["thread"] == head["thread"]])
        seen[word] = (got[0] if got else None, hit, head)

    def source_ok(word):
        r, hit, head = seen[word]
        want = "attachment" if any(word in a.lower() for a in hit["attachments"]) else "text" if hit["id"] == head["id"] else "thread"
        lead = {"attachment": [a for a in hit["attachments"] if word in a.lower()][:1] or [""], "thread": [hit["from_name"]], "text": [""]}[want][0]
        return r is not None and r["found"] == want and r["hit_message"] == hit["id"] and r["found_line"].startswith(lead) and (want != "text" or word in r["found_line"].lower())
    add("passage-line-names-its-source", all(source_ok(w) for w in PASSAGE_WORDS),
        "each result shows its passage and where it is from: %r" % {w: (seen[w][0] and (seen[w][0]["found"], seen[w][0]["hit_message"], seen[w][0]["found_line"][:30])) for w in PASSAGE_WORDS})
    shown = until(res)
    j = [n for n, x in enumerate(shown) if x["thread"] == seen[PASSAGE_WORDS[-1]][2]["thread"]]
    if j:
        d.tap(S + "results.%d" % j[0])
    add("opens-the-mail-with-the-passage", bool(j) and state()["opened_hit"] == seen[PASSAGE_WORDS[-1]][1]["id"] != state()["opened"],
        "the result opens on its row %r and names the mail that holds the passage: %r" % (state()["opened"], state()["opened_hit"]))
    d.tap("iphone-thread.back")
    d.tap(S + "field.clear")
    d.type(LETTERS)
    got = until(res)
    want = [r for r in got if r.get("part") == "match" and LETTERS in (BY_ID[r["id"]]["from_name"] + " " + BY_ID[r["id"]]["subject"]).lower()]
    add("typed-letters-are-bold-on-a-match", bool(want) and all(LETTERS in [b.lower() for b in r["bold"]] for r in want) and all(r["bold"] == [] for r in got if r.get("part") == "related"),
        "%d matches hold %r and show it in bold, a related mail shows none: %r" % (len(want), LETTERS, [r["bold"] for r in got][:3]))
    offers = until(lambda: d.rows(S + "domain"))
    people = len({m["from"].lower() for m in MSGS if m["from"].lower().endswith("@" + DOMAIN)})
    add("domain-offered-with-its-people", len(offers) >= 1 and offers[0]["domain"] == DOMAIN and offers[0]["title"] == "From domain" and offers[0]["people"] == ("1 person" if people == 1 else "%d people" % people),
        "typing %r offers 'From domain %s' with %d people: %r" % (LETTERS, DOMAIN, people, offers[:1]))
    d.tap(S + "domain.0")
    pill = [c for c in d.rows(S + "filters") if c["kind"] == "domain"]
    got = until(res)
    add("domain-pill-keeps-only-that-domain", len(pill) == 1 and pill[0]["label"] == "From @" + DOMAIN and state()["text"] == "" and bool(got) and all(BY_ID[r["id"]]["from"].lower().endswith("@" + DOMAIN) for r in got),
        "the pill reads %r, the field is empty and every row is from %s: %r" % (pill[:1], DOMAIN, [r["id"] for r in got]))
    d.tap(S + "filter.domain.clear")
    add("domain-pill-comes-off", [c for c in d.rows(S + "filters") if c["kind"] == "domain"] == [] and state()["state"] == "idle", "taking the pill off leaves no pill and the search at rest")
    # J2.8: the words inside an attachment
    d.tap(S + "field.clear")
    d.type(EXP["photo_query"])
    got = until(lambda: res() and res()[0]["id"] == EXP["photo_message"] and res())
    add("photo-query-in-the-field", d.row(S + "field")["text"] == EXP["photo_query"], "the field holds exactly %r" % EXP["photo_query"])
    add("photo-message-ranks-first", bool(got) and got[0]["id"] == EXP["photo_message"], "the message with the photo is first: %r" % [r["id"] for r in (got or res())][:3])
    r = (got or [{}])[0]
    add("photo-attachment-named-in-the-row", (r.get("attachment"), str(r.get("matched_in")).startswith("attachment")) == (EXP["photo_attachment"], True), "the row names %r, found in %r" % (r.get("attachment"), r.get("matched_in")))
    d.tap(S + "field.clear")
    d.type(EXP["photo_content_query"])
    got = until(lambda: res() and res()[0]["id"] == EXP["photo_message"] and res())
    add("photo-found-by-what-it-shows", bool(got) and (got[0]["id"], got[0]["matched_in"], got[0]["attachment"], d.row(S + "field")["text"]) == (EXP["photo_message"], "attachment-content", EXP["photo_attachment"], EXP["photo_content_query"]),
        "found by what the photo shows: %r" % [(x["id"], x["matched_in"], x["attachment"]) for x in (got or res())][:1])
    d.tap(S + "field.clear")
    d.type(EXP["not_downloaded_query"])
    r = (until(lambda: [x for x in res() if x["id"] == EXP["not_downloaded_message"]]) or [{}])[0]
    add("not-downloaded-row-says-so", (r.get("attachment"), r.get("attachment_says")) == (EXP["not_downloaded_attachment"], EXP["not_downloaded_note"]), "the row says %r" % ((r.get("attachment"), r.get("attachment_says")),))
    d.tap(S + "field.clear")
    d.type(EXP["unreadable_query"])
    r = (until(lambda: [x for x in res() if x["id"] == EXP["unreadable_message"]]) or [{}])[0]
    add("unreadable-kind-searched-by-name-and-says-so", (r.get("attachment"), EXP["unreadable_note_words"] in (r.get("attachment_says") or ""), r.get("matched_in")) == (EXP["unreadable_attachment"], True, "attachment-name"),
        "found by its name only, and says so: %r" % ((r.get("attachment"), r.get("attachment_says"), r.get("matched_in")),))

    # no result, a long query
    d.tap(S + "field.clear")
    d.type("supplier " * 40 + "x" * LONG)
    add("long-query-answers", state()["state"] in ("results", "none", "searching"), "a %d character query is answered: %r" % (LONG, state()["state"]))
    d.tap(S + "field.clear")
    d.type("zzqxv wlkrt")
    until(lambda: d.rows(S + "empty"))
    # SPEC A150: with no match and nothing related the list says "No match" and still offers the closed line over every other mail, so the screen's state
    # is "results" when other mail stands behind that line (main97), "none" only when no mail is there at all
    add("no-result-says-so", (state()["state"] == "none" or (state()["state"] == "results" and len(d.rows(S + "more")) == 1)) and res() == []
        and len(d.rows(S + "empty")) == 1 and d.rows(S + "empty")[0]["title"] == "No match",
        "nothing found: state %r, empty rows %r (the title is the Mac's 'No match')" % (state()["state"], d.rows(S + "empty")))

    # J2.8 / J5.4: the meaning model is not on the phone
    d.click("ai-setup.preview.meaning.missing")
    line = d.rows(S + "meaning")
    add("meaning-line-says-missing", len(line) == 1 and line[0]["state"] == "missing" and state()["meaning_missing"] is True, "the line shows while the model is missing: %r" % line)
    add("meaning-line-words", len(line) == 1 and line[0]["text"] == NEW_SENTENCE and "Mac" not in line[0]["text"], "the line says it plainly for the phone: %r" % (line[:1] and line[0]["text"]))
    d.tap(S + "meaning.download")
    after_door = d.rows(S + "meaning")
    add("door-pressed", len(after_door) == 1 and after_door[0]["taps"] == 1, "the door was pressed once: %r" % (after_door[:1] and after_door[0]["taps"]))
    d.click("ai-setup.preview.meaning.ready")
    add("meaning-line-gone-when-ready", d.rows(S + "meaning") == [], "no line once the model is on the phone")
    d.click("ai-setup.preview.meaning.clear")

    # every control tapped once, then close
    d.tap(S + "close")
    closed = (d.uistate().get("surface"), state()["state"], state()["text"])   # each value printed: phone rounds 5 and 6 failed with none of them shown
    add("close-returns-to-inbox", closed == ("iphone-inbox", "idle", ""), "the close control lands on the inbox with an empty field: surface %r, state %r, text %r" % closed)
    tapped = {norm(a) for a in d.tapped}
    miss = [c for c in CONTROLS if c not in tapped]
    add("every-control-tapped", not miss and not d.refused, "controls not tapped: %s; refused: %s" % (miss or "none", d.refused[:2] or "none"))
    return out


# ---------------------------------------------------------------- the stand-in: the intended screen, in Python, for --selftest
def stem(w):
    for suf in ("iers", "ier", "ies", "ing", "es", "ed", "s", "y"):
        if len(w) > len(suf) + 2 and w.endswith(suf):
            return w[:-len(suf)]
    return w


class Model(Socket):
    """The intended behaviour of the phone search over the J2 corpus. A mutant breaks exactly one thing."""

    STOP = {"the", "in", "a", "an", "of", "what", "my"}      # the small words of a question never decide a match while a content word is typed

    def __init__(self, mutant=""):
        self.m = mutant
        self.tapped, self.refused = set(), []
        self.surface, self.overlay = "iphone-inbox", None
        self.active, self.text, self.submitted = False, "", False
        self._reset_filters()
        self.menu, self.naming, self.name, self.says, self.saved = "", False, "", "", None
        self.recents, self.folders = [], []
        self.meaning, self.taps, self.opened, self.hit, self.hold = "ready", 0, None, None, False
        self.read = set()
        self.hitmsg = {}

    def _reset_filters(self):
        self.f = {"account": None, "sender": None, "date": None, "attachment": False, "domain": None}

    # ---- the search
    def _terms(self):
        low = self.text.lower()
        phrase = re.findall(r'"([^"]+)"', low)
        words = [w for w in re.findall(r"[a-z0-9]+", re.sub(r'"[^"]*"', " ", low)) if len(w) > 1]
        content = [w for w in words if w not in self.STOP]
        return (content or words), phrase

    def _att_text(self, name):
        a = AC.get(name)
        return a["text"] if a and a.get("state") == "indexed" and self.m != "attachment-content-not-read" else ""

    def _hay(self, m):
        if self.m == "attachment-not-searched":
            return m["subject"] + " " + m["body"] + " " + m["from_name"]
        return (m["subject"] + " " + m["body"] + " " + " ".join(m["attachments"]) + " " + " ".join(self._att_text(a) for a in m["attachments"]) + " " + m["from_name"])

    def _hit_in(self, m, words):
        toks = re.findall(r"[a-z0-9.,]+", self._hay(m).lower())
        return [w for w in words if any(t.startswith(stem(w)) for t in toks)]

    def _name_hit(self, m, words):
        for a in m["attachments"]:
            if self.m == "unreadable-dropped" and AC.get(a, {}).get("state") == "unreadable":
                continue
            toks = re.findall(r"[a-z0-9.,]+", a.lower())
            if any(t.startswith(stem(w)) for w in words for t in toks):
                return a
        return None

    def _content_hit(self, m, words):
        for a in m["attachments"]:
            toks = re.findall(r"[a-z0-9.,]+", self._att_text(a).lower())
            if any(t.startswith(stem(w)) for w in words for t in toks):
                return a
        return None

    def _rows(self):
        if self.text.strip() == "" and not any(self.f.values()):
            return [], []
        words, phrase = self._terms()
        hit = []
        for m in MSGS:
            ok = True
            if phrase:
                ok = all(re.search(r"(?<![a-z0-9.,/-])" + re.escape(p) + r"(?![a-z0-9])", (m["subject"] + " " + m["body"]).lower()) for p in phrase)
            elif words:
                ok = set(self._hit_in(m, words)) == set(words)   # every typed word (item 3bb3d817)
            if ok:
                hit.append(m)
        if self.text == QUERY and self.m != "meaning-missing-hits":
            keep = set(EXP["meaning_query_must_include_threads"] + [EXP["meaning_query_semantic_only_thread"]])
            hit += [next(m for m in MSGS if m["thread"] == t) for t in sorted(keep) if t not in {h["thread"] for h in hit}]
        seen, rows, self.hitmsg = set(), [], {}
        for m in hit:
            if self.m == "one-row-per-message":
                rows.append(m)
                continue
            if m["thread"] in seen:
                continue
            seen.add(m["thread"])
            head = HEAD_OF[m["thread"]]
            self.hitmsg[m["thread"]] = head["id"] if head in hit else m["id"]
            rows.append(head)
        words_only = words if not phrase else []
        def key(m):
            hm = BY_ID[self.hitmsg.get(m["thread"], m["id"])]
            hits = self._hit_in(hm, words_only) if words_only else [None]
            score = len(hits) + (2 if self._name_hit(m, words_only) else 0)
            return (0 if hits else 1, -score)
        rows = sorted(rows, key=key) if words_only else rows
        cand = [m for m in rows if not self.f["account"] or m["account"] == self.f["account"]] if self.m != "account-filter-dead" else rows
        if self.f["date"] and self.m != "date-filter-dead":
            lim = "2026-%02d-01" % (MONTHS.index(self.f["date"]) + 1)
            cand = [m for m in cand if m["date"][:10] < lim]
        if self.f["attachment"] and self.m != "attachment-filter-dead":
            cand = [m for m in cand if m["attachments"]]
        if self.f["domain"] and self.m != "domain-filter-dead":
            cand = [m for m in cand if m["from"].lower().endswith("@" + self.f["domain"])]
        out = [m for m in cand if not self.f["sender"] or self.f["sender"].lower() in m["from_name"].lower()] if self.m != "sender-filter-dead" else cand
        if self.m == "waits-for-return" and not self.submitted:
            out = []
        return cand, out

    def _idle(self):
        return self.text.strip() == "" and not any(self.f.values())

    def _state(self):
        cand, out = self._rows()
        if self.m == "idle-lists-results" and self._idle():
            return "idle", MSGS[:2]
        if self._idle():
            return "idle", []
        if self.hold and self.m != "skeleton-never-held":
            return "searching", []
        if self.m == "waits-for-return" and not self.submitted:
            return "searching", []
        if not out:
            return ("results", []) if self.m == "no-result-state-missing" else ("none", [])
        return "results", out

    def _info(self, m):
        """What one row says about why it is listed: part, where the passage is from, the bold letters, the attachment."""
        words, phrase = self._terms()
        w = words if not phrase else []
        hm = BY_ID[self.hitmsg.get(m["thread"], m["id"])]
        part = "match" if (phrase or set(self._hit_in(hm, w)) == set(w) or not w) else "related"   # every typed word (item 3bb3d817)
        name, content = self._name_hit(m, w), self._content_hit(m, w)
        att = name or content
        if self.m == "paperclip-everywhere" and m["attachments"]:
            att = m["attachments"][0]
        if self.m == "row-names-no-attachment":
            att = None
        says = None
        if name and AC.get(name, {}).get("state") == "not-downloaded":
            says = None if self.m == "not-downloaded-silent" else EXP["not_downloaded_note"]
        elif name and AC.get(name, {}).get("state") == "unreadable":
            says = "name only, this kind of file cannot be read"
        elif content and not name:
            says = "in the attachment"
        matched = "attachment-name" if name else "attachment-content" if content else "subject" if self._hit_in({**m, "body": "", "attachments": [], "from_name": ""}, w) else "body" if part == "match" else "meaning"
        found, line = "", ""
        if part == "related":
            found, line = "text", "\u201c" + hm["body"][:60] + "\u201d"
        elif w and not self._hit_in({**m, "body": "", "attachments": [], "from_name": ""}, w) and not self._hit_in({**m, "subject": "", "body": "", "attachments": []}, w):
            if att:
                found, line = "attachment", att + ((" " + self._att_text(att)) if content and not name else "")
            elif hm["id"] == m["id"]:
                found, line = "text", hm["body"][:90]
            else:
                found, line = "thread", "%s \u00b7 %s %s" % (hm["from_name"], hm["date"][:10], hm["body"][:60])
        if self.m == "found-missing":
            found, line = "", ""
        bold = []
        if part == "match" and w and self.m != "no-bold":
            for src in (m["from_name"], m["subject"]):
                bold += [mm.group(0) for x in w for mm in re.finditer(re.escape(stem(x)) + r"[A-Za-z0-9]*", src, re.I) if mm.start() == 0 or not src[mm.start() - 1].isalnum()]
        return {"part": part, "attachment": att, "attachment_says": says, "matched_in": matched, "found": found, "found_line": line, "bold": bold, "hit": hm["id"]}

    def _marks(self, m):
        if self.m == "no-marks":
            return []
        words, phrase = self._terms()
        if phrase:
            return [x for p in phrase for part in p.split() for x in re.findall(re.escape(part), m["subject"] + " " + m["body"], re.I)] if self.m != "figure-not-highlighted" else []
        toks = re.findall(r"[A-Za-z0-9]+", m["subject"] + " " + m["body"])
        got = [t for t in toks if any(t.lower().startswith(stem(w)) for w in words)]
        if self.m == "meaning-row-tinted" and not got:
            return ["print"]
        return got

    def _rule(self):
        parts = []
        if self.f["sender"]:
            parts.append('from:"%s"' % self.f["sender"])
        if self.f["domain"]:
            parts.append("from:@%s" % self.f["domain"])
        if self.f["date"]:
            parts.append("before %s" % self.f["date"].capitalize())
        if self.f["attachment"]:
            parts.append("has attachment")
        return " ".join(parts + ([self.text.strip()] if self.text.strip() else []))

    def _line_visible(self):
        if self.m == "meaning-line-never":
            return False
        return self.active and not self._idle() and (self.meaning != "ready" or self.m == "line-stays-when-ready")

    def _offers(self):
        """'From domain <domain>' for the word being typed: the domains of the corpus's senders that start with it, most mail first."""
        words = self.text.split()
        if self.m == "domain-offer-missing" or not words or self.text[-1:].isspace():
            return []
        letters = words[-1].lstrip("@").lower()
        if not letters:
            return []
        people, mails = {}, {}
        for m in MSGS:
            a = m["from"].lower()
            if m["from_name"] == "Assem Chammah" or not a.split("@")[1].startswith(letters):
                continue
            people.setdefault(a.split("@")[1], set()).add(a)
            mails[a.split("@")[1]] = mails.get(a.split("@")[1], 0) + 1
        return [(d, len(people[d])) for d in sorted(mails, key=lambda x: (-mails[x], x))[:2]]

    # ---- the socket
    def raw(self, verb, *a):
        a = [str(x) for x in a]
        if verb == "fixture":
            return {"ok": True}
        if verb == "type":
            if self.naming:
                self.name += a[0]
            else:
                self.text += a[0]
            return {"ok": True}
        if verb == "uistate":
            return {"ok": True, "state": {"surface": self.surface, "overlay": self.overlay}}
        if verb == "measure":
            if self.m == "measure-refused":
                return {"ok": False, "error": "not judged: no frame within INTERACTION_MEASURE_WAIT_S"}
            if a[0] == "iphone-search.list":
                return {"ok": True, "frame": {"x": 0, "y": 100, "w": 402, "h": 700}}
            if a[0] == "iphone-search.skeleton":
                return {"ok": True, "frame": {"x": 0, "y": 120, "w": 402, "h": 20 if self.m == "skeleton-too-small" else 480}}
            return {"ok": False, "error": "no measurement at %s" % a[0]}
        if verb == "core":
            p = a[1].split("/")
            if p == ["smart_folder", "count"]:
                return {"ok": True, "value": len(self.folders)}
            if len(p) == 3 and p[0] == "message" and p[2] == "read":
                return {"ok": True, "value": p[1] in self.read}
            f = {x["id"]: x for x in self.folders}.get(p[1] if len(p) > 1 else "")
            if f and p[2] == "rule":
                return {"ok": True, "value": f["rule"]}
            if f and p[2] == "threads":
                return {"ok": True, "value": f["threads"]}
            return {"ok": True, "value": None}
        if verb == "click":
            return self._click(a[0])
        if verb == "rows":
            return {"ok": True, "rows": self._read(a[0])}
        return {"ok": False, "error": "no verb %s" % verb}

    def _click(self, ax):
        k = ax.replace("iphone-search.", "")
        ok = {"ok": True}
        no = {"ok": False, "error": "no click target %s" % ax}
        if self.m == "control-missing" and k == "filter.attachment.clear":
            return no
        if ax.startswith("ai-setup.preview.meaning."):
            self.meaning = ax.rsplit(".", 1)[1] if ax.rsplit(".", 1)[1] != "clear" else "ready"
            return ok
        if k == "open":
            self.active, self.surface, self.text, self.overlay, self.hold = True, "iphone-search", "", None, False
            self._reset_filters()
            return ok
        if k == "close":
            if self.m != "close-stays":
                # as the app: the search closes under an open thread and the surface is the screen on top (PhoneNav.surfaceName)
                on_top = "iphone-thread" if self.overlay == "iphone-thread" else "iphone-inbox"
                self.active, self.surface, self.text, self.hold = False, on_top, "", False
                self._reset_filters()
            return ok
        if ax == "iphone-thread.back":
            if self.m != "thread-back-dead" and self.overlay == "iphone-thread":
                self.overlay, self.surface = None, "iphone-search" if self.active else "iphone-inbox"
            return ok
        if k == "skeleton.hold":
            self.hold = True
            return ok
        if k == "skeleton.release":
            self.hold = False
            return ok
        if k == "field.clear":
            if self.m != "field-clear-keeps-text":
                self.text = ""
            return ok
        if k == "field.submit":
            if self.text.strip():
                if self.m != "recent-not-kept":
                    self.recents = [self.text] + [r for r in self.recents if r != self.text]
                self.submitted = True
            return ok
        if k in ("filter.account", "filter.sender", "filter.date"):
            kind = k.split(".")[1]
            if self.m != "menu-never-opens":
                self.menu = "" if self.menu == kind else kind
            return ok
        if k == "filter.attachment":
            if self.m != "attachment-filter-dead":
                self.f["attachment"] = not self.f["attachment"]
            return ok
        if re.fullmatch(r"filter\.(account|sender|date|attachment|domain)\.clear", k):
            kind = k.split(".")[1]
            if self.m != "clear-does-not-widen" and not (kind == "domain" and self.m == "domain-pill-stays"):
                self.f[kind] = False if kind == "attachment" else None
            self.menu = ""
            return ok
        if k == "filter.menu.close":
            self.menu = ""
            return ok
        if k.startswith("filter.account."):
            self.f["account"] = None if k.endswith(".all") else k.split(".", 2)[2]
            self.menu = ""
            return ok
        if k.startswith("filter.sender."):
            senders = self._senders()
            i = int(k.rsplit(".", 1)[1])
            if i < len(senders):
                self.f["sender"] = senders[i]
            self.menu = ""
            return ok
        if k.startswith("filter.date."):
            self.f["date"] = k.rsplit(".", 1)[1]
            self.menu = ""
            return ok
        if k.startswith("domain."):
            offers = self._offers()
            i = int(k.rsplit(".", 1)[1])
            if i < len(offers):
                self.f["domain"] = offers[i][0]
                self.text = self.text[: self.text.rstrip().rfind(self.text.split()[-1])].rstrip()
            return ok
        if k.startswith("results."):
            st, out = self._state()
            i = int(k.split(".")[1])
            if i < len(out) and self.m != "open-does-nothing":
                self.overlay, self.surface, self.opened = "iphone-thread", "iphone-thread", out[i]["id"]
                self.hit = self._info(out[i])["hit"] if self.m != "opened-hit-missing" else out[i]["id"]
                if self.m != "open-does-not-mark-read":
                    self.read.add(out[i]["id"])
                self.recents = [self.text] + [r for r in self.recents if r != self.text]
                if self.m == "text-lost-on-open":
                    self.text = ""
            return ok
        if k.startswith("recent."):
            i = int(k.split(".")[1])
            if i < len(self.recents):
                self.text = self.recents[i]
            return ok
        if k == "save":
            if self._state()[0] in ("results", "none") and self._rule() and self.m != "save-dead":
                self.naming, self.name, self.says, self.saved = True, "", "", None
            return ok
        if k == "save.cancel":
            self.naming, self.name, self.says = False, "", ""
            return ok
        if k == "save.confirm":
            n = self.name.strip()
            if not n:
                self.says = "Give the folder a name"
            elif any(x["name"].lower() == n.lower() for x in self.folders) and self.m != "save-duplicate-allowed":
                self.says = "the name exists already"
            else:
                st, out = self._state()
                self.folders.append({"id": "sf-%d" % (len(self.folders) + 1), "name": n, "rule": self._rule(), "threads": [m["id"] for m in out]})
                self.naming, self.name, self.says, self.saved = False, "", "", n
            return ok
        if k == "meaning.download":
            if self._line_visible() and self.m != "door-dead":
                self.taps += 1
            return ok
        return no

    def _senders(self):
        cand, out = self._rows()
        seen = []
        for m in cand:
            if m["from_name"] not in seen:
                seen.append(m["from_name"])
        return seen

    def _read(self, ax):
        S = "iphone-search."
        st, out = self._state()
        infos = [self._info(m) for m in out]
        n_match = len([i for i in infos if i["part"] == "match"])
        n_rel = len(infos) - n_match
        if ax == S + "state":
            return [{"state": st, "active": self.active, "text": self.text, "count": len(out), "meaning": self.meaning,
                     "meaning_missing": self._line_visible(), "naming": self.naming, "menu": self.menu, "opened": self.opened, "opened_hit": self.hit}]
        if ax == S + "field":
            return [{"text": self.text}]
        if ax == S + "filters":
            labels = {"account": self.f["account"], "sender": self.f["sender"], "date": self.f["date"], "attachment": self.f["attachment"]}
            rows = []
            for kind, v in labels.items():
                on = bool(v)
                label = {"sender": "From %s" % v, "date": "Before %s" % str(v or "").capitalize(), "attachment": "Has attachment", "account": str(v)}[kind] if on else kind.capitalize()
                if self.m == "chip-not-shown":
                    on = False
                if self.m == "typed-from-makes-a-pill" and kind == "sender" and self.text.lower().startswith("from "):
                    on = True
                rows.append({"kind": kind, "on": on, "label": label})
            if self.f["domain"]:
                rows.insert(2, {"kind": "domain", "on": True, "label": "From @%s" % self.f["domain"]})
            return rows
        if ax == S + "filter.menu":
            if self.menu == "sender":
                return [{"id": str(i), "title": n} for i, n in enumerate(self._senders())]
            return []
        if ax == S + "results":
            res = []
            for m, i in zip(out, infos):
                row = {"id": m["id"], "thread": m["thread"], "account": m["account"], "account_name": m["account"], "sender": m["from_name"],
                       "subject": m["subject"], "highlights": self._marks(BY_ID[i["hit"]] if i["found"] else m), "attachment": i["attachment"], "attachment_says": i["attachment_says"],
                       "matched_in": i["matched_in"], "found": i["found"], "found_line": i["found_line"], "bold": i["bold"], "hit_message": i["hit"]}
                if self.m != "part-missing":
                    row["part"] = i["part"]
                res.append(row)
            return res
        if ax == S + "header":
            line = "1 conversation" if len(out) == 1 else "%d conversations" % len(out)   # a mutant: the old line that counted every row
            if self.m != "count-line-counts-conversations":
                line = count_line(n_match, n_rel)
            return [{"count": line, "matches": n_match, "related": n_rel, "rest": 0}] if st in ("results", "none") else []
        if ax == S + "divider":
            return [{"title": "Related", "before": n_match}] if n_rel and self.m != "related-line-missing" else []
        if ax == S + "domain":
            if self.m == "domain-offer-missing":
                return []
            return [{"domain": dm, "title": "From domain", "people": "1 person" if n == 1 else "%d people" % n, "chip": "From @%s" % dm} for dm, n in self._offers()]
        if ax == S + "skeleton":
            return [{"loading": st == "searching" and not out and self.m != "skeleton-never"}]
        if ax == "iphone-thread.attachments":
            if self.overlay != "iphone-thread" or self.m == "thread-no-attachments":
                return []
            return [{"ax": "iphone-thread.attachments.%d" % i, "name": n} for i, n in enumerate(BY_ID[self.opened]["attachments"])]
        if ax == S + "sections":
            return ([{"title": "Top results"}] + ([{"title": "All results"}] if len(out) > 3 else [])) if out else []
        if ax == S + "footer":
            return [{"text": EXP["results_footer"]}] if out else []
        if ax == S + "empty":
            return [{"title": "Nothing matches" if self.m == "no-result-title-old" else "No match"}] if st == "none" else []
        if ax == S + "recent":
            return [{"text": r} for r in self.recents]
        if ax == S + "meaning":
            if not self._line_visible():
                return []
            words = NEW_SENTENCE if self.m != "meaning-line-for-mac" else NEW_SENTENCE.replace("iPhone", "Mac")
            return [{"state": self.meaning if self.meaning != "ready" else "missing", "text": words, "taps": self.taps}]
        if ax == S + "save":
            return [{"open": self.naming, "text": self.name, "says": self.says, "saved": self.saved, "rule": self._rule()}]
        return []

    def settle(self):
        pass


# ---------------------------------------------------------------- selftest
MODEL_MUTANTS = {          # mutant -> the case that must go red
    "waits-for-return": "results-while-typing", "one-row-per-message": "grouped-by-thread", "no-marks": "matched-words-marked",
    "open-does-nothing": "opens-a-result", "text-lost-on-open": "text-kept-after-open", "thread-back-dead": "close-returns-to-inbox", "sender-filter-dead": "sender-narrows",
    "account-filter-dead": "account-narrows", "date-filter-dead": "date-narrows", "attachment-filter-dead": "attachment-narrows",
    "clear-does-not-widen": "clear-restores", "no-result-state-missing": "no-result-says-so", "meaning-line-never": "meaning-line-says-missing",
    "meaning-line-for-mac": "meaning-line-words", "door-dead": "door-pressed", "line-stays-when-ready": "meaning-line-gone-when-ready",
    "save-dead": "save-opens-the-name", "save-duplicate-allowed": "save-refuses-a-taken-name", "close-stays": "close-returns-to-inbox",
    "idle-lists-results": "idle-before-typing", "recent-not-kept": "submit-keeps-a-recent", "chip-not-shown": "chip-shows-its-value",
    "menu-never-opens": "picker-opens", "control-missing": "every-control-tapped", "count-line-counts-conversations": "sections-header-footer",
    "part-missing": "matches-related-and-the-closed-line",
    "skeleton-never": "skeleton-on-the-same-frame", "skeleton-too-small": "skeleton-fills-the-pane",
    "meaning-row-tinted": "meaning-only-row-tints-nothing", "paperclip-everywhere": "paperclip-only-where-the-match-is-in-the-attachment",
    "typed-from-makes-a-pill": "typed-from-makes-no-pill", "field-clear-keeps-text": ("field-shows-only-the-exact-query", "photo-query-in-the-field"),
    "figure-not-highlighted": "figure-highlighted", "attachment-not-searched": "photo-message-ranks-first", "row-names-no-attachment": "photo-attachment-named-in-the-row",
    "attachment-content-not-read": "photo-found-by-what-it-shows", "not-downloaded-silent": "not-downloaded-row-says-so",
    "unreadable-dropped": "unreadable-kind-searched-by-name-and-says-so",
    "open-does-not-mark-read": "open-marks-read", "thread-no-attachments": "open-shows-the-attachment", "related-line-missing": "related-line-above-related-mail",
    "found-missing": "passage-line-names-its-source", "opened-hit-missing": "opens-the-mail-with-the-passage", "no-bold": "typed-letters-are-bold-on-a-match",
    "domain-offer-missing": "domain-offered-with-its-people", "domain-filter-dead": "domain-pill-keeps-only-that-domain", "domain-pill-stays": "domain-pill-comes-off",
    "no-result-title-old": "no-result-says-so",
}
NOT_JUDGED_MUTANTS = {"measure-refused": ("skeleton-fills-the-pane",)}   # a refused measure is NOT JUDGED, never a pass or a fail
SOURCE_MUTANTS = [          # (name, file key, regex, replacement, the check that must go red)
    ("literal-text", "Results", r"SearchPhoneHeader\(model: model, line: model.countLine\)", 'Text("Results found"); SearchPhoneHeader(model: model, line: model.countLine)', "no-literal-text"),
    ("network-call", "Search", r"func subscribe\(\) \{", 'func subscribe() { _ = URLSession.shared', "no-network"),
    ("reads-app-folder", "Acts", r"func reloadRecents\(\) \{", 'func reloadRecents() { _ = "Application Support"', "no-private-reads"),
    ("sleeps-on-typing", "Acts", r"await Task\.yield\(\)", "try? await Task.sleep(for: .milliseconds(80))", "nothing-waits"),
    ("search-in-the-key", "Acts", r"(func typedChanged\(\) \{\n\s*guard active else \{ return \}\n)", r"\1        refresh()\n", "typing-never-waits"),
    ("read-skips-settle", "Acts", r"(func settle\(\) \{\n)(.*?)(        refresh\(\)\n)", r"\1\2        _ = 0\n", "read-settles-first"),
    ("id-dropped", "Cards", r'\.accessibilityIdentifier\("iphone-search\.save\.cancel"\)', "", "every-control-has-id"),
    ("click-dropped", "Feature", r'"iphone-search\.field\.submit": \{ _ in s\.active && s\.submit\(\) \},\n', "", "every-control-has-click"),
    ("file-too-long", "Row", r"\Z", "\n" * 220, "file-size"),
    ("edge-strip", "Row", r"\.background\(PhoneGround\.list\)", ".background(PhoneGround.list).border(Color.red)", "no-edge-strip"),
    ("unknown-sentence", "Cards", r'Words\.text\("Name the smart folder"\)', 'Words.text("Name this folder please")', "strings-in-catalogue"),
    ("unlisted-number", "Row", r"static let skeletonBarHeight: CGFloat = 9", "static let skeletonBarHeight: CGFloat = 9\n    static let zzNew: CGFloat = 3", "new-numbers-listed"),
    ("door-is-own", "Save", r"AISetupDoors\.fix\(\.meaning\)", "()", "meaning-door-is-ai-setup"),
    ("own-search", "Search", r"SearchEngine\.fromHits", "SearchEngineX.fromHits", "same-search-as-mac"),
    ("own-domain-offers", "Domain", r"SearchSuggest\.build\(draft", "OwnSuggest.build(draft", "domain-offers-are-the-macs"),
    ("own-related-line", "Parts", r"SearchResultsList\.titles\(related", "OwnTitles.titles(related", "related-line-is-the-macs"),
    ("own-passage", "Found", r"r\.senderMarks\.map", "SearchEngine.wordRanges(r.row.senderName).map", "passage-is-the-engines"),
    ("hit-not-read", "Acts", r"r\.hitMessageId\.isEmpty \? r\.row\.messageId : r\.hitMessageId", "r.row.messageId", "hit-mail-is-read"),
]


def run_rows(rows):
    """Prints one line per case; returns (cases, failed, not judged). A row whose verdict is None could not be judged (its measure was refused)."""
    failed = unjudged = 0
    for name, ok, why in rows:
        print("%s %-32s %s" % ("NOT JUDGED" if ok is None else "ok  " if ok else "FAIL", name, why))
        failed += 1 if ok is False else 0
        unjudged += 1 if ok is None else 0
    return len(rows), failed, unjudged


def selftest():
    n = f = u = 0
    base = load_sources()
    for rows in (judge_sources(base), judge_steps(journey_text()), cases(Model())):
        a, b, c = run_rows(rows)
        n, f, u = n + a, f + b, u + c
    print("\nMUTANTS (each must turn its own check red)")
    missed = 0
    for name, key, pat, rep, check in SOURCE_MUTANTS:
        mutated, count = re.subn(pat, rep, base[key], count=1, flags=re.S)
        if not count:
            print("NOT APPLIED  %-24s pattern not found in %s" % (name, key))
            missed += 1
            continue
        t = dict(base)
        t[key] = mutated
        red = {r[0] for r in judge_sources(t) if r[1] is False}
        ok = check in red
        print("%s %-24s red: %s" % ("red " if ok else "MISSED", name, ", ".join(sorted(red)) or "none"))
        missed += 0 if ok else 1
    for mutant, check in MODEL_MUTANTS.items():
        try:
            red = {c[0] for c in cases(Model(mutant)) if c[1] is False}
        except (RuntimeError, KeyError, IndexError) as e:
            red = {"<stopped: %s>" % e}
        ok = all(c in red for c in ((check,) if isinstance(check, str) else check))
        print("%s %-24s red: %s" % ("red " if ok else "MISSED", "model:" + mutant, ", ".join(sorted(red)) or "none"))
        missed += 0 if ok else 1
    for mutant, names in NOT_JUDGED_MUTANTS.items():
        rows = cases(Model(mutant))
        nj = {c[0] for c in rows if c[1] is None}
        wrongly = {c[0] for c in rows if c[1] is False and c[0] in names}
        ok = set(names) <= nj and not wrongly
        print("%s %-24s not judged: %s%s" % ("nj  " if ok else "MISSED", "model:" + mutant, ", ".join(sorted(nj)) or "none", ("; recorded as FAIL: %s" % sorted(wrongly)) if wrongly else ""))
        missed += 0 if ok else 1
    for mutant, row in JOURNEY_MUTANTS.items():
        red = {r[0] for r in judge_steps(journey_text(mutant)) if r[1] is False}
        ok = row in red
        print("%s %-24s red: %s" % ("red " if ok else "MISSED", "journey:" + mutant, ", ".join(sorted(red)) or "none"))
        missed += 0 if ok else 1
    n += len(SOURCE_MUTANTS) + len(MODEL_MUTANTS) + len(NOT_JUDGED_MUTANTS) + len(JOURNEY_MUTANTS)
    f += missed
    print("\n%d checks, %d failed, %d not judged" % (n, f, u))
    return 1 if f or u else 0


def main(argv):
    if "--selftest" in argv:
        return selftest()
    rows = judge_sources(load_sources()) + judge_steps(journey_text())
    unjudged = 0
    if "--live" in argv:
        try:
            rows += cases(Socket())
        except (RuntimeError, OSError, ValueError, KeyError, IndexError) as e:
            print("NOT JUDGED live cases: %s" % e)
            unjudged += 1
    n, f, u = run_rows(rows)
    unjudged += u
    print("\n%d checks, %d failed, %d not judged" % (n, f, unjudged))
    return 1 if f or unjudged else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
