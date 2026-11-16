import SwiftUI

/// The iPhone composer's registrations: rows, clicks, keys and the screen register here, never in a hub.
/// ax ids follow `<surface>.<region>[.<element>...][.<index>]` (checks/composerphone-check.py).
public enum ComposerPhoneFeature: FeatureModule {
    public static let id = "composerphone"
    public static let surfaces: [String] = ["iphone-composer"]

    static var state: ComposerPhoneState { .shared }
    static var model: ComposerModel { .shared }
    static var speech: PhoneSpeech { .shared }

    /// The doors the inbox and the thread call instead of setting the overlay by hand (docs/lanes/composerphone.md "Wiring").
    @MainActor public static func compose(_ m: AppModel) { _ = state.open(reply: nil, m) }
    @MainActor public static func reply(_ m: AppModel, messageId: String) { _ = state.open(reply: messageId, m) }

    public static func view(for slot: SurfaceSlot) -> AnyView? {
        slot.id == ComposerPhoneIds.surface ? AnyView(ComposerPhoneScreen()) : nil
    }

    /// The socket's `type`, a paste and the finished dictation: the field that has the cursor takes it; words from the microphone go to the body.
    public static func typeText(_ text: String, _ m: AppModel) -> Bool {
        guard model.isOpen, hasFocus(m) else { return false }
        if speech.isDictating { state.insertDictated(text); return true }
        return state.type(text)
    }

    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        ["⌘Enter": { m in hasFocus(m) && model.isOpen ? state.send(m) : false }]
    }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            ComposerPhoneIds.header: { m in ComposerPhoneProbe.header(m) },
            ComposerPhoneIds.from: { m in ComposerPhoneProbe.from(m) },
            ComposerPhoneIds.fromChoices: { m in ComposerPhoneProbe.fromChoices(m) },
            ComposerPhoneIds.body: { _ in ComposerPhoneProbe.body() },
            ComposerPhoneIds.line: { _ in model.isOpen ? [["ax": ComposerPhoneIds.line, "text": model.line]] : [] },
            ComposerPhoneIds.dictate: { _ in ComposerPhoneProbe.dictate() },
            ComposerPhoneIds.speechModel: { _ in ComposerPhoneProbe.speechModel() },
            ComposerPhoneIds.undo: { _ in ComposerPhoneProbe.undo() },
            ComposerPhoneIds.completions: { _ in
                guard model.isOpen else { return [] }
                return Array(model.completions.prefix(3)).enumerated().map { ["ax": ComposerPhoneIds.completion($0.offset), "name": $0.element.name, "address": $0.element.address] }
            },
            ComposerPhoneIds.quoted: { _ in
                model.isOpen && !model.quoted.isEmpty ? [["ax": ComposerPhoneIds.quoted, "count": model.quoted.count, "open": state.quotedOpen]] : []
            },
        ]
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [
            ComposerPhoneIds.cancel: { m in guard model.isOpen, !model.isCounting else { return false }; state.close(m); return true },
            ComposerPhoneIds.send: { m in state.send(m) },
            ComposerPhoneIds.undoCancel: { m in state.undoSend(m) },
            ComposerPhoneIds.dictate: { _ in guard model.isOpen else { return false }; speech.tap(); return true },
            ComposerPhoneIds.attach: { _ in guard model.isOpen, !model.isCounting else { return false }; state.attaching.toggle(); return true },
            ComposerPhoneIds.from: { _ in guard model.isOpen, !model.isCounting else { return false }; state.fromOpen.toggle(); return true },
            ComposerPhoneIds.ccToggle: { _ in
                guard model.isOpen else { return false }
                let on = !(model.showCc || model.showBcc); model.showCc = on; model.showBcc = on; return true
            },
            ComposerPhoneIds.quotedToggle: { _ in guard model.isOpen, !model.quoted.isEmpty else { return false }; state.quotedOpen.toggle(); return true },
            ComposerPhoneIds.to: { _ in focusField(.to) },
            ComposerPhoneIds.cc: { _ in focusField(.cc) },
            ComposerPhoneIds.bcc: { _ in focusField(.bcc) },
            ComposerPhoneIds.subject: { _ in focusField(.subject) },
            ComposerPhoneIds.body: { _ in focusField(.body) },
        ]
        for i in 0..<max(model.accounts.count, 8) { t[ComposerPhoneIds.fromChoice(i)] = { m in state.pickFrom(i, m) } }
        for i in 0..<3 {
            t[ComposerPhoneIds.completion(i)] = { _ in
                let found = Array(model.completions.prefix(3))
                guard found.indices.contains(i) else { return false }
                model.to.append(found[i].address); model.toInput = ""; return true
            }
        }
        for i in 0..<8 {
            t[ComposerPhoneIds.chipRemove("to", i)] = { _ in model.to.indices.contains(i) && removeAt(i, in: \.to) }
            t[ComposerPhoneIds.chipRemove("cc", i)] = { _ in model.cc.indices.contains(i) && removeAt(i, in: \.cc) }
            t[ComposerPhoneIds.chipRemove("bcc", i)] = { _ in model.bcc.indices.contains(i) && removeAt(i, in: \.bcc) }
        }
        return t
    }

    private static func focusField(_ f: ComposerPhoneState.Field) -> Bool {
        guard model.isOpen, !model.isCounting else { return false }
        if f == .cc { model.showCc = true }
        if f == .bcc { model.showBcc = true }
        state.focus = f
        return true
    }

    private static func removeAt(_ i: Int, in list: KeyPath<ComposerModel, [String]>) -> Bool {
        model.removeRecipient(model[keyPath: list][i]); return true
    }
}

