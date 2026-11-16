# Checks on a private copy of the installed store

Each script runs inside `scripts/launch-one.sh` (hidden, real engine). It stops the hidden copy, puts a snapshot of the installed
store in the copy's own data folder, starts the copy again and drives it. It prints ids' ends, codes and counts only, never a word of
a mail. The hidden copy holds no password and reaches no mail server. The launcher removes the data when the run ends.

| Script | Pass line |
|---|---|
| `list-walk.py` | in All, after 90 presses of Down from the first conversation, the focused conversation is number 91 and the list holds more than 50 |
| `every-mail.py` | 150 conversations or more read in the reader; none shows page code, none shows under 40% of its expected characters, none is unpainted |
| `page-code.py` | the mails whose text part is page code show no code; with `P216_KEEP_DIR` set, the reader pane of mail ..144880 is kept for a look and must be deleted after |

Run (one at a time): `WHOOSH_RIG_HIDDEN=1 WHOOSH_BUNDLE=<app> WHOOSH_BUILD_LABEL=<label> LAUNCH_ENV="WHOOSH_RIG_HIDDEN=1" sh scripts/launch-one.sh <app> -- --rig-hidden sh -c 'WHOOSH_SOCKET_DIR="$D" python3 "$0"' checks/private-copy/<script>`
