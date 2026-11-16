import SwiftUI

/// `.sbh` a section header: 11 / 600, secondary, 24 tall; optional trailing words and control. A section header also carries the
/// fold chevron (round 6): hidden until the pointer is over the header, always shown while the section is folded.
struct SidebarHeader<Trailing: View>: View {
    let title: String
    /// A measurable id on the 24 pt header box itself (not its top pad), so a check reads the header's own centre.
    var ax: String? = nil
    /// The section this header folds ("mailboxes", "smart-folders", "labels"); nil draws no chevron.
    var section: String? = nil
    var collapsed = false
    var onCollapse: () -> Void = {}
    /// A "+" at the right that shows while the pointer is over the header (or `forceHover`); nil draws none.
    var onAdd: (() -> Void)? = nil
    var forceHover = false
    @ViewBuilder var trailing: () -> Trailing
    @State private var hovering = false
    var body: some View {
        HStack(spacing: WhooshSpacing.s4) {
            Text(Words.text(title)).whooshText(WhooshTypography.caption, emphasized: true)
                .lineLimit(1).fixedSize(horizontal: true, vertical: false).layoutPriority(1)   // the title is a word: the picker and the fold chevron give way before it is cut (960: "Boîtes aux le…")
            if let section {
                Button(action: onCollapse) {
                    Image(systemName: "chevron.right").font(.system(size: WhooshSpacing.s8, weight: .bold))
                        .rotationEffect(.degrees(collapsed ? 0 : 90))
                        .frame(minWidth: 0, maxWidth: WhooshMetrics.sidebarRow - 4, minHeight: WhooshMetrics.sidebarRow - 4, maxHeight: WhooshMetrics.sidebarRow - 4)   // compressible: it shrinks before the title is cut
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .opacity(hovering || forceHover || collapsed ? 1 : 0)
                .help(Words.text(collapsed ? "Show %@" : "Hide %@", Words.text(title)))
                .accessibilityIdentifier("mac-main.sidebar.section.\(section).chevron")
            }
            Spacer(minLength: WhooshSpacing.s4)
            trailing()
            if let onAdd {
                Button(action: onAdd) {
                    Image(systemName: "plus").font(.system(size: WhooshSpacing.s10, weight: .semibold)).frame(width: WhooshMetrics.sidebarRow - 4, height: WhooshMetrics.sidebarRow - 4)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .opacity(hovering || forceHover ? 1 : 0)
                .help(Words.text(title == "Labels" ? "New label" : "New smart folder"))
                .accessibilityIdentifier("mac-main.sidebar.\(title.lowercased()).add")
            }
        }
        .foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
        .frame(height: WhooshMetrics.sidebarRow)
        .contentShape(Rectangle())
        .onHover { h in withAnimation(WhooshMotion.hover) { hovering = h } }
        .modifier(HeaderAx(id: ax))
        .padding(.horizontal, WhooshSpacing.s8)
        .padding(.top, WhooshSpacing.s8)
    }
}

private struct HeaderAx: ViewModifier {
    let id: String?
    @ViewBuilder func body(content: Content) -> some View {
        if let id { content.whooshAx(id) } else { content }
    }
}

/// `.sbr` a sidebar row: 15 pt accent glyph, 13 pt label, trailing count; hover and selection tint the 7 pt rounded row.
struct SidebarRow: View {
    let symbol: String
    let title: String
    var count: Int? = nil
    var indent: CGFloat = 0
    var chevron: Bool? = nil
    /// A chevron drawn in the row's left gutter (All inboxes), so the row's text keeps the column the other rows use.
    var gutterChevron: Bool? = nil
    var onGutter: () -> Void = {}
    var selected = false
    /// The sidebar's selection namespace and fade epoch (SidebarSelection.swift); nil draws the highlight in place.
    var selectionNS: Namespace.ID? = nil
    var selectionEpoch = 0
    /// A label's pin: nil draws none; true draws the filled pin (click unpins); false draws it on hover (click pins).
    var pinned: Bool? = nil
    var onPin: () -> Void = {}
    var pinAx: String? = nil
    var forceHover = false
    /// A label that belongs to one account carries that account's mark.
    var scopeAccount: Account? = nil
    var onChevron: () -> Void = {}
    let action: () -> Void
    @State private var hovering = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: WhooshSpacing.s8) {
                if let open = chevron {
                    Image(systemName: open ? "chevron.down" : "chevron.right")
                        .font(.system(size: WhooshSpacing.s8, weight: .bold))
                        .foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
                        .frame(width: WhooshSpacing.s10)
                        .contentShape(Rectangle())
                        .onTapGesture(perform: onChevron)
                }
                Image(systemName: symbol).font(.system(size: WhooshMetrics.actionIcon - 3)).foregroundStyle(WhooshPalette.accent)
                    .frame(width: WhooshMetrics.actionIcon)
                Text(title).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label).lineLimit(1).layoutPriority(1)   // the label is a word: the pin and the count give way first
                Spacer(minLength: WhooshSpacing.s4)
                if let a = scopeAccount {
                    AccountMark(account: a, size: .compact, needsAttention: false, hasUnread: false).help(Words.text("Only in %@", a.displayName))
                }
                if let pinned, pinned || hovering || forceHover {
                    Image(systemName: pinned ? "pin.fill" : "pin").font(.system(size: WhooshSpacing.s10, weight: .semibold))
                        .foregroundStyle(pinned ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(WhooshPalette.sidebarSecondaryLabel))
                        .frame(minWidth: 0, maxWidth: WhooshSpacing.s16, minHeight: WhooshMetrics.sidebarRow, maxHeight: WhooshMetrics.sidebarRow)
                        .clipped().layoutPriority(-1)
                        .contentShape(Rectangle())
                        .onTapGesture(perform: onPin)
                        .help(Words.text(pinned ? "Unpin tab" : "Pin as a tab"))
                        .modifier(OptionalAx(id: pinAx))
                }
                if let count, count > 0 { SidebarCount(value: count).foregroundStyle(WhooshPalette.sidebarSecondaryLabel) }
            }
            .padding(.leading, WhooshSpacing.s8 + indent)
            .padding(.trailing, WhooshSpacing.s8)
            .frame(height: WhooshMetrics.sidebarRow)
            .background {
                ZStack {
                    if hovering && !selected { RoundedRectangle(cornerRadius: WhooshRadius.control).fill(WhooshPalette.hover) }
                    if selected, let ns = selectionNS { SidebarSelectionHighlight(ns: ns, epoch: selectionEpoch) }
                    else if selected { RoundedRectangle(cornerRadius: WhooshRadius.control).fill(WhooshPalette.hover) }
                }
            }
            .overlay(alignment: .leading) {
                if let open = gutterChevron {
                    Image(systemName: open ? "chevron.down" : "chevron.right").font(.system(size: WhooshSpacing.s8 - 1, weight: .bold))
                        .foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
                        .frame(width: WhooshSpacing.s10, height: WhooshMetrics.sidebarRow)
                        .contentShape(Rectangle())
                        .onTapGesture(perform: onGutter)
                        .help(Words.text(open ? "Hide the accounts" : "Show one row per account"))
                        .whooshAx("mac-main.sidebar.all-inboxes.chevron")
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(SidebarPressStyle())
        .onHover { h in withAnimation(WhooshMotion.hover) { hovering = h } }
    }
}

private struct OptionalAx: ViewModifier {
    let id: String?
    @ViewBuilder func body(content: Content) -> some View {
        if let id { content.whooshAx(id) } else { content }
    }
}
