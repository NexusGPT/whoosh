"""Stand-in control socket for checks/shots-sheet.py --selftest. Standard library plus Pillow (the tool needs Pillow too).

A real Unix stream socket at <dir>/whoosh.sock that speaks the wire contract of docs/CONTROL-SOCKET.md for the verbs the tool
uses: fixture reset, theme, uistate, resize, shot (a surface id, or `<name> window:<id>`), rows, labels. It is a stand-in, never
proof of the real app. `mode` switches one fault on (a mutant):
  good | missing-picture | one-colour | clipped | theme-same | size-same | no-theme-verb | no-resize-verb
  | labels-clipped | labels-wrap-clipped | no-labels-verb | labels-null | labels-capped | labels-bad-count
  | labels-tolerance (not a fault: a need 0.4 pt over its box)
"""
import json
import os
import socket
import threading

from PIL import Image, ImageDraw

SURFACES = ["mac-main", "mac-composer", "mac-search", "mac-palette", "mac-trainer", "mac-notch", "mac-welcome-accounts",
            "mac-settings-accounts", "mac-settings-ai", "mac-settings-general", "mac-labels-contacts", "mac-mcp-notes",
            "mac-quickreply", "mac-settings-replies"]
START = {"w": 1000, "h": 700}
BAND = 28


