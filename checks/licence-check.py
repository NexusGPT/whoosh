#!/usr/bin/env python3
"""Licence check: gap G5, "open source under MIT". Three sources of copyleft or copied text:

  1. no source file carries a GPL, AGPL or GNU licence header;
  2. no string of 40 characters or more is shared with the AGPL boilerplate's prompt files
     (dir: env AGPL_BOILERPLATE_DIR, default the org systems/apps/ai-email-app folder; absent = NOT JUDGED);
  3. no Cargo dependency is GPL-family (cargo metadata, offline; unavailable = NOT JUDGED) and no SwiftPM pin
     exists without a row in docs/LICENCES.md (identity, repository, licence, LICENSE file URL, revision; missing or stale = FAIL; GPL family = FAIL).

Repo root: env WHOOSH_REPO, else this repo. Exit 1 when F > 0 or U > 0 (a check that could not run is not green).
"""
import json, os, re, subprocess, sys

ROOT = os.environ.get("WHOOSH_REPO") or os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BOILER = os.environ.get("AGPL_BOILERPLATE_DIR") or "~/Documents/Doc-Assem/Claude Code/systems/apps/ai-email-app"
SHINGLE = 40
SCAN_DIRS = ("Sources", "core/crates", "apps", "relay", "Tests", "fixtures", "journeys", "tools", "scripts")
SCAN_EXT = (".swift", ".rs", ".py", ".sh", ".json", ".toml", ".md", ".txt")
HEADER_RX = re.compile(r"GNU (?:Affero )?General Public License|\bAGPL\b|\bGPL-[23]", re.I)
SELF = os.path.join("checks", "licence-check.py")
ALNUM_MIN = 25   # a 40-char window with fewer letters/digits is a divider or table rule, not copied prose
norm = lambda t: re.sub(r"\s+", " ", t).strip().lower()


def repo_files():
    for base in SCAN_DIRS:
        for d, ds, fs in os.walk(os.path.join(ROOT, base)):
            ds[:] = [x for x in ds if x not in ("node_modules", "target", ".build", "Generated")]
            for f in fs:
                if f.endswith(SCAN_EXT):
                    p = os.path.join(d, f)
                    yield p, os.path.relpath(p, ROOT)


def windows(t):
    for i in range(0, max(0, len(t) - SHINGLE + 1)):
        w = t[i:i + SHINGLE]
        if sum(c.isalnum() for c in w) >= ALNUM_MIN:
            yield w


def boiler_shingles():
    sh = set()
    n = 0
    for d, ds, fs in os.walk(BOILER):
        ds[:] = [x for x in ds if x not in ("node_modules", ".git", ".next", "dist")]
        for f in fs:
            p = os.path.join(d, f)
            if not f.endswith((".ts", ".tsx", ".md", ".txt", ".json")) or not re.search(r"prompt|/ai/|assistant|agent", p, re.I):
                continue
            n += 1
            t = norm(open(p, encoding="utf-8", errors="replace").read())
            sh.update(hash(w) for w in windows(t))
    return sh, n


# Public standards are facts, not prompt text: a MIME type (application/vnd.openxmlformats-officedocument...) and the markup Gmail
# wraps a quoted reply in are written by everyone who handles mail. A window made of them is not a copied boilerplate line.
STANDARD = re.compile(r"[a-z]+/[a-z0-9][a-z0-9.+-]*|<blockquote[^>]*>", re.I)


def shared_with_boiler(text, sh):
    t = norm(STANDARD.sub(" ", text))
    return any(hash(w) in sh for w in windows(t))


COPYLEFT_RX = re.compile(r"(?<!L)GPL|\bGNU\b|Affero", re.I)


def read_licence_table(path):
    """docs/LICENCES.md rows: | identity | repository | licence | LICENSE file | revision |  ->  {identity: (licence, url, revision)}"""
    rows = {}
    for line in open(path, encoding="utf-8"):
        c = [x.strip() for x in line.strip().strip("|").split("|")]
        if len(c) == 5 and c[0] and not c[0].startswith(("identity", "-")):
            rows[c[0]] = (c[2], c[3], c[4])
    return rows


