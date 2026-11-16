import SwiftUI

/// One 44 px two-line row (docs/DESIGN.md density "regular"): the unread dot, the receiving account's mark,
/// sender + count + marks + time, subject. Hover swaps the time for the actions and "Why?". The marks follow board message-card-r1 design A
/// (`marks()`): VIP, pin, star, clip, mute, after the count on the first line; a snoozed mail says when it comes back in the time's place.
struct InboxRowView: View, nonisolated Equatable {
    let row: InboxRow
    let account: RowAccount
    let focused: Bool
    let selected: Bool
    /// the row carries a star (the core's `message/<id>/starred`, read once per change of the rows)
    let starred: Bool
    /// the newest message of this thread is the owner's own: the small reply arrow sits before the subject (ThreadFacts)
    let repliedLast: Bool
    let compact: Bool
    /// "Show account emblems in the message list"; off draws EmblemSize.hidden and gives the text its width back
    let showEmblem: Bool
    /// the list's one focus fill (docs/MOTION.md rule 11), drawn under the focused row only
    let highlight: FocusHighlight
    let why: String?
    /// the grey tag naming what placed this row here (the inspector's « Pourquoi ici ? » switch); nil = no tag
    let whyTag: WhyTag?
    /// the highlight's own animation (nil = jump); scoped to the highlight, so nothing else in the row animates on a move
    let highlightAnimation: Animation?
    /// false on the focused row while keys are still arriving: the strip waits
    let stripAllowed: Bool
    /// the thread is muted (the core's `message/<id>/muted`, RowMarks): the mute mark, sender and subject in the tertiary ink (board state 13)
    let muted: Bool
    /// the sender is a VIP (the VIPs view's senders, RowMarks): the VIP chip before the other marks (board state 14)
    let vip: Bool
    /// the snoozed mail's return in words ("back 18:00", RowMarks), drawn in the time's place with the clock (board state 10); nil = not snoozed
    let snoozeWords: String?
    /// the pointer rests on this row (InboxListState.rested: the pointer, or the rig's `fixture pointer-rest`): the hover fill and the strip (board state 7)
    let pointerRested: Bool
    var onHover: (Bool) -> Void = { _ in }
    /// the pointer stayed HOVER_REVEAL_MS on the row (true) or left it (false): the list keeps it in InboxListState.rested
    var onRest: (Bool) -> Void = { _ in }
    var onAction: (String) -> Void = { _ in }

    @State private var hovering = false
    /// the pointer has rested on the row for HOVER_REVEAL_MS: the strip may show
    @State private var rested = false
    @State private var restTask: Task<Void, Never>?
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    /// What the row draws, as plain values: SwiftUI skips the row's body when this is unchanged (closures excluded).
    nonisolated let key: RowKey
    nonisolated static func == (a: InboxRowView, b: InboxRowView) -> Bool { a.key == b.key }

    private var onSelection: Bool { focused }
    private var primary: Color { onSelection ? WhooshPalette.onAccent.color(scheme) : WhooshPalette.label.color(scheme) }
    private var secondary: Color {
        onSelection ? WhooshPalette.onAccent.color(scheme) : WhooshPalette.secondaryLabel.color(scheme)
    }
    /// the board's `.muted .who, .muted .subj { color: var(--ter) }`: a muted row's sender and subject (the focus fill keeps its white)
    private var quiet: Bool { muted && !onSelection }
    private var senderInk: Color { quiet ? WhooshPalette.tertiaryLabel.color(scheme) : primary }
    private var subjectInk: Color { quiet ? WhooshPalette.tertiaryLabel.color(scheme) : (row.unread ? primary : secondary) }
    /// the pointer is over the row: its own hover, or a rest the list holds
    private var pointerOver: Bool { hovering || pointerRested }

