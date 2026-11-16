import SwiftUI

/// The door from a label's page: every message that carries the label, newest first, in the inbox's own rows. A row opens its thread.
struct LabelMailPhone: View {
    let id: String
    private let m = AppModel.shared
    private let state = LabelsContactsPhoneState.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let _ = state.revision
        let _ = m.mailRevision          // the label's figure and its mail are read from the core here: drawn again when the mail changes
        let label = m.core.labels().first { $0.id == id }
        let rows = PhoneLabelReader.mail(id, m)
        ZStack(alignment: .top) {
            PhoneGround.list.ignoresSafeArea()
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    PhoneLargeTitle(text: label?.name ?? LabelsWords.gone, dot: LabelColourPhone.style(label?.colour))
                    Text(PhoneWords.count(rows.count)).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                        .contentTransition(.numericText()).padding(.horizontal, PhoneMetrics.gutter).padding(.bottom, WhooshSpacing.s12)
                        .accessibilityIdentifier("iphone-labels-contacts.mail.count")
                    if rows.isEmpty {
                        PhoneEmpty(symbol: "tray", title: label?.name ?? LabelsWords.gone, line: Words.text("No mail carries this label yet."))
                            .accessibilityIdentifier("iphone-labels-contacts.mail.empty")
                    } else {
                        LazyVStack(spacing: 0) {
                            ForEach(Array(rows.enumerated()), id: \.element.messageId) { i, r in
                                PhoneRow(row: r, account: PhoneAccounts.of(r.accountId, in: m))
                                    .onTapGesture { _ = state.openThread(r.messageId, in: m) }
                                    .accessibilityIdentifier("iphone-labels-contacts.mail.row.\(i)")
                            }
                        }
                    }
                }
                .padding(.bottom, PhoneMetrics.listBottomRoom)
            }
            .scrollIndicators(.hidden)
        }
        .overlay(alignment: .topLeading) { PhoneBackButton { state.back(m) } }
        .phoneBarHidden()
    }
}
