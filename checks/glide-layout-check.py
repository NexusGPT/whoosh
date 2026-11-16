#!/usr/bin/env python3
"""glide-layout-check: the Swift layout of GLiDE writes the authors' own prompt (SPEC A133, the third family of decision model).

    python3 checks/glide-layout-check.py              compile Sources/Features/OnDeviceAI/Core/GlideLayout.swift alone with a small harness,
                                                      write the prompt of every question of the made-up mails, compare byte for byte
    python3 checks/glide-layout-check.py --selftest   the same on broken copies of the layout: each must turn a case red

The reference (checks/corpora/glide/reference.json) is the output of the authors' own code (the `glide` package of fastino/GLiDE-0.8B, unchanged)
on the made-up mails of checks/corpora/label-reader/cases.json. The harness reads the cases (the raw state and questions), not the reference's
prompts, so the port of the authors' option rules is judged too. This check judges TEXT and the probabilities' arithmetic; the token ids and the
model's numbers are judged on the running app. Four more rows have no line in the reference: their expected text is written here with the
authors' rule beside it. Launches no app and opens no window. Exit 1 when a case fails.
"""
import json
import math
import os
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LAYOUT = os.path.join(ROOT, "Sources", "Features", "OnDeviceAI", "Core", "GlideLayout.swift")
REFERENCE = os.path.join(ROOT, "checks", "corpora", "glide", "reference.json")
CASES = os.path.join(ROOT, "checks", "corpora", "label-reader", "cases.json")
TAIL = "\n\nThink it through briefly, then answer with the option letter only."

HARNESS = r'''
import Foundation

let cases = (try! JSONSerialization.jsonObject(with: FileManager.default.contents(atPath: CommandLine.arguments[1])!) as! [String: Any])["cases"] as! [[String: Any]]
let ref = try! JSONSerialization.jsonObject(with: FileManager.default.contents(atPath: CommandLine.arguments[2])!) as! [String: Any]
let codes = ref["codes"] as! [String]
let temperature = ref["temperature"] as! Double
var out: [[String: Any]] = []
for r in ref["cases"] as! [[String: Any]] {
    let c = cases.first { ($0["id"] as! String) == (r["id"] as! String) }!
    var fields: [String: String] = [:]
    var text = ""
    for (k, v) in c["state"] as! [String: Any] { if k == "text" { text = v as! String } else { fields[k] = (v as! String) } }
    let state = GlideLayout.state(fields: fields, text: text)
    let asked = c["questions"] as! [String: Any]
    for rq in r["questions"] as! [[String: Any]] {
        let id = rq["id"] as! String
        let q = asked[id] as! [String: Any]
        // a question with no words is shown by the authors as the JSON word "null"; the app never asks one
        let ins = (q["instructions"] as? String) ?? "null"
        let question: GlideLayout.Question
        switch q["type"] as! String {
        case "choice":
            let crit = q["criteria"] as! [String: Any]
            // the options in the request's order (a JSON object read here keeps none): the reference's keys give it
            question = .choice(id: id, instructions: ins, options: (rq["keys"] as! [String]).map { (label: $0, description: crit[$0] as? String) })
        case "score": question = .score(id: id, instructions: ins, levels: q["criteria"] as! [String])
        default: question = .noul(id: id, instructions: ins)
        }
        let probs = GlideLayout.probabilities(rq["logits"] as! [Double], temperature: temperature) ?? []
        out.append(["case": c["id"] as! String, "id": id, "prompt": GlideLayout.prompt(state: state, question: question, codes: codes), "keys": question.keys, "probabilities": probs])
    }
}
let end = GlideLayout.body(state: "s", question: .choice(id: "e", instructions: "Pick.", options: [(label: "a", description: nil), (label: "b", description: "last  \n")]), codes: codes)
let empty = GlideLayout.body(state: "s", question: .choice(id: "e", instructions: "Pick.", options: [(label: "a", description: ""), (label: "b", description: "bee")]), codes: codes)
let escapes = GlideLayout.state([(key: "k\"", value: "a\u{01}b\tc\\d\"e\u{1f}\u{7f}\u{e9}\u{2028}\u{08}\u{0c}\r")])
let t2 = GlideLayout.probabilities([1.0, 2.0, 0.5], temperature: 2.0) ?? []
FileHandle.standardOutput.write(try! JSONSerialization.data(withJSONObject: ["rows": out, "end_body": end, "empty_description": empty, "escapes": escapes, "probabilities_at_2": t2]))
'''

