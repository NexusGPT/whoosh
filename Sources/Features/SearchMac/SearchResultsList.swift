import SwiftUI

/// The results in the inbox's own 44 pt row: the unread dot, the account mark, sender, count, time, then the subject with the matched words
/// softly tinted and, only when the match is inside it, the attachment's name with a paperclip. One row per thread, in three parts (SPEC A150): the matches,
/// then the thin line "Related" and the related mail, then the closed line "All other mail · N" that opens the rest. A result found in the text, an
/// attachment or an earlier mail of the thread has a third line with the passage (SPEC A152, the drawing's P1).
struct SearchResultsList: View {
    let model: SearchMacModel
    @Environment(\.colorScheme) private var scheme

    static let topCount = 3
    /// the line under the results; the socket's `mac-search.footer` row serves the same words
    static var footerLine: String { Words.text("Matched by words and by meaning, attachments included.") }
    static var topTitle: String { Words.text("Top results") }
    static var allTitle: String { Words.text("All results") }
    /// the thin line before the related mail (the drawing's `.sx-div`)
    static var relatedTitle: String { Words.text("Related") }
    /// the closed line's words (the count follows it: "All other mail · 17")
    static var moreTitle: String { Words.text("All other mail") }

    /// What stands in the place of "Related" by how the engine made the answer (`SearchMacModel.stage`): while the ranking is still being made a line that
    /// says so, with the other mails not listed yet; with no meaning order (no model, or it failed) no mail can be related, so the date order says so above the
    /// closed line; else "Related" when there is related mail, and no line when there is none.
    static func dividerTitle(_ stage: String, related: Int = 0) -> String? {
        switch stage {
        case "words": return Words.text("Ranking the other mails by meaning…")
        case "no_model", "model_failed": return Words.text("Other mails, by date")
        default: return related > 0 ? relatedTitle : nil
        }
    }

    /// The line drawn above the closed line (the `mac-search.sections` row): there when related mail is listed, when the ranking is still being made, or when
    /// the rest is listed by date.
    static func titles(related: Int, rest: Int, stage: String = "", tailPending: Bool = false) -> [String] {
        switch stage {
        case "words": return tailPending ? dividerTitle(stage).map { [$0] } ?? [] : []
        // related mail listed is always under "Related" (the test engine's stand-in for meaning marks some with no model: main97 iPhone, the line said
        // "Other mails, by date" above related rows); with none, the date order says so above the closed line
        case "no_model", "model_failed": return related > 0 ? [relatedTitle] : rest > 0 ? dividerTitle(stage).map { [$0] } ?? [] : []
        default: return dividerTitle(stage, related: related).map { [$0] } ?? []
        }
    }

    /// While the meaning index is still being built: how far it is, said once above the rows (nil when it is not building).
    static func indexLine(_ s: MeaningIndexBuilder.Status) -> String? {
        guard IndexProgress.isBuilding(s) else { return nil }
        return Words.text("The meaning index is still being built: %@ of %@ mails.", IndexProgress.number(s.indexed), IndexProgress.number(IndexProgress.total(s)))
    }

