#!/usr/bin/env python3
"""par-composerrich: the Mac composer's signature with formatting and pictures inside the text (item 05d1d9c3, parts 1 and 2; lane
gap-composer-rich). Drives the RUNNING hidden rig copy over the control socket through journeys/lib, as par-composermac.py does, and reads
the fixture core's record of the sent mail (`sent/last/*`, Sources/CoreBridge/FixtureCoreRich.swift: the model of what the core's builder,
core/crates/send/src/rich.rs, makes of the composer's text; the core itself is proven by core/crates/send/tests/rich_body.rs).
Run (hidden test copy only): sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/par-composerrich.py'
    python3 checks/parity/par-composerrich.py --selftest   every case against a model of the app, clean (all green), then once per mutant (each must go red)
Prints one line per case (`<label> pass <detail>`), the MUTANTS table, then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0.
Nothing leaves this Mac: a send reaches the fixture core's outbox only. The rig verbs set the From account's signature in the fixture core.
"""
import json
import os
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, print_cases, print_model_cases   # noqa: E402
from lib.ctl import Unreadable   # noqa: E402

ID = "par-composerrich"
TITLE = "Mac composer: signature with formatting, pictures in the text"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "the signature under the text shows its bold, italic and link; a picture dropped on the text sits in the text and goes out inline"
HEAD = "mac-composer.header"
SPEED = {"settle": 0.3}                                               # the selftest sets it to 0: the model has no frames to wait for

# MUTANTS (a case is green only after it has been seen red on its mutant; the selftest switches each on in the model)
# par-composerrich/signature-is-read-as-rich | a signature with marks is read as plain (ComposerRichMail.isRich always false)
# par-composerrich/rich-signature-html-holds-its-bold | the bold of the signature is not drawn in the HTML part
# par-composerrich/rich-signature-plain-part-has-its-words-without-marks | the plain part keeps the ** marks
# par-composerrich/plain-signature-outgoing-as-before | a plain signature is dropped from the text that goes out
# par-composerrich/plain-signature-gives-no-html | an HTML part is built for every mail, a plain signature too
# par-composerrich/picture-lands-in-the-text | a picture dropped on the text goes as a file tile only
# par-composerrich/picture-is-not-a-tile | the picture in the text is also drawn as a file tile
# par-composerrich/picture-kept-after-escape-and-reopen | Escape and reopen lose the picture
# par-composerrich/inline-picture-is-one-inline-part | the text that goes out does not name the picture, so no inline part
# par-composerrich/inline-picture-html-references-its-cid | the HTML part has no img for the picture
# par-composerrich/inline-picture-plain-part-has-a-fallback-line | the plain part has no line for the picture


def settle():
    time.sleep(SPEED["settle"])


def hrow(s):
    rows = s.ctl.rows(HEAD)
    return rows[0] if rows else {}


def core_json(s, path):
    """The core's value at a path, decoded from its JSON text; None when the core has no value."""
    v = s.ctl.core(path)
    if v is None:
        return None
    try:
        return json.loads(v) if isinstance(v, str) else v
    except ValueError:
        return v


def plain_mail_html(s):
    """The HTML part of the plain-signature mail, None when it has none. The socket's `core state` answers ok:false "no core value at
    <path>" for a nil value (ControlSocket/Verbs.swift:78) and the fixture serves nil both for "no HTML part" and "no sent mail"
    (FixtureCoreRich.swift:9, stateRichServed), so the plain part is read first: it proves the newest entry is THIS mail (its text and
    the plain signature, without the rich one); only then does a nil HTML part mean "no HTML part"."""
    plain = core_json(s, "sent/last/plain") or ""
    if not ("Thanks Karen." in plain and plain.rstrip().endswith("Whoosh studio") and "Paris" not in plain):
        return "the newest sent mail is not the plain-signature one: plain part %r" % plain[-80:]
    try:
        return core_json(s, "sent/last/html")
    except Unreadable as e:
        if "no core value at sent/last/html" in str(e):
            return None
        raise


