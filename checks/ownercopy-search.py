#!/usr/bin/env python3
"""The search on a PRIVATE COPY of the owner's mail database, on this Mac only (items 83ca4389 and 6836382a). The LEAD runs it; nobody else reads the owner's data.

Run (inside the launcher, the real engine, hidden; the copy is the launcher's own run folder):
    WHOOSH_RIG_HIDDEN=1 sh scripts/launch-one.sh <bundle> -- --rig-hidden sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/ownercopy-search.py [query]'
    python3 checks/ownercopy-search.py --selftest      judges the pure rules on a made-up database; launches nothing, reads nothing of the owner's

What it does: stops the hidden copy, takes a consistent snapshot of the installed database with SQLite's backup API (the installed file is opened read-only, never written)
into the copy's own data folder (mode 0700, the absolute path is printed), starts the same copy again on it, types the query in the real search field, reads the engine's
own state and the screen's rows, then stops the copy and deletes the snapshot. The hidden copy reaches no mail server and holds no password.

PRINTS COUNTS AND IDS ONLY: never a subject, a sender, a passage or any text of a mail. Every line goes through `say`, which refuses a line that holds a word of the copy's own text.

Today's rule (SPEC A148 to A150): the match group holds EVERY mail whose subject, sender's name, sender's address or text holds the typed letters (inside a longer word too),
every row of the match group serves a non-empty evidence, the related count is read (the header's and the engine's), the reader's mail is the selected result's after each move,
and Enter opens that mail with the results kept.
Verdict lines: `RESULT: PASS | exact K of M | ...` (PASS only when K == M and M >= 1 and every part below holds), `A148 RESULT: PASS|FAIL`.
NOT JUDGED (exit 1): `NOT JUDGED: no installed database` when the installed database is absent; the copy not answering after the restart (exit 3).
"""
import json
import os
import re
import shutil
import signal
import sqlite3
import subprocess
import sys
import time
import unicodedata

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OWNER = os.path.expanduser("~/Library/Application Support/Whoosh/Core")
DB_NAME = "whoosh.sqlite"
DEFAULT_QUERY = "consultant"
LEFT_OUT_FOLDERS = re.compile(r"junk|trash|spam|deleted|bin$", re.I)      # the engine leaves Junk and Trash out (search_cmd.rs module comment)
FINAL_STAGES = ("ranked", "no_model", "model_failed", "filtered", "failed")
NOT_EXACT = ("meaning", "related")                                       # matched_in of the hits that are not in the match group
ASK_WAIT_S = 40
READER_WAIT_S = 3.0
FORBIDDEN = set()                                                        # words of the copy's own text: a printed line that holds one is refused


def fold(text):
    """Case and accents folded, as the engine matches (rust_core::people::fold)."""
    return "".join(ch for ch in unicodedata.normalize("NFKD", (text or "").casefold()) if not unicodedata.combining(ch))


def say(*parts):
    """The only way out. A line holding a protected word (the selftest's canary, a subject word the run registered) is refused, never printed."""
    line = " ".join(str(p) for p in parts)
    low = line.lower()
    if any(w and w in low for w in FORBIDDEN):
        line = "REFUSED a line that held mail text"
    print(line, flush=True)
    return line


def snapshot(src_dir, dest_dir):
    """A consistent copy of the installed database through SQLite's backup API (the installed file is opened read-only), the index folder as it is on disk. Returns the copy's path."""
    os.makedirs(dest_dir, mode=0o700, exist_ok=True)
    src = sqlite3.connect("file:%s?mode=ro" % os.path.join(src_dir, DB_NAME), uri=True)
    dst = sqlite3.connect(os.path.join(dest_dir, DB_NAME))
    try:
        src.backup(dst)
    finally:
        dst.close()
        src.close()
    if os.path.isdir(os.path.join(src_dir, "index")):
        shutil.copytree(os.path.join(src_dir, "index"), os.path.join(dest_dir, "index"))
    return os.path.join(dest_dir, DB_NAME)


