import Foundation

/// One entry of a decision list: a rule sentence, an "ask the model" question, or the closing default.
struct DecisionEntry: Identifiable, Equatable {
    enum Kind: String { case rule, model, `default` }
    let id: UUID
    var kind: Kind
    /// rule: the condition in words · model: the question sent · default: the outcome word
    var text: String
    var outcome: String
    var enabled = true
    var trust: Int?
    var shippedTrust: Int?
    /// the rule's match key in the sample (interim): "vip", "list", "me", "code", "reply", "from:<x>", "subject:<x>"
    var match: String?

    init(_ kind: Kind, _ text: String, outcome: String, trust: Int? = nil, match: String? = nil) {
        id = UUID(); self.kind = kind; self.text = text; self.outcome = outcome; self.trust = trust; shippedTrust = trust; self.match = match
    }
}

struct DecisionPage: Identifiable, Equatable {
    let id: String
    let title: String
    let settles: String
    var entries: [DecisionEntry]
}

/// The six pages and their shipped lists, verbatim from docs/DECISION-MODEL.md v2 §1.
/// INTERIM: the decision crate (core/crates/decision) owns these lists; until its read lands they are seeded here.
enum DecisionCatalog {
    static func shipped() -> [DecisionPage] {
        [
            DecisionPage(id: "which-tab", title: "Which tab", settles: "the split tab a message sits in", entries: [
                .init(.rule, "from a VIP", outcome: "Primary", match: "vip"),
                .init(.rule, "has a list header", outcome: "Updates", match: "list"),
                .init(.model, "Which tab?", outcome: "the tab it picks", trust: 80),
                .init(.default, "Primary", outcome: "Primary")]),
            DecisionPage(id: "needs-reply", title: "Needs my reply", settles: "the Needs Reply flag and the waiting count", entries: [
                .init(.rule, "from me", outcome: "no", match: "me"),
                .init(.rule, "has a list header", outcome: "no", match: "list"),
                .init(.model, "Does this email ask me, personally, to answer or to do something?", outcome: "yes or no", trust: 75),
                .init(.default, "no", outcome: "no")]),
            DecisionPage(id: "sign-in-codes", title: "Sign-in codes", settles: "the notch’s code pill", entries: [
                .init(.rule, "subject contains “code”, “verification”, “sign in”", outcome: "yes", match: "code"),
                .init(.model, "Does this email carry a one-time code, a login link or a verification number?", outcome: "yes or no", trust: 90),
                .init(.default, "no", outcome: "no")]),
            DecisionPage(id: "vips", title: "VIPs", settles: "the VIPs tab and the waiting count", entries: [
                .init(.rule, "on your VIP list", outcome: "yes", match: "vip"),
                .init(.default, "no", outcome: "no")]),
            DecisionPage(id: "notify-me", title: "Notify me", settles: "which arrivals ring the Mac and the iPhone", entries: [
                .init(.rule, "from a VIP", outcome: "notify", match: "vip"),
                .init(.rule, "Needs my reply is yes", outcome: "notify", match: "reply"),
                .init(.rule, "has a list header", outcome: "silent", match: "list"),
                .init(.default, "silent", outcome: "silent")]),
            // "your rules only (no model entry shipped)" (docs/DECISION-MODEL.md v2 §1): the list ships with its default alone; the owner's
            // rules come from "Add a rule" and an accepted proposal (hid-ai: a sample rule with an example domain shipped here before)
            DecisionPage(id: "labels", title: "Labels", settles: "labels added on arrival", entries: [
                .init(.default, "no label", outcome: "none")]),
        ]
    }
}
