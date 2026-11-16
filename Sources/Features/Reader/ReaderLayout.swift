import SwiftUI

/// The reader's measured frames, in the pane's own coordinate space, for the `mac-main.reader.layout` probe: the pane,
/// the summary callout and the open message block. Written when a frame changes; no view reads them, so a write
/// never redraws anything.
@MainActor
final class ReaderLayout {
    static let shared = ReaderLayout()
    static let space = "mac-main.reader.pane"
    var frames: [String: CGRect] = [:]

    /// Pane right edge minus the named block's right edge, points; nil while the block is not on screen.
    func gap(_ name: String) -> Double? {
        guard let pane = frames["pane"], let f = frames[name] else { return nil }
        return Double(pane.maxX - f.maxX)
    }
}

extension View {
    /// Reports this view's frame in the pane's space under `name`.
    func readerFrame(_ name: String) -> some View {
        onGeometryChange(for: CGRect.self) { $0.frame(in: .named(ReaderLayout.space)) } action: { ReaderLayout.shared.frames[name] = $0 }
    }
}
