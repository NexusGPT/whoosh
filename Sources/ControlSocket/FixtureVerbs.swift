import Foundation

/// The `fixture` verb family, under `--fixture-core` only (docs/CONTROL-SOCKET.md "## Deltas").
///   fixture reauth <accountId>                       the failing account signs in again
///   fixture deliver <arrival> <fixture-file> [source] an arrival from the file's `arrivals` table
///   fixture device-link scan                         the paired iPhone's first request reaches this copy: the Move-accounts token is used (J4.10)
///   fixture fail-next <op> <reason words...>          the next run of <op> fails once with the reason
///   fixture default-client set|clear                 SPEC A88: Whoosh's default Microsoft client ID present (a stand-in) or absent
///   fixture refuse <cause>                            the next sign-in check fails with that cause (SPEC A87: basic_auth_disabled,
///                                                     bad_password, host_unreachable, tls_failed, smtp_refused, unknown;
///                                                     or an engine code: engine.not_built_yet, oauth.no_client_id, oauth.sign_in_not_done,
///                                                     oauth.refused, oauth.no_refresh_token, oauth.browser)
///   fixture corpus <name|path>                       the fixture core serves that file's mail
///   fixture load full-history-<n>                    n accounts, FIXTURE_FULL_HISTORY_PER_ACCOUNT messages each (contracts delta 13)
///   fixture theme light|dark|system                  the same as the verb `theme`
///   fixture mcp-request <app> <tool> [words...]       an outside app asks for a tool: a pending request (FixtureCore.requestMcpTool)
///   fixture format bold|list|link|picture <args...> | reload-draft | paste-html <html>   the composer's rich text draft (RichTextFixture)
///   fixture reply-arrived <id>                       the person answers the thread of a message, draft or outbox id: a new unread message lands in it
///   fixture speech-model missing|ready | sync broken|ok | notification-tap <message id>   the iPhone screens' verbs (docs/lanes/gap-phone-wire.md)
///   fixture offline on|off                           a send that is due waits for the network while on; `server-refuse <reason...>|off` fails it with the reason
///   fixture search-hold on|off                       the test engine holds the second answer of a search until off (the first: exact matches, then the rest by date, stage words)
///   fixture index-state building|ready               J5.4: the meaning index still building (search-hold on) or ready (off: the held ranking joins)
///   fixture index-progress <indexed> <queued>        the meaning index's progress as the builder announces it (`<n> 0` ends the line)
///   fixture seed-corpus <N>                          exactly N generated mails in the fixture core (perf/search.py); `reset` brings the sample back
///   fixture search-refuse <words...>|off             every search is refused with these words until off
///   fixture forward-refuse on|off                    the forward command is refused in words while on; `reset` puts it off
///   fixture star <message id> on|off                 the fixture core stars or unstars a mail; `reset` clears every star
///   fixture providers none|default                   no AI provider connected (not even "On this Mac"), or the launch state again; `reset` restores it
///   fixture accounts none|all                        the core lists no account (the sidebar's empty state), or every account again
///   fixture reply-folder-gone <account id> <folder name> [back]   a mail folder removed on the server (delta 24), or back; `reset` restores it
///   fixture digest                                   one value over the core's whole state, one per part, and the view state (FixtureReset.swift)
///   fixture reset | paste | paste-accounts | speech-fail | speech | speak | first-sync | mcp-search | mcp-server
///           | upload-icon | add-account | provider-fails | delete-message | agent-allowed: FixtureEvents.swift
///   fixture first-start on|off | reduce on|off      the first-start flow (J9) opens at step 1 (flag cleared) or closes; `reduce` is its Reduced Motion switch
///   fixture front-app <name> | pointer-rest <ax-id> | drop-file <ax-id> <file> | mute-account <id> | notch-geometry none|notch
///                                                    the notch's rig inputs: a notification each, read by Features/Notch
///   fixture pointer-rest mac-main.list.row.<n> | pointer-rest off   the same rest on a list row (its hover fill and action strip), or off the list
///                                                    (Features/InboxList/ListPointerRest; reply `{rested_on, list_row}`, a place with no row is refused)
@MainActor
enum FixtureVerbs {
    /// Posted on `fixture deliver`; `userInfo` is the arrival's fields plus `name` and `source`.
    static let arrival = Notification.Name("whoosh.fixture.arrival")
    /// The notch's rig inputs (lane notch, N1), each with its payload in userInfo.
    static let frontApp = Notification.Name("whoosh.fixture.frontApp")           // name
    static let pointerRest = Notification.Name("whoosh.fixture.pointerRest")     // target (an ax-id)
    static let dropFile = Notification.Name("whoosh.fixture.dropFile")           // target, file
    static let muteAccount = Notification.Name("whoosh.fixture.muteAccount")     // account
    static let notchGeometry = Notification.Name("whoosh.fixture.notchGeometry") // shape: "none" | "notch"
    /// What `fixture deliver` delivered, in order.
    static var delivered: [ControlRow] = []
    /// `fixture sync broken|ok` (gap-phone-wire): the state a core that serves `settings_sync/error` reads. The fixture core keeps its own copy (`fixtureSyncBroken`), which `settings_sync/error` reads; this mirror is what a test reads back.
    static var syncBroken = false

