import SwiftUI

/// Geometry this screen adds to `PhoneMetrics` (the inbox's own numbers; nothing else is invented here). Both are listed in
/// docs/lanes/searchphone.md for the constants ledger.
enum SearchPhoneMetrics {
    /// The skeleton's bar widths per row (sender, time, subject): the first six of the Mac skeleton's board set (SearchSkeleton.swift),
    /// so the first answer shows six rows; the bars are 9 pt tall, the Mac's value.
    static let skeletonBars: [(sender: CGFloat, time: CGFloat, subject: CGFloat)] = [(96, 38, 232), (118, 34, 196), (84, 40, 248), (132, 36, 170), (104, 38, 214), (90, 34, 236)]
    static let skeletonBarHeight: CGFloat = 9
    /// the accent's strength behind a chip that holds a value or has its picker open
    static let chipTint = 0.14
    /// a filter picker shows this many 44 pt lines before it scrolls
    static let pickerRows = 6
    static var pickerMaxHeight: CGFloat { PhoneMetrics.mailboxRow * CGFloat(pickerRows) }
    /// the last result clears the field, the chips and the cards above them
    static var barRoom: CGFloat { PhoneMetrics.listBottomRoom + PhoneMetrics.roundButton + PhoneMetrics.barGap }
}

/// One result: the inbox row's anatomy at the inbox row's numbers (tile, sender, count, clip, time, subject, up to three lines), with
/// what the Mac's search row adds: the matched words softly tinted in the subject and the passage, and, when the match is inside a file,
/// the attachment's name with a paperclip. The passage is the core's snippet; it replaces the preview line.
struct SearchPhoneRow: View {
    let result: SearchResult
    let account: PhoneAccount
    let terms: [String]
    @Environment(\.colorScheme) private var scheme

    private var row: InboxRow { result.row }
    /// a match has its typed letters in bold; a related mail and the rest do not
    private var isMatch: Bool { result.part == .match }
    /// the matched words' tint: the accent, soft (the Mac's `ResultRow` values)
    private var tint: Color { WhooshPalette.accent.color(scheme).opacity(scheme == .dark ? 0.24 : 0.13) }

    var body: some View {
        let passage = SearchPhoneWords.passage(result)
        ZStack(alignment: .topLeading) {
            VStack(alignment: .leading, spacing: 0) {
                HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s6) {
                    Text(isMatch ? SearchPhoneFound.marked(row.senderName, result.senderMarks, tint: tint) : AttributedString(row.senderName))
                        .phoneText(PhoneType.headline).foregroundStyle(WhooshPalette.label).lineLimit(1)
                    if row.threadCount > 1 {
                        Text("\(row.threadCount)").phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                    }
                    Spacer(minLength: WhooshSpacing.s6)
                    meta
                }
                Text(isMatch ? SearchPhoneFound.marked(row.subject, SearchPhoneWords.subjectSpans(result, terms), tint: tint) : ResultRow.tinted(row.subject, SearchPhoneWords.subjectSpans(result, terms), tint))
                    .phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.label).lineLimit(1)
                if let f = result.found {
                    foundLines(f)                                  // the passage that made the mail a result, with where it is from (SPEC A152)
                } else if let name = result.attachmentName { attachment(name) }
                if result.found == nil, !passage.isEmpty {
                    Text(ResultRow.tinted(passage, SearchPhoneWords.spans(terms, in: passage), tint))
                        .phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                        .lineLimit(result.attachmentName == nil ? PhoneMetrics.previewLines : PhoneMetrics.previewLines - 1)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .padding(.leading, PhoneMetrics.rowLeading).padding(.trailing, PhoneMetrics.gutter)
            .padding(.top, PhoneMetrics.rowTop).padding(.bottom, PhoneMetrics.rowBottom)
            .frame(maxWidth: .infinity, alignment: .leading)

            if row.unread {
                Circle().fill(WhooshPalette.accent).frame(width: PhoneMetrics.unreadDot, height: PhoneMetrics.unreadDot)
                    .offset(x: PhoneMetrics.unreadX, y: PhoneMetrics.unreadY).accessibilityHidden(true)
            }
            PhoneTile(account: account, unread: row.unread).offset(x: PhoneMetrics.tileX, y: PhoneMetrics.tileY)
        }
        .background(PhoneGround.list)
        .overlay(alignment: .bottom) {
            Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator / 2).padding(.leading, PhoneMetrics.rowLeading)
        }
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityValue(row.unread ? Words.text("Unread") : "")
    }

    /// The third line (the Mac's `ResultRow.lineThree`, at the phone's row): a paperclip and the file's name, or an arrow and the earlier mail's sender and
    /// date, then the passage (the typed letters in bold on a match, in quotes on a related mail). A text passage stands alone.
    private func foundLines(_ f: SearchFound) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            if !f.lead.isEmpty {
                HStack(spacing: WhooshSpacing.s4) {
                    if f.source == .attachment { Image(systemName: "paperclip").font(.system(size: PhoneType.footnote.size)) }
                    if f.source == .thread { ThreadArrow().stroke(style: StrokeStyle(lineWidth: 1.8, lineCap: .round, lineJoin: .round)).frame(width: 12, height: 12) }
                    Text(f.lead).phoneText(PhoneType.subhead, emphasized: true).foregroundStyle(WhooshPalette.label).lineLimit(1).truncationMode(.middle)
                }
                .accessibilityIdentifier("iphone-search.results.found.lead")
            }
            if !f.passage.isEmpty {
                Text(f.quoted ? AttributedString("\u{201C}" + f.passage + "\u{201D}") : SearchPhoneFound.marked(f.passage, f.marks, tint: tint))
                    .phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                    .lineLimit(f.lead.isEmpty ? PhoneMetrics.previewLines : PhoneMetrics.previewLines - 1)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .foregroundStyle(WhooshPalette.secondaryLabel)
    }

    /// The file the words matched in: its name tinted, and where in it ("in the attachment", or why only its name was read).
    private func attachment(_ name: String) -> some View {
        HStack(spacing: WhooshSpacing.s4) {
            Image(systemName: "paperclip").font(.system(size: PhoneType.footnote.size))
            Text(ResultRow.tinted(name, result.attachmentMarks, tint)).phoneText(PhoneType.subhead).lineLimit(1).truncationMode(.middle)
            if let says = result.attachmentSays {
                Text(says).phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.tertiaryLabel).lineLimit(1).fixedSize()
            }
        }
        .foregroundStyle(WhooshPalette.secondaryLabel)
    }

    private var meta: some View {
        HStack(spacing: WhooshSpacing.s4) {
            if row.pinned { Image(systemName: "pin").font(.system(size: PhoneType.footnote.size)) }
            if row.attachmentCount > 0 && result.attachmentName == nil { Image(systemName: "paperclip").font(.system(size: PhoneType.footnote.size)) }
            Text(PhoneTime.words(row.receivedAtMs)).phoneText(PhoneType.subhead)
            Image(systemName: "chevron.right").font(.system(size: PhoneType.footnote.size - 1, weight: .semibold))
                .foregroundStyle(WhooshPalette.tertiaryLabel)
        }
        .foregroundStyle(WhooshPalette.secondaryLabel).fixedSize()
    }
}

/// A press tints the row on the input frame, no delay: the row holds no buttons, so the whole row is the one target.
struct SearchRowPressStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.background(configuration.isPressed ? AnyShapeStyle(WhooshPalette.hover) : AnyShapeStyle(Color.clear))
    }
}
