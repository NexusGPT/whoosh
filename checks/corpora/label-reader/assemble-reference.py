#!/usr/bin/env python3
"""Assembles checks/corpora/label-reader/reference.json from the output of run-authors-front-end.py.

    1. python run-authors-front-end.py <folder with the authors' knowline_server.py and knowline_layout.py> <model folder> cases.json out.json --long
       (a Python with mlx-lm, transformers and requests; the model folder is PelaAI/KnowLine-0.8B-Gen1 at the revision below, checked against
       its SHA256SUMS)
    2. the same run with --float32 added, to out-f32.json: the answers without the rounding of the 16-bit weights
    3. python3 assemble-reference.py <authors' source folder> out.json cases.json reference.json out-f32.json [<repository> <revision>]
The long case's text is stored as a sentence and a count, and its prompt as a hash: the file stays small.
"""
import hashlib, json, os, sys

src, out_path, cases_path, ref_path, exact_path = sys.argv[1:6]
PARA = "The quarterly report follows. Revenue rose in the north region and fell in the south. Costs stayed flat. "
ref = json.load(open(out_path))
exact = {(c["id"], q["id"]): q for c in json.load(open(exact_path))["cases"] for q in c["questions"]}
cases = {c["id"]: c for c in json.load(open(cases_path))["cases"]}
cases["over-the-read-twice-limit"] = {"id": "over-the-read-twice-limit", "state": {"from": "reports@northfield.example", "subject": "Quarterly report", "text": PARA * 260},
                                      "questions": {"q1": {"type": "noul", "instructions": "The sender asks the reader a direct question or expects an answer."}}}
out = {
    "what": "Reference of the label-reader layout and answers: the authors' own front end (knowline_server.py, knowline_layout.py, unchanged; read twice on; the repository's temperatures.json) on made-up mail, with MLX (mlx-lm) as the scoring backend. No real mail.",
    "model": sys.argv[6] if len(sys.argv) > 7 else "PelaAI/KnowLine-0.8B-Gen1", "revision": sys.argv[7] if len(sys.argv) > 7 else "01952e55d70c9cc087db25656cafb125590ea4a6",
    "sources_sha256": {f: hashlib.sha256(open(os.path.join(src, f), "rb").read()).hexdigest() for f in ("knowline_server.py", "knowline_layout.py")},
    "made_with": "run-authors-front-end.py, 15 Nov 2026: mlx-lm 0.32.2, mlx 0.32.4, transformers 5.23.0, tokenizers 0.23.3",
    "pass_line": {"probability_gap_to_exact": 0.03, "why": "set on 15 Nov 2026 from the measure of the weights' own rounding: against the answers of the whole model in 32-bit numbers (exact_probabilities), the authors' front end on the published 16-bit weights is at most 0.019 away and most questions under 0.006; a runner on the same weights that is farther than 0.03 from the exact answer, or whose token ids differ, does not run the model as its authors do."},
    "labels": ref["labels"], "label_ids": ref["label_ids"], "temperatures": ref["temperatures"], "cases": []}
for c in ref["cases"]:
    src_case = cases[c["id"]]
    st = src_case["state"]
    long = c["id"] == "over-the-read-twice-limit"
    state = [[k, v] for k, v in st.items()]
    if long:
        state = [[k, v] for k, v in st.items() if k != "text"] + [["text", {"repeat": PARA, "times": 260}]]
    qs = []
    for q in c["questions"]:
        sq = src_case["questions"][q["id"]]
        row = {"id": q["id"], "kind": q["type"], "instructions": sq.get("instructions")}
        if q["type"] == "choice": row["options"] = [[k, v] for k, v in sq["criteria"].items()]
        if q["type"] == "score": row["levels"] = list(sq["criteria"])
        row.update({"read": q["read"], "temperature": q["temperature"], "prompt_sha256": q["prompt_sha256"], "prompt_chars": q["prompt_chars"],
                    "tokens": q["tokens"], "ids_sha256": q["ids_sha256"], "first_ids": q["first_ids"], "last_ids": q["last_ids"], "label_ids": q["label_ids"],
                    "logprobs": q["logprobs"], "probabilities": q["probabilities"], "answer": q["answer"]})
        e = exact[(c["id"], q["id"])]
        assert e["ids_sha256"] == q["ids_sha256"], "the two runs did not read the same tokens"
        row["exact_probabilities"] = e["probabilities"]
        if not long: row["prompt"] = q["prompt"]
        qs.append(row)
    out["cases"].append({"id": c["id"], "state": state, "questions": qs})
json.dump(out, open(ref_path, "w", encoding="utf-8"), indent=1, ensure_ascii=False)
open(ref_path, "a").write("\n")
print("reference:", len(out["cases"]), "cases,", sum(len(c["questions"]) for c in out["cases"]), "questions")
