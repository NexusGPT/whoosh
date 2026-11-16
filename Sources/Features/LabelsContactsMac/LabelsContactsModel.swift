import Foundation
import Observation

/// The three screens of mac-labels-contacts: which one is open, the rule being edited, the card being read.
/// UI state only. Counts come from the core (`ruleMatchCount`) and cards from `contact(address:)`; nothing is stored twice.
@MainActor @Observable
final class LabelsContactsModel {
    static let shared = LabelsContactsModel()

    enum Pane: String, CaseIterable { case labels, folder, contact }

    var pane: Pane = .labels
    var folderId: String?
    var isNewFolder = false
    var draftName = ""
    var draftRule = ""
    var draftParent: String?
    var folderRefusal: String?
    var folders: [SmartFolder] = []
    var folderCounts: [String: Int] = [:]
    var contactAddress: String?
    var card: SenderCard?
    var mergeCandidate: Contact?
    var people: [Contact] = []
    var labels = LabelTree(rows: [])
    var selectedLabelId: String?
    var labelRefusal: String?
    var newLabelName = ""
    var renameText = ""

    let source = LabelsContactsSource.shared
    var app: AppModel { AppModel.shared }

    var rows: [InboxRow] { source.allRows(app.core, tabs: app.tabIds) }
    var editing: SmartFolder? { folders.first { $0.id == folderId } }
    var draftMatches: Int { count(draftRule) }
    func savedCount(_ f: SmartFolder) -> Int { folderCounts[f.id] ?? 0 }
    private func count(_ rule: String) -> Int { Int(app.core.ruleMatchCount(ruleWords: rule)) }

    // MARK: smart folders

    /// Re-reads the folder list and each saved rule's match count from the core.
    func reloadFolders() {
        let list = source.folders(app.core)
        if list != folders { folders = list }
        let counts = Dictionary(uniqueKeysWithValues: list.map { ($0.id, count($0.ruleWords)) })
        if counts != folderCounts { folderCounts = counts }
    }

    /// Opens the rule editor on a folder, its saved rule and parent as the draft.
    func edit(_ id: String) -> Bool {
        reloadFolders()
        guard let f = folders.first(where: { $0.id == id }) else { return false }
        folderId = id; isNewFolder = false; draftName = f.name; draftRule = f.ruleWords; draftParent = parentName(f.parentId)
        folderRefusal = nil; pane = .folder
        return true
    }

    /// Opens the editor on a folder that does not exist yet; Save makes it through `saveSmartFolder`.
    func startNewFolder() -> Bool {
        folderId = nil; isNewFolder = true; draftName = ""; draftRule = ""; draftParent = nil
        folderRefusal = nil; pane = .folder
        return true
    }

    /// What the socket's `type` verb sent lands in the editor as keystrokes would: the name of a new folder first, then the rule.
    func takeTyped(_ m: AppModel) {
        guard pane == .folder, !m.typed.isEmpty else { return }
        if isNewFolder && draftName.isEmpty { draftName = m.typed } else { draftRule += m.typed }
        m.typed = ""
    }

    /// What a saved parent shows as: a label's name (the chip's value, and what the sidebar row publishes), else the name of the smart folder it is, else the id as it is.
    func parentName(_ id: String?) -> String? {
        guard let id else { return nil }
        return labels.item(id)?.name ?? folders.first { $0.id == id }?.name ?? id
    }

    func setParent(_ label: String?) -> Bool { guard pane == .folder else { return false }; draftParent = label; return true }

    /// The core wants a smart folder's ID as a parent. The chip's value is the name it shows, so a name is turned into the id of the smart
    /// folder that carries it; an id stays; a value no folder carries goes to the core as it is, and the core's refusal says why.
    func parentFolderId(_ value: String?) -> String? {
        guard let v = value, !v.isEmpty else { return nil }
        // JOURNEYS J2.6: a smart folder nests under a label. The chip's value is the label's name; the engine takes the label's id.
        if let l = labels.items.first(where: { $0.id == v || ($0.parentId == nil && $0.name == v) }) { return l.id }
        if folders.contains(where: { $0.id == v }) { return v }
        return folders.first { $0.id != folderId && $0.name.caseInsensitiveCompare(v) == .orderedSame }?.id ?? v
    }

