import SwiftUI

/// The line under the tab row while the inspector is open (pane 2): "4 matches · rules only" and the Matches | Mailbox switch.
/// A saved label whose rules moved shows "Re-run preview" and the join, leave and stay counts instead (AI sorting on: how many the rules pick, the AI rules on each after Save). The tab row and the
/// search field above it do not move.
struct LabelProofLine: View {
    @State private var model = LabelInspectorModel.shared  // local-ui: the inspector's state, observed

    var body: some View {
        let p = model.proof
        HStack(spacing: WhooshSpacing.s6) {
            if p.hasPreview {
                Text(Words.text("Re-run preview")).font(WhooshTypography.caption.font(emphasized: true)).foregroundStyle(WhooshPalette.label)
                Spacer(minLength: WhooshSpacing.s8)
                Text(model.previewLine ?? "")
                    .font(WhooshTypography.caption.font(emphasized: false)).foregroundStyle(WhooshPalette.secondaryLabel).monospacedDigit()
            } else {
                lead(p)
                Spacer(minLength: WhooshSpacing.s6)
                switcher
            }
        }
        .lineLimit(1)
        .padding(.horizontal, WhooshMetrics.listRowInset + WhooshSpacing.s12)
        .frame(height: WhooshMetrics.listHeaderHeight)
        .animation(.snappy, value: p.count)
        .accessibilityElement(children: .contain)
        .whooshAx("mac-labels.inspector.line")
    }

    @ViewBuilder private func lead(_ p: LabelProof) -> some View {
        if model.refusal != nil { Image(systemName: "exclamationmark.circle").font(.system(size: 11)).foregroundStyle(WhooshPalette.warning) }
        else { Circle().fill(p.count == 0 ? WhooshPalette.tertiaryLabel : WhooshPalette.accent).frame(width: 6, height: 6).opacity(0.9) }
        Text(p.count == 1 ? Words.text("1 match") : Words.text("%lld matches", p.count))
            .font(WhooshTypography.caption.font(emphasized: true)).foregroundStyle(WhooshPalette.label).monospacedDigit()
            .contentTransition(.numericText(value: Double(p.count)))
        Text(tail).font(WhooshTypography.caption.font(emphasized: false)).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
    }

    private var tail: String {
        if model.counting { return Words.text("· counting %lld accounts…", AccountBook.shared.accounts(core: model.core).count) }
        if model.refusal != nil { return Words.text("· last valid preview") }
        return Words.text("· rules only")
    }

    private var switcher: some View {
        HStack(spacing: 0) {
            seg(Words.text("Matches"), on: model.showMatches, id: "matches") { model.showMatches = true }
            seg(Words.text("Mailbox"), on: !model.showMatches, id: "mailbox") { model.showMatches = false }
        }
        .padding(2).background(WhooshPalette.fill, in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
        .animation(WhooshMotion.hover, value: model.showMatches)
    }

    private func seg(_ title: String, on: Bool, id: String, _ action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title).font(.system(size: 11, weight: on ? .semibold : .regular)).foregroundStyle(on ? WhooshPalette.label : WhooshPalette.secondaryLabel)
                .padding(.horizontal, WhooshSpacing.s8).frame(height: 18)
                .background { if on { RoundedRectangle(cornerRadius: WhooshRadius.keyCap, style: .continuous).fill(WhooshPalette.thumb).shadow(radius: 0.5, y: 0.5) } }
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain).help(title)
        .accessibilityIdentifier("mac-labels.inspector.switch.\(id)")
    }
}

/// The rows of pane 2 while the inspector is open: the messages the draft matches, each with its rule tag, or, for a saved
/// label whose rules moved, the Join, Leave and Stay groups with a 2 px bar at each row's left edge. Same 44 pt row as the list.
struct LabelProofRows: View {
    @State private var model = LabelInspectorModel.shared  // local-ui: the inspector's state, observed
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let p = model.proof
        let book = RowAccount.directory(model.app)
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                if model.counting && model.holdCounting { ProgressView(value: 0.4).progressViewStyle(.linear).tint(WhooshPalette.accent).padding(.horizontal, WhooshSpacing.s20).padding(.bottom, WhooshSpacing.s4) }
                if p.hasPreview {
                    group(Words.text("Join %@", model.draft.name), p.join, book)
                    group(Words.text("Leave %@", model.draft.name), p.leave, book)
                    group(Words.text("Stay"), p.stay, book)
                    note(Words.text("%lld other messages are not affected. Nothing moves until you press Re-run.", p.unaffected))
                    if let w = p.engine?.aiWords { note(w).accessibilityIdentifier("mac-labels.inspector.aiwords") }   // the engine's own sentence: no model can answer, so the AI adds none
                } else if p.rows.isEmpty && !model.counting {
                    ProofEmpty(model: model)
                } else {
                    rows(p.rows, book)
                    if model.counting { Skeleton() }
                    note(model.counting ? Words.text("The last count stays until the new one lands. Typing never waits for it.")
                         : (model.isNew ? Words.text("Rules only. After Create, the decision model also files mail that fits the meaning.") : Words.text("Rules only. A re-run also files by meaning.")))
                }
            }
            .padding(.bottom, WhooshSpacing.s16)
            .animation(WhooshMotion.animation(WhooshMotion.newRow, reduceMotion: reduceMotion), value: p.rows.map(\.id))
        }
        .scrollIndicators(.hidden)
        .whooshAx("mac-labels.inspector.rows")
    }

    private func rows(_ list: [ProofRow], _ book: [String: RowAccount]) -> some View {
        ForEach(list) { r in
            ProofRowView(row: r, account: RowAccount.of(r.accountId, in: book), stale: model.counting, aiOn: model.draft.flow.aiOn)
                .transition(reduceMotion ? .opacity : .opacity.combined(with: .move(edge: .top)))
        }
    }

    @ViewBuilder private func group(_ title: String, _ list: [ProofRow], _ book: [String: RowAccount]) -> some View {
        if !list.isEmpty {
            HStack(spacing: WhooshSpacing.s6) {
                Text(title).font(WhooshTypography.caption.font(emphasized: true)).foregroundStyle(WhooshPalette.secondaryLabel)
                Text("\(list.count)").font(WhooshTypography.caption.font(emphasized: false)).foregroundStyle(WhooshPalette.tertiaryLabel)
                Spacer()
            }
            .padding(.horizontal, WhooshMetrics.listRowInset + WhooshSpacing.s12).frame(height: 26, alignment: .bottom).padding(.bottom, 4)
            rows(list, book)
        }
    }

    private func note(_ words: String) -> some View {
        Text(words).font(WhooshTypography.caption.font(emphasized: false)).foregroundStyle(WhooshPalette.secondaryLabel)
            .padding(.horizontal, WhooshSpacing.s24 + WhooshSpacing.s4).padding(.top, WhooshSpacing.s14).fixedSize(horizontal: false, vertical: true)
    }
}
