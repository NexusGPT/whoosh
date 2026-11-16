import Foundation

/// The label gestures. The sidebar's drag and drop, the detail pane's fields and the socket's clicks all land here,
/// so each gesture has one path whichever way it is made. Reads come from the core's `labels()`; create, rename and
/// move are the core's `createLabel`, `renameLabel` and `moveLabel` commands (delta 6).
extension LabelsContactsModel {
    /// Rebuilds the tree from the core's rows.
    func reloadLabels() {
        let tree = LabelTree(rows: app.core.labels())
        if tree != labels { labels = tree }
        if selectedLabelId.map({ tree.item($0) == nil }) ?? true {
            selectedLabelId = tree.ordered().first?.id
            renameText = selectedLabelId.flatMap(tree.item)?.name ?? ""
        }
    }

    func selectLabel(_ id: String) -> Bool {
        guard let l = labels.item(id) else { return false }
        selectedLabelId = id; renameText = l.name; labelRefusal = nil; pane = .labels
        return true
    }

    /// Runs one label command; the tree is re-read from `core.labels()` after it, and a refusal shows the core's words.
    @discardableResult
    private func labelCommand(_ command: Command) -> CommandOutcome? {
        do {
            let out = try app.core.run(command: command)
            labelRefusal = nil; reloadLabels()
            return out
        } catch let e as CoreError {
            labelRefusal = Self.words(of: e); return nil
        } catch {
            labelRefusal = error.localizedDescription; return nil
        }
    }

    /// Creates `name` inside the selected label (or at the top) through `createLabel`, then selects it.
    @discardableResult
    func createLabel(_ name: String, atTop: Bool = false) -> Bool {
        guard let out = labelCommand(.createLabel(name: name, parentId: atTop ? nil : selectedLabelId)) else { return false }
        newLabelName = ""
        return out.createdId.map(selectLabel) ?? true
    }

    @discardableResult
    func renameSelected(_ name: String) -> Bool {
        guard let id = selectedLabelId, labelCommand(.renameLabel(labelId: id, name: name)) != nil else { return false }
        renameText = labels.item(id)?.name ?? ""
        return true
    }

    /// Drops label `id` onto `parent` (nil: the top of the tree) through `moveLabel`.
    @discardableResult
    func moveLabel(_ id: String, into parent: String?) -> Bool {
        guard labelCommand(.moveLabel(labelId: id, parentId: parent)) != nil else { return false }
        _ = selectLabel(id)
        return true
    }

    /// The core's refusal in the core's own words.
    static func words(of e: CoreError) -> String {
        switch e {
        case .Invalid(_, let words): return words
        case .NotFound: return LabelsWords.gone
        default: return "\(e)"
        }
    }

    /// Socket rows for the tree: one per label, parents first.
    func labelRows(_ ax: String) -> [ControlRow] {
        reloadLabels()
        return labels.ordered().map { l in
            ["ax": "\(ax).labels.\(l.id)", "id": l.id, "name": l.name, "path": labels.pathText(l.id),
             "depth": labels.depth(l.id), "parent": l.parentId ?? NSNull(), "selected": l.id == selectedLabelId, "pane": pane.rawValue]
        }
    }
}
