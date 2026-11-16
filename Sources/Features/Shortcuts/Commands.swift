import Foundation

/// One thing the owner can do from the keyboard, the palette or the mouse. `title` is the plain word the
/// preset rows, the palette and the trainer show.
///  - live: the engine or a screen runs it today.
///  - stub: the feature exists but another lane owns the handler (`wiredBy`); until it lands the key passes on.
///  - soon: the feature is not built yet (`comingSoon` names it); a press shows a one-line notice and passes on.
public struct ShortcutCommand: Sendable, Equatable {
    public let id: String
    public let title: String
    public let wiredBy: String?
    public let comingSoon: String?

    public var isLive: Bool { wiredBy == nil && comingSoon == nil }
    /// The palette lists what can run now: no stub, no coming-soon command, no digit variant.
    public var inPalette: Bool { isLive && id != "enter" && !id.hasPrefix("switch-account-") }
    /// The one line a press of a coming-soon key shows.
    public var notice: String? { comingSoon.map { "\($0) is coming soon." } }
}

private func live(_ id: String, _ title: String) -> ShortcutCommand { ShortcutCommand(id: id, title: title, wiredBy: nil, comingSoon: nil) }
private func stub(_ id: String, _ title: String, by lane: String) -> ShortcutCommand { ShortcutCommand(id: id, title: title, wiredBy: lane, comingSoon: nil) }
private func soon(_ id: String, _ title: String, feature: String) -> ShortcutCommand { ShortcutCommand(id: id, title: title, wiredBy: nil, comingSoon: feature) }

public enum Commands {
    /// Every action of Superhuman's v8 list (docs/reference/superhuman-shortcuts-v8.json) plus the Whoosh ones
    /// (palette, accounts, keys). One call per line so checks/superhuman-v8-check.py can read the status of each id.
    public static let all: [ShortcutCommand] = live_ + stubs_ + soon_

    private static let live_: [ShortcutCommand] = [
        live("next", "next"), live("previous", "previous"), live("archive", "archive"), live("snooze", "snooze"),
        live("label", "label"), live("pin", "pin"), live("open", "open"), live("reply", "reply"),
        live("compose", "compose"), live("undo", "undo"), live("palette", "palette"), live("search", "search"),
        live("next-split", "next split"), live("previous-split", "previous split"), live("keys", "keys"),
        live("accounts", "accounts"), live("reply-all", "reply all"), live("forward", "forward"),
        live("delete", "delete"), live("junk", "junk"), live("move", "move"), live("star", "star"),
        live("unread", "mark read or unread"), live("mute", "mute"),
        // the reader (round 9): N P O ⇧O ⌘O Tab space ⇧space ⌘↑ ⌘↓ answer by focus (ReaderKeys), ⇧E is the list's command for the thread on screen
        live("next-message", "next message"), live("previous-message", "previous message"),
        live("expand-message", "expand message"), live("expand-all-messages", "expand all messages"),
        live("toggle-message-header", "expand or collapse message header"),
        live("open-links", "open links and attachments"), live("cycle-links", "cycle through links and dates"),
        live("scroll-down", "scroll down"), live("scroll-up", "scroll up"),
        live("jump-top", "jump to top"), live("jump-bottom", "jump to bottom"),
        live("mark-not-done", "mark not done"),
        live("not-spam", "not spam"),   // palette only, no key (the owner's rule: Superhuman's list has no not-spam key); acts like TriageKeys.notSpam
        live("enter", "open or reply all"),
        live("switch-account-1", "switch to account 1"), live("switch-account-2", "switch to account 2"),
        live("switch-account-3", "switch to account 3"), live("switch-account-4", "switch to account 4"),
        live("switch-account-5", "switch to account 5"), live("switch-account-6", "switch to account 6"),
        live("switch-account-7", "switch to account 7"), live("switch-account-8", "switch to account 8"),
        live("switch-account-9", "switch to account 9"),
        // pop-out compose (composermac, round 3)
        live("popout-draft", "pop draft in or out"), live("popout-reply", "pop out reply"),
        live("popout-reply-all", "pop out reply all"), live("popout-new", "pop out new message"),
        live("popout-forward", "pop out forward"), live("popout-draft-search", "pop out a draft and search"),
        live("switch-draft", "switch to or from a draft"),
        // the AI reply in the thread (reply-thread, contracts delta 20): ⌘J, the quick reply's own key (QuickReplyFeature)
        live("ai-reply", "write a reply draft"),
        // the mail view (A129): the open mail in the sender's colours or the app's. A context key beside the list's filter-important: only while the reader pane has the focus.
        live("toggle-colours", "switch colours"),
    ]

