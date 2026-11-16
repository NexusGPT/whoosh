import Foundation

/// The design fixture (docs/design/fixtures/mac-main-sample.json) as it crosses into the app.
/// External input: decoded, never cast; a missing optional field takes its documented default.
struct FixtureFile: Decodable {
    struct Account: Decodable { let id: String; let address: String; let name: String; let provider: String; let colour: String; let unread: Int?; let group: String?; let paused: Bool?; let icon_id: String?; let backfill: Double?
        /// the account's signature (contracts delta 15): two lines with its display name; absent = none
        var signature: String? = nil }
    struct Tab: Decodable { let id: String; let name: String; let unread: Int; let waiting: Int }
    struct Mailbox: Decodable { let name: String; let count: Int? }
    struct Message: Decodable {
        let id: String; let sender: String; let from: String; let account: String; let subject: String
        let preview: String; let time: String; let unread: Bool; let pinned: Bool
        let attachments: [String]?; let thread_count: Int?; let trackers_blocked: Int?; let tab: String?
        /// body paragraphs and, for a reply, the quoted paragraphs (root delta 3)
        let body: [String]?; let quoted: [String]?
        /// the List-Unsubscribe URLs and whether List-Unsubscribe-Post offered one-click (delta 10); absent = not a list message
        var list_unsubscribe: [String]? = nil
        var list_unsubscribe_one_click: Bool? = nil
        /// an absolute local time "2026-04-16T09:12:00" (a corpus row); base rows carry only `time` ("09:12", "Yesterday", "Thu")
        var date: String? = nil
        /// the corpus row's thread id (a search corpus groups messages into threads; base rows have none)
        var thread: String? = nil
        /// the sign-in code the message carries, if any (delta 6)
        var login_code: String? = nil
        /// who placed the message in its tab ("Laya", "Owner", a rule name); absent = no category yet; "Laya", "Jev", "Owner", or "Rule: <id>"
        var decided_by: String? = nil
        /// delta 9: the other recipients of the message, for Reply all (absent = the owner only)
        var to: [String]? = nil
        var cc: [String]? = nil
        /// Bcc recipients: only on the owner's own sent copies (contracts delta 12); absent = none
        var bcc: [String]? = nil
        /// the owner sent it (contracts delta 15): a sent reply listed in its thread, never an inbox row
        var outgoing: Bool? = nil
    }
    struct ThreadMessage: Decodable { let id: String?; let from: String; let time: String?; let attachments: [String]?; var agent_summary: String? = nil }
    /// A label's fixed rules in the fixture (delta 9): `match` any|all and `conditions` in the store's JSON shape ({"kind":"from","words":"x"}, ...)
    struct RuleNode: Decodable { let match: String?; let conditions: [ConditionNode] }
    struct ConditionNode: Decodable { let kind: String; let words: String?; let domain: String?; let account_id: String?; var fact: String? = nil; var wanted: Bool? = nil; var field: String? = nil; var value: String? = nil }
    struct LabelNode: Decodable {
        let name: String; let unread: Int?; let children: [LabelNode]?
        /// delta 9: the meaning, colour, pin, the one account it belongs to (absent = every mailbox) and its fixed rules; all optional
        var colour: String? = nil; var description: String? = nil; var pinned: Bool? = nil; var account: String? = nil; var rules: RuleNode? = nil
    }
    struct SmartFolderNode: Decodable { let name: String; let rule: String }
    struct QuickReply: Decodable { let from: String; let signature: String }
    struct OpenThread: Decodable { let id: String; let subject: String; let account: String; let participants: [String]; let messages: [ThreadMessage]?; let quick_reply: QuickReply? }
    /// delta 17: a row that exercises one card of the À répondre flow. `step` is "entry" (the entry rule catches it), "kept" (the AI picks the label and the
    /// exit filter keeps it: unanswered) or "rejected" (the AI picks it, the owner already replied, the exit filter sends it to the fallback).
    struct FlowExampleNode: Decodable {
        let id: String; let account: String; let from: String; let sender: String; let subject: String; let body: String
        let step: String; let attachments: [String]?
    }
    let accounts: [Account]
    let tabs: [Tab]
    let mailboxes: [Mailbox]
    let messages: [Message]
    let open_thread: OpenThread?
    let focused_message: String?
    let labels: [LabelNode]?
    let smart_folders: [SmartFolderNode]?
    /// delta 17: the rows that exercise every card of the À répondre label (absent = none); served as inserted rows, outside the tab figures
    let flow_example: [FlowExampleNode]?
    /// people whose addresses are all known up front (delta 8, the search dropdown's People rows); absent = none declared
    struct ContactNode: Decodable { let name: String; let addresses: [String] }
    let contacts: [ContactNode]?

