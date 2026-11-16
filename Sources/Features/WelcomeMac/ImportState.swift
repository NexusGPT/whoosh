import Foundation

/// One source as `import/sources` serves it (docs/lanes/contracts.md "Delta 18", state paths).
struct ImportSourceItem: Identifiable, Equatable {
    let id: String
    let kind: String
    let name: String
    let location: String
    let count: Int?
    let imported: Int
    let access: String
    let words: String
    var readable: Bool { access == "readable" }
}

/// One run as `import/<run>/progress` serves it.
struct ImportRunItem: Equatable {
    let runId: String
    let sourceId: String
    let state: String
    let done: Int
    let total: Int?
    let fraction: Double?
    let current: String
    let stored: Int
    let skipped: Int
    let refusalCode: String?
    let refusalWords: String?
    var isOver: Bool { state != "running" }
}

/// An account the mail can be put into.
struct ImportAccountItem: Identifiable, Equatable { let id: String; let name: String }

/// What a finished run reports (`import/<run>/result`).
struct ImportResultItem: Equatable {
    let seen: Int
    let stored: Int
    let alreadyThere: Int
    let skipped: Int
}

/// Reads of the three state paths. The values are JSON text; a null, a missing path or a shape this reader does not know is nil.
enum ImportState {
    private static func object(_ core: WhooshCore, _ path: String) -> Any? {
        guard let text = core.state(path: path), let data = text.data(using: .utf8) else { return nil }
        return try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])
    }
    private static func int(_ v: Any?) -> Int? { (v as? NSNumber)?.intValue }

    static func sources(_ core: WhooshCore) -> [ImportSourceItem] {
        guard let list = object(core, "import/sources") as? [[String: Any]] else { return [] }
        return list.compactMap { d in
            guard let id = d["id"] as? String, let kind = d["kind"] as? String else { return nil }
            return ImportSourceItem(id: id, kind: kind, name: d["name"] as? String ?? id, location: d["location"] as? String ?? "",
                                    count: int(d["message_count"]), imported: int(d["imported_count"]) ?? 0,
                                    access: d["access"] as? String ?? "readable", words: d["words"] as? String ?? "")
        }
    }

    static func progress(_ core: WhooshCore, run: String) -> ImportRunItem? {
        guard let d = object(core, "import/\(run)/progress") as? [String: Any], let state = d["state"] as? String else { return nil }
        let refusal = d["refusal"] as? [String: Any]
        return ImportRunItem(runId: run, sourceId: d["source_id"] as? String ?? "", state: state, done: int(d["done"]) ?? 0, total: int(d["total"]),
                             fraction: (d["fraction"] as? NSNumber)?.doubleValue, current: d["current"] as? String ?? "",
                             stored: int(d["stored"]) ?? 0, skipped: int(d["skipped"]) ?? 0,
                             refusalCode: refusal?["code"] as? String, refusalWords: refusal?["words"] as? String)
    }

    static func result(_ core: WhooshCore, run: String) -> ImportResultItem? {
        guard let d = object(core, "import/\(run)/result") as? [String: Any] else { return nil }
        return ImportResultItem(seen: int(d["seen"]) ?? 0, stored: int(d["stored"]) ?? 0, alreadyThere: int(d["already_there"]) ?? 0,
                                skipped: (d["skipped"] as? [Any])?.count ?? 0)
    }
}
