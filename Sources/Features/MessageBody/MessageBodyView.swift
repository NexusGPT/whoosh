import SwiftUI
import WebKit
import Observation

/// What a body view tells the outside (the control-socket probe reads it): what is shown and what the
/// live web view reports about its own safety settings.
@MainActor @Observable
public final class MessageBodyModel {
    public var content: MessageBodyContent
    public var quotedOpen = false
    /// The one-line note under the pictures line: shown after a click (the control socket reads and flips it too).
    public var explainOpen = false
    /// B3: the colours shown now. `startColours` is how the document was built; ⇧I flips `colours` and the page follows with a class, no reload.
    public var colours: BodyColours
    public fileprivate(set) var startColours: BodyColours
    /// The sender's address (Always for it); empty when the mail carries none.
    public fileprivate(set) var sender: String
    /// A129 part 3: this viewing's pictures were asked for (Load once, or the sender is on the Always list).
    public fileprivate(set) var picturesAsked: Bool
    /// What the page says about the pictures it asked for, after Load once or Always (journey finding 63, 13 Nov 2026: the line kept saying "3 pictures not
    /// loaded" with the pictures on screen, and offered nothing when none came). nil while the pictures document loads; then how many pictures the page asked a
    /// server for and how many of those did not come.
    public struct PicturesReport: Equatable, Sendable { public let asked: Int; public let failed: Int }
    public fileprivate(set) var picturesReport: PicturesReport?
    /// The sender is on the Always list (observed: the line drops its "Always for" once it is).
    public fileprivate(set) var alwaysOn: Bool
    fileprivate var reloadPictures: () -> Void = {}
    /// The owner asked for this viewing's pictures himself (Load once, Always); false when the rule loaded them (on open, or an Always sender).
    public fileprivate(set) var picturesByOwner = false
    /// The mail is read in Junk: its pictures ask first whatever the rule says.
    public fileprivate(set) var quarantined: Bool
    /// Pictures of this mail the page can ask a server for (an address was kept); the others stay boxes whatever is asked.
    public fileprivate(set) var askable: Int
    /// Pictures a second try can still bring: the ones that did not come and the ones not asked for although they have an address. nil while they load.
    public var picturesRetryable: Int? { picturesReport.map { $0.failed + max(0, askable - $0.asked) } }
    /// The one-line note under the pictures line.
    public var picturesNote: String {
        PicturesPolicy.loadsOnOpen && !quarantined ? Words.text("Pictures load when you open a mail. Tracking pixels are never loaded.")
                                                    : Words.text("Images from the web are never loaded. Nothing leaves this Mac.")
    }
    /// Pictures of this mail that are not on screen after the ask: the ones that did not come and the ones the page could not ask for. nil while they load.
    public var picturesMissing: Int? { picturesReport.map { max(0, content.blockedPictures - $0.asked) + $0.failed } }
    /// The pictures line's sentence in its state: what was blocked; after the ask "Loading pictures…", then what loaded or how many could not be loaded.
    public var picturesSentence: String? {
        guard let blocked = content.blockedLabel else { return nil }
        guard picturesAsked, content.blockedPictures > 0 else { return blocked }
        if !picturesByOwner {
            // Loaded by the rule: pictures on screen need no sentence. The line speaks when a second try can bring a picture, and of the trackers it blocked.
            if let retry = picturesRetryable, retry > 0, let missing = picturesMissing {
                return MessageBodyWords.picturesLine(asked: content.blockedPictures, missing: missing, trackers: content.pixelTrackers)
            }
            return content.pixelTrackers > 0 ? MessageBodyWords.picturesLine(pictures: 0, trackers: content.pixelTrackers) : nil
        }
        return MessageBodyWords.picturesLine(asked: content.blockedPictures, missing: picturesMissing, trackers: content.pixelTrackers)
    }
    public fileprivate(set) var height: CGFloat = WhooshTypography.body.lineHeight
    /// Where the written text ends in the page: the three-dot pill sits 10 pt under it, folded or open (F2).
    public fileprivate(set) var mainBottom: CGFloat = 0
    public fileprivate(set) var javaScriptAllowed: Bool?
    public fileprivate(set) var blockListInstalled = false
    fileprivate(set) var measured = false
    /// This model's document has finished loading in the web view (false while a load is on its way). A measure taken before that reads a blank page, or
    /// the page of the mail the pooled view showed before: it must not count as the first measure (`measured`), or the real height then unfolds
    /// under the reader's eyes at every open (lead, 13 Nov, hidden probe on main145: the view's height grew from 32 to 166 over 0.7 s after a load).
    @ObservationIgnored fileprivate(set) var pageLoaded = false
    fileprivate var applyState: () -> Void = {}
    func setQuoted(_ open: Bool) { applyState() }
    func setColours(_ c: BodyColours) { applyState() }
    fileprivate var remeasure: () -> Void = {}
    /// The passage of a search result to mark in this page (nil: none), and what the page says after it was marked (share of the page's height where the first
    /// mark stands). The app's own script marks it after each load and when the passage changes; the reader scrolls to it.
    fileprivate(set) var passage: PassageMark.Page?
    @ObservationIgnored var onMarked: ((Double) -> Void)?
    @ObservationIgnored fileprivate(set) var marksMade = 0
    @ObservationIgnored fileprivate var markedOnce = false
    fileprivate var applyPassage: () -> Void = {}
    func setPassage(_ p: PassageMark.Page?) { passage = p; applyPassage() }
    /// What the web view and its layers say now (frame, window, hidden, filters up the layer chain): read by the socket row `web`.
    fileprivate(set) var webFacts: () -> [String: Any] = { [:] }
    /// What the page itself holds: characters of text the web process laid out (-1 until a measure answered), and counters of what happened to the view.
    /// NOT observed, all three: they are a test's probe (the socket row `web` reads them), never a view's input. `load` runs inside
    /// `updateNSView` and counts in `stats`; while `stats` was observed, that write invalidated the view it was updating, so the mail view
    /// redrew without end (main78: 100 % of a core and 2 GB, the sample stood at `BodyCoordinator.load`, this file's `stats["updates"] += 1`).
    @ObservationIgnored fileprivate(set) var textLength = -1
    @ObservationIgnored fileprivate(set) var measureAnswered = false
    /// What the PAGE itself says it draws, read back from the web view on a test copy only (lead, 13 Nov, owner picture 198: every check read the
    /// generated text, and a style rule that matched nothing stayed unseen): the page's class, the body's left padding, the fill of the first
    /// blocked-picture tile, and whether the app's and the sender's document are shown. Empty until the first measure of a test copy.
    @ObservationIgnored fileprivate(set) var drawn: [String: String] = [:]
    @ObservationIgnored var stats = ["attaches": 0, "loads": 0, "finishes": 0, "terminations": 0, "measures": 0, "measure_errors": 0, "updates": 0, "rescues": 0]
    /// A146, the fit (NOT observed, as above: the probe reads them; `height` is the one value the view follows). The content's natural
    /// width and the page's width as the last measure read them (0 before one answered), the scale shown, and the owner's pinch on top of the fit.
    @ObservationIgnored fileprivate(set) var naturalWidth: CGFloat = 0
    @ObservationIgnored fileprivate(set) var paneWidth: CGFloat = 0
    @ObservationIgnored fileprivate(set) var scale: CGFloat = 1
    @ObservationIgnored var zoom: CGFloat = 1
    var fitScale: CGFloat { WideMailFit.scale(content: naturalWidth, pane: paneWidth) }
    var scrollsHorizontally: Bool { WideMailFit.scrollsHorizontally(content: naturalWidth, pane: paneWidth, scale: scale) }

