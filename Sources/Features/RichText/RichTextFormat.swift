#if canImport(AppKit)
import AppKit

/// The format commands of the Mac editor: marks, colour, list and quote blocks, indent, the format read-back and the
/// inline `;` and `:` lists. The session's hooks call these; the keys and the toolbar reach them only through the hooks.
/// Blocks are rebuilt from the document (the way the list key always worked) with the selection carried to the same words.
extension PlatformRichText.Coordinator {
    func installFormatHooks() {
        let h = session
        h.hooks.toggleItalic = { [weak self] in self?.toggleMark(RichAttributed.italicKey) }
        h.hooks.toggleUnderline = { [weak self] in self?.toggleMark(RichAttributed.underlineKey) }
        h.hooks.toggleStrike = { [weak self] in self?.toggleMark(RichAttributed.strikeKey) }
        h.hooks.setColor = { [weak self] name in self?.setColor(name) }
        h.hooks.toggleList = { [weak self] in self?.toggleBlock(numbered: false) }
        h.hooks.toggleNumbered = { [weak self] in self?.toggleBlock(numbered: true) }
        h.hooks.toggleQuote = { [weak self] in self?.toggleQuote() }
        h.hooks.indentList = { [weak self] delta in self?.indentList(delta) ?? false }
        h.hooks.indentBlock = { [weak self] delta in self?.indentBlock(delta) }
        h.hooks.formatState = { [weak self] in self?.formatState() ?? RichFormatState() }
        h.hooks.focusBody = { [weak self] in if let tv = self?.textView { tv.window?.makeFirstResponder(tv) } }
        h.hooks.selectAll = { [weak self] in if let tv = self?.textView { tv.window?.makeFirstResponder(tv); tv.selectAll(nil) } }
        h.hooks.replaceTrigger = { [weak self] text in self?.replaceTrigger(text) }
    }

    // MARK: marks and colour

    /// Bold's way for the other marks: on the selection, or on the text typed next. All marked turns it off, else on.
    func toggleMark(_ key: NSAttributedString.Key) {
        guard let tv = textView, let st = tv.textStorage else { return }
        let r = tv.selectedRange()
        if r.length == 0 {
            var a = tv.typingAttributes
            RichAttributed.setMark(key, a[key] == nil, in: &a)
            tv.typingAttributes = a
            return
        }
        let runs = attributeRuns(in: r, of: st)
        let allOn = runs.allSatisfy { $0.attrs[key] != nil }
        edit(r) { s in
            for (a, range) in runs {
                var b = a
                RichAttributed.setMark(key, !allOn, in: &b)
                s.setAttributes(b, range: range)
            }
        }
    }

    func setColor(_ name: String?) {
        guard let tv = textView, let st = tv.textStorage else { return }
        let r = tv.selectedRange()
        if r.length == 0 {
            var a = tv.typingAttributes
            RichAttributed.setColor(name, in: &a)
            tv.typingAttributes = a
            return
        }
        let runs = attributeRuns(in: r, of: st)
        edit(r) { s in
            for (a, range) in runs where a[.link] == nil {
                var b = a
                RichAttributed.setColor(name, in: &b)
                s.setAttributes(b, range: range)
            }
        }
    }

    /// The attribute runs of a range, without the list markers (a marker keeps its own look).
    private func attributeRuns(in r: NSRange, of st: NSTextStorage) -> [(attrs: [NSAttributedString.Key: Any], range: NSRange)] {
        var out: [(attrs: [NSAttributedString.Key: Any], range: NSRange)] = []
        st.enumerateAttributes(in: r) { a, range, _ in if a[RichAttributed.markerKey] == nil { out.append((a, range)) } }
        return out
    }

    // MARK: blocks

    /// Rebuilds the paragraphs the selection touches. `force` rebuilds even when nothing in the model changed
    /// (a new line that has to receive its list marker).
    func mutateParagraphs(force: Bool = false, _ f: (inout [RichParagraph]) -> Void) {
        guard let tv = textView, let st = tv.textStorage else { return }
        let ns = st.string as NSString
        let sel = tv.selectedRange()
        func index(_ loc: Int) -> Int { ns.substring(to: min(loc, ns.length)).filter { $0 == "\n" }.count }
        let first = index(sel.location), last = max(first, index(sel.length > 0 ? NSMaxRange(sel) - 1 : sel.location))
        let doc = RichAttributed.read(st)
        guard doc.paragraphs.indices.contains(first), doc.paragraphs.indices.contains(last) else { return }
        var slice = Array(doc.paragraphs[first...last])
        f(&slice)
        var next = doc
        next.paragraphs.replaceSubrange(first...last, with: slice)
        guard force || next != doc else { return }
        let built = RichAttributed.make(next)
        let a = RichAttributed.remap(sel.location, from: st, to: built)
        let b = RichAttributed.remap(NSMaxRange(sel), from: st, to: built)
        let whole = NSRange(location: 0, length: st.length)
        guard tv.shouldChangeText(in: whole, replacementString: built.string) else { return }
        st.beginEditing(); st.setAttributedString(built); st.endEditing()
        tv.didChangeText()
        tv.setSelectedRange(NSRange(location: a, length: max(0, b - a)))
        tv.typingAttributes = RichAttributed.typing(for: next.paragraphs[min(index(a), next.paragraphs.count - 1)])
    }

    func toggleBlock(numbered: Bool) {
        mutateParagraphs { ps in
            let on = !ps.allSatisfy { $0.isListItem && $0.numbered == numbered }
            for i in ps.indices { ps[i].isListItem = on; ps[i].numbered = on && numbered; if on { ps[i].quote = false } }
        }
    }

