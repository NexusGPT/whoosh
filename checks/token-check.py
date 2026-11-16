#!/usr/bin/env python3
"""Token check: vis-designtokens. Counts colour literals outside Sources/Design; the count must be 0.

A colour has one home, Sources/Design (the tokens). Anywhere else a hex string, a hex integer or a numeric
Color(red:...) is a second home. Scans the .swift files of Sources/ (minus Sources/Design and the generated
bridge) and apps/. ONE exception: a vendor's brand mark drawn in its real colours (owner: "make sure each of the
logo are the right one"), allowed only in BRAND_MARK_FILES and only on a line that ends in the marker
`// brand-mark: <vendor>`; the same line without the marker counts, and a marker in any other file is red.
Repo root: env WHOOSH_REPO, else this repo. Prints the MUTANTS table then
'N checks, F failed, U not judged'; exit 1 when F > 0 or U > 0.
"""
import os, re, sys

ROOT = os.environ.get("WHOOSH_REPO") or os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXEMPT = ("Sources/Design/", "Sources/CoreBridge/Generated/")
BRAND_MARK_FILES = ("Sources/Features/AddAccount/AddAccountMarks.swift", "Sources/Features/SettingsWindow/ProviderMarks.swift")
BRAND_MARK = re.compile(r"//\s*brand-mark:\s*[a-z][a-z0-9-]*\s*$")
PATTERNS = [
    ("hex-string-literal", re.compile(r'"#[0-9A-Fa-f]{6}(?:[0-9A-Fa-f]{2})?"')),
    ("hex-integer-colour", re.compile(r'(?:Color|NSColor|UIColor|CGColor)[^\n]*\b0x[0-9A-Fa-f]{6,8}\b|\b0x[0-9A-Fa-f]{6,8}\b[^\n]*(?:Color|hex)')),
    ("numeric-rgb-initialiser", re.compile(r'\b(?:Color|NSColor|UIColor)\(\s*(?:red|srgbRed|white|hue)\s*:\s*[0-9.]')),
    ("hex-initialiser-call", re.compile(r'\bColor\(\s*hex\s*:')),
]


def swift_files(root):
    for base in ("Sources", "apps"):
        for d, _, fs in os.walk(os.path.join(root, base)):
            for f in fs:
                p = os.path.join(d, f)
                rel = os.path.relpath(p, root)
                if f.endswith(".swift") and not rel.startswith(EXEMPT):
                    yield p, rel


def exempt(rel, line):
    """True when this line is a marked brand-mark line in an allowed file."""
    return rel in BRAND_MARK_FILES and bool(BRAND_MARK.search(line.rstrip("\n")))


def scan(root):
    hits = {name: [] for name, _ in PATTERNS}
    hits["brand-mark-outside-allowed-file"] = []
    marked = []
    n = 0
    for p, rel in swift_files(root):
        n += 1
        for i, line in enumerate(open(p, encoding="utf-8", errors="replace"), 1):
            if line.lstrip().startswith("//"):
                continue
            if BRAND_MARK.search(line.rstrip("\n")) and rel not in BRAND_MARK_FILES:
                hits["brand-mark-outside-allowed-file"].append("%s:%d" % (rel, i))
            if exempt(rel, line):
                marked.append("%s:%d" % (rel, i))
                continue
            for name, rx in PATTERNS:
                if rx.search(line):
                    hits[name].append("%s:%d" % (rel, i))
    scan.marked = marked
    return n, hits


def main():
    n, hits = scan(ROOT)
    print("brand-mark lines exempted (marked, in %s): %d" % (", ".join(BRAND_MARK_FILES), len(scan.marked)))
    checks = []   # (label, ok, detail)
    checks.append(("scanner-reads-files", n > 0, "%d swift files scanned" % n))
    # positive control: every pattern must fire on a known literal, or a 0 means a broken probe
    samples = {"hex-string-literal": 'let c = "#FF0000"', "hex-integer-colour": "let c = Color(0xFF0000)",
               "numeric-rgb-initialiser": "let c = Color(red: 1, green: 0, blue: 0)", "hex-initialiser-call": 'Color(hex: "x")'}
    for name, rx in PATTERNS:
        checks.append(("control-" + name, bool(rx.search(samples[name])), "sample line " + repr(samples[name])))
    marked_line = 'Rectangle().fill(Color(hex: 0xF25022)) // brand-mark: microsoft'
    checks.append(("control-brand-mark-exempts", exempt(BRAND_MARK_FILES[0], marked_line), "a marked line in the allowed file"))
    checks.append(("control-brand-mark-needs-marker", not exempt(BRAND_MARK_FILES[0], 'Color(hex: 0xF25022)'), "an unmarked line"))
    checks.append(("control-brand-mark-needs-file", not exempt("Sources/Features/X/Y.swift", marked_line), "a marked line elsewhere"))
    total = 0
    for name in [k for k, _ in PATTERNS] + ["brand-mark-outside-allowed-file"]:
        total += len(hits[name])
        checks.append(("none-" + name, not hits[name], "%d hit(s): %s" % (len(hits[name]), ", ".join(hits[name][:5]))))
    print("hex colour literals outside Sources/Design: %d" % total)
    print("MUTANTS (a check is green only after it has been seen red on its mutant)\n| check | mutant |\n|---|---|")
    for k, v in [("scanner-reads-files", "WHOOSH_REPO points at an empty tree"),
                 ("none-hex-string-literal", 'a feature file holds "#2F6FED"'),
                 ("none-hex-integer-colour", "a feature file holds Color(0x2F6FED)"),
                 ("none-numeric-rgb-initialiser", "a feature file holds Color(red: 0.2, green: 0.4, blue: 0.9)"),
                 ("none-hex-initialiser-call", 'a feature file holds Color(hex: "x")'),
                 ("none-brand-mark-outside-allowed-file", "a marker on a line of another file"),
                 ("brand-mark exemption", "a marker removed from one AddAccountMarks line: that line counts again"),
                 ("control-*", "a pattern is loosened so that it no longer matches its own sample")]:
        print("| %s | %s |" % (k, v))
    failed = [c for c in checks if not c[1]]
    for label, ok, detail in checks:
        if not ok:
            sys.stderr.write("FAIL %s: %s\n" % (label, detail))
    print("%d checks, %d failed, 0 not judged" % (len(checks), len(failed)))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