def swiftpm_results(pins, table_path):
    """Every pin needs a row (licence named, LICENSE url, revision equal to the pin's); a missing pin or a stale revision is red, a GPL-family row is red.
    No pins = nothing to judge (pass). Pins with no table file = red for every pin."""
    if not pins:
        return [("no-swiftpm-pin-without-a-licence-source", "pass", "0 pin(s)")]
    out = []
    table = read_licence_table(table_path) if os.path.exists(table_path) else {}
    missing = [p["identity"] for p in pins if p["identity"] not in table]
    stale = [p["identity"] for p in pins if p["identity"] in table and table[p["identity"]][2] != p["state"].get("revision")]
    unnamed = [i for i, (l, u, r) in table.items() if not l or not u.startswith("https://")]
    out.append(("no-swiftpm-pin-without-a-licence-source", "pass" if not (missing or stale or unnamed) else "fail",
                "%d pin(s), %d row(s); missing: %s; stale revision: %s; no licence or url: %s" % (len(pins), len(table), ", ".join(missing) or "none", ", ".join(stale) or "none", ", ".join(unnamed) or "none")))
    gpl = [p["identity"] for p in pins if p["identity"] in table and COPYLEFT_RX.search(table[p["identity"]][0])]
    ctl = COPYLEFT_RX.search("AGPL-3.0") and COPYLEFT_RX.search("GPL-2.0-only") and not any(COPYLEFT_RX.search(x) for x in ("Apache-2.0 with Runtime Library Exception", "MIT", "LGPL-3.0"))
    out.append(("control-swiftpm-copyleft-matcher", "pass" if ctl else "fail", "AGPL yes, GPL yes, MIT no, Apache no, LGPL no"))
    out.append(("no-gpl-family-swiftpm-dependency", "pass" if not gpl else "fail", "%d: %s" % (len(gpl), ", ".join(gpl))))
    return out


