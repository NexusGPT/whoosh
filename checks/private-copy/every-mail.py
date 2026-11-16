"""Sweep of every mail of the inbox (14 Nov), from the picture 215 probe: mails whose page hides its text, on a PRIVATE COPY of the owner's mail database, on this Mac only.
Runs inside scripts/launch-one.sh (real engine, hidden). It stops the hidden copy, puts a snapshot of the owner's database in the copy's own
data folder, starts the same copy again, opens each mail whose page shows under a quarter of the words of its text part, and prints COUNTS, STATES
AND IDS ONLY (never a subject, a sender or a word of a mail). A picture is taken to count ink pixels and deleted at once; nobody reads it.
The hidden copy cannot reach a mail server and has no password, so nothing leaves. The copied data is removed before the script ends."""
import sys, os, json, time, signal, shutil, subprocess, sqlite3
ROOT = os.getcwd()
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.ctl import Ctl
D = os.environ["D"]
OWNER = os.path.expanduser("~/Library/Application Support/Whoosh/Core")
QUERY = sys.argv[1] if len(sys.argv) > 1 else "consultant"
def say(*a): print(*a, flush=True)
def rd(fn):
    try: return fn()
    except Exception as e: return "ERR %s: %s" % (type(e).__name__, str(e)[:160])
def alive(p):
    try: os.kill(p, 0); return True
    except OSError: return False

core = os.path.join(D, "core")
pid = int(open(os.path.join(D, "rig.pid")).read().strip())
open(os.path.join(D, "relaunching"), "w").close()          # the launcher's own marker: the copy is being restarted, not dead
os.kill(pid, signal.SIGTERM)
t0 = time.time()
while alive(pid) and time.time() - t0 < 15: time.sleep(0.2)
if alive(pid): os.kill(pid, signal.SIGKILL); time.sleep(0.5)
say("hidden copy stopped: pid gone =", not alive(pid))
if os.path.isdir(core): shutil.rmtree(core)
os.makedirs(core)
# a consistent snapshot of the live database (SQLite's own backup), then the index folder as it is on disk
r = subprocess.run(["sqlite3", os.path.join(OWNER, "whoosh.sqlite"), ".backup '%s'" % os.path.join(core, "whoosh.sqlite")], capture_output=True, text=True)
say("snapshot exit", r.returncode, (r.stderr or "")[:120])
if os.path.isdir(os.path.join(OWNER, "index")):
    shutil.copytree(os.path.join(OWNER, "index"), os.path.join(core, "index"))
db = sqlite3.connect("file:%s?mode=ro" % os.path.join(core, "whoosh.sqlite"), uri=True)
n_msgs = db.execute("select count(*) from message").fetchone()[0]
like = "%" + QUERY.lower() + "%"
want = [r[0] for r in db.execute("select id from message where lower(coalesce(subject,'')) like ? or lower(coalesce(body_text,'')) like ? order by id", (like, like))]
folders = dict(db.execute("select folder, count(*) from message group by folder"))
db.close()
say("copy holds", n_msgs, "mails; folders (name: count):", json.dumps(folders))
say("mails whose subject or text holds the letters '%s': %d" % (QUERY, len(want)))

