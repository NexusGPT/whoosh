import SwiftUI

/// The band beside the camera: as tall as the housing, its content in the two wings, never under the camera.
/// Rest: the newest unread mail's account (left) and the unread count with its blue light (right).
/// Arrival: the account's name (left) and "now" (right). List: Latest | Unread (left), Open in Whoosh (right).
/// Open: the chevron back to the list (left), Open in Whoosh and the close mark (right).
struct NotchBand: View {
    let model: NotchModel
    let housing: CGRect
    let shapeWidth: CGFloat

    private var wing: CGFloat { max((shapeWidth - housing.width) / 2, 0) }

    var body: some View {
        HStack(spacing: 0) {
            left.frame(width: wing, alignment: .leading)
            Color.clear.frame(width: housing.width)
            right.frame(width: wing, alignment: .trailing)
        }
        .frame(height: housing.height)
        .transition(.opacity)
        .whooshAx(NotchIds.band)
        // a new mail as one line: a click anywhere on the line opens the mail in place
        .contentShape(Rectangle())
        .gesture(model.stage == .line ? TapGesture().onEnded { if let it = model.dropItem { _ = model.open(id: it.id, AppModel.shared) } } : nil)
    }

    @ViewBuilder private var left: some View {
        switch model.stage {
        case .rest, .light:
            if model.wingsShown, let it = model.newestUnread {
                mark(it.accountId, size: .row).padding(.leading, WhooshSpacing.s12)
            }
        case .line:
            if let it = model.dropItem {
                HStack(spacing: WhooshSpacing.s6) {
                    mark(it.accountId, size: .reply)
                    Text(it.sender).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.label).lineLimit(1)
                }
                .padding(.leading, NotchMetrics.bandInset)
            }
        case .drop:
            if model.notice == nil, let it = model.dropItem {
                HStack(spacing: WhooshSpacing.s6) {
                    mark(it.accountId, size: .reply)
                    Text(RowAccount.of(it.accountId, in: AppModel.shared).name).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                }
                .padding(.leading, NotchMetrics.bandInset)
            }
        case .list:
            if model.search.active {
                // an answer is on screen: the way back to the list (Latest | Unread reads the list, not the answer)
                Button { model.setSearch("", AppModel.shared) } label: {
                    HStack(spacing: WhooshSpacing.s4) {
                        Image(systemName: "chevron.left").font(.system(size: WhooshTypography.caption.size, weight: .semibold))
                        Text(model.settings.show == .unread ? NotchCopy.unread : NotchCopy.latest).whooshText(WhooshTypography.subhead)
                    }
                    .foregroundStyle(WhooshPalette.secondaryLabel)
                    .padding(.horizontal, WhooshSpacing.s6).padding(.vertical, WhooshSpacing.s2)
                    .hoverAck(radius: WhooshRadius.control)
                }
                .buttonStyle(.plain)
                .notchTip(NotchCopy.back)
                .accessibilityLabel(NotchCopy.back)
                .whooshAx(NotchIds.searchBack)
                .padding(.leading, WhooshSpacing.s10)
            } else {
                NotchSegment(model: model).padding(.leading, NotchMetrics.bandInset)
            }
        case .open:
            Button { _ = model.close() } label: {
                HStack(spacing: WhooshSpacing.s4) {
                    Image(systemName: "chevron.left").font(.system(size: WhooshTypography.caption.size, weight: .semibold))
                    Text(NotchCopy.backWord(model)).whooshText(WhooshTypography.subhead)
                }
                .foregroundStyle(WhooshPalette.secondaryLabel)
                .padding(.horizontal, WhooshSpacing.s6).padding(.vertical, WhooshSpacing.s2)
                .hoverAck(radius: WhooshRadius.control)
            }
            .buttonStyle(.plain)
            .notchTip(NotchCopy.back)
            .accessibilityLabel(NotchCopy.back)
            .whooshAx(NotchIds.openBack)
            .padding(.leading, WhooshSpacing.s10)
        }
    }

    @ViewBuilder private var right: some View {
        switch model.stage {
        case .line:
            if let it = model.dropItem {
                HStack(spacing: WhooshSpacing.s6) {
                    Text(it.subject).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                    KeyCap("Enter")
                }
                .padding(.trailing, NotchMetrics.bandInset)
                .whooshAx(NotchIds.line)
            }
        case .rest, .light:
            if model.wingsShown {
                HStack(spacing: WhooshSpacing.s4) {
                    Circle().fill(WhooshPalette.accent).frame(width: WhooshMetrics.warningDot, height: WhooshMetrics.warningDot)
                        .shadow(color: WhooshPalette.focusHalo.color(.dark), radius: WhooshSpacing.s4)
                    Text("\(model.unreadCount)").whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label)
                }
                .padding(.trailing, WhooshSpacing.s12)
                .whooshAx(NotchIds.wings)
                .accessibilityLabel(NotchCopy.unreadCount(model.unreadCount))
            }
        case .drop:
            if model.notice == nil, let it = model.dropItem {
                Text(it.timeWords()).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).padding(.trailing, NotchMetrics.bandInset)
            }
        case .list:
            openInWhoosh(words: true).padding(.trailing, WhooshSpacing.s10)
        case .open:
            HStack(spacing: WhooshSpacing.s2) {
                openInWhoosh(words: false)
                NotchIconButton(symbol: "xmark", title: NotchCopy.close, key: "Escape") { _ = model.collapse() }.whooshAx(NotchIds.openClose)
            }
            .padding(.trailing, WhooshSpacing.s10)
        }
    }

    private func mark(_ accountId: String, size: AccountMarkSize) -> some View {
        let a = RowAccount.of(accountId, in: AppModel.shared)
        return AccountMark(name: a.name, address: a.address, colour: a.colour, icon: a.iconId.map { AccountMarkIcon.library($0) } ?? AccountMarkIcon.providerDefault, size: size,
                           needsAttention: a.needsAttention, hasUnread: false, onSelection: false)
            .notchTip(Words.text("To %@ · %@", a.name, a.address))
    }

    /// Open in Whoosh: the one act that brings the main window up, by the person's click.
    private func openInWhoosh(words: Bool) -> some View {
        Button { _ = model.openInWhoosh(AppModel.shared) } label: {
            HStack(spacing: WhooshSpacing.s6) {
                Image(systemName: "arrow.up.forward.square").font(.system(size: WhooshTypography.subhead.size, weight: .regular))
                if words { Text(NotchCopy.openInWhoosh).whooshText(WhooshTypography.subhead).lineLimit(1) }
            }
            .foregroundStyle(WhooshPalette.secondaryLabel)
            .padding(.horizontal, WhooshSpacing.s6).padding(.vertical, WhooshSpacing.s2)
            .frame(minHeight: words ? 0 : WhooshMetrics.actionButton)
            .hoverAck(radius: WhooshRadius.control)
        }
        .buttonStyle(.plain)
        .notchTip(NotchCopy.openInWhoosh)
        .accessibilityLabel(NotchCopy.openInWhoosh)
        .whooshAx(NotchIds.whoosh)
    }
}

