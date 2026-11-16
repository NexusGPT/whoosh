import Foundation

/// The rows the socket reads (`rows mac-entra-guide.<region>`): what the screen shows now, so a check reads exactly what a person sees.
@MainActor enum EntraGuideRows {
    static let A = EntraGuideWords.surface
    static var m: EntraGuideModel { EntraGuideModel.shared }

    static func state() -> [ControlRow] {
        [["ax": "\(A).state", "open": m.isOpen, "leaving": m.leaving, "steps": m.total, "chapters": m.data.chapters, "step": m.number, "opened": m.opened,
          "copied": m.copied, "clipboard": m.clipboard, "pasted": m.pasted, "valid_id": m.validId, "opened_links": m.openedLinks,
          "surface": AppModel.shared.surface == A, "has_save_handler": m.onSave != nil, "last_saved": m.lastSaved]]
    }

    /// The step on screen: its count as the head says it, its words, its picture and ring.
    static func step() -> [ControlRow] {
        guard m.isOpen, m.total > 0 else { return [] }
        return [row(m.step, current: true)]
    }

    /// Every step, in order: the check walks them and reads each one's words and target.
    static func steps() -> [ControlRow] { m.data.steps.map { row($0, current: $0.n == m.number) } }

    private static func row(_ s: EntraGuideStep, current: Bool) -> ControlRow {
        let p = m.data.plate(s.plate)
        return ["ax": "\(A).step.\(s.n)", "n": s.n, "of": m.total, "count": EntraGuideWords.stepOf(s.n, m.total), "chapter": s.chapter, "chapter_title": s.chapterTitle,
                "say": s.say(domain: m.domain), "cap": s.cap, "hint": s.hintText ?? "", "plate": s.plate, "drawn": p?.isDrawn ?? false, "location": p?.location ?? "",
                "ring": s.ring, "focus": s.focus, "covers": s.cover?.count ?? 0, "group": s.group ?? "", "slot": s.slot == true, "paste": s.paste == true,
                "primary": s.primary == true, "current": current]
    }

    /// Every button on screen, with the key it shows (the Copy buttons are numbered, ⌘1 to ⌘3).
    static func controls() -> [ControlRow] {
        guard m.isOpen, m.total > 0 else { return [] }
        let W = EntraGuideWords.self
        var out: [ControlRow] = [control("close", W.close, enabled: true)]
        if !m.isLast { out.append(control("skip", W.skip, enabled: true)) }
        out.append(control("back", W.back, enabled: m.index > 0))
        for v in m.values where v.number > 0 {
            out.append(["ax": "\(A).copy.\(v.number)", "id": "copy.\(v.number)", "title": m.copied == v.number ? W.copied : W.copy, "key": "⌘\(v.number)", "enabled": true,
                        "tooltip": W.tooltip(W.copy, "⌘\(v.number)")])
        }
        if m.isLast { out.append(control("save", W.save, enabled: m.validId)) }
        else { out.append(control("next", m.nextIsOpen ? W.openEntra : W.next, enabled: true)) }
        return out
    }

    private static func control(_ id: String, _ title: String, enabled: Bool) -> ControlRow {
        let key = EntraGuideWords.key(id)
        return ["ax": "\(A).\(id)", "id": id, "title": title, "key": key, "enabled": enabled, "tooltip": key.isEmpty ? title : EntraGuideWords.tooltip(title, key)]
    }

    /// The current step's value rows: what is copied or picked, which one glows, whether Copy shows Copied.
    static func values() -> [ControlRow] {
        guard m.isOpen else { return [] }
        return m.values.map { v in
            ["ax": "\(A).value.\(v.def.label)", "label": Words.text(v.def.label), "value": v.value, "copy": v.def.copy, "number": v.number, "current": v.current,
             "copied": v.number > 0 && m.copied == v.number, "tag": v.def.copy ? "" : EntraGuideWords.pickTag]
        }
    }

    /// The ID field (only on the slot step and the last step): its text, the tick, the note under it.
    static func paste() -> [ControlRow] {
        guard m.isOpen, m.step.wantsId else { return [] }
        let wrong = !m.pasted.isEmpty && !m.validId
        return [["ax": "\(A).paste", "kind": m.step.slot == true ? "slot" : "paste", "text": m.pasted, "valid": m.validId, "tick": m.validId, "focused": m.pasteFocus,
                 "clearable": !m.pasted.isEmpty, "note": m.step.slot == true ? "" : (wrong ? EntraGuideWords.idWrong : EntraGuideWords.keychain), "wrong": wrong,
                 "label": m.step.slot == true ? EntraGuideWords.slotLabel : EntraGuideWords.pasteLabel]]
    }

    /// The hairline: one segment per step, filled up to the current one; the aurora once a valid ID is in at the last step.
    static func progress() -> [ControlRow] {
        guard m.isOpen else { return [] }
        return [["ax": "\(A).progress", "segments": m.total, "filled": m.number, "aurora": m.isLast && m.validId]]
    }

    /// The plate: which picture, the ring and the focus box as fractions of it, the browser-bar line.
    static func plate() -> [ControlRow] {
        guard m.isOpen, m.total > 0, let p = m.data.plate(m.step.plate) else { return [] }
        return [["ax": "\(A).plate", "plate": p.id, "drawn": p.isDrawn, "flag": p.isDrawn ? EntraGuideWords.drawn : "", "location": p.location, "ring": m.step.ring,
                 "focus": m.step.focus, "caption": m.step.cap, "covers": m.step.cover?.count ?? 0, "host": EntraGuideWords.entraHost]]
    }
}
