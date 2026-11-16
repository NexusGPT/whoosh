#!/usr/bin/env python3
"""Account mark check: ix-account-mark, ix-accounts-section, ix-account-icon-picker (J1.1, J4.12). Sizes and limits from docs/CONSTANTS.md.

Drives the RUNNING rig copy over the control socket (docs/CONTROL-SOCKET.md) through journeys/lib; expected
values come from docs/design/fixtures/mac-main-sample.json and docs/CONSTANTS.md, never from a built screen.
Run: sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 checks/accountmark-check.py'
Prints the MUTANTS table then 'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0 (a check that could not run is not green).
"""
import os
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "journeys"))
from lib.harness import run_journey                                  # noqa: E402
from lib.ctl import Unreadable                                       # noqa: E402
from lib.fixture import FX, ROWS, ACCOUNT, MSG, const               # noqa: E402

ID = "accountmark"
TITLE = "Account mark: ring and icon"
TARGET = "LOCAL"
VISIBLE_OUTCOME = "each account has a ring in its colour at the minimum size; an icon over the limit is refused"
LIST = "mac-main.list"


def ids(s):
    return [r["id"] for r in s.ctl.rows(LIST)]


def ring_px(row):
    """The ring size of a list row. Judged only when `ring_source` is "row" (drawn) or "list" (copied from a drawn row);
    null source (emblems off) or no field is not judged, never passed."""
    if "ring_px" not in row:
        raise Unreadable("list rows serve no ring size (field ring_px absent; the emblem field names the tier %r)" % row.get("emblem"))
    if row.get("ring_source") not in ("row", "list"):
        raise Unreadable("ring_px is not judged: ring_source is %r (only \"row\" and \"list\" are judged)" % row.get("ring_source"))
    return row["ring_px"]


def drawn_rows(s, secs=3.0):
    """The list rows once their rings are measured: polled up to `secs` (a row draws after the list is laid out), the last read otherwise."""
    end = time.time() + secs
    rows = s.ctl.rows(LIST)
    while time.time() < end and not all(r.get("ring_source") in ("row", "list") for r in rows):
        time.sleep(0.1)
        rows = s.ctl.rows(LIST)
    return rows


def row_rings_at_ledger_size(rows):
    """True when at least one row is listed and every row's ring measures ACCOUNT_MARK_ROW_PX (within half a point)."""
    sizes = [ring_px(r) for r in rows]
    return len(sizes) > 0 and all(abs(p - const("ACCOUNT_MARK_ROW_PX")) <= 0.5 for p in sizes)


