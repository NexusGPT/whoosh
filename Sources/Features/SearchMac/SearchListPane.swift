import SwiftUI

/// What the main window's list pane shows once Enter launched a search: the header (what is shown, Save as smart folder), then skeleton rows on the
/// frame Enter lands, each row cross-fading into its result when the answer is in. The inbox list mounts it in place of its rows
/// (InboxListView `content`); the tab strip, the field and the list header above stay the list's own.
struct SearchResultsPane: View {
    let model: SearchMacModel
    @Bindable var app: AppModel
    let compact: Bool

    static let headerHeight: CGFloat = 30
    static let rowsTop: CGFloat = headerHeight + 4

    var body: some View {
        ZStack(alignment: .top) {
            header.padding(.leading, WhooshSpacing.s16 + WhooshSpacing.s2).padding(.trailing, WhooshSpacing.s12)
                .frame(maxWidth: .infinity, alignment: .topLeading)
            GeometryReader { g in
                ZStack(alignment: .top) {
                    if model.loading {
                        SearchSkeletonList(height: g.size.height - Self.rowsTop).transition(.opacity)
                    } else if let words = model.refusal {
                        SearchRefusedState(words: words).transition(.opacity)
                    } else if model.allResults.isEmpty, !model.tailPending {
                        SearchEmptyState().transition(.opacity)
                    } else {
                        SearchResultsList(model: model).transition(.opacity)
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
                .padding(.top, Self.rowsTop)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(WhooshPalette.background)
        .overlay(alignment: .bottom) {   // ai-setup W1: the meaning model is missing, so the meaning hits never come: say it and fix it here
            // Only while the mark says something: with the model set up the mark is empty, and its padding and ground still stood as a 10 pt blank
            // band over the foot of the results (measured on walk-main183/08: the results ended 18.5 pt above the foot strip, the inbox list 3.5).
            if MissingSetupMark.says(.meaning) {
                MissingSetupMark(need: .meaning, sentence: .consequence).padding(.horizontal, WhooshSpacing.s16).padding(.bottom, WhooshSpacing.s10)
                    .frame(maxWidth: .infinity).background(WhooshPalette.background)
            }
        }
        .task { AISetupStatus.shared.refresh() }   // the mark refreshed the status when it was always mounted
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("mac-search.list")
    }

    /// "3 conversations": the phone's header count (the Mac's header counts the matching conversations, `SearchMacModel.countLine`)
    static func countLine(_ n: Int) -> String { n == 1 ? Words.text("1 conversation") : Words.text("%lld conversations", n) }

    @ViewBuilder private var header: some View {
        if model.naming {
            SaveNameField(model: model, app: app)
        } else if model.loading {
            HStack(spacing: WhooshSpacing.s8) {
                Text(Words.text("Searching")).whooshText(WhooshTypography.body, emphasized: true)
                Text(Words.text("· All %lld accounts", AccountBook.shared.accounts().count)).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.tertiaryLabel)
                Spacer(minLength: 0)
            }
            .frame(height: Self.headerHeight)
        } else {
            HStack(spacing: WhooshSpacing.s8) {
                if model.refusal == nil {
                    // "N matches · M related" (SPEC A150): "N matches" alone when nothing is related, "No match · M related" when nothing matches, no line when both are 0;
                    // the rest is told apart by the closed line "All other mail · N", never counted here
                    let line = SearchMacModel.countLine(matches: model.matchedConversations, related: model.relatedCount)
                    if !line.isEmpty { Text(line).whooshText(WhooshTypography.body, emphasized: true).lineLimit(1).layoutPriority(1) }
                }
                Spacer(minLength: WhooshSpacing.s8)
                if model.refusal == nil {
                    // Never a cut label (lead, 13 Nov, picture s1-135 at 960 points: "Save as smart…"): the whole words where they fit beside
                    // the count line, else the short word; the count line keeps its width first.
                    if compact {
                        SaveButton(compact: true) { _ = model.startSaving() }
                    } else {
                        ViewThatFits(in: .horizontal) {
                            SaveButton(compact: false) { _ = model.startSaving() }
                            SaveButton(compact: true) { _ = model.startSaving() }
                        }
                    }
                }
            }
            .frame(height: Self.headerHeight)
        }
    }
}

/// The quiet button at the right of the results header: "Save as smart folder", "Save" in a narrow list.
struct SaveButton: View {
    let compact: Bool
    let action: () -> Void
    @State private var hovering = false
    var body: some View {
        Button(action: action) {
            Label(compact ? Words.text("Save") : Words.text("Save as smart folder"), systemImage: "folder.badge.plus")
                .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                .padding(.horizontal, WhooshSpacing.s8).padding(.vertical, WhooshSpacing.s4 + 1)
                .background(RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous).fill(hovering ? AnyShapeStyle(WhooshPalette.hover) : AnyShapeStyle(Color.clear)))
        }
        .buttonStyle(.plain)
        .onHover { h in withAnimation(.easeOut(duration: Double(WhooshConstants.SEARCH_HOVER_TINT_MS) / 1000)) { hovering = h } }
        .accessibilityIdentifier("mac-search.save-as-smart-folder")
    }
}

/// The engine refused the search: one bold sentence, the engine's own words under it, no list.
struct SearchRefusedState: View {
    let words: String
    var body: some View {
        SearchNote(title: SearchMacModel.refusedSentence, line: words)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
    }
}

/// Nothing is listed at all (a filter keeps no mail): the same words as the "No match" state, with no closed line under them.
struct SearchEmptyState: View {
    var body: some View {
        SearchNoMatch().frame(maxWidth: .infinity, alignment: .top)
    }
}
