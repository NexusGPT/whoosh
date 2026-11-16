import SwiftUI

/// Registration for the settingsreplies-mac lane: Settings › Réponses on the Mac (six panes and the page « Quels messages »). Its rows read
/// what the panes draw; its clicks are the panes' own setters (RepliesDoors); `mac-settings-replies.controls` lists every control with the
/// page it lives on, so a check clicks each one. FeatureRegistry is frozen: one line adds this type (docs/lanes/settingsreplies-mac.md F1).
public enum SettingsRepliesMacFeature: FeatureModule {
    public static let id = "settingsreplies-mac"
    public static let surfaces: [String] = [RepliesAx.P]
    public static let slot = SurfaceSlot(RepliesAx.P)

    public static func view(for slot: SurfaceSlot) -> AnyView? {
        slot == Self.slot ? AnyView(RepliesPaneSwitch(page: RepliesRouter.current())) : nil
    }

    /// hid-ai round 18: at launch (AppLaunch.afterFirstFrame) and on every language change, the untouched shipped defaults the model reads take the
    /// app's language (RepliesStore.followLanguage), so a draft written before Settings is ever opened already reads them in that language.
    @MainActor private static var languageToken: NSObjectProtocol?
    @MainActor public static func start() {
        guard languageToken == nil else { return }
        RepliesStore.shared.followLanguage()
        languageToken = NotificationCenter.default.addObserver(forName: Words.languageDidChange, object: nil, queue: .main) { _ in
            MainActor.assumeIsolated { RepliesStore.shared.followLanguage() }
        }
    }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        let s = RepliesStore.shared
        let P = RepliesAx.P
        return [
            "\(P).typed": { _ in RepliesDoors.typedRow() },
            "\(P).place": { _ in
                let p = RepliesRouter.current()
                return [["page": p.rawValue, "title": p.title, "path": p.path, "line": p.line, "parent": p.parent?.rawValue ?? NSNull(),
                         "panes": RepliesPage.panes.map(\.rawValue)]]
            },
            "\(P).state": { _ in s.load(); return [RepliesRows.state(s)] },
            "\(P).sources": { _ in s.load(); return RepliesRows.sources(s) },
            "\(P).labels": { _ in s.load(); return RepliesRows.labels(s) },
            "\(P).controls": { _ in s.load(); return RepliesDoors.controls(s).map { ["ax": $0.ax, "page": $0.page, "kind": $0.kind] } },
            "\(P).error": { _ in s.error.map { [["text": $0, "refusals": s.refusals]] } ?? [] },
            "\(P).editors": { _ in RepliesEditorDoors.rows(s) },
        ]
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        let s = RepliesStore.shared
        var out: [String: (AppModel) -> Bool] = [:]
        for d in RepliesDoors.controls(s) { let act = d.act; out[d.ax] = { _ in act() } }
        out.merge(RepliesDoors.typedClickTargets(s)) { a, _ in a }
        // the check's seam, after `fixture reset`: the panes' own state (popover, editor, picks, the last trial) goes back to its start
        out["\(RepliesAx.P).fixture.reset"] = { _ in s.resetForTest(); return true }
        // the system's own Cancel of the folder chooser (a seam, not a control of the pane: on a hidden rig the chooser never opens)
        out["\(RepliesAx.P).sources.picker.cancel"] = { _ in RepliesFolderPicker.cancel(); return true }
        RepliesGoneSeam.add(&out, s)
        RepliesEditorDoors.seams(s, &out)
        // a relaunch of the pane's model: every bit of pane state dropped, everything read again from the core (delta 24's case reads the choice after it)
        out["\(RepliesAx.P).fixture.relaunch"] = { _ in s.resetForTest(); return true }
        return out
    }

    /// `type` after a `.typed.focus` door: the words go to the popover field's buffer.
    public static func typeText(_ text: String, _ m: AppModel) -> Bool { RepliesDoors.typeInto(text) }

    /// The editors' keys while one holds the focus (RepliesEditorKeys); else Esc closes an open popover.
    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        let s = RepliesStore.shared
        var out = RepliesEditorKeys.bindings()
        let editorEsc = out["Escape"]
        out["Escape"] = { m in
            if editorEsc?(m) == true { return true }
            guard m.surface.hasPrefix("mac-settings"), s.popover != nil else { return false }
            s.popover = nil; return true
        }
        // ⌘N on the Sources page: Add a source (the button's own key, SourcesPane.swift `.keyboardShortcut("n")`). The key verb and the shell's
        // monitor reach the window's bindings, not the button's shortcut (main92 sweep-settingsreplies sources-add-command-n: no folder chooser asked for).
        // Round 12: the page is the one the window shows (RepliesRouter.current: the Settings window routes pages through its own place once it is
        // drawn, SettingsWindowView.swift:63-64), never the store's `page`, which stays where it was (main103: route monitor+keyEquivalent, seen 1,
        // handled, no chooser: the binding read `.auto` and passed the key on to the menu).
        out["⌘N"] = { m in
            guard m.surface.hasPrefix("mac-settings"), RepliesRouter.current() == .sources else { return false }
            RepliesFolderPicker.choose(); return true
        }
        return out
    }

    /// Asked first while a Replies editor holds the keys (its text view is its window's first responder), or while a Réponses part shows.
    public static func hasFocus(_ m: AppModel) -> Bool {
        #if os(macOS)
        if RepliesEditorViews.focused != nil { return true }
        #endif
        return m.surface == RepliesAx.P
    }
    public static func paletteCommands() -> [(title: String, key: String)] { [] }
}
