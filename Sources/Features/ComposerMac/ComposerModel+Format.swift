import Foundation

/// A text colour the Colour key offers: a name the editor knows (`RichColor`), or nil for the default.
struct ComposerColor: Equatable {
    let name: String?
    let slug: String
    var title: String {
        switch name {
        case "secondary": return ComposerWords.colourGrey
        case "accent": return ComposerWords.colourBlue
        case "warning": return ComposerWords.colourOrange
        default: return ComposerWords.colourDefault
        }
    }
    /// Only palette tokens: the design tokens hold no red, green or purple text colour yet (reported in docs/lanes/composermac.md).
    static let all: [ComposerColor] = [ComposerColor(name: nil, slug: "default")] + RichColor.allCases.map { ComposerColor(name: $0.rawValue, slug: $0.rawValue) }
}

/// The format keys (bold to indent), the link and colour popovers, and the inline `;` and `:` lists.
extension ComposerModel {
    // MARK: link and colour

    @discardableResult
    public func toggleLink() -> Bool {
        guard isOpen, !isCounting else { return false }
        let open = !linkOpen; closePanels(); linkOpen = open; linkInput = ""
        return true
    }

    /// The address becomes a link on the selection, or goes in as linked text when nothing is selected.
    @discardableResult
    public func applyLink() -> Bool {
        var s = linkInput.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !s.isEmpty else { line = ComposerWords.noAddress; return true }
        if !s.contains("://") && !s.hasPrefix("mailto:") { s = (s.contains("@") && !s.contains("/")) ? "mailto:" + s : "https://" + s }
        guard RichHTML.safeURL(s) != nil else { line = ComposerWords.notAWebAddress(s); return true }
        session.hooks.setLink(s)
        closePanels(); line = ""
        session.hooks.focusBody()
        return true
    }

    @discardableResult
    public func toggleColor() -> Bool {
        guard isOpen, !isCounting else { return false }
        let open = !colorOpen; closePanels(); colorOpen = open; listIndex = 0
        if open { resignText() }
        return true
    }

    @discardableResult
    func pickColor(_ name: String?) -> Bool {
        guard colorOpen else { return false }
        closePanels()
        session.hooks.focusBody()
        session.hooks.setColor(name)
        return true
    }

    // MARK: the inline lists

    struct InlineItem: Equatable, Identifiable {
        let id: String
        let title: String
        let glyph: String
        let text: String
    }

    /// The rows of the list the caret opened (`;name` or `:smi`); empty when none is open or the owner pressed Esc on it.
    var inlineItems: [InlineItem] {
        guard isOpen, let t = session.triggerState?.trigger, t != inlineDismissed else { return [] }
        switch t.kind {
        case .snippet:
            let all = library.snippets
            return ComposerSnippetMatch.indices(t.query, names: all.map(\.name)).prefix(ComposerEmoji.limit).map {
                InlineItem(id: all[$0].id, title: all[$0].name, glyph: "", text: all[$0].text)
            }
        case .emoji:
            return ComposerEmoji.matches(t.query).map { InlineItem(id: $0.name, title: ":" + $0.name, glyph: $0.glyph, text: $0.glyph) }
        }
    }

    @discardableResult
    func pickInline(_ i: Int) -> Bool {
        let items = inlineItems
        guard items.indices.contains(i) else { return false }
        inlineDismissed = nil
        session.hooks.replaceTrigger(items[i].text)
        return true
    }

    /// The editor hands Up, Down, Return, Tab and Escape here while an inline list is open. False leaves the key to the text.
    func inlineKey(_ k: RichListKey) -> Bool {
        let items = inlineItems
        guard !items.isEmpty else { return false }
        switch k {
        case .up: inlineIndex = max(0, inlineIndex - 1)
        case .down: inlineIndex = min(items.count - 1, inlineIndex + 1)
        case .accept: _ = pickInline(min(inlineIndex, items.count - 1))
        case .cancel: inlineDismissed = session.triggerState?.trigger
        }
        return true
    }

    /// Called when the open trigger changes: the highlight starts on the first row, a new trigger is not dismissed.
    func triggerChanged(_ t: RichTrigger?) {
        inlineIndex = 0
        if t != inlineDismissed { inlineDismissed = nil }
    }

    // MARK: keys that put a character in the text

    /// The Use Snippet Inline key (`;`) and the Insert Emoji command (`:`) type their character, which opens the list.
    @discardableResult
    func startInline(_ mark: String) -> Bool {
        guard isOpen, !isCounting else { return false }
        insertAtCaret(mark)
        return true
    }
}

/// What the control socket reads of the format keys and the keys' controls.
extension ComposerModel {
    /// Esc on a popover from outside the keyboard: closes whatever is open and gives the text the cursor back.
    @discardableResult
    func dismissPanels() -> Bool {
        guard anyPanelOpen else { return false }
        let format = linkOpen || colorOpen
        closePanels()
        if format { session.hooks.focusBody() }
        return true
    }

    /// A format-bar button pressed from outside the mouse.
    @discardableResult
    func runFormat(_ act: (RichTextSession) -> Void) -> Bool {
        guard isOpen, !isCounting else { return false }
        act(session)
        return true
    }

    /// The format at the caret: marks, colour, list, quote, indent, selection length, and whether the text holds the cursor.
    var formatRow: [String: Any] {
        let f = session.hooks.formatState()
        return ["ax": ComposerIds.format, "bold": f.bold, "italic": f.italic, "underline": f.underline, "strike": f.strike,
                "color": f.color ?? "", "list": f.list, "quote": f.quote, "indent": f.indent, "selection": f.selection, "body_focused": f.bodyFocused]
    }

    /// One row per control that carries a key: the key, the tooltip it shows, and whether the key is also a cap on it.
    var controlRows: [[String: Any]] {
        var rows: [[String: Any]] = ComposerControlKeys.entries.map {
            ["ax": $0.id, "title": Words.text($0.title), "key": $0.key, "tooltip": ComposerControlKeys.tooltip($0.title, $0.key), "cap": $0.cap]
        }
        var windowControls: [ComposerControlKeys.Entry] = []
        if kind.canPopIn { windowControls.append(ComposerControlKeys.popIn) }
        if kind != .new { windowControls.append(ComposerControlKeys.thread) }
        for e in windowControls {
            rows.append(["ax": e.id, "title": Words.text(e.title), "key": e.key, "tooltip": ComposerControlKeys.tooltip(e.title, e.key), "cap": e.cap])
        }
        #if canImport(AppKit)
        rows += RichTextToolbar.items.flatMap { $0 }.map {
            ["ax": ComposerIds.formatBarButton($0.id), "title": Words.text($0.title), "key": $0.key, "tooltip": Words.text($0.title) + "  " + $0.key, "cap": false]
        }
        #endif
        return rows
    }
}
