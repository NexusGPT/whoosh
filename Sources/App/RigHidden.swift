import Foundation

/// `uistate.hidden` on every platform: the rig's hidden mode (Sources/App/HiddenRig.swift, Mac only) and what it refused.
@MainActor
public enum RigHidden {
    public static var state: [String: Any] {
        #if os(macOS)
        return ["on": HiddenRig.isOn, "refused": HiddenRig.refused, "system_ui": HiddenRig.systemUI.mapValues(\.count)]
        #else
        return ["on": false, "refused": [String](), "system_ui": [String: Int]()]
        #endif
    }
}
