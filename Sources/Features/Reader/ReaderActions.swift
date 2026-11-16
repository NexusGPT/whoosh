import Foundation
#if canImport(AppKit)
import AppKit
#endif

/// What the reader does on a click or a key: copy the sign-in code, open the sender's card, delete the message.
/// Each runs on the input frame; the pane animates afterwards and nothing waits for it.
@MainActor
enum ReaderActions {
    /// The code goes on the clipboard and the message is marked read. False when the open message carries no code.
    static func copyCode(_ m: AppModel) -> Bool {
        guard let t = ReaderState.open(m), let code = t.open.code else { return false }
        #if canImport(AppKit)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(code, forType: .string)
        #endif
        ReaderState.shared.copiedFor = t.messageId
        m.perform(.markRead(messageIds: [t.messageId], read: true))
        return true
    }

    static func clipboardText() -> String? {
        #if canImport(AppKit)
        NSPasteboard.general.string(forType: .string)
        #else
        nil
        #endif
    }

    /// The sender's card: the core's contact for the address (a merged card answers for any of its addresses),
    /// opened in the contacts window on the card's first address.
    static func openSender(_ m: AppModel) -> Bool {
        guard let t = ReaderState.open(m), !t.open.address.isEmpty else { return false }
        let card = m.core.contact(address: t.open.address)
        LabelsContactsModel.shared.open(address: card?.addresses.first ?? t.open.address)
        return WindowBridge.open(LabelsContactsMacFeature.slot, in: m)
    }

    /// How many messages the core holds from the open message's sender (the card's list, newest first).
    static func senderMessageCount(_ m: AppModel, _ t: ReaderThread) -> Int {
        guard let c = m.core.contact(address: t.open.address) else { return 0 }
        return m.core.contactMessages(contactId: c.id).count
    }

    /// The Delete key in an open reader: the open message goes (see `deleteFocused`).
    /// A draft card's own Delete is ⌘⌫ (owner, 13 Nov: "cmd+del not just del it would clash"), so the plain key means the same with or without a card.
    static func deleteOpen(_ m: AppModel) -> Bool { m.surface == ReaderState.surface && deleteFocused(m) }

    /// The delete command (⇧#, the strip's trash): the focused message goes with an undo entry (the capsule offers Z)
    /// and the reader closes on the list. A field that is open (the quick reply) keeps the key.
    static func deleteFocused(_ m: AppModel) -> Bool {
        guard let id = m.shownMessageId, !QuickReplyModel.shared.isOpen else { return false }
        guard m.perform(.deleteMessage(messageId: id)) else { return false }
        TriageActions.state.say(Said("Deleted"))  // the list's undo capsule (Undo / Z): the key, so the words follow a language switch
        m.surface = SurfaceSlot.macMain.id
        return true
    }

    /// A button of the strip or the header: the engine's command runs on the message the pane shows, through the same
    /// table the key uses (`ShortcutEngine.click` counts the mouse use and shows the key hint). The list's focus first
    /// moves to the shown message, so the command acts on what the person is looking at.
    @discardableResult
    static func run(_ command: String, _ m: AppModel = .shared) -> Bool {
        guard let id = ReaderFocus.shared.target(m) else { return false }
        if !m.resultsShow { m.focus(messageId: id) }   // results show: the shown mail is the selected result, the inbox focus stays where it was
        return ShortcutEngine.shared.click(command, m)
    }
}
