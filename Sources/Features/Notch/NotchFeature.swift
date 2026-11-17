import Foundation
#if os(macOS)
import AppKit
import ApplicationServices
#endif

/// The notch lane's registrations and its one wiring call for the app shell. Rows: NotchRows.swift;
/// the rig's stage clicks: NotchStage.swift.
@MainActor
public enum NotchFeature: FeatureModule {
    public static let id = "notch"
    public static let surfaces: [String] = [NotchIds.surface]
    static var model: NotchModel { .shared }
    private static var subscription: (NotchFeed, UInt64)?

    /// The app shell's call, once: every drop from the push feed lands in the panel. The panel itself is made on the
    /// first drop (deliver), not at launch: the first frame holds the main window only, and an idle app keeps no panel.
    public static func attach(feed: NotchFeed) {
        NotchEvents.start()
        wire()
        #if os(macOS)
        NotchHotKey.start()     // the notch's key from any app; never in a hidden test copy
        #endif
        if let (old, token) = subscription { old.unsubscribe(token) }
        let token = feed.subscribe { drop in
            DispatchQueue.main.async { MainActor.assumeIsolated { deliver(drop) } }
        }
        subscription = (feed, token)
    }

    /// The composer's door for a dropped file (docs/lanes/notch.md N4, closed round 1 of look A): a file dropped on the
    /// notch goes to the open composer, or opens a new one with it. Set once, from attach and from the first event.
    static func wire() {
        guard model.fileSink == nil else { return }
        model.fileSink = { url in ComposerMacFeature.acceptFile(url); return true }
    }

    /// One drop into the panel, with the preview, thread, sender address and time read from the inbox the app already holds.
    public static func deliver(_ drop: NotchDrop, preview: String? = nil, threadId: String? = nil, address: String? = nil, _ m: AppModel = .shared) {
        #if os(macOS)
        NotchPanel.shared.install()
        #endif
        NotchEvents.start()
        wire()
        model.settings = NotchSettings.read(m.core, from: model.settings, seen: &model.seenCore)   // a setting changed since the last drop applies now
        NotchFocus.refresh()
        let row = m.rows.first { $0.messageId == drop.messageId }
        let at = row.map { Date(timeIntervalSince1970: Double($0.receivedAtMs) / 1000) } ?? Date()
        // the alert settings and the mail's kind are the engine's (one rule for both engines: contracts notch_alerts.rs); a drop that carries a
        // sign-in code is that kind whatever the store says of the mail yet
        model.alerts = m.core.notchAlerts()
        let kind: NotchKind = drop.loginCode != nil ? .signInCode : m.core.notchKind(messageId: drop.messageId)
        model.arrive(drop, preview: preview ?? row?.preview ?? "", threadId: threadId ?? row?.threadId ?? "",
                     address: address ?? row?.senderAddress ?? "", receivedAt: at, kind: kind, style: notchAlertStyle(alerts: model.alerts, kind: kind))
    }

    /// A tracked message was opened by its recipient (the core has no such event yet: N10).
    public static func trackedOpened(messageId: String, subject: String, who: String, seen: NotchSeenStore = .standard) {
        #if os(macOS)
        NotchPanel.shared.install()
        #endif
        model.trackedOpened(messageId: messageId, subject: subject, who: who, seen: seen)
    }

    /// Settings changed (or may have): read the core's values again and apply the one with a visible effect,
    /// the window's sharing type, at once.
    static func rereadSettings(_ m: AppModel = .shared) {
        let fresh = NotchSettings.read(m.core, from: model.settings, seen: &model.seenCore)
        guard fresh != model.settings else { return }
        model.settings = fresh
        #if os(macOS)
        NotchPanel.shared.applySharing()
        #endif
        model.onChange()
    }

    public static var arrivalMs: Double? { model.lastArrivalMs }

    @discardableResult
    static func copyCode() -> Bool {
        model.copyCode(.shared) { code in
            #if os(macOS)
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(code, forType: .string)
            #endif
        }
    }

