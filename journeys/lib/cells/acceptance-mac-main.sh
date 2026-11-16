#!/bin/sh
# round: rig
# Manifest cell acceptance-mac-main: acceptance/mac-main.md must hold the owner's signed J1 words VERBATIM (read from
# docs/JOURNEYS.md, so they cannot drift) and name its driven check; then the cell's verdict is that check's: the J1 run.
. "$(dirname "$0")/_sum.sh"
[ -f acceptance/mac-main.md ] || unjudged "acceptance/mac-main.md does not exist: the owner's words for mac-main"
python3 - <<'PY' || { emit 1 1 0; exit 1; }
import re, sys
doc = open("docs/JOURNEYS.md", encoding="utf-8").read()
acc = open("acceptance/mac-main.md", encoding="utf-8").read()
j1 = doc[doc.index("## J1:"):doc.index("## J2:")]
trigger = next(l for l in j1.splitlines() if l.startswith("Trigger:"))
bad = []
if trigger not in acc: bad.append("the J1 trigger and done-when are not quoted verbatim")
if "journeys/J1.py" not in acc: bad.append("the driven check journeys/J1.py is not named")
if bad:
    for b in bad: print("FAIL acceptance/mac-main.md: " + b)
    sys.exit(1)
PY
python3 journeys/run.py J1 2>&1
