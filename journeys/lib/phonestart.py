"""A test copy of the iPhone app always starts its FIRST launch (WelcomeTestCopy, PhoneRoot.launchIsDone: the welcome flow is what welcomephone-check judges), so
it opens on `iphone-welcome-accounts` even with the fixture's accounts. A check that is about the inbox leaves the welcome first, through the flow's own
controls (docs/JOURNEYS.md J9: scan, the code, Continue, Not now), and never through a test door."""
import time

WELCOME = "iphone-welcome-accounts"
# the flow's own steps (WelcomePhoneFlow): `scan` moves to the scanning step, the typed `fixture:valid` is the code as if the camera read it (a test copy), the
# accounts arrive, Continue goes to the notification step, Not now ends the flow
FLOW = [("click", "iphone-welcome.scan"), ("type", "fixture:valid"), ("click", "iphone-welcome.continue"), ("click", "iphone-welcome.not-now")]


def leave_welcome(ctl, secs=3.0):
    """No-op unless the app is on the welcome. Returns the surface it ended on (or None when there is no socket or it cannot be read)."""
    if ctl is None:
        return None
    try:
        surface = ctl.uistate().get("surface")
        if surface != WELCOME:
            return surface
        for kind, what in FLOW:
            (ctl.click if kind == "click" else ctl.type)(what)
        end = time.time() + secs
        while surface == WELCOME and time.time() < end:
            time.sleep(0.1)
            surface = ctl.uistate().get("surface")
        return surface
    except Exception:         # a build or stand-in without the flow: the check's own first read then says where the app is
        return None
