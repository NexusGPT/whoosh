import Foundation
import Observation

/// The notch's state. Views read it; the socket reaches the same acts through NotchFeature (acts: NotchActs.swift).
@MainActor @Observable
public final class NotchModel {
    public static let shared = NotchModel()

    public internal(set) var items: [NotchItem] = []      // newest first
    public var stage: NotchStage = .rest
    public var shapeOverride: NotchShape?                  // the rig's `notch-geometry`; nil reads the screen
    public internal(set) var errorWords: String?
    public internal(set) var droppedFile: String?
    public internal(set) var lastArrivalMs: Double?
    /// The one-time notice that a tracked message was first opened (NotchNotice.swift).
    public internal(set) var notice: NotchNotice?
    /// A Focus is on and the notch follows Focus: it stays shut and drops nothing.
    public internal(set) var focusOn = false
    /// The quick send's last result, kept until the mail closes: "Sent" after the undo window ran out.
    public internal(set) var sentShown = false
    /// The silhouette's laid-out size, reported by the view; the pointer tracker hit-tests against it.
    public var shapeSize: CGSize = .zero
    /// The composer's door for a dropped file. Nil until the composer lane provides one (docs/lanes/notch.md N4).
    public var fileSink: ((URL) -> Bool)?
    public var settings = NotchSettings()
    /// The alert settings (owner's picks of 14 Nov 2026: one style per kind of mail, how long an alert stays), read from the core at each arrival.
    public var alerts: NotchAlerts = notchAlertsDefault()
    /// The mail the alert shape shows now (a light, a line or a card). A smaller alert that arrives under a larger one joins the list only.
    public internal(set) var alertId: String?
    /// How many alerts asked for their sound (Settings > Notifications, "Sound"); a hidden test copy counts the ask and plays nothing.
    public internal(set) var soundsAsked = 0
    /// The open mail shows the sender's page (the owner's "Full mail" button) for the mail with this id; it lasts for that mail only.
    public internal(set) var fullMailId: String?
    /// The snooze choices are out under the buttons of the mail with this id.
    public internal(set) var snoozeId: String?
    /// The notch's search (owner's pick 3A, 14 Nov 2026): the field on the list's first line and its answer.
    public let search = NotchSearch()
    public init() { search.onAnswer = { [weak self] in self?.answerArrived() } }
    /// The cursor is in the search field (the notch's key, "/", a click on the field): typed letters go to it.
    public internal(set) var searchFocused = false
    /// The mail of the row the keyboard is on; nil while the cursor is in the search field or the pointer drives.
    public internal(set) var rowFocusId: String?
    /// The person asked for the keyboard in the notch (its key from any app, "/", a click in the search field): the notch keeps it until it
    /// closes, a pointer passing by does not close it, and an alert taken this way does not leave on its timer.
    public internal(set) var keyboardWanted = false
    /// The mail Open in Whoosh last handed to the main window (a check reads it; the main window's own state is the proof).
    public internal(set) var handedOver: String?
    /// When the notch last took the keyboard, and whether it already took it back once after losing it while the take settled (keyboardLost).
    @ObservationIgnored var keyboardTakenAt: Date?
    @ObservationIgnored var keyboardRetaken = false
    /// The open mail was opened from the list (a row, a search result): Escape goes back there. A mail opened from an alert closes the notch.
    public internal(set) var openedFromList = false
    /// The pointer is on the notch (NotchPointer's crossings; the rig's hover click).
    public internal(set) var pointerOn = false
    /// The mail the notch started a reply on (Reply on the open mail, a Reply button): the reply ends when that mail leaves the notch.
    @ObservationIgnored var replyStartedFor: String?
    /// The words of the mails opened lately (contracts `notchWords` over the body the reader gets): one read per mail.
    @ObservationIgnored var wordsOf: [String: NotchWords] = [:]
    /// What the core answered at each settings path the last time it was read (NotchSettings.read).
    var seenCore: [String: String] = [:]
    /// Accounts the rig muted (`fixture mute-account`); in the real flow the push router decides muting before a drop.
    public var muted: Set<String> = []
    /// The app in front, as the rig says it (`fixture front-app`); Fill pastes into the real front app.
    public var frontApp = ""
    /// Told after every change, so the panel can show itself and measure the arrival.
    var onChange: () -> Void = {}
    var arrivedAt: [String: Date] = [:]
    /// Changes whenever the shape drops out for a new arrival or notice; the view's hold timer restarts on it.
    public internal(set) var dropToken = 0

