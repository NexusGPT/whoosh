import Foundation

/// Every control id and every user-facing sentence of the notch, in one place (docs/lanes/tests-j6.md "Assumed ids").
public enum NotchIds {
    public static let surface = "mac-notch"
    public static let list = "mac-notch.list"
    public static let codePill = "mac-notch.codepill"
    public static let fill = "mac-notch.fill"
    public static let empty = "mac-notch.empty"
    public static let error = "mac-notch.error"
    public static let undo = "mac-notch.undo"
    public static let openBody = "mac-notch.open.body"
    public static let openActions = "mac-notch.open.actions"
    public static let openQuickReply = "mac-notch.open.quickreply"
    public static let openClose = "mac-notch.open.close"
    public static let drop = "mac-notch.drop"
    public static let stageLight = "mac-notch.stage.light"
    public static let stageDark = "mac-notch.stage.dark"
    public static let stageNoNotch = "mac-notch.stage.nonotch"
    public static let stageHover = "mac-notch.stage.hover"
    public static let stageSample = "mac-notch.stage.sample"
    public static let stageTracked = "mac-notch.stage.tracked"
    public static let stageEdge = "mac-notch.stage.edge"
    public static let stageDropFile = "mac-notch.stage.dropfile"
    public static func stageFocus(_ on: Bool) -> String { "mac-notch.stage.focus.\(on ? "on" : "off")" }
    public static func stageSetting(_ key: String, _ value: String) -> String { "mac-notch.stage.setting.\(key).\(value)" }
    public static let collapse = "mac-notch.collapse"
    public static let send = "mac-notch.open.send"
    public static let undoAction = "mac-notch.undo.action"
    public static let notice = "mac-notch.notice"
    public static let settings = "mac-notch.settings"
    public static func delete(_ i: Int) -> String { "\(list).row.\(i).delete" }
    public static let buttons = "mac-notch.buttons"
    public static func row(_ i: Int) -> String { "\(list).row.\(i)" }
    public static func archive(_ i: Int) -> String { "\(list).row.\(i).archive" }
    public static func markRead(_ i: Int) -> String { "\(list).row.\(i).markread" }
    public static func reply(_ i: Int) -> String { "\(list).row.\(i).reply" }
    // the arrival card's own controls (NotchArrivalView): its Mark read, Reply, Delete and Archive buttons and a tap anywhere on the card
    public static let dragLeft = "mac-notch.drag.left"
    public static let dragRight = "mac-notch.drag.right"
    public static let arrivalMarkRead = "mac-notch.arrival.markread"
    public static let arrivalReply = "mac-notch.arrival.reply"
    public static let arrivalDelete = "mac-notch.arrival.delete"
    public static let arrivalArchive = "mac-notch.arrival.archive"
    public static let arrivalOpen = "mac-notch.arrival.open"
    public static var windowTitle: String { Words.text("Notch") }
    // Round 1 of look A (board a, Continuum): the band beside the camera, the wings at rest, the list's end mark,
    // the open mail's header, its From line, the segmented Latest | Unread, Open in Whoosh, the code block's Copy.
    public static let shape = "mac-notch.shape"
    public static let band = "mac-notch.band"
    public static let wings = "mac-notch.wings"
    public static let end = "mac-notch.end"
    public static let foot = "mac-notch.foot"
    public static let copy = "mac-notch.copy"
    public static let whoosh = "mac-notch.whoosh"
    public static let segLatest = "mac-notch.seg.latest"
    public static let segUnread = "mac-notch.seg.unread"
    public static let openBack = "mac-notch.open.back"
    public static let openHeader = "mac-notch.open.header"
    public static let openAttachments = "mac-notch.open.attachments"
    public static let openFrom = "mac-notch.open.from"
    public static func openFromPick(_ accountId: String) -> String { "mac-notch.open.from.\(accountId)" }
    public static let openArchive = "mac-notch.open.archive"
    public static let openMarkRead = "mac-notch.open.markread"
    public static let stageOpen = "mac-notch.stage.open"
    public static let stageSampleThree = "mac-notch.stage.sample3"
    public static let stageAged = "mac-notch.stage.aged"
    // The alerts by kind of mail (owner's picks of 14 Nov 2026): the light, the one line, the model's copy of the alert settings, and the rig's stage
    // clicks (an arrival of one kind through the real delivery door; one kind's style and the stay, saved through the engine).
    public static let light = "mac-notch.light"
    public static let line = "mac-notch.line"
    public static let lineOpen = "mac-notch.line.open"
    public static let alerts = "mac-notch.alerts"
    public static func stageArrive(_ kind: String) -> String { "mac-notch.stage.arrive.\(kind)" }
    public static func stageAlert(_ kind: String, _ style: String) -> String { "mac-notch.stage.alert.\(kind).\(style)" }
    public static func stageStay(_ word: String) -> String { "mac-notch.stage.stay.\(word)" }
    /// The rig: the mail the notch shows (open, else the alert's, else the newest) becomes a designed one in the test engine, so it has a page.
    public static let stageDesigned = "mac-notch.stage.designed"
    // The open mail as picked on 14 Nov 2026: its words, the folded parts, Full mail and Its words, the page, Reply, Snooze and its choices, the menu.
    public static let openFolded = "mac-notch.open.folded"
    public static let openFolds = "mac-notch.open.folds"
    public static let openFullMail = "mac-notch.open.fullmail"
    public static let openPageFacts = "mac-notch.open.page.facts"
    public static let openPage = "mac-notch.open.page"
    public static let openReply = "mac-notch.open.reply"
    public static let openSnooze = "mac-notch.open.snooze"
    public static func openSnoozePick(_ word: String) -> String { "mac-notch.open.snooze.\(word)" }
    public static let openMore = "mac-notch.open.more"
    public static let openMoreUnread = "mac-notch.open.more.unread"
    public static let openMoreDelete = "mac-notch.open.more.delete"
    // The search (owner's pick 3A, 14 Nov 2026): the field on the list's first line, its answer, the hints under the list.
    public static let search = "mac-notch.search"
    public static let searchField = "mac-notch.search.field"
    public static let searchClear = "mac-notch.search.clear"
    public static let searchBack = "mac-notch.search.back"
    public static let searchResults = "mac-notch.search.results"
    public static func searchRow(_ i: Int) -> String { "mac-notch.search.row.\(i)" }
    public static let hints = "mac-notch.hints"
    /// The rig: the notch's key from any app (a hidden test copy registers no system-wide key; this runs the key's own act).
    public static let stageSummon = "mac-notch.stage.summon"
}

