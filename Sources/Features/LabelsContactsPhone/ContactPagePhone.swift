import SwiftUI

/// One sender: their name, every address joined as one person, and all the mail from them, newest first, each row with its account.
/// The mark is the sender's real one or none: no shared component draws a sender's mark yet, so the portrait is a quiet person glyph.
struct ContactPagePhone: View {
    let address: String
    private let m = AppModel.shared
    private let state = LabelsContactsPhoneState.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let _ = state.revision
        ZStack(alignment: .top) {
            PhoneGround.grouped.ignoresSafeArea()
            if let c = PhoneContactReader.read(address: address, m) {
                page(c)
            } else {
                PhoneEmpty(symbol: "person.crop.circle", title: LabelsWords.pickPerson, line: "")
            }
        }
        .overlay(alignment: .topLeading) { PhoneBackButton { state.back(m) } }
        .phoneBarHidden()
    }

    private func page(_ c: PhoneContactRead) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
                Image(systemName: "person.fill").font(.system(size: LabelsPhoneMetrics.portrait * 0.45))
                    .foregroundStyle(WhooshPalette.secondaryLabel)
                    .frame(width: LabelsPhoneMetrics.portrait, height: LabelsPhoneMetrics.portrait)
                    .background(Circle().fill(PhoneGround.trackFill))
                    .padding(.horizontal, PhoneMetrics.gutter).padding(.top, PhoneMetrics.titleTop)
                    .accessibilityHidden(true)
                Text(c.name).phoneText(PhoneType.largeTitle).foregroundStyle(WhooshPalette.label).lineLimit(2)
                    .padding(.horizontal, PhoneMetrics.gutter).accessibilityAddTraits(.isHeader)
                    .accessibilityIdentifier("iphone-labels-contacts.contact.name")
                PhoneSectionTitle(Words.text("Addresses"), trailing: "\(c.addresses.count)")
                PhoneGroupCard {
                    ForEach(Array(c.addresses.enumerated()), id: \.offset) { i, a in
                        if i > 0 { PhoneHair() }
                        Text(a).phoneText(PhoneType.body).foregroundStyle(WhooshPalette.label).lineLimit(1)
                            .padding(.horizontal, PhoneMetrics.gutter).frame(maxWidth: .infinity, minHeight: PhoneMetrics.mailboxRow, alignment: .leading)
                            .accessibilityIdentifier("iphone-labels-contacts.contact.address.\(i)")
                    }
                }
                if let other = c.lookAlike { mergeOffer(c, other) }
                PhoneSectionTitle(Words.text("Mail"), trailing: PhoneWords.count(c.messages.count))
                PhoneGroupCard {
                    if c.messages.isEmpty {
                        Text(Words.text("No mail from this sender yet.")).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                            .padding(PhoneMetrics.gutter).frame(maxWidth: .infinity, alignment: .leading)
                    }
                    ForEach(Array(c.messages.enumerated()), id: \.element.messageId) { i, r in
                        PhoneRow(row: r, account: PhoneAccounts.of(r.accountId, in: m))
                            .onTapGesture { _ = state.openThread(r.messageId, in: m) }
                            .accessibilityIdentifier("iphone-labels-contacts.contact.messages.\(i)")
                    }
                }
            }
            .padding(.bottom, PhoneMetrics.listBottomRoom)
        }
        .scrollIndicators(.hidden)
        .animation(reduceMotion ? nil : WhooshMotion.newRow, value: c.addresses)
    }

    private func mergeOffer(_ c: PhoneContactRead, _ other: PhoneLookAlike) -> some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            Text(LabelsWords.sameSender(other.address)).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.label)
            Button { if PhoneContactReader.merge(c, m) { state.touch() } } label: {
                Text(LabelsWords.mergeInto).phoneText(PhoneType.headline).foregroundStyle(WhooshPalette.label)
                    .padding(.horizontal, PhoneMetrics.gutter).frame(minHeight: PhoneMetrics.roundButton).whooshGlass(in: Capsule())
            }
            .buttonStyle(PressStyle()).accessibilityIdentifier("iphone-labels-contacts.contact.merge")
        }
        .padding(PhoneMetrics.gutter).frame(maxWidth: .infinity, alignment: .leading)
        .background(PhoneGround.card, in: RoundedRectangle(cornerRadius: PhoneMetrics.cardRadius, style: .continuous))
        .padding(.horizontal, PhoneMetrics.gutter).padding(.top, WhooshSpacing.s8)
        .transition(.opacity)
    }
}
