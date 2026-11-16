import Foundation
import Testing
@testable import WhooshUI

// Item 1b64473f (send attachments with a message), the composer half: a file attached by hand in the composer (ComposerModel.attach, the
// path of a drop, a paste and the file picker) is on the core's draft, and the Send puts it on the queued message the core sends.
// The composer hands its files to the send in two steps: ComposerModel+Files.swift settleFiles() -> .addDraftAttachment(draftId:source:)
// (:73-80 addSource), then ComposerModel+Send.swift submit() -> settleFiles(), saveOnCoreDraft(), .send(draftId:) (:64-67).
// The core's own half (the bytes in the SMTP message) is engine_files_r6 / attachments_d22 under cargo.
// Run with WHOOSH_FIXTURE_CORE=1 (the fixture moves a draft's files to the outbox entry at the send, as the real core does).
// MUTANTS (one per test, the one-line broken copy that must turn it red):
//   aFileAttachedByHandIsOnTheQueuedMessage : ComposerModel+Send.swift submit(): `settleFiles()` removed (the files still being added are not handed over),
//                                             or ComposerModel+Files.swift settleFiles(): `try addSource(.localFile(path: url.path))` replaced by `_ = url`
//   aFileTakenOffBeforeTheSendIsNotSent     : ComposerModel+Files.swift removeAttachment(): `pendingRemovals.append(a.id)` removed

@MainActor private func model() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    setenv("WHOOSH_FIXTURE_PATH", repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()   // launch state once per test (Tests/Support/SharedAppFresh.swift)
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    return m
}

private func tempFile(_ name: String, _ text: String) throws -> URL {
    let dir = FileManager.default.temporaryDirectory.appendingPathComponent("whoosh-replies-files-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    let url = dir.appendingPathComponent(name)
    try Data(text.utf8).write(to: url)
    return url
}

private struct Carried: Decodable {
    struct Row: Decodable { struct Source: Decodable { let kind: String; let path: String? }; let fileName: String; let sizeBytes: UInt64; let source: Source }
    let attachments: [Row]
}
@MainActor private func carried(_ m: AppModel, outbox id: String) throws -> [Carried.Row] {
    let raw = try #require(m.core.state(path: "outbox/\(id)/attachments"), "the core serves no outbox/\(id)/attachments")
    let d = JSONDecoder(); d.keyDecodingStrategy = .convertFromSnakeCase
    return try d.decode(Carried.self, from: Data(raw.utf8)).attachments
}

@MainActor private func openMessage(_ m: AppModel) throws -> ComposerModel {
    let c = ComposerModel.shared
    c.discardDraft()
    try #require(c.openNew(m), "a composer opens on the fixture")
    c.to = ["someone@example.com"]
    c.subject = "With a file"
    c.session.setPlain("The file is attached.")
    return c
}
@MainActor private func finish(_ c: ComposerModel, _ m: AppModel) {
    c.dropCoreDraft(); c.discardDraft(); _ = c.close(m)
}

@MainActor @Test func aFileAttachedByHandIsOnTheQueuedMessage() throws {
    let m = try model()
    let c = try openMessage(m)
    defer { finish(c, m) }
    let url = try tempFile("budget.txt", "q3 numbers")
    #expect(c.attach(url), "the composer takes the file")
    #expect(c.send(m))
    guard case .counting(let outboxId, _) = c.phase else { Issue.record("the send did not queue the message: \(c.phase) / \(c.line)"); return }
    let rows = try carried(m, outbox: outboxId)
    #expect(rows.count == 1, "the queued message carries \(rows.count) files")
    #expect(rows.first?.fileName == "budget.txt")
    #expect(rows.first?.sizeBytes == UInt64("q3 numbers".utf8.count))
    #expect(rows.first?.source.kind == "local_file")
    #expect(rows.first?.source.path == url.path, "the file the core holds is the one the owner attached")
}

@MainActor @Test func aFileTakenOffBeforeTheSendIsNotSent() throws {
    let m = try model()
    let c = try openMessage(m)
    defer { finish(c, m) }
    let keep = try tempFile("keep.txt", "keep"), drop = try tempFile("drop.txt", "drop")
    #expect(c.attach(keep)); #expect(c.attach(drop))
    c.syncFiles()
    let tile = try #require(c.attachments.first { $0.name == "drop.txt" }, "a tile for drop.txt")
    c.removeAttachment(tile)
    #expect(c.send(m))
    guard case .counting(let outboxId, _) = c.phase else { Issue.record("the send did not queue the message: \(c.phase) / \(c.line)"); return }
    let names = try carried(m, outbox: outboxId).map(\.fileName)
    #expect(names == ["keep.txt"], "the queued message carries \(names)")
}
