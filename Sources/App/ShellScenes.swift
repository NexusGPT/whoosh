import SwiftUI
#if os(macOS)
import AppKit
#endif

/// The app's scenes, shared by the Mac and iPhone shells. A surface window is tagged with its surface id
/// (WindowSurfaceTag), so the socket's `shot <surface>` finds the window it opened (WindowBridge.window(for:)) whatever its title says.
public struct WhooshScene: Scene {
    public init() {}
    public var body: some Scene {
        let _ = LaunchClock.once("scene-body")   // perf-archive r3: a launch phase (one Bool read once the launch is over)
        WindowGroup("Whoosh", id: SurfaceSlot.macMain.id) { RootView() }
            #if os(macOS)
            .commands { SettingsCommand() }
            #endif
        #if os(macOS)
        Window(Words.text("Settings"), id: WindowBridge.settingsWindowId) { SettingsRoot().navigationTitle(Words.text("Settings")) }
        WindowGroup(id: WindowBridge.surfaceWindowId, for: String.self) { $surface in
            SurfaceWindow(slot: SurfaceSlot(surface ?? SurfaceSlot.macMain.id))
        }
        #endif
    }
}

#if os(macOS)
/// Tags the window this view sits in with the surface it draws (WindowBridge.tag), so a window is found by its surface and
/// its title bar can say anything.
struct WindowSurfaceTag: NSViewRepresentable {
    let surface: String
    /// A settings slot is never tagged as a screen window: it must not pass for (or hide) the Settings window.
    var isSettings = false
    func makeNSView(context: Context) -> TagView { TagView(surface, isSettings) }
    func updateNSView(_ v: TagView, context: Context) { v.surface = surface; v.isSettings = isSettings; v.apply() }
    final class TagView: NSView {
        var surface: String
        var isSettings: Bool
        init(_ surface: String, _ isSettings: Bool) { self.surface = surface; self.isSettings = isSettings; super.init(frame: .zero) }
        required init?(coder: NSCoder) { fatalError("not from a nib") }
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            apply()
            DispatchQueue.main.async { [weak self] in   // SwiftUI names and places the window after this view lands: tag again once it has, then its opening height
                guard let self else { return }
                self.apply()
                if let w = self.window, !self.isSettings { WindowBridge.sizeAtOpen(w, surface: self.surface) }
            }
        }
        func apply() { if let w = window, !isSettings { WindowBridge.tag(w, surface: surface) } }
    }
}
#endif

/// Any registered surface in a window of its own: the claiming feature's view, else its stub page.
struct SurfaceWindow: View {
    let slot: SurfaceSlot
    var body: some View {
        (Registrations.view(for: slot) ?? AnyView(SlotStub(slot)))
            // The title is the screen's name in the app's language, or nothing; never the surface id. The window is found by
            // the id it is tagged with (WindowBridge.surfaceId(of:)), not by what its title bar says.
            .navigationTitle(WindowBridge.title(for: slot))
            #if os(macOS)
            .background(WindowSurfaceTag(surface: slot.id, isSettings: WindowBridge.isSettingsSlot(slot)))
            #endif
    }
}

#if os(macOS)
/// ⌘, opens the Settings window on its current page (the app menu's "Settings…").
struct SettingsCommand: SwiftUI.Commands {
    @Environment(\.openWindow) private var openWindow
    var body: some SwiftUI.Commands {
        let _ = LaunchClock.once("commands-body")   // perf-archive r4: a launch phase
        CommandGroup(replacing: .appSettings) {
            Button(Words.text("Settings…")) { AppModel.shared.surface = AppModel.shared.settingsPage.id; openWindow(id: WindowBridge.settingsWindowId) }
                .keyboardShortcut(",", modifiers: .command)
        }
    }
}
#endif
