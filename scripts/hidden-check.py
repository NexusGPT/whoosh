#!/usr/bin/env python3
"""The hidden rig's proof: while a hidden rig copy is driven through every verb that opens a window, the WINDOW SERVER
shows none of its windows, the app never becomes the front app, and it never gets a Dock tile; and the drive itself
still works (keys move the focus, rows answer, frames measure, shots hold a picture).

Run (every rig copy is hidden; checks/rig-hidden-check.py is the proof, this is the wider drive under its watch):
    sh scripts/launch-one.sh <bundle> -- --fixture-core python3 scripts/hidden-check.py
The instrument is scripts/rig-windows.swift, compiled once into $TMPDIR, sampling the window server from OUTSIDE the
app every 20 ms for the whole drive (a window that flashes between two steps is caught too). A window is SHOWN when
the window server has it on screen, at an alpha above zero, over an active display.

Other checks can run under the same watch, before the drive above (their own lines are printed, their exit codes noted;
they are judged by themselves, not here):
    ... python3 scripts/hidden-check.py -- python3 checks/parity/par-macshell.py ';;' python3 other-check.py
Prints 'N checks, F failed, U not judged'; exit 1 when F > 0, U > 0 or N = 0. Writes the samples beside the shots in
_captures/appshell/r3/ (hidden-watch.jsonl, hidden-check.txt).

Limit of the shots this drive takes (docs/CONTROL-SOCKET.md delta 49): a hidden shot renders the view tree, which draws
no NSVisualEffectView material, so a glass surface comes out empty and its text sits bare over what is behind it. The
designtokens lane's rig-mode stand-in in WhooshGlass fills a glass panel in hidden mode so its bounds show; that fill
is not the product's look. `shot-has-a-picture` and `shot-<surface>` prove a picture exists, never the material: it is
judged on the owner's screen.
"""
import hashlib
import json
import os
import socket
import subprocess
import sys
import tempfile
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
import importlib.util as _ilu
_sw = _ilu.spec_from_file_location("shotwait", os.path.join(ROOT, "journeys", "lib", "shotwait.py")); shotwait = _ilu.module_from_spec(_sw); _sw.loader.exec_module(shotwait)   # how long a `shot` reply may take (one place)
OUT = os.path.join(ROOT, "_captures", "appshell", "r3")
SURFACES = ["mac-settings-general", "mac-settings-accounts", "mac-labels-contacts", "mac-quickreply"]

# MUTANTS: per check, the mutation of the app that must turn it red.
# no-window-shown | WHOOSH_RIG_HIDDEN_MUTANT=alpha-1-255 counts as shown only when alpha > 0: see checks/rig-hidden-check.py (alpha == 0 exactly)
# instrument-sees-the-windows | the helper reads another pid (RIG_PID of a process with no window)
# hidden-mode-on | the app launched without WHOOSH_RIG_HIDDEN (refused by launch-one since main ae0a169)
# never-front | (no mutant yet: activating the rig would take the owner's keyboard; asked of the lead)
# no-dock-tile | (no mutant yet: a Dock tile on the owner's screen; asked of the lead)

results = []


def record(name, ok, detail="", judged=True):
    results.append((name, ("PASS" if ok else "FAIL") if judged else "NOT JUDGED", detail))


class Ctl:
    def __init__(self, path):
        self.s = socket.socket(socket.AF_UNIX)
        self.s.settimeout(30)
        self.s.connect(path)
        self.f = self.s.makefile("rwb")

    def send(self, verb, *args):
        self.s.settimeout(shotwait.timeout_for(verb, args, 30))
        self.f.write((json.dumps({"verb": verb, "args": [str(a) for a in args]}) + "\n").encode())
        self.f.flush()
        return json.loads(self.f.readline().decode() or "{}")


def helper():
    src = os.path.join(ROOT, "scripts", "rig-windows.swift")
    tag = hashlib.sha256(open(src, "rb").read()).hexdigest()[:12]
    exe = os.path.join(tempfile.gettempdir(), "whoosh-rig-windows-" + tag)
    if not os.path.exists(exe):
        r = subprocess.run(["swiftc", "-O", src, "-o", exe], capture_output=True, text=True)
        if r.returncode != 0:
            raise RuntimeError("the window reader did not compile: " + r.stderr[-400:])
    return exe


def png_colours(path):
    """Distinct colours in a 64-px copy (sips), or 0 when unreadable: a blank picture has one or two."""
    small = path + ".small.bmp"
    if subprocess.run(["sips", "-Z", "64", "-s", "format", "bmp", path, "--out", small], capture_output=True).returncode != 0:
        return 0
    data = open(small, "rb").read()
    off = int.from_bytes(data[10:14], "little")
    return len({data[i:i + 3] for i in range(off, len(data) - 3, 4)})