def expected_ids(db_path, query):
    """The message ids the match group must hold: every mail outside Junk and Trash whose subject, sender name, sender address or text holds the typed letters, folded.
    Returns (ids, mails in the copy, {folder: count}, mails left out by folder). Counts and ids only."""
    db = sqlite3.connect("file:%s?mode=ro" % db_path, uri=True)
    try:
        q = fold(query)
        total = db.execute("select count(*) from message").fetchone()[0]
        folders = dict(db.execute("select folder, count(*) from message group by folder"))
        ids, left_out = [], 0
        for mid, folder, subject, sender, address, body in db.execute("select id, folder, subject, sender_name, from_address, body_text from message order by id"):
            if LEFT_OUT_FOLDERS.search(folder or ""):
                left_out += 1
                continue
            if any(q in fold(x) for x in (subject, sender, address, body)):
                ids.append(mid)
        return ids, total, folders, left_out
    finally:
        db.close()


def protect_words(db_path):
    """Registers the words of the copy's own subjects as never printable (a guard on `say`, not output): the first words are enough to refuse a line that leaks one."""
    db = sqlite3.connect("file:%s?mode=ro" % db_path, uri=True)
    try:
        for (subject,) in db.execute("select subject from message limit 4000"):
            for w in re.findall(r"[a-z]{6,}", (subject or "").lower()):
                FORBIDDEN.add(w)
    finally:
        db.close()
    # the words of this script's own fixed sentences are never protected (a template word that is also in a subject must not hide a verdict line)
    FORBIDDEN.difference_update(re.findall(r"[a-z]{6,}", open(os.path.abspath(__file__), encoding="utf-8").read().lower()))


def exact_group(hits):
    """The ids of the hits before the first one found by meaning or as related: the engine's match group (hits are (id, matched_in) in the engine's order)."""
    out = []
    for mid, where in hits:
        if where in NOT_EXACT:
            break
        out.append(mid)
    return out


def judge(want, hits, rows, header, engine_related, reader):
    """Every part of today's pass line, from what the run read. `rows` are the screen's result rows ({id, hit_id, part, evidence_in, evidence_bold}), `header` the count row
    ({matches, related}), `engine_related` the engine's `search/last/related`, `reader` the A148 reads. Returns {part: (ok, text)}; text holds counts and ids only."""
    exact = exact_group(hits)
    missing = [m for m in want if m not in exact]
    shown = {r.get("id") for r in rows} | {r.get("hit_id") for r in rows}
    no_row = [m for m in want if m not in shown]
    match_rows = [r for r in rows if r.get("part") == "match"]
    bare = [r.get("id") for r in match_rows if r.get("evidence_in") in (None, "none") or not r.get("evidence_bold")]
    related_rows = sum(1 for r in rows if r.get("part") == "related")
    out = {
        "match group": (len(want) >= 1 and not missing, "exact %d of %d, missing ids %s" % (len(want) - len(missing), len(want), missing)),
        "rows": (len(want) >= 1 and not no_row, "rows %d of %d, ids with no row %s" % (len(want) - len(no_row), len(want), no_row)),
        "evidence": (len(match_rows) >= 1 and not bare, "evidence %d of %d match rows, rows with none %s" % (len(match_rows) - len(bare), len(match_rows), bare)),
        "related": (isinstance(engine_related, int) and isinstance(header.get("related"), int) and header.get("related") == related_rows,
                    "related: engine %r, header %r; related rows %d" % (engine_related, header.get("related"), related_rows)),
    }
    s0, r0, s1, r1, surface, r2, listed = (reader.get(k) for k in ("sel0", "rdr0", "sel1", "rdr1", "surface", "rdr2", "listed"))
    out["reader follows"] = (s0 is not None and s0 == r0 and s1 is not None and s1 == r1, "selected tail %s reader tail %s; after Down selected tail %s reader tail %s" % (tail(s0), tail(r0), tail(s1), tail(r1)))
    out["Enter opens it"] = (surface == "mac-reader" and r2 is not None and r2 == s1 and isinstance(listed, int) and listed > 0,
                             "surface %s, reader tail %s, the selected result %s, results still listed %s" % (surface, tail(r2), r2 is not None and r2 == s1, listed))
    return out