    static func run(_ m: AppModel, _ a: [String]) throws -> [String: Any] {
        guard let f = m.host.fixture else { throw ControlFailure("fixture verbs need --fixture-core") }
        NotchEvents.start()   // the notch listens for the rig inputs below from the first fixture verb on, not from its first click
        switch (a.first, a.count) {
        case ("reauth", 2):
            guard f.reauth(accountId: a[1]) else { throw ControlFailure("\(a[1]) is not the failing account") }
            m.refresh()
            return [:]
        case ("device-link", 2) where a[1] == "scan":
            f.simulateDeviceLinkScan()   // the phone's scan of this copy's Move-accounts code: the token is used and `device_link/token` reads nil
            return [:]
        case ("deliver", 3...4):
            let reply = try deliver(name: a[1], file: a[2], source: a.count == 4 ? a[3] : "fixture", core: f)
            m.refresh()
            return reply
        case ("deliver", _):
            throw ControlFailure("usage: fixture deliver <arrival> <fixture-file> [source]")
        case ("reply", 2):
            do {
                let r = try f.fixtureReply(messageId: a[1])
                m.refresh()
                return ["message": a[1], "replied_by_me": r.repliedByMe, "unanswered": r.unanswered, "ended_in": r.endedIn ?? NSNull()]
            } catch CoreError.NotFound(_, let what) { throw ControlFailure("no message \(what)") }
        case ("reply-arrived", 2):                                      // fixture-r3: someone answers the thread of a message, draft or outbox id
            do {
                let r = try f.fixtureReplyLands(id: a[1])
                m.refresh()
                return ["thread_of": a[1], "message": r.messageId, "reminder": r.state]
            } catch CoreError.NotFound(_, let what) { throw ControlFailure("no thread \(what)") }
        case ("reply-arrived", _):
            throw ControlFailure("usage: fixture reply-arrived <message, draft or outbox id>")
        case ("accounts", 2):                                          // lane brandmark-r2: the core lists no account (the sidebar's empty state)
            guard ["none", "all"].contains(a[1]) else { throw ControlFailure("usage: fixture accounts none|all") }
            f.fixtureAccounts(none: a[1] == "none")
            m.refresh()
            SidebarState.shared.accountsChanged()      // refresh() keeps equal rows/counts, so the sidebar's empty branch needs its own redraw
            return ["accounts": a[1]]
        case ("format", _), ("reload-draft", _), ("paste-html", _):     // lane richtext-r2: the rich text check's door (Features/RichText/RichTextFixture.swift)
            do { return try RichTextFixture.run(a[0], Array(a.dropFirst())) }
            catch let r as RichTextFixture.Refusal { throw ControlFailure(r.words) }
        case ("reply", _):
            throw ControlFailure("usage: fixture reply <messageId>")
        case ("second-draft", 2...):                                    // lead, 13 Nov (owner picture 206): one MORE saved reply draft on the conversation on screen,
            // the state of a mailbox the builds before 13 Nov left (a new draft at every R). The reply itself can no longer make one: R continues the saved draft.
            guard let t = ThreadCards.shownThread(m) else { throw ControlFailure("fixture second-draft: no conversation on screen") }
            let words = a.dropFirst().joined(separator: " ")
            guard let id = try? CoreDraftSource().save(threadId: t, accountId: nil, body: words, core: m.core) else { throw ControlFailure("fixture second-draft: the core saved nothing") }
            QuickReplyModel.shared.draftsRevision += 1
            m.refresh()
            return ["draft": id, "thread": t]
        case ("second-draft", _):
            throw ControlFailure("usage: fixture second-draft <words...>")
        case ("import-sources", _):                                     // delta 18 (welcomemac-r4): what the next scan finds on this Mac
            do { try f.fixtureImportSources(Array(a.dropFirst())) } catch CoreError.Invalid(_, let words) { throw ControlFailure(words) }
            m.refresh()
            return ["sources": a.count - 1]
        case ("import-progress", 3):                                    // one step of a scripted run: done | finish | refuse | fail
            do {
                let word = try f.fixtureImportProgress(runId: a[1], step: a[2])
                m.refresh()
                return ["run": a[1], "state": word]
            } catch CoreError.NotFound(_, let what) { throw ControlFailure("no import run \(what)") }
              catch CoreError.Invalid(_, let words) { throw ControlFailure(words) }
        case ("import-progress", _):
            throw ControlFailure("usage: fixture import-progress <run> <done|finish|refuse|fail>")
        case ("first-start", 2...3):                                    // firststart lane (J9): open or close the first-start flow on demand
            switch (a[1], a.count) {
            case ("on", 2), ("off", 2): FirstStartModel.shared.rig(on: a[1] == "on")
            case ("reduce", 3) where ["on", "off"].contains(a[2]): FirstStartModel.shared.rigReduceMotion = a[2] == "on"
            default: throw ControlFailure("usage: fixture first-start on|off, or fixture first-start reduce on|off")
            }
            return ["first_start": a.dropFirst().joined(separator: " ")]
        case ("front-app", 2...):
            let name = a[1...].joined(separator: " ")
            NotificationCenter.default.post(name: frontApp, object: nil, userInfo: ["name": name])
            return ["front_app": name]
        case ("pointer-rest", 2):
            let row: String?
            do { row = try ListPointerRest.rest(m, on: a[1]) }
            catch { throw ControlFailure("no row at \(a[1]): the list holds \(m.rows.count) rows") }
            NotificationCenter.default.post(name: pointerRest, object: nil, userInfo: ["target": a[1]])
            return ["rested_on": a[1], "list_row": row ?? NSNull()]
        case ("drop-file", 3):
            NotificationCenter.default.post(name: dropFile, object: nil, userInfo: ["target": a[1], "file": a[2]])
            return ["target": a[1], "file": a[2]]
        case ("filing-refuse", 2...):   // triage-keys: the filing AI refuses this mail, in these words (the screen's Why here says them)
            do { try f.fixtureFilingRefuse(messageId: a[1], words: a.count > 2 ? a[2...].joined(separator: " ") : "The writing model is not installed on this Mac.") }
            catch CoreError.NotFound(_, let what) { throw ControlFailure("no message \(what)") }
            m.refresh()
            return ["message": a[1], "ai": "refused"]
        case ("mute-account", 2):
            NotificationCenter.default.post(name: muteAccount, object: nil, userInfo: ["account": a[1]])
            return ["muted": a[1]]
        case ("notch-geometry", 2):
            guard ["none", "notch"].contains(a[1]) else { throw ControlFailure("notch-geometry takes none or notch") }
            NotificationCenter.default.post(name: notchGeometry, object: nil, userInfo: ["shape": a[1]])
            return ["shape": a[1]]
        case ("system-ui", 2):   // the real framework call for one kind of system UI (the hidden proof's drive)
            return try FixtureSystemUI.run(a[1])
        case ("mcp-request", 3...):   // an outside app asks the MCP server to run a tool: a pending request for Settings > Outside apps
            let words = a.count > 3 ? a[3...].joined(separator: " ") : Words.text("%@ asks to use %@", a[1], a[2])
            do { return ["request_id": try f.requestMcpTool(app: a[1], tool: a[2], words: words), "app": a[1], "tool": a[2]] }
            catch CoreError.Invalid(_, let reason) { throw ControlFailure(reason) }
        case ("mcp-request", _):
            throw ControlFailure("usage: fixture mcp-request <app> <tool> [words...]")
        case ("default-client", 2):
            switch a[1] {
            case "set": DefaultSignInClients.fixture = .set
            case "clear": DefaultSignInClients.fixture = .cleared
            default: throw ControlFailure("usage: fixture default-client set|clear")
            }
            return ["default_client": a[1]]
        case ("refuse", 2):
            guard f.armRefusal(a[1]) else { throw ControlFailure("no refusal cause \(a[1]): basic_auth_disabled, bad_password, host_unreachable, tls_failed, smtp_refused, unknown, or an engine code (engine.*, oauth.*)") }
            return ["armed": a[1]]
        case ("fail-next", 3...):
            let reason = a[2...].joined(separator: " ")
            FixtureFaults.arm(a[1], reason: reason)
            return ["armed": a[1], "reason": reason]
        case ("corpus", 2):
            guard let url = FixtureCorpus.locate(a[1]) else { throw ControlFailure("no corpus \(a[1]): give a path, or a name in $WHOOSH_FIXTURES_DIR") }
            guard let loader = f as AnyObject as? CorpusLoading else {
                throw ControlFailure("the fixture core cannot load a corpus yet (FixtureCore: CorpusLoading, contracts lane)")
            }
            do { try loader.loadCorpus(from: url) }
            catch CoreError.Invalid(_, let words) { throw ControlFailure(words) }
            m.focusIndex = 0
            m.refresh()
            return ["corpus": url.path, "rows": m.rows.count]
        case ("load", 2):   // contracts delta 13: `fixture load full-history-<n>`, the memory budget's fixture (perf/memory-mac.py)
            guard a[1].hasPrefix("full-history-"), let n = Int(a[1].dropFirst("full-history-".count)) else {
                throw ControlFailure("usage: fixture load full-history-<n>, n the number of accounts")
            }
            do { try f.loadFullHistory(accounts: n) } catch CoreError.Invalid(_, let words) { throw ControlFailure(words) }
            m.focusIndex = 0
            m.refresh()
            return ["loaded": a[1], "accounts": n, "messages": n * WhooshConstants.FIXTURE_FULL_HISTORY_PER_ACCOUNT]
        case ("reply-folder-gone", 3...4):   // delta 24 (replies-contract): a mail folder removed on the server, or `back`; `fixture reset` restores it
            guard a.count == 3 || a[3] == "back" else { throw ControlFailure("usage: fixture reply-folder-gone <account id> <folder name> [back]") }
            do { try f.fixtureReplyFolderGone(account: a[1], folder: a[2], gone: a.count == 3) }
            catch CoreError.NotFound(_, let what) { throw ControlFailure("no mail folder \(a[2]) on \(a[1]) in the Sources pane (\(what))") }
            catch CoreError.Invalid(_, let words) { throw ControlFailure(words) }
            m.refresh()
            return ["account": a[1], "folder": a[2], "gone": a.count == 3]
        case ("install-state", 2):   // probes-r1 R7: the fixture core serves a new installation (FixtureFirstRun.swift); `fixture reset` brings the sample back
            guard a[1] == "new" else { throw ControlFailure("usage: fixture install-state new") }
            f.installNew()
            m.focusIndex = 0
            m.refresh()
            return ["install_state": "new"]
        case ("providers", 2):   // start-fixes: stage "no provider connected" (the fixture's "On this Mac" provider is otherwise always on); `fixture reset` brings it back
            guard ["none", "default"].contains(a[1]) else { throw ControlFailure("usage: fixture providers none|default") }
            f.fixtureProviders(none: a[1] == "none")
            AISetupStatus.shared.refresh()          // the AI setup reads the providers again now, not when the app next comes to the front
            return ["providers": a[1]]
        case ("offline", 2):   // a send that is due waits as `waiting_for_network` while on, and goes when off (J3.5, J7); `fixture reset` must put it off
            guard ["on", "off"].contains(a[1]) else { throw ControlFailure("usage: fixture offline on|off") }
            f.fixtureOutbox(offline: a[1] == "on")
            m.refresh()
            return ["offline": a[1]]
        case ("server-refuse", 2...):   // a send that is due fails with the reason (the Outbox keeps the draft with a Retry) until `off` (J3.7)
            let reason = a[1...].joined(separator: " ")
            f.fixtureServerRefuse(reason == "off" ? nil : reason)
            m.refresh()
            return ["server_refuse": reason]
        case ("index-progress", 3):   // item 98a2ddec, 8c3a0430: the meaning index's progress as the builder announces it (the search's index line, Settings, the status line); a real announcement replaces it
            guard let indexed = Int(a[1]), let queued = Int(a[2]), indexed >= 0, queued >= 0 else { throw ControlFailure("usage: fixture index-progress <indexed> <queued>") }
            var s = MeaningIndexBuilder.Status()
            s.indexed = indexed; s.queued = queued
            IndexProgressModel.shared.take(s)   // the figures are on screen when the reply is sent (the notification below reaches the model on the main queue, after)
            NotificationCenter.default.post(name: IndexProgress.didChange, object: nil, userInfo: [IndexProgress.statusKey: s])
            return ["index_progress": [indexed, queued]]
        case ("index-progress", _):
            throw ControlFailure("usage: fixture index-progress <indexed> <queued>")
        case ("seed-corpus", 2):      // item 8149bdd7 piece C: exactly N generated mails in the fixture core (perf/search.py); `fixture reset` brings the sample back
            guard let n = Int(a[1]) else { throw ControlFailure("usage: fixture seed-corpus <N>") }
            do { try f.seedCorpus(count: n) } catch CoreError.Invalid(_, let words) { throw ControlFailure(words) }
            m.focusIndex = 0
            m.refresh()
            return ["seeded": n, "messages": n]
        case ("seed-corpus", _):
            throw ControlFailure("usage: fixture seed-corpus <N>")
        case ("search-hold", 2):      // the test engine answers a search in two steps and holds the second until `off` (the ranking-in-progress line, the swap)
            guard ["on", "off"].contains(a[1]) else { throw ControlFailure("usage: fixture search-hold on|off") }
            f.fixtureSearchHold(a[1] == "on")
            return ["search_hold": a[1]]
        case ("search-hold", _):
            throw ControlFailure("usage: fixture search-hold on|off")
        case ("index-state", 2):      // J5.4: the meaning index still building (a search answers its word matches first, the ranking line says the rest is coming) or ready (the held ranking joins)
            guard ["building", "ready"].contains(a[1]) else { throw ControlFailure("usage: fixture index-state building|ready") }
            f.fixtureSearchHold(a[1] == "building")
            return ["index_state": a[1]]
        case ("index-state", _):
            throw ControlFailure("usage: fixture index-state building|ready")
        case ("search-refuse", 2...):  // every search is refused with these words until `off` (the could-not-run note); `reset` puts it off
            let words = a[1...].joined(separator: " ")
            f.fixtureSearchRefuse(words == "off" ? nil : words)
            return ["search_refuse": words]
        case ("search-refuse", _):
            throw ControlFailure("usage: fixture search-refuse <words...>|off")
        case ("forward-refuse", 2):   // the fixture core refuses the forward command with words (the composer's line says them); `fixture reset` puts it off
            guard ["on", "off"].contains(a[1]) else { throw ControlFailure("usage: fixture forward-refuse on|off") }
            f.fixtureForwardRefuse(a[1] == "on")
            return ["forward_refuse": a[1]]
        case ("forward-refuse", _):
            throw ControlFailure("usage: fixture forward-refuse on|off")
        case ("star", 3):   // the fixture core stars a mail (no corpus file carries a star); `fixture reset` clears every star
            guard ["on", "off"].contains(a[2]) else { throw ControlFailure("usage: fixture star <message id> on|off") }
            do { try f.fixtureStar(a[1], on: a[2] == "on") } catch CoreError.NotFound(_, let what) { throw ControlFailure("no message \(what)") }
            m.refresh()
            return ["message": a[1], "starred": a[2] == "on"]
        case ("star", _):
            throw ControlFailure("usage: fixture star <message id> on|off")
        case ("providers", _):
            throw ControlFailure("usage: fixture providers none|default")
        case ("digest", 1):   // probes-r1: one value over the fixture core's whole state (`digest`), one per stored property (`parts`), and the view state
            let d = f.stateDigest()
            return ["digest": d.total, "parts": d.parts, "view": FixtureEvents.viewState(m)]
        case ("theme", 2):   // the journeys' spelling of `theme <name>` (J1.py:87)
            return ["theme": try WindowControl.setTheme(a[1])]
        case ("power", 2) where a[1] == "real":                          // bg-hooks: read the real battery and heat again
            BackgroundWork.simulatePower(nil); return ["source": "real"]
        case ("power", 4):                                              // bg-hooks: a scripted reading, as the engine is told it
            guard let pct = Int(a[1]), (0...100).contains(pct), ["charging", "battery"].contains(a[2]),
                  ["nominal", "fair", "serious", "critical"].contains(a[3])
            else { throw ControlFailure("usage: fixture power <0-100> <charging|battery> <nominal|fair|serious|critical> | fixture power real") }
            BackgroundWork.simulatePower(PowerConditions(batteryPercent: pct, charging: a[2] == "charging", thermalSerious: a[3] == "serious" || a[3] == "critical"))
            return ["percent": pct]
        case ("embedder", 2):                                           // bg-hooks: the model is loaded or unloaded, as the engine sees it
            guard ["loaded", "unloaded"].contains(a[1]) else { throw ControlFailure("usage: fixture embedder loaded|unloaded") }
            BackgroundWork.simulateEmbedder(loaded: a[1] == "loaded"); return ["embedder": a[1]]
        case ("power", _):
            throw ControlFailure("usage: fixture power <0-100> <charging|battery> <nominal|fair|serious|critical> | fixture power real")
        case ("embedder", _):
            throw ControlFailure("usage: fixture embedder loaded|unloaded")
        case ("provider-endpoint", 3) where providerEndpointRefused(a[2]):   // rig-net-swift: a test copy names only an endpoint on this Mac
            throw ControlFailure("provider-endpoint on a test copy takes only an address on this Mac (127.0.0.1, ::1 or localhost); \(a[2]) is not")
        // gap-phone-wire: the verbs the iPhone screens' notes ask for (docs/lanes/gap-phone-wire.md), served here so no other file changes.
        case ("speech-model", 2):   // composerphone: `missing|ready` puts the dictation model where the check needs it (PhoneSpeech listens for the notification)
            guard ["missing", "ready"].contains(a[1]) else { throw ControlFailure("usage: fixture speech-model missing|ready") }
            NotificationCenter.default.post(name: FixtureEvents.name("speech-model"), object: nil, userInfo: ["verb": "speech-model", "args": [a[1]], "text": a[1]])
            return ["posted": "speech-model", "args": [a[1]]]
        case ("sync", 2):           // settingsphone: `broken|ok` the sync line's failure. The core's `settings_sync/error` reads `syncBroken` once FixtureCore serves it (the note's finding 1)
            guard ["broken", "ok"].contains(a[1]) else { throw ControlFailure("usage: fixture sync broken|ok") }
            syncBroken = a[1] == "broken"
            f.fixtureSyncBroken(syncBroken)   // the fixture core's `settings_sync/error` answers its reason while broken
            NotificationCenter.default.post(name: FixtureEvents.name("sync"), object: nil, userInfo: ["verb": "sync", "args": [a[1]], "text": a[1]])
            m.refresh()
            return ["posted": "sync", "args": [a[1]], "broken": syncBroken]
        case ("notification-tap", 2):   // a notification's tap on a mail: the thread opens over the inbox (PhoneRouter.notificationTapped)
            guard PhoneRouter.shared.notificationTapped(messageId: a[1]) else { throw ControlFailure("fixture notification-tap: \(a[1]) is not a mail the inbox can show") }
            return ["opened": a[1], "overlay": m.overlay ?? ""]
        case (let verb?, _) where FixtureEvents.served[verb] != nil:   // round 7: the screen-facing verbs
            return try FixtureEvents.run(m, verb: verb, args: Array(a.dropFirst()))
        case ("fail-next", _):
            throw ControlFailure("usage: fixture fail-next <op> <reason>")
        default:
            throw ControlFailure("fixture \(a.first ?? "") is not served; served: reauth, deliver, fail-next, theme, corpus, load, mcp-request, front-app, pointer-rest, drop-file, mute-account, notch-geometry, format, reload-draft, paste-html, power, embedder, \(FixtureEvents.served.keys.sorted().joined(separator: ", "))")
        }
    }

