import SwiftUI

private let M = "mac-mcp-notes"

/// Settings > Outside apps: the requests waiting for your yes, the local server, and the apps that may read your mail.
/// The standalone surface (McpPaneView) keeps its own window; this is the same state drawn as a pane.
struct McpSettingsPane: View {
    @State private var session = McpSession.shared

    var body: some View {
        SettingsPaneScaffold(pane: .outside) {
            if !session.requests.isEmpty {
                SettingsGroup(title: McpWords.waiting(session.requests.count), ax: "\(M).requests") {
                    ForEach(session.requests) { r in
                        SettingsRow(ax: "\(M).request.\(r.id)", title: McpWords.wants(r.app, r.words), caption: r.tool,
                                    leading: AnyView(McpAppIcon(name: r.app))) {
                            HStack(spacing: 8) {
                                SettingsButton(title: McpWords.refuse, ax: "\(M).request.\(r.id).refuse") { _ = session.answer(r.id, allowed: false) }
                                SettingsButton(title: McpWords.allow, primary: true, ax: "\(M).request.\(r.id).allow") { _ = session.answer(r.id, allowed: true) }
                            }
                        }
                        .transition(.opacity)
                    }
                }
            }
            SettingsGroup(title: McpWords.localServer, ax: "\(M).server") {
                SettingsRow(ax: "\(M).server.row", title: session.serverOn ? McpWords.serverOnTitle : McpWords.serverOffTitle,
                            caption: session.serverOn ? McpWords.serverOnLine : McpWords.serverOffLine) {
                    if session.serverOn {
                        SettingsChip(text: Words.text("On"), tone: .good)
                    } else {
                        SettingsButton(title: McpWords.start, primary: true, ax: "\(M).server.start") { _ = session.start() }
                    }
                }
            }
            SettingsGroup(title: McpWords.allowedApps, note: McpWords.allowedCount(session.allowedCount), ax: "\(M).apps") {
                ForEach(session.ordered) { app in
                    SettingsRow(ax: "\(M).apps.\(app.slug)", title: app.name,
                                caption: app.allowed ? McpWords.allowedLine : McpWords.notAllowedLine,
                                leading: AnyView(McpAppIcon(name: app.name, dim: !app.allowed))) {
                        if app.allowed {
                            SettingsButton(title: McpWords.revoke, ax: "\(M).revoke.\(app.slug)") { session.set(app.name, allowed: false) }
                        } else {
                            SettingsButton(title: McpWords.allow, primary: true, ax: "\(M).allow.\(app.slug)") { session.set(app.name, allowed: true) }
                        }
                    }
                    .transition(.opacity)
                }
            }
            SettingsNotice(text: session.failure, ax: "\(M).notice")
            SettingsFootnote(text: McpWords.footer)
        }
        .animation(.easeOut(duration: WhooshMotion.rowInsert), value: session.ordered)
        .animation(.easeOut(duration: WhooshMotion.rowInsert), value: session.requests)
        .onAppear { session.syncFromCore() }
        .accessibilityIdentifier("mac-settings-mcp")
    }
}
