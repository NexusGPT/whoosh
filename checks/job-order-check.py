#!/usr/bin/env python3
"""job-order-check (T-21, A114 "I see no new app since 12:12"; plan realcore-firstrun section 6).

Reads scripts/main-job.sh (the build job, moved into the repo) and asserts two things about its ORDER, no build, no app:
  1. the install step (`sh scripts/install.sh`) comes BEFORE the first long check: the perf gates, the check loop, the round. main63 was compiled at 12:12 and
     reached the owner at 13:35 because 80 minutes of checks ran first.
  2. the installed app is started with `open`, never as a bare binary: macOS attributes a permission prompt (Calendar, microphone) to the process that launched
     a bare binary, so the owner's Allow button asked nothing. A line that runs `.../Contents/MacOS/Whoosh` itself is red; `pgrep`/`grep`/`echo` that only name it are not.
    python3 checks/job-order-check.py              check the real script
    python3 checks/job-order-check.py --selftest   a model script that passes, then the two mutants (swapped install and round; bare binary): each must go red
Prints `N checks, F failed, 0 not judged`; exit 1 when F > 0.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
JOB = os.path.join(ROOT, "scripts", "main-job.sh")
INSTALL = re.compile(r"\bsh\s+scripts/install\.sh\b")
LONG = [re.compile(p) for p in (r"perf/launch\.py", r"perf/interaction\.py", r"perf/memory-mac\.py", r"perf/footprint\.py",
                                r"\bfor\s+c\s+in\s+checks/", r"\bsh\s+scripts/round\.sh\b", r"journeys/run\.py")]
BARE = re.compile(r"Contents/MacOS/Whoosh")
BENIGN = re.compile(r"\b(pgrep|grep|echo|pkill|ps)\b")
OPEN = re.compile(r"(^|[;&|(]\s*)open\s[^\n]*Whoosh\.app")


def code_lines(text):
    """(number, line) for the lines that run: comments and blank lines dropped."""
    return [(i + 1, l) for i, l in enumerate(text.splitlines()) if l.strip() and not l.lstrip().startswith("#")]


def checks(text):
    lines = code_lines(text)
    inst = [n for n, l in lines if INSTALL.search(l)]
    longs = [n for n, l in lines if any(p.search(l) for p in LONG)]
    bare = [(n, l.strip()[:90]) for n, l in lines if BARE.search(l) and not BENIGN.search(l.split("open", 1)[0] if OPEN.search(l) else l)]
    opens = [n for n, l in lines if OPEN.search(l)]
    out = []
    out.append(("the install step is in the job", bool(inst), "no `sh scripts/install.sh` line"))
    out.append(("the job has long checks (positive control: the probe finds them)", bool(longs), "no perf, check-loop or round line found"))
    out.append(("install-precedes-the-first-long-check", bool(inst and longs) and inst[0] < longs[0],
                "install at line %s, first long check at line %s" % (inst[:1] or "none", longs[:1] or "none")))
    out.append(("the app is started with open", bool(opens), "no `open ... Whoosh.app` line"))
    out.append(("never-started-as-a-bare-binary", not bare, "bare binary: %s" % bare))
    return out


def report(text):
    res = checks(text)
    bad = 0
    for name, ok, why in res:
        print("%s %s%s" % ("PASS" if ok else "FAIL", name, "" if ok else ": " + why))
        bad += not ok
    print("%d checks, %d failed, 0 not judged" % (len(res), bad))
    return bad


MODEL = '''#!/bin/sh
LBL="$1"
sh scripts/build.sh "$LBL" . >> "$L" 2>&1
INSTALL_ANYWAY=1 sh scripts/install.sh "$LBL" > "$S/install.log" 2>&1
# a comment that names sh scripts/round.sh and Contents/MacOS/Whoosh changes nothing
open --stdout a --stderr b "$HOME/Applications/Whoosh.app"; sleep 3
echo "relaunched: $(pgrep -fl 'Applications/Whoosh.app/Contents/MacOS/Whoosh' | head -1)"
python3 perf/launch.py --bundle "$B" > "$log" 2>&1
for c in checks/parity/par-keys.py; do sh scripts/launch-one.sh "$B" -- "$c"; done
sh scripts/round.sh "$LBL" > "$S/round.log" 2>&1
'''


def selftest():
    def red(text):
        return [n for n, ok, _ in checks(text) if not ok]
    bad = 0
    def case(name, got, want):
        nonlocal bad
        ok = got == want
        bad += not ok
        print("%-66s %s%s" % (name, "ok" if ok else "FAIL", "" if ok else "  got %r want %r" % (got, want)))
    case("the model passes every check", red(MODEL), [])
    swapped = MODEL.replace('INSTALL_ANYWAY=1 sh scripts/install.sh "$LBL" > "$S/install.log" 2>&1\n', "").replace(
        'sh scripts/round.sh "$LBL" > "$S/round.log" 2>&1\n', 'sh scripts/round.sh "$LBL" > "$S/round.log" 2>&1\nINSTALL_ANYWAY=1 sh scripts/install.sh "$LBL" > "$S/install.log" 2>&1\n')
    case("mutant: the install runs after the round", red(swapped), ["install-precedes-the-first-long-check"])
    after_perf = MODEL.replace('INSTALL_ANYWAY=1 sh scripts/install.sh "$LBL" > "$S/install.log" 2>&1\n', "").replace(
        'for c in checks/', 'INSTALL_ANYWAY=1 sh scripts/install.sh "$LBL" > "$S/install.log" 2>&1\nfor c in checks/')
    case("mutant: the install runs after the perf gates", red(after_perf), ["install-precedes-the-first-long-check"])
    bare = MODEL.replace('open --stdout a --stderr b "$HOME/Applications/Whoosh.app"; sleep 3', '"$HOME/Applications/Whoosh.app/Contents/MacOS/Whoosh" & sleep 3')
    case("mutant: the app is started as a bare binary", red(bare), ["the app is started with open", "never-started-as-a-bare-binary"])
    both = MODEL.replace('open --stdout a --stderr b "$HOME/Applications/Whoosh.app"; sleep 3', 'open "$HOME/Applications/Whoosh.app"; $HOME/Applications/Whoosh.app/Contents/MacOS/Whoosh &')
    case("mutant: open AND a bare start on one line pair is still red", red(both), ["never-started-as-a-bare-binary"])
    case("positive control: no long check at all is red, not green", "the job has long checks (positive control: the probe finds them)" in red("sh scripts/install.sh x\nopen /A/Whoosh.app\n"), True)
    real = report_quiet(open(JOB, encoding="utf-8").read()) if os.path.exists(JOB) else []
    case("the real scripts/main-job.sh passes (when it exists)", real, [])
    print("%d problems" % bad)
    return 1 if bad else 0


def report_quiet(text):
    return [n for n, ok, _ in checks(text) if not ok]


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        sys.exit(selftest())
    if not os.path.exists(JOB):
        print("FAIL scripts/main-job.sh does not exist\n1 checks, 1 failed, 0 not judged"); sys.exit(1)
    sys.exit(1 if report(open(JOB, encoding="utf-8").read()) else 0)
