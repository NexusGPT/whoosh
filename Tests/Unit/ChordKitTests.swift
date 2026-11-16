import Foundation
import Testing
@testable import WhooshUI

#if os(macOS)
import AppKit

// SPEC A99 (owner, 2026-11-09 23:25): "cmd+2, cmd+3, etc. don't work on my azerty keyboard". ChordKit.chord reads the digit row by
// physical key (key codes 18 to 29), not by the character the layout types. Every event below is a real NSEvent shaped as the
// hardware sends it. The expected chord text is typed here from the SPEC, never read back from ChordKit or KeyCodeVerb.
// MUTANTS: each test names, in its trailing comment, the mutation of ChordKit.chord (or KeyCodeVerb) that must turn it red.
// Not run in the round that wrote them (no build); the mutants are stated, not yet seen red.

private func event(_ code: UInt16, typed: String, _ mods: NSEvent.ModifierFlags = []) throws -> NSEvent {
    try #require(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: mods, timestamp: 0, windowNumber: 0, context: nil,
                                  characters: typed, charactersIgnoringModifiers: typed, isARepeat: false, keyCode: code))
}

/// The digit row, key code by key code: what a French AZERTY keyboard types unshifted, and the digit the chord must name.
private let azertyDigitRow: [(code: UInt16, typed: String, digit: String)] = [
    (18, "&", "1"), (19, "é", "2"), (20, "\"", "3"), (21, "'", "4"), (23, "(", "5"),
    (22, "§", "6"), (26, "è", "7"), (28, "!", "8"), (25, "ç", "9"), (29, "à", "0"),
]

@Test func anAzertyTwoKeyUnderCommandIsCommandTwo() throws {
    let e = try event(19, typed: "é", [.command])
    #expect(ChordKit.chord(e) == "⌘2")                      // mutant: the chord is read from charactersIgnoringModifiers ("⌘É")
}

@Test func everyAzertyDigitKeyUnderCommandNamesItsDigit() throws {
    #expect(azertyDigitRow.count == 10)                     // control: the table holds the whole row, so the loop below cannot be empty
    for k in azertyDigitRow {
        #expect(ChordKit.chord(try event(k.code, typed: k.typed, [.command])) == "⌘" + k.digit, "key code \(k.code) typing \(k.typed)")   // mutant: any one code dropped from the digit table
    }
}

@Test func aQwertyTwoKeyUnderCommandIsStillCommandTwo() throws {
    #expect(ChordKit.chord(try event(19, typed: "2", [.command])) == "⌘2")      // mutant: the digit table is keyed by character, so a QWERTY digit stops matching
    #expect(ChordKit.chord(try event(29, typed: "0", [.command])) == "⌘0")      // mutant: the last code (29) is left out
}

@Test func controlAndShiftCommandDigitsAreReadByKeyToo() throws {
    #expect(ChordKit.chord(try event(19, typed: "é", [.control])) == "⌃2")             // account switch on AZERTY; mutant: only ⌘ is read by key
    #expect(ChordKit.chord(try event(26, typed: "è", [.shift, .command])) == "⇧⌘7")   // the numbered-list chord; mutant: Shift plus another modifier is read by character
    #expect(ChordKit.chord(try event(19, typed: "é", [.command, .option])) == "⌥⌘2")  // modifier order is ⌃⌥⇧⌘
}

@Test func aShiftOnlyChordKeepsItsTypedCharacter() throws {
    #expect(ChordKit.chord(try event(32, typed: "U", [.shift])) == "⇧U")               // mutant: Shift alone is read by key (a letter has no digit: the chord empties)
    // the QWERTY "1" key under Shift types "!": v8 binds ⇧! (Spam), so the chord keeps the character and is never "⇧1"
    #expect(ChordKit.chord(try event(18, typed: "!", [.shift])) == "⇧!")                // mutant: Shift alone is read by the digit row ("⇧1")
    #expect(ChordKit.chord(try event(20, typed: "#", [.shift])) == "⇧#")                // ⇧# is Trash; same mutant
}

