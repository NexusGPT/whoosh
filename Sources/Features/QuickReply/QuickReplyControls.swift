import SwiftUI

/// "Envoyer ⌘↵" (board cards-s3 `.b1`): the one filled blue control of the slab, a 28 pt pill with the word and its
/// key cap; quiet (`.off`) while the field is empty. ⌘Enter is the shown key; ⌃Enter still sends, unshown (a second, invisible button carries
/// the second shortcut); the press acknowledges on the input frame.
struct QuickReplySendPill: View {
    let enabled: Bool
    var action: () -> Void
    var body: some View {
        Button(action: action) {
            HStack(spacing: WhooshSpacing.s6) {
                Text(QuickReplyCopy.sendWord).whooshText(WhooshTypography.caption, emphasized: true)
                    .foregroundStyle(enabled ? WhooshPalette.onAccent : WhooshPalette.secondaryLabel)
                KeyCap(QuickReplyCopy.sendCap, onSelection: enabled)
            }
            .padding(.horizontal, WhooshSpacing.s12).frame(height: WhooshMetrics.segmentHeight)
            .background(enabled ? WhooshPalette.accent : WhooshPalette.fill, in: Capsule())
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .keyboardShortcut(.return, modifiers: .command)
        .background {
            Button(action: action) { EmptyView() }
                .keyboardShortcut(.return, modifiers: .control)
                .opacity(0).frame(width: 0, height: 0).allowsHitTesting(false).accessibilityHidden(true)
        }
        .animation(WhooshMotion.hover, value: enabled)
        .help(QuickReplyCopy.sendHelp)
        .accessibilityLabel(QuickReplyCopy.send)
        .whooshAx(QuickReplyIds.send)
    }
}

/// The notch card's send button (round 4): the round arrow with its key cap `⌘↵` beside it.
struct QuickReplySendButton: View {
    let enabled: Bool
    var compact = false
    var action: () -> Void
    var body: some View {
        Button(action: action) {
            HStack(spacing: WhooshSpacing.s6) {
                KeyCap(QuickReplyCopy.sendCap).opacity(enabled ? 1 : 0.7)
                Image(systemName: "arrow.up").font(.system(size: WhooshMetrics.actionIcon - WhooshSpacing.s4, weight: .bold))
                    .foregroundStyle(enabled ? WhooshPalette.onAccent : WhooshPalette.secondaryLabel)
                    .frame(width: WhooshMetrics.actionButton, height: WhooshMetrics.actionButton)
                    .background(enabled ? WhooshPalette.accent : WhooshPalette.fill, in: Circle())
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .keyboardShortcut(.return, modifiers: .command)
        .background {
            Button(action: action) { EmptyView() }
                .keyboardShortcut(.return, modifiers: .control)
                .opacity(0).frame(width: 0, height: 0).allowsHitTesting(false).accessibilityHidden(true)
        }
        .animation(WhooshMotion.hover, value: enabled)
        .help(QuickReplyCopy.sendHelp)
        .accessibilityLabel(QuickReplyCopy.send)
        .whooshAx(QuickReplyIds.on(compact, QuickReplyIds.send))
    }
}

/// The emoji button and its row of quick reactions, a popover that scales from its anchor.
struct QuickReplyEmojiButton: View {
    @Bindable var model: QuickReplyModel
    @Bindable var app: AppModel
    var compact = false
    var body: some View {
        Button { model.emojiOpen.toggle() } label: {
            Image(systemName: "face.smiling").font(.system(size: WhooshMetrics.actionIcon - WhooshSpacing.s2))
                .foregroundStyle(model.emojiOpen ? WhooshPalette.accent : WhooshPalette.secondaryLabel)
                .frame(width: WhooshMetrics.actionButton, height: WhooshMetrics.actionButton)
                .contentShape(Circle())
                .modifier(HoverAck(radius: WhooshMetrics.actionButton / 2))
        }
        .buttonStyle(.plain)
        .disabled(model.outboxId != nil)
        .accessibilityLabel(QuickReplyCopy.emoji)
        .whooshAx(QuickReplyIds.on(compact, QuickReplyIds.emoji))
        .popover(isPresented: $model.emojiOpen, arrowEdge: .top) {
            HStack(spacing: WhooshSpacing.s2) {
                ForEach(Array(QuickReplyEmoji.set.enumerated()), id: \.offset) { i, e in
                    Button { _ = model.insert(e, app) } label: {
                        Text(e).font(.system(size: WhooshTypography.title3.size))
                            .frame(width: WhooshMetrics.actionButton + WhooshSpacing.s4, height: WhooshMetrics.actionButton + WhooshSpacing.s4)
                            .modifier(HoverAck(radius: WhooshRadius.item))
                    }
                    .buttonStyle(.plain)
                    .whooshAx(QuickReplyIds.on(compact, QuickReplyIds.emojiItem(i)))
                }
            }
            .padding(WhooshSpacing.s6)
            .modifier(PopoverEntrance(anchor: .top))
            .whooshAx(QuickReplyIds.on(compact, QuickReplyIds.emojiPicker))
        }
    }
}

/// A send the core refused before the outbox (no entry to show as a card): the reason in words, the text kept above,
/// and Retry. A server's refusal of a queued send is the thread's failed card instead (ThreadFailedCard).
struct QuickReplyFailure: View {
    let words: String
    /// The engine's own words behind the sentence (a tooltip), nil when the sentence is the engine's own.
    var details: String? = nil
    var compact = false
    var retry: () -> Void
    var body: some View {
        HStack(spacing: WhooshSpacing.s6) {
            Image(systemName: "exclamationmark.circle.fill").foregroundStyle(WhooshPalette.warning).accessibilityHidden(true)
            Text(words).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.label).lineLimit(2)
                .help(details.map { Words.text("Details: %@", $0) } ?? "")
            Spacer(minLength: WhooshSpacing.s8)
            Button(action: retry) {
                Text(QuickReplyCopy.retry).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.accent)
            }
            .buttonStyle(.plain)
            .whooshAx(QuickReplyIds.on(compact, QuickReplyIds.retry))
        }
        .transition(.opacity)
    }
}

/// The From control (board `.from`): "De", the account's emblem, the address, a chevron, and the ⇧⌘F cap; it opens the
/// picker, a lifted panel: "Envoyer depuis", a filter, one row per account (emblem, name, address, the check on the
/// sender), "N autres comptes" past the fit, and the key line "↑ ↓ choisir · ↩ valider · esc fermer". A pick switches
/// the sender on the same frame; the panel scales from its anchor. The notch keeps the round-4 compact line.
struct QuickReplyFromButton: View {
    @Bindable var model: QuickReplyModel
    var compact = false
    private typealias L = WhooshGlass.Lift
    private var current: AccountSummary? { model.accounts.first { $0.id == model.context.accountId } }
    /// The rows the panel fits before "N autres comptes" takes over (board: 8 at 1280, 7 at 960).
    private static let fit = 8

