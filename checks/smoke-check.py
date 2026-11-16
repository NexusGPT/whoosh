#!/usr/bin/env python3
"""Smoke check: fn-release-smoke. Launch, first frame, one archive over the socket, quit, on the INSTALLED bytes.

Run (what scripts/install.sh does after a copy, and what the manifest names):
    sh scripts/launch-one.sh <installed bundle> -- sh -c 'exec python3 "$0" "$D"' checks/smoke-check.py
    python3 checks/smoke-check.py --installed          (inside a launch-one command: the run directory is $D)
    python3 checks/smoke-check.py <run-dir>            the same, the directory named

The copy under test is the rig copy launch-one.sh made from the installed bundle (test identity
<bundle-id>.rig, LSUIElement). This check proves the driven bytes ARE the installed bytes: the stamp
(NexusBuildLabel + NexusBuildCommit) of the installed bundle equals the build's stamp file, and the
stamp of the running copy equals the installed one. It never launches or kills anything.

The archive needs a core with mail in it, so the copy must start on the fixture core:
    LAUNCH_ENV="WHOOSH_FIXTURE_CORE=1" sh scripts/install.sh <label>
With the real core unavailable the archive is not judged, and a check that is not judged is not green.

Quit: scripts/launch-one.sh stops the instance by the pid it captured, after this check returns; a quit
sent from here would read to it as DIED (rc=4). This check names the instance pid and starts a watcher
that records, in _captures/release/quit-last.txt, whether that pid is gone and how fast.

Prints 'N checks, F failed, U not judged' then the MUTANTS table; exit 1 when F > 0, U > 0 or N = 0.
First frame is judged by state: one visible window of real size, a measured first paint, and the inbox rows
served. The picture is written and counted as a file; its pixels are reported in a NOTE, never judged, because
the shot verb can return a picture with no opaque pixel (see the NOTE line) and a capture proves only what it shows.
Numbers: none of its own. The first-paint time is printed, never judged here (perf/launch.py judges it
against LAUNCH_WARM_MS in docs/CONSTANTS.md, and a clock verdict under load is not judged).
"""
import json
import os
import plistlib
import re
import shutil
import socket
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
import importlib.util as _ilu
_sw = _ilu.spec_from_file_location("shotwait", os.path.join(ROOT, "journeys", "lib", "shotwait.py")); shotwait = _ilu.module_from_spec(_sw); _sw.loader.exec_module(shotwait)   # how long a `shot` reply may take (one place)
SOCK_WAIT_S = 10          # one socket read; the reply of a loaded Mac, not a budget
CAPTURE_DIR = os.path.join(ROOT, "_captures", "release")
VERB_NEEDED = ("key", "rows", "uistate", "winframes", "measure", "core", "shot", "verbs")

MUTANTS = {
    "stamp-compared-to-wrong-file": "the installed stamp is compared with another build's stamp file",
    "running-copy-is-not-installed": "the running copy is compared with a bundle of another commit",
    "archive-presses-snooze": "the archive key does something else (H is snooze): nothing is archived, the core flag stays false",
    "two-windows-expected": "the app opens a second window",
    "shot-refused": "the shot verb refuses (no picture of the first frame is written)",
    "list-region-missing": "the inbox list is not drawn (rows of a region nobody registers)",
}

results = []              # (name, "PASS" | "FAIL" | "NOT JUDGED", detail)
notes = []                # facts printed beside the verdicts, never judged


def record(name, ok, detail="", judged=True):
    results.append((name, ("PASS" if ok else "FAIL") if judged else "NOT JUDGED", detail))
    return ok


class Ctl:
    def __init__(self, d):
        self.path = os.path.join(d, "whoosh.sock")
        self.s = None
        self.buf = b""

    def connect(self):
        if not os.path.exists(self.path):
            raise RuntimeError("no socket at %s" % self.path)
        self.s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.s.settimeout(SOCK_WAIT_S)
        self.s.connect(self.path)

    def send(self, verb, *args):
        """The reply dict; ok:false is returned as it is, so the caller judges it."""
        self.s.settimeout(shotwait.timeout_for(verb, args, SOCK_WAIT_S))   # a `shot` may wait for motion and the page, longer than one socket read
        self.s.sendall((json.dumps({"verb": verb, "args": list(args)}) + "\n").encode())
        while b"\n" not in self.buf:
            chunk = self.s.recv(65536)
            if not chunk:
                raise OSError("socket closed")
            self.buf += chunk
        line, self.buf = self.buf.split(b"\n", 1)
        return json.loads(line.decode())


