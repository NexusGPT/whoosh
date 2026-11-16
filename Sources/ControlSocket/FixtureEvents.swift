import Foundation
#if os(macOS)
import AppKit
#endif

/// The screen-facing `fixture` verbs: each posts one Notification the screen that owns the behaviour
/// consumes (the shape of `fixture deliver`). A verb nobody consumes changes nothing and says so:
/// the reply carries `posted`, never a claim that a screen acted.
///
///   Notification name: `whoosh.fixture.<verb>`; userInfo `verb`, `args` ([String]), `text` (args joined by a space).
///   A screen that appears AFTER the post reads `FixtureEvents.latest(verb)` (the newest args), so order does not matter.
@MainActor
enum FixtureEvents {
    /// verb -> how many arguments it takes. Served here; `reset` also changes the shell (see `reset`).
    static let served: [String: ClosedRange<Int>] = [
        "speak-file": 2...Int.max, "mic-refuse": 0...0, "cloud-no-key": 0...0, "caret": 1...1,   // dictation round 2: speak-file <path> <words...>; caret <offset>
        "reset": 0...0,
        "paste": 1...Int.max,              // text the owner pastes into the focused field (an API key, an account list)
        "paste-accounts": 1...Int.max,     // account lines, or a fixture name, pasted into Welcome
        "speech-fail": 0...Int.max,        // the speech engine fails (optional reason words)
        "provider-endpoint": 2...2,        // where a pasted provider key is checked (provider, url); settingsaimac-r3
        "speech": 1...Int.max,             // dictated text, fed without the microphone
        "speak": 1...Int.max,              // the same as `speech` under the name the dictation checks use
        "first-sync": 1...1,               // n accounts indexed so far
        "oauth": 2...Int.max,              // approve|refuse <address> [words]: the fake authority answers the waiting sign-in (contracts delta 14)
        "indexing": 1...1,                 // <n>|all: the core's sync status holds n fixture accounts indexed, the rest still loading (contracts delta 12)
        "mcp-search": 1...2,               // an app asks the MCP server to search: <app> [query]
        "mcp-server": 1...1,               // on | off
        "mcp-request": 2...Int.max,        // an allowed app asks for something that needs the owner's yes: <app> <tool> [words...]
        "upload-icon": 1...1,              // a file path, or a byte size for a generated file: the open account icon picker takes it (accepted, reason_words)
        "add-account": 1...3,              // <address> [name] [colour]
        "provider-fails": 1...Int.max,     // Ask: the AI provider fails (reason: quota, ...)
        "delete-message": 1...1,           // Ask: the cited message is deleted (id)
        "calendar-access": 1...1,          // not-asked | allowed | refused | will-refuse: the fixture's macOS Calendar access (settingscalendars-mac)
        "ai-set-aside": 2...2,             // <label> <n>: the label AI's judge set n picked mails aside (the card says it)
        "agent-allowed": 1...1,            // Ask: the agent's allowed actions, comma separated
        "decide": 4...4,                   // hid-ai: <page> <sample message> <label> <confidence>: the page's list decides one message (AISettingsStore, FixtureDecide.swift)
        "add-rule": 3...3,                 // hid-ai: <page> <rule words> <outcome>: a rule goes first in the page's list
        "consent": 2...2,                  // hid-ai: jev yes|no: the owner's consent to send mail text to Jev, as Allow and Withdraw give it
        "corrections": 3...3,              // hid-ai F5: <page> <n> <shared sender or domain|mixed>: the owner's corrections on a page since a day
        "examples": 1...1,                 // hid-ai F5: <n>: the examples the on-device model may be refitted on
        "textjudge": 2...2,                // hid-ai round 10: <cases.tsv> <out.json>: a test copy asks its text backends to judge labels (TextJudgeFixture.swift)
        "mail-picture": 1...1,             // lead, 13 Nov: <address on this Mac>: a mail with one blocked picture kept at that address and one pixel beside it, in the shown body's place
        "chatgpt": 2...2,                  // hid-ai round 9 (348b2ea0): approve|refuse <loopback base>: a test copy signs in with ChatGPT against a stand-in
    ]

    /// Set by the open account icon picker (AccountIconPicker.swift): takes `<bytes|path>` as the file panel's answer. Nil = no picker is open.
    static var uploadIcon: ((String) -> (accepted: Bool, words: String))?

    static func name(_ verb: String) -> Notification.Name { Notification.Name("whoosh.fixture.\(verb)") }

    private static var newest: [String: [String]] = [:]
    /// The newest arguments a verb was posted with since the last `reset`, or nil.
    static func latest(_ verb: String) -> [String]? { newest[verb] }

