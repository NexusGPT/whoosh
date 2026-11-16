#!/usr/bin/env python3
"""Launch order check (launch-r1): the first frame is set before the core is read, the change subscription starts, the push
client starts and the notch feed attaches. Two judges, one verdict shape.

  python3 checks/launch-order-check.py                  reads the SOURCES (Sources/App/{AppModel,RootView}.swift and every
                                                        call of PushClientMac.start / NotchFeature.attach / CoreChanges.shared.start,
                                                        and (launch-r2) that the message-body web view is built behind BodyWebGate,
                                                        after the first frame)
  python3 checks/launch-order-check.py --trace FILE     reads a launch trace: the `launch-trace <phase> <ms>` lines a rig copy
                                                        writes to stderr (app.out in the run folder) when launched with
                                                        WHOOSH_LAUNCH_TRACE=1 in LAUNCH_ENV; judges the order and prints the phase table
  python3 checks/launch-order-check.py --selftest       the real sources must pass; each mutant (a copy with the order broken) and each
                                                        bad trace must fail on the case it breaks

Prints 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0 (a check that could not run is not green).
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
APPMODEL = "Sources/App/AppModel.swift"
ROOTVIEW = "Sources/App/RootView.swift"
MSGBODY = "Sources/Features/MessageBody/MessageBodyView.swift"
LATE = ("settled-refresh-end", "settled-changes-end", "settled-push-end", "settled-notch-end")


def body_of(text, anchor):
    """The brace-balanced body that opens after the first `anchor` in text, or None."""
    i = text.find(anchor)
    if i < 0:
        return None
    j = text.find("{", i)
    if j < 0:
        return None
    depth = 0
    for k in range(j, len(text)):
        depth += {"{": 1, "}": -1}.get(text[k], 0)
        if depth == 0:
            return text[j:k + 1]
    return None


def calls(sources, token):
    """(path, offset) of every call of `token` in the sources, comment lines skipped."""
    hits = []
    for path, text in sources.items():
        for m in re.finditer(re.escape(token), text):
            line_start = text.rfind("\n", 0, m.start()) + 1
            if text[line_start:m.start()].lstrip().startswith("//"):
                continue
            hits.append((path, m.start()))
    return hits


def judge_sources(sources):
    """list of (name, True=pass / False=fail / None=not judged, detail)."""
    out = []
    am, rv = sources.get(APPMODEL), sources.get(ROOTVIEW)
    if am is None or rv is None:
        return [("sources readable", None, "%s or %s not found" % (APPMODEL, ROOTVIEW))]
    init = body_of(am, "private init()")
    settle = body_of(am, "func settle()")
    after = body_of(rv, "static func afterFirstFrame()")
    appear = body_of(rv, ".onAppear")
    if None in (init, settle, after, appear):
        return [("bodies found", None, "init/settle/afterFirstFrame/onAppear not all found")]
    # 1. AppModel.init reads nothing and subscribes to nothing: its only settle() sits on the unit-test line.
    code = [l for l in init.splitlines() if not l.lstrip().startswith("//")]
    bad = [l for l in code if re.search(r"\brefresh\(\)|CoreChanges|PushClientMac|NotchFeature", l)]
    bad += [l for l in code if "settle()" in l and "XCTestConfigurationFilePath" not in l]
    out.append(("AppModel.init draws from an empty model", not bad, "; ".join(l.strip() for l in bad) or "init holds no read, no subscription"))
    # 2. the change subscription starts only inside settle(), after the first read
    subs = calls(sources, "CoreChanges.shared.start(")
    inside = [h for h in subs if h[0] == APPMODEL and settle in am and am.find(settle) <= h[1] <= am.find(settle) + len(settle)]
    out.append(("CoreChanges.start only inside settle()", len(subs) == len(inside) == 1 and am.find("refresh()", am.find(settle)) < am.find("CoreChanges.shared.start(")
                if settle else False, "%d call(s), %d inside settle()" % (len(subs), len(inside))))
    # 3. push and notch start only inside afterFirstFrame()
    for token in ("PushClientMac.start(", "NotchFeature.attach("):
        hits = calls(sources, token)
        inside = [h for h in hits if h[0] == ROOTVIEW and rv.find(after) <= h[1] <= rv.find(after) + len(after)]
        out.append(("%s only inside afterFirstFrame()" % token.rstrip("("), len(hits) == len(inside) >= 1, "%d call(s), %d inside afterFirstFrame()" % (len(hits), len(inside))))
    # 4. onAppear order: firstFrameMs set, then settle(), then afterFirstFrame() one turn later; no push/notch call before
    ff = appear.find("firstFrameMs = LaunchClock")
    st = appear.find("model.settle()")
    af = appear.find("AppLaunch.afterFirstFrame()")
    ordered = 0 <= ff < st < af
    out.append(("onAppear: firstFrameMs, then settle(), then afterFirstFrame()", ordered, "offsets firstFrameMs=%d settle=%d afterFirstFrame=%d" % (ff, st, af)))
    nested = af >= 0 and appear[:af].count("DispatchQueue.main.async") >= 2
    out.append(("afterFirstFrame() runs a main-queue turn after the first-frame turn", nested, "%d DispatchQueue.main.async before it (needs 2)" % appear[:af].count("DispatchQueue.main.async") if af >= 0 else "call not found"))
    out.append(judge_webview(sources))
    return out


def judge_webview(sources):
    """launch-r2, the one rule: the message-body web view is not built in the first layout. The only `BodyWebView(` call sits behind
    `if gate.isOpen`, the gate starts closed, the first body view arms it in onAppear, and it opens two main-queue turns later (so after
    the one turn that stamps firstFrameMs). Returns one (name, verdict, detail)."""
    name = "message-body web view is built after the first frame"
    t = sources.get(MSGBODY)
    if t is None:
        return (name, None, "%s not found" % MSGBODY)
    body, arm = body_of(t, "public var body: some View"), body_of(t, "func arm()")
    if body is None or arm is None or "final class BodyWebGate" not in t:
        return (name, None, "MessageBodyView.body, BodyWebGate or arm() not found")
    problems = []
    call, guard = body.find("BodyWebView("), re.search(r"if\s+gate\.isOpen\s*\{", body)
    if call < 0 or guard is None or guard.start() > call:
        problems.append("BodyWebView( is not behind `if gate.isOpen`")
    if len(calls(sources, "BodyWebView(")) != 1:
        problems.append("%d BodyWebView( calls (needs exactly 1)" % len(calls(sources, "BodyWebView(")))
    if not re.search(r"private\(set\) var isOpen = false", t):
        problems.append("the gate does not start closed")
    if not re.search(r"\.onAppear\s*\{[^}]*gate\.arm\(\)", body):
        problems.append("onAppear does not arm the gate")
    if arm.count("DispatchQueue.main.async") < 2 or "isOpen = true" not in arm:
        problems.append("the gate opens after %d main-queue turn(s) (needs 2)" % arm.count("DispatchQueue.main.async"))
    return (name, not problems, "; ".join(problems) or "web view behind the gate; gate closed at start, armed in onAppear, opens two turns later")


def parse_trace(text):
    ph = {}
    for line in text.splitlines():
        m = re.match(r"launch-trace (\S+) ([0-9.]+)\s*$", line.strip())
        if m and m.group(1) not in ph:
            ph[m.group(1)] = float(m.group(2))
    return ph


def judge_trace(text):
    ph = parse_trace(text)
    if "first-frame" not in ph:
        return [("trace holds first-frame", None, "no `launch-trace first-frame` line: launched without WHOOSH_LAUNCH_TRACE=1, or an old build")], ph
    out = []
    for p in LATE:
        if p not in ph:
            out.append(("%s after first-frame" % p, None, "phase missing from the trace"))
        else:
            out.append(("%s after first-frame" % p, ph[p] >= ph["first-frame"], "%s=%.1f ms, first-frame=%.1f ms" % (p, ph[p], ph["first-frame"])))
    if "settled-push-end" in ph and "settled-notch-end" in ph and "settled-refresh-end" in ph:
        out.append(("push and notch after the first read", ph["settled-push-end"] >= ph["settled-refresh-end"] and ph["settled-notch-end"] >= ph["settled-push-end"], "refresh<push<notch"))
    if "first-appear" in ph:
        out.append(("first-appear before first-frame", ph["first-appear"] <= ph["first-frame"], "%.1f <= %.1f" % (ph["first-appear"], ph["first-frame"])))
    return out, ph


def phase_table(ph):
    order = ["corehost-begin", "corehost-end", "appmodel-init-end", "first-appear", "control-socket-end", "first-frame"] + list(LATE)
    rows, prev = [], 0.0
    for p in sorted((p for p in ph), key=lambda p: ph[p]):
        rows.append("%-24s %9.1f ms   +%.1f" % (p, ph[p], ph[p] - prev)); prev = ph[p]
    return rows


def report(name, results):
    bad = [r for r in results if r[1] is False]
    nj = [r for r in results if r[1] is None]
    for n, v, d in results:
        print("%s: %-62s %s  (%s)" % (name, n, {True: "ok", False: "FAIL", None: "NOT JUDGED"}[v], d))
    print("%d checks, %d failed, %d not judged" % (len(results), len(bad), len(nj)))
    return 1 if bad or nj or not results else 0


def read_sources(root):
    out = {}
    for d, _, fs in os.walk(os.path.join(root, "Sources")):
        for f in fs:
            if f.endswith(".swift"):
                p = os.path.join(d, f)
                try:
                    out[os.path.relpath(p, root)] = open(p, encoding="utf-8").read()
                except OSError:
                    pass
    return out


GOOD_TRACE = """launch-trace corehost-begin 1200.0
launch-trace corehost-end 1300.0
launch-trace appmodel-init-end 1301.0
launch-trace first-appear 1500.0
launch-trace control-socket-end 1502.0
launch-trace first-frame 1510.0
launch-trace settled-refresh-end 1515.0
launch-trace settled-changes-end 1516.0
launch-trace settled-push-end 1530.0
launch-trace settled-notch-end 1540.0
"""


def selftest():
    src = read_sources(ROOT)
    cases = []   # (name, results, want_fail_substring or None)
    cases.append(("real sources", judge_sources(src), None))
    rv, am = src.get(ROOTVIEW, ""), src.get(APPMODEL, "")
    # mutant 1: the push client and the notch start first, in the first appearance (the order this lane removed)
    m1 = dict(src)
    m1[ROOTVIEW] = rv.replace('            LaunchClock.trace("first-appear")', '            LaunchClock.trace("first-appear")\n            AppLaunch.afterFirstFrame()', 1)
    cases.append(("mutant: afterFirstFrame called in the first appearance", judge_sources(m1), "onAppear: firstFrameMs"))
    # mutant 2: the push client started from AppModel.init
    m2 = dict(src)
    m2[APPMODEL] = am.replace('        LaunchClock.trace("appmodel-init-end")', '        _ = PushClientMac.start(core: host.core)\n        LaunchClock.trace("appmodel-init-end")', 1)
    cases.append(("mutant: PushClientMac.start in AppModel.init", judge_sources(m2), "PushClientMac.start only inside"))
    # mutant 3: the first read and the subscription back in init
    m3 = dict(src)
    m3[APPMODEL] = am.replace('        LaunchClock.trace("appmodel-init-end")', '        refresh()\n        CoreChanges.shared.start(core: host.core)\n        LaunchClock.trace("appmodel-init-end")', 1)
    cases.append(("mutant: refresh and CoreChanges.start in AppModel.init", judge_sources(m3), "AppModel.init draws"))
    # mutant 4: the notch attach in the same turn as the first frame (one turn too early)
    m4 = dict(src)
    m4[ROOTVIEW] = rv.replace("DispatchQueue.main.async { AppLaunch.afterFirstFrame() }", "AppLaunch.afterFirstFrame()", 1)
    cases.append(("mutant: afterFirstFrame in the first-frame turn", judge_sources(m4), "a main-queue turn after"))
    # launch-r2 mutants: the web view back in the first layout, a gate born open, no arm, a gate that opens in one turn
    bv = src.get(MSGBODY, "")
    for label, old, new in [("web view not behind the gate", "if gate.isOpen {", "if true {"),
                            ("gate born open", "private(set) var isOpen = false", "private(set) var isOpen = true"),
                            ("onAppear does not arm", "fadeIn(); gate.arm() }", "fadeIn() }"),
                            ("gate opens in one turn", 'DispatchQueue.main.async { DispatchQueue.main.async { self.isOpen = true; LaunchClock.mark("body-web-gate-open") } }',
                             'DispatchQueue.main.async { self.isOpen = true; LaunchClock.mark("body-web-gate-open") }')]:
        mw = dict(src)
        mw[MSGBODY] = bv.replace(old, new, 1)
        cases.append(("mutant: " + label, judge_sources(mw), "web view is built after the first frame" if old in bv else "MUTANT DID NOT APPLY"))
    # traces
    cases.append(("good trace", judge_trace(GOOD_TRACE)[0], None))
    bad = GOOD_TRACE.replace("launch-trace settled-push-end 1530.0", "launch-trace settled-push-end 1400.0")
    cases.append(("mutant trace: push client started before the first frame", judge_trace(bad)[0], "settled-push-end after first-frame"))
    cases.append(("no trace lines", judge_trace("nothing here\n")[0], "NOT JUDGED"))
    bad_n = 0
    for name, results, want in cases:
        failed = [r for r in results if r[1] is False]
        nj = [r for r in results if r[1] is None]
        if want is None:
            ok = not failed and not nj and bool(results)
        elif want == "NOT JUDGED":
            ok = bool(nj) and not failed
        else:
            ok = any(want in r[0] for r in failed)
        print("%-4s %-62s fails=%s" % ("ok" if ok else "BAD", name, [r[0] for r in failed] or ("not judged" if nj else "none")))
        bad_n += not ok
    print("%d checks, %d failed, 0 not judged" % (len(cases), bad_n))
    return 1 if bad_n else 0


def main(argv):
    if "--selftest" in argv:
        return selftest()
    if "--trace" in argv:
        path = argv[argv.index("--trace") + 1]
        try:
            text = open(path, encoding="utf-8", errors="replace").read()
        except OSError as e:
            print("launch-order: cannot read %s: %s" % (path, e)); print("0 checks, 0 failed, 1 not judged"); return 1
        results, ph = judge_trace(text)
        for row in phase_table(ph):
            print("launch-order: phase %s" % row)
        return report("launch-order", results)
    return report("launch-order", judge_sources(read_sources(ROOT)))


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
