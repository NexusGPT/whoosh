import SwiftUI

/// Welcome and Accounts (Mac). Left: add one address, see how it connects. Right: every account, its progress,
/// drag to order. Native Glass: flat panes, one accent, glass only on the floating controls.
public struct WelcomeView: View {
    @State private var model = WelcomeModel.shared
    @State private var appModel = AppModel.shared
    @State private var importModel = ImportModel.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private let fixedLeft: CGFloat?

    /// The add column is 380 wide in a wide window and 340 in a narrow one, read from the window itself.
    public init(leftWidth: CGFloat? = nil) { fixedLeft = leftWidth }

    public var body: some View {
        GeometryReader { g in
            let leftWidth = fixedLeft ?? (g.size.width >= 1100 ? 380 : 340)
            HStack(spacing: 0) {
                AddAccountView().padding(.top, WhooshMetrics.sidebarTop).padding(.horizontal, WhooshSpacing.s24).frame(width: leftWidth, alignment: .top)   // addaccount r2 hook: the inline card replaces the address column
                Rectangle().fill(WhooshPalette.separator).frame(width: WhooshMetrics.separator)
                listColumn
            }
        }
        .background(WhooshPalette.background)
        .ignoresSafeArea()
        .accessibilityIdentifier("mac-welcome-accounts")
    }

