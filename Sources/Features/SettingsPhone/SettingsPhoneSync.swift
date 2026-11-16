import Foundation

/// The Sync line's facts, read from the core's state paths and nothing else (a path the core does not serve reads as absent):
/// `settings_sync/devices` (JSON array of the paired Macs' names), `settings_sync/last_sync_ms` (the last successful sync),
/// `settings_sync/error` (a string, only while the last sync failed; the reason in the core's words).
struct SyncReading: Equatable {
    enum Phase: String { case unpaired, waiting, inSync, failing }
    let phase: Phase
    let device: String
    let lastMs: Int64?
    let reason: String

    /// Whole seconds since the last successful sync; nil when there is none.
    func age(nowMs: Int64) -> Int? { lastMs.map { Int(max(nowMs - $0, 0) / 1000) } }
    var retry: Bool { phase == .failing }
}

@MainActor
enum SettingsPhoneSync {
    static func read(_ m: AppModel) -> SyncReading {
        let core = m.core
        let devices = (value(core, "settings_sync/devices") as? [String]) ?? []
        let last = (value(core, "settings_sync/last_sync_ms") as? NSNumber)?.int64Value
        let error = (value(core, "settings_sync/error") as? String) ?? ""
        guard let mac = devices.first, !mac.isEmpty else { return SyncReading(phase: .unpaired, device: "", lastMs: nil, reason: "") }
        if !error.isEmpty { return SyncReading(phase: .failing, device: mac, lastMs: last, reason: error) }
        if last == nil { return SyncReading(phase: .waiting, device: mac, lastMs: nil, reason: "") }
        return SyncReading(phase: .inSync, device: mac, lastMs: last, reason: "")
    }

    static func nowMs() -> Int64 { CoreHost.shared.clock.nowMs() }

    /// The headline: "In sync with <Mac>", or why it is not.
    static func line(_ r: SyncReading) -> String {
        switch r.phase {
        case .unpaired: return SettingsPhoneWords.noMac
        case .waiting: return SettingsPhoneWords.notSyncedYet
        case .inSync: return SettingsPhoneWords.inSync(r.device)
        case .failing: return SettingsPhoneWords.notInSync(r.device)
        }
    }

    /// The second line: the last sync time, or the core's reason while it is failing.
    static func detail(_ r: SyncReading) -> String {
        if r.phase == .failing { return r.reason.isEmpty ? SettingsPhoneWords.syncFailed : r.reason }
        guard let age = r.age(nowMs: nowMs()) else { return "" }
        return SettingsPhoneWords.lastSync(SettingsPhoneWords.ago(age))
    }

    private static func value(_ core: WhooshCore, _ path: String) -> Any? {
        guard let text = core.state(path: path), let data = text.data(using: .utf8) else { return nil }
        return try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])
    }
}