    private static let stubs_: [ShortcutCommand] = [
        // the sidebar: hide or show (⌘\\, the owner's default 2026-11-08)
        stub("sidebar", "hide or show the sidebar", by: "sidebar"),
        // the list: selection, state, filters, labels
        stub("select-conversation", "select conversation", by: "inboxlist"), stub("extend-selection-down", "add next to selection", by: "inboxlist"),
        stub("extend-selection-up", "add previous to selection", by: "inboxlist"), stub("clear-selection", "clear selection", by: "inboxlist"),
        stub("select-from-here", "select all from here", by: "inboxlist"), stub("select-all", "select all", by: "inboxlist"),
        stub("filter-unread", "show unread", by: "inboxlist"), stub("filter-starred", "show starred", by: "inboxlist"),
        stub("filter-important", "show important", by: "inboxlist"), stub("filter-no-reply", "show no reply", by: "inboxlist"),
        stub("remove-label", "remove label", by: "inboxlist"), stub("remove-label-next", "remove label, next", by: "inboxlist"),
        stub("remove-label-previous", "remove label, previous", by: "inboxlist"), stub("remove-all-labels", "remove all labels", by: "inboxlist"),
        // the reader
        stub("unsubscribe", "unsubscribe", by: "reader"), live("cycle-links-back", "cycle back through links and dates"),
        // go to a mailbox
        stub("go-inbox", "go to inbox", by: "sidebar"), stub("go-starred", "go to starred", by: "sidebar"), stub("go-drafts", "go to drafts", by: "sidebar"),
        stub("go-sent", "go to sent", by: "sidebar"), stub("go-done", "go to done", by: "sidebar"), stub("go-reminders", "go to reminders", by: "sidebar"),
        stub("go-muted", "go to muted", by: "sidebar"), stub("go-spam", "go to spam", by: "sidebar"), stub("go-trash", "go to trash", by: "sidebar"),
        stub("go-all-mail", "go to all mail", by: "sidebar"), stub("go-label", "go to label", by: "sidebar"),
        // the composer
        stub("bold", "bold", by: "composermac"), stub("italic", "italic", by: "composermac"), stub("underline", "underline", by: "composermac"),
        stub("hyperlink", "hyperlink", by: "composermac"), stub("color", "color", by: "composermac"), stub("strikethrough", "strikethrough", by: "composermac"),
        stub("numbered-list", "numbered list", by: "composermac"), stub("bullet-list", "bulleted list", by: "composermac"), stub("quote", "quote", by: "composermac"),
        stub("indent-list", "indent list", by: "composermac"), live("outdent-list", "outdent list"),
        stub("increase-indent", "increase indent", by: "composermac"), stub("decrease-indent", "decrease indent", by: "composermac"),
        stub("compose-to", "edit to", by: "composermac"), stub("compose-cc", "edit cc", by: "composermac"), stub("compose-bcc", "edit bcc", by: "composermac"),
        stub("compose-from", "edit from", by: "composermac"), stub("edit-subject", "edit subject", by: "composermac"), stub("edit-message", "edit message", by: "composermac"),
        live("attach", "attach"), stub("discard-draft", "discard draft", by: "composermac"), live("move-contacts-to-bcc", "move contacts to bcc"),
        live("remind-me", "remind me if no reply"), stub("send-later", "send later", by: "composermac"),
        stub("snippet-inline", "use snippet inline", by: "composermac"), stub("use-snippet", "use snippet", by: "composermac"),
        stub("emoji", "insert emoji", by: "composermac"), stub("send", "send", by: "composermac"),
        live("send-and-done", "send and mark done"), live("instant-send", "send now"),
        // the window
        live("back", "back to list"), live("focus-left", "focus left"), live("focus-right", "focus right"),
        stub("new-window", "new window", by: "macshell"), stub("font-bigger", "increase font size", by: "macshell"),
        stub("font-smaller", "decrease font size", by: "macshell"), stub("font-reset", "reset font size", by: "macshell"),
        stub("copy-page-link", "copy page link", by: "macshell"),
    ]