    var body: some View {
        Button { _ = model.toggleFrom() } label: {
            // hid-compose round 18 (main111 sending-dark-960x632: "assem…ample.de"): the address is never cut while something else can give
            // way. Whole with the emblem and the ⇧⌘F cap; then without the cap (the key stays in the tooltip, `fromHelp`: owner, a key shows as a
            // cap where there is room, a tooltip elsewhere); then without the emblem; the middle cut only as the last resort. No id inside the
            // candidates (main93: an id on each candidate unregistered itself); the button's own id is outside them.
            ViewThatFits(in: .horizontal) {
                line(mark: true, cap: !compact).fixedSize()
                line(mark: true, cap: false).fixedSize()
                line(mark: false, cap: false).fixedSize()
                line(mark: false, cap: false)
            }
            .padding(.horizontal, WhooshSpacing.s4)
            .contentShape(Rectangle())
            .modifier(HoverAck(radius: WhooshRadius.item))
        }
        .buttonStyle(.plain)
        .disabled(model.outboxId != nil)
        .help(QuickReplyCopy.fromHelp)
        .accessibilityLabel(QuickReplyCopy.fromMenuTitle)
        .whooshAx(QuickReplyIds.on(compact, QuickReplyIds.from))
        .popover(isPresented: $model.fromOpen, arrowEdge: compact ? .bottom : .top) { picker }
    }

    private func line(mark: Bool, cap: Bool) -> some View {
        HStack(spacing: WhooshSpacing.s6) {
            Text(QuickReplyCopy.from).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
            if mark, !compact, let a = current {
                AccountMark(name: a.name, address: a.address, colour: a.colour, icon: .providerDefault, size: .compact)
            }
            Text(model.context.from).whooshText(WhooshTypography.caption, emphasized: !compact).foregroundStyle(WhooshPalette.label)
                .lineLimit(1).truncationMode(.middle)
            Image(systemName: "chevron.down").font(.system(size: WhooshTypography.caption.size - WhooshSpacing.s2, weight: .semibold))
                .foregroundStyle(model.fromOpen ? WhooshPalette.accent : WhooshPalette.secondaryLabel).accessibilityHidden(true)
            if cap { KeyCap(QuickReplyCopy.fromCap) }
        }
    }

