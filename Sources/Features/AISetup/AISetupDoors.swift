import Foundation

/// The fix of each need, in place or at its Settings pane. One function per need; a mark, the quick-start step and the socket call these.
@MainActor
enum AISetupDoors {
    /// the Settings pane's own door for a need whose fix is a choice (Jev key, consent, the engine row)
    /// The Settings window opens on that pane (hid-ai round 6: `select` alone chose the pane of a closed window, so "Create a rule from this
    /// message" left the surface on mac-main, main92 J5.9 arrives-by-control and decision-check J5.9/rule-from-message-prefilled).
    static func openSettings(_ pane: SettingsPane) {
        let model = SettingsWindowModel.shared
        _ = model.select(pane)
        _ = WindowBridge.open(model.pane.slot ?? .macSettingsGeneral, in: AppModel.shared)
    }

    /// Writing: Settings on AI and agents, the providers section, the first provider's key field focused (the socket's `type` goes there).
    static func openWritingKey() {
        let store = AISettingsStore.shared
        openSettings(.ai)
        store.section = .providers
        store.keyFocus = "anthropic"
    }

    static func fix(_ need: AINeed) {
        let status = AISetupStatus.shared
        switch need {
        case .writing: openWritingKey()
        case .meaning: status.downloadMeaning()
        case .decision:
            switch status.status(.decision).reason {
            case "laya-missing", "laya-failed": status.downloadLaya()
            default: openSettings(.decisions)
            }
        }
    }

    /// Cancel of a download in progress (decision model only: the meaning store has no cancel).
    static func cancel(_ need: AINeed) { if need == .decision { AISetupStatus.shared.cancelLaya() } }
}
