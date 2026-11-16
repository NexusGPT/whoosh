# Footprint: memory, CPU and launch (owner 2026-11-08 14:46)

Owner: "beware of app speed and heavyness on ram and CPU". Owner of this file: the perffootprint lane. Gate: `perf/footprint.py`. Budgets live in docs/CONSTANTS.md and are read from it (RSS_IDLE_MAX_MB, RSS_MODEL_MAX_MB, CPU_IDLE_MAX_PCT, EMBED_IDLE_UNLOAD_S, LAUNCH_WARM_MS, PERF_LOAD_MAX); nothing here sets a number.

## The gate

```
python3 perf/footprint.py <Whoosh.app> [--weights DIR] [--measure footprint|rss]
python3 perf/footprint.py <Whoosh.app> --unload-s 20          # test-only shorter unload wait (named in the output)
python3 perf/footprint.py <Whoosh.app> --mutant launch|keep   # must go RED
python3 perf/footprint.py --fake clean|launch|keep|busy       # dry run on a fake process, no app (PERF_LOAD_MAX=1000 on a loaded Mac)
python3 perf/footprint.py --selftest
```

One run launches the rig copy once through scripts/launch-one.sh on the fixture core (with `WHOOSH_EMBED_WEIGHTS` from `--weights`, else the env, else `rig/embedding/weights` found upward from the repo) and takes five readings:

| Check | When | Budget |
|---|---|---|
| resident memory at idle, no search yet | 60 s after the first frame | RSS_IDLE_MAX_MB |
| CPU, averaged | the next 30 s, still idle, no sync (the fixture core has none) | CPU_IDLE_MAX_PCT |
| resident memory, model loaded | 3 s after the first results of a driven meaning search | RSS_MODEL_MAX_MB |
| resident memory, model unloaded | EMBED_IDLE_UNLOAD_S + 5 s after the search | RSS_IDLE_MAX_MB |
| launch to first frame | `measure mac-main.launch`, one sample | LAUNCH_WARM_MS |

Rules the gate keeps: it refuses to launch above PERF_LOAD_MAX (a timing taken above it is a load figure); if the load is above it at the end, the CPU and launch rows are NOT JUDGED and the memory rows stay judged; "model loaded" means the memory after the search exceeds RSS_IDLE_MAX_MB, else rows 3 and 4 are NOT JUDGED (the model never loaded) and never a pass; a red names the budget and the measured value; the last line is `N checks, F failed, U not judged`. Every reading (one every 5 s) goes to `_captures/perf/footprint/<UTC time>-<mode>.json`, both memory figures side by side.

2026-11-08 (lead's ruling): the RSS_* budgets are judged on the PHYSICAL FOOTPRINT (Activity Monitor's Memory column), the gate's default; the ledger names stay RSS_*. Why: on a loaded Mac the OS squeezes an idle process out of RAM and its resident size falls while its footprint stays; in the dry run a fake process holding 540 MB read "12 to 17 MB resident four seconds later" (first samples: 377.6 MB, then 16.8 MB with nothing freed). `--measure rss` judges ps resident size instead; every sample records both.

## Hidden rig mode (owner 2026-11-08 16:30: "on all tests, EVER, i want you to stop the preview opening EVER. NEVER do it again")

- The gate does not launch a bundle that has neither `Contents/Resources/rig-hidden.ok` (written by the build once the app honours `WHOOSH_RIG_HIDDEN`, appshell round 3) nor the `WHOOSH_RIG_HIDDEN` string in an executable: the output says "HELD, not launched". `scripts/launch-one.sh` applies the same test and exits 6; the gate sets `WHOOSH_RIG_HIDDEN=1` for it and reads exit 6 as "HELD: hidden mode not available", not as a red. There is no override.
- It passes `--rig-hidden` and `WHOOSH_RIG_HIDDEN=1` through `launch-one.sh`.
- Every reading (first frame on, every 5 s) it asks the window server (`CGWindowListCopyWindowInfo`, on-screen only) for windows of the app's pids. Any window: the pids are killed and the sixth check, "windows of the app on screen", is RED. Detector controls in `--selftest`: it sees windows for the set of all pids, none for the script itself, and a bundle with and without the switch is told apart.

