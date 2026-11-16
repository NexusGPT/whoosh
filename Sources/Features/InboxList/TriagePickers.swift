import SwiftUI
import Observation
#if os(macOS)
import AppKit
#endif

/// H's and L's pickers over the list. Lead, 13 Nov (pictures of a hidden walk on main151): both keys opened a picker that drew NOTHING. The app held the
/// pending action and took the next words blind, and with real keys those letters ran the list's own commands. The typed words live in
/// `AppModel.typed` (the socket's `type` writes them there; the field here shows and edits them); this holds the highlighted line.
@MainActor @Observable
final class TriagePickState {
    static let shared = TriagePickState()
    var highlight = 0
    /// Snooze only: "Pick a date…" was taken, the calendar step shows.
    var pickingDate = false
    func reset() { highlight = 0; pickingDate = false }
}

/// What the two pickers list, and the line Enter takes. One place, so the screen, the keys and the socket's doors agree.
@MainActor
enum TriagePick {
    static var state: TriagePickState { .shared }

    static func snoozeOptions(now: Date = Date()) -> [SnoozeOption] { TimeInWords.options(now: now) }

    /// The snooze line the typed words name (its English or its translated title, either way round as a prefix); nil when they name none.
    static func snoozeMatch(_ words: String, _ options: [SnoozeOption]) -> Int? {
        let w = words.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !w.isEmpty else { return nil }
        return options.firstIndex { o in
            [o.title, Words.text(o.title)].contains { title in
                let t = title.lowercased()
                return t.hasPrefix(w) || w.hasPrefix(t)
            }
        }
    }

    /// The snooze line Enter takes: the one the typed words name; with nothing typed, the highlighted one.
    static func snoozeIndex(_ m: AppModel, _ options: [SnoozeOption]) -> Int? {
        let w = m.typed.trimmingCharacters(in: .whitespacesAndNewlines)
        if !w.isEmpty { return snoozeMatch(w, options) }
        return options.indices.contains(state.highlight) ? state.highlight : nil
    }

    static func when(_ kind: SnoozeOption.Kind) -> SnoozeWhen? {
        switch kind {
        case .laterToday: return .laterToday
        case .tomorrowMorning: return .tomorrowMorning
        case .nextWeek: return .nextWeek
        case .pickDate: return nil
        }
    }

    /// The owner's labels the typed words narrow, each with the path above it ("Work > Clients"): a name that is the words first, then one
    /// that starts with them, then one that holds them, then one whose path holds them. With nothing typed, every label.
    static func labels(_ m: AppModel) -> [(label: MailLabel, path: String)] {
        let all = m.core.labels()
        func path(_ l: MailLabel) -> String {
            var names: [String] = [], cur = l.parentId
            while let p = cur, let up = all.first(where: { $0.id == p }), names.count < WhooshConstants.LABEL_DEPTH_MAX { names.insert(up.name, at: 0); cur = up.parentId }
            return names.joined(separator: " > ")
        }
        let rows = all.filter { $0.kind == .owner }.map { (label: $0, path: path($0)) }
        let q = m.typed.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !q.isEmpty else { return rows }
        var ranked: [(rank: Int, at: Int, row: (label: MailLabel, path: String))] = []
        for (i, r) in rows.enumerated() {
            let n = r.label.name.lowercased()
            let rank: Int? = n == q ? 0 : (n.hasPrefix(q) ? 1 : (n.contains(q) ? 2 : (r.path.lowercased().contains(q) ? 3 : nil)))
            if let rank { ranked.append((rank, i, r)) }
        }
        ranked.sort { ($0.rank, $0.at) < ($1.rank, $1.at) }
        return ranked.map { $0.row }
    }

    /// ↑ ↓ in an open picker. A snooze line chosen by an arrow drops the typed words (they would name another line).
    @discardableResult
    static func move(_ delta: Int, _ m: AppModel) -> Bool {
        switch InboxListState.shared.pending {
        case .snooze:
            let options = snoozeOptions()
            let from = snoozeIndex(m, options) ?? (delta > 0 ? -1 : options.count)
            m.typed = ""
            state.highlight = min(max(from + delta, 0), options.count - 1)
            return true
        case .label:
            let n = labels(m).count
            guard n > 0 else { return true }
            state.highlight = min(max(state.highlight + delta, 0), n - 1)
            return true
        case nil:
            return false
        }
    }