    var body: some View {
        let _ = InteractionProbe.noteBody(.rowBodies, inDispatch: .rowBodiesInDispatch)   // perf-archive r7: a count for the measure
        VStack(alignment: .leading, spacing: 0) {
            lineOne
            if !compact { lineTwo }
            if let why { Text(why).font(WhooshTypography.caption.font(emphasized: false)).foregroundStyle(secondary).lineLimit(1) }
        }
        .padding(.leading, showEmblem ? WhooshMetrics.listRowLeading + WhooshSpacing.s4 : Self.leadingWithoutEmblem)
        .padding(.trailing, WhooshSpacing.s12)
        .frame(maxWidth: .infinity, minHeight: compact ? Self.compactHeight : WhooshMetrics.listRow, alignment: .leading)
        .overlay(alignment: .topLeading) { leadingMarks }
        .background {
            ZStack {
                RoundedRectangle(cornerRadius: WhooshRadius.row, style: .continuous).fill(background)
                if focused { highlight }
            }
            .animation(highlightAnimation, value: focused)
        }
        .overlay(alignment: .bottom) { if separated { hairline } }
        .padding(.horizontal, WhooshMetrics.listRowInset)
        .contentShape(Rectangle())
        .onHover { h in
            withAnimation(WhooshMotion.animation(WhooshMotion.hover, reduceMotion: reduceMotion)) { hovering = h }
            onHover(h)
            restTask?.cancel()
            if h {
                restTask = Task { @MainActor in
                    try? await Task.sleep(for: .milliseconds(WhooshConstants.HOVER_REVEAL_MS))
                    if !Task.isCancelled { withAnimation(WhooshMotion.animation(WhooshMotion.hover, reduceMotion: reduceMotion)) { rested = true }; onRest(true) }
                }
            } else { rested = false; onRest(false) }
        }
        .accessibilityElement(children: .combine)
        // the row's place id (`mac-main.list.row.<n>`) is set by InboxListView OUTSIDE this equatable view (perf-archive r7): the place is
        // not part of the picture, so an archive or an undo that shifts every row below by one no longer redraws each of them
        .accessibilityLabel(spoken)
        .accessibilityAddTraits(focused ? .isSelected : [])
    }

    /// What VoiceOver says for the row: its state, sender, subject and account, the why tag, then the board's marks (VIP, muted, the snooze return).
    private var spoken: String {
        let head: String = (row.unread ? Words.text("Unread, ") : "") + (row.pinned ? Words.text("pinned, ") : "")
        let body: String = Words.text("%@, %@, to %@", row.senderName, row.subject, account.name)
        let tag: String = whyTag.map { ", " + $0.line } ?? ""
        var marks: [String] = []
        if vip { marks.append(Words.text("VIP")) }
        if muted { marks.append(Words.text("Muted")) }
        if let snoozeWords { marks.append(snoozeWords) }
        return head + body + tag + marks.map { ", " + $0 }.joined()
    }

    /// the drawing's `.sep`: a 1 px rule under the row, inset from the emblem's column, gone under the focus fill and the hover fill
    /// (board message-card-r1 `.row.focus::after, .row.hov::after { display: none }`)
    private var separated: Bool { !focused && !pointerOver }
    private var hairline: some View {
        Rectangle().fill(WhooshPalette.separator).frame(height: WhooshGlass.rimWidth * 2)
            .padding(.leading, WhooshMetrics.listRowInset * 5).padding(.trailing, WhooshMetrics.listRowInset)
            .accessibilityHidden(true)
    }

    /// the unread dot's column only: the text starts where the emblem used to
    static let leadingWithoutEmblem = WhooshSpacing.s4 + WhooshMetrics.listRowInset + WhooshMetrics.unreadDot + WhooshSpacing.s8

    private var background: some ShapeStyle {
        if selected { return AnyShapeStyle(WhooshPalette.mine) }
        if pointerOver { return AnyShapeStyle(WhooshPalette.hover) }
        return AnyShapeStyle(Color.clear)
    }

