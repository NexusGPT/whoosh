#!/usr/bin/env python3
"""Run the seam checks, each on a FRESH rig copy (scripts/launch-one.sh, --fixture-core), so one check's
actions never leak into the next.

    python3 checks/seams/run.py --bundle <path/to/Whoosh.app> [--slice 1|2|3|4|5|6|7|pick|grammar] [--mutants-only]

--mutants-only runs the judges' mutant self-test without an app. Exit 1 when any check exits non-zero.
"""
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
SLICES = {
    "1": ["counts", "account-colour", "error-vocabulary", "undo"],
    "2": ["search-colour", "smart-folder-count", "date-words"],
    "3": ["agent-words", "citation"],
    "4": ["undo-window", "from-signature"],
    "5": ["account-identity"],
    "6": ["notch-inbox"],
    "7": ["device-parity"],
    "pick": ["account-mark", "split-count", "decided-by"],
    "grammar": ["component-grammar"],
}


def main(a):
    sl = a[a.index("--slice") + 1] if "--slice" in a else None
    names = SLICES[sl] if sl else [n for k in SLICES for n in SLICES[k]]
    bundle = a[a.index("--bundle") + 1] if "--bundle" in a else None
    if not bundle and "--mutants-only" not in a:
        print("usage: run.py --bundle <Whoosh.app> [--slice N] | --mutants-only")
        return 2
    rc = 0
    for n in names:
        if "--mutants-only" in a:
            cmd = [sys.executable, os.path.join(HERE, n + ".py")]
            env = {k: v for k, v in os.environ.items() if k not in ("WHOOSH_SOCKET_DIR", "D")}
        else:
            cmd = ["sh", "scripts/launch-one.sh", bundle, "--", "--fixture-core", "python3", "checks/seams/%s.py" % n]
            # no window may open, ever (owner 2026-11-08): hidden rig mode is forced here, whatever the caller set
            env = dict(os.environ, WHOOSH_RIG_HIDDEN="1")
            le = env.get("LAUNCH_ENV", "")
            for kv in ("WHOOSH_RIG_HIDDEN=1", "WHOOSH_FIXTURE_CORE=1"):
                if kv not in le.split():
                    le = (le + " " + kv).strip()
            env["LAUNCH_ENV"] = le
        p = subprocess.run(cmd, cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        sys.stdout.write(p.stdout)
        last = [l for l in p.stdout.splitlines() if l.strip()][-1:] or ["(no output)"]
        print(">> %-20s exit %d  %s" % (n, p.returncode, last[0]))
        rc |= p.returncode != 0
    return 1 if rc else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
