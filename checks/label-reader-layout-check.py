#!/usr/bin/env python3
"""label-reader-layout-check: the Swift layout of a label reader writes the authors' own text (SPEC A133, the second family of decision model).

    python3 checks/label-reader-layout-check.py              compile Sources/Features/OnDeviceAI/Core/LabelReaderLayout.swift alone with a small
                                                             harness, write every prompt of the reference's made-up mails, compare byte for byte
    python3 checks/label-reader-layout-check.py --selftest   the same on broken copies of the layout: each must turn a case red

The reference (checks/corpora/label-reader/reference.json) is the output of the authors' own front end (knowline_server.py and knowline_layout.py of
PelaAI/KnowLine-0.8B-Gen1, unchanged) on made-up mail. This check judges the TEXT of a prompt: the state, the question block, the turns read once or
twice, and the probabilities' arithmetic. The harness joins the turns as the model's chat template does for user turns (each turn's text trimmed,
thinking off); the template itself, the token ids and the model's numbers are judged on the running app by checks/label-reader-real-check.py.
Launches no app and opens no window. Exit 1 when a case fails.
"""
import hashlib
import json
import os
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LAYOUT = os.path.join(ROOT, "Sources", "Features", "OnDeviceAI", "Core", "LabelReaderLayout.swift")
REFERENCE = os.path.join(ROOT, "checks", "corpora", "label-reader", "reference.json")

HARNESS = r'''
import Foundation

func pyStrip(_ s: String) -> String {
    func space(_ u: Unicode.Scalar) -> Bool { u.properties.isWhitespace || (0x1C...0x1F).contains(u.value) }
    var a = Array(s.unicodeScalars)
    while let l = a.last, space(l) { a.removeLast() }
    while let f = a.first, space(f) { a.removeFirst() }
    var v = String.UnicodeScalarView(); v.append(contentsOf: a)
    return String(v)
}
/// The model's chat template for user turns with thinking off: each turn's text trimmed between the two marks, then the assistant's opening.
func chatml(_ turns: [[String: String]]) -> String {
    turns.map { "<|im_start|>\($0["role"]!)\n" + pyStrip($0["content"]!) + "<|im_end|>\n" }.joined()
        + "<|im_start|>assistant\n<think>\n\n</think>\n\n" + LabelReaderLayout.answer
}
let data = FileManager.default.contents(atPath: CommandLine.arguments[1])!
let doc = try! JSONSerialization.jsonObject(with: data) as! [String: Any]
let labels = LabelReaderLayout.labelCandidates
var out: [[String: Any]] = []
for c in doc["cases"] as! [[String: Any]] {
    var fields: [String: String] = [:]
    var text = ""
    for pair in c["state"] as! [[Any]] {
        let k = pair[0] as! String
        var v = ""
        if let s = pair[1] as? String { v = s }
        else if let r = pair[1] as? [String: Any] { v = String(repeating: r["repeat"] as! String, count: r["times"] as! Int) }
        if k == "text" { text = v } else { fields[k] = v }
    }
    let state = LabelReaderLayout.state(fields: fields, text: text)
    for q in c["questions"] as! [[String: Any]] {
        let id = q["id"] as! String
        let ins = q["instructions"] as? String
        let question: LabelReaderLayout.Question
        switch q["kind"] as! String {
        case "choice":
            question = .choice(id: id, instructions: ins, options: (q["options"] as! [[Any]]).map { (label: $0[0] as! String, description: $0[1] as? String) })
        case "score": question = .score(id: id, instructions: ins, levels: q["levels"] as! [String])
        default: question = .noul(id: id, instructions: ins)
        }
        let block = LabelReaderLayout.block(question, labels: labels)
        let turns = (q["read"] as! String) == "twice" ? LabelReaderLayout.twice(state: state, block: block) : LabelReaderLayout.once(state: state, block: block)
        let probs = LabelReaderLayout.probabilities(q["logprobs"] as! [Double], temperature: q["temperature"] as! Double) ?? []
        out.append(["case": c["id"] as! String, "id": id, "prompt": chatml(turns), "keys": question.keys, "probabilities": probs,
                    "confidence": LabelReaderLayout.confidence(probs)])
    }
}
let endBlock = LabelReaderLayout.block(.choice(id: "e", instructions: "Pick.", options: [(label: "a", description: nil), (label: "b", description: "last  \n")]), labels: labels)
FileHandle.standardOutput.write(try! JSONSerialization.data(withJSONObject: ["rows": out, "end_block": endBlock, "once_tokens": LabelReaderLayout.onceTokens(prefix: 100, prompts: [130, 150, 125]),
                                                                              "candidates": Array(labels.prefix(30)), "candidate_count": labels.count]))
'''

