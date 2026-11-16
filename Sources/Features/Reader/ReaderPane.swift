import SwiftUI

/// The reading pane the shell's reader slot hosts: the conversation of the shown message while the surface is
/// "mac-reader" (and at rest before Enter), else the empty state. The pane follows the list's focus through
/// `ReaderFocus`: a passing focus changes nothing here, the settled one cross-fades under a short blur over
/// READER_CROSSFADE_MS, and nothing waits for the animation.
struct ReaderPane: View {
    @State private var model = AppModel.shared  // local-ui: the shared app model, observed
    @State private var reader = ReaderState.shared  // local-ui: the reader's own fold state, observed
    @State private var quick = QuickReplyModel.shared  // local-ui: the quick reply's state, observed (its bar is mounted below)
    @State private var focus = ReaderFocus.shared  // local-ui: which message the pane shows, observed
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    /// labels r2 hook: the label inspector (⌘I) takes the pane in its second mode; the reader cross-fades back on Esc.
    var body: some View {
        ZStack {
            if LabelInspector.isOpen { LabelInspectorView().transition(.readerSwap(reduceMotion: reduceMotion)) }
            else { readerBody.transition(.readerSwap(reduceMotion: reduceMotion)) }
        }
        .animation(WhooshMotion.animation(WhooshMotion.readerSwap, reduceMotion: reduceMotion), value: LabelInspector.isOpen)
    }

    @ViewBuilder private var readerBody: some View {
        let _ = ReaderPerf.shared.notePaneEval()
        let _ = LaunchClock.once("reader-body")   // perf-archive r4: a launch phase
        // A129: a selected mail is a reading view: its whole body shows with no extra key. The build reads no body (OpenBodyHost does, once).
        let thread = ReaderThread.make(model, messageId: focus.target(model), resting: model.surface != ReaderState.surface, withBody: false)
        ZStack {
            if let thread {
                ConversationView(thread: thread, account: RowAccount.of(thread.accountId, in: model),
                                 earlierOpen: reader.earlierShown(thread), note: reader.deletedNote, copied: reader.copiedFor == thread.messageId,
                                 passage: focus.passage.flatMap { $0.isFor(thread) ? $0 : nil },
                                 quickOpen: quick.showsBox && quick.threadId == thread.threadId,
                                 openSender: { _ = ReaderActions.openSender(model) }, copyCode: { _ = ReaderActions.copyCode(model) },
                                 toggleEarlier: { withAnimation(WhooshMotion.animation(WhooshMotion.unfold, reduceMotion: reduceMotion)) { reader.toggleEarlier(thread) } })
                    .id(thread.messageId)
                    .task(id: thread.threadId) { await ReaderSummaryStore.shared.load(model, threadId: thread.threadId) }
                    .transition(.readerSwap(reduceMotion: reduceMotion))
            } else {
                ReaderEmpty(note: reader.deletedNote)
                    .transition(.readerSwap(reduceMotion: reduceMotion))
            }
        }
        .animation(WhooshMotion.animation(WhooshMotion.readerSwap, reduceMotion: reduceMotion), value: thread?.messageId)
        .onChange(of: thread?.messageId) { ReaderPerf.shared.noteSwap() }
        .onAppear { focus.ensure(model) }
        // The window's foot strip (macshell r9) carries the key hints, and the shell keeps its room under the reader slot (MacShellRoot, safeAreaInset): the pane
        // adds none. Both kept 36 pt since 9 Nov, so the content ended 36 pt above the strip: a band of empty pane under a cut reply box (the lead's walk, main184).
        .whooshAx("mac-main.reader.content")
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(WhooshPalette.background)
        .readerFrame("pane")
        .coordinateSpace(name: ReaderLayout.space)
        .accessibilityIdentifier("mac-main.reader")
    }
}

/// One thread as a conversation: header, the earlier-messages rule, the open message, your reply block, the keys.
struct ConversationView: View {
    let thread: ReaderThread
    let account: RowAccount
    let earlierOpen: Bool
    let note: String?
    let copied: Bool
    /// What the selected search result hands the reader for this conversation (A152), or nil: the mail that holds it opens and the passage is marked.
    var passage: ReaderPassage?
    /// The quick reply is open under this thread: its bar takes the foot, the reply block steps aside.
    let quickOpen: Bool
    let openSender: () -> Void
    let copyCode: () -> Void
    let toggleEarlier: () -> Void
    @State private var position = ScrollPosition(edge: .top)  // local-ui: the scroll view's offset, driven by the keys
    @State private var scroll = ReaderScroll.shared  // local-ui: scroll requests from the keys, observed

