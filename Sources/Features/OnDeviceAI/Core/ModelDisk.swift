import Foundation

/// What the installer reads and writes on disk: the ready manifest, file sizes, free space.
enum ModelDisk {
    static func size(of url: URL) -> Int64? {
        (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? NSNumber)?.int64Value
    }

    static func manifestURL(_ dir: URL) -> URL { dir.appendingPathComponent("ready.json") }

    /// Ready only when the manifest lists exactly the spec's files and every file is on disk at its size.
    static func isReady(_ spec: MlxModelSpec, in dir: URL) -> Bool {
        guard let data = try? Data(contentsOf: manifestURL(dir)),
              let listed = try? JSONDecoder().decode([ModelFile].self, from: data), listed == spec.files else { return false }
        return spec.files.allSatisfy { size(of: dir.appendingPathComponent($0.path)) == $0.bytes }
    }

    static func writeManifest(_ spec: MlxModelSpec, in dir: URL) throws {
        try JSONEncoder().encode(spec.files).write(to: manifestURL(dir), options: .atomic)
    }

    /// Bytes already on this Mac for the spec: finished files at their size plus the `.part` of the file in flight.
    static func bytesOnDisk(_ spec: MlxModelSpec, in dir: URL) -> Int64 {
        spec.files.reduce(0) { sum, f in
            let done = size(of: dir.appendingPathComponent(f.path)) == f.bytes ? f.bytes : 0
            return sum + (done > 0 ? done : min(f.bytes, size(of: dir.appendingPathComponent(f.path + ".part")) ?? 0))
        }
    }

    /// Free bytes on the volume that holds `dir`, or nil when the system does not say.
    static func freeBytes(near dir: URL) -> Int64? {
        var probe = dir
        while !FileManager.default.fileExists(atPath: probe.path), probe.pathComponents.count > 1 { probe.deleteLastPathComponent() }
        return (try? probe.resourceValues(forKeys: [.volumeAvailableCapacityForImportantUsageKey]))?.volumeAvailableCapacityForImportantUsage
    }
}
