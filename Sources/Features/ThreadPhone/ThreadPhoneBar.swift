import SwiftUI

/// The floating glass bar: Reply (the wide pill) · Archive · More. Same glass, sizes and press feedback as the inbox bar.
/// More opens a glass panel above it (Note, Mark unread, Pin, Snooze); the panel is a view, not a system menu, so a
/// driven check can open it and tap each item.
struct ThreadPhoneBar: View {
    let state: ThreadPhoneState
    private let m = AppModel.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        HStack(spacing: PhoneMetrics.barGap) {
            Button { _ = state.reply(m) } label: {
                HStack(spacing: WhooshSpacing.s8) {
                    Image(systemName: "arrowshape.turn.up.left").font(.system(size: PhoneType.headline.size, weight: .semibold))
                    Text(ThreadPhoneWords.reply).phoneText(PhoneType.body)
                    Spacer(minLength: 0)
                }
                .foregroundStyle(WhooshPalette.label)
                .padding(.leading, PhoneMetrics.gutter)
                .frame(maxWidth: .infinity).frame(height: PhoneMetrics.barButton)
                .whooshGlass(in: Capsule()).contentShape(Capsule())
            }
            .buttonStyle(PressStyle()).accessibilityIdentifier("iphone-thread.reply")
            Button { _ = state.leave(.archive, m) } label: { glyph("archivebox") }
                .buttonStyle(PressStyle()).accessibilityLabel(ThreadPhoneWords.archive).accessibilityIdentifier("iphone-thread.archive")
            Button { state.moreOpen.toggle() } label: { glyph("ellipsis") }
                .buttonStyle(PressStyle()).accessibilityLabel(ThreadPhoneWords.more)
                .accessibilityValue(state.moreOpen ? "open" : "").accessibilityIdentifier("iphone-thread.more")
        }
        .padding(.horizontal, PhoneMetrics.gutter)
    }

    private func glyph(_ symbol: String) -> some View {
        Image(systemName: symbol).font(.system(size: PhoneType.title3.size + WhooshSpacing.s2, weight: .semibold))
            .foregroundStyle(WhooshPalette.label)
            .frame(width: PhoneMetrics.barButton, height: PhoneMetrics.barButton).whooshGlass(in: Circle())
    }
}

/// The More panel: one glass card above the bar's trailing end, four rows of 52 pt (`groupRow`).
struct ThreadMorePanel: View {
    let state: ThreadPhoneState
    let pinned: Bool
    private let m = AppModel.shared

    var body: some View {
        VStack(spacing: 0) {
            row("note.text", ThreadPhoneWords.note, id: "note") { state.openNote(m) }
            row("envelope.badge", ThreadPhoneWords.markUnread, id: "unread") { _ = state.markUnread(m) }
            row("pin", pinned ? ThreadPhoneWords.unpin : ThreadPhoneWords.pin, id: "pin") { _ = state.togglePin(m) }
            row("clock", ThreadPhoneWords.snooze, id: "snooze") { _ = state.leave(.snooze, m) }
        }
        .frame(width: ThreadMetrics.menuWidth)
        .whooshGlass(in: RoundedRectangle(cornerRadius: PhoneMetrics.cardRadius, style: .continuous))
        .accessibilityElement(children: .contain).accessibilityIdentifier("iphone-thread.more.panel")
    }

    private func row(_ symbol: String, _ title: String, id: String, _ act: @escaping () -> Void) -> some View {
        Button {
            state.moreOpen = false
            act()
        } label: {
            HStack(spacing: WhooshSpacing.s12) {
                Image(systemName: symbol).frame(width: WhooshSpacing.s24)
                Text(title).phoneText(PhoneType.body)
                Spacer(minLength: 0)
            }
            .foregroundStyle(WhooshPalette.label)
            .padding(.horizontal, PhoneMetrics.gutter).frame(height: PhoneMetrics.groupRow).contentShape(Rectangle())
        }
        .buttonStyle(PressStyle()).accessibilityIdentifier("iphone-thread.more.\(id)")
    }
}

/// A refused Archive or Snooze, in words, above the bar: the thread stays and says why.
struct ThreadFailureLine: View {
    let words: String
    let dismiss: () -> Void

    var body: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Circle().fill(WhooshPalette.warning).frame(width: PhoneMetrics.unreadDot, height: PhoneMetrics.unreadDot)
            Text(words).foregroundStyle(WhooshPalette.label).lineLimit(2)
            Spacer(minLength: 0)
            Button(Words.text("OK"), action: dismiss).buttonStyle(.plain).foregroundStyle(WhooshPalette.selection)
                .phoneText(PhoneType.subhead, emphasized: true).frame(minWidth: PhoneMetrics.roundButton, minHeight: PhoneMetrics.roundButton)
                .accessibilityIdentifier("iphone-thread.failure.ok")
        }
        .phoneText(PhoneType.subhead)
        .padding(.leading, WhooshSpacing.s20).padding(.trailing, WhooshSpacing.s6)
        .frame(width: PhoneMetrics.undoWidth).frame(minHeight: PhoneMetrics.roundButton)
        .whooshGlass(in: Capsule())
        .accessibilityElement(children: .contain).accessibilityIdentifier("iphone-thread.failure")
    }
}