    var body: some View {
        let rs = model.results
        let matches = model.matchedConversations, upper = model.upperCount
        let book = RowAccount.directory(AppModel.shared)
        let divider = Self.titles(related: model.relatedCount, rest: model.restCount, stage: model.stage, tailPending: model.tailPending).first
        ScrollViewReader { proxy in
            ScrollView {
                // lazy: the list holds every mail the engine sends, and only the rows on screen are built
                LazyVStack(alignment: .leading, spacing: 0) {
                    if !model.notes.isEmpty {
                        Text(model.notes.joined(separator: " · ")).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                            .padding(.horizontal, WhooshSpacing.s12).padding(.bottom, WhooshSpacing.s4)
                            .accessibilityIdentifier("mac-search.notes")
                    }
                    if let line = Self.indexLine(IndexProgressModel.shared.status) { SearchNote(title: nil, line: line) }
                    if let sentence = SearchMacModel.stageSentence(model.stage) { SearchNote(title: nil, line: sentence, detail: model.stageReason, id: "mac-search.stage-reason") }
                    if upper == 0, !model.tailPending { SearchNoMatch() }   // no match and nothing related: the sentence in the middle, the closed line under it
                    // Lead, 13 Nov (owner A162, walk l120): ONE loop over every drawn line, each row identified by its MAIL. The rows were three
                    // loops over index ranges (matches, related, the rest), each row identified by its place: a second search with another
                    // split (2 matches and 3 related, then 10 matches) left the old related rows on screen at places 2 to 4, one still
                    // highlighted, while the list the keys act on held other mails.
                    ForEach(Self.lines(rs, matches: matches, upper: upper, divider: divider, showsMore: model.showsMore, restOpen: model.restOpen)) { line in
                        switch line.kind {
                        case .row(let i): row(i, rs, book)
                        case .divider(let title): SearchDivider(title: title)
                        case .more: SearchMoreLine(title: Self.moreTitle, count: model.restCount, open: model.restOpen, selected: model.onMore) { _ = model.toggleRest() }
                        }
                    }
                    Text(Self.footerLine).whooshText(WhooshTypography.caption)   // two lines in a narrow list, never cut (lead, 13 Nov, picture s1-135 at 960 points)
                        .foregroundStyle(WhooshPalette.tertiaryLabel).lineLimit(2).fixedSize(horizontal: false, vertical: true)
                        .padding(.horizontal, WhooshSpacing.s12).padding(.top, WhooshSpacing.s14)
                }
                .padding(.horizontal, 0).padding(.bottom, WhooshSpacing.s16)
            }
            .onChange(of: model.selected) { _, i in if !model.onMore { proxy.scrollTo(Self.rowId(model.results, i)) } }
            .onChange(of: model.onMore) { _, on in if on { proxy.scrollTo(Self.moreId) } else { proxy.scrollTo(Self.rowId(model.results, model.selected)) } }
        }
        .whooshAx("mac-search.results")   // the frame `measure mac-search.results` reads
    }

    private func row(_ i: Int, _ rs: [SearchResult], _ book: [String: RowAccount]) -> some View {
        ResultRow(index: i, result: rs[i], account: RowAccount.of(rs[i].row.accountId, in: book), selected: (model.picked.isEmpty ? i == model.selected : model.picked.contains(rs[i].row.threadId)) && !model.onMore,
                  edited: model.edited[rs[i].row.messageId]) { _ = model.openResult(i) }
            .whooshAx("mac-search.results.\(i)")   // a measurable frame, so a check can press the row with the real pointer (lead, 13 Nov: only the socket's closure "clicked" it)
    }

    /// One drawn line of the list: a result (by its place in the results), the line above the related mail, the closed line.
    struct Line: Identifiable {
        enum Kind { case row(Int), divider(String), more }
        let id: String
        let kind: Kind
    }

    static let moreId = "more"

    /// A row's identity: its mail. A mail listed twice (the engine should never send one) keeps a second identity by its place, so no two lines share one.
    static func rowId(_ rs: [SearchResult], _ i: Int) -> String {
        guard rs.indices.contains(i) else { return "r@\(i)" }
        let mail = rs[i].row.messageId
        return rs[..<i].contains { $0.row.messageId == mail } ? "r:\(mail)@\(i)" : "r:\(mail)"
    }

    /// The drawn lines, top to bottom: the matches, the line above the related mail, the related mail, the closed line, and the rest when it is open.
    static func lines(_ rs: [SearchResult], matches: Int, upper: Int, divider: String?, showsMore: Bool, restOpen: Bool) -> [Line] {
        let m = min(max(matches, 0), rs.count), u = min(max(upper, m), rs.count)
        var seen = Set<String>(), out: [Line] = []
        func add(_ i: Int) {
            let mail = rs[i].row.messageId
            out.append(Line(id: seen.insert(mail).inserted ? "r:\(mail)" : "r:\(mail)@\(i)", kind: .row(i)))
        }
        for i in 0..<m { add(i) }
        if let divider { out.append(Line(id: "divider", kind: .divider(divider))) }
        for i in m..<u { add(i) }
        if showsMore { out.append(Line(id: moreId, kind: .more)) }
        if restOpen { for i in u..<rs.count { add(i) } }
        return out
    }
}

