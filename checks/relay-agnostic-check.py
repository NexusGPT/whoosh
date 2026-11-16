#!/usr/bin/env python3
"""relay-agnostic check: the open-source code names no relay host. SPEC A80 (owner, 2026-11-09): "It needs to be
agnostic in the open source code so that people can decide what they do, but in our case we can host on nexus".

    python3 checks/relay-agnostic-check.py        audit the tracked files of this repository

Six checks over `git ls-files`, then a MUTANTS table, then `N checks, F failed, U not judged` as the LAST line.
Every run also plants each defect in an in-memory copy of the files and requires its check to go red
(docs/TESTING.md: a check is green only after it has been seen red on its mutant); a mutant that stays green
counts as a failed check. Our own hosted instance is recorded in docs/RELAY-HOSTING.md, which this check skips,
and `_captures/` is skipped. Exit 1 when F > 0 or U > 0.
Repo root: env WHOOSH_REPO, else this repo.
"""
import os, re, subprocess, sys

ROOT = os.environ.get("WHOOSH_REPO") or os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SKIP_PREFIX = ("_captures/", ".git/")
SKIP_FILES = {"docs/RELAY-HOSTING.md", "checks/relay-agnostic-check.py"}
SKIP_EXT = (".png", ".jpg", ".jpeg", ".gif", ".ico", ".icns", ".pdf", ".zip", ".gz", ".sqlite3", ".db", ".a", ".dylib", ".p8", ".ttf", ".otf", ".woff", ".woff2")
MAX_BYTES = 2_000_000

# A relay or push host under a Nexus domain, in either order.
NEXUS_RELAY = re.compile(
    r"(?i)(?:[a-z0-9-]+\.)*(?:relay|push)[a-z0-9.-]*\.(?:gpt\.nexus|nexusgpt\.[a-z]+|nexus\.(?:app|io|run|dev|cloud))\b"
    r"|(?:gpt\.nexus|nexusgpt\.[a-z]+)[^\s\"'<>)]*(?:relay|push)")
NEXUS_URL = re.compile(r"(?i)https?://[^\s\"'<>)/]*(?:nexusgpt|gpt\.nexus|nexus)[^\s\"'<>)]*")
URL = re.compile(r"https?://([A-Za-z0-9._\[\]:{}$<>%-]+)")
RELAY_ASSIGN = re.compile(
    r"(?i)(?:relay[_-]?(?:url|base|host|address|endpoint)|relayURL|RELAY_PUBLIC_BASE_URL)[\"']?\s*[:=(,]?\s*(?:\w+\(\s*(?:string:\s*)?)?[\"'](https?://[^\"'\s]+)"
    r"|baseURL:\s*(?:URL\(string:\s*)?\"(https?://[^\"]+)\"")
ENV_NAME = re.compile(r'"(RELAY_[A-Z0-9_]+)"')
ALLOWED_SUFFIX = (".example", ".test", ".invalid", ".example.com", ".example.org", ".example.net", ".apple.com", ".google.com",
                  ".googleapis.com", ".microsoft.com", ".microsoftonline.com", ".fastmail.com")
ALLOWED_EXACT = {"localhost", "127.0.0.1", "[::1]", "::1", "example.com", "example.org", "example.net", "apple.com", "google.com"}


def in_code_scope(p):
    return (p.startswith(("relay/", "Sources/", "apps/", "scripts/", "tools/")) and not p.startswith("relay/tests/")) or p == "docs/pipeline.env"


def in_host_scope(p):
    return p.startswith(("relay/src/", "relay/tests/", "Sources/Features/PushClient/", "apps/ios/NotificationService/")) or \
        p in ("relay/Dockerfile", "relay/.env.example", "relay/README.md")


def host_allowed(h):
    h = h.lower().split("/")[0]
    if any(c in h for c in "{}$<>%"):
        return True
    if h.startswith("["):
        h = h.split("]")[0] + "]"
    else:
        h = h.rsplit(":", 1)[0] if re.search(r":\d+$", h) else h
    return h in ALLOWED_EXACT or "." not in h or h.endswith(ALLOWED_SUFFIX)


