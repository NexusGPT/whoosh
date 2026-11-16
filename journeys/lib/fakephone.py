"""A stand-in for the iPhone app's control socket and for the Mac rig copy, for journeys/J7.py --selftest (no simulator runtime needed).

Ported from _captures/tests-j7/fakephone.py and cut to what J7 (J7.1 to J7.5) reads and does. It serves what docs/JOURNEYS.md J7, the design
fixture (docs/design/fixtures/mac-main-sample.json) and fixtures/tests-j7.json say the phone shows, so J7's own checks can be seen green on a model
of the right answer and red on one broken copy each. It proves the checks can fail; it proves nothing about the app.

A broken copy is `Phone.mut`:
    ("read", ("rows", ax) | ("core", path) | ("measure", ax) | ("uistate", ""), kind)   one read changed; kind: "empty" | "dropfirst" | "dup" | "change" | "dec" | "allids" | dict (row appended)
    ("act", (verb, key))                                                               one action answers ok and does nothing; key as in `Server.handle`
    ("flag", name)                                                                     a named behaviour of the model (`Phone.flag(name)` is asked where it applies)
Every read and act the model served is listed in `Phone.reads` / `Phone.acts` (a self-test can show what a broken copy touched).

`CopyServer` serves a journeys/lib/sharedserver.py stand-in copy (its --selftest model of one phone or Mac copy on one mail server) on the same
socket wire, so J7.5's shared-server mode (`drive_shared`) can run its own code against it.
"""
import json
import os
import re
import socket
import tempfile
import threading

from lib.fixture import FX, ROOT, const

T = json.load(open(os.path.join(ROOT, "fixtures", "tests-j7.json"), encoding="utf-8"))
ACC = {a["id"]: a for a in FX["accounts"]}
ARR = T["arrival"]
NOTE = "No push while your Mac sleeps"


