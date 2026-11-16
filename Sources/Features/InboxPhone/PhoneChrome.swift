import SwiftUI

/// The top of the inbox: glass Back · Select · "…" floating over the paper, the large title, the sync line and the
/// split tabs with their counts (the Mac's tabs, in the Mac's order, from the same core counts).
struct PhoneTopButtons: View {
    let state: InboxPhoneState
    var onBack: () -> Void

    var body: some View {
        HStack(spacing: WhooshSpacing.s8) {
            GlassCircle(symbol: "chevron.left", side: PhoneMetrics.roundButton, label: "Mailboxes", action: onBack)
                .accessibilityIdentifier("iphone-inbox.back")
            Spacer()
            Button { state.selecting.toggle(); state.selected = [] } label: {
                Text(state.selecting ? "Done" : "Select").phoneText(PhoneType.headline).foregroundStyle(WhooshPalette.label)
                    .frame(width: PhoneMetrics.selectWidth, height: PhoneMetrics.roundButton).whooshGlass(in: Capsule())
            }
            .buttonStyle(PressStyle()).accessibilityIdentifier("iphone-inbox.select")
            Menu {
                Button("Edit split tabs", systemImage: "rectangle.split.3x1") {}
                Button("Rules", systemImage: "list.bullet.indent") {}
                Button("Settings", systemImage: "gear") { PhoneRouter.shared.openSettings() }
            } label: {
                Image(systemName: "ellipsis").font(.system(size: PhoneType.headline.size, weight: .semibold))
                    .foregroundStyle(WhooshPalette.label)
                    .frame(width: PhoneMetrics.roundButton, height: PhoneMetrics.roundButton).whooshGlass(in: Circle())
            }
            .menuStyle(.button).buttonStyle(PressStyle()).accessibilityLabel("More")
        }
        .padding(.horizontal, PhoneMetrics.gutter)
    }
}

struct PhoneHeader: View {
    let title: String
    let syncWords: String
    let failing: String?
    let tabs: [(id: String, title: String, unread: Int)]
    let current: String
    var onTab: (String) -> Void
    @Namespace private var thumb
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(title).phoneText(PhoneType.largeTitle).foregroundStyle(WhooshPalette.label).lineLimit(1)
                .accessibilityAddTraits(.isHeader).padding(.horizontal, PhoneMetrics.gutter)
            status.padding(.horizontal, PhoneMetrics.gutter).padding(.bottom, WhooshSpacing.s12)
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 0) { ForEach(tabs, id: \.id) { segment($0) } }
                    .background {   // the one thumb, never a source: it takes the current tab's frame
                        if tabs.contains(where: { $0.id == current }) {
                            Capsule().fill(WhooshPalette.thumb)
                                .shadow(color: WhooshGlass.thumbShadow.color(scheme), radius: swiftUIShadowRadius(cssBlur: WhooshGlass.thumbShadowBlur), y: WhooshGlass.thumbShadowY)
                                .matchedGeometryEffect(id: current, in: thumb, isSource: false)
                                .animation(reduceMotion ? nil : WhooshMotion.tabThumb, value: current)
                        }
                    }
                    .padding((PhoneMetrics.trackHeight - PhoneMetrics.segmentHeight) / 2)
                    .background(Capsule().fill(PhoneGround.trackFill))
                    .padding(.horizontal, PhoneMetrics.gutter)
            }
            .padding(.bottom, WhooshSpacing.s12)
        }
    }

    @ViewBuilder private var status: some View {
        HStack(spacing: WhooshSpacing.s6) {
            if let failing {
                Circle().fill(WhooshPalette.warning).frame(width: WhooshMetrics.unreadDot, height: WhooshMetrics.unreadDot)
                Text(failing).foregroundStyle(WhooshPalette.label).lineLimit(1)
                Spacer(minLength: WhooshSpacing.s6)
                Text("Sign in").phoneText(PhoneType.subhead, emphasized: true).foregroundStyle(WhooshPalette.selection)
            } else {
                Text(syncWords).foregroundStyle(WhooshPalette.secondaryLabel)
            }
        }
        .phoneText(PhoneType.subhead)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("iphone-inbox.status")
    }

    private func segment(_ t: (id: String, title: String, unread: Int)) -> some View {
        let on = t.id == current
        return Button { onTab(t.id) } label: {
            HStack(spacing: WhooshSpacing.s6) {
                Text(t.title).phoneText(PhoneType.subhead, emphasized: on).foregroundStyle(WhooshPalette.label)
                Text("\(t.unread)").phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
                    .contentTransition(.numericText())
            }
            .padding(.horizontal, PhoneMetrics.segmentPad)
            .frame(height: PhoneMetrics.segmentHeight)
            .matchedGeometryEffect(id: t.id, in: thumb, isSource: true)   // hid-ai round 4: one matched-geometry source per id (the MatchedGeometryScope trap of main88): each item is the source of its own id, one thumb follows the chosen one
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .animation(reduceMotion ? nil : WhooshMotion.tabThumb, value: current)
        .accessibilityLabel("\(t.title), \(t.unread) unread")
        .accessibilityAddTraits(on ? [.isButton, .isSelected] : .isButton)
    }
}

/// A round glass button (`.rb`): the press compresses it the same frame (Liquid Glass response).
struct GlassCircle: View {
    let symbol: String
    let side: CGFloat
    let label: String
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol).font(.system(size: side * 0.42, weight: .semibold)).foregroundStyle(WhooshPalette.label)
                .frame(width: side, height: side).whooshGlass(in: Circle())
        }
        .buttonStyle(PressStyle()).accessibilityLabel(label)
    }
}

/// Feedback on the input frame: a press scales and brightens at once, no delay, no completion handler.
struct PressStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.scaleEffect(configuration.isPressed ? 0.94 : 1).brightness(configuration.isPressed ? 0.04 : 0)
    }
}
