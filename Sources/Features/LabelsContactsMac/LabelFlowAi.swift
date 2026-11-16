import SwiftUI

/// Card 2, « Classement par l'IA »: the label's description is what the AI sorts by, and the owner chooses what it reads. Never named after a
/// model: it is "the AI". The card has no rules of its own.
struct AiCard: View {
    @Bindable var model: LabelInspectorModel
    var focus: FocusState<LabelFocus?>.Binding

    var body: some View {
        let f = model.draft.flow
        VStack(alignment: .leading, spacing: WhooshSpacing.s6) {
            FlowCardShell(key: "ai", title: Words.text("AI sorting"), subtitle: Words.text("from the description"),
                          count: model.aiCountText, after: model.aiAfterText, on: f.aiOn, tinted: true, note: model.aiRefusalShown, door: model.refusalDoor.map { d in (d.title, d.id, { _ = model.runRefusalDoor(d.id) }) }, toggle: { model.setCard(.ai, $0) }) {
                readsRow
                MeaningField(model: model, focus: focus)
                if let w = model.refusal(for: [FlowRefusal.aiReadsNothing]) {
                    InlineError(words: w).padding(.horizontal, WhooshSpacing.s8).padding(.bottom, WhooshSpacing.s8).whooshAx(LabelGeo.free("ai.refusal"))
                }
            }
            .animation(WhooshMotion.hover, value: model.refusal?.words)
            FlowMeta(words: Words.text("One label per message: the AI reads this description with those of your other labels."))
        }
    }

    private var readsRow: some View {
        let r = model.draft.flow.reads
        return HStack(spacing: WhooshSpacing.s8) {
            Text(Words.text("The AI reads")).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
            FlowLayout(spacing: WhooshSpacing.s6) {
                ReadTick(title: Words.text("Subject"), on: r.subject, id: "subject") { model.toggleRead(.subject) }
                ReadTick(title: Words.text("Body"), on: r.body, id: "body") { model.toggleRead(.body) }
                ReadTick(title: Words.text("Rest of the thread"), on: r.restOfThread, id: "rest_of_thread") { model.toggleRead(.restOfThread) }
                ReadTick(title: Words.text("Attachments"), on: r.attachments, id: "attachments") { model.toggleRead(.attachments) }
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, WhooshSpacing.s12).padding(.vertical, WhooshSpacing.s6)
        .frame(maxWidth: .infinity, minHeight: WhooshMetrics.listRow, alignment: .leading)
        .whooshAx(LabelGeo.row("ai", "reads"))
    }
}

/// One tick of « L'IA lit »: a checkbox and its word on a quiet field. The fill changes on the click's own frame.
struct ReadTick: View {
    let title: String
    let on: Bool
    let id: String
    let action: () -> Void
    @State private var hover = false  // local-ui: hover tint only

    var body: some View {
        Button(action: action) {
            HStack(spacing: WhooshSpacing.s6) {
                RoundedRectangle(cornerRadius: WhooshRadius.keyCap - 1, style: .continuous).fill(on ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(.clear))
                    .overlay { RoundedRectangle(cornerRadius: WhooshRadius.keyCap - 1, style: .continuous).strokeBorder(WhooshPalette.tertiaryLabel, lineWidth: WhooshSpacing.s2 * 0.75).opacity(on ? 0 : 1) }
                    .overlay { Image(systemName: "checkmark").font(.system(size: WhooshTypography.caption.size - 3, weight: .bold)).foregroundStyle(WhooshPalette.onAccent).opacity(on ? 1 : 0) }
                    .frame(width: WhooshSpacing.s14, height: WhooshSpacing.s14)
                Text(title).whooshText(WhooshTypography.subhead).foregroundStyle(on ? WhooshPalette.label : WhooshPalette.secondaryLabel).lineLimit(1)
            }
            .padding(.horizontal, WhooshSpacing.s8).frame(height: WhooshSpacing.s24)
            .background(RoundedRectangle(cornerRadius: WhooshRadius.item, style: .continuous).fill(hover ? WhooshPalette.hover : WhooshPalette.field))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain).onHover { hover = $0 }
        .animation(WhooshMotion.hover, value: on).animation(WhooshMotion.hover, value: hover)
        .accessibilityAddTraits(on ? .isSelected : []).accessibilityLabel(title)
        .accessibilityIdentifier("\(LabelGeo.base).flow.ai.reads.\(id)").whooshAx(LabelGeo.ctl("ai", "reads.\(id)"))
    }
}

/// The label's description, in the owner's words: the one thing the AI sorts by.
struct MeaningField: View {
    @Bindable var model: LabelInspectorModel
    var focus: FocusState<LabelFocus?>.Binding

    var body: some View {
        ZStack(alignment: .topLeading) {
            TextEditor(text: Binding(get: { model.draft.meaning }, set: { v in model.edit { $0.meaning = v } }))
                .scrollContentBackground(.hidden).whooshText(WhooshTypography.subhead)
                .focused(focus, equals: .meaning)
                .accessibilityIdentifier("mac-labels.inspector.meaning")
            if model.draft.meaning.isEmpty {
                Text(Words.text("In your words: what belongs here. The AI reads it."))
                    .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.tertiaryLabel)
                    .padding(.leading, WhooshSpacing.s4).allowsHitTesting(false)
            }
        }
        .frame(maxWidth: .infinity, minHeight: WhooshMetrics.listRow * 1.5, alignment: .topLeading)
        .whooshAx(LabelGeo.ctl("ai", "editor"))
        .padding(.horizontal, WhooshSpacing.s8).padding(.vertical, WhooshSpacing.s6)
        .background(RoundedRectangle(cornerRadius: WhooshRadius.row, style: .continuous).fill(WhooshPalette.field))
        .padding(.horizontal, WhooshSpacing.s12).padding(.bottom, WhooshSpacing.s8)
    }
}