## The model on demand (hook commit "perffootprint hook: embedding")

- Launch and corpus load load nothing: `EmbeddingFeature.view` no longer preloads; `MeaningSearch.warm` is a no-op (the first search embeds every message it is given; `FixtureCore.loadCorpus` still calls `warm`, harmlessly, and CoreBridge is frozen to this lane).
- The first `MeaningSearch.rank` loads the model. After the last search, `EMBED_IDLE_UNLOAD_S` seconds later (ledger 300), the weights container is dropped and MLX returns its buffers (`Memory.clearCache()`). The message vectors stay.
- Test-only environment: `WHOOSH_EMBED_IDLE_UNLOAD_S` (shorter wait), `WHOOSH_EMBED_MUTANT=launch` (loads at launch, the old behaviour) or `keep` (never unloads), the way `WHOOSH_SHOT_RENDER` is the shot verb's mutant switch.

## Findings

- **First search loads the model on the main thread (fixture core only).** `SearchMacModel.askCore` (Sources/Features/SearchMac/SearchMacModel.swift, a `@MainActor` class) calls `app.core.run(command: .search)`; `FixtureCore.run` (Sources/CoreBridge/FixtureCore.swift, `case .search`) calls `MeaningSearch.rank` synchronously, which loads the model and embeds the corpus. Before this lane the model loaded at launch, so the first search did not pay for it. The real core ranks in Rust and is unaffected. Left as is this round; the lead files the fix as a search-round item. The gate prints how long the first search took and how many socket reads timed out meanwhile.

## Measured table

NOT YET MEASURED ON THE APP. No rig copy was launched (owner 2026-11-08 14:55 and 16:30: no preview may open, ever; the gate runs only on a build with the hidden rig mode). What exists is the gate's own proof against a fake process, below; those are NOT product figures.

| Run (fake process, footprint figure, scaled waits: idle 4 s, CPU 4 s, unload 8 s) | idle | CPU | model | unloaded | launch | last line |
|---|---|---|---|---|---|---|
| clean | 51 MB ok | 0.00 % ok | 562 MB ok | 61 MB ok | 123 ms ok | 5 checks, 0 failed, 0 not judged |
| mutant launch (model at launch) | 562 MB RED RSS_IDLE_MAX_MB | ok | ok | ok | ok | 5 checks, 1 failed, 0 not judged |
| mutant keep (never unloads) | 51 MB ok | ok | 562 MB ok | 562 MB RED RSS_IDLE_MAX_MB | ok | 5 checks, 1 failed, 0 not judged |
| mutant busy (spinning thread) | 51 MB ok | 20.51 % RED CPU_IDLE_MAX_PCT | ok | ok | ok | 5 checks, 1 failed, 0 not judged |

Raw: `_captures/perf/footprint/dryrun-*.txt`, the `.json` samples beside them and `selftest.txt` (judgement flips at each budget: 11 checks, 0 failed). The app-side mutants (`--mutant launch`, `--mutant keep`) and the real table are owed to the first announced window.


## Round 2 (2026-11-09): frame time, memory, launch. Build under test: installed main40 (db9e842e, Debug), hidden rig, fixture core

Machine load was 11 to 57 for every run below (other lanes compiling and running checks), so every timing here is a load figure, not a product verdict. Raw files: `_captures/perf/footprint/r2/`.

### Frame time (row efa93fcb): per action, 100 actions, `perf/interaction.py --dump`, load 16.9

| Action | n | commit min / median / p95 / max (ms) | frame min / median / p95 / max (ms) |
|---|---|---|---|
| archive (E, then Z) | 34 | 38.8 / 42.2 / 58.1 / 105.9 | 52.8 / 61.1 / 72.8 / 131.6 |
| next (J, K) | 33 | 8.7 / 9.9 / 10.5 / 47.0 | 21.6 / 23.7 / 24.2 / 61.7 |
| open (Enter, Esc) | 33 | 17.8 / 21.3 / 24.4 / 85.2 | 26.4 / 32.3 / 40.8 / 99.1 |
| all | 100 | 8.7 / 21.6 / 49.2 / 105.9 | 21.6 / 32.9 / 69.2 / 131.6 |

