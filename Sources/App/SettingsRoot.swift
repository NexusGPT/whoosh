import SwiftUI

/// The Settings window: the settingswindow lane's sidebar-and-cards window (slot `mac-settings-window`, mounted through
/// the registry like every other view). Until a feature claims the slot, the page stub shows, so the window exists from day one.
public struct SettingsRoot: View {
    @State private var model = AppModel.shared
    @State private var language = Words.selected   // settingsgeneral hook
    public init() {}

    public var body: some View {
        (Registrations.view(for: SurfaceSlot("mac-settings-window")) ?? AnyView(SlotStub(SurfaceSlot("mac-settings-window"))))
            .id(language)
            .onReceive(NotificationCenter.default.publisher(for: Words.languageDidChange)) { _ in language = Words.selected }
            .onDisappear { if model.surface.hasPrefix("mac-settings") { model.surface = SurfaceSlot.macMain.id } }
    }
}