    public var openItem: NotchItem? { if case .open(let id) = stage { return items.first { $0.id == id } }; return nil }
    public var codeItem: NotchItem? { arrived.first { $0.code != nil } }
    /// The mails that arrived (the push feed's drops), newest first: a mail the search found is in `items` only while the notch acts on it.
    public var arrived: [NotchItem] { items.filter { !$0.found } }
    /// What the hover list shows: the latest mail, or only the unread, as the setting says.
    public var listed: [NotchItem] { settings.show == .unread ? arrived.filter { !$0.read } : arrived }
    public var unreadCount: Int { arrived.filter { !$0.read }.count }
    /// The rows under the search field: the search's answer while words are typed there, else the list.
    public var shownRows: [NotchItem] { search.active ? search.results : Array(listed.prefix(NotchMetrics.listMax)) }
    /// The row the keyboard is on (the down arrow from the search field, then the arrows), by its mail.
    public var focusedRow: NotchItem? { rowFocusId.flatMap { id in shownRows.first { $0.id == id } } }
    /// Board a's rest: the housing grows two wings (the newest unread mail's account left, the unread count right)
    /// while unread mail waits; with nothing unread the shape is the housing alone.
    public var wingsShown: Bool { (stage == .rest || stage == .light) && unreadCount > 0 && notice == nil }
    public var newestUnread: NotchItem? { arrived.first { !$0.read } }
    /// The mail the alert shape shows (the light's colour, the line, the card): the one that raised it, else the newest.
    public var dropItem: NotchItem? { alertId.flatMap { id in arrived.first { $0.id == id } } ?? arrived.first }

    /// A drop from the push feed (muting and resync are decided there). A repeat id changes nothing;
    /// with Focus on, or the delivery set to notifications only, the notch stays shut.
    /// `kind` and `style` come from the engine (NotchFeature.deliver: `notchKind`, then the alert settings); a caller that names neither shows a card.
    public func arrive(_ drop: NotchDrop, preview: String = "", threadId: String = "", address: String = "", receivedAt: Date = Date(),
                       kind: NotchKind = .primary, style: NotchAlertStyle = .card) {
        guard !muted.contains(drop.accountId), !items.contains(where: { $0.id == drop.messageId }) else { return }
        items.insert(NotchItem(id: drop.messageId, threadId: threadId, accountId: drop.accountId, sender: drop.senderName,
                               subject: drop.subject, preview: preview, code: drop.loginCode, address: address, receivedAt: receivedAt, kind: kind), at: 0)
        guard settings.delivery.usesNotch, !(focusOn && settings.followFocus), style != .nothing else { onChange(); return }
        arrivedAt[drop.messageId] = Date()
        if show(style, id: drop.messageId) {
            dropToken &+= 1
            if alerts.sound { soundsAsked += 1; NotchSound.play() }
        }
        errorWords = nil
        onChange()
    }

    /// How much of the screen an alert shape takes: rest 0, a light 1, a line 2, a card 3. The list and an open mail are the person's own: nil.
    private static func size(_ stage: NotchStage) -> Int? {
        switch stage { case .rest: return 0; case .light: return 1; case .line: return 2; case .drop: return 3; case .list, .open: return nil }
    }

