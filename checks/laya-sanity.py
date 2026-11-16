#!/usr/bin/env python3
"""laya-sanity: does the on-device decision model (Laya) tell an obvious "this label applies" from an obvious "it does not", in which
statement form, at which temperature, and does the meaning model's similarity separate them better?

Lane fix-aisort rounds 5 and 6. On the owner's Mac every per-label yes-probability sits between 0.4 and 0.64 (50 mails, 4 labels), so nothing
reaches MODEL_TRUST_LABEL. The lead read the checkpoint's settings (rl_agent_config.json): max_len 1024, head_max_len 256, temperature
noul:2 = 1.983, so the description is not cut and the temperature halves every score gap. Open: (S3) the statement's FORM, (S4) the temperature.

What it does: 12 synthetic mails written for this check (no real data) x 4 labels; each pair is marked obvious-yes, obvious-no or unclear.
Each mail is asked 4 statement FORMS per label in one call (16 yes/no questions): (a) today's `decide::label_statement`, (b) the description
turned into a claim about the mail, (c) the description alone, (d) a short generic claim. It launches the HIDDEN copy of the app
(scripts/launch-one.sh, --rig-hidden) with rig-only variables:
  WHOOSH_CORE_CHECK_LAYA=<cases.tsv>      the engine (core/crates/engine/src/check_laya.rs) asks the shell's Laya through the label flow's own
                                          port and state (decide::state_for); form (a) is built there by label_statement
  WHOOSH_CORE_CHECK_LAYA_OUT=<table.json> the yes-probability per (mail, form, label), then the cosine per (mail, label)
  WHOOSH_LAYA_TRACE=<trace.jsonl>         the Laya runtime (Sources/Features/OnDeviceAI/Core/LayaTrace.swift): per question, the token counts,
                                          the raw logits, the temperature; the probability at temperature 1.0 is computed here from the logits
  WHOOSH_RIG_EMBED_HANDOVER_DELAY_MS      with --weights: the shell waits before it looks for the meaning model, so the inner step can link it
Folders: --models <copy of Models/laya> (linked as <run>/Models/laya); --weights <copy of the meaning model's folder, the pinned
EmbeddingGemma 2 revision> (linked as <run>/models/embeddinggemma-2/<revision>; optional: without it the similarity table says so).

It prints, per form, the probability table with the temperature-1.0 answer beside each value, the three rules (every obvious-yes at or
above the trust level, every obvious-no under 0.5, every mail answered), the count of obvious-yes at or above the trust level at the
checkpoint's temperature and at 1.0, and the SEPARATION per label (the smallest obvious-yes minus the largest obvious-no: above 0 = a
threshold exists that files every obvious-yes and none of the obvious-no). Then the cosine table and its separation per label, and the
best of all per label.

    python3 checks/laya-sanity.py <bundle> --models <laya folder> [--weights <meaning model folder>]   one hidden launch (the lead)
    python3 checks/laya-sanity.py <bundle> --models <laya folder> --mutant   the same with WHOOSH_LAYA_MUTANT=keep-cache (rig only, LayaEngine.answer
                                                                            keeps MLX's cache): the memory case must go red (exit 0 when it did)
    python3 checks/laya-sanity.py <bundle> --models <laya folder> --judge   round 8: the label flow's OWN question through the shell's router with
                                                                           Apple's on-device model armed (WHOOSH_RIG_APPLE_JUDGE=real, rig only):
                                                                           at least JUDGE_MIN_YES of the 12 obvious-yes filed, no obvious-no filed,
                                                                           and every mail answered by Apple's model (checkpoint apple-on-device)
    python3 checks/laya-sanity.py --selftest                                                           launches nothing

Memory (round 7; the installed app reached 10 to 19 GB while it judged a batch): the inner step reads the hidden copy's physical footprint
(proc_pid_rusage, the method of perf/footprint.py `rusage`) every second, prints it after every mail (16 questions) and the peak, and judges
`laya-peak-under-RSS_MODEL_MAX_MB`: the peak while Laya judges (before the similarity part) under the ledger's RSS_MODEL_MAX_MB.
Round 13: and `laya-growth-under-LAYA_GROWTH_MAX_MB`: that peak minus the copy's first sample (taken before the first mail) under the
ledger's LAYA_GROWTH_MAX_MB. The kept cache stayed under the absolute bound on main111 (2181 MB) but added 2023 MB; the plain run added 944.

Expected time: 192 questions of about 150 tokens on ModernBERT-large plus loading it, and 16 embeddings plus loading the meaning model,
after a 30 s hand-over hold: about 2 to 4 minutes with the launch; the inner step gives up after WAIT_S.
The exit code follows form (a) (today's wording) and the run itself; the other tables are for reading.
"""
import json
import math
import os
import re
import subprocess
import sys
import tempfile
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WAIT_S = 1100     # the check's own bound for the whole inner step (no product meaning); the run must stay under 1200 s
NOT_UNDER = 0.5   # an obvious "no" must be a no: under one half
HANDOVER_HOLD_MS = 30000   # under HANDOVER_RIG_DELAY_MAX_MS (60000, docs/CONSTANTS.md): time to link the meaning model before the shell looks
EMBED_REVISION = "914f7f89142e33e77833254d9c9b90c3cef7303b"   # MlxModelSpec.embeddingGemma2Revision (Sources/Features/OnDeviceAI/Core/MlxModelSpec.swift:41)

LABELS = [
    ("needs-reply", "Needs Reply", "Emails where someone asks me a question or waits for my answer or action."),
    ("promotions", "Promotions", "Marketing emails: sales, discounts, special offers and advertising from shops and brands."),
    ("social", "Social", "Notifications from social networks: new followers, likes, comments, friend requests, mentions."),
    ("updates", "Updates", "Automatic notices: receipts, order and shipping confirmations, invoices, account and security alerts."),
]

