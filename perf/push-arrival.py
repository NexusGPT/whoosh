#!/usr/bin/env python3
"""Performance: the 95th percentile of push arrival on the iPhone stays at or under PUSH_ARRIVAL_P95_S (docs/CONSTANTS.md).

Delivers N mails through the rig (`fixture deliver <id>`) and reads `measure iphone-lock.notifications` -> {"ms": <number>}
after each (the time from the server receiving the mail to the notification showing; the iPhone shell registers it).
On the simulator the relay leg is a stand-in; the cell that counts is the one run on the real iPhone 17 Pro (docs/lane-dag.json
fn-tests-j7). Exit 0 only when judged and within budget; exit 1 over budget; exit 1 NOT JUDGED when the socket or a calm
machine is missing. Prints the build stamp and the machine load first.
Self-test, no app: python3 perf/push-arrival.py --selftest runs main() against a stand-in socket; exit 1 when a case differs from its expected exit and verdict.
Run: sh scripts/launch-one.sh <iphone-bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 perf/push-arrival.py'
"""

import contextlib
import io
import json
import os
import re
import socketserver
import subprocess
import sys
import tempfile
import threading

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable                               # noqa: E402


def constant(name):
    """A number from docs/CONSTANTS.md. A row 'A / B | x / y' (two names, one cell) is split on the slash, which
    journeys/lib/fixture.const() does not do (it would give MEM_IPHONE_MB the Mac's 600). None when the cell has no number."""
    for line in open(os.path.join(ROOT, "docs", "CONSTANTS.md"), encoding="utf-8"):
        if not line.startswith("|"):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        names = [n.strip() for n in re.split(r"[·/]", cells[0])]
        if len(cells) > 1 and name in names:
            parts = [p for p in re.split(r"\s/\s|·", cells[1])]
            part = parts[names.index(name)] if len(parts) == len(names) else parts[0]
            m = re.search(r"\d[\d,]*(?:\.\d+)?", part)
            return float(m.group(0).replace(",", "")) if m else None
    raise KeyError("%s is not in docs/CONSTANTS.md" % name)


