#!/usr/bin/env python3
"""Drives ONE kind of action many times on the running rig copy, for an external profiler (xctrace) attached to it. Information only, no verdict.

  python3 perf/profile-actions.py archive|next|open [--count N]       run under scripts/launch-one.sh (hidden rig only)
Prints min / median / max of the commit and frame measures for that kind. Used by docs/PERF-FOOTPRINT.md "Frame time" to find what runs on the main thread per key."""
import os, statistics, sys
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl      # noqa: E402

def main(argv):
    kind, n = argv[0], int(argv[argv.index("--count") + 1]) if "--count" in argv else 80
    ctl, fr, co = Ctl().connect(), [], []
    for i in range(n):
        ctl.key({"archive": "E", "next": "J" if i % 2 == 0 else "K", "open": "Enter"}[kind])
        fr.append(float(ctl.measure("mac-main.interaction.frame")["ms"])); co.append(float(ctl.measure("mac-main.interaction.commit")["ms"]))
        if kind == "archive": ctl.key("Z")
        if kind == "open": ctl.key("Escape")
    print("profile-actions: %s n=%d commit min %.1f median %.1f max %.1f | frame min %.1f median %.1f max %.1f" % (kind, n, min(co), statistics.median(co), max(co), min(fr), statistics.median(fr), max(fr)))
    return 0

if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
