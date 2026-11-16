#!/usr/bin/env python3
"""Every Words.text("...") literal in the AI setup screens has an entry in Localizable.xcstrings with a non-empty value in all 14 languages.

  python3 checks/words-catalogue-check.py                 the three owned areas (AISetup, SettingsAIMac/AIWords.swift, FirstStart/FirstStartAI.swift)
  python3 checks/words-catalogue-check.py --all           the whole of Sources/ (measure; prints every miss with file and line)
  python3 checks/words-catalogue-check.py --catalogue F   read another catalogue file (the mutant run)
  python3 checks/words-catalogue-check.py --no-dash       skip the dash rule (default: a dash character, or a spaced or doubled hyphen, in any of the 14 values of a key of the scope fails)
Exit 1 when a key is absent or lacks a language; exit 0 otherwise. The last line says which.
"""
import json, os, re, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CAT = os.path.join(ROOT, "Sources", "Resources", "Localizable.xcstrings")
OWNED = ["Sources/Features/AISetup", "Sources/Features/SettingsAIMac/AIWords.swift", "Sources/Features/FirstStart/FirstStartAI.swift"]
LANGS = ["en", "fr", "nl", "de", "es", "it", "pt", "pl", "sv", "da", "nb", "fi", "ja", "zh-Hans"]
CALL = re.compile(r'Words\.text\(\s*"((?:[^"\\]|\\.)*)"')
ESC = {"n": "\n", "t": "\t", '"': '"', "\\": "\\", "'": "'", "0": "\0"}

def unescape(s):
    s = re.sub(r"\\u\{([0-9A-Fa-f]+)\}", lambda m: chr(int(m.group(1), 16)), s)
    return re.sub(r"\\(.)", lambda m: ESC.get(m.group(1), m.group(1)), s)

def files(paths):
    for p in paths:
        p = os.path.join(ROOT, p)
        if os.path.isfile(p): yield p
        for d, _, fs in os.walk(p):
            for f in sorted(fs):
                if f.endswith(".swift"): yield os.path.join(d, f)

def literals(paths):
    out = []
    for f in files(paths):
        for n, line in enumerate(open(f, encoding="utf-8"), 1):
            for m in CALL.finditer(line):
                k = m.group(1)
                if "\\(" in k: continue   # interpolated: not a catalogue key
                out.append((unescape(k), os.path.relpath(f, ROOT), n))
    return out

def main(a):
    cat_path = a[a.index("--catalogue") + 1] if "--catalogue" in a else CAT
    cat = json.load(open(cat_path, encoding="utf-8"))["strings"]
    whole = "--all" in a
    lits = literals(["Sources"] if whole else OWNED)
    missing, seen = [], set()
    for k, f, n in lits:
        loc = cat.get(k, {}).get("localizations", {})
        lack = [c for c in LANGS if not loc.get(c, {}).get("stringUnit", {}).get("value", "").strip()]
        if lack and (k, f, n) not in seen:
            seen.add((k, f, n)); missing.append((k, f, n, "absent" if k not in cat else "lacks " + ",".join(lack)))
    dashes = []
    if "--no-dash" not in a:
        for k in sorted({k for k, _, _ in lits}):
            for c in LANGS:
                v = cat.get(k, {}).get("localizations", {}).get(c, {}).get("stringUnit", {}).get("value", "")
                if re.search("[\u2010-\u2015\u2212]|\\s-\\s|--", v): dashes.append((k, c, v))
    for k, f, n, why in missing: print("MISSING %s:%d  %r  %s" % (f, n, k, why))
    for k, c, v in dashes: print("DASH %s %r -> %r" % (c, k, v))
    keys = {k for k, _, _ in lits}
    bad = {m[0] for m in missing}
    print("%d Words.text literals, %d distinct keys, %d distinct keys missing or incomplete (%d sites), scope %s, catalogue %s"
          % (len(lits), len(keys), len(bad), len(missing), "Sources/" if whole else "owned areas", os.path.basename(cat_path)))
    fail = bool(missing or dashes)
    print("%d dash hits" % len(dashes))
    print("RED words-catalogue-check: %d keys missing or incomplete, %d dash hits" % (len(bad), len(dashes)) if fail else "GREEN words-catalogue-check: every key holds all 14 languages")
    return 1 if fail else 0

sys.exit(main(sys.argv[1:]))
