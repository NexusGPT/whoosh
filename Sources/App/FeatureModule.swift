import SwiftUI

/// A row a list region exposes to the socket verb `rows <ax-id>`: JSON-ready fields.
public typealias ControlRow = [String: Any]

/// What a feature registers, from its own folder. Adding a feature never edits a hub: the feature's
/// stub type conforms to this protocol, and `FeatureRegistry.all` (written complete at the root
/// commit) lists every feature type. A lane fills its own type; every member has a default.
@MainActor
public protocol FeatureModule {
    /// the lane id in docs/lane-dag.json
    static var id: String { get }
    /// surfaces this feature draws (docs/DESIGN.md roster), e.g. "mac-main"
    static var surfaces: [String] { get }
    /// settings tabs this feature contributes
    static var settingsTabs: [String] { get }
    /// ax-id -> the rows that region shows (socket verb `rows`)
    static func rowSources() -> [String: (AppModel) throws -> [ControlRow]]
    /// key chord -> action; returns false when the key does nothing in the current state
    static func keyBindings() -> [String: (AppModel) -> Bool]
    /// ax-id -> click action (socket verb `click`, and the real control)
    static func clickTargets() -> [String: (AppModel) -> Bool]
    /// palette commands: (title, key hint or "")
    static func paletteCommands() -> [(title: String, key: String)]
    /// the view this feature draws in a shell slot, or nil when it does not claim that slot
    static func view(for slot: SurfaceSlot) -> AnyView?
    /// the list's zero state in words (`uistate.empty_state`), or nil while there are rows
    static func emptyState(_ m: AppModel) -> [String: Any]?
    /// true while this feature holds the focus (its surface or overlay is up, its field or pane is
    /// focused): it is asked first for every key, and a handler returning false passes the key on
    static func hasFocus(_ m: AppModel) -> Bool
    /// true while this feature holds the keyboard in a window of its own (the notch's panel is the key window): it is asked for every key
    /// BEFORE the features that only hold the focus, so a key pressed there never runs as a command of the screen behind
    static func holdsKeyboard(_ m: AppModel) -> Bool
    /// The socket's `type` and a paste: a field that owns its own text takes `text` while it holds the keys and
    /// returns true; false leaves it to the next feature, then to the shared buffer `AppModel.typed`
    static func typeText(_ text: String, _ m: AppModel) -> Bool
    /// The end of a `drag` from one ax-id onto another, handed to the screen's own drop function: true when this
    /// feature owns the pair and the drop is done (an already-done drop answers true and changes nothing)
    static func drop(from: String, onto: String, _ m: AppModel) -> Bool
}

public extension FeatureModule {
    static var surfaces: [String] { [] }
    static var settingsTabs: [String] { [] }
    static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] { [:] }
    static func keyBindings() -> [String: (AppModel) -> Bool] { [:] }
    static func clickTargets() -> [String: (AppModel) -> Bool] { [:] }
    static func paletteCommands() -> [(title: String, key: String)] { [] }
    static func view(for slot: SurfaceSlot) -> AnyView? { nil }
    static func emptyState(_ m: AppModel) -> [String: Any]? { nil }
    static func typeText(_ text: String, _ m: AppModel) -> Bool { false }
    static func holdsKeyboard(_ m: AppModel) -> Bool { false }
    static func drop(from: String, onto: String, _ m: AppModel) -> Bool { false }
    /// Default: the current surface (other than the main window) or overlay is one of this feature's.
    static func hasFocus(_ m: AppModel) -> Bool {
        (m.surface != SurfaceSlot.macMain.id && surfaces.contains(m.surface)) || m.overlay.map(surfaces.contains) == true
    }
}