/// The `rows` answers: the same facts the screen draws.
@MainActor
enum ComposerPhoneProbe {
    static func header(_ m: AppModel) -> [ControlRow] {
        let s = ComposerPhoneState.shared, cm = ComposerModel.shared
        guard cm.isOpen else { return [["ax": ComposerPhoneIds.header, "is_open": false]] }
        return [["ax": ComposerPhoneIds.header, "is_open": true, "kind": cm.kind.rawValue, "title": ComposerWords.title(cm.kind),
                 "to": cm.to, "cc": cm.cc, "bcc": cm.bcc, "cc_visible": cm.showCc, "bcc_visible": cm.showBcc, "subject": cm.subject,
                 "focused": s.focus?.rawValue ?? "", "line": cm.line, "phase": s.phaseWord, "accounts": cm.accounts.count,
                 "from_open": s.fromOpen, "attach_open": s.attaching, "can_send": cm.canSend, "reply_message_id": cm.replyMessageId ?? "",
                 "attachments": cm.attachments.count]]
    }

    static func from(_ m: AppModel) -> [ControlRow] {
        let s = ComposerPhoneState.shared
        guard ComposerModel.shared.isOpen, let a = s.current(m) else { return [] }
        return [["ax": ComposerPhoneIds.from, "text": a.address, "id": a.id, "name": a.name, "open": s.fromOpen,
                 "mark_label": a.name, "mark_colour": a.colour, "mark_icon": a.markIcon, "mark_attention": a.needsAttention]]
    }

    static func fromChoices(_ m: AppModel) -> [ControlRow] {
        let s = ComposerPhoneState.shared
        guard ComposerModel.shared.isOpen else { return [] }
        return s.accountChoices(m).enumerated().map { i, a in
            ["ax": ComposerPhoneIds.fromChoice(i), "text": a.address, "id": a.id, "name": a.name, "picked": a.id == ComposerModel.shared.fromId,
             "mark_label": a.name, "mark_colour": a.colour, "mark_icon": a.markIcon]
        }
    }

    static func body() -> [ControlRow] {
        let cm = ComposerModel.shared
        guard cm.isOpen else { return [] }
        return [["ax": ComposerPhoneIds.body, "text": cm.bodyText, "outgoing": cm.outgoing, "signature": cm.signature, "dictated": PhoneSpeech.shared.dictated]]
    }

    static func dictate() -> [ControlRow] {
        let p = PhoneSpeech.shared
        guard ComposerModel.shared.isOpen else { return [] }
        return [["ax": ComposerPhoneIds.dictate, "state": p.mic.rawValue, "percent": p.mic == .downloading ? p.percent : 0, "live": p.liveWords, "line": p.line]]
    }

    static func speechModel() -> [ControlRow] {
        let p = PhoneSpeech.shared
        let word: String
        switch p.model {
        case .ready: word = "ready"
        case .missing: word = "missing"
        case .downloading: word = "downloading"
        case .failed: word = "failed"
        }
        return [["ax": ComposerPhoneIds.speechModel, "state": word, "progress": p.progress, "percent": p.percent]]
    }

    /// The undo bar: "Sending" with the seconds left of the core's window and Undo offered, then "Sent"; no row while editing.
    static func undo() -> [ControlRow] {
        let s = ComposerPhoneState.shared, cm = ComposerModel.shared
        switch cm.phase {
        case .counting(let id, _):
            return [["ax": ComposerPhoneIds.undo, "text": ComposerWords.sending, "seconds": s.secondsLeft(), "undo_offered": true, "outbox_id": id]]
        case .sent:
            return [["ax": ComposerPhoneIds.undo, "text": ComposerWords.sent, "seconds": 0, "undo_offered": false]]
        default:
            return []
        }
    }
}
