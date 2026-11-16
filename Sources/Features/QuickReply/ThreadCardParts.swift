import SwiftUI

/// The pieces every card of the thread's foot shares (board cards-s3 `.mine`): the head line (the sending account's
/// emblem, "Vous", the address, and the state slot at the right), the action pills (`.b1` filled accent, `.b2` quiet),
/// and the ids and words of the cards. The cards themselves are ThreadOutgoingCard (waiting · sending · sent · failed)
/// and ThreadDraftCard.
struct ThreadCardHead<Slot: View>: View {
    let account: AccountSummary?
    /// The address's frame id for the socket (the draft card's), nil for none.
    var addressAx: String? = nil
    @ViewBuilder let state: () -> Slot
    var body: some View {
        // hid-compose round 18 (board cards-s3, sending-960 and sent-1280): "Vous" with the state at its right on the first line, the
        // address WHOLE on a line of its own under it. Beside "Vous" the address was cut in the middle at 960 ("assem…ample.de", main111).
        HStack(alignment: .top, spacing: WhooshSpacing.s8) {
            if let a = account {
                AccountMark(name: a.name, address: a.address, colour: a.colour, icon: .providerDefault, size: .compact)
            }
            VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                HStack(alignment: .center, spacing: WhooshSpacing.s8) {
                    marked(Text(ThreadCardCopy.you).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.label), suffix: ".you")
                    Spacer(minLength: WhooshSpacing.s8)
                    state()
                }
                // The socket's id sits on the address once (ThreadCardHead `marked`). The whole line is the column's; the middle cut is
                // only the last resort of a column narrower than the address.
                if let a = account { marked(address(a.address)) }
            }
        }
        .lineLimit(1)
    }

    private func address(_ words: String) -> some View {
        Text(words).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1).truncationMode(.middle)
    }

    /// The head's ids hang off the card's: `<card>.address` (the chooser) and `<card>.you` (a plain text beside it, so a missing
    /// address frame is told from a head that is not drawn at all).
    /// Not `.whooshAx` here: the head is drawn inside the card's lift, whose landing light is a layer effect (AuroraLanding: `visualEffect`
    /// with `layerEffect`), and an NSView frame anchor inside a layer effect is never mounted (main95 to main100: no frame for the head's You
    /// nor its address, while the card's own id, outside the lift, had one). The bounds go out as a preference; the card marks them
    /// outside the lift (`threadHeadMarks`).
    @ViewBuilder private func marked<V: View>(_ v: V, suffix: String = "") -> some View {
        if let addressAx {
            let id = suffix.isEmpty ? addressAx : String(addressAx.dropLast(".address".count)) + suffix
            v.anchorPreference(key: ThreadHeadMarks.self, value: .bounds) { [id: $0] }
        } else { v }
    }
}

/// Where a card's text column starts: under "Vous", past the head's emblem (none drawn: at the card's edge).
enum ThreadCardInset {
    static func text(_ account: AccountSummary?) -> CGFloat { account == nil ? 0 : AccountMarkSize.compact.points + WhooshSpacing.s8 }
}

/// `.b1` / `.b2`: a 28 pt pill with the word and its key cap; `accent` fills it blue with white words.
struct ThreadCardPill: View {
    let word: String
    let cap: String
    var accent = false
    let ax: String
    var action: () -> Void
    var body: some View {
        Button(action: action) {
            HStack(spacing: WhooshSpacing.s6) {
                Text(word).whooshText(WhooshTypography.caption, emphasized: true)
                    .foregroundStyle(accent ? WhooshPalette.onAccent : WhooshPalette.label)
                if !cap.isEmpty { KeyCap(cap, onSelection: accent) }
            }
            .fixedSize()
            .padding(.horizontal, WhooshSpacing.s10).frame(height: WhooshMetrics.segmentHeight)
            .background(accent ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(WhooshPalette.fill), in: Capsule())
            .contentShape(Capsule())
            .modifier(HoverAck(radius: WhooshMetrics.segmentHeight / 2))
        }
        .buttonStyle(.plain)
        .help("\(word) (\(KeyCap.glyph(for: cap)))")
        // hid-compose round 18: not `.whooshAx`. Every pill sits inside its card's lift, whose landing light is a layer effect, and a frame
        // anchor (an NSView) mounted there is drawn by SwiftUI as its yellow "not supported here" box with the red no-entry mark: main111
        // shots, sending-*-960x632 and 1280x820, "Annuler Z" painted over. The id stays on the button; the frame goes out as a preference
        // and is marked outside the lift by the card (`threadHeadMarks`), as the head's address is.
        .accessibilityIdentifier(ax)
        .anchorPreference(key: ThreadHeadMarks.self, value: .bounds) { [ax: $0] }
    }
}

/// `.dot` the warning dot before "Non envoyé" and before the reason.
struct ThreadCardDot: View {
    var body: some View { Circle().fill(WhooshPalette.warning).frame(width: WhooshSpacing.s6, height: WhooshSpacing.s6).accessibilityHidden(true) }
}