/// Lookups over the registry: a feature's registration wins over the root's base one.
@MainActor
public enum Registrations {
    public static func rowSource(_ ax: String) -> ((AppModel) throws -> [ControlRow])? {
        for f in FeatureRegistry.all { if let s = f.rowSources()[ax] { return s } }
        return BaseFeature.rowSources()[ax]
    }
    /// Features in key order: a feature that holds the keyboard in its own window first, then those holding the focus, then the rest in
    /// registry order.
    public static func byFocus(_ m: AppModel) -> [any FeatureModule.Type] {
        let all = FeatureRegistry.all
        let rest = all.filter { !$0.holdsKeyboard(m) }
        return all.filter { $0.holdsKeyboard(m) } + rest.filter { $0.hasFocus(m) } + rest.filter { !$0.hasFocus(m) }
    }
    /// The key chain: every binding of the chord, focused features first, the root's base last; the
    /// first handler that returns true takes the key; one that
    /// returns false after an action failed (AppModel.lastFailure set) also ends the chain. Nil when nothing binds the chord at all.
    /// Which feature's key binding took the last key, for `rows mac-keyevent.monitor` (field `taker`): a key that changed nothing visible says who took it
    /// (lane hid-search round 10: J5.6's Esc in the all-journeys run left the reader with no trace of its taker).
    public static var lastKeyTaker = ""
    public static func key(_ chord: String) -> ((AppModel) -> Bool)? {
        let bound = FeatureRegistry.all.contains { $0.keyBindings()[chord] != nil } || BaseFeature.keyBindings()[chord] != nil
        guard bound else { return nil }
        return { m in
            for f in byFocus(m) {
                guard let k = f.keyBindings()[chord] else { continue }
                let before = m.failures
                if k(m) { lastKeyTaker = "\(chord) by \(f.id)"; return true }
                if m.failures != before { lastKeyTaker = "\(chord) refused by \(f.id)"; return false }   // it tried and failed: the key stops here, in words
            }
            let base = BaseFeature.keyBindings()[chord]?(m) ?? false
            lastKeyTaker = base ? "\(chord) by the shell's base" : "\(chord) by no key binding (the shortcut commands next)"
            return base
        }
    }
    /// Every feature's key table is read once, before the first key is handled: a feature that installs a hook of the key monitor with its table
    /// (the reply's, the pickers', the composer's) has it in place for that first key. Until 13 Nov 2026 a table was first read when a key reached
    /// its feature, and a key the shortcuts took first never did: the first letters after L ran list commands on the mail (key trace on main156).
    public static func primeKeyTables() {
        guard !primed else { return }
        primed = true
        for f in FeatureRegistry.all { _ = f.keyBindings() }
    }
    private static var primed = false

    /// `type <text>`: the focused features first; the first field that owns its text takes it, else the shared buffer.
    public static func type(_ text: String, _ m: AppModel) {
        let shared = ProcessInfo.processInfo.environment["WHOOSH_TYPE_SHARED"] == "1"   // the mutant: one buffer for every field, as before round 3
        for f in byFocus(m) where !shared && f.typeText(text, m) { return }
        m.typed += text
    }
    /// `drag`'s end: the first feature that owns the pair takes the drop. Nil when none does.
    public static func drop(from a: String, onto b: String, _ m: AppModel) -> String? {
        guard ProcessInfo.processInfo.environment["WHOOSH_DROP_OFF"] != "1" else { return nil }   // the mutant: no screen takes a drop
        return FeatureRegistry.all.first { $0.drop(from: a, onto: b, m) }?.id
    }
    public static func click(_ ax: String) -> ((AppModel) -> Bool)? {
        for f in FeatureRegistry.all { if let c = f.clickTargets()[ax] { return c } }
        return BaseFeature.clickTargets()[ax]
    }
    /// The first feature that claims the slot draws it; nil leaves the shell's stub in place.
    public static func view(for slot: SurfaceSlot) -> AnyView? {
        for f in FeatureRegistry.all { if let v = f.view(for: slot) { return v } }
        return nil
    }
    public static func emptyState(_ m: AppModel) -> [String: Any]? {
        for f in FeatureRegistry.all { if let e = f.emptyState(m) { return e } }
        return nil
    }
    /// Every surface id a feature (or the shell) registers, sorted.
    public static var surfaces: [String] {
        Array(Set(FeatureRegistry.all.flatMap { $0.surfaces } + BaseFeature.surfaces)).sorted()
    }
    public static var rowRegions: [String] {
        Array(Set(FeatureRegistry.all.flatMap { $0.rowSources().keys } + BaseFeature.rowSources().keys)).sorted()
    }
}
