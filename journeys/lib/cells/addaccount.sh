#!/bin/sh
# round: rig
# Manifest cell addaccount: the add-account flow (the sheet that replaced the Welcome address column) against its lane's own
# parity check. checks/welcomemac-check.py drives the retired Welcome UI and is no longer a cell.
python3 checks/parity/par-addaccount.py 2>&1
