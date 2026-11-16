#!/usr/bin/env python3
"""The proof runner: reads checks/proofs/g*/<item>.json (format: checks/proofs/README.md) and proves each item on the current tree.

  run.py --list                      parse and validate every file; one line per item; counts per state; exit 1 on an invalid file
  run.py --run static [--items a,b]  run the proofs of kind `static` (no app, no compile) and their broken copies; write results
  run.py --plan cargo|unit|hidden    print, for the lead's own runs, what each open proof of that kind needs (one compile at a time is the lead's)
  run.py --judge <kind> <log> [...]  judge proofs of that kind against output the lead already produced (a cargo log, a unit log, a check log)
  run.py --selftest                  the runner fails a proof whose pass line is absent, and fails a broken copy that stays green

Results: <out>/results.tsv (item, proof index, kind, verdict, log) and <out>/items.tsv (item, state, verdict). Default out: _captures/proofs/<label>.
A verdict is PASS only when the pass expression is found and the fail expression is not. NOT RUN is never a pass.
"""
import sys, os, re, json, glob, subprocess, tempfile, shutil, time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
STATES = ("prove", "gap", "owner", "outside")
KINDS = ("cargo", "unit", "hidden", "static", "lead")


def load(root=ROOT, only=None):
    """Every proof file, validated. Returns (items, problems): problems is a list of 'file: what is wrong'."""
    items, problems = [], []
    for path in sorted(glob.glob(os.path.join(root, "checks", "proofs", "g*", "*.json"))):
        rel = os.path.relpath(path, root)
        try:
            d = json.load(open(path, encoding="utf-8"))
        except Exception as e:
            problems.append("%s: not JSON (%s)" % (rel, str(e)[:80])); continue
        name = os.path.basename(path)[:-5]
        if d.get("item") != name: problems.append("%s: item is %r, the file name says %r" % (rel, d.get("item"), name))
        if d.get("state") not in STATES: problems.append("%s: state %r is not one of %s" % (rel, d.get("state"), "/".join(STATES)))
        proofs = d.get("proofs") or []
        for i, p in enumerate(proofs):
            if p.get("kind") not in KINDS: problems.append("%s: proof %d kind %r" % (rel, i, p.get("kind")))
            if not p.get("run"): problems.append("%s: proof %d has no run" % (rel, i))
            if not p.get("pass"): problems.append("%s: proof %d has no pass expression" % (rel, i))
            for key in ("pass", "fail"):
                if p.get(key):
                    try: re.compile(p[key])
                    except re.error as e: problems.append("%s: proof %d %s is not a regular expression (%s)" % (rel, i, key, e))
            if p.get("kind") == "unit":
                for t in [x.strip() for x in p["run"].split(",") if x.strip()]:
                    if not _unit_exists(root, t): problems.append("%s: proof %d names the Swift test %s, which is not under Tests/" % (rel, i, t))
            if p.get("kind") in ("static", "hidden"):
                m = re.search(r"(?:python3|sh)\s+(\S+\.(?:py|sh))", p.get("run") or "")
                if m and not os.path.exists(os.path.join(root, m.group(1))): problems.append("%s: proof %d runs %s, which does not exist" % (rel, i, m.group(1)))
        for j, m in enumerate(d.get("mutants") or []):
            f = os.path.join(root, m.get("file") or "")
            if not os.path.isfile(f): problems.append("%s: broken copy %d names %s, which does not exist" % (rel, j, m.get("file"))); continue
            n = open(f, encoding="utf-8", errors="replace").read().count(m.get("find") or "\0")
            if n != 1: problems.append("%s: broken copy %d: its find string occurs %d times in %s (it must occur once)" % (rel, j, n, m.get("file")))
            if not isinstance(m.get("proof"), int) or not (0 <= m["proof"] < len(proofs)): problems.append("%s: broken copy %d points at proof %r" % (rel, j, m.get("proof")))
            if m.get("find") == m.get("replace"): problems.append("%s: broken copy %d changes nothing" % (rel, j))
        if d.get("state") == "gap" and not d.get("gap"): problems.append("%s: state gap with no gap text" % rel)
        if d.get("state") == "owner" and not d.get("owner"): problems.append("%s: state owner with no owner text" % rel)
        if d.get("state") == "prove" and not proofs: problems.append("%s: state prove with no proof" % rel)
        d["_path"] = rel
        if only is None or name in only: items.append(d)
    return items, problems


