import SwiftUI

/// The sender card: who they are, every address that is theirs, and every message they sent to any account,
/// newest first, each row carrying its account mark. A second address that looks like theirs is offered for merge.
struct ContactCardView: View {
    let model: LabelsContactsModel

    var body: some View {
        if let card = model.card {
            VStack(alignment: .leading, spacing: WhooshSpacing.s20) {
                header(card)
                if let other = model.mergeCandidate { mergeOffer(other.addresses.first ?? other.name) }
                Text(card.messages.count == 1 ? LabelsWords.oneMessage : LabelsWords.messages(card.messages.count))
                    .whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
                    .contentTransition(.numericText())
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(Array(card.messages.enumerated()), id: \.element.messageId) { i, r in
                            SenderMessageRow(row: r, source: model.source, index: i) { _ = model.openThread(ofMessage: r.messageId, threadId: r.threadId) }
                        }
                    }
                }
                .accessibilityIdentifier("mac-labels-contacts.contact.messages")
            }
            .padding(.horizontal, WhooshMetrics.readerSide).padding(.top, WhooshMetrics.readerTop).padding(.bottom, WhooshMetrics.readerSide)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        } else {
            EmptyState(symbol: "person.crop.circle", title: LabelsWords.pickPerson,
                       line: LabelsWords.clickSender)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private func header(_ card: SenderCard) -> some View {
        HStack(alignment: .center, spacing: WhooshSpacing.s16) {
            Text(initials(card.name)).whooshText(WhooshTypography.title3, emphasized: true)
                .foregroundStyle(WhooshPalette.onAccent)
                .frame(width: 2 * WhooshMetrics.avatar, height: 2 * WhooshMetrics.avatar)
                .background(Circle().fill(WhooshPalette.accent))
            VStack(alignment: .leading, spacing: WhooshSpacing.s6) {
                Text(card.name).whooshText(WhooshTypography.largeTitle, emphasized: true).foregroundStyle(WhooshPalette.label)
                HStack(spacing: WhooshSpacing.s6) {
                    ForEach(card.addresses, id: \.self) { a in
                        Text(a).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                            .padding(.horizontal, WhooshSpacing.s8).padding(.vertical, WhooshSpacing.s2)
                            .background(Capsule().fill(WhooshPalette.fill))
                            .transition(.scale.combined(with: .opacity))
                    }
                }
                .animation(.spring, value: card.addresses)
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("mac-labels-contacts.contact.header")
    }

    private func mergeOffer(_ other: String) -> some View {
        HStack(spacing: WhooshSpacing.s12) {
            Image(systemName: "person.2.fill").foregroundStyle(WhooshPalette.accent)
            Text(LabelsWords.sameSender(other)).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label)
            Spacer()
            Button(LabelsWords.mergeInto) { withAnimation(.spring) { _ = model.mergeSuggested() } }
                .accessibilityIdentifier("mac-labels-contacts.contact.merge")
        }
        .padding(WhooshSpacing.s12)
        .background(RoundedRectangle(cornerRadius: WhooshRadius.sheet, style: .continuous).fill(WhooshPalette.mine))
        .transition(.opacity)
    }

    private func initials(_ name: String) -> String {
        name.split(separator: " ").prefix(2).compactMap(\.first).map(String.init).joined()
    }
}

/// One message in a card or a rule preview: account mark, sender and subject, the first line, when.
struct SenderMessageRow: View {
    let row: InboxRow
    let source: LabelsContactsSource
    /// Where the row sits in a sender card; nil in a rule preview, where the row opens nothing.
    var index: Int?
    /// A click on the row of a sender card opens its thread.
    var onOpen: (() -> Void)?

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s10) {
            if let a = source.account(row.accountId) {
                AccountMark(name: a.name, address: a.address, colour: a.colour, icon: a.iconId.map { .library($0) } ?? .providerDefault).alignmentGuide(.firstTextBaseline) { $0[.bottom] - WhooshSpacing.s2 }
            }
            VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                Text(row.subject).whooshText(WhooshTypography.body, emphasized: row.unread).foregroundStyle(WhooshPalette.label).lineLimit(1)
                Text(row.preview).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
            }
            Spacer(minLength: WhooshSpacing.s8)
            if row.receivedAtMs > 0 {
                Text(Date(timeIntervalSince1970: Double(row.receivedAtMs) / 1000).formatted(date: .abbreviated, time: .omitted))
                    .whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
            }
        }
        .padding(.vertical, WhooshSpacing.s8).padding(.horizontal, WhooshSpacing.s8)
        .hoverAck(radius: WhooshRadius.row)
        .overlay(alignment: .bottom) { Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator) }
        .contentShape(Rectangle())
        .onTapGesture { onOpen?() }
        .accessibilityIdentifier(index.map { "mac-labels-contacts.contact.messages.\($0)" } ?? "mac-labels-contacts.rule.preview.row")
    }
}