def shell_env(repo):
    """INSTALL_DIR, BUILD_ROOT and APP_NAME as the pipeline reads them: the environment first, then docs/pipeline.env."""
    code = '. "$1"; printf "%s\\n%s\\n%s\\n" "$APP_NAME" "$BUILD_ROOT" "$INSTALL_DIR"'
    env = dict(os.environ)
    out = subprocess.run(["sh", "-c", ': "${APP_NAME:=}"; ' + code, "sh", os.path.join(repo, "docs", "pipeline.env")],
                         capture_output=True, text=True, env=env)
    if out.returncode != 0:
        raise RuntimeError("docs/pipeline.env did not load: %s" % out.stderr.strip())
    app, broot, idir = out.stdout.split("\n")[:3]
    if not os.path.isabs(broot):
        broot = os.path.join(repo, broot)
    return app, broot, idir.rstrip("/")


def stamp_of(bundle):
    with open(os.path.join(bundle, "Contents", "Info.plist"), "rb") as f:
        p = plistlib.load(f)
    return p.get("NexusBuildLabel"), p.get("NexusBuildCommit"), p.get("CFBundleIdentifier"), p.get("LSUIElement")


def png_ok(path):
    try:
        with open(path, "rb") as f:
            head = f.read(8)
        return head == b"\x89PNG\r\n\x1a\n" and os.path.getsize(path) > 1000
    except OSError:
        return False


def picture_content(path, tmp_dir):
    """(distinct RGBA values, opaque pixels, all pixels) of a 128-px-wide copy of the picture. None when it cannot be read."""
    import struct
    import zlib
    small = os.path.join(tmp_dir, "smoke-small.png")
    r = subprocess.run(["sips", "-Z", "128", path, "--out", small], capture_output=True, text=True)
    if r.returncode != 0 or not os.path.exists(small):
        return None
    d = open(small, "rb").read()
    i, idat, w, h, ct, bd = 8, b"", 0, 0, 0, 0
    while i < len(d):
        n, = struct.unpack(">I", d[i:i + 4])
        kind = d[i + 4:i + 8]
        if kind == b"IHDR":
            w, h, bd, ct = struct.unpack(">IIBB", d[i + 8:i + 18])
        elif kind == b"IDAT":
            idat += d[i + 8:i + 8 + n]
        i += 12 + n
    bpp = {6: 4, 2: 3, 0: 1, 4: 2}.get(ct)
    if bpp is None or bd != 8:
        return None
    raw = zlib.decompress(idat)
    stride = w * bpp
    prev = bytearray(stride)
    seen = set()
    opaque = 0
    pos = 0
    for _ in range(h):
        ft = raw[pos]
        line = bytearray(raw[pos + 1:pos + 1 + stride])
        pos += 1 + stride
        for x in range(stride):
            a = line[x - bpp] if x >= bpp else 0
            b = prev[x]
            c = prev[x - bpp] if x >= bpp else 0
            if ft == 1:
                line[x] = (line[x] + a) & 255
            elif ft == 2:
                line[x] = (line[x] + b) & 255
            elif ft == 3:
                line[x] = (line[x] + ((a + b) >> 1)) & 255
            elif ft == 4:
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[x] = (line[x] + pr) & 255
        for x in range(0, stride, bpp):
            seen.add(bytes(line[x:x + bpp]))
            if bpp != 4 or line[x + 3] >= 128:
                opaque += 1
        prev = line
    return len(seen), opaque, w * h


def instance_pid(rig_app):
    """The pid whose own executable sits in this run's rig copy (first word of its command line)."""
    prefix = os.path.join(rig_app, "Contents", "MacOS") + "/"
    out = subprocess.run(["ps", "-axo", "pid=,command="], capture_output=True, text=True).stdout
    for line in out.splitlines():
        pid, _, cmd = line.strip().partition(" ")
        if cmd.startswith(prefix):
            return int(pid)
    return None


