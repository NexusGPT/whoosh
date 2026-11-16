import SwiftUI
#if os(macOS)
import AppKit
#endif

/// Sources: the only place where sources are chosen (owner round 7: "Sources I fear there is duplicate, we shouldn't have that"). One card per
/// account with its mail folders as tiles you select; the folders of this Mac as tiles; one switch per calendar (calendars are connected in their
/// own pane); where a reply from one account searches.
struct SourcesPane: View {
    @State private var store = RepliesStore.shared
    private let P = RepliesAx.id("sources")

    var body: some View {
        let s = store.sources
        RepliesScaffold(page: .sources) {
            SettingsButton(title: RepliesWords.addSource, key: "⌘N", ax: "\(P).add") { RepliesFolderPicker.choose() }
                .keyboardShortcut("n", modifiers: .command)
        } content: {
            VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
                ForEach(s.accounts, id: \.accountId) { a in AccountSourcesCard(account: a) }
                if s.accounts.isEmpty { SettingsFootnote(text: RepliesWords.noAccount, ax: "\(P).no-account") }
            }
            .padding(.top, WhooshSpacing.s4)
            RepliesGroup(title: Words.text("On this Mac"), card: false) {
                SourceTileGrid {
                    ForEach(Array(s.local.enumerated()), id: \.element.source) { i, src in
                        SourceTile(source: src, ax: "\(P).local.\(i)")
                            .contextMenu {
                                Button(Words.text("Remove")) { if case .localFolder(let p) = src.source { store.removeFolder(p) } }
                                    .accessibilityIdentifier("\(P).local.\(i).remove")
                            }
                    }
                    AddFolderTile(ax: "\(P).add-folder")
                }
            }
            RepliesGroup(title: RepliesWords.calendars, link: (title: CalendarWords.openCalendars, ax: "\(P).calendars.open", action: { SettingsCalendarsMacFeature.openPane() })) {
                if s.calendars.isEmpty { SettingsRow(ax: "\(P).calendars.none", title: RepliesWords.noCalendar) { EmptyView() } }
                ForEach(s.calendars, id: \.calendarId) { c in
                    SettingsRow(ax: "\(P).calendar.\(c.calendarId)", title: c.name, caption: c.accountLabel, dot: settingsColor(hex: c.colour),
                                onTap: { store.toggleCalendar(c) }) {
                        SettingsSwitch(on: c.enabled, refusals: store.refusals)
                    }
                }
            }
            GroupedCard {
                SettingsRow(ax: "\(P).scope", title: RepliesWords.searchIn) {
                    SettingsSegmented(axPrefix: "\(P).scope", options: [(RetrievalScope.thisAccount, "this_account", RepliesWords.thisAccount),
                                                                         (RetrievalScope.allAccounts, "all_accounts", Words.text("All accounts"))],
                                      selection: s.scope) { store.setSearchScope($0) }
                }
            }
            .padding(.top, WhooshSpacing.s8)
        }
        .onAppear { RepliesEvents.start() }
        .onDisappear { RepliesEvents.stop() }
    }
}

/// The folder chooser of « Ajouter un dossier » and « Ajouter une source »: the system's open panel, folders only. The chosen path goes to the
/// core, which starts its sync and reports progress by event.
@MainActor
enum RepliesFolderPicker {
    #if os(macOS)
    /// The chooser while it is open: the socket reads that it opened, and its door closes it.
    private(set) static var panel: NSOpenPanel?
    #endif
    static var isOpen: Bool {
        #if os(macOS)
        return panel != nil
        #else
        return false
        #endif
    }

    static func choose() {
        #if os(macOS)
        guard panel == nil else { panel?.makeKeyAndOrderFront(nil); return }
        // a hidden rig never shows a system panel: the ask is recorded (`core state system-ui/save-panel`) and nothing opens
        guard HiddenRig.allowSystemUI("save-panel", "NSOpenPanel replies folder") else { return }
        let p = NSOpenPanel()
        p.canChooseDirectories = true
        p.canChooseFiles = false
        p.allowsMultipleSelection = false
        p.prompt = RepliesWords.addFolder
        panel = p
        p.begin { answer in
            let path = answer == .OK ? p.url.map { ($0.path as NSString).abbreviatingWithTildeInPath } : nil
            MainActor.assumeIsolated {
                panel = nil
                if let path { RepliesStore.shared.addFolder(path) }
            }
        }
        #endif
    }

