#!/usr/bin/env python3
"""The app's route for GLiDE on MLX, tried in Python before the Swift port: mlx-lm's own Qwen3.5 text model, unchanged, given for its
full-attention layers a cache object whose mask is "no mask" (so those layers attend in both directions, as GLiDE's authors run them:
decision_config.json attention_mode noncausal_full_attention), then the 255-way readout on the last token's hidden state, in 32-bit numbers.

    python3 -I run-mlx-two-way.py <model folder> <reference.json> <out.json> [--float32] [--causal]

<reference.json> is what run-authors-front-end.py wrote (the authors' own code, every weight in 32-bit numbers). Each question's token ids are
read from it, so the two runs score the same tokens. Prints, per question, the worst difference between the two runs' probabilities, then the
worst of all. --float32 casts the weights to 32-bit numbers first (the published ones are 16-bit). --causal runs the library's usual one-way
mask instead: the control that shows the mask is what makes the answers equal.
"""
import json, math, sys, time

model_dir, ref_path, out_path = sys.argv[1:4]
import mlx.core as mx                     # noqa: E402
from mlx_lm import load                   # noqa: E402
from mlx_lm.models.cache import ArraysCache   # noqa: E402


class TwoWay:
    """A cache entry for a full-attention layer that keeps nothing and asks for no mask: the layer sees every token of the prompt."""
    offset = 0
    def make_mask(self, N, return_array=False, window_size=None): return None
    def update_and_fetch(self, keys, values): return keys, values


def softmax(v, T):
    v = [x / T for x in v]; m = max(v); e = [math.exp(x - m) for x in v]; z = sum(e)
    return [x / z for x in e]


t0 = time.time()
model, tok = load(model_dir)
if "--float32" in sys.argv:
    from mlx.utils import tree_map
    model.update(tree_map(lambda p: p.astype(mx.float32), model.parameters()))
    mx.eval(model.parameters())
lm = model.language_model
readout = mx.load(model_dir + "/readout.safetensors")["weight"].astype(mx.float32)
ref = json.load(open(ref_path))
T = float(ref.get("temperature", 1.0))
load_s = time.time() - t0
out = {"route": "one-way mask (control)" if "--causal" in sys.argv else "two-way cache entries", "float32": "--float32" in sys.argv, "load_s": round(load_s, 2), "cases": []}
worst_all = 0.0
for c in ref["cases"]:
    qs = []
    for q in c["questions"]:
        ids = q["ids"]
        mine = tok.encode(q["prompt"], add_special_tokens=False)
        caches = None if "--causal" in sys.argv else [ArraysCache(size=2) if l.is_linear else TwoWay() for l in lm.layers]
        t = time.time()
        hidden = lm.model(mx.array([ids]), cache=caches)[0, -1].astype(mx.float32)
        logits = readout[: len(q["keys"])] @ hidden
        mx.eval(logits)
        took = time.time() - t
        row = [float(v) for v in logits.tolist()]
        probs = softmax(row, T)
        want = softmax(q["logits"], T)
        worst = max(abs(a - b) for a, b in zip(probs, want))
        worst_all = max(worst_all, worst)
        top_same = probs.index(max(probs)) == want.index(max(want))
        qs.append({"id": q["id"], "tokens": len(ids), "tokenizer_gives_the_same_ids": mine == ids, "logits": row, "probabilities": probs, "authors_probabilities": want,
                   "worst_difference": worst, "same_top_option": top_same, "seconds": round(took, 3)})
        print("%-28s %-4s %5d tokens  %6.2f s  worst difference %.4f  same top option %s  same ids from the tokenizer %s" % (c["id"], q["id"], len(ids), took, worst, top_same, mine == ids), flush=True)
    out["cases"].append({"id": c["id"], "questions": qs})
out["worst_difference"] = worst_all
json.dump(out, open(out_path, "w"), indent=1)
print("route: %s; float32 %s; load %.1f s; worst difference of all %.4f; peak memory %.0f MB; wrote %s" % (out["route"], out["float32"], load_s, worst_all, mx.get_peak_memory() / 1e6, out_path))
