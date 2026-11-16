#!/usr/bin/env python3
"""Counts on a PRIVATE COPY of the owner's mail database, on this Mac only. The LEAD runs it; nobody else reads the owner's data.
Items: ca7f4e9d p0 (every stored mail is in the word index and every piece has a vector), d99929d0 p6 (no raw mail-format mark is left in a stored text
or a list preview), 7e1fc235 p2 (the body of UID 143411 and the engine's line about it), 021cf2d6 p5 (Settings, Replies, Sources lists the account's folders).

Run (inside the launcher, the real engine, hidden; the copy is the launcher's own run folder), as checks/ownercopy-search.py:
    WHOOSH_RIG_HIDDEN=1 sh scripts/launch-one.sh <bundle> -- --rig-hidden sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/ownercopy-counts.py'
    python3 checks/ownercopy-counts.py --selftest      the pure rules on made-up text; launches nothing, reads nothing of the owner's

It stops the hidden copy, takes a consistent snapshot of the installed database (ownercopy-search.py `snapshot`: the installed file is opened read-only),
counts in the snapshot with SQL and local patterns, starts the same copy again on the snapshot, reads the rows of Settings, Replies, Sources, then stops the copy
and deletes the snapshot.

PRINTS COUNTS, STATES AND KINDS ONLY: never a subject, an address, a folder's own name or a word of a mail. Every line goes through ownercopy-search.py's `say`.
"""
import importlib.util
import json
import os
import re
import shutil
import sqlite3
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
UID = "143411"
LOG_WORDS = "the server sent no body for UID %s" % UID
CASE_WORDS = ("size", "literal", "fetch")        # the case the engine's line may name (item 7e1fc235)

# Raw mail-format marks a decoded text never holds (item d99929d0): a header line of a part, a boundary line, quoted-printable soft breaks and escapes.
HEADER = re.compile(r"(?mi)^(content-type|content-transfer-encoding|mime-version):\s")
BOUNDARY = re.compile(r"(?m)^--[-=_A-Za-z0-9.+]{12,}(--)?\s*$")
SOFT_BREAK = re.compile(r"=\r?\n")
QP_ESCAPE = re.compile(r"(=[0-9A-F]{2}){2,}")


def marks(text):
    """Which raw marks a text holds: a set of names. Three soft breaks, or two runs of escapes, count (one `=` at a line end is ordinary writing)."""
    out = set()
    if not text:
        return out
    if HEADER.search(text): out.add("part header")
    if BOUNDARY.search(text): out.add("boundary line")
    if len(SOFT_BREAK.findall(text)) >= 3: out.add("soft breaks")
    if len(QP_ESCAPE.findall(text)) >= 2: out.add("quoted-printable escapes")
    return out


def count_marks(rows):
    """rows: (body_text, preview). -> {where: {mark: number of mails}} and the number of mails read."""
    got = {"text": {}, "preview": {}}
    n = 0
    for body, preview in rows:
        n += 1
        for where, t in (("text", body), ("preview", preview)):
            for m in marks(t):
                got[where][m] = got[where].get(m, 0) + 1
    return got, n


def selftest():
    bad = 0
    def expect(ok, what):
        nonlocal bad
        print(("ok  " if ok else "NOT AS EXPECTED  ") + what)
        bad += 0 if ok else 1
    clean = "Hello,\n\nThe price is 12 = 3 x 4.\nSee you on Monday.\n-- \nAnna"
    expect(marks(clean) == set(), "a plain text with a signature dash line and one equals sign holds no mark")
    expect(marks("Content-Type: text/plain; charset=utf-8\n\nHello") == {"part header"}, "a part header at a line start is a mark")
    expect(marks("Hello\n--_000_AM9PR08MB1234_\nmore") == {"boundary line"}, "a boundary line is a mark")
    expect(marks("one=\ntwo=\nthree=\nfour") == {"soft breaks"}, "three soft breaks are a mark")
    expect(marks("caf=C3=A9 and d=C3=A9j=C3=A0") == {"quoted-printable escapes"}, "two runs of quoted-printable escapes are a mark")
    expect(marks("a=\nb") == set(), "one equals sign at a line end is not a mark")
    got, n = count_marks([(clean, "Hello"), ("Content-Type: text/html\n\nx", "x"), (None, "caf=C3=A9 d=C3=A9j=C3=A0")])
    expect(n == 3 and got == {"text": {"part header": 1}, "preview": {"quoted-printable escapes": 1}}, "the counts name the place and the mark, one mail each (%s)" % got)
    print("selftest: %d not as expected" % bad)
    return 1 if bad else 0


