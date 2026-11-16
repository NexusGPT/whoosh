import Foundation

/// The guide's steps as data (Sources/Resources/EntraGuide/entraguide-steps.json): geometry as fractions of each picture, every word by
/// its catalogue key `entra-guide.*`. One step is one click the admin makes; the seven chapters of the board are only the heads.
struct EntraGuidePlate: Decodable, Sendable {
    let id: String
    let file: String?
    let drawn: String?
    let w: Double
    let h: Double
    let location: String
    var isDrawn: Bool { drawn != nil }
}

struct EntraGuideValue: Decodable, Sendable {
    let label: String
    let value: String
    let copy: Bool
    let steps: [Int]
    /// A98: a row whose value is built when the guide opens, not typed here ("consent-link" = the admin-consent link for the mailbox's domain)
    let dynamic: String?
}

struct EntraGuideStep: Decodable, Sendable {
    let n: Int
    let chapter: Int
    let plate: String
    let focus: [Double]
    let ring: [Double]
    let cover: [[Double]]?
    let group: String?
    let hint: Bool?
    let primary: Bool?
    let slot: Bool?
    let paste: Bool?
    /// A98: the sentence names the mailbox's organisation (its one %@ is the domain)
    let consent: Bool?

    /// The sentence; the consent step fills in the mailbox's domain ("your mailbox's domain" when the guide opened with no address).
    func say(domain: String) -> String {
        let key = String(format: "entra-guide.s%02d.say", n)
        return consent == true ? Words.text(key, EntraGuideWords.consentDomain(domain)) : Words.text(key)
    }
    var cap: String { Words.text(String(format: "entra-guide.s%02d.cap", n)) }
    var hintText: String? { hint == true ? Words.text(String(format: "entra-guide.s%02d.hint", n)) : nil }
    var chapterTitle: String { Words.text("entra-guide.c\(chapter).title") }
    var wantsId: Bool { slot == true || paste == true }
    /// what a missing data file leaves: no step, so every control does nothing
    static let blank = EntraGuideStep(n: 0, chapter: 1, plate: "", focus: [0, 0, 1, 1], ring: [0, 0, 0, 0], cover: nil, group: nil, hint: nil, primary: nil, slot: nil, paste: nil, consent: nil)
}

struct EntraGuideData: Decodable, Sendable {
    let chapters: Int
    let plates: [EntraGuidePlate]
    let groups: [String: [EntraGuideValue]]
    let steps: [EntraGuideStep]

    static let shared: EntraGuideData = load()

    func plate(_ id: String) -> EntraGuidePlate? { plates.first { $0.id == id } }
    func values(of step: EntraGuideStep) -> [EntraGuideValue] { step.group.flatMap { groups[$0] } ?? [] }

    /// A bundled resource: `.process` may keep the folder or flatten it, so both places are looked in.
    static func url(_ name: String, _ ext: String) -> URL? {
        Bundle.module.url(forResource: name, withExtension: ext, subdirectory: "EntraGuide") ?? Bundle.module.url(forResource: name, withExtension: ext)
    }

    /// A missing or broken file gives an empty guide; the `state` row then reads zero steps and a check goes red.
    private static func load() -> EntraGuideData {
        guard let u = url("entraguide-steps", "json"), let d = try? Data(contentsOf: u),
              let v = try? JSONDecoder().decode(EntraGuideData.self, from: d) else {
            return EntraGuideData(chapters: 0, plates: [], groups: [:], steps: [])
        }
        return v
    }
}
