import Foundation
import Testing
@testable import WhooshUI

/// Probes for root delta 4: real received times, per-account icons, a second corpus.
private let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func core() throws -> FixtureCore {
    try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
}
private func primary(_ c: FixtureCore) -> [InboxRow] { c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)) }

@Test func everyRowHasARealTimeNewestFirst() throws {
    let c = try core()
    let rows = primary(c).filter { !$0.pinned }
    #expect(rows.allSatisfy { $0.receivedAtMs > 0 })
    #expect(zip(rows, rows.dropFirst()).allSatisfy { $0.receivedAtMs >= $1.receivedAtMs }, "\(rows.map(\.receivedAtMs))")
    let now = Int64(Date().timeIntervalSince1970 * 1000)
    #expect(rows.allSatisfy { now - $0.receivedAtMs < 8 * 24 * 3600 * 1000 })
}

@Test func aClockTimeIsTodayAndYesterdayIsYesterday() throws {
    let cal = Calendar.current
    let now = Int64(Date().timeIntervalSince1970 * 1000)
    func msg(_ t: String) -> FixtureFile.Message {
        FixtureFile.Message(id: "x", sender: "", from: "", account: "acc-01", subject: "", preview: "", time: t, unread: false, pinned: false,
                            attachments: nil, thread_count: nil, trackers_blocked: nil, tab: nil, body: nil, quoted: nil)
    }
    let d = { (ms: Int64?) in Date(timeIntervalSince1970: Double(ms ?? 0) / 1000) }
    let today = d(FixtureCore.receivedAtMs(msg("09:12"), sameLabelIndex: 0, now: now))
    #expect(cal.isDateInToday(today) && cal.component(.hour, from: today) == 9 && cal.component(.minute, from: today) == 12)
    #expect(cal.isDateInYesterday(d(FixtureCore.receivedAtMs(msg("Yesterday"), sameLabelIndex: 0, now: now))))
    let thu = d(FixtureCore.receivedAtMs(msg("Thu"), sameLabelIndex: 0, now: now))
    #expect(cal.component(.weekday, from: thu) == 5 && !cal.isDateInToday(thu))
    #expect(FixtureCore.receivedAtMs(msg("someday"), sameLabelIndex: 0, now: now) == nil)
}

@Test func accountsCarryTheBoardsIconPicks() throws {
    let c = try core()
    let a = c.accounts()
    #expect(a.count == 30)
    #expect(a.allSatisfy { $0.iconId != nil })
    #expect(a.first { $0.id == "acc-18" }?.health?.accountName == "Provanta RFP")
    #expect(a.first { $0.id == "acc-01" }?.health == nil)
    #expect(a.first { $0.id == "acc-18" }?.iconId == "phone")
    #expect(Set(a.map(\.colour)).count == 30)
}

@Test func aCorpusReplacesTheRows() throws {
    let c = try core()
    try c.loadCorpus(from: root.appendingPathComponent("fixtures/tests-j2.json"))
    let rows = primary(c)
    // as many rows as the corpus file holds (tests-j2 grows; read it, never a typed number)
    let corpus = try JSONSerialization.jsonObject(with: Data(contentsOf: root.appendingPathComponent("fixtures/tests-j2.json"))) as? [String: Any]
    #expect(rows.count == (corpus?["messages"] as? [Any])?.count)
    #expect(rows.first { $0.messageId == "msg-01" }?.receivedAtMs == Int64(ISO8601DateFormatter.localNoon("2026-04-16T09:12:00")))
    #expect(c.accounts().count == 30)
    #expect(throws: CoreError.self) { try c.loadCorpus(from: root.appendingPathComponent("fixtures/tests-j1.json")) }
}

private extension ISO8601DateFormatter {
    static func localNoon(_ s: String) -> Double {
        let f = DateFormatter(); f.locale = Locale(identifier: "en_US_POSIX"); f.timeZone = .current; f.dateFormat = "yyyy-MM-dd'T'HH:mm:ss"
        return (f.date(from: s)?.timeIntervalSince1970 ?? 0) * 1000
    }
}
