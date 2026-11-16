#!/usr/bin/env python3
"""The catalogue of on-device decision models (SPEC A133, Sources/Features/OnDeviceAI/Core/DecisionModelCatalogue.swift), read from the sources alone.

    python3 checks/decision-models-check.py             judge the tree
    python3 checks/decision-models-check.py --selftest   the tree is green and each broken copy is red on its own case

Cases: every line of the catalogue holds every field (an id, a name, a family the app knows, a size, a licence, a page, an address, files, its folders);
ids are unique and `all` lists every line; each size is a ledger number (docs/CONSTANTS.md); the download code holds no second copy of a line's address or
files (it reads the catalogue)."""
import os, re, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CAT = "Sources/Features/OnDeviceAI/Core/DecisionModelCatalogue.swift"
STORE = "Sources/Features/OnDeviceAI/Core/LayaModelStore.swift"
LEDGER = "docs/CONSTANTS.md"
TABLE = "research/decision-models-on-device.md"
PORT = "Sources/Features/OnDeviceAI/Contract/DecisionModels.swift"
# one adapter per family (SPEC A133): the files that may name each family's engine in code. Every other file reaches a model through the port.
ENGINES = {"LayaEngine": ("OnDeviceAI/Core/LayaEngine.swift", "OnDeviceAI/Contract/LayaOnDevice.swift", "OnDeviceAI/Contract/LayaDecisionPort.swift"),
           "LabelReaderEngine": ("OnDeviceAI/Core/LabelReaderEngine.swift", "OnDeviceAI/Contract/DecisionModels.swift"),
           "GlideEngine": ("OnDeviceAI/Core/GlideEngine.swift", "OnDeviceAI/Contract/DecisionModels.swift")}
FIELDS = ("id", "name", "indexRow", "family", "parametersBillions", "downloadMB", "licence", "page", "score", "scoreEdition", "base", "files", "checked", "folder", "checkpoint")


def read(rel):
    return open(os.path.join(ROOT, rel), encoding="utf-8").read()


def features():
    """Every Swift file under Sources/Features, by its path inside that folder."""
    out, base = {}, os.path.join(ROOT, "Sources", "Features")
    for dp, _, fn in os.walk(base):
        for f in fn:
            if f.endswith(".swift"):
                p = os.path.join(dp, f)
                out[os.path.relpath(p, base)] = open(p, encoding="utf-8").read()
    return out


def entries(cat):
    """[(swift name, {field: source text})] for each `static let x = DecisionModelEntry(...)`."""
    out = []
    for m in re.finditer(r"public static let (\w+) = DecisionModelEntry\((.*?)\n\s*checkpoint: (\"[^\"]*\")\)", cat, re.S):
        body = m.group(2) + "\n        checkpoint: " + m.group(3)
        fields = {}
        for f in FIELDS:
            fm = re.search(r"^\s*%s: (.*?)(?=,\n\s*\w+: |\Z)" % f, body, re.S | re.M)
            if fm:
                fields[f] = fm.group(1).strip().rstrip(",")
        out.append((m.group(1), fields))
    return out


