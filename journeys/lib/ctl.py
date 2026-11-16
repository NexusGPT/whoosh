"""Control socket client. Standard library only.

Wire contract (planned verbs; docs/CONTROL-SOCKET.md replaces this once it exists
in the repo, and only this file then changes):
  socket   : $WHOOSH_SOCKET_DIR/whoosh.sock, a Unix stream socket
  request  : one JSON line  {"verb": "<verb>", "args": [...]}
  reply    : one JSON line  {"ok": true, ...} or {"ok": false, "error": "<words>"}
  actions  : type <text> | key <chord> | click <ax-id>
  asserts  : rows <ax-id> | uistate | winframes | measure <ax-id> | core state <path>
  capture  : capture shot <name>
Reply fields the journeys read (one place to adapt):
  rows     -> {"rows": [{"ax","sender","subject","account","unread","pinned","chips"}]}
  uistate  -> {"state": {"surface","tab","counts","focus","overlay","theme", ...}}
  measure  -> {"ms": <number>} or {"frame": {"x","y","w","h"}}
  core     -> {"value": <any>}
"""
import json
import os
import socket

TIMEOUT_S = 5.0
from . import shotwait


class CouldNotDrive(Exception):
    """No socket, or it refused: nothing can be judged."""


class Unreadable(Exception):
    """One read or action failed: the check that needed it is not judged."""


class Ctl:
    def __init__(self, sock_dir=None):
        self.dir = sock_dir if sock_dir is not None else os.environ.get("WHOOSH_SOCKET_DIR", "")
        self.path = os.path.join(self.dir, "whoosh.sock") if self.dir else ""
        self.sock = None
        self.buf = b""

    def connect(self):
        if not self.dir:
            raise CouldNotDrive("WHOOSH_SOCKET_DIR is not set")
        if not os.path.exists(self.path):
            raise CouldNotDrive("no socket at %s (the app is not running)" % self.path)
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.settimeout(TIMEOUT_S)
        try:
            s.connect(self.path)
        except OSError as e:
            s.close()
            raise CouldNotDrive("socket %s refused: %s" % (self.path, e))
        self.sock = s
        return self

    def send(self, verb, *args):
        """One request, one reply. Raises Unreadable on timeout, bad JSON or ok:false."""
        if self.sock is None:
            raise Unreadable("not connected")
        line = (json.dumps({"verb": verb, "args": list(args)}) + "\n").encode()
        try:
            self.sock.settimeout(shotwait.timeout_for(verb, args, TIMEOUT_S))   # a shot may wait for motion and the page (journeys/lib/shotwait.py)
            self.sock.sendall(line)
            reply = json.loads(self._readline().decode())
            self.sock.settimeout(TIMEOUT_S)
        except (OSError, ValueError) as e:
            raise Unreadable("%s %s: %s" % (verb, args, e))
        if not isinstance(reply, dict) or reply.get("ok") is not True:
            err = reply.get("error") if isinstance(reply, dict) else reply
            raise Unreadable("%s %s: %s" % (verb, args, err))
        return reply

    def _readline(self):
        while b"\n" not in self.buf:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise OSError("socket closed")
            self.buf += chunk
        line, self.buf = self.buf.split(b"\n", 1)
        return line

    def close(self):
        if self.sock is not None:
            self.sock.close()
            self.sock = None

    # actions
    def type(self, text):
        return self.send("type", text)

    def key(self, chord):
        return self.send("key", chord)

    def click(self, ax):
        return self.send("click", ax)

    # assertions
    def rows(self, ax):
        return self.send("rows", ax).get("rows", [])

    def uistate(self):
        return self.send("uistate").get("state", {})

    def winframes(self):
        return self.send("winframes").get("frames", [])

    def measure(self, ax):
        return self.send("measure", ax)

    def core(self, path):
        return self.send("core", "state", path).get("value")

    def shot(self, name):
        return self.send("capture", "shot", name)
