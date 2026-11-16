#if os(macOS)
import Foundation

/// Where attachment files live on disk. Under the rig (WHOOSH_SOCKET_DIR set) every write lands in the rig's run
/// directory, so a Quick Look, open or save test never touches the owner's files or Downloads folder.
public enum AttachmentPlaces {
    static var env: [String: String] { ProcessInfo.processInfo.environment }
    /// The rig's run directory, or nil on the owner's copy.
    public static var rigDir: URL? { env["WHOOSH_SOCKET_DIR"].flatMap { $0.isEmpty ? nil : URL(fileURLWithPath: $0) } }
    public static var isRig: Bool { rigDir != nil }

    /// Pasted and dropped pictures written to a file (lead ruling, fix-realcore finding): the engine reads a file only at send, so a send
    /// later, an offline wait or a cleanup of the system's temporary folder would lose it. They live under the app's own data folder,
    /// one folder per picture: `<data>/pasted-files/<uuid>/` (`ModelsFolder.whooshRoot`: the owner's Application Support folder, or the
    /// test copy's own run folder, never the owner's). Files picked or copied in the Finder are never copied here.
    public nonisolated static var pastedRoot: URL { ModelsFolder.whooshRoot().appendingPathComponent("pasted-files", isDirectory: true) }

    /// A new folder for one pasted picture; nil when it cannot be made.
    public nonisolated static func newPastedFolder() -> URL? {
        let dir = pastedRoot.appendingPathComponent(UUID().uuidString, isDirectory: true)
        return (try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)) != nil ? dir : nil
    }

    /// True for a file this app wrote for a paste (it lies under `pastedRoot`).
    public nonisolated static func isPasted(_ url: URL) -> Bool {
        url.standardizedFileURL.path.hasPrefix(pastedRoot.standardizedFileURL.path + "/")
    }

    /// Removes the pasted ones among `urls` (their own folder with them); any other file is left where it is.
    public nonisolated static func removePasted(_ urls: [URL]) {
        if AttachmentRig.mutant("keep-pasted") { return }
        for u in urls where isPasted(u) { try? FileManager.default.removeItem(at: u.deletingLastPathComponent()) }
    }

    /// Pasted files of sends still in the outbox: removed once their entry has gone out (the engine has read them then).
    @MainActor static var afterSend: [String: [URL]] = [:]
    @MainActor static func sweepSent(_ core: WhooshCore) {
        let open = Dictionary(core.outbox().map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        for (id, urls) in afterSend {
            if let e = open[id], case .sent = e.state {} else if open[id] != nil { continue }
            removePasted(urls)
            afterSend[id] = nil
        }
    }

    /// Local copies of opened attachments: one folder per message.
    public static func cache(_ messageId: String) -> URL {
        let root = rigDir?.appendingPathComponent("attachments", isDirectory: true)
            ?? FileManager.default.temporaryDirectory.appendingPathComponent("whoosh-attachments", isDirectory: true)
        return root.appendingPathComponent(messageId, isDirectory: true)
    }

    /// The Downloads folder (the rig's own under the rig).
    public static var downloads: URL {
        if let r = rigDir { return r.appendingPathComponent("downloads", isDirectory: true) }
        return FileManager.default.urls(for: .downloadsDirectory, in: .userDomainMask).first
            ?? FileManager.default.temporaryDirectory                   // the system always names Downloads; never reached
    }

    /// The sample files the fixture core serves: WHOOSH_ATTACHMENT_FIXTURES, else the worktree the build was made from.
    public static var fixtures: URL? {
        if let d = env["WHOOSH_ATTACHMENT_FIXTURES"], !d.isEmpty { return URL(fileURLWithPath: d, isDirectory: true) }
        // the build stamp's worktree key (scripts/build.sh), matched without case
        guard let wt = Bundle.main.infoDictionary?.first(where: { $0.key.lowercased() == "nexusbuildworktree" })?.value as? String else { return nil }
        let u = URL(fileURLWithPath: wt).appendingPathComponent("docs/design/fixtures/attachments", isDirectory: true)
        return FileManager.default.fileExists(atPath: u.path) ? u : nil
    }

    /// A name not yet taken in `dir`: "quote.pdf", then "quote 2.pdf", as the Finder does.
    public static func free(_ name: String, in dir: URL) -> URL {
        let base = (name as NSString).deletingPathExtension, ext = (name as NSString).pathExtension
        var url = dir.appendingPathComponent(name), n = 2
        while FileManager.default.fileExists(atPath: url.path) {
            let numbered = [base, String(n)].joined(separator: " ")         // a file name, not words: "quote 2"
            url = dir.appendingPathComponent(ext.isEmpty ? numbered : [numbered, ext].joined(separator: "."))
            n += 1
        }
        return url
    }
}
#endif
