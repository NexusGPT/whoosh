import Foundation
import Testing
#if os(macOS)
import AppKit
#endif
@testable import WhooshUI

// The socket's `scroll` verb (Sources/ControlSocket/WindowVerbs.swift, `ScrollVerb`): the shots sheet reaches the end of a long page with it.
// The pure rule is tested on numbers; the scroll itself is driven on a real NSScrollView holding a document taller than its clip.
// Broken copies (each turns the named test red):
//   1. `case "end": return far` -> `case "end": return 0`                          : theEndRuleStopsAtTheLastOffset, scrollEndMovesToTheBottomAndSaysEnd
//   2. `let y = flipped ? to : far - to` -> `let y = to`                            : aNonFlippedDocumentScrollsFromItsTopToo
//   3. `over($0) > slack` -> `over($0) > -1000` (a view with nothing to move)        : aPageThatFitsIsNotScrollable
//   4. `ScrollVerb.commands.contains(a[0])` -> `true`                                : theVerbRefusesAWordThatIsNoCommandAndAWindowThatIsNotOpen

@Test func theEndRuleStopsAtTheLastOffset() {
    #expect(ScrollVerb.target("top", offset: 300, content: 1000, view: 400) == 0)
    #expect(ScrollVerb.target("end", offset: 0, content: 1000, view: 400) == 600)
    #expect(ScrollVerb.target("page-down", offset: 0, content: 1000, view: 400) == 360)
    #expect(ScrollVerb.target("page-down", offset: 500, content: 1000, view: 400) == 600, "a page never goes past the end")
    #expect(ScrollVerb.target("page-up", offset: 100, content: 1000, view: 400) == 0, "a page never goes above the top")
    #expect(ScrollVerb.target("end", offset: 0, content: 300, view: 400) == 0, "a page that fits has its end at its top")
    #expect(ScrollVerb.target("sideways", offset: 0, content: 1000, view: 400) == nil)
}

#if os(macOS)
private final class FlipDoc: NSView {
    var flip = true
    override var isFlipped: Bool { flip }
}

@MainActor private func scroller(flipped: Bool, content: CGFloat = 1000, view: CGFloat = 400) -> (NSView, NSScrollView) {
    let root = NSView(frame: NSRect(x: 0, y: 0, width: 300, height: view))
    let sv = NSScrollView(frame: root.bounds)
    let doc = FlipDoc(frame: NSRect(x: 0, y: 0, width: 300, height: content))
    doc.flip = flipped
    sv.documentView = doc
    root.addSubview(sv)
    sv.layoutSubtreeIfNeeded()
    if !flipped { sv.contentView.scroll(to: NSPoint(x: 0, y: max(0, content - view))); sv.reflectScrolledClipView(sv.contentView) }   // a non-flipped document starts at its top
    return (root, sv)
}

@MainActor @Test func scrollEndMovesToTheBottomAndSaysEnd() throws {
    let (root, sv) = scroller(flipped: true)
    let r = try ScrollVerb.run("end", in: root)
    #expect(r["scrollable"] as? Bool == true)
    #expect(r["at"] as? String == "end")
    #expect(r["offset"] as? Double == 600 && r["max"] as? Double == 600)
    #expect(sv.contentView.bounds.origin.y == 600, "the clip really moved: \(sv.contentView.bounds.origin.y)")
    let back = try ScrollVerb.run("top", in: root)
    #expect(back["at"] as? String == "top" && sv.contentView.bounds.origin.y == 0)
    let mid = try ScrollVerb.run("page-down", in: root)
    #expect(mid["at"] as? String == "middle" && mid["offset"] as? Double == 360)
}

@MainActor @Test func aNonFlippedDocumentScrollsFromItsTopToo() throws {
    let (root, sv) = scroller(flipped: false)
    let top = try ScrollVerb.run("top", in: root)
    #expect(top["offset"] as? Double == 0 && sv.contentView.bounds.origin.y == 600, "top of a bottom-up document is its highest y")
    let end = try ScrollVerb.run("end", in: root)
    #expect(end["at"] as? String == "end" && sv.contentView.bounds.origin.y == 0)
}

@MainActor @Test func aPageThatFitsIsNotScrollable() throws {
    let (root, _) = scroller(flipped: true, content: 300, view: 400)
    let r = try ScrollVerb.run("end", in: root)
    #expect(r["scrollable"] as? Bool == false && r["offset"] as? Double == 0)
}

@MainActor @Test func theLargestScrollViewThatHasMoreToShowIsTheOneMoved() throws {
    let (root, big) = scroller(flipped: true)
    let side = NSScrollView(frame: NSRect(x: 0, y: 0, width: 40, height: 100))
    side.documentView = FlipDoc(frame: NSRect(x: 0, y: 0, width: 40, height: 5000))
    root.addSubview(side)
    _ = try ScrollVerb.run("end", in: root)
    #expect(big.contentView.bounds.origin.y == 600 && side.contentView.bounds.origin.y == 0, "the small side list stays where it was")
}
#endif

@MainActor @Test func theVerbRefusesAWordThatIsNoCommandAndAWindowThatIsNotOpen() {
    let bad = ControlVerbs.handle(#"{"verb":"scroll","args":["sideways"]}"#)
    #expect(bad.contains("\"ok\":false") && bad.contains("usage: scroll top|end|page-down|page-up [window:<id>]"), "\(bad)")
    let none = ControlVerbs.handle(#"{"verb":"scroll","args":["end","window:no-such-window"]}"#)
    #expect(none.contains("\"ok\":false") && none.contains("no open window has the id no-such-window"), "\(none)")
    let listed = ControlVerbs.handle(#"{"verb":"verbs","args":[]}"#)
    #expect(listed.contains("\"scroll\""), "the registry lists scroll: \(listed)")
}
