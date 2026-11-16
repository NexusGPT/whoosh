import SwiftUI

/// The row tile (`.tile`): the account mark at phone size. The same mark as the Mac's AccountMark (the account's
/// icon inside a ring in its colour, never a fill; dashed ring + orange dot = needs a sign-in), drawn as the
/// rounded square the glass-light board gives the 56 pt and 40 pt tiles. Accessibility: the account name.
struct PhoneTile: View {
    let account: PhoneAccount
    var small = false
    var unread = false

    private var side: CGFloat { small ? PhoneMetrics.tileSmall : PhoneMetrics.tile }
    private var radius: CGFloat { small ? PhoneMetrics.tileRadiusSmall : PhoneMetrics.tileRadius }
    private var stroke: CGFloat { small ? PhoneMetrics.tileStrokeSmall : PhoneMetrics.tileStroke }
    private var dash: [CGFloat] { account.needsAttention ? [CGFloat(WhooshConstants.ACCOUNT_RING_DASH_PX)] : [] }

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: radius, style: .continuous)
        ZStack {
            shape.fill(PhoneGround.list)
            shape.strokeBorder(AccountColour.ring(account.colour), style: StrokeStyle(lineWidth: stroke, dash: dash))
            Image(systemName: account.symbol)
                .font(.system(size: small ? PhoneMetrics.tileGlyphSmall : PhoneMetrics.tileGlyph, weight: .regular))
                .foregroundStyle(unread ? AnyShapeStyle(WhooshPalette.label) : AnyShapeStyle(WhooshPalette.secondaryLabel))
        }
        .frame(width: side, height: side)
        .overlay(alignment: .topTrailing) {
            if account.needsAttention {
                Circle().fill(WhooshPalette.warning).frame(width: PhoneMetrics.tileBadge, height: PhoneMetrics.tileBadge)
                    .padding(WhooshSpacing.s2).background(Circle().fill(PhoneGround.list))
                    .offset(x: PhoneMetrics.tileBadge / 2 + WhooshSpacing.s2, y: -(PhoneMetrics.tileBadge / 2 + WhooshSpacing.s2))
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(account.markLabel)
        .accessibilityValue(account.needsAttention ? "\(account.address), needs a new sign-in" : account.address)
        .accessibilityIdentifier("account-mark")
    }
}

/// One mail row (`.r .in`): tile, sender + time, subject, a three-line preview. The row carries no buttons:
/// its actions live in the swipes and the long-press menu (board restraint rule).
struct PhoneRow: View {
    let row: InboxRow
    let account: PhoneAccount
    var selecting = false
    var selected = false

    var body: some View {
        ZStack(alignment: .topLeading) {
            VStack(alignment: .leading, spacing: 0) {
                HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s6) {
                    Text(row.senderName).phoneText(PhoneType.headline).foregroundStyle(WhooshPalette.label).lineLimit(1)
                    if row.threadCount > 1 {
                        Text("\(row.threadCount)").phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                    }
                    Spacer(minLength: WhooshSpacing.s6)
                    meta
                }
                Text(row.subject).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.label).lineLimit(1)
                Text(row.preview).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                    .lineLimit(PhoneMetrics.previewLines).fixedSize(horizontal: false, vertical: true)
            }
            .padding(.leading, PhoneMetrics.rowLeading).padding(.trailing, PhoneMetrics.gutter)
            .padding(.top, PhoneMetrics.rowTop).padding(.bottom, PhoneMetrics.rowBottom)
            .frame(maxWidth: .infinity, alignment: .leading)

            if selecting {
                Image(systemName: selected ? "checkmark.circle.fill" : "circle").font(.system(size: PhoneMetrics.unreadDot * 2))
                    .foregroundStyle(selected ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(WhooshPalette.tertiaryLabel))
                    .offset(x: WhooshSpacing.s2, y: PhoneMetrics.unreadY - WhooshSpacing.s4)
            } else if row.unread {
                Circle().fill(WhooshPalette.accent).frame(width: PhoneMetrics.unreadDot, height: PhoneMetrics.unreadDot)
                    .offset(x: PhoneMetrics.unreadX, y: PhoneMetrics.unreadY).accessibilityHidden(true)
            }
            PhoneTile(account: account, unread: row.unread).offset(x: PhoneMetrics.tileX, y: PhoneMetrics.tileY)
        }
        .background(PhoneGround.list)
        .overlay(alignment: .bottom) {
            Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator / 2).padding(.leading, PhoneMetrics.rowLeading)
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityValue(row.unread ? "unread" : "")
    }

    private var meta: some View {
        HStack(spacing: WhooshSpacing.s4) {
            if row.pinned { Image(systemName: "pin").font(.system(size: PhoneType.footnote.size)) }
            if row.attachmentCount > 0 { Image(systemName: "paperclip").font(.system(size: PhoneType.footnote.size)) }
            Text(PhoneTime.words(row.receivedAtMs)).phoneText(PhoneType.subhead)
            Image(systemName: "chevron.right").font(.system(size: PhoneType.footnote.size - 1, weight: .semibold))
                .foregroundStyle(WhooshPalette.tertiaryLabel)
        }
        .foregroundStyle(WhooshPalette.secondaryLabel).fixedSize()
    }
}

/// Today "09:12", yesterday "Yesterday", this week the weekday, older "4 Nov" (the fixture's words).
enum PhoneTime {
    /// The app speaks English on both devices (the Mac list formats with en_GB too).
    static let locale = Locale(identifier: "en_GB")
    static func words(_ ms: Int64, now: Date = Date(), calendar: Calendar = .current) -> String {
        let d = Date(timeIntervalSince1970: Double(ms) / 1000)
        if calendar.isDate(d, inSameDayAs: now) { return d.formatted(.dateTime.hour(.twoDigits(amPM: .omitted)).minute().locale(locale)) }
        if calendar.isDateInYesterday(d) { return "Yesterday" }
        if let days = calendar.dateComponents([.day], from: d, to: now).day, days < 7 { return d.formatted(.dateTime.weekday(.abbreviated).locale(locale)) }
        return d.formatted(.dateTime.day().month(.abbreviated).locale(locale))
    }
}

extension View {
    /// A phone type role: the ramp's font, tabular figures and the board's line height.
    func phoneText(_ style: TextStyleToken, emphasized: Bool = false) -> some View { whooshText(style, emphasized: emphasized) }
}
