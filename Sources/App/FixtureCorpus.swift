import Foundation

/// What the fixture core adopts to serve `fixture corpus <name>`: replace its in-memory mail with the
/// named fixture file's (accounts, tabs, messages with their dates and thread ids), then send the
/// Inbox, Counts and SyncStatus streams their new state. The contracts lane conforms FixtureCore
/// (`extension FixtureCore: CorpusLoading {}`, once FixtureCore.loadCorpus(from:) is on main); until it does, the verb answers ok:false in words.
public protocol CorpusLoading: AnyObject {
    func loadCorpus(from url: URL) throws
}

public enum FixtureCorpus {
    /// `<name>` is a path when it holds a "/", else `<name>.json` in $WHOOSH_FIXTURES_DIR, else beside
    /// the fixture the app was launched with.
    public static func locate(_ name: String) -> URL? {
        let fm = FileManager.default
        if name.contains("/") { return fm.fileExists(atPath: name) ? URL(fileURLWithPath: name) : nil }
        var dirs: [URL] = []
        if let d = ProcessInfo.processInfo.environment["WHOOSH_FIXTURES_DIR"], !d.isEmpty { dirs.append(URL(fileURLWithPath: d)) }
        if let f = FixtureFile.locate() { dirs.append(f.deletingLastPathComponent()) }
        return dirs.map { $0.appendingPathComponent(name + ".json") }.first { fm.fileExists(atPath: $0.path) }
    }
}

/// The contracts lane's FixtureCore.loadCorpus(from:) serves `fixture corpus` (needs lane/contracts merged).
extension FixtureCore: CorpusLoading {}
