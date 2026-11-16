import Foundation

/// The rich text fixture verbs, served by the control socket under `--fixture-core` (docs/CONTROL-SOCKET.md, item 52).
///   fixture format bold <text>                 the draft becomes that one bold paragraph
///   fixture format list <item> [item...]       the draft becomes a list, one item each
///   fixture format link <text> <href>          the draft becomes that text linked to the address
///   fixture format picture <cid>               the draft becomes one inline picture with that content id
///   fixture reload-draft                       the draft is written to its MIME HTML part and read back from it
///   fixture paste-html <html>                  the HTML is pasted: parsed to the composer's own set, so no source style or script survives
/// Each acts on the one shared session (`RichTextFeature.session`), the same one the composer on screen edits, and the
/// socket reads the result back through `rows mac-composer.runs` and `core state draft/current/mime`.
@MainActor
public enum RichTextFixture {
    public static let verbs: Set<String> = ["format", "reload-draft", "paste-html"]

    /// `fixture reset` promises a clean screen (FixtureEvents.reset: typed text cleared, windows closed, the screens clear their
    /// own state on the notification). The composer keeps its draft across a close by design (Escape), so without this the
    /// words one check left in the shared session were still in the next check's composer. The reset notification is posted
    /// synchronously on the main thread, so the session is empty before the next socket line runs.
    public static func emptyOnReset(_ session: RichTextSession) {
        NotificationCenter.default.addObserver(forName: FixtureEvents.name("reset"), object: nil, queue: nil) { _ in
            if Thread.isMainThread { MainActor.assumeIsolated { session.reset() } }
            else { DispatchQueue.main.sync { MainActor.assumeIsolated { session.reset() } } }
        }
    }

    /// What a verb refused, in words the socket sends back.
    public struct Refusal: Error { public let words: String }

    @discardableResult
    public static func run(_ verb: String, _ args: [String], session: RichTextSession = RichTextFeature.session) throws -> [String: Any] {
        switch (verb, args.first, args.count) {
        case ("format", "bold", 2): session.setBold(args[1])
        case ("format", "list", 2...): session.setList(Array(args.dropFirst()))
        case ("format", "link", 3): session.setLink(args[1], href: args[2])
        case ("format", "picture", 2): session.setPicture(cid: args[1])
        case ("format", _, _):
            throw Refusal(words: "usage: fixture format bold <text> | list <item> [item...] | link <text> <href> | picture <cid>")
        case ("reload-draft", _, 0): session.load(html: session.mimeHTML)
        case ("reload-draft", _, _): throw Refusal(words: "usage: fixture reload-draft")
        case ("paste-html", _, 1): session.paste(html: args[0])
        case ("paste-html", _, _): throw Refusal(words: "usage: fixture paste-html <html>")
        default: throw Refusal(words: "fixture \(verb) is not a rich text verb")
        }
        return ["verb": verb, "paragraphs": session.document.paragraphs.count]
    }

    /// `core state draft/current/<field>`: the draft on screen, read from the shared session. `mime` is the HTML part,
    /// `text` the plain words. Any other field has no value. Safe from any thread: the session lives on the main actor.
    public nonisolated static func draftValue(_ field: String) -> String? {
        func read() -> String? {
            MainActor.assumeIsolated {
                switch field {
                case "mime": return RichTextFeature.session.mimeHTML
                case "text": return RichTextFeature.session.document.plainText
                default: return nil
                }
            }
        }
        return Thread.isMainThread ? read() : DispatchQueue.main.sync(execute: read)
    }
}
