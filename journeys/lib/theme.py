"""The theme a journey runs in: WHOOSH_THEME=light|dark (default light), set through the test copy's `fixture theme` verb and then read back.

The same two lines journeys/J1.py:257-258 hold (`fixture theme`, then the `theme-applied` check on uistate["theme"]); J2, J4, J5 and J6 call
`apply(s)` in their first step so they behave as J1 does. `python3 journeys/run.py --themes light,dark` sets WHOOSH_THEME once per pass.
"""
import os

THEMES = ("light", "dark")


def current():
    """The theme this process was asked for. Read when the journey is loaded: run.py loads the module again for each theme."""
    return os.environ.get("WHOOSH_THEME", "light")


def title(base):
    return "%s (%s theme)" % (base, current())


def apply(s):
    """Set the theme through the test copy, then check the app shows it. One check: `theme-applied` (a verb the app refuses leaves it on light)."""
    want = current()
    s.raw("fixture", "theme", want)
    s.expect("theme-applied", lambda: s.ctl.uistate()["theme"], want)
