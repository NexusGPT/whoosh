import SwiftUI
#if os(macOS)
import AppKit
#endif

/// The key hints the window's foot strip shows for the focused surface (owner 08:58, Superhuman: "You have the
/// shortcut at the bottom"; SPEC A78). Keys come from the live preset (MacHintTable.keyLabel), never literals, so a
/// preset switch changes the strip; words come from the catalogue (Words), one language. Served as
/// `rows mac-main.foot.hints`.
@MainActor
enum MacFootHints {
    struct Hint: Equatable { let command: String; let key: String; let words: String }

    enum Surface: String { case list, reader, composer, search }

    /// Each surface's commands with their catalogue words, in reading order (the first ones survive a narrow window).
    /// The list and the reader show the same keys (owner 2026-11-10, A105: "we need to have more shortcuts at the bottom like
    /// R for reply E for mark as done"): E in Superhuman's own word "Mark done" (lead default; the sidebar folder and the row
    /// strip keep "Archive"), R, Reply all, F, H, L, ⌘K. Reply all has a key only where the preset gives one (Enter while a
    /// thread is open; the preset's ⇧R is "no reply", a filter), so the hint follows the key, never a typed one.
    private static let triage: [(command: String, words: String)] = [
        ("archive", "Mark done"), ("reply", "Reply"), ("reply-all", "Reply all"), ("forward", "Forward"),
        ("snooze", "Snooze"), ("label", "Label"), ("search", "Search"),
    ]
    /// The reader's keys: the triage set with the colour switch after Reply (the drawing: E, R, ⇧I, H, ⌘K), shown only while it applies.
    private static let readerKeys: [(command: String, words: String)] = [("back", "Back to list")] + Array(triage.prefix(2)) + [("toggle-colours", "Original colours")] + Array(triage.dropFirst(2))
    static let sets: [Surface: [(command: String, words: String)]] = [
        .list: triage, .reader: readerKeys,
        // ⌘J as "AI draft", the catalogue's word for the AI mark (hid-compose round 18): "Draft" alone read as a second meaning beside the reply's ✦ ⌘J
        // button and its "Draft to <name>" title (main111, 637ae165 p5; owner: "shrotcuts are contextual", one meaning per context).
        .composer: [("send", "Send"), ("ai-reply", "AI draft"), ("discard-draft", "Discard"), ("popout-draft", "Pop out")],
        // The search's own keys (owner: "WHEN pressing cmd+k enter is to validate the search"): Enter runs it, Escape closes it. Both read from the preset
        // (`enter`, `back`). Next/previous result are the fixed arrows and clear/save-as-smart-folder have no preset key, so they show no hint.
        .search: [("enter", "Search"), ("back", "Close search")],
    ]

    /// Mutant old-set only: the strip's sets before round 4 of the keys lane (the check's proof).
    private static let oldSets: [Surface: [(command: String, words: String)]] = [
        .list: [("archive", "Archive"), ("snooze", "Snooze"), ("compose", "Compose"), ("search", "Search"), ("palette", "Command palette")],
        .reader: [("reply", "Reply"), ("reply-all", "Reply all"), ("forward", "Forward"), ("archive", "Archive")],
        .composer: sets[.composer] ?? [],
        .search: sets[.composer] ?? [],   // the composer's set during a search: the defect the search check must catch
    ]

    /// The last item of every state (owner A106: "we also should have at the bottom to have hold cmd to see all shortcuts"):
    /// the ⌘ glyph as its cap, the words from the catalogue. Not a command of the preset, so it is no preset key.
    static let hold = Hint(command: "hold-cmd", key: "⌘", words: "Hold for all shortcuts")

    /// The surface whose keys the strip shows: the composer's only while its field holds the keyboard (the quick reply's
    /// field focused, or the inline composer shown in place); a thread open with its reply field unfocused shows the
    /// reader's (lead ruling, main41: the quick reply sits under every open thread). Else the list's (the sidebar
    /// and the list pane show the list's keys, the keys that act on the list it filters). Mutant stale-set: always the list.
    static func surface(_ m: AppModel) -> Surface {
        if MacShellGeometry.mutant == "stale-set" { return .list }
        // `fieldFocused` is set by the editor's focus change and is not cleared when the reply closes with the field still focused (main77: the strip read composer after Return, Escape); it counts only while the reply is open.
        if (QuickReplyModel.shared.fieldFocused && QuickReplyModel.shared.isOpen) || m.surface == ComposerIds.surface { return .composer }
        let s = SearchMacModel.shared   // the search's keys while its field holds the keyboard or a validated search is shown (beside an open mail too); the composer test above came first
        if s.holds || (s.active && s.launched) { return .search }
        return MessageBodyFeature.readerHoldsKeys(m) ? .reader : .list   // the one reader-state answer (a mail is open), the same ⇧I and O read; never the pane focus
    }