def main(argv):
    if "--selftest" in argv:
        return selftest()
    spec = importlib.util.spec_from_file_location("ownercopy_search", os.path.join(ROOT, "checks", "ownercopy-search.py"))
    oc = importlib.util.module_from_spec(spec); spec.loader.exec_module(oc)
    say = oc.say
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl                                              # noqa: E402
    d = os.environ.get("D") or os.environ.get("WHOOSH_SOCKET_DIR")
    if not d:
        say("NOT JUDGED: no run folder ($D): run it inside scripts/launch-one.sh"); return 1
    if not os.path.exists(os.path.join(oc.OWNER, oc.DB_NAME)):
        say("NOT JUDGED: no installed database"); return 1
    core = os.path.join(d, "core")
    say("hidden copy stopped: pid gone =", oc.stop_copy(d))
    if os.path.isdir(core):
        shutil.rmtree(core)
    path = oc.snapshot(oc.OWNER, core)
    say("private copy at", os.path.abspath(core), "(mode %o)" % (os.stat(core).st_mode & 0o777))
    oc.protect_words(path)
    db = sqlite3.connect("file:%s?mode=ro" % path, uri=True)
    try:
        one = lambda q, *a: db.execute(q, a).fetchone()[0]
        mails = one("select count(*) from message")
        with_text = one("select count(*) from message where coalesce(body_text, '') <> '' or coalesce(body_html, '') <> ''")
        manifest = dict(db.execute("select index_kind, documents from index_manifest").fetchall())
        chunks, embedded = db.execute("select count(*), coalesce(sum(embedded_at_ms is not null), 0) from chunk").fetchone()
        chunk_mails = one("select count(distinct message_id) from chunk")
        say("ca7f4e9d p0: stored mails %d (with a stored text %d) | word index documents %s | pieces %d, with a vector %d, mails that hold a piece %d | vector index documents %s"
            % (mails, with_text, manifest.get("text"), chunks, embedded, chunk_mails, manifest.get("vector")))
        say("ca7f4e9d p0 verdict: (a) = (b) %s; (c) = (d) %s; (a) > 0 %s" % ("yes" if manifest.get("text") == mails else "NO (%s against %s)" % (mails, manifest.get("text")),
            "yes" if chunks == embedded else "NO (%d of %d)" % (embedded, chunks), "yes" if mails > 0 else "NO"))
        got, n = count_marks(db.execute("select body_text, preview from message"))
        say("d99929d0 p6: mails read %d | raw marks in a stored text: %s | raw marks in a list preview: %s" % (n, json.dumps(got["text"], sort_keys=True), json.dumps(got["preview"], sort_keys=True)))
        say("d99929d0 p6 verdict: %s" % ("no raw mark in any stored text or preview" if not got["text"] and not got["preview"] else "RAW MARKS LEFT"))
        uid_rows = db.execute("select coalesce(body_text, '') <> '' or coalesce(body_html, '') <> '' from message where remote_id = ? or remote_id like ?", (UID, "%:" + UID)).fetchall()
        say("7e1fc235 p2: stored mails with UID %s: %d, of which with a stored body: %d" % (UID, len(uid_rows), sum(1 for (b,) in uid_rows if b)))
    finally:
        db.close()
    log = os.path.join(oc.OWNER, "core.log")
    if os.path.exists(log):
        lines = [l for l in open(log, errors="replace") if LOG_WORDS in l]
        cases = {w: sum(1 for l in lines if re.search(r"\b%s\b" % w, l.split(LOG_WORDS, 1)[1], re.I)) for w in CASE_WORDS}
        say("7e1fc235 p2: lines of the engine's log that say '%s': %d; of which naming a case word %s" % (LOG_WORDS, len(lines), json.dumps(cases, sort_keys=True)))
    else:
        say("7e1fc235 p2: the engine's log is not there")
    # Settings, Replies, Sources on the snapshot: kinds and states only
    p = subprocess.run(["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), "--relaunch"], env=dict(os.environ, D=d, WHOOSH_RIG_HIDDEN="1"), capture_output=True, text=True, cwd=ROOT)
    say("relaunch exit", p.returncode)
    c, t0 = None, time.time()
    while time.time() - t0 < 60:
        try:
            c = Ctl(d); c.connect(); c.uistate(); break
        except Exception:                                                # noqa: BLE001
            c = None; time.sleep(0.5)
    if c is None:
        say("021cf2d6 p5: NOT JUDGED, the hidden copy did not answer after the restart")
    else:
        time.sleep(3)
        try:
            c.shot("mac-settings-replies")
            try: c.click("mac-settings-replies.page.sources")
            except Exception as e: say("021cf2d6 p5: the Sources page click was refused:", type(e).__name__)   # noqa: BLE001
            time.sleep(1.5)
            rows = c.rows("mac-settings-replies.sources")
            kinds = {}
            for r in rows:
                k = (r.get("kind"), bool(r.get("enabled")) if "enabled" in r else None)
                kinds[k] = kinds.get(k, 0) + 1
            well_known = {str(r.get("role") or "").lower() for r in rows if r.get("kind") == "mail_folder" and r.get("enabled")}
            try: no_account = len(c.rows("mac-settings-replies.no-account"))
            except Exception: no_account = 0                                  # noqa: BLE001
            say("021cf2d6 p5: rows %d | by kind and enabled: %s | roles of the enabled mail folders: %s | no-account footnote rows: %d"
                % (len(rows), sorted((str(k), v) for k, v in kinds.items()), sorted(well_known), no_account))
            say("021cf2d6 p5: fields a row serves: %s" % sorted({k for r in rows for k in r})[:24])
        except Exception as e:                                               # noqa: BLE001
            say("021cf2d6 p5: NOT JUDGED, the rows could not be read:", type(e).__name__)
    with open(os.path.join(d, "stopped-by-command"), "w") as f:
        f.write(open(os.path.join(d, "rig.pid")).read().strip())
    oc.stop_copy(d)
    shutil.rmtree(core, ignore_errors=True)
    say("private copy deleted:", "yes" if not os.path.exists(core) else "no")
    say("DONE (the launcher stops the copy and removes the run folder)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
