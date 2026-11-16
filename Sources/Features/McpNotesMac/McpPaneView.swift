import SwiftUI

/// The MCP half of "MCP and Notes": the server's state with Start, then the apps that may search the mail.
/// A calm canvas, one opaque card per group, one accent; glass only on the floating close pill (docs/DESIGN.md, native-glass).
struct McpPaneView: View {
    @State private var session = McpSession.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var clearance = TitleBandClearance.shared
    /// The close pill sits 16 pt below the pane's top (its padding); the title bar's real band decides how much more it needs.
    static let closeTop = WhooshSpacing.s16

    var body: some View {
        ZStack(alignment: .topTrailing) {
            Rectangle().fill(WhooshPalette.panel).ignoresSafeArea()
            ScrollView {
                VStack(alignment: .leading, spacing: WhooshSpacing.s24) {
                    McpHeader()
                    McpServerCard(session: session)
                    McpAppsCard(session: session)
                    Text(McpWords.footer)
                        .whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.tertiaryLabel)
                        .fixedSize(horizontal: false, vertical: true)
                        .padding(.horizontal, WhooshSpacing.s4)
                }
                .frame(maxWidth: WhooshMetrics.readerMeasure, alignment: .leading)
                .padding(.horizontal, WhooshSpacing.s24)
                .padding(.vertical, WhooshSpacing.s32 * 2)
                .frame(maxWidth: .infinity)
            }
            .scrollIndicators(.never)
            #if os(macOS)
            Color.clear.frame(width: 0, height: 0).background(TitleBandAnchor(key: "mcp", firstControl: Self.closeTop))
            #endif
            McpCloseButton().padding(WhooshSpacing.s16).padding(.top, clearance.extra("mcp"))
        }
        .animation(WhooshMotion.animation(WhooshMotion.newRow, reduceMotion: reduceMotion), value: session.ordered)
        .animation(WhooshMotion.animation(WhooshMotion.readerSwap, reduceMotion: reduceMotion), value: session.serverOn)
        .frame(minWidth: McpLayout.windowMin.width, minHeight: McpLayout.windowMin.height)
        .accessibilityIdentifier("mac-mcp-notes")
        #if os(macOS)
        .background(McpWindowChrome())
        #endif
        .onAppear { session.syncFromCore() }
    }
}

private struct McpHeader: View {
    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            Image(systemName: "point.3.connected.trianglepath.dotted")
                .font(.system(size: WhooshTypography.largeTitle.size, weight: .regular))
                .foregroundStyle(WhooshPalette.accent)
                .padding(.bottom, WhooshSpacing.s4)
            Text(McpWords.title).whooshText(WhooshTypography.largeTitle).foregroundStyle(WhooshPalette.label).whooshAx("mac-mcp-notes.title")
            Text(McpWords.lead).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.secondaryLabel).whooshAx("mac-mcp-notes.lead")
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

/// An opaque card on the canvas: the same surface for both groups so the pane reads as two things.
private struct McpCard<Content: View>: View {
    @ViewBuilder let content: Content
    var body: some View {
        content
            .background(RoundedRectangle(cornerRadius: WhooshRadius.sheet, style: .continuous).fill(WhooshPalette.background))
            .overlay(RoundedRectangle(cornerRadius: WhooshRadius.sheet, style: .continuous)
                .strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator))
    }
}

private struct McpServerCard: View {
    let session: McpSession
    var body: some View {
        McpCard {
            HStack(spacing: WhooshSpacing.s12) {
                ZStack {
                    Circle().fill(session.serverOn ? WhooshPalette.mine : WhooshPalette.fill)
                        .frame(width: WhooshMetrics.avatar, height: WhooshMetrics.avatar)
                    Circle().fill(session.serverOn ? WhooshPalette.accent : WhooshPalette.tertiaryLabel)
                        .frame(width: WhooshMetrics.unreadDot + WhooshSpacing.s2, height: WhooshMetrics.unreadDot + WhooshSpacing.s2)
                }
                VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                    Text(session.serverOn ? McpWords.serverOnTitle : McpWords.serverOffTitle)
                        .whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label)
                        .contentTransition(.opacity)
                    Text(session.serverOn ? McpWords.serverOnLine : McpWords.serverOffLine)
                        .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                        .fixedSize(horizontal: false, vertical: true)
                        .contentTransition(.opacity)
                }
                Spacer(minLength: WhooshSpacing.s12)
                if !session.serverOn {
                    McpPillButton(title: McpWords.start, filled: true, id: "mac-mcp-notes.server.start") { _ = session.start() }
                        .transition(.opacity)
                }
            }
            .padding(WhooshSpacing.s16)
            .accessibilityElement(children: .contain)
            .accessibilityIdentifier("mac-mcp-notes.server")
        }
    }
}