class Tree:
    """The tracked text files of the repo, with an in-memory overlay for the mutants (None = deleted)."""
    def __init__(self, root, overlay=None):
        self.root, self.overlay = root, overlay or {}
        try:
            out = subprocess.run(["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"], cwd=root, capture_output=True, check=True).stdout.decode()
            self.paths = [p for p in out.split("\0") if p]
        except (OSError, subprocess.CalledProcessError):
            self.paths = [os.path.relpath(os.path.join(d, f), root) for d, _, fs in os.walk(root) for f in fs]
        self.paths = sorted(set(self.paths) | set(k for k, v in self.overlay.items() if v is not None))

    def files(self):
        for p in self.paths:
            if p.startswith(SKIP_PREFIX) or p in SKIP_FILES or p.endswith(SKIP_EXT):
                continue
            t = self.read(p)
            if t is not None:
                yield p, t

    def read(self, p):
        if p in self.overlay:
            return self.overlay[p]
        full = os.path.join(self.root, p)
        try:
            if not os.path.isfile(full) or os.path.getsize(full) > MAX_BYTES:
                return None
            return open(full, encoding="utf-8").read()
        except (OSError, UnicodeDecodeError):
            return None

    def with_edit(self, path, fn):
        t = self.read(path)
        return Tree(self.root, {**self.overlay, path: None if fn is None else fn(t or "")})


def line_of(text, pos):
    return text.count("\n", 0, pos) + 1


# ------------------------------------------------------------------ the checks: each returns a list of problems
def c_no_nexus_relay_host(t):
    bad = []
    for p, text in t.files():
        for m in NEXUS_RELAY.finditer(text):
            bad.append("%s:%d names a Nexus relay host: %s" % (p, line_of(text, m.start()), m.group(0)[:60]))
        if in_code_scope(p):
            for m in NEXUS_URL.finditer(text):
                bad.append("%s:%d code names a Nexus address: %s" % (p, line_of(text, m.start()), m.group(0)[:60]))
    return bad


def c_no_hard_coded_relay_url(t):
    bad = []
    for p, text in t.files():
        if not in_code_scope(p):
            continue
        for m in RELAY_ASSIGN.finditer(text):
            if m.group(2) and not in_host_scope(p):
                continue   # a bare `baseURL:` only means the relay in the relay's own and the client's folders
            url = m.group(1) or m.group(2)
            host = URL.match(url).group(1) if URL.match(url) else url
            if not host_allowed(host):
                bad.append("%s:%d hard-codes a relay address: %s" % (p, line_of(text, m.start()), url[:60]))
    return bad


def c_no_unlisted_host_literal(t):
    bad = []
    for p, text in t.files():
        if not in_host_scope(p):
            continue
        for m in URL.finditer(text):
            if not host_allowed(m.group(1)):
                bad.append("%s:%d has a host literal that is not loopback, a placeholder or a provider's: %s" % (p, line_of(text, m.start()), m.group(0)[:60]))
    return bad


def c_address_is_a_setting(t):
    s = t.read("Sources/Features/PushClient/RelaySettings.swift")
    if s is None:
        return ["Sources/Features/PushClient/RelaySettings.swift is missing: the client has no relay_url setting"]
    bad = []
    if 'static let key = "relay_url"' not in s:
        bad.append('RelaySettings does not declare the key "relay_url"')
    if not re.search(r"guard let text = clean\(raw\), !text\.isEmpty else \{ return \.off \}", s):
        bad.append("RelaySettings does not resolve an absent or empty relay_url to .off (the default must be none)")
    return bad