def judge(src):
    cat, store, ledger, table = src[CAT], src[STORE], src[LEDGER], src[TABLE]
    es = entries(cat)
    families = set(re.findall(r"^\s*case (\w+)\s*$", cat.split("public enum DecisionModelFamily")[1].split("}")[0], re.M))
    rows = []
    rows.append(("the-catalogue-holds-at-least-one-model", len(es) >= 1, "%d lines" % len(es)))
    lacking = []
    for name, f in es:
        for k in FIELDS:
            v = f.get(k)
            empty = v is None or v in ('""', "[]") or (k in ("id", "name", "indexRow", "licence", "folder") and not re.fullmatch(r'"[^"]+"', v or ""))
            if k == "checkpoint": empty = v is None                                    # an empty checkpoint is the model's folder itself
            if k == "checked": empty = v is None                                       # an empty list: the download is checked by size alone
            if k == "page" and not re.fullmatch(r'"https://[^"]+"', v or ""): empty = True
            if k == "base" and not re.fullmatch(r'URL\(string: "https://[^"]+/"\)!', v or ""): empty = True
            if k == "files" and not re.findall(r'"[^"]+"', v or ""): empty = True
            if k == "family" and (v or "").lstrip(".") not in families: empty = True
            if k == "parametersBillions" and not re.fullmatch(r"\d+(\.\d+)?", v or "") : empty = True
            if k == "parametersBillions" and v and float(v) <= 0: empty = True
            if k in ("score", "scoreEdition") and v is not None: empty = False       # nil is a value: the line then shows no score
            if empty: lacking.append("%s.%s" % (name, k))
    rows.append(("every-line-holds-every-field", not lacking, "lacking: %s" % (", ".join(lacking) or "none")))
    scored = [n for n, f in es if (f.get("score") == "nil") != (f.get("scoreEdition") == "nil")]
    rows.append(("a-score-names-its-edition", not scored, "a score without its edition, or the reverse: %s" % (", ".join(scored) or "none")))
    # a score is the index's Full score of that model in the research table (| rank | model | parameters | Full | ...), and its edition the table's date
    wrong = []
    stamp = re.search(r"last modified (\d{4}-\d{2}-\d{2})", table)
    for name, f in es:
        if f.get("score") in (None, "nil"): continue
        shown = (f.get("indexRow") or "").strip('"')
        row = re.search(r"^\| \d+ \| %s \| [\d.]+ \| ([\d.]+) \|" % re.escape(shown), table, re.M)
        if not row or abs(float(row.group(1)) - float(f["score"])) > 0.005 or not stamp or (f.get("scoreEdition") or "").strip('"') != stamp.group(1):
            wrong.append("%s (catalogue %s on %s, table %s on %s)" % (shown, f.get("score"), f.get("scoreEdition"), row.group(1) if row else "no row", stamp.group(1) if stamp else "no date"))
    rows.append(("scores-are-the-index-tables", not wrong, "not the table's figure: %s" % (", ".join(wrong) or "none")))
    ids = [f.get("id") for _, f in es]
    rows.append(("ids-are-unique", len(set(ids)) == len(ids), "ids %s" % ids))
    listed = re.search(r"public static let all: \[DecisionModelEntry\] = \[([^\]]*)\]", cat)
    names = [x.strip() for x in (listed.group(1).split(",") if listed else []) if x.strip()]
    # a line is either shown (`all`) or held for the owner's word (`held`), never both and never neither
    kept = re.search(r"public static let held: \[DecisionModelEntry\] = \[([^\]]*)\]", cat)
    held = [x.strip() for x in (kept.group(1).split(",") if kept else []) if x.strip()]
    rows.append(("all-lists-every-line", sorted(names + held) == sorted(n for n, _ in es), "all = %s, held = %s, lines = %s" % (names, held, [n for n, _ in es])))
    # a held line waits for the owner's word: it is not in the list, and its licence is not an open one the list could show without asking
    both = sorted(set(names) & set(held))
    rows.append(("a-held-line-is-not-in-the-list", not both, "both shown and held: %s" % (both or "none")))
    sizes = []
    for name, f in es:
        m = re.fullmatch(r"WhooshConstants\.(\w+)", f.get("downloadMB") or "")
        if not m or not re.search(r"^\| %s \| \d+ \|" % m.group(1), ledger, re.M): sizes.append(name)
    rows.append(("each-size-is-a-ledger-number", not sizes, "not a ledger number: %s" % (", ".join(sizes) or "none")))
    # a line whose download is checked against checksums: its checked files are its files, in order, each with a size and a SHA-256, and their
    # sizes add up to the line's ledger size
    bad = []
    for name, f in es:
        chk = re.findall(r'ModelFile\(path: "([^"]+)", bytes: ([\d_]+), sha256: "([0-9a-f]*)"\)', f.get("checked") or "")
        if not chk: continue
        files = re.findall(r'"([^"]+)"', f.get("files") or "")
        m = re.fullmatch(r"WhooshConstants\.(\w+)", f.get("downloadMB") or "")
        led = re.search(r"^\| %s \| (\d+) \|" % m.group(1), ledger, re.M) if m else None
        total = sum(int(b.replace("_", "")) for _, b, _ in chk)
        if [c[0] for c in chk] != files: bad.append("%s: the checked files are not the files, in order" % name)
        if any(len(c[2]) != 64 for c in chk): bad.append("%s: a checksum is not 64 hex characters" % name)
        if not led or round(total / 1e6) != int(led.group(1)): bad.append("%s: the files weigh %d MB, the ledger says %s" % (name, round(total / 1e6), led.group(1) if led else "nothing"))
    rows.append(("checked-files-match-the-line", not bad, "%s" % ("; ".join(bad) or "every checked line agrees with its files and its ledger size")))
    # one adapter per family: the port's switch names every family, and an engine is named in code by its own family's files alone
    port = src[PORT]
    switch = port[port.find("final class ChosenDecisionPort"):]
    missing = [f for f in sorted(families) if not re.search(r"case \.%s:" % f, switch)]
    strays = []
    for engine, allowed in ENGINES.items():
        for rel, text in src["__features__"].items():
            if rel in allowed: continue
            for n, line in enumerate(text.split("\n"), 1):
                code = line.split("//")[0]
                if re.search(r"\b%s\b" % engine, code): strays.append("%s:%d names %s" % (rel, n, engine))
    rows.append(("one-adapter-per-family", not missing and not strays and len(ENGINES) == len(families),
                 "families the port's switch lacks: %s; engines named outside their adapter: %s; %d engines for %d families" % (missing or "none", strays[:3] or "none", len(ENGINES), len(families))))
    copies = [a for a in re.findall(r'"https://[^"]+"', cat) if a in store] + [x for _, f in es for x in re.findall(r'"[^"]+\.(?:safetensors|json)"', f.get("files") or "") if x in store]
    rows.append(("the-download-code-holds-no-second-copy", not copies and "DecisionModelCatalogue." in store, "second copies in %s: %s" % (STORE.rsplit("/", 1)[1], copies or "none")))
    return rows