/// Latest | Unread N: the list's two readings, the setting's own value. The on segment is a filled capsule.
struct NotchSegment: View {
    let model: NotchModel

    var body: some View {
        HStack(spacing: WhooshSpacing.s2) {
            segment(NotchCopy.latest, on: model.settings.show == .latest, count: nil, ax: NotchIds.segLatest) { _ = model.setShow(.latest) }
            segment(NotchCopy.unread, on: model.settings.show == .unread, count: model.unreadCount, ax: NotchIds.segUnread) { _ = model.setShow(.unread) }
        }
        .padding(WhooshSpacing.s2)
        .background(WhooshPalette.fill, in: Capsule())
        .accessibilityElement(children: .contain)
    }

    private func segment(_ title: String, on: Bool, count: Int?, ax: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: WhooshSpacing.s4) {
                Text(title).whooshText(WhooshTypography.caption, emphasized: true)
                if let count, count > 0 {
                    Text("\(count)").whooshText(WhooshTypography.caption2, emphasized: true)
                        .foregroundStyle(on ? WhooshPalette.onAccent : WhooshPalette.accent)
                        .padding(.horizontal, WhooshSpacing.s4)
                        .frame(minWidth: WhooshSpacing.s14, minHeight: WhooshSpacing.s14)
                        .background(on ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(Color.clear), in: Capsule())
                }
            }
            .foregroundStyle(on ? WhooshPalette.label : WhooshPalette.secondaryLabel)
            .padding(.horizontal, WhooshSpacing.s8)
            .frame(height: NotchMetrics.segment - 2 * WhooshSpacing.s2)
            .background(on ? AnyShapeStyle(WhooshPalette.keyCap) : AnyShapeStyle(Color.clear), in: Capsule())
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(on ? .isSelected : [])
        .whooshAx(ax)
    }
}
