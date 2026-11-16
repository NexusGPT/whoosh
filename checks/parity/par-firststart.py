#!/usr/bin/env python3
"""par-firststart: the first-start flow of J9 (docs/JOURNEYS.md "## J9", rows J9.1 to J9.8; SPEC A82 to A89), driven on the running rig copy.

The flow is the three picked boards built: 1a Staged inbox (the real window, empty, one pane lit per step), 2b Aurora stage (dot art
on an aurora ground, loops of 8 s), 3a Icon hero (the welcome card), and 2a's real-row try-it on the keys step. Expected values are
written here from the boards' copy and notes, never read from a built screen.

  `fixture first-start on|off`          opens the flow at step 1 (flag cleared) or closes it (docs/CONTROL-SOCKET.md delta 57)
  `fixture first-start reduce on|off`   its Reduced Motion switch: loops freeze on a key frame, the morph is a cut
  `fixture import-sources apple-mail=120`  what step 8's scan finds, then Start and `fixture import-progress`

Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/parity/par-firststart.py'
  python3 checks/parity/par-firststart.py --dry        every check counted, none judged (no socket, no traceback)
  python3 checks/parity/par-firststart.py --selftest   every check against a model, clean (all green), then once per mutant (each must go red)
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0.
Step ids are the journey's J9.1 to J9.8; the harness reads mutant labels only in that form (journeys/lib/harness.py:113).
Step 7 is the AI step (J9.10) and step 8 the Réponses step (J9.9 here: the journey's ids J9.7 and J9.8 keep their numbers); its writes are read from the core's own
state paths (reply/use/auto, reply/sources, reply/voice).
Gaps named, not hidden: the live drive cannot reach "no account yet" (the fixture core always holds accounts), so Continue-disabled at
step 6 and the launch rule's `would_show` true are read in the model and in the source checks only; the rig has no clock verb, so the 8 s
loop seam waits 8 s of wall time per step."""
import http.server
import json
import os
import re
import shutil
import socketserver
import sys
import tempfile
import threading
import time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey, Journey, print_cases, print_model_cases                      # noqa: E402
from lib.fixture import const                                      # noqa: E402
from lib.ctl import Ctl, Unreadable                                # noqa: E402

ID = "par-firststart"
TITLE = "First start: the 9-step flow, the first account, the reply choices, the inbox"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "the first launch teaches the app in 9 steps, connects the first account, sets how replies are written and opens the inbox; a second launch never shows it"

X = "mac-first-start"
IMP = "mac-welcome-accounts.import"
ADD = "mac-add-account"
FAST = "--selftest" in sys.argv
SLEEP = time.sleep
NOW = time.monotonic          # the clock the loop seam reads; --selftest points it at the model's clock
SHOT_LAT = 1.1                # the model's `shot` takes this long, like the rig's (1.1 to 1.6 s measured on main54): the seam must not depend on it
SRC = os.path.join(ROOT, "Sources", "Features", "FirstStart")
XCS = os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings")
FIXCORE = os.path.join(ROOT, "Sources", "CoreBridge", "FixtureCore.swift")
LANGS = ["da", "de", "en", "es", "fi", "fr", "it", "ja", "nb", "nl", "pl", "pt", "sv", "zh-Hans"]
CLICKED, SEEN = set(), set()
AI_CLICKED, AI_SEEN = set(), set()      # the AI step's doors as `<line>.<action>`: clicked by a case, and offered by a line's `actions`

# The boards' copy, word for word (3a for step 1, 2b for steps 2 to 5)
WELCOME = ("Welcome to", "Whoosh", "All your inboxes in one fast, quiet window, sorted by a model that never leaves this Mac.")
SEARCH = ("Search", "Find it by what it means",
          "Type what you remember, not the exact words. Whoosh searches every account and every year at once.")
KEYS = ("Keys", "Five keys do most of it",
        "One letter each, never a chord. Watch the stage, then try them: J and K move, E marks done, H reminds you later.")
LABELS = ("Labels", "Your rules first, then the model",
          "Every line carries its label and who decided it. One of your rules wins when it matches. When none does, the model on this Mac picks.")
PRIVACY = ("Privacy", "Your mail stays on this Mac",
           "Your mail, the search index and the model that sorts it live on this Mac. Two parts are optional and stay off until you turn them on.")
NEVER = "Nothing is sent to Microsoft or Google."
AI_CARD = ("AI", "What the AI may use", "Three things make the AI parts work. Set them now or later: Whoosh marks what is missing.")
REPLIES = ("Replies", "Replies written like you",
           "Whoosh prepares a draft; you read it and send it. Three choices are enough, and you change everything later in Settings.")
REPLY_ROWS = [("draft", "Write a draft", "", "When I ask"), ("search", "Search in", "Indexed on this Mac. Nothing leaves it.", "Sent and Inbox"),
              ("voice", "Write like", "An analysis of the messages you sent.", "My last 100 messages")]
REPLY_PENDING = os.path.join(ROOT, "docs", "design", "fixtures", "firststart-catalogue-additions.json")   # strings not in the catalogue yet: French and English
RP = X + ".replies"
SAMPLE = ["Linea Supply", "Marie Lefèvre", "Ton Verheyden"]
FLAG = "whoosh.firstStart.finished"
MOTION_MS = lambda: const("MOTION_MAX_MS")            # read inside a check, so a missing row fails that check only


def pick(row, *keys):
    return tuple(row.get(k) for k in keys)


# ---- the driver's helpers: every click is recorded, every controls row is noted ----
def state(s):
    return s.ctl.rows(X + ".state")[0]


def card(s):
    r = s.ctl.rows(X + ".card")
    return r[0] if r else {}


def controls(s):
    out = [(r["id"], r["title"], r["key"], r["enabled"]) for r in s.ctl.rows(X + ".controls")]
    SEEN.update(c[0] for c in out)
    return out


def click(s, ax):
    CLICKED.add(ax.rsplit(".", 1)[-1])
    if ax.startswith(X + ".ai."): AI_CLICKED.add(ax[len(X) + 4:])
    s.click(ax)


def on(s):
    s.raw("fixture", "first-start", "reduce", "off")
    s.raw("fixture", "first-start", "on")


def go_to(s, n):
    """From step 1, Continue until step n."""
    for _ in range(n - 1):
        click(s, X + ".continue")


def waited(s, thunk, want, tries=10):
    """A read that follows an act the screen finishes on its next turn (the add card's list, the import panel's Skip)."""
    got = thunk()
    for _ in range(tries):
        if got == want:
            return got
        SLEEP(0.3)
        got = thunk()
    return got


def inbox(s):
    return [(r.get("sender"), r.get("subject")) for r in s.ctl.rows("mac-main.list")]


def window_w(s):
    return s.ctl.rows(X + ".geometry")[0]["window"]["w"]


def png_diff(a, b):
    """The share of pixels that differ between two shots, 0 to 100."""
    from PIL import Image, ImageChops
    x, y = Image.open(a).convert("RGB"), Image.open(b).convert("RGB")
    if x.size != y.size:
        return 100.0
    d = ImageChops.difference(x, y).convert("L").point(lambda v: 255 if v > 8 else 0)
    return 100.0 * sum(1 for v in d.tobytes() if v) / (x.size[0] * x.size[1])


def shot(s, name):
    return s.ctl.shot(name)["path"]


# A mark drawn on the stage pane (a ring, a word) shows in each theme. The least contrast it must reach against the pane it sits on: 3 to 1, the WCAG 2.1 bar for
# a non-text mark (1.4.11). Measured on main182's pictures (14 Nov 2026): the pane's own dots reach 1.17 to 2.32, the white rings 5.5 on the dark pane and 1.24 to
# 1.33 on the light pane (not seen), the title 11.5 and 12.4.
STAGE_MARK_MIN_CONTRAST = 3.0


def _lum(c):
    def lin(v):
        v /= 255.0
        return v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4
    return 0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])