    private static let soon_: [ShortcutCommand] = [
        soon("new-tab", "new tab", feature: "Tabs"), soon("next-tab", "next tab", feature: "Tabs"),
        soon("previous-tab", "previous tab", feature: "Tabs"), soon("close-tab", "close tab", feature: "Tabs"),
        soon("switch-tab-1", "switch to tab 1", feature: "Tabs"), soon("switch-tab-2", "switch to tab 2", feature: "Tabs"),
        soon("switch-tab-3", "switch to tab 3", feature: "Tabs"), soon("switch-tab-4", "switch to tab 4", feature: "Tabs"),
        soon("switch-tab-5", "switch to tab 5", feature: "Tabs"), soon("switch-tab-6", "switch to tab 6", feature: "Tabs"),
        soon("switch-tab-7", "switch to tab 7", feature: "Tabs"), soon("switch-tab-8", "switch to tab 8", feature: "Tabs"),
        soon("switch-tab-9", "switch to tab 9", feature: "Tabs"),
        soon("go-other", "go to other", feature: "Important and Other"), soon("go-snippets", "go to snippets", feature: "Snippets"),
        soon("cal-day", "open day", feature: "Calendar"), soon("cal-week", "open week", feature: "Calendar"),
        soon("cal-today", "today", feature: "Calendar"), soon("cal-previous", "previous day or week", feature: "Calendar"),
        soon("cal-next", "next day or week", feature: "Calendar"), soon("cal-create-event", "create event", feature: "Calendar"),
    ]

    private static let byId: [String: ShortcutCommand] = Dictionary(uniqueKeysWithValues: all.map { ($0.id, $0) })

    public static func title(_ id: String) -> String? { byId[id]?.title }
    public static func command(_ id: String) -> ShortcutCommand? { byId[id] }
}

/// A screen lane adopts this in its own feature type to take over a command the engine only answers with a
/// stand-in (the overlay name). Returning false means "nothing to do in this state" and the key is refused.
@MainActor
public protocol CommandProviding {
    static func commands() -> [String: (AppModel) -> Bool]
}

/// The one table every key, click and palette row dispatches through. A screen's handler (CommandProviding)
/// wins over the stand-in below. Providers are asked by focus (Registrations.byFocus: the feature holding the
/// focus first, then registry order); one returning false passes the command on (appshell lane, lead-approved edit).
@MainActor
public enum CommandRegistry {
    public static func handler(_ id: String) -> ((AppModel) -> Bool)? {
        let provided = FeatureRegistry.all.contains { ($0 as? any CommandProviding.Type)?.commands()[id] != nil }
        guard provided || standIns[id] != nil else { return nil }
        return { m in
            for f in Registrations.byFocus(m) {
                guard let p = f as? any CommandProviding.Type, let h = p.commands()[id] else { continue }
                let before = m.failures
                if h(m) { return true }
                if m.failures != before { return false }   // it tried and failed: not retried by the next provider
            }
            return standIns[id]?(m) ?? false
        }
    }

    /// Answers until the owning screen lands: acts on the list where the shell already can, otherwise names
    /// the overlay it would open, so uistate shows the key arrived. Escape closes it (BaseFeature).
    static let standIns: [String: (AppModel) -> Bool] = {
        var table = baseStandIns
        // A stub or coming-soon id has no handler of its own: it answers "not mine" so the key passes on. The
        // owning lane's CommandProviding handler wins over this (see handler).
        for c in Commands.all where !c.isLive && table[c.id] == nil { table[c.id] = { _ in false } }
        table["mute"] = { _ in false }                 // InboxList provides it; false when nothing is focused
        for n in 1...9 { table["switch-account-\(n)"] = { m in switchAccount(m, n) } }
        table["enter"] = { m in enter(m) }
        table["not-spam"] = { m in MainActor.assumeIsolated { TriageKeys.notSpam(m) } }   // the palette and `click mac-shortcuts.action.not-spam`
        return table
    }()

    /// Enter, the one key with three meanings, picked by the app state. In the plain list view (nothing laid over it) it does
    /// what R does: opens the thread and the quick reply under it, the cursor in the reply field (owner 2026-11-10: "when
    /// pressing enter on a message (it should open it and do the same thing as reply R)"; SPEC A102; the owner's word
    /// overrides Superhuman's "open" for the list view only). With a thread open it replies all, as Superhuman does. Under a
    /// picker, the search or the palette it stays "open", the screen's own handler (InboxList, SearchMac, PaletteMac).
    static func enter(_ m: AppModel) -> Bool {
        let target = ShortcutEngine.threadIsOpen(m) ? "reply-all" : (m.overlay == nil && m.surface == "mac-main" ? "reply" : "open")
        ShortcutEngine.current.resolved = target
        return handler(target)?(m) ?? false
    }