# (id, from, subject, body, list header, {label: "yes" | "no" | "unclear"}); every mail is written for this check
MAILS = [
    ("m01", "anna.berg@client-example.com", "Can you confirm Thursday at 10?",
     "Hi, could you confirm whether Thursday at 10 works for the review meeting? I need your answer by tomorrow to book the room. Thanks, Anna", False,
     {"needs-reply": "yes", "promotions": "no", "social": "no", "updates": "no"}),
    ("m02", "deals@shop-example.com", "48 hours only: 40% off everything",
     "Our biggest sale of the year is on. Take 40% off every item with code SAVE40. Free delivery on orders over 50 euros. Shop now before it ends!", True,
     {"needs-reply": "no", "promotions": "yes", "social": "no", "updates": "no"}),
    ("m03", "notify@social-example.com", "Paul Martin started following you",
     "Paul Martin started following you. You have 3 new followers this week and 12 people liked your latest post. See who else follows you.", True,
     {"needs-reply": "no", "promotions": "no", "social": "yes", "updates": "no"}),
    ("m04", "orders@store-example.com", "Your order 48213 has shipped",
     "Good news: your order 48213 has shipped. Carrier: PostNL. Tracking number 3SXY12345. Expected delivery: Friday. This is an automatic message.", True,
     {"needs-reply": "no", "promotions": "no", "social": "no", "updates": "yes"}),
    ("m05", "jeroen@partner-example.nl", "Contract draft: your comments please",
     "Hello, attached is the second draft of the contract. Could you send me your comments before Monday? Let me know if anything is unclear.", False,
     {"needs-reply": "yes", "promotions": "no", "social": "no", "updates": "no"}),
    ("m06", "security@bank-example.com", "New sign-in to your account",
     "We noticed a new sign-in to your account from Safari on macOS in Brussels at 09:14. If this was you, no action is needed.", False,
     {"needs-reply": "no", "promotions": "no", "social": "no", "updates": "yes"}),
    ("m07", "newsletter@brand-example.com", "New collection: discover the autumn range",
     "Discover our new autumn collection. Members get 15% off and early access this weekend. Unsubscribe at any time.", True,
     {"needs-reply": "no", "promotions": "yes", "social": "no", "updates": "unclear"}),
    ("m08", "events@social-example.com", "Sophie commented on your photo",
     "Sophie wrote a comment on your photo: 'Great view!'. Reply to Sophie or see all 5 comments.", True,
     {"needs-reply": "unclear", "promotions": "no", "social": "yes", "updates": "no"}),
    ("m09", "billing@telecom-example.com", "Your invoice for October is ready",
     "Your invoice for October (32.50 euros) is ready. It will be paid by direct debit on 5 November. No action is needed.", True,
     {"needs-reply": "no", "promotions": "no", "social": "no", "updates": "yes"}),
    ("m10", "marc@friend-example.com", "Dinner on Saturday?",
     "Hey, are you free for dinner on Saturday evening? Let me know and I will book a table.", False,
     {"needs-reply": "yes", "promotions": "no", "social": "unclear", "updates": "no"}),
    ("m11", "info@airline-example.com", "Your boarding pass and a special upgrade offer",
     "Your boarding pass for flight XB123 is attached. Upgrade to business class today for only 89 euros.", True,
     {"needs-reply": "no", "promotions": "unclear", "social": "no", "updates": "yes"}),
    ("m12", "lisa@client-example.com", "Quick question about the invoice",
     "Hello, I see two lines for the same service on your last invoice. Could you check and tell me which one is right?", False,
     {"needs-reply": "yes", "promotions": "no", "social": "no", "updates": "unclear"}),
]


# the statement forms (b) to (d) per label, written for this check; (a) is built by the engine (decide::label_statement)
FORMS = {
    "b": {"needs-reply": "This email asks the reader a direct question or a request and waits for a reply.",
          "promotions": "This email advertises products or services: a sale, a discount or a special offer.",
          "social": "This email is a notification from a social network about followers, likes, comments or friend requests.",
          "updates": "This email is an automatic notice such as a receipt, an order or shipping confirmation, an invoice or a security alert."},
    "c": {i: d for i, _n, d in LABELS},
    "d": {"needs-reply": "This email needs a reply.", "promotions": "This email is a promotion or a newsletter.",
          "social": "This email comes from a social network.", "updates": "This email is an automatic notification."},
}
FORM_NAMES = {"a": "today: This email belongs under the label \"<path>\": <description>", "b": "the description as a claim about the mail",
              "c": "the description alone", "d": "a short generic claim"}
FORM_KEYS = ["a", "b", "c", "d"]


def trust_level():
    """MODEL_TRUST_LABEL from docs/CONSTANTS.md (a fraction; a value over 1 is a percentage)."""
    for line in open(os.path.join(ROOT, "docs", "CONSTANTS.md"), encoding="utf-8"):
        if line.strip().startswith("| MODEL_TRUST_LABEL |"):
            v = float(re.findall(r"[0-9.]+", line.split("|")[2])[0])
            return v / 100 if v > 1 else v
    raise SystemExit("docs/CONSTANTS.md has no MODEL_TRUST_LABEL row")


JUDGE_MIN_YES = 10           # the lead's bar for the judge (round 8): at least 10 of the 12 obvious-yes filed
APPLE_CHECKPOINT = "apple-on-device"   # Sources/Features/OnDeviceAI/Contract/AppleLabelJudge.swift LabelJudgeRouter.appleCheckpoint


def judge_cases(table, trust):
    """Round 8: the router's answers to the label flow's own per-label question (`labels/model#<label>`): a label is filed when its
    yes-probability reaches the trust level (the walk's own rule)."""
    rows = {r.get("mail"): r for r in (table.get("rows") or [])}
    filed_yes = filed_no = total_yes = 0
    not_apple = []
    for m, _f, _s, _b, _l, marks in MAILS:
        r = rows.get(m) or {}
        if r.get("checkpoint") != APPLE_CHECKPOINT:
            not_apple.append("%s:%s" % (m, r.get("checkpoint") or r.get("error") or "no answer"))
        yes = r.get("yes") or {}
        for i, mark in marks.items():
            p = yes.get("labels/model#" + i)
            filed = isinstance(p, (int, float)) and p >= trust
            total_yes += mark == "yes"
            filed_yes += mark == "yes" and filed
            filed_no += mark == "no" and filed
    return [("judge-every-mail-answered-by-apple", not not_apple, "mails not answered by Apple's model: %s" % (", ".join(not_apple) or "none")),
            ("judge-files-at-least-%d-obvious-yes" % JUDGE_MIN_YES, filed_yes >= JUDGE_MIN_YES, "%d of %d obvious-yes filed" % (filed_yes, total_yes)),
            ("judge-files-no-obvious-no", filed_no == 0, "%d obvious-no filed" % filed_no)]


def esc(v):
    return v.replace("\\", "\\\\").replace("\t", "\\t").replace("\n", "\\n")


def cases_tsv(judge=False):
    """The cases file core/crates/engine/src/check_laya.rs reads (labels, forms b to d, mails; `judge` = the label flow's own question only)."""
    lines = (["judge"] if judge else []) + ["\t".join(["label", i, n, esc(d)]) for i, n, d in LABELS]
    lines += ["\t".join(["form", k, i, esc(FORMS[k][i])]) for k in FORM_KEYS[1:] for i, _n, _d in LABELS]
    lines += ["\t".join(["mail", m, esc(f), esc(s), esc(b), "yes" if lst else "no"]) for m, f, s, b, lst, _ in MAILS]
    return "\n".join(lines) + "\n"