def ink_contrast(path, mapping, fr):
    """The contrast of what is drawn in the frame against the pane it is drawn on: the frame's pixel of median brightness is taken as the ground (the pane for a ring
    or a word, the mark for a filled ring), and the answer is the mean contrast of the tenth of the pixels that differ most from it."""
    from PIL import Image
    sc, ox, oy = mapping.get("scale") or 1, mapping.get("x") or 0, mapping.get("y") or 0
    box = [int(round(v)) for v in ((fr["x"] + ox) * sc, (fr["y"] + oy) * sc, (fr["x"] + fr["w"] + ox) * sc, (fr["y"] + fr["h"] + oy) * sc)]
    with Image.open(path) as im:
        b = (max(0, box[0]), max(0, box[1]), min(im.size[0], box[2]), min(im.size[1], box[3]))
        if b[2] - b[0] < 4 or b[3] - b[1] < 4:
            raise Unreadable("the frame %r lies outside the picture %dx%d" % (fr, im.size[0], im.size[1]))
        rgb = im.convert("RGB").crop(b)
        raw = rgb.tobytes()
    lums = sorted(_lum(raw[i:i + 3]) for i in range(0, len(raw), 3))
    ground = lums[len(lums) // 2]
    cs = sorted(((max(v, ground) + 0.05) / (min(v, ground) + 0.05) for v in lums), reverse=True)
    top = cs[:max(1, len(cs) // 10)]
    return sum(top) / len(top)


def stage_marks(s, tag, ids):
    """Each mark's contrast on the pane, in the light theme and in the dark theme: {theme: [reaches STAGE_MARK_MIN_CONTRAST, ...]}. The figures go to stderr."""
    out = {}
    try:
        for theme in ("light", "dark"):
            s.ctl.send("theme", theme)
            # a ring that has just filled is still fading in (POPOVER_MS): main187 read a filled ring at 3.44 in the first picture and 14.09 in the second
            SLEEP(2 * const("POPOVER_MS") / 1000.0 + 0.2)
            reply = s.ctl.send("capture", "shot", "fs-%s-%s" % (tag, theme), *ids)
            frames, mp = reply.get("frames") or {}, reply.get("mapping") or {}
            got = []
            for ax in ids:
                if not frames.get(ax):
                    raise Unreadable("the shot answered no frame for %s (frames: %r)" % (ax, sorted(frames)))
                got.append(ink_contrast(reply["path"], mp, frames[ax]))
            sys.stderr.write("par-firststart stage marks, %s, %s theme: %s (least %.1f)\n" % (tag, theme, ", ".join("%.2f" % c for c in got), STAGE_MARK_MIN_CONTRAST))
            out[theme] = [c >= STAGE_MARK_MIN_CONTRAST for c in got]
    finally:
        s.ctl.send("theme", "system")
    return out


def seam(s, step):
    """A84: two shots of one step one loop apart differ by under 1 % of the pixels (the loop's last frame equals its first).
    The rig's `shot` takes 1.1 to 1.6 s (main54, _captures/firststart-r2/probe-main54.log), so a sleep after the first shot returns would
    compare phases 1 s apart, and the first shot follows a Continue whose morph is still in flight (step 5 shot A showed step 4's card): the second shot is started one loop after the FIRST ONE STARTED, so both land on the same loop phase."""
    st = state(s)
    SLEEP(st["entrance_ms"] / 1000 + 0.45)                      # settle: the card morph, the entrance (550 ms) and a row's exit are not the loop
    # Up to three pairs: on a loaded machine a shot can start late and land on another phase of the loop (main144 at load 102: steps 3 and 5 red; main148 at
    # load 38: green; main150 at load 66: step 3 red). A loop whose last frame differs from its first differs in EVERY pair taken one loop apart, so one pair
    # that matches proves the seam; a late shot proves nothing.
    d = 100.0
    for attempt in range(1 if FAST else 3):
        t0 = NOW()
        a = shot(s, "fs-seam-%d-a" % step)
        while NOW() < t0 + st["loop_s"]:
            SLEEP(max(0.0, min(0.005, t0 + st["loop_s"] - NOW())))    # never negative: main81 crashed here (ValueError) when the deadline passed between the test and the sleep, under load
        b = shot(s, "fs-seam-%d-b" % step)
        d = round(png_diff(a, b), 2)
        keep = os.environ.get("FS_KEEP_SHOTS")                      # a drive may keep the pair side by side (_captures/firststart-r2/)
        if keep and not FAST:
            for n, f in (("a", a), ("b", b)):
                shutil.copy(f, os.path.join(keep, "seam-step%d-%s.png" % (step, n)))
            sys.stderr.write("seam step %d, pair %d: %.2f %% differ\n" % (step, attempt + 1, d))
        if d < 1.0:
            return True
    return False


# ---- the source checks read these, so --selftest can point them at a scratch copy with one edit ----
def _src(name):
    return open(os.path.join(SRC, name), encoding="utf-8").read()


def _all_src():
    return {f: _src(f) for f in sorted(os.listdir(SRC)) if f.endswith(".swift")}


def no_account(j):
    """`fixture accounts none` (docs/CONTROL-SOCKET.md delta 57, main54): the fixture core lists no account, the state the flow opens on.
    Bytes that do not serve the verb predate the flow (main53 has neither): the drive says so and judges nothing on the socket
    (COULD-NOT-DRIVE, never red); the source checks still run."""
    if j.ctl is None:
        return
    try:
        j.ctl.send("fixture", "accounts", "none")
    except Unreadable as e:
        why = "the first-start flow is not in these bytes: `fixture accounts none` answered %s (the verb and the flow land in main54)" % e
        sys.stderr.write("par-firststart could-not-drive: %s\n" % why)
        j.notes.append("could-not-drive: " + why)
        j.ctl, j.dry_reason = None, why


def drive(j):
    no_account(j)
    steps_1_to_5(j)
    steps_6_to_8(j)
    steps_ai(j)
    steps_replies(j)    # after J9.6: Continue at step 6 needs the account that step added
    steps_again(j)      # J9.11: an installation that has an account opens the flow by its two doors
    steps_source(j)


def steps_1_to_5(j):
    # ---- J9.1: "What Whoosh is" (3a Icon hero) ----
    s = j.step("J9.1", "What Whoosh is")
    on(s)
    s.expect("welcome-words", lambda: pick(card(s), "kicker", "title", "body", "count", "dot_current"), WELCOME + ("Step 1 of 10", 1))
    s.expect("welcome-controls-show-their-keys", lambda: controls(s), [("skip", "Skip", "esc", True), ("continue", "Continue", "↩", True)])
    s.expect("welcome-sizes-from-the-board", lambda: sizes(s) == sizes_expected(s), True)
    s.expect("motion-budget-in-the-state", lambda: tuple(pick(state(s), "morph_ms", "veil_ms", "loop_s", "entrance_ms")), (MOTION_MS(), MOTION_MS(), 8, 550))
    s.expect("only-the-flow-answers-keys", lambda: keys_stay_out(s), True)
    s.expect("loop-seam-step1", lambda: seam(s, 1), True)
    click(s, X + ".skip")
    s.expect("skip-from-step-1-jumps-to-6", lambda: state(s)["step"], 6)
    on(s)
    s.key("Enter")
    s.expect("enter-continues", lambda: state(s)["step"], 2)

    # ---- J9.2: "Search" ----
    s = j.step("J9.2", "Search")
    s.expect("search-card-words", lambda: pick(card(s), "kicker", "title", "body", "count", "dot_current"), SEARCH + ("Step 2 of 10", 2))
    s.expect("back-continue-skip-show-their-keys", lambda: controls(s),
             [("back", "Back", "⌘[", True), ("skip", "Skip", "esc", True), ("continue", "Continue", "↩", True)])
    s.expect("search-lights-the-list-only", lambda: lit(s), (False, False, True, False))
    s.type("boat")
    s.expect("boat-finds-the-sailing-club-by-meaning", lambda: pick(s.ctl.rows(X + ".search")[0], "typed", "match", "why"),
             ("boat", "Sailing club", "Found by meaning: boat ≈ Sailing club"))
    s.key("⌘[")
    s.key("Enter")
    s.type("zzzz")
    s.expect("a-word-with-no-meaning-says-so", lambda: pick(s.ctl.rows(X + ".search")[0], "match", "why"),
             ("", "No sample line matches “zzzz”. Try boat, invoice, logo or race."))
    s.expect("loop-seam-step2", lambda: seam(s, 2), True)
    s.raw("fixture", "first-start", "reduce", "on")
    s.expect("reduced-motion-freezes-the-loop", lambda: frozen(s), (True, 0.0))
    s.raw("fixture", "first-start", "reduce", "off")
    s.expect("loops-run-without-reduced-motion", lambda: moving(s), True)
    s.key("⌘[")
    s.expect("back-goes-to-step-1", lambda: state(s)["step"], 1)
    s.key("Enter")
    s.key("Escape")
    s.expect("esc-skips-to-6", lambda: state(s)["step"], 6)
    on(s)
    go_to(s, 3)

    # ---- J9.3: "Keys" (2b with 2a's real-row try-it) ----
    s = j.step("J9.3", "Keys")
    s.expect("keys-card-words", lambda: pick(card(s), "kicker", "title", "body", "count"), KEYS + ("Step 3 of 10",))
    s.expect("three-sample-rows-and-the-caps", lambda: pick(s.ctl.rows(X + ".keys")[0], "rows", "focus", "caps"), (SAMPLE, 0, ["E", "H", "C", "/", "J", "K"]))
    before = []
    s.expect("j-and-k-move-the-focus", lambda: (before.append(inbox(s)), key_focus(s))[1], (1, 0))
    s.key("J")
    s.key("E")
    s.expect("e-makes-the-focused-line-leave", lambda: pick(s.ctl.rows(X + ".keys")[0], "rows", "last", "last_row"), (["Linea Supply", "Ton Verheyden"], "done", "Marie Lefèvre"))
    s.key("H")
    s.expect("h-reminds-and-the-line-leaves", lambda: pick(s.ctl.rows(X + ".keys")[0], "rows", "last", "last_row"), (["Linea Supply"], "remind", "Ton Verheyden"))
    s.expect("the-try-it-never-touches-the-inbox", lambda: inbox(s) == before[0], True)
    s.expect("loop-seam-step3", lambda: seam(s, 3), True)
    s.key("Enter")

    # ---- J9.4: "Labels and decisions" ----
    s = j.step("J9.4", "Labels and decisions")
    s.expect("labels-card-words", lambda: pick(card(s), "kicker", "title", "body", "count"), LABELS + ("Step 4 of 10",))
    s.expect("labels-lights-the-sidebar-and-the-list", lambda: lit(s), (False, True, True, False))
    s.expect("loop-seam-step4", lambda: seam(s, 4), True)
    s.key("Enter")

    # ---- J9.5: "Privacy" ----
    s = j.step("J9.5", "Privacy")
    s.expect("privacy-card-words", lambda: pick(card(s), "kicker", "title", "body", "count"), PRIVACY + ("Step 5 of 10",))
    s.expect("the-two-optional-parts-are-off", lambda: ([(o["name"], o["on"]) for o in card(s)["optional"]], card(s)["never"]),
             ([("Jev key", False), ("Push relay", False)], NEVER))
    s.expect("privacy-lights-the-reader-only", lambda: lit(s), (False, False, False, True))
    s.expect("loop-seam-step5", lambda: seam(s, 5), True)


def reply_core(s):
    """What the core holds for the reply settings: the auto switch, each default folder's switch, the voice sample (the contract's own state paths)."""
    src = s.ctl.core("reply/sources")
    folders = {n: sorted({f["enabled"] for a in src["accounts"] for f in a["folders"] if f["source"]["folder"] == n}) for n in ("INBOX", "Sent")}
    return s.ctl.core("reply/use/auto")["enabled"], folders, s.ctl.core("reply/voice")["voice"]["sample_size"]


def at_replies(s):
    on(s)
    go_to(s, 8)


def ai_lines(s):
    out = {r["id"]: r for r in s.ctl.rows(X + ".ai")}
    AI_SEEN.update("%s.%s" % (k, a) for k, v in out.items() for a in v.get("actions", []))
    return out


LAYA_FILES = ["typed-decisions/rl_agent_config.json", "typed-decisions/encoder/config.json", "typed-decisions/tokenizer/tokenizer.json",
              "typed-decisions/tokenizer/tokenizer_config.json", "typed-decisions/model.safetensors"]       # what LayaOnDevice asks for (the same list as checks/settingsaimac-check.py)
LAYA_SIZE = 150000


class Loop:
    """A provider and a model server on 127.0.0.1 only (docs/CONTROL-SOCKET.md delta 60: `fixture provider-endpoint <provider|laya> <url>`).
    POST /validate answers a key check with 200; GET/HEAD of the Laya files serve LAYA_SIZE bytes, the big one in ten pieces `slow` seconds apart."""
    def __init__(self):
        self.posts, self.slow = [], 0.0
        outer = self

        class H(http.server.BaseHTTPRequestHandler):
            def log_message(self, *a):
                pass

            def do_POST(self):
                self.rfile.read(int(self.headers.get("Content-Length", "0")))
                outer.posts.append((self.path, self.headers.get("Authorization", "").startswith("Bearer ")))     # only whether a key came, never the key
                data = json.dumps({"ok": True, "models": ["claude-sonnet-5-5", "claude-haiku-5-5"]}).encode()
                self.send_response(200); self.send_header("Content-Type", "application/json"); self.send_header("Content-Length", str(len(data))); self.end_headers()
                self.wfile.write(data)

            def _file(self, head):
                path = self.path.lstrip("/")
                if path not in LAYA_FILES:
                    self.send_response(404); self.send_header("Content-Length", "0"); self.end_headers(); return
                self.send_response(200); self.send_header("Content-Length", str(LAYA_SIZE)); self.end_headers()
                if not head:
                    try:
                        for _ in range(10):
                            self.wfile.write(b"A" * (LAYA_SIZE // 10)); self.wfile.flush()
                            time.sleep(outer.slow if path == LAYA_FILES[-1] else 0)
                    except (BrokenPipeError, ConnectionResetError):      # the app cancelled
                        pass

            def do_HEAD(self):
                self._file(True)

            def do_GET(self):
                self._file(False)

        self.srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
        self.url = "http://127.0.0.1:%d" % self.srv.server_address[1]
        threading.Thread(target=self.srv.serve_forever, daemon=True).start()

    def close(self):
        self.srv.shutdown(); self.srv.server_close()


def rig_folder(s, *parts):
    """A folder of the rig copy's OWN run folder (`<run>/<parts>`), emptied so a download starts from nothing: the owner's folder is refused.
    The weights stay on the disk across `fixture reset` (LayaModelStore.refresh reads the disk), so a download that ended in an earlier run of the
    same copy leaves the model ready and the Download door has nothing to start (main88: 'ready' where 'working' was expected). Unreadable when
    the run folder is not known or the folder cannot be emptied."""
    if FAST:
        return
    run = getattr(s.ctl, "dir", "")
    if not run or not os.path.exists(os.path.join(run, "whoosh.sock")):
        raise Unreadable("no rig run folder (WHOOSH_SOCKET_DIR): the models folder to empty is unknown")
    owner = os.path.realpath(os.path.expanduser("~/Library/Application Support/Whoosh"))
    folder = os.path.realpath(os.path.join(run, *parts))
    if folder == owner or folder.startswith(owner + os.sep):
        raise Unreadable("the models folder %s is the owner's, never emptied by a check" % folder)
    shutil.rmtree(folder, ignore_errors=True)
    if os.path.exists(folder):
        raise Unreadable("the rig's models folder %s could not be emptied" % folder)


def meaning_row(s):
    return [r for r in s.ctl.rows("ai-setup.status") if r["need"] == "meaning"][0]


def meaning_missing(s):
    """The search model not on this Mac, as a person with a new install has it: the rig's run folder holds no weights and the engine holds no embedder
    (`fixture embedder unloaded`). Unreadable when the line still does not say 'meaning-missing' (the Download door then proves nothing)."""
    rig_folder(s, "models", "embeddinggemma-2")
    s.raw("fixture", "embedder", "unloaded")
    r = meaning_row(s)
    if r["reason"] != "meaning-missing":
        raise Unreadable("the search model line reads %r (%r) after the run folder's weights were emptied and the embedder unloaded, not 'meaning-missing': the Download door cannot be tried from a known state" % (r["reason"], r["state"]))


def sorting(s):
    r = ai_lines(s)["sorting"]
    return r["state"], r.get("progress"), r["actions"]


def ai_setup(s, *pairs):
    for need, st in pairs:
        s.click("ai-setup.preview.%s.%s" % (need, st))


def at_ai(s):
    on(s)
    go_to(s, 7)


def steps_ai(j):
    # ---- J9.10: "AI" (design Q1 of ai-setup-r1: one card, three lines; the models and the key the AI parts need) ----
    s = j.step("J9.10", "AI")
    AI_CLICKED.clear(); AI_SEEN.clear()
    loop = Loop()
    try:
        steps_ai_run(s, loop)
    finally:
        loop.close()
        for who in ("anthropic", "laya"):
            try: s.ctl.send("fixture", "provider-endpoint", who, "")
            except Unreadable: pass
        s.raw("fixture", "providers", "default")
        s.raw("fixture", "first-start", "off")


def steps_ai_run(s, loop):
    at_ai(s)
    s.raw("fixture", "providers", "none")   # the fixture core holds "On this Mac" connected from the start, so the writing need would read ready: stage a core with no provider
    ai_setup(s, ("decision", "missing"), ("writing", "missing"), ("meaning", "missing"))
    s.expect("ai-card-words", lambda: pick(card(s), "kicker", "title", "body", "count", "dot_current"), AI_CARD + ("Step 7 of 10", 7))
    def body_under_card():
        """The hidden rig's `shot` pastes each web view's own picture LAST (Sources/App/WindowShot.swift, ShotWeb.draw), so a web body lands over a SwiftUI card drawn above it: an
        artefact of the picture, not of the app (the card is above the body in the window). The verb now clips the web picture around the first-start card and the palette. Judged
        here: when the card's frame meets the body's frame, the shot's `source` says it clipped around a cover. True = a picture that pastes the body over the card."""
        try:
            s.ctl.rows("mac-main.reader.body")[0]
        except (Unreadable, IndexError):
            return False                    # no body on screen: nothing can be pasted over the card
        # both frames come back with the picture, read in the same main-thread turn as its pixels (a `measure` alone found nothing at the card on main111: the probe's readers
        # were not mounted yet, "9 accessibility elements walked")
        reply = s.ctl.send("capture", "shot", "fs-cover", X + ".card", "mac-main.reader.body")
        fr = reply.get("frames") or {}
        c, b = fr.get(X + ".card"), fr.get("mac-main.reader.body")
        if not c or not b:
            raise Unreadable("the shot answered no frame for the card or the body: %r" % fr)
        meet = c["x"] < b["x"] + b["w"] and b["x"] < c["x"] + c["w"] and c["y"] < b["y"] + b["h"] and b["y"] < c["y"] + c["h"]
        src = str(reply.get("source", ""))
        return bool(meet and "clipped around" not in src)
    s.expect("ai-card-hides-the-mail-body-under-it", body_under_card, False)
    s.expect("ai-controls-show-their-keys", lambda: controls(s), [("back", "Back", "⌘[", True), ("skip", "Skip", "esc", True), ("continue", "Continue", "↩", True)])
    s.expect("ai-lights-the-list-only", lambda: lit(s), (False, False, True, False))
    s.expect("ai-three-lines-with-their-actions", lambda: [(k, v["actions"]) for k, v in sorted(ai_lines(s).items())],
             [("meaning", ["download"]), ("sorting", ["use-jev", "download"]), ("writing", ["key"])])
    ai_setup(s, ("decision", "working"), ("writing", "ready"))
    s.expect("ai-in-progress-states-as-drawn", lambda: (lambda l: (l["sorting"]["words"], round(l["sorting"]["progress"], 2), l["sorting"]["actions"], l["writing"]["detail"], l["writing"]["actions"]))(ai_lines(s)),
             ("Downloading 312 MB of 843 MB", 0.37, ["cancel"], "Anthropic · 6 models", []))
    s.expect("ai-rings-show-on-the-pane-in-both-themes", lambda: stage_marks(s, "rings", [X + ".ai.ring.%s.mark" % n for n in ("writing", "decision", "meaning")]),
             {"light": [True, True, True], "dark": [True, True, True]})   # one ring filled (writing), two empty: both shapes are read
    s.expect("ai-rings-fill-as-each-need-is-met", lambda: card(s)["rings"], [True, False, False])   # the row lists the needs in AINeed order (writing, decision, meaning: AISetupStatus.swift:8, FirstStartAIRows.swift:42); staged above: writing ready, decision working, meaning missing
    ai_setup(s, ("decision", "clear"), ("writing", "clear"), ("meaning", "clear"))
    s.click("ai-setup.reset")
    s.raw("fixture", "provider-endpoint", "anthropic", loop.url)
    s.expect("ai-pasted-key-reaches-the-provider", lambda: (click(s, RP.replace(".replies", ".ai") + ".writing.key"), s.ctl.rows("ai-setup.status")[0]["reason"])[-1] != "provider-none", True)
    s.expect("ai-pasted-key-leaves-the-writing-line-connected", lambda: (waited(s, lambda: (lambda l: (l["writing"]["state"], l["writing"]["actions"]))(ai_lines(s)), ("ready", [])), loop.posts[:1]),
             (("ready", []), [("/validate", True)]))      # the loopback provider was asked once to check the pasted key (a bearer came with it) and the line reads connected
    # The decision model is the core's state and `ai-setup.reset` keeps it: once the Jev cases below chose Jev the sorting line reads ready (main92: 'ready', no
    # actions, where 'working' with Cancel was expected). So the Laya download cases run FIRST, while the decision model is still the on-device one.
    s.click("ai-setup.reset")
    rig_folder(s, "Models", "laya")      # the decision model not on this Mac yet: the first case below downloads it, the second cancels the next one
    s.raw("fixture", "provider-endpoint", "laya", loop.url)
    loop.slow = 0.5          # ten pieces, five seconds: the app's progress tick has pieces to show and Cancel has pieces left to stop
    seen = []
    s.expect("ai-decision-download-shows-progress-and-a-cancel", lambda: (click(s, X + ".ai.sorting.download"), seen.append(waited(s, lambda: sorting(s)[0], "working")), SLEEP(0.6), seen.append(sorting(s)),
                                                                          (seen[0], seen[1][0], round(seen[1][1] or 0, 1) > 0, seen[1][2]))[-1], ("working", "working", True, ["cancel"]))
    s.expect("ai-cancel-stops-the-decision-download", lambda: (click(s, X + ".ai.sorting.cancel"), waited(s, lambda: sorting(s)[0], "missing"), sorting(s)[2])[-1], ["use-jev", "download"])
    loop.slow = 0.0
    s.raw("fixture", "provider-endpoint", "laya", "")
    s.click("ai-setup.reset")
    s.expect("ai-use-jev-asks-the-key-then-the-consent-then-chooses",
             lambda: [(click(s, X + ".ai.sorting." + d), ai_lines(s)["sorting"]["jev"])[-1] for d in ("use-jev", "jev-key", "jev-allow")] + [s.ctl.core("provider_credential/jev/stored"), s.ctl.rows("mac-settings-ai.decisions.jev.consent")[0]["given"],
                                                                                                                     [r["state"] for r in s.ctl.rows("ai-setup.status") if r["need"] == "decision"][0]],
             ["key", "consent", "chosen", True, True, "ready"])
    s.click("ai-setup.reset")
    s.expect("ai-jev-key-door-stores-the-key-and-asks-the-consent", lambda: (click(s, X + ".ai.sorting.use-jev"), click(s, X + ".ai.sorting.jev-key"),
                                                                            (ai_lines(s)["sorting"]["jev"], s.ctl.core("provider_credential/jev/stored")))[-1], ("consent", True))
    s.expect("ai-jev-allow-door-gives-the-consent-and-chooses-jev", lambda: (click(s, X + ".ai.sorting.jev-allow"),
                                                                             (ai_lines(s)["sorting"]["jev"], s.ctl.rows("mac-settings-ai.decisions.jev.consent")[0]["given"]))[-1], ("chosen", True))
    s.click("ai-setup.reset")
    s.expect("ai-meaning-download-starts", lambda: (meaning_missing(s), click(s, X + ".ai.meaning.download"), meaning_row(s)["reason"])[-1] != "meaning-missing", True)
    s.raw("fixture", "embedder", "loaded")
    ai_setup(s, ("meaning", "clear"))
    s.expect("ai-every-control-of-the-step-clicked-once", lambda: sorted(AI_SEEN - AI_CLICKED), [])
    s.expect("ai-continue-is-always-on", lambda: (click(s, X + ".continue"), state(s)["step"])[-1], 8)
    at_ai(s)
    s.raw("fixture", "providers", "none")   # the pasted key above connected a provider in the core and `ai-setup.reset` keeps it: stage the core with none again so the writing mark can be on
    s.click("ai-setup.reset")
    s.expect("ai-skip-leaves-the-marks-on", lambda: (click(s, X + ".skip"), [r["usable"] for r in s.ctl.rows("ai-setup.status") if r["need"] == "writing"][0])[-1], False)


def steps_replies(j):
    # ---- J9.9: "Replies" (the step between the account and the import; ai-replies-r5 FS(...), written through the delta 20 commands) ----
    s = j.step("J9.9", "Replies")
    at_replies(s)
    s.expect("replies-card-words", lambda: pick(card(s), "kicker", "title", "body", "count", "dot_current"), REPLIES + ("Step 8 of 10", 8))
    s.expect("replies-controls-show-their-keys", lambda: controls(s),
             [("back", "Back", "⌘[", True), ("skip", "Skip", "esc", True), ("continue", "Continue", "↩", True)])
    s.expect("replies-lights-the-list-only", lambda: lit(s), (False, False, True, False))
    s.expect("replies-example-word-shows-in-both-themes", lambda: stage_marks(s, "example", [X + ".replies.example"]), {"light": [True], "dark": [True]})
    s.expect("replies-three-rows-with-defaults", lambda: [(c["id"], c["title"], c["caption"], c["words"]) for c in card(s)["choices"]], REPLY_ROWS)
    s.expect("replies-options-per-row", lambda: [[o["id"] for o in c["options"]] for c in s.ctl.rows(RP)],
             [["on-demand", "auto"], ["sent-inbox", "inbox", "sent"], ["100", "300", "1000"]])
    before = []
    s.expect("replies-skip-leaves-the-defaults", lambda: (before.append(reply_core(s)), click_all(s, "draft.auto", "search.inbox", "voice.300"), click(s, X + ".skip"),
                                                          (reply_core(s) == before[0], state(s)["step"]))[-1], (True, 9))
    at_replies(s)
    s.expect("replies-when-i-ask-writes-auto-off", lambda: (click_all(s, "draft.on-demand"), click(s, X + ".continue"), reply_core(s)[0])[-1], False)
    s.expect("replies-continue-goes-to-the-import-step", lambda: state(s)["step"], 9)
    s.key("⌘[")
    s.expect("replies-back-returns-to-the-replies-step", lambda: state(s)["step"], 8)
    s.expect("replies-continue-writes-auto-on", lambda: (click_all(s, "draft.auto"), click(s, X + ".continue"), reply_core(s)[0])[-1], True)
    at_replies(s)
    s.expect("replies-search-choice-reaches-the-folders", lambda: (click_all(s, "search.inbox"), click(s, X + ".continue"), reply_core(s)[1])[-1],
             {"INBOX": [True], "Sent": [False]})
    at_replies(s)
    s.expect("replies-voice-choice-reaches-the-analysis", lambda: (click_all(s, "voice.300"), click(s, X + ".continue"), reply_core(s)[2])[-1], 300)


def click_all(s, *doors):
    for d in doors:
        click(s, RP + "." + d)


def sizes(s):
    g = s.ctl.rows(X + ".geometry")[0]
    return (g["medallion"], g["welcome_card"]["w"], g["welcome_card"]["h"], g["compact"])


def sizes_expected(s):
    w = window_w(s)
    big, compact = w >= 1280, w < const("COMPACT_BELOW_W_PX")
    return (120.0 if big else 108.0, 420.0 if big else 392.0, 280.0 if big else 272.0, compact)


def keys_stay_out(s):
    """E and C at step 1 are the inbox's keys; behind the veil they must change nothing."""
    before = inbox(s)
    s.key("E")
    s.key("C")
    return inbox(s) == before and state(s)["step"] == 1


def lit(s):
    return tuple(r["lit"] for r in s.ctl.rows(X + ".lit"))


def key_focus(s):
    s.key("J")
    a = s.ctl.rows(X + ".keys")[0]["focus"]
    s.key("K")
    return (a, s.ctl.rows(X + ".keys")[0]["focus"])


def frozen(s):
    a = shot(s, "fs-reduce-a")
    SLEEP(2)
    b = shot(s, "fs-reduce-b")
    return (state(s)["frozen"], round(png_diff(a, b), 2))


def moving(s):
    """Not frozen, and the loop's picture changes (more than 5 % of the pixels) between some two of three shots taken at different gaps. One pair 2 s
    apart read 'not moving' on main88 when that gap fell on a whole number of the loop's own beats; three shots at 0.9 s and 1.7 s gaps cannot all
    fall on one. The limit (0.05) is the old one; a loop that never moves still gives three equal pictures and stays red."""
    shots = [shot(s, "fs-move-a")]
    for gap, tag in ((0.9, "b"), (1.7, "c")):
        SLEEP(gap)
        shots.append(shot(s, "fs-move-" + tag))
    return (not state(s)["frozen"]) and max(png_diff(shots[i], shots[j]) for i in range(3) for j in range(i + 1, 3)) > 0.05


ADD_ADDRESS = "assem.fixture@srv.example.test"     # fixtures/addaccount: the recorded server that accepts the fixture password
ADD_PASSWORD = "fixture-password"
N = [0]


def add_account(s):
    """J4.1 to J4.5 inside step 6: the address, the password, Continue, Add. Each click is the add-account lane's own control."""
    N[0] += 1
    s.click(ADD + ".address")
    s.type("u%d.%s" % (N[0], ADD_ADDRESS))
    s.click(ADD + ".password")
    s.type(ADD_PASSWORD)
    s.click(ADD + ".continue")
    s.click(ADD + ".add")


def steps_6_to_8(j):
    # ---- J9.6: "Your first account" (the inline add-account card) ----
    s = j.step("J9.6", "Your first account")
    on(s)
    go_to(s, 6)
    s.expect("step-6-is-the-add-card-with-its-count", lambda: pick(card(s), "step", "sheet", "count"), (6, "add-account", "6 of 10"))
    s.expect("back-skip-continue-keys-at-6", lambda: [c[:3] for c in controls(s)], [("back", "Back", "⌘["), ("skip", "Skip", "esc"), ("continue", "Continue", "↩")])
    s.expect("continue-follows-the-account", lambda: [c[3] for c in controls(s) if c[0] == "continue"] == [state(s)["accounts"] > 0], True)
    s.expect("every-pane-sits-under-the-veil-at-6", lambda: lit(s), (False, False, False, False))
    add_account(s)
    s.expect("an-account-added-moves-the-flow-to-the-ai-step", lambda: waited(s, lambda: state(s)["step"], 7), 7)
    s.click(ADD + ".done")
    on(s)
    go_to(s, 6)
    click(s, X + ".skip")
    s.expect("skip-at-6-opens-the-empty-inbox", lambda: pick(state(s), "step", "skipped", "lifted", "finished"), (6, True, True, True))
    s.expect("empty-inbox-offers-add-account-with-its-key", lambda: controls(s), [("add-account", "Add account", "↩", True)])
    click(s, X + ".add-account")
    s.expect("add-account-opens-the-sheet", lambda: s.ctl.uistate().get("surface"), ADD)
    s.click(ADD + ".cancel")
    s.raw("fixture", "first-start", "off")

    # ---- J9.7: "Import mail" (optional) ----
    s = j.step("J9.7", "Import mail from Apple Mail or Outlook")
    s.raw("fixture", "import-sources", "apple-mail=120")
    on(s)
    go_to(s, 9)
    s.click(IMP + ".reset")
    s.raw("fixture", "import-sources", "apple-mail=120")
    s.click(IMP + ".scan")
    s.expect("the-import-step-is-the-panel-with-finish-and-skip", lambda: ([c[:3] for c in controls(s)], card(s).get("sheet")),
             ([("back", "Back", "⌘["), ("skip", "Skip", "esc"), ("continue", "Finish", "↩")], "import"))
    s.click(IMP + ".select.apple-mail")
    # the source choice says what it holds (board row 7df97540: "the source choice, progress and count")
    s.expect("the-scan-lists-apple-mail-with-its-count", lambda: [(r["kind"], r["count"], r["selected"]) for r in s.ctl.rows(IMP + ".sources")], [("apple-mail", 120, True)])
    s.click(IMP + ".start")
    run = []
    s.expect("the-scan-found-apple-mail-and-start-began-a-run", lambda: (run.append(s.ctl.rows(IMP + ".progress")[0]["run_id"]), len(run))[1], 1)
    progress = lambda: s.ctl.rows(IMP + ".progress")[0]
    s.expect("a-run-says-how-far-it-is", lambda: (lambda p: (p["state"], p["done"], p["total"]))(progress()), ("running", 0, 120))
    s.raw("fixture", "import-progress", run[0] if run else "run-1", "30")
    s.expect("a-step-moves-the-count", lambda: (lambda p: (p["done"], p["total"], round(p["fraction"], 2)))(progress()), (30, 120, 0.25))
    s.raw("fixture", "import-progress", run[0] if run else "run-1", "finish")
    s.expect("a-finished-run-keeps-the-flow-on-the-import-step", lambda: (s.ctl.rows(IMP + ".progress")[0]["state"], state(s)["step"]), ("finished", 9))
    s.expect("a-finished-run-says-what-it-kept", lambda: (lambda p: (p["done"], p["stored"] + p["skipped"] == p["total"], p["stored"] > 0))(progress()), (120, True, True))
    s.click(IMP + ".skip")
    s.expect("the-panels-skip-lifts-the-veil", lambda: waited(s, lambda: state(s)["step"], 10), 10)

    # ---- J9.8: "Your inbox" ----
    s = j.step("J9.8", "Your inbox")
    s.expect("the-veil-lifts-on-the-panes", lambda: (lit(s), pick(state(s), "lifted", "step")), ((True, True, True, True), (True, 10)))
    s.expect("one-ring-crosses-then-clears", lambda: waited(s, lambda: state(s)["ring_showing"], False), False)
    s.expect("the-first-account-is-in-the-inbox", lambda: len(inbox(s)) > 0, True)
    s.expect("finish-remembers-it", lambda: pick(s.ctl.rows(X + ".launch")[0], "finished", "flag"), (True, FLAG))
    click(s, X + ".relaunch")
    s.expect("a-second-launch-shows-nothing", lambda: pick(state(s), "active", "visible"), (False, False))
    on(s)
    go_to(s, 9)
    click(s, X + ".continue")
    s.expect("finish-at-the-import-step-lifts-the-veil-too", lambda: pick(state(s), "step", "lifted"), (10, True))
    on(s)
    click(s, X + ".skip")
    click(s, X + ".back")
    s.expect("back-at-step-6-goes-to-5", lambda: state(s)["step"], 5)
    s.raw("fixture", "first-start", "off")
    s.expect("every-control-seen-was-clicked-once", lambda: sorted(SEEN - CLICKED), [])


QS_DOOR, QS_PALETTE = "mac-settings-general.quick-start.show", "mac-palette.result.show-the-quick-start"


def steps_again(j):
    """J9.11 (owner, 16 Nov 2026: "we need to have the quickstart of whoosh live (I feel like it's not the case yet ?)"): a launch opens the flow only with no
    account, so an installation that has one never saw it. Settings, General (`Show again`) and the palette's line open it again from step 1, the seen flag
    keeps its value, and Skip at the account step goes on to the AI step (there is no empty inbox to open)."""
    s = j.step("J9.11", "The quick start again, on an installation with an account")
    s.raw("fixture", "first-start", "off")
    # the flow was finished above and the account of step 6 exists: the state an owner's installation is in
    s.click(X + ".relaunch")
    s.expect("a-launch-with-an-account-shows-nothing", lambda: (state(s)["active"], state(s)["accounts"] > 0), (False, True))
    seen = pick(s.ctl.rows(X + ".launch")[0], "finished")
    s.click(QS_DOOR)
    s.expect("the-settings-door-shows-the-flow-again", lambda: pick(state(s), "active", "step", "skipped"), (True, 1, False))
    s.expect("the-flow-again-keeps-the-seen-flag", lambda: pick(s.ctl.rows(X + ".launch")[0], "finished") == seen, True)
    go_to(s, 6)
    s.click(X + ".skip")
    s.expect("skip-at-the-account-step-goes-on-when-an-account-exists", lambda: pick(state(s), "step", "skipped", "lifted"), (7, False, False))
    s.raw("fixture", "first-start", "off")
    s.click(X + ".relaunch")      # the launch rule closes what `off` left: the state before the second door is the installation's again
    s.click(QS_PALETTE)
    s.expect("the-palette-line-shows-the-flow-again", lambda: pick(state(s), "active", "step"), (True, 1))
    s.raw("fixture", "first-start", "off")

    # REAL key presses (`keyevent` builds the events the keyboard sends; the socket's `key` calls the app's own press and never meets a focused text).
    # Owner, 16 Nov 2026 on main203: "shortcut don't work on quickstart, I cannot press enter": at step 2 the try field holds the keyboard and kept Return.
    s = j.step("J9.12", "The quick start under real key presses")
    on(s)
    s.raw("keyevent", "Return")
    s.expect("a-real-return-continues-at-step-1", lambda: waited(s, lambda: state(s)["step"], 2), 2)
    s.raw("keyevent", "boat")
    s.expect("a-real-typed-word-lands-in-the-search-try", lambda: waited(s, lambda: s.ctl.rows(X + ".search")[0]["typed"], "boat"), "boat")
    s.raw("keyevent", "Return")
    s.expect("a-real-return-continues-while-the-search-field-holds-the-keyboard", lambda: waited(s, lambda: state(s)["step"], 3), 3)
    # the account step: Return in a field of the Add account card is the card's Continue (the same class: the fields kept the key)
    go_to(s, 4)                                    # from step 3: Continue to 4, 5 and 6
    N[0] += 1
    s.click(ADD + ".address")
    s.type("u%d.%s" % (N[0], ADD_ADDRESS))
    s.click(ADD + ".password")
    s.type(ADD_PASSWORD)
    s.expect("the-account-card-waits-at-entry", lambda: (state(s)["step"], s.ctl.rows(ADD + ".state")[0]["stage"]), (6, "entry"))
    s.raw("keyevent", "Return")
    s.expect("a-real-return-in-the-password-field-is-continue", lambda: waited(s, lambda: s.ctl.rows(ADD + ".state")[0]["stage"] != "entry", True), True)
    s.click(ADD + ".cancel")
    s.raw("fixture", "first-start", "off")
    s.expect("the-door-gives-the-main-window-the-keyboard", door_scan, True)


def door_scan():
    """The door of Settings is in another window: FirstStartModel.replay hands the keyboard to the main window (WindowBridge.giveMainTheKeyboard: forward and
    key). A hidden copy has no key window, so the call is read in the source; the helper itself must make the window key."""
    model, bridge = _src("FirstStartModel.swift"), open(os.path.join(ROOT, "Sources", "App", "WindowBridge.swift"), encoding="utf-8").read()
    body = model.split("func replay()", 1)[1].split("\n    }", 1)[0] if "func replay()" in model else ""
    helper = bridge.split("static func giveMainTheKeyboard()", 1)[1].split("\n    }", 1)[0] if "static func giveMainTheKeyboard()" in bridge else ""
    return "WindowBridge.giveMainTheKeyboard()" in body and "makeKey()" in helper and "orderFrontRegardless()" in helper


def steps_source(j):
    s = j.step("J9.1", "Source: motion, words, languages, the rule")
    s.expect("motion-within-the-budget", motion_scan, (True, True, []))
    s.expect("no-bare-string", bare_strings, [])
    s.expect("keys-in-14-languages", catalogue, (True, [], [], "Bienvenue dans"))
    s.expect("first-launch-rule-is-no-account-and-no-flag", rule_scan, (True, True))
    s.expect("finish-and-skip-at-6-both-set-the-flag", flag_scan, (True, True))
    s.expect("replies-continue-applies-and-skip-does-not", replies_scan, (True, True, True))
    s.expect("accounts-counted-from-the-add-cards-list", count_scan, True)
    s.expect("an-add-ends-accounts-none-in-the-fixture-core", fixture_add_scan, 2)


def motion_scan():
    """Every literal duration of the flow is at most MOTION_MAX_MS; the card, veil and morph read the ledger's row; no step waits on a loop."""
    files = _all_src()
    secs = [float(x) for t in files.values() for x in re.findall(r"duration:\s*([0-9]+\.?[0-9]*)\b", t)]
    ledger = "MOTION_MAX_MS" in files["FirstStartLook.swift"] and "morphS" in files["FirstStartCard.swift"]
    # a loop is content; what answers a key or a click is not allowed to be a loop's length
    slow = [(f, m) for f, t in files.items() for m in re.findall(r"duration:\s*(?:FirstStartLook\.)?(loopS)\b", t)]
    return max(secs or [0]) <= MOTION_MS() / 1000, ledger, slow


def bare_strings():
    bad = []
    for f, t in _all_src().items():
        for m in re.findall(r'\b(?:Text|Button|Label|Picker|TextField|Toggle)\(\s*(?:title:\s*)?"([^"\n]*)"', t):
            if re.search(r"[A-Za-z]{2}", re.sub(r"\\\([^)]*\)", "", m)):
                bad.append((f, m))
        bad += [(f, m) for m in re.findall(r'\.(?:help|navigationTitle|accessibilityLabel)\("([^"]*)"\)', t)]
    return bad


def catalogue():
    cat = json.load(open(XCS, encoding="utf-8"))["strings"]
    pending = json.load(open(REPLY_PENDING, encoding="utf-8"))   # keys the step adds: French and English here until the catalogue lane merges them
    keys = re.findall(r'Words\.text\("((?:[^"\\]|\\.)*)"', _src("FirstStartWords.swift"))
    holes, fmt = [], []
    for k in keys:
        loc = cat.get(k, {}).get("localizations", {})
        if k not in cat and k in pending:
            holes += [(k, l) for l in ("en", "fr") if not pending[k].get(l)]
            fmt += [(k, l) for l in ("en", "fr") if sorted(re.findall(r"%(?:lld|@)", pending[k].get(l, ""))) != sorted(re.findall(r"%(?:lld|@)", k))]
            continue
        holes += [(k, l) for l in LANGS if not loc.get(l, {}).get("stringUnit", {}).get("value")]
        want = sorted(re.findall(r"%(?:lld|@)", k))
        fmt += [(k, l) for l in LANGS if sorted(re.findall(r"%(?:lld|@)", loc.get(l, {}).get("stringUnit", {}).get("value", ""))) != want]
    fr = cat.get("Welcome to", {}).get("localizations", {}).get("fr", {}).get("stringUnit", {}).get("value")
    return (len(keys) >= 40, holes, fmt, fr)


def rule_scan():
    t = _src("FirstStartModel.swift")
    rule = re.search(r"static func shouldShow\(accounts: Int, finished: Bool\) -> Bool \{ ([^}]*) \}", t)
    return (rule is not None and rule.group(1) == "accounts == 0 && !finished", 'finishedKey = "%s"' % FLAG in t)


def count_scan():
    """Step 6 goes on when the add card saved an account even if the core (the fixture under `accounts none`) still lists none."""
    t = _src("FirstStartModel.swift")
    m = re.search(r"func refreshAccounts\(\) \{(.*?)\n    \}", t, re.S)
    return m is not None and "AddAccountModel.shared.added.count" in m.group(1) and "addedAtOpen" in t


def fixture_add_scan():
    """FixtureCore: both add paths clear `noAccounts` right after `added.append`, so `fixture accounts none` ends at the first add (step 6 counts it);
    `fixtureAccounts(none:)` still sets and clears it. Returns how many add paths clear the flag."""
    t = open(FIXCORE, encoding="utf-8").read()
    ok = "func fixtureAccounts(none: Bool)" in t and "noAccounts = none" in t
    return len(re.findall(r"added\.append\([^\n]*\)\n\s*noAccounts = false", t)) if ok else -1


def replies_scan():
    """Continue at the replies step writes the reply choices (RepliesStepModel.apply) and Skip there does not; the flow counts ten steps."""
    t = _src("FirstStartModel.swift")
    nxt = re.search(r"func next\(\) -> Bool \{(.*?)\n    \}", t, re.S)
    skp = re.search(r"func skip\(\) -> Bool \{(.*?)\n    \}", t, re.S)
    return ("stepCount = 10" in t and "aiStep = 7, repliesStep = 8" in t, nxt is not None and "RepliesStepModel.shared.apply()" in nxt.group(1),
            skp is not None and "apply()" not in skp.group(1) and "case Self.repliesStep: go(Self.importStep)" in skp.group(1))


def flag_scan():
    t = _src("FirstStartModel.swift")
    n = len(re.findall(r"UserDefaults\.standard\.set\(true, forKey: Self\.finishedKey\)", t))
    return (n >= 2, "func skipToEmptyInbox" in t)


# MUTANTS: per check, the mutation of the app that must turn it red (the name after the slash is the model's switch).
# J9.1/welcome-words | the welcome card shows other words than the 3a board's
# J9.1/welcome-controls-show-their-keys | a Back appears at step 1, or Continue shows no ↩
# J9.1/welcome-sizes-from-the-board | the medallion is drawn 96 pt instead of 120 (108 at 960)
# J9.1/motion-budget-in-the-state | the card morph takes 260 ms: over MOTION_MAX_MS
# J9.1/only-the-flow-answers-keys | E at step 1 archives a message behind the veil
# J9.1/loop-seam-step1 | the welcome loop's last frame differs from its first
# J9.1/skip-from-step-1-jumps-to-6 | Skip at step 1 goes to step 2
# J9.1/enter-continues | Return does not run Continue
# J9.2/search-card-words | the search card shows other words than the 2b board's
# J9.2/back-continue-skip-show-their-keys | Skip shows no esc
# J9.2/search-lights-the-list-only | the search step lights the reader instead of the list
# J9.2/boat-finds-the-sailing-club-by-meaning | a typed word is ignored: the stage answers nothing
# J9.2/a-word-with-no-meaning-says-so | a word with no meaning leaves the stage silent
# J9.2/loop-seam-step2 | the search loop's last frame differs from its first
# J9.2/reduced-motion-freezes-the-loop | Reduced Motion still moves the loop
# J9.2/loops-run-without-reduced-motion | the loop never moves
# J9.2/back-goes-to-step-1 | Back ⌘[ does nothing
# J9.2/esc-skips-to-6 | Esc at step 2 goes to the next step
# J9.3/keys-card-words | the keys card shows other words than the 2b board's
# J9.3/three-sample-rows-and-the-caps | the try-it holds five rows
# J9.3/j-and-k-move-the-focus | J moves the focus but K does not move it back
# J9.3/e-makes-the-focused-line-leave | E does nothing on the sample rows
# J9.3/h-reminds-and-the-line-leaves | H leaves the row in place
# J9.3/the-try-it-never-touches-the-inbox | E at step 3 also archives a message behind the veil
# J9.3/loop-seam-step3 | the keys loop's last frame differs from its first
# J9.4/labels-card-words | the labels card shows other words than the 2b board's
# J9.4/labels-lights-the-sidebar-and-the-list | the labels step lights only the list
# J9.4/loop-seam-step4 | the labels loop's last frame differs from its first
# J9.5/privacy-card-words | the privacy card shows other words than the 2b board's
# J9.5/the-two-optional-parts-are-off | the Jev key switch is drawn on
# J9.5/privacy-lights-the-reader-only | the privacy step lights the whole window
# J9.5/loop-seam-step5 | the privacy loop's last frame differs from its first
# J9.6/step-6-is-the-add-card-with-its-count | step 6 shows the privacy card again
# J9.6/back-skip-continue-keys-at-6 | Back is missing at step 6
# J9.6/continue-follows-the-account | Continue is on at step 6 with no account
# J9.6/every-pane-sits-under-the-veil-at-6 | step 6 lights the list
# J9.6/an-account-added-moves-the-flow-to-the-ai-step | an added account leaves the flow on step 6
# J9.6/skip-at-6-opens-the-empty-inbox | Skip at step 6 goes on to the import step
# J9.6/empty-inbox-offers-add-account-with-its-key | the empty inbox has no Add account button
# J9.6/add-account-opens-the-sheet | Add account does nothing
# J9.10/ai-card-words | the AI card shows other words than the board's
# J9.10/ai-controls-show-their-keys | Skip shows no esc at the AI step
# J9.10/ai-lights-the-list-only | the AI step lights the reader instead of the list
# J9.10/ai-three-lines-with-their-actions | the Sorting line offers no Use Jev
# J9.10/ai-in-progress-states-as-drawn | a download in progress shows no Cancel
# J9.10/ai-rings-fill-as-each-need-is-met | the stage rings stay empty when a need is met
# J9.10/ai-rings-show-on-the-pane-in-both-themes | the stage rings are drawn white in the light theme too
# J9.10/ai-pasted-key-reaches-the-provider | the key field sends nothing: the writing model stays missing
# J9.10/ai-use-jev-asks-the-key-then-the-consent-then-chooses | Allow does not choose Jev as the decision model
# J9.10/ai-pasted-key-leaves-the-writing-line-connected | the pasted key leaves the writing line checking, never connected
# J9.10/ai-jev-key-door-stores-the-key-and-asks-the-consent | the Jev key door stores no key
# J9.10/ai-jev-allow-door-gives-the-consent-and-chooses-jev | the Jev Allow door gives no consent
# J9.10/ai-decision-download-shows-progress-and-a-cancel | the decision model's Download starts nothing the line can show
# J9.10/ai-cancel-stops-the-decision-download | Cancel leaves the decision download running
# J9.10/ai-every-control-of-the-step-clicked-once | the Meaning line offers a Pause button no case clicks
# J9.10/ai-meaning-download-starts | the meaning Download starts nothing
# J9.10/ai-continue-is-always-on | Continue at the AI step needs a need set
# J9.10/ai-card-hides-the-mail-body-under-it | the reader's body stays drawn under the AI step's card
# J9.10/ai-skip-leaves-the-marks-on | Skip at the AI step marks every need done
# J9.9/replies-card-words | the replies card shows other words than the board's
# J9.9/replies-controls-show-their-keys | Skip shows no esc at the replies step
# J9.9/replies-lights-the-list-only | the replies step lights the reader instead of the list
# J9.9/replies-example-word-shows-in-both-themes | the word EXAMPLE is drawn white in the light theme too
# J9.9/replies-three-rows-with-defaults | the draft row starts on Automatically
# J9.9/replies-options-per-row | an option is missing from a row
# J9.9/replies-skip-leaves-the-defaults | Skip at the replies step writes the choices
# J9.9/replies-when-i-ask-writes-auto-off | When I ask leaves the auto switch as it was
# J9.9/replies-continue-goes-to-the-import-step | Continue at the replies step stays on it
# J9.9/replies-back-returns-to-the-replies-step | Back at the import step does nothing
# J9.9/replies-continue-writes-auto-on | Automatically is chosen and Continue never switches auto on
# J9.9/replies-search-choice-reaches-the-folders | Inbox only is chosen and the folders are not written
# J9.9/replies-voice-choice-reaches-the-analysis | 300 messages is chosen and the analysis keeps 100
# J9.1/replies-continue-applies-and-skip-does-not | Skip at the replies step calls the write
# J9.7/the-import-step-is-the-panel-with-finish-and-skip | Continue at step 8 still reads Continue
# J9.7/the-scan-found-apple-mail-and-start-began-a-run | Start begins no run
# J9.7/the-scan-lists-apple-mail-with-its-count | the found source shows no message count
# J9.7/a-run-says-how-far-it-is | a running import shows no total
# J9.7/a-step-moves-the-count | the count stays at 0 while the run goes on
# J9.7/a-finished-run-says-what-it-kept | a finished run says nothing of what it stored and left out
# J9.7/a-finished-run-keeps-the-flow-on-the-import-step | a finished run ends the flow by itself
# J9.7/the-panels-skip-lifts-the-veil | the panel's Skip leaves the flow on the import step
# J9.8/the-veil-lifts-on-the-panes | step 10 leaves one pane under the veil
# J9.8/one-ring-crosses-then-clears | the ring never clears
# J9.8/the-first-account-is-in-the-inbox | the inbox is empty after the flow
# J9.8/finish-remembers-it | Finish does not set the flag
# J9.8/a-second-launch-shows-nothing | a relaunch after Finish opens the flow again
# J9.8/finish-at-the-import-step-lifts-the-veil-too | Finish at the import step does not reach step 10
# J9.8/back-at-step-6-goes-to-5 | Back at step 6 does nothing
# J9.8/every-control-seen-was-clicked-once | a control is listed that no check clicked
# J9.11/a-launch-with-an-account-shows-nothing | a launch opens the flow on an installation that has an account
# J9.11/the-settings-door-shows-the-flow-again | the Show again button of Settings, General does nothing
# J9.11/the-flow-again-keeps-the-seen-flag | showing the flow again clears the seen flag: the next launch would open it by itself
# J9.11/skip-at-the-account-step-goes-on-when-an-account-exists | Skip at the account step opens the empty inbox although an account exists
# J9.11/the-palette-line-shows-the-flow-again | the palette's line Show the quick start does nothing
# J9.12/a-real-return-continues-at-step-1 | a real Return at step 1 does nothing
# J9.12/a-real-typed-word-lands-in-the-search-try | real letters typed at step 2 do not reach the try field
# J9.12/a-real-return-continues-while-the-search-field-holds-the-keyboard | the try field keeps Return: the step never continues (the field without its onSubmit)
# J9.12/the-account-card-waits-at-entry | a click in the address field already moves the flow past the account step when the installation has accounts (the count compared with 0, not with the count at opening; main204)
# J9.12/a-real-return-in-the-password-field-is-continue | the fields of the Add account card keep Return: Continue is never pressed (the fields without their onSubmit)
# J9.12/the-door-gives-the-main-window-the-keyboard | the door brings the main window forward without making it key: the keys stay in Settings
# J9.1/motion-within-the-budget | a literal 400 ms animation is added to the flow
# J9.1/no-bare-string | a label is a literal that no language file holds
# J9.1/keys-in-14-languages | one key has no Japanese
# J9.1/first-launch-rule-is-no-account-and-no-flag | the rule opens the flow when either holds
# J9.1/an-add-ends-accounts-none-in-the-fixture-core | an added account leaves the fixture core listing none
# J9.1/accounts-counted-from-the-add-cards-list | the flow counts only the core's list: an add leaves step 6
# J9.1/finish-and-skip-at-6-both-set-the-flag | Skip at step 6 does not set the flag


class Model:
    """The rig as the design says it should answer, with the mutant of the check under test switched on (mutant name = check name)."""
    FINISHED = FLAG

    def __init__(self, mut):
        self.m = set(mut)
        self.t, self.surface, self.accounts, self.flag, self.reduce = 0.0, "mac-main", 0, False, False   # the first launch: no account yet
        self.active, self.step, self.skipped, self.lifted, self.start = False, 1, False, False, 0.0
        self.typed, self.ring_until = "", 0.0
        self.shown_again = False        # the flow was opened by its door (Settings, the palette), not by a launch
        self.add_stage = "entry"        # the Add account card: entry until Continue
        self.mail = [("Linea Supply", "Quote for the Q1 print run: 12,000 units"), ("Marie Lefèvre", "Final logo files + brand book v3"), ("Ton Verheyden", "Re: Clarification points, section 4.2")]
        self.keys = None
        self.found, self.sources, self.run, self.imp_skipped = [], [], None, False
        self.addr = ""
        self.rep = {"draft": "on-demand", "search": "sent-inbox", "voice": 100}
        self.ai_prev, self.ai_key, self.jev, self.jev_stored, self.jev_consent, self.ai_dl = {}, False, "none", False, False, False
        self.endpoints, self.laya_run, self.laya_reads = {}, False, 0
        self.ai_local = True    # the fixture core's launch state: "On this Mac" connected; `fixture providers none` removes it, `default` brings it back
        self.use, self.folders, self.sample = True, {"INBOX": True, "Sent": True}, 100   # the fixture core: auto on, both default folders on, 100 sent
        self.tmp = tempfile.mkdtemp(prefix="par-fs-model-")
        self.shots = 0
        self.theme = "light"

    def on(self, name):
        return name in self.m

    # ---- the flow ----
    def go(self, n):
        self.step, self.start = n, self.t
        if n == 2: self.typed = ""
        if n == 3: self.keys = {"rows": list(SAMPLE), "focus": 0, "last": "", "last_row": "", "started": False}

    def reset_flow(self, open_):
        self.active, self.step, self.skipped, self.lifted, self.start, self.typed = open_, 1, False, False, self.t, ""
        self.shown_again = False
        if open_: self.flag = False
        self.rep = {"draft": "on-demand", "search": "sent-inbox", "voice": 100}

    def ai_need(self, n):
        st = self.ai_prev.get(n)
        if st is None and n == "writing" and self.ai_local: st = "ready"
        if st is None and n == "writing" and self.ai_key and self.on("ai-pasted-key-leaves-the-writing-line-connected"): st = "working"
        if st is None and n == "writing" and self.ai_key and not self.on("ai-pasted-key-reaches-the-provider"): st = "ready"
        if st is None and n == "decision" and self.jev == "chosen" and not self.on("ai-use-jev-asks-the-key-then-the-consent-then-chooses"): st = "ready"
        if st is None and n == "decision" and self.laya_run: st = "working"; self.laya_reads += 1
        if st is None and n == "meaning" and self.ai_dl and not self.on("ai-meaning-download-starts"): st = "working"
        st = st or "missing"
        reason = {"writing": "provider-none", "decision": "laya-missing", "meaning": "meaning-missing"}[n] if st == "missing" else ("ok" if st == "ready" else "x-working")
        if n == "writing" and st == "working": reason = "provider-checking"
        return {"need": n, "state": st, "reason": reason, "usable": st == "ready", "words": "Downloading 312 MB of 843 MB" if st == "working" else "w",
                "detail": "Anthropic · 6 models" if (n == "writing" and st == "ready") else "", "progress": (0.1 * self.laya_reads if self.laya_run else 312 / 843) if st == "working" else None}

    def ai_actions(self, n):
        x = self.ai_need(n)
        if n == "decision":
            if x["state"] == "ready": return []
            if x["state"] == "working": return [] if self.on("ai-in-progress-states-as-drawn") else ["cancel"]
            return {"key": ["jev-key"], "consent": ["jev-allow"]}.get(self.jev, ["use-jev", "download"] if not self.on("ai-three-lines-with-their-actions") else ["download"])
        if n == "writing": return [] if x["state"] == "ready" else ["key"]
        return [] if x["state"] in ("ready", "working") else ["download"] + (["pause"] if self.on("ai-every-control-of-the-step-clicked-once") else [])

    def apply_replies(self):
        """Continue's three writes (SetReplyUseEnabled, SetReplySourceEnabled for the Inbox and Sent folders, RunVoiceAnalysis)."""
        r = self.rep
        if self.on("replies-continue-writes-auto-on") and r["draft"] == "auto": return
        if not (self.on("replies-when-i-ask-writes-auto-off") and r["draft"] == "on-demand"): self.use = r["draft"] == "auto"
        if not self.on("replies-search-choice-reaches-the-folders"):
            self.folders["INBOX"], self.folders["Sent"] = r["search"] != "sent", r["search"] != "inbox"
        if not self.on("replies-voice-choice-reaches-the-analysis"): self.sample = r["voice"]

    def finish(self):
        if self.step == 9: self.imp_skipped = True
        self.go(10); self.lifted = True; self.active = False; self.ring_until = self.t + 0.6
        if not self.on("finish-remembers-it"): self.flag = True

    def next(self):
        if not self.active or self.skipped: return False
        if self.step <= 5: self.go(self.step + 1); return True
        if self.step == 6:
            if self.accounts == 0 and not self.on("continue-follows-the-account"): return False
            self.go(7); return True
        if self.step == 7:
            if self.on("ai-continue-is-always-on"): return False
            self.go(8); return True
        if self.step == 8:
            self.apply_replies()
            if not self.on("replies-continue-goes-to-the-import-step"): self.go(9)
            return True
        if self.step == 9:
            if self.on("finish-at-the-import-step-lifts-the-veil-too"): return False
            self.finish(); return True
        return False

    def back(self):
        if not self.active or self.skipped or self.step <= 1 or self.step > 9: return False
        if self.on("replies-back-returns-to-the-replies-step") and self.step == 9: return False
        if self.on("back-goes-to-step-1") and self.step == 2: return False
        if self.on("back-at-step-6-goes-to-5") and self.step == 6: return False
        self.go(self.step - 1); return True

    def skip(self):
        if not self.active or self.skipped: return False
        if self.step <= 5:
            self.go(2 if self.on("skip-from-step-1-jumps-to-6") and self.step == 1 else 6)
            return True
        if self.step == 6:
            if self.on("skip-at-6-opens-the-empty-inbox"): self.go(7); return True
            if self.shown_again and self.accounts > 0 and not self.on("skip-at-the-account-step-goes-on-when-an-account-exists"): self.go(7); return True
            self.skipped = self.lifted = True
            self.flag = True
            return True
        if self.step == 7:
            if self.on("ai-skip-leaves-the-marks-on"): self.ai_prev = {n: "ready" for n in ("decision", "writing", "meaning")}
            self.go(8); return True
        if self.step == 8:
            if self.on("replies-skip-leaves-the-defaults"): self.apply_replies()
            self.go(9); return True
        if self.step == 9: self.finish(); return True
        return False

    def swallow(self, k):
        if self.active and self.step == 3 and k in "JKEH": self.try_key(k)
        elif self.active and k == "E" and self.on("only-the-flow-answers-keys") and self.mail: self.mail = self.mail[1:]
        return self.active

    def try_key(self, k):
        d = self.keys
        d["started"] = True
        n = len(d["rows"])
        if k == "J": d["focus"] = min(n - 1, d["focus"] + 1)
        elif k == "K" and not self.on("j-and-k-move-the-focus"): d["focus"] = max(0, d["focus"] - 1)
        elif k == "K": pass
        elif k in "EH" and n:
            if k == "E" and self.on("e-makes-the-focused-line-leave"): return
            if k == "H" and self.on("h-reminds-and-the-line-leaves"): d["last"] = "remind"; return
            d["last_row"] = d["rows"].pop(d["focus"]); d["last"] = "done" if k == "E" else "remind"
            d["focus"] = min(d["focus"], max(0, len(d["rows"]) - 1))
            if k == "E" and self.on("the-try-it-never-touches-the-inbox") and self.mail: self.mail = self.mail[1:]

    def add_account(self):
        self.accounts += 1
        self.surface = ADD
        if self.active and self.step == 6 and not self.on("an-account-added-moves-the-flow-to-the-ai-step"):
            self.go(7)

    # ---- the verbs ----
    def handle(self, verb, a):
        ok = {"ok": True}
        if verb == "fixture" and a[:1] == ["first-start"]:
            if a[1] == "on": self.reset_flow(True)
            elif a[1] == "off": self.reset_flow(False); self.ring_until = 0
            elif a[1] == "reduce": self.reduce = a[2] == "on"
            return ok
        if verb == "fixture" and a[:1] == ["import-sources"]:
            self.found = a[1:]; return ok
        if verb == "fixture" and a[:1] == ["import-progress"]:
            step = a[2] if len(a) > 2 else "finish"
            if self.run and step.isdigit():        # a step is a number of messages
                if not self.on("a-step-moves-the-count"):
                    self.run["done"] = min(self.run["total"], self.run["done"] + int(step))
                    self.run["fraction"] = self.run["done"] / self.run["total"] if self.run["total"] else 0
                return ok
            if self.run:
                self.run.update(state="finished", done=120, fraction=1)
                if not self.on("a-finished-run-says-what-it-kept"): self.run.update(stored=115, skipped=5)
                if self.on("a-finished-run-keeps-the-flow-on-the-import-step"): self.finish()
            return ok
        if verb == "fixture" and a[:1] == ["providers"]:
            if a[1:] not in (["none"], ["default"]): return {"ok": False, "error": "usage: fixture providers none|default"}
            self.ai_local = a[1] == "default"
            if a[1] == "none": self.ai_key = False      # the core forgets every key and check (FixtureAgents.stageProviders)
            return ok
        if verb == "fixture" and a[:1] == ["embedder"]:
            return ok
        if verb == "fixture" and a[:1] == ["provider-endpoint"]:
            self.endpoints[a[1]] = a[2]
            return ok
        if verb == "fixture" and a[:1] == ["accounts"]:
            if a[1:] not in (["none"], ["all"]): return {"ok": False, "error": "fixture accounts takes none or all"}
            self.accounts = 0 if a[1] == "none" else 1
            return {"ok": True, "accounts": self.accounts}
        if verb == "click" and a[0].startswith("ai-setup."):
            k = a[0][len("ai-setup."):]
            if k == "reset": self.ai_prev, self.jev, self.jev_stored, self.jev_consent, self.ai_dl = {}, "none", False, False, False      # a connected provider is the core's state: reset (AISetupStatus.reset) does not disconnect it
            elif k.startswith("preview."):
                _, need, st = k.split(".")
                if st == "clear": self.ai_prev.pop(need, None)
                else: self.ai_prev[need] = st
            return ok
        if verb == "rows" and a[0] == "mac-main.reader.body": return ok_rows([{"format": "html"}])
        if verb == "theme": self.theme = "light" if a[0] == "system" else a[0]; return {"ok": True, "theme": self.theme}
        if verb == "capture" and a[:1] == ["shot"] and len(a) > 2 and all(x.endswith(".mark") or x.endswith(".replies.example") for x in a[2:]): return self.mark_shot(a[1], a[2:])
        if verb == "capture" and a[:1] == ["shot"] and len(a) > 2:
            r = self.shot(a[1]); r["frames"] = {x: {"x": 300.0 if x.endswith("reader.body") else 380.0, "y": 100.0, "w": 400.0, "h": 300.0} for x in a[2:]}; return r
        if verb == "measure": return {"ok": True, "frame": {"x": 300.0 if a[0].endswith("reader.body") else 380.0, "y": 100.0, "w": 400.0, "h": 300.0}}
        if verb == "rows" and a[0] == "ai-setup.status": return ok_rows([self.ai_need(n) for n in ("writing", "decision", "meaning")])
        if verb == "core" and a[:1] == ["state"] and a[1] == "provider_credential/jev/stored": return {"ok": True, "value": self.jev_stored}
        if verb == "rows" and a[0] == "mac-settings-ai.decisions.jev.consent": return ok_rows([{"given": self.jev_consent}])
        if verb == "key": return self.key(a[0])
        if verb == "keyevent":       # a real key: at step 2 the try field holds the keyboard, so letters land in it and Return is the field's (its onSubmit continues)
            chord = a[-1]
            if chord == "reset": return ok
            if chord in ("Return", "Enter"):
                if self.active and self.step == 6 and self.surface == ADD:      # a field of the card holds the keyboard: Return is the card's Continue
                    if not self.on("a-real-return-in-the-password-field-is-continue"): self.add_stage = "summary"
                    return ok
                if self.active and self.step == 1 and self.on("a-real-return-continues-at-step-1"): return ok
                if self.active and self.step == 2 and self.on("a-real-return-continues-while-the-search-field-holds-the-keyboard"): return ok
                if self.active: self.next()
                return ok
            if self.active and self.step == 2 and not self.on("a-real-typed-word-lands-in-the-search-try"): self.typed += chord
            return ok
        if verb == "type":
            if self.active and self.step == 2 and not self.on("boat-finds-the-sailing-club-by-meaning"): self.typed += a[0]
            elif self.surface == ADD: self.addr += a[0]
            return ok
        if verb == "click": return self.click(a[0])
        if verb == "core" and a[:1] == ["state"]:
            path = a[1]
            if path == "reply/use/auto": return {"ok": True, "value": {"kind": "auto", "enabled": self.use}}
            if path == "reply/voice": return {"ok": True, "value": {"voice": {"sample_size": self.sample}, "analysis": {"state": "idle"}}}
            if path == "reply/sources":
                return {"ok": True, "value": {"accounts": [{"account_id": "acc-1", "folders": [{"source": {"kind": "mail_folder", "account_id": "acc-1", "folder": n}, "enabled": v}
                                                                                              for n, v in self.folders.items()]}]}}
            return {"ok": False, "error": "no core value at %s" % path}
        if verb == "uistate": return {"ok": True, "state": {"surface": self.surface}}
        if verb == "rows": return self.rows(a[0])
        if verb == "capture": return self.shot(a[1])
        return {"ok": False, "error": "model: %s not served" % verb}

    def key(self, c):
        if c == "Enter": r = False if self.on("enter-continues") and self.step == 1 else self.next()
        elif c == "Escape":
            if self.on("esc-skips-to-6") and self.step == 2: self.go(3); return ok_or(True)
            r = self.skip()
        elif c == "⌘[": r = self.back()
        elif len(c) == 1 and c.upper() in "JKEHC": r = self.swallow(c.upper())
        else: r = False
        return ok_or(r)

    def again(self):
        """The flow from step 1 with the seen flag as it was (FirstStartModel.replay)."""
        self.active, self.step, self.skipped, self.lifted, self.start, self.typed = True, 1, False, False, self.t, ""
        self.shown_again = True
        if self.on("the-flow-again-keeps-the-seen-flag"): self.flag = not self.flag

    def click(self, ax):
        if ax in (QS_DOOR, QS_PALETTE):
            if not self.on("the-settings-door-shows-the-flow-again" if ax == QS_DOOR else "the-palette-line-shows-the-flow-again"): self.again()
            return {"ok": True}
        if ax.startswith(X + "."):
            k = ax[len(X) + 1:]
            if k == "continue": return ok_or(self.next() and not (self.on("welcome-controls-show-their-keys") and False))
            if k == "back": return ok_or(self.back())
            if k == "skip": return ok_or(self.skip())
            if k == "add-account" and self.skipped:
                if self.on("add-account-opens-the-sheet"): return ok_or(False)
                self.surface = ADD; return ok
            if k == "relaunch":
                if self.on("a-second-launch-shows-nothing") and self.flag: self.reset_flow(True)
                elif self.on("a-launch-with-an-account-shows-nothing") and self.accounts > 0: self.again()
                elif not (self.accounts == 0 and not self.flag): self.active = False
                return {"ok": True}
        if ax.startswith(X + ".ai."):
            if not (self.active and self.step == 7): return {"ok": False, "error": "did nothing"}
            d = ax[len(X) + 4:]
            if d == "writing.key" and not self.on("ai-pasted-key-reaches-the-provider"):
                self.ai_key = True
                if self.endpoints.get("anthropic"):      # the app posts the pasted key to the endpoint it was told (a real request to the check's loopback server)
                    import urllib.request
                    urllib.request.urlopen(urllib.request.Request(self.endpoints["anthropic"] + "/validate", data=b"{}", headers={"Authorization": "Bearer x"}), timeout=5).read()
            elif d == "sorting.use-jev": self.jev = "key"
            elif d == "sorting.jev-key" and self.jev == "key": self.jev, self.jev_stored = "consent", not self.on("ai-jev-key-door-stores-the-key-and-asks-the-consent")
            elif d == "sorting.download" and self.endpoints.get("laya") and not self.on("ai-decision-download-shows-progress-and-a-cancel"): self.laya_run, self.laya_reads = True, 0
            elif d == "sorting.cancel" and not self.on("ai-cancel-stops-the-decision-download"): self.laya_run = False
            elif d == "sorting.jev-allow" and self.jev == "consent":
                self.jev_consent = not self.on("ai-jev-allow-door-gives-the-consent-and-chooses-jev")
                if not self.on("ai-use-jev-asks-the-key-then-the-consent-then-chooses"): self.jev = "chosen"
                else: self.jev = "chosen"
            elif d == "meaning.download": self.ai_dl = True
            return ok
        if ax.startswith(RP + "."):
            if not (self.active and self.step == 8): return {"ok": False, "error": "did nothing"}
            row, _, opt = ax[len(RP) + 1:].partition(".")
            self.rep[row] = int(opt) if row == "voice" else opt
            return ok
        if ax == ADD + ".address" or ax == ADD + ".password" or ax == ADD + ".continue":
            self.surface = ADD
            if ax.endswith(".continue"): self.add_stage = "summary"
            if self.on("the-account-card-waits-at-entry") and ax.endswith(".address") and self.active and self.step == 6: self.go(7)
            return {"ok": True}
        if ax == ADD + ".add": self.add_account(); self.add_stage = "entry"; return ok
        if ax in (ADD + ".done", ADD + ".cancel"): self.surface = "mac-main"; self.add_stage = "entry"; return ok
        if ax == IMP + ".reset": self.sources, self.run = [], None; return ok
        if ax == IMP + ".scan": self.sources = [("apple-mail", 120)] if self.found else []; return ok
        if ax == IMP + ".select.apple-mail": return ok
        if ax == IMP + ".start":
            if not self.on("the-scan-found-apple-mail-and-start-began-a-run"):
                self.run = {"run_id": "run-1", "state": "running", "done": 0, "total": 0 if self.on("a-run-says-how-far-it-is") else 120, "fraction": 0, "stored": 0, "skipped": 0}
            return ok
        if ax == IMP + ".skip":
            self.imp_skipped = True
            if self.active and self.step == 9 and not self.on("the-panels-skip-lifts-the-veil"): self.finish()
            return ok
        return {"ok": False, "error": "model: nothing at %s" % ax}

    def frozen(self):
        return self.reduce and not self.on("reduced-motion-freezes-the-loop")

    def card(self):
        if not self.active or self.skipped: return []
        n = self.step
        if n == 7:
            body = "Welcome back." if self.on("ai-card-words") else AI_CARD[2]
            return [{"step": 7, "kicker": AI_CARD[0], "title": AI_CARD[1], "body": body, "count": "Step 7 of 10", "dot_current": 7,
                     "rings": [self.ai_need(n)["usable"] and not self.on("ai-rings-fill-as-each-need-is-met") for n in ("writing", "decision", "meaning")]}]      # the app lists the needs in AINeed order (AISetupStatus.swift:8), not stage order
        if n == 8:
            body = "Welcome back." if self.on("replies-card-words") else REPLIES[2]
            ch = [{"id": "draft", "title": "Write a draft", "caption": "", "words": "Automatically" if self.on("replies-three-rows-with-defaults") else "When I ask"},
                  {"id": "search", "title": "Search in", "caption": "Indexed on this Mac. Nothing leaves it.", "words": "Sent and Inbox"},
                  {"id": "voice", "title": "Write like", "caption": "An analysis of the messages you sent.", "words": "My last 100 messages"}]
            return [{"step": 8, "kicker": REPLIES[0], "title": REPLIES[1], "body": body, "count": "Step 8 of 10", "dot_current": 8, "choices": ch}]
        if n > 5:
            return [{"step": n, "sheet": "add-account" if n == 6 else "import", "count": "%d of 10" % n,
                     **({"sheet": "privacy"} if self.on("step-6-is-the-add-card-with-its-count") and n == 6 else {})}]
        copy = {1: WELCOME, 2: SEARCH, 3: KEYS, 4: LABELS, 5: PRIVACY}[n]
        title_words = copy[1]
        body = copy[2]
        for name, step in (("welcome-words", 1), ("search-card-words", 2), ("keys-card-words", 3), ("labels-card-words", 4), ("privacy-card-words", 5)):
            if self.on(name) and n == step: body = "Welcome back."
        row = {"step": n, "kicker": copy[0], "title": title_words, "body": body, "count": "Step %d of 10" % n, "dot_current": n}
        if n == 5:
            row["optional"] = [{"name": "Jev key", "on": self.on("the-two-optional-parts-are-off")}, {"name": "Push relay", "on": False}]
            row["never"] = NEVER
        return [row]

    def controls(self):
        if not self.active: return []
        if self.skipped:
            return [] if self.on("empty-inbox-offers-add-account-with-its-key") else [{"id": "add-account", "title": "Add account", "key": "↩", "enabled": True}]
        out = []
        if (self.step > 1 or self.on("welcome-controls-show-their-keys")) and self.step <= 9:
            out.append({"id": "back", "title": "Back", "key": "⌘[", "enabled": True})
        if self.on("back-skip-continue-keys-at-6") and self.step == 6: out = []
        out.append({"id": "skip", "title": "Skip", "key": "" if (self.on("back-continue-skip-show-their-keys") and self.step == 2) or (self.on("replies-controls-show-their-keys") and self.step == 8) or (self.on("ai-controls-show-their-keys") and self.step == 7) else "esc", "enabled": True})
        title = "Finish" if self.step == 9 and not self.on("the-import-step-is-the-panel-with-finish-and-skip") else "Continue"
        out.append({"id": "continue", "title": title, "key": "↩", "enabled": self.step != 6 or self.accounts > 0 or self.on("continue-follows-the-account")})
        if self.on("every-control-seen-was-clicked-once"): out.append({"id": "help", "title": "Help", "key": "", "enabled": True})
        return out

    def lit(self):
        if self.lifted or not self.active:
            names = (True, True, True, True)
            return names if not (self.on("the-veil-lifts-on-the-panes") and self.step == 10) else (True, True, True, False)
        table = {7: (False, False, True, False), 2: (False, False, True, False), 3: (False, False, True, False), 4: (False, True, True, False), 5: (False, False, False, True), 7: (False, False, True, False), 8: (False, False, True, False)}
        t = table.get(self.step, (False, False, False, False))
        if self.on("search-lights-the-list-only") and self.step == 2: t = (False, False, False, True)
        if self.on("labels-lights-the-sidebar-and-the-list") and self.step == 4: t = (False, False, True, False)
        if self.on("privacy-lights-the-reader-only") and self.step == 5: t = (True, True, True, True)
        if self.on("every-pane-sits-under-the-veil-at-6") and self.step == 6: t = (False, False, True, False)
        if self.on("replies-lights-the-list-only") and self.step == 8: t = (False, False, False, True)
        if self.on("ai-lights-the-list-only") and self.step == 7: t = (False, False, False, True)
        return t

    def rows(self, ax):
        if ax == X + ".state":
            medal = 96 if self.on("welcome-sizes-from-the-board") else 120
            morph = 260 if self.on("motion-budget-in-the-state") else const("MOTION_MAX_MS")
            ring = self.t < self.ring_until or self.on("one-ring-crosses-then-clears") and self.step == 10
            return ok_rows([{"active": self.active, "visible": self.active or self.t < self.ring_until, "step": self.step, "skipped": self.skipped, "lifted": self.lifted,
                             "finished": self.flag, "accounts": self.accounts, "frozen": self.frozen(), "loop_s": 8, "morph_ms": morph,
                             "veil_ms": const("MOTION_MAX_MS"), "entrance_ms": 550, "ring_showing": ring}])
        if ax == ADD + ".state": return ok_rows([{"ax": ax, "stage": self.add_stage}])
        if ax == X + ".card": return ok_rows(self.card())
        if ax == X + ".controls": return ok_rows(self.controls())
        if ax == X + ".lit": return ok_rows([{"pane": p, "lit": v} for p, v in zip(("toolbar", "sidebar", "list", "reader"), self.lit())])
        if ax == X + ".geometry":
            return ok_rows([{"window": {"w": 1280, "h": 820}, "compact": False, "medallion": 96.0 if self.on("welcome-sizes-from-the-board") else 120.0, "welcome_card": {"w": 420.0, "h": 280.0}}])
        if ax == X + ".search":
            t = self.typed
            hit = "Sailing club" if re.search("boat|sail", t) else ""
            why = "Found by meaning: %s ≈ %s" % (t, hit) if hit else ("No sample line matches “%s”. Try boat, invoice, logo or race." % t if len(t) >= 2 and not self.on("a-word-with-no-meaning-says-so") else "")
            return ok_rows([{"typed": t, "match": hit, "why": why}])
        if ax == X + ".keys":
            d = self.keys or {"rows": list(SAMPLE), "focus": 0, "last": "", "last_row": ""}
            rows = d["rows"] + (["Nina Okafor", "Hetzner-style host"] if self.on("three-sample-rows-and-the-caps") else [])
            return ok_rows([{"rows": rows, "focus": d["focus"], "last": d["last"], "last_row": d["last_row"], "caps": ["E", "H", "C", "/", "J", "K"]}])
        if ax == X + ".ai":
            if not (self.active and self.step == 7): return ok_rows([])
            return ok_rows([dict(self.ai_need(n), id=i, actions=self.ai_actions(n), jev=self.jev) for n, i in (("decision", "sorting"), ("writing", "writing"), ("meaning", "meaning"))])
        if ax == X + ".replies":
            if not (self.active and self.step == 8): return ok_rows([])
            opts = {"draft": ["on-demand", "auto"], "search": ["sent-inbox", "inbox"] + ([] if self.on("replies-options-per-row") else ["sent"]), "voice": ["100", "300", "1000"]}
            return ok_rows([{"id": k, "chosen": str(v), "options": [{"id": o} for o in opts[k]]} for k, v in self.rep.items()])
        if ax == X + ".launch": return ok_rows([{"finished": self.flag, "flag": FLAG, "accounts": self.accounts, "would_show": self.accounts == 0 and not self.flag}])
        if ax == "mac-main.list": return ok_rows([] if self.on("the-first-account-is-in-the-inbox") and self.step == 10 else [{"sender": a, "subject": b} for a, b in self.mail])
        if ax == IMP + ".progress":
            return ok_rows([dict(self.run)] if self.run else [])
        if ax == IMP + ".sources":
            return ok_rows([{"kind": k, "count": 0 if self.on("the-scan-lists-apple-mail-with-its-count") else n, "selected": True} for k, n in self.sources])
        return {"ok": False, "error": "model: no rows at %s" % ax}

    def mark_shot(self, name, ids):
        """The stage's marks on the pane, in the theme's colours: the pane with its dots, each mark an 18 pt square outline in the text colour at 55 %. The two
        broken copies draw the marks white in both themes, as the app did before."""
        from PIL import Image
        dark = self.theme == "dark"
        pane, dot = ((14, 19, 37), (40, 60, 150)) if dark else ((240, 243, 253), (205, 215, 250))
        white = (self.on("ai-rings-show-on-the-pane-in-both-themes") and ids[0].endswith(".mark")) or (self.on("replies-example-word-shows-in-both-themes") and ids[0].endswith(".example"))
        ink = (242, 242, 245) if dark or white else (29, 29, 31)
        mark = tuple(int(round(0.55 * ink[i] + 0.45 * pane[i])) for i in range(3))
        im = Image.new("RGB", (40 * len(ids) + 20, 40), pane)
        for x in range(0, im.size[0], 4):
            for y in range(0, im.size[1], 4): im.putpixel((x, y), dot)
        frames = {}
        for k, ax in enumerate(ids):
            x0, y0 = 10 + 40 * k, 10
            for i in range(18):
                for d in (0, 1, 16, 17):
                    im.putpixel((x0 + i, y0 + d), mark); im.putpixel((x0 + d, y0 + i), mark)
            frames[ax] = {"x": float(x0), "y": float(y0), "w": 18.0, "h": 18.0}
        path = os.path.join(self.tmp, "%s.png" % name)
        im.save(path)
        return {"ok": True, "path": path, "source": "render", "frames": frames, "mapping": {"x": 0, "y": 0, "scale": 1}}

    def shot(self, name):
        from PIL import Image
        self.shots += 1
        phase = 2.0 if self.frozen() else ((self.t - self.start) % (8.5 if self.on("loop-seam-jump") or self.on("loop-seam-step%d" % self.step) else 8))
        if self.on("loops-run-without-reduced-motion"): phase = 2.0
        im = Image.new("RGB", (100, 100), (250, 250, 252))
        x = int(phase * 10)
        for i in range(x, min(x + 10, 100)):
            for k in range(40, 50): im.putpixel((i, k), (20, 40, 90))
        path = os.path.join(self.tmp, "%s.png" % name)
        im.save(path)
        self.t += SHOT_LAT                                   # the capture is taken at the call; the reply comes back later
        return {"ok": True, "path": path, "source": "render" if self.on("ai-card-hides-the-mail-body-under-it") else "render (hidden rig; web pictures clipped around 1 cover(s))"}


ok = {"ok": True}


def ok_or(r):
    return {"ok": True} if r else {"ok": False, "error": "did nothing"}


def ok_rows(rows):
    return {"ok": True, "rows": rows}


def model_run(mut):
    global SRC, XCS, FIXCORE
    realsrc, realxcs, realfix = SRC, XCS, FIXCORE
    scratch = None
    src_mut = {"motion-within-the-budget", "no-bare-string", "keys-in-14-languages", "first-launch-rule-is-no-account-and-no-flag", "finish-and-skip-at-6-both-set-the-flag", "the-door-gives-the-main-window-the-keyboard", "accounts-counted-from-the-add-cards-list", "an-add-ends-accounts-none-in-the-fixture-core", "replies-continue-applies-and-skip-does-not"} & set(mut)
    if src_mut:
        scratch = tempfile.mkdtemp(prefix="par-fs-src-")
        SRC = os.path.join(scratch, "FirstStart"); shutil.copytree(realsrc, SRC)
        XCS = os.path.join(scratch, "Localizable.xcstrings"); shutil.copy(realxcs, XCS)
        FIXCORE = os.path.join(scratch, "FixtureCore.swift"); shutil.copy(realfix, FIXCORE)
        edit_source(src_mut)
    model = Model(mut)
    d = tempfile.mkdtemp(prefix="par-fs-")

    class H(socketserver.StreamRequestHandler):
        def handle(self):
            for line in self.rfile:
                q = json.loads(line.decode())
                self.wfile.write((json.dumps(model.handle(q["verb"], q["args"])) + "\n").encode())
    srv = socketserver.ThreadingUnixStreamServer(os.path.join(d, "whoosh.sock"), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    global SLEEP, NOW
    realsleep, realnow = SLEEP, NOW
    SLEEP = lambda sec: setattr(model, "t", model.t + sec)
    NOW = lambda: model.t
    CLICKED.clear(); SEEN.clear()
    ctl = None
    try:
        ctl = Ctl(d).connect()
        j = Journey(ID, ctl)
        drive(j)
        return {c.label: (c.state, c.detail) for c in j.checks()}
    finally:
        if ctl is not None:
            ctl.close()          # on every path: an open connection keeps its handler thread alive and server_close() joins it for ever
        SLEEP, NOW, SRC, XCS, FIXCORE = realsleep, realnow, realsrc, realxcs, realfix
        srv.shutdown(); srv.server_close()
        shutil.rmtree(d, ignore_errors=True); shutil.rmtree(model.tmp, ignore_errors=True)
        if scratch: shutil.rmtree(scratch, ignore_errors=True)


def edit_source(which):
    def sub(name, fn):
        p = os.path.join(SRC, name)
        text = open(p, encoding="utf-8").read()      # read first: opening for write truncates
        open(p, "w", encoding="utf-8").write(fn(text))
    if "motion-within-the-budget" in which: sub("FirstStartRoot.swift", lambda t: t + "\n.animation(.easeOut(duration: 0.4), value: x)\n")
    if "no-bare-string" in which: sub("FirstStartCard.swift", lambda t: t + '\nlet bare = Text("Hello there")\n')
    if "keys-in-14-languages" in which:
        d = json.load(open(XCS, encoding="utf-8")); d["strings"]["Skip"]["localizations"].pop("ja")
        open(XCS, "w", encoding="utf-8").write(json.dumps(d, ensure_ascii=False))
    if "first-launch-rule-is-no-account-and-no-flag" in which: sub("FirstStartModel.swift", lambda t: t.replace("accounts == 0 && !finished", "accounts == 0 || !finished"))
    if "an-add-ends-accounts-none-in-the-fixture-core" in which:
        t = open(FIXCORE, encoding="utf-8").read().replace("                noAccounts = false   // same: the configured add path\n", "")
        open(FIXCORE, "w", encoding="utf-8").write(t)
    if "replies-continue-applies-and-skip-does-not" in which:
        sub("FirstStartModel.swift", lambda t: t.replace("case Self.repliesStep: go(Self.importStep); return true", "case Self.repliesStep: RepliesStepModel.shared.apply(); go(Self.importStep); return true"))
    if "accounts-counted-from-the-add-cards-list" in which: sub("FirstStartModel.swift", lambda t: t.replace("AddAccountModel.shared.added.count", "0"))
    if "the-door-gives-the-main-window-the-keyboard" in which: sub("FirstStartModel.swift", lambda t: t.replace("WindowBridge.giveMainTheKeyboard()", "WindowBridge.bringMainToFront()"))
    if "finish-and-skip-at-6-both-set-the-flag" in which:
        sub("FirstStartModel.swift", lambda t: t.replace("        skipped = true; lifted = true\n        UserDefaults.standard.set(true, forKey: Self.finishedKey)\n", "        skipped = true; lifted = true\n"))


def selftest():
    clean = model_run(set())
    bad = 0
    for label, (state_, why) in clean.items():
        if state_ != "pass":
            print("RED on the model: %s %s" % (label, why)); bad += 1
    print_model_cases(clean)
    print("model, no mutant: %d of %d checks green" % (sum(1 for st, _ in clean.values() if st == "pass"), len(clean)))
    declared = {}
    for line in open(os.path.abspath(__file__), encoding="utf-8"):
        if re.match(r"# J9\.\d+/", line) and " | " in line:
            k, v = line[2:].split(" | ", 1)
            declared[k.strip()] = v.strip()
    for label in clean:
        if label not in declared:
            print("NO MUTANT %s" % label); bad += 1
            continue
        name = label.split("/", 1)[1]
        got = model_run({name})
        st = got[label][0]
        others = sorted(k.split("/", 1)[1] for k, (s_, _) in got.items() if s_ == "fail" and k != label)
        print("mutant %-52s -> %s%s" % (name, "RED" if st == "fail" else "NOT RED (%s)" % st, ("  also red: " + ", ".join(others)) if others else ""))
        if st != "fail": bad += 1
    for label in sorted(set(declared) - set(clean)):
        print("STRAY MUTANT %s" % label); bad += 1
    print("%d checks green on the model, %d mutants, %d not red" % (len(clean), len(declared), bad))
    return 1 if bad else 0


def main():
    path = os.path.abspath(__file__)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    rows_ = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# J9.") and " | " in l]
    print_cases(r)
    print("MUTANTS (a check is green only after it has been seen red on its mutant)")
    print("| check | mutant |")
    print("|---|---|")
    for k, v in rows_:
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


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else main())
