#if os(macOS)
import AppKit
import SwiftUI
import UniformTypeIdentifiers

/// The icon and colour picker (ix-account-icon-picker): the whole emblem library by family, the colour swatches, and an
/// uploaded image. A pick shows on the live preview, the row behind it and every other mark on the same frame.
struct AccountIconPicker: View {
    let session: AccountsSession
    let account: AccountEntry
    @State private var dropping = false

    private var palette: [String] {
        var seen: [String] = []
        for a in session.accounts where !a.colour.isEmpty && !seen.contains(a.colour) { seen.append(a.colour) }
        return seen.sorted { (EmblemRGB(hex: $0)?.hue ?? 0) < (EmblemRGB(hex: $1)?.hue ?? 0) }
    }

    var body: some View {
        ZStack {
            Rectangle().fill(WhooshPalette.windowScrim).ignoresSafeArea().onTapGesture { session.editing = nil }
            VStack(alignment: .leading, spacing: 0) {
                header
                Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator)
                ScrollView {
                    VStack(alignment: .leading, spacing: WhooshSpacing.s20) {
                        colours
                        upload
                        ForEach(EmblemFamily.allCases, id: \.self) { family in library(family) }
                    }
                    .padding(WhooshSpacing.s20)
                }
                .scrollIndicators(.never)
            }
            .frame(width: AccountsLayout.pickerWidth)
            .frame(maxHeight: AccountsLayout.pickerMaxHeight)
            .background(RoundedRectangle(cornerRadius: WhooshRadius.sheet, style: .continuous).fill(WhooshPalette.background))
            .overlay(RoundedRectangle(cornerRadius: WhooshRadius.sheet, style: .continuous)
                .strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator))
            .shadow(color: .black.opacity(0.22), radius: 24, y: 10)
            .accessibilityElement(children: .contain).whooshAx("mac-settings-accounts.picker")
        }
        // `fixture upload-icon`: while this picker is open the verb stands in for the file panel (FixtureEvents.uploadIcon)
        .onAppear { FixtureEvents.uploadIcon = { arg in AccountIconFixtureDoor.take(arg, session: session, accountId: account.id) } }
        .onDisappear { FixtureEvents.uploadIcon = nil }
    }

    private var header: some View {
        HStack(spacing: WhooshSpacing.s16) {
            EmblemMark(glyph: EmblemLibrary.resolve(account.iconId), colourHex: account.colour, size: .settings,
                       imagePath: account.imagePath, name: account.name, address: account.address)
            VStack(alignment: .leading, spacing: WhooshSpacing.s4) {
                Text(AccountsWords.pickerTitle).whooshText(WhooshTypography.title3).foregroundStyle(WhooshPalette.label)
                Text(account.name).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label)
                Text(AccountsWords.pickerLead).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: WhooshSpacing.s12)
            AccountsPill(title: AccountsWords.done, filled: true, id: "mac-settings-accounts.picker.done") { session.editing = nil }
        }
        .padding(WhooshSpacing.s20)
    }

    private var colours: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            Text(AccountsWords.colourHeader).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
            LazyVGrid(columns: [GridItem(.adaptive(minimum: WhooshAccountMark.swatch + WhooshSpacing.s8), spacing: WhooshSpacing.s6)], alignment: .leading, spacing: WhooshSpacing.s6) {
                ForEach(palette, id: \.self) { hex in
                    let selected = hex == account.colour
                    Button { session.pickColour(hex, for: account.id) } label: {
                        Circle().fill(Color(emblem: hex)).frame(width: WhooshAccountMark.swatch, height: WhooshAccountMark.swatch)
                            .padding(WhooshSpacing.s2)
                            .overlay(Circle().strokeBorder(WhooshPalette.label, lineWidth: selected ? WhooshAccountMark.pickWidth : 0))
                            .contentShape(Circle())
                    }
                    .buttonStyle(.plain).hoverAck(radius: WhooshRadius.glassPill)
                    .accessibilityLabel(hex).accessibilityValue(selected ? "chosen" : "")
                    .accessibilityIdentifier("mac-settings-accounts.colour.\(hex.trimmingCharacters(in: CharacterSet(charactersIn: "#")))")
                }
            }
        }
    }

    private var upload: some View {
        let note = session.refusal(account.id)
        return VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            Text(AccountsWords.uploadHeader).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
            HStack(spacing: WhooshSpacing.s12) {
                AccountsPill(title: AccountsWords.uploadButton, filled: false, id: "mac-settings-accounts.upload") { choose() }
                Text(note ?? AccountsWords.uploadHint)
                    .whooshText(WhooshTypography.subhead).foregroundStyle(note == nil ? WhooshPalette.secondaryLabel : WhooshPalette.warning)
                    .contentTransition(.opacity)
                Spacer(minLength: 0)
            }
            .padding(WhooshSpacing.s12)
            .background(RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous).fill(dropping ? WhooshPalette.mine : WhooshPalette.fill))
            .onDrop(of: [.fileURL], isTargeted: $dropping) { providers in drop(providers) }
        }
    }

    private func library(_ family: EmblemFamily) -> some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            Text(AccountsWords.familyTitle(family.rawValue)).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
            LazyVGrid(columns: [GridItem(.adaptive(minimum: CGFloat(WhooshConstants.ACCOUNT_MARK_SETTINGS_PX) + WhooshSpacing.s8), spacing: WhooshSpacing.s6)], alignment: .leading, spacing: WhooshSpacing.s6) {
                ForEach(EmblemLibrary.glyphs(in: family)) { g in
                    let selected = account.imagePath == nil && EmblemLibrary.resolve(account.iconId).id == g.id
                    Button { session.pickGlyph(g.id, for: account.id) } label: {
                        EmblemMark(glyph: g, colourHex: account.colour, size: .settings, name: g.name)
                            .padding(WhooshSpacing.s2)
                            .overlay(Circle().strokeBorder(WhooshPalette.accent, lineWidth: selected ? WhooshAccountMark.pickWidth : 0))
                            .contentShape(Circle())
                    }
                    .buttonStyle(.plain).hoverAck(radius: WhooshRadius.glassPill)
                    .accessibilityLabel(g.name).accessibilityValue(selected ? "chosen" : "")
                    .accessibilityIdentifier("mac-settings-accounts.icon-library.\(g.id)")
                }
            }
        }
    }

    private func choose() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.image]; panel.allowsMultipleSelection = false; panel.canChooseDirectories = false
        guard panel.runModal() == .OK, let url = panel.url else { return }
        keep(url)
    }

    private func drop(_ providers: [NSItemProvider]) -> Bool {
        guard let p = providers.first(where: { $0.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) }) else { return false }
        _ = p.loadObject(ofClass: URL.self) { url, _ in
            guard let url else { return }
            DispatchQueue.main.async { keep(url) }
        }
        return true
    }

    private func keep(_ url: URL) { _ = AccountIconFixtureDoor.keep(url, session: session, accountId: account.id) }
}