def tail(i):
    return str(i)[-12:]


def verdict(parts, want, hits_n, stage):
    """The one line the runner matches, and the A148 line."""
    exact_ok, rows_ok, ev_ok, rel_ok = (parts[k][0] for k in ("match group", "rows", "evidence", "related"))
    a148 = parts["reader follows"][0] and parts["Enter opens it"][0]
    k = int(re.search(r"exact (\d+) of", parts["match group"][1]).group(1))
    ok = exact_ok and rows_ok and ev_ok and rel_ok and a148
    return ("RESULT: %s | exact %d of %d | %s | %s | %s | engine %d mails | stage %s" % ("PASS" if ok else "FAIL", k, len(want), parts["rows"][1].split(",")[0],
            parts["evidence"][1].split(",")[0], parts["related"][1].split(";")[0], hits_n, stage),
            "A148 RESULT: %s" % ("PASS" if a148 else "FAIL"))


# ---------------------------------------------------------------- the run (inside the launcher)
def alive(pid):
    try:
        os.kill(pid, 0)
        return True
    except OSError:
        return False


def stop_copy(d):
    """Stops the hidden copy by the pid in its run folder (the launcher's own marker says it is being restarted, not dead). Returns whether the pid is gone."""
    pid = int(open(os.path.join(d, "rig.pid")).read().strip())
    open(os.path.join(d, "relaunching"), "w").close()
    os.kill(pid, signal.SIGTERM)
    t0 = time.time()
    while alive(pid) and time.time() - t0 < 15:
        time.sleep(0.2)
    if alive(pid):
        os.kill(pid, signal.SIGKILL)
        time.sleep(0.5)
    return not alive(pid)


def run(query):
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl                                              # noqa: E402
    d = os.environ.get("D") or os.environ.get("WHOOSH_SOCKET_DIR")
    if not d:
        say("NOT JUDGED: no run folder ($D): run it inside scripts/launch-one.sh")
        return 1
    if not os.path.exists(os.path.join(OWNER, DB_NAME)):
        say("NOT JUDGED: no installed database")
        return 1
    core = os.path.join(d, "core")
    say("hidden copy stopped: pid gone =", stop_copy(d))
    if os.path.isdir(core):
        shutil.rmtree(core)
    path = snapshot(OWNER, core)
    say("private copy at", os.path.abspath(core), "(mode %o)" % (os.stat(core).st_mode & 0o777))
    protect_words(path)
    want, total, folders, left_out = expected_ids(path, query)
    say("copy holds %d mails; folders %s" % (total, json.dumps(folders, sort_keys=True)))
    say("mails whose subject, sender or text holds the letters: %d (mails in Junk or Trash left out: %d)" % (len(want), left_out))
    say("exact group expected for the query: %d ids %s" % (len(want), want))
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
    time.sleep(3)                                                        # the engine opens its indexes

    def cv(path_):
        try:
            v = c.core(path_)
        except Exception:                                                # noqa: BLE001
            return None
        if isinstance(v, str):
            try:
                return json.loads(v)
            except ValueError:
                return v
        return v

    def rd(fn, default):
        try:
            return fn()
        except Exception:                                                # noqa: BLE001
            return default

    rd(lambda: c.click("mac-search.open"), None)
    rd(lambda: c.click("mac-search.bar"), None)
    c.key("⌘A")
    c.type(query)
    c.key("Enter")
    t0 = time.time()
    while time.time() - t0 < ASK_WAIT_S:
        if cv("search/last/query") == query and cv("search/last/stage") in FINAL_STAGES:
            break
        time.sleep(0.3)
    n = int(cv("search/last/count") or 0)
    hits = [(cv("search/last/hit/%d/message_id" % i), cv("search/last/hit/%d/matched_in" % i)) for i in range(min(n, 1000))]
    rows = rd(lambda: c.rows("mac-search.results"), [])
    header = (rd(lambda: c.rows("mac-search.header"), [{}]) or [{}])[0]
    engine_related = cv("search/last/related")
    stage = cv("search/last/stage")

    def reader_id():
        r = rd(lambda: c.rows("mac-main.reader.subject"), [])
        return r[0].get("id") if r else None

    def selected_id():
        return next((x.get("id") for x in rd(lambda: c.rows("mac-search.results"), []) if x.get("selected")), None)

    def settle():
        end = time.time() + READER_WAIT_S
        while time.time() < end and not (selected_id() is not None and selected_id() == reader_id()):
            time.sleep(0.1)

    settle()
    reader = {"sel0": selected_id(), "rdr0": reader_id()}
    c.key("Down")
    settle()
    reader.update(sel1=selected_id(), rdr1=reader_id())
    c.key("Enter")
    time.sleep(1.0)
    reader.update(surface=rd(lambda: c.uistate().get("surface"), None), rdr2=reader_id(), listed=len(rd(lambda: c.rows("mac-search.results"), [])))
    parts = judge(want, hits, rows, header, engine_related, reader)
    for name, (ok, text) in parts.items():
        say("%s %s: %s" % ("PASS" if ok else "FAIL", name, text))
    line, a148 = verdict(parts, want, n, stage)
    say(a148)
    say(line)
    # the copy goes: the hidden copy is stopped by its pid, the snapshot deleted, and only then "deleted: yes" is said. The stop is announced to the launcher
    # first: `$D/stopped-by-command` holds the pid this command stops, so scripts/launch-one.sh reads its end as this command's own stop, not a death
    # (main92: "DIED ... exit 4" after a PASS; the launcher's exemption is the lead's change, docs/lanes/hid-search.md round 5, finding L1)
    with open(os.path.join(d, "stopped-by-command"), "w") as f:
        f.write(open(os.path.join(d, "rig.pid")).read().strip())
    stop_copy(d)
    shutil.rmtree(core, ignore_errors=True)
    say("private copy deleted:", "yes" if not os.path.exists(core) else "no")
    say("DONE (the launcher stops the copy and removes the run folder)")
    return 0 if line.startswith("RESULT: PASS") else 1


