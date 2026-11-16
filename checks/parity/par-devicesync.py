#!/usr/bin/env python3
"""par-devicesync: the GotMail parity rows "Sync between Macs" and "Sync retry without network" (research/gotmail.md:65,125),
plus "Sync with iPhone" and "Accounts to iPhone by QR" as the parity group sees them.

    python3 checks/parity/par-devicesync.py            run the parity checks; exit 1 when F > 0 or U > 0
    python3 checks/parity/par-devicesync.py --selftest every check must go RED on its mutant

Shares the harness of checks/devicesync-check.py (same fake container, same driver, mode "parity")."""
import importlib.util
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("devicesync_check", os.path.join(os.path.dirname(HERE), "devicesync-check.py"))
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)


def main():
    if "--selftest" in sys.argv:
        return base.selftest(base.PARITY_IDS, "parity", "parity")
    return base.report(base.run_group(base.PARITY_IDS, "parity", with_static=False))


if __name__ == "__main__":
    sys.exit(main())
