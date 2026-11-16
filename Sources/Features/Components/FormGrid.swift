import SwiftUI

// The form grid: the grouped card, its label/control rows and its section header. The Settings window draws every pane with
// it; the label inspector and the add-account sheet reuse it. Every gap, inset and radius is a token (WhooshSpacing,
// WhooshRadius, WhooshMetrics, WhooshTypography); the contract is docs/lanes/settingswindow.md "## Form grid".

/// A quiet card: a fill, a hairline edge, and a hairline between rows, inset from both sides. No coloured edge, ever:
/// a state is told by a dot, a word or a control inside a row.
public struct GroupedCard<Content: View>: View {
    @ViewBuilder let content: Content
    public init(@ViewBuilder content: () -> Content) { self.content = content() }

    public var body: some View {
        VStack(spacing: 0) {
            Group(subviews: content) { rows in
                ForEach(rows.indices, id: \.self) { i in
                    if i > 0 {
                        Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator)
                            .padding(.horizontal, WhooshSpacing.s14)
                    }
                    rows[i]
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous).fill(WhooshPalette.fill))
        .overlay(RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous)
            .strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator))
    }
}

/// One row of a card: the label and one grey caption at the left, the control at the right. The label column takes what the
/// control leaves; the caption wraps under the label. With a tap the whole row is the target and tints on hover.
public struct FormGridRow<Control: View>: View {
    let title: String
    var caption: String?
    var captionStrong: Bool
    var dot: Color?
    var leading: AnyView?
    var onTap: (() -> Void)?
    var probeId: String?
    @ViewBuilder let control: Control
    @State private var hover = false
    /// The narrowest a label column may be before the control goes under it: four 32 pt steps.
    static var labelMin: CGFloat { WhooshSpacing.s32 * 4 }
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    public init(title: String, caption: String? = nil, captionStrong: Bool = false, dot: Color? = nil, leading: AnyView? = nil,
                onTap: (() -> Void)? = nil, probeId: String? = nil, @ViewBuilder control: () -> Control) {
        self.title = title; self.caption = caption; self.captionStrong = captionStrong; self.dot = dot
        self.leading = leading; self.onTap = onTap; self.probeId = probeId; self.control = control()
    }

    public var body: some View {
        FormSplit(labelMin: Self.labelMin, probeId: probeId, title: title, caption: caption) {
            HStack(alignment: .center, spacing: WhooshSpacing.s10) {
                if let leading { leading }
                if let dot { RoundedRectangle(cornerRadius: WhooshRadius.keyCap, style: .continuous).fill(dot).frame(width: WhooshSpacing.s10, height: WhooshSpacing.s10) }
                VStack(alignment: .leading, spacing: 0) {
                    Text(title).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label).fixedSize(horizontal: false, vertical: true)
                    if let caption {
                        Text(caption).whooshText(WhooshTypography.subhead)
                            .foregroundStyle(captionStrong ? WhooshPalette.label : WhooshPalette.secondaryLabel)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                Spacer(minLength: 0)
            }
        } control: {
            control
        }
        .padding(.horizontal, WhooshSpacing.s14).padding(.vertical, WhooshSpacing.s8)
        .frame(minHeight: WhooshMetrics.listRow)
        .background(RoundedRectangle(cornerRadius: WhooshRadius.row, style: .continuous).fill(WhooshPalette.hover)
            .opacity(hover && onTap != nil ? 1 : 0).padding(.horizontal, WhooshSpacing.s4).padding(.vertical, WhooshSpacing.s2 / 2))
        .contentShape(Rectangle())
        .onHover { h in withAnimation(WhooshMotion.animation(WhooshMotion.hover, reduceMotion: reduceMotion)) { hover = h } }
        .onTapGesture { onTap?() }
    }
}

/// The heading above a card: the section's name at the left, an optional grey note at the right.
public struct FormSectionHeader: View {
    let title: String?
    let note: String?
    public init(title: String?, note: String? = nil) { self.title = title; self.note = note }

    public var body: some View {
        if title != nil || note != nil {
            HStack(spacing: WhooshSpacing.s6) {
                if let title { Text(title).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label) }
                Spacer(minLength: WhooshSpacing.s8)
                if let note { Text(note).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).multilineTextAlignment(.trailing) }
            }
            .padding(.horizontal, WhooshSpacing.s4).padding(.bottom, WhooshSpacing.s6)
            .accessibilityAddTraits(.isHeader)
        }
    }
}

/// A section: its heading, then its card.
public struct FormSection<Content: View>: View {
    let title: String?
    let note: String?
    @ViewBuilder let content: Content
    public init(title: String? = nil, note: String? = nil, @ViewBuilder content: () -> Content) {
        self.title = title; self.note = note; self.content = content()
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            FormSectionHeader(title: title, note: note)
            GroupedCard { content }
        }
        .padding(.top, WhooshSpacing.s16)
    }
}

