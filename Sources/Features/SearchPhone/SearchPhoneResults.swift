import SwiftUI

/// The results region: what the list shows in place of the inbox rows while a search is open. Idle shows the recent searches; the first
/// answer shows quiet bars in the row's own anatomy; results show a header, "Top results" then "All results" and the Mac's footer line;
/// no result says so. The rows already shown stay while a newer answer is on its way, so typing never blanks the list.
struct SearchPhoneResults: View {
    let model: SearchPhoneModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) { content }
                .padding(.top, PhoneMetrics.gutter)
                .padding(.bottom, SearchPhoneMetrics.barRoom)
                .animation(reduceMotion ? nil : WhooshMotion.newRow, value: model.phase)
        }
        .scrollIndicators(.hidden)
        .scrollDismissesKeyboard(.interactively)
        .whooshAx("iphone-search.list")   // the phone's `measure` reads only frames .whooshAx publishes (phone round 6: 0 elements walked for this id)
        .accessibilityIdentifier("iphone-search.list")   // the same id, written where the static check of every control's id reads it
    }

    @ViewBuilder private var content: some View {
        switch model.phase {
        case .idle: SearchPhoneRecents(model: model)
        case .searching where model.results.isEmpty: SearchPhoneSkeleton()
        case .none: SearchPhoneNone(notes: model.notes)
        default: list
        }
    }

    private var list: some View {
        let rs = model.results
        let m = AppModel.shared
        let terms = SearchPhoneWords.terms(model.engineQuery(includingSender: true))
        return VStack(alignment: .leading, spacing: 0) {
            SearchPhoneHeader(model: model, line: model.countLine)
            if !model.notes.isEmpty {
                Text(model.notes.joined(separator: " · ")).phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.secondaryLabel)
                    .padding(.horizontal, PhoneMetrics.gutter).padding(.bottom, WhooshSpacing.s8)
                    .accessibilityIdentifier("iphone-search.notes")
            }
            let upper = min(rs.count, model.upperCount)           // the rows above the closed line: the matches, then the related mail
            if model.showsNoMatch { SearchPhoneNoMatch() }       // no match and nothing related: the sentence, the closed line under it
            upperRows(upper, rs, m, terms)
            if model.showsMore {
                SearchPhoneMoreLine(model: model).padding(.top, upper > 0 ? WhooshSpacing.s8 : 0)
                if model.restOpen {
                    section(nil, rs.indices.dropFirst(upper), rs, m, terms)
                }
            }
            Text(SearchResultsList.footerLine).phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.tertiaryLabel)
                .padding(.horizontal, PhoneMetrics.gutter).padding(.top, WhooshSpacing.s16)
                .accessibilityIdentifier("iphone-search.footer")
        }
    }

    /// The matches, the thin line "Related" and the related mail, in the engine's order: the first three under "Top results", the others under "All
    /// results" (the phone's own titles), the line where the first related mail stands, or at the end when no related mail follows it.
    private func upperRows(_ upper: Int, _ rs: [SearchResult], _ m: AppModel, _ terms: [String]) -> some View {
        let matched = min(model.matchedCount, upper), divider = model.dividerTitle
        return LazyVStack(alignment: .leading, spacing: 0) {
            ForEach(0..<upper, id: \.self) { i in
                if i == 0 || i == SearchResultsList.topCount {
                    Text(i == 0 ? SearchResultsList.topTitle : SearchResultsList.allTitle).phoneText(PhoneType.footnote, emphasized: true).foregroundStyle(WhooshPalette.tertiaryLabel)
                        .padding(.horizontal, PhoneMetrics.gutter).padding(.bottom, WhooshSpacing.s6).padding(.top, i == 0 ? 0 : WhooshSpacing.s8)
                        .accessibilityAddTraits(.isHeader)
                }
                if i == matched, let divider { SearchPhoneDivider(title: divider) }
                resultRow(i, rs, m, terms)
            }
            if matched >= upper, let divider { SearchPhoneDivider(title: divider) }
        }
    }

    private func resultRow(_ i: Int, _ rs: [SearchResult], _ m: AppModel, _ terms: [String]) -> some View {
        Button { model.openResult(i) } label: {
            SearchPhoneRow(result: rs[i], account: PhoneAccounts.of(rs[i].row.accountId, in: m), terms: terms)
        }
        .buttonStyle(SearchRowPressStyle())
        .transition(.opacity)
        .accessibilityIdentifier("iphone-search.results.\(i)")
    }

    private func section(_ title: String?, _ idx: some Sequence<Int>, _ rs: [SearchResult], _ m: AppModel, _ terms: [String]) -> some View {
        LazyVStack(alignment: .leading, spacing: 0) {
            if let title {
                Text(title).phoneText(PhoneType.footnote, emphasized: true).foregroundStyle(WhooshPalette.tertiaryLabel)
                    .padding(.horizontal, PhoneMetrics.gutter).padding(.bottom, WhooshSpacing.s6)
                    .accessibilityAddTraits(.isHeader)
            }
            ForEach(Array(idx), id: \.self) { i in resultRow(i, rs, m, terms) }
        }
    }
}

