import SwiftUI

/// Draws the thread over the iPhone inbox while `AppModel.overlay` is "iphone-thread" (the inbox sets it on a row tap).
/// The inbox's root mounts it with one line (docs/lanes/threadphone.md "Wiring"). It slides in from the trailing edge on the
/// short insert curve; nothing waits for it.
public struct ThreadPhoneHost: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private let m = AppModel.shared

    public init() {}

    public var body: some View {
        let up = m.overlay == ThreadPhoneState.overlayId
        ZStack {
            if up { ThreadPhoneScreen().transition(reduceMotion ? .opacity : .move(edge: .trailing)) }
        }
        .animation(reduceMotion ? nil : WhooshMotion.newRow, value: up)
        .onChange(of: up) { _, now in if !now { ThreadPhoneState.shared.dismissed() } }
    }
}

/// The iPhone thread (layout mail-26's own conversation screen, style glass-light): paper-white, the messages as cards,
/// glass only on what floats (Back, the code pill, the bar, the More panel). Loading, empty and error each have a state:
/// the thread draws from the local store at once, a body not on the phone says "Not downloaded yet", a message the core
/// no longer lists shows the empty state with Back.
public struct ThreadPhoneScreen: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.colorScheme) private var scheme
    private let m = AppModel.shared
    private let state = ThreadPhoneState.shared

    public init() {}

    public var body: some View {
        let thread = state.thread(m)
        ZStack(alignment: .bottom) {
            PhoneGround.list.ignoresSafeArea()
            if let thread { conversation(thread) } else { unavailable }
            fade
            if state.moreOpen {
                Color.clear.contentShape(Rectangle()).onTapGesture { state.moreOpen = false }
                    .accessibilityHidden(true).ignoresSafeArea()
            }
            VStack(alignment: .trailing, spacing: WhooshSpacing.s12) {
                if thread != nil {
                    if state.moreOpen {
                        ThreadMorePanel(state: state, pinned: pinned(thread))
                            .padding(.trailing, PhoneMetrics.gutter)
                            .transition(reduceMotion ? .opacity : .scale(scale: ThreadMetrics.panelFrom, anchor: .bottomTrailing).combined(with: .opacity))
                    }
                    if let words = state.failure {
                        ThreadFailureLine(words: words) { state.clearFailure() }
                            .frame(maxWidth: .infinity).transition(.opacity)
                    }
                    ThreadPhoneBar(state: state)
                }
            }
            .animation(reduceMotion ? nil : WhooshMotion.newRow, value: state.moreOpen)
            .animation(reduceMotion ? nil : WhooshMotion.newRow, value: state.failure)
        }
        .overlay(alignment: .top) {
            HStack {
                GlassCircle(symbol: "chevron.left", side: PhoneMetrics.roundButton, label: ThreadPhoneWords.back) { _ = state.close(m) }
                    .accessibilityIdentifier("iphone-thread.back")
                Spacer()
            }
            .padding(.horizontal, PhoneMetrics.gutter)
        }
        .sheet(isPresented: Binding(get: { state.noteOpen }, set: { state.noteOpen = $0 })) { ThreadNoteSheet(state: state) }
        .simultaneousGesture(edgeBack)
        .onAppear { state.begin(m) }
    }

    private func pinned(_ thread: ReaderThread?) -> Bool {
        guard let id = thread?.messageId else { return false }
        return m.rows.first { $0.messageId == id }?.pinned ?? false
    }

    private func unread(_ thread: ReaderThread) -> Bool {
        m.rows.first { $0.messageId == thread.messageId }?.unread ?? false
    }

    private func conversation(_ thread: ReaderThread) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: ThreadMetrics.cardGap) {
                ThreadHeader(thread: thread, account: PhoneAccounts.of(thread.accountId, in: m))
                    .padding(.top, PhoneMetrics.titleTop)
                ForEach(thread.entries(earlierOpen: false), id: \.index) { e in
                    if e.index < thread.earlierCount {
                        ThreadFoldedCard(index: e.index, fact: e.fact, expanded: state.expanded.contains(e.index)) {
                            withAnimation(reduceMotion ? nil : WhooshMotion.unfold) { state.toggleExpanded(e.index) }
                        }
                    } else {
                        ThreadOpenCard(thread: thread, unread: unread(thread), copied: state.copiedFor == thread.messageId) { _ = state.copyCode(m) }
                    }
                }
            }
            .padding(.bottom, PhoneMetrics.listBottomRoom)
        }
        .scrollIndicators(.hidden)
        .refreshable { _ = state.reload(m) }
        .accessibilityIdentifier("iphone-thread.list")
    }

    private var unavailable: some View {
        PhoneEmpty(symbol: "envelope", title: ThreadPhoneWords.unavailableTitle, line: ThreadPhoneWords.unavailableLine)
            .frame(maxHeight: .infinity, alignment: .top).padding(.top, PhoneMetrics.titleTop + PhoneMetrics.roundButton)
            .accessibilityIdentifier("iphone-thread.unavailable")
    }

    /// `.edgeB`: the card text fades to paper before it reaches the glass bar.
    private var fade: some View {
        LinearGradient(stops: [.init(color: PhoneGround.list(scheme).opacity(0), location: 0), .init(color: PhoneGround.list(scheme), location: 0.72)],
                       startPoint: .top, endPoint: .bottom)
            .frame(height: PhoneMetrics.edgeFade).frame(maxHeight: .infinity, alignment: .bottom)
            .ignoresSafeArea(edges: .bottom).allowsHitTesting(false)
    }

    /// A drag that starts at the leading edge and goes right past `edgeBackPast` leaves the thread, like the system's Back.
    private var edgeBack: some Gesture {
        DragGesture(minimumDistance: ThreadMetrics.edgeBackMin, coordinateSpace: .global).onEnded { v in
            if v.startLocation.x < ThreadMetrics.edgeBackStart, v.translation.width > ThreadMetrics.edgeBackPast,
               abs(v.translation.height) < v.translation.width { _ = state.close(m) }
        }
    }
}
