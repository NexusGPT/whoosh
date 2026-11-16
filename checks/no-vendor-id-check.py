#!/usr/bin/env python3
"""no-vendor-id-check: the repository holds NO Google OAuth client ID, NO client secret, and at most ONE Microsoft client ID:
the owner's own registration (SPEC A88, 2026-11-09, reversing A77 for that one ID), in exactly one place:

    Sources/Features/AddAccount/DefaultSignInClients.swift   static let microsoft: String? = "<GUID>"

A person may still paste the client ID of an app registered in THEIR OWN admin (the override); it lives in their Keychain.

    python3 checks/no-vendor-id-check.py              scan Sources, core and docs; exit 1 when an ID appears
    python3 checks/no-vendor-id-check.py --selftest   the scan on a clean tree and on a tree with each kind of ID planted

What counts as an ID:
  allowed    one GUID, on the `static let microsoft` line of the file above; a second one, or a GUID on any other line of that
             file, is red
  secret     client_secret given a value (client_secret = "...", "client_secret": "...", client_secret=... in a URL)
  google     <digits>-<letters and digits>.apps.googleusercontent.com (the shape Google issues)
  microsoft  a GUID (8-4-4-4-12 hex) on a line that speaks of a client or application ID, Azure, MSAL or the Microsoft
             sign-in authority; a GUID elsewhere (a fixture's uuid, a board id) is not an OAuth client ID.
Build outputs (target, .build, build) are not scanned. Tests build sample IDs from parts at run time, so they never match.
"""
import os
import re
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DIRS = ["Sources", "core", "docs"]
SKIP = {"target", ".build", "build", "node_modules", ".git"}
TEXT = (".swift", ".rs", ".md", ".json", ".toml", ".py", ".sh", ".plist", ".strings", ".xcstrings", ".yml", ".yaml", ".txt", ".h", ".sql")
GOOGLE = re.compile(r"\b\d{6,}-[a-z0-9]{8,}\.apps\.googleusercontent\.com\b", re.I)
GUID = re.compile(r"\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b", re.I)
SECRET = re.compile(r"client_secret[\"']?\s*[:=]\s*[\"']?[A-Za-z0-9~._-]{8,}", re.I)
DEFAULT_FILE = os.path.join("Sources", "Features", "AddAccount", "DefaultSignInClients.swift")
DEFAULT_LINE = re.compile(r'^\s*(public\s+)?static\s+let\s+microsoft\s*:\s*String\?\s*=\s*"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"\s*(//.*)?$', re.I)
MS_CONTEXT = re.compile(r"client[ _-]?id|clientid|application \(client\) id|app(lication)?[ _-]?id|azure|msal|login\.microsoftonline", re.I)


def scan(root):
    hits = []
    allowed = 0
    for d in DIRS:
        base = os.path.join(root, d)
        for dirpath, dirnames, files in os.walk(base):
            dirnames[:] = [n for n in dirnames if n not in SKIP]
            for f in files:
                if not f.endswith(TEXT):
                    continue
                p = os.path.join(dirpath, f)
                try:
                    lines = open(p, encoding="utf-8", errors="replace").read().splitlines()
                except OSError:
                    continue
                rel = os.path.relpath(p, root)
                for i, line in enumerate(lines, 1):
                    if GOOGLE.search(line):
                        hits.append((rel, i, "google"))
                    elif SECRET.search(line):
                        hits.append((rel, i, "client secret"))
                    elif rel == DEFAULT_FILE and GUID.search(line):
                        # the one allowed ID: on the microsoft constant's own line, once; anything else in the file is red
                        if DEFAULT_LINE.match(line) and allowed == 0 and len(GUID.findall(line)) == 1:
                            allowed += 1
                        else:
                            hits.append((rel, i, "microsoft (beyond the one default)"))
                    elif GUID.search(line) and MS_CONTEXT.search(line):
                        hits.append((rel, i, "microsoft"))
    return hits


def report(hits):
    for p, i, kind in hits:
        print("VENDOR ID (%s): %s:%d" % (kind, p, i))   # the value itself is not printed
    print("%d checks, %d failed, 0 not judged" % (1, 1 if hits else 0))
    return 1 if hits else 0


def selftest():
    guid = "-".join("b" * n for n in (8, 4, 4, 4, 12))
    gid = "%s-%s%s" % ("7" * 9, "q" * 12, ".apps" + ".googleusercontent" + ".com")
    secret = "client_" + "secret = \"%s\"" % ("s" * 20)
    default = 'static let microsoft: String? = "%s"' % guid
    other = os.path.join("Sources", "Planted.swift")
    cases = [("clean tree", None, 0), ("a planted Google client ID", (other, 'let id = "%s"' % gid), 1),
             ("a planted Microsoft client ID", (other, 'clientId: "%s"' % guid), 1), ("a GUID that is no client ID", (other, 'uuid: "%s"' % guid), 0),
             ("the one default, in its file", (DEFAULT_FILE, default), 0),
             ("the default's line in another file", (other, 'static let microsoftClientId: String? = "%s"' % guid), 1),
             ("a second GUID in the default file", (DEFAULT_FILE, default + "\n" + 'static let backup = "%s"' % guid), 1),
             ("two defaults in the file", (DEFAULT_FILE, default + "\n" + default), 1),
             ("a Google ID in the default file", (DEFAULT_FILE, default + "\n" + 'static let google: String? = "%s"' % gid), 1),
             ("a client secret anywhere", (other, secret), 1)]
    bad = 0
    for name, planted, want in cases:
        with tempfile.TemporaryDirectory() as t:
            os.makedirs(os.path.join(t, "Sources"))
            if planted:
                path, text = planted
                os.makedirs(os.path.dirname(os.path.join(t, path)), exist_ok=True)
                open(os.path.join(t, path), "w").write(text + "\n")
            got = 1 if scan(t) else 0
        ok = got == want
        bad += not ok
        print("%-4s %-32s found=%d" % ("ok" if ok else "BAD", name, got))
    print("%d checks, %d failed, 0 not judged" % (len(cases), bad))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv[1:] else report(scan(ROOT)))
