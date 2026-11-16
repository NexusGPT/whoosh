import SwiftUI

/// One card of the flow: a head (its name, what it does, its count, its switch) and the card under it. A card switched off keeps everything it
/// holds and fades to 40 %: opacity only, on the hover tint's clock. The AI card wears the drawing's tinted edge; the other two are quiet.
struct FlowCardShell<Content: View>: View {
    let key: String
    let title: String
    let subtitle: String
    let count: String
    /// the figure after Save and a re-run, while changes are not saved (« catches 0 → 48 after Save »)
    var after: String?
    let on: Bool
    var tinted = false
    /// a sentence under the header line (the AI card: why it cannot sort now)
    var note: String?
    /// the control under the note: the door that fixes it (download the weights, open Decisions)
    var door: (title: String, id: String, run: () -> Void)?
    let toggle: (Bool) -> Void
    @ViewBuilder var content: Content
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var shape: RoundedRectangle { RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous) }

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s6) {
            head
            VStack(spacing: 0) { content }
                .frame(maxWidth: .infinity, alignment: .leading)
                .background { shape.fill(tinted ? AnyShapeStyle(wash) : AnyShapeStyle(WhooshPalette.fill)) }
                .overlay { shape.strokeBorder(tinted ? WhooshPalette.accent.color(scheme).opacity(0.4) : WhooshPalette.separator.color(scheme), lineWidth: WhooshMetrics.separator) }
                .clipShape(shape)
                .opacity(on ? 1 : 0.4)
                .whooshAx(LabelGeo.card(key))
        }
        .animation(WhooshMotion.animation(WhooshMotion.hover, reduceMotion: reduceMotion), value: on)
    }

    private var wash: LinearGradient {
        LinearGradient(colors: [FlowTone.indigo.color(scheme).opacity(0.12), WhooshPalette.accent.color(scheme).opacity(0.08), FlowTone.teal.color(scheme).opacity(0.12)],
                       startPoint: .topLeading, endPoint: .bottomTrailing)
    }

    private var head: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s2) { headLine; noteLine }
        .padding(.horizontal, WhooshSpacing.s2)
        .accessibilityElement(children: .contain).accessibilityIdentifier("\(LabelGeo.base).flow.\(key)")
    }

    @ViewBuilder private var noteLine: some View {
        if let note {
            Text(note).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(2).fixedSize(horizontal: false, vertical: true)
                .accessibilityIdentifier("\(LabelGeo.base).flow.\(key).note").whooshAx(LabelGeo.free("\(key).note"))
            if let door {
                Button(action: door.run) { Text(door.title).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.accent) }
                    .buttonStyle(.plain).accessibilityIdentifier("\(LabelGeo.base).flow.\(key).door")
            }
        }
    }

    private var headLine: some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: WhooshSpacing.s8) { name; Text(subtitle).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1); Spacer(minLength: WhooshSpacing.s8); figures }
            VStack(alignment: .leading, spacing: 0) {
                HStack(spacing: WhooshSpacing.s8) { name; Spacer(minLength: WhooshSpacing.s8); figures }
                Text(subtitle).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
            }
        }
        .frame(minHeight: WhooshSpacing.s24)
    }

    private var name: some View {
        HStack(spacing: WhooshSpacing.s6) {
            if tinted { Image(systemName: "sparkles").font(WhooshTypography.subhead.font).foregroundStyle(WhooshPalette.accent) }
            Text(title).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(tinted ? WhooshPalette.accent : WhooshPalette.label).lineLimit(1)
        }
    }

    private var figures: some View {
        HStack(spacing: WhooshSpacing.s10) {
            HStack(spacing: WhooshSpacing.s4) {
                Text(count).whooshText(WhooshTypography.subhead).foregroundStyle(on ? WhooshPalette.secondaryLabel : WhooshPalette.tertiaryLabel).lineLimit(1)
                    .accessibilityIdentifier("\(LabelGeo.base).flow.\(key).count")
                if let after {
                    Text("\u{2192}").whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).accessibilityHidden(true)
                    Text(after).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.accent).lineLimit(1)
                        .accessibilityIdentifier("\(LabelGeo.base).flow.\(key).after")
                }
            }
            .animation(WhooshMotion.hover, value: after)
            // hid-ai round 18 (the lead's request; hid-labels owns this file and was idle): the system switch drew its ON state grey whenever the window
            // was not the key window (main107 d58d8ad5, the same card copied in Settings › Replies); the Settings window's drawn switch keeps the accent.
            // Same socket id, tooltip and door.
            let isOn = on
            SettingsSwitch(on: isOn)
                .contentShape(Rectangle())
                .onTapGesture { toggle(!isOn) }
                .accessibilityElement().accessibilityLabel(title)
                .accessibilityAddTraits(.isButton).accessibilityValue(isOn ? Words.text("On") : Words.text("Off"))
                .help(Words.text("Turn this card on or off")).accessibilityIdentifier("\(LabelGeo.base).flow.\(key).switch")
                .whooshAx(LabelGeo.free("\(key).switch"))
        }
    }
}

