import SwiftUI

/// The first-sync list: content-shaped skeleton rows (the 44 px row's two lines and its mark) under the words that say
/// how far the history is (docs/surfaces mac-main, ux-inboxlist-states). The bars stand still: no shimmer, no timer
/// (docs/MOTION.md: nothing decorative, no weight).
struct LoadingList: View {
    let words: EmptyStateWords
    static let rowCount = 6

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(0..<Self.rowCount, id: \.self) { i in SkeletonRow(index: i) }
            VStack(spacing: WhooshSpacing.s4) {
                Text(words.title).font(WhooshTypography.body.font(emphasized: true)).foregroundStyle(WhooshPalette.label)
                Text(words.line).font(WhooshTypography.subhead.font(emphasized: false)).foregroundStyle(WhooshPalette.secondaryLabel)
                    .multilineTextAlignment(.center)
            }
            .frame(maxWidth: .infinity).padding(.top, WhooshSpacing.s24)
            Spacer(minLength: 0)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(words.title). \(words.line)")
        .whooshAx("mac-main.list.loading")
    }
}

/// One placeholder row: a ring where the account mark goes, a sender bar and a subject bar of varying length.
struct SkeletonRow: View {
    let index: Int
    /// bar lengths in spacing steps, so six rows read as six different messages (one table, used once)
    private static let senderSteps: [CGFloat] = [9, 7, 11, 8, 10, 6]
    private static let subjectSteps: [CGFloat] = [19, 16, 21, 14, 18, 15]
    private var senderWidth: CGFloat { WhooshSpacing.s16 * Self.senderSteps[index % Self.senderSteps.count] }
    private var subjectWidth: CGFloat { WhooshSpacing.s16 * Self.subjectSteps[index % Self.subjectSteps.count] }

    var body: some View {
        HStack(alignment: .top, spacing: WhooshSpacing.s10) {
            Circle().strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator)
                .frame(width: CGFloat(WhooshConstants.ACCOUNT_MARK_ROW_PX), height: CGFloat(WhooshConstants.ACCOUNT_MARK_ROW_PX))
            VStack(alignment: .leading, spacing: WhooshSpacing.s6) {
                bar(senderWidth)
                bar(subjectWidth)
            }
            .padding(.top, WhooshSpacing.s4)
        }
        .padding(.leading, WhooshMetrics.listRowInset + WhooshSpacing.s14)
        .frame(maxWidth: .infinity, minHeight: WhooshMetrics.listRow, alignment: .leading)
    }

    private func bar(_ w: CGFloat) -> some View {
        RoundedRectangle(cornerRadius: WhooshRadius.progress, style: .continuous).fill(WhooshPalette.fill)
            .frame(width: w, height: WhooshSpacing.s8)
    }
}

/// The line under the last row: the list is complete, how many conversations, and the next tab's key.
struct ListEndLine: View {
    let model: AppModel
    let shown: Int
    /// the words are built here, so the list's body does not pay for them on every focus move (perf r2)
    var body: some View {
        let words = ListEndWords.make(model, shown: shown)
        HStack(spacing: WhooshSpacing.s8) {
            Rectangle().fill(WhooshPalette.separator).frame(width: WhooshSpacing.s24, height: WhooshMetrics.separator)
            Text(words).font(WhooshTypography.caption.font(emphasized: false)).foregroundStyle(WhooshPalette.tertiaryLabel)
            Rectangle().fill(WhooshPalette.separator).frame(width: WhooshSpacing.s24, height: WhooshMetrics.separator)
        }
        .frame(maxWidth: .infinity).frame(height: WhooshMetrics.listHeaderHeight + WhooshSpacing.s16)
        .whooshAx("mac-main.list.end")
    }
}

@MainActor
enum ListEndWords {
    /// "End of Primary · 16 conversations · Tab for Needs Reply"
    static func make(_ m: AppModel, shown: Int) -> String {
        if let folder = SmartFolderWords.name(m) { return Words.text("End of %@ · %lld conversations", folder, shown) }
        if let box = m.mailbox { return Words.text("End of %@ · %lld conversations", MailboxWords.name(box), shown) }
        let tabs = PinnedTabs.all(m)          // read once (perf r2: twice per render was 17.6% of next's samples)
        let name: (String) -> String = { id in tabs.first { $0.id == id }?.name ?? Words.text(CoreText.tabName(id, in: m)) }
        var w = Words.text("End of %@ · %lld conversations", name(m.tab), shown)
        let ids = m.tabIds
        if let i = ids.firstIndex(of: m.tab), ids.count > 1 {
            w += " · " + Words.text("Tab for %@", name(ids[(i + 1) % ids.count]))
        }
        return w
    }
}

/// The list's 36 pt bottom inset under the WINDOW's foot strip (macshell r9 draws the one strip across list and reader,
/// with the key hints): it keeps the rows and the capsule above the strip and draws nothing itself.
struct ListFootStrip: View {
    var body: some View {
        Color.clear
            .frame(maxWidth: .infinity).frame(height: WhooshMetrics.footStrip)
            .whooshAx("mac-main.list.foot")
    }
}
