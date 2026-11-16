import Foundation
import Testing
@testable import WhooshUI

// Labels, smart folders, contacts (Mac), lane labelscontactsmac. The corpus tests pass today and prove the sender-card
// expectation is computed, not copied. The registration tests fail today: the feature is a stub.
// HANDBACK to the lane: no pure symbol exists for "which messages belong on a sender card" or "how many threads a rule matches";
// expose them (a function over [message summaries] and the sender address) and drive them with these corpus cases.

private struct Msg: Decodable { let id: String, thread: String, account: String, from: String, date: String }
private struct Corpus: Decodable {
    let messages: [Msg]
    let expect: Expect
    struct Expect: Decodable {
        let sender_address: String
        let sender_messages_newest_first: [String]
        let sender_accounts: [String]
    }
}

private func corpus() throws -> Corpus {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    return try JSONDecoder().decode(Corpus.self, from: Data(contentsOf: root.appendingPathComponent("fixtures/tests-j2.json")))
}

@Test func senderCardIsTheSendersMailNewestFirstWithItsAccounts() throws {
    let c = try corpus()
    let card = c.messages.filter { $0.from == c.expect.sender_address }.sorted { $0.date > $1.date }
    #expect(card.map(\.id) == c.expect.sender_messages_newest_first)
    #expect(card.map(\.account) == c.expect.sender_accounts)
}

@Test func senderCardSpansMoreThanOneAccount() throws {
    // positive control for any "card lists every account" check: the corpus really holds two accounts for the sender
    let c = try corpus()
    #expect(Set(c.expect.sender_accounts).count >= 2)
}

@Test func smartFolderNestingIsPartOfTheContractType() {
    let flat = SmartFolder(id: "sf-1", name: "Supplier quotes", ruleWords: "from Linea Supply", parentId: nil)
    let nested = SmartFolder(id: "sf-1", name: "Supplier quotes", ruleWords: "from Linea Supply", parentId: "label-work")
    #expect(flat != nested)
    #expect(nested.parentId == "label-work")
}

@MainActor @Test func labelsContactsScreenIsRegistered() {
    #expect(LabelsContactsMacFeature.surfaces == ["mac-labels-contacts"])
}

@MainActor @Test func labelsContactsRegionsServeRows() {
    let rows = LabelsContactsMacFeature.rowSources()
    for ax in ["mac-labels-contacts.sidebar", "mac-labels-contacts.rule.matches", "mac-labels-contacts.contact.messages", "mac-labels-contacts.contact.header"] {
        #expect(rows[ax] != nil, "no row source for \(ax)")
    }
}

@MainActor @Test func labelsContactsClicksAreRegistered() {
    let clicks = LabelsContactsMacFeature.clickTargets()
    for ax in ["mac-labels-contacts.parent.work", "mac-labels-contacts.contact.merge"] {
        #expect(clicks[ax] != nil, "no click target for \(ax)")
    }
    // swift-tests lane: `mac-main.reader.sender` (the click that opens the sender card) is registered by the READER, not by this lane (Sources/Features/Reader/ReaderFeature.swift:40,
    // ReaderActions.openSender), so it is asked of the registry, not of LabelsContactsMacFeature.clickTargets().
    #expect(Registrations.click("mac-main.reader.sender") != nil, "no click target for mac-main.reader.sender")
}