# broken copies of the layout: (name, text in the file, its replacement, what the copy does)
MUTANTS = [
    ("last-sentence-reworded", 'answer with the option letter only."', 'answer with the letter."', "the last sentence of the body is not the authors'"),
    ("a-bare-label-shown-twice", r'o.0 == o.1 ? "(\(c)) \(o.0)" : "(\(c)) \(o.0): \(o.1)"', r'"(\(c)) \(o.0): \(o.1)"', "a label with no description is shown as 'label: label'"),
    ("no-space-after-a-key", 'jsonString($0.key) + ": " + jsonString($0.value)', 'jsonString($0.key) + ":" + jsonString($0.value)', "the state's fields are written without the space after the colon"),
    ("a-line-break-kept-raw", r'case 0x0A: out.append(contentsOf: "\\n".unicodeScalars)', 'case 0x0A: out.append(u)', "a line break in a field is not escaped"),
    ("the-empty-thought-dropped", r'<|im_start|>assistant\n<think>\n\n</think>\n\nAnswer: (', r'<|im_start|>assistant\nAnswer: (', "the assistant's turn opens without the empty thought"),
    ("yes-no-in-small-letters", 'descriptions: ["Yes", "No"]', 'descriptions: ["yes", "no"]', "a yes or no question shows 'yes' and 'no'"),
    ("end-white-space-kept", "return trimmedEnd(text) + bodyTail", "return text + bodyTail", "white space after the last option is kept"),
    ("temperature-ignored", "logits.map { exp($0 / temperature - peak) }", "logits.map { exp($0 - peak) }", "the temperature is not applied"),
    ("a-level-shown-without-its-index", "keys: levels.indices.map(String.init), descriptions: levels", "keys: levels, descriptions: levels", "a level of a scale is shown without its index"),
    ("fields-in-reverse-order", "fields.keys.sorted().map", "fields.keys.sorted(by: >).map", "the fields are not in the order of their keys"),
    ("a-control-character-kept-raw", r'case 0x00..<0x20: out.append(contentsOf: ("\\u00" + (u.value < 0x10 ? "0" : "") + String(u.value, radix: 16)).unicodeScalars)', 'case 0x00..<0x20: out.append(u)',
     "a control character in a field is not escaped"),
    ("an-empty-description-shown", "o.description.flatMap { $0.isEmpty ? nil : $0 } ?? o.label", 'o.description ?? o.label', "an empty description is shown after the label"),
]


class Unreadable(Exception):
    pass


def run_layout(layout_text):
    """Compile the layout text with the harness, run it on the cases and the reference, -> the harness's JSON."""
    with tempfile.TemporaryDirectory(prefix="whoosh-glide-layout-") as d:
        lay, main, exe = os.path.join(d, "GlideLayout.swift"), os.path.join(d, "main.swift"), os.path.join(d, "layout")
        open(lay, "w", encoding="utf-8").write(layout_text)
        open(main, "w", encoding="utf-8").write(HARNESS)
        r = subprocess.run(["swiftc", "-O", lay, main, "-o", exe], capture_output=True, text=True, timeout=600)
        if r.returncode != 0:
            raise Unreadable("the layout does not compile alone: %s" % r.stderr.strip()[:600])
        r = subprocess.run([exe, CASES, REFERENCE], capture_output=True, text=True, timeout=120)
        if r.returncode != 0:
            raise Unreadable("the harness stopped (exit %d): %s" % (r.returncode, r.stderr.strip()[:400]))
        return json.loads(r.stdout)


def softmax(v, t):
    v = [x / t for x in v]
    m = max(v)
    e = [math.exp(x - m) for x in v]
    return [x / sum(e) for x in e]


def first_difference(a, b):
    i = next((k for k in range(min(len(a), len(b))) if a[k] != b[k]), min(len(a), len(b)))
    return "at character %d: wrote %r, the authors %r" % (i, a[max(0, i - 20):i + 30], b[max(0, i - 20):i + 30])


