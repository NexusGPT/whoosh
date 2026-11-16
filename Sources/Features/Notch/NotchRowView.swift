import SwiftUI

/// One mail in the hover list (board a): the unread light, the account's emblem, the sender and the time on the first
/// line, the subject and the code pill on the second. Mark read and Archive appear on hover; a click opens the mail in place.
struct NotchRowView: View {
    let item: NotchItem
    var index = 1
    /// The new-mail buttons the setting asks for (GotMail 0.9.0); on the list, Mark read and Archive on hover.
    var buttons: [NotchSettings.Button] = [.markRead, .archive, .reply]
    /// The keyboard is on this row (Down from the search field, then the arrows): it is drawn as the pointer's row is, with the accent's edge.
    var keyed = false
    /// The row's id for a check: a list row by default; a row of the search's answer says so (NotchIds.searchRow).
    var ax: String?
    /// A row of the search's answer has a third line: the passage that made the mail a result (else its first words).
    var detail: String?
    @State private var hovering = false

    var body: some View {
        HStack(spacing: WhooshSpacing.s10) {
            HStack(spacing: WhooshSpacing.s6) {
                Circle().fill(WhooshPalette.accent).frame(width: WhooshMetrics.warningDot, height: WhooshMetrics.warningDot).opacity(item.read ? 0 : 1)
                mark
            }
            VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s8) {
                    Text(item.sender).whooshText(WhooshTypography.body, emphasized: !item.read).foregroundStyle(item.read ? WhooshPalette.secondaryLabel : WhooshPalette.label)
                    Spacer(minLength: 0)
                    Text(item.timeWords()).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                }
                HStack(spacing: WhooshSpacing.s8) {
                    Text(item.subject).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                    Spacer(minLength: 0)
                    if hovering || keyed { actions.transition(.opacity) }
                    else if let code = item.code { NotchCodeControls(code: code, copied: item.codeCopied) }
                }
                if let detail, !detail.isEmpty {
                    Text(detail).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.tertiaryLabel)
                }
            }
            .lineLimit(1)
            .truncationMode(.tail)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.leading, WhooshSpacing.s6)
        .padding(.trailing, WhooshSpacing.s10)
        .frame(height: detail?.isEmpty == false ? NotchMetrics.searchRow : NotchMetrics.row)
        .background(hovering || keyed ? AnyShapeStyle(WhooshPalette.hover) : AnyShapeStyle(Color.clear),
                    in: RoundedRectangle(cornerRadius: NotchMetrics.rowCorner, style: .continuous))
        .overlay {
            if keyed {
                RoundedRectangle(cornerRadius: NotchMetrics.rowCorner, style: .continuous).strokeBorder(WhooshPalette.accent.opacity(0.7), lineWidth: WhooshMetrics.separator)
            }
        }
        .contentShape(Rectangle())
        .onHover { h in withAnimation(WhooshMotion.hover) { hovering = h } }
        .onTapGesture { NotchModel.shared.openRow(item, AppModel.shared) }
        .whooshAx(ax ?? NotchIds.row(index))
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityHint(Words.text("Opens the mail here"))
    }

    private var mark: some View {
        let a = RowAccount.of(item.accountId, in: AppModel.shared)
        return AccountMark(name: a.name, address: a.address, colour: a.colour, icon: a.iconId.map { AccountMarkIcon.library($0) } ?? AccountMarkIcon.providerDefault, size: .sidebar,
                           needsAttention: a.needsAttention, hasUnread: false, onSelection: false)
            .notchTip(Words.text("To %@ · %@", a.name, a.address))
    }

    private var actions: some View {
        HStack(spacing: WhooshSpacing.s2) {
            if buttons.contains(.markRead) && !item.read {
                NotchIconButton(symbol: "envelope.open", title: NotchCopy.markRead, key: NotchFeature.readKey) { act { id, m in NotchModel.shared.markRead(id: id, m) } }
            }
            if buttons.contains(.reply) {   // opens the mail in place, the quick reply ready
                NotchIconButton(symbol: "arrowshape.turn.up.left", title: NotchCopy.reply) { NotchModel.shared.openRow(item, AppModel.shared, reply: true) }
            }
            if buttons.contains(.delete) {
                NotchIconButton(symbol: "trash", title: NotchCopy.delete) { act { id, m in NotchModel.shared.delete(id: id, m) } }
            }
            if buttons.contains(.archive) {
                NotchIconButton(symbol: "archivebox", title: NotchCopy.archive, key: NotchFeature.archiveKey) { act { id, m in NotchModel.shared.archive(id: id, m) } }
            }
        }
    }

    /// A row's button on its mail: a mail of the search's answer joins the notch's mails first (NotchModel.act), then the act runs as on any row.
    private func act(_ run: (String, AppModel) -> Bool) {
        NotchModel.shared.act(on: item, AppModel.shared, run)
    }
}

/// The compact code pill (the shared Components pill) and, beside it, Fill: the list row's and the open mail's form.
struct NotchCodeControls: View {
    let code: String
    let copied: Bool

    var body: some View {
        HStack(spacing: WhooshSpacing.s4) {
            if copied {
                Label(NotchCopy.copied, systemImage: "checkmark")
                    .whooshText(WhooshTypography.caption, emphasized: true)
                    .foregroundStyle(WhooshPalette.accent)
                    .padding(.horizontal, WhooshSpacing.s6)
                    .frame(height: WhooshMetrics.keyCap)
                    .background(WhooshPalette.fill, in: RoundedRectangle(cornerRadius: WhooshRadius.item, style: .continuous))
                    .transition(.opacity)
                    .accessibilityLabel(Words.text("Code %@ copied", code))
            } else {
                CodePill(code) { _ in NotchFeature.copyCode() }.whooshAx(NotchIds.codePill, radius: WhooshRadius.item)
            }
            Button(NotchCopy.fill) { NotchFeature.fillCode() }
                .buttonStyle(.plain)
                .whooshText(WhooshTypography.caption, emphasized: true)
                .foregroundStyle(WhooshPalette.accent)
                .padding(.horizontal, WhooshSpacing.s6)
                .frame(height: WhooshMetrics.keyCap)
                .hoverAck(radius: WhooshRadius.item)
                .notchTip(NotchCopy.fillHelp)
                .whooshAx(NotchIds.fill)
        }
    }
}

/// A small icon button with its name in the tooltip and for VoiceOver.
struct NotchIconButton: View {
    let symbol: String
    let title: String
    /// The control's key, shown in its tip where the button has no room for a cap.
    var key: String? = nil
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: WhooshMetrics.actionIcon * 0.8, weight: .regular))
                .foregroundStyle(WhooshPalette.secondaryLabel)
                .frame(width: WhooshMetrics.actionButton, height: WhooshMetrics.actionButton)
                .hoverAck(radius: WhooshRadius.control)
        }
        .buttonStyle(.plain)
        .notchTip(key.map { "\(title) · \(KeyCap.glyph(for: $0))" } ?? title)
        .accessibilityLabel(title)
    }
}
