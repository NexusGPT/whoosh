#!/usr/bin/env python3
"""textjudge-sanity: can the on-device LANGUAGE model judge the labels the decision model (Laya) cannot? (the owner's "AI filter seeing 0 messages";
hid-ai round 10). The lead's laya-sanity on main100: 0 of 12 obvious-yes reach 0.80 with any wording, and for 3 of 4 labels an obvious no scores
above an obvious yes. This check asks the TEXT backends the same question with the SAME 12 mails, 4 labels and markings (imported from
checks/laya-sanity.py, never copied): per mail, the label flow's own state (decision/src/adapters/wire.rs `state_text`: from, to, cc, subject,
list header, a blank line, the body cut at DECISION_BODY_CHARS), the labels with their descriptions, and the instruction to answer yes or no for
each label. Two forms: all labels in one request ("all"), one request per label ("per-label"). Apple Intelligence answers through guided
generation (a typed yes/no per label); the MLX language model answers in text, read strictly (Sources/Features/OnDeviceAI/TextJudgeFixture.swift
`parseAll`, `parseOne`): an unreadable answer is a FAILURE, never a "no".

It launches the HIDDEN copy (scripts/launch-one.sh --rig-hidden, fixture core: no mail of the owner's) and, inside, links the MLX model folder
(--mlx <a copy of the owner's Models/mlx-language-model>, optional) where the rig copy looks for it (<run>/Models/mlx-language-model), writes the
cases file and sends `fixture textjudge <cases.tsv> <out.json>` (rig only; the copy's TextJudgeRun asks each backend it has, nothing leaves this
Mac). While the run lasts it samples the copy's physical footprint every SAMPLE_S (perf/footprint.py `rusage`, what Activity Monitor shows).

Printed per backend and form: the yes/no table (marks Y n ?), obvious-yes right of N, obvious-no right of M, mails with more than one yes, median
and worst time per mail; the copy's memory peak over the run. Judged: per backend and form, every obvious-yes is yes and every obvious-no is no;
once, the peak under RSS_MODEL_MAX_MB (docs/CONSTANTS.md). A backend this copy does not have (Apple Intelligence off or not eligible, the MLX folder
not given) is NOT JUDGED with its own words.

    python3 checks/textjudge-sanity.py <bundle> [--mlx <folder>]     one hidden launch (the lead)
    python3 checks/textjudge-sanity.py --selftest                     launches nothing
"""
import importlib.util
import json
import os
import re
import statistics
import subprocess
import sys
import tempfile
import threading
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WAIT_S = 1100        # the check's own bound for the inner step (no product meaning): 12 mails x (1 + 4) requests x 2 backends
SAMPLE_S = 0.5       # how often the copy's footprint is read during the run
START_S = 60         # round 11: a run that has not written `started` this long after the verb is a failure, never a wait to the end
FORMS = ["all", "per-label"]
BACKENDS = ["apple", "mlx"]
MLX_FOLDER = "mlx-language-model"     # MlxModelSpec.folderName's default (Sources/Features/OnDeviceAI/Core/MlxModelSpec.swift)


def _load(name, rel):
    spec = importlib.util.spec_from_file_location(name, os.path.join(ROOT, rel))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


LS = _load("laya_sanity", "checks/laya-sanity.py")          # the 12 mails, the 4 labels, the markings: one source
LABELS, MAILS = LS.LABELS, LS.MAILS


def constant(name):
    """A number of docs/CONSTANTS.md (the ledger), by its row name."""
    for line in open(os.path.join(ROOT, "docs", "CONSTANTS.md"), encoding="utf-8"):
        if line.strip().startswith("| %s |" % name):
            return float(re.findall(r"[0-9.]+", line.split("|")[2])[0])
    raise SystemExit("docs/CONSTANTS.md has no %s row" % name)


def state_text(sender, subject, body, list_header, body_chars):
    """The label flow's own state as one text (decision/src/adapters/wire.rs `state_text`; decide::state_for: from, to, cc, subject when read, list header,
    the body cut at DECISION_BODY_CHARS). The synthetic mails have no to and no cc, so those lines are empty, as the engine writes them."""
    return "from: %s\nto: \ncc: \nsubject: %s\nlist header: %s\n\n%s" % (sender, subject, "yes" if list_header else "no", body[:body_chars])