# broken copies of the layout: (name, text in the file, its replacement, what the copy does)
MUTANTS = [
    ("instruction-reworded", "Pick exactly one option and reply with its label only.", "Pick one option and reply with its label.", "the instruction is not the authors' sentence"),
    ("second-read-not-introduced", 'public static let readAgain = "Read the same input again before answering:\\n"', 'public static let readAgain = ""', "the second copy of the state is not introduced"),
    ("lines-not-indented", 'map { "  " + $0 }', 'map { "" + $0 }', "a value of several lines is not indented under its key"),
    ("end-white-space-kept", '+ trimmedEnd(lines)', '+ lines', "white space after the last option is kept"),
    ("yes-no-shown-bare", 'texts: ["Yes: yes", "No: no"]', 'texts: ["yes", "no"]', "a yes or no question shows 'yes' and 'no' without 'Yes:' and 'No:'"),
    ("fields-in-arrival-order", "fields.keys.sorted().map", "fields.keys.sorted(by: >).map", "the fields are not in the order of their keys"),
    ("temperature-ignored", "exp(($0 - peak) / temperature)", "exp($0 - peak)", "the score question's temperature is not applied"),
    ("a-score-level-shown-bare", '"\\($0.offset): \\($0.element)"', '"\\($0.element)"', "a level of a scale is shown without its index"),
    ("a-last-empty-line-kept", "if open { lines.append(String(current)) }", "lines.append(String(current))", "a text that ends with a line break gets an empty last line"),
]


class Unreadable(Exception):
    pass


def run_layout(layout_text):
    """Compile the layout text with the harness, run it on the reference, -> the harness's JSON."""
    with tempfile.TemporaryDirectory(prefix="whoosh-label-reader-") as d:
        lay, main, exe = os.path.join(d, "LabelReaderLayout.swift"), os.path.join(d, "main.swift"), os.path.join(d, "layout")
        open(lay, "w", encoding="utf-8").write(layout_text)
        open(main, "w", encoding="utf-8").write(HARNESS)
        r = subprocess.run(["swiftc", "-O", lay, main, "-o", exe], capture_output=True, text=True, timeout=300)
        if r.returncode != 0:
            raise Unreadable("the layout does not compile alone: %s" % r.stderr.strip()[:600])
        r = subprocess.run([exe, REFERENCE], capture_output=True, text=True, timeout=120)
        if r.returncode != 0:
            raise Unreadable("the harness stopped (exit %d): %s" % (r.returncode, r.stderr.strip()[:400]))
        return json.loads(r.stdout)