    /// One measure's answer, at the page's own size: sets the scale the page must show and the height the reader gives the view (the
    /// laid-out height at that scale, so nothing is cut at the bottom and no empty band is left). Returns the scale.
    @discardableResult func fit(natural: CGFloat, pane: CGFloat, layoutHeight: CGFloat, layoutMainBottom: CGFloat) -> CGFloat {
        naturalWidth = natural; paneWidth = pane
        let s = WideMailFit.shown(fit: fitScale, zoom: zoom)
        scale = s
        if layoutHeight > 0 { let h = WideMailFit.height(layout: layoutHeight, scale: s); if abs(height - h) > 0.5 { height = h } }
        let bottom = (layoutMainBottom * s).rounded(.up)
        if abs(mainBottom - bottom) > 0.5 { mainBottom = bottom }
        return s
    }
    /// TRUE only when text is on screen as far as this process can tell without a screenshot: the view is in a window, shown, not under a hidden
    /// or filtered layer, has a size, and the web process answered a measure just now with text in the page. Ask for a fresh measure first (`refresh`).
    var painted: Bool {
        let f = webFacts()
        let wants = !content.plainText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        return (f["in_window"] as? Bool) == true && (f["hidden"] as? Bool) == false && (f["ancestor_hidden"] as? Bool) == false
            && (f["alpha"] as? Double) == 1 && (f["width"] as? Double ?? 0) > 0 && (f["height"] as? Double ?? 0) > 0
            && (f["layer_filters"] as? Int) == 0 && (f["loading"] as? Bool) == false && measureAnswered && (!wants || textLength > 0)
    }
    func refresh() { remeasure() }

    /// A caller's own rule for this viewing's pictures (the notch's full mail: owner, 14 Nov 2026, "Full email load pictures by default", with its
    /// own switch in Settings); nil = the reader's rule (the General setting, the Always list, Junk asks first).
    fileprivate var picturesOverride: Bool?

    public init(content: MessageBodyContent, sender: String = "", quarantined: Bool = false, pictures: Bool? = nil) {
        self.content = content
        self.sender = sender
        self.quarantined = quarantined
        self.picturesOverride = pictures
        self.askable = MessageBodyPictures.askable(html: content.prepared?.displayHtml ?? content.safeHtml)
        let start = BodyColours.start(for: content.layout)
        self.colours = start
        self.startColours = start
        self.alwaysOn = PicturesAlways.contains(sender)
        self.picturesAsked = content.blockedPictures > 0 && (pictures ?? ((PicturesPolicy.loadsOnOpen && !quarantined) || PicturesAlways.contains(sender)))
    }

