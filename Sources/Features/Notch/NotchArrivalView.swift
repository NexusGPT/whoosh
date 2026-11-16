import SwiftUI

/// The arrival card (board a): who and what, then the code block when the mail carries a sign-in code (the code is
/// the biggest thing on the screen, Copy and Fill beside it, one line under it saying what Fill does), else the
/// first lines and the buttons. Owner's pick of 14 Nov 2026 (look 2A): Open (the one filled button) and Reply on the left, Archive and Mark read
/// on the right; Mark read and Delete show their picture and key only, so every button fits the card. Open is always there; Reply, Archive, Mark read
/// and Delete follow the setting. A click anywhere opens the mail in place.
struct NotchArrivalView: View {
    let item: NotchItem
    var buttons: [NotchSettings.Button] = [.markRead, .archive, .reply]

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
            Text(item.sender).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label).lineLimit(1)
            Text(item.subject).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
            if let code = item.code {
                NotchCodeBlock(code: code, copied: item.codeCopied).padding(.top, WhooshSpacing.s10)
                Text(NotchCopy.fillHelp).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.tertiaryLabel).padding(.top, WhooshSpacing.s6)
            } else {
                if !item.preview.isEmpty {
                    Text(item.preview).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label)
                        .lineLimit(2).fixedSize(horizontal: false, vertical: true).padding(.top, WhooshSpacing.s6)
                }
                HStack(spacing: WhooshSpacing.s6) {
                    NotchGhostButton(symbol: nil, title: NotchCopy.open, key: "Enter", primary: true) { NotchModel.shared.open(id: item.id, AppModel.shared) }
                        .whooshAx(NotchIds.arrivalOpen)
                    if buttons.contains(.reply) {   // opens the mail in place, the quick reply ready
                        NotchGhostButton(symbol: "arrowshape.turn.up.left", title: NotchCopy.reply, key: NotchFeature.replyKey) { NotchModel.shared.open(id: item.id, AppModel.shared, reply: true) }
                            .whooshAx(NotchIds.arrivalReply)
                    }
                    Spacer(minLength: 0)
                    if buttons.contains(.archive) {
                        NotchGhostButton(symbol: "archivebox", title: NotchCopy.archive, key: NotchFeature.archiveKey) { NotchModel.shared.archive(id: item.id, AppModel.shared) }
                            .whooshAx(NotchIds.arrivalArchive)
                    }
                    if buttons.contains(.markRead) && !item.read {
                        NotchGhostButton(symbol: "envelope.open", title: NotchCopy.markRead, key: NotchFeature.readKey, titleShown: false) { NotchModel.shared.markRead(id: item.id, AppModel.shared) }
                            .whooshAx(NotchIds.arrivalMarkRead)
                    }
                    if buttons.contains(.delete) {
                        NotchGhostButton(symbol: "trash", title: NotchCopy.delete, titleShown: false) { NotchModel.shared.delete(id: item.id, AppModel.shared) }
                            .whooshAx(NotchIds.arrivalDelete)
                    }
                }
                .padding(.top, WhooshSpacing.s10)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .contentShape(Rectangle())
        .onTapGesture { NotchModel.shared.open(id: item.id, AppModel.shared) }
        .accessibilityElement(children: .contain)
        .accessibilityAddTraits(.isButton)
        .accessibilityHint(Words.text("Opens the mail here"))
    }
}

/// The code block (board a): a key mark, the digits in groups, Copy, and Fill as the one filled control.
/// Copy turns into "Copied" on the same frame; the mail is marked read, never archived (NotchModel.copyCode).
struct NotchCodeBlock: View {
    let code: String
    let copied: Bool

    /// Six digits read as two groups of three, eight as two of four; anything else as it came.
    static func grouped(_ code: String) -> String {
        let half = code.count / 2
        guard code.allSatisfy(\.isNumber), [6, 8].contains(code.count) else { return code }
        return String(code.prefix(half)) + " " + String(code.suffix(code.count - half))
    }

    var body: some View {
        HStack(spacing: WhooshSpacing.s10) {
            Image(systemName: "key.fill").font(.system(size: WhooshTypography.subhead.size, weight: .regular)).foregroundStyle(WhooshPalette.secondaryLabel)
            Text(Self.grouped(code))
                .font(.system(size: WhooshTypography.largeTitle.size, weight: .semibold, design: .rounded))
                .monospacedDigit()
                .tracking(WhooshSpacing.s2)
                .foregroundStyle(WhooshPalette.label)
                .lineLimit(1)
                .accessibilityLabel(Self.grouped(code))
            Spacer(minLength: WhooshSpacing.s8)
            Button { NotchFeature.copyCode() } label: {
                HStack(spacing: WhooshSpacing.s6) {
                    Image(systemName: copied ? "checkmark" : "doc.on.doc").font(.system(size: WhooshTypography.subhead.size, weight: .semibold))
                    Text(copied ? NotchCopy.copied : NotchCopy.copy).whooshText(WhooshTypography.subhead, emphasized: true)
                }
                .foregroundStyle(copied ? WhooshPalette.accent : WhooshPalette.label)
                .padding(.horizontal, WhooshSpacing.s12)
                .frame(height: NotchMetrics.button)
                .background(WhooshPalette.keyCap, in: Capsule())
                .hoverAck(radius: NotchMetrics.button / 2)
                .contentShape(Capsule())
            }
            .buttonStyle(.plain)
            .notchTip(copied ? NotchCopy.copied : NotchCopy.copy)
            .accessibilityLabel(copied ? NotchCopy.copied : NotchCopy.copy)
            .whooshAx(NotchIds.codePill)
            Button { NotchFeature.fillCode() } label: {
                HStack(spacing: WhooshSpacing.s6) {
                    Image(systemName: "text.cursor").font(.system(size: WhooshTypography.subhead.size, weight: .semibold))
                    Text(NotchCopy.fill).whooshText(WhooshTypography.subhead, emphasized: true)
                }
                .foregroundStyle(WhooshPalette.onAccent)
                .padding(.horizontal, WhooshSpacing.s12)
                .frame(height: NotchMetrics.button)
                .background(WhooshPalette.accent, in: Capsule())
                .hoverAck(radius: NotchMetrics.button / 2)
                .contentShape(Capsule())
            }
            .buttonStyle(.plain)
            .notchTip(NotchCopy.fillHelp)
            .accessibilityLabel(NotchCopy.fill)
            .whooshAx(NotchIds.fill)
        }
        .padding(.leading, WhooshSpacing.s14)
        .padding(.trailing, WhooshSpacing.s8)
        .frame(height: NotchMetrics.codeBlock)
        .background(WhooshPalette.fill, in: RoundedRectangle(cornerRadius: NotchMetrics.codeCorner, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: NotchMetrics.codeCorner, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator))
    }
}
