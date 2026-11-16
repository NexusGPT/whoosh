#!/usr/bin/env python3
"""dictation-check: dictation on the Mac, fed by a recorded audio fixture; no microphone is ever needed.

    python3 checks/dictation-check.py              drive the running rig copy ($WHOOSH_SOCKET_DIR); exit 1 when F > 0 or U > 0
    python3 checks/dictation-check.py --selftest   baseline green and one red per MUTANT, on the model below (every mutant must go red)

Acceptance (PRD dictation, docs/plan.json fn-dictation-mac), one line each, each with its check:
  text at the cursor of the focused field     cleaned-u1, cleaned-u2 (composer body) · text-in-quick-reply · text-in-search · dictation-caret-insert
  nothing typed when no field holds the keys  no-field-types-nothing (the check lets go of a field an earlier check left open)
  the audio file is gone afterwards           audio-file-gone-after-text · audio-file-gone-after-failure
  a refused microphone shows the how-to line  mic-refused-line-explains · refused-line-in-app-language
  release to text under DICTATION_MS          release-to-text-under-budget (the ledger's number, read from docs/CONSTANTS.md)
  the clean-up pass, on this device           cleaner-i-and-months-english · cleaner-keeps-legit-you-know · cleaner-other-language-untouched
  one session at a time                       double-fn-down-one-session
  an empty or keyless utterance inserts none  dictation-empty-utterance-inserts-nothing · cloud-key-missing-line-names-setting

Verbs used (the app serves them under --fixture-core; `speak-file`, `mic-refuse`, `cloud-no-key` and `caret` need one entry each in
FixtureEvents.served, see docs/lanes/dictation.md "## Round 2"): fixture reset | speak <words> | speak-file <path> <words> | mic-refuse |
cloud-no-key | caret <n>; key Fn:down, Fn:up; rows mac-composer.dictation (state, live, line, engine, latency_ms, audio_files),
mac-composer.body, mac-main.quickreply.field, mac-search.bar; click mac-settings-general.appearance.language.<code>.
The model below is the SPECIFIED behaviour, so every check can be run red on a deliberate mutation without launching anything.
It measures the pipeline, not the recogniser: the fixture engine returns the words it is told, so a latency read here is the overhead
of keys, clean-up and insertion; the real engine's figure is read on a rig copy with the real model (docs/lanes/dictation.md).
"""
import base64
import importlib.util
import json
import os
import shutil
import sys
import tempfile
import time

_p = os.path.join(os.path.dirname(os.path.abspath(__file__)), "composermac-check.py")
_s = importlib.util.spec_from_file_location("composermac_check", _p)
H = importlib.util.module_from_spec(_s)
sys.modules["composermac_check"] = H
_s.loader.exec_module(H)
T, UTT, BODY, compose, tolerate, Unreadable, Err = H.T, H.UTT, H.BODY, H.compose, H.tolerate, H.Unreadable, H.Err
ROOT = H.ROOT