    /// A click on a snooze line (the row's own click, and the socket's door).
    static func clickSnooze(_ kind: String, _ m: AppModel) -> Bool {
        guard case .snooze? = InboxListState.shared.pending, let i = snoozeOptions().firstIndex(where: { $0.kind.rawValue == kind }) else { return false }
        m.typed = ""
        state.highlight = i
        return TriageActions.enter(m)
    }

    /// A click on a label line.
    static func clickLabel(_ id: String, _ m: AppModel) -> Bool {
        guard case .label? = InboxListState.shared.pending, let i = labels(m).firstIndex(where: { $0.label.id == id }) else { return false }
        state.highlight = i
        return TriageActions.enter(m)
    }

    /// What the socket reads of the open picker (`rows mac-main.picker`): its kind, its lines as drawn, the highlighted one, the typed words.
    static func rows(_ m: AppModel) -> [ControlRow] {
        switch TriageActions.live(m) {
        case .snooze:
            let options = snoozeOptions()
            return [["kind": "snooze", "ax": "mac-main.snooze.picker", "lines": options.map { Words.text($0.title) }, "highlight": snoozeIndex(m, options) ?? -1,
                     "typed": m.typed, "picking_date": state.pickingDate, "key": LiveKeys.chord("snooze", "H")]]
        case .label:
            let lines = labels(m)
            return [["kind": "label", "ax": "mac-main.label.picker", "lines": lines.map { $0.label.name }, "highlight": lines.isEmpty ? -1 : min(state.highlight, lines.count - 1),
                     "typed": m.typed, "picking_date": false, "key": LiveKeys.chord("label", "L")]]
        case nil:
            return []
        }
    }
}

/// The field both pickers start with: an icon, the typed words, the key that opened the picker. It holds the keyboard while the picker is up, so
/// letters are words here and never the list's keys.
private struct TriagePickField: View {
    let symbol: String
    let placeholder: String
    let key: String
    @FocusState private var focused: Bool
    private let app = AppModel.shared

    var body: some View {
        let words = Binding(get: { app.typed }, set: { app.typed = $0 })
        HStack(spacing: WhooshSpacing.s6) {
            Image(systemName: symbol).font(.system(size: 11)).foregroundStyle(WhooshPalette.secondaryLabel)
            TextField(placeholder, text: words)
                .textFieldStyle(.plain).whooshText(WhooshTypography.body).focused($focused)
                .onSubmit { _ = TriageActions.enter(app) }
                .onKeyPress(.downArrow) { TriagePick.move(1, app); return .handled }
                .onKeyPress(.upArrow) { TriagePick.move(-1, app); return .handled }
                .onKeyPress(.escape) { _ = InboxListFeature.keyBindings()["Escape"]?(app); return .handled }
            KeyCap(key)
        }
        .padding(.horizontal, WhooshSpacing.s10).frame(height: WhooshMetrics.segmentHeight + WhooshSpacing.s2)
        .background(WhooshPalette.field, in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
        .overlay { RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator) }
        .padding(.bottom, WhooshSpacing.s4)
        // the field takes the keyboard as it appears, and again one turn later: on the first picker of a run the first ask came before the view was in
        // its window and no text held the keys (walk on main153); the key monitor's hook (TriagePick.installOnce) is the net under both
        // with words already typed (keys that came before the field was in its window) the field is left to the key monitor's hook, which
        // puts the cursor after them: a field that takes the keyboard by itself selects its words, and the next letter would replace them
        .onAppear { if app.typed.isEmpty { focused = true } }
        .task { if app.typed.isEmpty { focused = true } }
    }
}

/// H's picker: "Snooze until", the three times in words with their clock, "Pick a date…" (the calendar step of the designed time picker).
/// ↑ ↓ choose, ↩ snoozes, Esc closes; typed words name a line ("tomorrow").
struct SnoozePickerOverlay: View {
    @State private var state = TriagePickState.shared  // local-ui: the picker's highlight and its date step, observed
    private let app = AppModel.shared
    private let now = Date()

