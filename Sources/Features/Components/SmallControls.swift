import SwiftUI

/// The measurable id of a component on one surface (component-grammar seam): the id plus the radius the component draws, so `measure` serves frame and radius.
/// Without an id the component is drawn as before.
struct ComponentAx: ViewModifier {
    let id: String?
    let radius: CGFloat
    func body(content: Content) -> some View {
        if let id { content.whooshAx(id, radius: radius) } else { content }
    }
}

/// `.kc` a key cap: 20 pt minimum, 5 pt radius, the key fill with an inset bottom edge, 11 pt semibold.
public struct KeyCap: View {
    let key: String
    var onSelection = false
    var ax: String?
    public init(_ key: String, onSelection: Bool = false) { self.key = key; self.onSelection = onSelection }
    /// The cap's measurable id (frame and radius for `measure`).
    public func axId(_ id: String?) -> KeyCap { var c = self; c.ax = id; return c }
    /// Apple's own key glyphs for the named keys (as the menu bar draws them); a letter or a chord passes through.
    static let glyphs: [String: String] = ["Enter": "\u{21A9}", "Return": "\u{21A9}", "Space": "\u{2423}", "Escape": "\u{238B}", "Esc": "\u{238B}", "Tab": "\u{21E5}", "Delete": "\u{232B}", "Backspace": "\u{232B}", "Up": "\u{2191}", "Down": "\u{2193}", "Left": "\u{2190}", "Right": "\u{2192}"]
    public static func glyph(for key: String) -> String {
        if let g = glyphs[key] { return g }
        // a chord such as "⌘Enter" or "⇧Space": swap its named tail
        for (name, g) in glyphs where key.hasSuffix(name) && key.count > name.count { return String(key.dropLast(name.count)) + g }
        return key
    }
    public var body: some View {
        Text(KeyCap.glyph(for: key))
            .whooshText(WhooshTypography.caption, emphasized: true)
            .foregroundStyle(onSelection ? WhooshPalette.onAccent : WhooshPalette.label)
            .padding(.horizontal, WhooshSpacing.s4 + 1)
            .frame(minWidth: WhooshMetrics.keyCap, minHeight: WhooshMetrics.keyCap)
            .background {
                RoundedRectangle(cornerRadius: WhooshRadius.keyCap, style: .continuous)
                    .fill(onSelection ? AnyShapeStyle(WhooshPalette.onAccent.opacity(0.2)) : AnyShapeStyle(WhooshPalette.keyCap))
                    .overlay(alignment: .bottom) { Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator) }
                    .clipShape(RoundedRectangle(cornerRadius: WhooshRadius.keyCap, style: .continuous))
            }
            .modifier(ComponentAx(id: ax, radius: WhooshRadius.keyCap))
            .accessibilityHidden(true)
    }
}

/// A key and what it does: `[ Z ] Undo`. Used by the trainer hint, the undo bar, empty states and the palette.
public struct KeyHint: View {
    let keys: [String]
    let label: String
    var capAx: String?
    public init(_ keys: [String], _ label: String) { self.keys = keys; self.label = label }
    /// The measurable id of the hint's first key cap (the cap is the 20 pt, 5 pt radius piece; the hint around it also holds its words).
    public func capAx(_ id: String?) -> KeyHint { var c = self; c.capAx = id; return c }
    public init(_ key: String, _ label: String) { self.init([key], label) }
    public var body: some View {
        HStack(spacing: WhooshSpacing.s6) {
            HStack(spacing: WhooshSpacing.s2) { ForEach(Array(keys.enumerated()), id: \.offset) { i, k in KeyCap(k).axId(i == 0 ? capAx : nil) } }
            Text(label).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.label)
        }
        .padding(.horizontal, WhooshSpacing.s6).padding(.vertical, WhooshSpacing.s2)
        .hoverAck(radius: WhooshRadius.control)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("\(label), key \(keys.joined(separator: " "))")
    }
}

/// A label chip on a list row or in the reader: 20 pt tall, 6 pt radius, quiet fill, caption text.
public struct LabelChip: View {
    let name: String
    var ax: String?
    public init(_ name: String) { self.name = name }
    /// The chip's measurable id (frame and radius for `measure`).
    public func axId(_ id: String?) -> LabelChip { var c = self; c.ax = id; return c }
    public var body: some View {
        Text(name)
            .whooshText(WhooshTypography.caption)
            .foregroundStyle(WhooshPalette.secondaryLabel)
            .lineLimit(1)
            .padding(.horizontal, WhooshSpacing.s6)
            .frame(height: WhooshMetrics.keyCap)
            .background(WhooshPalette.fill, in: RoundedRectangle(cornerRadius: WhooshRadius.item, style: .continuous))
            .hoverAck(radius: WhooshRadius.item)
            .modifier(ComponentAx(id: ax, radius: WhooshRadius.item))
    }
}

