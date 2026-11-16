import Foundation

/// The label tree as the Labels page shows it: every node of `core.labels()` as a display path ("client › Provanta").
/// Paths come from the core (`MailLabel.path`, built by the contract's `labelPath`) and are split by `parseLabelPath`,
/// never by hand, so a name holding the separator still reads right.
@MainActor
enum AILabelTree {
    static let separator = " › "

    /// Every node, depth first in the core's order, as display paths.
    static func nodes(_ core: WhooshCore = AppModel.shared.core) -> [String] {
        core.labels().map(display)
    }

    static func display(_ label: MailLabel) -> String {
        ((try? parseLabelPath(path: label.path)) ?? [label.name]).joined(separator: separator)
    }

    /// The real node whose last name matches the last name of `path`; `path` itself when the tree has none.
    static func resolve(_ path: String, in tree: [String]) -> String {
        if tree.contains(path) { return path }
        let leaf = path.components(separatedBy: separator).last ?? path
        return tree.first { ($0.components(separatedBy: separator).last ?? $0).caseInsensitiveCompare(leaf) == .orderedSame } ?? path
    }

    /// INTERIM: the model's proposed child, hung under a real node, until the decision engine proposes one itself.
    static func proposal(in tree: [String]) -> String? {
        let parent = resolve("Provanta", in: tree)
        guard tree.contains(parent) else { return tree.max { $0.count < $1.count }.map { $0 + separator + AIWords.proposedName } }
        return parent + separator + AIWords.proposedName
    }

    /// The depth of a display path (1 for a root node), for the menu's indent.
    static func depth(_ path: String) -> Int { path.components(separatedBy: separator).count }
    static func leaf(_ path: String) -> String { path.components(separatedBy: separator).last ?? path }
}

extension AILabelTree {
    /// The path one level up ("" for a root node).
    static func parent(_ path: String) -> String {
        path.components(separatedBy: separator).dropLast().joined(separator: separator)
    }
}
