import Foundation

/// One mail the notch shows: what the push feed dropped, plus the preview line read from the inbox.
public struct NotchItem: Equatable, Identifiable, Sendable {
    public var id: String
    public var threadId: String
    public var accountId: String
    public var sender: String
    public var subject: String
    public var preview: String
    public var code: String?
    public var read = false
    public var codeCopied = false
    /// The sender's address (the open mail's second line) and when the mail arrived (the row's time; "now" when fresh).
    public var address = ""
    public var receivedAt = Date()
    /// The kind the engine gives the mail (contracts `notch_kind_of`: sign-in code, VIP, needs reply, updates, promotions, primary). The alert
    /// settings give each kind its style (NotchModel.arrive).
    public var kind: NotchKind = .primary
    /// The mail came from the notch's search, not from an arrival: it is acted on like any mail of the notch (open, archive, read, snooze) and it is
    /// never part of the Latest and Unread lists, the unread count or an alert.
    public var found = false

    /// "now" inside NotchMetrics.nowWindowS of the arrival, else the list's own clock words (today HH:mm, this week the day).
    public func timeWords(now: Date = Date()) -> String {
        now.timeIntervalSince(receivedAt) < NotchMetrics.nowWindowS ? NotchCopy.now : RowFacts.time(Int64(receivedAt.timeIntervalSince1970 * 1000), now: now)
    }
}

/// Where the panel sits. `notch`: it continues the camera housing; `none`: a panel under the menu bar.
public enum NotchShape: String, Sendable { case notch, none }

/// What the panel shows, smallest first. Each step only grows the panel in place.
public enum NotchStage: Equatable, Sendable {
    case rest          // nothing to show: an invisible hover strip over the housing
    case light         // a new mail as a light: the housing's lower edge in the account's colour, no words
    case line          // a new mail as one line beside the camera: who, and the subject
    case drop          // the newest mail has just dropped out
    case list          // the pointer rests on it: the latest mail, newest first
    case open(String)  // one mail open in place, with the quick reply under it
}
