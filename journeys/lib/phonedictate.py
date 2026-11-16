"""The iPhone composer's dictation as the checks drive it: ONE sequence for checks/composerphone-check.py and journeys/J7.py (J7.4), so
the two can never drive it differently again (phone round 6: the journey set `fixture speech-model ready` while the model was downloading,
which cancels the download (PhoneSpeech.rigSet: `downloading?.cancel()`), so the session that waited for the model never listened and the
words went nowhere; composerphone-check let the download finish and passed on the same build).

Use, with the composer open:
    start(ctl)                          the model is missing, the Dictate tap starts its download (the caller reads the progress now)
    seen = finish(ctl, spoken)          waits for the model and for listening, speaks, taps Dictate, waits for the words
seen = {"model": state, "listening": state, "after": state}: the last value read of each wait ("ready", "listening", "text" when it worked).
"""
import time

DICT = "iphone-composer.dictate"
MODEL = "iphone-composer.speech-model"


def _wait(ctl, read, want, secs):
    """Reads until the value is `want` or `secs` are up; a stand-in that answers at once (`instant`) is read once."""
    got = read()
    end = time.monotonic() + secs
    while got != want and not getattr(ctl, "instant", False) and time.monotonic() < end:
        time.sleep(0.15)
        got = read()
    return got


def _state(ctl, ax):
    """The `state` of the first row of `ax`; None when the region has no row (a closed composer): the caller's case then reads None."""
    rows = ctl.rows(ax)
    return rows[0].get("state") if rows else None


def start(ctl):
    """The model is missing; the tap on Dictate starts its download (and the session listens once it lands)."""
    ctl.send("fixture", "speech-model", "missing")
    ctl.click(DICT)


def finish(ctl, spoken, secs=6.0):
    """After `start`: the download is left to land on its own (never `speech-model ready`, which cancels it), the session listens, the
    words are spoken, the second tap ends the session, the words arrive. Returns what each wait read last."""
    seen = {"model": _wait(ctl, lambda: _state(ctl, MODEL), "ready", secs)}
    seen["listening"] = _wait(ctl, lambda: _state(ctl, DICT), "listening", secs)
    ctl.send("fixture", "speak", spoken)
    ctl.click(DICT)
    seen["after"] = _wait(ctl, lambda: _state(ctl, DICT), "text", secs)
    return seen