def drive(j):
    s = j.step("par-composerrich", TITLE)
    # (1) a signature with formatting: a reply from the account that received the thread
    s.click("mac-main.reader.open-full-composer")
    settle()
    s.click("mac-composer.dev.rich-signature")
    s.expect("signature-is-read-as-rich", lambda: hrow(s).get("signature_rich"), True)
    s.type("Thanks Karen.")
    s.key("⌘Enter")
    settle()
    s.expect("rich-signature-html-holds-its-bold", lambda: "<b>Whoosh</b>" in (core_json(s, "sent/last/html") or ""), True)
    s.expect("rich-signature-plain-part-has-its-words-without-marks",
             lambda: (lambda p: "Thanks Karen." in p and "Whoosh studio, Paris" in p and "**" not in p)(core_json(s, "sent/last/plain") or ""), True)
    s.click("mac-composer.undo")
    settle()
    # (2) a plain signature behaves as before: the text and the signature, no HTML part
    s.click("mac-composer.dev.plain-signature")
    s.expect("plain-signature-outgoing-as-before", lambda: (lambda h: h.get("outgoing") == "Thanks Karen.\n\n" + h.get("signature", "?"))(hrow(s)), True)
    s.key("⌘Enter")
    settle()
    s.expect("plain-signature-gives-no-html", lambda: plain_mail_html(s), None)
    s.click("mac-composer.undo")
    settle()
    # (3) a picture dropped on the text (the rig verb stands for the drop): in the text, not a tile, kept, out as one inline part
    s.click("mac-composer.dev.picture")
    settle()
    s.expect("picture-lands-in-the-text", lambda: len(hrow(s).get("pictures") or []), 1)
    s.expect("picture-is-not-a-tile", lambda: [r.get("name") for r in s.ctl.rows("mac-composer.attachments")], [])
    s.key("Escape")
    settle()
    s.click("mac-main.reader.open-full-composer")
    settle()
    s.expect("picture-kept-after-escape-and-reopen", lambda: len(hrow(s).get("pictures") or []), 1)
    s.key("⌘Enter")
    settle()
    s.expect("inline-picture-is-one-inline-part", lambda: core_json(s, "sent/last/inline_parts"), 1)
    s.expect("inline-picture-html-references-its-cid", lambda: '<img src="cid:' in (core_json(s, "sent/last/html") or ""), True)
    s.expect("inline-picture-plain-part-has-a-fallback-line", lambda: "[picture: " in (core_json(s, "sent/last/plain") or ""), True)
    s.click("mac-composer.undo")


def main():
    path = os.path.abspath(__file__)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    print_cases(r)
    rows = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# par-composerrich/") and " | " in l]
    print("MUTANTS (a check is green only after it has been seen red on its mutant)")
    print("| check | mutant |")
    print("|---|---|")
    for k, v in rows:
        print("| %s | %s |" % (k, v))
    for line in r["fail_list"]:
        sys.stderr.write("FAIL %s\n" % line)
    for line in r["no_mutant"]:
        sys.stderr.write("NO MUTANT %s\n" % line)
    for line in r["stray_mutant"]:
        sys.stderr.write("STRAY MUTANT %s\n" % line)
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    ok = r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"]
    return 0 if ok else 1


# ---- the selftest: every case against a model of the app, clean and once per mutant ----
# RichModel answers the socket verbs `drive` uses the way ComposerModel+Pictures.swift, ComposerModel+Send.swift `outgoing`, ComposerRichMail.swift
# and the fixture's record (FixtureCoreRich.swift) are written to answer them. One mutant (named by its case) is on at a time; that case must go red.
class RichModel:
    RICH = "Assem\n**Whoosh** studio, *Paris*\n[whoosh.example](https://whoosh.example)"
    PLAIN = "Assem\nWhoosh studio"

    def __init__(self, mut):
        self.m = mut
        self.open, self.surface = False, "mac-main"
        self.sig, self.body, self.pics, self.tiles = "", "", [], []
        self.sent, self.counting, self.n = None, False, 0

    def on(self, name):
        return name in self.m

    # -- the composer
    def is_rich(self, sig):
        if self.on("signature-is-read-as-rich"):
            return False
        return any(k in sig for k in ("**", "*", "](http"))

    def runs_plain(self, sig):
        if self.on("rich-signature-plain-part-has-its-words-without-marks"):
            return sig
        return sig.replace("**", "").replace("*", "").replace("[whoosh.example](https://whoosh.example)", "whoosh.example <https://whoosh.example>")

    def outgoing(self):
        body = self.body + "".join("\n[picture:%s]" % p for p in self.pics if not self.on("inline-picture-is-one-inline-part"))
        if self.on("plain-signature-outgoing-as-before") and not self.is_rich(self.sig):
            return body
        return body + ("\n\n" + self.sig if self.sig else "")

    # -- the fixture's record of the send
    def send(self):
        body = self.outgoing()
        inline = [p for p in self.pics if "[picture:%s]" % p in body]
        rich = self.is_rich(self.sig)
        if not rich and not inline and not self.on("plain-signature-gives-no-html"):
            self.sent = {"html": None, "plain": body, "inline": 0}
        else:
            text = body.replace("\n\n" + self.sig, "") if self.sig else body
            plain = text
            html = "<!DOCTYPE html><html><body><div>" + text + "</div>"
            for p in inline:
                plain = plain.replace("[picture:%s]" % p, "" if self.on("inline-picture-plain-part-has-a-fallback-line") else "[picture: pic.png]")
                html = html.replace("[picture:%s]" % p, "" if self.on("inline-picture-html-references-its-cid") else '<img src="cid:pic-%s@x" alt="pic.png">' % p)
            if self.sig:
                sig_html = self.sig if self.on("rich-signature-html-holds-its-bold") else self.sig.replace("**Whoosh**", "<b>Whoosh</b>")
                html += "<br><div>-- <br>" + sig_html + "</div>"
                plain += "\n\n-- \n" + self.runs_plain(self.sig)
            self.sent = {"html": html + "</body></html>", "plain": plain, "inline": len(inline)}
        self.counting = True

    def click(self, ax):
        if ax == "mac-main.reader.open-full-composer":
            self.open, self.surface = True, "mac-composer"
            return True
        if not self.open:
            return False
        if ax == "mac-composer.dev.rich-signature":
            self.sig = self.RICH; return True
        if ax == "mac-composer.dev.plain-signature":
            self.sig = self.PLAIN; return True
        if ax == "mac-composer.dev.picture":
            self.n += 1
            if self.on("picture-lands-in-the-text"):
                self.tiles.append("whoosh-rig-picture.png")
            else:
                self.pics.append("attachment-%d" % self.n)
                if self.on("picture-is-not-a-tile"):
                    self.tiles.append("whoosh-rig-picture.png")
            return True
        if ax == "mac-composer.undo":
            if not self.counting:
                return False
            self.counting = False
            return True
        return False

    def key(self, chord):
        if chord == "⌘Enter" and self.open and not self.counting:
            self.send(); return True
        if chord == "Escape" and self.open:
            self.open, self.surface = False, "mac-main"
            if self.on("picture-kept-after-escape-and-reopen"):
                self.pics = []
            return True
        return False

    def rows(self, ax):
        if not self.open:
            return []
        if ax == HEAD:
            return [{"signature": self.sig, "signature_rich": self.is_rich(self.sig), "outgoing": self.outgoing(), "pictures": list(self.pics)}]
        if ax == "mac-composer.attachments":
            return [{"name": t} for t in self.tiles]
        return []

    def core(self, path):
        if self.sent is None or not path.startswith("sent/last/"):
            return None
        v = {"sent/last/html": self.sent["html"], "sent/last/plain": self.sent["plain"], "sent/last/inline_parts": self.sent["inline"]}.get(path)
        return None if v is None else json.dumps(v)

    def handle(self, verb, args):
        if verb == "key": return {"ok": True} if self.key(args[0]) else {"ok": False, "error": "key does nothing"}
        if verb == "type": self.body += args[0]; return {"ok": True}
        if verb == "click": return {"ok": True} if self.click(args[0]) else {"ok": False, "error": "did nothing"}
        if verb == "rows": return {"ok": True, "rows": self.rows(args[0])}
        if verb == "core":    # as the socket does (ControlSocket/Verbs.swift:78): a nil value is ok:false
            v = self.core(args[1])
            return {"ok": True, "value": v} if v is not None else {"ok": False, "error": "no core value at %s" % args[1]}
        if verb == "uistate": return {"ok": True, "state": {"surface": self.surface}}
        return {"ok": False, "error": "nothing"}


