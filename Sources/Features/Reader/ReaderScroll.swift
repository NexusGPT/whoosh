import SwiftUI
import Observation

/// The reader's scroll, as keys drive it (space, ⇧space, ⌘↑, ⌘↓) and as a message mark asks to be seen (N, P).
/// The pane's ScrollView reports its geometry here and performs `request` and `reveal` when they change. The
/// offset a key asks for is stored on the key's frame, so a read of the state is right before the view catches up.
@MainActor @Observable
final class ReaderScroll {
    static let shared = ReaderScroll()

    struct Metrics: Equatable {
        var offset = 0.0
        var content = 0.0
        var viewport = 0.0
    }
    struct Request: Equatable { var n = 0; var y = 0.0 }
    /// `anchor`: where in the block the view's own point of interest stands (0 top, 1 bottom); 0.5 brings the block's middle to the middle of the pane.
    struct Reveal: Equatable { var n = 0; var id = ""; var anchor = 0.5 }

    /// Written by the ScrollView's geometry callback and by a key's own request; read by probes and key handlers.
    @ObservationIgnored var metrics = Metrics()
    /// Observed by the pane: a new `n` means "scroll to `y`" or "bring `id` into view".
    private(set) var request = Request()
    private(set) var reveal = Reveal()

    var maxOffset: Double { max(0, metrics.content - metrics.viewport) }

    /// One page down (+1) or up (-1): the viewport less a margin, so the last line stays in view. False when the
    /// content does not move (it fits, or the end is already shown), so the key passes on.
    func page(_ direction: Double) -> Bool {
        scroll(to: metrics.offset + direction * max(0, metrics.viewport - Double(WhooshSpacing.s32)))
    }

    /// "To the end" (`.greatestFiniteMagnitude`, the quick reply's reveal) is asked while the slab's room is still being laid out, so the end it clamps to is
    /// the old one (main92: offset 87 of 336 after R). For a moment the end is followed: each new geometry that leaves the offset short of the end scrolls on.
    @ObservationIgnored private var endPinUntil: ContinuousClock.Instant?
    static let endPinMs = 1_200

    /// Counted by every "go to the end": the pane scrolls to its bottom EDGE (`ScrollPosition.scrollTo(edge: .bottom)`), which the ScrollView resolves against
    /// the content it has when it applies it, never against the metrics read a turn earlier (main95: a y taken from those metrics stopped at 311 of 423).
    private(set) var endEdge = 0

    /// Three more looks while the follow is on, after the layout has had time to settle: any that finds the offset short of the end asks for the end by the metrics
    /// of THAT moment (a y, which differs from the offset, so the pane cannot take it for the position it already holds). main103: the offset stopped at 251 of 423,
    /// steady at 0.2, 0.6 and 1.5 s, because no geometry change followed the last scroll, so nothing looked again.
    private func retryEnd() {
        Task { @MainActor [weak self] in
            var spent = 0
            for ms in [150, 400, 800] {
                try? await Task.sleep(for: .milliseconds(ms - spent))
                spent = ms
                guard let self, let until = self.endPinUntil, ContinuousClock.now < until else { return }
                let end = self.maxOffset
                if end - self.metrics.offset > 1 { self.request = Request(n: self.request.n + 1, y: end) }
            }
        }
    }

    func followEnd() {
        guard let until = endPinUntil else { return }
        guard ContinuousClock.now < until else { endPinUntil = nil; return }
        guard maxOffset - metrics.offset > 1 else { return }
        endEdge += 1
    }

    func scroll(to y: Double) -> Bool {
        if y == .greatestFiniteMagnitude {
            endPinUntil = ContinuousClock.now + .milliseconds(Self.endPinMs)
            endEdge += 1
            retryEnd()
            return true
        }
        endPinUntil = nil   // a key's own scroll ends the follow
        let target = min(max(0, y), maxOffset)
        guard abs(target - metrics.offset) > 0.5 else { return false }
        request = Request(n: request.n + 1, y: target)
        metrics.offset = target
        return true
    }

    /// Brings one message block into view (N and P).
    func show(_ id: String, anchor: Double = 0.5) { reveal = Reveal(n: reveal.n + 1, id: id, anchor: anchor) }

    static func blockId(_ index: Int) -> String { "reader.message.\(index)" }
}
