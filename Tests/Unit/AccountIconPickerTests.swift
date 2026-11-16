import AppKit
import Foundation
import Testing
@testable import WhooshUI

// Account icon picker (item 5b704a2d): a pick reaches the list row, the sidebar ring and the reader's line in the same update,
// and an uploaded picture is kept as the account's icon. Run with WHOOSH_FIXTURE_CORE=1 (the shared AppModel serves the design fixture).
// MUTANTS: each test names, in its trailing comment, the one-line break of the target that must turn it red.

@MainActor private func model() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    setenv("WHOOSH_FIXTURE_PATH", repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()   // launch state once per test (Tests/Support/SharedAppFresh.swift)
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    AccountsSession.shared.reload()
    return m
}

/// The three places that draw an account's mark, read the way each reads it: the list row's directory (the cached one the list and
/// its probe use), the sidebar's account (SidebarData.accounts) and the reader's line (RowAccount.of, ReaderPane.swift:29).
@MainActor private func marks(_ id: String, _ m: AppModel) throws -> (list: RowAccount, sidebar: Account, reader: RowAccount) {
    let list = try #require(RowAccount.cachedDirectory(m)[id], "the list directory has no \(id)")
    let side = try #require(SidebarData.accounts().first { $0.id == id }, "the sidebar has no \(id)").account
    return (list, side, RowAccount.of(id, in: m))
}

/// A 200 x 120 PNG made in the test (the repo holds no logo file), so the store has a non-square picture to cut.
private func logoPNG() throws -> Data {
    let image = NSImage(size: NSSize(width: 200, height: 120), flipped: false) { r in NSColor.systemOrange.setFill(); r.fill(); return true }
    let tiff = try #require(image.tiffRepresentation)
    return try #require(NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:]))
}

@MainActor @Test func aLibraryPickShowsOnTheListRowTheSidebarAndTheReaderInTheSameUpdate() throws {
    let m = try model()
    let id = "acc-02"
    let before = try marks(id, m)                       // primes the list's cached directory with the old icon
    #expect(before.list.iconId != "bolt")
    #expect(AccountsSession.shared.pickGlyph("bolt", for: id))
    let after = try marks(id, m)                        // no wait, no second event: the very next read
    #expect(after.list.iconId == "bolt", "the list row still says \(after.list.iconId ?? "nil")")     // mutant: AccountsSession.run no longer bumps AccountMarkRevision
    #expect(after.sidebar.iconId == "bolt")
    #expect(after.reader.iconId == "bolt")
    #expect(m.core.state(path: "account_icon/\(id)") == "\"bolt\"")
}

@MainActor @Test func aColourPickShowsOnTheListRowTheSidebarAndTheReaderInTheSameUpdate() throws {
    let m = try model()
    let id = "acc-02"
    let before = try marks(id, m)
    let hex = "#2B8A3E"
    #expect(before.list.colour != hex)
    #expect(AccountsSession.shared.pickColour(hex, for: id))
    let after = try marks(id, m)
    #expect(after.list.colour == hex, "the list row still says \(after.list.colour)")                  // mutant: AccountsSession.run no longer bumps AccountMarkRevision
    #expect(after.sidebar.colour == hex)
    #expect(after.reader.colour == hex)
}

@MainActor @Test func theListDirectoryIsKeptWhileNothingAboutAMarkChanged() throws {
    let m = try model()
    _ = RowAccount.cachedDirectory(m)
    let rev = AccountMarkRevision.shared.value
    _ = RowAccount.cachedDirectory(m)
    #expect(AccountMarkRevision.shared.value == rev, "a read must not move the revision")
    AccountsSession.shared.pickGlyph("bolt", for: "acc-02")
    #expect(AccountMarkRevision.shared.value == rev + 1, "one write is one revision")                  // mutant: bump() adds 2
}

@Test func anUploadedPictureIsKeptAsTheAccountsIconFile() throws {
    let dir = FileManager.default.temporaryDirectory.appendingPathComponent("whoosh-icon-test-\(UUID().uuidString)")
    setenv("WHOOSH_ICON_DIR", dir.path, 1)
    defer { unsetenv("WHOOSH_ICON_DIR"); try? FileManager.default.removeItem(at: dir) }
    let path: String
    switch AccountIconStore.store(try logoPNG(), accountId: "acc-01") {
    case .success(let p): path = p
    case .failure(let e): Issue.record("the store refused a small PNG: \(e.words)"); return
    }
    #expect(path == dir.appendingPathComponent("acc-01.png").path, "the picture is kept at <folder>/<id>.png, got \(path)")    // mutant: the file name drops the account id
    #expect(FileManager.default.fileExists(atPath: path))
    let rep = try #require(NSBitmapImageRep(data: try Data(contentsOf: URL(fileURLWithPath: path))))
    let px = WhooshConstants.ACCOUNT_ICON_PX * 2
    #expect(rep.pixelsWide == px && rep.pixelsHigh == px, "a square of \(px) px, got \(rep.pixelsWide) x \(rep.pixelsHigh)")   // mutant: the crop is skipped (size 1x)
}

@MainActor @Test func anUploadedPictureEndsAsTheAccountsIconReferenceInTheCore() throws {
    let m = try model()
    let dir = FileManager.default.temporaryDirectory.appendingPathComponent("whoosh-icon-test-\(UUID().uuidString)")
    setenv("WHOOSH_ICON_DIR", dir.path, 1)
    defer { unsetenv("WHOOSH_ICON_DIR"); try? FileManager.default.removeItem(at: dir) }
    let path: String
    switch AccountIconStore.store(try logoPNG(), accountId: "acc-01") {
    case .success(let p): path = p
    case .failure(let e): Issue.record("the store refused a small PNG: \(e.words)"); return
    }
    AccountsSession.shared.setImage(path: path, for: "acc-01")
    #expect(AccountsSession.shared.refusal("acc-01") == nil)
    #expect(m.core.state(path: "account_icon/acc-01") == "\"uploaded\"", "the core does not hold the upload as the icon")   // mutant: FixtureSettings.iconWord serves the library glyph under an upload
    let file = m.core.accounts().first { $0.id == "acc-01" }?.iconFile
    #expect(file == path, "the account's icon file is \(file ?? "nil")")                                                      // mutant: FixtureSettings.iconFile returns nil
    #expect(AccountsSession.shared.entry("acc-01")?.imagePath == path)
}