def main():
    res = []   # (label, state, detail) state: pass | fail | unjudged
    lic = os.path.join(ROOT, "LICENSE")
    head = open(lic, encoding="utf-8", errors="replace").read(200) if os.path.exists(lic) else ""
    res.append(("repo-licence-is-mit", "pass" if head.lstrip().startswith("MIT License") else "fail", head[:40].replace("\n", " ")))
    # 1. headers
    hits, n = [], 0
    for p, rel in repo_files():
        if rel == SELF:
            continue
        n += 1
        if HEADER_RX.search(open(p, encoding="utf-8", errors="replace").read()):
            hits.append(rel)
    res.append(("scanner-reads-files", "pass" if n > 0 else "fail", "%d files" % n))
    res.append(("control-header-pattern", "pass" if HEADER_RX.search("Licensed under the GNU Affero General Public License") else "fail", "sample header"))
    res.append(("no-gpl-header-in-tree", "pass" if not hits else "fail", "%d: %s" % (len(hits), ", ".join(hits[:5]))))
    # 2. shared strings
    if not os.path.isdir(BOILER):
        res.append(("no-40-char-string-shared-with-boilerplate", "unjudged", "boilerplate folder not found: %s" % BOILER))
    else:
        sh, nb = boiler_shingles()
        sample = next(iter(sh), None)
        res.append(("boilerplate-prompt-files-found", "pass" if nb > 0 and sample is not None else "fail", "%d files, %d shingles" % (nb, len(sh))))
        ok_ctl = False
        for d, _, fs in os.walk(BOILER):
            for f in fs:
                p = os.path.join(d, f)
                if f.endswith((".ts", ".md")) and re.search(r"prompt", p, re.I):
                    t = open(p, encoding="utf-8", errors="replace").read()
                    if len(norm(t)) > 200:
                        ok_ctl = shared_with_boiler("x " + t[:300] + " y", sh)
                        break
            if ok_ctl:
                break
        res.append(("control-shingle-matcher-finds-a-boilerplate-line", "pass" if ok_ctl else "fail", "first 300 chars of a boilerplate prompt file"))
        shared = [rel for p, rel in repo_files() if rel != SELF and shared_with_boiler(open(p, encoding="utf-8", errors="replace").read(), sh)]
        res.append(("no-40-char-string-shared-with-boilerplate", "pass" if not shared else "fail", "%d: %s" % (len(shared), ", ".join(shared[:5]))))
    # 3. dependencies
    try:
        out = subprocess.run([os.path.expanduser("~/.cargo/bin/cargo") if os.path.exists(os.path.expanduser("~/.cargo/bin/cargo")) else "cargo", "metadata", "--offline", "--locked", "--format-version", "1", "--manifest-path", os.path.join(ROOT, "core/Cargo.toml")],
                             capture_output=True, text=True, timeout=120,
                             env=dict(os.environ, PATH=os.environ.get("PATH", "") + ":" + os.path.expanduser("~/.cargo/bin")))
        if out.returncode != 0:
            raise RuntimeError(out.stderr.strip().splitlines()[-1] if out.stderr.strip() else "cargo metadata failed")
        pk = json.loads(out.stdout)["packages"]
        def copyleft(l):
            alts = [a.strip(" ()") for a in re.split(r"\s+OR\s+|/", l or "")]
            return bool(alts) and all(re.search(r"(?<!L)GPL", a) for a in alts if a)
        bad = [p["name"] for p in pk if copyleft(p.get("license"))]
        res.append(("cargo-metadata-lists-packages", "pass" if len(pk) > 0 else "fail", "%d packages" % len(pk)))
        res.append(("control-copyleft-matcher", "pass" if copyleft("AGPL-3.0-only") and not copyleft("MIT OR GPL-3.0") and not copyleft("MIT") else "fail", "AGPL yes, MIT no, MIT OR GPL no"))
        res.append(("no-gpl-family-cargo-dependency", "pass" if not bad else "fail", "%d: %s" % (len(bad), ", ".join(bad[:5]))))
    except (OSError, RuntimeError, ValueError, subprocess.TimeoutExpired) as e:
        res.append(("no-gpl-family-cargo-dependency", "unjudged", "cargo metadata did not run: %s" % e))
    pr = os.path.join(ROOT, "Package.resolved")
    pins = json.load(open(pr)).get("pins", []) if os.path.exists(pr) else []
    res.extend(swiftpm_results(pins, os.path.join(ROOT, "docs", "LICENCES.md")))
    print("MUTANTS (a check is green only after it has been seen red on its mutant)\n| check | mutant |\n|---|---|")
    for k, v in [("repo-licence-is-mit", "LICENSE starts with GNU text"), ("no-gpl-header-in-tree", "a source file carries a GNU Affero header"),
                 ("no-40-char-string-shared-with-boilerplate", "a boilerplate prompt line is pasted into a source file"),
                 ("no-gpl-family-cargo-dependency", "Cargo.lock gains a GPL-3.0 crate"), ("no-swiftpm-pin-without-a-licence-source", "a pin is added to Package.resolved without a row"), ("no-gpl-family-swiftpm-dependency", "a row names GPL-3.0"), ("control-*", "a matcher is loosened until its own sample passes unseen")]:
        print("| %s | %s |" % (k, v))
    f = [r for r in res if r[1] == "fail"]
    u = [r for r in res if r[1] == "unjudged"]
    for l, s, d in f + u:
        sys.stderr.write("%s %s: %s\n" % ("FAIL" if s == "fail" else "NOT JUDGED", l, d))
    print("%d checks, %d failed, %d not judged" % (len(res), len(f), len(u)))
    return 1 if f or u else 0


if __name__ == "__main__":
    sys.exit(main())
