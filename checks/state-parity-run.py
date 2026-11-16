#!/usr/bin/env python3
"""state-parity-run (tests-r58 round 5): the five runs the lead asked for on core/crates/engine/tests/state_parity.rs, each with its file edit made and undone.

    python3 checks/state-parity-run.py --selftest                 the edits only, on temp copies: each anchor found once, each edit differs, undo is byte-identical
    python3 checks/state-parity-run.py --plan                     what would run, and the preflight numbers (nothing runs)
    python3 checks/state-parity-run.py --go --target <dir>        RUN (only after the lead announces an installed build): preflight 8 GB free on /System/Volumes/Data and
                                                                  load under 30, `nice -n 19`, CARGO_BUILD_JOBS=4, one target dir, deleted at the end

Runs, each printing the `test result` line and the first finding line (cargo's own last line is always "error: test failed") and whether it is the expected one:
  clean        no edit                                        expected `test result: ok` (3 tests)
  P8           the row loop `continue`s first                 expected red: "but only 0 concrete paths were asked"
  gap-control  a scratch file with ONE `gap` row              expected `test result: ok`  (the control for P9: a declared gap is asked and stays unanswered)
  P9           that file, and `gap_paths_seen += 1` deleted   expected red: "only 0 gap path(s) were put to the core"
  P10          the `inbox/tabs` row's kind set to null        expected red in words: "UNTYPED inbox/tabs: add its kind", and no `unwrap` / `None value` panic (the assert's own panic line is how cargo shows it)
"""
import json
import os
import re
import shutil
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TEST = os.path.join(ROOT, "core", "crates", "engine", "tests", "state_parity.rs")
FILE = os.path.join(ROOT, "docs", "STATE-PATHS.json")
LOG = os.path.join(ROOT, "_captures", "tests-r58")


def p8(src):
    old = "    for row in rows {\n        if let Some(why) = row[\"rig_only\"].as_str()"
    assert src.count(old) == 1, "P8 anchor"
    return src.replace(old, "    for row in rows {\n        continue;\n        if let Some(why) = row[\"rig_only\"].as_str()", 1)


def p9(src):
    old = "                gap_paths_seen += 1;\n"
    assert src.count(old) == 1, "P9 anchor"
    return src.replace(old, "", 1)


GAP_ROW = {"path": "zz/<message>/gap-control", "kind": "string", "gap": "tests-r58 control: a declared gap the real core must leave unanswered", "read_by": []}


def with_gap(text):
    d = json.loads(text)
    d["paths"].append(GAP_ROW)
    return json.dumps(d, indent=1, ensure_ascii=False) + "\n"


def untyped(text):
    d = json.loads(text)
    row = next(r for r in d["paths"] if r["path"] == "inbox/tabs")
    row["kind"] = None
    return json.dumps(d, indent=1, ensure_ascii=False) + "\n"


# name: (edit of the test source, edit of the file, a regexp the expected output matches, the output must contain "test result: ok" / FAILED)
RUNS = [("clean", None, None, r"test result: ok\. 3 passed", True),
        ("P8", p8, None, r"but only 0 concrete paths were asked", False),
        ("gap-control", None, with_gap, r"test result: ok\. 3 passed", True),
        ("P9", p9, with_gap, r"only 0 gap path\(s\) were put to the core", False),
        ("P10", None, untyped, r"UNTYPED inbox/tabs: add its kind", False)]


FINDING = re.compile(r"^(UNTYPED|KIND|EMPTY|SERVED|NOT JSON|NOVALUES|RIG-ONLY)\b|rows \(of which|the file declares \d+ gap")


def preflight():
    free = int(subprocess.run(["df", "-g", "/System/Volumes/Data"], capture_output=True, text=True).stdout.splitlines()[1].split()[3])
    load = os.getloadavg()[0]
    return free, load, free >= 8 and load < 30


