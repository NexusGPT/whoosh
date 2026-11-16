import Foundation

/// One label: its own id (stable across renames and moves), its name, the label it sits in.
struct LabelItem: Identifiable, Equatable, Sendable {
    let id: String
    var name: String
    var parentId: String?
}

/// The label tree as the core serves it (`WhooshCore.labels()`: flat rows, a parent before its children). A read
/// model only: create, rename and move go through the core's commands, which refuse in the core's own words.
struct LabelTree: Equatable, Sendable {
    private(set) var items: [LabelItem] = []
    static let separator = " > "

    init(rows: [MailLabel]) {
        items = rows.map { LabelItem(id: $0.id, name: $0.name, parentId: $0.parentId) }
    }

    func item(_ id: String) -> LabelItem? { items.first { $0.id == id } }
    func children(of id: String?) -> [LabelItem] { items.filter { $0.parentId == id } }

    /// 1 for a top-level label.
    func depth(_ id: String) -> Int { path(id).count }

    func path(_ id: String) -> [String] {
        var out: [String] = []
        var cur = item(id)
        while let c = cur, out.count <= items.count { out.insert(c.name, at: 0); cur = c.parentId.flatMap(item) }
        return out
    }

    func pathText(_ id: String) -> String { path(id).joined(separator: Self.separator) }

    /// Levels from this label down to its deepest descendant, itself included.
    func height(_ id: String) -> Int { 1 + (children(of: id).map { height($0.id) }.max() ?? 0) }

    /// Every label, parents before children, in tree order.
    func ordered() -> [LabelItem] {
        children(of: nil).flatMap { walk($0) }
    }
    private func walk(_ l: LabelItem) -> [LabelItem] { [l] + children(of: l.id).flatMap { walk($0) } }
}