struct ResultRow: View {
    let index: Int
    let result: SearchResult
    let account: RowAccount
    let selected: Bool
    /// what an action changed on this result since the search ran (`SearchMacModel.edited`): the row draws that, not the snapshot
    var edited: (unread: Bool?, pinned: Bool?)?
    let open: () -> Void
    @State private var hovering = false
    @Environment(\.colorScheme) private var scheme

    private var row: InboxRow { Self.shown(result.row, edited: edited) }

    /// The result's row with the unread state and the pin an action set on it (the toggles read the same, `SearchMacModel.selectedRow`).
    static func shown(_ row: InboxRow, edited: (unread: Bool?, pinned: Bool?)?) -> InboxRow {
        var r = row
        if let e = edited { r.unread = e.unread ?? r.unread; r.pinned = e.pinned ?? r.pinned }
        return r
    }
    private var primary: Color { selected ? WhooshPalette.onAccent.color(scheme) : WhooshPalette.label.color(scheme) }
    private var secondary: Color { selected ? WhooshPalette.onAccent.color(scheme).opacity(0.88) : WhooshPalette.secondaryLabel.color(scheme) }
    /// the matched words are bold, in the accent (as the row's own text over the selection)
    private var tint: Color? { selected ? nil : WhooshPalette.accent.color(scheme) }

    var body: some View {
        Button(action: open) {
            VStack(alignment: .leading, spacing: 0) { lineOne; lineTwo; lineThree }
                .padding(.leading, WhooshMetrics.listRowLeading + WhooshSpacing.s4).padding(.trailing, WhooshSpacing.s12)
                .frame(maxWidth: .infinity, minHeight: WhooshMetrics.listRow, alignment: .leading)
                .overlay(alignment: .topLeading) { marks }
                .background(RoundedRectangle(cornerRadius: WhooshRadius.row, style: .continuous)
                    .fill(selected ? AnyShapeStyle(WhooshPalette.selection) : hovering ? AnyShapeStyle(WhooshPalette.hover) : AnyShapeStyle(Color.clear)))
                .padding(.horizontal, WhooshMetrics.listRowInset)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { h in withAnimation(.easeOut(duration: Double(WhooshConstants.SEARCH_HOVER_TINT_MS) / 1000)) { hovering = h } }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(selected ? .isSelected : [])
    }

    private var marks: some View {
        HStack(spacing: WhooshSpacing.s4) {
            Circle().fill(row.unread ? (selected ? WhooshPalette.onAccent.color(scheme) : WhooshPalette.accent.color(scheme)) : .clear)
                .frame(width: WhooshMetrics.unreadDot, height: WhooshMetrics.unreadDot)
            EmblemMark(glyph: EmblemLibrary.resolve(account.iconId), colourHex: account.colour, size: .row,
                       state: account.needsAttention ? .needsSignIn : .live, name: account.name, address: account.address)
        }
        .padding(.leading, WhooshSpacing.s4 + WhooshMetrics.listRowInset).padding(.top, WhooshSpacing.s4 + WhooshSpacing.s2)
    }

    /// Lead, 13 Nov (owner A162, walk a116 finding 11): a mail the owner sent is a search result too (Sent is searched). Its row read no name at
    /// all on the test engine and the owner's own name on the real one; it reads "You", as the thread's cards do. A name the core does not hold
    /// falls back to the address: a row never starts with an empty line.
    private var ownMail: Bool {
        !row.senderAddress.isEmpty && row.senderAddress.caseInsensitiveCompare(account.address) == .orderedSame
    }

    private var lineOne: some View {
        HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s6) {
            (ownMail ? Text(ThreadCardCopy.you) : Text(Self.emphasised(row.senderName.isEmpty ? row.senderAddress : row.senderName, row.senderName.isEmpty ? [] : result.senderMarks, tint)))
                .font(WhooshTypography.body.font(emphasized: row.unread)).foregroundStyle(primary).lineLimit(1)
            if row.threadCount > 1 { Text("\(row.threadCount)").font(WhooshTypography.caption.font(emphasized: false)).foregroundStyle(secondary) }
            Spacer(minLength: WhooshSpacing.s8)
            Text(RowFacts.time(row.receivedAtMs)).font(WhooshTypography.caption.font(emphasized: false)).foregroundStyle(secondary).fixedSize()
        }
        .frame(height: WhooshMetrics.listRowLine).padding(.top, WhooshSpacing.s4)
    }

