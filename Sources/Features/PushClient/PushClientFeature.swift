/// The pushclient lane's registration. Its one served region is the phone rig's recording of the notices the router decided on
/// (PushClientRig.swift): empty on every copy that is not a phone test copy.
@MainActor
public enum PushClientFeature: FeatureModule {
    public static let id = "pushclient"

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        ["iphone-lock.notifications": { m in PushClientRig.notificationRows(m) }]
    }
}
