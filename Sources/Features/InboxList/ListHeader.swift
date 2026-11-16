import SwiftUI

/// The chords this list names in words, read from the live preset (Superhuman's in the default), never typed.
@MainActor enum LiveKeys {
    static var search: String { ShortcutEngine.shared.chord(for: "search") ?? "⌘K" }
    static var palette: String { ShortcutEngine.shared.chord(for: "palette") ?? "⌘P" }
    /// the key bound to a command in the live preset; the fallback is Superhuman's
    static func chord(_ command: String, _ fallback: String) -> String { ShortcutEngine.shared.chord(for: command) ?? fallback }
    /// a control's tooltip: its word and its key ("Archive · E")
    static func help(_ word: String, _ command: String, _ fallback: String) -> String { Words.text(word) + " · " + chord(command, fallback) }
    /// a tab's tooltip: its name and the key that steps through the tabs
    /// a view's tooltip also says what the view holds (`about`); a label's names the tab only
    static func tabHelp(_ name: String, about: String? = nil) -> String {
        (about.map { name + " · " + $0 } ?? name) + " · " + chord("next-split", "Tab") + " " + chord("previous-split", "⇧Tab")
    }
}

/// The glass command field above the list (the v2 drawing's `.cmd`): "Search or type a command" and ⌘K. It is the
/// one search: a click or ⌘K opens it. A pane too narrow for the words keeps the one round entry (`.cmdc`).
struct CommandField: View {
    let model: AppModel

    var body: some View {
        Group {
            if SearchMacModel.shared.active {
                SearchField(model: SearchMacModel.shared, app: model)   // ⌘K or a click: the pill becomes the live search field, in place
            } else {
                ViewThatFits(in: .horizontal) {
                    wide
                    narrow
                }
            }
        }
        .padding(.horizontal, WhooshMetrics.listRowInset + WhooshSpacing.s4)
        .whooshAx("mac-main.list.command")
    }

    private var wide: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Image(systemName: "magnifyingglass").font(WhooshTypography.subhead.font(emphasized: false))
            Text(Words.text("Search or type a command")).font(WhooshTypography.subhead.font(emphasized: false)).lineLimit(1)
            Spacer(minLength: WhooshSpacing.s4)
            KeyCap(LiveKeys.search)
        }
        .foregroundStyle(WhooshPalette.secondaryLabel)
        .padding(.leading, WhooshSpacing.s12).padding(.trailing, WhooshSpacing.s4)
        .frame(height: WhooshMetrics.glassPillHeight)
        .contentShape(Capsule())
        .onTapGesture { _ = model.press(LiveKeys.search) }
        .whooshGlass(in: Capsule())
    }

    private var narrow: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Spacer(minLength: 0)
            Button { _ = model.press(LiveKeys.search) } label: { round("magnifyingglass", WhooshPalette.secondaryLabel) }
                .buttonStyle(.plain).accessibilityLabel(Words.text("Search or type a command"))
        }
    }

    private func round(_ symbol: String, _ tint: ThemeColor) -> some View {
        Image(systemName: symbol).font(WhooshTypography.subhead.font(emphasized: false)).foregroundStyle(tint)
            .frame(width: WhooshMetrics.glassPillHeight, height: WhooshMetrics.glassPillHeight)
            .whooshGlass(in: Circle())
    }
}

/// "All accounts · 23 unread" (the drawing's `.lh`): the account filter's name on the left, the split's unread on the right.
struct ListHeader: View {
    let model: AppModel

    var body: some View {
        HStack(spacing: WhooshSpacing.s4) {
            if let folder = SmartFolderWords.name(model) {
                // a saved smart folder from the sidebar: its own name, the count of the rows its rule matches
                Text(folder).font(WhooshTypography.caption.font(emphasized: true)).foregroundStyle(WhooshPalette.label)
            } else if let box = model.mailbox {
                // a mailbox from the sidebar: its name and how many conversations it holds, in place of the account filter
                Text(MailboxWords.name(box)).font(WhooshTypography.caption.font(emphasized: true)).foregroundStyle(WhooshPalette.label)
            } else {
                Text(ListHeaderWords.filterName(model)).font(WhooshTypography.caption.font(emphasized: true)).foregroundStyle(WhooshPalette.label)
            }
            Spacer(minLength: WhooshSpacing.s8)
            if (model.mailbox == nil && model.smartFolderId == nil) || InboxListState.shared.filter == .unread {
                UnreadCount(model: model)   // the count is the Unread switch (a mailbox or folder keeps it while the filter is on, so it can be turned off)
            } else {
                Text(Words.text("%lld conversations", model.rows.count))
                    .font(WhooshTypography.caption.font(emphasized: false)).foregroundStyle(WhooshPalette.secondaryLabel).monospacedDigit()
            }
        }
        .lineLimit(1)
        .padding(.horizontal, WhooshMetrics.listRowInset + WhooshSpacing.s12)
        .frame(height: WhooshMetrics.listHeaderHeight)
        .accessibilityElement(children: .contain)   // the count is its own control (UnreadCount), so the header holds it instead of combining it
        .accessibilityLabel(ListHeaderWords.line(model))
        .whooshAx("mac-main.list.header")
    }
}