def serve_model(model, name):
    """A Unix socket answered by `model.handle`; returns (ctl, close)."""
    import shutil
    import socketserver
    import tempfile
    import threading
    from lib.ctl import Ctl                                              # noqa: E402
    d = tempfile.mkdtemp(prefix=name)

    class H(socketserver.StreamRequestHandler):
        def handle(self):
            for line in self.rfile:
                q = json.loads(line.decode())
                self.wfile.write((json.dumps(model.handle(q["verb"], q["args"])) + "\n").encode())
    srv = socketserver.ThreadingUnixStreamServer(os.path.join(d, "whoosh.sock"), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    ctl = Ctl(d).connect()

    def close():
        ctl.close(); srv.shutdown(); srv.server_close(); shutil.rmtree(d, ignore_errors=True)
    return ctl, close


def model_run(mut):
    """Drives every case over a socket served by RichModel with the mutants `mut` on; returns {case label: (state, detail)}."""
    from lib.harness import Journey                                      # noqa: E402
    SPEED["settle"] = 0.0
    ctl, close = serve_model(RichModel(mut), "par-crich-")
    try:
        j = Journey(ID, ctl)
        drive(j)
        return {c.label: (c.state, c.detail) for c in j.checks()}
    finally:
        close()


def selftest():
    bad = 0
    clean = model_run(set())
    print_model_cases(clean)
    for label, (state, why) in clean.items():
        if state != "pass":
            print("RED on the model: %s %s" % (label, why)); bad += 1
    print("model, no mutant: %d of %d checks green" % (sum(1 for st, _ in clean.values() if st == "pass"), len(clean)))
    declared = {l.split(" | ")[0][2:].strip(): l.split(" | ", 1)[1].strip() for l in open(os.path.abspath(__file__), encoding="utf-8") if l.startswith("# par-composerrich/")}
    for label in clean:
        name = label.split("/", 1)[1]
        if label not in declared:
            print("NO MUTANT %s" % label); bad += 1; continue
        got = model_run({name})
        state = got[label][0]
        others = sorted(k.split("/", 1)[1] for k, (st, _) in got.items() if st == "fail" and k != label)
        print("mutant %-58s -> %s%s" % (name, "RED" if state == "fail" else "NOT RED (%s)" % state, ("  also red: " + ", ".join(others)) if others else ""))
        if state != "fail": bad += 1
    for l in sorted(set(declared) - set(clean)):
        print("STRAY MUTANT %s" % l); bad += 1
    print("selftest: %d checks, %d mutants not red or missing" % (len(clean), bad))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else main())