/// The verification-code pill: the code in the mono face, one click copies it. Same 20 pt height and radius as a chip.
public struct CodePill: View {
    let code: String
    var onCopy: (String) -> Void = { _ in }
    @State private var copied = false
    public init(_ code: String, onCopy: @escaping (String) -> Void = { _ in }) { self.code = code; self.onCopy = onCopy }
    public var body: some View {
        Button {
            onCopy(code)
            copied = true
        } label: {
            HStack(spacing: WhooshSpacing.s4) {
                Image(systemName: copied ? "checkmark" : "doc.on.doc").font(.system(size: 9, weight: .semibold))
                Text(code).font(.system(size: WhooshTypography.caption.size, weight: .semibold, design: .monospaced))
            }
            .foregroundStyle(copied ? WhooshPalette.accent : WhooshPalette.label)
            .padding(.horizontal, WhooshSpacing.s6)
            .frame(height: WhooshMetrics.keyCap)
            .background(WhooshPalette.fill, in: RoundedRectangle(cornerRadius: WhooshRadius.item, style: .continuous))
            .hoverAck(radius: WhooshRadius.item)
        }
        .buttonStyle(.plain)
        .help(copied ? "Copied" : "Copy the code")
        .accessibilityLabel("Code \(code)")
        .accessibilityHint("Copies the code")
    }
}

/// The sync line: a dot and one sentence in the user's words. The warning dot is the only orange; the words carry the meaning.
public struct StatusLine: View {
    public enum State: Equatable, Hashable, Sendable {
        case synced(String)
        case syncing(String)
        case attention(String, action: String?)
    }
    let state: State
    var onAction: () -> Void = {}
    public init(_ state: State, onAction: @escaping () -> Void = {}) { self.state = state; self.onAction = onAction }
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    public var body: some View {
        // A state change cross-fades (opacity only); the old line leaves as the new one arrives.
        ZStack(alignment: .leading) { line.id(state).transition(.opacity) }
            .animation(WhooshMotion.animation(WhooshMotion.readerSwap, reduceMotion: false), value: state)
    }
    private var line: some View {
        HStack(spacing: WhooshSpacing.s6) {
            dot
            Text(text).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
            if case .attention(_, let action?) = state {
                Button(action: onAction) { Text(action).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.accent) }
                    .buttonStyle(.plain)
            }
        }
        .accessibilityElement(children: .combine)
    }
    private var text: String {
        switch state { case .synced(let t), .syncing(let t), .attention(let t, _): return t }
    }
    @ViewBuilder private var dot: some View {
        switch state {
        case .synced: Circle().fill(WhooshPalette.tertiaryLabel).frame(width: WhooshMetrics.warningDot, height: WhooshMetrics.warningDot)
        case .syncing: ProgressView().controlSize(.mini).scaleEffect(0.7).frame(width: WhooshMetrics.warningDot + 2, height: WhooshMetrics.warningDot + 2)
        case .attention: Circle().fill(WhooshPalette.warning).frame(width: WhooshMetrics.warningDot, height: WhooshMetrics.warningDot)
        }
    }
}

/// Nothing here yet: a 44 pt quiet glyph, a 17 pt title, one 12 pt line (300 pt at most), an optional key in the accent.
public struct EmptyState: View {
    let symbol: String
    let title: String
    let line: String
    var key: (String, String)?
    public init(symbol: String, title: String, line: String, key: (String, String)? = nil) {
        self.symbol = symbol; self.title = title; self.line = line; self.key = key
    }
    public var body: some View {
        VStack(spacing: 0) {
            Image(systemName: symbol).font(.system(size: 36, weight: .light)).foregroundStyle(WhooshPalette.tertiaryLabel)
                .frame(width: 44, height: 44).padding(.bottom, WhooshSpacing.s14).accessibilityHidden(true)
            Text(title).whooshText(WhooshTypography.title3).foregroundStyle(WhooshPalette.label)
            Text(line).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                .multilineTextAlignment(.center).frame(maxWidth: 300).padding(.top, WhooshSpacing.s6)
            if let key {
                HStack(spacing: WhooshSpacing.s6) {
                    KeyCap(key.0)
                    Text(key.1).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.accent)
                }.padding(.top, WhooshSpacing.s14)
            }
        }
        .padding(.horizontal, WhooshSpacing.s32)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .fadeIn()
        .accessibilityElement(children: .combine)
    }
}
