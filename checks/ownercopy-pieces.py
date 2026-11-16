#!/usr/bin/env python3
"""Words in no vector, on a PRIVATE COPY of the owner's mail database, on this Mac only (item c1436f49 "Cut mail text so the model reads every word",
and item eaf347e2's attachment pieces). The LEAD runs it; nobody else reads the owner's data.

Run (inside the launcher, the real engine, hidden; the copy is the launcher's own run folder), as checks/ownercopy-search.py:
    WHOOSH_RIG_HIDDEN=1 sh scripts/launch-one.sh <bundle> -- --rig-hidden sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/ownercopy-pieces.py'
    python3 checks/ownercopy-pieces.py --selftest      judges the pure rules on made-up engine replies; launches nothing, reads nothing of the owner's

What it does: stops the hidden copy, takes a consistent snapshot of the installed database (and its index folder) with SQLite's backup API into the copy's own
data folder (ownercopy-search.py `snapshot`: the installed file is opened read-only), starts the same copy again on it, and asks the ENGINE for its counts page
by page: `core state index/pieces/<after row>/<limit>` (core/crates/engine/src/indexer.rs `piece_counts`). The engine reads each stored mail with its own text
(`text_of`), cleans it with its own cleaner (search_vector::pieces::clean_for_meaning) and sizes with its own counter: this script holds no copy of the rule.
Then it stops the copy and deletes the snapshot.

PRINTS COUNTS ONLY: never a subject, an address or a word of a mail (the engine's reply holds none; every line goes through ownercopy-search.py's `say`,
which refuses a line that holds a word of the copy's subjects).

Lines: mails read (under the cap INDEX_BODY_MAX_CHARS) and mails over the cap, words in all, words in no stored piece, words in no vector, pieces, pieces the
model had to cut, attachment pieces with a vector, the counter the engine sized with. The last line:
    RESULT: PASS | words in no vector outside code pieces 0 of <N> | words in code pieces <C> | ...   (PASS only when that count is 0, N >= 1, and no
    piece had to be cut; the words in pieces left out as mostly codes, which get no vector by rule, are printed beside it: the lead's ruling, round 21)
    RESULT: FAIL | words in no vector outside code pieces <K> of <N> | ...
Item eaf347e2 reads `attachment pieces with a vector: <n>` (above 0 for mail whose files are on disk).
NOT JUDGED (exit 1): no run folder, no installed database, an engine that serves no `index/pieces` (an older build); the copy not answering (exit 3).
"""
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PAGE = 200                     # mails per engine read: one read stays well inside the socket's 5 s (journeys/lib/ctl.py TIMEOUT_S)
SETTLE_S = 3                   # the engine opens its store and indexes after the restart
# stale-fix (item c1436f49, main194): a build whose piece rule marker moved removes the copied meaning index at its first open and cuts and embeds every stored
# mail again. The count is the verdict only once the index stands still: the same figures on STILL_READS reads, INDEX_POLL_S apart, inside INDEX_WAIT_S.
INDEX_WAIT_S, INDEX_POLL_S, STILL_READS = 1500, 10, 3
# main194, second run (16 Nov): the app handed its model after the first three reads, so the index "stood still" on the copied state, before any cut. No read
# counts as still before INDEX_MIN_S: the time the app has to load its model and start.
INDEX_MIN_S = 120
KEYS = ("mails", "over_cap", "words", "words_in_no_piece", "words_in_no_vector", "pieces", "pieces_cut", "attachment_pieces_with_vector")
# round 20 (main111: "words in no stored piece 4 of 72809"): why a word is in no piece, served by the engine since round 20 (an older engine serves none)
CAUSES = ("words_lost_by_the_cut", "words_in_code_pieces", "words_in_stale_pieces")
LISTED_MAX = 40

_spec = importlib.util.spec_from_file_location("ownercopy_search", os.path.join(ROOT, "checks", "ownercopy-search.py"))
OS = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(OS)   # snapshot, stop_copy, protect_words, say: one copy of the private-copy rules
say = OS.say


