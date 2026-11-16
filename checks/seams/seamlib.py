"""Shared machinery for the cross-surface seam checks (checks/seams/*.py). Standard library only.

A seam check asserts that ONE datum looks the same on every surface that shows it. Every expected value
comes from three read-only sources and nowhere else:
  * the fixture the FixtureCore serves: docs/design/fixtures/mac-main-sample.json (WHOOSH_FIXTURE_PATH
    overrides, exactly as it does for the app);
  * docs/CAPABILITY-CONTRACT.md (invariants, fixed word sets);
  * docs/CONSTANTS.md (every number lives once there).
Never from a built file, never from the app's own reply.

Each check file defines expected() -> dict, observe(ctl, exp) -> obs, judge(obs, exp) -> [(label, verdict,
detail)], oracle(exp) -> the observation a correct app gives, and MUTANTS {name: (what it breaks, label
that must go red, fn(obs) -> mutated obs)}. run() does two things:
  1. LIVE: drives the running rig copy over its socket (needs $WHOOSH_SOCKET_DIR or $D, as set by
     scripts/launch-one.sh). A region the app does not register yet answers ok:false and is NOT JUDGED.
  2. MUTANTS: the judge must pass the oracle and go red on each mutant, on the label the mutant names.
     A mutant that stays green is a FAILED check: the judge cannot see that defect.
Prints `N checks, F failed, U not judged`; exit 1 when F > 0 or U > 0.

Region ids below are NOT frozen in docs/CONTROL-SOCKET.md (it serves mac-main.list and mac-main.tabs
only). They follow the ids journeys/J*.py already drive; the lane that registers a region either serves
the id named here or the seams lane re-points this table (one place).
"""
import json
import os
import re
import socket
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

REGION = {
    "list": "mac-main.list",                      # served: ax, id, sender, subject, account, unread, pinned, chips (+ colour, icon, decided_by once InboxList serves them)
    "tabs": "mac-main.tabs",                      # served: ax, id, title, unread, waiting
    "status": "mac-main.status",                  # {text, unread, tab}
    "sidebar_mailboxes": "mac-main.sidebar.mailboxes",   # {name, count}
    "sidebar_accounts": "mac-main.sidebar.accounts",     # {id, name, colour, icon, line}
    "sidebar_smart": "mac-main.sidebar.smart-folders",   # {name, count}
    "reader_header": "mac-main.reader.header",    # {message_id, account, colour, icon, date}
    "undo_bar": "mac-main.undo.bar",              # {text, seconds, entry}
    "palette": "mac-palette.results",             # {title, key}
    "search": "mac-search.results",               # {id, account, colour, icon}
    "search_suggest": "mac-search.dropdown",      # served (SearchMacRows): kind, title, detail, chip, account; no resolves_to
    "search_chips": "mac-search.chips",           # served: kind, value, title, remove
    "snooze": "mac-main.snooze.options",          # served (ComponentsFeature): id, title, detail, resolves_to (ISO), refused
    "sidebar_labels": "mac-main.sidebar.labels",  # served (SidebarTargets): id, name, raw_name, path, count, own, total
    "composer": "mac-composer.header",            # {from, signature}
    "composer_undo": "mac-composer.undo.bar",     # {text, seconds_left} (ComposerMacFeature.swift:69)
    "quickreply": "mac-main.quickreply.field",     # {from, signature} (QuickReplyFeature.swift:22)
    "notch_list": "mac-notch.list",               # {id, account, colour, icon}
    "notch_undo": "mac-notch.undo",               # {text}; no seconds field (NotchRows.swift:75)
    "notch_reply": "mac-notch.open.from",          # {from}; the notch serves no signature (NotchRows.swift:65)
    "agent_card": "mac-settings-ai.agents.card",  # {name, allowed: [words]}
    "welcome_accounts": "mac-welcome-accounts.accounts",   # {id, name, colour} (WelcomeMacFeature.swift:39)
    "settings_accounts": "mac-settings-accounts.accounts",  # {id, name, colour, line}
    "iphone_accounts": "iphone-mailboxes.accounts",  # {id, name, address, group, mark_label, mark_colour, mark_icon, picked} (InboxPhoneFeature.swift:25, InboxPhoneProbe.accounts)
    "iphone_list": "iphone-inbox.list",           # {id, account, mark_label, mark_colour, mark_icon, mark_attention, ...} (InboxPhoneProbe.list)
}


