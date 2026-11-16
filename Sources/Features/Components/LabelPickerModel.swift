import Foundation

/// A label and the labels under it. Labels are shared by every account, so the tree is one tree.
public struct LabelNode: Equatable, Sendable, Decodable {
    public let name: String
    public let children: [LabelNode]
    private enum CodingKeys: String, CodingKey { case name, children }
    public init(name: String, children: [LabelNode] = []) { self.name = name; self.children = children }
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        name = try c.decode(String.self, forKey: .name)
        children = try c.decodeIfPresent([LabelNode].self, forKey: .children) ?? []
    }
}

/// One line of the picker: the label, where it sits, how deep.
public struct LabelRow: Equatable, Sendable, Identifiable {
    public let path: [String]
    public let hasChildren: Bool
    public var name: String { path.last ?? "" }
    public var depth: Int { path.count - 1 }
    public var id: String { path.joined(separator: "/") }
    public var breadcrumb: String { path.dropLast().joined(separator: " › ") }
}

/// The label picker's logic: every label nested, a search that keeps the way back to the parent, and Create
/// when the typed name is new. Pure value type, so the same logic serves the Mac popover and the iPhone sheet.
public struct LabelPickerModel: Equatable, Sendable {
    public var nodes: [LabelNode]
    public var query = ""
    public var selected: Set<String> = []
    /// Row ids of groups folded shut; the search ignores folding.
    public var collapsed: Set<String> = []

    public init(nodes: [LabelNode], selected: Set<String> = [], collapsed: Set<String> = []) {
        self.nodes = nodes; self.selected = selected; self.collapsed = collapsed
    }

    public func allRows() -> [LabelRow] {
        var out: [LabelRow] = []
        func walk(_ n: [LabelNode], _ prefix: [String]) {
            for l in n {
                out.append(LabelRow(path: prefix + [l.name], hasChildren: !l.children.isEmpty))
                walk(l.children, prefix + [l.name])
            }
        }
        walk(nodes, [])
        return out
    }

    /// With no query, the whole tree. With one, the matching labels, each with its parents' names as a breadcrumb.
    public func visibleRows() -> [LabelRow] {
        let q = Self.fold(query)
        guard !q.isEmpty else {
            return allRows().filter { row in !collapsed.contains { row.id.hasPrefix($0 + "/") } }
        }
        return allRows().filter { Self.fold($0.name).contains(q) }
    }

    /// The name to offer under "Create", or nil when the field is empty or the label exists.
    public var createOffer: String? {
        let name = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty else { return nil }
        return allRows().contains { Self.fold($0.name) == Self.fold(name) } ? nil : name
    }

    public mutating func toggleGroup(_ row: LabelRow) {
        if collapsed.contains(row.id) { collapsed.remove(row.id) } else { collapsed.insert(row.id) }
    }

    public mutating func toggle(_ row: LabelRow) {
        if selected.contains(row.id) { selected.remove(row.id) } else { selected.insert(row.id) }
    }

    static func fold(_ s: String) -> String {
        s.trimmingCharacters(in: .whitespacesAndNewlines).folding(options: [.caseInsensitive, .diacriticInsensitive], locale: .current)
    }
}

/// The design fixture's label tree (the `labels` key of mac-main-sample.json); empty when the fixture is absent.
enum LabelFixture {
    private struct File: Decodable { let labels: [LabelNode]? }
    static func nodes() -> [LabelNode] {
        guard let url = FixtureFile.locate(), let data = try? Data(contentsOf: url),
              let f = try? JSONDecoder().decode(File.self, from: data) else { return [] }
        return f.labels ?? []
    }
}