    /// Save runs the core's commands: `saveSmartFolder` for a new folder (a name that exists already is refused in the core's words), then
    /// `updateSmartFolder(folderId, ruleWords, parentId)` for the rule and the parent of a saved one (and for the parent of a new one).
    /// A refusal stays on the screen's own line (`folderRefusal`), in the core's words.
    func save() -> Bool {
        let rule = draftRule.trimmingCharacters(in: .whitespaces)
        let parent = parentFolderId(draftParent)
        do {
            if isNewFolder {
                let out = try app.core.run(command: .saveSmartFolder(name: draftName, ruleWords: rule))
                folderRefusal = nil; reloadFolders()
                guard let id = out.createdId else { return true }
                folderId = id; isNewFolder = false      // the folder exists from here on: a refused parent leaves the editor on it
                if parent != nil { _ = try app.core.run(command: .updateSmartFolder(folderId: id, ruleWords: rule, parentId: parent)) }
            } else {
                guard let id = folderId else { return false }
                _ = try app.core.run(command: .updateSmartFolder(folderId: id, ruleWords: rule, parentId: parent))
            }
            draftParent = parentName(parent)
            folderRefusal = nil; reloadFolders()
            return true
        } catch let e as CoreError {
            folderRefusal = Self.words(of: e); return false
        } catch {
            folderRefusal = error.localizedDescription; return false
        }
    }

    /// Esc and Cancel leave the editor and keep the folder as it was saved: a new folder is dropped, a saved one is reverted, and the
    /// editor closes either way (the window comes back to the labels).
    func cancel() -> Bool {
        folderRefusal = nil
        if isNewFolder { draftName = ""; draftRule = ""; draftParent = nil; pane = .labels; return true }
        guard let f = editing else { return false }
        draftRule = f.ruleWords; draftParent = parentName(f.parentId)
        pane = .labels
        return true
    }

    // MARK: contacts

    /// Re-reads the distinct cards of every sender in the rows, so the sidebar lists people and merges show at once.
    func reloadPeople() {
        var seen = Set<String>()
        let list = rows.compactMap { r -> Contact? in
            guard !r.senderAddress.isEmpty, let c = app.core.contact(address: r.senderAddress), seen.insert(c.id).inserted else { return nil }
            return c
        }
        if list.map(\.id) != people.map(\.id) || list.map(\.addresses) != people.map(\.addresses) { people = list }
    }

    /// Reads the open card again from the core: its addresses, its messages newest first, a look-alike to merge.
    func refreshCard() {
        guard let a = contactAddress, let c = app.core.contact(address: a) else { card = nil; mergeCandidate = nil; return }
        let next = SenderCard(id: c.id, name: c.name, addresses: c.addresses, messages: app.core.contactMessages(contactId: c.id))
        if next != card { card = next }
        let domains = Set(c.addresses.compactMap { $0.split(separator: "@").last.map(String.init) })
        // a look-alike: the same sender name, or an address at the same domain
        mergeCandidate = people.first { p in
            p.id != c.id && (p.name.caseInsensitiveCompare(c.name) == .orderedSame
                || p.addresses.contains { a in a.split(separator: "@").last.map(String.init).map(domains.contains) ?? false })
        }
    }

    func open(address: String) -> Bool {
        reloadPeople()
        guard app.core.contact(address: address) != nil else { return false }
        contactAddress = address; refreshCard(); pane = .contact
        return true
    }

    func openSender(ofMessage id: String) -> Bool {
        guard let a = app.core.message(messageId: id)?.fromAddress, !a.isEmpty else { return false }
        return open(address: a)
    }

    /// A click on a message row of the sender card: that thread opens in the main window's reader, the person's own act (as Open in Whoosh
    /// does from the notch). The row's tab is found the way the search opens a hit, so the list shows it focused.
    @discardableResult
    func openThread(ofMessage id: String, threadId: String) -> Bool {
        if let tab = app.tabIds.first(where: { t in app.core.inbox(query: InboxQuery(tabId: t, accountId: nil, labelId: nil)).contains { $0.messageId == id } }) {
            app.tab = tab; app.accountFilter = nil
        }
        app.refresh(); app.focus(messageId: id)
        guard app.perform(.openThread(threadId: threadId)) else { return false }
        #if os(macOS)
        _ = WindowBridge.open(.macMain, in: app)     // bringing the window forward names the surface mac-main: the reader's name goes on after it
        #endif
        app.surface = "mac-reader"
        return true
    }

    /// Folds the look-alike card into the open one through `mergeContacts`: one card is left.
    func mergeSuggested() -> Bool {
        guard let keep = card?.id, let other = mergeCandidate?.id else { return false }
        do { _ = try app.core.run(command: .mergeContacts(keepId: keep, mergeId: other)) } catch { return false }
        reloadPeople(); refreshCard()
        return true
    }

    /// Per-account message counts of one label, so the tree reads across accounts.
    func accountsCarrying(_ labelId: String) -> [(id: String, count: Int)] {
        let leaf = labels.item(labelId)?.name ?? labelId
        let hits = rows.filter { $0.labelNames.contains(leaf) }
        return Dictionary(grouping: hits, by: \.accountId).map { ($0.key, $0.value.count) }.sorted { $0.id < $1.id }
    }
}
