#if os(macOS)
import AppKit

/// The clipboard on a hidden rig (17 Nov 2026). A hidden test copy that copies (the notch's code pill and Copy, the reader's code, ⌘C in
/// a field) wrote the Mac's own clipboard: the owner copies a text, a hidden run clicks Copy, and the owner's next paste gives the test's
/// sign-in code. On a hidden rig the general pasteboard IS a private named one, for this app's code and for AppKit's own text views
/// alike: `NSPasteboard.general` and `NSPasteboard(name: .general)` both answer the board named `HiddenRig.clipboardName`. A check reads
/// and writes that name from outside (checks/parity/par-notch.py, journeys/J6.py) and proves the Mac's own clipboard kept its change count.
@MainActor
public extension HiddenRig {
    /// The private board's name; the checks carry the same word.
    nonisolated static let clipboardName = "io.nexus.whoosh.rig.clipboard"

    internal static func installPrivateClipboard() {
        guard let meta = object_getClass(NSPasteboard.self) else { return }
        let byName = NSSelectorFromString("pasteboardWithName:")
        guard let m = class_getInstanceMethod(meta, byName) else { return }
        typealias Orig = @convention(c) (AnyObject, Selector, NSString) -> AnyObject
        let orig = unsafeBitCast(method_getImplementation(m), to: Orig.self)
        let general = NSPasteboard.Name.general.rawValue
        let mine = clipboardName as NSString
        let named: @convention(block) (AnyObject, NSString) -> AnyObject = { cls, name in
            orig(cls, byName, (name as String) == general ? mine : name)
        }
        method_setImplementation(m, imp_implementationWithBlock(named))
        hooked.append("NSPasteboard +pasteboardWithName: (general is private)")
        let gen = NSSelectorFromString("generalPasteboard")
        guard let g = class_getInstanceMethod(meta, gen) else { return }
        let own: @convention(block) (AnyObject) -> AnyObject = { cls in orig(cls, byName, mine) }
        method_setImplementation(g, imp_implementationWithBlock(own))
        hooked.append("NSPasteboard +generalPasteboard (private)")
    }
}
#endif