def watch_quit(pid):
    """Detached: records whether the instance is gone, and how fast, once launch-one stops it. Never signals."""
    os.makedirs(CAPTURE_DIR, exist_ok=True)
    out = os.path.join(CAPTURE_DIR, "quit-last.txt")
    script = ('t0=$(date +%%s); i=0; while [ $i -lt 120 ]; do '
              'if ! kill -0 %d 2>/dev/null; then printf "pid %d gone after %%ss of watching, started %%s\\n" "$(( $(date +%%s) - t0 ))" "$t0" > "%s"; exit 0; fi; '
              'sleep 0.5; i=$((i+1)); done; printf "pid %d still alive after 60s of watching\\n" > "%s"' % (pid, pid, out, pid, out))
    subprocess.Popen(["sh", "-c", script], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                     stderr=subprocess.DEVNULL, start_new_session=True)
    return out


def bundle_under_test(installed):
    """The bundle whose bytes the rig copy was made from. scripts/install.sh runs this check on the installed copy
    (WHOOSH_BUNDLE unset), so the installed bundle is the one under test. scripts/round.sh runs it BEFORE the install,
    on the round's own build (WHOOSH_BUNDLE set): the installed folder then still holds the previous build, and
    comparing the running copy with it reads a stale stamp (round main48: installed main47, build file deleted)."""
    env = os.environ.get("WHOOSH_BUNDLE")
    if env:
        b = env if os.path.isabs(env) else os.path.join(ROOT, env)
        if os.path.isdir(b):
            return b
    return installed


def judge_stamps(under, installed, rig_app, broot, d, mutant):
    """Two checks: the bundle under test equals the build's stamp file; the running copy carries that stamp (and the test identity)."""
    app = os.path.basename(installed)[:-len(".app")]
    try:
        label, commit, _, _ = stamp_of(under)
    except OSError as e:
        record("installed-stamp-equals-build-file", False, "bundle under test unreadable: %s" % e)
        label = commit = None
    bfile = os.path.join(broot, "%s-%s" % (app, label), "build-%s.json" % label)
    if mutant == "stamp-compared-to-wrong-file":
        wrong = json.load(open(bfile)) if os.path.exists(bfile) else {}
        wrong["commit"] = "0" * 40
        bfile = os.path.join(d, "other-build.json")
        json.dump(wrong, open(bfile, "w"))
    if label is not None:
        try:
            built = json.load(open(bfile))
            record("installed-stamp-equals-build-file", bool(label) and built.get("label") == label and built.get("commit") == commit,
                   "%s %s %s; %s says %s %s" % (os.path.basename(under), label, (commit or "")[:12], os.path.basename(bfile), built.get("label"), (built.get("commit") or "")[:12]))
        except (OSError, ValueError) as e:
            record("installed-stamp-equals-build-file", False, "build file unreadable: %s" % e)

    # ---- the running copy is the bytes under test, under the test identity
    try:
        rl, rc, rid, rui = stamp_of(rig_app)
        want = (label, commit) if mutant != "running-copy-is-not-installed" else (label, "f" * 40)
        record("running-copy-has-installed-stamp", (rl, rc) == want, "running %s %s; under test %s %s" % (rl, (rc or "")[:12], want[0], (want[1] or "")[:12]))
        record("test-identity", bool(rid) and rid.endswith(".rig") and rui is True and os.path.realpath(rig_app) != os.path.realpath(under),
               "id %s LSUIElement %s" % (rid, rui))
    except OSError as e:
        record("running-copy-has-installed-stamp", False, "no rig copy at %s: %s" % (rig_app, e))


