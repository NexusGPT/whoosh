import Foundation

/// The check's seam for delta 24 until the socket verb `fixture reply-folder-gone <account> <folder> [back]` exists (lane probes-r1):
/// `mac-settings-replies.fixture.folder-gone.<account>.<i>` and `.folder-back.<account>.<i>` call the fixture core's own entry for the i-th
/// folder of the account's card, then the pane reads the core again. Fixture core only (a real core answers false: "did nothing").
@MainActor
enum RepliesGoneSeam {
    static func add(_ out: inout [String: (AppModel) -> Bool], _ s: RepliesStore) {
        for a in s.sources.accounts {
            for (i, f) in a.folders.enumerated() {
                guard case .mailFolder(let account, let folder) = f.source else { continue }
                for (word, gone) in [("folder-gone", true), ("folder-back", false)] {
                    out[RepliesAx.id("fixture", word, a.accountId, "\(i)")] = { m in
                        guard let core = m.host.fixture else { return false }
                        do { try core.fixtureReplyFolderGone(account: account, folder: folder, gone: gone) } catch { return false }
                        s.load()
                        return true
                    }
                }
            }
        }
    }
}
