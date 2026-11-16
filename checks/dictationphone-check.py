#!/usr/bin/env python3
"""dictationphone-check: dictation on the iPhone, fed by the recorded audio fixture; no microphone is ever asked.

    python3 checks/dictationphone-check.py --live        drive the running iPhone app ($WHOOSH_SOCKET_DIR), after the source case
    python3 checks/dictationphone-check.py               the source case only (no app, no socket)
    python3 checks/dictationphone-check.py --selftest    a stand-in socket: every case green on the clean model, red on its own broken copy; launches nothing
Run live (the lead):  WHOOSH_RIG_HIDDEN=1 sh scripts/iphone-rig.sh "$IPHONE_APP" -- python3 checks/dictationphone-check.py --live
Prints one line per case (PASS / FAIL / NOT JUDGED), then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0.

Item e2dd9288 (fn-dictation-ios), one named case per fact:
  model-absent-before-first-use     a fresh app (a fresh install: scripts/iphone-rig.sh uninstalls the app before every run) has no speech model: row iphone-composer.speech-model says missing, the microphone says idle
  model-present-after-first-use     the first tap on the microphone downloads it (downloading, then ready) and listening starts
  text-arrives-from-the-fixture-audio  `fixture speak-file <wav> <words>` (the recorded fixture, fixtures/tests-j3.json dictation.audio) and a second tap:
                                    the microphone says text and the draft body holds the cleaned words (not the raw ones)
  audio-file-gone-after-text        the wav exists while the key is held, is gone from the disk afterwards (the simulator shares the host's file system,
                                    so the host looks at the same path) and `rows mac-composer.dictation` counts 0 audio files
  no-microphone-request             nothing asks iOS for the microphone in the test copy: (a) source: the rig's recorder (FixtureRecorder) holds no
                                    AVAudioSession, AVAudioApplication or AVCaptureDevice call, and the phone's own session over MicRecorder is built
                                    only when DictationRig.active is false (Sources/Features/ComposerPhone/PhoneSpeech.swift controller); (b) live: after the
                                    whole run the dictation line is empty (a refused or asked microphone leaves a line)
LIMIT (measured by reading, not by the run): under the rig the "model" is a stand-in state that counts up on a timer (PhoneSpeech.swift rigSteps/rigStepMs);
no model FILE exists in the test copy. The file's own rules (absent before the first use, present after, no .part after a failed download) are the unit
tests of Tests/Unit/DictationModelStoreTests.swift (proof 4 of item e2dd9288). The real engine and a real microphone are a real iPhone's and are not here.
The stand-in of --selftest repeats the intended rules in Python so the cases can be proved able to fail; it is not evidence about the app.
"""
import json
import os
import re
import shutil
import socket
import sys
import tempfile
import threading
import time
import base64

sys.dont_write_bytecode = True
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable      # noqa: E402
from lib import phonestart                              # noqa: E402

with open(os.path.join(ROOT, "fixtures", "tests-j3.json"), encoding="utf-8") as _f:
    J3 = json.load(_f)
DICT = J3["dictation"]
AUDIO = DICT["audio"]
UTT = {u["id"]: u for u in DICT["utterances"]}[AUDIO["utterance"]]
C = "iphone-composer"
ROW_MODEL, ROW_DICT, ROW_BODY, ROW_HEAD, ROW_SESSION = C + ".speech-model", C + ".dictate", C + ".body", C + ".header", "mac-composer.dictation"
CASES = ["model-absent-before-first-use", "model-present-after-first-use", "text-arrives-from-the-fixture-audio", "audio-file-gone-after-text", "no-microphone-request"]
SETTLE_S = 10.0

# the source files the source case reads, as {key: text}; the selftest hands it copies with one edit each
SOURCES = {"recorders": "Sources/Features/Dictation/Core/Recorders.swift", "phonespeech": "Sources/Features/ComposerPhone/PhoneSpeech.swift"}
ASKS = re.compile(r"AVAudioSession|AVAudioApplication|AVCaptureDevice|requestRecordPermission|requestAccess")