    /// Closes the chooser without a folder (its own Cancel).
    static func cancel() {
        #if os(macOS)
        panel?.cancel(nil)
        panel = nil
        #endif
    }
}

/// A grid of tiles, three to a row (`.tiles`): a tile never narrower than a folder name and its tick.
struct SourceTileGrid<Content: View>: View {
    @ViewBuilder var content: Content
    /// The narrowest a folder tile may be: its icon, a name of about « Clients / Lorvan Hotels », its tick and its sides.
    static var tileMin: CGFloat { 180 }
    var body: some View {
        // hid-ai round 24 (main114 d58d8ad5 p8, sources 700x480: « Clients… » twice): as many equal columns of at least `tileMin` as the card holds (three in
        // the 720 pt column of 1280x820, as wide as before; two at 700x480); a name wraps to a second line rather than being cut (SourceTile).
        // Lead, 14 Nov 2026 (picture of main170 at 700x480): a tile whose name took two lines stood 13 pt taller than its neighbour, and the row's tiles were
        // centred on each other. The tiles of a row now share the row's height and start at one top edge (`SourceTileRows`).
        SourceTileRows(minWidth: Self.tileMin, spacing: WhooshSpacing.s8) { content }
    }
}

/// Equal columns of at least `minWidth`, as many as the width holds; the tiles of a row are as tall as the tallest of them.
struct SourceTileRows: Layout {
    var minWidth: CGFloat
    var spacing: CGFloat

    private func columns(_ width: CGFloat) -> (count: Int, width: CGFloat) {
        let n = max(1, Int((width + spacing) / (minWidth + spacing)))
        return (n, max(0, (width - CGFloat(n - 1) * spacing) / CGFloat(n)))
    }

    private func rowHeights(_ subviews: Subviews, _ c: (count: Int, width: CGFloat)) -> [CGFloat] {
        stride(from: 0, to: subviews.count, by: c.count).map { start in
            subviews[start..<min(start + c.count, subviews.count)].map { $0.sizeThatFits(ProposedViewSize(width: c.width, height: nil)).height }.max() ?? 0
        }
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? minWidth
        let heights = rowHeights(subviews, columns(width))
        return CGSize(width: width, height: heights.reduce(0, +) + spacing * CGFloat(max(0, heights.count - 1)))
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let c = columns(bounds.width)
        var y = bounds.minY
        for (row, height) in rowHeights(subviews, c).enumerated() {
            let start = row * c.count
            for (i, tile) in subviews[start..<min(start + c.count, subviews.count)].enumerated() {
                tile.place(at: CGPoint(x: bounds.minX + CGFloat(i) * (c.width + spacing), y: y), anchor: .topLeading, proposal: ProposedViewSize(width: c.width, height: height))
            }
            y += height + spacing
        }
    }
}

/// « + Ajouter un dossier »: the dashed tile at the end of the folders of this Mac.
struct AddFolderTile: View {
    let ax: String
    @State private var hover = false  // local-ui: hover tint only
    var body: some View {
        Button { RepliesFolderPicker.choose() } label: {
            VStack(spacing: 1) {
                Text("+").font(.system(size: 18, weight: .light))
                Text(RepliesWords.addFolder).whooshText(WhooshTypography.subhead)
            }
            .foregroundStyle(hover ? WhooshPalette.accent : WhooshPalette.secondaryLabel)
            .frame(maxWidth: .infinity, minHeight: 74, maxHeight: .infinity)
            .background(RoundedRectangle(cornerRadius: 9, style: .continuous).fill(WhooshPalette.hover).opacity(hover ? 1 : 0))
            .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: 1))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain).onHover { hover = $0 }
        .animation(.easeOut(duration: WhooshMotion.hoverTint), value: hover)
        .whooshAx(ax)
    }
}