    /// Control-1 to Control-9: filter the list to the Nth account in the core's order. False past the last one.
    private static func switchAccount(_ m: AppModel, _ n: Int) -> Bool {
        let accounts = m.core.accounts()
        guard accounts.indices.contains(n - 1) else { return false }
        m.accountFilter = accounts[n - 1].id
        m.focusIndex = 0
        m.refresh()
        return true
    }

    private static let baseStandIns: [String: (AppModel) -> Bool] = [
        "next": { $0.moveFocus(1) },
        "previous": { $0.moveFocus(-1) },
        "archive": { $0.onFocused { .archive(messageIds: [$0]) } },
        "pin": { m in m.onFocused { id in .pin(messageId: id, pinned: !(m.shownRow?.pinned ?? false)) } },
        "undo": { $0.perform(.undo) },
        "next-split": { $0.switchTab(1) },
        "previous-split": { $0.switchTab(-1) },
        "snooze": overlayOnRow("snooze"),
        "label": overlayOnRow("label"),
        "reply": overlayOnRow("quick-reply"),
        "open": overlayOnRow("mac-thread"),
        "compose": overlay("mac-composer"),
        "palette": overlay("mac-palette"),
        "search": overlay("mac-search"),
        "keys": overlay("mac-trainer.keys"),
        "reply-all": { m in ReplyDoors.replyAll(m) },   // the reply box with every recipient (the key, the toolbar and the strip all land here)
        "forward": { m in ReplyDoors.forward(m) },      // the composer window with the message quoted (was an overlay name no view answered)
        "delete": overlayOnRow("delete"),
        "junk": overlayOnRow("junk"),
        "move": overlayOnRow("move"),
        "star": overlayOnRow("star"),
        "unread": { m in MainActor.assumeIsolated { TriageActions.toggleRead(m) } },   // U toggles read state, as the list's U did (par-inboxlist J1.25)
    ]

    private static func overlay(_ name: String) -> (AppModel) -> Bool {
        { m in m.overlay = name; return true }
    }

    private static func overlayOnRow(_ name: String) -> (AppModel) -> Bool {
        { m in
            guard m.shownMessageId != nil else { return false }
            m.overlay = name
            return true
        }
    }
}

/// Reply all and Forward, the two commands that only set an overlay name before (nothing drew it: a dead key, a dead toolbar
/// button, a dead strip button). One door each, so the key, the toolbar and the reader strip do the same thing.
@MainActor
enum ReplyDoors {
    /// Reply all: the reply box opens under the focused thread (what R does) with the header open and filled: To is the
    /// sender then the other recipients, Cc the original Cc, never an address of the owner's own accounts; a mail with a Reply-To goes
    /// To the Reply-To and copies every other To and Cc (`ReplyToRule.all`).
    static func replyAll(_ m: AppModel) -> Bool {
        let slab = QuickReplyModel.shared
        let words = slab.isOpen ? slab.draft : ""          // opening the box clears the field: words already typed come back
        guard m.shownMessageId != nil, slab.open(m), let id = slab.replyMessageId, let msg = m.core.message(messageId: id) else { return false }
        if !words.isEmpty { slab.draft = words }
        let own = m.core.accounts().map(\.address)
        let state: (String) -> String? = { m.core.state(path: $0) }
        let r = ReplyToRule.all(messageId: id, fromAddress: msg.fromAddress, to: msg.to, cc: msg.cc, own: own, state: state)
        let named = ReplyToRule.entries(id, state: state)
        let people = r.to.map { a in
            ReplyRecipient(name: ComposerContext.contactName(a, core: m.core) ?? named.first { ReplyToRule.same($0.address, a) }?.name ?? "", address: a)
        }
        slab.header.restore(to: people, cc: r.cc, bcc: [], subject: "")
        if !slab.header.open { slab.header.toggle() }
        return true
    }

    /// Forward: the composer window with "Fwd: ", the message quoted and its files listed (ComposerModel.popOutForward).
    static func forward(_ m: AppModel) -> Bool { ComposerMacFeature.model.popOutForward(m) }
}
