import Foundation

/// The ⌘J key (KeyPreset "accounts"; it was A until A became reply all): pulls the accounts menu up, or down when it is up.
extension SidebarFeature: CommandProviding {
    public static func commands() -> [String: (AppModel) -> Bool] {
        let st = SidebarState.shared
        var c: [String: (AppModel) -> Bool] = ["accounts": { _ in st.setAccountsMenu(!st.accountsMenuOpen) },
                                                "sidebar": { _ in SidebarVisibility.toggle() }]
        // Go to: every mailbox row, the built-in views included. G then L opens the Labels section on its first label.
        for (command, slug) in goMailboxes {
            c[command] = { m in
                guard st.select("mailbox.\(slug)") else { return false }
                if slug == "all-inboxes" { st.clearAccounts(model: m) }
                return true
            }
        }
        // Round 11: G then I opens the Primary VIEW (the inbox on its Primary tab), and the All view has its own command. Neither is a
        // label row: they are what the tab strip's first two tabs are (docs/PRD.md "A91 shape").
        for g in SidebarState.goViews { c[g.command] = { m in st.openView(g.view, model: m) } }
        // The arrows in the sidebar pane (lead ruling, round 7): Right on a folded label opens it, Left on an open label folds it;
        // anything else returns false and the arrow moves the pane focus (MacShell's focus-left / focus-right, asked next).
        c["focus-right"] = { m in hasFocus(m) && st.labelRight() }
        c["focus-left"] = { m in hasFocus(m) && st.labelLeft() }
        c["go-label"] = { m in
            if st.isCollapsed("labels") { _ = st.toggleSection("labels") }
            guard let first = SidebarData.labels(core: m.core).first(where: { st.labelVisible($0, in: SidebarData.labels(core: m.core)) }) else { return false }
            st.focusLabel(first.id)
            return true
        }
        return c
    }
}

extension SidebarFeature {
    /// Command id to the mailbox row it selects.
    static let goMailboxes: [(String, String)] = [
        ("go-inbox", "all-inboxes"), ("go-drafts", "drafts"), ("go-sent", "sent"), ("go-done", "archive"),
        ("go-reminders", "snoozed"), ("go-spam", "junk"), ("go-trash", "trash"),
        // the contract's built-in views (delta 10, Mailbox.starred / .muted / .allMail)
        ("go-starred", "starred"), ("go-muted", "muted"), ("go-all-mail", "all-mail"),
    ]
}

/// Round 2's socket surface: the accounts menu, the selection motion (rule 11) and the label tree's keys.
/// The same SidebarState functions back the real controls, so a driven check exercises what a click or a key does.
extension SidebarFeature {
    static func round2Rows(_ st: SidebarState) -> [String: (AppModel) throws -> [ControlRow]] {
        [
            "mac-main.sidebar.accounts-menu": { _ in
                [["open": st.accountsMenuOpen, "fade_below": st.menuMoreBelow, "foot_px": Int(SidebarAccountsMenu.foot), "pull_ms": WhooshConstants.ACCOUNTS_MENU_MS, "tooltip": "\(Words.text(st.accountsMenuOpen ? "Pull the accounts down" : "Pull the accounts up"))  \(SidebarAccountsBar.chord)"]]
            },
            "mac-main.sidebar.selection": { _ in
                [["id": st.selection.id, "motion": st.selection.motion.rawValue, "distance": st.selection.distance,
                  "epoch": st.selection.epoch, "slide_max_rows": WhooshConstants.SLIDE_MAX_ROWS]]
            },
            "mac-main.sidebar.labels": { m in
                let all = SidebarData.labels(core: m.core)
                return all.map { l in
                    let open = st.openLabels.contains(l.id)
                    return ["ax": "mac-main.sidebar.label.\(l.id)", "id": l.id, "name": SidebarData.title(l.label), "raw_name": l.label.name, "path": l.label.path, "parent": l.label.parentId ?? "", "depth": l.depth,
                            "visible": st.labelVisible(l, in: all), "has_children": l.hasChildren, "open": open, "own": l.own, "total": l.total,
                            "count": l.hasChildren && !open ? l.total : l.own, "drawn": st.drawnLabelCounts[l.id] ?? -1, "focus": st.labelFocus == l.id,
                            "pinned": l.label.pinned, "scope": Self.scopeWord(l.label.scope), "mark": Self.scopeAccount(l.label.scope) ?? "", "kind": l.label.kind == .builtIn ? "built-in" : "owner"]
                }
            },
        ]
    }

    static func round2Clicks(_ st: SidebarState) -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [
            "mac-main.sidebar.accounts-menu": { _ in st.setAccountsMenu(!st.accountsMenuOpen) },
            "mac-main.sidebar.accounts-menu.outside": { _ in st.setAccountsMenu(false) },
            /// Opens and stays open (the drive's idempotent opener; ⌘J toggles through the "accounts" command).
            "mac-main.sidebar.accounts-menu.up": { _ in st.setAccountsMenu(true); return true },
        ]
        t["mac-main.sidebar.mailbox.outbox"] = { m in
            guard SidebarState.outboxWaiting(core: m.core) > 0 else { return false }
            return st.select(SidebarState.outboxId)
        }
        for b in SidebarState.mailboxes + SidebarState.views {   // the built-in views click like the mailboxes (round 8)
            let s = SidebarState.slug(b.name)
            t["mac-main.sidebar.mailbox.\(s)"] = { m in
                guard st.select("mailbox.\(s)") else { return false }
                if s == "all-inboxes" { st.clearAccounts(model: m) }
                return true
            }
        }
        for f in SidebarData.smartFolders() { t["mac-main.sidebar.smart.\(f.id)"] = { _ in st.select("smart.\(f.id)") } }
        for l in SidebarData.labels() {
            t["mac-main.sidebar.label.\(l.id)"] = { _ in
                guard st.labelVisible(l) else { return false }
                st.focusLabel(l.id)
                return true
            }
            /// F-P1: a jump names a label, so its parents open first (SidebarLabelsSection.revealLabel). The click door above still refuses a row not drawn.
            t["mac-main.sidebar.label.\(l.id).jump"] = { _ in st.revealLabel(l.id) }
            if l.hasChildren {
                t["mac-main.sidebar.label.\(l.id).open"] = { _ in st.openLabels.insert(l.id); return true }
            }
        }
        return t
    }

    /// Escape pulls the accounts menu down (MacShell's Esc chain). The arrows are commands (focus-left / focus-right, below):
    /// one key path, the preset's.
    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        let st = SidebarState.shared
        return ["Escape": { _ in st.setAccountsMenu(false) }]
    }

    /// The sidebar holds the keys while its pane has the focus on the main window with no overlay up.
    public static func hasFocus(_ m: AppModel) -> Bool {
        m.surface == SurfaceSlot.macMain.id && m.overlay == nil && MacShellState.shared.focusedPane == .sidebar
    }
}

extension SidebarFeature {
    /// "all" for a shared label, "account" for one that belongs to a single account (its id is `mark`).
    static func scopeWord(_ s: LabelScope) -> String { if case .account = s { return "account" }; return "all" }
    static func scopeAccount(_ s: LabelScope) -> String? { if case .account(let id) = s { return id }; return nil }
}
