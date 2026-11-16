import SwiftUI

/// One entry as a sentence: number, kind word, the condition or the question, the outcome, the trust, its switch.
/// A model entry opens in place to edit the question word for word (what is sent is what you read).
struct DecisionEntryRow: View {
    let store: AISettingsStore
    let pageId: String
    let entry: DecisionEntry
    let n: Int
    let last: Bool
    @State private var open = false
    @State private var hover = false

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            HStack(spacing: WhooshSpacing.s10) {
                Image(systemName: "line.3.horizontal").foregroundStyle(WhooshPalette.tertiaryLabel.opacity(last ? 0 : 1))
                Text("\(n)").whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.tertiaryLabel)
                    .frame(width: WhooshSpacing.s16)
                AIChip(text: kindWord, tone: entry.kind == .model ? .accent : .neutral)
                sentence.lineLimit(open ? nil : 1)
                Spacer(minLength: WhooshSpacing.s8)
                if let t = entry.trust { trustMenu(t) }
                if !last {
                    SettingsSwitch(on: entry.enabled)
                        .contentShape(Rectangle())
                        .onTapGesture { store.update(page: pageId, entry: entry.id) { $0.enabled.toggle() } }
                        .accessibilityHidden(false).accessibilityAddTraits(.isButton)
                        .accessibilityIdentifier("mac-settings-ai.decisions.page.\(pageId).entry.\(n).enabled")
                }
            }
            if open && entry.kind == .model {
                TextField(AIWords.askTheModel, text: Binding(get: { entry.text }, set: { v in store.update(page: pageId, entry: entry.id) { $0.text = v } }), axis: .vertical)
                    .textFieldStyle(.plain).whooshText(WhooshTypography.body)
                    .padding(WhooshSpacing.s8)
                    .background(RoundedRectangle(cornerRadius: WhooshRadius.field).fill(WhooshPalette.field))
                    .overlay(RoundedRectangle(cornerRadius: WhooshRadius.field).strokeBorder(WhooshPalette.separator))
                    .padding(.leading, WhooshSpacing.s32 * 2)
                    .transition(.opacity)
                    .accessibilityIdentifier("mac-settings-ai.decisions.page.\(pageId).entry.\(n).question")
            }
        }
        .padding(.horizontal, WhooshSpacing.s16).padding(.vertical, WhooshSpacing.s10)
        .opacity(entry.enabled ? 1 : 0.45)
        .background(WhooshPalette.hover.opacity(hover ? 1 : 0))
        .contentShape(Rectangle())
        .onTapGesture { if entry.kind == .model { open.toggle() } }
        .onHover { hover = $0 }
        .animation(.easeOut(duration: WhooshMotion.hoverTint), value: hover)
        .animation(WhooshMotion.unfold, value: open)
        .accessibilityIdentifier("mac-settings-ai.decisions.page.\(pageId).entry.\(n)")
    }

    private var kindWord: String { [.rule: Words.text("Rule"), .model: Words.text("Model"), .default: Words.text("Default")][entry.kind]! }

    @ViewBuilder private var sentence: some View {
        HStack(spacing: WhooshSpacing.s4) {
            switch entry.kind {
            case .rule:
                Text(AIWords.ifWord).foregroundStyle(WhooshPalette.secondaryLabel)
                Text(Words.text(entry.text)).foregroundStyle(WhooshPalette.label)
                Text("→").foregroundStyle(WhooshPalette.tertiaryLabel)
                outcome
            case .model:
                Text(AIWords.askTheModel + ":").foregroundStyle(WhooshPalette.secondaryLabel)
                Text("“\(Words.text(entry.text))”").foregroundStyle(WhooshPalette.label)
            case .default:
                Text(AIWords.otherwise).foregroundStyle(WhooshPalette.secondaryLabel)
                outcome
            }
        }
        .whooshText(WhooshTypography.body)
    }

    @ViewBuilder private var outcome: some View {
        if pageId == "labels" && entry.kind == .rule {
            DecisionLabelMenu(path: entry.outcome) { v in store.update(page: pageId, entry: entry.id) { $0.outcome = v } }
                .accessibilityIdentifier("mac-settings-ai.decisions.page.\(pageId).entry.\(n).label")
        } else {
            Text(Words.text(entry.outcome)).fontWeight(.semibold).foregroundStyle(WhooshPalette.label)
        }
    }

    private func trustMenu(_ t: Int) -> some View {
        Menu {
            ForEach(InterimTrust.levels(shipped: entry.shippedTrust ?? t), id: \.1) { name, v in
                Button("\(Words.text(name)) · \(v) %") { store.update(page: pageId, entry: entry.id) { $0.trust = v } }
            }
        } label: {
            Text("\(AIWords.trust) \(t) %").whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
        }
        .menuStyle(.borderlessButton).fixedSize()
        .accessibilityIdentifier("mac-settings-ai.decisions.page.\(pageId).entry.\(n).trust")
    }
}

/// INTERIM: the board's Strict and Eager levels are proposed values with no docs/CONSTANTS.md row yet (gap handed back);
/// Balanced is the page's shipped trust from docs/DECISION-MODEL.md v2.
enum InterimTrust {
    static func levels(shipped: Int) -> [(String, Int)] {
        [("Strict", 90), ("Balanced", shipped), ("Eager", 70)].filter { $0.1 != shipped || $0.0 == "Balanced" }
    }
}
