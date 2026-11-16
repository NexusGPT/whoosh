import SwiftUI

/// What the grown shape holds under the band, one region per stage. Every swap between stages is a cross-fade
/// under the shape's growth; nothing inside moves on its own.
struct NotchCard: View {
    let model: NotchModel

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s6) {
            Group {
                switch model.stage {
                case .rest, .light, .line: EmptyView()
                case .drop: dropContent
                case .list: NotchListView(model: model)
                case .open: if let item = model.openItem { NotchOpenView(item: item, model: model) }
                }
            }
            .transition(.opacity)
            if let words = model.errorWords {
                Label(words, systemImage: "exclamationmark.circle")
                    .whooshText(WhooshTypography.caption)
                    .foregroundStyle(WhooshPalette.warning)
                    .fixedSize(horizontal: false, vertical: true)
                    .transition(.opacity)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    @ViewBuilder private var dropContent: some View {
        if let n = model.notice {
            NotchNoticeView(notice: n) { model.dismissNotice() }
        } else if let first = model.dropItem {
            NotchArrivalView(item: first, buttons: model.settings.buttons)
        }
    }
}

/// The one-time "opened" notice for a tracked message.
struct NotchNoticeView: View {
    let notice: NotchNotice
    let dismiss: () -> Void

    var body: some View {
        HStack(spacing: WhooshSpacing.s10) {
            Image(systemName: "eye")
                .font(.system(size: WhooshMetrics.actionIcon, weight: .regular))
                .foregroundStyle(WhooshPalette.accent)
                .frame(width: WhooshMetrics.avatar, height: WhooshMetrics.avatar)
            Text(notice.line)
                .whooshText(WhooshTypography.body)
                .foregroundStyle(WhooshPalette.label)
                .lineLimit(2)
                .frame(maxWidth: .infinity, alignment: .leading)
            NotchIconButton(symbol: "xmark", title: NotchCopy.close, key: "Escape", action: dismiss)
        }
        .frame(minHeight: WhooshMetrics.listRow)
        .accessibilityElement(children: .combine)
    }
}

/// "That's everything": the plain end mark (board a). Alone when the list is empty; under the Unread list otherwise,
/// with the unread count as its second line.
struct NotchDoneRow: View {
    let title: String
    var detail: String?

    var body: some View {
        HStack(spacing: WhooshSpacing.s12) {
            ZStack {
                Circle().fill(WhooshPalette.accent.opacity(0.16))
                Image(systemName: "checkmark").font(.system(size: WhooshTypography.subhead.size, weight: .semibold)).foregroundStyle(WhooshPalette.accent)
            }
            .frame(width: WhooshMetrics.actionButton, height: WhooshMetrics.actionButton)
            VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                Text(title).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label)
                if let detail { Text(detail).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel) }
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, WhooshSpacing.s8)
        .frame(minHeight: detail == nil ? WhooshMetrics.listRow : NotchMetrics.doneRow)
        .accessibilityElement(children: .combine)
    }
}

/// A quiet pill button with its key cap: Archive E, Mark read U, Copy (board a's ghost button).
struct NotchGhostButton: View {
    /// nil: the button shows its title and key only (the card's Open).
    let symbol: String?
    let title: String
    var key: String?
    var height: CGFloat = NotchMetrics.smallButton
    /// The one filled control of a card (owner's pick 2A, 14 Nov 2026: black, and one blue for the main button).
    var primary = false
    /// false: the picture and the key only, the title in the tip and for VoiceOver (mark read and snooze on a card, so every button fits).
    var titleShown = true
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: WhooshSpacing.s6) {
                if let symbol {
                    Image(systemName: symbol).font(.system(size: WhooshTypography.subhead.size, weight: .regular))
                        .foregroundStyle(primary ? AnyShapeStyle(WhooshPalette.onAccent) : AnyShapeStyle(WhooshPalette.secondaryLabel))
                }
                if titleShown {
                    Text(title).whooshText(WhooshTypography.subhead, emphasized: true)
                        .foregroundStyle(primary ? AnyShapeStyle(WhooshPalette.onAccent) : AnyShapeStyle(WhooshPalette.label))
                }
                if let key { KeyCap(key, onSelection: primary) }
            }
            .padding(.leading, WhooshSpacing.s10)
            .padding(.trailing, key == nil ? WhooshSpacing.s10 : WhooshSpacing.s6)
            .frame(height: height)
            .background(primary ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(WhooshPalette.fill), in: Capsule())
            .hoverAck(radius: height / 2)
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .notchTip(key.map { "\(title) · \(KeyCap.glyph(for: $0))" } ?? title)
        .accessibilityLabel(title)
    }
}