def load_sources():
    return {k: open(os.path.join(ROOT, p), encoding="utf-8").read() for k, p in SOURCES.items()}


def judge_source(src):
    """(ok, why): the rig's recorder asks for nothing, and the phone's own microphone session is built only outside the rig."""
    rec = src["recorders"]
    m = re.search(r"final class FixtureRecorder\b.*?\n\}\n", rec, re.S)
    if not m:
        return False, "FixtureRecorder is not in Recorders.swift"
    hit = ASKS.search(m.group(0))
    if hit:
        return False, "FixtureRecorder calls %s" % hit.group(0)
    ps = src["phonespeech"]
    if not re.search(r"if !DictationRig\.active \{ return phoneSession \}", ps):
        return False, "PhoneSpeech.controller builds the phone's microphone session without the DictationRig.active guard"
    if len(re.findall(r"MicRecorder\(\)", ps)) != 1 or not re.search(r"phoneSession: DictationController = \{\s*\n\s*let c = DictationController\(recorder: MicRecorder\(\)", ps):
        return False, "MicRecorder() is built somewhere other than the phone session"
    return True, "FixtureRecorder asks for nothing; MicRecorder() only in the phone session, behind !DictationRig.active"


def norm(s):
    return re.sub(r"[^a-z0-9 ]", "", (s or "").lower()).strip()


def settle(fn, want, limit=SETTLE_S):
    end = time.time() + limit
    got = fn()
    while got != want and time.time() < end:
        time.sleep(0.1)
        got = fn()
    return got


INBOX_WAIT_S = 8.0


def to_inbox(ctl, limit=None):
    """A test copy starts its first launch on the welcome, and `fixture reset` leaves the surface at another word for a moment (phone4: palettephone, the
    thread, the composer and the shots all read it): leave the welcome through its own controls (journeys/lib/phonestart.py), wait for the inbox."""
    limit = INBOX_WAIT_S if limit is None else limit
    end = time.time() + limit
    surface = ctl.uistate().get("surface")
    while surface != "iphone-inbox" and time.time() < end:
        if surface == phonestart.WELCOME:
            phonestart.leave_welcome(ctl)
        time.sleep(0.2)
        surface = ctl.uistate().get("surface")
    if surface != "iphone-inbox":
        raise Unreadable("the app is on %r, not the inbox, after %ds" % (surface, limit))


def one(ctl, ax):
    rows = ctl.rows(ax)
    if not rows:
        raise Unreadable("no row at %s" % ax)
    return rows[0]


