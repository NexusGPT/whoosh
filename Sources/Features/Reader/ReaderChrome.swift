import SwiftUI

/// `.cv-h` the title (22/28, 600) and the participants line: the account mark, the account, the people, the count.
struct ReaderHeader: View {
    let thread: ReaderThread
    let account: RowAccount
    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s4) {
            Text(thread.subject)
                .whooshText(WhooshTypography.largeTitle).foregroundStyle(WhooshPalette.label)
                .lineLimit(2).fixedSize(horizontal: false, vertical: true)
                .accessibilityIdentifier("mac-main.reader.subject")
            HStack(spacing: WhooshSpacing.s6) {
                AccountMark(name: account.name, address: account.address, colour: account.colour,
                            icon: .providerDefault, size: .row, needsAttention: account.needsAttention)
                    .axId("mac-main.reader.account-mark")
                Text(account.name).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.label)
                    .layoutPriority(1)
                Text("· " + thread.shownParticipants(own: Set(AccountBook.shared.accounts().map { $0.address.lowercased() }), you: ThreadCardCopy.you).joined(separator: ", "))
                    .whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                    .truncationMode(.tail)
                Spacer(minLength: WhooshSpacing.s8)
                Text(ReaderWords.count(thread.messageCount))
                    .whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).fixedSize()
            }
            .lineLimit(1)
            .accessibilityIdentifier("mac-main.reader.participants")
        }
    }
}

/// `.cv-none` nothing open: one line, no illustration.
struct ReaderEmpty: View {
    /// Said under the line after a delete: the message is gone and what that means for undo.
    var note: String?
    var body: some View {
        VStack(spacing: WhooshSpacing.s2) {
            Text(ReaderWords.emptyTitle).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label)
            Text(ReaderWords.emptyLine).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                .multilineTextAlignment(.center)
            if let note {
                Text(note).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                    .multilineTextAlignment(.center).padding(.top, WhooshSpacing.s8)
                    .accessibilityIdentifier("mac-main.reader.deleted")
            }
        }
        .padding(.horizontal, WhooshMetrics.readerSide)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .accessibilityIdentifier("mac-main.reader.empty")
    }
}

/// MOTION.md rule 8: content swaps under a short blur cross-fade (opacity + blur), never a slide.
struct ReaderSwapBlur: ViewModifier {
    /// MOTION.md rule 8, "blur radius 0 to 6 pt"; asked of the root as CONSTANTS row READER_CROSSFADE_BLUR_PT.
    static let radius: CGFloat = WhooshSpacing.s6
    let on: Bool
    func body(content: Content) -> some View { content.blur(radius: on ? Self.radius : 0) }
}

extension AnyTransition {
    static func readerSwap(reduceMotion: Bool) -> AnyTransition {
        reduceMotion ? .opacity : .opacity.combined(with: .modifier(active: ReaderSwapBlur(on: true), identity: ReaderSwapBlur(on: false)))
    }
}
