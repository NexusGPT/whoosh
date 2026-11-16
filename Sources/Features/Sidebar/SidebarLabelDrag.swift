import SwiftUI

/// Re-arranging labels by drag (owner 2026-11-08 09:45: "re-arrange by drag"). The pointer over a row's top quarter drops the label
/// before it, over its bottom quarter after it, between them inside it (as its sub-label), over the Labels header at the top level.
/// The result is the core's `reorderLabel`: the saved order is the tab order of the pinned labels; the core refuses in words a drop
/// into itself or past LABEL_DEPTH_MAX.
enum LabelDropZone: String { case before, into, after }

struct LabelDropTarget: Equatable {
    /// A label id, or `topId` for the Labels header.
    let rowId: String
    let zone: LabelDropZone
    static let topId = "top"
}

enum LabelDrag {
    static let space = "sidebarLabels"

    /// The row under the pointer (never the dragged one) and the zone of the row the pointer is in.
    static func target(at point: CGPoint, frames: [String: CGRect], dragging: String) -> LabelDropTarget? {
        guard let hit = frames.first(where: { $0.key != dragging && $0.value.contains(point) }) else { return nil }
        if hit.key == LabelDropTarget.topId { return LabelDropTarget(rowId: hit.key, zone: .into) }
        let f = (point.y - hit.value.minY) / max(1, hit.value.height)
        return LabelDropTarget(rowId: hit.key, zone: f < 0.25 ? .before : (f > 0.75 ? .after : .into))
    }

    /// The `reorderLabel` arguments for a drop: the new parent and the index among that parent's other children.
    static func destination(_ t: LabelDropTarget, moving id: String, labels: [MailLabel]) -> (parent: String?, index: UInt32)? {
        if t.rowId == LabelDropTarget.topId { return (nil, UInt32.max) }
        guard let row = labels.first(where: { $0.id == t.rowId }) else { return nil }
        if t.zone == .into { return (row.id, UInt32.max) }
        let sibs = labels.filter { $0.parentId == row.parentId && $0.id != id }
        let at = sibs.firstIndex { $0.id == row.id } ?? sibs.count
        return (row.parentId, UInt32(t.zone == .before ? at : at + 1))
    }
}

/// Where each label row sits in the section's coordinate space, so a drag can find the row under the pointer.
struct LabelRowFrames: PreferenceKey {
    static let defaultValue: [String: CGRect] = [:]
    static func reduce(value: inout [String: CGRect], nextValue: () -> [String: CGRect]) { value.merge(nextValue()) { $1 } }
}

extension View {
    func reportLabelFrame(_ id: String) -> some View {
        background(GeometryReader { g in
            Color.clear.preference(key: LabelRowFrames.self, value: [id: g.frame(in: .named(LabelDrag.space))])
        })
    }
}

/// The drag in flight: the label, the pointer and where a drop would land. `held` marks a drag the socket left mid-way for a shot.
struct SidebarLabelDragState: Equatable {
    var id: String
    var point: CGPoint
    var target: LabelDropTarget?
    var held = false
}

/// The thin line or outline that shows where the dragged label would land.
struct LabelDropMark: View {
    let zone: LabelDropZone?
    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: WhooshRadius.control).strokeBorder(WhooshPalette.accent, lineWidth: 2).opacity(zone == .into ? 1 : 0)
            VStack {
                Capsule().fill(WhooshPalette.accent).frame(height: 2).opacity(zone == .before ? 1 : 0)
                Spacer(minLength: 0)
                Capsule().fill(WhooshPalette.accent).frame(height: 2).opacity(zone == .after ? 1 : 0)
            }
        }
        .animation(WhooshMotion.hover, value: zone)
        .allowsHitTesting(false)
    }
}

extension SidebarState {
    /// Pins or unpins a label (a pinned label is a tab at the top of the list). The core refuses unpinning the last one in words.
    @discardableResult func togglePin(_ id: String, model: AppModel) -> Bool {
        guard let cur = SidebarData.labels(core: model.core).first(where: { $0.id == id })?.label.pinned else { return false }
        let ok = perform(.pinLabel(labelId: id, pinned: !cur), model: model)
        if ok { revision += 1 }
        return ok
    }

    /// A drop on `target`: the core saves the new parent and position (the gesture's release and the drive's `drop-<zone>` both land here).
    @discardableResult func land(_ target: LabelDropTarget, moving id: String, model: AppModel) -> Bool {
        drag = nil
        guard let d = LabelDrag.destination(target, moving: id, labels: SidebarData.labels(core: model.core).map(\.label)) else { return false }
        return moveLabel(id, parent: d.parent, index: d.index, model: model)
    }

    /// The drop: the core saves the new parent and position (the tab order of the pinned labels follows the tree).
    @discardableResult func moveLabel(_ id: String, parent: String?, index: UInt32, model: AppModel) -> Bool {
        let ok = perform(.reorderLabel(labelId: id, newParentId: parent, newIndex: index), model: model)
        if ok {
            if let parent { openLabels.insert(parent) }
            revision += 1
        }
        return ok
    }
}

extension SidebarFeature {
    static func labelRows(_ st: SidebarState) -> [String: (AppModel) throws -> [ControlRow]] {
        [
            "mac-main.sidebar.drag": { _ in
                [["active": st.drag != nil, "id": st.drag?.id ?? "", "target": st.drag?.target?.rowId ?? "", "zone": st.drag?.target?.zone.rawValue ?? "",
                  "held": st.drag?.held ?? false]]
            },
            "mac-main.sidebar.pick-items": { _ in SidebarVisibility.pickRows() },
        ]
    }

    static func labelClicks(_ st: SidebarState) -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [
            "mac-main.sidebar.drag.cancel": { _ in guard st.drag != nil else { return false }; st.drag = nil; return true },
        ]
        for l in SidebarData.labels() {
            t["mac-main.sidebar.label.\(l.id).pin"] = { m in st.togglePin(l.id, model: m) }
            /// The drive cannot hover: this forces the row's hover state, which shows the pin.
            t["mac-main.sidebar.label.\(l.id).hover"] = { _ in st.hoverLabel = st.hoverLabel == l.id ? nil : l.id; return true }
            /// The drive cannot hold a pointer down: `lift` takes the label up (a held drag), `over-<zone>` moves it over another
            /// label (the outline or line shows), `drop-<zone>` lands it through the same function the gesture's release calls.
            t["mac-main.sidebar.label.\(l.id).lift"] = { _ in
                st.drag = SidebarLabelDragState(id: l.id, point: st.labelFrames[l.id].map { CGPoint(x: $0.midX, y: $0.midY) } ?? .zero, target: nil, held: true)
                return true
            }
            for zone in [LabelDropZone.before, .into, .after] {
                t["mac-main.sidebar.label.\(l.id).over-\(zone.rawValue)"] = { _ in
                    guard var d = st.drag, d.held, d.id != l.id, let f = st.labelFrames[l.id] else { return false }
                    d.target = LabelDropTarget(rowId: l.id, zone: zone)
                    d.point = CGPoint(x: f.midX, y: f.midY)
                    st.drag = d
                    return true
                }
                t["mac-main.sidebar.label.\(l.id).drop-\(zone.rawValue)"] = { m in
                    guard let d = st.drag, d.held, d.id != l.id else { return false }
                    return st.land(LabelDropTarget(rowId: l.id, zone: zone), moving: d.id, model: m)
                }
            }
        }
        return t
    }
}
