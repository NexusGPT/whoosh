import SwiftUI

/// The floating glass bar: Filter · the Search pill with the microphone · Compose. Signature move: tap Search and the
/// pill widens and rises with the keyboard on one spring; Compose dissolves into its edge; × sinks it back.
struct PhoneBottomBar: View {
    @Bindable var state: InboxPhoneState
    let tabs: [(id: String, title: String, unread: Int)]
    var onTab: (String) -> Void
    var onCompose: () -> Void
    @State private var query = ""
    @FocusState private var focused: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        HStack(spacing: PhoneMetrics.barGap) {
            Menu {
                ForEach(tabs, id: \.id) { t in Button("\(t.title) · \(t.unread)") { onTab(t.id) } }
                Divider()
                Button("Unread", systemImage: "envelope.badge") {}
                Button("Flagged", systemImage: "flag") {}
                Button("Attachments", systemImage: "paperclip") {}
            } label: { barGlyph("line.3.horizontal.decrease", label: "Filter") }
                .menuStyle(.button).buttonStyle(PressStyle())
            pill
            if !state.searching {
                Button(action: onCompose) { barGlyph("square.and.pencil", label: "Compose") }
                    .buttonStyle(PressStyle()).transition(.opacity.combined(with: .scale(scale: 0.8, anchor: .leading)))
                    .accessibilityIdentifier("iphone-inbox.compose")
            }
        }
        .padding(.horizontal, PhoneMetrics.gutter)
        .animation(reduceMotion ? nil : .spring(response: WhooshMotion.tabMorph, dampingFraction: WhooshMotion.tabMorphDamping), value: state.searching)
    }

    private var pill: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Image(systemName: "magnifyingglass").font(.system(size: PhoneType.headline.size, weight: .semibold))
                .foregroundStyle(WhooshPalette.label)
            if state.searching {
                TextField("Search", text: $query).focused($focused).phoneText(PhoneType.body)
                    .submitLabel(.search).accessibilityIdentifier("iphone-inbox.search.field")
            } else {
                Text("Search").phoneText(PhoneType.body).foregroundStyle(WhooshPalette.secondaryLabel)
                Spacer(minLength: 0)
            }
            Button { if state.searching { close() } } label: {
                Image(systemName: state.searching ? "xmark" : "mic").font(.system(size: PhoneType.headline.size, weight: .semibold))
                    .foregroundStyle(WhooshPalette.label).frame(width: PhoneMetrics.mic, height: PhoneMetrics.mic)
                    .contentShape(Circle())
            }
            .buttonStyle(PressStyle()).accessibilityLabel(state.searching ? "Close search" : "Dictate a search")
        }
        .padding(.leading, PhoneMetrics.gutter).padding(.trailing, WhooshSpacing.s8)
        .frame(width: state.searching ? PhoneMetrics.searchPillOpen : PhoneMetrics.searchPill, height: PhoneMetrics.barButton)
        .whooshGlass(in: Capsule())
        .contentShape(Capsule())
        .onTapGesture { if !state.searching { state.searching = true; PhoneRouter.shared.openSearch() } }
        .accessibilityIdentifier("iphone-inbox.search")
    }

    private func close() { query = ""; focused = false; state.searching = false }

    private func barGlyph(_ symbol: String, label: String) -> some View {
        Image(systemName: symbol).font(.system(size: PhoneType.title3.size + WhooshSpacing.s2, weight: .semibold))
            .foregroundStyle(WhooshPalette.label)
            .frame(width: PhoneMetrics.barButton, height: PhoneMetrics.barButton).whooshGlass(in: Circle())
            .accessibilityLabel(label)
    }
}

/// The capsule above the bar: the undo after a swipe ("Archived · Marie Lefèvre  Undo"), or a refusal in words.
/// It rises UNDO_RISE_PX on the insert curve; the undo window is UNDO_SEND_DEFAULT_S, no number shown on the phone.
struct PhoneCapsule: View {
    let state: InboxPhoneState
    var onUndo: () -> Void

    var body: some View {
        Group {
            if let f = state.failure {
                HStack(spacing: WhooshSpacing.s8) {
                    Circle().fill(WhooshPalette.warning).frame(width: WhooshMetrics.unreadDot, height: WhooshMetrics.unreadDot)
                    Text(f).foregroundStyle(WhooshPalette.label).lineLimit(2)
                    Spacer(minLength: 0)
                    Button("OK") { state.clearFailure() }.buttonStyle(.plain).foregroundStyle(WhooshPalette.selection)
                        .phoneText(PhoneType.subhead, emphasized: true)
                }
                .accessibilityIdentifier("iphone-inbox.failure")
            } else if let u = state.undo {
                HStack(spacing: WhooshSpacing.s6) {
                    Text(u.verb).foregroundStyle(WhooshPalette.label)
                    Text("· \(u.sender)").foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                    Spacer(minLength: 0)
                    Button(action: onUndo) {
                        Text("Undo").phoneText(PhoneType.subhead, emphasized: true).foregroundStyle(WhooshPalette.selection)
                            .padding(.horizontal, WhooshSpacing.s14).frame(height: PhoneMetrics.segmentHeight).contentShape(Capsule())
                    }
                    .buttonStyle(PressStyle()).accessibilityIdentifier("iphone-inbox.undo.button")
                }
                .task(id: u.at) {
                    let w = UndoCountdown(startedAt: u.at)
                    try? await Task.sleep(for: .seconds(w.remaining(at: Date())))
                    if state.undo == u { state.expireUndo() }
                }
            }
        }
        .phoneText(PhoneType.subhead)
        .padding(.leading, WhooshSpacing.s20).padding(.trailing, WhooshSpacing.s6)
        .frame(width: PhoneMetrics.undoWidth, height: PhoneMetrics.roundButton)
        .whooshGlass(in: Capsule())
        .accessibilityElement(children: .contain)
    }
}
