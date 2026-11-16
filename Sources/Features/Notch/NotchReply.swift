import SwiftUI

/// The notch's quick reply (board a): one box with the From line (the account's emblem and address, a chevron to pick
/// another) and the field with its send key and the one filled send control. The behaviour is the QuickReply lane's model
/// (send to the outbox, the undo window, the refusal words); the view is sized for the notch (docs/lanes/notch.md N8, N13).
/// Return sends, Option-Return breaks a line; ⌃Enter sends from anywhere in the app (NotchFeature.keyBindings).
struct NotchReply: View {
    let model: NotchModel
    @Bindable var app = AppModel.shared
    @Bindable var qr = QuickReplyModel.shared
    @FocusState private var focused: Bool

    /// The send key this card shows: ⌘↵, the composer's, as on every reply control (spec A145). ⌃↵ and plain Return still send, unshown.
    static let sendCap = QuickReplyCopy.sendCap

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s6) {
            if qr.isCounting || model.sentShown {
                NotchSendBar(model: model)
            } else {
                VStack(alignment: .leading, spacing: WhooshSpacing.s6) {
                    // From on the left; on the right the AI pill of the main window's reply (⌘J writes a draft), the same control and the same model
                    HStack(spacing: WhooshSpacing.s6) {
                        fromLine
                        Spacer(minLength: WhooshSpacing.s6)
                        QuickReplyAIPill(model: qr, app: app)
                    }
                    if qr.fromOpen { fromPicks.transition(.opacity) }
                    if qr.ai.phase == .asking { QuickReplyInstructionRow(model: qr, app: app) }
                    HStack(alignment: .center, spacing: WhooshSpacing.s8) {
                        TextField(qr.context.placeholder, text: $qr.draft, axis: .vertical)
                            .textFieldStyle(.plain)
                            .whooshText(WhooshTypography.body)
                            .foregroundStyle(WhooshPalette.label)
                            .lineLimit(1...3)
                            .focused($focused)
                            .simultaneousGesture(TapGesture().onEnded { Self.askKeyboard() })   // a click into the box asks again
                            .onSubmit { _ = qr.send(app) }
                            .whooshAx(NotchIds.openQuickReply)
                        KeyCap(Self.sendCap)
                        Button { _ = qr.send(app) } label: {
                            Image(systemName: "arrow.up")
                                .font(.system(size: WhooshTypography.body.size, weight: .bold))
                                .foregroundStyle(WhooshPalette.onAccent)
                                .frame(width: NotchMetrics.button, height: NotchMetrics.button)
                                .background(WhooshPalette.accent, in: Circle())
                                .opacity(qr.draft.isEmpty ? 0.4 : 1)
                        }
                        .buttonStyle(.plain)
                        .keyboardShortcut(.return, modifiers: .command)
                        .notchTip(NotchCopy.send + " · " + Self.sendCap)
                        .accessibilityLabel(NotchCopy.send)
                        .whooshAx(NotchIds.send)
                    }
                }
                .padding(.leading, WhooshSpacing.s14)
                .padding(.trailing, WhooshSpacing.s8)
                .padding(.vertical, WhooshSpacing.s8)
                .background(WhooshPalette.field, in: RoundedRectangle(cornerRadius: NotchMetrics.replyCorner, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: NotchMetrics.replyCorner, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator))
                .animation(WhooshMotion.hover, value: qr.fromOpen)
                if qr.ai.changeFieldShown { QuickReplyChangeRow(model: qr, app: app) }
                if let words = qr.ai.failureWords { QuickReplyAIFailure(words: words) }
                if let words = NotchFailure.replyWords(model) {
                    Label(words, systemImage: "exclamationmark.circle")
                        .whooshText(WhooshTypography.caption)
                        .foregroundStyle(WhooshPalette.warning)
                }
            }
        }
        .onAppear { Self.askKeyboard(); focused = true }
    }

    /// From · the sending account's emblem and address · a chevron: the address this reply leaves from.
    private var fromLine: some View {
        let a = RowAccount.of(qr.context.accountId, in: app)
        return Button { _ = qr.toggleFrom() } label: {
            HStack(spacing: WhooshSpacing.s6) {
                Text(NotchCopy.from).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.tertiaryLabel)
                AccountMark(name: a.name, address: a.address, colour: a.colour, icon: a.iconId.map { AccountMarkIcon.library($0) } ?? AccountMarkIcon.providerDefault, size: .reply,
                            needsAttention: a.needsAttention, hasUnread: false, onSelection: false)
                Text(qr.context.from).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.label).lineLimit(1)
                Image(systemName: qr.fromOpen ? "chevron.up" : "chevron.down").font(.system(size: WhooshTypography.caption2.size, weight: .semibold)).foregroundStyle(WhooshPalette.secondaryLabel)
            }
            .padding(.horizontal, WhooshSpacing.s4).padding(.vertical, WhooshSpacing.s2)
            .hoverAck(radius: WhooshRadius.control)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .notchTip(Words.text("Send from"))
        .accessibilityLabel(NotchCopy.from + " " + qr.context.from)
        .whooshAx(NotchIds.openFrom)
    }

    /// Every account the core lists, the sender checked; a pick changes the From line on the same frame.
    private var fromPicks: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(qr.accounts, id: \.id) { acc in
                let a = RowAccount.of(acc.id, in: app)
                Button { _ = qr.pickAccount(acc.id) } label: {
                    HStack(spacing: WhooshSpacing.s8) {
                        AccountMark(name: a.name, address: a.address, colour: a.colour, icon: a.iconId.map { AccountMarkIcon.library($0) } ?? AccountMarkIcon.providerDefault, size: .reply,
                                    needsAttention: a.needsAttention, hasUnread: false, onSelection: false)
                        Text(acc.address).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.label).lineLimit(1)
                        Spacer(minLength: 0)
                        if acc.id == qr.context.accountId {
                            Image(systemName: "checkmark").font(.system(size: WhooshTypography.caption.size, weight: .semibold)).foregroundStyle(WhooshPalette.accent)
                        }
                    }
                    .padding(.horizontal, WhooshSpacing.s6)
                    .frame(height: WhooshMetrics.actionButton)
                    .hoverAck(radius: WhooshRadius.control)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityAddTraits(acc.id == qr.context.accountId ? .isSelected : [])
                .whooshAx(NotchIds.openFromPick(acc.id))
            }
        }
    }
}

