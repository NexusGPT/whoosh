import SwiftUI

/// Design W1 (ai-setup-r1): "Amber dot and one line". Where a feature needs a model that is not there, it keeps its place and gains one amber
/// dot, one line that says what is missing and one button that fixes it right there. Both forms read `AISetupStatus` and draw NOTHING when the
/// need is met. The warn colour is the app's own (`WhooshPalette.warning`).

/// The dot alone: 7 pt, a soft ring (the warn colour at 24 %, 3 pt wide), for a sidebar row or a card head. `needs`: the dot shows while any is unmet.
struct MissingSetupDot: View {
    let needs: [AINeed]
    var id: String? = nil
    @State private var status = AISetupStatus.shared

    var body: some View {
        if needs.contains(where: { !status.status($0).usable }) {
            Circle().fill(WhooshPalette.warning).frame(width: 7, height: 7)
                .background(Circle().fill(WhooshPalette.warning.opacity(0.24)).frame(width: 13, height: 13))
                .frame(width: 13, height: 13)
                .accessibilityLabel(Text(status.missing.first(where: { needs.contains($0.need) })?.words ?? ""))
                .whooshAx(id ?? "ai-setup.dot.\(needs.map(\.rawValue).joined(separator: "+"))")
                .transition(.opacity)
        }
    }
}

/// The sentence a line carries: what is missing (`status`), or what it costs the feature (`consequence`, the label card and the summary card).
enum MissingSetupSentence { case status, consequence }

/// The line: dot, one 12 pt sentence in the warn colour, the fix button on the right. While a download runs it becomes the progress line,
/// a 4 pt bar and Cancel. `key`: the key cap on the button, bound to it while the line is on screen (the placement decides; none = no cap).
struct MissingSetupMark: View {
    let need: AINeed
    var sentence: MissingSetupSentence = .status
    var key: String? = nil
    var cancelKey: String? = nil
    var id: String? = nil
    @State private var status = AISetupStatus.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    /// The mark draws something for this need now (a download in progress, or a model that is missing): a wrapper that paints a ground or keeps room
    /// around the mark asks this first.
    @MainActor static func says(_ need: AINeed) -> Bool {
        let x = AISetupStatus.shared.status(need)
        return x.state == .working || !x.usable
    }

    var body: some View {
        let x = status.status(need)
        Group {
            if x.state == .working { progress(x) }
            else if !x.usable { line(x) }
        }
        .animation(WhooshMotion.animation(.easeOut(duration: Double(WhooshConstants.POPOVER_MS) / 1000), reduceMotion: reduceMotion), value: x.state)
        .task { status.refresh() }
        .whooshAx(id ?? "ai-setup.mark.\(need.rawValue)")
    }

    private func line(_ x: AINeedStatus) -> some View {
        HStack(spacing: 8) {
            Circle().fill(WhooshPalette.warning).frame(width: 7, height: 7)
                .background(Circle().fill(WhooshPalette.warning.opacity(0.24)).frame(width: 13, height: 13))
                .frame(width: 13, height: 13)
            Text(words(x)).font(.system(size: 12)).foregroundStyle(WhooshPalette.warning).fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 8)
            AISetupButton(title: button(x), key: key, amber: true) { AISetupDoors.fix(need) }.whooshAx("ai-setup.mark.\(need.rawValue).fix")
        }
        .padding(.top, 7)
    }

    private func progress(_ x: AINeedStatus) -> some View {
        HStack(alignment: .center, spacing: 10) {
            VStack(alignment: .leading, spacing: 0) {
                Text(x.words).font(.system(size: 12)).foregroundStyle(WhooshPalette.secondaryLabel)
                AISetupBar(fraction: x.progress ?? 0).padding(.top, 8)
            }
            if need == .decision { AISetupButton(title: AISetupWords.cancel, key: cancelKey, amber: false) { AISetupDoors.cancel(need) }.whooshAx("ai-setup.mark.\(need.rawValue).cancel") }
        }
    }

    private func words(_ x: AINeedStatus) -> String { AISetupMarkText.words(need, sentence, x) }
    private func button(_ x: AINeedStatus) -> String { AISetupMarkText.button(need, sentence, x) }
}

