#!/usr/bin/env python3
"""Every journey check names the mutation that turns it red, and every named mutation names a check.

    python3 checks/mutant-coverage-check.py          exit 0 when every journeys/J*.py is covered, 1 otherwise

docs/TESTING.md: a check is green only after it has been seen red on its mutant. This check holds the
first half: the mutant is DECLARED (the `# MUTANTS` block). Runs each journey dry (no socket), so it
needs no app and never writes docs/COVERAGE.md.
"""
import glob
import importlib.util
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey  # noqa: E402


def main():
    jdir = os.environ.get("WHOOSH_JOURNEYS_DIR") or os.path.join(ROOT, "journeys")
    paths = sorted(glob.glob(os.path.join(jdir, "J*.py")))
    if not paths:
        print("0 checks, 0 failed, 0 not judged")
        print("no journeys found")
        return 1
    n = bad = 0
    for p in paths:
        spec = importlib.util.spec_from_file_location(os.path.basename(p)[:-3], p)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        r = run_journey(mod, p, force_dry=True)
        n += r["checks"]
        for label in r["no_mutant"]:
            bad += 1
            print("NO MUTANT  %s" % label)
        for label in r["stray_mutant"]:
            bad += 1
            print("STRAY      %s names no check" % label)
    print("%d checks, %d failed, 0 not judged" % (n, bad))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
