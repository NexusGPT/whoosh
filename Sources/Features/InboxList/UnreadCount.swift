import SwiftUI

/// The Unread filter as the count line's own control (owner 2026-11-10, board docs/design/boards/unread-filter-r1, pick F2
/// "The count is the switch"): the "N unread" under the search field is the switch, with the ⇧U key cap beside it. Off: the count in
/// secondary text. On: the count in accent and semibold, then "· Show all" in secondary, and the list shows the unread rows only.
/// A click on the count or ⇧U toggles it: both run `ListFilters.toggle(.unread, model)`, so the filter applies to whatever tab is open, view
/// or label (contract delta 10: `InboxQuery.filters`). It keeps the accessibility id the chip had, `mac-main.splits.unread`.
/// The change of state is the colour and the "Show all" fading (opacity only, HOVER_TINT_MS); the rows leaving or returning glide as a removal does.
struct UnreadCount: View {
    let model: AppModel
    private let list = InboxListState.shared   // @Observable: the body re-reads `filter` when it changes
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var on: Bool { list.filter == .unread }
    private var key: String { LiveKeys.chord(ListFilter.unread.command, ListFilter.unread.fallbackKey) }

    /// The words the control draws, as plain text: the count, and the "· Show all" tail only while the filter is on.
    /// The view and the `mac-main.list.state` probe both read this, so what is checked is what is drawn.
    @MainActor static func words(on: Bool, count: Int) -> (count: String, tail: String?) {
        (Words.text("%lld unread", count), on ? "\u{00B7} " + Words.text("Show all") : nil)
    }

    /// The colour of the count: accent while the filter is on, secondary while off. The view and the probe's `count_style` both read this.
    enum CountStyle: String { case accent, secondary }
    static func countStyle(on: Bool) -> CountStyle { on ? .accent : .secondary }

    var body: some View {
        let w = Self.words(on: on, count: ListHeaderWords.unread(model))
        return Button { _ = ListFilters.toggle(.unread, model) } label: {
            HStack(spacing: WhooshSpacing.s6) {
                Text(w.count).font(WhooshTypography.caption.font(emphasized: on)).monospacedDigit()
                    .foregroundStyle(Self.countStyle(on: on) == .accent ? WhooshPalette.accent : WhooshPalette.secondaryLabel)
                if let tail = w.tail {
                    Text(tail).font(WhooshTypography.caption.font(emphasized: false)).foregroundStyle(WhooshPalette.secondaryLabel)
                        .transition(.opacity)
                }
                KeyCap(key).fixedSize()
            }
            .lineLimit(1).fixedSize()
            .frame(height: WhooshMetrics.listHeaderHeight)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(on ? Words.text("Showing only unread · %@ shows all", key) : Words.text("Show only unread · %@", key))
        .animation(WhooshMotion.animation(WhooshMotion.hover, reduceMotion: reduceMotion), value: on)
        .whooshAx("mac-main.splits.unread")
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}
