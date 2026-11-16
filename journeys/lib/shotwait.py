"""How long a check's socket must wait for the reply of `shot` / `capture shot`.

The hidden `shot` verb waits for the window's motion (bound B) and for the reader's page (bound B again) before it asks WebKit for the picture (snapshotTimeoutS), with
B = settleBaseS * (load per core, at most settleMaxLoadFactor) (Sources/App/WindowShotWeb.swift). The longest answer is therefore 2*B_max + motionS + snapshotTimeoutS plus the
cover readers' own turns; a margin covers the render. The numbers are READ from that source file, never copied: a change there moves every caller. If the file or a number is
missing the answer is a long fixed fallback, never a short one.
"""
import os
import re

SOURCE = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "Sources", "App", "WindowShotWeb.swift")
MARGIN_S = 10.0
FALLBACK_S = 90.0
SHOT_VERBS = ("shot", "capture")


def shot_reply_wait_s():
    try:
        text = open(SOURCE, encoding="utf-8").read()
        num = lambda name: float(re.search(r"%s: Double = ([0-9.]+)" % name, text).group(1))
        base, factor, motion, snap = num("settleBaseS"), num("settleMaxLoadFactor"), num("motionS"), num("snapshotTimeoutS")
    except (OSError, AttributeError, ValueError):
        return FALLBACK_S
    return 2 * base * factor + motion + snap + MARGIN_S


def timeout_for(verb, args, base_s):
    """The socket timeout for one request: the base, or the shot wait when the verb is `shot` or `capture shot`."""
    if verb == "shot" or (verb == "capture" and args and str(args[0]) == "shot"):
        return max(base_s, shot_reply_wait_s())
    return base_s


def selftest():
    """The wait covers the verb's longest answer (read from the source), is read from it (a model source with other numbers moves it) and a plain verb keeps its own timeout."""
    import tempfile
    global SOURCE
    real = shot_reply_wait_s()
    ok = real > 2 * 3 * 6 + 0.25 + 3
    ok = ok and timeout_for("rows", ("x",), 5) == 5 and timeout_for("shot", ("n",), 5) == real and timeout_for("capture", ("shot", "n"), 5) == real
    saved = SOURCE
    with tempfile.TemporaryDirectory() as d:
        SOURCE = os.path.join(d, "m.swift")
        open(SOURCE, "w").write("settleBaseS: Double = 4\nsettleMaxLoadFactor: Double = 10\nmotionS: Double = 1\nsnapshotTimeoutS: Double = 2\n")
        moved = shot_reply_wait_s() == 2 * 4 * 10 + 1 + 2 + MARGIN_S
        SOURCE = os.path.join(d, "none.swift")
        fallback = shot_reply_wait_s() == FALLBACK_S
    SOURCE = saved
    print("shotwait selftest: wait %.2f s (covers 2*3*6+0.25+3), plain verbs unchanged, read from the source %s, long fallback %s" % (real, moved, fallback))
    return 0 if ok and moved and fallback else 1


if __name__ == "__main__":
    import sys
    sys.exit(selftest())