    private var lineTwo: some View {
        HStack(spacing: WhooshSpacing.s8 + WhooshSpacing.s2) {
            Text(Self.emphasised(row.subject, result.subjectMarks, tint)).font(WhooshTypography.subhead.font(emphasized: false))
                .foregroundStyle(row.unread ? primary : secondary).lineLimit(1).truncationMode(.tail)
                .background { cutProbe(row.subject, WhooshTypography.subhead, .tail, "subject") }
            Spacer(minLength: 0)
        }
        .frame(height: WhooshMetrics.listRowLine)
    }

    /// The file a match was found in, when the row has no passage line of its own (a mail found in its subject AND in an attachment's name, the
    /// not-downloaded and unreadable files): the drawing's P1 `.why` line, paperclip, the file's name, then what the file says. P1 puts the file on
    /// this third line, never in the date column (round 18, d1ffe36c clause 2: the name sat at the right of the subject line).
    @ViewBuilder private var fileLine: some View {
        if let name = result.attachmentName {
            HStack(spacing: WhooshSpacing.s4 + 1) {
                Image(systemName: "paperclip").font(.system(size: 11)).frame(width: 12)
                Text(Self.emphasised(name, result.attachmentMarks, tint)).font(WhooshTypography.subhead.font(emphasized: false)).fontWeight(.medium).foregroundStyle(primary)
                    .lineLimit(1).truncationMode(.middle)
                    .background { cutProbe(name, WhooshTypography.subhead, .middle, "attachment") }
                    .accessibilityIdentifier("mac-search.results.attachment")
                if let says = result.attachmentSays {
                    Text(says).font(WhooshTypography.subhead.font(emphasized: false)).lineLimit(1).truncationMode(.tail)
                        .accessibilityIdentifier("mac-search.results.attachment.says")
                }
                Spacer(minLength: 0)
            }
            .foregroundStyle(secondary)
            .frame(maxWidth: .infinity, minHeight: WhooshMetrics.listRowLine, maxHeight: WhooshMetrics.listRowLine, alignment: .leading)
            .padding(.bottom, WhooshSpacing.s6)
        }
    }

    /// The third line (SPEC A152, the drawing's P1 `.why`): the passage that made the mail a result, made once when the answer arrived (`SearchResult.found`).
    /// A text passage stands alone; an attachment's has a paperclip and the file's name before it; an earlier mail of the thread's has an arrow and that mail's
    /// sender and date. A match's passage has the typed letters in bold, a related mail's is in quotes. A result found in its subject or sender has none.
    @ViewBuilder private var lineThree: some View {
        if result.found == nil || (result.found?.source == .text && result.attachmentName != nil && result.matchedIn.hasPrefix("attachment")) { fileLine }
        else if let f = result.found {
            HStack(spacing: WhooshSpacing.s4 + 1) {
                switch f.source {
                case .attachment: Image(systemName: "paperclip").font(.system(size: 11)).frame(width: 12)
                case .thread: ThreadArrow().stroke(style: StrokeStyle(lineWidth: 1.8, lineCap: .round, lineJoin: .round)).frame(width: 12, height: 12)
                case .text: EmptyView()
                }
                if !f.lead.isEmpty {
                    Text(f.lead).font(WhooshTypography.subhead.font(emphasized: false)).fontWeight(.medium).foregroundStyle(primary).lineLimit(1).fixedSize()
                }
                if !f.passage.isEmpty {
                    Text(f.quoted ? AttributedString("“" + f.passage + "”") : Self.emphasised(f.passage, f.marks, tint))
                        .font(WhooshTypography.subhead.font(emphasized: false)).lineLimit(1).truncationMode(.tail)
                }
                Spacer(minLength: 0)
            }
            .foregroundStyle(secondary)
            .frame(maxWidth: .infinity, minHeight: WhooshMetrics.listRowLine, maxHeight: WhooshMetrics.listRowLine, alignment: .leading)
            .padding(.bottom, WhooshSpacing.s6)
            .accessibilityIdentifier("mac-search.results.\(index).found")
        }
    }

