import Foundation

/// Where recordings live while they are audio. `remove` is the only way out; the controller calls it
/// the moment a recording becomes text, and on every failure path, so no audio outlives its use.
public struct AudioFileStore: Sendable {
    public let directory: URL

    public init(directory: URL = FileManager.default.temporaryDirectory.appendingPathComponent("whoosh-dictation", isDirectory: true)) {
        self.directory = directory
    }

    public func newFile() throws -> URL {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory.appendingPathComponent("rec-\(UUID().uuidString).wav")
    }

    @discardableResult
    public func remove(_ url: URL) -> Bool {
        (try? FileManager.default.removeItem(at: url)) != nil
    }

    /// The recordings still on disk: the core path `dictation/audio_files`.
    public var files: [URL] {
        ((try? FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)) ?? []).filter { $0.pathExtension == "wav" }
    }
}
