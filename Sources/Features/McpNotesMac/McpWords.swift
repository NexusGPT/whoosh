import Foundation

/// Every word the pane shows, in one place (one source for the i18n pass later).
enum McpWords {
    static var title: String { Words.text("Outside AI apps") }
    static var paletteTitle: String { Words.text("Outside AI apps (MCP)") }
    static var lead: String { Words.text("Let an app such as Claude Desktop search your mail through Whoosh. It sees nothing until you allow it, and every search asks you first.") }
    static var serverOnTitle: String { Words.text("Local server is on") }
    static var serverOnLine: String { Words.text("Allowed apps can reach it from this Mac. Nothing leaves this Mac.") }
    static var serverOffTitle: String { Words.text("Local server is off") }
    static var serverOffLine: String { Words.text("The server is off, so no app can reach your mail. Start it to let allowed apps connect.") }
    static var start: String { Words.text("Start") }
    static var appsHeader: String { Words.text("Apps") }
    static var allow: String { Words.text("Allow") }
    static var revoke: String { Words.text("Revoke") }
    static var allowed: String { Words.text("Allowed") }
    static var notAllowed: String { Words.text("Not allowed") }
    static var allowedLine: String { Words.text("Can search your mail after you approve each search") }
    static var notAllowedLine: String { Words.text("Gets nothing from your mail") }
    static var footer: String { Words.text("Whoosh answers only over a connection on this Mac. Revoking an app ends its access at once.") }

    static var waitingTitle: String { Words.text("Waiting for you") }
    static func waiting(_ n: Int) -> String { Words.text("Waiting for you · %lld", n) }
    static var allowedApps: String { Words.text("Allowed apps") }
    static var localServer: String { Words.text("Local server") }
    static var answerFailed: String { Words.text("The answer was not saved. Try again.") }
    static var refuse: String { Words.text("Refuse") }
    static func wants(_ app: String, _ words: String) -> String { Words.text("%@ wants to %@", app, words) }

    static func allowedCount(_ n: Int) -> String { n == 0 ? Words.text("None allowed yet") : Words.text("%lld allowed", n) }
    static var grantUnsaved: String { Words.text("Saved for this session only. The app's core has no command yet to keep an allowed app (setMcpGrant).") }
    static func slug(_ app: String) -> String { app.lowercased().replacingOccurrences(of: " ", with: "-") }
}
