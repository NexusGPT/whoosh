import SwiftUI

/// The seam other lanes call: pane 3 (reader) mounts `LabelInspectorView`, pane 2 (list) mounts `LabelProofLine` and
/// `LabelProofRows`, the sidebar opens the inspector. Everything behind it lives in this folder.
@MainActor
enum LabelInspector {
    static var model: LabelInspectorModel { LabelInspectorModel.shared }
    /// pane 3 shows the inspector instead of the reader
    static var isOpen: Bool { model.isOpen }
    /// pane 2 draws the live proof (the count line and the matched rows) instead of the mailbox
    static var showsProof: Bool { model.isOpen && model.showMatches }
    /// the sidebar draws the edit ring on this label's row
    static var editingLabelId: String? { model.labelId }
    /// the sidebar draws a "New" row under `parentId` carrying `name` while a new label is being defined
    static var newRow: (parentId: String?, name: String)? { model.isNew ? (model.draft.parentId, model.draft.name) : nil }

    /// `name` is the words already typed (the label picker's "Create and define…" passes them).
    @discardableResult static func openNew(parent: String?, name: String = "") -> Bool { model.openNew(parent: parent, name: name) }
    @discardableResult static func openModify(_ id: String) -> Bool { model.openModify(id) }
    @discardableResult static func toggle() -> Bool {
        let sel = SidebarState.shared.selection.id
        let id = sel.hasPrefix("label.") ? String(sel.dropFirst("label.".count)) : nil
        return model.toggle(selected: id)
    }
}
