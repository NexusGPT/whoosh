# Testing: Whoosh (P1 draft)

The ten test types of REF/native-desktop/test-plan.md, filled with the planned paths of the repo at ~/work/smart-email/whoosh (proposed; created by the root commit). Written by architect-p1, 2026-11-07. The root commit copies this file into the repo's docs/ and turns the manifest section below into `docs/feature-manifest.md`, the round's INPUT.

Rules that bind every row:
- A check is green only after it has been seen red on its mutant.
- The seat that wrote the code never certifies its own check.
- Every check prints `N checks, F failed, U not judged` and returns its verdict as the exit code.
- An owner complaint becomes a regression file before the fix.
- A check that could not run is not green.
- No real credential in a rig: lanes drive fixtures (a local IMAP, SMTP and JMAP test server seeded from `fixtures/`). The owner's real accounts are touched only on the INSTALLED copy, driven by Cue.

## The ten types

| Type | File (in the repo) | Owner seat | Runner + exit contract | MADE when | KEPT how | RETIRED how |
|---|---|---|---|---|---|---|
| unit (Swift) | `Tests/Unit/<Feature>Tests.swift`, one per folder under `Sources/Features/` (owned by the tests-jN lane of the slice that first delivers the feature) | app-tests | `xcodebuild test -scheme WhooshUI` (Mac and the iPhone simulator); exit ≠ 0 on any failure | day 0, one file per feature folder | runs on every build; red blocks install | a marker line with the reason; the file stays |
| unit (Rust) | `core/crates/<crate>/tests/*.rs`, drafted by the crate's lane | app-tests certifies (reads, runs cargo-mutants, records the verdict); the crate lane drafts | `cargo test --workspace` in `core/`; exit ≠ 0 on any failure | with the crate's first commit | runs on every build; red blocks install | marker line |
| integration | `Tests/Integration/<Seam>Tests.swift` over the UniFFI seam: InboxStream, ActionsSeam, OutboxSeam, SyncSeam (slice 1); SearchSeam (2); AgentSeam (3); AIJobsSeam (4); DeviceSync (7) | app-tests | same as unit (Swift) | when the capability-contract row it covers lands | same | same |
| functional | `checks/<feature>-check.py`, driving the RUNNING rig copy through `tools/whoosh-ctl.py` | app-tests certifies; a producing lane may draft | prints the summary line; exit 1 when F > 0 or U > 0 | per feature, before its lane closes | listed in the manifest; the round refuses a missing cell | marker |
| contract | `Tests/Contract/<verb>Tests.swift` (the socket verbs of docs/CONTROL-SOCKET.md) and `Tests/Contract/Relay<Route>Tests.swift` (the relay API, once its rows exist, gap G10); `checks/contract-types-check.py` | app-contracts | same as unit (Swift); the check prints the summary line | when docs/CAPABILITY-CONTRACT.md freezes (root commit) | same | same |
| end-to-end | `journeys/J1.py` … `journeys/J7.py`, one per journey of docs/JOURNEYS.md, run by the root-owned `journeys/run.py` (globs `journeys/J*.py`), every theme | app-tests writes; journey-verifier certifies | writes `docs/COVERAGE.md`; exit 1 on red OR undriven | at scaffold, all seven, seen red before any slice lands | `could-not-drive` is amber for one round, then red | marker |
| acceptance | `acceptance/<surface>.md` (the owner's words and the observable outcome) + the driven check it names | Cue + the owner | the driven check's exit | at each surface's design pick | re-run before every install | marker |
| regression | `checks/regression/<complaint-id>-check.py` | app-tests (regression lane) | exit 1 | AT THE COMPLAINT, before the fix | never removed while the feature exists | marker naming the complaint |
| performance | `perf/launch.py`, `perf/interaction.py`, `perf/memory-mac.py` (slice 1); `perf/search.py` (2); `perf/thirty-one.py` (5); `perf/notch-arrival.py` (6); `perf/push-arrival.py`, `perf/memory-iphone.py` (7); budgets read from docs/CONSTANTS.md through the root-owned `perf/lib/` | app-tests | exit 1 over budget; prints the build stamp and the machine load; above PERF_LOAD_MAX the cell is `not judged` | at the PRD budget row (slice 1 for the first three) | red blocks install | marker |
| smoke | `checks/smoke-check.py` (launch, first frame, one archive, quit) | app-release | exit 1 | day 0 | first cell of every round; runs on the INSTALLED bytes under the test identity after every install; Cue then opens the real installed copy, drives it with computer use and reads the screenshots | never |
| mutation | a `MUTANTS` table inside every check above; `cargo mutants` for the Rust crates | the check's author | the check fails itself when a mutant does not go red | per check, at authoring | `checks/mutant-coverage-check.py` (root-owned) refuses a check with no table | with its check |

The iPhone target runs unit, integration, end-to-end and performance on the simulator and on one real iPhone 17 Pro. The Mac target runs all ten. Cross-surface seams have their own files under `checks/seams/` (owner: the seams lane), written from the fixture and this contract, never from a built file.

## Feature manifest, slice 1 (the first rows of docs/feature-manifest.md)

Slice 1 = J1 on the Mac (plan-p1.json `slice-j1-triage`). Every row is a cell `scripts/round.sh` runs; status at scaffold is red for every row by construction. Surface lanes are held until their surface is locked-on-build (plan gap G1).

| cell | type | file | runner | owner seat | covers (todo · journey steps) | surface |
|---|---|---|---|---|---|---|
| smoke | smoke | checks/smoke-check.py | `python3 checks/smoke-check.py --installed` | app-release | fn-release-smoke | mac-main |
| core-unit | unit (Rust) | core/crates/{contracts,store,secrets,parse,sync-imap,sync-engine,actions,smart-inbox,outbox,send}/tests | `cargo test --workspace` | app-tests | fn-contract-types, fn-migration-v1, fn-store-writer, fn-secrets, fn-parse, fn-sync-imap, fn-sync-engine, fn-actions, fn-smart-inbox-rules, fn-outbox-undo-send, fn-send-smtp | none |
| core-mutants | mutation | the same crates | `cargo mutants --workspace` (missed mutants = F) | app-tests | the same todos | none |
| swift-unit | unit (Swift) | Tests/Unit/{Spine,Components,Shortcuts,Sidebar,InboxList,Reader,QuickReply,PaletteMac,Trainer,MacShell,WelcomeMac}Tests.swift | `xcodebuild test -scheme WhooshUI -destination 'platform=macOS'` | app-tests | fn-spine, ix-components-pickers, fn-shortcuts-engine, ix-sidebar, ix-inboxlist-triage, ix-reader-thread, ix-quickreply, ix-palettemac, ix-trainer, ix-macshell, ux-welcome-imap | mac-main, mac-palette, mac-trainer, mac-welcome-accounts |
| integration | integration | Tests/Integration/{InboxStream,ActionsSeam,OutboxSeam,SyncSeam}Tests.swift | same | app-tests | C-J1.1, C-J1.2-C-J1.8 | mac-main |
| contract | contract | Tests/Contract/{Type,Key,Click,Rows,Uistate,Winframes,Measure,Shot,Fixture,Clock,Net}Tests.swift; checks/contract-types-check.py | `xcodebuild test -only-testing:WhooshTests/Contract`; `python3 checks/contract-types-check.py` | app-contracts | fn-contracttests-s1, fn-contract-types | none |
| spine | functional | checks/spine-check.py | `python3 checks/spine-check.py` | app-tests | fn-spine | none |
| tokens | functional | checks/token-check.py | `python3 checks/token-check.py` | app-tests | vis-designtokens, vis-* | all slice-1 surfaces |
| licence | functional | checks/licence-check.py | `python3 checks/licence-check.py` (0 strings over 40 characters shared with the AGPL boilerplate prompt files; 0 GPL/AGPL dependencies in Cargo.lock and Package.resolved) | app-tests | gap G5, interpretation "open source MIT" | none |
| shortcuts | functional | checks/shortcuts-check.py | `python3 checks/shortcuts-check.py` | app-tests | fn-shortcuts-engine · J1.2-J1.5, J1.8-J1.10 | mac-main |
| macshell | functional | checks/macshell-check.py | `python3 checks/macshell-check.py` | app-tests | ix-macshell · J1.1, J4.8 link | mac-main |
| sidebar | functional | checks/sidebar-check.py | `python3 checks/sidebar-check.py` | app-tests | ix-sidebar · J1.1, J1.5 | mac-main |
| inboxlist | functional | checks/inboxlist-check.py | `python3 checks/inboxlist-check.py` | app-tests | ix-inboxlist-triage, ux-inboxlist-states · J1.1-J1.5, J1.8, J1.12 | mac-main |
| messagebody | functional | checks/messagebody-check.py | `python3 checks/messagebody-check.py` | app-tests | fn-messagebody · J1.6 | mac-main |
| reader | functional | checks/reader-check.py | `python3 checks/reader-check.py` | app-tests | ix-reader-thread · J1.6 | mac-main |
| quickreply | functional | checks/quickreply-check.py | `python3 checks/quickreply-check.py` | app-tests | ix-quickreply · J1.7 | mac-main |
| palettemac | functional | checks/palettemac-check.py | `python3 checks/palettemac-check.py` | app-tests | ix-palettemac · J1.9 | mac-palette |
| trainer | functional | checks/trainer-check.py | `python3 checks/trainer-check.py` | app-tests | ix-trainer · J1.10, J1.11 | mac-trainer |
| welcomemac-imap | functional | checks/welcomemac-check.py (IMAP routes only in slice 1) | `python3 checks/welcomemac-check.py --routes imap` | app-tests | ux-welcome-imap · J4.1, J4.2, J4.4, J4.6 | mac-welcome-accounts |
| seams-s1 | functional | checks/seams/{counts,account-colour,error-vocabulary,undo,component-grammar}.py | `python3 checks/seams/run.py --slice 1` | app-tests (seams lane) | con-seams-s1, vis-components | mac-main, mac-palette |
| design-build | end-to-end | journeys/J1.py --fixture-core | `python3 journeys/run.py J1 --fixture-core --themes light,dark` | app-tests writes; journey-verifier certifies | fn-fixture-core, vis-design-build-macmain · J1.1-J1.12 on the fixture (first visible milestone, before real sync) | mac-main |
| splittabs | functional | checks/splittabs-check.py | `python3 checks/splittabs-check.py` | app-tests | fn-split-tabs, ix-split-tabs-edit, ix-settings-splittabs · J1.13 | mac-main, mac-settings-general |
| accountmark | functional | checks/accountmark-check.py | `python3 checks/accountmark-check.py` | app-tests | ix-account-mark, ix-accounts-section, ix-account-icon-picker · J1.1, J4.12 | mac-main, mac-settings-accounts |
| J1 | end-to-end | journeys/J1.py | `python3 journeys/run.py J1 --themes light,dark` | app-tests writes; journey-verifier certifies | fn-tests-j1 · J1.1-J1.13 (J1.13 added at the direction pick) and J4.12 driven inside J1's setup | mac-main, mac-palette, mac-trainer |
| perf-launch | performance | perf/launch.py | `python3 perf/launch.py` | app-tests | fn-perf-mac · LAUNCH_WARM_MS, LAUNCH_COLD_MS, FIRST_FRAME_MS | mac-main |
| perf-interaction | performance | perf/interaction.py | `python3 perf/interaction.py` | app-tests | fn-perf-mac · FRAME_MS, COMMIT_MS | mac-main |
| perf-memory | performance | perf/memory-mac.py | `python3 perf/memory-mac.py --accounts 30` | app-tests | fn-perf-mac · MEM_MAC_MB | mac-main |
| acceptance-mac-main | acceptance | acceptance/mac-main.md + journeys/J1.py | the J1 cell's exit | Cue + the owner | the owner's words for mac-main at its style pick | mac-main |

Slice-1 demo (the rank-1 slice must be drivable end to end): after the J1 cell is green on the rig, `scripts/install.sh` installs the build, smoke runs on the installed bytes, then Cue drives J1 on the owner's real accounts on the installed copy with computer use and reads its own screenshots. Never a typed URL, never the owner pressing keys.

<!-- RESULT: done, ten types filled with planned repo paths; 28 slice-1 manifest cells; decision-model check (checks/decision-check.py, J5.8) joins slice 3. -->
