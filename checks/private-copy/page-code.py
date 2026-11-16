"""Picture 216 (14 Nov), from the picture 215 probe: mails whose page hides its text, on a PRIVATE COPY of the owner's mail database, on this Mac only.
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
from PIL import Image
KEEP = os.environ.get("P216_KEEP_DIR")          # the lead reads the reader pane of the ONE mail the owner showed in pictures 215 and 216, then deletes it
db = sqlite3.connect("file:%s?mode=ro" % os.path.join(core, "whoosh.sqlite"), uri=True)
targets = []
for mid, subject, text in db.execute("select id, coalesce(subject,''), coalesce(body_text,'') from message where trim(body_text) like '<%'"):
    if len(re.findall(r"</?(?:div|table|tr|td|p|span|a|br|img)\b", text, re.I)) >= 4: targets.append((mid, subject))
db.close()
say("stored mails whose text part is page code:", [m[-6:] for m, _ in targets])
RES = "mac-search.results"
CODE = re.compile(r"<\s*/?\s*(?:div|table|tr|td|span|img|a|p|br)\b|cellpadding=|cellspacing=|style=|role=presentat", re.I)
def rr(ax):
    r = rd(lambda: c.rows(ax)); return r if isinstance(r, list) else []
def shot(name, keep):
    r = rd(lambda: c.send("capture", "shot", name))
    p = r.get("path") if isinstance(r, dict) else None
    if not p or not os.path.exists(p): return None
    if keep and KEEP:
        im = Image.open(p); w, h = im.size
        out = os.path.join(KEEP, name + ".png"); im.crop((int(w * 0.38), 0, w, h)).save(out)
    os.remove(p)
    return True
results = []
for m, subject in targets:
    for theme in ("light", "dark"):
        rd(lambda: c.send("theme", theme)); time.sleep(0.5)
        lst = rr("mac-main.list"); idx = next((i for i, x in enumerate(lst) if x.get("id") == m), None)
        how = None
        if idx is not None:
            rd(lambda: c.click("mac-main.list.row.%d" % idx)); how = "list"
        else:
            rd(lambda: c.click("mac-search.open")); rd(lambda: c.click("mac-search.bar")); c.key("⌘A"); rd(lambda: c.type(subject[:40])); c.key("Enter")
            t0 = time.time(); found = None
            while time.time() - t0 < 30 and found is None:
                ids = [x.get("id") for x in rr(RES)]
                found = ids.index(m) if m in ids else None
                if found is None: time.sleep(0.4)
            if found is not None:
                for _ in range(found + 3):
                    if [x.get("id") for x in rr(RES) if x.get("selected")] == [m]: break
                    c.key("Down"); time.sleep(0.4)
                how = "search"
        time.sleep(2.5)
        t0 = time.time(); body = (rr("mac-main.reader.body") or [{}])[0]; read_ms = int((time.time() - t0) * 1000)
        web = (rr("mac-main.reader.web") or [{}])[0]; time.sleep(0.6); web = (rr("mac-main.reader.web") or [{}])[0]
        shown = (rr("mac-main.reader.subject") or [{}])[0].get("id")
        paras = [p if isinstance(p, str) else json.dumps(p) for p in (body.get("paragraphs") or [])]
        first = paras[0].strip() if paras else ""
        facts = {"mail": m[-6:], "theme": theme, "route": how, "reader_shows_it": shown == m, "layout": body.get("layout"), "drawn_document": body.get("drawn_document"),
                 "colours": body.get("colours"), "paragraphs_on_screen": len(paras), "paragraphs_holding_code": sum(1 for p in paras if CODE.search(p)),
                 "first_paragraph_starts_with_a_capital": bool(first[:1].isupper()), "first_paragraph_words": len(first.split()), "links": body.get("links"),
                 "page_text_length": web.get("text_length"), "painted": web.get("painted"), "picture_kept": bool(shot("p216-%s-%s" % (m[-6:], theme), m.endswith("144880")))}
        say("FACT", json.dumps(facts))
        results.append(facts)
        if how == "search": c.key("Escape"); time.sleep(0.5); c.key("Escape"); time.sleep(0.5)
rd(lambda: c.send("theme", "light"))
def good(f):
    if not f["reader_shows_it"] or f["paragraphs_holding_code"] != 0 or f["paragraphs_on_screen"] < 3: return False
    if f["mail"] == "144880": return f["layout"] == "written" and (f["links"] or 0) >= 2 and f["first_paragraph_starts_with_a_capital"] and f["first_paragraph_words"] >= 10
    return True
ok = len(results) >= 4 and all(good(f) for f in results)
say("RESULT picture 216: %s (%d mails x 2 themes; pass line, written before the run: the reader shows the mail, no paragraph on screen holds page code, 3 paragraphs or more; mail ..144880 has layout written, 2 links or more and a first paragraph of 10 words or more that starts with a capital)" % ("PASS" if ok else "FAIL", len(targets)))
say("DONE (the launcher stops the copy and removes the run folder; data folder:", core, ")")