# ---------- read-only sources ----------

def fixture():
    p = os.environ.get("WHOOSH_FIXTURE_PATH") or os.path.join(ROOT, "docs/design/fixtures/mac-main-sample.json")
    with open(p, encoding="utf-8") as f:
        return json.load(f)


def _read(rel):
    with open(os.path.join(ROOT, rel), encoding="utf-8") as f:
        return f.read()


def constant_row(first):
    """A docs/CONSTANTS.md row by its first name: (names, values), both split on ' · '.
    `UNDO_SEND_DEFAULT_S · MIN · MAX | 10 · 5 · 30` -> (['UNDO_SEND_DEFAULT_S', 'MIN', 'MAX'], ['10', '5', '30'])."""
    for line in _read("docs/CONSTANTS.md").splitlines():
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) >= 2:
            names = [x.strip() for x in cells[0].split("·")]
            if names[0] == first:
                return names, [x.strip() for x in cells[1].split("·")]
    raise KeyError("no docs/CONSTANTS.md row starts with %s" % first)


def constant_int(first, index=0):
    v = constant_row(first)[1][index]
    m = re.match(r"\d[\d,]*", v)
    if not m:
        raise ValueError("%s[%d] = %r is not a number" % (first, index, v))
    return int(m.group(0).replace(",", ""))


def contract_row(row_id):
    for line in _read("docs/CAPABILITY-CONTRACT.md").splitlines():
        if line.startswith("| %s |" % row_id):
            return line
    raise KeyError("contract row %s missing" % row_id)


# ---------- the drive channel ----------

class NJ:
    """Not judged: the probe could not read the datum (region unregistered, verb refused, no socket)."""
    def __init__(self, why):
        self.why = why

    def __repr__(self):
        return "NOT JUDGED (%s)" % self.why

    def __eq__(self, other):
        return False

    __hash__ = None


def nj(v):
    return isinstance(v, NJ)


class Ctl:
    def __init__(self, sockdir_env=("WHOOSH_SOCKET_DIR", "D")):
        d = ""
        for e in sockdir_env:
            d = d or os.environ.get(e, "")
        self.path = os.path.join(d, "whoosh.sock") if d else ""
        self.s, self.buf, self.log = None, b"", []
        if self.path and os.path.exists(self.path):
            self.s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            self.s.settimeout(10)
            self.s.connect(self.path)

    @property
    def live(self):
        return self.s is not None

    def call(self, verb, *args):
        if not self.s:
            return {"ok": False, "error": "no socket (run under scripts/launch-one.sh)"}
        self.s.sendall((json.dumps({"verb": verb, "args": [str(a) for a in args]}) + "\n").encode())
        while b"\n" not in self.buf:
            chunk = self.s.recv(65536)
            if not chunk:
                return {"ok": False, "error": "socket closed"}
            self.buf += chunk
        line, self.buf = self.buf.split(b"\n", 1)
        r = json.loads(line.decode())
        self.log.append((verb, args, r.get("ok"), r.get("error")))
        return r

    def act(self, verb, *args):
        """An action (key, click, type, clock). Its failure is logged; the check after it shows the effect."""
        return bool(self.call(verb, *args).get("ok"))

    def rows(self, region_key):
        ax = REGION.get(region_key, region_key)
        r = self.call("rows", ax)
        return r["rows"] if r.get("ok") else NJ("rows %s: %s" % (ax, r.get("error")))

    def ui(self, field=None):
        r = self.call("uistate")
        if not r.get("ok"):
            return NJ("uistate: %s" % r.get("error"))
        st = r["state"]
        return st if field is None else st.get(field, NJ("uistate has no %s" % field))

    def core(self, path):
        r = self.call("core", "state", path)
        return r["value"] if r.get("ok") else NJ("core %s: %s" % (path, r.get("error")))


