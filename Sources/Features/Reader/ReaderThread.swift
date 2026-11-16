import Foundation
import QuartzCore

/// The open conversation as the reader draws it, built from what the core serves: the shown message's row, `thread(threadId:)`
/// (subject, participants, account, message ids), `message(id)` for every message (root delta 6) and `body(messageId:)`
/// for the open one. Not served yet: the summary and the invitations.
struct ReaderThread: Equatable {
    struct Entry: Equatable {
        let index: Int
        let fact: ReaderFact?
        let folded: Bool
        var from: String? { fact?.from }
        var time: String? { fact?.time }
    }

    let threadId: String
    let messageId: String
    let accountId: String
    let subject: String
    /// "Name <address>" as the core serves them
    let participants: [String]
    /// Earlier messages, oldest first; nil where the core does not answer for that id.
    let earlier: [ReaderFact?]
    /// The open message's facts.
    let open: ReaderFact
    let attachmentCount: Int
    let messageCount: Int
    /// The open message's body from the core; nil while it is not downloaded (the preview shows instead).
    let body: MessageBodyContent?
    /// The thread was built before Enter (the list holds the keys). It is a reading view all the same: the body shows (A129).
    var resting = false
    /// `message(id)` answered for the open message (its tracker count is then the message's own).
    var openServed = true

    var participantNames: [String] { participants.map { $0.components(separatedBy: " <").first ?? $0 } }

    /// The people line as drawn: each person by name, the owner's own addresses as "You", once (lead, 13 Nov, real-engine walk on main121:
    /// after a reply the line read "Nadia Benali, owner@…", the owner's own address in full).
    func shownParticipants(own: Set<String>, you: String) -> [String] {
        var out: [String] = []
        for p in participants {
            let parts = p.components(separatedBy: " <")
            let address = (parts.count > 1 ? String(parts[1].dropLast()) : p).lowercased()
            let name = own.contains(address) ? you : (parts.first ?? p)
            if !out.contains(name) { out.append(name) }
        }
        return out
    }
    var earlierCount: Int { messageCount - 1 }
    var sender: String { open.from }
    var time: String { open.time }
    var preview: String { open.preview }
    var attachmentNames: [String] { open.attachments }

    /// Every message in reading order; the latest is open, the earlier ones folded unless the rule is open.
    func entries(earlierOpen: Bool) -> [Entry] {
        let before = (0..<earlierCount).map { Entry(index: $0, fact: $0 < earlier.count ? earlier[$0] : nil, folded: !earlierOpen) }
        return before + [Entry(index: earlierCount, fact: open, folded: false)]
    }

    @MainActor
    /// `withBody: false` builds the thread without reading its body: the pane draws the body itself (`OpenBodyHost`), so a
    /// build stays a facts-only read and the one body read is counted as a body read, never inside a build.
    static func make(_ m: AppModel, messageId: String?, resting: Bool = false, withBody: Bool? = nil) -> ReaderThread? {
        let withBody = withBody ?? !resting
        let began = CACurrentMediaTime()
        defer {
            let ms = (CACurrentMediaTime() - began) * 1000
            ReaderPerf.shared.noteMake(ms: ms, loadedBody: withBody)
            InteractionProbe.phaseAdd(.readerMake, ms: ms)   // the same two reads, no third
        }
        // the mail's own row, wherever it is (A148): a search result outside the loaded inbox rows is drawn too, never "No conversation open"
        guard let id = messageId, let r = m.knownRow(id) else { return nil }
        // An answer for another thread is not this thread's. The fixture core keys rows by message id and its
        // one thread by "thr-01", so the subject is the identity check until the ids agree (reported to the lead).
        let t = m.core.thread(threadId: r.threadId).flatMap { $0.id == r.threadId || $0.subject == r.subject ? $0 : nil }
        let ids = t?.messageIds.filter { $0 != r.messageId } ?? []
        let count = max(Int(r.threadCount), ids.count + 1, 1)
        let facts = ids.map { id in m.core.message(messageId: id).map(ReaderFact.init) }
        let served = m.core.message(messageId: r.messageId)
        let open = served.map(ReaderFact.init) ?? ReaderFact(row: r)
        return ReaderThread(
            threadId: r.threadId, messageId: r.messageId, accountId: t?.accountId ?? r.accountId,
            subject: t?.subject ?? r.subject, participants: t?.participants ?? [r.senderName],
            earlier: facts + Array(repeating: nil, count: max(0, count - 1 - facts.count)), open: open,
            attachmentCount: max(Int(r.attachmentCount), open.attachments.count), messageCount: count,
            body: !withBody ? nil : ReaderBodies.shared.body(m, r.messageId).map { content($0, trackers: served == nil ? Int($0.trackersBlocked) : open.trackers) },
            resting: resting, openServed: served != nil)
    }

    /// The open message's body for the pane: the one the build read, else the stored body (read once, kept by `ReaderBodies`);
    /// nil while the core holds none.
    @MainActor func shownBody(_ m: AppModel) -> MessageBodyContent? {
        if let body { return body }
        return ReaderBodies.shared.content(m, messageId) { Self.content($0, trackers: openServed ? open.trackers : Int($0.trackersBlocked)) }
    }

    /// The core's body as the body view takes it: sanitised HTML when there is some, else the plain text.
    /// The tracker count is the message's own (`message(id)`).
    static func content(_ b: MessageBody, trackers: Int) -> MessageBodyContent {
        let prepared = b.layout.map { layout in
            MessageBodyContent.PreparedBody(
                layout: layout == .designed ? .designed : .written, displayHtml: b.displayHtml, writtenHtml: b.writtenHtml,
                segmentHtml: b.segments.map(\.safeHtml), foldFrom: b.foldFrom.map(Int.init), warnings: b.bodyWarnings,
                pictures: Int(b.picturesBlocked), pixels: Int(b.pixelTrackers))
        }
        if let html = b.safeHtml, !html.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return .init(id: b.messageId, safeHtml: html, plainText: BodyText.plain(fromHTML: html), trackersBlocked: trackers, quoteOffset: b.quoteOffset.map(Int.init), prepared: prepared)
        }
        return .init(id: b.messageId, safeHtml: nil, plainText: b.plainText, trackersBlocked: trackers, quoteOffset: b.quotedRanges.first.map { Int($0.start) }, prepared: prepared)
    }
}
