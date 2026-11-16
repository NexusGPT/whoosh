import Foundation

/// One field-and-operator a fixed rule can test. The title is what the pop-up shows ("Domain is"); `tag` is the short word a
/// matched row carries in pane 2. Both are the contract's own condition kinds: nothing here invents a field the core lacks.
/// A row's condition word. Only the forms the core stores and matches are cases.
enum RuleOp: String, CaseIterable, Identifiable, Sendable {
    case isExactly = "is", contains, isNot = "is_not", doesNotContain = "does_not_contain"
    var id: String { rawValue }
    var title: String {
        switch self {
        case .isExactly: Words.text("is")
        case .contains: Words.text("contains")
        case .isNot: Words.text("is not")
        case .doesNotContain: Words.text("does not contain")
        }
    }
    /// The negative forms: the core's `RuleCondition.not` twin of the field's own condition.
    var negated: Bool { self == .isNot || self == .doesNotContain }
}

enum RuleKind: String, CaseIterable, Identifiable, Sendable {
    case domain, from, subject, body, account, to, cc, listHeader
    var id: String { rawValue }
    var title: String {
        switch self {
        case .domain: Words.text("Domain is")
        case .from: Words.text("From contains")
        case .subject: Words.text("Subject contains")
        case .body: Words.text("Body contains")
        case .account: Words.text("Account is")
        case .to: Words.text("To contains")
        case .cc: Words.text("Cc contains")
        case .listHeader: Words.text("List header contains")
        }
    }
    /// The FIELD a row tests, as its own menu shows it (owner, 13 Nov 07:30: "the actual rule (e.g., Domain) and condition (is/is not) separately").
    var fieldTitle: String {
        switch self {
        case .domain: Words.text("Domain")
        case .from: Words.text("From")
        case .subject: Words.text("Subject")
        case .body: Words.text("Body")
        case .account: Words.text("Account")
        case .to: Words.text("To")
        case .cc: Words.text("Cc")
        case .listHeader: Words.text("List header")
        }
    }
    /// The conditions this field offers, each a form the core's rule records and matcher really have today: a domain and an account are exact ("is"), the text
    /// fields look inside the text ("contains"). The negative forms ("is not", "does not contain") are not offered until the contract carries them (docs/lanes/hid-labels.md, Round 17).
    var operators: [RuleOp] { self == .domain || self == .account ? [.isExactly, .isNot] : [.contains, .doesNotContain] }
    /// The condition word of this field, affirmative or negated.
    func op(negated: Bool) -> RuleOp { operators[negated ? 1 : 0] }
    var op: RuleOp { operators[0] }
    /// The core's field of this kind, for its negated twin.
    var ruleField: RuleField {
        switch self {
        case .domain: .domain
        case .from: .from
        case .subject: .subjectContains
        case .body: .bodyContains
        case .account: .account
        case .to: .to
        case .cc: .cc
        case .listHeader: .listHeader
        }
    }
    init(_ f: RuleField) {
        switch f {
        case .domain: self = .domain
        case .from: self = .from
        case .subjectContains: self = .subject
        case .bodyContains: self = .body
        case .account: self = .account
        case .to: self = .to
        case .cc: self = .cc
        case .listHeader: self = .listHeader
        }
    }
    /// A stored negated condition's field word (the positive kind's own word).
    static func field(key: String) -> RuleField? {
        switch key {
        case "domain": .domain
        case "from": .from
        case "subject": .subjectContains
        case "body": .bodyContains
        case "account": .account
        case "to": .to
        case "cc": .cc
        case "list_header": .listHeader
        default: nil
        }
    }
    func condition(_ value: String, negated: Bool) -> RuleCondition { negated ? .not(field: ruleField, value: value) : condition(value) }
    var tag: String {
        switch self {
        case .domain: Words.text("Domain")
        case .from: Words.text("From")
        case .subject: Words.text("Subject")
        case .body: Words.text("Body")
        case .account: Words.text("Account")
        case .to: Words.text("To")
        case .cc: Words.text("Cc")
        case .listHeader: Words.text("List")
        }
    }
    var placeholder: String {
        switch self {
        case .domain: Words.text("example.com")
        case .account: Words.text("Pick an account")
        default: Words.text("a word or an address")
        }
    }
    func condition(_ value: String) -> RuleCondition {
        switch self {
        case .domain: .domain(domain: value)
        case .from: .from(words: value)
        case .subject: .subjectContains(words: value)
        case .body: .bodyContains(words: value)
        case .account: .account(accountId: value)
        case .to: .to(words: value)
        case .cc: .cc(words: value)
        case .listHeader: .listHeader(words: value)
        }
    }
    /// The text kinds only: a fact (and the old attachment condition, one fact of the grid) is a cell of the grid, never a row.
    init?(_ c: RuleCondition) {
        switch c {
        case .domain: self = .domain
        case .from: self = .from
        case .subjectContains: self = .subject
        case .bodyContains: self = .body
        case .account: self = .account
        case .to: self = .to
        case .cc: self = .cc
        case .listHeader: self = .listHeader
        case .not(let field, _): self = RuleKind(field)
        case .hasAttachment, .fact: return nil
        }
    }
    /// A row's condition read back: the kind and whether it is the negated twin.
    static func isNegated(_ c: RuleCondition) -> Bool { if case .not = c { return true }; return false }
    static func value(of c: RuleCondition) -> String {
        switch c {
        case .domain(let v), .from(let v), .subjectContains(let v), .bodyContains(let v), .to(let v), .cc(let v), .listHeader(let v): v
        case .account(let v): v
        case .not(_, let v): v
        case .hasAttachment, .fact: ""
        }
    }
}