def selftest():
    """The stamp judgement on fake bundles (no app is launched): green on a matching round, red on each wrong pairing."""
    import tempfile
    t = tempfile.mkdtemp(prefix="smoke-selftest-")

    def bundle(path, label, commit, ident="io.test", ui=None):
        os.makedirs(os.path.join(path, "Contents"))
        info = {"NexusBuildLabel": label, "NexusBuildCommit": commit, "CFBundleIdentifier": ident}
        if ui is not None:
            info["LSUIElement"] = ui
        with open(os.path.join(path, "Contents", "Info.plist"), "wb") as f:
            plistlib.dump(info, f)

    broot = os.path.join(t, "build")
    installed = os.path.join(t, "inst", "Fake.app")
    under = os.path.join(broot, "Fake-m2", "Build", "Fake.app")
    rig = os.path.join(t, "run", "rig", "Fake.app")
    bundle(installed, "m1", "1" * 40)
    bundle(under, "m2", "2" * 40)
    bundle(rig, "m2", "2" * 40, ident="io.test.rig", ui=True)
    json.dump({"label": "m2", "commit": "2" * 40}, open(os.path.join(broot, "Fake-m2", "build-m2.json"), "w"))
    bad = []

    def run(under_b, mutant=None, rig_b=rig):
        del results[:]
        judge_stamps(under_b, installed, rig_b, broot, os.path.join(t, "run"), mutant)
        return {n: v for n, v, _ in results}

    got = run(under)
    if any(v != "PASS" for v in got.values()) or len(got) != 3:
        bad.append("round on a build ahead of the install: expected 3 PASS, got %s" % got)
    got = run(installed)
    if got.get("installed-stamp-equals-build-file") != "FAIL" or got.get("running-copy-has-installed-stamp") != "FAIL":
        bad.append("the stale installed bundle read as the bundle under test: expected both FAIL, got %s" % got)
    if run(under, "running-copy-is-not-installed").get("running-copy-has-installed-stamp") != "FAIL":
        bad.append("mutant running-copy-is-not-installed stayed green")
    if run(under, "stamp-compared-to-wrong-file").get("installed-stamp-equals-build-file") != "FAIL":
        bad.append("mutant stamp-compared-to-wrong-file stayed green")
    other = os.path.join(t, "other-rig", "rig", "Fake.app")
    bundle(other, "m1", "1" * 40, ident="io.test.rig", ui=True)
    if run(under, None, other).get("running-copy-has-installed-stamp") != "FAIL":
        bad.append("a running copy of another build read as the bundle under test")
    del results[:]
    env = os.environ.pop("WHOOSH_BUNDLE", None)
    try:
        if bundle_under_test(installed) != installed:
            bad.append("without WHOOSH_BUNDLE the installed bundle must be the one under test")
        os.environ["WHOOSH_BUNDLE"] = under
        if bundle_under_test(installed) != under:
            bad.append("with WHOOSH_BUNDLE the round's build must be the one under test")
    finally:
        os.environ.pop("WHOOSH_BUNDLE", None)
        if env is not None:
            os.environ["WHOOSH_BUNDLE"] = env
    shutil.rmtree(t, ignore_errors=True)
    for b in bad:
        print("SELFTEST FAIL: " + b)
    print("selftest: 6 cases, %d failed" % len(bad))
    return 1 if bad else 0


