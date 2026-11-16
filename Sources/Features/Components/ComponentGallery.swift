import SwiftUI
import Observation

public enum GalleryPage: String, CaseIterable, Sendable {
    case overview, brand, suggest, snooze, snoozedate, labels, undo, status, keys, empty, marks, chips
}

/// Which page and which theme the gallery shows. The socket's click targets move it; the window follows.
@MainActor @Observable
final class GalleryState {
    static let shared = GalleryState()
    var page: GalleryPage = .overview
    var dark = false
}

/// The shared controls on one board, for the design build's `shot` (docs/CONTROL-SOCKET.md): one page per control, plus an overview.
/// The sample accounts are the design fixture's; their icons are the board's proposed ones (native-glass v2 `ICON`).
struct ComponentGalleryView: View {
    @State private var state = GalleryState.shared
    @State private var labels = LabelPickerModel(nodes: LabelFixture.nodes(), selected: ["Work/Clients"], collapsed: ["Personal"])
    @State private var typing = LabelPickerModel(nodes: LabelFixture.nodes(), selected: [])
    private let now = Date()

    var body: some View {
        ZStack {
            Rectangle().fill(WhooshPalette.background)
            content.padding(WhooshSpacing.s24)
        }
        .preferredColorScheme(state.dark ? .dark : .light)
        .frame(width: 1040, height: 680)
        .onAppear { typing.query = "Suppliers 2" }
    }

    @ViewBuilder private var content: some View {
        switch state.page {
        case .overview: overview
        case .brand: centered { brandStage }
        case .suggest: centered { suggestStage }
        case .snooze: centered { TimePickerInWords(now: now) { _ in } }
        case .snoozedate: centered { TimePickerInWords(now: now, openOnDate: true) { _ in } }
        case .labels:
            HStack(alignment: .top, spacing: WhooshSpacing.s32) {
                captioned("Every account's labels, nested") { LabelPicker(model: $labels) { _ in } }
                captioned("A new name offers Create") { LabelPicker(model: $typing) { _ in } }
            }
        case .undo: centered { undoStage }
        case .status: centered { statusStack }
        case .keys: centered { keysStack }
        case .empty: EmptyState(symbol: "tray", title: "Inbox zero", line: "Nothing is waiting for you. New mail appears here as it arrives.", key: ("⌘N", "Write a message"))
        case .marks: centered { marksBoard }
        case .chips: centered { chipsStack }
        }
    }

