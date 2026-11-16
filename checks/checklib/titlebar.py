"""Title-bar clearance (probes-r1, round 4): no control of a full-size-content window sits above the bottom of its title-bar band.

Reads the socket verb `winbands` (one row per titled window: id, title, band, fullsize, lights) and each window's
`<surface>.controls` rows, measured by `measure <ax>` (frames in the window's content coordinates, origin top-left).
A control whose top is above the band's bottom is an offender, by (band - top) points, unless it is a traffic light itself
(its centre lies inside one of the window's own measured light frames).
`mutant` is for the selftest only: ignore-window | band-zero | skip-under-band.
"""
import re

TOL = 0.5


def surface_of(win):
    """The surface id a window draws: its identifier or its title, whichever names a `mac-...` surface."""
    for t in (win.get("id", ""), win.get("title", "")):
        m = re.match(r"mac-[a-z0-9]+(?:-[a-z0-9]+)*", t)
        if m:
            return m.group(0)
    return None


def in_light(frame, lights):
    cx, cy = frame["x"] + frame["w"] / 2.0, frame["y"] + frame["h"] / 2.0
    return any(l["x"] <= cx <= l["x"] + l["w"] and l["y"] <= cy <= l["y"] + l["h"] for l in lights)


def offenders(windows, controls_of, measure, mutant=None):
    """-> (offenders [(window, ax, points)], unjudged [(window, why)]). controls_of(surface) -> rows or None; measure(ax) -> frame or None."""
    bad, unjudged = [], []
    for i, w in enumerate(windows):
        if not w.get("fullsize"):
            continue
        if mutant == "ignore-window" and i == 0:
            continue
        band = 0.0 if mutant == "band-zero" else float(w.get("band", 0))
        surface = surface_of(w)
        rows = controls_of(surface) if surface else None
        if rows is None:
            unjudged.append((w.get("title") or w.get("id"), "no `%s.controls` row is served" % surface if surface else "the window names no surface"))
            continue
        for r in rows:
            ax = r.get("ax")
            f = measure(ax) if ax else None
            if not f:
                continue
            if f["y"] < band - TOL and not in_light(f, w.get("lights", [])):
                if mutant == "skip-under-band":
                    continue
                bad.append((w.get("title") or w.get("id"), ax, round(band - f["y"], 1)))
    return bad, unjudged