def main(argv):
    if "--selftest" in argv:
        return selftest()
    mutant = None
    if "--mutant" in argv:
        mutant = argv[argv.index("--mutant") + 1]
        if mutant not in MUTANTS:
            print("unknown mutant %r; known: %s" % (mutant, ", ".join(MUTANTS)))
            return 2
    args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--mutant")]
    d = args[0] if args else (os.environ.get("D") or os.environ.get("WHOOSH_SOCKET_DIR") or "")
    if not d:
        print("usage: smoke-check.py --installed | <run-dir>   (run it inside scripts/launch-one.sh)")
        return 2
    app, broot, idir = shell_env(ROOT)
    installed = os.path.join(idir, app + ".app")
    rig_app = os.path.join(d, "rig", app + ".app")
    under = bundle_under_test(installed)
    judge_stamps(under, installed, rig_app, broot, d, mutant)

    # ---- launch: the socket answers
    ctl = Ctl(d)
    try:
        ctl.connect()
        verbs = ctl.send("verbs")
        have = verbs.get("verbs") or []
        missing = [v for v in VERB_NEEDED if v not in have]
        record("launch-socket-answers", verbs.get("ok") is True and not missing, "verbs missing: %s" % (missing or "none"))
    except (OSError, RuntimeError, ValueError) as e:
        record("launch-socket-answers", False, "no drive channel: %s" % e)
        return finish(None)

    # ---- first frame: one visible window, a first paint, a picture of it
    wf = ctl.send("winframes")
    # The menu bar's status item is a window of the app too (title "Item-0"); the first frame is the app's own windows.
    frames = [f for f in (wf.get("frames") or []) if f.get("kind") != "status-item"]
    n_want = 2 if mutant == "two-windows-expected" else 1
    record("first-frame-one-window", wf.get("ok") is True and len(frames) == n_want and all(f.get("w", 0) > 0 and f.get("h", 0) > 0 for f in frames),
           "frames %s" % [(f.get("title"), f.get("w"), f.get("h")) for f in frames])
    m = ctl.send("measure", "mac-main.launch")
    ms = m.get("ms")
    record("first-paint-measured", m.get("ok") is True and isinstance(ms, (int, float)) and ms > 0, "process start to first paint %s ms (printed, judged by perf/launch.py)" % ms)
    shot = ctl.send("shot", "smoke/first-frame" if mutant == "shot-refused" else "smoke-first-frame")
    spath = shot.get("path") or ""
    if "web: not ready" in str(shot.get("source", "")):
        # the picture was taken before the reader's page was ready (a slow first body on a loaded Mac): the first frame is judged by window and rows above, so this is a NOTE
        notes.append("first-frame picture: the shot says the page was not ready (%s); the window and rows above are the verdict" % shot.get("source"))
    record("first-frame-picture-file", shot.get("ok") is True and png_ok(spath), "shot %s" % spath)
    if png_ok(spath):
        os.makedirs(CAPTURE_DIR, exist_ok=True)
        shutil.copyfile(spath, os.path.join(CAPTURE_DIR, "smoke-first-frame.png"))
        pc = picture_content(spath, d)
        if pc is None:
            notes.append("first-frame picture could not be read back for a content count")
        else:
            notes.append("first-frame picture: %d of %d pixels opaque in a 128-px copy, %d distinct colours%s. The first frame is judged above by window and rows, not by pixels."
                         % (pc[1], pc[2], pc[0], " (a picture with no opaque pixel shows nothing: the shot verb drew no content, so the pixels of the installed copy need a screenshot of its real window)" if pc[1] == 0 else ""))

    # ---- one archive over the socket
    ui = ctl.send("uistate").get("state") or {}
    core = ui.get("core")
    region = "mac-main.list.missing" if mutant == "list-region-missing" else "mac-main.list"
    rows = ctl.send("rows", region)
    ids = [r.get("id") for r in (rows.get("rows") or [])]
    record("inbox-list-has-rows", rows.get("ok") is True and len(ids) > 0, "region %s: %d rows, core %s" % (region, len(ids), core))
    judgeable = isinstance(core, str) and "unavailable" not in core.lower() and "empty" not in core.lower()
    if not ids:
        record("archive-over-socket", False, "no rows to archive")
    elif not judgeable:
        record("archive-over-socket", False, "the core is %r: start the copy on the fixture core (LAUNCH_ENV=WHOOSH_FIXTURE_CORE=1)" % core, judged=False)
    else:
        target = ui.get("focus_message") if ui.get("focus_message") in ids else ids[0]
        if target != ui.get("focus_message"):
            for _ in range(len(ids)):
                if (ctl.send("uistate").get("state") or {}).get("focus_message") == target:
                    break
                ctl.send("key", "J" if ids.index(target) > 0 else "K")
        before = ctl.send("core", "state", "message/%s/archived" % target).get("value")
        acted = ctl.send("key", "H" if mutant == "archive-presses-snooze" else "E")
        after_ids = [r.get("id") for r in (ctl.send("rows", "mac-main.list").get("rows") or [])]
        flag = ctl.send("core", "state", "message/%s/archived" % target).get("value")
        record("archive-over-socket",
               acted.get("ok") is True and before in (False, "false", 0) and flag in (True, "true", 1) and target not in after_ids and len(after_ids) == len(ids) - 1,
               "%s: archived %s -> %s, rows %d -> %d, key reply %s" % (target, before, flag, len(ids), len(after_ids), acted.get("ok")))

    # ---- quit: named here, performed by launch-one.sh, watched
    pid = instance_pid(rig_app)
    if pid is None:
        record("quit-instance-named", False, "no process runs from %s" % rig_app)
        return finish(None)
    out = watch_quit(pid)
    record("quit-instance-named", True, "pid %d; the watcher writes %s once launch-one.sh stops it" % (pid, os.path.relpath(out, ROOT)))
    return finish(pid)


def finish(_pid):
    for name, verdict, detail in results:
        print("%-11s %-36s %s" % (verdict, name, detail))
    n = len(results)
    f = sum(1 for r in results if r[1] == "FAIL")
    u = sum(1 for r in results if r[1] == "NOT JUDGED")
    for note in notes:
        print("NOTE        " + note)
    print()
    print("MUTANTS (a check is green only after it has been seen red on its mutant; python3 checks/smoke-check.py <run-dir> --mutant NAME)")
    print("| mutant | what it changes |")
    print("|---|---|")
    for k, v in MUTANTS.items():
        print("| %s | %s |" % (k, v))
    print("%d checks, %d failed, %d not judged" % (n, f, u))
    return 0 if (n > 0 and f == 0 and u == 0) else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