def build_stamp(ctl):
    try:
        head = subprocess.run(["git", "-C", ROOT, "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip()
        dirty = subprocess.run(["git", "-C", ROOT, "status", "--porcelain"], capture_output=True, text=True).stdout.strip()
    except OSError:
        head, dirty = "unknown", ""
    app = "unread"
    if ctl is not None:
        try:
            app = str(ctl.core("build/stamp"))
        except Unreadable:
            app = "not served by the app"
    return "source %s%s, app build stamp: %s" % (head, "+dirty" if dirty else "", app)


def machine_load():
    return os.getloadavg()[0] / (os.cpu_count() or 1)


def reason_not_driven():
    d = os.environ.get("WHOOSH_SOCKET_DIR", "")
    if d and os.path.exists(os.path.join(d, "whoosh.sock")):
        return ""
    try:
        out = subprocess.run(["xcrun", "simctl", "list", "runtimes"], capture_output=True, text=True, timeout=20).stdout
    except (OSError, subprocess.SubprocessError):
        out = ""
    if "iOS" not in out:
        return "the iPhone simulator cannot run: Xcode reports iOS 26.5 is not installed (owner step: Xcode > Settings > Components > iOS 26.5)"
    return "no iPhone app socket: launch the iPhone app in the simulator, then set WHOOSH_SOCKET_DIR"


def finish(title, mutants, verdict, detail, ctl):
    """Prints the stamp, load, MUTANTS and the summary line; returns the exit code (1 unless judged and within budget)."""
    print("build: %s" % build_stamp(ctl))
    print("machine load: %.2f per core" % machine_load())
    print("MUTANTS (a check is green only after it has been seen red on its mutant)")
    print("| check | mutant |")
    print("|---|---|")
    for k, v in mutants:
        print("| %s | %s |" % (k, v))
    sys.stderr.write("%s: %s: %s\n" % (title, verdict, detail))
    failed, unjudged = (1, 0) if verdict == "OVER BUDGET" else (0, 1) if verdict == "NOT JUDGED" else (0, 0)
    print("1 checks, %d failed, %d not judged" % (failed, unjudged))
    return 0 if verdict == "WITHIN BUDGET" else 1


def load_avg():
    """The one-minute load average, the figure PERF_LOAD_MAX is stated for (docs/CONSTANTS.md: 'the one-minute load average above which...')."""
    return os.getloadavg()[0]


def judged_load():
    """(ok, why). Not judged when PERF_LOAD_MAX has no number, or the one-minute load average is above it."""
    cap = constant("PERF_LOAD_MAX")
    if cap is None:
        return False, "PERF_LOAD_MAX has no number in docs/CONSTANTS.md, so a load cannot be judged"
    if load_avg() > cap:
        return False, "load average %.2f is above PERF_LOAD_MAX %s" % (load_avg(), cap)
    return True, ""

N = 20

MUTANTS = [
    ("push/over-budget", "the push waits for the next poll, so the 95th percentile passes PUSH_ARRIVAL_P95_S"),
    ("push/tail-not-mean", "one slow push in twenty is hidden by a good mean"),
    ("push/read", "the app registers no arrival measure"),
]


def p95(values):
    v = sorted(values)
    return v[min(len(v) - 1, int(round(0.95 * (len(v) - 1))))]


def main():
    ctl = None
    try:
        ctl = Ctl().connect()
    except CouldNotDrive:
        return finish("push-arrival", MUTANTS, "NOT JUDGED", reason_not_driven(), None)
    try:
        ok, why = judged_load()
        if not ok:
            return finish("push-arrival", MUTANTS, "NOT JUDGED", why, ctl)
        limit_ms = constant("PUSH_ARRIVAL_P95_S") * 1000.0
        ms = []
        try:
            for k in range(N):
                ctl.send("fixture", "deliver", "arrival-%02d" % k)
                ms.append(float(ctl.measure("iphone-lock.notifications")["ms"]))
        except (Unreadable, KeyError, TypeError, ValueError) as e:
            return finish("push-arrival", MUTANTS, "NOT JUDGED", "read failed after %d of %d: %s" % (len(ms), N, e), ctl)
        got = p95(ms)
        verdict = "OVER BUDGET" if got > limit_ms else "WITHIN BUDGET"
        return finish("push-arrival", MUTANTS, verdict, "p95 %.0f ms over %d pushes (max %.0f ms), budget %.0f ms" % (got, N, max(ms), limit_ms), ctl)
    finally:
        ctl.close()


# ---------------------------------------------------------------- --selftest: the script's own main() against a stand-in socket
class _H(socketserver.StreamRequestHandler):
    def handle(self):
        for line in self.rfile:
            try:
                req = json.loads(line.decode())
                rep = self.server.reply(req["verb"], req.get("args", []))
            except Exception as e:  # a stand-in bug must show as a refused request, never as a number
                rep = {"ok": False, "error": "stand-in: %s" % e}
            self.wfile.write((json.dumps(rep) + "\n").encode())


class _S(socketserver.ThreadingMixIn, socketserver.UnixStreamServer):
    daemon_threads = True


def _run_main(reply, load, serve=True):
    """Runs main() once with the stand-in answering and `load` as the machine's load average. Returns (exit code, stdout, stderr)."""
    global load_avg, machine_load
    saved = (load_avg, machine_load, os.environ.get("WHOOSH_SOCKET_DIR"))
    d = tempfile.mkdtemp(prefix="perfst")
    srv = None
    try:
        if serve:
            srv = _S(os.path.join(d, "whoosh.sock"), _H)
            srv.reply = reply
            threading.Thread(target=srv.serve_forever, daemon=True).start()
        os.environ["WHOOSH_SOCKET_DIR"] = d
        load_avg, machine_load = (lambda: load), (lambda: load)
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = main()
        return code, out.getvalue(), err.getvalue()
    finally:
        load_avg, machine_load = saved[0], saved[1]
        if saved[2] is None:
            os.environ.pop("WHOOSH_SOCKET_DIR", None)
        else:
            os.environ["WHOOSH_SOCKET_DIR"] = saved[2]
        if srv is not None:
            srv.shutdown()
            srv.server_close()
        for n in os.listdir(d):
            os.unlink(os.path.join(d, n))
        os.rmdir(d)


def _verdict(err):
    m = re.search(r": (WITHIN BUDGET|OVER BUDGET|NOT JUDGED): ", err)
    return m.group(1) if m else "none"


def selftest():
    """Every case below has its expected exit code and verdict written before it runs; the table lists each case that differs as a problem."""
    limit_ms = constant("PUSH_ARRIVAL_P95_S") * 1000.0
    cap = constant("PERF_LOAD_MAX")

    def standin(values, measure_ok=True):
        seq = list(values)
        seen = {"deliver": 0}

        def reply(verb, args):
            if verb == "core":
                return {"ok": True, "value": "stand-in"}
            if verb == "fixture" and args[:1] == ["deliver"]:
                seen["deliver"] += 1
                return {"ok": True}
            if verb == "measure" and args == ["iphone-lock.notifications"]:
                if not measure_ok:
                    return {"ok": False, "error": "no such measure"}
                return {"ok": True, "ms": seq[min(seen["deliver"], len(seq)) - 1]}
            return {"ok": False, "error": "stand-in: no verb %s" % verb}
        return reply, seen

    calm = 1.0
    two_slow = [1000.0] * (N - 2) + [limit_ms + 1000.0] * 2
    mean_two_slow = sum(two_slow) / len(two_slow)
    # (case, mutant it stands for, values, measure ok, load, expected exit, expected verdict)
    cases = [
        ("p95-at-the-limit", "-", [limit_ms] * N, True, calm, 0, "WITHIN BUDGET"),
        ("p95-one-ms-over", "push/over-budget", [limit_ms + 1.0] * N, True, calm, 1, "OVER BUDGET"),
        ("two-slow-in-twenty", "push/tail-not-mean", two_slow, True, calm, 1, "OVER BUDGET"),
        ("no-measure", "push/read", [1.0] * N, False, calm, 1, "NOT JUDGED"),
        ("busy-machine", "-", [1.0] * N, True, cap + 0.5, 1, "NOT JUDGED"),
        ("no-socket", "-", None, True, calm, 1, "NOT JUDGED"),
    ]
    print("| case | mutant | expected exit, verdict | got exit, verdict | pushes delivered | same |")
    print("|---|---|---|---|---|---|")
    bad = 0
    for name, mutant, values, measure_ok, load, want_code, want_verdict in cases:
        reply, seen = standin(values or [0.0], measure_ok)
        code, _out, err = _run_main(reply, load, serve=values is not None)
        got = (code, _verdict(err))
        delivered = seen["deliver"]
        same = got == (want_code, want_verdict)
        if name in ("p95-at-the-limit", "p95-one-ms-over", "two-slow-in-twenty"):
            same = same and delivered == N
        bad += 0 if same else 1
        print("| %s | %s | %d, %s | %d, %s | %d | %s |" % (name, mutant, want_code, want_verdict, got[0], got[1], delivered, "yes" if same else "NO"))
    print("the two-slow case has a mean of %.0f ms, below the limit %.0f ms, so only a tail judge turns it red" % (mean_two_slow, limit_ms))
    print("limit read from docs/CONSTANTS.md: PUSH_ARRIVAL_P95_S %g s, PERF_LOAD_MAX %g; pushes per run %d" % (limit_ms / 1000.0, cap, N))
    print("selftest: %d cases, %d differ" % (len(cases), bad))
    return 1 if bad else 0


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        sys.exit(selftest())
    sys.exit(main())
