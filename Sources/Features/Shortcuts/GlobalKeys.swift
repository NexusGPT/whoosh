import Foundation

/// System-wide keys (they work while another app is in front) go through the KeyboardShortcuts package by
/// Sindre Sorhus, licence MIT (https://github.com/sindresorhus/KeyboardShortcuts; named in docs/lanes/shortcuts.md).
/// The package is not in Package.swift yet (a frozen root file; see docs/lanes/shortcuts.md "Contract deltas"),
/// so the bridge compiles to a no-op until the root adds it. Single-key shortcuts inside the window never use it.
@MainActor
public enum GlobalKeys {
    nonisolated public static let showWhoosh = "show-whoosh"
    nonisolated public static let composeAnywhere = "compose-anywhere"

    /// True when the package is linked and the global keys are live.
    public static var available: Bool {
        #if canImport(KeyboardShortcuts)
        return true
        #else
        return false
        #endif
    }
}

#if canImport(KeyboardShortcuts) && os(macOS)
import KeyboardShortcuts

extension KeyboardShortcuts.Name {
    static let showWhoosh = Self(GlobalKeys.showWhoosh)
    static let composeAnywhere = Self(GlobalKeys.composeAnywhere)
}

extension GlobalKeys {
    /// Call once from the app shell. `model.press` is not used: a global key brings the window forward first.
    public static func start(model: AppModel = .shared) {
        KeyboardShortcuts.onKeyUp(for: .showWhoosh) {
            MainActor.assumeIsolated { model.surface = "mac-main"; WindowBridge.bringMainToFront() }
        }
        KeyboardShortcuts.onKeyUp(for: .composeAnywhere) {
            MainActor.assumeIsolated {
                WindowBridge.bringMainToFront()
                _ = ShortcutEngine.shared.click("compose", model)
            }
        }
    }
}
#endif