def read_cases(text):
    """The reader's side of `cases_tsv` (the engine splits on tabs and unescapes the same way), for the selftest."""
    un = lambda v: v.replace("\\n", "\n").replace("\\t", "\t")
    recs = [[un(x) for x in l.split("\t")] for l in text.splitlines() if l.strip()]
    return [r for r in recs if r[0] == "label"], [r for r in recs if r[0] == "form"], [r for r in recs if r[0] == "mail"]


def question_order():
    """The question ids in the order the engine asks them for each mail: form a per label, then the form records in file order."""
    return ["a#%s" % i for i, *_ in LABELS] + ["%s#%s" % (k, i) for k in FORM_KEYS[1:] for i, *_ in LABELS]


def t1_table(trace):
    """{mail: {question id: p(yes) at temperature 1.0}} from the trace's raw logits; the trace holds one line per question, mail after mail."""
    out, order, i = {}, question_order(), 0
    for m, *_ in MAILS:
        row = {}
        for qid in order:
            if i >= len(trace):
                return out
            t = trace[i]; i += 1
            z = t.get("logits") or []
            if len(z) == 2 and t.get("id", qid).endswith(qid):
                row[qid] = 1 / (1 + math.exp(z[0] - z[1]))
        out[m] = row
    return out


def scores(table, form):
    """{mail: {label: p}} for one form."""
    out = {}
    for r in table.get("rows") or []:
        yes = r.get("yes") or {}
        out[r.get("mail")] = {i: yes.get("%s#%s" % (form, i)) for i, *_ in LABELS if isinstance(yes.get("%s#%s" % (form, i)), (int, float))}
    return out


def separation(values):
    """{label: (min obvious-yes - max obvious-no, min yes, max no)} over {mail: {label: value}}; None where a side is empty."""
    out = {}
    for i, *_ in LABELS:
        ys = [values.get(m, {}).get(i) for m, *_r, marks in MAILS if marks.get(i) == "yes"]
        ns = [values.get(m, {}).get(i) for m, *_r, marks in MAILS if marks.get(i) == "no"]
        ys, ns = [v for v in ys if v is not None], [v for v in ns if v is not None]
        out[i] = (min(ys) - max(ns), min(ys), max(ns)) if ys and ns else None
    return out


def judge(table, trust, form="a"):
    """[(case, ok, detail)] for one form: per obvious pair, then whether every mail was answered. ok None = not judged."""
    rows = {r.get("mail"): r for r in (table.get("rows") or [])}
    out = []
    if table.get("error"):
        out.append(("the-run-reached-the-model", False, table["error"]))
    out.append(("every-mail-was-answered", table.get("done") is True and all(m in rows and "yes" in rows[m] for m, *_ in MAILS),
                "%d of %d mails answered%s" % (sum(1 for m, *_ in MAILS if "yes" in (rows.get(m) or {})), len(MAILS), "" if table.get("done") else ", the run did not end")))
    sc = scores(table, form)
    for m, _f, subject, _b, _l, marks in MAILS:
        for label, mark in marks.items():
            p = (sc.get(m) or {}).get(label)
            name = "%s/%s/%s" % (form, m, label)
            if mark == "unclear" or p is None:
                out.append((name, None, "%s %s, p %s" % (mark, subject, p)))
            elif mark == "yes":
                out.append((name, p >= trust, "obvious yes, p %.3f (want >= %.2f): %s" % (p, trust, subject)))
            else:
                out.append((name, p < NOT_UNDER, "obvious no, p %.3f (want < %.2f): %s" % (p, NOT_UNDER, subject)))
    return out


def yes_reaching(values, trust):
    return sum(1 for m, *_r, marks in MAILS for i, mk in marks.items() if mk == "yes" and (values.get(m) or {}).get(i, -1) >= trust)


def form_lines(table, t1, trust, form):
    sc = scores(table, form)
    t1f = {m: {i: v for i, v in ((q.split("#", 1)[1], p) for q, p in (t1.get(m) or {}).items() if q.startswith(form + "#"))} for m in t1}
    lines = ["FORM %s (%s): p at the checkpoint's temperature [p at temperature 1.0]" % (form, FORM_NAMES[form])]
    lines.append("  mail  " + "  ".join("%-22s" % i for i, *_ in LABELS))
    for m, _f, subject, _b, _l, marks in MAILS:
        cells = []
        for i, *_ in LABELS:
            p, q = (sc.get(m) or {}).get(i), (t1f.get(m) or {}).get(i)
            cells.append("%-22s" % ("%s %s [%s]" % ({"yes": "Y", "no": "n", "unclear": "?"}[marks[i]], "%.3f" % p if p is not None else "-", "%.3f" % q if q is not None else "-")))
        lines.append("  %s   %s  %s" % (m, "  ".join(cells), subject))
    sep = separation(sc)
    lines.append("  obvious-yes at or above %.2f: %d at the checkpoint's temperature, %d at temperature 1.0 (of %d)" % (
        trust, yes_reaching(sc, trust), yes_reaching(t1f, trust), sum(1 for *_r, mk in MAILS for v in mk.values() if v == "yes")))
    for i, *_ in LABELS:
        s = sep[i]
        lines.append("  separation %s/%s: %s" % (form, i, "%.3f (smallest yes %.3f, largest no %.3f)" % s if s else "-"))
    return lines


def similarity_lines(table):
    sim = table.get("similarity")
    if not isinstance(sim, dict):
        return ["SIMILARITY: not measured: %s" % (table.get("similarity_error") or "no similarity in the table")], {}
    lines = ["SIMILARITY: cosine between each mail (subject in the title, body as the text) and each label description (the query form); %.1f s to embed, %.1f s waited for the model" % (
        table.get("similarity_seconds") or 0, table.get("similarity_waited_seconds") or 0)]
    for m, _f, subject, _b, _l, marks in MAILS:
        lines.append("  %s   %s  %s" % (m, "  ".join("%-18s" % ("%s %.3f" % ({"yes": "Y", "no": "n", "unclear": "?"}[marks[i]], sim.get(m, {}).get(i, float("nan")))) for i, *_ in LABELS), subject))
    sep = separation(sim)
    for i, *_ in LABELS:
        lines.append("  separation similarity/%s: %s" % (i, "%.3f (smallest yes %.3f, largest no %.3f)" % sep[i] if sep[i] else "-"))
    return lines, sep


def trace_lines(trace):
    """One line per question of the trace, with the token counts and the raw margin."""
    out = []
    for t in trace:
        k = t.get("tokens") or {}
        z = t.get("logits") or []
        cut = []
        if k.get("head_kept", 0) < k.get("head_full", 0):
            cut.append("HEAD CUT %d of %d" % (k.get("head_kept", 0), k.get("head_full", 0)))
        if k.get("state_kept", 0) < k.get("state_full", 0):
            cut.append("STATE CUT %d of %d" % (k.get("state_kept", 0), k.get("state_full", 0)))
        out.append("trace %s: head %s/%s options %s state %s/%s total %s; logits %s margin %s; T %s (%s)%s" % (
            t.get("id"), k.get("head_kept"), k.get("head_full"), k.get("options_kept"), k.get("state_kept"), k.get("state_full"), k.get("total"),
            ["%.3f" % v for v in z], "%.3f" % (z[1] - z[0]) if len(z) == 2 else "-", t.get("temperature"), t.get("temperature_from"),
            ("  " + ", ".join(cut)) if cut else ""))
    return out