Budgets FRAME_MS 16, INTERACTION_COMMIT_MS 50. Archive is the heaviest action by 4 times over next. A second run under the profiler (load 22): worst frame 233.1, worst commit 189.2.

Two facts that bound the fix, before any code:
- **The installed bundle is a Debug build** (`Contents/MacOS/Whoosh.debug.dylib`; `scripts/build-mac.sh` runs `-configuration Debug`). Debug Swift turns SwiftUI generics, keypaths and metadata lookups into runtime calls; 6.2% of main-thread time sat in swift runtime demangle / metadata / keypath frames alone (257 of 4168 ms). No Release bundle exists to compare, and none was built (disk 4 GB free). The gate's numbers are for the Debug build until a Release run says otherwise.
- **The frame measure carries a floor.** Frame is the next display tick after the commit: for `next`, commit 9.9 ms median and frame 23.7 ms median, a gap of 13.8 ms, about one refresh interval. A frame budget of 16 ms measured from the key's arrival cannot be met by an action whose commit is above about 2 ms. The ledger decides whether FRAME_MS means arrival to presented (as measured) or commit to presented.

ShotLive does not explain the gap. `WHOOSH_SHOT_RENDER` was unset; 0 ms of 4168 ms of main-thread samples had a ShotLive, CGWindowListCreateImage, ScreenCaptureKit or ImageRenderer frame, because the 100 actions call no `shot`. A visible-versus-hidden comparison was not run: no window may open (owner 2026-11-08 16:30).

Profile: `xctrace record --template 'Time Profiler' --attach <rig pid>` during the 100 actions and again for 60 archive, 60 open and 60 next actions alone (`perf/profile-actions.py`), aggregated by `perf/profile-aggregate.py`. Share = main-thread running time outside the control-socket verbs (render work after the key), per kind. Summaries: `profile-summary-*.txt`.

| # | Cost (file: function) | next | archive | open | all 100 |
|---|---|---|---|---|---|
| 1 | `Sources/Features/InboxList/ListStates.swift: ListEndWords.make`, which calls `PinnedTabs.all` twice, each time `FixtureCore.pinnedLabels` (labelsLocked, orderedTree, countsLocked), evaluated eagerly in `InboxListView.rowsList` on every list render | 17.6% (156 of 888 ms) | 7.7% (234 of 3036) | n/a | 6.5% (236 of 3658) |
| 2 | `Sources/Features/InboxList/InboxRowView.swift: body` -> `lineOne` -> `RowFacts.time` (a new DateFormatter per row per render, 3.6% in archive) and the account directory `RowAccount.directory` -> `AccountBook.accounts` -> `FixtureCore.accountsInFixtureOrder` (3.5%) | 4.3% | 8.9% (269 ms) | 1.8% | 5.6% (206 ms) |
| 3 | `Sources/Features/Reader/ReaderPane.swift: body` -> `BodyWebView.makeNSView` -> `PassThroughWebView.init` (a WKWebView built on every open), plus `ReaderThread.make` | n/a | n/a | 6.0% WebView + 3.9% ReaderThread.make (of 1922 ms) | 2.6% |

Everything else is SwiftUI and AttributeGraph work inside the Debug build: 58% of the render samples in the 100-action run (2128 of 3658 ms) have no Whoosh frame below the run loop. The three items above are the app-owned part and the only part a code change reaches.