/// One rule row of the inspector: a kind and the values it tests, any of them. `pending` is the text still being typed:
/// it counts at once (the live count follows each keystroke) and turns into a value on Return.
struct RuleDraft: Identifiable, Equatable, Sendable {
    var id = UUID()
    var kind: RuleKind
    /// the condition word of the row: one of the two forms its field offers (`RuleKind.operators`: the affirmative one, or its negated twin); a pick outside them changes nothing
    var negated = false
    var op: RuleOp {
        get { kind.op(negated: negated) }
        set { if kind.operators.contains(newValue) { negated = newValue.negated } }
    }
    var values: [String] = []
    var pending = ""
    var expanded = false

    var allValues: [String] {
        let p = pending.trimmingCharacters(in: .whitespacesAndNewlines)
        return values.filter { !$0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty } + (p.isEmpty ? [] : [p])
    }
    /// A stored value that is blank: it would match every message in the core's `has`, so it is never a condition (it matches nothing) and the row says so.
    var hasBlankValue: Bool { values.contains { $0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty } }
    var conditions: [RuleCondition] {
        allValues.map { kind.condition($0, negated: negated) }
    }
    /// The row's own rule set: any of its values.
    /// (a negated row holds for a message that avoids every value, so its values combine with AND)
    var ruleSet: RuleSet { RuleSet(conditions: conditions, matchMode: negated ? .all : .any) }
    var isEmpty: Bool { allValues.isEmpty }

    /// Changes the field: the values stay where they still fit (text to text); to or from Account they go, since an account's values are ids. The condition follows the field.
    mutating func setField(_ k: RuleKind) {
        if (kind == .account) != (k == .account) { values = []; pending = "" }
        kind = k
    }

    /// What counts as the same rule when the draft is compared with the saved one (the id is a view concern).
    var signature: [String] { [kind.rawValue + (negated ? "!" : "")] + allValues }
}

/// The label as the owner is editing it: the five fields, chain 0 of the Entry card (`entry`, the one `LabelSpec.rules` carries,
/// its rows and its mode forwarded as `rules` and `matchMode`) and the three cards' own state (`flow`, LabelFlowDraft.swift).
/// `spec` is the one write the core takes for the fields; the flow goes through the delta 17 commands (LabelFlowWrites.swift).
struct LabelDraft: Equatable, Sendable {
    var name = ""
    var parentId: String?
    var colour: String?
    var scope: LabelScope = .all
    var pinned = false
    var meaning = ""
    var entry = ChainDraft()
    var flow = FlowCards()

    var rules: [RuleDraft] { get { entry.rows } set { entry.rows = newValue } }
    var matchMode: MatchMode { get { entry.matchMode } set { entry.matchMode = newValue } }
    var ruleSet: RuleSet { entry.ruleSet }
    var spec: LabelSpec {
        LabelSpec(name: name.trimmingCharacters(in: .whitespacesAndNewlines), parentId: parentId, colour: colour,
                  description: meaning.trimmingCharacters(in: .whitespacesAndNewlines), scope: scope, rules: ruleSet, pinned: pinned)
    }

    init() {}

    /// A label the core holds: its fields, its chains (entry first, then the others, then exit) and its cards.
    init(saved l: MailLabel, entry chains: [DecodedChain], exit exits: [DecodedChain]) {
        name = l.name; parentId = l.parentId; colour = l.colour; scope = l.scope; pinned = l.pinned; meaning = l.description
        entry = chains.first.map(ChainDraft.init) ?? ChainDraft(rows: [])
        flow = FlowCards(saved: l, others: Array(chains.dropFirst()), exits: exits)
    }

    /// What differs from the saved draft: a changed field is one change, a changed rule row, fact cell, switch, tick or chain is one change.
    func changes(from saved: LabelDraft) -> Int {
        var n = 0
        if name != saved.name { n += 1 }
        if parentId != saved.parentId { n += 1 }
        if colour != saved.colour { n += 1 }
        if scope != saved.scope { n += 1 }
        if pinned != saved.pinned { n += 1 }
        if meaning != saved.meaning { n += 1 }
        return n + entry.changes(from: saved.entry) + flow.changes(from: saved.flow)
    }

    /// Meaning, rules or any card moved: the filed mail may no longer fit, so the Re-run question is worth asking.
    func filingMoved(from saved: LabelDraft) -> Bool {
        meaning != saved.meaning || entry.signature != saved.entry.signature || flow != saved.flow
    }
}

/// The six colours the board offers; "none" is nil. They are data the owner chooses between, stored as the contract's
/// `colour` ("#RRGGBB"). Requested as a Sources/Design token (docs/lanes/labelscontactsmac.md, Round 2).
enum LabelPalette {
    static let swatches: [String] = [0xB7791F, 0x2F6FED, 0x2B8A3E, 0x9C36B5, 0xE03131, 0x0B7285].map { String(format: "#%06X", $0) }
}
