# BUILD-MODE — whoosh
| Row | Value |
|---|---|
| Build | build-started: 2026-11-07T14:52Z (the owner's direction pick with the go "Plan and build, review later") · seed: none (first build; PRD v0.2 signed 2026-11-07, journeys signed, data map confirmed, layout three-pane picked, look Native Glass picked) |
| Archetype | 4 (native Mac + iPhone, Rust core; the push relay is a hosted half, archetype 1, planned in its own slice) |
| Path | ENGINE |
| Authority | ENGINE: first `app-architect` dispatch (architect-p1, this session, 2026-11-07); the owner's go for the unattended build, verbatim: "Plan and build, review later (Recommended)" and "autonomous mode on" |
| Lanes + models | lanes: architect=1 · contracts=2 · migrations=1 · backend=24 · executor=37 · tests=10 · release=1 · reviewer=7 · assessor=20 · taste=1 · tester=1 · director=0 (dropped: the owner picked the direction from ten boards; a director round opens only on a whole-app verdict) · review wave: RUN from wave 1 (owner 2026-11-07 18:08) |
|  | models: contracts=opus or sonnet · migrations=sonnet · backend=sonnet or opus (sync-engine, search, agents = opus) · executor=sonnet or opus (owner rule 2026-10-11: build and test helpers on sonnet or opus, never fable), opus for the design foundation (designtokens) and the first unit of each owner-judged surface (macshell, sidebar, inboxlist, reader) · tests=sonnet (seams=opus) · release=sonnet · reviewer=sonnet · assessor=sonnet · director=opus · architect=opus · taste=opus · tester=sonnet · investigations=haiku |
| Units + artifacts | units: 82 · artifacts: 78 (plan-format Step 4b, taken before the Path row; the tree is pre-split by the root commit: feature per folder) |
| Bar | bar: 9/10 ("award winning", owner 2026-11-07) |
| Disclosure given | This is a multi-hour build. SPAN: a native Mac + iPhone client with a Rust core; expect days, not hours (the owner's two unattended hours cover the root commit, the stand-in core and the first design build of the Mac window). CREW: 78 lanes over 8 slices; the review wave is 27 seats per round; expect several rounds. CADENCE: a running Mac window from slice 1's design build, re-shown at every wave boundary; a status in chat at every wave boundary; a silent lane gets a progress check at 5 minutes. YOUR PART: interrupts welcome for scope changes or a wrong direction; otherwise it runs unattended. (Given in chat 2026-11-07 16:12.) |
| Preview owner | Cue: the rig copy launched by scripts/launch-one.sh; the owner sees the installed design build in /Applications after install.sh |
| Verified by | verified: journeys=INDEPENDENT (journey-verifier) · captures=SELF (the lane's shot verb) · deploy=OWNER (the installed copy opened by the owner) · gate=INDEPENDENT (scripts/round.sh exit codes) |
| Repo | git init'd at ~/work/smart-email/whoosh, first commit by Cue (this card, the docs, the scripts, the markers) ✓ |
| Test engine | test-engine: native driven checks over the control socket (docs/CONTROL-SOCKET.md) · access: none (no model-driven tester yet; journeys are Python drivers) · screens-to-model: twin-only (the fixture core; real records named at the first test on the owner's accounts) |
| Contract deltas since PRD sign-off | listed, each signed by the owner's ask or disclosed: (1) four capabilities at the direction pick (owner's words, PRD "## Changes asked at the direction pick"); (2) Access-permissions line (Accessibility, camera; Cue, disclosed); (3) test path Tests/ at the root (Cue, disclosed); (4) five data-map rows for the plan (Cue, disclosed); (5) taste defaults: relay never holds passwords, Apple crash reports, Polar licensing (Cue, disclosed); (6) open and click tracking OFF in v1 until the owner sets the stance (Cue, disclosed) |

## Fates (declared-vs-fired, reconciled at every wave boundary)

| Lane class | Declared | Fired | Fate | Evidence / user's quoted drop |
|---|---|---|---|---|
| architect | 1 | 1 | fired | docs/plan.json (architect-p1, 2026-11-07) |
| contracts (root) | 1 | 0 | wave 1 | docs/lanes/contracts.brief.md |
| executor (surfaces) | 37 | 0 | held until each surface is locked-on-build | docs/DESIGN.md state |
| backend (core crates) | 24 | 0 | wave 2+ after the root | docs/lane-dag.json |
| tests | 10 | 0 | wave 2+ | docs/TESTING.md |
| reviewer / assessor / taste / tester | 7 / 20 / 1 / 1 | 0 | RUN from the first wave (owner 2026-11-07 18:08: "Run it from the first wave"); the round opens at the close of wave 1 of mac-main | owner card answer |
| release | 1 | 0 | at the first install | scripts/install.sh |
