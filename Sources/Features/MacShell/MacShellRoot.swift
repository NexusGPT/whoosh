import SwiftUI

/// The Mac main window: sidebar, list and reader in one split view, the sync line at the sidebar foot.
/// A pane a sibling lane has not delivered yet shows the shell's own quiet placeholder.
public struct MacShellRoot: View {
    @State private var model = AppModel.shared
    @State private var shell = MacShellState.shared
    @State private var clearance = TitleBandClearance.shared
    private let sizesWindow: Bool

    public init() { sizesWindow = true }
    init(sizesWindow: Bool) { self.sizesWindow = sizesWindow }

    public var body: some View {
        let _ = LaunchClock.once("shell-body")   // perf-archive r4: a launch phase
        #if os(macOS)
        // The panes come from the features that claimed the slots (lead edit, integration round): the shell's own
        // sidebar, list placeholder and reader placeholder stay as the fallback when nobody claimed a slot.
        MacSplitView(sidebar: AnyView(pane(.sidebar, firstControl: Self.sidebarFirstControl) { Registrations.view(for: .macSidebar).map { v in AnyView(ZStack { Group { if WhooshRigStandIn.isOn { Rectangle().fill(WhooshRigStandIn.underlay) } else { MacSidebarVibrancy() } }.ignoresSafeArea(); v }) } ?? AnyView(sidebar) }),
                     list: AnyView(pane(.list, firstControl: Self.listFirstControl) { Registrations.view(for: .macList) ?? AnyView(MacListPlaceholder(model: model, focused: shell.focusedPane == .list)) }),
                     reader: AnyView(pane(.reader) { (Registrations.view(for: .macReader) ?? AnyView(MacReaderPlaceholder()))
                         .safeAreaInset(edge: .bottom, spacing: 0) { Color.clear.frame(height: WhooshMetrics.footStrip) } }),   // the foot strip's room
                     state: shell, sizesWindow: sizesWindow)
            .frame(minWidth: MacShellGeometry.windowMinW, idealWidth: MacShellGeometry.windowW,
                   minHeight: MacShellGeometry.windowMinH, idealHeight: MacShellGeometry.windowH)
            .overlay(alignment: .bottomLeading) {   // the foot strip: the focused surface's keys, under the list and the reader
                MacFootStrip(model: model)
                    .padding(.leading, shell.sidebarCollapsed ? 0 : (shell.paneWidths[.sidebar] ?? 0) + WhooshMetrics.separator)
            }
            .overlay { MacHintOverlay() }   // ⌘ held: the key badges (zero work while ⌘ is not held)
            .ignoresSafeArea()
            .task { MacHintMonitor.shared.start() }
            .task(id: MacSystemBridges.totalUnread(model)) { MacSystemBridges.menuBar.show(unread: MacSystemBridges.totalUnread(model)) }
        #else
        EmptyView()
        #endif
    }