    var body: some View {
        VStack(spacing: 0) {
            // the action icons sit in the band above the title, at the pane's right edge
            HStack { Spacer(minLength: 0); ReaderHeaderActions() }
                .padding(.horizontal, WhooshMetrics.readerSide - WhooshSpacing.s8)
                .padding(.top, WhooshSpacing.s16)
            if let note {
                Text(note).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                    .padding(.horizontal, WhooshMetrics.readerSide)
                    .frame(maxWidth: .infinity, alignment: .leading).accessibilityIdentifier("mac-main.reader.deleted")
            }
            ReaderActionNotice(sender: thread.sender)
            ReaderHeader(thread: thread, account: account)
                .padding(.horizontal, WhooshMetrics.readerSide)
                .padding(.top, WhooshMetrics.readerTop - WhooshSpacing.s16 - ReaderIconButton.height)
                .padding(.bottom, WhooshSpacing.s20)
            ScrollViewReader { proxy in
                ScrollView { blocks }
                    .scrollBounceBehavior(.basedOnSize)
                    .defaultScrollAnchor(.top)
                    .scrollPosition($position)
                    .onScrollGeometryChange(for: ReaderScroll.Metrics.self) { g in
                        ReaderScroll.Metrics(offset: g.contentOffset.y, content: g.contentSize.height, viewport: g.containerSize.height)
                    } action: { _, new in ReaderScroll.shared.metrics = new; ReaderScroll.shared.followEnd() }
                    .onChange(of: scroll.request) { _, r in position.scrollTo(y: r.y) }
                    .onChange(of: scroll.endEdge) { _, _ in position.scrollTo(edge: .bottom) }
                    .onChange(of: scroll.reveal) { _, r in proxy.scrollTo(r.id, anchor: UnitPoint(x: 0.5, y: r.anchor)) }
                    .onAppear { revealPassage(proxy) }
                    .onChange(of: passage) { _, _ in revealPassage(proxy) }
            }
            .frame(maxHeight: .infinity, alignment: .top)
            // The box stays MOUNTED through its send's window, drawn or not: the send animation starts from its Send button (owner, 13 Nov,
            // picture 208 on main126: "animation don't work on send anymore"; the box left the tree at the send and took the animation with it).
            .overlay(alignment: .bottom) { if quickOpen || (QuickReplyModel.shared.outboxId != nil && QuickReplyModel.shared.threadId == thread.threadId) { QuickReplyBar(inset: .reader) } }   // quickreply-r5 hook: the slab floats over the thread's foot
        }
    }

    private var blocks: some View {
        let marked = ReaderState.shared.focusedIndex
        let expanded = ReaderState.shared.expandedShown(thread)
        let held = passage?.earlierIndex(in: thread)
        // the blocks under the open mail's text are drawn once the text has its height (BodySettle.swift): before, they stood under the header and jumped
        let openId = thread.messageId
        return VStack(spacing: WhooshMetrics.blockGap) {
            if let words = ReaderSummaryStore.shared.texts[thread.threadId] { ReaderSummaryCard(text: words) }
            if thread.earlierCount > 0 {
                EarlierRule(count: thread.earlierCount, open: earlierOpen, toggle: toggleEarlier)
                if earlierOpen {
                    ForEach(0..<thread.earlierCount, id: \.self) { i in
                        let fact = i < thread.earlier.count ? thread.earlier[i] : nil
                        EarlierMessageBlock(index: i, fact: fact, expanded: expanded.contains(i), marked: marked == i, links: linkRow(fact?.id),
                                            passage: i == held ? passage : nil)
                            .id(ReaderScroll.blockId(i))
                    }
                }
            }
            OpenMessageBlock(thread: thread, copied: copied, openSender: openSender, copyCode: copyCode,
                             stripVisible: ReaderState.shared.headerHover || (!thread.resting && MacShellState.shared.focusedPane == .reader && !QuickReplyModel.shared.fieldFocused),   // lead, 13 Nov: not while a reply is being typed (the keys are the text's, and the strip covered the mail's date)
                             quickOpen: quickOpen, marked: marked == thread.earlierCount, links: linkRow(thread.messageId),
                             passage: passage?.holdsOpenMail(thread) == true ? passage : nil)
                .id(ReaderScroll.blockId(thread.earlierCount))
            QuickReplyThreadCards(threadId: thread.threadId).modifier(UnderTextShown(messageId: openId))   // my sends and the thread's draft (quickreply round 4)
            // One entry to the reply (owner, 13 Nov, picture 206): the open box, else the conversation's draft card, else this closed line.
            if !quickOpen, !ThreadCardsRegistry.showsDraftCard(threadId: thread.threadId) { ReplyBlock(thread: thread, account: account) { _ = ReaderActions.run("reply") }.modifier(UnderTextShown(messageId: openId)) }
        }
        .onPreferenceChange(BodySettleKey.self) { v in if v == .shown { ReaderUnderText.shared.show(openId) } }
        .task(id: openId) {      // a page that never answers hides nothing for good
            try? await Task.sleep(for: .milliseconds(WhooshConstants.BODY_UNDER_TEXT_WAIT_MS))
            if !Task.isCancelled { ReaderUnderText.shared.show(openId) }
        }
        .padding(.horizontal, WhooshMetrics.readerSide)
        .padding(.bottom, WhooshSpacing.s16)
        .accessibilityIdentifier("mac-main.reader.thread")
    }

    /// The reader opens at the mail that holds the passage: an earlier mail is brought into view (the open mail is the last block and its text mark brings itself:
    /// the page says where the mark stands, `OpenBodyHost`); an attachment's tile is at the open mail's foot. No animation: the pane's own scroll, one jump.
    private func revealPassage(_ proxy: ScrollViewProxy) {
        guard let p = passage else { return }
        let id: String, anchor: Double
        if let i = p.earlierIndex(in: thread) { id = ReaderScroll.blockId(i); anchor = 0.5 }
        else if p.source == .attachment, p.holdsOpenMail(thread) { id = ReaderScroll.blockId(thread.earlierCount); anchor = 1 }
        else { return }
        DispatchQueue.main.async { proxy.scrollTo(id, anchor: UnitPoint(x: 0.5, y: anchor)) }   // after this layout pass: the blocks have their ids by then
    }

    /// The ring row, under the message the ring is on.
    private func linkRow(_ id: String?) -> ReaderLinkRow? {
        let links = ReaderLinks.shared
        guard let id, links.messageId == id, links.cursor != nil else { return nil }
        return ReaderLinkRow(items: links.items, cursor: links.cursor) { _ = ReaderLinks.shared.openCursor(AppModel.shared, at: $0) }
    }
}