class Standin:
    def __init__(self, sock_dir, mode="good"):
        self.dir, self.mode = sock_dir, mode
        self.path = os.path.join(sock_dir, "whoosh.sock")
        self.theme = "light"
        self.wins = {}
        self.cur = None
        self.srv = None
        self.th = None
        self.stop_flag = False

    def start(self):
        os.makedirs(self.dir, exist_ok=True)
        self.srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.srv.bind(self.path)
        self.srv.listen(1)
        self.srv.settimeout(0.2)
        self.th = threading.Thread(target=self._serve, daemon=True)
        self.th.start()
        return self

    def stop(self):
        self.stop_flag = True
        if self.th:
            self.th.join(timeout=5)
        if self.srv:
            self.srv.close()
        if os.path.exists(self.path):
            os.remove(self.path)

    def _serve(self):
        while not self.stop_flag:
            try:
                conn, _ = self.srv.accept()
            except socket.timeout:
                continue
            except OSError:
                return
            conn.settimeout(0.5)
            buf = b""
            while not self.stop_flag:
                try:
                    chunk = conn.recv(65536)
                except socket.timeout:
                    continue
                except OSError:
                    break
                if not chunk:
                    break
                buf += chunk
                while b"\n" in buf:
                    line, buf = buf.split(b"\n", 1)
                    try:
                        req = json.loads(line.decode())
                        rep = self.handle(req["verb"], [str(a) for a in req.get("args", [])])
                    except Exception as e:  # a stand-in fault is a refused request, never a crash of the thread
                        rep = {"ok": False, "error": "stand-in: %s" % e}
                    conn.sendall((json.dumps(rep) + "\n").encode())
            conn.close()

    # ---- verbs
    def handle(self, verb, a):
        m = self.mode
        if verb == "fixture" and a[:1] == ["reset"]:
            self.wins, self.cur = {}, None
            return {"ok": True}
        if verb == "theme":
            if m == "no-theme-verb":
                return {"ok": False, "error": "unknown verb theme"}
            if m != "theme-same":
                self.theme = a[0]
            return {"ok": True, "theme": self.theme}
        if verb == "uistate":
            return {"ok": True, "state": {"surface": self.cur or "mac-main", "theme": self.theme}}
        if verb == "resize":
            if m == "no-resize-verb":
                return {"ok": False, "error": "unknown verb resize"}
            w = self.wins.get(self.cur)
            if w is None:
                return {"ok": False, "error": "no window"}
            if m != "size-same":
                w["w"], w["h"] = int(float(a[0])), int(float(a[1]))
            return {"ok": True, "content": dict(w), "frame": {"w": w["w"], "h": w["h"] + BAND}, "window": self.cur}
        if verb == "rows":
            if a[0] == "mac-main.status.link":
                box = 180
                return {"ok": True, "rows": [{"ax": a[0], "text": "Provanta RFP: sign in again", "text_w": box + (40 if m == "clipped" else -12),
                                              "box_w": box, "line": "Synced", "line_w": 40, "line_box_w": box}]}
            return {"ok": False, "error": "no rows at %s" % a[0]}
        if verb == "labels":
            if m == "no-labels-verb":
                return {"ok": False, "error": "unknown verb labels"}
            win = [x[len("window:"):] for x in a if x.startswith("window:")]
            if not win or win[0] not in self.wins:
                return {"ok": False, "error": "no open window has the id %s" % (win[0] if win else "")}
            rows = self.labels()
            judged = sum(1 for r in rows if r["text"] is not None) + (1 if m == "labels-bad-count" else 0)
            return {"ok": True, "labels": rows, "judged": judged, "marked": len(rows), "capped": m == "labels-capped", "window": win[0]}
        if verb in ("shot", "capture"):
            args = a[1:] if verb == "capture" else a
            name, rest = args[0], args[1:]
            win = [x[len("window:"):] for x in rest if x.startswith("window:")]
            if win:
                if win[0] not in self.wins:
                    return {"ok": False, "error": "no open window has the id %s" % win[0]}
                self.cur = win[0]
            elif name in SURFACES:
                self.wins.setdefault(name, dict(START))
                self.cur = name
            elif self.cur is None:
                return {"ok": False, "error": "no window"}
            return self.shot(name)
        return {"ok": False, "error": "unknown verb %s" % verb}

    def labels(self):
        """Three marked views: a one-line title, a wrapping note, an icon with no text (text null, equal sizes)."""
        m = self.mode
        title = {"ax": "settings.row.title", "text": "Notifications for every account", "need_w": 150.0, "need_h": 17.0, "box_w": 180.0, "box_h": 17.0}
        note = {"ax": "settings.row.note", "text": "A long note that wraps", "need_w": 180.0, "need_h": 34.0, "box_w": 180.0, "box_h": 40.0}
        icon = {"ax": "settings.row.icon", "text": None, "need_w": 16.0, "need_h": 16.0, "box_w": 16.0, "box_h": 16.0}
        if m == "labels-clipped":
            title["need_w"] = 1800.0   # a title far wider than its box: under the ellipsis rule (shots-sheet.ellipsis_cut) fewer than 6 characters stay, so it is clipped
        if m == "labels-wrap-clipped":
            note["box_h"] = 17.0
        if m == "labels-tolerance":
            title["need_w"] = title["box_w"] + 0.4
        if m == "labels-null":
            return [{"ax": "settings.row.icon", "text": None, "need_w": 400.0, "need_h": 16.0, "box_w": 16.0, "box_h": 16.0}]
        return [title, note, icon]

    def shot(self, name):
        w = self.wins[self.cur]
        path = os.path.join(self.dir, "shots", name + ".png")
        os.makedirs(os.path.dirname(path), exist_ok=True)
        if self.mode != "missing-picture":
            dark = self.theme == "dark"
            bg = (30, 30, 32) if dark else (245, 245, 247)
            im = Image.new("RGB", (w["w"], w["h"] + BAND), bg)
            if self.mode != "one-colour":
                d = ImageDraw.Draw(im)
                ink = (230, 230, 235) if dark else (20, 20, 24)
                d.rectangle([0, 0, w["w"], BAND], fill=(60, 60, 64) if dark else (225, 225, 228))
                for i in range(6):
                    d.rectangle([20, BAND + 20 + i * 40, 20 + 100 + (hash(self.cur) % 7 + i) * 20, BAND + 40 + i * 40], fill=ink)
            im.save(path)
            im.close()
        return {"ok": True, "path": path, "source": "stand-in", "window": self.cur, "title": self.cur,
                "mapping": {"x": 0, "y": BAND, "scale": 1}}
