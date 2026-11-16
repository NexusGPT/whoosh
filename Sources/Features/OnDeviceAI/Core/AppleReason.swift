import Foundation

/// Why Apple Intelligence cannot answer, with the one sentence the Settings pane shows for each reason.
/// The real backend and the rig's armed backend read these same sentences, so what a test sees is what an owner sees.
public enum AppleReason: String, CaseIterable, Sendable {
    case osTooOld = "os_too_old"
    case deviceNotEligible = "device_not_eligible"
    case notEnabled = "not_enabled"
    case modelNotReady = "model_not_ready"
    case other = "other"

    public var words: String {
        switch self {
        case .osTooOld: return Words.text("Apple Intelligence needs macOS 26 or iOS 26. Update this device to use the on-device model.")
        case .deviceNotEligible: return Words.text("This Mac does not support Apple Intelligence, so the on-device model is not available here.")
        case .notEnabled: return Words.text("Apple Intelligence is turned off. Turn it on in System Settings, then Apple Intelligence and Siri.")
        case .modelNotReady: return Words.text("Apple Intelligence is still downloading its model. Try again in a few minutes.")
        case .other: return Words.text("Apple Intelligence is not available on this Mac right now.")
        }
    }
}
