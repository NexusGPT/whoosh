#if os(macOS)
import SwiftUI

/// The attachments-mac lane's registration: a chip that holds the keys (selected, or Quick Look open) is asked first
/// for Space, Return, ← →, Escape, ⌘C and ⌘V; ⌘O comes through the command table as "open-links" (Superhuman v8:
/// "Open links & attachments"). Rows and clicks are the socket surface par-attachments drives (AttachmentSocket).
@MainActor
public enum AttachmentsMacFeature: FeatureModule, CommandProviding {
    public static let id = "attachments-mac"
    public static let surfaces: [String] = ["mac-main"]

    public static func hasFocus(_ m: AppModel) -> Bool { AttachmentKeys.holdsKeys(m) }

    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        AttachmentIntake.installOnce()
        return AttachmentKeys.bindings
    }

    public static func commands() -> [String: (AppModel) -> Bool] { ["open-links": AttachmentKeys.openLinks] }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        AttachmentIntake.installOnce()
        return AttachmentSocket.rows
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] { AttachmentSocket.clicks(.shared) }

    /// `drag <attachment> desktop` on a rig copy: the test copy's desktop (AttachmentDesk).
    public static func drop(from: String, onto: String, _ m: AppModel) -> Bool { onto == AttachmentDesk.target && AttachmentDesk.drop(from, m) }
}
#else
import SwiftUI

/// Off the Mac the attachments lane draws nothing and holds no keys: the registry still names it, so the type stays.
@MainActor
public enum AttachmentsMacFeature: FeatureModule {
    public static let id = "attachments-mac"
    public static let surfaces: [String] = []
}
#endif
