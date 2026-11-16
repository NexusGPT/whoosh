import SwiftUI

/// The top of the conversation, under the floating Back: the subject as a large title, who is in it, the receiving
/// account (the same mark as the inbox rows and the Mailboxes screen) and the message count.
struct ThreadHeader: View {
    let thread: ReaderThread
    let account: PhoneAccount

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            Text(thread.subject).whooshText(ThreadMetrics.subject).foregroundStyle(WhooshPalette.label)
                .fixedSize(horizontal: false, vertical: true).textSelection(.enabled)
                .accessibilityAddTraits(.isHeader).accessibilityIdentifier("iphone-thread.subject")
            Text(thread.participantNames.joined(separator: ", ")).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                .lineLimit(2).accessibilityIdentifier("iphone-thread.participants")
            HStack(spacing: WhooshSpacing.s8) {
                AccountMark(name: account.name, address: account.address, colour: account.colour, icon: account.icon, size: .compact,
                            needsAttention: account.needsAttention)
                Text(account.name).phoneText(PhoneType.footnote, emphasized: true).foregroundStyle(WhooshPalette.label).lineLimit(1)
                Text(ReaderWords.count(thread.messageCount)).phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
            }
            .accessibilityElement(children: .combine).accessibilityIdentifier("iphone-thread.account")
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, PhoneMetrics.gutter).padding(.bottom, WhooshSpacing.s8)
    }
}