def judge(got, ref):
    """-> [(name, ok, detail)]: one case per prompt, then the arithmetic."""
    rows = []
    by = {(r["case"], r["id"]): r for r in got["rows"]}
    wrong_p, n_p = [], 0
    for c in ref["cases"]:
        for q in c["questions"]:
            g = by.get((c["id"], q["id"]))
            name = "prompt/%s/%s" % (c["id"], q["id"])
            if g is None:
                rows.append((name, False, "the harness wrote no prompt")); continue
            sha = hashlib.sha256(g["prompt"].encode("utf-8")).hexdigest()
            ok = sha == q["prompt_sha256"] and len(g["prompt"]) == q["prompt_chars"]
            detail = "%d characters, read %s, sha256 %s" % (len(g["prompt"]), q["read"], sha[:12])
            if not ok:
                want = q.get("prompt")
                if want is not None:
                    i = next((k for k, (a, b) in enumerate(zip(g["prompt"], want)) if a != b), min(len(g["prompt"]), len(want)))
                    detail = "differs at character %d: wrote %r, the authors wrote %r" % (i, g["prompt"][max(0, i - 30):i + 40], want[max(0, i - 30):i + 40])
                else:
                    detail = "wrote %d characters sha256 %s, the authors %d characters sha256 %s" % (len(g["prompt"]), sha[:12], q["prompt_chars"], q["prompt_sha256"][:12])
            rows.append((name, ok, detail))
            n_p += 1
            if len(g["probabilities"]) != len(q["probabilities"]) or any(abs(a - b) > 1e-9 for a, b in zip(g["probabilities"], q["probabilities"])):
                wrong_p.append("%s/%s wrote %s, the authors %s" % (c["id"], q["id"], [round(x, 6) for x in g["probabilities"]], [round(x, 6) for x in q["probabilities"]]))
            conf = q["answer"].get("confidence")
            if conf is not None and abs(conf - g["confidence"]) > 1e-9:
                wrong_p.append("%s/%s confidence %.6f, the authors %.6f" % (c["id"], q["id"], g["confidence"], conf))
    rows.append(("probabilities-from-the-letters", not wrong_p, "%d questions, wrong: %s" % (n_p, "; ".join(wrong_p[:3]) or "none")))
    # once_tokens of the authors' render_request: p + sum(n - p) with p 100 and prompts of 130, 150 and 125 tokens = 205
    rows.append(("once-tokens-count-the-shared-start-once", got.get("once_tokens") == 205, "wrote %r, wanted 205" % got.get("once_tokens")))
    # the authors' `lines.rstrip()`: white space after the last option's text is cut (the model's chat template trims a turn too, so the prompts
    # above cannot show it: it is judged on the block)
    want_end = "Question: Pick.\nOptions:\nA. a\nB. b: last"
    rows.append(("the-block-ends-on-its-last-option", got.get("end_block") == want_end, "wrote %r, wanted %r" % (got.get("end_block"), want_end)))
    cand = got.get("candidates") or []
    rows.append(("letters-are-a-to-z-then-aa", cand[:27] == [chr(65 + i) for i in range(26)] + ["AA"] and got.get("candidate_count") == 702,
                 "first %s, then %s; %s in all" % (" ".join(cand[:3]), " ".join(cand[26:28]), got.get("candidate_count"))))
    return rows


def main(argv):
    ref = json.load(open(REFERENCE, encoding="utf-8"))
    layout = open(LAYOUT, encoding="utf-8").read()
    try:
        rows = judge(run_layout(layout), ref)
    except Unreadable as e:
        print("NOT JUDGED %s" % e)
        print("0 checks, 0 failed, 1 not judged")
        return 1
    failed = [r for r in rows if not r[1]]
    for name, ok, detail in rows:
        print("%s %s  %s" % ("PASS" if ok else "FAIL", name, detail))
    if "--selftest" not in argv:
        print("%d checks, %d failed, 0 not judged" % (len(rows), len(failed)))
        return 1 if failed else 0
    red = 0
    for name, old, new, what in MUTANTS:
        if layout.count(old) != 1:
            print("NO MUTANT %s: its text is in the layout %d times, not once" % (name, layout.count(old))); continue
        try:
            bad = [r for r in judge(run_layout(layout.replace(old, new)), ref) if not r[1]]
        except Unreadable as e:
            print("NO MUTANT %s: the broken copy was not judged: %s" % (name, e)); continue
        if bad:
            red += 1
            print("broken copy %s (%s) RED on %d cases, first %s: %s" % (name, what, len(bad), bad[0][0], bad[0][2][:150]))
        else:
            print("NO MUTANT %s (%s): every case stayed green" % (name, what))
    print("%d checks, %d failed, 0 not judged; %d broken copies, %d red" % (len(rows), len(failed), len(MUTANTS), red))
    return 1 if failed or red != len(MUTANTS) else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