    func toggleQuote() {
        mutateParagraphs { ps in
            let on = !ps.allSatisfy { $0.quote && !$0.isListItem }
            for i in ps.indices { ps[i].quote = on; if on { ps[i].isListItem = false; ps[i].numbered = false } }
        }
    }

    func indentBlock(_ delta: Int) {
        mutateParagraphs { ps in for i in ps.indices { ps[i].indent = min(max(ps[i].indent + delta, 0), RichParagraph.maxIndent) } }
    }

    /// Tab inside a list. A caret outside a list gets false and the key goes on as it did.
    func indentList(_ delta: Int) -> Bool {
        var inList = false
        mutateParagraphs { ps in
            guard ps.allSatisfy(\.isListItem) else { return }
            inList = true
            for i in ps.indices { ps[i].indent = min(max(ps[i].indent + delta, 0), RichParagraph.maxIndent) }
        }
        return inList
    }

    /// Return inside a list: the next item opens with the same marker, an empty item ends the list.
    func listReturn() -> Bool {
        guard let tv = textView, let st = tv.textStorage else { return false }
        let sel = tv.selectedRange()
        let doc = RichAttributed.read(st)
        let idx = (st.string as NSString).substring(to: min(sel.location, st.length)).filter { $0 == "\n" }.count
        guard doc.paragraphs.indices.contains(idx), doc.paragraphs[idx].isListItem else { return false }
        let p = doc.paragraphs[idx]
        if p.runs.allSatisfy({ $0.text.isEmpty }) {
            mutateParagraphs { ps in for i in ps.indices { ps[i].isListItem = false; ps[i].numbered = false; ps[i].indent = 0 } }
            return true
        }
        tv.insertText("\n", replacementRange: sel)
        mutateParagraphs(force: true) { ps in
            for i in ps.indices { ps[i].isListItem = true; ps[i].numbered = p.numbered; ps[i].indent = p.indent; ps[i].quote = false }
        }
        return true
    }

    // MARK: read-back

    func formatState() -> RichFormatState {
        var s = RichFormatState()
        guard let tv = textView, let st = tv.textStorage else { return s }
        let sel = tv.selectedRange()
        s.selection = sel.length
        s.bodyFocused = tv.window?.firstResponder === tv
        var a = tv.typingAttributes
        if sel.length > 0, sel.location < st.length { a = st.attributes(at: sel.location, effectiveRange: nil) }
        s.bold = a[RichAttributed.boldKey] != nil; s.italic = a[RichAttributed.italicKey] != nil
        s.underline = a[RichAttributed.underlineKey] != nil; s.strike = a[RichAttributed.strikeKey] != nil
        s.color = a[RichAttributed.colorKey] as? String
        let doc = RichAttributed.read(st)
        let idx = (st.string as NSString).substring(to: min(sel.location, st.length)).filter { $0 == "\n" }.count
        if doc.paragraphs.indices.contains(idx) {
            let p = doc.paragraphs[idx]
            s.list = p.isListItem ? (p.numbered ? "number" : "bullet") : "none"
            s.quote = p.quote; s.indent = p.indent
        }
        return s
    }

    // MARK: the inline lists

    /// The caret's rectangle (or a range's) in the editor's own coordinates.
    func rectInEditor(_ range: NSRange) -> CGRect? {
        guard let tv = textView, let clip = tv.enclosingScrollView?.contentView, let win = tv.window else { return nil }
        let screen = tv.firstRect(forCharacterRange: range, actualRange: nil)
        return clip.convert(win.convertFromScreen(screen), from: nil)
    }

    func updateTrigger() {
        guard let tv = textView else { return }
        let sel = tv.selectedRange()
        let ns = tv.string as NSString
        var found: RichTriggerState?
        if sel.length == 0, sel.location <= ns.length {
            let line = ns.lineRange(for: sel)
            let before = ns.substring(with: NSRange(location: line.location, length: sel.location - line.location))
            if let t = RichTrigger.detect(before: before), let r = rectInEditor(sel) { found = RichTriggerState(trigger: t, caret: r) }
        }
        if session.triggerState != found { session.triggerState = found }
    }

    func replaceTrigger(_ text: String) {
        guard let tv = textView, let t = session.triggerState?.trigger else { return }
        let caret = tv.selectedRange().location
        guard caret >= t.length else { return }
        tv.insertText(text, replacementRange: NSRange(location: caret - t.length, length: t.length))
    }

    /// Return, Tab, Up, Down and Escape: an open inline list takes them first, a list item takes Return and Tab next.
    func command(_ selector: Selector) -> Bool {
        if session.triggerState != nil {
            let key: RichListKey? = {
                switch selector {
                case #selector(NSResponder.moveUp(_:)): return .up
                case #selector(NSResponder.moveDown(_:)): return .down
                case #selector(NSResponder.insertNewline(_:)), #selector(NSResponder.insertTab(_:)): return .accept
                case #selector(NSResponder.cancelOperation(_:)): return .cancel
                default: return nil
                }
            }()
            if let key, session.listKeyHandler?(key) == true { return true }
        }
        switch selector {
        case #selector(NSResponder.insertNewline(_:)): return listReturn()
        case #selector(NSResponder.insertTab(_:)): return indentList(1)
        case #selector(NSResponder.insertBacktab(_:)): return indentList(-1)
        default: return false
        }
    }
}
#endif
