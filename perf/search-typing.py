"""The main window's search against the 50 ms limit (docs/CONSTANTS.md OPERATION_MS), on the hidden test copy and the test engine's sample mail:
the time from a typed letter to the frame that shows its suggestions, and from Enter to the frame that shows the first result rows. The app's own
measures (`measure mac-main.interaction.frame` / `.commit`, the method of perf/interaction.py). Seven runs each; the median and the worst are printed."""
import json, os, statistics, sys, time
sys.path.insert(0, os.getcwd())
from journeys.lib.ctl import Ctl
c = Ctl().connect()
LIMIT = 50.0
def ms(kind):
    return float(c.measure("mac-main.interaction." + kind)["ms"])
def rows(ax):
    try: return c.rows(ax)
    except Exception as e: return [{"err": str(e)[:120]}]
def key(k):
    try: c.key(k); return True
    except Exception as e: return "ERR %s" % str(e)[:120]
letter, enter, shown = [], [], []
words = ["quote", "invoice", "thursday", "agenda", "tender", "brand", "sign"]
print("load at start %.1f" % os.getloadavg()[0])
for w in words:
    print("open search:", key("⌘K"), end=" ")
    # ⌘K's own frame is waited for before the letter (as perf/interaction.py waits for the way back): typed right behind ⌘K, the letter waited in the
    # queue for the search field's opening and that time was read as the letter's (main189: arrival.queue 31 ms of a 60 to 76 ms median; a main-thread
    # sample of 28 letters typed one by one reads 13 ms of work a letter).
    try: ms("frame")
    except Exception: pass
    c.type(w[0]); f, cm = ms("frame"), ms("commit"); letter.append(max(f, cm))
    sug = rows("mac-search.suggestions")
    c.type(w[1:]); time.sleep(0.05)
    key("Enter"); f2, cm2 = ms("frame"), ms("commit"); enter.append(max(f2, cm2))
    res = rows("mac-search.results")
    shown.append((len(sug), len(res)))
    print("%-9s letter: frame %.1f commit %.1f ms, %d suggestion rows | Enter: frame %.1f commit %.1f ms, %d result rows" % (w, f, cm, len(sug), f2, cm2, len(res)), flush=True)
    key("Escape"); key("Escape"); time.sleep(0.15)
def line(name, xs):
    med, worst = statistics.median(xs), max(xs)
    print("%s: median %.1f ms, worst %.1f ms of %d runs -> %s (limit %.0f ms)" % (name, med, worst, len(xs), "UNDER" if worst < LIMIT else ("median under, worst over" if med < LIMIT else "OVER"), LIMIT))
    return med < LIMIT, worst < LIMIT
a = line("suggestions after a typed letter", letter)
b = line("first result rows after Enter", enter)
print("rows shown per run (suggestions, results):", shown)
# where the time goes for one more letter and one more Enter: the app's own work (dispatch) beside the waits for the main queue and the display
key("⌘K")
try: ms("frame")
except Exception: pass
c.type("q"); p = c.measure("mac-main.interaction.phases")["phases"]
print("one typed letter: the app's work %.2f ms, wait for the main queue %.1f ms, wait for the display update %.1f ms" % (p.get("dispatch", 0), p.get("arrival.queue", 0), p.get("update.queue", 0)))
c.type("uote"); key("Enter"); p = c.measure("mac-main.interaction.phases")["phases"]
print("one Enter: the app's work %.2f ms, wait for the main queue %.1f ms, wait for the display update %.1f ms" % (p.get("dispatch", 0), p.get("arrival.queue", 0), p.get("update.queue", 0)))
key("Escape"); key("Escape")
end = os.getloadavg()[0]
print("load at end %.1f" % end)
print("verdict: %s" % ("judged: both under the limit" if a[1] and b[1] else ("NOT JUDGED: a time is over the limit with the machine load above 12 (docs/CONSTANTS.md PERF_LOAD_MAX)" if end > 12 else "OVER the limit")))
sys.exit(0 if a[1] and b[1] else (2 if end > 12 else 1))
