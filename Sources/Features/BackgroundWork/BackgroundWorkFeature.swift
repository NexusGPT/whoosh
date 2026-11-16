import SwiftUI

/// Registration of the background download's hooks (lane bg-hooks). Listed in FeatureRegistry.all by the lead.
/// The socket region `mac-main.background` has one row: what the app read, what the engine was told, and what that makes the
/// crawl do (see BackgroundState.row).
public enum BackgroundWorkFeature: FeatureModule {
    public static let id = "bg-hooks"
    public static let region = "mac-main.background"

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        [region: { m in
            var row = BackgroundState.shared.row(floor: WhooshConstants.ATTACHMENT_INDEX_BATTERY_MIN, enginePause: BackgroundState.enginePause(m.core))
            row["model"] = MeaningModelStore.shared.state.key
            return [row]
        },
         MeaningModelDoor.region: { _ in [MeaningModelDoor.row()] }]
    }

    /// The button of the search model's row, for a check: starts the download (or the retry); false when nothing can start.
    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        [MeaningModelDoor.downloadId: { _ in MeaningModelStore.shared.startInBackground() }]
    }

    /// Draws nothing. The shell asks every feature for its view at launch, the one launch hook a feature has (EmbeddingFeature
    /// does the same): the first reading of the battery and heat is taken here, off the main thread.
    public static func view(for slot: SurfaceSlot) -> AnyView? { BackgroundWork.start(); return nil }
}
