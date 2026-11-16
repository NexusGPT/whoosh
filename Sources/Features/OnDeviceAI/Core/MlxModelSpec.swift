import Foundation

/// One file of the on-device language model: where it lives, how big it is, and the SHA-256 it must have.
public struct ModelFile: Equatable, Sendable, Codable {
    public let path: String
    public let bytes: Int64
    public let sha256: String
    public init(path: String, bytes: Int64, sha256: String) { self.path = path; self.bytes = bytes; self.sha256 = sha256 }
}

/// The pinned on-device language model: a fixed revision, so a file never changes under a checksum.
/// Qwen2.5 1.5B Instruct, 4-bit MLX conversion by mlx-community. Licence Apache-2.0 (the model card says so;
/// MLX is MIT). The 0.5B model of the same family answered the Ask prompt wrongly (it said all three mails wait on
/// the owner), so this is the smallest that answers both prompts correctly; docs/lanes/ondeviceai.md "## Round 3".
public struct MlxModelSpec: Equatable, Sendable, Codable {
    public let name: String
    public let licence: String
    public let baseURL: URL
    public let files: [ModelFile]
    /// The folder name under Application Support/Whoosh/Models (a path with a slash makes a nested folder).
    public let folderName: String

    public init(name: String, licence: String, baseURL: URL, files: [ModelFile], folderName: String = "mlx-language-model") {
        self.name = name; self.licence = licence; self.baseURL = baseURL; self.files = files; self.folderName = folderName
    }

    public var totalBytes: Int64 { files.reduce(0) { $0 + $1.bytes } }
    public func url(for file: ModelFile) -> URL { baseURL.appendingPathComponent(file.path) }
    public static let revision = "8b403126fc14f14cfc99bb4cfa72ecbc129ea677"
    public static let qwen25 = MlxModelSpec(
        name: "Qwen2.5 1.5B Instruct (4-bit)",
        licence: "Apache-2.0",
        baseURL: URL(string: "https://huggingface.co/mlx-community/Qwen2.5-1.5B-Instruct-4bit/resolve/\(revision)/")!,
        files: [
            ModelFile(path: "config.json", bytes: 784, sha256: "636d3e2a15e8914b8cf82b05cc2288a811f9bd93c3bf1afc00cab701a70b47c0"),
            ModelFile(path: "tokenizer_config.json", bytes: 7308, sha256: "f7c61e32b7a17d19bf8e7037dcb74079a833e53ea9801f24008cac68458f03b7"),
            ModelFile(path: "tokenizer.json", bytes: 7_031_673, sha256: "a8506e7111b80c6d8635951a02eab0f4e1a8e4e5772da83846579e97b16f61bf"),
            ModelFile(path: "model.safetensors", bytes: 868_628_559, sha256: "0979f33d1bc58afcf696d13f57977644e7b11a6f0eec3e631d8e9463d18c0717"),
        ])

    public static let embeddingGemma2Revision = "914f7f89142e33e77833254d9c9b90c3cef7303b"
    /// The search model: EmbeddingGemma 2 (text, image, audio), Apache-2.0, bf16 as published (embedding lane, round 2:
    /// docs/lanes/embedding.md). The folder equals GemmaWeights.inSupportFolder(), which loads it.
    public static let embeddingGemma2 = MlxModelSpec(
        name: "EmbeddingGemma 2 (text, image, audio)",
        licence: "Apache-2.0",
        baseURL: URL(string: "https://huggingface.co/google/embeddinggemma-2/resolve/\(embeddingGemma2Revision)/")!,
        files: [
            ModelFile(path: "config.json", bytes: 4455, sha256: "b8f1e9931b57fbc054acdb445c41765d55b0074c58d145fa82839941ad1b5bb3"),
            ModelFile(path: "tokenizer_config.json", bytes: 1599, sha256: "17bd5d6e9364ca49a534e1502076593317c298d4a663623091ed45388f004874"),
            ModelFile(path: "preprocessor_config.json", bytes: 511, sha256: "ea2ae257e901064abdd98dceb19f2b0da06af600bed15e0f99f5c85c37ee9d78"),
            ModelFile(path: "processor_config.json", bytes: 1788, sha256: "168f6a08522f3ce5dea596d94d003af2fd691742d4f41fe1f9d8cce76bfbf69c"),
            ModelFile(path: "tokenizer.model", bytes: 4_689_013, sha256: "e594c8a90eb08d8bda498ff4747977dc827ae0c3c56b5c0d41a605a22d02ef03"),
            ModelFile(path: "tokenizer.json", bytes: 32_170_510, sha256: "4d777ef5bdc1aa36227abdfb77c3e49e7b9c892d16e1b6bda41c393504828be4"),
            ModelFile(path: "model.safetensors", bytes: 1_488_915_288, sha256: "197a32965d4b1105faf060417baa899e193fb73cd401f42ec9295234d5553d79"),
        ],
        folderName: "embeddinggemma-2/\(embeddingGemma2Revision)")
}