    var body: some View {
        if state.pickingDate {
            // the calendar opens on tomorrow morning, a time that can be taken as it stands
            TimePickerInWords(now: now, openOnDate: true, start: TriagePick.snoozeOptions(now: now).first { $0.kind == .tomorrowMorning }?.when) { date in
                let ms = Int64(date.timeIntervalSince1970 * 1000)
                _ = TriageActions.finishSnooze(app, when: .at(atMs: ms), words: ReaderWords.when(ms))
            }
            .whooshAx("mac-main.snooze.picker")
        } else {
            let options = TriagePick.snoozeOptions(now: now)
            let on = TriagePick.snoozeIndex(app, options)
            PickerPanel(width: WhooshSpacing.s32 * 9) {
                TriagePickField(symbol: "clock", placeholder: Words.text("Snooze until"), key: LiveKeys.chord("snooze", "H"))
                ForEach(Array(options.enumerated()), id: \.element.id) { i, o in
                    Button { _ = TriagePick.clickSnooze(o.kind.rawValue, app) } label: {
                        PickerRow(title: Words.text(o.title), highlighted: on == i && o.refusal == nil, dimmed: o.refusal != nil) {
                            if o.refusal != nil {
                                EmptyView()      // a line that cannot be taken shows no time (late in the evening "Later today" read "Sat 01:30")
                            } else if let when = o.when {
                                Text(ReaderWords.when(Int64(when.timeIntervalSince1970 * 1000), now: now)).whooshText(WhooshTypography.caption)
                                    .foregroundStyle(on == i && o.refusal == nil ? WhooshPalette.onAccent : WhooshPalette.secondaryLabel)
                            } else {
                                Image(systemName: "calendar").font(.system(size: WhooshMetrics.actionIcon - 3))
                                    .foregroundStyle(on == i ? WhooshPalette.onAccent : WhooshPalette.secondaryLabel)
                            }
                        }
                    }
                    .buttonStyle(.plain)
                    .onHover { if $0, app.typed.isEmpty { state.highlight = i } }
                    .accessibilityIdentifier("mac-main.snooze.option.\(o.kind.rawValue)")
                }
                HStack(spacing: WhooshSpacing.s12) { KeyHint("↩", Words.text("Snooze")); KeyHint("Esc", Words.text("Close")) }
                    .padding(.horizontal, WhooshSpacing.s6).padding(.top, WhooshSpacing.s4)
            }
            .whooshAx("mac-main.snooze.picker")
        }
    }
}

/// L's picker: "Label as…", the owner's labels with their path, narrowed by the typed words. ↑ ↓ choose, ↩ labels, Esc closes.
struct LabelPickerOverlay: View {
    @State private var state = TriagePickState.shared  // local-ui: the picker's highlight, observed
    private let app = AppModel.shared

    var body: some View {
        let rows = TriagePick.labels(app)
        let on = rows.isEmpty ? -1 : min(state.highlight, rows.count - 1)
        let rowHeight = WhooshMetrics.sidebarRow + WhooshSpacing.s4 + WhooshSpacing.s2
        PickerPanel(width: WhooshSpacing.s32 * 9) {
            TriagePickField(symbol: "tag", placeholder: Words.text("Label as…"), key: LiveKeys.chord("label", "L"))
            if rows.isEmpty {
                Text(Words.text("No label has these words")).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                    .padding(.horizontal, WhooshSpacing.s10).padding(.vertical, WhooshSpacing.s6)
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                        ForEach(Array(rows.enumerated()), id: \.element.label.id) { i, r in
                            Button { _ = TriagePick.clickLabel(r.label.id, app) } label: {
                                PickerRow(title: r.label.name, highlighted: i == on) {
                                    Text(r.path).whooshText(WhooshTypography.caption).lineLimit(1)
                                        .foregroundStyle(i == on ? WhooshPalette.onAccent : WhooshPalette.secondaryLabel)
                                }
                            }
                            .buttonStyle(.plain)
                            .onHover { if $0 { state.highlight = i } }
                            .accessibilityIdentifier("mac-main.label.option.\(r.label.id)")
                        }
                    }
                }
                .scrollIndicators(.automatic)
                .frame(height: CGFloat(min(rows.count, 7)) * rowHeight)
            }
            HStack(spacing: WhooshSpacing.s12) { KeyHint("↩", Words.text("Label")); KeyHint("Esc", Words.text("Close")) }
                .padding(.horizontal, WhooshSpacing.s6).padding(.top, WhooshSpacing.s4)
        }
        .whooshAx("mac-main.label.picker")
        .onChange(of: app.typed) { _, _ in state.highlight = 0 }
    }
}