    /// `fixture provider-endpoint <provider> <url>`: true when a test copy must refuse the url (its host is not this Mac, `RigNet`).
    /// An empty url (clear the endpoint) is never refused; the owner's copy refuses nothing. `env` nil is this process.
    nonisolated static func providerEndpointRefused(_ url: String, env: [String: String]? = nil) -> Bool {
        guard !url.isEmpty else { return false }
        return !RigNet.allows(URL(string: url), env: env)
    }

    private static func deliver(name: String, file: String, source: String, core: FixtureCore) throws -> [String: Any] {
        guard let data = FileManager.default.contents(atPath: file) else { throw ControlFailure("no fixture file at \(file)") }
        guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw ControlFailure("the fixture file \(file) is not a JSON object")
        }
        let table = obj["arrivals"] as? [String: Any] ?? [:]
        guard var fields = table[name] as? [String: Any] else {
            throw ControlFailure("no arrival \(name) in \(file); arrivals: \(table.keys.sorted().joined(separator: ", "))")
        }
        fields["name"] = name
        fields["source"] = source
        // contracts delta 13: the arrival becomes an inbox row in the fixture core, filed by the label rules, then by the
        // fixture model's meaning, as it arrives; the event still goes to the screens
        guard let id = fields["id"] as? String, let acc = fields["account"] as? String else {
            throw ControlFailure("arrival \(name) needs an id and an account")
        }
        let labels: [String]
        do {
            labels = try core.deliverArrival(id: id, account: acc, from: fields["from"] as? String ?? "", sender: fields["sender"] as? String ?? "",
                                             subject: fields["subject"] as? String ?? "", body: (fields["body"] ?? fields["first_lines"]) as? String ?? "", code: fields["code"] as? String)
        } catch CoreError.Invalid(_, let words) { throw ControlFailure(words) }
        catch CoreError.NotFound(_, let what) { throw ControlFailure("arrival \(name): no account \(what)") }
        delivered.append(fields)
        NotificationCenter.default.post(name: arrival, object: nil, userInfo: fields)
        return ["delivered": name, "id": id, "in_inbox": true, "labels": labels]
    }
}
