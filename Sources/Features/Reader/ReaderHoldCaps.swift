import SwiftUI

/// Hold ⌘ (docs/PRD.md "## Hold ⌘"): while the shell shows its key badges (MacHintState, read only), each icon of the
/// reader's header band shows its key in one cap centred under it, like the toolbar. The key is the one the icon's
/// tooltip shows (ReaderKeyLabel, the active preset). The caps fade in and out over POPOVER_MS; nothing waits on them.
struct ReaderHoldCap: View {
    let key: String
    @State private var hints = MacHintState.shared  // local-ui: the shell's hold state, observed
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        KeyCap(key)
            .fixedSize()
            .offset(y: WhooshMetrics.keyCap + WhooshSpacing.s2)
            .opacity(ReaderHoldCaps.shown(key, hints.visible) ? 1 : 0)
            .animation(WhooshMotion.animation(.easeOut(duration: Double(WhooshConstants.POPOVER_MS) / 1000), reduceMotion: reduceMotion), value: hints.visible)
            .allowsHitTesting(false)
            .accessibilityHidden(true)
    }
}

@MainActor
enum ReaderHoldCaps {
    static func shown(_ key: String, _ holding: Bool) -> Bool { holding && !key.isEmpty }

    /// `mac-main.reader.keycaps`: one row per header icon, in drawing order: its id, its key, whether its cap shows.
    static func rows(_ m: AppModel) throws -> [ControlRow] {
        _ = try ReaderProbe.thread(m)
        let holding = MacHintState.shared.visible
        return ReaderAction.all.map { a in ["id": a.id, "command": a.command, "key": a.key, "visible": shown(a.key, holding)] }
    }
}
