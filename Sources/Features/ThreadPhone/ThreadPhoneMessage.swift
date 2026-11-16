import SwiftUI

/// "Name" of the recipients without their addresses: `["Nina Okafor <nina@x>", "bob@y"]` is "Nina Okafor, bob@y".
private func names(_ list: [String]) -> String { list.map { $0.components(separatedBy: " <").first ?? $0 }.joined(separator: ", ") }

private var cardShape: RoundedRectangle { RoundedRectangle(cornerRadius: PhoneMetrics.cardRadius, style: .continuous) }

/// An earlier message: a soft card with the sender, the time and the first words. A tap unfolds it in place (its whole
/// text when the core holds it); a message the core does not hold says "Not downloaded yet".
struct ThreadFoldedCard: View {
    let index: Int
    let fact: ReaderFact?
    let expanded: Bool
    let toggle: () -> Void

    var body: some View {
        Button(action: toggle) {
            VStack(alignment: .leading, spacing: WhooshSpacing.s4) {
                HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s6) {
                    Text(fact?.from ?? Words.text("Message %@", index + 1)).phoneText(PhoneType.headline)
                        .foregroundStyle(WhooshPalette.label).lineLimit(1)
                    Spacer(minLength: WhooshSpacing.s6)
                    if let time = fact?.time { Text(time).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel) }
                    Image(systemName: "chevron.down").rotationEffect(.degrees(expanded ? 180 : 0))
                        .font(.system(size: PhoneType.footnote.size, weight: .semibold)).foregroundStyle(WhooshPalette.tertiaryLabel)
                }
                Text(words).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                    .lineLimit(expanded ? nil : ThreadMetrics.foldedLines).fixedSize(horizontal: false, vertical: true)
                    .multilineTextAlignment(.leading)
            }
            .padding(ThreadMetrics.cardPad)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(PhoneGround.trackFill, in: cardShape)
            .contentShape(cardShape)
        }
        .buttonStyle(PressStyle())
        .padding(.horizontal, PhoneMetrics.gutter)
        .accessibilityValue(expanded ? "expanded" : "folded")
        .accessibilityHint(expanded ? "" : Words.text("Show the whole message"))
        .accessibilityIdentifier("iphone-thread.messages.\(index)")
    }

    /// The first words, or the whole text when unfolded and the core holds the body; else the sentence.
    var words: String {
        guard let fact else { return ThreadPhoneWords.notDownloaded }
        if expanded, let whole = ReaderFact.wholeText(fact.id), !whole.isEmpty { return whole }
        return fact.preview.isEmpty ? ThreadPhoneWords.notDownloaded : fact.preview
    }
}

/// The latest message, open: sender and time, the code pill when the message carries a sign-in code, the body from the
/// local store (the same `MessageBodyView` the Mac reader draws), the attachments. While the core holds no body the card
/// shows the preview and "Not downloaded yet", and swaps to the body in place when the core announces it.
struct ThreadOpenCard: View {
    let thread: ReaderThread
    let unread: Bool
    let copied: Bool
    let copy: () -> Void
    @State private var bodies = ReaderBodies.shared  // local-ui: observed, a body that lands later redraws the card
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private let m = AppModel.shared

    var body: some View {
        let _ = bodies.revision
        let content = thread.shownBody(m)
        VStack(alignment: .leading, spacing: WhooshSpacing.s12) {
            header
            if let code = thread.open.code { ThreadCodePill(code: code, copied: copied, copy: copy) }
            Group {
                if let content {
                    MessageBodyView(content: content, sender: thread.open.address).id(content.id)
                } else {
                    missing
                }
            }
            .animation(reduceMotion ? nil : WhooshMotion.readerSwap, value: content != nil)
            .accessibilityIdentifier("iphone-thread.body")
            if !thread.attachmentNames.isEmpty { attachments }
        }
        .padding(ThreadMetrics.cardPad)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(PhoneGround.list, in: cardShape)
        .overlay { cardShape.strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator) }
        .padding(.horizontal, PhoneMetrics.gutter)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("iphone-thread.messages.\(thread.earlierCount)")
    }

    private var header: some View {
        HStack(alignment: .top, spacing: WhooshSpacing.s8) {
            if unread {
                Circle().fill(WhooshPalette.accent).frame(width: PhoneMetrics.unreadDot, height: PhoneMetrics.unreadDot)
                    .padding(.top, WhooshSpacing.s6).accessibilityHidden(true)
            }
            VStack(alignment: .leading, spacing: 0) {
                Text(thread.sender).phoneText(PhoneType.headline).foregroundStyle(WhooshPalette.label).lineLimit(1)
                if !thread.open.to.isEmpty {
                    Text(ThreadPhoneWords.to(names(thread.open.to))).phoneText(PhoneType.footnote)
                        .foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                }
            }
            Spacer(minLength: WhooshSpacing.s6)
            Text(thread.time).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
        }
        .accessibilityElement(children: .combine)
    }

    private var missing: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            if !thread.preview.isEmpty {
                Text(thread.preview).phoneText(PhoneType.body).foregroundStyle(WhooshPalette.label)
                    .fixedSize(horizontal: false, vertical: true).textSelection(.enabled)
            }
            HStack(spacing: WhooshSpacing.s6) {
                Image(systemName: "arrow.down.circle")
                Text(ThreadPhoneWords.notDownloaded)
            }
            .phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
            if thread.open.trackers > 0 {
                Text(ReaderWords.trackers(thread.open.trackers)).phoneText(PhoneType.footnote).foregroundStyle(WhooshPalette.secondaryLabel)
            }
        }
        .accessibilityElement(children: .combine)
    }

    private var attachments: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s8) {
            ForEach(Array(thread.attachmentNames.enumerated()), id: \.offset) { i, name in
                HStack(spacing: WhooshSpacing.s8) {
                    Image(systemName: "paperclip")
                    Text(name).lineLimit(1)
                }
                .phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.label)
                .padding(.horizontal, WhooshSpacing.s12).frame(height: ThreadMetrics.chipHeight)
                .background(PhoneGround.trackFill, in: Capsule())
                .accessibilityElement(children: .combine)
                .accessibilityIdentifier("iphone-thread.attachments.\(i)")
            }
        }
    }
}
