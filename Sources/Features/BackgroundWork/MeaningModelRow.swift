import SwiftUI

/// The search model's plain row for Settings > AI and agents: the state in words and ONE button (Download, or Retry after a failure).
/// No design: the look is the lead's to draw. Ids: `mac-settings-ai.meaning.state` and `mac-settings-ai.meaning.download`; the socket
/// region `mac-settings-ai.meaning` and the click target of the button are registered by BackgroundWorkFeature.
/// The row re-reads twice a second ONLY while a download runs (a paused timeline otherwise: no wake-ups on an idle pane).
public struct MeaningModelRow: View {
    @State private var started = 0
    private let store = MeaningModelStore.shared
    public init() {}

    public var body: some View {
        let running = store.state.isDownloading || started < 0   // `started` only makes the view read again after a press
        TimelineView(.animation(minimumInterval: 0.5, paused: !running)) { _ in
            let s = store.refresh()
            VStack(alignment: .leading, spacing: 6) {
                Text(s.words).accessibilityIdentifier(MeaningModelDoor.stateId)
                if let title = s.button {
                    Button(title) { if store.startInBackground() { started += 1 } }.accessibilityIdentifier(MeaningModelDoor.downloadId)
                }
            }
        }
    }
}

/// The ids of the door, once.
public enum MeaningModelDoor {
    public static let region = "mac-settings-ai.meaning"
    public static let stateId = "mac-settings-ai.meaning.state"
    public static let downloadId = "mac-settings-ai.meaning.download"
    public static let indexId = "mac-settings-ai.meaning.index"

    /// The row of the socket region: the state word, the sentence, the bytes, the button label (empty when none).
    @MainActor static func row() -> ControlRow {
        let s = MeaningModelStore.shared.state   // a read answers what the store holds; it never re-reads the disk, so it never moves the screen
        var row: ControlRow = ["ax": stateId, "state": s.key, "words": s.words, "button": s.button ?? "", "button_ax": downloadId]
        if case .downloading(let r, let t) = s { row["received"] = r; row["total"] = t }
        if case .ready(let b) = s { row["bytes"] = b }
        // The index figures, as the one index model holds them (a read never asks the builder). The line is absent unless the model is on this Mac.
        let idx = IndexProgressModel.shared.status
        row["indexing"] = IndexProgress.isBuilding(idx); row["index_done"] = idx.indexed; row["index_total"] = IndexProgress.total(idx)
        row["index_paused"] = IndexProgress.isPaused(idx); row["index_failed"] = idx.failed
        if case .ready = s { row["index_text"] = IndexProgress.settingsLine(idx)?.text ?? NSNull() } else { row["index_text"] = NSNull() }
        return row
    }
}
