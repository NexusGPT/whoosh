#!/usr/bin/env python3
"""The RELATED cut-off, measured on a PRIVATE COPY of the owner's mail database, on this Mac only (item 7c5301c6). The LEAD runs it; nobody else reads the owner's data.

Run (inside the launcher, the real engine, hidden; the copy is the launcher's own run folder):
    WHOOSH_RIG_HIDDEN=1 sh scripts/launch-one.sh <bundle> -- --rig-hidden sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/ownercopy-related.py'
    python3 checks/ownercopy-related.py --selftest     judges the counting rule on made-up reads; launches nothing, reads nothing of the owner's
    ... python3 checks/ownercopy-related.py --queries=cadeau,gift     only these queries (item ca7f4e9d): the engine's own counts of what it holds, then per
        query the stage, the word matches, the related mails and the closeness figures; no number is picked

What it does: the snapshot, the restart and the clean-up are checks/ownercopy-search.py's own (the installed database is opened read-only and copied with SQLite's backup
API into the copy's own folder, the hidden copy is started again on it, and the copy is deleted at the end). Then each query below is typed in the real search field
and the engine's own state is read: the stage, the row count, and for each row where it was found (`matched_in`) and its lift (`search/last/hit/<n>/lift`, the engine's
figure: how many deviations the mail's closeness stands above this search's mean).

PRINTS COUNTS ONLY: per query, how many mails hold the words, how many have a vector, and for each tried cut-off how many mails that are not a word match stand at or
above it. Never an id, a subject, a sender or any text of a mail. Every line goes through ownercopy-search's `say`, which refuses a line that holds a word of the copy.

THE RULE THAT PICKS THE NUMBER (written before the first run): of the tried values, the lowest one at which
  (1) at most one of the topic queries marks more mails than RELATED_MAX (the engine cuts the group at RELATED_MAX: a group that is always cut says nothing), and
  (2) at least half of the topic queries mark at least one mail (a cut-off no search reaches shows no related mail at all).
The two made-up queries (letters no mail is about) are printed beside them and do not pick the number.
Verdict line: `PICK: <value> | topic queries over the cap <k> of <n> | topic queries with at least one <k> of <n>` or `PICK: none | ...`.

SECOND RULE, written 14 Nov after the first run and before the run that applies it. The first run showed that a made-up query (letters no mail is about) marks as many
mails as a topic at every cut-off: the lift is relative to the search itself, so some mail always stands out. A CLOSENESS FLOOR is tried beside the cut-off of today
(lift >= 2.0 AND closeness >= floor). The floor is the lowest tried value at which NO made-up query marks any mail; it is adopted only when at least half of the topic
queries still mark at least one mail at that floor. Else no floor is adopted.
Verdict line: `FLOOR: <value> | made-up queries marking a mail 0 of <n> | topic queries with at least one <k> of <n>` or `FLOOR: none | ...`.
Exit 0 when a value is picked, 1 when none is or a query was not ranked, 3 when the hidden copy did not answer."""
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
_spec = importlib.util.spec_from_file_location("ownercopy_search", os.path.join(ROOT, "checks", "ownercopy-search.py"))
oc = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(oc)
say = oc.say

TOPICS = ["invoice", "meeting tomorrow", "flight booking", "contract to sign", "password reset", "delivery of the order", "facture", "rendez-vous",
          "payment received", "job application", "newsletter", "security alert", "hotel reservation", "proposal for the client", "subscription renewal", "réunion demain"]
MADE_UP = ["zzqxv kjwq", "plorbnik vexaru", "qwrtz", "blaxo meeru tink", "xkcdqz", "vrompt nuel", "oiuyt lkjhg", "zxcvbnm", "frindle wocket", "aaaa bbbb", "glorptak", "mnbvc poiuy"]
FLOORS = [0.64, 0.66, 0.68, 0.70, 0.72, 0.74]
LIFT_NOW = 2.0                      # docs/CONSTANTS.md RELATED_LIFT_MIN
CUTS = [1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0]
RELATED_MAX = 50                    # docs/CONSTANTS.md RELATED_MAX
RANK_WAIT_S = 90
NOT_EXACT = ("meaning", "related")