def judge(got):
    """-> [(case, ok, detail)]: every prompt of the reference, the keys, the probabilities; then the four rows written here."""
    ref = json.load(open(REFERENCE, encoding="utf-8"))
    rows = {(r["case"], r["id"]): r for r in got["rows"]}
    out = []
    for c in ref["cases"]:
        for q in c["questions"]:
            name = "%s/%s" % (c["id"], q["id"])
            r = rows.get((c["id"], q["id"]))
            if r is None:
                out.append((name, False, "the harness wrote no row")); continue
            same = r["prompt"] == q["prompt"]
            out.append(("prompt " + name, same, "%d characters, equal to the authors'" % len(q["prompt"]) if same else first_difference(r["prompt"], q["prompt"])))
            out.append(("keys " + name, r["keys"] == q["keys"], "keys %r, the authors %r" % (r["keys"], q["keys"])))
            want = softmax(q["logits"], ref["temperature"])
            worst = max((abs(a - b) for a, b in zip(r["probabilities"], want)), default=1.0) if len(r["probabilities"]) == len(want) else 1.0
            theirs = q["answer"].get("probabilities")
            named = worst if theirs is None else max(abs(theirs[k] - p) for k, p in zip(q["keys"], r["probabilities"]))
            if theirs is None and q["type"] == "noul":
                named = abs(q["answer"]["noul"] - r["probabilities"][0])
            out.append(("probabilities " + name, worst < 1e-9 and named < 1e-6, "worst difference to the softmax %.2e, to the authors' answer %.2e" % (worst, named)))
    # the authors' rule: the text of the body, then `.rstrip()`, then the last sentence
    end = "Context:\ns\n\nQuestion: Pick.\n\nOptions:\n(A) a\n(B) b: last  \n".rstrip() + TAIL
    out.append(("white-space-after-the-last-option-is-cut", got["end_body"] == end, "wrote %r" % got["end_body"][-90:]))
    # the authors' rule (options_for): a description that is "" or null is the key itself, and such a line shows the key alone
    empty = "Context:\ns\n\nQuestion: Pick.\n\nOptions:\n(A) a\n(B) b: bee" + TAIL
    out.append(("an-empty-description-shows-the-label-alone", got["empty_description"] == empty, "wrote %r" % got["empty_description"][-90:]))
    # the authors' rule: json.dumps(state, ensure_ascii=False)
    esc = json.dumps({'k"': 'a\x01b\tc\\d"e\x1f\x7fé \x08\x0c\r'}, ensure_ascii=False)
    out.append(("a-field-is-written-as-python-writes-json", got["escapes"] == esc, "wrote %r, python %r" % (got["escapes"], esc)))
    t2 = softmax([1.0, 2.0, 0.5], 2.0)
    worst = max(abs(a - b) for a, b in zip(got["probabilities_at_2"], t2)) if len(got["probabilities_at_2"]) == 3 else 1.0
    out.append(("the-temperature-divides-the-logits", worst < 1e-9, "worst difference %.2e at temperature 2" % worst))
    return out


def report(results):
    for n, ok, d in results:
        print("%-5s %s  %s" % ("PASS" if ok else "FAIL", n, d))
    failed = sum(1 for _, ok, _ in results if not ok)
    print("%d checks, %d failed, 0 not judged" % (len(results), failed))
    return failed


def selftest(text):
    clean = judge(run_layout(text))
    bad = [n for n, ok, _ in clean if not ok]
    survived = []
    for name, old, new, what in MUTANTS:
        if text.count(old) != 1:
            print("NOT MADE  %s: the text to break is in the file %d times" % (name, text.count(old))); survived.append(name); continue
        try:
            red = [n for n, ok, _ in judge(run_layout(text.replace(old, new))) if not ok]
        except Unreadable as e:
            print("NOT MADE  %s: %s" % (name, e)); survived.append(name); continue
        print("%-9s %s (%s): %s" % ("RED" if red else "SURVIVED", name, what, ", ".join(red[:3]) + (" and %d more" % (len(red) - 3) if len(red) > 3 else "")))
        if not red:
            survived.append(name)
    print("selftest glide-layout: the layout %s; %d broken copies, %d red" % ("passes" if not bad else "FAILS %s" % bad[:3], len(MUTANTS), len(MUTANTS) - len(survived)))
    return 0 if not bad and not survived else 1


def main(argv):
    text = open(LAYOUT, encoding="utf-8").read()
    try:
        if "--selftest" in argv:
            return selftest(text)
        return 1 if report(judge(run_layout(text))) else 0
    except Unreadable as e:
        print("NOT JUDGED  %s" % e)
        print("1 checks, 0 failed, 1 not judged")
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
