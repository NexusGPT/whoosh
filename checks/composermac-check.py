#!/usr/bin/env python3
"""composermac-check: the Mac composer's failure paths and adversarial cases over the control socket.

    python3 checks/composermac-check.py              drive the running rig copy ($WHOOSH_SOCKET_DIR); exit 1 when F > 0 or U > 0
    python3 checks/composermac-check.py --selftest   run every check against ModelApp, once clean (must be green) and once per MUTANT
                                                     (the named check must go red); exit 1 when any mutant is not seen red

Prints `N checks, F failed, U not judged` and a MUTANTS table. docs/TESTING.md: a check is green only after it has been
seen red on its mutant. ModelApp below is a model of the SPECIFIED behaviour (docs/JOURNEYS.md J3, fixtures/tests-j3.json),
not the production app: it exists so that every check, and journeys/J3.py, can be run red on a deliberate mutation
without touching production code. dictation-check.py and richtext-check.py import this file for the harness and the model.
Items the socket contract does not name yet are ASSUMPTIONS (docs/lanes/tests-j3.md): the fixture verbs reset, speak,
speak-file, offline, server-refuse, provider-fail, mic-refuse, cloud-no-key, format, reload-draft, paste-html, caret,
select-all, select-paragraph; the ids mac-composer.*; core paths draft/*, dictation/*, ai_job/*, server/*, outbox_entry/*.
"""
import html as _html
import importlib.util
import json
import os
import re
import shutil
import socketserver
import sys
import tempfile
import threading
import time
import copy

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl, CouldNotDrive, Unreadable  # noqa: E402
from lib.fixture import ACCOUNT, FX, MSG, const  # noqa: E402

T = json.load(open(os.path.join(ROOT, "fixtures", "tests-j3.json"), encoding="utf-8"))
R1 = MSG[T["record"]["message"]]
ACC = ACCOUNT[R1["account"]]
SIG = FX["open_thread"]["quick_reply"]["signature"]
REPLYER = [m["from"] for m in FX["open_thread"]["messages"] if m["from"] != "Linea Supply"][0]
UNDO = const("UNDO_SEND_DEFAULT_S")
SENT = "server/%s/sent/count" % R1["account"]
RECIP = "server/%s/inbox/count" % T["record"]["test_recipient"]
UTT = {u["id"]: u for u in T["dictation"]["utterances"]}
DRAFT = "Hi Karen,\n\nThe price works for us. Please confirm delivery in March.\n\nAssem"
SHORT = "The price works for us."
MIC_LINE = "The microphone is off for Whoosh. Open System Settings, Privacy and Security, Microphone, and turn Whoosh on."
KEY_LINE = "Add your cloud speech key in Settings: AI providers, speech and agents."


class Err(Exception):
    pass


