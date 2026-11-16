#!/usr/bin/env python3
"""Laya with its real weights on the fixture's mail (item 49c8d129; lane fix-aisort round 14).

    python3 checks/laya-real-check.py <bundle> --models <folder>   the lead's run: <folder> holds the five Laya files
                                                                    (typed-decisions/..., the paths LayaModelStore asks); the check serves it
                                                                    on 127.0.0.1 only, starts the hidden copy, chooses Laya, downloads it,
                                                                    types the Needs Reply description, re-runs, and judges the run against
                                                                    docs/lanes/laya-real-weights.passline.md
    python3 checks/laya-real-check.py <bundle> --models <folder> --model <catalogue id>
                                                                    the same run with another line of the model list (SPEC A133): <folder> holds that
                                                                    model's files at the names its catalogue line lists; the check stages them with
                                                                    `fixture provider-endpoint model:<id> <loopback address>`, presses Download on the
                                                                    model's line and waits until it is the model in use
    python3 checks/laya-real-check.py --selftest                    launches nothing: the judge on a stand-in answer and its broken copies,
                                                                    the loopback server on a stand-in folder of five small files

What it does (the pass-line file says why each number is what it is):
1. The 16 messages of docs/design/fixtures/mac-main-sample.json go into the loopback IMAP fake (checks/realcore_mailfake.py).
2. The folder is served by `python3 -m http.server --bind 127.0.0.1` (the check's own pid, stopped at the end; its request log is kept).
3. scripts/launch-one.sh starts the hidden copy (WHOOSH_RIG_HIDDEN=1) with WHOOSH_RIG_LAYA_BASE=http://127.0.0.1:<port>/ and the mail servers.
4. Inside launch-one (`--inner`), in this order (round 15): Settings, Decisions, Laya (read back from the pane's engine rows), Download,
   ready; then the account through its sheet, the 16 messages on the All view, the arrival's own judging until no Needs Reply mail waits
   for the AI; Needs Reply's description typed and saved; what the core says a re-run would do (flow/ai_refusal, rerun/candidates,
   rerun/preview); Re-run; the footprint and resident size of the copy sampled from before the first mail, the re-run's progress read per
   step; the filing of each message read; the copy's network connections listed with lsof. A step the app does not take is NOT JUDGED
   with what the app answered (`step_cases`), and every message's line then names that step.
5. One line per message: `PASS laya <id> decided_by=<who> <ms> ms rss <MB> MB` (or FAIL, with why), then a totals line.
The memory sampling and the copy's pids are laya-sanity's own (`app_footprint_mb`, `app_pids`, `ledger_number`).
"""
import importlib.util
import json
import os
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.parse
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FIXTURE = os.path.join(ROOT, "docs", "design", "fixtures", "mac-main-sample.json")
PASSLINE_REL = "docs/lanes/laya-real-weights.passline.md"
PASSLINE = os.path.join(ROOT, PASSLINE_REL)
# the five files the app asks for, relative to WHOOSH_RIG_LAYA_BASE (Sources/Features/OnDeviceAI/Core/LayaModelStore.swift:25-29,
# checks/settingsaimac-check.py LAYA_FILES)
LAYA_FILES = ["typed-decisions/rl_agent_config.json", "typed-decisions/encoder/config.json", "typed-decisions/tokenizer/tokenizer.json",
              "typed-decisions/tokenizer/tokenizer_config.json", "typed-decisions/model.safetensors"]
ADD = "mac-add-account"                          # the account sheet's doors (checks/realcore-check.py flow_drive)
SETTINGS = "mac-settings-ai.decisions"           # Sources/Features/SettingsAIMac/SettingsAIMacFeature.swift:50,207,217
INS = "mac-labels.inspector"                     # Sources/Features/LabelsContactsMac/LabelInspectorSocket.swift:65-80
LABEL = "needs-reply"                            # the built-in label's id (core/crates/store/src/label_meta.rs builtin_seeds)
SAMPLE_S = 0.25          # the memory and progress reads during the re-run (laya-sanity's mutant pace)
SYNC_WAIT_S = 120        # the 16 messages reach the list
READY_WAIT_S = 900       # the download of about 0.85 GB from this Mac's own loopback, and the check of its files
RERUN_WAIT_S = 900       # the re-run of 16 messages with the model's load
DETECT_WAIT_S = 90       # the account sheet's steps (checks/realcore-check.py DETECT_WAIT_S)
SETTLE_WAIT_S = 300      # the arrival's own judging of the 16 messages with Laya (round 15)
SETTLE_READS = 3         # the arrival is settled when nothing waits and this many reads in a row, a second apart, say the same (main115)


