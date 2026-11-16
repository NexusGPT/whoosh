import SwiftUI
import Observation
#if os(macOS)
import AppKit
#endif

/// What V's picker holds: the words typed to narrow it and the highlighted label. Reset each time V opens it.
@MainActor @Observable
final class LabelMoveState {
    static let shared = LabelMoveState()
    var query = ""
    var highlight = 0

    /// Owner labels whose name or path holds the typed words, each with the path above it ("Work > Clients").
    func targets(_ m: AppModel) -> [(label: MailLabel, path: String)] {
        let all = m.core.labels()
        func path(_ l: MailLabel) -> String {
            var names: [String] = [], cur = l.parentId
            while let p = cur, let up = all.first(where: { $0.id == p }), names.count < WhooshConstants.LABEL_DEPTH_MAX { names.insert(up.name, at: 0); cur = up.parentId }
            return names.joined(separator: " > ")
        }
        let q = query.trimmingCharacters(in: .whitespaces).lowercased()
        return LabelDoors.moveTargets(m).map { ($0, path($0)) }
            .filter { q.isEmpty || $0.label.name.lowercased().contains(q) || $0.path.lowercased().contains(q) }
    }

    func reset() { query = ""; highlight = 0 }
}

/// V's picker: "Move to", a field that narrows the labels, then the owner's labels with their path. ↑ ↓ choose, ↩ moves the
/// thread there (labelled and archived, one undo), Esc closes. The list's overlay host mounts it while the overlay is "move".
struct LabelMovePicker: View {
    @State private var state = LabelMoveState.shared  // local-ui: the picker's own query and highlight, observed
    @FocusState private var fieldFocused: Bool
    private let app = AppModel.shared

    var body: some View {
        let rows = state.targets(app)
        // The same parts as H's and L's pickers (InboxList/TriagePickers.swift): one field with its icon, its words and the key that opened it, then
        // the lines, as tall as they are. Lead, 13 Nov 2026 (picture on main156): this picker had a title line over a bare field and kept six lines
        // of height for one result.
        let rowHeight = WhooshMetrics.sidebarRow + WhooshSpacing.s4 + WhooshSpacing.s2
        PickerPanel(width: WhooshSpacing.s32 * 9) {
            HStack(spacing: WhooshSpacing.s6) {
                Image(systemName: "folder").font(.system(size: 11)).foregroundStyle(WhooshPalette.secondaryLabel)
                TextField(Words.text("Move to"), text: $state.query)
                    .textFieldStyle(.plain).whooshText(WhooshTypography.body).focused($fieldFocused)
                    .onSubmit { pick(rows) }
                    .onKeyPress(.downArrow) { state.highlight = min(state.highlight + 1, max(0, rows.count - 1)); return .handled }
                    .onKeyPress(.upArrow) { state.highlight = max(state.highlight - 1, 0); return .handled }
                    .onKeyPress(.escape) { app.overlay = nil; return .handled }
                    .onChange(of: state.query) { _, _ in state.highlight = 0 }
                KeyCap(LiveKeys.chord("move", "V"))
            }
            .padding(.horizontal, WhooshSpacing.s10).frame(height: WhooshMetrics.segmentHeight + WhooshSpacing.s2)
            .background(WhooshPalette.field, in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
            .overlay { RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator) }
            .padding(.bottom, WhooshSpacing.s4)
            if rows.isEmpty {
                Text(Words.text("No label has these words")).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                    .padding(.horizontal, WhooshSpacing.s10).padding(.vertical, WhooshSpacing.s6)
            } else {
            ScrollView {
            VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
            ForEach(Array(rows.enumerated()), id: \.element.label.id) { i, r in
                Button { _ = LabelDoors.move(app, to: r.label.id) } label: {
                    PickerRow(title: r.label.name, highlighted: i == state.highlight) {
                        Text(r.path).whooshText(WhooshTypography.caption).lineLimit(1)
                            .foregroundStyle(i == state.highlight ? WhooshPalette.onAccent : WhooshPalette.secondaryLabel)
                    }
                }
                .buttonStyle(.plain)
                .onHover { if $0 { state.highlight = i } }
                .help(Words.text("Move to %@", r.label.name) + " · ↩")
                .accessibilityIdentifier("mac-labels.move.\(r.label.id)")
            }
            }
            }
            .scrollIndicators(.automatic)
            .frame(height: CGFloat(min(rows.count, 7)) * rowHeight)
            }
            HStack(spacing: WhooshSpacing.s12) { KeyHint("↩", Words.text("Move")); KeyHint("Esc", Words.text("Close")) }
                .padding(.horizontal, WhooshSpacing.s6).padding(.top, WhooshSpacing.s4)
        }
        .whooshAx("mac-labels.move.picker")
        // the picker's state is reset where V opens it (the "move" command), not here: letters typed before this view was in its window are its words.
        // With such words the field is left to the key monitor's hook, which puts the cursor after them.
        .onAppear { if state.query.isEmpty { fieldFocused = true } }
        .task { if state.query.isEmpty { fieldFocused = true } }
    }

    private func pick(_ rows: [(label: MailLabel, path: String)]) {
        guard rows.indices.contains(state.highlight) else { return }
        _ = LabelDoors.move(app, to: rows[state.highlight].label.id)
    }
}

#if os(macOS)
extension LabelMoveState {
    private static var installed = false

    /// The key monitor's hook for V's picker, as the snooze and label pickers have (TriagePick.installOnce): while it is up a typed key is its own.
    static func installOnce() {
        guard !installed else { return }
        installed = true
        ChordKit.typingOverlays.insert(LabelDoors.moveOverlay)
        ChordKit.pickerTyping.append { e in
            let m = AppModel.shared, state = LabelMoveState.shared
            guard m.overlay == LabelDoors.moveOverlay else { ChordKit.note("move-not-up"); return 0 }
            if TriagePick.focusField(e, placeholders: [Words.text("Move to")]) { ChordKit.note("move-field"); return 1 }
            ChordKit.note("move-took")
            let rows = state.targets(m)
            switch ChordKit.chord(e) {
            case "Down": state.highlight = min(state.highlight + 1, max(0, rows.count - 1))
            case "Up": state.highlight = max(state.highlight - 1, 0)
            case "Enter": if rows.indices.contains(state.highlight) { _ = LabelDoors.move(m, to: rows[state.highlight].label.id) }
            case "Backspace", "\u{7F}": if !state.query.isEmpty { state.query.removeLast(); state.highlight = 0 }
            case "Left", "Right", "Tab", "Delete": break
            default:
                if let s = e.characters, !s.isEmpty, s.unicodeScalars.allSatisfy({ !CharacterSet.controlCharacters.contains($0) && !(0xF700...0xF8FF).contains($0.value) }) {
                    state.query += s; state.highlight = 0
                }
            }
            return 2
        }
    }
}
#endif

/// V's picker on the list's overlay mount (InboxList/ListOverlayMount.swift): the list draws it while the overlay is "move".
extension LabelsContactsMacFeature: ListOverlayProviding {
    public static func listOverlay(_ id: String, model: AppModel) -> AnyView? {
        id == LabelDoors.moveOverlay ? AnyView(LabelMovePicker()) : nil
    }
}