enum ThreadCardIds {
    static let outgoingRegion = "mac-main.thread.outgoing"
    static func outgoing(_ i: Int) -> String { "mac-main.thread.outgoing.\(i)" }
    static func outgoingUndo(_ i: Int) -> String { "mac-main.thread.outgoing.\(i).undo" }
    static func outgoingRetry(_ i: Int) -> String { "mac-main.thread.outgoing.\(i).retry" }
    static func outgoingDiscard(_ i: Int) -> String { "mac-main.thread.outgoing.\(i).discard" }
    static func outgoingEdit(_ i: Int) -> String { "mac-main.thread.outgoing.\(i).edit" }
    static let draftRegion = "mac-main.thread.draft"
    static let draft = "mac-main.thread.draft.0"
    static let draftSend = "mac-main.thread.draft.0.send"
    static let draftDelete = "mac-main.thread.draft.0.delete"
    static let draftDeleted = "mac-main.thread.draft.deleted"
    static let draftUndo = "mac-main.thread.draft.deleted.undo"
    /// The room the thread keeps under its last card while the slab floats (a spacer, measured by the gap check).
    static let room = "mac-main.thread.room"
}

/// Every word of the cards, through the 14-language catalogue.
enum ThreadCardCopy {
    static var you: String { Words.text("You") }
    static func sendingIn(_ s: Int) -> String { Words.text("Sending in %@ s", "\(max(s, 0))") }
    static var sendingNow: String { Words.text("Sending…") }
    static var offline: String { Words.text("Sends when the network is back") }
    static var undo: String { Words.text("Cancel") }
    static let undoCap = "Z"
    static var undoHelp: String { Words.text("Undo (Z)") }
    static func sent(_ atMs: Int64) -> String {
        Words.text("Sent %@", Date(timeIntervalSince1970: Double(atMs) / 1000).formatted(date: .omitted, time: .shortened))
    }
    static var draft: String { Words.text("Draft") }
    static var send: String { Words.text("Send") }
    static let sendCap = QuickReplyCopy.sendCap   // one constant for every reply send control (⌘↵)
    static var delete: String { Words.text("Delete") }
    static let deleteCap = "⌘⌫"   // owner, 13 Nov: "cmd+del not just del it would clash"
    static var draftDeleted: String { Words.text("Draft deleted") }
    static var notSent: String { Words.text("Not sent") }
    static var retry: String { Words.text("Retry") }
    static var edit: String { Words.text("Edit") }
    /// ⌘ and one key (owner 2026-11-07: no two-modifier keys); free in every preset's reader and reply context.
    static let editCap = "⌘E"
    /// "<account>'s server refused the send. The text stays here as a draft." (board `.reason`)
    static func refused(_ account: String) -> String { Words.text("%@'s server refused the send. The text stays here as a draft.", account) }
    /// A send whose CONNECTION broke (the engine's code `network_<step>`): the server refused nothing. While the engine tries again by itself the
    /// card says so and when; after its last try the words are the draft's (lead, 13 Nov, owner picture 204: "server refused the send" stood over
    /// "Connection reset by peer").
    static func tryingAgainIn(_ s: Int) -> String { Words.text("Trying again in %@ s", "\(max(s, 0))") }
    /// The server ANSWERED with a try-later code (4xx) and the engine tries again by itself (owner, 13 Nov, picture 208: Microsoft 365's 451).
    static func laterRetrying(_ account: String) -> String { Words.text("%@'s mail server asked to try later. Whoosh tries again by itself.", account) }
    static func brokeRetrying(_ account: String) -> String { Words.text("The connection to %@'s mail server broke. Whoosh tries again by itself.", account) }
    static func broke(_ account: String) -> String { Words.text("The connection to %@'s mail server broke. The text stays here as a draft.", account) }
    static var retryNow: String { Words.text("Retry now") }
}

/// The head's marked texts as anchors, carried out of the card's lift (see `ThreadCardHead.marked`).
struct ThreadHeadMarks: PreferenceKey {
    static var defaultValue: [String: Anchor<CGRect>] { [:] }
    static func reduce(value: inout [String: Anchor<CGRect>], nextValue: () -> [String: Anchor<CGRect>]) { value.merge(nextValue()) { $1 } }
}

extension View {
    /// Applied OUTSIDE the card's lift: one clear view on each marked bound of the head, with its id, only while the socket measures
    /// (`FrameProbe.isMeasuring`); otherwise nothing is drawn or laid out.
    func threadHeadMarks() -> some View {
        overlayPreferenceValue(ThreadHeadMarks.self) { marks in Group {
            if FrameProbe.isMeasuring {
                GeometryReader { g in
                    ForEach(marks.keys.sorted(), id: \.self) { k in
                        if let a = marks[k] {
                            let r = g[a]
                            Color.clear.frame(width: r.width, height: r.height).position(x: r.midX, y: r.midY).whooshAx(k)
                        }
                    }
                }
                .allowsHitTesting(false)
            }
        } }
    }
}