/// "Results · 3 conversations" and the quiet Save button: the Mac's header, at the phone's size. The button is a 44 pt target.
struct SearchPhoneHeader: View {
    let model: SearchPhoneModel
    let line: String

    var body: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Text(Words.text("Results")).phoneText(PhoneType.headline).foregroundStyle(WhooshPalette.label)
            Text(line).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.tertiaryLabel).lineLimit(1)
            Spacer(minLength: WhooshSpacing.s8)
            Button { model.startSaving() } label: {
                Label(Words.text("Save"), systemImage: "folder.badge.plus").phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.selection)
                    .padding(.horizontal, WhooshSpacing.s8).frame(minHeight: PhoneMetrics.roundButton).contentShape(Rectangle())
            }
            .buttonStyle(PressStyle()).accessibilityLabel(Words.text("Save as smart folder")).accessibilityIdentifier("iphone-search.save")
        }
        .padding(.leading, PhoneMetrics.gutter).padding(.trailing, WhooshSpacing.s8)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("iphone-search.header")
    }
}

/// Nothing found: the Mac's "No match" and its sentence, then the lines that explain a reading (a filter not understood).
struct SearchPhoneNone: View {
    let notes: [String]
    var body: some View {
        VStack(spacing: WhooshSpacing.s12) {
            SearchPhoneNoMatch()
            if !notes.isEmpty {
                Text(notes.joined(separator: " · ")).phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.secondaryLabel)
                    .multilineTextAlignment(.center).padding(.horizontal, PhoneMetrics.gutter)
            }
        }
        .frame(maxWidth: .infinity)
    }
}

/// Recent searches, newest first: the idle screen. A tap fills the field and runs it.
struct SearchPhoneRecents: View {
    let model: SearchPhoneModel
    var body: some View {
        if !model.recents.isEmpty {
            VStack(alignment: .leading, spacing: 0) {
                Text(Words.text("Recent")).phoneText(PhoneType.footnote, emphasized: true).foregroundStyle(WhooshPalette.tertiaryLabel)
                    .padding(.horizontal, PhoneMetrics.gutter).padding(.bottom, WhooshSpacing.s6)
                    .accessibilityAddTraits(.isHeader)
                ForEach(Array(model.recents.enumerated()), id: \.offset) { i, q in
                    Button { model.useRecent(i) } label: {
                        HStack(spacing: WhooshSpacing.s12) {
                            Image(systemName: "clock.arrow.circlepath").foregroundStyle(WhooshPalette.tertiaryLabel)
                            Text(q).phoneText(PhoneType.body).foregroundStyle(WhooshPalette.label).lineLimit(1)
                            Spacer(minLength: 0)
                        }
                        .padding(.horizontal, PhoneMetrics.gutter).frame(minHeight: PhoneMetrics.accountRow).contentShape(Rectangle())
                    }
                    .buttonStyle(SearchRowPressStyle()).accessibilityIdentifier("iphone-search.recent.\(i)")
                }
            }
        }
    }
}

/// The first answer is on its way: soft bars in the row's anatomy (tile, sender, time, subject), still, no loop.
struct SearchPhoneSkeleton: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: WhooshSpacing.s8) {
                Text(Words.text("Searching")).phoneText(PhoneType.headline).foregroundStyle(WhooshPalette.label)
                Text(Words.text("· All %lld accounts", PhoneAccounts.all(AppModel.shared).count))
                    .phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.tertiaryLabel)
            }
            .padding(.horizontal, PhoneMetrics.gutter).padding(.bottom, WhooshSpacing.s8)
            ForEach(0..<SearchPhoneMetrics.skeletonBars.count, id: \.self) { i in
                let w = SearchPhoneMetrics.skeletonBars[i]
                ZStack(alignment: .topLeading) {
                    VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
                        HStack { bar(w.sender); Spacer(); bar(w.time) }
                        bar(w.subject)
                        bar(w.subject * 0.7)
                    }
                    .padding(.leading, PhoneMetrics.rowLeading).padding(.trailing, PhoneMetrics.gutter)
                    .padding(.top, PhoneMetrics.rowTop + WhooshSpacing.s4).padding(.bottom, PhoneMetrics.rowBottom)
                    RoundedRectangle(cornerRadius: PhoneMetrics.tileRadius, style: .continuous).fill(WhooshPalette.fill)
                        .frame(width: PhoneMetrics.tile, height: PhoneMetrics.tile).offset(x: PhoneMetrics.tileX, y: PhoneMetrics.tileY)
                }
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Words.text("Searching"))
        // the measured id (FrameProbe): on the phone the socket's `measure` reads only the frames these views publish, never the accessibility tree
        // (main phone round 5: "0 accessibility elements walked" for a plain accessibility id)
        .whooshAx("iphone-search.skeleton")
        .accessibilityIdentifier("iphone-search.skeleton")   // the same id, written where the static check of every control's id reads it
    }

    private func bar(_ w: CGFloat) -> some View {
        RoundedRectangle(cornerRadius: SearchPhoneMetrics.skeletonBarHeight / 2, style: .continuous).fill(WhooshPalette.fill)
            .frame(width: w, height: SearchPhoneMetrics.skeletonBarHeight)
    }
}
