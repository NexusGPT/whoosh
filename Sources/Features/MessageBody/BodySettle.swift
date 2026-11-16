import SwiftUI
import Observation

/// What the open mail's text says to the blocks under it (the card's files, the thread's cards, the reply line): `.waiting` while the text's page is on
/// its way and its height is not known, `.shown` once its first height is given. A mail shown by its preview has no page: it says `.shown` at once.
/// Why: the text starts with one line of room, so the blocks under it stood right under the header and moved down when the text came. Measured on
/// main188 (the lead's probe, 20 moves): an ordinary mail has its height 45 ms after the key and nothing is seen; a 1300 pt page mail kept the one line
/// from about 45 to 155 ms; the first mail after launch from 290 to 384 ms.
public enum BodySettle: Equatable, Sendable { case none, waiting, shown }

public struct BodySettleKey: PreferenceKey {
    public static let defaultValue: BodySettle = .none
    public static func reduce(value: inout BodySettle, nextValue: () -> BodySettle) {
        let n = nextValue()
        if n == .waiting || value == .none { value = n }      // a text still waiting wins over any other
    }
}

/// The mail whose blocks under the text are shown now. The reader's conversation writes it from the text's own word (`BodySettleKey`), and after
/// BODY_UNDER_TEXT_WAIT_MS at the latest, so a page that never answers hides nothing for good. A check reads it beside the text's box.
@MainActor @Observable
public final class ReaderUnderText {
    public static let shared = ReaderUnderText()
    public private(set) var shownFor: String?

    public func show(_ messageId: String) {
        guard shownFor != messageId else { return }
        var now = Transaction(); now.disablesAnimations = true      // they appear in place: no fade, no slide, nothing added to the open
        withTransaction(now) { shownFor = messageId }
    }
    public func isShown(_ messageId: String) -> Bool { shownFor == messageId }
}

/// Draws what it is put on only once the open mail's text has its height. Opacity: the room is kept and nothing moves when it shows. A modifier, so the
/// view under it is not built again when the answer changes, and the conversation that holds it is not evaluated again either.
public struct UnderTextShown: ViewModifier {
    let messageId: String
    public init(messageId: String) { self.messageId = messageId }
    public func body(content: Content) -> some View { content.opacity(ReaderUnderText.shared.isShown(messageId) ? 1 : 0) }
}
