import Foundation
#if os(macOS)
import AppKit
#endif

/// What the Settings window registers for the control socket: its rows (`rows mac-settings-window.*`), its clicks and its
/// keys. The window has no feature module of its own (the registry is frozen), so SettingsGeneralMacFeature merges these in.
/// Every control a pane draws is listed in `mac-settings-window.controls` with its pane, so a check can click each one.
@MainActor
enum SettingsWindowRegistry {
    static let W = "mac-settings-window"
    private static var model: SettingsWindowModel { .shared }

    // MARK: rows

    static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        let accounts = AccountsSession.shared
        return [
            "\(W).panes": { _ in
                SettingsPane.allCases.map { p in
                    ["ax": "\(W).pane-row.\(p.id)", "id": p.id, "title": p.title, "group": p.group.title, "key": p.chord,
                     "current": p == model.pane, "symbol": p.symbol, "badge": p == .accounts ? accounts.failing.count : 0,
                     "open": p == .replies ? model.repliesOpen : NSNull()]
                }
            },
            // The six parts of Réponses listed under their parent in the sidebar (layout L2), and the part that shows.
            "\(W).replies.parts": { _ in
                RepliesPage.panes.map { p in
                    ["ax": "\(W).pane-row.replies.\(p.rawValue)", "id": p.rawValue, "title": p.title,
                     "current": model.place.pane == .replies && (model.place.reply == p || model.place.reply?.parent == p)]
                }
            },
            "\(W).chrome": { _ in [["titlebar_h": Double(model.titlebarHeight)]] },
            "\(W).groups": { _ in SettingsPane.Group.allCases.map { ["id": $0.rawValue, "title": $0.title, "panes": $0.panes.map(\.id)] } },
            "\(W).place": { _ in
                var title = model.pane.title
                if let id = model.accountPage, let a = accounts.entry(id) { title += " › " + a.name }
                if model.pane == .replies { title = model.place.reply?.path ?? RepliesPage.auto.title }   // the toolbar's words
                return [["pane": model.pane.id, "reply": model.place.reply?.rawValue ?? NSNull(), "account": model.accountPage ?? NSNull(), "title": title, "line": model.pane.line,
                         "can_back": model.canGoBack, "can_forward": model.canGoForward, "query": model.query,
                         "ring": model.ring ?? NSNull(), "ai_section": String(describing: AISettingsStore.shared.section), "adding": accounts.adding,
                         "slot": (model.pane.slot ?? AppModel.shared.settingsPage).id]]
            },
            "\(W).search": { _ in
                model.results.enumerated().map { i, e in
                    ["ax": "\(W).result.\(i)", "title": e.title, "path": e.path, "pane": e.pane.id, "row": e.ax, "selected": i == model.selectedResult]
                }
            },
            "\(W).search.index": { _ in SettingsSearch.all().map { ["row": $0.ax, "pane": $0.pane.id, "title": $0.title, "path": $0.path] } },
            "\(W).summary": { _ in
                [["accounts": accounts.accounts.count, "needs_you": accounts.failing.count,
                  "marks": Array(accounts.accounts.prefix(5).map(\.name)), "more": max(0, accounts.accounts.count - 5)]]
            },
            "\(W).controls": { _ in controls() },
            "\(W).rowfit": { _ in rowFit() },
            "\(W).footprint": { _ in [["views_alive": SettingsLifeProbe.alive]] },
        ]
    }

    /// What each row of the current pane laid out: the width its label column got, whether the control went under it, and the width of the
    /// longest word of its title and caption at their type sizes. A label narrower than its longest word is a cut word.
    static func rowFit() -> [ControlRow] {
        #if os(macOS)
        func longest(_ text: String?, _ font: NSFont) -> CGFloat {
            (text ?? "").split(whereSeparator: { $0 == " " || $0 == "\n" }).map { ($0 as NSString).size(withAttributes: [.font: font]).width }.max() ?? 0
        }
        return FormFitLog.shared.entries.sorted { $0.key < $1.key }.map { id, e in
            let header = id.hasPrefix("\(W).header.")
            let titleFont = NSFont.systemFont(ofSize: header ? WhooshTypography.title3.size : WhooshTypography.body.size, weight: header ? .semibold : .regular)
            let words = max(longest(e.title, titleFont), longest(e.caption, NSFont.systemFont(ofSize: WhooshTypography.subhead.size)))
            return ["id": id, "title": e.title, "label_width": Double(e.labelWidth), "longest_word": Double(words.rounded(.up)), "stacked": e.stacked]
        }
        #else
        return []
        #endif
    }

    /// Every clickable control of every pane: the check clicks them all and compares with this list.
    static func controls() -> [ControlRow] {
        let G = "mac-settings-general", A = "mac-settings-accounts", D = "mac-settings-ai", M = "mac-mcp-notes"
        var out: [ControlRow] = []
        func add(_ pane: String, _ ax: String, _ kind: String) { out.append(["pane": pane, "ax": ax, "kind": kind]) }
        for p in SettingsPane.allCases { add("window", "\(W).pane-row.\(p.id)", "pane") }
        for p in RepliesPage.panes { add("window", "\(W).pane-row.replies.\(p.rawValue)", "pane") }
        add("window", "\(W).pane-row.replies.chevron", "pane")
        add("window", "\(W).back", "button"); add("window", "\(W).forward", "button"); add("window", "\(W).summary", "button")
        add("window", "\(W).search.clear", "button")
        for o in GeneralLanguagePopup.options { add("general", "\(G).appearance.language.\(o.code)", "choice") }
        for a in GeneralStore.Appearance.allCases { add("general", "\(G).appearance.theme.\(a.rawValue)", "choice") }
        for d in GeneralStore.Density.allCases { add("general", "\(G).appearance.density.\(d.rawValue)", "choice") }
        for m in GeneralStore.MotionMode.allCases { add("general", "\(G).appearance.motion.\(m.rawValue)", "choice") }
        for n in GeneralStore.undoChoices { add("general", "\(G).appearance.undo-send.\(n)", "choice") }
        add("general", "\(G).appearance.emblems", "switch")
        add("general", "\(G).privacy.pictures", "switch")
        for l in AppModel.shared.core.labels() { add("general", "\(G).pinned.label.\(l.id)", "switch") }
        for b in GeneralStore.allButtons { add("notifications", "\(G).notch.buttons.\(b)", "check") }
        for k in ["show", "focus-filter", "hidden"] { add("notifications", "\(G).notch.\(k)", "switch") }
        for a in AccountBook.shared.accounts() { add("notifications", "\(G).notifications.account.\(a.id)", "switch") }
        for p in ShortcutEngine.shared.allPresets { add("shortcuts", "\(G).shortcuts.preset.\(p.id)", "choice") }
        add("shortcuts", "\(G).shortcuts.hints", "switch")
        add("accounts", "\(A).add", "button"); add("accounts", "\(A).move-to-iphone", "button")
        let session = AccountsSession.shared
        for a in session.accounts {
            add("accounts", "\(A).\(a.id)", "row")
            add("account-page", "\(A).\(a.id).pause", "switch"); add("account-page", "\(A).\(a.id).notifications", "switch")
            add("account-page", "\(A).\(a.id).icon", "button"); add("account-page", "\(A).\(a.id).remove", "button")
        }
        for a in session.failing { add("accounts", "\(A).\(a.id).sign-in-again", "button") }
        for e in AISettingsStore.Engine.allCases { add("decisions", "\(D).decisions.engine.\(e.rawValue)", "segment") }
        for p in AISettingsStore.shared.pages { add("decisions", "\(D).decisions.open.\(p.id)", "row") }
        add("decisions", "\(D).decisions.jev.connect", "button")
        add("ai", "\(D).speech.local", "button"); add("ai", "\(D).agents.new", "button")
        for p in AISettingsStore.shared.providers { add("ai", "\(D).provider.\(p.id).connect", "button") }
        let mcp = McpSession.shared
        mcp.syncFromCore()   // the core is the truth for the server state: a Start control exists only while the core says off
        if !mcp.serverOn { add("outside", "\(M).server.start", "button") }
        for app in mcp.apps { add("outside", "\(M).allow.\(app.slug)", "button"); add("outside", "\(M).revoke.\(app.slug)", "button") }
        for r in mcp.requests { add("outside", "\(M).request.\(r.id).allow", "button"); add("outside", "\(M).request.\(r.id).refuse", "button") }
        return out
    }

    // MARK: clicks

    static func clickTargets() -> [String: (AppModel) -> Bool] {
        var out: [String: (AppModel) -> Bool] = [
            "\(W).back": { _ in model.goBack() },
            "\(W).forward": { _ in model.goForward() },
            "\(W).summary": { _ in model.select(.accounts) || model.pane == .accounts },
            "\(W).search.clear": { _ in guard model.searching else { return false }; model.clearQuery(); return true },
            // The search field's three keys as doors (the arrow keys and Return reach the field itself; the socket has no name for the arrows): same model calls.
            "\(W).search.down": { _ in guard !model.results.isEmpty else { return false }; model.moveSelection(1); return true },
            "\(W).search.up": { _ in guard !model.results.isEmpty else { return false }; model.moveSelection(-1); return true },
            "\(W).search.commit": { _ in guard !model.results.isEmpty else { return false }; model.commitSearch(); return true },
            "mac-settings-accounts.add": { _ in AccountsSession.shared.adding = true; return true },
        ]
        for p in SettingsPane.allCases { out["\(W).pane-row.\(p.id)"] = { _ in _ = model.select(p); return true } }
        out["\(W).pane-row.replies.chevron"] = { _ in model.toggleReplies(); return true }
        for p in RepliesPage.panes { out["\(W).pane-row.replies.\(p.rawValue)"] = { _ in _ = model.openReply(p); return true } }
        for i in 0..<12 { out["\(W).result.\(i)"] = { _ in
            let r = model.results; guard i < r.count else { return false }
            model.open(r[i]); return true
        } }
        // The test seams for the search field: the `type` verb reaches it after ⌘F; these set the words straight.
        for word in ["emblem", "undo", "jev", "show", "qqqq"] { out["\(W).fixture.query.\(word)"] = { _ in model.setQuery(word); return true } }
        #if os(macOS)
        // Open, and the two sizes the window is shot at (the shell's `resize` moves the main window only).
        out["\(W).open"] = { m in WindowBridge.open(model.pane.slot ?? .macSettingsGeneral, in: m) }
        out["\(W).size.default"] = { m in resize(m, to: SettingsMetrics.windowDefault) }
        out["\(W).size.min"] = { m in resize(m, to: SettingsMetrics.windowMin) }
        out["\(W).close"] = { m in
            guard let w = WindowBridge.window(for: model.pane.slot ?? .macSettingsGeneral) else { return false }
            w.close(); m.surface = SurfaceSlot.macMain.id; return true
        }
        #endif
        let s = AccountsSession.shared
        for a in s.accounts {
            out["mac-settings-accounts.\(a.id)"] = { _ in model.openAccount(a.id) }
            out["mac-settings-accounts.\(a.id).pause"] = { _ in s.togglePause(a.id) }
            out["mac-settings-accounts.\(a.id).remove"] = { _ in s.askRemove(a.id) }
            for (key, d) in [("notch", NotificationDelivery.notch), ("notifications", .notifications), ("both", .both)] {
                out["mac-settings-accounts.\(a.id).delivery.\(key)"] = { _ in s.setDelivery(d, for: a.id) }
            }
            out["mac-settings-accounts.\(a.id).fixture.signature"] = { _ in s.setSignature(a.id, to: Words.text("Best regards")) }
            out["mac-settings-accounts.\(a.id).fixture.rename"] = { _ in s.rename(a.id, to: Words.text("Renamed")) }
        }
        // The find field: `type` reaches the search field only; these set the accounts query straight (test seam).
        for word in ["provanta", "work", "zzzz", ""] { out["mac-settings-accounts.fixture.find.\(word)"] = { _ in s.query = word; return true } }
        out["mac-settings-accounts.remove.confirm"] = { _ in s.confirmRemove() }
        out["mac-settings-accounts.remove.cancel"] = { _ in guard s.removing != nil else { return false }; s.removing = nil; return true }
        return out
    }

    #if os(macOS)
    /// Sets the content size of the Settings window and lays it out, so the next shot and measure see it.
    private static func resize(_ m: AppModel, to size: CGSize) -> Bool {
        guard WindowBridge.open(model.pane.slot ?? .macSettingsGeneral, in: m),
              let w = WindowBridge.window(for: model.pane.slot ?? .macSettingsGeneral) else { return false }
        w.setFrame(NSRect(x: w.frame.minX, y: w.frame.maxY - size.height, width: size.width, height: size.height), display: true)
        w.contentView?.layoutSubtreeIfNeeded()
        return true
    }
    #endif

    // MARK: keys

    /// ⌘1 to ⌘0, ⌘F, ⌘[, ⌘], ⌘N: each answers only while the Settings window holds the focus.
    static func keyBindings() -> [String: (AppModel) -> Bool] {
        func inSettings(_ m: AppModel) -> Bool { m.surface.hasPrefix("mac-settings") }
        var out: [String: (AppModel) -> Bool] = [
            "⌘F": { m in guard inSettings(m) else { return false }; model.requestSearch(); return true },
            "⌘[": { m in inSettings(m) && model.goBack() },
            "⌘]": { m in inSettings(m) && model.goForward() },
            "⌘N": { m in
                if inSettings(m), model.pane == .replies, SettingsRepliesMacFeature.keyBindings()["⌘N"]?(m) == true { return true }   // hid-ai: Réponses › Sources adds a source
                guard inSettings(m), model.pane == .accounts, model.accountPage == nil else { return false }
                AccountsSession.shared.adding = true; return true
            },
        ]
        #if os(macOS)
        // ⌘, from anywhere: the Settings window comes forward on its current pane. The app menu's "Settings…" (ShellScenes.swift SettingsCommand)
        // is the same open for a real key; the engine's key verb has no menu, so without this binding `key ⌘,` did nothing (main88 par-keys).
        out["⌘,"] = { m in WindowBridge.open(model.pane.slot ?? .macSettingsGeneral, in: m) }
        #endif
        out["Escape"] = { m in
            guard inSettings(m) else { return false }
            // The open pane answers first (Réponses: close a popover, leave an editor); the window's own Escape only when the pane does not take it.
            if model.pane == .replies, SettingsRepliesMacFeature.keyBindings()["Escape"]?(m) == true { return true }
            guard model.searching else { return false }
            model.clearQuery(); return true
        }
        for p in SettingsPane.allCases where !p.chord.isEmpty { out[p.chord] = { m in guard inSettings(m) else { return false }; _ = model.select(p); return true } }
        return out
    }
}
