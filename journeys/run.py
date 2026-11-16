#!/usr/bin/env python3
"""Run the journeys: python3 journeys/run.py [J1 J2 ...] [--dry] [--themes light,dark] [--fixture-core]

Discovers journeys/J*.py, runs each over the control socket, prints exactly one
summary line `N checks, F failed, U not judged`, writes docs/COVERAGE.md, and
exits 1 unless every journey is GREEN. Per-journey lines go to stderr.
Before that summary line, stdout holds ONE line per judged case, passing ones too (harness.case_line: `<label> <pass|fail|unjudged> <detail>`,
for example `J5.1/theme-applied pass expected 'dark', got 'dark'`), in the order the journey judged them; with --themes each line is led by `[<theme>] `.

--themes light,dark (bare `--themes` = light,dark): every journey runs once per theme, in the order of the list, each pass with WHOOSH_THEME set to
that theme and the module loaded again (so TITLE and the theme the journey applies are that theme's); one stderr line per journey and theme, led by
"[<theme>] ". Each pass writes its results files under <WHOOSH_RESULTS_DIR, else _captures/results>/<theme>/ (the file name has no theme in it, so
two passes would overwrite each other in one folder). Both passes drive the same running app: a fixture core cannot be reset (see J1.py), so the lead
launches the test copy once per theme when the passes must not share state. Without the flag a journey runs as before, in WHOOSH_THEME or light.
"""
import glob
import importlib.util
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from lib.harness import run_journey, print_cases  # noqa: E402


def load(path):
    spec = importlib.util.spec_from_file_location(os.path.basename(path)[:-3], path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def write_coverage(results, dry):
    rows = ["# Coverage: journeys", "",
            "Written by `python3 journeys/run.py`. Verdicts: GREEN (every check passed), RED (a check failed), "
            "COULD-NOT-DRIVE (no running app to drive, or a read failed; amber for one round, then red), "
            "TIMING-NOT-JUDGED (the app was driven and every other check passed; a timing over budget was measured above PERF_LOAD_MAX, so it is not judged; not green).", "",
            "| journey | target | visible_outcome | verdict | evidence |", "|---|---|---|---|---|"]
    for r in results:
        cell = lambda s: s.replace("|", "/").replace("\n", " ")
        rows.append("| %s | %s | %s | %s | %s |" % (r["id"], r["target"], cell(r["visible_outcome"]),
                                                    r["verdict"], cell(r["evidence"])))
    rows += ["", "Checks per journey: " + ", ".join("%s %d" % (r["id"], r["checks"]) for r in results) + "."]
    path = os.path.join(ROOT, "docs", "COVERAGE.md")
    # The file holds more than this table: the control tables of the other checks follow it, each under its own "## " heading. They are kept as
    # they are (a full run on 14 Nov 2026 wrote the journeys table alone and took 890 lines of control tables with it).
    kept = ""
    try:
        with open(path, encoding="utf-8") as f:
            old = f.read()
        at = old.find("\n## ")
        if at >= 0:
            kept = "\n" + old[at + 1:].rstrip("\n") + "\n"
    except OSError:
        pass
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(rows) + "\n" + kept)


class _Lead:
    """A stdout that puts `lead` in front of every line written to it (print_cases writes whole lines)."""
    def __init__(self, lead):
        self.lead = lead

    def write(self, text):
        sys.stdout.write("".join(self.lead + ln for ln in text.splitlines(True)))


def themes_arg(argv):
    """None without the flag; else the list of themes (`--themes a,b` or `--themes=a,b`; bare = every theme). A name outside lib.theme.THEMES raises ValueError."""
    from lib.theme import THEMES
    for i, a in enumerate(argv):
        if a == "--themes" or a.startswith("--themes="):
            raw = a.split("=", 1)[1] if "=" in a else (argv[i + 1] if i + 1 < len(argv) and not argv[i + 1].startswith("-") and not re.fullmatch(r"J\d+", argv[i + 1]) else ",".join(THEMES))
            names = [x for x in raw.split(",") if x]
            bad = [x for x in names if x not in THEMES]
            if bad or not names:
                raise ValueError("--themes takes %s, got %r" % (" or ".join(THEMES), raw))
            return names
    return None


def _lists(r):
    for line in r["fail_list"]:
        sys.stderr.write("    FAIL %s\n" % line)
    if r["no_mutant"]:
        sys.stderr.write("    MUTANTS missing for: %s\n" % ", ".join(r["no_mutant"]))
    if r["stray_mutant"]:
        sys.stderr.write("    MUTANTS naming no check: %s\n" % ", ".join(r["stray_mutant"]))


def main(argv):
    only = [a for a in argv if re.fullmatch(r"J\d+", a)]
    dry = "--dry" in argv
    try:
        themes = themes_arg(argv)
    except ValueError as e:
        sys.stderr.write("%s\n" % e)
        return 2
    paths = sorted(glob.glob(os.path.join(HERE, "J*.py")), key=lambda p: int(re.search(r"J(\d+)", p).group(1)))
    paths = [p for p in paths if not only or os.path.basename(p)[:-3] in only]
    if not paths:
        print("0 checks, 0 failed, 0 not judged")
        sys.stderr.write("no journeys found in %s\n" % HERE)
        return 1
    results = []
    env0 = {k: os.environ.get(k) for k in ("WHOOSH_THEME", "WHOOSH_RESULTS_DIR")}
    base_results = env0["WHOOSH_RESULTS_DIR"] or os.path.join(ROOT, "_captures", "results")
    passes = [(None, p) for p in paths] if themes is None else [(t, p) for t in themes for p in paths]
    try:
        for theme, p in passes:
            if theme is not None:
                os.environ["WHOOSH_THEME"] = theme
                os.environ["WHOOSH_RESULTS_DIR"] = os.path.join(base_results, theme)
            r = run_journey(load(p), p, force_dry=dry)
            r["theme"] = theme
            if theme is not None:
                r["id"] = "%s (%s)" % (r["id"], theme)     # the coverage record gets one row per journey and theme
            results.append(r)
            print_cases(r, _Lead("[%s] " % theme if theme else ""))
            sys.stderr.write("%s%s %s: %d steps, %d checks, %d failed, %d not judged: %s\n" % (
                "[%s] " % theme if theme else "", r["id"].split(" (")[0], r["verdict"], r["steps"], r["checks"], r["failed"], r["unjudged"], r["evidence"]))
            _lists(r)
    finally:
        for k, v in env0.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
    if not only and not dry:
        write_coverage(results, dry)   # a dry run judges nothing: it never rewrites the record the merge gate reads
    n = sum(r["checks"] for r in results)
    f = sum(r["failed"] for r in results)
    u = sum(r["unjudged"] for r in results)
    print("%d checks, %d failed, %d not judged" % (n, f, u))
    if not dry:
        _record_run("%d checks, %d failed, %d not judged" % (n, f, u))
    green = all(r["verdict"] == "GREEN" for r in results)
    return 0 if green and n > 0 else 1




def _record_run(summary_line):
    """Write journeys/.last-pass (untracked): the build door reads its mtime as proof the suite ran."""
    import os, time
    try:
        here = os.path.dirname(os.path.abspath(__file__))
        with open(os.path.join(here, ".last-pass"), "w") as f:
            f.write(time.strftime("%Y-%m-%dT%H:%M:%S") + " " + summary_line + "\n")
    except OSError:
        pass

if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