def live(ctl, work, src):
    """Every case in order. Returns [(name, verdict, why)]; a step the app refuses makes the cases that need it NOT JUDGED naming the step."""
    out = {}

    def put(name, ok, why):
        out[name] = ("PASS" if ok else "FAIL", why)

    def nj(names, why):
        for n in names:
            out.setdefault(n, ("NOT JUDGED", why))

    wav = os.path.join(work, "dictation-fixture.wav")
    with open(wav, "wb") as f:
        f.write(base64.b64decode(AUDIO["base64"]))
    try:
        # No `fixture reset`: the rig starts the app from a fresh install (scripts/iphone-rig.sh uninstalls first), so the model is already missing, and on the phone a
        # reset leaves the model's surface at 'mac-main' for good when the stack did not move (PhoneRouter.swift send(): `guard nav != before`; phone5: 8 s on mac-main)
        to_inbox(ctl)
        ctl.send("click", "iphone-inbox.list.row.0")
        ctl.send("click", "iphone-thread.reply")
        # the Reply door sets the overlay; the model opens when the sheet appears (ComposerPhoneState.appeared, ComposerPhoneScreen.swift:42): wait for it
        if not settle(lambda: bool(ctl.rows(ROW_HEAD) and ctl.rows(ROW_HEAD)[0].get("is_open")), True):
            raise Unreadable("the composer did not open within %ds (the sheet never appeared: on %r)" % (SETTLE_S, ctl.uistate().get("surface")))
    except Unreadable as e:
        nj(CASES, "setup refused: %s" % e)
        return out
    # 1. no model before the first use
    try:
        m, d = one(ctl, ROW_MODEL)["state"], one(ctl, ROW_DICT)["state"]
        put("model-absent-before-first-use", (m, d) == ("missing", "idle"), "model %r, microphone %r (want missing, idle)" % (m, d))
    except (Unreadable, KeyError) as e:
        nj(["model-absent-before-first-use"], "row unreadable: %s" % e)
    # 2. the first tap downloads it, then listens
    try:
        ctl.send("click", ROW_DICT)
        first = one(ctl, ROW_DICT)["state"]
        ready = settle(lambda: one(ctl, ROW_MODEL)["state"], "ready")
        listening = settle(lambda: one(ctl, ROW_DICT)["state"], "listening")
        put("model-present-after-first-use", first == "downloading" and ready == "ready" and listening == "listening",
            "after the tap %r, then model %r, then microphone %r (want downloading, ready, listening)" % (first, ready, listening))
    except (Unreadable, KeyError) as e:
        nj(["model-present-after-first-use", "text-arrives-from-the-fixture-audio", "audio-file-gone-after-text"], "the first tap was refused or unreadable: %s" % e)
        listening = None
    # 3 and 4. the fixture audio becomes text; the file is gone
    if listening == "listening":
        try:
            ctl.send("fixture", "speak-file", wav, UTT["raw"])
            during = os.path.exists(wav)
            ctl.send("click", ROW_DICT)
            state = settle(lambda: one(ctl, ROW_DICT)["state"], "text")
            body = one(ctl, ROW_BODY)
            put("text-arrives-from-the-fixture-audio", state == "text" and norm(UTT["cleaned"]) in norm(body.get("text")) and norm(body.get("dictated")) == norm(UTT["cleaned"]),
                "microphone %r; body %r; dictated %r (want text, the cleaned words %r)" % (state, body.get("text"), body.get("dictated"), UTT["cleaned"]))
            files = one(ctl, ROW_SESSION).get("audio_files")
            gone = settle(lambda: os.path.exists(wav), False, 3.0)
            put("audio-file-gone-after-text", (during, gone, files) == (True, False, 0),
                "exists while held %s, exists after %s, audio_files %s (want True, False, 0)" % (during, gone, files))
        except (Unreadable, KeyError) as e:
            nj(["text-arrives-from-the-fixture-audio", "audio-file-gone-after-text"], "a step was refused or unreadable: %s" % e)
    elif listening is not None:
        nj(["text-arrives-from-the-fixture-audio", "audio-file-gone-after-text"], "the microphone never listened (state %r), so there was nothing to speak into" % listening)
    # 5. no microphone request
    ok_src, why_src = judge_source(src)
    try:
        line = one(ctl, ROW_SESSION).get("line", "")
        put("no-microphone-request", ok_src and line == "", "%s; dictation line %r (want empty)" % (why_src, line))
    except (Unreadable, KeyError) as e:
        nj(["no-microphone-request"], "the dictation row is unreadable: %s" % e)
    return out


def report(out, source_only=None):
    n = f = u = 0
    for name in CASES:
        v, why = out.get(name) or source_only.get(name) or ("NOT JUDGED", "not run")
        n += 1; f += v == "FAIL"; u += v == "NOT JUDGED"
        print("%s %s %s" % (v, name, why))
    print("%d checks, %d failed, %d not judged" % (n, f, u))
    return 0 if f == 0 and u == 0 else 1