def load_sanity():
    """checks/laya-sanity.py as a module (its file name holds a dash): the copy's pids, its footprint, the ledger's numbers."""
    spec = importlib.util.spec_from_file_location("laya_sanity", os.path.join(ROOT, "checks", "laya-sanity.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


LS = load_sanity()


# ---------------------------------------------------------------- the pass line and the fixture

def passline(path=PASSLINE):
    """The machine lines of the pass-line file: must-land and must-not-land ids, the description, the three ledger names."""
    out = {}
    for line in open(path, encoding="utf-8"):
        m = re.match(r"^(must-land|must-not-land|description|seconds-per-message|peak-mb|growth-mb): (.+)$", line.rstrip("\n"))
        if m:
            out[m.group(1)] = m.group(2).split() if m.group(1) in ("must-land", "must-not-land") else m.group(2).strip()
    return out


def bars(pl):
    """The numbers the pass line names, read from docs/CONSTANTS.md now."""
    return {"seconds": LS.ledger_number(pl.get("seconds-per-message", "")), "peak_mb": LS.ledger_number(pl.get("peak-mb", "")),
            "growth_mb": LS.ledger_number(pl.get("growth-mb", ""))}


def fixture_messages(path=FIXTURE):
    return json.load(open(path, encoding="utf-8"))["messages"]


def raw_message(m, uid, to):
    """One RFC 5322 message from a fixture message: its sender, subject and body lines; uid is also the Date's minute."""
    body = m.get("body")
    text = "\r\n".join(body) if isinstance(body, list) else str(body or m.get("preview") or "")
    sender = "%s <%s>" % (m.get("sender") or m["from"], m["from"])
    return ("From: %s\r\nTo: %s\r\nSubject: %s\r\nMessage-ID: <%s@whoosh-check.invalid>\r\nDate: Mon, 09 Nov 2026 10:%02d:00 +0000\r\n"
            "Content-Type: text/plain; charset=utf-8\r\n\r\n%s\r\n" % (sender, to, m["subject"], m["id"], uid, text)).encode("utf-8")


def git_facts():
    """(commit, the pass-line file's commit date or None, whether the file differs from its commit)."""
    run = lambda *a: subprocess.run(["git", "-C", ROOT] + list(a), capture_output=True, text=True)
    head = run("rev-parse", "--short", "HEAD").stdout.strip()
    date = run("log", "-1", "--format=%cI", "--", PASSLINE_REL).stdout.strip() or None
    dirty = bool(run("status", "--porcelain", "--", PASSLINE_REL).stdout.strip())
    return head, date, dirty


# ---------------------------------------------------------------- the loopback server

def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def serve(folder, log_path):
    """`python3 -m http.server` on 127.0.0.1 only, serving `folder`; its request log goes to `log_path`. Returns (process, port) once it
    answers, or raises OSError."""
    port = free_port()
    log = open(log_path, "w", encoding="utf-8")
    p = subprocess.Popen([sys.executable, "-m", "http.server", "--bind", "127.0.0.1", "--directory", folder, str(port)],
                         stdout=log, stderr=subprocess.STDOUT)
    end = time.time() + 10
    while time.time() < end:
        try:
            socket.create_connection(("127.0.0.1", port), timeout=0.5).close()
            return p, port
        except OSError:
            if p.poll() is not None:
                break
            time.sleep(0.1)
    stop(p)
    raise OSError("the loopback server did not answer on 127.0.0.1:%d" % port)


def stop(p):
    """Ends the server by ITS pid: TERM, then KILL after 5 s. Returns its exit code."""
    if p.poll() is None:
        p.terminate()
        try:
            p.wait(5)
        except subprocess.TimeoutExpired:
            p.kill()
            p.wait(5)
    return p.returncode


def served_paths(log_text):
    """[(method, path, status)] from http.server's log lines (`... "GET /a/b HTTP/1.1" 200 -`)."""
    out = []
    for m in re.finditer(r'"(GET|HEAD|POST|PUT) (\S+) HTTP/[0-9.]+" (\d{3})', log_text):
        path = urllib.parse.unquote(urllib.parse.urlsplit(m.group(2)).path).lstrip("/")
        out.append((m.group(1), path, int(m.group(3))))
    return out


def remote_addresses(lsof_text):
    """The remote ends of the connections `lsof -i -n -P` lists (`a:b->c:d (ESTABLISHED)`): ['c:d', ...]."""
    return [m.group(1) for m in re.finditer(r"->(\S+)", lsof_text)]


def loopback(addr):
    host = addr.rsplit(":", 1)[0].strip("[]")
    return host in ("127.0.0.1", "::1", "localhost") or host.startswith("127.")


# ---------------------------------------------------------------- inside launch-one

def rss_mb(pids):
    """The resident size summed over the copy's processes (`ps -o rss=`, kilobytes), or None."""
    if not pids:
        return None
    r = subprocess.run(["ps", "-o", "rss=", "-p", ",".join(str(p) for p in pids)], capture_output=True, text=True)
    vals = [int(v) for v in r.stdout.split() if v.isdigit()]
    return sum(vals) / 1024.0 if vals else None


def sample(run, t0):
    pids = LS.app_pids(run)
    return {"t": round(time.time() - t0, 2), "mb": LS.app_footprint_mb(run), "rss": rss_mb(pids)}


def inner(c=None, run=None, sampler=None, waits=None):
    """Inside launch-one. Round 15 (main114: the provider read a path the real core does not serve, and the re-run stayed idle with total 0
    and model_pending true): Laya is chosen and made ready BEFORE the mail arrives, so no judgement fails for want of the weights (a failed
    Laya judgement is kept by the core until one succeeds, decision_model.rs `laya_failure`, and while it stands `ai_can_answer` is false
    and a re-run skips the AI: flow_ctx.rs `rerun_label` `ai_skipped`). Every step records what the app answered, for its NOT JUDGED line.
    `c`, `run`, `sampler` and `waits` are the self-test's stand-ins; the lead's run passes none."""
    if c is None:
        sys.path.insert(0, os.path.join(ROOT, "journeys"))
        from lib.ctl import Ctl  # noqa: E402
        c = Ctl().connect()
    if run is None:
        run = os.environ.get("WHOOSH_SOCKET_DIR") or ""
    w = dict(SYNC=SYNC_WAIT_S, READY=READY_WAIT_S, RERUN=RERUN_WAIT_S, DETECT=DETECT_WAIT_S, SETTLE=SETTLE_WAIT_S, SETTLE_EVERY=1.0, SAMPLE=SAMPLE_S, POLL=0.3)
    w.update(waits or {})
    pl = passline()
    subjects = {m["subject"]: m["id"] for m in fixture_messages()}
    answer = {"errors": [], "steps": {}, "doors": []}
    t0 = time.time()
    samples = []
    take = sampler or (lambda: sample(run, t0))

    def step(name, fn):
        t = time.time()
        try:
            v = fn()
            answer["steps"][name] = round(time.time() - t, 2)
            return v
        except Exception as e:      # noqa: BLE001 (a refused click is a finding, recorded with its step)
            answer["errors"].append("%s: %s" % (name, str(e)[:300]))
            return None

    def click(name, ax):
        answer["doors"].append(ax)
        return step(name, lambda: c.click(ax))

    def read(fn):
        try:
            return fn()
        except Exception as e:      # noqa: BLE001
            return {"unreadable": str(e)[:200]}

    def wait(fn, done, seconds, every=None, sampling=False):
        v, end = None, time.time() + seconds
        while True:
            if sampling:
                samples.append(take())
            v = read(fn)
            if done(v):
                return v, True
            if time.time() >= end:
                return v, False
            time.sleep(every if every is not None else w["POLL"])

    def core_json(path):
        v = c.core(path)
        return json.loads(v) if isinstance(v, str) and v[:1] in "{[" else v

    # 1. Settings, Decisions: Laya chosen (read back from the engine rows the pane serves), Download, ready
    click("open Settings, Decisions", "mac-settings-window.pane-row.decisions")
    click("choose Laya", SETTINGS + ".engine.local")
    answer["engine"] = read(lambda: c.rows(SETTINGS + ".engine"))
    t_dl = time.time()
    other = os.environ.get("WHOOSH_REAL_MODEL") or ""          # another line of the model list (`--model`): staged on this Mac, downloaded, in use
    if other:
        mx = SETTINGS + ".model." + other
        answer["staged"] = "at launch: WHOOSH_RIG_MODEL_ID and WHOOSH_RIG_MODEL_BASE %s" % (os.environ.get("WHOOSH_REAL_MODEL_BASE") or "")
        click("press Download on the model's line", mx + ".download")
        # a download that asks nothing within START seconds is not waited for (main191: 900 s spent on a line that could not start)
        began, _ = wait(lambda: c.rows(mx)[0], lambda v: isinstance(v, dict) and (v.get("in_use") is True or v.get("state") in ("failed", "ready") or (v.get("received") or 0) > 0), w.get("START", 60), 1.0)
        row = began
        if isinstance(began, dict) and began.get("state") != "failed" and ((began.get("received") or 0) > 0 or began.get("state") == "ready"):
            row, _ = wait(lambda: c.rows(mx)[0], lambda v: isinstance(v, dict) and (v.get("in_use") is True or v.get("state") == "failed"), w["READY"], 1.0)
        answer["model_line"] = row
        answer["model_kept"] = read(lambda: c.core("decision_model/model"))
        if isinstance(row, dict) and row.get("in_use") is True:
            row = dict(row, state="ready")                      # the judge's step reads `state`: in use means on this Mac and chosen
    else:
        click("press Download", SETTINGS + ".local.download")
        row, _ = wait(lambda: c.rows(SETTINGS + ".local")[0], lambda v: isinstance(v, dict) and v.get("state") in ("ready", "failed"), w["READY"], 1.0)
    answer["laya"] = dict(row if isinstance(row, dict) else {"read": row}, seconds=round(time.time() - t_dl, 1))
    answer["engine_after_ready"] = read(lambda: c.rows(SETTINGS + ".engine"))
    # 2. the account, then the 16 messages on the All view (Primary holds only what no label took); the footprint from before the first mail
    samples.append(take())
    click("open the account sheet", ADD + ".open")
    click("focus the address", ADD + ".address"); step("type the address", lambda: c.type("owner@whoosh-check.invalid"))
    click("focus the password", ADD + ".password"); step("type the password", lambda: c.type("check-password"))
    click("continue", ADD + ".continue")
    wait(lambda: c.rows(ADD + ".state"), lambda v: isinstance(v, list) and v and v[0].get("stage") not in ("detecting", "entry"), w["DETECT"])
    click("add", ADD + ".add")
    st, _ = wait(lambda: c.rows(ADD + ".state"), lambda v: isinstance(v, list) and v and v[0].get("stage") in ("added", "summary"), w["DETECT"])
    answer["account_stage"] = st[0].get("stage") if isinstance(st, list) and st else st
    click("close the sheet", ADD + ".done")

    def ids_now():
        splits = c.rows("mac-main.splits")
        i = next((k for k, r in enumerate(splits) if r.get("id") == "all"), None)
        if i is not None:
            answer["doors"].append("mac-main.splits.%d" % i)
            c.click("mac-main.splits.%d" % i)
        return {subjects[r["subject"]]: r.get("id") for r in c.rows("mac-main.list") if r.get("subject") in subjects and r.get("id")}
    ids, _ = wait(ids_now, lambda v: isinstance(v, dict) and "unreadable" not in v and len(v) == len(subjects), w["SYNC"], 1.0, sampling=True)
    answer["ids"] = ids if isinstance(ids, dict) and "unreadable" not in ids else {}
    answer["ids_read"] = ids if not answer["ids"] else None
    # 3. the arrival's own judging ends (no mail of Needs Reply waits for the AI), so the re-run's time is its own
    # main115: Needs Reply has no step 1, so `caught_by_entry` stays 0 (the old test required it above 0 and never passed). Settled: no mail
    # waits for the AI (round 17: a mail whose judgement failed is counted as waiting on such a label) and the counts with the label's refusal
    # read the same SETTLE_READS times in a row, so a read taken before the worker began is not taken for the end
    seen = []

    def settled_now(v):
        seen.append(json.dumps(v, sort_keys=True))
        return isinstance(v, dict) and (v.get("counts") or {}).get("waiting_for_ai") == 0 and len(seen) >= SETTLE_READS and len(set(seen[-SETTLE_READS:])) == 1
    both, settled = wait(lambda: {"counts": core_json("label/%s/flow/counts" % LABEL), "ai_refusal": core_json("label/%s/flow/ai_refusal" % LABEL)},
                         settled_now, w["SETTLE"], w["SETTLE_EVERY"], sampling=True)
    both = both if isinstance(both, dict) else {}
    answer["arrival"] = {"counts": both.get("counts"), "settled": settled, "ai_refusal": both.get("ai_refusal"), "reads": len(seen)}
    # 4. Needs Reply: the description typed and saved; what the inspector and the core say a re-run would do
    click("open Needs Reply", "%s.open.modify.%s" % (INS, LABEL))
    step("type the description", lambda: c.type(pl.get("description", "")))
    click("set the description", INS + ".meaning")
    click("Save", INS + ".apply")
    summary = read(lambda: c.rows(INS)[0])
    answer["inspector"] = {k: summary.get(k) for k in ("meaning", "status", "failure", "refusal", "offer", "foot", "preview_line")} if isinstance(summary, dict) else summary
    answer["meaning"] = summary.get("meaning") if isinstance(summary, dict) else None
    answer["before_rerun"] = {"ai_refusal": read(lambda: core_json("label/%s/flow/ai_refusal" % LABEL)),
                              "candidates": read(lambda: c.core("label/%s/rerun/candidates" % LABEL)),
                              "preview": read(lambda: core_json("label/%s/rerun/preview" % LABEL))}
    # 5. Re-run: memory every SAMPLE_S, progress per step, until the core says done
    samples.append(take())
    answer["rerun_click"] = click("Re-run", INS + ".rerun")
    t_click = time.time()
    progress, seen_running, done_state = [], False, None
    while time.time() - t_click < w["RERUN"] and answer["rerun_click"] is not None:
        samples.append(take())
        try:
            state, done, total = c.core("label/%s/rerun/state" % LABEL), c.core("label/%s/rerun/done" % LABEL), c.core("label/%s/rerun/total" % LABEL)
        except Exception as e:  # noqa: BLE001
            answer["errors"].append("read the re-run: %s" % str(e)[:200])
            break
        if not progress or (progress[-1]["done"], progress[-1]["state"]) != (done, state):
            progress.append({"t": round(time.time() - t_click, 2), "done": done, "total": total, "state": state})
        seen_running = seen_running or state == "running"
        if state == "done" and isinstance(total, int) and total > 0 and done == total:
            done_state = {"state": state, "done": done, "total": total, "seen_running": seen_running, "seconds": round(time.time() - t_click, 2)}
            break
        time.sleep(w["SAMPLE"])
    answer["rerun"] = done_state or {"state": "not done", "progress_last": progress[-1] if progress else None, "seconds": round(time.time() - t_click, 2)}
    answer["progress"] = progress
    answer["samples"] = samples
    for leaf in ("filed", "refiled", "unfiled", "model_pending"):
        answer["rerun"][leaf] = read(lambda leaf=leaf: c.core("label/%s/rerun/%s" % (LABEL, leaf)))
    answer["ai_refusal"] = read(lambda: core_json("label/%s/flow/ai_refusal" % LABEL))
    # 6. each message's filing, and the copy's connections
    answer["filings"] = {fid: read(lambda mid=mid: core_json("message/%s/filing" % mid)) for fid, mid in sorted(answer["ids"].items())}
    # 7. "Why here?" of the first mail filed in the label: the core's read and the panel's own words (SPEC A133: a model of the list is named)
    filed = [fid for fid, f in sorted(answer["filings"].items()) if isinstance(f, dict) and f.get("place") == LABEL]
    if filed:
        mid = answer["ids"][filed[0]]
        why = {"message": filed[0], "core": read(lambda: core_json("message/%s/why/%s" % (mid, LABEL)))}
        served = read(lambda: [r.get("id") for r in c.rows("mac-main.list")])
        if isinstance(served, list) and mid in served:
            click("focus the filed mail", "mac-main.list.row.%d" % served.index(mid))
            click("Why here?", INS + ".why")
            why["lines"] = read(lambda: [r.get("text") for r in c.rows(INS + ".why")])
            click("Why here? closed", INS + ".why")
        else:
            why["lines"] = {"unreadable": "the filed mail %s is not in the served list (%r)" % (mid, served if not isinstance(served, list) else len(served))}
        answer["why"] = why
    pids = LS.app_pids(run) if run else []
    r = subprocess.run(["lsof", "-a", "-p", ",".join(str(p) for p in pids), "-i", "-n", "-P"], capture_output=True, text=True) if pids else None
    answer["lsof"] = {"pids": pids, "exit": r.returncode if r else None, "text": (r.stdout + r.stderr) if r else ""}
    print("ANSWER " + json.dumps(answer))
    return 0


# ---------------------------------------------------------------- the judge

def step_cases(answer, pl, fixture_ids):
    """Round 15: the run's steps in order, each (case, ok, detail) with what the app answered; ok True or None (a step the app did not
    take is not judged, never failed: the run did not reach what the item measures)."""
    eng = answer.get("engine_after_ready") or answer.get("engine")
    chosen = [r.get("id") for r in eng if isinstance(r, dict) and r.get("chosen")] if isinstance(eng, list) else None
    local = next((r for r in eng if isinstance(r, dict) and r.get("id") == "local"), {}) if isinstance(eng, list) else {}
    laya = answer.get("laya") or {}
    ids = answer.get("ids") or {}
    arrival = answer.get("arrival") or {}
    before = answer.get("before_rerun") or {}
    rerun = answer.get("rerun") or {}
    cand = before.get("candidates")
    preview = before.get("preview")
    cand = int(cand) if isinstance(cand, (int, str)) and str(cand).isdigit() else None
    out = [
        ("step-1-laya-is-chosen", chosen == ["local"] and not local.get("refusal"),
         "the pane's engine rows say chosen %r, Laya's refusal %r, labels judged by %r (want local chosen, no refusal)" % (chosen, local.get("refusal"), local.get("labels_by"))),
        ("step-2-laya-is-ready", laya.get("state") == "ready", "the Laya row %r" % laya),
        ("step-3-the-16-messages-are-listed", len(ids) == len(fixture_ids),
         "%d of %d found on the All view (account stage %r; last read %r)" % (len(ids), len(fixture_ids), answer.get("account_stage"), answer.get("ids_read"))),
        ("step-4-the-arrival-judging-settled", bool(arrival.get("settled")),
         "label/needs-reply/flow/counts %r, flow/ai_refusal %r, %r reads (want waiting_for_ai 0, the same %d reads in a row)" % (
             arrival.get("counts"), arrival.get("ai_refusal"), arrival.get("reads"), SETTLE_READS)),
        ("step-5-the-description-is-saved", answer.get("meaning") == pl.get("description"), "the inspector %r" % answer.get("inspector")),
        # main115: an under-trust refusal (`ai_under_trust`: the AI judged them and was sure of none) does not stop a re-run; the core's own
        # preview says whether the AI will be asked (`ai_words` null and `by_ai` above 0), flow_ctx.rs `rerun_preview`
        ("step-6-the-ai-can-answer-before-the-re-run", bool(cand) and isinstance(preview, dict) and preview.get("ai_words") is None and (preview.get("by_ai") or 0) > 0,
         "flow/ai_refusal %r, rerun/candidates %r, rerun/preview %r (want candidates > 0 and the preview's ai_words null with by_ai > 0: else the re-run skips the AI)" % (
             before.get("ai_refusal"), before.get("candidates"), preview)),
        ("step-7-the-re-run-ran", answer.get("rerun_click") is not None and rerun.get("state") == "done" and rerun.get("total") == len(fixture_ids),
         "the Re-run click %s; re-run %r; flow/ai_refusal after %r" % ("answered" if answer.get("rerun_click") is not None else "refused", rerun, answer.get("ai_refusal"))),
    ]
    out = [(n, True if ok else None, d) for n, ok, d in out]
    errors = answer.get("errors") or []
    if errors:
        out.append(("the-doors-answered", None, "refused: %r" % errors))
    return out



def step_seconds(progress):
    """[(seconds per message of the step, messages it added)] from the re-run's progress reads; the first step from the click (t 0, done 0)."""
    out, last_t, last_done = [], 0.0, 0
    for p in progress or []:
        d = p.get("done")
        if not isinstance(d, int) or d <= last_done:
            continue
        out.append(((p["t"] - last_t) / (d - last_done), d - last_done))
        last_t, last_done = p["t"], d
    return out


def judge(answer, pl, b, served, fixture_ids):
    """(results [(case, ok, detail)], message lines, totals line). ok None = not judged."""
    results, lines = [], []
    must, must_not = set(pl.get("must-land") or []), set(pl.get("must-not-land") or [])
    steps_done = step_cases(answer, pl, fixture_ids)
    results += steps_done
    stuck = next(((n, d) for n, ok, d in steps_done if ok is None), None)
    reached = stuck is None
    steps = step_seconds(answer.get("progress"))
    slow = max((s for s, _ in steps), default=None)
    samples = [x for x in answer.get("samples") or [] if isinstance(x, dict)]
    peaks = {k: max((x[k] for x in samples if isinstance(x.get(k), (int, float))), default=None) for k in ("mb", "rss")}
    start = samples[0].get("mb") if samples and isinstance(samples[0].get("mb"), (int, float)) else None
    grew = peaks["mb"] - start if peaks["mb"] is not None and start is not None else None
    filings = answer.get("filings") or {}
    # another line of the model list (`--model`): a filing names that model ("model:<id>", SPEC A133) and the server gives that line's files
    chosen = (answer.get("model_line") or {}).get("model")
    want_by = "model:%s" % chosen if chosen else "laya"
    allowed = (catalogue_files(chosen) or []) if chosen else LAYA_FILES
    for fid in fixture_ids:
        f = filings.get(fid)
        if not isinstance(f, dict) or not reached:
            why = "the run stopped at %s" % stuck[0] if stuck else "no filing read (%r)" % (f,)
            results.append(("laya/" + fid, None, why))
            lines.append("NOT JUDGED laya %s %s" % (fid, why))
            continue
        by, place = f.get("decided_by"), f.get("place")
        why = []
        if place == LABEL and by != want_by:
            why.append("in Needs Reply but decided_by %s (want %s)" % (by, want_by))
        if fid in must and place != LABEL:
            why.append("must land in Needs Reply, filed at %s" % place)
        if fid in must_not and place == LABEL:
            why.append("must not land in Needs Reply")
        if slow is None or b["seconds"] is None or slow > b["seconds"]:
            why.append("seconds per message %s over %s" % ("unmeasured" if slow is None else "%.2f" % slow, b["seconds"]))
        for k, word in (("rss", "rss"), ("mb", "footprint")):
            if peaks[k] is None or b["peak_mb"] is None or peaks[k] > b["peak_mb"]:
                why.append("%s %s over %s MB" % (word, "unmeasured" if peaks[k] is None else "%.0f MB" % peaks[k], b["peak_mb"]))
        ok = not why
        results.append(("laya/" + fid, ok, "; ".join(why) or "place %s" % place))
        lines.append("%s laya %s decided_by=%s %s ms rss %s MB  place %s%s" % (
            "PASS" if ok else "FAIL", fid, by, "-" if slow is None else "%.0f" % (slow * 1000), "-" if peaks["rss"] is None else "%.0f" % peaks["rss"],
            place, "" if ok else "  (" + "; ".join(why) + ")"))
    results.append(("footprint-growth-under-LAYA_GROWTH_MAX_MB", None if grew is None or b["growth_mb"] is None else grew <= b["growth_mb"],
                    "grew %s MB from %s MB (want at most %s MB)" % ("-" if grew is None else "%.0f" % grew, start, b["growth_mb"])))
    why = answer.get("why")
    if chosen and isinstance(why, dict):      # a model of the list filed a mail: "Why here?" names it (LabelWhy.swift: "AI (<name>): yes")
        core, lines, name = why.get("core"), why.get("lines"), catalogue_name(chosen)
        judged = isinstance(core, dict) and "unreadable" not in core and isinstance(lines, list) and name is not None
        ok = judged and core.get("decided_by") == want_by and any(("AI (%s): yes" % name) in (l or "") for l in lines)
        results.append(("why-here-names-the-model", ok if judged else None,
                        "mail %s: the core says decided_by %r (want %s); the panel reads %r (want a line holding 'AI (%s): yes')" % (
                            why.get("message"), core.get("decided_by") if isinstance(core, dict) else core, want_by, lines, name)))
    bad_paths = [p for _, p, _ in served if p not in allowed]
    results.append(("the-server-answered-only-the-five-laya-files", None if not served else not bad_paths,
                    "%d requests, outside %s: %r" % (len(served), "the %d files of %s" % (len(allowed), chosen) if chosen else "the five", bad_paths)))
    lsof = answer.get("lsof") or {}
    remote = remote_addresses(lsof.get("text") or "")
    outside = [a for a in remote if not loopback(a)]
    results.append(("the-copy-reaches-no-address-outside-this-mac", None if not lsof.get("pids") else not outside,
                    "remote ends %r (outside: %r), lsof exit %r" % (sorted(set(remote)), outside, lsof.get("exit"))))
    in_box = sorted(fid for fid, f in filings.items() if isinstance(f, dict) and f.get("place") == LABEL)
    failed = sum(1 for _, ok, _ in results if ok is False)
    unjudged = sum(1 for _, ok, _ in results if ok is None)
    totals = ("TOTAL laya: %d messages, %d in Needs Reply %r, slowest step %s ms per message (bar %s s), peak rss %s MB and footprint %s MB "
              "(bar %s MB), footprint grew %s MB (bar %s MB); %d checks, %d failed, %d not judged") % (
        len(fixture_ids), len(in_box), in_box, "-" if slow is None else "%.0f" % (slow * 1000), b["seconds"],
        "-" if peaks["rss"] is None else "%.0f" % peaks["rss"], "-" if peaks["mb"] is None else "%.0f" % peaks["mb"], b["peak_mb"],
        "-" if grew is None else "%.0f" % grew, b["growth_mb"], len(results), failed, unjudged)
    return results, lines, totals


# ---------------------------------------------------------------- the lead's run

def catalogue_files(model):
    """The file names of a line of the model list, read from the catalogue's source (the line whose id is `model`)."""
    cat = open(os.path.join(ROOT, "Sources", "Features", "OnDeviceAI", "Core", "DecisionModelCatalogue.swift"), encoding="utf-8").read()
    at = cat.find('id: "%s",' % model)
    if at < 0:
        return None
    body = cat[at:]
    files = body[body.index("files: ["):]
    return re.findall(r'"([^"]+)"', files[:files.index("]")])


def catalogue_name(model):
    """The name the list shows for the line `model` (DecisionModelCatalogue.swift `name:`), or None when the catalogue has no such line."""
    cat = open(os.path.join(ROOT, "Sources", "Features", "OnDeviceAI", "Core", "DecisionModelCatalogue.swift"), encoding="utf-8").read()
    m = re.search(r'DecisionModelEntry\(\s*id: "%s",\s*name: "([^"]+)"' % re.escape(model), cat)
    return m.group(1) if m else None


def main_run(bundle, models, model=None):
    models = os.path.abspath(models)
    wanted = LAYA_FILES if not model else catalogue_files(model)
    if wanted is None:
        print("--model %s is not a line of the catalogue (DecisionModelCatalogue.swift)" % model)
        print("1 checks, 0 failed, 1 not judged"); return 1
    missing = [f for f in wanted if not os.path.isfile(os.path.join(models, f))]
    if missing:
        print("--models must hold the model's files at their paths; missing: %s" % ", ".join(missing))
        print("1 checks, 0 failed, 1 not judged"); return 1
    head, date, dirty = git_facts()
    print("commit %s; pass line %s committed %s" % (head, PASSLINE_REL, date))
    if date is None or dirty:
        print("REFUSED: the pass-line file %s" % ("has no commit" if date is None else "differs from its commit"))
        print("1 checks, 0 failed, 1 not judged"); return 1
    pl, fixture = passline(), fixture_messages()
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import realcore_mailfake as fake
    folder = tempfile.mkdtemp(prefix="laya-real-")
    server_log = os.path.join(folder, "server.log")
    try:
        mail, imap, smtp = fake.start(0, 0, inbox=0)
        server, port = serve(models, server_log)
    except OSError as e:
        print("the local servers did not start: %s" % e)
        print("1 checks, 0 failed, 1 not judged"); return 1
    mail.boxes["INBOX"] = [(i, raw_message(m, i, fake.ADDRESS)) for i, m in enumerate(fixture, 1)]
    mail.boxes["Sent"] = []
    extra = ["WHOOSH_RIG_HIDDEN=1", "WHOOSH_CORE_PLAIN_LOOPBACK=1", "WHOOSH_CORE_CHECK_SERVERS=%d:%d" % (imap.server_address[1], smtp.server_address[1]),
             "WHOOSH_RIG_LAYA_BASE=http://127.0.0.1:%d/" % port]
    if model:
        # this line's files come from the same loopback server. The copy runs the real engine, which serves no fixture verb, so the address
        # goes in at launch (DecisionModels.init reads the two names on a test copy only). main191: the verb was refused, nothing was staged,
        # and the download asked the test copy's refusing address (0 requests reached this server)
        extra += ["WHOOSH_RIG_MODEL_ID=%s" % model, "WHOOSH_RIG_MODEL_BASE=http://127.0.0.1:%d/" % port]
        # a line the catalogue holds back from the list (DecisionModelCatalogue.held: GLiDE, until the owner's word on its licence) is listed
        # on this test copy alone, so its real files can be run; for a line the list already shows the name changes nothing
        extra += ["WHOOSH_RIG_HELD_MODELS=%s" % model]
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(extra))
    if model:
        env.update(WHOOSH_REAL_MODEL=model, WHOOSH_REAL_MODEL_BASE="http://127.0.0.1:%d/" % port)
    env.pop("WHOOSH_FIXTURE_CORE", None)
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden",
           "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner', os.path.abspath(__file__)]
    t0 = time.time()
    try:
        r = subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True, text=True)
    finally:
        code = stop(server)
        imap.shutdown(); smtp.shutdown()
    served = served_paths(open(server_log, encoding="utf-8").read())
    print("launcher exit %d; run folder %s; %.0f s in all; the loopback server (pid %d, 127.0.0.1:%d) ended with %s and answered %d requests" % (
        r.returncode, folder, time.time() - t0, server.pid, port, code, len(served)))
    out = r.stdout + r.stderr
    line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
    if line is None:
        print(out.rstrip()[-3000:])
        print("1 checks, 0 failed, 1 not judged"); return 1
    answer = json.loads(line[len("ANSWER "):])
    results, lines, totals = judge(answer, pl, bars(pl), served, [m["id"] for m in fixture])
    print("LAYA-REAL: the fixture's 16 messages, the real weights of %s from %s served on 127.0.0.1, description %r; download %s s; re-run %s" % (
        model or "Laya", models, pl.get("description"), (answer.get("laya") or {}).get("seconds"), answer.get("rerun")))
    if model:
        print("model line %s; the core keeps model %r; staging answered %r" % (
            {k: (answer.get("model_line") or {}).get(k) for k in ("model", "state", "in_use", "facts")}, answer.get("model_kept"), answer.get("staged")))
    for n, ok, d in results:
        if not n.startswith("laya/"):
            print("%-10s laya-real/%s  %s" % ("PASS" if ok else ("NOT JUDGED" if ok is None else "FAIL"), n, d))
    print("\n".join(lines))
    print(totals)
    failed = sum(1 for _, ok, _ in results if ok is False)
    unjudged = sum(1 for _, ok, _ in results if ok is None)
    return 0 if failed == 0 and unjudged == 0 else 1