#if os(macOS)
extension TriagePick {
    /// The field of the open picker in a window (the editable field whose placeholder is a picker's); nil when none is mounted there.
    static func field(in w: NSWindow, placeholders: Set<String>? = nil) -> NSTextField? {
        let names: Set<String> = placeholders ?? [Words.text("Snooze until"), Words.text("Label as…")]
        func find(_ v: NSView) -> NSTextField? {
            if let t = v as? NSTextField, t.isEditable, let p = t.placeholderString, names.contains(p) { return t }
            for s in v.subviews { if let t = find(s) { return t } }
            return nil
        }
        return w.contentView.flatMap(find)
    }

    /// The key monitor's hook: while a picker is up, a typed key that finds no text holding the keyboard is the picker's. It goes to the picker's
    /// field (the cursor after the words it holds); while the field is not in its window yet (the first picker of a run on a loaded machine: three
    /// letters ran as list commands in a probe on main154, starring the mail and marking it read) the picker takes the key itself.
    static func installOnce() {
        guard !installed else { return }
        installed = true
        ChordKit.typingOverlays.formUnion([Overlay.snooze, Overlay.label])
        ChordKit.pickerTyping.append { e in
            guard TriageActions.live(AppModel.shared) != nil else { ChordKit.note("triage-not-up"); return 0 }
            guard !TriagePickState.shared.pickingDate else { ChordKit.note("triage-date-step"); return 0 }
            if TriagePick.focusField(e, placeholders: nil) { ChordKit.note("triage-field"); return 1 }
            let took = TriagePick.takeKey(e)
            ChordKit.note(took ? "triage-took" : "triage-refused")
            return took ? 2 : 0
        }
    }
    private static var installed = false

    /// Makes a picker's field the first responder of the event's window, the cursor after the words it holds. False while no such field is there.
    static func focusField(_ e: NSEvent, placeholders: Set<String>?) -> Bool {
        guard let w = e.window ?? NSApp.keyWindow else { ChordKit.note("no-window"); return false }
        guard let f = field(in: w, placeholders: placeholders) else { ChordKit.note("no-field"); return false }
        guard w.makeFirstResponder(f) else { ChordKit.note("field-refused"); return false }
        if let editor = w.firstResponder as? NSText { editor.selectedRange = NSRange(location: (editor.string as NSString).length, length: 0) }
        return true
    }

    /// A key the picker takes itself: ↑ ↓ move its line, Return takes it, Backspace drops the last typed letter, a character joins the typed words.
    static func takeKey(_ e: NSEvent) -> Bool {
        let m = AppModel.shared
        switch ChordKit.chord(e) {
        case "Down": return move(1, m)
        case "Up": return move(-1, m)
        case "Enter": _ = TriageActions.enter(m); return true
        case "Backspace", "\u{7F}": if !m.typed.isEmpty { m.typed.removeLast() }; return true
        case "Left", "Right", "Tab", "Delete": return true
        default:
            guard let s = e.characters, !s.isEmpty, s.unicodeScalars.allSatisfy({ !CharacterSet.controlCharacters.contains($0) && !(0xF700...0xF8FF).contains($0.value) }) else { return true }
            m.typed += s
            return true
        }
    }
}
#endif

/// The list's own two pickers on its overlay mount (ListOverlayMount.swift), beside the labels lane's Move picker.
extension InboxListFeature: ListOverlayProviding {
    public static func listOverlay(_ id: String, model: AppModel) -> AnyView? {
        if id == Overlay.snooze { return AnyView(SnoozePickerOverlay()) }
        if id == Overlay.label { return AnyView(LabelPickerOverlay()) }
        return nil
    }
}
