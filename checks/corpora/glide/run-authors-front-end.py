#!/usr/bin/env python3
"""Reference answers of GLiDE for the app's port: the authors' own code, unchanged (the `glide` package of the model repository
fastino/GLiDE-0.8B: prompt.py builds the prompt, model.py runs the Qwen3.5 text model with no causal mask in its full-attention layers and
reads the 255-way readout at the last token), on this Mac's processor with every weight in 32-bit numbers: the answer without the rounding
of the published 16-bit weights, the measure the app's 16-bit runs are judged against.

    python3 -I run-authors-front-end.py <model folder> <cases.json> <out.json>

<model folder> holds the repository's files at the pinned revision (config.json, decision_config.json, tokenizer files, the weights, readout.safetensors
and the folder glide/). The cases are made-up mail (checks/corpora/label-reader/cases.json). A state that is an object is shown to the model as the
authors show it: json.dumps(state, ensure_ascii=False).
Writes, per case and question: the prompt (and its sha256), the token ids, the option keys, the readout's logits of the options,
the probabilities, the answer; and once: the two constant parts of the prompt around the body (the chat template's wrapper), the codes.
"""
import hashlib, json, sys, time

model_dir, cases_path, out_path = sys.argv[1:4]
sys.path.insert(0, model_dir)
import torch                              # noqa: E402
from glide import Glide                   # noqa: E402
from glide.prompt import options_for      # noqa: E402

t0 = time.time()
g = Glide.from_pretrained(model_dir, device="cpu", dtype=torch.float32, vision=False)
load_s = time.time() - t0
seen = []
inner = g._logits
def spy(seqs, vis):
    rows = inner(seqs, vis)
    seen.extend(zip(seqs, rows))
    return rows
g._logits = spy

doc = json.load(open(cases_path))
out = {"model": g.name, "revision_folder": model_dir.rstrip("/").split("/")[-1], "dtype": "float32", "device": "cpu", "temperature": g.temperature,
       "codes": g.codes[:30], "wrapper_before_the_body": g.prompter.pre, "wrapper_after_the_body": g.prompter.post, "load_s": round(load_s, 2), "cases": []}
for c in doc["cases"]:
    del seen[:]
    t = time.time()
    r = g.decide(c["state"], c["questions"])
    took = time.time() - t
    qs = []
    for (qid, q), (ids, row) in zip(c["questions"].items(), seen):
        opts = options_for(q)
        text = g.prompter.text(c["state"], q, opts)
        assert g.tokenizer(text, add_special_tokens=False)["input_ids"] == ids, "the prompt read back differs from the one scored"
        n = len(opts)
        qs.append({"id": qid, "type": q["type"], "keys": [k for k, _ in opts], "prompt_sha256": hashlib.sha256(text.encode()).hexdigest(), "prompt_chars": len(text),
                   "prompt": text, "tokens": len(ids), "ids_sha256": hashlib.sha256(json.dumps(ids).encode()).hexdigest(), "ids": ids,
                   "logits": [float(v) for v in row[:n]], "answer": r["answers"][qid]})
    out["cases"].append({"id": c["id"], "state": c["state"], "seconds": round(took, 3), "usage": r["usage"], "questions": qs})
    print("%-28s %6.2f s  %s" % (c["id"], took, "; ".join("%s=%s" % (q["id"], json.dumps({k: (round(v, 4) if isinstance(v, float) else v) for k, v in q["answer"].items() if k in ("noul", "choice", "score", "confidence")})) for q in qs)), flush=True)
json.dump(out, open(out_path, "w"), indent=1, ensure_ascii=False)
print("load %.1f s; wrote %s" % (load_s, out_path))
