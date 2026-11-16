import Foundation

/// The model hub the line "Other model" asks (Settings, Decisions; SPEC A133): Hugging Face. A check asks two public reads, with no key: the
/// model's card (`/api/models/<owner>/<name>?blobs=true`: its revision `sha`, whether it is `gated`, its `license:` tag, each file with its size and,
/// for a large file, its `lfs.sha256`) and its `config.json` at that revision (`model_type`). From them it decides whether the app can run the model
/// (the label reader family, the engine of `DecisionModels`) and which files a download takes. It downloads no weights: the line's installer does,
/// on the owner's click.
/// Every request passes `ModelsFolder.rigSafe`: a test copy asks only this Mac (`fixture provider-endpoint model-hub <loopback url>`, `useRigBase`).
public enum ModelHub {
    /// The hub's address on the owner's copy.
    public static let hub = URL(string: "https://huggingface.co")!

    /// A model's address on the hub: `owner/name`.
    public struct Address: Equatable, Sendable {
        public let owner: String
        public let name: String
        /// The id of the line it becomes (`DecisionModelEntry.id`, the saved choice).
        public var id: String { "hub:\(owner)/\(name)" }
        /// The model's page, as a built-in line names its own (`DecisionModelEntry.page`).
        public var page: String { "https://huggingface.co/\(owner)/\(name)" }
    }

    /// Why a model is not added; each has one sentence (`AIWords.hubRefusal`).
    public enum Refusal: Error, Equatable, Sendable {
        case notAnAddress
        case noModel
        case gated
        case kind(String)
        case ownHead
        case missing(String)
        case tooLarge(Int64)
        case unreachable
        case already
    }

    /// A model the app can run, as the hub described it: what `added-models.json` keeps for its line.
    public struct Found: Codable, Equatable, Sendable {
        public let id: String
        public let owner: String
        public let name: String
        /// The revision every file is fetched at (the card's `sha`), so a file never changes under its checksum.
        public let revision: String
        /// The id of the card's `license:` tag ("apache-2.0", "other"); empty when the card names none.
        public let licence: String
        /// The files the download takes, each with its size and its SHA-256 when the hub gives one (empty: checked by its size alone).
        public let files: [ModelFile]

        public init(owner: String, name: String, revision: String, licence: String, files: [ModelFile]) {
            self.id = "hub:\(owner)/\(name)"; self.owner = owner; self.name = name; self.revision = revision; self.licence = licence; self.files = files
        }
        public var totalBytes: Int64 { files.reduce(0) { $0 + $1.bytes } }
    }

    // MARK: what the app runs (the label reader family)

    /// The kinds (`model_type` of config.json) the label reader reads.
    static let kinds: Set<String> = ["qwen3_5", "qwen3_5_text"]
    /// The files a label reader cannot load without, besides its weights.
    static let needed = ["config.json", "tokenizer.json", "tokenizer_config.json"]
    /// The files it reads when the repository has them.
    static let optional = ["vocab.json", "merges.txt", "chat_template.jinja", "temperatures.json"]
    /// The file of a model that reads decisions through its own head and attention mode, which the app does not run.
    static let ownHeadFile = "decision_config.json"
    /// The share of this Mac's memory the weights may take.
    static let memoryShare = 0.6

    // MARK: the address

    /// The typed address as `owner/name`: `owner/name`, `huggingface.co/owner/name` or `https://huggingface.co/owner/name`; anything after the name
    /// (a path, a query, a fragment) is dropped. Anything else is nil.
    public static func address(_ typed: String) -> Address? {
        var s = typed.trimmingCharacters(in: .whitespacesAndNewlines)
        if let cut = s.firstIndex(where: { $0 == "?" || $0 == "#" }) { s = String(s[..<cut]) }
        let scheme = "https://", host = "huggingface.co/"
        if s.lowercased().hasPrefix(scheme) {
            s = String(s.dropFirst(scheme.count))
            guard s.lowercased().hasPrefix(host) else { return nil }
        }
        if s.lowercased().hasPrefix(host) { s = String(s.dropFirst(host.count)) }
        let parts = s.split(separator: "/", omittingEmptySubsequences: false).map(String.init)
        guard parts.count >= 2, isPart(parts[0]), isPart(parts[1]) else { return nil }
        return Address(owner: parts[0], name: parts[1])
    }

