import Foundation

/// A speech model file that exists only after first use. The download is the one allowed network act (a model, never audio).
public protocol ModelDownloader: Sendable { func download(from: URL, to: URL, progress: @Sendable (Double) -> Void) async throws }

public struct ModelStore: Sendable {
    public let directory: URL, fileName: String, source: URL
    public init(directory: URL, fileName: String, source: URL) { self.directory = directory; self.fileName = fileName; self.source = source }
    public var modelURL: URL { directory.appendingPathComponent(fileName) }
    public var isInstalled: Bool { FileManager.default.fileExists(atPath: modelURL.path) }

    /// Downloads on first use only. The file is moved into place when complete, so a half download is never taken for a model.
    @discardableResult
    public func ensure(using d: any ModelDownloader, progress: @Sendable (Double) -> Void = { _ in }) async throws -> URL {
        if isInstalled { return modelURL }
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let part = directory.appendingPathComponent(fileName + ".part")
        try? FileManager.default.removeItem(at: part)
        do { try await d.download(from: source, to: part, progress: progress) } catch { try? FileManager.default.removeItem(at: part); throw error }
        try FileManager.default.moveItem(at: part, to: modelURL)
        return modelURL
    }
}