/// What a line says and what its button says, once: the view and the socket rows both read these.
@MainActor
enum AISetupMarkText {
    static func words(_ need: AINeed, _ sentence: MissingSetupSentence, _ x: AINeedStatus) -> String {
        if !["provider-none", "laya-missing", "meaning-missing"].contains(x.reason) { return x.words }   // a failure, a refusal, Jev's key or consent: the status' own sentence
        switch (need, sentence) {
        case (.decision, .consequence): return AISetupWords.cannotSort
        case (.decision, .status): return AISetupWords.notOnThisMac(AISetupStatus.byteWords(AISetupWords.layaBytes))
        case (.writing, .consequence): return AISetupWords.needsWriting
        case (.writing, .status): return x.words
        case (.meaning, .consequence): return AISetupWords.searchOff
        case (.meaning, .status): return AISetupWords.notOnThisMac(AISetupStatus.byteWords(MeaningModelStore.shared.spec.totalBytes))
        }
    }

    static func button(_ need: AINeed, _ sentence: MissingSetupSentence, _ x: AINeedStatus) -> String {
        if x.state == .failed { return AISetupWords.retry }
        switch need {
        case .writing: return AISetupWords.connectModel
        case .decision: return x.reason == "laya-missing" && sentence == .consequence ? AISetupWords.downloadSize(AISetupStatus.byteWords(AISetupWords.layaBytes)) : (x.door?.title ?? AISetupWords.doorDownload)
        case .meaning: return AISetupWords.doorMeaning
        }
    }
}

/// The board's `.b2`: 26 pt tall, 7 pt radius, 12.5 pt; the amber one is the warn colour with dark text, the plain one the fill with a hairline.
struct AISetupButton: View {
    let title: String
    var key: String? = nil
    var amber = false
    var primary = false
    let action: () -> Void

    var body: some View {
        let label = HStack(spacing: 7) {
            Text(title).font(.system(size: 12.5, weight: amber || primary ? .semibold : .medium))
            if let key { FirstStartKbd(text: key, onAccent: primary) }
        }
        .padding(.horizontal, 10).frame(height: 26)
        .foregroundStyle(amber ? AnyShapeStyle(Color(.sRGB, red: 0x1A / 255, green: 0x12 / 255, blue: 0, opacity: 1)) : (primary ? AnyShapeStyle(WhooshPalette.onAccent) : AnyShapeStyle(WhooshPalette.label)))
        .background(amber ? AnyShapeStyle(WhooshPalette.warning) : (primary ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(WhooshPalette.fill)), in: RoundedRectangle(cornerRadius: 7, style: .continuous))
        .overlay { if !amber && !primary { RoundedRectangle(cornerRadius: 7, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: 0.5) } }
        .contentShape(RoundedRectangle(cornerRadius: 7, style: .continuous))
        let button = Button(action: action) { label }.buttonStyle(FirstStartPressStyle()).fixedSize()
        if let key, let shortcut = AISetupButton.shortcut(key) { button.keyboardShortcut(shortcut, modifiers: []) } else { button }
    }

    /// the keys a cap may bind: ↩ and esc
    static func shortcut(_ cap: String) -> KeyEquivalent? { cap == "↩" ? .return : (cap == "esc" ? .escape : nil) }
}

/// The board's `.bar`: 4 pt, rounded, the fill with an accent bar that grows; the width change is a transform-free resize of one capsule (no layout of its neighbours).
struct AISetupBar: View {
    let fraction: Double
    var body: some View {
        ZStack(alignment: .leading) {
            Capsule().fill(WhooshPalette.fill)
            Capsule().fill(WhooshPalette.accent).scaleEffect(x: max(0.02, min(1, fraction)), y: 1, anchor: .leading)
        }
        .frame(height: 4).clipShape(Capsule())
    }
}
