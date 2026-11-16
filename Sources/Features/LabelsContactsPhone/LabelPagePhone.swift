import SwiftUI

/// A label's page: its name and colour, what it is, the pin, the mail in it with a door to that mail, then its rules as the
/// three cards the Mac shows. Read only except the pin. Rules are edited on the Mac in this version, and the page says so in one line.
struct LabelPagePhone: View {
    let id: String
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private let m = AppModel.shared
    private let state = LabelsContactsPhoneState.shared

    var body: some View {
        let _ = state.revision
        ZStack(alignment: .top) {
            PhoneGround.grouped.ignoresSafeArea()
            if let l = PhoneLabelReader.read(id, m) {
                page(l)
            } else {
                PhoneEmpty(symbol: "tag", title: LabelsWords.gone, line: LabelsWords.pickLabel)
            }
        }
        .overlay(alignment: .topLeading) { PhoneBackButton { state.back(m) } }
        .phoneBarHidden()
    }

    private func page(_ l: PhoneLabelRead) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
                PhoneLargeTitle(text: l.name, dot: LabelColourPhone.style(l.colour))
                Text(l.depth > 1 ? "\(l.path) · \(l.scope)" : l.scope).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                    .padding(.horizontal, PhoneMetrics.gutter).padding(.bottom, WhooshSpacing.s4)
                    .accessibilityIdentifier("iphone-labels-contacts.label.scope")
                PhoneSectionTitle(Words.text("What this label is"))
                PhoneGroupCard {
                    Text(l.meaning.isEmpty ? Words.text("No description yet.") : l.meaning)
                        .phoneText(PhoneType.body).foregroundStyle(l.meaning.isEmpty ? WhooshPalette.secondaryLabel : WhooshPalette.label)
                        .fixedSize(horizontal: false, vertical: true)
                        .padding(PhoneMetrics.gutter).frame(maxWidth: .infinity, alignment: .leading)
                        .accessibilityIdentifier("iphone-labels-contacts.label.meaning")
                }
                PhoneSectionTitle(Words.text("Mail"))
                PhoneGroupCard { mailDoor(l); PhoneHair(); pinRow(l) }
                if let failure = state.pinFailure {
                    PhoneNote(words: failure, tint: AnyShapeStyle(WhooshPalette.warning))
                        .accessibilityIdentifier("iphone-labels-contacts.label.pin.failure")
                }
                PhoneSectionTitle(Words.text("Rules"))
                ForEach(l.cards) { c in
                    PhoneGroupCard { LabelCardPhone(card: c) }
                }
                PhoneNote(words: Words.text("Rules are edited on the Mac."))
                    .accessibilityIdentifier("iphone-labels-contacts.label.edit-on-mac")
            }
            .padding(.bottom, PhoneMetrics.listBottomRoom)
        }
        .scrollIndicators(.hidden)
        .animation(reduceMotion ? nil : WhooshMotion.newRow, value: state.pinFailure)
    }

    private func mailDoor(_ l: PhoneLabelRead) -> some View {
        Button { state.push(.mail(l.id)) } label: {
            HStack(spacing: WhooshSpacing.s12) {
                Image(systemName: "tray.full").foregroundStyle(WhooshPalette.accent).frame(width: WhooshSpacing.s24)
                Text(Words.text("Mail")).foregroundStyle(WhooshPalette.label)
                Spacer()
                Text(PhoneWords.count(l.mail)).foregroundStyle(WhooshPalette.sidebarSecondaryLabel).contentTransition(.numericText())
                Image(systemName: "chevron.right").font(.system(size: PhoneType.footnote.size, weight: .semibold)).foregroundStyle(WhooshPalette.tertiaryLabel)
            }
            .phoneText(PhoneType.body).padding(.horizontal, PhoneMetrics.gutter)
            .frame(maxWidth: .infinity, minHeight: PhoneMetrics.groupRow).contentShape(Rectangle())
        }
        .buttonStyle(PhoneRowPress())
        .accessibilityIdentifier("iphone-labels-contacts.label.mail")
    }

    private func pinRow(_ l: PhoneLabelRead) -> some View {
        HStack(spacing: WhooshSpacing.s12) {
            Image(systemName: l.pinned ? "pin.fill" : "pin").foregroundStyle(WhooshPalette.accent).frame(width: WhooshSpacing.s24)
            VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                Text(Words.text("Pinned")).phoneText(PhoneType.body).foregroundStyle(WhooshPalette.label)
                Text(Words.text("A pinned label is a tab at the top of the list.")).phoneText(PhoneType.footnote)
                    .foregroundStyle(WhooshPalette.sidebarSecondaryLabel).fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: WhooshSpacing.s8)
            Toggle(Words.text("Pinned"), isOn: Binding(get: { l.pinned }, set: { _ = state.setPinned(l.id, $0, m) }))
                .labelsHidden().frame(minHeight: PhoneMetrics.roundButton).accessibilityIdentifier("iphone-labels-contacts.label.pin")
        }
        .padding(.horizontal, PhoneMetrics.gutter).padding(.vertical, WhooshSpacing.s8)
        .frame(maxWidth: .infinity, minHeight: PhoneMetrics.groupRow)
    }
}