    /// The search's two states the drawing gives their own keys (docs/design/boards/search-r3-parts/designs.js FOOT_MORE and FOOT_TYPE, SPEC A150/A153): the
    /// closed line "All other mail" selected (Enter shows or hides the rest, the arrows move, Esc closes), and the field open on its suggestions (the arrows move,
    /// Tab adds the filter, Enter searches, Esc closes). The arrows and Tab are fixed keys, so they are literal; Enter and Esc come from the preset.
    @MainActor static func searchSet(_ m: AppModel) -> [(command: String, key: String?, words: String)]? {
        guard surface(m) == .search else { return nil }
        let s = SearchMacModel.shared
        if s.dropdownVisible {
            return [("move", "↑↓", "Move"), ("add-filter", "⇥", "Add as filter"), ("enter", MacHintTable.keyLabel("enter"), "Search"), ("back", MacHintTable.keyLabel("back"), "Close search")]
        }
        if s.onMore && s.showsMore {
            return [("enter", MacHintTable.keyLabel("enter"), s.restOpen ? "Hide all other mail" : "Show all other mail"), ("move", "↑↓", "Move"),
                    ("back", MacHintTable.keyLabel("back"), "Close search")]
        }
        // the drawing's FOOT (After Enter, On a related mail, the small window): a launched search with its results shown says what Enter does now,
        // Open (SearchMacActs.enter opens the selected result once the field holds the launched words), and, while the list or the reader has the keys,
        // the result's own acts (A148: every action on the mail acts on that result): Mark done, Reply, Snooze. While the field holds the keys a letter
        // types, so those three are not offered there. Round 19, d1ffe36c clause 2: the bar read "Search · Close search" after Enter.
        // Lead, 13 Nov (owner A162, walk f119): a result OPEN in the reader (Enter or a click on it). The strip still read "Open" and "Close search"
        // there; Enter opens nothing more and Esc goes back to the results, so it names the open mail's acts and that way back.
        if s.launched && !s.holds && m.surface == ReaderState.surface {
            return [("reply", MacHintTable.keyLabel("reply"), "Reply"), ("archive", MacHintTable.keyLabel("archive"), "Mark done"),
                    ("snooze", MacHintTable.keyLabel("snooze"), "Snooze"), ("back", MacHintTable.keyLabel("back"), "Back to results")]
        }
        if s.launched && !s.fieldMode && !s.results.isEmpty {
            if s.holds { return [("enter", MacHintTable.keyLabel("enter"), "Open"), ("back", MacHintTable.keyLabel("back"), "Close search")] }
            // the keys are on the results (after Return): the result's own acts, then the way back into the words ("/" or ⌘K, the preset's search key)
            return [("enter", MacHintTable.keyLabel("enter"), "Open"), ("archive", MacHintTable.keyLabel("archive"), "Mark done"),
                    ("reply", MacHintTable.keyLabel("reply"), "Reply"), ("snooze", MacHintTable.keyLabel("snooze"), "Snooze"),
                    ("search", MacHintTable.keyLabel("search"), "Edit search"),
                    ("back", MacHintTable.keyLabel("back"), "Close search")]
        }
        return nil
    }

    /// The pickers the list draws over itself (H snooze, L label, V move), each with the word of what Enter does there.
    static let pickerActs: [String: String] = [Overlay.snooze: "Snooze", Overlay.label: "Label", LabelDoors.moveOverlay: "Move"]