def c_config_is_environment(t):
    need = ["relay/Dockerfile", "relay/README.md", "relay/.env.example", "docs/guides/host-your-own-relay.md", "docs/RELAY-HOSTING.md"]
    bad = ["%s is missing" % p for p in need if t.read(p) is None]
    readme, example = t.read("relay/README.md") or "", t.read("relay/.env.example") or ""
    names = set()
    for p in t.paths:
        if p.startswith("relay/src/") and p.endswith(".rs"):
            names |= set(ENV_NAME.findall(t.read(p) or ""))
    for n in sorted(names):
        if n not in readme:
            bad.append("%s is read by relay/src but not documented in relay/README.md" % n)
        if n not in example:
            bad.append("%s is read by relay/src but absent from relay/.env.example" % n)
    for n in ("RELAY_BIND", "RELAY_DB", "RELAY_APNS_KEY_PATH", "RELAY_APNS_KEY_ID", "RELAY_APNS_TEAM_ID", "RELAY_APNS_TOPIC"):
        if n not in names:
            bad.append("relay/src no longer reads %s from the environment" % n)
    return bad


INCLUDE = re.compile(r'#\[\s*path\s*=\s*"([^"]+)"\s*\]|include(?:_str|_bytes)?!\s*\(\s*(?:concat!\([^)]*\)|"([^"]+)")')
TOML_PATH = re.compile(r'(?:path|workspace)\s*=\s*"([^"]+)"')


def c_builds_alone(t):
    """Static scan, no cargo: a copy of relay/ alone must hold every file a build reads. Resolves each
    #[path], include!, include_str! and include_bytes! of relay/**/*.rs against the file's own folder, each
    Cargo.toml path dependency against relay/, and each Dockerfile COPY source against the build context."""
    files = {p for p, _ in ((p, 1) for p in t.paths) if p.startswith("relay/") and t.read(p) is not None}
    bad = []

    def resolve(src, rel, base):
        full = os.path.normpath(os.path.join(os.path.dirname(src) if base == "file" else "relay", rel))
        if full == ".." or full.startswith("../") or not (full == "relay" or full.startswith("relay/")):
            return "%s leaves relay/: %s" % (src, rel)
        if full not in files and not any(f.startswith(full + "/") for f in files):
            return "%s names %s which is not in relay/" % (src, rel)
        return None
    for src in sorted(files):
        text = t.read(src) or ""
        if src.endswith(".rs"):
            text = "\n".join("" if ln.lstrip().startswith("//") else ln for ln in text.split("\n"))   # comments name attributes without using them
            for m in INCLUDE.finditer(text):
                rel = m.group(1) or m.group(2)
                if not rel:
                    bad.append("%s:%d include with a computed path" % (src, line_of(text, m.start())))
                    continue
                r = resolve(src, rel, "file")
                if r:
                    bad.append("%s:%d %s" % (src, line_of(text, m.start()), r.split(" ", 1)[1] if r.startswith(src) else r))
        elif os.path.basename(src) == "Cargo.toml":
            for m in TOML_PATH.finditer(text):
                if m.group(1) in ("true", "false"):
                    continue
                if ".." in m.group(1) or m.group(1).startswith("/"):
                    bad.append("%s:%d dependency path leaves relay/: %s" % (src, line_of(text, m.start()), m.group(1)))
        elif os.path.basename(src) == "Dockerfile":
            for i, ln in enumerate(text.splitlines(), 1):
                if re.match(r"\s*(COPY|ADD)\b", ln) and "--from" not in ln:
                    for a in ln.split()[1:-1]:
                        if a.startswith("--"):
                            continue
                        if ".." in a or a.startswith("/"):
                            bad.append("%s:%d copies from outside the build context: %s" % (src, i, a))
                        elif a != "." and not any(f == "relay/" + a or f.startswith("relay/" + a.rstrip("/") + "/") for f in files):
                            bad.append("%s:%d copies %s which is not in relay/" % (src, i, a))
    return bad