# ---------------------------------------------------------------- the model of the specified behaviour
class ModelApp:
    def __init__(self, mutants=()):
        self.m = set(mutants)
        self.lock = threading.Lock()
        self.reset()

    def has(self, label):
        return label in self.m

    def reset(self):
        self.surface, self.mode, self.picker = "mac-main", None, False
        self.runs = []
        self.hist, self.marks, self.ai_in, self.caret = [], [], "", None
        self.dstate, self.live, self.audio, self.audio_path, self.utt = "idle", "", 0, None, None
        self.now, self.out, self.sent, self.recip, self.last_body = 0.0, [], 0, 0, ""
        self.offline = self.refuse = self.provider_fail = None
        self.mic_refused = self.cloud_nokey = False
        self.ai_jobs, self.ai_last, self.line, self.pending = 0, {}, "", None
        self.thread = []
        self.raw_html = None

    # -- document
    @property
    def body(self):
        return "".join(r["text"] for r in self.runs)

    def plain(self, text):
        return {"text": text, "format": None, "href": None, "cid": None, "font": None}

    def set_body(self, text):
        self.runs = [self.plain(text)] if text else []

    def insert(self, text):
        if self.caret is not None:
            b = self.body
            self.set_body(b[:self.caret] + text + b[self.caret:])
            self.caret += len(text)
        elif self.runs and self.runs[-1]["format"] is None:
            self.runs[-1]["text"] += text
        else:
            self.runs.append(self.plain(text))

    def edit(self, text):
        if self.has("typed-fix-appended-not-replaced") or self.has("J3.2/typed-fix-appended-not-replaced"):
            self.set_body(text)
        else:
            self.insert(text)
        if not self.has("J3.3/mark-clears-on-edit"):
            self.marks = []

    def mime(self):
        if self.raw_html is not None:
            return self.raw_html
        if not self.runs:
            return "<p></p>" if self.has("empty-draft-mime-is-empty") else ""
        out = []
        i = 0
        runs = self.runs
        while i < len(runs):
            r = runs[i]
            f = r["format"]
            if f == "list-item":
                items = []
                while i < len(runs) and runs[i]["format"] == "list-item":
                    items.append("<li>%s</li>" % _html.escape(runs[i]["text"], quote=False))
                    i += 1
                out.append("<ul>%s</ul>" % "".join(items))
                continue
            t = r["text"] if self.has("special-characters-escape-and-return") else _html.escape(r["text"], quote=False)
            if self.has("long-text-roundtrip-keeps-all-chars"):
                t = t[:1000]
            if f == "bold":
                out.append("<strong>%s</strong>" % t)
            elif f == "link":
                out.append('<a href="%s">%s</a>' % (r["href"], t))
            elif f == "picture":
                out.append('<img src="cid:%s">' % r["cid"])
            else:
                out.append(t)
            i += 1
        return "".join(out)

    def reload_from_mime(self):
        s, runs = self.mime(), []
        for m in re.finditer(r"<strong>(.*?)</strong>|<ul>(.*?)</ul>|<a href=\"(.*?)\">(.*?)</a>|<img src=\"cid:(.*?)\">|([^<]+)", s):
            b, ul, href, lt, cid, plain = m.groups()
            if b is not None:
                runs.append({**self.plain(_html.unescape(b)), "format": None if self.has("round-trip-bold") else "bold"})
            elif ul is not None:
                items = re.findall(r"<li>(.*?)</li>", ul)
                if self.has("round-trip-list"):
                    items = ["".join(items)]
                runs += [{**self.plain(_html.unescape(x)), "format": "list-item"} for x in items]
            elif href is not None:
                runs.append({**self.plain(_html.unescape(lt)), "format": "link", "href": None if self.has("round-trip-link") else href})
            elif cid is not None:
                if not self.has("round-trip-picture"):
                    runs.append({**self.plain(""), "format": "picture", "cid": cid})
            else:
                runs.append(self.plain(_html.unescape(plain)))
        self.runs = runs

    # -- sending
    def new_entry(self, state):
        e = {"state": state, "t": self.now, "body": self.body, "send_at": None}
        self.out.append(e)
        return e

    def deliver(self, e):
        if self.offline:
            return
        if self.refuse:
            e["state"] = "failed"
            e["reason"] = self.refuse
            return
        e["state"] = "sending" if self.has("J3.7/outbox-entry-sent") else "sent"
        if not self.has("J3.7/one-copy-in-sent-folder"):
            self.sent += 1
        if not self.has("J3.7/recipient-got-exactly-one"):
            self.recip += 1
        body = e["body"][:1000] if self.has("long-body-sends-whole") else e["body"]
        self.last_body = body if self.has("J3.7/sent-body-ends-with-signature") else body + "\n" + SIG
        self.thread = [{"sender": "Linea Supply", "text": "quote"}, {"sender": REPLYER, "text": e["body"]}]
        if self.has("J3.7/reply-last-in-thread-from-owner"):
            self.thread = [{"sender": "Linea Supply", "text": "quote"}]
        if self.has("J3.7/reply-text-in-thread"):
            self.thread[-1]["text"] = "other"
        if self.has("J3.7/one-copy-in-sent-folder") is False and self.has("J3.7/recipient-got-exactly-one") is False:
            pass

    def tick(self):
        for e in self.out:
            if e["state"] == "undoable" and self.now - e["t"] >= UNDO:
                self.surface = "mac-main" if self.surface == "mac-composer" and not self.has("J3.7/bar-says-sent") else self.surface
                self.deliver(e)
            elif e["state"] == "failed" or (e["state"] == "undoable" and False):
                pass
            elif e["state"] == "waiting" and not self.offline:
                self.deliver(e)
            if e["state"] == "undoable" and self.offline and self.now - e["t"] >= UNDO:
                e["state"] = "waiting"
            if e["state"] == "cancelled" and self.has("J3.6/still-nothing-after-window-passes") and self.now - e["t"] >= UNDO and not e.get("leaked"):
                e["leaked"] = True
                self.sent += 1
                self.recip += 1
            if e["state"] == "scheduled" and e.get("due") and self.now >= e["due"]:
                if not self.has("J3.8/leaves-at-its-time"):
                    self.deliver(e)
                    if self.has("J3.8/delivered-once-more"):
                        self.sent += 1

    def active(self):
        for i in range(len(self.out) - 1, -1, -1):
            if self.out[i]["state"] == "undoable":
                return i
        return None

    # -- verbs
    def handle(self, verb, a):
        with self.lock:
            return getattr(self, "v_" + verb)(*a) or {}

    def v_type(self, text):
        if self.mode == "ai-field":
            self.ai_in += text
        else:
            self.edit(text)

    def finish_dictation(self):
        self.dstate = "cleaning"
        if self.cloud_nokey:
            self.line = "" if self.has("cloud-key-missing-line-names-setting") else KEY_LINE
        elif self.utt is not None:
            u = UTT[self.utt]
            if u["cleaned"] and not self.has("dictation-empty-utterance-inserts-nothing"):
                raw = u["raw"]
                txt = raw if self.has("J3.2/cleaned-text-at-cursor") else (raw.replace("uh ", "") if False else u["cleaned"])
                if self.has("J3.2/fillers-removed"):
                    txt = "Um " + u["cleaned"]
                if self.has("cleaned-" + u["id"]):
                    txt = u["cleaned"].rstrip(".")
                self.insert(txt) if not self.has("dictation-caret-insert") else self.set_body(self.body + txt)
            elif self.has("dictation-empty-utterance-inserts-nothing"):
                self.insert("[empty]")
            if not self.has("J3.2/draft-saved-in-core"):
                pass
        if self.audio_path and os.path.exists(self.audio_path) and not self.has("audio-file-gone-after-text"):
            os.remove(self.audio_path)
        if not self.has("J3.2/audio-deleted-at-text"):
            self.audio = 0
        self.dstate = "cleaning" if self.has("J3.2/dictation-idle-after") else "idle"
        self.utt = None

    def v_key(self, chord):
        if chord == "R":
            if self.surface != "mac-main":
                raise Err("R does nothing here")
            self.mode = "quick"
            self.set_body("")
        elif chord == "Fn:down":
            if self.surface != "mac-composer":
                raise Err("no composer")
            if self.mic_refused:
                self.line = "" if self.has("mic-refused-line-explains") else MIC_LINE
                return
            if self.dstate == "listening":
                if self.has("double-fn-down-one-session"):
                    self.audio += 1
                return
            self.dstate = "idle" if self.has("J3.1/state-listening") else "listening"
            self.audio = 0 if self.has("J3.1/audio-held-while-listening") else 1
        elif chord == "Fn:up":
            if self.dstate != "listening":
                raise Err("not listening")
            self.finish_dictation()
        elif chord == "⌘Enter":
            ai = self.active()
            if ai is not None:
                if self.has("double-send-key-makes-one-entry"):
                    self.new_entry("undoable")
                    return
                raise Err("already sending")
            self.surface = "mac-composer" if not self.has("J3.5/arrives-by-control") else "mac-main"
            st = "sent" if self.has("J3.5/outbox-entry-undoable") else "undoable"
            e = self.new_entry(st)
            if self.has("J3.5/outbox-has-one-entry"):
                self.new_entry(st)
            if self.has("J3.5/nothing-delivered-during-window") or self.has("J3.7/arrives-by-control") and len(self.out) > 1:
                self.sent += 1
            if self.has("J3.7/arrives-by-control") and len(self.out) > 1:
                self.surface = "mac-main"
        elif chord == "Z":
            i = self.active()
            if i is None:
                if self.has("undo-after-window-is-refused") and self.out:
                    self.out[-1]["state"] = "cancelled"
                    return
                raise Err("nothing to undo")
            if not self.has("J3.6/outbox-entry-cancelled"):
                self.out[i]["state"] = "cancelled"
            self.surface = "mac-composer" if not self.has("J3.6/arrives-by-control") else "mac-main"
            if self.has("J3.6/composer-reopens-with-same-text"):
                self.set_body("")
            if self.has("J3.6/server-sent-folder-holds-no-copy"):
                self.sent += 1
            if self.has("J3.6/recipient-received-nothing"):
                self.recip += 1
        elif chord == "⌘Z":
            if not self.hist:
                raise Err("nothing to undo")
            runs = self.hist.pop()
            if self.has("J3.4/cmd-z-restores-exact-old-text"):
                runs = [self.plain("".join(r["text"] for r in runs).strip()[:-1])]
            if self.has("rewrite-undo-keeps-formatting"):
                runs = [self.plain("".join(r["text"] for r in runs))]
            self.runs = runs
        elif chord == "Enter":
            if self.mode == "ai-field":
                self.run_ai()
            elif self.mode == "rewrite" and self.pending:
                self.hist.append(copy.deepcopy(self.runs))
                if not self.has("J3.4/paragraph-replaced-shorter"):
                    self.set_body(SHORT)
                self.mode, self.pending = None, None
            else:
                raise Err("Enter does nothing here")
        elif chord == "⌘⇧L":
            self.surface = "mac-composer" if not self.has("J3.8/arrives-by-control") else "mac-main"
            self.picker = True
        else:
            raise Err("no binding for %s" % chord)

    def run_ai(self):
        ins = self.ai_in
        self.ai_in, self.mode = "", None
        if self.provider_fail:
            self.line = "The provider said: " + self.provider_fail
            if self.has("provider-failure-keeps-owners-text"):
                self.set_body("")
            return
        if ins == "" and not self.has("empty-instruction-writes-nothing"):
            return
        if ins == "0" and self.has("zero-instruction-is-a-real-instruction"):
            return
        self.ai_jobs += 1
        self.ai_last = {"agent": None if self.has("J3.3/draft-uses-writer-agent") else T["ai"]["writer_agent"],
                        "voice_profile_address": None if self.has("J3.3/draft-uses-voice-profile-of-address") else ACC["address"]}
        self.hist.append(copy.deepcopy(self.runs))
        d = "" if self.has("J3.3/draft-written") else DRAFT
        self.set_body(d)
        self.marks = [] if self.has("J3.3/marked-as-ai") else [{"words": "zzz" if self.has("J3.3/mark-covers-the-draft") else d}]

    def v_click(self, ax):
        if ax == "mac-main.reader.open-full-composer":
            self.surface = "mac-main" if self.has("J3.1/arrives-by-control") else "mac-composer"
            if self.has("J3.1/dictation-idle-first"):
                self.dstate = "listening"
        elif ax == "mac-composer.ai-draft":
            self.mode = "ai-field"
            self.ai_in = ""
            if self.has("J3.3/arrives-by-control"):
                self.surface = "mac-main"
        elif ax == "mac-composer.rewrite":
            self.mode = "rewrite"
            if self.has("J3.4/arrives-by-control"):
                self.surface = "mac-main"
        elif ax == "mac-composer.rewrite.shorter":
            self.pending = "shorter"
        elif ax == "mac-composer.send-later.tomorrow-0800":
            e = self.new_entry("undoable" if self.has("J3.8/outbox-entry-scheduled") else "scheduled")
            e["send_at"] = None if self.has("J3.8/outbox-send-at-local") else T["schedule"]["local_time"]
            e["due"] = 10 ** 9
            self.picker = False
            if self.has("J3.8/not-delivered-before-its-time"):
                self.deliver(e)
        elif ax == "mac-main.outbox.retry":
            for e in self.out:
                if e["state"] == "failed":
                    e["state"] = "undoable"
                    e["t"] = self.now - UNDO
            self.tick()
        else:
            raise Err("nothing at " + ax)

    def v_rows(self, ax):
        if ax == "mac-composer.body":
            if self.has("J3.4/old-text-on-screen-before-rewrite") and self.mode == "rewrite":
                return {"rows": [{"text": ""}]}
            return {"rows": [{"text": self.body}]}
        if ax == "mac-composer.header":
            frm = "assem@nexus-example.io" if self.has("J3.1/from-is-receiving-account") else ACC["address"]
            sg = "Other Person" if self.has("J3.1/signature-of-that-address") else SIG
            return {"rows": [{"from": frm, "signature": sg, "quoted_folded": not self.has("J3.1/quoted-text-folded")}]}
        if ax == "mac-composer.ai-marks":
            return {"rows": self.marks}
        if ax == "mac-composer.rewrite.choices":
            ch = T["ai"]["rewrite_choices"][:2] if self.has("J3.4/choices-in-words") else T["ai"]["rewrite_choices"]
            return {"rows": [{"title": c} for c in ch]}
        if ax == "mac-composer.undo.bar":
            i = self.active()
            left = UNDO if self.has("J3.5/bar-counting-down") else UNDO - int(self.now - self.out[i]["t"]) if i is not None else 0
            # while the window is open the bar says it is sending and offers undo; after it the bar says it is sent and offers none (J3.6: "the window has passed")
            return {"rows": [{"seconds_left": left, "text": "Sending" if i is not None else "Sent", "undo_offered": i is not None}]}
        if ax == "mac-main.undo.bar":
            sent = any(e["state"] == "sent" for e in self.out)
            txt = "" if self.has("J3.7/bar-says-sent") else ("Sent" if sent else "")
            offered = True if self.has("undo-after-window-is-refused") is False and self.has("J3.7/undo-no-longer-offered") else False
            return {"rows": [{"text": txt if sent else "Sending", "undo_offered": offered}]}
        if ax == "mac-main.reader.thread":
            return {"rows": self.thread}
        if ax == "mac-composer.send-later.options":
            ts = ["08:00", "13:00"] if self.has("J3.8/picker-in-words") else [T["schedule"]["words"], "next week"]
            return {"rows": [{"title": t} for t in ts]}
        if ax == "mac-main.sidebar.scheduled":
            ok = [e for e in self.out if e["state"] == "scheduled"] if not self.has("J3.8/listed-in-scheduled-at-time") else []
            return {"rows": [{"time": T["schedule"]["local_time"]} for _ in ok]}
        if ax == "mac-main.sidebar.outbox":
            keep = [e for e in self.out if e["state"] in ("waiting", "failed")]
            if self.has("offline-send-waits-then-goes"):
                keep = []
            return {"rows": [{"state": e["state"]} for e in keep]}
        if ax == "mac-main.outbox.status":
            e = [x for x in self.out if x["state"] == "failed"]
            reason = e[0].get("reason", "") if e else ""
            return {"rows": [{"text": "" if self.has("server-refusal-keeps-draft-with-reason") else "Not sent: " + reason,
                              "retry_offered": bool(e)}]}
        if ax == "mac-composer.line":
            return {"rows": [{"text": self.line}]}
        if ax == "mac-composer.runs":
            return {"rows": [dict(r) for r in self.runs]}
        raise Err("no rows at " + ax)

    def v_uistate(self):
        live = self.live if not self.has("J3.1/live-words") else ""
        return {"state": {"surface": self.surface, "dictation": {"state": self.dstate, "live_text": live if self.dstate == "listening" else ""}}}

    def v_core(self, kind, path):
        p = path.replace(".", "/").split("/")
        v = None
        if path == "draft/current/text":
            v = "" if self.has("J3.2/draft-saved-in-core") else self.body
        elif path == "draft/current/mime":
            v = self.mime()
        elif path == "dictation/audio_files":
            v = self.audio
        elif path == "ai_job/count":
            v = self.ai_jobs
        elif p[:2] == ["ai_job", "last"]:
            v = self.ai_last.get(p[2])
        elif path == "outbox_entry/count":
            v = len(self.out)
        elif p[0] == "outbox_entry" and len(p) == 3:
            i = int(p[1])
            if i < len(self.out):
                e = self.out[i]
                v = e["state"] if p[2] == "state" else e["send_at"]
                if p[2] == "state" and self.has("J3.5/bar-counting-down") is False and False:
                    pass
        elif path == SENT or path == "counts/sent":
            v = self.sent
        elif path == RECIP:
            v = self.recip
        elif path == "server/%s/sent/last_body" % R1["account"]:
            v = self.last_body
        return {"value": v}

    def v_clock(self, how, arg):
        if how == "advance":
            self.now += float(arg)
        elif how == "advance_to":
            self.now += 10 ** 6
            for e in self.out:
                if e["state"] == "scheduled":
                    e["due"] = self.now - 1
        else:
            raise Err("clock " + how)
        self.tick()

    def v_measure(self, ax):
        return {"ms": 40}

    def v_fixture(self, name, *a):
        n = name
        if n == "reset":
            self.reset()
        elif n == "speak":
            if self.dstate != "listening":
                raise Err("not listening")
            self.utt = a[0]
            raw = UTT[a[0]]["raw"]
            self.live = raw
            if self.has("raw-had-fillers") or self.has("J3.2/raw-had-fillers"):
                self.live = UTT[a[0]]["cleaned"].lower()
        elif n == "speak-file":
            if self.dstate != "listening":
                raise Err("not listening")
            self.utt, self.audio_path = "u1", a[0]
            self.live = UTT["u1"]["raw"]
        elif n == "offline":
            self.offline = a[0] == "on"
            self.tick()
        elif n == "server-refuse":
            self.refuse = None if a[0] == "off" else a[0]
        elif n == "provider-fail":
            self.provider_fail = a[0]
        elif n == "mic-refuse":
            self.mic_refused = True
        elif n == "cloud-no-key":
            self.cloud_nokey = True
        elif n == "caret":
            self.caret = int(a[0])
        elif n in ("select-all", "select-paragraph"):
            pass
        elif n == "format":
            kind = a[0]
            if kind == "bold":
                self.runs = [{**self.plain(a[1]), "format": "bold"}]
            elif kind == "list":
                self.runs = [{**self.plain(x), "format": "list-item"} for x in a[1:]]
            elif kind == "link":
                self.runs = [{**self.plain(a[1]), "format": "link", "href": a[2]}]
            elif kind == "picture":
                self.runs = [{**self.plain(""), "format": "picture", "cid": a[1]}]
            else:
                raise Err("format " + kind)
        elif n == "reload-draft":
            self.reload_from_mime()
        elif n == "paste-html":
            h = a[0]
            if "<script" in h or "onclick" in h:
                if self.has("hostile-html-is-neutralised"):
                    self.runs = [{**self.plain("x"), "format": None}]
                    self.raw_html = h
                else:
                    inner = re.search(r"<b[^>]*>(.*?)</b>", h)
                    self.runs = [{**self.plain(inner.group(1)), "format": "bold"}]
            else:
                txt = re.sub(r"<[^>]+>", "", h)
                self.runs = [{**self.plain(txt), "font": "Papyrus" if self.has("paste-match-style-strips-styles") else None}]
        else:
            raise Err("fixture " + n)


