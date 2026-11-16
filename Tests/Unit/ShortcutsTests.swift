import Foundation
import Testing
@testable import WhooshUI

// Written from docs/reference/superhuman-shortcuts-v8.json, docs/CONSTANTS.md (SEQUENCE_WAIT_MS) and the fixture,
// never from a built screen. The key map is read from the JSON: there is no hand-typed list of keys here.
// Run with WHOOSH_FIXTURE_CORE=1 so the shared AppModel serves the design fixture.
// MUTANTS: each test names, in its trailing comment, the mutation of the target that must turn it red.

@MainActor private func model() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    setenv("WHOOSH_FIXTURE_PATH", repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()   // launch state once per test (Tests/Support/SharedAppFresh.swift)
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    return m
}
private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()

private func json(_ path: String) throws -> [String: Any] {
    let data = try Data(contentsOf: repoRoot.appendingPathComponent(path))
    return try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
}

// MARK: v8 chord text -> the chord ChordKit writes (⌃ ⌥ ⇧ ⌘ order, symbols shifted, named keys by name)

private struct V8Entry { let action: String, group: String, chords: [String], sequence: (String, String)? }

private func chord(_ token: String) -> String {
    var t = token.trimmingCharacters(in: .whitespaces)
    if t.lowercased().hasPrefix("ctrl ") { t = "⌃" + t.dropFirst(5).trimmingCharacters(in: .whitespaces) }
    var mods = ""
    while let f = t.first, "⌃⌥⇧⌘".contains(f) { mods.append(f); t.removeFirst() }
    let named = ["esc": "Escape", "enter": "Enter", "tab": "Tab", "space": "Space", "↑": "Up", "↓": "Down", "←": "Left", "→": "Right"]
    var key = named[t.lowercased()] ?? t
    if key == "Tab", mods == "⇧" { return "Shift+Tab" }
    if key.count == 1, key.first?.isLetter == true { key = key.uppercased() }
    if mods.isEmpty, "!#?".contains(key) { mods = "⇧" }
    return "⌃⌥⇧⌘".filter { mods.contains($0) } + key
}

private func v8() throws -> [V8Entry] {
    let entries = try #require(try json("docs/reference/superhuman-shortcuts-v8.json")["entries"] as? [[String: Any]])
    return entries.map { e in
        let text = (e["chord"] as? String ?? "").trimmingCharacters(in: .whitespaces)
        let base = (action: e["action"] as? String ?? "", group: e["group"] as? String ?? "")
        if text.hasPrefix(":") { return V8Entry(action: base.action, group: base.group, chords: [], sequence: nil) }
        if text.contains(" then ") {
            let p = text.components(separatedBy: " then ")
            return V8Entry(action: base.action, group: base.group, chords: [], sequence: (chord(p[0]), chord(p[1])))
        }
        let arrows = text.split(separator: " ").map(String.init)
        if !arrows.isEmpty, arrows.allSatisfy({ "↑↓←→".contains($0) }) {
            return V8Entry(action: base.action, group: base.group, chords: arrows.map(chord), sequence: nil)
        }
        if text.hasSuffix("1-9") {
            var prefix = String(text.dropLast(3)).trimmingCharacters(in: .whitespaces)
            if prefix.lowercased() == "ctrl" { prefix = "⌃" }
            return V8Entry(action: base.action, group: base.group, chords: (1...9).map { chord(prefix + String($0)) }, sequence: nil)
        }
        return V8Entry(action: base.action, group: base.group, chords: text.components(separatedBy: " / ").map(chord), sequence: nil)
    }
}

/// A fresh engine on the Superhuman preset with a clock the test moves.
@MainActor private func engine(clock: Clock) -> ShortcutEngine {
    let e = ShortcutEngine()
    e.now = { clock.t }
    return e
}
private final class Clock: @unchecked Sendable { var t: TimeInterval = 1000 }

// MARK: tests

