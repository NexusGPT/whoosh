import Foundation

/// A text prompt the menu or the "+" opens: new label, rename, new smart folder.
@MainActor struct SidebarPrompt {
    enum Kind { case newLabel(parent: String?), renameLabel(id: String), renameSmartFolder(id: String), newSmartFolder }
    let kind: Kind
    var title: String {
        switch kind {
        case .newLabel(let p): return Words.text(p == nil ? "New label" : "New label inside")
        case .renameLabel: return Words.text("Rename label")
        case .renameSmartFolder: return Words.text("Rename smart folder")
        case .newSmartFolder: return Words.text("New smart folder")
        }
    }
    var confirmTitle: String { switch kind { case .renameLabel, .renameSmartFolder: Words.text("Rename"); default: Words.text("Create") } }
    var hasRule: Bool { if case .newSmartFolder = kind { return true }; return false }
    var name: String {
        switch kind {
        case .newLabel(let p): return p.flatMap { id in SidebarData.labels().first { $0.id == id }?.label.name } ?? "top level"
        case .renameLabel(let id): return SidebarData.labels().first { $0.id == id }?.label.name ?? ""
        case .renameSmartFolder(let id): return SidebarData.smartFolders().first { $0.id == id }?.name ?? ""
        case .newSmartFolder: return ""
        }
    }
}

/// A destructive item waiting for the owner's yes.
@MainActor struct SidebarDeleteAsk {
    enum Kind { case label(String), smart(String) }
    let kind: Kind
    var name: String {
        switch kind {
        case .label(let id): return SidebarData.labels().first { $0.id == id }?.label.name ?? id
        case .smart(let id): return SidebarData.smartFolders().first { $0.id == id }?.name ?? id
        }
    }
    var noun: String { if case .label = kind { return Words.text("label") }; return Words.text("smart folder") }
}

extension SidebarState {
    @discardableResult func openPrompt(_ kind: SidebarPrompt.Kind) -> Bool {
        let p = SidebarPrompt(kind: kind)
        promptText = { switch kind { case .renameLabel, .renameSmartFolder: return p.name; default: return "" } }()
        promptRule = ""
        prompt = p
        return true
    }

    func cancelPrompt() { prompt = nil; promptText = ""; promptRule = "" }

    /// Runs the prompt's core command. `typed` is the socket's `type` text when the field was not edited on screen.
    @discardableResult func confirmPrompt(model: AppModel, typed: String = "") -> Bool {
        guard let p = prompt else { return false }
        let text = (typed.isEmpty ? promptText : typed).trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return false }
        let ok: Bool
        switch p.kind {
        case .newLabel(let parent):
            ok = perform(.createLabel(name: text, parentId: parent), model: model)
            if ok, let parent { openLabels.insert(parent) }
        case .renameLabel(let id): ok = perform(.renameLabel(labelId: id, name: text), model: model)
        case .renameSmartFolder(let id): ok = model.perform(.renameSmartFolder(folderId: id, name: text))
        case .newSmartFolder: ok = model.perform(.saveSmartFolder(name: text, ruleWords: promptRule))
        }
        if ok { revision += 1; cancelPrompt() }
        return ok
    }

    @discardableResult func askDelete(_ kind: SidebarDeleteAsk.Kind) -> Bool { deleteAsk = SidebarDeleteAsk(kind: kind); return true }
    func cancelDelete() { deleteAsk = nil }

    /// The owner said yes: the core deletes (a label with the labels under it, the messages stay; a smart folder, its children move up).
    /// A refusal reaches the list's failure line in the core's words. A deleted row that was selected hands the highlight back to All inboxes.
    @discardableResult func confirmDelete(model: AppModel) -> Bool {
        guard let ask = deleteAsk else { return false }
        let ok: Bool, gone: String
        switch ask.kind {
        case .label(let id): ok = perform(.deleteLabel(labelId: id), model: model); gone = "label.\(id)"
        case .smart(let id): ok = model.perform(.deleteSmartFolder(folderId: id)); gone = "smart.\(id)"
        }
        deleteAsk = nil
        if ok { revision += 1; if selection.id == gone { select("mailbox.all-inboxes") } }
        return ok
    }

    /// Marks every unread message read: All inboxes = every tab of the strip; a label = its messages in every tab. The list read is
    /// the core's (`inbox` per tab), the write is `markRead`, so the account filter narrows it like the list. The tabs are the strip's
    /// (`tabIds`: the Primary and All views, the pinned labels, the rule tabs; delta 16), not `splitTabs()` alone: main52 left 8 rows
    /// unread because the views and the labels are not split tabs.
    @discardableResult func markAllRead(label: String?, model: AppModel) -> Bool {
        var ids: [String] = []
        var tabs = model.tabIds
        for t in model.core.splitTabs().map(\.id) where !tabs.contains(t) { tabs.append(t) }
        for tab in tabs {
            for r in model.core.inbox(query: InboxQuery(tabId: tab, accountId: model.accountFilter, labelId: label)) where r.unread && !ids.contains(r.messageId) {
                ids.append(r.messageId)
            }
        }
        // A built-in label is a tab (Needs Reply, Updates...): its unread is the tab's, and no message is filed under it, so the label read above finds none.
        if let label, tabs.contains(label) {
            for r in model.core.inbox(query: InboxQuery(tabId: label, accountId: model.accountFilter, labelId: nil)) where r.unread && !ids.contains(r.messageId) {
                ids.append(r.messageId)
            }
        }
        guard !ids.isEmpty else { return false }
        let ok = model.perform(.markRead(messageIds: ids, read: true))
        if ok { revision += 1 }
        return ok
    }
}
