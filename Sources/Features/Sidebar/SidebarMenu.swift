import SwiftUI

/// What a right-click menu is about: one row or one section header.
enum SidebarSubject {
    case mailbox(String), smart(String), label(String), labelsHeader, smartHeader
    /// The id the socket and the rows use: "mailbox.<slug>", "smart.<id>", "label.<id>", "header.labels", "header.smart".
    var key: String {
        switch self {
        case .mailbox(let s): return "mailbox.\(s)"
        case .smart(let id): return "smart.\(id)"
        case .label(let id): return "label.\(id)"
        case .labelsHeader: return "header.labels"
        case .smartHeader: return "header.smart"
        }
    }
}

/// One menu item. `needs` names the core door the item would drive when none is served yet: the item is drawn disabled
/// and its help says what is missing (docs/lanes/sidebar.md "Round 5"); nothing is faked.
struct SidebarMenuItem: Identifiable {
    let id: String
    let title: String
    var destructive = false
    var needs: String? = nil
    let run: (AppModel) -> Bool
    var enabled: Bool { needs == nil }
}

extension SidebarState {
    /// The items for a subject, in the order the owner's list gives (2026-11-08 09:10 and the labels note).
    func menu(_ subject: SidebarSubject) -> [SidebarMenuItem] {
        switch subject {
        case .mailbox(let slug): return [
            SidebarMenuItem(id: "mark-read", title: Words.text("Mark all as read"),
                            needs: slug == "all-inboxes" ? nil : "a mailbox read (InboxQuery has no mailbox field, only tab, account and label)",
                            run: { m in self.markAllRead(label: nil, model: m) }),
            SidebarMenuItem(id: "hide", title: Words.text("Hide"), run: { _ in self.hidden.insert(slug).inserted }),
            SidebarMenuItem(id: "account-info", title: Words.text("Account info"), run: { _ in self.setAccountsMenu(true) }),
        ] + showHiddenItems()
        case .smart(let id): return [
            SidebarMenuItem(id: "edit-rule", title: Words.text("Edit rule…"),
                            needs: "updateSmartFolder{id, ruleWords} (saveSmartFolder refuses an existing name)", run: { _ in false }),
            SidebarMenuItem(id: "rename", title: Words.text("Rename"), run: { _ in self.openPrompt(.renameSmartFolder(id: id)) }),
            SidebarMenuItem(id: "delete", title: Words.text("Delete…"), destructive: true, run: { _ in self.askDelete(.smart(id)) }),
        ]
        case .label(let id): return [
            SidebarMenuItem(id: "new-inside", title: Words.text("New label inside…"), run: { _ in self.openLabels.insert(id); return LabelInspector.openNew(parent: id) }),
            SidebarMenuItem(id: "modify", title: Words.text("Modify label…"), run: { _ in LabelInspector.openModify(id) }),
            SidebarMenuItem(id: "rename", title: Words.text("Rename"), run: { _ in self.openPrompt(.renameLabel(id: id)) }),
            SidebarMenuItem(id: "colour", title: Words.text("Change colour"), needs: "setLabelColour{labelId, colour}", run: { _ in false }),
            SidebarMenuItem(id: SidebarData.labels().first { $0.id == id }?.label.pinned == true ? "unpin" : "pin",
                            title: Words.text(SidebarData.labels().first { $0.id == id }?.label.pinned == true ? "Unpin tab" : "Pin as a tab"),
                            run: { m in self.togglePin(id, model: m) }),
            SidebarMenuItem(id: "mark-read", title: Words.text("Mark all as read"), run: { m in self.markAllRead(label: id, model: m) }),
            SidebarMenuItem(id: "delete", title: Words.text("Delete…"), destructive: true, run: { _ in self.askDelete(.label(id)) }),
        ]
        case .labelsHeader: return [SidebarMenuItem(id: "new", title: Words.text("New label…"), run: { _ in LabelInspector.openNew(parent: nil) })] + showHiddenItems()
        case .smartHeader: return [SidebarMenuItem(id: "new", title: Words.text("New smart folder…"), run: { _ in self.openPrompt(.newSmartFolder) })] + showHiddenItems()
        }
    }

    /// F-A1: the way back from a right-click Hide. One item, drawn only while something is hidden, on the mailbox menus and on both
    /// section headers (a header stays drawn when every mailbox is hidden, so the way back never disappears with the rows).
    func showHiddenItems() -> [SidebarMenuItem] {
        hidden.isEmpty ? [] : [SidebarMenuItem(id: "show-hidden", title: Words.text("Show hidden mailboxes"), run: { _ in self.showHidden() })]
    }

    @discardableResult func showHidden() -> Bool {
        guard !hidden.isEmpty else { return false }
        hidden = []
        return true
    }

    /// Every subject the sidebar shows right now. The built-in views (Starred, Muted, All mail) draw the same right-click menu as the
    /// mailboxes (SidebarView.mailbox), so they are subjects too: their items answer the socket like the others (doors-nav).
    func allSubjects() -> [SidebarSubject] {
        (Self.mailboxes + Self.views).map { .mailbox(Self.slug($0.name)) } + SidebarData.smartFolders().map { .smart($0.id) }
            + SidebarData.labels().map { .label($0.id) } + [.labelsHeader, .smartHeader]
    }
}

/// The native context menu content for a subject.
struct SidebarMenuContent: View {
    let subject: SidebarSubject
    let model: AppModel
    let state: SidebarState
    var body: some View {
        ForEach(state.menu(subject)) { item in
            if item.destructive { Divider() }
            Button(role: item.destructive ? .destructive : nil) { _ = item.run(model) } label: { Text(item.title) }
                .disabled(!item.enabled)
                .help(item.needs.map { Words.text("Needs: %@", $0) } ?? "")
                .accessibilityIdentifier("mac-main.sidebar.menu.\(subject.key).\(item.id)")
        }
    }
}