# ---------------------------------------------------------------- the self-test

def engine_rows(chosen="local", refusal=""):
    """The pane's `mac-settings-ai.decisions.engine` rows (SettingsAIMacFeature.swift:92-102) as the stand-ins serve them."""
    return [{"id": e, "chosen": e == chosen, "refusal": refusal if e == chosen else "", "labels_by": {"local": "laya", "jev": "jev"}.get(e, "none")}
            for e in ("local", "jev", "off")]


def stand_in_answer(fixture_ids, pl, in_box=None, by="laya", progress=None, mb=1100, rss=1150, start=180, lsof="", ids=True):
    in_box = set(pl["must-land"]) if in_box is None else set(in_box)
    filings = {f: {"decided_by": by if f in in_box else "default", "place": LABEL if f in in_box else "primary", "ai": "answered"} for f in fixture_ids}
    return {"engine": engine_rows(), "engine_after_ready": engine_rows(), "laya": {"state": "ready", "seconds": 40.0}, "meaning": pl["description"],
            "account_stage": "added", "arrival": {"counts": {"caught_by_entry": 16, "waiting_for_ai": 0}, "settled": True, "ai_refusal": None},
            "inspector": {"meaning": pl["description"], "offer": {"filed": 16}}, "before_rerun": {"ai_refusal": None, "candidates": 16, "preview": {"ai_words": None, "by_ai": 16}},
            "rerun_click": {"ok": True}, "rerun": {"state": "done", "done": 16, "total": 16}, "ids": {f: "m-" + f for f in fixture_ids} if ids else {},
            "progress": progress if progress is not None else [{"t": 3.0, "done": 10}, {"t": 4.5, "done": 16}],
            "samples": [{"t": 0, "mb": start, "rss": start + 20}, {"t": 1, "mb": mb, "rss": rss}, {"t": 2, "mb": mb - 50, "rss": rss - 40}],
            "filings": filings, "lsof": {"pids": [11], "exit": 0, "text": lsof or "Whoosh 11 u 5u IPv4 TCP 127.0.0.1:50001->127.0.0.1:61000 (ESTABLISHED)\n"},
            "errors": []}


