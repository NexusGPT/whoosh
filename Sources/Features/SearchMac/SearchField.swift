import SwiftUI

/// The field: a glass pill with the chips of the accepted filters before the words. It keeps its place and its key;
/// what changes is what opens under it. While a search runs, one light travels along its lower rim.
struct SearchField: View {
    let model: SearchMacModel
    @Bindable var app: AppModel
    @FocusState private var focused: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    /// the list header's glass pill (WhooshMetrics.glassPillHeight): the field takes the place of the list's own command pill
    static let height = WhooshMetrics.glassPillHeight

    /// What the text view shows: the words after the chips. A key writes the chips and these words back as the bar's text.
    private var draft: Binding<String> {
        Binding(get: { SearchQuery.draft(of: model.text) },
                set: { new in
                    guard new != SearchQuery.draft(of: model.text) else { return }   // the view re-writing its own text is not an edit
                    model.text = SearchQuery.text(chips: SearchQuery.typed(model.text).chips, draft: new)
                })
    }

    var body: some View {
        let chips = model.naming ? [] : model.query.chips
        HStack(spacing: WhooshSpacing.s6) {
            Image(systemName: "magnifyingglass").font(.system(size: 12.5, weight: .medium)).foregroundStyle(WhooshPalette.secondaryLabel)
            ForEach(Array(chips.enumerated()), id: \.element.kind) { i, chip in
                SearchChipView(chip: chip, shown: model.pillNames[chip.value.lowercased()]) { _ = model.removeChip(i) }
                    .accessibilityIdentifier("mac-search.chips.\(i)")
                    .transition(.scale(scale: 0.88).combined(with: .opacity))
            }
            if model.naming {
                Text(model.savedQuery).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                Spacer(minLength: 0)
            } else {
                TextField(Words.text("Search"), text: draft)
                    .textFieldStyle(.plain).whooshText(WhooshTypography.body)
                    .focused($focused)
                    .onSubmit { model.claim(); _ = model.enter() }   // Return typed IN the field is always the search's: never the list's Enter (reply), even if the field had let go of the keys
                    // Real keys: the shell's monitor (ChordKit.handle) hands Down, Up, Tab and Return to a text field that has the cursor, so the
                    // field itself must send them to the search. Each takes the keys first: a field with the cursor is the search's, whatever released.
                    .onKeyPress(.downArrow) { model.claim(); return model.move(1) ? .handled : .ignored }
                    .onKeyPress(.upArrow) { model.claim(); return model.move(-1) ? .handled : .ignored }
                    .onKeyPress(.tab) { model.claim(); _ = model.tab(); return .handled }   // never moves the cursor out of the field
                    .onKeyPress(.return) { model.claim(); return model.enter() ? .handled : .ignored }
                    .onKeyPress(.escape) { model.holds ? (model.close() ? .handled : .ignored) : .ignored }   // the field's own Esc: clear, close, back to the list
                    .whooshAx("mac-search.bar")
            }
            if !model.barText.isEmpty && !model.naming {
                Button { model.clearField() } label: {
                    Image(systemName: "xmark.circle.fill").font(.system(size: 13)).foregroundStyle(WhooshPalette.tertiaryLabel)
                }
                .buttonStyle(.plain).accessibilityLabel(Words.text("Clear")).whooshAx("mac-search.bar.clear")
            }
        }
        .animation(WhooshMotion.animation(.spring(response: 0.22, dampingFraction: 0.85), reduceMotion: reduceMotion), value: chips.map(\.kind))
        .padding(.leading, WhooshSpacing.s10 + 1).padding(.trailing, WhooshSpacing.s10)
        .frame(height: Self.height)
        .whooshGlass(in: Capsule(style: .continuous))
        .overlay { if focused { Capsule(style: .continuous).stroke(WhooshPalette.accent.opacity(0.16), lineWidth: 3).padding(-1.5) } }
        .overlay(alignment: .bottom) { if model.loading { RunningLight().clipShape(Capsule(style: .continuous)) } }
        // The field takes the keyboard only when the model says it holds it (⌘K, `/`, a click on it); it never grabs it on its own.
        .onAppear { focused = model.holds; ClickAway.shared.start(model) }
        .onChange(of: model.focusTick) { _, _ in focused = model.holds }
        .onChange(of: model.holds) { _, h in focused = h }
        .onChange(of: focused) { _, f in
            model.fieldFocused = f
            // a focus lost in the moments after the keys were claimed is the window settling (another text view, such as a reply that just closed, gives
            // the cursor up late), not the owner leaving: the keys stay (main93 to main103: ⌘K after a reply let go of the keys at once, the bar trace
            // said surface mac-main, holds False, active True right after ⌘K). A later loss of the cursor lets go as before.
            if f { if !model.holds { model.caret = true }; model.claim() } else if model.holds, !model.naming, Date().timeIntervalSince(model.claimedTime) > SearchMacModel.claimSettleSeconds {
                model.release("field-lost-the-cursor")
            }   // naming: the name field takes the keys, the field is not on screen
        }
        .onChange(of: app.typed) { _, _ in model.typedChanged() }
        .onChange(of: app.surface) { _, _ in model.surfaceMoved() }
        .onChange(of: app.focusIndex) { _, _ in model.focusMoved() }
        .background(FrameReader { model.fieldFrame = $0 })
        .onDisappear { model.fieldFocused = false; ClickAway.shared.stop() }
        .accessibilityElement(children: .contain)
    }
}