    private var sidebar: some View {
        ZStack(alignment: .bottomLeading) {
            #if os(macOS)
            // hidden rig: the opaque underlay (behind-window vibrancy draws nothing in a hidden shot); the owner's build: the real material
            Group { if WhooshRigStandIn.isOn { Rectangle().fill(WhooshRigStandIn.underlay) } else { MacSidebarVibrancy() } }.ignoresSafeArea()
            #endif
            Rectangle().fill(WhooshSidebarMaterial.tint).ignoresSafeArea()
            Rectangle().fill(WhooshSidebarMaterial.edge).frame(width: WhooshSidebarMaterial.edgeWidth)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .trailing).ignoresSafeArea()
            MacStatusLine(line: MacSyncLine.read(model)) { _ = MacShellActions.openAccounts(model) }
        }
    }

    /// Every pane: its ax-id, the shell's environment, and a click that gives it focus.
    /// Where a pane's first control sits below the pane's top: the sidebar's first header (SidebarView.contentTop 8 plus the header's own 8)
    /// and the list's tab strip (WhooshMetrics.toolbarTop 12). The pane clears the title bar's real band from these (TitleBandClearance).
    static let sidebarFirstControl: CGFloat = SidebarView.contentTop + WhooshSpacing.s8
    static let listFirstControl: CGFloat = WhooshMetrics.toolbarTop

    private func pane<V: View>(_ p: MacShellState.Pane, firstControl: CGFloat? = nil, @ViewBuilder _ content: () -> V) -> some View {
        content()
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .modifier(BandRoom(key: p.rawValue, firstControl: firstControl, extra: clearance.extra(p.rawValue)))
            .environment(\.macShellFocusedPane, shell.focusedPane)
            .environment(\.whooshTextScale, shell.textScale)
            .contentShape(Rectangle())
            .simultaneousGesture(TapGesture().onEnded { shell.focusedPane = p; shell.focusByKey = false })
            .overlay {   // the keyboard-focus ring: the app's focus halo, inset, only after a key moved the focus
                RoundedRectangle(cornerRadius: WhooshRadius.row, style: .continuous)
                    .strokeBorder(WhooshPalette.focusHalo, lineWidth: 2)
                    .padding(2)
                    .opacity(shell.focusByKey && shell.focusedPane == p ? 1 : 0)
                    .animation(.easeOut(duration: Double(WhooshConstants.HOVER_TINT_MS) / 1000), value: shell.focusByKey && shell.focusedPane == p)
                    .allowsHitTesting(false)
            }
            .accessibilityElement(children: .contain)
            .accessibilityIdentifier("mac-main.\(p.rawValue)")
    }
}

/// A pane whose first control could start above the title bar's band takes the missing points as top room (a safe-area inset, so the
/// pane's own backgrounds still fill under the bar). No room is added while the pane already clears the band.
private struct BandRoom: ViewModifier {
    let key: String
    let firstControl: CGFloat?
    let extra: CGFloat
    func body(content: Content) -> some View {
        #if os(macOS)
        if let firstControl {
            content
                .safeAreaInset(edge: .top, spacing: 0) { Color.clear.frame(height: extra) }
                .background(TitleBandAnchor(key: key, firstControl: firstControl))
        } else { content }
        #else
        content
        #endif
    }
}

/// Until the inboxlist lane merges: two-line rows on the locked ramp, the focused row on the selection fill.
struct MacListPlaceholder: View {
    let model: AppModel
    let focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 0) {
                ForEach(Array(model.rows.enumerated()), id: \.element.messageId) { i, r in
                    let sel = i == model.focusIndex
                    VStack(alignment: .leading, spacing: 0) {
                        Text(r.senderName).whooshText(WhooshTypography.body, emphasized: r.unread).lineLimit(1)
                        Text(r.subject).whooshText(WhooshTypography.subhead).lineLimit(1)
                            .foregroundStyle(sel && focused ? AnyShapeStyle(WhooshPalette.onAccent) : AnyShapeStyle(WhooshPalette.secondaryLabel))
                    }
                    .foregroundStyle(sel && focused ? AnyShapeStyle(WhooshPalette.onAccent) : AnyShapeStyle(WhooshPalette.label))
                    .padding(.leading, WhooshMetrics.listRowLeading - WhooshMetrics.listRowInset)
                    .padding(.trailing, WhooshSpacing.s12)
                    .frame(maxWidth: .infinity, minHeight: WhooshMetrics.listRow, alignment: .leading)
                    .background(sel ? AnyShapeStyle(focused ? WhooshPalette.selection : WhooshPalette.fill) : AnyShapeStyle(.clear),
                                in: RoundedRectangle(cornerRadius: WhooshRadius.row))
                    .padding(.horizontal, WhooshMetrics.listRowInset)
                    .accessibilityIdentifier("mac-main.list.row.\(i)")
                }
            }
            .padding(.top, WhooshMetrics.sidebarTop + WhooshMetrics.listHeaderHeight)
            Spacer(minLength: 0)
        }
        .clipped()
        .background(WhooshPalette.background)
    }
}

/// Until the reader lane merges: the board's empty reader (.rdempty).
struct MacReaderPlaceholder: View {
    var body: some View {
        Text("No message selected")
            .whooshText(WhooshTypography.body)
            .foregroundStyle(WhooshPalette.secondaryLabel)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(WhooshPalette.background)
    }
}