# ---------------------------------------------------------------- the model of the J3 journey (journeys/J3.py as brought to main53)
# ModelApp above stays what dictation-check.py and richtext-check.py import. This model serves what journeys/J3.py reads TODAY, one field at a time from the
# app's own socket code (the file and line beside each). It models the SPECIFIED behaviour; each `J3.x/<case>` mutant (the labels in the journey's MUTANTS
# block) breaks exactly one thing. The journey's wall-clock waits are virtual here: selftest() points time.sleep at `advance_wall`.
JSIG = ACC["signature"]      # journeys/J3.py:43: the receiving address's own signature (the design fixture's account row), not the quick reply's


class J3Model:
    UNDO_S = UNDO

    def __init__(self, mutants=()):
        self.m = set(mutants)
        self.lock = threading.Lock()
        self.wall = 0.0
        self.reset()

    def has(self, label):
        return label in self.m

    def advance_wall(self, secs):
        with self.lock:
            self.wall += secs

    def reset(self):
        self.surface, self.quick, self.composer_open, self.spent_body = "mac-main", False, False, None
        self.body, self.marks, self.hist = "", [], []
        self.ai_field, self.ai_in, self.focused, self.rewrite_open = False, "", "body", False
        self.dstate, self.live, self.audio, self.utt = "idle", "", 0, None
        self.phase, self.attempts = None, 0                 # phase: None | ("counting", entry index, wall at send) | ("sent",)
        self.out, self.sent, self.recip, self.core_now = [], 0, 0, 0.0
        self.cards, self.line, self.picker, self.discard_ask = [], "", False, False
        self.ai_last = {}
        self.lang = "system"                                # the app language (J3 picks English through the language door: journeys/J3.py LANG_DOOR)
        self.theme = "system"                               # `fixture theme <light|dark>` (J3.1 theme-set), read back from uistate (theme-applied)

    # -- verbs
    def handle(self, verb, args):
        with self.lock:
            fn = getattr(self, "v_" + verb, None)
            if fn is None:
                raise Err("model: no verb %s" % verb)
            return fn(*args) or {}

    def v_uistate(self):
        # Sources/ControlSocket/Verbs.swift:50-64: the surface is `m.surface`
        return {"state": {"surface": self.surface, "theme": self.theme}}

    def v_type(self, text):
        # ComposerMacFeature.swift:36-40 typeText: the text lands at the caret (the end here); the AI field keeps its own buffer
        if self.ai_field:
            self.ai_in += text
        elif self.composer_open:
            if self.has("J3.2/typed-fix-appended-not-replaced"):
                self.body = text
            elif not self.has("J3.3/edit-lands-in-the-draft"):
                self.body += text
            if not self.has("J3.3/mark-clears-on-edit"):
                self.marks = []
        return {}

    def v_key(self, chord):
        if chord == "Escape":
            return {}
        if chord == "R":
            self.quick = True
            return {}
        if chord == "Fn:down":
            if not self.composer_open:
                raise Err("the key Fn:down does nothing on %s" % self.surface)
            if not self.has("J3.1/state-listening"):
                self.dstate = "listening"
                if not self.has("J3.1/audio-held-while-listening"):
                    self.audio += 1
            return {}
        if chord == "Fn:up":
            if self.dstate != "listening":
                raise Err("the key Fn:up does nothing here")
            u = UTT.get(self.utt or "u1")
            if self.utt is not None:
                text = u["raw"] if self.has("J3.2/cleaned-text-at-cursor") else ("Um " + u["cleaned"] if self.has("J3.2/fillers-removed") else u["cleaned"])
                self.body += text
            if not self.has("J3.2/audio-deleted-at-text"):
                self.audio = 0
            self.dstate = "cleaning" if self.has("J3.2/dictation-idle-after") else "idle"
            self.utt = None
            return {}
        if chord == "⌘Enter":
            self.attempts += 1
            if not self.composer_open or (self.has("J3.5/arrives-by-control") and self.attempts == 1) or (self.has("J3.7/arrives-by-control") and self.attempts == 2):
                return {}
            for _ in range(2 if self.has("J3.5/outbox-has-one-entry") else 1):
                self.out.append({"state": "sent" if self.has("J3.5/outbox-entry-undoable") else "undoable", "t": self.core_now, "body": self.body})
            self.phase = ("counting", len(self.out) - 1, self.wall)
            if self.has("J3.5/nothing-delivered-during-window"):
                self.sent += 1
            return {}
        if chord == "Z":
            if self.phase is None or self.phase[0] != "counting":
                raise Err("the key Z does nothing on %s" % self.surface)
            if self.has("J3.6/arrives-by-control"):
                return {}
            e = self.out[self.phase[1]]
            if not self.has("J3.6/outbox-entry-cancelled"):
                e["state"] = "cancelled"
            self.phase, self.surface = None, "mac-composer"
            if self.has("J3.6/composer-reopens-with-same-text"):
                self.body = ""
            if self.has("J3.6/server-sent-folder-holds-no-copy"):
                self.sent += 1
            if self.has("J3.6/recipient-received-nothing"):
                self.recip += 1
            return {}
        if chord == "⌘Z":
            if not self.hist:
                raise Err("the key ⌘Z does nothing on %s" % self.surface)
            old = self.hist.pop()
            self.body = old[:-1] if self.has("J3.4/cmd-z-restores-exact-old-text") else old
            if self.has("J3.4/still-in-the-composer"):
                self.surface = "mac-reader"
            return {}
        if chord == "⌘⇧L":
            if self.has("J3.8/arrives-by-control"):
                return {}
            self.composer_open, self.surface, self.picker, self.body, self.focused = True, "mac-composer", True, "", "body"
            return {}
        if chord == "Enter":
            if not self.ai_field:
                raise Err("the key Enter does nothing on %s" % self.surface)
            self.ai_field, self.ai_in = False, ""
            if self.has("J3.3/draft-in-owners-voice"):
                draft = "Hi Karen,\n\nThe price works for us. Please confirm delivery in March!"
            else:
                draft = DRAFT
            self.body = "" if self.has("J3.3/draft-written") else draft
            self.marks = [] if self.has("J3.3/marked-as-ai") else [{"ax": "mac-composer.ai-marks", "words": "zzz" if self.has("J3.3/mark-covers-the-draft") else self.body}]
            self.ai_last = {"agent": None if self.has("J3.3/draft-uses-writer-agent") else T["ai"]["writer_agent"],
                            "voice_profile_address": None if self.has("J3.3/draft-uses-voice-profile-of-address") else ACC["address"]}
            self.focused = "none" if self.has("J3.3/focus-back-in-the-text-after-draft") else "body"
            return {}
        raise Err("the key %s does nothing on %s" % (chord, self.surface))

    def v_click(self, ax):
        if ax.startswith("mac-settings-general.appearance.language."):
            # the language pick of Settings > General (a click door that needs no window); its broken copy: the door does nothing
            if not self.has("J3.1/app-language-pinned"):
                self.lang = ax.rsplit(".", 1)[1]
            return {}
        if ax == "mac-main.reader.open-full-composer":
            if not self.quick:
                raise Err("%s did nothing" % ax)
            self.composer_open, self.body, self.marks, self.dstate = True, "", [], ("listening" if self.has("J3.1/dictation-idle-first") else "idle")
            self.surface = "mac-main" if self.has("J3.1/arrives-by-control") else "mac-composer"
            return {}
        if ax == "mac-composer.dev.fixture-writer":
            return {}
        if ax == "mac-composer.ai-draft":
            self.ai_field, self.ai_in, self.focused = True, "", "ai"
            if self.has("J3.3/arrives-by-control"):
                self.surface = "mac-main"
            return {}
        if ax == "mac-composer.rewrite":
            self.rewrite_open = True
            if self.has("J3.4/arrives-by-control"):
                self.surface = "mac-main"
            return {}
        if ax == "mac-composer.rewrite." + T["ai"]["rewrite_chosen"]:
            if not self.rewrite_open:
                raise Err("%s did nothing" % ax)
            self.hist.append(self.body)
            if not self.has("J3.4/paragraph-replaced-shorter"):
                self.body = SHORT
            self.rewrite_open = False
            return {}
        if ax == "mac-composer.send-later.tomorrow-0800":
            if not self.picker:
                raise Err("%s did nothing" % ax)
            # `due`: the model's clock at tomorrow 08:00 (the picker row's own time): the journey advances the clock by the seconds to it (`clock advance`)
            import datetime as _dt
            now = _dt.datetime.now().astimezone()
            due = self.core_now + ((now + _dt.timedelta(days=1)).replace(hour=8, minute=0, second=0, microsecond=0) - now).total_seconds()
            self.out.append({"state": "undoable" if self.has("J3.8/outbox-entry-scheduled") else "scheduled", "t": self.core_now, "due": due, "body": self.body})
            self.line = "" if self.has("J3.8/composer-says-scheduled") else "Scheduled for tomorrow 8:00"
            self.picker = False
            if self.has("J3.8/not-delivered-before-its-time"):
                self.sent += 1
            return {}
        if ax == "mac-composer.discard":
            self.discard_ask = True
            return {}
        if ax == "mac-composer.discard.confirm":
            if not self.discard_ask:
                raise Err("%s did nothing" % ax)
            if self.has("J3.8/composer-closed-to-the-thread"):
                # journeys/J3.py:233-235 discards a composer the window left open before J3.8, so a composer that merely stayed open would be closed by that
                # setup and the case could never see it. The mutant is a composer that STAYS open: the discard does not close it either.
                self.discard_ask = False
                return {}
            self.composer_open, self.body, self.discard_ask, self.surface = False, "", False, "mac-main"
            return {}
        raise Err("%s did nothing" % ax)

    def v_fixture(self, name, *a):
        if name == "speak":
            if self.dstate != "listening":
                raise Err("fixture speak: not listening")
            u = UTT["u1"]
            self.utt = "u1"
            self.live = "" if self.has("J3.1/live-words") else (u["cleaned"].lower() if self.has("J3.2/raw-had-fillers") else u["raw"])
            return {}
        if name == "reset":
            self.reset()
            return {}
        if name == "theme":
            if self.has("J3.1/theme-set"):
                raise Err("fixture theme: refused")            # the verb answers ok:false
            if not self.has("J3.1/theme-applied"):
                self.theme = a[0]                               # the broken copy takes the verb and keeps the old theme
            return {}
        raise Err("model: no fixture event %s" % name)

    def deliver(self, e):
        e["state"] = "sending" if self.has("J3.7/outbox-entry-sent") else "sent"
        if not self.has("J3.7/one-copy-in-sent-folder"):
            self.sent += 1
        if not self.has("J3.7/recipient-got-exactly-one"):
            self.recip += 1
        if not self.has("J3.7/reply-shows-in-the-thread-as-sent"):
            text = "other" if self.has("J3.7/reply-text-in-thread") else (e["body"] if self.has("J3.7/sent-body-ends-with-signature") else e["body"] + "\n" + JSIG)
            self.cards.append({"text": text, "account": R1["account"], "state": "sent"})

    def close_composer_after_send(self):
        if self.has("J3.8/composer-closed-to-the-thread"):
            self.body = ""            # the composer stays open, empty
            return
        if self.has("J3.7/composer-text-spent-after-sent"):
            self.spent_body = self.body
        self.composer_open, self.body = False, ""

    def v_clock(self, how, arg):
        if how == "advance":
            self.core_now += float(arg)
            for e in self.out:
                if e["state"] == "undoable" and self.core_now - e["t"] >= self.UNDO_S:
                    self.deliver(e)
                    self.close_composer_after_send()
                elif e["state"] == "cancelled" and self.has("J3.6/still-cancelled-after-window-passes") and self.core_now - e["t"] >= self.UNDO_S:
                    e["state"] = "sent"
                    self.sent += 1
                elif e["state"] == "scheduled" and self.core_now >= e.get("due", float("inf")) and not self.has("J3.8/leaves-at-its-time"):
                    self.deliver(e)                           # the scheduled mail leaves when the clock passes its time (journeys/J3.py advances by seconds)
                    if self.has("J3.8/delivered-once-more"):
                        self.sent += 1
            return {}
        if how == "advance_to":
            for e in self.out:
                if e["state"] == "scheduled" and not self.has("J3.8/leaves-at-its-time"):
                    self.deliver(e)
                    if self.has("J3.8/delivered-once-more"):
                        self.sent += 1
            return {}
        raise Err("model: no clock %s" % how)

    def v_core(self, kind, path):
        p = path.split("/")
        v = None
        if path == "draft/count":
            v = None if self.has("J3.2/draft-saved-in-core") else (1 if self.composer_open and self.body else 0)
        elif p[:2] == ["ai_job", "last"]:
            v = self.ai_last.get(p[2])
        elif path == "outbox_entry/count":
            v = len(self.out)
        elif p[0] == "outbox_entry" and len(p) == 3 and p[1].isdigit() and int(p[1]) < len(self.out):
            v = self.out[int(p[1])]["state"]
        elif path == "server/%s/sent/count" % R1["account"]:
            v = self.sent
        elif path == "counts/sent":
            v = self.sent                                       # journeys/J3.py SENT: the outbox entries that left (FixtureLabels.swift)
        elif path == "counts/outbox":
            # journeys/J3.py WAITING: the entries still waiting, undoable or scheduled; the broken copy of J3.8/listed-in-scheduled leaves the scheduled ones out
            v = sum(1 for e in self.out if e["state"] == "undoable" or (e["state"] == "scheduled" and not self.has("J3.8/listed-in-scheduled")))
        elif path == "server/%s/inbox/count" % T["record"]["test_recipient"]:
            v = self.recip
        elif path == "mailbox/scheduled/count":
            v = 0 if self.has("J3.8/listed-in-scheduled") else sum(1 for e in self.out if e["state"] == "scheduled")
        if v is None:
            raise Err("no core value at %s" % path)
        return {"value": v}

    def v_rows(self, ax):
        left = None
        if ax == "mac-settings-general.settings":
            return {"rows": [{"language": self.lang}]}      # SettingsGeneralMacFeature: the settings row serves the picked language
        if ax == "mac-composer.dictation":
            # DictationFeature.swift:71-76: state, live, line, audio_files (always registered)
            return {"rows": [{"state": self.dstate, "live": self.live if self.dstate == "listening" else "", "line": "", "audio_files": self.audio}]}
        if ax == "mac-composer.header":
            # ComposerMacFeature.swift:46-57: one row while the composer is open
            if not self.composer_open:
                return {"rows": []}
            # `to`: the reply's recipients (J3.1 reads their count and takes the first as the address its delivery reads name)
            to = [T["record"]["test_recipient"]] + (["second@example.test"] if self.has("J3.1/reply-addressed-to-one-recipient") else [])
            return {"rows": [{"ax": ax, "from": "assem@nexus-example.io" if self.has("J3.1/from-is-receiving-account") else ACC["address"],
                              "signature": "Other Person" if self.has("J3.1/signature-of-that-address") else JSIG, "to": to,
                              "quoted_folded": self.has("J3.1/quoted-text-folded") is False, "focused": self.focused}]}
        if ax == "mac-composer.body":
            # ComposerMacFeature.swift:58
            if self.composer_open:
                return {"rows": [{"ax": ax, "text": "" if (self.has("J3.4/old-text-on-screen-before-rewrite") and self.rewrite_open) else self.body}]}
            return {"rows": [{"ax": ax, "text": self.spent_body}] if self.spent_body else []}
        if ax == "mac-composer.ai-marks":
            return {"rows": list(self.marks)}          # ComposerMacFeature.swift:59-62: words, start, end
        if ax == "mac-composer.rewrite.choices":
            if not self.rewrite_open:
                return {"rows": []}
            ch = T["ai"]["rewrite_choices"][:2] if self.has("J3.4/choices-in-words") else T["ai"]["rewrite_choices"]
            return {"rows": [{"title": c.capitalize()} for c in ch]}      # ComposerMacFeature.swift:63-65
        if ax == "mac-composer.undo.bar":
            # ComposerMacFeature.swift:66-72: counting -> seconds_left/text/undo_offered, sent -> text "Sent", undo_offered false
            if self.phase is None:
                return {"rows": []}
            elapsed = self.wall - self.phase[2]
            if elapsed < self.UNDO_S:
                start = self.UNDO_S - 3 if self.has("J3.5/bar-starts-at-the-window") else self.UNDO_S
                left = start if self.has("J3.5/bar-counting-down") else start - int(elapsed)
                return {"rows": [{"ax": ax, "seconds_left": left, "text": "Sending", "undo_offered": True}]}
            return {"rows": [{"ax": ax, "seconds_left": 0, "text": "" if self.has("J3.7/bar-says-sent") else "Sent",
                              "undo_offered": self.has("J3.7/undo-no-longer-offered")}]}
        if ax == "mac-composer.send-later.options":
            # ComposerMacFeature.swift:73-77: title and resolves_to (the wall time the words resolve to)
            if not self.picker:
                return {"rows": []}
            import datetime as _dt
            tomorrow = (_dt.datetime.now().astimezone() + _dt.timedelta(days=1)).replace(hour=8, minute=0, second=0, microsecond=0)
            if self.has("J3.8/picker-in-words"):
                return {"rows": [{"title": "Tomorrow 08:00", "resolves_to": tomorrow.strftime("%Y-%m-%d %H:%M:%S %z")}]}
            return {"rows": [{"title": "Tomorrow morning", "resolves_to": tomorrow.strftime("%Y-%m-%d %H:%M:%S %z")},
                             {"title": "Next week", "resolves_to": (tomorrow + _dt.timedelta(days=6)).strftime("%Y-%m-%d %H:%M:%S %z")}]}
        if ax == "mac-composer.line":
            return {"rows": [{"ax": ax, "text": self.line}]}
        if ax == "mac-main.thread.outgoing":
            # ThreadCardsRegistry.swift:9-27: id, text, account, state
            return {"rows": [dict(c, ax="mac-main.thread.outgoing.%d" % i, id="out-%d" % i) for i, c in enumerate(self.cards)]}
        if ax == "mac-main.list":
            return {"rows": [] if self.has("J3.8/composer-closed-positive-control") else [{"id": "msg-01", "subject": R1["subject"]}, {"id": "msg-02", "subject": "x"}]}
        raise Err("no rows at " + ax)


