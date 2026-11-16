import SwiftUI
import Observation

/// What the search model line shows: the store's state, followed. The store announces each CHANGE (`MeaningModelStore.didChange`), whoever started the
/// download (this line's click, another surface, a relaunch); this model reads the state again on the announcement and when the page opens.
/// Nothing polls: no timeline, no timer here. The store's own byte timer runs only inside a download.
@MainActor @Observable
final class MeaningLineModel {
    static let shared = MeaningLineModel()

    let store: MeaningModelStore
    private(set) var state: MeaningModelState
    @ObservationIgnored private var listener: NSObjectProtocol?

    init(store: MeaningModelStore = .shared) {
        self.store = store
        state = store.state
        listener = NotificationCenter.default.addObserver(forName: MeaningModelStore.didChange, object: store, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.follow() }
        }
    }

    /// The store moved: take what it holds (never reads the disk, so it cannot call itself).
    func follow() { state = store.state }
    /// The page opened: read the disk once.
    func open() { state = store.refresh() }
    /// The Download or Retry button: the line changes on the click; the store takes over from there.
    func download() {
        guard store.startInBackground() else { return }
        state = .downloading(received: ModelDisk.bytesOnDisk(store.spec, in: store.directory), total: store.spec.totalBytes)
    }
}

/// The search model's row in Settings > AI and agents, drawn like the Laya row (board ai-setup-r1, W1) over bg-hooks' `MeaningModelStore`.
/// Ids are the store's own door (`MeaningModelDoor`: `mac-settings-ai.meaning`, `.download`); BackgroundWorkFeature registers the socket region and the click.
struct MeaningSetupRow: View {
    private let line = MeaningLineModel.shared
    private let index = IndexProgressModel.shared
    var body: some View {
        let s = line.state
        // The index line exists only while the model is on this Mac; it reads the one index model (no timer, no poll).
        let figures: IndexProgress.SettingsLine? = { if case .ready = s { return IndexProgress.settingsLine(index.status) } else { return nil } }()
        VStack(alignment: .leading, spacing: 0) {
            ModelSetupRow(ax: MeaningModelDoor.region, name: AIWords.searchByMeaning, suffix: nil,
                          caption: AIWords.onThisMacOnce(AttachmentSize.words(line.store.spec.totalBytes)),
                          phase: ModelPhase.of(s, expected: line.store.spec.totalBytes),
                          buttonTitle: s.button ?? AIWords.layaDownload, buttonId: MeaningModelDoor.downloadId,
                          cancelTitle: AIWords.cancel, cancelId: "\(MeaningModelDoor.region).cancel",
                          download: { line.download() }, cancel: nil, chip: AIWords.speechStateReady)
            if let figures {
                VStack(alignment: .leading, spacing: 0) {
                    Text(figures.text).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                        .contentTransition(.numericText()).lineLimit(1)
                    if let fraction = figures.fraction { ProgressBar(fraction: fraction) }
                }
                .padding(.horizontal, SettingsMetrics.rowSideX).padding(.bottom, WhooshSpacing.s10)
                .padding(.top, -WhooshSpacing.s6)   // tucks the line under the caption: the row above ends with its own bottom padding
                .frame(maxWidth: .infinity, alignment: .leading)
                .animation(.easeOut(duration: WhooshMotion.rowInsert), value: figures)
                .accessibilityElement(children: .combine)
                .accessibilityIdentifier(MeaningModelDoor.indexId)
            }
        }
        .onAppear { line.open() }
    }
}