Exact changes, per file owner (this lane edits `perf/**` only):
1. **inboxlist, `Sources/Features/InboxList/ListStates.swift`** `ListEndWords.make`: read the pinned tabs once, `let tabs = PinnedTabs.all(m)`, and use `tabs` in both the current-tab lookup and the next-tab lookup (today the function calls `PinnedTabs.all(m)` twice per render). Second change in `Sources/Features/InboxList/InboxListView.swift` line 79: pass the end line its inputs (`ListEndLine(model: model, shown: shown.count)`) and build the words inside `ListEndLine.body`, so a list render that changes neither the tab nor the shown count does not rebuild the words.
2. **inboxlist, `Sources/Features/InboxList/RowFacts.swift`** `RowFacts.time`: replace the per-call `DateFormatter()` with three static formatters (`HH:mm`, `EEE`, `d MMM`, locale `en_GB`), setting `calendar` per call only when it differs from `.current`. Same file family: `Sources/Features/Reader/ReaderPane.swift` line 28 calls `RowAccount.of(thread.accountId, in: model)`, which rebuilds the whole directory for one account; pass a directory read once for the render (the list already does).
3. **CoreBridge owner (appshell or contracts, frozen path), `Sources/CoreBridge/FixtureCore.swift`** `accountsInFixtureOrder` (line 937): the unread correction runs `messages.filter { $0.account == a.id && $0.unread && gone($0.id) }.count` once per account, so each call costs accounts times messages; count the gone-unread messages per account in one pass over `messages` first. `labelsLocked` / `pinnedLabels`: cache the ordered tree until a label write bumps it. Both are fixture-core costs; the real core pays an FFI decode instead (`FfiConverterSequenceTypeMailLabel.read`, 3.6% of next).
4. **messagebody, reader**: keep one `WKWebView` and reload its content on open instead of `BodyWebView.makeNSView` building a new `PassThroughWebView` per open.

None of the four is proven to bring the frame under 16 ms; together they target the app-owned share named above (about 12% to 22% of render time per kind: next 22%, archive 17%, open 12%). The Debug-build and frame-floor facts decide the rest.

### Memory (row 57586479)

`fixture load full-history-30` is NOT served by main40: the verb list in the server's own reply has no `load`, and delta 13 (main 3efc5234) is not an ancestor of main40 (`_captures/perf/footprint/r2/full-history-30-not-served-by-main40.txt`). The 30-account figure needs a bundle built from 3efc5234 or later; none was available and none was built. Measured instead, main40 on the standard fixture (16 messages, 30 accounts' rows), `perf/footprint.py --allow-load`, physical footprint, load 29 at the start (the timing rows are therefore NOT JUDGED; memory is not a timing):

| State | Footprint | Budget | Verdict |
|---|---|---|---|
| idle, 60 s after the first frame, no search | 71 MB | RSS_IDLE_MAX_MB 300 | ok |
| model loaded, 3 s after a driven meaning search (first search returned in 2.4 s) | 944 MB | RSS_MODEL_MAX_MB 2200 | ok |
| model unloaded, EMBED_IDLE_UNLOAD_S + 5 s after the search | 182 MB | RSS_IDLE_MAX_MB 300 | ok |
| windows of the app on screen | 0 | 0 | ok |

The unloaded figure is 111 MB above idle: the message vectors and the allocator keep that much. The fixture core holds its mail in memory, so these figures are an upper bound for a real core. `perf/footprint.py` gained `--fixture-load NAME` (memory before and after the load, then the same protocol) and `--allow-load` (runs above PERF_LOAD_MAX, memory rows judged, CPU and launch NOT JUDGED); the large-fixture run is owed to a bundle that serves it. Search over 300,000 messages embeds every row on the first search (`MeaningSearch.rank` is given all rows), which the 240 s search wait would not cover; that is the next thing to look at once the fixture serves.

