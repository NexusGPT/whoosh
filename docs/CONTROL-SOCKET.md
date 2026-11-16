# Control socket: Whoosh

The drive channel of the native app (REF/native-desktop/rig-contract.md). Frozen per wave; a change is its own request row. Implemented in `Sources/ControlSocket/` (server and the verb registry `ControlVerbs.all`); features never add a verb, they register rows, keys and clicks in their own `FeatureModule` stub (`Sources/App/FeatureModule.swift`).

## Channel

- Socket: `$WHOOSH_SOCKET_DIR/whoosh.sock`, a Unix stream socket. The app opens it only when `WHOOSH_SOCKET_DIR` is set, which `scripts/launch-one.sh` does for the rig copy (`$D`). The owner's installed copy publishes no socket.
- Request: one JSON line `{"verb": "<verb>", "args": ["...", ...]}`. Arguments are text or numbers; a boolean, null, list or object is refused with `ok:false` (write a flag as text).
- Reply: one JSON line, `{"ok": true, ...}` or `{"ok": false, "error": "<words>"}`. Every request runs on the main actor, so a reply reflects what is on screen.
- Client: `tools/whoosh-ctl.py` (standard library). `python3 tools/whoosh-ctl.py <verb> [args...]` or `python3 tools/whoosh-ctl.py --script FILE` (one request per line). Exit 0 when every reply is ok, 1 when one is not, 3 with no socket.
- Launch: `sh scripts/launch-one.sh <bundle> -- [--app-flag ...] <command>`. Leading `--` words are the app's launch flags (`--fixture-core`); a command that is not a program on PATH is a verb and runs through `tools/whoosh-ctl.py`, so `scripts/launch-one.sh <bundle> -- --fixture-core rows mac-main.list` works as written. The journeys: `sh scripts/launch-one.sh <bundle> -- --fixture-core sh -c 'WHOOSH_SOCKET_DIR="$D" python3 journeys/run.py'`.

## Launch flags

| flag | effect |
|---|---|
| `--fixture-core` (or env `WHOOSH_FIXTURE_CORE=1`) | the core is `FixtureCore`, serving `docs/design/fixtures/mac-main-sample.json` (bundled in the app; env `WHOOSH_FIXTURE_PATH` overrides) in memory. The failing account is acc-18 Provanta RFP. |
| none | the Rust core through UniFFI; until the core lanes land, `UnavailableCore`: empty reads, every command fails with ActionFailed in words. `uistate.core` says which. |

## Verbs

| verb | args | reply fields | notes |
|---|---|---|---|
| `type` | text | none | appends to the focused text input |
| `key` | chord | none | the same path as the real keyboard (`AppModel.press`). `Z` undoes the most recent action not yet undone (last in, first out; see "## Undo"). A single letter is case-blind (`e` = `E`); named keys: `Enter`, `Escape`, `Tab`, `Shift+Tab`, `⌘K`, `⇧?`. A key that does nothing in the current state is `ok:false` |
| `click` | ax-id | none | `ok:false` when nothing is registered at the id, or it did nothing |
| `rows` | ax-id | `rows: [ {...} ]` | the rows a region shows; `ok:false` "no rows at <ax-id>" when no feature registers the region yet |
| `uistate` | none | `state: {surface, tab, counts, waiting, focus_message, focus_index, overlay, theme, account_filter, empty_state, core, last_failure}` | `counts` maps tab id to unread and mailbox name (lowercase) to its count; `waiting` maps tab id to waiting |
| `winframes` | none | `frames: [{title, x, y, w, h}]` | visible windows |
| `measure` | ax-id | `ms` or `frame: {x, y, w, h}` | `mac-main.launch` = process start to first paint, in ms |
| `labels` | optional `window:<id>` | `labels: [{ax, text, need_w, need_h, box_w, box_h}]`, `window` | every `.whooshAx` view on screen in that window (default: the key window, else the first visible one); `text` is null where the probe found no text; points. See "## Deltas, labels-verb" |
| `core` | `state`, path | `value` | the core's stored value at a path (`message/msg-01/archived`, `counts/primary`; `/` and `.` both separate) |
| `capture` | `shot`, name | `path` | PNG of the front window at `$WHOOSH_SOCKET_DIR/shots/<name>.png` (Mac) |
| `shot` | name | `path` | alias of `capture shot` |
| `clock` | `advance`, seconds | none | moves the fixture core's clock by a finite number of seconds up to ten years (315360000); larger, negative beyond that, or not finite is `ok:false`; `ok:false` on the real core |
| `fixture` | name, ... | none | fixture events, under `--fixture-core` only. Served: `fixture reauth <accountId>`: the failing account (acc-18, Provanta RFP) signs in again and becomes healthy, its commands succeed, its sync state clears, and a SyncStateChanged event fires; any other account id is `ok:false`. `fixture mcp-request <app> <tool> [words...]`: an allowed app asks for something that needs the owner's yes (the app must hold a grant, else `ok:false`). `fixture refuse <cause>` (SPEC A87): the next sign-in check fails with that cause and its recorded server words, once; causes basic_auth_disabled, bad_password, host_unreachable, tls_failed, smtp_refused, unknown; another id is `ok:false`. Others the journeys name (deliver, speak, paste, ...) are `ok:false` at this commit |
| `verbs` | none | `verbs: [...]` | the registry |

`rows` field sets served at this commit (base registrations, overridable by the owning feature lane):

| ax-id | row fields |
|---|---|
| `mac-main.list` | `ax` (`mac-main.list.row.<i>`), `id`, `sender`, `subject`, `account`, `unread`, `pinned`, `chips` |
| `mac-main.tabs` | `ax`, `id`, `title`, `unread`, `waiting` |

`core state` paths served by FixtureCore: `message/<id>/{archived,read,account,snoozed}`, `counts/<tab-or-mailbox>`, `pin/<id>`, `label_assignment/<id>`, `undo_entry/{count,consumed_last}`, `outbox_entry/count`, `outbox_entry/<n>/state`, `split_tab/<id>/name`, `snooze/<id>/return_local_time`.