    private var leadingMarks: some View {
        HStack(spacing: WhooshSpacing.s4) {
            if selected {
                Image(systemName: "checkmark.circle.fill").font(WhooshTypography.caption.font(emphasized: true))
                    .foregroundStyle(onSelection ? WhooshPalette.onAccent.color(scheme) : WhooshPalette.accent.color(scheme))
                    .frame(width: WhooshMetrics.unreadDot, height: WhooshMetrics.unreadDot)
                    .whooshAx("mac-main.list.row.\(row.messageId).check")
            } else {
                Circle().fill(row.unread ? (onSelection ? WhooshPalette.onAccent.color(scheme) : WhooshPalette.accent.color(scheme)) : .clear)
                    .frame(width: WhooshMetrics.unreadDot, height: WhooshMetrics.unreadDot)
            }
            if showEmblem {
                EmblemMark(glyph: EmblemLibrary.resolve(account.iconId), colourHex: account.colour, size: .row,
                           state: account.needsAttention ? .needsSignIn : .live, name: account.name, address: account.address)
                    .help(Words.text("To %@ · %@", account.name, account.address))
                    .whooshAx("mac-main.list.row.\(row.messageId).emblem")
            } else {
                EmblemMark(glyph: EmblemLibrary.resolve(account.iconId), colourHex: account.colour, size: .hidden,
                           name: account.name, address: account.address)
            }
        }
        .padding(.leading, WhooshSpacing.s4 + WhooshMetrics.listRowInset)
        .padding(.top, compact ? WhooshSpacing.s6 : WhooshSpacing.s4 + WhooshSpacing.s2)
    }

