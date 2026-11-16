import SwiftUI

/// Words the mail view adds (the catalogue holds them in 14 languages; one name per thing).
enum MessageBodyWords {
    static var loading: String { Words.text("Loading the message…") }
    static var degraded: String { Words.text("This mail is shown as plain text.") }
    static var loadOnce: String { Words.text("Load once") }
    static func always(_ address: String) -> String { Words.text("Always for %@", address) }
    static var appColours: String { Words.text("App colours") }
    static var originalColours: String { Words.text("Original colours") }
    static func showRest(_ open: Bool) -> String { Words.text(open ? "Hide quoted text" : "Show quoted text") }
    static var pictureNotLoaded: String { Words.text("Picture not loaded") }
    static var picture: String { Words.text("Picture") }
    /// "3 pictures not loaded, 2 trackers blocked"; the trackers part only when there are some.
    static func picturesLine(pictures: Int, trackers: Int) -> String {
        if pictures == 0 { return trackers == 1 ? Words.text("1 tracker blocked") : Words.text("%@ trackers blocked", trackers) }   // trackers only: no picture to load
        let p = pictures == 1 ? Words.text("1 picture not loaded") : Words.text("%@ pictures not loaded", pictures)
        guard trackers > 0 else { return p }
        return Words.text("%1$@, %2$@", p, trackers == 1 ? Words.text("1 tracker blocked") : Words.text("%@ trackers blocked", trackers))
    }
    static var tryAgain: String { Words.text("Try again") }
    /// The same line after the owner asked for the pictures: "Loading pictures…" while they load (`missing` nil), then "3 pictures loaded" or
    /// "2 of 3 pictures could not be loaded"; the trackers part follows as before.
    static func picturesLine(asked: Int, missing: Int?, trackers: Int) -> String {
        let p: String
        if let missing {
            if missing == 0 { p = asked == 1 ? Words.text("1 picture loaded") : Words.text("%@ pictures loaded", asked) }
            else { p = asked == 1 ? Words.text("The picture could not be loaded") : Words.text("%1$@ of %2$@ pictures could not be loaded", missing, asked) }
        } else {
            p = Words.text("Loading pictures…")
        }
        guard trackers > 0 else { return p }
        return Words.text("%1$@, %2$@", p, trackers == 1 ? Words.text("1 tracker blocked") : Words.text("%@ trackers blocked", trackers))
    }
}

/// The body view's frame in the reader's scroll view (the pane's visible area is the origin), and the rule that says a mark is in view.
final class BodyPlace {
    var rect: CGRect = .null

    /// The point at share `y` of the body (0 its top, 1 its foot) stands in the pane: a line under the top edge, and above READER_MARK_IN_VIEW_PERCENT of the
    /// pane's height (the reply box floats over the foot). False while the frame is not known: the reader then scrolls as it did.
    func shows(_ y: Double, viewport: Double) -> Bool {
        guard !rect.isNull, rect.height > 0, viewport > 0 else { return false }
        let at = Double(rect.minY) + y * Double(rect.height)
        return at >= Double(WhooshSpacing.s16) && at <= viewport * Double(WhooshConstants.READER_MARK_IN_VIEW_PERCENT) / 100
    }
}

/// The open message's text inside its card, from the moment the mail is selected (A129 part 1): the whole body from the
/// local store at once. While the core holds no body yet the card shows the preview and one quiet line, and swaps to the
/// body in place when the core announces it (`ReaderBodies.revision`). The read is one store read on the settled move,
/// never on a passing key: the pane builds this view only for the message it settled on.
struct OpenBodyHost: View {
    let thread: ReaderThread
    /// The selected search result's passage when this mail holds it (A152): marked in the page; the page says where, and the reader scrolls there.
    var passage: ReaderPassage?
    @State private var bodies = ReaderBodies.shared  // local-ui: observed, a body that lands later redraws the card
    /// The card's body is built one turn AFTER the key's frame (Enter into the reader): the key frame draws the header, the summary and the card's frame with the body's
    /// first line of room (the height a body view starts with anyway, before its first measure), and the store read, the web view and its load follow in the next
    /// run-loop turn, after the commit. No animation, no flash: the web view is hidden until its page has loaded in either case. (docs/lanes/hid-list.md, round 23.)
    @State private var built = false
    /// Where the body stands in the reader's scroll view. A reference: the scroll writes it on every move and nothing is redrawn for it.
    @State private var place = BodyPlace()

    var body: some View {
        if !built {
            Color.clear.frame(height: WhooshTypography.body.lineHeight)
                .preference(key: BodySettleKey.self, value: .waiting)
                .onAppear {
                    ReaderPerf.shared.mark("body.deferred")
                    // one run-loop iteration later: the main-queue blocks of the key's own turn run before its commit, a timer of the next iteration after it
                    DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(1)) { built = true }
                }
        } else { builtBody }
    }

    @ViewBuilder private var builtBody: some View {
        let _ = bodies.revision
        let _ = ReaderPerf.shared.mark("body.host")   // the turn that evaluates the card's body (the store read of the settled move is inside it)
        if let content = thread.shownBody(AppModel.shared) {
            MessageBodyView(content: content, sender: thread.open.address, passage: passage?.page(in: thread, content: content),
                            marked: passage == nil ? nil : { [place] (y: Double) in
                                // A mark already in view moves nothing: the reader used to scroll to every mark, so a sentence in the middle of the pane still
                                // pushed the thread up and cut the summary's top (the lead's walk on main182, the word "quote").
                                if place.shows(y, viewport: ReaderScroll.shared.metrics.viewport) { return }
                                ReaderScroll.shared.show(ReaderScroll.blockId(thread.earlierCount), anchor: y)
                            },
                            quarantined: AppModel.shared.mailbox == .junk).id(content.id)
                .onGeometryChange(for: CGRect.self) { $0.frame(in: .scrollView(axis: .vertical)) } action: { place.rect = $0 }
        } else {
            Text(thread.preview)
                .whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: WhooshMetrics.readerMeasure, alignment: .leading)
                .textSelection(.enabled)
                .preference(key: BodySettleKey.self, value: .shown)      // no page to wait for
            Text(MessageBodyWords.loading)
                .whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityIdentifier("mac-main.reader.body.loading")
            if thread.open.trackers > 0 {
                Text(ReaderWords.trackers(thread.open.trackers))
                    .whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
            }
        }
    }
}