    /// What the socket's `labels` verb reads to say whether a result's text is cut: a hidden copy of the text at the size the shown one got,
    /// mounted only while that verb's switch is on (the same copy every `.whooshAx` leaf text carries; nothing is measured on a key or a scroll).
    /// `mac-search.results.<i>.<part>` is read back by SearchMacRows.
    @ViewBuilder
    private func cutProbe(_ text: String, _ style: TextStyleToken, _ mode: Text.TruncationMode, _ part: String) -> some View {
        if FrameProbe.gate.labels {
            Text(verbatim: text).font(style.font(emphasized: false)).lineLimit(1).truncationMode(mode).opacity(0)
                .whooshAx("mac-search.results.\(index).\(part)")
        }
    }

    /// The text with the matched spans in bold (and the accent when `colour` is given); the other glyphs stay as they are.
    static func emphasised(_ s: String, _ ranges: [Range<String.Index>], _ colour: Color?) -> AttributedString {
        var a = AttributedString(s)
        for r in ranges {
            guard let ar = Range(r, in: a) else { continue }
            a[ar].inlinePresentationIntent = .stronglyEmphasized
            if let colour { a[ar].foregroundColor = colour }
        }
        return a
    }

    /// The text with the matched spans on a soft background tint (the phone's rows).
    static func tinted(_ s: String, _ ranges: [Range<String.Index>], _ tint: Color) -> AttributedString {
        var a = AttributedString(s)
        for r in ranges { if let ar = Range(r, in: a) { a[ar].backgroundColor = tint } }
        return a
    }
}

/// The thin line where the second group starts: the words between two hairlines (the drawing's "Close in meaning").
struct SearchDivider: View {
    let title: String
    var body: some View {
        HStack(spacing: WhooshSpacing.s10) {
            line
            Text(title).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1).fixedSize()
            line
        }
        .padding(.horizontal, WhooshSpacing.s12).frame(height: 26)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("mac-search.divider")
    }
    private var line: some View { Rectangle().fill(WhooshPalette.separator).frame(height: WhooshGlass.rimWidth) }
}

/// A quiet note in a hairline box above the rows (the drawing's note): an optional bold line, then the words.
struct SearchNote: View {
    let title: String?
    let line: String
    /// the engine's own words, small, under the sentence, behind "Details" (as the add-account sheet shows its details)
    var detail: String = ""
    var id: String? = nil
    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
            if let title { Text(title).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label) }
            if !line.isEmpty { Text(line).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.secondaryLabel) }
            if !detail.isEmpty { Text(Words.text("Details") + ": " + detail).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.tertiaryLabel) }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, WhooshSpacing.s12).padding(.vertical, WhooshSpacing.s10)
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(WhooshPalette.separator, lineWidth: WhooshGlass.rimWidth))
        .padding(.horizontal, WhooshSpacing.s8).padding(.vertical, WhooshSpacing.s10)
        .accessibilityIdentifier(id ?? (title == nil ? "mac-search.index-note" : "mac-search.refused"))
    }
}

