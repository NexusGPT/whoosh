#!/usr/bin/env python3
"""The Whoosh control-socket client (docs/CONTROL-SOCKET.md). Standard library only.

    python3 tools/whoosh-ctl.py <verb> [args...]          one request, the JSON reply on stdout
    python3 tools/whoosh-ctl.py --script FILE             one request per line (verb args, shell-quoted), one reply per line

The socket is $WHOOSH_SOCKET_DIR/whoosh.sock; under scripts/launch-one.sh, $D is used when
WHOOSH_SOCKET_DIR is unset. Exit 0 when every reply says ok, 1 when one does not, 3 with no socket.
"""
import json
import os
import shlex
import socket
import sys


def connect():
    d = os.environ.get("WHOOSH_SOCKET_DIR") or os.environ.get("D") or ""
    path = os.path.join(d, "whoosh.sock") if d else ""
    if not path or not os.path.exists(path):
        print("no socket at %r (set WHOOSH_SOCKET_DIR, or run under scripts/launch-one.sh)" % path, file=sys.stderr)
        sys.exit(3)
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(10)
    s.connect(path)
    return s


def ask(s, buf, verb, args):
    s.sendall((json.dumps({"verb": verb, "args": args}) + "\n").encode())
    while b"\n" not in buf[0]:
        chunk = s.recv(65536)
        if not chunk:
            raise OSError("socket closed")
        buf[0] += chunk
    line, buf[0] = buf[0].split(b"\n", 1)
    return line.decode()


def main(argv):
    if not argv:
        print(__doc__)
        return 2
    reqs = []
    if argv[0] == "--script":
        for line in open(argv[1], encoding="utf-8"):
            words = shlex.split(line)
            if words and not words[0].startswith("#"):
                reqs.append(words)
    else:
        reqs.append(argv)
    s, buf, rc = connect(), [b""], 0
    for words in reqs:
        reply = ask(s, buf, words[0], words[1:])
        print(reply)
        if not json.loads(reply).get("ok"):
            rc = 1
    s.close()
    return rc


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