Delta 16 (Primary and All are views, not labels; both cores serve them): `inbox/tabs` (JSON array, left to right: `id`, `name`, `kind` view|label, `unread`, `waiting`; views first), `view/<primary|all>/{name,unread,waiting,count,is_label}` (`is_label` is always false), `counts/primary` and `counts/all` (unread of the view), and `split_tab/<primary|all>/{name,order,enabled,count,unread,rule}` so the list's own tab reads keep working (`rule` reads `view:<id>`, `enabled` is always true). `label/<primary|all>/<field>` has no value: a view has no label row.

Delta 17 (a label is a flow of three cards; both cores serve it, the fixture core today): `label/<id>/flow/{entry_enabled,ai_enabled,exit_enabled}` (JSON bool), `label/<id>/flow/ai_reads` (`{"subject":..,"body":..,"attachments":..}`), `label/<id>/flow/exit_fallback` (a label id, `"primary"` or `"junk"`; no value = not chosen), `label/<id>/flow/counts` (`{"caught_by_entry","sorted_by_ai","kept_by_exit","sent_to_fallback"}`), `label/<id>/chains/<entry|exit>` (JSON array, one object per chain in number order: `chain_id`, `match` all|any, `conditions` (the store's JSON, a fact is `{"kind":"fact","fact":"unread","wanted":true}`), `facts` (the eight keys `unread, unanswered, has_attachment, starred, vip_sender, cc_me, mailing_list, replied_by_me`, each `any|yes|no`), `rows` (the text conditions under the grid)), `label/<id>/chains/<entry|exit>/count`, `message/<id>/why/<label id | primary | junk>` (`{"card":"entry|ai|exit|fallback|hand","chain_id":n|null,"decided_by":"rule:<id>|laya|jev|owner|default","matched":[conditions that held]}`; no value when the label does not hold the message), `message/<id>/flow_rejection` (`{"ai_label_id","sent_to","chain_id"}`; no value when the exit filter rejected nothing). Refusals: `exit_needs_fallback`, `bad_exit_fallback`, `bad_chain`, `ai_reads_nothing`, and `view_not_a_label` for a view as the label.

## Undo

Undo is last in, first out: `Z` (and the `Undo` command) reverses the most recent action that has not been undone yet, then the one before it, up to UNDO_DEPTH entries. Actions that write an undo entry: archive, snooze, pin, label, move to another account, quick send (undo cancels the send while it is still in its undo window). Opening a thread and marking read write none. With nothing left to undo, the reply is `ok:false` "Nothing to undo".

## Events

A core delivers its events in commit order from one serial queue: FixtureCore on the main thread (synchronously when the caller is on it, else through the main queue). On subscribe, a stream first sends its current state: Inbox (the Primary rows), Counts, SyncStatus (one SyncStateChanged per account; acc-18 carries its error until `fixture reauth acc-18`), Outbox (one OutboxTick per open entry, so an empty outbox sends nothing: read `WhooshCore.outbox()` for it).

## Region ids (every screen lane reads this table)

The `rows` regions the checks read (2026-11-07: the two base regions plus the 23 the seams lane reads in `checks/seams/seamlib.py`). The lane named serves the region by registering it in its `FeatureModule` stub; until it does, `rows` answers `ok:false` and a check that needs it is not judged. The row fields are the ones the seams lane reads; a lane may add fields, never rename these.

| ax-id | what it is | row fields | served by |
|---|---|---|---|
| `mac-main.list` | the inbox list of the current tab | ax, id, sender, subject, account, unread, pinned, chips (+ colour, icon, decided_by) | root base now; inboxlist |
| `mac-main.tabs` | the split-tab strip | ax, id, title, unread, waiting | root base now; inboxlist |
| `mac-main.status` | the status line under the list | text, unread, tab | macshell |
| `mac-main.sidebar.mailboxes` | the sidebar mailboxes (All inboxes, VIPs, Pinned, Snoozed, Sent, Drafts) | name, count | sidebar |
| `mac-main.sidebar.accounts` | the sidebar account rows | id, name, colour, icon, line | sidebar |
| `mac-main.sidebar.smart-folders` | the sidebar smart folders | name, count | sidebar |
| `mac-main.reader.header` | the reader's message header | message_id, account, colour, icon, date | reader |
| `mac-main.undo.bar` | the undo bar after an action or a send | text, seconds, entry | macshell |
| `mac-palette.results` | the command palette results | title, key | palettemac |
| `mac-search.results` | the search results list | id, account, colour, icon | searchmac |
| `mac-search.suggestions` | the search suggestions under the field | title, resolves_to | searchmac |
| `mac-main.snooze` | the snooze time picker | title, resolves_to | inboxlist |
| `mac-composer.header` | the full composer's From and signature | from, signature | composermac |
| `mac-composer.undo.bar` | the composer's undo-send bar | text, seconds | composermac |
| `mac-main.reply` | the quick reply under the reader | from, signature | quickreply |
| `mac-notch.list` | the notch's short list | id, account, colour, icon | notch |
| `mac-notch.undo.bar` | the notch's undo bar | text, seconds | notch |
| `mac-notch.reply` | the notch's quick reply | from, signature | notch |
| `mac-settings-ai.agents.card` | an agent's card in Settings: AI | name, allowed (words) | settingsaimac |
| `mac-ask.refusal` | Ask's refusal line when an action is not allowed | text, allowed (words) | askmac |
| `mac-ask.citations` | Ask's cited messages | message_id, account, date | askmac |
| `mac-welcome.accounts` | the accounts added on the welcome screen | id, name, colour | welcomemac |
| `mac-settings-accounts.accounts` | the account list in Settings: Accounts | id, name, colour, line | settingsaccountsmac |
| `iphone-inbox.accounts` | the iPhone account filter | id, name, colour, icon | inboxphone |
| `iphone-inbox.list` | the iPhone inbox list | id, account, colour, icon | inboxphone |

The journeys drive more ids than these (for example `mac-main.reader.subject`, `mac-trainer.hint`); each journey file names them, and the lane of that surface serves them under the same rule.

## Deltas from the runner's assumption

The runner (`journeys/lib/ctl.py`) assumed the planned contract; the real one differs here. The tests lanes adjust `journeys/lib/ctl.py` and the journeys; this lane never edits them.

1. `uistate.counts.snoozed` comes from the mailbox counts, lowercased (`snoozed`, `drafts`, `all inboxes`); a tab and a mailbox with the same lowercase name (`vips`) report the tab's unread.
2. `uistate` adds `waiting`, `focus_index`, `account_filter`, `core` and `last_failure`; `empty_state` is `null` until the InboxList lane serves it.
3. `rows` on a region nobody registers is `ok:false` ("no rows at <ax-id>"), never an empty list, so an absence check reads a real zero only from a region that exists.
4. `key` on a chord bound to nothing is `ok:false`; the harness records it as a failed action, and the check after it shows the effect.
5. `core state` accepts `.` as well as `/` between path parts.
6. `capture shot <name>` replies with the PNG `path`; `shot <name>` is the same verb.
7. `fixture <name>` is reserved and answers `ok:false` at this commit; `clock advance <s>` is served; `clock advance_to <words>` is not.
8. Accessibility ids follow `<surface>.<region>[.<element>...][.<index>]` (the contract crate's `parse_ax_id`): region-level ids like `mac-main.list` have two parts, element paths may be longer (`mac-main.tabs.edit.rename.needs-reply`).

## Deltas, 2026-11-07 (findings from the contracttests and tests-j1 lanes)

9. `clock advance` refuses a move that is not finite or exceeds ten years (it crashed the app on `1e308`).
10. A boolean argument is refused with `ok:false`; it was read as text "1" or "0", against this page.
11. `fixture reauth <accountId>` is served (above).
12. The SyncStatus and Outbox streams send their current state on subscribe; `WhooshCore` gains two reads, `outbox()` and `sync_states()` (Swift `syncStates()`). A Swift conformer written before them keeps compiling: `Sources/CoreBridge/WhooshCoreDefaults.swift` gives both an empty default.
13. Undo is last in, first out (written down above; the code did this already).
14. Events are delivered in commit order from one serial queue (above). No sequence number is added: it would change every event's shape.
15. After `P`, focus follows the pinned message to its new place at the top (the shell owns focus; it stayed on the old index before).
16. The Xcode projects are no longer tracked: `scripts/build-mac.sh` and `scripts/build-ios.sh` generate them from `apps/*/project.yml` into the build tree (`$OUT/project`), so a build never changes the worktree.

## Deltas, 2026-11-07 (root delta 2)

17. The region-id table above is the one source for `rows` ids; it includes the 23 regions the seams lane reads.
18. The design fixture's counts are consistent (docs/lanes/contracts.md "## Fixture"): `list_page` states that `messages[]` is the first page of Primary (16 rows, 8 unread, of 23 unread in the tab).

## Deltas, 2026-11-07 (appshell lane)

17. `shot <surface>`: when the argument is a registered surface id, the app opens that surface's window first (main window, Settings window on that page, or a surface window titled with the id) and replies `{path, surface}`; any other name shoots the front window as before.
18. `uistate.empty_state` is served from the registry (`FeatureModule.emptyState`, the inbox feature's zero-state words); `uistate` adds `settings_page`.
19. `fixture deliver <arrival> <fixture-file> [source]`: reads `arrivals.<arrival>` from the file, posts it as an arrival event; reply `{delivered, id, in_inbox: false}` (no inbox row until FixtureCore has an insert door). Unknown arrival or missing file is `ok:false` with words.
20. `fixture fail-next <op> <reason words...>`: the next run of `<op>` fails once with the reason (`last_failure.reason_words`); a core command's op is its case in kebab case (`archive`, `move-to-account`); a feature's own save consumes it with `FixtureFaults.take`.
21. `measure <ax-id>` returns `frame: {x, y, w, h, source}` in window points, origin top-left, for any view marked `.whooshAx("<ax-id>")` (`source: "probe"`); else from the accessibility tree when an assistive client has built it (`source: "accessibility"`); else `ok:false` "no measurement at <id>: nothing on screen carries that id (<n> accessibility elements walked)". SwiftUI builds no accessibility tree in-process without a client, so a plain `.accessibilityIdentifier` is NOT measurable: mark the view with `.whooshAx` (it sets the same accessibility id). `mac-main.launch` still answers `ms`.
22. `resize <width> <height>`: sets the main window's content size in points, waits until the frame holds still, replies `{content: {w, h}, frame: {w, h}}` as settled (the shell's minimum may clamp it: `resize 960 600` settles at 960x632 with the macshell root at this commit). Non-positive or non-finite is `ok:false`.
23. `theme light|dark|system`: sets the app's and every window's appearance, waits until each visible window draws in it, replies `{theme}`. `uistate.theme` is now read from the app's effective appearance (it was the constant "light").
24. `fixture theme light|dark|system` is the same as `theme` (the journeys call it this way, J1.py:87). Before this, J1.1/theme-applied passed only because `uistate.theme` was the constant "light".

## Deltas, 2026-11-07 (appshell lane, round 3)

25. Keys route by FOCUS (searchmac D10). `key <chord>` (and the real keyboard) asks every feature that binds the chord: first the features holding the focus (`FeatureModule.hasFocus`: by default, the current surface other than mac-main or the overlay is one of the feature's surfaces; a feature overrides it for a focused pane or field), then the rest in registry order, then the root's base. A handler returning false passes the key to the next; one returning false AFTER an action failed (`last_failure` set) ends the chain, so a refused action is not retried by another feature. `ok:false` only when no handler took the key. Enter reaches its handler through the Shortcuts lane's command table (`open`), which still takes the first provider in registry order: see docs/lanes/appshell.md.
26. `fixture corpus <name|path>`: the fixture core serves that file's mail. A path when the argument holds "/", else `<name>.json` in `$WHOOSH_FIXTURES_DIR`, else beside the launch fixture. Reply `{corpus, rows}`. Until FixtureCore adopts `CorpusLoading` (asked of the contracts lane), `ok:false` "the fixture core cannot load a corpus yet"; a missing file is `ok:false` "no corpus <name>".
27. `drag <from-ax-id> <to-ax-id>`: mouse down at the first element's centre, DRAG_STEPS moves, mouse up at the second's, posted into the app's own event queue (the owner's cursor never moves). Reply `{from: {x, y}, to: {x, y}, window, delivered}` in AppKit window points. Both ids must be measurable (`.whooshAx`, delta 21) and in one window; else `ok:false` naming the id. Proven: events reach the element's window. Not proven: that an AppKit drag session (`.draggable`/`.dropDestination`, the labels tree) completes from queued events; no measurable drag target exists at this commit.
28. `uistate.theme` / `theme` unchanged; `measure` probe frames now carry `window` (taken from the id's surface prefix).
29. `fixture corpus <path>` loads a `whoosh-search-corpus/1` file through FixtureCore.loadCorpus(from:) (lane/contracts); the rows are replaced, accounts, tabs and labels stay; a file of another schema is `ok:false` with the core's words. Without `--fixture-core`: `ok:false` "fixture verbs need --fixture-core".
30. `drag` refuses an id it cannot place with `ok:false` "nothing at <ax-id>".
31. `rows mac-main.account-book`: the root's account directory (AccountBook): `{id, name, address, colour, icon_id, unread, health}` per account, health = the sync error in words or null.
32. Enter (and every key that reaches a command through the Shortcuts command table) is routed by focus like delta 25: `CommandRegistry.handler` asks the providers in `Registrations.byFocus` order, a provider returning false passes the command on, the stand-in answers last. The note in delta 25 that Enter still went to the first provider no longer holds.

## Deltas, 2026-11-07 (shell round 6)

33. `shot` captures the LIVE window as the window server composites it (Sources/App/ShotLive.swift), not a render of the SwiftUI tree. It asks CGWindowListCreateImage on the window's own id (no Screen Recording permission is needed for a window of the same process; the SDK marks the call unavailable, so it is looked up by name), then ScreenCaptureKit, then the old tree render. The reply gains `source`: `live`, `render` (forced by `WHOOSH_SHOT_RENDER=1`, the mutant switch), or `render (live capture failed: <why>)`. Path, size (window points x backing scale) and theme behave as before; the shot is no longer flattened onto the window background (the window's own pixels are the shot). A notch panel and an overlay slot are windows with a window number: they capture the same way. `measure` is unchanged.
34. `resize <w> <h>` acts on the window of the current surface (main window, the Settings window on a settings page, a surface window opened by `shot <surface>`), lifts a scene's fixed size (`.resizable`, no max), and replies `{content, frame, window}` with the size AppKit kept and the window's title. The shell's own minimum clamps only a smaller request: the main window's 960x600 content minimum is already 600 high, but in a build where `resize 960 600` answers 632 the clamp comes from the main window, not from this verb.
35. `drag` replies `received` (mouse events the app's dispatch saw while it ran) and `active` (the app was active during the drag), and now asks the app to activate for the drag (the app that was in front gets the focus back after). NOT FIXED: on the rig copy `active` stays false and `received` stays 0, so the posted events still do not reach the window. Next step, not built: dispatch each event with `NSApp.sendEvent` (the window's own event path) instead of `postEvent` plus a run-loop turn, which does not drain the NSApp event queue.
36. `shot` asks for the window image again for up to one second when the window was opened a moment ago and is not on screen yet.

## Deltas, 2026-11-08 (shell round 7)

37. `activate`: asks for the app to come forward (replies `{active}` at once; the window server answers after the verb returns, so call another verb before `drag`). The front app is restored after the next drag. `drag` hands each event (press, DRAG_STEPS moves, release) to `NSApp.sendEvent` on the main actor, the app activated for the gesture and the front app restored after. The reply keeps `{from, to, window, delivered, received, active}`: `delivered` = DRAG_STEPS + 2 and `received` counts the mouse events the app's dispatch saw.
38. Screen-facing `fixture` verbs (Sources/ControlSocket/FixtureEvents.swift). Each posts `whoosh.fixture.<verb>` (userInfo `verb`, `args`, `text`) and answers `{posted, args}`; the screen that owns the behaviour consumes it. A verb no screen consumes changes nothing. One line each:
    - `fixture reset`: clears typed text, the overlay, armed faults and delivered arrivals, closes every window but the main one, returns to mac-main.
    - `fixture paste <text...>`: pastes text into the focused field (an API key).
    - `fixture paste-accounts <text...>`: pastes account lines (or a fixture name) into Welcome.
    - `fixture speech-fail [reason...]`: the speech engine fails. `fixture speech <text...>` and `fixture speak <text...>`: dictated text without the microphone.
    - `fixture first-sync <n>`: n accounts indexed so far (whole number from 0).
    - `fixture mcp-search <app> [query]`: an app asks the MCP server to search. `fixture mcp-server on|off`: the server switch.
    - `fixture upload-icon <path-or-bytes>`: the account icon upload. `fixture add-account <address> [name] [colour]`: a new account.
    - Ask: `fixture provider-fails <reason...>`, `fixture delete-message <id>`, `fixture agent-allowed <comma-separated actions>` (with `paste` and `reset` above).
    A wrong argument count is `ok:false` in words; an unknown verb lists the served ones.

## Deltas, 2026-11-08 (notch lane, round 4)

33. The `fixture` table gains five rows, each a notification the notch reads (Sources/ControlSocket/FixtureVerbs.swift): `fixture front-app <name>` (reply `{front_app}`), `fixture pointer-rest <ax-id>` (a rest of the pointer on that surface; `mac-notch` opens the list; reply `{rested_on}`), `fixture drop-file <ax-id> <file>` (a file dropped on that surface; reply `{target, file}`), `fixture mute-account <accountId>` (that account's arrivals never drop into the notch; reply `{muted}`), `fixture notch-geometry none|notch` (the panel as on a Mac without or with a notch; anything else is `ok:false`; reply `{shape}`).

## Retired (2026-11-08): the Ask panel
The `mac-ask.*` rows, clicks and the Ask panel are gone with Sources/Features/AskMac (owner 01:46: no separate "Ask AI"; the search carries the question). The fixture verbs `provider-fails`, `delete-message` and `agent-allowed` stay: other screens use them. `checks/parity/par-askmac.py` and `checks/seams/citation.py` are retired.


## Deltas, 2026-11-08 (appshell lane, round 3)

39. Hidden rig (owner 14:55: "stop doing tests with previews across the app please"). `scripts/launch-one.sh` launches every rig copy HIDDEN unless `RIG_VISIBLE=1`: it hands the app `WHOOSH_RIG_HIDDEN=1` and opens it with `open -g`. The app (Sources/App/HiddenRig.swift) then keeps every window at its real place and size but at alpha 0 and click-through, never activates, refuses a Dock tile, and makes its status item invisible. Every verb answers as before, with these differences: `shot` renders the window's view tree (source `render (hidden rig)`, the window's full frame, title band included) because the live image of an alpha-0 window is empty; `activate` answers `{active, hidden: true}` and does nothing; `drag` never activates the app (a synthetic click acts as on an active app); `window fullscreen` is refused (delta 41); a Dock switch is refused and listed in `uistate.hidden.refused`. `uistate` adds `hidden: {on, refused}`. `winframes` rows add `kind`: `window`, `panel`, `sheet` or `status-item`. Proof: `scripts/hidden-check.py` (the window server read from outside the app every 20 ms by `scripts/rig-windows.swift`).
    Checks that need the real screen, run with `RIG_VISIBLE=1` in a window the lead announces: par-sidebar's toggle pixel scan (`external_dy`, `screencapture -R`), par-macshell J0.2 (the Dock tile), the full-screen round trip (delta 41), screen recordings, and any pixel comparison against the live window (the sendwave strips, the shell-r6 live-shot checks).
40. `type <text>` asks the focused features first (`FeatureModule.typeText`); a field that owns its text takes it, else it goes to the shared buffer `AppModel.typed`. The quick reply owns its draft: a reply survives ⌘K and Esc, and the search's words never reach it.
41. `window fullscreen on|off`: the main window enters or leaves full screen and the reply `{fullscreen}` comes once AppKit says it is there; `ok:false` in words when it did not happen in time. Refused on a hidden rig: full screen moves the screen to a Space of its own.
42. `fixture mcp-request <app> <tool> [words...]`: an outside app asks the MCP server for a tool (FixtureCore.requestMcpTool); the reply carries `request_id`, and Settings shows a pending request. An app without a grant is `ok:false` in the core's words.
43. `drag <from> <to> [app|view]`: `app` hands each event to NSApp.sendEvent (`received` counts what the app's dispatch saw), `view` to the window's own sendEvent (`received` counts what was handed). Default `app` on a hidden rig, `view` on a visible one (sidebar round 6: the window's path commits a SwiftUI drag on an active app). Queued events (postEvent, then a drained queue) never reached an inactive window, so the queue is not used. The reply adds `path` and `hidden`.
44. System UI on a hidden rig (Sources/App/HiddenSystemUI.swift): Quick Look, open and save panels, alerts, context and pop-up menus, the sharing picker, NSWorkspace opening another app or Finder, and user notifications (posts and the permission prompt) are never shown; each ask is recorded and answered as a cancel. `core state system-ui/<kind>` = how many asks (`quick-look`, `save-panel`, `alert`, `menu`, `share`, `open`, `notification`); `core state system-ui/<kind>/<n>` = what the n-th ask was. `uistate.hidden.system_ui` maps kind to count. `fixture system-ui <kind>` (hidden rig only) makes the real framework call for quick-look, menu, save-panel, alert or share; `fixture system-ui hooks` lists every hooked entry point. Lanes call `HiddenRig.allowSystemUI(kind)` before showing system UI.

## Deltas, 2026-11-08 (appshell lane, round 4: shot pixels and measure frames in one space)

45. `shot` and `capture shot` reply `mapping: {x, y, scale}`. A `measure` frame point (x, y), in content-view points with origin top-left, is the PNG pixel ((x + mapping.x) * scale, (y + mapping.y) * scale). `mapping.x`/`mapping.y` are where the content view's top-left sits in the shot, in points: the title band's height (and 0 for x) when the shot holds the whole window (hidden rig render, live capture), 0 when it holds only the content view (the render fallback on a visible copy). `scale` is pixels per point. Every pixel check maps frames through it; no check needs to know which path drew the shot.
46. `shot <name> [ax-id ...]` and `capture shot <name> [ax-id ...]`: each ax-id after the name comes back in `frames: {id: {x, y, w, h} | null}`, read in the same main-thread turn as the pixels (content-view points, origin top-left; map them with `mapping`). A frame taken by a separate `measure` can belong to an earlier layout: on a thread still settling, a chip's `measure` read y 612.5 while the shot drew it at 473 (the content shrank 139.5 pt in between). A pixel check reads its frames from the shot reply, never from a `measure` before it.
47. `measure` (probe source) now reads the marked view's AppKit geometry at request time, converted to its window's content view (it was SwiftUI's `.global`, the last value a GeometryReader published).
48. Every Mac window measured at this commit (mac-main, the Settings window, and 9 surface windows) draws its content under the title band: `mapping` is `{x: 0, y: 0, scale: 2}` for all of them on a hidden rig. The mapping matters for any window that does not.

## Deltas, 2026-11-09 (contracts delta 12)

49. `fixture indexing <n>|all` (fixture core only): the first n of the fixture's accounts, in their listed order, are indexed and the others are still loading. The core's sync status says so: `syncStates()` and the sync-status stream (one `syncStateChanged` per account) give `history_fetched: false`, `messages_indexed: 0`, status words "indexing your mail" for each loading account. This is what the search screen's "accounts still loading" header reads. `all` puts every account back to indexed. The failing account stays failing. n below 0 or above the account count is `ok:false` in words. `fixture reset` does not change it; `fixture indexing all` does. Unlike `fixture first-sync <n>`, which only posts to the screens, this changes the core's answer.
50. `fixture import-sources [<kind>=<count> ...]` (fixture core only; delta 18, welcomemac-r4): what the next `ImportScan` finds on this Mac, replacing the default (Apple Mail); kinds apple-mail, olm, mbox; no spec = nothing found. A malformed spec is `ok:false` with the core's words.
51. `fixture import-progress <run> <done|finish|refuse|fail>` (fixture core only): one step of a scripted import run; answers `{run, state}`; an unknown run or a run already over is `ok:false` with the contract's words.

## Deltas, 2026-11-09 (appshell lane: the hidden shot's limit)

50. Hidden rig, `shot` (delta 39's family): the hidden render draws the window's view tree with `cacheDisplay`, and that path draws NO NSVisualEffectView material: a surface on the system glass (sidebar, panels, popovers, the notch) comes out empty, so its text sits bare over whatever is under it (the components lane saw text drawn over list rows in a hidden shot of a panel that was right on screen). A hidden shot therefore proves layout, text and placement, never the material. Stand-in: the designtokens lane gives WhooshGlass a rig-mode fill (hidden rig only) so a glass panel shows its bounds in shots; that fill is not the product's look. The real material is judged on the owner's screen, never from a hidden shot.

## Deltas, 2026-11-09 (contracts delta 13)

51. `fixture mcp-server on|off` sets the fixture core's MCP server (SetMcpServer), and `fixture mcp-search <app> [query]` asks the core (`FixtureCore.mcpSearch`): the server must be on and the app must hold a grant, else the reply is ok:true with `refused` (words) and no prompt; otherwise a pending request (`request_id`) and an `McpApprovalNeeded` event; after `AnswerMcpApproval` allowed, `core state mcp_request/<id>/results` lists the subjects found.
52. `fixture load full-history-<n>`: the fixture's first n accounts each get FIXTURE_FULL_HISTORY_PER_ACCOUNT messages (the memory budget's fixture); reply `{loaded, accounts, messages}`; n outside 1..30 is ok:false in words.
53. `fixture deliver` (supersedes delta 19's `in_inbox: false`): the arrival becomes an inbox row in the fixture core, filed as it arrives by the label rules, then by the fixture model's meaning; reply `{delivered, id, in_inbox: true, labels}`. An arrival without an id or an account, a taken id, or an unknown account is ok:false in words.

## Deltas, 2026-11-09 (appshell lane, round 5: the perf gates' measures)

54. `measure mac-main.first-frame` replies `{ms}`: process start to the first presented frame, taken on the main queue's next turn after the first RootView appears (`AppModel.firstFrameMs`, set once). Before that turn: `ok:false` "not judged: no first frame has been presented yet". It is never below `measure mac-main.launch` (taken in that same appearance).
55. `measure mac-main.interaction.gap` replies `{ms, event}`: for the last event the socket's `key` or `click` delivered, the longest interval between two consecutive display-link ticks from the event's arrival to its frame (the arrival is the first point, the frame's target time the last). A main thread blocked for three refreshes shows as a gap of about three refreshes even when the frame lands on time. The display link now runs from the arrival (it started at the commit); `.frame` and `.commit` keep their meaning. Not judged as before: no event yet, or no frame within INTERACTION_MEASURE_WAIT_S.

## Deltas, 2026-11-09 (contracts delta 14)

56. `fixture oauth approve|refuse <address> [words]` (fixture core only): the fake authority answers the address's waiting OAuth2 sign-in (after `SetOAuthClient` and `StartOAuthSignIn`); `approve` makes `oauthSignInState` Done, `refuse` makes it Refused with "the provider refused the sign-in: <words>". No browser opens, no network is dialled and no Keychain item is written under --fixture-core. An address with no waiting sign-in is ok:false in words.

57. `fixture accounts none|all` (fixture core only): `none` makes the core list no account (`accounts()` empty, `account/count` 0), which is the sidebar's empty state (glass brand mark, "No account yet", an "Add account" button `mac-main.sidebar.empty.add`; rows `mac-main.sidebar.empty`); `all` lists every account again. `fixture reset` does not change it; `fixture accounts all` does. Reply `{accounts}`; any other word is ok:false.
58. `fixture first-start on|off` (fixture core only): `on` clears the flow's flag (`whoosh.firstStart.finished`, the app's own UserDefaults, never the owner's) and opens the first-start flow at step 1 over the main window; `off` closes it with nothing left over. `fixture first-start reduce on|off` is the flow's Reduced Motion switch for the rig (loops freeze on a key frame, the card morph is a cut, the ring is not drawn). Rows: `mac-first-start.state` (active, step, skipped, lifted, finished, accounts, frozen, loop_s, morph_ms, veil_ms, ring_ms), `.card` (step, kicker, title, body, count "Step N of 8", dot_current), `.controls` (back, skip, continue, or add-account in the skip state: title, key, enabled), `.dots` (8 rows), `.lit` (toolbar, sidebar, list, reader: lit), `.keys` (step 3: the sample rows left, focus, last key), `.search` (step 2: typed, match, why), `.launch` (accounts, finished, would_show), `.geometry` (window, card, welcome_card, medallion, halo). Clicks: `mac-first-start.continue|back|skip|add-account` (false when off or not on screen), `mac-first-start.relaunch` (the launch rule again: the flow opens only when no account exists and the flag is unset). The step-2 try-it takes `type <word>`; the step-3 try-it takes `key J|K|E|H`; every other key is swallowed while the flow runs. The flow is not a surface of its own: `shot <name>` shoots the main window with it on.

59. The rich text draft's doors (lane richtext-r2; fixture core only; Sources/Features/RichText/RichTextFixture.swift). `fixture format bold <text>` | `list <item> [item...]` | `link <text> <href>` | `picture <cid>`: the draft in the shared rich text session becomes that content (a new call replaces the draft); `fixture reload-draft`: the draft is written to its MIME HTML part and read back from it; `fixture paste-html <html>`: the HTML is pasted and parsed to the composer's own set (no script, event handler or source font survives). Reply `{verb, paragraphs}`; a malformed call is `ok:false` with its usage. `core state draft/current/mime` = the HTML part of the draft on screen (a JSON string, `""` for an empty draft); `core state draft/current/text` = its plain words. Read the structure back with `rows mac-composer.runs`. `checks/richtext-check.py` drives these.
60. `fixture provider-endpoint <provider> <url>` (fixture core only; served at Sources/ControlSocket/FixtureEvents.swift:21, stored by `FixtureCore.setProviderEndpoint` from Sources/Features/SettingsAIMac/AIFixture.swift:51): names where a pasted provider key is checked. After it, a key saved for `<provider>` is POSTed to `<url>/validate` with the key as a Bearer token; HTTP 200 connects the provider (its `models` list from the JSON reply, else the seed list), any other status refuses it with the reply's `error` words. An empty `<url>` removes the endpoint (the length rule only: 8 characters or more, no space); `fixture reset` removes every endpoint. `laya` names where a test copy fetches the decision model's weights (LayaOnDevice). Loopback rule (rig-net-swift, owner: "nothing leaves this Mac except the relay"): on a test copy (`CorePlace.rig`) the `<url>` must be on this Mac, host 127.0.0.1, ::1 or localhost (`RigNet`, Sources/CoreBridge/CoreHost.swift); any other host is `ok:false` "provider-endpoint on a test copy takes only an address on this Mac (127.0.0.1, ::1 or localhost); <url> is not", and the key check refuses it again before the POST (`FixtureCore.keyCheckURL`): `ai_provider/<id>/state` is "refused" and `ai_provider/<id>/refusal` says "This test copy reaches only addresses on this Mac, so nothing was sent." (in the chosen language). The owner's copy takes any `<url>`.

## Deltas, 2026-11-10 (fixture-r3: four doors)

- `fixture reply-arrived <message, draft or outbox id>` (fixture core only): the person answers; a new unread message lands in that thread, so `reply_reminder/<id>/fired` reads false after the clock passes and `thread/<id>/reminder` reads `answered`. Reply `{thread_of, message, reminder}`.
- `core state mailbox/reminders/rows` (and `/count`): the number of threads in the Reminders view (the threads whose reminder fired); the same rows `inbox(query:)` lists for the `reminders` mailbox.
- `fixture upload-icon <bytes|path>`: consumed by the OPEN account icon picker as the file panel's answer; reply adds `accepted` and `reason_words`. With no picker open the reply has `refused` and no `accepted`. `rows mac-settings-accounts.icon.stored` = `[{account, px, pixels, bytes}]`, one per uploaded icon, `px` the stored side at 1x.
- Palette command "Not spam" (no key; id `not-spam`): `click mac-shortcuts.action.not-spam` or Enter on its palette line sends `MarkNotSpam` for the selected rows, else the focused row.
- `app-active on|off` (lane keys-r4, A106): runs the two handlers a real focus change runs on the held-⌘ caps, off = the app resigned active (the caps hide), on = the app became active with the live `NSEvent.modifierFlags` read (the caps hide unless ⌘ alone is down). The hidden rig never moves the real focus, and no notification is posted (other screens listen to them). Reply `{active, simulated: true}`; another word is `ok:false`.

## Regions added by doors-compose (2026-11-10)
- `rows mac-composer.to.completions`: the suggestion rows under the To field (one row per suggestion; click door per row).
- `rows mac-main.quickreply.from.filter`: the From menu's filter row in the quick reply (the typed filter text).

## Regions and verbs added by settings-integrate (bg-hooks, 2026-11-10)
- `fixture power <percent> <charging|battery> <nominal|fair|serious|critical>`: scripts the power reading the app tells the engine (percent 0 to 100; `serious` and `critical` both mean heat at serious or worse). Reply `{percent}`. Anything else is `ok:false` with the usage line. `fixture power real` goes back to the Mac's real reading (reply `{source: "real"}`).
- `fixture embedder loaded|unloaded`: the meaning model is handed to the engine or taken back, as its own load and unload do. Reply `{embedder}`.
- `rows mac-main.background`: one row: `source`, `battery_percent`, `charging`, `thermal_serious` (what was read); `sent`, `sent_battery_percent`, `sent_charging`, `sent_thermal_serious`, `sends` (what the engine was told); `crawl` running|paused, `reason`, `floor`; `owner_notes`, `owner_calls`; `embedder` handed|held, `handovers`, `takebacks`; `pending_ms` while a drift is held back.

## Deltas, probes-r1 (rounds 4 to 6)
- `winbands`: `{bands: [{id, title, band, fullsize, lights}]}`, one row per visible titled window; `band` is `frame.height - contentLayoutRect.height`, `lights` the traffic-light frames in content points.
- `keyevent [arm:a|b|c|d] <chord|text>` and `rows mac-keyevent.monitor` (`seen`, `sent`, `arm`): real NSEvents (Sources/ControlSocket/KeyEventVerb.swift).
- `fixture digest`, `core state fixture/digest`, `fixture install-state new`, `fixture reset` (restores the fixture core to its launch state), `scripts/launch-one.sh --relaunch`.
- `shot <name> window:<id>`: photographs the OPEN window whose identifier (or title) is `<id>` (the ids `winbands` serves), moving nothing. The reply carries `window` and `title`: the window the picture is of. `shot <name>` for a registered surface opens its window as before; a Settings pane that has no window of its own (`mac-settings-replies`) moves the Settings window to that pane and photographs it, and never opens a window titled with the slot. A name that reads as a surface or window id (`mac-...`) but is neither a registered surface nor an open window id or title is `ok:false` with a sentence naming the open windows; any other name still shoots the front window, now with `window` and `title` in the reply.

## Deltas, labels-verb (2026-11-11)
- `labels [window:<id>]` replies `{labels: [{ax, text, need_w, need_h, box_w, box_h}], judged, marked, capped, window}`: one row per view marked `.whooshAx` whose frame anchor sits in that window (identifier, else title, as `shot <name> window:<id>`; default the key window, else the first visible one) and overlaps its content view. `marked` counts the rows, `judged` the rows with text. `box_*` is the view's frame in content points (the frame `measure` reads).
- `text` and `need_*` exist only for a LEAF text: a `Text`, or a `Text` wrapped only in modifiers on the closed list `FrameProbe.leafModifiers` (padding, frames, fixed size, opacity, layout priority, accessibility metadata, font, colour, line limit, alignment, truncation, scale factor, foreground style). Any other view (a container, a control, a list, a field, a custom view, anything holding an action or a task) answers `text: null` with `need_*` equal to `box_*`, mounts nothing, and is not counted. `text` is the `Text`'s verbatim string or localization key.
- `need_*` comes from two hidden copies of the leaf mounted behind it: one with its width free, one at the box's width with its height free. When the second is no taller than the first (by more than 0.5 pt) the text is one line: `need_w` is its ideal width, `need_h` its one-line height; otherwise it wraps: `need_w` is the box's width (the copy's, if a word makes it wider), `need_h` the wrapped height. A clipped label is `need_w > box_w + 0.5` or `need_h > box_h + 0.5` (`checks/shots-sheet.py` `CLIP_TOL_PT`).
- Its own switch: `FrameProbe.switchLabelsOn` sets `gate.on` and `gate.labels`, settles 0.15 s if either was off, and arms both 3 s linger timers. `measure` and `shot` never set `gate.labels`, so they mount exactly the readers they mounted before this verb: no copy, no Mirror walk. At most `FrameProbe.labelCopyCap` (200) leaf texts carry copies at once; past it a leaf answers `text: null` and `capped: true`.
- One row per ax id: an id shown by several views reports the last one mounted. `ok:false` "usage: labels [window:<id>]", "no open window has the id <id>", "no visible window to read labels from". An older build answers `unknown verb labels`.

## Deltas, start-fixes (2026-11-11)
- `fixture providers none|default` (fixture core only): `none` stages a core with no AI provider connected: every provider, the on-device "On this Mac" included, reads `connected: false`, the stored keys and key checks are forgotten, so the writing need of the AI setup reads `missing` with reason `provider-none` (rows `ai-setup.status`; before this the fixture core always held "On this Mac" connected and the need read `ready`). `default` puts the launch state back (only "On this Mac" connected). `fixture reset` also restores it (it builds a new agent book). Reply `{providers}`; any other word is `ok:false` "usage: fixture providers none|default". The AI setup re-reads at once. Without `--fixture-core` the reply is `ok:false` "fixture verbs need --fixture-core" (Sources/ControlSocket/FixtureVerbs.swift: `guard let f = m.host.fixture`, before any verb is read), so the owner's copy is never changed.
- `fixture forward-refuse on|off` (fixture core only): while `on`, the fixture core answers the `forward` command with `CoreError.Invalid(field: "forward", reasonWords: "The server refused to forward this message.")`, so the composer's line reads those words; before this the core refused a forward only for an unknown message. `off` and `fixture reset` (a new agent book) put it off. Reply `{forward_refuse}`; any other word is `ok:false` "usage: fixture forward-refuse on|off". Without `--fixture-core` the reply is `ok:false` "fixture verbs need --fixture-core" (`FixtureVerbs.swift:40`, `guard let f = m.host.fixture`), and only `FixtureCore` reads the flag, so the owner's copy never refuses.
- `fixture star <message id> on|off` (fixture core only): the core stars or unstars that mail in the same set the Star command writes, so `message/<id>/starred`, the Starred view, a rule's Favori fact (`message/<id>/facts/starred`) and the counts all read it; the corpus file carries no star field. `fixture reset` clears every star (`resetMessages`: `starred = []`). Reply `{message, starred}`; an unknown or deleted id is `ok:false` "no message <id>"; any other word is `ok:false` "usage: fixture star <message id> on|off". Without `--fixture-core` the reply is `ok:false` "fixture verbs need --fixture-core" (`FixtureVerbs.swift:40`, `guard let f = m.host.fixture`), so the owner's copy never stars through it.
- `label/<id>/rerun/preview` on the fixture core (it was served by the real engine only): a mirror of `rerun_preview` (core/crates/engine/src/flow_ctx.rs:259-277) over the fixture's own labels, entry chains and messages (`rerunPreviewLocked`, Sources/CoreBridge/FixtureAgents.swift, each engine line cited beside it). JSON `{"by_rule","by_ai","leave","stay","beyond","ai_words"}`: `by_rule` = messages the entry rules match that are not filed under it; `by_ai` = the newest 500 inbound inbox mails not filed and not rule-matched, when its AI card is on with a description AND the saved decision model can answer (Laya, or Jev with its key and consent), else 0 with `ai_words` the engine's line ("The AI cannot sort yet: no model on this Mac." / "...the Jev key is missing." / "...you have not allowed Jev to read your mail."), else null; `leave` = filed mails no entry rule keeps (0 with no rule and no entry chain, or while the AI part applies); `stay` = filed minus leave; `beyond` = inbound mails past the 500. No value for an unknown label or a built-in one. Not mirrored: the engine's continuation of a partial run (a fixture re-run is never partial). Left over: the fixture's re-run files by the fixture model's meaning whatever the saved decision model is, so with no model the preview says 0 by the AI while a re-run of a described label can still file.

## Deltas, gap-shots (2026-11-11)
- `scroll top|end|page-down|page-up [window:<id>]` scrolls the largest visible scroll view of that window (default: the key window, else the first visible one) that has more to show than fits, and replies `{window, command, scrollable, offset, max, content_h, view_h, at}` (points from the top; `at` top|middle|end; `scrollable: false` and nothing moved when no scroll view overflows). A page is nine tenths of the visible height. Anything else is `ok:false` "usage: scroll top|end|page-down|page-up [window:<id>]"; an unknown window id is `ok:false` "no open window has the id <id>" (Sources/ControlSocket/WindowVerbs.swift, `ScrollVerb`; Tests/Unit/ScrollVerbTests.swift).