    // MARK: left (the add column is AddAccountView's inline card; its one brand mark is the card's own head)
    private var addressField: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Image(systemName: "at").foregroundStyle(WhooshPalette.secondaryLabel)
            TextField(Words.text("name@example.com"), text: Binding(get: { model.address }, set: { model.address = $0 }))
                .textFieldStyle(.plain).whooshText(WhooshTypography.body).onSubmit { _ = model.addTyped() }
                .accessibilityIdentifier("mac-welcome-accounts.address")
            Button(Words.text("Continue")) { _ = model.addTyped() }.buttonStyle(WelcomeFilledStyle()).disabled(!WelcomeRoutes.isValid(model.address))
                .keyboardShortcut(.return, modifiers: [])
        }
        .padding(.horizontal, WhooshSpacing.s12).frame(height: WhooshMetrics.glassPillHeight + 6)
        .whooshGlass(in: RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous))
    }

    private func routeCard(_ r: WelcomeRoute) -> some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s10) {
            HStack(spacing: WhooshSpacing.s8) {
                Text(r.name).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label)
                    .accessibilityIdentifier("mac-welcome-accounts.detected")
                if r.macOnly { chip(Words.text("This Mac only")) }
                if r.usesSheet { chip(Words.text("Sign-in sheet")) }
            }
            Text(r.how).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.label)
            VStack(alignment: .leading, spacing: WhooshSpacing.s6) {
                ForEach(Array(r.steps.enumerated()), id: \.offset) { i, s in
                    HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s8) {
                        Text("\(i + 1)").whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
                            .frame(width: WhooshMetrics.keyCap, height: WhooshMetrics.keyCap)
                            .background(WhooshPalette.keyCap, in: RoundedRectangle(cornerRadius: WhooshRadius.keyCap, style: .continuous))
                        Text(s).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.label)
                    }
                }
            }
            if let l = r.link, let u = URL(string: l) { Link(r.linkWords ?? Words.text("Open the page"), destination: u).font(.system(size: 12, weight: .semibold)) }
            Text(Words.text("Server: %@", r.server)).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                .accessibilityIdentifier("mac-welcome-accounts.server")
        }
        .padding(WhooshSpacing.s14).frame(maxWidth: .infinity, alignment: .leading)
        .background(WhooshPalette.field, in: RoundedRectangle(cornerRadius: WhooshRadius.hint, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: WhooshRadius.hint, style: .continuous).strokeBorder(WhooshPalette.separator))
    }

    private func chip(_ t: String) -> some View {
        Text(t).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
            .padding(.horizontal, WhooshSpacing.s6)
            .background(WhooshPalette.fill, in: Capsule())
    }

    private var pasteSection: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            Button { model.pasteOpen.toggle() } label: {
                Label(model.pasteOpen ? Words.text("Hide the list") : Words.text("Paste a list of addresses"), systemImage: "list.bullet")
            }.buttonStyle(.plain).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.accent)
                .accessibilityIdentifier("mac-welcome-accounts.paste-list")
            if model.pasteOpen {
                TextEditor(text: $model.pasteText).font(.system(size: 12)).scrollContentBackground(.hidden).frame(height: 96)
                    .padding(WhooshSpacing.s6).background(WhooshPalette.field, in: RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous).strokeBorder(WhooshPalette.separator))
                Button(Words.text("Make a row for each")) { _ = model.addPasted() }.buttonStyle(WelcomeFilledStyle()).disabled(model.pasteText.isEmpty)
                    .accessibilityIdentifier("mac-welcome-accounts.paste-add")
            }
        }
    }

    private var moveButton: some View {
        Button { model.qrOpen = true } label: { Label(Words.text("Move accounts to iPhone"), systemImage: "qrcode") }
            .buttonStyle(.plain).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.accent)
            .accessibilityIdentifier("mac-welcome-accounts.move-to-iphone")
    }

    // MARK: right
    private var listColumn: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                Text(model.rows.isEmpty ? Words.text("Your accounts") : Words.text("Your accounts · %@", model.rows.count)).whooshText(WhooshTypography.title3, emphasized: true)
                    .foregroundStyle(WhooshPalette.label).whooshAx("mac-welcome-accounts.title")   // marked as a leaf text, so the `labels` verb can judge it clipped or not
                Text(summary).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                    .whooshAx("mac-welcome-accounts.summary")
            }.padding(.horizontal, WhooshSpacing.s24).padding(.top, WhooshMetrics.sidebarTop).padding(.bottom, WhooshSpacing.s10)
            if importModel.showing {
                ImportPanelView().padding(.horizontal, WhooshSpacing.s24).padding(.bottom, WhooshSpacing.s10).transition(ImportMotion.enter)
            }
            if model.rows.isEmpty { empty } else {
                List {
                    ForEach(model.rows) { r in WelcomeRowView(row: r, model: model).listRowSeparator(.visible).listRowBackground(Color.clear) }
                        .onMove { model.move(from: $0, to: $1) }
                }.listStyle(.plain).scrollContentBackground(.hidden)
                    .padding(.horizontal, WhooshSpacing.s24)   // the rows, their dividers and the status column end where the import card above them ends (main107: they ran to the window's edge)
                    .animation(WhooshMotion.animation(WhooshMotion.newRow, reduceMotion: reduceMotion), value: model.rows.map(\.id))
            }
            footer
        }.frame(maxWidth: .infinity)
            .animation(ImportMotion.fade(reduceMotion), value: importModel.showing)
            .onChange(of: model.healthyCount) { _, _ in importModel.reconnect() }
            .task {   // the add-account card connects through the core, not through these rows: look until the first account exists, then stop
                while !Task.isCancelled && !importModel.connected { importModel.reconnect(); try? await Task.sleep(for: .seconds(2)) }
            }
    }

    private var summary: String {
        if model.rows.isEmpty { return Words.text("Add the first one on the left.") }
        let f = model.rows.filter { $0.inFlight || $0.phase == .waiting }.count
        return Words.text("%@ ready · %@ in progress · %@ refused. Drag to order.", model.healthyCount, f, model.refusedCount)
    }

    private var empty: some View {
        VStack(spacing: WhooshSpacing.s8) {
            Image(systemName: "tray").font(.system(size: 28)).foregroundStyle(WhooshPalette.tertiaryLabel)
            Text(Words.text("No account yet")).whooshText(WhooshTypography.title3, emphasized: true).foregroundStyle(WhooshPalette.label)
            Text(Words.text("Type an address, or paste several.")).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
        }.frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var footer: some View {
        HStack(spacing: WhooshSpacing.s12) {
            Text(Words.text("Keys")).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
            Picker(Words.text("Keys"), selection: Binding(get: { model.preset }, set: { model.setPreset($0) })) {
                ForEach(["Superhuman", "GotEmail", "Mail"], id: \.self) { Text($0).tag($0) }
            }.pickerStyle(.segmented).labelsHidden().frame(width: 230).accessibilityIdentifier("mac-welcome-accounts.preset")
            Spacer()
            Button(Words.text("Open inbox")) { _ = WelcomeMacFeature.openInbox(appModel) }.buttonStyle(WelcomeFilledStyle()).disabled(!model.canOpenInbox)
                .accessibilityIdentifier("mac-welcome-accounts.open-inbox")
        }.padding(.horizontal, WhooshSpacing.s24).padding(.vertical, WhooshSpacing.s12)
            .overlay(alignment: .top) { Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator) }
    }
}
