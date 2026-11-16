import SwiftUI

/// The inspector's foot: where the label's messages came from (« 24 messages · 1 par règle · 31 par l'IA · 8 vers Principal »), « Pourquoi ici ? »,
/// Close (or Cancel), Save (or Create). The Re-run question replaces all of it in place with a cross-fade (READER_CROSSFADE_MS);
/// every button names its key. On a narrow pane the words and the buttons take two lines instead of cutting a word.
struct LabelInspectorFoot: View {
    @Bindable var model: LabelInspectorModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private enum Kind: String { case offer, create, apply, clean }
    private var kind: Kind { model.offer != nil ? .offer : (model.isNew ? .create : (model.dirty ? .apply : .clean)) }

    var body: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Group {
                if kind == .offer { offer } else { standing }
            }
            .transition(.opacity)
        }
        .id(kind)
        .padding(.horizontal, WhooshMetrics.readerSide).padding(.vertical, WhooshSpacing.s12)
        .frame(maxWidth: .infinity)
        .background(WhooshPalette.background)
        .overlay(alignment: .top) { Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator) }
        .animation(WhooshMotion.animation(WhooshMotion.readerSwap, reduceMotion: reduceMotion), value: kind)
        .accessibilityIdentifier("mac-labels.inspector.foot")
        .onChange(of: model.whyOpen, initial: true) { _, _ in readFiling() }
        .onChange(of: model.app.focusedMessageId) { _, _ in readFiling() }
    }

    private var standing: some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: WhooshSpacing.s8) { words; Spacer(minLength: WhooshSpacing.s8); buttons }
            VStack(alignment: .leading, spacing: WhooshSpacing.s8) { words; HStack(spacing: WhooshSpacing.s8) { Spacer(minLength: 0); buttons } }
        }
    }

    /// What the core says about the mail in focus when no card placed it (`message/<id>/filing`): read when « Pourquoi ici ? » opens or the focus
    /// moves, never in a body (the read opens the store).
    @State private var unsorted: String?
    private func readFiling() {
        guard model.whyOpen, let id = model.app.focusedMessageId else { unsorted = nil; return }
        unsorted = LabelInspectorFoot.unsorted(model.core.state(path: "message/\(id)/filing"))
    }

    /// When the AI could not sort the mail, plain words with the engine's own (`ai_refusal`); never "sorted by AI". Nil when the AI answered,
    /// was not reached, or the read says nothing (a refusal with no words is not worded here: the engine always gives them).
    static func unsorted(_ filing: String?) -> String? {
        guard let f = StoredRules.object(filing) as? [String: Any] else { return nil }
        switch f["ai"] as? String {
        case "refused":
            guard let why = (f["ai_refusal"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines), !why.isEmpty else { return nil }
            return Words.text("The AI could not sort this mail: %@", why)
        case "no_provider": return Words.text("No AI is set up to sort this mail.")
        default: return nil
        }
    }

    private var trouble: String? { model.failure ?? model.refusal?.words }

    @ViewBuilder private var words: some View {
        if trouble != nil {
            HStack(spacing: WhooshSpacing.s6) {
                Circle().fill(WhooshPalette.warning).frame(width: WhooshMetrics.warningDot, height: WhooshMetrics.warningDot)
                line(model.failure ?? (model.isNew ? Words.text("Fix the rule to create") : Words.text("Fix the rule to apply")))
            }
        } else if model.whyOpen, let u = unsorted {   // « Pourquoi ici ? » is open and the AI could not sort this mail: the engine's words, not "sorted by AI"
            HStack(spacing: WhooshSpacing.s6) {
                Circle().fill(WhooshPalette.warning).frame(width: WhooshMetrics.warningDot, height: WhooshMetrics.warningDot)
                Text(u).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(2)
                    .accessibilityIdentifier("mac-labels.inspector.unsorted")
            }
        } else if kind == .clean, let r = model.result {
            // the engine's own result: how many mails the judge answered about, how many it accepted into the label, then what moved (never a "sure of none" sentence here)
            let moved = Words.text("Re-filed %lld messages · %lld left it", r.refiled, r.unfiled)
            line(r.total > 0 ? [Words.text("Sorted %@ of %@", "\(r.judged)", "\(r.total)"), Words.text("%lld accepted", r.filed), moved].joined(separator: " · ") : moved)
        } else if kind == .apply, let full = model.nowAfterLine() {   // changes not saved: what the label holds now, then what Save and a re-run give
            ViewThatFits(in: .horizontal) {
                nowAfter(full)
                if let short = model.nowAfterLine(short: true), short != full { nowAfter(short) }
            }
        } else if let p = provenance {
            Text(p).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                .accessibilityIdentifier("mac-labels.inspector.provenance")
        } else {
            line(model.draft.name.isEmpty ? Words.text("Name it to create") : Words.text("Not created yet"))
        }
    }

    /// "Now 0 → the AI rules on 48" (AI sorting on) or "Now 0 → 48 by rule · 0 by the AI": what the label holds now and the arrow in the secondary colour, the figures after Save in the accent, semibold. One line, never wrapped.
    private func nowAfter(_ s: String) -> some View {
        let arrow = s.firstIndex(of: "\u{2192}").map { s.index(after: $0) } ?? s.startIndex
        let (now, after) = (String(s[..<arrow]), String(s[arrow...]))
        return HStack(spacing: 0) {
            Text(now).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
            Text(after).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.accent)
        }
        .lineLimit(1).fixedSize(horizontal: true, vertical: false)
        .accessibilityElement(children: .combine).accessibilityLabel(s)
        .accessibilityIdentifier("mac-labels.inspector.provenance")
    }

    private func line(_ s: String) -> some View {
        Text(s).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
    }

    /// The model's four figures, the numbers in bold.
    private var provenance: AttributedString? {
        guard let parts = model.provenanceParts else { return nil }
        var out = AttributedString()
        for (i, p) in parts.enumerated() {
            if i > 0 { out.append(AttributedString(" · ")) }
            var a = AttributedString(p)
            if let r = p.range(of: "\\d+", options: .regularExpression), let ar = Range(r, in: a) { a[ar].inlinePresentationIntent = .stronglyEmphasized }
            out.append(a)
        }
        return out
    }

    private var buttons: some View {
        HStack(spacing: WhooshSpacing.s8) {
            KeyedButton(title: Words.text("Why here?"), key: "?") { _ = model.toggleWhy() }.accessibilityIdentifier("mac-labels.inspector.why")
                .whooshAx(LabelGeo.free("why"))
            if kind == .clean {
                KeyedButton(title: Words.text("Close"), key: "esc") { _ = model.close() }.accessibilityIdentifier("mac-labels.inspector.done").whooshAx(LabelGeo.free("close.done"))
            } else {
                KeyedButton(title: Words.text("Cancel"), key: "esc") { _ = model.close() }.accessibilityIdentifier("mac-labels.inspector.cancel").whooshAx(LabelGeo.free("close.cancel"))
            }
            if kind == .create {
                KeyedButton(title: Words.text("Create"), key: "↩", primary: true, enabled: model.canWrite) { _ = model.create() }
                    .accessibilityIdentifier("mac-labels.inspector.create")
            } else {
                KeyedButton(title: Words.text("Save"), key: "↩", primary: true, enabled: model.canWrite) { _ = model.apply() }
                    .accessibilityIdentifier("mac-labels.inspector.apply")
            }
        }
    }

    @ViewBuilder private var offer: some View {
        if let o = model.offer {
            VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                Text(Words.text("Re-run on %lld messages?", o.filed)).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label).lineLimit(1)
                Text(o.beyond > 0 ? Words.text("%lld older messages are not sorted yet. Re-run sorts the next ones.", o.beyond)
                                  : Words.text("%lld join %@, %lld leaves, %lld stay. Not now keeps today's filing.", o.join, model.draft.name, o.leave, o.stay))
                    .whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
            }
            .accessibilityIdentifier("mac-labels.inspector.offer")
            Spacer(minLength: WhooshSpacing.s8)
            KeyedButton(title: Words.text("Not now"), key: "esc") { _ = model.notNow() }.accessibilityIdentifier("mac-labels.inspector.notnow")
            KeyedButton(title: Words.text("Re-run"), key: "R", primary: true) { _ = model.rerun() }.accessibilityIdentifier("mac-labels.inspector.rerun")
        }
    }
}