class StandInApp:
    """A scripted control socket for `inner` (no app): the doors and state paths the real copy serves, with the answers main114 gave
    when `stale` is set (the arrival's Laya failure kept: flow/ai_refusal stands, rerun/candidates 0, the re-run stays idle)."""
    def __init__(self, fixture, description, stale=False, choose_takes=True, under_trust=False, moving=False):
        self.fixture, self.description, self.stale, self.choose_takes, self.under_trust = fixture, description, stale, choose_takes, under_trust
        self.moving, self.count_reads = moving, 0   # `moving`: the arrival's judging still files mails on the first reads (sorted_by_ai 1 to 5)
        self.clicks, self.typed, self.chosen, self.laya, self.stage, self.meaning, self.rerun_reads = [], "", "off", "absent", "entry", "", 0
        self.arrived_with_laya = None

    def click(self, ax):
        self.clicks.append(ax)
        if ax == SETTINGS + ".engine.local" and self.choose_takes:
            self.chosen = "local"
        elif ax == SETTINGS + ".local.download":
            self.laya = "ready"
        elif ax == ADD + ".add":
            self.stage, self.arrived_with_laya = "added", self.laya == "ready"
        elif ax == INS + ".meaning":
            self.meaning, self.typed = self.typed, ""
        elif ax == INS + ".rerun" and self.stale:
            pass
        return {"ok": True}

    def type(self, text):
        self.typed = text
        return {"ok": True}

    def rows(self, ax):
        if ax == ADD + ".state":
            return [{"stage": self.stage}]
        if ax == "mac-main.splits":
            return [{"id": "primary"}, {"id": "all"}]
        if ax == "mac-main.list":
            return [{"subject": m["subject"], "id": "m-" + m["id"]} for m in self.fixture] if self.stage == "added" else []
        if ax == SETTINGS + ".engine":
            return engine_rows(self.chosen)
        if ax == SETTINGS + ".local":
            return [{"state": self.laya, "words": ""}]
        if ax == INS:
            return [{"meaning": self.meaning, "status": None, "failure": None, "refusal": None, "offer": {"filed": 0 if self.stale else 16}, "foot": "offer"}]
        raise RuntimeError("no rows at %s" % ax)

    def core(self, path):
        refusal = {"code": "ai_no_model", "words": "Laya is not on this Mac yet."} if self.stale else None
        if self.under_trust:   # main115's own answers: Laya judged the 16 and was sure of none (Needs Reply has no step 1)
            refusal = {"code": "ai_under_trust", "highest_percent": 43, "judged": 16, "trust_percent": 80, "words": "The AI judged %d mails and was sure of none"}
        if path == "label/%s/flow/counts" % LABEL:
            self.count_reads += 1
            return {"caught_by_entry": 0 if self.under_trust else 16, "waiting_for_ai": 16 if self.stale else 0, "sorted_by_ai": min(self.count_reads, 5) if self.moving else 0}
        if path == "label/%s/flow/ai_refusal" % LABEL:
            return refusal
        if path == "label/%s/rerun/candidates" % LABEL:
            return 0 if self.stale else 16
        if path == "label/%s/rerun/preview" % LABEL:
            return {"by_ai": 0, "ai_words": "The AI cannot sort yet: no model on this Mac."} if self.stale else {"by_ai": 16, "ai_words": None}
        if path.startswith("label/%s/rerun/" % LABEL):
            leaf = path.rsplit("/", 1)[1]
            if self.stale:
                return {"state": "idle", "done": 0, "total": 0, "model_pending": True}.get(leaf, 0)
            if leaf == "state":
                self.rerun_reads += 1
            n = self.rerun_reads
            return {"state": "running" if n < 3 else "done", "done": 0 if n < 2 else (10 if n < 3 else 16), "total": 16, "model_pending": False}.get(leaf, 0)
        if path.startswith("message/"):
            fid = path.split("/")[1][2:]
            return json.dumps({"decided_by": "laya" if fid in ("msg-03", "msg-07") else "default", "place": LABEL if fid in ("msg-03", "msg-07") else "primary"})
        raise RuntimeError("no core value at %s" % path)