    /// Where the row's time or action strip stands. In the two-line row the strip comes in three widths and the first that leaves the sender's name whole is drawn
    /// (full, archive-snooze-label, then the time): a name is never cut for a control (owner rule, round 20: "plain one-line text, nothing cut where a shorter layout holds it").
    @ViewBuilder private func lineOneRow(strip: Int) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s6) {
            Text(row.senderName).font(WhooshTypography.body.font(emphasized: row.unread)).foregroundStyle(senderInk).lineLimit(1)
                .fixedSize(horizontal: !compact, vertical: false).layoutPriority(compact ? 1 : 0)
                .whooshAx("mac-main.list.row.\(row.messageId).text")
            countPill
            if !compact { marks }
            if compact { draftMark }
            if compact { repliedMark }
            if compact { Text(row.subject).font(WhooshTypography.subhead.font(emphasized: false)).foregroundStyle(quiet ? subjectInk : secondary).lineLimit(1) }
            if compact { whyTagView }
            if compact { reminderChip }
            Spacer(minLength: WhooshSpacing.s8)
            if strip > 0 && (rested || pointerRested || focused) && stripAllowed { RowActions(tint: secondary, messageId: row.messageId, onAction: onAction, short: strip == 2).transition(.opacity) }
            else if let snoozeWords { SnoozeReturn(words: snoozeWords, ink: onSelection ? WhooshPalette.onAccent.color(scheme) : WhooshPalette.warning.color(scheme))
                .whooshAx("mac-main.list.row.\(row.messageId).snooze") }
            else { Text(RowFacts.time(row.receivedAtMs)).font(WhooshTypography.caption.font(emphasized: false)).foregroundStyle(secondary).fixedSize() }
        }
    }

    @ViewBuilder private var lineOneBody: some View {
        if compact { lineOneRow(strip: 1) }
        else { ViewThatFits(in: .horizontal) { lineOneRow(strip: 1); lineOneRow(strip: 2); lineOneRow(strip: 0) } }
    }

    private var lineOne: some View {
        lineOneBody
        .frame(height: WhooshMetrics.listRowLine)
        .padding(.top, WhooshSpacing.s4)
    }

    /// The marks after the sender and the count, in the board's order (message-card-r1 `marks()`): the VIP chip, pin, star, clip, mute.
    /// Inks are the board's: the pin in the accent, the star filled in the warning colour, clip and mute in the secondary ink; white on the focus fill.
    @ViewBuilder private var marks: some View {
        if vip { VipChip(ink: onSelection ? WhooshPalette.onAccent.color(scheme) : WhooshPalette.warning.color(scheme)).whooshAx("mac-main.list.row.\(row.messageId).vip") }
        if row.pinned { RowMark(symbol: "pin", ink: onSelection ? WhooshPalette.onAccent.color(scheme) : WhooshPalette.accent.color(scheme)) }
        if starred { RowMark(symbol: "star.fill", ink: onSelection ? WhooshPalette.onAccent.color(scheme) : WhooshPalette.warning.color(scheme)) }
        if row.attachmentCount > 0 { RowMark(symbol: "paperclip", ink: secondary) }
        if muted { RowMark(symbol: "bell.slash", ink: secondary).whooshAx("mac-main.list.row.\(row.messageId).mute") }
    }

    /// The thread has a saved draft: the word "Draft" before the subject, caption, secondary; no colour edge (owner 15:46).
    @ViewBuilder private var draftMark: some View {
        if RowFacts.hasDraft(threadId: row.threadId, in: AppModel.shared) {
            Text(Words.text("Draft")).font(WhooshTypography.caption.font(emphasized: true)).foregroundStyle(secondary).fixedSize()
                .whooshAx("mac-main.list.row.\(row.messageId).draft")
        }
    }

    /// gap-remind: a reply reminder of this thread fired (its time came, no reply): the row's chip says so, drawn like the label chips
    /// beside it (caption, emphasised, secondary). The words come from the catalogue ("Reminder").
    nonisolated static func showsReminderChip(_ r: InboxRow) -> Bool { r.reminderFired }
    @ViewBuilder private var reminderChip: some View {
        if Self.showsReminderChip(row) {
            Text(Words.text("Reminder")).font(WhooshTypography.caption.font(emphasized: true)).foregroundStyle(secondary).fixedSize()
                .whooshAx("mac-main.list.row.\(row.messageId).reminder")
        }
    }

    /// A thread (more than one message) carries its count in a pill after the sender; a single message carries none.
    @ViewBuilder private var countPill: some View {
        if let n = ThreadFacts.pillCount(row.threadCount) {
            CountPill(count: n, unread: row.unread, focused: onSelection).whooshAx("mac-main.list.row.\(row.messageId).count")
        }
    }

    @ViewBuilder private var repliedMark: some View {
        if repliedLast && row.threadCount > 1 { RepliedMark(tint: secondary).whooshAx("mac-main.list.row.\(row.messageId).replied") }
    }

    private var lineTwo: some View {
        HStack(spacing: WhooshSpacing.s6) {
            draftMark
            repliedMark
            Text(row.subject).font(WhooshTypography.subhead.font(emphasized: false))
                .foregroundStyle(subjectInk).lineLimit(1)
            Spacer(minLength: WhooshSpacing.s4)
            reminderChip
            ForEach(row.labelNames, id: \.self) { Text($0).font(WhooshTypography.caption.font(emphasized: true)).foregroundStyle(secondary) }
            whyTagView
        }
        .frame(height: WhooshMetrics.listRowLine)
    }

    /// The why tag fades in over HOVER_TINT_MS (opacity only; its 16 pt capsule sits inside the 18 pt line, so no row moves).
    @ViewBuilder private var whyTagView: some View {
        if let whyTag {
            WhyTagView(tag: whyTag, messageId: row.messageId, onSelection: onSelection)
                .transition(.opacity)
                .animation(WhooshMotion.animation(WhooshMotion.hover, reduceMotion: reduceMotion), value: whyTag)
        }
    }
}

/// The values a row's picture depends on. A focus move changes this for two rows only, so the other rows never re-evaluate.
struct RowKey: Equatable, Sendable {
    let messageId: String, threadId: String, senderName: String, subject: String
    let unread: Bool, pinned: Bool, reminderFired: Bool, repliedLast: Bool, threadCount: Int, receivedAtMs: Int64, attachmentCount: Int, labelNames: [String]
    let account: RowAccount, focused: Bool, selected: Bool, starred: Bool, compact: Bool, showEmblem: Bool   // no index: see InboxRowView body
    let highlightKey: Int, animation: Int, why: String?, whyTag: WhyTag?, stripAllowed: Bool
    let muted: Bool, vip: Bool, snoozeWords: String?, pointerRested: Bool
    /// the app language when the row was made: the date words follow it, so a language switch redraws the rows
    let language: String = Words.current