    private func centered<C: View>(@ViewBuilder _ c: () -> C) -> some View { c().frame(maxWidth: .infinity, maxHeight: .infinity) }
    private func captioned<C: View>(_ t: String, @ViewBuilder _ c: () -> C) -> some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s12) {
            Text(t).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
            c()
        }
    }

    private var overview: some View {
        HStack(alignment: .top, spacing: WhooshSpacing.s24) {
            VStack(alignment: .leading, spacing: WhooshSpacing.s20) {
                TimePickerInWords(now: now) { _ in }
                undoStage
                smallMarks
            }
            LabelPicker(model: $labels) { _ in }
            VStack(alignment: .leading, spacing: WhooshSpacing.s20) { chipsStack; keysStack; statusStack }.fixedSize()
        }
    }

    private var smallMarks: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s10) {
            ForEach([AccountMarkSize.row, .compact, .sidebar], id: \.points) { size in
                HStack(spacing: WhooshSpacing.s10) {
                    ForEach(GallerySample.accounts.prefix(8), id: \.id) { a in
                        AccountMark(name: a.name, address: a.address, colour: a.colour, icon: .library(a.icon), size: size, hasUnread: a.id.hasSuffix("1"))
                    }
                }
            }
        }
    }

    private var undoStage: some View {
        ZStack {
            GalleryWallpaper(dark: state.dark)
            VStack(spacing: 0) {
                ForEach(0..<3, id: \.self) { _ in Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator).frame(maxHeight: .infinity, alignment: .bottom) }
            }
            UndoBar(message: "Message sent", countdown: UndoCountdown(startedAt: now.addingTimeInterval(-3)), onUndo: {})
        }
        .frame(width: 360, height: 120).clipShape(RoundedRectangle(cornerRadius: WhooshRadius.hint, style: .continuous))
    }

    /// The brand mark on the board wallpaper at the three sizes it ships at.
    private var brandStage: some View {
        ZStack {
            GalleryWallpaper(dark: state.dark)
            HStack(alignment: .center, spacing: WhooshSpacing.s32) { BrandMark(size: 24, surface: "gallery"); BrandMark(size: 56, surface: "gallery"); BrandMark(size: 160, surface: "gallery") }
        }.frame(width: 640, height: 300).clipShape(RoundedRectangle(cornerRadius: WhooshRadius.hint, style: .continuous))
    }

    /// The follow-ups on the board wallpaper, one per kind, in the undo bar's place.
    private var suggestStage: some View {
        ZStack {
            GalleryWallpaper(dark: state.dark)
            VStack(spacing: WhooshSpacing.s16) {
                ForEach([SuggestionKind.blockSender, .alwaysJunkDomain, .unsubscribe, .muteThread, .alwaysLabel], id: \.self) { kind in
                    SuggestionBar(SuggestionOffer(Suggestion(id: "g-\(kind)", kind: kind, subject: "news@shop-example.com", labelId: nil, offeredAtMs: 0, accepted: false)),
                                  onAccept: {}, onDismiss: {})
                }
            }
        }.frame(width: 420, height: 300).clipShape(RoundedRectangle(cornerRadius: WhooshRadius.hint, style: .continuous))
    }

    private var statusStack: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s12) {
            StatusLine(.synced("Synced · 2 minutes ago"))
            StatusLine(.syncing("Syncing Work · 4,210 of 12,880"))
            StatusLine(.attention("Provanta RFP needs you to sign in", action: "Sign in"))
        }
    }
    private var keysStack: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s12) {
            KeyHint("E", "Archive").capAx("mac-components.gallery.key-hint.cap"); KeyHint("Z", "Undo"); KeyHint(["⌘", "K"], "Command palette"); KeyHint(["⇧", "Tab"], "Previous tab")
        }
    }
    private var chipsStack: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s12) {
            HStack(spacing: WhooshSpacing.s6) { LabelChip("Nordlicht").axId("mac-components.gallery.label-chip"); LabelChip("Suppliers"); LabelChip("Invoices") }
            HStack(spacing: WhooshSpacing.s6) { CodePill("482 913"); CodePill("T-7740") }
        }
    }

    private var marksBoard: some View {
        let accounts = GallerySample.accounts
        return VStack(alignment: .leading, spacing: WhooshSpacing.s16) {
            ForEach([AccountMarkSize.row, .compact, .sidebar, .settings], id: \.points) { size in
                HStack(spacing: WhooshSpacing.s12) {
                    Text("\(Int(size.points)) pt").whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).frame(width: 44, alignment: .leading)
                    ForEach(accounts.prefix(size == .settings ? 3 : 8), id: \.id) { a in
                        AccountMark(name: a.name, address: a.address, colour: a.colour, icon: .library(a.icon), size: size, hasUnread: a.id.hasSuffix("1"))
                    }
                }
            }
            HStack(spacing: WhooshSpacing.s16) {
                let bad = GallerySample.account("acc-18", icon: "phone"), work = GallerySample.account("acc-01", icon: "briefcase")
                let fam = GallerySample.account("acc-10", icon: "heart")
                captioned("Needs a sign-in") { AccountMark(name: bad.name, address: bad.address, colour: bad.colour, icon: .library(bad.icon), size: .settings, needsAttention: true) }
                captioned("On the selection") { ZStack { RoundedRectangle(cornerRadius: WhooshRadius.row).fill(WhooshPalette.selection).frame(width: 56, height: 40)
                    AccountMark(name: work.name, address: work.address, colour: work.colour, icon: .library(work.icon), onSelection: true) } }
                captioned("Chosen in Settings") { AccountMark(name: fam.name, address: fam.address, colour: fam.colour, icon: .library(fam.icon), size: .sidebar, chosen: true) }
                captioned("Missing image") { AccountMark(name: work.name, address: work.address, colour: work.colour, icon: .uploaded(path: "/nonexistent.png"), size: .settings) }
            }
        }
    }
}

/// Sample marks for the gallery: the fixture's accounts and the board's proposed icon per account.
enum GallerySample {
    struct Item { let id: String; let name: String; let address: String; let colour: String; let icon: String }
    private static let icons = ["acc-01": "briefcase", "acc-02": "house", "acc-03": "chart", "acc-04": "star", "acc-05": "pen",
                                "acc-06": "bolt", "acc-07": "globe", "acc-08": "code", "acc-09": "bank", "acc-10": "heart"]
    private struct File: Decodable { let accounts: [Acc]; struct Acc: Decodable { let id: String; let address: String; let name: String; let colour: String } }
    static func account(_ id: String, icon: String) -> Item {
        let a = accounts.first { $0.id == id }
        return Item(id: id, name: a?.name ?? id, address: a?.address ?? id, colour: a?.colour ?? "", icon: icon)
    }
    static var accounts: [Item] {
        guard let url = FixtureFile.locate(), let data = try? Data(contentsOf: url), let f = try? JSONDecoder().decode(File.self, from: data) else { return [] }
        return f.accounts.map { Item(id: $0.id, name: $0.name, address: $0.address, colour: $0.colour, icon: icons[$0.id] ?? "mail") }
    }
}

/// The board's stand-in desktop (WhooshBoardWallpaper): a base gradient and three soft blobs, so the glass has something to refract.
struct GalleryWallpaper: View {
    let dark: Bool
    var body: some View {
        let scheme: ColorScheme = dark ? .dark : .light
        GeometryReader { g in
            ZStack {
                LinearGradient(colors: [WhooshBoardWallpaper.baseTop.color(scheme), WhooshBoardWallpaper.baseBottom.color(scheme)], startPoint: .top, endPoint: .bottom)
                ForEach(WhooshBoardWallpaper.blobs.indices, id: \.self) { i in
                    let b = WhooshBoardWallpaper.blobs[i]
                    RadialGradient(colors: [b.color.color(scheme), .clear], center: b.center, startRadius: 0,
                                   endRadius: max(b.radius.width * g.size.width, b.radius.height * g.size.height) * WhooshBoardWallpaper.blobFalloff)
                }
            }
        }
    }
}
