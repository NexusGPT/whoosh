import SwiftUI

/// docs/MOTION.md rule 11 on the list: the focus highlight slides from the old row to the new one only when the two
/// are at most SLIDE_MAX_ROWS rows apart; further than that the old highlight fades out in place and the new one fades
/// in at its destination (HOVER_TINT_MS). The focus itself lands on the key's frame; the highlight only follows it,
/// interruptible, and never gates the next key (laws 1 and 2). `SelectionMotion` is the sidebar's word for the same rule.
struct FocusMark: Equatable {
    /// the focus index the highlight last settled on
    var last = 0
    /// when the focus last moved: a second key inside FOCUS_SLIDE_MS jumps, with no slide and no fade
    var lastMoveAt: CFAbsoluteTime = 0
    /// bumped on every fade: the highlight's geometry id changes, so the old one leaves and the new one arrives
    var epoch = 0

    func motion(to i: Int) -> SelectionMotion {
        if i == last { return .none }
        return abs(i - last) <= WhooshConstants.SLIDE_MAX_ROWS ? .slide : .fade
    }

    /// The geometry id the focused row draws its highlight under, during the very render the focus moved in.
    func key(to i: Int) -> Int { motion(to: i) == .fade ? epoch + 1 : epoch }

    /// true when this move lands inside FOCUS_SLIDE_MS of the last one: it jumps (docs/MOTION.md rules 3 and 11)
    func rapid(now: CFAbsoluteTime = CFAbsoluteTimeGetCurrent()) -> Bool {
        (now - lastMoveAt) * 1000 < Double(WhooshConstants.FOCUS_SLIDE_MS)
    }

    func animation(to i: Int, reduceMotion: Bool) -> Animation? {
        if rapid() { return nil }
        return motion(to: i) == .slide && !reduceMotion ? WhooshMotion.tabThumb : WhooshMotion.hover
    }

    /// After the move has rendered: the next move counts from here.
    mutating func settle(at i: Int) {
        if motion(to: i) == .fade { epoch += 1 }
        last = i
        lastMoveAt = CFAbsoluteTimeGetCurrent()
    }
}

/// The focused row's fill. One per list: a slide keeps its geometry id, a fade changes it.
struct FocusHighlight: View {
    let ns: Namespace.ID
    let key: Int
    var body: some View {
        RoundedRectangle(cornerRadius: WhooshRadius.row, style: .continuous)
            .fill(WhooshPalette.selection)
            .matchedGeometryEffect(id: "mac-main.list.selection.\(key)", in: ns)
            .transition(.opacity)
    }
}