/// What the file panel, a drop and `fixture upload-icon` share: the size gate, the square crop at ACCOUNT_ICON_PX (2x), the core's
/// SetAccountIcon, and the refusal in words under the row. The verb only replaces the panel; the words and the stored image are the product.
@MainActor
enum AccountIconFixtureDoor {
    /// A picture at `url` for the account. `accepted` is true when the core holds it; `words` is the refusal ("" when accepted).
    @discardableResult
    static func keep(_ url: URL, session: AccountsSession, accountId id: String) -> (accepted: Bool, words: String) {
        let size = (try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0
        if let why = AccountIconStore.verdict(byteCount: size) { session.refuse(why, for: id); return (false, why) }
        guard let data = try? Data(contentsOf: url) else { session.refuse(AccountsWords.notAnImage, for: id); return (false, AccountsWords.notAnImage) }
        return finish(AccountIconStore.store(data, accountId: id), session: session, id: id)
    }

    /// `fixture upload-icon <bytes|path>`: a whole number is the size the gate sees (a generated picture), else a file path.
    static func take(_ arg: String, session: AccountsSession, accountId id: String) -> (accepted: Bool, words: String) {
        if let n = Int(arg), n >= 0 { return finish(AccountIconStore.fixtureUpload(declaredBytes: n, accountId: id), session: session, id: id) }
        return keep(URL(fileURLWithPath: (arg as NSString).expandingTildeInPath), session: session, accountId: id)
    }

    private static func finish(_ r: Result<String, AccountIconStore.StoreError>, session: AccountsSession, id: String) -> (accepted: Bool, words: String) {
        switch r {
        case .success(let path):
            session.setImage(path: path, for: id)
            let note = session.refusal(id)
            return (note == nil, note ?? "")
        case .failure(let e): session.refuse(e.words, for: id); return (false, e.words)
        }
    }

    /// Rows `mac-settings-accounts.icon.stored`: one per account holding an uploaded image; `px` the stored square side at 1x
    /// (ACCOUNT_ICON_PX), `pixels` the file's real width (2x), `pixels_high` its real height (2x, so a square file reads equal), `bytes` the file size.
    static func storedRows(_ session: AccountsSession) -> [ControlRow] {
        session.accounts.compactMap { a in
            guard let path = a.imagePath, let data = FileManager.default.contents(atPath: path), let rep = NSBitmapImageRep(data: data) else { return nil }
            return ["account": a.id, "px": rep.pixelsWide / 2, "pixels": rep.pixelsWide, "pixels_high": rep.pixelsHigh, "bytes": data.count]
        }
    }
}

private extension Color {
    /// An account colour from its stored "#RRGGBB" (the core's own form).
    init(emblem hex: String) {
        let c = EmblemRGB(hex: hex) ?? EmblemRGB(0.5, 0.5, 0.5)
        self.init(.sRGB, red: c.r, green: c.g, blue: c.b, opacity: 1)
    }
}

private extension EmblemRGB {
    /// Hue in turns (0 to 1), to order the swatches around the wheel.
    var hue: Double {
        let mx = max(r, g, b), mn = min(r, g, b), d = mx - mn
        guard d > 0 else { return 0 }
        let h = mx == r ? ((g - b) / d).truncatingRemainder(dividingBy: 6) : (mx == g ? (b - r) / d + 2 : (r - g) / d + 4)
        return (h < 0 ? h + 6 : h) / 6
    }
}
#endif
