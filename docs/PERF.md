# Perf numbers (measured; never adjusted)

Owner of this file: appshell lane. Each table row is one run of `perf/interaction.py` against one stamped build, with the machine load at that run. Budgets are read from docs/CONSTANTS.md (FRAME_MS 16, INTERACTION_COMMIT_MS 50); a run above PERF_LOAD_MAX (12) is a load figure, not a product figure, and is labelled so.

## Interaction: 100 actions (34 archive, 33 next, 33 open), `measure mac-main.interaction.frame|commit`

| Field | Value |
|---|---|
| Build stamp | label=as2 commit=9d1f9a0efe2285f34603b294fdd573a130694c87 built=2026-11-08T11:00:29Z (Debug, fixture core; dirty=1 was the regenerated core/Cargo.lock, reverted) |
| Date | 2026-11-08 |
| Machine load (1 min) | 24.2 at the gate run; 30.5 after the distribution run (limit PERF_LOAD_MAX 12). The gate refuses at this load; both runs here used the gate's documented override `PERF_LOAD_MAX=100` for the gate and the dump script for the distribution. Other lanes were building (xcodebuild, cargo) the whole time. |
| Gate last line | `2 checks, 2 failed, 0 not judged` (exit 1) |
| Gate rows | frame: worst 173.6 (budget < 16) OVER; commit: worst 163.8 (budget < 50) OVER |