    static func hints(_ m: AppModel) -> [Hint] {
        // A picker over the list holds the keys: a letter types its words there, so the strip names the picker's own keys, never the list's
        // (lead, 13 Nov 2026, picture on main156: "E Mark done · R Reply · F Forward" under an open label picker; owner: "shrotcuts are contextual").
        if MacShellGeometry.mutant.isEmpty, let up = m.overlay, let act = pickerActs[up] {
            return [Hint(command: "picker-select", key: "↑↓", words: Words.text("Select")),
                    Hint(command: "picker-take", key: MacHintTable.keyLabel("enter") ?? "↩", words: Words.text(act)),
                    Hint(command: "picker-close", key: MacHintTable.keyLabel("back") ?? "esc", words: Words.text("Close"))]
        }
        if MacShellGeometry.mutant.isEmpty, let set = searchSet(m) {
            let own = set.compactMap { c in c.key.map { Hint(command: c.command, key: $0, words: Words.text(c.words)) } }
            return own + [Hint(command: hold.command, key: hold.key, words: Words.text(hold.words))]
        }
        var keys: [Hint] = ((MacShellGeometry.mutant == "old-set" ? oldSets : sets)[surface(m)] ?? []).compactMap { c in
            if c.command == "toggle-colours" {   // A129: only while the reader pane has the focus and the open mail has colours of its own; the word names what the key does now
                guard let k = MessageBodyFeature.coloursHint(), let key = ShortcutEngine.shared.preset.contextKeys.first(where: { $0.command == c.command })?.chord else { return nil }
                return Hint(command: c.command, key: key, words: k)
            }
            let key = MacShellGeometry.mutant == "literal-key" ? literal[c.command] : MacHintTable.keyLabel(c.command)
            guard let key else { return nil }   // a command the preset leaves without a key shows no hint
            // a conversation with a saved draft: R continues it (its card is the only entry to the reply; owner, 13 Nov, picture 206)
            let words = c.command == "reply" && ThreadCardsRegistry.hasShownDraft(m) ? "Continue draft" : c.words
            return Hint(command: c.command, key: key, words: Words.text(words))
        }
        // the Muted view: ⇧M takes the thread out of it (TriageKeys.mute unmutes when the target is muted), so the strip names it first
        if surface(m) == .list, m.mailbox == .muted, let key = MacHintTable.keyLabel("mute") { keys.insert(Hint(command: "mute", key: key, words: Words.text("Unmute")), at: 0) }
        // the Archive view: ⇧E brings the thread back (TriageKeys.markNotDone), so the strip names it first
        if surface(m) == .list, m.mailbox == .archive, let key = MacHintTable.keyLabel("mark-not-done") { keys.insert(Hint(command: "mark-not-done", key: key, words: Words.text("Mark not done")), at: 0) }
        if MacShellGeometry.mutant == "no-hold-hint" { return keys }   // the check's proof: the strip without its last item
        return keys + [Hint(command: hold.command, key: hold.key, words: Words.text(hold.words))]
    }

    /// Mutant literal-key only: the Superhuman keys typed in, the defect the check must catch on a preset switch.
    private static let literal = ["archive": "E", "label": "L", "snooze": "H", "compose": "C", "search": "⌘K", "palette": "⌘P",
                                  "reply": "R", "reply-all": "Enter", "forward": "F", "next-message": "N",
                                  "previous-message": "P", "send": "⌘Enter", "discard-draft": "⇧⌘,", "popout-draft": "⇧⌘P"]

    /// How many hints fit the strip's width, in order: each is a cap, a gap, its words, then the gap to the next.
    static func fitting(_ hs: [Hint], width: CGFloat) -> Int { shownIndexes(hs, width: width).count }

    /// One hint's drawn width: its cap, the gap, its words, its side padding, and `hintSlack` for what the estimate cannot see (the cap's glyph
    /// advance, the words' tracking and tabular digits).
    private static let hintSlack: CGFloat = 4
    private static func hintWidth(_ h: Hint) -> CGFloat {
        #if os(macOS)
        let font = NSFont.systemFont(ofSize: WhooshTypography.subhead.size)
        return MacHintPlace.size(h.key).width + WhooshSpacing.s6 + ceil((h.words as NSString).size(withAttributes: [.font: font]).width) + 2 * WhooshSpacing.s6 + hintSlack
        #else
        return 0
        #endif
    }

    /// Which hints the strip draws at this width. "Hold for all shortcuts" (the last one, owner A106) is always one of them: its room is taken
    /// first, then the surface's own hints in reading order while they fit WHOLE. A hint that does not fit is left out, never cut: lead, 13 Nov
    /// (owner A162, main117 picture at 960 points): the strip read "Hold for all shortcu…", the one hint the owner asked to always have.
    static func shownIndexes(_ hs: [Hint], width: CGFloat) -> Set<Int> {
        #if os(macOS)
        guard !hs.isEmpty else { return [] }
        let holdAt = hs.last?.command == hold.command ? hs.count - 1 : nil
        var shown = Set<Int>(), used: CGFloat = 0
        if let holdAt { shown.insert(holdAt); used = hintWidth(hs[holdAt]) }
        for i in hs.indices where i != holdAt {
            let w = hintWidth(hs[i]) + (shown.isEmpty ? 0 : WhooshSpacing.s12)
            if used + w > width { break }
            used += w; shown.insert(i)
        }
        return shown
        #else
        return Set(hs.indices)
        #endif
    }

