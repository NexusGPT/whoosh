import Foundation

/// The accessibility ids and key chords this lane answers (docs/CONTROL-SOCKET.md naming:
/// `<surface>.<region>[.<element>...]`).
public enum QuickReplyIds {
    public static let surface = "mac-reader"
    /// The surface a window shows this screen on by itself (`shot mac-quickreply`), through the shell's surface scene.
    public static let shotSurface = "mac-quickreply"
    public static let field = "mac-main.quickreply.field"
    public static let send = "mac-main.quickreply.send"
    public static let from = "mac-main.quickreply.from"
    public static let fromMenu = "mac-main.quickreply.from.menu"
    public static func fromItem(_ i: Int) -> String { "mac-main.quickreply.from.\(i)" }
    public static let emoji = "mac-main.quickreply.emoji"
    public static let emojiPicker = "mac-main.quickreply.emoji.picker"
    public static func emojiItem(_ i: Int) -> String { "mac-main.quickreply.emoji.\(i)" }
    public static let undoBar = "mac-main.undo.bar"
    public static let undo = "mac-main.undo.bar.undo"
    public static let retry = "mac-main.reply.retry"
    /// Round 5 (board cards-s3): the floating slab, its header line and the rows the header opens.
    public static let slab = "mac-main.quickreply.slab"
    public static let header = "mac-main.quickreply.header"
    public static let headerToggle = "mac-main.quickreply.header.toggle"
    public static let popout = "mac-main.quickreply.popout"
    public static let headerToRegion = "mac-main.quickreply.header.to"
    public static func headerTo(_ i: Int) -> String { "mac-main.quickreply.header.to.\(i)" }
    public static func headerToRemove(_ i: Int) -> String { "mac-main.quickreply.header.to.\(i).remove" }
    public static let headerCc = "mac-main.quickreply.header.cc"
    public static let headerBcc = "mac-main.quickreply.header.bcc"
    public static let headerSubject = "mac-main.quickreply.header.subject"
    /// The people list under the Cc or Cci line (`field` is `cc` or `bcc`), its lines, and the sentence of a send that did not start.
    public static func headerOffers(_ field: String) -> String { "mac-main.quickreply.header.\(field).completions" }
    public static func headerOffer(_ field: String, _ i: Int) -> String { "mac-main.quickreply.header.\(field).completions.\(i)" }
    public static let headerRefusal = "mac-main.quickreply.header.refusal"
    /// The From picker's filter field and its foot line of keys.
    public static let fromFilter = "mac-main.quickreply.from.filter"
    public static let fromMore = "mac-main.quickreply.from.more"

    /// An id of this bar as drawn on a surface: the reader's bar keeps the `mac-main` ids; the notch's compact bar
    /// carries `mac-notch` ones, so the two never write one frame entry (the sendwave lane's finding).
    public static func on(_ compact: Bool, _ id: String) -> String {
        compact ? "mac-notch" + id.dropFirst("mac-main".count) : id
    }
}

/// The quick reactions the emoji button offers, in order; the system palette (⌃⌘Space) carries the rest.
public enum QuickReplyEmoji {
    public static let set = ["👍", "🙏", "🎉", "😊", "👀", "✅", "❤️", "🚀"]
}
