import Foundation

/// Where the downloaded models of the on-device features live. The owner's copy keeps them in Application Support; a rig copy (the hidden
/// test copy) keeps them under its own run folder, so a test never reads or fills the owner's folder (`CorePlace` decides what a rig is).
public enum ModelsFolder {
    /// The folder everything of the app's own files lives in: the owner's `~/Library/Application Support/Whoosh`, or the rig's run folder on a rig copy.
    public static func whooshRoot(env: [String: String] = ProcessInfo.processInfo.environment) -> URL {
        let place = CorePlace.current(env: env)
        if place.rig { return place.dataDir.deletingLastPathComponent() }
        return FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("Whoosh", isDirectory: true)
    }

    /// `.../Models`: the owner's `~/Library/Application Support/Whoosh/Models`, or `<rig run folder>/Models` on a rig copy.
    public static func root(env: [String: String] = ProcessInfo.processInfo.environment) -> URL {
        whooshRoot(env: env).appendingPathComponent("Models", isDirectory: true)
    }

    /// Where a rig copy may fetch the Laya weights from instead of the real host: `WHOOSH_RIG_LAYA_BASE`, loopback only (127.0.0.1, ::1 or localhost).
    /// A rig copy given any other host gets an address that refuses at once, so nothing leaves the Mac. nil on the owner's copy and when unset.
    public static func rigLayaBase(env: [String: String] = ProcessInfo.processInfo.environment) -> URL? {
        guard CorePlace.current(env: env).rig, let raw = env["WHOOSH_RIG_LAYA_BASE"], !raw.isEmpty else { return nil }
        return loopbackOnly(raw)
    }

    /// The address a model download may ask. The owner's copy: the address as given. A rig copy (the hidden test copy): the address
    /// only when its host is this Mac (127.0.0.1, ::1, localhost), else an address that refuses at once, so a test copy asks nothing
    /// outside this Mac whatever a caller passes (owner: "nothing leaves this Mac except the relay"; found 2026-11-11: a rig copy with no
    /// WHOOSH_RIG_LAYA_BASE fell back to the real host for the decision model).
    /// The rule itself is `RigNet.safe` (Sources/CoreBridge/CoreHost.swift), the one definition every Swift caller shares.
    public static func rigSafe(_ url: URL, env: [String: String] = ProcessInfo.processInfo.environment) -> URL {
        RigNet.safe(url, env: env)
    }

    /// The loopback address as given, else a refusing one.
    public static func loopbackOnly(_ raw: String) -> URL {
        if let u = URL(string: raw), RigNet.isThisMac(u.host) { return u }
        return RigNet.refusing
    }
}
