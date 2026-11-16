import Foundation

/// Where the search model's download goes and comes from. The owner's copy: the pinned spec (Hugging Face, a fixed revision) into
/// `~/Library/Application Support/Whoosh/Models/embeddinggemma-2/<revision>`. A rig copy (it has `WHOOSH_SOCKET_DIR`, its run folder):
/// its own folder under the run folder, never the owner's models folder, and no network: the address comes only from
/// `WHOOSH_RIG_EMBED_BASE` and only a loopback address is accepted (same pattern as the decision model's WHOOSH_RIG_LAYA_BASE).
/// An optional `embed-fixture.json` in the run folder (`{"files":[{"path","bytes","sha256"}]}`) swaps the file list for a small fixture.
public struct MeaningModelRig: Sendable {
    public let spec: MlxModelSpec
    public let directory: URL
    /// Set when this is a rig copy that was given no usable loopback address: the download then refuses in words and reaches nothing.
    public let blocked: String?

    public static func loopback(_ url: URL) -> Bool {
        guard let host = url.host?.lowercased(), url.scheme == "http" || url.scheme == "https" else { return false }
        return host == "127.0.0.1" || host == "localhost" || host == "::1" || host == "[::1]"
    }

    /// The rig's models folder (also searched by `GemmaWeights.candidates`), or nil on the owner's copy.
    public static func runModelsFolder(environment: [String: String]) -> URL? {
        guard let run = environment["WHOOSH_SOCKET_DIR"], !run.isEmpty else { return nil }
        return URL(fileURLWithPath: run, isDirectory: true).appendingPathComponent("models/embeddinggemma-2/\(MlxModelSpec.embeddingGemma2Revision)", isDirectory: true)
    }

    public static func current(environment: [String: String] = ProcessInfo.processInfo.environment) -> MeaningModelRig {
        let pinned = MlxModelSpec.embeddingGemma2
        guard let folder = runModelsFolder(environment: environment) else {
            // the folder follows the environment this call was given, not the process's own (a test passes one)
            return MeaningModelRig(spec: pinned, directory: ModelsFolder.root(env: environment).appendingPathComponent(pinned.folderName, isDirectory: true), blocked: nil)
        }
        guard let text = environment["WHOOSH_RIG_EMBED_BASE"], let base = URL(string: text), loopback(base) else {
            return MeaningModelRig(spec: pinned, directory: folder, blocked: Words.text("This test copy downloads only from a loopback address (WHOOSH_RIG_EMBED_BASE). Nothing was asked."))
        }
        struct Fixture: Decodable { let files: [ModelFile] }
        let run = environment["WHOOSH_SOCKET_DIR"] ?? ""
        let files = FileManager.default.contents(atPath: run + "/embed-fixture.json").flatMap { try? JSONDecoder().decode(Fixture.self, from: $0).files } ?? pinned.files
        let spec = MlxModelSpec(name: pinned.name, licence: pinned.licence, baseURL: base, files: files, folderName: pinned.folderName)
        return MeaningModelRig(spec: spec, directory: folder, blocked: nil)
    }
}