def counts(rows, cuts=CUTS):
    """`rows` are (matched_in, lift or None) in the engine's order. Returns {word matches, with a vector, engine related, per cut-off: mails that are not a word match
    with lift >= cut}. The engine's own related count is the rows it marked `related`."""
    exact = sum(1 for where, _ in rows if where not in NOT_EXACT)
    tail = [lift for where, lift in rows if where in NOT_EXACT and lift is not None]
    return {"matches": exact, "with_vector": sum(1 for _, lift in rows if lift is not None), "engine_related": sum(1 for where, _ in rows if where == "related"),
            "at": {c: sum(1 for lift in tail if lift >= c) for c in cuts}}


def closeness_line(close):
    """`close` are (cosine, lift) of the rows that are not word matches, closest first. Figures only: the closest, the 5th, the 10th, the lowest one at or above
    lift 2.0, and this search's mean and deviation (solved from two rows: lift = (cosine - mean) / deviation)."""
    if len(close) < 2:
        return "under two rows with a vector"
    at = lambda n: "%.3f" % close[n - 1][0] if len(close) >= n else "none"          # noqa: E731
    edge = [c for c, lift in close if lift >= 2.0]
    (c1, l1), (c2, l2) = close[0], close[-1]
    dev = (c1 - c2) / (l1 - l2) if l1 != l2 else 0.0
    mean = c1 - l1 * dev
    return "closest %s, 5th %s, 10th %s, lowest at lift 2.0 %s, mean %.3f, deviation %.3f" % (at(1), at(5), at(10), "%.3f" % min(edge) if edge else "none", mean, dev)


def floor_counts(close, floors=FLOORS, lift_min=LIFT_NOW):
    """Per floor: the rows that are not word matches with lift >= lift_min AND closeness >= floor."""
    return {f: sum(1 for c, lift in close if lift >= lift_min and c >= f) for f in floors}


def pick_floor(topic, made_up, floors=FLOORS):
    """The second rule of the module comment. `topic` and `made_up` are lists of floor_counts. Returns (value or None, {floor: (made-up queries marking, topic queries with one)})."""
    table = {f: (sum(1 for q in made_up if q[f] >= 1), sum(1 for q in topic if q[f] >= 1)) for f in floors}
    for f in floors:
        if made_up and table[f][0] == 0:
            return (f if 2 * table[f][1] >= len(topic) else None), table
    return None, table


def pick(per_topic, cuts=CUTS, cap=RELATED_MAX):
    """The rule of the module comment over the topic queries' counts. Returns (value or None, {cut: (over the cap, with at least one)})."""
    table, n = {}, len(per_topic)
    for c in cuts:
        table[c] = (sum(1 for q in per_topic if q["at"][c] > cap), sum(1 for q in per_topic if q["at"][c] >= 1))
    for c in cuts:
        over, some = table[c]
        if over <= 1 and 2 * some >= n:
            return c, table
    return None, table


