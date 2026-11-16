# GLiDE: reference answers on made-up mail

Test data of the GLiDE adapter (SPEC A133, board item 6da51e83). No real mail: the cases are `checks/corpora/label-reader/cases.json`.

| File | What it is |
|---|---|
| `run-authors-front-end.py` | Runs the authors' own code (the `glide` package of fastino/GLiDE-0.8B, unchanged) on this Mac's processor, every weight in 32-bit numbers. Writes `reference.json`. |
| `reference.json` | Per question: the prompt, its token ids, the option keys, the readout's logits, the answer. Once: the fixed text before and after the body. Model revision 9eacdeec79fa48ec52a8d6f54b331376b4bbfaf0. |
| `run-mlx-two-way.py` | The app's route, in Python: mlx-lm's Qwen3.5 text model, unchanged, with a cache entry whose mask is "no mask" for the six full-attention layers, then the 255-way readout on the last token. Compares with `reference.json`. |
| `mlx-route-trial.json` | The three runs of 15 Nov 2026, per question. |
| `swift-engine-main192.txt` | The app's own engine (`GlideEngine.swift` and `GlideLayout.swift`, unchanged), compiled alone against the packages of build main192 and run on `reference.json`: one line per question, then the totals. |

## How the model reads a question (from the authors' files)

- One prompt per question: `<|im_start|>user\n` + `Context:\n<state>\n\nQuestion: <instructions>\n\nOptions:\n(A) key: description…` + `\n\nThink it through briefly, then answer with the option letter only.` + `<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\nAnswer: (`.
- A state that is an object is shown as `json.dumps(state, ensure_ascii=False)`.
- The Qwen3.5-0.8B text model (24 layers: 18 linear-attention, 6 full-attention). The full-attention layers run with no causal mask (`decision_config.json`: `attention_mode` `noncausal_full_attention`).
- The last token's hidden state goes through `readout.safetensors` (1024 to 255, no bias). Softmax over the first n options, temperature 1.0. Nothing is generated.

## Measured, 15 Nov 2026 (13 questions)

| Run | Worst difference to the authors' probabilities | Questions with another top option |
|---|---|---|
| two-way cache entries, 32-bit weights | 0.0030 | 0 |
| two-way cache entries, published 16-bit weights | 0.0390 | 0 |
| one-way mask (control) | 0.5407 | 2 |

Published 16-bit weights on MLX: 0.03 to 0.15 s a question, peak MLX memory 1855 MB.

## Pass line of the Swift engine (written before its first run)

On the same token ids as `reference.json`: the same top option on every question, and every probability within 0.05 of the authors' (the 16-bit Python run's worst is 0.039).

## The Swift engine against the pass line (15 Nov 2026, build main192)

`GlideEngine.swift` and `GlideLayout.swift` as the app compiles them, with the published 16-bit weights, outside the app (a small runner linked against the packages of build main192; the runner is the lead's, not in this repository).

| Measure | Result | Pass line |
|---|---|---|
| Questions | 13 | 13 |
| Token ids equal to the authors' | 13 of 13 | all |
| Another top option than the authors' | 0 | 0 |
| Worst difference of a probability | 0.0146 | under 0.05 |
| Seconds a question after the first | 0.03 to 0.25 (median 0.05) | none set |
| The first question (the model warms up) | 2.06 s | none set |
| Load | 1.17 s | none set |
| Peak resident memory of the runner | 1667 MB | none set |

## Licence

Fastino Community License (`LICENSE` of the model repository). Evaluation, testing, benchmarking and research are not Production (clause 6b). Commercial use must show "Built with Fastino GLiDE" (clause 4). Production commercial use by an entity over 10,000,000 US dollars of annual revenue needs a written agreement (clause 2). The model is listed in the app only on the owner's word.

## Regenerate

    <venv>/bin/python -I checks/corpora/glide/run-authors-front-end.py <model folder> checks/corpora/label-reader/cases.json checks/corpora/glide/reference.json
    <venv>/bin/python -I checks/corpora/glide/run-mlx-two-way.py <model folder> checks/corpora/glide/reference.json <out.json> [--float32] [--causal]

The venv needs torch 2.14.0, transformers with `qwen3_5`, mlx-lm, safetensors. The model folder holds the repository's files at the pinned revision.