class Phone:
    def __init__(self):
        self.mut = None
        self.reads, self.acts = [], []
        self.speech = ""
        self.reset()

    def flag(self, name):
        return self.mut == ("flag", name)

    def reset(self):
        self.undo_s = const("UNDO_SEND_DEFAULT_S")
        self.surface, self.cur, self.thread_from = "iphone-inbox", None, "iphone-inbox"
        self.m = {x["id"]: dict(x, archived=False, snoozed=False, read=not x["unread"], fetched=True) for x in FX["messages"]}
        self.order = [x["id"] for x in FX["messages"]]
        self.m[ARR["id"]] = dict(id=ARR["id"], account=ARR["account"], sender=ARR["sender"], subject=ARR["subject"], unread=True,
                                 archived=False, snoozed=False, read=False, fetched=False)
        self.undone = set()
        self.delivered, self.undo, self.toast, self.failure = False, [], False, None
        self.clip, self.offline, self.pill_copied = None, False, False
        self.outbox, self.clock, self.body_dictated, self.model = [], 0, "", "ready"
        self.sent_text = None
        self.speech = ""
        self.hidden = set()                    # rows a swipe hid without the core holding the change (the flags "archive-hides-only", "snooze-hides-only")

    # ---------- derived
    def visible(self):
        return [i for i in self.order if i not in self.hidden and not self.m[i]["archived"] and not self.m[i]["snoozed"]]

    def out_state(self, e):
        if e["cancelled"]:
            return "cancelled"
        return "sent" if self.clock >= e["until"] else "undoable"

    # ---------- reads
    def rows(self, ax):
        m = self.m
        if ax == "iphone-inbox.list":
            shown = self.visible()
            if self.flag("archive-keeps-row"):             # the swiped row is still drawn, at the end of the list
                shown += [i for i in self.order if m[i]["archived"]]
            if self.flag("snooze-keeps-row"):
                shown += [i for i in self.order if m[i]["snoozed"]]
            return [dict(id=i, sender=m[i]["sender"], subject=m[i]["subject"], account=m[i]["account"], unread=not m[i]["read"],
                         mark_colour=ACC[m[i]["account"]]["colour"]) for i in shown]
        if ax == "iphone-inbox.undo":
            return [dict(text="Undo")] if self.toast else []
        if ax == "iphone-inbox.failure":
            return [dict(text=self.failure)] if self.failure else []
        if ax == "iphone-inbox.accounts":
            return [dict(id=a["id"], name=a["name"], address=a["address"], colour=a["colour"],
                         note=(NOTE if (a["provider"] in T["no_push_providers"] and not self.flag("no-push-silent")) or self.flag("every-account-noted") else ""))
                    for a in FX["accounts"]]
        if ax == "iphone-lock.notifications":
            return [dict(sender=ARR["sender"], subject=ARR["subject"], account=ACC[ARR["account"]]["name"])] if self.delivered else []
        if ax == "iphone-lock.notifications.actions":
            return [dict(title="Archive"), dict(title="Reply")] if self.delivered else []
        if ax.startswith("iphone-thread."):
            return self.thread_rows(ax)
        if ax == "iphone-composer.from":
            return [dict(text=ACC[self.m[self.cur]["account"]]["address"])]
        if ax == "iphone-composer.body":
            return [dict(text=self.body_dictated, dictated=self.body_dictated, signature="" if self.flag("no-signature") else FX["open_thread"]["quick_reply"]["signature"])]
        if ax == "iphone-composer.dictate":
            return [dict(text="mic")]
        if ax == "iphone-composer.speech-model":
            return [dict(state="ready", progress=1.0)] if self.model == "ready" else [dict(state="downloading", progress=0.3)]
        if ax == "iphone-composer.undo":
            if any(self.out_state(e) == "sent" for e in self.outbox):
                return [dict(text="Sent")]
            return [dict(text="Undo", seconds=self.undo_s)] if any(self.out_state(e) == "undoable" for e in self.outbox) else []
        if ax == "mac-main.list":
            return [dict(id=i) for i in self.visible()]
        return None

    def thread_rows(self, ax):
        m = self.m[self.cur]
        if ax == "iphone-thread.subject":
            return [dict(subject=m["subject"])]
        if ax == "iphone-thread.code-pill":
            if self.cur != ARR["id"]:
                return []
            return [dict(code=ARR["code"], text=("Copied" if self.pill_copied else "Copy " + ARR["code"]), clipboard=("000000" if self.flag("wrong-clipboard") else self.clip))]
        if ax == "iphone-thread.body":
            return [dict(text="Not downloaded yet" if self.offline else "Your code is " + ARR["code"])]
        return None

    def core(self, path):
        p = [x for x in re.split(r"[/.]", path) if x]
        if p[0] == "message":
            x = self.m[p[1]]
            return {"archived": x["archived"], "read": x["read"], "snoozed": x["snoozed"], "account": x["account"], "fetched": x["fetched"]}[p[2]]
        if p[:2] == ["undo_entry", "count"]:
            return len(self.undo)
        if p[0] == "outbox_entry":
            return self.out_state(self.outbox[int(p[1])])
        raise KeyError(path)

    # ---------- acts
    def click(self, ax):
        m = re.match(r"iphone-inbox\.list\.row\.(\d+)(?:\.swipe-(archive|snooze))?$", ax)
        v = self.visible()
        if m:
            i = int(m.group(1))
            if i >= len(v):
                return False
            if not m.group(2):
                return self.open(v[i], "iphone-inbox")
            x = self.m[v[i]]
            if x["account"] == T["refused_account"]:
                self.failure = ACC[x["account"]]["name"] + " needs you to sign in again"
                if self.flag("refused-drops-row"):
                    self.hidden.add(x["id"])
                return True
            if self.flag("snooze-takes-row-above") and m.group(2) == "snooze" and i > 0:
                self.hidden.add(v[i - 1])
            if not self.flag(m.group(2) + "-hides-only"):
                x["archived" if m.group(2) == "archive" else "snoozed"] = True
            else:
                self.hidden.add(x["id"])
            self.undo.append(x["id"])
            self.toast = True
            return True
        if ax == "iphone-thread.back":
            self.surface = self.thread_from
            return True
        if ax == "iphone-thread.code-pill":
            self.pill_copied, self.clip = True, ARR["code"]
            self.m[ARR["id"]]["read"] = True
            return True
        if ax == "iphone-thread.reload":
            return True
        if ax == "iphone-thread.reply":
            self.surface, self.body_dictated = "iphone-composer", ""
            return True
        if ax == "iphone-composer.dictate":
            if self.model == "missing":
                self.model = "missing-active"           # the first tap with the model missing starts its download
                return True
            if self.speech and self.model == "ready":
                self.body_dictated = self.speech
            return True
        if ax == "iphone-composer.send":
            if self.surface != "iphone-composer":
                return False
            self.outbox.append(dict(until=self.clock + self.undo_s, cancelled=False))
            self.sent_text = self.body_dictated
            return True
        return False

    def open(self, mid, came_from):
        self.cur, self.surface, self.thread_from = mid, "iphone-thread", came_from
        x = self.m[mid]
        x["read"] = True
        if mid == ARR["id"]:
            x["fetched"] = True
        return True

    def fixture(self, args):
        a = args[0]
        if a == "reset":
            self.reset()
            return True
        if a == "deliver":
            if args[1] != ARR["id"]:
                return True
            self.delivered = True
            self.m[ARR["id"]]["fetched"] = True
            if ARR["id"] not in self.order:
                self.order.insert(0, ARR["id"])
            return True
        if a == "notification-tap":
            return self.open(ARR["id"], "iphone-inbox") if self.delivered else False
        if a == "offline":
            self.offline = args[1] == "on"
            return True
        if a == "speech-model":
            self.model = args[1]
            return True
        if a == "speak":
            self.speech = args[1]
            self.model = "ready"                        # the download landed while the words were being prepared
            return True
        return False

    def clock_adv(self, s):
        self.clock += s
        return True

    def uistate(self):
        return {"surface": self.surface}