def serve(app):
    d = tempfile.mkdtemp(prefix="wm", dir="/tmp")

    class H(socketserver.StreamRequestHandler):
        def handle(self):
            for line in self.rfile:
                try:
                    req = json.loads(line.decode())
                    out = {"ok": True, **app.handle(req["verb"], req.get("args", []))}
                except Err as e:
                    out = {"ok": False, "error": str(e)}
                except Exception as e:  # a model bug must never read as a pass
                    out = {"ok": False, "error": "model: %r" % e}
                self.wfile.write((json.dumps(out) + "\n").encode())

    class S(socketserver.ThreadingUnixStreamServer):
        daemon_threads = True

    srv = S(os.path.join(d, "whoosh.sock"), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv, d


# ---------------------------------------------------------------- harness
class Registry:
    def __init__(self):
        self.items = []

    def check(self, name, mutant, expected):
        def deco(fn):
            self.items.append((name, mutant, fn, expected))
            return fn
        return deco


def BODY(c):
    return c.rows("mac-composer.body")[0]["text"]


def compose(c, text=None):
    """Open the full composer on record 1 through its named controls."""
    c.send("fixture", "reset")
    c.key("R")
    c.click("mac-main.reader.open-full-composer")
    if text:
        c.type(text)


def tolerate(fn, *a):
    try:
        fn(*a)
    except Unreadable:
        pass


def run_checks(reg, ctl, why=""):
    res = []
    for name, mutant, fn, expected in reg.items:
        if ctl is None:
            res.append((name, "unjudged", why))
            continue
        try:
            got = fn(ctl)
            res.append((name, "pass" if got == expected else "fail", "expected %r, got %r" % (expected, got)))
        except Unreadable as e:
            res.append((name, "unjudged", str(e)))
        except KeyError as e:
            res.append((name, "unjudged" if "not in docs/CONSTANTS" in str(e) else "fail", str(e)))
        except (IndexError, TypeError) as e:
            res.append((name, "fail", "reply shape: %s %s" % (type(e).__name__, e)))
    return res


def selftest(name, regs, mutants, journey=None, allowed_unjudged=()):
    """regs: Registry list. mutants: label -> words. journey: path of a journeys/J*.py judged too."""
    import importlib.util as iu
    from lib.harness import run_journey
    out_dir = os.path.join(ROOT, "_captures", "tests-j3")
    os.makedirs(out_dir, exist_ok=True)

    def run(ms):
        app = ModelApp(ms)
        srv, d = serve(app)
        os.environ["WHOOSH_SOCKET_DIR"] = d
        failed, unjudged = set(), set()
        jmodel, jsrv, jd = None, None, None
        try:
            for reg in regs:
                c = Ctl().connect()
                try:
                    for n, st, det in run_checks(reg, c):
                        (failed if st == "fail" else unjudged if st == "unjudged" else set()).add(n)
                finally:
                    c.close()
            if journey:
                jmodel = J3Model(ms)                 # the journey runs against its own model (the one that serves what J3.py reads today)
                jsrv, jd = serve(jmodel)
                os.environ["WHOOSH_SOCKET_DIR"] = jd
                spec = iu.spec_from_file_location("J", journey)
                mod = iu.module_from_spec(spec)
                spec.loader.exec_module(mod)
                # the model has no wall clock (it answers `clock advance` itself), but the journey sleeps real seconds for the owner's: J3.7 sleeps UNDO+1 s and J3.2
                # 1 s on EVERY run, one run per mutant, so the self-test never ended within 300 s (main76: exit 142). A model run caps each sleep.
                if hasattr(mod, "until"):       # J3.until polls up to 4 to 6 s (some calls pass their own timeout) for a value the model already holds: a model run waits at most 0.05 s
                    orig_until = mod.until
                    mod.until = lambda fn, ok, timeout=4.0: orig_until(fn, ok, min(timeout, 0.05))
                real_sleep = time.sleep

                def virtual_sleep(secs):        # the owner's wall clock is the model's `wall`: a sleep of s seconds moves it by s, and costs 2 ms
                    jmodel.advance_wall(secs)
                    real_sleep(min(secs, 0.002))
                time.sleep = virtual_sleep
                try:
                    r = run_journey(mod, journey)
                finally:
                    time.sleep = real_sleep
                failed |= {x.split(":")[0] for x in r["fail_list"]}
                if r["unjudged"]:
                    unjudged.add("journey:%d not judged" % r["unjudged"])
        finally:
            srv.shutdown()
            srv.server_close()
            shutil.rmtree(d, ignore_errors=True)
            if jsrv is not None:
                jsrv.shutdown()
                jsrv.server_close()
                shutil.rmtree(jd, ignore_errors=True)
        return failed, unjudged

    n = sum(len(r.items) for r in regs)
    base_f, base_u = run(())
    bad = len(base_f) + len(base_u - set(allowed_unjudged))
    rows = []
    for label, words in mutants.items():
        f, _ = run((label,))
        rows.append((label, words, label in f, sorted(f - {label})[:3]))
    missed = [r for r in rows if not r[2]]
    json.dump({"check": name, "baseline_failed": sorted(base_f), "baseline_unjudged": sorted(base_u),
               "mutants": [{"label": l, "mutation": w, "seen_red": red, "also_red": also} for l, w, red, also in rows]},
              open(os.path.join(out_dir, "%s-mutants.json" % name), "w"), indent=1)
    print("%d checks, %d failed, %d not judged" % (n, bad + len(missed), len(base_u - set(allowed_unjudged))))
    print("baseline on the model: failed=%s unjudged=%s" % (sorted(base_f), sorted(base_u)))
    print("MUTANTS\n| check | mutation made | seen red |")
    for l, w, red, also in rows:
        print("| %s | %s | %s |" % (l, w, "RED on this case" if red else "NOT RED"))
    return 1 if (bad or missed) else 0


def main(name, regs, mutants, journey=None, allowed_unjudged=(), argv=None):
    argv = sys.argv[1:] if argv is None else argv
    if "--selftest" in argv:
        return selftest(name, regs, mutants, journey, allowed_unjudged)
    ctl, why = None, ""
    try:
        ctl = Ctl().connect()
    except CouldNotDrive as e:
        why = str(e)
    res = [r for reg in regs for r in run_checks(reg, ctl, why)]
    f = [r for r in res if r[1] == "fail"]
    u = [r for r in res if r[1] == "unjudged"]
    for n, st, det in res:
        if st != "pass":
            print("%s %s: %s" % ("FAIL" if st == "fail" else "NOT JUDGED", n, det), file=sys.stderr)
    print("%d checks, %d failed, %d not judged" % (len(res), len(f), len(u)))
    seen = {}
    p = os.path.join(ROOT, "_captures", "tests-j3", "%s-mutants.json" % name)
    if os.path.exists(p):
        seen = {m["label"]: m["seen_red"] for m in json.load(open(p))["mutants"]}
    print("MUTANTS\n| check | mutation made | seen red (last --selftest) |")
    for l, w in mutants.items():
        print("| %s | %s | %s |" % (l, w, {True: "yes", False: "NO"}.get(seen.get(l), "not run")))
    return 1 if (f or u) else 0


# ---------------------------------------------------------------- the composer checks
REG = Registry()
check = REG.check


@check("offline-send-waits-then-goes", "an offline send is not kept in the Outbox", (0, 1, 1, 0))
def _(c):
    compose(c, "Hello.")
    c.send("fixture", "offline", "on")
    c.key("⌘Enter")
    c.send("clock", "advance", UNDO + 1)
    held, listed = c.core(SENT), len(c.rows("mac-main.sidebar.outbox"))
    c.send("fixture", "offline", "off")
    c.send("clock", "advance", 1)
    return (held, listed, c.core(SENT), len(c.rows("mac-main.sidebar.outbox")))


@check("undo-after-window-is-refused", "Z after the window cancels a message that already left", (True, False, "sent", 1))
def _(c):
    # The composer's own bar (`mac-composer.undo.bar`, ComposerWords.undoBar; the main window's bar is the quick reply's and holds no row here) and the
    # Sent mailbox's served count (`counts/sent`; `server/<account>/sent/count` is not served). The bar's words are the app's language, so the case
    # compares that they CHANGED once the window passed, never the sentence "Sent" (a check compares ids and values).
    compose(c, "Hello.")
    sent0 = c.core("counts/sent")
    c.key("⌘Enter")
    during = c.rows("mac-composer.undo.bar")[0]["text"]
    c.send("clock", "advance", UNDO + 1)
    tolerate(c.key, "Z")
    bar = c.rows("mac-composer.undo.bar")[0]
    for _ in range(20):                                              # the bar reads the core every 200 ms: bounded wait for it to say the window passed
        if not bar["undo_offered"]:
            break
        time.sleep(0.1)
        bar = c.rows("mac-composer.undo.bar")[0]
    return (bar["text"] != during, bar["undo_offered"], c.core("outbox_entry/0/state"), c.core("counts/sent") - sent0)


@check("double-send-key-makes-one-entry", "a second Cmd-Enter inside the window queues a second copy", 1)
def _(c):
    compose(c, "Hello.")
    c.key("⌘Enter")
    tolerate(c.key, "⌘Enter")
    return c.core("outbox_entry/count")


@check("server-refusal-keeps-draft-with-reason", "the Outbox shows no reason when the server refuses", ("failed", True, True, 0, 1))
def _(c):
    compose(c, "Hello.")
    reason = T["send"]["refuse_reason"]
    c.send("fixture", "server-refuse", reason)
    c.key("⌘Enter")
    c.send("clock", "advance", UNDO + 1)
    row = c.rows("mac-main.outbox.status")[0]
    first = (c.core("outbox_entry/0/state"), reason in row["text"], row["retry_offered"], c.core(SENT))
    c.send("fixture", "server-refuse", "off")
    c.click("mac-main.outbox.retry")
    c.send("clock", "advance", 1)
    return first + (c.core(SENT),)


@check("provider-failure-keeps-owners-text", "a failed AI draft wipes the owner's text", ("My text.", True))
def _(c):
    compose(c, "My text.")
    reason = T["ai"]["provider_failure_reason"]
    c.send("fixture", "provider-fail", reason)
    c.click("mac-composer.ai-draft")
    c.type(T["ai"]["instruction"])
    c.key("Enter")
    return (BODY(c), reason in c.rows("mac-composer.line")[0]["text"])


@check("empty-instruction-writes-nothing", "an empty AI instruction still runs a job", ("Keep.", 0, 1))
def _(c):
    compose(c, "Keep.")
    c.click("mac-composer.ai-draft")
    tolerate(c.key, "Enter")
    first = (BODY(c), c.core("ai_job/count"))
    c.click("mac-composer.ai-draft")
    c.type(T["ai"]["instruction"])
    c.key("Enter")
    return first + (c.core("ai_job/count"),)


@check("zero-instruction-is-a-real-instruction", "the instruction \"0\" is treated as empty", 1)
def _(c):
    compose(c, "Keep.")
    c.click("mac-composer.ai-draft")
    c.type(T["adversarial"]["coercion_bait"][0])
    tolerate(c.key, "Enter")
    return c.core("ai_job/count")


@check("long-body-sends-whole", "the body is cut at one thousand characters on send", (5000, True))
def _(c):
    compose(c, T["richtext"]["long_text"])
    c.key("⌘Enter")
    c.send("clock", "advance", UNDO + 1)
    last = c.core("server/%s/sent/last_body" % R1["account"])
    return (len(last) - len(last.lstrip(T["send"]["long_char"])), last.rstrip().endswith(SIG))


MUTANTS = {n: m for n, m, _, _ in REG.items}
J3_MUTANTS = {}
for line in open(os.path.join(ROOT, "journeys", "J3.py"), encoding="utf-8"):
    mm = re.match(r"#\s+(J3\.\d+/[\w-]+)\s*\|\s*(.+)", line)
    if mm:
        J3_MUTANTS[mm.group(1)] = mm.group(2).strip()

if __name__ == "__main__":
    sys.exit(main("composermac", [REG], {**MUTANTS, **J3_MUTANTS}, journey=os.path.join(ROOT, "journeys", "J3.py")))