struct SectionLabel: View {
    let text: String
    init(_ text: String) { self.text = text }
    var body: some View {
        Text(text).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.tertiaryLabel)
    }
}

/// The arrow before an earlier mail's sender in a result's third line: the drawing's path (`THR`: M9 14 L4 9 l5-5, M4 9 h10 a6 6 0 0 1 6 6 v5) on its 24 pt grid.
struct ThreadArrow: Shape {
    func path(in rect: CGRect) -> Path {
        let k = min(rect.width, rect.height) / 24
        func pt(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: rect.minX + x * k, y: rect.minY + y * k) }
        var p = Path()
        p.move(to: pt(9, 14)); p.addLine(to: pt(4, 9)); p.addLine(to: pt(9, 4))
        p.move(to: pt(4, 9)); p.addLine(to: pt(14, 9))
        p.addArc(center: pt(14, 15), radius: 6 * k, startAngle: .degrees(-90), endAngle: .degrees(0), clockwise: false)
        p.addLine(to: pt(20, 20))
        return p
    }
}

/// The closed line under the related mail (the drawing's `.sx-more`): a chevron, "All other mail", its count, and, while it is selected, the Return key. One
/// click or Return opens the rest under it, the same again closes it; the chevron turns a quarter with ROW_INSERT_MS, the list's own motion.
struct SearchMoreLine: View {
    let title: String
    let count: Int
    let open: Bool
    let selected: Bool
    let toggle: () -> Void
    @State private var hovering = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.colorScheme) private var scheme

    /// " · 17": the count after the title, in the regular weight (a number and a dot, no word, so no catalogue key)
    private var countTail: String { [" ", "\u{B7}", " ", String(count)].joined() }

    var body: some View {
        let ink = selected ? WhooshPalette.onAccent.color(scheme) : WhooshPalette.secondaryLabel.color(scheme)
        Button(action: toggle) {
            HStack(spacing: WhooshSpacing.s8) {
                Image(systemName: "chevron.right").font(.system(size: 9, weight: .semibold)).frame(width: 11, height: 11)
                    .rotationEffect(.degrees(open ? 90 : 0))
                    .animation(WhooshMotion.animation(WhooshMotion.newRow, reduceMotion: reduceMotion), value: open)
                (Text(title).font(WhooshTypography.subhead.font(emphasized: true)) + Text(verbatim: countTail).font(WhooshTypography.subhead.font(emphasized: false)))
                    .lineLimit(1)
                Spacer(minLength: WhooshSpacing.s8)
                if selected { KeyCap("Enter", onSelection: true) }
            }
            .foregroundStyle(ink)
            .padding(.leading, WhooshSpacing.s14).padding(.trailing, WhooshSpacing.s8)
            .frame(maxWidth: .infinity, minHeight: 32, maxHeight: 32, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: WhooshRadius.row, style: .continuous)
                .fill(selected ? AnyShapeStyle(WhooshPalette.selection) : hovering ? AnyShapeStyle(WhooshPalette.hover) : AnyShapeStyle(Color.clear)))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .padding(.horizontal, WhooshMetrics.listRowInset).padding(.top, WhooshSpacing.s4)
        .onHover { h in withAnimation(.easeOut(duration: Double(WhooshConstants.SEARCH_HOVER_TINT_MS) / 1000)) { hovering = h } }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(selected ? .isSelected : [])
        .whooshAx("mac-search.more")
    }
}

/// No mail holds the words and none is close to them (the drawing's `nothing`, `.sx-none`): "No match" in bold and the sentence under it, in the middle.
struct SearchNoMatch: View {
    var body: some View {
        VStack(spacing: WhooshSpacing.s4 - 1) {
            Text(Words.text("No match")).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label)
            Text(Words.text("No mail holds these words, and none is close to them.")).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.secondaryLabel)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(.horizontal, WhooshSpacing.s12).padding(.top, 26).padding(.bottom, WhooshSpacing.s10)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("mac-search.empty")
    }
}