def load_corpus(ctl, name="fixtures/tests-j2.json"):
    """Load the J2 search corpus into the fixture core (delta 26/29). Returns None when loaded, else NJ(why)."""
    r = ctl.call("fixture", "corpus", os.path.join(ROOT, name))
    return None if r.get("ok") else NJ("fixture corpus %s: %s" % (name, r.get("error")))


def pick(rows, key, val, field):
    """The `field` of the row whose `key` equals val; NJ when rows is NJ, the row is absent or lacks it."""
    if nj(rows):
        return rows
    for r in rows:
        if r.get(key) == val:
            return r.get(field, NJ("row %s=%s has no field %s" % (key, val, field)))
    return NJ("no row with %s=%r" % (key, val))


def col(rows, field):
    if nj(rows):
        return rows
    return [r.get(field) for r in rows]


# ---------- judging ----------

def equal(label, expected, observed_by_surface):
    """One check per surface: the surface shows exactly `expected`. NJ -> not judged."""
    out = []
    for surface, got in observed_by_surface.items():
        if nj(got):
            out.append(("%s@%s" % (label, surface), None, got.why))
        else:
            ok = got == expected
            out.append(("%s@%s" % (label, surface), ok, "" if ok else "expected %r, got %r" % (expected, got)))
    return out


def norm_words(s):
    return re.sub(r"[^a-z0-9 ]+", " ", str(s).lower()).split()


def contains_words(text, phrase):
    t, p = norm_words(text), norm_words(phrase)
    return any(t[i:i + len(p)] == p for i in range(len(t) - len(p) + 1))


def run(name, expected, observe, judge, oracle, mutants):
    exp = expected()
    lines, n, f, u = [], 0, 0, 0
    selftest = "--selftest" in sys.argv[1:]     # the judge's own mutant test only: no socket is read, exit 0 when it can fail
    ctl = Ctl()
    if selftest:
        live = []
        print("== %s  selftest (no app, no socket)" % name)
    elif ctl.live:
        live = judge(observe(ctl, exp), exp)
    else:
        live = [("live-drive", None, "no socket: run under scripts/launch-one.sh <bundle> -- --fixture-core python3 checks/seams/%s.py" % name)]
    if not selftest:
        print("== %s  live drive (%s)" % (name, ctl.path or "no socket"))
    for label, v, detail in live:
        n += 1
        tag = "PASS" if v is True else ("FAIL" if v is False else "NOT JUDGED")
        f += v is False
        u += v is None
        print("  %-10s %s  %s" % (tag, label, detail))
    # mutant self-test of the judge
    good = oracle(exp)
    base = {l: v for l, v, _ in judge(good, exp)}
    print("== %s  MUTANTS (the judge must pass the oracle and go red on each)" % name)
    bad_oracle = [l for l, v in base.items() if v is not True]
    n += 1
    if bad_oracle:
        f += 1
        print("  FAIL       oracle  the correct-app observation is not green on: %s" % ", ".join(bad_oracle))
    else:
        print("  PASS       oracle  all %d labels green on the fixture-derived observation" % len(base))
    print("  | mutant | what it breaks | label that must go red | seen |")
    for mname, (what, must, fn) in mutants.items():
        n += 1
        res = judge(fn(json.loads(json.dumps(good))), exp)
        red = [(l, d) for l, v, d in res if v is False and (l == must or l.startswith(must + "@"))]
        if red:
            print("  | %s | %s | %s | RED: %s: %s |" % (mname, what, must, red[0][0], red[0][1]))
        else:
            f += 1
            print("  | %s | %s | %s | STAYED GREEN (judge blind to this defect) |" % (mname, what, must))
    if ctl.live and ctl.log:
        refused = [(v, a, e) for v, a, ok, e in ctl.log if not ok]
        if refused:
            print("  actions/probes refused by the app: %d, first: %s %s -> %s" % ((len(refused),) + refused[0]))
    print("%d checks, %d failed, %d not judged" % (n, f, u))
    return 1 if (f or u) else 0
