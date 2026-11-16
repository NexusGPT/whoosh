import SwiftUI

/// The overlay-slot view (`mac-main.overlay`): the shell's default overlay (the trainer's hint) stays, and the
/// palette rises above it while it is open. The scrim fades; the card scales from its top edge (popoverEntrance).
/// State changes on the input frame; this animation only follows and never gates a key.
struct PaletteMacOverlay: View {
    let model = PaletteMacModel.shared

    var body: some View {
        ZStack {
            RootView.defaultOverlay
            if model.isOpen { PaletteMacView(model: model, app: .shared).transition(.opacity) }
        }
        .animation(WhooshMotion.readerSwap, value: model.isOpen)
    }
}

/// One field, one list in two sections (Commands, Go to), keys right-aligned in key caps, over the inbox it came from.
struct PaletteMacView: View {
    @Bindable var model: PaletteMacModel
    @Bindable var app: AppModel
    @FocusState private var focused: Bool
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .top) {
                Rectangle().fill(WhooshPalette.windowScrim)
                    .contentShape(Rectangle())
                    .onTapGesture { _ = model.close() }
                    .whooshAx("mac-palette.scrim")
                card(windowHeight: geo.size.height)
                    .frame(maxWidth: min(PaletteMacMetrics.width, geo.size.width - PaletteMacMetrics.edgeGutter * 2))
                    .padding(.top, PaletteMacMetrics.verticalGap)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .onAppear { focused = true }
        .onChange(of: app.typed) { _, _ in model.queryChanged() }
    }

    private func card(windowHeight: CGFloat) -> some View {
        VStack(spacing: 0) {
            field
            Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator)
            list(windowHeight: windowHeight)
            Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator)
            footer
        }
        .background(WhooshPalette.panel, in: RoundedRectangle(cornerRadius: WhooshRadius.sheet, style: .continuous))
        .overlay { RoundedRectangle(cornerRadius: WhooshRadius.sheet, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator) }
        .shadow(color: WhooshGlass.shadow.color(scheme), radius: WhooshSpacing.s24, y: WhooshSpacing.s8)
        .popoverEntrance(anchor: .top)
        .accessibilityElement(children: .contain)
        .whooshAx("mac-palette")
    }

    private var field: some View {
        HStack(spacing: WhooshSpacing.s10) {
            Image(systemName: model.step == .commands ? "command" : "arrow.right.circle")
                .font(WhooshTypography.title3.font(emphasized: false)).foregroundStyle(WhooshPalette.secondaryLabel)
            TextField(model.step == .commands ? PaletteMacWords.commandsPlaceholder : PaletteMacWords.accountsPlaceholder, text: $app.typed)
                .textFieldStyle(.plain).whooshText(WhooshTypography.title3)
                .focused($focused)
                .onSubmit { _ = model.run() }
                .onKeyPress(.downArrow) { model.move(1) ? .handled : .ignored }
                .onKeyPress(.upArrow) { model.move(-1) ? .handled : .ignored }
                .whooshAx("mac-palette.field")
            KeyCap(PaletteMacWords.escKey)
        }
        .padding(.horizontal, WhooshSpacing.s16)
        .frame(height: PaletteMacMetrics.fieldHeight)
    }

    private func list(windowHeight: CGFloat) -> some View {
        let sections = model.step == .commands ? (model.commands.isEmpty ? 0 : 1) + (model.jumps.isEmpty ? 0 : 1) : 1
        return ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: PaletteMacMetrics.rowGap) {
                    if model.step == .commands { commandSections } else { accountRows }
                    if model.count == 0 { nothing }
                }
                .padding(PaletteMacMetrics.listPadding)
            }
            .frame(height: PaletteMacMetrics.listHeight(rows: model.count, headers: sections, windowHeight: windowHeight))
            .onChange(of: model.selected) { _, i in
                let ids = model.step == .commands ? model.items.map(\.id) : model.accounts.map(\.id)
                if ids.indices.contains(i) { proxy.scrollTo(ids[i]) }
            }
        }
    }

    @ViewBuilder private var commandSections: some View {
        let commands = model.commands, jumps = model.jumps
        if !commands.isEmpty {
            header(PaletteMacWords.commandsHeader)
            ForEach(Array(commands.enumerated()), id: \.element.id) { i, item in row(item, index: i, ax: "mac-palette.results.\(i)") }
        }
        if !jumps.isEmpty {
            header(PaletteMacWords.jumpHeader)
            ForEach(Array(jumps.enumerated()), id: \.element.id) { j, item in row(item, index: commands.count + j, ax: "mac-palette.jumps.\(j)") }
        }
    }

    private func row(_ item: PaletteItem, index: Int, ax: String) -> some View {
        Button { _ = model.run(index) } label: { PaletteRowView(item: item, highlighted: index == model.selected) }
            .buttonStyle(.plain)
            .id(item.id)
            .whooshAx(ax)
    }

    @ViewBuilder private var accountRows: some View {
        header(PaletteMacWords.accountsHeader)
        ForEach(Array(model.accounts.enumerated()), id: \.element.id) { i, a in
            Button { _ = model.pick(i) } label: { PaletteAccountRowView(account: a, highlighted: i == model.selected) }
                .buttonStyle(.plain)
                .whooshAx("mac-palette.accounts.\(i)")
        }
    }

    private func header(_ text: String) -> some View {
        Text(text).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
            .frame(maxWidth: .infinity, minHeight: PaletteMacMetrics.headerHeight, alignment: .bottomLeading)
            .padding(.horizontal, WhooshSpacing.s10)
            .accessibilityAddTraits(.isHeader)
    }

    private var nothing: some View {
        Text(PaletteMacWords.empty(app.typed))
            .whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.secondaryLabel)
            .frame(maxWidth: .infinity, minHeight: PaletteMacMetrics.emptyHeight)
            .whooshAx("mac-palette.empty")
    }

    private var footer: some View {
        HStack(spacing: WhooshSpacing.s14) {
            KeyHint(["↑", "↓"], PaletteMacWords.move)
            KeyHint("↵", model.step == .accounts ? PaletteMacWords.moveHere : PaletteMacWords.run)
            Spacer(minLength: 0)
            KeyHint(PaletteMacWords.escKey, PaletteMacWords.close)
        }
        .padding(.horizontal, WhooshSpacing.s10).padding(.vertical, WhooshSpacing.s6)
        .frame(height: PaletteMacMetrics.footerHeight)
    }
}
