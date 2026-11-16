#!/usr/bin/env python3
"""Performance: the iPhone's resident memory with the 30-account fixture stays at or under MEM_IPHONE_MB (docs/CONSTANTS.md).

Drives the running iPhone app in the simulator over the control socket. Reads `measure iphone.memory` -> {"mb": <number>}
(assumed reply field; the iPhone shell registers it) after the 30-account fixture has loaded and one scroll of the list.
Exit 0 only when judged and within budget; exit 1 over budget, and exit 1 NOT JUDGED when the socket, the 30-account
fixture or a calm machine is missing. Prints the build stamp and the machine load first.
Self-test, no app: python3 perf/memory-iphone.py --selftest runs main() against a stand-in socket; exit 1 when a case differs from its expected exit and verdict.
Run: sh scripts/launch-one.sh <iphone-bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 perf/memory-iphone.py'
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


LEDGER_CORES = 12   # docs/CONSTANTS.md PERF_LOAD_MAX: "one runnable per core on this 12-core Mac"


def load_cap():
    """PERF_LOAD_MAX is stated for a 12-core Mac as one runnable per core: on another core count the same rule scales with the cores (None: no number)."""
    cap = constant("PERF_LOAD_MAX")
    return None if cap is None else cap * (os.cpu_count() or 1) / LEDGER_CORES


def judged_load():
    """(ok, why). Not judged when PERF_LOAD_MAX has no number, or the one-minute load average is above one runnable per core. Both figures are named."""
    cap = load_cap()
    if cap is None:
        return False, "PERF_LOAD_MAX has no number in docs/CONSTANTS.md, so a load cannot be judged"
    n = os.cpu_count() or 1
    if load_avg() > cap:
        return False, "load average %.2f (%.2f per core on %d cores) is above PERF_LOAD_MAX %s scaled to %d cores = %.2f (%.2f per core)" % (
            load_avg(), load_avg() / n, n, constant("PERF_LOAD_MAX"), n, cap, cap / n)
    return True, ""


MUTANTS = [
    ("memory/over-budget", "the iPhone holds more than MEM_IPHONE_MB with 30 accounts"),
    ("memory/fixture-is-30", "the fixture loads fewer than 30 accounts, so the memory figure is flattering"),
    ("memory/read", "the app registers no iphone.memory measure"),
    ("memory/load-gate", "a busy machine makes the memory check NOT JUDGED, hiding an over-budget figure"),
]


def main():
    ctl = None
    try:
        ctl = Ctl().connect()
    except CouldNotDrive:
        return finish("memory-iphone", MUTANTS, "NOT JUDGED", reason_not_driven(), None)
    try:
        # Memory does not depend on other processes' load: judged at any load (lead's decision, journeys/lib/loadgate.py). Only a missing ledger number stops it.
        if constant("PERF_LOAD_MAX") is None:
            return finish("memory-iphone", MUTANTS, "NOT JUDGED", "PERF_LOAD_MAX has no number in docs/CONSTANTS.md", ctl)
        limit = constant("MEM_IPHONE_MB")
        try:
            accounts = ctl.core("accounts/count")
            mb = float(ctl.measure("iphone.memory")["mb"])
        except (Unreadable, KeyError, TypeError, ValueError) as e:
            return finish("memory-iphone", MUTANTS, "NOT JUDGED", "read failed: %s" % e, ctl)
        if accounts != len(json.load(open(os.path.join(ROOT, "docs/design/fixtures/mac-main-sample.json")))["accounts"]):
            return finish("memory-iphone", MUTANTS, "NOT JUDGED", "the fixture holds %r accounts, not the 30 the budget is stated for" % accounts, ctl)
        verdict = "OVER BUDGET" if mb > limit else "WITHIN BUDGET"
        return finish("memory-iphone", MUTANTS, verdict, "%.1f MB resident, budget %g MB" % (mb, limit), ctl)
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


def _run_main(reply, load, serve=True, cpus=12):
    """Runs main() once with the stand-in answering and `load` as the machine's load average. Returns (exit code, stdout, stderr)."""
    global load_avg, machine_load
    saved = (load_avg, machine_load, os.environ.get("WHOOSH_SOCKET_DIR"), os.cpu_count)
    os.cpu_count = lambda: cpus
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
        load_avg, machine_load, os.cpu_count = saved[0], saved[1], saved[3]
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
    limit = constant("MEM_IPHONE_MB")
    cap = constant("PERF_LOAD_MAX")
    fixture_accounts = len(json.load(open(os.path.join(ROOT, "docs/design/fixtures/mac-main-sample.json")))["accounts"])

    def standin(mb, accounts, measure_ok=True):
        def reply(verb, args):
            if verb == "core" and args[-1] == "accounts/count":
                return {"ok": True, "value": accounts}
            if verb == "core":
                return {"ok": True, "value": "stand-in"}
            if verb == "measure" and args == ["iphone.memory"]:
                return {"ok": True, "mb": mb} if measure_ok else {"ok": False, "error": "no such measure"}
            return {"ok": False, "error": "stand-in: no verb %s" % verb}
        return reply

    calm = 1.0
    # (case, mutant it stands for, reply, load, expected exit, expected verdict)
    cases = [
        ("at-the-limit", "-", standin(limit, fixture_accounts), calm, 0, "WITHIN BUDGET"),
        ("one-over", "memory/over-budget", standin(limit + 0.5, fixture_accounts), calm, 1, "OVER BUDGET"),
        ("fixture-is-29", "memory/fixture-is-30", standin(100.0, fixture_accounts - 1), calm, 1, "NOT JUDGED"),
        ("no-measure", "memory/read", standin(100.0, fixture_accounts, measure_ok=False), calm, 1, "NOT JUDGED"),
        ("busy-machine-is-judged-all-the-same", "-", standin(100.0, fixture_accounts), cap + 0.5, 0, "WITHIN BUDGET"),
        ("no-socket", "-", None, calm, 1, "NOT JUDGED"),
        # PERF_LOAD_MAX scales with the cores (one runnable per core): raw 20 on 24 cores is 0.83 per core, raw 5 on 4 cores is 1.25 per core
        ("busy-24-cores-is-judged", "memory/load-gate", standin(100.0, fixture_accounts), 20.0, 0, "WITHIN BUDGET", 24),
        ("busy-4-cores-is-judged-and-over-is-red", "memory/load-gate", standin(300.0, fixture_accounts), 50.0, 1, "OVER BUDGET", 4),
        ("the-round-9-load-30-over-budget-is-red-not-hidden", "-", standin(300.0, fixture_accounts), 88.36, 1, "OVER BUDGET", 12),
    ]
    cases = [c if len(c) == 7 else c + (12,) for c in cases]
    print("| case | mutant | expected exit, verdict | got exit, verdict | same |")
    print("|---|---|---|---|---|")
    bad = 0
    for name, mutant, reply, load, want_code, want_verdict, cpus in cases:
        code, _out, err = _run_main(reply, load, serve=reply is not None, cpus=cpus)
        got = (code, _verdict(err))
        same = got == (want_code, want_verdict)
        bad += 0 if same else 1
        print("| %s | %s | %d, %s | %d, %s | %s |" % (name, mutant, want_code, want_verdict, got[0], got[1], "yes" if same else "NO"))
    print("limit read from docs/CONSTANTS.md: MEM_IPHONE_MB %g, PERF_LOAD_MAX %g; fixture accounts %d" % (limit, cap, fixture_accounts))
    print("selftest: %d cases, %d differ" % (len(cases), bad))
    return 1 if bad else 0


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        sys.exit(selftest())
    sys.exit(main())
