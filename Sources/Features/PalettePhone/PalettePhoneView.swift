import SwiftUI

/// The phone palette: an iOS 26 sheet (glass, the one thing that floats). The search field takes the keyboard on the first frame,
/// the commands are one grouped list, a tap runs the line and closes the sheet. Typing never animates: the list answers the key
/// on the same frame; only the refusal line fades in.
struct PalettePhoneSheet: View {
    private let model = PalettePhoneModel.shared
    @FocusState private var focused: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let sections = model.sections
        VStack(spacing: 0) {
            header
            if let line = model.line { refusal(line) }
            if sections.isEmpty { empty } else { list(sections) }
        }
        .animation(reduceMotion ? nil : WhooshMotion.newRow, value: model.line)
        .accessibilityIdentifier("iphone-palette.sheet")
        .onAppear { focused = true }
    }

    private var text: Binding<String> {
        Binding(get: { model.query }, set: { AppModel.shared.typed = $0 })
    }

    private var header: some View {
        HStack(spacing: WhooshSpacing.s8) {
            HStack(spacing: WhooshSpacing.s8) {
                Image(systemName: "magnifyingglass").font(.system(size: PhoneType.headline.size, weight: .semibold))
                    .foregroundStyle(WhooshPalette.secondaryLabel)
                field
                if !model.query.isEmpty {
                    Button { model.clear() } label: {
                        Image(systemName: "xmark.circle.fill").font(.system(size: PhoneType.headline.size))
                            .foregroundStyle(WhooshPalette.tertiaryLabel)
                            .frame(width: PhoneMetrics.roundButton, height: PhoneMetrics.roundButton).contentShape(Rectangle())
                    }
                    .buttonStyle(PressStyle()).accessibilityLabel(PalettePhoneWords.clear)
                    .accessibilityIdentifier("iphone-palette.clear")
                }
            }
            .padding(.leading, PhoneMetrics.gutter)
            .frame(height: PalettePhoneMetrics.fieldHeight)
            .background(Capsule().fill(PhoneGround.trackFill))
            Button { model.close() } label: {
                Text(PalettePhoneWords.close).phoneText(PhoneType.body).foregroundStyle(WhooshPalette.accent)
                    .frame(minWidth: PhoneMetrics.roundButton, minHeight: PhoneMetrics.roundButton).contentShape(Rectangle())
            }
            .buttonStyle(PressStyle()).accessibilityIdentifier("iphone-palette.close")
        }
        .padding(.horizontal, PhoneMetrics.gutter).padding(.top, WhooshSpacing.s24).padding(.bottom, WhooshSpacing.s8)
    }

    @ViewBuilder private var field: some View {
        let f = TextField(PalettePhoneWords.placeholder, text: text)
            .focused($focused).phoneText(PhoneType.body)
            .submitLabel(.go).onSubmit { model.runFirst() }
            .autocorrectionDisabled()
            .accessibilityIdentifier("iphone-palette.field")
        #if os(iOS)
        f.textInputAutocapitalization(.never)
        #else
        f
        #endif
    }

    private func refusal(_ words: String) -> some View {
        HStack(spacing: WhooshSpacing.s6) {
            Circle().fill(WhooshPalette.warning).frame(width: WhooshMetrics.unreadDot, height: WhooshMetrics.unreadDot)
            Text(words).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.label).lineLimit(1)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, PhoneMetrics.gutter).padding(.bottom, WhooshSpacing.s8)
        .transition(.opacity)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("iphone-palette.line")
    }

    private func list(_ sections: [PalettePhoneModel.Section]) -> some View {
        // Each line's index is its place in the flat list the socket and the click targets use: counted up front, never while drawing.
        var starts = [0]
        for s in sections { starts.append(starts[starts.count - 1] + s.items.count) }
        return ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                ForEach(Array(sections.enumerated()), id: \.element.id) { k, section in
                    Text(section.header).phoneText(PhoneType.subhead, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
                        .padding(.horizontal, PhoneMetrics.gutter).padding(.top, WhooshSpacing.s16).padding(.bottom, WhooshSpacing.s4)
                        .accessibilityAddTraits(.isHeader)
                    ForEach(Array(section.items.enumerated()), id: \.element.id) { j, item in
                        let i = starts[k] + j
                        PalettePhoneRow(item: item, index: i) { model.run(i) }
                        Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator)
                            .padding(.leading, PalettePhoneMetrics.separatorInset)
                    }
                }
            }
            .padding(.bottom, PhoneMetrics.gutter)
        }
        .scrollIndicators(.hidden)
        .scrollDismissesKeyboard(.interactively)
        .accessibilityIdentifier("iphone-palette.results")
    }

    private var empty: some View {
        PhoneEmpty(symbol: "magnifyingglass", title: PalettePhoneWords.noMatch(model.query), line: PalettePhoneWords.emptyLine)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
            .accessibilityIdentifier("iphone-palette.empty")
    }
}

/// The door: the screen that opens the palette applies this once, and the sheet follows `AppModel.overlay`.
struct PalettePhoneHost: ViewModifier {
    func body(content: Content) -> some View {
        let open = PalettePhoneModel.shared.isOpen
        content.sheet(isPresented: Binding(get: { open }, set: { if !$0 { PalettePhoneModel.shared.close() } })) {
            PalettePhoneSheet()
                .presentationDetents([.fraction(PalettePhoneMetrics.sheetFraction)])
                .presentationDragIndicator(.visible)
        }
    }
}

public extension View {
    /// Draws the phone palette as a sheet over this view while `PalettePhone.open()` has it open.
    func palettePhoneSheet() -> some View { modifier(PalettePhoneHost()) }
}