| Measure | min | median | p95 | max | budget | verdict |
|---|---|---|---|---|---|---|
| frame ms (arrival to next presented frame) | 29.0 | 76.1 | 152.8 | 245.8 | < 16 | OVER (loaded machine; not a product verdict) |
| commit ms (arrival to idle point after Core Animation's commit) | 16.5 | 58.3 | 142.9 | 236.7 | < 50 | OVER (loaded machine; not a product verdict) |

Raw output: `_captures/appshell/r2/perf-interaction-loaded.txt` and `_captures/appshell/r2/dist-loaded.txt`. A run at load <= 12 is still owed; the numbers above must not be read as the product's.

## What the measures are

- Arrival: the instant the control socket read the request line of the last `key` or `click` (so the wait for the main thread is counted, as for a real key).
- Commit: the first main run-loop idle point after Core Animation's own commit observer. Every pending SwiftUI update has run by then.
- Frame: the presentation time (`targetTimestamp`) of the first display-link tick of the main window after that commit. It carries up to one refresh interval of vsync alignment by construction, so its floor is above its commit.
- Mechanism proof: `_captures/appshell/r2/check-slowed.txt` (a 30 ms sleep on every second event moves both measures by 28.5 and 31.5 ms), `check-baseline.txt` (no sleep: the even/odd gap is -0.1 and 0.2 ms), `check-mutant-constant.txt` (a server answering a constant 1 ms: RED, 3 failures).

## FrameProbe on-demand (appshell round 2, item 2)

Change: Sources/App/FrameProbe.swift mounts its GeometryReaders only after a socket frame request and unmounts them 3 s after the last one (before: one GeometryReader and a global-frame onChange behind every `.whooshAx` view, re-measured on every focus move).

| Run | Build stamp | Machine load at the gate | frame min / median / p95 / max | commit min / median / p95 / max | gate last line |
|---|---|---|---|---|---|
| before | as2 9d1f9a0 | 24 to 30 | 29.0 / 76.1 / 152.8 / 245.8 | 16.5 / 58.3 / 142.9 / 236.7 | 2 checks, 2 failed, 0 not judged |
| after | as2 6d45cbf | 55 to 70 | 27.2 / 75.7 / 148.2 / 166.5 | 15.5 / 58.1 / 133.1 / 156.0 | 2 checks, 2 failed, 0 not judged |
| low load (owed row) | main39 f1346781, the installed bytes, hidden rig | 4.38 (under PERF_LOAD_MAX 12, no helper build running) | worst 96.8 (the gate's own line; the distribution run was not taken) | worst 80.7 | 2 checks, 2 failed, 0 not judged (2026-11-09 07:30, scratchpad perf-main39-installed.log) |
| after (main105, perf-archive rounds 3 to 6 inside) | main105 e46cf3c7, hidden rig, fixture core | 10.10 (under PERF_LOAD_MAX 12) | 9.2 / 24.2 / 40.1 / 116.0 | 4.5 / 17.3 / 33.3 / 108.1; longest frame gap worst 52.2 (archive median 26.1, open 21.4, next 10.3) | 3 checks, 3 failed, 0 not judged (the lead's run, _captures/lead/main105/perf/interaction.log lines 6-127) |

The first two runs used `PERF_LOAD_MAX=100` (the gate refuses above 12); the low-load row ran under the limit with no override, so its OVER is a product figure (an upper bound: the hidden rig draws the picture on the main thread). The red belongs to the perf lane (perffootprint) with the shell and the list. The two rows were taken at different loads, so the table shows no measurable change and proves no win either way; a paired run at load 12 or below, before and after, is still owed (raw: perf-interaction-loaded-after.txt, dist-loaded-after.txt in _captures/appshell/r2/).

Third run, final build (as2 c27c764, FrameProbe constants read from the ledger), load 43 to 78 (other lanes building), `PERF_LOAD_MAX=100`: frame min 27.9 / median 68.7 / p95 142.5 / max 159.4; commit min 15.0 / median 49.1 / p95 128.4 / max 148.1; gate last line `2 checks, 2 failed, 0 not judged` (the gate's own worst-case rows were 324.8 and 316.1: a load spike in that run). Raw: perf-interaction-final.txt, dist-final.txt. Medians of the three runs sit within 8 ms of each other across loads from 24 to 78, so the machine load, not the probe, dominates these numbers. The on-demand probe works: `measure mac-main.accounts.all` returned a frame in 0.16 s on the first read (readers mounted by the request) and 0.00 s on the second.

## On-device decision models on the fixture's 16 messages (test copy main192, 15 Nov 2026)

One run of `checks/laya-real-check.py` per line of the model list, with the model's real weights served on 127.0.0.1, the hidden test copy, the description "Asking a question from me" typed on Needs Reply, then Re-run. The pass line is `docs/lanes/laya-real-weights.passline.md`: four messages must land in Needs Reply (msg-03, msg-05, msg-07, msg-12), seven must not. A mail is filed when the model's sureness reaches MODEL_TRUST_LABEL (0.8). Machine load 25 to 97 during the runs (other lanes building): the seconds are upper bounds. Logs: `_captures/lead/main192/models/`.

| Model | Download | Must-land filed | Must-not filed | Slowest step, per message | Peak footprint | Footprint growth | Peak resident |
|---|---|---|---|---|---|---|---|
| Laya | 843 MB | 0 of 4 | 0 of 7 | 115 ms | 1578 MB | 1515 MB | 1137 MB |
| KnowLine 0.8B, run 1 | 1729 MB | 1 of 4 (msg-07) | 0 of 7 | 938 ms | 2767 MB | 2680 MB | 1585 MB |
| KnowLine 0.8B, run 2 | 1729 MB | 1 of 4 (msg-07) | 0 of 7 | 355 ms | 2683 MB | 2592 MB | 1335 MB |
| GLiDE 0.8B, run 1 | 1525 MB | 2 of 4 (msg-03, msg-07) | 0 of 7 | 283 ms | 2228 MB | 2135 MB | 1895 MB |
| GLiDE 0.8B, run 2 | 1525 MB | 2 of 4 (msg-03, msg-07) | 0 of 7 | 215 ms | 2391 MB | 2291 MB | 1921 MB |

Against the ledger: every run is under LAYA_SECONDS_PER_MESSAGE_MAX (1.0 s). The two 0.8B models are over RSS_MODEL_MAX_MB (2200) and every model is over LAYA_GROWTH_MAX_MB (1400); neither number was changed. No model files all four must-land messages at the default trust. GLiDE also filed msg-04 and msg-11 under Updates (another label's question, not Needs Reply).

GLiDE's engine outside the app, on the authors' 13 reference questions: `checks/corpora/glide/README.md` (same top option on 13 of 13, worst difference 0.0146, 0.03 to 0.25 s a question after the first, peak footprint about 2.0 GB whatever MLX's buffer cache limit: 0, 64, 256 MB and the default gave 2016, 2008, 2009 and 1993 MB).