def run_all(target):
    ok_all = True
    src0, file0 = open(TEST, encoding="utf-8").read(), open(FILE, encoding="utf-8").read()
    os.makedirs(LOG, exist_ok=True)
    try:
        for name, edit_src, edit_file, expect, want_ok in RUNS:
            open(TEST, "w", encoding="utf-8").write(edit_src(src0) if edit_src else src0)
            open(FILE, "w", encoding="utf-8").write(edit_file(file0) if edit_file else file0)
            env = dict(os.environ, CARGO_BUILD_JOBS="4", CARGO_INCREMENTAL="0", CARGO_TARGET_DIR=target)
            p = subprocess.run(["nice", "-n", "19", os.path.expanduser("~/.cargo/bin/cargo"), "test", "-p", "engine", "--test", "state_parity", "--offline", "--", "--nocapture"],
                               cwd=os.path.join(ROOT, "core"), env=env, capture_output=True, text=True)
            out = p.stdout + p.stderr
            open(os.path.join(LOG, "state-parity-%s.log" % name), "w", encoding="utf-8").write(out)
            result = next((l.strip() for l in out.splitlines() if l.startswith("test result:")), "(no `test result` line: it did not compile or run)")
            finding = next((l.strip() for l in out.splitlines() if FINDING.search(l.strip())), "")
            # an assert that fails in words is still a panic line in cargo's output: P10 is right when its sentence is there and the panic is not the old unwrap
            matched = re.search(expect, out) is not None and (not re.search(r"unwrap\(\)|None value", out) if name == "P10" else True)
            ok = matched and ((p.returncode == 0) == want_ok)
            ok_all &= ok
            print("%-12s %s | %s | %s" % (name, "AS EXPECTED" if ok else "NOT AS EXPECTED", result[:110], finding[:170] or "no finding line"))
    finally:
        open(TEST, "w", encoding="utf-8").write(src0)
        open(FILE, "w", encoding="utf-8").write(file0)
        if os.path.isdir(target) and "state-parity" in os.path.basename(target.rstrip("/")):
            shutil.rmtree(target, ignore_errors=True)
    return ok_all


def selftest():
    src0, file0 = open(TEST, encoding="utf-8").read(), open(FILE, encoding="utf-8").read()
    bad = 0
    def case(n, ok):
        nonlocal bad
        bad += not ok
        print("%-60s %s" % (n, "ok" if ok else "FAIL"))
    case("P8 edit changes the source and finds its anchor once", p8(src0) != src0)
    case("P9 edit changes the source and finds its anchor once", p9(src0) != src0)
    case("gap edit adds exactly one gap row", sum(1 for r in json.loads(with_gap(file0))["paths"] if "gap" in r) == sum(1 for r in json.loads(file0)["paths"] if "gap" in r) + 1)
    case("untyped edit nulls exactly the inbox/tabs kind", [r["path"] for r in json.loads(untyped(file0))["paths"] if r["kind"] is None] == ["inbox/tabs"])
    case("the original is left byte-identical (no write in the selftest)", open(TEST, encoding="utf-8").read() == src0 and open(FILE, encoding="utf-8").read() == file0)
    case("the five runs are named", [r[0] for r in RUNS] == ["clean", "P8", "gap-control", "P9", "P10"])
    case("the finding line is found, cargo's last line is not", FINDING.search("KIND a/b: x") is not None and FINDING.search("error: test failed") is None)
    p10_ok = lambda out: re.search(RUNS[4][3], out) is not None and not re.search(r"unwrap\(\)|None value", out)
    case("P10: the UNTYPED sentence without the old unwrap panic is as expected", p10_ok('thread panicked:\nUNTYPED inbox/tabs: add its kind (x)'))
    case("P10: the old panic (unwrap on a None) is NOT as expected", not p10_ok("panicked at state_parity.rs:74:53: called `Option::unwrap()` on a `None` value\nUNTYPED inbox/tabs: add its kind"))
    free, load, ok = preflight()
    print("preflight now: %d GB free, load %.1f -> %s" % (free, load, "would run" if ok else "would refuse"))
    print("%d problems" % bad)
    return 1 if bad else 0


if __name__ == "__main__":
    a = sys.argv[1:]
    if "--selftest" in a:
        sys.exit(selftest())
    if "--plan" in a or "--go" not in a:
        free, load, ok = preflight()
        print("plan: %s; preflight %d GB free (need 8), load %.1f (need under 30): %s" % (", ".join(r[0] for r in RUNS), free, load, "ok" if ok else "REFUSED"))
        sys.exit(0)
    if "--target" not in a:
        print("usage: state-parity-run.py --go --target <dir named *state-parity*, outside the repo>"); sys.exit(2)
    target = os.path.abspath(a[a.index("--target") + 1])
    if "state-parity" not in os.path.basename(target) or target.startswith(ROOT):
        print("REFUSED: the target dir must be named *state-parity* and sit outside the repo (it is deleted at the end)"); sys.exit(2)
    free, load, ok = preflight()
    if not ok:
        print("REFUSED: %d GB free (need 8), load %.1f (need under 30)" % (free, load)); sys.exit(2)
    sys.exit(0 if run_all(target) else 1)