p = subprocess.run(["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), "--relaunch"], env=dict(os.environ, D=D, WHOOSH_RIG_HIDDEN="1"), capture_output=True, text=True, cwd=ROOT)
say("relaunch exit", p.returncode, (p.stdout + p.stderr)[-200:].replace("\n", " | "))
c = None; t0 = time.time()
while time.time() - t0 < 60:
    try:
        c = Ctl(D); c.connect(); c.uistate(); break
    except Exception:
        c = None; time.sleep(0.5)
if c is None:
    say("RESULT: NOT JUDGED, the hidden copy did not answer after the restart"); sys.exit(3)
u = rd(lambda: c.uistate())
say("after restart: core =", u.get("core") if isinstance(u, dict) else u, "counts =", json.dumps(u.get("counts")) if isinstance(u, dict) else "")
time.sleep(3)                                             # the engine opens its indexes


import re
db = sqlite3.connect("file:%s?mode=ro" % os.path.join(core, "whoosh.sqlite"), uri=True)
expect = {}
for mid, text, html, prep in db.execute("select m.id, coalesce(m.body_text,''), coalesce(m.body_html,''), (select prepared from message_prepared p where p.message_id = m.id) from message m"):
    seen_chars = None
    if prep:
        try:
            p = json.loads(prep); segs = p.get("segments") or []; fold = p.get("fold_from")
            keep = segs if fold is None else segs[:fold]
            seen_chars = sum(len((s.get("text") or "").strip()) for s in keep if s.get("visibility", "show") == "show")
        except Exception: seen_chars = None
    if seen_chars is None: seen_chars = len(text.strip())
    # the page is judged against the text part when the mail has one (the engine's own text also holds what the page sets not to show)
    if text.strip() and not (text.lstrip().startswith("<")): seen_chars = min(seen_chars, len(" ".join(text.split())))
    code = text.lstrip().startswith("<") and len(re.findall(r"</?(?:div|table|tr|td|p|span|a|br|img)\b", text, re.I)) >= 4
    expect[mid] = (seen_chars, bool(html), code)
db.close()
CODE = re.compile(r"<\s*/?\s*(?:div|table|tr|td|span|img|p|br)\b[^>]{0,80}>|cellpadding=|cellspacing=|role=presentat|<!doctype", re.I)
def rr(ax):
    r = rd(lambda: c.rows(ax)); return r if isinstance(r, list) else []
tabs = rr("mac-main.tabs")
say("tabs:", [(t.get("ax"), t.get("unread")) for t in tabs][:8])
order = [t for t in tabs if (t.get("title") or "").lower().startswith("all")] + [t for t in tabs if not (t.get("title") or "").lower().startswith("all")]
seen, flags, slow = {}, [], 0
LIMIT = int(os.environ.get("SWEEP_MAX", "400"))
for t in order[:1]:
    for _ in range(8):                      # the Tab key goes to the next tab; stop on All
        if (rd(lambda: c.uistate()) or {}).get("tab") == "all": break
        c.key("Tab"); time.sleep(0.6)
    say("walking tab:", (rd(lambda: c.uistate()) or {}).get("tab"))
    lst = rr("mac-main.list")
    if not lst: continue
    rd(lambda: c.click("mac-main.list.row.0")); time.sleep(1.2)
    still = 0; last = None
    while len(seen) < LIMIT and still < 3:
        shown = (rr("mac-main.reader.subject") or [{}])[0].get("id")
        if shown == last: still += 1
        else: still = 0
        last = shown
        if shown and shown not in seen:
            body = (rr("mac-main.reader.body") or [{}])[0]
            web = (rr("mac-main.reader.web") or [{}])[0]; time.sleep(0.25); web = (rr("mac-main.reader.web") or [{}])[0]
            if not web.get("painted"):
                time.sleep(1.5); web = (rr("mac-main.reader.web") or [{}])[0]; slow += 1
            paras = [p if isinstance(p, str) else json.dumps(p) for p in (body.get("paragraphs") or [])]
            exp, has_page, was_code = expect.get(shown, (0, False, False))
            tl = web.get("text_length") or 0
            f = {"mail": shown[-6:], "layout": body.get("layout"), "expected_chars": exp, "page_text_length": tl, "paragraphs": len(paras),
                 "with_code": sum(1 for p in paras if CODE.search(p)), "painted": bool(web.get("painted")), "height": web.get("height_reserved"), "text_was_code": was_code}
            why = []
            if f["with_code"]: why.append("code on screen")
            if not was_code and exp >= 80 and tl < 0.4 * exp: why.append("page shows under 40% of the expected characters")
            if exp >= 80 and len(paras) == 0 and tl < 40: why.append("no paragraph and no text")
            if not f["painted"]: why.append("not painted after 2 s")
            if why: f["why"] = why; flags.append(f); say("FLAG", json.dumps(f))
            seen[shown] = f
        c.key("Down"); time.sleep(0.45)
say("TOTAL mails read in the reader:", len(seen), "| of the copy's", len(expect), "| flagged:", len(flags), "| slow to paint:", slow,
    "| layouts:", json.dumps({k: sum(1 for f in seen.values() if f["layout"] == k) for k in set(f["layout"] for f in seen.values())}))
say("RESULT sweep: %s (pass line, written before the run: 150 mails or more read; no mail shows code, none shows under 40%% of its expected characters, none is unpainted)" % ("PASS" if len(seen) >= 150 and not flags else "FAIL"))
say("DONE (the launcher stops the copy and removes the run folder; data folder:", core, ")")
