import SwiftUI

/// Pane 3 in its second mode (docs/surfaces/mac-labels-contacts.md, picked look LC-S1 "Quiet inspector", flow picked 2026-11-09, names A): a calm
/// Settings-style form beside the live list. Three compact cards in the order a message meets them (Entry rules, AI sorting, Exit filter), then
/// the Label card on the shared form grid, then the foot that says where the messages went and creates, applies or asks to re-run.
/// Every gap is a token; the geometry check (rows `mac-labels.inspector.geometry`) proves nothing overlaps.
struct LabelInspectorView: View {
    @State private var model = LabelInspectorModel.shared  // local-ui: the inspector's own state, observed
    @FocusState private var focus: LabelFocus?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(spacing: 0) {
            toolbar
            header
            ScrollView {
                VStack(alignment: .leading, spacing: WhooshSpacing.s16) {
                    EntryCard(model: model, focus: $focus)
                    AiCard(model: model, focus: $focus)
                    ExitCard(model: model, focus: $focus)
                    LabelBlocks(model: model, focus: $focus)
                }
                .padding(.horizontal, WhooshMetrics.readerSide)
                .padding(.bottom, WhooshSpacing.s24)
            }
            .scrollIndicators(.hidden)
            .whooshAx(LabelGeo.clip("scroll"))
            .overlay(alignment: .bottom) {
                if model.whyOpen {
                    LabelWhyPanel(model: model).padding(.horizontal, WhooshMetrics.readerSide).padding(.bottom, WhooshSpacing.s8)
                        .transition(.opacity.combined(with: .offset(y: WhooshMotion.undoRise)))
                }
            }
            .animation(WhooshMotion.animation(WhooshMotion.readerSwap, reduceMotion: reduceMotion), value: model.whyOpen)
            LabelInspectorFoot(model: model)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .background(WhooshPalette.background)
        .onChange(of: focus) { _, f in
            model.editing = f != nil
            if case .rule(let id)? = f { model.focusedRule = id } else { model.focusedRule = nil }
        }
        .onChange(of: model.blurTick) { _, _ in focus = nil }
        .onChange(of: model.focusRequest) { _, r in if let r { focus = r; model.focusRequest = nil } }
        .onChange(of: model.app.focusedMessageId) { _, _ in if model.whyOpen { model.refreshWhy() } }
        .onAppear { if model.isNew { focus = .name } }
        .accessibilityElement(children: .contain)
        .whooshAx("mac-labels.inspector")
    }

    /// The reader's toolbar row keeps its place: the actions step back (they belong to a message), the inspector button is on.
    private var toolbar: some View {
        HStack(spacing: WhooshSpacing.s2) {
            Spacer(minLength: 0)
            ForEach(["archivebox", "clock", "tag", "pin", "arrowshape.turn.up.left", "ellipsis"], id: \.self) { s in
                Image(systemName: s).font(.system(size: WhooshTypography.body.size, weight: .medium))
                    .foregroundStyle(WhooshPalette.tertiaryLabel)
                    .frame(width: WhooshSpacing.s32, height: ReaderIconButton.height)
            }
            .accessibilityHidden(true).allowsHitTesting(false)
            Rectangle().fill(WhooshPalette.separator).frame(width: WhooshMetrics.separator, height: WhooshSpacing.s16).padding(.horizontal, WhooshSpacing.s4)
            InspectorToggleButton(on: true) { _ = model.close() }.whooshAx(LabelGeo.free("toggle"))
        }
        .padding(.horizontal, WhooshMetrics.readerSide - WhooshSpacing.s8).padding(.top, WhooshSpacing.s16)
    }

    private var header: some View {
        HStack(alignment: .top, spacing: WhooshSpacing.s10) {
            Circle().fill(dotFill).frame(width: WhooshSpacing.s12, height: WhooshSpacing.s12)
                .overlay { Circle().strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator) }
                .padding(.top, WhooshSpacing.s14).animation(WhooshMotion.hover, value: model.draft.colour)
            VStack(alignment: .leading, spacing: 0) {
                breadcrumb
                Text(model.draft.name.isEmpty ? Words.text("New label") : model.draft.name)
                    .whooshText(WhooshTypography.largeTitle, emphasized: true).foregroundStyle(WhooshPalette.label).lineLimit(1)
                    .accessibilityIdentifier("mac-labels.inspector.title").whooshAx(LabelGeo.free("title"))
                Text(statusWords).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                    .accessibilityIdentifier("mac-labels.inspector.status").whooshAx(LabelGeo.free("status"))
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, WhooshMetrics.readerSide).padding(.top, WhooshSpacing.s8).padding(.bottom, WhooshSpacing.s16)
    }

    private var dotFill: AnyShapeStyle {
        model.draft.colour.map { AnyShapeStyle(AccountColour.ring($0)) } ?? AnyShapeStyle(WhooshPalette.tertiaryLabel)
    }

    private var breadcrumb: some View {
        HStack(spacing: WhooshSpacing.s4) {
            Text(Words.text("Labels"))
            if let p = model.parentLabel {
                Image(systemName: "chevron.right").font(WhooshTypography.caption.font).imageScale(.small)
                Text(p.name)
            }
        }
        .whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
    }

    private var statusWords: String {
        if let s = model.statusLine { return s }
        if model.isNew { return Words.text("New label · not created yet") }
        if model.dirty { return model.changes == 1 ? Words.text("1 change not applied yet") : Words.text("%lld changes not applied yet", model.changes) }
        if let c = model.counts { return Words.text("Saved · %lld messages", c.held(aiOn: model.saved.flow.aiOn, exitOn: model.saved.flow.exitOn)) }
        return Words.text("Saved · new mail follows its rules")
    }
}

/// What holds the keys inside the inspector.
enum LabelFocus: Hashable { case name, meaning, rule(UUID) }

/// The reader toolbar's last button: the label inspector, lit while it is open. ⌘I.
struct InspectorToggleButton: View {
    let on: Bool
    let action: () -> Void
    @State private var hovering = false  // local-ui: hover tint only
    var body: some View {
        Button(action: action) {
            Image(systemName: "sidebar.right").font(.system(size: WhooshTypography.body.size, weight: .medium))
                .foregroundStyle(on ? WhooshPalette.accent : (hovering ? WhooshPalette.label : WhooshPalette.secondaryLabel))
                .frame(width: WhooshSpacing.s32, height: ReaderIconButton.height)
                .background { RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous).fill(on ? WhooshPalette.fill : WhooshPalette.hover).opacity(on || hovering ? 1 : 0) }
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain).onHover { hovering = $0 }.animation(WhooshMotion.hover, value: hovering)
        .help(Words.text("Label inspector") + " · ⌘I · Esc")
        .accessibilityLabel(Words.text("Label inspector")).accessibilityIdentifier("mac-labels.inspector.toolbar")
    }
}
