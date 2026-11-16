import Foundation

/// How a mail is drawn (owner pick B3, A129): a WRITTEN mail (a person writing, even from Outlook) is drawn in the app's colours
/// and type; a DESIGNED mail (its own backgrounds, tables, pictures) keeps its sender's colours on a white sheet.
public enum MailLayout: String, Equatable, Sendable {
    case designed, written
}

/// Which colours the open mail shows now. A mail starts in its layout's way; ⇧I switches it to the other for this viewing.
public enum BodyColours: String, Equatable, Sendable {
    case app, original
    var flipped: BodyColours { self == .app ? .original : .app }
    static func start(for layout: MailLayout) -> BodyColours { layout == .designed ? .original : .app }
}

/// The tag scan for a body the engine did not prepare (an old stored mail, the drive samples): each blocked picture is a `cid:whoosh-blocked` image; a pixel is one drawn 0 to 2 points wide or tall.
enum MessageBodyPictures {
    static func counts(html: String?) -> (blocked: Int, pixels: Int) {
        guard let html, html.contains("whoosh-blocked") else { return (0, 0) }
        let range = NSRange(html.startIndex..., in: html)
        let tags = blockedTag.matches(in: html, range: range)
        let pixels = tags.filter { m in
            guard let r = Range(m.range, in: html) else { return false }
            return isPixel(String(html[r]))
        }.count
        return (tags.count, pixels)
    }

    /// A tracking pixel: an image tag drawn 0 to 2 points wide or tall. Never fetched, even after Load once.
    static func isPixel(_ tag: String) -> Bool { pixelSize.firstMatch(in: tag, range: NSRange(tag.startIndex..., in: tag)) != nil }

    /// Pictures the page can ask a server for: blocked, not a pixel, with a kept http or https address (`data-whoosh-src`).
    static func askable(html: String?) -> Int {
        guard let html, html.contains("whoosh-blocked") else { return 0 }
        return blockedTag.matches(in: html, range: NSRange(html.startIndex..., in: html)).filter { m in
            guard let r = Range(m.range, in: html) else { return false }
            let tag = String(html[r])
            return !isPixel(tag) && keptAddress.firstMatch(in: tag, range: NSRange(tag.startIndex..., in: tag)) != nil
        }.count
    }

    private static let keptAddress = try! NSRegularExpression(pattern: "data-whoosh-src\\s*=\\s*[\"']https?://", options: .caseInsensitive)
    private static let blockedTag = try! NSRegularExpression(pattern: "<img\\b[^>]*whoosh-blocked[^>]*>", options: .caseInsensitive)
    private static let pixelSize = try! NSRegularExpression(pattern: "\\s(?:width|height)\\s*=\\s*[\"']?[0-2](?![0-9])", options: .caseInsensitive)
}

/// Web pictures of an opened mail (owner 13 Nov 2026, pictures 213 and 214: "lots of picture not loaded or difficult to load"; 237 of his mails held web
/// pictures, from 72 senders, and each waited for a click): they load when the mail is opened, unless the owner picked "ask first" (Settings > General >
/// Privacy) or the mail is read in Junk. A tracking pixel is never fetched in either case, and nothing is fetched before the mail is opened. Kept locally
/// in UserDefaults, like `PicturesAlways`.
enum PicturesPolicy {
    static let key = "whoosh.messagebody.pictures.onOpen"
    /// The hidden test copy, read as `HiddenRig.isOn` reads it (that flag is tied to the main thread; this rule is read from any).
    private static let onTestCopy = ProcessInfo.processInfo.environment["WHOOSH_RIG_HIDDEN"] == "1" || CommandLine.arguments.contains("--rig-hidden")
    /// Unset: on. A test copy starts with "ask first": it may ask no server off this Mac, so every fixture mail would say that its pictures did not come.
    static var loadsOnOpen: Bool {
        get { (UserDefaults.standard.object(forKey: key) as? Bool) ?? !onTestCopy }
        set { UserDefaults.standard.set(newValue, forKey: key) }
    }
}

/// A test copy's broken copy for the check of the first height (`WHOOSH_BODY_MUTANT=unfold-open`): a measure of a page still on its way counts as the first
/// measure again, so the real height unfolds at the open. Never on the owner's copy.
enum BodyOpenMutant {
    static let unfolds = (ProcessInfo.processInfo.environment["WHOOSH_RIG_HIDDEN"] == "1" || CommandLine.arguments.contains("--rig-hidden"))
        && ProcessInfo.processInfo.environment["WHOOSH_BODY_MUTANT"] == "unfold-open"
}

/// "Always for <sender address>" (A129 part 3): the senders whose pictures the owner asked to load every time. Kept LOCALLY in
/// UserDefaults until the contract lane gives it a command and a state path (proposed: command `SetSenderPictures { address, always }`,
/// state `mail/sender-pictures/<address>`). Addresses are stored lower-cased; nothing here leaves this Mac.
enum PicturesAlways {
    static let key = "whoosh.messagebody.pictures.always"
    static func contains(_ address: String) -> Bool {
        let a = address.lowercased()
        return !a.isEmpty && (UserDefaults.standard.stringArray(forKey: key) ?? []).contains(a)
    }
    static func add(_ address: String) {
        let a = address.lowercased()
        guard !a.isEmpty else { return }
        var all = UserDefaults.standard.stringArray(forKey: key) ?? []
        if !all.contains(a) { all.append(a); UserDefaults.standard.set(all, forKey: key) }
    }
    static func remove(_ address: String) {
        let a = address.lowercased()
        UserDefaults.standard.set((UserDefaults.standard.stringArray(forKey: key) ?? []).filter { $0 != a }, forKey: key)
    }
}