def rusage_footprint_mb(pid):
    """The physical footprint (MB) of one process from proc_pid_rusage flavor 2, word 9 (perf/footprint.py `rusage`), or None."""
    try:
        import ctypes
        lib, buf = ctypes.CDLL("/usr/lib/libproc.dylib"), (ctypes.c_uint64 * 64)()
        if lib.proc_pid_rusage(pid, 2, ctypes.byref(buf)) != 0:
            return None
        return buf[9] / 1048576.0
    except (OSError, AttributeError):
        return None


def app_footprint_mb(run):
    """The footprint summed over the hidden copy's processes (their executable lies under <run>/rig/<App>.app/Contents/MacOS/), or None."""
    rig = os.path.join(run, "rig")
    apps = [a for a in (os.listdir(rig) if os.path.isdir(rig) else []) if a.endswith(".app")]
    if not apps:
        return None
    prefix = os.path.join(rig, apps[0], "Contents", "MacOS") + "/"
    ps = subprocess.run(["ps", "-axo", "pid=,command="], capture_output=True, text=True).stdout
    pids = [int(l.split(None, 1)[0]) for l in ps.splitlines() if len(l.split(None, 1)) == 2 and l.split(None, 1)[1].startswith(prefix)]
    vals = [v for v in (rusage_footprint_mb(p) for p in pids) if v is not None]
    return sum(vals) if vals else None


def ledger_number(name):
    for line in open(os.path.join(ROOT, "docs", "CONSTANTS.md"), encoding="utf-8"):
        if line.strip().startswith("| %s |" % name):
            nums = re.findall(r"[0-9][0-9,.]*", line.split("|")[2])
            return float(nums[0].replace(",", "")) if nums else None
    return None


GROWTH_CASE = "laya-growth-under-LAYA_GROWTH_MAX_MB"


def start_mb(samples):
    """Round 13: the footprint before Laya: the first sample with a number, when it was taken before the first mail was judged; else None."""
    first = next((x for x in samples or [] if isinstance(x.get("mb"), (int, float))), None)
    return first["mb"] if first is not None and first.get("rows", 0) == 0 and not first.get("done") else None


def growth_case(peak, start, growth_mb):
    """Round 13: (case, ok, detail): what Laya added (its peak minus the first sample) under LAYA_GROWTH_MAX_MB."""
    if growth_mb is None:
        return (GROWTH_CASE, None, "docs/CONSTANTS.md holds no LAYA_GROWTH_MAX_MB")
    if start is None or peak is None:
        return (GROWTH_CASE, None, "no sample before the first mail, so no start to grow from")
    return (GROWTH_CASE, peak - start < growth_mb, "grew %.0f MB while Laya judged, from %.0f MB to %.0f MB (want < %d MB)" % (peak - start, start, peak, int(growth_mb)))


def memory_cases(memory, limit_mb, growth_mb=None):
    """[(case, ok, detail)] and the report lines from the inner step's samples: [{"t", "rows", "done", "mb"}]."""
    samples = [x for x in (memory or {}).get("samples") or [] if isinstance(x.get("mb"), (int, float))]
    lines = []
    if not samples:
        return [("laya-peak-under-RSS_MODEL_MAX_MB", None, "no footprint sample (the app's processes were not found)"),
                (GROWTH_CASE, None, "no footprint sample (the app's processes were not found)")], ["MEMORY: not measured"]
    per_mail = (memory or {}).get("questions_per_mail") or len(question_order())   # round 13: 4 in the mutant run (form a only), 16 otherwise
    n = len(MAILS)
    laya = []
    for x in samples:
        laya.append(x)
        if x.get("rows", 0) >= n or x.get("done"):
            break
    peak_laya, peak_all = max(x["mb"] for x in laya), max(x["mb"] for x in samples)
    lines.append("MEMORY: %d samples; peak while Laya judged %.0f MB, peak of the whole run %.0f MB (RSS_MODEL_MAX_MB %s)" % (len(samples), peak_laya, peak_all, limit_mb))
    seen = -1
    for x in samples:
        if x.get("rows", 0) != seen:
            seen = x.get("rows", 0)
            lines.append("  after %d mails (%d questions): %.0f MB at %.0f s" % (seen, seen * per_mail, x["mb"], x.get("t", 0)))
    grown = growth_case(peak_laya, start_mb((memory or {}).get("samples")), growth_mb)
    if limit_mb is None:
        return [("laya-peak-under-RSS_MODEL_MAX_MB", None, "docs/CONSTANTS.md holds no RSS_MODEL_MAX_MB"), grown], lines
    return [("laya-peak-under-RSS_MODEL_MAX_MB", peak_laya < limit_mb, "peak %.0f MB while Laya judged (want < %s MB)" % (peak_laya, int(limit_mb))), grown], lines


MUTANT_SAMPLE_S = 0.25   # round 10: the mutant run reads the footprint four times a second, so one sample past the bound is caught early
MUTANT_HARD_FACTOR = 2   # the copy is never let past 2 x RSS_MODEL_MAX_MB (the owner's "beware of ram"; the Mac's memory guard kills above 8 GB)


def app_pids(run):
    """The pids of the hidden copy's processes: their executable lies under <run>/rig/<App>.app/Contents/MacOS/ (the copy launch-one made)."""
    rig = os.path.join(run, "rig")
    apps = [a for a in (os.listdir(rig) if os.path.isdir(rig) else []) if a.endswith(".app")]
    if not apps:
        return []
    prefix = os.path.join(rig, apps[0], "Contents", "MacOS") + "/"
    ps = subprocess.run(["ps", "-axo", "pid=,command="], capture_output=True, text=True).stdout
    return [int(l.split(None, 1)[0]) for l in ps.splitlines() if len(l.split(None, 1)) == 2 and l.split(None, 1)[1].startswith(prefix)]


def end_copy(pids):
    """Ends the hidden copy BY ITS PIDS (never by name): TERM, then KILL for one still there after a second. Returns the pids ended."""
    import signal
    ended = []
    for pid in pids:
        try:
            os.kill(pid, signal.SIGTERM); ended.append(pid)
        except OSError:
            pass
    time.sleep(1)
    for pid in ended:
        try:
            os.kill(pid, signal.SIGKILL)
        except OSError:
            pass
    return ended


