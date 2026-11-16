/// The send shockwave's registration: the socket region `mac-main.sendwave` (one row per pane that has waved:
/// state, scheme, elapsed, and what the run cost). Listed in FeatureRegistry.all by the root.
public enum SendWaveFeature: FeatureModule {
    public static let id = "sendwave"
    public static let region = "mac-main.sendwave"
    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        [region: { _ in SendWaveCenter.shared.rows() }]
    }
}
