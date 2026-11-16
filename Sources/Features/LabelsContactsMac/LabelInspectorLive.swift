import SwiftUI

/// Wraps its children onto new lines at the width it is given (rule values wrap under their pop-up).
struct FlowLayout: Layout {
    var spacing: CGFloat = 4
    /// The gap between two lines; nil: the same as between two children of a line.
    var lineSpacing: CGFloat? = nil
    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        arrange(proposal.width ?? .infinity, subviews).size
    }
    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let a = arrange(bounds.width, subviews)
        for (i, p) in a.points.enumerated() { subviews[i].place(at: CGPoint(x: bounds.minX + p.x, y: bounds.minY + p.y), proposal: .unspecified) }
    }
    private func arrange(_ width: CGFloat, _ subviews: Subviews) -> (size: CGSize, points: [CGPoint]) {
        var x: CGFloat = 0, y: CGFloat = 0, row: CGFloat = 0, maxX: CGFloat = 0
        var points: [CGPoint] = []
        for v in subviews {
            let s = v.sizeThatFits(.unspecified)
            if x > 0, x + s.width > width { x = 0; y += row + (lineSpacing ?? spacing); row = 0 }
            points.append(CGPoint(x: x, y: y))
            x += s.width + spacing; row = max(row, s.height); maxX = max(maxX, x - spacing)
        }
        return (CGSize(width: width.isFinite ? width : maxX, height: y + row), points)
    }
}

extension LabelInspectorModel {
    /// A rule made from a message: its sender's domain (a message dropped on the zone, or the focused row on a click), on the first Entry chain.
    @discardableResult
    func ruleFromMessage(_ messageId: String) -> Bool {
        guard let a = core.message(messageId: messageId)?.fromAddress, let domain = a.split(separator: "@").last.map(String.init), !domain.isEmpty else { return false }
        edit { d in
            if let i = d.rules.firstIndex(where: { $0.kind == .domain }) {
                if !d.rules[i].values.contains(domain) { d.rules[i].values.append(domain) }
            } else { d.rules.append(RuleDraft(kind: .domain, values: [domain])) }
        }
        return true
    }
}

/// "Drop a message here to make a rule from it": one quiet line under the Entry card that takes a message id dropped on it, or the focused row on a click.
struct DropHint: View {
    @Bindable var model: LabelInspectorModel
    @State private var over = false  // local-ui: the drop highlight
    var body: some View {
        Button { if let id = model.app.focusedMessageId { _ = model.ruleFromMessage(id) } } label: {
            HStack(spacing: WhooshSpacing.s6) {
                Image(systemName: "line.3.horizontal").rotationEffect(.degrees(90)).font(WhooshTypography.caption.font).imageScale(.small)
                Text(Words.text("Drop a message here to make a rule from it")).whooshText(WhooshTypography.caption).lineLimit(1)
                Spacer(minLength: 0)
            }
            .foregroundStyle(over ? WhooshPalette.accent : WhooshPalette.secondaryLabel)
            .padding(.horizontal, WhooshSpacing.s4).padding(.vertical, WhooshSpacing.s2).contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .dropDestination(for: String.self) { ids, _ in ids.first.map(model.ruleFromMessage) ?? false } isTargeted: { over = $0 }
        .animation(WhooshMotion.hover, value: over)
        .help(Words.text("Drop a message here, or click to use the message in focus"))
        .accessibilityIdentifier("mac-labels.inspector.drop")
    }
}