    /// Fill: the code goes on the clipboard and is pasted into the field of the app in front. Pasting into
    /// another app needs the Accessibility permission; without it the code stays on the clipboard and the notch says so.
    /// A test copy never reads that permission and never sends the paste key (17 Nov 2026): the key goes to whatever app the owner
    /// has in front, and a test copy carries the installed app's permissions (NotchFocus). A hidden rig counts the ask instead
    /// (`core state system-ui/paste-into-front-app`) and answers as a Mac without the permission does.
    @discardableResult
    static func fillCode() -> Bool {
        guard copyCode() else { return false }
        #if os(macOS)
        guard HiddenRig.allowSystemUI("paste-into-front-app", "Command-V"), AppModel.shared.host.fixture == nil, AXIsProcessTrusted() else {
            model.showError(NotchCopy.fillNeedsAccess); return true
        }
        let src = CGEventSource(stateID: .combinedSessionState)
        for down in [true, false] {   // virtual key 9 is V: Command-V pastes into the app in front
            let e = CGEvent(keyboardEventSource: src, virtualKey: 9, keyDown: down); e?.flags = .maskCommand; e?.post(tap: .cghidEventTap)
        }
        #endif
        return true
    }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] { NotchEvents.start(); return NotchRows.all.merging(NotchRows.searchRows) { mine, _ in mine } }

    /// The keys the open mail shows on its controls, read from the preset so a copied preset's caps follow it.
    static func keyCap(_ command: String, fallback: String) -> String { ShortcutEngine.shared.chord(for: command) ?? fallback }
    static var archiveKey: String { keyCap("archive", fallback: "E") }
    static var readKey: String { keyCap("unread", fallback: "U") }
    static var replyKey: String { keyCap("reply", fallback: "R") }
    static var snoozeKey: String { keyCap("snooze", fallback: "H") }
    /// Full mail and Its words: the preset's key that expands a message in the main window.
    static var fullMailKey: String { keyCap("expand-message", fallback: "O") }
    static let sendKey = "⌃Enter"   // the owner's send key (QuickReplyFeature binds ⌃Enter and ⌘Enter)

    /// A key is the notch's only when it is meant for the notch: the pointer is on it, or it holds the keyboard (a reply being written). A key
    /// pressed in the main window while a card or a mail merely shows in the notch stays the main window's: before 14 Nov 2026 E pressed on the
    /// list archived the mail open in the notch instead of the selected one.
    static var takesKeys: Bool { model.pointerOn || NotchRows.keyed }

    /// Open in Whoosh, on every state of the notch.
    static let openInWhooshKey = "⌘O"
    /// The key that goes to the notch's search field: the preset's search key, and "/" in every preset.
    static var searchKey: String { keyCap("search", fallback: "/") }

    /// The notch's keys, each behind `takesKeys`. Escape goes back one step; Return opens; Down and Up move between the search field and the
    /// mails; "/" goes to the search field; ⌘O opens the mail in Whoosh; the preset's archive, read, reply and snooze keys act on the mail the
    /// keyboard is on (the open mail, the list's row, an alert's mail); on an open mail the expand key shows the full mail and ⌃Enter sends
    /// the reply. A key the notch has no use for passes on, except while the notch holds the keyboard: there every key it binds stops with it,
    /// so it never runs as a command of the main window behind.
    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        var k: [String: (AppModel) -> Bool] = [
            "Escape": { m in model.back(m) },
            "Enter": { m in !model.replyTargetsOpen && model.enter(m) },
            // the arrows move in the list only for a notch taken with the keyboard: a pointer resting on the notch never takes the arrows of
            // the main window's list
            "Down": { _ in model.keyboardWanted && !model.replyTargetsOpen && model.moveRow(1) },
            "Up": { _ in model.keyboardWanted && !model.replyTargetsOpen && model.moveRow(-1) },
            openInWhooshKey: { m in model.stage != .rest && model.stage != .light && model.openInWhoosh(m) },
            sendKey: { m in model.replyTargetsOpen && QuickReplyModel.shared.send(m) },
        ]
        // to the search field, from a card, a line, an open mail or a row; "⇧/" is the same key on a keyboard that types "/" with Shift
        for chord in Set(["/", "⇧/", AppModel.normalise(searchKey)]) {
            k[chord] = { _ in model.stage != .rest && model.stage != .light && !model.replyTargetsOpen && !(model.stage == .list && model.searchFocused) && model.focusSearch() }
        }
        k[AppModel.normalise(archiveKey)] = { m in
            guard !model.replyTargetsOpen, let it = model.keyTarget else { return false }
            return model.act(on: it, m) { id, m in model.archive(id: id, m) }
        }
        // the read key marks an unread mail read and a read one unread, as it does on the main window's list
        k[AppModel.normalise(readKey)] = { m in
            guard !model.replyTargetsOpen, let it = model.keyTarget else { return false }
            return model.act(on: it, m) { id, m in it.read ? model.markUnread(id: id, m) : model.markRead(id: id, m) }
        }
        k[AppModel.normalise(replyKey)] = { m in
            guard !model.replyTargetsOpen, let it = model.keyTarget else { return false }
            if case .open = model.stage { return model.reply(m) }
            return model.openRow(it, m, reply: true)      // a row or an alert: the mail opens with the reply started
        }
        k[AppModel.normalise(snoozeKey)] = { m in
            guard !model.replyTargetsOpen, let it = model.keyTarget else { return false }
            if case .open = model.stage { return model.toggleSnooze() }
            return model.openRow(it, m) && model.toggleSnooze()   // a row or an alert: the mail opens with the three choices out
        }
        k[AppModel.normalise(fullMailKey)] = { m in model.openItem != nil && !model.replyTargetsOpen && model.toggleFullMail(m) }
        for c in NotchSnoozeChoices.choices {
            k[c.key] = { m in model.snoozeId != nil && model.snoozeId == model.openItem?.id && !model.replyTargetsOpen && model.snooze(c.when, m) }
        }
        var gated: [String: (AppModel) -> Bool] = [:]
        for (chord, act) in k { gated[chord] = { m in takesKeys && (act(m) || holdsKeyboard(m)) } }
        #if os(macOS)
        // which of these keys a pressed letter is left to (NotchTyping): all of them on a card or a mail; on the list the open mail's own keys
        // (the full mail, the snooze choices, the send key) mean nothing, so there they are letters for the search field
        let own = Set(gated.keys)
        let onList = own.subtracting([AppModel.normalise(fullMailKey), sendKey] + NotchSnoozeChoices.choices.map(\.key))
        NotchTyping.installOnce { model.stage == .list ? onList : own }
        #endif
        return gated
    }

    /// The notch's panel is the key window and no reply is written in it: the notch is asked for every key before any other screen, and every
    /// key it binds stops with it. While a reply is written there the reply's own keys come first (its Escape, its send key, its From list).
    public static func holdsKeyboard(_ m: AppModel) -> Bool { NotchRows.keyed && !model.replyTargetsOpen }

    /// The socket's `type` and a paste with no text field under them: while the notch has the keyboard on its list, the letters go to its
    /// search field (the reply, asked before the notch, takes them while it is written).
    public static func typeText(_ text: String, _ m: AppModel) -> Bool {
        takesKeys && model.keyboardWanted && model.typeSearch(text, m)
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        NotchEvents.start()
        wire()
        var t: [String: (AppModel) -> Bool] = [
            NotchIds.codePill: { _ in copyCode() },
            NotchIds.copy: { _ in copyCode() },
            NotchIds.fill: { _ in fillCode() },
            NotchIds.openClose: { _ in model.collapse() },          // board a: the close mark returns to rest
            NotchIds.openBack: { _ in model.close() },               // the chevron: back to the list
            NotchIds.whoosh: { m in model.openInWhoosh(m) },
            NotchIds.segLatest: { _ in model.stage != .rest && model.setShow(.latest) },
            NotchIds.segUnread: { _ in model.stage != .rest && model.setShow(.unread) },
            NotchIds.openArchive: { m in guard let it = model.openItem else { return false }; return model.archive(id: it.id, m) },
            NotchIds.openMarkRead: { m in guard let it = model.openItem else { return false }; return model.markRead(id: it.id, m) },
            NotchIds.openFrom: { _ in model.openItem != nil && QuickReplyModel.shared.toggleFrom() },
            NotchIds.collapse: { _ in model.collapse() },
            NotchIds.openQuickReply: { m in model.openItem.map { model.openReply(for: $0, m) } ?? false },
            NotchIds.send: { m in model.replyTargetsOpen && QuickReplyModel.shared.send(m) },
            NotchIds.undoAction: { m in QuickReplyModel.shared.undo(m) },
            NotchIds.settings: { m in
                #if os(macOS)
                return WindowBridge.open(.macSettingsGeneral, in: m)
                #else
                return false
                #endif
            },
            NotchIds.notice: { _ in guard model.notice != nil else { return false }; model.dismissNotice(); return true },
            NotchIds.lineOpen: { m in guard model.stage == .line, let it = model.dropItem else { return false }; return model.open(id: it.id, m) },
            NotchIds.openFullMail: { m in model.toggleFullMail(m) },
            NotchIds.openReply: { m in model.reply(m) },
            NotchIds.openSnooze: { _ in model.toggleSnooze() },
            NotchIds.openMoreUnread: { m in guard let it = model.openItem else { return false }; return model.markUnread(id: it.id, m) },
            NotchIds.openMoreDelete: { m in guard let it = model.openItem else { return false }; return model.delete(id: it.id, m) },
        ]
        // The arrival card's controls (sweep-notch F1 and F3): each answers only while the card draws it (the card is up, no notice over it, the newest mail has no sign-in
        // code, and for a button the setting asks for it), then does what the button does.
        func arrival(_ button: NotchSettings.Button?, _ act: @escaping (NotchItem, AppModel) -> Bool) -> (AppModel) -> Bool {
            { m in
                guard model.stage == .drop, model.notice == nil, let it = model.dropItem, it.code == nil else { return false }
                if let button, !model.settings.buttons.contains(button) { return false }
                return act(it, m)
            }
        }
        t[NotchIds.arrivalMarkRead] = arrival(.markRead) { it, m in !it.read && model.markRead(id: it.id, m) }
        t[NotchIds.arrivalReply] = arrival(.reply) { it, m in model.open(id: it.id, m, reply: true) }
        t[NotchIds.arrivalDelete] = arrival(.delete) { it, m in model.delete(id: it.id, m) }
        t[NotchIds.arrivalArchive] = arrival(.archive) { it, m in model.archive(id: it.id, m) }
        #if os(macOS)
        // the option-drag along the top edge (NotchDrag), as a door: a tenth of the screen's width either way (sweep-notch F4); the fraction is served on the surface row
        t[NotchIds.dragLeft] = { _ in NotchDrag.shared.moveByDoor(-0.1) }
        t[NotchIds.dragRight] = { _ in NotchDrag.shared.moveByDoor(0.1) }
        #endif
        t[NotchIds.arrivalOpen] = arrival(nil) { it, m in model.open(id: it.id, m) }
        for a in AccountBook.shared.accounts(core: AppModel.shared.core) {
            t[NotchIds.openFromPick(a.id)] = { _ in model.openItem != nil && QuickReplyModel.shared.pickAccount(a.id) }
        }
        for i in 1...NotchMetrics.socketRows {
            t[NotchIds.row(i)] = { m in model.listed.indices.contains(i - 1) && model.open(id: model.listed[i - 1].id, m) }
            t[NotchIds.delete(i)] = { m in model.listed.indices.contains(i - 1) && model.delete(id: model.listed[i - 1].id, m) }
            t[NotchIds.archive(i)] = { m in model.listed.indices.contains(i - 1) && model.archive(id: model.listed[i - 1].id, m) }
            t[NotchIds.markRead(i)] = { m in model.listed.indices.contains(i - 1) && model.markRead(id: model.listed[i - 1].id, m) }
            // the row's Reply (sweep-notch F2): drawn on hover when the setting asks for it, it opens the mail in place with the quick reply ready
            t[NotchIds.reply(i)] = { m in model.stage != .rest && model.settings.buttons.contains(.reply) && model.listed.indices.contains(i - 1) && model.open(id: model.listed[i - 1].id, m, reply: true) }
        }
        for c in NotchSnoozeChoices.choices { t[NotchIds.openSnoozePick(c.word)] = { m in model.snoozeId != nil && model.snooze(c.when, m) } }
        // the search: a click on the field puts the cursor there (and takes the keyboard), the clear mark empties it, a row of the answer opens its mail
        t[NotchIds.searchField] = { _ in model.stage == .list && model.focusSearch() }
        t[NotchIds.searchClear] = { m in guard model.stage == .list, model.search.active else { return false }; model.setSearch("", m); return true }
        t[NotchIds.searchBack] = { m in guard model.stage == .list, model.search.active else { return false }; model.setSearch("", m); return true }
        for i in 1...NotchMetrics.listMax {
            t[NotchIds.searchRow(i)] = { m in
                guard model.stage == .list, model.search.active, model.search.results.indices.contains(i - 1) else { return false }
                return model.openRow(model.search.results[i - 1], m)
            }
        }
        NotchRigStage.add(to: &t)
        return t
    }

    public static func paletteCommands() -> [(title: String, key: String)] { NotchEvents.start(); return [(Words.text("Copy the sign-in code"), "")] }

    /// The shell asks every feature for focus on each key, so the event listeners are up by the first key even
    /// before the shell calls `attach` (N2). The rule is the protocol's default.
    /// A grown shape holds the keys first (its Escape, and the open mail's archive, read and send keys); each binding
    /// answers false for a key it has no use for, so the key goes on to the window beneath.
    public static func hasFocus(_ m: AppModel) -> Bool {
        NotchEvents.start()
        return model.stage != .rest || (m.surface != SurfaceSlot.macMain.id && surfaces.contains(m.surface)) || m.overlay.map(surfaces.contains) == true
    }
}
