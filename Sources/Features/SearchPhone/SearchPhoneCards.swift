import SwiftUI

/// A filter picker: a glass card over the chips, 44 pt lines, the picked one ticked. A pick narrows the rows on the same frame and
/// puts the card away. Accounts carry their small tile; senders are the people in the rows now; dates are the twelve months.
struct SearchPhoneMenuCard: View {
    let model: SearchPhoneModel
    let menu: SearchPhoneMenu

    var body: some View {
        let options = SearchPhoneRows.menuOptions(menu)
        ScrollView {
            LazyVStack(spacing: 0) {
                ForEach(options, id: \.id) { o in line(o) }
            }
        }
        .scrollIndicators(.hidden)
        .frame(maxHeight: SearchPhoneMetrics.pickerMaxHeight)
        .fixedSize(horizontal: false, vertical: options.count <= SearchPhoneMetrics.pickerRows)
        .padding(.vertical, WhooshSpacing.s4)
        .whooshGlass(in: RoundedRectangle(cornerRadius: PhoneMetrics.cardRadius, style: .continuous))
        .padding(.horizontal, PhoneMetrics.gutter)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("iphone-search.filter.menu")
    }

    private func line(_ o: SearchPhoneOption) -> some View {
        Button { pick(o) } label: {
            HStack(spacing: WhooshSpacing.s12) {
                if let id = o.accountId { PhoneTile(account: PhoneAccounts.of(id, in: AppModel.shared), small: true) }
                Text(o.title).phoneText(PhoneType.body, emphasized: o.on).foregroundStyle(WhooshPalette.label).lineLimit(1)
                Spacer(minLength: 0)
                if o.on { Image(systemName: "checkmark").font(.system(size: PhoneType.headline.size, weight: .semibold)).foregroundStyle(WhooshPalette.selection) }
            }
            .padding(.horizontal, PhoneMetrics.gutter).frame(minHeight: PhoneMetrics.mailboxRow).contentShape(Rectangle())
        }
        .buttonStyle(SearchRowPressStyle())
        .accessibilityAddTraits(o.on ? .isSelected : [])
        .accessibilityIdentifier("iphone-search.filter.\(menu.rawValue).\(o.id)")
    }

    private func pick(_ o: SearchPhoneOption) {
        switch menu {
        case .account: _ = model.setAccount(o.id == "all" ? nil : o.id)
        case .sender: _ = model.setSender(Int(o.id) ?? -1)
        case .date: _ = model.setMonth(o.id)
        }
    }
}

/// "Name the smart folder": the field, Save and Cancel, and the refusal in words beside them. Save is the keyboard's Return too.
struct SearchPhoneNameCard: View {
    @Bindable var model: SearchPhoneModel
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s4) {
            HStack(spacing: WhooshSpacing.s8) {
                Image(systemName: "folder.badge.plus").foregroundStyle(WhooshPalette.accent)
                TextField(Words.text("Name the smart folder"), text: $model.name)
                    .focused($focused).phoneText(PhoneType.body).submitLabel(.done)
                    .onSubmit { _ = model.confirmName() }
                    .accessibilityIdentifier("iphone-search.save.field")
                Button(Words.text("Cancel")) { _ = model.cancelNaming() }
                    .buttonStyle(PressStyle()).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                    .frame(minWidth: PhoneMetrics.roundButton, minHeight: PhoneMetrics.roundButton).contentShape(Rectangle())
                    .accessibilityIdentifier("iphone-search.save.cancel")
                Button(Words.text("Save")) { _ = model.confirmName() }
                    .buttonStyle(PressStyle()).phoneText(PhoneType.subhead, emphasized: true).foregroundStyle(WhooshPalette.selection)
                    .frame(minWidth: PhoneMetrics.roundButton, minHeight: PhoneMetrics.roundButton).contentShape(Rectangle())
                    .accessibilityIdentifier("iphone-search.save.confirm")
            }
            if !model.fieldSays.isEmpty {
                Text(model.fieldSays).phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.warning).lineLimit(2)
                    .accessibilityIdentifier("iphone-search.save.says")
            }
        }
        .padding(.leading, PhoneMetrics.gutter).padding(.trailing, WhooshSpacing.s4).padding(.vertical, WhooshSpacing.s4)
        .whooshGlass(in: RoundedRectangle(cornerRadius: PhoneMetrics.cardRadius, style: .continuous))
        .padding(.horizontal, PhoneMetrics.gutter)
        .onAppear { focused = true }
    }
}

/// The meaning model is not on the phone: one amber dot, one plain line, the door (the AI setup's own fix, the Mac's line made 44 pt).
/// While the model downloads the line becomes the AI setup's progress words and the door rests.
struct SearchPhoneMeaningCard: View {
    let model: SearchPhoneModel

    var body: some View {
        let working = model.meaning.state == .working
        HStack(spacing: WhooshSpacing.s8) {
            Circle().fill(WhooshPalette.warning).frame(width: WhooshMetrics.unreadDot, height: WhooshMetrics.unreadDot)
            Text(model.meaningWords).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.label).lineLimit(3)
                .accessibilityIdentifier("iphone-search.meaning.text")
            Spacer(minLength: 0)
            if !working {
                Button { _ = model.pressDoor() } label: {
                    Text(model.meaningButton).phoneText(PhoneType.subhead, emphasized: true).foregroundStyle(WhooshPalette.selection)
                        .padding(.horizontal, WhooshSpacing.s8).frame(minHeight: PhoneMetrics.roundButton).contentShape(Rectangle())
                }
                .buttonStyle(PressStyle()).accessibilityIdentifier("iphone-search.meaning.download")
            }
        }
        .padding(.leading, PhoneMetrics.gutter).padding(.trailing, WhooshSpacing.s6).padding(.vertical, WhooshSpacing.s4)
        .whooshGlass(in: RoundedRectangle(cornerRadius: PhoneMetrics.cardRadius, style: .continuous))
        .padding(.horizontal, PhoneMetrics.gutter)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("iphone-search.meaning")
    }
}