def mutant_stop(mb, limit_mb, start=None, growth_mb=None):
    """Round 10: the mutant run stops the copy at the first sample past the bound (the case is then red), and so always before twice it.
    Round 13: also at the first sample that grew LAYA_GROWTH_MAX_MB or more above the start. Returns the bar passed ("RSS_MODEL_MAX_MB",
    "LAYA_GROWTH_MAX_MB") or None."""
    if mb is None:
        return None
    if limit_mb is not None and mb > limit_mb:
        return "RSS_MODEL_MAX_MB"
    if start is not None and growth_mb is not None and mb - start >= growth_mb:
        return "LAYA_GROWTH_MAX_MB"
    return None


def count_lines(path):
    try:
        with open(path, encoding="utf-8") as f:
            return sum(1 for l in f if l.strip())
    except OSError:
        return 0


def mutant_cases_tsv():
    """Round 10: the production statement form only (no `form` records): 4 questions a mail, at most 48 for the 12 mails."""
    return "\n".join(l for l in cases_tsv().splitlines() if not l.startswith("form\t")) + "\n"


def mutant_verdict(memory, limit_mb, growth_mb=None):
    """(red, line): red when the copy passed RSS_MODEL_MAX_MB, or grew LAYA_GROWTH_MAX_MB above its start (round 13), with the cache
    kept (the runner then ended it by its pids)."""
    stopped = (memory or {}).get("stopped")
    samples = [x["mb"] for x in (memory or {}).get("samples") or [] if isinstance(x.get("mb"), (int, float))]
    peak = max(samples) if samples else None
    start = start_mb((memory or {}).get("samples"))
    if stopped:
        bar = stopped.get("bar") or "RSS_MODEL_MAX_MB"
        what = ("grew past LAYA_GROWTH_MAX_MB %d MB above its start %.0f MB" % (int(growth_mb), start)) if bar == "LAYA_GROWTH_MAX_MB" and growth_mb is not None and start is not None \
            else "passed RSS_MODEL_MAX_MB %d MB" % int(limit_mb)
        return True, "mutant keep-cache: RED as it must be (the cache kept, the footprint %s: peak %.0f MB after %d questions at %.0f s; the copy was ended by its pids %s)" % (
            what, stopped["mb"], stopped.get("questions") or 0, stopped.get("t") or 0, stopped.get("pids"))
    if peak is None:
        return False, "mutant keep-cache: NOT JUDGED (no footprint sample: the copy's processes were not found)"
    if start is None or growth_mb is None:
        return False, "mutant keep-cache: NOT JUDGED (the peak stayed at %.0f MB, under RSS_MODEL_MAX_MB %d MB, and %s)" % (
            peak, int(limit_mb), "no sample was taken before the first mail" if start is None else "docs/CONSTANTS.md holds no LAYA_GROWTH_MAX_MB")
    return False, "mutant keep-cache: SURVIVED: the peak stayed at %.0f MB, under RSS_MODEL_MAX_MB %d MB, and grew %.0f MB, under LAYA_GROWTH_MAX_MB %d MB, with the cache kept" % (
        peak, int(limit_mb), peak - start, int(growth_mb))


def inner(folder, models, weights, judge=False, mutant=False):
    """Inside launch-one: link the checkpoints where the rig copy looks for them, then hand the engine its cases, wait for the table."""
    run = os.environ.get("WHOOSH_SOCKET_DIR") or ""
    links = [(models, os.path.join(run, "Models", "laya"))]
    if weights != "-":
        links.append((weights, os.path.join(run, "models", "embeddinggemma-2", EMBED_REVISION)))
    for src, dest in links:
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        if not os.path.exists(dest):
            os.symlink(src, dest)
    with open(os.path.join(folder, "cases.tsv"), "w", encoding="utf-8") as f:
        # round 10: the mutant run asks the production form only (4 questions a mail, at most 48): enough to pass the bound, nothing more
        f.write(mutant_cases_tsv() if mutant else cases_tsv(judge))
    table, t0, samples, stopped = {}, time.time(), [], None
    limit_mb, growth_mb = ledger_number("RSS_MODEL_MAX_MB"), ledger_number("LAYA_GROWTH_MAX_MB")
    while time.time() - t0 < WAIT_S:
        try:
            with open(os.path.join(folder, "table.json"), encoding="utf-8") as f:
                table = json.load(f)
        except (OSError, ValueError):
            table = {}
        # round 7: the hidden copy's footprint every second, with how far the run is (rows = mails judged)
        samples.append({"t": round(time.time() - t0, 1), "rows": len(table.get("rows") or []), "done": bool(table.get("done")), "mb": app_footprint_mb(run)})
        bar = mutant_stop(samples[-1]["mb"], limit_mb, start_mb(samples), growth_mb) if mutant else None
        if bar:
            stopped = {"pids": end_copy(app_pids(run)), "mb": samples[-1]["mb"], "t": samples[-1]["t"], "questions": count_lines(os.path.join(folder, "trace.jsonl")), "bar": bar}
            break
        if table.get("done"):
            break
        time.sleep(MUTANT_SAMPLE_S if mutant else 1)
    trace = []
    try:
        with open(os.path.join(folder, "trace.jsonl"), encoding="utf-8") as f:
            trace = [json.loads(l) for l in f if l.strip()]
    except (OSError, ValueError) as e:
        trace = [{"unreadable": str(e)}]
    print("ANSWER " + json.dumps({"table": table, "trace": trace, "seconds": round(time.time() - t0, 1), "memory": {
        "samples": samples, "stopped": stopped, "questions_per_mail": len(LABELS) if (mutant or judge) else len(question_order())}}))
    return 0