def cases_tsv(body_chars):
    """The cases file TextJudgeRun reads: `label <id> <name> <description>`, `mail <id> <state text>`, tabs and new lines escaped."""
    lines = ["\t".join(["label", i, LS.esc(n), LS.esc(d)]) for i, n, d in LABELS]
    lines += ["\t".join(["mail", m, LS.esc(state_text(f, s, b, lst, body_chars))]) for m, f, s, b, lst, _ in MAILS]
    return "\n".join(lines) + "\n"


def read_cases(text):
    """The reader's side (TextJudgeRunner.read splits on tabs and unescapes the same way), for the selftest."""
    un = lambda v: v.replace("\\n", "\n").replace("\\t", "\t")
    recs = [[un(x) for x in l.split("\t")] for l in text.splitlines() if l.strip()]
    return [r for r in recs if r[0] == "label"], [r for r in recs if r[0] == "mail"]


def rows_of(table, backend, form):
    """{mail: row} of one backend and form; None when the backend is absent from the table, {} when it ran no such form."""
    for b in table.get("backends") or []:
        if b.get("backend") == backend:
            return {r.get("mail"): r for r in ((b.get("forms") or {}).get(form) or [])}
    return None


def backend_entry(table, backend):
    return next((b for b in table.get("backends") or [] if b.get("backend") == backend), None)


def counts(rows):
    """(obvious-yes right, obvious-yes total, obvious-no right, obvious-no total, mails with more than one yes, unreadable answers, [ms])."""
    yr = yt = nr = nt = multi = bad = 0
    ms = []
    for m, *_r, marks in MAILS:
        r = rows.get(m) or {}
        ans, unread = r.get("answers") or {}, set(r.get("unreadable") or [])
        if isinstance(r.get("ms"), (int, float)):
            ms.append(r["ms"])
        if sum(1 for v in ans.values() if v == "yes") > 1:
            multi += 1
        for i, mk in marks.items():
            a = None if i in unread else ans.get(i)
            bad += a is None
            if mk == "yes":
                yt += 1; yr += a == "yes"
            elif mk == "no":
                nt += 1; nr += a == "no"
    return yr, yt, nr, nt, multi, bad, ms


def judge(table, peak_mb, limit_mb):
    """[(case, ok, detail)]: per backend and form the two rules; once the memory peak. ok None = not judged."""
    out = []
    if table.get("error"):
        out.append(("the-run-reached-the-copy", False, table["error"]))
    for backend in BACKENDS:
        e = backend_entry(table, backend)
        for form in FORMS:
            name = "%s/%s" % (backend, form)
            if e is None:
                out += [(name + "/every-obvious-yes-is-yes", None, "the copy's table has no %s backend (the run did not reach it)" % backend),
                        (name + "/every-obvious-no-is-no", None, "the copy's table has no %s backend" % backend)]
                continue
            if not e.get("available"):
                why = "%s is not available on this copy: %s (the system's own value: %s)" % (backend, e.get("words"), e.get("reason"))
                out += [(name + "/every-obvious-yes-is-yes", None, why), (name + "/every-obvious-no-is-no", None, why)]
                continue
            rows = rows_of(table, backend, form) or {}
            yr, yt, nr, nt, multi, bad, ms = counts(rows)
            answered = len(rows) == len(MAILS)
            tail = "" if answered else " (only %d of %d mails answered: the run did not end)" % (len(rows), len(MAILS))
            out.append((name + "/every-obvious-yes-is-yes", answered and yr == yt, "%d of %d obvious-yes answered yes; %d unreadable answers in all%s" % (yr, yt, bad, tail)))
            out.append((name + "/every-obvious-no-is-no", answered and nr == nt, "%d of %d obvious-no answered no%s" % (nr, nt, tail)))
    if peak_mb is None:
        out.append(("memory-peak-under-RSS_MODEL_MAX_MB", None, "the copy's footprint was not read (no pid found)"))
    else:
        out.append(("memory-peak-under-RSS_MODEL_MAX_MB", peak_mb < limit_mb, "peak footprint %.0f MB (limit %.0f MB)" % (peak_mb, limit_mb)))
    return out


