import Foundation

/// Where the EmbeddingGemma 2 weights live and how they arrive.
/// The weights are never committed: they download once, on first use, into the app's support folder.
/// Revision and files are the ones the owner's launch-search project pins (pipeline/embed/config.py:22).
public struct GemmaWeights: Sendable {
    public static let modelId = "google/embeddinggemma-2"
    public static let revision = "914f7f89142e33e77833254d9c9b90c3cef7303b"
    /// Every file the runtime reads, text and media (one `model.safetensors` holds the text, vision and audio towers; it cannot
    /// be fetched in parts). Round 2 read the loaders: the pooling and sentence-transformers files round 1 also fetched are never
    /// opened (MLXEmbedders pools EmbeddingGemma 2 inside the model), and the rig copy, a full snapshot of the revision, has no
    /// special_tokens_map.json or added_tokens.json. The on-device lane's installer spec lists the same seven files, with sizes and SHA-256.
    public static let files = [
        "config.json", "tokenizer_config.json", "preprocessor_config.json", "processor_config.json",
        "tokenizer.model", "tokenizer.json", "model.safetensors",
    ]

    public let directory: URL

    public init(directory: URL) { self.directory = directory }

    /// `~/Library/Application Support/Whoosh/Models/embeddinggemma-2/<revision>` (the app's support folder).
    public static func inSupportFolder(base: URL? = nil) -> GemmaWeights {
        let models = base.map { $0.appendingPathComponent("Whoosh/Models", isDirectory: true) } ?? ModelsFolder.root()   // a rig copy: its own run folder
        return GemmaWeights(directory: models
            .appendingPathComponent("embeddinggemma-2", isDirectory: true)
            .appendingPathComponent(revision, isDirectory: true))
    }

    /// Every file is present. The installer writes a file under its final name only when it is whole (`.part` until then).
    public var isComplete: Bool {
        Self.files.allSatisfy { FileManager.default.fileExists(atPath: directory.appendingPathComponent($0).path) }
    }

    /// One file read back from the disk against the installer's spec (sizes live once, in `MlxModelSpec.embeddingGemma2`).
    public struct FileReadBack: Equatable, Sendable {
        public let path: String
        public let expectedBytes: Int64
        public let foundBytes: Int64?
        public var ok: Bool { foundBytes == expectedBytes }
    }

    /// Reads the location back: each spec file's size on disk (a symlink, as the Hugging Face cache uses, is followed).
    public func readBack(spec: MlxModelSpec = .embeddingGemma2) -> [FileReadBack] {
        spec.files.map { f in
            let url = directory.appendingPathComponent(f.path).resolvingSymlinksInPath()
            let size = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? NSNumber)?.int64Value
            return FileReadBack(path: f.path, expectedBytes: f.bytes, foundBytes: size)
        }
    }

    /// True when every file is there at its published size. The installer also checks the SHA-256 once, at install time;
    /// hashing 1.5 GB at every launch would cost seconds, so a copy used in place is judged by size.
    public func isIntact(spec: MlxModelSpec = .embeddingGemma2) -> Bool { readBack(spec: spec).allSatisfy(\.ok) }

    /// The user's home folder: the Mac's own call on the Mac, the app container's home elsewhere.
    public static var defaultHome: URL {
        #if os(macOS)
        FileManager.default.homeDirectoryForCurrentUser
        #else
        URL(fileURLWithPath: NSHomeDirectory(), isDirectory: true)
        #endif
    }

    /// The folders a copy of the weights may already sit in, best first. Used in place, never copied:
    /// 1. WHOOSH_EMBED_WEIGHTS (the rig folder), 2. the installer's folder, 3. a Hugging Face cache snapshot (2 and 3 on the owner's copy only)
    /// (the owner's own Mac; searchbetter does the same, app/Sources/Embedder.swift:18-23).
    public static func candidates(environment: [String: String] = ProcessInfo.processInfo.environment,
                                  home: URL = GemmaWeights.defaultHome,
                                  supportBase: URL? = nil) -> [GemmaWeights] {
        var out: [GemmaWeights] = []
        // a rig copy downloads into (and reads from) its own run folder, never the owner's models folder
        if let run = MeaningModelRig.runModelsFolder(environment: environment) { out.append(GemmaWeights(directory: run)) }
        if let env = environment["WHOOSH_EMBED_WEIGHTS"], !env.isEmpty { out.append(GemmaWeights(directory: URL(fileURLWithPath: env))) }
        // A test copy, and any process on the test engine, reads the weights only from its own run folder or WHOOSH_EMBED_WEIGHTS: never the
        // owner's installed model or cache. (2026-11-11: a unit test that typed in the search field woke the query vector, the test process
        // found the owner's installed model, and the model library stopped the whole run.)
        if CorePlace.isRig(env: environment) || environment["WHOOSH_FIXTURE_CORE"] == "1" { return out }
        out.append(inSupportFolder(base: supportBase))
        let snapshots = home.appendingPathComponent(".cache/huggingface/hub/models--google--embeddinggemma-2/snapshots", isDirectory: true)
        let names = (try? FileManager.default.contentsOfDirectory(atPath: snapshots.path)) ?? []
        // The pinned revision first, then any other snapshot: a different revision fails the size check if it differs.
        for n in names.sorted(by: { $0 == revision && $1 != revision }) { out.append(GemmaWeights(directory: snapshots.appendingPathComponent(n, isDirectory: true))) }
        return out
    }

    /// The first candidate that is whole, or nil: the model is then not installed, and the caller says so in words.
    public static func locate(environment: [String: String] = ProcessInfo.processInfo.environment,
                              home: URL = GemmaWeights.defaultHome,
                              supportBase: URL? = nil,
                              spec: MlxModelSpec = .embeddingGemma2) -> GemmaWeights? {
        candidates(environment: environment, home: home, supportBase: supportBase).first { $0.isIntact(spec: spec) }
    }
}