extension NotchReply {
    /// The panel is non-activating: the field types only while it holds the keyboard (NotchKeyboard).
    static func askKeyboard() {
        #if os(macOS)
        NotchKeyboard.shared.take()
        #endif
    }
}

/// The undo-send bar (GotMail 0.8.0): the Components undo window while the message waits in the outbox, and
/// "Sent" when the window runs out.
struct NotchSendBar: View {
    let model: NotchModel
    var qr = QuickReplyModel.shared

    var body: some View {
        HStack(spacing: WhooshSpacing.s10) {
            if case .counting(let id, let countdown) = qr.phase {
                // One implementation per pair (owner's fork, Notch/README.md "What it shares with the window"):
                // the undo window the main window draws is the Components UndoBar, so the notch draws that one.
                UndoBar(message: NotchCopy.sending, countdown: countdown, onUndo: { _ = qr.undo(AppModel.shared) })
                    .whooshAx(NotchIds.undoAction)
                    .task(id: id) {
                    // The reader's own undo bar shares this model and may end the window first, which tears this bar
                    // down and cancels this task at the same instant. So the window's end is judged by the clock,
                    // not by whether the sleep ran to the end: an Undo earlier than the last tick never gets here.
                    let end = Date().addingTimeInterval(countdown.remaining(at: Date()))
                    try? await Task.sleep(for: .seconds(countdown.remaining(at: Date())))
                    if Date() >= end.addingTimeInterval(-NotchMetrics.expiryGrace) { model.sendExpired(AppModel.shared) }
                }
            } else {
                Image(systemName: "checkmark.circle.fill").foregroundStyle(WhooshPalette.accent)
                Text(NotchCopy.sent).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.label)
                Spacer()
            }
        }
        .frame(minHeight: WhooshMetrics.glassPillHeight)
        .whooshAx(NotchIds.undo)
    }
}