_UNIT = {}
def _unit_exists(root, name):
    if root not in _UNIT:
        names = set()
        for p in glob.glob(os.path.join(root, "Tests", "**", "*.swift"), recursive=True):
            names.update(re.findall(r"func\s+([A-Za-z0-9_]+)\s*\(", open(p, encoding="utf-8", errors="replace").read()))
        _UNIT[root] = names
    return name in _UNIT[root]


def judge(proof, text):
    """PASS, or FAIL with the reason. `text` is the whole output of the run."""
    if proof.get("fail") and re.search(proof["fail"], text, re.M): return "FAIL", "the fail expression %r is in the output" % proof["fail"]
    if not re.search(proof["pass"], text, re.M): return "FAIL", "the pass expression %r is not in the output" % proof["pass"]
    return "PASS", ""


def run_static(proof, root, log):
    t = int(proof.get("timeout_s") or 300)
    try:
        r = subprocess.run(proof["run"], shell=True, cwd=root, capture_output=True, text=True, timeout=t)
        # a fail expression `^$` means "the command printed nothing": the runner adds no blank line of its own and no final
        # newline (with re.M, `^$` also matches after a final newline, so every such proof failed on output that was there)
        out = (r.stdout + r.stderr).rstrip("\n") + "\n[exit %d]" % r.returncode
    except subprocess.TimeoutExpired as e:
        out = ((e.stdout or b"").decode("utf-8", "replace") if isinstance(e.stdout, bytes) else (e.stdout or "")) + "\n[time limit %d s]\n" % t
        open(log, "w", encoding="utf-8").write(out); return "FAIL", "time limit %d s" % t
    open(log, "w", encoding="utf-8").write(out)
    return judge(proof, out)


def run_kind_static(items, root, out):
    os.makedirs(out, exist_ok=True)
    rows = []
    for d in items:
        for i, p in enumerate(d.get("proofs") or []):
            if p.get("kind") != "static": continue
            log = os.path.join(out, "%s-p%d.log" % (d["item"], i))
            v, why = run_static(p, root, log)
            rows.append((d["item"], "p%d" % i, "static", v, why, log))
            print("%s %s proof %d static: %s %s" % (v, d["item"], i, p["run"][:70], why), flush=True)
            if v != "PASS": continue
            for j, m in enumerate(d.get("mutants") or []):
                if m.get("proof") != i: continue
                f = os.path.join(root, m["file"]); src = open(f, encoding="utf-8").read()
                log2 = os.path.join(out, "%s-p%d-broken%d.log" % (d["item"], i, j))
                try:
                    open(f, "w", encoding="utf-8").write(src.replace(m["find"], m["replace"], 1))
                    v2, _ = run_static(p, root, log2)
                    red = v2 == "FAIL" and (not m.get("red") or re.search(m["red"], open(log2, encoding="utf-8").read(), re.M))
                finally:
                    open(f, "w", encoding="utf-8").write(src)
                verdict = "SEEN RED" if red else "NOT RED"
                rows.append((d["item"], "p%d-broken%d" % (i, j), "static", verdict, "", log2))
                print("%s %s proof %d broken copy %d" % (verdict, d["item"], i, j), flush=True)
    with open(os.path.join(out, "results.tsv"), "a", encoding="utf-8") as f:
        for r in rows: f.write("\t".join(r) + "\n")
    return rows


def plan(items, kind):
    n = 0
    for d in items:
        for i, p in enumerate(d.get("proofs") or []):
            if p.get("kind") != kind: continue
            n += 1
            core = (" core=%s" % p["core"]) if p.get("core") else ""
            print("%s\tp%d\t%s%s\t%s" % (d["item"], i, p["run"], core, p["pass"]))
    print("%d proofs of kind %s" % (n, kind), file=sys.stderr)


