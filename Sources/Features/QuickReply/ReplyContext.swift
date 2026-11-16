import Foundation

/// Who the quick reply speaks as and to: the From address (the thread's receiving account), its signature,
/// and the placeholder naming the person replied to. One value, read when the field opens.
public struct ReplyContext: Equatable, Sendable {
    public var from: String
    public var signature: String
    public var placeholder: String
    /// The account the From address belongs to (the thread's receiving account until the owner picks another).
    public var accountId: String
    public init(from: String, signature: String, placeholder: String, accountId: String = "") {
        self.from = from; self.signature = signature; self.placeholder = placeholder; self.accountId = accountId
    }
}

/// The port the field reads its identity through; tests and the notch can serve another one.
@MainActor
public protocol ReplyContextSource {
    func context(threadId: String, messageId: String, replyTo: String, core: WhooshCore) -> ReplyContext
}

/// The contract's own reads: the MESSAGE replied to names the account that received it (`message(messageId:)`),
/// the thread's account only when the message is not served; `accounts()` names that account's address.
/// (The fixture core's `thread(threadId:)` answers its one open thread for any single-message id, so a thread-only
/// read gave every reply the Nordlicht address: verdict sweep on main41, 5 of 5 threads.)
/// The signature has no read on WhooshCore yet (finding: `signature(address:) -> Signature?`), so it stays
/// empty here and is never invented; the one line that fills it is `signature` below.
public struct CoreReplyContext: ReplyContextSource {
    public init() {}
    public func context(threadId: String, messageId: String, replyTo: String, core: WhooshCore) -> ReplyContext {
        let accountId = core.message(messageId: messageId)?.accountId ?? core.thread(threadId: threadId)?.accountId
        let address = core.accounts().first { $0.id == accountId }?.address ?? ""
        return ReplyContext(from: address, signature: signature(address, core), placeholder: QuickReplyCopy.placeholder(replyTo), accountId: accountId ?? "")
    }

    private func signature(_ address: String, _ core: WhooshCore) -> String { core.signature(address: address)?.text ?? "" }
}

/// Words the field shows, in one place, through the 14-language catalogue.
public enum QuickReplyCopy {
    public static var placeholderFallback: String { Words.text("Reply…") }
    public static var from: String { Words.text("From") }
    public static var sending: String { Words.text("Sending") }
    public static var retry: String { Words.text("Retry") }
    public static var failedFallback: String { Words.text("The reply was not sent") }
    public static var send: String { Words.text("Send, ⌘ Return") }
    /// The key cap on every reply send control (the same glyphs the composer's cap uses) and the tooltip that names that one key; ⌃↵ and Enter still send, unshown.
    public static let sendCap = "⌘↵"
    public static var sendHelp: String { Words.text("Send (⌘↵)") }
    public static var fromMenuTitle: String { Words.text("Send from") }
    public static var emoji: String { Words.text("Emoji") }
    public static func placeholder(_ name: String) -> String {
        name.isEmpty ? placeholderFallback : Words.text("Reply to %@…", name)
    }

    // Round 5 (board cards-s3): the slab's header, its rows, the picker and the pop-out.
    /// The header while the field is empty: "Répondre à <x>"; with words it reads "Brouillon à <x>" (owner 23:04).
    public static func replyTo(_ name: String) -> String { name.isEmpty ? Words.text("Reply") : Words.text("Reply to %@", name) }
    public static func draftTo(_ name: String) -> String { name.isEmpty ? Words.text("Draft") : Words.text("Draft to %@", name) }
    public static let headerCap = "⇧H"
    /// The header's key while the reply's text holds the keyboard: ⇧H is a typed letter there, ⇧⌘C (Cc) opens the header from the text.
    public static let headerCapTyping = "⇧⌘C"
    public static var expandHeader: String { Words.text("Expand header (⇧H)") }
    public static var collapseHeader: String { Words.text("Collapse header (⇧H)") }
    public static let popoutCap = "⇧⌘P"
    public static var popout: String { Words.text("Pop out (⇧⌘P)") }
    public static var to: String { Words.text("To") }
    public static var cc: String { Words.text("Cc") }
    public static var bcc: String { Words.text("Bcc") }
    public static var subject: String { Words.text("Subject") }
    public static var add: String { Words.text("Add") }
    public static var remove: String { Words.text("Remove") }
    public static let fromCap = "⇧⌘F"
    public static var fromHelp: String { Words.text("Send from (⇧⌘F)") }
    public static var filterAccounts: String { Words.text("Filter accounts") }
    public static func otherAccounts(_ n: Int) -> String { Words.text("%@ other accounts", "\(n)") }
    public static var pickerChoose: String { Words.text("choose") }
    public static var pickerConfirm: String { Words.text("confirm") }
    public static var pickerClose: String { Words.text("close") }
    public static var sendWord: String { Words.text("Send") }
    public static var cancel: String { Words.text("Cancel") }
    public static let cancelCap = "Z"
    public static var cancelHelp: String { Words.text("Undo (Z)") }
    public static func sendingIn(_ s: Int) -> String { Words.text("Sending in %@ s", "\(max(s, 0))") }
}