    /// Shows an arrival in its style. An alert never replaces what the person opened (the list, a mail), and a smaller alert never replaces a
    /// larger one that still shows; the new mail is in the list either way. "Opened" opens the mail with no reply started, so the notch asks
    /// for no keyboard (owner's plan of 14 Nov: R or a click in the reply does). True when the shape now shows this mail.
    private func show(_ style: NotchAlertStyle, id: String) -> Bool {
        guard let now = Self.size(stage) else { return false }
        let next: NotchStage, size: Int
        switch style {
        case .nothing: return false
        case .light: next = .light; size = 1
        case .line: next = .line; size = 2
        case .card: next = .drop; size = 3
        case .opened: next = .open(id); size = 4
        }
        guard size >= now else { return false }
        stage = next
        alertId = id
        sentShown = false
        return true
    }

    /// The panel reports the first frame it showed the newest mail on.
    func shown() {
        guard let id = items.first?.id, let t = arrivedAt.removeValue(forKey: id) else { return }
        lastArrivalMs = Date().timeIntervalSince(t) * 1000
    }

    /// NOTCH_DROP_HOLD_S ran out: a dropped mail that nothing touched shrinks back into the housing. The mail stays
    /// in the list; only the shape goes. Pointer, Escape or an act changed the stage first, so none of them is undone here.
    func holdEnded(_ token: Int) {
        guard token == dropToken, !keyboardWanted, stage == .drop || stage == .light || stage == .line else { return }
        stage = .rest
        notice = nil
        onChange()
    }

    /// Pointer on the shape (or resting at the top edge) grows it into the list; leaving shrinks it back.
    /// An open mail with a reply being written stays open when the pointer leaves.
    public func hover(_ on: Bool) {
        pointerOn = on
        if on, focusOn && settings.followFocus { return }
        // the person took the notch with the keyboard (its key, "/", a click in the search field): a pointer passing by does not close it
        if !on, keyboardWanted { return }
        switch (on, stage) {
        case (true, .rest), (true, .drop), (true, .light), (true, .line): stage = .list
        case (false, .list), (false, .drop), (false, .light), (false, .line): stage = .rest
        case (false, .open): guard QuickReplyModel.shared.draft.isEmpty, !QuickReplyModel.shared.isCounting else { return }; endOpenReply(); stage = .rest
        default: return
        }
        onChange()
    }

    /// Escape: the shape shrinks back into the housing from any stage.
    @discardableResult
    public func collapse() -> Bool {
        guard stage != .rest || notice != nil else { return false }
        endOpenReply()
        stage = .rest
        notice = nil
        leaveSearch()
        onChange()
        return true
    }

    /// The notch closed: the search is emptied, the keyboard's place is forgotten (the panel gives the keyboard back, NotchPanel.changed), and a
    /// found mail the notch no longer shows leaves `items`.
    func leaveSearch() {
        search.clear()
        searchFocused = false
        rowFocusId = nil
        keyboardWanted = false
        openedFromList = false
        let open = openItem?.id
        items.removeAll { $0.found && $0.id != open }
    }

    /// The mail the notch showed is leaving it (back, the close mark, Escape, an act that removed it, another mail opened): a reply the notch
    /// started on it ends with it. Its words stay as the conversation's draft (the reply's own close keeps them), so Reply continues them; a send
    /// still in its undo window is left alone. Without this the reply stayed open behind a closed notch and kept taking the main window's keys.
    func endOpenReply() {
        fullMailId = nil
        snoozeId = nil
        guard let id = replyStartedFor else { return }
        replyStartedFor = nil
        let qr = QuickReplyModel.shared
        guard qr.isOpen, qr.replyMessageId == id, !qr.isCounting else { return }
        _ = qr.close(AppModel.shared)
    }

    /// A Focus turned on or off. On, with the notch following Focus, it shuts at once.
    public func setFocus(_ on: Bool) {
        focusOn = on
        if on && settings.followFocus { endOpenReply(); stage = .rest; notice = nil; leaveSearch() }
        onChange()
    }

    func showError(_ words: String?) { errorWords = words; onChange() }

    /// The segmented control in the list's band: Latest or Unread (the same value the setting holds).
    @discardableResult
    public func setShow(_ show: NotchSettings.Show) -> Bool {
        guard settings.show != show else { return false }
        settings.show = show
        onChange()
        return true
    }
}

extension QuickReplyModel {
    var isCounting: Bool { outboxId != nil }
}