def drive(c, exe, pid):
    st = c.send("uistate").get("state", {})
    record("hidden-mode-on", (st.get("hidden") or {}).get("on") is True, "uistate.hidden %s" % st.get("hidden"))
    frames = c.send("winframes").get("frames") or []
    record("app-has-windows", len(frames) >= 1, "winframes %s" % [(f.get("title"), f.get("kind")) for f in frames])
    rows = c.send("rows", "mac-main.list").get("rows") or []
    record("rows-answer", len(rows) > 0, "%d rows" % len(rows))
    before = st.get("focus_index")
    c.send("key", "J")
    after = c.send("uistate").get("state", {}).get("focus_index")
    record("key-moves-focus", after != before, "focus %s -> %s" % (before, after))
    m = c.send("measure", "mac-main.sidebar")
    record("measure-answers", m.get("ok") is True and (m.get("frame") or {}).get("w", 0) > 0, str(m.get("frame") or m.get("error")))
    shot = c.send("shot", "hidden-main")
    n = png_colours(shot.get("path", "")) if shot.get("ok") else 0
    record("shot-has-a-picture", n > 16, "%s source %s, %d colours" % (shot.get("path"), shot.get("source"), n))
    for s in SURFACES:   # every one opens a window of its own
        r = c.send("shot", s)
        record("shot-" + s, r.get("ok") is True and png_colours(r.get("path", "")) > 16, str(r.get("source") or r.get("error")))
    for step in [("click", "mac-main.status"), ("click", "mac-main.message.window"), ("click", "mac-notch.stage.sample"),
                 ("click", "macshell.dock.toggle"), ("activate",), ("key", "R"), ("type", "hidden check"), ("key", "⌘Enter"),
                 ("key", "⌘K"), ("key", "Escape"), ("fixture", "reset")]:
        c.send(*step)
        time.sleep(0.3)
    st = c.send("uistate").get("state", {})
    refused = (st.get("hidden") or {}).get("refused") or []
    record("dock-toggle-refused", "dock-tile" in refused, "refused %s" % refused)
    snap = json.loads(subprocess.run([exe, str(pid)], capture_output=True, text=True).stdout or "{}")
    return snap


def judge(watch, pid):
    lines = [json.loads(x) for x in open(watch) if x.strip()]
    samples = [x for x in lines if "windows" in x]
    shown = [x for x in samples if x.get("shown", 0) > 0]
    seen = max((len(x["windows"]) for x in samples), default=0)
    record("instrument-sees-the-windows", seen >= 1, "%d samples, up to %d windows of pid %s on the window server" % (len(samples), seen, pid))
    taken = max((x.get("n", 0) for x in lines), default=0)
    record("no-window-shown", not shown, "%d of %d distinct states (%d samples taken) show a window%s" % (
        len(shown), len(samples), taken, ": %s" % [w for w in shown[0]["windows"] if w["shown"]] if shown else ""))
    record("never-front", all(x.get("front_pid") != pid for x in samples), "front pids %s" % sorted({x.get("front_pid") for x in samples}))
    record("no-dock-tile", not any(x.get("dock_tile") for x in samples), "")


def main():
    d = os.environ.get("D") or os.environ.get("WHOOSH_SOCKET_DIR")
    pid = int(os.environ.get("RIG_PID", "0"))
    if not d or not pid:
        print("run inside scripts/launch-one.sh (needs $D and $RIG_PID)")
        print("0 checks, 0 failed, 1 not judged")
        return 1
    os.makedirs(OUT, exist_ok=True)
    exe = helper()
    watch = os.path.join(OUT, "hidden-watch.jsonl")
    w = subprocess.Popen([exe, str(pid), "--watch", watch, "--every", "20"])
    extra = []
    if "--" in sys.argv:
        words = sys.argv[sys.argv.index("--") + 1:]
        cmd = []
        for x in words + [";;"]:
            if x == ";;":
                if cmd:
                    extra.append(cmd)
                cmd = []
            else:
                cmd.append(x)
    try:
        time.sleep(0.5)
        for cmd in extra:   # under the same watch, each as it would run alone
            r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
            print("---- %s: exit %d" % (" ".join(cmd), r.returncode))
            print((r.stdout + r.stderr).rstrip()[-6000:])
        drive(Ctl(os.path.join(d, "whoosh.sock")), exe, pid)
        time.sleep(0.5)
    finally:
        w.terminate()
        w.wait()
    judge(watch, pid)
    lines = ["%-10s %s  %s" % (v, n, det) for n, v, det in results]
    failed = sum(1 for _, v, _ in results if v == "FAIL")
    unjudged = sum(1 for _, v, _ in results if v == "NOT JUDGED")
    lines.append("%d checks, %d failed, %d not judged" % (len(results), failed, unjudged))
    open(os.path.join(OUT, "hidden-check.txt"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0 if failed == 0 and unjudged == 0 and results else 1


if __name__ == "__main__":
    sys.exit(main())
