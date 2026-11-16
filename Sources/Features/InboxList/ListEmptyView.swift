import SwiftUI

/// The zero state: "All done", the next snooze return, and the next-tab hint (docs/DESIGN.md v2 frame 7).
struct ListEmptyView: View {
    let words: EmptyStateWords
    var body: some View {
        VStack(spacing: WhooshSpacing.s6) {
            Image(systemName: "checkmark.circle").font(.system(size: WhooshMetrics.avatar + WhooshSpacing.s12, weight: .light))
                .foregroundStyle(WhooshPalette.tertiaryLabel).padding(.bottom, WhooshSpacing.s8)
            Text(words.title).font(WhooshTypography.title3.font(emphasized: true)).foregroundStyle(WhooshPalette.label)
            Text(words.line).font(WhooshTypography.subhead.font(emphasized: false)).foregroundStyle(WhooshPalette.secondaryLabel)
                .multilineTextAlignment(.center).frame(maxWidth: WhooshMetrics.blockWidth * 0.6)
            if !words.keyHint.isEmpty {
                Text(words.keyHint).font(WhooshTypography.subhead.font(emphasized: true)).foregroundStyle(WhooshPalette.accent)
                    .padding(.top, WhooshSpacing.s8)
            }
        }
        .padding(.horizontal, WhooshSpacing.s32)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .accessibilityElement(children: .combine)
        .whooshAx("mac-main.list.empty")
    }
}