def add_page(sums, page):
    """One engine reply added to the sums. Returns the reply's `last` row (None: no mail left). A reply that is not the engine's counts raises ValueError."""
    if not isinstance(page, dict) or "error" in page or any(not isinstance(page.get(k), int) for k in KEYS):
        raise ValueError("the engine's index/pieces reply is not its counts: keys %s" % (sorted(page) if isinstance(page, dict) else type(page).__name__))
    for k in KEYS:
        sums[k] = sums.get(k, 0) + page[k]
    for k in CAUSES:
        if isinstance(page.get(k), int):
            sums[k] = sums.get(k, 0) + page[k]
    # the mails with a word in no piece: [row id, words, cause] (numbers and a fixed word, never text)
    for m in page.get("mails_with_words_in_no_piece") or []:
        # round 23: [row id, words, cause, the cleaned text's bytes, stored pieces, words outside the cut] (an engine before round 23 serves the first three)
        if isinstance(m, list) and len(m) in (3, 6) and all(isinstance(x, int) for x in m[:2] + m[3:]) and m[2] in ("stale", "cut", "code-piece"):
            sums.setdefault("listed", [])
            if len(sums["listed"]) < LISTED_MAX:
                sums["listed"].append(m)
    sums["counter"] = page.get("counter")
    return page.get("last")


def lines(sums):
    """The count lines and the verdict line, from the sums alone (numbers only)."""
    out = ["mails read: %d (over the cap, left out: %d)" % (sums["mails"], sums["over_cap"]),
           "words in all: %d" % sums["words"],
           "words in no stored piece: %d" % sums["words_in_no_piece"],
           "words in no vector: %d" % sums["words_in_no_vector"],
           "pieces: %d" % sums["pieces"],
           "pieces the model had to cut: %d" % sums["pieces_cut"],
           "attachment pieces with a vector: %d" % sums["attachment_pieces_with_vector"],
           "counter: %s" % sums.get("counter")]
    if any(k in sums for k in CAUSES):
        out.append("words in no piece by cause: lost by the cut %d, in pieces left out as codes %d, in stale pieces %d"
                   % tuple(sums.get(k, 0) for k in CAUSES))
        out.append("mails with a word in no piece (row id, words, cause[, text bytes, stored pieces, words outside the cut]): %s"
                   % ("; ".join(" ".join(str(x) for x in m) for m in sums.get("listed", [])) or "none"))
    # the lead's ruling (round 21, 13 Nov): a piece left out as mostly codes keeps no vector by the pieces lane's rule (pieces::mostly_codes), so the verdict
    # counts the words in no vector OUTSIDE such pieces; the words in code pieces are printed beside it, never hidden
    codes = sums.get("words_in_code_pieces", 0)
    outside = max(0, sums["words_in_no_vector"] - codes)
    ok = sums["words"] >= 1 and outside == 0 and sums["pieces_cut"] == 0
    out.append("RESULT: %s | words in no vector outside code pieces %d of %d | words in code pieces %d | words in no stored piece %d | pieces cut %d of %d | mails %d | attachment pieces with a vector %d"
               % ("PASS" if ok else "FAIL", outside, sums["words"], codes, sums["words_in_no_piece"], sums["pieces_cut"], sums["pieces"],
                  sums["mails"], sums["attachment_pieces_with_vector"]))
    return out


def still(reads, n=STILL_READS):
    """The index stands still: the last `n` reads (each a tuple of figures) are the same. Fewer than `n` reads is never still."""
    return len(reads) >= n and all(r == reads[-1] for r in reads[-n:])


def settled_count(read, wait_s=INDEX_WAIT_S, poll_s=INDEX_POLL_S, sleep=time.sleep, clock=time.time, min_s=INDEX_MIN_S):
    """The sums once the index stands still (`still`), with how the wait went: (sums, reads, seconds, stood_still). When the wait runs out the last sums are
    answered with stood_still False: the verdict line is then of an index still at work, and the caller says so."""
    reads, t0 = [], clock()
    while True:
        sums = count(read)
        reads.append((sums["words_in_no_vector"], sums["pieces"], sums["words_in_no_piece"]))
        if still(reads) and clock() - t0 >= min_s:
            return sums, reads, clock() - t0, True
        if clock() - t0 >= wait_s:
            return sums, reads, clock() - t0, False
        sleep(poll_s)