    /// A new body for the same view: every state starts again from the new mail.
    func adopt(_ new: MessageBodyContent, sender newSender: String? = nil, quarantined newQuarantined: Bool? = nil) {
        content = new; quotedOpen = false; explainOpen = false; zoom = 1; naturalWidth = 0; paneWidth = 0; scale = 1
        if let newSender { sender = newSender }
        if let newQuarantined { quarantined = newQuarantined }
        colours = BodyColours.start(for: new.layout); startColours = colours
        askable = MessageBodyPictures.askable(html: new.prepared?.displayHtml ?? new.safeHtml)
        picturesAsked = new.blockedPictures > 0 && (picturesOverride ?? ((PicturesPolicy.loadsOnOpen && !quarantined) || PicturesAlways.contains(sender)))
        picturesReport = nil; picturesByOwner = false; alwaysOn = PicturesAlways.contains(sender)
    }

    /// ⇧I: the other colours for this mail. A plain-text mail has only one way, so the key passes on.
    @discardableResult func switchColours() -> Bool {
        guard content.safeHtml != nil else { return false }
        colours = colours.flipped
        return true
    }
    /// O or a click on the dots: everything that is not the sender's own text opens, or folds back.
    @discardableResult func toggleFold() -> Bool {
        guard content.hasQuote else { return false }
        quotedOpen.toggle()
        return true
    }
    /// I or a click on "Load once": this viewing's pictures load; the block list of this one document is lifted.
    @discardableResult func loadOnce() -> Bool {
        guard content.blockedPictures > 0, !picturesAsked else { return false }
        picturesReport = nil
        picturesByOwner = true
        picturesAsked = true
        return true
    }
    /// "Always for <address>": the sender is remembered (locally, see `PicturesAlways`) and this viewing loads too.
    @discardableResult func loadAlways() -> Bool {
        guard content.blockedPictures > 0, !sender.isEmpty, !PicturesAlways.contains(sender) else { return false }
        PicturesAlways.add(sender)
        alwaysOn = true
        if !picturesAsked { picturesReport = nil; picturesByOwner = true; picturesAsked = true }
        return true
    }
    /// I or a click on "Try again", after pictures did not come: the same document loads again.
    @discardableResult func retryPictures() -> Bool {
        guard picturesAsked, let retry = picturesRetryable, retry > 0 else { return false }
        picturesReport = nil
        reloadPictures()
        return true
    }
    /// The Always list changed outside the line (the drive's forget door): the line follows.
    func syncAlways() { alwaysOn = PicturesAlways.contains(sender) }
}

/// The message body: sanitised HTML in a script-less web view, remote pictures blocked and counted (the pictures line above it),
/// everything that is not the sender's own text behind one three-dot pill. A written mail is drawn in the app's colours; a
/// designed one keeps its sender's on a white sheet with rounded corners (B3). The reader embeds this and gives it `.id(content.id)`.
public struct MessageBodyView: View {
    @State private var model: MessageBodyModel
    @Environment(\.colorScheme) private var scheme
    @Environment(\.openURL) private var openURL
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var settled = true
    private let gate = BodyWebGate.shared

    private let incoming: MessageBodyContent

    private let incomingPassage: PassageMark.Page?
    private let marked: ((Double) -> Void)?
    private let quarantined: Bool
    /// false: the caller says what happened to the pictures in its own line (the notch's full mail), so the reader's line stays out.
    private let picturesLine: Bool

    /// `passage`: what to mark in the page, when the mail is a search result's (A152); `marked` hears where the first mark stands (share of the height).
    public init(content: MessageBodyContent, sender: String = "") { self.init(content: content, sender: sender, passage: nil, marked: nil) }

    /// `quarantined`: the mail is read in Junk, where pictures ask first whatever the rule says.
    init(content: MessageBodyContent, sender: String, passage: PassageMark.Page?, marked: ((Double) -> Void)?, quarantined: Bool = false) {
        incoming = content
        incomingPassage = passage
        self.marked = marked
        self.quarantined = quarantined
        picturesLine = true
        let m = MessageBodyModel(content: content, sender: sender, quarantined: quarantined)
        m.passage = passage; m.onMarked = marked
        _model = State(initialValue: m)
    }

    /// The notch's full mail: the page with this viewing's pictures loaded or not as the notch's own setting says, and no pictures line of the reader's.
    init(content: MessageBodyContent, sender: String, pictures: Bool) {
        incoming = content
        incomingPassage = nil
        marked = nil
        quarantined = false
        picturesLine = false
        _model = State(initialValue: MessageBodyModel(content: content, sender: sender, quarantined: false, pictures: pictures))
    }

    /// The designed mail's paper: the sender's colours, white in light and dark.
    private var sheet: Bool { model.colours == .original && model.content.safeHtml != nil }
    private static let paperRing = Color.black.opacity(0.25)