def report(answer, trust, judge_mode=False):
    table, trace = answer.get("table") or {}, answer.get("trace") or []
    if judge_mode:
        results = judge_cases(table, trust) + memory_cases(answer.get("memory"), ledger_number("RSS_MODEL_MAX_MB"))[0][:1]   # the peak only: Apple's model is out of process
        lines = ["LAYA-SANITY --judge: the label flow's own per-label question through the shell's router, Apple's on-device model armed; trust level %.2f" % trust]
        lines += ["%-10s laya-sanity/%s  %s" % ("PASS" if ok else ("NOT JUDGED" if ok is None else "FAIL"), n, d) for n, ok, d in results]
        for r in table.get("rows") or []:
            lines.append("  %s %s %s ms  %s" % (r.get("mail"), r.get("checkpoint"), r.get("ms"), " ".join("%s=%s" % (k.split("#")[-1], v) for k, v in sorted((r.get("yes") or {}).items()))))
        failed = sum(1 for _, ok, _ in results if ok is False)
        lines.append("%d checks, %d failed, %d not judged" % (len(results), failed, sum(1 for _, ok, _ in results if ok is None)))
        return "\n".join(lines), failed
    t1 = t1_table(trace)
    lines = ["LAYA-SANITY: synthetic mails, the real checkpoint on the hidden copy, the label flow's own port and state; trust level %.2f; %s s in the inner step, %s s of Laya" % (
        trust, answer.get("seconds"), table.get("laya_seconds"))]
    results = judge(table, trust, "a")
    mem_results, mem_lines = memory_cases(answer.get("memory"), ledger_number("RSS_MODEL_MAX_MB"), ledger_number("LAYA_GROWTH_MAX_MB"))
    results += mem_results
    lines += ["%-10s laya-sanity/%s  %s" % ("PASS" if ok else ("NOT JUDGED" if ok is None else "FAIL"), n, d) for n, ok, d in results]
    lines += mem_lines
    best = {i: [] for i, *_ in LABELS}
    for form in FORM_KEYS:
        lines += form_lines(table, t1, trust, form)
        if form != "a":
            other = judge(table, trust, form)
            lines.append("  form %s by the three rules: %d pass, %d fail, %d not judged" % (form, sum(1 for _, ok, _ in other if ok), sum(1 for _, ok, _ in other if ok is False), sum(1 for _, ok, _ in other if ok is None)))
        for i, s in separation(scores(table, form)).items():
            if s:
                best[i].append((s[0], "form %s" % form))
    sim_lines, sim_sep = similarity_lines(table)
    lines += sim_lines
    for i, s in sim_sep.items():
        if s:
            best[i].append((s[0], "similarity"))
    for i, *_ in LABELS:
        ranked = sorted(best[i], reverse=True)
        lines.append("BEST SEPARATION %s: %s" % (i, ", ".join("%s %.3f" % (n, v) for v, n in ranked) if ranked else "-"))
    lines += trace_lines(trace)
    failed = sum(1 for _, ok, _ in results if ok is False)
    unjudged = sum(1 for _, ok, _ in results if ok is None)
    lines.append("%d checks, %d failed, %d not judged (form a, today's wording, and the memory peak; the unclear pairs are not judged)" % (len(results), failed, unjudged))
    return "\n".join(lines), failed


def main_run(bundle, models, weights, mutant=False, judge=False):
    models = os.path.abspath(models)
    if not os.path.isfile(os.path.join(models, "ready.json")):
        print("--models must be a copy of the owner's Models/laya folder (it holds ready.json): %s" % models)
        print("1 checks, 0 failed, 1 not judged"); return 1
    if weights and not os.path.isdir(weights):
        print("--weights must be the meaning model's folder (the pinned EmbeddingGemma 2 revision): %s" % weights)
        print("1 checks, 0 failed, 1 not judged"); return 1
    folder = tempfile.mkdtemp(prefix="laya-sanity-")
    extra = ["WHOOSH_RIG_HIDDEN=1", "WHOOSH_CORE_CHECK_LAYA=" + os.path.join(folder, "cases.tsv"),
             "WHOOSH_CORE_CHECK_LAYA_OUT=" + os.path.join(folder, "table.json"), "WHOOSH_LAYA_TRACE=" + os.path.join(folder, "trace.jsonl")]
    if weights:
        extra.append("WHOOSH_RIG_EMBED_HANDOVER_DELAY_MS=%d" % HANDOVER_HOLD_MS)
    if mutant:
        extra.append("WHOOSH_LAYA_MUTANT=keep-cache")
    if judge:
        extra.append("WHOOSH_RIG_APPLE_JUDGE=real")   # round 8: the router asks Apple's real on-device model (rig only; nothing leaves the Mac)   # round 7: LayaEngine.answer keeps MLX's cache (rig only)
    env = dict(os.environ, WHOOSH_RIG_HIDDEN="1", LAUNCH_ENV=" ".join(extra))
    env.pop("WHOOSH_FIXTURE_CORE", None)
    cmd = ["sh", os.path.join(ROOT, "scripts", "launch-one.sh"), bundle, "--", "--rig-hidden",
           "sh", "-c", 'WHOOSH_SOCKET_DIR="$D" python3 "$0" --inner "$1" "$2" "$3" "$4"', os.path.abspath(__file__), folder, models,
           os.path.abspath(weights) if weights else "-", "judge" if judge else ("mutant" if mutant else "forms")]
    t0 = time.time()
    r = subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True, text=True)
    out = r.stdout + r.stderr
    line = next((l for l in out.splitlines() if l.startswith("ANSWER ")), None)
    if line is None:
        print(out.rstrip()[-3000:])
        print("launcher exit %d" % r.returncode)
        print("1 checks, 0 failed, 1 not judged"); return 1
    answer = json.loads(line[len("ANSWER "):])
    text, failed = report(answer, trust_level(), judge)
    print("launcher exit %d; run folder %s; %.0f s in all%s" % (r.returncode, folder, time.time() - t0, "; MUTANT keep-cache" if mutant else ""))
    print(text)
    if mutant:
        # round 10: the copy was ended by its pids at the first sample past the bound (never past 2 x RSS_MODEL_MAX_MB)
        red, line = mutant_verdict(answer.get("memory"), ledger_number("RSS_MODEL_MAX_MB"), ledger_number("LAYA_GROWTH_MAX_MB"))
        print(line)
        return 0 if red else 1
    return 0 if failed == 0 else 1


def model_table(p_yes, p_no, p_unclear=0.5, forms=FORM_KEYS):
    rows = []
    for m, _f, _s, _b, _l, marks in MAILS:
        yes = {}
        for k in forms:
            for i, v in marks.items():
                yes["%s#%s" % (k, i)] = p_yes if v == "yes" else p_no if v == "no" else p_unclear
        rows.append({"mail": m, "ms": 300, "yes": yes})
    return {"done": True, "rows": rows}