private struct McpAppsCard: View {
    let session: McpSession
    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            HStack {
                Text(McpWords.appsHeader).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel).whooshAx("mac-mcp-notes.apps.header")
                Spacer()
                Text(McpWords.allowedCount(session.allowedCount)).whooshText(WhooshTypography.caption)
                    .foregroundStyle(WhooshPalette.tertiaryLabel).contentTransition(.numericText())
            }
            .padding(.horizontal, WhooshSpacing.s4)
            McpCard {
                VStack(spacing: 0) {
                    let apps = session.ordered
                    ForEach(apps) { app in
                        if app.id != apps.first?.id { Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator).padding(.leading, WhooshSpacing.s16 + WhooshMetrics.avatar + WhooshSpacing.s12) }
                        McpAppRow(app: app, session: session).transition(.opacity)
                    }
                }
                .accessibilityIdentifier("mac-mcp-notes.apps")
            }
        }
    }
}

private struct McpAppRow: View {
    let app: McpApp
    let session: McpSession
    var body: some View {
        HStack(spacing: WhooshSpacing.s12) {
            ZStack {
                Circle().fill(app.allowed ? WhooshPalette.accent : WhooshPalette.fill)
                Text(String(app.name.prefix(1)))
                    .whooshText(WhooshTypography.body, emphasized: true)
                    .foregroundStyle(app.allowed ? WhooshPalette.onAccent : WhooshPalette.secondaryLabel)
            }
            .frame(width: WhooshMetrics.avatar, height: WhooshMetrics.avatar)
            VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                Text(app.name).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label).whooshAx("mac-mcp-notes.app.\(app.slug).name")
                Text(app.allowed ? McpWords.allowedLine : McpWords.notAllowedLine)
                    .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                    .lineLimit(1).contentTransition(.opacity)
            }
            Spacer(minLength: WhooshSpacing.s12)
            if app.allowed {
                McpPillButton(title: McpWords.revoke, filled: false, id: "mac-mcp-notes.revoke.\(app.slug)") { session.set(app.name, allowed: false) }
            } else {
                McpPillButton(title: McpWords.allow, filled: true, id: "mac-mcp-notes.allow.\(app.slug)") { session.set(app.name, allowed: true) }
            }
        }
        .padding(.horizontal, WhooshSpacing.s16).padding(.vertical, WhooshSpacing.s12)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("\(app.name), \(app.allowed ? McpWords.allowed : McpWords.notAllowed)")
    }
}

/// The one button shape: a filled accent pill for the move forward, a quiet outline for taking it back.
private struct McpPillButton: View {
    let title: String
    let filled: Bool
    let id: String
    let action: () -> Void
    var body: some View {
        Button(action: action) {
            Text(title).whooshText(WhooshTypography.subhead, emphasized: true)
                .foregroundStyle(filled ? WhooshPalette.onAccent : WhooshPalette.label)
                .padding(.horizontal, WhooshSpacing.s14).frame(height: WhooshMetrics.actionButton)
                .background(Capsule().fill(filled ? WhooshPalette.filledControl : WhooshPalette.fill))
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .hoverAck(radius: WhooshRadius.glassPill)
        .whooshAx(id)   // the pill's id plus a frame `measure` reads
    }
}

private struct McpCloseButton: View {
    var body: some View {
        Button {
            #if os(macOS)
            _ = McpWindowDoor.close(AppModel.shared)
            #endif
        } label: {
            Image(systemName: "xmark").font(.system(size: WhooshTypography.subhead.size, weight: .semibold))
                .foregroundStyle(WhooshPalette.secondaryLabel)
                .frame(width: WhooshSpacing.s32, height: WhooshSpacing.s32)
                .background(Circle().fill(WhooshPalette.background.opacity(0.88)))
                .whooshGlass(in: Circle())
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .hoverAck(radius: WhooshRadius.glassPill)
        .accessibilityLabel("Close")
        .whooshAx("mac-mcp-notes.close")   // the id plus a frame `measure` reads (the J5.7 title-bar check measures the pill)
    }
}