class Server:
    """The control socket of one copy: the phone (mac=False) or the Mac rig copy (mac=True, rows only)."""

    def __init__(self, phone, mac=False):
        self.p, self.mac = phone, mac
        self.dir = tempfile.mkdtemp(prefix="fakephone")
        self.path = os.path.join(self.dir, "whoosh.sock")
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.bind(self.path)
        self.sock.listen(4)
        self.alive = True
        threading.Thread(target=self.loop, daemon=True).start()

    def loop(self):
        while self.alive:
            try:
                c, _ = self.sock.accept()
            except OSError:
                return
            threading.Thread(target=self.serve, args=(c,), daemon=True).start()

    def serve(self, c):
        buf = b""
        while True:
            try:
                d = c.recv(65536)
            except OSError:
                return
            if not d:
                return
            buf += d
            while b"\n" in buf:
                line, buf = buf.split(b"\n", 1)
                try:
                    rep = self.handle(json.loads(line))
                    c.sendall((json.dumps(rep) + "\n").encode())
                except OSError:
                    return

    def perturb(self, key, val):
        mu = self.p.mut
        if not mu or mu[0] != "read" or mu[1] != key:
            return val
        kind = mu[2]
        if isinstance(val, list) and isinstance(kind, dict):
            return val + [kind]
        if isinstance(val, list) and kind == "allids":
            return val + [dict(id=i) for i in self.p.m]
        if isinstance(val, list):
            if kind == "empty":
                return []
            if kind == "dropfirst":
                return val[1:]
            if kind == "dup":
                return val + val[:1]
            out = []
            for r in val:
                r = dict(r)
                for k, v in r.items():
                    r[k] = (v + "X") if isinstance(v, str) else (not v) if isinstance(v, bool) else (v + 1) if isinstance(v, (int, float)) else ([] if isinstance(v, list) else v)
                out.append(r)
            return out
        if isinstance(val, bool):
            return not val
        if isinstance(val, (int, float)):
            return val * 1000 + 1 if kind != "dec" else val - 1
        if isinstance(val, str):
            return val + "X"
        return "X"

    def handle(self, req):
        v, a, p = req["verb"], req.get("args", []), self.p
        ok = lambda **kw: dict(ok=True, **kw)
        err = lambda w: dict(ok=False, error=w)
        try:
            if v in ("click", "key", "type", "fixture", "clock") and not self.mac:
                key = (v, a[0] if v != "fixture" else a[0] + ":" + (a[1] if len(a) > 1 else ""))
                p.acts.append(key)
                if p.mut and p.mut[0] == "act" and p.mut[1] == key:
                    return ok()
                if v == "click":
                    return ok() if p.click(a[0]) else err("nothing at " + a[0])
                if v == "fixture":
                    return ok() if p.fixture(a) else err("reserved")
                if v == "clock":
                    return ok() if p.clock_adv(float(a[1])) else err("no")
                return err("no input")
            if v == "rows":
                r = p.rows(a[0])
                if r is None:
                    return err("no rows at " + a[0])
                p.reads.append(("rows", a[0]))
                return ok(rows=self.perturb(("rows", a[0]), r))
            if v == "uistate" and not self.mac:
                p.reads.append(("uistate", ""))
                s = p.uistate()
                if p.mut and p.mut[:2] == ("read", ("uistate", "")):
                    s = dict(s, surface=s["surface"] + "X")
                return ok(state=s)
            if v == "measure" and not self.mac:
                p.reads.append(("measure", a[0]))
                ms = 2000
                if p.mut and p.mut[:2] == ("read", ("measure", a[0])):
                    ms = 10 ** 7
                return ok(ms=ms)
            if v == "core" and not self.mac:
                val = p.core(a[1])
                p.reads.append(("core", a[1]))
                return ok(value=self.perturb(("core", a[1]), val))
        except (KeyError, IndexError, ValueError) as e:
            return err("unknown: %s" % e)
        return err("verb " + v)

    def close(self):
        self.alive = False
        self.sock.close()


class CopyServer(Server):
    """The same wire over a sharedserver.py stand-in copy (an object with click, type, key, uistate, rows; raising lib.ctl.Unreadable when the app refuses)."""

    def __init__(self, copy):
        self.copy = copy
        Server.__init__(self, None)

    def handle(self, req):
        from lib.ctl import Unreadable
        v, a, c = req["verb"], req.get("args", []), self.copy
        try:
            if v == "rows":
                return dict(ok=True, rows=c.rows(a[0]))
            if v == "uistate":
                return dict(ok=True, state=c.uistate())
            if v in ("click", "type", "key"):
                getattr(c, v)(a[0])
                return dict(ok=True)
        except Unreadable as e:
            return dict(ok=False, error=str(e))
        return dict(ok=False, error="verb " + v)