def run():
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl                                              # noqa: E402
    d = os.environ.get("D") or os.environ.get("WHOOSH_SOCKET_DIR")
    if not d:
        say("NOT JUDGED: no run folder ($D): run it inside scripts/launch-one.sh")
        return 1
    if not os.path.exists(os.path.join(oc.OWNER, oc.DB_NAME)):
        say("NOT JUDGED: no installed database")
        return 1
    core = os.path.join(d, "core")
    say("hidden copy stopped: pid gone =", oc.stop_copy(d))
    if os.path.isdir(core):
        shutil.rmtree(core)
    path = oc.snapshot(oc.OWNER, core)
    say("private copy made (mode %o)" % (os.stat(core).st_mode & 0o777))
    oc.protect_words(path)
    # the words of this file's own fixed sentences are never protected (the first run refused its summary lines: a template word was also in a subject)
    oc.FORBIDDEN.difference_update(re.findall(r"[a-z]{6,}", open(os.path.abspath(__file__), encoding="utf-8").read().lower()))
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
    time.sleep(3)

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

    TIMES = {"letters": [], "enter": [], "first_list_s": [], "ranked_s": []}

    def ms(kind):
        """The app's own measure of the last key: milliseconds from the key to its frame (`measure mac-main.interaction.frame`); None when it serves none."""
        try:
            return float(c.measure("mac-main.interaction." + kind)["ms"])
        except Exception:                                                # noqa: BLE001
            return None

    def ask(query):
        """Types the query, waits for the ranked list, reads (matched_in, lift) down to the first meaning row under the lowest cut-off (the meaning rows are closest first)."""
        try:
            c.click("mac-search.open")
        except Exception:                                                # noqa: BLE001
            pass
        try:
            c.click("mac-search.bar")
        except Exception:                                                # noqa: BLE001
            pass
        c.key("⌘A")
        c.type(query)
        TIMES["letters"].append(ms("frame"))                 # the last typed letter to the frame that shows it and its suggestions
        c.key("Enter")
        TIMES["enter"].append(ms("frame"))                   # Enter to the frame that shows the first screen of the answer
        t0 = time.time()
        stage, first = None, None
        while time.time() - t0 < RANK_WAIT_S:
            stage = cv("search/last/stage")
            if first is None and cv("search/last/query") == query and stage is not None:
                first = time.time() - t0                     # the engine's first list for these words (word matches)
            if cv("search/last/query") == query and stage in oc.FINAL_STAGES:
                break
            time.sleep(0.05)
        TIMES["first_list_s"].append(first)
        TIMES["ranked_s"].append(time.time() - t0)
        n = int(cv("search/last/count") or 0)
        rows, below, close = [], 0, []
        for i in range(n):
            where, lift = cv("search/last/hit/%d/matched_in" % i), cv("search/last/hit/%d/lift" % i)
            lift = float(lift) if isinstance(lift, (int, float)) else None
            rows.append((where, lift))
            if where in NOT_EXACT and lift is not None:
                score = cv("search/last/hit/%d/score" % i)
                if isinstance(score, (int, float)):
                    close.append((float(score), lift))
            if where in NOT_EXACT:
                below = below + 1 if (lift is None or lift < CUTS[0]) else 0
                if below >= 25:                                           # 25 meaning rows in a row under the lowest cut-off: the rest are lower still
                    break
        return stage, n, rows, close, time.time() - t0

    # item ca7f4e9d ("is the embedding working ?"): the engine's own figures of what it holds, per folder, summed here (counts only)
    prog = cv("reply/progress")
    if isinstance(prog, list):
        tot = {k: sum(int(r.get(k) or 0) for r in prog if isinstance(r, dict)) for k in ("messages", "bodies", "indexed")}
        say("the engine holds: %d folders, %d messages, %d with their text stored, %d indexed for words and meaning" % (len(prog), tot["messages"], tot["bodies"], tot["indexed"]))
    else:
        say("the engine serves no reply/progress on this build")
    per_topic, unranked, floor_topic, floor_made = [], 0, [], []
    own = [a.split("=", 1)[1] for a in sys.argv[1:] if a.startswith("--queries=")]
    plan = (("asked", [q for q in own[0].split(",") if q]), ) if own else (("topic", TOPICS), ("made-up", MADE_UP))
    for kind, queries in plan:
        for qi, q in enumerate(queries):
            stage, n, rows, close, took = ask(q)
            k = counts(rows)
            if stage != "ranked":
                unranked += 1
            say("%s query %d: stage %s, list %d rows, word matches %d, engine related %d, at or above %s (%.0f s)" % (
                kind, qi + 1, stage, n, k["matches"], k["engine_related"], " ".join("%.1f:%d" % (cut, k["at"][cut]) for cut in CUTS), took))
            say("%s query %d closeness: %s" % (kind, qi + 1, closeness_line(close)))
            fc = floor_counts(close)
            say("%s query %d with the floor: %s" % (kind, qi + 1, " ".join("%.2f:%d" % (f, fc[f]) for f in FLOORS)))
            if stage == "ranked":
                (floor_topic if kind == "topic" else floor_made).append(fc)
            if kind == "topic" and stage == "ranked":
                per_topic.append(k)
    def spread(v):
        v = sorted(x for x in v if x is not None)
        return "none read" if not v else "median %.1f, worst %.1f over %d" % (v[len(v) // 2], v[-1], len(v))
    say("machine load %.1f" % os.getloadavg()[0])
    say("typed words to their frame, ms: %s" % spread(TIMES["letters"]))
    say("Enter to the frame of the first screen, ms: %s" % spread(TIMES["enter"]))
    say("Enter to the engine's first list, ms: %s" % spread([None if x is None else x * 1000 for x in TIMES["first_list_s"]]))
    say("Enter to the ranked list (the model's order), ms: %s" % spread([x * 1000 for x in TIMES["ranked_s"]]))
    if own:                                             # the asked queries only: their lines are the answer, no number is picked
        with open(os.path.join(d, "stopped-by-command"), "w") as f:
            f.write(open(os.path.join(d, "rig.pid")).read().strip())
        oc.stop_copy(d)
        shutil.rmtree(core, ignore_errors=True)
        say("private copy deleted:", "yes" if not os.path.exists(core) else "no")
        say("DONE (the launcher stops the copy and removes the run folder)")
        return 0 if not unranked else 1
    value, table = pick(per_topic) if per_topic else (None, {})
    for cut in CUTS:
        if cut in table:
            say("cut-off %.1f: topic queries over the cap of %d: %d of %d; with at least one related mail: %d of %d" % (cut, RELATED_MAX, table[cut][0], len(per_topic), table[cut][1], len(per_topic)))
    if value is None:
        line = "PICK: none | topic queries ranked %d of %d" % (len(per_topic), len(TOPICS))
    else:
        line = "PICK: %.1f | topic queries over the cap %d of %d | topic queries with at least one %d of %d" % (value, table[value][0], len(per_topic), table[value][1], len(per_topic))
    say(line)
    fvalue, ftable = pick_floor(floor_topic, floor_made)
    for f in FLOORS:
        say("floor %.2f: made-up queries marking a mail: %d of %d; topic queries with at least one: %d of %d" % (f, ftable[f][0], len(floor_made), ftable[f][1], len(floor_topic)))
    if fvalue is None:
        say("FLOOR: none | made-up queries ranked %d | topic queries ranked %d" % (len(floor_made), len(floor_topic)))
    else:
        say("FLOOR: %.2f | made-up queries marking a mail 0 of %d | topic queries with at least one %d of %d" % (fvalue, len(floor_made), ftable[fvalue][1], len(floor_topic)))
    with open(os.path.join(d, "stopped-by-command"), "w") as f:
        f.write(open(os.path.join(d, "rig.pid")).read().strip())
    oc.stop_copy(d)
    shutil.rmtree(core, ignore_errors=True)
    say("private copy deleted:", "yes" if not os.path.exists(core) else "no")
    say("DONE (the launcher stops the copy and removes the run folder)")
    return 0 if value is not None and not unranked else 1


def selftest():
    """The counting rule and the picking rule on made-up reads; each rule is also run broken and must change its answer."""
    bad = []
    rows = [("subject", 3.2), ("text", None), ("related", 4.1), ("related", 2.6), ("meaning", 1.7), ("meaning", 1.2), ("meaning", 0.4), ("meaning", None)]
    k = counts(rows)
    want = {"matches": 2, "with_vector": 6, "engine_related": 2, "at": {1.0: 4, 1.5: 3, 2.0: 2, 2.5: 2, 3.0: 1, 3.5: 1, 4.0: 1, 5.0: 0}}
    print("%s counts: %r" % ("ok " if k == want else "BAD", k))
    if k != want:
        bad.append("counts")
    # a word match with a high lift is never counted as related
    if counts([("subject", 9.0)])["at"][1.0] != 0:
        bad.append("a word match counted as related")
    line = closeness_line([(0.80, 3.0), (0.74, 2.0), (0.70, 1.3333), (0.68, 1.0), (0.62, 0.0)])
    want_line = "closest 0.800, 5th 0.620, 10th none, lowest at lift 2.0 0.740, mean 0.620, deviation 0.060"
    print("%s closeness line: %s" % ("ok " if line == want_line else "BAD", line))
    if line != want_line:
        bad.append("closeness line")
    fc = floor_counts([(0.75, 3.1), (0.71, 2.2), (0.69, 2.0), (0.69, 1.9), (0.60, 2.5)])
    want_fc = {0.64: 3, 0.66: 3, 0.68: 3, 0.70: 2, 0.72: 1, 0.74: 1}
    print("%s floor counts: %r" % ("ok " if fc == want_fc else "BAD", fc))
    if fc != want_fc:
        bad.append("floor counts")
    fl = lambda *v: dict(zip(FLOORS, v))                                 # noqa: E731
    fv, ft = pick_floor([fl(9, 8, 6, 4, 2, 0), fl(5, 5, 3, 1, 0, 0), fl(2, 1, 0, 0, 0, 0), fl(7, 7, 7, 3, 1, 1)], [fl(4, 2, 1, 0, 0, 0), fl(3, 1, 0, 0, 0, 0)])
    print("%s floor pick: %r %r" % ("ok " if fv == 0.70 and ft[0.70] == (0, 3) else "BAD", fv, ft.get(0.70)))
    if fv != 0.70 or ft[0.70] != (0, 3):
        bad.append("floor pick")
    if pick_floor([fl(1, 0, 0, 0, 0, 0)] * 4, [fl(4, 2, 1, 0, 0, 0)])[0] is not None:
        bad.append("a floor that silences the topics too was adopted")
    mk = lambda *v: {"at": dict(zip(CUTS, v))}                          # noqa: E731
    topics = [mk(900, 300, 120, 40, 9, 2, 0, 0), mk(700, 200, 80, 20, 4, 0, 0, 0), mk(60, 30, 10, 2, 0, 0, 0, 0), mk(400, 90, 60, 51, 12, 1, 0, 0)]
    value, table = pick(topics)
    print("%s pick: %r %r" % ("ok " if value == 2.5 else "BAD", value, table.get(2.5)))
    if value != 2.5 or table[2.5] != (1, 4):
        bad.append("pick")
    if pick([mk(900, 900, 900, 900, 900, 900, 900, 900)] * 3)[0] is not None:
        bad.append("a group always over the cap picked a value")
    if pick([mk(0, 0, 0, 0, 0, 0, 0, 0)] * 3)[0] is not None:
        bad.append("a cut-off no search reaches picked a value")
    # the broken copies of the two rules: each must change the answer above
    broken_counts = sum(1 for where, lift in rows if lift is not None and lift >= 2.0)          # forgets "not a word match"
    print("%s broken copy `word matches counted`: %d (the rule says %d)" % ("red" if broken_counts != want["at"][2.0] else "STILL GREEN", broken_counts, want["at"][2.0]))
    if broken_counts == want["at"][2.0]:
        bad.append("mutant word-matches-counted not red")
    first_under_cap = next((c for c in CUTS if sum(1 for q in topics if q["at"][c] > RELATED_MAX) == 0), None)   # "no query over the cap" instead of "at most one"
    print("%s broken copy `no query over the cap`: %r (the rule says 2.5)" % ("red" if first_under_cap != 2.5 else "STILL GREEN", first_under_cap))
    if first_under_cap == 2.5:
        bad.append("mutant no-query-over-the-cap not red")
    print("%d checks, %d failed, 0 not judged" % (11, len(bad)))
    for b in bad:
        print("FAIL", b)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else run())