# ---------------------------------------------------------------- the self-test
def selftest():
    """The pure rules on a made-up database and made-up reads: the expected ids (letters inside a word, in the sender, the address, the text; Junk left out), the snapshot (a
    consistent copy, the source untouched), the judge (correct reads pass, each broken read is red), and the privacy guard (a canary subject word is never printed)."""
    import tempfile
    CANARY = "CANARY" + "SUBJ" + "ECTWORD"                                # one token, never written whole in this file
    bad, lines = [], []
    work = tempfile.mkdtemp(prefix="whoosh-ownercopy-selftest-")
    try:
        src = os.path.join(work, "src")
        os.makedirs(src)
        db = sqlite3.connect(os.path.join(src, DB_NAME))
        db.execute("create table message (id text primary key, folder text, subject text, sender_name text, from_address text, body_text text)")
        rows = [("a:1", "INBOX", "Quarterly review", "Ann", "ann@example.test", "the consultants will visit"),          # the letters inside a longer word, in the text
                ("a:2", "INBOX", "brightconsultants.example offer", "Bob", "bob@example.test", "hello"),                  # inside a longer word, in the subject
                ("a:3", "INBOX", "Invoice", "Carl", "info@brightconsultantsusa.example", "pay"),                            # inside the sender's address
                ("a:4", "INBOX", "Lunch", "Consultant Dora", "dora@example.test", "noon"),                              # in the sender's name
                ("a:5", "Junk", "consultants", "Eve", "eve@example.test", "consultant"),                                 # left out: Junk
                ("a:6", "INBOX", "Plain", "Finn", "finn@example.test", "nothing here"),
                ("a:7", "INBOX", CANARY, "Gus", "gus@example.test", "CONSULTANT")]                         # folded: upper case
        db.executemany("insert into message values (?,?,?,?,?,?)", rows)
        db.commit()
        db.close()
        before = open(os.path.join(src, DB_NAME), "rb").read()
        snap = snapshot(src, os.path.join(work, "dest"))
        if open(os.path.join(src, DB_NAME), "rb").read() != before:
            bad.append("the source database changed during the snapshot")
        if (os.stat(os.path.dirname(snap)).st_mode & 0o777) != 0o700:
            bad.append("the snapshot folder is not mode 0700")
        ids, total, folders, left_out = expected_ids(snap, "consultant")
        lines.append("expected ids %s of %d mails, left out %d" % (ids, total, left_out))
        if ids != ["a:1", "a:2", "a:3", "a:4", "a:7"] or total != 7 or left_out != 1 or folders != {"INBOX": 6, "Junk": 1}:
            bad.append("expected_ids wrong: %r %r %r %r" % (ids, total, folders, left_out))
        if expected_ids(snap, "zzqxv")[0] != []:
            bad.append("a query nothing holds expected ids")
        # the judge
        want = ids
        hits = [(i, "subject") for i in want] + [("x:9", "related"), ("x:10", "meaning")]
        rws = [{"id": i, "hit_id": i, "part": "match", "evidence_in": "text", "evidence_bold": ["consultant"]} for i in want] + [{"id": "x:9", "hit_id": "x:9", "part": "related"}]
        head = {"matches": len(want), "related": 1}
        rdr = {"sel0": "a:1", "rdr0": "a:1", "sel1": "a:2", "rdr1": "a:2", "surface": "mac-reader", "rdr2": "a:2", "listed": 6}

        def run_judge(**kw):
            a = dict(want=want, hits=hits, rows=rws, header=head, engine_related=1, reader=rdr)
            a.update(kw)
            parts = judge(**a)
            return {k: v[0] for k, v in parts.items()}, parts

        good, parts = run_judge()
        lines += ["%s %s" % ("model ok" if v else "model BAD", k) for k, v in good.items()]
        if not all(good.values()):
            bad.append("the correct reads are not green: %r" % good)
        line, a148 = verdict(parts, want, len(hits), "ranked")
        lines += [line, a148]
        if not re.search(r"^RESULT: PASS \| exact 5 of 5", line) or a148 != "A148 RESULT: PASS":
            bad.append("the verdict line of the correct reads is %r / %r" % (line, a148))
        broken = {
            "match group": dict(hits=[(i, "subject") for i in want[:3]] + [("x:9", "related")]),                       # the prefix-only engine: letters inside words not found
            "rows": dict(rows=rws[1:]),
            "evidence": dict(rows=[dict(r, evidence_in="none") if r["id"] == "a:3" else r for r in rws]),
            "related": dict(engine_related=None),
            "reader follows": dict(reader=dict(rdr, rdr1="a:1")),
            "Enter opens it": dict(reader=dict(rdr, surface="mac-main")),
        }
        for part, kw in broken.items():
            got, parts2 = run_judge(**kw)
            lines.append("broken %-14s red=%s" % (part, not got[part]))
            if got[part]:
                bad.append("the broken read for %r stays green" % part)
            if verdict(parts2, want, len(hits), "ranked")[0].startswith("RESULT: PASS"):
                bad.append("the verdict line passes with %r broken" % part)
        if run_judge(want=[])[0]["match group"]:
            bad.append("an empty expected group passes (M >= 1 is required)")
        # the privacy guard
        protect_words(snap)
        out = say("search result for", CANARY, "and a plain fixed line is not hidden:", say("exact group expected for the query"))
        lines.append("guard: %s" % out)
        if CANARY in out or "REFUSED" not in out:
            bad.append("a line with a subject word of the copy was printed")
        FORBIDDEN.clear()
        # the lines of the run never carry text: the judge's details are counts and ids
        for part, (ok, text) in parts.items():
            if re.search(r"consultant|Quarterly|Invoice", text, re.I):
                bad.append("the detail of %r holds mail text: %r" % (part, text))
    finally:
        shutil.rmtree(work, ignore_errors=True)
    print("\n".join(lines))
    print("%d problems; the working folder is gone: %s" % (len(bad), not os.path.exists(work)))
    for b in bad:
        sys.stderr.write("SELFTEST FAIL %s\n" % b)
    return 1 if bad else 0


def main(argv):
    if "--selftest" in argv:
        return selftest()
    return run(argv[0] if argv else DEFAULT_QUERY)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
