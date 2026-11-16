import SwiftUI

/// A place in the shell a feature can fill with its own view (FeatureModule.view(for:)).
/// The id is the surface id of docs/DESIGN.md, or `<surface>.<pane>` for a pane inside a surface.
public struct SurfaceSlot: Hashable, Sendable, CustomStringConvertible {
    public let id: String
    public init(_ id: String) { self.id = id }
    public var description: String { id }

    /// The Mac main window: the macshell lane's root (sidebar, list and reader live inside it).
    public static let macMain = SurfaceSlot("mac-main")
    /// Panes inside the Mac main window, mounted by the shell that claims `macMain`.
    public static let macSidebar = SurfaceSlot("mac-main.sidebar")
    public static let macList = SurfaceSlot("mac-main.list")
    public static let macReader = SurfaceSlot("mac-main.reader")
    /// Drawn over the whole main window (hints, coach marks); empty content lets every click through.
    public static let macOverlay = SurfaceSlot("mac-main.overlay")
    /// The Settings window's pages, in tab order (⌘,).
    public static let macSettingsGeneral = SurfaceSlot("mac-settings-general")
    public static let macSettingsAccounts = SurfaceSlot("mac-settings-accounts")
    public static let macSettingsAI = SurfaceSlot("mac-settings-ai")
    /// The MCP server's page: Settings > MCP (the McpNotesMac pane, drawn here until a lane claims this slot).
    public static let macSettingsMcp = SurfaceSlot("mac-settings-mcp")
    public static let settingsPages: [SurfaceSlot] = [.macSettingsGeneral, .macSettingsAccounts, .macSettingsAI, .macSettingsMcp]

    public var isSettingsPage: Bool { Self.settingsPages.contains(self) }
}

/// The page a slot shows while no feature claims it: its id in words, so a shot says what is missing.
public struct SlotStub: View {
    let slot: SurfaceSlot
    public init(_ slot: SurfaceSlot) { self.slot = slot }
    public var body: some View {
        VStack(spacing: 6) {
            Text(slot.id).font(.headline).whooshAx("\(slot.id).stub.title")
            Text("Not built yet: no feature claims this slot.").foregroundStyle(.secondary).whooshAx("\(slot.id).stub.line")
        }
        .padding()
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .whooshAx("\(slot.id).stub")
    }
}