def drive(j):
    s = j.step("J1.1", "Rings")
    s.arrive(("click", "dock.whoosh"), "mac-main")
    # Emblems are a preference of the rig identity (whoosh.list.showEmblems in io.nexus.whoosh.rig, which the rig copy keeps between runs
    # and other cells flip), and with emblems hidden no ring is drawn, so `ring_source` is null and the ring checks are not judged. The
    # check sets the state it needs (the door is a toggle: only when it reads off) and proves it.
    if j.ctl is not None:
        try:
            if not j.ctl.uistate().get("show_emblems", True):
                s.click("mac-main.list.emblems")
        except Unreadable:
            pass
    s.expect("emblems-are-on", lambda: s.ctl.uistate().get("show_emblems"), True)
    s.expect("sidebar-ring-colour-per-account", lambda: [r["colour"] for r in s.ctl.rows("mac-main.account-book")], [a["colour"] for a in FX["accounts"]])   # sidebar rows carry no colour; the account book does (seams lane, sidebar lane)
    s.expect("sidebar-ring-min-px", lambda: min(r["ring_px"] for r in s.ctl.rows("mac-main.sidebar.accounts")) >= const("ACCOUNT_RING_MIN_PX"), True)
    s.expect("row-ring-colour-per-row", lambda: [r["account_colour"] for r in s.ctl.rows(LIST)], [ACCOUNT[m["account"]]["colour"] for m in ROWS])
    # A list row draws the emblem at ACCOUNT_MARK_ROW_PX (16, docs/CONSTANTS.md; EmblemSize.row), not at the sidebar's ACCOUNT_RING_MIN_PX (20): main58 measured
    # 16 on every row and the old expectation (>= 20) read red. Every drawn row is judged (a list, not a minimum over rows that hides a short one).
    s.expect("row-ring-min-px", lambda: row_rings_at_ledger_size(drawn_rows(s)), True)
    s.expect("rings-differ-across-accounts", lambda: len({r["account_colour"] for r in s.ctl.rows(LIST)}) > 1, True)
    s = j.step("J4.12", "Icon picker")
    # Served doors (Sources/Features/SettingsAccountsMac): click .open and .<account>.icon (SettingsAccountsMacFeature.swift clickTargets) open the
    # picker; measure .picker (AccountIconPicker.swift:41), .upload (:86) and .picker.done (:56) name its parts; rows .<account> carry `icon`
    # ("uploaded" once an image is kept) and `note` (the refusal in words). The picker's file button opens a system panel, which no cell drives, so
    # the bytes come by `fixture upload-icon <bytes>`: the socket accepts the verb (FixtureEvents.swift:28) but no screen consumes it and its reply
    # is {posted, args}. Asked of contracts: docs/lanes/accountmark.md "## Door asked of contracts (delta 15)". A door not served is NOT JUDGED
    # with its name, never a red for the app and never a pass.
    S = "mac-settings-accounts"
    acct = FX["accounts"][0]["id"]
    limit_mb = int(const("ACCOUNT_ICON_MAX_BYTES")) // 1048576   # the refusal says the limit in MB (AccountsWords.swift:39)

    def upload(n):
        s.ctl.measure(S + ".picker")          # the picker is open for the account (else not judged: no frame)
        reply = s.ctl.send("fixture", "upload-icon", str(n))
        if "accepted" not in reply:
            raise Unreadable("not judged: door fixture upload-icon not served (the reply is %s: no `accepted`; no screen consumes the verb)" % sorted(k for k in reply if k != "ok"))
        return reply

    def stored_px():
        rows = s.ctl.rows(S + ".icon.stored")
        if not rows:
            raise Unreadable("not judged: door rows %s.icon.stored not served (no rows)" % S)
        return rows[0]["px"]

    s.click(S + ".open")
    s.click("%s.%s.icon" % (S, acct))
    s.expect("icon-over-limit-refused", lambda: upload(const("ACCOUNT_ICON_MAX_BYTES") + 1)["accepted"], False)
    s.expect("refusal-names-the-limit", lambda: (upload(const("ACCOUNT_ICON_MAX_BYTES") + 1), "%d MB" % limit_mb in s.ctl.rows("%s.%s" % (S, acct))[0]["note"])[1], True)
    s.expect("icon-at-limit-accepted", lambda: upload(const("ACCOUNT_ICON_MAX_BYTES"))["accepted"], True)
    s.expect("stored-icon-px", stored_px, const("ACCOUNT_ICON_PX"))

    def stored_file_and_ref():
        """What the upload at the limit left behind: the core's reference for the account and the stored file (a row exists only while the app can read the file)."""
        ref = s.ctl.core("account_icon/%s" % acct)
        rows = [r for r in s.ctl.rows(S + ".icon.stored") if r.get("account") == acct]
        if not rows:
            return ref, None
        return ref, (rows[0].get("pixels"), rows[0].get("pixels_high"), rows[0].get("bytes", 0) > 0)

    # The file is kept as a square at ACCOUNT_ICON_PX x 2 (AccountIconStore.store crops to a square): the row serves the width (`pixels`) and the height (`pixels_high`), both judged here.
    side = int(const("ACCOUNT_ICON_PX")) * 2
    s.expect("upload-stores-the-file-and-writes-the-ref", stored_file_and_ref, ("uploaded", (side, side, True)))


# MUTANTS: per check, the mutation of the app that must turn it red.
# J1.1/arrives-by-control | the Dock launch opens another surface
# J1.1/emblems-are-on | the emblems door does nothing, or the emblems stay hidden (no ring is drawn)
# J1.1/sidebar-ring-colour-per-account | every sidebar ring has the same colour
# J1.1/sidebar-ring-min-px | the sidebar ring is 12 px
# J1.1/row-ring-colour-per-row | row rings ignore the account colour
# J1.1/row-ring-min-px | the row ring is 4 px
# J1.1/rings-differ-across-accounts | all rings are one colour
# J4.12/icon-over-limit-refused | an oversize icon is accepted
# J4.12/refusal-names-the-limit | the refusal omits the limit
# J4.12/icon-at-limit-accepted | an icon exactly at the limit is refused
# J4.12/stored-icon-px | the stored icon keeps its upload size
# J4.12/upload-stores-the-file-and-writes-the-ref | the upload is kept but the core is not told (account_icon stays a glyph), or no file is kept, or the kept file is not square


def main():
    path = os.path.abspath(__file__)
    r = run_journey(sys.modules[__name__], path, force_dry="--dry" in sys.argv)
    rows = [l[1:].strip().split(" | ", 1) for l in open(path, encoding="utf-8") if l.startswith("# J") and " | " in l]
    print("MUTANTS (a check is green only after it has been seen red on its mutant)")
    print("| check | mutant |")
    print("|---|---|")
    for k, v in rows:
        print("| %s | %s |" % (k, v))
    for line in r["fail_list"]:
        sys.stderr.write("FAIL %s\n" % line)
    for line in r["no_mutant"]:
        sys.stderr.write("NO MUTANT %s\n" % line)
    for line in r["stray_mutant"]:
        sys.stderr.write("STRAY MUTANT %s\n" % line)
    sys.stderr.write("%s: %s\n" % (r["verdict"], r["evidence"]))
    print("%d checks, %d failed, %d not judged" % (r["checks"], r["failed"], r["unjudged"]))
    ok = r["failed"] == 0 and r["unjudged"] == 0 and r["checks"] > 0 and not r["no_mutant"] and not r["stray_mutant"]
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