    /// One part of a hub address: letters, digits, `-`, `_` and `.`, not only dots, at most 96 characters.
    private static func isPart(_ p: String) -> Bool {
        let allowed = CharacterSet(charactersIn: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_.")
        return !p.isEmpty && p.count <= 96 && p.unicodeScalars.allSatisfy { allowed.contains($0) } && !p.allSatisfy { $0 == "." }
    }

    // MARK: the check

    private struct Card: Decodable {
        struct Sibling: Decodable {
            struct LFS: Decodable { let sha256: String?; let size: Int64? }
            let rfilename: String
            let size: Int64?
            let lfs: LFS?
        }
        let sha: String?
        /// false, or the hub's word for how access is granted ("auto", "manual").
        let gated: Bool?
        let tags: [String]?
        let siblings: [Sibling]?

        enum CodingKeys: String, CodingKey { case sha, gated, tags, siblings }
        init(from d: Decoder) throws {
            let c = try d.container(keyedBy: CodingKeys.self)
            sha = try? c.decode(String.self, forKey: .sha)
            if let b = try? c.decode(Bool.self, forKey: .gated) { gated = b } else { gated = (try? c.decode(String.self, forKey: .gated)) == nil ? nil : true }
            tags = try? c.decode([String].self, forKey: .tags)
            siblings = try? c.decode([Sibling].self, forKey: .siblings)
        }
    }

    private struct Config: Decodable { let model_type: String? }

    /// Asks the hub about one model and answers the line it may become, or why not. `memory` is this Mac's memory in bytes.
    public static func check(_ a: Address, memory: UInt64 = ProcessInfo.processInfo.physicalMemory) async -> Result<Found, Refusal> {
        let base = currentBase
        var card = URLComponents(url: base.appendingPathComponent("api/models/\(a.owner)/\(a.name)"), resolvingAgainstBaseURL: false)
        card?.queryItems = [URLQueryItem(name: "blobs", value: "true")]
        guard let cardURL = card?.url, let answer = await get(cardURL) else { return .failure(.unreachable) }
        let (code, data) = answer
        // the hub answers 404 for an address that holds no model (401 when it holds none it shows without a sign-in: no public model either)
        if code == 404 || code == 401 { return .failure(.noModel) }
        guard code == 200, let c = try? JSONDecoder().decode(Card.self, from: data), let sha = c.sha, !sha.isEmpty else { return .failure(.unreachable) }
        guard c.gated == false else { return .failure(.gated) }
        let files = Dictionary((c.siblings ?? []).map { ($0.rfilename, $0) }) { first, _ in first }
        guard files["config.json"] != nil else { return .failure(.missing("config.json")) }
        let configURL = base.appendingPathComponent("\(a.owner)/\(a.name)/resolve/\(sha)/config.json")
        guard let configAnswer = await get(configURL) else { return .failure(.unreachable) }
        let (ccode, cdata) = configAnswer
        if ccode == 404 { return .failure(.missing("config.json")) }
        guard ccode == 200, let config = try? JSONDecoder().decode(Config.self, from: cdata) else { return .failure(.unreachable) }
        let kind = config.model_type ?? ""
        guard kinds.contains(kind) else { return .failure(.kind(kind.isEmpty ? "?" : kind)) }
        if files[ownHeadFile] != nil { return .failure(.ownHead) }
        for n in needed where files[n] == nil { return .failure(.missing(n)) }
        let weights: [String]
        switch weightFiles(Set(files.keys)) {
        case .success(let w): weights = w
        case .failure(let r): return .failure(r)
        }
        var taken: [ModelFile] = []
        for n in needed + optional.filter({ files[$0] != nil }) + weights {
            guard let s = files[n], let bytes = s.size ?? s.lfs?.size else { return .failure(.unreachable) }
            taken.append(ModelFile(path: n, bytes: bytes, sha256: s.lfs?.sha256 ?? ""))
        }
        let weightBytes = taken.filter { weights.contains($0.path) }.reduce(Int64(0)) { $0 + $1.bytes }
        if Double(weightBytes) > Double(memory) * memoryShare { return .failure(.tooLarge(weightBytes)) }
        let licence = (c.tags ?? []).first { $0.hasPrefix("license:") }.map { String($0.dropFirst("license:".count)) } ?? ""
        return .success(Found(owner: a.owner, name: a.name, revision: sha, licence: licence, files: taken))
    }

    /// The weights: `model.safetensors`, or every piece `model-NNNNN-of-NNNNN.safetensors` of one count. A missing piece is named.
    static func weightFiles(_ names: Set<String>) -> Result<[String], Refusal> {
        if names.contains("model.safetensors") { return .success(["model.safetensors"]) }
        let pieces = names.compactMap { n -> (Int, Int)? in
            let p = n.split(separator: "-")
            guard n.hasSuffix(".safetensors"), p.count == 4, p[0] == "model", p[2] == "of", p[1].count == 5,
                  let i = Int(p[1]), let total = Int(p[3].dropLast(".safetensors".count)), p[3].count == 5 + ".safetensors".count, i >= 1, i <= total else { return nil }
            return (i, total)
        }
        guard let total = pieces.map(\.1).max() else { return .failure(.missing("model.safetensors")) }
        let all = (1...total).map { String(format: "model-%05d-of-%05d.safetensors", $0, total) }
        if let gap = all.first(where: { !names.contains($0) }) { return .failure(.missing(gap)) }
        return .success(all)
    }

    /// One GET, through the rig rule; nil when the hub could not be reached.
    private static func get(_ url: URL) async -> (Int, Data)? {
        let req = URLRequest(url: ModelsFolder.rigSafe(url), cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 20)
        let session = URLSession(configuration: .ephemeral)
        defer { session.finishTasksAndInvalidate() }
        guard let answer = try? await session.data(for: req), let http = answer.1 as? HTTPURLResponse else { return nil }
        return (http.statusCode, answer.0)
    }

    // MARK: the address a check asks

    private final class Rig: @unchecked Sendable {
        let lock = NSLock()
        var base: URL?
    }
    private static let rig = Rig()

    /// The hub a check asks: the owner's copy asks Hugging Face; a test copy asks the loopback address its check staged (else the real one, which
    /// `ModelsFolder.rigSafe` turns into a refusing address: "Could not reach Hugging Face").
    public static var currentBase: URL { rig.lock.withLock { rig.base } ?? hub }

    /// Rig copies only (`fixture provider-endpoint model-hub <url>`): the hub's answers come from a loopback server of the check, as
    /// `DecisionModels.useRigBase` does for a line's files. An empty address puts the real one back. The owner's copy ignores it.
    public static func useRigBase(_ raw: String) {
        guard CorePlace.current(env: ProcessInfo.processInfo.environment).rig else { return }
        let url = raw.isEmpty ? nil : ModelsFolder.loopbackOnly(raw)
        rig.lock.withLock { rig.base = url }
    }
}