    public var body: some View {
        let content = model.content
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            if picturesLine, let label = model.picturesSentence { PicturesLine(model: model, label: label) }
            if content.degraded {
                Text(MessageBodyWords.degraded).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                    .accessibilityIdentifier("mac-main.reader.body.degraded")
            }
            Group {
                if gate.isOpen {
                    BodyWebView(model: model,
                                html: MessageBodyDocument.html(for: content, quotedOpen: false, scheme: scheme, colours: model.startColours, loadPictures: model.picturesAsked),
                                fallback: MessageBodyDocument.textOnly(for: content, scheme: scheme),
                                loadPictures: model.picturesAsked,
                                onOpen: { openURL($0) })
                } else {
                    Color.clear      // the web view is built after the first frame (BodyWebGate), never inside the first layout
                }
            }
                .frame(height: model.height, alignment: .top)
                .whooshAx("mac-main.reader.body")   // the body's box, for a picture's `frames` and `measure` (no reader is mounted at rest)
                .background { if sheet { Color.white } }
                .clipShape(RoundedRectangle(cornerRadius: sheet ? WhooshRadius.row : 0, style: .continuous))
                .overlay { if sheet { RoundedRectangle(cornerRadius: WhooshRadius.row, style: .continuous).strokeBorder(Self.paperRing, lineWidth: WhooshMetrics.separator) } }
                .overlay(alignment: .topLeading) {
                    if content.hasQuote { DotsPill(model: model).offset(y: model.mainBottom + WhooshSpacing.s10).opacity(model.measured ? 1 : 0) }
                }
                .opacity(settled ? 1 : 0)   // opacity only: a blur filter over a web view can keep a stale, blank layer when the page paints during the fade (main76 blank body)
                .animation(model.measured ? WhooshMotion.animation(WhooshMotion.unfold, reduceMotion: reduceMotion) : nil, value: model.height)
                .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { _ in model.remeasure() }
        }
        .animation(WhooshMotion.animation(WhooshMotion.unfold, reduceMotion: reduceMotion), value: model.explainOpen)
        .animation(WhooshMotion.animation(WhooshMotion.unfold, reduceMotion: reduceMotion), value: model.quotedOpen)
        .frame(maxWidth: sheet ? .infinity : MessageBodyMetrics.measure, alignment: .leading)
        .preference(key: BodySettleKey.self, value: model.measured ? .shown : .waiting)      // the blocks under the text wait for its first height
        .onAppear { MessageBodyProbe.shared.show(model); fadeIn(); gate.arm() }
        .onChange(of: incoming) { _, new in model.adopt(new, quarantined: quarantined); fadeIn() }
        .onChange(of: incomingPassage) { _, new in model.onMarked = marked; model.setPassage(new) }
        .onChange(of: model.quotedOpen) { _, open in model.setQuoted(open) }
        .onChange(of: model.colours) { _, c in model.setColours(c) }
    }

    /// The new body is in place at once (state first); only its look settles in. No completion handler.
    private func fadeIn() {
        var t = Transaction(animation: nil)
        t.disablesAnimations = true
        // The hidden test copy's window is never in front: it gets no display ticks, so a time-based fade can be left at a few percent in its pictures (main111/112: the body as a
        // ghost under a fully drawn card). It takes the body at once; a window on screen keeps the fade, which finishes by the clock on the next frame it is given.
        #if os(macOS)   // the hidden test copy exists on the Mac only (the iPhone build of 15 Nov did not compile here)
        if HiddenRig.isOn { withTransaction(t) { settled = true }; return }
        #endif
        withTransaction(t) { settled = false }
        withAnimation(WhooshMotion.animation(WhooshMotion.readerSwap, reduceMotion: false)) { settled = true }
    }
}

/// The pictures line (drawn above the body when pictures were blocked): what was blocked, then "Load once" with its key cap I, then
/// "Always for <address>". Nothing loads before the owner asks. The sentence is a button: it opens the one-line note under it.
struct PicturesLine: View {
    let model: MessageBodyModel
    let label: String
    private var sentence: some View {
        Button { model.explainOpen.toggle() } label: { Text(label).fixedSize(horizontal: false, vertical: true) }
            .buttonStyle(PressTint()).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
            .accessibilityIdentifier("mac-main.reader.trackers")
    }

    private var actions: some View {
        HStack(spacing: WhooshSpacing.s8) {
            if !model.picturesAsked && model.content.blockedPictures > 0 {
                Button { model.loadOnce() } label: { Text(MessageBodyWords.loadOnce).lineLimit(1) }
                    .buttonStyle(PressTint()).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.accent)
                    .accessibilityIdentifier("mac-main.reader.pictures.load-once")
                KeyCap("I")
                if !model.sender.isEmpty { always }
            } else if model.picturesAsked, let retry = model.picturesRetryable {
                // after the ask (journey finding 63): pictures a second try can bring offer the load again; a load the owner asked for, and that worked, offers to keep the sender
                if retry > 0 {
                    Button { model.retryPictures() } label: { Text(MessageBodyWords.tryAgain).lineLimit(1) }
                        .buttonStyle(PressTint()).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.accent)
                        .accessibilityIdentifier("mac-main.reader.pictures.retry")
                    KeyCap("I")
                } else if model.picturesByOwner, model.picturesMissing == 0, !model.sender.isEmpty, !model.alwaysOn {
                    always
                }
            }
        }
    }

    private var always: some View {
        Button { model.loadAlways() } label: { Text(MessageBodyWords.always(model.sender)).lineLimit(1).truncationMode(.middle) }
            .buttonStyle(PressTint()).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.accent)
            .accessibilityIdentifier("mac-main.reader.pictures.always")
    }

    /// One row when it fits (as drawn); else the sentence whole and the actions under it: the line is never cut.
    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s4) {
            ViewThatFits(in: .horizontal) {
                HStack(spacing: WhooshSpacing.s8) { sentence.lineLimit(1); Spacer(minLength: WhooshSpacing.s8); actions }
                VStack(alignment: .leading, spacing: WhooshSpacing.s4) { sentence; actions }
            }
            if model.explainOpen {
                Text(model.picturesNote)
                    .whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                    .transition(.opacity)
            }
        }
    }
}