struct SearchChipView: View {
    let chip: SearchChip
    /// what to draw in place of the value (a person's name for the address the pill carries)
    var shown: String? = nil
    let remove: () -> Void
    @State private var hovering = false

    /// The pill's words: the kind in the regular weight, the value in semibold ("From" then "Claire Martin").
    private var label: Text {
        let v = shown ?? chip.valueTitle
        let t = shown == nil ? chip.title : chip.title.replacingOccurrences(of: chip.valueTitle, with: v)
        guard !v.isEmpty, let r = t.range(of: v) else { return Text(t).fontWeight(.semibold) }
        return Text(t[..<r.lowerBound]).fontWeight(.regular) + Text(t[r]).fontWeight(.semibold) + Text(t[r.upperBound...]).fontWeight(.regular)
    }

    var body: some View {
        HStack(spacing: WhooshSpacing.s4) {
            label.font(WhooshTypography.subhead.font(emphasized: false)).lineLimit(1).truncationMode(.tail)
            Button(action: remove) { Image(systemName: "xmark").font(WhooshTypography.caption.font(emphasized: true)) }
                .buttonStyle(.plain)
                .opacity(hovering ? 1 : 0.6)
                .accessibilityLabel(Words.text("Remove %@", chip.title))
        }
        .foregroundStyle(WhooshPalette.accent)
        .padding(.horizontal, WhooshSpacing.s8)
        .frame(height: SearchField.height - WhooshSpacing.s8)
        .frame(maxWidth: 150)
        .background(Capsule().fill(WhooshPalette.mine))
        .onHover { h in withAnimation(WhooshMotion.hover) { hovering = h } }
        .fixedSize(horizontal: true, vertical: false)
    }
}

/// One light along the field's lower rim while a search runs (900 ms loop, transform only); gone when the last row lands.
struct RunningLight: View {
    @State private var on = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.colorScheme) private var scheme
    var body: some View {
        GeometryReader { g in
            Rectangle()
                .fill(LinearGradient(colors: [.clear, WhooshPalette.accent.color(scheme).opacity(0.75), .clear], startPoint: .leading, endPoint: .trailing))
                .frame(width: g.size.width * 0.38, height: 1.5)
                .offset(x: on ? g.size.width : -g.size.width * 0.38)
        }
        .frame(height: 1.5).allowsHitTesting(false)
        .onAppear { if !reduceMotion { withAnimation(.timingCurve(0.45, 0, 0.25, 1, duration: 0.9).repeatForever(autoreverses: false)) { on = true } } }
    }
}
