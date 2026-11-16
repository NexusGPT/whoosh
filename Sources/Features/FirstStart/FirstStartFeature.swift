import SwiftUI

/// The firststart lane: the first-start flow of J9 (docs/surfaces/mac-first-start.md): 1a Staged inbox, 2b Aurora stage, 3a Icon hero.
/// It is not a window of its own: it draws in the main window's overlay slot (`mac-main.overlay`), over the three empty panes, and
/// it holds the keys (Enter, Esc, ⌘[) only while it runs (`hasFocus`). It keeps the slot's other overlays (the palette, the trainer).
@MainActor
public enum FirstStartFeature: FeatureModule {
    public static let id = "firststart"
    static let X = "mac-first-start"
    static var m: FirstStartModel { .shared }

    /// Focus is the flow itself, until the add-account sheet takes the window (the skip state's Add account).
    public static func hasFocus(_ app: AppModel) -> Bool { m.active && app.surface == "mac-main" }

    public static func view(for slot: SurfaceSlot) -> AnyView? {
        slot == .macOverlay ? AnyView(FirstStartOverlayHost()) : nil
    }

    /// The keys the flow owns. Enter, Esc and ⌘[ are its buttons; J K E H are the step-3 try-it; every other key that would act on the
    /// inbox behind the veil is swallowed while the flow runs, so no press changes mail nobody can see.
    static let swallowed = ["C", "/", "Z", "U", "R", "A", "F", "S", "L", "G", "I", "⌘K", "⌘N", "Down", "Up", "Tab", "Shift+Tab"]

    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        var out: [String: (AppModel) -> Bool] = [
            "Enter": { _ in enter() },
            "Escape": { _ in m.active && m.skip() },
            "⌘[": { _ in m.back() },
        ]
        for k in ["J", "K", "E", "H"] { out[k] = { _ in m.tryKey(k) } }
        for k in swallowed { out[k] = { _ in m.active } }
        return out
    }

    /// Return: Continue (Finish at step 9); at step 6 the add card's own primary action while no account exists; in the skip state Add account.
    static func enter() -> Bool {
        guard m.active else { return false }
        if m.skipped { return m.addAccountFromEmptyInbox() }
        if m.step == FirstStartModel.addStep, m.accountCount == 0 { return AddAccountModel.shared.primaryAction() }
        return m.next()
    }

    public static func typeText(_ text: String, _ app: AppModel) -> Bool { m.tryType(text) }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] { FirstStartRows.sources() }
    public static func clickTargets() -> [String: (AppModel) -> Bool] { FirstStartRows.clicks() }
    public static func paletteCommands() -> [(title: String, key: String)] { [(title: FirstStartWords.paletteKey, key: "")] }
}
