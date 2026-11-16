#if os(macOS)
import AppKit
import SwiftUI

/// A window that shows the live trainer (the hint and the keys overlay) over the list column at the spec window
/// sizes, so the socket's `shot` captures it before the shell mounts the views. It hosts the same views on the same
/// models: what the drive did is what the shot shows. Reached only through `mac-trainer.stage.*`.
@MainActor
final class TrainerStage {
    static let shared = TrainerStage()
    enum Size { case wide, narrow
        var frame: NSSize { self == .wide ? NSSize(width: 1280, height: 820) : NSSize(width: 960, height: 600) }
    }
    private var window: NSWindow?
    private var dark = false
    private var size = Size.wide

    @discardableResult
    func show(dark: Bool? = nil, size: Size? = nil) -> Bool {
        if let dark { self.dark = dark }
        if let size { self.size = size }
        let w = window ?? make()
        window = w
        w.appearance = NSAppearance(named: self.dark ? .darkAqua : .aqua)
        w.setContentSize(self.size.frame)
        for other in (NSApp as NSApplication?)?.windows ?? [] where other !== w { other.orderOut(nil) }
        w.makeKeyAndOrderFront(nil)
        w.contentView?.layoutSubtreeIfNeeded()
        w.displayIfNeeded()
        return true
    }

    private func make() -> NSWindow {
        let w = NSWindow(contentRect: NSRect(origin: .zero, size: size.frame), styleMask: [.titled, .closable], backing: .buffered, defer: false)
        w.title = "Whoosh trainer"
        w.isReleasedWhenClosed = false
        w.contentView = NSHostingView(rootView: TrainerStageView())
        w.center()
        return w
    }
}

/// The list column with the inbox rows as plain lines, the hint docked at its foot, the overlay over everything.
struct TrainerStageView: View {
    @State private var model = AppModel.shared

    var body: some View {
        ZStack {
            Rectangle().fill(WhooshPalette.background).ignoresSafeArea()
            VStack(alignment: .leading, spacing: 0) {
                ForEach(Array(model.rows.prefix(8).enumerated()), id: \.element.messageId) { i, r in
                    VStack(alignment: .leading, spacing: 0) {
                        Text(r.senderName).whooshText(WhooshTypography.body, emphasized: r.unread).lineLimit(1)
                        Text(r.subject).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                    }
                    .padding(.horizontal, WhooshSpacing.s16)
                    .frame(maxWidth: .infinity, minHeight: WhooshMetrics.listRow, alignment: .leading)
                    .background(i == model.focusIndex ? AnyShapeStyle(WhooshPalette.fill) : AnyShapeStyle(.clear),
                                in: RoundedRectangle(cornerRadius: WhooshRadius.row))
                    .padding(.horizontal, WhooshMetrics.listRowInset)
                }
                Spacer(minLength: 0)
            }
            .padding(.top, WhooshMetrics.sidebarTop)
            .frame(width: WhooshMetrics.readerMeasure - WhooshSpacing.s32 * 2, alignment: .topLeading)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            TrainerHintView()
                .padding(.bottom, WhooshMetrics.footStrip + WhooshSpacing.s8)
                .padding(.leading, WhooshMetrics.listRowInset + WhooshSpacing.s16)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomLeading)
            TrainerOverlayView()
        }
    }
}
#endif