Gate fixes found while running: results arrive on Enter, not per keystroke, so the driven search presses Enter after typing; the window check counts only windows with alpha above 0.01 (the hidden rig's window is listed by the window server at alpha 0 and must not read as visible); runs go through `scripts/queue.sh` because two rig launches at once kill each other's instance (first attempt lost to another lane's launch: "a lock with no holder pid cleared", rig=2).

### Launch (row: launch measures)

`perf/launch.py` `sample()` returned None because it launched without the hidden flags and `launch-one.sh` answered exit 6. It now sets `WHOOSH_RIG_HIDDEN=1` and `LAUNCH_ENV`, passes `--rig-hidden`, holds when the bundle has no `rig-hidden.ok`, and prints the reason when a sample gives no number. First frame is sampled with `measure mac-main.first-frame`, which the app does not serve yet: NOT JUDGED, never a pass.

main40, hidden, `--samples 4`, load 11.7 at the start (PERF_LOAD_MAX 12, run with the override 100 because the load rose past 12 while it ran):

| Measure | Value | Budget | Verdict |
|---|---|---|---|
| warm launch to inbox, median of 3 | 1944.8 ms | LAUNCH_WARM_MS 400 | OVER |
| cold launch (first sample) | 1965.4 ms | LAUNCH_COLD_MS 1500 | OVER |
| first frame | not served | FIRST_FRAME_MS 100 | NOT JUDGED |

Last line: `3 checks, 2 failed, 1 not judged`. A hand run at 08:22 read 1872.98 ms, in line. The "cold" sample is only the first of this script's launches, not a purged-cache launch.

For appshell, the two lines the first-frame measure needs (not applied; those files are appshell's):
- `Sources/App/AppModel.swift`, after `public var launchMs: Double?` (line 31): `public var firstFrameMs: Double?`
- `Sources/App/RootView.swift`, in `.onAppear` after the `launchMs` line (line 20): `DispatchQueue.main.async { if model.firstFrameMs == nil { model.firstFrameMs = LaunchClock.msSinceProcessStart() } }` (the run-loop turn after the first commit, the closest cheap point to the first presented frame; the hidden window is alpha 0, so a display link cannot tell)
- `Sources/ControlSocket/WindowVerbs.swift`, after the `mac-main.launch` line (line 11): `if ax == "mac-main.first-frame", let ms = m.firstFrameMs { return ["ms": ms] }`

## Round 3 (2026-11-09): memory baseline with `fixture load full-history-30`, installed main41 (e9365eb8, DEBUG build), hidden rig, fixture core

Every line below is a Debug build (`Whoosh.debug.dylib`), physical footprint, load 23 to 26 at the start (timing rows NOT JUDGED; memory is not a timing). The Release runs come with main42. The fixture core keeps all mail in memory, so each figure is an upper bound for a real core. Raw: `_captures/perf/footprint/r3/`.

| State (Debug, main41) | Footprint | Budget | Verdict |
|---|---|---|---|
| idle, standard fixture, 60 s after the first frame, no search | 70 MB | RSS_IDLE_MAX_MB 300 | ok |
| idle, `fixture load full-history-30` loaded (15 s to load), no model, 60 s later | 721 MB | RSS_IDLE_MAX_MB 300 | OVER (the 30 accounts x 10,000 messages sit in memory) |
| model loaded after a driven meaning search on the standard fixture (4.0 s) | 976 MB | RSS_MODEL_MAX_MB 2200 | ok |
| model loaded plus `full-history-30` loaded on top | 1567 MB | RSS_MODEL_MAX_MB 2200 | ok (information) |
| after the model unloaded (305 s idle), `full-history-30` still loaded, no model | 745 MB | RSS_IDLE_MAX_MB 300 | OVER, 24 MB above the 721 MB fixture-only idle |

Reading: the 300 MB idle budget is exceeded only by the fixture's own corpus (about 650 MB for 300,000 messages); the model, once unloaded, leaves 24 MB behind with the large corpus and 111 MB with the standard one (round 2). Whether RSS_IDLE_MAX_MB should be judged on the standard fixture or on this one is the ledger's call.

Why the model states are taken before the big fixture is loaded: a first meaning search with the 300,000-message corpus resident embeds every row (`MeaningSearch.rank` is given all rows); the first attempt (`--fixture-load full-history-30` alone) reached the search, climbed to about 1 GB, and the app was gone before the 240 s search wait ended, with no crash report written (cause not determined; memory pressure on a loaded Mac is one candidate). `--after-search` loads the large fixture after the search; without it the large fixture loads first. `FIXTURE_LOAD_WAIT_S` (180 s) replaces the socket's 5 s timeout for the load, which timed out at load 25.

Also this round: `perf/interaction.py` judges the longest presented-frame gap against 1.5 x FRAME_MS (24 ms; a perfect 60 Hz run gaps 16.7 ms, a dropped frame 33 ms), selftest 10 checks, 0 failed; the gap measure is NOT JUDGED until appshell serves `mac-main.interaction.gap`.
