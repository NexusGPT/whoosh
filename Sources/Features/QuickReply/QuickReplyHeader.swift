import SwiftUI

/// The slab's header line (board cards-s3 `.qh`; owner 22:03 "the reply box header expands and collapses like each
/// message's header"). Editing: a chevron and "Répondre à <x>" (empty field) or "Brouillon à <x>" (words), the ⇧H cap
/// beside it, the pop-out and its ⇧⌘P cap at the right. While a send counts down the line IS the countdown: the ring,
/// "Envoi dans N s", and "Annuler Z" at the right; it returns to the title on the landing frame (LAND_STATE_MS fade).
struct QuickReplyHeaderLine: View {
    @Bindable var model: QuickReplyModel
    @Bindable var app: AppModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var spine = Spine.shared   // local-ui: the outbox stream says when the window closed (the core's clock, not the wall's)

    var body: some View {
        let entryState = spine.outbox.entries.first { $0.id == model.outboxId }?.state
        ZStack {
            if case .counting(_, let countdown) = model.phase { counting(countdown) } else { title }
        }
        .frame(height: WhooshSpacing.s24)   // the board's `.h` min-height: the AI pill (24 pt) sits in the line, nothing shifts when it tints
        .animation(.easeOut(duration: Double(WhooshConstants.LAND_STATE_MS) / 1000), value: model.outboxId == nil)
        // The landing frame: the core sent (or cancelled) the entry; the header returns to the title on that frame even
        // when the wall clock and the core's clock disagree (a moved fixture clock), not on the next key.
        .onChange(of: entryState) { _, st in
            guard let st else { return }
            if case .undoable = st { return }
            model.settle(app)
        }
        .whooshAx(QuickReplyIds.header)
    }

    private var title: some View {
        HStack(spacing: WhooshSpacing.s6) {
            Button { _ = model.toggleHeader() } label: {
                HStack(spacing: WhooshSpacing.s6) {
                    Image(systemName: "chevron.down").font(.system(size: WhooshTypography.caption.size - WhooshSpacing.s2, weight: .semibold))
                        .foregroundStyle(WhooshPalette.secondaryLabel)
                        .rotationEffect(.degrees(model.header.open ? 180 : 0))
                        .animation(WhooshMotion.hover, value: model.header.open)
                        .accessibilityHidden(true)
                    Text(model.headerTitle).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.label)
                        .lineLimit(1).truncationMode(.tail)
                        .whooshAx("mac-main.quickreply.header.title")
                        .contentTransition(reduceMotion ? .identity : .opacity)
                    KeyCap(QuickReplyCopy.headerCapTyping   /* one key wherever the cursor is (owner, 13 Nov: one meaning, one key); ⇧H still folds, unshown */)   // the key that works where the keys are
                }
                .padding(.horizontal, WhooshSpacing.s4)
                .contentShape(Rectangle())
                .modifier(HoverAck(radius: WhooshRadius.item))
            }
            .buttonStyle(.plain)
            .help(model.header.open ? QuickReplyCopy.collapseHeader : QuickReplyCopy.expandHeader)
            .accessibilityLabel(model.header.open ? QuickReplyCopy.collapseHeader : QuickReplyCopy.expandHeader)
            .whooshAx(QuickReplyIds.headerToggle)
            Spacer(minLength: WhooshSpacing.s8)
            QuickReplyAIPill(model: model, app: app)   // reply-thread: "✦ Brouillon ⌘J", the AI mark and its discard
            Button { _ = model.popOut(app) } label: {
                HStack(spacing: WhooshSpacing.s4) {
                    Image(systemName: "arrow.up.forward.square").font(.system(size: WhooshTypography.caption.size + WhooshSpacing.s2))
                        .foregroundStyle(WhooshPalette.secondaryLabel).accessibilityHidden(true)
                    KeyCap(QuickReplyCopy.popoutCap)
                }
                .padding(.horizontal, WhooshSpacing.s4)
                .contentShape(Rectangle())
                .modifier(HoverAck(radius: WhooshRadius.item))
            }
            .buttonStyle(.plain)
            .help(QuickReplyCopy.popout).accessibilityLabel(QuickReplyCopy.popout)
            .whooshAx(QuickReplyIds.popout)
        }
        .transition(.opacity)
    }

    private func counting(_ countdown: UndoCountdown) -> some View {
        TimelineView(.animation(minimumInterval: 1.0 / 30)) { ctx in
            let now = ctx.date
            HStack(spacing: WhooshSpacing.s8) {
                LiftRing(fraction: countdown.fraction(at: now))
                Text(QuickReplyCopy.sendingIn(countdown.secondsLeft(at: now))).whooshText(WhooshTypography.caption, emphasized: true)
                    .foregroundStyle(WhooshPalette.label)
                    .contentTransition(reduceMotion ? .opacity : .numericText(countsDown: true))
                Spacer(minLength: WhooshSpacing.s8)
                Button { _ = model.undo(app) } label: {
                    HStack(spacing: WhooshSpacing.s4) {
                        Text(QuickReplyCopy.cancel).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.label)
                        KeyCap(QuickReplyCopy.cancelCap)
                    }
                    .padding(.horizontal, WhooshSpacing.s8).frame(height: WhooshMetrics.segmentHeight)
                    .contentShape(Capsule())
                    .modifier(HoverAck(radius: WhooshMetrics.segmentHeight / 2))
                }
                .buttonStyle(.plain)
                .help(QuickReplyCopy.cancelHelp).accessibilityLabel(QuickReplyCopy.cancelHelp)
                .whooshAx(QuickReplyIds.undo)
            }
            .onChange(of: countdown.isExpired(at: now)) { _, gone in if gone { model.expire(app) } }
        }
        .transition(.opacity)
        .whooshAx(QuickReplyIds.undoBar)
    }
}