/// F2: the one pill under the written text, with its key cap O. One press opens everything that is not the sender's own text in
/// place; a second folds it. The text above never moves (the page keeps the pill's room folded or open).
struct DotsPill: View {
    let model: MessageBodyModel
    /// The key of "expand message" in the preset in force (never a literal): the cap and the tooltip show it, none when the preset has no key for it.
    static var key: String? { MacHintTable.keyLabel("expand-message") }
    var body: some View {
        Button { model.toggleFold() } label: {
            HStack(spacing: WhooshSpacing.s8) {
                Text("\u{2022}\u{2022}\u{2022}").font(.system(size: WhooshTypography.caption.size + 1, weight: .bold)).tracking(1.5)
                if let key = DotsPill.key { KeyCap(key) }
            }
            .foregroundStyle(WhooshPalette.secondaryLabel)
            .padding(.horizontal, WhooshSpacing.s8 + 1)
            .frame(height: WhooshMetrics.keyCap)
            .background(WhooshPalette.fill, in: Capsule())
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .help(MessageBodyWords.showRest(model.quotedOpen) + (DotsPill.key.map { "  " + $0 } ?? ""))
        .accessibilityLabel(MessageBodyWords.showRest(model.quotedOpen))
        .whooshAx("mac-main.reader.body.dots")
    }
}

/// When the first `WKWebView` may be built. Creating it costs about 77 ms (`WebProcessPool.create` 52 of them, measured by launch-r1 in
/// the first layout, before the first-frame stamp). The first body view that appears arms the gate; the gate opens two main-queue turns
/// later, and `RootView.onAppear` stamps `firstFrameMs` in ONE main-queue turn queued during that same appearance pass, so the web view
/// is always created after the stamp whichever of the two appearances is delivered first. After it opens, a body view builds its web
/// view in the same layout as before (the pool is unchanged): only the very first body of the process waits, two turns.
@MainActor @Observable
final class BodyWebGate {
    static let shared = BodyWebGate()
    private(set) var isOpen = false
    private var armed = false

    /// The first body of the process used to pay for the first web view (about 77 ms: main100 J1.6, `update.queue` 110 ms on the Enter that opened the first thread).
    /// One second after the list is on screen, with the main thread idle, one web view is built and put in the pool, so that first open finds it there. The first-frame
    /// stamp is long past by then (the launch budget is untouched); a second call does nothing; a pool that already holds a view keeps it.
    private var warmed = false
    func warmUp() {
        guard !warmed else { return }
        warmed = true
        MessageBodyPolicy.startRules()   // the one lookup or compile of the block list, now, off the key's path: every body view awaits this single result
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
            BodyWebGate.shared.isOpen = true
            BodyWebPool.shared.prewarm { BodyWebView.fresh() }
        }
    }

    func arm() {
        guard !isOpen, !armed else { return }
        armed = true
        DispatchQueue.main.async { DispatchQueue.main.async { self.isOpen = true; LaunchClock.mark("body-web-gate-open") } }   // a launch phase (perf-archive r2)
    }
}

/// A press tints by one step on the same frame: no animation on the press itself.
struct PressTint: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .padding(.horizontal, WhooshSpacing.s6)
                        .background { RoundedRectangle(cornerRadius: WhooshRadius.item).fill(WhooshPalette.fill).opacity(configuration.isPressed ? 1 : 0) }
            .padding(.horizontal, -WhooshSpacing.s6)
            .contentShape(Rectangle())
    }
}

/// Owns the web view: waits for the block list, loads, measures the height. If the block list cannot be
/// installed it loads the escaped text only, never the HTML. The document that carries pictures the owner asked for
/// gets the pictures list instead (images over https only), for that one document.
@MainActor
final class BodyCoordinator {
    let nav = BodyNavigationGuard()
    private var loaded = ""
    private var lastDocument = ""
    /// Set when the view is given back to the pool: a load still waiting for the rule list must not touch a view another card now holds.
    private var released = false
    func release() { released = true }
    /// The load that last asked, and the one that last began: a document begins once, by the rule list's answer or, when that does not come in time, by the watchdog.
    private var askedToken = 0
    private var begunToken = 0
    /// The document the view was last given: the owner asked for its pictures (`askedPictures`), and it is the pictures document itself, not the text-only
    /// fallback (`picturesDocument`). Read when a load finishes, to ask the page what came.
    private var askedPictures = false
    private var picturesDocument = false
    /// How long a document waits for the block list to compile before it loads as plain text (the list's compile is the one wait in a body's way: main111/112 left a body unrevealed
    /// 16 s). Proposed ledger row BODY_RULES_WAIT_MS = 5000 (docs/CONSTANTS.md is generated: the lead adds the row and the constant).
    private static let rulesWaitMs = WhooshConstants.BODY_RULES_WAIT_MS   // docs/CONSTANTS.md

