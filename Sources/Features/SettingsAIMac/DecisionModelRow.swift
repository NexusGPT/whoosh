import SwiftUI

private let D = "mac-settings-ai"

/// One line of the list of on-device decision models (SPEC A133; the owner's pick M1, board decision-models-r1): the circle of the model in
/// use, the name, the facts (size, licence, index score), and its state at the right: Ready, a Download button, or Cancel while it downloads.
/// The model in use that is not on this Mac says so in the amber line the owner picked (W1), with its one button and its key. A model asked
/// for while another is in use downloads first; the line of the model in use then says "In use until … is ready.", and only when that
/// model is on this Mac: a model that is not here sorts nothing, so its line keeps its amber sentence and its Download button.
struct DecisionModelRow: View {
    let entry: DecisionModelEntry
    let store: AISettingsStore

    /// Laya keeps the ids its checks and the label inspector know; every other catalogue line is named by its catalogue id, and an added line by its
    /// place among the added ones (`decisions.added.<k>`, k from 1: its id holds a slash, and its doors are registered before it exists).
    static func ax(_ entry: DecisionModelEntry) -> String {
        if entry.family == .laya { return "\(D).decisions.local" }
        if let k = addedPlace(entry.id) { return "\(D).decisions.added.\(k)" }
        return "\(D).decisions.model.\(entry.id)"
    }
    /// The place of an added line among the added ones, from 1; nil for a catalogue line.
    static func addedPlace(_ id: String) -> Int? {
        let added = DecisionModels.shared.lines.filter { DecisionModels.shared.isAdded($0.id) }
        return added.firstIndex { $0.id == id }.map { $0 + 1 }
    }
    private var ax: String { Self.ax(entry) }
    private var inUse: Bool { store.chosenModel == entry.id }
    /// The one line whose buttons take Return and Escape: the model in use, or the one being fetched to replace it. While the field of "Other model"
    /// is open, Return and Escape are its own (Check, leave the field).
    private var keyed: Bool { (inUse || store.wantedModel == entry.id) && !store.otherOpen }
    /// An added line that is not in use and not downloading carries Remove.
    private var removable: Bool { Self.removable(entry, store) }
    @MainActor static func removable(_ entry: DecisionModelEntry, _ s: AISettingsStore) -> Bool {
        guard DecisionModels.shared.isAdded(entry.id), s.chosenModel != entry.id else { return false }
        if case .downloading = s.modelState(entry) { return false }
        return true
    }
    private var phase: ModelPhase { ModelPhase.of(store.modelState(entry), expected: store.modelBytes(entry)) }

    var body: some View {
        HStack(alignment: .top, spacing: WhooshSpacing.s10) {
            Button { store.chooseModel(entry.id) } label: { DecisionModelMark(inUse: inUse) }
                .buttonStyle(.plain).padding(.top, WhooshSpacing.s2)
                .accessibilityLabel(entry.name).accessibilityAddTraits(inUse ? .isSelected : [])
                .accessibilityIdentifier(ax + ".choose")
            VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                Text(entry.name).fontWeight(inUse ? .semibold : .regular).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label)
                factsLine
                if inUse, case .ready = phase, let other = store.wantedModel, other != entry.id {
                    Text(AIWords.inUseUntil(DecisionModels.shared.line(id: other).name))
                        .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                        .accessibilityIdentifier(ax + ".until")
                }
                switch phase {
                case .ready: EmptyView()
                case .missing(let line): if inUse { warnLine(line, AIWords.layaDownload) }
                case .failed(let line): warnLine(line, AIWords.retry)
                case .downloading(let line, let fraction):
                    Text(line).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).contentTransition(.numericText())
                    ProgressBar(fraction: fraction)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            trailing
        }
        .padding(.horizontal, SettingsMetrics.rowSideX).padding(.vertical, WhooshSpacing.s10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .animation(.easeOut(duration: WhooshMotion.rowInsert), value: phase)
        .animation(.easeOut(duration: WhooshMotion.rowInsert), value: inUse)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier(ax)
    }

