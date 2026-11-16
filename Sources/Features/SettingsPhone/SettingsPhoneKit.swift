import SwiftUI

/// The pieces every page of the iPhone Settings is made of. They reuse the inbox's grounds, type roles, glass and press.

/// The coloured rounded square (the Mac's `SettingsPaneIcon`, at phone size): a white symbol on the hue, a soft top light.
struct PhonePaneIcon: View {
    let symbol: String
    let hue: ThemeColor
    var size: CGFloat = SettingsPhoneMetrics.paneIcon
    var radius: CGFloat = SettingsPhoneMetrics.paneIconRadius
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let h = hue.value(scheme)
        let top = Color(.sRGB, red: h.red * 0.82 + 0.18, green: h.green * 0.82 + 0.18, blue: h.blue * 0.82 + 0.18)
        RoundedRectangle(cornerRadius: radius, style: .continuous)
            .fill(LinearGradient(colors: [top, h.color], startPoint: .top, endPoint: .bottom))
            .overlay(Image(systemName: symbol).font(.system(size: size * 0.5, weight: .medium)).foregroundStyle(Color.white))
            .frame(width: size, height: size)
            .accessibilityHidden(true)
    }
}

/// A row darkens on the input frame and comes back with the finger: feedback with no delay and no completion handler.
struct SettingsRowPress: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .background(Color.black.opacity(configuration.isPressed ? SettingsPhoneMetrics.pressDim : 0))
    }
}

/// A group's name above its card, in the Mailboxes screen's section voice.
struct SettingsPhoneSection: View {
    let title: String
    var body: some View {
        Text(title).phoneText(PhoneType.footnote, emphasized: true).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, PhoneMetrics.gutter * 2).padding(.top, WhooshSpacing.s12)
            .accessibilityAddTraits(.isHeader)
    }
}

/// A one-line note under a card.
struct SettingsPhoneNote: View {
    let text: String
    var body: some View {
        Text(text).phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, PhoneMetrics.gutter * 2)
    }
}

/// The rounded card of a group: the grouped ground's own card colour, clipped so a pressed row keeps the corners.
struct SettingsPhoneCard<Content: View>: View {
    @ViewBuilder var content: Content
    var body: some View {
        let shape = RoundedRectangle(cornerRadius: PhoneMetrics.cardRadius, style: .continuous)
        VStack(spacing: 0) { content }
            .background(PhoneGround.card, in: shape)
            .clipShape(shape)
            .padding(.horizontal, PhoneMetrics.gutter)
    }
}

/// The hairline between two rows, inset to the text.
struct SettingsPhoneDivider: View {
    var inset: CGFloat = PhoneMetrics.gutter
    var body: some View {
        Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator / 2).padding(.leading, inset)
    }
}

/// One row of the home list: icon, name, a value, the chevron. The whole row is the target.
struct SettingsPhoneRow: View {
    let page: SettingsPhonePage
    var value: String = ""
    var attention = false
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: WhooshSpacing.s12) {
                PhonePaneIcon(symbol: page.symbol, hue: page.hue)
                Text(page.title).phoneText(PhoneType.body).foregroundStyle(WhooshPalette.label).lineLimit(1)
                Spacer(minLength: WhooshSpacing.s8)
                if attention {
                    Circle().fill(WhooshPalette.warning).frame(width: PhoneMetrics.unreadDot, height: PhoneMetrics.unreadDot).accessibilityHidden(true)
                }
                if !value.isEmpty {
                    Text(value).phoneText(PhoneType.body).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                }
                Image(systemName: "chevron.right").font(.system(size: PhoneType.footnote.size, weight: .semibold)).foregroundStyle(WhooshPalette.tertiaryLabel)
            }
            .padding(.horizontal, PhoneMetrics.gutter).frame(minHeight: SettingsPhoneMetrics.row).contentShape(Rectangle())
        }
        .buttonStyle(SettingsRowPress())
        .accessibilityIdentifier("iphone-settings.row.\(page.rawValue)")
    }
}

/// One row of a choice list: the name, a check on the one picked.
struct SettingsPhoneChoice: View {
    let title: String
    let on: Bool
    let ax: String
    let action: () -> Void
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        Button(action: action) {
            HStack(spacing: WhooshSpacing.s12) {
                Text(title).phoneText(PhoneType.body).foregroundStyle(WhooshPalette.label).lineLimit(1)
                Spacer(minLength: WhooshSpacing.s8)
                Image(systemName: "checkmark").font(.system(size: PhoneType.body.size, weight: .semibold)).foregroundStyle(WhooshPalette.accent)
                    .opacity(on ? 1 : 0)
            }
            .padding(.horizontal, PhoneMetrics.gutter).frame(minHeight: SettingsPhoneMetrics.choiceRow).contentShape(Rectangle())
        }
        .buttonStyle(SettingsRowPress())
        .animation(reduceMotion ? nil : .easeOut(duration: WhooshMotion.hoverTint), value: on)
        .accessibilityAddTraits(on ? .isSelected : [])
        .accessibilityIdentifier(ax)
    }
}

/// A page: the paper-grey grouped ground, the large title under a glass Back button, the content, room at the foot.
struct SettingsPhoneFrame<Content: View>: View {
    let title: String
    let titleAx: String
    let onBack: () -> Void
    @ViewBuilder var content: Content

    var body: some View {
        ZStack(alignment: .top) {
            PhoneGround.grouped.ignoresSafeArea()
            ScrollView {
                VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
                    Text(title).phoneText(PhoneType.largeTitle).foregroundStyle(WhooshPalette.label)
                        .padding(.horizontal, PhoneMetrics.gutter).padding(.top, PhoneMetrics.titleTop)
                        .padding(.bottom, SettingsPhoneMetrics.titleBottom)
                        .accessibilityAddTraits(.isHeader).accessibilityIdentifier(titleAx)
                    content
                }
                .padding(.bottom, PhoneMetrics.listBottomRoom)
            }
            .scrollIndicators(.hidden)
        }
        .overlay(alignment: .topLeading) {
            GlassCircle(symbol: "chevron.left", side: PhoneMetrics.roundButton, label: SettingsPhoneWords.back, action: onBack)
                .accessibilityIdentifier("iphone-settings.back").padding(.horizontal, PhoneMetrics.gutter)
        }
        .phoneBarHidden()
    }
}