CHECKS = [
    ("no-nexus-relay-hostname-anywhere", c_no_nexus_relay_host),
    ("no-hard-coded-relay-url-in-code", c_no_hard_coded_relay_url),
    ("no-unlisted-host-literal-in-relay-and-client", c_no_unlisted_host_literal),
    ("relay-address-is-a-setting-default-none", c_address_is_a_setting),
    ("server-config-is-environment-and-documented", c_config_is_environment),
    ("relay-builds-alone", c_builds_alone),
]
CHECK_FN = dict(CHECKS)

# Each mutant plants one defect in the in-memory tree; its check must then report a problem.
MUTANTS = [
    ("no-nexus-relay-hostname-anywhere", "Rust source defaults to a Nexus relay host", "relay/src/main.rs",
     lambda s: s + '\nconst DEFAULT_RELAY: &str = "https://relay.gpt.nexus";\n'),
    ("no-nexus-relay-hostname-anywhere", "a Swift file in the client names a Nexus push host", "Sources/Features/PushClient/RelaySettings.swift",
     lambda s: s + '\nlet fallback = URL(string: "https://push.nexusgpt.io")!\n'),
    ("no-hard-coded-relay-url-in-code", "the client hard-codes some other relay address", "Sources/Features/PushClient/RelaySettings.swift",
     lambda s: s + '\nlet relayURL = "https://relay.acme.net"\n'),
    ("no-unlisted-host-literal-in-relay-and-client", "the server defaults to a fixed host", "relay/src/main.rs",
     lambda s: s + '\nfn base() -> String { "https://push.acme.net".into() }\n'),
    ("relay-address-is-a-setting-default-none", "an empty relay_url no longer means push off", "Sources/Features/PushClient/RelaySettings.swift",
     lambda s: s.replace("else { return .off }", "else { return .refused(\"empty\") }")),
    ("server-config-is-environment-and-documented", "the server reads a variable nobody documents", "relay/src/main.rs",
     lambda s: s + '\nfn extra() -> Option<String> { std::env::var("RELAY_UNDOCUMENTED_THING").ok() }\n'),
    ("server-config-is-environment-and-documented", "the Dockerfile is gone", "relay/Dockerfile", None),
    ("relay-builds-alone", "the registry includes the core's constants file again", "relay/src/registry/mod.rs",
     lambda s: s.replace('"../constants.rs"', '"../../../core/crates/contracts/src/constants.rs"')),
    ("relay-builds-alone", "a source file includes a text from the repo's docs", "relay/src/main.rs",
     lambda s: s + '\nconst T: &str = include_str!("../../docs/CONSTANTS.md");\n'),
    ("relay-builds-alone", "a path dependency points at the core", "relay/Cargo.toml",
     lambda s: s + '\ncontracts = { path = "../core/crates/contracts" }\n'),
    ("relay-builds-alone", "the generated relay constants file is missing", "relay/src/constants.rs", None),
]


def main():
    base = Tree(ROOT)
    if not base.paths or base.read("relay/src/main.rs") is None:
        print("0 checks, 0 failed, %d not judged" % len(CHECKS))
        print("no relay/src/main.rs under %s: nothing to audit" % ROOT)
        return 1
    results = []
    for name, fn in CHECKS:
        problems = fn(base)
        results.append((name, problems))
        print("  %-8s %s" % ("PASS" if not problems else "FAIL", name))
        for p in problems[:8]:
            print("           - " + p)
    rows = []
    for check_name, what, path, edit in MUTANTS:
        mutated = base.with_edit(path, edit)
        problems = CHECK_FN[check_name](mutated)
        rows.append((check_name, what, bool(problems)))
    print("MUTANTS\n| check | mutation made | seen red |\n|---|---|---|")
    for check_name, what, red in rows:
        print("| %s | %s | %s |" % (check_name, what, "yes" if red else "NO"))
    failed = sum(1 for _, p in results if p)
    for check_name, what, red in rows:
        if not red:
            failed += 1
            print("FAIL  mutant stayed green: %s / %s" % (check_name, what))
    print("%d checks, %d failed, 0 not judged" % (len(CHECKS), failed))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