/// The notch's words. Short, in the owner's register; no build state ever reaches the screen.
public enum NotchCopy {
    public static var empty: String { Words.text("That's everything") }
    public static var copied: String { Words.text("Copied") }
    public static var fill: String { Words.text("Fill") }
    public static var fillHelp: String { Words.text("Type the code into the field in front") }
    public static var fillNeedsAccess: String { Words.text("Allow Whoosh in Accessibility to fill codes") }
    public static var archive: String { Words.text("Archive") }
    public static var delete: String { Words.text("Delete") }
    public static var settings: String { Words.text("Notch settings") }
    public static var markRead: String { Words.text("Mark read") }
    public static var reply: String { Words.text("Reply") }
    public static var open: String { Words.text("Open") }
    /// Where the open mail's back chevron goes: the search's answer the mail was opened from, else the list as it is read (Latest, Unread).
    @MainActor public static func backWord(_ model: NotchModel) -> String {
        if model.search.active && model.openedFromList { return searchWord }
        return model.settings.show == .unread ? unread : latest
    }
    public static var searchPlaceholder: String { Words.text("Search all mail") }
    public static var searchWord: String { Words.text("Search") }
    public static var related: String { Words.text("Related") }
    public static var noMatch: String { Words.text("No match") }
    public static var typeToSearch: String { Words.text("Type to search") }
    public static var toTheMails: String { Words.text("To the mails") }
    public static var select: String { Words.text("Select") }
    public static var fullMail: String { Words.text("Full mail") }
    public static var itsWords: String { Words.text("Its words") }
    public static var snooze: String { Words.text("Snooze") }
    public static var markUnread: String { Words.text("Mark unread") }
    public static var more: String { Words.text("More") }
    public static func picturesNotLoaded(_ n: Int) -> String { n == 1 ? Words.text("1 picture not loaded") : Words.text("%@ pictures not loaded", n) }
    /// Under the full mail: its pictures (loaded, or left out when the setting is off) and its tracking pixels, the two facts joined by a dot.
    public static func pageFacts(pictures: Int, trackers: Int, loaded: Bool) -> String {
        var parts: [String] = []
        if pictures > 0 { parts.append(loaded ? (pictures == 1 ? Words.text("1 picture loaded") : Words.text("%@ pictures loaded", pictures)) : picturesNotLoaded(pictures)) }
        if trackers > 0 { parts.append(trackers == 1 ? Words.text("1 tracker blocked") : Words.text("%@ trackers blocked", trackers)) }
        return parts.joined(separator: " · ")
    }
    public static func fold(_ f: NotchFold) -> String {
        switch f.kind {
        case .earlierMessages: return f.count == 1 ? Words.text("1 earlier message") : Words.text("%@ earlier messages", Int(f.count))
        case .forwardedMessage: return Words.text("Forwarded message")
        case .signature: return Words.text("Signature")
        case .legalNotice: return Words.text("Legal notice")
        }
    }
    public static func folded(_ folds: [NotchFold]) -> String { Words.text("Folded: %@", folds.map(fold).joined(separator: ", ")) }
    public static func snoozeChoice(_ when: SnoozeWhen) -> String {
        switch when {
        case .laterToday: return Words.text("Later today")
        case .tomorrowMorning: return Words.text("Tomorrow morning")
        case .nextWeek: return Words.text("Next week")
        case .at: return Words.text("Snooze")
        }
    }
    public static var close: String { Words.text("Close") }
    public static var latest: String { Words.text("Latest") }
    public static var unread: String { Words.text("Unread") }
    public static var replyPlaceholder: String { Words.text("Reply…") }
    public static var send: String { Words.text("Send") }
    public static var sending: String { Words.text("Sending") }
    public static var sent: String { Words.text("Sent") }
    public static var undo: String { Words.text("Undo") }
    public static func more(_ n: Int) -> String { n == 1 ? Words.text("1 more") : Words.text("%lld more", n) }
    public static func dropTitle(_ name: String) -> String { Words.text("New message with %@", name) }
    // Round 1 (board a). Three keys are new to the catalogue; their 14 translations: _captures/notch/r1/i18n/.
    public static var openInWhoosh: String { Words.text("Open in Whoosh") }
    public static var hiddenWhileSharing: String { Words.text("Hidden while you share your screen") }
    public static var now: String { Words.text("now") }
    public static var copy: String { Words.text("Copy") }
    public static var back: String { Words.text("Back") }
    public static var from: String { Words.text("From") }
    public static func unreadCount(_ n: Int) -> String { Words.text("%lld unread", n) }
    public static func attachments(_ n: Int) -> String { n == 1 ? Words.text("1 attachment") : Words.text("%@ attachments", n) }
}
