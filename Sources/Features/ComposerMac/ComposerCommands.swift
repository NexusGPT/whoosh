import Foundation

/// The Superhuman composer keys, as the engine's commands (Commands.swift names each id; the owning lane is composermac).
/// A command runs only while the composer holds the focus and answers false otherwise, so the same chord (⌘K, ⌘O, ⌘U, Tab)
/// keeps its meaning in the reader and the list.
@MainActor
enum ComposerCommands {
    private static var model: ComposerModel { ComposerMacFeature.model }

    /// True while this composer is the window the keys go to.
    private static func active(_ m: AppModel) -> Bool { model.isOpen && ComposerMacFeature.hasFocus(m) }

    private static func when(_ run: @escaping (AppModel) -> Bool) -> (AppModel) -> Bool {
        { m in active(m) ? run(m) : false }
    }

    /// The door ran, or it was refused with its sentence: either way the key was this door's.
    private static func taken(_ door: @escaping (AppModel) -> Bool) -> (AppModel) -> Bool {
        { m in let n = model.refusals; return door(m) || model.refusals != n }
    }

    private static func hook(_ f: @escaping (RichTextViewHooks) -> Void) -> (AppModel) -> Bool {
        when { _ in f(model.session.hooks); return true }
    }

    static let table: [String: (AppModel) -> Bool] = [
        // fields
        "compose-to": when { _ in model.focus(.to) },
        "compose-cc": when { _ in model.focus(.cc) },
        "compose-bcc": when { _ in model.focus(.bcc) },
        "compose-from": when { _ in model.focus(.from) },
        "edit-subject": when { _ in model.focus(.subject) },
        "edit-message": when { _ in model.focus(.body) },
        "attach": when { _ in model.toggleAttachPanel() },
        "discard-draft": when { model.requestDiscard($0) },
        "move-contacts-to-bcc": when { _ in model.moveContactsToBcc() },
        // snippets and emoji
        "snippet-inline": when { _ in model.startInline(";") },
        "use-snippet": when { _ in model.toggleSnippets() },
        "emoji": when { _ in model.startInline(":") },
        // format
        "bold": hook { $0.toggleBold() },
        "italic": hook { $0.toggleItalic() },
        "underline": hook { $0.toggleUnderline() },
        "strikethrough": hook { $0.toggleStrike() },
        "hyperlink": when { _ in model.toggleLink() },
        "color": when { _ in model.toggleColor() },
        "bullet-list": hook { $0.toggleList() },
        "numbered-list": hook { $0.toggleNumbered() },
        "quote": hook { $0.toggleQuote() },
        "indent-list": when { _ in model.session.hooks.indentList(1) },
        "outdent-list": when { _ in model.session.hooks.indentList(-1) },
        "increase-indent": hook { $0.indentBlock(1) },
        "decrease-indent": hook { $0.indentBlock(-1) },
        // send
        "send": when { model.send($0) },
        "send-and-done": when { model.sendAndMarkDone($0) },
        "instant-send": when { model.sendInstantly($0) },
        "remind-me": when { _ in model.toggleRemind() },
        "send-later": when { _ in
            model.closePanels(); model.sendLaterOpen = true; return true
        },
        // pop-out windows (round 3): they work from the list, the thread and the window alike, so they do not wait for the focus
        // A door refused because the window's draft has work said so in the window (canReplaceDraft): the key was taken. It must not
        // pass on to the chord's other meaning: ⇧R's is the list's No Reply filter (KeyPreset contextKeys then the row), which main92
        // and main93 found switched on after "a second draft never replaces the first".
        "popout-reply": taken { model.popOutReply($0) },
        "popout-reply-all": taken { model.popOutReplyAll($0) },
        "popout-new": taken { model.popOutNew($0) },
        "popout-forward": taken { model.popOutForward($0) },
        "popout-draft": { model.togglePop($0) },
        "popout-draft-search": { model.popOutAndSearch($0) },
        "switch-draft": { model.switchDraft($0) },
    ]

    /// Up, Down, Left, Right, Tab and Return while a list of the composer is open (the inline list under the caret first).
    /// They are plain keys: on the real keyboard they reach the text and its list directly, and arrive here only from the
    /// socket or when the lists have taken the cursor out of the text.
    static let listKeys: [String: (AppModel) -> Bool] = [
        "Up": { _ in model.isOpen && (model.inlineKey(.up) || model.listKey(-1)) },
        "Down": { _ in model.isOpen && (model.inlineKey(.down) || model.listKey(1)) },
        "Left": { _ in model.isOpen && model.colorOpen && model.listKey(-1) },
        "Right": { _ in model.isOpen && model.colorOpen && model.listKey(1) },
        "Tab": { _ in model.isOpen && model.inlineKey(.accept) },
    ]
}