// MARK: adaptive split (a label column and a control column that stack when the row is narrow)

/// What the last layout of each row decided: the label column's width and whether the control went under it. A check reads it
/// (`rows mac-settings-window.rowfit`) to prove no label is narrower than its longest word.
@MainActor
public final class FormFitLog {
    public static let shared = FormFitLog()
    public struct Entry { public var title: String; public var caption: String?; public var labelWidth: CGFloat; public var stacked: Bool }
    public private(set) var entries: [String: Entry] = [:]
    func record(_ id: String, _ e: Entry) { entries[id] = e }
    /// Forgets what the panes laid out before: the next pane records afresh.
    func reset() { entries.removeAll() }
}

/// Two columns that become two rows. The label column always gets at least `labelMin`: when the control, at its own width, would leave the
/// label less than that, the control moves under the label (right-aligned) and the label takes the whole width. A word is never cut for lack of room.
public struct FormSplitLayout: Layout {
    let labelMin: CGFloat
    let spacing: CGFloat
    let stackGap: CGFloat
    /// Reports the width given to the label column and whether the row stacked, on each placement.
    let report: (CGFloat, Bool) -> Void

    private func stacks(_ width: CGFloat, _ control: CGSize) -> Bool {
        control.width > 0 && width - control.width - spacing < labelMin
    }

    public func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let control = subviews[1].sizeThatFits(.unspecified)
        guard let w = proposal.width else {
            let label = subviews[0].sizeThatFits(.unspecified)
            return CGSize(width: label.width + (control.width > 0 ? spacing + control.width : 0), height: max(label.height, control.height))
        }
        if stacks(w, control) {
            let label = subviews[0].sizeThatFits(ProposedViewSize(width: w, height: nil))
            let c = subviews[1].sizeThatFits(ProposedViewSize(width: w, height: nil))
            return CGSize(width: w, height: label.height + stackGap + c.height)
        }
        let label = subviews[0].sizeThatFits(ProposedViewSize(width: control.width > 0 ? w - control.width - spacing : w, height: nil))
        return CGSize(width: w, height: max(label.height, control.height))
    }

    public func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let control = subviews[1].sizeThatFits(.unspecified)
        if stacks(bounds.width, control) {
            let label = subviews[0].sizeThatFits(ProposedViewSize(width: bounds.width, height: nil))
            subviews[0].place(at: CGPoint(x: bounds.minX, y: bounds.minY), anchor: .topLeading, proposal: ProposedViewSize(width: bounds.width, height: label.height))
            let c = subviews[1].sizeThatFits(ProposedViewSize(width: bounds.width, height: nil))
            subviews[1].place(at: CGPoint(x: bounds.maxX, y: bounds.minY + label.height + stackGap), anchor: .topTrailing,
                              proposal: ProposedViewSize(width: min(c.width, bounds.width), height: c.height))
            report(bounds.width, true)
        } else {
            let labelWidth = control.width > 0 ? bounds.width - control.width - spacing : bounds.width
            let label = subviews[0].sizeThatFits(ProposedViewSize(width: labelWidth, height: nil))
            subviews[0].place(at: CGPoint(x: bounds.minX, y: bounds.midY), anchor: .leading, proposal: ProposedViewSize(width: labelWidth, height: label.height))
            subviews[1].place(at: CGPoint(x: bounds.maxX, y: bounds.midY), anchor: .trailing, proposal: ProposedViewSize(width: control.width, height: control.height))
            report(labelWidth, false)
        }
    }
}

/// A label column and a control column that stack when the row is narrow (see FormSplitLayout).
public struct FormSplit<Label: View, Control: View>: View {
    let labelMin: CGFloat
    let probeId: String?
    let title: String
    let caption: String?
    @ViewBuilder let label: Label
    @ViewBuilder let control: Control

    public init(labelMin: CGFloat, probeId: String? = nil, title: String = "", caption: String? = nil,
                @ViewBuilder label: () -> Label, @ViewBuilder control: () -> Control) {
        self.labelMin = labelMin; self.probeId = probeId; self.title = title; self.caption = caption
        self.label = label(); self.control = control()
    }

    public var body: some View {
        let id = probeId, t = title, c = caption
        FormSplitLayout(labelMin: labelMin, spacing: WhooshSpacing.s12, stackGap: WhooshSpacing.s8, report: { w, stacked in
            guard let id else { return }
            MainActor.assumeIsolated { FormFitLog.shared.record(id, .init(title: t, caption: c, labelWidth: w, stacked: stacked)) }
        }) {
            // each column is one view whatever it holds: an empty control must not drop out of the layout's subviews
            HStack(spacing: 0) { label }
            HStack(spacing: 0) { control }
        }
    }
}