@MainActor @Test func everyKeyOfTheV8ListIsBoundInTheSuperhumanPreset() throws {
    let preset = Presets.superhuman
    var checked = 0
    for e in try v8() {
        for c in e.chords {
            #expect(!preset.commands(forChord: c).isEmpty, "\(e.action): \(c) is bound to nothing")   // mutant: a wrong chord in the preset
            #expect(Registrations.key(c) != nil, "\(e.action): \(c) reaches no handler")             // mutant: a chord left out of knownChords
            checked += 1
        }
        if let s = e.sequence {
            #expect(preset.sequences.contains { $0.first == s.0 && $0.second == s.1 }, "\(e.action): \(s.0) then \(s.1) is not a sequence")   // mutant: a sequence removed
            checked += 1
        }
    }
    #expect(checked >= 110)
}

@MainActor @Test func theSuperhumanPresetBindsNothingElseButTheDeclaredExtras() throws {
    let listed = Set(try v8().flatMap(\.chords) + [ "/", "⇧?" ])   // "/" is a v8 entry; "?" opens the search (owner's card answer)
    // ⌘J (write a reply draft) is a declared extra: the owner approved AI replies with ⌘J in a thread (docs/JOURNEYS.md J10; Commands.swift:59; checks/superhuman-v8-check.py EXTRAS).
    let extras: Set<String> = ["⌘P", "⇧P", "⌘\\", "⌘J"]
    let rows = Presets.superhuman.bindings.map(\.chord) + Presets.superhuman.alsoKeys.map(\.chord) + Presets.superhuman.contextKeys.map(\.chord)
    #expect(Set(rows).subtracting(listed).subtracting(extras).isEmpty, "bound but not in v8: \(Set(rows).subtracting(listed).subtracting(extras))")   // mutant: A or M back in the preset
    for gone in ["A", "M"] { #expect(Presets.superhuman.commands(forChord: gone).isEmpty, "\(gone) must be gone") }   // ⌘J left this list: bound to "ai-reply" on purpose (owner approval, J10)
    #expect(Presets.superhuman.commands(forChord: "⌘J").contains("ai-reply"), "⌘J writes the reply draft")   // mutant: ⌘J unbound
    #expect(Presets.superhuman.duplicates.isEmpty)       // mutant: one chord bound twice
    #expect(Presets.superhuman.unregistered.isEmpty)     // mutant: a command id with no title
}

@MainActor @Test func comingSoonFollowsTheOwnersFiveFeatures() throws {
    for e in try v8() {
        let soon = ["calendar", "tabs"].contains(e.group)
        for c in e.chords {
            let kinds = Presets.superhuman.commands(forChord: c).map { Commands.command($0)?.comingSoon != nil }
            #expect(soon ? kinds.contains(true) : kinds.contains(false), "\(e.action): \(c) has the wrong status")   // mutant: a calendar key marked live
        }
        if let s = e.sequence, let q = Presets.superhuman.sequences.first(where: { $0.first == s.0 && $0.second == s.1 }) {
            let isSoon = Commands.command(q.command)?.comingSoon != nil
            #expect(isSoon == (soon || ["O", ";"].contains(s.1) && s.0 == "G"), "\(q.label) has the wrong status")   // mutant: G then O live
        }
    }
}

@MainActor @Test func theFixtureMapIsTheSuperhumanRows() throws {
    let map = try #require(try json("docs/design/fixtures/mac-main-sample.json")["shortcuts"] as? [String: String])
    let rows = Dictionary(uniqueKeysWithValues: Presets.superhuman.bindings.map { ($0.chord, Commands.title($0.command) ?? "") })
    #expect(map == rows)                                 // mutant: a row retitled or removed without regenerating the fixture
}

@MainActor @Test func aSequenceFiresOnItsSecondKeyNotItsFirst() throws {
    let m = try model(), clock = Clock(), e = engine(clock: clock)
    _ = e.press("G", m)
    #expect(!e.log.contains { $0.command == "go-inbox" })   // mutant: a sequence that fires on the first key
    #expect(e.log.last?.outcome == "armed")
    clock.t += 0.2
    _ = e.press("I", m)
    #expect(e.log.last?.command == "go-inbox")              // mutant: the second key ignored
}

@MainActor @Test func aSequenceLapsesAfterTheWait() throws {
    let m = try model(), clock = Clock(), e = engine(clock: clock)
    _ = e.press("G", m)
    clock.t += Double(WhooshConstants.SEQUENCE_WAIT_MS) / 1000 + 0.1
    _ = e.press("I", m)
    #expect(!e.log.contains { $0.command == "go-inbox" })   // mutant: no wait limit
    #expect(e.log.contains { $0.outcome == "lapsed" })
}

@MainActor @Test func aSecondKeyOfASequenceNeverLeaksToTheSingleKey() throws {
    let m = try model(), clock = Clock(), e = engine(clock: clock)
    let before = e.keyUses("star")
    _ = e.press("G", m)
    #expect(e.press("S", m))                                // G then S is "go to starred", not star
    #expect(e.keyUses("star") == before)                    // mutant: a matched sequence returns false and the key goes on
}

@MainActor @Test func zeroThenZeroIsTheWeek() throws {
    let m = try model(), clock = Clock(), e = engine(clock: clock)
    _ = e.press("0", m)
    _ = e.press("0", m)
    #expect(e.log.last?.command == "cal-week")
    #expect(e.notice == "Calendar is coming soon.")
}

@MainActor @Test func enterRepliesFromTheListAsRDoesAndRepliesAllInAThread() throws {
    let m = try model(), e = engine(clock: Clock())
    m.surface = "mac-main"; m.overlay = nil
    _ = e.press("Enter", m)
    #expect(e.log.last?.resolved == "reply")                // A102: the list's Enter is R's act; mutant: Enter opens only, or always replies all
    m.surface = "mac-reader"; m.overlay = nil
    _ = e.press("Enter", m)
    #expect(e.log.last?.resolved == "reply-all")            // mutant: Enter always opens
    m.surface = "mac-main"; m.overlay = nil
}

@MainActor @Test func aComingSoonKeyShowsItsLineAndPassesTheKeyOn() throws {
    let m = try model(), e = engine(clock: Clock())
    #expect(!e.press("⌘T", m))
    #expect(e.notice == "Tabs is coming soon.")             // mutant: the notice dropped
    #expect(e.log.last?.outcome == "notice")
    _ = e.press("J", m)
    #expect(e.notice == nil)                                // the next key clears it
}

@MainActor @Test func controlDigitsSwitchAccounts() throws {
    let m = try model(), e = engine(clock: Clock())
    let accounts = m.core.accounts()
    try #require(accounts.count >= 2)
    #expect(e.press("⌃2", m))
    #expect(m.accountFilter == accounts[1].id)              // mutant: ⌃2 switches to the first account
    m.accountFilter = nil; m.refresh()
}

#if os(macOS)
// The real path of SPEC A99, from the key to the account: the socket's key-code variant builds the AZERTY event, ChordKit reads it,
// the engine runs the command. ⌃2 on an AZERTY keyboard is key code 19 typing "é".
@MainActor @Test func controlTwoOnAnAzertyKeyboardSwitchesToTheSecondAccount() throws {
    let m = try model()
    let accounts = m.core.accounts()
    try #require(accounts.count >= 2)
    m.accountFilter = nil; m.refresh()
    let reply = try KeyCodeVerb.press(m, ["code:19", "mods:ctrl"])
    #expect(reply["chord"] as? String == "⌃2")
    #expect(m.accountFilter == accounts[1].id)              // mutant: ChordKit reads the character ("⌃É"), so the key does nothing and the verb throws
    m.accountFilter = nil; m.refresh()
    do { _ = try KeyCodeVerb.press(m, ["code:19", "mods:cmd"]); Issue.record("⌘2 is coming soon: the key must be refused") }
    catch let f as ControlFailure { #expect(f.words.hasPrefix("the key ⌘2 (code 19) does nothing")) }   // the refusal names the resolved chord (par-keys reads it)
}
#endif

@MainActor @Test func anUnboundChordReachesTheEngineAndIsRefused() throws {
    let m = try model(), e = engine(clock: Clock())
    #expect(Registrations.key("Q") != nil)         // mutant: the key is left out of Presets.pressable, so a sequence cannot end on it
    #expect(!e.press("Q", m))                      // mutant: an unbound key is taken
    #expect(e.log.isEmpty)
    #expect(Registrations.key("J") != nil)         // positive control
}

@MainActor @Test func aWrongSecondKeyDropsTheSequenceAndIsReadAsAFreshKey() throws {
    let m = try model(), clock = Clock(), e = engine(clock: clock)
    _ = e.press("G", m)
    #expect(!e.press("Q", m))
    #expect(e.log.last?.outcome == "cancelled")    // mutant: the wrong key leaves the sequence armed
    clock.t += 0.1
    _ = e.press("I", m)
    #expect(!e.log.contains { $0.command == "go-inbox" })   // I alone is no sequence
    _ = e.press("G", m)
    _ = e.press("J", m)                            // a bound key is a fresh key: it runs
    #expect(e.log.contains { $0.command == "next" })
}

@MainActor @Test func theArrowKeysAreInTheKeyMapOfEveryPreset() {
    for p in Presets.builtIn {
        let ways = p.ways.filter { $0.kind == "fixed" }
        #expect(ways.contains { $0.key == "Down" && $0.command == "next" }, "\(p.id): Down")   // mutant: a fixed key left out of the map
        #expect(ways.contains { $0.key == "Up" && $0.command == "previous" }, "\(p.id): Up")
    }
}

@MainActor @Test func twoFeaturesBindingOneKeyIsResolvedByTheRegistryNotByChance() {
    // Coverage of the rule in Registrations.key: a feature's binding wins over the base one, first feature in the registry wins.
    let hits = FeatureRegistry.all.filter { $0.keyBindings()["E"] != nil }.count
    var binders: [String] = []
    for f in FeatureRegistry.all where f.keyBindings()["E"] != nil { binders.append(f.id) }   // a loop: a key path over the existential crashed the compiler
    // E is bound by exactly three features, each in its own state (the owner's rule: shortcuts are contextual, one key one meaning per state):
    // Shortcuts (ShortcutsFeature.swift:15, every known chord, the engine decides), FirstStart (FirstStartFeature.swift:29, the step-3 try-it, only while
    // the flow runs: FirstStartModel.swift:189) and Notch (NotchFeature.swift:112, archives the open notch item, only while one is open). The registry
    // asks a feature holding the focus first and the rest in registry order, so a key never goes to two by chance. A fourth binder, or a changed registry
    // order, fails here and must be reviewed before it ships.
    #expect(binders == ["shortcuts", "firststart", "notch"], "features binding E, in registry order: \(binders)")   // mutant: InboxList also binds E, or the registry order changes
    #expect(hits == 3)
}

// lead F18 (main74): the table of TextKeys.swift, who gets a chord while a text view holds the focus.
@Test func aFocusedTextViewTakesItsEditingKeysFirst() {
    for c in ["⌘Z", "⇧⌘Z", "⌘A", "⌘X", "⌘C", "⌘V", "⌥⌘V"] { #expect(TextKeys.owns(c), "\(c)") }   // mutant: the six chords go to the engine
    for c in ["Q", "⇧Q", "Enter", "Tab", "Shift+Tab", "Left", "Down", "\u{7F}", "/", "⇧?"] { #expect(TextKeys.owns(c), "\(c)") }   // mutant: plain Return or "/" goes to the engine
    for c in ["⌘Left", "⇧⌘Right", "⌥Left", "⌥\u{7F}", "⌘\u{7F}", "⌃A", "⌃K"] { #expect(TextKeys.owns(c), "\(c)") }   // mutant: line and word moves go to the engine
}

@Test func aFocusedTextViewLeavesTheAppChordsToTheEngine() {
    for c in ["Escape", "⌘K", "⌘Enter", "⇧⌘Enter", "⌘,", "⌘N", "⌘1", "⌘9", "⌃1", "⌃9", "⌃Tab", "⌘B", "⇧⌘7", "⌘P"] { #expect(!TextKeys.owns(c), "\(c)") }   // mutant: the field takes every ⌘ chord
}
