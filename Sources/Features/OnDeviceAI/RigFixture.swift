import Foundation

/// A test rig can point the install step at a local fixture server. The pointer is a JSON file in the rig's run
/// folder (`$WHOOSH_SOCKET_DIR`, which only a rig copy has), so the owner's installed copy never reads one.
struct RigFixture {
    let spec: MlxModelSpec
    let directory: URL
    /// What the rig arms Apple Intelligence as: "ready" or an `AppleReason` raw value. Absent means "device_not_eligible",
    /// so a rig run is the same on a Mac with Apple Intelligence and on one without.
    let appleKey: String
    var apple: ArmedAppleBackend { .armed(appleKey) }

    private struct File: Decodable { let name: String; let base: String; let dir: String; let files: [ModelFile]; let apple: String? }

    static func load() -> RigFixture? {
        guard let run = ProcessInfo.processInfo.environment["WHOOSH_SOCKET_DIR"],
              let data = FileManager.default.contents(atPath: run + "/ondeviceai-fixture.json"),
              let f = try? JSONDecoder().decode(File.self, from: data), let base = URL(string: f.base) else { return nil }
        return RigFixture(spec: MlxModelSpec(name: f.name, licence: "Apache-2.0", baseURL: base, files: f.files),
                          directory: URL(fileURLWithPath: f.dir, isDirectory: true), appleKey: f.apple ?? AppleReason.deviceNotEligible.rawValue)
    }
}