@Test func aDigitRowKeyWithNoModifierIsTypedTextNotAChord() throws {
    #expect(ChordKit.chord(try event(19, typed: "é")) == "É")                           // mutant: an unmodified digit-row key is read by key ("2")
    #expect(ChordKit.chord(try event(19, typed: "2")) == "2")                           // the QWERTY key typed plain stays its character
}

@Test func keysOutsideTheDigitRowAreUnchanged() throws {
    #expect(ChordKit.chord(try event(40, typed: "k", [.command])) == "⌘K")             // mutant: the digit table swallows other codes
    #expect(ChordKit.chord(try event(36, typed: "\r", [.command])) == "⌘Enter")        // a named key keeps its name and modifiers
    #expect(ChordKit.chord(try event(125, typed: "", [.command])) == "⌘Down")
}

// MARK: the socket's key-code variant builds the same events

@MainActor @Test func theKeyCodeVerbBuildsTheAzertyEventAndChordKitReadsIt() throws {
    let e = try KeyCodeVerb.event(["code:19", "mods:cmd"])
    #expect(e.keyCode == 19)
    #expect(e.characters == "é")                            // the verb types what an AZERTY keyboard types; mutant: the verb defaults to QWERTY
    #expect(ChordKit.chord(e) == "⌘2")
    let q = try KeyCodeVerb.event(["code:19", "mods:cmd", "layout:qwerty"])
    #expect(q.characters == "2")                            // control: the layouts differ, so the AZERTY case above is not the QWERTY one
    #expect(ChordKit.chord(q) == "⌘2")
}

@MainActor @Test func theKeyCodeVerbBuildsEveryDigitChordTheSourceBinds() throws {
    for k in azertyDigitRow {
        let e = try KeyCodeVerb.event(["code:\(k.code)", "mods:ctrl+cmd"])
        #expect(ChordKit.chord(e) == "⌃⌘" + k.digit, "code \(k.code)")                  // mutant: the verb's mods:ctrl is dropped
    }
    let shifted = try KeyCodeVerb.event(["code:18", "mods:shift"])
    #expect(shifted.characters == "1")                      // Shift on the digit row types the digit on both layouts
    #expect(ChordKit.chord(shifted) == "⇧1")                // Shift alone keeps the typed character
}

@MainActor @Test func theKeyCodeVerbRefusesGarbageInWords() {
    func words(_ args: [String]) -> String? {
        do { _ = try KeyCodeVerb.event(args); return nil } catch let f as ControlFailure { return f.words } catch { return "other error" }
    }
    #expect(words(["mods:cmd"]) == "key code needs code:<n>")                           // mutant: a missing code is read as key 0
    #expect(words(["code:abc"]) == "key code: \"abc\" is not a key code")               // coercion bait
    #expect(words(["code:-1"]) == "key code: \"-1\" is not a key code")                 // negative
    #expect(words(["code:99999"]) == "key code: \"99999\" is not a key code")           // out of the UInt16 range
    #expect(words(["code:19", "mods:hyper"]) == "key code: the modifier \"hyper\" is not cmd, shift, ctrl or opt")
    #expect(words(["code:19", "layout:dvorak"]) == "key code: the layout \"dvorak\" is not azerty or qwerty")
    #expect(words(["code:19", "bogus"]) == "key code: \"bogus\" is not name:value")
    #expect(words(["code:40", "mods:cmd"]) == "key code 40 is not on the digit row: pass char:<c>")   // a letter key needs its character
    #expect(words(["code:40", "mods:cmd", "char:k"]) == nil)                           // positive control: with its character it builds
    #expect(words(["code:19", "mods:cmd"]) == nil)                                      // positive control: the probe is not refusing everything
}

@MainActor @Test func theSameKeyCodeTwiceResolvesTheSameChord() throws {
    let a = ChordKit.chord(try KeyCodeVerb.event(["code:21", "mods:cmd"]))
    let b = ChordKit.chord(try KeyCodeVerb.event(["code:21", "mods:cmd"]))
    #expect(a == "⌘4")
    #expect(a == b)                                         // no state carries from one press to the next
}
#endif