    func attach(_ view: WKWebView, model: MessageBodyModel) {
        view.navigationDelegate = nav
        model.stats["attaches", default: 0] += 1
        model.webFacts = { [weak view] in view?.diagnose() ?? [:] }
        nav.onTerminate = { [weak self, weak view, weak model] in
            guard let self, let view, let model else { return }
            model.stats["terminations", default: 0] += 1
            self.reloadLast(in: view, model: model)
        }
        model.javaScriptAllowed = MessageBodyPolicy.javaScriptAllowed(in: view)
        nav.onFinish = { [weak self, weak view, weak model] in
            guard let view, let model else { return }
            model.stats["finishes", default: 0] += 1
            model.pageLoaded = true
            view.setShown(true)
            let pictures = self?.picturesDocument ?? false, asked = self?.askedPictures ?? false
            Task {
                await Self.apply(view, model)
                if pictures { await Self.pictures(view, model) }
                else if asked, model.picturesAsked, model.picturesReport == nil { model.picturesReport = .init(asked: 0, failed: 0) }   // the text-only fallback loaded: no picture was asked for
                await Self.mark(view, model)
            }
        }
        model.reloadPictures = { [weak self, weak view, weak model] in
            guard let self, let view, let model else { return }
            self.reloadLast(in: view, model: model)
        }
        // The page's own state (folded part, colours) follows the model with the app's script (page script stays off); after a reload too.
        model.applyState = { [weak view, weak model] in
            guard let view, let model else { return }
            Task { await Self.apply(view, model) }
        }
        model.applyPassage = { [weak view, weak model] in
            guard let view, let model else { return }
            Task { await Self.mark(view, model) }
        }
        model.remeasure = { [weak view, weak model] in
            guard let view, let model else { return }
            Task { await Self.measure(view, model) }
        }
        #if os(macOS)
        // A146: a pooled view starts each mail unzoomed with no sideways scroll; a pinch or a two-finger double tap re-runs the measure once.
        if let p = view as? PassThroughWebView {
            p.scrollsSideways = false
            p.onPinch = { [weak view, weak model] m in
                guard let view, let model else { return }
                model.zoom = WideMailFit.zoom(after: m, from: model.zoom, fit: model.fitScale)
                Task { await Self.measure(view, model) }
            }
            p.onSmartZoom = { [weak view, weak model] in
                guard let view, let model else { return }
                model.zoom = WideMailFit.smartZoom(from: model.zoom, fit: model.fitScale)
                Task { await Self.measure(view, model) }
            }
        }
        #endif
    }

