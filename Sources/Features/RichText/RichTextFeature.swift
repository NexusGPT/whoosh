import Foundation

/// The richtext lane's registrations. The editor state is one shared session (`RichTextFeature.session`),
/// so the Mac composer, the quick reply and later the iPhone composer all edit the same kind of draft.
public enum RichTextFeature: FeatureModule {
    public static let id = "richtext"

    /// The shared editor state of the composer on screen.
    @MainActor public static let session: RichTextSession = {
        let s = RichTextSession(undoDepth: WhooshConstants.UNDO_DEPTH)
        RichTextFixture.emptyOnReset(s)
        return s
    }()

    @MainActor
    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        ["mac-composer.runs": { _ in session.document.rows }]
    }

    @MainActor
    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        ["⌘Z": { _ in session.undoRewrite() }]
    }
}