def final(result_line, counter, stood):
    """What the run ends on: (the extra line to print or None, the exit code). The counts are a verdict only when the index stood still AND the engine sized
    with the model's own tokenizer: with the byte bound (the tokenizer not known to the engine) the pieces of a correct cut read as over the model's input and
    the code pieces as stale (main194, 16 Nov: 491 of 548 pieces "cut", after the app had released the model)."""
    if not stood:
        return "RESULT: NOT JUDGED, the index was still at work after %d s (the count above is of a moment, not a verdict)" % INDEX_WAIT_S, 1
    if counter != "model tokenizer":
        return "RESULT: NOT JUDGED, the engine sized with %r, not the model's tokenizer: the counts above are not the model's" % (counter,), 1
    return None, 0 if result_line.startswith("RESULT: PASS") else 1


def count(read):
    """Pages through `read(after, limit)` (the engine's reply, parsed) until the engine says no mail is left. Returns the sums."""
    sums, after = {}, 0
    while True:
        last = add_page(sums, read(after, PAGE))
        if last is None:
            return sums
        if last <= after:
            raise ValueError("the engine's last row %r does not move past %r" % (last, after))
        after = last


# ---------------------------------------------------------------- the run (inside the launcher)
def run():
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl                                              # noqa: E402
    d = os.environ.get("D") or os.environ.get("WHOOSH_SOCKET_DIR")
    if not d:
        say("NOT JUDGED: no run folder ($D): run it inside scripts/launch-one.sh")
        return 1
    if not os.path.exists(os.path.join(OS.OWNER, OS.DB_NAME)):
        say("NOT JUDGED: no installed database")
        return 1
    core = os.path.join(d, "core")
    say("hidden copy stopped: pid gone =", OS.stop_copy(d))
    if os.path.isdir(core):
        shutil.rmtree(core)
    path = OS.snapshot(OS.OWNER, core)
    say("private copy at", os.path.abspath(core), "(mode %o)" % (os.stat(core).st_mode & 0o777))
    OS.protect_words(path)
    # the installed layout: the model's folder sits beside the data folder (indexer.rs `installed_tokenizer`: <data dir>/../Models/embeddinggemma-2/<revision>),
    # so the engine knows the model's tokenizer also after the app released the model. The copy gets the same: a link to the weights this run was given.
    weights = os.environ.get("WHOOSH_EMBED_WEIGHTS") or ""
    if weights and os.path.exists(os.path.join(weights, "tokenizer.json")):
        mdir = os.path.join(d, "Models", "embeddinggemma-2")
        os.makedirs(mdir, exist_ok=True)
        if not os.path.lexists(os.path.join(mdir, "run-weights")):
            os.symlink(weights, os.path.join(mdir, "run-weights"))
        say("model folder linked beside the copy: yes")
    else:
        say("model folder linked beside the copy: no (WHOOSH_EMBED_WEIGHTS names no folder with a tokenizer.json)")
    p = subprocess.run(["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), "--relaunch"], env=dict(os.environ, D=d, WHOOSH_RIG_HIDDEN="1"), capture_output=True, text=True, cwd=ROOT)
    say("relaunch exit", p.returncode)
    c, t0 = None, time.time()
    while time.time() - t0 < 60:
        try:
            c = Ctl(d)
            c.connect()
            c.uistate()
            break
        except Exception:                                                # noqa: BLE001
            c = None
            time.sleep(0.5)
    if c is None:
        say("RESULT: NOT JUDGED, the hidden copy did not answer after the restart")
        return 3
    time.sleep(SETTLE_S)

    def read(after, limit):
        v = c.core("index/pieces/%d/%d" % (after, limit))
        return json.loads(v) if isinstance(v, str) else v

    code = 1
    try:
        sums, reads, took, stood = settled_count(read)
        say("index read %d times over %d s; words in no vector at the last reads: %s; %s" % (
            len(reads), took, [r[0] for r in reads[-6:]], "it stood still" if stood else "it did NOT stand still inside %d s" % INDEX_WAIT_S))
        out = lines(sums)
        for line in out:
            say(line)
        extra, code = final(out[-1], sums.get("counter"), stood)
        if extra:
            say(extra)
    except Exception as e:                                               # noqa: BLE001  an engine with no index/pieces read is not a verdict
        say("RESULT: NOT JUDGED, the engine served no piece counts (%s)" % type(e).__name__)
    with open(os.path.join(d, "stopped-by-command"), "w") as f:         # the launcher reads the stop as this command's own (ownercopy-search.py)
        f.write(open(os.path.join(d, "rig.pid")).read().strip())
    OS.stop_copy(d)
    shutil.rmtree(core, ignore_errors=True)
    say("private copy deleted:", "yes" if not os.path.exists(core) else "no")
    say("DONE (the launcher stops the copy and removes the run folder)")
    return code


# ---------------------------------------------------------------- the self-test
def selftest():
    """The pure rules on made-up engine replies: the sums over pages, the verdict (0 words in no vector passes; ONE word in no piece of one mail fails; a piece
    the model had to cut fails; nothing read fails), a reply that is not the counts, a `last` that does not move, and the privacy guard on the printed lines."""
    bad, shown = [], []

    def page(**kw):
        p = {"mails": 2, "over_cap": 0, "words": 40, "words_in_no_piece": 0, "words_in_no_vector": 0, "pieces": 3, "pieces_cut": 0,
             "attachment_pieces_with_vector": 1, "last": 10, "counter": "model tokenizer"}
        p.update(kw)
        return p

    def engine(pages):
        it = iter(pages)
        return lambda after, limit: next(it)

    # the causes and the mail rows (round 20): summed and printed as numbers and fixed words
    why = lines(count(engine([page(words_in_no_piece=2, words_in_no_vector=2, words_lost_by_the_cut=0, words_in_code_pieces=2, words_in_stale_pieces=0,
                                   mails_with_words_in_no_piece=[[4127, 2, "code-piece", 5120, 3, 0]], last=None)])))
    shown += why[-3:]
    if "words in no piece by cause: lost by the cut 0, in pieces left out as codes 2, in stale pieces 0" not in why or \
            "mails with a word in no piece (row id, words, cause[, text bytes, stored pieces, words outside the cut]): 4127 2 code-piece 5120 3 0" not in why:
        bad.append("the causes or the mail rows are not printed: %r" % why[-3:])
    good = count(engine([page(last=10), page(last=25), page(mails=0, words=0, pieces=0, attachment_pieces_with_vector=0, last=None)]))
    out = lines(good)
    shown += out
    if not out[-1].startswith("RESULT: PASS | words in no vector outside code pieces 0 of 80 | words in code pieces 0 |") or "attachment pieces with a vector: 2" not in out:
        bad.append("the correct pages do not pass with their sums: %r" % out[-1])
    # one mail with a word in no piece (the engine counts it in no piece and in no vector): red
    one = lines(count(engine([page(last=10), page(words_in_no_piece=1, words_in_no_vector=1, last=None)])))
    shown.append("one word in no piece: " + one[-1])
    if not one[-1].startswith("RESULT: FAIL | words in no vector outside code pieces 1 of 80"):
        bad.append("a mail with a word in no piece passes: %r" % one[-1])
    # words only in pieces left out as codes: PASS, the count printed beside (the lead's ruling); one more word outside them: FAIL
    cp = lines(count(engine([page(words_in_no_piece=3, words_in_no_vector=3, words_in_code_pieces=3, last=None)])))
    shown.append("code pieces only: " + cp[-1])
    if not cp[-1].startswith("RESULT: PASS | words in no vector outside code pieces 0 of 40 | words in code pieces 3 |"):
        bad.append("words only in code pieces do not pass with their count: %r" % cp[-1])
    if not lines(count(engine([page(words_in_no_piece=4, words_in_no_vector=4, words_in_code_pieces=3, last=None)])))[-1].startswith("RESULT: FAIL"):
        bad.append("a word outside code pieces passes")
    # a word in a piece that has no vector yet: red
    novec = lines(count(engine([page(words_in_no_vector=3, last=None)])))
    if not novec[-1].startswith("RESULT: FAIL"):
        bad.append("a word in a piece with no vector passes")
    # a piece the model had to cut: red
    if not lines(count(engine([page(pieces_cut=1, last=None)])))[-1].startswith("RESULT: FAIL"):
        bad.append("a piece over the model's input passes")
    # nothing read: red, never a vacuous pass
    if not lines(count(engine([page(mails=0, words=0, pieces=0, last=None)])))[-1].startswith("RESULT: FAIL"):
        bad.append("an empty copy passes")
    # a reply that is not the counts, and a row that does not move
    for name, pages in (("error reply", [{"error": "the store did not open a read"}]), ("missing key", [{"mails": 1, "last": None}]),
                        ("stuck row", [page(last=5), page(last=5)])):
        try:
            count(engine(pages))
            bad.append("%s was summed" % name)
        except ValueError:
            shown.append("%s refused" % name)
    # the wait for a still index (stale-fix): three equal reads end it; a count that keeps moving runs the wait out and is not a verdict
    if still([(5, 1, 0), (0, 2, 0), (0, 2, 0)]) or not still([(5, 1, 0), (0, 2, 0), (0, 2, 0), (0, 2, 0)]) or still([]):
        bad.append("`still` is wrong on its samples")
    tick = [0.0]
    moving = iter([page(words_in_no_vector=40 - 10 * i, last=None) for i in range(3)] + [page(last=None)] * 9)
    sm, rd, _, stood = settled_count(lambda a, l: next(moving), wait_s=600, poll_s=10, sleep=lambda s_: tick.__setitem__(0, tick[0] + s_), clock=lambda: tick[0], min_s=0)
    if not stood or sm["words_in_no_vector"] != 0 or [r[0] for r in rd] != [40, 30, 20, 0, 0, 0]:
        bad.append("an index at work was counted before it stood still: %r %r" % (stood, [r[0] for r in rd]))
    else:
        shown.append("the count waits for a still index: reads %s" % [r[0] for r in rd])
    # a late start: three equal reads of the copied state, then the cut begins; the floor keeps the first three from being the verdict
    tick[0] = 0.0
    late = iter([page(words_in_no_vector=1416, last=None)] * 4 + [page(words_in_no_vector=900, last=None)] + [page(last=None)] * 20)
    sm3, rd3, _, stood3 = settled_count(lambda a, l: next(late), wait_s=600, poll_s=10, sleep=lambda s_: tick.__setitem__(0, tick[0] + s_), clock=lambda: tick[0], min_s=40)
    if not stood3 or sm3["words_in_no_vector"] != 0:
        bad.append("an index that starts late was counted on its copied state: %r" % [r[0] for r in rd3])
    else:
        shown.append("a late start is waited out: reads %s" % [r[0] for r in rd3])
    tick[0] = 0.0
    never = iter(page(words_in_no_vector=1000 - i, last=None) for i in range(200))
    _, rd2, _, stood2 = settled_count(lambda a, l: next(never), wait_s=60, poll_s=10, sleep=lambda s_: tick.__setitem__(0, tick[0] + s_), clock=lambda: tick[0])
    if stood2 or len(rd2) != 7:
        bad.append("an index that never stands still was called still, or the wait did not end at its ceiling: %r reads %d" % (stood2, len(rd2)))
    else:
        shown.append("an index that keeps moving runs the wait out: not a verdict")
    # what the run ends on: a moving index and a byte-bound count are NOT JUDGED, never a pass and never a fail
    if final("RESULT: PASS | x", "byte bound", True)[1] != 1 or "NOT JUDGED" not in (final("RESULT: PASS | x", "byte bound", True)[0] or ""):
        bad.append("a count sized with the byte bound is a verdict")
    if "NOT JUDGED" not in (final("RESULT: PASS | x", "model tokenizer", False)[0] or ""):
        bad.append("a count of a moving index is a verdict")
    if final("RESULT: PASS | x", "model tokenizer", True) != (None, 0) or final("RESULT: FAIL | x", "model tokenizer", True) != (None, 1):
        bad.append("a still index sized with the model's tokenizer does not end on its own verdict")
    else:
        shown.append("byte bound or a moving index: NOT JUDGED; the model's tokenizer on a still index: the verdict")
    # the privacy guard: a subject word of the copy is never printed, the fixed lines are
    canary = "CANARY" + "PIECE" + "WORD"
    OS.FORBIDDEN.add(canary.lower())
    leaked = say("a line holding", canary)
    kept = [say(x) for x in out]
    OS.FORBIDDEN.clear()
    if canary in leaked or any(k.startswith("REFUSED") for k in kept):
        bad.append("the guard printed mail text or hid a count line")
    print("\n".join(shown))
    print("%d problems" % len(bad))
    for b in bad:
        sys.stderr.write("SELFTEST FAIL %s\n" % b)
    return 1 if bad else 0


def main(argv):
    if "--selftest" in argv:
        return selftest()
    return run()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