    func load(_ html: String, fallback: String, loadPictures: Bool, in view: WKWebView, model: MessageBodyModel, onOpen: @escaping (URL) -> Void) {
        ReaderPerf.shared.mark("web.load")   // the web view takes a document: the cost of the load itself is in this turn
        nav.onOpenLink = onOpen
        model.stats["updates", default: 0] += 1
        // The pane was rebuilt with this same page: nothing reloads, so nothing would show the view again. A hidden, idle view is shown at once.
        if html == loaded, !view.isLoading { model.pageLoaded = true }   // this very page is already in the view: a measure reads it
        if html == loaded, !view.isLoading, !view.isVisibleAndIdle { view.setShown(true); Task { await Self.apply(view, model) } }
        guard html != loaded else { return }
        model.pageLoaded = false
        loaded = html
        askedPictures = loadPictures
        askedToken += 1
        let token = askedToken
        // A bounded fallback that needs no callback of a window: if the list has not answered, the mail loads as its text-only document (nothing remote can load in it: its
        // CSP is default-src 'none') and the view is revealed by its own finish, so a body is never left hidden behind a compile that does not return.
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(Self.rulesWaitMs)) { [weak self, weak view, weak model] in
            guard let self, let view, let model, !self.released, self.askedToken == token, self.begunToken != token else { return }
            self.begunToken = token
            model.blockListInstalled = false
            model.stats["rules-timeouts", default: 0] += 1
            self.lastDocument = fallback
            self.picturesDocument = false
            model.stats["loads", default: 0] += 1
            view.loadHTMLString(fallback, baseURL: nil)
        }
        Task {
            let pool = BodyWebPool.shared
            var listInstalled = pool.hasRules(view)
            if loadPictures {
                if !pool.carriesPicturesList(view), MessageBodyPolicy.replace(rules: await MessageBodyPolicy.picturesRuleList(), on: view) {
                    pool.markPicturesList(view, true); pool.markRules(view)
                }
                listInstalled = pool.carriesPicturesList(view)
            } else if pool.carriesPicturesList(view) {
                // a pooled view that served pictures goes back to the default list before any other mail loads
                pool.markPicturesList(view, false)
                listInstalled = MessageBodyPolicy.replace(rules: await MessageBodyPolicy.ruleList(), on: view)
            } else if !listInstalled, MessageBodyPolicy.install(rules: await MessageBodyPolicy.ruleList(), on: view) {
                pool.markRules(view)
                listInstalled = true
            }
            guard !released, begunToken != token else { return }   // the watchdog already began this document as text
            begunToken = token
            model.blockListInstalled = listInstalled
            self.lastDocument = listInstalled ? html : fallback
            self.picturesDocument = loadPictures && listInstalled
            model.stats["loads", default: 0] += 1
            view.loadHTMLString(self.lastDocument, baseURL: nil)
        }
    }

    /// The web process ended (memory, a crash): the view would stay blank with its height kept. Load the same document again.
    func reloadLast(in view: WKWebView, model: MessageBodyModel) {
        guard !lastDocument.isEmpty, !released else { return }
        model.stats["loads", default: 0] += 1
        model.pageLoaded = false
        view.loadHTMLString(lastDocument, baseURL: nil)
    }

    /// Sets the page to the model's state (folded part, colours), then reads the laid-out height.
    private static func apply(_ view: WKWebView, _ model: MessageBodyModel) async {
        let js = "document.documentElement.className='\(model.colours.rawValue)';var q=document.getElementById('q');if(q)q.hidden=\(!model.quotedOpen);"
        _ = try? await view.evaluateJavaScript(js)
        await measure(view, model)
    }

    /// Marks the search result's passage in the loaded page with the app's own script (`PassageMark.script`), or takes the earlier marks off; when a mark
    /// stands, tells the reader where (after the height was measured, so the scroll lands on the final layout). Runs after a load and when the passage changes,
    /// never per frame; a page that was never marked and has no passage costs nothing.
    private static func mark(_ view: WKWebView, _ model: MessageBodyModel) async {
        guard model.passage != nil || model.markedOnce else { return }
        let r = try? await view.evaluateJavaScript(PassageMark.script(model.passage))
        let v = (r as? [NSNumber])?.map(\.doubleValue) ?? []
        model.marksMade = Int(v.first ?? 0)
        model.markedOnce = model.passage != nil
        if model.marksMade > 0 { model.onMarked?(v.count > 1 ? min(max(v[1], 0), 1) : 0) }
    }

    /// After the pictures document loaded (its load event has fired, so every picture it asked for is settled): how many pictures the page asked a server for
    /// and how many of those are not drawn. The line then says what loaded, or how many could not be loaded with "Try again". The app's own script.
    private static func pictures(_ view: WKWebView, _ model: MessageBodyModel) async {
        guard model.picturesAsked, model.content.blockedPictures > 0 else { return }
        let r = try? await view.evaluateJavaScript(picturesScript)
        guard let v = (r as? [NSNumber])?.map(\.intValue), v.count == 2, v[0] >= 0 else { return }   // not the pictures document: its own finish answers
        let report = MessageBodyModel.PicturesReport(asked: v[0], failed: v[1])
        if model.picturesReport != report { model.picturesReport = report }
    }

    /// [pictures asked for, pictures that did not come]; [-1, 0] when the page is not a pictures document (`data-wp` on its body). A picture still on its
    /// way (a lazy one) is not counted as failed.
    private static let picturesScript = "(function(){var b=document.body;if(!b||b.getAttribute('data-wp')!=='1')return [-1,0];var l=document.images,n=0,f=0;for(var i=0;i<l.length;i++){var s=l[i].getAttribute('src')||'';if(!/^https?:/i.test(s))continue;n++;if(l[i].complete&&l[i].naturalWidth===0&&l[i].naturalHeight===0)f++}return [n,f]})()"

    /// Reads the laid-out height, where the written text ends, how much text the page holds and (A146) the content's natural width
    /// against the page's width. A mail wider than the pane is scaled to fit it in the page itself (`WideMailFit.script`: no reload), then
    /// the reader's height follows the scaled page. Runs after a load and on a width change only, never per frame. The app's own script
    /// runs even though page script is off.
    private static func measure(_ view: WKWebView, _ model: MessageBodyModel) async {
        model.stats["measures", default: 0] += 1
        let r = try? await view.evaluateJavaScript(WideMailFit.measureScript)
        if let v = (r as? [NSNumber])?.map(\.doubleValue), v.count == 6 {
            let s = WideMailFit.shown(fit: WideMailFit.scale(content: v[3], pane: v[4]), zoom: model.zoom)
            let side = WideMailFit.scrollsHorizontally(content: v[3], pane: v[4], scale: s)
            #if os(macOS)
            let pass = view as? PassThroughWebView
            let sideBefore = pass?.scrollsSideways ?? false
            #else
            let sideBefore = side
            #endif
            // the page first, then the height: no frame where the reader cuts the mail or leaves a band under it
            if abs(s - v[5]) > 0.0005 || side != sideBefore { _ = try? await view.evaluateJavaScript(WideMailFit.script(scale: s, sideways: side)) }
            if model.pageLoaded, !model.measured, !BodyOpenMutant.unfolds {
                // The first height of a loaded mail is given at once, in the same update that marks it measured: the unfold motion is for a height that
                // changes under the reader's eyes (the folded part opened, pictures arrived), never for the open itself.
                var now = Transaction(); now.disablesAnimations = true
                withTransaction(now) {
                    model.fit(natural: v[3], pane: v[4], layoutHeight: v[0], layoutMainBottom: v[1])
                    model.measured = true
                }
            } else {
                if BodyOpenMutant.unfolds { model.measured = true }   // the broken copy: measured and the first height land in one update, with animations on
                model.fit(natural: v[3], pane: v[4], layoutHeight: v[0], layoutMainBottom: v[1])
            }
            #if os(macOS)
            pass?.scrollsSideways = side
            #endif
            model.textLength = Int(v[2]); model.measureAnswered = true
        } else {
            model.stats["measure_errors", default: 0] += 1; model.measureAnswered = false
        }
        #if os(macOS)
        if HiddenRig.isOn, let s = try? await view.evaluateJavaScript(MessageBodyPolicy.drawnScript) as? String {
            let v = s.components(separatedBy: "|")
            // quote_bars: the rules drawn left of the first quote of the folded part (the part's own rule and each quote level's): 1 for one level
            if v.count == 7 { model.drawn = ["class": v[0], "pad_left": v[1], "tile_fill": v[2], "app_document": v[3], "sender_document": v[4], "page_fill": v[5], "quote_bars": v[6]] }
        }
        #endif
        if model.pageLoaded || BodyOpenMutant.unfolds { model.measured = true }   // a measure of a page still on its way is not the first measure
    }

}

