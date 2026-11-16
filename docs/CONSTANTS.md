# Constants ledger — Whoosh

Every number the build depends on, with what it depends on. A number in two files is a defect: scripts, perf checks and the pipeline read this file. Source: docs/CAPABILITY-CONTRACT.md (the P1 architect, 2026-11-07); owner confirms the rows marked so.

## The constants

| Constant | Value | Depends on |
|---|---|---|
| LAUNCH_WARM_MS | 400 | PRD Perf-budget (docs/PRD.md:38) |
| LAUNCH_COLD_MS | 1500 | same |
| FIRST_FRAME_MS | 100 | same |
| LAUNCH_SAME_COPY_MS | 400 | PRD Perf-budget "launch to inbox <400ms warm" (docs/PRD.md:38): the warm launch is the SECOND launch of one installed copy (perf/launch.py `warm, same copy`, scripts/launch-one.sh LAUNCH_RUNS=2); the first launch of a new copy pays the OS's first-run scan of the file and is judged against LAUNCH_COLD_MS. Row added by launch-r3 on the lead's ask (2026-11-10); the value is the PRD's, not a new number |
| FRAME_MS | 16 | same; 2026-11-09 (lead): the longest gap between two presented frames during an interaction (the dropped-frame measure, `measure mac-main.interaction.gap`) is judged against 1.5 x FRAME_MS (a perfect 60 Hz run has a 16.7 ms gap); it is not the time from arrival to the first frame |
| COMMIT_MS / INTERACTION_COMMIT_MS | 50 | same; 2026-11-09 (lead): the commit (`mac-main.interaction.commit`) is under it; the time from the input's arrival to the first presented frame (`.frame`) is under it plus one refresh (FRAME_MS); owner 2026-11-10 08:0x: "The objective is <50ms for any operation": this one number binds EVERY operation the owner can make (open, move, archive, label, search typing, reply open, send tap), on the real core as on the fixture (checks/realcore-perf.py judges the real core against it); a slower operation is a defect, never a budget change |
| SEARCH_FIRST_SCREEN_MS | 50 on 1M messages | the owner, 2026-11-10: "The objective is <50ms for any operation" (the row OPERATION_MS); it replaces the PRD's 100 (docs/PRD.md:39, docs/CAPABILITY-CONTRACT.md:112, which still say 100 until their owners change them); the screen must stay UNDER it (perf/search.py fails at >=); judged by journeys/J2.py `first-screen-ms`, checks/searchphone-check.py `first-screen-ms`, perf/thirty-one.py and, on 1M messages, perf/search.py; no measured run on 1,000,000 messages exists yet; changed by gap-ledger-2 |
| MEM_MAC_MB / MEM_IPHONE_MB | 600 with 30 accounts / 250 | same |
| UNDO_SEND_DEFAULT_S · MIN · MAX | 10 · 5 · 30 | JOURNEYS answer 21 (docs/JOURNEYS.md:147) |
| TRAINER_HINT_STOP_AFTER | 3 key uses | JOURNEYS answer 10 (docs/JOURNEYS.md:136) |
| QR_EXPIRY_S | 180 | "a few minutes" (docs/JOURNEYS.md:149); architect default, disclosed |
| SNOOZE_LATER_TODAY · TOMORROW · NEXT_WEEK | +3 h rounded · 08:00 next day · Monday 08:00 local | J1.3 words; architect default, disclosed |
| UNDO_DEPTH | 20 | architect default |
| IMAP_IDLE_REISSUE_MIN | 29 | RFC 2177 (research/mail-sync.md:19) |
| IMAP_CONN_PER_ACCOUNT | 2 | Gmail's 15-client cap per account shared with the owner's other clients (research/mail-sync.md:9) |
| IMAP_COMMAND_TIMEOUT_MS | 30000 | how long the IMAP adapter lets the server stay SILENT (no byte received) while a command waits for its answer, and the longest a connection open or an IDLE start/end may take; a long FETCH still receiving bytes is never cut; then the call ends in a retryable refusal. 30 s sits above the engine's 10 s for a provider's token and settings requests (OAUTH_HTTP_TIMEOUT_MS, AI_HTTP_TIMEOUT_MS), because a mail server may think for a while on a SELECT or SEARCH of a large folder, and below the 60 s the engine gives an AI draft (AI_DRAFT_TIMEOUT_MS); it is also rusted-mail's default (models.rs:36-38, research/dossier-rusted-mail-gaps.md row 13); number Cue (provider-limits lane, 2026-11-11) |
| GRAPH_CONCURRENT · GRAPH_REQ_PER_10MIN | 4 · 10,000 | research/mail-sync.md:12,82 |
| GMAIL_UNITS_PER_MIN | 6,000 | research/mail-sync.md:9 |
| GMAIL_WATCH_RENEW_H | 24 (must be under 7 days) | research/ios-push.md:39 |
| GRAPH_SUB_RENEW_MIN | under 10,080 (rich: under 1,440) | research/ios-push.md:40 |
| BACKFILL_CONCURRENT_MAC · IPHONE | 8 · 2 accounts | perf budget; re-measured by perf/thirty-one.py |
| BODY_CACHE_DAYS (iPhone) | 90 | inferred reading of "full history, all indexed" (plan interpretation 2) |
| RRF_K | 60 | research/local-search.md:71 |
| EMBED_DIM | 512 (Matryoshka truncation of 768), i8 | provisional until the fn-embedding rival probe |
| CHUNK_TOKENS · CHUNK_OVERLAP | 512 · 64 | EmbeddingGemma context and the Core ML 512 window (research/local-search.md:11,32) |
| INDEX_LAG_MS | 2000 | search freshness for new mail; re-measured |
| AGENT_MAX_STEPS | 10 | architect default |
| AGENT_CONTEXT_CHUNKS | 20 | plan interpretation 3 |
| AGENT_MAX_OUTPUT_TOKENS | 1500 | the ceiling the caller passes to a provider for one Ask answer with citations; the agents crate holds no number (lead, 2026-11-07, from the agents lane) |
| EMAIL_ADDRESS_MAX | 254 | longest address Welcome accepts, RFC 5321 path limit (lead, 2026-11-07, from the Welcome lane) |
| EMAIL_LOCAL_PART_MAX | 64 | longest part before the @ Welcome accepts, RFC 5321 (lead, 2026-11-07, from the Welcome lane) |
| MCP_MAX_REQUEST_BYTES | 65536 | longest JSON-RPC line the local MCP server reads before refusing; a local process could otherwise send one endless line (lead, 2026-11-08, from the mcp crate) |
| MCP_APPROVAL_TIMEOUT_S | 60 | how long an outside app's request waits for the owner's approval before it is refused (lead, 2026-11-08, from the mcp crate) |
| SHOT_LIVE_TIMEOUT_S | 5 | how long the shot verb waits for a live window image before falling back to the render path (lead, 2026-11-08, from shell round 6) |
| PERF_LOAD_MAX | 12 | the one-minute load average above which a timing is judged one way only: one runnable per core on this 12-core Mac (lead, 2026-11-08, from the J2 verifier). Since 2026-11-13 (lead's decision, journeys/lib/loadgate.py, used by every speed and memory check): above it a check still runs; a timing inside its budget is a PASS whose line names the load it was measured at (a busier machine only makes timings worse), a timing over its budget is NOT JUDGED with its number printed. Memory and the app's own CPU are judged at any load (other processes do not change them). At or below it nothing changes. No budget is lowered or raised by this rule |
| NOTCH_DROP_HOLD_S | 10 | how long a mail that dropped into the notch stays shown before the shape shrinks by itself, unless the pointer passes, Escape or an act ends it first (lead, 2026-11-08, from the notch lane's N15; owner may change) |
| NOTCH_STAY_SHORT_S | 5 | the shorter time an alert can stay in the notch before it shrinks by itself (Settings › Notifications, "It stays": 5 s · 10 s = NOTCH_DROP_HOLD_S · until I act); depends on: the owner's picks of 14 Nov 2026 (docs/surfaces/mac-notch.md, the Settings drawing), NOTCH_DROP_HOLD_S |
| DRAG_STEPS | 12 | how many pointer moves the test socket's drag verb posts between the press and the release, so a SwiftUI drag gesture recognises the motion (lead, 2026-11-08, from shell round 6) |
| AUTOCONFIG_HTTP_TIMEOUT_MS | 4000 | the plan's value for each autoconfig HTTP request (ISPDB, the domain's autoconfig URLs, Autodiscover) (autoconfig crate, 2026-11-08) |
| AUTOCONFIG_BODY_MAX_BYTES | 262144 | the most bytes read from one autoconfig, ISPDB or Autodiscover answer (256 KiB); a larger answer is refused in words; source: rusted-mail provider_autodiscovery.rs:28 and 215-225 (research/dossier-rusted-mail-gaps.md row 2), number Cue (provider-limits lane, 2026-11-11) |
| PROBE_TIMEOUT_MS | 3000 | the plan's value for each TCP+TLS probe of the guess step (connect, handshake, greeting) (autoconfig crate, 2026-11-08) |
| IMAPS_PORT | 993 | IMAP over implicit TLS, RFC 8314 (autoconfig crate, 2026-11-08) |
| IMAP_PORT | 143 | IMAP with STARTTLS, RFC 3501 (autoconfig crate, 2026-11-08) |
| SMTPS_PORT | 465 | submission over implicit TLS, RFC 8314 (autoconfig crate, 2026-11-08) |
| SUBMISSION_PORT | 587 | submission with STARTTLS, RFC 6409 (autoconfig crate, 2026-11-08) |
| MLX_MODEL_ID | mlx-community/Qwen2.5-1.5B-Instruct-4bit | the on-device language model the install step downloads (Apache-2.0; the 0.5B answered the Ask prompt wrongly in the lane's run) (ondeviceai round 3, 2026-11-08) |
| MLX_MODEL_REVISION | 8b403126fc14f14cfc99bb4cfa72ecbc129ea677 | the pinned revision of MLX_MODEL_ID; the installer verifies every file's SHA-256 against MlxModelSpec (ondeviceai round 3) |
| MLX_MODEL_BYTES | 875668324 | the total size of the four model files the install step shows and checks (ondeviceai round 3) |
| IMAP_MAX_CONNECTIONS_DEFAULT | 5 | the PROVIDER cap: connections one account may hold open on a host with no special rule (the lane's proposal; the owner may set it); Whoosh itself opens IMAP_CONN_PER_ACCOUNT, which this cap bounds (IMAP round 2, 2026-11-08) |
| IMAP_MAX_CONNECTIONS_GMAIL | 15 | the PROVIDER cap for Gmail: 15 clients at a time per account across every app (research/mail-sync.md); Whoosh keeps IMAP_CONN_PER_ACCOUNT under it (IMAP round 2) |
| IMAP_IDLE_CONNECTIONS_PER_ACCOUNT | 1 | one IDLE connection per account, the push listener; a second is refused (IMAP round 2) |
| IMAP_TRANSCRIPT_LINES | 2000 | lines the per-account protocol log keeps for the Settings > Accounts download, secrets redacted (IMAP round 2, the lane's proposal) |
| VOICE_REFRESH_DAYS | 7 | JOURNEYS answer 13 (docs/JOURNEYS.md:139) |
| NOTCH_ARRIVAL_MS | 5000 after the core's arrival event | JOURNEYS answer 23 sends it here; architect value, owner confirms |
| PUSH_ARRIVAL_P95_S | 180 | Graph's 3-minute maximum notification latency (research/mail-sync.md:12); owner confirms |
| CODE_EXTRACT_MIN | 0.95 precision and recall | architect bar on the labelled parse fixture |
| CATEGORY_RULE_MIN | 0.90 | architect bar on the labelled split fixture |
| LIST_MIN_W | measured on the live design build | docs/design/geometry/mac-main.json min 960x600 |
| LIST_W_WIDE_PX | 480 | the list column's width when the sidebar is hidden: the list takes part of the freed width, the reader the rest; owner 2026-11-08 14:52 'please use the space better when extended'; number Cue, re-set on sight |
| LIST_W_WIDE_COMPACT_PX | 400 | the list's width when the sidebar is hidden at the compact window size (960 wide): LIST_W_COMPACT_PX plus the same 80 pt the regular size gains; owner 2026-11-08 14:52; number Cue, re-set on sight |
| DESIGN_TOLERANCE_PX | 1 | design-on-screen.md §2 re-measure tolerance |
| DECISION_PREVIEW_N | 20 messages | J5.8 'runs it on 20 recent messages' (docs/JOURNEYS.md:158) |
| DECISION_PREVIEW_MS | measured at the fn-decision-model rival probe | Laya's runtime (gap G20) |
| DECISION_BODY_CHARS | 2000 | what reaches Jev per message (plan interpretation 3); architect value, owner confirms |
| SPLIT_TABS_DEFAULT | Primary, Needs Reply, Updates, Promotions, Social, VIPs | docs/PRD.md:59; docs/DATA-MAP.md:48; the core SEEDS from this row (store seed.rs, contracts label_tabs.rs; tests tab_counts_equal_the_hand_labelled_ones): the six seeded tabs, Social and VIPs not pinned by default. 2026-11-09: two lead edits (Social dropped 11:20, VIPs dropped 13:20) each broke a core test and were reverted; the product's tab row = the pinned labels (D1), a seed change is the contracts lane's with its tests |
| ACCOUNT_RING_MIN_PX | 20 | docs/design/boards/directions/native-glass/index.html:53,216 |
| ACCOUNT_ICON_MAX_BYTES | 1,048,576 | upload limit; architect value |
| ACCOUNT_ICON_PX | 64 stored square at 2x | rendered inside the ring; architect value |
| PERF_LOAD_MAX | measured machine load above which a timing over its budget is not judged (a timing inside it passes; journeys/lib/loadgate.py) | REF/native-desktop/pipeline.md:47 |
| READER_CROSSFADE_MS | 80 | reader cross-fade when the selection changes; from the v2 board motion line; source: Cue, requested by the designtokens lane 2026-11-07 |; was 120, shortened 2026-11-08 10:51 on the owner's words "the animation is not snappy enough" |
| WIDE_MAIL_ZOOM_MAX | 3 | A146: the largest scale a pinch shows a mail at (three times its own size); the fit to the reading pane is the floor; Sources/Features/MessageBody/MessageBodyFit.swift `WideMailFit.zoomMax`; lane wide-mail 2026-11-11; number Cue, the owner may re-set it |
| TAB_MORPH_MS | 160 | split-tab morph duration; v2 board; source: Cue (designtokens request) |; was 220, shortened 2026-11-08 10:51 on the owner's words "the animation is not snappy enough" |
| TAB_MORPH_DAMPING | 0.8 | split-tab morph spring damping; v2 board; source: Cue (designtokens request) |
| ROW_COLLAPSE_MS | 120 | list row collapse after archive; v2 board; source: Cue (designtokens request) |; was 160, shortened 2026-11-08 10:51 on the owner's words "the animation is not snappy enough" |
| ROW_INSERT_MS | 140 | list row insert on arrival; v2 board; source: Cue (designtokens request) |; was 180, shortened 2026-11-08 10:51 on the owner's words "the animation is not snappy enough" |
| HOVER_TINT_MS | 80 | hover tint fade; v2 board; source: Cue (designtokens request) |
| UNDO_RISE_PX | 8 | undo capsule rise distance; v2 board; source: Cue (designtokens request) |
| FOOT_STRIP_PX | 36 | window foot strip height; v2 board; source: Cue (designtokens request) |
| LIST_PAGE_ROWS | 50 | conversations the list asks the engine for at first, and gains each time the owner nears the end of what it holds (AppModel `rowBudget`, `wantMoreRows`; equals the engine's first page, store read.rs INBOX_PAGE_ROWS; owner's store 14 Nov 2026: a tab of 178 conversations showed 50) |
| LIST_ROWS_AHEAD | 10 | a focus or a row scrolled into view this close to the end of the rows the list holds asks for the next page (set after LIST_PAGE_ROWS: a fifth of a page, so the page is there before the last row is reached at key repeat) |
| LIST_ROW_PX | 44 | list row height at density regular; DESIGN.md density line; source: Cue (designtokens request) |
| ACCOUNT_MARK_ROW_PX | 16 | account mark on a list row; v2 board; source: Cue (designtokens request) |
| ROW_MARK_PT | 12 | side of the pin, star, clip, mute and clock glyphs on a list row; board message-card-r1 design A (`.ic`); held in RowMarkSize.icon (InboxRowView.swift), set 2026-11-15 by the row-marks build |
| ROW_VIP_CHIP_H_PT | 12 | height of the VIP chip on a list row; same board (`.vip` line-height); RowMarkSize.vipHeight |
| ROW_VIP_CHIP_SIDE_PT | 3 | space left and right of the letters in the VIP chip; same board (`.vip` padding); RowMarkSize.vipSide |
| ROW_VIP_CHIP_R_PT | 3 | corner radius of the VIP chip; same board (`.vip` border-radius); RowMarkSize.vipRadius |
| ROW_VIP_CHIP_TRACKING_PT | 0.36 | letter spacing of the VIP chip; depends on: the board's .04em and the 9 pt of caption2 (0.04 x 9); RowMarkSize.vipTracking |
| ACCOUNT_MARK_COMPACT_PX | 18 | account mark in compact rows; v2 board; source: Cue (designtokens request) |
| ACCOUNT_MARK_REPLY_PX | 14 | account mark in the quick reply; v2 board; source: Cue (designtokens request) |
| ACCOUNT_MARK_SETTINGS_PX | 56 | account mark in Settings > Accounts; v2 board; source: Cue (designtokens request) |
| SETTINGS_WINDOW_W_PT | 940 | the Settings window's default width; picked board SB-S3 (owner 2026-11-08 15:4x); number Cue, re-set on sight |
| SETTINGS_WINDOW_H_PT | 680 | the Settings window's default height; picked board SB-S3; number Cue, re-set on sight |
| SETTINGS_WINDOW_MIN_W_PT | 700 | the Settings window's minimum width; below it the pane names shorten; number Cue |
| SETTINGS_WINDOW_MIN_H_PT | 480 | the Settings window's minimum height; number Cue |
| SETTINGS_SIDEBAR_W_PT | 250 | the Settings sidebar's width at the default window size; picked board SB-S3; number Cue |
| SETTINGS_SIDEBAR_MIN_W_PT | 210 | the Settings sidebar's width at the minimum window size; number Cue |
| ACCOUNT_ICON_RATIO | 0.6 | icon size over mark size inside the ring; v2 board; source: Cue (designtokens request) |
| ACCOUNT_RING_DASH_PX | 3 | dash length of the ring on an account that needs attention; depends on ACCOUNT_RING_MIN_PX; source: Cue, finding F4 of the designtokens lane |
| MOTION_FEEDBACK_FRAMES | 1 | frames between an input and its state change; owner 2026-11-07: no added latency; source: Cue (docs/MOTION.md) |
| MOTION_MAX_MS | 200 | longest animation; source: Cue (docs/MOTION.md) |; was 260, shortened 2026-11-08 10:51 on the owner's words "the animation is not snappy enough" |
| POPOVER_SCALE_FROM | 0.96 | popover entrance scale; source: Cue (docs/MOTION.md) |
| POPOVER_MS | 120 | popover entrance duration; source: Cue (docs/MOTION.md) |; was 160, shortened 2026-11-08 10:51 on the owner's words "the animation is not snappy enough" |
| ON_DEVICE_CONTEXT_TOKENS | 4096 | Apple Foundation Models context window; source: ondeviceai lane 2026-11-07 (Apple) |
| ON_DEVICE_CHARS_PER_TOKEN_FLOOR | 2.5 | a prompt is refused before the call when even this chars-per-token ratio cannot fit the window; an estimate, no tokenizer is exposed; source: ondeviceai lane |
| DECISION_MODEL_LAYA_MB | 843 | what the download of the decision model Laya weighs, in MB: the size the catalogue of on-device decision models shows on its line (Sources/Features/OnDeviceAI/Core/DecisionModelCatalogue.swift, SPEC A133) and the total the progress line counts to (the not-downloaded sentence rounds it to 'about 840 MB'); depends on the five files of the typed-decisions checkpoint on its page; number read from the app's own sentence, ondeviceai lane |
| DECISION_MODEL_KNOWLINE_08_MB | 1729 | what the download of the decision model KnowLine 0.8B weighs, in MB: the eight files of PelaAI/KnowLine-0.8B-Gen1 at revision 01952e55 the engine reads (1,728,946,660 bytes, summed from the repository's file sizes on 15 Nov 2026); the size its line shows (Sources/Features/OnDeviceAI/Core/DecisionModelCatalogue.swift, SPEC A133) and the total its progress line counts to |
| DECISION_MODEL_GLIDE_08_MB | 1525 | what the download of the decision model GLiDE 0.8B weighs, in MB: the seven files of fastino/GLiDE-0.8B at revision 9eacdeec the engine reads (1,525,378,481 bytes, summed from the files fetched on 15 Nov 2026); the size its line shows (Sources/Features/OnDeviceAI/Core/DecisionModelCatalogue.swift, SPEC A133) and the total its progress line counts to. The line is held: it joins the list only on the owner's word about its licence |
| LAYA_OPTION_TOKEN_CAP | 48 | Laya: tokens per answer option; the authors' rl_common.py build_sequence; source: ondeviceai lane |
| LAYA_OPTION_BUDGET_FLOOR | 16 | Laya: minimum token budget per option; same source |
| LAYA_MIN_HEAD_TOKENS | 8 | Laya: minimum head tokens; same source |
| SIDEBAR_W_PX | 220 | sidebar width at 1280x820; v2 board; source: macshell lane measurement 2026-11-07 |
| SIDEBAR_W_COMPACT_PX | 200 | sidebar width at 960x600; v2 board; same source |
| LIST_W_PX | 400 | list width at 1280x820; v2 board; same source |
| LIST_W_COMPACT_PX | 320 | list width at 960x600; v2 board; same source |
| SIDEBAR_FOOT_PX | 76 | sidebar foot reserved for the status lines; same source |
| SLIDE_MAX_ROWS | 4 | a selection highlight slides only when the old and new rows are at most this many rows apart, else it fades; source: owner reference 2026-11-07 (Filipiuk) |
| ICON_LIBRARY_MIN | 96 | the emblem icon library holds at least this many glyphs across its families; source: owner 2026-11-07 "rich enough icon library", number Cue, disclosed |
| EMBLEM_LIST_SATURATION | 0.85 | the 16 pt emblem on a list row uses the account colour at this share of its saturation so the mark stays quieter than the text; source: owner 2026-11-07 "subtle enough on the UI", number Cue, disclosed |
| ACCOUNTS_MENU_MS | 160 | the sidebar accounts menu pulls up and down in this time on the system spring; source: docs/MOTION.md, number Cue |; was 220, shortened 2026-11-08 10:51 on the owner's words "the animation is not snappy enough" |
| LABEL_DEPTH_MAX | 5 | a label path holds at most this many levels (client > Provanta > RFP > 2026 > Q4); source: owner 2026-11-07 "up to like 3-5 levels", the upper bound taken |
| OUTBOX_LATE_AFTER_MS | 60000 | a scheduled send counts as late this long after its time with no device awake; source: outbox crate tests (60 s), owner-visible in Scheduled |
| OUTBOX_TICK_MS | 1000 | how often the real core's outbox worker looks for a due entry while one is open (undo-send windows, send-later); engine-act 2026-11-10, number Cue; a tick is a cheap store read, nothing is sent before an entry is due |
| SEND_RETRY_TRIES_MAX | 4 | delivery tries of one queued message whose connection to the mail server broke (reset, closed, timed out, TLS) before the end of its body, the first included, before it stays failed for the owner's Retry; a break after the body is never retried (the server may hold the message); number Cue (fix-realcore round 15, owner 13 Nov: a reply failed on its first try with "Connection reset by peer" and was never tried again), depends on SEND_RETRY_START_S |
| SEND_RETRY_START_S | 30 | seconds before the second try of such a message; the pause doubles per try up to SEND_RETRY_PAUSE_MAX_S (30, 60, 120 s); number Cue (fix-realcore round 15) |
| SEND_RETRY_PAUSE_MAX_S | 600 | seconds, the longest pause between two tries of a message whose connection broke; number Cue (fix-realcore round 15), depends on SEND_RETRY_START_S and SEND_RETRY_TRIES_MAX |
| OUTBOX_STANDBY_AFTER_MS | 120000 | the standby device (the iPhone by default; the Mac is Primary) takes a scheduled entry this long after the primary missed it; source: Cue, disclosed |
| SYNC_HISTORY_PAGE | 200 | messages fetched per backfill page per folder; source: sync-engine lane, number Cue (IMAP servers answer 200-row FETCHes within a second on fast links) |
| SYNC_BACKOFF_START_MS | 2000 | first wait after a provider limit answer; source: sync-engine lane, number Cue |
| SYNC_BACKOFF_MAX_MS | 300000 | the longest wait between retries after provider limits; source: sync-engine lane, number Cue |
| SYNC_RETRY_COUNT | 5 | attempts per page before the account is marked failing with the server's words; source: sync-engine lane, number Cue |
| SYNC_FIRST_SCREEN | 50 | messages fetched per account before any backfill starts (the first screen); source: sync-engine lane tests (50) |
| SYNC_FIRST_SCREEN_DEADLINE_MS | 20000 | an account whose first screen has not answered by then joins the backfill queue so it cannot hold the others; source: sync-engine lane "not verified" item, number Cue |
| ACCOUNTS_MENU_CHORD | A | the sidebar accounts menu key: one letter, in every preset; owner 2026-11-07: 'dont be having complex shortcuts like "⌃⌘A"'; source: Cue, after the owner's rule |
| VECTOR_RSS_MB | 150 | the vector index's DIRTY memory footprint (phys_footprint, what the OS cannot reclaim) at 1M messages; clean pages of the memory-mapped index file are not counted because the OS drops them under pressure; measured 54 MB dirty at 1,100,000 chunks on 2026-11-07 (RSS 748 MB with the whole file touched); source: Cue ruling on the search-vector lane's red |
| SEARCH_WRITER_HEAP_BYTES | 200000000 | text index limit; source: the search-text lane's limits.rs, moved here so the number lives once |
| SEARCH_WRITER_THREADS | 2 | text index limit; source: the search-text lane's limits.rs, moved here so the number lives once |
| SEARCH_BATCH_ROWS | 5000 | text index limit; source: the search-text lane's limits.rs, moved here so the number lives once |
| SEARCH_RECHECK_TAIL_ROWS | 2000 | text index limit; source: the search-text lane's limits.rs, moved here so the number lives once |
| SEARCH_DEFAULT_HITS | 50 | text index limit; source: the search-text lane's limits.rs, moved here so the number lives once |
| SEARCH_SNIPPET_CHARS | 160 | text index limit; source: the search-text lane's limits.rs, moved here so the number lives once |
| SEARCH_RECONCILE_WINDOW_ROWIDS | 50000 | text index limit; source: the search-text lane's limits.rs, moved here so the number lives once |
| RELAY_RETRY_COUNT | 3 | attempts for a failed push relay send before it is marked failed; source: Cue, after the relay lane's open decision |
| RELAY_RETRY_BACKOFF_MS | 30000 | wait between push relay retries, doubled each time; source: Cue |
| RELAY_RENEW_TICK_S | 60 | the relay's renew job wakes this often; source: relay-adapters lane |
| RELAY_GRAPH_RENEW_MARGIN_MIN | 60 | a Graph subscription is renewed this long before it expires; source: relay-adapters lane |
| RELAY_APNS_REF_MAX_B | 3500 | the longest reference the relay puts in an APNs payload (Apple's payload cap is 4,096 bytes: the margin is the rest of the payload); source: relay-adapters lane, number from memory of Apple's limit, to verify against the APNs docs |
| SEARCH_RECENT_MAX | 8 | recent searches kept per device; owner 2026-11-08 "whatever you want to type and the actual result email"; source: search round 4 plan (default disclosed) |
| SEARCH_DROPDOWN_ENTER_MS | 140 | the suggestions dropdown fades in and rises 2 px; S1 Quiet rows board notes; within MOTION_MAX_MS |
| SEARCH_SKELETON_FADE_MS | 180 | each skeleton row crossfades into its result; S1 Quiet rows board notes; within MOTION_MAX_MS |
| SEARCH_SLIDE_MAX_ROWS | 4 | the dropdown highlight slides when the new row is this close or nearer, else fades (docs/MOTION.md rule 11); S1 board notes |
| SEARCH_HOVER_TINT_MS | 80 | hover tint on a suggestion or result row; S1 board notes |
| ATTACHMENT_INDEX_BATTERY_MIN | 50 | percent; attachment content indexing waits below it; search round 4 plan default |
| VIDEO_FRAME_EVERY_S | 10 | one sampled frame per this many seconds of video for the meaning index; search round 4 plan default |
| VIDEO_FRAMES_MAX | 30 | sampled frames per video file at most; search round 4 plan default |
| COMPACT_BELOW_W_PX | 1100 | window width below which the Mac shell places its compact column widths; shell round 7 (appshell lane) |
| EMBED_MODEL_BYTES | 1488915288 | the one EmbeddingGemma 2 safetensors file (text, vision and audio towers) the install carries; observed size of rig/embedding/weights/model.safetensors, 2026-11-08; embedding round 2. Contracts delta 15 (2026-11-09): this row stays the measured size of the one model.safetensors file; the 1,525,783,164 in embedding round 2's table is the sum of all seven loader files, a different quantity, so the two do not disagree once named |
| EMBED_MODEL_FILES | 7 | files the EmbeddingGemma 2 loaders open (config, tokenizer_config, preprocessor_config, processor_config, tokenizer.model, tokenizer.json, model.safetensors); embedding round 2 |
| HOVER_REVEAL_MS | 80 | a hover strip or control fades in over this; reader round 5 (the message header strip); equals HOVER_TINT_MS by design |
| SENDWAVE_MS | 720 | the send shockwave's whole run, from the send button to the pane's edge and fade; a celebration moment outside MOTION_MAX_MS by the owner's ask (12:32); number Cue from the reference video's pace; re-set on sight |
| SENDWAVE_RINGS | 4 | colour rings in the halo (the post) |
| SENDWAVE_CHANNEL_OFFSET | 0.08 | red and blue channel offset as a share of the green channel's (the post: +8% / -8%) |
| SENDWAVE_SCHEME | aurora | the default colour scheme (aurora · halo · frost · spark); owner default 12:32, a Settings choice later |
| SENDWAVE_LENS_PT | 6 | lens distortion of the send shockwave: how far content stretches outside the ring and compresses inside, in points; number the sendwave lane 2026-11-08, re-set on sight |
| SENDWAVE_BAND | 0.11 | the ring band's width as a share of the pane's diagonal; sendwave lane 2026-11-08 |
| SENDWAVE_TINT_LIGHT | 0.55 | ring tint strength in light mode (0 to 1); sendwave lane 2026-11-08 |
| SENDWAVE_SCREEN_DARK | 0.62 | ring screen-blend strength in dark mode (0 to 1); sendwave lane 2026-11-08 |
| SENDWAVE_EDGE_LIGHT | 0.75 | edge highlight strength on dividers and borders as the ring passes, light mode; sendwave lane 2026-11-08 |
| SENDWAVE_EDGE_DARK | 0.45 | the same in dark mode, subtler (the post); sendwave lane 2026-11-08 |
| SENDWAVE_SNAPSHOT_SCALE | 2 | the pane snapshot's scale for the wave's base layer (2 = Retina); sendwave lane 2026-11-08 |
| ATTACHMENT_THUMB_PT | 40 | the attachment chip's thumbnail side in points (generated at 2x); fits the chip at body + caption line height without growing the row; attachments-mac lane 2026-11-08 |
| ATTACHMENT_THUMB_CACHE_MAX | 64 | thumbnails kept in memory per run; bounds memory on image-heavy threads; attachments-mac lane 2026-11-08 |
| ATTACHMENT_DRAG_START_PT | 3 | pointer travel in points before a press on an attachment chip becomes a drag out; below it the press stays a click so a double-click with jitter still opens; attachments-mac lane 2026-11-08 |
| ATTACHMENT_GALLERY_TILE_PT | 88 | the side of an image or video tile in the attachment gallery (thumbnail at 2x); 120 at first, owner on the first build 16:14 'Make them smaller please' -> 88; number Cue, re-set on sight |
| ATTACHMENT_GALLERY_ROWS | 1 | rows of tiles the collapsed attachment gallery shows; tiles per row follow the pane width at ATTACHMENT_GALLERY_TILE_PT and the last slot of the last row is the "+N" tile when files remain; replaces ATTACHMENT_GALLERY_MAX (6, a tile count that left "+9" alone on a second row at 88 pt, seen 2026-11-08 18:30); number Cue, re-set on sight |
| BODY_PAPER_PAD_PX | 16 | the inner margin on all four sides of a designed mail's white page (the text never touches the page's edge); owner 2026-11-13 07:24 'some emails not super well displayed'; number Cue, re-set on sight |
| BODY_BLOCKED_TEXT_MIN_PX | 130 | a blocked picture declared narrower than this shows a small neutral mark with no words; the words ("Picture not loaded") need about 110 pt on one line; owner 2026-11-13 07:24; number Cue |
| BODY_RULES_WAIT_MS | 5000 | the longest a mail waits for the one shared block-list lookup or compile before it loads as plain text (a last resort, never the normal first open); round 21 and 24 |
| BODY_LINK_MAX_CHARS | 40 | a web address in a plain text mail longer than this is drawn as a short link naming its site ("tmcnet.com link"), on one line, never the tracking code; the full address stays in the tooltip, the copy and the click; owner 2026-11-13 07:29; number Cue |
| RSS_IDLE_MAX_MB | 300 | the app's resident memory at idle on the fixture corpus, no embedding model loaded, measured 60 s after launch; owner 2026-11-08 'beware of app speed and heavyness on ram and CPU'; number Cue, re-set on the first measurement (main35 measured 58 to 69 MB) |
| RSS_MODEL_MAX_MB | 2200 | resident memory with the embedding model loaded for a meaning search (weights 1.49 GB on disk); number Cue, re-set on measurement |
| LAYA_GROWTH_MAX_MB | 1400 | what the hidden copy's footprint may grow while Laya judges a batch: the peak while Laya judged minus the first sample, taken before the first mail (checks/laya-sanity.py `memory_cases` case laya-growth-under-LAYA_GROWTH_MAX_MB, and `mutant_stop`, which ends the keep-cache copy at the first sample past it). Source: the lead's main111 runs measured 2026-11-13 (plain 944 MB, keep-cache 2023 MB), _captures/lead/main111/proofs-hidden/lead-laya-sanity.log lines 53 and 54 (cache released per question: 179 MB at the start, peak 1123 MB, grew 944 MB over 192 questions) and lead-laya-sanity-mutant.log lines 53 to 55 (keep-cache: 158 MB at the start, 2061 MB after 3 mails, peak 2181 MB, grew 2023 MB, under RSS_MODEL_MAX_MB and so a survivor of that bound alone). 1400 is 456 MB over the plain growth and 503 MB under the kept cache's growth after its first 3 mails. A bar on the level after the first questions does not separate them (plain 1033 to 1123 MB, kept cache 2061 to 2181 MB: both about 100 MB). Depends on: the Laya checkpoint loaded in the app (ModernBERT-large) and one call's buffers (16 questions of about 140 tokens in the plain run, 4 in the mutant run), the sampling step (1 s plain, 0.25 s mutant); RSS_MODEL_MAX_MB stays the absolute bound; number Cue (fix-aisort round 13, 2026-11-13), re-set on measurement |
| LAYA_SECONDS_PER_MESSAGE_MAX | 1 | the most seconds one message may take while Laya with its real weights judges a Re-run of Needs Reply (checks/laya-real-check.py; the pass line docs/lanes/laya-real-weights.passline.md): a re-run step's time over the messages it added, the first step from the Re-run click, so it carries the model's load. The lead's rule (round 14 of fix-aisort, 2026-11-13): twice the median per-question time of the main111 plain laya-sanity run, rounded up to a whole second, never above 5. That log (_captures/lead/main111/proofs-hidden/lead-laya-sanity.log) holds no per-question time: line 2 gives 7.13932275 s of Laya, line 62 192 questions, a mean of 37.2 ms; twice it is 74.4 ms, rounded up to 1. Depends on: the Laya checkpoint and the Mac it runs on, the questions a message carries (one per AI-on label offered, LABEL_AI_QUESTION), RERUN_CHUNK 10 (core/crates/engine/src/flow_ctx.rs:186); number Cue from the lead's rule, re-set on measurement |
| FIXTURE_FULL_HISTORY_PER_ACCOUNT | 10000 | messages per account in `fixture load full-history-<n>`, the fixture the memory budgets are judged on (30 accounts = 300,000 messages, one every three hours per account, about 3.4 years of history); depends on: perf/memory-mac.py and fixtures/tests-j1.json "perf"; number Cue (a proposal), re-set when a real 30-account mailbox is measured. Contracts delta 13, 2026-11-09 |
| OAUTH_REFRESH_MARGIN_S | 300 | an OAuth2 access token is refreshed when fewer than this many seconds are left before it expires, so a sync or a send never starts with a token that dies mid-way (oauth::RefreshingToken); depends on: the providers' one-hour access tokens (expires_in 3600 in both token answers); number Cue. Contracts delta 14, 2026-11-09 |
| OAUTH_LOOPBACK_TIMEOUT_S | 300 | how long the sign-in listens on 127.0.0.1 for the browser's answer before it says so in words (oauth::SignIns); depends on: the person signing in and approving in the browser; number Cue. Contracts delta 14, 2026-11-09 |
| CPU_IDLE_MAX_PCT | 1.0 | the app's average CPU over 30 s at idle, no sync running; number Cue, re-set on measurement (main35 measured 0 to 5% under machine load 90) |
| EMBED_IDLE_UNLOAD_S (also the speech and on-device models) | 300 | the embedding model unloads this long after the last meaning search; it loads on the first search, never at launch; number Cue |
| EMBED_BATCH | 32 | mails per embedder call and per index step (4 forward passes of 8); a query waits at most one step, about 0.4 s at the Debug 13 ms per message; depends on: embedding r8 (docs/lanes/embedding.md "Round 8"); number Cue, re-set on measurement. Contracts delta 15, 2026-11-09 |
| EMBED_INDEX_CEILING_MB | 2100 | no index step starts at or above this footprint: the builder writes the vectors already done, clears the MLX cache, pauses EMBED_PAUSE_MS and retries; RSS_MODEL_MAX_MB 2200 minus 100 for one batch. Re-set on measurement, 2026-11-13: with the model resident the real engine's copy sits at 1822 to 1897 MB before any batch (main112, perf/footprint.py crawl, _captures/lead/main112q/proofs-hidden/alone-010.log), so the old 1800 let one step run and then held the indexer until the model unloaded (indexed 0 of 100 after 300 s); the buffer cache is now capped by EMBED_MLX_CACHE_MB. depends on: RSS_MODEL_MAX_MB, EMBED_MLX_CACHE_MB, embedding r8; number Cue. Contracts delta 15, 2026-11-09 |
| EMBED_MLX_CACHE_MB | 128 | the most MLX's buffer cache (the buffers a forward pass freed, kept for reuse) may hold, set when the embedding model loads (GemmaEmbedder.load, `MLX.Memory.cacheLimit`); without a cap the cache kept what the batches freed and the model-loaded footprint read 2445 and 2424 MB against RSS_MODEL_MAX_MB 2200 (main112, perf/footprint.py, _captures/lead/main112q/proofs-hidden/alone-008.log and alone-009.log: resident 911 and 919 MB, footprint 2445 and 2424 MB, the same through the unload wait); the weights are 1.49 GB on disk, so about 0.9 GB was cache; mlx-swift Memory.swift: "relatively small cache sizes (e.g., 2MB) perform just as well"; depends on: RSS_MODEL_MAX_MB, EMBED_INDEX_CEILING_MB; number Cue, re-set on measurement (hid-search round 24) |
| EMBED_I8_SCALE | 500 | the int8 scale of a unit vector's values (a value v is stored as round(v x 500), so it clamps at +-0.254 and rounds within 0.001); from searchbetter MeaningIndex.swift:25; depends on: EMBED_DIM, embedding r8; number Cue, re-set on measurement. Contracts delta 15, 2026-11-09 |
| EMBED_PAUSE_MS | 2000 | how long a paused index build waits before it reads the footprint again (EMBED_INDEX_CEILING_MB); depends on: embedding r8; number Cue, re-set on measurement. Contracts delta 15, 2026-11-09 |
| EMBED_NOTIFY_MS | 1000 | at most one MeaningSearch.onUpdate per this many ms while the index builds, so the search screen asks again for the hits so far without flooding; depends on: embedding r8, searchmac r9; number Cue, re-set on measurement. Contracts delta 15, 2026-11-09 |
| TAB_PINNED_MAX | 8 | the most pinned labels shown as tabs; the rest fold into the +N chip (D1, the plan of 2026-11-08); inbox list round 5 |
| PROBE_LINGER_S | 3 | the frame probe's geometry readers stay mounted this long after the last socket frame request, then unmount (appshell round 2) |
| PROBE_SETTLE_S | 0.15 | the frame probe waits this long for the view set to settle before it reads frames (appshell round 2) |
| MEANING_MIN_COSINE | 0.65 | a meaning hit needs at least this cosine to the query; provisional from 9 queries over the 11-message J2 corpus (unrelated queries 0.54-0.62, relevant mail 0.68-0.90, non-relevant mail for a related query at most 0.72); re-set on real mail; embedding round 4, 2026-11-08; RETIRED 2026-11-11: no shipped code reads it; SPEC A150 replaced a fixed cosine cut-off by the per-search lift (RELATED_LIFT_MIN) and SPEC A141 lists every mail by meaning with no cut-off (gap-ledger grep of core/crates, Sources, perf, checks, tools, scripts, relay: only the generated constants and comments) |
| ATTACHMENT_TEXT_MAX_CHARS | 200000 | the most text one attachment contributes to the word and meaning indexes (PDF, document, OCR); attachment-index round 1, 2026-11-08 (the lane's suggestion) |
| TRANSCRIPT_MAX_CHARS | 100000 | the most transcript text one audio or video attachment contributes; attachment-index round 1, 2026-11-08 |
| INTERACTION_MEASURE_WAIT_S | 2 | the socket's frame and commit measures answer not-judged when no frame presents within this; appshell round 2, 2026-11-08 (InteractionProbe.frameWaitS) |
| ADD_ACCOUNT_CONNECTING_MS | 3000 | a new account says "connecting" this long before "fetching the inbox"; round 1 held it as a private value; add-account round 2, 2026-11-08 (the lane's request, number the lane's) |
| ADD_ACCOUNT_MARK_SHEEN_MS | 240 | the provider mark's glass sheen fades in over this; the mark itself arrives in POPOVER_MS, the head's mark swap in READER_CROSSFADE_MS; add-account round 2 |
| ADD_ACCOUNT_UNFOLD_RESPONSE_S | 0.22 | the sheet's bottom edge spring response when Manual setup, the error block or the stop row unfold (the board's system spring); add-account round 2 |
| ADD_ACCOUNT_UNFOLD_DAMPING | 0.85 | the same spring's damping fraction; add-account round 2 |
| EMBLEM_DOT_PT | 0 | the account-colour dot on the G3 emblem chip, diameter in points; 0 hides it: owner 11:09 2026-11-08 picked 'Grey glyph but no colour dot' on the emblem board; EmblemTreatment.glass3DotDiameter |
| FOCUS_SLIDE_MS | 80 | the focus highlight slides to an adjacent row in this time; a second key inside it jumps with no slide (docs/MOTION.md rules 3 and 11); owner 2026-11-08 10:51: "navigation with arrows is WAYYYY TOO SLOW AND HEAVY"; number Cue |
| READER_MARK_IN_VIEW_PERCENT | 75 | a marked passage counts as in view, and the reader does not scroll to it, when it stands under the pane's top edge by a line (16 points) and above this share of the pane's height; depends on how much of the pane's foot the floating reply box and the last lines of a card take (about a quarter at the default window); under it the passage is brought into view as before; the lead's walk on main182 (the word quote: the sentence stood at 56 percent and the reader still moved, cutting the summary's top); number Cue, 14 Nov 2026 |
| SETTINGS_FOUND_SCROLL_MS | 60 | Settings scrolls its column to a found setting this long after the ring is set or the pane opens, so the column is laid out when the scroll is asked; depends on the time one pane needs to be laid out (a scroll asked in the same turn, or one turn later, moved nothing: main182, the hidden copy at the minimum window, 23 found entries stayed below the foot); number Cue, 14 Nov 2026 |
| SETTINGS_FOUND_SCROLL_AGAIN_MS | 400 | the same scroll is asked once more, without animation, this long after the first: a column still being laid out at the first ask, or an animation that did not run, ends on the found setting all the same; depends on the scroll's own animation (READER_CROSSFADE_MS) having ended; a second ask for a setting already in place moves nothing; number Cue, 14 Nov 2026 |
| SIDEBAR_COUNTS_FOLLOW_MS | 150 | the sidebar's label counts are read and drawn again this long after the mail last changed (a read, an archive, an arrival): one redraw after a burst of keys, never inside a key's frame; depends on the cost of that redraw on the real engine (drawn before the key's frame it added 17 ms to the archive's median: 34.4 ms on main184, 51.6 ms on main187, 34 archives, load 6 to 8) and on the key repeat rate (a held key repeats about every 33 ms, under this); the count may lag the tab strip by this long; source: Cue (lead, 14 Nov 2026) |
| BODY_UNDER_TEXT_WAIT_MS | 600 | the blocks under an open mail's text (its files, the thread's cards, the reply line) are drawn at the latest this long after the mail opens, whatever the text's page says; before that they wait for the text's first height, so they never stand under the header and jump down; depends on the longest time a page takes to give its first height (measured on main188, test engine: 45 ms for an ordinary mail, 155 ms for a 1300 pt page mail, 384 ms for the first mail after launch); number Cue, 14 Nov 2026 |
| ROW_EXIT_SLIDE_PT | 130 | a mail that leaves the list (E, H, L, P, a closing filter) slides this many points to the left as it dissolves; 0 or less = the row's own width; owner 2026-11-11 (SPEC A139): "on superhman they go on the left and dissolve gently", "but fast"; picked design L2 "a third of the way" (docs/design/boards/done-motion-r1/PICK.md); number the owner's pick |
| ROW_EXIT_SLIDE_MS | 150 | the slide and the dissolve of a leaving mail last this long; same pick |
| ROW_EXIT_CLOSE_DELAY_MS | 60 | the mails below start to close the gap this long after the key; same pick |
| ROW_EXIT_CLOSE_MS | 100 | the gap closes in this time; the whole exit is max(slide, delay + close) = 160 ms; same pick |
| LIST_REFRESH_COALESCE_MS | 80 | the trailing window in which the core's change events (a sync page, an import, an outbox tick) become ONE list refresh; the first event opens it; a held window (the owner mid-key) waits at most 3 more; shell-refresh 2026-11-10, number Cue; depends on: FOCUS_SLIDE_MS (same scale, one key frame) |
| HINT_HOLD_MS | 350 | holding ⌘ this long shows the key badges over every control with a bound key; release hides them; owner 2026-11-08 16:01 'when holding cmd we should be able to see shortcut appear across the screen'; number Cue, re-set on sight |
| ROUND_CELL_BUDGET_S | 600 | the verdict sweep kills a cell (and its rig copy) after this many seconds and reports it over budget; a manifest row may raise it in its budget column (core-unit 900); measured 2026-11-08: hidden copies died silently and a busy lock stalled a mutant job; number Cue, re-set on sight |
| MUTANTS_SHARDS | 8 | the round's core-mutants cell tests one shard of the workspace's mutants per build (shard = the build label's digits modulo this), so this many consecutive rounds cover every mutant inside ROUND_CELL_BUDGET_S; 2026-11-10 (lead): a full run never fit one round, and the cell's own copy of core/ could not build since main52 |
| TEXT_SIZE_MIN | 0.85 | the smallest text scale ⌘- reaches (1 = the default); macshell round 8; number Cue |
| TEXT_SIZE_MAX | 1.5 | the largest text scale ⌘= reaches; macshell round 8; number Cue |
| TEXT_SIZE_STEP | 0.15 | one ⌘= or ⌘- step of the text scale; ⌘0 returns to 1; macshell round 8; number Cue |
| COMPOSER_RECENT_FILES | 4 | recent files offered by the composer's attach panel; composer round 2; number Cue |
| COMPOSER_INLINE_ROWS | 6 | rows shown at once in the composer's inline snippet and emoji lists (; and :); composer round 2; number Cue |
| SEQUENCE_WAIT_MS | 1500 | a two-key sequence (G then I, 0 then 0) waits this long for its second key before it lapses and the first key is dropped; Cue's default 2026-11-08 (Superhuman publishes no wait); the Shortcuts engine, round 3 |
| ATTACHMENT_OCR_PAGES_MAX | 20 | a scanned PDF (no text layer) is OCRd page by page up to this many pages; owner default 2026-11-08 12:52 (contracts delta 10) |
| DRAFT_SUMMARY_LINES | 2 | lines of a saved draft shown on the thread's draft card (collapses to 1 while the reply field is focused) and in the row's preview; board thread-cards B, owner pick 17:57 (contracts delta 11, Cue default for the quickreply lane's draft card) |
| SUGGEST_SNOOZE_COUNT | 3 | the snooze of one thread that offers "Mute this thread" (the third); depends on: the suggestion rules, PRD 09:35. Read by FixtureCore and by the real core's suggestion engine when it lands (contracts delta 12, 2026-11-09) |
| SUGGEST_LABEL_REPEAT | 2 | the times the owner puts the same label on one sender before "Always label <sender>" is offered; depends on: the suggestion rules, PRD 09:35. Read by FixtureCore and by the real core's suggestion engine when it lands (contracts delta 12, 2026-11-09) |
| QUERY_CACHE_ENTRIES | 500 | typed queries whose vectors the query encoder keeps (the oldest goes first); depends on: searchbetter Embedder.swift:132-160 pattern (embedding lane). Contracts delta 12, 2026-11-09 |
| DICTATION_MS | 1500 | key release to cleaned text in the body, for 10 s of speech on the local engine; depends on: 10 s of speech, key release to cleaned text in the body on the local engine; a proposal from the dictation lane, re-measured at its first build (the latency_ms field of the mac-composer.dictation row), the owner confirms against 'dictation on the level of superwhisper'. Contracts delta 12, 2026-11-09 |
| MODEL_TRUST_LABEL | 0.8 | the trust of the Labels page's model entry (a fraction): the decision model files a message under a label by its meaning only at or above it; depends on: the decision model's label page; re-measured against the recorded Jev fixture. Read by the decision crate (core-decision r1). Contracts delta 12, 2026-11-09 |
| LABEL_AI_QUESTION | per-label | how the Labels model entry asks the decision model: one yes/no question per offered label ("This email belongs under the label <path>: <description>"), whose yes-probability is the number MODEL_TRUST_LABEL is compared with. Without this row the entry asks one choice among the labels, whose number is a share of a forced pick (measured on the owner's Mac, 2026-11-12: 50 mails judged, best share 0.58, 0 accepted at 0.8). Depends on: MODEL_TRUST_LABEL, the offered labels. Read by the decision crate (tuning::labels_per_label). Set by the lead 2026-11-12. |
| DECISION_RERUN_MS | 2000 | the bound of one re-run batch ("Re-run on N filed messages"); depends on: 20 messages per batch, PRD 09:55 re-run. Read by the decision crate (core-decision r1). Contracts delta 12, 2026-11-09 |
| NOTCH_ARRIVAL_W | 460 | width of the notch when a mail arrives (pt); depends on: board a Continuum, docs/design/boards/notch/a, night default 02:50. Contracts delta 12, 2026-11-09 |
| NOTCH_LIST_W | 540 | width of the notch's list (pt): the search field, the mails, the key hints; depends on: the pick page of 14 Nov 2026 (the list and the search drawn 540 wide; owner "1B 2A 3A"); was 480 (board a) until the search field joined the list |
| NOTCH_OPEN_W | 540 | width of the opened notch (pt); depends on: board a Continuum, docs/design/boards/notch/a, night default 02:50. Contracts delta 12, 2026-11-09 |
| NOTCH_SHOULDER_R | 10 | radius of the notch's shoulders where it meets the screen edge (pt); depends on: board a Continuum, docs/design/boards/notch/a, night default 02:50. Contracts delta 12, 2026-11-09 |
| NOTCH_CORNER_R | 24 | radius of the notch's lower corners (pt); depends on: board a Continuum, docs/design/boards/notch/a, night default 02:50. Contracts delta 12, 2026-11-09 |
| NOTCH_REST_WING | 40 | width of each wing beside the camera housing at rest (pt); depends on: board a Continuum, docs/design/boards/notch/a, night default 02:50. Contracts delta 12, 2026-11-09 |
| NOTCH_ROW_H | 58 | height of one row in the notch's list (pt); depends on: board a Continuum, docs/design/boards/notch/a, night default 02:50. Contracts delta 12, 2026-11-09 |
| NOTCH_CODE_BLOCK_H | 52 | height of the sign-in code block in the notch (pt); depends on: board a Continuum, docs/design/boards/notch/a, night default 02:50. Contracts delta 12, 2026-11-09 |
| NOTCH_RIG_PARK_PT | 6000 | how far below the screen a rig copy draws its notch panel (pt), so a drive never shows the fixture's mail on the owner's screen; checks subtract the `parked` field of `rows mac-notch`; from the owner's ghostty fork, branch one-terminal, macos/Sources/Features/OneTerminal/NotchPanel.swift `parkedIfTesting` (notch lane, lead's grant 2026-11-09) |
| NOTCH_POINTER_POLL_MS | 80 | the beat that re-reads a parked pointer behind the mouse monitors, so a shape that grows or shrinks under a still pointer updates its hover; from the owner's ghostty fork, branch one-terminal, macos/Sources/Features/OneTerminal/Notch/NotchShim.swift `NotchClock.pointerPoll` 0.08 s |
| NOTCH_POINTER_MIN_MS | 30 | the least time between two pointer decisions (about 33 per second); from the owner's ghostty fork, branch one-terminal, macos/Sources/Features/OneTerminal/NotchPanel.swift `pointerMoved` 0.03 s gate |
| NOTCH_TWITCH_SAMPLE_HZ | 30 | how often the twitch recorder samples the notch's laid-out tree while a drive has armed it; faster than any beat that moves a frame; from the owner's ghostty fork, branch one-terminal, macos/Sources/Features/OneTerminal/Notch/NotchTwitch.swift `sampleEvery` 1/30 s |
| NOTCH_TWITCH_TREE_DEPTH | 16 | how deep the twitch recorder walks the panel's view tree, deep enough to reach the rows; from the owner's ghostty fork, branch one-terminal, macos/Sources/Features/OneTerminal/Notch/NotchTwitch.swift `treeDepth` |
| NOTCH_TIP_DELAY_MS | 350 | how long the pointer rests on a notch control before its tip (name and key) shows; a non-key panel shows no system tooltip; from the owner's ghostty fork, branch one-terminal, macos/Sources/Features/OneTerminal/Tokens.swift `notch.wait.tip` duration 0.35 s |
| NOTCH_REST_SHOULDER_R | 8 | radius of the wings' smaller flare into the top edge at rest (pt); depends on: board a Continuum, docs/design/boards/notch/a; pairs with NOTCH_SHOULDER_R |
| NOTCH_ROW_R | 13 | corner radius of a row's hover plate in the notch's list (pt); depends on: board a Continuum, docs/design/boards/notch/a |
| NOTCH_CODE_R | 15 | corner radius of the arrival's code block (pt); depends on: board a Continuum, docs/design/boards/notch/a, NOTCH_CODE_BLOCK_H |
| NOTCH_BUTTON_H | 30 | height of Copy and Fill on the code block (pt); depends on: board a Continuum, docs/design/boards/notch/a, NOTCH_CODE_BLOCK_H |
| NOTCH_DONE_ROW_H | 62 | height of "That's everything" under the Unread list (pt); depends on: board a Continuum, docs/design/boards/notch/a |
| NOTCH_REPLY_R | 15 | corner radius of the reply field in the open mail (pt); depends on: board a Continuum, docs/design/boards/notch/a |
| NOTCH_SEGMENT_H | 22 | height of the Latest / Unread switch (pt); depends on: board a Continuum, docs/design/boards/notch/a |
| NOTCH_UNDO_EXPIRY_GRACE_MS | 300 | how close to the undo window's end a cancelled wait still counts as the end, so "Sent" shows when the reader's bar ends the window first; depends on: UNDO_SEND_DEFAULT_S |
| NOTCH_LIST_MAX | 5 | rows the hover list shows before "N more"; depends on: board a Continuum, docs/design/boards/notch/a, NOTCH_ROW_H |
| NOTCH_SOCKET_ROWS | 50 | row ids the control socket can click in the notch list (the list holds no more); depends on: docs/CONTROL-SOCKET.md |
| NOTCH_NOW_WINDOW_S | 60 | a mail younger than this reads "now" in the notch; depends on: board a Continuum, docs/design/boards/notch/a |
| NOTCH_WORDS_MIN_OWN | 5 | an open mail in the notch with fewer own words than this, whose content was forwarded, reads the forwarded content (a bare forward: what was forwarded is the mail); depends on: the owner's pick of 14 Nov 2026 ("its word by default"), contracts/src/notch_words.rs |
| NOTCH_WORDS_JOIN_BELOW | 60 | in the notch's words of a mail, a line shorter than this many characters joins the paragraph before it when that one is as short and does not end a sentence (text typed with hard line breaks reads as one paragraph); depends on: contracts/src/notch_words.rs |
| NOTCH_WORDS_MAX_PARAGRAPHS | 8 | the most paragraphs of a mail the notch's open card holds before the text fades out (the rest is one key away: Full mail, or Open in Whoosh); depends on: NOTCH_OPEN_W, the open card's height |
| NOTCH_WORDS_MAX_H | 118 | the most height the words of an open mail take in the notch before they are cut with a fade (pt); depends on: the open card of the owner's picks of 14 Nov 2026 (540 x 320 pt), NOTCH_WORDS_MAX_PARAGRAPHS |
| NOTCH_PAGE_H | 196 | height of the scroll area that holds the full mail (the sender's page) in the open card (pt); the card grows from 320 to 404 pt; depends on: the owner's pick of 14 Nov 2026 ("a button to display full mail"), NOTCH_OPEN_W |
| NOTCH_FOLD_LINE_H | 26 | height of the open mail's one-line strips in the notch: the folded parts, and the line that holds Full mail or Its words (pt); depends on: the pick page's open mail |
| NOTCH_SEARCH_H | 32 | height of the search field on the first line of the notch's list (pt); depends on: the pick page's list, NOTCH_ROW_R |
| NOTCH_SEARCH_ROW_H | 76 | height of one row of the search's answer in the notch: a list row with a third line, the passage that made the mail a result (pt); depends on: NOTCH_ROW_H, the pick page's search drawing (three lines a result) |
| NOTCH_KEY_SETTLE_MS | 600 | after the notch takes the keyboard, a loss of the key status inside this time is the app's activation settling, not the person leaving: the panel takes the keyboard back once (ms); depends on: NotchKeyboard.take (activation, then the panel made key) |
| NOTCH_LINE_W | 520 | width of the notch when a new mail shows as one line, who and the subject beside the camera (pt); depends on: the owner's picks of 14 Nov 2026 (docs/surfaces/mac-notch.md, alert style "One line"), NOTCH_OPEN_W |
| NOTCH_LIGHT_H | 3 | height of the light on the notch's lower edge when a new mail shows as a light, in the account's colour (pt); depends on: the owner's picks of 14 Nov 2026 (alert style "Light") |
| NOTCH_CANVAS_H | 600 | height of the notch panel's fixed transparent canvas, in which the shape grows (pt); depends on: the open mail's tallest stage, NOTCH_OPEN_W |
| CAP_H | 9 | height of a row action key cap while ⌘ is held (the caps float above the row icons); the room between the row above's subject text box (12 pt, 14.1 pt tall, centred in its 18 pt line) and the tallest icon ink (trash, tag: 2 pt into the 18 pt icon frame) is 11.5 pt, minus 1 pt clearance; lead ruling 2026-11-08 ("lower the caps; nothing covered"), number macshell; was 10, set to 9 on 2026-11-09 after ms16 measured 0.54 pt between the cap and the row above's subject box at 960x632 (lead ruling: shrink before moving) |
| LIFT_PANEL_INSET_PT | 16 | the floating reply box sits this far in from the reader's left and right edges; board thread-cards-r2/cards-s3 `.qr{left:16px;right:16px}`, owner pick "Lifted until it lands" 2026-11-08 00:35; designtokens round 4 |
| LIFT_PANEL_BOTTOM_PT | 14 | the floating reply box sits this far above the reader's bottom edge; cards-s3 `.qr{bottom:14px}`; designtokens round 4 |
| LIFT_PANEL_RADIUS_PT | 16 | corner radius of the floating reply box; cards-s3 `.qr{border-radius:16px}`; designtokens round 4 |
| LIFT_CARD_RADIUS_PT | 14 | corner radius of a lifted in-flight card (sending, draft, failed); cards-s3 `.mine::before{border-radius:14px}`; designtokens round 4 |
| LIFT_CARD_BLUR_PT | 18 | backdrop blur under a lifted card; cards-s3 `.mine::before{backdrop-filter:blur(18px)}`; the reply box keeps the glass blur (16); designtokens round 4 |
| LIFT_CARD_SATURATION | 1.6 | backdrop saturation under a lifted card; cards-s3 `.mine::before{saturate(1.6)}`; designtokens round 4 |
| LIFT_POPOVER_BLUR_PT | 22 | backdrop blur under the From picker popover, which takes the lift material; cards-s3 `.pop{backdrop-filter:blur(22px)}`; designtokens round 4 |
| LAND_AURORA_MS | 480 | the landing ring travels from the timer to the card's far corner and fades over its last 40 %; above MOTION_MAX_MS like SENDWAVE_MS, gates nothing; depends on: the landing board cards-s3 02:05 (docs/design/boards/thread-cards-r2/cards-s3/notes.md "Timings"), owner-confirmed 2026-11-09; designtokens round 5 |
| LAND_BAND | 0.11 | the landing rings' total width as a share of the card diagonal (the send wave's rule, on the card); depends on: the landing board cards-s3 02:05 (docs/design/boards/thread-cards-r2/cards-s3/notes.md "Timings"), owner-confirmed 2026-11-09; designtokens round 5 |
| LAND_GLASS_MS | 360 | the lift glass and shadow fade out (ease-in), from the landing frame; depends on: the landing board cards-s3 02:05 (docs/design/boards/thread-cards-r2/cards-s3/notes.md "Timings"), owner-confirmed 2026-11-09; designtokens round 5 |
| LAND_STATE_MS | 120 | the state word ("Envoyé 23:19"), the reply box header and Annuler cross-fade; also the reduced-motion glass fade; depends on: the landing board cards-s3 02:05 (docs/design/boards/thread-cards-r2/cards-s3/notes.md "Timings"), owner-confirmed 2026-11-09; designtokens round 5 |
| LAND_HAIRLINE_AT_MS | 300 | the flat message's hairline starts fading in this long after landing; depends on: the landing board cards-s3 02:05 (docs/design/boards/thread-cards-r2/cards-s3/notes.md "Timings"), owner-confirmed 2026-11-09; designtokens round 5 |
| LAND_HAIRLINE_MS | 120 | the hairline's fade-in; depends on: the landing board cards-s3 02:05 (docs/design/boards/thread-cards-r2/cards-s3/notes.md "Timings"), owner-confirmed 2026-11-09; designtokens round 5 |
| LAND_SETTLE_AT_MS | 300 | the card starts settling into the page this long after landing; depends on: the landing board cards-s3 02:05 (docs/design/boards/thread-cards-r2/cards-s3/notes.md "Timings"), owner-confirmed 2026-11-09; designtokens round 5 |
| LAND_SETTLE_MS | 160 | the settle's duration; depends on: the landing board cards-s3 02:05 (docs/design/boards/thread-cards-r2/cards-s3/notes.md "Timings"), owner-confirmed 2026-11-09; designtokens round 5 |
| LAND_SETTLE_PT | 2 | the card settles this far down into the page; depends on: the landing board cards-s3 02:05 (docs/design/boards/thread-cards-r2/cards-s3/notes.md "Timings"), owner-confirmed 2026-11-09; designtokens round 5 |
| LAND_ACTS_AT_MS | 420 | the Annuler row starts collapsing this long after landing; depends on: the landing board cards-s3 02:05 (docs/design/boards/thread-cards-r2/cards-s3/notes.md "Timings"), owner-confirmed 2026-11-09; designtokens round 5 |
| LAND_ACTS_MS | 120 | the Annuler row's collapse (the one collapsing row, MOTION rule 5); depends on: the landing board cards-s3 02:05 (docs/design/boards/thread-cards-r2/cards-s3/notes.md "Timings"), owner-confirmed 2026-11-09; designtokens round 5 |
| HINT_FOLLOW_MS | 50 | while ⌘ is held the key caps re-read their controls' frames this often, so a cap stays on its icon when the list moves its rows under it (a scroll to the focused row, the undo capsule or a suggestion entering the list's bottom inset); main38 J0.15 float-above red at 960x632; 2026-11-09, number macshell |
| RIG_FRAME_TICK_HZ | 120 | hidden rig only: the interaction measure's stand-in for the display link (none ticks on a hidden, alpha-0, accessory window: main44 rig-hidden-check), a main run-loop timer at this rate; the owner's copy keeps the display link; 2026-11-09, lead (appshell r7) |


### Added 2026-11-09 (lead): the rows four crates carried in their own pending tables (autoconfig, engine, junk, import); each crate's fallback table is deleted the same day

| Constant | Value | Depends on |
|---|---|---|
| PROTON_BRIDGE_IMAP_PORT | 1143 | Proton Mail Bridge's local IMAP port (https://proton.me/support/imap-smtp-and-pop3-setup); the autoconfig provider table |
| PROTON_BRIDGE_SMTP_PORT | 1025 | Proton Mail Bridge's local SMTP port (same page) |
| IMAP_PREVIEW_BYTES | 2048 | bytes of BODY[TEXT] the sync fetches per message for the row preview (sync-imap FetchOptions.preview_bytes); enough for the parse crate's preview after a quoted header block; number Cue (real core slice 2), re-set on measurement |
| BODY_PREFETCH_COUNT | 50 | newest messages per folder whose full body the sync downloads in the background right after a fetch and on each arrival, so opening or moving to a message never waits on the server; number Cue (body-r1, owner 2026-11-10 07:2x "in real inbox performance is really bad"), re-set on measurement |
| BODY_PREFETCH_BATCH | 10 | UIDs per UID FETCH (BODY.PEEK[]) in the body prefetch; number Cue (body-r1), re-set on measurement |
| BODY_FETCH_RETRY_S | 60 | seconds before a message whose body download failed is tried again (a failed body is never retried on every visit); number Cue (body-r1) |
| BODY_NO_BODY_TRIES_MAX | 4 | tries, in one launch, for a message whose server answer carries no body before the engine stops asking (the pause doubles from BODY_FETCH_RETRY_S between tries); a message no longer listed on the server is removed at once and is not counted here; number Cue (fix-realcore round 9, from the owner's log: 8 messages asked up to 76 times since 10 Nov), depends on BODY_FETCH_RETRY_S |
| BODY_NO_BODY_PAUSE_MAX_S | 3600 | seconds, the longest pause between two tries for a body the server did not send; number Cue (fix-realcore round 9), depends on BODY_FETCH_RETRY_S and BODY_NO_BODY_TRIES_MAX |
| BODY_RULES_WAIT_MS | 5000 | milliseconds, the longest a mail waits for the remote-content block list to compile before it loads as its text-only document (nothing remote can load in that document); a mail body is never left blank behind a compile that does not answer; number Cue (hid-list round 21, 13 Nov, after a picture of main112 showed a body not drawn 16 s after open) |
| OAUTH_HTTP_TIMEOUT_MS | 10000 | each request the real core's sign-in makes to the provider's authority (OpenID configuration, token exchange, refresh); covers a slow network without leaving the sheet waiting longer than a person would; number Cue (real core slice 2b), re-set on measurement |
| AI_HTTP_TIMEOUT_MS | 10000 | each request the core makes to an AI provider for the labelling flow (the rig's local fake included); same scale as OAUTH_HTTP_TIMEOUT_MS, which it borrowed until 2026-11-10 (realcore-slice3b); number Cue, re-set on measurement |
| ATTACHMENT_FILE_MAX_BYTES | 10485760 | bytes; the most ONE file may weigh on a draft being sent (10 MiB); depends on ATTACHMENT_TOTAL_MAX_BYTES and on the smallest provider message limit (about 20 MB once base64 has added a third); replies-contract delta 22 |
| ATTACHMENT_TOTAL_MAX_BYTES | 14680064 | bytes; all the files of ONE message together (14 MiB, about 19 MB encoded); depends on the smallest provider whole-message limit and on base64 growing a file by a third; replies-contract delta 22 |
| ATTACHMENT_KEEP_MAX_BYTES | 26214400 | bytes; the largest received file kept on this Mac when its mail is opened, and the largest DownloadAttachment accepts (25 MiB); depends on the smallest server message limit and on the disk an opened message may use; body-r2 |
| ATTACHMENT_BACKFILL_GAP_BYTES | 30000 | bytes; a stored mail whose server size exceeds its stored text and html by more than this is read again once for its file rows; depends on what a plain mail's headers and quoted text weigh (a plain mail's size is its text, so the gap is a file); body-r2 |
| AI_DRAFT_TIMEOUT_MS | 60000 | the longest one AI request may take while writing or rewriting a draft; depends on a hosted model's time for a whole reply; reply-engine round 6; the key check keeps AI_HTTP_TIMEOUT_MS |
| AI_VOICE_TIMEOUT_MS | 90000 | the same for the voice analysis of the sent mails; reply-engine round 6 |
| IMPORT_BATCH_MESSAGES | 200 | RAM held per store write: one batch of parsed messages sits in memory until the write commits; also the messages per SQLite transaction; owner rule "beware of app speed and heavyness on ram and CPU" |
| IMPORT_BATCH_BYTES | 16777216 | the same RAM bound counted in message bytes (16 MiB): a batch is written when either limit is reached |
| IMPORT_MAX_MESSAGE_BYTES | 134217728 | peak RAM for one imported message (128 MiB): a larger one is skipped by name |
| JUNK_MOVE_THRESHOLD | 0.90 | owner's brief (A91 spam): a message at or above it moves to Junk; SpamBayes' own `spam_cutoff` default; Settings tunes it; unmeasured, re-set on a real mailbox |
| JUNK_REVIEW_THRESHOLD | 0.70 | owner's brief: marks "suspected", moves nothing; Settings tunes it; above 0 and not above JUNK_MOVE_THRESHOLD |
| JUNK_UNKNOWN_WORD_PROB | 0.5 | Robinson's x, SpamBayes `unknown_word_prob` default |
| JUNK_UNKNOWN_WORD_STRENGTH | 0.45 | Robinson's s, SpamBayes `unknown_word_strength` default |
| JUNK_MIN_PROB_STRENGTH | 0.1 | SpamBayes `minimum_prob_strength` default; tokens closer than this to 0.5 are ignored |
| JUNK_MAX_DISCRIMINATORS | 150 | SpamBayes `max_discriminators` default |
| JUNK_PROB_EDGE | 0.000001 | keeps a probability off 0 and 1 before the logarithm |
| JUNK_MODEL_MIN_PER_CLASS | 20 | the model gives no opinion below this many learned messages in each class; depends on the first-run size of a real Junk folder |
| JUNK_TOKEN_TABLE_MAX | 60000 | owner rule "beware of app speed and heavyness on ram and CPU": the token table is pruned past this; the serialised size is asserted under 2 MB in tests/serialise.rs |
| JUNK_TOKEN_TABLE_PRUNE_TO | 48000 | a prune stops here so it does not run again at once |
| JUNK_TOKEN_BYTES_MAX | 300 | longest token the stored table accepts (a host token is at most 4 + 253 bytes) |
| JUNK_MAX_TOKENS_PER_MESSAGE | 400 | distinct tokens read from one message; bounds scoring time |
| JUNK_BODY_CHARS_MAX | 20000 | characters of body text read per message |
| JUNK_HTML_CHARS_MAX | 100000 | characters of HTML read for links |
| JUNK_SUBJECT_CHARS_MAX | 1000 | characters of the subject read for tokens; a longer subject is cut (tokens.rs read a bare 1_000 until 2026-11-10) |
| JUNK_FROMNAME_CHARS_MAX | 200 | characters of the sender's display name read for tokens (tokens.rs read a bare 200 until 2026-11-10) |
| JUNK_WORD_MIN_CHARS | 3 | SpamBayes' tokeniser keeps words of 3 to 12 characters and buckets longer ones |
| JUNK_WORD_MAX_CHARS | 12 | same |
| JUNK_LINKS_SCANNED_MAX | 200 | anchors examined per message |
| JUNK_BOOTSTRAP_MAX_PER_CLASS | 2000 | messages learned per class at first run, newest first |
| JUNK_RULE_SPF_FAIL | 0.30 | fixed score of a header rule: the chance, alone, that the message is junk |
| JUNK_RULE_SPF_SOFTFAIL | 0.15 | same; softfail is common on forwarded mail |
| JUNK_RULE_DKIM_FAIL | 0.30 | same; only when no signature of the message passes |
| JUNK_RULE_DMARC_FAIL | 0.50 | same |
| JUNK_RULE_REPLY_TO_MISMATCH | 0.25 | same; mailing lists trip it, so it is low |
| JUNK_RULE_DISPLAY_NAME_IMPERSONATION | 0.55 | same |
| JUNK_RULE_LINK_MISMATCH | 0.35 | same |

### Added 2026-11-09 (lead, main50 merge): the pop-out window rise

| Constant | Value | Depends on |
|---|---|---|
| COMPOSER_POPOUT_RISE_PT | 6 | the pop-out compose window opens with opacity and this rise (composer round 3, A-class motion: transform and opacity only); number Cue, the same rise the guide and the sheets use |

### Added 2026-11-09 (lead, main51 merge): the first sync's AI bound

| Constant | Value | Depends on |
|---|---|---|
| FLOW_FIRST_SYNC_AI_MAX | 200 | on a first sync the AI judges new mail and the newest this-many stored messages only; older history gets the entry rules and exit filters until the owner presses Re-run on a label (one model call per message: cost); number Cue, re-set on measurement |
| UNDO_CHIP_MS | 6000 | how long the undo capsule stays up; its outline timer ends with it; depends on nothing; set by lane undo-timer 2026-11-10 (the chip had no clock before), owner confirmation asked |

## The mail content engine (body-r4 constants, rows added by body-wire 2026-11-10; source: the round-4 brief, docs/lanes/body-r2.md "For the other builders")

| Constant | Value | Depends on |
|---|---|---|
| PARSE_HIDE_MIN_CONFIDENCE | 0.95 | at or above, with a structural anchor, a body segment is hidden (core/crates/parse/src/segment.rs 18) |
| PARSE_COLLAPSE_MIN_CONFIDENCE | 0.75 | at or above (below the hide band or without an anchor) a segment is collapsed; below, shown (segment.rs 20) |
| PARSE_HEADER_BLOCK_MIN_LINES | 3 | consecutive header lines (From and a date-like field) that make an embedded header block strong (segment.rs 22) |
| PARSE_SIGNATURE_MAX_LINES | 10 | non-blank lines at the end of the sender's text a signature may span (segment.rs 24) |
| PARSE_FOOTER_MAX_BLOCKS | 8 | non-blank blocks at the end of the sender's text a footer may span (segment.rs 26) |
| PARSE_BOTTOM_POST_MIN_LINES | 4 | a single quoted run on top with no attribution is a bottom post from this many lines (segment.rs 28) |
| PARSE_THREAD_MATCH_MIN_CHARS | 40 | a trailing run matched in an earlier message is history only from this many characters (segment.rs 30) |
| PARSE_THREAD_MATCH_MIN_WORDS | 3 | a block shorter than this is never matched against the thread (segment.rs 32) |
| PARSE_SHINGLE_WORDS | 3 | words per shingle of the thread match (segment.rs 34) |
| PARSE_SHINGLE_MIN_SHARE | 0.8 | share of a block's shingles found in an earlier message that matches it (segment.rs 35) |
| PARSE_ALIGN_MIN_SHARE | 0.85 | aligned share that matches a block in the bounded alignment (segment.rs 38) |
| PARSE_ALIGN_CANDIDATE_SHARE | 0.4 | shingle share from which the bounded alignment runs on a block (segment.rs 39) |
| PARSE_ALIGN_MAX_TOKENS | 400 | tokens the bounded alignment (LCS) runs over at most (segment.rs 40) |
| PARSE_PARTS_DISAGREE_FACTOR | 0.8 | confidence multiplied by this where the plain part shows as the sender's text what the HTML puts away (segment.rs 42) |
| PARSE_UTF8_REPLACEMENT_MAX_SHARE | 0.02 | declared UTF-8 with more replacement characters than this share is read as windows-1252, with a warning (canon.rs 8) |
| PARSE_STYLE_BYTES_MAX | 262144 | bytes of style text (256 KiB) past which no rule is inlined (css.rs 12) |
| PARSE_SELECTORS_MAX | 5000 | selectors past which no rule is inlined (css.rs 13) |
| PARSE_ELEMENTS_MAX | 50000 | elements past which no rule is inlined (css.rs 14) |
| PARSE_MATCHES_MAX | 5000000 | selectors x elements past which no rule is inlined (css.rs 15) |
| PARSE_CSS_DEPTH_MAX | 256 | nesting depth of the style inliner's walk (css.rs 16) |
| PARSE_DISPLAY_DEPTH_MAX | 128 | the display serialiser's depth cap (display.rs 15) |
| PARSE_PLAIN_QUOTE_DEPTH_MAX | 100 | nested ">" quotes a plain mail is drawn with at most (display.rs 17) |
| PARSE_LAYOUT_WRAPPER_MIN_WIDTH_PX | 300 | a wrapper with a background is a design signal from this width in px (display.rs 19) |
| PARSE_LAYOUT_WRAPPER_MIN_WIDTH_PERCENT | 50 | the same, as a share of the width in % (display.rs 20) |
| PARSE_LAYOUT_FIXED_WIDTH_MIN_PX | 480 | a fixed-width table from this width is a design signal (display.rs 21) |
| PARSE_LAYOUT_FIXED_WIDTH_MAX_PX | 800 | up to this width (display.rs 22) |
| PARSE_LAYOUT_CELL_BACKGROUNDS_MIN | 2 | cells with a background that make a design signal (display.rs 23) |
| PARSE_LAYOUT_PICTURES_MIN | 3 | pictures from which a mail is picture-heavy (display.rs 24) |
| PARSE_LAYOUT_TEXT_CHARS_PER_PICTURE_MAX | 300 | text characters per picture up to which a mail is picture-heavy (display.rs 25) |
| PARSE_LAYOUT_STYLE_RULES_MIN | 12 | style rules that make a medium design signal (display.rs 26) |
| PARSE_PAGE_HIDES_TEXT_MIN_WORDS | 12 | words of the text part from which a page is judged against it (display.rs `page_hides_text`; owner's picture 215, 14 Nov 2026) |
| PARSE_PAGE_HIDES_TEXT_SHARE | 4 | a page that shows under one in this many of the text part's words, and no picture over PARSE_LAYOUT_ICON_MAX_PX, is not served: the reader gets the text part (set after PARSE_PAGE_HIDES_TEXT_MIN_WORDS and PARSE_LAYOUT_ICON_MAX_PX; measured on the owner's 291 stored mails: 2 pages show under a tenth of the words, one of them built of 19 pictures; every other page shows a half or more) |
| PARSE_TEXT_PAGE_CODE_MIN_TAGS | 4 | tags of page elements, one of them a closing tag, from which a text part that starts with such a tag is page code and is read as its words (pagecode.rs `text_is_page_code`; owner's picture 216, 14 Nov 2026; measured on the owner's 297 stored mails: 2 text parts start with a tag, with 51 and 85 tags; no other text part holds more than 2) |
| PARSE_PAGE_HOLDS_TEXT_PERCENT | 75 | share of the text part's words from which a page holds what the mail says: under it, a stored page is made again from a text part of page code, and a page that hides its words gives way to the text part (set after PARSE_PAGE_HIDES_TEXT_SHARE; reads.rs `page_from_code`, `show_hidden_words`; the stored page of picture 216 held 49 percent) |
| PARSE_LAYOUT_TABLE_TEXT_SHARE_MIN | 0.5 | share of the text in layout tables that makes a medium design signal (display.rs 27) |
| PARSE_PREPARE_TIME_LIMIT_MS | 2000 | the time one message may take in the engine at body fetch before it degrades to escaped text with a warning (engine.rs 19) |
| PARSE_PREPARE_RETRY_TIME_LIMIT_MS | 20000 | the time the one further run of a message gets after its first run reached PARSE_PREPARE_TIME_LIMIT_MS (wall time: a loaded Mac spends it without the engine having worked); one such run at a time (engine.rs, bodies.rs `prepare_within`); owner's picture 210, 13 Nov 2026: 7 stored mails kept as escaped text |
| PARSE_HTML_NESTING_MAX | 2000 | HTML nesting past which a body is shown as escaped text, by a linear scan before any parser (engine.rs 22) |
| PARSE_BODY_BYTES_MAX | 8388608 | bytes (8 MiB) of one rendering (HTML or plain) past which the engine shows the body as escaped text with a warning; at body fetch the old sanitiser is then not run either (core/crates/parse/src/engine.rs 25) |
| PARSE_PICTURE_MIN_PX | 3 | a remote image is a tracking pixel only when it says it is invisible: a stated width or height under this, or hidden by its style; every other remote image is a picture (core/crates/parse/src/display.rs 65) |
| SEARCH_RANKED_MAX | 1000 | the most mails one search answer lists (exact matches, then by meaning, then by date); depends on the list's scroll cost and the 50 ms answer budget; number Cue; owner 2026-11-11 (SPEC A141): "It shoumld rank all emails based on similarity meaning from exact match to no match" |
| NO_VECTOR_SCORE_BELOW | 2.0 | a mail that has no meaning vector yet gets the score minus this number, under any cosine (the least is -1), so such mails sort below every ranked mail; written as a magnitude because the generator reads positive numbers only; depends on the cosine range only; number Cue |
| PREFIX_MIN_CHARS | 3 | a typed word of at least this many letters also finds the words that start with it (consultant finds consultants); a shorter word matches only itself; depends on how many index words share a short start; number Cue (SPEC A141); RETIRED 2026-11-11: no shipped code reads it; SPEC A149 withdrew the start-only rule and SUBSTRING_MIN_CHARS replaced it (gap-ledger grep: only its generated constant and the re-export in search-text/src/limits.rs, removed) |
| PREFIX_EXPANSIONS_MAX | 50 | the most index words one typed word stands for, the shortest first; depends on PREFIX_MIN_CHARS and the 50 ms answer budget; number Cue; RETIRED 2026-11-11: no shipped code reads it; SUBSTRING_EXPANSIONS_MAX replaced it (gap-ledger grep: only its generated constant and the re-export in search-text/src/limits.rs, removed). PREFIX_SCAN_MAX is NOT retired: search-text/src/query.rs:145 still reads it |
| PREFIX_SCAN_MAX | 2000 | index words read per field and segment while one typed word is widened; bounds the walk; depends on PREFIX_EXPANSIONS_MAX; number Cue |
| EXACT_WORD_BOOST | 2.0 | the weight of the typed word itself over a word that only starts with it; depends on the word score scale; number Cue |
| PEOPLE_PREFIX_MAX | 8 | people rows one suggestion read answers at most (SPEC A143); depends on the height of the suggestion list of design S1; number Cue |
| PEOPLE_BUILD_PAGE_ROWS | 2000 | mail rows read per step while the people list is first built; the list's lock is held for one step only; depends on the 50 ms read budget; number Cue |
| PIECE_TEXT_TOKENS | 400 | tokens of text in one search piece at most; 400 + title (PIECE_TITLE_TOKENS 48) + the document form's words (about 9) + 2 special tokens is about 459, under the model's 512 input (CHUNK_TOKENS), leaving about 50 tokens for differences between the Rust and the Swift tokenizer; DigUp keeps about 450 under the same cap (docs/lanes/pieces.md) |
| PIECE_OVERLAP_TOKENS | 50 | tokens two neighbouring pieces share, about one sentence or two; 1/8 of a piece, as CHUNK_OVERLAP (64) was of CHUNK_TOKENS (512) (docs/lanes/pieces.md) |
| PIECE_TITLE_TOKENS | 48 | tokens of a piece's title at most (a long subject plus " · file name"); the title is cut beyond; depends on PIECE_TEXT_TOKENS (the three sum under CHUNK_TOKENS) (docs/lanes/pieces.md) |
| CODE_MIN_CHARS | 16 | a word this long or longer of letters, digits and `+/=_-%.~`, with a digit in it, is a code (a tracking id, a hash) and is left out of the piece text; shorter ids (dates, prices, order numbers) stay findable (docs/lanes/pieces.md) |
| BASE64_MIN_CHARS | 32 | a word this long or longer of base64 letters with upper and lower case inside it is a code even without a digit; a long word without digits counts only when it is long and mixes case (docs/lanes/pieces.md) |
| SUBSTRING_MIN_CHARS | 3 | a typed word of at least this many letters matches every index word that HOLDS those letters (itself, a word that starts with it, a word that holds it inside: consultant finds brightconsultants); a shorter word matches only itself; depends on how many index words hold a short run of letters; SPEC A149 (docs/lanes/match-inside.md); replaces PREFIX_MIN_CHARS |
| SUBSTRING_SCAN_MAX | 400000 | index words read per typed word, across fields and segments, while looking for the words that hold its letters inside (the word list cannot be walked from a start for that, so it is read whole up to this bound); depends on the 50 ms answer budget and the size of the word lists; past it inside-word matches in the words not read are missed, while whole and start matches stay complete (docs/lanes/match-inside.md) |
| SUBSTRING_EXPANSIONS_MAX | 1000 | the most index words one typed word stands for (itself first, then the words that start with it, then the words that hold it, each group shortest first); depends on the 50 ms answer budget (one dictionary lookup and one posting list per word, field and segment); replaces PREFIX_EXPANSIONS_MAX (docs/lanes/match-inside.md) |
| SEARCH_PASSAGE_CHARS | 120 | characters of the passage a text match shows, cut at word boundaries around the first occurrence; depends on one line of the search row (SPEC A149 "one line of the text around them"); SEARCH_SNIPPET_CHARS (160) stays for the meaning rows (docs/lanes/match-inside.md) |
| RELATED_LIFT_MIN | 2.0 | a mail that is not a word match is RELATED when its lift (deviations above the search's mean closeness) is at least this; depends on the model's spread of cosines per query; DigUp's gate (Searcher.swift:116 `zGate: Double = 2.0`); measured on a private copy of the owner's mail (310 mails, counts only, checks/ownercopy-related.py, 14 Nov 2026): 16 topic queries mark 5 to 22 mails at 2.0 (170 in all; none over RELATED_MAX), 12 made-up queries mark 6 to 19, so the lift alone does not tell a topic from letters no mail is about: RELATED_MIN_COSINE stands beside it and 2.0 stays; SPEC A150 (docs/lanes/rank.md) |
| RELATED_DEVIATION_FLOOR | 0.02 | the least deviation a lift divides by, so a flat field (every closeness alike) lifts no mail; depends on the model's cosine scale (EmbeddingGemma: unrelated text 0.56 to 0.69); DigUp's floor (Searcher.swift:338) (docs/lanes/rank.md) |
| RELATED_MIN_POPULATION | 20 | under this many mails with a vector in scope no mail is marked related, because a mean and a deviation over a handful of mails say nothing; depends on how few mails make those two meaningless; DigUp's small-group line (Searcher.swift:342) (docs/lanes/rank.md) |
| RELATED_MAX | 50 | the most related mails one answer marks (the closest); depends on the list's first screen, not the model; lead's proposal (docs/lanes/rank.md) |
| RELATED_MIN_COSINE | 0.70 | a related mail's closeness (the cosine of its best piece to the query) is at least this, beside its lift; depends on the closeness the model gives a mailbox for text that is about nothing in it (EmbeddingGemma on the owner's 310 mails: the closest mail to each of 12 made-up queries reads 0.632 to 0.692; to each of 16 topic queries 0.659 to 0.780); set by the rule written in checks/ownercopy-related.py before its run: the lowest of 0.64 to 0.74 at which no made-up query marks a mail (0.68: 4 of 12 still do; 0.70: 0 of 12) while at least half of the topic queries still mark one (0.70: 12 of 16; 0.72: 8 of 16); item 7c5301c6, 14 Nov 2026 |
| ATTACH_AUTO_FETCH_MAX_BYTES | 10485760 | the biggest file fetched by itself (10 MiB); depends on the disk a file takes (ATTACH_AUTO_KEEP_TOTAL_MAX_BYTES), the time one fetch holds the connection and the memory of one decode (the whole part, about 1.4 x this in base64 plus the decoded copy) (docs/lanes/attach-fetch.md) |
| ATTACH_AUTO_FETCH_PER_STEP | 4 | files fetched per step before the queue yields; depends on how long the owner's own action may wait behind a step (4 small files, a second or two) (docs/lanes/attach-fetch.md) |
| ATTACH_AUTO_STEP_PAUSE_MS | 2000 | the pause between two automatic fetch steps; depends on ATTACH_AUTO_FETCH_PER_STEP: the connection is free to the body fetches and the owner's actions in between (docs/lanes/attach-fetch.md) |
| ATTACH_AUTO_KEEP_TOTAL_MAX_BYTES | 2147483648 | the total the automatic files may take on this Mac (2 GiB); depends on the owner's disk, about 10,000 files of 200 KB; at the cap the queue is dropped (docs/lanes/attach-fetch.md) |
| ATTACH_AUTO_QUEUE_MAX | 5000 | ids the automatic fetch queue holds at most; depends on memory (ids only, a few hundred KB) (docs/lanes/attach-fetch.md) |
| ATTACH_AUTO_RETRY_AFTER_MS | 604800000 | a failed automatic fetch is asked again after this long at the earliest (7 days), so a file is tried at most once a week, never in a loop; number Cue (docs/lanes/attach-fetch.md) |
| SENDER_DOMAINS_MAX | 8 | sender-domain rows one suggestion read answers at most (SPEC A153); depends on the height of the suggestion list, the same as PEOPLE_PREFIX_MAX (docs/lanes/domain.md) |
| IMAP_UID_SET_CHUNK | 500 | UIDs in one `UID FETCH <set>` line at most; depends on the command-line length servers accept: RFC 7162 section 4 asks clients to keep a line under 8192 octets, and 500 UIDs that are not consecutive take at most 500 x 11 = 5500 octets (docs/lanes/sync-vanish.md) |
| HELD_PAGES_MAX | 2 | pages of arrival headers held between a page's fetch and its store, at most; the engine's HELD_MAX is this times SYNC_HISTORY_PAGE; past it the oldest are dropped and the pass asks the server for them; a kick follows every stored page, so the list holds one page in practice; number Cue |
| ROWS_FOR_MESSAGES_CHUNK | 500 | ids per statement in the store's `rows_for_messages`; depends on SQLite's bound-variable limit (999 before SQLite 3.32, 32766 since) and on keeping the statement cache to at most two texts (a full chunk and the last one) |
| QUERY_MODEL_WORDS_MAX_CHARS | 400 | the most characters of the search model's own error words one log line keeps (an MLX error can be long); the 400 the lead called the query embed cap is this, not a cap on the text embedded; number Cue (docs/lanes/query-embed.md) |
| SYNC_RECENT_S | 300 | seconds within which the last successful sync still reads as recent on the iPhone Settings Sync row and for an icon travelling to the iPhone (plan fn-icon-file-sync, mob-settingsphone); depends on the sync interval; proposed by gap-phone-b (docs/lanes/gap-phone-b.md); checks/settingsphone-check.py:37 and SettingsPhoneTests.swift still hold a private 300 until they read this row; number Cue |
| DECISION_THREAD_CHARS | 2000 | characters of earlier messages one Jev request carries, newest first, the oldest dropped (decision `thread_context.rs` `jev_budget_chars`); equal to DECISION_BODY_CHARS today, so nothing changes until the owner picks another number; Laya's thread room is its window minus the body (ON_DEVICE_CHARS_PER_TOKEN_FLOOR), not this row; depends on: DECISION_BODY_CHARS, the provider's context; architect value, owner confirms (docs/lanes/gap-engine-tests.md, fix-core) |
| REPLY_WORK_START_MIN | 540 | minute of the local day (09:00) the owner's working hours start; the writer's free times (replies_work `calendar_free.rs`) are only offered inside working hours, Monday to Friday; depends on: the owner's own hours (a question to the owner); architect value (docs/lanes/gap-calendar.md, fix-core) |
| REPLY_WORK_END_MIN | 1080 | minute of the local day (18:00) the owner's working hours end; same use as REPLY_WORK_START_MIN; depends on: REPLY_WORK_START_MIN; architect value (docs/lanes/gap-calendar.md, fix-core) |
| REPLY_FREE_MIN_MIN | 30 | minutes: a free stretch shorter than this is not offered to the writer; depends on: the shortest meeting a person would accept; architect value (docs/lanes/gap-calendar.md, fix-core) |
| INLINE_PICTURE_MAX_BYTES | 10485760 | the largest picture (10 MiB) that goes inside the text of a message as an inline part; a larger one stays a file attachment; the core builds the message from these bytes in memory and the editor decodes a thumbnail, so this is a RAM bound ("beware of ram"); depends on: SMTP servers' message size limits (commonly 20 to 25 MB for the whole message), the composer's memory; architect value (docs/lanes/gap-composer-rich.md, fix-core) |
| EMBED_MS_MAC | 100 | median milliseconds to embed one 512-token chunk with the model loaded, Mac, release build; measured so far: median 74 ms, p95 123 ms on a DEBUG build (docs/lanes/embedding.md Perf line), no release figure; indexing runs off the main thread, so this is not a UI operation under the 50 ms objective; depends on: EMBED_DIM, CHUNK_TOKENS; number Cue, re-set on a release measurement (docs/lanes/gap-perf.md, fix-core) |
| EMBED_MS_IPHONE | not yet measured | milliseconds to embed one chunk on the iPhone 17 Pro; no number: no device or simulator runtime on this Mac, so Cue sets it from a device run (a simulator figure would not count); depends on: EMBED_MS_MAC (docs/lanes/gap-perf.md, fix-core) |
| OPERATION_MS | 50 | the owner's general budget, 2026-11-10 08:0x: "The objective is <50ms for any operation"; every operation the owner can make is under it, on the real core as on the fixture; a slower operation is a defect, never a budget change; same value as COMMIT_MS / INTERACTION_COMMIT_MS (which names the commit of an interaction) and as SEARCH_FIRST_SCREEN_MS and DECISION_PREVIEW_RULES_ONLY_MS (which name one operation each); tests that still hold a literal 50 for it are listed in docs/lanes/gap-ledger-2.md, not yet wired to this row; depends on: the owner's word (docs/lanes/gap-ledger-2.md) |
| DECISION_PREVIEW_RULES_ONLY_MS | 50 | milliseconds the FIRST run of the rules-only decision preview (20 mails of a 2,000-mail store, nothing warm) must stay within; no model is asked, so it is 20 reads and 20 pure walks and the owner's budget for any operation binds it (OPERATION_MS); judged by core/crates/engine/tests/engine_decision_apply.rs `the_rules_only_preview_on_20_of_2000_mails_stays_under_50_ms`, which prints `DECISION_PREVIEW_RULES_ONLY_MS`; the preview WITH a model keeps its own row DECISION_PREVIEW_MS; depends on: OPERATION_MS, DECISION_PREVIEW_N (docs/lanes/gap-decisions-real.md, gap-preview-core.md) |
| SIGNATURE_TAIL_MAX_LINES | 2 | non-blank lines that may follow the signature in a body and still leave it "already there" (the app's footer is one; room for one more); send crate `split_signature`; depends on: the footer being one short line the app adds after the signature; number Cue (docs/lanes/fix-signature-order.md) |
| SIGNATURE_TAIL_LINE_MAX_CHARS | 80 | characters of the longest line that may follow the signature (see SIGNATURE_TAIL_MAX_LINES); a longer line is text of the mail, so the signature is added again; depends on: the longest footer sentence in the 14 languages, which is well under it (docs/lanes/fix-signature-order.md); number Cue |
| HANDOVER_RIG_DELAY_MAX_MS | 60000 | the longest a TEST COPY may hold the meaning model's hand-over (env `WHOOSH_RIG_EMBED_HANDOVER_DELAY_MS`, read only when the copy is a rig; the owner's app gives 0), so a typo cannot leave a test copy without its model for good; Swift `EmbedderHandOver.rigDelayMaxMs` in BackgroundWork.swift; depends on: the longest search-before-the-model window a check needs (3,000 ms in docs/lanes/fix-rest-app.md) with a wide margin; number Cue (docs/lanes/fix-rest-app.md) |
| RICH_PICTURE_THUMB_PX | 600 | pixels of the longer side of the thumbnail the composer's text draws for a picture inside it (ImageIO decodes only this size, never the full picture), so a long draft stays light on RAM ("beware of ram"); Swift `RichPictures.maxSide`; depends on: the side the picture is drawn at in the text (280 pt in `RichPictures.drawnMaxSide`, a bare number with no row yet; 560 px at 2x, so 600 px is just above it); the reason for 600 itself is not written in the code or the notes: number Cue (docs/lanes/fix-screens.md) |
| RICH_PICTURE_CACHE_MAX | 12 | thumbnails the composer's text keeps in memory at most (NSCache count limit); at 600 px a thumbnail is at most 1.4 MB, so 12 are about 17 MB; depends on: RICH_PICTURE_THUMB_PX, the "beware of ram" rule; number Cue (docs/lanes/fix-screens.md) |
| REPLY_HEADER_LINE_MAX_CHARS | 400 | the most characters a line may hold and still be read as one reply header ("On ... wrote:" and its translations); a longer line is body text; search-vector `is_reply_header` in pieces.rs (the bare 400 docs/lanes/fix-core.md finding F named; it is NOT the piece size PIECE_TEXT_TOKENS); depends on: how long a mail program lets a header line grow (a header wrapped over two lines is read by `reply_header_at`, whose own opener cut is a separate bare 300); number Cue (docs/lanes/gap-ledger.md finding 3, gap-ledger-2) |