    static func run(_ m: AppModel, verb: String, args: [String]) throws -> [String: Any] {
        guard let arity = served[verb] else { throw ControlFailure("fixture \(verb) is not served") }
        guard arity.contains(args.count) else { throw ControlFailure("fixture \(verb) takes \(describe(arity)) argument(s), got \(args.count)") }
        // hid-ai round 11: the AI settings' fixture observers (decide, add-rule, consent, corrections, examples, chatgpt, ...) were installed only when
        // a click first built that feature's doors (SettingsAIMacFeature.clickTargets): a copy driven by fixture verbs alone posted to no one, and the
        // verb still answered "posted" (main101 textjudge-sanity: nothing ever ran). Installed here, once, before any post.
        AIFixture.listen(AISettingsStore.shared)
        switch verb {
        case "mcp-server": guard ["on", "off"].contains(args[0]) else { throw ControlFailure("usage: fixture mcp-server on|off") }
        case "first-sync": guard let n = Int(args[0]), n >= 0 else { throw ControlFailure("usage: fixture first-sync <n>, n a whole number from 0") }
        default: break
        }
        if verb == "mail-picture" {
            guard MessageBodyFeature.pictureSample(args[0], m) else { throw ControlFailure("fixture mail-picture needs the fixture core, an open mail and an address on this Mac") }
        }
        if verb == "indexing" {
            guard let core = CoreHost.shared.fixture else { throw ControlFailure("fixture indexing needs the fixture core") }
            let n: Int? = args[0] == "all" ? nil : Int(args[0])
            guard args[0] == "all" || n != nil else { throw ControlFailure("usage: fixture indexing <n>|all, n a whole number from 0") }
            do { try core.setIndexing(accounts: n) } catch { throw ControlFailure("\(error)") }
        }
        if verb == "oauth" {
            guard let core = CoreHost.shared.fixture else { throw ControlFailure("fixture oauth needs the fixture core") }
            guard ["approve", "refuse"].contains(args[0]) else { throw ControlFailure("usage: fixture oauth approve|refuse <address> [words]") }
            do { try core.answerOAuth(address: args[1], approve: args[0] == "approve", words: args.dropFirst(2).joined(separator: " ")) }
            catch CoreError.Invalid(_, let words) { throw ControlFailure(words) }
        }
        if verb == "ai-set-aside" {
            guard let core = CoreHost.shared.fixture, let n = Int(args[1]), n >= 0 else { throw ControlFailure("usage: fixture ai-set-aside <label> <n>, n a whole number from 0") }
            core.armSetAside(label: args[0], count: n)
        }
        // `add-account <address> [name] [colour]`: the account is made in the core (as a Welcome row or the sign-in route would), then named and coloured
        // as given; the screens read it from the core (the Settings pane's thirty-first account, settingsaccountsmac-check)
        if verb == "add-account" {
            guard let core = CoreHost.shared.fixture else { throw ControlFailure("fixture add-account needs the fixture core") }
            do {
                let made = try core.run(command: .addAccount(address: args[0], provider: .imap))
                if let id = made.createdId {
                    if args.count > 1 { _ = try core.run(command: .renameAccount(accountId: id, name: args[1])) }
                    if args.count > 2 { _ = try core.run(command: .setAccountColour(accountId: id, colour: args[2])) }
                }
            } catch CoreError.Invalid(_, let words) { throw ControlFailure(words) }
            catch { throw ControlFailure("\(error)") }
            m.refresh()
        }
        // contracts delta 13: the two MCP verbs act on the core, not only on the screens
        var extra: [String: Any] = [:]
        if verb == "mcp-server", let core = CoreHost.shared.fixture {
            do { _ = try core.run(command: .setMcpServer(on: args[0] == "on")) } catch { throw ControlFailure("\(error)") }
        }
        if verb == "mcp-search" {
            guard let core = CoreHost.shared.fixture else { throw ControlFailure("fixture mcp-search needs the fixture core") }
            // an app with no grant (or a server that is off) gets nothing: the verb answers what the core refused, ok:true
            do { extra["request_id"] = try core.mcpSearch(app: args[0], query: args.count > 1 ? args[1] : "") }
            catch CoreError.Invalid(_, let words) { extra["refused"] = words }
        }
        if verb == "mcp-request" {
            guard let core = CoreHost.shared.fixture else { throw ControlFailure("fixture mcp-request needs the fixture core") }
            let words = args.count > 2 ? args[2...].joined(separator: " ") : args[1]
            do { _ = try core.requestMcpTool(app: args[0], tool: args[1], words: words) } catch { throw ControlFailure("\(error)") }
        }
        if verb == "upload-icon" {
            // fixture-r3: the open picker takes the bytes as the file panel's answer; with no picker open the reply carries no `accepted` (not judged)
            if let take = uploadIcon { let r = take(args[0]); extra["accepted"] = r.accepted; extra["reason_words"] = r.words; m.refresh() }
            else { extra["refused"] = "no account icon picker is open" }
        }
        if verb == "textjudge" {      // hid-ai round 11: started here and answered here: a refusal is the verb's error, never a silent post
            if let words = TextJudgeRun.start(cases: args[0], out: args[1]) { throw ControlFailure(words) }
            extra["started"] = true
        }
        if verb == "reset" { reset(m) }
        // J5.5: the message is deleted in the test engine too (to Trash, as the owner's Delete), so a search result still listing it finds it gone when opened
        if verb == "delete-message", let core = CoreHost.shared.fixture {
            do { _ = try core.run(command: .deleteMessages(messageIds: [args[0]])); extra["deleted"] = true }
            catch { extra["deleted"] = false; extra["delete_refused"] = "\(error)" }   // an id the core does not hold: the notification still goes out (Ask's use)
        }
        newest[verb] = args
        NotificationCenter.default.post(name: name(verb), object: nil,
                                        userInfo: ["verb": verb, "args": args, "text": args.joined(separator: " ")])
        let reply: [String: Any] = ["posted": verb, "args": args]
        return reply.merging(extra) { a, _ in a }
    }