# ---------------------------------------------------------------------------------------------- the stand-in (--selftest)
class Model:
    """The specified phone, with one broken copy per case (MUTANTS)."""
    def __init__(self, mutant, work):
        self.mutant, self.work, self.start = mutant, work, "iphone-welcome-accounts"
        self.reset()

    def reset(self):
        self.surface, self.reads, self.t_reply = "mac-main", 0, None
        self.model = "ready" if self.mutant == "model-present-at-start" else "missing"
        self.open = False
        self.mic, self.t0, self.heard, self.audio, self.body, self.dictated, self.line, self.open = "idle", 0, "", None, "", "", "", False

    def surf(self):
        self.reads += 1
        if self.surface == "mac-main" and self.reads > 2: self.surface = self.start
        return self.surface

    def tick(self):
        if self.mic == "downloading" and self.mutant != "model-never-ready" and time.time() - self.t0 > 0.3:
            self.model, self.mic = "ready", "listening"

    def files(self):
        return 1 if self.audio and os.path.exists(self.audio) else 0

    def handle(self, verb, a):
        self.tick()
        if verb == "fixture":
            if a[0] == "reset":
                self.reset()
                if self.mutant == "reset-strands-the-surface": self.start = "mac-main"
            elif a[0] == "speech-model": self.model = a[1]
            elif a[0] == "speak-file": self.audio, self.heard = a[1], " ".join(a[2:])
            return {"ok": True}
        if verb == "type":
            return {"ok": True}
        if verb == "uistate":
            return {"ok": True, "state": {"surface": self.surf()}}
        if verb == "click":
            if a[0] == "iphone-welcome.not-now" and self.mutant != "stuck-welcome": self.start = self.surface = "iphone-inbox"
            if a[0] == "iphone-thread.reply": self.t_reply = time.time()
            elif a[0] == ROW_DICT:
                if self.mic == "listening":
                    self.mic = "text"
                    if self.mutant != "no-text":
                        words = UTT["cleaned"] if self.heard == UTT["raw"] else self.heard
                        self.body, self.dictated = self.body + words, words
                    if self.audio and os.path.exists(self.audio) and self.mutant != "audio-kept":
                        os.remove(self.audio)
                    if self.mutant == "mic-requested":
                        self.line = "The microphone is off for Whoosh."
                elif self.model == "ready": self.mic = "listening"
                elif self.mic == "idle": self.mic, self.t0 = "downloading", time.time()
            return {"ok": True}
        if verb == "rows":
            ax = a[0]
            if ax == ROW_HEAD: return {"ok": True, "rows": [{"is_open": self.t_reply is not None and time.time() - self.t_reply > 0.4}]}   # the sheet appears a moment after the door
            if ax == ROW_MODEL: return {"ok": True, "rows": [{"state": "downloading" if self.mic == "downloading" else self.model}]}
            if ax == ROW_DICT: return {"ok": True, "rows": [{"state": self.mic, "line": self.line}]}
            if ax == ROW_BODY: return {"ok": True, "rows": [{"text": self.body, "dictated": self.dictated}]}
            if ax == ROW_SESSION: return {"ok": True, "rows": [{"audio_files": self.files(), "line": self.line}]}
            return {"ok": True, "rows": []}
        return {"ok": False, "error": "unknown verb %s" % verb}


MUTANTS = {"model-present-at-start": "model-absent-before-first-use", "model-never-ready": "model-present-after-first-use", "no-text": "text-arrives-from-the-fixture-audio",
           "audio-kept": "audio-file-gone-after-text", "mic-requested": "no-microphone-request"}
# broken copies of the SOURCES for the source half of no-microphone-request: (key, regex, replacement)
SOURCE_MUTANTS = [("recorders", r"(public func authorize\(\) async -> Bool \{ )", r"\1_ = AVAudioSession.sharedInstance(); "),
                  ("phonespeech", r"if !DictationRig\.active \{ return phoneSession \}", "return phoneSession")]


