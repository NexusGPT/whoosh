import SwiftUI

/// Small shared pieces of the AI panes, drawn with the Settings window's kit: the card, a grey lead line, the chip, the button.
struct AICard<Content: View>: View {
    @ViewBuilder let content: Content
    var body: some View {
        content
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: SettingsMetrics.cardRadius, style: .continuous).fill(SettingsPalette.card))
            .overlay(RoundedRectangle(cornerRadius: SettingsMetrics.cardRadius, style: .continuous)
                .strokeBorder(SettingsPalette.cardEdge, lineWidth: 0.5))
    }
}

/// A line of grey words under a pane's header (the pane header carries the name).
struct AIHeader: View {
    let title: String
    let lead: String
    var body: some View {
        Text(lead).font(.system(size: 12)).foregroundStyle(WhooshPalette.secondaryLabel).fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, WhooshSpacing.s4).padding(.top, WhooshSpacing.s8)
            .frame(maxWidth: .infinity, alignment: .leading)
    }
}

struct AIChip: View {
    let text: String
    var tone: Tone = .neutral
    enum Tone { case neutral, good, warn, accent }
    var body: some View {
        SettingsChip(text: text, tone: tone == .warn ? .warn : (tone == .neutral ? .neutral : .good))
    }
}

struct AIButton: View {
    let title: String
    var primary = false
    var id: String
    let action: () -> Void
    var body: some View { SettingsButton(title: title, primary: primary, ax: id, action: action) }
}