CATALOGUE = json.load(open(os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings"), encoding="utf-8"))["strings"]
K_MIC = "The microphone is off for Whoosh. Open System Settings, Privacy and Security, Microphone, and turn Whoosh on."
K_CLOUD = "Add your cloud speech key in Settings: AI providers, speech and agents."
K_NOFIELD = "Dictation found no text field to type into."
LANG_ROW = "mac-settings-general.languages"  # the rows id the app serves (SettingsGeneralMacFeature: "\(P).languages"); main52: the old ".appearance." id judged nothing
LANG_CLICK = "mac-settings-general.appearance.language."


def words(key, lang):
    """The catalogue's text for a key in a language: what the app shows when it speaks that language."""
    return CATALOGUE[key]["localizations"][lang]["stringUnit"]["value"]


# ---------------------------------------------------------------- the clean-up rule, as specified (LocalCleaner.swift)
FILLERS = {"um", "umm", "uh", "uhh", "uhm", "erm", "hmm"}
PREPOSITIONS = {"in", "on", "by", "until", "till", "from", "since", "during", "before", "after", "of", "this", "next", "last", "early", "late", "mid"}
ALWAYS = {"january", "february", "april", "june", "july", "august", "september", "october", "november", "december",
          "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"}
ENDERS = ".!?…。！？"


def bare(w):
    return w.strip(".,;:!?\"'()[]{}…").lower()


def clean(raw, lang, mutants=()):
    ws = raw.split()
    english = lang == "en" or "cleaner-other-language-untouched" in mutants
    if english:
        if len(ws) > 2 and bare(ws[0]) == "you" and bare(ws[1]) == "know":
            ws = ws[2:]
        if "cleaner-keeps-legit-you-know" in mutants:
            out, i = [], 0
            while i < len(ws):
                if i + 1 < len(ws) and bare(ws[i]) == "you" and bare(ws[i + 1]) == "know":
                    i += 2
                    continue
                out.append(ws[i])
                i += 1
            ws = out
        ws = [w for w in ws if bare(w) not in FILLERS]
    if not ws:
        return ""
    if english:
        for i, w in enumerate(ws):
            b = bare(w)
            up = b == "i" or b in ("i'm", "i'll", "i've", "i'd")
            if "cleaner-i-and-months-english" in mutants:
                up = False
            up = up or b in ALWAYS
            if b in ("march", "may"):
                before = bare(ws[i - 1]) if i > 0 else ""
                after = ws[i + 1] if i + 1 < len(ws) else ""
                up = up or before in PREPOSITIONS or after[:1].isdigit()
            if up:
                ws[i] = w.replace(b, b[:1].upper() + b[1:], 1) if b in w else w
    text = " ".join(ws)
    out, start = [], True
    for ch in text:
        if start and ch.isalpha():
            out.append(ch.upper())
            start = False
            continue
        out.append(ch)
        if ch in ENDERS:
            start = True
        elif not ch.isspace() and ch not in ",;:\"'()":
            start = False
    text = "".join(out)
    if text[-1] not in ENDERS:
        text += "。" if lang in ("ja", "zh-Hans") else "."
    return text


# ---------------------------------------------------------------- the model of the specified behaviour
class DictModel(H.ModelApp):
    def reset(self):
        # The app's `fixture reset` clears the shell, not the screens' own state: the composer keeps its draft (until Discard) and a
        # search or quick reply that was open keeps the keys (until Escape). The model keeps both, so a check that forgets is red here too.
        runs, field, quick, search = getattr(self, "runs", []), getattr(self, "field", None), getattr(self, "quick", ""), getattr(self, "search", "")
        super().reset()
        self.runs = runs
        self.field, self.quick, self.search, self.typed = (field if field in ("search", "quick") else None), quick, search, ""
        if not hasattr(self, "lang"):
            self.lang = "en"          # `fixture reset` does not change the app's language
        self.state, self.live, self.d_line, self.latency = "idle", "", "", None
        self.heard, self.files, self.audio, self.denied, self.nokey = "", [], None, False, False

    def line_words(self, key):
        return words(key, "en" if self.has("refused-line-in-app-language") else self.lang)

    def _drop_audio(self, keep=False):
        if not keep:
            for p in self.files:
                if os.path.exists(p):
                    os.remove(p)
            if self.audio and os.path.exists(self.audio):
                os.remove(self.audio)
        self.files = []

    def v_click(self, ax):
        if ax.startswith(LANG_CLICK):
            self.lang = ax[len(LANG_CLICK):]
            return
        if ax == "mac-composer.discard":
            return
        if ax == "mac-composer.discard.confirm":
            self.set_body("")
            self.surface, self.field = "mac-main", None
            return
        super().v_click(ax)
        if ax == "mac-main.reader.open-full-composer":
            self.field = "composer"

    def v_key(self, chord):
        if chord == "Fn:down":
            return self.fn_down()
        if chord == "Fn:up":
            return self.fn_up()
        if chord == "Escape":
            if self.field in ("search", "quick"):
                self.field, self.search, self.quick, self.surface = None, "", "", "mac-main"
                return
            raise Err("Escape does nothing here")
        if chord == "/":
            self.field, self.surface = "search", "mac-search"
            self.search, self.typed = (self.typed if self.has("no-field-types-nothing") else ""), ""
            return
        kept = self.runs
        super().v_key(chord)
        if chord == "R":
            self.runs = kept                      # opening the reply does not drop the composer's draft
            self.field, self.quick = "quick", ""

    def fn_down(self):
        if self.denied:
            self.d_line = "" if self.has("mic-refused-line-explains") else self.line_words(K_MIC)
            return
        if self.state == "listening":
            if self.has("double-fn-down-one-session"):
                self.files.append(self.files[0] + ".2")
                return
            raise Err("a session is running")
        self.state, self.live, self.d_line, self.heard, self.audio = "listening", "", "", "", None
        path = os.path.join(tempfile.gettempdir(), "whoosh-model-rec-%d.wav" % id(self))
        open(path, "wb").write(b"RIFF")
        self.files = [path]

    def deliver(self, text):
        if self.field == "composer":
            self.set_body(self.body + text) if self.has("dictation-caret-insert") else self.insert(text)
        elif self.field == "quick":
            self.quick += "" if self.has("text-in-quick-reply") else text
        elif self.field == "search":
            self.search += "" if self.has("text-in-search") else text
        elif self.has("no-field-types-nothing"):
            self.typed += text
        else:
            self.d_line = words(K_NOFIELD, self.lang)

    def fn_up(self):
        if self.state != "listening":
            raise Err("not listening")
        self.state = "cleaning"
        if self.nokey:
            self.d_line = "" if self.has("cloud-key-missing-line-names-setting") else self.line_words(K_CLOUD)
            self._drop_audio(keep=self.has("audio-file-gone-after-failure"))
        else:
            raw = self.heard
            if raw.strip():
                text = clean(raw, self.lang, self.m)
                for u in UTT.values():
                    if raw == u["raw"] and self.has("cleaned-" + u["id"]):
                        text = text.rstrip(".")
                self.deliver(text)
            elif self.has("dictation-empty-utterance-inserts-nothing"):
                self.deliver("[empty]")
            self._drop_audio(keep=self.has("audio-file-gone-after-text"))
        self.latency = 5000 if self.has("release-to-text-under-budget") else 40
        self.state, self.live = "idle", ""

    def v_fixture(self, name, *a):
        if name == "speak":
            self.heard = " ".join(a)
            self.live = self.heard
        elif name == "speak-file":
            self.audio, self.heard = a[0], " ".join(a[1:])
            self.live = self.heard
        elif name == "mic-refuse":
            self.denied = True
        elif name == "cloud-no-key":
            self.nokey = True
        else:
            super().v_fixture(name, *a)

    def v_rows(self, ax):
        if ax == "mac-composer.dictation":
            return {"rows": [{"state": self.state, "live": self.live, "line": self.d_line, "engine": "fixture",
                              "latency_ms": self.latency, "audio_files": len(self.files)}]}
        if ax == "mac-main.quickreply.field":
            return {"rows": [{"text": self.quick}] if self.field == "quick" else []}
        if ax == "mac-search.bar":
            return {"rows": [{"text": self.search}] if self.field == "search" else []}
        if ax == LANG_ROW:
            return {"rows": [{"code": c, "current": c == self.lang} for c in ("en", "fr")]}
        return super().v_rows(ax)


# ---------------------------------------------------------------- the checks
REG = H.Registry()
check = REG.check


def wait_for(fn, what, timeout=6.0):
    end = time.time() + timeout
    while True:
        v = fn()
        if v:
            return v
        if time.time() > end:
            raise Unreadable("timed out waiting for %s" % what)
        time.sleep(0.05)


def drow(c):
    return c.rows("mac-composer.dictation")[0]


def press(c, refused=False):
    """Key down; returns once the session is listening or the line says why it is not. `refused`: no session is expected, so a line
    that never comes is a failed check, not a check that could not be read."""
    c.key("Fn:down")
    try:
        wait_for(lambda: drow(c)["state"] == "listening" or drow(c)["line"], "the session to start or refuse", 1.0 if refused else 6.0)
    except Unreadable:
        if not refused:
            raise


def release(c):
    """Key up; returns once the session is idle again (text inserted, or the failure said)."""
    c.key("Fn:up")
    wait_for(lambda: drow(c)["state"] == "idle", "the session to finish")


def speak(c, utt="u1"):
    c.send("fixture", "speak", UTT[utt]["raw"])


def pin(c, code):
    """The app speaks `code`; returns the language it spoke before, so the check can leave the rig as it found it."""
    before = [r["code"] for r in c.rows(LANG_ROW) if r["current"]]
    c.click(LANG_CLICK + code)
    return before[0] if before else "en"


def dictate(c, raw=None, utt="u1"):
    press(c)
    c.send("fixture", "speak", raw if raw is not None else UTT[utt]["raw"])
    release(c)


def fresh(c, text=None):
    """An EMPTY composer: `fixture reset` leaves the draft of record 1 in place, so a later check would dictate after the earlier one's
    words. Discard the draft first, as the owner does (the discard control, then its confirm), then open it again."""
    compose(c)
    rows = c.rows("mac-composer.body")
    if rows and rows[0]["text"]:
        c.click("mac-composer.discard")
        c.click("mac-composer.discard.confirm")
        compose(c)
    if text:
        c.type(text)


def no_field(c):
    """Nothing holds the keys: reset, then Escape lets go of a search or reply a check before left open (its text stays until Escape)."""
    c.send("fixture", "reset")
    for _ in range(2):
        try:
            c.key("Escape")
        except Exception:
            pass


def quick(c):
    c.send("fixture", "reset")
    c.key("R")


@check("cleaned-u1", "the clean-up pass drops the full stop on the first utterance", UTT["u1"]["cleaned"])
def _(c):
    prev = pin(c, "en")
    try:
        fresh(c)
        dictate(c, utt="u1")
        return BODY(c)
    finally:
        c.click(LANG_CLICK + prev)


@check("cleaned-u2", "the clean-up pass drops the full stop on the second utterance", UTT["u2"]["cleaned"])
def _(c):
    prev = pin(c, "en")
    try:
        fresh(c)
        dictate(c, utt="u2")
        return BODY(c)
    finally:
        c.click(LANG_CLICK + prev)


@check("text-in-quick-reply", "the quick reply's field ignores dictated text", UTT["u1"]["cleaned"])
def _(c):
    prev = pin(c, "en")
    try:
        quick(c)
        dictate(c, utt="u1")
        return c.rows("mac-main.quickreply.field")[0]["text"]
    finally:
        c.click(LANG_CLICK + prev)


@check("text-in-search", "the search field ignores dictated text", UTT["u1"]["cleaned"])
def _(c):
    prev = pin(c, "en")
    try:
        c.send("fixture", "reset")
        c.key("/")
        dictate(c, utt="u1")
        return c.rows("mac-search.bar")[0]["text"]
    finally:
        c.click(LANG_CLICK + prev)


@check("no-field-types-nothing", "dictation with no field in focus keeps the words in a buffer a later search shows", (True, ""))
def _(c):
    prev = pin(c, "en")
    try:
        no_field(c)
        dictate(c, utt="u1")
        said = words(K_NOFIELD, "en") in drow(c)["line"]
        c.key("/")
        return (said, c.rows("mac-search.bar")[0]["text"])
    finally:
        c.click(LANG_CLICK + prev)


@check("audio-file-gone-after-text", "the audio file stays on disk after the text appears", (True, False, 0, UTT["u1"]["cleaned"]))
def _(c):
    d = tempfile.mkdtemp(prefix="wa")
    path = os.path.join(d, "a1.wav")
    open(path, "wb").write(base64.b64decode(T["dictation"]["audio"]["base64"]))
    prev = pin(c, "en")
    try:
        fresh(c)
        press(c)
        c.send("fixture", "speak-file", path, UTT["u1"]["raw"])
        during = os.path.exists(path)          # positive control: the probe sees the file while it is held
        release(c)
        return (during, os.path.exists(path), drow(c)["audio_files"], BODY(c))
    finally:
        c.click(LANG_CLICK + prev)
        shutil.rmtree(d, ignore_errors=True)


@check("audio-file-gone-after-failure", "the audio file stays on disk when the engine fails", (True, False, 0))
def _(c):
    d = tempfile.mkdtemp(prefix="wa")
    path = os.path.join(d, "a2.wav")
    open(path, "wb").write(base64.b64decode(T["dictation"]["audio"]["base64"]))
    try:
        fresh(c)
        c.send("fixture", "cloud-no-key")
        press(c)
        c.send("fixture", "speak-file", path, UTT["u1"]["raw"])
        during = os.path.exists(path)
        release(c)
        return (during, os.path.exists(path), drow(c)["audio_files"])
    finally:
        shutil.rmtree(d, ignore_errors=True)


@check("dictation-empty-utterance-inserts-nothing", "an empty utterance inserts a placeholder", ("Keep.", True))
def _(c):
    prev = pin(c, "en")
    try:
        fresh(c, "Keep.")
        dictate(c, raw="")
        first = BODY(c)
        dictate(c, utt="u1")                  # positive control: the same path inserts a real utterance
        b = BODY(c)
        return (first, b.startswith("Keep.") and UTT["u1"]["cleaned"] in b)
    finally:
        c.click(LANG_CLICK + prev)


@check("dictation-caret-insert", "dictated text is appended at the end, not at the caret", (1, True, True))
def _(c):
    prev = pin(c, "en")
    try:
        fresh(c, "AB")
        try:
            c.send("fixture", "caret", 1)
        except Unreadable as e:
            raise Unreadable("fixture caret is not served (shell, FixtureEvents.served): %s" % e)
        dictate(c, utt="u1")
        b = BODY(c)
        return (b.find(UTT["u1"]["cleaned"]), b.startswith("A"), b.endswith("B"))
    finally:
        c.click(LANG_CLICK + prev)


@check("double-fn-down-one-session", "a second Fn press starts a second recording", 1)
def _(c):
    fresh(c)
    press(c)
    tolerate(c.key, "Fn:down")
    speak(c)
    n = drow(c)["audio_files"]
    release(c)
    return n


@check("mic-refused-line-explains", "a refused microphone shows no how-to line", (True, "idle", 0))
def _(c):
    prev = pin(c, "en")
    try:
        fresh(c)
        c.send("fixture", "mic-refuse")
        press(c, refused=True)
        d = drow(c)
        return (all(w in d["line"] for w in T["dictation"]["refused_line_must_name"]), d["state"], d["audio_files"])
    finally:
        c.click(LANG_CLICK + prev)


@check("refused-line-in-app-language", "the how-to line stays English when the app speaks French", (True, True))
def _(c):
    prev = pin(c, "fr")
    try:
        fresh(c)
        c.send("fixture", "mic-refuse")
        press(c, refused=True)
        line = drow(c)["line"]
        return (line == words(K_MIC, "fr"), line != words(K_MIC, "en"))
    finally:
        c.click(LANG_CLICK + prev)


@check("cloud-key-missing-line-names-setting", "a missing cloud key inserts nothing and says nothing", (True, "Keep."))
def _(c):
    prev = pin(c, "en")
    try:
        fresh(c, "Keep.")
        c.send("fixture", "cloud-no-key")
        press(c)
        speak(c)
        release(c)
        line = drow(c)["line"]
        return (all(w in line for w in T["dictation"]["missing_key_line_must_name"]), BODY(c))
    finally:
        c.click(LANG_CLICK + prev)


@check("cleaner-i-and-months-english", "the clean-up leaves 'i' and the months in lower case", "Yes I think we may meet in May on Friday.")
def _(c):
    prev = pin(c, "en")
    try:
        fresh(c)
        dictate(c, raw="yes i think we may meet in may uh on friday")
        return BODY(c)
    finally:
        c.click(LANG_CLICK + prev)


@check("cleaner-keeps-legit-you-know", "the clean-up drops 'you know' from the middle of a sentence", "Do you know the price.")
def _(c):
    prev = pin(c, "en")
    try:
        fresh(c)
        dictate(c, raw="do you know the price um")
        return BODY(c)
    finally:
        c.click(LANG_CLICK + prev)


@check("cleaner-other-language-untouched", "the English filler and 'I' rules run on French words", "Voilà um le prix i.")
def _(c):
    prev = pin(c, "fr")
    try:
        fresh(c)
        dictate(c, raw="voilà um le prix i")
        return BODY(c)
    finally:
        c.click(LANG_CLICK + prev)


@check("release-to-text-under-budget", "the clean-up pass takes longer than the budget", (True, True))
def _(c):
    budget = H.const("DICTATION_MS")
    prev = pin(c, "en")
    try:
        fresh(c)
        dictate(c, utt="u1")
        ms = drow(c)["latency_ms"]
        return (isinstance(ms, (int, float)), isinstance(ms, (int, float)) and ms < budget)
    finally:
        c.click(LANG_CLICK + prev)


MUTANTS = {n: m for n, m, _, _ in REG.items}


def selftest():
    out_dir = os.path.join(ROOT, "_captures", "dictation")
    os.makedirs(out_dir, exist_ok=True)

    def run(ms):
        app = DictModel(ms)
        srv, d = H.serve(app)
        os.environ["WHOOSH_SOCKET_DIR"] = d
        failed, unjudged = set(), set()
        c = None
        try:
            c = H.Ctl().connect()
            for n, st, det in H.run_checks(REG, c):
                (failed if st == "fail" else unjudged if st == "unjudged" else set()).add(n)
                if st != "pass" and not ms:
                    print("baseline %s %s: %s" % (st, n, det), file=sys.stderr)
        finally:
            if c is not None:
                c.close()      # on every path: an open connection keeps its handler thread, and server_close() joins it for ever
            srv.shutdown()
            srv.server_close()
            shutil.rmtree(d, ignore_errors=True)
        return failed, unjudged

    n = len(REG.items)
    base_f, base_u = run(())
    rows = []
    for label, w in MUTANTS.items():
        f, _ = run((label,))
        rows.append((label, w, label in f, sorted(f - {label})[:3]))
    missed = [r for r in rows if not r[2]]
    json.dump({"check": "dictation", "baseline_failed": sorted(base_f), "baseline_unjudged": sorted(base_u),
               "mutants": [{"label": l, "mutation": w, "seen_red": red, "also_red": also} for l, w, red, also in rows]},
              open(os.path.join(out_dir, "dictation-mutants.json"), "w"), indent=1)
    bad = len(base_f) + len(base_u)
    print("%d checks, %d failed, %d not judged" % (n, bad + len(missed), len(base_u)))
    print("baseline on the model: failed=%s unjudged=%s" % (sorted(base_f), sorted(base_u)))
    print("MUTANTS\n| check | mutation made | seen red |")
    for l, w, red, also in rows:
        print("| %s | %s | %s |" % (l, w, "RED on this case" if red else "NOT RED"))
    return 1 if (bad or missed) else 0


def live():
    try:
        ctl = H.Ctl().connect()
        why = ""
    except H.CouldNotDrive as e:
        ctl, why = None, str(e)
    res = H.run_checks(REG, ctl, why)
    f = [r for r in res if r[1] == "fail"]
    u = [r for r in res if r[1] == "unjudged"]
    for n, st, det in res:
        if st != "pass":
            print("%s %s: %s" % ("FAIL" if st == "fail" else "NOT JUDGED", n, det), file=sys.stderr)
    print("%d checks, %d failed, %d not judged" % (len(res), len(f), len(u)))
    p = os.path.join(ROOT, "_captures", "dictation", "dictation-mutants.json")
    seen = {m["label"]: m["seen_red"] for m in json.load(open(p))["mutants"]} if os.path.exists(p) else {}
    print("MUTANTS\n| check | mutation made | seen red (last --selftest) |")
    for l, w in MUTANTS.items():
        print("| %s | %s | %s |" % (l, w, {True: "yes", False: "NO"}.get(seen.get(l), "not run")))
    return 1 if (f or u) else 0


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv[1:] else live())
