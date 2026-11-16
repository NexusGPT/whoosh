import SwiftUI
/// Registration stub for the embedding lane (folder Sources/Features/Embedding/), written by the root commit.
/// The lane fills it: rows, keys, clicks and palette commands register here, never in a hub.
public enum EmbeddingFeature: FeatureModule {
    public static let id = "embedding"
    /// Draws nothing. The shell asks every feature for its view at launch, which is the one launch hook a feature has: the model loads here, off the main thread.
    public static func view(for slot: SurfaceSlot) -> AnyView? { MeaningSearch.preload(); return nil }
}
