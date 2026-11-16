#!/usr/bin/env python3
"""The send shockwave's frame budget on the owner's own copy: reads the lines the app appends at every send it runs
on the fixture core (Sources/Design/SendWave/SendWavePerfLog.swift) and prints the same summary as
_captures/sendwave/timings.txt, ending with its last line. Exit 1 when a send dropped a frame or held the main
thread for FRAME_MS or longer; exit 3 with no sends; lines written in the hidden test rig are left out (their picture
is drawn on the main thread there, a cost the visible app does not pay).
Usage: python3 perf/sendwave-report.py [--kind send|landing] [path]
       (default kind send; default path ~/Library/Application Support/Whoosh/perf/sendwave.jsonl)
"""
import json
import os
import statistics
import sys

args = sys.argv[1:]
kind = "send"
if "--kind" in args:
    i = args.index("--kind"); kind = args[i + 1]; del args[i:i + 2]
path = args[0] if args else os.path.expanduser("~/Library/Application Support/Whoosh/perf/sendwave.jsonl")
if not os.path.exists(path):
    print(f"no log at {path}: no send has run on the fixture core yet"); sys.exit(3)
rows = []
for line in open(path, encoding="utf-8"):
    line = line.strip()
    if line:
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError:
            print("skipped a line that is not JSON:", line[:80])
rows = [r for r in rows if r.get("kind", "send") == kind]
hidden = [r for r in rows if r.get("hidden")]
rows = [r for r in rows if not r.get("hidden") and r.get("state") == "done"]
print(f"# {len(rows)} {kind} runs from {path} ({len(hidden)} hidden-rig lines left out)")
if not rows:
    sys.exit(3)
budget = float(rows[0].get("frame_ms_budget", 16))


def stat(k):
    v = sorted(float(r.get(k, 0)) for r in rows)
    return "median %.1f  p95 %.1f  max %.1f" % (statistics.median(v), v[min(len(v) - 1, int(len(v) * 0.95))], v[-1])


for k in ("max_busy_ms", "max_frame_ms", "snapshot_ms", "first_frame_ms"):
    print("%-15s %s" % (k, stat(k)))
print("picture sources:", ", ".join(sorted({str(r.get('snapshot_source')) for r in rows})))
dropped = sum(1 for r in rows if r.get("dropped", 0))
over = sum(1 for r in rows if float(r.get("max_busy_ms", 0)) >= budget)
print("runs with a dropped frame: %d of %d; runs with a main turn at or over FRAME_MS: %d of %d" % (dropped, len(rows), over, len(rows)))
sys.exit(1 if dropped or over else 0)