def table_lines(table):
    lines = []
    for backend in BACKENDS:
        e = backend_entry(table, backend)
        if e is None:
            lines.append("BACKEND %s: absent from the table" % backend); continue
        if not e.get("available"):
            lines.append("BACKEND %s (%s): not available: %s | the system's own value: %s" % (backend, e.get("name"), e.get("words"), e.get("reason"))); continue
        for form in FORMS:
            rows = rows_of(table, backend, form) or {}
            yr, yt, nr, nt, multi, bad, ms = counts(rows)
            guided = any(r.get("guided") for r in rows.values())
            lines.append("BACKEND %s (%s), form %s, %s: obvious-yes right %d of %d, obvious-no right %d of %d, mails with more than one yes %d, unreadable %d, time per mail median %s ms, worst %s ms" % (
                backend, e.get("name"), form, "guided generation" if guided else "text read strictly", yr, yt, nr, nt, multi, bad,
                "%.0f" % statistics.median(ms) if ms else "-", "%.0f" % max(ms) if ms else "-"))
            lines.append("  mail  " + "  ".join("%-12s" % i for i, *_ in LABELS))
            for m, _f, subject, _b, _l, marks in MAILS:
                r = rows.get(m) or {}
                ans, unread = r.get("answers") or {}, set(r.get("unreadable") or [])
                cells = ["%-12s" % ("%s %s" % ({"yes": "Y", "no": "n", "unclear": "?"}[marks[i]], "UNREADABLE" if i in unread else (ans.get(i) or "-"))) for i, *_ in LABELS]
                err = ("  error: " + str(r["error"])[:160]) if r.get("error") else ""
                lines.append("  %s   %s  %s%s" % (m, "  ".join(cells), subject, err))
    return lines


def report(answer, limit_mb):
    table, peak = answer.get("table") or {}, answer.get("peak_mb")
    lines = ["TEXTJUDGE-SANITY: 12 synthetic mails x 4 labels (checks/laya-sanity.py), the label flow's state; %s s in the inner step; memory peak %s MB over %d readings" % (
        answer.get("seconds"), "%.0f" % peak if peak is not None else "-", answer.get("samples") or 0)]
    results = judge(table, peak, limit_mb)
    lines += ["%-10s textjudge-sanity/%s  %s" % ("PASS" if ok else ("NOT JUDGED" if ok is None else "FAIL"), n, d) for n, ok, d in results]
    lines += table_lines(table)
    failed, unjudged = sum(1 for _, ok, _ in results if ok is False), sum(1 for _, ok, _ in results if ok is None)
    lines.append("%d checks, %d failed, %d not judged" % (len(results), failed, unjudged))
    return "\n".join(lines), failed


def read_json(path):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


def wait_for(read, where, clock=time.time, pause=time.sleep):
    """The table once it says done. No `started` within START_S is a failure at once (round 11: main101 waited 11 minutes on nothing); no `done`
    within WAIT_S ends the wait with the partial table and the reason."""
    t0, table = clock(), {}
    while clock() - t0 < WAIT_S:
        table = read()
        if table.get("done"):
            return table
        if not table.get("started") and clock() - t0 > START_S:
            return {"done": True, "error": "the copy wrote no `started` in %d s after `fixture textjudge` (file %s): the run never began" % (START_S, where)}
        pause(1)
    return dict(table, done=True, error="the run did not end within %d s (the partial table is kept)" % WAIT_S)