    static func load(from url: URL) throws -> FixtureFile {
        try JSONDecoder().decode(FixtureFile.self, from: Data(contentsOf: url))
    }

    /// Where the fixture is: $WHOOSH_FIXTURE_PATH, else the copy bundled in the app.
    static func locate() -> URL? {
        if let p = ProcessInfo.processInfo.environment["WHOOSH_FIXTURE_PATH"], !p.isEmpty { return URL(fileURLWithPath: p) }
        return Bundle.main.url(forResource: "mac-main-sample", withExtension: "json")
    }
}

extension MailProvider {
    /// The fixture writes providers as words ("Google", "Microsoft", "iCloud", "Fastmail", ...).
    init(words: String) {
        switch words.lowercased() {
        case let w where w.contains("google") || w.contains("gmail"): self = .google
        case let w where w.contains("microsoft") || w.contains("outlook"): self = .microsoft
        case let w where w.contains("icloud"): self = .icloud
        case let w where w.contains("fastmail"): self = .fastmail
        default: self = .imap
        }
    }
}

/// One attachment of the corpus: its message, kind, state (indexed | not-downloaded | unreadable) and the text its reading yields.
struct AttachmentCorpusEntry: Decodable {
    let message: String; let kind: String; let state: String; let text: String
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        message = try c.decode(String.self, forKey: .message); kind = try c.decode(String.self, forKey: .kind)
        state = try c.decode(String.self, forKey: .state); text = try c.decode(String.self, forKey: .text)
    }
    private enum Keys: String, CodingKey { case message, kind, state, text }
}

/// An `attachment_content` value that is not an attachment (the map's "note" string) reads as nil.
struct LossyAttachment: Decodable {
    let entry: AttachmentCorpusEntry?
    init(from decoder: Decoder) throws { entry = try? AttachmentCorpusEntry(from: decoder) }
}

/// A second corpus a check loads over the fixture (`fixture corpus <path>`, e.g. fixtures/tests-j2.json,
/// schema "whoosh-search-corpus/1"): its messages replace the fixture's; accounts, tabs and labels stay.
struct CorpusFile: Decodable {
    struct Row: Decodable {
        let id: String; let thread: String?; let account: String; let from_name: String; let from: String
        let date: String; let subject: String; let body: String; let attachments: [String]?
    }
    let schema: String
    let messages: [Row]
    /// the corpus's own label tree and smart folders (delta 8); absent = none
    let labels: [FixtureFile.LabelNode]?
    let smart_folders: [FixtureFile.SmartFolderNode]?
    /// the corpus's pinned labels (built-in ids or label paths), authoritative when present: those labels are the tabs, in tree order (delta 9)
    var pinned_labels: [String]? = nil
    /// what the on-device reading returns per attachment file (fixtures/tests-j2.json `attachment_content`); the "note" key is skipped
    var attachment_content: [String: LossyAttachment]? = nil

    var asMessages: [FixtureFile.Message] {
        var perThread: [String: Int] = [:]
        for r in messages { if let t = r.thread { perThread[t, default: 0] += 1 } }
        return messages.map { r in
            FixtureFile.Message(id: r.id, sender: r.from_name, from: r.from, account: r.account, subject: r.subject,
                                preview: String(r.body.prefix(160)), time: "", unread: false, pinned: false,
                                attachments: r.attachments, thread_count: r.thread.flatMap { perThread[$0] } ?? 1,
                                trackers_blocked: 0, tab: "primary", body: [r.body], quoted: nil, date: r.date, thread: r.thread)
        }
    }
}