    private var picker: some View {
        let rows = model.pickerAccounts
        let shown = Array(rows.prefix(Self.fit))
        return VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
            Text(QuickReplyCopy.fromMenuTitle).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
                .padding(.horizontal, WhooshSpacing.s8).padding(.top, WhooshSpacing.s4)
            HStack(spacing: WhooshSpacing.s6) {
                Image(systemName: "magnifyingglass").font(.system(size: WhooshTypography.caption.size)).foregroundStyle(WhooshPalette.secondaryLabel).accessibilityHidden(true)
                TextField(QuickReplyCopy.filterAccounts, text: $model.fromFilter).textFieldStyle(.plain)
                    .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.label)
                    .onChange(of: model.fromFilter) { _, _ in model.fromHighlight = 0 }
            }
            .padding(.horizontal, WhooshSpacing.s8).frame(height: WhooshMetrics.segmentHeight)
            .background(WhooshPalette.field, in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
            .whooshAx(QuickReplyIds.on(compact, QuickReplyIds.fromFilter))
            ForEach(Array(shown.enumerated()), id: \.element.id) { i, a in
                Button { _ = model.pickAccount(a.id) } label: {
                    HStack(spacing: WhooshSpacing.s8) {
                        AccountMark(name: a.name, address: a.address, colour: a.colour, icon: .providerDefault, size: .compact)
                        Text(a.name).whooshText(WhooshTypography.subhead, emphasized: i == model.fromHighlight).lineLimit(1)
                            .foregroundStyle(i == model.fromHighlight ? WhooshPalette.onAccent : WhooshPalette.label)
                        Spacer(minLength: WhooshSpacing.s8)
                        Text(a.address).whooshText(WhooshTypography.caption).lineLimit(1).truncationMode(.middle)
                            .foregroundStyle(i == model.fromHighlight ? WhooshPalette.onAccent : WhooshPalette.secondaryLabel)
                        Image(systemName: "checkmark").font(.system(size: WhooshTypography.caption.size, weight: .semibold))
                            .foregroundStyle(i == model.fromHighlight ? WhooshPalette.onAccent : WhooshPalette.accent)
                            .opacity(a.id == model.context.accountId ? 1 : 0).frame(width: WhooshSpacing.s12).accessibilityHidden(true)
                    }
                    .padding(.horizontal, WhooshSpacing.s8).frame(height: WhooshMetrics.segmentHeight)
                    .background(i == model.fromHighlight ? AnyShapeStyle(WhooshPalette.selection) : AnyShapeStyle(.clear),
                                in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
                    .contentShape(Rectangle())
                    .modifier(HoverAck(radius: WhooshRadius.control))
                }
                .buttonStyle(.plain)
                .whooshAx(QuickReplyIds.on(compact, QuickReplyIds.fromItem(i)))
            }
            if rows.count > shown.count {
                Text(QuickReplyCopy.otherAccounts(rows.count - shown.count)).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                    .padding(.horizontal, WhooshSpacing.s8).frame(height: WhooshSpacing.s20)
                    .whooshAx(QuickReplyIds.on(compact, QuickReplyIds.fromMore))
            }
            HStack(spacing: WhooshSpacing.s4) {
                KeyCap("Up"); KeyCap("Down"); foot(QuickReplyCopy.pickerChoose)
                KeyCap("Return"); foot(QuickReplyCopy.pickerConfirm)
                KeyCap("Esc"); foot(QuickReplyCopy.pickerClose)
            }
            .padding(.horizontal, WhooshSpacing.s8).padding(.top, WhooshSpacing.s4).frame(height: WhooshSpacing.s24)
        }
        .padding(WhooshSpacing.s6)
        .frame(width: compact ? WhooshMetrics.readerMeasure / 2 : WhooshMetrics.readerMeasure * 0.7)
        .whooshLift(in: RoundedRectangle(cornerRadius: WhooshRadius.hint, style: .continuous))
        .padding(WhooshSpacing.s4)
        .modifier(PopoverEntrance(anchor: compact ? .bottom : .top))
        .whooshAx(QuickReplyIds.on(compact, QuickReplyIds.fromMenu))
    }

    private func foot(_ word: String) -> some View {
        Text(word).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).padding(.trailing, WhooshSpacing.s6)
    }
}
