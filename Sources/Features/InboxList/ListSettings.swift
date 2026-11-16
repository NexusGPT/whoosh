import Foundation

/// The list's one persisted preference: "Show account emblems in the message list" (Settings > General, default on;
/// docs/DESIGN.md "Account badge: emblem"). The settingsgeneralmac lane's toggle writes this key; the list reads it
/// live through @AppStorage, and the socket probe reads the same key, so screen and probe cannot disagree.
@MainActor
enum ListSettings {
    static let showEmblemsKey = "whoosh.list.showEmblems"

    static var showEmblems: Bool {
        get { UserDefaults.standard.object(forKey: showEmblemsKey) as? Bool ?? true }
        set { UserDefaults.standard.set(newValue, forKey: showEmblemsKey) }
    }
}