def inner(folder, mlx):
    """Inside launch-one: link the MLX folder, hand the copy its cases, sample its footprint until the table says done."""
    run = os.environ.get("WHOOSH_SOCKET_DIR") or ""
    if mlx != "-":
        dest = os.path.join(run, "Models", MLX_FOLDER)
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        if not os.path.exists(dest):
            os.symlink(mlx, dest)
    # round 11: the files live in the copy's own run folder, which the copy writes (its socket is there); main101's folder under $TMPDIR may be
    # out of a sandboxed copy's reach, and the copy said nothing
    work = os.path.join(run, "textjudge")
    os.makedirs(work, exist_ok=True)
    cases, out = os.path.join(work, "cases.tsv"), os.path.join(work, "table.json")
    with open(cases, "w", encoding="utf-8") as f:
        f.write(cases_tsv(int(constant("DECISION_BODY_CHARS"))))
    fp = _load("perf_footprint", "perf/footprint.py")
    pids = fp.pids_of(prefix=os.path.join(run, "rig"))
    samples, stop = [], threading.Event()

    def sample():
        while not stop.is_set():
            for p in pids:
                r = fp.rusage(p)
                if r:
                    samples.append(r[1])
            stop.wait(SAMPLE_S)
    t = threading.Thread(target=sample, daemon=True)
    t.start()
    sys.path.insert(0, os.path.join(ROOT, "journeys"))
    from lib.ctl import Ctl      # noqa: E402
    table, t0 = {}, time.time()
    try:
        ctl = Ctl().connect()
        ctl.send("fixture", "textjudge", cases, out)
    except Exception as e:      # noqa: BLE001  the verb's refusal is the answer, said in the table
        table = {"done": True, "error": "fixture textjudge: %s" % e}
    if not table.get("done"):
        table = wait_for(lambda: read_json(out), out)
    try:
        import shutil
        shutil.copy(out, os.path.join(folder, "table.json"))      # the table beside the check's other files, for reading after the run
    except OSError:
        pass
    stop.set(); t.join(2)
    print("ANSWER " + json.dumps({"table": table, "peak_mb": max(samples) if samples else None, "samples": len(samples), "pids": pids,
                                  "seconds": round(time.time() - t0, 1)}))
    return 0


def main_run(bundle, mlx):
    if mlx and not os.path.isdir(mlx):
        print("--mlx must be a copy of the owner's Models/%s folder: %s" % (MLX_FOLDER, mlx)); print("1 checks, 0 failed, 1 not judged"); return 1
    folder = tempfile.mkdtemp(prefix="textjudge-sanity-")
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1")
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden", "--fixture-core",
           "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner "$1" "$2"', os.path.abspath(__file__), folder, os.path.abspath(mlx) if mlx else "-"]
    t0 = time.time()
    r = subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True, text=True)
    out = r.stdout + r.stderr
    line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
    if line is None:
        print(out.rstrip()[-3000:]); print("launcher exit %d" % r.returncode); print("1 checks, 0 failed, 1 not judged"); return 1
    text, failed = report(json.loads(line[len("ANSWER "):]), constant("RSS_MODEL_MAX_MB"))
    print("launcher exit %d; run folder %s; %.0f s in all" % (r.returncode, folder, time.time() - t0))
    print(text)
    return 0 if failed == 0 else 1


# ---------------------------------------------------------------- self-test: launches nothing
def model_table(yes_for_yes=True, no_for_no=True, unreadable=None, apple=True, mlx=True, unclear="no", stop_at=None):
    """A table as TextJudgeRun writes it. `unreadable` = (mail, label) answered unreadably; `stop_at` = the number of mails answered."""
    def rows():
        out = []
        for m, *_r, marks in MAILS[:stop_at]:
            ans = {i: ("yes" if yes_for_yes else "no") if mk == "yes" else ("no" if no_for_no else "yes") if mk == "no" else unclear for i, mk in marks.items()}
            un = []
            if unreadable and unreadable[0] == m:
                ans.pop(unreadable[1]); un = [unreadable[1]]
            out.append({"mail": m, "answers": ans, "unreadable": un, "ms": 400, "guided": True})
        return out
    b = [{"backend": "apple", "name": "Apple Intelligence", "available": apple, "words": "ready" if apple else "Apple Intelligence is off on this Mac.",
          **({"forms": {f: rows() for f in FORMS}} if apple else {})},
         {"backend": "mlx", "name": "Language model", "available": mlx, "words": "ready" if mlx else "On-device model not installed yet.",
          **({"forms": {f: rows() for f in FORMS}} if mlx else {})}]
    return {"done": True, "backends": b}


