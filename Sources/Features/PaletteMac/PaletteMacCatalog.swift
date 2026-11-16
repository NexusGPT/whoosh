import Foundation

/// The two pools the palette ranks: commands (every feature's `paletteCommands()`, the command registry's titles
/// first) and jumps (accounts from AccountBook, split tabs and labels from the core, Settings pages).
/// Built once when the palette opens; typing only ranks them.
enum PaletteCatalog {
    @MainActor
    static func commands() -> [PaletteItem] {
        var seen = Set<String>(), out: [PaletteItem] = []
        for feature in FeatureRegistry.all {
            for c in feature.paletteCommands() {
                let title = display(c.title)
                guard seen.insert(title.lowercased()).inserted else { continue }
                let (what, kind) = act(for: c)
                out.append(PaletteItem(id: "command:\(title)", title: Words.text(title), key: key(for: c, act: what), kind: kind, act: what))
            }
        }
        return out
    }

    @MainActor
    private static func act(for c: (title: String, key: String)) -> (PaletteItem.Act, PaletteItem.Kind) {
        if c.title == PaletteMacWords.moveTitle { return (.moveAccount, .command) }
        if c.title == PaletteMacWords.markVip { return (.markVip(true), .command) }
        if c.title == PaletteMacWords.removeVip { return (.markVip(false), .command) }
        if c.title == PaletteMacWords.generalSettings { return (.openSettings(SurfaceSlot.macSettingsGeneral.id), .settings) }
        if let id = Commands.all.first(where: { $0.title.lowercased() == c.title.lowercased() })?.id { return (.command(id), .command) }
        if !c.key.isEmpty { return (.chord(c.key), .command) }
        // A feature that owns a line with no key answers `click mac-palette.result.<slug>` (e.g. "AI settings").
        let slug = resultSlug(c.title)
        if Registrations.click("mac-palette.result.\(slug)") != nil { return (.resultTarget(slug), .command) }
        return (.none, .command)
    }

    /// A line's key: the feature's own when it gave one, else the live preset's chord for the command (a fixed key such as
    /// Shift+Tab for "Previous split" is in no preset row, so the engine's `chord(for:)` alone answers nothing for it).
    @MainActor
    private static func key(for c: (title: String, key: String), act: PaletteItem.Act) -> String {
        guard c.key.isEmpty, case .command(let id) = act else { return c.key }
        return ShortcutEngine.shared.chord(for: id) ?? Presets.fixed.first { $0.command == id }?.chord ?? ""
    }

    /// "AI settings" -> "ai-settings"; "Outside AI apps (MCP)" -> "outside-ai-apps" (the parenthesis is dropped).
    static func resultSlug(_ title: String) -> String {
        var t = title.lowercased()
        if let open = t.firstIndex(of: "("), let close = t.lastIndex(of: ")"), open < close { t.removeSubrange(open...close) }
        let parts = t.split { !$0.isLetter && !$0.isNumber }
        return parts.joined(separator: "-")
    }

    /// "archive" reads "Archive"; a title already capitalised is left as the lane wrote it.
    static func display(_ t: String) -> String {
        guard let f = t.first, f.isLowercase else { return t }
        return f.uppercased() + t.dropFirst()
    }

    @MainActor
    static func jumps(_ app: AppModel) -> [PaletteItem] {
        accounts(app, excluding: nil).map { a in
            PaletteItem(id: "account:\(a.id)", title: a.name, detail: a.address, key: "", kind: .account, act: .jumpAccount(a.id), emblem: emblem(a))
        } + tabs(app) + labels(app)
    }

    @MainActor
    static func accounts(_ app: AppModel, excluding id: String?) -> [AccountBook.Entry] {
        AccountBook.shared.accounts(core: app.core).filter { $0.id != id }
    }

    static func emblem(_ a: AccountBook.Entry) -> PaletteEmblem {
        PaletteEmblem(name: a.name, address: a.address, colour: a.colour, iconId: a.iconId)
    }

    /// The split tabs as the core serves them (`counts()` lists the ids, `split_tab/<id>/name` the names).
    @MainActor
    private static func tabs(_ app: AppModel) -> [PaletteItem] {
        app.core.counts().tabs.map { t in
            let raw = app.core.state(path: "split_tab/\(t.tabId)/name")?.trimmingCharacters(in: CharacterSet(charactersIn: "\""))
            let name = (raw?.isEmpty == false && raw != "null") ? raw! : t.tabId
            return PaletteItem(id: "tab:\(t.tabId)", title: name, detail: PaletteMacWords.tabKind, key: "", kind: .tab, act: .jumpTab(t.tabId))
        }
    }

    /// Every label at any depth, shown as its path: "Work › Client › Provanta".
    @MainActor
    private static func labels(_ app: AppModel) -> [PaletteItem] {
        app.core.labels().map { l in
            PaletteItem(id: "label:\(l.id)", title: l.path.replacingOccurrences(of: " > ", with: " › "), detail: PaletteMacWords.labelKind,
                        key: "", kind: .label, act: .jumpLabel(l.id))
        }
    }
}