    private var factsLine: some View {
        let facts = DecisionModelFacts(entry.shownOnList)
        return (Text(facts.size).fontWeight(.semibold).foregroundStyle(WhooshPalette.label) + Text(facts.rest.isEmpty ? "" : " · " + facts.rest))
            .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
            .accessibilityIdentifier(ax + ".facts")
    }

    private var trailing: some View {
        HStack(spacing: WhooshSpacing.s8) {
            stateView
            if removable { SettingsButton(title: AIWords.remove, ax: ax + ".remove") { store.removeModel(entry.id) } }
        }
    }

    @ViewBuilder private var stateView: some View {
        switch phase {
        case .ready:
            if inUse { AIChip(text: AIWords.speechStateReady, tone: .good) }
            else { Text(AIWords.speechStateReady).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel) }
        case .downloading:
            if keyed {
                SettingsButton(title: AIWords.cancel, key: "esc", ax: ax + ".cancel") { store.cancelModel(entry.id) }.keyboardShortcut(.cancelAction)
            } else {
                SettingsButton(title: AIWords.cancel, ax: ax + ".cancel") { store.cancelModel(entry.id) }
            }
        case .missing:
            if !inUse { SettingsButton(title: AIWords.layaDownload, ax: ax + ".download") { store.downloadModel(entry.id) } }
        case .failed: EmptyView()
        }
    }

    /// The amber line (board `.wl`): the dot, one sentence, one button. Return presses it on the keyed line only.
    private func warnLine(_ line: String, _ button: String) -> some View {
        HStack(spacing: WhooshSpacing.s8) {
            AmberDot()
            Text(line).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.warning).fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: WhooshSpacing.s8)
            if keyed {
                AmberButton(title: button, key: "\u{21A9}", ax: ax + ".download", action: { store.downloadModel(entry.id) }).keyboardShortcut(.defaultAction)
            } else {
                SettingsButton(title: button, ax: ax + ".download") { store.downloadModel(entry.id) }
            }
        }
        .padding(.top, WhooshSpacing.s6)
    }
}

extension DecisionModelRow {
    /// What a line shows, as the checks read it: the same fields for every line of the list. A pure read of what the screen holds.
    @MainActor static func row(_ entry: DecisionModelEntry, _ s: AISettingsStore) -> [String: Any] {
        let inUse = s.chosenModel == entry.id, keyed = (inUse || s.wantedModel == entry.id) && !s.otherOpen
        var row: [String: Any] = ["model": entry.id, "name": entry.name, "facts": DecisionModelFacts(entry.shownOnList).line, "in_use": inUse, "wanted": s.wantedModel == entry.id,
                                  "ax": ax(entry), "added": DecisionModels.shared.isAdded(entry.id), "remove": removable(entry, s)]
        let state = s.modelState(entry)
        if inUse, case .ready = state, let other = s.wantedModel, other != entry.id { row["until"] = AIWords.inUseUntil(DecisionModels.shared.line(id: other).name) }
        let phase = ModelPhase.of(state, expected: s.modelBytes(entry))
        switch state {
        case .ready:
            // the ready line draws no sentence, only the word Ready at its right (a chip on the model in use)
            row.merge(["state": "ready", "words": "", "chip": AIWords.speechStateReady, "download": false]) { _, new in new }
        case .downloading(let r, let t):
            var words = ""; if case .downloading(let l, _) = phase { words = l }
            row.merge(["state": "downloading", "words": words, "received": r, "total": t, "download": false, "cancel": true, "key": keyed ? "esc" : ""]) { _, new in new }
        case .notDownloaded:
            var words = ""; if inUse, case .missing(let l) = phase { words = l }
            row.merge(["state": "absent", "words": words, "download": true, "button": AIWords.layaDownload, "key": inUse && keyed ? "\u{21A9}" : ""]) { _, new in new }
        case .failed(let why):
            row.merge(["state": "failed", "words": why, "download": true, "retry": true, "button": AIWords.retry, "key": keyed ? "\u{21A9}" : ""]) { _, new in new }
        }
        return row
    }
}
