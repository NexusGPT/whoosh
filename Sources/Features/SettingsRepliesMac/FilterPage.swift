import SwiftUI

/// Brouillon automatique › Quels messages: the label inspector's three cards, reused (owner: "we already have this design in labels so we should
/// reuse a lot of it, it is literally the same"). Entry filter (which labels, then the facts grid and the sender), the AI decision (what it
/// reads, its description), exit filter (the grid), and the week's figures under them.
struct FilterPage: View {
    @State private var store = RepliesStore.shared
    private let P = RepliesAx.id("filter")

    var body: some View {
        RepliesScaffold(page: .filter) {
            if let f = store.filter {
                VStack(alignment: .leading, spacing: WhooshSpacing.s12) {
                    ReplyCardShell(ax: "\(P).entry", title: RepliesWords.entryFilter, subtitle: RepliesWords.entryLine, count: keeps(\.afterEntry, \.received),
                                   on: f.entryEnabled, toggle: { store.setCard(.entry, $0) }) {
                        LabelScopeRow(filter: f)
                        ReplyRuleCard(card: .entry, rule: f.entryRule, lead: RepliesWords.andWhen)
                        Hair()
                        SenderRow(sender: RepliesFacts.sender(f.entryRule.conditions))
                    }
                    ReplyCardShell(ax: "\(P).ai", title: RepliesWords.aiDecision, subtitle: RepliesWords.aiLine, count: aiCount,
                                   on: f.aiEnabled, tinted: true, toggle: { store.setCard(.ai, $0) }) {
                        ReadsRow(reads: f.aiReads)
                        DescriptionBox()
                    }
                    ReplyCardShell(ax: "\(P).exit", title: RepliesWords.exitFilter, subtitle: RepliesWords.exitLine, count: keeps(\.afterExit, \.afterAi),
                                   on: f.exitEnabled, toggle: { store.setCard(.exit, $0) }) {
                        ReplyRuleCard(card: .exit, rule: f.exitRule, lead: Words.text("Keep when"))
                    }
                    meta
                }
                .padding(.top, WhooshSpacing.s8)
            } else {
                SettingsFootnote(text: RepliesWords.notReady, ax: "\(P).empty")
            }
        }
    }

    private func keeps(_ kept: KeyPath<ReplyStepCounts, UInt32>, _ of: KeyPath<ReplyStepCounts, UInt32>) -> String {
        store.counts.map { RepliesWords.keeps($0[keyPath: kept], of: $0[keyPath: of]) } ?? ""
    }
    private var aiCount: String { store.counts.map { RepliesWords.yesFor($0.afterAi, of: $0.afterEntry) } ?? "" }

    /// « 128 reçus · 31 après le filtre d'entrée · 12 par l'IA · 9 brouillons », and the window of days at the right.
    @ViewBuilder private var meta: some View {
        if let c = store.counts {
            let funnel = Text(RepliesWords.funnel(c)).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
            let days = Text(RepliesWords.lastDaysShort(c.days)).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
            // hid-ai round 21 (main111 b1d38560, 700x480: « 9 drafts » alone on a second line): the figures stay on ONE line; when the line and the
            // window of days do not fit side by side, the window of days goes to a row of its own under them; only a column too narrow for the figures
            // themselves lets them wrap. Measured on layout only (ViewThatFits), nothing per frame.
            ViewThatFits(in: .horizontal) {
                HStack(spacing: WhooshSpacing.s8) {
                    funnel.lineLimit(1).fixedSize()
                    Spacer(minLength: WhooshSpacing.s8)
                    days.lineLimit(1).fixedSize()
                }
                VStack(alignment: .leading, spacing: WhooshSpacing.s4) {
                    funnel.lineLimit(1).fixedSize()
                    days.lineLimit(1)
                }
                VStack(alignment: .leading, spacing: WhooshSpacing.s4) { funnel; days }
            }
            .contentTransition(.numericText())
            .padding(.horizontal, WhooshSpacing.s2)
            .whooshAx("\(P).meta")
        }
    }
}

/// `.lf-row`: a grey key at the left (at least 118 pt, the board's), then what the row holds.
struct ReplyCardRow<Content: View>: View {
    let key: String
    var ax: String? = nil
    /// round 24 (main114: the « Labels » key floated halfway down a tall column of chips): a row whose content wraps puts its key at the first line.
    var keyAtTop = false
    @ViewBuilder var content: Content
    var body: some View {
        HStack(alignment: keyAtTop ? .top : .center, spacing: WhooshSpacing.s8) {
            Text(key).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                .frame(minWidth: 118, minHeight: keyAtTop ? 24 : nil, alignment: .leading)   // 24: a chip's height, so the key centres on the first row
            content
            Spacer(minLength: 0)
        }
        .padding(.horizontal, WhooshSpacing.s12).padding(.vertical, WhooshSpacing.s4)
        .frame(maxWidth: .infinity, minHeight: 34, alignment: .leading)
        .modifier(SettingsOptionalAx(ax: ax))
    }
}

/// « Étiquettes »: Toutes, or the labels ticked (owner round 7: "be able to select existing labels potentially or all labels").
struct LabelScopeRow: View {
    let filter: ReplyFilter
    @State private var store = RepliesStore.shared
    private let P = RepliesAx.id("filter", "scope")

    var body: some View {
        let ticked: [String] = { if case .labels(let ids) = filter.scope { return ids } else { return [] } }()
        let all = { if case .all = filter.scope { return true } else { return false } }()
        ReplyCardRow(key: Words.text("Labels"), ax: "\(P).row", keyAtTop: true) {
            FlowLayout(spacing: WhooshSpacing.s6) {
                ReplyTick(title: RepliesWords.allLabels, on: all, ax: "\(P).all") { store.setScope(.all) }
                ForEach(store.labels, id: \.id) { l in
                    ReplyTick(title: l.kind == .builtIn ? Words.text(l.name) : l.path.replacingOccurrences(of: " > ", with: " › "),
                              on: ticked.contains(l.id), ax: "\(P).label.\(l.id)") { store.toggleLabel(l.id) }
                }
            }
        }
        if !all && ticked.isEmpty {
            InlineError(words: RepliesWords.noLabelTicked).padding(.horizontal, WhooshSpacing.s12).padding(.bottom, WhooshSpacing.s6)
                .whooshAx("\(P).none")
        }
        Hair()
    }
}