/// The ring beside "Envoi dans N s" (board `.ring`, 14 pt): the share of the window left, draining clockwise from
/// 12 o'clock like the account mark's first-sync arc. Drawn by the header and by the waiting card.
struct LiftRing: View {
    let fraction: Double
    static let diameter: CGFloat = WhooshSpacing.s14
    var body: some View {
        ZStack {
            Circle().stroke(WhooshPalette.separator, lineWidth: WhooshAccountMark.stroke)
            Circle().trim(from: 0, to: max(0, min(fraction, 1))).rotation(.degrees(WhooshAccountMark.arcStartDegrees))
                .stroke(WhooshPalette.accent, style: StrokeStyle(lineWidth: WhooshAccountMark.stroke, lineCap: .round))
        }
        .frame(width: Self.diameter, height: Self.diameter)
        .accessibilityHidden(true)
    }
}

/// The rows the header opens (board `.qx`): À with one chip per person (name, address, ×), Cc and Cci with their
/// "Ajouter" fields, Objet with the subject. A click on a row takes the socket's `type` until the field is clicked.
struct QuickReplyHeaderRows: View {
    let model: QuickReplyModel
    /// The header object itself (a `let` of the model, so its fields bind here, not through the model).
    @Bindable var header: QuickReplyHeaderState
    init(model: QuickReplyModel) { self.model = model; self.header = model.header }

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s4) {
        // A grid: the label column takes the widest label of the language (never a word broken in two; owner 23:04).
        Grid(alignment: .leading, horizontalSpacing: WhooshSpacing.s8, verticalSpacing: WhooshSpacing.s4) {
            row(QuickReplyCopy.to) {
                HStack(spacing: WhooshSpacing.s4) {
                    ForEach(Array(header.to.enumerated()), id: \.element.id) { i, r in chip(i, r) }
                    Spacer(minLength: 0)
                }
            }
            row(QuickReplyCopy.cc, z: 3) { entry(.cc, text: $header.cc, ax: QuickReplyIds.headerCc) }      // a line's people list draws over the lines under it
            row(QuickReplyCopy.bcc, z: 2) { entry(.bcc, text: $header.bcc, ax: QuickReplyIds.headerBcc) }
            row(QuickReplyCopy.subject) { entry(.subject, text: $header.subject, ax: QuickReplyIds.headerSubject, placeholder: "") }
        }
        .zIndex(1)   // the people list of a Cc or Cci line opens over what is under the rows
        if let words = header.refusal {
            HStack(spacing: WhooshSpacing.s6) {
                Image(systemName: "exclamationmark.circle.fill").foregroundStyle(WhooshPalette.warning).accessibilityHidden(true)
                Text(words).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.label).lineLimit(2)
            }
            .whooshAx(QuickReplyIds.headerRefusal)
        }
        }
        .padding(.bottom, WhooshSpacing.s2)
        .overlay(alignment: .bottom) { Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator) }
    }

    private func row<Content: View>(_ label: String, z: Double = 0, @ViewBuilder content: () -> Content) -> some View {
        GridRow(alignment: .center) {
            Text(label).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                .lineLimit(1).fixedSize()
            content().frame(maxWidth: .infinity, alignment: .leading).zIndex(z)
        }
        .frame(minHeight: WhooshSpacing.s24)
    }

    /// One person on the To line: name, address, and × (board `.chipr`).
    private func chip(_ i: Int, _ r: ReplyRecipient) -> some View {
        HStack(spacing: WhooshSpacing.s4) {
            Text(r.name.isEmpty ? r.address : r.name).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.label).lineLimit(1)
            if !r.name.isEmpty {
                Text(r.address).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1).truncationMode(.middle)
                    .layoutPriority(-1)
            }
            Button { _ = model.removeRecipient(i) } label: {
                Image(systemName: "xmark").font(.system(size: WhooshTypography.caption2.size, weight: .bold))
                    .foregroundStyle(WhooshPalette.secondaryLabel)
                    .frame(width: WhooshSpacing.s16, height: WhooshSpacing.s16)
                    .contentShape(Circle())
                    .modifier(HoverAck(radius: WhooshSpacing.s8))
            }
            .buttonStyle(.plain)
            .help(QuickReplyCopy.remove).accessibilityLabel(QuickReplyCopy.remove)
            .whooshAx(QuickReplyIds.headerToRemove(i))
        }
        .padding(.leading, WhooshSpacing.s8).padding(.trailing, WhooshSpacing.s4)
        .frame(height: WhooshSpacing.s20)
        .background(WhooshPalette.fill, in: Capsule())
        .whooshAx(QuickReplyIds.headerTo(i))
    }

    private func entry(_ field: QuickReplyHeaderState.Field, text: Binding<String>, ax: String, placeholder: String? = nil) -> some View {
        let core = AppModel.shared.core
        return TextField(placeholder ?? QuickReplyCopy.add, text: text)
            .textFieldStyle(.plain)
            .whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.label)
            .simultaneousGesture(TapGesture().onEnded { _ = model.focusHeader(field) })
            // Cc and Cci: the people for the word being typed; ↑ ↓ choose, ↩ or ⇥ takes the line. A change of the line clears the refusal sentence.
            .onChange(of: text.wrappedValue) { _, _ in header.offerIndex = 0; if field != .subject { header.refusal = nil } }
            .onKeyPress(.downArrow) { header.moveOffer(1, field, core: core) ? .handled : .ignored }
            .onKeyPress(.upArrow) { header.moveOffer(-1, field, core: core) ? .handled : .ignored }
            .onKeyPress(.tab) { header.takeOffer(nil, field, core: core) ? .handled : .ignored }
            .onKeyPress(.return) { header.takeOffer(nil, field, core: core) ? .handled : .ignored }
            .frame(maxWidth: .infinity, alignment: .leading)
            .contentShape(Rectangle())
            .overlay(alignment: .topLeading) { offeredList(field, core: core).offset(y: WhooshSpacing.s24) }
            .whooshAx(ax)
    }

    /// The people offered under a Cc or Cci line (the composer's list, the same parts).
    @ViewBuilder private func offeredList(_ field: QuickReplyHeaderState.Field, core: WhooshCore) -> some View {
        let list = header.offered(field, core: core)
        if !list.isEmpty {
            PickerPanel(width: WhooshSpacing.s32 * 10) {
                ForEach(Array(list.enumerated()), id: \.element.id) { i, c in
                    let on = i == header.offerIndex
                    Button { _ = header.takeOffer(i, field, core: core) } label: {
                        PickerRow(title: c.name.isEmpty ? c.address : c.name, highlighted: on) {
                            Text(c.name.isEmpty ? "" : c.address).whooshText(WhooshTypography.caption).lineLimit(1)
                                .foregroundStyle(on ? WhooshPalette.onAccent : WhooshPalette.secondaryLabel)
                        }
                    }.buttonStyle(.plain).whooshAx(QuickReplyIds.headerOffer(field == .cc ? "cc" : "bcc", i))
                }
            }
            .whooshAx(QuickReplyIds.headerOffers(field == .cc ? "cc" : "bcc"))
        }
    }
}