    init(row r: InboxRow, index: Int, account: RowAccount, focused: Bool, selected: Bool, starred: Bool, repliedLast: Bool, compact: Bool, showEmblem: Bool,
         highlightKey: Int, animation: Animation?, why: String?, whyTag: WhyTag?, stripAllowed: Bool,
         muted: Bool, vip: Bool, snoozeWords: String?, pointerRested: Bool) {
        (messageId, threadId, senderName, subject) = (r.messageId, r.threadId, r.senderName, r.subject)
        (unread, pinned, threadCount, receivedAtMs, attachmentCount, labelNames) =
            (r.unread, r.pinned, Int(r.threadCount), Int64(r.receivedAtMs), Int(r.attachmentCount), r.labelNames)
        self.repliedLast = repliedLast
        self.reminderFired = r.reminderFired
        (self.account, self.focused, self.selected, self.starred, self.compact, self.showEmblem) = (account, focused, selected, starred, compact, showEmblem)   // `index` is accepted and not kept
        (self.highlightKey, self.why, self.whyTag, self.stripAllowed) = (highlightKey, why, whyTag, stripAllowed)
        self.animation = animation == nil ? 0 : (animation == WhooshMotion.hover ? 1 : 2)
        (self.muted, self.vip, self.snoozeWords, self.pointerRested) = (muted, vip, snoozeWords, pointerRested)
    }
}

extension InboxRowView {
    /// a compact row's height: one line plus its padding (the collapse animates from it)
    static let compactHeight = WhooshMetrics.listRowLine + WhooshSpacing.s12
}

/// The board's sizes for the row marks (message-card-r1 style block). Their values are the rows ROW_MARK_PT and ROW_VIP_CHIP_* of docs/CONSTANTS.md.
enum RowMarkSize {
    /// `.ic { width: 12px; height: 12px }` the pin, star, clip, mute and clock glyphs
    static let icon = CGFloat(WhooshConstants.ROW_MARK_PT)
    /// `.vip { line-height: 12px; padding: 0 3px; border-radius: 3px; letter-spacing: .04em }` (font-size 9px is caption2)
    static let vipHeight = CGFloat(WhooshConstants.ROW_VIP_CHIP_H_PT)
    static let vipSide = CGFloat(WhooshConstants.ROW_VIP_CHIP_SIDE_PT)
    static let vipRadius = CGFloat(WhooshConstants.ROW_VIP_CHIP_R_PT)
    static let vipTracking = CGFloat(WhooshConstants.ROW_VIP_CHIP_TRACKING_PT)
}

/// One of the row's glyph marks (pin, star, clip, mute) at the board's 12 pt.
struct RowMark: View {
    let symbol: String
    let ink: Color
    var body: some View {
        Image(systemName: symbol).resizable().scaledToFit().frame(width: RowMarkSize.icon, height: RowMarkSize.icon)
            .foregroundStyle(ink).accessibilityHidden(true)
    }
}

/// The board's VIP chip: "VIP" in caption2 bold, the warning ink, a 1 pt outline of the same ink, 3 pt corners (`.vip`).
struct VipChip: View {
    let ink: Color
    var body: some View {
        Text(Words.text("VIP")).font(WhooshTypography.caption2.font(emphasized: true)).tracking(RowMarkSize.vipTracking)
            .foregroundStyle(ink).lineLimit(1)
            .padding(.horizontal, RowMarkSize.vipSide).frame(height: RowMarkSize.vipHeight)
            .overlay { RoundedRectangle(cornerRadius: RowMarkSize.vipRadius, style: .continuous).strokeBorder(ink, lineWidth: WhooshMetrics.separator) }
            .fixedSize()
    }
}

/// A snoozed mail's return in the time's place: the clock and the words, both in the warning ink (board `.time.snz`, « revient 18:00 »).
struct SnoozeReturn: View {
    let words: String
    let ink: Color
    var body: some View {
        (Text(Image(systemName: "clock")) + Text(" " + words))
            .font(WhooshTypography.caption.font(emphasized: false)).foregroundStyle(ink).lineLimit(1).fixedSize()
    }
}
