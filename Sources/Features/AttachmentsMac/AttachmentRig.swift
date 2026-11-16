#if os(macOS)
import Foundation

/// The rig's switches for this feature, read only under the rig (WHOOSH_SOCKET_DIR set), never on the owner's copy.
/// WHOOSH_ATTACH_MUTANT names one defect to re-introduce, so par-attachments can be seen red on it with one build:
/// no-panel · no-follow · no-step · no-open · no-thumbnail · no-drag · no-wait · no-intake · no-keys · menu-without-<item>
/// · picture-as-tile (a picture pasted as data becomes a tile, not a picture in the text) · paste-to-temp (a pasted picture is written to the system's temporary folder) · keep-pasted (a pasted picture's file outlives its tile)
/// · no-collapse · no-first-frame · no-inline (the gallery) · remove-button-dead (the pending file's xmark door) · size-when-absent (the strip prints a size for files that are not here) · absent-tile (a file that is not here draws the big tile) · cap-when-hovered (an absent chip shows its cap on hover, not only selected).
/// WHOOSH_ATTACH_OPEN=launch lets Open launch the real app; by default the rig records what would open and with which
/// app, so a test never puts Preview on the owner's screen.
enum AttachmentRig {
    static var mutant: String { AttachmentPlaces.isRig ? (AttachmentPlaces.env["WHOOSH_ATTACH_MUTANT"] ?? "") : "" }
    static func mutant(_ name: String) -> Bool { mutant == name }
    /// Never on a hidden rig copy: another app's window is system UI the owner would see.
    @MainActor static var launchesApps: Bool {
        !HiddenRig.isOn && (!AttachmentPlaces.isRig || AttachmentPlaces.env["WHOOSH_ATTACH_OPEN"] == "launch")
    }
}
#endif
