import Foundation
import Observation

/// One person the reply goes to: the name the core knows and the address that is sent.
public struct ReplyRecipient: Equatable, Sendable, Identifiable {
    public var id: String { address }
    public let name: String
    public let address: String
}

/// The reply box's header (board cards-s3; owner 22:03 "same for reply"): compact, it is one line, "Répondre à <x>"
/// while the field is empty and "Brouillon à <x>" once there are words; open (chevron or ⇧H) it shows the To chips,
/// Cc, Cci and Objet rows. Everything here is the header's own state; the words go out with the send.
@MainActor @Observable
public final class QuickReplyHeaderState {
    public enum Field: Equatable { case none, cc, bcc, subject }
    public private(set) var open = false
    public private(set) var to: [ReplyRecipient] = []
    public var cc = ""
    public var bcc = ""
    public var subject = ""
    /// The header row the socket's `type` lands in while the header is open (click a row to pick it).
    public var focus: Field = .none
    /// What the thread gives a plain reply: compare against it to know whether the send needs a draft of its own.
    private var defaultTo: [ReplyRecipient] = []
    private var defaultSubject = ""

    /// Read once when the field opens on a thread: To is the person answered, Objet is "Re: <subject>".
    func load(to: [ReplyRecipient], subject: String) {
        defaultTo = to; defaultSubject = subject
        self.to = to; self.subject = subject
        cc = ""; bcc = ""; focus = .none
        refusal = nil; offerIndex = 0
    }

    func reset() { open = false; focus = .none; load(to: defaultTo, subject: defaultSubject) }

    /// A reopened draft brings its own header back (the thread's defaults stay, so a Cc it carries still counts as
    /// an edited header and sends through the draft).
    func restore(to: [ReplyRecipient], cc: [String], bcc: [String], subject: String) {
        self.to = to.isEmpty ? defaultTo : to
        self.cc = cc.joined(separator: ", "); self.bcc = bcc.joined(separator: ", ")
        self.subject = subject.isEmpty ? defaultSubject : subject
        focus = .none
    }

    func toggle() { open.toggle(); if !open { focus = .none } }

    /// × on a chip: the person leaves the To line (the send then goes to whoever is left).
    func remove(at i: Int) -> Bool {
        guard to.indices.contains(i) else { return false }
        to.remove(at: i)
        return true
    }

    /// An address typed into Cc or Cci and confirmed with a comma, a space or Return becomes part of that line.
    func addresses(_ line: String) -> [String] {
        line.split(whereSeparator: { ", ;\n".contains($0) }).map { String($0) }.filter { $0.contains("@") }
    }

    // MARK: the people offered under Cc and Cci, and the word that is not an address

    /// The highlighted line of the people list under Cc or Cci.
    public var offerIndex = 0
    /// Why a send did not start (a word on Cc or Cci that is not an address); shown under the rows until those lines change.
    public var refusal: String?
    @ObservationIgnored private var offerKept: (key: String, list: [ComposerSuggestion])?

    /// The word being typed on a Cc or Cci line: what stands after its last comma or semicolon.
    static func lastWord(_ line: String) -> String {
        String(line.split(omittingEmptySubsequences: false, whereSeparator: { ",;\n".contains($0) }).last ?? "").trimmingCharacters(in: .whitespaces)
    }

    /// The people offered for the word being typed in Cc or Cci: the composer's own list (the people of all mail by first letters, those the owner
    /// wrote to first), without the addresses the reply already goes to. A whole address needs no list. Lead, 13 Nov 2026: these lines offered nobody.
    func offered(_ field: Field, core: WhooshCore) -> [ComposerSuggestion] {
        guard field == .cc || field == .bcc else { return [] }
        let word = Self.lastWord(field == .cc ? cc : bcc)
        guard !word.isEmpty, !ComposerModel.isAddress(word) else { return [] }
        let taken = Set((to.map(\.address) + addresses(cc) + addresses(bcc)).map { $0.lowercased() })
        let key = "\(field)|\(word)|\(taken.sorted().joined(separator: ","))"
        if let kept = offerKept, kept.key == key { return kept.list }
        let list = Array(ComposerContext.suggestions(for: word, replyTo: nil, core: core).filter { !taken.contains($0.address.lowercased()) }.prefix(5))
        offerKept = (key, list)
        return list
    }

    /// ↑ ↓ on a Cc or Cci line while its list shows. False when there is no list.
    func moveOffer(_ delta: Int, _ field: Field, core: WhooshCore) -> Bool {
        let n = offered(field, core: core).count
        guard n > 0 else { return false }
        offerIndex = min(max(offerIndex + delta, 0), n - 1)
        return true
    }

    /// ↩, ⇥ or a click on a line: the person's address takes the place of the word being typed, with a comma after it for the next one.
    func takeOffer(_ index: Int? = nil, _ field: Field, core: WhooshCore) -> Bool {
        let list = offered(field, core: core)
        let i = index ?? offerIndex
        guard list.indices.contains(i) else { return false }
        let line = field == .cc ? cc : bcc
        var parts = line.split(omittingEmptySubsequences: false, whereSeparator: { ",;\n".contains($0) }).map { $0.trimmingCharacters(in: .whitespaces) }
        if !parts.isEmpty { parts.removeLast() }
        parts.append(list[i].address)
        let next = parts.filter { !$0.isEmpty }.joined(separator: ", ") + ", "
        if field == .cc { cc = next } else { bcc = next }
        offerIndex = 0
        return true
    }

    /// The first word of Cc or Cci that is not an address ("karen"): the send does not start with it on the line.
    var strayWord: String? {
        (cc + "," + bcc).split(whereSeparator: { ", ;\n".contains($0) }).map(String.init).first { !$0.contains("@") }
    }

    /// The send stops on a word that is not an address: the rows open and say which word.
    func refuse(_ word: String) {
        open = true
        refusal = ComposerWords.notAnAddress(word)
    }

    /// true when the send must carry more than the plain reply does: another To set, a Cc, a Cci or a changed subject.
    var isExpandedSend: Bool {
        to != defaultTo || !addresses(cc).isEmpty || !addresses(bcc).isEmpty
            || subject.trimmingCharacters(in: .whitespaces) != defaultSubject.trimmingCharacters(in: .whitespaces)
    }

    /// The socket's `type` while a header row has the focus; false hands the text back to the field.
    func type(_ text: String) -> Bool {
        switch focus {
        case .none: return false
        case .cc: cc += text
        case .bcc: bcc += text
        case .subject: subject += text
        }
        return true
    }
}
