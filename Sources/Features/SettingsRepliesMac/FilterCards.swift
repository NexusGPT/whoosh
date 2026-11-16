import SwiftUI

/// The rule of a filter card: « Et quand [Toutes] ces conditions sont vraies », then the eight facts in three states, two to a row (the label
/// inspector's FactCell and TriState, reused as they are).
struct ReplyRuleCard: View {
    let card: FlowCard
    let rule: RuleSet
    let lead: String
    @State private var store = RepliesStore.shared
    private var P: String { RepliesAx.id("filter", card == .exit ? "exit" : "entry") }

    var body: some View {
        ReplyCardRow(key: lead, ax: "\(P).match.row") {
            Menu {
                Button(RepliesWords.allOfThem) { store.setMode(card, .all) }.accessibilityIdentifier("\(P).match.all")
                Button(RepliesWords.oneOfThem) { store.setMode(card, .any) }.accessibilityIdentifier("\(P).match.any")
            } label: { PopPill { Text(rule.matchMode == .all ? RepliesWords.allOfThem : RepliesWords.oneOfThem) } }
                .menuStyle(.button).buttonStyle(.plain).menuIndicator(.hidden).fixedSize()
                .whooshAx("\(P).match")
            Text(rule.matchMode == .all ? RepliesWords.areTrue : RepliesWords.isTrue)
                .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
        }
        Hair()
        ViewThatFits(in: .horizontal) {
            Grid(horizontalSpacing: WhooshSpacing.s24, verticalSpacing: WhooshSpacing.s2) {
                ForEach(0..<RepliesFacts.order.count / 2, id: \.self) { r in GridRow { cell(2 * r); cell(2 * r + 1) } }
            }
            VStack(spacing: WhooshSpacing.s2) { ForEach(0..<RepliesFacts.order.count, id: \.self) { cell($0) } }
        }
        .padding(.horizontal, WhooshSpacing.s12).padding(.vertical, WhooshSpacing.s6)
        .frame(maxWidth: .infinity, alignment: .leading)
        .whooshAx("\(P).facts")
    }

    private func cell(_ i: Int) -> some View {
        let fact = RepliesFacts.order[i], key = messageFactKey(fact: fact)
        return FactCell(name: FlowFacts.title(fact), state: RepliesFacts.state(rule.conditions, fact), id: "\(P).fact.\(key)") { store.setFact(card, fact, $0) }
            .whooshAx("\(P).fact.\(key)")
    }
}

/// « Expéditeur »: one field, a name, an address or a domain; Return keeps it, the cross clears it.
struct SenderRow: View {
    let sender: String
    @State private var store = RepliesStore.shared
    @State private var text = ""   // local-ui: the words being typed, kept by Return
    private let P = RepliesAx.id("filter", "entry", "sender")

    var body: some View {
        ReplyCardRow(key: RepliesWords.sender) {
            HStack(spacing: WhooshSpacing.s6) {
                TextField(RepliesWords.senderHint, text: $text).textFieldStyle(.plain).whooshText(WhooshTypography.subhead)
                    .onSubmit { store.setSender(text) }
                    .accessibilityIdentifier(P)
                if !sender.isEmpty {
                    Button { text = ""; store.setSender("") } label: { Image(systemName: "xmark.circle.fill").foregroundStyle(WhooshPalette.tertiaryLabel) }
                        .buttonStyle(.plain).help(RepliesWords.clear).whooshAx("\(P).clear")
                }
            }
            .padding(.horizontal, WhooshSpacing.s8).frame(height: WhooshSpacing.s24)
            .background(RoundedRectangle(cornerRadius: WhooshRadius.item, style: .continuous).fill(WhooshPalette.fill))
            .whooshAx(P)
        }
        .onAppear { text = sender }
        .onChange(of: sender) { _, s in text = s }
    }
}

/// « L'IA lit »: subject, body, the rest of the thread, attachments. The core refuses a card that reads nothing, in words.
struct ReadsRow: View {
    let reads: AiReads
    @State private var store = RepliesStore.shared
    var body: some View {
        let P = RepliesAx.id("filter", "ai", "reads")
        ReplyCardRow(key: Words.text("The AI reads"), ax: "\(P).row") {
            FlowLayout(spacing: WhooshSpacing.s6) {
                ReplyTick(title: Words.text("Subject"), on: reads.subject, ax: "\(P).subject") { store.toggleRead(.subject) }
                ReplyTick(title: Words.text("Body"), on: reads.body, ax: "\(P).body") { store.toggleRead(.body) }
                ReplyTick(title: RepliesWords.restOfThread, on: reads.restOfThread, ax: "\(P).rest") { store.toggleRead(.rest) }
                ReplyTick(title: Words.text("Attachments"), on: reads.attachments, ax: "\(P).attachments") { store.toggleRead(.attachments) }
            }
        }
    }
}

/// `.lf-desc`: the question the AI answers per message, in the owner's words: the one Replies editor (RepliesTextEditor).
struct DescriptionBox: View {
    var body: some View {
        RepliesTextEditor(key: .aiDescription, placeholder: RepliesWords.descriptionHint)
            .padding(.horizontal, WhooshSpacing.s12).padding(.top, WhooshSpacing.s2).padding(.bottom, WhooshSpacing.s8)
    }
}
