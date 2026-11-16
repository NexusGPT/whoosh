"""Print what a journey's results files say, one line per fact, for the proof files (checks/proofs/g1): python3 journeys/lib/summary.py <results folder> <J5>

  STEP J5.1 PASS=3                      per step, the count of each verdict (the lines the per-step proofs read)
  THEME J5.1 theme-applied PASS got=dark   the theme-applied case: its verdict and the theme the app answered

Reads <folder>/<journey>-*.results.jsonl (journeys/lib/harness.py write_results). Launches nothing.
"""
import collections
import glob
import json
import os
import sys


def main(argv):
    if len(argv) != 2:
        sys.stderr.write("usage: summary.py <results folder> <journey id>\n")
        return 2
    folder, jid = argv
    files = sorted(glob.glob(os.path.join(folder, "%s-*.results.jsonl" % jid)))
    if not files:
        print("NO RESULTS FILE %s-*.results.jsonl in %s" % (jid, folder))
        return 1
    counts = collections.defaultdict(collections.Counter)
    themes = []
    for path in files:
        for line in open(path, encoding="utf-8"):
            if not line.strip():
                continue
            r = json.loads(line)
            counts[r["step"]][r["verdict"]] += 1
            if r["case"] == "theme-applied":
                themes.append("THEME %s theme-applied %s got=%s" % (r["step"], r["verdict"], r["got"]))
    for step in sorted(counts):
        print("STEP", step, ",".join("%s=%d" % (k, v) for k, v in sorted(counts[step].items())))
    for line in themes or ["THEME none: no theme-applied case in the results"]:
        print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