MUTANTS = [
    ("every-line-holds-every-field", CAT, 'licence: "Apache-2.0",', 'licence: "",'),
    ("every-line-holds-every-field", CAT, "family: .laya,", "family: .unknown,"),
    ("a-score-names-its-edition", CAT, 'scoreEdition: "2026-11-12",', "scoreEdition: nil,"),
    ("scores-are-the-index-tables", CAT, "score: 9.21,", "score: 28.3,"),
    ("ids-are-unique", CAT, "public static let all:", 'public static let twin = DecisionModelEntry(\n        id: "laya",\n        name: "Twin",\n        family: .laya,\n        parametersBillions: 1,\n        downloadMB: WhooshConstants.DECISION_MODEL_LAYA_MB,\n        licence: "x",\n        page: "https://example.com/x",\n        score: nil,\n        scoreEdition: nil,\n        base: URL(string: "https://example.com/x/")!,\n        files: ["a.json"],\n        folder: "twin",\n        checkpoint: "c")\n\n    public static let all:'),
    ("one-adapter-per-family", "Sources/Features/SettingsAIMac/AISettingsStore.swift", "    enum Speech: String { case none, downloading, ready, failed }", "    enum Speech: String { case none, downloading, ready, failed }\n    var strayEngine: LayaEngine? { nil }"),
    ("one-adapter-per-family", PORT, "        case .labelReader: return try models.decide(with: entry, state: state, questions: questions)", "        default: return try models.decide(with: entry, state: state, questions: questions)"),
    ("all-lists-every-line", CAT, "= [laya, knowline08]", "= [laya]"),
    ("all-lists-every-line", CAT, "= [glide08]", "= []"),
    ("a-held-line-is-not-in-the-list", CAT, "= [laya, knowline08]", "= [laya, knowline08, glide08]"),
    ("one-adapter-per-family", PORT, "        case .glide: return try models.decide(with: entry, state: state, questions: questions)\n", ""),
    ("checked-files-match-the-line", CAT, 'ModelFile(path: "temperatures.json", bytes: 17,', 'ModelFile(path: "temperatures.json", bytes: 17_000_000,'),
    ("checked-files-match-the-line", CAT, 'ModelFile(path: "vocab.json",', 'ModelFile(path: "vocab.txt",'),
    ("each-size-is-a-ledger-number", CAT, "downloadMB: WhooshConstants.DECISION_MODEL_LAYA_MB,", "downloadMB: 843,"),
    ("the-download-code-holds-no-second-copy", STORE, "public static let defaultBase = DecisionModelCatalogue.laya.base", 'public static let defaultBase = URL(string: "https://huggingface.co/convaiinnovations/laya/resolve/main/")!'),
]


def main():
    src = {p: read(p) for p in (CAT, STORE, LEDGER, TABLE, PORT)}
    src["__features__"] = features()
    rows = judge(src)
    bad = [r for r in rows if not r[1]]
    for name, ok, why in rows:
        print("%s %s  %s" % ("PASS" if ok else "FAIL", name, why))
    if "--selftest" in sys.argv:
        n = 0
        for case, path, find, repl in MUTANTS:
            feat = path[len("Sources/Features/"):] if path.startswith("Sources/Features/") else None
            base = src[path] if path in src else src["__features__"][feat]
            assert find in base, (case, find)
            broken = dict(src)
            if path in src: broken[path] = base.replace(find, repl, 1)
            if feat: broken["__features__"] = dict(src["__features__"], **{feat: base.replace(find, repl, 1)})
            got = {r[0]: r[1] for r in judge(broken)}
            red = got.get(case) is False
            n += red
            print("broken copy (%s -> %s...) %s on %s" % (find[:38].replace("\n", " "), repl[:24].replace("\n", " "), "RED" if red else "STILL GREEN", case))
        print("%d checks, %d failed, 0 not judged; %d broken copies, %d red" % (len(rows), len(bad), len(MUTANTS), n))
        return 0 if not bad and n == len(MUTANTS) else 1
    print("%d checks, %d failed, 0 not judged" % (len(rows), len(bad)))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
