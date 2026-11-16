import SwiftUI

/// The pieces every page of this screen shares, in the inbox's own metrics: a grouped card, a section title, a pressed row, Back.
enum LabelsPhoneMetrics {
    static let colourDot: CGFloat = WhooshSpacing.s12      // a label's colour as a dot, never an edge
    static let portrait: CGFloat = 72                       // the contact page's portrait circle (listed in docs/lanes/labelscontactsphone.md)
    static let nest: CGFloat = WhooshSpacing.s20            // one tree level of indent on the labels list
}

/// `.cardL` of the Mailboxes screen: rounded 24, the grouped ground's card fill, the gutter on both sides.
struct PhoneGroupCard<C: View>: View {
    @ViewBuilder var content: C
    var body: some View {
        VStack(spacing: 0) { content }
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(PhoneGround.card, in: RoundedRectangle(cornerRadius: PhoneMetrics.cardRadius, style: .continuous))
            .clipShape(RoundedRectangle(cornerRadius: PhoneMetrics.cardRadius, style: .continuous))
            .padding(.horizontal, PhoneMetrics.gutter)
    }
}

/// A section title over a card (Mailboxes' `section`): footnote, emphasised, secondary.
struct PhoneSectionTitle: View {
    let title: String
    var trailing = ""
    init(_ title: String, trailing: String = "") { self.title = title; self.trailing = trailing }
    var body: some View {
        HStack { Text(title); Spacer(); Text(trailing).contentTransition(.numericText()) }
            .phoneText(PhoneType.footnote, emphasized: true).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
            .padding(.horizontal, PhoneMetrics.gutter * 2).padding(.top, WhooshSpacing.s12)
            .accessibilityAddTraits(.isHeader)
    }
}

/// One plain grey line under a card.
struct PhoneNote: View {
    let words: String
    var tint: AnyShapeStyle = AnyShapeStyle(WhooshPalette.sidebarSecondaryLabel)
    var body: some View {
        Text(words).phoneText(PhoneType.footnote).foregroundStyle(tint)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, PhoneMetrics.gutter * 2)
    }
}

/// A row's press: the fill answers on the touch's own frame, no delay.
struct PhoneRowPress: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.background(configuration.isPressed ? PhoneGround.trackFill : Color.clear)
    }
}

/// The hairline between two rows of a card, inset to the text.
struct PhoneHair: View {
    var inset: CGFloat = PhoneMetrics.gutter
    var body: some View { Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator / 2).padding(.leading, inset) }
}

/// The glass Back button over the paper, 44 pt (the inbox's `.rb`).
struct PhoneBackButton: View {
    var action: () -> Void
    var body: some View {
        HStack {
            GlassCircle(symbol: "chevron.left", side: PhoneMetrics.roundButton, label: Words.text("Back"), action: action)
                .accessibilityIdentifier("iphone-labels-contacts.back")
            Spacer()
        }
        .padding(.horizontal, PhoneMetrics.gutter)
    }
}

/// A page's large title with its optional leading colour dot.
struct PhoneLargeTitle: View {
    let text: String
    var dot: AnyShapeStyle?
    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s10) {
            if let dot {
                Circle().fill(dot).frame(width: LabelsPhoneMetrics.colourDot, height: LabelsPhoneMetrics.colourDot).accessibilityHidden(true)
            }
            Text(text).phoneText(PhoneType.largeTitle).foregroundStyle(WhooshPalette.label).lineLimit(2)
        }
        .padding(.horizontal, PhoneMetrics.gutter).padding(.top, PhoneMetrics.titleTop)
        .accessibilityElement(children: .combine).accessibilityAddTraits(.isHeader)
    }
}

enum LabelColourPhone {
    /// "#RRGGBB" as a fill; nil or a malformed string is the quiet grey (a label with no colour has none).
    static func style(_ hex: String?) -> AnyShapeStyle {
        guard let hex, hex.hasPrefix("#"), hex.count == 7, let v = UInt32(hex.dropFirst(), radix: 16) else { return AnyShapeStyle(WhooshPalette.tertiaryLabel) }
        return AnyShapeStyle(Color(red: Double((v >> 16) & 0xFF) / 255, green: Double((v >> 8) & 0xFF) / 255, blue: Double(v & 0xFF) / 255))
    }
}

enum PhoneWords {
    static func count(_ n: Int) -> String { n == 1 ? LabelsWords.oneMessage : LabelsWords.messages(n) }
}