def run_stand_in(app):
    """`inner` against a stand-in, its ANSWER line read back. Short waits: the stand-in answers at once."""
    import contextlib
    import io
    buf = io.StringIO()
    n = iter(range(10 ** 6))
    with contextlib.redirect_stdout(buf):
        inner(c=app, run="", sampler=lambda: {"t": next(n), "mb": 300.0, "rss": 320.0},
              waits={"SYNC": 0.2, "READY": 0.2, "RERUN": 0.5, "DETECT": 0.2, "SETTLE": 0.3, "SETTLE_EVERY": 0.01, "SAMPLE": 0.01, "POLL": 0.01})
    line = next((l for l in buf.getvalue().splitlines() if l.startswith("ANSWER ")), None)
    return json.loads(line[len("ANSWER "):]) if line else None


def selftest():
    pl, fixture = passline(), fixture_messages()
    ids = [m["id"] for m in fixture]
    b = bars(pl)
    served_ok = [("GET", f, 200) for f in LAYA_FILES]
    verdict = lambda a, served=served_ok: judge(a, pl, b, served, ids)
    fails = lambda a, served=served_ok: [n for n, ok, _ in verdict(a, served)[0] if ok is False]
    unjudged = lambda a, served=served_ok: [n for n, ok, _ in verdict(a, served)[0] if ok is None]
    checks = {}
    # the pass line: its ids are the fixture's, the two lists apart, the description and the ledger's numbers read
    checks["the pass line"] = set(pl["must-land"]) <= set(ids) and set(pl["must-not-land"]) <= set(ids) and not set(pl["must-land"]) & set(pl["must-not-land"]) \
        and pl["description"] == "Asking a question from me" and b == {"seconds": 1, "peak_mb": 2200, "growth_mb": 1400}
    # the fixture's mail: 16 messages, distinct subjects, each raw message holds its subject and body
    raws = [raw_message(m, i, "owner@whoosh-check.invalid") for i, m in enumerate(fixture, 1)]
    checks["the fixture's mail"] = len(fixture) == 16 and len({m["subject"] for m in fixture}) == 16 \
        and b"Can you confirm the penalty cap by Monday?" in raws[2] and b"Subject: Can we move Thursday to 15:00?" in raws[6]
    good = stand_in_answer(ids, pl)
    res, lines, totals = verdict(good)
    checks["the stand-in run"] = all(ok for _, ok, _ in res) and len(lines) == 16 \
        and any(l.startswith("PASS laya msg-03 decided_by=laya 300 ms rss 1150 MB") for l in lines) and totals.endswith("0 failed, 0 not judged")
    # the step times: 10 messages in 3.0 s (0.30 s each, the load), 6 in 1.5 s (0.25 s each)
    st = step_seconds(good["progress"])
    checks["the step times"] = [round(s, 3) for s, _ in st] == [0.3, 0.25] and [n for _, n in st] == [10, 6]
    # the broken copies: each must turn the run red (or not judged where nothing can be judged)
    broken = {
        "a message in Needs Reply decided by another model": (stand_in_answer(ids, pl, by="apple"), "laya/msg-03"),
        "a must-land message left out": (stand_in_answer(ids, pl, in_box=["msg-03", "msg-05", "msg-12"]), "laya/msg-07"),
        "a must-not message filed": (stand_in_answer(ids, pl, in_box=pl["must-land"] + ["msg-04"]), "laya/msg-04"),
        "a slow step (1.5 s a message)": (stand_in_answer(ids, pl, progress=[{"t": 15.0, "done": 10}, {"t": 16.0, "done": 16}]), "laya/msg-01"),
        "a peak over RSS_MODEL_MAX_MB": (stand_in_answer(ids, pl, mb=2300, rss=2350), "laya/msg-01"),
        "a resident size over it alone": (stand_in_answer(ids, pl, mb=1500, rss=2250), "laya/msg-01"),
        "a growth over LAYA_GROWTH_MAX_MB": (stand_in_answer(ids, pl, mb=1700, rss=1800, start=200), "footprint-growth-under-LAYA_GROWTH_MAX_MB"),
        "a connection outside this Mac": (stand_in_answer(ids, pl, lsof="Whoosh 11 u 5u IPv4 TCP 192.168.1.4:50001->17.253.144.10:443 (ESTABLISHED)\n"),
                                          "the-copy-reaches-no-address-outside-this-mac"),
    }
    caught = {n: want in fails(a) for n, (a, want) in broken.items()}
    caught["a request for another file"] = "the-server-answered-only-the-five-laya-files" in fails(good, served_ok + [("GET", "secrets.txt", 404)])
    # another line of the model list: its filings carry its own name and the server gives its own files
    line = {"model": "knowline-0.8b", "state": "ready", "in_use": True}
    other_ok = [("GET", f, 200) for f in catalogue_files("knowline-0.8b") or []]
    why_ok = {"message": "msg-03", "core": {"ai": "yes", "card": "ai", "decided_by": "model:knowline-0.8b"}, "lines": ["Can you confirm Thursday?", "AI (%s): yes" % catalogue_name("knowline-0.8b")]}
    as_line = dict(stand_in_answer(ids, pl, by="model:knowline-0.8b"), model_line=line, why=why_ok)
    checks["another model's run"] = bool(other_ok) and not fails(as_line, other_ok) and not unjudged(as_line, other_ok)
    caught["another model's mail filed under Laya's name"] = "laya/msg-03" in fails(dict(stand_in_answer(ids, pl), model_line=line), other_ok)
    caught["Why here? says AI and no name"] = "why-here-names-the-model" in fails(dict(as_line, why=dict(why_ok, lines=[why_ok["lines"][0], "AI: yes"])), other_ok)
    caught["Why here? of a mail the core says Laya decided"] = "why-here-names-the-model" in fails(dict(as_line, why=dict(why_ok, core=dict(why_ok["core"], decided_by="laya"))), other_ok)
    caught["another model's server asked for a Laya file"] = "the-server-answered-only-the-five-laya-files" in fails(as_line, other_ok + [("GET", LAYA_FILES[0], 200)])
    never = dict(good, rerun={"state": "running", "done": 10, "total": 16})
    caught["a re-run that never ends is not judged"] = "step-7-the-re-run-ran" in unjudged(never) and "laya/msg-03" in unjudged(never) and not fails(never)
    nomodel = dict(good, laya={"state": "failed", "words": "Could not download the decision model."})
    caught["a download that failed is not judged"] = "step-2-laya-is-ready" in unjudged(nomodel)
    # round 15: main114's own answers: the engine read nothing, the re-run idle with total 0 and model_pending true; each step says what the app said
    main114 = dict(good, engine=None, engine_after_ready=None, before_rerun={"ai_refusal": {"code": "ai_no_model", "words": "Laya is not on this Mac yet."},
                                                                            "candidates": 0, "preview": None},
                   rerun={"state": "not done", "progress_last": {"t": 0.11, "done": 0, "total": 0, "state": "idle"}, "model_pending": True})
    m114 = dict((n, (ok, d)) for n, ok, d in verdict(main114)[0])
    caught["main114's run stops at the steps the app did not take, with its words"] = m114["step-1-laya-is-chosen"][0] is None \
        and m114["step-6-the-ai-can-answer-before-the-re-run"][0] is None and "Laya is not on this Mac yet." in m114["step-6-the-ai-can-answer-before-the-re-run"][1] \
        and "rerun/candidates 0" in m114["step-6-the-ai-can-answer-before-the-re-run"][1] and m114["step-7-the-re-run-ran"][0] is None \
        and m114["laya/msg-03"] == (None, "the run stopped at step-1-laya-is-chosen") and not fails(main114)
    other = dict(good, engine_after_ready=engine_rows("jev"))
    caught["another engine chosen is not judged"] = "step-1-laya-is-chosen" in unjudged(other) and "chosen ['jev']" in dict((n, d) for n, _, d in verdict(other)[0])["step-1-laya-is-chosen"]
    refused = dict(good, errors=["Re-run: rerun ['mac-labels.inspector.rerun']: mac-labels.inspector.rerun did nothing"], rerun_click=None)
    caught["a refused Re-run door is not judged, with its words"] = "step-7-the-re-run-ran" in unjudged(refused) and "the-doors-answered" in unjudged(refused)
    checks["the broken copies"] = all(caught.values())
    # round 15: the inner step itself, against a scripted socket: the doors in order (Laya ready before the account), the reads it records
    app = StandInApp(fixture, pl["description"])
    a_good = run_stand_in(app)
    order = app.clicks
    first = lambda ax: order.index(ax) if ax in order else 10 ** 6
    steps_good = dict((n, ok) for n, ok, _ in step_cases(a_good or {}, pl, ids))
    checks["the inner step on a stand-in app"] = a_good is not None and app.arrived_with_laya is True \
        and first(SETTINGS + ".engine.local") < first(SETTINGS + ".local.download") < first(ADD + ".open") < first(INS + ".apply") < first(INS + ".rerun") \
        and all(v is True for v in steps_good.values()) and a_good["rerun"].get("total") == 16 and len(a_good["filings"]) == 16 \
        and a_good["filings"]["msg-03"]["decided_by"] == "laya" and a_good["meaning"] == pl["description"]
    stale = StandInApp(fixture, pl["description"], stale=True)
    a_stale = run_stand_in(stale)
    s_stale = dict((n, (ok, d)) for n, ok, d in step_cases(a_stale or {}, pl, ids))
    lazy = StandInApp(fixture, pl["description"], choose_takes=False)
    s_lazy = dict((n, (ok, d)) for n, ok, d in step_cases(run_stand_in(lazy) or {}, pl, ids))
    # main115: no step 1 (caught_by_entry 0), an under-trust refusal, nothing waiting: steps 4 and 6 are taken and the run goes on
    s_trust = dict((n, (ok, d)) for n, ok, d in step_cases(run_stand_in(StandInApp(fixture, pl["description"], under_trust=True)) or {}, pl, ids))
    checks["the inner step on main115's answers"] = s_trust["step-4-the-arrival-judging-settled"][0] is True and s_trust["step-6-the-ai-can-answer-before-the-re-run"][0] is True \
        and s_trust["step-7-the-re-run-ran"][0] is True
    # the arrival still filing on its first reads: the step settles on the final counts only (the same reads in a row)
    a_moving = run_stand_in(StandInApp(fixture, pl["description"], moving=True)) or {}
    checks["the arrival settles on its final counts"] = ((a_moving.get("arrival") or {}).get("counts") or {}).get("sorted_by_ai") == 5 \
        and (a_moving.get("arrival") or {}).get("settled") is True
    checks["the inner step on a stale app"] = a_stale is not None and s_stale["step-4-the-arrival-judging-settled"][0] is None \
        and s_stale["step-6-the-ai-can-answer-before-the-re-run"][0] is None and "Laya is not on this Mac yet." in s_stale["step-6-the-ai-can-answer-before-the-re-run"][1] \
        and s_stale["step-7-the-re-run-ran"][0] is None and "model_pending" in s_stale["step-7-the-re-run-ran"][1] \
        and s_lazy["step-1-laya-is-chosen"][0] is None and "chosen ['off']" in s_lazy["step-1-laya-is-chosen"][1]
    # the parsers
    checks["the parsers"] = served_paths('127.0.0.1 - - [13/Nov/2026 05:00:00] "GET /typed-decisions/model.safetensors HTTP/1.1" 200 -\n'
                                         '127.0.0.1 - - [13/Nov/2026 05:00:01] "HEAD /typed-decisions/encoder/config.json?x=1 HTTP/1.1" 200 -') == \
        [("GET", "typed-decisions/model.safetensors", 200), ("HEAD", "typed-decisions/encoder/config.json", 200)] \
        and remote_addresses("a TCP 127.0.0.1:1->[::1]:993 (ESTABLISHED)\nb TCP *:5000 (LISTEN)\n") == ["[::1]:993"] \
        and loopback("[::1]:993") and loopback("127.0.0.1:80") and not loopback("10.0.0.2:443")
    # the loopback server, for real, on a stand-in folder of five small files: it answers on 127.0.0.1, its log names the file, its pid ends
    scratch = tempfile.mkdtemp(prefix="laya-real-selftest-")
    try:
        for f in LAYA_FILES:
            os.makedirs(os.path.dirname(os.path.join(scratch, "w", f)), exist_ok=True)
            with open(os.path.join(scratch, "w", f), "w", encoding="utf-8") as h:
                h.write("stand-in %s\n" % f)
        p, port = serve(os.path.join(scratch, "w"), os.path.join(scratch, "server.log"))
        got = urllib.request.urlopen("http://127.0.0.1:%d/%s" % (port, LAYA_FILES[1]), timeout=5).read()
        listen = subprocess.run(["lsof", "-a", "-p", str(p.pid), "-iTCP", "-sTCP:LISTEN", "-n", "-P"], capture_output=True, text=True).stdout
        code = stop(p)
        gone = subprocess.run(["ps", "-p", str(p.pid)], capture_output=True, text=True).returncode != 0
        log = open(os.path.join(scratch, "server.log"), encoding="utf-8").read()
        checks["the loopback server"] = got == ("stand-in %s\n" % LAYA_FILES[1]).encode() and ("127.0.0.1:%d" % port) in listen \
            and "*:%d" % port not in listen and served_paths(log) == [("GET", LAYA_FILES[1], 200)] and code is not None and gone
    except OSError as e:
        checks["the loopback server"] = False
        print("the loopback server: %s" % e)
    finally:
        shutil.rmtree(scratch, ignore_errors=True)
    print("selftest laya-real: " + ", ".join("%s %s" % (n, "ok" if v else "WRONG") for n, v in checks.items())
          + "; broken copies: " + ", ".join("%s %s" % (n, "caught" if v else "SURVIVES") for n, v in caught.items()))
    return 0 if all(checks.values()) else 1


def main():
    if "--selftest" in sys.argv:
        return selftest()
    if "--inner" in sys.argv:
        return inner()
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    if "--models" not in sys.argv or not args:
        print(__doc__)
        return 2
    models = sys.argv[sys.argv.index("--models") + 1]
    model = sys.argv[sys.argv.index("--model") + 1] if "--model" in sys.argv else None
    bundle = next(a for a in args if a not in (models, model))
    return main_run(bundle, models, model)


if __name__ == "__main__":
    sys.exit(main())
