import Foundation
#if canImport(UIKit)
import UIKit
#elseif canImport(AppKit)
import AppKit
#endif

/// The clipboard, written once per copy. The code stays on this device (`localOnly`: no Universal Clipboard to the Mac).
/// `text` answers only with what this screen wrote, and only while it is still what the clipboard holds, so a check can
/// read it without reading another app's content (no paste prompt).
@MainActor
enum ThreadClipboard {
    private static var written: (text: String, count: Int)?
    #if canImport(AppKit) && !canImport(UIKit)
    /// The board the code goes to. A unit test sets a private board, so a test run never writes over the owner's clipboard.
    static var board = NSPasteboard.general { didSet { written = nil } }
    #endif

    static func put(_ text: String) {
        #if canImport(UIKit)
        // "public.utf8-plain-text" is UTType.utf8PlainText's identifier.
        UIPasteboard.general.setItems([["public.utf8-plain-text": text]], options: [.localOnly: true])
        written = (text, UIPasteboard.general.changeCount)
        #elseif canImport(AppKit)
        board.clearContents()
        board.setString(text, forType: .string)
        written = (text, board.changeCount)
        #endif
    }

    static var text: String? {
        guard let w = written else { return nil }
        #if canImport(UIKit)
        return UIPasteboard.general.changeCount == w.count ? w.text : nil
        #elseif canImport(AppKit)
        return board.changeCount == w.count ? w.text : nil
        #else
        return nil
        #endif
    }
}
