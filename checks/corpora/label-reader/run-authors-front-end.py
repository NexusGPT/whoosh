#!/usr/bin/env python3
"""Reference answers of KnowLine for the app's port: the authors' own front end (knowline_server.py and knowline_layout.py, unchanged, read twice on,
the repository's temperatures.json), with MLX as the scoring backend in place of their transformers one. The output layer is computed in 32-bit
numbers, as the authors' serving setting does (serve_knowline.sh: --enable-fp32-lm-head): the last position's hidden state and the label tokens' rows
of the output weights are cast to float32 before their product. In 16-bit numbers one step of a logit near 20 is 0.125, which moved a yes/no
probability by up to 0.05. The values returned are the label tokens' logits: their log-probabilities up to one constant, which the softmax over the
labels removes.

    run-authors-front-end.py <authors' source folder> <model folder> <cases.json> <out.json> [--long] [--float32]
--float32 casts every weight to 32-bit numbers before the run: the answer without the rounding of the published 16-bit weights (about twice the
memory; the measure the 16-bit runs are judged against, not a setting of the app).
Writes, per case and question: the prompt text (and its sha256), its token ids' count and hash, the label ids read, their log-probabilities, the
probabilities after the temperature, the answer. --long adds a case whose state is over the read-twice limit (read once).
"""
import hashlib, json, sys, time

src, model_dir, cases_path, out_path = sys.argv[1:5]
sys.path.insert(0, src)
import knowline_server as ks            # noqa: E402
from knowline_layout import Layout      # noqa: E402
import mlx.core as mx                   # noqa: E402
from mlx_lm import load                 # noqa: E402
from transformers import AutoTokenizer  # noqa: E402


class MLX:
    """ks.HF's contract on MLX: score(text, media, ids) -> (log-probabilities of ids at the last position, prompt tokens)."""
    def __init__(self, path):
        self.model, _ = load(path)
        if "--float32" in sys.argv:
            from mlx.utils import tree_map
            self.model.update(tree_map(lambda p: p.astype(mx.float32), self.model.parameters()))
            mx.eval(self.model.parameters())
        self.tok = AutoTokenizer.from_pretrained(path)
        self.seen = []
    def warm(self, prefix, media): pass
    def score(self, text, media, ids):
        toks = self.tok.encode(text, add_special_tokens=False)
        lm = self.model.language_model
        hidden = lm.model(mx.array([toks]))[0, -1].astype(mx.float32)
        weight = lm.model.embed_tokens.weight if lm.args.tie_word_embeddings else lm.lm_head.weight
        lp = weight[mx.array(list(ids))].astype(mx.float32) @ hidden
        mx.eval(lp)
        row = [float(v) for v in lp.tolist()]
        self.seen.append({"text": text, "tokens": len(toks), "ids_sha256": hashlib.sha256(json.dumps(toks).encode()).hexdigest(), "first_ids": toks[:12], "last_ids": toks[-12:],
                          "label_ids": list(ids), "logprobs": row})
        return row, len(toks)


t0 = time.time()
back = MLX(model_dir)
temps = json.load(open(model_dir + "/temperatures.json"))
eng = ks.KnowLine(back.tok, back, 1.0, 1, temps, Layout(read_twice=True, twice_max_tokens=3000))
load_s = time.time() - t0
doc = json.load(open(cases_path))
cases = list(doc["cases"])
if "--long" in sys.argv:
    para = "The quarterly report follows. Revenue rose in the north region and fell in the south. Costs stayed flat. "
    cases.append({"id": "over-the-read-twice-limit", "state": {"from": "reports@northfield.example", "subject": "Quarterly report", "text": para * 260},
                  "questions": {"q1": {"type": "noul", "instructions": "The sender asks the reader a direct question or expects an answer."}}})
out = {"model": model_dir.rstrip("/").split("/")[-1], "labels": eng.labels[:30], "label_ids": eng.ids[:30], "temperatures": temps, "load_s": round(load_s, 2), "cases": []}
for c in cases:
    back.seen = []
    t = time.time()
    answers, usage = eng.run(c["state"], c["questions"])
    took = time.time() - t
    qs = []
    for (qid, q), seen in zip(c["questions"].items(), back.seen):
        T = temps.get(q["type"], 1.0)
        qs.append({"id": qid, "type": q["type"], "temperature": T, "prompt_sha256": hashlib.sha256(seen["text"].encode()).hexdigest(), "prompt_chars": len(seen["text"]),
                   "prompt": seen["text"] if len(seen["text"]) < 6000 else seen["text"][:1500] + " …[cut for the file]… " + seen["text"][-1500:],
                   "read": "twice" if ks.__dict__.get("READ_AGAIN_TEXT", "Read the same input again before answering:") in seen["text"] else "once",
                   "tokens": seen["tokens"], "ids_sha256": seen["ids_sha256"], "first_ids": seen["first_ids"], "last_ids": seen["last_ids"],
                   "label_ids": seen["label_ids"], "logprobs": seen["logprobs"], "probabilities": ks.softmax(seen["logprobs"], T), "answer": answers[qid]})
    out["cases"].append({"id": c["id"], "state": c["state"] if len(json.dumps(c["state"])) < 4000 else "(long: built by --long)", "seconds": round(took, 3), "usage": usage, "questions": qs})
    print("%-28s %5.2f s  %s" % (c["id"], took, "; ".join("%s=%s" % (q["id"], json.dumps({k: (round(v, 4) if isinstance(v, float) else v) for k, v in q["answer"].items() if k in ("noul", "choice", "score")})) for q in qs)))
json.dump(out, open(out_path, "w"), indent=1, ensure_ascii=False)
print("load %.1f s; labels %s; wrote %s" % (load_s, " ".join(eng.labels[:8]), out_path))
