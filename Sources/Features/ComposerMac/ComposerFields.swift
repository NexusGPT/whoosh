import SwiftUI

/// The places the keyboard can put the cursor in the composer. The order is the order on screen.
public enum ComposerField: String, CaseIterable, Sendable {
    case from, to, cc, bcc, subject, body
}

/// A key asked for this field. The serial makes a second request for the same field a change the view can see.
public struct ComposerFocusRequest: Equatable, Sendable {
    public let field: ComposerField
    public let serial: Int
}

/// The focus ring: one rounded tint behind the field that holds the cursor. It exists on exactly one field at a time, so
/// when the cursor moves the ring leaves one row and arrives on the next as one shape (matched geometry): it slides when
/// the rows are close and fades when they are far (docs/MOTION.md rule 11). The state changes on the key's frame; the
/// animation follows and gates nothing.
struct ComposerRing: View {
    @Bindable var model: ComposerModel
    let field: ComposerField
    let ns: Namespace.ID

    var body: some View {
        if model.ringField == field {
            RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous)
                .fill(WhooshPalette.mine)
                .opacity(field == .body ? 0 : 1)      // the message gets the outline only
                .overlay { RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous).strokeBorder(WhooshPalette.focusHalo, lineWidth: WhooshSpacing.s2) }
                .padding(.horizontal, -WhooshSpacing.s6)
                .matchedGeometryEffect(id: model.ringSlide ? "composer.ring" : "composer.ring." + field.rawValue, in: ns)
                .allowsHitTesting(false)
                .transition(.opacity)
        }
    }
}

extension ComposerModel {
    /// The ring's animation: a short spring when it slides, the hover fade when it fades, none under Reduced Motion
    /// (the ring then changes row at once).
    func ringAnimation(reduceMotion: Bool) -> Animation? {
        if reduceMotion { return nil }
        return ringSlide ? WhooshMotion.tabThumb : WhooshMotion.hover
    }

    /// The fields on screen, top to bottom.
    var visibleFields: [ComposerField] { [.from, .to] + (showCc ? [.cc] : []) + (showBcc ? [.bcc] : []) + [.subject, .body] }

    /// Slide when the two rows are at most SLIDE_MAX_ROWS apart; the first appearance of the ring fades in.
    static func ringSlides(from: ComposerField?, to: ComposerField, visible: [ComposerField]) -> Bool {
        guard let from, let a = visible.firstIndex(of: from), let b = visible.firstIndex(of: to) else { return false }
        return abs(a - b) <= WhooshConstants.SLIDE_MAX_ROWS
    }
}

/// Every control that carries a key, with the name and the key it shows. One table: the buttons build their tooltip from it
/// and the control socket's `mac-composer.controls` rows read it, so what the check reads is what the owner sees.
enum ComposerControlKeys {
    struct Entry { let id: String; let title: String; let key: String; let cap: Bool }

    static let entries: [Entry] = [
        Entry(id: ComposerIds.attach, title: "Attach a file", key: "⌘⇧A", cap: false),
        Entry(id: ComposerIds.from, title: "From", key: "⌘⇧F", cap: true),
        Entry(id: ComposerIds.showCc, title: "Cc", key: "⌘⇧C", cap: false),
        Entry(id: ComposerIds.showBcc, title: "Bcc", key: "⌘⇧B", cap: false),
        Entry(id: ComposerIds.send, title: "Send", key: "⌘↵", cap: true),
        Entry(id: ComposerIds.sendLater, title: "Send later", key: "⌘⇧L", cap: true),
        Entry(id: ComposerIds.remind, title: "Remind me if no reply", key: "⌘⇧H", cap: false),
        Entry(id: ComposerIds.sendDone, title: "Send and mark done", key: "⌘⇧↵", cap: true),
        Entry(id: ComposerIds.sendInstant, title: "Send now, no undo", key: "⌘⇧Z", cap: true),
        Entry(id: ComposerIds.snippets, title: "Snippets", key: "⌘;", cap: false),
        Entry(id: ComposerIds.discard, title: "Discard draft", key: "⌘⇧,", cap: false),
    ]

    /// The two window controls (round 3). They are drawn only while they can act (a reply can go back, a new message
    /// has no thread), so the control rows list them under the same condition.
    static let popIn = Entry(id: ComposerIds.popIn, title: "Pop in", key: ComposerWindowKeys.popIn, cap: true)
    static let thread = Entry(id: ComposerIds.thread, title: "Thread", key: ComposerWindowKeys.thread, cap: true)

    static func entry(_ id: String) -> Entry? { (entries + [popIn, thread]).first { $0.id == id } }

    /// "Attach a file  ⌘⇧A": the words, two spaces, the key.
    static func tooltip(_ title: String, _ key: String) -> String { Words.text(title) + "  " + key }
    static func tooltip(_ id: String) -> String { entry(id).map { tooltip($0.title, $0.key) } ?? "" }
}

/// The keys of the window controls, spelled once: the buttons, the tooltips and the control rows read them here.
enum ComposerWindowKeys {
    static let popIn = "⇧⌘P"
    static let thread = "⌘D"
}