def serve_model(model, base):
    path = os.path.join(base, "whoosh.sock")
    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM); srv.bind(path); srv.listen(4)
    def loop():
        while True:
            try: c, _ = srv.accept()
            except OSError: return
            buf = b""
            while True:
                try: d = c.recv(65536)
                except OSError: break
                if not d: break
                buf += d
                while b"\n" in buf:
                    line, buf = buf.split(b"\n", 1)
                    q = json.loads(line.decode())
                    c.sendall((json.dumps(model.handle(q["verb"], q.get("args", []))) + "\n").encode())
            c.close()
    threading.Thread(target=loop, daemon=True).start()
    return srv


def run_live(model_mutant, src, base):
    work = tempfile.mkdtemp(prefix="w", dir=base)
    model = Model(model_mutant, work)
    srv = serve_model(model, work)
    ctl = Ctl(work).connect()
    try:
        return live(ctl, work, src)
    finally:
        ctl.close(); srv.close()


def selftest():
    base = tempfile.mkdtemp(prefix="dph.", dir="/tmp")          # short: a unix socket path is limited to 104 bytes
    n = bad = 0
    def ok(name, cond):
        nonlocal n, bad
        n += 1; bad += not cond
        print(("PASS " if cond else "FAIL ") + name)
    src = load_sources()
    ok("the real sources pass the source half", judge_source(src)[0])
    clean = run_live(None, src, base)
    ok("clean model: every case green", all(clean.get(c, ("", ""))[0] == "PASS" for c in CASES))
    for mut, case in MUTANTS.items():
        r = run_live(mut, src, base)
        ok("broken copy %s turns %s red" % (mut, case), r.get(case, ("", ""))[0] == "FAIL")
    strand = run_live("reset-strands-the-surface", src, base)
    ok("the check never sends fixture reset: a reset that strands the surface on mac-main changes nothing", all(strand.get(c, ("", ""))[0] == "PASS" for c in CASES))
    global INBOX_WAIT_S
    INBOX_WAIT_S = 2.0
    stuck = run_live("stuck-welcome", src, base)
    ok("a copy that never leaves the welcome is NOT JUDGED on every case, naming the surface", all(stuck.get(c, ("", ""))[0] == "NOT JUDGED" for c in CASES) and "iphone-welcome-accounts" in stuck[CASES[0]][1])
    for key, pat, rep in SOURCE_MUTANTS:
        broken = dict(src); broken[key] = re.sub(pat, rep, src[key], count=1)
        ok("source broken copy (%s) turns the source half red" % key, broken[key] != src[key] and not judge_source(broken)[0])
    nolive = Ctl(base)
    try:
        nolive.connect(); gone = False
    except CouldNotDrive:
        gone = True
    ok("no socket is could-not-drive, not a pass", gone or os.path.exists(os.path.join(base, "whoosh.sock")))
    shutil.rmtree(base, ignore_errors=True)
    print("selftest: %d cases, %d failed" % (n, bad))
    return 0 if bad == 0 else 1


def main(argv):
    if "--selftest" in argv:
        return selftest()
    src = load_sources()
    if "--live" not in argv:
        ok_src, why = judge_source(src)
        return _source_only(ok_src, why)
    d = os.environ.get("WHOOSH_SOCKET_DIR", "")
    try:
        ctl = Ctl(d).connect()
    except CouldNotDrive as e:
        print("NOT JUDGED all: %s" % e)
        print("%d checks, 0 failed, %d not judged" % (len(CASES), len(CASES)))
        return 1
    work = tempfile.mkdtemp(prefix="dph.", dir=d)
    try:
        return report(live(ctl, work, src))
    finally:
        ctl.close(); shutil.rmtree(work, ignore_errors=True)


def _source_only(ok_src, why):
    print("%s no-microphone-request (source half only) %s" % ("PASS" if ok_src else "FAIL", why))
    print("1 checks, %d failed, 0 not judged" % (0 if ok_src else 1))
    return 0 if ok_src else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
