import Foundation

/// The one state the Settings provider row shows for on-device AI: ready, downloading with a percentage, or
/// unavailable with the reason. Pure: it reads the backends and the install state, nothing else.
public struct OnDeviceStatus: Equatable, Sendable {
    public enum Kind: String, Sendable { case ready, downloading, unavailable }
    public let kind: Kind
    /// One sentence the pane shows as it is. Never empty.
    public let words: String
    /// 0...100 while downloading (100 while checking the download), otherwise nil.
    public let percent: Int?
    /// The name of the backend that answers when ready, otherwise empty.
    public let backend: String

    public static func make(backends: [any TextBackend], install: ModelInstallState) -> OnDeviceStatus {
        if let ready = backends.first(where: { $0.availability().isReady }) {
            return OnDeviceStatus(kind: .ready, words: LocalModelAvailability.ready.words, percent: nil, backend: ready.name)
        }
        if install.isBusy, let f = install.fraction {
            return OnDeviceStatus(kind: .downloading, words: install.words, percent: Int(f * 100), backend: "")
        }
        let reason = OnDeviceTextEngine(backends: backends).availability().words
        return OnDeviceStatus(kind: .unavailable, words: reason, percent: nil, backend: "")
    }

    /// What the socket's `rows` verb reads at `mac-settings-ai.provider.local`.
    public var row: [String: Any] {
        ["state": kind.rawValue, "words": words, "percent": percent ?? 0, "backend": backend,
         "reason": kind == .unavailable ? words : ""]
    }
}
