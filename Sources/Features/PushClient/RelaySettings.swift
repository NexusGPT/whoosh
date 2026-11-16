import Foundation

/// The relay address is the user's setting, and there is none built in: Whoosh ships no relay hostname.
/// Anyone can run the open-source relay (relay/README.md, docs/guides/host-your-own-relay.md) and type its
/// address here; the maker's own instance, if any, is recorded outside the code (docs/RELAY-HOSTING.md).
///
/// Setting key `relay_url`, default none = push off. The Settings row belongs to the settings lane; the value
/// is read from the core's state probe at `push_setting/relay_url` (DATA-MAP needs a `push_setting` row; this
/// lane does not edit docs/DATA-MAP.md).
public enum RelaySettings {
    public static let key = "relay_url"
    public static let statePath = "push_setting/relay_url"

    /// What the setting resolves to. `off` is the default and means no registration is attempted.
    public enum Setup {
        case off
        case ready(RelayClient)
        case refused(String)
    }

    /// Reads the setting; the probe may hold the bare text or a JSON string.
    public static func raw(core: WhooshCore) -> String? { core.state(path: statePath) }

    /// Absent, empty or unreadable = push off. https is required, except for a loopback relay (a relay
    /// run on this machine for testing), because the registration carries the device token and routing id.
    /// `allowRemote` is the owner-away guard of RelayClient: the app shell passes true once a remote relay
    /// may be contacted; until then only a loopback relay resolves to `ready`.
    public static func resolve(_ raw: String?, session: URLSession = .shared, allowRemote: Bool = false) -> Setup {
        guard let text = clean(raw), !text.isEmpty else { return .off }
        guard let url = URL(string: text), let scheme = url.scheme?.lowercased(), let host = url.host, !host.isEmpty,
              scheme == "https" || scheme == "http" else { return .refused("relay_url is not an http(s) address") }
        let loopback = ["127.0.0.1", "localhost", "::1"].contains(host)
        if scheme == "http" && !loopback { return .refused("relay_url must be https (plain http is accepted for a loopback relay only)") }
        do { return .ready(try RelayClient(baseURL: url, session: session, loopbackOnly: !allowRemote)) }
        catch { return .refused("a remote relay is not allowed yet: \(host)") }
    }

    public static func resolve(core: WhooshCore, session: URLSession = .shared, allowRemote: Bool = false) -> Setup {
        resolve(raw(core: core), session: session, allowRemote: allowRemote)
    }

    private static func clean(_ raw: String?) -> String? {
        guard let raw = raw?.trimmingCharacters(in: .whitespacesAndNewlines) else { return nil }
        if let data = raw.data(using: .utf8),
           let decoded = try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed]) as? String {
            return decoded.trimmingCharacters(in: .whitespacesAndNewlines)
        }
        return raw
    }
}