def selftest():
    limit = 2200.0
    verdict = lambda t, peak=900.0: {n: ok for n, ok, _ in judge(t, peak, limit)}
    good = verdict(model_table())
    ok_good = all(v is True for v in good.values())
    ok_yes = verdict(model_table(yes_for_yes=False))["apple/all/every-obvious-yes-is-yes"] is False      # broken copy: a judge that says no to everything
    ok_no = verdict(model_table(no_for_no=False))["mlx/per-label/every-obvious-no-is-no"] is False       # broken copy: a judge that says yes to everything
    # an unreadable answer on an obvious NO is a failure, never read as the "no" it would have been
    ok_unread = verdict(model_table(unreadable=("m02", "needs-reply")))["apple/all/every-obvious-no-is-no"] is False
    ok_peak = verdict(model_table(), 2600.0)["memory-peak-under-RSS_MODEL_MAX_MB"] is False and verdict(model_table(), None)["memory-peak-under-RSS_MODEL_MAX_MB"] is None
    off = verdict(model_table(apple=False))
    ok_off = off["apple/all/every-obvious-yes-is-yes"] is None and off["mlx/all/every-obvious-yes-is-yes"] is True
    ok_half = verdict(model_table(stop_at=6))["apple/all/every-obvious-yes-is-yes"] is False
    ok_err = verdict({"done": True, "error": "fixture textjudge is not served"})["the-run-reached-the-copy"] is False
    tick = iter(range(0, 10 ** 6, 5))
    never = wait_for(lambda: {}, "x", clock=lambda: next(tick), pause=lambda s: None)
    ok_start = never.get("done") is True and "no `started`" in never.get("error", "")
    seq = iter([{"done": False, "started": True}] * 3 + [{"done": True, "started": True, "backends": []}])
    finished = wait_for(lambda: next(seq), "x", clock=lambda: 0, pause=lambda s: None)
    ok_start = ok_start and finished.get("done") is True and "error" not in finished
    off_lines = "\n".join(table_lines(dict(model_table(apple=False), backends=[dict(model_table(apple=False)["backends"][0], reason="unavailable(.appleIntelligenceNotEnabled)"), model_table()["backends"][1]])))
    ok_reason = "unavailable(.appleIntelligenceNotEnabled)" in off_lines
    c = counts(rows_of(model_table(unclear="yes"), "apple", "all"))
    ok_multi = c[4] == sum(1 for *_r, mk in MAILS if "unclear" in mk.values())     # the unclear "yes" makes those mails carry two yes answers
    labels, mails = read_cases(cases_tsv(2000))
    st = mails[0][2]
    ok_cases = [l[1] for l in labels] == [i for i, *_ in LABELS] and len(mails) == 12 and st.startswith("from: anna.berg@client-example.com\nto: \ncc: \nsubject: Can you confirm")
    ok_cut = len(state_text("a", "b", "x" * 3000, False, 2000).split("\n\n", 1)[1]) == 2000
    ok_same = LABELS is LS.LABELS and MAILS is LS.MAILS and len(MAILS) == 12 and len(LABELS) == 4
    text, failed = report({"table": model_table(), "peak_mb": 900.0, "samples": 10, "seconds": 1}, limit)
    ok_report = failed == 0 and "obvious-yes right 12 of 12" in text and "BACKEND mlx" in text
    ok_const = constant("RSS_MODEL_MAX_MB") > 0 and constant("DECISION_BODY_CHARS") > 0
    parts = [("a good table", ok_good), ("broken: always no", ok_yes), ("broken: always yes", ok_no), ("an unreadable answer is a failure", ok_unread),
             ("the memory peak", ok_peak), ("a backend that is off is not judged", ok_off), ("a run that stopped", ok_half), ("an error", ok_err), ("the system's reason printed", ok_reason), ("a run that never starts fails in 60 s", ok_start),
             ("mails with more than one yes", ok_multi), ("the cases file", ok_cases), ("the body cut", ok_cut), ("the cases are laya-sanity's", ok_same),
             ("the report", ok_report), ("the ledger", ok_const)]
    print("selftest textjudge-sanity: " + ", ".join("%s %s" % (n, "ok" if ok else "WRONG") for n, ok in parts))
    return 0 if all(ok for _, ok in parts) else 1


def main():
    if "--selftest" in sys.argv:
        return selftest()
    if "--inner" in sys.argv:
        i = sys.argv.index("--inner")
        return inner(sys.argv[i + 1], sys.argv[i + 2])
    if len(sys.argv) < 2 or not os.path.isdir(sys.argv[1]):
        print(__doc__); return 2
    mlx = sys.argv[sys.argv.index("--mlx") + 1] if "--mlx" in sys.argv else None
    return main_run(os.path.abspath(sys.argv[1]), mlx)


if __name__ == "__main__":
    sys.exit(main())