def judge_logs(items, kind, logs, out):
    """Judge every proof of `kind` against output the lead produced. For `unit`, each named test needs its own passed line."""
    text = "\n".join(open(l, encoding="utf-8", errors="replace").read() for l in logs)
    os.makedirs(out, exist_ok=True); rows = []
    for d in items:
        for i, p in enumerate(d.get("proofs") or []):
            if p.get("kind") != kind: continue
            if kind == "unit":
                names = [x.strip() for x in p["run"].split(",") if x.strip()]
                bad = [t for t in names if not re.search(r"Test %s\(\) passed" % re.escape(t), text)]
                red = [t for t in names if re.search(r"Test %s\(\) (failed|recorded an issue)" % re.escape(t), text)]
                v, why = ("PASS", "") if not bad and not red else ("FAIL", "no passed line for: %s" % ", ".join(bad or red))
            elif kind == "cargo":
                # only judged when the log holds this proof's own run line
                if p["run"] not in text and not re.search(p["pass"], text, re.M): v, why = "NOT RUN", "this log does not hold the run"
                else: v, why = judge(p, text)
            else:
                v, why = judge(p, text)
            rows.append((d["item"], "p%d" % i, kind, v, why, ";".join(logs)))
            print("%s %s proof %d %s: %s" % (v, d["item"], i, kind, why), flush=True)
    with open(os.path.join(out, "results.tsv"), "a", encoding="utf-8") as f:
        for r in rows: f.write("\t".join(r) + "\n")
    return rows


def selftest():
    tmp = tempfile.mkdtemp(prefix="proofs-selftest-")
    try:
        g = os.path.join(tmp, "checks", "proofs", "g0"); os.makedirs(g); os.makedirs(os.path.join(tmp, "Tests"))
        open(os.path.join(tmp, "value.txt"), "w").write("answer 42\n")
        def item(name, pas, find="answer 42", rep="answer 41"):
            json.dump({"item": name, "state": "prove", "proofs": [{"kind": "static", "run": "cat value.txt", "pass": pas, "timeout_s": 10}],
                       "mutants": [{"file": "value.txt", "find": find, "replace": rep, "proof": 0, "red": ""}]}, open(os.path.join(g, name + ".json"), "w"))
        item("aaaaaaaa", "answer 42"); item("bbbbbbbb", "answer 43"); item("cccccccc", "answer", rep="answer 42 ")
        items, problems = load(tmp)
        rows = run_kind_static(items, tmp, os.path.join(tmp, "out"))
        got = {(r[0], r[1]): r[3] for r in rows}
        want = {("aaaaaaaa", "p0"): "PASS", ("aaaaaaaa", "p0-broken0"): "SEEN RED", ("bbbbbbbb", "p0"): "FAIL", ("cccccccc", "p0"): "PASS", ("cccccccc", "p0-broken0"): "NOT RED"}
        ok = got == want and not problems and open(os.path.join(tmp, "value.txt")).read() == "answer 42\n"
        # the loader refuses a find string that occurs twice and a Swift test that does not exist
        json.dump({"item": "dddddddd", "state": "prove", "proofs": [{"kind": "unit", "run": "noSuchTest", "pass": "passed"}],
                   "mutants": [{"file": "value.txt", "find": "zz", "replace": "b", "proof": 0}]}, open(os.path.join(g, "dddddddd.json"), "w"))
        _, problems = load(tmp)
        ok = ok and any("noSuchTest" in p for p in problems) and any("occurs" in p for p in problems)
        print("selftest:", "PASS" if ok else "FAIL", got, problems)
        return 0 if ok else 1
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def main(a):
    if "--selftest" in a: return selftest()
    only = None
    if "--items" in a: only = set(a[a.index("--items") + 1].split(","))
    label = a[a.index("--label") + 1] if "--label" in a else time.strftime("run-%Y%m%d-%H%M")
    out = a[a.index("--out") + 1] if "--out" in a else os.path.join(ROOT, "_captures", "proofs", label)
    items, problems = load(ROOT, only)
    if "--list" in a:
        from collections import Counter
        c = Counter(d.get("state") for d in items); k = Counter(p.get("kind") for d in items for p in d.get("proofs") or [])
        for d in items: print("%s\t%s\t%d proofs\t%d broken copies\t%s" % (d["item"], d.get("state"), len(d.get("proofs") or []), len(d.get("mutants") or []), (d.get("title") or "")[:60]))
        print("%d files; states %s; proofs by kind %s; %d problems" % (len(items), dict(c), dict(k), len(problems)))
        for p in problems: print("PROBLEM", p)
        return 1 if problems else 0
    if "--run" in a and a[a.index("--run") + 1] == "static": run_kind_static(items, ROOT, out); return 0
    if "--plan" in a: plan(items, a[a.index("--plan") + 1]); return 0
    if "--judge" in a:
        i = a.index("--judge"); judge_logs(items, a[i + 1], [x for x in a[i + 2:] if not x.startswith("--") and os.path.exists(x)], out); return 0
    print(__doc__); return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
