import SwiftUI

/// `.bk.me.rp` your turn: the reply field under the last message, lifted glass like the slab it opens into (ThreadFootLayout.fill(.reply)), From your receiving
/// address. A click does what R does (the quick-reply lane owns the field itself).
struct ReplyBlock: View {
    let thread: ReaderThread
    let account: RowAccount
    let reply: () -> Void

    var body: some View {
        Button(action: reply) {
            ReaderBlock(mine: true, ax: ReplyBlock.ax) {
                HStack(spacing: WhooshSpacing.s6) {
                    Text(Words.text("From")).foregroundStyle(WhooshPalette.secondaryLabel)
                    AccountMark(name: account.name, address: account.address, colour: account.colour,
                                icon: .providerDefault, size: .reply, needsAttention: account.needsAttention)
                    Text(account.address.isEmpty ? account.name : account.address).foregroundStyle(WhooshPalette.label)
                    Spacer(minLength: 0)
                }
                .whooshText(WhooshTypography.caption).lineLimit(1)
                Text(ReaderWords.replyTo(thread.sender))
                    .whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.secondaryLabel)
                    .padding(.top, WhooshSpacing.s2)
                HStack {
                    Spacer()
                    KeyCap("R")
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier(ReplyBlock.ax)   // VoiceOver; the probe frame is the card's (ReaderBlock ax:)
        .accessibilityLabel(ReaderWords.replyTo(thread.sender))
    }
}

extension ReplyBlock {
    static let ax = "mac-main.reader.reply"

    /// The fill the closed field is drawn with, the word the socket row serves: read from the same `ThreadFootLayout.fill(.reply)` that `ReaderBlock` draws it by.
    static func material(_ fill: ThreadFootLayout.Fill = ThreadFootLayout.fill(.reply)) -> String { fill == .lifted ? "lifted" : "flat" }

    /// The row `mac-main.reader.reply`: one row while the closed field is drawn, none while the quick reply is open on this thread (ReaderPane hides the field then).
    static func rows(quickOpenHere: Bool) -> [ControlRow] { quickOpenHere ? [] : [["ax": ax, "material": material()]] }
}