def selftest():
    trust = 0.8
    verdict = lambda t, form="a": dict((n.split("/", 1)[-1] if n.startswith(form + "/") else n, ok) for n, ok, _ in judge(t, trust, form))
    good = verdict(model_table(0.93, 0.04))
    flat = verdict(model_table(0.58, 0.55))           # the owner's Mac: everything between 0.5 and 0.6
    half = dict(model_table(0.93, 0.04)); half["rows"] = half["rows"][:6]; half["done"] = False
    ok_good = all(v is not False for v in good.values())
    pairs = [n for n in flat if "/" in n]
    ok_flat = all(flat[n] is False for n in pairs if flat[n] is not None) and any(flat[n] is False for n in pairs)
    ok_half = verdict(half)["every-mail-was-answered"] is False
    ok_error = verdict({"done": True, "error": "no on-device decision model was handed to the core"})["the-run-reached-the-model"] is False
    # a form judged on its own keys: a table where only form b separates passes on b and fails on a
    mixed = model_table(0.58, 0.55, forms=["a"])
    for r, g in zip(mixed["rows"], model_table(0.93, 0.04, forms=["b"])["rows"]):
        r["yes"].update(g["yes"])
    ok_forms = any(v is False for v in verdict(mixed, "a").values()) and all(v is not False for v in verdict(mixed, "b").values())
    sep_good, sep_flat = separation(scores(model_table(0.93, 0.04), "b")), separation(scores(model_table(0.58, 0.55), "a"))
    ok_sep = all(s and abs(s[0] - 0.89) < 1e-9 for s in sep_good.values()) and all(s and abs(s[0] - 0.03) < 1e-9 for s in sep_flat.values())
    labels, forms, mails = read_cases(cases_tsv())
    ok_cases = [l[1] for l in labels] == [i for i, *_ in LABELS] and len(forms) == 12 and len(mails) == len(MAILS) and mails[0][4] == MAILS[0][3] and all(len(r) == 6 for r in mails)
    marks = [mk for *_, ms in MAILS for mk in ms.values()]
    ok_marks = len(MAILS) == 12 and len(LABELS) == 4 and all(set(ms) == {i for i, *_ in LABELS} for *_, ms in MAILS) and marks.count("yes") >= 12 and marks.count("no") >= 24
    # temperature 1.0 from the raw logits: a gap of 1.983 logits at T 1.983 is p 0.731; at 1.0 it is 0.879
    order = question_order()
    trace = [{"id": q, "logits": [0.0, 1.983], "temperature": 1.983, "temperature_from": "temperature_by_options",
              "tokens": {"head_full": 40, "head_kept": 40, "state_full": 100, "state_kept": 100, "total": 170}} for _ in MAILS for q in order]
    t1 = t1_table(trace)
    ok_t1 = len(t1) == 12 and all(abs(v - 1 / (1 + math.exp(-1.983))) < 1e-9 for row in t1.values() for v in row.values()) and len(t1["m01"]) == 16
    tl = trace_lines([dict(trace[0], tokens={"head_full": 40, "head_kept": 30, "state_full": 300, "state_kept": 300, "total": 355})])
    ok_trace = "HEAD CUT 30 of 40" in tl[0] and "margin 1.983" in tl[0]
    sim = {"done": True, "rows": [], "similarity": {m: {i: (0.62 if mk == "yes" else 0.31 if mk == "no" else 0.45) for i, mk in marks_.items()} for m, *_r, marks_ in MAILS}}
    sl, ssep = similarity_lines(sim)
    ok_sim = all(s and abs(s[0] - 0.31) < 1e-9 for s in ssep.values()) and similarity_lines({"done": True, "similarity_error": "x"})[0][0].startswith("SIMILARITY: not measured")
    text, failed = report({"table": model_table(0.93, 0.04), "trace": trace, "seconds": 1}, trust)
    ok_report = failed == 0 and "BEST SEPARATION needs-reply" in text and "FORM d" in text
    ok_trust = 0 < trust_level() <= 1
    # round 10: the safe mutant run: it stops at the first sample past the bound, asks at most 48 questions, and its verdict names the peak
    ok_stop = mutant_stop(2201, 2200) and not mutant_stop(2200, 2200) and not mutant_stop(None, 2200) and not mutant_stop(1500, None)
    _, m_forms, m_mails = read_cases(mutant_cases_tsv())
    ok_cap = m_forms == [] and len(m_mails) == 12 and len(LABELS) * len(m_mails) == 48
    red_line = mutant_verdict({"samples": [{"mb": 1700}, {"mb": 2350}], "stopped": {"mb": 2350, "t": 41.5, "questions": 9, "pids": [123]}}, 2200)
    survived = mutant_verdict({"samples": [{"mb": 1700}, {"mb": 1900}], "stopped": None}, 2200)
    ok_verdict = red_line[0] and red_line[1].startswith("mutant keep-cache: RED as it must be") and "peak 2350 MB after 9 questions" in red_line[1] \
        and mutant_verdict({"samples": []}, 2200)[0] is False
    survived = mutant_verdict({"samples": [{"rows": 0, "mb": 1700}, {"rows": 1, "mb": 1900}], "stopped": None}, 2200, 1400)
    ok_verdict = ok_verdict and survived == (False, "mutant keep-cache: SURVIVED: the peak stayed at 1900 MB, under RSS_MODEL_MAX_MB 2200 MB, and grew 200 MB, under LAYA_GROWTH_MAX_MB 1400 MB, with the cache kept") \
        and mutant_verdict({"samples": [{"rows": 2, "mb": 1900}], "stopped": None}, 2200, 1400)[1].startswith("mutant keep-cache: NOT JUDGED")
    # round 13: the second bar, on main111's own numbers (lead-laya-sanity.log lines 53 to 62, lead-laya-sanity-mutant.log lines 53 to 59):
    # the plain run (cache released) grew 944 MB and holds both bars; the kept cache stayed under 2200 MB (the round-10 survivor) and grew
    # 2023 MB, so it breaks LAYA_GROWTH_MAX_MB; the safe run ends it at the first sample past the growth bar, after 3 mails (12 questions)
    plain111 = {"samples": [{"t": 0, "rows": 0, "mb": 179}, {"t": 1, "rows": 1, "mb": 1033}, {"t": 2, "rows": 2, "mb": 1014}, {"t": 3, "rows": 3, "mb": 1041},
                            {"t": 4, "rows": 5, "mb": 1123}, {"t": 5, "rows": 7, "mb": 1100}, {"t": 6, "rows": 9, "mb": 1123}, {"t": 7, "rows": 11, "mb": 994},
                            {"t": 8, "rows": 12, "mb": 904}], "questions_per_mail": 16}
    mutant111 = {"samples": [{"t": 0, "rows": 0, "mb": 158}, {"t": 1, "rows": 3, "mb": 2061}, {"t": 1, "rows": 5, "mb": 1956}, {"t": 2, "rows": 8, "mb": 1975},
                             {"t": 2, "rows": 10, "mb": 2181}, {"t": 2, "rows": 12, "mb": 2176}], "questions_per_mail": 4}
    growth = ledger_number("LAYA_GROWTH_MAX_MB")
    g_plain, g_mutant = (dict((n, ok) for n, ok, _ in memory_cases(x, 2200, growth)[0]) for x in (plain111, mutant111))
    ok_growth = growth == 1400 and g_plain == {"laya-peak-under-RSS_MODEL_MAX_MB": True, GROWTH_CASE: True} \
        and g_mutant == {"laya-peak-under-RSS_MODEL_MAX_MB": True, GROWTH_CASE: False}
    first_stop = next(((x, mutant_stop(x["mb"], 2200, start_mb(mutant111["samples"][:i + 1]), growth)) for i, x in enumerate(mutant111["samples"])
                       if mutant_stop(x["mb"], 2200, start_mb(mutant111["samples"][:i + 1]), growth)), (None, None))
    ok_growth = ok_growth and first_stop[0] is mutant111["samples"][1] and first_stop[1] == "LAYA_GROWTH_MAX_MB" \
        and not any(mutant_stop(x["mb"], 2200, 179, growth) for x in plain111["samples"])
    ok_growth = ok_growth and mutant_stop(1558, 2200, 158, 1400) == "LAYA_GROWTH_MAX_MB" and mutant_stop(1557, 2200, 158, 1400) is None \
        and mutant_stop(2300, 2200, 2000, 1400) == "RSS_MODEL_MAX_MB" and mutant_stop(1600, 2200, None, 1400) is None
    grown_red = mutant_verdict(dict(mutant111, stopped={"mb": 2061, "t": 1, "questions": 12, "pids": [7], "bar": "LAYA_GROWTH_MAX_MB"}), 2200, growth)
    ok_growth = ok_growth and grown_red[0] and grown_red[1].startswith("mutant keep-cache: RED as it must be (the cache kept, the footprint grew past LAYA_GROWTH_MAX_MB 1400 MB above its start 158 MB: peak 2061 MB after 12 questions") \
        and any(l == "  after 3 mails (12 questions): 2061 MB at 1 s" for l in memory_cases(mutant111, 2200, growth)[1]) \
        and any(l == "  after 5 mails (80 questions): 1123 MB at 4 s" for l in memory_cases(plain111, 2200, growth)[1]) \
        and growth_case(1500, None, 1400)[1] is None and growth_case(1500, 100, None)[1] is None
    ok_mutant = ok_stop and ok_cap and ok_verdict and ok_growth
    # round 8: the judge's cases on a model of the router's answers (1.0 or 0.0): 10 of 12 obvious-yes filed and no obvious-no passes; a
    # fallback to Laya, a filed obvious-no, or only 9 obvious-yes, each fails its case
    def router(pick_ok=12, false_yes=0, checkpoint=APPLE_CHECKPOINT):
        rows, k, fy = [], 0, 0
        for m, _f, _s, _b, _l, marks in MAILS:
            yes = {}
            for i, mk in marks.items():
                filed = (mk == "yes" and k < pick_ok) or (mk == "no" and fy < false_yes)
                if mk == "yes": k += 1
                if mk == "no" and filed: fy += 1
                yes["labels/model#" + i] = 1.0 if filed else 0.0
            rows.append({"mail": m, "checkpoint": checkpoint, "yes": yes, "ms": 372})
        return {"done": True, "rows": rows}
    jv = lambda t: dict((n, ok) for n, ok, _ in judge_cases(t, trust))
    j_good, j_nine, j_noisy, j_laya = jv(router(10)), jv(router(9)), jv(router(12, 1)), jv(router(12, 0, "fallback:not_enabled"))
    ok_judge = all(j_good.values()) and j_nine["judge-files-at-least-10-obvious-yes"] is False and j_noisy["judge-files-no-obvious-no"] is False \
        and j_laya["judge-every-mail-answered-by-apple"] is False and read_cases(cases_tsv(True))[0] == read_cases(cases_tsv())[0] and cases_tsv(True).startswith("judge\n")
    # round 7: the memory case: a run under the bound passes, one over it fails, no sample is not judged; the peak counts the Laya phase only
    under = {"samples": [{"t": i, "rows": min(i, 12), "done": i > 13, "mb": 1500 + i} for i in range(15)] + [{"t": 20, "rows": 12, "done": True, "mb": 3500}]}
    over = {"samples": [{"t": i, "rows": min(i, 12), "done": i > 13, "mb": 1500 + 900 * i} for i in range(15)]}
    m_under, m_over, m_none = (dict((n, ok) for n, ok, _ in memory_cases(x, 2200)[0]) for x in (under, over, {"samples": [{"t": 0, "rows": 0, "mb": None}]}))
    ok_mem = m_under["laya-peak-under-RSS_MODEL_MAX_MB"] is True and m_over["laya-peak-under-RSS_MODEL_MAX_MB"] is False and m_none["laya-peak-under-RSS_MODEL_MAX_MB"] is None
    ok_mem = ok_mem and ledger_number("RSS_MODEL_MAX_MB") == 2200 and any("after 12 mails" in l for l in memory_cases(under, 2200)[1])
    ok_mem = ok_mem and m_none[GROWTH_CASE] is None and dict((n, ok) for n, ok, _ in memory_cases(under, 2200, 1400)[0])[GROWTH_CASE] is True
    print("selftest laya-sanity: model table %s, the owner's flat table %s, a run that stopped %s, an error %s, forms judged apart %s, separation %s, the cases file %s, the marks %s, temperature 1.0 %s, the trace reader %s, the similarity table %s, the report %s, the ledger's trust level %s" % (
        "all pass" if ok_good else "FAILS", "red" if ok_flat else "SURVIVES", "red" if ok_half else "SURVIVES", "red" if ok_error else "SURVIVES",
        "yes" if ok_forms else "NO", "computed" if ok_sep else "WRONG", "round-trips" if ok_cases else "WRONG", "12 mails x 4 labels" if ok_marks else "WRONG",
        "from the logits" if ok_t1 else "WRONG", "names the cut and the margin" if ok_trace else "WRONG", "read" if ok_sim else "WRONG",
        "complete" if ok_report else "WRONG", "read" if ok_trust else "WRONG") + ", the memory case %s" % ("under passes, over fails, none not judged" if ok_mem else "WRONG") + ", the judge's cases %s" % ("10 of 12 pass, 9, a filed no and a fallback fail" if ok_judge else "WRONG")
          + ", the safe mutant run %s" % ("stops past the bound, asks at most 48 questions, names the peak" if ok_mutant else "WRONG")
          + ", the growth bar %s" % ("main111's plain run holds it, main111's kept cache breaks it after 12 questions" if ok_growth else "WRONG"))
    return 0 if all([ok_good, ok_flat, ok_half, ok_error, ok_forms, ok_sep, ok_cases, ok_marks, ok_t1, ok_trace, ok_sim, ok_report, ok_trust, ok_mem, ok_judge, ok_mutant]) else 1


def main():
    if "--selftest" in sys.argv:
        return selftest()
    if "--inner" in sys.argv:
        i = sys.argv.index("--inner")
        mode = sys.argv[i + 4] if len(sys.argv) > i + 4 else "forms"
        return inner(sys.argv[i + 1], sys.argv[i + 2], sys.argv[i + 3], mode == "judge", mode == "mutant")
    if len(sys.argv) < 4 or "--models" not in sys.argv or not os.path.isdir(sys.argv[1]):
        print(__doc__); return 2
    weights = sys.argv[sys.argv.index("--weights") + 1] if "--weights" in sys.argv else None
    return main_run(os.path.abspath(sys.argv[1]), sys.argv[sys.argv.index("--models") + 1], weights, "--mutant" in sys.argv, "--judge" in sys.argv)


if __name__ == "__main__":
    sys.exit(main())
