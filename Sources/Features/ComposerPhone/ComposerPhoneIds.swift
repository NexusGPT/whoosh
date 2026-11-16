import Foundation

/// Every ax id of the iPhone composer, in one place. Ids follow `<surface>.<region>[.<element>...][.<index>]`
/// (checks/composerphone-check.py reads exactly these).
enum ComposerPhoneIds {
    static let surface = "iphone-composer"
    static let header = "iphone-composer.header"
    static let cancel = "iphone-composer.cancel"
    static let send = "iphone-composer.send"
    static let to = "iphone-composer.to"
    static let ccToggle = "iphone-composer.cc.show"
    static let cc = "iphone-composer.cc"
    static let bcc = "iphone-composer.bcc"
    static let from = "iphone-composer.from"
    static let fromChoices = "iphone-composer.from.choices"
    static let subject = "iphone-composer.subject"
    static let body = "iphone-composer.body"
    static let quoted = "iphone-composer.quoted"
    static let quotedToggle = "iphone-composer.quoted.toggle"
    static let line = "iphone-composer.line"
    static let attach = "iphone-composer.attach"
    static let dictate = "iphone-composer.dictate"
    static let speechModel = "iphone-composer.speech-model"
    static let undo = "iphone-composer.undo"
    static let undoCancel = "iphone-composer.undo.cancel"
    static let completions = "iphone-composer.to.completions"
    static func fromChoice(_ i: Int) -> String { "iphone-composer.from.\(i)" }
    static func completion(_ i: Int) -> String { "iphone-composer.to.completions.\(i)" }
    static func chipRemove(_ field: String, _ i: Int) -> String { "iphone-composer.\(field).chip.\(i).remove" }
}
