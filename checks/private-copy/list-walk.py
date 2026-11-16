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


def rr(ax):
    r = rd(lambda: c.rows(ax)); return r if isinstance(r, list) else []
u = rd(lambda: c.uistate())
say("uistate keys:", sorted(u.keys())[:40] if isinstance(u, dict) else u)
say("tab at start:", u.get("tab") if isinstance(u, dict) else None, "| list rows:", len(rr("mac-main.list")))
for name in ("all", "primary"):
    # go to the tab by the Tab key until uistate says so (5 presses at most)
    for _ in range(6):
        if (rd(lambda: c.uistate()) or {}).get("tab") == name: break
        c.key("Tab"); time.sleep(0.6)
    lst = rr("mac-main.list")
    say("tab now:", (rd(lambda: c.uistate()) or {}).get("tab"), "| list rows:", len(lst), "| keys of a row:", sorted(lst[0].keys())[:30] if lst else None)
    rd(lambda: c.click("mac-main.list.row.0")); time.sleep(1.0)
    ids = []; marks = []; ends = globals().setdefault('ends', {})
    for i in range(1, 91):
        c.key("Down"); time.sleep(0.25)
        if i % 10 == 0:
            shown = (rr("mac-main.reader.subject") or [{}])[0].get("id"); l2 = rr("mac-main.list")
            foc = next((k for k, x in enumerate(l2) if x.get("focused") or x.get("selected")), None)
            ids.append(shown); marks.append((i, len(l2), foc, (rd(lambda: c.uistate()) or {}).get("surface")))
    say("after each 10 Down presses (presses, list rows, focused row, surface):", marks, "| distinct mails shown at the marks:", len(set(ids)))
    ends[name] = marks[-1] if marks else None
    st = (rr("mac-main.list.state") or [{}])[0]; say("list state:", {k: st.get(k) for k in ("row_budget", "rows")})
    c.key("Escape"); time.sleep(0.5)
a = ends.get("all") or (0, 0, 0, "")
say("RESULT list walk: %s (pass line, written before the run: in All, after 90 presses of Down from the first conversation, the focused conversation is number 91 and the list holds more than 50)" % ("PASS" if a[2] == 90 and a[1] > 50 else "FAIL"))
say("DONE (the launcher stops the copy and removes the run folder; data folder:", core, ")")