    /// The strip's width: the list and reader panes, less its side insets.
    static var width: CGFloat {
        let w = MacShellState.shared.paneWidths
        return (w[.list] ?? 0) + (w[.reader] ?? 0) - 2 * WhooshSpacing.s16
    }

    /// The AI entry (the amber dot, the ⌘J cap and "Reply with AI · connect a model") is on the strip while no writing model is usable, outside the composer.
    @MainActor static func showsAIEntry(_ m: AppModel) -> Bool { surface(m) != .composer && !AISetupStatus.shared.status(.writing).usable }

    /// The room the AI entry takes, with the gap before it; 0 when it is not drawn. The hints are fitted in what is left, so the entry is never cut: main200 at
    /// 1280 points with a search open read "Reply with AI · connect a mo…" (the same rule as the hints: left out or whole, never cut).
    @MainActor static func aiEntryRoom(_ m: AppModel) -> CGFloat {
        #if os(macOS)
        guard showsAIEntry(m) else { return 0 }
        let font = NSFont.systemFont(ofSize: WhooshTypography.subhead.size)
        let words = ceil((AISetupWords.replyWithAI as NSString).size(withAttributes: [.font: font]).width)
        let button = MacHintPlace.size(MacHintTable.keyLabel("ai-reply") ?? "⌘J").width + WhooshSpacing.s6 + words + 2 * WhooshSpacing.s6
        return WhooshSpacing.s12 + 13 + WhooshSpacing.s6 + button + hintSlack
        #else
        return 0
        #endif
    }

    static func rows(_ m: AppModel) -> [ControlRow] {
        let hs = hints(m), shown = shownIndexes(hs, width: width - aiEntryRoom(m))
        return hs.enumerated().map { i, h in
            ["surface": surface(m).rawValue, "command": h.command, "key": h.key, "words": h.words, "shown": shown.contains(i), "index": i]
        }
    }
}

/// THE window's foot strip, under the list and the reader (lead ruling, round 9: one strip, the shell's): the hairline,
/// the background and the focused surface's keys, right-aligned (the left of the window's foot line is the sidebar's
/// status words; the list's undo capsule rises above the strip's centre). The panes keep 36 pt for it and draw nothing there.
struct MacFootStrip: View {
    let model: AppModel
    @State private var shell = MacShellState.shared
    @State private var ai = AISetupStatus.shared

    var body: some View {
        let hs = MacFootHints.hints(model)
        let keep = MacFootHints.shownIndexes(hs, width: MacFootHints.width - MacFootHints.aiEntryRoom(model))
        let shown = hs.indices.filter { keep.contains($0) }.map { hs[$0] }
        HStack(spacing: WhooshSpacing.s12) {
            Spacer(minLength: 0)
            ForEach(shown, id: \.command) { h in
                KeyHint(h.key, h.words).capAx("mac-main.foot.cap.\(h.command)").accessibilityIdentifier("mac-main.foot.hint.\(h.command)")
            }
            if MacFootHints.showsAIEntry(model) {   // ai-setup W1: ⌘J with no writing model, in the warn colour
                HStack(spacing: WhooshSpacing.s6) {
                    MissingSetupDot(needs: [.writing], id: "mac-main.foot.mark")   // the app's own W1 mark (one amber dot), drawn while this entry is
                    Button { AISetupDoors.fix(.writing) } label: {
                        HStack(spacing: WhooshSpacing.s6) { KeyCap(MacHintTable.keyLabel("ai-reply") ?? "⌘J"); Text(AISetupWords.replyWithAI).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.warning).lineLimit(1).whooshAx("mac-main.foot.ai-mark.words") }
                            .padding(.horizontal, WhooshSpacing.s6).padding(.vertical, WhooshSpacing.s2)
                    }
                        .buttonStyle(.plain).accessibilityIdentifier("mac-main.foot.ai-mark")
                }
            }
        }
        .padding(.horizontal, WhooshSpacing.s16)
        .frame(maxWidth: .infinity).frame(height: WhooshMetrics.footStrip)
        .background(WhooshPalette.background)
        .overlay(alignment: .top) { Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator) }
        .whooshAx("mac-main.foot.strip")   // the strip's frame: a hidden picture keeps the pasted mail page out of it (ShotWeb.coverIds)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("mac-main.foot.hints")
        .animation(nil, value: MacFootHints.surface(model))   // the set swaps on the frame the state changes (a mail opens or closes, the reply field takes the keys) (docs/MOTION.md rule 1)
    }
}
