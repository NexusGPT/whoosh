import SwiftUI

/// What floats at the foot of the screen, docked over the keyboard: the cards (a filter picker, the name of a smart folder, the
/// meaning model's line), the filter chips, then the field. Glass only on these; the list under them is paper.
struct SearchPhoneBar: View {
    let model: SearchPhoneModel
    @Bindable var app: AppModel
    @FocusState private var focused: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(spacing: PhoneMetrics.barGap) {
            if !model.domainOffers.isEmpty, model.menu == nil, !model.naming { SearchPhoneDomainCard(model: model).transition(card) }
            if let m = model.menu { SearchPhoneMenuCard(model: model, menu: m).transition(card) }
            if model.naming { SearchPhoneNameCard(model: model).transition(card) }
            if model.meaningLineVisible { SearchPhoneMeaningCard(model: model).transition(card) }
            SearchPhoneChips(model: model)
            HStack(spacing: PhoneMetrics.barGap) {
                field
                GlassCircle(symbol: "xmark", side: PhoneMetrics.barButton, label: Words.text("Close search")) { _ = model.close() }
                    .accessibilityIdentifier("iphone-search.close")
            }
            .padding(.horizontal, PhoneMetrics.gutter)
        }
        .animation(reduceMotion ? nil : WhooshMotion.newRow, value: model.menu)
        .animation(reduceMotion ? nil : WhooshMotion.newRow, value: model.domainOffers.map(\.id))
        .animation(reduceMotion ? nil : WhooshMotion.newRow, value: model.naming)
        .animation(reduceMotion ? nil : WhooshMotion.newRow, value: model.meaningLineVisible)
        .onChange(of: app.typed) { model.typedChanged() }
        .onAppear { focused = true }
    }

    /// A card rises UNDO_RISE_PX on the insert curve and fades: the inbox capsule's own move.
    private var card: AnyTransition {
        reduceMotion ? .opacity : .offset(y: WhooshMotion.undoRise).combined(with: .opacity)
    }

    /// The field: Return keeps the typed words as a recent search and puts the keyboard away so the results show whole.
    private var field: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Image(systemName: "magnifyingglass").font(.system(size: PhoneType.headline.size, weight: .semibold)).foregroundStyle(WhooshPalette.label)
            TextField(Words.text("Search"), text: $app.typed)
                .focused($focused).phoneText(PhoneType.body)
                .submitLabel(.search)
                .autocorrectionDisabled()
                #if os(iOS)
                .textInputAutocapitalization(.never)
                #endif
                .onSubmit { _ = model.submit(); focused = false }
                .accessibilityIdentifier("iphone-search.field")
            if !app.typed.isEmpty {
                Button { _ = model.clearText(); focused = true } label: {
                    Image(systemName: "xmark.circle.fill").font(.system(size: PhoneType.headline.size))
                        .foregroundStyle(WhooshPalette.tertiaryLabel).frame(width: PhoneMetrics.mic, height: PhoneMetrics.mic).contentShape(Circle())
                }
                .buttonStyle(PressStyle()).accessibilityLabel(Words.text("Clear")).accessibilityIdentifier("iphone-search.field.clear")
                .transition(.opacity)
            }
        }
        .padding(.leading, PhoneMetrics.gutter).padding(.trailing, WhooshSpacing.s8)
        .frame(maxWidth: .infinity).frame(height: PhoneMetrics.barButton)
        .whooshGlass(in: Capsule())
        .contentShape(Capsule())
        .onTapGesture { focused = true }
    }
}

/// Account · Sender · Date · Attachments: four glass chips that scroll sideways. A chip with a value shows it and a × that takes it off;
/// the picker opens from the chip. Each target is 44 pt tall, the chip itself 32 pt (the inbox's segment).
struct SearchPhoneChips: View {
    let model: SearchPhoneModel

    var body: some View {
        ScrollView(.horizontal) {
            HStack(spacing: PhoneMetrics.barGap) {
                ForEach(SearchPhoneRows.chips) { c in chip(c) }
            }
            .padding(.horizontal, PhoneMetrics.gutter)
        }
        .scrollIndicators(.hidden)
        .frame(height: PhoneMetrics.roundButton)
        .accessibilityIdentifier("iphone-search.filters")
    }

    private func chip(_ c: SearchPhoneChip) -> some View {
        let on = c.value != nil
        let open = model.menu?.rawValue == c.kind
        return HStack(spacing: 0) {
            Button { toggle(c.kind) } label: {
                HStack(spacing: WhooshSpacing.s6) {
                    Text(SearchPhoneRows.chipLabel(c)).phoneText(PhoneType.subhead, emphasized: on).foregroundStyle(WhooshPalette.label).lineLimit(1)
                    if !on && c.kind != "attachment" && c.kind != "domain" {
                        Image(systemName: "chevron.down").font(.system(size: PhoneType.footnote.size - 1, weight: .semibold))
                            .foregroundStyle(WhooshPalette.secondaryLabel)
                    }
                }
                .padding(.leading, PhoneMetrics.segmentPad).padding(.trailing, on ? WhooshSpacing.s4 : PhoneMetrics.segmentPad)
                .frame(height: PhoneMetrics.roundButton).contentShape(Rectangle())
            }
            .buttonStyle(PressStyle()).accessibilityIdentifier("iphone-search.filter.\(c.kind)")
            .accessibilityAddTraits(on ? .isSelected : [])
            if on {
                Button { _ = model.clearFilter(c.kind) } label: {
                    Image(systemName: "xmark").font(.system(size: PhoneType.footnote.size, weight: .semibold)).foregroundStyle(WhooshPalette.secondaryLabel)
                        .frame(width: PhoneMetrics.roundButton - WhooshSpacing.s8, height: PhoneMetrics.roundButton).contentShape(Rectangle())
                }
                .buttonStyle(PressStyle()).accessibilityLabel(Words.text("Clear")).accessibilityIdentifier("iphone-search.filter.\(c.kind).clear")
            }
        }
        .frame(height: PhoneMetrics.roundButton)
        .background {
            ZStack {
                Capsule().fill(Color.clear).whooshGlass(in: Capsule())
                Capsule().fill(WhooshPalette.accent.opacity(on || open ? SearchPhoneMetrics.chipTint : 0))
            }
            .frame(height: PhoneMetrics.segmentHeight)
        }
        .animation(WhooshMotion.hover, value: on)
    }

    private func toggle(_ kind: String) {
        if kind == "attachment" { _ = model.toggleAttachment() } else if kind == "domain" { _ = model.clearFilter(kind) } else if let m = SearchPhoneMenu(rawValue: kind) { _ = model.toggleMenu(m) }
    }
}