#if os(macOS)
/// Hands wheel events to the reader's scroll view: the body is as tall as its text, so it never scrolls itself up or down. A146: a
/// zoomed mail wider than the pane keeps its sideways strokes (`scrollsSideways`, set by the measure); a pinch zooms the mail, applied
/// once when the gesture ends (one measure per pinch, never one per frame).
final class PassThroughWebView: WKWebView {
    var scrollsSideways = false
    var onPinch: (CGFloat) -> Void = { _ in }
    var onSmartZoom: () -> Void = {}
    private var pinch: CGFloat = 0
    override func scrollWheel(with event: NSEvent) {
        if scrollsSideways, abs(event.scrollingDeltaX) > abs(event.scrollingDeltaY) { super.scrollWheel(with: event) } else { nextResponder?.scrollWheel(with: event) }
    }
    override func magnify(with event: NSEvent) {
        pinch += event.magnification
        guard event.phase.isEmpty || event.phase.contains(.ended) || event.phase.contains(.cancelled) else { return }
        let m = pinch; pinch = 0
        if m != 0 { onPinch(m) }
    }
    override func smartMagnify(with event: NSEvent) { onSmartZoom() }
}

struct BodyWebView: NSViewRepresentable {
    let model: MessageBodyModel, html: String, fallback: String, loadPictures: Bool, onOpen: (URL) -> Void
    func makeCoordinator() -> BodyCoordinator { BodyCoordinator() }
    func makeNSView(context: Context) -> WKWebView {
        let v = BodyWebPool.shared.checkout { Self.fresh() }
        context.coordinator.attach(v, model: model)
        return v
    }
    /// One new body web view: what `makeNSView` builds, and what the idle warm-up puts in the pool (BodyWebGate.warmUp).
    static func fresh() -> WKWebView {
        let w = PassThroughWebView(frame: .zero, configuration: MessageBodyPolicy.makeConfiguration())
        w.setValue(false, forKey: "drawsBackground")
        return w
    }
    static func dismantleNSView(_ v: WKWebView, coordinator: BodyCoordinator) { coordinator.release(); BodyWebPool.shared.checkin(v) }
    func updateNSView(_ v: WKWebView, context: Context) {
        context.coordinator.load(html, fallback: fallback, loadPictures: loadPictures, in: v, model: model, onOpen: onOpen)
    }
}
#else
struct BodyWebView: UIViewRepresentable {
    let model: MessageBodyModel, html: String, fallback: String, loadPictures: Bool, onOpen: (URL) -> Void
    func makeCoordinator() -> BodyCoordinator { BodyCoordinator() }
    func makeUIView(context: Context) -> WKWebView {
        let v = BodyWebPool.shared.checkout { Self.fresh() }
        context.coordinator.attach(v, model: model)
        return v
    }
    static func fresh() -> WKWebView {
        let w = WKWebView(frame: .zero, configuration: MessageBodyPolicy.makeConfiguration())
        w.isOpaque = false
        w.backgroundColor = .clear
        w.scrollView.isScrollEnabled = false
        return w
    }
    static func dismantleUIView(_ v: WKWebView, coordinator: BodyCoordinator) { BodyWebPool.shared.checkin(v) }
    func updateUIView(_ v: WKWebView, context: Context) {
        context.coordinator.load(html, fallback: fallback, loadPictures: loadPictures, in: v, model: model, onOpen: onOpen)
    }
}
#endif