/// A line of grey words under a card (what the card does for the card above it).
struct FlowMeta: View {
    let words: String
    var body: some View {
        Text(words).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
            .fixedSize(horizontal: false, vertical: true).padding(.horizontal, WhooshSpacing.s4)
    }
}

/// Card 1, « Règles d'entrée »: step 1 picks the mails the AI reads (with the AI card off, they go straight in). Chain 0, then the alternatives.
struct EntryCard: View {
    @Bindable var model: LabelInspectorModel
    var focus: FocusState<LabelFocus?>.Binding

    var body: some View {
        let f = model.draft.flow
        VStack(alignment: .leading, spacing: WhooshSpacing.s6) {
            FlowCardShell(key: "entry", title: Words.text("Entry rules"), subtitle: f.aiOn ? Words.text("picks the mails the AI reads") : Words.text("these mails go straight in"),
                          count: model.entryCountText, after: model.entryAfterText, on: f.entryOn, toggle: { model.setCard(.entry, $0) }) {
                ChainView(model: model, ref: .entry, card: "entry", lead: Words.text("File when"), focus: focus, offersOther: true)
                ForEach(Array(f.others.enumerated()), id: \.element.id) { i, c in
                    OrSeam()
                    ChainView(model: model, ref: .other(c.id), card: "other\(i + 1)", lead: Words.text("File when"), focus: focus, removable: true)
                }
            }
            .animation(WhooshMotion.hover, value: f.others.map(\.id))
            VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                FlowMeta(words: f.aiOn ? Words.text("These rules run first. The AI reads only the mails they pick.") : Words.text("These rules file the mail. AI sorting is off for this label."))
                DropHint(model: model).whooshAx(LabelGeo.free("drop"))
            }
        }
    }
}

/// Card 3, « Filtre de sortie »: what the AI sorted here must pass; the others go where the owner says.
struct ExitCard: View {
    @Bindable var model: LabelInspectorModel
    var focus: FocusState<LabelFocus?>.Binding

    var body: some View {
        let f = model.draft.flow
        FlowCardShell(key: "exit", title: Words.text("Exit filter"), subtitle: Words.text("what the AI sorted here must pass"),
                      count: model.exitCountText, on: f.exitOn, toggle: { model.setCard(.exit, $0) }) {
            ChainView(model: model, ref: .exit, card: "exit", lead: Words.text("Keep when"), focus: focus)
            Hair()
            fallbackRow
            if let w = model.refusal(for: [FlowRefusal.exitNeedsFallback, FlowRefusal.badFallback]) {
                InlineError(words: w).padding(.horizontal, WhooshSpacing.s8).padding(.bottom, WhooshSpacing.s8).whooshAx(LabelGeo.free("exit.refusal"))
            }
        }
        .animation(WhooshMotion.hover, value: model.refusal?.words)
    }

    private var fallbackRow: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Text(Words.text("The others go to")).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
            fallbackMenu
            Spacer(minLength: WhooshSpacing.s8)
            if let c = model.counts { Text(LabelsWords.messages(c.sent)).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1) }
        }
        .padding(.horizontal, WhooshSpacing.s12).frame(maxWidth: .infinity, minHeight: WhooshMetrics.listRow, alignment: .leading)
        .whooshAx(LabelGeo.row("exit", "fallback"))
        .accessibilityElement(children: .contain).accessibilityIdentifier("\(LabelGeo.base).flow.exit.fallback.row")
    }

    private var fallbackMenu: some View {
        let place = model.fallbackName(model.draft.flow.fallback)
        let owners = model.core.labels().filter { $0.kind == .owner && $0.id != model.labelId }
        return Menu {
            Button(Words.text("Primary")) { model.setFallback("primary") }.accessibilityIdentifier("\(LabelGeo.base).flow.exit.fallback.primary")
            Button(Words.text("Spam")) { model.setFallback("junk") }.accessibilityIdentifier("\(LabelGeo.base).flow.exit.fallback.junk")
            if !owners.isEmpty { Divider() }
            ForEach(owners, id: \.id) { l in
                Button(l.path.isEmpty ? l.name : l.path) { model.setFallback(l.id) }.accessibilityIdentifier("\(LabelGeo.base).flow.exit.fallback.\(l.id)")
            }
        } label: {
            PopPill { Text(place ?? Words.text("Pick where they go")).foregroundStyle(place == nil ? WhooshPalette.warning : WhooshPalette.label) }
        }
        .menuStyle(.button).buttonStyle(.plain).menuIndicator(.hidden).fixedSize()
        .help(Words.text("Where a message goes when the exit filter does not keep it"))
        .accessibilityIdentifier("\(LabelGeo.base).flow.exit.fallback").whooshAx(LabelGeo.ctl("exit", "fallback"))
    }
}