    /// `fixture reset`: typed text cleared, every other window and the overlay closed, the main window shown on
    /// mac-main, armed faults and delivered arrivals forgotten. The screens clear their own state on the notification.
    private static func reset(_ m: AppModel) {
        m.typed = ""
        m.overlay = nil
        m.surface = SurfaceSlot.macMain.id
        m.lastFailure = nil
        FixtureFaults.clear()
        // probes-r1: the fixture core returns to its launch state (mail, labels, drafts, outbox, settings, accounts, the
        // served list after `fixture corpus`), then the view state the checks read
        CoreHost.shared.fixture?.restoreSeed()
        QuickReplyModel.shared.resetToLaunch()   // an open reply holds the keyboard as a state: it must not outlive the reset
        viewState(m, reset: true)
        DefaultSignInClients.fixture = nil
        FixtureVerbs.delivered = []
        newest = [:]
        #if os(macOS)
        // The composer's window is closed below, and a closing composer saves what it holds in a task that runs AFTER this turn: the draft came back
        // into the core the reset had just restored (main188, par-macshell J0.20: digest parts draftRows, draftSavedAt, nextId differ after the reset,
        // six toolbar cases not judged). Its draft is let go first, so that save writes nothing.
        ComposerModel.shared.resetToLaunch()
        let main = WindowBridge.mainWindow()
        for w in NSApp.windows where w.isVisible && w !== main && w.styleMask.contains(.titled) { w.close() }
        WindowBridge.bringMainToFront()
        #endif
        m.refresh()
    }

    /// The view state a check reads: tab, account, mailbox, smart folder, label, focus, overlay, surface, typed text, the
    /// list's selection and filters. `reset` puts each back to its launch value; the text is what `fixture digest` returns.
    /// Not reachable from here (a feature's own model, cleared by the feature): the open thread's reader state and the
    /// quick reply's field, the composer's draft. They follow `overlay`/`surface` and the screens' own reset observers.
    @discardableResult
    static func viewState(_ m: AppModel, reset: Bool = false) -> String {
        let list = InboxListState.shared
        if reset {
            m.tab = "primary"; m.accountFilter = nil; m.mailbox = nil; m.smartFolderId = nil; m.labelId = nil
            m.focusIndex = 0; m.settingsPage = SurfaceSlot.macSettingsGeneral
            list.clearSelection(); list.filter = nil; list.labelFilter = nil; list.labelReturnTab = nil
        }
        let parts: [String] = ["surface=\(m.surface)", "tab=\(m.tab)", "account=\(m.accountFilter ?? "-")", "mailbox=\(m.mailbox.map { "\($0)" } ?? "-")",
                               "smart=\(m.smartFolderId ?? "-")", "label=\(m.labelId ?? "-")", "focus=\(m.focusIndex)", "overlay=\(m.overlay ?? "-")",
                               "typed=\(m.typed)", "settings=\(m.settingsPage.id)", "selection=\(list.selection.joined(separator: "+"))",
                               "filter=\(list.filter?.rawValue ?? "-")", "labelFilter=\(list.labelFilter ?? "-")", "faults=\(FixtureFaults.pending.count)",
                               "delivered=\(FixtureVerbs.delivered.count)"]
        return parts.joined(separator: ";")
    }

    private static func describe(_ r: ClosedRange<Int>) -> String {
        r.upperBound == Int.max ? "\(r.lowerBound) or more" : r.count == 1 ? "\(r.lowerBound)" : "\(r.lowerBound) to \(r.upperBound)"
    }
}
